CREATE TABLE temp_mappings (
    client_id TEXT NOT NULL,
    temporary_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    PRIMARY KEY(client_id,temporary_id)
) STRICT;
CREATE TABLE security_config (key TEXT PRIMARY KEY,value TEXT NOT NULL) STRICT;
CREATE TABLE api_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    secret_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1))
) STRICT;
CREATE TABLE external_sessions (
    token_hash TEXT PRIMARY KEY,
    csrf_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE security_audit (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    subject_id TEXT,
    created_at TEXT NOT NULL
) STRICT;
