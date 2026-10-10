#!/usr/bin/env bash
# Keep the filefind indexes fresh after every pull or merge (issue #11210).
# The repository's .githooks/post-merge calls this. Runs in the background, one
# refresh at a time, and never fails the merge. Embeddings use OPENROUTER_API_KEY or
# ~/.openagents/openrouter.json; without either, history and the token indexes still
# refresh (queries also refresh history on their own).
set -u
root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
dir="${HOME}/.cache/openagents/filefind"
mkdir -p "$dir"
lock="$dir/post-merge.lock"
mkdir "$lock" 2>/dev/null || exit 0   # another refresh is running
(
  trap 'rmdir "$lock"' EXIT
  python3 "$root/scripts/filefind/filefind.py" index --repo "$root" --rev HEAD \
    && python3 "$root/scripts/filefind/filefind.py" feedback --repo "$root"
) >>"$dir/post-merge.log" 2>&1 &
exit 0
