# Splice Bots jam build on the web: gap analysis and port plan

GitHub #13653 (umbrella #13646). Written 16 Nov 2026.

The goal is one realm on openagents.com that plays the Epic jam entry's whole route with the
jam's content (its levels and models, the retargeted Director and raiders, the Microduck) and
the jam's rules, on the engine prototype's systems.

Sources compared:

- The jam build: `~/games/SPLICEBOTS-jam` at `70c5bad` (Unreal 5.9). Gameplay in
  `Source/SpliceGame`, `Source/SpliceUI` and `Source/SpliceRules`; tables in `Data/`; notes in
  `Docs/` (`first-scene.md`, `cabin.md`, `TODO.md`, `jam/01-submission.md`, `decisions/LOG.md`).
- The engine prototype: `metaverse/splice-bots/` (`/worlds/splice-bots`).
- The importer's output: `~/.openagents/unreal/splice-bots-jam/` (private, never in git).

## What the comparison found

The jam build is a port of the prototype, not a new game:

- Of the 138 rows in the jam's `Data/tuning.csv`, 121 name a prototype constant and all 121 hold
  the prototype's value. The other 17 are expressions in the prototype, not different numbers.
- The jam's maps are built in the prototype's world frame. After the importer's axis change
  (Unreal X to engine x, Unreal Y to engine z), the farm's first bank stands at (-929.3, -189),
  which is `FF_XW + FF_PITCH / 2` and `FF_ZN - FF_LINE_DZ` in `farm.coax`, and `SM_thing_garage`
  stands at (0, -195), the prototype's `GA_X`, `GA_Z`. `L_SolarFarm`, `L_Plain`, `L_Ring` and
  `L_Lab` are four pieces of one world.
- The jam's own `Docs/TODO.md` says the target was "back on par with the prototype, which played
  start to end", and its P0 list asks to drop the level loads so she walks or drives to the garage
  "in one world". The prototype already does that.
- In the jam build, only the solar farm plays as a scene with objectives and story cards. The
  plain, the ring and the lab are reached by full map loads. The raider chase is not placed, the
  ring's and lab's pickups are props, and the ending is reachable only through a debug command.

