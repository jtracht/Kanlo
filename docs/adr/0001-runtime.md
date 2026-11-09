# ADR 0001: Single authoritative server, isolated adapters

Status: historical first-slice decision; extended/superseded by ADR 0002 for
domain persistence, web, recovery and ingress. Native graphics still requires
hardware measurements. Descriptions below refer to the first slice, not the
current implementation.
Date: 2026-11-09.

## Decision

Use a Rust workspace: `kanlo-core` contains shared contracts and field validation;
`kanlo-db` is the sole transactional mutation service and SQLite repository;
`kanlo-api` adapts HTTP to that service. SQLite uses WAL, foreign keys, a five-second
busy timeout and FULL synchronous writes. Versioned migration and initial seed run
in IMMEDIATE transactions. A newer schema aborts startup instead of being downgraded.

All create/move validation happens inside the write transaction, including WIP
counts. A move requires the current revision. Public ticket IDs are assigned once
from the originating board's monotonic counter and remain unchanged after moves.
Mutations record a metadata event in the same transaction. These events are not
yet an undo journal or statistics subsystem.

The first server uses one mutex-protected connection on Tokio's blocking pool.
This deliberately serializes local writes. Separate connections/processes still
serialize through SQLite's IMMEDIATE transaction. Measure contention on Pi before
introducing a read pool. Never perform SQLite work on an async executor thread.

Web and future MCP/GPT adapters call the same service. The current static online
client is a disposable runtime probe, built without a JavaScript toolchain; full
Preact/TypeScript, i18n, modal, PWA and offline editing belong to M3/M5.
No TV renderer is substituted with a mandatory browser. SDL2/Pango/Cairo selection
remains provisional until actual ARM32 display/resource measurements.

## Ingress

Current development build binds only loopback; non-loopback configuration fails.
Host allowlist and exact same-Origin checks prevent ordinary browser cross-site
writes and DNS rebinding. Forwarded headers are ignored. These controls are not
external authentication. Do not proxy or expose this development listener.
M8 must implement separate LAN/protected ingress and actual passcode/key/session
authorization before enabling LAN or remote product access. Production API keys
will replace the browser Origin requirement for authenticated API callers.

## Sources checked during implementation

- [Axum serve](https://docs.rs/axum/latest/axum/fn.serve.html)
- [rusqlite transactions](https://docs.rs/rusqlite/latest/rusqlite/struct.Transaction.html)

Resolved versions, rather than `latest` documentation links, are pinned in Cargo.lock.
No architecture compatibility claim follows from compiling on Windows.
