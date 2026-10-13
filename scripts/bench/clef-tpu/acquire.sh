#!/usr/bin/env bash
# Try to create one TPU VM and log the attempt (timing, result, EID) as JSON.
# usage: acquire.sh NAME ZONE TYPE spot|ondemand [LOGDIR]
set -u
NAME=$1 ZONE=$2 TYPE=$3 MODE=$4 LOGDIR=${5:-${CLEF_TPU_LOGDIR:-./ops}}
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
PROJECT=${PROJECT:-openagentsgemini}
SA=${SA:-oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com}
case $TYPE in
  v6e-*) RT=v2-alpha-tpuv6e;;
  v5litepod-*) RT=v2-alpha-tpuv5-lite;;
  v5p-*) RT=v2-alpha-tpuv5;;
  *) RT=${RUNTIME:?set RUNTIME};;
esac
FLAGS=()
[ "$MODE" = spot ] && FLAGS+=(--spot)
mkdir -p "$LOGDIR"
t0=$(date +%s)
out=$(gcloud compute tpus tpu-vm create "$NAME" --zone "$ZONE" --project "$PROJECT" \
  --accelerator-type "$TYPE" --version "$RT" --service-account "$SA" \
  --scopes https://www.googleapis.com/auth/cloud-platform "${FLAGS[@]}" --quiet 2>&1)
rc=$?
t1=$(date +%s)
msg=$(printf '%s' "$out" | grep -o '"message": "[^"]*"' | head -1 | sed 's/"message": //')
[ -z "$msg" ] && msg=$(printf "%s" "$out" | grep -v "^\.*$" | tail -3 | tr "\n\"" "  ")
eid=$(printf '%s' "$out" | grep -o 'EID: 0x[0-9a-f]*' | head -1 | sed 's/EID: //')
printf '{"ts":"%s","name":"%s","zone":"%s","type":"%s","mode":"%s","runtime":"%s","rc":%d,"seconds":%d,"eid":"%s","message":%s}\n' \
  "$(date -u +%FT%TZ)" "$NAME" "$ZONE" "$TYPE" "$MODE" "$RT" "$rc" "$((t1-t0))" "$eid" \
  "$(printf '%s' "${msg:-ok}" | python3 -c 'import json,sys;print(json.dumps(sys.stdin.read().strip().strip(chr(34))))')" \
  | tee -a "$LOGDIR/attempts.jsonl"
exit $rc
