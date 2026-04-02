use std::collections::BTreeSet;
use std::path::Path;

use mongodb::bson::{Bson, Document};
use mongodb::sync::Client as MongoClient;
use mysql::prelude::Queryable;
use postgres::{Client as PostgresClient, NoTls, SimpleQueryMessage};
use serde_json::Value;

use crate::error::{AppError, AppResult};

const SQLITE_TABLE_LIST_SQL: &str =
    "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name";
const POSTGRES_TABLE_LIST_SQL: &str = "SELECT table_schema, table_name FROM information_schema.tables WHERE table_type='BASE TABLE' AND table_schema NOT IN ('pg_catalog', 'information_schema') ORDER BY table_schema, table_name LIMIT 500";
const MYSQL_TABLE_LIST_SQL: &str = "SELECT table_schema, table_name FROM information_schema.tables WHERE table_type='BASE TABLE' ORDER BY table_schema, table_name LIMIT 500";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Csv,
    Json,
    Sqlite,
    Postgres,
    MySql,
    Mongo,
}

#[derive(Debug, Clone)]
pub struct DataSet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub source: String,
    pub source_locator: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone)]
pub enum QueryExecution {
    Data(DataSet, String),
    Message(String),
}

impl DataSet {
    pub fn from_source(source: &str, initial_query: Option<&str>) -> AppResult<Self> {
        let kind = detect_source_kind(source)?;
        match kind {
            SourceKind::Csv => load_csv(Path::new(source)),
            SourceKind::Json => load_json(Path::new(source)),
            SourceKind::Sqlite => load_sqlite(
                Path::new(source),
                initial_query.unwrap_or(SQLITE_TABLE_LIST_SQL),
            ),
            SourceKind::Postgres => {
                load_postgres(source, initial_query.unwrap_or(POSTGRES_TABLE_LIST_SQL))
            }
            SourceKind::MySql => load_mysql(source, initial_query.unwrap_or(MYSQL_TABLE_LIST_SQL)),
            SourceKind::Mongo => load_mongo_collections(source),
        }
    }
}

pub fn detect_source_kind(source: &str) -> AppResult<SourceKind> {
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
        return Ok(SourceKind::Postgres);
    }
    if lower.starts_with("mysql://") {
        return Ok(SourceKind::MySql);
    }
    if lower.starts_with("mongodb://") || lower.starts_with("mongodb+srv://") {
        return Ok(SourceKind::Mongo);
    }

    let path = Path::new(source);
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "csv" => Ok(SourceKind::Csv),
        "json" => Ok(SourceKind::Json),
        "sqlite" | "db" => Ok(SourceKind::Sqlite),
        _ => Err(AppError::UnsupportedSource(source.to_string())),
    }
}

pub fn execute_query(
    source_locator: &str,
    kind: SourceKind,
    query: &str,
) -> AppResult<QueryExecution> {
    match kind {
        SourceKind::Sqlite => execute_sqlite_query(Path::new(source_locator), query),
        SourceKind::Postgres => execute_postgres_query(source_locator, query),
        SourceKind::MySql => execute_mysql_query(source_locator, query),
        SourceKind::Mongo => execute_mongo_command(source_locator, query),
        SourceKind::Csv | SourceKind::Json => Err(AppError::DbOperation(
            "Ad-hoc query execution is only available for database sources".to_string(),
        )),
    }
}

pub fn execute_sql_file(
    source_locator: &str,
    kind: SourceKind,
    sql_file: &Path,
) -> AppResult<String> {
    let sql = std::fs::read_to_string(sql_file)?;
    match kind {
        SourceKind::Sqlite => {
            let conn = rusqlite::Connection::open(source_locator)?;
            conn.execute_batch(&sql)?;
            Ok(format!("Executed SQL file {}", sql_file.display()))
        }
        SourceKind::Postgres => {
            let mut client = PostgresClient::connect(source_locator, NoTls)?;
            client.batch_execute(&sql)?;
            Ok(format!("Executed SQL file {}", sql_file.display()))
        }
        SourceKind::MySql => {
            let opts = mysql::Opts::from_url(source_locator)
                .map_err(|e| AppError::DbConfig(format!("Invalid MySQL URL: {e}")))?;
            let pool = mysql::Pool::new(opts)?;
            let mut conn = pool.get_conn()?;
            for stmt in split_sql_statements(&sql) {
                conn.query_drop(stmt)?;
            }
            Ok(format!("Executed SQL file {}", sql_file.display()))
        }
        SourceKind::Mongo => Err(AppError::DbOperation(
            "SQL files are not supported for MongoDB sources".to_string(),
        )),
        SourceKind::Csv | SourceKind::Json => Err(AppError::DbOperation(
            "SQL files can only be executed against SQL databases".to_string(),
        )),
    }
}

