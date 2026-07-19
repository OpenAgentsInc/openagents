#!/usr/bin/env bash
set -euo pipefail

proxy_pid=""
cleanup() {
  if [[ -n "$proxy_pid" ]]; then
    kill "$proxy_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

if [[ -n "${OA_CODEX_CONTROL_TOKEN_SECRET:-}" ]]; then
  umask 077
  install -d -m 0700 /run/openagents-secrets
  gcloud secrets versions access latest \
    --secret "$OA_CODEX_CONTROL_TOKEN_SECRET" \
    > /run/openagents-secrets/control-token
  export OA_CODEX_CONTROL_TOKEN_FILE=/run/openagents-secrets/control-token
  unset OA_CODEX_CONTROL_TOKEN
fi

if [[ -n "${OA_MANAGED_SANDBOX_PROVIDER_BROKER_URL:-}" ]]; then
  /usr/local/bin/managed-sandbox-provider-proxy.py &
  proxy_pid="$!"
fi

exec /usr/local/bin/oa-codex-control "$@"
