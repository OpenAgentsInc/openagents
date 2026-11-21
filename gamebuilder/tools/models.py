# Firm reason: 6 (Blender's own Python API builds and renders the models; it has no Coax binding).
# Builds Scrappers' models and portraits.
#
#   /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/models.py
#
# Writes host/Models/scrappers.gbm (every mesh the match draws) and the three
# Scrappers' portraits into host/Assets.xcassets. Both are committed: run this
# again only to change the art.
#
# The Scrappers are GameBuilder's own designs. Each is built from parts (body,
# head, arm, leg, blaster) so the game can move them: a part's origin is its
# joint. Shapes are made here from rounded boxes, spheres, capsules, cones and
# rings; nothing is imported.
#
# The file (little-endian), read by gamebuilder/core/meshes.coax:
#   "GBM1", u32 mesh count
#   a mesh: 16 bytes of name (NUL padded), u32 first vertex, u32 vertex count, 8 f32 numbers
#   then the vertices, 7 words each: x y z, nx ny nz (f32), r g b a (u8)
# Metres, y up, +z the way a Scrapper faces. A vertex's alpha says how it is
# coloured: 255 its own colour; 0 its team's colour; 64 the instance's colour
# (a crate, a wall); 128 its own colour, glowing; 192 the instance's colour, glowing.
# A Scrapper's body carries its numbers: the hip's height and half spacing, the
# shoulder's height and half spacing, the neck's height, the arm's length.
import bpy, bmesh, math, struct, os
from mathutils import Matrix, Vector

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OWN, TEAM, INSTANCE, GLOW, GLOW_INSTANCE = 255, 0, 64, 128, 192

# ---- shapes: each returns triangles as (position, normal) in Blender's axes (x right, -y front, z up)

def tris_of(bm, smooth, matrix):
    bmesh.ops.transform(bm, matrix=matrix, verts=bm.verts)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.normal_update()
    out = []
    for f in bm.faces:
        for l in f.loops:
            n = l.vert.normal if smooth else f.normal
            out.append((tuple(l.vert.co), tuple(n)))
    bm.free()
    return out

def at(x=0, y=0, z=0, rx=0, ry=0, rz=0, sx=1, sy=1, sz=1):
    return Matrix.Translation((x, y, z)) @ Matrix.Rotation(rz, 4, 'Z') @ Matrix.Rotation(ry, 4, 'Y') @ Matrix.Rotation(rx, 4, 'X') @ Matrix.Diagonal((sx, sy, sz, 1))

def box(w, d, h, r, m):
    """A box w wide, d deep, h tall, its edges rounded by r, centred."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1)
    bmesh.ops.scale(bm, vec=(w, d, h), verts=bm.verts)
    if r > 0:
        bmesh.ops.bevel(bm, geom=bm.edges[:], offset=r, segments=4, profile=0.5, affect='EDGES')
    return tris_of(bm, True, m)

def ball(r, m, sx=1, sy=1, sz=1):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=20, v_segments=12, radius=r)
    bmesh.ops.scale(bm, vec=(sx, sy, sz), verts=bm.verts)
    return tris_of(bm, True, m)

def tube(r1, r2, h, m, segments=18):
    """A cylinder or cone standing on z = 0, r1 at the bottom and r2 at the top."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segments, radius1=r1, radius2=max(r2, 0.0001), depth=h)
    bmesh.ops.translate(bm, vec=(0, 0, h / 2), verts=bm.verts)
    return tris_of(bm, True, m)

def capsule(r, h, m):
    """A capsule hanging from z = 0 down to z = -h."""
    return tube(r, r, h - 2 * r, at(z=-(h - r)) @ Matrix.Identity(4)) if False else (
        [(tuple((m @ Vector(p))), tuple((m.to_3x3() @ Vector(n)).normalized())) for p, n in
         tube(r, r, max(h - 2 * r, 0.001), at(z=-(h - r))) + ball(r, at(z=-r)) + ball(r, at(z=-(h - r)))])

def ring(R, r, m):
    bm = bmesh.new()
    seg, sides = 24, 8
    for i in range(seg):
        a0, a1 = 2 * math.pi * i / seg, 2 * math.pi * (i + 1) / seg
        for j in range(sides):
            b0, b1 = 2 * math.pi * j / sides, 2 * math.pi * (j + 1) / sides
            def p(a, b):
                return bm.verts.new(((R + r * math.cos(b)) * math.cos(a), (R + r * math.cos(b)) * math.sin(a), r * math.sin(b)))
            bm.faces.new([p(a0, b0), p(a1, b0), p(a1, b1), p(a0, b1)])
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.0005)
    return tris_of(bm, True, m)

