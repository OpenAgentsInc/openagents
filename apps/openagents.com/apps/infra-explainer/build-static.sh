#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PUBLIC_DIR="$APP_DIR/../start/public/infra"

if ! command -v trunk >/dev/null 2>&1; then
  echo "infra_explainer_web requires Trunk: cargo install trunk --locked" >&2
  exit 1
fi

cd "$APP_DIR"
env -u NO_COLOR trunk build --release --locked --public-url /infra/

mkdir -p "$PUBLIC_DIR"
cp dist/index.html "$PUBLIC_DIR/index.html"
cp dist/infra_explainer_web.js "$PUBLIC_DIR/infra_explainer_web.js"
cp dist/infra_explainer_web_bg.wasm "$PUBLIC_DIR/infra_explainer_web_bg.wasm"

echo "Infra explainer assets staged in $PUBLIC_DIR"
