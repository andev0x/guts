# Contributing to guts

Thanks for your interest in contributing.

## Development Setup

1. Install stable Rust.
2. Clone the repository.
3. Build and run checks:

```bash
cd guts
cargo check
```

## Code Guidelines

- Keep modules focused and readable.
- Prefer small, composable functions.
- Avoid breaking keyboard-first workflows.
- Keep rendering and data logic decoupled.

## Pull Requests

- Use clear commit messages.
- Describe what changed and why.
- Include screenshots or terminal recordings for UI changes when possible.
- Ensure `cargo fmt` and `cargo check` pass.

## Issue Reports

For bug reports, include:

- Source type (CSV/JSON/SQLite)
- Reproduction steps
- Expected vs. actual behavior
- Platform details
