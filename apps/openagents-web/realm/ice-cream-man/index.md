# Ice Cream Man

A Verse realm at https://openagents.com/realm/ice-cream-man: the Ice Cream Man's house, built in the browser from a seed. You stand where a run starts, behind the kid, in the first room. There is no play yet; it comes next.

## What it does

- The house is the game's own: its rooms, doors, furniture, ceiling lights and the kid with the fudge bucket, from the same seed the game uses. It looks the same as the original game's house for that seed.
- Add `#seed=12` (any number) to the address to see another house. Without one it is house 3.
- GALAXY (or Escape) goes back to the galaxy. The galaxy shows this realm only at https://openagents.com/verse/galaxy?realms=all.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/icecream/` holds the game in Coax: `rules.coax` (the house and the rules), `mesh.coax` (every room, piece of furniture and model as vertices, checked vertex by vertex against the original C++), `camera.coax` (the view) and `scene.coax` (the house packed for the GPU, and its light).
- `apps/openagents-web/icecream.coax` is the realm's HUD, `icecream-face.c` binds both for the browser, `icecream.wgsl` draws them: banded light, each room's fluorescent flicker, the fog and the kid's ink outline.
- The page is written from `realms.json` by `coax/web/realms.ts`. The contract every realm follows is `docs/verse/realm-host.md` in the OpenAgents repository.
