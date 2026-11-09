# ADR 0002: Typed domain transitions and incremental SQLite records

Date: 2026-11-09. Status: implemented on the 50-percent development branch.

The original runtime schema migrates transactionally to versioned JSON records
in SQLite, indexed by kind, stable ID and owning board. Only this table is the
authoritative entity store after migration; old entity tables are removed after
successful conversion. SQL enforces unique ticket public IDs/board prefixes,
valid JSON and deferred board foreign keys. The Rust domain validates every
reference, hierarchy, type activation, WIP limit and dependency graph before commit.

Each writer acquires an IMMEDIATE SQLite transaction, loads the domain aggregate,
applies a pure transition to a candidate, validates it, and persists only changed
records. HTTP, sync, scheduler and recovery use this transaction service. A failed
transition leaves records, counters, revisions, history and statistics unchanged.
The aggregate is presently loaded in memory to validate cross-board graph rules.
This favors correctness and a reviewable central validator. Its peak memory at
large descriptions/board counts remains an explicit Pi benchmark risk; an indexed
read projection and graph-specific loading can replace this after measurements.
No Pi resource budget is claimed from desktop tests.

Undo stores per-record before/after deltas, groups cascades/batches, rejects
incompatible later changes and validates the resulting graph. Issued identifiers
and board counters never roll back. History depth zero clears/disables undo and
does not affect trash or historical statistics. Data-only backups explicitly
allowlist workspace/history/statistics; authentication data lives in separate
tables and is never included. Restore stores a safety snapshot, increments the
epoch and invalidates API keys, sessions, receipts and temporary mappings.

LAN and protected proxy access are separate router instances/listeners over the
same store. Only the socket/router configuration selects the trust policy.
Forwarded headers do not authorize requests. External browser writes require
the configured HTTPS Origin, a valid secure HttpOnly session and CSRF token;
API callers use independently revocable hashed bearer keys. Shared passcodes
use Argon2id v19 (19456 KiB, two iterations, one lane), with random salts.

Sources checked: [rusqlite backup](https://docs.rs/rusqlite/latest/rusqlite/backup/),
[Argon2](https://docs.rs/argon2/latest/argon2/),
[constant-time comparison](https://docs.rs/subtle/latest/subtle/trait.ConstantTimeEq.html).
These describe APIs, not proof of Kanlo security or hardware compatibility.
