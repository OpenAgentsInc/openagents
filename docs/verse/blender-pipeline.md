# Generated models with Blender

Status: proven, October 5, 2026; the plan below is proposed.

Verse's zones are built from CC0 kits, and the kits run out. The city map needs
landmarks no kit has, such as a fountain and an observatory. Wild Shape needs
animals. The demolition yard had to make its sledgehammer out of boxes in Rust.
Blender, driven by scripts, can fill that gap: agents write a Python script
that builds or converts a model, Blender runs it headless, and the glTF it
writes enters a zone pack like any kit model.

## What was proven

On October 5, with Blender 5.2.2 LTS on this Mac
(`/Applications/Blender.app/Contents/MacOS/Blender`):

- `scripts/blender/sledgehammer.py` builds a beveled iron-and-wood
  sledgehammer from code and writes binary glTF (18 KB).
- `scripts/blender/convert_fbx.py` converts the Spider from Quaternius's Easy
  Animated Enemy Pack (CC0, FBX only) to glTF with its armature and all five
  actions: Attack, Death, Idle, Jump, and Walk. The pack's materials import
  with alpha 0 and hashed blending, so they render invisible; the converter
  makes every material opaque.
- `scripts/blender/preview.py` renders a framed PNG of any glTF, so an agent
  can look at a model before admitting it.

