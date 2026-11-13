use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct Record {
    pub board_id: Option<String>,
    pub body: Value,
}
pub type Records = BTreeMap<(String, String), Record>;

pub fn to_records(state: &Workspace) -> Result<Records> {
    let mut records = Records::new();
    let mut put = |kind: &str, id: &str, board: Option<&str>, body: Value| {
        records.insert(
            (kind.into(), id.into()),
            Record {
                board_id: board.map(String::from),
                body,
            },
        );
    };
    macro_rules! value {
        ($v:expr) => {
            serde_json::to_value($v).map_err(|_| Error::Storage)?
        };
    }
    for board in &state.boards {
        put("board", &board.id, None, value!(board));
    }
    for column in &state.columns {
        put("column", &column.id, Some(&column.board_id), value!(column));
    }
    for ticket in &state.tickets {
        put("ticket", &ticket.id, Some(&ticket.board_id), value!(ticket));
    }
    for kind in &state.types {
        put("type", &kind.id, None, value!(kind));
    }
    for label in &state.labels {
        put("label", &label.id, None, value!(label));
    }
    for comment in &state.comments {
        let board = &state.find_ticket(&comment.ticket_id)?.board_id;
        put("comment", &comment.id, Some(board), value!(comment));
    }
    for dependency in &state.dependencies {
        let board = &state.find_ticket(&dependency.ticket_id)?.board_id;
        put(
            "dependency",
            &format!("{}:{}", dependency.ticket_id, dependency.prerequisite_id),
            Some(board),
            value!(dependency),
        );
    }
    for link in &state.links {
        let board = &state.find_ticket(&link.ticket_id)?.board_id;
        put(
            "link",
            &format!("{}:{}", link.ticket_id, link.target_id),
            Some(board),
            value!(link),
        );
    }
    for rule in &state.recurrences {
        let board = &state.find_ticket(&rule.template_id)?.board_id;
        put("recurrence", &rule.id, Some(board), value!(rule));
    }
    for (board, options) in &state.board_options {
        put("board_options", board, Some(board), value!(options));
    }
    for (context, view) in &state.views {
        put("view", context, Some(&view.board_id), value!(view));
    }
    put("settings", "global", None, value!(&state.settings));
    put("registry", "prefixes", None, value!(&state.issued_prefixes));
    put(
        "registry",
        "tickets",
        None,
        value!(&state.issued_ticket_ids),
    );
    Ok(records)
}

pub fn from_records(records: &Records) -> Result<Workspace> {
    let mut state = Workspace::default();
    for ((kind, id), record) in records {
        macro_rules! get {
            () => {
                serde_json::from_value(record.body.clone()).map_err(|_| Error::Storage)?
            };
        }
        match kind.as_str() {
            "board" => state.boards.push(get!()),
            "column" => state.columns.push(get!()),
            "ticket" => state.tickets.push(get!()),
            "type" => state.types.push(get!()),
            "label" => state.labels.push(get!()),
            "comment" => state.comments.push(get!()),
            "dependency" => {
                state.dependencies.insert(get!());
            }
            "link" => {
                state.links.insert(get!());
            }
            "recurrence" => state.recurrences.push(get!()),
            "board_options" => {
                state.board_options.insert(id.clone(), get!());
            }
            "view" => {
                state.views.insert(id.clone(), get!());
            }
            "settings" => state.settings = get!(),
            "registry" if id == "prefixes" => state.issued_prefixes = get!(),
            "registry" if id == "tickets" => state.issued_ticket_ids = get!(),
            _ => return Err(Error::Storage),
        }
    }
    Ok(state)
}
pub fn load(connection: &Connection) -> Result<Workspace> {
    let mut query = connection
        .prepare("SELECT kind,id,board_id,body FROM domain_records ORDER BY kind,id")
        .map_err(storage)?;
    let rows = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(storage)?;
    let mut records = Records::new();
    for row in rows {
        let (kind, id, board_id, body) = row.map_err(storage)?;
        records.insert(
            (kind, id),
            Record {
                board_id,
                body: decode(&body)?,
            },
        );
    }
    from_records(&records)
}

