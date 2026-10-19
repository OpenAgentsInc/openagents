#!/bin/sh
# #11816 rerun, part 3: native upgrade and rollback with an existing suspended computer.
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
step upgrade; U=$(fresh upgrade); $Q call POST /v1/computers/$U/stop '{}' >/dev/null; $Q wait $U stopped --seconds 600 >/dev/null
python3 $T/q16_faults.py upgrade $U a > $V/run-fault-upgrade.log 2>&1; mv -f fault-upgrade.json $V/ 2>/dev/null
gone $U; step done; echo done-part3
