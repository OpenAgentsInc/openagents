#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:?set OPENAGENTS_BASE_URL (for example https://openagents.com)}"
BASE_URL="${BASE_URL%/}"
ACCESS_TOKEN="${OPENAGENTS_CONTROL_ACCESS_TOKEN:-}"
MAINTENANCE_BYPASS_TOKEN="${OPENAGENTS_MAINTENANCE_BYPASS_TOKEN:-}"
COOKIE_JAR="$(mktemp -t openagents-smoke-cookie.XXXXXX)"
trap 'rm -f "${COOKIE_JAR}"' EXIT

require_header_contains() {
  local headers="$1"
  local needle="$2"
  if ! printf '%s\n' "${headers}" | tr -d '\r' | grep -qi "${needle}"; then
    echo "error: expected header pattern '${needle}' was not present" >&2
    exit 1
  fi
}

curl_with_session() {
  if [[ -s "${COOKIE_JAR}" ]]; then
    curl -fsS -b "${COOKIE_JAR}" "$@"
  else
    curl -fsS "$@"
  fi
}

curl_head_with_session() {
  if [[ -s "${COOKIE_JAR}" ]]; then
    curl -fsSI -b "${COOKIE_JAR}" "$@"
  else
    curl -fsSI "$@"
  fi
}

http_status_with_session() {
  if [[ -s "${COOKIE_JAR}" ]]; then
    curl -sS -o /dev/null -w '%{http_code}' -b "${COOKIE_JAR}" "$@"
  else
    curl -sS -o /dev/null -w '%{http_code}' "$@"
  fi
}

if [[ -n "${MAINTENANCE_BYPASS_TOKEN}" ]]; then
  echo "[smoke] maintenance bypass bootstrap"
  curl -fsS -c "${COOKIE_JAR}" \
    "${BASE_URL}/?maintenance_bypass=${MAINTENANCE_BYPASS_TOKEN}" >/dev/null
fi

echo "[smoke] readiness"
curl_with_session "${BASE_URL}/readyz" >/dev/null

echo "[smoke] health (best-effort)"
if ! curl_with_session "${BASE_URL}/healthz" >/dev/null 2>&1; then
  echo "[smoke] /healthz unavailable for ${BASE_URL}; continuing with /readyz as canonical probe"
fi

echo "[smoke] control status"
if [[ -n "${ACCESS_TOKEN}" ]]; then
  curl_with_session -H "authorization: Bearer ${ACCESS_TOKEN}" "${BASE_URL}/api/v1/control/status" >/dev/null
else
  control_status_code="$(http_status_with_session "${BASE_URL}/api/v1/control/status")"
  if [[ "${control_status_code}" != "200" && "${control_status_code}" != "401" ]]; then
    echo "error: expected /api/v1/control/status to return 200 or 401; got ${control_status_code}" >&2
    exit 1
  fi
fi

echo "[smoke] static manifest + cache policy"
manifest="$(curl_with_session "${BASE_URL}/manifest.json")"
printf '%s\n' "${manifest}" | grep -q '"manifestVersion"[[:space:]]*:[[:space:]]*"openagents.webshell.v2"'
manifest_headers="$(curl_head_with_session "${BASE_URL}/manifest.json")"
require_header_contains "${manifest_headers}" '^cache-control:.*no-cache'

sw_headers="$(curl_head_with_session "${BASE_URL}/sw.js")"
require_header_contains "${sw_headers}" '^cache-control:.*no-cache'

js_asset="$(printf '%s\n' "${manifest}" | sed -n 's/.*"js"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
if [[ -n "${js_asset}" ]]; then
  asset_headers="$(curl_head_with_session "${BASE_URL}/${js_asset}")"
  if [[ "${js_asset}" =~ [0-9a-fA-F]{8,} ]]; then
    require_header_contains "${asset_headers}" '^cache-control:.*immutable'
  else
    require_header_contains "${asset_headers}" '^cache-control:.*max-age='
  fi
fi

if [[ -n "${ACCESS_TOKEN}" ]]; then
  echo "[smoke] auth/session/token routes"
  curl_with_session -H "authorization: Bearer ${ACCESS_TOKEN}" "${BASE_URL}/api/auth/session" >/dev/null
  curl_with_session -H "authorization: Bearer ${ACCESS_TOKEN}" "${BASE_URL}/api/v1/auth/session" >/dev/null
  curl_with_session -X POST \
    -H "authorization: Bearer ${ACCESS_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"scopes":["runtime.codex_worker_events"]}' \
    "${BASE_URL}/api/sync/token" >/dev/null
  curl_with_session -X POST \
    -H "authorization: Bearer ${ACCESS_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"scopes":["runtime.codex_worker_events"]}' \
    "${BASE_URL}/api/v1/sync/token" >/dev/null
else
  echo "[smoke] OPENAGENTS_CONTROL_ACCESS_TOKEN is unset; skipping authenticated session/token checks"
fi

echo "ok: control-service smoke checks passed for ${BASE_URL}"
