#!/bin/bash

# Script to find and move JetBrains Mono font from Desktop to project

FONT_DIR="$HOME/Desktop"
TARGET_DIR="$(dirname "$0")/public/fonts"

echo "Looking for JetBrains Mono font files on Desktop..."

# Find font files
FONT_FILES=$(find "$FONT_DIR" -type f \( -iname "*jetbrains*" -o -iname "*JetBrains*" \) \( -name "*.ttf" -o -name "*.otf" -o -name "*.woff" -o -name "*.woff2" \) 2>/dev/null)

if [ -z "$FONT_FILES" ]; then
    echo "No JetBrains Mono font files found on Desktop."
    echo "Please ensure the font files are on your Desktop and try again."
    exit 1
fi

echo "Found font files:"
echo "$FONT_FILES"
echo ""

# Create target directory
mkdir -p "$TARGET_DIR"

# Copy files
echo "Copying font files to $TARGET_DIR..."
for file in $FONT_FILES; do
    filename=$(basename "$file")
    echo "  Copying $filename..."
    cp "$file" "$TARGET_DIR/"
done

echo ""
echo "Done! Font files have been copied to public/fonts/"
echo "You may need to rename them to match the expected names:"
echo "  - JetBrainsMono-Regular.woff2 (or .woff, .ttf)"
echo "  - JetBrainsMono-Medium.woff2 (or .woff, .ttf)"
echo "  - JetBrainsMono-Bold.woff2 (or .woff, .ttf)"
