#!/usr/bin/env bash
set -euo pipefail

# Copies the "Moltbook pack" (docs + scripts, excluding feed snapshots) to the system
# clipboard with file path dividers. Useful for sharing context with another model or
# pasting into an issue/doc.

repo_root=$(
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
)

tmp_file="$(mktemp)"
trap 'rm -f "$tmp_file"' EXIT

append_file() {
  local abs_path="$1"
  local rel_path="${abs_path#"$repo_root"/}"

  {
    printf "\n===== FILE: %s =====\n\n" "$rel_path"
    cat "$abs_path"
    printf "\n\n===== END FILE: %s =====\n" "$rel_path"
  } >>"$tmp_file"
}

# 1) Root-level Moltbook guidance.
if [[ -f "$repo_root/MOLTBOOK.md" ]]; then
  append_file "$repo_root/MOLTBOOK.md"
fi

# 2) All Moltbook docs/ops inputs except snapshots/log dumps (observations).
# We still include drafts/responses/queue/state since those are part of the operating pack.
find "$repo_root/docs/moltbook" -type f \
  ! -path "$repo_root/docs/moltbook/observations/*" \
  | LC_ALL=C sort \
  | while IFS= read -r f; do
      [[ -z "$f" ]] && continue
      append_file "$f"
    done

# 3) Moltbook automation scripts.
find "$repo_root/scripts/moltbook" -type f \
  | LC_ALL=C sort \
  | while IFS= read -r f; do
      [[ -z "$f" ]] && continue
      append_file "$f"
    done

# Copy to clipboard (macOS pbcopy; fall back to common linux tools).
if command -v pbcopy >/dev/null 2>&1; then
  pbcopy <"$tmp_file"
elif command -v wl-copy >/dev/null 2>&1; then
  wl-copy <"$tmp_file"
elif command -v xclip >/dev/null 2>&1; then
  xclip -selection clipboard <"$tmp_file"
elif command -v xsel >/dev/null 2>&1; then
  xsel --clipboard --input <"$tmp_file"
else
  echo "No clipboard tool found (pbcopy, wl-copy, xclip, xsel)." >&2
  exit 1
fi

bytes="$(wc -c <"$tmp_file" | tr -d ' ')"
echo "Copied Moltbook bundle to clipboard (${bytes} bytes)."

