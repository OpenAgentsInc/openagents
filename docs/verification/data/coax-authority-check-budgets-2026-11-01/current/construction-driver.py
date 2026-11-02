#!/usr/bin/env python3
import hashlib,json,os,pathlib,shutil,signal,subprocess,time
root=pathlib.Path('/Users/christopherdavid/work/openagents-aethyr-check-budgets-current');out=pathlib.Path('/tmp/aethyr-authority-budget-current-construction');out.mkdir(exist_ok=True);scratch=out/'scratch';scratch.mkdir(exist_ok=True);source=pathlib.Path('/tmp/aethyr-authority-budget-current-source');retained=source/'retained-stage0';old=pathlib.Path('/tmp/aethyr-authority-budget-checker-shards/incomplete.manifest')
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
expected=subprocess.check_output(['coax/provenance.sh','fingerprint',str(root)],cwd=root,text=True,timeout=60).strip();env=os.environ.copy();env['TMPDIR']=str(scratch.resolve())+'/';env['COAX_CHECKER_MUTATION_SHARDS']=str(old);command=['coax/build.sh'];started=time.monotonic();proc=None;saved=None
assert not source.exists(),'refuse overwrite of retained source'
def interrupt(sig,frame):raise KeyboardInterrupt('signal '+str(sig))
for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP):signal.signal(sig,interrupt)
try:
 with (out/'build.log').open('wb') as stream:
  proc=subprocess.Popen(command,cwd=root,env=env,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
  while proc.poll() is None:
   if saved is None:
    for candidate in scratch.glob('coax-build-snapshot.*'):
     records=list((candidate/'coax/bin').glob('.bootstrap-*/stage0.record'))
     if not records:continue
     assert len(records)==1,'ambiguous construction';record=records[0];lines=record.read_text().splitlines()
     if len(lines)!=4:continue
     assert lines[0]=='coax-stage0-artifact-v1' and lines[3]==expected
     binary=record.parent/'stage0';assert sha(binary)==lines[2]
     shutil.copytree(candidate,source,ignore=shutil.ignore_patterns('bin'));retained.mkdir()
     for name in ('stage0','proto_runtime.inc','stage0.record'):shutil.copy2(record.parent/name,retained/name)
     copied=subprocess.check_output([str(source/'coax/provenance.sh'),'fingerprint',str(source)],cwd=source,text=True,timeout=60).strip();assert copied==expected
     saved={'snapshot':str(candidate),'retained_source':str(source),'retained_stage0':str(retained/'stage0'),'stage0_identity':lines[1],'stage0_sha256':lines[2],'input_fingerprint':expected,'stage0_record_sha256':sha(retained/'stage0.record')}
     (out/'construction.json').write_text(json.dumps(saved,indent=2)+'\n');print(json.dumps(saved),flush=True)
   if time.monotonic()-started>825:raise TimeoutError('bounded construction command exceeded825seconds')
   time.sleep(.1)
  code=proc.wait()
except BaseException:
 if proc and proc.poll() is None:
  os.killpg(proc.pid,signal.SIGTERM)
  try:proc.wait(timeout=3)
  except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait(timeout=3)
 raise
finally:
 after=subprocess.check_output(['coax/provenance.sh','fingerprint',str(root)],cwd=root,text=True,timeout=60).strip()
 receipt={'schema':'coax.bounded-current-stage0-construction.v1','root':str(root),'command':command,'proof_manifest':str(old),'exit_code':None if proc is None else proc.returncode,'seconds':round(time.monotonic()-started,3),'input_fingerprint_before':expected,'input_fingerprint_after':after,'construction':saved,'log':str(out/'build.log')}
 (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
assert saved and code==1 and after==expected
assert 'compiler binary differs from shard proof' in (out/'build.log').read_text(),'expected failclosed old-proof boundary not reached'
print('New exactstage0 retained; old-bound manifest correctly refused.',flush=True)
