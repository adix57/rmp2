use crate::proto::{FAVORITE_TAG, MediaInfo, TagInfo};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Library {
    conn: Connection,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn classify_uri(uri: &str) -> (&'static str, Option<String>) {
    for scheme in ["http://", "https://"] {
        if let Some(rest) = uri.strip_prefix(scheme) {
            let host = rest
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("")
                .to_string();
            return ("online", if host.is_empty() { None } else { Some(host) });
        }
    }
    ("offline", None)
}

pub struct NewMedia<'a> {
    pub uri: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub title: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub duration: Option<f64>,
    pub bitrate: Option<u64>,
    pub source: Option<&'a str>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let lib = Self { conn };
        lib.migrate()?;
        Ok(lib)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS media (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 uri TEXT NOT NULL UNIQUE,
                 name TEXT NOT NULL,
                 kind TEXT NOT NULL DEFAULT 'offline',
                 title TEXT,
                 artist TEXT,
                 duration REAL,
                 bitrate INTEGER,
                 source TEXT,
                 added_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tags (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL UNIQUE
             );
             CREATE TABLE IF NOT EXISTS media_tags (
                 media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                 tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                 PRIMARY KEY (media_id, tag_id)
             );
             CREATE INDEX IF NOT EXISTS idx_media_tags_tag ON media_tags(tag_id);",
        )?;
        let _ = self.ensure_tag(FAVORITE_TAG)?;
        Ok(())
    }

    fn tag_data(&self, name: &str) -> Result<Option<i64>, rusqlite::Error> {
        self.conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
                r.get(0)
            })
            .optional()
    }

    fn ensure_tag(&self, name: &str) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tags(name) VALUES (?1)",
            params![name],
        )?;
        Ok(self.tag_data(name)?.unwrap_or(0))
    }

    fn tags_for(&self, media_id: i64) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT t.name FROM media_tags mt JOIN tags t ON t.id = mt.tag_id
             WHERE mt.media_id = ?1 ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![media_id], |r| r.get::<_, String>(0))?;
        rows.collect()
    }

    fn set_tags(&self, media_id: i64, tags: &[String]) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM media_tags WHERE media_id = ?1",
            params![media_id],
        )?;
        for t in tags {
            let tag_id = self.ensure_tag(t)?;
            self.conn.execute(
                "INSERT OR IGNORE INTO media_tags(media_id, tag_id) VALUES (?1, ?2)",
                params![media_id, tag_id],
            )?;
        }
        Ok(())
    }

    pub fn add_media(&mut self, m: NewMedia<'_>, tags: &[String]) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO media(uri, name, kind, title, artist, duration, bitrate, source, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(uri) DO UPDATE SET
                 name = excluded.name,
                 kind = excluded.kind,
                 title = excluded.title,
                 artist = excluded.artist,
                 duration = excluded.duration,
                 bitrate = excluded.bitrate,
                 source = excluded.source,
                 added_at = excluded.added_at",
            params![
                m.uri,
                m.name,
                m.kind,
                m.title,
                m.artist,
                m.duration,
                m.bitrate,
                m.source,
                now_ms()
            ],
        )?;
        let id =
            self.conn
                .query_row("SELECT id FROM media WHERE uri = ?1", params![m.uri], |r| {
                    r.get(0)
                })?;
        let _ = self.ensure_tag(FAVORITE_TAG)?;
        self.set_tags(id, tags)?;
        Ok(id)
    }

    pub fn update_media(
        &mut self,
        id: i64,
        name: &str,
        title: Option<&str>,
        artist: Option<&str>,
        tags: &[String],
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE media SET name = ?2, title = ?3, artist = ?4 WHERE id = ?1",
            params![id, name, title, artist],
        )?;
        self.set_tags(id, tags)?;
        Ok(())
    }

    pub fn delete_media(&mut self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn
            .execute("DELETE FROM media WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn row_to_media(&self, row: &rusqlite::Row) -> Result<MediaInfo, rusqlite::Error> {
        let id: i64 = row.get(0)?;
        let tags = self.tags_for(id)?;
        Ok(MediaInfo {
            id,
            uri: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            title: row.get(4)?,
            artist: row.get(5)?,
            duration: row.get(6)?,
            bitrate: row.get(7)?,
            source: row.get(8)?,
            favorite: tags.iter().any(|t| t == FAVORITE_TAG),
            tags,
        })
    }

    pub fn media(&mut self, id: i64) -> Result<Option<MediaInfo>, rusqlite::Error> {
        self.conn
            .query_row(
                "SELECT id, uri, name, kind, title, artist, duration, bitrate, source
                 FROM media WHERE id = ?1",
                params![id],
                |r| self.row_to_media(r),
            )
            .optional()
    }

    pub fn all_media(&mut self) -> Result<Vec<MediaInfo>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, uri, name, kind, title, artist, duration, bitrate, source
             FROM media ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], |r| self.row_to_media(r))?;
        rows.collect()
    }

    pub fn tags(&mut self, checked: &[String]) -> Result<Vec<TagInfo>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT t.name, COUNT(mt.media_id) AS c FROM tags t
             LEFT JOIN media_tags mt ON mt.tag_id = t.id
             GROUP BY t.id ORDER BY c DESC, t.name",
        )?;
        let rows = stmt.query_map([], |r| {
            let name: String = r.get(0)?;
            let count: i64 = r.get(1)?;
            let checked = checked.contains(&name);
            Ok(TagInfo {
                name,
                checked,
                count,
            })
        })?;
        rows.collect()
    }

    pub fn set_favorite(&mut self, id: i64, on: bool) -> Result<(), rusqlite::Error> {
        let tag_id = self.ensure_tag(FAVORITE_TAG)?;
        if on {
            self.conn.execute(
                "INSERT OR IGNORE INTO media_tags(media_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM media_tags WHERE media_id = ?1 AND tag_id = ?2",
                params![id, tag_id],
            )?;
        }
        Ok(())
    }

    pub fn set_title_artist(
        &mut self,
        id: i64,
        title: Option<&str>,
        artist: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE media SET title = COALESCE(NULLIF(title, ''), ?2),
             artist = COALESCE(NULLIF(artist, ''), ?3) WHERE id = ?1",
            params![id, title, artist],
        )?;
        Ok(())
    }

    pub fn update_playback_stats(
        &mut self,
        id: i64,
        duration: Option<f64>,
        bitrate: Option<u64>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE media SET
                 duration = CASE WHEN duration IS NULL OR duration <= 0 THEN ?2 ELSE duration END,
                 bitrate = CASE WHEN bitrate IS NULL THEN ?3 ELSE bitrate END
             WHERE id = ?1",
            params![id, duration, bitrate],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_kinds() {
        assert_eq!(classify_uri("https://example.com/a.mp3").0, "online");
        assert_eq!(
            classify_uri("https://example.com/a.mp3").1.as_deref(),
            Some("example.com")
        );
        assert_eq!(
            classify_uri("http://x.io:8080/s").1.as_deref(),
            Some("x.io")
        );
        assert_eq!(classify_uri("/home/u/m.mp3").0, "offline");
    }

    #[test]
    fn roundtrip_media() {
        let dir = std::env::temp_dir().join(format!("rmp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("t.sqlite3");
        let _ = std::fs::remove_file(&db);
        let mut lib = Library::open(&db).unwrap();
        let id = lib
            .add_media(
                NewMedia {
                    uri: "/tmp/a.mp3",
                    name: "A Song",
                    kind: "offline",
                    title: Some("A Song"),
                    artist: Some("Artist"),
                    duration: Some(220.0),
                    bitrate: Some(128_000),
                    source: None,
                },
                &["rock".into(), "favorite".into()],
            )
            .unwrap();
        let m = lib.media(id).unwrap().unwrap();
        assert_eq!(m.name, "A Song");
        assert!(m.tags.contains(&"rock".into()));
        assert!(m.tags.contains(&"favorite".into()));

        lib.set_favorite(id, false).unwrap();
        let m = lib.media(id).unwrap().unwrap();
        assert!(!m.favorite);

        lib.update_media(id, "New Name", Some("T"), Some("Ar"), &["jazz".into()])
            .unwrap();
        let m = lib.media(id).unwrap().unwrap();
        assert_eq!(m.name, "New Name");
        assert_eq!(m.tags, vec!["jazz".to_string()]);

        let all = lib.all_media().unwrap();
        assert_eq!(all.len(), 1);
        let tags = lib.tags(&["jazz".into()]).unwrap();
        assert!(tags.iter().any(|t| t.name == "jazz" && t.checked));

        lib.delete_media(id).unwrap();
        assert!(lib.media(id).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_updates_existing_uri() {
        let dir = std::env::temp_dir().join(format!("rmp-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("t.sqlite3");
        let _ = std::fs::remove_file(&db);
        let mut lib = Library::open(&db).unwrap();
        let id1 = lib
            .add_media(
                NewMedia {
                    uri: "/x.mp3",
                    name: "One",
                    kind: "offline",
                    title: None,
                    artist: None,
                    duration: None,
                    bitrate: None,
                    source: None,
                },
                &[],
            )
            .unwrap();
        let id2 = lib
            .add_media(
                NewMedia {
                    uri: "/x.mp3",
                    name: "Two",
                    kind: "offline",
                    title: None,
                    artist: None,
                    duration: None,
                    bitrate: None,
                    source: None,
                },
                &[],
            )
            .unwrap();
        assert_eq!(id1, id2);
        let m = lib.media(id1).unwrap().unwrap();
        assert_eq!(m.name, "Two");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
