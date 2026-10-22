#!/usr/bin/env bash
# The GPU half of the V0 oracle (#12371), run on an 8-GPU Google Compute
# machine started through scripts/dev/cloud-budget.py. The weights come from
# the Cloud Storage copy that weights_vm.sh made, never from a workstation.
#
#   oracle_vm.sh setup    mount the local SSDs as /mnt/ssd, install the reference's packages
#   oracle_vm.sh smoke    run the reference self-test (`python model.py`) and the dump
#                         tool's --self-test on one GPU: the FP8 and FP4 TileLang kernels
#   oracle_vm.sh fetch    copy the reference sources and the converted TP8 files from the bucket
#   oracle_vm.sh dump     run the four prompts on eight GPUs and upload the dumps
#   oracle_vm.sh capture  rerun the cases with attention diagnostics and rank partials
#                         (#12404) and upload them beside the original dump
#   oracle_vm.sh holdout  run the frozen G1 v2 holdout corpus (#12405), with every
#                         row's best router scores, and upload it as its own dump
#
# Environment: SCRIPTS (this directory on the VM), DUMP_TOOL_COMMIT (recorded in the report);
# for capture, CAPTURE (bucket folder under the oracle prefix), EXTRA_ROWS and ONLY
# (passed to oracle_dump.py).
set -euo pipefail

REPO=deepseek-ai/DeepSeek-V4.1-Flash-1017
SHORT=1bae0e1
MODELS=gs://openagents-psionic-models/$REPO
ORACLE=gs://openagents-psionic-oracle/deepseek-v41/$SHORT
SSD=/mnt/ssd
REF=$SSD/ref
TP8=$SSD/tp8
OUT=$SSD/out
VENV=$SSD/venv
SCRIPTS=${SCRIPTS:-$(cd "$(dirname "$0")" && pwd)}
PY=$VENV/bin/python
export PATH=$HOME/.local/bin:/usr/local/cuda/bin:$PATH

log() { printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }

