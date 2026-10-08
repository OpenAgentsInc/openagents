import os, subprocess, json, hashlib, time
from pathlib import Path
scratch=Path(__file__).parent
root=Path('/Users/christopherdavid/.codex/worktrees/10907-budgeted-relight/openagents')
target=Path('/Users/christopherdavid/work/openagents-target-agent2/debug')
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
env=os.environ.copy()
env.pop('VERSE_KIT_UNPINNED',None)
env.update(VERSE_QUALITY='high',VERSE_HOME=str(scratch/'10907-matched-bake-cache'),VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp',VERSE_KIT_BAKE='/Users/christopherdavid/.openagents/scratch/codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/b2-verification/14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23.vlay')
cmd=[str(target/'examples/baked_light_capture'),str(scratch/'10907-integrated-published-preflight'),'128','10441','--preflight-only']
print('Recording published-bake identity diagnostic before regeneration',flush=True)
with (scratch/'10907-integrated-published-preflight.log').open('w') as log:
 r=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
print('Published-bake preflight exit '+str(r.returncode),flush=True)
env.pop('VERSE_KIT_BAKE',None)
binary=target/'verse-bake'
cmd=[str(binary),'--everglade',str(root/'assets/verse/everglade/a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f.vtp'),'--layers','--backend','cpu','--threads','12','--vertex-rays','128','--probe-rays','256','--bounces','2','--sun-rays','4','--seed','1592593228','--out',str(scratch/'10907-matched-bake')]
meta={'source':source,'profile':'dev with verse-bake and its ray/geometry dependencies optimized by pinned workspace profile','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'capture_binary_sha256':hashlib.sha256((target/'examples/baked_light_capture').read_bytes()).hexdigest(),'command':cmd,'environment':{k:env[k] for k in ('VERSE_QUALITY','VERSE_HOME','VERSE_KIT_PACK')},'start_unix':time.time()}
(scratch/'10907-matched-bake-command.json').write_text(json.dumps(meta,indent=2)+'\n')
print('Starting full128/256-ray current-scene bake on12threads',flush=True)
with (scratch/'10907-matched-bake.log').open('w') as log:
 r=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
meta.update(exit=r.returncode,end_unix=time.time())
(scratch/'10907-matched-bake-command.json').write_text(json.dumps(meta,indent=2)+'\n')
print('Matched bake finished with exit '+str(r.returncode),flush=True)
raise SystemExit(r.returncode)
