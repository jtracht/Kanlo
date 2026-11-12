use super::*;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    #[default]
    Skip,
    Update,
    New,
}

impl Store {
    pub fn import_preview(
        &mut self,
        package: BoardExport,
        mode: ImportMode,
        prefix_overrides: BTreeMap<String, String>,
    ) -> Result<RecoveryPreview> {
        if package.schema_version != 1 {
            return Err(Error::Validation("Unsupported export version".into()));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = records::load(&tx)?;
        let (candidate, summary) = merge_import(&current, &package, mode, &prefix_overrides)?;
        candidate.validate()?;
        let preview = save_preview(&tx, "import", &candidate, &json!({}), summary)?;
        tx.commit().map_err(storage)?;
        Ok(preview)
    }

    pub fn export_csv(&self, board_id: &str) -> Result<String> {
        let package = self.export_board(board_id)?;
        let state = &package.workspace;
        let mut writer = csv::Writer::from_writer(vec![]);
        writer
            .write_record([
                "kanlo_csv_version",
                "public_id",
                "title",
                "description",
                "importance",
                "deadline",
                "column_name",
            ])
            .map_err(|_| Error::Storage)?;
        for ticket in &state.tickets {
            let column = state
                .columns
                .iter()
                .find(|c| Some(&c.id) == ticket.column_id.as_ref())
                .map_or("", |c| c.name.as_str());
            writer
                .write_record([
                    "1".into(),
                    safe_cell(&ticket.public_id),
                    safe_cell(&ticket.title),
                    safe_cell(&ticket.description),
                    ticket.importance.to_string(),
                    safe_cell(ticket.details.deadline.as_deref().unwrap_or("")),
                    safe_cell(column),
                ])
                .map_err(|_| Error::Storage)?;
        }
        String::from_utf8(writer.into_inner().map_err(|_| Error::Storage)?)
            .map_err(|_| Error::Storage)
    }

    pub fn csv_preview(
        &mut self,
        board_id: &str,
        input: &str,
        mode: ImportMode,
    ) -> Result<RecoveryPreview> {
        if input.len() > 8 * 1024 * 1024 {
            return Err(Error::Validation("CSV exceeds 8 MiB".into()));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let mut state = records::load(&tx)?;
        if !state.boards.iter().any(|b| b.id == board_id) {
            return Err(Error::NotFound);
        }
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::Headers)
            .from_reader(input.as_bytes());
        let headers = reader
            .headers()
            .map_err(|_| Error::Validation("Invalid CSV header".into()))?
            .clone();
        if !headers.iter().any(|h| h == "title") {
            return Err(Error::Validation("CSV requires title column".into()));
        }
        let index = |field: &str| headers.iter().position(|h| h == field);
        let mut created = 0;
        let mut updated = 0;
        let mut skipped = 0;
        for (number, row) in reader.records().enumerate() {
            if number >= 10000 {
                return Err(Error::Validation("CSV exceeds 10000 rows".into()));
            }
            let row =
                row.map_err(|_| Error::Validation(format!("Invalid CSV row {}", number + 2)))?;
            let generated = index("kanlo_csv_version").and_then(|i| row.get(i)) == Some("1");
            let cell = |field: &str| {
                let value = index(field).and_then(|i| row.get(i)).unwrap_or("");
                if generated {
                    restore_cell(value)
                } else {
                    value.to_string()
                }
            };
            let public_id = cell("public_id");
            let existing = state
                .tickets
                .iter()
                .find(|t| !public_id.is_empty() && t.public_id == public_id)
                .cloned();
            if existing.is_some() && mode == ImportMode::Skip {
                skipped += 1;
                continue;
            }
            let title = cell("title");
            let description = cell("description");
            let importance = if cell("importance").is_empty() {
                0
            } else {
                cell("importance").parse::<u8>().map_err(|_| {
                    Error::Validation(format!("Invalid importance in row {}", number + 2))
                })?
            };
            let deadline = match cell("deadline") {
                value if value.is_empty() => None,
                value => Some(value),
            };
            let column_name = cell("column_name");
            let column = if column_name.is_empty() {
                None
            } else {
                Some(
                    state
                        .columns
                        .iter()
                        .find(|c| c.board_id == board_id && c.name == column_name)
                        .ok_or_else(|| {
                            Error::Conflict(format!("Unknown column in row {}", number + 2))
                        })?
                        .id
                        .clone(),
                )
            };
            let id = if let Some(existing) = existing.filter(|_| mode != ImportMode::New) {
                if existing.board_id != board_id {
                    return Err(Error::Conflict(
                        "Matching ID belongs to another board".into(),
                    ));
                }
                state.apply(Command::PatchTicket {
                    id: existing.id.clone(),
                    base_revision: existing.revision,
                    fields: BTreeMap::from([
                        ("title".into(), json!(title)),
                        ("description".into(), json!(description)),
                        ("importance".into(), json!(importance)),
                        ("deadline".into(), json!(deadline)),
                    ]),
                })?;
                if existing.details.parent_id.is_none() {
                    let current = state.find_ticket(&existing.id)?;
                    state.apply(Command::MoveTicket {
                        id: existing.id.clone(),
                        base_revision: current.revision,
                        board_id: board_id.into(),
                        column_id: column,
                        activate_types: false,
                        convert_types: BTreeMap::new(),
                    })?;
                }
                updated += 1;
                existing.id
            } else {
                let result = state.apply(Command::CreateTicket {
                    board_id: board_id.into(),
                    title,
                    description,
                    importance,
                    column_id: column,
                    parent_id: None,
                    type_id: None,
                })?;
                created += 1;
                result.entity_id.ok_or(Error::Storage)?
            };
            if let Some(deadline) = deadline {
                let revision = state.find_ticket(&id)?.revision;
                state.apply(Command::PatchTicket {
                    id,
                    base_revision: revision,
                    fields: BTreeMap::from([("deadline".into(), json!(deadline))]),
                })?;
            }
        }
        let preview = save_preview(
            &tx,
            "csv_import",
            &state,
            &json!({}),
            json!({"created":created,"updated":updated,"skipped":skipped,"limitations":"CSV contains scalar root-ticket data, no hierarchy/comments/links/labels/custom fields; use JSON for a full roundtrip"}),
        )?;
        tx.commit().map_err(storage)?;
        Ok(preview)
    }
}

pub fn safe_cell(value: &str) -> String {
    if value.starts_with('\'')
        || value
            .trim_start_matches(|c: char| c.is_whitespace() || c.is_control())
            .starts_with(['=', '+', '-', '@'])
        || value.starts_with(['\t', '\r', '\n'])
    {
        format!("'{value}")
    } else {
        value.into()
    }
}
fn restore_cell(value: &str) -> String {
    value
        .strip_prefix('\'')
        .filter(|v| safe_cell(v) == value)
        .unwrap_or(value)
        .to_string()
}

fn merge_import(
    current: &Workspace,
    package: &BoardExport,
    mode: ImportMode,
    overrides: &BTreeMap<String, String>,
) -> Result<(Workspace, Value)> {
    let source = &package.workspace;
    let mut result = current.clone();
    let mut boards = BTreeMap::new();
    let mut columns = BTreeMap::new();
    let mut tickets = BTreeMap::new();
    let mut selected = BTreeSet::new();
    let mut created = 0;
    let mut updated = 0;
    let mut skipped = 0;
    for kind in &source.types {
        if let Some(index) = result.types.iter().position(|t| t.id == kind.id) {
            if mode == ImportMode::Update {
                result.types[index] = kind.clone();
            }
        } else {
            result.types.push(kind.clone());
        }
    }
    for label in &source.labels {
        if let Some(index) = result.labels.iter().position(|l| l.id == label.id) {
            if mode == ImportMode::Update {
                result.labels[index] = label.clone();
            }
        } else {
            result.labels.push(label.clone());
        }
    }
    for board in &source.boards {
        let existing = result
            .boards
            .iter()
            .position(|b| b.id == board.id || b.prefix == board.prefix);
        let destination = if let Some(index) = existing.filter(|_| mode != ImportMode::New) {
            if result.boards[index].prefix != board.prefix {
                return Err(Error::Conflict(
                    "Board UUID and prefix identify different boards".into(),
                ));
            }
            if mode == ImportMode::Update {
                result.boards[index].name = board.name.clone();
                result.boards[index].revision += 1;
            }
            result.boards[index].id.clone()
        } else {
            let prefix = overrides
                .get(&board.id)
                .cloned()
                .unwrap_or_else(|| board.prefix.clone());
            if result.issued_prefixes.contains(&prefix) {
                return Err(Error::Conflict(format!(
                    "Prefix {prefix} already issued; provide a new prefix override"
                )));
            }
            let key = if mode == ImportMode::New {
                Uuid::new_v4().to_string()
            } else {
                board.id.clone()
            };
            if result.boards.iter().any(|b| b.id == key) {
                return Err(Error::Conflict("Board ID collision".into()));
            }
            result.issued_prefixes.insert(prefix.clone());
            result.boards.push(Board {
                id: key.clone(),
                name: board.name.clone(),
                prefix,
                revision: 1,
                next_ticket_number: board.next_ticket_number.max(1),
            });
            result
                .board_options
                .insert(key.clone(), BoardOptions::default());
            key
        };
        if let Some(options) = source.board_options.get(&board.id) {
            let destination_options = result.board_options.entry(destination.clone()).or_default();
            if mode == ImportMode::Update {
                *destination_options = options.clone();
            } else {
                destination_options
                    .enabled_types
                    .extend(options.enabled_types.iter().cloned());
            }
        }
        boards.insert(board.id.clone(), destination);
    }
    for column in &source.columns {
        let board = boards
            .get(&column.board_id)
            .ok_or_else(|| Error::Validation("Column references missing source board".into()))?;
        let existing = result
            .columns
            .iter()
            .position(|c| c.board_id == *board && (c.id == column.id || c.name == column.name));
        let key = if let Some(index) = existing.filter(|_| mode != ImportMode::New) {
            let key = result.columns[index].id.clone();
            if mode == ImportMode::Update {
                let mut value = column.clone();
                value.id = key.clone();
                value.board_id = board.clone();
                value.position = result.columns[index].position;
                result.columns[index] = value;
            }
            key
        } else {
            let key = if mode == ImportMode::New {
                Uuid::new_v4().to_string()
            } else {
                column.id.clone()
            };
            if result.columns.iter().any(|c| c.id == key) {
                return Err(Error::Conflict("Column ID collision".into()));
            }
            let position = result
                .columns
                .iter()
                .filter(|c| c.board_id == *board)
                .map(|c| c.position)
                .max()
                .unwrap_or(-1)
                + 1;
            let mut value = column.clone();
            value.id = key.clone();
            value.board_id = board.clone();
            value.position = position;
            result.columns.push(value);
            key
        };
        columns.insert(column.id.clone(), key);
    }
    for ticket in &source.tickets {
        let existing = result
            .tickets
            .iter()
            .find(|t| t.id == ticket.id || t.public_id == ticket.public_id);
        if let Some(existing) = existing.filter(|_| mode != ImportMode::New) {
            if existing.id == ticket.id && existing.public_id != ticket.public_id {
                return Err(Error::Conflict("Ticket UUID/public ID collision".into()));
            }
            tickets.insert(ticket.id.clone(), existing.id.clone());
            if mode == ImportMode::Skip {
                skipped += 1;
            } else {
                updated += 1;
                selected.insert(ticket.id.clone());
            }
        } else {
            let key = if mode == ImportMode::New {
                Uuid::new_v4().to_string()
            } else {
                ticket.id.clone()
            };
            if result.tickets.iter().any(|t| t.id == key) {
                return Err(Error::Conflict("Ticket UUID collision".into()));
            }
            tickets.insert(ticket.id.clone(), key);
            selected.insert(ticket.id.clone());
            created += 1;
        }
    }
    for external in &package.external_references {
        let target = current
            .find_ticket(&external.id)
            .or_else(|_| current.find_ticket(&external.public_id))
            .map_err(|_| {
                Error::Conflict(format!(
                    "Unresolved external reference {}",
                    external.public_id
                ))
            })?;
        tickets
            .entry(external.id.clone())
            .or_insert(target.id.clone());
    }
    for original in source.tickets.iter().filter(|t| selected.contains(&t.id)) {
        let mut value = original.clone();
        let key = tickets.get(&original.id).ok_or(Error::Storage)?.clone();
        value.id = key.clone();
        value.board_id = boards
            .get(&original.board_id)
            .ok_or(Error::NotFound)?
            .clone();
        value.column_id = original
            .column_id
            .as_ref()
            .map(|id| columns.get(id).cloned().ok_or(Error::NotFound))
            .transpose()?;
        value.details.last_active_column = original
            .details
            .last_active_column
            .as_ref()
            .and_then(|id| columns.get(id).cloned());
        value.details.parent_id = original
            .details
            .parent_id
            .as_ref()
            .map(|id| tickets.get(id).cloned().ok_or(Error::NotFound))
            .transpose()?;
        if mode == ImportMode::New {
            let board = result
                .boards
                .iter_mut()
                .find(|b| b.id == value.board_id)
                .ok_or(Error::NotFound)?;
            value.public_id = format!("{}-{}", board.prefix, board.next_ticket_number);
            board.next_ticket_number += 1;
            value.revision = 1;
            value.details.created_at = engine::now();
            value.details.updated_at = value.details.created_at.clone();
        }
        let existing = result.tickets.iter().position(|t| t.id == key);
        if existing.is_none() && result.issued_ticket_ids.contains(&value.public_id) {
            return Err(Error::Conflict(
                "Import cannot resurrect a previously issued/deleted ticket ID".into(),
            ));
        }
        result.issued_ticket_ids.insert(value.public_id.clone());
        if let Some(index) = existing {
            value.revision = result.tickets[index].revision + 1;
            value.details.updated_at = engine::now();
            result.tickets[index] = value;
        } else {
            result.tickets.push(value);
        }
    }
    let changed: BTreeSet<_> = selected
        .iter()
        .filter_map(|id| tickets.get(id))
        .cloned()
        .collect();
    if mode == ImportMode::Update {
        result
            .dependencies
            .retain(|d| !changed.contains(&d.ticket_id));
        result.links.retain(|l| !changed.contains(&l.ticket_id));
    }
    for dependency in source
        .dependencies
        .iter()
        .filter(|d| selected.contains(&d.ticket_id))
    {
        result.dependencies.insert(Dependency {
            ticket_id: tickets
                .get(&dependency.ticket_id)
                .cloned()
                .ok_or(Error::NotFound)?,
            prerequisite_id: tickets
                .get(&dependency.prerequisite_id)
                .cloned()
                .ok_or_else(|| Error::Conflict("Unresolved dependency".into()))?,
        });
    }
    for link in source
        .links
        .iter()
        .filter(|l| selected.contains(&l.ticket_id))
    {
        result.links.insert(TicketLink {
            ticket_id: tickets
                .get(&link.ticket_id)
                .cloned()
                .ok_or(Error::NotFound)?,
            target_id: tickets
                .get(&link.target_id)
                .cloned()
                .ok_or_else(|| Error::Conflict("Unresolved link".into()))?,
        });
    }
    for comment in source
        .comments
        .iter()
        .filter(|c| selected.contains(&c.ticket_id))
    {
        let mut value = comment.clone();
        value.ticket_id = tickets
            .get(&comment.ticket_id)
            .cloned()
            .ok_or(Error::NotFound)?;
        if mode == ImportMode::New {
            value.id = Uuid::new_v4().to_string();
        }
        if let Some(index) = result.comments.iter().position(|c| c.id == value.id) {
            if result.comments[index].ticket_id != value.ticket_id {
                return Err(Error::Conflict("Comment ID collision".into()));
            }
            if mode == ImportMode::Update {
                result.comments[index] = value;
            }
        } else {
            result.comments.push(value);
        }
    }
    for rule in source
        .recurrences
        .iter()
        .filter(|r| selected.contains(&r.template_id))
    {
        let mut value = rule.clone();
        value.template_id = tickets
            .get(&rule.template_id)
            .cloned()
            .ok_or(Error::NotFound)?;
        value.last_created = rule
            .last_created
            .as_ref()
            .and_then(|id| tickets.get(id).cloned());
        if mode == ImportMode::New {
            value.id = Uuid::new_v4().to_string();
        }
        if let Some(index) = result.recurrences.iter().position(|r| r.id == value.id) {
            if mode == ImportMode::Update {
                result.recurrences[index] = value;
            }
        } else {
            result.recurrences.push(value);
        }
    }
    for board in &mut result.boards {
        let highest = result
            .tickets
            .iter()
            .filter_map(|t| {
                t.public_id
                    .strip_prefix(&format!("{}-", board.prefix))
                    .and_then(|n| n.parse::<i64>().ok())
            })
            .max()
            .unwrap_or(0);
        board.next_ticket_number = board.next_ticket_number.max(highest + 1);
    }
    result
        .issued_prefixes
        .extend(result.tickets.iter().filter_map(|t| {
            t.public_id
                .rsplit_once('-')
                .map(|(prefix, _)| prefix.to_string())
        }));
    result.validate()?;
    Ok((
        result,
        json!({"created":created,"updated":updated,"skipped":skipped,"mode":mode,"ticket_id_mapping":tickets,"board_id_mapping":boards}),
    ))
}