def rgb(hex_):
    return tuple(int(hex_[i:i + 2], 16) for i in (1, 3, 5))

# ---- the meshes: name -> list of (triangles, colour, alpha)

MESHES = {}
NUMBERS = {}

def part(name, pieces, numbers=None):
    MESHES[name] = pieces
    NUMBERS[name] = (list(numbers or []) + [0.0] * 8)[:8]

DARK, WHITE, STEEL = rgb('#23283B'), rgb('#F4F1E8'), rgb('#8D93A6')

# Bolt: small and quick. A round helmet with two bolt-shaped fins, a visor, a short blaster.
YEL, YEL2 = rgb('#FFD23F'), rgb('#F2A516')
part('bolt-body', [
    (box(0.44, 0.30, 0.46, 0.10, at(z=0.25)), WHITE, TEAM),
    (box(0.46, 0.32, 0.10, 0.04, at(z=0.03)), DARK, OWN),
    (box(0.16, 0.05, 0.20, 0.02, at(y=-0.15, z=0.30, ry=0.35)), YEL, GLOW),
    (ball(0.11, at(x=0.25, z=0.42)), YEL2, OWN), (ball(0.11, at(x=-0.25, z=0.42)), YEL2, OWN),
], [0.56, 0.12, 0.98, 0.29, 1.06, 0.40])
part('bolt-head', [
    (ball(0.25, at(z=0.24), sz=0.92), YEL, OWN),
    (box(0.40, 0.14, 0.13, 0.05, at(y=-0.17, z=0.25)), DARK, OWN),
    (ball(0.045, at(x=0.09, y=-0.235, z=0.26)), rgb('#7CF3FF'), GLOW), (ball(0.045, at(x=-0.09, y=-0.235, z=0.26)), rgb('#7CF3FF'), GLOW),
    (tube(0.07, 0.0, 0.26, at(x=0.19, z=0.40, ry=0.5)), YEL2, OWN), (tube(0.07, 0.0, 0.26, at(x=-0.19, z=0.40, ry=-0.5)), YEL2, OWN),
])
part('bolt-arm', [(capsule(0.07, 0.34, at()), WHITE, TEAM), (ball(0.095, at(z=-0.36)), DARK, OWN)])
part('bolt-leg', [(capsule(0.085, 0.42, at()), DARK, OWN), (box(0.20, 0.30, 0.14, 0.05, at(y=-0.04, z=-0.49)), YEL2, OWN)])
part('bolt-gun', [
    (box(0.09, 0.30, 0.12, 0.03, at(y=-0.10)), STEEL, OWN), (box(0.07, 0.09, 0.16, 0.02, at(y=0.02, z=-0.10)), DARK, OWN),
    (tube(0.035, 0.035, 0.16, at(y=-0.25, rx=math.pi / 2)), DARK, OWN), (ball(0.05, at(y=-0.42)), YEL, GLOW),
])

# Boulder: wide and heavy. A slab of a body with shoulder plates, a small head sunk into it, a cannon.
ROCK, ROCK2, ORANGE = rgb('#7C7F8E'), rgb('#5B5E6E'), rgb('#FF8A3D')
part('boulder-body', [
    (box(0.82, 0.54, 0.62, 0.14, at(z=0.33)), ROCK, OWN),
    (box(0.50, 0.10, 0.34, 0.05, at(y=-0.27, z=0.36)), WHITE, TEAM),
    (box(0.34, 0.40, 0.20, 0.08, at(x=0.46, z=0.60)), WHITE, TEAM), (box(0.34, 0.40, 0.20, 0.08, at(x=-0.46, z=0.60)), WHITE, TEAM),
    (box(0.70, 0.50, 0.12, 0.05, at(z=0.04)), ROCK2, OWN),
], [0.46, 0.20, 1.00, 0.56, 1.06, 0.50])
part('boulder-head', [
    (ball(0.22, at(z=0.12), sz=0.8), ROCK2, OWN),
    (box(0.30, 0.10, 0.07, 0.03, at(y=-0.17, z=0.13)), ORANGE, GLOW),
    (box(0.10, 0.10, 0.12, 0.03, at(x=0.20, z=0.20)), ROCK, OWN), (box(0.10, 0.10, 0.12, 0.03, at(x=-0.20, z=0.20)), ROCK, OWN),
])
part('boulder-arm', [(capsule(0.13, 0.42, at()), ROCK, OWN), (ball(0.18, at(z=-0.46)), ROCK2, OWN), (box(0.30, 0.30, 0.08, 0.03, at(z=-0.30)), WHITE, TEAM)])
part('boulder-leg', [(capsule(0.14, 0.32, at()), ROCK2, OWN), (box(0.30, 0.38, 0.16, 0.06, at(y=-0.03, z=-0.38)), ROCK, OWN)])
part('boulder-gun', [
    (tube(0.13, 0.15, 0.46, at(y=0.05, rx=math.pi / 2)), ROCK2, OWN), (tube(0.16, 0.16, 0.08, at(y=-0.38, rx=math.pi / 2)), ORANGE, OWN),
    (ball(0.10, at(y=-0.44)), ORANGE, GLOW), (box(0.10, 0.20, 0.14, 0.03, at(y=0.02, z=-0.14)), DARK, OWN),
])

