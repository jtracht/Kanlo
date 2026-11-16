CREATE TABLE planning_tickets (
    ticket_id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES calendar_entries(id) ON DELETE CASCADE
) STRICT;
