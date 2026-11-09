use super::*;

#[test]
fn initial_domain_integrity() {
    let db = Store::open(":memory:").unwrap();
    assert_eq!(db.integrity_check().unwrap(), "ok");
}

fn ticket(db: &mut Store, board: &str, title: &str) -> Ticket {
    db.create_ticket(
        board,
        NewTicket {
            title: title.into(),
            description: String::new(),
            importance: 100,
            column_id: None,
        },
    )
    .unwrap()
}
fn child(db: &mut Store, parent: &Ticket, title: &str) -> Ticket {
    let result = db
        .execute(Command::CreateTicket {
            board_id: parent.board_id.clone(),
            title: title.into(),
            description: String::new(),
            importance: 0,
            column_id: None,
            parent_id: Some(parent.id.clone()),
            type_id: None,
        })
        .unwrap();
    db.ticket(result.entity_id.as_ref().unwrap()).unwrap()
}
fn patch(db: &mut Store, id: &str, fields: Value) -> Result<ChangeResult> {
    db.execute(Command::PatchTicket {
        id: id.into(),
        base_revision: db.ticket(id)?.revision,
        fields: serde_json::from_value(fields).unwrap(),
    })
}
fn done(db: &mut Store, id: &str, value: bool) -> Result<ChangeResult> {
    db.execute(Command::SetDone {
        id: id.into(),
        base_revision: db.ticket(id)?.revision,
        done: value,
    })
}
fn confirm(db: &mut Store, command: Command) -> Result<Receipt> {
    let preview = db.preview(command)?;
    db.confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
}
fn dependency(db: &mut Store, target: &str, prerequisite: &str) -> Result<ChangeResult> {
    db.execute(Command::AddDependency {
        ticket_id: target.into(),
        prerequisite_id: prerequisite.into(),
    })
}

#[test]
fn types_are_board_enabled_fields_and_global_labels_survive_moves() {
    let mut db = Store::open(":memory:").unwrap();
    let source = db.boards().unwrap()[0].id.clone();
    let target = db
        .create_board(NewBoard {
            name: "Web".into(),
            prefix: "WEB".into(),
        })
        .unwrap()
        .id;
    let t = ticket(&mut db, &source, "Root");
    db.execute(Command::UpsertType {
        value: TicketType {
            id: "bug".into(),
            name: "Bug".into(),
            color: "#cc0000".into(),
            icon: "bug".into(),
            rank: 42,
            allow_children: true,
        },
    })
    .unwrap();
    db.execute(Command::UpsertLabel {
        value: Label {
            id: "urgent".into(),
            name: "Urgent".into(),
            color: "#cc0000".into(),
        },
    })
    .unwrap();
    let mut options = BoardOptions::default();
    options.enabled_types.insert("bug".into());
    db.execute(Command::SetBoardOptions {
        board_id: source.clone(),
        options,
    })
    .unwrap();
    patch(
        &mut db,
        &t.id,
        json!({"type_id":"bug","custom_fields":{"Auftrag":"ABC"},"labels":["urgent"]}),
    )
    .unwrap();
    let moving = Command::MoveTicket {
        id: t.id.clone(),
        base_revision: db.ticket(&t.id).unwrap().revision,
        board_id: target.clone(),
        column_id: None,
        activate_types: false,
        convert_types: BTreeMap::new(),
    };
    assert!(matches!(
        db.execute(moving.clone()),
        Err(Error::Conflict(_))
    ));
    assert_eq!(db.ticket(&t.id).unwrap().board_id, source);
    let mut moving = moving;
    if let Command::MoveTicket { activate_types, .. } = &mut moving {
        *activate_types = true;
    }
    db.execute(moving).unwrap();
    let moved = db.ticket(&t.id).unwrap();
    assert_eq!(moved.public_id, t.public_id);
    assert_eq!(moved.details.custom_fields["Auftrag"], "ABC");
    assert!(moved.details.labels.contains("urgent"));
    assert!(
        db.preview(Command::DeleteType { id: "bug".into() })
            .is_err()
    );
    patch(&mut db, &t.id, json!({"type_id":null})).unwrap();
    assert_eq!(
        db.ticket(&t.id).unwrap().details.custom_fields["Auftrag"],
        "ABC"
    );
    confirm(&mut db, Command::DeleteType { id: "bug".into() }).unwrap();
}

