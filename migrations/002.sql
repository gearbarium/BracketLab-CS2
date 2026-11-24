CREATE TABLE IF NOT EXISTS audit_event (
    id INTEGER PRIMARY KEY,
    action TEXT NOT NULL,
    revision_id INTEGER NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO audit_event (action, revision_id, payload, created_at)
SELECT action, id, payload, created_at FROM revision ORDER BY id;
PRAGMA user_version = 2;
