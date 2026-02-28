#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DATE_STAMP="$(date +%F)"
DEFAULT_OUTPUT="$ROOT_DIR/docs/audits/snapshots/${DATE_STAMP}-architecture-audit.json"
OUTPUT_PATH="${1:-$DEFAULT_OUTPUT}"

mkdir -p "$(dirname "$OUTPUT_PATH")"
LOG_DIR="${OUTPUT_PATH}.logs"
mkdir -p "$LOG_DIR"

tmp_results="$(mktemp)"
tmp_metrics="$(mktemp)"
cleanup() {
    rm -f "$tmp_results" "$tmp_metrics"
}
trap cleanup EXIT

run_check() {
    local name="$1"
    local command="$2"
    local slug="$3"
    local log_path="$LOG_DIR/${slug}.log"
    local start_ts end_ts duration exit_code

    start_ts="$(date +%s)"
    set +e
    (
        cd "$ROOT_DIR"
        bash -lc "$command"
    ) >"$log_path" 2>&1
    exit_code=$?
    set -e
    end_ts="$(date +%s)"
    duration=$((end_ts - start_ts))

    printf '%s\t%s\t%s\t%s\t%s\n' \
        "$name" \
        "$command" \
        "$exit_code" \
        "$duration" \
        "$log_path" >>"$tmp_results"
}

run_check "fmt_check" "cargo fmt --all -- --check" "01-fmt-check"
run_check "workspace_check" "cargo check --workspace" "02-workspace-check"
run_check "workspace_test" "cargo test --workspace" "03-workspace-test"
run_check "ownership_boundary_check" "./scripts/lint/ownership-boundary-check.sh" "04-ownership-boundary"
run_check "workspace_dependency_drift_check" "./scripts/lint/workspace-dependency-drift-check.sh" "05-workspace-dependency-drift"
run_check "skills_registry_validate" "./scripts/skills/validate_registry.sh" "06-skills-registry-validate"
run_check "strict_production_hardening_check" "./scripts/lint/strict-production-hardening-check.sh" "07-strict-production-hardening"
run_check "clippy_regression_check" "./scripts/lint/clippy-regression-check.sh" "08-clippy-regression"
run_check "clippy_warning_budget_check" "./scripts/lint/clippy-warning-budget-check.sh" "09-clippy-warning-budget"

python3 - "$ROOT_DIR" >"$tmp_metrics" <<'PY'
import json
import pathlib

root = pathlib.Path(__import__("sys").argv[1]).resolve()
files = sorted(
    list((root / "apps").rglob("*.rs")) + list((root / "crates").rglob("*.rs"))
)
total_loc = 0
largest = []
for path in files:
    try:
        loc = sum(1 for _ in path.open("r", encoding="utf-8"))
    except UnicodeDecodeError:
        continue
    total_loc += loc
    largest.append((loc, path.relative_to(root).as_posix()))

largest.sort(reverse=True)
largest = largest[:10]

payload = {
    "rust_file_count": len(files),
    "rust_loc": total_loc,
    "largest_files": [{"path": path, "loc": loc} for loc, path in largest],
}
print(json.dumps(payload))
PY

python3 - "$ROOT_DIR" "$tmp_results" "$tmp_metrics" "$OUTPUT_PATH" <<'PY'
import datetime as dt
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1]).resolve()
results_path = pathlib.Path(sys.argv[2])
metrics_path = pathlib.Path(sys.argv[3])
output_path = pathlib.Path(sys.argv[4]).resolve()

checks = []
passed = 0
failed = 0

for raw in results_path.read_text().splitlines():
    if not raw.strip():
        continue
    name, command, exit_code, duration, log_path = raw.split("\t", 4)
    exit_code_int = int(exit_code)
    status = "pass" if exit_code_int == 0 else "fail"
    if status == "pass":
        passed += 1
    else:
        failed += 1
    log_path_resolved = pathlib.Path(log_path).resolve()
    try:
        log_path_value = log_path_resolved.relative_to(root).as_posix()
    except ValueError:
        log_path_value = log_path_resolved.as_posix()

    checks.append(
        {
            "name": name,
            "command": command,
            "status": status,
            "exit_code": exit_code_int,
            "duration_seconds": int(duration),
            "log_path": log_path_value,
        }
    )

metrics = json.loads(metrics_path.read_text())

payload = {
    "schema_version": 1,
    "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    "workspace_root": root.as_posix(),
    "checks": checks,
    "summary": {
        "total": len(checks),
        "passed": passed,
        "failed": failed,
        "status": "pass" if failed == 0 else "fail",
    },
    "metrics": metrics,
}

output_path.write_text(json.dumps(payload, indent=2) + "\n")
print(output_path.as_posix())
print(f"checks_total={len(checks)} passed={passed} failed={failed}")
PY

# Exit non-zero if any checks failed, but only after writing the snapshot.
if awk -F'\t' '$3 != "0" { found = 1 } END { exit(found ? 0 : 1) }' "$tmp_results"; then
    exit 1
fi
