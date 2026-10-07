#!/usr/bin/env bash
# Run a Pylon on this machine: Psionic serving a model on CUDA, and the
# pylon provider publishing beacons and answering free jobs from it.
#
#   scripts/pylon-psionic.sh setup                 # CUDA libraries, the model, both builds
#   scripts/pylon-psionic.sh start --allow NPUB    # start both as transient user units
#   scripts/pylon-psionic.sh status
#   scripts/pylon-psionic.sh stop                  # stop both; nothing stays installed
#   scripts/pylon-psionic.sh whoami                # this pylon's keys, as JSON
#
# Both run as transient systemd user units (`pylon-psionic` and
# `pylon-provider`), so `stop`, a reboot, or `systemctl --user stop` removes
# them and no unit file is written. Without systemd they run under nohup and
# `stop` kills them by their PID files.
#
# Environment:
#   PYLON_DIR       State: CUDA links, the model, logs (default ~/.openagents/compute/run).
#   PYLON_TARGET    Cargo target directory (default ~/work/openagents-target-pylon).
#   PYLON_RELAY     Relay (default wss://relay.openagents.com).
#   PYLON_PORT      Psionic's loopback port (default 18080).
#   PYLON_SLUG      The beacon's name (default the host name).
#   PYLON_LABEL     The beacon's label (default "<host> (Psionic)").
#   PYLON_BACKEND   cuda, metal, or cpu (default cuda).
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
dir="${PYLON_DIR:-$HOME/.openagents/compute/run}"
target="${PYLON_TARGET:-$HOME/work/openagents-target-pylon}"
relay="${PYLON_RELAY:-wss://relay.openagents.com}"
port="${PYLON_PORT:-18080}"
host="$(hostname -s 2>/dev/null || hostname)"
slug="${PYLON_SLUG:-$host}"
label="${PYLON_LABEL:-$host (Psionic)}"
backend="${PYLON_BACKEND:-cuda}"
model_name="qwen3.5-0.8b-q8_0"
model="$dir/models/$model_name.gguf"
# Qwen3.5 0.8B Q8_0, the Psionic qwen35 pilot row (Ollama registry blob).
model_sha="afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5"
model_url="https://registry.ollama.ai/v2/library/qwen3.5/blobs/sha256:$model_sha"

mkdir -p "$dir/models" "$dir/cuda"

cuda_env() {
  [ "$backend" = cuda ] || return 0
  local r="$dir/cuda"
  if [ -x "$r/nvcc/bin/nvcc" ]; then
    export NVCC="$r/nvcc/bin/nvcc"
    export NVCC_PREPEND_FLAGS="-I$r/crt/include -I$r/cudart/include -I$r/cccl/include"
    export LIBRARY_PATH="$r/cudart/lib:$r/cublas-lib/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
    export LD_LIBRARY_PATH="$r/cudart/lib:$r/cublas-lib/lib:/run/opengl-driver/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  fi
  export PSI_CUDA_ARCH="${PSI_CUDA_ARCH:-sm_89}"
}

setup_cuda() {
  [ "$backend" = cuda ] || return 0
  if command -v nvcc >/dev/null 2>&1 || [ -x /usr/local/cuda/bin/nvcc ] || [ -x /opt/cuda/bin/nvcc ]; then
    echo "using the system CUDA toolkit"
    return 0
  fi
  command -v nix >/dev/null 2>&1 || { echo "no nvcc and no nix; install the CUDA 13 toolkit" >&2; exit 1; }
  echo "fetching CUDA 13 from nixpkgs into $dir/cuda"
  local r="$dir/cuda"
  # `-o X` on a non-default output links X-lib, so cublas becomes cublas-lib.
  for pair in nvcc:cuda_nvcc crt:cuda_crt cudart:cuda_cudart cccl:cuda_cccl cublas:libcublas.lib; do
    NIXPKGS_ALLOW_UNFREE=1 nix build --impure -o "$r/${pair%%:*}" "nixpkgs#cudaPackages_13.${pair#*:}"
  done
}

