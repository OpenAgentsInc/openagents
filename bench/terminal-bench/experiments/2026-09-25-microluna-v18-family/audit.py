"""Verify retained evidence and extract counts without transcript interpretation."""
from pathlib import Path
import json,hashlib,shutil
h=Path.home();base=h/'.cache/openagents/v18-family';out=Path('/tmp/v18-publication-9640');state=json.loads((base/'state.json').read_text())
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def put(n,d):
 p=out/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,indent=2)+'\n')
verification=[];counts=[]
for row in state['trials']:
 job=h/'.openagents/terminal-bench/jobs'/row['job'];trial=job/row['trial'];m=json.loads((job/'tbench/manifests'/f"{row['trial']}.json").read_text())
 def walk(v):
  if isinstance(v,dict):
   if v.get('resolved') and v.get('sha256') and v.get('path'):
    p=Path(v['path']);verification.append({'path':str(p),'expected':v['sha256'],'actual':sha(p) if p.is_file() else None})
   else:
    for x in v.values():walk(x)
  elif isinstance(v,list):
   for x in v:walk(x)
 walk(m['evidence'])
 card=json.loads(next((base/'cards').glob('*'+row['trial']+'.card.json')).read_text())
 first=min((s['start_ms']+s['moments']['first_edit_ms'] for s in card['sessions'] if s['start_ms'] is not None and s['moments']['first_edit_ms'] is not None),default=None)
 notfound={t for n in card['waste']['not_found'] for t in n['turns']};times={};sessions=[]
 for s in card['sessions']:
  p=trial/'agent/episode/artifacts'/f"{s['id']}.atif.jsonl";records=[json.loads(x) for x in p.read_text().splitlines()]
  start=next(x['at'] for x in records if x['record']=='session')
  ts=[x['step']['at'] for x in records if x.get('step',{}).get('source')=='Agent' and not x['step'].get('call') and isinstance(x['step'].get('milliseconds'),int)]
  assert len(ts)==s['turns']
  for i,t in enumerate(ts,1):times[f"{s['number']}.{i}"]=s['start_ms']+t-start
  sessions.append({'session':s['number'],'turns':len(ts),'model_ms':s['model_ms'],'command_ms':s['command_ms'],'cost_usd':s['cost_usd']})
 counts.append({'task':row['task'],'label':row['label'],'first_edit_ms':first,'missing_program_turns':len(notfound),'missing_program_turns_before_first_edit':sum(times[t]<first for t in notfound) if first is not None else None,'missing_program_turns_without_edit':len(notfound) if first is None else None,'sessions':sessions})
 # Only numeric verifier summaries. No confirmation failure text is emitted.
 summaries=[]
 for p in (trial/'verifier').rglob('ctrf.json'):
  d=json.loads(p.read_text());summaries.append({'path':str(p.relative_to(trial)),**d.get('results',{}).get('summary',{})})
 put('attempts/'+row['task']+'/'+row['label']+'/test-counts.json',summaries)
put('evidence-verification.json',{'checked':len(verification),'mismatches':[x for x in verification if x['expected']!=x['actual']],'files':verification})
put('timing-counts.json',counts)
voids=[]
for job in sorted((h/'.openagents/terminal-bench/failed').glob('tb4--coder-one-microluna-v18--*--family-*-contamination-void')):
 files=[]
 for p in job.rglob('*'):
  if p.is_file():files.append({'path':str(p.relative_to(job)),'sha256':sha(p),'bytes':p.stat().st_size})
 dest=out/'setup-refusals'/job.name;dest.mkdir(parents=True,exist_ok=True)
 for p in job.glob('tbench/attempts/*.json'):shutil.copyfile(p,dest/'attempt.json')
 outcomes=[]
 for p in job.glob('*/result.json'):
  d=json.loads(p.read_text());outcomes.append({k:d.get(k) for k in ['trial_name','exception_info','agent_execution','agent_result','started_at','finished_at','verifier_result']})
 put(str((dest/'outcomes.json').relative_to(out)),outcomes)
 voids.append({'remote_path':str(job),'files':files})
put('setup-refusals/inventory.json',voids)
print('verified',len(verification),'mismatches',sum(x['expected']!=x['actual'] for x in verification),'void jobs',len(voids))
