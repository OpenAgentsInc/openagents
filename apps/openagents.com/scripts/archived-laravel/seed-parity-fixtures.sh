#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/openagents.com"
FIXTURE_PATH="${FIXTURE_PATH:-${APP_DIR}/docs/parity-fixtures/baseline/shared-seed-state.json}"
REPLACE_MODE="${REPLACE_MODE:-1}"

cd "${APP_DIR}"

if [[ "${REPLACE_MODE}" == "1" ]]; then
  php artisan ops:seed-parity-fixtures --fixture="${FIXTURE_PATH}" --replace
else
  php artisan ops:seed-parity-fixtures --fixture="${FIXTURE_PATH}"
fi

"${ROOT_DIR}/apps/openagents.com/service/scripts/seed-parity-fixtures.sh"

echo "[parity-seed] completed laravel + rust seeding from ${FIXTURE_PATH}"
