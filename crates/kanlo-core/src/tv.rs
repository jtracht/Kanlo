use crate::{Error, Result, Ticket, Workspace};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Board,
    Overview,
    Statistics,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RotationEntry {
    pub view: View,
    pub board_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Overlay {
    Ticket { ticket_id: String },
    Backlog { board_id: String },
    Children { ticket_id: String },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DisplayState {
    pub preferences: Preferences,
    pub overlays: Vec<Overlay>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revision: u64,
    pub highlights: Vec<String>,
    pub highlights_expire_at: Option<DateTime<Utc>>,
    pub scroll_offset: i64,
}

#[derive(Debug, Deserialize, Serialize)]
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
        while let Some(expires) = self.expires_at.filter(|expires| *expires <= now) {
            self.overlays.pop();
            self.expires_at = (!self.overlays.is_empty() && self.preferences.overlay_minutes > 0)
                .then(|| expires + Duration::minutes(self.preferences.overlay_minutes.into()));
        }
        if let Some(first_missing) = self.overlays.iter().position(|overlay| match overlay {
            Overlay::Ticket { ticket_id } | Overlay::Children { ticket_id } => {
                active(workspace, ticket_id).is_err()
            }
            Overlay::Backlog { board_id } => require_board(workspace, board_id).is_err(),
        }) {
            self.overlays.truncate(first_missing);
        }
        if self.overlays.is_empty() {
            self.expires_at = None;
        }
    }

    pub fn apply(
        &mut self,
        action: Action,
        workspace: &Workspace,
        now: DateTime<Utc>,
    ) -> Result<()> {
        self.reconcile(workspace, now);
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
                }
                self.preferences = preferences;
            }
            Action::Select { view, board_id } => {
                if let Some(id) = &board_id {
                    require_board(workspace, id)?;
                }
                self.preferences.view = view;
                self.preferences.board_id = board_id;
                self.overlays.clear();
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
                }
                self.overlays.push(Overlay::Children { ticket_id });
            }
            Action::ShowTicket { ticket_id } => {
                let ticket = active(workspace, &ticket_id)?;
                match self.overlays.last() {
                    Some(Overlay::Backlog { board_id })
                        if ticket.board_id == *board_id && ticket.column_id.is_none() => {}
                    Some(Overlay::Children { ticket_id: parent })
                        if ticket.details.parent_id.as_ref() == Some(parent) =>
                    {
                        self.overlays.pop();
                    }
                    _ => {
                        self.preferences.view = View::Board;
                        self.preferences.board_id = Some(ticket.board_id.clone());
                        self.overlays.clear();
                    }
                }
                self.overlays.push(Overlay::Ticket {
                    ticket_id: ticket.id.clone(),
                });
            }
            Action::Back => {
                self.overlays.pop();
            }
            Action::Close => self.overlays.clear(),
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
            }
        }
        self.expires_at = (!self.overlays.is_empty() && self.preferences.overlay_minutes > 0)
            .then(|| now + Duration::minutes(self.preferences.overlay_minutes.into()));
        self.revision = self.revision.checked_add(1).ok_or(Error::Storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

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
}
