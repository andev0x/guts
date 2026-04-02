use crate::config::LogConfig;
use crate::error::AppResult;
use std::fs;
use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(config: &LogConfig) -> AppResult<()> {
    // Expand tilde in log file path
    let log_file_path = expand_path(&config.file)?;

    // Ensure log directory exists
    if let Some(parent) = log_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create rolling file appender
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_file_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
        log_file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("guts.log"),
    );

    // Parse log level
    let level = parse_log_level(&config.level);

    // Create filter
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Set up subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(file_appender).with_ansi(false))
        .init();

    tracing::info!(
        "Logging initialized: level={}, file={}",
        config.level,
        config.file
    );

    Ok(())
}

fn expand_path(path: &str) -> AppResult<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home).join(stripped));
        }
    }
    Ok(PathBuf::from(path))
}

fn parse_log_level(level: &str) -> &'static str {
    match level.to_lowercase().as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "info" => "info",
        "warn" | "warning" => "warn",
        "error" => "error",
        "off" => "off",
        _ => "info",
    }
}

// Convenience macros for common logging operations
#[macro_export]
macro_rules! log_operation {
    ($operation:expr, $details:expr) => {
        tracing::info!(operation = $operation, details = $details);
    };
}

#[macro_export]
macro_rules! log_error {
    ($error:expr, $context:expr) => {
        tracing::error!(error = ?$error, context = $context);
    };
}

#[macro_export]
macro_rules! log_query {
    ($query:expr, $rows:expr, $duration_ms:expr) => {
        tracing::debug!(
            query = $query,
            rows = $rows,
            duration_ms = $duration_ms,
            "Query executed"
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path() {
        let path = "~/.local/share/guts/guts.log";
        let expanded = expand_path(path).unwrap();
        assert!(
            expanded
                .to_string_lossy()
                .contains(".local/share/guts/guts.log")
        );
    }

    #[test]
    fn test_parse_log_level() {
        assert_eq!(parse_log_level("debug"), "debug");
        assert_eq!(parse_log_level("DEBUG"), "debug");
        assert_eq!(parse_log_level("info"), "info");
        assert_eq!(parse_log_level("invalid"), "info");
    }
}
