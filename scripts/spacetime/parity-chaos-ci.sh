#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/output/spacetime/parity-chaos/ci-$(date -u +"%Y%m%dT%H%M%SZ")}" \
  "$ROOT_DIR/scripts/spacetime/parity-chaos-gate.sh" \
  --output-dir "$OUTPUT_DIR"

echo "Spacetime parity/chaos CI artifacts: $OUTPUT_DIR"
