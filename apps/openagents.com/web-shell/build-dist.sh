#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="${SCRIPT_DIR}"
OUT_DIR="${APP_DIR}/dist"
ASSETS_DIR="${OUT_DIR}/assets"
HOST_SHIM="${APP_DIR}/host/host-shim.js"

rm -rf "${OUT_DIR}"
mkdir -p "${ASSETS_DIR}"

if [[ ! -f "${HOST_SHIM}" ]]; then
  echo "error: host shim not found at ${HOST_SHIM}" >&2
  exit 1
fi

echo "[web-shell] building wasm package"
wasm-pack build \
  --release \
  --target web \
  --out-dir "${ASSETS_DIR}" \
  --out-name openagents_web_shell \
  "${APP_DIR}"

BUILD_ID="$(date -u +%Y%m%dT%H%M%SZ)"

cat > "${OUT_DIR}/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OpenAgents Web Shell</title>
    <style>
      html, body {
        margin: 0;
        width: 100%;
        height: 100%;
        background: #080a10;
      }
    </style>
  </head>
  <body>
    <div id="openagents-web-shell-status"></div>
    <canvas id="openagents-web-shell-canvas"></canvas>
    <script type="module" src="./assets/host-shim.js"></script>
  </body>
</html>
HTML

cp "${HOST_SHIM}" "${ASSETS_DIR}/host-shim.js"

cat > "${OUT_DIR}/manifest.json" <<EOF
{
  "manifestVersion": "openagents.webshell.v1",
  "buildId": "${BUILD_ID}",
  "entry": {
    "html": "index.html",
    "js": "assets/openagents_web_shell.js",
    "wasm": "assets/openagents_web_shell_bg.wasm"
  }
}
EOF

echo "[web-shell] dist output ready: ${OUT_DIR}"
