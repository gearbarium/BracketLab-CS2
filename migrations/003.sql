CREATE TABLE tournament (
    id TEXT PRIMARY KEY,
    revision_id INTEGER REFERENCES revision(id),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE revision ADD COLUMN tournament_id TEXT REFERENCES tournament(id) ON DELETE CASCADE;
ALTER TABLE audit_event ADD COLUMN tournament_id TEXT REFERENCES tournament(id) ON DELETE CASCADE;
CREATE TABLE active_tournament (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    tournament_id TEXT NOT NULL REFERENCES tournament(id) ON DELETE CASCADE
);
INSERT INTO tournament (id, revision_id, updated_at)
SELECT 'legacy', revision_id, COALESCE((SELECT MAX(created_at) FROM audit_event), CURRENT_TIMESTAMP)
FROM current_revision;
UPDATE revision SET tournament_id='legacy' WHERE EXISTS (SELECT 1 FROM tournament);
UPDATE audit_event SET tournament_id='legacy' WHERE EXISTS (SELECT 1 FROM tournament);
INSERT INTO active_tournament SELECT 1, id FROM tournament;
DROP TABLE current_revision;
CREATE INDEX revision_tournament ON revision(tournament_id, id);
CREATE INDEX audit_tournament ON audit_event(tournament_id, id);
PRAGMA user_version = 3;
