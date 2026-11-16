use super::{Store, decode, encode, params, storage};
use kanlo_core::{Error, Result, Workspace};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Sprint,
    Milestone,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub kind: Kind,
    pub title: String,
    pub start: Option<String>,
    pub end: String,
    #[serde(default)]
    pub milestone_id: Option<String>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Entry {
    pub id: String,
    #[serde(flatten)]
    pub draft: Draft,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub ticket_id: String,
    pub entry_id: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlannedTicket {
    pub id: String,
    pub public_id: String,
    pub title: String,
    pub in_backlog: bool,
    pub entry_id: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlanningLabel {
    pub sprint: Option<String>,
    pub milestone: Option<String>,
}

fn get_entry(connection: &Connection, id: &str) -> Result<Entry> {
    let body: String = connection
        .query_row(
            "SELECT body FROM calendar_entries WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or(Error::NotFound)?;
    decode(&body)
}

pub(crate) fn keep_sprints_in_backlog(
    connection: &Connection,
    workspace: &Workspace,
) -> Result<()> {
    let mut query = connection
        .prepare(
            "SELECT p.ticket_id FROM planning_tickets p JOIN calendar_entries e ON e.id=p.entry_id
         WHERE json_extract(e.body, '$.kind')='sprint'",
        )
        .map_err(storage)?;
    let mut assigned = query
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .peekable();
    if assigned.peek().is_none() {
        return Ok(());
    }
    let outside: std::collections::BTreeSet<_> = workspace
        .tickets
        .iter()
        .filter(|ticket| {
            !ticket.details.trashed && !ticket.details.archived && ticket.column_id.is_some()
        })
        .map(|ticket| ticket.id.as_str())
        .collect();
    for id in assigned {
        if outside.contains(id.map_err(storage)?.as_str()) {
            return Err(Error::Conflict(
                "Sprint-Tickets bleiben im Backlog. Entferne zuerst die Sprint-Zuordnung.".into(),
            ));
        }
    }
    Ok(())
}

pub fn valid_day(value: &str) -> bool {
    value.len() == 10
        && chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .is_ok_and(|day| day.format("%Y-%m-%d").to_string() == value)
}

impl Store {
    pub fn planning_labels(
        &self,
        workspace: &Workspace,
    ) -> Result<BTreeMap<String, PlanningLabel>> {
        let entries: BTreeMap<_, _> = self
            .calendar()?
            .into_iter()
            .map(|entry| (entry.id, entry.draft))
            .collect();
        let mut query = self
            .connection
            .prepare("SELECT entry_id FROM planning_tickets WHERE ticket_id=?1")
            .map_err(storage)?;
        let mut labels = BTreeMap::new();
        for ticket in &workspace.tickets {
            let id = ticket.details.parent_id.as_deref().unwrap_or(&ticket.id);
            let entry_id: Option<String> = query
                .query_row([id], |row| row.get(0))
                .optional()
                .map_err(storage)?;
            if let Some(entry) = entry_id.as_ref().and_then(|id| entries.get(id)) {
                let label = match entry.kind {
                    Kind::Sprint => PlanningLabel {
                        sprint: Some(entry.title.clone()),
                        milestone: entry
                            .milestone_id
                            .as_ref()
                            .and_then(|id| entries.get(id))
                            .map(|entry| entry.title.clone()),
                    },
                    Kind::Milestone => PlanningLabel {
                        sprint: None,
                        milestone: Some(entry.title.clone()),
                    },
                };
                labels.insert(ticket.id.clone(), label);
            }
        }
        Ok(labels)
    }

    pub fn calendar(&self) -> Result<Vec<Entry>> {
        let mut statement = self
            .connection
            .prepare("SELECT body FROM calendar_entries")
            .map_err(storage)?;
        let records = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage)?;
        let mut entries: Vec<Entry> = records
            .map(|row| decode(&row.map_err(storage)?))
            .collect::<Result<_>>()?;
        entries.sort_by(|a, b| (&a.draft.end, &a.id).cmp(&(&b.draft.end, &b.id)));
        Ok(entries)
    }

    pub fn add_calendar_entry(&mut self, mut draft: Draft) -> Result<Entry> {
        draft.title = draft.title.trim().to_owned();
        if draft.title.is_empty()
            || draft.title.chars().count() > 160
            || !valid_day(&draft.end)
            || match (&draft.kind, &draft.start) {
                (Kind::Sprint, Some(start)) => !valid_day(start) || start > &draft.end,
                (Kind::Milestone, None) => false,
                _ => true,
            }
        {
            return Err(Error::Validation("Invalid calendar title or dates".into()));
        }
        if let Some(id) = &draft.milestone_id
            && (draft.kind != Kind::Sprint
                || get_entry(&self.connection, id)?.draft.kind != Kind::Milestone)
        {
            return Err(Error::Validation(
                "Only sprints can belong to a milestone".into(),
            ));
        }
        let entry = Entry {
            id: Uuid::new_v4().to_string(),
            draft,
        };
        self.connection
            .execute(
                "INSERT INTO calendar_entries (id, body) VALUES (?1, ?2)",
                params![entry.id, encode(&entry)?],
            )
            .map_err(storage)?;
        Ok(entry)
    }

    pub fn planning_tickets(&self) -> Result<Vec<PlannedTicket>> {
        let mut query = self
            .connection
            .prepare("SELECT ticket_id, entry_id FROM planning_tickets")
            .map_err(storage)?;
        let mut assignments: BTreeMap<String, String> = query
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)?;
        Ok(self
            .workspace()?
            .tickets
            .into_iter()
            .filter(|ticket| {
                !ticket.details.archived
                    && !ticket.details.trashed
                    && ticket.details.parent_id.is_none()
            })
            .map(|ticket| PlannedTicket {
                entry_id: assignments.remove(&ticket.id),
                id: ticket.id,
                public_id: ticket.public_id,
                title: ticket.title,
                in_backlog: ticket.column_id.is_none(),
            })
            .collect())
    }

    pub fn assign_planning_ticket(&mut self, assignment: Assignment) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        super::require_writable(&tx)?;
        let workspace = super::records::load(&tx)?;
        let ticket = workspace.find_ticket(&assignment.ticket_id)?;
        if let Some(entry_id) = &assignment.entry_id {
            if ticket.details.archived
                || ticket.details.trashed
                || ticket.details.parent_id.is_some()
            {
                return Err(Error::NotFound);
            }
            if get_entry(&tx, entry_id)?.draft.kind == Kind::Sprint && ticket.column_id.is_some() {
                return Err(Error::Conflict(
                    "Verschiebe das Ticket zuerst ins Backlog.".into(),
                ));
            }
            tx.execute(
                "INSERT INTO planning_tickets(ticket_id, entry_id) VALUES (?1, ?2)
                ON CONFLICT(ticket_id) DO UPDATE SET entry_id=excluded.entry_id",
                params![assignment.ticket_id, entry_id],
            )
            .map_err(storage)?;
        } else {
            tx.execute(
                "DELETE FROM planning_tickets WHERE ticket_id=?1",
                [&assignment.ticket_id],
            )
            .map_err(storage)?;
        }
        tx.execute(
            "UPDATE metadata SET value=CAST(CAST(value AS INTEGER)+1 AS TEXT) WHERE key='revision'",
            [],
        )
        .map_err(storage)?;
        tx.execute("INSERT INTO event_stream(payload,created_at) VALUES (?1,?2)", params![
            encode(&serde_json::json!({"revision":super::revision(&tx)?, "boards":[ticket.board_id], "affected":[ticket.id]}))?,
            chrono::Utc::now().to_rfc3339(),
        ]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(())
    }
}
