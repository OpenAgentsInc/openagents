#!/bin/sh
# Cached metrics only. Explicit prepare/carry/brake acquisition is separate.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
COAX=${COAX:-"$ROOT/coax/bin/coax"}
if [ "$#" -gt 0 ]; then OUT=$1; else OUT=$(mktemp -d "${TMPDIR:-/tmp}/panda-scheduled-score.XXXXXX"); rmdir "$OUT"; fi
exec python3 "$ROOT/psionic/tests/robotics/panda_scheduled_pd_cached.py" --compiler "$COAX" --output "$OUT"
