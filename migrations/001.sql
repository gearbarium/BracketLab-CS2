CREATE TABLE IF NOT EXISTS revision (
    id INTEGER PRIMARY KEY,
    action TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS current_revision (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    revision_id INTEGER NOT NULL REFERENCES revision (id)
);
PRAGMA user_version = 1;
