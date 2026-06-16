use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashSet;
use std::sync::Mutex;

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS categories (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS movies (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                category_id INTEGER NOT NULL,
                stream_id INTEGER,
                container_extension TEXT,
                stream_icon TEXT,
                rating TEXT,
                release_date TEXT,
                plot TEXT,
                genre TEXT
            );
            CREATE TABLE IF NOT EXISTS wishlist (
                movie_id INTEGER PRIMARY KEY
            );
            CREATE INDEX IF NOT EXISTS idx_movies_cat
                ON movies(category_id);
            CREATE INDEX IF NOT EXISTS idx_movies_name
                ON movies(name);",
        )?;
        let _ = conn.execute("ALTER TABLE movies ADD COLUMN genre TEXT", []);
        Ok(())
    }

    pub fn load_wishlist(&self) -> HashSet<i64> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT movie_id FROM wishlist").unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap();
        rows.flatten().collect()
    }

    pub fn toggle_wishlist(&self, movie_id: i64) -> bool {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM wishlist WHERE movie_id = ?1",
                rusqlite::params![movie_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);
        if exists {
            conn.execute("DELETE FROM wishlist WHERE movie_id = ?1", rusqlite::params![movie_id])
                .ok();
        } else {
            conn.execute("INSERT INTO wishlist (movie_id) VALUES (?1)", rusqlite::params![movie_id])
                .ok();
        }
        !exists
    }

    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("DELETE FROM movies; DELETE FROM categories;")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open(":memory:").unwrap()
    }

    #[test]
    fn open_creates_tables() {
        let db = test_db();
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' \
                 AND name IN ('categories','movies','wishlist')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn toggle_adds_and_removes() {
        let db = test_db();
        assert!(db.toggle_wishlist(42));
        assert_eq!(db.load_wishlist().len(), 1);
        assert!(db.load_wishlist().contains(&42));
        assert!(!db.toggle_wishlist(42));
        assert!(db.load_wishlist().is_empty());
    }

    #[test]
    fn toggle_multiple() {
        let db = test_db();
        db.toggle_wishlist(1);
        db.toggle_wishlist(2);
        db.toggle_wishlist(3);
        assert_eq!(db.load_wishlist().len(), 3);
        db.toggle_wishlist(2);
        assert_eq!(db.load_wishlist().len(), 2);
    }

    #[test]
    fn load_wishlist_empty() {
        let db = test_db();
        assert!(db.load_wishlist().is_empty());
    }
}
