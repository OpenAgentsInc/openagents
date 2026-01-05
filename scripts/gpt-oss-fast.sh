#!/bin/bash
#
# Start a fast llama.cpp GPT-OSS server and warm it up.
#
# Env overrides:
#   LLAMA_SERVER, GPT_OSS_GGUF_MODEL_PATH, GPT_OSS_PORT, GPT_OSS_CTX,
#   GPT_OSS_BATCH, GPT_OSS_UBATCH, GPT_OSS_LOG,
#   GPT_OSS_WARMUP_COUNT, GPT_OSS_WARMUP_PROMPT, GPT_OSS_WARMUP_MAX_TOKENS,
#   GPT_OSS_CACHE_TYPE_K, GPT_OSS_CACHE_TYPE_V, GPT_OSS_KV_UNIFIED, GPT_OSS_FLASH_ATTN,
#   GPT_OSS_KEEPALIVE_MAX_TOKENS
#
set -euo pipefail

LLAMA_SERVER="${LLAMA_SERVER:-$HOME/code/llama.cpp/build/bin/llama-server}"

default_model_path() {
    local base="$HOME/models/gpt-oss-20b/gguf"
    local candidates=(
        "gpt-oss-20b-Q2_K.gguf"
        "gpt-oss-20b-Q3_K_S.gguf"
        "gpt-oss-20b-Q4_0.gguf"
        "gpt-oss-20b-Q4_K_M.gguf"
    )
    for candidate in "${candidates[@]}"; do
        local path="$base/$candidate"
        if [[ -f "$path" ]]; then
            echo "$path"
            return 0
        fi
    done
    return 1
}

MODEL_PATH="${GPT_OSS_GGUF_MODEL_PATH:-}"
if [[ -z "$MODEL_PATH" ]]; then
    MODEL_PATH="$(default_model_path || true)"
fi
PORT="${GPT_OSS_PORT:-8000}"
CTX="${GPT_OSS_CTX:-512}"
BATCH="${GPT_OSS_BATCH:-256}"
UBATCH="${GPT_OSS_UBATCH:-256}"
PARALLEL="${GPT_OSS_PARALLEL:-1}"
LOG_PATH="${GPT_OSS_LOG:-/tmp/llama-server-gptoss.log}"
WARMUP_COUNT="${GPT_OSS_WARMUP_COUNT:-2}"
WARMUP_PROMPT="${GPT_OSS_WARMUP_PROMPT:-warmup}"
WARMUP_MAX_TOKENS="${GPT_OSS_WARMUP_MAX_TOKENS:-8}"
KEEPALIVE_SECS="${GPT_OSS_KEEPALIVE_SECS:-0}"
KEEPALIVE_PID_FILE="${GPT_OSS_KEEPALIVE_PID_FILE:-/tmp/gpt-oss-keepalive.pid}"
FORCE_WARMUP="${GPT_OSS_FORCE_WARMUP:-0}"
KEEPALIVE_MAX_TOKENS="${GPT_OSS_KEEPALIVE_MAX_TOKENS:-1}"
CACHE_TYPE_K="${GPT_OSS_CACHE_TYPE_K:-q8_0}"
CACHE_TYPE_V="${GPT_OSS_CACHE_TYPE_V:-q8_0}"
KV_UNIFIED="${GPT_OSS_KV_UNIFIED:-0}"
CACHE_FLASH_ATTN="${GPT_OSS_FLASH_ATTN:-1}"

if [[ ! -x "$LLAMA_SERVER" ]]; then
    echo "llama-server not found: $LLAMA_SERVER" >&2
    exit 1
fi

if [[ -z "$MODEL_PATH" ]]; then
    echo "Model not found: set GPT_OSS_GGUF_MODEL_PATH" >&2
    exit 1
fi

if [[ ! -f "$MODEL_PATH" ]]; then
    echo "Model not found: $MODEL_PATH" >&2
    exit 1
fi

