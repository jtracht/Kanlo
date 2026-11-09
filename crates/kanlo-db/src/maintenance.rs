use super::*;
use std::{fs, io::Write, path::PathBuf};

impl Store {
    /// Run explicitly enabled timers. Repeated calls cannot duplicate a recurrence occurrence.
    pub fn tick(&mut self, at: chrono::DateTime<chrono::Utc>) -> Result<ChangeResult> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let before = records::load(&tx)?;
        let mut after = before.clone();
        let mut aggregate = ChangeResult::default();
        let mut collect = |result: ChangeResult| {
            aggregate.affected.extend(result.affected);
            aggregate.boards.extend(result.boards);
            aggregate.warnings.extend(result.warnings);
        };
        for index in 0..after.recurrences.len() {
            let rule = after.recurrences[index].clone();
            if !rule.enabled {
                continue;
            }
            let due = chrono::DateTime::parse_from_rfc3339(&rule.next_at)
                .map_err(|_| Error::Storage)?
                .with_timezone(&chrono::Utc);
            if due > at {
                continue;
            }
            let predecessor = rule.last_created.as_deref().unwrap_or(&rule.template_id);
            if rule.after_completion
                && !after
                    .find_ticket(predecessor)
                    .is_ok_and(|t| t.done && !t.details.trashed)
            {
                continue;
            }
            let created = after.apply(Command::CloneTicket {
                id: rule.template_id.clone(),
            })?;
            let created_id = created.entity_id.clone().ok_or(Error::Storage)?;
            let title = after.find_ticket(&rule.template_id)?.title.clone();
            let revision = after.find_ticket(&created_id)?.revision;
            collect(created);
            collect(after.apply(Command::PatchTicket {
                id: created_id.clone(),
                base_revision: revision,
                fields: BTreeMap::from([("title".into(), json!(title))]),
            })?);
            after.recurrences[index].last_created = Some(created_id);
            after.recurrences[index].next_at =
                kanlo_core::sync::recurrence_next(&rule, at)?.to_rfc3339();
        }
        let mut archive = vec![];
        let mut purge = vec![];
        for t in after
            .tickets
            .iter()
            .filter(|t| t.details.parent_id.is_none())
        {
            if let Some(options) = after.board_options.get(&t.board_id) {
                if let Some(days) = options.auto_archive_days
                    && t.done
                    && !t.details.archived
                    && !t.details.trashed
                    && t.details
                        .completed_at
                        .as_ref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .is_some_and(|when| {
                            at.signed_duration_since(when).num_days() >= days as i64
                        })
                {
                    archive.push(t.id.clone());
                }
                if let Some(days) = options.trash_retention_days
                    && t.details.trashed
                    && t.details
                        .deleted_at
                        .as_ref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .is_some_and(|when| {
                            at.signed_duration_since(when).num_days() >= days as i64
                        })
                {
                    purge.push(t.id.clone());
                }
            }
        }
        if !archive.is_empty() {
            collect(after.apply(Command::Archive {
                ids: archive,
                archived: true,
            })?);
        }
        if !purge.is_empty() {
            collect(after.apply(Command::Purge { ids: purge })?);
        }
        after.validate()?;
        commit_state(
            &tx,
            &before,
            &after,
            &aggregate,
            "scheduled maintenance",
            true,
        )?;
        tx.commit().map_err(storage)?;
        Ok(aggregate)
    }

    pub fn write_backup(&mut self, directory: &Path) -> Result<PathBuf> {
        private_directory(directory)?;
        let backup = self.backup()?;
        let path = directory.join(format!(
            "kanlo-{}-{}.json",
            chrono::Utc::now().format("%Y%m%dT%H%M%S"),
            Uuid::new_v4()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|_| Error::Storage)?;
        file.write_all(encode(&backup)?.as_bytes())
            .map_err(|_| Error::Storage)?;
        file.sync_all().map_err(|_| Error::Storage)?;
        Ok(path)
    }

    pub fn scheduled_backup(
        &mut self,
        directory: &Path,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<PathBuf>> {
        let settings = self.workspace()?.settings;
        let Some(hours) = settings.backup_every_hours else {
            return Ok(None);
        };
        let last: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM metadata WHERE key='last_backup'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if last
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .is_some_and(|last| at.signed_duration_since(last).num_hours() < hours as i64)
        {
            return Ok(None);
        }
        let path = self.write_backup(directory)?;
        self.connection.execute("INSERT INTO metadata(key,value) VALUES ('last_backup',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[at.to_rfc3339()]).map_err(storage)?;
        let mut files: Vec<_> = fs::read_dir(directory)
            .map_err(|_| Error::Storage)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_ok_and(|t| t.is_file())
                    && e.file_name().to_string_lossy().starts_with("kanlo-")
                    && e.path().extension().is_some_and(|s| s == "json")
            })
            .collect();
        files.sort_by_key(|e| e.file_name());
        let keep = settings.backup_keep as usize;
        let remove = files.len().saturating_sub(keep);
        for entry in files.into_iter().take(remove) {
            fs::remove_file(entry.path()).map_err(|_| Error::Storage)?;
        }
        Ok(Some(path))
    }

    /// Copy under a reserved writer lock, verify, then leave a fail-closed marker on the old database.
    /// The caller must persist the new KANLO_DATA_DIR in its service configuration.
    pub fn relocate(&mut self, directory: &Path) -> Result<PathBuf> {
        let source = self
            .connection
            .path()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| Error::Validation("In-memory database cannot be relocated".into()))?;
        if directory.file_name().is_none() || directory.parent().is_none() {
            return Err(Error::Validation(
                "Choose a dedicated target directory".into(),
            ));
        }
        private_directory(directory)?;
        if fs::read_dir(directory)
            .map_err(|_| Error::Storage)?
            .next()
            .is_some()
        {
            return Err(Error::Conflict(
                "Migration destination must be empty".into(),
            ));
        }
        let target = directory
            .canonicalize()
            .map_err(|_| Error::Storage)?
            .join("kanlo.sqlite3");
        if source.canonicalize().map_err(|_| Error::Storage)? == target {
            return Err(Error::Conflict("Source equals destination".into()));
        }
        self.write_backup(&source.parent().ok_or(Error::Storage)?.join("backups"))?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let reader =
            Connection::open_with_flags(&source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(storage)?;
        reader
            .backup(rusqlite::MAIN_DB, &target, rusqlite::backup::NO_PROGRESS)
            .map_err(storage)?;
        let mut candidate = Store::open(&target)?;
        if candidate.integrity_check()? != "ok"
            || candidate.identity()? != identity(&tx)?
            || encode(&candidate.workspace()?)? != encode(&records::load(&tx)?)?
        {
            return Err(Error::Storage);
        }
        tx.execute("INSERT INTO metadata(key,value) VALUES ('relocated_to',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[target.to_string_lossy().to_string()]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        std::mem::swap(&mut self.connection, &mut candidate.connection);
        Ok(target)
    }
}

pub fn private_directory(directory: &Path) -> Result<()> {
    if directory.exists()
        && fs::symlink_metadata(directory)
            .map_err(|_| Error::Storage)?
            .file_type()
            .is_symlink()
    {
        return Err(Error::Validation("Directory must not be a symlink".into()));
    }
    fs::create_dir_all(directory).map_err(|_| Error::Storage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::Storage)?;
    }
    Ok(())
}
