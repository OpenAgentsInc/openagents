#!/usr/bin/env python3
"""Check source identity, row mapping and allocation arithmetic without workloads."""
from pathlib import Path
import ast,hashlib,json,os
D=Path(__file__).resolve().parent
W=Path(os.environ['CLEF_REPO']).resolve()
m=json.loads((D/'manifest.json').read_text())
for entry in m['production_sources']:
 assert hashlib.sha256((W/entry['path']).read_bytes()).hexdigest()==entry['sha256']
prod=(W/'crates/psionic/crates/psionic-backend-metal/src/kernels/clef_prefill.metal').read_text()
assert (D/'production-fixed-extract.metal').read_text() in prod
silu=(D/'production-silu-extract.metal').read_text()
helper,kernel=silu.split('\n',1)
assert helper in prod and kernel in prod
assert (D/'production-shapes.metal').read_bytes()==(W/'crates/psionic/crates/psionic-backend-metal/src/kernels/clef_gemm_shapes.metal').read_bytes()
assert (D/'production-planner.rs').read_bytes()==(W/'crates/psionic/crates/psionic-backend-metal/src/clef_gemm.rs').read_bytes()
assert all((D/n).read_text() in (D/'baseline.metal').read_text() for n in ['production-fixed-extract.metal','production-silu-extract.metal','production-shapes.metal'])
count=0
for f in [1,7,31,32,33,63,64,65,127,128,129,255,256,257,12288]:
 p=((f+127)//128)*256
 forward=[(c//128)*256+c%128+u*128 for c in range(f) for u in [0,1]]
 assert len(set(forward))==2*f and min(forward)==0 and max(forward)<p
 for c in range(f):
  for u in [0,1]:
   i=(c//128)*256+c%128+u*128
   assert (i//256)*128+i%128==c and int(i%256>=128)==u
 for i in set(range(p))-set(forward):assert (i//256)*128+i%128>=f
 for n in [1,15,63,64,65,127,128,129,512,1024,2048]:
  grid=(p//256,(n+63)//64)
  assert grid[0]*256==p and (grid[1]-1)*64<n<=grid[1]*64
  count+=1
swift=(D/'probe.swift').read_text();metal=(D/'candidate.metal').read_text()
assert '64, 256, static_cast<int>(dynamic_extent)' in metal and 'execution_simdgroups<4>' in metal
assert 'out[gid] = half((g / (1.0f + exp(-g))) * paired[gate + 128]);' in metal
assert 'Data(bytes:$0.pointer,count:$0.bytes)' in swift
assert 'commands==(benchmark ? 292:46) && dispatches==(benchmark ? 602:161)' in swift
assert 'state.paired=paired' in swift and '2*w.f==packedColumns(w.f)' in swift
for p in D.glob('*.py'):ast.parse(p.read_text())
assert m['memory_accounting']['timing_combined_payload_bytes']<1536*1024**2
assert m['memory_accounting']['timing_explicit_bytes']<1024**3
assert 18*2+5*2+3*2+240==292
assert 18*7+5*7+3*7+5*3*(6+4+4+14)==602
result={'passed':True,'mapping_grid_cases':count,'packing_inverse_checked':True,'production_sources_unchanged':True,'python_ast_passed':True,'counts_checked':True,'memory_accounting':m['memory_accounting'],'compiled':False,'gpu_executed':False}
(D/'source-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
