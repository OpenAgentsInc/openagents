"""Export only the completed v18 cohort and its outcome metadata."""
from pathlib import Path
import hashlib,json,shutil,subprocess
h=Path.home();base=h/'.cache/openagents/v18-family';out=Path('/tmp/v18-publication-9640');out.mkdir(exist_ok=False)
def put(name,obj):
 p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(obj,indent=2)+'\n')
def copy(src,name):
 p=out/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(src,p)
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
state=json.loads((base/'state.json').read_text());assert len(state['trials'])==18 and state['finished']
for n in ['state.json','state-void-contamination.json','driver.py','driver.log','driver-void-contamination.log','tally.py','tally.json','pieces.py','candidates.log']:
 copy(base/n,'original/'+n)
copy(base/'candidates/batch.json','candidates/batch.json')
for p in (base/'cards').glob('*.card.*'):copy(p,'cards/'+p.name)
pieces=json.loads((base/'pieces.json').read_text())
# Do not export confirmation instructions, commands, or transcript prose as analysis.
allowed=['task','label','trial','loop_record','stopped','stopped_by','scores','submitted_score','submitted_session','sessions','not_found_turns','self_score_agrees','frozen_score','untouched_score']
put('loop-counts.json',[{**{k:x.get(k) for k in allowed},'baseline_commands_count':len(x.get('baseline_commands') or []),'baseline_runs_count':len(x.get('baseline_runs') or []),'baseline_no_entry_point':bool(x.get('baseline_none'))} for x in pieces])
inventory=[];audit=[]
for row in state['trials']:
 job=h/'.openagents/terminal-bench/jobs'/row['job'];trial=job/row['trial'];dest='attempts/'+row['task']+'/'+row['label']
 for f in ['tbench/attempts/'+row['trial']+'.json','tbench/manifests/'+row['trial']+'.json']:
  copy(job/f,dest+'/'+('attempt.json' if '/attempts/' in f else 'manifest.json'))
 for f in ['agent/episode/evaluation/usage.json','agent/episode-doctor.txt']:
  copy(trial/f,dest+'/'+Path(f).name)
 result=json.loads((trial/'result.json').read_text());config=json.loads((job/'config.json').read_text())
 summary={k:result.get(k) for k in ['id','task_name','trial_name','task_checksum','verifier_result','exception_info','started_at','finished_at','environment_setup','agent_setup','agent_execution','verifier']}
 put(dest+'/outcome.json',summary)
 put(dest+'/launch.json',config)
 # Hash retained evidence without reading transcript content for lessons.
 files=[]
 for root in [trial/'agent',trial/'verifier',job/'tbench']:
  for p in sorted(root.rglob('*')):
   if p.is_file() and not p.is_symlink():files.append({'path':str(p.relative_to(job)),'bytes':p.stat().st_size,'sha256':sha(p)})
 inventory.append({'job':row['job'],'trial':row['trial'],'remote_path':str(job),'files':files})
 manifests=json.loads((job/'tbench/manifests'/f"{row['trial']}.json").read_text())
 audit.append({'job':row['job'],'evidence_shape':type(manifests['evidence']).__name__,'evidence_keys':list(manifests['evidence']) if isinstance(manifests['evidence'],dict) else None})
for p in (h/'.openagents/terminal-bench').glob('tb4--coder-one-microluna-v18--*--family-*.log'):copy(p,'launch-logs/'+p.name)
put('trace-inventory.json',inventory)
w=h/'.cache/openagents/worktrees/microluna-v18-family';artifact=h/'.cache/openagents/artifacts/coder-one-3a25a0ff1f'
put('audit.json',{'harness_commit':subprocess.check_output(['git','-C',str(w),'rev-parse','HEAD'],text=True).strip(),'harness_status':subprocess.check_output(['git','-C',str(w),'status','--porcelain'],text=True),'reflog':subprocess.check_output(['git','-C',str(w),'reflog','-5','--date=iso'],text=True),'artifact_sha256':sha(artifact),'manifest_sha256':sha(w/'crates/coder-one/policies/microluna-v18.json'),'evidence_shapes':audit})
print(out)
