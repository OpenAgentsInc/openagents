import os, subprocess, json, time, hashlib
from pathlib import Path
scratch=Path(__file__).parent
root=Path('/Users/christopherdavid/.codex/worktrees/10907-budgeted-relight/openagents')
binary=Path('/Users/christopherdavid/work/openagents-target-agent2/debug/examples/baked_light_capture')
env=os.environ.copy()
env.update(VERSE_QUALITY='high',VERSE_HOME=str(scratch/'10907-capture-cache'),VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp',VERSE_KIT_BAKE=str(scratch/'10907-matched-bake/090f106f9c459f62f9cd67902a18531b7e00f9d15e292e0b60a0dd7b231b0b6b.vlay'),VERSE_KIT_UNPINNED='1')
cmd=[str(binary),str(scratch/'10907-full-wall-capture'),'128','10441']
meta={'source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'profile':'dev with pinned workspace optimized dependencies','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'command':cmd,'environment':{k:env[k] for k in ('VERSE_QUALITY','VERSE_HOME','VERSE_KIT_PACK','VERSE_KIT_BAKE','VERSE_KIT_UNPINNED')},'start_unix':time.time()}
manifest=scratch/'10907-full-wall-capture-manifest.json'
manifest.write_text(json.dumps(meta,indent=2)+'\n')
with (scratch/'10907-full-wall-capture.log').open('w') as log:
 r=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
meta.update(exit=r.returncode,end_unix=time.time())
manifest.write_text(json.dumps(meta,indent=2)+'\n')
print('Full capture exit '+str(r.returncode),flush=True)
raise SystemExit(r.returncode)
