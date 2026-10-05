"""The arcane sheet: four sprites for nature magic, in a 2 by 2 sheet.

Run headless (or through `scripts/blender/build-fx.sh`):
    Blender -b --factory-startup --python scripts/blender/fx/arcane.py -- OUT.png [CELL]

Cells, row by row:

0. rune: a druid's rune circle seen from above, for a particle laid flat
   on the ground: an outer and an inner ring, a band of twelve glyphs
   between them, and a seven-pointed star of thin strokes inside. Pure
   emission in white, so the particle's color tints it.
1. leaf: an oak-like leaf with a midrib and veins, lit by a sun from
   above, pale so the particle's color tints it green, gold, or russet.
2. glint: a four-pointed sparkle, a hot center with two crossed rays, for
   motes of arcane light.
3. leaf2: a rounder leaf with a serrated edge, slightly cupped, lit the same
   way.

The rune and the glint are emission in front of an orthographic camera
over a transparent film, so the premultiplied color is the glow and the
alpha is the coverage. The leaves are opaque lit surfaces. Cells are
256 px by default, a 512 px sheet.
"""

import math
import os
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(__file__))
import common  # noqa: E402

TAU = 2 * math.pi


# The emission helpers, as `sparks.py` defines them.
def emissive(n, value, color, coverage_scale=1.0):
    """Emission of `value` in `color`, over coverage `value` clamped.

    The value is windowed by (1 - r²)², so it reaches exactly zero before
    the cell's edge: a particle drawn hundreds of times brighter than the
    sheet must not show its quad's square outline."""
    _, _, r = xy(n)
    window = n.math("POWER", n.math("SUBTRACT", 1.0, n.math("MULTIPLY", r, r), clamp=True), 2.0)
    value = n.math("MULTIPLY", value, window)
    emission = n.node("ShaderNodeEmission", Color=color)
    n.link(value, emission.inputs["Strength"])
    coverage = n.math("MULTIPLY", value, coverage_scale, clamp=True)
    clear = n.node("ShaderNodeBsdfTransparent")
    inverse = n.node("ShaderNodeCombineColor")
    rest = n.math("SUBTRACT", 1.0, coverage)
    for i in range(3):
        n.link(rest, inverse.inputs[i])
    n.link(inverse.outputs[0], clear.inputs["Color"])
    add = n.node("ShaderNodeAddShader")
    n.link(emission.outputs[0], add.inputs[0])
    n.link(clear.outputs[0], add.inputs[1])
    out = n.node("ShaderNodeOutputMaterial")
    n.link(add.outputs[0], out.inputs["Surface"])


def xy(n):
    """The plane's x, y, and r, each in -1..1 across the cell."""
    sep = n.node("ShaderNodeSeparateXYZ")
    n.link(n.node("ShaderNodeTexCoord").outputs["Object"], sep.inputs[0])
    x, y = sep.outputs["X"], sep.outputs["Y"]
    r = n.math("SQRT", n.math("ADD", n.math("MULTIPLY", x, x), n.math("MULTIPLY", y, y)))
    return x, y, r


def gauss(n, d2, width2):
    """exp(-d2 / width2)."""
    return n.math("EXPONENT", n.math("MULTIPLY", d2, -1.0 / width2))


def glow_material(name, strength=1.0):
    m, n = common.material(name)
    e = n.node("ShaderNodeEmission", Color=(1.0, 1.0, 1.0, 1.0), Strength=strength)
    out = n.node("ShaderNodeOutputMaterial")
    n.link(e.outputs[0], out.inputs["Surface"])
    return m


def mesh_object(name, build, material):
    bm = bmesh.new()
    build(bm)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(material)
    return o


def annulus(bm, r0, r1, segs=96):
    inner = [bm.verts.new((r0 * math.cos(TAU * i / segs), r0 * math.sin(TAU * i / segs), 0)) for i in range(segs)]
    outer = [bm.verts.new((r1 * math.cos(TAU * i / segs), r1 * math.sin(TAU * i / segs), 0)) for i in range(segs)]
    for i in range(segs):
        j = (i + 1) % segs
        bm.faces.new((inner[i], outer[i], outer[j], inner[j]))


