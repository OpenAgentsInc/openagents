#!/bin/bash
# Start MechaCoder - the overnight autonomous coding agent
# Runs every 5 minutes to pick up and complete tasks from .openagents/tasks.jsonl

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OPENAGENTS_ROOT="$(dirname "$SCRIPT_DIR")"
PLIST_NAME="com.openagents.mechacoder"
PLIST_SRC="$SCRIPT_DIR/$PLIST_NAME.plist"
PLIST_DST="$HOME/Library/LaunchAgents/$PLIST_NAME.plist"

echo "=== MechaCoder Setup ==="
echo "OpenAgents root: $OPENAGENTS_ROOT"

# Create logs directory
mkdir -p "$OPENAGENTS_ROOT/logs"
echo "✓ Created logs directory"

# Check for API key
if [ -z "$OPENROUTER_API_KEY" ]; then
    echo "⚠ OPENROUTER_API_KEY not set in environment"
    echo "  Make sure it's available when the agent runs"
fi

# Stop existing if running
if launchctl list | grep -q "$PLIST_NAME"; then
    echo "Stopping existing agent..."
    launchctl unload "$PLIST_DST" 2>/dev/null || true
fi

# Copy and load plist
cp "$PLIST_SRC" "$PLIST_DST"
echo "✓ Installed plist to $PLIST_DST"

# Add API key to plist if available
if [ -n "$OPENROUTER_API_KEY" ]; then
    # Insert API key into plist
    /usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:OPENROUTER_API_KEY string $OPENROUTER_API_KEY" "$PLIST_DST" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c "Set :EnvironmentVariables:OPENROUTER_API_KEY $OPENROUTER_API_KEY" "$PLIST_DST"
    echo "✓ Added API key to plist"
fi

# Load the agent
launchctl load "$PLIST_DST"
echo "✓ Loaded launchd agent"

echo ""
echo "=== MechaCoder Started ==="
echo "• Running every 5 minutes"
echo "• Working on: /Users/christopherdavid/code/nostr-effect"
echo "• Logs: $OPENAGENTS_ROOT/docs/logs/YYYYMMDD/"
echo "• Stdout: $OPENAGENTS_ROOT/logs/mechacoder-stdout.log"
echo "• Stderr: $OPENAGENTS_ROOT/logs/mechacoder-stderr.log"
echo ""
echo "To stop: launchctl unload ~/Library/LaunchAgents/$PLIST_NAME.plist"
echo "To check: launchctl list | grep mechacoder"
