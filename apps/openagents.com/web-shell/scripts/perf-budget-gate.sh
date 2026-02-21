#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_SHELL_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Fast gate defaults for local CI and pre-release checks.
LATENCY_SAMPLES="${LATENCY_SAMPLES:-20}" \
AUTH_CHURN_SAMPLES="${AUTH_CHURN_SAMPLES:-10}" \
SOAK_SECONDS="${SOAK_SECONDS:-20}" \
FAIL_ON_BUDGET="${FAIL_ON_BUDGET:-1}" \
"${WEB_SHELL_DIR}/scripts/perf-soak-signoff.sh"
