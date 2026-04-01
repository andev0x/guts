mod action;
mod app;
mod data;
mod detect;
mod error;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use app::{App, InputMode};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use data::DataSet;
use error::AppResult;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

#[derive(Debug, Parser)]
#[command(name = "guts", version, about = "Fast terminal data explorer")]
struct Cli {
    #[arg(value_name = "SOURCE", help = "Path to .csv, .json, or .sqlite/.db")]
    source: PathBuf,

    #[arg(
        long,
        value_name = "SQL",
        help = "Initial SQL query for SQLite sources"
    )]
    query: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let dataset = DataSet::from_path(&cli.source, cli.query.as_deref())
        .map_err(|e| format!("Failed to open source: {e}"))?;
    let mut app = App::new(dataset);

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

fn handle_query_mode(app: &mut App, key: KeyEvent) -> AppResult<bool> {
    match key.code {
        KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.set_status("Query cancelled");
        }
        KeyCode::Enter => {
            let query = app.query_input.trim().to_string();
            if query.is_empty() {
                app.apply_filter_query();
                app.mode = InputMode::Normal;
                return Ok(false);
            }

            if app.source_kind == data::SourceKind::Sqlite {
                match DataSet::from_path(&app.source_path, Some(&query)) {
                    Ok(dataset) => {
                        app.replace_dataset(dataset);
                        app.set_status("Executed SQL query");
                    }
                    Err(err) => {
                        app.set_status(format!("SQL error: {err}"));
                    }
                }
            } else {
                app.apply_filter_query();
            }
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.query_input.pop();
        }
        KeyCode::Char(ch) => {
            app.query_input.push(ch);
        }
        _ => {}
    }
    Ok(false)
}
