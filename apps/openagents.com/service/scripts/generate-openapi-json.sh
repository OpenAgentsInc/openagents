#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
OUTPUT_REL="${1:-apps/openagents.com/service/openapi/openapi.json}"
OUTPUT_PATH="${REPO_ROOT}/${OUTPUT_REL}"

mkdir -p "$(dirname "${OUTPUT_PATH}")"

cargo run \
  --manifest-path "${REPO_ROOT}/apps/openagents.com/service/Cargo.toml" \
  --bin openagents-control-openapi-export > "${OUTPUT_PATH}"

echo "wrote ${OUTPUT_REL}"