setup() {
  if ! mountpoint -q $SSD; then
    mapfile -t disks < <(ls /dev/disk/by-id/google-local-nvme-ssd-* 2>/dev/null || ls /dev/nvme*n1 | grep -v nvme0n1)
    if [ ${#disks[@]} -gt 1 ]; then
      command -v mdadm >/dev/null || { sudo apt-get -qq update && sudo apt-get -qq install -y mdadm >/dev/null; }
      sudo mdadm --create /dev/md0 --level=0 --raid-devices=${#disks[@]} "${disks[@]}" --force --run
      dev=/dev/md0
    else
      dev=${disks[0]}
    fi
    sudo mkfs.ext4 -q -F -E lazy_itable_init=1,lazy_journal_init=1 $dev
    sudo mkdir -p $SSD && sudo mount -o noatime $dev $SSD && sudo chown "$(id -u):$(id -g)" $SSD
  fi
  mkdir -p $OUT $REF
  # TileLang compiles its kernels with nvcc, which needs a host C++ compiler; the
  # deep-learning image ships gcc without g++
  command -v g++ >/dev/null || {
    sudo apt-get -o DPkg::Lock::Timeout=300 update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get -o DPkg::Lock::Timeout=300 install -y -qq --no-install-recommends g++ >/dev/null
  }
  command -v uv >/dev/null || curl -LsSf https://astral.sh/uv/install.sh | sh
  [ -x $PY ] || uv venv -q -p 3.12 $VENV
  # Preserve the versions that produced #12371's retained oracle. Updating a
  # package creates a different comparison environment, even on the same GPU.
  uv pip install -q -p $PY --index-url https://download.pytorch.org/whl/cu129 "torch==2.13.0+cu129"
  uv pip install -q -p $PY "transformers==5.20.2" "tokenizers==0.23.3" \
    "safetensors==0.8.0" "numpy==2.5.4" "sympy==1.14.0" \
    "triton==3.7.1" Pillow "tilelang==0.1.12" tqdm
  $PY -c 'import torch, tilelang; print("torch", torch.__version__, "cuda", torch.version.cuda, "tilelang", tilelang.__version__, torch.cuda.get_device_name(0), torch.cuda.get_device_capability(0))'
  nvidia-smi --query-gpu=name,driver_version --format=csv,noheader | head -1
}

refsrc() {
  [ -f $REF/inference/model.py ] && return
  for f in config.json tokenizer.json tokenizer_config.json model.safetensors.index.json; do
    gcloud storage cp -q $MODELS/$SHORT/$f $REF/$f
  done
  gcloud storage cp -q -r $MODELS/$SHORT/inference $MODELS/$SHORT/encoding $REF/
}

smoke() {
  refsrc
  log "reference self-test (python model.py)"
  (cd $REF/inference && CUDA_VISIBLE_DEVICES=0 $PY model.py) 2>&1 | tail -20
  log "dump tool self-test"
  CUDA_VISIBLE_DEVICES=0 $PY $SCRIPTS/oracle_dump.py --reference $REF --out $OUT/selftest --self-test
  $PY -c "
import json; m = json.load(open('$OUT/selftest/run.json'))
for p in m['prompts']: print(p['id'], p['prompt_tokens'], p['generated_ids'])
from safetensors import safe_open
f = safe_open('$OUT/selftest/tensors-rank0.safetensors', 'pt'); print(len(list(f.keys())), 'tensors')"
  log "smoke done"
}

fetch() {
  refsrc
  log "copy converted TP8 files"
  mkdir -p $TP8
  gcloud storage cp -q -r "$MODELS/$SHORT-tp8-fp4/*" $TP8/
  gcloud storage cp -q $MODELS/$SHORT-tp8-fp4.manifest.json $OUT/tp8-manifest.json
  log "verify"
  $PY -I - "$TP8" "$OUT/tp8-manifest.json" <<'EOF'
import hashlib, json, sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
root, manifest = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text())
def check(f):
    h = hashlib.sha256()
    with open(root / f["path"], "rb") as fh:
        while b := fh.read(64 << 20):
            h.update(b)
    return f["path"], h.hexdigest() == f["sha256"]
with ThreadPoolExecutor(16) as pool:
    bad = [p for p, ok in pool.map(check, manifest["files"]) if not ok]
print("verified", len(manifest["files"]) - len(bad), "files;", "mismatched:", bad)
sys.exit(1 if bad else 0)
EOF
  log "fetch done"
}

dump() {
  log "dump"
  DUMP_TOOL_COMMIT=${DUMP_TOOL_COMMIT:-} $VENV/bin/torchrun --nproc-per-node 8 $SCRIPTS/oracle_dump.py \
    --ckpt $TP8 --reference $REF --out $OUT/dump --upload $ORACLE/dump
  $PY -I $SCRIPTS/oracle_manifest.py --dump $OUT/dump --out $OUT/dump/manifest.json --fixtures $OUT/fixtures
  gcloud storage rsync -r $OUT/fixtures $ORACLE/fixtures
  gcloud storage rsync -r -x '^patched/__pycache__/' $OUT/dump $ORACLE/dump
  log "dump done"
}

capture() {
  : "${CAPTURE:?set CAPTURE, a folder name under $ORACLE}"
  log "capture $CAPTURE"
  # Record which NCCL algorithm and protocol each collective used; logging changes no arithmetic.
  mkdir -p $OUT/$CAPTURE
  NCCL_DEBUG=INFO NCCL_DEBUG_SUBSYS=INIT,TUNING,NVLS,COLL NCCL_DEBUG_FILE=$OUT/$CAPTURE/nccl.%h.%p.log \
    DUMP_TOOL_COMMIT=${DUMP_TOOL_COMMIT:-} $VENV/bin/torchrun --nproc-per-node 8 $SCRIPTS/oracle_dump.py \
    --ckpt $TP8 --reference $REF --out $OUT/$CAPTURE --upload $ORACLE/$CAPTURE \
    --attention-diagnostics --captures-only --only "${ONLY:-}" --extra-rows "${EXTRA_ROWS:-}"
  # No fixtures: a diagnostic capture is far larger than the repository fixture budget.
  $PY -I $SCRIPTS/oracle_manifest.py --dump $OUT/$CAPTURE --out $OUT/$CAPTURE/manifest.json
  gcloud storage rsync -r -x '^patched/__pycache__/' $OUT/$CAPTURE $ORACLE/$CAPTURE
  log "capture done"
}

holdout() {
  : "${CORPUS:?set CORPUS, the frozen holdout corpus JSON}"
  log "holdout"
  # DEST and CORPUS_NAME default to the holdout; the policy 2.2 confirmation
  # set uses its own folder (g1-v2/confirmation-2.2) and name.
  local out=$OUT/g1-v2-${DEST_NAME:-holdout} dest=$ORACLE/g1-v2/${DEST_NAME:-holdout}
  DUMP_TOOL_COMMIT=${DUMP_TOOL_COMMIT:-} $VENV/bin/torchrun --nproc-per-node 8 $SCRIPTS/oracle_dump.py \
    --ckpt $TP8 --reference $REF --out $out --upload $dest \
    --cases-file "$CORPUS" --router-tops 8 --captures-only
  $PY -I $SCRIPTS/oracle_manifest.py --dump $out --out $out/manifest.json
  cp "$CORPUS" $out/${CORPUS_NAME:-holdout-corpus.json}
  gcloud storage rsync -r -x '^patched/__pycache__/' $out $dest
  if [ -n "${SUITE:-}" ]; then
    # G2 v2: the frozen task suite's complete greedy answers, to the end token or cap
    local g2=$OUT/g2-v2-oracle g2dest=$ORACLE/g2-v2/oracle
    DUMP_TOOL_COMMIT=${DUMP_TOOL_COMMIT:-} $VENV/bin/torchrun --nproc-per-node 8 $SCRIPTS/oracle_dump.py \
      --ckpt $TP8 --reference $REF --out $g2 --upload $g2dest --cases-file "$SUITE"
    cp "$SUITE" $g2/task-suite.json
    gcloud storage rsync -r -x '^patched/__pycache__/' $g2 $g2dest
  fi
  log "holdout done"
}

"$@"
