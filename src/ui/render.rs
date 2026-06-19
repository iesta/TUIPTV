use crate::app::{App, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;
use ratatui_image::{Resize, StatefulImage};

fn top_bar(frame: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" tuiptv v0.2.0 ")
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
    if focussed && app.movie_offset < movies.len() {
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

fn movies_table_pane(frame: &mut Frame, area: Rect, app: &App) {
    let movies = app.active_movies();
    let total = app.movies.len();
    let shown = movies.len();
    let focussed = app.focus == Focus::Movies;
    let title = if app.show_filter {
        format!(" Movies ({shown}/{total}) (table) ")
    } else {
        format!(" Movies ({total}) (table) ")
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
    frame.render_widget(block, area);

    let visible_rows = inner.height as usize;
    let selected = if app.movie_offset < movies.len() {
        app.movie_offset
    } else {
        0
    };
    let offset = if movies.len() <= visible_rows {
        0
    } else {
        let half = visible_rows / 2;
        if selected < half {
            0
        } else if selected >= movies.len().saturating_sub(half) {
            movies.len().saturating_sub(visible_rows)
        } else {
            selected.saturating_sub(half)
        }
    };

    let rows: Vec<Row> = movies
        .iter()
        .skip(offset)
        .take(visible_rows)
        .map(|m| {
            let wishlist = app.wishlist.contains(&m.id);
            let name_cell = if wishlist {
                format!("★ {}", m.name)
            } else {
                m.name.clone()
            };
            let year_cell = m
                .year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "—".to_string());
            let rating_cell = m
                .rating
                .clone()
                .unwrap_or_else(|| "—".to_string());
            let genre_cell = m
                .genre
                .clone()
                .unwrap_or_else(|| "—".to_string());
            Row::new(vec![
                Cell::from(name_cell),
                Cell::from(year_cell),
                Cell::from(rating_cell),
                Cell::from(genre_cell),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().fg(Color::Cyan)),
        Cell::from("Year").style(Style::default().fg(Color::Cyan)),
        Cell::from("Rating").style(Style::default().fg(Color::Cyan)),
        Cell::from("Genre").style(Style::default().fg(Color::Cyan)),
    ])
    .height(1);

    let widths = [
        Constraint::Min(20),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(15),
    ];

    let mut state = TableState::default();
    state.select(Some(selected.saturating_sub(offset)));

    let highlight_style = if focussed {
        Style::default().bg(Color::White).fg(Color::Black)
    } else {
        Style::default().fg(Color::LightBlue)
    };

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(highlight_style)
        .highlight_symbol("> ");
    frame.render_stateful_widget(table, inner, &mut state);

    if app.show_filter {
        let bar = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let text = format!("/ {}", app.filter_query);
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::Yellow)),
            bar,
        );
    }
}

fn details_pane(frame: &mut Frame, area: Rect, app: &App) {
    if app.focus != Focus::Movies {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Details ");
        frame.render_widget(block, area);
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Details ")
        .border_style(Style::default().fg(Color::Yellow));

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
            "Title: {}\nRating: {}\nYear: {}\nGenre: {}\nCast: {}\nDirector: {}\nWishlist: {}\n\n{}\n\n[{}]",
            m.name,
            m.rating.as_deref().unwrap_or("N/A"),
            m.year.map(|y| y.to_string()).as_deref().unwrap_or("N/A"),
            m.genre.as_deref().unwrap_or("N/A"),
            m.cast.as_deref().unwrap_or("N/A"),
            m.director.as_deref().unwrap_or("N/A"),
            if app.wishlist.contains(&m.id) { "❤️" } else { "—" },
            m.plot.as_deref().unwrap_or("No plot available."),
            app.poster_debug,
        );
        frame.render_widget(Paragraph::new(text), text_area);
    } else {
        frame.render_widget(Paragraph::new("Select a movie to see details."), inner);
    }
}

