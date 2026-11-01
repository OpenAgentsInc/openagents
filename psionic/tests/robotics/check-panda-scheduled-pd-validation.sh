#!/bin/sh
# Cached C/D comparison/reducers only. One-shot SDK/OWN workflows are separate.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
COAX=${COAX:-"$ROOT/coax/bin/coax"}
if [ "$#" -gt 0 ]; then OUT=$1; else OUT=$(mktemp -d "${TMPDIR:-/tmp}/panda-cd-cached.XXXXXX"); rmdir "$OUT"; fi
exec python3 "$ROOT/psionic/tests/robotics/panda_scheduled_pd_validation_cached.py" --compiler "$COAX" --output "$OUT"
