use crate::tournament::Project;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::path::Path;
use uuid::Uuid;

pub struct Storage(Connection, Option<String>);

pub struct TournamentEntry {
    pub id: String,
    pub project: Project,
    pub updated_at: String,
}

impl Storage {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let mut db = Connection::open(path)?;
        db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 3 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if (1..3).contains(&version) && path != Path::new(":memory:") {
            let backup =
                path.with_extension(format!("v{version}-{}.backup.sqlite3", Uuid::new_v4()));
            db.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
        }
        if version < 3 {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let version: u32 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            if version == 0 {
                tx.execute_batch(include_str!("../migrations/001.sql"))?;
            }
            if version < 2 {
                tx.execute_batch(include_str!("../migrations/002.sql"))?;
            }
            if version < 3 {
                tx.execute_batch(include_str!("../migrations/003.sql"))?;
            }
            tx.commit()?;
        }
        let active = db
            .query_row("SELECT tournament_id FROM active_tournament", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(Self(db, active))
    }

    pub fn active_id(&self) -> Option<&str> {
        self.1.as_deref()
    }

    pub fn current(&self) -> Result<Option<Project>, String> {
        let payload: Option<String> = self.0.query_row(
            "SELECT payload FROM tournament JOIN revision ON revision.id = revision_id WHERE tournament.id=?1",
            [&self.1], |r| r.get(0),
        ).optional().map_err(message)?;
        payload.map(|value| decode(&value)).transpose()
    }

    pub fn catalog(&self) -> Result<Vec<TournamentEntry>, String> {
        let mut query = self.0.prepare("SELECT tournament.id, payload, updated_at
            FROM tournament JOIN revision ON revision.id = revision_id ORDER BY updated_at DESC, tournament.id").map_err(message)?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(message)?;
        rows.map(|row| {
            let (id, payload, updated_at) = row.map_err(message)?;
            Ok(TournamentEntry {
                id,
                project: decode(&payload)?,
                updated_at,
            })
        })
        .collect()
    }

    pub fn select(&mut self, id: &str) -> Result<Project, String> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(message)?;
        let project = load(&tx, id)?;
        select(&tx, id).map_err(message)?;
        tx.commit().map_err(message)?;
        self.1 = Some(id.into());
        Ok(project)
    }

    pub fn create(&mut self, project: &Project, action: &str) -> Result<(), String> {
        let payload = encode(project)?;
        let id = Uuid::new_v4().to_string();
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(message)?;
        tx.execute("INSERT INTO tournament(id) VALUES (?1)", [&id])
            .map_err(message)?;
        append(&tx, &id, &payload, action).map_err(message)?;
        select(&tx, &id).map_err(message)?;
        tx.commit().map_err(message)?;
        self.1 = Some(id);
        Ok(())
    }

    pub fn rename(&mut self, id: &str, name: String) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Введите название турнира".into());
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(message)?;
        let mut project = load(&tx, id)?;
        project.name = name;
        tx.execute("DELETE FROM revision WHERE tournament_id=?1 AND id > (SELECT revision_id FROM tournament WHERE id=?1)",[id]).map_err(message)?;
        append(&tx, id, &encode(&project)?, "Турнир переименован").map_err(message)?;
        tx.commit().map_err(message)
    }

    pub fn delete(&mut self, id: &str) -> Result<Option<Project>, String> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(message)?;
        let changed = tx
            .execute("UPDATE tournament SET revision_id=NULL WHERE id=?1", [id])
            .map_err(message)?;
        if changed == 0 {
            return Err("Турнир не найден".into());
        }
        tx.execute("DELETE FROM tournament WHERE id=?1", [id])
            .map_err(message)?;
        let next = if self.1.as_deref() == Some(id) {
            tx.query_row(
                "SELECT id FROM tournament ORDER BY updated_at DESC,id LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(message)?
        } else {
            self.1.clone()
        };
        let project = next.as_deref().map(|id| load(&tx, id)).transpose()?;
        if let Some(id) = &next {
            select(&tx, id).map_err(message)?;
        }
        tx.commit().map_err(message)?;
        self.1 = next;
        Ok(project)
    }

    pub fn commit(&mut self, project: &Project, action: &str) -> Result<(), String> {
        if self.1.is_none() {
            return self.create(project, action);
        }
        let payload = encode(project)?;
        let id = self.1.as_deref().unwrap();
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(message)?;
        tx.execute("DELETE FROM revision WHERE tournament_id=?1 AND id > (SELECT revision_id FROM tournament WHERE id=?1)",[id]).map_err(message)?;
        append(&tx, id, &payload, action).map_err(message)?;
        tx.commit().map_err(message)
    }

    pub fn step(&mut self, redo: bool) -> Result<Option<Project>, String> {
        let Some(id) = self.1.as_deref() else {
            return Ok(None);
        };
        let query = if redo {
            "SELECT id,payload FROM revision WHERE tournament_id=?1 AND id > (SELECT revision_id FROM tournament WHERE id=?1) ORDER BY id LIMIT 1"
        } else {
            "SELECT id,payload FROM revision WHERE tournament_id=?1 AND id < (SELECT revision_id FROM tournament WHERE id=?1) ORDER BY id DESC LIMIT 1"
        };
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(message)?;
        let candidate: Option<(i64, String)> = tx
            .query_row(query, [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(message)?;
        let Some((revision, payload)) = candidate else {
            return Ok(None);
        };
        let project = decode(&payload)?;
        record(
            &tx,
            id,
            revision,
            &payload,
            if redo {
                "Повтор действия"
            } else {
                "Отмена действия"
            },
        )
        .map_err(message)?;
        tx.commit().map_err(message)?;
        Ok(Some(project))
    }

    pub fn audit(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut query = self.0.prepare("SELECT created_at,action FROM audit_event WHERE tournament_id=?1 ORDER BY id DESC LIMIT 200")?;
        let rows = query.query_map([&self.1], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}

fn select(tx: &Transaction<'_>, id: &str) -> rusqlite::Result<()> {
    tx.execute("INSERT INTO active_tournament VALUES (1,?1) ON CONFLICT(singleton) DO UPDATE SET tournament_id=excluded.tournament_id",[id])?;
    Ok(())
}

fn load(tx: &Transaction<'_>, id: &str) -> Result<Project, String> {
    let payload: String = tx.query_row("SELECT payload FROM tournament JOIN revision ON revision.id=revision_id WHERE tournament.id=?1",[id],|r|r.get(0)).map_err(message)?;
    decode(&payload)
}

fn append(tx: &Transaction<'_>, id: &str, payload: &str, action: &str) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO revision(action,payload,tournament_id) VALUES (?1,?2,?3)",
        params![action, payload, id],
    )?;
    record(tx, id, tx.last_insert_rowid(), payload, action)
}

fn record(
    tx: &Transaction<'_>,
    id: &str,
    revision: i64,
    payload: &str,
    action: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE tournament SET revision_id=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
        params![revision, id],
    )?;
    tx.execute(
        "INSERT INTO audit_event(action,revision_id,payload,tournament_id) VALUES (?1,?2,?3,?4)",
        params![action, revision, payload, id],
    )?;
    Ok(())
}

fn message(error: rusqlite::Error) -> String {
    error.to_string()
}

fn encode(project: &Project) -> Result<String, String> {
    project.validate()?;
    serde_json::to_string(project).map_err(|e| e.to_string())
}

pub fn decode(payload: &str) -> Result<Project, String> {
    let project: Project = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    project.validate()?;
    Ok(project)
}