setup_model() {
  if [ -f "$model" ] && [ "$(sha256sum "$model" | cut -d' ' -f1)" = "$model_sha" ]; then
    echo "model present: $model"
    return 0
  fi
  echo "downloading $model_name"
  curl -fL --retry 3 -o "$model.part" "$model_url"
  [ "$(sha256sum "$model.part" | cut -d' ' -f1)" = "$model_sha" ] || { echo "model digest mismatch" >&2; exit 1; }
  mv "$model.part" "$model"
}

build() {
  cuda_env
  echo "building psionic-openai-server from crates/psionic"
  nice cargo build --release --manifest-path "$repo/crates/psionic/Cargo.toml" \
    -p psionic-serve --bin psionic-openai-server --target-dir "$target/psionic"
  echo "building pylon"
  nice cargo build --release --manifest-path "$repo/Cargo.toml" -p pylon --bin pylon \
    --target-dir "$target/openagents"
}

have_systemd() {
  command -v systemd-run >/dev/null 2>&1 && systemctl --user is-system-running >/dev/null 2>&1
}

launch() {
  local name="$1"
  shift
  if have_systemd; then
    systemctl --user stop "$name" >/dev/null 2>&1 || true
    systemctl --user reset-failed "$name" >/dev/null 2>&1 || true
    systemd-run --user --unit="$name" --collect --quiet \
      --setenv=LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}" \
      --setenv=OPENAGENTS_PYLON_HOME="${OPENAGENTS_PYLON_HOME:-$HOME/.openagents/compute}" \
      -p StandardOutput="append:$dir/$name.log" -p StandardError="append:$dir/$name.log" \
      "$@"
  else
    nohup "$@" >>"$dir/$name.log" 2>&1 </dev/null &
    echo $! >"$dir/$name.pid"
  fi
}

stop_one() {
  local name="$1"
  if have_systemd; then
    systemctl --user stop "$name" >/dev/null 2>&1 || true
  fi
  if [ -f "$dir/$name.pid" ]; then
    kill "$(cat "$dir/$name.pid")" 2>/dev/null || true
    rm -f "$dir/$name.pid"
  fi
}

start() {
  local allow=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --allow) allow+=(--allow "$2"); shift 2 ;;
      --allow-any) allow+=(--allow-any); shift ;;
      *) echo "unknown option $1" >&2; exit 2 ;;
    esac
  done
  [ ${#allow[@]} -gt 0 ] || { echo "name who may send jobs: --allow NPUB (or --allow-any)" >&2; exit 2; }
  cuda_env
  local server="$target/psionic/release/psionic-openai-server"
  local pylon="$target/openagents/release/pylon"
  [ -x "$server" ] && [ -x "$pylon" ] || { echo "run setup first" >&2; exit 1; }
  launch pylon-psionic "$server" -m "$model" --backend "$backend" \
    --host 127.0.0.1 --port "$port" --mesh-coordination disabled
  for _ in $(seq 1 60); do
    curl -fs "http://127.0.0.1:$port/v1/models" >/dev/null 2>&1 && break
    sleep 1
  done
  curl -fs "http://127.0.0.1:$port/v1/models" >/dev/null || { echo "psionic did not start; see $dir/pylon-psionic.log" >&2; exit 1; }
  launch pylon-provider "$pylon" serve --relay "$relay" --engine "http://127.0.0.1:$port" \
    --model "$model_name" --pylon "$slug" --label "$label" "${allow[@]}"
  sleep 3
  status
}

status() {
  if have_systemd; then
    systemctl --user --no-pager --lines=0 status pylon-psionic pylon-provider 2>/dev/null | grep -E '●|Active:' || true
  fi
  tail -n 3 "$dir/pylon-provider.log" 2>/dev/null || true
  "$target/openagents/release/pylon" whoami 2>/dev/null | head -1 || true
}

case "${1:-}" in
  setup) setup_cuda; setup_model; build ;;
  start) shift; start "$@" ;;
  stop) stop_one pylon-provider; stop_one pylon-psionic; echo stopped ;;
  status) status ;;
  whoami) "$target/openagents/release/pylon" --json whoami ;;
  *) sed -n '2,24p' "$0"; exit 2 ;;
esac
