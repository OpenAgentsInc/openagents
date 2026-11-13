# The Metaverse Engine editor in the browser

Status: spec, 2026-11-12. It pulls together three sessions (the coordinator
thread `feace06f`, the Splice build `eaaf10a3`, and the Unreal comparison and
exporter `81f826d1`), the chat and world-building plan
(`docs/audits/2026-11-12-chat-and-world-building.md`), the AAA program
(#13257), and an inventory of `origin/main` at `2d99d8af62`.

The owner's goal: a browser-based editor for the OpenAgents Metaverse Engine.
Anyone (a person or an agent) can make easy edits to a world, see each change
in the world at once, keep playing while code, shaders, content and assets
reload live, and build a game programmatically.

There is no separate "Forge" product (owner, 2026-11-12). The editor is part
of the Metaverse Engine. The older Aethyrforge spec (`docs/metaverse/aethyrforge.md`)
is superseded where it differs from this one.

## The editor in one paragraph

Any Metaverse Engine page opens in edit mode with `?edit`: Splice, Everglade,
a made realm or a WorldBuilder world. The world keeps running in the middle.
Around it are an outliner of the World's entities, an inspector for the
selected entity and its content rows, and a toolbar: select, move, rotate,
scale, place, terrain, play, pause, step. Along the bottom runs the same
conversation as `/chat`, with your agent in it. Everything is built from the
`/chat` page's components.

Each change is a typed edit in one log, with undo and redo, and it shows in
the world in under a second. That includes a change made in another tab, by
your agent or from the CLI. Saving a code, shader, content or asset file
reloads that part in place. The player stays where they stand, and the game
keeps its state.

## What exists (checked on `origin/main`, 2026-11-12)

| Area | State | Where |
|---|---|---|
| Live channel | **Partial.** `serve` rebuilds on change and pushes over the `/coaxial/live` WebSocket: `wasm` swaps the engine, `wgsl` swaps shaders, `page` reloads, `error` keeps the last good engine. It watches only the top level of the app and host folders (`MAX_WATCH 256`), not shared code in `metaverse-engine/` or `components/`, packs or textures | `coax/web/serve.c`, `coax/web/host.js` `live()` |
| Outside edits on a page | **Exists for Splice (E4, #13413).** A page reads its lane's document once, then takes each saved edit in order through its edit log; the dev server answers the world's routes and tells every tab when it was written | `metaverse-engine/editor/world-follow.coax`, `metaverse/splice/world.coax`, `metaverse/splice/tools/world-local.coax`, `coax/web/world-tabs.coax` |
| Rebuild time | **Good.** A one-line edit is running in about 2.5 s (resident and incremental compiler, #13138, #13142); a dev engine build takes 118-135 ms; type errors show in 0.63 s | `coax/book/ch46-resident-compilation.md` |
| State across a code reload | **Good on the dev server (#13411).** A development module carries its state's field table. A reload that changes the state's layout keeps every field both layouts name, starts new fields from their defaults and names the new and dropped fields in the dev note. Not built: declared migrations (a renamed field) and a rollback when the first frame traps | `coax/web/state-keep.js`, `host.js` `loadEngine`, `metaverse-engine/book/ch082-change-code-while-it-runs.md` |
| One function swapped live | **Experiment.** `door_ease` only, 900 ms from edit to running | `coax/web/fast-edit.*` |
| Agents drive and see | **Exists, no edits.** `coax drive`: `tree`, `state`, `tap`, `type`, `walk-to`, `use`, `clock`, `screenshot`, `logs`, `perf`. It serves `--static`, so it has no live channel, and it has no edit verbs | `coax/web/drive.coax`, `docs/metaverse/agent-clock.md` |
| The scene model | **Partial.** The World (entities, columns, systems, spatial index) is used only by Splice's edit layer. A World saves and loads whole, and a world has an ID, a document, fenced edits and dev and live revisions (E3, #13412). No prefabs | `metaverse-engine/core/world.coax`, `metaverse-engine/editor/world-doc.coax`, `world-edit.coax`, `world-live.coax`, `coax/web/api/worlds.c` |
| Typed edits, undo and redo | **Exists** | `metaverse-engine/editor/edit.coax` |
| Content tables | **Exists, CLI only.** The editor isn't reachable from a page; the page doesn't load the tables yet (#13381) | `metaverse-engine/content/tables.coax`, `metaverse-engine/editor/content-edit.coax` |
| Picking | **Partial.** `query_ray` exists; every editor picks its own way (Everglade by 56 px screen distance) | `metaverse-engine/physics/query.coax` |
| Gizmos, outliner, inspector, fly camera, snapping, asset browser, play/simulate | **Missing** | — |
| In-world editors | **Two in the shell, two separate.** Splice's layer and WorldBuilder run in the editor shell (E13, #13422). Everglade admin EDIT and the realm edit layer with its agent pane are still their own | `metaverse/splice/world.coax`, `worldbuilder-world.coax`, `everglade-edit.coax`, `metaverse/edit/` |
| Programmatic building | **Exists for worlds with an ID (E6, #13415).** The edit verbs are rows of `REALM_COMMANDS`: `realm world`, `world-place`, `world-move` and the rest, each also a `realm_*` MCP tool and a `coax drive` verb. There is no second `world_*` family. Every row has an effect and an approval rule, and answers one result shape | `apps/openagents-cli/commands.coax`, `realm-world.coax`, `metaverse-engine/editor/op-result.coax`, `coax/web/drive.coax` |
| Code changes from a browser | **Exists on the dev server** (E12, #13421): the editor's Code pane opens and saves the realm's own sources, and the dev server checks and rebuilds them. A deployed page has no such path | `coax/web/serve.c`, `metaverse-engine/editor/code-wire.coax` |
| Agents in `/chat` | **Exists.** Claude Code, Codex, Antigravity and Microcoder through the local Coder | `apps/openagents-web/chat-web.coax`, `coder/tui/mcp_http.coax` |
| UI components | **Exist.** The frames set (frame, text, button, field, select, card, row, header, menu, notice, composer, tool call, code block), skinned by `mono.coax`, drawn by the browser through `dom.coax`: real text, Find, selection and screen readers | `components/kit/frames.coax`, `components/skin/mono.coax`, `components/backend/dom.coax` |
| Retained rendering | **Splice's layer only.** Shared retained rendering is P6.1 (#13303) | `metaverse/splice/layer-draw.coax` |

## What the sessions teach

- **The agent must see its own work.** This was the main reason the Splice
  build beat Unreal's (`docs/unreal/2026-11-12-splice-unreal-vs-aethyr-session-audit.md`).
  In the editor, every edit an agent makes returns a frame and the World's
  state, not just "ok".
- **Fail loudly.** A refused edit says why, in words, naming the rule or zone,
  as Splice's checker does. A reload that fails keeps the last good build
  running and shows the error in the editor; it never shows a blank page.
- **Seconds, not minutes.** The 2.5 s code loop and the 118 ms engine rebuild
  are the bar. The waits that hurt (`feace06f`, 11-10: a two-hour release, 84%
  of agent time spent waiting) came from release gates and builds from
  scratch, not from the dev loop. The editor runs on the dev loop.
- **Headless passing is not real Chrome passing** (`feace06f`, 11-07). The
  editor's acceptance includes a real-input run in a real browser, as Splice's
  audit recommends.
- **Never hand-roll** (`docs/reuse.md`, 11-11). WorldBuilder's broken text keys
  came from a hand-made input box. The editor uses only shared components:
  the `/chat` components for UI, and the edit log, World and content tables
  for data.
- **A non-programmer needs a way to bring in a mesh or texture.** Splice's
  audit calls this "the one reason the jam entry stays in Unreal".

## Design

### One World, one edit log, one wire

- **The scene is the World** (`metaverse-engine/core/world.coax`). The editor
  edits entities and their columns: transform, kind (a content ID), collider
  and the realm's own columns. Content rows (pieces, landmarks, creatures,
  spawns) are edited through the content tables.
- **Every change is a typed edit** in `metaverse-engine/editor/edit.coax`'s log,
  with its inverse: spawn, despawn, set a column, move, rotate, scale, set a
  content cell, paint or shape terrain. An edit is a domain event
  (`proto/registry.json`, owned by realm-authoring) with one wire shape,
  `proto/world/edit-v1`.
- **A world has an id and a document:** the World's entities, the content
  revision and the edit log's head, saved and loaded whole
  (`world_save`/`world_load`), with edits fenced by revision (409 on a stale
  one). The server checks every edit with the same Coax the page runs.

### One live channel

`serve`'s WebSocket becomes the editor's only channel in development. In
production the API's event stream plays the same part. It carries:

- **Code.** A rebuilt engine is swapped in, with the state migrated field by
  field. A field that can't migrate starts at its default, and the editor says
  which.
- **Shaders.** Any WGSL that changed, including WGSL generated from Coax.
- **Content.** A new content revision is applied in place.
- **Assets.** Packs and textures that changed are fetched again and swapped by
  content hash.
- **Edits.** Edits made anywhere (another tab, an agent, the CLI) are applied
  through the edit log, so UNDO works on them too.

What `serve` watches is what the build read: the compiler's dependency list,
not a flat folder scan.

### The editor shell

- **Opening it:** `?edit` on a page, or EDIT for an owner or admin. The shell
  loads with the page's own module; the world module never loads the chat
  module, and the chat never loads a world module (`contexts.txt`).
- **Components:** all panels come from the `/chat` page's components, drawn by
  the DOM backend over the canvas:
  - the outliner is a `k_row` list;
  - the inspector is `k_field`s and `k_select`s inside a `k_card`;
  - the toolbar is `k_button`s and `k_icon_button`s;
  - refusals are `k_notice`s;
  - the agent strip is `k_composer` and the message components.

  New component kinds go into the kit, never into the editor.
- **As built (E7, #13416):** the shell is a module of its own,
  `metaverse-engine/editor/shell.coax`, built as `editor-shell.wasm`. The
  page's host (`coax/web/host.js`) opens it over the canvas with
  `coax/web/dom-host.js` and carries bytes between the two:
  - the page tells the shell what its world holds, as `editor.model.v1`
    ([the shape](../protocol/editor-model-v1.md));
  - the shell asks the page to send a `world.edit.v1` batch or to select a
    thing.

  The shell keeps no world and decides nothing. Its panes, props and facts are
  `components/kit/dock.coax`, shown on `/components`. The editor has no look
  of its own: `coax/web/tool.sh editor-skin-check` fails when an editor file
  names a colour, a size or a face, or loads a skin other than mono. On a
  deployed page the shell opens only for an admin. Splice is the first page.
- **The agent strip is the `/chat` conversation.** The same thread, the same
  agents and the same transcript appear on `/chat` and in the editor. The
  agent gets `world_*` tools plus the editor's selection and "here" (the last
  point tapped).
  As built so far (E9, #13418): Agent in the shell's strip opens a foot whose
  slot holds the chat page's own module (`chat-web`), opened by the page's
  host, so it is the same conversation, components and agents as `/chat`.
  `?edit=agent` opens the editor with the foot open. A local agent's edits
  through the realm commands show in the open editor as any outside edit does.
  The rest ([the shapes](../protocol/world-turn-v1.md)):
  - Each turn carries the world, the selection and "here". The shell tells
    the conversation (`world.turn.v1`) whenever they change. An agent that
    works by itself is told in words after your message.
  - A Gemini turn in the editor is a hosted world turn: one model call under
    a schema whose steps are the hosted world tools (place, move, turn, size,
    remove, undo, redo). The model's typed answer chooses talk or edit and
    the tools; no code matches words. The steps go to the shell as a plan and
    leave the page as one `world.edit.v1` batch, so the server's rules answer
    it and one undo takes the turn back.
  - On `/chat`, an agent's `realm_world` tool brings the world's card
    (`world.card.v1`, `GET /api/worlds/ID/card`): its name, what it holds and
    a link that opens the editor with the conversation in its foot.

  Not built: world tools for the hosted agent on a Claude account (its runner
  has no realm commands), and a picture on the card.
- **Viewport tools:**
  - picking through `query_ray` against the World's spatial index;
  - move, rotate and scale gizmos that snap to a grid;
  - a fly camera;
  - play, pause and step on the agent clock;
  - "simulate", which runs the World without the player.
- **As built (E8, #13417):** `metaverse-engine/editor/viewport.coax` is the
  one module of the viewport's tools. A realm keeps one `Viewport`, gives it
  the pointer while a tool is picked, and reads back the selection, the edit a
  drag made and the view.
  - A tap picks with `vp_pick`, which is `query_ray` on the realm's colliders,
    and marks the point as "here".
  - Move, Turn and Size are drags on the selected thing that end as one typed
    edit. A move snaps to a grid of 10, 25, 50 or 100 cm, a turn to 15 degrees
    and a size to 5 percent.
  - Fly frees the camera: W, S, A and D move it, Q and E lower and raise it,
    and a drag looks around.
  - Pause stops the World, Step runs one sixtieth of a second, and Simulate
    runs the World with the player's hands off.

  The shell's strip picks the tool, the grid and the rest, and Add places a
  new thing at "here". `coax drive` taps and drags at points of the page
  (`tap X Y`, `drag X0 Y0 X1 Y1`). Splice is the first realm
  (`metaverse/splice/world.coax`, `spv_*`).
- **As built (E10, #13419):** where a realm names the table its kinds are
  rows of, the inspector shows the selected thing's content row as fields. A
  changed cell is a `cell` edit of the world's one log, checked by the server
  through `metaverse-engine/editor/content-edit.coax` and the realm's own
  rules, and saved with the lane's tables. Pages carry the event out in
  place. Splice is the first realm
  ([the cell edit](../protocol/world-edit-v1.md)).
- **As built (E5, #13414):** `metaverse-engine/editor/place.coax` is the one
  placement planner: an anchor, a side as the camera sees it, a spread over a
  disc or a ring, and one thing nudged to where nothing stands. It asks a
  catalogue one thing of a kind (its footprint and whether it grows).
  WorldBuilder fills that from the Quaternius pack and Splice from its
  `pieces` table, and WorldBuilder's own copy is gone. A template is a group
  of things saved as one row of a realm's `templates` table and placed as one
  batch, so one undo takes it back. In the shell, a row's control gathers a
  thing into a group, the group saves under a name, and Add lists the
  templates after the kinds and places several of a kind spread out.
- **Assets:** an asset browser over the realm's pack and the shared kits.
  Drop a glTF, PNG or audio file to add it: it is packed with a declared
  licence (`metaverse-engine/editor/pack.coax`) and appears in the browser.
- **Code:** a realm's Coax source opens in a code panel (`k_code`, plus an edit
  field from `components/core/edit.coax`). Saving sends it to the paired Coder,
  which compiles it with the resident compiler; the live channel swaps it in.
  Without a Coder, a hosted build seat does the same (later).
- **As built (E12, #13421):** code from the browser is for the owner and
  admins, on the dev server only (owner decision, 2026-11-13: realm code is
  ours, or arrives as a pull request). The shell has a Code button and a Code
  pane only when the page's host says the dev server serves the page and that
  server lists the realm's files. A deployed page has neither, and its server
  has no such route.
  - **The route.** `POST /coaxial/code` on `coax serve` while it watches
    (`realm.code.v1`, [the shape](../protocol/realm-code-v1.md)). It lists,
    reads and writes the page's own realm sources: the Coax and WGSL files
    under `metaverse/PAGE/` that the page's module or its shaders were built
    from. It refuses every other path, a link, another site and another
    machine.
  - **The pane.** A select of the realm's files, the open file in a code field
    (`k_code_field`, new in the kit and shown on `/components`), and Save. The
    pane is a wide right pane of the dock (`DK_WIDE`). The field is the
    browser's own text area, as every field of the frames set is on the
    browser backend; `components/core/edit.coax` is the edit buffer for
    surfaces the GPU draws, and is not used here.
  - **Save and compile.** A save writes the file and answers with
    `coax check` of the module the file is part of. On Splice the answer came
    in 110 ms with errors and 684 ms without. Errors show as a notice with the
    compiler's words in a code block. The dev server's watch then builds the
    module and the live channel swaps the engine with its state kept (E1,
    E2): 1.9 s from Save to the new engine on Splice. A failed build keeps
    the last good engine running.
  - **Shaders.** `metaverse/splice/gpu/splice.coax`, the Coax that writes
    Splice's WGSL, is in the list. Saving it writes the WGSL again and the
    live channel swaps the shaders.
  - **Not built:** the paired Coder for a deployed page, and a hosted build
    seat for people without a Coder. The realm origin (#13432) is not wired
    in.
  - Checks: `coax/web/tool.sh code-route-check`, the shell's and the codec's
    own checks, and `proto/world/code-v1.json`.
  - Proved in headless Chrome on the dev server
    ([picture](shots/2026-11-13-e12-code-pane.png)): a changed line of
    `splice-game.coax` was live 876 ms after Save with the player where they
    stood and the world's revision unchanged; a compile error showed in the
    pane in 104 ms and the last good build kept running; a changed shader
    constant in `gpu/splice.coax` was swapped in 453 ms after Save.

- **As built (E13, #13422): WorldBuilder in the shell.** `/create-world?edit`
  (the page's EDIT button) opens the shell over WorldBuilder, for anyone: the
  world is the visitor's own, kept in their browser.
  - **Its world is behind `world.edit.v1`.** Every edit of a thing and every
    dab of a brush is a typed edit, checked by the page's own Coax
    (`apps/openagents-web/worldbuilder-world.coax`) and carried out on the
    engine's edit log. No server holds the world, so there is no route: the
    same codec, the same batch and the same fence by revision, on the page
    ([the rules](../protocol/world-edit-v1.md#worldbuilders-world)). The
    chat's edits of things go the same way.
  - **The land.** WorldBuilder is the first world to take the `terrain` edit.
    The viewport has a Land tool (`viewport.coax`, `vp_dab`), and the shell
    shows it only for a world whose model says its land takes edits: the
    brush's kind, the material it paints, and its radius and strength as
    steppers (`k_stepper`, new in the kit and shown on `/components`).
  - **What the model says more** (`editor.model.v1`): how many things the
    world holds and takes (the shell lists the newest 256 and says so), how
    many kinds it has, that it is kept on this device (no Publish), and its
    land's brushes and materials.
  - **Deleted from WorldBuilder:** its UNDO and REDO buttons, its brush bar
    (tools, layers and two sliders), its own ray from the screen to the
    ground, and its per-frame brush. Undo and redo are the shell's (and
    Cmd/Ctrl+Z and "undo" typed in the line). Picking is `vp_pick`.
  - **Kept:** the chat line. It asks WorldBuilder's own model under the day's
    shared cap with no account; the shell's agent strip is the `/chat`
    conversation and does not do that. The panel with the limits, WALK,
    FRAME and the hints are the game's. The host tells the page the part of
    the window the shell's panes leave free, and the page lays these out
    inside it.
  - Proved in headless Chrome on the dev server
    ([picture](shots/2026-11-13-e13-worldbuilder-shell.png),
    [a phone's size](shots/2026-11-13-e13-worldbuilder-phone.png),
    [a full world](shots/2026-11-13-e13-worldbuilder-full.png)):
    `docs/editor/acceptance/worldbuilder-run.ops` places, selects, moves,
    turns, sizes and removes a cottage and raises and paints the ground; each
    edit of a thing is one revision and shows in the same frame; Undo and
    Redo walk them. A world of 4,096 things kept in the old save's bytes
    loads, one more is refused in words ("This world is full: 4096 things at
    most. Delete some to add more."), and after one is removed the next
    stands.
  - **A phone's size.** The strip's menus lie over the side panes, and on a
    narrow window a menu opens across the strip, so the Land tool and its
    brush can be picked there. Not proved with a finger: the driver's tap on
    a menu row did not register on this page at 390 px wide, though a script's
    click does and the same tap does on Splice.
  - **Not moved:** Everglade's admin EDIT and the realm edit layer. What each
    needs is on #13422.

### Programmatic building

- **CLI and MCP:** rows of `REALM_COMMANDS`, the table the other realm
  commands come from: `openagents realm world`, `world-place`, `world-apply`,
  `world-undo` and the rest, each also a `realm_*` MCP tool
  ([the list](../protocol/world-edit-v1.md)). Coder gets them by default.
  There is no second `world_*` family (2026-11-13).
  - `show` answers entities, content rows, landmarks, kept-clear zones, the
    player's position and "here".
  - `apply` answers the new revision, a frame and any refusal.
- **`coax drive` edits:** `drive` gains edit verbs (`place`, `move`, `set`,
  `undo`) and a live mode against the dev server, so an agent can edit, watch
  the reload and check the result in one session.
- **A realm's code** is still code: the agent edits the Coax and the live
  channel reloads it. Edits handle content and placement; code handles rules
  and behaviour.

## Gaps, and the issues that close them

Existing issues this builds on:
- **Rendering and assets:** #13303 P6.1 (retained rendering), #13304 P6.2 (one pack reader).
- **Content:** #13381 P4.6 (the page loads content tables), #13293 P4.4 (creature templates).
- **The World:** #13279 P2.6 (broadphase), #13280 P2.7 (cells).
- **Protocol and layout:** #13250 (protocol), #13270 P1.4 (engine layers), #13405 (the rename).

The new issues are in the table below, E1-E14 (#13410-#13423), on project 22. The chat and world-building
plan's slices 0b-7 are folded into them: E5 is slice 0b, E3 is slices 1-2, E6
is slice 3, E4 is slice 4 and E9 is slices 5-6.

| # | Gap | Issue |
|---|---|---|
| E1 | `serve` watches what the build read, recursively, including shared modules, generated WGSL, packs and textures | #13410 |
| E2 | Code reload keeps the game's state, migrating field by field (the ch082 design) | #13411 |
| E3 | A world document: id, save and load of a World, revisions, fenced edits, `proto/world/edit-v1` and its codec | #13412 |
| E4 | Pages apply outside edits from the live channel or the API's events | #13413 |
| E5 | One placement planner (`metaverse-engine/editor/place.coax`) and prefabs: a saved group of entities as a template row | #13414 |
| E6 | `openagents world` CLI and `world_*` MCP tools; `coax drive` edit verbs and a live mode | #13415 |
| E7 | The editor shell on `?edit`, built from the `/chat` components: outliner, inspector, toolbar, notices | #13416 |
| E8 | Viewport tools: ray picking, gizmos, snapping, fly camera, play, pause, step, simulate | #13417 |
| E9 | The agent strip: the `/chat` conversation inside the editor, with `world_*` tools, the selection and "here" | #13418 |
| E10 | Content tables edited in the browser, and a content revision applied live | #13419 |
| E11 | Asset import: drop glTF, PNG or audio, pack it with a licence, browse it, swap it live | #13420 |
| E12 | Code from the browser, owner and admin only: edit a realm's Coax on the dev server, check it on save, swap it live | #13421 |
| E13 | The four editors move onto the shell: Everglade EDIT, WorldBuilder, the realm edit layer and Splice | #13422 |
| E14 | Acceptance: a real-input run of the editor in real Chrome, on a laptop and a phone | #13423 |

The acceptance run that repeats is `docs/editor/acceptance/` (E14, #13423): a
file of `coax drive` operations and how to run it. One pass on the dev server
places, moves, turns, edits a content row, removes, undoes and redoes; does
the same kind of edits through the agent in the editor's foot, with the
model's answers scripted; and changes a line of game code and a shader from
the Code pane, each swapped in live with the world's revision and the
player's place kept. On the build it was written against the engine swapped
about 1 s after Save and the shaders in 0.2 to 0.6 s.

## Order

1. **E1 and E2,** the dev loop. These help every realm today, so they come
   first and run in parallel.
2. **E3, then E4 and E6,** so worlds can be reached from outside the page.
3. **E7, then E8,** the shell and its tools.
4. **E9 and E10.**
5. **E5, E11 and E12.**
6. **E13,** when the shell can do what each old editor does. Each old editor's
   own code is deleted once it has moved.
7. **E14** gates "done".

Splice is the first realm through every step (it already has the World,
content tables and a typed edit layer). Everglade is the second.

## Done means

- On a laptop and a phone, a person opens `/worlds/splice?edit`, places,
  moves and deletes pieces, edits a content row, and changes a shader and a
  line of game code. Each change shows in under 2 seconds, and the run keeps
  its state.
- From `/chat`, an agent does the same through `world_*` tools and code edits,
  and the person sees each change in the open editor within a second.
- Every edit undoes. A refused edit says why. A failed build keeps the last
  good one running and shows the error.
- The editor's UI has no component of its own: every panel is a `/chat`
  component.
- One real-input run in real Chrome passes, along with the headless checks.
