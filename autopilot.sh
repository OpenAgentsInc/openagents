#!/bin/bash
# OpenAgents Autopilot - Autonomous task runner
#
# Usage:
#   ./autopilot.sh "Your task description"
#   ./autopilot.sh --model opus "Complex task needing more intelligence"
#   ./autopilot.sh --max-turns 50 "Long running task"
#
# Environment:
#   AUTOPILOT_MODEL - Default model (sonnet, opus, haiku)
#   AUTOPILOT_BUDGET - Default max budget in USD
#   AUTOPILOT_FULL_AUTO - Enable full auto mode (1 or true)
#   AUTOPILOT_UI - Launch desktop UI (1 or true)
#
# Logs saved to: docs/logs/YYYYMMDD/

set -e

# Defaults
MODEL="${AUTOPILOT_MODEL:-sonnet}"
MAX_TURNS="${AUTOPILOT_MAX_TURNS:-200}"
MAX_BUDGET="${AUTOPILOT_BUDGET:-300.0}"
FULL_AUTO="${AUTOPILOT_FULL_AUTO:-}"
UI="${AUTOPILOT_UI:-}"

# Parse arguments
EXTRA_ARGS=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --model)
            MODEL="$2"
            shift 2
            ;;
        --max-turns)
            MAX_TURNS="$2"
            shift 2
            ;;
        --max-budget)
            MAX_BUDGET="$2"
            shift 2
            ;;
        --verbose|-v)
            EXTRA_ARGS="$EXTRA_ARGS --verbose"
            shift
            ;;
        --dry-run)
            EXTRA_ARGS="$EXTRA_ARGS --dry-run"
            shift
            ;;
        --full-auto)
            FULL_AUTO="1"
            shift
            ;;
        --ui)
            UI="1"
            shift
            ;;
        *)
            PROMPT="$1"
            shift
            ;;
    esac
done

if [ -z "$PROMPT" ]; then
    echo "Usage: ./autopilot.sh \"Your task description\""
    echo ""
    echo "Options:"
    echo "  --model MODEL      Model to use (sonnet, opus, haiku)"
    echo "  --max-turns N      Maximum turns (default: 200)"
    echo "  --max-budget USD   Maximum budget (default: 300.0)"
    echo "  --full-auto        Keep working on issues and discover new work"
    echo "  --ui               Launch desktop UI with live visualization"
    echo "  --verbose          Show all messages"
    echo "  --dry-run          Don't save logs"
    exit 1
fi

# Add full-auto flag if enabled
if [ -n "$FULL_AUTO" ]; then
    EXTRA_ARGS="$EXTRA_ARGS --full-auto"
fi

# Add ui flag if enabled
if [ -n "$UI" ]; then
    EXTRA_ARGS="$EXTRA_ARGS --ui"
fi

# Run autopilot
cargo autopilot run \
    --with-issues \
    --model "$MODEL" \
    --max-turns "$MAX_TURNS" \
    --max-budget "$MAX_BUDGET" \
    $EXTRA_ARGS \
    "$PROMPT"
