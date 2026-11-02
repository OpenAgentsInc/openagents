#!/usr/bin/env python3
# Firm reason: 2 (independent documentation/maintenance-gate process and input-identity oracle).
from pathlib import Path
import hashlib,importlib.util,json,os,time
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve();OUT=Path('/tmp/coax-12625-closure/book-final-post-rebase');OUT.mkdir(exist_ok=False)
spec=importlib.util.spec_from_file_location('bp',ROOT/'coax/tools/bounded_process.py');bp=importlib.util.module_from_spec(spec);spec.loader.exec_module(bp)
rows=[];deadline=time.monotonic()+830
for name,argv in [('book',['python3','coax/book/check-structure.py']),('shell',['bash','-n','coax/tests/run.sh']),('tools',['coax/web/tool.sh','tools-check']),('whitespace',['git','diff','--check','origin/main'])]:
 r=bp.run(argv,ROOT,os.environ.copy(),min(180,max(1,deadline-time.monotonic())),33554432,deadline,quiet_faults=True)
 for k in ['stdout','stderr']:(OUT/(name+'.'+k)).write_bytes(r.pop(k))
 r['argv']=argv;(OUT/(name+'.json')).write_text(json.dumps(r,indent=2)+'\n')
 rows.append({'name':name,'exit_status':r['returncode'],'timeout':r['timeout'],'record':name+'.json'})
 print(name,r['returncode'],flush=True)
record={'schema':'coax.12625.closure.reader-maintenance.v1','commands':rows,'scope':'Changed Coax book/route registry, maintenance shell and tracked-tools policy; tools output retains any independent upstream gate.'}
(OUT/'record.json').write_text(json.dumps(record,indent=2)+'\n')
