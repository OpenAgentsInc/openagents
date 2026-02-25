#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
EXPECTED_PATH="${REPO_ROOT}/apps/openagents.com/openapi/openapi.json"
GENERATED_PATH="$(mktemp)"
trap 'rm -f "${GENERATED_PATH}"' EXIT

cargo run \
  --manifest-path "${REPO_ROOT}/apps/openagents.com/Cargo.toml" \
  --bin openagents-control-openapi-export > "${GENERATED_PATH}"

if [[ ! -f "${EXPECTED_PATH}" ]]; then
  echo "missing expected OpenAPI snapshot: apps/openagents.com/openapi/openapi.json"
  echo "run apps/openagents.com/scripts/generate-openapi-json.sh"
  exit 1
fi

if ! cmp -s "${EXPECTED_PATH}" "${GENERATED_PATH}"; then
  echo "OpenAPI snapshot drift detected."
  echo "run apps/openagents.com/scripts/generate-openapi-json.sh"
  diff -u "${EXPECTED_PATH}" "${GENERATED_PATH}" || true
  exit 1
fi

echo "OpenAPI snapshot is up to date."
