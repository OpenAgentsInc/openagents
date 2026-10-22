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
# BUILD (the BUILD.json SHA-256 v41-stage-build.sh printed: the independent
# pin, never read from the bucket), MANIFEST (the one-quad group manifest's
# SHA-256), RESULTS (gs:// folder). v41-build-check.py from scripts/pylon/
# sits beside this script on the machine. FRONT (optional) is a newer
# psionic-v41-stage that drives the pinned worker with --worker; POLICY is the
# frozen policy file for run-holdout.
set -euo pipefail

ROOT=/mnt/stage
MODELS=${MODELS:-gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1}
ORACLE=gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump
BIN=$ROOT/bin/psionic-v41-stage
export LD_LIBRARY_PATH=$ROOT/bin/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
log() { printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }

setup() {
  : "${WORKER:?}" "${BUILD:?}" "${MANIFEST:?}"
  command -v mdadm >/dev/null || { sudo apt-get -qq update && sudo apt-get -qq install -y mdadm >/dev/null; }
  if ! mountpoint -q $ROOT && ! ls /dev/disk/by-id/google-local-nvme-ssd-* >/dev/null 2>&1; then
    # No local SSDs (a G4 without Titanium SSDs): use the boot disk.
    sudo mkdir -p $ROOT && sudo chown "$(id -u):$(id -g)" $ROOT
  elif ! mountpoint -q $ROOT; then
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
  # The bundle must match the pin (#12410): BUILD.json, the worker, every library.
  python3 "$(dirname "$0")/v41-build-check.py" build $ROOT/bin "$MANIFEST" --pin "$BUILD"
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
  local front=$BIN extra=()
  if [ -n "${FRONT:-}" ]; then
    mkdir -p $ROOT/front
    [ -x $ROOT/front/psionic-v41-stage ] || { gcloud storage cp -q "$FRONT" $ROOT/front/psionic-v41-stage && chmod 755 $ROOT/front/psionic-v41-stage; }
    front=$ROOT/front/psionic-v41-stage
    extra=(--worker $BIN)
  fi
  timeout 21600 $front qualify "${extra[@]}" --weights $ROOT/weights --manifest "sha256:$MANIFEST" --worker-build "$BUILD" \
    --oracle ${ORACLE_DIR:-$ROOT/oracle} --out "$out" --max-context ${MAX_CONTEXT:-2560} "$@" > "$out.log" 2>&1 \
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
  # p4-20k is unsupported: the unchunked profile's checked budget is 2,872
  # positions on a four-stage split, so it stays missing coverage.
  for case in p1-chat-thinking p2-code e-window-127 e-window-128 e-window-129 e-engram p3-2k; do
    name=calibration-$case
    gcloud storage ls "$RESULTS/$name/report.json" >/dev/null 2>&1 && continue
    qualify $ROOT/out/$name --observe --gpus 0,1,2,3 --cases $case
  done
  for control in wrong-weights wrong-routing wrong-rope stale-cache; do
    name=control-$control
    gcloud storage ls "$RESULTS/$name/report.json" >/dev/null 2>&1 && continue
    qualify $ROOT/out/$name --observe --gpus 0,1,2,3 --cases p1-chat-thinking --control $control
  done
  log "GPU runs done"
}

# The CPU reference control (the CPU port, bf16-tp8 profile) on the six
# shorter cases; the 2k and 20k prompts take more than a day on CPU.
run-cpu() {
  : "${MANIFEST:?}" "${RESULTS:?}"
  qualify $ROOT/out/reference-cpu --backend cpu --arithmetic bf16-tp8 --observe --step-secs 36000 \
    --cases p1-chat-thinking,p2-code,e-window-127,e-window-128,e-window-129,e-engram
  log "CPU reference done"
}

# The frozen policy on the holdout (G1 v2's verdict), then the G2 v2 suite's
# complete answers, both on the launch profile.
run-holdout() {
  : "${MANIFEST:?}" "${RESULTS:?}" "${POLICY:?}"
  local oracle=gs://openagents-psionic-oracle/deepseek-v41/1bae0e1 cases
  mkdir -p $ROOT/holdout $ROOT/g2-oracle
  gcloud storage rsync -q -r $oracle/g1-v2/holdout $ROOT/holdout
  gcloud storage rsync -q -r $oracle/g2-v2/oracle $ROOT/g2-oracle
  gcloud storage cp -q "$POLICY" $ROOT/policy.json
  cases=$(python3 -c 'import json,sys; print(",".join(c["id"] for c in json.load(open(sys.argv[1]))["cases"]))' $ROOT/holdout/holdout-corpus.json)
  ORACLE_DIR=$ROOT/holdout qualify $ROOT/out/holdout --observe --gpus 0,1,2,3 --policy $ROOT/policy.json --cases "$cases"
  # The calibration cases under the same frozen policy, for the per-check record
  qualify $ROOT/out/qualified-calibration --observe --gpus 0,1,2,3 --policy $ROOT/policy.json \
    --cases p1-chat-thinking,p2-code,e-window-127,e-window-128,e-window-129,e-engram,p3-2k
  cases=$(python3 -c 'import json,sys; print(",".join(c["id"] for c in json.load(open(sys.argv[1]))["cases"]))' $ROOT/g2-oracle/task-suite.json)
  ORACLE_DIR=$ROOT/g2-oracle MAX_CONTEXT=8192 qualify $ROOT/out/g2-answers --gpus 0,1,2,3 --answers 4096 --cases "$cases"
  log "holdout runs done"
}

# The rest of a qualification a reclaimed machine cut off: CASES on the
# holdout under the frozen policy, then the G2 suite's answers, at the launch
# context (8,192 positions, 2,872-token steps).
run-final() {
  : "${MANIFEST:?}" "${RESULTS:?}" "${POLICY:?}" "${CASES:?}"
  local oracle=gs://openagents-psionic-oracle/deepseek-v41/1bae0e1
  mkdir -p $ROOT/holdout $ROOT/g2-oracle
  gcloud storage rsync -q -r $oracle/g1-v2/holdout $ROOT/holdout
  gcloud storage rsync -q -r $oracle/g2-v2/oracle $ROOT/g2-oracle
  gcloud storage cp -q "$POLICY" $ROOT/policy.json
  ORACLE_DIR=$ROOT/holdout MAX_CONTEXT=8192 qualify $ROOT/out/holdout-rest --observe --gpus 0,1,2,3 \
    --max-step-tokens 2872 --policy $ROOT/policy.json --cases "$CASES"
  ORACLE_DIR=$ROOT/g2-oracle MAX_CONTEXT=8192 qualify $ROOT/out/g2-answers-final --gpus 0,1,2,3 \
    --max-step-tokens 2872 --answers 4096 \
    --cases g2-p1-sky,g2-p2-fib,g2-gcd,g2-speed,g2-json-moon,g2-palindrome-py
  log "final runs done"
}

"$@"
