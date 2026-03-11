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
packages_tmp="$(mktemp)"
clippy_tmp="$(mktemp)"
cleanup() {
    rm -f "$changed_tmp" "$packages_tmp" "$clippy_tmp"
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

python3 - "$ROOT_DIR" "$changed_tmp" >"$packages_tmp" <<'PY'
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(sys.argv[1]).resolve()
changed = [
    line.strip()
    for line in pathlib.Path(sys.argv[2]).read_text().splitlines()
    if line.strip()
]

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=root,
        text=True,
    )
)
workspace_members = set(metadata["workspace_members"])
packages = []
for package in metadata["packages"]:
    if package["id"] not in workspace_members:
        continue
    packages.append(
        (
            pathlib.Path(package["manifest_path"]).resolve().parent,
            package["name"],
        )
    )

selected = set()
unresolved = []
for rel in changed:
    path = (root / rel).resolve()
    best_name = None
    best_length = -1
    for manifest_dir, package_name in packages:
        try:
            path.relative_to(manifest_dir)
        except ValueError:
            continue
        current_length = len(manifest_dir.as_posix())
        if current_length > best_length:
            best_name = package_name
            best_length = current_length
    if best_name is None:
        unresolved.append(rel)
    else:
        selected.add(best_name)

if unresolved:
    for rel in unresolved:
        print(rel, file=sys.stderr)
    sys.exit(1)

for package_name in sorted(selected):
    print(package_name)
PY

if [[ ! -s "$packages_tmp" ]]; then
    printf 'Touched-file clippy gate failed: unable to resolve changed Rust files to workspace packages.\n' >&2
    exit 1
fi

clippy_command=(
    cargo
    clippy
    --all-targets
    --no-deps
    --message-format=json
)
while IFS= read -r package_name; do
    clippy_command+=(-p "$package_name")
done <"$packages_tmp"
clippy_command+=(-- -W clippy::all)

"${clippy_command[@]}" >"$clippy_tmp"

python3 - "$ROOT_DIR" "$changed_tmp" "$clippy_tmp" "$ALLOWLIST_FILE" <<'PY'
import datetime as dt
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
issue_re = re.compile(r"^#\d+$")
policy_cutover = dt.date(2026, 2, 27)
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
    try:
        added_date = dt.date.fromisoformat(metadata["added"])
    except ValueError:
        print(
            f"Invalid allowlist entry (added is not a valid date): {raw}",
            file=sys.stderr,
        )
        sys.exit(1)
    expires_on_date = None
    if "expires_on" in metadata:
        if not date_re.match(metadata["expires_on"]):
            print(
                f"Invalid allowlist entry (expires_on must be YYYY-MM-DD): {raw}",
                file=sys.stderr,
            )
            sys.exit(1)
        try:
            expires_on_date = dt.date.fromisoformat(metadata["expires_on"])
        except ValueError:
            print(
                f"Invalid allowlist entry (expires_on is not a valid date): {raw}",
                file=sys.stderr,
            )
            sys.exit(1)
    if "expiry_issue" in metadata and not issue_re.match(metadata["expiry_issue"]):
        print(
            f"Invalid allowlist entry (expiry_issue must be issue-formatted '#123'): {raw}",
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
    if added_date >= policy_cutover and not (
        "expiry_issue" in metadata or "expires_on" in metadata
    ):
        print(
            "Invalid allowlist entry (new entries must include expiry_issue or expires_on): "
            f"{raw}",
            file=sys.stderr,
        )
        sys.exit(1)
    if expires_on_date and expires_on_date <= added_date:
        print(
            "Invalid allowlist entry (expires_on must be after added): "
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
