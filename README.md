# guts

`guts` is a modern, keyboard-first, terminal data explorer for engineers.

It is designed for speed and clarity when exploring CSV, JSON, and SQLite data directly in the terminal.

## Features

- Fast loading for CSV, JSON, and SQLite (`.db`, `.sqlite`)
- Table rendering with sticky header and row selection
- Vim-style navigation (`h/j/k/l`, `g`, `G`, page up/down)
- Virtual scrolling for large datasets
- Incremental search (`/`, `n`, `N`)
- Query/filter mode (`:`)
  - CSV/JSON: text filter across all columns
  - SQLite: execute SQL queries
- Smart cell detection (URL, email, IP, number)
- Open action for links/emails (`o`)
- Copy selected cell to clipboard (`y`)

## Install

Prerequisites:

- Rust (stable)

Build locally:

```bash
cd guts
cargo build --release
```

## Usage

```bash
cd guts
cargo run -- <path/to/data.csv>
```

Examples:

```bash
# CSV
cargo run -- ./examples/users.csv

# JSON array of objects
cargo run -- ./examples/users.json

# SQLite table listing (default)
cargo run -- ./examples/app.db

# SQLite custom SQL query
cargo run -- ./examples/app.db --query "SELECT id, email FROM users LIMIT 100"
```

## Keybindings

- `q`: quit
- `h/j/k/l` or arrow keys: move cell/row selection
- `g` / `G`: jump to top/bottom
- `PageUp` / `PageDown`: move by page
- `/`: search mode
- `n` / `N`: next/previous search result
- `:`: query/filter mode
- `o`: open selected URL/email
- `y`: copy selected cell value

## Project Status

The core 1.0 feature set is implemented for local exploration workflows:

- CSV loading
- Table rendering
- Basic navigation
- Virtual scrolling
- Improved UI
- Row highlighting
- Search functionality
- Performance-oriented rendering path
- Smart detection (links, emails)
- Open/copy actions
- JSON support
- Query system
- SQLite integration

## Contributing

Contributions are welcome. Please read `CONTRIBUTING.md` before opening pull requests.

## License

MIT. See `LICENSE`.
