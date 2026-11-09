//! All writes are validated within an IMMEDIATE SQLite transaction.
use kanlo_core::{
    Board, Column, Error, MoveTicket, NewBoard, NewColumn, NewTicket, Result, Ticket,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use std::{path::Path, time::Duration};
use uuid::Uuid;

pub struct Store {
    connection: Connection,
}

fn storage(error: rusqlite::Error) -> Error {
    if let rusqlite::Error::SqliteFailure(code, _) = &error
        && code.code == rusqlite::ErrorCode::ConstraintViolation
    {
        return Error::Conflict("Constraint violation".into());
    }
    Error::Storage
}

fn board(row: &Row<'_>) -> rusqlite::Result<Board> {
    Ok(Board {
        id: row.get(0)?,
        name: row.get(1)?,
        prefix: row.get(2)?,
        revision: row.get(3)?,
    })
}

fn column(row: &Row<'_>) -> rusqlite::Result<Column> {
    Ok(Column {
        id: row.get(0)?,
        board_id: row.get(1)?,
        name: row.get(2)?,
        position: row.get(3)?,
        is_done: row.get(4)?,
        wip_limit: row.get(5)?,
        wip_strict: row.get(6)?,
    })
}

fn ticket(row: &Row<'_>) -> rusqlite::Result<Ticket> {
    Ok(Ticket {
        id: row.get(0)?,
        public_id: row.get(1)?,
        board_id: row.get(2)?,
        column_id: row.get(3)?,
        title: row.get(4)?,
        description: row.get(5)?,
        importance: row.get(6)?,
        done: row.get(7)?,
        revision: row.get(8)?,
    })
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut connection = Connection::open(path).map_err(storage)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(storage)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(storage)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(storage)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let version: i64 = tx
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(storage)?;
        if version > 1 {
            return Err(Error::Conflict(
                "Database schema is newer than this program; refusing downgrade".into(),
            ));
        }
        if version == 0 {
            tx.execute_batch(include_str!("../migrations/001_initial.sql"))
                .map_err(storage)?;
            tx.execute(
                "INSERT INTO metadata VALUES ('instance_id', ?1), ('epoch', ?2)",
                params![Uuid::new_v4().to_string(), Uuid::new_v4().to_string()],
            )
            .map_err(storage)?;
            tx.pragma_update(None, "user_version", 1).map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        let mut store = Self { connection };
        store.seed()?;
        Ok(store)
    }

    fn write<T>(&mut self, action: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let result = action(&tx)?;
        tx.commit().map_err(storage)?;
        Ok(result)
    }

    fn seed(&mut self) -> Result<()> {
        self.write(|tx| {
            let count: i64 = tx
                .query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0))
                .map_err(storage)?;
            if count == 0 {
                let id = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO boards(id, name, prefix) VALUES (?1, 'Kanlo', 'KAN')",
                    [&id],
                )
                .map_err(storage)?;
                for (position, name, done) in [
                    (0, "Offen", false),
                    (1, "In Arbeit", false),
                    (2, "Erledigt", true),
                ] {
                    tx.execute(
                        "INSERT INTO board_columns VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0)",
                        params![Uuid::new_v4().to_string(), id, name, position, done],
                    )
                    .map_err(storage)?;
                }
            }
            Ok(())
        })
    }

    pub fn integrity_check(&self) -> Result<String> {
        self.connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(storage)
    }

    pub fn identity(&self) -> Result<(String, String)> {
        let get = |key: &str| {
            self.connection
                .query_row("SELECT value FROM metadata WHERE key=?1", [key], |r| {
                    r.get(0)
                })
                .map_err(storage)
        };
        Ok((get("instance_id")?, get("epoch")?))
    }

    pub fn boards(&self) -> Result<Vec<Board>> {
        let mut query = self
            .connection
            .prepare("SELECT id, name, prefix, revision FROM boards ORDER BY rowid")
            .map_err(storage)?;
        query
            .query_map([], board)
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    pub fn create_board(&mut self, input: NewBoard) -> Result<Board> {
        let name = kanlo_core::text(&input.name, 120, "name")?;
        let prefix = kanlo_core::board_prefix(&input.prefix)?;
        self.write(|tx| {
            let value = Board {
                id: Uuid::new_v4().to_string(),
                name,
                prefix,
                revision: 1,
            };
            tx.execute(
                "INSERT INTO boards(id, name, prefix) VALUES (?1, ?2, ?3)",
                params![value.id, value.name, value.prefix],
            )
            .map_err(storage)?;
            event(tx, &value.id, "board.created", 1)?;
            Ok(value)
        })
    }

    pub fn columns(&self, board_id: &str) -> Result<Vec<Column>> {
        require_board(&self.connection, board_id)?;
        let mut query = self.connection.prepare("SELECT id, board_id, name, position, is_done, wip_limit, wip_strict FROM board_columns WHERE board_id=?1 ORDER BY position").map_err(storage)?;
        query
            .query_map([board_id], column)
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    pub fn create_column(&mut self, board_id: &str, input: NewColumn) -> Result<Column> {
        let name = kanlo_core::text(&input.name, 120, "name")?;
        self.write(|tx| {
            require_board(tx, board_id)?;
            let position = tx
                .query_row(
                    "SELECT COALESCE(MAX(position), -1)+1 FROM board_columns WHERE board_id=?1",
                    [board_id],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            let value = Column {
                id: Uuid::new_v4().to_string(),
                board_id: board_id.into(),
                name,
                position,
                is_done: input.is_done,
                wip_limit: input.wip_limit,
                wip_strict: input.wip_strict,
            };
            tx.execute(
                "INSERT INTO board_columns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    value.id,
                    value.board_id,
                    value.name,
                    position,
                    value.is_done,
                    value.wip_limit,
                    value.wip_strict
                ],
            )
            .map_err(storage)?;
            event(tx, &value.id, "column.created", 1)?;
            Ok(value)
        })
    }

    pub fn tickets(&self, board_id: &str) -> Result<Vec<Ticket>> {
        require_board(&self.connection, board_id)?;
        let mut query = self.connection.prepare("SELECT id, public_id, board_id, column_id, title, description, importance, done, revision FROM tickets WHERE board_id=?1 ORDER BY rowid").map_err(storage)?;
        query
            .query_map([board_id], ticket)
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    pub fn ticket(&self, id: &str) -> Result<Ticket> {
        find_ticket(&self.connection, id)
    }

    pub fn create_ticket(&mut self, board_id: &str, input: NewTicket) -> Result<Ticket> {
        let title = kanlo_core::text(&input.title, 300, "title")?;
        if input.description.len() > 65536 {
            return Err(Error::Validation("description exceeds 64 KiB".into()));
        }
        self.write(|tx| {
            require_board(tx, board_id)?;
            let done = validate_destination(tx, board_id, input.column_id.as_deref(), None)?;
            let (prefix, number): (String, i64) = tx.query_row("SELECT prefix, next_ticket_number FROM boards WHERE id=?1", [board_id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(storage)?;
            let value = Ticket { id: Uuid::new_v4().to_string(), public_id: format!("{prefix}-{number}"), board_id: board_id.into(), column_id: input.column_id,
                title, description: input.description, importance: input.importance, done, revision: 1 };
            tx.execute("UPDATE boards SET next_ticket_number=next_ticket_number+1, revision=revision+1 WHERE id=?1", [board_id]).map_err(storage)?;
            tx.execute("INSERT INTO tickets VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
                params![value.id, value.public_id, value.board_id, value.column_id, value.title, value.description, value.importance, value.done]).map_err(storage)?;
            event(tx, &value.id, "ticket.created", 1)?;
            Ok(value)
        })
    }

    pub fn move_ticket(&mut self, id: &str, input: MoveTicket) -> Result<Ticket> {
        self.write(|tx| {
            let mut value = find_ticket(tx, id)?;
            if value.revision != input.base_revision {
                return Err(Error::Conflict(
                    "Stale revision; reload ticket before moving".into(),
                ));
            }
            require_board(tx, &input.board_id)?;
            let done = validate_destination(
                tx,
                &input.board_id,
                input.column_id.as_deref(),
                Some(&value.id),
            )?;
            if value.board_id == input.board_id && value.column_id == input.column_id {
                return Ok(value);
            }
            value.board_id = input.board_id;
            value.column_id = input.column_id;
            value.done = done;
            value.revision += 1;
            tx.execute(
                "UPDATE tickets SET board_id=?1, column_id=?2, done=?3, revision=?4 WHERE id=?5",
                params![
                    value.board_id,
                    value.column_id,
                    done,
                    value.revision,
                    value.id
                ],
            )
            .map_err(storage)?;
            event(tx, &value.id, "ticket.moved", value.revision)?;
            Ok(value)
        })
    }
}

fn require_board(connection: &Connection, id: &str) -> Result<()> {
    connection
        .query_row("SELECT 1 FROM boards WHERE id=?1", [id], |_| Ok(()))
        .optional()
        .map_err(storage)?
        .ok_or(Error::NotFound)
}

fn find_ticket(connection: &Connection, id: &str) -> Result<Ticket> {
    connection.query_row("SELECT id, public_id, board_id, column_id, title, description, importance, done, revision FROM tickets WHERE id=?1 OR public_id=?1", [id], ticket)
        .optional().map_err(storage)?.ok_or(Error::NotFound)
}

fn validate_destination(
    connection: &Connection,
    board_id: &str,
    column_id: Option<&str>,
    except: Option<&str>,
) -> Result<bool> {
    let Some(id) = column_id else {
        return Ok(false);
    };
    let destination = connection.query_row("SELECT id, board_id, name, position, is_done, wip_limit, wip_strict FROM board_columns WHERE id=?1", [id], column).optional().map_err(storage)?.ok_or(Error::NotFound)?;
    if destination.board_id != board_id {
        return Err(Error::Validation(
            "Column belongs to a different board".into(),
        ));
    }
    if destination.wip_strict
        && let Some(limit) = destination.wip_limit
    {
        let count: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM tickets WHERE column_id=?1 AND (?2 IS NULL OR id != ?2)",
                params![id, except],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if count >= limit {
            return Err(Error::WipLimit);
        }
    }
    Ok(destination.is_done)
}

fn event(tx: &Transaction<'_>, id: &str, kind: &str, revision: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO mutation_events(entity_id, kind, revision) VALUES (?1, ?2, ?3)",
        params![id, kind, revision],
    )
    .map_err(storage)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_ticket(column_id: Option<String>) -> NewTicket {
        NewTicket {
            title: "Erstes Ticket".into(),
            description: "".into(),
            importance: 255,
            column_id,
        }
    }

    #[test]
    fn migration_restart_preserves_identity_data_and_counter() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("kanlo.sqlite3");
        let mut db = Store::open(&path).unwrap();
        let identity = db.identity().unwrap();
        let board = db.boards().unwrap().remove(0);
        let first = db.create_ticket(&board.id, new_ticket(None)).unwrap();
        assert_eq!(first.public_id, "KAN-1");
        assert_eq!(db.integrity_check().unwrap(), "ok");
        drop(db);
        let mut db = Store::open(&path).unwrap();
        assert_eq!(db.boards().unwrap().len(), 1);
        assert_eq!(db.identity().unwrap(), identity);
        assert_eq!(db.ticket(&first.id).unwrap().title, first.title);
        assert_eq!(
            db.create_ticket(&board.id, new_ticket(None))
                .unwrap()
                .public_id,
            "KAN-2"
        );
    }

    #[test]
    fn failed_move_is_atomic_and_revision_prevents_lost_update() {
        let mut db = Store::open(":memory:").unwrap();
        let source = db.boards().unwrap().remove(0);
        let target = db
            .create_board(NewBoard {
                name: "Web".into(),
                prefix: "WEB".into(),
            })
            .unwrap();
        let column = db
            .create_column(
                &target.id,
                NewColumn {
                    name: "Work".into(),
                    is_done: false,
                    wip_limit: Some(1),
                    wip_strict: true,
                },
            )
            .unwrap();
        let first = db.create_ticket(&source.id, new_ticket(None)).unwrap();
        db.create_ticket(&target.id, new_ticket(Some(column.id.clone())))
            .unwrap();
        assert!(matches!(
            db.move_ticket(
                &first.id,
                MoveTicket {
                    board_id: target.id.clone(),
                    column_id: Some(column.id),
                    base_revision: 1
                }
            ),
            Err(Error::WipLimit)
        ));
        assert_eq!(db.ticket(&first.id).unwrap().board_id, source.id);
        let moved = db
            .move_ticket(
                &first.id,
                MoveTicket {
                    board_id: target.id.clone(),
                    column_id: None,
                    base_revision: 1,
                },
            )
            .unwrap();
        assert_eq!(moved.public_id, first.public_id);
        assert_eq!(moved.revision, 2);
        assert!(matches!(
            db.move_ticket(
                &first.id,
                MoveTicket {
                    board_id: source.id,
                    column_id: None,
                    base_revision: 1
                }
            ),
            Err(Error::Conflict(_))
        ));
        assert_eq!(db.integrity_check().unwrap(), "ok");
    }

    #[test]
    fn invalid_creation_does_not_consume_number_and_done_follows_column() {
        let mut db = Store::open(":memory:").unwrap();
        let board = db.boards().unwrap().remove(0);
        assert!(
            db.create_ticket(&board.id, new_ticket(Some("missing".into())))
                .is_err()
        );
        let done = db
            .columns(&board.id)
            .unwrap()
            .into_iter()
            .find(|c| c.is_done)
            .unwrap();
        let created = db
            .create_ticket(&board.id, new_ticket(Some(done.id)))
            .unwrap();
        assert_eq!(created.public_id, "KAN-1");
        assert!(created.done);
        assert!(
            !db.move_ticket(
                &created.id,
                MoveTicket {
                    board_id: board.id,
                    column_id: None,
                    base_revision: 1
                }
            )
            .unwrap()
            .done
        );
    }

    #[test]
    fn cross_board_column_is_rejected() {
        let mut db = Store::open(":memory:").unwrap();
        let initial = db.boards().unwrap().remove(0);
        let other = db
            .create_board(NewBoard {
                name: "Other".into(),
                prefix: "OTH".into(),
            })
            .unwrap();
        let column = db.columns(&initial.id).unwrap().remove(0);
        assert!(matches!(
            db.create_ticket(&other.id, new_ticket(Some(column.id))),
            Err(Error::Validation(_))
        ));
        assert!(db.tickets(&other.id).unwrap().is_empty());
    }

    #[test]
    fn future_schema_is_refused_without_downgrade() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("future.sqlite3");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 999)
            .unwrap();
        assert!(matches!(Store::open(&path), Err(Error::Conflict(_))));
        let version: i64 = Connection::open(path)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 999);
    }

    #[test]
    fn concurrent_connections_cannot_overfill_strict_column() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("concurrent.sqlite3");
        let mut db = Store::open(&path).unwrap();
        let board = db.boards().unwrap().remove(0);
        let column = db
            .create_column(
                &board.id,
                NewColumn {
                    name: "Limited".into(),
                    is_done: false,
                    wip_limit: Some(1),
                    wip_strict: true,
                },
            )
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let mut db = Store::open(&path).unwrap();
                let barrier = barrier.clone();
                let board = board.id.clone();
                let column = column.id.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    db.create_ticket(&board, new_ticket(Some(column)))
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Err(Error::WipLimit)))
                .count(),
            1
        );
    }
}
