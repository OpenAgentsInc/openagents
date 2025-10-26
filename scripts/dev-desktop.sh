#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "[desktop-dev] Ensuring Convex sidecar binary is present..."
bash "$ROOT_DIR/scripts/fetch-convex-backend.sh"

echo "[desktop-dev] Starting Tauri dev (single terminal)"
cd "$ROOT_DIR/tauri"
cargo tauri dev

