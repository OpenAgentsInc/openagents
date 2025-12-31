#!/usr/bin/env bash
# Copy all runtime docs to clipboard with filepaths
# Usage: ./scripts/copy-runtime-docs.sh

set -euo pipefail

RUNTIME_DIR="crates/runtime"

# Find all markdown files in docs/ and README
{
    # First the README
    if [[ -f "$RUNTIME_DIR/README.md" ]]; then
        echo "=== $RUNTIME_DIR/README.md ==="
        echo ""
        cat "$RUNTIME_DIR/README.md"
        echo ""
        echo ""
    fi

    # Then all docs
    find "$RUNTIME_DIR/docs" -type f -name "*.md" | sort | while read -r file; do
        echo "=== $file ==="
        echo ""
        cat "$file"
        echo ""
        echo ""
    done
} | pbcopy

echo "Copied all runtime docs to clipboard:"
if [[ -f "$RUNTIME_DIR/README.md" ]]; then
    echo "  - $RUNTIME_DIR/README.md"
fi
find "$RUNTIME_DIR/docs" -type f -name "*.md" | sort | while read -r file; do
    echo "  - $file"
done