# Lumen: tall and slight. A robe that flares, a glowing orb for a head under a halo, a long crystal rifle.
VIOLET, CYAN, PALE = rgb('#6B4BD6'), rgb('#7CF3FF'), rgb('#E9E4FF')
part('lumen-body', [
    (tube(0.26, 0.15, 0.62, at(z=-0.04)), WHITE, TEAM),
    (tube(0.17, 0.19, 0.10, at(z=0.50)), PALE, OWN),
    (box(0.08, 0.06, 0.36, 0.02, at(y=-0.18, z=0.24)), CYAN, GLOW),
    (ring(0.24, 0.035, at(z=0.02)), PALE, OWN),
], [0.70, 0.09, 1.24, 0.20, 1.30, 0.46])
part('lumen-head', [
    (ball(0.20, at(z=0.24)), CYAN, GLOW),
    (ball(0.225, at(y=0.03, z=0.25), sy=0.9), VIOLET, OWN),
    (ring(0.22, 0.025, at(z=0.53)), rgb('#FFE680'), GLOW),
])
part('lumen-arm', [(capsule(0.055, 0.40, at()), WHITE, TEAM), (ball(0.07, at(z=-0.42)), PALE, OWN)])
part('lumen-leg', [(capsule(0.065, 0.56, at()), VIOLET, OWN), (box(0.15, 0.26, 0.10, 0.04, at(y=-0.04, z=-0.63)), PALE, OWN)])
part('lumen-gun', [
    (box(0.06, 0.62, 0.08, 0.02, at(y=-0.22)), VIOLET, OWN), (box(0.05, 0.10, 0.16, 0.02, at(y=0.04, z=-0.10)), DARK, OWN),
    (tube(0.07, 0.0, 0.22, at(y=-0.52, rx=math.pi / 2)), CYAN, GLOW), (ring(0.07, 0.015, at(y=-0.36, rx=math.pi / 2)), rgb('#FFE680'), GLOW),
])

# The arena's pieces. A crate is a unit cube on its base, rounded, in the instance's colour, with a paler lid.
part('crate', [
    (box(1.0, 1.0, 1.0, 0.06, at(z=0.5)), WHITE, INSTANCE),
    (box(0.86, 0.86, 0.04, 0.015, at(z=1.0)), rgb('#FFFFFF'), INSTANCE),
])
part('pack', [
    (box(0.44, 0.44, 0.34, 0.08, at(z=0.17)), rgb('#3FE07A'), OWN),
    (box(0.30, 0.10, 0.06, 0.02, at(z=0.35)), WHITE, GLOW), (box(0.10, 0.30, 0.06, 0.02, at(z=0.35)), WHITE, GLOW),
])
part('shot', [(ball(0.5, at(), sy=1.0), WHITE, GLOW_INSTANCE)])
part('disc', [(tube(1.0, 1.0, 0.012, at(), segments=28), rgb('#101425'), OWN)])
part('pad', [(tube(1.0, 0.92, 0.05, at(), segments=32), WHITE, INSTANCE), (ring(0.78, 0.05, at(z=0.05)), WHITE, GLOW_INSTANCE)])

# ---- the file

def game(p):   # Blender (x, y, z) to the game's (x, y up, z ahead): its front (-y) is the game's +z
    return (p[0], p[2], -p[1])

