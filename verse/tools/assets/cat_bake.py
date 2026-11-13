# Firm reason: 6 (Blender bpy and bmesh build the cat's surfaces and export the registry's GLB).
"""Build and bake Everglade's cat (everglade/creatures/cat.coax): a black and white tuxedo cat.

No CC0 animated cat with a sit exists, so this script builds one, an OpenAgents original (CC0), as a
stylised low-poly cat for the Metaverse's look. It is made of smooth closed surfaces placed as a cat's
anatomy: a round, wide head with a very short muzzle, a small pink nose, large green eyes and big
upright triangular ears; a short neck and a compact body; a long tail. The coat is a tuxedo: black
head, back, body and tail, with a white bib, muzzle, chin and paws, and pale whiskers.

The clips, a frame each pose:
  Idle  (EG_CAT_STAND frames) standing, the head looking round a little
  Walk  (EG_CAT_STEP frames) the diagonal gait, a full stride
  Sit   (EG_CAT_SITS frames) the sit: haunches on the ground, the front legs straight and close
        together, the tail curled round the front paws. The head is turned SIT_LOOK to the cat's left
        (the game turns the body so the head faces who it looks at, and the shot sits the camera
        there, so the body shows in three-quarter view). Over the clip it blinks twice, turns its
        head away and back, twitches an ear and flicks its tail's tip.

Each frame is Cat-e2-N.bin (N runs through the clips in order), the triangle record animal_bake.py
writes (<If count, seconds> then <6f4s> a corner: position x, z, -y, the normal likewise, sRGB
colour), in metres, the head toward +Z. It also writes Cat.glb (the sit's first frame, vertex
coloured), the interchange artifact the asset registry records (openagents/beasts/cat).

  Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \\
    --python verse/tools/assets/cat_bake.py -- OUTPUT_DIR [PREVIEW_DIR]
With PREVIEW_DIR it also renders stills of the sit and the stand there, for a look.
"""
import bpy, bmesh, math, struct, sys
from pathlib import Path
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index('--') + 1:]
output = Path(args[0]); output.mkdir(parents=True, exist_ok=True)
preview = Path(args[1]) if len(args) > 1 else None
STAND, STEP, SITS = 8, 8, 48                  # frames a clip (everglade/creatures/cat.coax)
STAND_S, STEP_S, SIT_S = 2.4, 0.8, 8.0        # seconds a loop
SIT_LOOK = 0.8                                 # rad the sitting cat's head turns to its left

BLACK = (0.022, 0.022, 0.026); WHITE = (0.80, 0.80, 0.77); NOSE = (0.60, 0.30, 0.32)
IRIS = (0.30, 0.50, 0.04); PUPIL = (0.004, 0.004, 0.004); INNER = (0.24, 0.12, 0.13); WHISKER = (0.75, 0.75, 0.72)

def srgb(v):
    return max(0, min(255, round(255 * (12.92*v if v <= .0031308 else 1.055*v**(1/2.4)-.055))))
def rgba(c): return bytes([*(srgb(v) for v in c), 255])

