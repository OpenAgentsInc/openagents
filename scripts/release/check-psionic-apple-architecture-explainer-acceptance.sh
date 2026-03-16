#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-apple-architecture-explainer-acceptance.sh [-- <extra harness args>]

Runs the full Rust-only Psionic Apple acceptance harness for the architecture-
explainer lane. The harness executes:

- overfit-non-zero train/export/runtime-smoke/benchmark
- standard train/export/runtime-smoke/benchmark

and exits non-zero if either gate fails. The machine-readable acceptance receipt
is written to:

- $OPENAGENTS_APPLE_ACCEPTANCE_REPORT_PATH, if set
- otherwise a temporary JSON path printed by this script
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

run_cmd() {
  echo
  echo "==> $*"
  "$@"
}

report_path="${OPENAGENTS_APPLE_ACCEPTANCE_REPORT_PATH:-$(mktemp -t openagents-apple-acceptance-XXXXXX.json)}"
echo "Acceptance receipt: ${report_path}"

run_cmd cargo run -p autopilot-desktop --bin apple_architecture_explainer_acceptance_harness -- \
  --acceptance-report-path "${report_path}" \
  "$@"

echo
echo "Psionic Apple acceptance harness passed."
