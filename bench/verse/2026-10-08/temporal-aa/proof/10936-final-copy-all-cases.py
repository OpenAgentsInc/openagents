from pathlib import Path
import json, subprocess, argparse
p=argparse.ArgumentParser();p.add_argument("--start-index",type=int,default=0);a=p.parse_args()
s=Path(__file__).parent; root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents'); source='fb9a5fd280db673926cfd0d649f924da3091a24b'
labels=['high-dev','high-release','high-paired','medium-paired','pan-paired','orbit-paired']
for i,label in enumerate(labels):
 if i<a.start_index: continue
 stem='10936-final-copy-'+label
 base=['python3',str(s/'10936-final-copy-runner.py'),'--source',source,'--case-index',str(i),'--build-binding',str(s/'10936-final-copy-build-binding.json')]
 cmd=['openagents','lease','gpu','--receipt',str(s/(stem+'-gpu.json')),'--','openagents','lease','quiet','--receipt',str(s/(stem+'-quiet.json')),'--']+base+['--run']
 print('Starting '+label,flush=True)
 r=subprocess.run(cmd,cwd=root)
 if r.returncode: raise SystemExit(r.returncode)
 r=subprocess.run(base+['--finish'],cwd=root)
 if r.returncode: raise SystemExit(r.returncode)
 report=json.loads((s/stem/'capture.json').read_text())
 summary={'case':label,'phase_p99_ms':{k:v['frame_ms']['p99'] for k,v in report['phases'].items() if v['frame_ms'] is not None}}
 if report.get('temporal_comparison'):
  summary['taa_wall_95pct_upper_ms']={k:v['wall_render_completion_mean_95pct_ci_ms']['upper'] for k,v in report['temporal_comparison']['phases'].items() if v['wall_render_completion_mean_95pct_ci_ms'] is not None}
 print(json.dumps(summary),flush=True)
print('All six cases finished',flush=True)