Each runs headless:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/convert_fbx.py -- Spider.fbx spider.glb
$B -b --factory-startup --python scripts/blender/preview.py -- spider.glb spider.png
```

The scripts are asset-build tooling, an infrastructure exception like the
existing bake scripts. Product behavior stays in Rust.

## Scripts, not the MCP, for assets

[Blender MCP](https://pypi.org/project/blender-mcp) is a Blender add-on that
listens on `localhost:9876` and runs Python an MCP client sends it, with
Blender's window open. It's good for interactive work: the owner watches a
model take shape while an agent sends code.

A pack needs something else. Every committed model must be rebuilt from its
sources by anyone. So the rule is:

- **Committed assets come from scripts.** Each generated or converted model has
  a script under `scripts/blender/` that, given its pinned inputs, writes the
  glTF. The script is the model's source, reviewed like code.
- **The MCP is for sketching.** An agent may explore a shape interactively,
  then writes the script that reproduces it. Nothing reaches a pack from an
  interactive session.

## Where outputs go

- **Generated models** are written under
  `assets/verse/<zone>/generated/<name>.glb`. A `PROVENANCE` record names the
  script and its Blender version.
- **Converted models** keep their source license. The spider goes to
  `assets/verse/<zone>/beasts/` with the pack's CC0 notice and the source
  archive's name and digest.
- **Admission** is the existing path: the pack compiler reads admitted glTF,
  checks budgets, and the pack is repinned with the old digest retained.
- **Determinism:** Blender's glTF export isn't promised byte-identical across
  versions. Commit the generated `.glb` and treat the script as its recipe.
  CI-free rebuilds compare previews, not bytes. Rebuilding the pack itself
  remains #10622's concern.

## First models

| Model | How | For |
| --- | --- | --- |
| Giant Spider | Convert the CC0 FBX; scale up; rename actions to Verse's clip names | Wild Shape (#10610), Grove |
| Sledgehammer | `sledgehammer.py` | The demolition yard, replacing the Rust-built boxes |
| Fountain | Generated: stepped basin, column, bowls | Fountain Plaza on the city map |
| Observatory | Generated: drum tower, dome with slit, stair | Observatory Hill |
| Bandshell, boathouse, market stalls with awnings | Generated from simple solids with the kits' textures | The Commons, Lantern Quarter |
| Training dummy variants (armored, warded) | Kit dummy plus generated plates and runes | The Grove's dummy types |
| Stylized bear, wolf, eagle | Generated low-poly bodies on a simple rig, or a CC0 pack converted once found | Wild Shape, until real packs exist |
| Rat, frog, snake, wasp | Convert from the same CC0 pack | Ambient wildlife in Everglade's town and woods |

Generated buildings reuse the village kit's textures, so they sit with the
kit's pieces. Low-poly animals with a simple rig and a few keyed actions
(idle, walk, attack) are within reach of scripts. Detailed organic models are
not; those come from CC0 packs.

## Built models

On October 5, the first models were built and staged in
`assets/verse/generated/`, outside every zone pack until each is admitted.
`assets/verse/generated/PROVENANCE.md` records each model's script, triangle
count, clips, and, for the converted creatures, the source archive's digest
and its CC0 license.

| Model | Script | Clips |
| --- | --- | --- |
| Giant spider, rat, frog, snake, wasp | `enemy_pack.py` (with `convert_fbx.py`) | The pack's clips, renamed to `idle`, `walk`, `run`, `fly`, `attack`, `jump`, and `death` |
| Sledgehammer | `sledgehammer.py` | |
| Fountain | `fountain.py` | |
| Observatory | `observatory.py` | |
| Bandshell | `bandshell.py` | |
| Market stalls, red and blue | `market_stall.py` | |
| Training dummies: straw, armored, warded | `training_dummy.py` | |
| Bear, wolf, eagle | `animals.py` | `idle` and `walk`; the eagle has `idle` and `flap` |

The scripts share `scripts/blender/kit.py` for solids, materials, rigs, and
export. To rebuild every model and render the gallery, run:

```sh
scripts/blender/build-models.sh [GALLERY_DIR]
```

Set `GALLERY_ONLY=1` to render the gallery from the committed models alone.
`BLENDER` overrides Blender's path, and `ENEMY_PACK` overrides the enemy
pack's path (default: `~/Downloads`). The gallery (`gallery.py`) writes
`gallery.png`, a labeled grid of every model posed on its `idle` clip;
`previews/<name>.png`, one model each; and `contact_sheet.png`, the previews
in one image.

## Village buildings

`scripts/blender/buildings.py` assembles whole buildings from the Medieval
Village MegaKit's own pieces, with one function per building, and writes each
to `assets/verse/generated/buildings/` as a glb with a
`<name>.footprint.json` of collision boxes. `scripts/blender/building_views.py`
renders review sheets (`views`), footprint overlays (`boxes`), close-ups
(`look`), and an overview of a folder (`gallery`).
`assets/verse/generated/buildings/PROVENANCE.md` lists the pieces and textures.
`build-models.sh` rebuilds them with the other models, and `gallery.py`
includes them, scaled to fit its grid.

| Model | Triangles | What it is |
| --- | ---: | --- |
| `townhouse_jettied` | 13,266 | Three storeys, each jettied further over the street, front gable |
| `townhouse_balcony` | 12,893 | Two storeys and an attic, ridge along the street, balcony, two dormers |
| `row_townhouse` | 10,447 | Narrow three-storey row house for Brownstone Row, stone ground floor, blind party walls |
| `library` | 17,826 | Stone hall on a plinth, tall arched windows, gabled entrance bay up steps, clock turret, reading-room bay |
| `tavern` | 16,074 | Lantern Quarter tavern and music hall: wide jettied front, double doors, lanterns, hanging sign, dormers |
| `market_hall` | 15,264 | Open timber arcade on stone footings under a jettied hall with twin gables |
| `corner_shop` | 11,112 | Shop windows on two faces under striped awnings, hanging sign |
| `l_house` | 13,660 | L-shaped house: side-gabled block with a gabled wing to the street |
| `cottage_tower` | 7,805 | One-storey cottage with a round stone tower and cone roof |
| `music_hall` | 7,783 | Octagonal hall of tall arched windows under a tiled cone and a lit cupola |
| `meeting_hall` | 12,555 | Tall stone hall, broad gable, columned porch, bell-cote |
| `guild_hall` | 19,224 | Stone and jettied timber, guild banners, dormers, a round corner turret |
| `bakery` | 9,492 | Two storeys with a round brick bread oven and a tall stack |
| `boardwalk_cafe` | 7,281 | One-storey café under a striped awning on a railed deck |
| `smithy` | 6,184 | Stone smithy with an open lean-to forge and a glowing hearth |
| `clock_tower` | 10,269 | Stone clock tower, open belfry, slate spire, a small hall behind |
| `windmill` | 3,019 | Tapering tower mill with a tiled cap and four canvas sails |
| `farmhouse`, `cottage_thatch` | 5,486, 3,645 | Timber-framed houses under thatch |
| `log_cabin` | 1,086 | Round-log cabin with a porch and a stone chimney |
| `lookout` | 555 | Timber lookout tower on four legs |
| `boathouse` | 2,570 | Boathouse with its arch to the water |
| `greenhouse` | 790 | Glasshouse with see-through glass |
| `gazebo` | 378 | Open octagonal gazebo |
| `hip_house` | 10,770 | Two storeys under a hipped roof (`hip_roof`), shuttered windows |
| `gambrel_barn` | 374 | Red board barn under a gambrel roof, big doors, hayloft door, cupola |

## Admitted into Everglade

On October 5, the buildings, the observatory, the fountain, the bandshell,
and both market stalls were admitted into the Everglade pack's `generated`
set and placed in the city ([Everglade](everglade.md#layout)), with three
more kinds of model made for it:

- `street_props.py` builds the city's street furniture: a lamp post, a
  barrel, a flower box, a well, a dry-stone wall, a hedge, a hand cart, a
  signpost, lily pads, a footbridge, wildflowers, bunting, and two low-poly
  far-forest trees (`pine_low` and `oak_low`), each under 500 triangles.
  They are written to `assets/verse/generated/street/`.
- `kit_lod.py` writes a lighter copy of the village kit's
  `Roof_RoundTiles_8x10`, thinned to 55 percent of its triangles, to
  `assets/verse/generated/kit/`. Every kit-built house in the town wears it.
- `everglade_admit.py` converts each committed glb into the glTF and `.bin`
  the pack compiler reads, without touching its geometry, and points every
  texture at the village set's admitted image with a base-color factor
  that keeps the model's color. Plaster and roof tiles in new colors sample
  two neutral images it derives from the kit, `T_Plaster_Luma` and
  `T_RoundTiles_Luma`. It writes the set's manifest, which records each
  file's source glb digest and the conversion.

A second round on the same day brought the town closer to its map:

- `buildings.py` gained fifteen buildings the map names: the Music Hall,
  the meeting hall, the guild hall, the bakery, a Boardwalk Café, the
  smithy, the clock tower, the windmill, a thatched farmhouse and cottage,
  a log cabin, the lookout tower, the boathouse, the glasshouse, and a
  gazebo (`assets/verse/generated/buildings/PROVENANCE.md`). They use
  cheaper generated parts where the kit costs the most: octagonal and
  tapered drums, cone roofs, roofs of two textured slabs (`slab_roof`),
  thatch, and round logs. Their footprint JSON also gives the landing
  roofs, the walk's end outside the door, and a point past an open doorway,
  which `layout::generated` copies.
- `town_props.py` builds twenty-six small pieces for the parks, gardens,
  farm, ponds, and woods, from statues and a sundial to reeds, jetties,
  rowboats, mossy rocks, fallen logs, toadstools, and cheap birches,
  spruces, poplars, and fruit trees, each under about 1,000 triangles.
  They're written to `assets/verse/generated/town/`.
- The building scripts add five plaster colors (sage, sky, butter,
  terracotta, and lilac) and four roof colors (teal, charcoal, ochre, and
  plum), which the admission maps onto the neutral luma images.

A third round on the same day made room first, then added variety:

- The pack shrank from 28.0 MB to 8.5 MB ([Everglade](everglade.md#admission-and-the-zone-pack)):
  quantized, deflated geometry and textures at their on-screen size.
- `buildings.py` gained two roof shapes no kit piece gives: `hip_house`
  under a hipped roof (`hip_roof`) and `gambrel_barn` under a gambrel
  (`gambrel_slab`). Their footprints' landing roofs follow the hip's long
  slopes and the gambrel's shallow upper roof.
- `town_props.py` gained seasonal flower drifts
  (`flower_patch_spring`, `_summer`, and `_autumn`), `lantern_string`
  (paper lanterns on a cord between two posts), `lamp_double` (a two-armed
  lamp post), `shop_sign` (a painted sign on a post), and `park_bench`.
- `market_stall.py` gained green and gold stalls, and builds only the
  variants it is given by name.

The pack's forms hold the skinned, animated creatures: the Wild Shape
beasts and, since the fourth round, Everglade's ambient wildlife.
`wildlife.py` builds a songbird, a mallard, and a sitting cat from
primitives, and lighter copies of the enemy pack's rat, frog, snake, and
wasp (686 triangles or fewer, and only the clips the town plays);
`beasts_admit.py` admits them beside the beasts. `town_props.py` gained
`fruit_tree_bloom` and `cafe_umbrella`, and `fx/butterfly.py` renders the
butterflies' sprite sheet. The training dummies are the Grove's own
concern.

### Levels of detail

`everglade_lod.py` makes the far levels of detail Everglade draws beyond
60 m ([Everglade](everglade.md#rendering)). It reads each model in its
`RECIPES` from the admitted glTF and writes a lighter copy to
`assets/verse/everglade/lod/<set>.<name>.gltf`, with the source's own
materials and images:

- Buildings, landmarks, and kit pieces: welds the parts, drops loose parts
  too small to see at 60 m, dissolves nearly flat faces within each texture
  island, collapses the rest to 15 percent of the triangles for a generated
  building and 8 to 40 percent for a kit piece, and shades by angle.
- Trees and bushes: drops twigs, collapses the bark to 45 percent, and
  keeps half of the leaf cards, each grown about its center so the canopy
  keeps its cover.

The 53 far levels hold 45,981 triangles. To rebuild them, run:

```sh
$B -b --factory-startup --python scripts/blender/everglade_lod.py [-- MODEL...]
```

## How an agent makes a model

1. Write `scripts/blender/<name>.py` that builds the model from primitives,
   modifiers, and the kit's textures, at Verse's scale (1 unit = 1 m, +Y up
   after export).
2. Run it headless and render a preview with `preview.py`; look at the
   preview and iterate.
3. Admit the glTF into the zone's sources, run the pack tests, and capture the
   model in the zone with that zone's capture example.
4. Commit the script, the glTF, the provenance, and the repinned pack.

## Delivery

1. Convert the spider and admit it for Wild Shape (#10610).
2. Replace the demolition yard's sledgehammer with the generated one.
3. Generate the fountain and the observatory for the city map.
4. Generate stylized bear, wolf, and eagle for Wild Shape.
5. Optionally install Blender MCP for interactive sketching sessions with the
   owner.

## Open questions

- Whether Blender's path should be configurable (`BLENDER`) for other
  machines, including the Linux hosts.
- How many generated models a pack may carry before it needs its own budget.
