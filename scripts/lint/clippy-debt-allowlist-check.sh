#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ALLOWLIST_FILE="${1:-$ROOT_DIR/scripts/lint/clippy-debt-allowlist.toml}"

if [[ ! -f "$ALLOWLIST_FILE" ]]; then
    printf 'Missing clippy debt allowlist: %s\n' "$ALLOWLIST_FILE" >&2
    exit 1
fi

python3 - "$ALLOWLIST_FILE" <<'PY'
import pathlib
import re
import sys

allowlist_path = pathlib.Path(sys.argv[1])
required_fields = {"owner", "added", "reason"}
timebound_fields = {"review_cadence", "expiry_issue", "expires_on"}
date_re = re.compile(r"^\d{4}-\d{2}-\d{2}$")

entries = 0
errors = []

for line_no, raw in enumerate(allowlist_path.read_text().splitlines(), start=1):
    line = raw.strip()
    if not line or line.startswith("#"):
        continue

    entries += 1
    parts = [part.strip() for part in raw.split("|")]
    path = parts[0].strip()
    if not path:
        errors.append(f"{line_no}: empty path field")
        continue

    metadata = {}
    for field in parts[1:]:
        field = field.strip()
        if not field:
            continue
        if ":" not in field:
            errors.append(f"{line_no}: malformed metadata field `{field}`")
            continue
        key, value = field.split(":", 1)
        key = key.strip()
        value = value.strip()
        if not key or not value:
            errors.append(f"{line_no}: malformed metadata field `{field}`")
            continue
        metadata[key] = value

    missing = sorted(required_fields - metadata.keys())
    if missing:
        errors.append(f"{line_no}: missing required fields: {', '.join(missing)}")

    if metadata.get("added") and not date_re.match(metadata["added"]):
        errors.append(f"{line_no}: added must be YYYY-MM-DD, got `{metadata['added']}`")

    if not any(key in metadata for key in timebound_fields):
        errors.append(
            f"{line_no}: missing time-bound field (one of: review_cadence, expiry_issue, expires_on)"
        )

if errors:
    print("Clippy debt allowlist validation failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    sys.exit(1)

print(f"Clippy debt allowlist validation passed ({entries} entries).")
PY
