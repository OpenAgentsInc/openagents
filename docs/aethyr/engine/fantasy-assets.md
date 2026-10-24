# Fantasy Props and Haunted House assets

`tools/assets/compile_fantasy.py` prepares the actual 94-model Standard download.
Original inputs stay unchanged outside Git. The importer validates local paths,
buffer views, normalized colors, transforms, UVs, indices, and finite geometry.
Unsupported imports fail before the accepted pointer changes. Chest_Wood is
baked from the first frame of **Chest_Closed**; this profile has no runtime
skinning or chest-opening interaction.

```sh
python3 tools/assets/compile_fantasy.py \
  --source "$FANTASY_PROPS_SOURCE" --out "$AF_FANTASY_ASSETS" \
  --town "$MEDIEVAL_TOWN_KIT_BUILD"
python3 tools/assets/export_fantasy_record.py \
  --build "$AF_FANTASY_ASSETS" --out "$PUBLIC_METADATA_OUTPUT"
TMPDIR="$OPENAGENTS_SCRATCH" python3 -m unittest discover \
  -s tools/assets -p test_compile_fantasy.py
AF_CAPTURE_DIR="$PRIVATE_CAPTURE_DIR" \
  openagents browser run -- node tools/verify-fantasy.mjs
```

The accepted `fantasy-current.json` selects immutable catalog and review-scene
manifests. Every output has a size and SHA-256. Identical maps share prepared
resources by input content, role, and profile. The 512-pixel profile decodes base
color as sRGB and data maps as linear. Source ORM becomes engine MRA by swapping
red and blue. Use the ordinary glTF OpenGL normals, not the Unreal variants.
UV0, UV1, colors, material slots, double-sided flags, and source pivots survive
conversion. Geometry is meters, Y up. Colliders are explicit per-instance boxes
or disabled decorations; imported buildings do not gain semantic interiors.

The selected review set contains 24 kit props, five original spooky models,
and six licensed town modules. It has 37,936 vertices and about 9.3 MiB of
prepared resources. Limits are 40 assets, 500,000 vertices, two million indices,
32 MiB of geometry, 32 MiB of transfer, and 32 MiB of decoded textures including
mips. The job inventories all sources but prepares only the selected runtime
subset. Catalog and source hashes are metadata, not access grants.

Original custom models fill the werewolf, ghost, pumpkin, coffin, and gravestone
gaps. Match broad low-poly silhouettes, meter-scale proportions, muted warm fur
and wood, worn stone, orange rind, sparse painted detail, and rough surfaces.
Review them beside the kit under the same lighting. The static werewolf is
2.25 meters tall; `werewolf.face` has a dedicated material, continuous UVs, and
declared cheek targets. Editing it must create an object-specific material
instead of changing a kit atlas. These models are OpenAgents original project
content under project runtime distribution terms, with no CC0 dedication.
Quaternius source assets alone retain their CC0-1.0 rights record. Town modules
retain their separate licensed-runtime restrictions and existing color-only
kit-build profile. Captures, geometry, textures, and source files stay private.

## Walkable tutorial

Run `python3 tools/assets/compile_haunt.py --assets "$AF_FANTASY_ASSETS"`
after catalog preparation. The accepted house profile selects immutable scene,
project, lighting, and asset manifests. It keeps Lantern Hall, the passage, and
the gallery, five pumpkin IDs, and the original entrance. Its mapping records
all 100 structure and prop instances, material IDs, rights, and collision policy.
Tiled kit floors also close the ceiling; doors and windows are intentional
openings. Triangle bounds preserve structural doorway holes. Decorative props
use bounded collision or explicitly disable it. The werewolf's face remains a
separate UV target.

Build with `VITE_WORLD_PROFILE=haunted-house`. This selects independent local
storage and a walking camera; it cannot open the town's checkpoints. The guest
house reports unsaved until the public persistence service records a save.
Existing first-version saves pass through the public migration adapter; this
profile does not read or discard them. Structural editing remains disabled.

