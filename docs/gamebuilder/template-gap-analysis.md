# What it would take to build Scrappers Arena and SPLICEBOTS in the editor

Written 2026-11-17. A gap analysis, nothing built. The question is: could
someone make either of our two games from nothing, using only GameBuilder's
editor? Today, no. This note lists what is missing and in what order to add
it.

**Decided 2026-11-17 (owner):** "buildable" includes writing new
behaviour, not only tuning and laying out a template we wrote. Gap 7
covers how. The engine already has the mechanism (realm scripts), so the
work is adopting it, not designing a language.

## What the editor makes today

The toolbar (`host/Sources/App.swift`) and the core (`coax/editor.coax`)
give a person:

- **Boxes.** "Add block" puts down one 1 m block. Things can be selected,
  moved, turned, scaled, copied, pasted, duplicated and deleted, with undo.
- **Kinds that can't be chosen.** A box has a kind (block, start, rival,
  tree, water, path), but nothing in the editor lets you pick one. Starts,
  rival starts, trees, water and paths appear only because the code puts
  them there (`ed_arena`, `ed_splice`). You can get more of them only by
  copying one that already exists.
- **No per-thing settings.** A thing has no inspector: you can't type a
  size, give it a colour, name it, or say what it does.
- **Levels, not games.** "New Level" always makes a Scrappers arena
  (`ed_new` calls `ed_arena`), and a level of yours always plays as
  Scrappers. The list of games is a constant in the code (`GAMES :: 2` in
  `coax/games.coax`): two titles, two owners, and two fixed ways to play.
- **Play.** It runs the game whose code is built into the app.

So the editor can **rearrange** the arena. It can't **make** a game.

## Where each game lives today

### Scrappers Arena

Almost all of it is code (`coax/play.coax`, `coax/modes.coax`, `net.coax`,
`party.coax`). The level is the one part that is data.

| Part | Where it is | Can the editor change it? |
| --- | --- | --- |
| Walls and cover | The level's boxes. Solid kinds block bodies and shots | Yes |
| Team starts | The `start` and `rival` boxes | Move only; you can't add them |
| Health packs (4) | Worked out from where the starts are (`play.coax` ~267) | No |
| Goals, hoops, the ball | Worked out from the starts (`GOAL_BACK`, `HOOP_*`) | No |
| Modes, scores to win, clocks | Constants (`MODE_GOAL`, `MODE_SECONDS`) | No |
| Team size (1 to 5 a side) | Chosen before a match | Only when playing |
| Classes: Bolt, Boulder, Lumen | Constant tables (`CLASS_HP`, `CLASS_SPEED`...) | No |
| Shots, supers, aim help, respawn, regen | Constants and code | No |
| Bots | Code | No |
| First-person camera, stick and buttons | Code | No |
| HUD, kill feed, sounds | Code and host | No |
| The arena's look (plates, dusk sky, wall and cover tints) | Shader and code | No |
| Playing together (lobby, party, net) | Code | No |

### SPLICEBOTS

Nearly none of it is the level. Play ignores the blockout the editor shows
and loads the imported jam world instead (`docs/splice-bots-port.md`).

| Part | Where it is | Can the editor change it? |
| --- | --- | --- |
| The world: 4,600 meshes, materials, sun, colliders | A private 61 MB pack from the Unreal importer (`coax/farm.coax`) | No. The editor shows a box sketch, and play doesn't use it |
| The Director: skinned model, clips, how she walks, jumps, jetpacks and boards | The pack, plus the jam's rules in openagents (`coax/her.coax`) | No |
| Orbit camera | Code | No |
| Pickups: which mesh is which item | A table copied from the jam's `pickups.csv` (`coax/scene.coax`) | No |
| Bag and hotbar | The jam's inventory rules | No |
| Digger, the companion | The jam's formation and companion rules | No |
| The rave crowd | Code over the characters pack (`coax/rave.coax`) | No |
| Travel between maps | Code (`ed_farm_travel`) | No |
| Not built anywhere yet: charge points, feeding, Heat, story cards, the edit bay, the boss | — | — |

