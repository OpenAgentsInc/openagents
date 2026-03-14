#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-qwen-pilot.sh

Runs the canonical non-GPT-OSS Qwen pilot for PSI-257.
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

run_cmd cargo test -p psionic-serve --lib generate_case_builder_uses_real_qwen2_fixture -- --nocapture
run_cmd cargo test -p psionic-serve --lib cpu_gguf_service_executes_qwen_family -- --nocapture
run_cmd cargo test -p psionic-serve --lib generic_server_qwen_pilot_is_end_to_end_machine_checkable -- --nocapture

echo
echo "Psionic Qwen pilot passed."
