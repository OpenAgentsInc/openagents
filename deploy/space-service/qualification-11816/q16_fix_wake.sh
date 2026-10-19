#!/bin/sh
# #11816: continue post-fix wake trials; replace a fixture stranded by #12141 and keep the failure.
set -u
Q="python3 /space/config/deployment-tools/q16.py"
cd /space/qualification-11816
fixture() { id=$($Q call POST /v1/computers "{\"size\":\"small\",\"idempotency_key\":\"q16-fix-wake-$1\"}" | python3 -c 'import json,sys;print(json.load(sys.stdin)["value"]["id"])'); $Q wait $id awake >/dev/null && $Q seed $id >/dev/null && echo $id; }
stranded() { $Q call GET /v1/computers/$1 | grep -q "couldn't confirm"; }
replace() { $Q call DELETE /v1/computers/$1 >/dev/null; $Q wait $1 deleted --seconds 600 >/dev/null; fixture "$2"; }
X=$1; Y=$2; n=0
for i in $(seq $3 19); do
  $Q trials wake fix-wake-warm-c1 --count 1 --start $i --computers $X --owners a
  if stranded $X; then n=$((n+1)); X=$(replace $X x$n); echo "replaced x -> $X"; fi
done
for i in 0 1 2 3; do
  $Q trials wake fix-wake-warm-c2 --count 1 --start $i --concurrency 2 --computers $X,$Y --owners a,a
  if stranded $X; then n=$((n+1)); X=$(replace $X x$n); echo "replaced x -> $X"; fi
  if stranded $Y; then n=$((n+1)); Y=$(replace $Y y$n); echo "replaced y -> $Y"; fi
done
echo "final $X $Y"; echo done-fix-wake