## The gaps

Both games need the same foundations. After that, SPLICEBOTS needs a great
deal more than the arena. In order:

### 1. A game is a document, not a constant

- **New game** in the UI: a title, its owners, and the **template** it plays
  on (arena match, or third-person adventure).
- The game's own document holds its levels, its tables, and which template
  it uses. It is published like a level (`docs/publishing.md`).
- `GAMES`, `GAME_MODE`, `GAME_OWNERS` and `ed_new`'s fixed arena go. Our two
  games become two documents like anyone else's.

### 2. A palette of kinds, and markers

- **Add** opens a palette instead of always adding a block. Its first
  entries are the kinds that exist already (block, start, rival, tree,
  water, path).
- **Markers**: things that are places, not walls. Each template declares its
  own markers:
  - arena: team start, health pack, goal, hoop, ball spot;
  - adventure: player start, pickup, companion start, charge point, travel
    point, trigger zone, NPC.
- Once there are markers, health packs, goals and hoops are **placed** in
  the level rather than worked out from the starts.
- The format work already lines up with this: a thing's kind is a row of
  the level's content table (`coax/level.coax`, 2026-11-17). A marker kind
  is just another table, or another column.

### 3. An inspector

- Select a thing and see its settings: its kind, its exact size in
  centimetres, its colour, its name, and the fields its kind has (which
  item a pickup gives, which map a travel point goes to, how long a health
  pack takes to come back).
- Every change is a `world.edit.v1` edit, so it can be undone, and an agent
  can make the same change.

### 4. A table editor for rules and tuning

- This is the template's numbers, shown as tables. Arena: modes (goal,
  clock), classes (health, speed, damage, cooldown, range, spread, super).
  Adventure: items, pickups, movement (walk, sprint, jump, jetpack, board),
  companions.
- The tables are `aethyr.content.v1`, the format shapes already use, and
  cell edits are already a `world.edit.v1` op (`WE_CELL`). The template's
  code reads its numbers from the tables instead of from constants.
- **This is the step that makes the arena buildable.** With a game
  document, markers, an inspector and tables, every row of the Scrappers
  table above can be set in the editor except bots, HUD, sounds and
  netcode. Those belong to the template.

### 5. Assets

- **Meshes and materials** beyond boxes. That needs the engine's renderer
  in the app (S2 in `docs/splice-bots-port.md`, shared with Everglade's
  E1), an asset library, and import: glTF first, and the Unreal importer
  that already exists in openagents.
- **Characters**: a skinned model, its clips, and which clip plays when.
  The skinning is already in the host for the Director. What's missing is a
  way to bring a character in and assign it.
- **Sky, sun and lights** for each level.
- **Sounds and music**, attached to events.
- Assets have a size problem. The jam's pack is 61 MB and private, so this
  step also needs storage and a way to decide who may see an asset.

### 6. Templates for player, camera and controls

- **Which body and camera** the game uses: an arena fighter in first or
  third person, or an adventurer with orbit, jetpack and board. (The open
  field, 2026-11-17, is the first step: the arena's body, camera, controls
  and look without its match, chosen by template.) The movement
  rules exist for both. The game picks one and tunes it in its tables.
- **Controls**, touch included: the arena's stick and buttons are the
  pattern (`coax/play.coax`), as the port plan already says.
- **The HUD**: score and clock for arena, hotbar and travel for adventure.
  It's chosen by the template and labelled from the tables.

### 7. Behaviour that isn't a number

This is the gap between "tune SPLICEBOTS" and "build SPLICEBOTS".

- **Triggers and events**: walk into a zone and travel; pick up an item and
  it goes in the bag; feed Digger at a charge point; open the rock wall with
  the drill.
- **Characters with goals**: Digger following, raiders on their plans, the
  two-phase boss.
- **Quests and story**: story cards, the night exit, Heat.

#### The engine already has this: realm scripts

