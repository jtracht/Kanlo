use super::*;
use kanlo_core::tv::{Action, DisplayState};

fn stored(connection: &Connection) -> Result<DisplayState> {
    let data: Option<String> = connection
        .query_row(
            "SELECT value FROM metadata WHERE key='tv_display'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    data.as_deref()
        .map(decode)
        .transpose()
        .map(Option::unwrap_or_default)
}

pub(crate) fn reset_context(tx: &Transaction<'_>) -> Result<()> {
    let mut state = stored(tx)?;
    if state.overlays.is_empty() && state.highlights.is_empty() {
        return Ok(());
    }
    state.overlays.clear();
    state.expires_at = None;
    state.highlights.clear();
    state.highlights_expire_at = None;
    state.revision = state.revision.checked_add(1).ok_or(Error::Storage)?;
    tx.execute(
        "UPDATE metadata SET value=?1 WHERE key='tv_display'",
        [encode(&state)?],
    )
    .map_err(storage)?;
    Ok(())
}

impl Store {
    pub fn display(&self) -> Result<DisplayState> {
        let mut state = stored(&self.connection)?;
        state.reconcile(&self.workspace()?, chrono::Utc::now());
        Ok(state)
    }

    pub fn control_display(&mut self, action: Action) -> Result<DisplayState> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        require_writable(&tx)?;
        let mut state = stored(&tx)?;
        let now = chrono::Utc::now();
        state.apply(action, &records::load(&tx)?, now)?;
        tx.execute(
            "INSERT INTO metadata(key,value) VALUES ('tv_display',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [encode(&state)?],
        ).map_err(storage)?;
        tx.execute(
            "INSERT INTO event_stream(payload,created_at) VALUES (?1,?2)",
            params![
                encode(&json!({"revision":revision(&tx)?,"boards":[],"display":true}))?,
                now.to_rfc3339()
            ],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanlo_core::tv::{Preferences, RotationEntry, View};

    #[test]
    fn saved_display_preferences_survive_reopening_the_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("kanlo.sqlite3");
        let mut store = Store::open(&path).unwrap();
        let board_id = store.boards().unwrap()[0].id.clone();
        let preferences = Preferences {
            view: View::Statistics,
            board_id: Some(board_id.clone()),
            rotation: vec![
                RotationEntry {
                    view: View::Statistics,
                    board_id: None,
                },
                RotationEntry {
                    view: View::Board,
                    board_id: Some(board_id.clone()),
                },
            ],
            rotation_seconds: 15,
            ..Preferences::default()
        };
        store
            .control_display(Action::Configure {
                preferences: preferences.clone(),
                revision: 0,
            })
            .unwrap();
        drop(store);
        let store = Store::open(path).unwrap();
        assert_eq!(store.display().unwrap().preferences, preferences);
        assert_eq!(store.display().unwrap().revision, 1);
    }
}
