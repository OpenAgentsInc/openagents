#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${APP_DIR}/dist"

"${APP_DIR}/build-dist.sh"

node --test "${APP_DIR}/host/update-policy.test.mjs"
node --test "${APP_DIR}/host/capability-policy.test.mjs"

python3 - <<'PY' "${DIST_DIR}/manifest.json" "${DIST_DIR}/sw.js" "${DIST_DIR}/assets/host-shim.js"
import json
import pathlib
import sys

manifest_path = pathlib.Path(sys.argv[1])
sw_path = pathlib.Path(sys.argv[2])
host_shim_path = pathlib.Path(sys.argv[3])

manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
sw_body = sw_path.read_text(encoding="utf-8")
host_shim = host_shim_path.read_text(encoding="utf-8")

assert manifest["manifestVersion"] == "openagents.webshell.v2"
build_id = manifest["buildId"]
assert isinstance(build_id, str) and build_id

compat = manifest.get("compatibility", {})
assert isinstance(compat.get("minClientBuildId"), str)
assert "syncSchemaMin" in compat and "syncSchemaMax" in compat

service_worker = manifest.get("serviceWorker", {})
assert service_worker.get("script") == "sw.js"
assert service_worker.get("cacheName") == f"openagents-web-shell::{build_id}"
assert isinstance(service_worker.get("pinnedAssets"), list) and service_worker["pinnedAssets"]

for asset in service_worker["pinnedAssets"]:
    assert asset in sw_body, f"missing pinned asset in sw.js: {asset}"

assert build_id in sw_body, "service worker build id mismatch"
assert build_id in host_shim, "host shim build id mismatch"
PY

echo "service worker policy verification passed"
