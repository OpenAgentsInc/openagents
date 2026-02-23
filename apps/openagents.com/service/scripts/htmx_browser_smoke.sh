#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
SCRIPT_PATH="${ROOT_DIR}/apps/openagents.com/service/scripts/htmx_browser_smoke.mjs"

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required to run HTMX browser smoke checks" >&2
  exit 1
fi

if ! (
  cd "${ROOT_DIR}/apps/openagents.com" && \
  node -e "import('@playwright/test').then(() => process.exit(0)).catch(() => process.exit(1))" >/dev/null 2>&1
); then
  echo "error: @playwright/test is not installed. run: npm install --prefix apps/openagents.com" >&2
  exit 1
fi

exec node "${SCRIPT_PATH}"
