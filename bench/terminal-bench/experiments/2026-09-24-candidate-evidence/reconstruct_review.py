from pathlib import Path
import hashlib,json,re,tempfile
from tbench import replay
root=Path.home()/'.openagents/terminal-bench'
job=root/'jobs/tb4--coder-one-microluna-v12--embedding-drift-monitor--candidate-evidence-9607-r1'
trial=next(p.parent for p in job.glob('*/result.json'))
stream=trial/'agent/episode/artifacts/microluna-1-2.atif.jsonl'
calls=[json.loads(line).get('step',{}).get('call') for line in stream.read_text().splitlines()]
calls=[c for c in calls if c]
read=next(c for c in calls if c['name']=='run_command' and c['arguments']['command'].startswith('for f in drift_monitor/'))
text=read['output'];assert '[truncated]' not in text
parts=re.split(r'^==== (drift_monitor/[^\n]+)\n',text,flags=re.M)
files=dict(zip(parts[1::2],parts[2::2]))
assert 'drift_monitor/windowing.py' in files
out=root/'experiments/candidate-evidence-9607/reconstructed-v12-embedding-r1-before-review'
out.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='candidate-before-review-') as temp:
 workspace=replay.workspace_of(trial,Path(temp))
 for name,body in files.items():
  path=workspace.root/'app'/name;assert path.is_file();path.write_text(body)
 source=out/'source-files'
 for name,body in files.items():
  p=source/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(body)
 record={'trial':trial.name,'source':str(stream),'stream_sha256':hashlib.sha256(stream.read_bytes()).hexdigest(),'read_call_id':read['id'],'method':'Final collected artifacts with every production module replaced by its complete pre-edit source dump in the review trace. This is a reconstruction, not an original snapshot.','source_files':{name:hashlib.sha256(body.encode()).hexdigest() for name,body in files.items()},'grade':replay.run_verifier(replay.task_dir(trial),workspace,out/'verification')}
 (out/'reconstruction.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record['grade'],indent=2))
