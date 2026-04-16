use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};

use crate::app::{App, FeedbackKind, FuzzyItemKind, FuzzyTarget, InputMode};
use crate::detect::CellKind;
use crate::keybinding::Keymap;

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
            Constraint::Min(8),
            Constraint::Length(4),
            Constraint::Length(3),
        ])
        .margin(1)
        .split(size);

    render_header(frame, chunks[0], app);
    let content_chunks = if chunks[1].width >= 120 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .split(chunks[1])
    };
    render_table(frame, content_chunks[0], app);
    render_preview(frame, content_chunks[1], app);
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

    let row_label = if app.total_view_rows() == 0 {
        "row 0/0".to_string()
    } else {
        format!(
            "row {}/{}",
            app.selected_view_row + 1,
            app.total_view_rows()
        )
    };

    let selected_col_name = app
        .headers
        .get(app.selected_col)
        .map(String::as_str)
        .unwrap_or("-");

    let line = Line::from(vec![
        Span::styled(
            " guts ",
            Style::default()
                .fg(palette.background)
                .bg(palette.border)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(row_label, Style::default().fg(palette.metrics_foreground)),
        Span::raw("  "),
        Span::styled(
            format!(
                "col {}/{} {}",
                app.selected_col + 1,
                app.total_cols(),
                ellipsize(selected_col_name, 20)
            ),
            Style::default().fg(palette.column_foreground),
        ),
        Span::raw("  "),
        Span::styled(
            format!("type {}", kind_text),
            Style::default().fg(palette.type_foreground),
        ),
        Span::raw("  "),
        Span::styled(
            ellipsize(&app.source_label, 72),
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
    let approx_cell_max = ((area.width as usize).saturating_sub(8) / column_count.max(1)).max(8);

    let header = Row::new(app.headers.iter().enumerate().map(|(idx, h)| {
        let header_label = if let Some(filter) = app.column_filter_for(idx) {
            format!("{} [{}]", h, ellipsize(filter, 12))
        } else {
            h.clone()
        };
        let is_selected_col = idx == app.selected_col;
        let mut base_style = Style::default()
            .fg(palette.header_foreground)
            .add_modifier(Modifier::BOLD);

        if is_selected_col {
            base_style = base_style
                .bg(palette.selected_background)
                .fg(palette.selected_foreground);
        } else if app.column_filter_for(idx).is_some() {
            base_style = base_style
                .bg(palette.match_background)
                .fg(palette.input_prompt_foreground);
        } else if app.mode == InputMode::FuzzySearch
            && app.fuzzy_target == FuzzyTarget::Columns
            && fuzzy_match_for_column(app, idx).is_some()
        {
            base_style = base_style.bg(palette.match_background);
        }

        if let Some(fuzzy_match) = fuzzy_match_for_column(app, idx) {
            Cell::from(highlight_line(
                &header_label,
                &fuzzy_match.matched_indices,
                base_style,
                base_style
                    .fg(palette.input_prompt_foreground)
                    .add_modifier(Modifier::UNDERLINED),
            ))
        } else {
            Cell::from(header_label).style(base_style)
        }
    }))
    .style(Style::default().bg(palette.header_background));

    let (start, end) = app.visible_range(viewport_height);
    let rows = (start..end).map(|view_idx| {
        let row = app.row_at_view(view_idx);
        let is_selected_row = view_idx == app.selected_view_row;

        let mut style = Style::default().fg(palette.row_foreground);

        if app.is_search_match_view_row(view_idx) {
            style = style.bg(palette.match_background);
        }
        if app.mode == InputMode::FuzzySearch && app.fuzzy_row_rank(view_idx).is_some() {
            style = style.bg(palette.match_background);
        }
        if is_selected_row {
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
            let is_selected_col = idx == app.selected_col;
            let is_openable = app.is_cell_openable(view_idx, idx);

            if is_selected_col && !is_selected_row {
                cell_style = cell_style
                    .bg(palette.header_background)
                    .fg(palette.column_foreground);
            }

            if is_selected_col && is_selected_row {
                cell_style = cell_style.add_modifier(Modifier::UNDERLINED);
            }

            if is_openable {
                cell_style = cell_style
                    .fg(palette.input_prompt_foreground)
                    .add_modifier(Modifier::UNDERLINED);
            }

            if let Some((success, _)) = app.open_marker_for_view_cell(view_idx, idx) {
                cell_style = if success {
                    cell_style.bg(Color::DarkGray).fg(Color::LightGreen)
                } else {
                    cell_style.bg(Color::DarkGray).fg(Color::LightRed)
                };
            }

            Cell::from(ellipsize(value, approx_cell_max)).style(cell_style)
        });
        Row::new(cells)
    });

    let title = if app.filter_active {
        format!(
            " Data (filtered: {}) ",
            ellipsize(&app.filter_summary(), 42)
        )
    } else {
        " Data ".to_string()
    };

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(palette.border))
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(palette.header_foreground)
                        .add_modifier(Modifier::BOLD),
                )),
        );

    frame.render_widget(table, area);
}

