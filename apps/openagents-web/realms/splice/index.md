# Splice

An unlisted Metaverse Engine world at `/worlds/splice`, written in Coax: a short game about a robot
smuggler. Scavenge banned robot parts, splice them in the rover's hidden edit bay, and restore a
scene the Emperor erased. It is our own take on the Splice vertical slice, built on the OpenAgents Metaverse Engine instead of Unreal.

## The run

The run follows the Splice game design document, Tier 1 and Tier 2.

1. **Title.** Click to play.
2. **Dome village, dawn.** Walk out through the scanner gate: "HEAT: COOL. PASS."
3. **Rail drive, day.** The rover follows the rail; W and S change its speed. Halfway a sensor
   ping stops it. Walk to the marker, pick up the Battery, and board again.
4. **Day mine.** Pick up the Drill Arm, the Treads and the MOVE and DRILL cards.
5. **Edit bay.** At the rover, drag the parts and cards onto the film-strip timeline and press
   SPLICE. Once its three parts are in, the Digger is built and follows you everywhere. When you
   deploy it, it leaves your side and drives to its work. DRILL then MOVE is a bad cut; MOVE then
   DRILL opens the rubble.
6. **Dusk, then night: the party.** The passage leads to a raiders' party: a hall with a main
   stage, a DJ, a dance floor and lamps on the beat, and rooms off it to explore: a bar, a
   workshop, a vault, and a lounge reached from the catwalk. Raiders dance where they stand and
   only fight if you hit them. Imperial robots patrol; they notice you from further away as Heat
   rises, chase you, and give up at the catwalk.
7. **Fights.** F hits whoever is in reach. A raider goes down in two hits, an Imperial robot in
   three. Each one you beat drops a robot piece into your inventory and adds 10 Heat.
8. **Battery cells.** Four lie in the rooms. Cells and robot pieces are kept to trade for
   battery juice later; nothing spends them yet.
9. **The one blue piece.** The special part, DRILL+, is in the vault. It adds 25 Heat. Put
   MOVE, DRILL+ on the strip at the stash crate; only DRILL+ breaks the boss wall.
10. **The banned bot.** Its arm sweeps round every few seconds. Stun its three weak points with
    F; a stunned point wakes after 12 seconds, so all three must be down together. Take its core.
11. **The memory.** Splice the core at the crate or the rover. A hologram of the old world plays,
    then SCENE RESTORED.

Three hearts. At zero hearts or 100 Heat you are back at the checkpoint post by the rover with
the inventory you had when you last stood there. Heat cools only by the rover or the stash crate.

## Controls

- **W, A, S, D** move; the **mouse** looks (click to hold the pointer, Escape frees it). The left
  and right arrows also turn.
- **E** picks up, boards the rover, and opens the edit bay.
- **TAB** opens and closes the edit bay at the rover or the stash crate.
- In the edit bay, drag or click a part or card to place it. Keys also work: **1, 2, 3** cards,
  **4, 5, 6** parts, **C** clears the strip, **Enter** is SPLICE, **X** is SPLICE CORE.
- **F** stuns a weak point within 2 m, or hits a raider or an Imperial robot in reach.
- **G** switches graybox on and off: the whole world in grays, with only orange, yellow and cyan
  left in colour. `?gray` in the address opens in it.

## Cast

The five characters of the slice are our own designs (`splice-cast.coax`): the Smuggler, the Rover,
the Digger, a raider and the banned bot. To review them outside the game, open `?scene=17` (the
line-up at true sizes), `?scene=18` to `?scene=22` (one each) or `?scene=23` (the line-up from
behind).

## Look

The interface follows the Game UI board: ink on cream paper (paper `#EADBC4`, ink `#1E1B18`,
orange `#B8431F` for danger, blue `#4FB3D9` for banned tech), Michroma for banners, titles and
buttons, IBM Plex Mono for labels and captions, film-strip sprockets on the title and the
timeline. The world is cel-shaded flat fills with ink contour lines, in four time-of-day
presets: dawn, day, dusk and night.

## Not in this build

- Sound: the document assigns the sound list to a later asset drop.
- Tier 3: the solar farm stop and the village trade stall.
- The memory's caption lines are placeholders, and "1 of ? erased scenes" keeps the document's
  open count.
- The characters are engine shapes until the art drops arrive; the smuggler is a cylinder.
- Nothing buys or sells yet: robot pieces and battery cells only count up.

## Source

The code is in `verse/splice/`, and `verse/splice/README.md` describes it file by file:

- `verse/splice/splice-game.coax`: the game: tuning, world, rules, interface, and tests
  that play the whole run.
- `verse/splice/splice.coax`: the page's frame, and the still scenes below.
- `verse/splice/gpu/splice.coax`: Anime Lab's shaders plus Splice's own overlay pass
  (ink on paper needs source-over blending) and the placed pieces' instances.
- `verse/splice/content/splice.content`: the content tables: the pieces that can be placed, the
  landmarks, and the zones the run keeps clear.
- `aethyr/kit/machines.coax`, `aethyr/kit/land.coax`, `aethyr/render/cel-mesh.coax`: the engine's
  cel kits and mesh writers.

A page script can stand the game at one beat with `coaxial.send(610, "N")`, N from 0 (title) to
19, for stills and checks.

The fonts are in `realms/splice/assets/fonts/` with their Open Font Licences.

## Edit layer

Things can be placed in Splice as data, without changing its code. A placed piece is a row of a
content table, a collider, and a typed edit that can be undone. An edit is refused when it would
stand in a zone the run needs, and a walk of the whole run is the second check. Nothing is placed
on this page yet: a saved world and the tools that edit it are the next steps. The developer
notes are in `verse/splice/README.md`, "Edit layer".

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
