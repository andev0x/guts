use crate::action::Action;
use crate::data::{DataSet, SourceKind};
use crate::detect::{CellKind, detect_kind};
use crate::export::{self, ExportFormat};
use crate::fuzzy::{FuzzyMatch, fuzzy_search};
use crate::history::QueryHistory;
use crate::keybinding::Keymap;
use crate::theme::ActiveTheme;
use std::cmp::{max, min};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Query,
    FuzzySearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyTarget {
    Columns,
    Rows,
    History,
}

impl FuzzyTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Columns => "Columns",
            Self::Rows => "Tables/Rows",
            Self::History => "History",
        }
    }
}

#[derive(Debug, Clone)]
pub enum FuzzyItemKind {
    Column(usize),
    Row(usize),
    History(String),
}

#[derive(Debug, Clone)]
pub struct FuzzyItem {
    pub label: String,
    pub detail: String,
    pub kind: FuzzyItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackKind {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct ActionFeedback {
    pub text: String,
    pub kind: FeedbackKind,
    ticks_left: u8,
}

impl ActionFeedback {
    fn new(text: impl Into<String>, kind: FeedbackKind) -> Self {
        Self {
            text: text.into(),
            kind,
            ticks_left: 18,
        }
    }
}

const MAX_FUZZY_ROW_ITEMS: usize = 2000;
const MAX_FUZZY_HISTORY_ITEMS: usize = 500;
const OPEN_MARKER_TICKS: u8 = 24;
const EXPORT_MARKER_TICKS: u8 = 24;

#[derive(Debug, Clone)]
pub struct ColumnFilter {
    pub col_idx: usize,
    pub col_name: String,
    pub needle: String,
}

#[derive(Debug, Clone)]
struct OpenMarker {
    data_row_idx: usize,
    col_idx: usize,
    success: bool,
    target: String,
    ticks_left: u8,
}

#[derive(Debug, Clone)]
struct ExportMarker {
    path: String,
    rows: usize,
    ticks_left: u8,
}

pub struct App {
    pub headers: Vec<String>,
    pub base_rows: Vec<Vec<String>>,
    pub view_rows: Vec<usize>,
    pub selected_view_row: usize,
    pub selected_col: usize,
    pub scroll: usize,
    pub search_input: String,
    pub search_matches: Vec<usize>,
    pub search_match_idx: usize,
    pub query_input: String,
    pub fuzzy_input: String,
    pub fuzzy_target: FuzzyTarget,
    pub fuzzy_items: Vec<FuzzyItem>,
    pub fuzzy_matches: Vec<FuzzyMatch>,
    pub fuzzy_selected_idx: usize,
    pub query_history: QueryHistory,
    pub filter_active: bool,
    pub text_filter: String,
    pub column_filters: Vec<ColumnFilter>,
    pub preview_expanded: bool,
    pub feedback: Option<ActionFeedback>,
    pub status: String,
    pub mode: InputMode,
    pub keymap: Keymap,
    pub source_label: String,
    pub source_locator: String,
    pub source_kind: SourceKind,
    pub theme: ActiveTheme,
    open_marker: Option<OpenMarker>,
    export_marker: Option<ExportMarker>,
}

impl App {
    pub fn new(dataset: DataSet, theme: ActiveTheme, keymap: Keymap, max_history: usize) -> Self {
        let view_rows = (0..dataset.rows.len()).collect::<Vec<_>>();
        let mut query_history =
            QueryHistory::load().unwrap_or_else(|_| QueryHistory::new(max_history));
        query_history.set_max_size(max_history);

        Self {
            headers: if dataset.headers.is_empty() {
                fallback_headers(&dataset.rows)
            } else {
                dataset.headers
            },
            base_rows: dataset.rows,
            view_rows,
            selected_view_row: 0,
            selected_col: 0,
            scroll: 0,
            search_input: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
            query_input: String::new(),
            fuzzy_input: String::new(),
            fuzzy_target: FuzzyTarget::Columns,
            fuzzy_items: Vec::new(),
            fuzzy_matches: Vec::new(),
            fuzzy_selected_idx: 0,
            query_history,
            filter_active: false,
            text_filter: String::new(),
            column_filters: Vec::new(),
            preview_expanded: false,
            feedback: None,
            status: theme.initial_status(),
            mode: InputMode::Normal,
            keymap,
            source_label: dataset.source,
            source_locator: dataset.source_locator,
            source_kind: dataset.kind,
            theme,
            open_marker: None,
            export_marker: None,
        }
    }

    pub fn replace_dataset(&mut self, dataset: DataSet) {
        self.headers = if dataset.headers.is_empty() {
            fallback_headers(&dataset.rows)
        } else {
            dataset.headers
        };
        self.base_rows = dataset.rows;
        self.view_rows = (0..self.base_rows.len()).collect();
        self.selected_view_row = 0;
        self.selected_col = 0;
        self.scroll = 0;
        self.source_label = dataset.source;
        self.source_locator = dataset.source_locator;
        self.source_kind = dataset.kind;
        self.filter_active = false;
        self.text_filter.clear();
        self.column_filters.clear();
        self.preview_expanded = false;
        self.open_marker = None;
        self.refresh_search_matches();
    }

    pub fn total_view_rows(&self) -> usize {
        self.view_rows.len()
    }

    pub fn total_cols(&self) -> usize {
        self.headers.len().max(1)
    }

    pub fn selected_data_row_index(&self) -> Option<usize> {
        self.view_rows.get(self.selected_view_row).copied()
    }

    pub fn selected_cell(&self) -> Option<&str> {
        let row_idx = self.selected_data_row_index()?;
        let row = self.base_rows.get(row_idx)?;
        row.get(self.selected_col).map(|s| s.as_str())
    }

    pub fn selected_cell_kind(&self) -> Option<CellKind> {
        self.selected_cell().map(detect_kind)
    }

    pub fn row_at_view(&self, view_idx: usize) -> Option<&Vec<String>> {
        let data_idx = *self.view_rows.get(view_idx)?;
        self.base_rows.get(data_idx)
    }

    pub fn selected_row(&self) -> Option<&Vec<String>> {
        self.row_at_view(self.selected_view_row)
    }

    pub fn visible_range(&self, height: usize) -> (usize, usize) {
        if height == 0 {
            return (0, 0);
        }
        let start = self.scroll;
        let end = min(self.scroll + height, self.view_rows.len());
        (start, end)
    }

    pub fn ensure_scroll_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 || self.view_rows.is_empty() {
            self.scroll = 0;
            return;
        }
        if self.selected_view_row < self.scroll {
            self.scroll = self.selected_view_row;
        } else if self.selected_view_row >= self.scroll + viewport_height {
            self.scroll = self.selected_view_row + 1 - viewport_height;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_view_row + 1 < self.view_rows.len() {
            self.selected_view_row += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_view_row > 0 {
            self.selected_view_row -= 1;
        }
    }

    pub fn move_left(&mut self) {
        if self.selected_col > 0 {
            self.selected_col -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.selected_col + 1 < self.total_cols() {
            self.selected_col += 1;
        }
    }

    pub fn page_down(&mut self, step: usize) {
        if self.view_rows.is_empty() {
            return;
        }
        self.selected_view_row = min(
            self.selected_view_row + max(step, 1),
            self.view_rows.len() - 1,
        );
    }

    pub fn page_up(&mut self, step: usize) {
        self.selected_view_row = self.selected_view_row.saturating_sub(max(step, 1));
    }

    pub fn go_top(&mut self) {
        self.selected_view_row = 0;
    }

    pub fn go_bottom(&mut self) {
        if !self.view_rows.is_empty() {
            self.selected_view_row = self.view_rows.len() - 1;
        }
    }

    pub fn set_status<S: Into<String>>(&mut self, status: S) {
        self.status = status.into();
    }

    pub fn tick_feedback(&mut self) {
        if let Some(feedback) = &mut self.feedback {
            if feedback.ticks_left > 0 {
                feedback.ticks_left -= 1;
            }
            if feedback.ticks_left == 0 {
                self.feedback = None;
            }
        }

        if let Some(marker) = &mut self.open_marker {
            if marker.ticks_left > 0 {
                marker.ticks_left -= 1;
            }
            if marker.ticks_left == 0 {
                self.open_marker = None;
            }
        }

        if let Some(marker) = &mut self.export_marker {
            if marker.ticks_left > 0 {
                marker.ticks_left -= 1;
            }
            if marker.ticks_left == 0 {
                self.export_marker = None;
            }
        }
    }

    pub fn set_feedback<S: Into<String>>(&mut self, text: S, kind: FeedbackKind) {
        self.feedback = Some(ActionFeedback::new(text, kind));
    }

    pub fn apply_filter_query(&mut self) {
        let query = self.query_input.trim().to_string();
        if query.is_empty() {
            self.clear_filters();
            self.set_status("Filters cleared");
            self.set_feedback("Text and column filters cleared", FeedbackKind::Info);
            return;
        }

        if query.eq_ignore_ascii_case("clear") || query.eq_ignore_ascii_case("clear filters") {
            self.clear_filters();
            self.set_status("Filters cleared");
            self.set_feedback("Text and column filters cleared", FeedbackKind::Info);
            return;
        }

        if let Some(rest) = query.strip_prefix("clear ") {
            let column = rest.trim();
            let before = self.column_filters.len();
            self.column_filters
                .retain(|item| !item.col_name.eq_ignore_ascii_case(column));
            self.rebuild_view_rows();
            if self.column_filters.len() < before {
                self.set_status(format!("Removed column filter: {column}"));
                self.set_feedback("Column filter removed", FeedbackKind::Info);
            } else {
                self.set_status(format!("No filter for column: {column}"));
                self.set_feedback("Column filter not found", FeedbackKind::Warn);
            }
            return;
        }

        if let Some((col_idx, col_name, needle)) = parse_column_filter(&query, &self.headers) {
            self.apply_column_filter(col_idx, col_name.clone(), needle.clone());
            self.set_status(format!(
                "Column filter {col_name} contains '{needle}' ({})",
                self.view_rows.len()
            ));
            self.set_feedback("Column filter applied", FeedbackKind::Success);
            return;
        }

        self.text_filter = query.to_ascii_lowercase();
        self.rebuild_view_rows();
        self.set_status(format!("Text filter matched {} rows", self.view_rows.len()));
        self.set_feedback(
            format!("Text filter active: '{}'", ellipsize_for_status(&query, 42)),
            FeedbackKind::Success,
        );
    }

    pub fn toggle_preview_expanded(&mut self) {
        self.preview_expanded = !self.preview_expanded;
        if self.preview_expanded {
            self.set_status("Preview expanded");
        } else {
            self.set_status("Preview collapsed");
        }
    }

    pub fn preview_max_chars(&self) -> usize {
        if self.preview_expanded { 1200 } else { 140 }
    }

    pub fn workflow_step(&self) -> &'static str {
        if self.export_marker.is_some() {
            "export"
        } else if self.open_marker.is_some() {
            "open"
        } else if self.filter_active {
            "filter"
        } else if !self.search_input.trim().is_empty() {
            "search"
        } else {
            "view"
        }
    }

    pub fn filter_summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.text_filter.is_empty() {
            parts.push(format!(
                "text:{}",
                ellipsize_for_status(&self.text_filter, 24)
            ));
        }
        for col in &self.column_filters {
            parts.push(format!(
                "{}:{}",
                col.col_name,
                ellipsize_for_status(&col.needle, 18)
            ));
        }

        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" | ")
        }
    }