def stroke(bm, a, b, width):
    ax, ay = a
    bx, by = b
    dx, dy = bx - ax, by - ay
    length = math.hypot(dx, dy) or 1.0
    nx, ny = -dy / length * width / 2, dx / length * width / 2
    vs = [bm.verts.new((x, y, 0)) for x, y in ((ax + nx, ay + ny), (bx + nx, by + ny), (bx - nx, by - ny), (ax - nx, ay - ny))]
    bm.faces.new(vs)


def rune():
    common.reset(CELL, samples=16, extent=1.0)
    mat = glow_material("rune")

    def build(bm):
        annulus(bm, 0.9, 0.955)
        annulus(bm, 0.66, 0.69)
        annulus(bm, 0.3, 0.32, segs=64)
        # Twelve glyphs in the band, each two or three strokes chosen from
        # a fixed pattern so the circle reads as writing.
        shapes = [
            [((-1, -1), (-1, 1)), ((-1, 1), (1, 0)), ((1, 0), (-1, -1))],
            [((0, -1), (0, 1)), ((-1, 0.3), (1, 0.3))],
            [((-1, -1), (1, 1)), ((-1, 1), (1, -1))],
            [((-1, -1), (0, 1)), ((0, 1), (1, -1)), ((-0.5, 0), (0.5, 0))],
            [((-1, 1), (1, 1)), ((0, 1), (0, -1)), ((-1, -1), (1, -1))],
            [((-1, -1), (-1, 1)), ((-1, 0), (1, 1)), ((-1, 0), (1, -1))],
        ]
        for g in range(12):
            a = TAU * g / 12 + TAU / 24
            c, s = math.cos(a), math.sin(a)
            center = (0.7775 * c, 0.7775 * s)
            for (x0, y0), (x1, y1) in shapes[g % len(shapes)]:
                # Glyph space: x along the ring, y across it.
                def place(x, y):
                    u, v = x * 0.055, y * 0.06
                    return (center[0] - s * u + c * v, center[1] + c * u + s * v)

                stroke(bm, place(x0, y0), place(x1, y1), 0.016)
            # A dot between glyphs.
            b = a + TAU / 24
            d = (0.7775 * math.cos(b), 0.7775 * math.sin(b))
            stroke(bm, (d[0] - 0.012, d[1]), (d[0] + 0.012, d[1]), 0.024)
        # A seven-pointed star of strokes between the inner rings.
        pts = [(0.64 * math.cos(TAU * i / 7 + math.pi / 2), 0.64 * math.sin(TAU * i / 7 + math.pi / 2)) for i in range(7)]
        for i in range(7):
            stroke(bm, pts[i], pts[(i + 3) % 7], 0.018)

    mesh_object("Rune", build, mat)
    # A faint wash inside the outer ring, so the circle glows as a whole.
    m, n = common.material("wash")
    _, _, r = xy(n)
    band = n.math("SUBTRACT", r, 0.8)
    value = n.math("MULTIPLY", gauss(n, n.math("MULTIPLY", band, band), 0.03), 0.18)
    emissive(n, value, (1.0, 1.0, 1.0, 1.0))
    p = common.plane(2.0, m)
    p.location.z = -0.01
    return common.render()


def leaf_outline(lobes, length=1.7, width=0.62, serrate=0.0, n=64):
    """A leaf's outline from base (-y) to tip (+y), as (x, y) points around."""
    pts = []
    for i in range(n + 1):
        t = i / n  # 0 at the base, 1 at the tip
        w = width * math.sin(math.pi * t) ** 0.8 * (1.0 - 0.25 * t)
        if lobes:
            w *= 0.78 + 0.22 * math.cos(TAU * lobes * t) ** 2
        if serrate:
            w *= 1.0 - serrate * (i % 2)
        pts.append((w, -length / 2 + length * t))
    right = pts
    left = [(-x, y) for x, y in reversed(pts[1:-1])]
    return right + left


