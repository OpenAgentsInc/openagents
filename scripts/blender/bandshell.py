"""Build the Commons bandshell and write it as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/bandshell.py -- [OUT.glb | OUT_DIR]

A half-dome shell over a raised half-round stage. The shell's open side and
the stage's front steps face -Y in Blender, which is +Z (glTF's front) after
export. Nested arches at the shell's mouth step out like a classic
bandshell, and the stage's face uses the village kit's brick. 9 m across.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

import bmesh  # noqa: E402
import bpy  # noqa: E402

out = kit.out_path("bandshell")
kit.reset()

brick = kit.tex_mat("Bandshell_Brick", "T_Brick_BaseColor.png", 256, tint=(1.0, 0.92, 0.82))
plaster = kit.mat("Bandshell_Shell", (0.93, 0.89, 0.8), 0.8)
inner = kit.mat("Bandshell_Inner", (0.3, 0.45, 0.66), 0.8)
accent = kit.mat("Bandshell_Accent", (0.62, 0.16, 0.14), 0.7)
wood = kit.mat("Bandshell_Boards", (0.55, 0.36, 0.2), 0.85)
stone = kit.mat("Bandshell_Stone", (0.62, 0.59, 0.54), 0.9)

R = 4.5  # stage radius
SH = 0.9  # stage height
SR = 4.0  # shell radius

# Stage: a half-round platform (back half, +Y) plus a strip at the front.
stage = kit.cyl("Stage", R, SH, (0, 0, SH / 2), brick, verts=32)
kit.cyl_uv(stage, tile=1.6)
# Cut the stage to its back half plus a 1 m apron by trimming with a box.
cut = kit.box("Cut", (2 * R + 1, R, SH + 1), (0, -R / 2 - 1.0, SH / 2))
mod = stage.modifiers.new("Trim", "BOOLEAN")
mod.operation = "DIFFERENCE"
mod.object = cut
mod.solver = "EXACT"
kit.apply_modifiers(stage)
bpy.data.objects.remove(cut)
kit.cyl_uv(stage, tile=1.6)
# Boards on the deck and a stone lip around it.
deck = kit.cyl("Deck", R - 0.1, 0.06, (0, 0, SH + 0.03), wood, verts=32)
cut = kit.box("Cut2", (2 * R + 1, R, 1), (0, -R / 2 - 0.9, SH))
mod = deck.modifiers.new("Trim", "BOOLEAN")
mod.operation = "DIFFERENCE"
mod.object = cut
mod.solver = "EXACT"
kit.apply_modifiers(deck)
bpy.data.objects.remove(cut)
kit.box("Lip", (2 * R, 0.25, 0.12), (0, -1.0 + 0.125, SH + 0.06), stone)

# Front steps, centered.
for i in range(4):
    h = SH * (i + 1) / 5
    kit.box("Step%d" % i, (2.4, 0.32, h), (0, -1.0 - 0.32 * (4 - i) + 0.16, h / 2), stone)

# Shell: a quarter sphere opening to -Y, solidified, warm inside.
shell = kit.patch("Shell", SR, (0, 180), (0, 90), (16, 8), (0, 0, SH), plaster)
kit.solidify(shell, 0.12)
lining = kit.patch("Lining", SR - 0.1, (0, 180), (0, 88), (16, 8), (0, 0, SH), inner)
# The lining faces the audience: flip it to face inward.
lining.data.flip_normals()
# Nested arches at the mouth: each a half ring in the shell's opening plane.
for k, (r, w, m) in enumerate(((SR + 0.15, 0.3, accent), (SR + 0.45, 0.26, plaster), (SR + 0.7, 0.2, accent))):
    arch = kit.ring("Arch%d" % k, r, w / 2, (0, -0.05 - 0.12 * k, SH), m, segs=24, minor_segs=4, rot=(math.pi / 2, 0, 0))
    # Keep the upper half of the ring.
    me = arch.data
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.co.y < -0.01], context="VERTS")
    bm.to_mesh(me)
    bm.free()
# Columns flanking the mouth.
for x in (-1, 1):
    kit.cyl("Column%d" % x, 0.22, 0.3, (x * (SR + 0.45), -0.2, SH + 0.15), stone, verts=10)

body = kit.join("Bandshell")
kit.ground(body)
kit.flat()
kit.export(out)
