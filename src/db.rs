use anyhow::Result;
use rusqlite::Connection;
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
                plot TEXT
            );",
        )?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("DELETE FROM movies; DELETE FROM categories;")?;
        Ok(())
    }
}
