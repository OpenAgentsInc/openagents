#!/usr/bin/env bash
# Keep spot queued resources alive: a SUSPENDED (preempted) or FAILED queued resource is
# deleted and filed again under the same name. Zones outside the US and Singapore (spot $0.27) are dropped (price).
HERE=$(cd "$(dirname "$0")" && pwd)
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
LOGDIR=${CLEF_TPU_LOGDIR:-./ops}
while true; do
  gcloud compute tpus queued-resources list --zone=- --project openagentsgemini \
    --format="value(name.basename(),name.segment(3),state.state,tpu.nodeSpec[0].node.acceleratorType)" 2>/dev/null |
  while read n z st t; do
    case $st in SUSPENDED|FAILED) ;; *) continue;; esac
    # us-south1-c fails every filing within a minute with "an internal error has occurred"; stop after 20
    if [ "$st" = FAILED ] && [ "$(grep -c "\"qr\":\"$n\".*FAILED" "$LOGDIR/requeue.jsonl" 2>/dev/null)" -ge 20 ]; then
      gcloud compute tpus queued-resources delete "$n" --zone "$z" --project openagentsgemini --force --quiet >/dev/null 2>&1
      echo "{\"ts\":\"$(date -u +%FT%TZ)\",\"qr\":\"$n\",\"zone\":\"$z\",\"was\":\"$st\",\"action\":\"give up\"}" >> "$LOGDIR/requeue.jsonl"; continue
    fi
    gcloud compute tpus queued-resources delete "$n" --zone "$z" --project openagentsgemini --force --quiet >/dev/null 2>&1
    echo "{\"ts\":\"$(date -u +%FT%TZ)\",\"qr\":\"$n\",\"zone\":\"$z\",\"was\":\"$st\",\"action\":\"$( [[ $z == us-* || $z == asia-southeast1-* ]] && echo refile || echo drop)\"}" >> "$LOGDIR/requeue.jsonl"
    [[ $z == us-* || $z == asia-southeast1-* ]] && CLEF_TPU_LOGDIR=$LOGDIR "$HERE/queue.sh" "$t" spot "$z" >/dev/null 2>&1
  done
  sleep 180
done
