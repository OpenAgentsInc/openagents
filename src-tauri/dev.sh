#!/usr/bin/env bash
set -euo pipefail

# Simple guard to ensure Trunk dev server is available on the expected port.
# - If port 1420 is already used by a Trunk process, reuse it (do nothing).
# - If port 1420 is used by a non-Trunk process, try to terminate stale Trunk
#   instances on that port; if it remains occupied by something else, exit
#   with a clear message so Tauri fails fast instead of flapping.

PORT="${TRUNK_PORT:-1420}"
# Always run from repo root so Trunk sees Trunk.toml and index.html
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$ROOT_DIR"

# Ensure optional watch-ignore directories exist so Trunk can canonicalize them
# (Trunk 0.21.14 errors if an ignore path doesn't exist).
mkdir -p .codex-dev/master-tasks dist >/dev/null 2>&1 || true

# Preflight compile once to surface real errors early instead of a silent reuse.
echo "[dev.sh] Preflight UI build (trunk build)" >&2
if ! trunk build >/dev/null 2>&1; then
  echo "[dev.sh] Trunk build failed. Showing verbose error:" >&2
  trunk build -v || exit 1
fi

is_trunk_pid() {
  local pid="$1"
  local cmd
  cmd=$(ps -p "$pid" -o comm= 2>/dev/null || true)
  if [[ "${cmd:-}" == *trunk* ]]; then
    return 0
  fi
  # Fallback to args if comm didn't include trunk
  cmd=$(ps -p "$pid" -o args= 2>/dev/null || true)
  [[ "${cmd:-}" == *trunk* ]]
}

in_use_pids() {
  # macOS has lsof by default; use it for a reliable PID list
  lsof -n -P -i TCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null || true
}

PIDS=( $(in_use_pids) )
if [[ ${#PIDS[@]} -gt 0 ]]; then
  # If it's already Trunk, reuse it silently
  reuse=true
  for p in "${PIDS[@]}"; do
    if ! is_trunk_pid "$p"; then
      reuse=false
      break
    fi
  done

  if [[ "$reuse" == true ]]; then
    echo "Reusing existing Trunk on http://localhost:${PORT}" >&2
    exit 0
  fi

  echo "Port ${PORT} is in use by PID(s): ${PIDS[*]} (not Trunk)." >&2
  echo "If this is a stale dev server, attempting to free the port..." >&2

  # Try to terminate any Trunk processes on that port specifically.
  for p in "${PIDS[@]}"; do
    if is_trunk_pid "$p"; then
      kill -TERM "$p" 2>/dev/null || true
    fi
  done
  # Give it a moment to stop
  sleep 1
  PIDS=( $(in_use_pids) )
  if [[ ${#PIDS[@]} -gt 0 ]]; then
  echo "Port ${PORT} still in use by PID(s): ${PIDS[*]}." >&2
  echo "Please stop the process using ${PORT} and retry (Tauri expects ${PORT})." >&2
    exit 1
  fi
fi

exec trunk serve --port "$PORT"
