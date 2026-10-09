#!/bin/sh
# The website's chat answered through a local inference gateway
# (docs/inference/gateway.md, #11064): the gateway on 127.0.0.1:8790 with
# its adapters, a chat worker from this checkout that sends every model call
# to it, and the website on 127.0.0.1:4300 pointed at that worker.
#
#   scripts/dev/inference-local.sh                # build, start all three, wait
#   scripts/dev/inference-local.sh --no-build     # start what is already built
#   scripts/dev/inference-local.sh --gateway-only # the gateway alone (no worker or site)
#
# The website passes /api/v1/... to the gateway (--inference), so
# http://127.0.0.1:$WEB_PORT/api/v1 is the same API as the gateway's /v1.
# The public API is on (#11065): any key works, with 20 free requests a
# day on the Pro door's models; keys outside the house tenant have no
# balance here, so other models answer 402 for them.
#
# Provider keys come from the environment or, when unset, from
# openrouter.env, ai-gateway.env, and typesafe.env in $OPENAGENTS_SECRETS
# (default ~/work/.secrets). Vertex, Z.ai, and the Pro door join routing
# when their keys are set (GOOGLE_APPLICATION_CREDENTIALS or
# VERTEX_ACCESS_TOKEN, ZAI_API_KEY, PRO_UPSTREAM_KEY); without them the
# router skips them. Nothing is printed from any key.
#
# State lives in $OPENAGENTS_INFERENCE_LOCAL (default
# ~/.openagents/inference-local): the gateway's registry with a `house`
# tenant and its service key, the admin token for /admin/inference, the
# worker's key, attempt day files, and the logs. The worker joins
# relay.openagents.com on its own fresh key and answers only the site this
# script starts; nothing touches the production worker.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
secrets=${OPENAGENTS_SECRETS:-$HOME/work/.secrets}
state=${OPENAGENTS_INFERENCE_LOCAL:-$HOME/.openagents/inference-local}
target=${CARGO_TARGET_DIR:-$root/target}
gateway_port=${INFERENCE_PORT:-8790}
web_port=${WEB_PORT:-4300}

load() {
    # load VAR FILE: source FILE for VAR when VAR is unset and FILE exists.
    eval "set_=\${$1:-}"
    if [ -z "$set_" ] && [ -f "$secrets/$2" ]; then
        set -a
        # shellcheck disable=SC1090
        . "$secrets/$2"
        set +a
    fi
}
load OPENROUTER_API_KEY openrouter.env
load AI_GATEWAY_API_KEY ai-gateway.env
load TYPESAFE_API_KEY typesafe.env

build=1
only=""
for argument in "$@"; do
    case "$argument" in
        --no-build) build="" ;;
        --gateway-only) only=1 ;;
        *) echo "unknown option $argument" >&2; exit 64 ;;
    esac
done

if [ -n "$build" ]; then
    if [ -n "$only" ]; then
        (cd "$root" && cargo build -q -p gateway --bin gateway \
            && cargo build -q -p tenancy --example bootstrap_registry)
    else
        (cd "$root" && cargo build -q -p gateway --bin gateway -p coder --bin coder-worker \
            -p openagents-web --bin openagents-web && cargo build -q -p tenancy --example bootstrap_registry)
    fi
fi

mkdir -p "$state/usage" "$state/attempts" "$state/store" "$state/chats"
chmod 700 "$state"
if [ ! -s "$state/service.key" ]; then
    # One house tenant and its service key. The door in the registry is a
    # placeholder the inference routes never use.
    (umask 077 && "$target/debug/examples/bootstrap_registry" --registry "$state/registry" \
        --tenant house --door local-kev --model kev-latest \
        --signature "sha256:$(printf '0%.0s' $(seq 1 64))" > "$state/service.key")
fi
[ -s "$state/admin.token" ] || (umask 077 && openssl rand -hex 24 > "$state/admin.token")
[ -s "$state/worker.secret" ] || (umask 077 && openssl rand -hex 32 > "$state/worker.secret")

