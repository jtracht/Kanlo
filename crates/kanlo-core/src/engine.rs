//! Pure domain transitions. Callers commit the returned state atomically.
use crate::*;
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use uuid::Uuid;

fn invalid(message: &str) -> Error {
    Error::Validation(message.into())
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn entity_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(invalid("Invalid entity ID"));
    }
    Ok(())
}
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn color(value: &str) -> Result<()> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|v| v.is_ascii_hexdigit())
    {
        return Err(invalid("Color must be #RRGGBB"));
    }
    Ok(())
}

impl ViewState {
    pub fn validate(&self) -> Result<()> {
        if self.query.len() > 4096
            || self.custom_fields.len() > 100
            || !self.scroll_x.is_finite()
            || !self.scroll_y.is_finite()
            || self
                .importance_min
                .zip(self.importance_max)
                .is_some_and(|(min, max)| min > max)
        {
            return Err(invalid("Invalid view filter"));
        }
        for date in [&self.deadline_from, &self.deadline_to]
            .into_iter()
            .flatten()
        {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.pred_opt())
                .is_none()
            {
                return Err(invalid("Filter date must be YYYY-MM-DD"));
            }
        }
        if self
            .deadline_from
            .as_ref()
            .zip(self.deadline_to.as_ref())
            .is_some_and(|(from, to)| from > to)
        {
            return Err(invalid("Deadline filter range is reversed"));
        }
        for (name, value) in &self.custom_fields {
            text(name, 120, "custom field name")?;
            if value.len() > 16384 {
                return Err(invalid("Custom field filter too long"));
            }
        }
        Ok(())
    }
}
fn filter_start(date: &str, timezone: &str) -> Option<DateTime<Utc>> {
    let previous = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()?
        .pred_opt()?;
    Some(
        crate::sync::deadline(&Some(previous.to_string()), timezone)?
            + chrono::Duration::nanoseconds(1),
    )
}

impl Workspace {
    pub fn initial() -> Self {
        let mut state = Self::default();
        let created = state
            .apply(Command::CreateBoard {
                name: "Kanlo".into(),
                prefix: "KAN".into(),
            })
            .expect("valid initial board");
        let board = created.entity_id.expect("created board");
        for (name, done) in [("Offen", false), ("In Arbeit", false), ("Erledigt", true)] {
            state
                .apply(Command::CreateColumn {
                    board_id: board.clone(),
                    name: name.into(),
                    is_done: done,
                    wip_limit: None,
                    wip_strict: false,
                })
                .expect("valid initial column");
        }
        state
    }

