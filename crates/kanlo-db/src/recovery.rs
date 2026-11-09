//! Explicit data-only packages. Authentication tables are deliberately outside this allowlist.
use super::*;

mod import;
pub use import::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalReference {
    pub id: String,
    pub public_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardExport {
    pub schema_version: u32,
    pub exported_at: String,
    pub workspace: Workspace,
    pub external_references: Vec<ExternalReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryGroup {
    pub sequence: i64,
    pub description: String,
    pub changes: String,
    pub created_at: String,
    pub boards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticSample {
    pub board_id: String,
    pub sample: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    pub schema_version: u32,
    pub created_at: String,
    pub workspace: Workspace,
    pub history: Vec<HistoryGroup>,
    pub statistics: Vec<StatisticSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPreview {
    pub token: String,
    pub revision: i64,
    pub purpose: String,
    pub summary: Value,
    pub expires_at: String,
}

pub fn parse_package<T: serde::de::DeserializeOwned>(input: &str) -> Result<T> {
    if input.len() > 32 * 1024 * 1024 {
        return Err(Error::Validation("Package exceeds 32 MiB".into()));
    }
    serde_json::from_str(input)
        .map_err(|_| Error::Validation("Invalid or unsupported data package".into()))
}

impl Store {
    pub fn export_board(&self, board_id: &str) -> Result<BoardExport> {
        let source = self.workspace()?;
        if !source.boards.iter().any(|b| b.id == board_id) {
            return Err(Error::NotFound);
        }
        let mut state = source.clone();
        state.boards.retain(|b| b.id == board_id);
        state.columns.retain(|c| c.board_id == board_id);
        state.tickets.retain(|t| t.board_id == board_id);
        let ids: BTreeSet<_> = state.tickets.iter().map(|t| t.id.clone()).collect();
        state.comments.retain(|c| ids.contains(&c.ticket_id));
        state.dependencies.retain(|d| ids.contains(&d.ticket_id));
        state.links.retain(|l| ids.contains(&l.ticket_id));
        state.recurrences.retain(|r| ids.contains(&r.template_id));
        let used_types: BTreeSet<_> = state
            .tickets
            .iter()
            .filter_map(|t| t.details.type_id.clone())
            .chain(
                state
                    .board_options
                    .get(board_id)
                    .into_iter()
                    .flat_map(|o| o.enabled_types.iter().cloned()),
            )
            .collect();
        state.types.retain(|t| used_types.contains(&t.id));
        let used_labels: BTreeSet<_> = state
            .tickets
            .iter()
            .flat_map(|t| t.details.labels.iter().cloned())
            .collect();
        state.labels.retain(|l| used_labels.contains(&l.id));
        state.board_options.retain(|id, _| id == board_id);
        state.views.clear();
        state.settings = Settings::default();
        state.issued_prefixes = state.boards.iter().map(|b| b.prefix.clone()).collect();
        state.issued_ticket_ids = state.tickets.iter().map(|t| t.public_id.clone()).collect();
        let external: BTreeSet<_> = state
            .dependencies
            .iter()
            .map(|d| d.prerequisite_id.clone())
            .chain(state.links.iter().map(|l| l.target_id.clone()))
            .filter(|id| !ids.contains(id))
            .collect();
        let external_references = external
            .into_iter()
            .map(|id| {
                source.find_ticket(&id).map(|t| ExternalReference {
                    id: t.id.clone(),
                    public_id: t.public_id.clone(),
                })
            })
            .collect::<Result<_>>()?;
        Ok(BoardExport {
            schema_version: 1,
            exported_at: engine::now(),
            workspace: state,
            external_references,
        })
    }

    pub fn backup(&mut self) -> Result<Backup> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let backup = make_backup(&tx)?;
        tx.commit().map_err(storage)?;
        Ok(backup)
    }

    pub fn restore_preview(
        &mut self,
        backup: Backup,
        boards: Option<Vec<String>>,
    ) -> Result<RecoveryPreview> {
        if backup.schema_version != 1 {
            return Err(Error::Validation("Unsupported backup schema".into()));
        }
        backup.workspace.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let before = records::load(&tx)?;
        let mut candidate = if let Some(selected) = &boards {
            selective_restore(&before, &backup.workspace, selected)?
        } else {
            backup.workspace.clone()
        };
        preserve_registry(&before, &mut candidate);
        candidate.validate()?;
        let summary = json!({"boards":candidate.boards.len(),"tickets":candidate.tickets.len(),"replaced_boards":boards,"epoch_will_change":true,"safety_backup_before_overwrite":true});
        let preview = save_preview(
            &tx,
            "restore",
            &candidate,
            &json!({"backup":backup,"boards":boards}),
            summary,
        )?;
        tx.commit().map_err(storage)?;
        Ok(preview)
    }

    pub fn recovery_confirm(
        &mut self,
        token: &str,
        op_id: &str,
        client_id: &str,
    ) -> Result<Receipt> {
        validate_operation_id(op_id, client_id)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let request = encode(&json!({"recovery_confirm":token}))?;
        if let Some(receipt) = receipt(&tx, op_id, client_id, &request)? {
            return Ok(receipt);
        }
        let row:Option<(i64,String,String,String,String)>=tx.query_row("SELECT revision,purpose,candidate,payload,created_at FROM recovery_previews WHERE token=?1",[token],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(storage)?;
        let (base, purpose, candidate, payload, created) = row.ok_or(Error::NotFound)?;
        let created = chrono::DateTime::parse_from_rfc3339(&created).map_err(|_| Error::Storage)?;
        if base != revision(&tx)?
            || chrono::Utc::now()
                .signed_duration_since(created)
                .num_seconds()
                > 300
        {
            return Err(Error::Conflict("Preview expired or state changed".into()));
        }
        let before = records::load(&tx)?;
        let mut after: Workspace = decode(&candidate)?;
        preserve_registry(&before, &mut after);
        after.validate()?;
        let old = records::to_records(&before)?;
        let new = records::to_records(&after)?;
        let mut result = ChangeResult::default();
        for key in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
            if old.get(key) != new.get(key) {
                if key.0 == "ticket" {
                    result.affected.insert(key.1.clone());
                }
                if let Some(board) = new
                    .get(key)
                    .or_else(|| old.get(key))
                    .and_then(|r| r.board_id.clone())
                {
                    result.boards.insert(board);
                }
            }
        }
        if purpose == "restore" {
            let safety = make_backup(&tx)?;
            tx.execute(
                "INSERT INTO restore_safety_snapshots VALUES (?1,?2,?3)",
                params![Uuid::new_v4().to_string(), encode(&safety)?, engine::now()],
            )
            .map_err(storage)?;
        }
        commit_state(
            &tx,
            &before,
            &after,
            &result,
            &purpose,
            purpose != "restore",
        )?;
        if purpose == "restore" {
            let payload: Value = decode(&payload)?;
            let backup: Backup =
                serde_json::from_value(payload["backup"].clone()).map_err(|_| Error::Storage)?;
            let selection: Option<Vec<String>> =
                serde_json::from_value(payload["boards"].clone()).map_err(|_| Error::Storage)?;
            if let Some(boards) = selection {
                for board in boards {
                    tx.execute("DELETE FROM undo_groups WHERE sequence IN (SELECT sequence FROM undo_boards WHERE board_id=?1)",[&board]).map_err(storage)?;
                    tx.execute("DELETE FROM statistics_samples WHERE board_id=?1", [&board])
                        .map_err(storage)?;
                    for sample in backup.statistics.iter().filter(|s| s.board_id == board) {
                        tx.execute("INSERT INTO statistics_samples(board_id,sample,created_at) VALUES (?1,?2,?3)",params![sample.board_id,encode(&sample.sample)?,sample.created_at]).map_err(storage)?;
                    }
                }
            } else {
                tx.execute("DELETE FROM undo_groups", []).map_err(storage)?;
                tx.execute("DELETE FROM statistics_samples", [])
                    .map_err(storage)?;
                for group in backup.history {
                    tx.execute("INSERT INTO undo_groups(sequence,description,changes,created_at) VALUES (?1,?2,?3,?4)",params![group.sequence,group.description,group.changes,group.created_at]).map_err(storage)?;
                    for board in group.boards {
                        tx.execute(
                            "INSERT INTO undo_boards VALUES (?1,?2)",
                            params![group.sequence, board],
                        )
                        .map_err(storage)?;
                    }
                }
                for sample in backup.statistics {
                    tx.execute("INSERT INTO statistics_samples(board_id,sample,created_at) VALUES (?1,?2,?3)",params![sample.board_id,encode(&sample.sample)?,sample.created_at]).map_err(storage)?;
                }
            }
            tx.execute(
                "UPDATE metadata SET value=?1 WHERE key='epoch'",
                [Uuid::new_v4().to_string()],
            )
            .map_err(storage)?;
            tx.execute("DELETE FROM receipts", []).map_err(storage)?;
            tx.execute("DELETE FROM external_sessions", [])
                .map_err(storage)?;
            tx.execute("DELETE FROM api_keys", []).map_err(storage)?;
            tx.execute("DELETE FROM temp_mappings", [])
                .map_err(storage)?;
        }
        tx.execute("DELETE FROM recovery_previews WHERE token=?1", [token])
            .map_err(storage)?;
        let response = Receipt {
            op_id: op_id.into(),
            revision: revision(&tx)?,
            result,
            temp_ids: BTreeMap::new(),
        };
        save_receipt(&tx, op_id, client_id, &request, &response)?;
        tx.commit().map_err(storage)?;
        Ok(response)
    }

    pub fn safety_snapshots(&self) -> Result<Vec<Value>> {
        let mut query = self
            .connection
            .prepare("SELECT id,created_at FROM restore_safety_snapshots ORDER BY created_at DESC")
            .map_err(storage)?;
        query
            .query_map([], |r| {
                Ok(json!({"id":r.get::<_,String>(0)?,"created_at":r.get::<_,String>(1)?}))
            })
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }
    pub fn safety_snapshot(&self, id: &str) -> Result<Backup> {
        let body: String = self
            .connection
            .query_row(
                "SELECT payload FROM restore_safety_snapshots WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
        decode(&body)
    }
}

fn make_backup(connection: &Connection) -> Result<Backup> {
    let workspace = records::load(connection)?;
    let mut query = connection
        .prepare(
            "SELECT sequence,description,changes,created_at FROM undo_groups ORDER BY sequence",
        )
        .map_err(storage)?;
    let groups = query
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    let mut history = vec![];
    for (sequence, description, changes, created_at) in groups {
        let mut query = connection
            .prepare("SELECT board_id FROM undo_boards WHERE sequence=?1")
            .map_err(storage)?;
        let boards = query
            .query_map([sequence], |r| r.get(0))
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)?;
        history.push(HistoryGroup {
            sequence,
            description,
            changes,
            created_at,
            boards,
        });
    }
    let mut query = connection
        .prepare("SELECT board_id,sample,created_at FROM statistics_samples ORDER BY sequence")
        .map_err(storage)?;
    let rows = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(storage)?;
    let statistics = rows
        .map(|r| {
            let (board_id, sample, created_at) = r.map_err(storage)?;
            Ok(StatisticSample {
                board_id,
                sample: decode(&sample)?,
                created_at,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Backup {
        schema_version: 1,
        created_at: engine::now(),
        workspace,
        history,
        statistics,
    })
}

fn save_preview(
    tx: &Transaction<'_>,
    purpose: &str,
    candidate: &Workspace,
    payload: &Value,
    summary: Value,
) -> Result<RecoveryPreview> {
    let token = Uuid::new_v4().to_string();
    let revision = revision(tx)?;
    tx.execute(
        "INSERT INTO recovery_previews VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            token,
            revision,
            purpose,
            encode(candidate)?,
            encode(payload)?,
            engine::now()
        ],
    )
    .map_err(storage)?;
    tx.execute(
        "DELETE FROM recovery_previews WHERE created_at<?1",
        [(chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339()],
    )
    .map_err(storage)?;
    Ok(RecoveryPreview {
        token,
        revision,
        purpose: purpose.into(),
        summary,
        expires_at: (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
    })
}

fn preserve_registry(before: &Workspace, after: &mut Workspace) {
    after
        .issued_prefixes
        .extend(before.issued_prefixes.iter().cloned());
    after
        .issued_ticket_ids
        .extend(before.issued_ticket_ids.iter().cloned());
    for board in &mut after.boards {
        if let Some(old) = before.boards.iter().find(|b| b.prefix == board.prefix) {
            board.next_ticket_number = board.next_ticket_number.max(old.next_ticket_number);
        }
    }
}

fn selective_restore(
    current: &Workspace,
    source: &Workspace,
    boards: &[String],
) -> Result<Workspace> {
    if boards.is_empty() {
        return Err(Error::Validation("Select at least one board".into()));
    }
    let selected: BTreeSet<_> = boards.iter().cloned().collect();
    if selected
        .iter()
        .any(|id| !source.boards.iter().any(|b| &b.id == id))
    {
        return Err(Error::NotFound);
    }
    let mut result = current.clone();
    let old_ids: BTreeSet<_> = result
        .tickets
        .iter()
        .filter(|t| selected.contains(&t.board_id))
        .map(|t| t.id.clone())
        .collect();
    result.boards.retain(|b| !selected.contains(&b.id));
    result.columns.retain(|c| !selected.contains(&c.board_id));
    result.tickets.retain(|t| !selected.contains(&t.board_id));
    result.comments.retain(|c| !old_ids.contains(&c.ticket_id));
    result
        .dependencies
        .retain(|d| !old_ids.contains(&d.ticket_id));
    result.links.retain(|l| !old_ids.contains(&l.ticket_id));
    result
        .recurrences
        .retain(|r| !old_ids.contains(&r.template_id));
    result.boards.extend(
        source
            .boards
            .iter()
            .filter(|b| selected.contains(&b.id))
            .cloned(),
    );
    result.columns.extend(
        source
            .columns
            .iter()
            .filter(|c| selected.contains(&c.board_id))
            .cloned(),
    );
    result.tickets.extend(
        source
            .tickets
            .iter()
            .filter(|t| selected.contains(&t.board_id))
            .cloned(),
    );
    let ids: BTreeSet<_> = result
        .tickets
        .iter()
        .filter(|t| selected.contains(&t.board_id))
        .map(|t| t.id.clone())
        .collect();
    result.comments.extend(
        source
            .comments
            .iter()
            .filter(|c| ids.contains(&c.ticket_id))
            .cloned(),
    );
    result.dependencies.extend(
        source
            .dependencies
            .iter()
            .filter(|d| ids.contains(&d.ticket_id))
            .cloned(),
    );
    result.links.extend(
        source
            .links
            .iter()
            .filter(|l| ids.contains(&l.ticket_id))
            .cloned(),
    );
    result.recurrences.extend(
        source
            .recurrences
            .iter()
            .filter(|r| ids.contains(&r.template_id))
            .cloned(),
    );
    for board in &selected {
        if let Some(options) = source.board_options.get(board) {
            result.board_options.insert(board.clone(), options.clone());
        }
    }
    for kind in &source.types {
        if let Some(existing) = result.types.iter().find(|t| t.id == kind.id) {
            if encode(existing)? != encode(kind)?
                && result
                    .tickets
                    .iter()
                    .any(|t| ids.contains(&t.id) && t.details.type_id.as_ref() == Some(&kind.id))
            {
                return Err(Error::Conflict(
                    "Global type differs; resolve before selective restore".into(),
                ));
            }
        } else {
            result.types.push(kind.clone());
        }
    }
    for label in &source.labels {
        if !result.labels.iter().any(|l| l.id == label.id) {
            result.labels.push(label.clone());
        }
    }
    result.views.retain(|_, v| !selected.contains(&v.board_id));
    result.validate()?;
    Ok(result)
}