#[test]
fn parent_children_move_atomically_and_only_one_level_is_allowed() {
    let mut db = Store::open(":memory:").unwrap();
    let source = db.boards().unwrap()[0].id.clone();
    let parent = ticket(&mut db, &source, "Parent");
    let child = child(&mut db, &parent, "Child");
    let nested = Command::CreateTicket {
        board_id: source.clone(),
        title: "Nested".into(),
        description: String::new(),
        importance: 0,
        column_id: None,
        parent_id: Some(child.id.clone()),
        type_id: None,
    };
    assert!(matches!(db.execute(nested), Err(Error::Validation(_))));
    assert!(
        db.move_ticket(
            &child.id,
            MoveTicket {
                board_id: source.clone(),
                column_id: None,
                base_revision: child.revision
            }
        )
        .is_err()
    );
    let target = db
        .create_board(NewBoard {
            name: "Other".into(),
            prefix: "OTH".into(),
        })
        .unwrap();
    db.move_ticket(
        &parent.id,
        MoveTicket {
            board_id: target.id.clone(),
            column_id: None,
            base_revision: db.ticket(&parent.id).unwrap().revision,
        },
    )
    .unwrap();
    assert_eq!(db.ticket(&child.id).unwrap().board_id, target.id);
    let done_column = db
        .create_column(
            &target.id,
            NewColumn {
                name: "Done".into(),
                is_done: true,
                wip_limit: None,
                wip_strict: false,
            },
        )
        .unwrap();
    db.move_ticket(
        &parent.id,
        MoveTicket {
            board_id: target.id.clone(),
            column_id: Some(done_column.id),
            base_revision: db.ticket(&parent.id).unwrap().revision,
        },
    )
    .unwrap();
    assert!(db.ticket(&child.id).unwrap().done);
    done(&mut db, &parent.id, false).unwrap();
    assert!(db.ticket(&child.id).unwrap().done);
    assert!(!db.ticket(&parent.id).unwrap().done);
}

#[test]
fn children_do_not_consume_wip_and_blockers_prevent_parent_completion() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let column = db
        .create_column(
            &board,
            NewColumn {
                name: "Limited".into(),
                is_done: false,
                wip_limit: Some(1),
                wip_strict: true,
            },
        )
        .unwrap();
    let parent = db
        .create_ticket(
            &board,
            NewTicket {
                title: "Parent".into(),
                description: String::new(),
                importance: 0,
                column_id: Some(column.id),
            },
        )
        .unwrap();
    let child = child(&mut db, &parent, "Child");
    let blocker = ticket(&mut db, &board, "Blocker");
    dependency(&mut db, &child.id, &blocker.id).unwrap();
    let before = db.revision().unwrap();
    assert!(matches!(
        done(&mut db, &parent.id, true),
        Err(Error::Blocked)
    ));
    assert_eq!(db.revision().unwrap(), before);
    assert!(!db.ticket(&parent.id).unwrap().done);
    assert!(!db.ticket(&child.id).unwrap().done);
    done(&mut db, &blocker.id, true).unwrap();
    done(&mut db, &parent.id, true).unwrap();
    assert!(db.ticket(&child.id).unwrap().done);
}

#[test]
fn cross_board_cycle_is_rejected_and_reopening_cascades_with_wip_fallback() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let a = ticket(&mut db, &board, "A");
    let b = ticket(&mut db, &board, "B");
    let c = ticket(&mut db, &board, "C");
    dependency(&mut db, &b.id, &a.id).unwrap();
    dependency(&mut db, &c.id, &b.id).unwrap();
    assert!(matches!(
        dependency(&mut db, &a.id, &c.id),
        Err(Error::Validation(_))
    ));
    let column = db
        .create_column(
            &board,
            NewColumn {
                name: "Limited".into(),
                is_done: false,
                wip_limit: Some(1),
                wip_strict: true,
            },
        )
        .unwrap();
    db.move_ticket(
        &b.id,
        MoveTicket {
            board_id: board.clone(),
            column_id: Some(column.id.clone()),
            base_revision: db.ticket(&b.id).unwrap().revision,
        },
    )
    .unwrap();
    done(&mut db, &a.id, true).unwrap();
    done(&mut db, &b.id, true).unwrap();
    done(&mut db, &c.id, true).unwrap();
    db.create_ticket(
        &board,
        NewTicket {
            title: "Occupant".into(),
            description: String::new(),
            importance: 0,
            column_id: Some(column.id),
        },
    )
    .unwrap();
    let result = done(&mut db, &a.id, false).unwrap();
    assert!(!db.ticket(&b.id).unwrap().done);
    assert!(!db.ticket(&c.id).unwrap().done);
    assert!(db.ticket(&b.id).unwrap().column_id.is_none());
    assert!(!result.warnings.is_empty());
    let rev = db.revision().unwrap();
    db.undo(&board, rev).unwrap();
    assert!(db.ticket(&a.id).unwrap().done);
    assert!(db.ticket(&b.id).unwrap().done);
    assert!(db.ticket(&c.id).unwrap().done);
}

