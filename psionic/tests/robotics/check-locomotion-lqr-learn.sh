#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
QUALIFIED="${1:?usage: check-locomotion-lqr-learn.sh QUALIFIED_MATH PERSISTENT_OUTPUT}"
OUT="${2:?usage: check-locomotion-lqr-learn.sh QUALIFIED_MATH PERSISTENT_OUTPUT}"
if [[ ! -f "$OUT/prepared.json" ]]; then
  "${PYTHON:-python3}" "$ROOT/psionic/tests/robotics/locomotion_lqr_learn.py" --prepare --qualified "$QUALIFIED" --output "$OUT"
fi
# One bounded action only. Repeat this explicit opt-in until the record is complete.
exec "${PYTHON:-python3}" "$ROOT/psionic/tests/robotics/locomotion_lqr_learn.py" --run-prepared "$OUT" --qualified "$QUALIFIED"
