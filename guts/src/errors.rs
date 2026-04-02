use crate::error::AppError;

/// Enhanced error message with actionable suggestions
pub struct EnhancedError {
    pub original_error: String,
    pub suggestion: Option<String>,
    pub help_text: Option<String>,
}

impl EnhancedError {
    pub fn from_error(error: &AppError, context: Option<&str>) -> Self {
        match error {
            AppError::Sql(e) => Self::from_sqlite_error(e, context),
            AppError::Postgres(e) => Self::from_postgres_error(e, context),
            AppError::MySql(e) => Self::from_mysql_error(e, context),
            AppError::Mongo(e) => Self::from_mongo_error(e),
            AppError::Io(e) => Self::from_io_error(e),
            _ => Self {
                original_error: error.to_string(),
                suggestion: None,
                help_text: None,
            },
        }
    }

    pub fn format_user_friendly(&self) -> String {
        let mut parts = vec![format!("Error: {}", self.original_error)];

        if let Some(suggestion) = &self.suggestion {
            parts.push(format!("\nSuggestion: {}", suggestion));
        }

        if let Some(help) = &self.help_text {
            parts.push(format!("\nHelp: {}", help));
        }

        parts.join("")
    }

    fn from_sqlite_error(error: &rusqlite::Error, query: Option<&str>) -> Self {
        let error_str = error.to_string();
        let (suggestion, help) = if error_str.contains("no such table") {
            (
                Some(
                    "Check available tables by running ':' without a query, or use .schema command"
                        .to_string(),
                ),
                Some("Example: SELECT name FROM sqlite_master WHERE type='table'".to_string()),
            )
        } else if error_str.contains("no such column") {
            (
                Some("Check column names in your table schema".to_string()),
                Some("Example: PRAGMA table_info(table_name)".to_string()),
            )
        } else if error_str.contains("syntax error") {
            let help_msg = if let Some(q) = query {
                format!("Query: {}\nCheck your SQL syntax. Common issues: missing quotes, wrong operators, typos", q)
            } else {
                "Check your SQL syntax. Common issues: missing quotes, wrong operators, typos"
                    .to_string()
            };
            (
                Some("Review your SQL query syntax".to_string()),
                Some(help_msg),
            )
        } else if error_str.contains("UNIQUE constraint failed") {
            (
                Some("A row with this unique key already exists".to_string()),
                Some("Try updating instead of inserting, or use INSERT OR REPLACE".to_string()),
            )
        } else if error_str.contains("NOT NULL constraint failed") {
            (
                Some("A required column is missing a value".to_string()),
                Some("Ensure all NOT NULL columns have values in your INSERT/UPDATE".to_string()),
            )
        } else {
            (None, None)
        };

        Self {
            original_error: error_str,
            suggestion,
            help_text: help,
        }
    }

    fn from_postgres_error(error: &postgres::Error, _context: Option<&str>) -> Self {
        let error_str = error.to_string();
        let (suggestion, help) = if error_str.contains("relation")
            && error_str.contains("does not exist")
        {
            (
                Some("Table or view not found. Check spelling and schema".to_string()),
                Some(
                    "Use \\dt to list tables in psql, or SELECT * FROM information_schema.tables"
                        .to_string(),
                ),
            )
        } else if error_str.contains("column") && error_str.contains("does not exist") {
            (
                Some("Column not found in table".to_string()),
                Some("Use \\d table_name to see columns".to_string()),
            )
        } else if error_str.contains("permission denied") {
            (
                Some("You don't have permission for this operation".to_string()),
                Some(
                    "Contact your database administrator or check connection credentials"
                        .to_string(),
                ),
            )
        } else if error_str.contains("connection") {
            (
                Some("Cannot connect to PostgreSQL server".to_string()),
                Some("Check: 1) Server is running, 2) Host/port are correct, 3) Credentials are valid, 4) Network access".to_string()),
            )
        } else {
            (None, None)
        };

        Self {
            original_error: error_str,
            suggestion,
            help_text: help,
        }
    }

    fn from_mysql_error(error: &mysql::Error, _context: Option<&str>) -> Self {
        let error_str = error.to_string();
        let (suggestion, help) = if error_str.contains("Unknown table")
            || error_str.contains("doesn't exist")
        {
            (
                Some("Table not found. Check table name and database".to_string()),
                Some("Use SHOW TABLES to list available tables".to_string()),
            )
        } else if error_str.contains("Unknown column") {
            (
                Some("Column not found in table".to_string()),
                Some("Use DESCRIBE table_name to see columns".to_string()),
            )
        } else if error_str.contains("Access denied") {
            (
                Some("Authentication failed or insufficient privileges".to_string()),
                Some("Check username, password, and database permissions".to_string()),
            )
        } else if error_str.contains("Can't connect") {
            (
                Some("Cannot connect to MySQL server".to_string()),
                Some("Check: 1) Server is running, 2) Host/port are correct, 3) Firewall allows connection".to_string()),
            )
        } else {
            (None, None)
        };

        Self {
            original_error: error_str,
            suggestion,
            help_text: help,
        }
    }

    fn from_mongo_error(error: &mongodb::error::Error) -> Self {
        let error_str = error.to_string();
        let (suggestion, help) = if error_str.contains("NamespaceNotFound") {
            (
                Some("Collection not found".to_string()),
                Some("Use 'database' to list collections or check collection name".to_string()),
            )
        } else if error_str.contains("Authentication failed") {
            (
                Some("Invalid credentials".to_string()),
                Some("Check username and password in connection string".to_string()),
            )
        } else if error_str.contains("connection") {
            (
                Some("Cannot connect to MongoDB server".to_string()),
                Some("Check: 1) MongoDB is running, 2) Connection string is correct, 3) Network access".to_string()),
            )
        } else {
            (None, None)
        };

        Self {
            original_error: error_str,
            suggestion,
            help_text: help,
        }
    }

    fn from_io_error(error: &std::io::Error) -> Self {
        let error_str = error.to_string();
        let (suggestion, help) = match error.kind() {
            std::io::ErrorKind::NotFound => (
                Some("File or directory not found".to_string()),
                Some("Check the path is correct and the file exists".to_string()),
            ),
            std::io::ErrorKind::PermissionDenied => (
                Some("Permission denied".to_string()),
                Some(
                    "Check file permissions or try running with appropriate privileges".to_string(),
                ),
            ),
            std::io::ErrorKind::AlreadyExists => (
                Some("File already exists".to_string()),
                Some("Choose a different filename or delete the existing file".to_string()),
            ),
            _ => (None, None),
        };

        Self {
            original_error: error_str,
            suggestion,
            help_text: help,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_error_formatting() {
        let error = EnhancedError {
            original_error: "Table not found".to_string(),
            suggestion: Some("Check table name".to_string()),
            help_text: Some("Use SHOW TABLES".to_string()),
        };

        let formatted = error.format_user_friendly();
        assert!(formatted.contains("Error:"));
        assert!(formatted.contains("Suggestion:"));
        assert!(formatted.contains("Help:"));
    }
}
