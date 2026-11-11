# Painted Village

A Verse world at https://openagents.com/worlds/painted-village: a showcase of Aethyr's hand-painted look. Scene 1 is the village road: a dirt road climbs between a timber-frame cottage and a stone house, past stone walls and grass verges, under bunting, to a cobbled plaza with stalls, lamps and a well, a church beyond it, trees all round and pale mountains under a sky of big clouds.

Every asset is generated from code. Nothing is imported: the meshes are written by generators with parameters, and the materials are painted by a shader from each surface's own metres.

## How to look around

- W A S D or the arrow keys walk; hold Shift to run. Drag with the mouse to look.
- 1 puts you back where the shot is taken. H hides and shows the HUD.
- On a phone, the left half of the screen is a walking stick and the right half looks.
- WORLDS at the top left (or Escape) goes to the [worlds list](https://openagents.com/worlds).
- `#shot` on the address opens the shot with no HUD: https://openagents.com/worlds/painted-village#shot

There are no walls to stop you yet: you can walk through buildings.

## How it is made

- `aethyr/arch/village.coax` and `aethyr/arch/flora.coax` are the asset library: generators for a cottage, a stone house, a round tower, a church, a dry-stone wall, a coped wall, a gate pier, a lamp post, a banner pole, a signpost, bunting, a market stall, a crate, trees by species and season, far hills and the ground. Each takes a record of sizes and colours. `aethyr/arch/README.md` lists every parameter.
- `aethyr/render/stylized.coax` is the look, an engine feature any world can use: materials painted from metres (courses of stone, scalloped roof tiles, boards, plaster, leaded lit windows, leaves), sunlight in soft bands, soft shadows from a shadow map, aerial haze, painterly grass, dirt paths with soft edges, and a sky with big clouds. `aethyr/render/stylized-frame.coax` writes a frame's numbers for it.
- `verse/painted/village-road.coax` is the scene. It only says which generators, with what parameters, and where.
- `apps/openagents-web/painted.coax` is the page: the camera you walk and the HUD. `painted-face.c` binds it for the browser, and `gpu/painted.coax` names the engine's shaders (`build.sh` writes `painted.wgsl` from it).
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
