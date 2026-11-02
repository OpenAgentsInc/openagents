# Firm reason: 2 (independent bounded compiler-qualification process recorder).
from pathlib import Path
import subprocess,time,json,os,signal,hashlib
root=Path('/tmp/openagents-12625-complete-20261101');out=Path('/tmp/parakeet-policy-mutation-v2');out.mkdir(exist_ok=False)
argv=[str(root/'coax/bin/coax'),'mutate','psionic/lib/parakeet/model-policy.coax','--library','--root-only','--jobs','8','--evidence',str(out/'evidence.json')]
pins={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for d in ['psionic/lib','psionic/tests'] for p in(root/d).rglob('*.coax')}
start=time.monotonic()
with (out/'stdout').open('wb') as so,(out/'stderr').open('wb') as se:
 p=subprocess.Popen(argv,cwd=root,stdout=so,stderr=se,start_new_session=True)
 try:rc=p.wait(timeout=840);timeout=False
 except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait();rc=p.returncode;timeout=True
r={'argv':argv,'returncode':rc,'timeout':timeout,'seconds':time.monotonic()-start,'source_pins':pins};(out/'process.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'returncode':rc,'timeout':timeout,'seconds':r['seconds']}))
