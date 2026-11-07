# Source mesh and lighting pass

The v4 scene replaces procedural grass with exact source mesh instancing,
restores repeating birch UVs, uses the painted pink/purple flower assets and
corrects the visible fern scale. The hero cliff retains its source parent and
an explicit Coax remix. The sky is an ImageGen-derived clean sky plate from the
reference, mapped directionally behind the 3D scene. It is not an original
Unreal scene recovery.

`scene.png` is the 1920×1080 comparison view. The other captures exercise the
normal character view, appearance reload, pan, male preview and phone layout.
`pack-oracle.json` independently checks source grass topology/positions/UVs,
finite packing and original wardrobe preservation. GPU receipts observe the
actual texture allocations and pipeline graph, not just scalar shader tests.
Frame timings are 24 headless RAF intervals; they are not a GPU benchmark.
Shader staging is tested offline without publishing a deployment.

The recorded image still has visible differences from the reference in crown
shape, plant distribution and material response. No numeric or visual pixel
parity claim is made. The original map was not recovered, so exact placement
and Unreal material behavior remain unavailable in this reconstruction.