pub fn import_into_sqlite(db_path: &Path, table: &str, input_path: &Path) -> AppResult<String> {
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let (headers, rows) = match ext.as_str() {
        "csv" => load_csv_rows(input_path)?,
        "json" => load_json_rows(input_path)?,
        _ => {
            return Err(AppError::DbOperation(format!(
                "Unsupported import format for {} (expected .csv or .json)",
                input_path.display()
            )));
        }
    };

    if headers.is_empty() {
        return Err(AppError::DbOperation(
            "Import input has no columns".to_string(),
        ));
    }

    let mut conn = rusqlite::Connection::open(db_path)?;
    ensure_table_columns(&conn, table, &headers)?;
    let inserted = insert_rows(&mut conn, table, &headers, &rows)?;

    Ok(format!(
        "Imported {} rows from {} into {}",
        inserted,
        input_path.display(),
        table
    ))
}

pub fn backup_sqlite(source_db: &Path, backup_path: &Path) -> AppResult<String> {
    let _conn = rusqlite::Connection::open(source_db)?;
    ensure_parent_dir(backup_path)?;
    let bytes = std::fs::copy(source_db, backup_path)?;
    Ok(format!(
        "Backup created: {} ({} bytes)",
        backup_path.display(),
        bytes
    ))
}

pub fn restore_sqlite(source_db: &Path, backup_path: &Path) -> AppResult<String> {
    if !backup_path.exists() {
        return Err(AppError::DbOperation(format!(
            "Backup file does not exist: {}",
            backup_path.display()
        )));
    }
    ensure_parent_dir(source_db)?;
    let bytes = std::fs::copy(backup_path, source_db)?;
    Ok(format!(
        "Database restored from {} ({} bytes)",
        backup_path.display(),
        bytes
    ))
}

fn load_sqlite(path: &Path, query: &str) -> AppResult<DataSet> {
    let conn = rusqlite::Connection::open(path)?;
    let (headers, rows) = run_sqlite_select_like(&conn, query)?;
    Ok(DataSet {
        headers,
        rows,
        source: format!("sqlite: {}", path.display()),
        source_locator: path.display().to_string(),
        kind: SourceKind::Sqlite,
    })
}

fn load_postgres(url: &str, query: &str) -> AppResult<DataSet> {
    let mut client = PostgresClient::connect(url, NoTls)?;
    let (headers, rows, _) = run_postgres_simple_query(&mut client, query)?;
    Ok(DataSet {
        headers,
        rows,
        source: format!("postgres: {}", redact_uri(url)),
        source_locator: url.to_string(),
        kind: SourceKind::Postgres,
    })
}

fn load_mysql(url: &str, query: &str) -> AppResult<DataSet> {
    let opts = mysql::Opts::from_url(url)
        .map_err(|e| AppError::DbConfig(format!("Invalid MySQL URL: {e}")))?;
    let pool = mysql::Pool::new(opts)?;
    let mut conn = pool.get_conn()?;
    let (headers, rows) = run_mysql_select_like(&mut conn, query)?;
    Ok(DataSet {
        headers,
        rows,
        source: format!("mysql: {}", redact_uri(url)),
        source_locator: url.to_string(),
        kind: SourceKind::MySql,
    })
}

