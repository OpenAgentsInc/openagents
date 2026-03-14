#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-weather-agent-pilot.sh

Runs the canonical structured-agent weather pilot for PSI-258.
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

run_cmd cargo test -p psionic-serve --lib generic_server_weather_agent_pilot_is_end_to_end_machine_checkable -- --nocapture
run_cmd scripts/release/check-psionic-product-class-acceptance.sh --structured-agent-only

echo
echo "Psionic weather agent pilot passed."
