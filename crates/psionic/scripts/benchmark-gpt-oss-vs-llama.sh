#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh [--server SERVER] [--model PATH] [--psionic-bin PATH] [--psionic-backend BACKEND] [--psionic-metal-mode MODE] [--llama-bin PATH] [--host HOST] [--port PORT] [--ctx N] [--ngl N] [--max-tokens N] [--startup-timeout-seconds N] [--request-timeout-seconds N] [--json-out DIR]

Defaults:
  macOS model:           /Users/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
  Linux model:           /home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
  macOS llama-bin:       /Users/christopherdavid/code/llama.cpp/build/bin/llama-server
  Linux llama-bin:       /home/christopherdavid/code/llama.cpp/build/bin/llama-server
  macOS psionic backend: metal
  Linux psionic backend: cuda
  psionic metal mode:    native
  host:                  127.0.0.1
  port:                  8099
  ctx:                   4096
  ngl:                   999
  max_tokens:            64
  startup_timeout:       60
  request_timeout:       60
  server:                both

Notes:
  - The script prefers ./target/release/psionic-gpt-oss-server, then ./target/debug/psionic-gpt-oss-server.
  - On macOS Metal, Psionic defaults to the native Rust/Metal path. Use `--psionic-metal-mode proxy` only for explicit `llama.cpp` oracle/debug runs.
  - `--server psionic` runs the Psionic lane only and does not require `llama.cpp`.
  - Each server is measured on three same-host cases:
      1. cold request
      2. warm non-hit request
      3. prompt-cache-hit request
  - When --json-out DIR is set, the script writes raw responses and per-case summaries to DIR.
EOF
}

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../../.." && pwd)
PLATFORM=$(uname -s)

if [[ "$PLATFORM" == "Darwin" ]]; then
  MODEL_PATH=/Users/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
  LLAMA_BIN=/Users/christopherdavid/code/llama.cpp/build/bin/llama-server
  PSI_BACKEND=metal
  CTX=1024
  NGL=4
else
  MODEL_PATH=/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
  LLAMA_BIN=/home/christopherdavid/code/llama.cpp/build/bin/llama-server
  PSI_BACKEND=cuda
  CTX=4096
  NGL=999
fi

HOST=127.0.0.1
PORT=8099
MAX_TOKENS=64
STARTUP_TIMEOUT_SECONDS=60
REQUEST_TIMEOUT_SECONDS=60
PSI_BIN=
JSON_OUT=
PSIONIC_METAL_MODE=
PSIONIC_METAL_MODE_SET=0
SERVER_SELECTION=both

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model)
      MODEL_PATH=${2:?missing value for --model}
      shift 2
      ;;
    --server)
      SERVER_SELECTION=${2:?missing value for --server}
      shift 2
      ;;
    --psionic-bin)
      PSI_BIN=${2:?missing value for --psionic-bin}
      shift 2
      ;;
    --psionic-backend)
      PSI_BACKEND=${2:?missing value for --psionic-backend}
      shift 2
      ;;
    --llama-bin)
      LLAMA_BIN=${2:?missing value for --llama-bin}
      shift 2
      ;;
    --psionic-metal-mode)
      PSIONIC_METAL_MODE=${2:?missing value for --psionic-metal-mode}
      PSIONIC_METAL_MODE_SET=1
      shift 2
      ;;
    --host)
      HOST=${2:?missing value for --host}
      shift 2
      ;;
    --port)
      PORT=${2:?missing value for --port}
      shift 2
      ;;
    --ctx)
      CTX=${2:?missing value for --ctx}
      shift 2
      ;;
    --ngl)
      NGL=${2:?missing value for --ngl}
      shift 2
      ;;
    --max-tokens)
      MAX_TOKENS=${2:?missing value for --max-tokens}
      shift 2
      ;;
    --startup-timeout-seconds)
      STARTUP_TIMEOUT_SECONDS=${2:?missing value for --startup-timeout-seconds}
      shift 2
      ;;
    --request-timeout-seconds)
      REQUEST_TIMEOUT_SECONDS=${2:?missing value for --request-timeout-seconds}
      shift 2
      ;;
    --json-out)
      JSON_OUT=${2:?missing value for --json-out}
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unrecognized argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

case "$SERVER_SELECTION" in
  both|psionic|llama)
    ;;
  *)
    echo "invalid --server value: $SERVER_SELECTION (expected both, psionic, or llama)" >&2
    exit 1
    ;;
esac

