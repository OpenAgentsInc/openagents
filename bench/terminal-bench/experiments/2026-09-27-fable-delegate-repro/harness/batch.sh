#!/usr/bin/env bash
# Usage: batch.sh <attempt-suffix> <task>...   (issue #9776)
# One attempt per task, in order, one at a time. Before each: wait while
# another tbench/harbor run or a print-mode claude session is active; stop
# under 25 GB free. After each: stop on a usage or rate-limit signal, or when
# the known plus estimated spend of this issue passes $60.
set -u
cd ~/fable-delegate-9776
A=$1; shift
for t in "$@"; do
  deadline=$(python3 -c "import json;print(next(x['delegate_deadline_sec'] for x in json.load(open('tasks.json'))['tasks'] if x['task']=='$t'))")
  w=0
  while pgrep -f 'tbench run|harbor run' >/dev/null || pgrep -f 'claude .*( -p |--print|stream-json)' >/dev/null; do
    [ $w -eq 0 ] && echo "WAIT other Claude-login run active $(date -u +%T)"; sleep 30; w=$((w+30))
  done
  t0=$(date +%s)
  ./drive.sh "$t" "$A" "$deadline" > "logs/$t-$A.drive.txt" 2>&1
  rc=$?
  echo "DONE $t $A rc=$rc $(date -u +%T) $(grep -m1 ' reward ' logs/$t-$A.drive.txt)"
  grep -m1 'BEAT' "logs/$t-$A.drive.txt"
  [ $rc -eq 3 ] && { echo "STOP disk"; exit 3; }
  # Remove only the warm images this attempt built (created after it started).
  for img in $(docker images --format '{{.Repository}}:{{.Tag}}' "tbench-warm/$t"); do
    c=$(date -d "$(docker image inspect -f '{{.Created}}' "$img")" +%s)
    [ "$c" -ge "$t0" ] && docker rmi "$img" >/dev/null && echo "RMI $img"
  done
  J=~/.openagents/terminal-bench/jobs/tb4--coder-one-delegate-fable-low-kb-jev2--$t--9776-$A
  S=$(ls $J/*/agent/episode/artifacts/delegate-1.stream.jsonl 2>/dev/null | head -1)
  if [ -n "$S" ] && grep -Eqi '"status":"(rejected|allowed_warning)"|usage limit|rate.limit.*exceeded|"error":"rate_limit' "$S"; then
    echo "LIMIT signal in $t $A stream"; grep -Eoi '"status":"[a-z_]*"|usage limit[^"]{0,80}' "$S" | sort | uniq -c
    grep -Eqi '"status":"rejected"|usage limit' "$S" && { echo "STOP limit"; exit 4; }
  fi
  spent=$(python3 - <<'PY'
import json,glob
tot=0
for f in glob.glob('rows/*.json'):
    try: r=json.load(open(f))[0]
    except Exception: continue
    c=r['cost']['total_with_search_usd']
    tot+= c if c is not None else r['delegate']['estimate_usd']['total_lower_bound']
print(round(tot,2))
PY
)
  echo "SPENT_AT_LEAST $spent"
  python3 -c "import sys; sys.exit(0 if $spent < 57 else 1)" || { echo "STOP spend"; exit 5; }
done
echo "BATCH COMPLETE"
