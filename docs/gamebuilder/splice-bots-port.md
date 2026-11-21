# Porting SPLICEBOTS into GameBuilder

For the current assessment of which steps were automated and what a general
ZIP-to-GameBuilder pipeline still needs, see the [Unreal port automation audit](unreal-port-automation-audit.md).
To assemble the playable local browser port from the original shipping ZIP, see
[SPLICEBOTS shipping build port](splicebots-shipping-port.md).

Written 2026-11-16. Status: a plan. Nothing here is built. It starts from
the port begun the same day in Claude session `0fbb4f00` in the openagents
folder, which stopped on a rate limit. Facts about that session come from
its transcript and from git; each is marked with a path or a commit.

## Progress

2026-11-16:

- **SPLICEBOTS is in the app.** It is in the Games menu, owned by
  `chris@openagents.com`, playable by anyone. Today it is a blockout of the
  solar farm (banks of panels, charge points, the garage, the edit bay, the
  lab) that PLAY walks in third person. A stand-in, not the game.
- **The pin is at the tip of the jam work** (`42092fc2e9`) and the prototype
  realm builds from it as a native library (see S0).
- **The Director moves as in the Unreal jam build.** `coax/her.coax` loads
  the jam's own rules from openagents (`rules/walk.coax`, `jetpack.coax`,
  `board.coax`: the C++ `SpliceWalk`, `SpliceJetpack` and `SpliceBoard`
  ported line for line, same numbers) and steps them against the level's
  floor and walls. Checked here: walk 6.9 m/s, sprint 10.35, sneak 2.52; a
  1.67 m jump and one air jump; the jetpack on a held jump once the air jump
  is spent, about 3.6 s from a full tank of three cells, climbing at most
  6.5 m/s, refilling on the ground; the board at 22 m/s and faster
  sprinting, carving, the ollie, no stepping off in the air, stopped by a
  wall. The camera is the jam's orbit (`ApplyOrbit`, which openagents had
  not ported): 5.5 m back, zoom 1.4 to 14 m, first person inside 1.4 m, 47
  degrees widening at a run.
- **The opening level is the jam's own** (2026-11-16). PLAY in SPLICEBOTS
  now loads L_SolarFarm from the imported world pack: 1,816 mesh instances
  (the panels, the Greek quarter, the hills, Digger at her start), their
  materials in the importer's two-tone shading, the level's sun, her start
  at the level's own player start, and its colliders (532 boxes, 956 hulls,
  5 capsules and the ground) under her feet and the board. Textures and sky are in: a material's
  texture layer is the pack's own image (on the farm only Digger has one;
  every other material is a flat colour by the jam's design), and the sky
  is the jam's day as its look doc gives it, one flat blue (`#98D6F8`) with
  a lilac haze along the horizon and its fade from 70 m. The pack's own sky
  image is a single black pixel, because Unreal's sky is a material the
  importer can't bake, so the sky is coloured in the app; the lilac is read
  off the doc's picture, not a number it gives. Pickups and Digger are live (`coax/scene.coax`): the
  farm's ten pickups spin and bob and are taken by walking within 1.3 m, or
  when Digger is within 1.8 m, into a bag that stacks by the jam's
  inventory rule and shows as a nine-slot hotbar; Digger follows 1.5 m to
  her left on the jam's formation and companion rules, is put back beside
  her past 30 m, and stays or follows on a press. Which item each pickup is
  comes from a table copied from the jam's `Data/pickups.csv`, since the
  level's catalog has only meshes. Digger moves as one rigid mesh with a
  bob: his legs don't step, because the level has him as a single static
  mesh. Still missing
  next to the jam's opening scene: Digger's stepping legs, health and feeding, the other robots as live characters (they
  are the level's static meshes), pickups, charge points and story cards as
  things you can use, sound and music, and the level in the editor (the
  editor still shows the blockout).
- **The whole route is one world** (2026-11-16, later). PLAY now draws and
  walks every route map the jam built in its one world frame, as the jam's
  web port does (`metaverse/splice-bots/jam/jam-world.coax`): the solar
  farm, the Greek quarter, the plain, the garage ring and the lab, about
  4,600 instances, 870 boxes, 1,500 hulls and their floors, gathered into
  one set of colliders (`coax/farm.coax`). The farm's three stand-ins over
  the ring and the lab (`StaticMeshActor_1300`, `_1302`, `_1303`, the jam's
  `SJ_OPEN_ALWAYS`) are left out, and her unposed figures in the ring and
  lab are not drawn. A **Travel** pill under the hotbar (T on a Mac) puts her
  at the next map's player start, Digger with her; the jam has no such jump
  (its maps are walked into), it is there so the route can be seen without
  the 840 m ride. Not done: the jam's sealed areas (in the garage it walks
  only the ring's and the lab's colliders; here every map's are walked
  everywhere, which looked right in the ring and the lab but was not checked
  corner by corner), the rock wall and the hallway's barricade opening (they
  stay shut: Digger's drill and the drill mech are not here), each map's own
  key light and point lights (the farm's sun lights everything, so the
  ring's and lab's interiors are flat), and the other maps' sounds and
  night.
