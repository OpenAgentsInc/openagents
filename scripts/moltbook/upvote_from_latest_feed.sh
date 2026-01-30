#!/usr/bin/env bash
set -euo pipefail

# Upvote posts from the latest feed snapshot that we have not yet upvoted.
# Usage: upvote_from_latest_feed.sh [max_count]
# Default max_count=10. Spacing ~2s between upvotes to stay under 100/min.

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
moltbook_docs="$repo_root/crates/moltbook/docs"
observations="$moltbook_docs/observations"
state_dir="$moltbook_docs/state"
upvoted_file="$state_dir/upvoted_post_ids.txt"
max_count=${1:-10}

mkdir -p "$state_dir"
touch "$upvoted_file"

# Latest feed-new-*.json (by name = by time)
latest_feed=$(ls -1 "$observations"/feed-new-*.json 2>/dev/null | tail -1)
if [[ -z "$latest_feed" || ! -f "$latest_feed" ]]; then
  echo "[upvote_from_latest_feed] no feed snapshot found" >&2
  exit 0
fi

# Post ids from feed (array of .id)
ids=$(python3 -c "
import json
with open('$latest_feed', 'r') as f:
    data = json.load(f)
posts = data.get('posts') or data.get('data') or []
for p in posts:
    if isinstance(p, dict) and p.get('id'):
        print(p['id'])
" 2>/dev/null || true)

if [[ -z "$ids" ]]; then
  echo "[upvote_from_latest_feed] no post ids in feed" >&2
  exit 0
fi

count=0
for post_id in $ids; do
  [[ $count -ge $max_count ]] && break
  if grep -qFx "$post_id" "$upvoted_file" 2>/dev/null; then
    continue
  fi
  if "$repo_root/scripts/moltbook/upvote_post.sh" "$post_id" >> "$moltbook_docs/observations/worker.log" 2>&1; then
    echo "$post_id" >> "$upvoted_file"
    count=$((count + 1))
    echo "[upvote_from_latest_feed] upvoted $post_id" >&2
  fi
  sleep 2
done

echo "[upvote_from_latest_feed] done ($count upvotes)" >&2
