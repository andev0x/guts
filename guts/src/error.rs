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

    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("Open action failed: {0}")]
    Open(#[from] opener::OpenError),

    #[error("Unsupported source: {0}")]
    UnsupportedSource(String),

    #[error("Invalid JSON shape: {0}")]
    JsonShape(String),

    #[error("Action failed: {0}")]
    Action(String),
}
