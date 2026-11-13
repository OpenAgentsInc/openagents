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
of the Metaverse Engine. The older Aethyrforge spec (`docs/verse/aethyrforge.md`)
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
| Rebuild time | **Good.** A one-line edit is running in about 2.5 s (resident and incremental compiler, #13138, #13142); a dev engine build takes 118-135 ms; type errors show in 0.63 s | `coax/book/ch46-resident-compilation.md` |
| State across a code reload | **Good on the dev server (#13411).** A development module carries its state's field table. A reload that changes the state's layout keeps every field both layouts name, starts new fields from their defaults and names the new and dropped fields in the dev note. Not built: declared migrations (a renamed field) and a rollback when the first frame traps | `coax/web/state-keep.js`, `host.js` `loadEngine`, `metaverse-engine/book/ch082-change-code-while-it-runs.md` |
| One function swapped live | **Experiment.** `door_ease` only, 900 ms from edit to running | `coax/web/fast-edit.*` |
| Agents drive and see | **Exists, no edits.** `coax drive`: `tree`, `state`, `tap`, `type`, `walk-to`, `use`, `clock`, `screenshot`, `logs`, `perf`. It serves `--static`, so it has no live channel, and it has no edit verbs | `coax/web/drive.coax`, `docs/verse/agent-clock.md` |
| The scene model | **Partial.** Aethyr's World (entities, columns, systems, spatial index) is used only by Splice's edit layer. No save or load of a World, no prefabs | `metaverse-engine/core/world.coax` |
| Typed edits, undo and redo | **Exists** | `metaverse-engine/editor/edit.coax` |
| Content tables | **Exists, CLI only.** The editor isn't reachable from a page; the page doesn't load the tables yet (#13381) | `metaverse-engine/content/tables.coax`, `metaverse-engine/editor/content-edit.coax` |
| Picking | **Partial.** `query_ray` exists; every editor picks its own way (Everglade by 56 px screen distance) | `metaverse-engine/physics/query.coax` |
| Gizmos, outliner, inspector, fly camera, snapping, asset browser, play/simulate | **Missing** | — |
| In-world editors | **Four, separate.** Everglade admin EDIT, the realm edit layer and agent pane, WorldBuilder (with a terrain brush, browser save only), Splice's layer (fixture only) | `everglade-edit.coax`, `metaverse/edit/`, `worldbuilder*.coax`, `metaverse/splice/layer*.coax` |
| Programmatic building | **Partial.** `openagents realm` has 37 verbs and MCP `realm_*` tools. No `world_*` tools, and no world document with an id | `apps/openagents-cli/` |
| Code changes from a browser | **Missing.** Coax compiles to C, then clang builds Wasm on a machine with the toolchain. A paired Coder has one; a browser doesn't | `coax/web/wasm.sh` |
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

  New component kinds go into `frames.coax`, never into the editor.
- **The agent strip is the `/chat` conversation.** The same thread, the same
  agents and the same transcript appear on `/chat` and in the editor. The
  agent gets `world_*` tools plus the editor's selection and "here" (the last
  point tapped).
- **Viewport tools:**
  - picking through `query_ray` against the World's spatial index;
  - move, rotate and scale gizmos that snap to a grid;
  - a fly camera;
  - play, pause and step on the agent clock;
  - "simulate", which runs the World without the player.
- **Assets:** an asset browser over the realm's pack and the shared kits.
  Drop a glTF, PNG or audio file to add it: it is packed with a declared
  licence (`metaverse-engine/editor/pack.coax`) and appears in the browser.
- **Code:** a realm's Coax source opens in a code panel (`k_code`, plus an edit
  field from `components/core/edit.coax`). Saving sends it to the paired Coder,
  which compiles it with the resident compiler; the live channel swaps it in.
  Without a Coder, a hosted build seat does the same (later).

### Programmatic building

- **CLI and MCP:** `openagents world new|show|apply|undo|redo|shot|open|save`
  and the `world_*` MCP tools. Coder gets them by default.
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
| E12 | Code from the browser: edit a realm's Coax, compile on the paired Coder, swap it live | #13421 |
| E13 | The four editors move onto the shell: Everglade EDIT, WorldBuilder, the realm edit layer and Splice | #13422 |
| E14 | Acceptance: a real-input run of the editor in real Chrome, on a laptop and a phone | #13423 |

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
