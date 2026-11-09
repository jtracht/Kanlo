CREATE TABLE metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE boards (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prefix TEXT NOT NULL UNIQUE,
    next_ticket_number INTEGER NOT NULL DEFAULT 1 CHECK(next_ticket_number > 0),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0)
) STRICT;
CREATE TABLE board_columns (
    id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    is_done INTEGER NOT NULL CHECK(is_done IN (0, 1)),
    wip_limit INTEGER CHECK(wip_limit >= 0),
    wip_strict INTEGER NOT NULL CHECK(wip_strict IN (0, 1)),
    UNIQUE(id, board_id),
    UNIQUE(board_id, position)
) STRICT;
CREATE TABLE tickets (
    id TEXT PRIMARY KEY,
    public_id TEXT NOT NULL UNIQUE,
    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE RESTRICT,
    column_id TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    importance INTEGER NOT NULL CHECK(importance BETWEEN 0 AND 255),
    done INTEGER NOT NULL CHECK(done IN (0, 1)),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    FOREIGN KEY(column_id, board_id) REFERENCES board_columns(id, board_id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX tickets_by_board ON tickets(board_id);
CREATE INDEX tickets_by_column ON tickets(column_id);
CREATE TABLE mutation_events (
    sequence INTEGER PRIMARY KEY,
    entity_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    revision INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
