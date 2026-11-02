from pathlib import Path
import subprocess,hashlib,json,os,signal,concurrent.futures,time,shutil
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');out=Path('/tmp/aethyr-ch11-beacon-alias');native=out/'native-before';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();inputs=[root/'psionic/lib/pylon/beacon.coax',root/'psionic/lib/pylon/template.coax',root/'coax/lib/nostr/json.coax',root/'psionic/tests/pylon/beacon_alias_probe.coax',root/'psionic/tests/pylon/beacon_alias_checks.coax',root/'psionic/tests/pylon/beacon_alias_native.c',native/'Probe.c',native/'Probe.h'];before={str(p):sha(p)for p in inputs};cc=shutil.which('cc');name='a borrowed beacon tag view sees scratch writes while a separate owner stays stable'
def command(cmd,label,expected=0):
 start=time.monotonic();p=subprocess.Popen(cmd,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
 try:text,_=p.communicate(timeout=180)
 except BaseException:
  os.killpg(p.pid,signal.SIGTERM)
  try:p.wait(timeout=3)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=3)
  raise
 (out/(label+'.log')).write_bytes(text);r={'command':cmd,'returncode':p.returncode,'expected_returncode':expected,'seconds':round(time.monotonic()-start,3),'stdout':text.decode()};assert p.returncode==expected,r;return r
def native_mode(mode):
 flags={'debug':['-O0'],'optimized':['-O2'],'sanitized':['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']}[mode];exe=native/(mode+'-probe');cmd=[cc,'-std=c11','-ffp-contract=off','-fno-fast-math',*flags,'-I'+str(native),str(native/'Probe.c'),str(root/'psionic/tests/pylon/beacon_alias_native.c'),'-lm','-o',str(exe)];build=command(cmd,mode+'-build');run=command([str(exe)],mode+'-run');observed=[int(v)for v in run['stdout'].split()];assert observed==[4,2,4,148,148,44,93],observed;return {'mode':mode,'build':build,'run':run,'observed':observed}
def ct(label,binary,expected):
 evidence=out/(label+'-alias-witness.evidence.json');cmd=[str(binary),'test','psionic/tests/pylon/beacon_alias_checks.coax','--library','--only',name,'--evidence',str(evidence)];r=command(cmd,label+'-alias-witness',expected);r.update(compiler_sha256=sha(binary),evidence=str(evidence));return r
with concurrent.futures.ThreadPoolExecutor(max_workers=5)as pool:
 fs=[pool.submit(native_mode,m)for m in ['debug','optimized','sanitized']];cs=[pool.submit(ct,'308',root/'coax/bin/coax',1),pool.submit(ct,'1fac',Path('/Users/christopherdavid/work/openagents-aethyr-check-budgets-current/coax/bin/coax'),0)];modes=[f.result()for f in fs];cts=[f.result()for f in cs]
changes=[p for p,h in before.items()if sha(Path(p))!=h];assert not changes
r={'schema':'aethyr.beacon.alias-native-ct.v1','status':'confirmed_preexisting_fixture_alias','scope':'NormalC from308 preserves borrowed-view aliasing and agrees with1fac CT.308 CT snapshots the old string and fails the independent alias witness. Productbeacon functions are unchanged.','source_sha256':before,'source_changes':changes,'native_emission_compiler':'30868e8cd36a54cf3b327ba05e74a4cd80d90af1c209d98105f070e6ce8b9980','native_emission':'coax c probe --library --header Probe.h; original308 authority checks passed before emission','native_modes':modes,'ct':cts,'compiler_version':subprocess.check_output([cc,'--version'],text=True),'expected_literal_tags':'[["d","studio-gpu"],["t","oa:pylon-beacon:v1"],["x","'+'1'*64+'"],["expiration","1791400300"]]','expected_count':148,'expected_statuses':{'before':'digest','borrowed_after_overwrite':'tags','separate_owner':'digest'}}
(out/'native-before.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'status':r['status'],'native_modes':[m['mode']for m in modes],'observed':modes[0]['observed'],'CT308':'independent witness fails','CT1fac':'independent witness passes','source_changes':changes}))