The pinned openagents tree has a complete script system
(`metaverse-engine/script/`, openagents #13467; the design is
`docs/research/2026-11-13-realm-scripts.md` and the contract is
`docs/protocol/realm-script-v1.md` there). The parts:

- **A restricted profile of Coax.** A `#script` unit is compiled to its own
  small WebAssembly module. It has no I/O, no clock, no allocation and no
  recursion, and its state is fixed-size structs.
- **Typed handlers:** `on_tick`, `on_enter`, `on_leave`, `on_notice`,
  `on_hit`, `on_use` and `on_timer`.
- **An API of 34 calls in one table.** Reads cover things, distance,
  nearest, time, a seeded random and the world's eight running numbers.
  Writes cover move, stop, face, a thing's four running numbers and
  timers. Engine calls cover strike, say a line, and `sa_emit`, which
  raises an event with a number whose meaning the game decides.
- **Run by an interpreter written in Coax.** Each call has an instruction
  budget, checked memory and a rollback, and a trap fails one call, never
  the game. The interpreter is plain Coax, so it runs anywhere the core
  runs: iPhone, Mac, Windows, Linux and the web, with no JIT. It is fuzzed
  and checked against V8.
- **Scripts are content.** A world names its scripts by hash in a
  `scripts` table (`ID|NAME|HASH|FOR`, where FOR is "kind K" or "who N").
  The table is edited with `world.edit.v1` cell edits, the same edits as
  our shape table since 2026-11-17. Undo, the revision fence and publishing
  apply to scripts unchanged.
- **The rest is built too:** the module check, the manifest (what a script
  can do, read from its bytes before it runs), a server store with a compile
  job for other authors (built, not yet deployed), and the web editor's
  Scripts pane (write, build, run, swap, publish).
- **SPLICEBOTS already uses it.** Its raider and Imperial robot are scripts
  (`metaverse/splice-bots/scripts/`), checked to play the same as their
  plan data. Everglade's room runs its monsters' scripts.

So the answer is realm scripts. We don't need a separate rule language, and
we don't need to load raw Coax modules. People get new behaviour in three
layers:

1. **Data first:** markers, the inspector and tables (gaps 2 to 4), and
   plans for things that walk. The engine's own rule is to offer data
   before a script.
