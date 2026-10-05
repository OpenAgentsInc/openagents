"""The smoke flipbook: 16 frames of a soft puff billowing out and thinning.

Run headless (or through `scripts/blender/build-fx.sh`):
    Blender -b --factory-startup --python scripts/blender/fx/smoke.py -- OUT.png [CELL]

A sphere holds a pale scattering volume whose density is a radial falloff
broken up by four-dimensional noise. A sun from above and a soft grey sky
light it, so the puff carries its own shading: bright crowns, shadowed
undersides. The particle's color curve tints it (soot, dust, steam), and
its alpha curve fades it. Frames 0 to 15 run from a tight young puff to a
wide, thin one.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import common  # noqa: E402

FRAMES = 16
COLUMNS = 4


def build(t):
    m, n = common.material("smoke")
    radius = 0.55 + 0.37 * common.ease_out(t)
    coords = n.node("ShaderNodeTexCoord").outputs["Object"]
    scaled = n.node("ShaderNodeVectorMath", _operation="SCALE")
    n.link(coords, scaled.inputs[0])
    scaled.inputs["Scale"].default_value = 1.0 / radius
    noise = n.node(
        "ShaderNodeTexNoise",
        _noise_dimensions="4D",
        W=0.9 * t + 5.0,
        Scale=1.1,
        Detail=5.0,
        Roughness=0.55,
        Distortion=0.5,
    )
    n.link(scaled.outputs[0], noise.inputs["Vector"])
    length = n.node("ShaderNodeVectorMath", _operation="LENGTH")
    n.link(coords, length.inputs[0])
    rough = n.math("MULTIPLY", n.math("SUBTRACT", noise.outputs["Fac"], 0.5), 1.6)
    inside = n.math("SUBTRACT", radius, length.outputs["Value"])
    surface = n.math("ADD", inside, n.math("MULTIPLY", rough, 0.8 * radius))
    shape = n.math("MULTIPLY", surface, 3.0 / radius, clamp=True)
    # Thinner as it spreads, and softer at the end.
    thickness = 30.0 * (1.0 - 0.75 * t)
    volume = n.node("ShaderNodeVolumePrincipled")
    n.link(n.math("MULTIPLY", n.math("POWER", shape, 2.0), thickness), volume.inputs["Density"])
    volume.inputs["Color"].default_value = (0.82, 0.82, 0.82, 1.0)
    volume.inputs["Absorption Color"].default_value = (0.3, 0.3, 0.3, 1.0)
    volume.inputs["Anisotropy"].default_value = 0.2
    out = n.node("ShaderNodeOutputMaterial")
    n.link(volume.outputs[0], out.inputs["Volume"])
    return m


def main():
    a = common.args()
    path = a[0]
    cell = int(a[1]) if len(a) > 1 else 128
    samples = int(os.environ.get("FX_SAMPLES", "64"))
    cells = []
    for i in range(FRAMES):
        t = i / (FRAMES - 1)
        common.reset(cell, samples=samples, extent=1.0)
        common.ambient(0.3, (0.75, 0.8, 0.9))
        common.sun((0.45, -1.0, -0.35), 4.5, (1.0, 0.96, 0.9))
        common.domain(1.0, build(t))
        cells.append(common.render())
    # One gain for every frame keeps the shading comparable; a fixed one
    # keeps a lit crown just under white.
    common.write_sheet(path, cells, COLUMNS)


main()
