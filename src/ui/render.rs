use crate::app::{App, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use ratatui_image::{Resize, StatefulImage};

fn top_bar(frame: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" tuiptv v0.1.0 ")
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);
}

fn categories_pane(frame: &mut Frame, area: Rect, app: &App) {
    let total = app.categories.len();
    let focussed = app.focus == Focus::Categories;
    let title = if focussed {
        format!(" Categories ({total}) (focused) ")
    } else {
        format!(" Categories ({total}) ")
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focussed {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let inner = block.inner(area);
    let items: Vec<ListItem> = app
        .categories
        .iter()
        .map(|c| ListItem::new(c.name.clone()))
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(if focussed {
            Style::default().bg(Color::White).fg(Color::Black)
        } else {
            Style::default().fg(Color::LightBlue)
        });
    let mut state = ratatui::widgets::ListState::default();
    if app.category_offset < app.categories.len() {
        state.select(Some(app.category_offset));
    }
    let visible = inner.height as usize;
    if visible > 0 {
        let half = visible / 2;
        let offset = app
            .category_offset
            .saturating_sub(half)
            .min(app.categories.len().saturating_sub(visible));
        *state.offset_mut() = offset;
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn movies_pane(frame: &mut Frame, area: Rect, app: &App) {
    let movies = app.active_movies();
    let total = app.movies.len();
    let shown = movies.len();
    let focussed = app.focus == Focus::Movies;
    let title = if app.show_filter {
        format!(" Movies ({shown}/{total}) ")
    } else {
        format!(" Movies ({total}) ")
    };
    let title = if focussed {
        format!("{title}(focused) ")
    } else {
        title
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focussed {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let inner = block.inner(area);
    let items: Vec<ListItem> = movies
        .iter()
        .map(|m| {
            let prefix = if app.wishlist.contains(&m.id) {
                "★ "
            } else {
                "  "
            };
            let label = format!("{}{}", prefix, m.name);
            ListItem::new(label)
        })
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(if focussed {
            Style::default().bg(Color::White).fg(Color::Black)
        } else {
            Style::default().fg(Color::LightBlue)
        });
    let mut state = ratatui::widgets::ListState::default();
    if app.movie_offset < movies.len() {
        state.select(Some(app.movie_offset));
    }
    let visible = inner.height as usize;
    if visible > 0 {
        let half = visible / 2;
        let offset = app
            .movie_offset
            .saturating_sub(half)
            .min(movies.len().saturating_sub(visible));
        *state.offset_mut() = offset;
    }
    frame.render_stateful_widget(list, area, &mut state);

    if app.show_filter {
        let bar_h = 1u16;
        if inner.height > bar_h {
            let bar_area = Rect {
                x: inner.x,
                y: inner.y + inner.height - bar_h,
                width: inner.width,
                height: bar_h,
            };
            frame.render_widget(Clear, bar_area);
            let display = if app.filter_query.is_empty() {
                "/".to_string()
            } else {
                format!("/{}/", app.filter_query)
            };
            frame.render_widget(
                Paragraph::new(display).style(Style::default().fg(Color::Cyan)),
                bar_area,
            );
        }
    }
}

fn details_pane(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.focus == Focus::Details {
        " Details (focused) "
    } else {
        " Details "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if app.focus == Focus::Details {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    frame.render_widget(&block, area);
    let inner = block.inner(area);

    if let Some(m) = app.movies.get(app.movie_offset) {
        let [poster_area, text_area] =
            *Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(inner)
        else {
            return;
        };

        if app.show_posters {
            if let Some(ref mut protocol) = *app.poster_protocol.borrow_mut() {
                let stateful_image = StatefulImage::new(None).resize(Resize::Fit(None));
                frame.render_stateful_widget(stateful_image, poster_area, protocol);
            } else {
                frame.render_widget(Clear, poster_area);
                if app.poster_loading {
                    frame.render_widget(
                        Paragraph::new("Loading poster...").style(Style::default().fg(Color::Gray)),
                        poster_area,
                    );
                }
            }
        }

        let text = format!(
            "Title: {}\nRating: {}\nYear: {}\nReleased: {}\n{}Genre: {}\nVersion: {}\nAudio: {}\nSubs: {}\nWishlist: {}\n\n{}\n\n[{}]",
            m.name,
            m.rating.as_deref().unwrap_or("N/A"),
            m.year.map(|y| y.to_string()).as_deref().unwrap_or("N/A"),
            m.release_date.as_deref().unwrap_or("N/A"),
            m.container_extension.as_ref().map(|e| format!("Extension: {e}\n")).unwrap_or_default(),
            m.genre.as_deref().unwrap_or("N/A"),
            m.version.as_deref().unwrap_or("N/A"),
            m.audio.as_deref().unwrap_or("N/A"),
            m.subs.as_deref().unwrap_or("N/A"),
            if app.wishlist.contains(&m.id) { "❤️" } else { "—" },
            m.plot.as_deref().unwrap_or("No plot available."),
            app.poster_debug,
        );
        frame.render_widget(Paragraph::new(text), text_area);
    } else {
        frame.render_widget(Paragraph::new("Select a movie to see details."), inner);
    }
}

fn config_screen(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Xtream Configuration ")
        .style(Style::default().fg(Color::Cyan));
    let fields =
        format!(
        "URL:      {}{}\nUsername:  {}{}\nPassword:  {}{}\n\nTab: next field  Enter: save  q: quit",
        app.config_fields.url,
        if app.config_fields.focus == 0 { "_" } else { "" },
        app.config_fields.username,
        if app.config_fields.focus == 1 { "_" } else { "" },
        "*".repeat(app.config_fields.password.len()),
        if app.config_fields.focus == 2 { "_" } else { "" },
    );
    let paragraph = Paragraph::new(fields).block(block);
    frame.render_widget(paragraph, area);
}

fn help_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .style(Style::default().fg(Color::Cyan));
    let text = "\
j / Down  scroll down
k / Up    scroll up
gg        go to beginning of list
G         go to end of list
h / Left  focus pane left
l / Right focus pane right
Tab       focus next pane
BackTab   focus previous pane
Enter     select category
q         quit
Ctrl+R    sync data from server
i         toggle poster images
r         sort movies by rating (toggle)
y         sort movies by year (toggle)
o         reset to default order
c         cycle category order
w         jump to wishlist
L         reload poster from cache
v         open in VLC
f         full-size poster
Space     toggle wishlist
Ctrl+D    toggle log window
( / )     shrink / grow column
?         toggle this help
Mouse     click: focus & toggle poster, drag gutters resize";
    let paragraph = Paragraph::new(text).block(block);
    let [help_area] = *Layout::vertical([Constraint::Min(0)]).margin(4).split(area) else {
        return;
    };
    frame.render_widget(Clear, help_area);
    frame.render_widget(paragraph, help_area);
}

fn log_pane(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Log ");
    let n = (area.height as usize).saturating_sub(2).min(app.logs.len());
    let lines: Vec<Line> = app.logs[app.logs.len().saturating_sub(n)..]
        .iter()
        .map(|s| {
            if s.len() > area.width.saturating_sub(2) as usize {
                Line::from(Span::raw(format!(
                    "{}…",
                    &s[..area.width.saturating_sub(3) as usize]
                )))
            } else {
                Line::from(Span::raw(s.clone()))
            }
        })
        .collect();
    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

pub fn draw(frame: &mut Frame, app: &App) {
    if app.show_config {
        config_screen(frame, frame.area(), app);
        return;
    }

    if app.show_full_poster {
        if let Some(ref mut protocol) = *app.poster_protocol.borrow_mut() {
            let stateful_image = StatefulImage::new(None).resize(Resize::Fit(None));
            let a = Rect {
                x: 1,
                y: 1,
                width: frame.area().width.saturating_sub(2),
                height: frame.area().height.saturating_sub(2),
            };
            frame.render_widget(Clear, a);
            frame.render_stateful_widget(stateful_image, a, protocol);
        }
        return;
    }

    let log_h = if app.show_logs { 3 } else { 0 };
    let [top, main, logs] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(log_h),
        ])
        .split(frame.area())
    else {
        return;
    };

    top_bar(frame, top, app);

    let [cat, mov, det] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(app.column_ratios[0]),
            Constraint::Percentage(app.column_ratios[1]),
            Constraint::Percentage(app.column_ratios[2]),
        ])
        .split(main)
    else {
        return;
    };

    categories_pane(frame, cat, app);
    movies_pane(frame, mov, app);
    details_pane(frame, det, app);
    log_pane(frame, logs, app);

    if app.show_help {
        help_screen(frame, frame.area());
    }
}
