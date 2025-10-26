#!/usr/bin/env bash
set -euo pipefail

# Fetch/install the Convex local backend binary and place it at
# tauri/src-tauri/bin/local_backend (sidecar path for bundling).

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT_BIN="$ROOT_DIR/tauri/src-tauri/bin/local_backend"

echo "[fetch-convex] root=$ROOT_DIR"

# Ensure bun is available
if ! command -v bun >/dev/null 2>&1; then
  echo "[fetch-convex] error: bun is required (https://bun.sh)." >&2
  exit 1
fi

echo "[fetch-convex] requesting convex local backend via bunx convex dev --once --skip-push --local-force-upgrade"
(
  cd "$ROOT_DIR"
  bun x convex dev --once --skip-push --local-force-upgrade || true
)

# Find the newest cached local backend binary
CACHE_ROOT="$HOME/.cache/convex/binaries"
if [ ! -d "$CACHE_ROOT" ]; then
  echo "[fetch-convex] error: convex cache not found at $CACHE_ROOT" >&2
  exit 2
fi

LATEST_BIN=""
LATEST_MTIME=0
while IFS= read -r -d '' file; do
  mtime=$(stat -f %m "$file" 2>/dev/null || stat -c %Y "$file" 2>/dev/null || echo 0)
  if [ "$mtime" -gt "$LATEST_MTIME" ]; then
    LATEST_MTIME="$mtime"
    LATEST_BIN="$file"
  fi
done < <(find "$CACHE_ROOT" -type f \( -name "convex-local-backend" -o -name "local_backend" -o -name "convex-local-backend.exe" -o -name "local_backend.exe" \) -print0)

if [ -z "$LATEST_BIN" ]; then
  echo "[fetch-convex] error: could not find convex local backend in cache ($CACHE_ROOT)" >&2
  exit 3
fi

echo "[fetch-convex] found cached binary: $LATEST_BIN"
mkdir -p "$(dirname "$OUT_BIN")"
cp -f "$LATEST_BIN" "$OUT_BIN"
chmod +x "$OUT_BIN" || true
echo "[fetch-convex] installed -> $OUT_BIN"

exit 0

