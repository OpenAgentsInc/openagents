#!/usr/bin/env python3
"""Check captured layer-zero outputs; no checkpoint or third-party runtime needed."""
import hashlib,math,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]/'psionic/fixtures/models/elephant'
FILES={'layer0-token32-psionic.f32':'bb24840a66cdec253c9626c565ddbbc4c46049aea5c9891f889109125f5c26d5','layer0-token32-transformers.f32':'96355941a92956255f1ad3aabf9a29500a88b6ee173aee55d50eaa74052790be'}
arrays=[]
for name,expected in FILES.items():
 raw=(ROOT/name).read_bytes()
 if len(raw)!=4096*4 or hashlib.sha256(raw).hexdigest()!=expected:raise ValueError('fixture identity')
 values=struct.unpack('<4096f',raw)
 if not all(math.isfinite(x) for x in values):raise ValueError('nonfinite output')
 arrays.append(values)
errors=[abs(a-b) for a,b in zip(*arrays)]
if max(errors)>0.000003:raise ValueError('reference tolerance')
print(f'Elephant layer-zero captured reference: max absolute difference {max(errors):.9g}')
