#!/bin/bash
#
# Fast raw completion against llama-server.
# Usage:
#   scripts/gpt-oss-query.sh "1+1="
#
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <prompt>" >&2
    exit 1
fi

PROMPT="$1"
BASE_URL="${GPT_OSS_URL:-http://localhost:8000}"
MODEL="${GPT_OSS_MODEL:-gpt-oss-20b}"
MAX_TOKENS="${GPT_OSS_MAX_TOKENS:-8}"
TEMP="${GPT_OSS_TEMPERATURE:-0}"

payload=$(python3 - <<PY
import json
print(json.dumps({
    "model": "${MODEL}",
    "prompt": "${PROMPT}",
    "max_tokens": int("${MAX_TOKENS}"),
    "temperature": float("${TEMP}"),
}))
PY
)

curl -4 -s "${BASE_URL}/v1/completions" \
    -H 'Content-Type: application/json' \
    -d "$payload" | \
    python3 -c 'import json,sys; resp=json.load(sys.stdin); choices=resp.get("choices", []); print(choices[0].get("text", "") if choices else "")'
