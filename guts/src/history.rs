use crate::data::SourceKind;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEntry {
    pub query: String,
    pub timestamp: SystemTime,
    pub source_kind: SourceKind,
    pub success: bool,
}

impl QueryEntry {
    pub fn new(query: String, source_kind: SourceKind, success: bool) -> Self {
        Self {
            query,
            timestamp: SystemTime::now(),
            source_kind,
            success,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistory {
    entries: Vec<QueryEntry>,
    max_size: usize,
    #[serde(skip)]
    current_idx: Option<usize>,
    #[serde(skip)]
    current_filtered_pos: Option<usize>,
    #[serde(skip)]
    current_filtered_source: Option<SourceKind>,
}

impl QueryHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            current_idx: None,
            current_filtered_pos: None,
            current_filtered_source: None,
        }
    }

    pub fn load() -> AppResult<Self> {
        let path = history_file_path()?;
        if !path.exists() {
            return Ok(Self::new(500));
        }

        let contents = fs::read_to_string(&path)?;
        let mut history: QueryHistory = serde_json::from_str(&contents)?;
        history.current_idx = None; // Reset navigation on load
        history.current_filtered_pos = None;
        history.current_filtered_source = None;
        Ok(history)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = history_file_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    pub fn add(&mut self, entry: QueryEntry) {
        // Don't add duplicate consecutive entries
        if self
            .entries
            .last()
            .map(|last| last.query == entry.query)
            .unwrap_or(false)
        {
            return;
        }

        self.entries.push(entry);

        // Trim to max_size
        if self.entries.len() > self.max_size {
            let excess = self.entries.len() - self.max_size;
            self.entries.drain(0..excess);
        }

        // Reset navigation
        self.current_idx = None;
        self.current_filtered_pos = None;
        self.current_filtered_source = None;
    }

    pub fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;
        if self.entries.len() > self.max_size {
            let excess = self.entries.len() - self.max_size;
            self.entries.drain(0..excess);
        }
        self.reset_navigation();
    }

    #[allow(dead_code)]
    pub fn search(&self, pattern: &str) -> Vec<&QueryEntry> {
        let pattern_lower = pattern.to_lowercase();
        self.entries
            .iter()
            .rev() // Most recent first
            .filter(|entry| entry.query.to_lowercase().contains(&pattern_lower))
            .collect()
    }

    #[allow(dead_code)]
    pub fn get_prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        match self.current_idx {
            None => {
                // Start from the end (most recent)
                self.current_idx = Some(self.entries.len() - 1);
            }
            Some(idx) if idx > 0 => {
                // Move backward
                self.current_idx = Some(idx - 1);
            }
            Some(_) => {
                // Already at the oldest entry
                return self
                    .current_idx
                    .and_then(|i| self.entries.get(i).map(|e| e.query.as_str()));
            }
        }

