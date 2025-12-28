#!/usr/bin/env bash
# Copy all neobank docs to clipboard with filepaths
# Usage: ./scripts/copy-neobank-docs.sh

set -euo pipefail

NEOBANK_DIR="crates/neobank"

# Find all markdown files and README
{
    find "$NEOBANK_DIR" -type f \( -name "*.md" -o -name "README*" \) | sort | while read -r file; do
        echo "=== $file ==="
        echo ""
        cat "$file"
        echo ""
        echo ""
    done
} | pbcopy

echo "Copied all neobank docs to clipboard:"
find "$NEOBANK_DIR" -type f \( -name "*.md" -o -name "README*" \) | sort | while read -r file; do
    echo "  - $file"
done
