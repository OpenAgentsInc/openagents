import argparse, hashlib, json, subprocess, time
from pathlib import Path
p=argparse.ArgumentParser(); p.add_argument('--source',required=True); p.add_argument('--profile',choices=['dev','release'],required=True); a=p.parse_args()
assert a.source == 'fb9a5fd280db673926cfd0d649f924da3091a24b', "Frozen final runtime source changed"
s=Path(__file__).parent; root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==a.source
assert not subprocess.check_output(['git','diff','--name-only','HEAD'],cwd=root,text=True).strip()
binding=s/'10936-final-copy-build-binding.json'
b=json.loads(binding.read_text() if binding.exists() else (s/'10936-final-copy-build-binding.template.json').read_text())
assert b['source'] in (None,a.source); b['source']=a.source
build=b['builds'][a.profile]; build['source']=a.source
command=build['command']; build['start_unix']=time.time()
with Path(build['build_log']).open('w') as log:
 r=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT)
build['end_unix']=time.time(); build['exit']=r.returncode
assert r.returncode==0, f'Build failed; see {build["build_log"]}'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==a.source
binary=Path('/Users/christopherdavid/work/openagents-target-agent0')/('debug' if a.profile=='dev' else 'release')/'examples/meteor_showcase_capture'
build['binary']={'path':str(binary),'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'bytes':binary.stat().st_size}
binding.write_text(json.dumps(b,indent=2)+'\n')
print(json.dumps({'source':a.source,'profile':a.profile,'binary':build['binary'],'exit':r.returncode}),flush=True)
