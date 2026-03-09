#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh [--model PATH] [--psionic-bin PATH] [--llama-bin PATH] [--host HOST] [--port PORT] [--ctx N] [--ngl N] [--json-out DIR]

Defaults:
  model:     /home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
  llama-bin: /home/christopherdavid/code/llama.cpp/build/bin/llama-server
  host:      127.0.0.1
  port:      8099
  ctx:       4096
  ngl:       999

Notes:
  - The script prefers ./target/release/psionic-gpt-oss-server, then ./target/debug/psionic-gpt-oss-server.
  - It warms each server once, times the second request, prints the visible response, and reports completion_tokens, seconds, and tok/s.
  - When --json-out DIR is set, the script writes raw responses and derived summaries to DIR.
EOF
}

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../../.." && pwd)

MODEL_PATH=/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
LLAMA_BIN=/home/christopherdavid/code/llama.cpp/build/bin/llama-server
HOST=127.0.0.1
PORT=8099
CTX=4096
NGL=999
PSI_BIN=
JSON_OUT=

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model)
      MODEL_PATH=${2:?missing value for --model}
      shift 2
      ;;
    --psionic-bin)
      PSI_BIN=${2:?missing value for --psionic-bin}
      shift 2
      ;;
    --llama-bin)
      LLAMA_BIN=${2:?missing value for --llama-bin}
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

if [[ -z "$PSI_BIN" ]]; then
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

if [[ ! -x "$PSI_BIN" ]]; then
  echo "psionic binary not executable: $PSI_BIN" >&2
  exit 1
fi

if [[ ! -x "$LLAMA_BIN" ]]; then
  echo "llama.cpp server not executable: $LLAMA_BIN" >&2
  exit 1
fi

TMP_DIR=$(mktemp -d)
SERVER_PID=

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR"
}

trap cleanup EXIT

REQUEST_JSON=$(cat <<'EOF'
{"model":"gpt-oss-20b-mxfp4.gguf","messages":[{"role":"system","content":"You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\nCurrent date: 2026-03-08\n\nReasoning: low\n\n# Valid channels: analysis, final. Channel must be included for every message."},{"role":"developer","content":"Be concise. Output exactly one sentence."},{"role":"user","content":"Reply with exactly this sentence and nothing else: HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website."}],"max_tokens":64,"temperature":0}
EOF
)

wait_for_server() {
  local health_url=$1
  local attempts=0
  until curl -fsS "$health_url" >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    if [[ $attempts -ge 120 ]]; then
      echo "server did not become ready: $health_url" >&2
      exit 1
    fi
    sleep 1
  done
}

run_request() {
  local output_file=$1
  curl --no-buffer -sS "http://$HOST:$PORT/v1/chat/completions" \
    -H 'Content-Type: application/json' \
    -d "$REQUEST_JSON" | tee "$output_file" | jq -r '.choices[0].message.content'
}

write_summary_json() {
  local name=$1
  local response_file=$2
  local elapsed=$3
  local tokens=$4
  local tokps=$5

  [[ -n "$JSON_OUT" ]] || return 0

  cp "$response_file" "$JSON_OUT/$name.response.json"
  jq -n \
    --arg server "$name" \
    --arg model "$MODEL_PATH" \
    --arg endpoint "http://$HOST:$PORT/v1/chat/completions" \
    --arg elapsed_seconds "$elapsed" \
    --arg completion_tokens "$tokens" \
    --arg tokens_per_second "$tokps" \
    --slurpfile response "$response_file" \
    '{
      server: $server,
      model_path: $model,
      endpoint: $endpoint,
      elapsed_seconds: ($elapsed_seconds | tonumber),
      completion_tokens: ($completion_tokens | tonumber),
      tokens_per_second: ($tokens_per_second | tonumber),
      response: $response[0]
    }' > "$JSON_OUT/$name.summary.json"
}

bench() {
  local name=$1
  shift

  echo
  echo "=== $name ==="
  "$@" &
  SERVER_PID=$!

  wait_for_server "http://$HOST:$PORT/health"

  echo "[warmup]"
  run_request "$TMP_DIR/$name.warm.json"

  echo "[timed]"
  local start end elapsed tokens tokps
  start=$(date +%s.%N)
  run_request "$TMP_DIR/$name.json"
  end=$(date +%s.%N)

  tokens=$(jq -r '.usage.completion_tokens' "$TMP_DIR/$name.json")
  elapsed=$(awk "BEGIN { printf \"%.3f\", $end - $start }")
  tokps=$(awk "BEGIN { printf \"%.2f\", $tokens / ($end - $start) }")
  echo "completion_tokens=$tokens seconds=$elapsed tok/s=$tokps"
  write_summary_json "$name" "$TMP_DIR/$name.json" "$elapsed" "$tokens" "$tokps"

  kill "$SERVER_PID" >/dev/null 2>&1 || true
  wait "$SERVER_PID" 2>/dev/null || true
  SERVER_PID=
  sleep 1
}

echo "repo_root=$REPO_ROOT"
echo "model=$MODEL_PATH"
echo "psionic_bin=$PSI_BIN"
echo "llama_bin=$LLAMA_BIN"
echo "endpoint=http://$HOST:$PORT/v1/chat/completions"

bench \
  "psionic" \
  "$PSI_BIN" \
  -m "$MODEL_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  -c "$CTX" \
  -ngl "$NGL" \
  --reasoning-budget 0 \
  --no-webui

bench \
  "llama" \
  "$LLAMA_BIN" \
  -m "$MODEL_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  -c "$CTX" \
  -ngl "$NGL" \
  --reasoning-budget 0 \
  --no-webui