class Cat:
    def __init__(self): self.corners = []
    def emit(self, bm, m, colour, flat=False):
        bm.transform(m); bmesh.ops.triangulate(bm, faces=bm.faces[:]); bm.normal_update()
        c = rgba(colour)
        for f in bm.faces:
            for v in f.verts:
                self.corners.append((v.co.copy(), (f.normal if flat else v.normal).copy(), c))
        bm.free()
    def ellipsoid(self, at, radii, colour, m=Matrix.Identity(4), rot=Matrix.Identity(4), subdiv=2):
        bm = bmesh.new(); bmesh.ops.create_icosphere(bm, subdivisions=subdiv, radius=1.0)
        self.emit(bm, m @ Matrix.Translation(Vector(at)) @ rot @ Matrix.Diagonal((*radii, 1)), colour)
    def tube(self, points, r0, r1, colour, m=Matrix.Identity(4), sides=8):
        # a closed tube through the points, its radius r0 at the start to r1 at the end, capped
        pts = [m @ Vector(p) for p in points]; n = len(pts); bm = bmesh.new(); rings = []
        for i, p in enumerate(pts):
            d = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
            a = Vector((0, 0, 1)) if abs(d.z) < 0.9 else Vector((1, 0, 0))
            u = d.cross(a).normalized(); w = d.cross(u).normalized(); r = r0 + (r1 - r0) * i / (n - 1)
            rings.append([bm.verts.new(p + (u * math.cos(k * 2 * math.pi / sides) + w * math.sin(k * 2 * math.pi / sides)) * r) for k in range(sides)])
        for i in range(n - 1):
            for k in range(sides):
                bm.faces.new((rings[i][k], rings[i][(k + 1) % sides], rings[i + 1][(k + 1) % sides], rings[i + 1][k]))
        start = bm.verts.new(pts[0] - (pts[1] - pts[0]).normalized() * r0 * 0.6); end = bm.verts.new(pts[-1] + (pts[-1] - pts[-2]).normalized() * r1 * 0.8)
        for k in range(sides):
            bm.faces.new((rings[0][(k + 1) % sides], rings[0][k], start)); bm.faces.new((rings[-1][k], rings[-1][(k + 1) % sides], end))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
        self.emit(bm, Matrix.Identity(4), colour)
    def ear(self, base, tilt, scale, colour, m, ahead=0.0):
        # a triangular ear: a three-sided pyramid, one flat face forward, tipped out by tilt
        bm = bmesh.new()
        w, d, h = 0.027 * scale, 0.016 * scale, 0.052 * scale
        a = bm.verts.new((-w, -d * 0.35 + ahead, 0)); b = bm.verts.new((w, -d * 0.35 + ahead, 0)); c = bm.verts.new((0, d, 0)); t = bm.verts.new((0, ahead * 0.5, h))
        for f in ((a, b, t), (b, c, t), (c, a, t), (a, c, b)): bm.faces.new(f)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
        self.emit(bm, m @ Matrix.Translation(Vector(base)) @ Matrix.Rotation(tilt, 4, 'Y') @ Matrix.Rotation(-0.12, 4, 'X'), colour, flat=True)
    def whisker(self, root, tip, m):
        bm = bmesh.new(); r = m @ Vector(root); t = m @ Vector(tip); up = Vector((0, 0, 0.0007))
        q = [bm.verts.new(r - up), bm.verts.new(r + up), bm.verts.new(t + up * 0.5), bm.verts.new(t - up * 0.5)]
        bm.faces.new(q); bm.faces.new(tuple(reversed([bm.verts.new(v.co) for v in q])))
        self.emit(bm, Matrix.Identity(4), WHISKER, flat=True)

def head(cat, neck, yaw, pitch, blink, ear_twitch):
    # the head, in a frame at the neck: +X the cat's left... its right is -X, it looks along -Y
    m = Matrix.Translation(Vector(neck)) @ Matrix.Rotation(yaw, 4, 'Z') @ Matrix.Rotation(pitch, 4, 'X')
    cat.ellipsoid((0, 0, 0.040), (0.064, 0.056, 0.053), BLACK, m)            # the round skull
    cat.ellipsoid((0, -0.016, 0.022), (0.062, 0.043, 0.034), BLACK, m)       # the wide cheeks
    for s in (-1, 1):
        cat.ellipsoid((s * 0.0125, -0.053, 0.019), (0.016, 0.013, 0.012), WHITE, m, subdiv=1)   # muzzle
        lid = 0.12 if blink else 1.0
        cat.ellipsoid((s * 0.025, -0.046, 0.047), (0.0145, 0.0085, 0.0135 * lid), IRIS, m, subdiv=2)
        cat.ellipsoid((s * 0.025, -0.0535, 0.047), (0.004, 0.0025, 0.0095 * lid), PUPIL, m, subdiv=1)
        tw = ear_twitch if s < 0 else 0.0
        cat.ear((s * 0.037, -0.004, 0.072), s * (0.30 + tw), 1.0, BLACK, m)
        cat.ear((s * 0.037, -0.0085, 0.077), s * (0.30 + tw), 0.62, INNER, m)
        for k in (-1, 0, 1):
            cat.whisker((s * 0.022, -0.058, 0.019 + k * 0.002), (s * 0.080, -0.064 + abs(k) * 0.004, 0.024 + k * 0.009), m)
    cat.ellipsoid((0, -0.047, 0.007), (0.014, 0.011, 0.009), WHITE, m, subdiv=1)     # the chin
    cat.ellipsoid((0, -0.0605, 0.031), (0.0072, 0.005, 0.0048), NOSE, m, subdiv=1)  # the nose

