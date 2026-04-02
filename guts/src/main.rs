mod action;
mod app;
mod config;
mod data;
mod detect;
mod error;
mod export;
mod fuzzy;
mod history;
mod logging;
mod theme;
mod ui;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use app::{App, InputMode};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use data::DataSet;
use error::AppResult;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use theme::load_active_theme;

#[derive(Debug, Parser)]
#[command(name = "guts", version, about = "Fast terminal data explorer")]
struct Cli {
    #[arg(
        value_name = "SOURCE",
        help = "Path to .csv/.json/.sqlite/.db or DB URI (postgres://, mysql://, mongodb://)",
        required_unless_present = "init_config"
    )]
    source: Option<String>,

    #[arg(
        long,
        value_name = "QUERY",
        help = "Initial query for database sources",
        requires = "source"
    )]
    query: Option<String>,

    #[arg(
        long,
        value_name = "SQL_FILE",
        help = "Execute SQL file directly (.sql) for SQLite/PostgreSQL/MySQL",
        requires = "source"
    )]
    sql_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "FILE",
        help = "Import CSV/JSON file into SQLite table",
        requires_all = ["source", "import_table"]
    )]
    import_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "TABLE",
        help = "Target SQLite table for --import-file",
        requires = "import_file"
    )]
    import_table: Option<String>,

    #[arg(
        long,
        value_name = "FILE",
        help = "Export current view/query results to file (format auto-detected from extension: .csv, .json, .sql)",
        requires = "source"
    )]
    export: Option<PathBuf>,

    #[arg(
        long,
        help = "Include CREATE TABLE statement in SQL exports",
        requires = "export"
    )]
    export_with_schema: bool,

    #[arg(
        long,
        value_name = "BACKUP_PATH",
        help = "Backup SQLite database to file",
        requires = "source"
    )]
    backup_to: Option<PathBuf>,

    #[arg(
        long,
        value_name = "BACKUP_PATH",
        help = "Restore SQLite database from backup file",
        requires = "source"
    )]
    restore_from: Option<PathBuf>,

    #[arg(
        long,
        help = "Open interactive UI after one-shot operations",
        requires = "source"
    )]
    open_ui: bool,

    #[arg(
        long,
        help = "Create ~/.config/guts/theme.toml with default template",
        conflicts_with = "source",
        conflicts_with = "query",
        conflicts_with = "sql_file",
        conflicts_with = "import_file",
        conflicts_with = "import_table",
        conflicts_with = "backup_to",
        conflicts_with = "restore_from",
        conflicts_with = "open_ui"
    )]
    init_config: bool,

    #[arg(long, help = "Show current configuration file path")]
    config_path: bool,

    #[arg(long, help = "Print merged configuration (with overrides)")]
    print_config: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load configuration
    let config = config::Config::load().unwrap_or_default();

    // Initialize logging (best effort, don't fail if it doesn't work)
    let _ = logging::init_logging(&config.logging);

    tracing::info!("Starting guts");

    // Handle config management commands
    if cli.config_path {
        match config::config_file_path() {
            Ok(path) => {
                println!("{}", path.display());
                if path.exists() {
                    println!("(file exists)");
                } else {
                    println!("(file does not exist - run with --init-config to create)");
                }
            }
            Err(e) => eprintln!("Error determining config path: {}", e),
        }
        return Ok(());
    }

    if cli.print_config {
        let config = config::Config::load()?;
        let serialized = toml::to_string_pretty(&config)?;
        println!("{}", serialized);
        return Ok(());
    }

    if cli.init_config {
        match config::Config::save_default()? {
            path => {
                println!("Created default config at {}", path.display());
                println!("\nYou can also create theme config with:");
                println!("  guts --init-config  (legacy theme.toml)");
            }
        }
        return Ok(());
    }

    let source = cli.source.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "SOURCE is required unless using --init-config, --config-path, or --print-config",
        )
    })?;

    let source_kind = data::detect_source_kind(source)
        .map_err(|e| format!("Failed to detect source type: {e}"))?;

    let has_one_shot_operation = cli.sql_file.is_some()
        || cli.import_file.is_some()
        || cli.backup_to.is_some()
        || cli.restore_from.is_some()
        || cli.export.is_some();

    if let Some(backup_path) = &cli.backup_to {
        ensure_sqlite_source(source_kind, "--backup-to")?;
        let message = data::backup_sqlite(Path::new(source), backup_path)
            .map_err(|e| format!("Backup failed: {e}"))?;
        println!("{message}");
    }

    if let Some(backup_path) = &cli.restore_from {
        ensure_sqlite_source(source_kind, "--restore-from")?;
        let message = data::restore_sqlite(Path::new(source), backup_path)
            .map_err(|e| format!("Restore failed: {e}"))?;
        println!("{message}");
    }

    if let Some(import_file) = &cli.import_file {
        ensure_sqlite_source(source_kind, "--import-file")?;
        let table = cli.import_table.as_deref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "--import-table is required")
        })?;
        let message = data::import_into_sqlite(Path::new(source), table, import_file)
            .map_err(|e| format!("Import failed: {e}"))?;
        println!("{message}");
    }

    if let Some(sql_file) = &cli.sql_file {
        let message = data::execute_sql_file(source, source_kind, sql_file)
            .map_err(|e| format!("SQL file execution failed: {e}"))?;
        println!("{message}");
    }

    if let Some(export_path) = &cli.export {
        // Load dataset
        let dataset = DataSet::from_source(source, cli.query.as_deref())
            .map_err(|e| format!("Failed to load data for export: {e}"))?;

        // Determine format from extension
        let format = export_path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(export::ExportFormat::from_extension)
            .ok_or_else(|| {
                "Unsupported export format. Use .csv, .json, or .sql extension".to_string()
            })?;

        // Apply schema flag for SQL exports
        let format = match format {
            export::ExportFormat::SqlDump { batch_size, .. } => export::ExportFormat::SqlDump {
                include_schema: cli.export_with_schema,
                batch_size,
            },
            other => other,
        };

        let message = export::export_dataset(&dataset, export_path, format)
            .map_err(|e| format!("Export failed: {e}"))?;
        println!("{message}");
    }

    if has_one_shot_operation && !cli.open_ui {
        return Ok(());
    }

    let dataset = DataSet::from_source(source, cli.query.as_deref())
        .map_err(|e| format!("Failed to open source: {e}"))?;
    let theme = load_active_theme();
    let mut app = App::new(dataset, theme);

    let terminal_result = run_terminal(&mut app);
    if let Err(err) = terminal_result {
        eprintln!("{err}");
    }

    Ok(())
}

