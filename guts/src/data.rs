use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Csv,
    Json,
    Sqlite,
}

#[derive(Debug, Clone)]
pub struct DataSet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub source: String,
    pub source_path: PathBuf,
    pub kind: SourceKind,
}

impl DataSet {
    pub fn from_path(path: &Path, sqlite_query: Option<&str>) -> AppResult<Self> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match ext.as_str() {
            "csv" => load_csv(path),
            "json" => load_json(path),
            "sqlite" | "db" => load_sqlite(path, sqlite_query),
            _ => Err(AppError::UnsupportedSource(path.display().to_string())),
        }
    }
}

pub fn load_sqlite(path: &Path, query: Option<&str>) -> AppResult<DataSet> {
    let conn = rusqlite::Connection::open(path)?;
    let sql = query.map(ToOwned::to_owned).unwrap_or_else(|| {
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name".to_string()
    });

    let mut stmt = conn.prepare(&sql)?;
    let headers = stmt
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let mut rows = Vec::new();
    let col_count = headers.len();
    let mapped = stmt.query_map([], |row| {
        let mut out = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let value = sqlite_value_to_string(row, i)?;
            out.push(value);
        }
        Ok(out)
    })?;

    for row in mapped {
        rows.push(row?);
    }

    Ok(DataSet {
        headers,
        rows,
        source: format!("sqlite: {}", path.display()),
        source_path: path.to_path_buf(),
        kind: SourceKind::Sqlite,
    })
}

fn sqlite_value_to_string(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<String> {
    use rusqlite::types::ValueRef;
    let value = row.get_ref(idx)?;
    let s = match value {
        ValueRef::Null => "null".to_string(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
        ValueRef::Blob(_) => "<blob>".to_string(),
    };
    Ok(s)
}

pub fn load_csv(path: &Path) -> AppResult<DataSet> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr
        .headers()?
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let rows = rdr
        .records()
        .map(|r| r.map(|record| record.iter().map(|s| s.to_string()).collect()))
        .collect::<Result<Vec<Vec<String>>, csv::Error>>()?;

    Ok(DataSet {
        headers,
        rows,
        source: format!("csv: {}", path.display()),
        source_path: path.to_path_buf(),
        kind: SourceKind::Csv,
    })
}

pub fn load_json(path: &Path) -> AppResult<DataSet> {
    let raw = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&raw)?;

    let arr = match value {
        Value::Array(items) => items,
        _ => {
            return Err(AppError::JsonShape(
                "JSON root must be an array of objects".to_string(),
            ));
        }
    };

    let mut headers = Vec::new();
    let mut rows = Vec::with_capacity(arr.len());

    for item in &arr {
        if let Value::Object(map) = item {
            for key in map.keys() {
                if !headers.iter().any(|h| h == key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    if headers.is_empty() {
        return Err(AppError::JsonShape(
            "JSON array must contain objects with fields".to_string(),
        ));
    }

    for item in arr {
        match item {
            Value::Object(map) => {
                let mut row = Vec::with_capacity(headers.len());
                for h in &headers {
                    let value = map.get(h).map(json_to_cell).unwrap_or_default();
                    row.push(value);
                }
                rows.push(row);
            }
            _ => {
                return Err(AppError::JsonShape(
                    "JSON array must contain only objects".to_string(),
                ));
            }
        }
    }

    Ok(DataSet {
        headers,
        rows,
        source: format!("json: {}", path.display()),
        source_path: path.to_path_buf(),
        kind: SourceKind::Json,
    })
}

fn json_to_cell(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
    }
}