#[test]
fn strict_wip_batch_rollback_warning_mode_and_no_history_on_failure() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let col = db
        .create_column(
            &board,
            NewColumn {
                name: "Limit".into(),
                is_done: false,
                wip_limit: Some(1),
                wip_strict: true,
            },
        )
        .unwrap();
    let a = ticket(&mut db, &board, "A");
    let b = ticket(&mut db, &board, "B");
    let history = db.undo_history(&board).unwrap().len();
    let revision = db.revision().unwrap();
    let moves = vec![a.clone(), b.clone()]
        .into_iter()
        .map(|t| Command::MoveTicket {
            id: t.id,
            base_revision: t.revision,
            board_id: board.clone(),
            column_id: Some(col.id.clone()),
            activate_types: false,
            convert_types: BTreeMap::new(),
        })
        .collect();
    assert!(matches!(
        db.execute(Command::Batch { commands: moves }),
        Err(Error::WipLimit)
    ));
    assert_eq!(db.revision().unwrap(), revision);
    assert_eq!(db.undo_history(&board).unwrap().len(), history);
    assert!(db.ticket(&a.id).unwrap().column_id.is_none());
    db.execute(Command::UpdateColumn {
        id: col.id.clone(),
        name: "Limit".into(),
        is_done: false,
        wip_limit: Some(1),
        wip_strict: false,
        color: "#cc0000".into(),
    })
    .unwrap();
    for t in [a, b] {
        let result = db.move_ticket(
            &t.id,
            MoveTicket {
                board_id: board.clone(),
                column_id: Some(col.id.clone()),
                base_revision: t.revision,
            },
        );
        assert!(result.is_ok());
    }
    assert_eq!(db.integrity_check().unwrap(), "ok");
}

#[test]
fn trash_requires_preview_lists_dependencies_and_preserves_children_for_recovery() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let parent = ticket(&mut db, &board, "Parent");
    let child = child(&mut db, &parent, "Child");
    let dependent = ticket(&mut db, &board, "Dependent");
    dependency(&mut db, &dependent.id, &child.id).unwrap();
    let command = Command::Trash {
        ids: vec![parent.id.clone()],
    };
    assert!(db.execute(command.clone()).is_err());
    let preview = db.preview(command).unwrap();
    assert_eq!(preview.removed_dependencies.len(), 1);
    assert_eq!(preview.result.affected.len(), 3);
    let op = Uuid::new_v4().to_string();
    let first = db.confirm(&preview.token, &op, "test").unwrap();
    let repeated = db.confirm(&preview.token, &op, "test").unwrap();
    assert_eq!(first.revision, repeated.revision);
    assert!(db.ticket(&child.id).unwrap().details.trashed);
    assert!(db.workspace().unwrap().dependencies.is_empty());
    assert!(
        db.preview(Command::DeleteBoard {
            id: board.clone(),
            base_revision: db.boards().unwrap()[0].revision
        })
        .is_err()
    );
    db.execute(Command::Restore {
        ids: vec![parent.id.clone()],
    })
    .unwrap();
    assert!(!db.ticket(&child.id).unwrap().details.trashed);
    assert!(db.workspace().unwrap().dependencies.is_empty());
}

