#!/usr/bin/env python3
# Firm reason: 2 (independent input-identity and combined authored-check oracle).
from pathlib import Path
import hashlib,importlib.util,json,os,time
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve()
OUT=Path('/tmp/coax-12625-closure/focused-v6');OUT.mkdir(exist_ok=False)
IMAGE=Path('/tmp/coax-12625-closure/native-v6/coax')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
policies=['results-borrow-policy','results-sum-policy','results-cache-policy','results-projection-policy','results-origin-policy']
entry=OUT/'combined.coax'
entry.write_text(''.join('#load "'+str(ROOT/'coax/tests'/(p+'.coax'))+'";\n' for p in policies)+'#authority;\n')
paths=[IMAGE,Path(__file__),entry,*[ROOT/'coax/tests'/(p+'.coax') for p in policies]]
paths.extend(p for d in ['coax/src','coax/lib'] for p in (ROOT/d).rglob('*') if p.is_file())
before={str(p):sha(p) for p in paths}
spec=importlib.util.spec_from_file_location('closure_bp',ROOT/'coax/tools/bounded_process.py');bp=importlib.util.module_from_spec(spec);spec.loader.exec_module(bp)
argv=[str(IMAGE),'test',str(entry),'--library','--evidence',str(OUT/'checks.json')]
r=bp.run(argv,ROOT,os.environ.copy(),840,33554432,time.monotonic()+845,quiet_faults=True)
for k in ['stdout','stderr']:(OUT/(k+'.log')).write_bytes(r.pop(k))
after={str(p):sha(p) for p in paths}
flags=['timeout','deadline_exhausted','stdout_limit','stderr_limit','output_limit','group_killed','launch_errno','launch_error','kill_errno','read_errno']
passed=r['started'] and r['returncode']==0 and r['stdout_eof'] and r['stderr_eof'] and not any(r[k] for k in flags) and before==after
r.update(argv=argv,inputs_before=before,inputs_after=after,inputs_unchanged=before==after,passed=passed,scope='All authored checker authority checks plus all five external initial-R2 policy suites, together on the final native image. No full mutation sweep.')
(OUT/'record.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'passed':passed,'seconds':r['elapsed_s'],'stdout_tail':(OUT/'stdout.log').read_text()[-1000:]}))
raise SystemExit(0 if passed else 1)
