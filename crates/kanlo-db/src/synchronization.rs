use super::*;
use kanlo_core::sync::{FieldConflict, Operation};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncResponse {
    pub status: String,
    pub receipt: Option<Receipt>,
    pub conflicts: Vec<FieldConflict>,
    pub epoch_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OfflineCreate {
    pub op_id: String,
    pub client_id: String,
    pub server_instance_id: String,
    pub base_epoch: String,
    pub temporary_id: String,
    pub board_id: String,
    pub title: String,
    pub description: String,
    pub importance: u8,
    pub column_id: Option<String>,
    pub parent_id: Option<String>,
    pub type_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OfflineComment {
    pub op_id: String,
    pub client_id: String,
    pub server_instance_id: String,
    pub base_epoch: String,
    pub entity_id: String,
    pub markdown: String,
}

impl Store {
    pub fn sync_create(&mut self, operation: OfflineCreate) -> Result<Receipt> {
        validate_operation_id(&operation.op_id, &operation.client_id)?;
        if !operation
            .temporary_id
            .strip_prefix("tmp:")
            .is_some_and(|id| Uuid::parse_str(id).is_ok())
        {
            return Err(Error::Validation("Temporary ID must be tmp:UUID".into()));
        }
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
                "Instance/epoch changed; review queued creation".into(),
            ));
        }
        let existing: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM temp_mappings WHERE client_id=?1 AND temporary_id=?2)",
                params![operation.client_id, operation.temporary_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if existing {
            return Err(Error::Conflict(
                "Temporary ID already mapped; retain the original operation ID".into(),
            ));
        }
        let parent = operation
            .parent_id
            .as_ref()
            .map(|id| map_temporary(&tx, &operation.client_id, id))
            .transpose()?;
        let (result, changes) = apply_transaction_detailed(
            &tx,
            Command::CreateTicket {
                board_id: operation.board_id,
                title: operation.title,
                description: operation.description,
                importance: operation.importance,
                column_id: operation.column_id,
                parent_id: parent,
                type_id: operation.type_id,
            },
            true,
        )?;
        let id = result.entity_id.clone().ok_or(Error::Storage)?;
        tx.execute(
            "INSERT INTO temp_mappings VALUES (?1,?2,?3)",
            params![operation.client_id, operation.temporary_id, id],
        )
        .map_err(storage)?;
        let response = Receipt {
            op_id: operation.op_id.clone(),
            revision: revision(&tx)?,
            result,
            temp_ids: BTreeMap::from([(operation.temporary_id, id)]),
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
    pub fn sync_comment(&mut self, operation: OfflineComment) -> Result<Receipt> {
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
                "Instance/epoch changed; review queued comment".into(),
            ));
        }
        let ticket_id = map_temporary(&tx, &operation.client_id, &operation.entity_id)?;
        let (result, changes) = apply_transaction_detailed(
            &tx,
            Command::AddComment {
                ticket_id,
                markdown: operation.markdown,
            },
            true,
        )?;
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
    pub fn sync_patch(&mut self, operation: Operation) -> Result<SyncResponse> {
        validate_operation_id(&operation.op_id, &operation.client_id)?;
        let serialized = encode(&operation)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = receipt(&tx, &operation.op_id, &operation.client_id, &serialized)? {
            return Ok(SyncResponse {
                status: "applied".into(),
                receipt: Some(receipt),
                conflicts: vec![],
                epoch_changed: false,
            });
        }
        let (instance, epoch) = identity(&tx)?;
        if operation.server_instance_id != instance {
            return Err(Error::Conflict("Different server instance".into()));
        }
        let state = records::load(&tx)?;
        let entity_id = map_temporary(&tx, &operation.client_id, &operation.entity_id)?;
        let ticket = state.find_ticket(&entity_id)?;
        if ticket.details.trashed {
            return Err(Error::Conflict(
                "Ticket deleted; offline edits cannot resurrect it".into(),
            ));
        }
        if operation.base_revision > ticket.revision && epoch == operation.base_epoch {
            return Err(Error::Conflict("Base revision is ahead of server".into()));
        }
        let merge = kanlo_core::sync::merge(
            &operation.base_fields,
            &operation.changed_fields,
            &state.ticket_fields(&ticket.id)?,
        )?;
        if !merge.conflicts.is_empty() {
            return Ok(SyncResponse {
                status: "conflict".into(),
                receipt: None,
                conflicts: merge.conflicts,
                epoch_changed: epoch != operation.base_epoch,
            });
        }
        // Structural epoch changes require explicit review, even when text fields happen to match.
        if epoch != operation.base_epoch {
            return Ok(SyncResponse {
                status: "epoch_review".into(),
                receipt: None,
                conflicts: vec![],
                epoch_changed: true,
            });
        }
        let mut candidate = state.clone();
        let key = ticket.id.clone();
        let mut result = ChangeResult::default();
        let structural =
            merge.fields.contains_key("board_id") || merge.fields.contains_key("column_id");
        if structural
            && (!operation.changed_fields.contains_key("board_id")
                || !operation.changed_fields.contains_key("column_id")
                || !operation.base_fields.contains_key("board_id")
                || !operation.base_fields.contains_key("column_id"))
        {
            return Err(Error::Validation(
                "Moves require both base and target board_id/column_id".into(),
            ));
        }
        if structural && merge.fields.contains_key("done") {
            return Err(Error::Validation(
                "Choose a move or a binary completion, not both".into(),
            ));
        }
        let metadata: BTreeMap<_, _> = merge
            .fields
            .iter()
            .filter(|(name, _)| {
                !["board_id", "column_id", "done", "archived"].contains(&name.as_str())
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut collect = |change: ChangeResult| {
            result.entity_id = change.entity_id;
            result.affected.extend(change.affected);
            result.boards.extend(change.boards);
            result.warnings.extend(change.warnings);
        };
        if !metadata.is_empty() {
            let revision = candidate.find_ticket(&key)?.revision;
            collect(candidate.apply(Command::PatchTicket {
                id: key.clone(),
                base_revision: revision,
                fields: metadata,
            })?);
        }
        if structural {
            let board_id = merge.fields["board_id"]
                .as_str()
                .ok_or_else(|| Error::Validation("Invalid target board".into()))?
                .to_string();
            let column_id: Option<String> =
                serde_json::from_value(merge.fields["column_id"].clone())
                    .map_err(|_| Error::Validation("Invalid target column".into()))?;
            let revision = candidate.find_ticket(&key)?.revision;
            collect(candidate.apply(Command::MoveTicket {
                id: key.clone(),
                base_revision: revision,
                board_id,
                column_id,
                activate_types: false,
                convert_types: BTreeMap::new(),
            })?);
        }
        if let Some(done) = merge.fields.get("done") {
            let done = done
                .as_bool()
                .ok_or_else(|| Error::Validation("done must be boolean".into()))?;
            let revision = candidate.find_ticket(&key)?.revision;
            collect(candidate.apply(Command::SetDone {
                id: key.clone(),
                base_revision: revision,
                done,
            })?);
        }
        if let Some(archived) = merge.fields.get("archived") {
            let archived = archived
                .as_bool()
                .ok_or_else(|| Error::Validation("archived must be boolean".into()))?;
            collect(candidate.apply(Command::Archive {
                ids: vec![key],
                archived,
            })?);
        }
        commit_state(
            &tx,
            &state,
            &candidate,
            &result,
            "offline reconciliation",
            true,
        )?;
        let response = Receipt {
            op_id: operation.op_id.clone(),
            revision: revision(&tx)?,
            result,
            temp_ids: BTreeMap::new(),
            changes: entity_changes(&state, &candidate)?,
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
        Ok(SyncResponse {
            status: "applied".into(),
            receipt: Some(response),
            conflicts: vec![],
            epoch_changed: false,
        })
    }
}

fn map_temporary(connection: &Connection, client: &str, id: &str) -> Result<String> {
    if id.starts_with("tmp:") {
        connection
            .query_row(
                "SELECT entity_id FROM temp_mappings WHERE client_id=?1 AND temporary_id=?2",
                params![client, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?
            .ok_or_else(|| {
                Error::Conflict(
                    "Temporary parent/entity has not been acknowledged for this client".into(),
                )
            })
    } else {
        Ok(id.into())
    }
}
