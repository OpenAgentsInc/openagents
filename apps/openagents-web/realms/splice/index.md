# Splice

An unlisted Aethyr world at `/worlds/splice`, written in Coax on Anime Lab's engine.
A robot stands on Mars at dusk: a dune floor, lavender mesas on the horizon, coral and
ochre rocks, and an original robot with a teal eye and chest light, drawn in the anime style
with coloured ink lines, cel bands and a vivid sky.

Only the world is new. The mesh writers, the uniform layout, the anime shader and the style
come from Anime Lab (`apps/openagents-web/animelab*.coax`, `gpu/animelab.coax`).

The source is `apps/openagents-web/splice.coax` (the world and its frame), `splice-face.c` (the
host face) and `gpu/splice.coax` (the shaders, loaded from Anime Lab's).

## Scenes

Choose one with `?scene=N` on the address, 0 to 4. Each is a still view, drawn from
Pinterest references for the look (ochre sand, coral rock, lavender mesas, cel bands):

- **0 Dunes** (the default): the robot on a dune floor under the mesas.
- **1 Arch**: the robot in the shadow of a red sandstone arch.
- **2 Scissors**: two red crab-bots with pincers on a coral plain.
- **3 Walker**: a white, orange-banded machine on four jointed legs.
- **4 Terraces**: a lavender city cut into stepped terraces, a dome and a sage tower, the robot at its foot.
