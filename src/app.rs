use crate::api;
use crate::config::{self, Config};
use crate::db::Database;
use anyhow::Result;
use crossterm::event::KeyCode;

use std::sync::mpsc;
use std::sync::Arc;

fn is_printable(c: char) -> bool {
    c.is_ascii_graphic() || c == ' '
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Categories,
    Movies,
    Details,
}

pub struct App {
    pub config: Option<Config>,
    pub db: Option<Arc<Database>>,
    pub focus: Focus,
    pub categories: Vec<CategoryItem>,
    pub movies: Vec<MovieItem>,
    pub category_offset: usize,
    pub movie_offset: usize,
    pub selected_category: Option<i64>,
    pub show_config: bool,
    pub config_fields: ConfigFields,
    pub status: String,
    pub logs: Vec<String>,
    pub log_rx: mpsc::Receiver<String>,
    pub log_tx: mpsc::Sender<String>,
    pub syncing: bool,
}

#[derive(Debug, Clone)]
pub struct CategoryItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct MovieItem {
    pub id: i64,
    pub name: String,
    pub stream_icon: Option<String>,
    pub rating: Option<String>,
    pub release_date: Option<String>,
    pub plot: Option<String>,
    pub container_extension: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigFields {
    pub url: String,
    pub username: String,
    pub password: String,
    pub focus: usize,
}

const MAX_LOG_LEN: usize = 500;

impl App {
    pub fn new() -> Result<Self> {
        let (log_tx, log_rx) = mpsc::channel();
        match config::load_env()? {
            Some(cfg) => {
                let db = Arc::new(Database::open("iptv_cache.db")?);
                log_tx
                    .send(format!("opened DB; Xtream URL: {}", cfg.url))
                    .ok();
                let mut s = Self {
                    config: Some(cfg),
                    db: Some(db),
                    focus: Focus::Categories,
                    categories: Vec::new(),
                    movies: Vec::new(),
                    category_offset: 0,
                    movie_offset: 0,
                    selected_category: None,
                    show_config: false,
                    config_fields: ConfigFields::default(),
                    status: String::new(),
                    logs: Vec::new(),
                    log_rx,
                    log_tx: log_tx.clone(),
                    syncing: false,
                };
                s.load_categories();
                let n_cats = s.categories.len();
                if n_cats > 0 {
                    let n_movies = 0;
                    s.status = format!("{n_cats} categories, {n_movies} movies cached");
                    s.log_tx
                        .send(format!("loaded {n_cats} categories from cache"))
                        .ok();
                } else {
                    s.status = "No cache — press r to sync".into();
                    s.log_tx.send("DB empty, waiting for sync".into()).ok();
                }
                Ok(s)
            }
            None => {
                log_tx
                    .send("no .env found, showing config screen".into())
                    .ok();
                Ok(Self {
                    config: None,
                    db: None,
                    focus: Focus::Categories,
                    categories: Vec::new(),
                    movies: Vec::new(),
                    category_offset: 0,
                    movie_offset: 0,
                    selected_category: None,
                    show_config: true,
                    config_fields: ConfigFields::default(),
                    status: "Configure your Xtream connection.".into(),
                    logs: Vec::new(),
                    log_rx,
                    log_tx,
                    syncing: false,
                })
            }
        }
    }

    pub fn drain_logs(&mut self) -> bool {
        let mut done = false;
        while let Ok(msg) = self.log_rx.try_recv() {
            if msg == "SYNC_DONE" {
                done = true;
                self.syncing = false;
                continue;
            }
            self.logs.push(msg);
            if self.logs.len() > MAX_LOG_LEN {
                self.logs.remove(0);
            }
        }
        done
    }

    pub fn log_tx(&self) -> mpsc::Sender<String> {
        self.log_tx.clone()
    }

    pub fn handle_paste(&mut self, text: &str) {
        if !self.show_config {
            return;
        }
        let sanitized: String = text.chars().filter(|&c| is_printable(c)).collect();
        let f = self.config_fields.focus;
        let field = match f {
            0 => &mut self.config_fields.url,
            1 => &mut self.config_fields.username,
            _ => &mut self.config_fields.password,
        };
        field.push_str(&sanitized);
    }

    pub fn handle_event(&mut self, key: KeyCode) -> bool {
        if self.show_config {
            return self.handle_config_event(key);
        }
        match key {
            KeyCode::Char('q') => return false,
            KeyCode::Char('h') | KeyCode::Left => {
                self.focus = match self.focus {
                    Focus::Movies => Focus::Categories,
                    Focus::Details => Focus::Movies,
                    other => other,
                };
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.focus = match self.focus {
                    Focus::Categories => Focus::Movies,
                    Focus::Movies => Focus::Details,
                    other => other,
                };
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Categories => Focus::Movies,
                    Focus::Movies => Focus::Details,
                    Focus::Details => Focus::Categories,
                };
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Categories => Focus::Details,
                    Focus::Movies => Focus::Categories,
                    Focus::Details => Focus::Movies,
                };
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(),
            KeyCode::Enter if self.focus == Focus::Categories => {
                if let Some(cat) = self.categories.get(self.category_offset) {
                    self.load_movies(cat.id);
                    self.focus = Focus::Movies;
                }
            }
            KeyCode::Char('r') => {
                self.status = "Syncing...".into();
            }
            _ => {}
        }
        true
    }

