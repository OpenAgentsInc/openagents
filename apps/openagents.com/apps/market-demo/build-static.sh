#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PUBLIC_DIR="$APP_DIR/../start/public/demo"

if ! command -v trunk >/dev/null 2>&1; then
  echo "market_demo_web requires Trunk: cargo install trunk --locked" >&2
  exit 1
fi

cd "$APP_DIR"
env -u NO_COLOR trunk build --release --locked --public-url /demo/

mkdir -p "$PUBLIC_DIR"
cp dist/index.html "$PUBLIC_DIR/index.html"
cp dist/market_demo_web.js "$PUBLIC_DIR/market_demo_web.js"
cp dist/market_demo_web_bg.wasm "$PUBLIC_DIR/market_demo_web_bg.wasm"

echo "Market demo assets staged in $PUBLIC_DIR"
