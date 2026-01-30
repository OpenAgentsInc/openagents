#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
action_json=$(cat)

type=$(printf '%s' "$action_json" | python3 - <<'PY'
import json, sys
print(json.load(sys.stdin).get('type',''))
PY
)

case "$type" in
  post)
    file=$(printf '%s' "$action_json" | python3 - <<'PY'
import json, sys
print(json.load(sys.stdin).get('file',''))
PY
)
    if [[ -z "$file" ]]; then
      echo "missing field: file" >&2
      exit 2
    fi
    "$repo_root/scripts/moltbook/post_json.sh" "$repo_root/$file"
    ;;
  comment)
    post_id=$(printf '%s' "$action_json" | python3 - <<'PY'
import json, sys
print(json.load(sys.stdin).get('post_id',''))
PY
)
    file=$(printf '%s' "$action_json" | python3 - <<'PY'
import json, sys
print(json.load(sys.stdin).get('file',''))
PY
)
    if [[ -z "$post_id" || -z "$file" ]]; then
      echo "missing field: post_id or file" >&2
      exit 2
    fi
    "$repo_root/scripts/moltbook/comment_json.sh" "$post_id" "$repo_root/$file"
    ;;
  snapshot)
    sort=$(printf '%s' "$action_json" | python3 - <<'PY'
import json, sys
print(json.load(sys.stdin).get('sort','new'))
PY
)
    limit=$(printf '%s' "$action_json" | python3 - <<'PY'
import json, sys
print(json.load(sys.stdin).get('limit',25))
PY
)
    "$repo_root/scripts/moltbook/snapshot_feed.sh" "$sort" "$limit" >/dev/null
    ;;
  *)
    echo "unknown action type: $type" >&2
    exit 2
    ;;
esac