fn render_preview(frame: &mut Frame, area: Rect, app: &App) {
    let palette = app.theme.palette;
    let selected_col_name = app
        .headers
        .get(app.selected_col)
        .map(String::as_str)
        .unwrap_or("-");

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Selected: ",
            Style::default()
                .fg(palette.metrics_foreground)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}", app.selected_view_row + 1),
            Style::default().fg(palette.status_text_foreground),
        ),
        Span::raw("  "),
        Span::styled(
            selected_col_name.to_string(),
            Style::default().fg(palette.column_foreground),
        ),
    ])];

    if let Some(marker) = app.open_marker_summary() {
        lines.push(Line::from(vec![
            Span::styled("Open: ", Style::default().fg(palette.metrics_foreground)),
            Span::styled(marker, Style::default().fg(palette.input_prompt_foreground)),
        ]));
    }

    if let Some(marker) = app.export_marker_summary() {
        lines.push(Line::from(vec![
            Span::styled("Export: ", Style::default().fg(palette.metrics_foreground)),
            Span::styled(marker, Style::default().fg(palette.status_mode_background)),
        ]));
    }

    lines.push(Line::from(Span::styled(
        if app.preview_expanded {
            "Preview mode: expanded"
        } else {
            "Preview mode: compact"
        },
        Style::default().fg(palette.help_foreground),
    )));

    lines.push(Line::from(""));

    if let Some(row) = app.selected_row() {
        for (idx, header) in app.headers.iter().enumerate() {
            let value = row.get(idx).map(String::as_str).unwrap_or("");
            let value = ellipsize(value, app.preview_max_chars());
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}: ", ellipsize(header, 20)),
                    Style::default()
                        .fg(if idx == app.selected_col {
                            palette.selected_foreground
                        } else {
                            palette.column_foreground
                        })
                        .add_modifier(if idx == app.selected_col {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(value, Style::default().fg(palette.row_foreground)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "No row selected",
            Style::default().fg(palette.help_foreground),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(palette.border))
                .title(Span::styled(
                    " Preview ",
                    Style::default()
                        .fg(palette.header_foreground)
                        .add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let palette = app.theme.palette;
    let mode = match app.mode {
        InputMode::Normal => "NORMAL",
        InputMode::Search => "SEARCH",
        InputMode::Query => "QUERY",
        InputMode::FuzzySearch => "FUZZY",
    };

    let search_state = if app.search_input.trim().is_empty() {
        "off".to_string()
    } else {
        format!("on({})", app.search_matches.len())
    };
    let fuzzy_state = if app.mode == InputMode::FuzzySearch {
        format!("on:{}", app.fuzzy_target.label())
    } else if app.fuzzy_input.trim().is_empty() {
        "off".to_string()
    } else {
        "ready".to_string()
    };
    let filter_state = if app.filter_active { "on" } else { "off" };
    let workflow_step = app.workflow_step();

    let line1 = Line::from(vec![
        state_chip(
            format!("MODE {mode}"),
            true,
            palette.status_mode_background,
            palette.status_mode_foreground,
        ),
        Span::raw(" "),
        state_chip(
            format!("SEARCH {search_state}"),
            !app.search_input.trim().is_empty() || app.mode == InputMode::Search,
            palette.match_background,
            palette.status_text_foreground,
        ),
        Span::raw(" "),
        state_chip(
            format!("FUZZY {fuzzy_state}"),
            app.mode == InputMode::FuzzySearch || !app.fuzzy_input.trim().is_empty(),
            palette.match_background,
            palette.status_text_foreground,
        ),
        Span::raw(" "),
        state_chip(
            format!("FILTER {filter_state}"),
            app.filter_active,
            palette.match_background,
            palette.status_text_foreground,
        ),
        Span::raw(" "),
        state_chip(
            format!("STEP {}", workflow_step.to_ascii_uppercase()),
            true,
            palette.header_background,
            palette.header_foreground,
        ),
    ]);

    let mut line2 = vec![Span::styled(
        app.status.clone(),
        Style::default().fg(palette.status_text_foreground),
    )];

    line2.push(Span::raw("  |  "));
    line2.push(Span::styled(
        format!("filters {}", ellipsize(&app.filter_summary(), 56)),
        Style::default().fg(palette.metrics_foreground),
    ));

    if let Some(open_summary) = app.open_marker_summary() {
        line2.push(Span::raw("  |  "));
        line2.push(Span::styled(
            format!("open {}", open_summary),
            Style::default().fg(palette.input_prompt_foreground),
        ));
    }

    if let Some(export_summary) = app.export_marker_summary() {
        line2.push(Span::raw("  |  "));
        line2.push(Span::styled(
            format!("export {}", export_summary),
            Style::default().fg(palette.status_mode_background),
        ));
    }

    if let Some(feedback) = &app.feedback {
        let color = feedback_color(
            feedback.kind,
            palette.type_foreground,
            palette.status_mode_background,
        );
        line2.push(Span::raw("  |  "));
        line2.push(Span::styled(
            feedback.text.clone(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(
        Paragraph::new(vec![line1, Line::from(line2)]).block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default().fg(palette.border)),
        ),
        area,
    );
}

fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    let palette = app.theme.palette;
    let k = &app.keymap;

    let nav = format!(
        "[Navigation] move {}/{}/{}/{}  top {}  bottom {}  page {}/{}",
        Keymap::labels(&k.left),
        Keymap::labels(&k.down),
        Keymap::labels(&k.up),
        Keymap::labels(&k.right),
        Keymap::labels(&k.top),
        Keymap::labels(&k.bottom),
        Keymap::labels(&k.page_up),
        Keymap::labels(&k.page_down)
    );

    let search = format!(
        "[Search] / {}  : {}  fuzzy {}  scope {}  next {} prev {}  history {}/{}",
        Keymap::labels(&k.search_mode),
        Keymap::labels(&k.query_mode),
        Keymap::labels(&k.fuzzy_mode),
        Keymap::labels(&k.fuzzy_cycle_scope),
        Keymap::labels(&k.next_match),
        Keymap::labels(&k.prev_match),
        Keymap::labels(&k.history_prev),
        Keymap::labels(&k.history_next)
    );

    let actions = format!(
        "[Actions] open {}  copy {}  export {}  preview {}  confirm {}  cancel {}  quit {}",
        Keymap::labels(&k.open),
        Keymap::labels(&k.copy),
        Keymap::labels(&k.export_csv),
        Keymap::labels(&k.toggle_preview),
        Keymap::labels(&k.confirm),
        Keymap::labels(&k.cancel),
        Keymap::labels(&k.quit)
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(nav),
            Line::from(search),
            Line::from(actions),
        ])
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(palette.help_foreground)),
        area,
    );
}

fn render_input_overlay(frame: &mut Frame, app: &App) {
    match app.mode {
        InputMode::FuzzySearch => render_fuzzy_search_overlay(frame, app),
        InputMode::Search | InputMode::Query => render_prompt_overlay(frame, app),
        InputMode::Normal => {}
    }
}

fn render_prompt_overlay(frame: &mut Frame, app: &App) {
    let palette = app.theme.palette;
    let area = bottom_overlay_rect(frame.size(), 88, 3);
    frame.render_widget(Clear, area);

    let (title, value, hint) = match app.mode {
        InputMode::Search => (
            "Search",
            app.search_input.as_str(),
            format!(
                "{} confirm  {} cancel",
                Keymap::labels(&app.keymap.confirm),
                Keymap::labels(&app.keymap.cancel)
            ),
        ),
        InputMode::Query => (
            "Query/Filter",
            app.query_input.as_str(),
            format!(
                "{} history prev  {} history next  {}  text=<term> | <column>=<term> | clear [column]",
                Keymap::labels(&app.keymap.history_prev),
                Keymap::labels(&app.keymap.history_next),
                crate::data::source_query_hint(app.source_kind)
            ),
        ),
        _ => ("", "", String::new()),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", title))
        .style(Style::default().fg(palette.border).bg(palette.background));

    let text = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(palette.input_prompt_foreground)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                value.to_string(),
                Style::default().fg(palette.input_text_foreground),
            ),
        ]),
        Line::from(Span::styled(
            hint,
            Style::default().fg(palette.help_foreground),
        )),
    ])
    .block(block);

    frame.render_widget(text, area);
}

