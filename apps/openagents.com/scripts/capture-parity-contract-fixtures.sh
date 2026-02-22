#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/openagents.com"
OUTPUT_PATH="${1:-docs/parity-fixtures/baseline}"

cd "${APP_DIR}"
php artisan ops:capture-parity-contract-fixtures --output="${OUTPUT_PATH}"
