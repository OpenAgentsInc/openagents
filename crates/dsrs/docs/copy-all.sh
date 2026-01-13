#!/bin/bash
# Copy ALL DSPy docs + root-level docs to clipboard with separators

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$SCRIPT_DIR/../../.."

# Root-level docs to include
ROOT_DOCS=(
    "$ROOT_DIR/ROADMAP.md"
    "$ROOT_DIR/SYNTHESIS_EXECUTION.md"
)

# Count files
dsrs_count=$(ls -1 "$SCRIPT_DIR"/*.md 2>/dev/null | wc -l | tr -d ' ')
root_count=0
for doc in "${ROOT_DOCS[@]}"; do
    [ -f "$doc" ] && root_count=$((root_count + 1))
done
file_count=$((dsrs_count + root_count))

{
    first=true

    # Root-level docs first
    for file in "${ROOT_DOCS[@]}"; do
        if [ -f "$file" ]; then
            filename=$(basename "$file")
            if [ "$first" = true ]; then
                first=false
            else
                echo ""
                echo ""
            fi
            echo "---- $filename (root) ----"
            echo ""
            cat "$file"
        fi
    done

    # DSPy docs
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

echo "Copied $file_count docs to clipboard ($root_count root + $dsrs_count dsrs)!"
wc -l "$SCRIPT_DIR"/*.md "${ROOT_DOCS[@]}" 2>/dev/null | tail -1