fn render_fuzzy_search_overlay(frame: &mut Frame, app: &App) {
    let palette = app.theme.palette;
    let area = side_overlay_rect(frame.size(), 48, 72);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Fuzzy Search [{}] ", app.fuzzy_target.label()))
        .style(Style::default().fg(palette.border).bg(palette.background));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(palette.input_prompt_foreground)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.fuzzy_input.clone(),
                Style::default().fg(palette.input_text_foreground),
            ),
        ])),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                "{} switch scope  {} apply  {} close",
                Keymap::labels(&app.keymap.fuzzy_cycle_scope),
                Keymap::labels(&app.keymap.confirm),
                Keymap::labels(&app.keymap.cancel)
            ),
            Style::default().fg(palette.help_foreground),
        ))),
        chunks[1],
    );

    if app.fuzzy_matches.is_empty() {
        frame.render_widget(
            Paragraph::new("No fuzzy matches").style(Style::default().fg(palette.help_foreground)),
            chunks[2],
        );
        return;
    }

    let visible = chunks[2].height as usize;
    let total = app.fuzzy_matches.len();
    let selected = app.fuzzy_selected_idx.min(total.saturating_sub(1));
    let start = selected
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible));
    let end = (start + visible).min(total);

    let lines = (start..end)
        .map(|idx| {
            let fuzzy_match = &app.fuzzy_matches[idx];
            let item = app
                .fuzzy_items
                .get(fuzzy_match.index)
                .cloned()
                .unwrap_or_else(|| crate::app::FuzzyItem {
                    label: "<invalid>".to_string(),
                    detail: String::new(),
                    kind: FuzzyItemKind::History(String::new()),
                });
            let is_selected = idx == selected;

            let base = if is_selected {
                Style::default()
                    .fg(palette.selected_foreground)
                    .bg(palette.selected_background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.row_foreground)
            };

            let mut spans = vec![Span::styled(if is_selected { "> " } else { "  " }, base)];
            spans.extend(highlighted_spans(
                &item.label,
                &fuzzy_match.matched_indices,
                base,
                base.fg(palette.input_prompt_foreground)
                    .add_modifier(Modifier::UNDERLINED),
            ));
            spans.push(Span::styled(
                format!("  [{}] score:{}", item.detail, fuzzy_match.score),
                if is_selected {
                    base
                } else {
                    Style::default().fg(palette.metrics_foreground)
                },
            ));
            Line::from(spans)
        })
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(lines), chunks[2]);
}