fn load_mongo_collections(url: &str) -> AppResult<DataSet> {
    let client = MongoClient::with_uri_str(url)?;
    let db = client.default_database().ok_or_else(|| {
        AppError::DbConfig("MongoDB URI must include a database name".to_string())
    })?;
    let rows = db
        .list_collection_names(None)?
        .into_iter()
        .map(|name| vec![name])
        .collect::<Vec<_>>();
    Ok(DataSet {
        headers: vec!["collection".to_string()],
        rows,
        source: format!("mongo: {}", redact_uri(url)),
        source_locator: url.to_string(),
        kind: SourceKind::Mongo,
    })
}

fn execute_sqlite_query(path: &Path, query: &str) -> AppResult<QueryExecution> {
    let conn = rusqlite::Connection::open(path)?;
    let mut stmt = conn.prepare(query)?;
    if stmt.column_count() == 0 {
        let affected = stmt.execute([])?;
        return Ok(QueryExecution::Message(format!(
            "SQL executed ({} rows affected)",
            affected
        )));
    }

    let headers = stmt
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let col_count = headers.len();
    let mapped = stmt.query_map([], |row| {
        let mut out = Vec::with_capacity(col_count);
        for idx in 0..col_count {
            out.push(sqlite_value_to_string(row, idx)?);
        }
        Ok(out)
    })?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row?);
    }

    let dataset = DataSet {
        headers,
        rows,
        source: format!("sqlite: {}", path.display()),
        source_locator: path.display().to_string(),
        kind: SourceKind::Sqlite,
    };
    Ok(QueryExecution::Data(
        dataset.clone(),
        format!("SQL returned {} rows", dataset.rows.len()),
    ))
}

fn execute_postgres_query(url: &str, query: &str) -> AppResult<QueryExecution> {
    let mut client = PostgresClient::connect(url, NoTls)?;
    let (headers, rows, affected_rows) = run_postgres_simple_query(&mut client, query)?;
    if headers.is_empty() {
        return Ok(QueryExecution::Message(format!(
            "SQL executed ({} rows affected)",
            affected_rows
        )));
    }

    let dataset = DataSet {
        headers,
        rows,
        source: format!("postgres: {}", redact_uri(url)),
        source_locator: url.to_string(),
        kind: SourceKind::Postgres,
    };
    Ok(QueryExecution::Data(
        dataset.clone(),
        format!("SQL returned {} rows", dataset.rows.len()),
    ))
}

fn execute_mysql_query(url: &str, query: &str) -> AppResult<QueryExecution> {
    let opts = mysql::Opts::from_url(url)
        .map_err(|e| AppError::DbConfig(format!("Invalid MySQL URL: {e}")))?;
    let pool = mysql::Pool::new(opts)?;
    let mut conn = pool.get_conn()?;

    let normalized = query.trim().to_ascii_lowercase();
    if looks_like_result_set_query(&normalized) {
        let (headers, rows) = run_mysql_select_like(&mut conn, query)?;
        let dataset = DataSet {
            headers,
            rows,
            source: format!("mysql: {}", redact_uri(url)),
            source_locator: url.to_string(),
            kind: SourceKind::MySql,
        };
        return Ok(QueryExecution::Data(
            dataset.clone(),
            format!("SQL returned {} rows", dataset.rows.len()),
        ));
    }

    conn.query_drop(query)?;
    Ok(QueryExecution::Message(format!(
        "SQL executed ({} rows affected)",
        conn.affected_rows()
    )))
}

fn execute_mongo_command(url: &str, command: &str) -> AppResult<QueryExecution> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        let dataset = load_mongo_collections(url)?;
        return Ok(QueryExecution::Data(
            dataset,
            "Listed collections".to_string(),
        ));
    }

    let client = MongoClient::with_uri_str(url)?;
    let db = client.default_database().ok_or_else(|| {
        AppError::DbConfig("MongoDB URI must include a database name".to_string())
    })?;

    let mut parts = trimmed.split_whitespace();
    let collection_name = parts
        .next()
        .ok_or_else(|| AppError::DbOperation("Mongo command is empty".to_string()))?;
    let limit = parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(100)
        .max(1);

    let collection = db.collection::<Document>(collection_name);
    let options = mongodb::options::FindOptions::builder()
        .limit(Some(limit))
        .build();
    let cursor = collection.find(None, options)?;

    let mut docs = Vec::new();
    for item in cursor {
        docs.push(item?);
    }

    let (headers, rows) = docs_to_rows(docs);
    let dataset = DataSet {
        headers,
        rows,
        source: format!("mongo: {}", redact_uri(url)),
        source_locator: url.to_string(),
        kind: SourceKind::Mongo,
    };
    Ok(QueryExecution::Data(
        dataset.clone(),
        format!(
            "Loaded {} documents from {}",
            dataset.rows.len(),
            collection_name
        ),
    ))
}