echo "Using model: $MODEL_PATH"

has_rg() {
    command -v rg >/dev/null 2>&1
}

health_ok() {
    local health
    health=$(curl -4 -s "http://localhost:${PORT}/health" || true)
    if [[ -z "$health" ]]; then
        return 1
    fi
    if has_rg; then
        echo "$health" | rg -q '"status"\s*:\s*"ok"'
    else
        echo "$health" | grep -q '"status"[[:space:]]*:[[:space:]]*"ok"'
    fi
}

started_server=0
if health_ok; then
    echo "llama-server already running on port $PORT"
else
    echo "Starting llama-server..."
    server_args=(
        -m "$MODEL_PATH"
        --port "$PORT"
        -ngl 999
        -c "$CTX"
        -b "$BATCH"
        -ub "$UBATCH"
        --no-warmup
        --no-mmap
    )
    if [[ -n "$CACHE_TYPE_K" ]]; then
        server_args+=(-ctk "$CACHE_TYPE_K")
    fi
    if [[ -n "$CACHE_TYPE_V" ]]; then
        server_args+=(-ctv "$CACHE_TYPE_V")
    fi
    if [[ "$KV_UNIFIED" -eq 1 ]]; then
        server_args+=(--kv-unified)
    fi
    if [[ "$CACHE_FLASH_ATTN" -eq 1 ]]; then
        server_args+=(--flash-attn)
    fi
    if [[ "$PARALLEL" -gt 1 ]]; then
        server_args+=(-np "$PARALLEL")
    fi

    nohup "$LLAMA_SERVER" "${server_args[@]}" \
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
    started_server=1
fi

if [[ "$WARMUP_COUNT" -lt "$PARALLEL" ]]; then
    WARMUP_COUNT="$PARALLEL"
fi

if [[ "$WARMUP_COUNT" -gt 0 ]] && { [[ "$started_server" -eq 1 ]] || [[ "$FORCE_WARMUP" -eq 1 ]]; }; then
    echo "Warming up ($WARMUP_COUNT requests)..."
    for _ in $(seq 1 "$WARMUP_COUNT"); do
        curl -4 -s "http://localhost:${PORT}/v1/completions" \
            -H 'Content-Type: application/json' \
            -d "{\"model\":\"gpt-oss-20b\",\"prompt\":\"${WARMUP_PROMPT}\",\"max_tokens\":${WARMUP_MAX_TOKENS},\"temperature\":0}" \
            >/dev/null || true
    done
fi

if [[ "$KEEPALIVE_SECS" -gt 0 ]]; then
    if [[ -f "$KEEPALIVE_PID_FILE" ]]; then
        existing_pid=$(cat "$KEEPALIVE_PID_FILE" 2>/dev/null || true)
        if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
            echo "Keepalive already running (pid $existing_pid)"
        else
            rm -f "$KEEPALIVE_PID_FILE"
        fi
    fi

    if [[ ! -f "$KEEPALIVE_PID_FILE" ]]; then
        echo "Starting keepalive every ${KEEPALIVE_SECS}s..."
        nohup bash -c "while true; do curl -4 -s \"http://localhost:${PORT}/v1/completions\" \
            -H 'Content-Type: application/json' \
            -d \"{\\\"model\\\":\\\"gpt-oss-20b\\\",\\\"prompt\\\":\\\"${WARMUP_PROMPT}\\\",\\\"max_tokens\\\":${KEEPALIVE_MAX_TOKENS},\\\"temperature\\\":0}\" \
            >/dev/null || true; sleep \"${KEEPALIVE_SECS}\"; done" \
            >/dev/null 2>&1 &
        keepalive_pid=$!
        disown "$keepalive_pid" 2>/dev/null || true
        echo "$keepalive_pid" > "$KEEPALIVE_PID_FILE"
        echo "Keepalive PID: $keepalive_pid"
    fi
fi

echo "Ready: http://localhost:${PORT}"
