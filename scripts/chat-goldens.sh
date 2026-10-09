#!/bin/sh
# Run the web chat goldens (docs/web/chat-goldens.md).
#
#   scripts/chat-goldens.sh check                         offline, every accepted answer meets its golden
#   scripts/chat-goldens.sh router [--golden ID]...       Jev and the router in this process
#   scripts/chat-goldens.sh http --base URL [...]         a running site's chat endpoints
#   scripts/chat-goldens.sh local [...]                   this checkout's chat worker and site, then http
#
# router and local need TYPESAFE_API_KEY, AI_GATEWAY_API_KEY, and (local)
# OPENROUTER_API_KEY. When they are unset, they are read from
# typesafe.env, ai-gateway.env, and openrouter.env in $OPENAGENTS_SECRETS
# (default ~/work/.secrets) if those files exist; nothing is printed.
#
# local starts a chat worker built from this checkout on a fresh key (on
# relay.openagents.com, answering only the site it starts) and the website
# on 127.0.0.1:${CHAT_GOLDENS_PORT:-4399} pointed at it, runs the http mode
# against it, and stops both. Nothing touches the production worker.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
mode=${1:-}
[ -n "$mode" ] || { sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
shift

secrets=${OPENAGENTS_SECRETS:-$HOME/work/.secrets}
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

build() {
    (cd "$root" && cargo build -q -p coder --bin chat-goldens "$@")
}

target=${CARGO_TARGET_DIR:-$root/target}
goldens="$target/debug/chat-goldens"

case "$mode" in
check | http)
    build
    exec "$goldens" "$mode" "$@"
    ;;
router)
    load TYPESAFE_API_KEY typesafe.env
    load AI_GATEWAY_API_KEY ai-gateway.env
    export CODER_AI_GATEWAY_KEY="${CODER_AI_GATEWAY_KEY:-${AI_GATEWAY_API_KEY:-}}"
    export OPENAGENTS_PRODUCT_KB_EMBEDDINGS="${OPENAGENTS_PRODUCT_KB_EMBEDDINGS:-gateway}"
    build
    exec "$goldens" router "$@"
    ;;
local)
    load TYPESAFE_API_KEY typesafe.env
    load AI_GATEWAY_API_KEY ai-gateway.env
    load OPENROUTER_API_KEY openrouter.env
    : "${TYPESAFE_API_KEY:?local needs TYPESAFE_API_KEY}"
    : "${AI_GATEWAY_API_KEY:?local needs AI_GATEWAY_API_KEY}"
    (cd "$root" && cargo build -q -p coder --bin chat-goldens --bin coder-worker -p openagents-web --bin openagents-web)
    port=${CHAT_GOLDENS_PORT:-4399}
    work=$(mktemp -d "${TMPDIR:-/tmp}/chat-goldens.XXXXXX")
    (umask 077 && openssl rand -hex 32 > "$work/secret")
    mkdir -p "$work/usage" "$work/store" "$work/chats"
    (
        CODER_WORKER_SECRET=$(cat "$work/secret")
        export CODER_WORKER_SECRET
        export CODER_RELAY=wss://relay.openagents.com CODER_WORKER_OPEN=1 CODER_WORKER_JOBS=16
        export CODER_WORKER_USAGE_DIR="$work/usage" CODER_DOOR_KEY="$AI_GATEWAY_API_KEY"
        export CODER_WORKER_MODEL=gemini CODER_PERSONALIZE=openrouter
        export OPENAGENTS_PRODUCT_KNOWLEDGE="$root/knowledge/openagents" OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway
        exec "$target/debug/coder-worker" > "$work/worker.log" 2>&1
    ) &
    worker_pid=$!
    trap 'kill $worker_pid ${web_pid:-} 2>/dev/null || true' EXIT INT TERM
    key=""
    for _ in $(seq 1 60); do
        key=$(sed -n 's/^worker  *\([0-9a-f]\{64\}\)$/\1/p' "$work/worker.log" | head -n 1)
        grep -q "subscribed" "$work/worker.log" 2>/dev/null && [ -n "$key" ] && break
        sleep 1
    done
    [ -n "$key" ] || { echo "the local worker didn't start; see $work/worker.log" >&2; exit 2; }
    (
        export OPENAGENTS_WEB_CHAT_WORKER="$key"
        cd "$work" && exec "$target/debug/openagents-web" --listen "127.0.0.1:$port" \
            --store "$work/store" --chat-store "$work/chats" > "$work/web.log" 2>&1
    ) &
    web_pid=$!
    for _ in $(seq 1 60); do
        curl -fsS "http://127.0.0.1:$port/health" > /dev/null 2>&1 && break
        sleep 1
    done
    status=0
    "$goldens" http --base "http://127.0.0.1:$port" "$@" || status=$?
    echo "Worker and site logs: $work" >&2
    exit "$status"
    ;;
*)
    sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
