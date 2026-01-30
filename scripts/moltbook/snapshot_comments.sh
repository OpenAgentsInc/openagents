#!/usr/bin/env bash
set -euo pipefail

post_id=${1:?"usage: snapshot_comments.sh <post-id> [sort] [limit]"}
sort=${2:-top}
limit=${3:-50}

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
out_dir="$repo_root/crates/moltbook/docs/observations/comments"
mkdir -p "$out_dir"

stamp=$(date -u +%Y%m%d-%H%M%S)
out_file="$out_dir/comments-$post_id-$sort-$stamp.json"

source "$(dirname "$0")/_auth.sh"
api_key=$(moltbook_api_key)

# Comments: we use GET /posts/<id> (single post) which includes comments in the
# response; we sort/limit locally. If GET /posts/<id>/comments works for you
# (see skill.md / Rust client), you could switch to it for paging.
tmp_file="$(mktemp)"
trap 'rm -f "$tmp_file"' EXIT

curl -sS "https://www.moltbook.com/api/v1/posts/$post_id" \
  -H "Authorization: Bearer $api_key" \
  > "$tmp_file"

jq --arg sort "$sort" --argjson limit "$limit" '
  def score: ((.upvotes // 0) - (.downvotes // 0));

  . as $root
  | {
      success: ($root.success // true),
      post: ($root.post // null),
      sort: $sort,
      limit: $limit,
      comments: (
        ($root.comments // [])
        | if $sort == "new" then
            sort_by(.created_at) | reverse
          elif $sort == "old" then
            sort_by(.created_at)
          elif $sort == "top" then
            sort_by([score, (.created_at // "")]) | reverse
          else
            .
          end
        | .[0:$limit]
      )
    }
' "$tmp_file" > "$out_file"

printf '%s\n' "$out_file"