- **The rave's raiders are people** (2026-11-16, later). The ring's sixteen
  `SpliceRaveRaider` actors and its 416 `SpliceCrowd` cards came through the
  import as a capsule and a flat quad each, so they are not drawn; instead,
  as the web port's `jam-cast.coax` does by day, each placed raider is one of
  the six raider bodies in the characters pack (by its place in the ring),
  and 48 more stand over the crowd's cards facing the dance floor, all
  playing their idle out of step, while she is within 90 m of the crowd
  (`coax/rave.coax`, skinned by `skin_vertex`). Not done: the party at night
  (dancing to the beat), the game's five raiders on their plans and the
  fights.
- **What is not the jam yet:**
  - **Her model is in** (owner's decision 2026-11-16: bundle the private
    pack). She is drawn as herself: the imported Director, 7,422 vertices on
    65 joints, skinned in the app's Metal host with the jam's own maths, her
    clip chosen by the jam's rule (idle, walk, jog, sprint, the jumps, the
    crouches, the board stance) and blended between baked frames. Still
    missing next to the jam: its anime materials (she is shaded in two flat
    tones from her vertex colours), her jetpack and board as their real
    meshes (they are grey boxes), and no clip for flying (the jam has none
    either: it uses the jump loop).
  - **She owns the jetpack and board from the start.** The jam's inventory
    and pickups are not here.
  - **One step of ordering.** The jetpack's push on her ground speed is
    applied after the walk's own step, not inside it as the jam does. The
    numbers are the same; a frame-exact comparison has not been made.
  - **The board's walls are the ground probe only.** Clipping a block's
    corner at speed is not stopped as the jam's sweep stops it.
  - **Touch controls are new.** The jam had none: a stick, hold-to-sprint, a
    board button and jump (held for the jetpack).
- **Next:** S0's remaining checks (the jam realm, whether play calls a
  server), then the page host shared with Everglade.

## Where the interrupted port stands

**What was being ported.** The Epic game jam entry submitted 2026-11-15,
built in Unreal 5.9: source `~/games/SPLICEBOTS` (github
`OpenAgentsInc/SPLICEBOTS`), pinned at `70c5bad` through the worktree
`~/games/SPLICEBOTS-jam`, plus the shipped build
`~/Downloads/OLD-SPLICEBOTS-Win64-Shipping-20261115-152753.zip`.

**The game.** A third-person action-adventure. A smuggler scavenges banned
parts on a Mars solar farm, commands the robot Digger, and splices cards
and recipes at an edit bay. A 9 + 27 slot inventory, a crew of up to three
robots, harvest, take and feed, charge points, Heat levels, a hoverboard, a
jetpack, a drill mech, a two-phase boss and a night exit. The route is farm,
plain, garage ring, lab boss, rave. Ten maps; about 62,000 lines of C++ of
which 14,000 are rules in 52 sets and 19,500 are tests.

**The key finding of that session.** The jam build is itself a port of the
Coax prototype already in openagents (`metaverse/splice-bots/`). 121 of the
138 tuning rows equal the prototype's constants and the maps sit in the
prototype's world frame. So the port is mostly the jam's content and its
newer rules on the prototype's systems, not a new game. Its plan is
`docs/unreal/2026-11-16-splice-bots-jam-port-plan.md` in openagents
(`a0c55488f0`).

**What it finished.** 71 commits on openagents `main` that day, tip
`42092fc2e9`:

