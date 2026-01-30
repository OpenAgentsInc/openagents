#!/usr/bin/env bash
# Post a batch of comments now. Rate limit: 50 comments/hour → ~75s between comments.
# Usage: burst_comments.sh [list.json]
# If no list: posts the 10 OpenAgents KB comments (post_id + file) defined below.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
moltbook_docs="$repo_root/crates/moltbook/docs"
responded_file="$moltbook_docs/state/responded_post_ids.txt"
COMMENT_INTERVAL_SEC=75

mkdir -p "$(dirname "$responded_file")"
touch "$responded_file"

# Default: the 10 comments we queued (post_id and response file path)
default_list() {
  cat <<'LIST'
a67ebb95-6894-42d4-aa14-7834751e40e4 crates/moltbook/docs/responses/comment-opusmolty-capability-autonomy.json
4eafe066-aa12-43b8-9aa6-8e2808d4bca8 crates/moltbook/docs/responses/comment-mowthebot-toy-infrastructure.json
1548d621-2b3b-4b97-adb6-9f1bb2ffbe0c crates/moltbook/docs/responses/comment-alfred-claimed.json
7d2b9797-b193-42be-95bf-0a11b6e1d202 crates/moltbook/docs/responses/comment-yolo-70m-governance.json
5c9dc95e-4991-49d6-b173-d8715f8cc078 crates/moltbook/docs/responses/comment-api-directory-welcome.json
67a49495-c1a3-4266-9132-489791c2218a crates/moltbook/docs/responses/comment-vela-capability-independence.json
3657707c-4c65-43af-b139-eb176d6902a9 crates/moltbook/docs/responses/comment-happycapy-identity-continuity.json
49c817a3-76c9-48fd-835b-17d408c08cd2 crates/moltbook/docs/responses/comment-octo-secrets-unsigned-skills.json
3e5f0bd1-3b14-46a5-be90-dd41b72eda31 crates/moltbook/docs/responses/comment-jon-containment-checklist.json
a5f018d8-a342-4a6d-b7a6-bcceafdebcf1 crates/moltbook/docs/responses/comment-presumably-alive-48hr-test.json
LIST
}

posted=0
skipped=0
first=true

while read -r post_id file_path; do
  [[ -z "$post_id" || -z "$file_path" ]] && continue
  abs_file="$repo_root/$file_path"
  if [[ ! -f "$abs_file" ]]; then
    echo "skip $post_id (missing $file_path)" >&2
    ((skipped++)) || true
    continue
  fi
  if rg -q "^${post_id}$" "$responded_file" 2>/dev/null; then
    echo "skip $post_id (already responded)" >&2
    ((skipped++)) || true
    continue
  fi
  if [[ "$first" != true ]]; then
    echo "wait ${COMMENT_INTERVAL_SEC}s (50 comments/hour limit)..." >&2
    sleep "$COMMENT_INTERVAL_SEC"
  fi
  first=false
  echo "post comment on $post_id ($file_path)" >&2
  if "$repo_root/scripts/moltbook/comment_json.sh" "$post_id" "$abs_file"; then
    echo "$post_id" >> "$responded_file"
    ((posted++)) || true
  else
    echo "failed $post_id" >&2
  fi
done < <(if [[ -n "${1:-}" && -f "$1" ]]; then cat "$1"; else default_list; fi)

echo "posted=$posted skipped=$skipped" >&2
