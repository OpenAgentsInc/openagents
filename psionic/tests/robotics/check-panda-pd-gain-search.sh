#!/bin/sh
# Default gate is cached scoring only. No grid or SDK entry is invoked.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
COAX=${COAX:-"$ROOT/coax/bin/coax"}
if [ "$#" -gt 0 ]; then OUT=$1; else OUT=$(mktemp -d "${TMPDIR:-/tmp}/panda-gain-score.XXXXXX"); rmdir "$OUT"; fi
exec python3 "$ROOT/psionic/tests/robotics/panda_gain_search_cached.py" --compiler "$COAX" --output "$OUT"
