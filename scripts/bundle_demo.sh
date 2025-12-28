#!/usr/bin/env bash
# DEPRECATED: Demo gallery moved to ~/code/backroom
# This script is no longer used. Demo and demos folders have been archived.

echo "ERROR: Demo gallery has been moved to ~/code/backroom"
echo "This script is deprecated and should not be run."
exit 1

# Bundle a .rlog session into a complete demo replay package
#
# Usage: ./bundle_demo.sh <path-to-rlog-file> [output-dir]
#
# Creates a .replay.tar.gz containing:
# - session.rlog (the main replay log)
# - metadata.json (session metrics, description)
# - changes.diff (git diff of changes made)
# - README.md (human-readable summary)
#
# Example:
#   ./bundle_demo.sh docs/logs/20251219/2138-start-working.rlog demos/

set -euo pipefail

RLOG_FILE="${1:?Usage: $0 <rlog-file> [output-dir]}"
OUTPUT_DIR="${2:-./demos}"

if [ ! -f "$RLOG_FILE" ]; then
    echo "Error: File not found: $RLOG_FILE"
    exit 1
fi

# Extract session info
BASENAME=$(basename "$RLOG_FILE" .rlog)
DATE_DIR=$(basename "$(dirname "$RLOG_FILE")")
SESSION_ID="${DATE_DIR}-${BASENAME}"
BUNDLE_NAME="${SESSION_ID}.replay"

# Create temp directory for bundle contents
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

echo "📦 Bundling demo: $SESSION_ID"
echo "   Source: $RLOG_FILE"
echo "   Output: $OUTPUT_DIR/$BUNDLE_NAME.tar.gz"

# Copy .rlog file
cp "$RLOG_FILE" "$TEMP_DIR/session.rlog"

# Extract metadata from .rlog
echo "📊 Extracting metadata..."
cat > "$TEMP_DIR/extract_metadata.py" << 'PYEOF'
import sys
import json
import re
from pathlib import Path

rlog_path = Path(sys.argv[1])
session_id = sys.argv[2]
date_dir = sys.argv[3]

metadata = {
    'session_id': session_id,
    'date': date_dir,
    'file_size_kb': round(rlog_path.stat().st_size / 1024, 1),
    'model': None,
    'tokens_in': 0,
    'tokens_out': 0,
    'tokens_cached': 0,
    'repo_sha': None,
    'branch': None,
    'tools_used': [],
}

with open(rlog_path, 'r') as f:
    in_header = False
    for line in f:
        if line.strip() == '---':
            if in_header:
                break
            in_header = True
            continue

        if in_header:
            if match := re.match(r'^(\w+):\s*(.+)', line):
                key, value = match.groups()
                if key in ['model', 'repo_sha', 'branch']:
                    metadata[key] = value.strip()
                elif key == 'tokens_total_in':
                    metadata['tokens_in'] = int(value)
                elif key == 'tokens_total_out':
                    metadata['tokens_out'] = int(value)
                elif key == 'tokens_cached':
                    metadata['tokens_cached'] = int(value)

    # Scan for tool usage
    f.seek(0)
    for line in f:
        if match := re.search(r't!:(\w+)', line):
            tool = match.group(1)
            if tool not in metadata['tools_used']:
                metadata['tools_used'].append(tool)

print(json.dumps(metadata, indent=2))
PYEOF

python3 "$TEMP_DIR/extract_metadata.py" "$RLOG_FILE" "$SESSION_ID" "$DATE_DIR" > "$TEMP_DIR/metadata.json"

# Try to extract git diff if commits were made
echo "📝 Extracting changes..."
REPO_SHA=$(grep "^repo_sha:" "$RLOG_FILE" | head -1 | cut -d' ' -f2)
if [ -n "$REPO_SHA" ]; then
    # Try to get diff from git history
    git show "$REPO_SHA" > "$TEMP_DIR/changes.diff" 2>/dev/null || echo "# No git diff available" > "$TEMP_DIR/changes.diff"
else
    echo "# No repo SHA found in session" > "$TEMP_DIR/changes.diff"
fi

# Generate README
echo "📄 Generating README..."
cat > "$TEMP_DIR/README.md" << 'README_EOF'
# Autopilot Demo Session

This bundle contains a complete autopilot session replay for demonstration purposes.

## Contents

- `session.rlog` - Complete session transcript in rlog format
- `metadata.json` - Session metrics and metadata
- `changes.diff` - Git diff of changes made during session
- `README.md` - This file

## Viewing the Replay

### Web Viewer

Upload `session.rlog` to the demo gallery viewer at:
https://openagents.com/demos/viewer

### CLI Replay (if available)

```bash
cargo run --bin replay-viewer session.rlog
```

## Session Details

See `metadata.json` for:
- Model used
- Token usage
- Tools invoked
- Files modified
- Completion status

## Quality Score

This session was selected based on:
- ✅ Successful completion
- ✅ Multiple tool usage (demonstrating capabilities)
- ✅ Real code changes
- ✅ Clear narrative flow
- ✅ Appropriate duration

## License

This replay log is provided for demonstration purposes as part of the OpenAgents project.
README_EOF

# Create tarball
mkdir -p "$OUTPUT_DIR"
TARBALL="$OUTPUT_DIR/$BUNDLE_NAME.tar.gz"

echo "🗜️  Creating tarball..."
tar -czf "$TARBALL" -C "$TEMP_DIR" .

SIZE=$(du -h "$TARBALL" | cut -f1)
echo "✅ Bundle created: $TARBALL ($SIZE)"

# Display bundle info
echo ""
echo "Bundle contents:"
tar -tzf "$TARBALL" | sed 's/^/  - /'

echo ""
echo "✨ Done!"
