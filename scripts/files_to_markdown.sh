#!/bin/bash

# Create temporary file to store output
temp_file=$(mktemp)

# Read filepaths from stdin or arguments
if [ $# -eq 0 ]; then
  # If no arguments provided, read from stdin
  while IFS= read -r filepath; do
    if [ -f "$filepath" ]; then
      echo -e "\n$filepath:\n\`\`\`" >> "$temp_file"
      cat "$filepath" >> "$temp_file"
      echo -e "\`\`\`" >> "$temp_file"
    else
      echo "Warning: File not found - $filepath" >&2
    fi
  done
else
  # Process command line arguments
  for filepath in "$@"; do
    if [ -f "$filepath" ]; then
      echo -e "\n$filepath:\n\`\`\`" >> "$temp_file"
      cat "$filepath" >> "$temp_file"
      echo -e "\`\`\`" >> "$temp_file"
    else
      echo "Warning: File not found - $filepath" >&2
    fi
  done
fi

# Copy to clipboard using pbcopy (macOS)
cat "$temp_file" | pbcopy

# Clean up
rm "$temp_file"

echo "File contents copied to clipboard in markdown format!" >&2
