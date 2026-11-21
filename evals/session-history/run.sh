#!/bin/sh
# Firm reason: 5 (thin wrapper with no logic)
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
HERE="$ROOT/evals/session-history"
CMD="${1:-help}"
if [ "$#" -gt 0 ]; then shift; fi

case "$CMD" in
  ingest)
    exec "$ROOT/coax/web/tool.sh" "$HERE/tools/ingest.coax" "$@"
    ;;
  mine)
    exec "$ROOT/coax/web/tool.sh" "$HERE/tools/mine.coax" "$@"
    ;;
  sanitize)
    exec "$ROOT/coax/web/tool.sh" "$HERE/tools/sanitize.coax" "$@"
    ;;
  compare)
    exec "$ROOT/coax/web/tool.sh" "$HERE/tools/compare.coax" "$@"
    ;;
  run)
    TS="$(date -u +%Y%m%dT%H%M%SZ)"
    SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo nogit)"
    RUN_DIR="$HERE/results/sh-${TS}-${SHA}"
    mkdir -p "$RUN_DIR"
    RC=0
    BROWSER=/usr/bin/true python3 "$ROOT/evals/openagents-desktop/tier2/run_tasks.py" \
      --tasks-dir "$HERE/tasks" --out "$RUN_DIR" "$@" || RC=$?
    "$ROOT/coax/web/tool.sh" "$HERE/tools/compare.coax" --tasks "$HERE/tasks" --run "$RUN_DIR"
    exit "$RC"
    ;;
  *)
    echo "Usage: $0 <ingest|mine|sanitize|run|compare> [args...]"
    exit 0
    ;;
esac
