# Splice

An unlisted Aethyr world at `/worlds/splice`, written in Coax on Anime Lab's engine.
A robot stands on Mars at dusk: a dune floor, lavender mesas on the horizon, coral and
ochre rocks, and an original robot with a teal eye and chest light, drawn in the anime style
with coloured ink lines, cel bands and a vivid sky.

Only the staging is new. The cel mesh writers (`aethyr/render/cel-mesh.coax`), the machine
and land kits (`aethyr/kit/machines.coax`, `aethyr/kit/land.coax`) are the engine's; the uniform
layout, the anime shader and the style come from Anime Lab (`apps/openagents-web/animelab*.coax`,
`gpu/animelab.coax`). Each scene picks its own anime variant and clock.

The source is `apps/openagents-web/splice.coax` (the world and its frame), `splice-face.c` (the
host face) and `gpu/splice.coax` (the shaders, loaded from Anime Lab's).

## Scenes

Choose one with `?scene=N` on the address, 0 to 16. Each is a still view, drawn from
Pinterest references for the look:

- **0 Dunes** (the default): the robot on a dune floor under lavender mesas, at dusk.
- **1 Arch**: the robot in the shadow of a red post-and-lintel arch.
- **2 Scissors**: two red crab-bots square off, the robot watching.
- **3 Walker**: a white, orange-banded machine on four jointed legs.
- **4 Terraces**: a lavender terraced city, a dome and a sage tower.
- **5 Pack walker**: a pack beast under a natural red arch, teal noon sky.
- **6 Yellow canyon**: the robot small on the floor of a deep ochre canyon, a moon above.
- **7 Palace**: sage-domed houses and palms by a teal sea.
- **8 Spider**: an orange spider bot on cinder ground under a neon purple sky.
- **9 Tank crabs**: three blue tank crabs on a white salt flat.
- **10 Salt caravan**: sand walkers in a line across a white flat.
- **11 Windmill**: a lone turbine on dusty ground under a purple dusk.
- **12 Sky city**: lavender towers in a canyon, airships and birds overhead.
- **13 Big mech**: a round white mech with orange plates and a rover on snow.
- **14 Spire**: a red night canyon and one tall banded spire.
- **15 Coral sea**: the robot on a shore rock over a red-orange sea.
- **16 Sun stairs**: a lavender stair to a wall with a blue sun banner.
