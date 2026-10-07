"""Build the Observatory Hill observatory and write it as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/observatory.py -- [OUT.glb | OUT_DIR]

A stone drum tower faced with the village kit's brick, a front door and four
windows, a cornice, and a slate dome with an open observing slit, from
which a brass telescope pokes out. An outside stair winds up the tower's
flank to a landing and a side door. The door faces -Y in Blender, which is
+Z (glTF's front) after export. The tower is 4 m across and about 7.6 m to
the top of the dome.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

import bmesh  # noqa: E402
import bpy  # noqa: E402

out = kit.out_path("observatory")
kit.reset()

brick = kit.tex_mat("Observatory_Brick", "T_Brick_BaseColor.png", 256, tint=(1.0, 0.94, 0.85))
stone = kit.mat("Observatory_Stone", (0.6, 0.57, 0.52), 0.9)
dome = kit.mat("Observatory_Dome", (0.3, 0.52, 0.5), 0.6, 0.3)
rib = kit.mat("Observatory_Rib", (0.2, 0.36, 0.35), 0.6, 0.3)
dark = kit.mat("Observatory_Dark", (0.05, 0.05, 0.07), 1.0)
wood = kit.mat("Observatory_Wood", (0.38, 0.22, 0.1), 0.85)
brass = kit.mat("Observatory_Brass", (0.8, 0.6, 0.25), 0.35, 0.8)
glass = kit.mat("Observatory_Glass", (0.95, 0.8, 0.45), 0.4, emit=(1.0, 0.75, 0.35), strength=0.6)

R = 2.0  # tower radius
H = 4.6  # tower height to the cornice
seg = 20
front = -math.pi / 2  # the door's azimuth (-Y)


def at(a, r, z):
    return (r * math.cos(a), r * math.sin(a), z)


# The drum, its plinth, and its cornice.
# The wall starts inside the plinth, so its bottom doesn't share the
# ground plane with the plinth's.
wall = kit.cyl("Wall", R, H - 0.33, (0, 0, (H + 0.33) / 2), brick, verts=seg)
kit.cyl_uv(wall, tile=1.6)
kit.cyl("Plinth", R + 0.12, 0.35, (0, 0, 0.175), stone, verts=seg)
kit.cyl("Band", R + 0.04, 0.12, (0, 0, 2.6), stone, verts=seg)
kit.cyl("Cornice", R + 0.18, 0.18, (0, 0, H + 0.09), stone, verts=seg, r2=R + 0.25)
kit.cyl("Drum", R + 0.05, 0.25, (0, 0, H + 0.3), stone, verts=seg)

# Dome: a hemisphere on the drum, its observing slit open from low on the
# front up over the crown, showing the dark inner shell.
DZ = H + 0.42
DR = R + 0.02
slit = 13  # half-width of the slit, in degrees of azimuth
el0 = 14  # where the slit starts, in degrees of elevation
fd = math.degrees(front)
kit.patch("DomeBase", DR, (0, 360), (0, el0), (24, 2), (0, 0, DZ), dome)
kit.patch("Dome", DR, (fd + slit, fd + 360 - slit), (el0, 90), (22, 7), (0, 0, DZ), dome)
kit.patch("Inner", DR - 0.14, (0, 360), (0, 90), (16, 6), (0, 0, DZ), dark)


def cheek(name, a_deg):
    """The slit's side wall: a strip from the outer shell to the inner one."""
    bm = bmesh.new()
    a = math.radians(a_deg)
    rows = []
    for k in range(8):
        e = math.radians(el0 + (90 - el0) * k / 7)
        rows.append([
            bm.verts.new((r * math.cos(e) * math.cos(a), r * math.cos(e) * math.sin(a), DZ + r * math.sin(e)))
            for r in (DR, DR - 0.14)
        ])
    for k in range(7):
        bm.faces.new((rows[k][0], rows[k + 1][0], rows[k + 1][1], rows[k][1]))
    # The sill at the slit's foot.
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(rib)
    kit.solidify(o, 0.03)
    return o


cheek("CheekL", fd - slit)
cheek("CheekR", fd + slit)
# Shutter rails along both edges of the slit.
for side in (-1, 1):
    a = fd + side * (slit + 2)
    r = kit.patch("Rail%d" % side, DR + 0.02, (a - 2, a + 2), (el0 - 4, 90), (1, 7), (0, 0, DZ), rib)
    kit.solidify(r, 0.05)
kit.patch("SlitSill", DR - 0.02, (fd - slit - 1, fd + slit + 1), (el0 - 3, el0), (3, 1), (0, 0, DZ), rib)

# Ribs over the dome and a cap at the crown.
for i in range(6):
    a = fd + 60 * i + 60 if i < 5 else None
    if a is None:
        continue
    rib_o = kit.patch("Rib%d" % i, DR + 0.01, (a - 1.5, a + 1.5), (0, 84), (1, 7), (0, 0, DZ), rib)
    kit.solidify(rib_o, 0.04)
kit.cyl("Crown", 0.22, 0.12, (0, 0, DZ + DR - 0.02), rib, verts=10)
kit.ball("Finial", 0.1, (0, 0, DZ + DR + 0.1), brass, segs=8, rings=5)

