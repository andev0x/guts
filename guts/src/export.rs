use crate::data::DataSet;
use crate::error::AppResult;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Csv {
        delimiter: char,
        include_headers: bool,
    },
    Json {
        pretty: bool,
        array: bool,
    },
    SqlDump {
        include_schema: bool,
        batch_size: usize,
    },
}

impl ExportFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "csv" => Some(Self::Csv {
                delimiter: ',',
                include_headers: true,
            }),
            "json" => Some(Self::Json {
                pretty: true,
                array: true,
            }),
            "sql" => Some(Self::SqlDump {
                include_schema: false,
                batch_size: 1000,
            }),
            _ => None,
        }
    }
}

/// Export a dataset to a file
pub fn export_dataset(
    dataset: &DataSet,
    output_path: &Path,
    format: ExportFormat,
) -> AppResult<String> {
    match format {
        ExportFormat::Csv {
            delimiter,
            include_headers,
        } => export_csv(dataset, output_path, delimiter, include_headers),
        ExportFormat::Json { pretty, array } => export_json(dataset, output_path, pretty, array),
        ExportFormat::SqlDump {
            include_schema,
            batch_size,
        } => export_sql(dataset, output_path, include_schema, batch_size),
    }
}

/// Export dataset as CSV
fn export_csv(
    dataset: &DataSet,
    output_path: &Path,
    delimiter: char,
    include_headers: bool,
) -> AppResult<String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter as u8)
        .from_path(output_path)?;

    if include_headers {
        wtr.write_record(&dataset.headers)?;
    }

    for row in &dataset.rows {
        wtr.write_record(row)?;
    }

    wtr.flush()?;
    Ok(format!(
        "Exported {} rows to {} (CSV)",
        dataset.rows.len(),
        output_path.display()
    ))
}

/// Export dataset as JSON
fn export_json(
    dataset: &DataSet,
    output_path: &Path,
    pretty: bool,
    array: bool,
) -> AppResult<String> {
    let mut file = File::create(output_path)?;

    if array {
        // Export as array of objects: [{"col1": "val1", "col2": "val2"}, ...]
        let objects: Vec<serde_json::Map<String, serde_json::Value>> = dataset
            .rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, header) in dataset.headers.iter().enumerate() {
                    let value = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    obj.insert(header.clone(), serde_json::Value::String(value.to_string()));
                }
                obj
            })
            .collect();

        let json_str = if pretty {
            serde_json::to_string_pretty(&objects)?
        } else {
            serde_json::to_string(&objects)?
        };
        file.write_all(json_str.as_bytes())?;
    } else {
        // Export as object with headers and rows
        let export_obj = serde_json::json!({
            "headers": dataset.headers,
            "rows": dataset.rows,
        });

        let json_str = if pretty {
            serde_json::to_string_pretty(&export_obj)?
        } else {
            serde_json::to_string(&export_obj)?
        };
        file.write_all(json_str.as_bytes())?;
    }

    file.flush()?;
    Ok(format!(
        "Exported {} rows to {} (JSON)",
        dataset.rows.len(),
        output_path.display()
    ))
}

/// Export dataset as SQL dump (INSERT statements)
fn export_sql(
    dataset: &DataSet,
    output_path: &Path,
    include_schema: bool,
    batch_size: usize,
) -> AppResult<String> {
    let mut file = File::create(output_path)?;

    // Infer table name from dataset source or use default
    let table_name = extract_table_name(&dataset.source).unwrap_or("exported_data");

    if include_schema {
        // Generate CREATE TABLE statement
        let create_table = generate_create_table(table_name, &dataset.headers);
        writeln!(file, "{}\n", create_table)?;
    }

    // Generate INSERT statements in batches
    if !dataset.rows.is_empty() {
        for chunk in dataset.rows.chunks(batch_size) {
            let insert_stmt = generate_batch_insert(table_name, &dataset.headers, chunk);
            writeln!(file, "{}", insert_stmt)?;
        }
    }

    file.flush()?;
    Ok(format!(
        "Exported {} rows to {} (SQL dump)",
        dataset.rows.len(),
        output_path.display()
    ))
}

fn extract_table_name(source: &str) -> Option<&str> {
    // Try to extract table name from source string
    if source.starts_with("sqlite:") || source.starts_with("table:") {
        source.split(':').nth(1)
    } else {
        None
    }
}

fn generate_create_table(table_name: &str, headers: &[String]) -> String {
    let columns = headers
        .iter()
        .map(|h| format!("  {} TEXT", escape_sql_identifier(h)))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        escape_sql_identifier(table_name),
        columns
    )
}

fn generate_batch_insert(table_name: &str, headers: &[String], rows: &[Vec<String>]) -> String {
    let column_list = headers
        .iter()
        .map(|h| escape_sql_identifier(h))
        .collect::<Vec<_>>()
        .join(", ");

    let values = rows
        .iter()
        .map(|row| {
            let vals = row
                .iter()
                .map(|v| escape_sql_string(v))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", vals)
        })
        .collect::<Vec<_>>()
        .join(",\n  ");

    format!(
        "INSERT INTO {} ({}) VALUES\n  {};",
        escape_sql_identifier(table_name),
        column_list,
        values
    )
}

fn escape_sql_identifier(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn escape_sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn create_test_dataset() -> DataSet {
        DataSet {
            headers: vec!["id".to_string(), "name".to_string(), "email".to_string()],
            rows: vec![
                vec![
                    "1".to_string(),
                    "Alice".to_string(),
                    "alice@example.com".to_string(),
                ],
                vec![
                    "2".to_string(),
                    "Bob".to_string(),
                    "bob@example.com".to_string(),
                ],
            ],
            source: "test".to_string(),
            source_locator: "test".to_string(),
            kind: crate::data::SourceKind::Csv,
        }
    }

    #[test]
    fn test_csv_export() {
        let dataset = create_test_dataset();
        let temp_file = std::env::temp_dir().join("test_export.csv");

        let result = export_csv(&dataset, &temp_file, ',', true);
        assert!(result.is_ok());

        let mut contents = String::new();
        File::open(&temp_file)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();

        assert!(contents.contains("id,name,email"));
        assert!(contents.contains("Alice"));

        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_json_export() {
        let dataset = create_test_dataset();
        let temp_file = std::env::temp_dir().join("test_export.json");

        let result = export_json(&dataset, &temp_file, true, true);
        assert!(result.is_ok());

        let mut contents = String::new();
        File::open(&temp_file)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();

        assert!(contents.contains("Alice"));
        assert!(contents.contains("alice@example.com"));

        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_sql_identifier_escape() {
        assert_eq!(escape_sql_identifier("table"), "\"table\"");
        assert_eq!(escape_sql_identifier("my\"table"), "\"my\"\"table\"");
    }

    #[test]
    fn test_sql_string_escape() {
        assert_eq!(escape_sql_string("hello"), "'hello'");
        assert_eq!(escape_sql_string("it's"), "'it''s'");
    }
}