    pub fn column_filter_for(&self, col_idx: usize) -> Option<&str> {
        self.column_filters
            .iter()
            .find(|item| item.col_idx == col_idx)
            .map(|item| item.needle.as_str())
    }

    pub fn open_marker_for_view_cell(
        &self,
        view_idx: usize,
        col_idx: usize,
    ) -> Option<(bool, &str)> {
        let marker = self.open_marker.as_ref()?;
        let data_idx = *self.view_rows.get(view_idx)?;
        if marker.data_row_idx == data_idx && marker.col_idx == col_idx {
            return Some((marker.success, marker.target.as_str()));
        }
        None
    }

    pub fn open_marker_summary(&self) -> Option<String> {
        self.open_marker.as_ref().map(|marker| {
            let state = if marker.success { "ok" } else { "error" };
            format!("{state}: {}", ellipsize_for_status(&marker.target, 72))
        })
    }

    pub fn export_marker_summary(&self) -> Option<String> {
        self.export_marker.as_ref().map(|marker| {
            format!(
                "{} rows -> {}",
                marker.rows,
                ellipsize_for_status(&marker.path, 72)
            )
        })
    }

    pub fn is_cell_openable(&self, view_idx: usize, col_idx: usize) -> bool {
        let Some(row) = self.row_at_view(view_idx) else {
            return false;
        };
        let Some(cell) = row.get(col_idx) else {
            return false;
        };
        matches!(detect_kind(cell), CellKind::Url | CellKind::Email)
    }

