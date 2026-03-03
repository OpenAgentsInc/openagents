#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

tmp_receipt="$(mktemp)"
CAST_SMOKE_RECEIPT_FILE="$tmp_receipt" "$ROOT_DIR/skills/cast/scripts/smoke-cast.sh"
rm -f "$tmp_receipt"
