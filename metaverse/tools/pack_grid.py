"""The Grid realm's assets (rl-grid), offline, from OpenAgents' own Grid
scene: assets/verse/grid/lander3-scene.json at 437ff8ef72 (the scene the
demo's tools/import_grid.py imported; sha256 checked below).

  git show 437ff8ef72d637ca198dd47404c34ece64a303f7:assets/verse/grid/lander3-scene.json > scene.json
  pack_grid.py scene.json apps/openagents-web/realms/the-grid/assets

Everything is the art's flat colours, unlit, written as the display values
the demo drew (its linear tints through the sRGB curve). Four files:

- grid.vertices (storage): the placed art's faces in world space, the
  walk-up stations' boxes, then the robot in its bind pose; 6 words a
  vertex: x, y, z, colour (RGBA8), joints (4 x u8), weights (4 x u8).
- grid.lines (storage): every line of the art and the stations' edges, in
  world space; 7 words a segment: ax, ay, az, bx, by, bz, colour. Then the
  robot's outline, skinned like its faces: its two ends are robot vertices
  (ax, ay as u32 vertex numbers) and its colour's alpha is 0.
- grid.bones (storage): the robot's clips (walk, idle, wave), each frame's
  67 skinning matrices as three rows of four floats.
- grid.indices (index): the faces' vertices in order, then each art line's
  quad (six corners; an index with its top bit set is a line's corner,
  the rest its number), then the robot's triangles, then its outline's quads.
- grid.data (data): little-endian; "AGRD", then u32 faces, art lines,
  robot lines, robot index count, robot vertex base, bones, clips; per clip u32 first frame,
  u32 frames, f32 duration; f32 robot scale, f32 sample rate; u32 patrols;
  per patrol u32 samples then f32 x, z, yaw, clip, clip time each.
"""
import base64, hashlib, json, struct, sys
from pathlib import Path

SCENE_SHA = '765c5ee05cc9560b8022a2e3082f9c2a0c14b96b27c66fcd7db8c2936ed7bb47'
source, out = Path(sys.argv[1]), Path(sys.argv[2])
raw = source.read_bytes()
if hashlib.sha256(raw).hexdigest() != SCENE_SHA: sys.exit('pack_grid: not the pinned Grid scene')
scene = json.loads(raw)

