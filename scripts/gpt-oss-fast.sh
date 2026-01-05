#!/bin/bash
#
# Start a fast llama.cpp GPT-OSS server and warm it up.
#
# Env overrides:
#   LLAMA_SERVER, GPT_OSS_GGUF_MODEL_PATH, GPT_OSS_PORT, GPT_OSS_CTX,
#   GPT_OSS_BATCH, GPT_OSS_UBATCH, GPT_OSS_LOG,
#   GPT_OSS_WARMUP_COUNT, GPT_OSS_WARMUP_PROMPT
#
set -euo pipefail

LLAMA_SERVER="${LLAMA_SERVER:-$HOME/code/llama.cpp/build/bin/llama-server}"
MODEL_PATH="${GPT_OSS_GGUF_MODEL_PATH:-$HOME/models/gpt-oss-20b/gguf/gpt-oss-20b-Q3_K_S.gguf}"
PORT="${GPT_OSS_PORT:-8000}"
CTX="${GPT_OSS_CTX:-512}"
BATCH="${GPT_OSS_BATCH:-256}"
UBATCH="${GPT_OSS_UBATCH:-256}"
LOG_PATH="${GPT_OSS_LOG:-/tmp/llama-server-gptoss.log}"
WARMUP_COUNT="${GPT_OSS_WARMUP_COUNT:-2}"
WARMUP_PROMPT="${GPT_OSS_WARMUP_PROMPT:-warmup}"
KEEPALIVE_SECS="${GPT_OSS_KEEPALIVE_SECS:-0}"

if [[ ! -x "$LLAMA_SERVER" ]]; then
    echo "llama-server not found: $LLAMA_SERVER" >&2
    exit 1
fi

if [[ ! -f "$MODEL_PATH" ]]; then
    echo "Model not found: $MODEL_PATH" >&2
    exit 1
fi

has_rg() {
    command -v rg >/dev/null 2>&1
}

health_ok() {
    local health
    health=$(curl -s "http://localhost:${PORT}/health" || true)
    if [[ -z "$health" ]]; then
        return 1
    fi
    if has_rg; then
        echo "$health" | rg -q '"status"\s*:\s*"ok"'
    else
        echo "$health" | grep -q '"status"[[:space:]]*:[[:space:]]*"ok"'
    fi
}

if health_ok; then
    echo "llama-server already running on port $PORT"
else
    echo "Starting llama-server..."
    nohup "$LLAMA_SERVER" \
        -m "$MODEL_PATH" \
        --port "$PORT" \
        -ngl 999 \
        -c "$CTX" \
        -b "$BATCH" \
        -ub "$UBATCH" \
        --no-warmup \
        --no-mmap \
        > "$LOG_PATH" 2>&1 &
    echo "llama-server PID: $!"

    for _ in {1..60}; do
        if health_ok; then
            break
        fi
        sleep 2
    done

    if ! health_ok; then
        echo "llama-server failed to become healthy. See $LOG_PATH" >&2
        exit 1
    fi
fi

if [[ "$WARMUP_COUNT" -gt 0 ]]; then
    echo "Warming up ($WARMUP_COUNT requests)..."
    for _ in $(seq 1 "$WARMUP_COUNT"); do
        curl -s "http://localhost:${PORT}/v1/completions" \
            -H 'Content-Type: application/json' \
            -d "{\"model\":\"gpt-oss-20b\",\"prompt\":\"${WARMUP_PROMPT}\",\"max_tokens\":1,\"temperature\":0}" \
            >/dev/null || true
    done
fi

if [[ "$KEEPALIVE_SECS" -gt 0 ]]; then
    echo "Starting keepalive every ${KEEPALIVE_SECS}s..."
    nohup bash -c "while true; do curl -s \"http://localhost:${PORT}/v1/completions\" \
        -H 'Content-Type: application/json' \
        -d \"{\\\"model\\\":\\\"gpt-oss-20b\\\",\\\"prompt\\\":\\\"${WARMUP_PROMPT}\\\",\\\"max_tokens\\\":1,\\\"temperature\\\":0}\" \
        >/dev/null || true; sleep \"${KEEPALIVE_SECS}\"; done" \
        >/dev/null 2>&1 & disown
fi

echo "Ready: http://localhost:${PORT}"