def leaf(lobes, serrate, cup):
    common.reset(CELL, samples=24, extent=1.0)
    common.ambient(0.25, (0.85, 0.9, 1.0))
    common.sun((0.35, -0.5, -1.0), 1.4, (1.0, 0.97, 0.9))
    m, n = common.material("leaf")
    bsdf = n.node("ShaderNodeBsdfPrincipled", Roughness=0.6)
    bsdf.inputs["Base Color"].default_value = (0.62, 0.62, 0.6, 1.0)
    if "Subsurface Weight" in bsdf.inputs:
        bsdf.inputs["Subsurface Weight"].default_value = 0.0
    out = n.node("ShaderNodeOutputMaterial")
    n.link(bsdf.outputs[0], out.inputs["Surface"])
    vein_m, vn = common.material("vein")
    vb = vn.node("ShaderNodeBsdfPrincipled", Roughness=0.7)
    vb.inputs["Base Color"].default_value = (0.3, 0.3, 0.27, 1.0)
    vn.link(vb.outputs[0], vn.node("ShaderNodeOutputMaterial").inputs["Surface"])
    outline = leaf_outline(lobes, serrate=serrate)

    def build(bm):
        center = bm.verts.new((0, 0, 0))
        ring = [bm.verts.new((x, y, cup * x * x)) for x, y in outline]
        for i in range(len(ring)):
            bm.faces.new((center, ring[i], ring[(i + 1) % len(ring)]))

    o = mesh_object("Leaf", build, m)
    o.rotation_euler = (0, 0, math.radians(35))

    def veins(bm):
        stroke(bm, (0, -0.95), (0, 0.8), 0.035)
        for k in range(5):
            y = -0.55 + k * 0.27
            t = (y + 0.22 + 0.85) / 1.7
            reach = 0.62 * math.sin(math.pi * t) ** 0.8 * (1.0 - 0.25 * t) * 0.6
            for sx in (-1, 1):
                stroke(bm, (0, y), (sx * reach, y + 0.22), 0.018)

    v = mesh_object("Veins", veins, vein_m)
    v.location.z = 0.02
    v.rotation_euler = (0, 0, math.radians(35))
    # The stem.
    stem = mesh_object("Stem", lambda bm: stroke(bm, (0, -1.0), (0, -0.82), 0.04), vein_m)
    stem.rotation_euler = (0, 0, math.radians(35))
    return common.render()


def glint():
    common.reset(CELL, samples=16, extent=1.0)
    m, n = common.material("glint")
    x, y, r = xy(n)
    r2 = n.math("MULTIPLY", r, r)
    core = gauss(n, r2, 0.006)
    halo = n.math("MULTIPLY", gauss(n, r2, 0.06), 0.3)
    xx = n.math("MULTIPLY", x, x)
    yy = n.math("MULTIPLY", y, y)
    ray_h = n.math("MULTIPLY", gauss(n, yy, 0.0008), gauss(n, xx, 0.3))
    ray_v = n.math("MULTIPLY", gauss(n, xx, 0.0008), gauss(n, yy, 0.3))
    value = n.math("ADD", n.math("ADD", core, halo), n.math("MULTIPLY", n.math("ADD", ray_h, ray_v), 0.8))
    emissive(n, value, (1.0, 1.0, 1.0, 1.0))
    common.plane(2.0, m)
    return common.render()


CELL = 256


def main():
    global CELL
    a = common.args()
    path = a[0]
    CELL = int(a[1]) if len(a) > 1 else 256
    cells = [rune(), leaf(lobes=3.5, serrate=0.0, cup=0.12), glint(), leaf(lobes=0, serrate=0.05, cup=0.2)]
    common.write_sheet(path, cells, 2, gain=1.0)


main()