    pub fn find_ticket(&self, key: &str) -> Result<&Ticket> {
        self.tickets
            .iter()
            .find(|t| t.id == key || t.public_id == key)
            .ok_or(Error::NotFound)
    }
    fn ticket_index(&self, key: &str) -> Result<usize> {
        self.tickets
            .iter()
            .position(|t| t.id == key || t.public_id == key)
            .ok_or(Error::NotFound)
    }
    fn board_index(&self, key: &str) -> Result<usize> {
        self.boards
            .iter()
            .position(|b| b.id == key)
            .ok_or(Error::NotFound)
    }
    fn active_ticket(&self, key: &str) -> Result<usize> {
        let index = self.ticket_index(key)?;
        if self.tickets[index].details.trashed {
            return Err(Error::Conflict("Ticket is in trash".into()));
        }
        Ok(index)
    }
    fn revision(&self, index: usize, expected: i64) -> Result<()> {
        if self.tickets[index].revision != expected {
            return Err(Error::Conflict("Stale revision".into()));
        }
        Ok(())
    }
    fn mark(&mut self, index: usize, result: &mut ChangeResult) {
        let ticket = &mut self.tickets[index];
        ticket.revision += 1;
        ticket.details.updated_at = now();
        result.affected.insert(ticket.id.clone());
        result.boards.insert(ticket.board_id.clone());
    }
    fn destination(
        &self,
        board: &str,
        column: Option<&str>,
        exclude: &BTreeSet<String>,
    ) -> Result<bool> {
        self.board_index(board)?;
        let Some(column) = column else {
            return Ok(false);
        };
        let column = self
            .columns
            .iter()
            .find(|c| c.id == column)
            .ok_or(Error::NotFound)?;
        if column.board_id != board {
            return Err(invalid("Column belongs to another board"));
        }
        if column.wip_strict
            && let Some(limit) = column.wip_limit
        {
            let count = self
                .tickets
                .iter()
                .filter(|t| {
                    t.column_id.as_deref() == Some(&column.id)
                        && t.details.parent_id.is_none()
                        && !t.details.trashed
                        && !t.details.archived
                        && !exclude.contains(&t.id)
                })
                .count();
            if count >= limit as usize {
                return Err(Error::WipLimit);
            }
        }
        Ok(column.is_done)
    }
    fn family(&self, index: usize) -> Vec<usize> {
        let id = &self.tickets[index].id;
        std::iter::once(index)
            .chain(
                self.tickets
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.details.parent_id.as_ref() == Some(id))
                    .map(|(i, _)| i),
            )
            .collect()
    }
    fn normalize_ids(&self, ids: &[String]) -> Result<BTreeSet<String>> {
        if ids.is_empty() || ids.len() > 10000 {
            return Err(invalid("Choose 1–10000 tickets"));
        }
        let mut selected = BTreeSet::new();
        for key in ids {
            let index = self.ticket_index(key)?;
            for index in self.family(index) {
                selected.insert(self.tickets[index].id.clone());
            }
        }
        Ok(selected)
    }
    fn complete_group(
        &mut self,
        indices: &[usize],
        done: bool,
        result: &mut ChangeResult,
    ) -> Result<()> {
        let selected: BTreeSet<_> = indices
            .iter()
            .map(|i| self.tickets[*i].id.clone())
            .collect();
        if done {
            for dependency in &self.dependencies {
                if selected.contains(&dependency.ticket_id)
                    && !selected.contains(&dependency.prerequisite_id)
                    && !self.find_ticket(&dependency.prerequisite_id)?.done
                {
                    return Err(Error::Blocked);
                }
            }
        }
        for &index in indices {
            if self.tickets[index].done != done {
                self.tickets[index].done = done;
                self.tickets[index].details.completed_at = if done { Some(now()) } else { None };
                self.mark(index, result);
            }
        }
        if !done {
            self.reopen_dependents(&selected, result)?;
        }
        Ok(())
    }
    fn reopen_dependents(
        &mut self,
        roots: &BTreeSet<String>,
        result: &mut ChangeResult,
    ) -> Result<()> {
        let mut queue: VecDeque<String> = roots.iter().cloned().collect();
        let mut visited = BTreeSet::new();
        while let Some(root) = queue.pop_front() {
            if !visited.insert(root.clone()) {
                continue;
            }
            let dependents: Vec<String> = self
                .dependencies
                .iter()
                .filter(|d| d.prerequisite_id == root)
                .map(|d| d.ticket_id.clone())
                .collect();
            for key in dependents {
                queue.push_back(key.clone());
                let index = self.active_ticket(&key)?;
                if !self.tickets[index].done {
                    continue;
                }
                self.reopen_at(index, result)?;
                // A child is binary, but a completed parent cannot contain an open child.
                if let Some(parent) = self.tickets[index].details.parent_id.clone() {
                    let p = self.active_ticket(&parent)?;
                    if self.tickets[p].done {
                        self.reopen_at(p, result)?;
                        queue.push_back(parent);
                    }
                }
            }
        }
        Ok(())
    }
    fn reopen_at(&mut self, index: usize, result: &mut ChangeResult) -> Result<()> {
        self.tickets[index].done = false;
        self.tickets[index].details.completed_at = None;
        if self.tickets[index].details.parent_id.is_none() {
            let old = self.tickets[index].details.last_active_column.clone();
            let board = self.tickets[index].board_id.clone();
            let exclude = BTreeSet::from([self.tickets[index].id.clone()]);
            let target = old.filter(|id| {
                self.columns
                    .iter()
                    .any(|c| c.id == *id && !c.is_done && c.board_id == board)
                    && self.destination(&board, Some(id), &exclude).is_ok()
            });
            if target.is_none() {
                result.warnings.push(format!(
                    "{} reopened into backlog",
                    self.tickets[index].public_id
                ));
            }
            self.tickets[index].column_id = target.clone();
            for child in self.family(index).into_iter().skip(1) {
                self.tickets[child].column_id = target.clone();
                self.mark(child, result);
            }
        }
        self.mark(index, result);
        Ok(())
    }

    /// Failed commands leave the caller's aggregate unchanged.
    pub fn apply(&mut self, command: Command) -> Result<ChangeResult> {
        let mut candidate = self.clone();
        let mut result = ChangeResult::default();
        candidate.transition(command, &mut result, 0)?;
        candidate.validate()?;
        for column in &candidate.columns {
            if !column.wip_strict
                && let Some(limit) = column.wip_limit
            {
                let count = candidate
                    .tickets
                    .iter()
                    .filter(|t| {
                        t.column_id.as_deref() == Some(&column.id)
                            && t.details.parent_id.is_none()
                            && !t.details.trashed
                            && !t.details.archived
                    })
                    .count();
                if count > limit as usize {
                    result
                        .warnings
                        .push(format!("WIP warning: {} ({count}/{limit})", column.name));
                }
            }
        }
        *self = candidate;
        Ok(result)
    }

    fn transition(
        &mut self,
        command: Command,
        result: &mut ChangeResult,
        depth: usize,
    ) -> Result<()> {
        if depth > 2 {
            return Err(invalid("Nested batch limit exceeded"));
        }
        match command {
            Command::CreateBoard { name, prefix } => {
                let name = text(&name, 120, "name")?;
                let prefix = board_prefix(&prefix)?;
                if !self.issued_prefixes.insert(prefix.clone()) {
                    return Err(Error::Conflict("Board prefix was already issued".into()));
                }
                let key = id();
                self.boards.push(Board {
                    id: key.clone(),
                    name,
                    prefix,
                    revision: 1,
                    next_ticket_number: 1,
                });
                self.board_options
                    .insert(key.clone(), BoardOptions::default());
                result.boards.insert(key.clone());
                result.entity_id = Some(key);
            }
            Command::UpdateBoard {
                id,
                name,
                base_revision,
            } => {
                let index = self.board_index(&id)?;
                if self.boards[index].revision != base_revision {
                    return Err(Error::Conflict("Stale board revision".into()));
                }
                self.boards[index].name = text(&name, 120, "name")?;
                self.boards[index].revision += 1;
                result.boards.insert(id.clone());
                result.entity_id = Some(id);
            }
            Command::DeleteBoard { id, base_revision } => {
                let index = self.board_index(&id)?;
                if self.boards[index].revision != base_revision {
                    return Err(Error::Conflict("Stale board revision".into()));
                }
                if self.tickets.iter().any(|t| t.board_id == id) {
                    return Err(Error::Conflict(
                        "Board still contains tickets, including archive/trash/backlog".into(),
                    ));
                }
                self.columns.retain(|c| c.board_id != id);
                self.boards.remove(index);
                self.board_options.remove(&id);
                self.views.retain(|_, v| v.board_id != id);
                result.boards.insert(id);
            }
            Command::CreateColumn {
                board_id,
                name,
                is_done,
                wip_limit,
                wip_strict,
            } => {
                self.board_index(&board_id)?;
                let key = id();
                let position = self
                    .columns
                    .iter()
                    .filter(|c| c.board_id == board_id)
                    .map(|c| c.position)
                    .max()
                    .unwrap_or(-1)
                    + 1;
                self.columns.push(Column {
                    id: key.clone(),
                    board_id: board_id.clone(),
                    name: text(&name, 120, "name")?,
                    position,
                    is_done,
                    wip_limit,
                    wip_strict,
                });
                result.boards.insert(board_id);
                result.entity_id = Some(key);
            }
            Command::UpdateColumn {
                id,
                name,
                is_done,
                wip_limit,
                wip_strict,
                color: tint,
            } => {
                color(&tint)?;
                let column = self
                    .columns
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or(Error::NotFound)?;
                // Reclassifying a populated column would silently complete/reopen many tickets.
                if column.is_done != is_done
                    && self
                        .tickets
                        .iter()
                        .any(|t| t.column_id.as_ref() == Some(&id))
                {
                    return Err(Error::Conflict(
                        "Change status only on an empty column".into(),
                    ));
                }
                column.name = text(&name, 120, "name")?;
                column.is_done = is_done;
                column.wip_limit = wip_limit;
                column.wip_strict = wip_strict;
                self.board_options
                    .entry(column.board_id.clone())
                    .or_default()
                    .column_colors
                    .insert(id.clone(), tint);
                result.boards.insert(column.board_id.clone());
                result.entity_id = Some(id);
            }
            Command::DeleteColumn { id } => {
                let column = self
                    .columns
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or(Error::NotFound)?;
                if self
                    .tickets
                    .iter()
                    .any(|t| t.column_id.as_ref() == Some(&id))
                {
                    return Err(Error::Conflict(
                        "Column still contains tickets, including archive/trash".into(),
                    ));
                }
                result.boards.insert(column.board_id.clone());
                for t in &mut self.tickets {
                    if t.details.last_active_column.as_ref() == Some(&id) {
                        t.details.last_active_column = None;
                    }
                }
                self.columns.retain(|c| c.id != id);
                for options in self.board_options.values_mut() {
                    options.column_colors.remove(&id);
                }
            }
            Command::ReorderColumns { board_id, ids } => {
                self.board_index(&board_id)?;
                let actual: BTreeSet<_> = self
                    .columns
                    .iter()
                    .filter(|c| c.board_id == board_id)
                    .map(|c| c.id.clone())
                    .collect();
                if ids.len() != actual.len()
                    || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                    || ids.iter().cloned().collect::<BTreeSet<_>>() != actual
                {
                    return Err(invalid("Reorder must contain each column once"));
                }
                for (position, id) in ids.into_iter().enumerate() {
                    self.columns
                        .iter_mut()
                        .find(|c| c.id == id)
                        .ok_or(Error::NotFound)?
                        .position = position as i64;
                }
                result.boards.insert(board_id);
            }
            Command::CreateTicket {
                board_id,
                title,
                description,
                importance,
                mut column_id,
                parent_id,
                type_id,
            } => {
                let board_index = self.board_index(&board_id)?;
                let done;
                if let Some(parent) = &parent_id {
                    let index = self.active_ticket(parent)?;
                    let p = &self.tickets[index];
                    if p.details.parent_id.is_some() || p.board_id != board_id || p.details.archived
                    {
                        return Err(invalid("Children require an active root on the same board"));
                    }
                    if let Some(type_id) = &p.details.type_id
                        && !self
                            .types
                            .iter()
                            .any(|t| &t.id == type_id && t.allow_children)
                    {
                        return Err(invalid("Parent type does not permit children"));
                    }
                    column_id = p.column_id.clone();
                    done = p.done;
                } else {
                    done = self.destination(&board_id, column_id.as_deref(), &BTreeSet::new())?;
                }
                if description.len() > 65536 {
                    return Err(invalid("Description exceeds 64 KiB"));
                }
                self.validate_type(&board_id, type_id.as_deref())?;
                let board = &mut self.boards[board_index];
                let public_id = format!("{}-{}", board.prefix, board.next_ticket_number);
                board.next_ticket_number = board
                    .next_ticket_number
                    .checked_add(1)
                    .ok_or_else(|| invalid("Ticket counter exhausted"))?;
                board.revision += 1;
                if !self.issued_ticket_ids.insert(public_id.clone()) {
                    return Err(Error::Conflict("Public ID already issued".into()));
                }
                let key = id();
                let stamp = now();
                let rank = self
                    .tickets
                    .iter()
                    .filter(|t| t.board_id == board_id && t.column_id == column_id)
                    .map(|t| t.details.rank)
                    .max()
                    .unwrap_or(0)
                    + 1024;
                let last_active_column = if !done { column_id.clone() } else { None };
                self.tickets.push(Ticket {
                    id: key.clone(),
                    public_id,
                    board_id: board_id.clone(),
                    column_id,
                    title: text(&title, 300, "title")?,
                    description,
                    importance,
                    done,
                    revision: 1,
                    details: TicketDetails {
                        completed_at: if done { Some(stamp.clone()) } else { None },
                        parent_id,
                        type_id,
                        rank,
                        last_active_column,
                        created_at: stamp.clone(),
                        updated_at: stamp,
                        ..Default::default()
                    },
                });
                result.entity_id = Some(key.clone());
                result.affected.insert(key);
                result.boards.insert(board_id);
            }
            Command::PatchTicket {
                id,
                base_revision,
                fields,
            } => {
                let index = self.active_ticket(&id)?;
                self.revision(index, base_revision)?;
                let old = self.tickets[index].clone();
                for (key, value) in fields {
                    let t = &mut self.tickets[index];
                    match key.as_str() {
                        "title" => {
                            t.title = text(
                                value
                                    .as_str()
                                    .ok_or_else(|| invalid("title must be text"))?,
                                300,
                                "title",
                            )?
                        }
                        "description" => {
                            let value = value
                                .as_str()
                                .ok_or_else(|| invalid("description must be text"))?;
                            if value.len() > 65536 {
                                return Err(invalid("Description exceeds 64 KiB"));
                            }
                            t.description = value.into();
                        }
                        "importance" => {
                            t.importance = value
                                .as_u64()
                                .and_then(|v| u8::try_from(v).ok())
                                .ok_or_else(|| invalid("importance must be 0–255"))?
                        }
                        "type_id" => {
                            t.details.type_id = serde_json::from_value(value)
                                .map_err(|_| invalid("Invalid type_id"))?
                        }
                        "labels" => {
                            t.details.labels = serde_json::from_value(value)
                                .map_err(|_| invalid("Invalid labels"))?
                        }
                        "custom_fields" => {
                            t.details.custom_fields = serde_json::from_value(value)
                                .map_err(|_| invalid("Custom fields must be a name/text map"))?
                        }
                        "deadline" => {
                            t.details.deadline = serde_json::from_value(value)
                                .map_err(|_| invalid("Invalid deadline"))?
                        }
                        _ => {
                            return Err(invalid(
                                "Unsupported ticket field; use an explicit structural command",
                            ));
                        }
                    }
                }
                if serde_json::to_value(&old).ok()
                    != serde_json::to_value(&self.tickets[index]).ok()
                {
                    self.mark(index, result);
                }
                result.entity_id = Some(old.id);
            }
            Command::MoveTicket {
                id,
                base_revision,
                board_id,
                column_id,
                activate_types,
                convert_types,
            } => {
                let index = self.active_ticket(&id)?;
                self.revision(index, base_revision)?;
                if self.tickets[index].details.parent_id.is_some() {
                    return Err(invalid("Children cannot move independently"));
                }
                if self.tickets[index].details.archived {
                    return Err(Error::Conflict("Unarchive before moving".into()));
                }
                let family = self.family(index);
                let excluded: BTreeSet<_> =
                    family.iter().map(|i| self.tickets[*i].id.clone()).collect();
                let done = self.destination(&board_id, column_id.as_deref(), &excluded)?;
                if self.tickets[index].board_id == board_id
                    && self.tickets[index].column_id == column_id
                {
                    result.entity_id = Some(self.tickets[index].id.clone());
                    return Ok(());
                }
                for &i in &family {
                    if let Some(new_type) = convert_types.get(&self.tickets[i].id) {
                        self.tickets[i].details.type_id = new_type.clone();
                    }
                    if let Some(type_id) = self.tickets[i].details.type_id.clone() {
                        if activate_types && self.types.iter().any(|t| t.id == type_id) {
                            self.board_options
                                .entry(board_id.clone())
                                .or_default()
                                .enabled_types
                                .insert(type_id.clone());
                        }
                        self.validate_type(&board_id, Some(&type_id))?;
                    }
                    result.boards.insert(self.tickets[i].board_id.clone());
                    if !self.tickets[i].done {
                        self.tickets[i].details.last_active_column =
                            self.tickets[i].column_id.clone();
                    }
                    self.tickets[i].board_id = board_id.clone();
                    self.tickets[i].column_id = column_id.clone();
                    if !done {
                        self.tickets[i].details.last_active_column = column_id.clone();
                    }
                    self.mark(i, result);
                }
                if done {
                    self.complete_group(&family, true, result)?;
                } else {
                    self.complete_group(&[index], false, result)?;
                }
                result.entity_id = Some(self.tickets[index].id.clone());
            }
            Command::SetDone {
                id,
                base_revision,
                done,
            } => {
                let index = self.active_ticket(&id)?;
                self.revision(index, base_revision)?;
                if self.tickets[index].details.parent_id.is_none() {
                    if done {
                        let board = self.tickets[index].board_id.clone();
                        let destination = self
                            .columns
                            .iter()
                            .find(|c| c.board_id == board && c.is_done)
                            .map(|c| c.id.clone())
                            .ok_or_else(|| invalid("Board has no Done column"))?;
                        return self.transition(
                            Command::MoveTicket {
                                id,
                                base_revision,
                                board_id: board,
                                column_id: Some(destination),
                                activate_types: false,
                                convert_types: BTreeMap::new(),
                            },
                            result,
                            depth,
                        );
                    }
                    self.reopen_at(index, result)?;
                    self.reopen_dependents(
                        &BTreeSet::from([self.tickets[index].id.clone()]),
                        result,
                    )?;
                } else {
                    if !done && let Some(parent) = self.tickets[index].details.parent_id.clone() {
                        let p = self.active_ticket(&parent)?;
                        if self.tickets[p].done {
                            return Err(Error::Conflict(
                                "Reopen parent before reopening child".into(),
                            ));
                        }
                    }
                    self.complete_group(&[index], done, result)?;
                }
                result.entity_id = Some(self.tickets[index].id.clone());
            }
            Command::ReorderTickets {
                board_id,
                column_id,
                ids,
            } => {
                let actual: BTreeSet<_> = self
                    .tickets
                    .iter()
                    .filter(|t| {
                        t.board_id == board_id
                            && t.column_id == column_id
                            && t.details.parent_id.is_none()
                            && !t.details.trashed
                            && !t.details.archived
                    })
                    .map(|t| t.id.clone())
                    .collect();
                if actual != ids.iter().cloned().collect() || actual.len() != ids.len() {
                    return Err(invalid("Reorder must include each active root ticket once"));
                }
                for (rank, key) in ids.into_iter().enumerate() {
                    let i = self.ticket_index(&key)?;
                    self.tickets[i].details.rank = rank as i64 * 1024;
                    self.mark(i, result);
                }
                result.boards.insert(board_id);
            }
            Command::UpsertType { mut value } => {
                value.name = text(&value.name, 80, "type name")?;
                color(&value.color)?;
                text(&value.icon, 64, "icon")?;
                if value.id.is_empty() {
                    value.id = id();
                }
                if let Some(index) = self.types.iter().position(|t| t.id == value.id) {
                    self.types[index] = value.clone();
                } else {
                    self.types.push(value.clone());
                }
                result.entity_id = Some(value.id);
                result
                    .boards
                    .extend(self.boards.iter().map(|b| b.id.clone()));
            }
            Command::DeleteType { id } => {
                if !self.types.iter().any(|t| t.id == id) {
                    return Err(Error::NotFound);
                }
                if self
                    .tickets
                    .iter()
                    .any(|t| t.details.type_id.as_ref() == Some(&id))
                {
                    return Err(Error::Conflict("Ticket type is still referenced".into()));
                }
                self.types.retain(|t| t.id != id);
                for options in self.board_options.values_mut() {
                    options.enabled_types.remove(&id);
                }
                result
                    .boards
                    .extend(self.boards.iter().map(|b| b.id.clone()));
            }
            Command::UpsertLabel { mut value } => {
                value.name = text(&value.name, 80, "label name")?;
                color(&value.color)?;
                if value.id.is_empty() {
                    value.id = id();
                }
                if let Some(index) = self.labels.iter().position(|l| l.id == value.id) {
                    self.labels[index] = value.clone();
                } else {
                    self.labels.push(value.clone());
                }
                result.entity_id = Some(value.id);
                result
                    .boards
                    .extend(self.boards.iter().map(|b| b.id.clone()));
            }
            Command::DeleteLabel { id } => {
                if !self.labels.iter().any(|l| l.id == id) {
                    return Err(Error::NotFound);
                }
                self.labels.retain(|l| l.id != id);
                for i in 0..self.tickets.len() {
                    if self.tickets[i].details.labels.remove(&id) {
                        self.mark(i, result);
                    }
                }
            }
            Command::SetBoardOptions { board_id, options } => {
                self.board_index(&board_id)?;
                if options.undo_depth > 1000 {
                    return Err(invalid("Undo depth must be 0–1000"));
                }
                color(&options.color)?;
                for value in options.column_colors.values() {
                    color(value)?;
                }
                self.board_options.insert(board_id.clone(), options);
                result.boards.insert(board_id);
            }
            Command::AddDependency {
                ticket_id,
                prerequisite_id,
            } => {
                let dependent = self.active_ticket(&ticket_id)?;
                let prerequisite = self.active_ticket(&prerequisite_id)?;
                let ticket_id = self.tickets[dependent].id.clone();
                let prerequisite_id = self.tickets[prerequisite].id.clone();
                if ticket_id == prerequisite_id {
                    return Err(invalid("Self dependency"));
                }
                self.dependencies.insert(Dependency {
                    ticket_id,
                    prerequisite_id,
                });
                self.mark(dependent, result);
            }
            Command::RemoveDependency {
                ticket_id,
                prerequisite_id,
            } => {
                let dependent = self.active_ticket(&ticket_id)?;
                let prerequisite = self.active_ticket(&prerequisite_id)?;
                self.dependencies.remove(&Dependency {
                    ticket_id: self.tickets[dependent].id.clone(),
                    prerequisite_id: self.tickets[prerequisite].id.clone(),
                });
                self.mark(dependent, result);
            }
            Command::AddLink {
                ticket_id,
                target_id,
            } => {
                let i = self.active_ticket(&ticket_id)?;
                let target = self.active_ticket(&target_id)?;
                if i == target {
                    return Err(invalid("Self link"));
                }
                self.links.insert(TicketLink {
                    ticket_id: self.tickets[i].id.clone(),
                    target_id: self.tickets[target].id.clone(),
                });
                self.mark(i, result);
            }
            Command::RemoveLink {
                ticket_id,
                target_id,
            } => {
                self.links.remove(&TicketLink {
                    ticket_id,
                    target_id,
                });
            }
            Command::AddComment {
                ticket_id,
                markdown,
            } => {
                let i = self.active_ticket(&ticket_id)?;
                let key = id();
                self.comments.push(Comment {
                    id: key.clone(),
                    ticket_id: self.tickets[i].id.clone(),
                    markdown: text(&markdown, 65536, "comment")?,
                    created_at: now(),
                });
                self.mark(i, result);
                result.entity_id = Some(key);
            }
            Command::DeleteComment { id } => {
                let index = self
                    .comments
                    .iter()
                    .position(|c| c.id == id)
                    .ok_or(Error::NotFound)?;
                let ticket = self.comments.remove(index).ticket_id;
                let i = self.active_ticket(&ticket)?;
                self.mark(i, result);
            }
            Command::Trash { ids } => {
                let selected = self.normalize_ids(&ids)?;
                for i in 0..self.tickets.len() {
                    if selected.contains(&self.tickets[i].id) {
                        self.tickets[i].details.trashed = true;
                        self.tickets[i].details.deleted_at = Some(now());
                        self.mark(i, result);
                    }
                }
                let affected: Vec<String> = self
                    .dependencies
                    .iter()
                    .filter(|d| {
                        selected.contains(&d.ticket_id) || selected.contains(&d.prerequisite_id)
                    })
                    .map(|d| d.ticket_id.clone())
                    .collect();
                self.dependencies.retain(|d| {
                    !selected.contains(&d.ticket_id) && !selected.contains(&d.prerequisite_id)
                });
                self.links.retain(|l| {
                    !selected.contains(&l.ticket_id) && !selected.contains(&l.target_id)
                });
                self.recurrences
                    .retain(|r| !selected.contains(&r.template_id));
                for id in affected {
                    let i = self.ticket_index(&id)?;
                    if !selected.contains(&id) {
                        self.mark(i, result);
                    }
                }
            }
            Command::Restore { ids } => {
                let selected = self.normalize_ids(&ids)?;
                for i in 0..self.tickets.len() {
                    if selected.contains(&self.tickets[i].id) {
                        self.tickets[i].details.trashed = false;
                        self.tickets[i].details.deleted_at = None;
                        self.mark(i, result);
                    }
                }
            }
            Command::Purge { ids } => {
                let selected = self.normalize_ids(&ids)?;
                if self
                    .tickets
                    .iter()
                    .any(|t| selected.contains(&t.id) && !t.details.trashed)
                {
                    return Err(invalid("Only trash can be permanently purged"));
                }
                for t in self.tickets.iter().filter(|t| selected.contains(&t.id)) {
                    result.affected.insert(t.id.clone());
                    result.boards.insert(t.board_id.clone());
                }
                self.comments.retain(|c| !selected.contains(&c.ticket_id));
                self.tickets.retain(|t| !selected.contains(&t.id));
            }
            Command::Archive { ids, archived } => {
                let selected = self.normalize_ids(&ids)?;
                if archived
                    && self
                        .tickets
                        .iter()
                        .any(|t| selected.contains(&t.id) && (!t.done || t.details.trashed))
                {
                    return Err(invalid("Only completed active tickets can be archived"));
                }
                for i in 0..self.tickets.len() {
                    if selected.contains(&self.tickets[i].id) {
                        self.tickets[i].details.archived = archived;
                        self.mark(i, result);
                    }
                }
            }
            Command::CloneTicket { id } => {
                let index = self.active_ticket(&id)?;
                if self.tickets[index].details.parent_id.is_some() {
                    return Err(invalid("Clone the root ticket"));
                }
                let original = self.tickets[index].clone();
                self.transition(
                    Command::CreateTicket {
                        board_id: original.board_id.clone(),
                        title: format!(
                            "{} (copy)",
                            original.title.chars().take(290).collect::<String>()
                        ),
                        description: original.description.clone(),
                        importance: original.importance,
                        column_id: None,
                        parent_id: None,
                        type_id: original.details.type_id.clone(),
                    },
                    result,
                    depth,
                )?;
                let root = result.entity_id.clone().ok_or(Error::Storage)?;
                let new_i = self.ticket_index(&root)?;
                self.tickets[new_i].details.labels = original.details.labels.clone();
                self.tickets[new_i].details.custom_fields = original.details.custom_fields.clone();
                self.tickets[new_i].details.deadline = original.details.deadline.clone();
                let children: Vec<_> = self
                    .family(index)
                    .into_iter()
                    .skip(1)
                    .map(|i| self.tickets[i].clone())
                    .collect();
                for child in children {
                    self.transition(
                        Command::CreateTicket {
                            board_id: child.board_id,
                            title: child.title,
                            description: child.description,
                            importance: child.importance,
                            column_id: None,
                            parent_id: Some(root.clone()),
                            type_id: child.details.type_id.clone(),
                        },
                        result,
                        depth,
                    )?;
                    let child_i =
                        self.ticket_index(result.entity_id.as_ref().ok_or(Error::Storage)?)?;
                    self.tickets[child_i].details.labels = child.details.labels;
                    self.tickets[child_i].details.custom_fields = child.details.custom_fields;
                    self.tickets[child_i].details.deadline = child.details.deadline;
                }
                result.entity_id = Some(root);
            }
            Command::SetSettings { settings } => {
                self.settings = settings;
            }
            Command::SetView { context, view } => {
                if !(context.starts_with("web:") || context.starts_with("tv:"))
                    || context.len() > 160
                {
                    return Err(invalid(
                        "View context must identify web:client or tv:display",
                    ));
                }
                self.board_index(&view.board_id)?;
                view.validate()?;
                if ![
                    "manual",
                    "title",
                    "importance",
                    "score",
                    "deadline",
                    "type",
                    "created",
                    "updated",
                    "public_id",
                ]
                .contains(&view.sort.as_str())
                    && !view.sort.starts_with("custom:")
                {
                    return Err(invalid("Unknown sorting field"));
                }
                if ![
                    "active", "all", "done", "open", "archive", "trash", "blocked", "backlog",
                    "overdue",
                ]
                .contains(&view.status.as_str())
                {
                    return Err(invalid("Unknown status filter"));
                }
                if !view.scroll_x.is_finite() || !view.scroll_y.is_finite() {
                    return Err(invalid("Invalid scroll position"));
                }
                self.views.insert(context, view);
            }
            Command::UpsertRecurrence { mut value } => {
                if value.anchor_at.is_none() {
                    value.anchor_at = Some(value.next_at.clone());
                }
                let i = self.active_ticket(&value.template_id)?;
                if self.tickets[i].details.parent_id.is_some() {
                    return Err(invalid("Recurrence requires a root ticket"));
                }
                crate::sync::validate_recurrence(&value)?;
                if value.id.is_empty() {
                    value.id = id();
                }
                if let Some(index) = self.recurrences.iter().position(|r| r.id == value.id) {
                    self.recurrences[index] = value.clone();
                } else {
                    self.recurrences.push(value.clone());
                }
                result.entity_id = Some(value.id);
                result.boards.insert(self.tickets[i].board_id.clone());
            }
            Command::DeleteRecurrence { id } => {
                self.recurrences.retain(|r| r.id != id);
            }
            Command::Batch { commands } => {
                if commands.is_empty() || commands.len() > 1000 {
                    return Err(invalid("Batch requires 1–1000 commands"));
                }
                for command in commands {
                    self.transition(command, result, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn validate_type(&self, board: &str, type_id: Option<&str>) -> Result<()> {
        if let Some(type_id) = type_id {
            if !self.types.iter().any(|t| t.id == type_id) {
                return Err(Error::NotFound);
            }
            if !self
                .board_options
                .get(board)
                .is_some_and(|o| o.enabled_types.contains(type_id))
            {
                return Err(Error::Conflict(
                    "Activate type on target board or explicitly convert each affected ticket"
                        .into(),
                ));
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        let mut public = BTreeSet::new();
        let mut prefixes = BTreeSet::new();
        for b in &self.boards {
            entity_id(&b.id)?;
            text(&b.name, 120, "board name")?;
            board_prefix(&b.prefix)?;
            if !ids.insert(&b.id) || !prefixes.insert(&b.prefix) || b.next_ticket_number < 1 {
                return Err(invalid("Duplicate board or invalid counter"));
            }
        }
        let mut column_ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for c in &self.columns {
            entity_id(&c.id)?;
            self.board_index(&c.board_id)?;
            text(&c.name, 120, "column name")?;
            if !column_ids.insert(&c.id) || !positions.insert((&c.board_id, c.position)) {
                return Err(invalid("Duplicate column or position"));
            }
        }
        let mut ticket_ids = BTreeSet::new();
        for t in &self.tickets {
            entity_id(&t.id)?;
            let (prefix, number) = t
                .public_id
                .rsplit_once('-')
                .ok_or_else(|| invalid("Invalid public ticket ID"))?;
            board_prefix(prefix)?;
            if number.parse::<u64>().is_err() || number.starts_with('0') {
                return Err(invalid("Invalid public ticket number"));
            }
            self.board_index(&t.board_id)?;
            if !ticket_ids.insert(&t.id) || !public.insert(&t.public_id) || t.revision < 1 {
                return Err(invalid("Duplicate ticket identity or invalid revision"));
            }
            text(&t.title, 300, "title")?;
            if t.description.len() > 65536 || t.details.custom_fields.len() > 100 {
                return Err(invalid("Ticket size limit exceeded"));
            }
            for (name, value) in &t.details.custom_fields {
                text(name, 120, "custom field name")?;
                if value.len() > 16384 {
                    return Err(invalid("Custom field value exceeds 16 KiB"));
                }
            }
            self.validate_type(&t.board_id, t.details.type_id.as_deref())?;
            if t.details
                .labels
                .iter()
                .any(|id| !self.labels.iter().any(|l| &l.id == id))
            {
                return Err(invalid("Unknown label"));
            }
            if let Some(deadline) = &t.details.deadline
                && NaiveDate::parse_from_str(deadline, "%Y-%m-%d").is_err()
                && DateTime::parse_from_rfc3339(deadline).is_err()
            {
                return Err(invalid(
                    "Deadline must be an ISO date or timestamp with offset",
                ));
            }
            if let Some(column) = &t.column_id {
                let column = self
                    .columns
                    .iter()
                    .find(|c| &c.id == column)
                    .ok_or(Error::NotFound)?;
                if column.board_id != t.board_id {
                    return Err(invalid("Ticket/column board mismatch"));
                }
                if t.details.parent_id.is_none() && column.is_done != t.done {
                    return Err(invalid("Root status must agree with its column"));
                }
            } else if t.done && t.details.parent_id.is_none() {
                return Err(invalid("A root in backlog cannot be Done"));
            }
            if let Some(parent) = &t.details.parent_id {
                let parent = self.find_ticket(parent)?;
                if parent.id == t.id
                    || parent.details.parent_id.is_some()
                    || parent.board_id != t.board_id
                    || parent.column_id != t.column_id
                    || parent.details.trashed != t.details.trashed
                    || parent.details.archived != t.details.archived
                {
                    return Err(invalid("Invalid one-level parent/child relationship"));
                }
                if parent.done && !t.done {
                    return Err(invalid("Completed parent cannot contain an open child"));
                }
                if let Some(kind) = &parent.details.type_id
                    && !self
                        .types
                        .iter()
                        .any(|ty| &ty.id == kind && ty.allow_children)
                {
                    return Err(invalid("Parent type forbids children"));
                }
            }
        }
        for c in &self.columns {
            if c.wip_strict
                && let Some(limit) = c.wip_limit
            {
                let count = self
                    .tickets
                    .iter()
                    .filter(|t| {
                        t.column_id.as_ref() == Some(&c.id)
                            && t.details.parent_id.is_none()
                            && !t.details.trashed
                            && !t.details.archived
                    })
                    .count();
                if count > limit as usize {
                    return Err(Error::WipLimit);
                }
            }
        }
        for (board, options) in &self.board_options {
            self.board_index(board)?;
            color(&options.color)?;
            for (column, tint) in &options.column_colors {
                color(tint)?;
                if !self
                    .columns
                    .iter()
                    .any(|c| &c.id == column && &c.board_id == board)
                {
                    return Err(invalid("Unknown board color column"));
                }
            }
            if options.undo_depth > 1000 {
                return Err(invalid("Undo limit exceeded"));
            }
            for key in &options.enabled_types {
                if !self.types.iter().any(|t| &t.id == key) {
                    return Err(invalid("Unknown board type"));
                }
            }
        }
        let mut incoming: BTreeMap<String, usize> =
            self.tickets.iter().map(|t| (t.id.clone(), 0)).collect();
        let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for edge in &self.dependencies {
            let dependent = self.find_ticket(&edge.ticket_id)?;
            let prerequisite = self.find_ticket(&edge.prerequisite_id)?;
            if dependent.details.trashed || prerequisite.details.trashed {
                return Err(invalid("Trash cannot participate in dependencies"));
            }
            if dependent.done && !prerequisite.done {
                return Err(Error::Blocked);
            }
            *incoming.get_mut(&edge.ticket_id).ok_or(Error::NotFound)? += 1;
            outgoing
                .entry(edge.prerequisite_id.clone())
                .or_default()
                .push(edge.ticket_id.clone());
        }
        let mut queue: VecDeque<_> = incoming
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(key, _)| key.clone())
            .collect();
        let mut visited = 0;
        while let Some(key) = queue.pop_front() {
            visited += 1;
            if let Some(next) = outgoing.get(&key) {
                for key in next {
                    let count = incoming.get_mut(key).ok_or(Error::NotFound)?;
                    *count -= 1;
                    if *count == 0 {
                        queue.push_back(key.clone());
                    }
                }
            }
        }
        if visited != incoming.len() {
            return Err(invalid("Dependency cycle"));
        }
        let mut type_ids = BTreeSet::new();
        for kind in &self.types {
            entity_id(&kind.id)?;
            text(&kind.name, 80, "type name")?;
            color(&kind.color)?;
            text(&kind.icon, 64, "icon")?;
            if !type_ids.insert(&kind.id) {
                return Err(invalid("Duplicate type"));
            }
        }
        let mut label_ids = BTreeSet::new();
        for label in &self.labels {
            entity_id(&label.id)?;
            text(&label.name, 80, "label name")?;
            color(&label.color)?;
            if !label_ids.insert(&label.id) {
                return Err(invalid("Duplicate label"));
            }
        }
        let mut rule_ids = BTreeSet::new();
        for rule in &self.recurrences {
            entity_id(&rule.id)?;
            if !rule_ids.insert(&rule.id) {
                return Err(invalid("Duplicate recurrence ID"));
            }
            crate::sync::validate_recurrence(rule)?;
            let template = self.find_ticket(&rule.template_id)?;
            if template.details.parent_id.is_some() || template.details.trashed {
                return Err(invalid("Invalid recurrence template"));
            }
        }
        for link in &self.links {
            self.find_ticket(&link.ticket_id)?;
            self.find_ticket(&link.target_id)?;
        }
        let mut comment_ids = BTreeSet::new();
        for comment in &self.comments {
            entity_id(&comment.id)?;
            if !comment_ids.insert(&comment.id) {
                return Err(invalid("Duplicate comment ID"));
            }
            self.find_ticket(&comment.ticket_id)?;
            text(&comment.markdown, 65536, "comment")?;
        }
        if !["de", "en"].contains(&self.settings.language.as_str())
            || !["light", "dark", "system"].contains(&self.settings.theme.as_str())
            || !["off", "subtle", "fluid", "dynamic"].contains(&self.settings.animation.as_str())
        {
            return Err(invalid("Invalid language, theme or animation profile"));
        }
        self.settings
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| invalid("Unknown IANA timezone"))?;
        if self
            .settings
            .backup_every_hours
            .is_some_and(|h| h == 0 || h > 8760)
            || self.settings.backup_keep == 0
            || self.settings.backup_keep > 365
        {
            return Err(invalid("Invalid backup interval/retention"));
        }
        color(&self.settings.accent_color)?;
        Ok(())
    }

    pub fn ticket_fields(&self, key: &str) -> Result<BTreeMap<String, Value>> {
        let t = self.find_ticket(key)?;
        Ok(BTreeMap::from([
            ("board_id".into(), json!(t.board_id)),
            ("column_id".into(), json!(t.column_id)),
            ("done".into(), json!(t.done)),
            ("archived".into(), json!(t.details.archived)),
            ("title".into(), json!(t.title)),
            ("description".into(), json!(t.description)),
            ("importance".into(), json!(t.importance)),
            ("type_id".into(), json!(t.details.type_id)),
            ("labels".into(), json!(t.details.labels)),
            ("custom_fields".into(), json!(t.details.custom_fields)),
            ("deadline".into(), json!(t.details.deadline)),
        ]))
    }

    pub fn blocked(&self, key: &str) -> bool {
        self.dependencies.iter().any(|d| {
            d.ticket_id == key && self.find_ticket(&d.prerequisite_id).is_ok_and(|t| !t.done)
        })
    }

    pub fn search(&self, view: &ViewState, now: DateTime<Utc>) -> Result<Vec<Ticket>> {
        view.validate()?;
        let from = view
            .deadline_from
            .as_deref()
            .map(|date| {
                filter_start(date, &self.settings.timezone)
                    .ok_or_else(|| invalid("Date does not exist in the configured timezone"))
            })
            .transpose()?;
        let to = view
            .deadline_to
            .as_ref()
            .map(|date| {
                crate::sync::deadline(&Some(date.clone()), &self.settings.timezone)
                    .ok_or_else(|| invalid("Date does not exist in the configured timezone"))
            })
            .transpose()?;
        let terms: Vec<_> = view
            .query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
        let exact = self
            .tickets
            .iter()
            .find(|t| {
                t.public_id.eq_ignore_ascii_case(view.query.trim()) || t.id == view.query.trim()
            })
            .map(|t| t.id.as_str());
        let mut tickets: Vec<Ticket> = self
            .tickets
            .iter()
            .filter(|t| {
                if !view.board_id.is_empty() && t.board_id != view.board_id {
                    return false;
                }
                let status = match view.status.as_str() {
                    "all" => true,
                    "archive" => t.details.archived && !t.details.trashed,
                    "trash" => t.details.trashed,
                    "done" => t.done && !t.details.trashed && !t.details.archived,
                    "open" => !t.done && !t.details.trashed && !t.details.archived,
                    "blocked" => self.blocked(&t.id) && !t.details.trashed && !t.details.archived,
                    "backlog" => t.column_id.is_none() && !t.details.trashed && !t.details.archived,
                    "overdue" => {
                        !t.done
                            && !t.details.trashed
                            && !t.details.archived
                            && crate::sync::deadline(&t.details.deadline, &self.settings.timezone)
                                .is_some_and(|d| d < now)
                    }
                    _ => !t.details.trashed && !t.details.archived,
                };
                if !status
                    || view
                        .importance_min
                        .is_some_and(|minimum| t.importance < minimum)
                    || view
                        .importance_max
                        .is_some_and(|maximum| t.importance > maximum)
                    || !view.custom_fields.iter().all(|(name, value)| {
                        t.details
                            .custom_fields
                            .get(name)
                            .is_some_and(|text| text.to_lowercase().contains(&value.to_lowercase()))
                    })
                    || from.is_some_and(|from| {
                        crate::sync::deadline(&t.details.deadline, &self.settings.timezone)
                            .is_none_or(|deadline| deadline < from)
                    })
                    || to.is_some_and(|to| {
                        crate::sync::deadline(&t.details.deadline, &self.settings.timezone)
                            .is_none_or(|deadline| deadline > to)
                    })
                    || !view.labels.is_subset(&t.details.labels)
                    || view
                        .type_id
                        .as_ref()
                        .is_some_and(|id| t.details.type_id.as_ref() != Some(id))
                {
                    return false;
                }
                if let Some(exact) = exact {
                    return t.id == exact;
                }
                let labels = self
                    .labels
                    .iter()
                    .filter(|l| t.details.labels.contains(&l.id))
                    .map(|l| l.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let haystack = format!(
                    "{} {} {} {} {:?}",
                    t.public_id, t.title, t.description, labels, t.details.custom_fields
                )
                .to_lowercase();
                terms.iter().all(|term| haystack.contains(term))
            })
            .cloned()
            .collect();
        tickets.sort_by(|a, b| {
            let comparison = match view.sort.as_str() {
                "title" => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                "importance" => a.importance.cmp(&b.importance),
                "public_id" => a.public_id.cmp(&b.public_id),
                "deadline" => {
                    let a = crate::sync::deadline(&a.details.deadline, &self.settings.timezone);
                    let b = crate::sync::deadline(&b.details.deadline, &self.settings.timezone);
                    match (a, b) {
                        (Some(a), Some(b)) => a.cmp(&b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                }
                "created" => a.details.created_at.cmp(&b.details.created_at),
                "updated" => a.details.updated_at.cmp(&b.details.updated_at),
                "type" => a.details.type_id.cmp(&b.details.type_id),
                "score" => self.score(a, now).cmp(&self.score(b, now)),
                key if key.starts_with("custom:") => a
                    .details
                    .custom_fields
                    .get(&key[7..])
                    .cmp(&b.details.custom_fields.get(&key[7..])),
                _ => a.details.rank.cmp(&b.details.rank),
            };
            (if view.descending {
                comparison.reverse()
            } else {
                comparison
            })
            .then(a.public_id.cmp(&b.public_id))
        });
        Ok(tickets)
    }

    pub fn score(&self, ticket: &Ticket, now: DateTime<Utc>) -> i64 {
        let options = self
            .board_options
            .get(&ticket.board_id)
            .cloned()
            .unwrap_or_default();
        let type_rank = self
            .types
            .iter()
            .find(|t| Some(&t.id) == ticket.details.type_id.as_ref())
            .map_or(0, |t| t.rank as i64);
        let urgency = crate::sync::deadline(&ticket.details.deadline, &self.settings.timezone)
            .map_or(0, |d| (30 - (d - now).num_days()).clamp(0, 255));
        ticket.importance as i64 * options.importance_weight as i64
            + type_rank * options.type_weight as i64
            + urgency * options.deadline_weight as i64
    }
}
