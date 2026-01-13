#!/bin/bash
# Copy ALL ADR docs to clipboard with separators

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Count files
file_count=$(ls -1 "$SCRIPT_DIR"/*.md 2>/dev/null | wc -l | tr -d ' ')

{
    first=true

    # ADR docs (INDEX first, then README, TEMPLATE, then ADRs in order)
    for file in "$SCRIPT_DIR/INDEX.md" "$SCRIPT_DIR/README.md" "$SCRIPT_DIR/TEMPLATE.md"; do
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

    # ADR files in numeric order
    for file in "$SCRIPT_DIR"/ADR-*.md; do
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

echo "Copied $file_count ADR docs to clipboard!"
wc -l "$SCRIPT_DIR"/*.md 2>/dev/null | tail -1