# Telescope: a brass tube rising out of the slit, angled up to the front.
tilt = math.radians(50)
tube_len = 2.6
mid = (0, -1.2, DZ + 1.25)
kit.cyl("Tube", 0.2, tube_len, mid, brass, verts=10, r2=0.17, rot=(tilt, 0, 0))
end = (0, mid[1] - math.sin(tilt) * tube_len / 2, mid[2] + math.cos(tilt) * tube_len / 2)
kit.cyl("Lens", 0.24, 0.16, end, brass, verts=10, rot=(tilt, 0, 0))
kit.cyl("LensGlass", 0.19, 0.17, end, dark, verts=10, rot=(tilt, 0, 0))
for t in (0.25, 0.65):
    p = (0, mid[1] + (t - 0.5) * -math.sin(tilt) * tube_len, mid[2] + (t - 0.5) * math.cos(tilt) * tube_len)
    kit.cyl("Hoop%.2f" % t, 0.215, 0.06, p, rib, verts=10, rot=(tilt, 0, 0))

# Front door: a dark wooden arched door in a stone frame.
door_w, door_h = 1.0, 1.9
kit.box("DoorFrame", (door_w + 0.3, 0.2, door_h + 0.15), at(front, R + 0.02, (door_h + 0.15) / 2 + 0.3), stone)
kit.cyl("DoorArchFrame", (door_w + 0.3) / 2, 0.2, at(front, R + 0.02, door_h + 0.45), stone, verts=12, rot=(math.pi / 2, 0, 0))
kit.box("Door", (door_w, 0.1, door_h), at(front, R + 0.1, door_h / 2 + 0.33), wood)
kit.cyl("DoorArch", door_w / 2, 0.1, at(front, R + 0.1, door_h + 0.33), wood, verts=12, rot=(math.pi / 2, 0, 0))
kit.box("Step", (door_w + 0.6, 0.6, 0.18), at(front, R + 0.3, 0.09), stone)
kit.ball("Knob", 0.05, at(front + 0.12, R + 0.17, 1.2), brass, segs=6, rings=4)

# Windows: lit, arched, with stone sills, in the upper drum.
for k, off in enumerate((-55, 55, 125, -125)):
    a = front + math.radians(off)
    z = 3.45
    turn = a + math.pi / 2
    kit.box("WinFrame%d" % k, (0.9, 0.12, 1.0), at(a, R + 0.0, z - 0.05), stone, rot=(0, 0, turn))
    kit.cyl("WinFrameArch%d" % k, 0.45, 0.12, at(a, R + 0.0, z + 0.45), stone, verts=10, rot=(math.pi / 2, 0, turn))
    kit.box("Win%d" % k, (0.62, 0.1, 0.9), at(a, R + 0.04, z - 0.05), glass, rot=(0, 0, turn))
    kit.cyl("WinArch%d" % k, 0.31, 0.1, at(a, R + 0.04, z + 0.4), glass, verts=10, rot=(math.pi / 2, 0, turn))
    kit.box("Mullion%d" % k, (0.05, 0.12, 1.15), at(a, R + 0.06, z + 0.05), wood, rot=(0, 0, turn))
    kit.box("Sill%d" % k, (1.0, 0.26, 0.1), at(a, R + 0.08, z - 0.58), stone, rot=(0, 0, turn))

# Outside stair: steps winding up the right flank to a landing and a side
# door at 2.6 m, with posts on the outer edge.
steps = 14
rise = 2.6 / (steps + 1)
a0 = front + math.radians(35)
for i in range(steps):
    a = a0 + math.radians(9.5) * i
    z = rise * (i + 1)
    kit.box("Stair%d" % i, (0.9, 0.42, rise), at(a, R + 0.45, z - rise / 2), stone, rot=(0, 0, a))
    # A solid riser under each step, so the stair reads as built masonry.
    kit.box("Under%d" % i, (0.9, 0.42, z - rise), at(a, R + 0.45, (z - rise) / 2), stone, rot=(0, 0, a))
    if i % 3 == 1:
        kit.cyl("Post%d" % i, 0.04, 0.9, at(a, R + 0.85, z + 0.45), wood, verts=6)
a_land = a0 + math.radians(9.5) * steps + math.radians(8)
kit.box("Landing", (1.0, 0.9, 0.2), at(a_land, R + 0.45, 2.6 - 0.1), stone, rot=(0, 0, a_land))
kit.box("LandingBase", (1.0, 0.9, 2.5), at(a_land, R + 0.45, 1.25), stone, rot=(0, 0, a_land))
kit.box("SideDoor", (0.12, 0.8, 1.6), at(a_land + math.radians(4), R + 0.02, 2.6 + 0.8), wood, rot=(0, 0, a_land + math.radians(4)))
kit.cyl("RailPost", 0.04, 0.9, at(a_land, R + 0.88, 2.6 + 0.45), wood, verts=6)

body = kit.join("Observatory")
kit.ground(body)
kit.flat()
kit.export(out)
kit.flickers(out)
kit.fail_on_flickers()
