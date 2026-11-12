use crate::{Error, Result, Ticket, Workspace};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Board,
    Overview,
    Statistics,
}

#[derive(
    Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RotationEntry {
    pub view: View,
    pub board_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub view: View,
    pub board_id: Option<String>,
    pub rotation: Vec<RotationEntry>,
    pub rotation_seconds: u16,
    pub overlay_minutes: u16,
    pub statistics_days: u16,
    pub auto_scroll_seconds: u16,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            view: View::Board,
            board_id: None,
            rotation: vec![],
            rotation_seconds: 30,
            overlay_minutes: 5,
            statistics_days: 7,
            auto_scroll_seconds: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Overlay {
    Ticket { ticket_id: String },
    Backlog { board_id: String },
    Children { ticket_id: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CompanionTicket {
    pub ticket_id: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DisplayState {
    pub preferences: Preferences,
    pub overlays: Vec<Overlay>,
    pub companion_ticket: Option<CompanionTicket>,
    pub ticket_minutes: BTreeMap<String, u16>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revision: u64,
    pub highlights: Vec<String>,
    pub highlights_expire_at: Option<DateTime<Utc>>,
    pub scroll_offset: i64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Configure {
        preferences: Preferences,
        revision: u64,
    },
    Select {
        view: View,
        board_id: Option<String>,
    },
    ShowTicket {
        ticket_id: String,
        minutes: Option<u16>,
    },
    ShowBacklog {
        board_id: String,
    },
    ShowChildren {
        ticket_id: String,
    },
    Highlight {
        ticket_ids: Vec<String>,
        seconds: u16,
    },
    Scroll {
        delta: i16,
    },
    Resume {
        revision: u64,
    },
    Back,
    Close,
}

fn active<'a>(workspace: &'a Workspace, id: &str) -> Result<&'a Ticket> {
    let ticket = workspace.find_ticket(id)?;
    if ticket.details.archived || ticket.details.trashed {
        return Err(Error::NotFound);
    }
    Ok(ticket)
}

fn require_board(workspace: &Workspace, id: &str) -> Result<()> {
    if workspace.boards.iter().any(|board| board.id == id) {
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

impl DisplayState {
    fn deadline(&self, overlay: &Overlay, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let minutes = match overlay {
            Overlay::Ticket { ticket_id } => self.ticket_minutes.get(ticket_id).copied(),
            _ => None,
        }
        .unwrap_or(self.preferences.overlay_minutes);
        (minutes > 0).then(|| now + Duration::minutes(minutes.into()))
    }

    fn refresh_companion_deadline(&mut self, now: DateTime<Utc>) {
        let minutes = self.companion_ticket.as_ref().map(|ticket| {
            self.ticket_minutes
                .get(&ticket.ticket_id)
                .copied()
                .unwrap_or(self.preferences.overlay_minutes)
        });
        if let (Some(ticket), Some(minutes)) = (&mut self.companion_ticket, minutes) {
            ticket.expires_at = (minutes > 0).then(|| now + Duration::minutes(minutes.into()));
        }
    }

    fn prune_ticket_minutes(&mut self) {
        self.ticket_minutes.retain(|id, _| {
            self.companion_ticket
                .as_ref()
                .is_some_and(|ticket| &ticket.ticket_id == id)
                || self.overlays.iter().any(
                    |overlay| matches!(overlay, Overlay::Ticket { ticket_id } if ticket_id == id),
                )
        });
    }

    pub fn reconcile(&mut self, workspace: &Workspace, now: DateTime<Utc>) {
        self.highlights.retain(|id| active(workspace, id).is_ok());
        if self
            .highlights_expire_at
            .is_some_and(|expires| expires <= now)
        {
            self.highlights.clear();
        }
        if self.highlights.is_empty() {
            self.highlights_expire_at = None;
        }
        if self
            .preferences
            .board_id
            .as_deref()
            .is_some_and(|id| require_board(workspace, id).is_err())
        {
            self.preferences.board_id = None;
        }
        self.preferences.rotation.retain(|entry| {
            entry
                .board_id
                .as_deref()
                .is_none_or(|id| require_board(workspace, id).is_ok())
        });
        if self.companion_ticket.as_ref().is_some_and(|ticket| {
            active(workspace, &ticket.ticket_id).is_err()
                || (self.overlays.len() == 1 && ticket.expires_at.is_some_and(|end| end <= now))
        }) {
            self.companion_ticket = None;
        }
        while let Some(expires) = self.expires_at.filter(|expires| *expires <= now) {
            self.overlays.pop();
            if self.overlays.is_empty()
                && let Some(ticket) = self.companion_ticket.take()
            {
                self.overlays.push(Overlay::Ticket {
                    ticket_id: ticket.ticket_id,
                });
                self.expires_at = ticket.expires_at;
                continue;
            }
            self.expires_at = self
                .overlays
                .last()
                .and_then(|overlay| self.deadline(overlay, expires));
            if self.overlays.len() == 1 {
                self.refresh_companion_deadline(expires);
            }
        }
        if let Some(first_missing) = self.overlays.iter().position(|overlay| match overlay {
            Overlay::Ticket { ticket_id } | Overlay::Children { ticket_id } => {
                active(workspace, ticket_id).is_err()
            }
            Overlay::Backlog { board_id } => require_board(workspace, board_id).is_err(),
        }) {
            self.overlays.truncate(first_missing);
        }
        if self.overlays.is_empty()
            && let Some(ticket) = self.companion_ticket.take()
        {
            self.overlays.push(Overlay::Ticket {
                ticket_id: ticket.ticket_id,
            });
            self.expires_at = ticket.expires_at;
        }
        if self.overlays.is_empty() {
            self.expires_at = None;
            self.companion_ticket = None;
        }
        self.prune_ticket_minutes();
    }

    pub fn apply(
        &mut self,
        action: Action,
        workspace: &Workspace,
        now: DateTime<Utc>,
    ) -> Result<()> {
        self.reconcile(workspace, now);
        let was_nested = self.overlays.len() > 1;
        let mut reset_timer = true;
        match action {
            Action::Configure {
                preferences,
                revision,
            } => {
                if revision != self.revision {
                    return Err(Error::Conflict("Display settings changed".into()));
                }
                if !(5..=3600).contains(&preferences.rotation_seconds)
                    || preferences.overlay_minutes > 300
                    || !(1..=90).contains(&preferences.statistics_days)
                    || preferences.auto_scroll_seconds > 3600
                    || preferences.rotation.len() > 50
                    || preferences.rotation.iter().collect::<BTreeSet<_>>().len()
                        != preferences.rotation.len()
                {
                    return Err(Error::Validation(
                        "Invalid rotation or overlay settings".into(),
                    ));
                }
                if let Some(id) = &preferences.board_id {
                    require_board(workspace, id)?;
                }
                for entry in &preferences.rotation {
                    if let Some(id) = &entry.board_id {
                        if entry.view != View::Board {
                            return Err(Error::Validation(
                                "Only a board rotation entry takes a board ID".into(),
                            ));
                        }
                        require_board(workspace, id)?;
                    }
                }
                if preferences.board_id != self.preferences.board_id {
                    self.highlights.clear();
                    self.highlights_expire_at = None;
                }
                if preferences.view != self.preferences.view
                    || preferences.board_id != self.preferences.board_id
                {
                    self.overlays.clear();
                    self.companion_ticket = None;
                }
                self.preferences = preferences;
                self.refresh_companion_deadline(now);
            }
            Action::Select { view, board_id } => {
                if let Some(id) = &board_id {
                    require_board(workspace, id)?;
                }
                self.preferences.view = view;
                self.preferences.board_id = board_id;
                self.overlays.clear();
                self.companion_ticket = None;
                self.highlights.clear();
                self.highlights_expire_at = None;
            }
            Action::ShowBacklog { board_id } => {
                require_board(workspace, &board_id)?;
                if let Some(index) = self
                    .overlays
                    .iter()
                    .position(|overlay| matches!(overlay, Overlay::Backlog { .. }))
                {
                    self.overlays.truncate(index);
                }
                self.overlays.push(Overlay::Backlog { board_id });
            }
            Action::ShowChildren { ticket_id } => {
                let root = active(workspace, &ticket_id)?;
                if root.details.parent_id.is_some() {
                    return Err(Error::Validation("Select a parent ticket".into()));
                }
                let ticket_id = root.id.clone();
                if self.overlays.last()
                    != Some(&Overlay::Ticket {
                        ticket_id: ticket_id.clone(),
                    })
                {
                    self.preferences.view = View::Board;
                    self.preferences.board_id = Some(root.board_id.clone());
                    self.overlays = vec![Overlay::Ticket {
                        ticket_id: ticket_id.clone(),
                    }];
                    self.companion_ticket = None;
                }
                self.overlays.push(Overlay::Children { ticket_id });
            }
            Action::ShowTicket { ticket_id, minutes } => {
                if minutes.is_some_and(|value| value > 300) {
                    return Err(Error::Validation(
                        "Ticket overlay duration must be 0–300 minutes".into(),
                    ));
                }
                let ticket = active(workspace, &ticket_id)?;
                match self.overlays.last() {
                    Some(Overlay::Backlog { board_id })
                        if ticket.board_id == *board_id && ticket.column_id.is_none() => {}
                    Some(Overlay::Children { ticket_id: parent })
                        if ticket.details.parent_id.as_ref() == Some(parent) =>
                    {
                        self.overlays.pop();
                    }
                    Some(Overlay::Ticket {
                        ticket_id: previous,
                    }) if self.overlays.len() == 1
                        && active(workspace, previous)?.details.parent_id.is_none()
                        && ticket.details.parent_id.is_none() =>
                    {
                        if previous != &ticket.id {
                            self.companion_ticket = Some(CompanionTicket {
                                ticket_id: previous.clone(),
                                expires_at: self.expires_at,
                            });
                        }
                        self.preferences.view = View::Board;
                        self.preferences.board_id = Some(ticket.board_id.clone());
                        self.overlays.clear();
                    }
                    _ => {
                        self.preferences.view = View::Board;
                        self.preferences.board_id = Some(ticket.board_id.clone());
                        self.overlays.clear();
                        self.companion_ticket = None;
                    }
                }
                self.overlays.push(Overlay::Ticket {
                    ticket_id: ticket.id.clone(),
                });
                if let Some(minutes) = minutes {
                    self.ticket_minutes.insert(ticket.id.clone(), minutes);
                } else {
                    self.ticket_minutes.remove(&ticket.id);
                }
            }
            Action::Back => {
                self.overlays.pop();
                if self.overlays.is_empty()
                    && let Some(ticket) = self.companion_ticket.take()
                {
                    self.overlays.push(Overlay::Ticket {
                        ticket_id: ticket.ticket_id,
                    });
                    self.expires_at = ticket.expires_at;
                    reset_timer = false;
                }
            }
            Action::Close => {
                self.overlays.clear();
                self.companion_ticket = None;
            }
            Action::Highlight {
                ticket_ids,
                seconds,
            } => {
                if ticket_ids.len() > 20 || seconds > 3600 {
                    return Err(Error::Validation(
                        "Highlight at most 20 tickets for 0–3600 seconds".into(),
                    ));
                }
                let mut board_id = None;
                let mut ids = BTreeSet::new();
                for id in &ticket_ids {
                    let ticket = active(workspace, id)?;
                    if board_id
                        .as_ref()
                        .is_some_and(|board| board != &ticket.board_id)
                    {
                        return Err(Error::Validation(
                            "Highlighted tickets must be on the same board".into(),
                        ));
                    }
                    board_id = Some(ticket.board_id.clone());
                    ids.insert(ticket.id.clone());
                }
                if let Some(board_id) = board_id {
                    self.preferences.board_id = Some(board_id);
                    self.preferences.view = View::Board;
                    self.overlays.clear();
                    self.companion_ticket = None;
                } else {
                    reset_timer = false;
                }
                self.highlights = ids.into_iter().collect();
                self.highlights_expire_at = (!self.highlights.is_empty() && seconds > 0)
                    .then(|| now + Duration::seconds(seconds.into()));
            }
            Action::Scroll { delta } => {
                if delta == 0 || !(-20..=20).contains(&delta) {
                    return Err(Error::Validation(
                        "Scroll increment must be between -20 and 20".into(),
                    ));
                }
                self.scroll_offset = self
                    .scroll_offset
                    .checked_add(i64::from(delta))
                    .ok_or(Error::Storage)?;
                reset_timer = false;
            }
            Action::Resume { revision } => {
                if revision != self.revision {
                    return Err(Error::Conflict("Display context changed".into()));
                }
                self.refresh_companion_deadline(now);
            }
        }
        if reset_timer {
            self.expires_at = self
                .overlays
                .last()
                .and_then(|overlay| self.deadline(overlay, now));
        }
        if was_nested && self.overlays.len() == 1 {
            self.refresh_companion_deadline(now);
        }
        self.prune_ticket_minutes();
        self.revision = self.revision.checked_add(1).ok_or(Error::Storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    #[test]
    fn two_independent_tickets_expire_separately_and_third_replaces_oldest() {
        let now = DateTime::parse_from_rfc3339("2026-11-01T10:00:00Z")
            .unwrap()
            .to_utc();
        let mut workspace = Workspace::initial();
        let board_id = workspace.boards[0].id.clone();
        let tickets: Vec<_> = ["First", "Second", "Third"]
            .into_iter()
            .map(|title| {
                workspace
                    .apply(Command::CreateTicket {
                        board_id: board_id.clone(),
                        title: title.into(),
                        description: String::new(),
                        importance: 0,
                        column_id: None,
                        parent_id: None,
                        type_id: None,
                    })
                    .unwrap()
                    .entity_id
                    .unwrap()
            })
            .collect();
        let mut state = DisplayState::default();
        for (index, ticket_id) in tickets.iter().enumerate() {
            state
                .apply(
                    Action::ShowTicket {
                        ticket_id: ticket_id.clone(),
                        minutes: (index == 2).then_some(9),
                    },
                    &workspace,
                    now + Duration::minutes(index as i64),
                )
                .unwrap();
        }
        assert_eq!(
            state.companion_ticket.as_ref().unwrap().ticket_id,
            tickets[1]
        );
        assert_eq!(
            state.companion_ticket.as_ref().unwrap().expires_at,
            Some(now + Duration::minutes(6))
        );
        assert_eq!(state.expires_at, Some(now + Duration::minutes(11)));
        state.reconcile(&workspace, now + Duration::minutes(6));
        assert!(state.companion_ticket.is_none());
        assert_eq!(
            state.overlays,
            vec![Overlay::Ticket {
                ticket_id: tickets[2].clone()
            }]
        );
        state.reconcile(&workspace, now + Duration::minutes(11));
        assert!(state.overlays.is_empty());
    }

    #[test]
    fn timed_child_overlay_returns_to_parent_unless_context_changes() {
        let now = DateTime::parse_from_rfc3339("2026-11-01T10:00:00Z")
            .unwrap()
            .to_utc();
        let mut workspace = Workspace::initial();
        let board = workspace.boards[0].id.clone();
        let mut create = |title: &str, parent_id| {
            workspace
                .apply(Command::CreateTicket {
                    board_id: board.clone(),
                    title: title.into(),
                    description: String::new(),
                    importance: 0,
                    column_id: None,
                    parent_id,
                    type_id: None,
                })
                .unwrap()
                .entity_id
                .unwrap()
        };
        let parent = create("Parent", None);
        let child = create("Child", Some(parent.clone()));
        let independent = create("Independent", None);
        let mut state = DisplayState::default();
        state
            .apply(
                Action::ShowChildren {
                    ticket_id: parent.clone(),
                },
                &workspace,
                now,
            )
            .unwrap();
        state
            .apply(
                Action::ShowTicket {
                    ticket_id: child.clone(),
                    minutes: None,
                },
                &workspace,
                now,
            )
            .unwrap();
        assert_eq!(state.overlays.len(), 2);
        state.reconcile(&workspace, now + Duration::minutes(5));
        assert_eq!(
            state.overlays,
            [Overlay::Ticket {
                ticket_id: parent.clone()
            }]
        );
        state
            .apply(
                Action::ShowChildren { ticket_id: parent },
                &workspace,
                now + Duration::minutes(6),
            )
            .unwrap();
        state
            .apply(
                Action::ShowTicket {
                    ticket_id: independent.clone(),
                    minutes: None,
                },
                &workspace,
                now + Duration::minutes(11),
            )
            .unwrap();
        assert_eq!(
            state.overlays,
            [Overlay::Ticket {
                ticket_id: independent
            }]
        );
        state.reconcile(&workspace, now + Duration::minutes(16));
        assert!(state.overlays.is_empty());
    }

    #[test]
    fn resume_restarts_the_timer_only_for_the_current_context() {
        let now = DateTime::parse_from_rfc3339("2026-11-01T10:00:00Z")
            .unwrap()
            .to_utc();
        let workspace = Workspace::initial();
        let board_id = workspace.boards[0].id.clone();
        let mut state = DisplayState::default();
        state
            .apply(Action::ShowBacklog { board_id }, &workspace, now)
            .unwrap();
        assert!(
            state
                .apply(Action::Resume { revision: 0 }, &workspace, now)
                .is_err()
        );
        state
            .apply(
                Action::Resume { revision: 1 },
                &workspace,
                now + Duration::minutes(2),
            )
            .unwrap();
        assert_eq!(state.expires_at, Some(now + Duration::minutes(7)));
    }
}
