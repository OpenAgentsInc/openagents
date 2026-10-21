#!/usr/bin/env bash
# G1 v2 calibration on one g4-standard-192 (four RTX PRO 6000) and the CPU
# reference control on a CPU machine, each started through
# scripts/dev/cloud-budget.py (#12405). Weights and the oracle come from their
# buckets, never from a workstation.
#
#   qualify_vm.sh setup   stripe the local SSDs, copy the checkpoint, the oracle
#                         dump, and the worker build; verify the GPU architecture
#   qualify_vm.sh run-gpu calibration (the eight cases with observations) and
#                         the four negative controls, on a g4-standard-192
#   qualify_vm.sh run-cpu the CPU reference control, on a CPU machine with
#                         local SSDs; both upload everything under RESULTS
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
  command -v mdadm >/dev/null || { sudo apt-get -qq update && sudo apt-get -qq install -y mdadm >/dev/null; }
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
    nvidia-smi --query-gpu=name,driver_version,compute_cap --format=csv,noheader || echo "no GPU"
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
  gcloud storage rsync -q -r -x 'record/.*\.u32$' $ROOT/out "$RESULTS" || true
}

# GPU calibration: each case with observations, then the four negative
# controls on the chat prompt. One group per run, so a spot machine that is
# reclaimed loses only the run in progress: a run whose report is already
# under RESULTS is skipped.
run-gpu() {
  : "${MANIFEST:?}" "${RESULTS:?}"
  local name
  for case in p1-chat-thinking p2-code e-window-127 e-window-128 e-window-129 e-engram p3-2k p4-20k; do
    name=calibration-$case
    gcloud storage ls "$RESULTS/$name/report.json" >/dev/null 2>&1 && continue
    qualify $ROOT/out/$name --observe --gpus 0,1,2,3 --cases $case
  done
  for control in wrong-weights wrong-routing wrong-rope stale-cache; do
    name=control-$control
    gcloud storage ls "$RESULTS/$name/report.json" >/dev/null 2>&1 && continue
    qualify $ROOT/out/$name --gpus 0,1,2,3 --cases p1-chat-thinking --control $control
  done
  log "GPU runs done"
}

# The CPU reference control (the CPU port, bf16-tp8 profile) on the six
# shorter cases; the 2k and 20k prompts take more than a day on CPU.
run-cpu() {
  : "${MANIFEST:?}" "${RESULTS:?}"
  qualify $ROOT/out/reference-cpu --backend cpu --arithmetic bf16-tp8 --step-secs 36000 \
    --cases p1-chat-thinking,p2-code,e-window-127,e-window-128,e-window-129,e-engram
  log "CPU reference done"
}

"$@"
