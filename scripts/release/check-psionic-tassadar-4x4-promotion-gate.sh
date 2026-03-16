#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-tassadar-4x4-promotion-gate.sh [run_dir]

Loads the machine-readable Tassadar 4x4 promotion-gate report and exits
non-zero unless the learned 4x4 lane clears all required thresholds.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
run_dir="${1:-crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1}"
if [[ "${run_dir}" = /* ]]; then
  report_path="${run_dir}/promotion_gate_report.json"
else
  report_path="${repo_root}/${run_dir}/promotion_gate_report.json"
fi

python3 - "${report_path}" <<'PY'
import json
import pathlib
import sys

report_path = pathlib.Path(sys.argv[1])
report = json.loads(report_path.read_text())

required_first_target = 10_000
required_first_32_exclusive = 9_000
required_exact_traces = 1

failures = []
if report.get("first_target_exactness_bps", 0) < required_first_target:
    failures.append({
        "kind": "first_target_exactness_below_threshold",
        "actual": report.get("first_target_exactness_bps", 0),
        "required": required_first_target,
    })
if report.get("first_32_token_exactness_bps", 0) <= required_first_32_exclusive:
    failures.append({
        "kind": "first_32_token_exactness_below_threshold",
        "actual": report.get("first_32_token_exactness_bps", 0),
        "required": required_first_32_exclusive + 1,
    })
if report.get("exact_trace_case_count", 0) < required_exact_traces:
    failures.append({
        "kind": "exact_trace_count_below_threshold",
        "actual": report.get("exact_trace_case_count", 0),
        "required": required_exact_traces,
    })

summary = {
    "report_path": str(report_path),
    "run_id": report.get("run_id"),
    "checkpoint_id": report.get("checkpoint_id"),
    "passed": len(failures) == 0,
    "first_target_exactness_bps": report.get("first_target_exactness_bps"),
    "first_32_token_exactness_bps": report.get("first_32_token_exactness_bps"),
    "exact_trace_case_count": report.get("exact_trace_case_count"),
    "failures": failures,
}
print(json.dumps(summary, indent=2, sort_keys=True))
if failures:
    sys.exit(1)
PY
