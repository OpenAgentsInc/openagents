#!/usr/bin/env bash
# Keep the filefind indexes fresh after every pull or merge (issue #11210).
#   ln -s ../../scripts/filefind/post-merge-hook.sh .git/hooks/post-merge   # or call it from your hook
# Runs in the background and never fails the merge. Needs OPENROUTER_API_KEY for embeddings
# (without it, history and the token indexes still refresh; queries also refresh history).
set -u
root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
log="${HOME}/.cache/openagents/filefind/post-merge.log"
mkdir -p "$(dirname "$log")"
( python3 "$root/scripts/filefind/filefind.py" index --repo "$root" --rev HEAD \
    && python3 "$root/scripts/filefind/filefind.py" feedback --repo "$root" ) >>"$log" 2>&1 &
exit 0
