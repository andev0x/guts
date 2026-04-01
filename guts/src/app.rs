use std::cmp::{max, min};
use std::path::PathBuf;

use crate::action::Action;
use crate::data::{DataSet, SourceKind};
use crate::detect::{CellKind, detect_kind};
use crate::theme::ActiveTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Query,
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
    pub status: String,
    pub mode: InputMode,
    pub source_label: String,
    pub source_path: PathBuf,
    pub source_kind: SourceKind,
    pub theme: ActiveTheme,
}

impl App {
    pub fn new(dataset: DataSet, theme: ActiveTheme) -> Self {
        let view_rows = (0..dataset.rows.len()).collect::<Vec<_>>();
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
            status: theme.initial_status(),
            mode: InputMode::Normal,
            source_label: dataset.source,
            source_path: dataset.source_path,
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
        self.source_path = dataset.source_path;
        self.source_kind = dataset.kind;
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

    pub fn apply_filter_query(&mut self) {
        let q = self.query_input.trim().to_ascii_lowercase();
        if q.is_empty() {
            self.view_rows = (0..self.base_rows.len()).collect();
            self.selected_view_row = 0;
            self.scroll = 0;
            self.set_status("Query cleared");
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
        self.set_status(format!("Query matched {} rows", self.view_rows.len()));
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
        } else {
            self.search_match_idx = 0;
            self.selected_view_row = self.search_matches[0];
            self.set_status(format!(
                "Search: {}/{}",
                self.search_match_idx + 1,
                self.search_matches.len()
            ));
        }
    }

    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            self.set_status("Search: no matches");
            return;
        }
        self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
        self.selected_view_row = self.search_matches[self.search_match_idx];
        self.set_status(format!(
            "Search: {}/{}",
            self.search_match_idx + 1,
            self.search_matches.len()
        ));
    }

    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            self.set_status("Search: no matches");
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
    }

    pub fn perform_open_action(&mut self) {
        match self.selected_cell().map(ToOwned::to_owned) {
            Some(cell) => match Action::open(&cell) {
                Ok(()) => self.set_status("Opened selected value"),
                Err(err) => self.set_status(format!("Open failed: {err}")),
            },
            None => self.set_status("No cell selected"),
        }
    }

    pub fn perform_copy_action(&mut self) {
        match self.selected_cell().map(ToOwned::to_owned) {
            Some(cell) => match Action::copy(&cell) {
                Ok(()) => self.set_status("Copied selected value"),
                Err(err) => self.set_status(format!("Copy failed: {err}")),
            },
            None => self.set_status("No cell selected"),
        }
    }

    pub fn is_search_match_view_row(&self, view_idx: usize) -> bool {
        self.search_matches.binary_search(&view_idx).is_ok()
    }
}

fn fallback_headers(rows: &[Vec<String>]) -> Vec<String> {
    let cols = rows.first().map(|r| r.len()).unwrap_or(1);
    (1..=cols).map(|i| format!("col_{i}")).collect()
}
