use crate::api;
use crate::config::{self, Config};
use crate::db::Database;
use anyhow::Result;
use crossterm::event::KeyCode;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use regex::RegexBuilder;

fn hash_url(url: &str) -> u64 {
    let mut h = 5381u64;
    for b in url.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

fn is_printable(c: char) -> bool {
    c.is_ascii_graphic() || c == ' '
}

fn sort_name(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    name.nfd()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn open_log_file() -> Option<std::fs::File> {
    std::fs::create_dir_all("logs").ok()?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/tuiptv.log")
        .ok()
}

fn extract_year(date: Option<&String>) -> Option<i32> {
    let s = date?.trim();
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(3) {
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
        {
            let y = (bytes[i] - b'0') as i32 * 1000
                + (bytes[i + 1] - b'0') as i32 * 100
                + (bytes[i + 2] - b'0') as i32 * 10
                + (bytes[i + 3] - b'0') as i32;
            if (1900..=2099).contains(&y) {
                return Some(y);
            }
        }
    }
    None
}

fn parse_movie_tags(name: &str) -> (Option<String>, Option<String>, Option<String>, Option<i32>) {
    let upper = name.to_uppercase();
    let mut version = None;
    let mut audio = None;
    let mut subs = None;
    let mut year = None;

    for token in
        upper.split(|c: char| c == ' ' || c == '(' || c == ')' || c == '[' || c == ']' || c == '.')
    {
        match token {
            "4K" | "UHD" | "2160P" => version = Some("4K".into()),
            "FHD" | "1080P" => version = Some("FHD".into()),
            "HD" | "720P" => version = Some("HD".into()),
            "SD" | "480P" => version = Some("SD".into()),
            "MULTI" | "MULTILANGUE" | "MULTI-LANGUE" => audio = Some("Multi".into()),
            "VO" | "VOST" => audio = Some("VO".into()),
            "VF" | "VFF" | "TRUEFRENCH" | "FRENCH" => audio = Some("VF".into()),
            "VOSTFR" => {
                audio = Some("VO".into());
                subs = Some("ST Fr".into());
            }
            "ENGLISH" | "ANGLAIS" => audio = Some("EN".into()),
            "SUBFRENCH" | "SUBFR" => subs = Some("Sub Fr".into()),
            "SUBBED" | "SUB" => subs = Some("Sub".into()),
            _ => {}
        }
    }

    let bytes = name.as_bytes();
    let n = bytes.len();
    let mut found_paren_year = false;
    for i in 0..n.saturating_sub(5) {
        if (bytes[i] == b'(' || bytes[i] == b'[')
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
            && bytes[i + 4].is_ascii_digit()
            && (bytes[i + 5] == b')' || bytes[i + 5] == b']')
        {
            let y = (bytes[i + 1] - b'0') as i32 * 1000
                + (bytes[i + 2] - b'0') as i32 * 100
                + (bytes[i + 3] - b'0') as i32 * 10
                + (bytes[i + 4] - b'0') as i32;
            if (1900..=2099).contains(&y) {
                year = Some(y);
                found_paren_year = true;
            }
            break;
        }
    }
    if !found_paren_year && n >= 4 {
        if bytes[n - 4].is_ascii_digit()
            && bytes[n - 3].is_ascii_digit()
            && bytes[n - 2].is_ascii_digit()
            && bytes[n - 1].is_ascii_digit()
        {
            let y = (bytes[n - 4] - b'0') as i32 * 1000
                + (bytes[n - 3] - b'0') as i32 * 100
                + (bytes[n - 2] - b'0') as i32 * 10
                + (bytes[n - 1] - b'0') as i32;
            if (1900..=2099).contains(&y) {
                year = Some(y);
            }
        }
    }

    (version, audio, subs, year)
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
    pub log_file: Option<std::fs::File>,
    pub syncing: bool,
    pub column_ratios: [u16; 3],
    pub dragging_gutter: Option<usize>,
    pub term_width: u16,
    pub term_height: u16,
    pub poster_loading: bool,
    pub poster_rx: mpsc::Receiver<(u64, String, Option<(image::DynamicImage, Box<dyn StatefulProtocol>)>)>,
    pub poster_tx: mpsc::Sender<(u64, String, Option<(image::DynamicImage, Box<dyn StatefulProtocol>)>)>,
    pub font_size: (u16, u16),
    pub poster_protocol: RefCell<Option<Box<dyn StatefulProtocol>>>,
    pub poster_img_cache: HashMap<String, image::DynamicImage>,
    pub cat_scroll_pos: HashMap<i64, usize>,
    pub poster_gen: Arc<AtomicU64>,
    pub picker: Picker,
    pub show_posters: bool,
    pub current_poster_url: Option<String>,
    pub poster_debug: String,
    pub show_help: bool,
    pub show_logs: bool,
    pub show_full_poster: bool,
    pub cat_sort_mode: u8,
    pub movie_sort: u8,
    pub show_filter: bool,
    pub filter_query: String,
    pub filtered_movies: Vec<MovieItem>,
    pub pending_g: bool,
    pub wishlist: HashSet<i64>,
    pub show_stats: bool,
    pub stats_total_movies: usize,
    pub stats_years: Vec<(i32, usize)>,
    pub pending_url: Option<(String, u64)>,
    pub table_mode: bool,
    pub show_movie_popover: bool,
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
    pub genre: Option<String>,
    pub cast: Option<String>,
    pub director: Option<String>,
    pub year: Option<i32>,
    pub version: Option<String>,
    pub audio: Option<String>,
    pub subs: Option<String>,
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
        let (poster_tx, poster_rx): (
            mpsc::Sender<(u64, String, Option<(image::DynamicImage, Box<dyn StatefulProtocol>)>)>,
            mpsc::Receiver<(u64, String, Option<(image::DynamicImage, Box<dyn StatefulProtocol>)>)>,
        ) = mpsc::channel();
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
                    log_file: open_log_file(),
                    syncing: false,
                    column_ratios: [20, 45, 35],
                    dragging_gutter: None,
                    term_width: 0,
                    term_height: 0,
                    poster_loading: false,
                    poster_rx,
                    poster_tx: poster_tx.clone(),
                    font_size: (8, 16),
                    poster_protocol: RefCell::new(None),
                    cat_scroll_pos: HashMap::new(),
                    poster_gen: Arc::new(AtomicU64::new(0)),
                    picker: Picker::new((8, 16)),
                    show_posters: true,
                    current_poster_url: None,
                    poster_debug: String::new(),
                    poster_img_cache: HashMap::new(),
                    show_help: false,
                    show_logs: false,
                    show_full_poster: false,
                    cat_sort_mode: 0,
                    movie_sort: 0,
                    show_filter: false,
                    filter_query: String::new(),
                    filtered_movies: Vec::new(),
                    pending_g: false,
                    wishlist: HashSet::new(),
                    show_stats: false,
                    stats_total_movies: 0,
                    stats_years: Vec::new(),
                    pending_url: None,
                    table_mode: false,
                    show_movie_popover: false,
                };
                s.load_layout();
                if let Some(ref db) = s.db {
                    s.wishlist = db.load_wishlist();
                }
                s.load_categories();
                let n_cats = s.categories.len();
if n_cats > 0 {
                    let target = if !s.wishlist.is_empty() {
                        Some(-1)
                    } else {
                        s.selected_category
                            .filter(|&id| s.categories.iter().any(|c| c.id == id))
                    }
                    .unwrap_or(s.categories[0].id);
                    s.load_movies(target);
                    s.load_current_poster();
                    let n_movies = s.movies.len();
                    s.status = format!("{n_cats} categories, {n_movies} movies");
                    s.log_tx
                        .send(format!("loaded {n_cats} categories, {n_movies} movies"))
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
                    log_file: open_log_file(),
                    syncing: false,
                    column_ratios: [20, 45, 35],
                    dragging_gutter: None,
                    term_width: 0,
                    term_height: 0,
                    poster_loading: false,
                    poster_rx,
                    poster_tx: poster_tx,
                    font_size: (8, 16),
                    poster_protocol: RefCell::new(None),
                    cat_scroll_pos: HashMap::new(),
                    poster_gen: Arc::new(AtomicU64::new(0)),
                    picker: Picker::new((8, 16)),
                    show_posters: true,
                    current_poster_url: None,
                    poster_debug: String::new(),
                    poster_img_cache: HashMap::new(),
                    show_help: false,
                    show_logs: false,
                    show_full_poster: false,
                    cat_sort_mode: 0,
                    movie_sort: 0,
                    show_filter: false,
                    filter_query: String::new(),
                    filtered_movies: Vec::new(),
                    pending_g: false,
                    wishlist: HashSet::new(),
                    show_stats: false,
                    stats_total_movies: 0,
                    stats_years: Vec::new(),
                    pending_url: None,
                    table_mode: false,
                    show_movie_popover: false,
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
            if let Some(ref mut f) = self.log_file {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                use std::io::Write;
                let _ = writeln!(f, "[{ts}] {msg}");
            }
            self.logs.push(msg);
            if self.logs.len() > MAX_LOG_LEN {
                self.logs.remove(0);
            }
        }
        done
    }

pub fn drain_posters(&mut self) {
        // Process pending poster URL (deferred from load_current_poster — once per frame)
        if let Some((url, gen)) = self.pending_url.take() {
            if self.poster_img_cache.contains_key(&url) {
                self.log_tx.send("🖼  poster: mem cache hit".into()).ok();
                let img = self.poster_img_cache.get(&url).unwrap();
                let protocol = self.picker.new_resize_protocol(img.clone());
                self.poster_protocol.replace(Some(protocol));
                return;
            }
            let cache_path = format!("poster_cache/{:016x}", hash_url(&url));
            let from_disk = std::fs::metadata(&cache_path).is_ok();
            if from_disk {
                self.log_tx.send(format!("🖼  poster: disk cache hit ({cache_path})")).ok();
                let gen_arc = self.poster_gen.clone();
                let tx = self.poster_tx.clone();
                let picker = self.picker;
                tokio::spawn(async move {
                    let bytes = match std::fs::read(&cache_path) {
                        Ok(d) => d,
                        Err(_) => {
                            let _ = tx.send((gen, url, None));
                            return;
                        }
                    };
                    if gen_arc.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    let tx_clone = tx.clone();
                    let mut blocking_picker = picker;
                    let res = tokio::task::spawn_blocking(move || {
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if gen_arc.load(Ordering::SeqCst) != gen {
                                return None;
                            }
                            match image::load_from_memory(&bytes) {
                                Ok(img) => {
                                    let p = blocking_picker.new_resize_protocol(img.clone());
                                    Some((img, p))
                                }
                                Err(_) => None,
                            }
                        }))
                        .ok()
                        .flatten()
                    })
                    .await;
                    match res {
                        Ok(Some((img, protocol))) => {
                            let _ = tx_clone.send((gen, url, Some((img, protocol))));
                        }
                        _ => {
                            let _ = tx_clone.send((gen, url, None));
                        }
                    }
                });
            } else {
                self.log_tx.send(format!("🖼  poster: HTTP fetch {url}")).ok();
                let gen_arc = self.poster_gen.clone();
                let tx = self.poster_tx.clone();
                let picker = self.picker;
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    let cache_path = format!("poster_cache/{:016x}", hash_url(&url));
                    let bytes = match std::fs::read(&cache_path) {
                        Ok(d) => d,
                        Err(_) => {
                            let client = reqwest::Client::builder()
                                .timeout(std::time::Duration::from_secs(15))
                                .build()
                                .unwrap();
                            let resp = match client.get(&url).send().await {
                                Ok(r) => r,
                                Err(_) => {
                                    let _ = tx.send((gen, url, None));
                                    return;
                                }
                            };
                            if gen_arc.load(Ordering::SeqCst) != gen {
                                return;
                            }
                            let data = match resp.bytes().await {
                                Ok(b) => b.to_vec(),
                                Err(_) => {
                                    let _ = tx.send((gen, url, None));
                                    return;
                                }
                            };
                            if gen_arc.load(Ordering::SeqCst) != gen {
                                return;
                            }
                            let _ = std::fs::create_dir_all("poster_cache");
                            let _ = std::fs::write(&cache_path, &data);
                            data
                        }
                    };
                    if gen_arc.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    let tx_clone = tx.clone();
                    let mut blocking_picker = picker;
                    let res = tokio::task::spawn_blocking(move || {
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if gen_arc.load(Ordering::SeqCst) != gen {
                                return None;
                            }
                            match image::load_from_memory(&bytes) {
                                Ok(img) => {
                                    let p = blocking_picker.new_resize_protocol(img.clone());
                                    Some((img, p))
                                }
                                Err(_) => None,
                            }
                        }))
                        .ok()
                        .flatten()
                    })
                    .await;
                    match res {
                        Ok(Some((img, protocol))) => {
                            let _ = tx_clone.send((gen, url, Some((img, protocol))));
                        }
                        _ => {
                            let _ = tx_clone.send((gen, url, None));
                        }
                    }
                });
            }
        }

        let current_gen = self.poster_gen.load(Ordering::SeqCst);
        while let Ok((msg_gen, msg_url, result)) = self.poster_rx.try_recv() {
            let is_current = msg_gen == current_gen
                && self.current_poster_url.as_ref() == Some(&msg_url);
            match result {
                Some((img, protocol)) => {
                    // Cache decoded image for instant revisit (max 50)
                    self.poster_img_cache.insert(msg_url.clone(), img);
                    if self.poster_img_cache.len() > 50 {
                        if let Some(key) = self.poster_img_cache.keys().next().cloned() {
                            self.poster_img_cache.remove(&key);
                        }
                    }
                    if is_current {
                        self.poster_loading = false;
                        self.poster_debug = format!("displayed (gen={msg_gen})");
                        self.poster_protocol.replace(Some(protocol));
                    }
                }
                None => {
                    if is_current {
                        self.poster_loading = false;
                        self.poster_debug = format!("load failed (gen={msg_gen})");
                        self.poster_protocol.replace(None);
                    }
                }
            }
        }
    }

    fn load_current_poster(&mut self) {
        self.pending_url = None;
        self.current_poster_url.take();

        if !self.show_posters {
            self.poster_loading = false;
            return;
        }
        let list = self.active_movies();
        let url = list
            .get(self.movie_offset)
            .and_then(|m| m.stream_icon.as_ref())
            .filter(|u| !u.is_empty())
            .cloned();
        if let Some(u) = url {
            if let Some(img) = self.poster_img_cache.get(&u) {
                let protocol = self.picker.new_resize_protocol(img.clone());
                self.poster_protocol.replace(Some(protocol));
                self.current_poster_url = Some(u);
                return;
            }
            let gen = self.poster_gen.fetch_add(1, Ordering::SeqCst) + 1;
            self.pending_url = Some((u.clone(), gen));
            self.current_poster_url = Some(u);
        }
    }

    pub fn load_stats(&mut self) {
        self.stats_total_movies = 0;
        self.stats_years.clear();
        if let Some(ref db) = self.db {
            let conn = db.conn.lock().unwrap();
            if let Ok(count) = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| {
                r.get::<_, usize>(0)
            }) {
                self.stats_total_movies = count;
            }
            let mut stmt = conn
                .prepare("SELECT name, release_date FROM movies")
                .unwrap();
            let mut year_counts: std::collections::HashMap<i32, usize> =
                std::collections::HashMap::new();
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                })
                .unwrap();
            for row in rows.flatten() {
                let (name, release_date) = row;
                let year = extract_year(release_date.as_ref())
                    .or_else(|| parse_movie_tags(&name).3);
                if let Some(y) = year {
                    *year_counts.entry(y).or_insert(0) += 1;
                }
            }
            let mut pairs: Vec<(i32, usize)> = year_counts.into_iter().collect();
            pairs.sort_by(|a, b| b.0.cmp(&a.0));
            self.stats_years = pairs;
        }
    }

    pub fn save_layout(&self) {
        let s = format!(
            "{},{},{},{},{},{},{},{},{}",
            self.column_ratios[0],
            self.column_ratios[1],
            self.column_ratios[2],
            self.show_logs as u8,
            self.cat_sort_mode,
            self.movie_sort,
            self.selected_category.unwrap_or(-2),
            self.movie_offset,
            self.table_mode as u8,
        );
        let _ = std::fs::write("iptv_layout.dat", s);
    }

    pub fn load_layout(&mut self) {
        if let Ok(s) = std::fs::read_to_string("iptv_layout.dat") {
            let parts: Vec<&str> = s.trim().split(',').collect();
            if parts.len() >= 3 {
                if let (Ok(a), Ok(b), Ok(c)) =
                    (parts[0].parse(), parts[1].parse(), parts[2].parse())
                {
                    if a >= 5 && b >= 5 && c >= 5 {
                        self.column_ratios = [a, b, c];
                    }
                }
                if parts.len() >= 4 {
                    self.show_logs = parts[3] == "1";
                }
                if parts.len() >= 5 {
                    self.cat_sort_mode = parts[4].parse().unwrap_or(0);
                }
                if parts.len() >= 6 {
                    self.movie_sort = parts[5].parse().unwrap_or(0);
                }
                if parts.len() >= 7 {
                    let cat: i64 = parts[6].parse().unwrap_or(-2);
                    if cat >= -1 {
                        self.selected_category = Some(cat);
                    }
                }
                if parts.len() >= 8 {
                    self.movie_offset = parts[7].parse().unwrap_or(0);
                }
                if parts.len() >= 9 {
                    self.table_mode = parts[8].parse::<u8>().unwrap_or(0) != 0;
                }
            }
        }
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
        if self.show_help {
            self.show_help = false;
            return true;
        }
        if self.show_full_poster {
            self.show_full_poster = false;
            return true;
        }
        if self.show_stats {
            if key == KeyCode::Char('h') || key == KeyCode::Esc {
                self.show_stats = false;
            }
            return true;
        }
        if self.show_movie_popover {
            if key == KeyCode::Esc || key == KeyCode::Char('q') || key == KeyCode::Enter {
                self.show_movie_popover = false;
            }
            return true;
        }
        if self.show_filter && self.focus == Focus::Movies {
            match key {
                KeyCode::Esc => {
                    let keep_id = self.active_movies().get(self.movie_offset).map(|m| m.id);
                    self.show_filter = false;
                    self.filter_query.clear();
                    self.filtered_movies.clear();
                    if let Some(id) = keep_id {
                        if let Some(pos) = self.movies.iter().position(|m| m.id == id) {
                            self.movie_offset = pos;
                        }
                    }
                    return true;
                }
                KeyCode::Enter => {
                    let keep_id = self.active_movies().get(self.movie_offset).map(|m| m.id);
                    self.show_filter = false;
                    if !self.filter_query.is_empty() {
                        self.apply_filter();
                    } else {
                        self.filtered_movies.clear();
                    }
                    self.filter_query.clear();
                    if let Some(id) = keep_id {
                        if let Some(pos) = self.movies.iter().position(|m| m.id == id) {
                            self.movie_offset = pos;
                        }
                    }
                    self.load_current_poster();
                    return true;
                }
                KeyCode::Backspace => {
                    self.filter_query.pop();
                    self.apply_filter();
                    return true;
                }
                KeyCode::Char(c) if is_printable(c) => {
                    self.filter_query.push(c);
                    self.apply_filter();
                    return true;
                }
                _ => {}
            }
        }

        // gg / G handling
        if self.pending_g {
            self.pending_g = false;
            if key == KeyCode::Char('g') && self.focus != Focus::Details {
                match self.focus {
                    Focus::Categories => {
                        self.category_offset = 0;
                        if let Some(cat) = self.categories.get(0) {
                            self.load_movies(cat.id);
                        }
                    }
                    _ => {
                        self.movie_offset = 0;
                        self.load_current_poster();
                    }
                }
                return true;
            }
            // fall through to handle the current key normally
        }
        match key {
            KeyCode::Char('q') => return false,
            KeyCode::Char('h') => {
                self.show_stats = !self.show_stats;
                if self.show_stats {
                    self.load_stats();
                }
            }
            KeyCode::Left => {
                if self.focus == Focus::Movies {
                    self.focus = Focus::Categories;
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.focus == Focus::Categories {
                    self.focus = Focus::Movies;
                    self.load_current_poster();
                } else {
                    self.table_mode = !self.table_mode;
                    self.poster_protocol.replace(None);
                    if !self.table_mode {
                        self.load_current_poster();
                    }
                }
            }
            KeyCode::Tab => {
                let prev = self.focus;
                self.focus = if self.focus == Focus::Categories {
                    Focus::Movies
                } else {
                    Focus::Categories
                };
                if prev == Focus::Categories && self.focus == Focus::Movies {
                    self.load_current_poster();
                }
            }
            KeyCode::BackTab => {
                let prev = self.focus;
                self.focus = if self.focus == Focus::Movies {
                    Focus::Categories
                } else {
                    Focus::Movies
                };
                if prev == Focus::Categories && self.focus == Focus::Movies {
                    self.load_current_poster();
                }
            }
            KeyCode::Char('(') => self.resize_focused(-3),
            KeyCode::Char(')') => self.resize_focused(3),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(),
            KeyCode::Char('g') if self.focus != Focus::Details => {
                self.pending_g = true;
            }
            KeyCode::Char('G') if self.focus != Focus::Details => match self.focus {
                Focus::Categories => {
                    self.category_offset = self.categories.len().saturating_sub(1);
                    if let Some(cat) = self.categories.get(self.category_offset) {
                        self.load_movies(cat.id);
                    }
                }
                _ => {
                    let n = self.active_movies().len();
                    self.movie_offset = n.saturating_sub(1);
                    self.load_current_poster();
                }
            },
            KeyCode::Enter if self.show_movie_popover => {
                self.show_movie_popover = false;
            }
            KeyCode::Enter if self.focus == Focus::Movies => {
                self.show_movie_popover = true;
                self.load_current_poster();
            }
            KeyCode::Enter if self.focus == Focus::Categories => {
                if let Some(cat) = self.categories.get(self.category_offset) {
                    if self.selected_category != Some(cat.id) {
                        self.load_movies(cat.id);
                    }
                    self.focus = Focus::Movies;
                    self.load_current_poster();
                }
            }
            KeyCode::Char('r') => self.sort_movies_by_rating(),
            KeyCode::Char('a') => self.sort_movies_by_name(),
            KeyCode::Char('i') => {
                self.show_posters = !self.show_posters;
                self.load_current_poster();
            }
            KeyCode::Char('f') => {
                self.show_full_poster = !self.show_full_poster;
            }
            KeyCode::Char('v') => {
                if let (Some(cfg), Some(m)) =
                    (self.config.as_ref(), self.movies.get(self.movie_offset))
                {
                    let url =
                        cfg.stream_url(m.id, m.container_extension.as_deref().unwrap_or("mp4"));
                    self.log_tx
                        .send(format!("▶ VLC: {}", &url[..url.len().min(120)]))
                        .ok();
                    #[cfg(target_os = "macos")]
                    std::process::Command::new("open")
                        .args(["-a", "VLC", &url])
                        .spawn()
                        .ok();
                    #[cfg(target_os = "linux")]
                    std::process::Command::new("vlc").arg(&url).spawn().ok();
                }
            }
            KeyCode::Char('y') => self.sort_movies_by_year(),
            KeyCode::Char('c') => self.cycle_cat_sort(),
            KeyCode::Char('w') => {
                if let Some(pos) = self.categories.iter().position(|c| c.id == -1) {
                    self.category_offset = pos;
                    self.load_movies(-1);
                }
            }
            KeyCode::Char('L') => {
                // Force re-load poster from disk cache
                let url = self.active_movies().get(self.movie_offset)
                    .and_then(|m| m.stream_icon.as_ref())
                    .filter(|u| !u.is_empty())
                    .cloned();
                if let Some(ref u) = url {
                    self.poster_img_cache.remove(u);
                }
                self.load_current_poster();
            }
            KeyCode::Char('o') => {
                self.movie_sort = 0;
                self.apply_movie_sort();
                self.status = "default order".into();
                self.log_tx.send("sort: default".into()).ok();
            }
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('/') => {
                self.focus = Focus::Movies;
                self.show_filter = true;
                self.filtered_movies.clear();
            }
            KeyCode::Char(' ') if self.focus == Focus::Movies || self.focus == Focus::Details => {
                self.toggle_wishlist();
            }
            _ => {}
        }
        true
    }

    fn apply_movie_sort(&mut self) {
        if self.movies.is_empty() {
            return;
        }
        let current_id = self.movies.get(self.movie_offset).map(|m| m.id);
        match self.movie_sort {
            0 => self.movies.sort_by(|a, b| sort_name(&a.name).cmp(&sort_name(&b.name))),
            1 => self.movies.sort_by(|a, b| b.year.cmp(&a.year)),
            2 => self.movies.sort_by(|a, b| a.year.cmp(&b.year)),
            3 => {
                self.movies.sort_by(|a, b| {
                    let ra = a
                        .rating
                        .as_deref()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let rb = b
                        .rating
                        .as_deref()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            4 => {
                self.movies.sort_by(|a, b| {
                    let ra = a
                        .rating
                        .as_deref()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let rb = b
                        .rating
                        .as_deref()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            5 => self.movies.sort_by(|a, b| sort_name(&a.name).cmp(&sort_name(&b.name))),
            6 => self.movies.sort_by(|a, b| sort_name(&b.name).cmp(&sort_name(&a.name))),
            _ => {}
        }
        if let Some(id) = current_id {
            if let Some(pos) = self.movies.iter().position(|m| m.id == id) {
                self.movie_offset = pos;
            }
        }
        if self.focus == Focus::Movies {
            self.load_current_poster();
        }
    }

    fn sort_movies_by_year(&mut self) {
        self.movie_sort = match self.movie_sort {
            0 | 2 => 1,
            1 => 2,
            _ => 1,
        };
        let label = match self.movie_sort {
            1 => "year ↓",
            2 => "year ↑",
            _ => "default",
        };
        self.status = format!("sorted by {label}");
        self.log_tx.send(format!("sort: {label}")).ok();
        self.apply_movie_sort();
    }

    fn sort_movies_by_rating(&mut self) {
        self.movie_sort = match self.movie_sort {
            0 => 3,
            3 => 4,
            4 => 0,
            _ => 3,
        };
        let label = match self.movie_sort {
            3 => "rating ↓",
            4 => "rating ↑",
            _ => "default",
        };
        self.status = format!("sorted by {label}");
        self.log_tx.send(format!("sort: {label}")).ok();
        self.apply_movie_sort();
    }

    fn sort_movies_by_name(&mut self) {
        self.movie_sort = match self.movie_sort {
            0 | 6 => 5,
            5 => 6,
            _ => 5,
        };
        let label = match self.movie_sort {
            5 => "name ↑",
            6 => "name ↓",
            _ => "default",
        };
        self.status = format!("sorted by {label}");
        self.log_tx.send(format!("sort: {label}")).ok();
        self.apply_movie_sort();
    }

    fn cycle_cat_sort(&mut self) {
        self.cat_sort_mode = (self.cat_sort_mode + 1) % 3;
        let label = match self.cat_sort_mode {
            0 => "name A→Z",
            1 => "provider order",
            _ => "name Z→A",
        };
        self.status = format!("categories: {label}");
        self.log_tx.send(format!("cat sort: {label}")).ok();
        self.load_categories();
    }

    pub fn toggle_wishlist(&mut self) {
        let movie = self.movies.get(self.movie_offset).map(|m| (m.id, m.name.clone()));
        if let (Some((movie_id, ref movie_name)), Some(ref db)) = (movie, self.db.as_ref()) {
            let added = db.toggle_wishlist(movie_id);
            if added {
                self.wishlist.insert(movie_id);
                self.log_tx
                    .send(format!("❤️ added to wishlist: {movie_name}"))
                    .ok();
            } else {
                self.wishlist.remove(&movie_id);
                self.log_tx
                    .send(format!("💔 removed from wishlist: {movie_name}"))
                    .ok();
            }
            // Update the wishlist category entry in-place
            let wc = self.wishlist.len();
            if wc > 0 {
                if self.categories.first().map(|c| c.id) == Some(-1) {
                    self.categories[0].name = format!("❤️ Wishlist ({wc})");
                } else {
                    self.categories.insert(
                        0,
                        CategoryItem {
                            id: -1,
                            name: format!("❤️ Wishlist ({wc})"),
                        },
                    );
                }
            } else {
                self.categories.retain(|c| c.id != -1);
            }
            // If viewing wishlist category, reload movies
            if self.selected_category == Some(-1) {
                self.load_movies(-1);
            }
        }
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
                let old = self.category_offset;
                if self.category_offset + 1 < self.categories.len() {
                    self.category_offset += 1;
                }
                if self.category_offset != old {
                    if let Some(cat) = self.categories.get(self.category_offset) {
                        self.load_movies(cat.id);
                    }
                }
            }
            Focus::Movies => {
                let list = self.active_movies();
                if self.movie_offset + 1 < list.len() {
                    self.movie_offset += 1;
                    self.load_current_poster();
                }
            }
            Focus::Details => {}
        }
    }

    fn scroll_up(&mut self) {
        match self.focus {
            Focus::Categories => {
                let old = self.category_offset;
                self.category_offset = self.category_offset.saturating_sub(1);
                if self.category_offset != old {
                    if let Some(cat) = self.categories.get(self.category_offset) {
                        self.load_movies(cat.id);
                    }
                }
            }
            Focus::Movies => {
                self.movie_offset = self.movie_offset.saturating_sub(1);
                self.load_current_poster();
            }
            Focus::Details => {}
        }
    }

    pub fn active_movies(&self) -> &[MovieItem] {
        if self.show_filter && !self.filtered_movies.is_empty() {
            &self.filtered_movies
        } else {
            &self.movies
        }
    }

    fn apply_filter(&mut self) {
        if self.filter_query.is_empty() {
            self.filtered_movies.clear();
            return;
        }
        let pattern = RegexBuilder::new(&self.filter_query)
            .case_insensitive(true)
            .build();
        match pattern {
            Ok(re) => {
                self.filtered_movies = self
                    .movies
                    .iter()
                    .filter(|m| re.is_match(&m.name))
                    .cloned()
                    .collect();
            }
            Err(_) => {
                // fallback: case-insensitive substring
                let lower = self.filter_query.to_lowercase();
                self.filtered_movies = self
                    .movies
                    .iter()
                    .filter(|m| m.name.to_lowercase().contains(&lower))
                    .cloned()
                    .collect();
            }
        }
        if self.movie_offset >= self.filtered_movies.len() {
            self.movie_offset = self.filtered_movies.len().saturating_sub(1);
        }
        self.load_current_poster();
    }

    pub fn load_categories(&mut self) {
        self.categories.clear();
        self.category_offset = 0;
        // Synthetic wishlist category at top
        let wish_count = self.wishlist.len();
        if wish_count > 0 {
            self.categories.push(CategoryItem {
                id: -1,
                name: format!("❤️ Wishlist ({wish_count})"),
            });
        }
        if let Some(ref db) = self.db {
            let conn = db.conn.lock().unwrap();
            let order = match self.cat_sort_mode {
                0 => "name",
                1 => "id",
                _ => "name DESC",
            };
            let sql = format!("SELECT id, name FROM categories ORDER BY {order}");
            let mut stmt = conn.prepare(&sql).unwrap();
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
        if let Some(old) = self.selected_category {
            if old != category_id {
                self.cat_scroll_pos.insert(old, self.movie_offset);
            }
        }
        self.movies.clear();
        self.movie_offset = 0;
        self.selected_category = Some(category_id);
        self.show_filter = false;
        self.filter_query.clear();
        self.filtered_movies.clear();
        if category_id == -1 {
            // Wishlist — load movies by IDs
            let ids: Vec<String> = self.wishlist.iter().map(|id| id.to_string()).collect();
            if !ids.is_empty() {
                if let Some(ref db) = self.db {
                    let conn = db.conn.lock().unwrap();
                    let sql = format!(
                        "SELECT id, name, stream_icon, rating, release_date, plot, container_extension, genre, \"cast\", director FROM movies WHERE id IN ({}) ORDER BY name",
                        ids.join(",")
                    );
                    let mut stmt = conn.prepare(&sql).unwrap();
                    let rows = stmt
                        .query_map([], |row| {
                            let name: String = row.get(1)?;
                            let release_date: Option<String> = row.get(4)?;
                            let (version, audio, subs, year_from_name) = parse_movie_tags(&name);
                            let year = extract_year(release_date.as_ref())
                                .or(year_from_name);
                            Ok(MovieItem {
                                id: row.get(0)?,
                                name,
                                stream_icon: row.get(2)?,
                                rating: row.get(3)?,
                                release_date,
                                plot: row.get(5)?,
                                container_extension: row.get(6)?,
                                genre: row.get(7)?,
                                cast: row.get(8)?,
                                director: row.get(9)?,
                                year,
                                version,
                                audio,
                                subs,
                            })
                        })
                        .unwrap();
                    for row in rows.flatten() {
                        self.movies.push(row);
                    }
                }
            }
        } else if let Some(ref db) = self.db {
            let conn = db.conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT id, name, stream_icon, rating, release_date, plot, container_extension, genre, \"cast\", director FROM movies WHERE category_id = ?1 ORDER BY name")
                .unwrap();
            let rows = stmt
                .query_map(rusqlite::params![category_id], |row| {
                    let name: String = row.get(1)?;
                    let release_date: Option<String> = row.get(4)?;
                    let (version, audio, subs, year_from_name) = parse_movie_tags(&name);
                    let year = release_date
                        .as_ref()
                        .and_then(|d| d[..4].parse::<i32>().ok())
                        .or(year_from_name);
                    Ok(MovieItem {
                        id: row.get(0)?,
                        name,
                        stream_icon: row.get(2)?,
                        rating: row.get(3)?,
                        release_date,
                        plot: row.get(5)?,
                        container_extension: row.get(6)?,
                        genre: row.get(7)?,
                        cast: row.get(8)?,
                        director: row.get(9)?,
                        year,
                        version,
                        audio,
                        subs,
                    })
                })
                .unwrap();
            for row in rows.flatten() {
                self.movies.push(row);
            }
        }
        if self.movie_sort != 0 {
            self.apply_movie_sort();
        }
        if let Some(&pos) = self.cat_scroll_pos.get(&category_id) {
            let max = self.movies.len().saturating_sub(1);
            self.movie_offset = pos.min(max);
        }
        if self.focus == Focus::Movies {
            self.load_current_poster();
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
        let started = std::time::Instant::now();
        tokio::spawn(async move {
            log.send("⤓ fetching categories...".into()).ok();
            let t0 = std::time::Instant::now();
            match api::sync_vod_categories(&cfg, &db).await {
                Ok(n) => log
                    .send(format!(
                        "✓ {n} categories in {}ms",
                        t0.elapsed().as_millis()
                    ))
                    .ok(),
                Err(e) => log.send(format!("✗ categories: {e}")).ok(),
            };
            log.send("⤓ fetching movies...".into()).ok();
            let t1 = std::time::Instant::now();
            match api::sync_vod_streams(&cfg, &db).await {
                Ok(n) => log
                    .send(format!(
                        "✓ {n} movies in {}ms",
                        t1.elapsed().as_millis()
                    ))
                    .ok(),
                Err(e) => log.send(format!("✗ movies: {e}")).ok(),
            };
            log.send(format!("⤓ done in {}ms", started.elapsed().as_millis())).ok();
            log.send("SYNC_DONE".into()).ok();
        });
    }

    fn resize_focused(&mut self, delta: i16) {
        let idx = match self.focus {
            Focus::Categories => 0,
            Focus::Movies => 1,
            Focus::Details => 2,
        };
        let old = self.column_ratios[idx];
        let new = (old as i16 + delta).clamp(5, 80) as u16;
        let diff = new as i16 - old as i16;
        self.column_ratios[idx] = new;
        let neighbor = if idx < 2 { idx + 1 } else { idx - 1 };
        let n_old = self.column_ratios[neighbor];
        let n_new = (n_old as i16 - diff).clamp(5, 80) as u16;
        let adjust = n_new as i16 - n_old as i16;
        self.column_ratios[neighbor] = n_new;
        let third = 3 - idx - neighbor;
        self.column_ratios[third] =
            ((self.column_ratios[third] as i16 - adjust) as i16).clamp(5, 80) as u16;
        let sum: u16 = self.column_ratios.iter().sum();
        if sum != 100 {
            let diff = 100i16 - sum as i16;
            let target = if diff > 0 { third } else { neighbor };
            self.column_ratios[target] =
                (self.column_ratios[target] as i16 + diff).clamp(5, 80) as u16;
        }
    }

    fn gutter_positions(&self) -> [[u16; 2]; 2] {
        let w = self.term_width.max(1);
        let g0 = w * self.column_ratios[0] / 100;
        let g1 = w * (self.column_ratios[0] + self.column_ratios[1]) / 100;
        [
            [g0.saturating_sub(1), g0.saturating_add(1)],
            [g1.saturating_sub(1), g1.saturating_add(1)],
        ]
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) {
        let col = event.column;
        let row = event.row;
        let main_top = 3u16;
        let main_bot = self.term_height.saturating_sub(5);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if row < main_top || row >= main_bot {
                    return;
                }
                for (i, &[lo, hi]) in self.gutter_positions().iter().enumerate() {
                    if col >= lo && col <= hi {
                        self.dragging_gutter = Some(i);
                        return;
                    }
                }
                // Click in non-gutter area → set focus or toggle poster
                let w = self.term_width.max(1);
                let g1 = w * (self.column_ratios[0] + self.column_ratios[1]) / 100;
                if col > g1 {
                    self.show_full_poster = !self.show_full_poster;
                } else if col < w * self.column_ratios[0] / 100 {
                    self.focus = Focus::Categories;
                } else {
                    self.focus = Focus::Movies;
                    self.load_current_poster();
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(idx) = self.dragging_gutter {
                    let w = self.term_width.max(1);
                    let pct = (col as f32 / w as f32 * 100.0).round() as u16;
                    let pct = pct.clamp(5, 95);
                    if idx == 0 {
                        let new_cat =
                            pct.min(100u16.saturating_sub(self.column_ratios[2] + 5).min(80));
                        let remaining = 100 - new_cat;
                        let new_det = self.column_ratios[2].clamp(5, remaining.saturating_sub(5));
                        let new_mov = remaining - new_det;
                        if new_mov >= 5 && new_cat >= 5 {
                            self.column_ratios = [new_cat, new_mov, new_det];
                        }
                    } else {
                        let cat = self.column_ratios[0];
                        let new_cat_mov = pct.max(cat + 5).min(95);
                        let new_mov = new_cat_mov - cat;
                        let new_det = (100 - cat - new_mov).clamp(5, 80);
                        let new_mov = (100 - cat - new_det).clamp(5, 80);
                        if new_mov >= 5 && new_det >= 5 {
                            self.column_ratios = [cat, new_mov, new_det];
                        }
                    }
                }
            }
            MouseEventKind::Up(_) => {
                self.dragging_gutter = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_url_is_deterministic() {
        let h = hash_url("http://example.com/poster.jpg");
        assert_eq!(h, hash_url("http://example.com/poster.jpg"));
    }

    #[test]
    fn hash_url_differs_for_diff_urls() {
        assert_ne!(hash_url("a"), hash_url("b"));
    }

    #[test]
    fn hash_url_empty() {
        let h = hash_url("");
        assert_eq!(h, hash_url(""));
    }

    #[test]
    fn is_printable_graphic() {
        assert!(is_printable('a'));
        assert!(is_printable('Z'));
        assert!(is_printable('0'));
        assert!(is_printable('.'));
        assert!(is_printable(' '));
    }

    #[test]
    fn is_printable_control() {
        assert!(!is_printable('\n'));
        assert!(!is_printable('\t'));
        assert!(!is_printable('\0'));
    }

    #[test]
    fn parse_tags_empty() {
        let (v, a, s, y) = parse_movie_tags("");
        assert_eq!(v, None);
        assert_eq!(a, None);
        assert_eq!(s, None);
        assert_eq!(y, None);
    }

    #[test]
    fn parse_tags_no_tags() {
        let (v, a, s, y) = parse_movie_tags("Casablanca");
        assert_eq!(v, None);
        assert_eq!(a, None);
        assert_eq!(s, None);
        assert_eq!(y, None);
    }

    #[test]
    fn parse_tags_multi_fhd_year() {
        let (v, a, s, y) = parse_movie_tags("Star Wars (MULTI) FHD (2024)");
        assert_eq!(v, Some("FHD".into()));
        assert_eq!(a, Some("Multi".into()));
        assert_eq!(s, None);
        assert_eq!(y, Some(2024));
    }

    #[test]
    fn parse_tags_4k_vostfr() {
        let (v, a, s, _y) = parse_movie_tags("The Matrix 4K VOSTFR");
        assert_eq!(v, Some("4K".into()));
        assert_eq!(a, Some("VO".into()));
        assert_eq!(s, Some("ST Fr".into()));
    }

    #[test]
    fn parse_tags_year_range() {
        assert_eq!(parse_movie_tags("Old (1899)").3, None);
        assert_eq!(parse_movie_tags("Movie (1900)").3, Some(1900));
        assert_eq!(parse_movie_tags("Movie (2000)").3, Some(2000));
        assert_eq!(parse_movie_tags("Movie (2099)").3, Some(2099));
        assert_eq!(parse_movie_tags("Future (2100)").3, None);
    }

    #[test]
    fn parse_tags_year_bracket() {
        assert_eq!(parse_movie_tags("Movie [2023]").3, Some(2023));
    }

    #[test]
    fn parse_tags_bare_year_at_end() {
        assert_eq!(parse_movie_tags("Blade Runner 2049").3, Some(2049));
        assert_eq!(parse_movie_tags("Movie 2024 FHD").3, None);
        assert_eq!(parse_movie_tags("20th Century Boys (MULTI) FHD 2008").3, Some(2008));
        assert_eq!(parse_movie_tags("47 Ronin (MULTI) HD 2013").3, Some(2013));
    }

    #[test]
    fn parse_tags_year_prefers_paren() {
        assert_eq!(parse_movie_tags("Movie (1999) FHD 2024").3, Some(1999));
    }

    #[test]
    fn parse_tags_vf_english() {
        let (_v, a, _s, _) = parse_movie_tags("Le Dîner de Cons VF");
        assert_eq!(a, Some("VF".into()));
        let (_, a2, _, _) = parse_movie_tags("Die Hard English");
        assert_eq!(a2, Some("EN".into()));
    }

    #[test]
    fn parse_tags_subbed() {
        let (_, _, s, _) = parse_movie_tags("Anime SUBBED");
        assert_eq!(s, Some("Sub".into()));
        let (_, _, s2, _) = parse_movie_tags("Anime SUBFRENCH");
        assert_eq!(s2, Some("Sub Fr".into()));
    }
}
