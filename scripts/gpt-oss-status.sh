#!/bin/bash
#
# Show GPT-OSS llama-server + keepalive status and quick timing sample.
#
set -euo pipefail

PORT="${GPT_OSS_PORT:-8000}"
BASE_URL="${GPT_OSS_URL:-http://localhost:${PORT}}"
MODEL="${GPT_OSS_MODEL:-gpt-oss-20b}"
PID_FILE="${GPT_OSS_KEEPALIVE_PID_FILE:-/tmp/gpt-oss-keepalive.pid}"
PROMPT="${GPT_OSS_STATUS_PROMPT:-1+1=}"
MAX_TOKENS="${GPT_OSS_STATUS_MAX_TOKENS:-8}"

health=$(curl -4 -s "${BASE_URL}/health" || true)
if [[ -z "$health" ]]; then
    echo "health: unavailable"
else
    echo "health: $health"
fi

if [[ -f "$PID_FILE" ]]; then
    pid=$(cat "$PID_FILE" 2>/dev/null || true)
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
        echo "keepalive: running (pid $pid)"
    else
        echo "keepalive: stale pid file ($pid)"
    fi
else
    echo "keepalive: not running"
fi

if [[ "$health" == *'"status":"ok"'* || "$health" == *'"status": "ok"'* ]]; then
    timings=$(curl -4 -s "${BASE_URL}/v1/completions" \
        -H 'Content-Type: application/json' \
        -d "{\"model\":\"${MODEL}\",\"prompt\":\"${PROMPT}\",\"max_tokens\":${MAX_TOKENS},\"temperature\":0}" | \
        python3 -c 'import json,sys; resp=json.load(sys.stdin); print(resp.get("timings", {}))')
    echo "timings: $timings"
fi
