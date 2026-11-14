# Splice Bots

Splice Bots, an unlisted game at `/worlds/splice`: a short game about a robot smuggler. Scavenge
banned robot parts, splice them in the rover's hidden edit bay, and restore a scene the Emperor
erased. It is a prototype of the Splice Bots vertical slice.

## The run

The run follows the Splice Bots game design document, Tier 1 and Tier 2.

1. **Title.** Click to play.
2. **Dome village, dawn.** Walk out through the scanner gate: "HEAT: COOL. PASS."
3. **Rail drive, day.** The rover follows the rail; W and S change its speed. Halfway a sensor
   ping stops it. Walk to the marker: the Digger picks up the Battery, and you board again. On the
   way on, a raider mech gives chase: hold W to outrun it, or press E to battle it from Sniper with
   keys 1 to 4.
4. **Day mine.** The Digger follows you from the start and picks up every part within his
   sensor's range: the Drill Arm, the Treads and the MOVE and DRILL cards. A better sensor in the
   scrap room doubles his range.
5. **Edit bay.** In the rover (Tab), drag the parts and cards onto the film-strip timeline and press
   SPLICE. Once its three parts are in, the Digger can dig. When you
   deploy it, it leaves your side and drives to its work. DRILL then MOVE is a bad cut; MOVE then
   DRILL opens the rubble.
6. **Dusk, then night: the party.** The passage leads to a raiders' party: a hall with a main
   stage, a DJ, a dance floor and lamps on the beat, and rooms off it to explore: a bar, a
   workshop, a vault, and a lounge reached from the catwalk. Raiders dance where they stand and
   only fight if you hit them. Imperial robots patrol; they notice you from further away as Heat
   rises, chase you, and give up at the catwalk.
7. **Fights.** F stuns a robot within 2 m for 4 seconds. Point the Digger at a foe with a left
   click and he fights it: a raider goes down in two strikes, an Imperial robot in three, and each
   strikes him back. Each one he beats drops a robot piece into your inventory and adds 10 Heat.
8. **Battery cells.** Four lie in the rooms. Cells and robot pieces are kept to trade for
   battery juice later; nothing spends them yet.
9. **The one blue piece.** The special part, DRILL+, is in the vault. It adds 25 Heat. Put it on
   the strip in the rover: it is Digger's hardest-hitting card.
10. **The drill mech.** A giant mech with a drill arm stands by the blocked hallway. Press E
    beside it to swap Digger's core into it: its lights turn Digger's and it follows you. Press E
    at the hallway and it drills through. Nothing else breaks it.
11. **The boss.** A dinosaur bot glowing blue. The mech battles it on its own, and keys 1 to 4
    pick its moves; it sweeps its tail, then below half health it charges. While it reels from a hit, is dazed after
    a charge, or has no health left, its three weak points glow: stun all three with F (a stunned
    point wakes after 12 seconds). It powers down, drops its core, and night falls. If your health
    or the mech's runs out, you are back at the rover.
12. **The memory.** Splice the core in the rover. A hologram plays, then SCENE RESTORED, then
    what comes next.

## Battles

Every bot has a health bar and a battery. Its cards are its moves: keys **1 to 4**. A move costs
battery charge (big moves cost more) and needs a moment before it is ready again. In a battle,
**E** swaps Digger's core into your other bot. A bot at zero powers down: tow a powered-down Digger
to Sniper's charger with **E**, and stand by the rover for a moment to repair Sniper.

You, the Digger and every foe have a health bar. At zero health or 100 Heat you are back at the
rover with the inventory you had when you last left it; banned pieces picked up since are lost.
At zero the Digger powers down: press E beside him to tow him to the rover, whose charger fixes
him. Heat cools by 5 a second at the rover and nowhere else, but never below 25 for each banned piece with
you, and the more Heat you have, the farther Imperial robots sense you. Your inventory has six
slots: press I and click a slot to drop what is in it.

## Controls

- **W, A, S, D** move; the **mouse** looks (click to hold the pointer). The left and right arrows
  also turn.
- **Space** jumps: over a low piece, or onto its top. Press it again in the air to jump higher. On the hoverboard it hops the board.
- Hold **Left Shift** to sneak: slower, and Imperial robots sense you from half as far.
- **Q** does everything on the hoverboard: the first press steps onto it at casual speed, then each
  press switches casual and fast. Hold **Ctrl** or double-tap **W** for very fast. Double-tap **Q** for super fast. Hold **Q** to step off.
- **E** interacts: boards the rover, takes the core, goes through a door, tows a powered-down Digger.
  On the drive, while a raider mech chases the rover, **E** battles it from the rover; hold **W** to outrun it.
- **Tab** opens and closes the edit bay, inside the rover only.
- In the edit bay, drag or click a part or card to place it. Keys also work: **1, 2, 3** cards,
  **4, 5, 6** parts, **C** clears the strip, **Enter** is SPLICE, **X** is SPLICE CORE.
- **I** opens the inventory. Click a slot to drop it.
- **Left click** points the Digger at what is under the crosshair: he digs it, picks it up or
  attacks it.
- **F** stuns a robot within 2 m, or a weak point of the banned bot.
- **1, 2, 3, 4** are a bot's battle moves; in a battle **E** swaps Digger's core into your other bot.
- Hold **R** for the bot wheel: move the mouse toward DIG, PICK UP, ATTACK or FOLLOW (or press
  1 to 4) and let go of **R** to tell the Digger.
- **M** shows the deep-garage map.
- **Esc** pauses and lists the controls.
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

The code is in `metaverse/splice-bots/`, and `metaverse/splice-bots/README.md` describes it file by file:

- `metaverse/splice-bots/splice-game.coax`: the game: tuning, world, rules, interface, and tests
  that play the whole run.
- `metaverse/splice-bots/splice-bots.coax`: the page's frame, and the still scenes below.
- `metaverse/splice-bots/gpu/splice-bots.coax`: Anime Lab's shaders plus Splice Bots' own overlay pass
  (ink on paper needs source-over blending) and the placed pieces' instances.
- `metaverse/splice-bots/content/splice.content`: the content tables: the pieces that can be placed, the
  landmarks, and the zones the run keeps clear.
- `metaverse-engine/kit/machines.coax`, `metaverse-engine/kit/land.coax`, `metaverse-engine/render/cel-mesh.coax`: the engine's
  cel kits and mesh writers.

A page script can stand the game at one beat with `coaxial.send(610, "N")`, N from 0 (title) to
33, for stills and checks.

The fonts are in `realms/splice/assets/fonts/` with their Open Font Licences.

## Edit layer

Things can be placed in Splice Bots as data, without changing its code. A placed piece is a row of a
content table, a collider, and a typed edit that can be undone. An edit is refused when it would
stand in a zone the run needs, and a walk of the whole run is the second check. Nothing is placed
on this page yet: a saved world and the tools that edit it are the next steps. The developer
notes are in `metaverse/splice-bots/README.md`, "Edit layer".

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