# Credit accounts for the burn-down. The Google figure is the spec's
# "about $30,000"; the Z.ai balance is a placeholder until the owner names
# it (docs/inference/gateway.md, section 14). Amounts are micro-dollars.
cat > "$state/gateway.json" <<EOF
{
  "v": "openagents.gateway.v1",
  "listen": "127.0.0.1:$gateway_port",
  "registry": "$state/registry",
  "inference": {
    "admin_token_env": "INFERENCE_ADMIN_TOKEN",
    "service_tenants": ["house"],
    "public": {"free_tier": {"requests_per_day": 20, "models": ["openai/gpt-5.6-luna", "openai/gpt-5.6-terra", "openai/gpt-5.6-sol"]}},
    "journal": "$state/attempts",
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

# A gateway stopped by a signal leaves its ledger lock behind; clear it
# when the process named inside is gone.
lock="$state/registry/quota-ledger.lock"
if [ -f "$lock" ]; then
    holder=$(sed -n 's/^pid \([0-9]*\)$/\1/p' "$lock")
    if [ -z "$holder" ] || ! kill -0 "$holder" 2>/dev/null; then
        rm -f "$lock"
    fi
fi

pids=""
trap 'kill $pids 2>/dev/null || true' EXIT INT TERM

(
    INFERENCE_ADMIN_TOKEN=$(cat "$state/admin.token")
    export INFERENCE_ADMIN_TOKEN
    exec "$target/debug/gateway" --config "$state/gateway.json" > "$state/gateway.log" 2>&1
) &
pids="$pids $!"
for _ in $(seq 1 60); do
    curl -fsS "http://127.0.0.1:$gateway_port/healthz" > /dev/null 2>&1 && break
    sleep 1
done

if [ -n "$only" ]; then
    echo "Gateway:   http://127.0.0.1:$gateway_port/v1 (service key in $state/service.key)"
    echo "Dashboard: http://127.0.0.1:$gateway_port/admin/inference (admin token in $state/admin.token)"
    echo "Log:       $state/gateway.log"
    wait
    exit 0
fi

(
    CODER_WORKER_SECRET=$(cat "$state/worker.secret")
    CODER_INFERENCE_KEY=$(cat "$state/service.key")
    export CODER_WORKER_SECRET CODER_INFERENCE_KEY
    export CODER_INFERENCE_URL="http://127.0.0.1:$gateway_port"
    export CODER_RELAY=wss://relay.openagents.com CODER_WORKER_OPEN=1 CODER_WORKER_JOBS=16
    export CODER_WORKER_USAGE_DIR="$state/usage"
    export OPENAGENTS_PRODUCT_KNOWLEDGE="$root/knowledge/openagents"
    # Generation goes through the gateway; these keys stay only for the
    # worker's other calls (Jev's doors, personalization, product search).
    export CODER_AI_GATEWAY_KEY="${AI_GATEWAY_API_KEY:-}" OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway
    exec "$target/debug/coder-worker" > "$state/worker.log" 2>&1
) &
pids="$pids $!"
key=""
for _ in $(seq 1 60); do
    key=$(sed -n 's/^worker  *\([0-9a-f]\{64\}\)$/\1/p' "$state/worker.log" | head -n 1)
    grep -q "subscribed" "$state/worker.log" 2>/dev/null && [ -n "$key" ] && break
    sleep 1
done
[ -n "$key" ] || { echo "the worker didn't start; see $state/worker.log" >&2; exit 2; }

(
    export OPENAGENTS_WEB_CHAT_WORKER="$key"
    cd "$state" && exec "$target/debug/openagents-web" --listen "127.0.0.1:$web_port" \
        --store "$state/store" --chat-store "$state/chats" \
        --inference "http://127.0.0.1:$gateway_port" > "$state/web.log" 2>&1
) &
pids="$pids $!"
for _ in $(seq 1 60); do
    curl -fsS "http://127.0.0.1:$web_port/health" > /dev/null 2>&1 && break
    sleep 1
done

echo "Website:   http://127.0.0.1:$web_port (the API also at /api/v1)"
echo "Gateway:   http://127.0.0.1:$gateway_port/v1/responses (service key in $state/service.key)"
echo "Dashboard: http://127.0.0.1:$gateway_port/admin/inference (admin token in $state/admin.token)"
echo "Logs:      $state/{gateway,worker,web}.log"
wait
