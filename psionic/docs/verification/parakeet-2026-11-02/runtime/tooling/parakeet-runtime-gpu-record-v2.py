# Firm reason: 2 (independent bounded actual-weight GPU corpus recorder).
from pathlib import Path
import subprocess,time,json,hashlib,os,signal
root=Path('/tmp/openagents-12625-complete-20261101');out=Path('/tmp/parakeet-gpu-corpus-v2');out.mkdir(exist_ok=False)
paths=[root/'psionic/platform/parakeet-face.c',root/'psionic/platform/psionic-parakeet.h',root/'psionic/tests/parakeet-runtime-gpu.c',Path('/tmp/parakeet-runtime-gpu-v2'),Path('/tmp/parakeet-gpu-inputs.tsv'),*list((root/'psionic/lib/parakeet').glob('*.coax')),*list(Path('/tmp/parakeet-qualified').glob('*.o')),*list(Path('/tmp/parakeet-qualified').glob('*.c')),*list(Path('/tmp/parakeet-qualified').glob('*.h'))]
pins={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
argv=['/tmp/parakeet-runtime-gpu-v2',str(Path.home()/'.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99'),'/tmp/parakeet-gpu-inputs.tsv',str(out)]
start=time.monotonic()
with (out/'stdout').open('wb') as so,(out/'stderr').open('wb') as se:
 p=subprocess.Popen(argv,cwd=root,stdout=so,stderr=se,start_new_session=True)
 try:rc=p.wait(timeout=840);timeout=False
 except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait();rc=p.returncode;timeout=True
r={'argv':argv,'returncode':rc,'timeout':timeout,'seconds':time.monotonic()-start,'pins_before':pins,'pins_after':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},'scope':'ONE actual pinned model open with GPUrequested, followed by all29 frozen PCM clips through public resident C API; frontend/encoder/TDT/tokenframes/text dumps plus timings. No full CPU encoder.'};r['inputs_unchanged']=r['pins_before']==r['pins_after'];(out/'process.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'returncode':rc,'timeout':timeout,'seconds':r['seconds'],'inputs_unchanged':r['inputs_unchanged']}))
