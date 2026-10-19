#!/bin/sh
# #11816 post-#12144 trial series on the private candidate (owner a only; owner b is closed).
set -eu
Q="python3 /space/config/deployment-tools/q16.py"
cd /space/qualification-11816
$Q trials cold fix-cold-warm-c2 --count 3 --concurrency 2 --owners a,a
X=$($Q call POST /v1/computers '{"size":"small","idempotency_key":"q16-fix-wake-x"}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["value"]["id"])')
Y=$($Q call POST /v1/computers '{"size":"small","idempotency_key":"q16-fix-wake-y"}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["value"]["id"])')
echo "fixtures $X $Y"
$Q wait $X awake >/dev/null; $Q seed $X >/dev/null
$Q wait $Y awake >/dev/null; $Q seed $Y >/dev/null
$Q trials wake fix-wake-warm-c1 --count 20 --computers $X --owners a
$Q trials wake fix-wake-warm-c2 --count 4 --concurrency 2 --computers $X,$Y --owners a,a
echo done-fix-run
