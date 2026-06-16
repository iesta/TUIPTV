mod api;
mod app;
mod config;
mod db;
mod ui;

use anyhow::Result;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let font_size = match crossterm::terminal::window_size() {
        Ok(ws) if ws.width > 0 && ws.columns > 0 && ws.height > 0 && ws.rows > 0 => {
            let fw = (ws.width / ws.columns).max(4);
            let fh = (ws.height / ws.rows).max(4);
            (fw, fh)
        }
        _ => (8, 16),
    };

    let mut app = app::App::new()?;
    app.font_size = font_size;
    app.picker = ratatui_image::picker::Picker::new(font_size);

    let (w, h) = size()?;
    app.term_width = w;
    app.term_height = h;

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(ratatui::crossterm::event::EnableBracketedPaste)?;
    stdout().execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    app.picker.guess_protocol();

    let ctrlc_flag = Arc::new(AtomicBool::new(false));
    let ctrlc = ctrlc_flag.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrlc.store(true, Ordering::SeqCst);
        }
    });

    let res = run(&mut terminal, &mut app, ctrlc_flag).await;
    app.save_layout();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    stdout().execute(ratatui::crossterm::event::DisableBracketedPaste)?;
    stdout().execute(DisableMouseCapture)?;
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

        app.drain_posters();
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

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;
            match ev {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }
                    if key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        app.show_logs = !app.show_logs;
                        continue;
                    }
                    if key.code == KeyCode::Char('r')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && !app.show_config
                    {
                        app.sync();
                        continue;
                    }
                    if !app.handle_event(key.code) {
                        return Ok(());
                    }
                }
                Event::Mouse(m) => {
                    app.handle_mouse(m);
                }
                Event::Resize(w, h) => {
                    app.term_width = w;
                    app.term_height = h;
                }
                Event::Paste(text) => {
                    app.handle_paste(&text);
                }
                _ => {}
            }
        }
    }
}