So most of the route already plays in the prototype. The port is mostly content (the jam's
levels, models and characters in place of the prototype's procedural world) and the jam's
newer rules (harvest, the hotbar inventory, the crew, feeding, the jetpack and the Microduck).

## Where the game lives

**Decision: the game is the prototype's code (`metaverse/splice-bots/`), fed the imported pack, and
it is served at `/worlds/splice-bots-jam`.** The jam world is a world source of the same game, not
a second game.

- The prototype has every system the route needs, with about 60 checks, two of which play the
  whole run. `importworld.coax` is a 232-line walker with no game in it. Growing the walker into
  the game would rebuild the prototype.
- The jam's levels sit in the prototype's frame, so the game's landmarks (`SpPlaces`) and
  colliders can come from the imported maps without moving anything.
- `/worlds/splice-bots` keeps its own world and does not load the private pack: on openagents.com
  a `private:` asset always loads, so the jam pack must not be listed on that realm.
- The jam realm needs its own page module because the private pack adds GPU bindings (the pack's
  vertices and index buffer, and an object store for its instances). It is the same game code
  built with the jam world source on, in a second page module, `splice-jam`.
- `importworld` stays as the generic walker for any imported pack. When the game draws the jam
  world, `/worlds/splice-bots-jam` changes its module from `importworld` to `splice-jam`.

## Feature by feature

Legend: **Have**: in the prototype and the same in the jam. **Differs**: in the prototype but
different in the jam; the jam wins. **Missing**: not in the prototype.

### The world and its content

| Feature | Jam | Prototype | Status |
| --- | --- | --- | --- |
| One continuous world | Four maps with full level loads (its own TODO asks for one world) | One world: farm, plain, garage, lab | Have (the prototype's way is the jam's goal) |
| Solar farm layout | `L_SolarFarm`: 84 banks in 4 rows, 6 charge points (18 actors with posts), 10 pickups, 6 Imperial stand-ins, Sniper, Digger, the buried crate, 1,777 static meshes | `farm.coax`: the same rows, drawn in code | Differs: draw the imported map |
| Greek quarter jump course | `L_SolarFarm_GreekQuarter`, 310 meshes, plateaus P1 to P3 at 1.0, 1.8, 2.6 m | none | Missing |
| The plain | `L_Plain`: mesas, giants, garage shell, the farm's road | `plain.coax`: ground, edge stakes, rocks, peaks, pads | Differs: draw the imported map |
| The garage ring | `L_Ring`: 1,903 meshes, 24 bay instances (`LI_Bay`, `LI_Bay_Balcony`), the rock wall (20 chunks), 16 rave raiders, a 400-card crowd | `garage.coax`: the ring, bays, LEDs, the rock wall (rubble) | Differs: draw the imported map; the game keeps its bay logic |
| The lab | `L_Lab`: the hallway and the round lab, 12 lights | `garage.coax ga_draw_lab` | Differs: draw the imported map |
| Sniper's cabin | A room 500 m below the farm (8 x 6 x 3 m, desk, edit bay, chest) | `drive.coax dv_room`: the cabin, desk, wardrobe, edit bay | Have; the chest is Missing |
| Models | 81 meshes, 263 materials, 69 textures | Kit pieces and the designer's Digger | Differs: the pack's models |
| Lighting by map | Key light and up to 16 point lights a map | Anime Lab's sun, sky and ink | Have the look; the pack's point lights are Missing |

### She and her body

| Feature | Jam | Prototype | Status |
| --- | --- | --- | --- |
| Her body | The Director (14k triangles) with her ten clips, retargeted to our 65-joint skeleton (`rig/director`) | Your crew character, posed in code (`crew-player.coax`) | Differs: the retargeted Director |
| Walk, sprint, sneak | 6.9 m/s walk, sprint 1.5x (Left Ctrl or W W), sneak 2.52 m/s | 5.6 m/s walk, Z or W W sprint, sneak 2.52 | Differs: the jam's speeds |
| Jump | 10.6 m/s (about 1.67 m), gravity 32 rising and 1.25x falling, double jump 1.15x, coyote 0.1 s, buffer 0.12 s | 9 m/s, gravity 30, the same coyote and buffer | Differs: the jam's numbers (check the solar rows' tops still land) |
| Camera | Free orbit, zoom 1.4 to 14 m, first person below 1.4 m, V toggles | Follow camera with zoom, rises over the boss | Have; first person is Missing |
| Hearts | 3, a hit costs 1 | `T_HEARTS` | Have |
| Jetpack | Jump, jump, hold Space: 100 fuel in 3 cells, 28/s, climbs 6.5 m/s | none | Missing (the item is never handed out in the jam; a pickup is needed) |
| Hoverboard | Right click to board; casual 22 m/s, sprint 40, ollie 6.5 m/s, R spins 900 deg/s | Q boards; casual and fast speeds from the `board` table | Differs: the jam's speeds and keys |

### Robots, the crew and the fights

