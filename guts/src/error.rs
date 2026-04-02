use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] postgres::Error),

    #[error("MySQL error: {0}")]
    MySql(#[from] mysql::Error),

    #[error("MongoDB error: {0}")]
    Mongo(#[from] mongodb::error::Error),

    #[error("Open action failed: {0}")]
    Open(#[from] opener::OpenError),

    #[error("Unsupported source: {0}")]
    UnsupportedSource(String),

    #[error("Invalid JSON shape: {0}")]
    JsonShape(String),

    #[error("Action failed: {0}")]
    Action(String),

    #[error("Database configuration error: {0}")]
    DbConfig(String),

    #[error("Database operation error: {0}")]
    DbOperation(String),
}
