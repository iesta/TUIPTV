mod api;
mod app;
mod config;
mod db;
mod ui;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = app::App::new()?;

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(ratatui::crossterm::event::EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let ctrlc_flag = Arc::new(AtomicBool::new(false));
    let ctrlc = ctrlc_flag.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrlc.store(true, Ordering::SeqCst);
        }
    });

    let res = run(&mut terminal, &mut app, ctrlc_flag).await;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    stdout().execute(ratatui::crossterm::event::DisableBracketedPaste)?;
    terminal.show_cursor()?;
    stdout().flush()?;

    res
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut app::App,
    ctrlc_flag: Arc<AtomicBool>,
) -> Result<()> {
    loop {
        if ctrlc_flag.load(Ordering::SeqCst) {
            return Ok(());
        }

        if app.drain_logs() {
            app.load_categories();
            let n_cats = app.categories.len();
            let n_movies: usize = app
                .db
                .as_ref()
                .map(|db| {
                    db.conn
                        .lock()
                        .unwrap()
                        .query_row("SELECT COUNT(*) FROM movies", [], |r| r.get::<_, usize>(0))
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            app.status = format!("{n_cats} categories, {n_movies} movies");
            if let Some(first) = app.categories.first().cloned() {
                app.load_movies(first.id);
            }
        }
        terminal.draw(|f| ui::render::draw(f, app))?;

        let event = event::read()?;
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }
                if !app.handle_event(key.code) {
                    return Ok(());
                }
                if key.code == KeyCode::Char('r') && !app.show_config {
                    app.sync();
                }
            }
            Event::Paste(text) => {
                app.handle_paste(&text);
            }
            _ => {}
        }
    }
}