#[test]
fn stale_preview_and_idempotency_payload_reuse_are_rejected() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let a = ticket(&mut db, &board, "A");
    let preview = db
        .preview(Command::Trash {
            ids: vec![a.id.clone()],
        })
        .unwrap();
    patch(&mut db, &a.id, json!({"title":"Changed"})).unwrap();
    assert!(
        db.confirm(&preview.token, &Uuid::new_v4().to_string(), "client")
            .is_err()
    );
    let (instance, epoch) = db.identity().unwrap();
    let mut operation = Mutation {
        op_id: Uuid::new_v4().to_string(),
        client_id: "client".into(),
        server_instance_id: instance,
        base_epoch: epoch,
        command: Command::PatchTicket {
            id: a.id.clone(),
            base_revision: db.ticket(&a.id).unwrap().revision,
            fields: BTreeMap::from([("importance".into(), json!(200))]),
        },
    };
    let receipt = db.mutate(operation.clone()).unwrap();
    assert_eq!(
        db.mutate(operation.clone()).unwrap().revision,
        receipt.revision
    );
    operation.command = Command::PatchTicket {
        id: a.id,
        base_revision: 1,
        fields: BTreeMap::from([("importance".into(), json!(1))]),
    };
    assert!(matches!(db.mutate(operation), Err(Error::Conflict(_))));
}

#[test]
fn undo_zero_is_persistent_does_not_disable_trash_or_statistics() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("db.sqlite3");
    let mut db = Store::open(&path).unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let options = BoardOptions {
        undo_depth: 0,
        ..Default::default()
    };
    db.execute(Command::SetBoardOptions {
        board_id: board.clone(),
        options,
    })
    .unwrap();
    let a = ticket(&mut db, &board, "A");
    confirm(
        &mut db,
        Command::Trash {
            ids: vec![a.id.clone()],
        },
    )
    .unwrap();
    assert!(db.undo_history(&board).unwrap().is_empty());
    assert!(
        !db.statistics(Some(&board), "2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .unwrap()
            .is_empty()
    );
    drop(db);
    let mut db = Store::open(path).unwrap();
    assert!(db.undo_history(&board).unwrap().is_empty());
    let revision = db.revision().unwrap();
    assert!(db.undo(&board, revision).is_err());
    db.execute(Command::Restore {
        ids: vec![a.id.clone()],
    })
    .unwrap();
    assert!(!db.ticket(&a.id).unwrap().details.trashed);
}

#[test]
fn undo_creation_never_recycles_ids_and_history_is_bounded() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let a = ticket(&mut db, &board, "A");
    let b = ticket(&mut db, &board, "B");
    let revision = db.revision().unwrap();
    db.undo(&board, revision).unwrap();
    let revision = db.revision().unwrap();
    db.undo(&board, revision).unwrap();
    assert!(db.ticket(&a.id).is_err());
    assert!(db.ticket(&b.id).is_err());
    assert_eq!(ticket(&mut db, &board, "C").public_id, "KAN-3");
    db.execute(Command::SetBoardOptions {
        board_id: board.clone(),
        options: BoardOptions {
            undo_depth: 2,
            ..Default::default()
        },
    })
    .unwrap();
    for n in 0..5 {
        ticket(&mut db, &board, &format!("Ticket {n}"));
    }
    assert_eq!(db.undo_history(&board).unwrap().len(), 2);
}

#[test]
fn archive_and_hidden_tickets_prevent_board_or_column_deletion() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let a = ticket(&mut db, &board, "A");
    done(&mut db, &a.id, true).unwrap();
    let column = db.ticket(&a.id).unwrap().column_id.unwrap();
    db.execute(Command::Archive {
        ids: vec![a.id.clone()],
        archived: true,
    })
    .unwrap();
    assert!(db.tickets(&board).unwrap().is_empty());
    assert!(db.preview(Command::DeleteColumn { id: column }).is_err());
    db.execute(Command::Archive {
        ids: vec![a.id.clone()],
        archived: false,
    })
    .unwrap();
    assert_eq!(db.tickets(&board).unwrap().len(), 1);
}

#[test]
fn migration_preserves_v1_data_identifiers_and_identity() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("v1.sqlite3");
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    raw.execute_batch("INSERT INTO metadata VALUES ('instance_id','original'),('epoch','original-epoch'); INSERT INTO boards VALUES ('board','Development','DEV',43,7); INSERT INTO board_columns VALUES ('column','board','Working',0,0,NULL,0); INSERT INTO tickets VALUES ('ticket','DEV-42','board','column','Preserved','Body',255,0,9); PRAGMA user_version=1;").unwrap();
    drop(raw);
    let mut db = Store::open(&path).unwrap();
    assert_eq!(
        db.identity().unwrap(),
        ("original".into(), "original-epoch".into())
    );
    assert_eq!(db.ticket("DEV-42").unwrap().revision, 9);
    assert_eq!(ticket(&mut db, "board", "Next").public_id, "DEV-43");
    assert_eq!(db.integrity_check().unwrap(), "ok");
}

