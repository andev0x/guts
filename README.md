# guts

[![Latest Release](https://img.shields.io/github/v/release/andev0x/guts?style=flat-square&color=A6E3A1)](https://github.com/andev0x/guts/releases)
[![CI Status](https://img.shields.io/github/actions/workflow/status/andev0x/guts/ci.yml?branch=main&style=flat-square)](https://github.com/andev0x/guts/actions)
[![Stars](https://img.shields.io/github/stars/andev0x/guts?style=flat-square&color=F9E2AF)](https://github.com/andev0x/guts/stargazers)
[![Downloads](https://img.shields.io/github/downloads/andev0x/guts/total?style=flat-square&color=89B4FA)](https://github.com/andev0x/guts/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](https://opensource.org/licenses/MIT)

A modern, keyboard-first terminal data explorer for engineers. Guts is designed for speed and clarity when exploring CSV, JSON, and SQLite data directly in the terminal. It is heavily inspired by Vim-style navigation and Unix philosophy.

## Overview

Guts enables rapid data exploration and analysis through an intuitive terminal interface. Whether you're working with local files (CSV, JSON, SQLite) or remote databases (PostgreSQL, MySQL, MongoDB), Guts provides a fast, keyboard-driven experience designed for modern data workflows.

## Demo
<div align="center">
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/guts/guts.gif" width="80%" alt="guts" />
</div>



## Features

- Fast loading and querying of multiple data formats
  - CSV and JSON files
  - SQLite databases (.db, .sqlite)
  - PostgreSQL, MySQL, and MongoDB databases
- Efficient table rendering with sticky headers and row selection
- Virtual scrolling for large datasets with minimal overhead
- Vim-style keyboard navigation (h/j/k/l, g, G, Page Up/Down)
- Powerful search and filtering capabilities
  - Incremental search with next/previous navigation
  - Fuzzy search across columns, rows, and history
  - Query mode with support for SQL and text filters
  - MongoDB quick collection queries
- Advanced features
  - Execute SQL files directly from command line or query mode
  - Import CSV/JSON files into SQLite tables
  - SQLite backup and restore functionality
  - Smart cell type detection (URLs, emails, IP addresses, numbers)
  - Open URLs and email addresses directly from cells
  - Copy cell values to clipboard
- Theming and customization
  - Built-in theme system with TOML configuration
  - Pre-configured theme presets (Nord, Gruvbox, Catppuccin, Monochrome)
  - Automatic fallback to 16-color ANSI palette for basic terminal compatibility
  - Fully configurable keybindings

## Installation

### Prerequisites

- Rust (stable or nightly)

### From Source

Clone the repository and build locally:

```bash
git clone https://github.com/andev0x/guts.git
cd guts
cargo build --release
```

Install the binary:

```bash
cargo install --path .
```

Verify the installation:

```bash
guts --help
```

### Prebuilt Binaries

Prebuilt binaries for Linux, macOS, and Windows are available in [GitHub Releases](https://github.com/andev0x/guts/releases). Tagged releases are automatically published with platform-specific builds.

## Quick Start

Run Guts with a data source:

```bash
guts <source>
```

The source can be a file path or database URI:

- Local files: `.csv`, `.json`, `.db`, `.sqlite`
- Databases: `postgres://`, `mysql://`, `mongodb://`

### Examples

```bash
# Explore a CSV file
guts users.csv

# Open a JSON file
guts data.json

# Browse SQLite database tables
guts app.db

# Execute a custom SQL query
guts app.db --query "SELECT id, email FROM users LIMIT 100"

# Execute SQL from a file
guts app.db --sql-file migrations/init.sql

# Connect to PostgreSQL
guts "postgres://user:password@localhost:5432/database"

# Import CSV into SQLite
guts app.db --import-file users.csv --import-table users

# Backup SQLite database
guts app.db --backup-to backups/app-backup.db

# Restore from backup
guts app.db --restore-from backups/app-backup.db
```

### Initial Configuration

Initialize the default configuration:

```bash
guts --init-config
```

This creates a default `theme.toml` at `$HOME/.config/guts/theme.toml` for customization.

## Configuration

### Themes

Guts includes a theming system based on TOML configuration. Configuration files are discovered in the following order:

1. `GUTS_THEME_FILE` environment variable (absolute or relative path)
2. `./theme.toml` (current working directory)
3. `$XDG_CONFIG_HOME/guts/theme.toml`
4. `$HOME/.config/guts/theme.toml`

If no theme file is found, Guts automatically falls back to a basic 16-color ANSI palette. If the terminal does not support TrueColor, Guts also uses the ANSI palette.

#### Built-in Presets

- `nord` - Snow blue, cool, and soothing theme
- `gruvbox` - Retro warm theme with vintage yellow tones
- `catppuccin` - Modern pastel color scheme
- `monochrome` - Minimalist white, black, and gray palette

#### Example Configuration

Create or edit `~/.config/guts/theme.toml`:

```toml
preset = "nord"

[colors]
border = "#81A1C1"
selected_background = "#5E81AC"
status_mode_background = "#A3BE8C"
```

### Keybindings

The default keybindings follow Vim conventions. Configure custom keybindings in `~/.config/guts/config.toml`:

#### Navigation

- `h` / `Left Arrow`: Move left
- `l` / `Right Arrow`: Move right
- `k` / `Up Arrow`: Move up
- `j` / `Down Arrow`: Move down
- `g`: Jump to top
- `G`: Jump to bottom
- `PageUp`: Scroll up by page
- `PageDown`: Scroll down by page

#### Search and Filtering

- `/`: Enter search mode (incremental search)
- `n`: Jump to next search result
- `N`: Jump to previous search result
- `:`: Enter query/filter mode
- `Ctrl-f`: Open fuzzy search overlay
- `Tab`: Cycle fuzzy search scope (Columns → Rows → History)
- `Ctrl-p`: Previous query history (in query mode)
- `Ctrl-n`: Next query history (in query mode)

#### Actions

- `o`: Open selected URL or email
- `y`: Copy selected cell value to clipboard
- `q`: Quit Guts

#### Custom Keybindings

Edit `~/.config/guts/config.toml` to customize keybindings:

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

## Status

Guts is actively developed and maintains a stable feature set for local and remote data exploration workflows. The 1.0 release includes:

- CSV and JSON file support
- SQLite database integration
- PostgreSQL, MySQL, and MongoDB connectivity
- Full table rendering with navigation and search
- Virtual scrolling for large datasets
- Query execution and filtering
- Smart cell type detection
- Comprehensive theming system
- Configurable keybindings

## Contributing

We welcome contributions to Guts. Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to submit issues, propose features, and create pull requests.

## Support

For bug reports, feature requests, or questions, please open an issue on the [GitHub repository](https://github.com/andev0x/guts/issues).

## License

Guts is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contributors

We appreciate all contributions to this project. See the [contributor graph](https://github.com/andev0x/guts/graphs/contributors) for a list of everyone who has helped.
<a href="https://github.com/andev0x/guts/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=andev0x/guts" />
</a>

