"""Nothing stands in a doorway: no object lies on a door's wall within its
2 m opening. The house's portrait-1 hung in the entry hall's west arch."""
import json, sys
from pathlib import Path
doc = json.loads((Path(__file__).resolve().parents[1] / 'haunt/house.json').read_text())
spans = []
for door in doc['doors']:
    x, z = door['cell']
    side = door['side']
    if side in 'we':
        spans.append((door['id'], 0, x + (2 if side == 'e' else 0), z, z + 2))
    else:
        spans.append((door['id'], 1, z + (2 if side == 'n' else 0), x, x + 2))
bad = []
for o in doc['objects']:
    if o.get('scale', 1) > 20:  # the ground
        continue
    at = (o.get('x', 0), o.get('z', 0))
    for name, axis, line, a, b in spans:
        if abs(at[axis] - line) < 0.6 and a - 0.1 < at[1 - axis] < b + 0.1:
            bad.append(f"{o['id']} stands in {name}")
print('\n'.join(bad) or 'no object in a doorway')
sys.exit(1 if bad else 0)
