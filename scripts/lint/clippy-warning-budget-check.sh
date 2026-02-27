#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUDGET_FILE="${1:-$ROOT_DIR/scripts/lint/clippy-warning-budgets.toml}"

if [[ ! -f "$BUDGET_FILE" ]]; then
    printf 'Missing clippy warning budget file: %s\n' "$BUDGET_FILE" >&2
    exit 1
fi

clippy_tmp="$(mktemp)"
cleanup() {
    rm -f "$clippy_tmp"
}
trap cleanup EXIT

cargo clippy --workspace --lib --bins --examples --message-format=json -- -W clippy::all >"$clippy_tmp"

python3 - "$ROOT_DIR" "$clippy_tmp" "$BUDGET_FILE" <<'PY'
import datetime as dt
import json
import pathlib
import re
import sys
from collections import defaultdict

root = pathlib.Path(sys.argv[1]).resolve()
clippy_path = pathlib.Path(sys.argv[2])
budget_path = pathlib.Path(sys.argv[3])

required_fields = {"budget", "owner", "added", "reason"}
timebound_fields = {"review_cadence", "expiry_issue", "expires_on"}
date_re = re.compile(r"^\d{4}-\d{2}-\d{2}$")
issue_re = re.compile(r"^#\d+$")
policy_cutover = dt.date(2026, 2, 27)

entries = {}
errors = []

for line_no, raw in enumerate(budget_path.read_text().splitlines(), start=1):
    line = raw.strip()
    if not line or line.startswith("#"):
        continue

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
        errors.append(
            f"{line_no}: missing required metadata fields: {', '.join(missing)}"
        )
        continue

    try:
        budget = int(metadata["budget"])
    except ValueError:
        errors.append(f"{line_no}: budget must be integer, got `{metadata['budget']}`")
        continue
    if budget < 0:
        errors.append(f"{line_no}: budget must be >= 0, got `{budget}`")
        continue

    if not date_re.match(metadata["added"]):
        errors.append(f"{line_no}: added must be YYYY-MM-DD, got `{metadata['added']}`")
        continue
    try:
        added_date = dt.date.fromisoformat(metadata["added"])
    except ValueError:
        errors.append(f"{line_no}: added is not a valid date: `{metadata['added']}`")
        continue

    expires_on_date = None
    if "expires_on" in metadata:
        if not date_re.match(metadata["expires_on"]):
            errors.append(
                f"{line_no}: expires_on must be YYYY-MM-DD, got `{metadata['expires_on']}`"
            )
            continue
        try:
            expires_on_date = dt.date.fromisoformat(metadata["expires_on"])
        except ValueError:
            errors.append(
                f"{line_no}: expires_on is not a valid date: `{metadata['expires_on']}`"
            )
            continue

    if "expiry_issue" in metadata and not issue_re.match(metadata["expiry_issue"]):
        errors.append(
            f"{line_no}: expiry_issue must be issue-formatted (`#123`), got `{metadata['expiry_issue']}`"
        )
        continue

    if not any(key in metadata for key in timebound_fields):
        errors.append(
            f"{line_no}: missing time-bound field (one of: review_cadence, expiry_issue, expires_on)"
        )
        continue

    has_expiry_bound = "expiry_issue" in metadata or "expires_on" in metadata
    if added_date >= policy_cutover and not has_expiry_bound:
        errors.append(
            f"{line_no}: entries added on/after {policy_cutover.isoformat()} require expiry_issue or expires_on"
        )
        continue

    if expires_on_date and expires_on_date <= added_date:
        errors.append(
            f"{line_no}: expires_on must be after added (added={added_date.isoformat()}, expires_on={expires_on_date.isoformat()})"
        )
        continue

    if path in entries:
        errors.append(f"{line_no}: duplicate budget entry for `{path}`")
        continue

    entries[path] = {
        "budget": budget,
        "line_no": line_no,
    }

if errors:
    print("Clippy warning budget file validation failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    sys.exit(1)

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

status = 0
print("Clippy warning budget check:")
for rel in sorted(entries):
    budget = entries[rel]["budget"]
    current = warning_counts.get(rel, 0)

    if current > budget:
        status = 1
        print(
            f" - FAIL {rel}: budget={budget}, current={current} (over by {current - budget})",
            file=sys.stderr,
        )
        continue

    if current < budget:
        status = 1
        print(
            f" - FAIL {rel}: budget={budget}, current={current} (stale budget, lower to {current})",
            file=sys.stderr,
        )
        continue

    print(f" - PASS {rel}: budget={budget}, current={current}")

if status != 0:
    print(
        "Clippy warning budget check failed. Keep budgets exact so warning totals trend downward.",
        file=sys.stderr,
    )
    sys.exit(status)

print(f"Clippy warning budget check passed ({len(entries)} files).")
PY
