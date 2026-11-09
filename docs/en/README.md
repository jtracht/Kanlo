# Kanlo development guide

Development version, not Kanlo 1.0 or a hardware-certified release. Run
`cargo run --locked -p kanlo-api --bin kanlo-server` and visit
`http://127.0.0.1:8080`. Rust 1.99.1 and a native C compiler are required to
build. Committed local webassets make Node unnecessary at runtime.

The Rust domain/SQLite authority implements boards, columns, stable ticket IDs,
types, global labels, custom text fields, one-level children, acyclic dependencies,
WIP, grouped undo, trash/archive, recurrence, comments, import/export and
data-only backup/restore. Mutation, offline, scheduler and recovery paths share
the same transactional validators.

The Preact/TypeScript web app supports DE/EN, themes, keyboard/touch, persistent
views and an IndexedDB operation queue. HTTPS PWA caches only visited board data.
Ordinary LAN HTTP is an online website; remote-device offline installation needs
trusted HTTPS. Same-field conflicts and old restore epochs require review.
Server receipts contain authoritative deltas, committed locally before further
network requests, so a connection loss after acknowledgment cannot lose that data.

`KANLO_DATA_DIR` selects private storage; `KANLO_BIND` defaults to loopback.
LAN bindings require exact `KANLO_ALLOWED_HOSTS` including ports. A separate
loopback `KANLO_EXTERNAL_BIND` and HTTPS `KANLO_PUBLIC_ORIGIN` enable the protected
proxy router after setting the shared passcode locally. Forwarded headers never
grant LAN trust. External browser writes require a valid session, CSRF and Origin;
API clients use separately revocable hashed keys. Restore revokes keys/sessions.

Build web changes with `npm ci --prefix web` and `npm run build --prefix web`.
Commit generated `crates/kanlo-api/assets` alongside sources. Run Cargo format,
tests and Clippy, then `npm ci --prefix tests/browser`, install Playwright Chromium
and `npm test --prefix tests/browser`. The suite uses an isolated pinned HTTPS
fixture without installing a user/system certificate. Debian network-none,
non-root runtime and ARM cross-build tests are described in the German test plan.

Native TV, installer/kiosk, OS images/updates, full OpenAPI/MCP/ChatGPT, physical
Pi measurements, public licensing and release approval remain open. Cross-builds
do not prove Pi runtime/display performance. Gitea is primary; GitHub mirrors
identical commits. See `docs/de/50-PROZENT.md` for the original-task accounting.