Run `AETHYRFORGE_PRIVATE_ASSETS="$AF_FANTASY_ASSETS" AF_CAPTURE_DIR="$PRIVATE_CAPTURE_DIR"
openagents browser run -- node tools/verify-haunt.mjs` to check real doorway
walking, wall collision, stable selection, failed-shader continuity, and missing
content recovery. The private captures complement public host qualification;
they do not establish account persistence, publishing, or multiplayer behavior.

## Open Ground haunted starter

The grass-world starter follows the asset direction in OpenAgents
[#12562](https://github.com/OpenAgentsInc/openagents/issues/12562), especially
[#12547](https://github.com/OpenAgentsInc/openagents/issues/12547),
[#12548](https://github.com/OpenAgentsInc/openagents/issues/12548), and
[#12552](https://github.com/OpenAgentsInc/openagents/issues/12552). The Standard
download contains props, not house walls, doors, floors, or roofs. The existing
Lantern Hall → passage → gallery tutorial and licensed town architecture remain
the reference for the later house shell.

Eight real kit models start a small witch's clearing 5–12 meters ahead of the
Paladin spawn. They retain their meter-scale proportions, material slots, UVs,
vertex colors, double-sided materials, and OpenGL normal maps. Nine shared
512-pixel textures include base color, normal, and correctly swizzled ORM maps.
The selected geometry adds 22,552 vertices and 22,756 triangles; prepared new
resource transfer is about 4.18 MB. Chest_Wood uses its closed static pose.

| Model | Starter object | Position X/Z in meters |
| --- | --- | --- |
| `Cauldron` | `haunt-cauldron` | −10 / 8 |
| `BookStand` | `haunt-bookstand` | −7.5 / 8.5 |
| `Book_Stack_2` | `haunt-books` | −7 / 7.5 |
| `Bookcase_2` | `haunt-bookcase` | −7.5 / 11.5 |
| `CandleStick_Stand` | `haunt-candles` | −11.8 / 8 |
| `Cage_Small` | `haunt-cage` | −13.5 / 10.5 |
| `Chain_Coil` | `haunt-chain` | −12.5 / 10 |
| `Chest_Wood` | `haunt-chest` | −10.5 / 12 |

The importer samples the same terrain height as the game, appends meshes and
materials, and seeds editable objects rather than baking them into scenery.
A six-meter grass clearing makes the props visible; grass elsewhere, the cloud
sky, source scenery, and the shared game identity stay unchanged. The importer
writes to a new private output directory and verifies its license and inputs.

```sh
python3 tools/assets/compile_ground_props.py \
  --kit "$FANTASY_PROPS_SOURCE" --base "$AETHYRFORGE_MMORPG_OUTPUT" \
  --output "$AETHYRFORGE_HAUNT_GROUND"
AETHYRFORGE_PRIVATE_ASSETS="$AETHYRFORGE_HAUNT_GROUND" npm run dev:ground
```

Enter the game, choose **Edit**, and use **Assets**, **Select**, or **Command**.
Click a placed prop to select it; move, resize, remove, undo, or redo through the
same typed transaction path. Examples include “add a cauldron to my left” and
“make it bigger.” **Done** returns to play with the same player and camera.
Edits save on this device under the starter's own identity and restore after
reload. Public and published players do not receive editing access.

This first step edits decorative props. It does not add a walkable house shell,
structural walls, shared building edits, or shared building collision. The props
are explicitly noncolliding; the MMO server still owns characters, terrain, and
tree collision. A later house must connect its complete visual and collision
generation to that authority before claiming shared interiors.

Run `tools/verify-ground-props.mjs` through `openagents browser run`, setting
`AF_CAPTURE_DIR` outside Git. It checks actual GPU rendering, pointer picking,
edits, undo/redo, pose preservation, saved reload, and player access restrictions.

Verification: all seven browser checks pass with no JavaScript errors or failed
requests. The nine-object edited draft restores exact IDs, transforms, and
noncolliding policy after a real reload/new anonymous visit. The same prepared
world passes melee, magic, enemy damage, death, and personal respawn. Its
15.75-second moving sample submitted 947 frames (60.14 FPS) at 1,280 × 720 on
the measured Mac. Repository checks pass: 1,221 TypeScript tests, two skipped;
production kernel/typecheck/Vite build; 16 Python importer tests; and WGSL
validation for both grass modules. These measurements do not qualify phone or
production performance.

The final seven-check edit run also passed on the user-facing local server at
port 55316, including an explicit saved-state check after reload. Private
evidence is `ground-props-browser-readable/ground-props-browser.json`, including
all eight asset names fitting their cards and the Add button remaining visible.
The complete
combat/lifecycle run is `mmorpg-final-haunt-lifecycle/mmorpg-world-browser.json`.

## Download inventory

The inspected `Fantasy Props MegaKit[Standard] 2` contains 94 model names, each
with glTF/BIN, FBX, and OBJ/MTL exports. Its 517 files total 162,871,281 bytes;
this is the source download, not the browser payload. The license is CC0 1.0;
the pinned license hash remains in `compile_fantasy.py`.

- Books and study: `BookGroup_Medium_1`, `BookGroup_Medium_2`,
  `BookGroup_Medium_3`, `BookGroup_Small_1`, `BookGroup_Small_2`,
  `BookGroup_Small_3`, `BookStand`, `Book_5`, `Book_7`,
  `Book_Simplified_Single`, `Book_Stack_1`, `Book_Stack_2`, `Bookcase_2`,
  `Scroll_1`, `Scroll_2`.
- Light and alchemy: `CandleStick`, `CandleStick_Stand`, `CandleStick_Triple`,
  `Candle_1`, `Candle_2`, `Cauldron`, `Chandelier`, `Lantern_Wall`,
  `Potion_1`, `Potion_2`, `Potion_4`, `SmallBottle`, `SmallBottles_1`,
  `Torch_Metal`.
- Furniture and storage: `Bed_Twin1`, `Bed_Twin2`, `Bench`, `Cabinet`,
  `Chair_1`, `Chest_Wood`, `Crate_Metal`, `Crate_Wooden`, `Nightstand_Shelf`,
  `Peg_Rack`, `Shelf_Arch`, `Shelf_Simple`, `Shelf_Small_Bottles`, `Stool`,
  `Table_Large`, `Workbench`, `Workbench_Drawers`.
- Workshop and equipment: `Anvil`, `Anvil_Log`, `Axe_Bronze`, `Dummy`,
  `Pickaxe_Bronze`, `Shield_Wooden`, `Sword_Bronze`, `WeaponStand`,
  `Whetstone`.
- Food, vessels, and market: `Barrel`, `Barrel_Apples`, `Barrel_Holder`,
  `Bottle_1`, `Bucket_Metal`, `Bucket_Wooden_1`, `Carrot`, `Chalice`,
  `FarmCrate_Apple`, `FarmCrate_Carrot`, `FarmCrate_Empty`, `Mug`, `Pot_1`,
  `Pot_1_Lid`, `Stall_Cart_Empty`, `Stall_Empty`, `Table_Fork`, `Table_Knife`,
  `Table_Plate`, `Table_Spoon`, `Vase_2`, `Vase_4`, `Vase_Rubble_Medium`.
- Dressing and valuables: `Bag`, `Banner_1`, `Banner_1_Cloth`, `Banner_2`,
  `Banner_2_Cloth`, `Cage_Small`, `Chain_Coil`, `Coin`, `Coin_Pile`,
  `Coin_Pile_2`, `Key_Gold`, `Key_Metal`, `Pouch_Large`, `Rope_1`, `Rope_2`,
  `Rope_3`.
