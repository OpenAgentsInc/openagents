#!/usr/bin/env bash
# Fire acquire.sh in parallel for one TYPE over many zones.
# usage: wave.sh TYPE spot|ondemand ZONE...
TYPE=$1 MODE=$2; shift 2
HERE=$(cd "$(dirname "$0")" && pwd)
LOGDIR=${CLEF_TPU_LOGDIR:-./ops}; mkdir -p "$LOGDIR"
short() { echo "$1" | sed -e 's/northamerica-northeast/nane/;s/southamerica-east/sae/;s/southamerica-west/saw/;s/asia-northeast/ane/;s/asia-southeast/ase/;s/asia-south/as/;s/asia-east/ae/;s/europe-west/ew/;s/europe-north/en/;s/us-central/uc/;s/us-east/ue/;s/us-south/us/;s/us-west/uw/' -e 's/-//g'; }
t=$(echo "$TYPE" | sed 's/litepod//;s/-//')
m=$([ "$MODE" = spot ] && echo s || echo d)
for z in "$@"; do
  n="cd-$t-$(short "$z")-$m"
  nohup "$HERE/acquire.sh" "$n" "$z" "$TYPE" "$MODE" "$LOGDIR" > "$LOGDIR/$n.log" 2>&1 &
done
wait
