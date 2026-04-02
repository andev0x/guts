use crate::action::Action;
use crate::data::{DataSet, SourceKind};
use crate::detect::{CellKind, detect_kind};
use crate::fuzzy::{FuzzyMatch, fuzzy_search};
use crate::history::QueryHistory;
use crate::keybinding::Keymap;
use crate::theme::ActiveTheme;
use std::cmp::{max, min};

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
    pub feedback: Option<ActionFeedback>,
    pub status: String,
    pub mode: InputMode,
    pub keymap: Keymap,
    pub source_label: String,
    pub source_locator: String,
    pub source_kind: SourceKind,
    pub theme: ActiveTheme,
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
            feedback: None,
            status: theme.initial_status(),
            mode: InputMode::Normal,
            keymap,
            source_label: dataset.source,
            source_locator: dataset.source_locator,
            source_kind: dataset.kind,
            theme,
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
    }

    pub fn set_feedback<S: Into<String>>(&mut self, text: S, kind: FeedbackKind) {
        self.feedback = Some(ActionFeedback::new(text, kind));
    }

    pub fn apply_filter_query(&mut self) {
        let q = self.query_input.trim().to_ascii_lowercase();
        if q.is_empty() {
            self.view_rows = (0..self.base_rows.len()).collect();
            self.selected_view_row = 0;
            self.scroll = 0;
            self.filter_active = false;
            self.set_status("Query cleared");
            self.set_feedback("Filter cleared", FeedbackKind::Info);
            self.refresh_search_matches();
            return;
        }

        self.view_rows = self
            .base_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                row.iter()
                    .any(|cell| cell.to_ascii_lowercase().contains(&q))
            })
            .map(|(idx, _)| idx)
            .collect();

        self.selected_view_row = 0;
        self.scroll = 0;
        self.filter_active = true;
        self.set_status(format!("Query matched {} rows", self.view_rows.len()));
        self.set_feedback(
            format!("Filter matched {} rows", self.view_rows.len()),
            FeedbackKind::Success,
        );
        self.refresh_search_matches();
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
        match self.selected_cell().map(ToOwned::to_owned) {
            Some(cell) => match Action::open(&cell) {
                Ok(()) => {
                    self.set_status("Opened selected value");
                    self.set_feedback("Open action succeeded", FeedbackKind::Success);
                }
                Err(err) => {
                    self.set_status(format!("Open failed: {err}"));
                    self.set_feedback("Open action failed", FeedbackKind::Error);
                }
            },
            None => {
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
