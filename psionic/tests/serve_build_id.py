#!/usr/bin/env python3
"""Build tooling: bind the generated program, native sources and tool identities."""
import hashlib,json,os
from pathlib import Path
import subprocess,sys
out=Path(sys.argv[1]);coax=Path(sys.argv[2]);cc=sys.argv[3]
paths=[out/'serve.c',out/'serve.h',out/'elephant-inventory.h']+[p for p in out.glob('*') if p.suffix in ('.cu','.metal') or p.name in ('plan.c','plan.h')]
names=['serve-host.c','serve-supervisor.c','serve-notify.c','serve-notify.h','serve-busy.c','serve-busy.h','verified-file.c','verified-file.h','decision-session-memory.c','decision-session-memory.h','spans.h','serve-build.sh']
if sys.argv[6]=='gpu':names+=['cuda-host.c','cuda-host.h','elephant-gpu.c','elephant-gpu.h']
if sys.argv[6]=='metal':names+=['metal-host.m','metal-host.h','elephant-metal.c','elephant-metal.h']
paths += [Path('psionic/platform')/name for name in names]
paths += [Path('psionic/tests/serve_build_id.py'),Path('psionic/fixtures/models/mammoth/inventory.h')]
items={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))}
shader=os.environ.get('COAX_METAL',str(coax))
record={'metal_coax_sha256':hashlib.sha256(Path(shader).read_bytes()).hexdigest() if sys.argv[6]=='metal' else None,'schema':'psionic-serve-build-v1','coax_sha256':hashlib.sha256(coax.read_bytes()).hexdigest(),'cc':subprocess.check_output([cc,'--version'],text=True).splitlines()[0],'mode':sys.argv[4],'native':sys.argv[5],'backend':sys.argv[6],'source_sha256':items}
# Output directory names do not alter the producer identity.
identity=dict(record,source_sha256={str(Path(k).name) if str(out) in k else k:v for k,v in items.items()})
digest=hashlib.sha256(json.dumps(identity,sort_keys=True,separators=(',',':')).encode()).hexdigest()
record['build']=digest;(out/'serve-build.json').write_text(json.dumps(record,indent=2,sort_keys=True)+'\n')
(out/'serve-build-id.h').write_text('#define PSIONIC_BUILD_ID "'+digest+'"\n')
