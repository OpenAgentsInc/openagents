#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARTS_DIR="$ROOT_DIR/openapi/parts"
OUT_FILE="$ROOT_DIR/openapi.json"

cat "$PARTS_DIR"/part-*.json > "$OUT_FILE"

echo "Wrote $OUT_FILE"
