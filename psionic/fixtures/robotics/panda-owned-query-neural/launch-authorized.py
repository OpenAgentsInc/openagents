#!/usr/bin/env python3
from pathlib import Path
import datetime,hashlib,json,os,sys
here=Path(__file__).resolve().parent
p=here/'prelaunch-audit-contract.json';raw=p.read_bytes();c=json.loads(raw)
def ident(p):
 b=Path(p).read_bytes();return dict(bytes=len(b),sha256=hashlib.sha256(b).hexdigest())
for name,item in c['targets'].items():
 if ident(item['path'])!=item['identity']:raise SystemExit('prelaunch identity refused: '+name)
g=json.loads(Path(c['targets']['reader_guards']['path']).read_text())
if not g['passed']or g['source']!=c['targets']['independent_auditor']['identity']:raise SystemExit('prelaunch reader qualification refused')
(here/'prelaunch-verified.json').write_text(json.dumps(dict(schema='psionic.panda-owned-query-neural.prelaunch-verified.v1',passed=True,verified_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),contract=ident(p),launcher=ident(__file__),targets=c['targets']),indent=2)+'\n')
os.execv(sys.executable,[sys.executable,c['targets']['driver']['path'],'--run-prepared',str(here)])
