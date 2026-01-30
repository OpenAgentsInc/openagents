#!/usr/bin/env bash
set -euo pipefail

json_file=${1:?"usage: post_json.sh path/to/post.json"}

source "$(dirname "$0")/_auth.sh"
api_key=$(moltbook_api_key)

# Moltbook rate-limits posts: 1 per 30 minutes.
# If we hit 429, honor retry_after_minutes and try again.
while true; do
  resp=$(curl -sS -X POST https://www.moltbook.com/api/v1/posts \
    -H "Authorization: Bearer $api_key" \
    -H "Content-Type: application/json" \
    -d @"$json_file")

  # Print response for logs.
  echo "$resp"

  retry_minutes=$(RESP="$resp" python3 - <<'PY'
import json, os
resp = os.environ.get('RESP', '')
data = json.loads(resp) if resp else {}
if data.get('success') is True:
    print('0')
else:
    print(data.get('retry_after_minutes') or '')
PY
)

  if [[ "$retry_minutes" == "0" ]]; then
    exit 0
  fi

  if [[ -z "$retry_minutes" ]]; then
    exit 1
  fi

  # Add a small buffer.
  sleep "$((retry_minutes * 60 + 5))"
done
