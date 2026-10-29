# Everglade medieval refactor

Status: approved October 7, 2026. The owner's decisions are recorded in
[Decisions](#decisions). The phases below are tracked as GitHub issues
#10894 to #10902 (P1 to P9) under the umbrella #10903.

The owner bought the Fab pack "Modular Medieval Town", a modular kit of
walls, floors, roofs, doors, and props on a 2 m and 4 m grid, with a demo
town of 70 houses. This plan rebuilds Everglade's vernacular buildings
from that kit, keeps its landmarks, the Greco-futurism district, the world
tree, and the townsfolk, and delivers the kit to desktop, web, and phone
builds without putting any of it in this repository.

This page holds names, counts, and the plan only. It holds no geometry, no
texture, and no derived data from the pack.

## Licensing rules

The owner's ruling, October 7, 2026:

- The pack goes through the private pipeline. Meshes, textures, and any
  geometry derived from them never enter git: not as glTF, not as a pack,
  not as a baked image or a LOD.
- Shipping the compiled content inside game builds is allowed, including
  Everglade's web build.
- The private store is the bucket `openagentsgemini-verse-private-assets`,
  managed with `verse-private` and `~/.openagents/verse/private-assets.json`
  ([Private assets](private-assets.md)).
- Export scripts and tooling are committed, as long as they hold no asset
  content.

Two rules in [Private assets](private-assets.md) were written for private
characters and conflict with a kit that builds the town:

1. "Committed code holds no reference to a private asset: no name, digest,
   length, or position." A rebuilt town needs committed layout that names
   kit pieces and places them.
2. "A private pack never enters the Everglade or Grid packs, and the web
   build has no private path."

The owner approved a third class beside public packs and
private characters, the **licensed kit**: its pieces have committed IDs of
our own (such as `kit/wall-4x4-timber`), committed placements, and a
committed pack digest and length, while its bytes live only in the private
bucket and in builds. Private characters keep today's stricter rules. See
[Decisions](#decisions).

## The export tool

Unreal's own editor exports the pack; there is no Rust `.uasset` reader.

| File | What it does |
| --- | --- |
| [`scripts/unreal/medieval_town_export.py`](../../scripts/unreal/medieval_town_export.py) | The driver. Builds a scratch project under `~/.openagents/verse/private/medieval-town/ue/`, clones the vault copy's content into it (APFS clones, no extra disk), and runs Unreal headless with a clean environment. It refuses an output inside the repository, stops under 25 GB free, and fails if the source's files change during the run. |
| [`scripts/unreal/medieval_town_ue.py`](../../scripts/unreal/medieval_town_ue.py) | Runs inside `UnrealEditor -run=pythonscript -unattended -nullrhi`: no window opens and nothing renders. Writes one `.glb` per static mesh (and a `.lod0.glb` where Unreal renders a reduced LOD0), every texture as PNG (EXR for floating-point sources), `materials.json`, and `maps/<map>.json` with every placed mesh and its transform. It never saves a package. |
| [`scripts/unreal/medieval_town_archive.py`](../../scripts/unreal/medieval_town_archive.py) | Digests an export (`digests.json`), compares two exports file by file, and archives the vendor content and the digests in the private bucket under `vendor/medieval-town/`. |
| [`scripts/unreal/medieval_town_catalog.py`](../../scripts/unreal/medieval_town_catalog.py) | Writes `catalog.json`, `layout.json`, and `catalog.md` beside the export. |
| [`scripts/blender/medieval_kit.py`](../../scripts/blender/medieval_kit.py) | Rebuilds each material as Verse draws it (base color, tiling, tint) and renders a thumbnail per mesh. |
| [`scripts/unreal/medieval_town_contact_sheet.py`](../../scripts/unreal/medieval_town_contact_sheet.py) | Lays the thumbnails out as labeled contact sheets per category. |
| [`scripts/blender/medieval_piece.py`](../../scripts/blender/medieval_piece.py) | Converts one piece into a private pack build (the proof). |
| [`crates/verse-zone-everglade/examples/private_pack.rs`](../../crates/verse-zone-everglade/examples/private_pack.rs) | Compiles a private build into a `VTP3` pack locally, without uploading. |

Run it:

```sh
scripts/unreal/medieval_town_export.py
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/blender/medieval_kit.py -- thumbs \
  ~/.openagents/verse/private/medieval-town/export \
  ~/.openagents/verse/private/medieval-town/thumbs
scripts/unreal/medieval_town_contact_sheet.py \
  ~/.openagents/verse/private/medieval-town/export \
  ~/.openagents/verse/private/medieval-town/thumbs OUT_DIR
scripts/unreal/medieval_town_archive.py archive \
  ~/.openagents/verse/private/medieval-town/export
```

The driver normalizes its output so the export is deterministic: Unreal's
glTF writer leaves the padding between buffer views uninitialized, and the
town map names each Blueprint's dynamic material instance by a per-run ID.
After the driver zeroes the padding and drops the IDs, two runs of
October 7, 2026 matched in all 708 digested files, except the one merged
backdrop building that exports past the end of its buffer (see
[Pieces that exported badly](#pieces-that-exported-badly)).

A full export takes about 2 to 4 minutes on this Mac and writes 2.2 GB,
almost all of it textures. The editor needed three workarounds, all in the
scripts:

- `EditorLoadingAndSavingUtils.load_map` opens the map-check message log,
  which asserts under a commandlet when the check warns (the town map
  warns about painted vertex colors). The script runs the editor's own
  `MAP LOAD` command instead, then loads every World Partition actor.
- Naming the PNG exporter asserts on a floating-point texture. The script
  lets Unreal pick an exporter and falls back to EXR.
- Seven source models export as an empty glTF scene. The script retries
  from the render data.

## The kit

From the export of October 7, 2026.

| Measure | Count |
| --- | ---: |
| Static meshes | 285 (185 architecture, 81 props, 13 VFX, 5 demo, 1 backdrop) |
| Exported to glTF | 285; 7 through the render-data fallback |
| Meshes whose LOD0 Unreal renders reduced | 69 |
| Triangles, LOD0 as Unreal renders | 896,014 |
| Triangles, source models | 974,596 |
| Meshes with more than one LOD | 4 (the three trees and one flower bed) |
| Textures | 347 (341 PNG, 6 EXR: two sky HDRIs and four VFX noise maps) |
| Texture sizes | 211 at 2048, 56 at 1024, 41 at 4096, 25 at 512, 14 others |
| Materials | 216: 47 masters, 169 instances |

Per category (LOD0 triangles as Unreal renders them; "placed" counts
instances in the town map):

| Category | Meshes | Triangles | Largest | Placed |
| --- | ---: | ---: | ---: | ---: |
| Walls (A plaster and timber, B stone) | 38 | 10,252 | 2,336 | 1,288 |
| Wall trims (string courses) | 6 | 66 | 18 | 844 |
| Windows (4×4 wall with window) | 21 | 11,085 | 1,150 | 704 |
| Doors and doorways | 11 | 1,841 | 476 | 329 |
| Interior walls | 9 | 302 | 70 | 151 |
| Roofs | 21 | 6,552 | 2,080 | 762 |
| Floors and ceilings | 1 | 4 | 4 | 402 |
| Basements (plinths) | 4 | 128 | 54 | 938 |
| Stairs | 6 | 3,591 | 1,048 | 152 |
| Chimneys | 5 | 4,144 | 2,622 | 114 |
| Porches | 9 | 1,059 | 255 | 560 |
| Fences and columns | 12 | 2,033 | 304 | 184 |
| Arches, towers, castle | 12 | 33,518 | 11,410 | 27 |
| Merged buildings | 22 | 622,901 | 42,235 | 10 |
| Props: market (stalls, tents, barrels, baskets, carts) | 17 | 16,455 | 3,231 | 209 |
| Props: lighting | 6 | 5,793 | 2,976 | 190 |
| Props: furniture, street, fences, building parts | 31 | 8,612 | 2,154 | 881 |
| Props: fountains | 2 | 11,188 | 7,752 | 3 |
| Props: water wheel, anvil | 10 | 3,972 | 1,580 | 15 |
| Nature (trees, ivy, flowers, produce) | 15 | 114,707 | 37,915 | 287 |

The modular pieces are light: a 4 m wall is 24 to 140 triangles, a window
wall 166 to 1,150, a roof section about 250. Their detail is in tiling
textures and trim sheets, not geometry. Walls run 2, 2.5, 3, and 4 m and
stand 4 m tall; basements are 2 m tall; floors are 4 × 4 m. The roof
sections' pivot is the ridge, 6 m in from the eave, so a two-sided roof
spans a 12 m deep house. The trees are the exception: 27,000 to 38,000
triangles each, with four LODs.

Contact sheets of every mesh, rendered offscreen, are private:
`/private/tmp/claude-501/medieval/contact/contact-<category>.png` and
`contact-all.png`.

### The demo town

From `maps/Maps__medieval_town.json` (the night scenario places the same
town with more lights):

- 8,082 placements, 6,596 of them inside 70 Blueprint buildings: 66 houses
  of 14 types (31 of type 02, 11 of type 03, 10 of type 01), a tavern, a
  restaurant, a market, and a blacksmith. Plus 780 decals, 88 Niagara
  effects, and 22 cine cameras.
- The town's building centers spread over 239 × 231 m, the size of
  Everglade's built area.
- A house is about 100 pieces (median 100, range 49 to 195), with a
  footprint of 150 to 300 m² (median 223 m², about 13 × 17 m) and 14.7 to
  15.4 m tall. The tavern is 32 × 25 m and 30 m tall.
- Houses stand at free angles: 18 of 70 are square to the map's axes, the
  rest turned 10 to 80 degrees, so the streets curve. Inside a house every
  piece snaps to the 2 m and 4 m grid.
- Houses form 11 terraces of 2 to 11 houses and 16 free-standing ones. The
  open ground between neighboring blocks: median 3.8 m (lanes), 75th
  percentile 7.2 m, 90th 10 m (the main streets), and 41 m at most (the
  market square).

How the pieces make a house (type 01, 87 pieces):

| Course | Pieces |
| --- | --- |
| Plinth | 14 basement pieces, 2 m tall, with porch stairs to the door. |
| Ground and upper floor | 17 walls, 11 window walls, 2 doorways, each story 4 m. |
| Floors | 5 floor slabs, at the plinth top and between stories. |
| Trim | 12 string courses at the floor lines. |
| Porch | 9 porch parapets, roofs, and stairs. |
| Roof | 12 roof sections, ridge pieces, and gables, about 7 m tall over a 12 m depth. |
| Extras | 1 chimney, 2 shutters or balconies. |

A town of 70 houses places about 2.05 million triangles of architecture
and props and 1.15 million of trees. The architecture's 54 materials use
only 22 base-color textures (plaster, bricks, stone, two wood trims,
planks, roof shingle, roof top, doors, door frames, shutters, signboards,
pavement, and a few more); the props add 34.

## What changes in Everglade

Everglade's layout is in `crates/verse-zone-everglade/src/zones/everglade/layout*`.
Its city has 77 buildings on 8 m wide, 10 m deep lots, 74 of them drawn as
whole generated models (`STAND_INS`) and 5 built from the Quaternius
village kit, plus the first town's kit houses around the Commons.

### Rebuilt from the kit

| District | Buildings | Notes |
| --- | --- | --- |
| Main Street | The shops on both sides, including the bakery, café, grocer, bookshop, tea house, and hardware store. | Keep each building's name, lot, and door side. |
| Fountain Plaza and Market Row | The market hall, the two plaza cafés, Market Row east and west, the plaza fountain. | The market hall follows the demo's market composition; the kit fountain replaces the generated one at `PLAZA_FOUNTAIN`. Market stalls, tents, carts, and baskets dress the row. |
| Stoop Lane | The townhouses and homes 1 to 3. | The pilot street (P5): a terrace, which is how the kit works best. |
| Lantern Quarter | The Hearth, the Lantern, and their neighbors. | The tavern's stone base and timber upper stories. |
| The Foundry | The workshop, the fab hall, and the server barn annex. | The smithy keeps its generated model and open forge (see P7's "As built"). |
| The Commons' first town | The cottage, the four shops, the four homes, the two cabins, the reading room, the makers' hall, the server barn. | Today's Quaternius kit houses. |
| Creative and Knowledge districts | The college, lecture, fab, and atelier halls; the district's houses. | Stone `Walls_B` pieces with arched windows suit the halls. |
| Farm | Farmhouse and barn. | Optional, after P7. |

### Kept as they are

- The Greco-futurism district: the owner's house, the Civic Hall, the
  Agora, and the belvedere ([Greco-futurism](greco-futurism.md)). Their
  interiors, lights, Alice's spots, and the reception seat are untouched.
- The workshop hall, the strongroom, the yard, and the ten `STATIONS`,
  which never move.
- Landmarks with their own identity: the concrete tower, the
  observatory, the windmill, the chapel, the stacks, the bandshell, the
  boathouse, the glasshouse, and the gazebo.
- Brownstone Row, unless the owner wants it rebuilt (the kit has no
  brownstone).
- Terrain, water, Lantern Pond, foliage, Walden Woods, Fernhollow, the
  Gardens, and the Wilds. The kit's trees are too heavy for Everglade's
  budgets and stay out.

## Mapping the kit to buildings and breaking

### Lots, bays, and stories

The layout stays committed Rust. A rebuilt building is a **kit house**:
a lot (center, half extents, yaw), a door side, a story count, a style,
and a recipe that chooses pieces per bay. The lot sizes Everglade uses
fit the kit's grid: an 8 m front is two 4 m bays; a 10 m depth is
4 + 4 + 2 m. Stories are the kit's 4 m, against today's 3.12 m
(`WALL_TOP`), so two-story houses grow from about 9 m to about 13 m tall
with their roofs. A plinth sinks 1.25 m into the ground so 0.75 m shows
and the door is three steps up, rather than the demo's 2 m.

Everglade's lots are square to the street; P6 can turn some by up to 10
degrees for the demo's organic look, where the world tree and the nav grid
allow.

### Committed proxies, licensed skins

Everything the game simulates comes from committed proxies, not from the
licensed meshes:

- **Collision and walking**: each piece's footprint box and top, from its
  kit ID's committed dimensions (`solids.rs`, `Solids`), with stairs and
  porch steps as ramps under the controller's 0.35 m step.
- **Navigation**: the same boxes as nav blockers on the 2 m A* grid
  (`social/nav.rs`), which matches the kit's 2 m module.
- **Destruction**: one `PieceSpec` per kit piece.
- **World tree**: buildings, doors, and standing points from the lots.

The kit pack draws over the proxies. Without the pack, as in a
contributor's checkout, a public build, or a test, Everglade draws the
proxies with today's public materials, so every test and every build
works with no licensed content.

### The support graph

Kit pieces map one to one onto the demolition roles in
`demolition/site.rs`, rather than being carved:

| Kit piece | Role | Matter | Notes |
| --- | --- | --- | --- |
| Wall, window wall, doorway | `Wall { side, index, count, story }` | Plaster (A), Brick (B) | Window and door walls take today's weaker hit points (22 against 27). |
| Corner | `Post` | Timber | Carries the walls beside it. |
| Basement | `Block { level: 0 }` | Brick | Footing for the walls above. |
| Floor slab | `Block { level }` | Timber | Rests on the walls under it. |
| Roof section | `Roof { span, spans }` | Tile | `MAX_SPAN` 2 and four wall sections a span still hold. |
| Gable, ridge | `Gable` | Tile | |
| Chimney | `Chimney` | Brick | |
| Trim, shutter, porch roof, sign | Dressing | n/a | Falls with its wall, as `WALL_DRESSING` does now. |

The carve lattice (`CELL` 3.5 m, `LEVEL` 3.2 m) doesn't apply to kit
houses; their pieces are already the cells. Each piece needs chunks for
breaking: split each kit mesh on its 2 m grid in the private build and
ship the chunks in the kit pack.

Destruction stays lazy: a kit house draws as merged geometry until it is
first hit, then rises into a `Site` of its pieces, under the existing
limits (`MAX_LIVE` 12 and 6, `MAX_CHUNKS` 220).

### As built (P2 and P4)

What the code does, where it differs from the sketch above:

- **Pieces.** `everglade_pack::kit::PIECES` holds 53 kit pieces, each a
  `kit/<id>` model name, a committed box in the kit's own frame (meters,
  Y up, a wall running along +x with its outer face toward +z), a proxy
  shape, and a coat. `scripts/unreal/medieval_kit_recipe.json` maps each ID
  to its vendor mesh, with a mirror or scale where a piece needs one, and a
  test keeps the two lists equal.
- **The pack.** `scripts/unreal/medieval_kit_build.py` writes one glTF per
  piece from the private export, with tints baked into vertex colors,
  tiling into texture coordinates, and one primitive per image; the
  `everglade_kit` example compiles it with `compile::kit` under
  `Limits::KIT` (512 px images) into an 11.3 MB pack of about 25,000
  triangles. `kit::install` puts the kit's models into the decoded
  Everglade pack, or a proxy for each piece when the kit is absent or a
  model leaves its box by more than 0.1 m.
- **Houses.** `layout::kit_house` builds a house from its lot: corner
  pieces 1 m along each side, 4 m and 2 m bays between them, stories 4.5 m
  floor to floor (a 4 m wall and a 0.5 m band), a 2 m plinth sunk 1.25 m,
  steps to the door, and a gabled roof whose 6 m slopes span the side that
  is 10 m: from a ridge along the front on an 8 m by 10 m lot, or from a
  ridge front to back, gable to the street, on a 10 m front.
- **Collision.** Kit pieces are carved, one block each, so the town's
  existing demolition breaks them piece by piece and the pieces above fall
  when what holds them goes. A kit piece's collision columns come from its
  proxy, never from the licensed mesh, so walking is the same with and
  without the kit. Villagers and navigation route around
  `KitHouse::walls`, open at the door.

### Coplanar faces

The kit overlaps on purpose: trims sit on wall faces, corners overlap wall
ends, floors meet plinth tops. Run `scripts/blender/coplanar.py` over each
merged house type in the private build, with an allowance for faces the
kit offsets by a few millimeters, and fix the house recipes, not the
pieces.

## Materials, textures, and LODs

Verse samples base color only. Each kit material becomes one base-color
texture, its tiling, and a tint:

- **Tints become vertex colors.** Many instances share one texture and
  differ by tint (green, yellow, and white plaster; brown and green
  trims). The pack's RGBA8 vertex colors carry the tint, so the 54
  architecture materials collapse to 22, one per texture.
- **Tiling stays tiling.** The kit's walls tile plaster and brick across a
  piece; the format allows texture coordinates up to 256. Tiling textures
  aren't atlased.
- **Trim sheets stay sheets.** The wood, stone, and roof-top trims are
  already atlases.
- **Layered masters reduce to their base layer.** The kit's blend masters
  mix two layers by a height mask and vertex color; Verse takes the base
  layer. P3 compares the two in captures and may bake the blend into the
  texture where it shows.
- **Water** takes Everglade's own water, not a texture.

Texture budget per tier (architecture's 22 textures plus about 34 for
props):

| Tier | Edge | Architecture, decoded | All kit textures, decoded |
| --- | ---: | ---: | ---: |
| Desktop (High) | 1024 | 88 MiB | 224 MiB; keep props at 512 for about 115 MiB |
| Web (Medium) | 512 | 22 MiB | 56 MiB |
| Phone (Low and Medium) | 256 to 512 | 6 to 22 MiB | 14 to 56 MiB |

Levels per house type:

| Level | What | Triangles | Draws |
| --- | --- | ---: | ---: |
| Near (under 40 m) | Merged pieces, interiors that never show removed, one primitive per texture. | about 10,000 | 8 to 12 |
| Middle (40 to 80 m) | Unreal's reduced LOD0 (`.lod0.glb`) for the 69 pieces that have one, trims and dressing dropped. | about 3,000 | 3 to 5 |
| Far (over 80 m) | Decimated shell, base color baked into one atlas (`everglade_lod.py`). | about 800 | 1 |

The pack format holds at most 16 primitives a model, 384 models, and 64
textures; one primitive per texture keeps a merged house under 16.

## The pack pipeline

1. **Export.** `scripts/unreal/medieval_town_export.py` writes the export
   to `~/.openagents/verse/private/medieval-town/export/`.
2. **Archive.** `verse-private` uploads the vendor files under
   `vendor/medieval-town/` and the export's manifest of digests to the
   private bucket, as it does for a character.
3. **Build.** A new `verse-private kit build` runs the Blender steps
   (materials, merges, LODs, chunks, tier textures) and compiles a `VTP3`
   kit pack per tier with a new `compile::kit` entry under a new
   `Limits::KIT`, from a committed recipe that maps vendor names to our
   kit IDs. It uploads each pack to `packs/<sha256>.vtp`.
4. **Repin.** A new artifact, `artifacts/everglade-kit.json`, puts the kit
   pack through the [artifact queue](../coder/runtime/artifact-queue.md):
   `openagents artifact submit everglade-kit` regenerates the pack on this
   machine (which has the private export), checks it, and commits the new
   `KIT_SHA256` and `KIT_BYTES` pins. Only the digest lands in git.
5. **Serve.** The web image's Cloud Build step copies the pinned kit pack
   from the bucket into `/everglade/kit/<sha256>.vtp`; the wasm fetches it
   from the same origin under the pinned loader's rules. Desktop and phone
   builds fetch the same URL and cache it by digest, as they cache the
   Everglade pack today. The broker and its per-reader grants stay for
   private characters.
6. **Fallback.** With no kit pack (a local web build without bucket
   access, an offline first run, a test), Everglade draws the proxies.

The public Everglade pack is at 12,303,751 of its 12 MiB. Once the rebuilt
districts no longer place the Quaternius village walls and the generated
stand-ins they replace, P9 removes those from the public pack.

## World tree, villagers, rumors, and walking

The world tree (`crates/world-tree/data/everglade.json`, 338 nodes) keys
every node on `everglade/<district>/<slug(name)>`. The rebuild keeps every
building's name and district, so IDs survive:

- The townsfolk's places: Main Street's bakery, café, grocer, bookshop,
  tea house, and hardware store; the Fountain Plaza's market hall and
  fountain; the Foundry's smithy; Stoop Lane's homes 1 to 3; the reading
  room; the Hearth and the Lantern; and the chapel.
- The rumor `team-in-the-hall`, at the bakery.
- The IDs hard-coded in Rust and fixtures:
  `everglade/commons/workshop-hall/hall` and
  `everglade/knowledge-district/owners-house/...`.

What changes is where doors and standing points are. Each phase:

1. regenerates the snapshot with
   `WORLD_TREE_WRITE=1 cargo test -p verse-zone-everglade world_tree` and
   reviews that only standing points and door positions moved;
2. keeps every node routable from the approach (the existing test);
3. runs the villagers' day (Mira, Tobin, and Wren) and checks each
   routine reaches its places; the villager files don't change, so their
   digests and the owner's admission stay valid;
4. checks the nav grid has a path through each new door, given the door
   pieces' 1.6 m openings and the porch steps.

## Performance budgets

Today's budgets, which the rebuild must keep:

| Budget | Value |
| --- | ---: |
| Placed triangles (`PLACED_TRIANGLE_BUDGET`) | 1,650,000 |
| Drawn triangles per street frame | 600,000 |
| Resident bytes | 160 MiB (about 117 MiB used) |
| Street-frame draw calls | 780 to 1,260 today |
| Frame time, every tier | 16.67 ms |

The rebuild's share:

| Measure | Desktop | Web | Phone |
| --- | ---: | ---: | ---: |
| Rebuilt buildings | about 110 | same | same |
| Placed building triangles | at most 1.1 M (near levels) | same | same |
| Drawn building triangles per street frame | at most 320,000 | 240,000 | 160,000 |
| Building draw calls per street frame | at most 300 | 260 | 200 |
| Kit pack transfer | at most 24 MiB | 12 MiB | 8 MiB |
| Kit textures, decoded | at most 120 MiB | 56 MiB | 24 MiB |

Kit-style houses cost 2,845 batches today; merging per house type is what
brings the building share under 300.

## Phases

Each phase ends with offscreen captures from `everglade_capture`, with the
kit pack loaded and without it, saved outside the repository.

### P1: Export and archive

- Run the export, archive the vendor files and the export's digests in the
  private bucket.
- Acceptance: the export finishes with no failures beyond the known ones
  below; the source is unchanged; two runs give the same mesh and texture
  digests; `git status` shows no asset content.
- Captures: the contact sheets.

### P2: The kit pack

- A `kit` kind in the private manifest, `compile::kit` for static models
  with `Limits::KIT`, a loader that draws kit models through the textured
  scene's instancing and light bake, the pinned fetch, and the proxy
  fallback.
- Acceptance: tests with a synthetic kit pack (no licensed content) for
  limits, decoding, and fallback; a capture of five pieces drawn as static
  kit models, not as guests.
- Captures: the proof's views, redrawn through the kit path.

### P3: Materials, tints, and levels

- The committed recipe, vertex-color tints, tier textures, merged house
  types with near, middle, and far levels, chunks, and coplanar checks.
- Acceptance: every house type within the triangle and draw budgets above;
  no coplanar failures; a side-by-side of a merged house against the same
  house in Unreal's demo shows the same massing and palette.
- Captures: one house type at 10, 50, and 120 m.

### P4: Kit houses in the layout

- The lot, bay, and story recipe in Rust; committed proxies for collision,
  navigation, and the support graph; kit pieces in `PieceSpec`s.
- Acceptance: a test house breaks piece by piece and collapses when its
  walls go; `cargo test -p verse-zone-everglade` passes with and without
  the kit pack.
- Captures: one house standing, half broken, and collapsed.

### P5: Pilot street, Stoop Lane

- Rebuild Stoop Lane's terrace.
- Acceptance: world-tree IDs unchanged; homes 1 to 3 routable; the
  villagers' routines pass; the street frame within budget on desktop.
- Captures: `city-stoop`, day and night.

### P6: Main Street, Fountain Plaza, and Market Row

- The shops, the market hall, the cafés, the kit fountain, stalls, lamps.
- Acceptance: every townsfolk place on these streets keeps its ID; the
  rumor's node stays; the frame budgets hold.
- Captures: `city-market`, `town-north`, and a night view with lamps lit.

### P7: The rest of the vernacular

- The Lantern Quarter, the Foundry, the first town around the Commons,
  the Creative and Knowledge districts' halls and houses, and optionally
  the farm.
- Acceptance: as P6, for every rebuilt district; the Greco-futurism
  district and the workshop hall unchanged (their captures match).
- Captures: `city-lantern`, `city-foundry`, `approach`, `overhead`.
- As built (`19cc9a02c1`, #10900): Market Row, the Lantern Quarter's pubs
  and halls, Well Square, the Knowledge District's college, and the
  Foundry's and Creative District's workshops and halls are kit houses
  (`city::KIT_LOTS`). The round Music Hall, the smithy with its open
  forge, the two Boardwalk Cafés, the cabins in Walden Woods and the long
  meadow, the beekeeper's hut, and the farm keep their generated models
  (`city::STAND_INS`); their shapes (a round hall, an open forge, decks on
  the street) are not the kit's lot-and-bay houses, and the decisions keep
  the farm. Rebuilding any of them later changes the
  town's geometry, so it needs a new private pack (whole-house levels for
  the new recipe) and a new bake, because the published light layers bind
  to the exact scene digest (`pbr::baked_layers::scene_digest`).

### P8: Web and phone

- The web image's Cloud Build step, the phone's cached fetch, the tier
  textures.
- Acceptance: the web build at 60 frames per second on the reference
  laptop within its page-work budget; a phone within its tier's budgets;
  a web build without the kit falls back.
- Captures: the web page's Everglade, and the phone's.
- As built: `cloudbuild.yaml` copies the pinned kit pack from the private
  bucket into the image, and the site serves it at
  `/everglade/kit/<KIT_SHA256>.vtp`. The browser fetches it beside the
  Everglade pack and installs it if its length and digest match; the
  desktop and the phone fetch the same URL once and keep it in the zone
  cache by digest. Any failure draws the proxies. One pack serves every
  tier for now: its images are 512 px, 10.2 MB to transfer and about
  28 MiB decoded, within the web's budget but over the phone's 8 MiB
  transfer; a smaller phone tier is left for B4.

### P9: Cleanup

- Remove replaced Quaternius walls and stand-ins from the public pack;
  amend [Private assets](private-assets.md) for the licensed-kit class;
  update [Zones](zones.md) and [Everglade](everglade.md).
- Acceptance: the public pack's size drops; docs match the code.

### B1 to B4: the offline lighting bake

The owner added four phases on October 7, 2026, under the same umbrella.
They bake Everglade's static lighting offline, so no device bakes it at
zone load. No NVIDIA SDK code is copied or linked; techniques are
reimplemented from published papers.

- **B1 (#10905): the baker.** A `verse-bake` tool with a GPU backend
  (Vulkan ray queries) run on `coderos-4080` through
  `openagents lease run --class bench --place remote:coderos-4080`, and a
  multithreaded CPU backend that agrees with it within a stated tolerance.
  Deterministic by a fixed seed. Starts once P2 gives a compiled kit-town
  scene.
  Landed in `crates/verse-bake`. Everglade's current scene (4.3 million
  vertices, 2.4 million triangles, 128 rays a vertex, two bounces) bakes
  in 122 s on the 4080's GPU and 317 s on its CPU with 24 threads; the
  first CPU bake, on a Mac with 4 threads, took 851 s. The two backends
  agree within `GPU_TOLERANCE`: mean differences are under 0.0001, the
  99th percentile is under 0.001, and the largest are 0.18 for one
  vertex's ambient light, 0.06 for sun visibility, and 0.01 for probes.
- **B2 (#10906): lightmap layers in the kit pack.** Sky, sun at four
  positions, and lamps, as second-UV lightmaps and a probe grid, through
  the artifact queue, with today's load-time bake as the fallback. Needs
  B1; lands with or right after P5.
  First landed as per-vertex layers (`verse-bake --layers`,
  `pbr::baked_layers`): multi-bounce sky light, the bounce and visibility
  of four suns (8:00, 12:00, 15:30, and 17:30), the probes of both, and
  the shadowed, bounced light of every emissive triangle, clustered into
  333 lamps. The town's 4.2 million vertices bake in 25 minutes on a Mac's
  CPU with 4 threads, into a 45 MB layer file beside the kit pack, pinned
  by `KIT_BAKE_SHA256` through the `everglade-kit-bake` artifact. The zone
  uses the layers only when their scene digest matches the town it builds;
  the shader adds the lamp layer, which fades in at dusk. Second-UV
  lightmaps, a denser probe grid with lamps, and tiers are next.
- **B3 (#10907): time of day and destruction.** The town clock blends the
  sun layers and fades the lamp layer in at dusk; a broken piece and its
  neighbors fall back to dynamic light. Needs B2.
  Landed October 9: the town blends the two baked suns either side of the
  hour (`Layers::sun_weights`, continuous along the suns' order) at the
  sun's strength, and combines the layers again on a worker whenever a
  weight or the strength moves a sixty-fourth (`baked::STEP`, about 3.5
  times a game hour). Through the day the light channel's mean change
  between quarter hours stays at most 0.0098 (at dawn, from the strength),
  where the nearest sun alone jumped 0.038, 0.034, and 0.033 at 10:00,
  14:00, and 16:45. Combining takes about 24 ms on the worker; the frame
  that uploads the new light costs about 1.7 ms more. Desktops relight
  what breaks over the layers (`pbr::relight`, rebased when the hour's
  light changes): a meteor on Stoop Lane's first townhouse hid 17,800
  triangles and relit 151,958 vertices and 65 probes in 2.4 s on the
  worker, the baked shade under the fallen houses gone; `R` restores the
  layered light exactly. Phones keep their baked light. Measured with
  `everglade_bake_timelapse` (private kit; its frames stay private).
- **B4 (#10908): tiers and measurement.** Half and quarter resolution for
  web and phones, and the measured download, memory, frame, and load
  costs. Needs B2 and P8.
  As built (October 9): the layers are per vertex, so a tier drops suns
  rather than resolution. Desktops and the web take the 512 px kit and all
  four suns; the browser verifies the VLAY and never runs the stepped bake.
  Phones take the phone tier (`kit::Tier::Phone`): the pinned kit with
  every image halved to at most 256 px (`everglade_kit --phone`) and the
  layers with the 8:00 and 15:30 suns (`verse-bake --phone-layers`), both
  derived from the pinned files without a rebake and pinned through the
  `everglade-kit-phone` and `everglade-kit-bake-phone` queue entries. A
  phone falls back to the full files when the phone files are not served.
  The phone kit changes the scene digest (its images), so its Mac scene
  `61e2b61c…` joins the reviewed compatibility targets; geometry,
  materials, and vertex count are unchanged. `cloudbuild.yaml` copies both
  phone files into the web image from the private bucket.
  Measured on the M-series Mac with `everglade_tier_measure` (decode and
  zone load on the CPU; frame cost unchanged from B3's +1.7 ms upload):

  | | Desktop and web | Phone |
  | --- | ---: | ---: |
  | Kit transfer | 21,467,658 B (20.5 MiB) | 8,748,100 B (8.34 MiB) |
  | Kit images, decoded | 60.5 MiB, 512 px | 15.1 MiB, 256 px |
  | Layers transfer | 51,684,139 B (49.3 MiB) | 34,206,277 B (32.6 MiB) |
  | Layers, decoded | 91.5 MiB, 4 suns | 57.3 MiB, 2 suns |
  | Kit / layers decode | 106 ms / 251 ms | 43 ms / 153 ms |
  | Town light at load, with layers | 1.17 s | 2.48 s (includes the scene audit) |
  | Town light at load, load-time bake | 38.6 s | 36.6 s |

  The budgets a test pins (`each_tier_pins_its_files_within_budget`): kit
  24 MiB and layers 56 MiB for desktops and the web, kit 8.5 MiB and
  layers 40 MiB for phones. The phone kit is 4 percent over the plan's soft
  8 MiB; the next step down (128 px) was not taken. The web keeps the
  512 px kit: switching it to the 256 px pack changes the browser's scene
  digest, which needs its own audited target first. Phone frame rate,
  memory, and load time on a device are the owner's check
  (`NEEDS_OWNER.md`). Second-UV lightmaps and denser lamp probes remain
  deferred under umbrella #10903.

## The proof

On October 7, 2026, five pieces went from the export through the existing
private-character path into an offscreen Everglade capture:

| Piece | Kit mesh | Triangles | Pack |
| --- | --- | ---: | ---: |
| Wall | `SM_wall_A_4x4_02_02` | 108 | 1.6 MB |
| Door | `SM_wall_A_doorway_4x4_02_02` | 208 | 1.7 MB |
| Roof | `SM_roof_A_part_4_02_01` (source model) | 624 | 2.1 MB |
| Market stall | `SM_tent_02` | 888 | 1.9 MB |
| Fountain | `SM_fountain_01` | 7,752 near, 4,000 far | 1.0 MB |

`medieval_piece.py` baked each piece's tiled base colors into one 1024
image and gave it a still one-joint rig with an idle clip, because the
private path draws characters only today; the `private_pack` example
compiled each into a `VTP3` pack under `Limits::PRIVATE`. A test Verse
home outside the repository placed them: a wall and a doorway side by
side, the roof section four times (two pairs turned 180 degrees) to make
a gabled house front, the stall, and the fountain, east of the approach.
`everglade_capture` loaded them through `PrivateLoader` from the
content-addressed cache with `VERSE_CAPTURE_PRIVATE`, and the new
`VERSE_CAPTURE_PRIVATE_COUNT` waited for all eight placements.

Captures, private: `/private/tmp/claude-501/medieval/proof-facade.png`,
`proof-oblique.png`, `proof-closeup.png`, and `pieces-converted.png`.

What the proof showed:

- The path works end to end, and pieces placed by the kit's own pivots
  meet: the roof sections sit on the walls with no adjustment.
- Placing pieces as guests floats them on slopes: the fountain's rim
  stands clear of the paving on its downhill side. Kit lots need level
  pads (P4).
- The kit's realistic textures read a little darker and grittier than
  Everglade's painted look. P3 grades the base colors; the owner should
  judge it from P3's captures.
- One roof section with open gable ends shows the interior; real houses
  need the gable and ridge pieces.

## Pieces that exported badly

- Seven meshes export an empty scene from their source model and come out
  through the render-data fallback: `SM_fence_03_02`,
  `SM_wall_A_window_4x4_03_01`, `SM_wall_B_corner_03_01`, and four merged
  buildings.
- One merged building, `SM_MERGED_StaticMeshActor_UAID_00E04CB184546F3D01_2057930594`,
  exports from its render data with an index out of range in one
  primitive, and Blender refuses to import it. The merged buildings are
  the demo's distant backdrop and aren't used.
- 69 meshes render a reduced LOD0 in Unreal (a roof section is 252
  triangles rendered against 624 in its source). The export keeps both.
- Material baking needs a renderer, which `-nullrhi` lacks, so the glTF
  files carry material names only; textures and parameters come from the
  separate export.

## Decisions

The owner answered the plan's open questions on October 7, 2026:

1. **The licensed-kit class is approved.** Our own kit IDs, the
   placements, and the kit pack's digest and length are committed; the
   bytes live only in the private bucket
   (`openagentsgemini-verse-private-assets`) and in builds.
2. **Serving the kit pack from the web origin is approved.** The compiled
   kit pack is served from the OpenAgents web origin to every client: web,
   desktop, and phone.
3. **Brownstone Row and the farm are kept.** The kit has no brownstone, so
   Brownstone Row stays as it is. The farm stays as it is too; P7 rebuilds
   the farmhouse and barn only if the kit recipe makes it cheap and
   clearly better.
4. **4 m stories are approved.** Two-story kit houses stand about 4 m
   taller than today's.
5. **Streets.** The pilot street, Stoop Lane, uses the demo's turned,
   curving lots. The main streets (Main Street, Market Row, and the
   plaza) stay straight and readable.

The questions as the plan first asked them:

1. Approve the licensed-kit class: committed kit IDs, placements, and the
   kit pack's digest, with the bytes only in the bucket and builds.
2. Approve serving the compiled kit pack from the OpenAgents web origin to
   every client (web, desktop, phone), which makes it downloadable by any
   visitor, as any web game's assets are.
3. Choose whether Brownstone Row and the farm are rebuilt.
4. Approve 4 m stories, which make two-story houses about 4 m taller.
5. Choose straight streets or the demo's turned lots.
