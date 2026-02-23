#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/output/vignettes/phase0/${TIMESTAMP}}"

mkdir -p "${OUTPUT_DIR}"

echo "[vignette-phase0] output: ${OUTPUT_DIR}"

cargo run --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" \
  --bin vignette-phase0-issue-to-pr \
  -- \
  --output-dir "${OUTPUT_DIR}"

echo "[vignette-phase0] PASS"

