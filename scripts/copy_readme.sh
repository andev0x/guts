#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
README_SOURCE="$ROOT_DIR/README.md"
README_DEST="$SCRIPT_DIR/../guts/README.md"

cp "$README_SOURCE" "$README_DEST"

echo "Copied README.md to guts/README.md"