set -u
base='/Users/christopherdavid/.openagents/scratch/codex-01a11c18-367b-7793-b27f-8381cea14e2f'
agent='bc-83e18906-00e2-436c-978b-13a4932f58b0'
run='run-7b90ddb0-9d8b-43cb-870a-22fdd0e4fd31'
stream="$base/cursor-agent-run-stream.sse"
while :; do
  last_id=$(python3 -c 'p="'"$stream"'"; print([x[4:] for x in open(p).read().splitlines() if x.startswith("id: ")][-1])')
  curl --silent --show-error --max-time 25 --no-buffer --user "${CURSOR_API_KEY}:" -H "Last-Event-ID: $last_id" "https://api.cursor.com/v1/agents/$agent/runs/$run/stream" --output "$base/cursor-agent-stream-segment.sse" || true
  cat "$base/cursor-agent-stream-segment.sse" >> "$stream"
  curl --fail --silent --show-error --user "${CURSOR_API_KEY}:" "https://api.cursor.com/v1/agents/$agent/runs/$run" --output "$base/cursor-agent-run-status.json"
  status=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("status","UNKNOWN"))' "$base/cursor-agent-run-status.json")
  python3 - <<'PY'
import json,collections,os
base='/Users/christopherdavid/.openagents/scratch/codex-01a11c18-367b-7793-b27f-8381cea14e2f/'
p=base+'cursor-agent-run-stream.sse'; lines=open(p).read().splitlines(); event=None; data=[]; event_id=None; calls=[]
for line in lines+['']:
 if line.startswith('id: '): event_id=line[4:]
 elif line.startswith('event: '): event=line[7:]
 elif line.startswith('data: '): data.append(line[6:])
 elif not line:
  if event=='tool_call':
   try: obj=json.loads('\n'.join(data)); obj['_event_id']=event_id; calls.append(obj)
   except json.JSONDecodeError: pass
  event=None; data=[]
for name, selected in [('cursor-agent-tool-calls.jsonl',calls),('cursor-agent-tool-outputs.jsonl',[c for c in calls if c.get('status')=='completed' and 'result' in c])]:
 with open(base+name,'w') as f:
  for item in selected: f.write(json.dumps(item,ensure_ascii=False)+'\n')
print('bytes',os.path.getsize(p),'tool_call_events',len(calls),'completed_with_result',sum(c.get('status')=='completed' and 'result' in c for c in calls))
PY
  echo "run_status=$status last_event_id=$last_id"
  if [ "$status" != 'RUNNING' ]; then break; fi
done
