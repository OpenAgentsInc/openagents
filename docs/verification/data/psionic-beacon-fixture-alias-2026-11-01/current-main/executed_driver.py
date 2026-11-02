#!/usr/bin/env python3
"""Check beacon fixture ownership in CT and generated native C; no network."""
from pathlib import Path
import argparse, concurrent.futures, hashlib, json, os, shutil, signal, subprocess, threading, time
p=argparse.ArgumentParser();p.add_argument('--coax',required=True,type=Path);p.add_argument('--out',required=True,type=Path);p.add_argument('--reuse-native-record',type=Path);a=p.parse_args()
root=Path(__file__).resolve().parents[3];binary=a.coax.resolve();out=a.out.resolve();out.mkdir(parents=True,exist_ok=True)
assert not(out/'qualification.json').exists(),'Use a fresh output directory; do not overwrite an earlier proof.'
sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest();compiler=sha(binary);before={str(f.relative_to(root)):sha(f)for f in root.rglob('*.coax')if f.is_file()};active={};lock=threading.Lock();deadline=time.monotonic()+810;started=time.monotonic();commands=[]
def stop(child):
 if child.poll()is not None:return
 try:os.killpg(child.pid,signal.SIGTERM)
 except ProcessLookupError:return
 try:child.wait(timeout=3)
 except subprocess.TimeoutExpired:
  try:os.killpg(child.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  child.wait(timeout=3)
def interrupted(sig,frame):raise KeyboardInterrupt('signal '+str(sig))
for sig in(signal.SIGINT,signal.SIGTERM,signal.SIGHUP):signal.signal(sig,interrupted)
def run(command,label,output=None):
 began=time.monotonic();stream=output.open('wb')if output else subprocess.PIPE
 try:
  with lock:
   child=subprocess.Popen(command,cwd=root,stdout=stream,stderr=subprocess.PIPE if output else subprocess.STDOUT,start_new_session=True);active[child.pid]=child
  try:stdout,stderr=child.communicate(timeout=max(.1,min(180,deadline-time.monotonic())))
  except BaseException:stop(child);raise
  finally:
   with lock:active.pop(child.pid,None)
 finally:
  if output:stream.close()
 text=stderr if output else stdout;(out/(label+'.log')).write_bytes(text or b'');r={'command':list(map(str,command)),'returncode':child.returncode,'seconds':round(time.monotonic()-began,3),'log':label+'.log','stdout':(text or b'').decode()};commands.append(r);assert child.returncode==0,r;return r
try:
 run([str(binary),'test','psionic/tests/pylon/beacon_alias_checks.coax','--library','--evidence',str(out/'checks.evidence.json')],'checks')
 run([str(binary),'mutate','psionic/lib/pylon/beacon.coax','--library','--root-only','--jobs','4','--evidence',str(out/'mutation.evidence.json')],'mutation')
 run([str(binary),'c','psionic/tests/pylon/beacon_alias_probe.coax','--library','--header',str(out/'Probe.h')],'emission',out/'Probe.c')
 driver=root/'psionic/tests/pylon/beacon_alias_native.c';old=json.loads(a.reuse_native_record.read_text())if a.reuse_native_record else None;reuse=False
 if old:
  old_driver=[h for name,h in old['source_sha256'].items()if name.endswith('/psionic/tests/pylon/beacon_alias_native.c')]
  reuse=old['status']=='pass'and old['generated_c_sha256']==sha(out/'Probe.c')and old['generated_header_sha256']==sha(out/'Probe.h')and old_driver==[sha(driver)]
 if reuse:
  native={'status':'reused_identical_generated_inputs','prior_record':str(a.reuse_native_record.resolve()),'prior_record_sha256':sha(a.reuse_native_record),'modes':old['native_modes'],'scope':'Exact C,header and driver bytes match the prior three native modes; no native execution repeated.'}
 else:
  cc=shutil.which('cc');assert cc
  def mode(label):
   flags={'debug':['-O0'],'optimized':['-O2'],'sanitized':['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']}[label];exe=out/(label+'-probe');build=run([cc,'-std=c11','-ffp-contract=off','-fno-fast-math',*flags,'-I'+str(out),str(out/'Probe.c'),str(driver),'-lm','-o',str(exe)],label+'-build');result=run([str(exe)],label+'-run');observed=list(map(int,result['stdout'].split()));assert observed==[4,2,4,148,148,44,93],observed;return{'mode':label,'build':build,'run':result,'observed':observed}
  with concurrent.futures.ThreadPoolExecutor(max_workers=3)as pool:modes=list(pool.map(mode,['debug','optimized','sanitized']))
  native={'status':'passed','modes':modes,'cc':cc,'cc_sha256':sha(cc),'cc_version':subprocess.check_output([cc,'--version'],text=True,timeout=30)}
 checks=json.loads((out/'checks.evidence.json').read_text());mutation=json.loads((out/'mutation.evidence.json').read_text());m=mutation['mutation'];assert m['outcome']=='passed'and m['killed']==m['mutants']and not m['survived']and m['marked_equivalent']==0
 used={str(Path(row['path']).relative_to(root)):row['sha256']for row in checks['sources']};changes=[name for name,h in used.items()if before.get(name)!=h or sha(root/name)!=h];assert not changes and sha(binary)==compiler
 q={'schema':'psionic.beacon.alias-parity.v1','status':'passed','root':str(root),'compiler':str(binary),'compiler_sha256':compiler,'compiler_source_digest':subprocess.check_output([str(binary),'source-digest',str(root)],text=True,timeout=60).strip(),'commands':commands,'seconds':round(time.monotonic()-started,3),'source_sha256':used,'source_changes':changes,'generated_c_sha256':sha(out/'Probe.c'),'generated_header_sha256':sha(out/'Probe.h'),'checks':sum(row.get('outcome')=='passed'for row in checks['claims']),'mutation':m,'native':native,'scope':'The original beacon fixture alone gains a separate baseline owner. Existing product procedures/exports and every assertion remain unchanged. CT uses the fixed full imported check suite plus the independent alias witness. Native uses normal checked Coax C; optimized means host-C O2,not Coax--release.'};(out/'qualification.json').write_text(json.dumps(q,indent=2)+'\n');print(json.dumps({'status':q['status'],'checks':q['checks'],'mutants':m['mutants'],'killed':m['killed'],'native_status':native['status'],'seconds':q['seconds'],'source_changes':changes}))
finally:
 with lock:children=list(active.values())
 for child in children:stop(child)