#[test]
fn views_are_separate_and_manual_order_survives_other_sorting() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let a = ticket(&mut db, &board, "Zulu");
    let b = ticket(&mut db, &board, "Alpha");
    db.execute(Command::ReorderTickets {
        board_id: board.clone(),
        column_id: None,
        ids: vec![a.id.clone(), b.id.clone()],
    })
    .unwrap();
    let view = ViewState {
        board_id: board.clone(),
        sort: "title".into(),
        ..Default::default()
    };
    db.execute(Command::SetView {
        context: "web:one".into(),
        view: view.clone(),
    })
    .unwrap();
    let state = db.workspace().unwrap();
    assert!(!state.views.contains_key("tv:one"));
    assert_eq!(state.search(&view, chrono::Utc::now()).unwrap()[0].id, b.id);
    let manual = ViewState {
        board_id: board,
        sort: "manual".into(),
        ..Default::default()
    };
    assert_eq!(
        state.search(&manual, chrono::Utc::now()).unwrap()[0].id,
        a.id
    );
}

#[test]
fn full_json_roundtrip_preserves_relationships_comments_labels_and_fields() {
    use recovery::*;
    let mut source = Store::open(":memory:").unwrap();
    let board = source
        .create_board(NewBoard {
            name: "Development".into(),
            prefix: "DEV".into(),
        })
        .unwrap();
    let root = ticket(&mut source, &board.id, "Parent");
    let child = child(&mut source, &root, "Child");
    let prerequisite = ticket(&mut source, &board.id, "Prerequisite");
    source
        .execute(Command::UpsertLabel {
            value: Label {
                id: "global".into(),
                name: "Global".into(),
                color: "#cc6600".into(),
            },
        })
        .unwrap();
    patch(&mut source,&root.id,json!({"labels":["global"],"custom_fields":{"Reference":"X42"},"description":"# Markdown","deadline":"2026-11-30"})).unwrap();
    source
        .execute(Command::AddComment {
            ticket_id: root.id.clone(),
            markdown: "A **comment**".into(),
        })
        .unwrap();
    dependency(&mut source, &child.id, &prerequisite.id).unwrap();
    source
        .execute(Command::AddLink {
            ticket_id: root.id.clone(),
            target_id: prerequisite.id.clone(),
        })
        .unwrap();
    let exported = source.export_board(&board.id).unwrap();
    let text = encode(&exported).unwrap();
    assert!(!text.contains("undo_groups"));
    assert!(!text.contains("api_keys"));
    let mut target = Store::open(":memory:").unwrap();
    let preview = target
        .import_preview(
            parse_package(&text).unwrap(),
            ImportMode::Skip,
            BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(preview.summary["created"], 3);
    target
        .recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert_eq!(target.ticket(&root.id).unwrap().public_id, root.public_id);
    assert_eq!(
        target.ticket(&child.id).unwrap().details.parent_id,
        Some(root.id.clone())
    );
    assert_eq!(
        target.ticket(&root.id).unwrap().details.custom_fields["Reference"],
        "X42"
    );
    let state = target.workspace().unwrap();
    assert_eq!(state.comments.len(), 1);
    assert_eq!(state.dependencies.len(), 1);
    assert_eq!(state.links.len(), 1);
    assert_eq!(target.integrity_check().unwrap(), "ok");
    let preview = target
        .import_preview(exported.clone(), ImportMode::Skip, BTreeMap::new())
        .unwrap();
    assert_eq!(preview.summary["skipped"], 3);
    target
        .recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert_eq!(target.workspace().unwrap().comments.len(), 1);
    let preview = target
        .import_preview(
            exported,
            ImportMode::New,
            BTreeMap::from([(board.id, "COPY".into())]),
        )
        .unwrap();
    assert_eq!(preview.summary["created"], 3);
    target
        .recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert_eq!(target.workspace().unwrap().tickets.len(), 6);
    assert_eq!(target.workspace().unwrap().comments.len(), 2);
}

#[test]
fn csv_protects_formula_cells_and_import_defaults_to_no_overwrite() {
    use recovery::*;
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let t = ticket(&mut db, &board, "=HYPERLINK(\"evil\")");
    patch(&mut db, &t.id, json!({"description":"\t=SUM(1,2)"})).unwrap();
    let csv = db.export_csv(&board).unwrap();
    assert!(csv.contains("'=HYPERLINK"));
    assert!(csv.contains("'\t=SUM"));
    patch(&mut db, &t.id, json!({"title":"Server change"})).unwrap();
    let preview = db.csv_preview(&board, &csv, ImportMode::Skip).unwrap();
    assert_eq!(preview.summary["skipped"], 1);
    db.recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert_eq!(db.ticket(&t.id).unwrap().title, "Server change");
    let preview = db.csv_preview(&board, &csv, ImportMode::Update).unwrap();
    db.recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert_eq!(db.ticket(&t.id).unwrap().title, t.title);
    assert!(
        db.csv_preview(&board, "title,importance\nInvalid,999\n", ImportMode::Skip)
            .is_err()
    );
}

#[test]
fn restore_is_previewed_preserves_other_boards_and_changes_epoch() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let other = db
        .create_board(NewBoard {
            name: "Other".into(),
            prefix: "OTH".into(),
        })
        .unwrap()
        .id;
    let a = ticket(&mut db, &board, "A");
    let b = ticket(&mut db, &other, "B");
    let backup = db.backup().unwrap();
    let epoch = db.identity().unwrap().1;
    patch(&mut db, &a.id, json!({"title":"Changed A"})).unwrap();
    patch(&mut db, &b.id, json!({"title":"Changed B"})).unwrap();
    let other_before = encode(&db.ticket(&b.id).unwrap()).unwrap();
    let preview = db
        .restore_preview(backup, Some(vec![board.clone()]))
        .unwrap();
    assert_eq!(db.ticket(&a.id).unwrap().title, "Changed A");
    let op = Uuid::new_v4().to_string();
    db.recovery_confirm(&preview.token, &op, "test").unwrap();
    let new_epoch = db.identity().unwrap().1;
    assert_ne!(epoch, new_epoch);
    assert_eq!(db.ticket(&a.id).unwrap().title, "A");
    assert_eq!(encode(&db.ticket(&b.id).unwrap()).unwrap(), other_before);
    assert_eq!(db.safety_snapshots().unwrap().len(), 1);
    db.recovery_confirm(&preview.token, &op, "test").unwrap();
    assert_eq!(db.identity().unwrap().1, new_epoch);
    let snapshot = db
        .safety_snapshot(db.safety_snapshots().unwrap()[0]["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        snapshot.workspace.find_ticket(&a.id).unwrap().title,
        "Changed A"
    );
}

#[test]
fn backup_uses_allowlist_and_full_restore_keeps_counters_monotonic() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    ticket(&mut db, &board, "First");
    db.connection.execute_batch("CREATE TABLE secret_test(api_key TEXT);INSERT INTO secret_test VALUES ('DO_NOT_EXPORT_THIS');").unwrap();
    let backup = db.backup().unwrap();
    assert!(!encode(&backup).unwrap().contains("DO_NOT_EXPORT_THIS"));
    ticket(&mut db, &board, "Second");
    let preview = db.restore_preview(backup, None).unwrap();
    db.recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert_eq!(ticket(&mut db, &board, "Third").public_id, "KAN-3");
}