| Feature | Jam | Prototype | Status |
| --- | --- | --- | --- |
| Digger follows, fetches parts | Within 4 m of him (8 m with the Sensor card), up to 45 m from her | `T_SENSE` 4 m, doubled by the better sensor | Have |
| Pointing Digger | Left click names a target; he digs, fights, fetches | `digger-orders.coax` | Have |
| Crew of up to 3, formation slots | `SpliceFormation`: 1.5 m beside and behind, leash 30 m | Digger only | Missing |
| Stay / follow | Right click a robot; G for all; an orange lamp | none | Missing |
| Imperial robot at the farm | Sluggish: 12 of 20 health, 1.5 m/s, a blow every 3.2 s, idle until hit; a sense ring at 6 m | Imperial robots on patrol plans | Differs: the jam's farm robot |
| Harvest | Hold left click 1 s on a powered-down robot: a core, 3 Imperial parts, a banned piece every third | none (fights drop scrap) | Missing |
| Take a robot | A core into a robot at 0 makes it hers at 20 % health | none | Missing |
| Feed a robot | Right click it with a battery: +40 % health | A battery cell heals Digger (`d_give_cell`) | Differs |
| Core in and out | Shift + right click takes her robot's core; the body becomes an item | The drill mech's core swap (`sb_swap`) | Differs: general core take is Missing |
| Charge points | Left click: a battery; regrows in 180 s | Sniper's charger repairs | Missing (the farm's charge points) |
| Drill mech | Bay C1, health 45, Digger's core in, drills the hall plugs | The same in C1/C2 | Have |
| Rock wall | 20 chunks, only a drill breaks it, 2.4 s | The rubble, drilled by Digger | Have |
| Raider mech chase | Rules only (16 m/s, claw 3, Sniper's hull 24); not placed | Chases Sniper on the drive, E battles it | Have (the prototype is ahead) |
| Boss | Asleep, wakes at 14 m; health 120; phase 1 tail sweep (6.2 m, 3); phase 2 below half charges (14 m/s, 6) then dazed 2.6 s; a bite every 3 s at 4.5 m | Phases 1 and 2, weak points stunned with F, health 70 | Differs: the jam's numbers and its bite; the jam has no weak points |
| Boss's end | Take it with a core, or harvest it; the experimental core drops | Three stuns power it down; E takes the core | Differs |
| Night exit | 4 floor robots and 2 guards, sight 6 / 18 / 40 m by Heat, reinforcements at High (1 an alert, 3 at most, through bay doors) | The rave's patrols on the way back to Sniper (`sp_night_walk`) | Differs: the jam's exit and reinforcements |
| Rave | 16 raiders dancing, 400 crowd cards, lights at 138 bpm | Five raiders dancing, LEDs | Differs: the jam's raiders and crowd |

### Carrying, splicing and the Microduck

| Feature | Jam | Prototype | Status |
| --- | --- | --- | --- |
| Inventory | 9 hotbar + 27 grid, stacks from `items.csv` (parts 64, battery 16) | Six slots (`d_bag`) | Differs: the ported `SpliceInventory` |
| Chest | 27 slots in the cabin | none | Missing |
| Drop | Q one, Ctrl+Q the stack | A drop key | Have |
| Edit bay | Splice and Recipes tabs; Drill (any robot), Jump (Digger only), Sensor (any) cards; CORE READER | Three hardware slots, a behaviour strip (MOVE, DRILL, DRILL+), SPLICE CORE | Differs: the jam's cards on the ported `SpliceSplice` |
| Recipes | Digger, drill mech, dinosaur (`SpliceRecipes`) | none | Missing (ported in `rules/recipes.coax`) |
| Microduck | `RECIPE_MICRODUCK`: 1 battery + 3 scrap builds the pet, which follows her (added on 15 Nov, after the jam commit) | none | Missing (rules ported; #13652 builds the bay demo) |

### Story, interface and flow

| Feature | Jam | Prototype | Status |
| --- | --- | --- | --- |
| Title, new run, continue, credits | Title over the world, CONTINUE, NEW RUN, SETTINGS, CREDITS; no character screen | The same, with character creation | Have (keep creation; the jam's Director is the default body) |
| Objective | Seven farm lines (`USpliceFarmStory::Objective`), farm only | The compass and the objective for the whole run (`hv_goal`) | Differs: the jam's farm lines, then the prototype's for the rest |
| Story cards | 33 triggers in three lanes (`card_triggers.csv`, `strings.csv`) | Pickup toasts and the objective card | Missing |
| Heat | Low below 4, Medium 4 to 7, High 8+, by what she carries (banned +1, Digger +1, mech +3, dinosaur +5) | 0 to 100, +25 a banned piece, cools at Sniper | Differs: the ported `SpliceHeat` |
| HUD | Heat at the top left in 8 segments, hotbar, prompts, crosshair, the drive strip | The SPLICE UI v2 HUD | Have the frame; the hotbar and crosshair are Missing |
| Map | M: the deep-garage map | M: the deep-garage map | Have |
| Desk refusal | "Not without a drill. Find the DRILL card first." | none | Missing |
| Checkpoint and continue | Leaving Sniper takes a checkpoint; continue restores her place and inventory | Sniper is the checkpoint; the run is kept in this browser | Have |
| Ending | 15 s memory with three captions, SCENE RESTORED, the teaser, RESTART | The same | Have |
| Sound and music | Many effects missing in the jam; four music tracks (title, cabin, desert, rave) | 25 sounds, all off for now | Have the system; the jam's sounds go in the private pack |

## Order of work

Each batch is pushed to `main` green, with its checks, and commented on #13653.

1. **The jam world as the game's world source** (`metaverse/splice-bots/jam/`). Read the imported
   pack's catalog, take the game's places from the imported maps' game actors (Sniper, Digger, the
   charge points, pickups, Imperial robots, the crate, the rock wall, raiders, player starts) and
   collide against the imported maps' colliders. The game's checks that play the run use a
   committed synthetic catalog; a tool plays the same run on the real private pack.
2. **The jam realm draws the jam world.** The `splice-jam` page module: the game with the jam
   world source on, the pack's instances drawn in the game's cel and ink pass, and
   `/worlds/splice-bots-jam` moved onto it. A headless shot at each scene.
3. **Her body and the raiders.** The retargeted Director as her body with her clips, the raiders
   at the rave. The private pack carries them (the importer's pack writer, coordinated on #13646).
4. **The jam's rules.** On the SpliceRules port as each batch lands (#13653): the jam's walk and
   jump, the 9 + 27 inventory and hotbar, harvest, take and feed, charge points, the crew and
   stay/follow, Heat levels, the farm's objective lines and story cards, the desk refusal, the
   boss's numbers and bite, the night exit.
5. **The edit bay's recipes and the Microduck.** The jam's cards and recipes in the edit bay, and the
   Microduck built at the bench and following her (the model only in the private pack). The game
   reuses the edit bay demo (`metaverse/microduck-bay/`, #13652): its craft at the bench, its
   Microduck spawn and wander, its LI_Bay and bay_A1 dressing. It does not copy them.
6. **The rest.** The Greek quarter's jump course and the jetpack, the chest, first person, the jam's
   sounds and music.

## Status

- Item 1 landed (ac93d16498, then the garage stand-in fix). The source of truth is the editor pack
  (`~/.openagents/unreal/splice-bots-jam/import-editor/splice-bots-jam-editor.pack`, all ten maps;
  the cooked build matches 4,511 of its 4,861 placements, #13650). On it, `jam-run-check.coax`
  passes the whole run, the farm's start to Sniper with the keys, and the garage door to the boss
  with the keys.
- The farm's and plain's colliders are walked. The ring's and lab's wait, because the jam loaded
  each as a sealed map. Inside the garage, `garage.coax`'s walls stand in. The farm map's 315 m
  block over the garage's spot (StaticMeshActor_1300) is left out.
- Item 3 landed. Her body is the Director and the rave's raiders are the jam's six raider bodies, from
  a second private pack, `private:splice-bots-jam-chars` (`jam/jam-chars.coax` has its format,
  `jam/jam-cast.coax` the draw). The game picks her clip from how she moves: idle, walk, jog,
  sprint, the jump's three parts, the crouch and the board. The game's five raiders are every third of
  the jam's 16 in `L_Ring`, and their plans in the garage start there. While the party is on, the
  other 11 dance as the crowd. The page skins the bodies on the GPU from palettes baked per frame, and
  each vertex has the source mesh's own colour. Without the pack, the page draws the prototype's bodies.
- The ring's audience is raiders, not the jam's crowd cards. The import does not bring the cards' look
  through, so the 416 cards drew as white slabs. The page hides them and stands up to 48 raiders where
  the cards are, facing the dance floor. They dance while the party is on and stand by day. The jam
  also places her body's mesh, unposed, on a board by Digger in the ring. That figure is hidden too,
  because the page draws her itself.

  The characters come as their own pack for three reasons. They do not wait on an importer rebuild of
  the world pack. A change to them does not mean uploading the world pack again. The page loads
  without either pack. Make the pack from the retargeted rigs:

  ```
  coax/web/tool.sh metaverse/splice-bots/tools/jam-chars-pack.coax \
    ~/.openagents/unreal/splice-bots-jam/rig \
    ~/.openagents/verse/private-build/web/splice-bots-jam-chars.pack
  ```

  To check the pack, run `jam-run-check.coax PACK CHARS`.

## Acceptance

A headless full-run check that plays the jam route end to end, as `g_play_run` and `sp_keys_play`
do for the prototype:

- `coax/bin/coax test metaverse/splice-bots/splice-bots.coax` keeps passing (the prototype's run
  and every other check), so `/worlds/splice-bots` does not break.
- A jam run check in the same suite plays the route on the committed synthetic jam world: the
  title, the farm by day (pick up the parts and cards, splice the Drill card, beat and harvest the
  farm robot, a battery from a charge point), Sniper's cabin and the edit bay, the drive to the
  garage, the rock wall, the drill mech, the hallway, the boss, the core, the night exit and the
  ending.
- `metaverse/splice-bots/tools/jam-run-check.coax PACK` plays the same run on the real private
  pack, against its own landmarks and colliders, and says which beat failed.
- Headless shots of `/worlds/splice-bots-jam?pack=splice-bots-jam` at each scene, looked at before a
  batch is called done.

## What stays private

The pack (models, textures, sounds, the Microduck) stays in `private:splice-bots-jam`, and the
characters in `private:splice-bots-jam-chars`. Locally, `?pack` serves both from
`/private/NAME.pack`. On openagents.com, the broker serves them after
`metaverse/tools/humans/upload_packs.coax`. Neither pack is ever listed on `/worlds/splice-bots`. Git holds the code, the checks and a synthetic
catalog of boxes, never the game's models or tables.