    pub fn export_current_view_csv(&mut self) {
        let file_name = format!("guts-export-{}.csv", unix_timestamp_secs());
        let path = PathBuf::from(file_name);

        let rows = self
            .view_rows
            .iter()
            .filter_map(|idx| self.base_rows.get(*idx).cloned())
            .collect::<Vec<_>>();

        let dataset = DataSet {
            headers: self.headers.clone(),
            rows,
            source: self.source_label.clone(),
            source_locator: self.source_locator.clone(),
            kind: self.source_kind,
        };

        let row_count = dataset.rows.len();
        let result = export::export_dataset(
            &dataset,
            &path,
            ExportFormat::Csv {
                delimiter: ',',
                include_headers: true,
            },
        );

        match result {
            Ok(msg) => {
                self.export_marker = Some(ExportMarker {
                    path: path.display().to_string(),
                    rows: row_count,
                    ticks_left: EXPORT_MARKER_TICKS,
                });
                self.set_status(msg);
                self.set_feedback("CSV export completed", FeedbackKind::Success);
            }
            Err(err) => {
                self.export_marker = None;
                self.set_status(format!("CSV export failed: {err}"));
                self.set_feedback("CSV export failed", FeedbackKind::Error);
            }
        }
    }

    fn clear_filters(&mut self) {
        self.text_filter.clear();
        self.column_filters.clear();
        self.rebuild_view_rows();
    }

