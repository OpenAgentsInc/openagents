from pathlib import Path
import json,subprocess,time
s=Path(__file__).parent; root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents'); source='f193982ec03bc55cd00345e7146c5b566a4c62b2'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
jobs=[('cli',['cargo','test','-p','verse','--example','meteor_showcase_capture','--features','capture']),('gles',['cargo','test','-p','verse','--lib','gles_tests']),('render',['cargo','test','-p','verse','--lib','render::'])]
rows=[]
for label,cmd in jobs:
 row={'source':source,'label':label,'command':cmd,'start_unix':time.time()}
 with (s/f'10936-main-integration-{label}-tests.log').open('w') as log: r=subprocess.run(cmd,cwd=root,stdout=log,stderr=subprocess.STDOUT)
 row.update(exit=r.returncode,end_unix=time.time());rows.append(row)
 (s/'10936-main-integration-consumer-tests.json').write_text(json.dumps(rows,indent=2)+'\n')
 print(label+' exit '+str(r.returncode),flush=True)
 if r.returncode: raise SystemExit(r.returncode)
