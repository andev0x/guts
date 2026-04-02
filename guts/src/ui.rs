use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};

use crate::app::{App, InputMode};
use crate::detect::CellKind;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let palette = app.theme.palette;
    let size = frame.size();
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.background)),
        size,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(size);

    render_header(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
    render_status(frame, chunks[2], app);
    render_help(frame, chunks[3], app);

    if app.mode != InputMode::Normal {
        render_input_overlay(frame, app);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let palette = app.theme.palette;
    let selected_kind = app.selected_cell_kind().unwrap_or(CellKind::Text);
    let kind_text = match selected_kind {
        CellKind::Url => "URL",
        CellKind::Email => "EMAIL",
        CellKind::Number => "NUMBER",
        CellKind::Ip => "IP",
        CellKind::Text => "TEXT",
    };

    let line = Line::from(vec![
        Span::styled(
            " guts ",
            Style::default()
                .fg(palette.background)
                .bg(palette.border)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} rows", app.total_view_rows()),
            Style::default().fg(palette.metrics_foreground),
        ),
        Span::raw("  "),
        Span::styled(
            format!("col {}", app.selected_col + 1),
            Style::default().fg(palette.column_foreground),
        ),
        Span::raw("  "),
        Span::styled(
            format!("type {}", kind_text),
            Style::default().fg(palette.type_foreground),
        ),
        Span::raw("  "),
        Span::styled(
            app.source_label.clone(),
            Style::default().fg(palette.source_foreground),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let palette = app.theme.palette;
    let viewport_height = area.height.saturating_sub(3) as usize;
    app.ensure_scroll_visible(viewport_height);

    let column_count = app.total_cols();
    let widths = column_widths(column_count);

    let header = Row::new(app.headers.iter().map(|h| {
        Cell::from(h.as_str()).style(
            Style::default()
                .fg(palette.header_foreground)
                .add_modifier(Modifier::BOLD),
        )
    }))
    .style(Style::default().bg(palette.header_background));

    let (start, end) = app.visible_range(viewport_height);
    let rows = (start..end).map(|view_idx| {
        let row = app.row_at_view(view_idx);
        let mut style = Style::default().fg(palette.row_foreground);

        if app.is_search_match_view_row(view_idx) {
            style = style.bg(palette.match_background);
        }
        if view_idx == app.selected_view_row {
            style = Style::default()
                .fg(palette.selected_foreground)
                .bg(palette.selected_background)
                .add_modifier(Modifier::BOLD);
        }

        let cells = (0..column_count).map(|idx| {
            let value = row
                .and_then(|r| r.get(idx))
                .map(String::as_str)
                .unwrap_or("");
            let mut cell_style = style;
            if idx == app.selected_col && view_idx == app.selected_view_row {
                cell_style = cell_style.add_modifier(Modifier::UNDERLINED);
            }
            Cell::from(ellipsize(value, 120)).style(cell_style)
        });
        Row::new(cells)
    });

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(palette.border))
                .title(Span::styled(
                    " Data ",
                    Style::default()
                        .fg(palette.header_foreground)
                        .add_modifier(Modifier::BOLD),
                )),
        );

    frame.render_widget(table, area);
}

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let palette = app.theme.palette;
    let mode = match app.mode {
        InputMode::Normal => "NORMAL",
        InputMode::Search => "SEARCH",
        InputMode::Query => "QUERY",
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", mode),
            Style::default()
                .fg(palette.status_mode_foreground)
                .bg(palette.status_mode_background),
        ),
        Span::raw("  "),
        Span::styled(
            app.status.clone(),
            Style::default().fg(palette.status_text_foreground),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default().fg(palette.border)),
        ),
        area,
    );
}

fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    let palette = app.theme.palette;
    let query_hint = crate::data::source_query_hint(app.source_kind);
    let help = format!(
        "q quit  h/j/k/l move  g/G top/bottom  PgUp/PgDn page  / search  n/N next/prev  {}  o open  y copy",
        query_hint
    );
    frame.render_widget(
        Paragraph::new(help)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(palette.help_foreground)),
        area,
    );
}

fn render_input_overlay(frame: &mut Frame, app: &App) {
    let palette = app.theme.palette;
    let area = centered_rect(70, 18, frame.size());
    frame.render_widget(Clear, area);

    let (title, value) = match app.mode {
        InputMode::Search => ("Search", app.search_input.as_str()),
        InputMode::Query => ("Query", app.query_input.as_str()),
        InputMode::Normal => ("", ""),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", title))
        .style(Style::default().fg(palette.border).bg(palette.background));

    let text = Paragraph::new(Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(palette.input_prompt_foreground)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(value),
    ]))
    .block(block)
    .style(Style::default().fg(palette.input_text_foreground));

    frame.render_widget(text, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, rect: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(rect);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn column_widths(column_count: usize) -> Vec<Constraint> {
    if column_count == 0 {
        return vec![Constraint::Percentage(100)];
    }
    let base = (100 / column_count) as u16;
    (0..column_count)
        .map(|idx| {
            if idx == column_count - 1 {
                let used = base.saturating_mul((column_count.saturating_sub(1)) as u16);
                Constraint::Percentage(100u16.saturating_sub(used).max(1))
            } else {
                Constraint::Percentage(base.max(1))
            }
        })
        .collect()
}

fn ellipsize(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    let mut out = String::with_capacity(max_len + 1);
    for (i, ch) in value.chars().enumerate() {
        if i >= max_len.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('~');
    out
}
