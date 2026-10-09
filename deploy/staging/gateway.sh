#!/bin/sh
# The staging gateway sidecar: the account service (GitHub sign-in,
# sessions, device sign-in, projects) and the inference gateway in one
# process, as scripts/dev/full-local.sh runs it, on 127.0.0.1:8791.
#
# State lives in the shared in-memory volume $STACK_STATE (/stack), so a
# new instance starts with an empty registry: staging accounts and
# sessions do not survive a restart. Secrets arrive as environment
# variables from Secret Manager and are written to owner-only files here;
# none is printed.
#
# Environment:
#   PUBLIC_ORIGIN               https://... the site's origin (the OAuth callback base)
#   GITHUB_OAUTH_JSON           the staging OAuth App's private file
#   INFERENCE_ADMIN_TOKEN       the /admin/inference bearer
#   VERTEX_SA_JSON              optional: a service-account key for Vertex
#   OPENROUTER_API_KEY, AI_GATEWAY_API_KEY, TYPESAFE_API_KEY: optional upstreams
set -eu
umask 077
state=${STACK_STATE:-/stack}
mkdir -p "$state/gateway/attempts" "$state/private"
chmod 700 "$state/gateway" "$state/private"

: "${PUBLIC_ORIGIN:?PUBLIC_ORIGIN is unset}"
: "${GITHUB_OAUTH_JSON:?GITHUB_OAUTH_JSON is unset}"
: "${INFERENCE_ADMIN_TOKEN:?INFERENCE_ADMIN_TOKEN is unset}"
printf '%s' "$GITHUB_OAUTH_JSON" > "$state/private/github-oauth.json"
unset GITHUB_OAUTH_JSON
if [ -n "${VERTEX_SA_JSON:-}" ]; then
    printf '%s' "$VERTEX_SA_JSON" > "$state/private/vertex.json"
    unset VERTEX_SA_JSON
    export GOOGLE_APPLICATION_CREDENTIALS="$state/private/vertex.json"
fi

# One registry: `house` is the service tenant the chat worker calls
# inference as (its key goes to $state/service.key for the worker
# sidecar); `signup` holds new accounts' personal workspaces.
if [ ! -s "$state/service.key" ]; then
    bootstrap_registry --registry "$state/gateway/registry" \
        --tenant house --also-tenant signup --door local-kev --model kev-latest \
        --signature "sha256:0000000000000000000000000000000000000000000000000000000000000000" \
        > "$state/service.key.tmp"
    mv "$state/service.key.tmp" "$state/service.key"
    chmod 644 "$state/service.key"
fi

cat > "$state/gateway.json" <<EOF
{
  "v": "openagents.gateway.v1",
  "listen": "0.0.0.0:8791",
  "registry": "$state/gateway/registry",
  "accounts": {
    "signup_tenant": "signup",
    "github": {
      "credentials": "$state/private/github-oauth.json",
      "redirect_url": "$PUBLIC_ORIGIN/auth/github/callback"
    }
  },
  "inference": {
    "admin_token_env": "INFERENCE_ADMIN_TOKEN",
    "service_tenants": ["house"],
    "journal": "$state/gateway/attempts",
    "accounts": [
      {"id": "google-credit", "upstream": "vertex", "granted": 30000000000, "balance": 30000000000, "basis": "prepaid"},
      {"id": "zai-credit", "upstream": "zai", "granted": 100000000, "balance": 100000000, "basis": "prepaid"},
      {"id": "pro-free-capacity", "upstream": "pro", "granted": 0, "balance": 0, "basis": "free_capacity"},
      {"id": "openrouter", "upstream": "openrouter", "granted": 0, "balance": 0, "basis": "pay_as_you_go"},
      {"id": "vercel", "upstream": "vercel", "granted": 0, "balance": 0, "basis": "pay_as_you_go"}
    ]
  }
}
EOF

# A lock left by a previous process in this instance names a pid that is gone.
rm -f "$state/gateway/registry/quota-ledger.lock"
cd "$state/gateway"
exec gateway --config "$state/gateway.json"