should_run_psionic() {
  [[ "$SERVER_SELECTION" == "both" || "$SERVER_SELECTION" == "psionic" ]]
}

should_run_llama() {
  [[ "$SERVER_SELECTION" == "both" || "$SERVER_SELECTION" == "llama" ]]
}

is_truthy() {
  local value=${1:-}
  [[ "$value" == "1" || "$value" == "true" || "$value" == "TRUE" || "$value" == "yes" || "$value" == "YES" ]]
}

if [[ "$PSI_BACKEND" == "metal" ]]; then
  if [[ -z "$PSIONIC_METAL_MODE" ]]; then
    PSIONIC_METAL_MODE=native
  fi
  case "$PSIONIC_METAL_MODE" in
    native|proxy)
      ;;
    *)
      echo "invalid --psionic-metal-mode value: $PSIONIC_METAL_MODE (expected native or proxy)" >&2
      exit 1
      ;;
  esac
else
  if (( PSIONIC_METAL_MODE_SET )); then
    echo "--psionic-metal-mode is only valid when --psionic-backend metal" >&2
    exit 1
  fi
  PSIONIC_METAL_MODE=not_applicable
fi

if [[ "$PSI_BACKEND" == "metal" && "$PSIONIC_METAL_MODE" == "native" ]] && is_truthy "${PSIONIC_METAL_PROXY_LLAMA_CPP:-}"; then
  echo "native Metal benchmark requested, but legacy PSIONIC_METAL_PROXY_LLAMA_CPP is still enabled" >&2
  echo "unset PSIONIC_METAL_PROXY_LLAMA_CPP or rerun with --psionic-metal-mode proxy" >&2
  exit 1
fi

if should_run_psionic && [[ -z "$PSI_BIN" ]]; then
  if [[ -x "$REPO_ROOT/target/release/psionic-gpt-oss-server" ]]; then
    PSI_BIN=$REPO_ROOT/target/release/psionic-gpt-oss-server
  elif [[ -x "$REPO_ROOT/target/debug/psionic-gpt-oss-server" ]]; then
    PSI_BIN=$REPO_ROOT/target/debug/psionic-gpt-oss-server
  else
    echo "could not find psionic-gpt-oss-server under $REPO_ROOT/target/{release,debug}" >&2
    echo "build it first, or pass --psionic-bin PATH" >&2
    exit 1
  fi
fi

if [[ -n "$JSON_OUT" ]]; then
  mkdir -p "$JSON_OUT"
fi

for tool in curl jq awk date; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing required tool: $tool" >&2
    exit 1
  fi
done

if [[ ! -f "$MODEL_PATH" ]]; then
  echo "model not found: $MODEL_PATH" >&2
  exit 1
fi

if should_run_psionic && [[ ! -x "$PSI_BIN" ]]; then
  echo "psionic binary not executable: $PSI_BIN" >&2
  exit 1
fi

if should_run_llama && [[ ! -x "$LLAMA_BIN" ]]; then
  echo "llama.cpp server not executable: $LLAMA_BIN" >&2
  exit 1
fi

TMP_DIR=$(mktemp -d)
SERVER_PID=

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    if child_pids=$(pgrep -P "$SERVER_PID" 2>/dev/null); then
      kill $child_pids >/dev/null 2>&1 || true
      wait $child_pids 2>/dev/null || true
    fi
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR"
}

trap cleanup EXIT

build_request_json() {
  local sentence=$1
  local model_name
  model_name=$(basename "$MODEL_PATH")
  # Keep the benchmark request explicit and stable across both servers. The
  # local llama.cpp GPT-OSS template only reliably reaches the final sentence
  # within the 64-token budget on this host when the canonical system contract
  # is present explicitly instead of relying on the built-in default.
  jq -cn \
    --arg sentence "$sentence" \
    --arg model_name "$model_name" \
    --argjson max_tokens "$MAX_TOKENS" \
    '{
      model: $model_name,
      messages: [
        {
          role: "system",
          content: "You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\nCurrent date: 2026-03-08\n\nReasoning: low\n\n# Valid channels: analysis, final. Channel must be included for every message."
        },
        {
          role: "developer",
          content: "Be concise. Output exactly one sentence."
        },
        {
          role: "user",
          content: ("Reply with exactly this sentence and nothing else: " + $sentence)
        }
      ],
      max_tokens: $max_tokens,
      temperature: 0,
      reasoning_format: "none",
      chat_template_kwargs: {
        enable_thinking: false
      }
    }'
}

