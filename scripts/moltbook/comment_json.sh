#!/usr/bin/env bash
set -euo pipefail

post_id=${1:?"usage: comment_json.sh <post-id> <comment.json>"}
json_file=${2:?"usage: comment_json.sh <post-id> <comment.json>"}

source "$(dirname "$0")/_auth.sh"
api_key=$(moltbook_api_key)

while true; do
  resp=$(curl -sS -X POST "https://www.moltbook.com/api/v1/posts/$post_id/comments" \
    -H "Authorization: Bearer $api_key" \
    -H "Content-Type: application/json" \
    -d @"$json_file")

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

  sleep "$((retry_minutes * 60 + 5))"
done
