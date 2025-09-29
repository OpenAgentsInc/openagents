#!/usr/bin/env bash
set -euo pipefail

# Wrapper to run the Tauri dev server's frontend (Trunk) robustly
# from either repo root or src-tauri/.

if [[ -f ./src-tauri/dev.sh ]]; then
  exec bash ./src-tauri/dev.sh
fi

# Fallback (shouldn't happen in this repo): run Trunk directly.
echo "src-tauri/dev.sh not found; running plain 'trunk serve'" >&2
exec trunk serve

