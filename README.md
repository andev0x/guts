# guts

[![Latest Release](https://img.shields.io/github/v/release/andev0x/guts?style=flat-square&color=A6E3A1)](https://github.com/andev0x/guts/releases)
[![CI Status](https://img.shields.io/github/actions/workflow/status/andev0x/guts/ci.yml?branch=main&style=flat-square)](https://github.com/andev0x/guts/actions)
[![Stars](https://img.shields.io/github/stars/andev0x/guts?style=flat-square&color=F9E2AF)](https://github.com/andev0x/guts/stargazers)
[![Downloads](https://img.shields.io/github/downloads/andev0x/guts/total?style=flat-square&color=89B4FA)](https://github.com/andev0x/guts/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](https://opensource.org/licenses/MIT)

`guts` is a modern, keyboard-first, terminal data explorer for engineers.

It is designed for speed and clarity when exploring CSV, JSON, and SQLite data directly in the terminal, heavily inspired by Vim-style navigation and Unix philosophy.

---

### Themes Showcase

<div align="center">
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/guts/catppuccin.png" width="49%" alt="Catppuccin Theme" />
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/guts/deep-sea.png" width="49%" alt="Deep Sea Theme" />
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/guts/gruvbox.png" width="49%" alt="Gruvbox Theme" />
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/guts/navy.png" width="49%" alt="Navy Theme" />
</div>

---

## Features

- Fast loading for CSV, JSON, and SQLite (`.db`, `.sqlite`)
- Database URI sources: PostgreSQL (`postgres://`), MySQL (`mysql://`), MongoDB (`mongodb://`)
- Table rendering with sticky header and row selection
- Vim-style navigation (`h/j/k/l`, `g`, `G`, page up/down)
- Virtual scrolling for large datasets
- Incremental search (`/`, `n`, `N`)
- Fuzzy search overlay across columns, table rows, and query history (`Ctrl-f`, `Tab`)
- Focus-aware highlighting for active row + column and fuzzy/search matches
- Query/filter mode (`:`)
  - CSV/JSON: text filter across all columns
  - SQLite/PostgreSQL/MySQL: execute SQL queries and non-SELECT statements
  - MongoDB: quick collection query (`collection_name [limit]`)
- Execute SQL files directly (`--sql-file` or query-mode `.read <file.sql>` / `\i <file.sql>`)
- Import CSV/JSON into SQLite tables (`--import-file`, `--import-table`)
- SQLite backup and restore (`--backup-to`, `--restore-from`)
- Smart cell detection (URL, email, IP, number)
- Open action for links/emails (`o`)
- Copy selected cell to clipboard (`y`)
- Theme system via `theme.toml` with built-in presets (Nord, Gravbox, Catppuccin, Monochrome)
- Safe color fallback to 16 ANSI colors when `theme.toml` is missing or terminal lacks TrueColor
- Configurable keybindings via `config.toml` (`[keybindings]`)

## Install

Prerequisites:

- Rust (stable)

Build locally:

```bash
cd guts
cargo build --release
```

Use 'guts' command

```bash
cargo install --path .

guts --help
```


Prebuilt binaries:

- Tagged releases (`v*`) automatically publish Linux/macOS/Windows binaries in GitHub Releases.

## Usage

```bash
cd guts
cargo run -- <source>
```

`<source>` can be either a file path (`.csv`, `.json`, `.db`, `.sqlite`) or database URI (`postgres://`, `mysql://`, `mongodb://`).

Initialize default theme config:

```bash
guts --init-config
```
> You’re running an older installed binary from ~/.cargo/bin/guts, not the newly built code.
- Your installed binary help still shows Usage: guts [OPTIONS] <SOURCE> and has no --init-config.
- The current local source does include it (local run shows Usage: guts [OPTIONS] [SOURCE] and --init-config).
Use one of these:
## from path
```bash
cargo install --path . --force
or run directly without installing:
cargo run -- --init-config
Then verify:
guts --help
You should see --init-config in the options.
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

# Execute SQL file against SQLite
cargo run -- ./examples/app.db --sql-file ./migrations/init.sql

# PostgreSQL URI source
cargo run -- "postgres://postgres:postgres@localhost:5432/app"

# Import CSV into SQLite table
cargo run -- ./examples/app.db --import-file ./examples/users.csv --import-table users

# Backup / restore SQLite
cargo run -- ./examples/app.db --backup-to ./backups/app-2026-04-02.db
cargo run -- ./examples/app.db --restore-from ./backups/app-2026-04-02.db
```

## Themes

`guts` can load a theme from `theme.toml`.

Config file discovery order:

1. `GUTS_THEME_FILE` (absolute or relative path)
2. `./theme.toml` (current working directory)
3. `$XDG_CONFIG_HOME/guts/theme.toml`
4. `$HOME/.config/guts/theme.toml`

`guts --init-config` creates `$HOME/.config/guts/theme.toml` with a ready-to-edit default template.

Built-in presets (`preset`):

- `nord` - snow blue, cool, soothing (great at night)
- `gravbox` - retro yellow, warm
- `catppuccin` - modern pastel
- `monochrome` - white/black/gray minimalist palette

Important fallback behavior:

- If no `theme.toml` is found, `guts` automatically uses a basic 16-color ANSI palette.
- If the terminal does not report TrueColor support, `guts` also falls back to the same ANSI palette.

Example `~/.config/guts/theme.toml`:

```toml
preset = "nord"

[colors]
border = "#81A1C1"
selected_background = "#5E81AC"
status_mode_background = "#A3BE8C"
```

## Keybindings

- Navigation
  - `h/j/k/l` or arrow keys: move cell/row selection
  - `g` / `G`: jump to top/bottom
  - `PageUp` / `PageDown`: move by page
- Search and filtering
  - `/`: search mode
  - `n` / `N`: next/previous search result
  - `:`: query/filter mode
  - `Ctrl-f`: fuzzy overlay
  - `Tab` (inside fuzzy): cycle target (`Columns` -> `Tables/Rows` -> `History`)
  - `Ctrl-p` / `Ctrl-n` (inside query): previous/next query history for current source type
- Actions
  - `o`: open selected URL/email
  - `y`: copy selected cell value
  - `q`: quit

Keybindings are configurable in `~/.config/guts/config.toml`:

```toml
[keybindings.navigation]
left = ["h", "Left"]
right = ["l", "Right"]
up = ["k", "Up"]
down = ["j", "Down"]

[keybindings.search]
fuzzy_mode = ["Ctrl-f"]
fuzzy_cycle_scope = ["Tab"]

[keybindings.actions]
copy = ["y"]
open = ["o"]
```

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

MIT. See [LICENSE](License).

## Contributors
<a href="https://github.com/andev0x/guts/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=andev0x/guts" />
</a>
