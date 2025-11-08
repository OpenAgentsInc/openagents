#!/bin/bash

# Script to copy all file contents and paths to clipboard
# Usage: ./copy-all.sh

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Temporary file to collect output
TEMP_FILE=$(mktemp)

# Find all files (not directories) and process them
find "$SCRIPT_DIR" -type f ! -name "copy-all.sh" | sort | while read -r filepath; do
    # Get relative path from script directory
    relpath="${filepath#$SCRIPT_DIR/}"

    # Skip hidden files and .git
    if [[ "$relpath" == .* ]] || [[ "$relpath" == */.* ]]; then
        continue
    fi

    echo "=== $relpath ===" >> "$TEMP_FILE"
    echo "" >> "$TEMP_FILE"
    cat "$filepath" >> "$TEMP_FILE"
    echo "" >> "$TEMP_FILE"
    echo "" >> "$TEMP_FILE"
done

# Copy to clipboard
cat "$TEMP_FILE" | pbcopy

# Count files
FILE_COUNT=$(grep -c "^=== " "$TEMP_FILE")

# Clean up
rm "$TEMP_FILE"

echo "✅ Copied $FILE_COUNT files to clipboard"
echo "📋 Ready to paste"