| Piece | State |
| --- | --- |
| Unreal importer: bundle format, converter, editor and cooked routes, rig retarget (`metaverse-engine/import/unreal/`, #13647 to #13651) | Landed |
| The jam's rules in Coax (`metaverse/splice-bots/rules/`, 89 files, 12,000 lines) | Landed, 249 of 249 checks |
| The jam game (`metaverse/splice-bots/jam/`, 18 files, 3,400 lines): the Director's body, raiders, collisions, boss numbers, night exit, jetpack, sounds, the cabin chest | Landed |
| A walker for any imported world (`metaverse-engine/world/imported.coax`, page `/worlds/splice-bots-jam`) | Landed (`19ff5d7715`) |
| The imported content as private packs, out of git, at `~/.openagents/unreal/splice-bots-jam/` (the editor pack is 58 MB) | On this machine |

**What it left.** It stopped between 13:47 and 14:01 CST; the limit resets
2026-11-17 at 15:00 CST.

- The gameplay agent died mid-work. Not confirmed done: the hotbar and
  inventory on the ported rules; harvest, take and feed; charge points; crew
  stay and follow; Heat levels; story cards; edit-bay recipes and the
  Microduck in play; first person; a real-key proof in a browser.
- The web deploy: staging has the API and rooms but not the page (the
  release stopped on a missing `demos/grid-crowd/assets/cast/Cast.bin`).
  Production was never deployed.
- Issues #13646 and #13653 are open.
- Owner decisions still open in the importer spec, section 10: whether
  content round-trips to the prototype, Fab assets, who may import, and
  whether to commit six jam data files.

**One thing to know about this repo.** GameBuilder's lock (`19ff5d7715`) is
the first commit of that work and is 78 commits behind its tip. At the pin
there is no `rules/`, no `jam/` and only part of the importer.

## What "port into GameBuilder" should mean

Three levels (the Everglade plan that also used them was removed with Everglade on 2026-11-16):

| Level | What the user gets |
| --- | --- |
| A | SPLICEBOTS is a game in the app. Tap it and play the jam route |
| B | It plays with no connection and no account |
| C | It opens in the editor as a project: change the farm, the cards, the recipes, play it, publish it |

SPLICEBOTS is a much better first port than Everglade, for four
reasons:

1. **It is single-player.** There is no room to run, so level B is close to
   level A. Confirm this at S0: the prototype's editing is server-backed
   (`world-live`, `world-follow`), and whether play itself calls the server
   needs a read.
2. **It is already built the way GameBuilder is.** The prototype loads the
   engine's `editor/world-doc`, `viewport`, `pack`, `place` and `edit`
   modules: the same document, viewport and edit log GameBuilder now uses.
   Level C starts near, not far.
3. **It is the owner's own content.** No third-party packs were used
   (`Docs/jam/04-third-party.md`), and Epic's sample content is substituted
   or retargeted, never imported.
4. **It is small enough.** About 41,000 lines of Coax against Everglade's
   100,000-line closure, and one 58 MB pack.

## Plan

### S0. Catch up and take stock (1 to 2 days)

Done so far, 2026-11-16: the pin is bumped to `42092fc2e9` and GameBuilder
builds on it. The prototype realm builds natively from the pinned tree
(`metaverse/native/build.sh splice-bots`): 15 seconds to C, 9 more to an
iOS simulator library of 1.9 MB. The jam realm (`splice-jam`) and whether
play calls a server are still to check.

- `scripts/pin.sh bump` to the tip of the jam work on openagents `main`.
  78 commits will move things; fix what breaks, with the build passing.
- Read whether play needs a server, and what the `splice-jam` page module
  needs from its host (its GPU bindings for the pack's vertices, index
  buffer and instance store).
- Check whether SPLICEBOTS builds natively today
  (`metaverse/native/build.sh`). Everglade does; I have not confirmed
  SPLICEBOTS does.
- List the private packs, their sizes and what each holds.

Exit: GameBuilder builds on the new pin, and a short note answers the three
unknowns.

### S1. Finish the jam game (in openagents; size depends on what the dead agent left)

The game has to be whole before it is ported twice. This is the interrupted
session's own list: inventory and hotbar on the ported rules, harvest, take
and feed, charge points, crew, Heat, story cards, the edit bay and the
Microduck, and a full run of the route under real keys. It lands in
openagents and closes #13653.

Who does it is a choice (question 2). It can be that session when its limit
resets, or GameBuilder agents working in an openagents worktree. It should
not be both at once on the same files.

Exit: the route plays start to end on the web, and its two whole-run checks
pass with the jam world.

### S2. The engine renderer in GameBuilder (shared with Everglade's E1)

SPLICEBOTS draws imported meshes with real materials. GameBuilder draws
lit boxes. This is the same prerequisite as Everglade's E1 and is done once:
adopt the engine's page host and frame plan, draw GameBuilder's own levels
through it, delete the hand-written shader.

Exit: as E1.

### S3. SPLICEBOTS runs in the app (level A and B, 1 to 2 weeks after S2)

- Build the `splice-jam` realm as a library from the pinned tree and bundle
  or fetch its pack (question 4).
- A games library screen: your levels, and SPLICEBOTS.
- Touch controls. The jam was built for a keyboard and a gamepad. The
  phone needs a move stick, look, and buttons for the hotbar, harvest, the
  jetpack and the board. The arena template's stick and button layout in
  `coax/play.coax` is the pattern; the general part should move into the
  engine for both.
- Save the run on the device.

Exit: on an iPhone with the network off, a new player finishes the farm.

### S4. SPLICEBOTS as a project (level C, 3 to 6 weeks)

1. **The tables.** Cards, recipes, tuning and story cards are data
   (`Data/*.csv` in the jam, tables in the port). Edit them in the app,
   undoably, and play the result.
2. **The maps.** Open a map as a world document in the level editor: move
   props, pickups, charge points and spawn points with the gizmos.
3. **New from it.** "Make my own from SPLICEBOTS": a copy the creator
   owns.

Exit: a creator changes a recipe and moves a charge point on a phone and
plays it.

### S5. Publish

A version on the games domain like any other (`docs/publishing.md`): the
documents, the tables, the pack and a pinned player build. The private pack
becomes something a published game serves, so who may see it is settled
here (question 5).

## The Unreal importer is a GameBuilder feature too

That session built a general importer, not a one-off. For GameBuilder it
means a creator with an Unreal project could bring it in. That is a large
product decision with its own rights questions (the spec's open "who may
import"), and it is out of this plan's scope, but it is why the importer's
work should stay general.

## What goes where

| Work | Repo |
| --- | --- |
| Finishing the jam game, its rules and checks | openagents (`metaverse/splice-bots/`) |
| The importer | openagents (`metaverse-engine/import/unreal/`) |
| A consumable page host; shared touch controls | openagents |
| The games library, the editor screens for tables and maps, "make my own", publishing | GameBuilder |
| The imported packs | Neither repo. Private files today; a bucket on the games domain later |

## Risks

- **Two efforts on one game.** The web port is mid-flight in another
  session. Decide who owns finishing it before anyone starts.
- **The pin jump.** 78 commits in one bump, across an area moving that fast,
  will break things. Do it first and alone.
- **The pack is private and local.** It exists on one machine, outside git.
  A release build needs it from somewhere reproducible.
- **The proprietary line.** The Coax port is in the open-source repo. If
  SPLICEBOTS is meant to be a closed game, its code is in the wrong place
  today (question 3).
- **Unconfirmed items in the jam's own record.** The Tripo plan's terms,
  where the character's animation clips came from, and the designer's
  concept images are marked CONFIRM in `Docs/jam/04-third-party.md`. The
  owner said legal is handled separately; this is the list.
- **Phone performance.** Imported Unreal meshes were not made for a phone.
  Measure at S3.

## Things I could not confirm

- Which of the gameplay items the dead agent finished beyond its commit
  titles.
- Whether play calls a server at all.
- Whether SPLICEBOTS has a native build today.
- Whether the private packs uploaded to openagents.com are current.
- Whether the 54 modified `.wgsl` files in the main openagents checkout
  belong to that session.

## Questions for the owner

1. **Order.** SPLICEBOTS before Everglade? I recommend it: smaller,
   single-player, your own content, and already on the same document model.
2. **Who finishes the jam game on the web?** That session when its limit
   resets on 2026-11-17, or agents from here in an openagents worktree?
3. **Is SPLICEBOTS open or closed?** Its Coax code is in the open
   openagents repo today, with the art private. Should the game's code move
   to a private home, with only general engine work staying open?
4. **Bundled or downloaded?** 58 MB in every GameBuilder install, or
   fetched when SPLICEBOTS is first opened?
5. **Where does it live once published?** On the games domain only, or
   also at `/worlds/splice-bots-jam` on openagents.com, where the web port
   was heading?
6. **Level.** Stop at "it plays in the app" for now, or go on to "it opens
   in the editor"?
7. **Which version is the base?** The jam commit `70c5bad`, as that session
   pinned, or the later work (Microduck crafting at `8a58f75`, the Tripo
   raiders)?
