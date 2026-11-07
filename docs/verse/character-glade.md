# Character Create's Nature glade

Character Create stands in a three-dimensional recreation of the supplied
Quaternius `Stylized Nature MegaKit[Source]/Preview_1.jpg`: a winding meadow
path between pale stepped rocks, green Common Trees and orange/gold birches,
with grass, ferns, flowering bushes, coral flowers and yellow mushrooms.

The reference is a composition target, not a background image. Geometry writes
real depth behind the character and responds to camera pitch, zoom and pan.
Coax shaders animate foliage and grass with wind and render the cyan sky,
layered white clouds, cirrus streaks, warm key light, leaf transmission,
distance haze and a ground contact shadow. The mesh lighting and cloud model
are an Aethyr interpretation; this does not claim pixel-identical reproduction
of the original Unreal rendering.

## Source inspection

The owner also unzipped `Engine Projects/Stylized Nature MegaKit`, an Unreal
4.27 project. Its only map is `L_Overview.umap`. Static package decoding found
landscape, foliage, atmosphere, volumetric clouds and real lighting settings.
Both saved cameras are aerial (104.63/108.97 m, pitches -45.60/-48.79 degrees),
and all 64 cached landscape component bounds have zero local height range.
This did not recover the ground-level, hilly reference composition. The
Godot and Unity project archives also supplied overview layouts. The packaged
project scripts were not executed and Unreal was not launched.

The retained evidence includes the Unreal map's SHA-256, table offsets,
component properties and source mesh/material links. Its directional-light
properties are retained as source evidence. The reference-directed lighting
uses a warm key toward `(-0.45, 0.78, 0.42)` with cool ambient fill. The composition is authored in Coax rather than
mislabeling the overview map as the preview scene.

## Build

`verse/character-glade.coax` owns terrain, the path, deterministic planting,
placements and foliage palettes. `verse/tools/assets/character_glade_pack.coax`
reads the pinned source glTF files through the existing Coax reader and adds
one static environment draw plus ten texture layers to the original wardrobe
pack. It preserves the original character payloads byte for byte.

```sh
SOURCE="$(coax/web/tool.sh verse/tools/assets/registry.coax source nature "$HOME/.openagents/assets")"
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax \
  "$SOURCE" apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/character-creator/assets/glade.pack
```

The combined pack stays within the existing 32 MiB budget. Its environment
uses 32-byte static vertices (position F32×3, packed normal/kind, UV, color,
texture layer, canopy ambient mask in v2/v3), separate from the unchanged 24-byte character
vertices. Its `C glade/environment` catalog row has zero joints. A scene
record with flag 16 selects the static vertex path; appearance markers remain
on character records only. Static vertices never read the animation palette.
The same indexed people pipeline performs this extra draw; no host-language
scene logic, new browser ABI or new rendering backend was added.

`apps/openagents-web/gpu/character-glade.coax` owns the sky, environment vertex
shader, wind and surface lighting. `gpu/fivelab.coax` selects these only for
Character Create (`u[10] == 2`); battle views retain their own scenery. Desktop
side panels and the entire mobile appearance panel retain opaque, depth-masked
backgrounds. The header fades the scene to keep its title readable.

The shared scene declaration in `assets/registry/scenes/character-glade.json`
pins each Nature asset revision, reference image, composition source and built
pack. It can be reused or edited alongside the registered Blender assets.

## Checks

* Focused Coax checks cover terrain/path behavior, placements, static position
  encoding, sky bounds and mobile panel coverage.
* Character checks cover the new static draw, catalog reset, record separation
  and existing save compatibility.
* `character_glade_oracle.py` independently validates the pack, all static
  vertex/index/layer extents, finite positions, every terrain triangle's upward
  winding, exact catalog size and byte-identical original character payloads.
* Headless Chrome checks real controls, appearance persistence, the visible
  environment, camera movement and desktop/phone layouts.

The detailed source investigation, screenshots and measured receipts are in
`docs/verification/character-glade-2026-11-06/`. These are legacy Character
Create observations, not a new shared/native renderer qualification.

## Softer foliage and reference lighting

The v2 scene uses short, curved five-vertex ribbons with three fine strands
shaped in the fragment shader. Blades range from 6 to 16 cm high. Screen-space
edge smoothing removes the hard triangular silhouettes. All derivative work
happens at the uniform fragment entry before material selection; the hardware
shader check verifies this separately from scalar Coax tests.

Leaf normals follow each crown's volume, with a per-vertex ambient mask and
warm transmission at its edges. Two-sided foliage retains that volume normal
rather than alternating between bright and dark polygon faces. Crown/rock
proxies cast soft analytic shadows onto the ground and small plants. These
are authored approximations, not a general shadow-map renderer.

The sky uses soft white cloud banks and angled cirrus. The title's darkening
is confined to its own area instead of tinting the whole sky. Two fully hidden
trees behind the left cliff are omitted to keep the denser curved grass within
the existing pack budget. See `character-glade-soft-2026-11-06` in verification.


## Reference composition pass (v3)

The camera now puts the horizon near 60% of the image height, preserving
close face inspection. `?scene=1` is a transient comparison view that hides
the character and UI; it never enters an appearance save. The 1920×1080 scene
capture uses the same camera and geometry as the normal desktop view.

The rear ridge now turns golden independently of the green foreground. The
path bends across it, with loose pebbles, terraced rocks, smaller foreground
flowers, a fern and an orange plant. Warm crowns retain brighter ambient fill
and yellow transmission. Cloud banks are smaller, with restrained thin cirrus.

Grass now stores one seed per tuft and generates its five curved corners in
the vertex shader. Bit 31 marks a procedural index, bits 27–29 select a corner,
and the lower 27 bits select a 32-byte seed. Its UV word stores height/width;
word 7 stores angle. The pack contains 26,911 tufts with patch-dependent
heights of 2.9–35.7 cm, within the original 32 MiB limit. Shader coverage blends
edges toward the meadow color, reducing hard ribbons in distant planting.

The source Godot `M_Leaves.gdshader` also contains camera-oriented card
expansion. Raw glTF exports already retain expanded leaf geometry, so v3
applies a restrained version only to tree crowns. Leaf kind occupies the low
three tag bits; the other five encode expansion scale in tenths of a meter.
Small plants keep their authored geometry and atlas UVs.

This pass improves the composition but does not establish exact image parity.
The original scene placement was not recovered; analytic shadows and procedural
clouds still differ from the supplied rendering. Evidence is retained under
`docs/verification/character-glade-parity-2026-11-06/`.
