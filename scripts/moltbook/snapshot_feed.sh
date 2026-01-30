#!/usr/bin/env bash
set -euo pipefail

sort=${1:-new}
limit=${2:-25}

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
out_dir="$repo_root/crates/moltbook/docs/observations"
mkdir -p "$out_dir"

stamp=$(date -u +%Y%m%d-%H%M%S)
out_file="$out_dir/feed-$sort-$stamp.json"

source "$(dirname "$0")/_auth.sh"
api_key=$(moltbook_api_key)

# Reading endpoints appear to work without auth, but we include it when available.
curl -sS "https://www.moltbook.com/api/v1/posts?sort=$sort&limit=$limit" \
  -H "Authorization: Bearer $api_key" \
  > "$out_file"

printf '%s\n' "$out_file"