fn state_chip(label: String, active: bool, bg: Color, fg: Color) -> Span<'static> {
    let style = if active {
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::DIM)
    };
    Span::styled(format!(" {} ", label), style)
}

fn feedback_color(kind: FeedbackKind, warn_color: Color, success_color: Color) -> Color {
    match kind {
        FeedbackKind::Info => Color::Cyan,
        FeedbackKind::Success => success_color,
        FeedbackKind::Warn => warn_color,
        FeedbackKind::Error => Color::Red,
    }
}

fn fuzzy_match_for_column(app: &App, col_idx: usize) -> Option<&crate::fuzzy::FuzzyMatch> {
    if app.mode != InputMode::FuzzySearch || app.fuzzy_target != FuzzyTarget::Columns {
        return None;
    }

    app.fuzzy_matches.iter().find(|fuzzy_match| {
        app.fuzzy_items
            .get(fuzzy_match.index)
            .map(|item| matches!(item.kind, FuzzyItemKind::Column(idx) if idx == col_idx))
            .unwrap_or(false)
    })
}

fn highlight_line(text: &str, matched: &[usize], base: Style, hit: Style) -> Line<'static> {
    Line::from(highlighted_spans(text, matched, base, hit))
}

fn highlighted_spans(text: &str, matched: &[usize], base: Style, hit: Style) -> Vec<Span<'static>> {
    if matched.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }

    let matched_set = matched.iter().copied().collect::<HashSet<_>>();
    text.chars()
        .enumerate()
        .map(|(idx, ch)| {
            if matched_set.contains(&idx) {
                Span::styled(ch.to_string(), hit)
            } else {
                Span::styled(ch.to_string(), base)
            }
        })
        .collect()
}

fn side_overlay_rect(rect: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let width = (rect.width.saturating_mul(width_percent) / 100)
        .max(40)
        .min(rect.width.saturating_sub(2));
    let height = (rect.height.saturating_mul(height_percent) / 100)
        .max(8)
        .min(rect.height.saturating_sub(2));
    Rect {
        x: rect.x + rect.width.saturating_sub(width + 1),
        y: rect.y + 1,
        width,
        height,
    }
}

fn bottom_overlay_rect(rect: Rect, width_percent: u16, height: u16) -> Rect {
    let width = (rect.width.saturating_mul(width_percent) / 100)
        .max(30)
        .min(rect.width.saturating_sub(2));
    Rect {
        x: rect.x + (rect.width.saturating_sub(width)) / 2,
        y: rect.y + rect.height.saturating_sub(height + 1),
        width,
        height,
    }
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
