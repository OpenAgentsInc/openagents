#!/usr/bin/env bash
# Episode 239 "Let's Make Money" — one-command staging funded-loop smoke.
#
# Push-button wrapper around scripts/ep239-staging-smoke.mjs. Run it the moment
# the staging admin token is set; it verifies the whole Ep239 revenue loop on
# the ISOLATED staging Worker and prints a PASS/FAIL/SKIP receipt-bearing report.
#
#   apps/openagents.com/scripts/ep239-staging-smoke.sh
#
# The staging admin token is read from the environment only and is NEVER
# printed. If OPENAGENTS_ADMIN_API_TOKEN is unset, the funded-grant and
# metered-spend legs are SKIPPED (not failed) with the exact owner command.
#
# Pass extra flags straight through, e.g.:
#   apps/openagents.com/scripts/ep239-staging-smoke.sh --json
#   apps/openagents.com/scripts/ep239-staging-smoke.sh --base-url https://...
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNNER="${SCRIPT_DIR}/ep239-staging-smoke.mjs"

if command -v bun >/dev/null 2>&1; then
  exec bun "${RUNNER}" "$@"
elif command -v node >/dev/null 2>&1; then
  exec node "${RUNNER}" "$@"
else
  echo "ep239-staging-smoke: neither bun nor node found on PATH" >&2
  exit 2
fi
