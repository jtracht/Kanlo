CREATE TABLE domain_records (
    kind TEXT NOT NULL,
    id TEXT NOT NULL,
    board_id TEXT,
    board_kind TEXT GENERATED ALWAYS AS ('board') VIRTUAL,
    body TEXT NOT NULL CHECK(json_valid(body)),
    PRIMARY KEY(kind, id),
    FOREIGN KEY(board_kind, board_id) REFERENCES domain_records(kind, id)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX domain_by_board ON domain_records(board_id, kind);
CREATE UNIQUE INDEX ticket_public_id ON domain_records(json_extract(body, '$.public_id')) WHERE kind='ticket';
CREATE UNIQUE INDEX board_prefix ON domain_records(json_extract(body, '$.prefix')) WHERE kind='board';
CREATE TABLE receipts (
    op_id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    response TEXT NOT NULL CHECK(json_valid(response)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE undo_groups (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    description TEXT NOT NULL,
    changes TEXT NOT NULL CHECK(json_valid(changes)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE undo_boards (
    sequence INTEGER NOT NULL REFERENCES undo_groups(sequence) ON DELETE CASCADE,
    board_id TEXT NOT NULL,
    PRIMARY KEY(sequence, board_id)
) STRICT;
CREATE TABLE previews (
    token TEXT PRIMARY KEY,
    revision INTEGER NOT NULL,
    command TEXT NOT NULL CHECK(json_valid(command)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE event_stream (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    created_at TEXT NOT NULL
) STRICT;
CREATE TABLE statistics_samples (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id TEXT NOT NULL,
    sample TEXT NOT NULL CHECK(json_valid(sample)),
    created_at TEXT NOT NULL
) STRICT;
CREATE INDEX statistics_by_board_time ON statistics_samples(board_id, created_at);
INSERT INTO metadata(key,value) VALUES ('revision','0');
