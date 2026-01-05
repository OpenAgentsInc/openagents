#!/bin/bash
#
# Stop the GPT-OSS keepalive loop started by scripts/gpt-oss-fast.sh.
#
set -euo pipefail

PID_FILE="${GPT_OSS_KEEPALIVE_PID_FILE:-/tmp/gpt-oss-keepalive.pid}"

if [[ ! -f "$PID_FILE" ]]; then
    echo "No keepalive PID file at $PID_FILE"
    exit 0
fi

pid=$(cat "$PID_FILE" 2>/dev/null || true)
if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" || true
    echo "Stopped keepalive (pid $pid)"
else
    echo "Keepalive not running (pid $pid)"
fi

rm -f "$PID_FILE"