fn run_sqlite_select_like(
    conn: &rusqlite::Connection,
    query: &str,
) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
    let mut stmt = conn.prepare(query)?;
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
            out.push(sqlite_value_to_string(row, i)?);
        }
        Ok(out)
    })?;

    for row in mapped {
        rows.push(row?);
    }

    Ok((headers, rows))
}

fn sqlite_value_to_string(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<String> {
    use rusqlite::types::ValueRef;
    let value = row.get_ref(idx)?;
    Ok(match value {
        ValueRef::Null => "null".to_string(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
        ValueRef::Blob(_) => "<blob>".to_string(),
    })
}

fn run_postgres_simple_query(
    client: &mut PostgresClient,
    query: &str,
) -> AppResult<(Vec<String>, Vec<Vec<String>>, u64)> {
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut affected_rows = 0_u64;

    for message in client.simple_query(query)? {
        match message {
            SimpleQueryMessage::Row(row) => {
                if headers.is_empty() {
                    headers = row
                        .columns()
                        .iter()
                        .map(|col| col.name().to_string())
                        .collect();
                }
                let mut out = Vec::with_capacity(row.len());
                for idx in 0..row.len() {
                    out.push(row.get(idx).unwrap_or("null").to_string());
                }
                rows.push(out);
            }
            SimpleQueryMessage::CommandComplete(count) => {
                affected_rows += count;
            }
            _ => {}
        }
    }

    Ok((headers, rows, affected_rows))
}

fn run_mysql_select_like(
    conn: &mut mysql::PooledConn,
    query: &str,
) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
    let mut result = conn.query_iter(query)?;
    let mut headers = result
        .columns()
        .as_ref()
        .iter()
        .map(|col| col.name_str().to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();

    while let Some(row_result) = result.next() {
        let row = row_result?;
        if headers.is_empty() {
            headers = row
                .columns_ref()
                .iter()
                .map(|col| col.name_str().to_string())
                .collect();
        }
        let mut out = Vec::with_capacity(row.len());
        for idx in 0..row.len() {
            out.push(
                row.as_ref(idx)
                    .map(mysql_value_to_string)
                    .unwrap_or_else(|| "null".to_string()),
            );
        }
        rows.push(out);
    }

    Ok((headers, rows))
}

fn mysql_value_to_string(value: &mysql::Value) -> String {
    match value {
        mysql::Value::NULL => "null".to_string(),
        mysql::Value::Bytes(v) => String::from_utf8_lossy(v).to_string(),
        mysql::Value::Int(v) => v.to_string(),
        mysql::Value::UInt(v) => v.to_string(),
        mysql::Value::Float(v) => v.to_string(),
        mysql::Value::Double(v) => v.to_string(),
        mysql::Value::Date(y, m, d, hh, mm, ss, micros) => {
            format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}.{micros:06}")
        }
        mysql::Value::Time(neg, days, hours, mins, secs, micros) => {
            let sign = if *neg { "-" } else { "" };
            format!("{sign}{days} {hours:02}:{mins:02}:{secs:02}.{micros:06}")
        }
    }
}

fn load_csv(path: &Path) -> AppResult<DataSet> {
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
        source_locator: path.display().to_string(),
        kind: SourceKind::Csv,
    })
}

fn load_json(path: &Path) -> AppResult<DataSet> {
    let (headers, rows) = load_json_rows(path)?;
    Ok(DataSet {
        headers,
        rows,
        source: format!("json: {}", path.display()),
        source_locator: path.display().to_string(),
        kind: SourceKind::Json,
    })
}

