#!/bin/sh
# The staging chat worker sidecar: coder-worker with every model call sent
# through the gateway sidecar (127.0.0.1:8791) on the `house` service key
# the gateway sidecar wrote to $STACK_STATE/service.key. It joins the relay
# on its own staging key (CODER_WORKER_SECRET), whose public key the web
# container names in OPENAGENTS_WEB_CHAT_WORKER.
set -eu
umask 077
state=${STACK_STATE:-/stack}
: "${CODER_WORKER_SECRET:?CODER_WORKER_SECRET is unset}"
i=0
while [ ! -s "$state/service.key" ]; do
    i=$((i + 1))
    if [ "$i" -gt 120 ]; then
        echo "The gateway sidecar never wrote its service key." >&2
        exit 1
    fi
    sleep 1
done
CODER_INFERENCE_KEY=$(cat "$state/service.key")
export CODER_INFERENCE_KEY
export CODER_INFERENCE_URL=${CODER_INFERENCE_URL:-http://127.0.0.1:8791}
export CODER_RELAY=${CODER_RELAY:-wss://relay.openagents.com}
export CODER_WORKER_OPEN=${CODER_WORKER_OPEN:-1}
export CODER_WORKER_JOBS=${CODER_WORKER_JOBS:-16}
mkdir -p "$state/worker/usage"
export CODER_WORKER_USAGE_DIR="$state/worker/usage"
export OPENAGENTS_PRODUCT_KNOWLEDGE=${OPENAGENTS_PRODUCT_KNOWLEDGE:-/srv/knowledge/openagents}
cd "$state/worker"
exec coder-worker