    fn apply_column_filter(&mut self, col_idx: usize, col_name: String, needle: String) {
        let needle = needle.to_ascii_lowercase();
        if let Some(existing) = self
            .column_filters
            .iter_mut()
            .find(|item| item.col_idx == col_idx)
        {
            existing.needle = needle;
            existing.col_name = col_name;
        } else {
            self.column_filters.push(ColumnFilter {
                col_idx,
                col_name,
                needle,
            });
        }

        self.rebuild_view_rows();
    }

    fn rebuild_view_rows(&mut self) {
        self.view_rows = self
            .base_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| self.row_matches_filters(row))
            .map(|(idx, _)| idx)
            .collect();

        self.selected_view_row = 0;
        self.scroll = 0;
        self.filter_active = !self.text_filter.is_empty() || !self.column_filters.is_empty();
        self.refresh_search_matches();
    }

    fn row_matches_filters(&self, row: &[String]) -> bool {
        let text_ok = if self.text_filter.is_empty() {
            true
        } else {
            row.iter()
                .any(|cell| cell.to_ascii_lowercase().contains(&self.text_filter))
        };

        let columns_ok = self.column_filters.iter().all(|filter| {
            row.get(filter.col_idx)
                .map(|cell| cell.to_ascii_lowercase().contains(&filter.needle))
                .unwrap_or(false)
        });

        text_ok && columns_ok
    }

    pub fn refresh_search_matches(&mut self) {
        let needle = self.search_input.trim().to_ascii_lowercase();
        if needle.is_empty() {
            self.search_matches.clear();
            self.search_match_idx = 0;
            return;
        }

        self.search_matches = self
            .view_rows
            .iter()
            .enumerate()
            .filter(|(_, data_idx)| {
                self.base_rows
                    .get(**data_idx)
                    .map(|row| {
                        row.iter()
                            .any(|cell| cell.to_ascii_lowercase().contains(&needle))
                    })
                    .unwrap_or(false)
            })
            .map(|(view_idx, _)| view_idx)
            .collect();

        if self.search_matches.is_empty() {
            self.search_match_idx = 0;
            self.set_status("Search: no matches");
            self.set_feedback("Search: no matches", FeedbackKind::Warn);
        } else {
            self.search_match_idx = 0;
            self.selected_view_row = self.search_matches[0];
            self.set_status(format!(
                "Search: {}/{}",
                self.search_match_idx + 1,
                self.search_matches.len()
            ));
            self.set_feedback(
                format!("Search found {} rows", self.search_matches.len()),
                FeedbackKind::Info,
            );
        }
    }

    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            self.set_status("Search: no matches");
            self.set_feedback("Search: no matches", FeedbackKind::Warn);
            return;
        }
        self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
        self.selected_view_row = self.search_matches[self.search_match_idx];
        self.set_status(format!(
            "Search: {}/{}",
            self.search_match_idx + 1,
            self.search_matches.len()
        ));
        self.set_feedback("Search moved to next match", FeedbackKind::Info);
    }

    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            self.set_status("Search: no matches");
            self.set_feedback("Search: no matches", FeedbackKind::Warn);
            return;
        }
        if self.search_match_idx == 0 {
            self.search_match_idx = self.search_matches.len() - 1;
        } else {
            self.search_match_idx -= 1;
        }
        self.selected_view_row = self.search_matches[self.search_match_idx];
        self.set_status(format!(
            "Search: {}/{}",
            self.search_match_idx + 1,
            self.search_matches.len()
        ));
        self.set_feedback("Search moved to previous match", FeedbackKind::Info);
    }

    pub fn perform_open_action(&mut self) {
        let selected_row = self.selected_data_row_index();
        let selected_col = self.selected_col;

        match (selected_row, self.selected_cell().map(ToOwned::to_owned)) {
            (Some(data_row_idx), Some(cell)) => match Action::open(&cell) {
                Ok(()) => {
                    self.open_marker = Some(OpenMarker {
                        data_row_idx,
                        col_idx: selected_col,
                        success: true,
                        target: cell.clone(),
                        ticks_left: OPEN_MARKER_TICKS,
                    });
                    self.set_status("Opened selected value");
                    self.set_feedback(
                        format!("Opened {}", ellipsize_for_status(&cell, 72)),
                        FeedbackKind::Success,
                    );
                }
                Err(err) => {
                    self.open_marker = Some(OpenMarker {
                        data_row_idx,
                        col_idx: selected_col,
                        success: false,
                        target: cell,
                        ticks_left: OPEN_MARKER_TICKS,
                    });
                    self.set_status(format!("Open failed: {err}"));
                    self.set_feedback("Open action failed", FeedbackKind::Error);
                }
            },
            _ => {
                self.open_marker = None;
                self.set_status("No cell selected");
                self.set_feedback("Open skipped: no selected cell", FeedbackKind::Warn);
            }
        }
    }

    pub fn perform_copy_action(&mut self) {
        match self.selected_cell().map(ToOwned::to_owned) {
            Some(cell) => match Action::copy(&cell) {
                Ok(()) => {
                    self.set_status("Copied selected value");
                    self.set_feedback("Copied to clipboard", FeedbackKind::Success);
                }
                Err(err) => {
                    self.set_status(format!("Copy failed: {err}"));
                    self.set_feedback("Copy action failed", FeedbackKind::Error);
                }
            },
            None => {
                self.set_status("No cell selected");
                self.set_feedback("Copy skipped: no selected cell", FeedbackKind::Warn);
            }
        }
    }

    pub fn is_search_match_view_row(&self, view_idx: usize) -> bool {
        self.search_matches.binary_search(&view_idx).is_ok()
    }

    pub fn refresh_fuzzy_matches(&mut self) {
        self.fuzzy_items = self.build_fuzzy_items();
        let labels = self
            .fuzzy_items
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();
        self.fuzzy_matches = fuzzy_search(&labels, &self.fuzzy_input);

        if self.fuzzy_matches.is_empty() {
            self.fuzzy_selected_idx = 0;
            return;
        }

        if self.fuzzy_selected_idx >= self.fuzzy_matches.len() {
            self.fuzzy_selected_idx = self.fuzzy_matches.len() - 1;
        }
        self.sync_fuzzy_row_preview();
    }

    pub fn cycle_fuzzy_target(&mut self) {
        self.fuzzy_target = match self.fuzzy_target {
            FuzzyTarget::Columns => FuzzyTarget::Rows,
            FuzzyTarget::Rows => FuzzyTarget::History,
            FuzzyTarget::History => FuzzyTarget::Columns,
        };
        self.fuzzy_selected_idx = 0;
        self.refresh_fuzzy_matches();
        self.set_status(format!("Fuzzy scope: {}", self.fuzzy_target.label()));
    }

    fn build_fuzzy_items(&self) -> Vec<FuzzyItem> {
        match self.fuzzy_target {
            FuzzyTarget::Columns => self
                .headers
                .iter()
                .enumerate()
                .map(|(idx, header)| FuzzyItem {
                    label: header.clone(),
                    detail: format!("col {}", idx + 1),
                    kind: FuzzyItemKind::Column(idx),
                })
                .collect(),
            FuzzyTarget::Rows => {
                let label_prefix = if matches!(
                    self.source_kind,
                    SourceKind::Sqlite
                        | SourceKind::Postgres
                        | SourceKind::MySql
                        | SourceKind::Mongo
                ) {
                    "table/row"
                } else {
                    "row"
                };

                (0..self.view_rows.len().min(MAX_FUZZY_ROW_ITEMS))
                    .filter_map(|view_idx| {
                        self.row_at_view(view_idx).map(|row| FuzzyItem {
                            label: row_fuzzy_label(row),
                            detail: format!("{} {}", label_prefix, view_idx + 1),
                            kind: FuzzyItemKind::Row(view_idx),
                        })
                    })
                    .collect()
            }
            FuzzyTarget::History => self
                .query_history
                .recent_queries_for_source(self.source_kind, MAX_FUZZY_HISTORY_ITEMS)
                .into_iter()
                .map(|query| FuzzyItem {
                    label: query.clone(),
                    detail: "history".to_string(),
                    kind: FuzzyItemKind::History(query),
                })
                .collect(),
        }
    }

    pub fn fuzzy_move_down(&mut self) {
        if self.fuzzy_selected_idx + 1 < self.fuzzy_matches.len() {
            self.fuzzy_selected_idx += 1;
            self.sync_fuzzy_row_preview();
        }
    }

    pub fn fuzzy_move_up(&mut self) {
        if self.fuzzy_selected_idx > 0 {
            self.fuzzy_selected_idx -= 1;
            self.sync_fuzzy_row_preview();
        }
    }

    pub fn fuzzy_select(&mut self) -> InputMode {
        let Some(fuzzy_match) = self.fuzzy_matches.get(self.fuzzy_selected_idx) else {
            self.set_status("No fuzzy match selected");
            self.set_feedback("No fuzzy match selected", FeedbackKind::Warn);
            return InputMode::Normal;
        };

        let Some(item) = self.fuzzy_items.get(fuzzy_match.index).cloned() else {
            self.set_status("Fuzzy selection out of range");
            self.set_feedback("Invalid fuzzy selection", FeedbackKind::Error);
            return InputMode::Normal;
        };

        match item.kind {
            FuzzyItemKind::Column(col_idx) => {
                self.selected_col = col_idx;
                let label = self.headers.get(col_idx).cloned().unwrap_or_default();
                self.set_status(format!("Selected column: {label}"));
                self.set_feedback("Column selection applied", FeedbackKind::Success);
                InputMode::Normal
            }
            FuzzyItemKind::Row(view_idx) => {
                self.selected_view_row = view_idx;
                self.set_status(format!("Focused row {}", view_idx + 1));
                self.set_feedback("Row selection applied", FeedbackKind::Success);
                InputMode::Normal
            }
            FuzzyItemKind::History(query) => {
                self.query_input = query;
                self.query_history.reset_navigation();
                self.set_status("Loaded query from history");
                self.set_feedback("History query loaded", FeedbackKind::Success);
                InputMode::Query
            }
        }
    }

    pub fn fuzzy_row_rank(&self, view_idx: usize) -> Option<usize> {
        if self.fuzzy_target != FuzzyTarget::Rows {
            return None;
        }

        self.fuzzy_matches
            .iter()
            .enumerate()
            .find_map(|(rank, fuzzy_match)| {
                self.fuzzy_items
                    .get(fuzzy_match.index)
                    .and_then(|item| match item.kind {
                        FuzzyItemKind::Row(idx) if idx == view_idx => Some(rank),
                        _ => None,
                    })
            })
    }

    fn sync_fuzzy_row_preview(&mut self) {
        if self.fuzzy_target != FuzzyTarget::Rows {
            return;
        }

        let selected_view_idx = self
            .fuzzy_matches
            .get(self.fuzzy_selected_idx)
            .and_then(|fuzzy_match| self.fuzzy_items.get(fuzzy_match.index))
            .and_then(|item| match item.kind {
                FuzzyItemKind::Row(view_idx) => Some(view_idx),
                _ => None,
            });

        if let Some(view_idx) = selected_view_idx {
            self.selected_view_row = view_idx;
        }
    }

    pub fn history_prev(&mut self) {
        if let Some(query) = self.query_history.get_prev_for_source(self.source_kind) {
            self.query_input = query.to_string();
        } else if !self.query_history.is_empty() {
            self.set_status("At oldest history entry");
        }
    }

    pub fn history_next(&mut self) {
        if let Some(query) = self.query_history.get_next_for_source(self.source_kind) {
            self.query_input = query.to_string();
        } else {
            self.query_input.clear();
        }
    }

    pub fn add_to_history(&mut self, query: String, success: bool) {
        use crate::history::QueryEntry;
        self.query_history
            .add(QueryEntry::new(query, self.source_kind, success));
        let _ = self.query_history.save();
    }
}

fn row_fuzzy_label(row: &[String]) -> String {
    if row.is_empty() {
        return String::new();
    }

    let mut out = row
        .iter()
        .take(4)
        .map(|cell| cell.trim())
        .filter(|cell| !cell.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");

    if out.chars().count() > 180 {
        out = out.chars().take(179).collect::<String>();
        out.push('~');
    }

    out
}

fn fallback_headers(rows: &[Vec<String>]) -> Vec<String> {
    let cols = rows.first().map(|r| r.len()).unwrap_or(1);
    (1..=cols).map(|i| format!("col_{i}")).collect()
}

fn parse_column_filter(query: &str, headers: &[String]) -> Option<(usize, String, String)> {
    let (lhs, rhs) = query.split_once('=')?;
    let column = lhs.trim();
    let needle = rhs.trim();
    if column.is_empty() || needle.is_empty() {
        return None;
    }

    let (idx, name) = headers
        .iter()
        .enumerate()
        .find(|(_, header)| header.eq_ignore_ascii_case(column))
        .map(|(idx, header)| (idx, header.clone()))?;

    Some((idx, name, needle.to_string()))
}

fn ellipsize_for_status(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let mut out = String::with_capacity(max_len + 1);
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_len.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('~');
    out
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