#[test]
fn sync_independent_edits_merge_same_fields_conflict_and_epoch_requires_review() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let t = ticket(&mut db, &board, "Base");
    let (instance, epoch) = db.identity().unwrap();
    let base = db.workspace().unwrap().ticket_fields(&t.id).unwrap();
    patch(&mut db, &t.id, json!({"title":"Server"})).unwrap();
    let operation = kanlo_core::sync::Operation {
        op_id: Uuid::new_v4().to_string(),
        client_id: "one".into(),
        server_instance_id: instance.clone(),
        base_epoch: epoch.clone(),
        entity_id: t.id.clone(),
        base_revision: 1,
        base_fields: base.clone(),
        changed_fields: BTreeMap::from([("importance".into(), json!(200))]),
        created_at: engine::now(),
    };
    let receipt = db.sync_patch(operation.clone()).unwrap();
    assert_eq!(receipt.status, "applied");
    assert_eq!(db.ticket(&t.id).unwrap().title, "Server");
    assert_eq!(db.ticket(&t.id).unwrap().importance, 200);
    assert_eq!(
        db.sync_patch(operation).unwrap().receipt.unwrap().revision,
        receipt.receipt.unwrap().revision
    );
    let conflict = kanlo_core::sync::Operation {
        op_id: Uuid::new_v4().to_string(),
        client_id: "two".into(),
        server_instance_id: instance,
        base_epoch: epoch,
        entity_id: t.id.clone(),
        base_revision: 1,
        base_fields: base,
        changed_fields: BTreeMap::from([("title".into(), json!("Local"))]),
        created_at: engine::now(),
    };
    let result = db.sync_patch(conflict.clone()).unwrap();
    assert_eq!(result.status, "conflict");
    assert_eq!(result.conflicts[0].server, "Server");
    let backup = db.backup().unwrap();
    let preview = db.restore_preview(backup, None).unwrap();
    db.recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    let mut old = conflict;
    old.op_id = Uuid::new_v4().to_string();
    old.changed_fields = BTreeMap::from([("description".into(), json!("Safe but old epoch"))]);
    old.base_fields = BTreeMap::from([("description".into(), json!(""))]);
    assert_eq!(db.sync_patch(old).unwrap().status, "epoch_review");
}

