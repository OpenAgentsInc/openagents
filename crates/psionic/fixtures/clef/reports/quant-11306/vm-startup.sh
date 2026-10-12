#!/bin/bash
# GCE startup script for the #11306 Clef 27B quantization comparison.
#
# Builds psionic-openai-server at a pinned openagents revision with the CUDA
# lane, downloads the released 27B Q4_K_M and the standard upstream Q8_0 (the
# same ggml-org/Clef-GGUF conversion, so the same checkpoint and head), checks
# their sizes and records SHA-256, then serves both decision models on loopback:
#   127.0.0.1:18201  Clef-Q4_K_M.gguf  (released artifact)
#   127.0.0.1:18202  Clef-Q8_0.gguf    (larger standard quantization)
# Only one server runs at a time (serve.sh <q4|q8>), so latency is not shared.
# The goldens harness runs elsewhere and reaches the port through an SSH tunnel,
# so product notes and keys stay off this VM.
#
# Progress: /var/log/clef-11306/state (one word) and /var/log/clef-11306/*.log
set -euo pipefail
REV="${REV:-$(curl -sf -H 'Metadata-Flavor: Google' http://metadata.google.internal/computeMetadata/v1/instance/attributes/oa-rev || echo main)}"
ROOT=/opt/clef-11306
LOG=/var/log/clef-11306
mkdir -p "$ROOT/models" "$LOG"
state() { echo "$1" > "$LOG/state"; echo "$(date -u +%FT%TZ) $1" >> "$LOG/progress.log"; }

state packages
export DEBIAN_FRONTEND=noninteractive
apt-get update -q >"$LOG/apt.log" 2>&1 || true
apt-get install -y -q build-essential pkg-config libssl-dev git curl clang >>"$LOG/apt.log" 2>&1

state models
fetch() { # name expected_bytes
  local url="https://huggingface.co/ggml-org/Clef-GGUF/resolve/main/$1"
  if [ ! -f "$ROOT/models/$1" ] || [ "$(stat -c %s "$ROOT/models/$1")" != "$2" ]; then
    curl -sfL --retry 5 -o "$ROOT/models/$1.partial" "$url"
    mv "$ROOT/models/$1.partial" "$ROOT/models/$1"
  fi
  [ "$(stat -c %s "$ROOT/models/$1")" = "$2" ] || { state "size-mismatch-$1"; exit 1; }
}
( fetch Clef-Q4_K_M.gguf 19232219200; fetch Clef-Q8_0.gguf 28732215360
  cd "$ROOT/models" && sha256sum Clef-Q4_K_M.gguf Clef-Q8_0.gguf > "$LOG/models.sha256" ) \
  >"$LOG/models.log" 2>&1 &
MODELS_PID=$!

state rust
if [ ! -x /root/.cargo/bin/cargo ]; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal >"$LOG/rustup.log" 2>&1
fi
export PATH=/root/.cargo/bin:/usr/local/cuda/bin:$PATH

state clone
if [ ! -d "$ROOT/src/.git" ]; then
  git clone -q --filter=blob:none --no-checkout https://github.com/OpenAgentsInc/openagents.git "$ROOT/src"
  git -C "$ROOT/src" sparse-checkout set crates/psionic
fi
# REV must be a full commit SHA (a short one cannot be fetched).
git -C "$ROOT/src" fetch -q origin main
git -C "$ROOT/src" fetch -q origin "$REV" || true
git -C "$ROOT/src" checkout -q "$REV"
git -C "$ROOT/src" rev-parse HEAD > "$LOG/rev"

state build
# The CUDA kernels include <cccl/cub/...>; the deep-learning image's CUDA 12.9
# keeps cub directly under include/, so give nvcc a directory that has cccl/.
mkdir -p "$ROOT/cccl-shim"
ln -sfn /usr/local/cuda/targets/x86_64-linux/include "$ROOT/cccl-shim/cccl"
export NVCC_PREPEND_FLAGS="-I$ROOT/cccl-shim"
( cd "$ROOT/src/crates/psionic" && cargo build --release -p psionic-serve --bin psionic-openai-server ) \
  >"$LOG/build.log" 2>&1
sha256sum "$ROOT/src/crates/psionic/target/release/psionic-openai-server" > "$LOG/binary.sha256"

state wait-models
wait "$MODELS_PID" || { state models-failed; exit 1; }

cat > "$ROOT/serve.sh" <<'EOF'
#!/bin/bash
# serve.sh q4|q8 [extra psionic flags]: stop any Clef server, start one.
set -euo pipefail
ROOT=/opt/clef-11306; LOG=/var/log/clef-11306
which="$1"; shift || true
case "$which" in
  q4) model=Clef-Q4_K_M.gguf; port=18201 ;;
  q8) model=Clef-Q8_0.gguf; port=18202 ;;
  *) echo "usage: serve.sh q4|q8" >&2; exit 2 ;;
esac
pkill -f psionic-openai-server || true; sleep 3
setsid nohup "$ROOT/src/crates/psionic/target/release/psionic-openai-server" \
  -m "$ROOT/models/$model" --host 127.0.0.1 --port "$port" \
  --decision-device cuda --decision-chunk 2048 --decision-max-tokens 65536 "$@" \
  > "$LOG/server-$which.log" 2>&1 < /dev/null &
for i in $(seq 300); do curl -sf "localhost:$port/health" >/dev/null && break; sleep 2; done
nvidia-smi --query-gpu=memory.used --format=csv,noheader > "$LOG/vram-$which-after-load.txt"
echo "up $which on $port"
EOF
chmod +x "$ROOT/serve.sh"
state ready

# Unattended run: when the instance carries the runner as metadata (oa-runner)
# and oa-autorun=1, start it in the background. The startup script runs on every
# boot, and run-goldens.sh skips rows that already finished, so a restarted VM
# resumes where it stopped. Keys come from Secret Manager inside the runner.
md=http://metadata.google.internal/computeMetadata/v1/instance/attributes
if [ "$(curl -sf -H 'Metadata-Flavor: Google' "$md/oa-autorun" || true)" = 1 ] \
   && curl -sf -H 'Metadata-Flavor: Google' "$md/oa-runner" > "$ROOT/run-goldens.sh.new"; then
  mv "$ROOT/run-goldens.sh.new" "$ROOT/run-goldens.sh"; chmod +x "$ROOT/run-goldens.sh"
  pgrep -f 'run-goldens.sh all' >/dev/null || \
    setsid nohup "$ROOT/run-goldens.sh" all > "$LOG/run.log" 2>&1 < /dev/null &
fi
