#!/usr/bin/env bash
# File one queued resource (spot or on-demand) and log the response.
# usage: queue.sh TYPE spot|ondemand ZONE...
TYPE=$1 MODE=$2; shift 2
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
LOGDIR=${CLEF_TPU_LOGDIR:-./ops}; mkdir -p "$LOGDIR"
case $TYPE in v6e-*) RT=v2-alpha-tpuv6e;; v5litepod-*) RT=v2-alpha-tpuv5-lite;; v5p-*) RT=v2-alpha-tpuv5;; esac
short() { echo "$1" | sed -e 's/asia-northeast/ane/;s/asia-southeast/ase/;s/asia-south/as/;s/asia-east/ae/;s/europe-west/ew/;s/us-central/uc/;s/us-east/ue/;s/us-south/us/;s/us-west/uw/;s/southamerica-east/sae/;s/southamerica-west/saw/' -e 's/-//g'; }
t=$(echo "$TYPE" | sed 's/litepod//;s/-//'); m=$([ "$MODE" = spot ] && echo s || echo d)
for z in "$@"; do
  n="cq-$t-$(short "$z")-$m"
  F=(); [ "$MODE" = spot ] && F+=(--spot) || F+=(--valid-until-duration 24h)
  out=$(gcloud compute tpus queued-resources create "$n" --node-id "$n" --zone "$z" --project openagentsgemini \
    --accelerator-type "$TYPE" --runtime-version "$RT" "${F[@]}" \
    --service-account oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com --async --quiet 2>&1)
  printf '{"ts":"%s","qr":"%s","zone":"%s","type":"%s","mode":"%s","rc":%d,"out":%s}\n' "$(date -u +%FT%TZ)" "$n" "$z" "$TYPE" "$MODE" $? \
    "$(printf '%s' "$out" | tail -2 | python3 -c 'import json,sys;print(json.dumps(sys.stdin.read()))')" | tee -a "$LOGDIR/queued.jsonl"
done
