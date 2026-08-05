#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PUBLIC_DIR="$APP_DIR/../start/public/work-demo"

if ! command -v trunk >/dev/null 2>&1; then
  echo "work_demo_web requires Trunk: cargo install trunk --locked" >&2
  exit 1
fi

cd "$APP_DIR"
env -u NO_COLOR trunk build --release --locked --public-url /work-demo/

mkdir -p "$PUBLIC_DIR"
cp dist/index.html "$PUBLIC_DIR/index.html"
cp dist/work_demo_web.js "$PUBLIC_DIR/work_demo_web.js"
cp dist/work_demo_web_bg.wasm "$PUBLIC_DIR/work_demo_web_bg.wasm"

echo "Work items demo assets staged in $PUBLIC_DIR"
