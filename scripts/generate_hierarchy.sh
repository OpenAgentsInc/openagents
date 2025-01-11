#!/bin/bash

# Create docs directory if it doesn't exist
mkdir -p docs

# Function to process gitignore patterns into grep-compatible patterns
process_gitignore() {
    while IFS= read -r pattern; do
        # Skip empty lines and comments
        [[ -z "$pattern" || "$pattern" =~ ^# ]] && continue
        
        # Convert .gitignore pattern to grep pattern
        pattern="${pattern#/}"  # Remove leading slash
        pattern="${pattern%/}"  # Remove trailing slash
        pattern="${pattern//./\\.}"  # Escape dots
        pattern="${pattern//\*/.*}"  # Convert * to .*
        echo "$pattern"
    done < .gitignore
}

# Create a temporary file for grep patterns
TEMP_PATTERNS=$(mktemp)
process_gitignore > "$TEMP_PATTERNS"

# Generate the tree output, excluding gitignore patterns
{
    echo "# Project Hierarchy"
    echo
    echo "\`\`\`"
    tree -I "$(paste -sd'|' "$TEMP_PATTERNS")" --dirsfirst
    echo "\`\`\`"
} > docs/hierarchy.md

# Clean up
rm "$TEMP_PATTERNS"

echo "Project hierarchy has been generated at docs/hierarchy.md"
