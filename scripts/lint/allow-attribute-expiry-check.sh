#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

BASE_REF="${1:-origin/main}"
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
    if git rev-parse --verify HEAD~1 >/dev/null 2>&1; then
        BASE_REF="HEAD~1"
    else
        BASE_REF="HEAD"
    fi
fi

if [[ "$BASE_REF" == "HEAD" ]]; then
    RANGE="HEAD"
else
    MERGE_BASE="$(git merge-base HEAD "$BASE_REF" 2>/dev/null || true)"
    if [[ -n "${MERGE_BASE:-}" ]]; then
        RANGE="${MERGE_BASE}..HEAD"
    else
        RANGE="${BASE_REF}..HEAD"
    fi
fi

DIFF_FILE="$(mktemp)"
cleanup() {
    rm -f "$DIFF_FILE"
}
trap cleanup EXIT

git diff --unified=0 "$RANGE" -- 'apps/autopilot-desktop/src/**/*.rs' 'crates/**/src/**/*.rs' >"$DIFF_FILE" || true

python3 - "$DIFF_FILE" <<'PY'
import pathlib
import re
import sys

diff_path = pathlib.Path(sys.argv[1])
text = diff_path.read_text()

allow_re = re.compile(r"^\+\s*#\s*\[\s*allow\s*\(", re.IGNORECASE)
reason_re = re.compile(r"reason\s*=\s*\"[^\"]*#\d+[^\"]*\"")

current_file = None
issues = []

for raw in text.splitlines():
    if raw.startswith("+++ b/"):
        current_file = raw[6:]
        continue

    if current_file is None:
        continue

    # Runtime path focus: skip explicit testing-only module trees.
    if current_file.startswith("crates/wgpui/src/testing/"):
        continue

    if raw.startswith("+") and not raw.startswith("+++") and allow_re.search(raw):
        if not reason_re.search(raw):
            issues.append((current_file, raw[1:].strip()))

if issues:
    print(
        "New #[allow(...)] attributes in runtime paths must include "
        "reason metadata with an expiry issue reference (e.g. reason = \"... #123\").",
        file=sys.stderr,
    )
    for file_name, line in issues:
        print(f" - {file_name}: {line}", file=sys.stderr)
    sys.exit(1)

print("Allow-attribute expiry check passed.")
PY
