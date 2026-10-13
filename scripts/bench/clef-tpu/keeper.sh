#!/usr/bin/env bash
# Hold capacity: every few minutes, if we hold fewer than TARGET VMs of a type (READY or
# CREATING, ours = name starts with cd-/cq-), try a spot create in the next zone of its list.
# Skips when the fleet's spot $/h is already above MAX_RATE.  Logs via acquire.sh.
HERE=$(cd "$(dirname "$0")" && pwd)
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
MAX_RATE=${MAX_RATE:-18}
declare -A TARGET=( [v6e-1]=${T_V6E1:-2} [v6e-4]=${T_V6E4:-1} [v6e-8]=${T_V6E8:-1} )
declare -A ZONES=(
  [v6e-1]="us-central1-b asia-southeast1-b us-central1-a us-east1-d us-central1-c us-south1-a us-south1-c us-west1-c"
  [v6e-4]="asia-southeast1-b us-central1-b us-east1-d us-central1-a us-central1-c us-south1-a us-south1-c us-west1-c"
  [v6e-8]="us-central1-b us-central1-c us-central1-a us-south1-c us-south1-a us-west1-c us-east1-d"
)
declare -A IDX
price() { case $1 in v6e-*) echo 0.584;; v5litepod-*) echo 0.359;; v5p-*) echo 0.93;; esac; }
chips() { local n=${1##*-}; case $1 in v5p-*) echo $((n/2));; *) echo $n;; esac; }
while true; do
  list=$(gcloud compute tpus tpu-vm list --zone=- --project openagentsgemini --format="value(name,acceleratorType.basename(),state)" 2>/dev/null | grep -E '^c[dq]-')
  rate=$(echo "$list" | awk '$3=="READY"||$3=="CREATING"{print $2}' | while read t; do [ -n "$t" ] && echo "$(chips $t) * $(price $t)"; done | paste -sd+ - | bc 2>/dev/null)
  rate=${rate:-0}
  for t in "${!TARGET[@]}"; do
    have=$(echo "$list" | awk -v t="$t" '$2==t && ($3=="READY"||$3=="CREATING")' | wc -l | tr -d ' ')
    if [ "$have" -lt "${TARGET[$t]}" ] && [ "$(echo "$rate < $MAX_RATE" | bc)" = 1 ]; then
      zs=(${ZONES[$t]}); i=${IDX[$t]:-0}; z=${zs[$((i % ${#zs[@]}))]}; IDX[$t]=$((i+1))
      n="cd-$(echo $t | tr -d '-')-k$(date +%H%M)-$(echo $z | sed 's/-//g' | cut -c1-12)"
      echo "$(date -u +%T) have $have/${TARGET[$t]} $t rate \$$rate/h -> try $z"
      "$HERE/acquire.sh" "$n" "$z" "$t" spot >/dev/null 2>&1 &
    fi
  done
  sleep ${INTERVAL:-240}
done
