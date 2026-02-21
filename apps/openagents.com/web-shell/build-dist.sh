#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="${SCRIPT_DIR}"
OUT_DIR="${APP_DIR}/dist"
ASSETS_DIR="${OUT_DIR}/assets"
HOST_SHIM_TEMPLATE="${APP_DIR}/host/host-shim.js"
CAPABILITY_POLICY_TEMPLATE="${APP_DIR}/host/capability-policy.js"
SW_TEMPLATE="${APP_DIR}/host/sw-template.js"

rm -rf "${OUT_DIR}"
mkdir -p "${ASSETS_DIR}"

if [[ ! -f "${HOST_SHIM_TEMPLATE}" ]]; then
  echo "error: host shim template not found at ${HOST_SHIM_TEMPLATE}" >&2
  exit 1
fi

if [[ ! -f "${CAPABILITY_POLICY_TEMPLATE}" ]]; then
  echo "error: capability policy template not found at ${CAPABILITY_POLICY_TEMPLATE}" >&2
  exit 1
fi

if [[ ! -f "${SW_TEMPLATE}" ]]; then
  echo "error: service worker template not found at ${SW_TEMPLATE}" >&2
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
MIN_CLIENT_BUILD_ID="${OA_MIN_CLIENT_BUILD_ID:-${BUILD_ID}}"
MAX_CLIENT_BUILD_ID="${OA_MAX_CLIENT_BUILD_ID:-}"
PROTOCOL_VERSION="${OA_PROTOCOL_VERSION:-khala.ws.v1}"
SYNC_SCHEMA_MIN="${OA_SYNC_SCHEMA_MIN:-1}"
SYNC_SCHEMA_MAX="${OA_SYNC_SCHEMA_MAX:-1}"
ROLLBACK_BUILD_IDS="${OA_ROLLBACK_BUILD_IDS:-}"
export ROLLBACK_BUILD_IDS

PINNED_ASSETS_JSON='["/index.html","/assets/openagents_web_shell.js","/assets/openagents_web_shell_bg.wasm","/assets/host-shim.js","/assets/capability-policy.js"]'

ROLLBACK_CACHE_NAMES_JSON="$(python3 - <<'PY'
import json
import os

prefix = "openagents-web-shell::"
raw = os.environ.get("ROLLBACK_BUILD_IDS", "")
ids = [item.strip() for item in raw.split(",") if item.strip()]
print(json.dumps([prefix + item for item in ids]))
PY
)"

python3 - <<'PY' "${HOST_SHIM_TEMPLATE}" "${ASSETS_DIR}/host-shim.js" "${BUILD_ID}"
import pathlib
import sys

template_path = pathlib.Path(sys.argv[1])
out_path = pathlib.Path(sys.argv[2])
build_id = sys.argv[3]

template = template_path.read_text(encoding="utf-8")
out_path.write_text(template.replace("__OA_BUILD_ID__", build_id), encoding="utf-8")
PY

cp "${CAPABILITY_POLICY_TEMPLATE}" "${ASSETS_DIR}/capability-policy.js"

python3 - <<'PY' "${SW_TEMPLATE}" "${OUT_DIR}/sw.js" "${BUILD_ID}" "${PINNED_ASSETS_JSON}" "${ROLLBACK_CACHE_NAMES_JSON}"
import pathlib
import sys

template_path = pathlib.Path(sys.argv[1])
out_path = pathlib.Path(sys.argv[2])
build_id = sys.argv[3]
pinned_assets = sys.argv[4]
rollback_cache_names = sys.argv[5]

template = template_path.read_text(encoding="utf-8")
rendered = (
    template.replace("__OA_BUILD_ID__", build_id)
    .replace("__OA_PINNED_ASSETS__", pinned_assets)
    .replace("__OA_ROLLBACK_CACHE_NAMES__", rollback_cache_names)
)
out_path.write_text(rendered, encoding="utf-8")
PY

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

python3 - <<'PY' \
  "${OUT_DIR}/manifest.json" \
  "${BUILD_ID}" \
  "${MIN_CLIENT_BUILD_ID}" \
  "${MAX_CLIENT_BUILD_ID}" \
  "${PROTOCOL_VERSION}" \
  "${SYNC_SCHEMA_MIN}" \
  "${SYNC_SCHEMA_MAX}" \
  "${PINNED_ASSETS_JSON}" \
  "${ROLLBACK_CACHE_NAMES_JSON}"
import json
import pathlib
import sys

manifest_path = pathlib.Path(sys.argv[1])
build_id = sys.argv[2]
min_client = sys.argv[3]
max_client = sys.argv[4] or None
protocol_version = sys.argv[5]
sync_schema_min = int(sys.argv[6])
sync_schema_max = int(sys.argv[7])
pinned_assets = json.loads(sys.argv[8])
rollback_cache_names = json.loads(sys.argv[9])

manifest = {
    "manifestVersion": "openagents.webshell.v2",
    "buildId": build_id,
    "entry": {
        "html": "index.html",
        "js": "assets/openagents_web_shell.js",
        "wasm": "assets/openagents_web_shell_bg.wasm",
        "hostShim": "assets/host-shim.js",
    },
    "compatibility": {
        "protocolVersion": protocol_version,
        "minClientBuildId": min_client,
        "maxClientBuildId": max_client,
        "syncSchemaMin": sync_schema_min,
        "syncSchemaMax": sync_schema_max,
    },
    "serviceWorker": {
        "script": "sw.js",
        "cacheName": f"openagents-web-shell::{build_id}",
        "pinnedAssets": pinned_assets,
        "rollbackCaches": rollback_cache_names,
    },
}

manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY

echo "[web-shell] dist output ready: ${OUT_DIR}"
