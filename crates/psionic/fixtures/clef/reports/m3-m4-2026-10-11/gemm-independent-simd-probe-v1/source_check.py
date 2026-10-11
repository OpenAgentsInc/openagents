#!/usr/bin/env python3
"""Small source-only checks; never imports Metal or reads weight payloads."""
import ast
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
ROOT = Path(__file__).resolve().parent
REPO = Path(os.environ['CLEF_REPO'])
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
manifest = json.loads((ROOT/'manifest.json').read_text())
for path in ROOT.glob('*.py'): ast.parse(path.read_text(), filename=str(path))
for entry in manifest['production_sources']:
    assert sha(REPO/entry['path']) == entry['sha256'], entry['path']
assert sha(ROOT/'weights.json') == manifest['weights_manifest_sha256']
# These preserved extracts and baseline are identical to the previous probe.
for name in ['baseline.metal','production-shapes.metal','production-fixed-extract.metal','production-silu-extract.metal','production-planner.rs','weights.json']:
    assert sha(ROOT/name) == {'baseline.metal': '9505a34d21d4d6bd11a532a62549d00c8cafbad90ab92f2bd07cccb4eae30725', 'production-shapes.metal': '392d4e53863fcfe61519e1108384bd614d2400750fe94830cb432e30fa40f978', 'production-fixed-extract.metal': '6a52d4323c13c85bf0bd7b6847f57216ed98d9bd07c8f95382a774f7e8920e25', 'production-silu-extract.metal': 'fe7844912a63f4219dc05a5a50a5b269ad48a0e3aa5f60cef221ae9223880e61', 'production-planner.rs': 'b0a83f7dafc9257b4f3f84afd3079dd27857f70e7c7cfdd00d7a15217debecac', 'weights.json': '8f05ea58d073e3fc31b73743ab12b922c1d56bfb2347e0e81e0c6bd70303f5f8'}[name], name
shapes = [(1,1,1),(3,7,127),(7,129,257),(63,1,4096),(64,128,65),(65,129,257),(257,1,12288),(1,33,4096),(33,1,12288)]
shapes += [(n,m,k) for n,m,k in [(31,31,129),(32,32,128),(33,33,129),(64,64,256),(65,65,257)]]
for n,m,k in shapes:
    owned = Counter()
    for gy in range((n+63)//64):
        for gx in range((m+63)//64):
            for sg in range(4):
                row,col = gy*64+(sg//2)*32, gx*64+(sg%2)*32
                if row>=n or col>=m: continue
                nr,mc = min(32,n-row),min(32,m-col)
                for r in range(nr):
                    for c in range(mc): owned[row+r,col+c] += 1
                pieces=[]
                for k0 in range(0,k,128):
                    kc=min(128,k-k0);pieces.extend(range(k0,k0+kc))
                    assert (row+nr-1)*k+k0+kc-1 < n*k
                    assert (col+mc-1)*k+k0+kc-1 < m*k
                assert pieces==list(range(k))
    assert len(owned)==n*m and set(owned.values())=={1}
expected=manifest['expected']
assert expected['check_only_commands']==18*2+5*2==46
assert expected['check_only_dispatches']==18*2+5*8==76
assert expected['total_commands']==46+3*3+240==295
assert expected['total_dispatches']==76+3*9+5*4*3*(1+1+1+4)==523
account=manifest['memory_accounting'];state=2048*(12288*10+4096*4)+4*512
weights=3*(4096*12288*2+512);x=2048*4096*2+512
assert account['initial_explicit_buffers']==weights+x+2*state==889198592
cpu=2*(state-4*512)+2048*4096*4
assert account['initial_expected_cpu_bytes']==cpu==603979776
assert account['initial_combined_payload_bytes']==weights+x+2*state+cpu < 1536*1024**2
assert account['timing_combined_payload_bytes']==weights+x+state+(2048*12288*2+512)+cpu
kernel=(ROOT/'candidate.metal').read_text();host=(ROOT/'probe.swift').read_text()
assert kernel.count('execution_simdgroup>')==1 and 'execution_simdgroups<' not in kernel
assert '32, 32, 128, false, true, false' in kernel
assert 'threadgroup_barrier(mem_flags::mem_none)' in kernel and 'result.load(residual)' in kernel
assert 'Data(bytes: $0.pointer, count: $0.bytes)' in host
assert 'initial workspaces remain live before timing' in host and 'memcmp(' in host
names=['baseline.metal','candidate.metal','production-shapes.metal','production-fixed-extract.metal','production-silu-extract.metal','production-planner.rs','probe.swift','weights.json','build.py','guard.py','manifest.json','summarize.py','source_check.py','README.md','half_ulp_check.py','half-ulp-fixtures.json','criterion-revision-v2.json']
result={'status':'source_checks_passed','mapping_shapes':len(shapes),'counts':expected,'source_sha256':{name:sha(ROOT/name) for name in names},'read_weight_payloads':False,'compiled':False,'gpu_executed':False}
(ROOT/'source-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='source_sha256'},indent=2))
