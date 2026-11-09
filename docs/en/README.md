# Kanlo development guide

This is an early local runtime slice, not Kanlo 1.0. Run
`cargo run --locked -p kanlo-api --bin kanlo-server`, then visit
`http://127.0.0.1:8080`. Rust 1.99.1 and a native C compiler are required.
`KANLO_DATA_DIR` selects persistent storage; `KANLO_BIND` accepts loopback only.

The single SQLite authority stores boards, columns and tickets. Writes share
transactional validation, strict WIP enforcement and revision checks. Public
ticket IDs survive board moves. The provisional German online browser client
supports creating and moving tickets. Full Preact/TypeScript, DE/EN, TV, PWA,
offline queue, backups and external authentication remain planned.

Run `cargo test --workspace --locked`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings` and
`node --check web/app.js`. Windows development tests do not prove Debian/ARM
runtime or Pi display compatibility. Hardware measurements are still required.

Gitea is the intended primary, GitHub the commit mirror and later CI/release host.
New repositories stay private pending final licensing and trademark review.
No production ingress should proxy this development listener.
