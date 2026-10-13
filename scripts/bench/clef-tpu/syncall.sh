#!/usr/bin/env bash
# Push every campaign VM's ~/out to GCS, then mirror GCS locally. usage: syncall.sh LOCAL_DIR [loop-seconds]
HERE=$(cd "$(dirname "$0")" && pwd); LOCAL=$1; LOOP=${2:-0}
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
DATE=${DATE:-2026-10-13}
while true; do
  gcloud compute tpus tpu-vm list --zone=- --project openagentsgemini --format="value(name,name.segment(3),state)" 2>/dev/null \
   | awk '$1 ~ /^c[dq]-/ && $3=="READY"{print $1, $2}' | while read n z; do
      DATE=$DATE timeout 120 "$HERE/vm.sh" sync "$n" "$z" >/dev/null 2>&1 &
   done; wait
  mkdir -p "$LOCAL"; gcloud storage rsync -r gs://openagentsgemini-decision-throughput/tpu/$DATE "$LOCAL" -q >/dev/null 2>&1
  [ "$LOOP" = 0 ] && break; sleep "$LOOP"
done
