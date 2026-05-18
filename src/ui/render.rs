use crate::app::{App, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

fn top_bar(frame: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" tuiptv v0.1.0 ")
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);
}

fn categories_pane(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.focus == Focus::Categories {
        " Categories (focused) "
    } else {
        " Categories "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if app.focus == Focus::Categories {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let items: Vec<ListItem> = app
        .categories
        .iter()
        .map(|c| ListItem::new(c.name.clone()))
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    let mut state = ratatui::widgets::ListState::default();
    if app.category_offset < app.categories.len() {
        state.select(Some(app.category_offset));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn movies_pane(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.focus == Focus::Movies {
        " Movies (focused) "
    } else {
        " Movies "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if app.focus == Focus::Movies {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let items: Vec<ListItem> = app
        .movies
        .iter()
        .map(|m| ListItem::new(m.name.clone()))
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    let mut state = ratatui::widgets::ListState::default();
    if app.movie_offset < app.movies.len() {
        state.select(Some(app.movie_offset));
    }
    frame.render_stateful_widget(list, area, &mut state);
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

    let text = if let Some(m) = app.movies.get(app.movie_offset) {
        format!(
            "Title: {}\nRating: {}\nReleased: {}\nExtension: {}\n\n{}",
            m.name,
            m.rating.as_deref().unwrap_or("N/A"),
            m.release_date.as_deref().unwrap_or("N/A"),
            m.container_extension.as_deref().unwrap_or("N/A"),
            m.plot.as_deref().unwrap_or("No plot available."),
        )
    } else {
        "Select a movie to see details.".into()
    };
    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn bottom_bar(frame: &mut Frame, area: Rect, _app: &App) {
    let text = Line::from(vec![
        Span::styled(
            " q:Quit ",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(
            " r:Sync ",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(
            " h/j/k/l:Navigate ",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
    ]);
    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
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

    let [top, main, logs, bottom] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(frame.area())
    else {
        return;
    };

    top_bar(frame, top, app);

    let [cat, mov, det] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(50),
            Constraint::Percentage(40),
        ])
        .split(main)
    else {
        return;
    };

    categories_pane(frame, cat, app);
    movies_pane(frame, mov, app);
    details_pane(frame, det, app);
    log_pane(frame, logs, app);
    bottom_bar(frame, bottom, app);
}
