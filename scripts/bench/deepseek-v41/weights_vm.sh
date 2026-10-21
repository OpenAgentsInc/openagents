#!/usr/bin/env bash
# The CPU half of the V0 oracle (#12371), run on a Google Compute CPU machine
# with local SSDs, started through scripts/dev/cloud-budget.py. It never runs
# on a workstation: the weights go from Hugging Face to Google Cloud only.
#
#   weights_vm.sh setup      mount the local SSDs as /mnt/ssd, install Python tools
#   weights_vm.sh fetch      download the pinned revision, verify every file
#                            against Hugging Face, upload it with its manifest
#   weights_vm.sh parity     write the dequantization parity file for #12373
#   weights_vm.sh convert    run the reference convert.py (TP8, FP4 experts)
#                            and upload the converted files with a manifest
#   weights_vm.sh rustcheck  build and run psionic-models' weights check (#12373)
#
# Environment: SCRIPTS (this directory on the VM, default: next to this file).
set -euo pipefail

REPO=deepseek-ai/DeepSeek-V4.1-Flash-1017
REV=1bae0e1088993b9b82de08f59de5be5a34e42170
SHORT=1bae0e1
MODELS=gs://openagents-psionic-models/$REPO
ORACLE=gs://openagents-psionic-oracle/deepseek-v41/$SHORT
SSD=/mnt/ssd
HF=$SSD/hf
TP8=$SSD/tp8
OUT=$SSD/out
VENV=$SSD/venv
SCRIPTS=${SCRIPTS:-$(cd "$(dirname "$0")" && pwd)}
PY=$VENV/bin/python

log() { printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }

setup() {
  if ! mountpoint -q $SSD; then
    # local SSDs as RAID 0, or else a persistent disk attached with device-name=data
    mapfile -t disks < <(ls /dev/disk/by-id/google-local-nvme-ssd-* 2>/dev/null || ls /dev/disk/by-id/google-data 2>/dev/null || true)
    [ ${#disks[@]} -gt 0 ] || { echo "no local SSDs or data disk" >&2; exit 1; }
    if [ ${#disks[@]} -gt 1 ]; then
      sudo apt-get -qq update && sudo apt-get -qq install -y mdadm >/dev/null
      sudo mdadm --create /dev/md0 --level=0 --raid-devices=${#disks[@]} "${disks[@]}" --force --run
      dev=/dev/md0
    else
      dev=${disks[0]}
    fi
    sudo mkfs.ext4 -q -F -E lazy_itable_init=1,lazy_journal_init=1 $dev
    sudo mkdir -p $SSD && sudo mount -o noatime $dev $SSD && sudo chown "$(id -u):$(id -g)" $SSD
  fi
  mkdir -p $OUT
  command -v uv >/dev/null || curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH=$HOME/.local/bin:$PATH
  [ -x $PY ] || uv venv -q -p 3.12 $VENV
  uv pip install -q -p $PY "huggingface_hub[hf_xet]>=0.35" "safetensors>=0.7.0" tqdm numpy
  uv pip install -q -p $PY --index-url https://download.pytorch.org/whl/cpu "torch>=2.10.0"
  $PY -c 'import torch, safetensors, huggingface_hub; print("torch", torch.__version__, "safetensors", safetensors.__version__, "hub", huggingface_hub.__version__)'
}

fetch() {
  log "download $REPO@$REV"
  HF_XET_HIGH_PERFORMANCE=1 $VENV/bin/hf download $REPO --revision $REV --local-dir $HF --max-workers 32 >/dev/null
  log "hash and verify"
  $PY -I $SCRIPTS/weights_manifest.py --dir $HF --out $OUT/weights-manifest.json
  log "upload"
  gcloud storage rsync -r --exclude='^\.cache/' $HF $MODELS/$SHORT
  gcloud storage cp $OUT/weights-manifest.json $MODELS/$SHORT.manifest.json
  log "fetch done"
}

parity() {
  $PY -I $SCRIPTS/dequant_parity.py --hf-dir $HF --reference-dir $HF/inference --out $OUT/dequant-parity.json
  gcloud storage cp $OUT/dequant-parity.json $ORACLE/dequant-parity.json
}

convert() {
  log "convert to TP8 with FP4 experts"
  rm -rf $TP8
  (cd $HF/inference && $PY convert.py --hf-ckpt-path $HF --save-path $TP8 --model-parallel 8 \
    --expert-dtype fp4 --tokenizer-path $HF)
  $PY -I $SCRIPTS/weights_manifest.py --dir $TP8 --out $OUT/tp8-manifest.json --no-hf
  log "upload converted"
  gcloud storage rsync -r $TP8 $MODELS/$SHORT-tp8-fp4
  gcloud storage cp $OUT/tp8-manifest.json $MODELS/$SHORT-tp8-fp4.manifest.json
  log "convert done"
}

rustcheck() {
  if ! command -v cargo >/dev/null; then
    sudo apt-get -qq install -y build-essential pkg-config git >/dev/null
    curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal
  fi
  . "$HOME/.cargo/env"
  if [ ! -d $SSD/openagents ]; then
    git clone -q --depth 1 --filter=blob:none --sparse https://github.com/OpenAgentsInc/openagents $SSD/openagents
    git -C $SSD/openagents sparse-checkout set crates/psionic
  fi
  git -C $SSD/openagents pull -q --depth 1 origin main
  git -C $SSD/openagents rev-parse HEAD > $OUT/rustcheck-commit.txt
  cd $SSD/openagents/crates/psionic
  export CARGO_TARGET_DIR=$SSD/target
  cargo build -q --release -p psionic-models --example deepseek_v41_weights_check
  log "weights check: sha256 and ranges"
  cargo run -q --release -p psionic-models --example deepseek_v41_weights_check -- \
    $HF --verify-sha256 --ranges 0-4,10-14,35-39 > $OUT/weights-check.json || echo "check exit $?"
  if [ -f $OUT/dequant-parity.json ]; then
    log "weights check: oracle parity"
    cargo run -q --release -p psionic-models --example deepseek_v41_weights_check -- \
      $HF --oracle $OUT/dequant-parity.json > $OUT/weights-check-oracle.json || echo "check exit $?"
  fi
  gcloud storage cp $OUT/weights-check*.json $OUT/rustcheck-commit.txt $ORACLE/weights-check/
  log "rustcheck done"
}

"$@"
