#!/usr/bin/env bash
# G1 v2 calibration on one g4-standard-192 (four RTX PRO 6000) started through
# scripts/dev/cloud-budget.py (#12405). Weights and the oracle come from their
# buckets, never from a workstation.
#
#   qualify_vm.sh setup   stripe the local SSDs, copy the checkpoint, the oracle
#                         dump, and the worker build; verify the GPU architecture
#   qualify_vm.sh run     calibration (the eight cases with observations), the
#                         four negative controls, and the CPU reference control;
#                         upload everything under RESULTS
#
# Environment: WORKER (the build's gs:// folder, .../COMMIT/sm_120),
# MANIFEST (the one-quad group manifest's SHA-256), RESULTS (gs:// folder).
set -euo pipefail

ROOT=/mnt/stage
MODELS=gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1
ORACLE=gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump
BIN=$ROOT/bin/psionic-v41-stage
export LD_LIBRARY_PATH=$ROOT/bin/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
log() { printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }

setup() {
  : "${WORKER:?}" "${MANIFEST:?}"
  if ! mountpoint -q $ROOT; then
    mapfile -t disks < <(ls /dev/disk/by-id/google-local-nvme-ssd-*)
    sudo mdadm --create /dev/md0 --level=0 --raid-devices=${#disks[@]} "${disks[@]}" --force --run
    sudo mkfs.ext4 -q -F -E lazy_itable_init=1,lazy_journal_init=1 /dev/md0
    sudo mkdir -p $ROOT && sudo mount -o noatime /dev/md0 $ROOT && sudo chown "$(id -u):$(id -g)" $ROOT
  fi
  mkdir -p $ROOT/weights/manifests $ROOT/oracle $ROOT/bin/lib $ROOT/out
  log "copy"
  gcloud storage cp -q "$WORKER/psionic-v41-stage" "$WORKER/BUILD.json" $ROOT/bin/
  gcloud storage cp -q "$WORKER/lib/*" $ROOT/bin/lib/
  gcloud storage cp -q "$WORKER/manifests/$MANIFEST.json" $ROOT/weights/manifests/
  chmod 755 $BIN
  gcloud storage rsync -q -r $ORACLE $ROOT/oracle &
  gcloud storage rsync -q -r $MODELS $ROOT/weights
  wait
  log "verify"
  {
    nvidia-smi --query-gpu=name,driver_version,compute_cap --format=csv,noheader
    sha256sum $BIN
    cat $ROOT/bin/BUILD.json
    # The kernels' architecture as built, not as requested (#12407 F1).
    /usr/local/cuda/bin/cuobjdump --list-elf $BIN 2>/dev/null | grep -o 'sm_[0-9]*' | sort | uniq -c || true
    /usr/local/cuda/bin/cuobjdump --list-elf $ROOT/bin/lib/*.so* 2>/dev/null | grep -o 'sm_[0-9]*' | sort -u | tr '\n' ' ' || true
  } > $ROOT/out/identity.txt 2>&1
  cat $ROOT/out/identity.txt
  log "setup done"
}

qualify() {
  local out=$1; shift
  timeout 21600 $BIN qualify --weights $ROOT/weights --manifest "sha256:$MANIFEST" \
    --oracle $ROOT/oracle --out "$out" --max-context 20608 "$@" > "$out.log" 2>&1 \
    || log "qualify $out exited $?"
  gcloud storage rsync -q -r -x 'record/.*\.f32$|record/.*\.u32$' $ROOT/out "$RESULTS" || true
}

run() {
  : "${MANIFEST:?}" "${RESULTS:?}"
  local main=p1-chat-thinking,p2-code,e-window-127,e-window-128,e-window-129,e-engram
  # The CPU reference control runs beside the GPU work: it uses the cores,
  # the GPU runs use the devices.
  (RAYON_NUM_THREADS=180 qualify $ROOT/out/reference-cpu --backend cpu --arithmetic bf16-tp8 \
    --cases $main --step-secs 36000) &
  local cpu=$!
  qualify $ROOT/out/calibration --observe --gpus 0,1,2,3 --cases $main,p3-2k,p4-20k
  for control in wrong-weights wrong-routing wrong-rope stale-cache; do
    qualify $ROOT/out/control-$control --gpus 0,1,2,3 --cases p1-chat-thinking --control $control
  done
  log "GPU runs done; waiting for the CPU reference"
  wait $cpu || true
  log "run done"
}

"$@"
