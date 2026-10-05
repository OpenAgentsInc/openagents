"""The fireball flipbook: 16 frames of an explosion swelling from a white-hot
core into rolling orange fire and cooling to sooty smoke.

Run headless (or through `scripts/blender/build-fx.sh`):
    Blender -b --factory-startup --python scripts/blender/fx/fireball.py -- OUT.png [CELL]

A sphere holds a Principled Volume. Its density is a radial falloff broken
up by four-dimensional noise, so each frame's billows differ while staying
continuous; heat follows density and fades over the frames, and drives the
emission through a black-body-like ramp. Smoke density grows as the heat
falls. Cells are 128 px by default, a 4 by 4 sheet of 512 px.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import common  # noqa: E402

FRAMES = 16
COLUMNS = 4


def build(t):
    m, n = common.material("fireball")
    radius = 0.52 + 0.4 * common.ease_out(t)
    heat_level = 2.4 * (1.0 - t) ** 0.75
    coords = n.node("ShaderNodeTexCoord").outputs["Object"]
    # The fire rises as it burns, and its billows grow with it.
    lifted = n.node("ShaderNodeVectorMath", _operation="SUBTRACT")
    n.link(coords, lifted.inputs[0])
    lifted.inputs[1].default_value = (0.0, 0.25 * t, 0.0)
    scaled = n.node("ShaderNodeVectorMath", _operation="SCALE")
    n.link(lifted.outputs[0], scaled.inputs[0])
    scaled.inputs["Scale"].default_value = 1.0 / radius
    noise = n.node(
        "ShaderNodeTexNoise",
        _noise_dimensions="4D",
        W=1.3 * t + 3.0,
        Scale=1.35,
        Detail=6.0,
        Roughness=0.58,
        Distortion=0.6,
    )
    n.link(scaled.outputs[0], noise.inputs["Vector"])
    fine = n.node(
        "ShaderNodeTexNoise",
        _noise_dimensions="4D",
        W=2.1 * t + 11.0,
        Scale=5.5,
        Detail=4.0,
        Roughness=0.55,
    )
    n.link(scaled.outputs[0], fine.inputs["Vector"])
    length = n.node("ShaderNodeVectorMath", _operation="LENGTH")
    n.link(lifted.outputs[0], length.inputs[0])
    r = length.outputs["Value"]
    # Signed distance inside the ragged surface, in units of the radius.
    rough = n.math("MULTIPLY", n.math("SUBTRACT", noise.outputs["Fac"], 0.5), 1.7)
    inside = n.math("SUBTRACT", radius, r)
    surface = n.math("ADD", inside, n.math("MULTIPLY", rough, 0.95 * radius))
    shape = n.math("MULTIPLY", surface, 5.0 / radius, clamp=True)
    # Heat: deeper inside is hotter, the fine noise breaks it into tongues.
    core = n.math("MULTIPLY", n.math("MULTIPLY", surface, 2.0 / radius), n.math("SUBTRACT", n.math("MULTIPLY", fine.outputs["Fac"], 1.8), 0.35))
    heat = n.math("MULTIPLY", n.math("POWER", n.math("MAXIMUM", core, 0.0), 1.5), heat_level)
    ramp = n.node("ShaderNodeValToRGB")
    els = ramp.color_ramp.elements
    els[0].position, els[0].color = 0.0, (0.0, 0.0, 0.0, 1.0)
    els[1].position, els[1].color = 1.0, (1.0, 0.9, 0.62, 1.0)
    for pos, color in [
        (0.08, (0.25, 0.02, 0.004, 1.0)),
        (0.25, (0.8, 0.13, 0.015, 1.0)),
        (0.5, (1.0, 0.36, 0.04, 1.0)),
        (0.75, (1.0, 0.6, 0.14, 1.0)),
    ]:
        e = els.new(pos)
        e.color = color
    n.link(n.math("MINIMUM", heat, 1.0), ramp.inputs[0])
    volume = n.node("ShaderNodeVolumePrincipled")
    n.link(ramp.outputs[0], volume.inputs["Emission Color"])
    n.link(n.math("MULTIPLY", heat, 14.0), volume.inputs["Emission Strength"])
    n.link(n.math("MULTIPLY", shape, 1.5 + 5.0 * t * t), volume.inputs["Density"])
    volume.inputs["Color"].default_value = (0.09, 0.08, 0.075, 1.0)
    volume.inputs["Absorption Color"].default_value = (0.02, 0.02, 0.02, 1.0)
    out = n.node("ShaderNodeOutputMaterial")
    n.link(volume.outputs[0], out.inputs["Volume"])
    return m


def main():
    a = common.args()
    path = a[0]
    cell = int(a[1]) if len(a) > 1 else 128
    cells = []
    for i in range(FRAMES):
        t = i / (FRAMES - 1)
        common.reset(cell, samples=int(os.environ.get("FX_SAMPLES", "64")), extent=1.0)
        common.ambient(0.35, (0.9, 0.85, 0.8))
        common.sun((0.3, -1.0, -0.4), 2.0)
        common.domain(1.0, build(t))
        cells.append(common.render())
    common.write_sheet(path, cells, COLUMNS)


main()