fn movie_popover(frame: &mut Frame, area: Rect, app: &App) {
    let m = match app.movies.get(app.movie_offset) {
        Some(m) => m,
        None => return,
    };
    let pop_w = ((area.width as i32) * 80 / 100).max(50) as u16;
    let pop_h = ((area.height as i32) * 80 / 100).max(20) as u16;
    let pop_x = (area.width.saturating_sub(pop_w)) / 2;
    let pop_y = (area.height.saturating_sub(pop_h)) / 2;
    let pop = Rect {
        x: pop_x,
        y: pop_y,
        width: pop_w,
        height: pop_h,
    };
    frame.render_widget(Clear, pop);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", m.name))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(pop);
    frame.render_widget(block, pop);

    if inner.width < 20 || inner.height < 8 {
        return;
    }

    let [poster_area, text_area] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
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
    } else {
        frame.render_widget(Clear, poster_area);
    }

    let mut lines: Vec<Line> = vec![
        Line::from(format!(
            "Rating: {}  Year: {}  Length: {}  Released: {}",
            m.rating.as_deref().unwrap_or("N/A"),
            m.year.map(|y| y.to_string()).as_deref().unwrap_or("N/A"),
            m.container_extension.as_deref().unwrap_or("N/A"),
            m.release_date.as_deref().unwrap_or("N/A"),
        )),
        Line::from(format!("Genre: {}", m.genre.as_deref().unwrap_or("N/A"))),
        Line::from(format!("Version: {}", m.version.as_deref().unwrap_or("N/A"))),
        Line::from(format!("Audio: {}", m.audio.as_deref().unwrap_or("N/A"))),
        Line::from(format!("Subs: {}", m.subs.as_deref().unwrap_or("N/A"))),
        Line::from(format!(
            "Cast: {}",
            m.cast.as_deref().unwrap_or("N/A")
        )),
        Line::from(format!(
            "Director: {}",
            m.director.as_deref().unwrap_or("N/A")
        )),
        Line::from(format!(
            "Wishlist: {}",
            if app.wishlist.contains(&m.id) { "❤️" } else { "—" }
        )),
        Line::from(""),
        Line::from(m.plot.as_deref().unwrap_or("No plot available.")),
    ];
    lines.push(Line::from(""));
    lines.push(
        Line::from("v: open in VLC (or other)  Esc/Enter/q: close")
            .style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        text_area,
    );
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
h         toggle statistics
Left      focus pane left
l / Right focus pane right
Tab       focus next pane
BackTab   focus previous pane
Enter     select category
q         quit
Ctrl+R    sync data from server
i         toggle poster images
r         sort movies by rating (toggle)
a         sort movies by name (toggle)
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

fn stats_screen(frame: &mut Frame, area: Rect, app: &App) {
    let total_movies: usize = app.stats_total_movies;
    let total_wish = app.wishlist.len();
    let total_cat = app.categories.len();
    let year_range = if app.stats_years.is_empty() {
        "N/A".to_string()
    } else {
        format!(
            "{} – {}",
            app.stats_years.last().map(|(y, _)| y).unwrap(),
            app.stats_years.first().map(|(y, _)| y).unwrap()
        )
    };
    let mut year_lines = String::new();
    for (year, count) in app.stats_years.iter().take(10) {
        year_lines.push_str(&format!("  {year}: {count}\n"));
    }

    let text = format!(
        "\
Categories: {total_cat}
Movies:     {total_movies}
Wishlist:   ❤️ {total_wish}
Year range: {year_range}

Years:
{year_lines}
Esc / h  close"
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Statistics ")
        .style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(text).block(block);
    let [stats_area] = *Layout::vertical([Constraint::Min(0)])
        .margin(4)
        .split(area)
    else {
        return;
    };
    frame.render_widget(Clear, stats_area);
    frame.render_widget(paragraph, stats_area);
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

    if app.table_mode {
        let [cat, mov] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(80),
            ])
            .split(main)
        else {
            return;
        };
        categories_pane(frame, cat, app);
        movies_table_pane(frame, mov, app);
        log_pane(frame, logs, app);
    } else {
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
    }

    if app.show_help {
        help_screen(frame, frame.area());
    }
    if app.show_stats {
        stats_screen(frame, frame.area(), app);
    }
    if app.show_movie_popover {
        movie_popover(frame, frame.area(), app);
    }
}
