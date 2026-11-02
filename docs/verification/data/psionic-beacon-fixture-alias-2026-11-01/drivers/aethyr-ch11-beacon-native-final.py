from pathlib import Path
import subprocess,hashlib,json,os,signal,concurrent.futures,time,shutil
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');out=Path('/tmp/aethyr-ch11-beacon-alias');native=out/'native-final';native.mkdir(exist_ok=True);binary=Path('/Users/christopherdavid/work/openagents-aethyr-check-budgets-current/coax/bin/coax');sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();cc=shutil.which('cc');compiler=sha(binary);assert compiler=='1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa'
evidence=json.loads((out/'final/checks.evidence.json').read_text());source={str(Path(r['path'])):r['sha256']for r in evidence['sources']};source[str(root/'psionic/tests/pylon/beacon_alias_probe.coax')]=sha(root/'psionic/tests/pylon/beacon_alias_probe.coax');source[str(root/'psionic/tests/pylon/beacon_alias_native.c')]=sha(root/'psionic/tests/pylon/beacon_alias_native.c');assert all(sha(Path(p))==h for p,h in source.items())
def run(cmd,name,output=None):
 start=time.monotonic()
 if output:
  with output.open('wb')as stream:p=subprocess.Popen(cmd,cwd=root,stdout=stream,stderr=subprocess.PIPE,start_new_session=True);dummy,text=p.communicate(timeout=180)
 else:
  p=subprocess.Popen(cmd,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
  try:text,_=p.communicate(timeout=180)
  except BaseException:
   os.killpg(p.pid,signal.SIGTERM)
   try:p.wait(timeout=3)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=3)
   raise
 (native/(name+'.log')).write_bytes(text);r={'command':cmd,'returncode':p.returncode,'seconds':round(time.monotonic()-start,3),'stdout':text.decode()};assert p.returncode==0,r;return r
emit=run([str(binary),'c','psionic/tests/pylon/beacon_alias_probe.coax','--library','--header',str(native/'Probe.h')],'emission',native/'Probe.c')
def mode(label):
 flags={'debug':['-O0'],'optimized':['-O2'],'sanitized':['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']}[label];exe=native/(label+'-probe');cmd=[cc,'-std=c11','-ffp-contract=off','-fno-fast-math',*flags,'-I'+str(native),str(native/'Probe.c'),str(root/'psionic/tests/pylon/beacon_alias_native.c'),'-lm','-o',str(exe)];build=run(cmd,label+'-build');result=run([str(exe)],label+'-run');observed=[int(v)for v in result['stdout'].split()];assert observed==[4,2,4,148,148,44,93];return{'mode':label,'build':build,'run':result,'observed':observed}
with concurrent.futures.ThreadPoolExecutor(max_workers=3)as pool:modes=list(pool.map(mode,['debug','optimized','sanitized']))
changes=[p for p,h in source.items()if sha(Path(p))!=h];assert not changes and sha(binary)==compiler
r={'schema':'aethyr.beacon.final-native-parity.v1','status':'pass','compiler_sha256':compiler,'source_sha256':source,'source_changes':changes,'emission':emit,'generated_c_sha256':sha(native/'Probe.c'),'generated_header_sha256':sha(native/'Probe.h'),'native_modes':modes,'scope':'Actual normal-C emission on the final1fac compiler, checked Coax arithmetic; optimized labels C-O2,not Coax--release. Same independent7word alias observation as the before-native proof and1fac CT witness. Full fixedbeacon CT checks and839-site mutation have separate receipts.'};(out/'native-final.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'status':r['status'],'modes':[m['mode']for m in modes],'source_changes':changes,'generated_c_sha256':r['generated_c_sha256']}))