2. **A script, written by an agent.** The person says what they want in the
   agents pane (OpenAgentsInc/gamebuilder#1). The agent writes the
   `#script` unit, it is built and checked, and it lands as one undoable
   step that adds a `scripts` row. The person plays it and keeps or undoes
   it.
3. **A script, written by the person**, in a scripts pane for those who
   want code. This is the same unit and the same check.

#### What GameBuilder has to add

- **Run scripts in play.** Each template fills a `script.frame.v1` from its
  running state (Scrappers, bots, the ball, markers; or the Director,
  Digger and pickups), runs the world's scripts once a tick, and applies the
  commands that come back. The models to follow are Splice's
  `scripts.coax` and Everglade's `server/scripts.coax`. In a party, the
  host's copy runs them, as it already runs every rule (`coax/net.coax`).
- **Event tables.** `sa_emit` raises a number; the template's table says
  what it means. For example: "event 3: the scoring team gets a point",
  "event 7: give the player a battery", "event 9: travel to map 2". Most
  new behaviour is a script deciding *when* and a table row deciding
  *what*, with no new API.
- **API rows the engine lacks**, added in openagents and then pinned. A
  script writes only its own thing, and can't spawn, change a score, give
  an item or move the player. Proposed rows, each capped by the engine:
  spawn and despawn from a marker, read the score and the round, give and
  take an item, set a marker's on/off (doors, zones), and play a sound.
  Grow it by request, as the engine's doc says.
- **Building a script from GameBuilder.** On the developer Mac build, the
  local `coax` and `clang` build it, which is what the agent needs. On the
  iPhone and the App Store Mac build, the compile job builds it once it is
  deployed. The world names the bytes by hash, so a built script runs
  everywhere without being built again.
- **A scripts pane**, drawn in Coax on the shared UI host. It shows each
  script's name, what it drives, what it may do (from its manifest), its
  live budget and its last error, and lets you switch it off. Editing the
  source comes later; first, the agent writes it.
- **App Store review.** The iPhone app would run scripts it downloads,
  under the interpreter. Read App Review guidelines 2.5.2 and 4.7 before
  creators' scripts run in the iPhone build. Our own published scripts are
  content in a game, as Roblox's are, but check this.

#### What it can't do (yet)

Scripts drive things. They don't replace a template's core: a new kind of
player body, a new camera or a new rendering effect is still template code
we write. "New behaviour" in this sense means new rules, modes, enemies,
puzzles, pickups and triggers on top of a template, which covers every
item listed above for SPLICEBOTS except the drill mech's body.

### 8. Several maps in one game

SPLICEBOTS is a route of five maps, and the jam had ten. A game document
holds several levels, joined by travel points (gap 2), with one of them
marked as the start.

### 9. Playing together

The arena's lobby, party and net code belong to the arena template. A game
built on it gets them by choosing the template, and it needs no editor
work. An adventure played together is a separate project, not part of this
analysis.

## What each game needs

| | Scrappers Arena | SPLICEBOTS |
| --- | --- | --- |
| 1. Game document | Needed | Needed |
| 2. Palette and markers | Needed | Needed |
| 3. Inspector | Needed | Needed |
| 4. Table editor | Needed: the last step to "buildable" | Needed |
| 5. Assets | No: it's boxes on purpose | Needed, and the largest piece |
| 6. Player, camera and HUD templates | The arena template as it is | An adventure template from `her.coax` and `scene.coax` |
| 7. Behaviour (realm scripts) | Needed for new modes, bots and gadgets beyond the four modes | Needed, a lot of it |
| 8. Several maps | No | Needed |

**The arena is close.** Gaps 1 to 4 are ordinary editor work on formats
that exist now, plus moving the arena's constants into tables. With realm
scripts in play (gap 7), a person could make Scrappers Arena, or a game
with a mode we never wrote (king of the hill, a moving platform, a boss
bot), from a new game in the editor.

**SPLICEBOTS is far, but its behaviour is not the hard part.** It needs
everything the arena needs, then assets (which in turn need the engine
renderer), an adventure template and several maps. Its unique parts (the
boss, the edit bay, Heat, charge points, story cards) become scripts and
tables on the adventure template, written by agents. Two of its characters
are scripts already.

## Proposed order

1. **Game document and template choice.** Turn `GAMES` into data, and add
   "New game". *Done 2026-11-17:* a game of yours is a content document
   (`game-N.game`: its name, its template, its levels) and New Game in the
   levels menu makes one on Arena or Adventure. *Changed 2026-11-17 (issue 2):*
   New Game opens a picker of game types (`coax/picker.coax`), the same nine
   cards as gamebuilder.org's plus the arena, adventure and open field, from
   one list (`shared/game-types.coax`) that the website reads too; a type's
   starter things, palette and bounds are there, and `scripts/check.sh` fails
   if the two sides make a type differently or a second list grows. The nine
   are built and saved in the app. *Step 2, 2026-11-17:* the meadow's four
   (third person, first person, top-down, side-scroller) play in the app:
   you alone on what was built, its balls solid, through that type's camera
   kind from the engine's `cam/kinds.coax` (top-down fixed high and never
   turning, side-on level beside you holding you on its line with
   `cam_kind_line`, W jumping); drawn with the app's boxes until the engine's
   renderer comes to the desktop. Balls are not yet rolling bodies there. The
   village and the worlds of their own still say in words that they play on
   gamebuilder.org for now. Each website type's own kinds (pad,
   spikes, shard, checkpoint, moving platform and lift; wall and crate pad; boost and cone) are the
   app's too, in its palette and inspector, and its limits (how far, how
   high, how big, how many) are held and said in gamebuilder.org's words
   with their numbers on both sides; `list-palette` and the Agents pane
   tell an agent the type, how building goes in it and those limits. The
   village's buildings are kinds of its palette and things of its level,
   drawn in the app by the website's scene on the engine's renderer
   (`docs/web-world.md`, "The medieval village"). The games that ship still
   come from code; renaming a game waits for the inspector or an agent's
   cell edit.
   *Added 2026-11-17 (owner's request):* a third template, **Open Field**
   (`field` in a game's document), extracted from the arena: the arena's
   first-person body, camera, stick and buttons, blaster and look (plates,
   dusk sky, wall and cover tints, the Scrappers' models), with the match
   taken out (no teams, modes, clock, packs, goals, hoops, ball or Scrapper
   picker). Both templates share one play loop; `play_arena_look` and
   `p.field` in `coax/play.coax` are the seam. Its starter place
   (`ed_open_field`) is a wide open floor with your start, a little cover,
   five targets and three enemies, and its palette is block, start,
   target, moving target and enemy: three new marker kinds (`target`,
   `moving-target`, `enemy`), at most 16 targets and 9 enemies, refused in
   words with the number past that. A target pops back up three seconds
   after it is shot; a moving one slides 2.5 m either way. An enemy is the
   arena's bot, coming in at its marker and back there three seconds after
   a knockout, as you are at your start. The HUD shows health, targets
   down of those in the field, enemies knocked out and standing, and your
   falls. It is the natural base for a free-for-all or survival template
   later (more of what the owner called "Battle Royale").
2. **Palette, markers and inspector.** Start with the arena's markers, and
   place health packs, goals and hoops instead of working them out.
   *Core done 2026-11-17* (`coax/palette.coax`): the palette by template,
   six marker kinds (health pack, each team's goal and hoop, the ball) that
   a new arena places and play uses, the inspector's fields with refusals,
   and renaming a game. *UI done 2026-11-17* (`coax/panel.coax`): a side
   panel drawn in Coax by the shared component kit, on the shared UI host's
   view, beside the viewport on the Mac and iPad and along the bottom of an
   upright phone. Its add buttons and the inspector's − and + steps go
   through the same calls an agent's command does, and a refusal is said on
   the panel in words.
3. **Arena tables.** Move modes and classes into content tables, add a
   table editor, and have play read the tables.
   - **Checkpoint:** Scrappers Arena rebuilt from New game, in the editor,
     plays the same as today's.
4. **Realm scripts in the arena.** Run the scripts in play with an event
   table. An agent builds them with the local toolchain on the developer
   Mac build, and they land as `scripts` rows.
   - **Checkpoint:** in the agents pane, "add king of the hill: a zone in
     the middle that scores a point every three seconds a team holds it
     alone" gives a working new mode. It's one zone marker, one script and
     two event rows, undone in one step.
5. **New API rows** in openagents (spawn, score, items, marker on/off,
   sound), pinned here. Then the compile job deployed, so that phones can
   build scripts too.
6. **Engine renderer and assets** (port plan S2), then glTF import, then
   the Unreal importer.
7. **The adventure template**, from `her.coax`, `scene.coax` and
   `farm.coax`: movement, items, pickups, a companion and travel, as
   markers, tables and scripts. The raider and robot scripts are
   SPLICEBOTS' first.
8. **Several maps in one game.**
   - **Checkpoint:** the SPLICEBOTS farm, rebuilt from New game on the
     adventure template, plays the jam's opening, with charge points and
     feeding Digger written as scripts by an agent.

Steps 1 to 4 also give the agents pane its reason to exist: an agent that
can add markers, edit tables, write scripts and lay out a level can build a
whole arena game, including rules we never wrote, from a sentence.

## Questions for the owner

1. **The compile job.** Should we deploy openagents' `oa-script-job` for
   GameBuilder's phones, or keep script building on the desktop until
   there's a reason to deploy it?
2. **Who may publish scripts in a game.** Owners only at first, as with
   levels? I assume yes.
3. Should the palette, inspector, table editor and scripts pane be drawn in
   Coax on the shared UI host, like the agents pane? I assume yes, per the
   no-Swift rule.
