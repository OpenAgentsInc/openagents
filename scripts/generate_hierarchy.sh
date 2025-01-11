#!/bin/bash

# Check if tree command is available
if ! command -v tree &> /dev/null; then
    echo "Error: 'tree' command not found. Please install it first:"
    echo "  brew install tree"
    exit 1
fi

# Create docs directory if it doesn't exist
mkdir -p docs

# Function to process gitignore patterns into grep-compatible patterns
process_gitignore() {
    if [ ! -f .gitignore ]; then
        echo "Warning: .gitignore file not found"
        return
    fi
    
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
if [ -s "$TEMP_PATTERNS" ]; then
    IGNORE_PATTERN=$(paste -sd'|' "$TEMP_PATTERNS")
    {
        echo "# Project Hierarchy"
        echo
        echo "Generated on: $(date '+%Y-%m-%d %H:%M:%S')"
        echo
        echo "### Legend"
        echo "- 📁 Directory"
        echo "- 📄 File"
        echo "- 🔒 Hidden file/directory"
        echo
        echo "### Structure"
        echo
        echo "\`\`\`"
        tree -a -I "$IGNORE_PATTERN" --dirsfirst -F --charset=ascii
        echo "\`\`\`"
        echo
        echo "### Summary"
        echo
        echo "\`\`\`"
        tree -a -I "$IGNORE_PATTERN" --dirsfirst -F --charset=ascii --summary
        echo "\`\`\`"
    } > docs/hierarchy.md
else
    {
        echo "# Project Hierarchy"
        echo
        echo "Generated on: $(date '+%Y-%m-%d %H:%M:%S')"
        echo
        echo "### Legend"
        echo "- 📁 Directory"
        echo "- 📄 File"
        echo "- 🔒 Hidden file/directory"
        echo
        echo "### Structure"
        echo
        echo "\`\`\`"
        tree -a --dirsfirst -F --charset=ascii
        echo "\`\`\`"
        echo
        echo "### Summary"
        echo
        echo "\`\`\`"
        tree -a --dirsfirst -F --charset=ascii --summary
        echo "\`\`\`"
    } > docs/hierarchy.md
fi

# Clean up
rm "$TEMP_PATTERNS"

echo "Project hierarchy has been generated at docs/hierarchy.md"