REQUEST_COLD=$(build_request_json "HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.")
REQUEST_WARM=$(build_request_json "TLS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.")
REQUEST_CACHE_HIT=$REQUEST_COLD

wait_for_server() {
  local health_url=$1
  local started_at now
  started_at=$(date +%s)
  until curl --connect-timeout 1 --max-time 1 -fsS "$health_url" >/dev/null 2>&1; do
    now=$(date +%s)
    if (( now - started_at >= STARTUP_TIMEOUT_SECONDS )); then
      echo "server did not become ready: $health_url" >&2
      exit 1
    fi
    sleep 0.2
  done
}

run_request() {
  local request_json=$1
  local output_file=$2
  curl --no-buffer --max-time "$REQUEST_TIMEOUT_SECONDS" -sS "http://$HOST:$PORT/v1/chat/completions" \
    -H 'Content-Type: application/json' \
    -d "$request_json" | tee "$output_file" >/dev/null
}

extract_visible_output() {
  local response_file=$1
  jq -r '
    def strip_visible_wrappers:
      gsub("(?s)<\\|channel\\|>analysis<\\|message\\|>.*?<\\|end\\|>"; "")
      | gsub("(?s)<\\|start\\|>assistant<\\|channel\\|>analysis<\\|message\\|>.*?<\\|end\\|>"; "")
      | gsub("(?s)<think>.*?</think>"; "")
      | gsub("^[\\n\\r]+"; "")
      | gsub("[\\n\\r]+$"; "");
    ((.choices[0].message.content // "") | strip_visible_wrappers)
  ' "$response_file"
}

capture_health_json() {
  local server_name=$1
  local output_file=$2

  if [[ "$server_name" == "psionic" ]]; then
    curl --max-time 2 -fsS "http://$HOST:$PORT/health" > "$output_file"
  else
    jq -cn \
      --arg model "$(basename "$MODEL_PATH")" \
      '{
        status: "ok",
        backend: "reference",
        execution_mode: "direct",
        execution_engine: "llama.cpp",
        model: $model
      }' > "$output_file"
  fi
}

write_summary_json() {
  local server_name=$1
  local case_name=$2
  local response_file=$3
  local elapsed=$4
  local tokens=$5
  local tokps=$6
  local visible_output=$7

  [[ -n "$JSON_OUT" ]] || return 0

  cp "$response_file" "$JSON_OUT/$server_name.$case_name.response.json"
  cp "$TMP_DIR/$server_name.health.json" "$JSON_OUT/$server_name.health.json"
  jq -n \
    --arg server "$server_name" \
    --arg case_name "$case_name" \
    --arg model "$MODEL_PATH" \
    --arg endpoint "http://$HOST:$PORT/v1/chat/completions" \
    --arg psionic_backend "$PSI_BACKEND" \
    --arg elapsed_seconds "$elapsed" \
    --arg completion_tokens "$tokens" \
    --arg tokens_per_second "$tokps" \
    --arg visible_output "$visible_output" \
    --slurpfile response "$response_file" \
    --slurpfile health "$TMP_DIR/$server_name.health.json" \
    '{
      server: $server,
      case: $case_name,
      model_path: $model,
      endpoint: $endpoint,
      psionic_backend: $psionic_backend,
      elapsed_seconds: ($elapsed_seconds | tonumber),
      completion_tokens: ($completion_tokens | tonumber),
      tokens_per_second: ($tokens_per_second | tonumber),
      visible_output: $visible_output,
      execution_mode: ($health[0].execution_mode // "unknown"),
      execution_engine: ($health[0].execution_engine // "unknown"),
      server_health: $health[0],
      response: $response[0]
    }' > "$JSON_OUT/$server_name.$case_name.summary.json"
}

run_case() {
  local server_name=$1
  local case_name=$2
  local request_json=$3
  local output_file=$4

  echo "[${case_name}]"
  local start end elapsed tokens tokps visible_output
  start=$(date +%s.%N)
  run_request "$request_json" "$output_file"
  end=$(date +%s.%N)

  visible_output=$(extract_visible_output "$output_file")
  printf '%s\n' "$visible_output"
  tokens=$(jq -r '.usage.completion_tokens' "$output_file")
  elapsed=$(awk "BEGIN { printf \"%.3f\", $end - $start }")
  tokps=$(awk "BEGIN { printf \"%.2f\", $tokens / ($end - $start) }")
  echo "case=$case_name completion_tokens=$tokens seconds=$elapsed tok/s=$tokps"
  write_summary_json "$server_name" "$case_name" "$output_file" "$elapsed" "$tokens" "$tokps" "$visible_output"
}

bench() {
  local server_name=$1
  shift

  echo
  echo "=== $server_name ==="
  if [[ "$server_name" == "psionic" ]]; then
    env -u PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS "$@" &
  else
    "$@" &
  fi
  SERVER_PID=$!

  wait_for_server "http://$HOST:$PORT/health"
  capture_health_json "$server_name" "$TMP_DIR/$server_name.health.json"
  echo "health=$(tr -d '\n' < "$TMP_DIR/$server_name.health.json")"
  run_case "$server_name" "cold" "$REQUEST_COLD" "$TMP_DIR/$server_name.cold.json"
  run_case "$server_name" "warm_non_hit" "$REQUEST_WARM" "$TMP_DIR/$server_name.warm_non_hit.json"
  run_case "$server_name" "prompt_cache_hit" "$REQUEST_CACHE_HIT" "$TMP_DIR/$server_name.prompt_cache_hit.json"

  if child_pids=$(pgrep -P "$SERVER_PID" 2>/dev/null); then
    kill $child_pids >/dev/null 2>&1 || true
    wait $child_pids 2>/dev/null || true
  fi
  kill "$SERVER_PID" >/dev/null 2>&1 || true
  wait "$SERVER_PID" 2>/dev/null || true
  SERVER_PID=
  sleep 1
}

echo "repo_root=$REPO_ROOT"
echo "platform=$PLATFORM"
echo "server_selection=$SERVER_SELECTION"
echo "model=$MODEL_PATH"
echo "psionic_bin=$PSI_BIN"
echo "psionic_backend=$PSI_BACKEND"
echo "psionic_metal_mode=$PSIONIC_METAL_MODE"
if should_run_llama; then
  echo "llama_bin=$LLAMA_BIN"
fi
echo "max_tokens=$MAX_TOKENS"
echo "startup_timeout_seconds=$STARTUP_TIMEOUT_SECONDS"
echo "request_timeout_seconds=$REQUEST_TIMEOUT_SECONDS"
echo "endpoint=http://$HOST:$PORT/v1/chat/completions"

PSIONIC_CMD=(
  "$PSI_BIN" \
  -m "$MODEL_PATH" \
  --backend "$PSI_BACKEND" \
  --host "$HOST" \
  --port "$PORT" \
  -c "$CTX" \
  -ngl "$NGL" \
  --reasoning-budget 0 \
  --no-webui
)

if [[ "$PSI_BACKEND" == "metal" ]]; then
  PSIONIC_CMD+=(--metal-mode "$PSIONIC_METAL_MODE")
fi

if [[ "$PSI_BACKEND" == "metal" && "$PSIONIC_METAL_MODE" == "proxy" ]]; then
  PSIONIC_CMD=(
    env
    PSIONIC_LLAMA_SERVER_BIN="$LLAMA_BIN"
    PSIONIC_LLAMA_BATCH_SIZE=64
    PSIONIC_LLAMA_UBATCH_SIZE=64
    "${PSIONIC_CMD[@]}"
  )
fi

if should_run_psionic; then
  bench \
    "psionic" \
    "${PSIONIC_CMD[@]}"
fi

LLAMA_CMD=(
  "$LLAMA_BIN" \
  -m "$MODEL_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  -c "$CTX" \
  -ngl "$NGL" \
  --reasoning-budget 0 \
  --no-webui
)

if [[ "$PLATFORM" == "Darwin" ]]; then
  LLAMA_CMD+=(-b 64 -ub 64 --cpu-moe)
fi

if should_run_llama; then
  bench \
    "llama" \
    "${LLAMA_CMD[@]}"
fi

if should_run_psionic && should_run_llama; then
  PSIONIC_VISIBLE_OUTPUT=$(extract_visible_output "$TMP_DIR/psionic.prompt_cache_hit.json")
  LLAMA_VISIBLE_OUTPUT=$(extract_visible_output "$TMP_DIR/llama.prompt_cache_hit.json")

  if [[ "$PSIONIC_VISIBLE_OUTPUT" == "$LLAMA_VISIBLE_OUTPUT" ]]; then
    echo
    echo "prompt_cache_hit_visible_output_match=true"
  else
    echo
    echo "prompt_cache_hit_visible_output_match=false"
    echo "psionic_visible_output=$PSIONIC_VISIBLE_OUTPUT"
    echo "llama_visible_output=$LLAMA_VISIBLE_OUTPUT"
  fi
fi
