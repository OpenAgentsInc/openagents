#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

ARTIFACTS_DIR="${OPENAGENTS_LDK_PROOF_ARTIFACTS_DIR:-${REPO_ROOT}/target/ldk-local-proof/latest}"
NETWORK="${OPENAGENTS_LDK_PROOF_NETWORK:-regtest}"
AMOUNT_SATS="${OPENAGENTS_LDK_PROOF_AMOUNT_SATS:-2500}"

mkdir -p "${ARTIFACTS_DIR}"

cd "${REPO_ROOT}"

cargo run -p nexus-control --bin ldk-local-proof-harness -- \
  --artifacts-dir "${ARTIFACTS_DIR}" \
  --network "${NETWORK}" \
  --amount-sats "${AMOUNT_SATS}" \
  --check