def arr(encoded, kind): b = base64.b64decode(encoded or ''); return list(struct.unpack('<%d%s' % (len(b) // struct.calcsize(kind), kind), b))
def display(linear):
  c = min(max(linear, 0.0), 1.0)
  s = c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055
  return round(s * 255)
def colour(tint, a=255): return display(tint[0]) | display(tint[1]) << 8 | display(tint[2]) << 16 | a << 24
def xf(m, p): return (m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14])

vertices = bytearray(); segments = bytearray(); faces = 0; lines = 0
def vertex(p, c, joints=(0, 0, 0, 0), weights=(255, 0, 0, 0)):
  global vertices
  vertices += struct.pack('<3fI4B4B', *p, c, *joints, *weights)
def segment(a, b, c):
  global segments, lines
  segments += struct.pack('<6fI', *a, *b, c); lines += 1

# The placed art (the floor's lines too: the Grid is its floor) and the gates; not the Game Lab
# or the Lagrange arch, which lead nowhere (owner, 2026-10-30: nothing there that doesn't work).
LEFT_OUT = {'grid/game-lab', 'grid/arch-lagrange-2'}
for placement in scene['statics'] + scene['gates']:
  if placement['model'] in LEFT_OUT: continue
  m = placement['matrix']
  for s in scene['models'][placement['model']]['surfaces']:
    P = arr(s['positions'], 'f'); idx = arr(s['indices'], 'I'); c = colour(s['tint'])
    pts = [xf(m, P[i:i + 3]) for i in range(0, len(P), 3)]
    if s['lines']:
      for k in range(0, len(idx), 2): segment(pts[idx[k]], pts[idx[k + 1]], c)
    else:
      for k in idx: vertex(pts[k], c); faces += 1
# The walk-up stations (grid_world.cpp addBox): dark faces, light edges.
DARK, EDGE = colour((0.008,) * 3), colour((0.35,) * 3)
QUADS = [(0, 2, 3, 1), (4, 5, 7, 6), (0, 1, 5, 4), (2, 6, 7, 3), (0, 4, 6, 2), (1, 3, 7, 5)]
STATIONS = [(0, 0, 4, 2.8, 1.0, 1.0), (0, 1, 4, 2.4, 1.4, .2), (-6, 0, -5, 2.6, 1.1, 1.0), (-6, 1.1, -5, .9, .65, .1),
            (12, 0, -10, .16, .95, .16), (12, .95, -10, .68, .55, .6), (30, 0, -7, .4, 5, .4), (30, 5, -7, 20, 8, .4)]
for x, y, z, w, h, d in STATIONS:
  corner = [(x + (w if k & 1 else -w) / 2, y + (h if k & 2 else 0), z + (d if k & 4 else -d) / 2) for k in range(8)]
  for q in QUADS:
    for k in (q[0], q[1], q[2], q[0], q[2], q[3]): vertex(corner[k], DARK); faces += 1
  for a in range(8):
    for bit in (1, 2, 4):
      if not a & bit: segment(corner[a], corner[a | bit], EDGE)

# The robot (the near model; the far one is the same rig with fewer faces).
robots = scene['robots']; model = scene['models'][robots['near']]
robot_base = len(vertices) // 24; robot_indices = []; art_lines = lines
for s in model['surfaces']:
  P = arr(s['positions'], 'f'); J = arr(s['joints'], 'H'); W = arr(s['weights'], 'f'); c = colour(s['tint'])
  base = len(vertices) // 24 - robot_base
  for i in range(len(P) // 3):
    w = W[i * 4:i * 4 + 4]; total = sum(w) or 1
    q = [round(x / total * 255) for x in w]; q[0] += 255 - sum(q)
    vertex(P[i * 3:i * 3 + 3], c, tuple(min(j, 255) for j in J[i * 4:i * 4 + 4]), tuple(q))
  idx = arr(s['indices'], 'I')
  if s['lines']:
    for k in range(0, len(idx), 2):
      segments += struct.pack('<2I4fI', robot_base + base + idx[k], robot_base + base + idx[k + 1], 0, 0, 0, 0, c & 0xffffff); lines += 1
  else: robot_indices += [base + k for k in idx]
bones = model['bones']
clips = []; frames = bytearray(); frame_at = 0
for pose in robots['poses'][robots['near']]:
  F = arr(pose['frames'], 'f'); n = len(F) // (bones * 16)
  for f in range(n):
    for j in range(bones):
      x = F[(f * bones + j) * 16:(f * bones + j) * 16 + 16]
      for r in range(3): frames += struct.pack('<4f', x[r], x[4 + r], x[8 + r], x[12 + r])
  clips.append((frame_at, n, pose['duration'])); frame_at += n

LINE = 0x80000000
indices = (list(range(faces)) + [LINE | k for k in range(6 * art_lines)] + [robot_base + k for k in robot_indices]
           + [LINE | k for k in range(6 * art_lines, 6 * lines)])
data = bytearray(b'AGRD') + struct.pack('<7I', faces, art_lines, lines - art_lines, len(robot_indices), robot_base, bones, len(clips))
for first, n, d in clips: data += struct.pack('<IIf', first, n, d)
data += struct.pack('<ff', robots['scale'], robots['rate']) + struct.pack('<I', len(robots['patrols']))
for patrol in robots['patrols']:
  S = arr(patrol['samples'], 'f'); data += struct.pack('<I', len(S) // 5) + struct.pack('<%df' % len(S), *S)

out.mkdir(parents=True, exist_ok=True)
(out / 'grid.vertices').write_bytes(vertices); (out / 'grid.lines').write_bytes(segments)
(out / 'grid.bones').write_bytes(frames); (out / 'grid.indices').write_bytes(struct.pack('<%dI' % len(indices), *indices))
(out / 'grid.data').write_bytes(data)
total = sum((out / f).stat().st_size for f in ('grid.vertices', 'grid.lines', 'grid.bones', 'grid.indices', 'grid.data'))
print(f'{faces} face vertices, {art_lines} art lines, {lines - art_lines} robot lines, robot {len(robot_indices)} indices, {len(clips)} clips {[c[1] for c in clips]}, {total} bytes')
