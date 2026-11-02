# Firm reason: 2 (independent native O0/O2/fatal-sanitizer ABI oracle and recorder).
from pathlib import Path
import subprocess,hashlib,os,time,json,signal
root=Path('/tmp/openagents-12625-complete-20261101');out=Path('/tmp/parakeet-api-profiles');out.mkdir(exist_ok=False);base=Path('/tmp/parakeet-qualified')
inputs=[base/'parakeet-runtime.c',base/'parakeet-runtime.h',root/'psionic/platform/parakeet-face.c',root/'psionic/platform/psionic-parakeet.h',root/'psionic/tests/parakeet-runtime-api.c'];before={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
rows=[];deadline=time.monotonic()+830
for name,flags in [('O0',['-O0','-g']),('O2',['-O2']),('asan',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
 folder=out/name;folder.mkdir();common=['cc','-std=c11','-ffp-contract=off','-fno-fast-math','-Wno-unused-function','-Wno-unused-parameter','-I'+str(base),'-I'+str(root/'psionic/platform'),*flags]
 commands=[common+['-c',str(base/'parakeet-runtime.c'),'-o',str(folder/'runtime.o')],common+['-DPSIONIC_PARAKEET_LIBRARY','-c',str(root/'psionic/platform/parakeet-face.c'),'-o',str(folder/'face.o')],common+[str(root/'psionic/tests/parakeet-runtime-api.c'),str(folder/'runtime.o'),str(folder/'face.o'),'-lm','-lpthread','-o',str(folder/'check')],[str(folder/'check')]]
 for i,argv in enumerate(commands):
  start=time.monotonic();env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'}
  with (folder/(str(i)+'.stdout')).open('wb') as so,(folder/(str(i)+'.stderr')).open('wb') as se:
   p=subprocess.Popen(argv,cwd=root,stdout=so,stderr=se,env=env,start_new_session=True)
   try:rc=p.wait(timeout=min(180,max(1,deadline-time.monotonic())));timeout=False
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait();rc=p.returncode;timeout=True
  row={'profile':name,'argv':argv,'returncode':rc,'timeout':timeout,'seconds':time.monotonic()-start};rows.append(row);print(name,i,rc,flush=True)
  if rc:raise RuntimeError(row)
after={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs};r={'schema':'psionic.parakeet.native-api-profiles.v1','commands':rows,'inputs_before':before,'inputs_after':after,'inputs_unchanged':before==after,'passed':all(x['returncode']==0 and not x['timeout'] for x in rows),'scope':'Changed public resident API lifecycle/refusals/output atomicity at native O0/O2 and fatal ASan+UBSan with leak checking; CPU-only build path links explicit unavailable-GPU stubs. Actual full model positives are separate resident decoder/GPU corpus observations.'};(out/'record.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'passed':r['passed'],'inputs_unchanged':r['inputs_unchanged']}))