#[test]
fn scheduler_is_idempotent_and_after_completion_waits_for_previous_ticket() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let template = ticket(&mut db, &board, "Recurring");
    let at: chrono::DateTime<chrono::Utc> = "2026-11-10T08:00:00Z".parse().unwrap();
    let rule = Recurrence {
        id: "daily".into(),
        template_id: template.id.clone(),
        frequency: "daily".into(),
        interval: 1,
        weekdays: Default::default(),
        month_day: None,
        nth_weekday: None,
        time: "09:00".into(),
        timezone: "Europe/Berlin".into(),
        next_at: at.to_rfc3339(),
        anchor_at: None,
        after_completion: true,
        last_created: None,
        enabled: true,
    };
    db.execute(Command::UpsertRecurrence { value: rule })
        .unwrap();
    db.tick(at).unwrap();
    assert_eq!(db.workspace().unwrap().tickets.len(), 1);
    done(&mut db, &template.id, true).unwrap();
    db.tick(at).unwrap();
    assert_eq!(db.workspace().unwrap().tickets.len(), 2);
    db.tick(at).unwrap();
    assert_eq!(db.workspace().unwrap().tickets.len(), 2);
    db.tick(at + chrono::Duration::days(1)).unwrap();
    assert_eq!(db.workspace().unwrap().tickets.len(), 2);
    let previous = db.workspace().unwrap().recurrences[0]
        .last_created
        .clone()
        .unwrap();
    done(&mut db, &previous, true).unwrap();
    db.tick(at + chrono::Duration::days(1)).unwrap();
    assert_eq!(db.workspace().unwrap().tickets.len(), 3);
}

#[test]
fn automatic_archive_trash_purge_and_backup_are_opt_in() {
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let t = ticket(&mut db, &board, "Done");
    done(&mut db, &t.id, true).unwrap();
    let future = chrono::Utc::now() + chrono::Duration::days(10);
    db.tick(future).unwrap();
    assert!(!db.ticket(&t.id).unwrap().details.archived);
    db.execute(Command::SetBoardOptions {
        board_id: board.clone(),
        options: BoardOptions {
            auto_archive_days: Some(1),
            trash_retention_days: Some(1),
            ..Default::default()
        },
    })
    .unwrap();
    db.tick(future).unwrap();
    assert!(db.ticket(&t.id).unwrap().details.archived);
    confirm(
        &mut db,
        Command::Trash {
            ids: vec![t.id.clone()],
        },
    )
    .unwrap();
    db.tick(future).unwrap();
    assert!(db.ticket(&t.id).is_err());
    let dir = tempfile::tempdir().unwrap();
    assert!(db.scheduled_backup(dir.path(), future).unwrap().is_none());
    db.execute(Command::SetSettings {
        settings: Settings {
            backup_every_hours: Some(24),
            ..Default::default()
        },
    })
    .unwrap();
    let path = db.scheduled_backup(dir.path(), future).unwrap().unwrap();
    assert!(path.exists());
    assert!(db.scheduled_backup(dir.path(), future).unwrap().is_none());
}

