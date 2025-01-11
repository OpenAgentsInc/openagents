#!/bin/bash

# Check if tree command is available
if ! command -v tree &> /dev/null; then
    echo "Error: 'tree' command not found. Please install it first:"
    echo "  brew install tree"
    exit 1
fi

# Create docs directory if it doesn't exist
mkdir -p docs

# Common patterns to always ignore
COMMON_IGNORES=".git|.DS_Store|node_modules|target|.idea|.vscode"

# Function to process gitignore patterns into grep-compatible patterns
process_gitignore() {
    local patterns="$COMMON_IGNORES"
    
    if [ -f .gitignore ]; then
        while IFS= read -r pattern; do
            # Skip empty lines and comments
            [[ -z "$pattern" || "$pattern" =~ ^# ]] && continue
            
            # Convert .gitignore pattern to grep pattern
            pattern="${pattern#/}"  # Remove leading slash
            pattern="${pattern%/}"  # Remove trailing slash
            pattern="${pattern//./\\.}"  # Escape dots
            pattern="${pattern//\*/.*}"  # Convert * to .*
            
            # Add to patterns
            patterns="$patterns|$pattern"
        done < .gitignore
    fi
    
    echo "$patterns"
}

# Generate the tree output with common and gitignore patterns
IGNORE_PATTERN=$(process_gitignore)
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
    tree -I "$IGNORE_PATTERN" --dirsfirst -F --charset=ascii
    echo "\`\`\`"
} > docs/hierarchy.md

echo "Project hierarchy has been generated at docs/hierarchy.md"