    fn handle_config_event(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('q') => return false,
            KeyCode::Tab => {
                self.config_fields.focus = (self.config_fields.focus + 1) % 3;
            }
            KeyCode::BackTab => {
                self.config_fields.focus = self.config_fields.focus.wrapping_sub(1) % 3;
            }
            KeyCode::Backspace => {
                let f = self.config_fields.focus;
                let field = match f {
                    0 => &mut self.config_fields.url,
                    1 => &mut self.config_fields.username,
                    _ => &mut self.config_fields.password,
                };
                field.pop();
            }
            KeyCode::Enter => {
                let cfg = Config {
                    url: self.config_fields.url.clone(),
                    username: self.config_fields.username.clone(),
                    password: self.config_fields.password.clone(),
                };
                if !cfg.url.is_empty() && !cfg.username.is_empty() && !cfg.password.is_empty() {
                    if config::write_env(&cfg.url, &cfg.username, &cfg.password).is_ok() {
                        match config::load_env() {
                            Ok(Some(c)) => {
                                let db = Arc::new(Database::open("iptv_cache.db").unwrap());
                                self.config = Some(c);
                                self.db = Some(db);
                                self.show_config = false;
                                self.log_tx.send("config saved, starting sync".into()).ok();
                                self.sync();
                            }
                            _ => self.status = "Failed to reload config.".into(),
                        }
                    } else {
                        self.status = "Failed to write .env.".into();
                    }
                }
            }
            KeyCode::Char(c) if is_printable(c) => {
                let f = self.config_fields.focus;
                let field = match f {
                    0 => &mut self.config_fields.url,
                    1 => &mut self.config_fields.username,
                    _ => &mut self.config_fields.password,
                };
                field.push(c);
            }
            _ => {}
        }
        true
    }

    fn scroll_down(&mut self) {
        match self.focus {
            Focus::Categories => {
                if self.category_offset + 1 < self.categories.len() {
                    self.category_offset += 1;
                }
            }
            Focus::Movies => {
                if self.movie_offset + 1 < self.movies.len() {
                    self.movie_offset += 1;
                }
            }
            Focus::Details => {}
        }
    }

    fn scroll_up(&mut self) {
        match self.focus {
            Focus::Categories => {
                self.category_offset = self.category_offset.saturating_sub(1);
            }
            Focus::Movies => {
                self.movie_offset = self.movie_offset.saturating_sub(1);
            }
            Focus::Details => {}
        }
    }

    pub fn load_categories(&mut self) {
        self.categories.clear();
        self.category_offset = 0;
        if let Some(ref db) = self.db {
            let conn = db.conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT id, name FROM categories ORDER BY name")
                .unwrap();
            let rows = stmt
                .query_map([], |row| {
                    Ok(CategoryItem {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                })
                .unwrap();
            for row in rows.flatten() {
                self.categories.push(row);
            }
        }
    }

    pub fn load_movies(&mut self, category_id: i64) {
        self.movies.clear();
        self.movie_offset = 0;
        self.selected_category = Some(category_id);
        if let Some(ref db) = self.db {
            let conn = db.conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT id, name, stream_icon, rating, release_date, plot, container_extension FROM movies WHERE category_id = ?1 ORDER BY name")
                .unwrap();
            let rows = stmt
                .query_map(rusqlite::params![category_id], |row| {
                    Ok(MovieItem {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        stream_icon: row.get(2)?,
                        rating: row.get(3)?,
                        release_date: row.get(4)?,
                        plot: row.get(5)?,
                        container_extension: row.get(6)?,
                    })
                })
                .unwrap();
            for row in rows.flatten() {
                self.movies.push(row);
            }
        }
    }

    pub fn sync(&mut self) {
        if self.syncing {
            return;
        }
        let cfg = match self.config.clone() {
            Some(c) => c,
            None => return,
        };
        let db = match self.db.clone() {
            Some(d) => d,
            None => return,
        };
        self.syncing = true;
        self.status = "Syncing...".into();
        let log = self.log_tx();
        tokio::spawn(async move {
            match api::sync_vod_categories(&cfg, &db).await {
                Ok(n) => log.send(format!("✓ {n} categories")).ok(),
                Err(e) => log.send(format!("✗ categories: {e}")).ok(),
            };
            match api::sync_vod_streams(&cfg, &db).await {
                Ok(n) => log.send(format!("✓ {n} movies")).ok(),
                Err(e) => log.send(format!("✗ movies: {e}")).ok(),
            };
            log.send("SYNC_DONE".into()).ok();
        });
    }
}
