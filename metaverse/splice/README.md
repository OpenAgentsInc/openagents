# Splice

An unlisted OpenAgents Metaverse Engine game at `/worlds/splice`: a robot smuggler scavenges
banned parts, splices them into a Digger in the rover's edit bay, and restores a scene the
Emperor erased. The public page is `apps/openagents-web/realms/splice/index.md`. This file is
for people and agents who change the code.

## Files

| File | What it holds |
| --- | --- |
| `splice-game.coax` | The game: tuning, world, rules, interface, and checks that play the whole run |
| `front.coax`, `front-checks.coax` | The front door: the title, character creation, the credits, the run this browser keeps, and you drawn as your character |
| `splice.coax` | The page's frame, the still scenes, and the host's exports |
| `splice-cast.coax` | The five characters of the slice |
| `gpu/splice.coax` | The shaders: Anime Lab's cel pass, the placed pieces' instances, and the ink overlay |
| `splice-face.c`, `splice-ue-face.c` | The C faces for the browser and for a native host |
| `content/splice.content`, `content.coax` | The content tables and their reader |
| `pieces.coax` | Which engine kit piece a row draws, and its measured footprint |
| `layer.coax` | The colliders (a World of entities) and the edit layer |
| `world.coax`, `world-api.coax` | Splice as the world `splice`: its document, its edits as the wire carries them, and what the API calls |
| `layer-draw.coax` | The layer's draw: a mesh for each kind, a row for each placed piece |
| `walk.coax` | The walk replay: the second check of a layer |
| `why.coax`, `why-checks.coax` | The answers to `coax drive ... "why ID"` and what `tree` lists: the diagnostic chain walked from the game's state (#13429) |
| `layer-fixture.coax` | A fixture layer: a piece beside every kept-clear zone |
| `layer-checks.coax`, `walk-checks.coax` | The checks |
| `splice-export.coax`, `splice-run.aescript` | The Unreal exporter and its scripted run |

Run the checks:

```sh
coax/bin/coax test metaverse/splice/splice.coax                 # everything, about 10 seconds
coax/web/tool.sh metaverse/splice/tools/content-check.coax metaverse/splice/content/splice.content --check
coax/remote/remote.sh run -- coax/remote/check.sh module splice   # the module builds inside its budget
```

## State and authority

Splice's row in `metaverse-engine/editor/authority.coax` (#13431,
[what the kinds are](../../docs/architecture/state-and-authority.md)):

