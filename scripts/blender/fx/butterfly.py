"""The butterfly sheet: two butterflies' wingbeats in a 4 by 4 sheet.

Run headless (or through `scripts/blender/build-fx.sh`):
    Blender -b --factory-startup --python scripts/blender/fx/butterfly.py -- OUT.png [CELL]

Frames 0 to 7 are an orange butterfly with black-edged wings, white-spotted
at the tips; frames 8 to 15 a pale lemon one. Each run of eight is one
wingbeat, from wings spread flat, up to nearly closed above the back, and
down again, so a particle that loops through them flutters. The camera looks
down on the back, a little from behind, so the wings foreshorten as they
rise. The wings are thin lit surfaces under a sun and a soft sky, drawn as
`light = "lit"` particles, whose color comes from the sheet. Cells are
128 px by default, a 512 px sheet.
"""

import math
import os
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(__file__))
import common  # noqa: E402

# Wing outlines in the right wing's frame: x out from the body, y forward.
FOREWING = [(0.0, 0.02), (0.18, 0.2), (0.62, 0.36), (0.78, 0.24), (0.66, 0.02), (0.08, -0.05)]
HINDWING = [(0.0, -0.02), (0.1, -0.06), (0.52, -0.12), (0.56, -0.38), (0.32, -0.5), (0.04, -0.2)]


def shaded(name, color, roughness=0.7):
    m, n = common.material(name)
    bsdf = n.node("ShaderNodeBsdfPrincipled", **{"Base Color": (*color, 1.0), "Roughness": roughness})
    out = n.node("ShaderNodeOutputMaterial")
    n.link(bsdf.outputs[0], out.inputs["Surface"])
    return m


def wing(name, outline, scale, material, side, z=0.0):
    bm = bmesh.new()
    verts = [bm.verts.new((side * x * scale, y * scale, z)) for x, y in outline]
    bm.faces.new(verts if side > 0 else list(reversed(verts)))
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(material)
    return o


def butterfly(colors, angle):
    """One butterfly, its wings raised `angle` radians off flat."""
    for o in list(bpy.data.objects):
        if o.type == "MESH":
            bpy.data.objects.remove(o)
    rim, face, spot = colors
    rim_m, face_m, spot_m = shaded("rim", rim), shaded("face", face), shaded("spot", spot)
    for side in (1, -1):
        pivot = bpy.data.objects.new("pivot", None)
        bpy.context.scene.collection.objects.link(pivot)
        parts = [
            wing("fore_rim", FOREWING, 1.0, rim_m, side),
            wing("hind_rim", HINDWING, 1.0, rim_m, side),
            wing("fore", FOREWING, 0.84, face_m, side, 0.004),
            wing("hind", HINDWING, 0.82, face_m, side, 0.004),
        ]
        for k, (x, y) in enumerate(((0.62, 0.26), (0.7, 0.16))):
            bpy.ops.mesh.primitive_circle_add(vertices=8, radius=0.035, fill_type="NGON",
                                              location=(side * x, y, 0.008))
            dot = bpy.context.object
            dot.data.materials.append(spot_m)
            parts.append(dot)
        for p in parts:
            p.parent = pivot
        # Wings rise about the body's long axis, mirrored on each side.
        pivot.rotation_euler = (0.0, -side * angle, 0.0)
    bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.03, depth=0.42, location=(0, -0.05, 0.01),
                                        rotation=(math.radians(90), 0, 0))
    body = bpy.context.object
    body.data.materials.append(rim_m)


def main():
    a = common.args()
    out = a[0] if a else "butterfly.png"
    cell = int(a[1]) if len(a) > 1 else 128
    samples = int(os.environ.get("FX_SAMPLES", "64"))
    s = common.reset(cell, samples=samples, extent=0.9)
    # Look down on the back from a little behind, so a raised wing
    # foreshortens.
    cam = s.camera
    cam.location = (0.0, -3.5, 9.4)
    cam.rotation_euler = (math.radians(20), 0.0, 0.0)
    common.ambient(0.6, (0.75, 0.85, 1.0))
    common.sun((0.3, 0.4, -1.0), 3.0, (1.0, 0.95, 0.85))
    kinds = [
        ((0.03, 0.02, 0.02), (0.95, 0.42, 0.04), (0.95, 0.92, 0.85)),
        ((0.35, 0.33, 0.12), (0.97, 0.93, 0.55), (0.98, 0.6, 0.12)),
    ]
    cells = []
    for colors in kinds:
        for f in range(8):
            # Spread flat (0), up to nearly closed (1.35 rad), and down.
            angle = 1.35 * 0.5 * (1 - math.cos(2 * math.pi * f / 8))
            butterfly(colors, angle)
            cells.append(common.render())
    common.write_sheet(out, cells, 4)


main()
