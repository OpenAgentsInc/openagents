#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUDGET_FILE="${1:-$ROOT_DIR/scripts/lint/module-size-budgets.toml}"

if [[ ! -f "$BUDGET_FILE" ]]; then
    printf 'Missing module-size budget file: %s\n' "$BUDGET_FILE" >&2
    exit 1
fi

python3 - "$ROOT_DIR" "$BUDGET_FILE" <<'PY'
import datetime as dt
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1]).resolve()
budget_path = pathlib.Path(sys.argv[2]).resolve()

required_fields = {"max_lines", "owner", "added", "reason"}
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
    rel_path = parts[0].strip()
    if not rel_path:
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
        errors.append(f"{line_no}: missing required metadata fields: {', '.join(missing)}")
        continue

    try:
        max_lines = int(metadata["max_lines"])
    except ValueError:
        errors.append(
            f"{line_no}: max_lines must be integer, got `{metadata['max_lines']}`"
        )
        continue
    if max_lines <= 0:
        errors.append(f"{line_no}: max_lines must be > 0, got `{max_lines}`")
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

    if rel_path in entries:
        errors.append(f"{line_no}: duplicate budget entry for `{rel_path}`")
        continue

    entries[rel_path] = {
        "max_lines": max_lines,
        "line_no": line_no,
    }

if errors:
    print("Module-size budget file validation failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    sys.exit(1)

status = 0
print("Module-size budget check:")
for rel_path in sorted(entries):
    file_path = (root / rel_path).resolve()
    if not file_path.exists():
        print(f" - FAIL {rel_path}: file not found", file=sys.stderr)
        status = 1
        continue

    current = sum(1 for _ in file_path.open("r", encoding="utf-8"))
    max_lines = entries[rel_path]["max_lines"]
    if current > max_lines:
        status = 1
        print(
            f" - FAIL {rel_path}: max_lines={max_lines}, current={current} (over by {current - max_lines})",
            file=sys.stderr,
        )
        continue

    ratio = current / max_lines
    if ratio >= 0.9:
        print(
            f" - WARN {rel_path}: max_lines={max_lines}, current={current} ({ratio:.0%} of limit)"
        )
    else:
        print(f" - PASS {rel_path}: max_lines={max_lines}, current={current}")

if status != 0:
    print(
        "Module-size budget check failed. Decompose modules or raise budget with explicit rationale.",
        file=sys.stderr,
    )
    sys.exit(status)

print(f"Module-size budget check passed ({len(entries)} files).")
PY
