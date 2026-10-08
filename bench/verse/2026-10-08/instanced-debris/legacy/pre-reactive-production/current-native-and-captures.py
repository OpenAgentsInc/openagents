import subprocess
from pathlib import Path
scratch=Path(__file__).parent
root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')


import os, hashlib, json, time
binary=Path('/Users/christopherdavid/work/openagents-target-agent2/debug/examples/baked_light_capture')
out=scratch/'10907-current-main-preflight'
env=os.environ.copy()
env.pop('VERSE_KIT_UNPINNED',None)
env.update(VERSE_QUALITY='high',VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp',VERSE_KIT_BAKE='/Users/christopherdavid/.openagents/scratch/codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/b2-verification/14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23.vlay')
cmd=[str(binary),str(out),'128','10441','--preflight-only']
meta={'source':subprocess.check_output(['git','rev-parse','3e64067821'],cwd=root,text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'command':cmd,'environment':{k:env[k] for k in ('VERSE_QUALITY','VERSE_KIT_PACK','VERSE_KIT_BAKE')},'start_unix':time.time()}
print('Running current-main B3 preflight',flush=True)
with (scratch/'10907-current-main-preflight.log').open('w') as log:
 r=subprocess.run(cmd,cwd='/Users/christopherdavid/.codex/worktrees/10907-budgeted-relight/openagents',env=env,stdout=log,stderr=subprocess.STDOUT)
meta.update(exit=r.returncode,end_unix=time.time())
(scratch/'10907-current-main-preflight-command.json').write_text(json.dumps(meta,indent=2)+'\n')
print('B3 preflight exit '+str(r.returncode),flush=True)

import re
for build_log, pattern, tests, prefix in [
 ('10936-10938-current-main-retry-build.log',r'Running unittests.*\(([^\n]*?/verse_pbr-[a-f0-9]+)\)',[
  'pbr::temporal::tests::hidden_motion_cannot_overwrite_the_visible_surface',
  'pbr::temporal::tests::empty_motion_keeps_camera_reprojection_with_stale_object_data'], '10936-frustum-native'),
 ('10907-current-main-retry-build.log',r'Running unittests.*\(([^\n]*?/verse_pbr-[a-f0-9]+)\)',[
  'pbr::gpu::baked_tests::static_light_patches_cross_rows_and_layers_and_survive_late_bakes_until_restore',
  'pbr::gpu::baked_tests::rigid_light_ranges_wrap_max_grow_rebind_and_restore_direct_ambient'], '10907-layer-current-native')]:
 binary=re.search(pattern,(scratch/build_log).read_text()).group(1)
 for i,test in enumerate(tests):
  print('Running native regression '+test,flush=True)
  with (scratch/(prefix+'-'+str(i)+'.log')).open('w') as log:
   result=subprocess.run([binary,test,'--ignored','--exact','--test-threads=1'],cwd=root,stdout=log,stderr=subprocess.STDOUT)
  if result.returncode: raise SystemExit(result.returncode)

import hashlib, json, os, subprocess, time
from pathlib import Path
scratch=Path(__file__).parent
root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
target=Path('/Users/christopherdavid/work/openagents-target-agent0')
common=['--live','--settle-light','--no-video','--seconds','16','--impact-frame','469','--smoke-frame','600']
jobs=[
 ('10937-high-frustum-dev', 'debug', 'high', common),
 ('10937-high-frustum-release', 'release', 'high', common),
 ('10936-high-frustum-release-paired', 'release', 'high', common+['--compare-temporal-aa','--sequence','469:484']),
 ('10936-pan-frustum-release-paired', 'release', 'high', ['--live','--settle-light','--no-video','--seconds','8','--static-houses','--camera','pan','--compare-temporal-aa','--sequence','120:135']),
 ('10936-orbit-frustum-release-paired', 'release', 'high', ['--live','--settle-light','--no-video','--seconds','8','--static-houses','--camera','orbit','--compare-temporal-aa','--sequence','120:135']),
 ('10936-medium-frustum-release-paired', 'release', 'medium', ['--live','--settle-light','--no-video','--seconds','8','--compare-temporal-aa','--sequence','360:375']),
 ('10938-frustum-release-off', 'release', 'high', common+['--serial-frames','--no-temporal-aa','--capture-rebuild','--no-destruction-relighting']),
 ('10938-frustum-release-on', 'release', 'high', common+['--serial-frames','--no-temporal-aa','--capture-rebuild']),
]
manifest=[]
for name, profile, quality, args in jobs:
 binary=target/profile/'examples/meteor_showcase_capture'
 out=scratch/name
 env=os.environ.copy()
 env.update(VERSE_QUALITY=quality, VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp')
 command=[str(binary),str(out)]+args
 source=subprocess.check_output(['git','rev-parse','46cf41188b'],cwd=root,text=True).strip() # Recorded current-main build lease.
 record={'name':name,'profile':profile,'quality':quality,'source':source,'build_command':['cargo','build']+(['--release'] if profile=='release' else [])+['-p','verse','--example','meteor_showcase_capture','--features','capture'],'environment':{k:env[k] for k in ('VERSE_QUALITY','VERSE_KIT_PACK')},'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'command':command,'start_unix':time.time()}
 print('Starting '+name,flush=True)
 with (scratch/(name+'.log')).open('w') as log:
  result=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 record.update(exit=result.returncode,end_unix=time.time())
 manifest.append(record)
 (scratch/'10936-10938-frustum-captures-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 print('Finished '+name+' with exit '+str(result.returncode),flush=True)
 if result.returncode: raise SystemExit(result.returncode)

