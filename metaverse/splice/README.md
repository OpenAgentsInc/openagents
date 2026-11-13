# Splice

An unlisted OpenAgents Metaverse Engine game at `/worlds/splice`: a robot smuggler scavenges
banned parts, splices them into a Digger in the rover's edit bay, and restores a scene the
Emperor erased. The public page is `apps/openagents-web/realms/splice/index.md`. This file is
for people and agents who change the code.

## Files

| File | What it holds |
| --- | --- |
| `splice-game.coax` | The game: tuning, world, rules, interface, and checks that play the whole run |
| `splice.coax` | The page's frame, the still scenes, and the host's exports |
| `splice-cast.coax` | The five characters of the slice |
| `gpu/splice.coax` | The shaders: Anime Lab's cel pass, the placed pieces' instances, and the ink overlay |
| `splice-face.c`, `splice-ue-face.c` | The C faces for the browser and for a native host |
| `content/splice.content`, `content.coax` | The content tables and their reader |
| `pieces.coax` | Which engine kit piece a row draws, and its measured footprint |
| `layer.coax` | The colliders (a World of entities) and the edit layer |
| `layer-draw.coax` | The layer's draw: a mesh for each kind, a row for each placed piece |
| `walk.coax` | The walk replay: the second check of a layer |
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
| Durability | Nothing durable: a run is over when the page closes |

An undo, a content rollback or a layer load changes the content and the run in
progress, and nothing else.

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

Edits and saves name a piece by its content ID. Names are for people and agents to read.

A footprint is how far a piece's body reaches to each side and ahead, below 2 m. A check measures
every piece's triangles and compares them with its row, so a collider is never wider than what
you see. The collider is the stadium inside that footprint, so it turns with the piece.

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

Slice 0 is the layer itself. A saved world document, the API, the command line and the chat pane
are the later slices of [the plan](../../docs/audits/2026-11-12-chat-and-world-building.md). The
Unreal host draws no placed pieces yet.
