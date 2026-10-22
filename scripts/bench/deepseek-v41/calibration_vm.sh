#!/usr/bin/env bash
# Collect independent reference or candidate regressions on a dedicated VM.
# The caller creates and deletes the VM through cloud-budget.py. This does
# not freeze a policy or turn a regression set into confirmation evidence.
set -euo pipefail
: "${WORKER:?}" "${BUILD:?}" "${MANIFEST:?}" "${ORACLE:?}" "${CASES:?}" "${RESULTS:?}"
ROOT=${ROOT:-/mnt/stage}
MODELS=${MODELS:-gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1}
BACKEND=${BACKEND:-cpu}
ARITHMETIC=${ARITHMETIC:-bf16}
BIN=$ROOT/bin/psionic-v41-stage
export LD_LIBRARY_PATH=$ROOT/bin/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
sudo mkdir -p "$ROOT"
sudo chown "$(id -u):$(id -g)" "$ROOT"
mkdir -p "$ROOT/bin/lib" "$ROOT/weights/manifests" "$ROOT/oracle" "$ROOT/out"
exec 9>"$ROOT/collection.lock"
flock -n 9 || { printf 'A collection already uses %s\n' "$ROOT" >&2; exit 2; }
upload() {
  gcloud storage rsync -q -r "$ROOT/out" "$RESULTS" || true
}
upload_case() {
  # Do not relist and compare every earlier token's records after each case.
  gcloud storage rsync -q -r "$ROOT/out/$1" "$RESULTS/$1" || return
  gcloud storage cp -q "$ROOT/out/$1.log" "$ROOT/out/collection-identity.txt" \
    "$RESULTS/" || true
}
trap upload EXIT
{
  date -u +%FT%TZ
  uname -a
  lscpu
  free -h
  sha256sum "$0" "$(dirname "$0")/v41-build-check.py"
  printf 'worker=%s\nbuild=%s\nmanifest=%s\noracle=%s\ncases=%s\nbackend=%s\narithmetic=%s\n' \
    "$WORKER" "$BUILD" "$MANIFEST" "$ORACLE" "$CASES" "$BACKEND" "$ARITHMETIC"
} > "$ROOT/out/collection-identity.txt"
gcloud storage cp -q "$WORKER/psionic-v41-stage" "$WORKER/BUILD.json" "$ROOT/bin/"
gcloud storage cp -q "$WORKER/lib/*" "$ROOT/bin/lib/"
gcloud storage cp -q "$WORKER/manifests/$MANIFEST.json" "$ROOT/weights/manifests/"
chmod 755 "$BIN"
python3 "$(dirname "$0")/v41-build-check.py" build "$ROOT/bin" "$MANIFEST" --pin "$BUILD"
gcloud storage cp -q "$ORACLE/manifest.json" "$ROOT/oracle/"
python3 - "$ROOT/oracle/manifest.json" "$CASES" <<'PY'
import json, sys
names = [t['name'] for t in json.load(open(sys.argv[1]))['tensors']]
missing = [c for c in sys.argv[2].split(',') if not c or not any(n.startswith(c + '.') for n in names)]
assert not missing, f'Cases absent from the pinned oracle: {missing}'
PY
gcloud storage rsync -q -r "$MODELS" "$ROOT/weights" > "$ROOT/out/copy-weights.log" 2>&1 &
weights_pid=$!
printf '%s\n' "$weights_pid" > "$ROOT/out/copy-weights.pid"
IFS=, read -r -a cases <<< "$CASES"
for case in "${cases[@]}"; do
  mkdir -p "$ROOT/oracle/$case"
  gcloud storage rsync -q -r "$ORACLE/$case" "$ROOT/oracle/$case"
done
wait "$weights_pid"
df -h "$ROOT" >> "$ROOT/out/collection-identity.txt"
for case in "${cases[@]}"; do
  out=$ROOT/out/$case
  if [ -f "$out/COLLECTED" ]; then continue; fi
  options=(--backend "$BACKEND" --arithmetic "$ARITHMETIC")
  if [ -n "${POLICY:-}" ]; then
    options+=(--policy "$POLICY")
  fi
  status=0
  timeout 36000 "$BIN" qualify --weights "$ROOT/weights" --manifest "sha256:$MANIFEST" \
    --worker-build "$BUILD" --oracle "$ROOT/oracle" --out "$out" \
    --max-context 8192 --max-step-tokens 2872 --step-secs 36000 \
    --observe --cases "$case" "${options[@]}" > "$out.log" 2>&1 || status=$?
  # A numerical failure is retained as a failure. Collection itself also
  # refuses a missing/truncated report instead of silently skipping a case.
  python3 - "$out" "$case" "$status" <<'PY'
import json, pathlib, sys
out, case, status = pathlib.Path(sys.argv[1]), sys.argv[2], int(sys.argv[3])
report = json.loads((out / 'report.json').read_text())
assert [c['case'] for c in report['cases']] == [case], 'wrong or missing case'
assert report['release'] is False, 'calibration is not release qualification'
summary = {'purpose': 'calibration-only', 'case': case, 'process_status': status,
           'reported_passed': report['passed'], 'profile': report['profile'],
           'stage_bundle_sha256': report['stage_bundle_sha256']}
(out / 'COLLECTED').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary))
PY
  upload_case "$case"
done
date -u +%FT%TZ > "$ROOT/out/COLLECTION_COMPLETE"
upload
