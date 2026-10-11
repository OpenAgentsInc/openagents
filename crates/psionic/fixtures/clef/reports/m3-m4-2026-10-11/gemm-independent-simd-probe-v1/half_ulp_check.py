#!/usr/bin/env python3
"""Check finite half ordering and frozen boundary cases without Metal or models."""
import json
import math
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parent

def value(bits):return struct.unpack('<e',struct.pack('<H',bits))[0]
def ordered(bits):
    magnitude=bits&0x7fff
    return 0x8000+magnitude if bits&0x8000==0 else 0x8000-magnitude
def distance(a,b):return abs(ordered(a)-ordered(b))
def allowed(a,b):
    x,y=value(a),value(b)
    return math.isfinite(x) and math.isfinite(y) and (abs(x-y)<=.001 or distance(a,b)<=1)

# Sort independently decoded IEEE half values, then compare their actual rank.
finite=sorted((value(bits),bits) for bits in range(65536) if math.isfinite(value(bits)))
rank=-1;previous=None;minimum=min(ordered(bits) for _,bits in finite)
for val,bits in finite:
    if previous is None or val!=previous:rank+=1
    assert ordered(bits)-minimum==rank
    previous=val
assert len(finite)==63488 and rank+1==63487
pairs=[(0,0),(0,0x8000),(1,0x8001),(0,1),(0,0x03ff),
       (0x4000,0x4001),(0x4000,0x4002),(0xc000,0xc001),(0xc000,0xc002),
       (0x3fff,0x4000),(0x7bfe,0x7bff),(0x7bfd,0x7bff),(0xfbfe,0xfbff),
       (0x7c00,0x7c00),(0x7e00,0x7e00),(0xfc00,0x7c00),(0,0x1400),(0,0x1401),(0,0x1800)]
expected=[True,True,True,True,True,True,False,True,False,True,True,False,True,False,False,False,True,True,False]
assert len(pairs)==len(expected)==19
cases=[]
for (a,b),want in zip(pairs,expected):
    assert allowed(a,b)==want and allowed(b,a)==want
    cases.append({'a_bits':a,'b_bits':b,'ulp_distance':distance(a,b),'allowed':want})
source=(ROOT/'probe.swift').read_text()
assert 'return (bits & 0x8000) == 0 ? 0x8000 + magnitude : 0x8000 - magnitude' in source
assert 'abs(Double(a) - Double(b)) <= 0.001 || halfULP(a.bitPattern, b.bitPattern) <= 1' in source
fixture={'criterion_version':2,'cases':cases}
path=ROOT/'half-ulp-fixtures.json'
if path.exists():assert json.loads(path.read_text())==fixture
else:path.write_text(json.dumps(fixture,indent=2)+'\n')
result={'status':'passed','finite_bit_patterns':len(finite),'distinct_finite_values':rank+1,'boundary_cases':len(cases),'includes_nonfinite_refusals':3,'criterion_version':2,'compiled':False,'gpu_executed':False}
(ROOT/'half-ulp-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
