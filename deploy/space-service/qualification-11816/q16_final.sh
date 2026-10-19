#!/bin/sh
# #11816 final rerun on the final private identity: every control-node case and trial,
# in one pass, into v4/. A failed step is kept and the run continues. Creates retry while
# an earlier computer is still deleting; the capacity check itself never retries.
T=/space/config/deployment-tools; Q="python3 $T/q16.py"; R=/space/qualification-11816; V=$R/v4
mkdir -p $V; cd $R
mv -f trials-*.jsonl case-*.json fault-*.json isolation.json egress*.json $R/v1-archive/ 2>/dev/null
mk1() { $Q call POST /v1/computers "{\"size\":\"small\",\"idempotency_key\":\"q16-v4-$1-$(date +%s)\"}" --who ${2:-a} | python3 -c 'import json,sys;print(json.load(sys.stdin)["value"].get("id",""))' 2>/dev/null; }
mk() { for i in $(seq 1 20); do c=$(mk1 "$@"); [ -n "$c" ] && { echo $c; return; }; sleep 15; done; }
ready() { $Q wait $1 awake --seconds 600 >/dev/null && $Q seed $1 --who ${2:-a} >/dev/null; }
gone() { [ -n "$1" ] || return 0; $Q call DELETE /v1/computers/$1 --who ${2:-a} >/dev/null; $Q wait $1 deleted --seconds 900 --who ${2:-a} >/dev/null; }
fresh() { c=$(mk $1); [ -n "$c" ] && ready $c; echo $c; }
step() { echo "== $(date -u +%T) $*"; }
stranded() { $Q call GET /v1/computers/$1 | grep -q "couldn't confirm"; }

step retention; RC=$(mk retention); $Q wait $RC awake --seconds 600 >/dev/null
$Q call POST /v1/computers/$RC/stop '{}' >/dev/null; $Q wait $RC stopped --seconds 600 >/dev/null
echo "$RC stopped $(date -u +%FT%TZ)" > $V/retention-computer

step cases-1; A=$(fresh cases1)
python3 $T/q16.py isolation $A trn_none > $V/isolation.log 2>&1; mv -f isolation.json $V/isolation.json 2>/dev/null
for c in attach lost_turn_reply cancellation; do
  stranded $A && { gone $A; A=$(fresh cases-$c); }
  python3 $T/q16_cases.py $c $A a > $V/run-$c.log 2>&1; mv -f case-$c.json $V/ 2>/dev/null
done
gone $A
step duplicates; python3 $T/q16_cases.py duplicates a > $V/run-duplicates.log 2>&1; mv -f case-duplicates.json $V/ 2>/dev/null
for c in $($Q call GET /v1/computers | python3 -c 'import json,sys;print(" ".join(x["id"] for x in json.load(sys.stdin)["value"]["computers"]))'); do [ "$c" = "$RC" ] || gone $c; done

step capacity; X=$(fresh capx); Y=$(fresh capy)
$Q call POST /v1/computers '{"size":"small","idempotency_key":"q16-v4-cap3"}' > $V/capacity-third.json 2>&1
$Q call POST /v1/computers '{"size":"medium","idempotency_key":"q16-v4-cap3m"}' >> $V/capacity-third.json 2>&1
step quotas; $Q call POST /v1/computers/$X/turns '{"command":"sleep 75; echo over-limit","timeout_seconds":120}' > $V/quota-over.json 2>&1
python3 - "$X" > $V/quota-at-limit.json 2>&1 <<'PY'
import json,sys,importlib.util
spec=importlib.util.spec_from_file_location('q16','/space/config/deployment-tools/q16.py'); q=importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
tid,r=q.turn(sys.argv[1],'sleep 75; echo over-limit',seconds=180,timeout=60); print(json.dumps({'turn':tid,'result':q.brief(r),'stdout':r.get('stdout')}))
PY
$Q call GET /v1/usage > $V/usage.json 2>&1
step egress; python3 $T/q16_egress.py $Y a > $V/egress.log 2>&1; cp -f egress.json $V/egress.json 2>/dev/null
step files-and-controls; python3 $T/q16_files.py $Y > $V/files-commands-controls.json 2>&1
gone $X; gone $Y

step faults; F=$(fresh faults)
for f in api_restart controller_restart delayed provider_outage; do
  stranded $F && { gone $F; F=$(fresh faults-$f); }
  python3 $T/q16_faults.py $f $F a > $V/run-fault-$f.log 2>&1; mv -f fault-$f.json $V/ 2>/dev/null
done
gone $F

step trials-cold
$Q trials cold v4-cold-warm-c1 --count 20 --owners a > $V/run-cold-c1.log 2>&1
$Q trials cold v4-cold-warm-c2 --count 3 --concurrency 2 --owners a,a > $V/run-cold-c2.log 2>&1
step trials-wake; X=$(fresh wakex); Y=$(fresh wakey)
if [ -n "$X" ] && [ -n "$Y" ]; then
  sh $T/q16_fix_wake.sh $X $Y 0 > $V/run-wake.log 2>&1
  for c in $(grep -o 'final .*' $V/run-wake.log | tail -1 | cut -d' ' -f2-); do gone $c; done
  mv -f trials-fix-wake-warm-c1.jsonl $V/trials-v4-wake-warm-c1.jsonl; mv -f trials-fix-wake-warm-c2.jsonl $V/trials-v4-wake-warm-c2.jsonl
else echo "no wake fixtures"; gone $X; gone $Y; fi
mv -f trials-v4-* $V/ 2>/dev/null

step upgrade; U=$(fresh upgrade); $Q call POST /v1/computers/$U/stop '{}' >/dev/null; $Q wait $U stopped --seconds 600 >/dev/null
python3 $T/q16_faults.py upgrade $U a > $V/run-fault-upgrade.log 2>&1; mv -f fault-upgrade.json $V/ 2>/dev/null
gone $U
step done; echo done-final
