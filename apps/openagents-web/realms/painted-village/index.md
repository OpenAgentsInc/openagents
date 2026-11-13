# Painted Village

A Metaverse realm at https://openagents.com/worlds/painted-village: a showcase of the OpenAgents Metaverse Engine's hand-painted look. Scene 1 is the village road: a broad dirt road climbs between a timber-frame cottage and a stone house, past stone walls and grass verges. Scene 2 is the plaza it leads into: flagstones, market stalls, lamps, banners, benches and bunting, and a flight of steps up to a terrace with a fountain and the church, trees all round and pale mountains under a sky with one great heap of cloud. South of the village the road runs on as a path through tall pines, over a plank bridge across a river, and into a meadow of long grass and great oaks; upstream the river falls from a cliff into a pool, in spray and a rainbow. It is one world: you walk from any scene into the next.

Every asset is generated from code. Nothing is imported: the meshes are written by generators with parameters, and the materials are painted by a shader from each surface's own metres.

## How to look around

- W A S D or the arrow keys walk; hold Shift to run. Drag with the mouse to look.
- The number keys stand you where a shot is taken: 1 the road, 2 the plaza, 3 the church steps, 5 6 7 8 the meadow, 9 and 0 the waterfall, - the bridge, = the pine path. H hides and shows the HUD.
- C shows what blocks you, in pink over the scene.
- On a phone, the left half of the screen is a walking stick and the right half looks.
- WORLDS at the top left (or Escape) goes to the [worlds list](https://openagents.com/worlds).
- `#shot`, `#shot2`, `#shot3` and `#shot5` to `#shot12` on the address open a shot with no HUD: https://openagents.com/worlds/painted-village#shot10

Houses, walls, trees, stalls, lamps, the bridge's rails and the cliff block you; the steps take you up to the terrace and the bridge's deck carries you over the river. You can wade the river. Doors don't open: buildings have no insides.

## How it is made

- `metaverse-engine/arch/village.coax`, `metaverse-engine/arch/plaza.coax` and `metaverse-engine/arch/flora.coax` are the asset library: generators for a cottage, a stone house, a round tower, a church, a dry-stone wall, a coped wall, a gate pier, a lamp post, a banner pole, a signpost, bunting, a market stall, a crate, steps, a terrace, a fountain, a bench, banners, trees by species and season, bushes, far hills and the ground. Each takes a record of sizes and colours, and writes the solids that block you with its triangles (`metaverse-engine/arch/solids.coax`). `metaverse-engine/arch/README.md` lists every parameter.
- `metaverse-engine/render/stylized.coax` is the look, an engine feature any world can use: materials painted from metres (courses of stone, scalloped roof tiles, boards, plaster, leaded lit windows, stained glass, banners, flagstones, leaves in four tones), sunlight in soft bands, soft shadows from a shadow map, aerial haze, painterly grass, dirt paths with soft edges, and a sky with heaped clouds placed by hand. `metaverse-engine/render/stylized-frame.coax` writes a frame's numbers for it.
- `metaverse-engine/arch/land.coax` is the wild land: a cliff, boulders, a river, a waterfall, spray, a rainbow, a plank bridge, reeds and flowers. Water is drawn last in the engine's stylized water pass: depth tint, ripples that run downstream, and foam at the shore and round rocks.
- `metaverse/painted/village-road.coax` is the village, and `metaverse/painted/wilds.coax` the country south of it. It only says which generators, with what parameters, and where.
- `apps/openagents-web/painted.coax` is the page: the camera you walk and the HUD. `painted-face.c` binds it for the browser, and `gpu/painted.coax` names the engine's shaders (`build.sh` writes `painted.wgsl` from it).
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [metaverse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/metaverse/SKILL.md).
