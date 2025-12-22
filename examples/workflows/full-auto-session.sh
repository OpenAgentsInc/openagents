#!/bin/bash
# Full Auto Autopilot Session Script
# This script demonstrates running autopilot in full-auto mode with proper setup

set -e  # Exit on error

# Configuration
PROJECT_NAME="${PROJECT_NAME:-openagents}"
MAX_BUDGET="${MAX_BUDGET:-10.0}"
MODEL="${MODEL:-sonnet}"
WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "=== OpenAgents Autopilot Full-Auto Session ==="
echo "Project: $PROJECT_NAME"
echo "Budget: \$$MAX_BUDGET"
echo "Model: $MODEL"
echo "Workspace: $WORKSPACE_ROOT"
echo

# Check prerequisites
echo "Checking prerequisites..."

if [ -z "$ANTHROPIC_API_KEY" ]; then
    echo "ERROR: ANTHROPIC_API_KEY not set"
    echo "Please set it with: export ANTHROPIC_API_KEY=your-key-here"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "ERROR: cargo not found"
    exit 1
fi

echo "✓ Prerequisites OK"
echo

# Change to workspace root
cd "$WORKSPACE_ROOT"

# Check for pending issues
echo "Checking for ready issues..."
READY_COUNT=$(cargo run --bin openagents -- autopilot issue list --status ready 2>/dev/null | grep -c "ready" || echo "0")
echo "Found $READY_COUNT ready issues"
echo

if [ "$READY_COUNT" -eq 0 ]; then
    echo "No ready issues found. Creating sample issues..."

    cargo run --bin openagents -- autopilot issue create \
        "Fix any clippy warnings in workspace" \
        --priority medium

    cargo run --bin openagents -- autopilot issue create \
        "Ensure all public functions have documentation" \
        --priority low

    echo "Created sample issues. Check with: cargo run --bin openagents -- autopilot issue list"
    echo
fi

# Show current git status
echo "Git status:"
git status --short
echo

# Confirm before running
read -p "Start full-auto session? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

# Run autopilot in full-auto mode
echo "Starting autopilot full-auto mode..."
echo "This will continuously process issues until the queue is empty or budget is exhausted."
echo "Press Ctrl+C to stop gracefully."
echo

cargo run --bin openagents -- autopilot run \
    --full-auto \
    --model "$MODEL" \
    --max-budget "$MAX_BUDGET" \
    "Process all ready issues"

# Session complete
echo
echo "=== Session Complete ==="
echo

# Show metrics
echo "Session metrics:"
LATEST_LOG=$(find docs/logs -name "*.json" -type f | sort -r | head -1)
if [ -n "$LATEST_LOG" ]; then
    echo "Log file: $LATEST_LOG"
    cargo run --bin openagents -- autopilot metrics "$LATEST_LOG"
else
    echo "No log file found"
fi

echo
echo "View detailed logs in: docs/logs/$(date +%Y%m%d)/"