pub fn persist(tx: &Transaction<'_>, old: &Records, new: &Records) -> Result<()> {
    for key in old.keys().filter(|key| !new.contains_key(*key)) {
        tx.execute(
            "DELETE FROM domain_records WHERE kind=?1 AND id=?2",
            params![key.0, key.1],
        )
        .map_err(storage)?;
    }
    let mut statement = tx.prepare_cached("INSERT INTO domain_records(kind,id,board_id,body) VALUES (?1,?2,?3,?4) ON CONFLICT(kind,id) DO UPDATE SET board_id=excluded.board_id,body=excluded.body").map_err(storage)?;
    for (key, value) in new {
        if old.get(key) != Some(value) {
            statement
                .execute(params![key.0, key.1, value.board_id, encode(&value.body)?])
                .map_err(storage)?;
        }
    }
    Ok(())
}

pub fn undo_equal(a: Option<&Record>, b: Option<&Record>) -> bool {
    fn normalize(record: &Record) -> Record {
        let mut record = record.clone();
        if let Some(object) = record.body.as_object_mut() {
            object.remove("revision");
            object.remove("next_ticket_number");
            if let Some(details) = object.get_mut("details").and_then(Value::as_object_mut) {
                details.remove("updated_at");
            }
        }
        record
    }
    a.map(normalize) == b.map(normalize)
}

pub fn migrate_v1(tx: &Transaction<'_>) -> Result<Workspace> {
    let mut state = Workspace::default();
    let mut query = tx
        .prepare("SELECT id,name,prefix,revision,next_ticket_number FROM boards ORDER BY rowid")
        .map_err(storage)?;
    state.boards = query
        .query_map([], |r| {
            Ok(Board {
                id: r.get(0)?,
                name: r.get(1)?,
                prefix: r.get(2)?,
                revision: r.get(3)?,
                next_ticket_number: r.get(4)?,
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<_>>()
        .map_err(storage)?;
    if state.boards.is_empty() {
        return Ok(Workspace::initial());
    }
    let mut query = tx.prepare("SELECT id,board_id,name,position,is_done,wip_limit,wip_strict FROM board_columns ORDER BY position").map_err(storage)?;
    state.columns = query
        .query_map([], |r| {
            Ok(Column {
                id: r.get(0)?,
                board_id: r.get(1)?,
                name: r.get(2)?,
                position: r.get(3)?,
                is_done: r.get(4)?,
                wip_limit: r.get(5)?,
                wip_strict: r.get(6)?,
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<_>>()
        .map_err(storage)?;
    let mut query = tx.prepare("SELECT id,public_id,board_id,column_id,title,description,importance,done,revision FROM tickets ORDER BY rowid").map_err(storage)?;
    state.tickets = query
        .query_map([], |r| {
            Ok(Ticket {
                id: r.get(0)?,
                public_id: r.get(1)?,
                board_id: r.get(2)?,
                column_id: r.get(3)?,
                title: r.get(4)?,
                description: r.get(5)?,
                importance: r.get(6)?,
                done: r.get(7)?,
                revision: r.get(8)?,
                details: TicketDetails::default(),
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<_>>()
        .map_err(storage)?;
    for (rank, ticket) in state.tickets.iter_mut().enumerate() {
        ticket.details.rank = rank as i64 * 1024;
        ticket.details.created_at = engine::now();
        ticket.details.updated_at = ticket.details.created_at.clone();
        if !ticket.done {
            ticket.details.last_active_column = ticket.column_id.clone();
        }
        state.issued_ticket_ids.insert(ticket.public_id.clone());
    }
    for board in &state.boards {
        state.issued_prefixes.insert(board.prefix.clone());
        state
            .board_options
            .insert(board.id.clone(), BoardOptions::default());
    }
    state.validate()?;
    Ok(state)
}
