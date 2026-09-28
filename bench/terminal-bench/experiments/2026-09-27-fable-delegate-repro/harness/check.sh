#!/usr/bin/env bash
# Usage: check.sh <task> <attempt>   (runs on coderos; issue #9776)
set -u
t=$1; A=$2
J=tb4--coder-one-delegate-fable-low-kb-jev2--$t--9776-$A
cd ~/fable-delegate-9776
read barc bars < <(python3 -c "
import json;d=json.load(open('tasks.json'))
r=next(x for x in d['tasks'] if x['task']=='$t');print(r['bar_cost_usd'],r['bar_seconds'])")
S=$(python3 -c "
import json,re;d=json.load(open('candidates/$t.json'))
print(re.search(r'\\\$([0-9.]+) for embeddings',d['search_output']).group(1))")
mkdir -p rows
python3 summarize.py --bar $barc $bars --search-usd $S $J > rows/$t-$A.json || { echo SUMMARIZE FAILED; }
python3 - rows/$t-$A.json <<'PY'
import json,sys
r=json.load(open(sys.argv[1]))[0]
d=r["delegate"]
print(r["trial_id"], r["trial"], "reward", r["reward"], "exc", (r["exception"] or {}).get("exception_type") if r["exception"] else None)
print("trial_s", r["trial_seconds"], r["phases_sec"], r["agent_sec"])
print("delegate", d["status"], "deadline", d["deadline_sec"], "calls", d["api_calls"], "cost", d["total_cost_usd"], d["stream_tokens"], "think", d["thinking_estimate"], "est", d["estimate_usd"])
print("brief", d["briefing_chars"], d["briefing_sha256"])
print("cost", r["cost"], "BEAT", r["beat_the_bar"], "jev_req", r["explore"]["jev_requests"])
print("knowledge in", [i.split()[1] for i in r["briefing_knowledge"]["included"]], "omitted", r["briefing_knowledge"]["omitted"])
PY
T=$(ls -d ~/.openagents/terminal-bench/jobs/$J/${t}__*/ | head -1)
tail -1 $T/verifier/test-stdout.txt 2>/dev/null
grep -E "^(FAILED|ERROR)" $T/verifier/test-stdout.txt 2>/dev/null | head -8
python3 - $T <<'PY'
import json,sys
T=sys.argv[1]
d=json.load(open(T+"agent/episode/artifacts/briefing-jev.json"))
for c in d["candidates"]: print(" ", c.get("rank"), c["id"], c["p"], c.get("fate", c.get("kept")))
for q in d["requirements"]: print("  req", q["p"], q["flagged"], q["text"][:100])
print(" how", d.get("how"), d.get("milliseconds"), d.get("input_tokens"))
PY
grep -o '"rate_limit_status":"[a-z_]*"\|"status":"[a-z_]*"' $T/agent/episode/artifacts/delegate-1.stream.jsonl | sort | uniq -c
df -h / | tail -1