def smooth(t): t = max(0.0, min(1.0, t)); return t * t * (3 - 2 * t)

def sit(t, f):
    cat = Cat()
    cat.ellipsoid((0, 0.048, 0.072), (0.066, 0.068, 0.072), BLACK)          # the rump on the ground
    for s in (-1, 1):
        cat.ellipsoid((s * 0.047, 0.028, 0.056), (0.042, 0.074, 0.056), BLACK)   # the haunches
        cat.ellipsoid((s * 0.055, -0.022, 0.010), (0.016, 0.030, 0.011), WHITE, subdiv=1)   # hind paws
    tilt = Matrix.Rotation(-0.28, 4, 'X')
    cat.ellipsoid((0, -0.004, 0.148), (0.057, 0.054, 0.100), BLACK, rot=tilt)     # the upright chest
    cat.ellipsoid((0, -0.043, 0.162), (0.041, 0.028, 0.074), WHITE, rot=tilt)     # the white bib
    for s in (-1, 1):
        cat.tube([(s * 0.021, -0.040, 0.125), (s * 0.020, -0.050, 0.060)], 0.0135, 0.0125, BLACK)
        cat.tube([(s * 0.020, -0.049, 0.064), (s * 0.019, -0.056, 0.018)], 0.0125, 0.012, WHITE)
        cat.ellipsoid((s * 0.019, -0.065, 0.010), (0.0155, 0.022, 0.011), WHITE, subdiv=1)  # front paws
    # the tail: out behind, round the cat's left side and over the front paws; its tip flicks
    flick = 0.010 * math.sin(2 * math.pi * t * 3)
    path = [(0, 0.098, 0.04), (0.04, 0.122, 0.02), (0.084, 0.094, 0.0145), (0.104, 0.030, 0.013),
            (0.092, -0.038, 0.013), (0.058, -0.080, 0.013 + flick * 0.3), (0.018 + flick, -0.094, 0.016 + abs(flick))]
    pts = []
    for i in range(len(path) - 1):
        a, b = Vector(path[i]), Vector(path[i + 1])
        for k in range(3): pts.append(a.lerp(b, k / 3))
    pts.append(Vector(path[-1]))
    cat.tube(pts, 0.0125, 0.0085, BLACK)
    # the head: SIT_LOOK to its left, turning away a little and back in the middle of the clip
    turn = 0.32 * (smooth((t - 0.30) / 0.12) - smooth((t - 0.58) / 0.12))
    nod = 0.05 * math.sin(2 * math.pi * t * 2)
    blink = f in (9, 10, 37)
    twitch = 0.35 if f in (23, 24) else 0.0
    head(cat, (0, -0.032, 0.212), SIT_LOOK + turn, nod, blink, twitch)
    return cat

def standing(t, walking):
    cat = Cat(); bob = 0.004 * math.sin(4 * math.pi * t) if walking else 0.0
    cat.ellipsoid((0, 0.020, 0.150 + bob), (0.054, 0.122, 0.058), BLACK)     # the body
    cat.ellipsoid((0, 0.075, 0.150 + bob), (0.056, 0.060, 0.060), BLACK)     # the hips
    cat.ellipsoid((0, -0.078, 0.128 + bob), (0.040, 0.038, 0.050), WHITE)    # the chest
    for front, y in ((True, -0.072), (False, 0.098)):
        for s in (-1, 1):
            phase = (0.0 if (s > 0) == front else math.pi)
            a = 0.42 * math.sin(2 * math.pi * t + phase) if walking else 0.0
            hip = Vector((s * 0.030, y, 0.128 + bob)); L = 0.118
            down = Vector((0, L * math.sin(a), -L * math.cos(a)))
            knee = hip + down * 0.55; foot = hip + down
            cat.tube([hip, knee], 0.0145, 0.0125, BLACK)
            cat.tube([knee, foot], 0.0125, 0.012, WHITE if front else BLACK)
            cat.ellipsoid((foot.x, foot.y - 0.008, max(foot.z - 0.002, 0.010)), (0.015, 0.021, 0.0105), WHITE, subdiv=1)
    sway = 0.03 * math.sin(2 * math.pi * t)
    tail = [(0, 0.128, 0.165), (sway * 0.3, 0.160, 0.195), (sway * 0.7, 0.180, 0.240), (sway, 0.182, 0.285), (sway * 1.4, 0.165, 0.315)]
    cat.tube([Vector(p) for p in tail], 0.012, 0.009, BLACK)
    look = 0.25 * math.sin(2 * math.pi * t) if not walking else 0.0
    head(cat, (0, -0.110, 0.168 + bob), look, -0.05, False, 0.0)
    return cat

