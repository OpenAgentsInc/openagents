#!/usr/bin/env bash
# Collect original-input stage measurements. The caller owns the VM through
# cloud-budget.py. This command does not freeze limits or qualify a release.
set -euo pipefail
: "${WORKER:?}" "${BUILD:?}" "${MANIFEST:?}" "${ORACLE:?}" "${CASES:?}" "${RESULTS:?}"
ROOT=${ROOT:-/mnt/isolation}
MODELS=${MODELS:-gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1}
BACKEND=${BACKEND:-cpu}
ARITHMETIC=${ARITHMETIC:-bf16}
STAGES=${STAGES:-0,1,2,3}
BIN=$ROOT/bin/psionic-v41-stage
sudo mkdir -p "$ROOT"
sudo chown "$(id -u):$(id -g)" "$ROOT"
mkdir -p "$ROOT/bin/lib" "$ROOT/weights/manifests" "$ROOT/oracle" "$ROOT/out"
export LD_LIBRARY_PATH=$ROOT/bin/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
exec 9>"$ROOT/collection.lock"
flock -n 9 || { printf 'A collection already uses %s\n' "$ROOT" >&2; exit 2; }
upload() { gcloud storage rsync -q -r "$ROOT/out" "$RESULTS" || true; }
trap upload EXIT
{
  date -u +%FT%TZ
  uname -a
  lscpu
  free -h
  sha256sum "$0" "$(dirname "$0")/v41-build-check.py"
  printf 'worker=%s\nbuild=%s\nmanifest=%s\noracle=%s\ncases=%s\nstages=%s\nbackend=%s\narithmetic=%s\n' \
    "$WORKER" "$BUILD" "$MANIFEST" "$ORACLE" "$CASES" "$STAGES" "$BACKEND" "$ARITHMETIC"
} > "$ROOT/out/collection-identity.txt"
gcloud storage cp -q "$WORKER/psionic-v41-stage" "$WORKER/BUILD.json" "$ROOT/bin/"
gcloud storage cp -q "$WORKER/lib/*" "$ROOT/bin/lib/"
gcloud storage cp -q "$WORKER/manifests/$MANIFEST.json" "$ROOT/weights/manifests/"
chmod 755 "$BIN"
python3 "$(dirname "$0")/v41-build-check.py" build "$ROOT/bin" "$MANIFEST" --pin "$BUILD"
gcloud storage cp -q "$ORACLE/manifest.json" "$ROOT/oracle/"
python3 - "$ROOT/oracle/manifest.json" "$CASES" "$STAGES" <<'PY'
import json, sys
names = [t['name'] for t in json.load(open(sys.argv[1]))['tensors']]
cases = sys.argv[2].split(',')
assert cases and all(c and any(n.startswith(c + '.') for n in names) for c in cases)
stages = sys.argv[3].split(',')
assert stages and len(set(stages)) == len(stages) and all(s in ('0','1','2','3') for s in stages)
PY
gcloud storage rsync -q -r "$MODELS" "$ROOT/weights" > "$ROOT/out/copy-weights.log" 2>&1 &
weights_pid=$!
IFS=, read -r -a cases <<< "$CASES"
for case in "${cases[@]}"; do
  mkdir -p "$ROOT/oracle/$case"
  gcloud storage rsync -q -r "$ORACLE/$case" "$ROOT/oracle/$case"
done
wait "$weights_pid"
IFS=, read -r -a stages <<< "$STAGES"
for stage in "${stages[@]}"; do
  out=$ROOT/out/stage$stage
  status=0
  timeout --kill-after=30s 36000 "$BIN" isolate --weights "$ROOT/weights" --manifest "sha256:$MANIFEST" \
    --worker-build "$BUILD" --oracle "$ROOT/oracle" --out "$out" --backend "$BACKEND" \
    --arithmetic "$ARITHMETIC" --stage-index "$stage" --gpus "${GPU:-0}" --observe \
    --max-context 8192 --max-step-tokens 2872 --step-secs 36000 --cases "$CASES" \
    > "$ROOT/out/stage$stage.log" 2>&1 || status=$?
  printf '%s\n' "$status" > "$ROOT/out/stage$stage.status"
  upload
  [ "$status" = 0 ] || exit "$status"
done
date -u +%FT%TZ > "$ROOT/out/COLLECTION_COMPLETE"
