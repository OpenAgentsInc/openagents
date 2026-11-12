# Splice

An unlisted Aethyr world at `/worlds/splice`, written in Coax: a short game about a robot
smuggler. Scavenge banned robot parts, splice them in the rover's hidden edit bay, and restore a
scene the Emperor erased. It is our own take on the Splice vertical slice, built on the Aethyr
engine instead of Unreal.

## The run

1. **Day mine.** Walk the scrap room for the Drill Arm, Treads, Battery and the MOVE and DRILL cards.
2. **Edit bay.** At the rover, put the cards on the film strip and deploy the Digger. DRILL then
   MOVE is a bad cut; MOVE then DRILL opens the rubble.
3. **Dusk, then night.** The tunnel leads to the raiders' party. Four banned pieces glow blue on
   the floor and the catwalk. Each adds 25 Heat; raiders notice you from further away as Heat
   rises, and the catwalk is safe from them. Three pieces make DRILL+. A fourth, without cooling
   off at the stash crate first, brings the Imperial Sweep.
4. **The boss wall.** Splice DRILL+ at the stash crate and deploy MOVE, DRILL+.
5. **The banned bot.** Its arm sweeps round every few seconds. Stun its three weak points between
   sweeps, take its core, and splice the core at the crate or the rover.
6. **The memory.** A hologram of the old world plays, then SCENE RESTORED.

Three hearts. At zero hearts or 100 Heat you are back at the rover, and banned pieces picked up
since then are lost.

## Controls

- **W, S** walk; **A, D** turn (the arrow keys also work).
- **E** opens and closes the edit bay at the rover or the stash crate.
- In the edit bay: **1, 2, 3** add a card, **C** clears the strip, **Enter** deploys, **X** splices
  DRILL+ or the core. The buttons also take clicks.
- **F** stuns a weak point within reach.

## Look

Ink on cream paper for the interface (paper `#EADBC4`, ink `#1E1B18`, orange for danger, blue for
banned tech), film-strip sprockets on the title and the timeline, and cel-shaded flat fills with
ink contour lines for the world: a bright day, dusk, and a neon night.

## Source

- `apps/openagents-web/splice-game.coax`: the game: tuning, world, rules, interface, and tests
  that play the whole run.
- `apps/openagents-web/splice.coax`: the page's frame, and the still scenes below.
- `apps/openagents-web/gpu/splice.coax`: Anime Lab's shaders plus Splice's own overlay pass
  (ink on paper needs source-over blending).
- `aethyr/kit/machines.coax`, `aethyr/kit/land.coax`, `aethyr/render/cel-mesh.coax`: the engine's
  cel kits and mesh writers.

A page script can stand the game at one beat with `coaxial.send(610, "N")`, N from 0 (title) to
10, for stills and checks.

## Still scenes

`?scene=N` on the address, 0 to 16, shows a still view instead of the game, drawn from
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
