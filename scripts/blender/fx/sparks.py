"""The sparks sheet: four single sprites in a 2 by 2 sheet.

Run headless (or through `scripts/blender/build-fx.sh`):
    Blender -b --factory-startup --python scripts/blender/fx/sparks.py -- OUT.png [CELL]

Cells, row by row:

0. spark: a white-hot dot with a soft streak along the sheet's horizontal
   axis, which a velocity-stretched particle lays along its flight.
1. flare: a soft round glow with a hot center and six faint rays.
2. ring: a shockwave, a bright band near the rim over a fainter inner
   wash, broken up by noise so it doesn't read as a perfect circle.
3. dust: a clumpy, wispy puff, lit from above, for debris dust and
   ground dirt; the particle's color tints it.

The first three are emission on a plane in front of an orthographic
camera; the shader adds the emission to a transparent surface whose
transparency is one minus the coverage, so the premultiplied color is the
glow and the alpha is the coverage. The dust is a scattering volume like
the smoke sheet's. Cells are 256 px by default, a 512 px sheet.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import common  # noqa: E402


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


def spark():
    m, n = common.material("spark")
    x, y, r = xy(n)
    core = gauss(n, n.math("MULTIPLY", r, r), 0.012)
    halo = n.math("MULTIPLY", gauss(n, n.math("MULTIPLY", r, r), 0.08), 0.25)
    across = n.math("MULTIPLY", y, y)
    along = n.math("MULTIPLY", x, x)
    streak = n.math(
        "MULTIPLY",
        gauss(n, across, 0.0035),
        n.math("MULTIPLY", gauss(n, along, 0.45), 0.7),
    )
    value = n.math("ADD", n.math("ADD", core, halo), streak)
    emissive(n, value, (1.0, 0.93, 0.78, 1.0))
    return m


def flare():
    m, n = common.material("flare")
    x, y, r = xy(n)
    r2 = n.math("MULTIPLY", r, r)
    core = gauss(n, r2, 0.01)
    glow = n.math("MULTIPLY", gauss(n, r2, 0.09), 0.45)
    wide = n.math("MULTIPLY", gauss(n, r2, 0.35), 0.12)
    angle = n.math("ARCTAN2", y, x)
    rays = n.math(
        "MULTIPLY",
        n.math("POWER", n.math("ABSOLUTE", n.math("COSINE", n.math("MULTIPLY", angle, 3.0))), 40.0),
        n.math("MULTIPLY", gauss(n, r2, 0.25), 0.16),
    )
    value = n.math("ADD", n.math("ADD", core, glow), n.math("ADD", wide, rays))
    emissive(n, value, (1.0, 0.9, 0.75, 1.0))
    return m


def ring():
    m, n = common.material("ring")
    x, y, r = xy(n)
    band = n.math("SUBTRACT", r, 0.82)
    rim = gauss(n, n.math("MULTIPLY", band, band), 0.0016)
    wash_d = n.math("SUBTRACT", r, 0.68)
    wash = n.math("MULTIPLY", gauss(n, n.math("MULTIPLY", wash_d, wash_d), 0.03), 0.35)
    noise = n.node("ShaderNodeTexNoise", Scale=4.0, Detail=4.0, Roughness=0.6)
    n.link(n.node("ShaderNodeTexCoord").outputs["Object"], noise.inputs["Vector"])
    breakup = n.math("ADD", n.math("MULTIPLY", noise.outputs["Fac"], 1.4), -0.1)
    value = n.math("MULTIPLY", n.math("ADD", rim, wash), n.math("MAXIMUM", breakup, 0.0))
    emissive(n, value, (1.0, 0.97, 0.9, 1.0), coverage_scale=0.8)
    return m


def dust():
    m, n = common.material("dust")
    coords = n.node("ShaderNodeTexCoord").outputs["Object"]
    noise = n.node(
        "ShaderNodeTexNoise", Scale=2.6, Detail=6.0, Roughness=0.65, Distortion=0.8
    )
    n.link(coords, noise.inputs["Vector"])
    length = n.node("ShaderNodeVectorMath", _operation="LENGTH")
    n.link(coords, length.inputs[0])
    rough = n.math("MULTIPLY", n.math("SUBTRACT", noise.outputs["Fac"], 0.5), 2.0)
    surface = n.math("ADD", n.math("SUBTRACT", 0.62, length.outputs["Value"]), n.math("MULTIPLY", rough, 0.5))
    shape = n.math("MULTIPLY", surface, 3.0, clamp=True)
    volume = n.node("ShaderNodeVolumePrincipled")
    n.link(n.math("MULTIPLY", n.math("POWER", shape, 2.5), 14.0), volume.inputs["Density"])
    volume.inputs["Color"].default_value = (0.85, 0.85, 0.85, 1.0)
    volume.inputs["Absorption Color"].default_value = (0.35, 0.35, 0.35, 1.0)
    out = n.node("ShaderNodeOutputMaterial")
    n.link(volume.outputs[0], out.inputs["Volume"])
    return m


def main():
    a = common.args()
    path = a[0]
    cell = int(a[1]) if len(a) > 1 else 256
    samples = int(os.environ.get("FX_SAMPLES", "64"))
    cells = []
    for make in [spark, flare, ring]:
        common.reset(cell, samples=max(samples // 4, 8), extent=1.0)
        common.plane(2.0, make())
        cells.append(common.render())
    common.reset(cell, samples=samples, extent=1.0)
    common.ambient(0.3, (0.8, 0.82, 0.88))
    common.sun((0.45, -1.0, -0.35), 4.5, (1.0, 0.96, 0.9))
    common.domain(1.0, dust())
    cells.append(common.render())
    # The glows peak at 1 by construction, and the dust is lit to about
    # white, so the sheet keeps unit gain: no cell sets another's level.
    common.write_sheet(path, cells, 2, gain=1.0)


main()
