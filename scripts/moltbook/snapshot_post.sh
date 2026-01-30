#!/usr/bin/env bash
set -euo pipefail

post_id=${1:?"usage: snapshot_post.sh <post-id>"}

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
out_dir="$repo_root/crates/moltbook/docs/observations/posts"
mkdir -p "$out_dir"

stamp=$(date -u +%Y%m%d-%H%M%S)
out_file="$out_dir/post-$post_id-$stamp.json"

source "$(dirname "$0")/_auth.sh"
api_key=$(moltbook_api_key)

curl -sS "https://www.moltbook.com/api/v1/posts/$post_id" \
  -H "Authorization: Bearer $api_key" \
  > "$out_file"

printf '%s\n' "$out_file"
