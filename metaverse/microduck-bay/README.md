# Edit bay workbench: Microducks in the Splice Bots bay

A standalone local world on the OpenAgents Metaverse Engine (GitHub #13652, first
demo). It opens the Splice Bots jam's **edit bay** (the garage bay with the
crafting bench) on its own and lets you spawn and drive Microducks there.

- **Spawn**: 1, 10, 20 or 30 Microducks, or drag the slider (0 to 30).
- **Motion**: **Wander** (each duck picks its own way over the open floor),
  **Line** (each duck follows the one ahead), **Stop** and **Reset** (the ducks
  stand again in front of the bench, and the stock goes back to its start).
- **Craft at bench** makes one Microduck by the game's recipe,
  `RECIPE_MICRODUCK`: 1 battery and 3 scrap parts. The duck steps out of the
  bench's front. The inventory shows what is left; **Refill** adds one more
  recipe of parts, up to each item's stack size. You start with two recipes'
  worth, as on the game's farm.
- The bay holds 40 Microducks at most: 30 from spawning, and the bench adds up
  to 40. The panel shows the count against that limit.

Drag to orbit, scroll to zoom, **1 / 2 / 3** for the wide, bench and overhead
views. Keys: **W** wander, **L** line, **S** stop, **C** craft, **F** refill,
**R** reset. The page's menu has every action too.

## Run it

The bay's models are the owner's private Unreal project and never enter git.
The pack tool reads the editor route's import bundle of that project
(`~/.openagents/unreal/splice-bots-jam/editor.aimport`, written by
`metaverse-engine/tools/unreal-level-export.coax`) and the project itself.
Build the private pack once (and again when the project changes), then build
the page and serve it:

```sh
coax/web/tool.sh metaverse/tools/assets/microduck-pack.coax --fetch /tmp/microduck.pack
coax/web/tool.sh metaverse/tools/assets/microduck-bay-pack.coax /tmp/microduck.pack ~/.openagents/unreal/splice-bots-jam/editor.aimport ~/games/SPLICEBOTS ~/.openagents/verse/private-build/web/microduck-bay.pack
coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/microduck-bay.coax apps/openagents-web/microduck-bay.wgsl apps/openagents-web/gpu/microduck-bay.module
OA_SERVE_NO_API=1 OA_SERVE_NO_CODER=1 BROWSER=/usr/bin/true coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9147 --build /usr/bin/true &
# after serve.sh starts (it clears apps/openagents-web/build/): the page and the host's pack and image readers
coax/web/wasm.sh apps/openagents-web/microduck-bay.coax apps/openagents-web/microduck-bay-face.c apps/openagents-web/build/microduck-bay.wasm
coax/web/tool.sh coax/web/build-packs.coax . apps/openagents-web/build/host-packs.wasm
coax/web/tool.sh coax/web/build-layers.coax . apps/openagents-web/build/host-layers.wasm
coax/web/tool.sh coax/web/build-services.coax . apps/openagents-web/build/host-services.wasm
coax/web/wasm.sh metaverse-engine/host/frame-plan-carrier.coax coax/web/frame-plan-face.c apps/openagents-web/build/frame-plan.wasm
```

`apps/openagents-web/build.sh` builds all of these too (and every other page).

Then open `http://127.0.0.1:9147/demos/microduck-bay/?pack=microduck-bay`.
The page loads its pack as `private:microduck-bay`, which a local address
serves from `~/.openagents/verse/private-build/web/` only when the URL asks for
it (`?pack=`). Anywhere else the pack does not load.

A headless capture:

```sh
coax/web/tool.sh shot "http://127.0.0.1:9147/demos/microduck-bay/?pack=microduck-bay" /tmp/bay.png 1440x900 9000
```

## What comes from where

- **Bay shell**: `LI_Bay`'s placements in the import bundle: its `Cube` boxes
  tagged `bay_part:KIND` (floor, back wall, side piers, loft deck, rails, the
  stair). The hidden stair ramps are left out.
- **Props**: `L_Ring`'s `bay_dress` placements whose origin stands on bay
  `bay_A1`'s floor, taken back into the bay's own frame by `bay_A1`'s row of
  `Data/level_layout.csv`. Each prop's mesh is the project's
  `Assets/Source/Things/thing_bay_*.glb` (it keeps the vertex colours), read with
  the engine's glTF importer (`pi_glb`) and turned into the engine's frame with
  the importer's axes (`metaverse-engine/import/unreal/axes.coax`).
- **Recipe**: `RECIPE_MICRODUCK` from the ported rules
  ([`../splice-bots/rules/recipes.coax`](../splice-bots/rules/recipes.coax));
  item names and stack sizes from `Data/items.csv` and the Microduck's speed
  from `Data/robots.csv`, each read with the rules' own readers. The Microduck's
  game height (0.62 m) is from `Data/robots.csv`'s notes.
- **Microduck**: the Microduck viewer's pack and RBA1 record
  ([`../microduck/README.md`](../microduck/README.md)), drawn once per part with
  one GPU instance per duck. The ducks share three pose palettes: the engine's
  gait at two phases half a stride apart, and the stand. The lowest foot stays
  on the floor through the viewer's ground anchor (`mdf_ground`).
- **Walkable floor**: the pack tool marks a 0.25 m grid where any bay mesh
  stands between 6 cm and 75 cm, widened by 25 cm. The ducks walk only on open
  cells inside the walls, so they go round the bench, crates, barrels, the
  crane's legs and the stair.

Files: [`crowd.coax`](crowd.coax) (the crowd, the bench and the catalog's
lines),
[`../tools/assets/microduck-bay-pack.coax`](../tools/assets/microduck-bay-pack.coax),
`apps/openagents-web/microduck-bay.coax` (page, panel and input),
`apps/openagents-web/gpu/microduck-bay.coax` (shaders).

## Limits of this first demo

- The walk is kinematic: the gait animates the legs while the duck slides at
  0.5 m/s (1.1 m/s at most in a line). There is no contact or balance physics.
- The bay has its own small pack tool. It reads the import bundle's placements
  but not yet its meshes and materials, and it does not go through the
  importer's pack (`unreal-level-import.coax`) and `world/imported.coax`. Moving
  onto those is the next step.
- The HUD draws additively, so the panel has no dark backdrop; ducks that walk
  behind it show through.
- It is a standalone page. Opening the same spawn panel from the world editor
  and the animation lab is the rest of #13652.

## Rights

The Microduck geometry is Pollen Robotics' and NVIDIA DLI's, **CC BY-NC-SA 4.0**,
for noncommercial research and internal playtests only. The Splice Bots models
and data are the jam project's, private, and are read at pack time only.
