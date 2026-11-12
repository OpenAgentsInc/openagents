# Character Create's Nature glade

Character Create stands in a three-dimensional recreation of the supplied
Quaternius `Stylized Nature MegaKit[Source]/Preview_1.jpg`: a winding meadow
path between pale stepped rocks, green Common Trees and orange/gold birches,
with grass, ferns, flowering bushes, coral flowers and yellow mushrooms.

The terrain, plants and stones are 3D geometry with depth behind the character.
The sky is a directional far shell using a clean sky texture derived from the
supplied reference with ImageGen; it is not the original Unreal sky capture.
Camera movement, wind, warm sunlight, geometry shadows and contact shading
remain live. The original scene placement was not recovered. Visual matching
is authored against the reference and is not a claim of pixel equality.

Since #13018 the glade is a region of Everglade's world, south of the South
Gate, drawn by Everglade's renderer in Everglade's light, and Character Create is
Everglade's page started at its clearing (docs/verse/everglade.md, "The glade
south of the gate"). The zone pack, `everglade-zone.pack`, carries the glade
without its sky shell or its distant town: Everglade's sky and Everglade's real
town stand in for them. Build it with
`coax/web/tool.sh verse/tools/assets/character_glade_pack.coax ... zone`; the
sections below describe the glade's composition and its original standalone
renderer (`everglade/scene/gpu/character-glade.coax`), kept as the lighting
reference for the glade checks.

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
uses a warm key toward `(-0.45, 0.78, -0.42)` with cool ambient fill. The composition is authored in Coax rather than
mislabeling the overview map as the preview scene.

## Build

`verse/character-glade.coax` owns terrain, the path, deterministic planting,
placements and foliage palettes. `verse/tools/assets/character_glade_pack.coax`
reads the pinned source glTF files through the existing Coax reader and adds
one static environment draw, three source grass instance groups and eleven texture layers to the original wardrobe
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

`everglade/scene/gpu/character-glade.coax` owns the sky, environment vertex
shader, wind and surface lighting. `everglade/scene/gpu/fivelab.coax` selects these only for
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

## Current private profile (v4)

`character-create-legacy-static-v4` supersedes the procedural v2/v3 grass and
analytic shadow experiments retained in earlier verification folders.
The current pack retains all five Common Trees and all five birches. Pink
flowers use `Flower_3_Group`; `Flower_2_Group` supplies the purple patch.
The foreground fern is scaled by its visible alpha silhouette rather than its
mostly transparent export bounds. The hero cliff remixes `Rock_Medium_4` with
the checked `cg_cliff_cap_y` transform; the source remains immutable.

The three grass templates preserve the source positions, UVs and topology of
`Grass_Common_Short`, `Grass_Wide_Short` and `Grass_Wispy_Short`. One 32-byte
seed holds each placement, height, width, tint and rotation. Static indices
are ordinary vertex indices. A flag-17 draw's indirect first instance stores
(object slot + 1) in its high 16 bits and the seed ID in its low 16 bits.
Each group is bounded to 65,535 instances. Flag 16 remains the static scene;
neither path reads the wardrobe's skinning palette.

Leaf geometry keeps its authored card size, volume normals and a finite
0.5–1 ambient mask in word 7. The extra camera expansion has been removed:
the source glTF already contains expanded geometry. Birch bark encodes source
UVs over [-32,32], preserving negative repeating coordinates. Material kinds
are sky 0, leaves 1, source grass 2, flowers 3, rock 4, ground 5, bark 6 and
mushrooms 7. The former three wind-ribbon meshes and their unlit material
branch were removed after they read as blue slash artifacts. The independent
pack oracle now requires zero such markers.

Only Character Create loads `charactercreator.wgsl`. Its private graph uses
alpha-tested geometry shadows with PCF, normal/depth MRT, contact occlusion,
foliage reconstruction, bloom and FXAA. The common `fivelab.wgsl` retains its
single-target graph. Page assembly and content-hashed staging preserve the
explicit shader path. This grants no shared/native renderer qualification.

The pack stays below 32 MiB and preserves the original wardrobe payloads byte
for byte, with 64 total texture layers. Desktop uses 1024-square layers and
phone uses 512 through the existing optional layer-side host hook. Sky and
leaf atlas payloads retain 1024 resolution; other added images use 512.
The desktop texture array plus mips is approximately 341 MiB; phone is 85 MiB.
No new host JavaScript is needed.

The scene registry pins the reference, all actual source files, generated sky,
cliff transform, composition, shaders, packer and final runtime bytes.
Updated evidence lives in
`docs/verification/character-glade-source-parity-2026-11-06/`.

## Linear daylight and height atmosphere

The current private shader keeps linear HDR through surface shading and bloom,
then applies its shoulder and output transfer once. Warm direct illumination,
hemisphere fill, leaf transmission and height-dependent atmospheric scattering
have separate terms. Contact shading uses an indirect-light weight stored beside
an octahedral normal in the auxiliary target. The v4 static pack is unchanged.
See [the Ore lighting implementation notes](character-glade-lighting.md) for
the research, precise color contract, approximations and GPU readback evidence.

## Crown and grass reference pass

The current scene has 27 tree/rock placements. The missing orange birch behind
the foreground tree and two left-edge green trees are restored, using existing
pinned source assets. The central rock cluster has a darker authored tint and
a wider silhouette; the path includes a flattened source stone. Diffuse rock
color now uses each placement's tint rather than a shader constant.

Tree leaf normals follow eight deterministic spatial lobes fitted to each
source crown with farthest-point seeds and four refinement steps. All four
corners of a source leaf card choose the same lobe, avoiding seams through a
card. Geometry and atlas UVs stay authored; the pipeline validates complete
four-corner source cards. Leaf ambient masks remain within the v4 bounds.

Grass is taller and fuller with a lower-light root and transmitted-light tips.
The winding path is narrower. Foreground atmosphere stays clear for six metres;
the removed blue veil and a warmer grass palette better preserve the reference's
yellow-green foreground. Warm crown fill and red/orange/gold variation are
authored separately. The unlit sky plate has a scene-specific warm/cyan grade
(linear R power .62, G power .75, B scale .965) before final output encoding.

The source pack stays below 32 MiB with original wardrobe bytes preserved.
Evidence: `docs/verification/character-glade-crowns-2026-11-06/`.

## The road and the town

Past the clearing, the path becomes a road (`cg_road_x` in
`verse/character-glade.coax`). The road climbs a carved grade over the back
ridge to a plateau at z -46. Past the plateau, low far hills close the view.
On the plateau stands Everglade's town (`cg_city`): a stone wall with merlons,
two gate towers with a timber lintel, round towers with cones, a keep with a
slate roof, and ten houses with red tile gables. The gate is at `CG_GATE_Z`
(-34.5), and the walk ends three metres before it. The town and far ground are
built in `gp_road_to_everglade` from the existing rock texture, so the pack
adds no texture layers. The right foreground has seven orange and red-orange
Plant_1 and Plant_3 instances beside the near rock, and six pebbles along the
path, kept clear of the walk line, as in the reference. The pack is now
33,525,296 bytes (272,039 vertices).
That is still under the 32 MiB budget, and the wardrobe bytes are unchanged.