#[test]
fn relocation_copies_and_verifies_database_and_old_writer_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.sqlite3");
    let target = temp.path().join("ssd");
    let mut db = Store::open(&source).unwrap();
    let mut old = Store::open(&source).unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let t = ticket(&mut db, &board, "Persistent");
    let identity = db.identity().unwrap();
    let path = db.relocate(&target).unwrap();
    assert!(path.exists());
    assert_eq!(db.identity().unwrap(), identity);
    assert_eq!(db.ticket(&t.id).unwrap().title, "Persistent");
    assert!(
        old.create_ticket(
            &board,
            NewTicket {
                title: "Forbidden old write".into(),
                description: String::new(),
                importance: 0,
                column_id: None
            }
        )
        .is_err()
    );
    assert_eq!(db.integrity_check().unwrap(), "ok");
}

#[test]
fn secrets_are_hashed_excluded_from_backups_and_revoked_after_restore() {
    let mut db = Store::open(":memory:").unwrap();
    db.set_passcode("a sufficiently long shared code").unwrap();
    assert!(db.login("wrong password").is_err());
    let session = db.login("a sufficiently long shared code").unwrap();
    db.authorize_session(&session.token, None, false).unwrap();
    assert!(db.authorize_session(&session.token, None, true).is_err());
    db.authorize_session(&session.token, Some(&session.csrf), true)
        .unwrap();
    assert_eq!(
        db.session_status(&session.token).unwrap()["csrf"],
        session.csrf
    );
    let key = db.mint_api_key("Integration").unwrap();
    let token = key["token"].as_str().unwrap();
    db.authorize_key(token).unwrap();
    let hash: String = db
        .connection
        .query_row("SELECT secret_hash FROM api_keys", [], |r| r.get(0))
        .unwrap();
    assert_ne!(hash, token);
    let backup = db.backup().unwrap();
    let serialized = encode(&backup).unwrap();
    for secret in [
        token,
        hash.as_str(),
        session.token.as_str(),
        session.csrf.as_str(),
        "a sufficiently long shared code",
    ] {
        assert!(!serialized.contains(secret));
    }
    let preview = db.restore_preview(backup, None).unwrap();
    db.recovery_confirm(&preview.token, &Uuid::new_v4().to_string(), "test")
        .unwrap();
    assert!(db.authorize_key(token).is_err());
    assert!(db.authorize_session(&session.token, None, false).is_err());
    assert!(db.login("a sufficiently long shared code").is_ok());
}

#[test]
fn offline_creation_maps_temporary_parent_ids_causally_without_duplicates() {
    use synchronization::*;
    let mut db = Store::open(":memory:").unwrap();
    let board = db.boards().unwrap()[0].id.clone();
    let (instance, epoch) = db.identity().unwrap();
    let temporary_id = format!("tmp:{}", Uuid::new_v4());
    let create = OfflineCreate {
        op_id: Uuid::new_v4().to_string(),
        client_id: "browser".into(),
        server_instance_id: instance.clone(),
        base_epoch: epoch.clone(),
        temporary_id: temporary_id.clone(),
        board_id: board,
        title: "Offline parent".into(),
        description: String::new(),
        importance: 0,
        column_id: None,
        parent_id: None,
        type_id: None,
    };
    let receipt = db.sync_create(create.clone()).unwrap();
    assert_eq!(
        db.sync_create(create.clone()).unwrap().revision,
        receipt.revision
    );
    assert_eq!(db.workspace().unwrap().tickets.len(), 1);
    let mut child = create;
    child.op_id = Uuid::new_v4().to_string();
    child.temporary_id = format!("tmp:{}", Uuid::new_v4());
    child.parent_id = Some(temporary_id.clone());
    child.title = "Offline child".into();
    let mapped = db.sync_create(child).unwrap();
    let child = db
        .ticket(mapped.result.entity_id.as_ref().unwrap())
        .unwrap();
    assert_eq!(child.details.parent_id, receipt.result.entity_id);
    let comment = OfflineComment {
        op_id: Uuid::new_v4().to_string(),
        client_id: "browser".into(),
        server_instance_id: instance,
        base_epoch: epoch,
        entity_id: temporary_id,
        markdown: "Independent comment".into(),
    };
    db.sync_comment(comment.clone()).unwrap();
    db.sync_comment(comment).unwrap();
    assert_eq!(db.workspace().unwrap().comments.len(), 1);
}
