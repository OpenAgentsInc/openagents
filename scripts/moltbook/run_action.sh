#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
action_json=$(cat)
moltbook_docs="$repo_root/crates/moltbook/docs"
responded_file="$moltbook_docs/state/responded_post_ids.txt"

mkdir -p "$(dirname "$responded_file")"
touch "$responded_file"

type=$(ACTION_JSON="$action_json" python3 -c 'import json, os; print(json.loads(os.environ.get("ACTION_JSON","{}")).get("type",""))')

case "$type" in
  post)
    file=$(ACTION_JSON="$action_json" python3 -c 'import json, os; print(json.loads(os.environ.get("ACTION_JSON","{}")).get("file",""))')
    if [[ -z "$file" ]]; then
      echo "missing field: file" >&2
      exit 2
    fi
    "$repo_root/scripts/moltbook/post_json.sh" "$repo_root/$file"
    ;;
  comment)
    post_id=$(ACTION_JSON="$action_json" python3 -c 'import json, os; print(json.loads(os.environ.get("ACTION_JSON","{}")).get("post_id",""))')
    file=$(ACTION_JSON="$action_json" python3 -c 'import json, os; print(json.loads(os.environ.get("ACTION_JSON","{}")).get("file",""))')
    if [[ -z "$post_id" || -z "$file" ]]; then
      echo "missing field: post_id or file" >&2
      exit 2
    fi

    # If we've already responded to this post, treat it as a no-op so the worker can
    # keep moving and we don't spam duplicate comments.
    if rg -q "^${post_id}$" "$responded_file"; then
      echo "already responded: $post_id" >&2
      exit 0
    fi

    "$repo_root/scripts/moltbook/comment_json.sh" "$post_id" "$repo_root/$file"
    # Best-effort dedupe marker so we don't keep replying to the same post across runs.
    if ! rg -q "^${post_id}$" "$responded_file"; then
      echo "$post_id" >> "$responded_file"
    fi
    ;;
  snapshot)
    sort=$(ACTION_JSON="$action_json" python3 -c 'import json, os; print(json.loads(os.environ.get("ACTION_JSON","{}")).get("sort","new"))')
    limit=$(ACTION_JSON="$action_json" python3 -c 'import json, os; print(json.loads(os.environ.get("ACTION_JSON","{}")).get("limit",25))')
    "$repo_root/scripts/moltbook/snapshot_feed.sh" "$sort" "$limit" >/dev/null
    ;;
  *)
    echo "unknown action type: $type" >&2
    exit 2
    ;;
esac
