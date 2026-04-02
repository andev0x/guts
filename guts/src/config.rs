use crate::error::{AppError, AppResult};
use crate::keybinding::KeybindingConfig;
use crate::theme::ThemeConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub import: ImportConfig,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub logging: LogConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub keybindings: KeybindingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_max_history")]
    pub max_history: usize,
    #[serde(default = "default_true")]
    pub auto_save_history: bool,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
    #[serde(default)]
    pub search_case_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_connect_timeout")]
    pub postgres_connect_timeout: u64,
    #[serde(default = "default_pool_size")]
    pub mysql_pool_size: usize,
    #[serde(default = "default_busy_timeout")]
    pub sqlite_busy_timeout: u64,
    #[serde(default = "default_mongo_limit")]
    pub mongo_default_limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    #[serde(default = "default_delimiter")]
    pub csv_delimiter: char,
    #[serde(default = "default_true")]
    pub csv_has_headers: bool,
    #[serde(default = "default_preview_rows")]
    pub preview_rows: usize,
    #[serde(default = "default_true")]
    pub infer_types: bool,
    #[serde(default = "default_true")]
    pub validate_before_insert: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    #[serde(default = "default_export_format")]
    pub default_format: String,
    #[serde(default = "default_true")]
    pub csv_include_headers: bool,
    #[serde(default = "default_true")]
    pub json_pretty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_file")]
    pub file: String,
    #[serde(default = "default_log_max_size")]
    pub max_size_mb: u64,
    #[serde(default = "default_log_rotate_count")]
    pub rotate_count: usize,
}

// Default value functions
fn default_max_history() -> usize {
    500
}
fn default_true() -> bool {
    true
}
fn default_page_size() -> usize {
    50
}
fn default_connect_timeout() -> u64 {
    30
}
fn default_pool_size() -> usize {
    5
}
fn default_busy_timeout() -> u64 {
    5000
}
fn default_mongo_limit() -> i64 {
    100
}
fn default_delimiter() -> char {
    ','
}
fn default_preview_rows() -> usize {
    10
}
fn default_export_format() -> String {
    "csv".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_file() -> String {
    "~/.local/share/guts/guts.log".to_string()
}
fn default_log_max_size() -> u64 {
    10
}
fn default_log_rotate_count() -> usize {
    3
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            max_history: default_max_history(),
            auto_save_history: default_true(),
            page_size: default_page_size(),
            search_case_sensitive: false,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            postgres_connect_timeout: default_connect_timeout(),
            mysql_pool_size: default_pool_size(),
            sqlite_busy_timeout: default_busy_timeout(),
            mongo_default_limit: default_mongo_limit(),
        }
    }
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            csv_delimiter: default_delimiter(),
            csv_has_headers: default_true(),
            preview_rows: default_preview_rows(),
            infer_types: default_true(),
            validate_before_insert: default_true(),
        }
    }
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            default_format: default_export_format(),
            csv_include_headers: default_true(),
            json_pretty: default_true(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: default_log_file(),
            max_size_mb: default_log_max_size(),
            rotate_count: default_log_rotate_count(),
        }
    }
}

impl Config {
    /// Load configuration with the following priority:
    /// 1. Default values
    /// 2. Config file (if exists)
    /// 3. Environment variable overrides
    pub fn load() -> AppResult<Self> {
        let mut config = Self::load_from_file().unwrap_or_default();
        config.apply_env_overrides();
        Ok(config)
    }

    fn load_from_file() -> AppResult<Self> {
        let path = config_file_path()?;
        if !path.exists() {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Config file not found",
            )));
        }

        let contents = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        // General
        if let Some(size) = env::var("GUTS_HISTORY_SIZE")
            .ok()
            .and_then(|val| val.parse::<usize>().ok())
        {
            self.general.max_history = size;
        }
        if let Some(size) = env::var("GUTS_PAGE_SIZE")
            .ok()
            .and_then(|val| val.parse::<usize>().ok())
        {
            self.general.page_size = size;
        }

        // Database
        if let Some(timeout) = env::var("GUTS_DB_TIMEOUT")
            .ok()
            .and_then(|val| val.parse::<u64>().ok())
        {
            self.database.postgres_connect_timeout = timeout;
        }

        // Logging
        if let Ok(level) = env::var("GUTS_LOG_LEVEL") {
            self.logging.level = level;
        }
        if let Ok(file) = env::var("GUTS_LOG_FILE") {
            self.logging.file = file;
        }
    }

    pub fn save_default() -> AppResult<PathBuf> {
        let path = config_file_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let default_config = Self::default();
        let contents = toml::to_string_pretty(&default_config)
            .map_err(|e| AppError::DbConfig(format!("Failed to serialize config: {}", e)))?;

        fs::write(&path, contents)?;
        Ok(path)
    }
}

pub fn config_file_path() -> AppResult<PathBuf> {
    // Priority: GUTS_CONFIG_FILE > XDG_CONFIG_HOME > HOME/.config
    if let Ok(path) = env::var("GUTS_CONFIG_FILE") {
        return Ok(PathBuf::from(path));
    }

    let config_dir = if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir)
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        return Err(AppError::DbConfig(
            "Could not determine config directory".to_string(),
        ));
    };

    Ok(config_dir.join("guts").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.max_history, 500);
        assert_eq!(config.database.mongo_default_limit, 100);
        assert_eq!(config.export.default_format, "csv");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config.general.max_history, deserialized.general.max_history);
    }
}
