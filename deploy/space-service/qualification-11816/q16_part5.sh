#!/bin/sh
# #11816 rerun, part 5: files and controls, wake trials, then upgrade, with capacity 4
# (one slot is held by the #12164 stranded row).
# Each step records its own file under v3/; a failed step is kept and the run continues.
# Steps continue after a failure; each records its own output.
T=/space/config/deployment-tools; Q="python3 $T/q16.py"; R=/space/qualification-11816; V=$R/v3
mkdir -p $V $R/v1-archive; cd $R
mk() { $Q call POST /v1/computers "{\"size\":\"small\",\"idempotency_key\":\"q16-v3-$1-$(date +%s)\"}" --who ${2:-a} | python3 -c 'import json,sys;print(json.load(sys.stdin)["value"]["id"])'; }
ready() { $Q wait $1 awake --seconds 600 >/dev/null && $Q seed $1 --who ${2:-a} >/dev/null; }
gone() { $Q call DELETE /v1/computers/$1 --who ${2:-a} >/dev/null; $Q wait $1 deleted --seconds 900 --who ${2:-a} >/dev/null; }
fresh() { c=$(mk $1); ready $c; echo $c; }
step() { echo "== $(date -u +%T) $*"; }
stranded() { $Q call GET /v1/computers/$1 | grep -q "couldn't confirm"; }
need() { [ -n "$1" ] || { echo "no computer for $2"; exit 3; }; }
step fixtures; X=$(fresh wakex); need "$X" x; Y=$(fresh wakey); need "$Y" y
step files-and-controls; python3 $T/q16_files.py $X > $V/files-commands-controls.json 2>&1
step trials-wake; sh $T/q16_fix_wake.sh $X $Y 0 > $V/run-wake.log 2>&1
for c in $(grep -o 'final .*' $V/run-wake.log | tail -1 | cut -d' ' -f2-); do gone $c; done
mv -f trials-fix-wake-warm-c1.jsonl $V/trials-v3-wake-warm-c1.jsonl; mv -f trials-fix-wake-warm-c2.jsonl $V/trials-v3-wake-warm-c2.jsonl
step upgrade; U=$(fresh upgrade); need "$U" upgrade; $Q call POST /v1/computers/$U/stop '{}' >/dev/null; $Q wait $U stopped --seconds 600 >/dev/null
mv -f $V/fault-upgrade.json $V/attempt1-fault-upgrade.json 2>/dev/null
python3 $T/q16_faults.py upgrade $U a > $V/run-fault-upgrade.log 2>&1; mv -f fault-upgrade.json $V/ 2>/dev/null
gone $U; step done; echo done-part5
