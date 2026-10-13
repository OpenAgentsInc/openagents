#!/usr/bin/env bash
# Every minute: any READY campaign VM (cd-/cq-) we have not started yet gets setup + its type's
# standard job (jobs/<type>.sh). Records cold-start timing (READY seen -> job started) in ops/autorun.jsonl.
HERE=$(cd "$(dirname "$0")" && pwd)
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
LOGDIR=${CLEF_TPU_LOGDIR:-./ops}; STARTED=$LOGDIR/autorun.started; touch "$STARTED"
while true; do
  gcloud compute tpus tpu-vm list --zone=- --project openagentsgemini --format="value(name,name.segment(3),acceleratorType.basename(),state,createTime)" 2>/dev/null |
  awk '$1 ~ /^c[dq]-/ && $4=="READY"' | while read n z t s ct; do
    key="$n/$ct"; grep -qxF "$key" "$STARTED" && continue
    job="$HERE/jobs/$t.sh"; [ -f "$job" ] || continue
    echo "$key" >> "$STARTED"
    ( t0=$(date +%s)
      "$HERE/vm.sh" push "$n" "$z" >/dev/null 2>&1; "$HERE/vm.sh" setup "$n" "$z" > "$LOGDIR/setup-$n.log" 2>&1
      t1=$(date +%s); "$HERE/startfile.sh" "$n" "$z" "$job" >> "$LOGDIR/setup-$n.log" 2>&1
      printf '{"ts":"%s","name":"%s","zone":"%s","type":"%s","created":"%s","setup_s":%d,"job":"%s"}\n' "$(date -u +%FT%TZ)" "$n" "$z" "$t" "$ct" $((t1-t0)) "$(basename $job)" >> "$LOGDIR/autorun.jsonl" ) &
  done
  sleep 60
done
