"""Offline content edit; never run by Xcode or shipped in the app.
Gives the Haunted House's lit windows warm glass: a flat arched pane
(Window_Wide_Round1_Pane) set into each glowing Window_Wide_Round1, inside
the wall where the frame and the wall hide its edge, carrying the glow the
frame had. The frame keeps no glow, so it reads as dark wood round the light.
Run once on the packed resources with the pinned protobuf tools venv:
haunt_panes.py RESOURCES_DIR (apps/openagents-ios/OpenAgents/Resources).
"""
import math, struct, subprocess, sys, tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
res = Path(sys.argv[1])
protoc = Path.home() / '.cache/openagents-engine-proto/protoc-29.3/bin/protoc'
tmp = tempfile.TemporaryDirectory(prefix='haunt-proto-')
subprocess.run([protoc, '-I' + str(ROOT / 'proto'), '--python_out=' + tmp.name, 'openagents/metareal/haunt/v1alpha1/haunt.proto'], check=True)
sys.path.insert(0, tmp.name)
from openagents.metareal.haunt.v1alpha1.haunt_pb2 import Resource
r = Resource(); r.ParseFromString((res / 'Haunt.pb').read_bytes())
FRAME, PANE = 'Window_Wide_Round1', 'Window_Wide_Round1_Pane'
if any(m.name == PANE for m in r.meshes): sys.exit(f'{PANE} is already there')
frame = next(i for i, m in enumerate(r.meshes) if m.name == FRAME)
# The pane: x ±HALF, from BOTTOM up to the arch's spring, then a half circle;
# its UVs span the grey glass strip at the foot of the frame sheet (layer 26).
HALF, BOTTOM, SPRING, Z, SEGMENTS = 0.62, 1.05, 2.05, 0.0, 12
layer = list(r.texture_layers).index(26)
outline = [(-HALF, BOTTOM), (HALF, BOTTOM), (HALF, SPRING)]
outline += [(HALF * math.cos(math.pi * k / SEGMENTS), SPRING + HALF * math.sin(math.pi * k / SEGMENTS)) for k in range(1, SEGMENTS)]
outline += [(-HALF, SPRING)]
centre = (0.0, (BOTTOM + SPRING) * 0.5)
def vertex(x, y):
  u, v = 0.1 + (x + HALF) / (2 * HALF) * 0.8, 0.82 + (y - BOTTOM) / (SPRING + HALF - BOTTOM) * 0.1
  return struct.pack('<8f4B', x, y, Z, 0, 0, 1, u, v, 255, 255, 255, layer) + bytes(12)
vertices = bytearray((res / 'Haunt.vertices').read_bytes()); indices = bytearray((res / 'Haunt.indices').read_bytes())
base = len(vertices) // 48; first = len(indices) // 4
vertices += vertex(*centre)
for x, y in outline: vertices += vertex(x, y)
tri = []
for k in range(len(outline)): tri += [base, base + 1 + k, base + 1 + (k + 1) % len(outline)]
indices += struct.pack('<%dI' % len(tri), *tri)
m = r.meshes.add(name=PANE, first_index=first, index_count=len(tri))
m.minimum.x, m.minimum.y, m.minimum.z = -HALF, BOTTOM, Z; m.maximum.x, m.maximum.y, m.maximum.z = HALF, SPRING + HALF, Z
pane = len(r.meshes) - 1
for e in list(r.entities):
  if e.mesh != frame or e.glow <= 0: continue
  p = r.entities.add(id=e.id + '/pane', name='Window glass', mesh=pane, yaw=e.yaw, scale=e.scale, glow=0.7, fixed=True, parent=e.id)
  p.position.CopyFrom(e.position); p.tint.x, p.tint.y, p.tint.z = 1.2, 0.68, 0.3
  e.glow = 0
r.vertex_count = len(vertices) // 48; r.index_count = len(indices) // 4
(res / 'Haunt.vertices').write_bytes(vertices); (res / 'Haunt.indices').write_bytes(indices)
(res / 'Haunt.pb').write_bytes(r.SerializeToString())
print(f'{PANE}: {len(tri) // 3} triangles; panes in', sum(1 for e in r.entities if e.mesh == pane), 'windows')
