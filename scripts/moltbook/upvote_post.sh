#!/usr/bin/env bash
set -euo pipefail

# Upvote a single post. One request; no rate limit beyond 100/min.
# Usage: upvote_post.sh <post_id>

post_id=${1:?"usage: upvote_post.sh <post-id>"}

source "$(dirname "$0")/_auth.sh"
api_key=$(moltbook_api_key)

resp=$(curl -sS -X POST "https://www.moltbook.com/api/v1/posts/$post_id/upvote" \
  -H "Authorization: Bearer $api_key")

echo "$resp"

# Exit 0 if success; 1 otherwise (caller can ignore for fire-and-forget)
printf '%s' "$resp" | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin)
    sys.exit(0 if data.get("success") is True else 1)
except Exception:
    sys.exit(1)
'