fn load_csv_rows(path: &Path) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
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
    Ok((headers, rows))
}

fn load_json_rows(path: &Path) -> AppResult<(Vec<String>, Vec<Vec<String>>)> {
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
                    row.push(map.get(h).map(json_to_cell).unwrap_or_default());
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

    Ok((headers, rows))
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

fn docs_to_rows(docs: Vec<Document>) -> (Vec<String>, Vec<Vec<String>>) {
    let mut headers_set = BTreeSet::new();
    for doc in &docs {
        for key in doc.keys() {
            headers_set.insert(key.to_string());
        }
    }
    let headers = headers_set.into_iter().collect::<Vec<_>>();
    let rows = docs
        .into_iter()
        .map(|doc| {
            headers
                .iter()
                .map(|key| doc.get(key).map(bson_to_string).unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    (headers, rows)
}

fn bson_to_string(value: &Bson) -> String {
    match value {
        Bson::Null => "null".to_string(),
        Bson::String(v) => v.clone(),
        Bson::Boolean(v) => v.to_string(),
        Bson::Int32(v) => v.to_string(),
        Bson::Int64(v) => v.to_string(),
        Bson::Double(v) => v.to_string(),
        _ => value.to_string(),
    }
}

fn ensure_table_columns(
    conn: &rusqlite::Connection,
    table: &str,
    headers: &[String],
) -> AppResult<()> {
    let table_exists: i64 = conn.query_row(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |row| row.get(0),
    )?;

    if table_exists == 0 {
        let columns = headers
            .iter()
            .map(|h| format!("{} TEXT", quote_ident(h)))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("CREATE TABLE {} ({columns})", quote_ident(table));
        conn.execute(&sql, [])?;
        return Ok(());
    }

    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(table)))?;
    let mut existing = BTreeSet::new();
    let mapped = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in mapped {
        existing.insert(name?);
    }

    for header in headers {
        if !existing.contains(header) {
            let sql = format!(
                "ALTER TABLE {} ADD COLUMN {} TEXT",
                quote_ident(table),
                quote_ident(header)
            );
            conn.execute(&sql, [])?;
        }
    }

    Ok(())
}

fn insert_rows(
    conn: &mut rusqlite::Connection,
    table: &str,
    headers: &[String],
    rows: &[Vec<String>],
) -> AppResult<usize> {
    if rows.is_empty() {
        return Ok(0);
    }

    let tx = conn.transaction()?;
    let columns = headers
        .iter()
        .map(|h| quote_ident(h))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = std::iter::repeat_n("?", headers.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({columns}) VALUES ({placeholders})",
        quote_ident(table)
    );

    let mut stmt = tx.prepare(&sql)?;
    let mut inserted = 0_usize;
    for row in rows {
        if row.len() != headers.len() {
            return Err(AppError::DbOperation(
                "Row/column mismatch while importing data".to_string(),
            ));
        }
        stmt.execute(rusqlite::params_from_iter(row.iter()))?;
        inserted += 1;
    }
    drop(stmt);
    tx.commit()?;
    Ok(inserted)
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn split_sql_statements(sql: &str) -> Vec<&str> {
    sql.split(';')
        .map(str::trim)
        .filter(|stmt| !stmt.is_empty())
        .collect()
}

fn looks_like_result_set_query(normalized_query: &str) -> bool {
    ["select", "show", "describe", "desc", "explain", "with"]
        .iter()
        .any(|prefix| normalized_query.starts_with(prefix))
}

fn redact_uri(uri: &str) -> String {
    match uri.split_once("://") {
        Some((scheme, rest)) => match rest.find('@') {
            Some(at_idx) => format!("{scheme}://***@{}", &rest[at_idx + 1..]),
            None => uri.to_string(),
        },
        None => uri.to_string(),
    }
}

fn ensure_parent_dir(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn source_query_hint(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Csv | SourceKind::Json => ": filter",
        SourceKind::Sqlite | SourceKind::Postgres | SourceKind::MySql => ": SQL query",
        SourceKind::Mongo => ": mongo (collection [limit])",
    }
}
