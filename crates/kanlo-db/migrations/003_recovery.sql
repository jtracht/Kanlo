CREATE TABLE recovery_previews (
    token TEXT PRIMARY KEY,
    revision INTEGER NOT NULL,
    purpose TEXT NOT NULL,
    candidate TEXT NOT NULL CHECK(json_valid(candidate)),
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE restore_safety_snapshots (
    id TEXT PRIMARY KEY,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    created_at TEXT NOT NULL
) STRICT;