fn run_terminal(app: &mut App) -> AppResult<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> AppResult<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if handle_key(app, key)? {
                        return Ok(());
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> AppResult<bool> {
    match app.mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::Search => {
            handle_search_mode(app, key);
            Ok(false)
        }
        InputMode::Query => handle_query_mode(app, key),
        InputMode::FuzzySearch => {
            handle_fuzzy_search_mode(app, key);
            Ok(false)
        }
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) -> AppResult<bool> {
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
        KeyCode::Char('h') | KeyCode::Left => app.move_left(),
        KeyCode::Char('l') | KeyCode::Right => app.move_right(),
        KeyCode::PageDown => app.page_down(20),
        KeyCode::PageUp => app.page_up(20),
        KeyCode::Char('g') => app.go_top(),
        KeyCode::Char('G') => app.go_bottom(),
        KeyCode::Char('/') => {
            app.mode = InputMode::Search;
            app.set_status("Search mode");
        }
        KeyCode::Char(':') => {
            app.mode = InputMode::Query;
            app.set_status("Query mode");
        }
        KeyCode::Char('f')
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.mode = InputMode::FuzzySearch;
            app.fuzzy_input.clear();
            app.refresh_fuzzy_matches();
            app.set_status("Fuzzy search mode");
        }
        KeyCode::Char('n') => app.search_next(),
        KeyCode::Char('N') => app.search_prev(),
        KeyCode::Char('o') => app.perform_open_action(),
        KeyCode::Char('y') => app.perform_copy_action(),
        _ => {}
    }
    Ok(false)
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.set_status("Search cancelled");
        }
        KeyCode::Enter => {
            app.refresh_search_matches();
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_input.pop();
            app.refresh_search_matches();
        }
        KeyCode::Char(ch) => {
            app.search_input.push(ch);
            app.refresh_search_matches();
        }
        _ => {}
    }
}

fn handle_fuzzy_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.set_status("Fuzzy search cancelled");
        }
        KeyCode::Enter => {
            app.fuzzy_select_column();
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.fuzzy_input.pop();
            app.refresh_fuzzy_matches();
        }
        KeyCode::Char(ch)
            if !key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.fuzzy_input.push(ch);
            app.refresh_fuzzy_matches();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.fuzzy_move_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.fuzzy_move_up();
        }
        _ => {}
    }
}

fn handle_query_mode(app: &mut App, key: KeyEvent) -> AppResult<bool> {
    match key.code {
        KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.query_history.reset_navigation();
            app.set_status("Query cancelled");
        }
        KeyCode::Char('p')
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.history_prev();
        }
        KeyCode::Char('n')
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.history_next();
        }
        KeyCode::Enter => {
            let query = app.query_input.trim().to_string();
            if query.is_empty() {
                app.apply_filter_query();
                app.mode = InputMode::Normal;
                return Ok(false);
            }

            let mut success = true;

            if matches!(
                app.source_kind,
                data::SourceKind::Sqlite
                    | data::SourceKind::Postgres
                    | data::SourceKind::MySql
                    | data::SourceKind::Mongo
            ) {
                if let Some(sql_file) = parse_sql_file_command(&query) {
                    match data::execute_sql_file(&app.source_locator, app.source_kind, &sql_file) {
                        Ok(message) => {
                            app.set_status(message);
                        }
                        Err(err) => {
                            app.set_status(format!("SQL file error: {err}"));
                            success = false;
                        }
                    }
                } else {
                    match data::execute_query(&app.source_locator, app.source_kind, &query) {
                        Ok(data::QueryExecution::Data(dataset, message)) => {
                            app.replace_dataset(dataset);
                            app.set_status(message);
                        }
                        Ok(data::QueryExecution::Message(message)) => {
                            app.set_status(message);
                        }
                        Err(err) => {
                            app.set_status(format!("Query error: {err}"));
                            success = false;
                        }
                    }
                }
            } else {
                app.apply_filter_query();
            }

            // Add to history if it's a database query
            if matches!(
                app.source_kind,
                data::SourceKind::Sqlite
                    | data::SourceKind::Postgres
                    | data::SourceKind::MySql
                    | data::SourceKind::Mongo
            ) {
                app.add_to_history(query, success);
            }

            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.query_input.pop();
            app.query_history.reset_navigation();
        }
        KeyCode::Char(ch)
            if !key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.query_input.push(ch);
            app.query_history.reset_navigation();
        }
        _ => {}
    }
    Ok(false)
}

fn ensure_sqlite_source(
    kind: data::SourceKind,
    operation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if kind != data::SourceKind::Sqlite {
        return Err(format!("{operation} is only supported for SQLite sources").into());
    }
    Ok(())
}

fn parse_sql_file_command(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix(".read ") {
        let path = rest.trim();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    if let Some(rest) = trimmed.strip_prefix("\\i ") {
        let path = rest.trim();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}
