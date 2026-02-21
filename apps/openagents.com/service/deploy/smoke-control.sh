#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:?set OPENAGENTS_BASE_URL (for example https://openagents.com)}"
BASE_URL="${BASE_URL%/}"
ACCESS_TOKEN="${OPENAGENTS_CONTROL_ACCESS_TOKEN:-}"

require_header_contains() {
  local headers="$1"
  local needle="$2"
  if ! printf '%s\n' "${headers}" | tr -d '\r' | grep -qi "${needle}"; then
    echo "error: expected header pattern '${needle}' was not present" >&2
    exit 1
  fi
}

echo "[smoke] health"
curl -fsS "${BASE_URL}/healthz" >/dev/null
curl -fsS "${BASE_URL}/readyz" >/dev/null

echo "[smoke] control status"
curl -fsS "${BASE_URL}/api/v1/control/status" >/dev/null

echo "[smoke] static manifest + cache policy"
manifest="$(curl -fsS "${BASE_URL}/manifest.json")"
printf '%s\n' "${manifest}" | grep -q '"manifestVersion"[[:space:]]*:[[:space:]]*"openagents.webshell.v2"'
manifest_headers="$(curl -fsSI "${BASE_URL}/manifest.json")"
require_header_contains "${manifest_headers}" '^cache-control:.*no-cache'

sw_headers="$(curl -fsSI "${BASE_URL}/sw.js")"
require_header_contains "${sw_headers}" '^cache-control:.*no-cache'

js_asset="$(printf '%s\n' "${manifest}" | sed -n 's/.*"js"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
if [[ -n "${js_asset}" ]]; then
  asset_headers="$(curl -fsSI "${BASE_URL}/${js_asset}")"
  require_header_contains "${asset_headers}" '^cache-control:.*immutable'
fi

if [[ -n "${ACCESS_TOKEN}" ]]; then
  echo "[smoke] auth/session/token routes"
  curl -fsS -H "authorization: Bearer ${ACCESS_TOKEN}" "${BASE_URL}/api/auth/session" >/dev/null
  curl -fsS -H "authorization: Bearer ${ACCESS_TOKEN}" "${BASE_URL}/api/v1/auth/session" >/dev/null
  curl -fsS -X POST \
    -H "authorization: Bearer ${ACCESS_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"scopes":["runtime.codex_worker_events"]}' \
    "${BASE_URL}/api/sync/token" >/dev/null
  curl -fsS -X POST \
    -H "authorization: Bearer ${ACCESS_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"scopes":["runtime.codex_worker_events"]}' \
    "${BASE_URL}/api/v1/sync/token" >/dev/null
else
  echo "[smoke] OPENAGENTS_CONTROL_ACCESS_TOKEN is unset; skipping authenticated session/token checks"
fi

echo "ok: control-service smoke checks passed for ${BASE_URL}"
