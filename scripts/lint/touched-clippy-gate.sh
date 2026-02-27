#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE_REF="${1:-origin/main}"
ALLOWLIST_FILE="${2:-$ROOT_DIR/scripts/lint/clippy-debt-allowlist.toml}"

if [[ ! -f "$ALLOWLIST_FILE" ]]; then
    printf 'Missing clippy debt allowlist: %s\n' "$ALLOWLIST_FILE" >&2
    exit 1
fi

if ! git -C "$ROOT_DIR" rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
    if git -C "$ROOT_DIR" rev-parse --verify HEAD~1 >/dev/null 2>&1; then
        BASE_REF="HEAD~1"
    else
        BASE_REF="HEAD"
    fi
fi

changed_tmp="$(mktemp)"
clippy_tmp="$(mktemp)"
cleanup() {
    rm -f "$changed_tmp" "$clippy_tmp"
}
trap cleanup EXIT

{
    git -C "$ROOT_DIR" diff --name-only "$BASE_REF"...HEAD -- '*.rs' || true
    git -C "$ROOT_DIR" diff --name-only -- '*.rs' || true
    git -C "$ROOT_DIR" diff --name-only --cached -- '*.rs' || true
} | sed '/^$/d' | sort -u >"$changed_tmp"

if [[ ! -s "$changed_tmp" ]]; then
    printf 'Touched-file clippy gate skipped: no changed Rust files.\n'
    exit 0
fi

cargo clippy --workspace --lib --bins --examples --message-format=json -- -W clippy::all >"$clippy_tmp"

python3 - "$ROOT_DIR" "$changed_tmp" "$clippy_tmp" "$ALLOWLIST_FILE" <<'PY'
import json
import pathlib
import re
import sys
from collections import defaultdict

root = pathlib.Path(sys.argv[1]).resolve()
changed_path = pathlib.Path(sys.argv[2])
clippy_path = pathlib.Path(sys.argv[3])
allowlist_path = pathlib.Path(sys.argv[4])

changed = {
    line.strip()
    for line in changed_path.read_text().splitlines()
    if line.strip()
}

allowlisted = set()
date_re = re.compile(r"^\d{4}-\d{2}-\d{2}$")
required_fields = {"owner", "added", "reason"}
timebound_fields = {"review_cadence", "expiry_issue", "expires_on"}
for raw in allowlist_path.read_text().splitlines():
    line = raw.strip()
    if not line or line.startswith("#"):
        continue
    fields = [field.strip() for field in line.split("|")]
    if len(fields) < 5:
        print(f"Invalid allowlist entry (expected 5+ fields): {raw}", file=sys.stderr)
        sys.exit(1)
    metadata = {}
    for field in fields[1:]:
        if ":" not in field:
            print(f"Invalid allowlist metadata field: {field}", file=sys.stderr)
            sys.exit(1)
        key, value = field.split(":", 1)
        key = key.strip()
        value = value.strip()
        if not key or not value:
            print(f"Invalid allowlist metadata field: {field}", file=sys.stderr)
            sys.exit(1)
        metadata[key] = value
    missing = sorted(required_fields - metadata.keys())
    if missing:
        print(
            f"Invalid allowlist entry (missing required metadata: {', '.join(missing)}): {raw}",
            file=sys.stderr,
        )
        sys.exit(1)
    if not date_re.match(metadata["added"]):
        print(
            f"Invalid allowlist entry (added must be YYYY-MM-DD): {raw}",
            file=sys.stderr,
        )
        sys.exit(1)
    if not any(key in metadata for key in timebound_fields):
        print(
            "Invalid allowlist entry (missing time-bound metadata: review_cadence, expiry_issue, or expires_on): "
            f"{raw}",
            file=sys.stderr,
        )
        sys.exit(1)
    allowlisted.add(fields[0])

warning_counts = defaultdict(int)
for raw in clippy_path.read_text().splitlines():
    raw = raw.strip()
    if not raw.startswith("{"):
        continue
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        continue
    message = payload.get("message", {})
    if message.get("level") != "warning":
        continue
    for span in message.get("spans", []):
        if not span.get("is_primary"):
            continue
        file_name = span.get("file_name")
        if not file_name:
            continue
        path = pathlib.Path(file_name)
        if not path.is_absolute():
            path = (root / path).resolve()
        else:
            path = path.resolve()
        try:
            rel = path.relative_to(root).as_posix()
        except ValueError:
            continue
        warning_counts[rel] += 1

violations = []
for rel in sorted(changed):
    count = warning_counts.get(rel, 0)
    if count == 0:
        continue
    if rel in allowlisted:
        continue
    violations.append((rel, count))

if violations:
    print("Touched-file clippy gate failed.", file=sys.stderr)
    print("Add debt entries to scripts/lint/clippy-debt-allowlist.toml with owner/date, or remove warnings from touched files.", file=sys.stderr)
    for rel, count in violations:
        print(f" - {rel}: {count} warning(s), not allowlisted", file=sys.stderr)
    sys.exit(1)

print("Touched-file clippy gate passed.")
for rel in sorted(changed):
    count = warning_counts.get(rel, 0)
    if count > 0:
        status = "allowlisted"
        print(f" - {rel}: {count} warning(s) [{status}]")
PY
