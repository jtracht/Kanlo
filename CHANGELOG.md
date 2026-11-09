# Changelog

## 0.1.0-dev.1 — first private development prerelease

- Deliver the transactional domain, editable DE/EN web app, durable offline
  synchronization, conflict handling and protected external access.
- Add children/dependencies/types/labels/custom fields, undo, trash/archive,
  recurrence and data-only import/export/backup/restore.
- Package Debian 13 server binaries for AMD64, ARM64 and armhf with source,
  build metadata, SHA256 checksums, third-party notices and DE/EN instructions.
- Gate automatic tag releases on all CI jobs and real packaged-binary startup.
- Keep native TV, kiosk/images/updater, physical device acceptance, complete
  OpenAPI/MCP and final licensing outside this development release.

### Initial groundwork

- Preserve the three original requirement documents and review scope differences.
- Establish Rust domain/storage/API workspace and locked dependencies.
- Add transactional SQLite schema, stable identity/epoch and initial board.
- Add boards, columns, tickets and revision-aware moves with strict WIP validation.
- Add provisional local browser client and loopback/Origin/Host development guards.
- Add concurrency, persistence, migration and HTTP regression tests.
- Prepare Debian 13 architecture CI and M0–M10 development plan.
