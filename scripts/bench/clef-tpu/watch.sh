#!/usr/bin/env bash
# Poll every TPU VM and queued resource; log state transitions to states.jsonl.
LOGDIR=${CLEF_TPU_LOGDIR:-./ops}; mkdir -p "$LOGDIR"
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
declare -A last
while true; do
  while IFS='|' read -r name zone type state spot health; do
    [ -z "$name" ] && continue
    k="vm/$name"; v="$state/$health"
    if [ "${last[$k]:-}" != "$v" ]; then
      printf '{"ts":"%s","kind":"vm","name":"%s","zone":"%s","type":"%s","spot":"%s","state":"%s","health":"%s"}\n' \
        "$(date -u +%FT%TZ)" "$name" "$zone" "$type" "$spot" "$state" "$health" >> "$LOGDIR/states.jsonl"
      last[$k]=$v
    fi
  done < <(gcloud compute tpus tpu-vm list --zone=- --project openagentsgemini \
     --format="value[separator='|'](name,name.segment(3),acceleratorType.basename(),state,schedulingConfig.spot,health)" 2>/dev/null)
  while IFS='|' read -r name zone state; do
    [ -z "$name" ] && continue
    k="qr/$name"
    if [ "${last[$k]:-}" != "$state" ]; then
      printf '{"ts":"%s","kind":"qr","name":"%s","zone":"%s","state":"%s"}\n' "$(date -u +%FT%TZ)" "$name" "$zone" "$state" >> "$LOGDIR/states.jsonl"
      last[$k]=$state
    fi
  done < <(gcloud compute tpus queued-resources list --zone=- --project openagentsgemini \
     --format="value[separator='|'](name.basename(),name.segment(3),state.state)" 2>/dev/null)
  sleep ${INTERVAL:-30}
done
