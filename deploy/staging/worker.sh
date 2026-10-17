#!/bin/sh
# The staging chat worker sidecar: coder-worker with every model call sent
# through the gateway sidecar (127.0.0.1:8791) on the `house` service key
# the gateway sidecar wrote to $STACK_STATE/service.key. It joins the relay
# on its own staging key (CODER_WORKER_SECRET), whose public key the web
# container names in OPENAGENTS_WEB_CHAT_WORKER, and only while the gateway
# sidecar answers (below).
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
# Only while this instance's gateway answers (#11991). A deploy keeps the
# outgoing revision warm for its drain, and its gateway hands the store to
# the new one and stops; every revision's worker joins the relay on the
# same key, so one whose gateway is gone took visitors' turns and declined
# them before the serving worker could answer. The worker leaves the relay
# within a few seconds of its gateway stopping and joins again when it is
# back (an update-traffic rollback).
health=${CODER_INFERENCE_URL%/}/healthz
up() {
    curl -fsS -o /dev/null --max-time 2 "$health" 2> /dev/null
}
child=
trap 'if [ -n "$child" ]; then kill "$child" 2> /dev/null || true; wait "$child" 2> /dev/null || true; fi; exit 0' TERM INT
while :; do
    until up; do
        sleep 2 &
        wait $! || true
    done
    coder-worker &
    child=$!
    echo "worker: the gateway answers; on the relay" >&2
    misses=0
    while kill -0 "$child" 2> /dev/null; do
        sleep 2 &
        wait $! || true
        if up; then
            misses=0
        else
            misses=$((misses + 1))
        fi
        if [ "$misses" -ge 3 ]; then
            echo "worker: this instance's gateway stopped answering; leaving the relay" >&2
            kill "$child" 2> /dev/null || true
            break
        fi
    done
    wait "$child" 2> /dev/null || true
    child=
    sleep 2 &
    wait $! || true
done
