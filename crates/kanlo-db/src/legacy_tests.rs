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
