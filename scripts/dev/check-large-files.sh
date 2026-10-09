#!/usr/bin/env bash
# Refuse new or changed files over 1 MB unless allowlisted (#11110).
#
# Usage:
#   scripts/dev/check-large-files.sh            # staged changes (pre-commit)
#   scripts/dev/check-large-files.sh A..B       # every change in a commit range
#
# Allowlist: scripts/dev/large-files-allowlist.txt (shell globs, `*` matches
# `/`). Bench captures and large reports go to the bucket instead:
#   scripts/bench-artifacts.py push RUN_DIR
# Limit override: LARGE_FILE_LIMIT=<bytes>.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
limit="${LARGE_FILE_LIMIT:-1048576}"
allowlist="$root/scripts/dev/large-files-allowlist.txt"

if [[ $# -gt 0 ]]; then
  raw="$(git -C "$root" diff --raw --no-renames --diff-filter=AM "$1")"
else
  raw="$(git -C "$root" diff --cached --raw --no-renames --diff-filter=AM)"
fi
[[ -z "$raw" ]] && exit 0

patterns=()
if [[ -f "$allowlist" ]]; then
  while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    patterns+=("$line")
  done <"$allowlist"
fi

# ":old new old_sha new_sha status<TAB>path" -> "new_sha path"
pairs="$(printf '%s\n' "$raw" | awk -F'\t' '{split($1, f, " "); print f[4] "\t" $2}')"
sizes="$(printf '%s\n' "$pairs" | cut -f1 | git -C "$root" cat-file --batch-check='%(objectsize)')"

bad=0
while IFS=$'\t' read -r size path; do
  [[ "$size" =~ ^[0-9]+$ ]] || continue
  (( size > limit )) || continue
  allowed=0
  for pat in "${patterns[@]+"${patterns[@]}"}"; do
    # shellcheck disable=SC2053
    if [[ "$path" == $pat ]]; then allowed=1; break; fi
  done
  if (( allowed == 0 )); then
    printf 'large file: %s (%s bytes > %s)\n' "$path" "$size" "$limit" >&2
    bad=1
  fi
done < <(paste <(printf '%s\n' "$sizes") <(printf '%s\n' "$pairs" | cut -f2))

if (( bad )); then
  cat >&2 <<'EOF'

Files over 1 MB don't go in git. For bench run output (captures, records,
large reports), upload it and commit the manifest instead:
  scripts/bench-artifacts.py push RUN_DIR
Assets that must ship with the code go in scripts/dev/large-files-allowlist.txt.
EOF
  exit 1
fi
