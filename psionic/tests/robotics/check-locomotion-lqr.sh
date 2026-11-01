#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="${1:?usage: check-locomotion-lqr.sh FRESH_OUTPUT}"
exec "${PYTHON:-python3}" "$ROOT/psionic/tests/robotics/locomotion_lqr_native_reference.py" --coax "${COAX:-$ROOT/coax/bin/coax}" --output "$OUT"
