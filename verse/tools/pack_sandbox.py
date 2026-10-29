"""The Sandbox realm's world document (rl-sandbox), offline.

Open ground to build on, after the demo's planetWorld('sandbox')
(aethyr-demo web/haunt-world.js): flat pale ground, a ring of eight lamps
round the starting plot, a builder's yard with crates, a barrel, a
workbench and a bench to move, paint or copy, rocks far out, and the
Builder. It is a Haunted House resource with the house's own meshes, so it
shares Haunt.vertices, Haunt.indices and the house's texture layers.

  pack_sandbox.py HAUNT.pb OUT.pb
"""
import math, subprocess, sys, tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
haunt, out = map(Path, sys.argv[1:])
protoc = Path.home() / '.cache/openagents-engine-proto/protoc-29.3/bin/protoc'
tmp = tempfile.TemporaryDirectory(prefix='sandbox-proto-')
subprocess.run([protoc, '-I' + str(ROOT / 'proto'), '--python_out=' + tmp.name, 'openagents/metareal/haunt/v1alpha1/haunt.proto'], check=True)
sys.path.insert(0, tmp.name)
from openagents.metareal.haunt.v1alpha1.haunt_pb2 import Resource

house = Resource(); house.ParseFromString(haunt.read_bytes())
r = Resource(version=1, world_id='sandbox', title='Sandbox', source_commit='sandbox-1',
             vertices_resource=house.vertices_resource, indices_resource=house.indices_resource,
             vertex_count=house.vertex_count, index_count=house.index_count)
r.meshes.extend(house.meshes); r.texture_layers.extend(house.texture_layers)
mesh = {m.name: i for i, m in enumerate(r.meshes)}

def vec(to, x, y, z): to.x, to.y, to.z = x, y, z
def put(id, name, m, x, z, y=0.0, yaw=0.0, scale=1.0, tint=(1, 1, 1), glow=0.0, fixed=False, solid=False, motion=0, parent=''):
  e = r.entities.add(id=id, name=name, mesh=mesh[m], yaw=yaw, scale=scale, glow=glow, fixed=fixed, solid=solid, motion=motion, parent=parent)
  vec(e.position, x, y, z); vec(e.tint, *tint)
  return e

# A seeded generator (the demo's), so the rocks are the same every build.
def rng(seed):
  s = [seed & 0xffffffff or 1]
  def nxt():
    s[0] = (s[0] * 1664525 + 1013904223) & 0xffffffff
    return s[0] / 4294967296
  return nxt
rand = rng(20261029)

put('ground', 'Ground', 'Custom_Ground', 0, 4, y=-0.03, scale=120, tint=(0.42, 0.44, 0.4), fixed=True)
# The ring of lamps round the starting plot.
for k in range(8):
  a = k / 8 * math.pi * 2; x, z = math.cos(a) * 14, 4 + math.sin(a) * 14
  put(f'lamp-{k}', 'Lamp', 'CandleStick_Stand', x, z)
  put(f'lamp-{k}/flame-0', '', 'Custom_Flame', x, z, y=1.3, scale=1.4, glow=0.5, fixed=True, motion=4, parent=f'lamp-{k}')
  l = r.lights.add(radius=7, flicker=0.2, intensity=1, parent=f'lamp-{k}'); vec(l.position, x, 1.6, z); vec(l.color, 1, 0.66, 0.34)
# The builder's yard.
deg = math.pi / 180
put('yard-crate-1', 'Crate', 'Crate_Wooden', 5, 8, yaw=10 * deg, solid=True)
put('yard-crate-2', 'Crate', 'Crate_Wooden', 5.2, 9.3, yaw=-8 * deg, solid=True)
put('yard-crate-3', 'Crate', 'Crate_Wooden', 5.1, 8.6, y=0.93, yaw=25 * deg, solid=True)
put('yard-barrel', 'Barrel', 'Barrel', 6.6, 8.4, solid=True)
put('yard-workbench', 'Workbench', 'Workbench', -6, 9, yaw=70 * deg, solid=True)
put('yard-bench', 'Bench', 'Bench', -4.5, 6, yaw=90 * deg, solid=True)
# Rocks far out, clear of the plot and of each other.
rocks = []
while len(rocks) < 10:
  a = rand() * math.pi * 2; d = 6 + math.sqrt(rand()) * 40; x, z = math.cos(a) * d, math.sin(a) * d
  if math.hypot(x, z - 4) <= 20 or any(math.hypot(x - p, z - q) < 3 for p, q in rocks): continue
  rocks.append((x, z))
  put(f'rock-{len(rocks) - 1}', 'Rock', 'Custom_Rock', x, z, yaw=rand() * math.pi * 2, scale=0.8 + rand() * 1.4, solid=True)
# The Builder, who says what the place is for.
put('builder', 'Builder', 'Custom_Fig_Caretaker', 1.5, 4, yaw=math.pi, fixed=True, solid=True)
p = r.people.add(id='builder', name='Builder', role='builder', range=3.2)
vec(p.position, 1.5, 0, 4)
p.greet.extend(['All of this is open ground. Press B and build.', 'Everyone shares this Sandbox: what you build, everyone sees.'])
p.hint.extend(['Press B to edit: pick a thing to move, paint or copy it, or place one from the bar.', 'Your changes go out to everyone when you press DONE.'])
p.answer.extend(['It is everyone\'s ground. I just keep the lamps lit.'])
p.emote.extend(['*waves*'])
r.areas.add(id='sandbox', name='Sandbox', surface=2).rect.CopyFrom(Resource().walk.add(x0=-50, z0=-46, x1=50, z1=54))
r.walk.add(x0=-50, z0=-46, x1=50, z1=54)
vec(r.arrival, 0, 0, -2); r.arrival_yaw = 0
out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(r.SerializeToString())
print(f'{out}: {len(r.entities)} entities, {len(r.lights)} lights, {out.stat().st_size} bytes')