        self.current_idx
            .and_then(|i| self.entries.get(i).map(|e| e.query.as_str()))
    }

    #[allow(dead_code)]
    pub fn get_next(&mut self) -> Option<&str> {
        match self.current_idx {
            None => None, // No navigation in progress
            Some(idx) if idx + 1 < self.entries.len() => {
                // Move forward
                self.current_idx = Some(idx + 1);
                self.current_idx
                    .and_then(|i| self.entries.get(i).map(|e| e.query.as_str()))
            }
            Some(_) => {
                // At the newest entry, return to "no selection"
                self.current_idx = None;
                None
            }
        }
    }

    pub fn get_prev_for_source(&mut self, source_kind: SourceKind) -> Option<&str> {
        if self.current_filtered_source != Some(source_kind) {
            self.current_filtered_source = Some(source_kind);
            self.current_filtered_pos = None;
        }

        let positions = self.filtered_positions(source_kind);
        if positions.is_empty() {
            return None;
        }

        match self.current_filtered_pos {
            None => {
                self.current_filtered_pos = Some(positions.len() - 1);
            }
            Some(pos) if pos > 0 => {
                self.current_filtered_pos = Some(pos - 1);
            }
            Some(_) => {}
        }

        self.current_filtered_pos
            .and_then(|pos| positions.get(pos).copied())
            .and_then(|idx| self.entries.get(idx).map(|entry| entry.query.as_str()))
    }

    pub fn get_next_for_source(&mut self, source_kind: SourceKind) -> Option<&str> {
        if self.current_filtered_source != Some(source_kind) {
            self.current_filtered_source = Some(source_kind);
            self.current_filtered_pos = None;
            return None;
        }

        let positions = self.filtered_positions(source_kind);
        if positions.is_empty() {
            return None;
        }

        match self.current_filtered_pos {
            None => None,
            Some(pos) if pos + 1 < positions.len() => {
                self.current_filtered_pos = Some(pos + 1);
                self.current_filtered_pos
                    .and_then(|new_pos| positions.get(new_pos).copied())
                    .and_then(|idx| self.entries.get(idx).map(|entry| entry.query.as_str()))
            }
            Some(_) => {
                self.current_filtered_pos = None;
                None
            }
        }
    }

    pub fn recent_queries_for_source(&self, source_kind: SourceKind, limit: usize) -> Vec<String> {
        let mut seen = HashSet::new();
        self.entries
            .iter()
            .rev()
            .filter(|entry| entry.source_kind == source_kind)
            .filter_map(|entry| {
                if seen.insert(entry.query.clone()) {
                    Some(entry.query.clone())
                } else {
                    None
                }
            })
            .take(limit)
            .collect()
    }

    pub fn reset_navigation(&mut self) {
        self.current_idx = None;
        self.current_filtered_pos = None;
        self.current_filtered_source = None;
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn filtered_positions(&self, source_kind: SourceKind) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| (entry.source_kind == source_kind).then_some(idx))
            .collect()
    }
}

fn history_file_path() -> AppResult<PathBuf> {
    let data_dir = if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/share")
    } else {
        return Err(crate::error::AppError::DbConfig(
            "Could not determine data directory".to_string(),
        ));
    };

    Ok(data_dir.join("guts").join("history.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_retrieve() {
        let mut history = QueryHistory::new(10);
        history.add(QueryEntry::new(
            "SELECT * FROM users".to_string(),
            SourceKind::Sqlite,
            true,
        ));

        assert_eq!(history.len(), 1);
        assert_eq!(history.get_prev(), Some("SELECT * FROM users"));
    }

    #[test]
    fn test_max_size() {
        let mut history = QueryHistory::new(3);
        history.add(QueryEntry::new(
            "query1".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "query2".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "query3".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "query4".to_string(),
            SourceKind::Sqlite,
            true,
        ));

        assert_eq!(history.len(), 3);
        assert_eq!(history.get_prev(), Some("query4"));
    }

    #[test]
    fn test_navigation() {
        let mut history = QueryHistory::new(10);
        history.add(QueryEntry::new(
            "query1".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "query2".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "query3".to_string(),
            SourceKind::Sqlite,
            true,
        ));

        assert_eq!(history.get_prev(), Some("query3"));
        assert_eq!(history.get_prev(), Some("query2"));
        assert_eq!(history.get_prev(), Some("query1"));
        assert_eq!(history.get_next(), Some("query2"));
        assert_eq!(history.get_next(), Some("query3"));
    }

    #[test]
    fn test_search() {
        let mut history = QueryHistory::new(10);
        history.add(QueryEntry::new(
            "SELECT * FROM users".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "INSERT INTO users VALUES".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "SELECT * FROM products".to_string(),
            SourceKind::Sqlite,
            true,
        ));

        let results = history.search("SELECT");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_no_duplicate_consecutive() {
        let mut history = QueryHistory::new(10);
        history.add(QueryEntry::new(
            "SELECT * FROM users".to_string(),
            SourceKind::Sqlite,
            true,
        ));
        history.add(QueryEntry::new(
            "SELECT * FROM users".to_string(),
            SourceKind::Sqlite,
            true,
        ));

        assert_eq!(history.len(), 1);
    }
}
