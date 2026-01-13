#!/bin/bash
# Copy ALL DSPy docs to clipboard with separators

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Count files
file_count=$(ls -1 "$SCRIPT_DIR"/*.md 2>/dev/null | wc -l)

{
    first=true
    for file in "$SCRIPT_DIR"/*.md; do
        if [ -f "$file" ]; then
            filename=$(basename "$file")
            if [ "$first" = true ]; then
                first=false
            else
                echo ""
                echo ""
            fi
            echo "---- $filename ----"
            echo ""
            cat "$file"
        fi
    done
} | pbcopy

echo "Copied $file_count docs to clipboard!"
wc -l "$SCRIPT_DIR"/*.md | tail -1
