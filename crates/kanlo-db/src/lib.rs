//! One transactional writer for HTTP, offline sync, scheduler and import.
use kanlo_core::*;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};
use uuid::Uuid;

mod display;
#[cfg(test)]
mod integration_tests;
pub mod maintenance;
mod records;
pub mod recovery;
pub mod security;
pub mod synchronization;

pub struct Store {
    connection: Connection,
}

pub(crate) fn storage(error: rusqlite::Error) -> Error {
    if let rusqlite::Error::SqliteFailure(code, _) = &error
        && code.code == rusqlite::ErrorCode::ConstraintViolation
    {
        return Error::Conflict("Database constraint violation".into());
    }
    Error::Storage
}
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| Error::Storage)
}
pub(crate) fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| Error::Storage)
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    pub op_id: String,
    pub client_id: String,
    pub server_instance_id: String,
    pub base_epoch: String,
    pub command: Command,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub base_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Receipt {
    pub op_id: String,
    pub revision: i64,
    pub result: ChangeResult,
    pub temp_ids: BTreeMap<String, String>,
    #[serde(default)]
    pub changes: Vec<EntityChange>,
    #[serde(default)]
    pub epoch: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EntityChange {
    pub kind: String,
    pub id: String,
    pub board_id: Option<String>,
    pub body: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Preview {
    pub token: String,
    pub revision: i64,
    pub result: ChangeResult,
    pub removed_dependencies: Vec<Dependency>,
    pub expires_at: String,
    pub tickets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecordChange {
    pub kind: String,
    pub id: String,
    pub before: Option<records::Record>,
    pub after: Option<records::Record>,
}

pub fn requires_confirmation(command: &Command) -> bool {
    match command {
        Command::Trash { .. }
        | Command::Purge { .. }
        | Command::DeleteBoard { .. }
        | Command::DeleteColumn { .. }
        | Command::DeleteType { .. }
        | Command::DeleteLabel { .. }
        | Command::DeleteComment { .. } => true,
        Command::Batch { commands } => {
            commands.len() > 10 || commands.iter().any(requires_confirmation)
        }
        _ => false,
    }
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
        if version > 4 {
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
        }
        if version < 2 {
            tx.execute_batch(include_str!("../migrations/002_domain.sql"))
                .map_err(storage)?;
            let old = records::migrate_v1(&tx)?;
            records::persist(&tx, &BTreeMap::new(), &records::to_records(&old)?)?;
            tx.execute_batch("DROP TABLE tickets; DROP TABLE board_columns; DROP TABLE boards;")
                .map_err(storage)?;
            tx.pragma_update(None, "user_version", 2).map_err(storage)?;
        }
        if version < 3 {
            tx.execute_batch(include_str!("../migrations/003_recovery.sql"))
                .map_err(storage)?;
            tx.pragma_update(None, "user_version", 3).map_err(storage)?;
        }
        if version < 4 {
            tx.execute_batch(include_str!("../migrations/004_security_sync.sql"))
                .map_err(storage)?;
            tx.pragma_update(None, "user_version", 4).map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        Ok(Self { connection })
    }

    pub fn integrity_check(&self) -> Result<String> {
        let integrity: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(storage)?;
        let fk_count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .map_err(storage)?;
        self.workspace()?.validate()?;
        if fk_count != 0 {
            return Err(Error::Storage);
        }
        Ok(integrity)
    }
    pub fn identity(&self) -> Result<(String, String)> {
        identity(&self.connection)
    }
    pub fn revision(&self) -> Result<i64> {
        revision(&self.connection)
    }
    pub fn workspace(&self) -> Result<Workspace> {
        records::load(&self.connection)
    }
    pub fn boards(&self) -> Result<Vec<Board>> {
        Ok(self.workspace()?.boards)
    }
    pub fn columns(&self, board_id: &str) -> Result<Vec<Column>> {
        let state = self.workspace()?;
        if !state.boards.iter().any(|b| b.id == board_id) {
            return Err(Error::NotFound);
        }
        let mut columns: Vec<_> = state
            .columns
            .into_iter()
            .filter(|c| c.board_id == board_id)
            .collect();
        columns.sort_by_key(|c| c.position);
        Ok(columns)
    }
    pub fn tickets(&self, board_id: &str) -> Result<Vec<Ticket>> {
        let state = self.workspace()?;
        if !state.boards.iter().any(|b| b.id == board_id) {
            return Err(Error::NotFound);
        }
        Ok(state
            .tickets
            .into_iter()
            .filter(|t| t.board_id == board_id && !t.details.trashed && !t.details.archived)
            .collect())
    }
    pub fn ticket(&self, id: &str) -> Result<Ticket> {
        Ok(self.workspace()?.find_ticket(id)?.clone())
    }
    pub fn create_board(&mut self, input: NewBoard) -> Result<Board> {
        let result = self.execute(Command::CreateBoard {
            name: input.name,
            prefix: input.prefix,
        })?;
        self.workspace()?
            .boards
            .into_iter()
            .find(|b| Some(&b.id) == result.entity_id.as_ref())
            .ok_or(Error::Storage)
    }
    pub fn create_column(&mut self, board_id: &str, input: NewColumn) -> Result<Column> {
        let result = self.execute(Command::CreateColumn {
            board_id: board_id.into(),
            name: input.name,
            is_done: input.is_done,
            wip_limit: input.wip_limit,
            wip_strict: input.wip_strict,
        })?;
        self.workspace()?
            .columns
            .into_iter()
            .find(|c| Some(&c.id) == result.entity_id.as_ref())
            .ok_or(Error::Storage)
    }
    pub fn create_ticket(&mut self, board_id: &str, input: NewTicket) -> Result<Ticket> {
        let result = self.execute(Command::CreateTicket {
            board_id: board_id.into(),
            title: input.title,
            description: input.description,
            importance: input.importance,
            column_id: input.column_id,
            parent_id: None,
            type_id: None,
        })?;
        self.ticket(result.entity_id.as_deref().ok_or(Error::Storage)?)
    }
    pub fn move_ticket(&mut self, id: &str, input: MoveTicket) -> Result<Ticket> {
        self.execute(Command::MoveTicket {
            id: id.into(),
            base_revision: input.base_revision,
            board_id: input.board_id,
            column_id: input.column_id,
            activate_types: false,
            convert_types: BTreeMap::new(),
        })?;
        self.ticket(id)
    }

    pub fn execute(&mut self, command: Command) -> Result<ChangeResult> {
        if requires_confirmation(&command) {
            return Err(Error::Conflict(
                "Preview and explicit confirmation required".into(),
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let result = apply_transaction(&tx, command, true)?;
        tx.commit().map_err(storage)?;
        Ok(result)
    }
    pub fn mutate(&mut self, operation: Mutation) -> Result<Receipt> {
        if requires_confirmation(&operation.command) {
            return Err(Error::Conflict(
                "Preview and explicit confirmation required".into(),
            ));
        }
        validate_operation_id(&operation.op_id, &operation.client_id)?;
        let serialized = encode(&operation)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = receipt(&tx, &operation.op_id, &operation.client_id, &serialized)? {
            return Ok(receipt);
        }
        let (instance, epoch) = identity(&tx)?;
        if instance != operation.server_instance_id || epoch != operation.base_epoch {
            return Err(Error::Conflict(
                "Instance/epoch changed; explicit reconciliation required".into(),
            ));
        }
        if let Some(context) = command_context(&records::load(&tx)?, &operation.command)?
            && context != operation.base_context
        {
            return Err(Error::Conflict("Configuration or structure changed; compare the saved base with the current server state".into()));
        }
        let (result, changes) = apply_transaction_detailed(&tx, operation.command, true)?;
        let response = Receipt {
            op_id: operation.op_id.clone(),
            revision: revision(&tx)?,
            result,
            temp_ids: BTreeMap::new(),
            changes,
            epoch: identity(&tx)?.1,
        };
        save_receipt(
            &tx,
            &operation.op_id,
            &operation.client_id,
            &serialized,
            &response,
        )?;
        tx.commit().map_err(storage)?;
        Ok(response)
    }

    pub fn preview(&mut self, command: Command) -> Result<Preview> {
        self.preview_impl(command, None)
    }
    pub fn preview_checked(&mut self, command: Command, base_context: Value) -> Result<Preview> {
        self.preview_impl(command, Some(base_context))
    }
    fn preview_impl(&mut self, command: Command, base_context: Option<Value>) -> Result<Preview> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let mut state = records::load(&tx)?;
        if let (Some(expected), Some(context)) = (base_context, command_context(&state, &command)?)
            && expected != context
        {
            return Err(Error::Conflict(
                "The target changed; inspect the current state before previewing".into(),
            ));
        }
        let before = state.dependencies.clone();
        let result = state.apply(command.clone())?;
        let token = Uuid::new_v4().to_string();
        let revision = revision(&tx)?;
        let created = engine::now();
        tx.execute(
            "INSERT INTO previews VALUES (?1, ?2, ?3, ?4)",
            params![token, revision, encode(&command)?, created],
        )
        .map_err(storage)?;
        tx.execute(
            "DELETE FROM previews WHERE created_at < ?1",
            [(chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339()],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        let tickets=state.tickets.iter().filter(|t|result.affected.contains(&t.id)).map(|t|json!({"id":t.id,"public_id":t.public_id,"title":t.title,"board_id":t.board_id})).collect();
        Ok(Preview {
            token,
            revision,
            result,
            removed_dependencies: before.difference(&state.dependencies).cloned().collect(),
            expires_at: (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
            tickets,
        })
    }
    pub fn confirm(&mut self, token: &str, op_id: &str, client_id: &str) -> Result<Receipt> {
        validate_operation_id(op_id, client_id)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let serialized = encode(&json!({"confirm": token}))?;
        if let Some(receipt) = receipt(&tx, op_id, client_id, &serialized)? {
            return Ok(receipt);
        }
        let (base, command, created): (i64, String, String) = tx
            .query_row(
                "SELECT revision, command, created_at FROM previews WHERE token=?1",
                [token],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
        let created = chrono::DateTime::parse_from_rfc3339(&created).map_err(|_| Error::Storage)?;
        if revision(&tx)? != base
            || chrono::Utc::now()
                .signed_duration_since(created)
                .num_seconds()
                > 300
        {
            return Err(Error::Conflict(
                "Preview expired or state changed; preview again".into(),
            ));
        }
        let (result, changes) = apply_transaction_detailed(&tx, decode(&command)?, true)?;
        tx.execute("DELETE FROM previews WHERE token=?1", [token])
            .map_err(storage)?;
        let response = Receipt {
            op_id: op_id.into(),
            revision: revision(&tx)?,
            result,
            temp_ids: BTreeMap::new(),
            changes,
            epoch: identity(&tx)?.1,
        };
        save_receipt(&tx, op_id, client_id, &serialized, &response)?;
        tx.commit().map_err(storage)?;
        Ok(response)
    }

    pub fn undo_history(&self, board: &str) -> Result<Vec<Value>> {
        let state = self.workspace()?;
        if state
            .board_options
            .get(board)
            .is_none_or(|o| o.undo_depth == 0)
        {
            return Ok(vec![]);
        }
        let mut query = self.connection.prepare("SELECT g.sequence, g.description, g.created_at FROM undo_groups g JOIN undo_boards b ON b.sequence=g.sequence WHERE b.board_id=?1 ORDER BY g.sequence DESC").map_err(storage)?;
        query.query_map([board], |r| Ok(json!({"id": r.get::<_, i64>(0)?, "description": r.get::<_, String>(1)?, "created_at": r.get::<_, String>(2)?}))).map_err(storage)?.collect::<rusqlite::Result<_>>().map_err(storage)
    }
    pub fn undo(&mut self, board: &str, expected_revision: i64) -> Result<ChangeResult> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if revision(&tx)? != expected_revision {
            return Err(Error::Conflict("State changed before undo".into()));
        }
        let before = records::load(&tx)?;
        if before
            .board_options
            .get(board)
            .is_none_or(|o| o.undo_depth == 0)
        {
            return Err(Error::Conflict("Undo disabled".into()));
        }
        let (sequence, changes): (i64, String) = tx.query_row("SELECT g.sequence,g.changes FROM undo_groups g JOIN undo_boards b ON b.sequence=g.sequence WHERE b.board_id=?1 ORDER BY g.sequence DESC LIMIT 1", [board], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(storage)?.ok_or(Error::NotFound)?;
        let changes: Vec<RecordChange> = decode(&changes)?;
        let mut current = records::to_records(&before)?;
        let mut result = ChangeResult::default();
        for change in &changes {
            let key = (change.kind.clone(), change.id.clone());
            if !records::undo_equal(current.get(&key), change.after.as_ref()) {
                return Err(Error::Conflict("A later change prevents safe undo".into()));
            }
            if let Some(record) = &change.before {
                current.insert(key, record.clone());
            } else {
                current.remove(&key);
            }
        }
        let mut after = records::from_records(&current)?;
        after
            .issued_prefixes
            .extend(before.issued_prefixes.iter().cloned());
        after
            .issued_ticket_ids
            .extend(before.issued_ticket_ids.iter().cloned());
        for b in &mut after.boards {
            if let Some(old) = before.boards.iter().find(|old| old.id == b.id) {
                b.next_ticket_number = b.next_ticket_number.max(old.next_ticket_number);
                b.revision = old.revision + 1;
            }
        }
        for ticket in &mut after.tickets {
            if changes
                .iter()
                .any(|c| c.kind == "ticket" && c.id == ticket.id)
            {
                ticket.revision = before
                    .find_ticket(&ticket.id)
                    .map_or(ticket.revision + 1, |t| t.revision + 1);
                ticket.details.updated_at = engine::now();
                result.affected.insert(ticket.id.clone());
                result.boards.insert(ticket.board_id.clone());
            }
        }
        after.validate()?;
        commit_state(&tx, &before, &after, &result, "undo", false)?;
        tx.execute("DELETE FROM undo_groups WHERE sequence=?1", [sequence])
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(result)
    }

    pub fn event_cursor(&self) -> Result<i64> {
        self.connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM event_stream",
                [],
                |row| row.get(0),
            )
            .map_err(storage)
    }

    pub fn events(&self, after: i64, limit: usize) -> Result<Vec<Value>> {
        let mut query = self.connection.prepare("SELECT sequence,payload,created_at FROM event_stream WHERE sequence>?1 ORDER BY sequence LIMIT ?2").map_err(storage)?;
        query
            .query_map(params![after, limit.min(1000) as i64], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?
            .map(|r| {
                let (seq, body, time) = r.map_err(storage)?;
                Ok(json!({"id":seq,"data":decode::<Value>(&body)?,"created_at":time}))
            })
            .collect()
    }
    pub fn lookup_receipt(&self, op_id: &str, client_id: &str) -> Result<Option<Receipt>> {
        let row: Option<String> = self
            .connection
            .query_row(
                "SELECT response FROM receipts WHERE op_id=?1 AND client_id=?2",
                params![op_id, client_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        row.map(|value| decode(&value)).transpose()
    }
    pub fn statistics(&self, board_id: Option<&str>, from: &str, to: &str) -> Result<Vec<Value>> {
        chrono::DateTime::parse_from_rfc3339(from)
            .map_err(|_| Error::Validation("Invalid from timestamp".into()))?;
        chrono::DateTime::parse_from_rfc3339(to)
            .map_err(|_| Error::Validation("Invalid to timestamp".into()))?;
        let mut query = self.connection.prepare("SELECT board_id,sample,created_at FROM statistics_samples WHERE (?1 IS NULL OR board_id=?1) AND created_at>=?2 AND created_at<=?3 ORDER BY sequence LIMIT 100000").map_err(storage)?;
        query
            .query_map(params![board_id, from, to], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?
            .map(|r| {
                let (board, sample, time) = r.map_err(storage)?;
                Ok(json!({"board_id":board,"sample":decode::<Value>(&sample)?,"created_at":time}))
            })
            .collect()
    }
}

pub(crate) fn identity(connection: &Connection) -> Result<(String, String)> {
    let get = |key: &str| {
        connection
            .query_row("SELECT value FROM metadata WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .map_err(storage)
    };
    Ok((get("instance_id")?, get("epoch")?))
}
pub(crate) fn revision(connection: &Connection) -> Result<i64> {
    connection
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM metadata WHERE key='revision'",
            [],
            |r| r.get(0),
        )
        .map_err(storage)
}
pub(crate) fn validate_operation_id(op_id: &str, client: &str) -> Result<()> {
    if Uuid::parse_str(op_id).is_err() || client.is_empty() || client.len() > 128 {
        return Err(Error::Validation(
            "Operation UUID and client ID required".into(),
        ));
    }
    Ok(())
}
pub(crate) fn receipt(
    tx: &Transaction<'_>,
    id: &str,
    client: &str,
    request: &str,
) -> Result<Option<Receipt>> {
    let saved: Option<(String, String, String)> = tx
        .query_row(
            "SELECT client_id,request_hash,response FROM receipts WHERE op_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(storage)?;
    if let Some((owner, fingerprint, response)) = saved {
        if owner != client || fingerprint != hash(request) {
            return Err(Error::Conflict(
                "Operation ID reused with different payload/client".into(),
            ));
        }
        return Ok(Some(decode(&response)?));
    }
    Ok(None)
}
pub(crate) fn save_receipt(
    tx: &Transaction<'_>,
    id: &str,
    client: &str,
    request: &str,
    response: &Receipt,
) -> Result<()> {
    tx.execute(
        "INSERT INTO receipts VALUES (?1,?2,?3,?4,?5)",
        params![id, client, hash(request), encode(response)?, engine::now()],
    )
    .map_err(storage)?;
    Ok(())
}
pub(crate) fn apply_transaction(
    tx: &Transaction<'_>,
    command: Command,
    history: bool,
) -> Result<ChangeResult> {
    Ok(apply_transaction_detailed(tx, command, history)?.0)
}
pub(crate) fn apply_transaction_detailed(
    tx: &Transaction<'_>,
    command: Command,
    history: bool,
) -> Result<(ChangeResult, Vec<EntityChange>)> {
    let before = records::load(tx)?;
    let mut after = before.clone();
    let description = encode(&command)?;
    let result = after.apply(command)?;
    commit_state(tx, &before, &after, &result, &description, history)?;
    Ok((result, entity_changes(&before, &after)?))
}
pub(crate) fn entity_changes(before: &Workspace, after: &Workspace) -> Result<Vec<EntityChange>> {
    let old = records::to_records(before)?;
    let new = records::to_records(after)?;
    let keys: BTreeSet<_> = old.keys().chain(new.keys()).collect();
    Ok(keys
        .into_iter()
        .filter(|key| key.0 != "registry" && old.get(*key) != new.get(*key))
        .map(|key| EntityChange {
            kind: key.0.clone(),
            id: key.1.clone(),
            board_id: new
                .get(key)
                .or_else(|| old.get(key))
                .and_then(|record| record.board_id.clone()),
            body: new.get(key).map(|record| record.body.clone()),
        })
        .collect())
}
fn require_writable(connection: &Connection) -> Result<()> {
    let relocated: Option<String> = connection
        .query_row(
            "SELECT value FROM metadata WHERE key='relocated_to'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    if relocated.is_some() {
        return Err(Error::Conflict(
            "Database relocated; configure the new data directory".into(),
        ));
    }
    Ok(())
}

pub(crate) fn commit_state(
    tx: &Transaction<'_>,
    before: &Workspace,
    after: &Workspace,
    result: &ChangeResult,
    description: &str,
    history: bool,
) -> Result<()> {
    require_writable(tx)?;
    let old = records::to_records(before)?;
    let new = records::to_records(after)?;
    if old == new {
        return Ok(());
    }
    records::persist(tx, &old, &new)?;
    tx.execute(
        "UPDATE metadata SET value=CAST(CAST(value AS INTEGER)+1 AS TEXT) WHERE key='revision'",
        [],
    )
    .map_err(storage)?;
    let time = engine::now();
    tx.execute(
        "INSERT INTO event_stream(payload,created_at) VALUES (?1,?2)",
        params![
            encode(
                &json!({"revision":revision(tx)?,"affected":result.affected,"boards":result.boards})
            )?,
            time
        ],
    )
    .map_err(storage)?;
    for board in &result.boards {
        let tickets: Vec<_> = after
            .tickets
            .iter()
            .filter(|t| &t.board_id == board && !t.details.trashed && !t.details.archived)
            .collect();
        let created = tickets
            .iter()
            .filter(|t| before.find_ticket(&t.id).is_err())
            .count();
        let completed = tickets
            .iter()
            .filter(|t| t.done && before.find_ticket(&t.id).is_ok_and(|old| !old.done))
            .count();
        let sample = json!({"active":tickets.len(),"done":tickets.iter().filter(|t|t.done).count(),"backlog":tickets.iter().filter(|t|t.column_id.is_none()).count(),"blocked":tickets.iter().filter(|t|after.blocked(&t.id)).count(),"created":created,"completed":completed});
        tx.execute(
            "INSERT INTO statistics_samples(board_id,sample,created_at) VALUES (?1,?2,?3)",
            params![board, encode(&sample)?, time],
        )
        .map_err(storage)?;
    }
    for (board, options) in &after.board_options {
        if options.undo_depth == 0 {
            tx.execute("DELETE FROM undo_groups WHERE sequence IN (SELECT sequence FROM undo_boards WHERE board_id=?1)", [board]).map_err(storage)?;
        }
    }
    if history
        && !result.boards.is_empty()
        && result
            .boards
            .iter()
            .all(|b| after.board_options.get(b).is_some_and(|o| o.undo_depth > 0))
    {
        let keys: BTreeSet<_> = old.keys().chain(new.keys()).cloned().collect();
        let changes: Vec<_> = keys
            .into_iter()
            .filter(|(kind, _)| kind != "registry")
            .filter_map(|key| {
                if old.get(&key) != new.get(&key) {
                    Some(RecordChange {
                        kind: key.0.clone(),
                        id: key.1.clone(),
                        before: old.get(&key).cloned(),
                        after: new.get(&key).cloned(),
                    })
                } else {
                    None
                }
            })
            .collect();
        tx.execute(
            "INSERT INTO undo_groups(description,changes,created_at) VALUES (?1,?2,?3)",
            params![description, encode(&changes)?, time],
        )
        .map_err(storage)?;
        let sequence = tx.last_insert_rowid();
        for board in &result.boards {
            tx.execute(
                "INSERT INTO undo_boards VALUES (?1,?2)",
                params![sequence, board],
            )
            .map_err(storage)?;
            let depth = after.board_options.get(board).map_or(0, |o| o.undo_depth);
            tx.execute("DELETE FROM undo_groups WHERE sequence IN (SELECT sequence FROM undo_boards WHERE board_id=?1 ORDER BY sequence DESC LIMIT -1 OFFSET ?2)", params![board,depth as i64]).map_err(storage)?;
        }
    }
    Ok(())
}

pub fn command_context(state: &Workspace, command: &Command) -> Result<Option<Value>> {
    let context = match command {
        Command::SetSettings { .. } => json!(state.settings),
        Command::SetBoardOptions { board_id, .. } => json!(state.board_options.get(board_id)),
        Command::UpdateColumn { id, .. } | Command::DeleteColumn { id } => {
            json!(state.columns.iter().find(|c| &c.id == id))
        }
        Command::ReorderColumns { board_id, .. } => {
            let mut columns: Vec<_> = state
                .columns
                .iter()
                .filter(|c| &c.board_id == board_id)
                .collect();
            columns.sort_by_key(|c| c.position);
            json!(columns)
        }
        Command::ReorderTickets {
            board_id,
            column_id,
            ..
        } => {
            let mut tickets: Vec<_> = state
                .tickets
                .iter()
                .filter(|t| {
                    &t.board_id == board_id
                        && &t.column_id == column_id
                        && t.details.parent_id.is_none()
                        && !t.details.archived
                        && !t.details.trashed
                })
                .collect();
            tickets.sort_by_key(|t| t.details.rank);
            json!(
                tickets
                    .iter()
                    .map(|t| (&t.id, t.details.rank))
                    .collect::<Vec<_>>()
            )
        }
        Command::UpsertType { value } => json!(state.types.iter().find(|t| t.id == value.id)),
        Command::UpsertLabel { value } => json!(state.labels.iter().find(|l| l.id == value.id)),
        Command::UpsertRecurrence { value } => {
            json!(state.recurrences.iter().find(|r| r.id == value.id))
        }
        Command::DeleteRecurrence { id } => json!(state.recurrences.iter().find(|r| &r.id == id)),
        Command::SetView { context, .. } => json!(state.views.get(context)),
        Command::RemoveDependency {
            ticket_id,
            prerequisite_id,
        } => json!(state.dependencies.contains(&Dependency {
            ticket_id: ticket_id.clone(),
            prerequisite_id: prerequisite_id.clone()
        })),
        Command::RemoveLink {
            ticket_id,
            target_id,
        } => json!(state.links.contains(&TicketLink {
            ticket_id: ticket_id.clone(),
            target_id: target_id.clone()
        })),
        Command::Batch { commands } => json!(
            commands
                .iter()
                .map(|c| command_context(state, c))
                .collect::<Result<Vec<_>>>()?
        ),
        _ => return Ok(None),
    };
    Ok(Some(context))
}

include!("legacy_tests.rs");
