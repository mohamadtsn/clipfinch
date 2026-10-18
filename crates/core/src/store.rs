//! SQLite-backed storage for clips, their raw formats and pinboards.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::classify;
use crate::model::{Clip, Format, Kind, NewClip, Pinboard};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Everything that is not on a pinboard.
    History,
    Pinboard(i64),
}

pub struct Store {
    conn: Connection,
}

const SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS pinboards(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS clips(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    title TEXT,
    source_app TEXT,
    hash TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    pinboard_id INTEGER REFERENCES pinboards(id) ON DELETE SET NULL,
    size INTEGER NOT NULL DEFAULT 0,
    thumb BLOB
);
CREATE TABLE IF NOT EXISTS formats(
    clip_id INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    mime TEXT NOT NULL,
    data BLOB NOT NULL,
    PRIMARY KEY(clip_id, mime)
);
CREATE INDEX IF NOT EXISTS idx_clips_updated ON clips(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_clips_pinboard ON clips(pinboard_id);
";

const COLS: &str = "id, kind, text, title, source_app, created_at, updated_at, pinboard_id, size, \
                    CASE WHEN kind='image' THEN thumb END";

fn map_clip(r: &Row) -> rusqlite::Result<Clip> {
    let kind: String = r.get(1)?;
    Ok(Clip {
        id: r.get(0)?,
        kind: Kind::parse(&kind).unwrap_or(Kind::Text),
        text: r.get(2)?,
        title: r.get(3)?,
        source_app: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
        pinboard_id: r.get(7)?,
        size: r.get(8)?,
        thumb: r.get(9)?,
    })
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    /// Inserts a clip, or bumps the existing one with the same hash to the top.
    /// Returns `(id, inserted)`.
    pub fn upsert_clip(&mut self, c: NewClip) -> Result<(i64, bool)> {
        let now = now_ms();
        let existing: Option<i64> = self
            .conn
            .query_row("SELECT id FROM clips WHERE hash=?1", params![c.hash], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(id) = existing {
            self.conn.execute(
                "UPDATE clips SET updated_at=MAX(?1, (SELECT COALESCE(MAX(updated_at), 0) + 1 FROM clips)), source_app=COALESCE(?2, source_app) WHERE id=?3",
                params![now, c.source_app, id],
            )?;
            return Ok((id, false));
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO clips(kind,text,title,source_app,hash,created_at,updated_at,pinboard_id,size,thumb)
             VALUES(?1,?2,NULL,?3,?4,?5,?5,NULL,?6,?7)",
            params![
                c.kind.as_str(),
                c.text,
                c.source_app,
                c.hash,
                now,
                c.size as i64,
                c.thumb
            ],
        )?;
        let id = tx.last_insert_rowid();
        for f in &c.formats {
            tx.execute(
                "INSERT OR REPLACE INTO formats(clip_id,mime,data) VALUES(?1,?2,?3)",
                params![id, f.mime, f.data],
            )?;
        }
        tx.commit()?;
        Ok((id, true))
    }

    pub fn list(&self, scope: Scope, limit: usize) -> Result<Vec<Clip>> {
        let (sql, pb) = match scope {
            Scope::History => (
                format!(
                    "SELECT {COLS} FROM clips WHERE pinboard_id IS NULL AND ?1 = ?1 \
                     ORDER BY updated_at DESC, id DESC LIMIT ?2"
                ),
                0i64,
            ),
            Scope::Pinboard(id) => (
                format!(
                    "SELECT {COLS} FROM clips WHERE pinboard_id = ?1 \
                     ORDER BY updated_at DESC, id DESC LIMIT ?2"
                ),
                id,
            ),
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![pb, limit as i64], map_clip)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get(&self, id: i64) -> Result<Option<Clip>> {
        let sql = format!("SELECT {COLS} FROM clips WHERE id=?1");
        Ok(self
            .conn
            .query_row(&sql, params![id], map_clip)
            .optional()?)
    }

    pub fn formats(&self, id: i64) -> Result<Vec<Format>> {
        let mut stmt = self
            .conn
            .prepare("SELECT mime, data FROM formats WHERE clip_id=?1 ORDER BY mime")?;
        let rows = stmt.query_map(params![id], |r| {
            Ok(Format {
                mime: r.get(0)?,
                data: r.get(1)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Full plain text of a clip (what "paste as plain text" pastes).
    pub fn plain_text(&self, id: i64) -> Result<String> {
        let clip = match self.get(id)? {
            Some(c) => c,
            None => return Ok(String::new()),
        };
        match clip.kind {
            Kind::Text | Kind::Link => {
                let formats = self.formats(id)?;
                Ok(classify::text_of(&formats).unwrap_or(clip.text))
            }
            Kind::Image => Ok(String::new()),
            Kind::File | Kind::Color => Ok(clip.text),
        }
    }

    pub fn touch(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE clips SET updated_at=MAX(?1, (SELECT COALESCE(MAX(updated_at), 0) + 1 FROM clips)) WHERE id=?2",
            params![now_ms(), id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM clips WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn rename(&self, id: i64, title: Option<&str>) -> Result<()> {
        self.conn
            .execute("UPDATE clips SET title=?1 WHERE id=?2", params![title, id])?;
        Ok(())
    }

    /// Moves a clip to a pinboard, or back to the history with `None`.
    pub fn pin(&self, id: i64, pinboard: Option<i64>) -> Result<()> {
        self.conn.execute(
            "UPDATE clips SET pinboard_id=?1, updated_at=MAX(?2, (SELECT COALESCE(MAX(updated_at), 0) + 1 FROM clips)) WHERE id=?3",
            params![pinboard, now_ms(), id],
        )?;
        Ok(())
    }

    pub fn pinboards(&self) -> Result<Vec<Pinboard>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM pinboards ORDER BY position, id")?;
        let rows = stmt.query_map([], |r| {
            Ok(Pinboard {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn create_pinboard(&self, name: &str) -> Result<i64> {
        let pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM pinboards",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO pinboards(name, position) VALUES(?1, ?2)",
            params![name, pos],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_pinboard(&self, id: i64, name: &str) -> Result<()> {
        self.conn
            .execute("UPDATE pinboards SET name=?1 WHERE id=?2", params![name, id])?;
        Ok(())
    }

    /// Deletes a pinboard; its clips return to the history.
    pub fn delete_pinboard(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM pinboards WHERE id=?1", params![id])?;
        Ok(())
    }

    /// Deletes every clip that is not on a pinboard.
    pub fn erase_history(&self) -> Result<usize> {
        Ok(self
            .conn
            .execute("DELETE FROM clips WHERE pinboard_id IS NULL", [])?)
    }

    pub fn purge_older_than(&self, cutoff_ms: i64) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM clips WHERE pinboard_id IS NULL AND updated_at < ?1",
            params![cutoff_ms],
        )?)
    }

    /// Keeps at most `max` unpinned clips (newest first).
    pub fn trim_history(&self, max: usize) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM clips WHERE pinboard_id IS NULL AND id NOT IN (
                SELECT id FROM clips WHERE pinboard_id IS NULL
                ORDER BY updated_at DESC, id DESC LIMIT ?1)",
            params![max as i64],
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::build_new_clip;

    fn text_clip(s: &str) -> NewClip {
        build_new_clip(
            vec![Format {
                mime: "text/plain;charset=utf-8".into(),
                data: s.as_bytes().to_vec(),
            }],
            Some("test".into()),
        )
        .unwrap()
    }

    #[test]
    fn insert_dedupe_and_list() {
        let mut s = Store::open_in_memory().unwrap();
        let (a, ins_a) = s.upsert_clip(text_clip("one")).unwrap();
        let (b, _) = s.upsert_clip(text_clip("two")).unwrap();
        let (a2, ins_a2) = s.upsert_clip(text_clip("one")).unwrap();
        assert!(ins_a && !ins_a2);
        assert_eq!(a, a2);
        let list = s.list(Scope::History, 10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, a); // bumped to the top
        assert_eq!(list[1].id, b);
        assert_eq!(s.plain_text(a).unwrap(), "one");
        assert_eq!(s.formats(a).unwrap().len(), 1);
    }

    #[test]
    fn pinboards_and_deletion() {
        let mut s = Store::open_in_memory().unwrap();
        let (a, _) = s.upsert_clip(text_clip("keep me")).unwrap();
        let (b, _) = s.upsert_clip(text_clip("drop me")).unwrap();
        let pb = s.create_pinboard("Work").unwrap();
        s.pin(a, Some(pb)).unwrap();
        assert_eq!(s.list(Scope::History, 10).unwrap().len(), 1);
        assert_eq!(s.list(Scope::Pinboard(pb), 10).unwrap().len(), 1);
        assert_eq!(s.erase_history().unwrap(), 1);
        assert!(s.get(b).unwrap().is_none());
        assert!(s.get(a).unwrap().is_some());
        s.delete_pinboard(pb).unwrap();
        assert_eq!(s.list(Scope::History, 10).unwrap().len(), 1);
    }

    #[test]
    fn retention_and_trim() {
        let mut s = Store::open_in_memory().unwrap();
        for i in 0..5 {
            s.upsert_clip(text_clip(&format!("clip {i}"))).unwrap();
        }
        assert_eq!(s.trim_history(3).unwrap(), 2);
        assert_eq!(s.list(Scope::History, 10).unwrap().len(), 3);
        assert_eq!(s.purge_older_than(now_ms() + 1000).unwrap(), 3);
        assert!(s.list(Scope::History, 10).unwrap().is_empty());
    }

    #[test]
    fn rename_clip() {
        let mut s = Store::open_in_memory().unwrap();
        let (a, _) = s.upsert_clip(text_clip("x")).unwrap();
        s.rename(a, Some("Title")).unwrap();
        assert_eq!(s.get(a).unwrap().unwrap().title.as_deref(), Some("Title"));
        s.rename(a, None).unwrap();
        assert!(s.get(a).unwrap().unwrap().title.is_none());
    }
}
