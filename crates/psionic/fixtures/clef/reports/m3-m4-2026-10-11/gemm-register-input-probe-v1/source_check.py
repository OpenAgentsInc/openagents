#!/usr/bin/env python3
"""Check bounded source metadata without compiling or reading weight payloads."""
import ast
import os
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
ROOT = Path(__file__).resolve().parent
REPO = Path(os.environ['CLEF_REPO']).resolve()
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
manifest = json.loads((ROOT/'manifest.json').read_text())
for path in ROOT.glob('*.py'): ast.parse(path.read_text(), filename=str(path))
for entry in manifest['production_sources']:
    assert sha(REPO/entry['path']) == entry['sha256'], entry['path']
assert sha(ROOT/'weights.json') == manifest['weights_manifest_sha256']
for name in ['baseline.metal','production-shapes.metal','production-fixed-extract.metal','production-silu-extract.metal','production-planner.rs','weights.json']:
    assert (ROOT/name).read_bytes() == (ROOT.parent/'gemm-independent-simd-probe'/name).read_bytes(), name
cases = json.loads((ROOT/'small-cases.json').read_text())
assert len(cases['cases']) == 12 and cases['projection_checks'] == 24
assert cases['accumulate_modes'] == [False, True]
assert [v[2] for v in cases['cases']] == [1,15,16,17,31,32,33,127,128,129,4096,12288]
kernel=(ROOT/'candidate.metal').read_text();host=(ROOT/'probe.swift').read_text()
line=next(x for x in host.splitlines() if x.startswith('let smallCases:'))
actual=[(int(n),int(m),int(k),f) for n,m,k,f in re.findall(r'\((\d+), (\d+), (\d+), "([a-z]+)"\)',line)]
assert actual == [tuple(x) for x in cases['cases']]
shapes=[tuple(c[:3]) for c in cases['cases']]+[(31,31,129),(32,32,128),(33,33,129),(64,64,256),(65,65,257)]
for n,m,k in shapes:
    owned=Counter()
    for gy in range((n+63)//64):
        for gx in range((m+63)//64):
            for sg in range(4):
                row,col=gy*64+(sg//2)*32,gx*64+(sg%2)*32
                if row>=n or col>=m: continue
                for half in range(2):
                    for ry in range(16):
                        for cx in range(32):
                            y,x=row+half*16+ry,col+cx
                            if y<n and x<m: owned[y,x]+=1
                visited=[]
                for block in range(0,k,128):
                    for k0 in range(block,min(block+128,k),32):
                        for half in range(2):
                            for ki in range(16):
                                kk=k0+half*16+ki
                                if kk<k: visited.append(kk)
                assert visited == list(range(k))
    assert len(owned)==n*m and set(owned.values())=={1}
    # Exact helper-level coordinate bounds and unique fragment coverage.
    row=0 if k==128 else (n-1)//32*32
    col=0 if k==128 else (m-1)//32*32
    k0=0 if k==128 else (k-1)//32*32
    logical=0
    for obj in range(8):
        width,height=(16,16) if obj<4 else ((16,32) if obj<6 else (32,16))
        for cy in range(height):
            for cx in range(width):
                if obj<4:
                    rr,kk=row+(obj//2)*16+cy,k0+(obj%2)*16+cx
                    if rr<n and kk<k: assert 0<=rr*k+kk<n*k
                elif obj<6:
                    cc,kk=col+cy,k0+(obj-4)*16+cx
                    if cc<m and kk<k: assert 0<=cc*k+kk<m*k
                else:
                    rr,cc=row+(obj-6)*16+cy,col+cx
                    if rr<n and cc<m: assert 0<=rr*m+cc<n*m
                logical+=1
    assert logical==3072
expected=manifest['expected']
assert expected['check_only_commands']==12+24*2+5*2==70
assert expected['check_only_dispatches']==12+24*2+5*8==100
assert expected['total_commands']==70+3*3+240==319
assert expected['total_dispatches']==100+3*9+5*4*3*(1+1+1+4)==547
account=manifest['memory_accounting'];state=2048*(12288*10+4096*4)+4*512
weights=3*(4096*12288*2+512);x=2048*4096*2+512;common=2048*12288*2+512
cpu=2*(state-4*512)+2048*4096*4
assert account['initial_explicit_buffers']==weights+x+state+common==654316032
assert account['initial_expected_cpu_bytes']==cpu==603979776
assert account['initial_combined_payload_bytes']==weights+x+state+common+cpu==1258295808
assert account['timing_combined_payload_bytes']==account['initial_combined_payload_bytes']<1536*1024**2
assert 8*32*(1+64*4)*4 == 263168
assert kernel.count('execution_simdgroup>')==1 and 'execution_simdgroups<' not in kernel
assert '16, 32, 16, false, true, false' in kernel
assert 'threadgroup_barrier(mem_flags::mem_none)' in kernel
assert 'get_multidimensional_index' in kernel and '.load(tile)' in kernel
assert all(x in kernel for x in ['op.run(a00, b0, c0);','op.run(a10, b0, c1);','op.run(a01, b1, c0);','op.run(a11, b1, c1);'])
assert 'Data(bytes: $0.pointer, count: $0.bytes)' in host
assert 'baseline workspace remains live' in host and 'initial workspaces remain live before timing' in host
assert 'memcmp(' in host and 'seen.insert(cy * width + cx).inserted' in host
names=['baseline.metal','candidate.metal','production-shapes.metal','production-fixed-extract.metal','production-silu-extract.metal','production-planner.rs','probe.swift','weights.json','build.py','guard.py','manifest.json','summarize.py','source_check.py','README.md','half_ulp_check.py','half-ulp-fixtures.json','small-cases.json']
for name in ['build.py','guard.py']:
    tree=ast.parse((ROOT/name).read_text())
    lists=[ast.literal_eval(node) for node in ast.walk(tree) if isinstance(node,ast.List) and len(node.elts)==len(names) and all(isinstance(x,ast.Constant) and isinstance(x.value,str) for x in node.elts)]
    assert lists==[names], (name,lists)
result={'status':'source_checks_passed','mapping_shapes':len(shapes),'counts':expected,'source_sha256':{name:sha(ROOT/name) for name in names},'read_weight_payloads':False,'compiled':False,'gpu_executed':False}
(ROOT/'source-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='source_sha256'},indent=2))
