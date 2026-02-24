#!/usr/bin/env bash
# Copy contents of all .proto files under proto/ to clipboard, with filename header before each.
# Run from repo root. macOS: uses pbcopy.

set -e
REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel)}"
cd "$REPO_ROOT"

OUT=$(mktemp)
trap 'rm -f "$OUT"' EXIT
count=0

while IFS= read -r f; do
  echo "" >> "$OUT"
  echo "========== $f ==========" >> "$OUT"
  cat "$f" >> "$OUT"
  ((count++)) || true
done < <(find proto -name '*.proto' 2>/dev/null | sort)

cat "$OUT" | pbcopy
echo "Copied contents of $count .proto files to clipboard (filename header before each)."