| Kind | Who decides |
| --- | --- |
| Content (the content tables, the edit layer) | The realm's owner and the site's admins |
| Simulation (the run in progress) | The player's own page; no server checks it |
| Durability | This browser only: your character (the crew page's record) and the last checkpoint of a run in the mine, in IndexedDB. Nothing reaches a server |

An undo, a content rollback or a layer load changes the content and the run in
progress, and nothing else.

## The front door

The page opens on the title screen (GitHub #13482). Each front screen is the engine's one
screen format (`metaverse-engine/ui/screen.coax`): a dark panel down the left over the live
world, the kicker and title at its top, the rows under them, the key hints at the bottom right.
Rows take the keys (up and down or W and S to move, Enter or Space to pick, A and D or left and
right to change a value, Escape or Backspace to go back) and the pointer (hover focuses, a press
picks).

| Screen | What it does |
| --- | --- |
| Title | The rover drives the rail at dusk and the camera follows the `dolly` table past it. The rows are the `menu` table's: CONTINUE (shown only when this browser keeps a run, with how far it got), NEW RUN and CREDITS |
| Character | Body, outfit, height, skin, hair and face on the crew document, shown on a turntable. USE THIS CHARACTER keeps it in the crew page's record (`/worlds/crew` reads the same one) and starts a run |
| Credits | The `credits` table's lines |

There is no SETTINGS row: Splice has no sound and nothing a setting would change yet.

You play as your character. `metaverse-engine/kit/crew-player.coax` writes ten poses of a walk
cycle from the crew document (the legs and arms swing at the hips and shoulders) into the piece
store after the placed pieces' meshes, once a character; each frame draws one pose as an
instance at the smuggler's place. The smuggler's gameplay is unchanged: the same circle, speed
and stride.

A run is kept from the mine's gate: each second in the mine the checkpoint's inventory and what
is open (the rubble, the boss wall, the banned bot, the core) is written as `splice.run.v1` when
it changed. CONTINUE puts you back at the checkpoint with them. SCENE RESTORED and a new run let
it go.

## Edit layer

You can place things in Splice without changing its code. A placed piece is data: a row of a
content table, an entity in the engine's World, and a typed edit you can undo.

### Content tables

`content/splice.content` is an `aethyr.content.v1` document
([profile](../../docs/protocol/aethyr-content-v1.md)). Edit it with the Metaverse Engine's table
editor:

```sh
coax/web/tool.sh metaverse-engine/tools/content-edit.coax metaverse/splice/content/splice.content row landmarks '16|Dance floor|-160|-5120|800'
coax/web/tool.sh metaverse/splice/tools/content-check.coax metaverse/splice/content/splice.content --carry
```

The second command checks the tables against Splice's rules and writes `content/shipped.coax`,
the copy the page is built with. Run it after every edit.

| Table | What a row is |
| --- | --- |
| `pieces` | A placeable piece: a stable content ID, a name, the engine kit piece it draws, its size, its look, its footprint and its height |
| `looks` | Three colours of Splice's palette. A table of its own, because a content table has eight columns at most |
| `landmarks` | A named place in centimetres, with a radius. The game reads its places from here, not from constants |
| `kept_clear` | A zone the run needs: a box, grown by a radius. Nothing may be placed in one |
| `areas` | Where a piece may stand at all: the dome village, the gate corridor, the rail stop and the mine |
| `menu` | A title row: its key (`continue`, `new` or `credits`) and its label. The rows' order is the table's |
| `credits` | A line of the credits screen |
| `dolly` | A point of the title camera's path past the rover: the eye and where it looks, in centimetres in the rover's frame, and seconds to the next point |
| `portals` | A way from one place to another: E at any placed piece of a kind (`kind K`) or at a point (`at X Z`) takes the player to `X Z`, facing a yaw. A vehicle that opens onto a room inside it is a portal from its kind ([the module](../../metaverse-engine/world/portal.coax)) |
| `starts` | Where a player starts at a time of day. The row with the lowest ID is where a new run starts, at its time of day; a respawn at a time of day with a row goes to that row ([the module](../../metaverse-engine/world/flow.coax)) |
| `beats` | Where the day moves on: standing in an area while it is `from` makes it `to`, never earlier |

Splice now has 16 tables, the most a content document holds. A new kind of row goes in an
existing table or needs the format's limit raised first.

Edits and saves name a piece by its content ID. Names are for people and agents to read.

A footprint is how far a piece's body reaches to each side and ahead, below 2 m. A check measures
every piece's triangles and compares them with its row, so a collider is never wider than what
you see. The collider is the stadium inside that footprint, so it turns with the piece.

### Foes' plans

The foes move by data, not by code (#13466). The table `plans` gives each of the nine actors
(`actors`: Raider 1 to 5, Imperial 1 to 4) a plan for dawn, day, dusk and night, rule-based with
no model call at run time ([the module](../../metaverse-engine/ai/npc-plan.coax),
[its steps](../../metaverse-engine/ai/plan.coax)):

| Column | What it holds |
| --- | --- |
| `do` | `stand`, `dance`, `sleep`, `guard`, `patrol`, `wander` or `follow` |
| `route` | Points in centimetres: one for stand, dance, sleep and wander (`X Z R`), the post and where it looks for guard, 2 to 8 for patrol, `foe N` for follow |
| `speed`, `notice` | Centimetres a second, and how near you come before it notices (0: never) |
| `on` | What it does on noticing you: `chase`, `call` (the others near it chase too), `flee` or `ignore` |

The shipped rows are the movement Splice had in code, so play is unchanged. The table `clock` holds
the editor's time of day (`story` in play). A route stays in the mine or in an area added after it
(ID 5 and up, such as a desert), and a point inside a wall
is refused with the nearest place a foe can stand. `world.coax` checks both on every cell edit.

An agent edits plans with the realm commands (`openagents realm world-foes`, `world-plan`,
`world-route`, `world-time`; [the table](../../docs/protocol/world-edit-v1.md)). A behaviour the
plans cannot express goes in a realm script first: `scripts/raider-desert.coax` makes raiders
hostile only while the player is in the desert (`coax/web/script.sh`, then `world-script-compile`
and `world-script-run --for "kind 1"`). Anything else the
plans cannot express is a change to `splice-game.coax` (`g_foes_step`) on a dev server, which
rebuilds and swaps the engine on save and keeps the world's plans and state.

### Edits

`layer.coax` holds the layer as a scene in the engine's edit log (`metaverse-engine/editor/edit.coax`). Places
are whole centimetres and turns are whole degrees.

| Call | What it does |
| --- | --- |
| `sl_place(g, piece, x_cm, z_cm, deg)` | Places a piece. Returns its item ID |
| `sl_move(g, id, x_cm, z_cm)`, `sl_turn(g, id, deg)` | Moves or turns a placed piece |
| `sl_remove(g, id)` | Removes one |
| `sl_undo(g)`, `sl_redo(g)` | Walk the log |
| `sl_why(g, verdict, out)` | A refusal in words |

Each edit is checked before it is taken. An edit is refused when the piece:

- has no row in `pieces`;
- reaches past the map's edge (it must stand inside one area);
- reaches into a kept-clear zone (the refusal names the zone);
- stands on the player;
- stands inside a wall, the rover, the banned bot or another placed piece.

A placed piece is scenery. It blocks you, the party's people and the camera, and it has no AI and
no hit points.

### The world `splice`

Splice is the first world with an ID ([edits](../../docs/protocol/world-edit-v1.md),
[the document](../../docs/protocol/world-doc-v1.md), [dev and live](../../docs/protocol/world-live-v1.md)).
`world.coax` turns a wire edit into the calls above and answers its inverse. The API
(`/api/worlds/splice`) runs the same file on every batch, then the walk replay, and saves the
document a fresh game builds from the pieces. Only an admin writes it.

| Call | What it does |
| --- | --- |
| `spw_edit(g, e, sure)` | One wire edit on the layer. `sure`: the server saved it already, so a page takes it where it stands |
| `spw_batch(...)` | A batch on a document: the new document, the batch as taken and its inverse, or the refusal |
| `spw_open(g, k, text)`, `spw_save(g, k, out)` | A document read into a game and written from one |
| `spw_page_open`, `spw_page_answer` | A page follows the world: the document once, then each saved edit in order. The run is not touched |

Open `/worlds/splice?edit` on the dev server to join the dev lane. An edit from another tab or
from outside shows in under a second, and a placed piece rises out of the ground.
`coax/web/tool.sh world-tabs http://127.0.0.1:PORT` checks it with two headless tabs.

### How it is built

- **Colliders.** Everything that blocks is an entity in an engine World (`metaverse-engine/core/world.coax`):
  the 30 walls, the rubble and the boss wall, the banned bot and every placed piece. A step asks
  the World's spatial index for what stands near, and the engine's walking rules push the walker
  out. No list is scanned.
- **Order.** The World's placed pieces are built from the layer's items in ID order each time the
  layer's revision changes. `sl_digest` is the World's digest plus Splice's shape columns, so two
  games given the same edits compare equal a step at a time.
- **Draw.** Each kind's mesh is written once and stays on the GPU. A placed piece is one row of a
  small store: where it stands and its turn. A kind's placed pieces are one instanced draw. An
  edit writes rows. It doesn't write the designed world's buffer or any mesh.

### The walk replay

`walk.coax` is the second check, made when a layer is saved. A walker goes the run's whole way on
foot, 46 legs from the dome village to the banned bot, against the proposed layer. It uses the
engine's steering service to go round things and the game's own step and collision. It refuses a
layer when:

- a place the run starts you in is taken;
- a place the run needs can't be reached;
- something stands on the rail line or in the Digger's lane;
- something stands in the rubble or the boss wall.

The mutant check takes each of the 29 kept-clear zones out of the tables in turn, places a blocker
where that zone was, and expects the walk to refuse.

### See placed pieces

The dev server loads the fixture layer (28 pieces, one beside every kept-clear zone with open
ground beside it):

```sh
coax/web/serve.sh apps/openagents-web
# then open http://127.0.0.1:8800/worlds/splice?layer=fixture
```

Only an address on this computer (`127.0.0.1` or `localhost`) loads it. On any other address the
page ignores `?layer=fixture`.

### Not built yet

The saved world, its API, the command line and the editor with the conversation in its foot are
built: see [the browser editor](../../docs/editor/2026-11-12-browser-editor.md).

- The Unreal host draws no placed pieces. It loads no edit layer, and its bridge draws stores 0
  and 1 only (`splice-ue-face.c`).
- The Unreal host draws no player either: you are drawn from the piece store (front.coax
  `fr_player`), which its bridge does not draw. It lists the smuggler (`splice_things`) for a
  host that draws its own.
- The walk is a rigid swing of the legs and arms at the hips and shoulders: no knee or elbow
  bend, no skinning, and the feet stay flat. The smuggler's own animation does not drive the
  crew figure's joints.
- The boss chamber has no piece beside it in the fixture, because no ground beside it takes one:
  rock and the boss wall are all that is there. A check holds that (`walk-checks.coax`).
- A turned piece's round parts (a machine's balls and masts) turn their facets with the piece,
  because a kind's mesh is written once and each piece is an instance of it. The shape and the
  middle are the same either way.