names = list(MESHES.keys())
vertices = bytearray()
table = bytearray()
first = 0
for name in names:
    count = 0
    for tris, colour, alpha in MESHES[name]:
        for pos, normal in tris:
            g, n = game(pos), game(normal)
            vertices += struct.pack('<6f4B', g[0], g[1], g[2], n[0], n[1], n[2], colour[0], colour[1], colour[2], alpha)
            count += 1
    table += name.encode().ljust(16, b'\0') + struct.pack('<2I8f', first, count, *NUMBERS[name])
    first += count
out = os.path.join(ROOT, 'host', 'Models', 'scrappers.gbm')
with open(out, 'wb') as f:
    f.write(b'GBM1' + struct.pack('<I', len(names)) + table + vertices)
print('wrote', out, len(names), 'meshes', first, 'vertices', os.path.getsize(out), 'bytes')

# ---- portraits: each Scrapper stood up from its parts, in a team's blue, on a clear ground

def stand(kind):
    for o in list(bpy.data.objects): bpy.data.objects.remove(o, do_unlink=True)
    n = NUMBERS[kind + '-body']
    hip_y, hip_x, sh_y, sh_x, neck_y, arm = n[0], n[1], n[2], n[3], n[4], n[5]
    def put(name, m):
        for i, (tris, colour, alpha) in enumerate(MESHES[name]):
            mesh = bpy.data.meshes.new(name)
            mesh.from_pydata([p for p, _ in tris], [], [(k, k + 1, k + 2) for k in range(0, len(tris), 3)])
            for poly in mesh.polygons: poly.use_smooth = True
            c = (64, 158, 255) if alpha in (TEAM, INSTANCE, GLOW_INSTANCE) else colour
            mat = bpy.data.materials.new(name)
            mat.diffuse_color = tuple((v / 255) ** 2.2 for v in c) + (1,)
            mesh.materials.append(mat)
            o = bpy.data.objects.new(name, mesh)
            o.matrix_world = m
            bpy.context.scene.collection.objects.link(o)
    put(kind + '-body', at(z=hip_y))
    put(kind + '-head', at(z=neck_y, rz=-0.25))
    put(kind + '-leg', at(x=hip_x, z=hip_y, rx=0.12)); put(kind + '-leg', at(x=-hip_x, z=hip_y, rx=-0.18))
    put(kind + '-arm', at(x=sh_x, z=sh_y, ry=0.25, rx=-0.1))
    put(kind + '-arm', at(x=-sh_x, z=sh_y, rx=1.25))            # the hand that holds the blaster, raised
    put(kind + '-gun', at(x=-sh_x, y=-arm * 0.95, z=sh_y - arm * 0.30))

scene = bpy.context.scene
scene.render.engine = 'BLENDER_WORKBENCH'
scene.display.shading.light = 'STUDIO'
scene.display.shading.color_type = 'MATERIAL'
scene.display.shading.show_cavity = True
scene.display.shading.show_object_outline = True
scene.display.shading.object_outline_color = (0.05, 0.06, 0.12)
scene.display.shading.show_shadows = False
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = 512
scene.render.image_settings.file_format = 'PNG'
scene.render.image_settings.color_mode = 'RGBA'
scene.view_settings.view_transform = 'Standard'
for k, kind in enumerate(['bolt', 'boulder', 'lumen']):
    stand(kind)
    cam_data = bpy.data.cameras.new('cam'); cam_data.lens = 70
    cam = bpy.data.objects.new('cam', cam_data)
    scene.collection.objects.link(cam)
    top = NUMBERS[kind + '-body'][4] + 0.55
    cam.location = (1.6, -4.4, top * 0.62)
    look = Vector((0, 0, top * 0.50)) - cam.location
    cam.rotation_euler = look.to_track_quat('-Z', 'Y').to_euler()
    scene.camera = cam
    folder = os.path.join(ROOT, 'host', 'Assets.xcassets', 'Scrapper%d.imageset' % k)
    os.makedirs(folder, exist_ok=True)
    scene.render.filepath = os.path.join(folder, 'scrapper.png')
    bpy.ops.render.render(write_still=True)
    with open(os.path.join(folder, 'Contents.json'), 'w') as f:
        f.write('{\n  "images" : [ { "filename" : "scrapper.png", "idiom" : "universal" } ],\n  "info" : { "author" : "xcode", "version" : 1 }\n}\n')
    print('rendered', scene.render.filepath)
