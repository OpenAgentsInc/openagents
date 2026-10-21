#!/usr/bin/env bash
# W7 (#12394): one DeepSeek-V4.1 stage on one RTX PRO 6000 (SM120). Run only
# on its guarded g4-standard-48 machine. Weights come from the models bucket;
# only the stage's shards are copied, and the others become sparse stand-ins
# that keep their real headers, so the pinned checkpoint opens.
#
#   stage_vm.sh setup             install the toolchain and build tools
#   stage_vm.sh fetch START END   copy the shards of layers START..END, the
#                                 oracle dump, and the source tarball
#   stage_vm.sh build             build the kernel tests and the stage runner for sm_120
#   stage_vm.sh kernels           run W4-W6's kernel suites (W8 evidence)
#   stage_vm.sh run START END [ARGS]  run the stage and compare it
#   stage_vm.sh upload            copy results to the run's bucket prefix
#
# Environment: RUN (the bucket run prefix under w7/), SRC (the source tarball URL).
set -euo pipefail
ROOT=/mnt/stage
MODELS=gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1
ORACLE=gs://openagents-psionic-oracle/deepseek-v41/1bae0e1
OUT=$ROOT/out
CKPT=$ROOT/checkpoint
SRCDIR=$ROOT/src
export PATH="$HOME/.cargo/bin:/usr/local/cuda/bin:$PATH"
export CARGO_TARGET_DIR=$ROOT/target CARGO_INCREMENTAL=0 PSI_CUDA_ARCH=sm_120
# The image's CUDA 12.9 keeps CCCL at include/cub; Psionic's kernels include
# CUDA 13's include/cccl/cub layout.
export NVCC_PREPEND_FLAGS="-I$ROOT/cccl-shim"
log() { printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*" | tee -a "$OUT/log.txt"; }

setup() {
  sudo mkdir -p "$ROOT" && sudo chown "$(id -u):$(id -g)" "$ROOT"
  mkdir -p "$OUT" "$CKPT" "$ROOT/oracle" "$SRCDIR"
  sudo sed -i -E 's|http://|https://|g;s|[a-z0-9-]+\.gce\.archive\.ubuntu\.com|archive.ubuntu.com|g' \
    /etc/apt/sources.list.d/ubuntu.sources 2>/dev/null || true
  sudo apt-get -o DPkg::Lock::Timeout=300 -o Acquire::Retries=2 -qq update
  sudo DEBIAN_FRONTEND=noninteractive apt-get -o DPkg::Lock::Timeout=300 -qq install -y \
    build-essential pkg-config libssl-dev curl >/dev/null
  if ! [ -x "$HOME/.cargo/bin/rustup" ]; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain 1.97.1
  fi
  mkdir -p "$ROOT/cccl-shim"
  ln -sfn /usr/local/cuda/include "$ROOT/cccl-shim/cccl"
  rustc -vV > "$OUT/machine.txt"
  nvcc --version >> "$OUT/machine.txt"
  nvidia-smi >> "$OUT/machine.txt"
  lscpu >> "$OUT/machine.txt"
  free -g >> "$OUT/machine.txt"
}

fetch() {
  local start=$1 end=$2
  mkdir -p "$OUT" "$CKPT" "$ROOT/oracle" "$SRCDIR"
  gcloud storage cp -q "$SRC" "$ROOT/src.tar.gz"
  tar -xzf "$ROOT/src.tar.gz" -C "$SRCDIR"
  for f in model.safetensors.index.json config.json tokenizer.json tokenizer_config.json; do
    gcloud storage cp -q "$MODELS/$f" "$CKPT/$f"
  done
  python3 - "$CKPT/model.safetensors.index.json" "$start" "$end" > "$ROOT/shards" <<'PY'
import json, sys
index, start, end = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
want = set()
for name, shard in json.load(open(index))["weight_map"].items():
    p = name.split(".")
    if p[0] == "layers" and start <= int(p[1]) < end:
        want.add(shard)
    elif (name == "embed.weight" and start == 0) or (name in ("norm.weight", "head.weight") and end == 40):
        want.add(shard)
print("\n".join(sorted(want)))
PY
  log "copying $(wc -l < "$ROOT/shards") shards for layers $start..$end"
  gcloud storage rsync -q "$ORACLE/dump" "$ROOT/oracle" -r > "$OUT/copy-oracle.log" 2>&1 &
  local oracle_pid=$!
  sed "s|^|$MODELS/|" "$ROOT/shards" | gcloud storage cp -q -I "$CKPT/"
  wait "$oracle_pid"
  # Sparse stand-ins for the other shards: the real header, then zeros to
  # the pinned size. The stage never reads their tensors.
  python3 - "$CKPT" "$MODELS" "$ROOT/shards" <<'PY'
import json, os, subprocess, sys
ckpt, models, shards = sys.argv[1], sys.argv[2], set(open(sys.argv[3]).read().split())
index = json.load(open(os.path.join(ckpt, "model.safetensors.index.json")))
for shard in sorted(set(index["weight_map"].values())):
    path = os.path.join(ckpt, shard)
    if shard in shards or os.path.exists(path):
        continue
    url = f"{models}/{shard}"
    size = int(subprocess.check_output(["gcloud", "storage", "ls", "-l", url]).split()[0])
    head = subprocess.check_output(["gcloud", "storage", "cat", "-r", "0-7", url])
    n = int.from_bytes(head, "little")
    header = subprocess.check_output(["gcloud", "storage", "cat", "-r", f"0-{7 + n}", url])
    with open(path, "wb") as f:
        f.write(header)
        f.truncate(size)
PY
  log "fetch complete"
  df -h "$ROOT" >> "$OUT/machine.txt"
}

build() {
  cd "$SRCDIR"
  log "building for $PSI_CUDA_ARCH"
  cargo build --release -p psionic-backend-cuda --example deepseek_v41_stage_run > "$OUT/build.log" 2>&1
  cargo test --release -p psionic-backend-cuda --lib --no-run >> "$OUT/build.log" 2>&1
  log "build complete"
}

kernels() {
  cd "$SRCDIR"
  for suite in v41_matvec v41_mix v41_attention v41_stage; do
    log "kernel suite $suite"
    PSIONIC_REQUIRE_CUDA=1 cargo test --release -p psionic-backend-cuda --lib "$suite" -- \
      --test-threads=1 --nocapture > "$OUT/kernels-$suite.log" 2>&1 || log "suite $suite failed"
    grep -E 'test result' "$OUT/kernels-$suite.log" | tee -a "$OUT/log.txt"
  done
  for t in v41_decode_bandwidth v41_mix_decode_latency v41_attention_decode_latency; do
    log "ignored test $t"
    PSIONIC_REQUIRE_CUDA=1 cargo test --release -p psionic-backend-cuda --lib "$t" -- \
      --ignored --test-threads=1 --nocapture > "$OUT/kernels-$t.log" 2>&1 || log "$t failed"
  done
}

run() {
  local start=$1 end=$2
  shift 2
  cd "$SRCDIR"
  log "stage run $start..$end $*"
  V41_SOURCE_COMMIT=$(cat "$SRCDIR/COMMIT" 2>/dev/null || echo unknown) \
    "$CARGO_TARGET_DIR/release/examples/deepseek_v41_stage_run" \
    --checkpoint "$CKPT" --oracle "$ROOT/oracle" --out "$OUT/stage-$start-$end" \
    --layers "$start-$end" "$@" > "$OUT/stage-$start-$end.log" 2>&1 &
  local pid=$!
  # Upload the report as it grows, so a reclaimed spot machine loses little.
  while kill -0 "$pid" 2>/dev/null; do
    sleep 60
    nvidia-smi --query-gpu=memory.used,memory.total,utilization.gpu --format=csv,noheader \
      >> "$OUT/gpu-memory-$start-$end.csv" 2>/dev/null || true
    upload >/dev/null 2>&1 || true
  done
  wait "$pid" && log "stage run $start..$end finished" || log "stage run $start..$end failed"
  upload
}

upload() {
  gcloud storage rsync -q -r "$OUT" "$RUN/out"
}

case "${1:-}" in
  setup) setup ;;
  fetch) fetch "$2" "$3" ;;
  build) build ;;
  kernels) kernels; upload ;;
  run) shift; run "$@" ;;
  upload) upload ;;
  *) echo 'Usage: stage_vm.sh setup|fetch START END|build|kernels|run START END [ARGS]|upload' >&2; exit 2 ;;
esac
