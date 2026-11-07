# Character Create Nature glade — 2026-11-06

The user asked for the Nature kit's `Preview_1.jpg` recreated as a 3D scene
behind the character, including lighting and sky. They confirmed 3D rather
than a flat image and asked to inspect the newly unzipped Unreal project.

The exact pictured scene was not recovered. The Unreal map was decoded
read-only: it contains landscape/foliage/atmosphere/clouds and lighting, but
its two saved cameras are aerial and all 64 cached terrain bounds are flat.
The retained JSON binds each observation to the map digest and property
positions. No project scripts were run, no Unreal editor was launched, and
no conclusion of runtime equivalence is inferred from static names alone.

The implementation uses the registered Nature sources with Coax-authored
terrain, path, placements and palettes. It produces actual indexed geometry
with real depth and camera parallax. Coax supplies outdoor lighting, cyan
sky/clouds, wind, haze and contact shadow. Coral flowers, orange mushrooms,
green woodland and gold/red birches follow the reference palette.

| Boundary | Result |
| --- | --- |
| Pinned source → mesh compiler | Three focused glade checks pass; the reference/source revisions are pinned in the scene declaration |
| Compiler → pack | Independent oracle validates 283,657 static vertices, 211,047 triangles, every index/layer extent and finite position |
| Terrain → light | All 20,000 terrain faces have upward winding; a first-render orientation defect was corrected |
| Wardrobe preservation | Every original character vertex/index prefix, catalog declaration and texture payload is byte-identical |
| Pack budget | 33,368,512 bytes, below the existing 32 MiB limit |
| Shader → GPU | Sky and mobile-panel checks pass; WGSL is emitted from Coax and accepted by hardware Metal WebGPU |
| Scene → wardrobe renderer | Fourteen creator checks pass, including zero-joint static draw separation and saved-appearance compatibility |
| Physical browser | Glade presence, Female/Male Noble selection, eye color, cast animation, reload persistence, zoom, right-drag pan, left-drag rotation and phone layout pass |
| Actual environment | Headless scene reports ready with 633,141 environment indices |

The title/header and opaque appearance panels retain contrast over the bright
scene. The phone panel begins above its first BODY/CLOTHES row; it does not
leave those controls over the grass. `desktop.png`, `phone.png` and
`pan-close.png` retain the final physical captures. All browser runs were
headless and muted; each build/check ran within fifteen minutes.

The original shared asset-library verification at `0598997576` remains the
source inventory baseline. Those unchanged passing source/registry gates
were not repeated. Current geometry and runtime receipts are recorded in
`receipt.json`. This is a reference-directed recreation in Aethyr, not a
claim of pixel-identical Unreal rendering or a recovered original layout.
