import hashlib,json,os,re,subprocess,time
from pathlib import Path
scratch=Path(__file__).parent
root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
target=Path('/Users/christopherdavid/work/openagents-target-agent0')
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
log=(scratch/'10936-reactive-native-fixture-build.log').read_text()
native=Path(re.search(r'Running unittests.*\(([^\n]*?/verse_pbr-[a-f0-9]+)\)',log).group(1))
manifest=[]
tests=['pbr::temporal::tests::hidden_motion_cannot_overwrite_the_visible_surface','pbr::temporal::tests::empty_motion_keeps_camera_reprojection_with_stale_object_data','pbr::temporal::reactive_tests::moving_reactive_geometry_cannot_leave_history_in_a_bright_trail','pbr::temporal::reactive_tests::reactive_history_rejection_matches_the_actual_bilinear_footprint']
for i,test in enumerate(tests):
 cmd=[str(native),test,'--ignored','--exact','--test-threads=1']
 record={'name':'10936-reactive-native-'+str(i),'source':source,'binary_sha256':hashlib.sha256(native.read_bytes()).hexdigest(),'command':cmd,'start_unix':time.time()}
 print('Running '+test,flush=True)
 with (scratch/(record['name']+'.log')).open('w') as out:
  result=subprocess.run(cmd,cwd=root,stdout=out,stderr=subprocess.STDOUT)
 record.update(exit=result.returncode,end_unix=time.time());manifest.append(record)
 (scratch/'10936-reactive-native-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 if result.returncode:raise SystemExit(result.returncode)
if os.environ.get('NATIVE_ONLY') == '1': raise SystemExit(0)
source=subprocess.check_output(['git','rev-parse','36df9ceb4b'],cwd=root,text=True).strip() # Exact production capture build; later fixture-only change does not rebuild it.
common=['--live','--settle-light','--no-video','--seconds','16','--impact-frame','469','--smoke-frame','600']
jobs=[('10937-high-reactive-dev','debug','high',common),('10937-high-reactive-release','release','high',common),('10936-high-reactive-release-paired','release','high',common+['--compare-temporal-aa','--sequence','439:484']),('10936-medium-reactive-release-paired','release','medium',common+['--compare-temporal-aa','--sequence','439:484']),('10936-pan-reactive-release-paired','release','high',['--live','--settle-light','--no-video','--seconds','8','--static-houses','--camera','pan','--compare-temporal-aa','--sequence','120:135']),('10936-orbit-reactive-release-paired','release','high',['--live','--settle-light','--no-video','--seconds','8','--static-houses','--camera','orbit','--compare-temporal-aa','--sequence','120:135'])]
manifest=[]
for name,profile,quality,args in jobs:
 binary=target/profile/'examples/meteor_showcase_capture';env=os.environ.copy()
 env.pop('VERSE_KIT_UNPINNED',None);env.pop('VERSE_KIT_BAKE',None);env.pop('VERSE_TEMPORAL_AA',None)
 env.update(VERSE_QUALITY=quality,VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp')
 cmd=[str(binary),str(scratch/name)]+args
 record={'name':name,'source':source,'profile':profile,'quality':quality,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'command':cmd,'environment':{k:env[k] for k in ('VERSE_QUALITY','VERSE_KIT_PACK')},'start_unix':time.time()}
 print('Starting '+name,flush=True)
 with (scratch/(name+'.log')).open('w') as out:result=subprocess.run(cmd,cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT)
 record.update(exit=result.returncode,end_unix=time.time());manifest.append(record)
 (scratch/'10936-reactive-captures-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 print('Finished '+name+' exit '+str(result.returncode),flush=True)
 if result.returncode:raise SystemExit(result.returncode)
