#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
QUALIFIED="${1:?usage: check-locomotion-lqr-longer-learn.sh QUALIFIED_EXTENSION PERSISTENT_OUTPUT}"
OUT="${2:?usage: check-locomotion-lqr-longer-learn.sh QUALIFIED_EXTENSION PERSISTENT_OUTPUT}"
if [[ ! -f "$OUT/prepared.json" ]]; then
  "${PYTHON:-python3}" "$ROOT/psionic/tests/robotics/locomotion_lqr_longer_learn.py" --prepare --qualified "$QUALIFIED" --output "$OUT"
fi
# One fixed bounded action only, from the authenticated original128 checkpoints.
# Repeated explicit opt-in advances the81 actions; FAILED/RUNNING never resumes.
exec "${PYTHON:-python3}" "$ROOT/psionic/tests/robotics/locomotion_lqr_longer_learn.py" --run-prepared "$OUT" --qualified "$QUALIFIED"