def write(cat, index, seconds):
    data = bytearray()
    for p, n, c in cat.corners:
        data.extend(struct.pack('<6f4s', p.x, p.z, -p.y, n.x, n.z, -n.y, c))
    (output / f'Cat-e2-{index}.bin').write_bytes(struct.pack('<If', len(cat.corners), seconds) + data)
    return len(cat.corners)

index = 0
for f in range(STAND): write(standing(f / STAND, False), index, STAND_S); index += 1
for f in range(STEP): write(standing(f / STEP, True), index, STEP_S); index += 1
for f in range(SITS): n = write(sit(f / SITS, f), index, SIT_S); index += 1
print(f'Cat: {index} frames, the sit {n // 3} triangles', flush=True)

def mesh_of(cat, name):
    me = bpy.data.meshes.new(name); verts = [tuple(p) for p, _, _ in cat.corners]
    me.from_pydata(verts, [], [(i, i + 1, i + 2) for i in range(0, len(verts), 3)])
    col = me.color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
    for i, (_, _, c) in enumerate(cat.corners): col.data[i].color_srgb = (c[0] / 255, c[1] / 255, c[2] / 255, 1)
    me.update(); ob = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(ob)
    for poly in me.polygons: poly.use_smooth = True
    return ob

bpy.ops.wm.read_factory_settings(use_empty=True)
sc = bpy.context.scene
ob = mesh_of(sit(0, 0), 'Cat')
mat = bpy.data.materials.new('Cat'); mat.use_nodes = True
nodes = mat.node_tree.nodes; attr = nodes.new('ShaderNodeVertexColor'); attr.layer_name = 'Col'
mat.node_tree.links.new(attr.outputs['Color'], nodes['Principled BSDF'].inputs['Base Color']); ob.data.materials.append(mat)
bpy.ops.export_scene.gltf(filepath=str(output / 'Cat.glb'), export_format='GLB', export_animations=False)
if preview:
    preview.mkdir(parents=True, exist_ok=True)
    cam = bpy.data.objects.new('cam', bpy.data.cameras.new('cam')); sc.collection.objects.link(cam); sc.camera = cam
    sc.render.engine = 'BLENDER_WORKBENCH'; sc.display.shading.light = 'STUDIO'; sc.display.shading.color_type = 'VERTEX'
    sc.render.resolution_x = 640; sc.render.resolution_y = 640
    shots = [('sit-three-quarter', sit(0, 0), SIT_LOOK, 0.16), ('sit-side', sit(0, 0), 1.5708, 0.14), ('sit-blink', sit(0.2, 9), SIT_LOOK, 0.16), ('stand', standing(0, False), 0.7, 0.14), ('walk', standing(0.25, True), 1.5708, 0.14)]
    for name, cat, yaw, aim in shots:
        for o in list(sc.objects):
            if o.type == 'MESH': bpy.data.objects.remove(o)
        mesh_of(cat, name); c = Vector((0, -0.01, aim))
        cam.location = c + Vector((math.sin(yaw) * 0.75, -math.cos(yaw) * 0.75, 0.05))
        cam.rotation_euler = (c - cam.location).to_track_quat('-Z', 'Y').to_euler()
        sc.render.filepath = str(preview / f'cat-{name}.png'); bpy.ops.render.render(write_still=True)
