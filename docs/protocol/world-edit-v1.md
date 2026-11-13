# A world's edits v1: one edit, one batch, one event

Issue: [#13412](https://github.com/OpenAgentsInc/openagents/issues/13412), the
browser editor's E3 ([spec](../editor/2026-11-12-browser-editor.md)).
Code: `metaverse-engine/editor/world-edit.coax` (the one codec),
`coax/web/api/worlds.c` with `coax/web/api/world-policy.coax` (the routes and
their rules, `#authority`), `metaverse/splice/world.coax` (Splice, the first
world).
Profile: `proto/world/edit-v1.json`, checked by
`coax/web/tool.sh metaverse-engine/tools/world-edit-profile.coax proto/world/edit-v1.json`.

## An edit

An edit is one JSON object. Places are whole centimetres, turns are whole
degrees (-179 to 180) and sizes are whole percent (25 to 400). An entity is
named by its ID, 1 and up. A kind is a content ID.

| Op | Bytes | What it does |
| --- | --- | --- |
| `spawn` | `{"op":"spawn","id":0,"kind":K,"x":X,"y":Y,"z":Z,"yaw":DEG,"scale":PCT}` | Makes an entity. ID 0 asks the world to number it |
| `despawn` | `{"op":"despawn","id":ID}` | Removes one |
| `set` | `{"op":"set","id":ID,"col":COLUMN,"v":N}` | Sets a column of the World: `x y z yaw vx vy vz radius height hp hp_max kind row` |
| `move` | `{"op":"move","id":ID,"x":X,"y":Y,"z":Z}` | Moves one |
| `rotate` | `{"op":"rotate","id":ID,"yaw":DEG}` | Turns one |
| `scale` | `{"op":"scale","id":ID,"scale":PCT}` | Sizes one |
| `cell` | `{"op":"cell","table":T,"row":ID,"col":COLUMN,"v":TEXT}` | Sets a cell of a content table. Column `*` sets the whole row (`ID|CELL|...`), or drops it when `v` is empty |
| `terrain` | `{"op":"terrain","brush":B,"x":X,"z":Z,"r":R,"v":N,"mat":M}` | Shapes the land: `paint`, `raise`, `lower`, `smooth` or `flatten` |

A reader refuses any bytes but the ones the writer writes: the keys in this
order, no spaces, every number in range.

A world takes the ops its own code allows and refuses the rest in words.
Splice takes `spawn`, `despawn`, `move`, `rotate`, a `set` of `x`, `z` or
`yaw`, and a `cell` of its content tables. It refuses a size and the land, and
says why. WorldBuilder takes every op of a thing and the land, and refuses a
`cell` ([WorldBuilder's world](#worldbuilders-world)).

### A cell of the content tables

A `cell` edit changes one cell of the world's content tables, or with column
`*` a whole row. It is an edit of the same log as a move: it has a revision,
an event, an inverse, and undo and redo walk it.

- The tables are saved beside the lane's document: dev's as they are edited,
  live's as a publish made them. Until a cell is edited, a world's tables are
  the ones its build shipped with.
- The server checks a cell the way a content document is checked
  (`metaverse-engine/editor/content-edit.coax`): the column takes the value,
  the tables' rules hold, the realm reads them, and every placed thing still
  stands where the realm's own check lets it. A refusal is in words.
- The tables keep the revision they shipped with. A cell edit never changes
  what a build reads, so no page is told to reload.
- A page reads the lane's tables after its document
  (`GET /api/worlds/<id>/content?lane=dev`, or without the lane for live) and
  carries a `cell` event out in place.
- To put a world's tables in the repository, save the answer of
  `openagents realm world ID --json` (its `content`) as the realm's content
  file.

### WorldBuilder's world

Code: `apps/openagents-web/worldbuilder-world.coax` (#13422, the browser
editor's E13).

A WorldBuilder world (`/create-world`) is kept in the visitor's browser and
has no account, so no server holds it and it has no route. The page's own Coax
checks each edit with this codec and carries it out on the engine's edit log
(`metaverse-engine/editor/edit.coax`), which keeps its inverse. A batch is
fenced by the world's revision, is taken whole or not at all, and is one step:
one undo takes it back. Each edit taken, undone or redone moves the revision
by one. The chat's edits of things, the editor shell's batches and the
viewport's drags all come this way.

| Op | What WorldBuilder does |
| --- | --- |
| `spawn` | Places a thing. `id` is 0: the world numbers it. `kind` is the model's place in the pack, from 1. The thing stands on the ground, so `y` is not read |
| `despawn`, `move`, `rotate`, `scale` | Removes, moves, turns or sizes a thing |
| `set` | `x`, `z`, `yaw` or `kind`. A `y` is refused (a thing's height is the ground's), and so is any other column |
| `cell` | Refused: a WorldBuilder world has no content tables |
| `terrain` | One dab of a brush on the ground. `brush` is `paint`, `raise`, `lower`, `smooth` or `flatten`; `x`, `z` and `r` are its middle and radius in centimetres (100 to 4,000); `v` is how hard it presses, 1 to 100 percent; `mat` is the material painted: 0 the bare grid, 1 grass, 2 dirt, 3 sand, 4 stone, 5 snow, 6 water |

At 100 percent one dab raises or lowers the middle by 47 cm, paints fully,
smooths by two thirds and levels by a third toward the height at its own
middle. A brush held down dabs 30 times a second. A stroke's dabs are one
step, however many revisions they are.

Its rules, each refused in words: a world holds 4,096 things; a kind is one of
the pack's models; a brush is 1 m to 40 m wide and acts on the ground, 256 m a
side; a batch changes things or shapes the ground, not both; the world has no
live lane, so a publish is refused. The day's shared $50 of AI and a visitor's
pace are the API's rules for the chat line (`coax/web/api/worldbuilder.c`) and
are unchanged.

Not typed edits: the chat's ground commands that are no disc (the whole
ground, a strip along an edge, a level given in metres). They are steps of the
same log and move the revision, but a `terrain` edit cannot say their shape. A
later version of this shape could carry a box and a level.

A world kept before this change loads as it was (the save's bytes, `WB1`, are
unchanged). A thing whose size is outside 25 to 400 percent keeps it until an
edit sizes it. The undo history is not kept with a world, as before.

## A batch

```json
{"schema":"world.edit.v1","rev":41,"do":"apply","edits":[EDIT, ...]}
```

`rev` is the dev revision the batch was made from. `do` is one of:

| `do` | Edits | What it does |
| --- | --- | --- |
| `apply` | 1 to 64 | Carries the edits out, in order. One refused edit refuses the whole batch |
| `undo` | none | Takes back the newest step that stands |
| `redo` | none | Takes the next step again |
| `original` | none | Removes every placed thing. An undo brings them back |
| `publish-now`, `publish-next` | none | Makes dev live ([dev and live](world-live-v1.md)) |

Each saved edit moves the revision by one. An undo is saved as edits of its
own (the inverses), so every reader follows it the same way.

## The routes

| Route | Answer |
| --- | --- |
| `GET /api/worlds/ID?lane=dev` | 200 and the state with the lane's document |
| `GET /api/worlds/ID` or `?lane=live` | The live revision. With `&engine=E&rules=R&content=C&wire=W` the page is given the revision its build reads, or 409 to reload. 404 until a publish |
| `POST /api/worlds/ID` | A batch. 200 and the state with the batch as taken (every spawn numbered) |
| `GET /api/worlds/ID/events?lane=L&after=N` | The lane's events after revision N, one a line, 200 at most |

The state:

```json
{"schema":"world.state.v1","id":"splice","lane":"dev","rev":43,"build":[1,1,2,1],"dev":43,"live":3,"when":"now","undo":2,"redo":1,"doc":"world.doc.v1 splice\n..."}
```

`rev` is the lane's revision. `build` is the pins of the build that reads it
(engine, rules, content, wire), so every answer names the world, the revision
and the build it touched (#13437). `undo` and `redo` count the steps that can be
taken. `doc` is the lane's [document](world-doc-v1.md) on a read; `did` is the
batch as taken on a write.

A refusal is `{"error":WORDS,"edit":I}`. `edit` names the edit of the batch
that was refused, when one was.

| Status | When |
| --- | --- |
| 400 | The body is not a batch |
| 401, 403 | Nobody is logged in; the caller may not author this world; the write came from another site's page |
| 404 | No world has this ID; nothing is published yet |
| 409 | The batch names another revision than the world's; the page must reload |
| 422 | An edit may not stand (the words say why, naming the zone or the place); nothing to undo or redo |

A world's ID is a lowercase letter or a digit, then lowercase letters, digits
and `-`, 48 bytes at most: a realm's slug is one.

Splice (`splice`) is written by admins only. Anyone reads it.

## A realm's world

Every realm with a board is a world of kind `realm` under its slug (#13422,
migration 0185): a made realm from the moment it is made, and a hand-built
one (The Grid is `/api/worlds/the-grid`). Its owner, its own admins and the
site's admins write it; a player of the game is known by `X-Verse-Player`, as
on the realm routes. A realm hidden from the caller is no world to them.

- Its document is empty: it places nothing. `GET /api/worlds/ID/content` is
  its board and its settings as two content tables, `rows` (`id key:text
  on:int label:text`, in the board's order) and `tools` (`id name:text
  on:int`).
- A batch is `cell` edits of `on`, `0` or `1`. The realm's own SQL
  (`verse.edit_switch`, `verse.edit_settings`) carries each out, so its
  refusals are the ones the realm's page meets. Anything else is refused in
  words, at its edit (`metaverse/edit/realm-world.coax`).
- Undo and redo take a batch back as switches. It has one lane: a read of
  live is answered with dev, and a publish is refused, since a switch reaches
  everyone at once.

## The server checks with the page's code

The API links the world's own Coax and runs it on every batch: Splice's check
(`sl_check`), then the walk of the whole run against the layer as it would be
saved (`sw_replay`). The page runs the same files. So the server and the page
cannot disagree about where a piece may stand.

## Events

A saved edit is the domain event `realm-authoring.world.edited.v1`, one a
revision of the dev lane, written to `verse.world_outbox` in the transaction
that changes the document:

```json
{"id":"realm-authoring.world.edited.v1","seq":912,"world":"splice","rev":42,"edit":EDIT,"at":1762190000000}
```

A reader at revision N takes the event whose `rev` is N+1 and no other
(`we_take`). A publish is `realm-authoring.world.published.v1` on the live
lane.

### Steps

A step is one batch, so one undo step: an `apply`, an `undo`, a `redo` or an
`original`. Every event of one batch carries the batch's `at`, and its `seq`
rises. A reader finds where a step begins without a field of its own (#13485):

- where `at` changes. The API gives each batch of a world an `at` above the
  last batch's, one millisecond later when the clock has not moved
  (`worlds.c` `step_at`), so two steps never share one.
- where `seq` does not rise past the last event's. The dev server numbers
  `seq` from the clock, one an edit, so a batch saved in the same millisecond
  as the last one starts with a `seq` that is not above the last one's.

An undo and a redo are saved as edits, so a reader tells them from the steps
(`metaverse-engine/editor/world-changes.coax`): a redo repeats a step that was
undone, edit for edit; an undo is the newest standing step's inverses, last
first. A step that began before the events a reader holds can't be told.

The editor's top bar says what the newest step changed beside the revision,
and the revision opens the Revisions list: the world's recent steps, newest
first, each with its revisions and its changes (`shell-revs.coax`). It reads
the dev lane's newest 200 events again each time the revision moves.

## A page follows the world

Code: `metaverse-engine/editor/world-follow.coax` (what to ask, and what an
answer is), `metaverse/splice/world.coax` `spw_page_*` (Splice carries a
document and each event out).

1. The page opens a lane: dev with `?edit`, else live. It reads the lane's
   document once. The run goes on as it was: where you stand, what you carry
   and the stage are not touched.
2. It asks for the lane's events and carries each out in order through its own
   edit log, so UNDO works on an outside edit as on its own. A placed piece
   rises out of the ground.
3. An event out of order, or one the layer can't take, sends the page back to
   the document.

| Lane | How often the page asks |
| --- | --- |
| dev | Once a second, and at once when the dev server says the world was written |
| live | Every 5 seconds. Every 30 while nothing is published |

## On the dev server

`coax serve` answers `/api/worlds/...` itself, so a page, a second tab, the
CLI and `coax drive` share one world before anything is deployed. The request
goes to `metaverse/splice/tools/world-local.coax`: the API's rules
(`world-policy.coax`) and Splice's own check, with files under
`apps/openagents-web/build/.worlds/` for rows. Everyone on this computer is
the admin. A saved write is told to every tab over the live channel
(`{"kind":"world","id":"splice"}`, inbox kind 614), and each tab asks for the
new events in its next frame.

Check it with two headless tabs:

```sh
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538
coax/web/tool.sh world-tabs http://127.0.0.1:9538
```

It places a piece from one tab and from outside any page, undoes one, and
requires the other tab to show each inside a second with its run as it was.

## From the command line, MCP and `coax drive`

The verbs are rows of `REALM_COMMANDS` (`apps/openagents-cli/commands.coax`),
the table the other realm commands come from. The CLI command, the `realm_*`
MCP tool and the `coax drive` verb of each row are that one row. There is no
second `world_*` family.

| Command | MCP tool | `coax drive` | What it does |
| --- | --- | --- | --- |
| `realm world ID` | `realm_world` | `world` | The revision and build, the placed things by number, the content tables (pieces, landmarks, kept-clear zones) and the reference page |
| `realm world-place ID KIND X Z [--yaw DEG]` | `realm_world_place` | `place KIND X Z` | Places a thing. KIND is a content ID |
| `realm world-move ID N X Z` | `realm_world_move` | `move N X Z` | Moves thing N |
| `realm world-turn ID N DEG` | `realm_world_turn` | `turn N DEG` | Turns it |
| `realm world-set ID N COLUMN VALUE` | `realm_world_set` | `set N COLUMN VALUE` | Sets a column |
| `realm world-remove ID N` | `realm_world_remove` | `remove N` | Removes it |
| `realm world-undo ID`, `world-redo ID` | `realm_world_undo`, `realm_world_redo` | `undo`, `redo` | Walks the world's undo log |
| `realm world-original ID` | `realm_world_original` | `original` | Removes every placed thing |
| `realm world-apply ID FILE.jsonl` | `realm_world_apply` | `apply FILE.jsonl` | Edits as JSON lines, one batch |
| `realm world-publish ID now\|next --yes` | `realm_world_publish` | `publish now --yes` | Makes dev live |
| `realm world-why ID THING` | `realm_world_why` | `why ID` | Why a thing isn't seen, reached or working |
| `realm world-open ID` | `realm_world_open` | | The address of the page that edits it |
| `realm world-foes ID` | `realm_world_foes` | `foes` | The editor's time of day, each actor by number and name with its plan at dawn, day, dusk and night, and the places a route may name |
| `realm world-plan ID FOE WHEN DO [--route "X Z, X Z"] [--speed CM/S] [--notice CM] [--on chase\|call\|flee\|ignore]` | `realm_world_plan` | `plan FOE WHEN DO` | Sets what actor FOE does at WHEN. Without `--route` the old route is cut or grown to fit the new DO. One edit: the plan's whole row |
| `realm world-route ID FOE WHEN "X Z, X Z"` | `realm_world_route` | `route FOE WHEN X Z, X Z` | Sets actor FOE's route at WHEN |
| `realm world-time ID story\|dawn\|day\|dusk\|night` | `realm_world_time` | `time WHEN` | The editor's time of day: the light and every actor follow it. Play keeps the run's own clock |
| `realm world-board ID on\|off [--casual CM/S] [--fast CM/S]` | `realm_world_board` | | Gives the player a hoverboard or takes it away, with its casual and very fast speeds (100 to 6000 cm/s; the fast one faster). What you leave out stays. One edit: the board table's whole row |

Places are whole centimetres. Each write answers
[one result](realm-result-v1.md): the new revision, or the refusal in words.
A plan, route or time the world already holds answers done at the revision it
stands at, with nothing sent.
`--shot OUT.png` adds a frame of the page at the edit's revision.
`--dry-run` carries the batch out in a copy and saves nothing.

Not built: `new`. A world gets its ID from a migration today (Splice).
WorldBuilder's worlds are kept in the browser and have no ID on the server
(E13, #13422), so these commands do not reach them.

### An agent edits and checks in one session

```sh
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538 --build true
coax/web/tool.sh drive web apps/openagents-web /worlds/splice --query edit --live http://127.0.0.1:9538 \
  "world" "place 113 17720 -7060" "where" "why piece-0" "screenshot out.png"
```

With `--live` the driver uses the dev server that is running, so the page is
the one that reloads when code, shaders or content change. An edit is sent for
the revision the page stands at: a page that shows another revision is
refused, never edited. The driver waits until the page stands at the new
revision and says so.
