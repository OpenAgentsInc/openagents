# Chat and world building: audit and proposal

Status: proposal, 2026-11-12. Nothing in it is built yet.

This doc covers Aethyrforge, the other places where a world is edited, the new
WorldBuilder at `/create-world`, and Splice, the first world to connect. It
then proposes how `/chat` and its agents (Claude Code, Codex, Antigravity and
Microcoder through your Coder, plus the hosted chat) can build worlds.

## Summary

- Four surfaces edit a world today. Each has its own document, its own edit
  shape and its own way in. They share one engine edit log
  (`aethyr/forge/edit.coax`), but no document or wire shape.
- The pattern we want already runs on the desktop. In Aethyrforge desktop, the
  chat interprets your words and calls a typed tool (`aethyrforge-scene apply
  JSON`). The tool checks the edit, saves it fenced by version, and the world
  updates. The web has nothing like it.
- WorldBuilder is the best fit for chat, because its planner already separates
  two steps: the server's model turns words into typed edit lines, and
  `worldbuilder-plan.coax` carries those lines out. An agent in `/chat` can
  write the same lines, so it never needs the server's model.
- One gap blocks this. A WorldBuilder world exists only in the browser that
  made it, so no agent, CLI or second tab can reach it.
- **The first demo is Splice, not an empty grid** (owner, 2026-11-12). Splice
  (`/worlds/splice`) is a hand-written Coax game, not a document. Its world is
  drawn by code (`g_world`) and its collision is a list of 44 walls in code
  (`g_layout`). So Splice needs an **edit layer over its designed world**: a
  list of placed kit pieces that is drawn and collided like the designed
  world, and that never breaks the run. Everglade does the same with its edit
  list over the designed city.
- **Proposal:**
  1. Give Splice an edit layer: a catalogue of its cel kit pieces, named
     landmarks to aim at, and the zones the run needs kept clear.
  2. Lift WorldBuilder's anchor-and-side placement into the engine so Splice
     and WorldBuilder share it.
  3. Store a world as a document with an id and edits fenced by revision. The
     first world is `splice`, and only admins can write it.
  4. Make the edit one wire shape, and expose it as `openagents world ...` CLI
     verbs and MCP tools, which Coder gets by default.
  5. Make the page follow outside edits.
  6. Give `/chat` a world pane and a world card.

  After that, "park a second rover by the stash crate and put two crab-bots in
  the party hall" works from any agent in `/chat`, and Splice changes beside
  the conversation.

## Audit: what edits a world today

### 1. Made realms: Aethyrforge on the web (`/realms/new`, `/realms/SLUG`)

- **Spec:** `docs/verse/aethyrforge.md`, the MVP spec for one `/forge` screen.
  The pieces exist, but the single screen does not.
- **Making a realm:** `builder.coax` has setup questions, a prompt and a board
  of catalogue features. `POST /api/playtest/realms/plan` (Gemini under a
  schema built from the catalogue) answers in 1-2 seconds. The catalogue has
  20 features: land, water, trees, village and so on.
- **The realm:** `myrealm.coax` (4,185 lines) shows it. For its owner it
  re-reads its spec once a second, so an outside edit shows within a second.
- **Edits:** `aethyr/forge/edit.coax` makes every revision path an invertible
  typed edit, with undo and redo. The server keeps the same log
  (`migrations/0111_realm_edit_log.sql`), and saves are fenced by revision
  (409 on a stale one). `doc.coax`, `publish.coax` and `pack.coax` cover the
  document, publishing (the engine build is pinned, migration 0107) and asset
  packs.
- **CLI and MCP:** `openagents realm` has 35 verbs: `new`, `revise`, `switch`,
  `move`, `place`, `apply FILE.jsonl`, `undo`, `publish`, `fork`, `shot`,
  `play`, `look`, `go`, `say` and more. Each verb is also an MCP tool,
  `realm_*` in `apps/openagents-cli/mcp.coax`.
- **Gate:** making a realm is invite only (`make 0` for the public).

**What chat could use:** the most complete agent surface, with typed verbs, MCP
tools, revision fencing, live follow and publishing. Its weak spot is the
catalogue: a realm is 20 feature switches, not "put that house left of the
tree".

### 2. WorldBuilder (`/create-world`), the new web world creator

- **Page:** `apps/openagents-web/worldbuilder.coax` (1,654 lines). It shows an
  empty grid with one chat line at the bottom (EditBuf and the controls kit,
  with SEND, UNDO and REDO).
  - It draws all 785 Quaternius models, one instanced draw a mesh.
  - The ground is an Aethyr terrain tile (`worldbuilder-land.coax`) that you
    paint and shape by chat or with a brush bar.
  - A tap sets "here". You can walk the world as a ghost on Everglade's
    controller (`worldbuilder-walk.coax`, #13225).
- **Planner:** `worldbuilder-plan.coax` (1,075 lines). It reads bounded edit
  lines and carries them out on `ForgeScene`, Aethyrforge's scene edit log
  (4,096 things, undo and redo, #13226). Placement is deterministic: from an
  anchor and a side, footprints kept apart, using a seeded draw. **It never
  reads the typed words.**
- **Server:** `coax/web/api/worldbuilder.c`.
  - `POST /api/worldbuilder/act` sends the page's JSON to Gemini 3.8 Flash on
    Vertex with a JSON schema. The answer is `S say` plus one `A
    op|asset|count|...` line an edit: place, move, rotate, scale, delete,
    duplicate, undo, redo, clear, paint, raise, lower, smooth or flatten.
  - `worldbuilder-policy.coax` decides the spend cap. It is $50 a day, shared
    by everyone; a call reserves its cost before it runs and settles after.
    Each address gets 4 calls at once, then one every 1.5 seconds. The page
    shows the spend and both limits.
  - The tables are in migrations 0141 and 0159.
- **Saving:** a world is saved only in the browser: the page slot's save, the
  text format `WB1`, up to 1.3 MB. There is no sign-in and no server copy.
- **Agent access:** none. There is no CLI verb, MCP tool or world id.

**What chat could use:** the right granularity for talking ("five rocks around
it", "make it bigger"), a typed edit line that already exists, and
deterministic placement. Missing: a document anyone besides the page can reach.

### 3. Everglade city edits (admin EDIT in Everglade)

- **Files:** `everglade-edit.coax` and `everglade-forge.coax`. An admin's EDIT
  replaces Everglade's HUD with the forge panels: AETHYRFORGE, CHANGES
  (realm or engine) and DONE.
  - The hotbar places the city's five things.
  - You can pick, move, turn, resize and delete them, with undo, redo and
    ORIGINAL.
  - `verse/everglade/edits.coax` refuses edits that break the city's rules.
  - The edit list is published to `POST /api/playtest/world/everglade` as an
    admin's write.
- **Voice:** `everglade-voice.coax` adds voice edits onto the same list.
- **Content tables:** P4.5 (#13294) gives every content table its own
  Aethyrforge editor (`aethyr/forge/content-edit.coax`).
- **Protocol work:** E11 is moving the edit and fork wire shapes onto shared
  codecs (#13250).

**What chat could use:** an admin-only world with strict placement rules. It is
a good second target once the tool shape exists, and it should stay
admin-only.

### 4. Aethyrforge desktop (`apps/aethyrforge-desktop/`)

- **The app:** a native Mac (AppKit and Metal) and Linux (Wayland and Vulkan)
  app around the Everglade 5e engine, with a local room and Parakeet voice.
- **Its chat:** `Sources/chat.m` and `chat-run.coax`. Each turn runs in
  process on the shared turn runner (`coax/lib/agent/engine/run.coax`, the
  same one `/chat`'s Claude path uses). The turns are saved in Coder's session
  log, so `coder ls` lists them.
- **Scene commands:** `aethyrforge-scene show` and `aethyrforge-scene apply
  JSON` ([v1](../protocol/aethyrforge-scene-v1.md),
  [v2](../protocol/aethyrforge-scene-v2.md)), with schemas in
  `proto/aethyrforge/`.
  - They take typed commands only, never natural language: "Coder interprets
    the user's words and calls this same tool."
  - The whole proposed document is checked before one write, fenced by
    version (409). The write is read back before the tool reports success.
  - `scene-session.json` supplies where the player stands, so "here" means
    something.

**This is the model for the web.** The words stay with the agent, the tool
takes typed fields, the server fences and checks, and the world updates
through its normal path.

### What they share and what they don't

| | Made realm | WorldBuilder | Everglade edit | Desktop scene |
|---|---|---|---|---|
| Document | Server, per slug, revisions | Browser slot only (`WB1`) | Server, one city | Local world API |
| Engine edit log | `aethyr/forge/edit.coax` | `ForgeScene` (same file) | Own list (`edits.coax`) | Everglade's rules |
| Words become edits by | Gemini planner, server | Gemini planner, server | Voice route, server | **Your agent** |
| Typed wire shape | JSONL ops (`realm apply`) | `A op\|asset\|...` lines, no schema file | Being moved to a codec (E11) | JSON Schema in `proto/` |
| Agent access | CLI + MCP, 35 verbs | None | None | `aethyrforge-scene` CLI |
| Version fence | Revision, 409 | None | Admin write | Version, 409 |
| Live follow | Once a second | n/a | Room edit event | Room edit event |
| Who | Invite only | Anyone, no sign-in | Admins | Local owner |

### 5. Splice (`/worlds/splice`), the first world to connect

- **What it is:** an unlisted Aethyr game about a robot smuggler
  (`apps/openagents-web/realms/splice/index.md`). It was built in one Claude
  Code session; `docs/unreal/2026-11-12-splice-unreal-vs-aethyr-session-audit.md`
  compares that session with the Unreal one.
  - `splice-game.coax` (1,457 lines) holds the tuning, world, rules and
    interface. `splice.coax` (394 lines) holds the page frame and 17 still
    scenes. `gpu/splice.coax` holds the shaders: Anime Lab's cel pass plus an
    ink overlay.
- **How its world is made:**
  - `g_world` draws the whole world in code from the engine's cel kits
    (`aethyr/kit/machines.coax`, `aethyr/kit/land.coax`): mesas, dunes,
    rocks, walls, the rover, crates, the party hall, the dome village and the
    rail line. The page writes the world once a scene, so a change means
    writing the world buffer again, which it already knows how to do.
  - Places are constants: `ROVER_X/Z`, `POST`, `CRATE`, `RUBBLE_Z`, `WALL_Z`,
    `BOSS`, `DECK`, `BAY`, `MEM`, `VIL`, `SCAN_Z` and `RAIL`. The pickups
    (`pick_x/z`) and the foes (`foe_ax/az`) are fixed tables.
  - Collision is `g_layout`'s list of up to 44 axis-aligned walls plus
    `g_push` for boxes.
- **The kits have about 30 pieces.**
  - Machines: robot, crab, walker, spider, beast, tank, mech, rover, turbine,
    airship, bird, digger and figure.
  - Land: arch (block and round), canyon, terraces, towers, steps, sea, palm,
    dome house, banner, moon, wall, crate, rocks, dunes and mesas.
  - Each piece is a function from a frame, a scale and tints to cel
    triangles, which is what a placed-thing catalogue needs.
- **Checks:** 9 `#test`s in the game, including one that plays the run from
  the title to SCENE RESTORED. On 2026-11-12 the file's 207 checks pass
  (remote build, 19 s). The page answers 200 in production and on the local
  dev server.
- **Agent access today:** a coding agent can edit `splice-game.coax`, rebuild
  in about 8 seconds and look with `coax drive`, the same way Splice was
  built. That works from `/chat` now through Claude Code, but it changes the
  game's source for everyone and needs a commit and a deploy. No data layer
  lets an agent place a thing without changing code.

**What chat could use:** a world people already know, a strong look, a run
that tests itself, and a kit catalogue that fits placed things. What's missing
is an edit layer.

The chat zero-base audit (`2026-11-11-chat-zero-base-audit.md`) already notes
that the conversation is written five times: web, desktop, the project agent,
Aethyrforge and Coder. WorldBuilder's box is a sixth input line, on the kit
(#13226).

## Proposal: building a world from `/chat`

The rule is the desktop's: **the agent turns words into typed edits; the world
checks them, saves them fenced by version, and shows them.** Our server's model
is one optional source of edits, not the only one.

### The first demo: Splice, from `/chat`

1. In `/chat`, Claude Code (or Microcoder, Codex or Antigravity) is asked:
   "In Splice, park a second rover by the stash crate, put two crab-bots on the
   party's dance floor, and a turbine on the dune past the rail marker."
2. The agent calls `world_show splice`. It sees the landmarks by name (rover,
   stash crate, dance floor, rail marker, dome village…), the pieces already
   placed, the kit catalogue, and the zones it may not build in.
3. The agent calls `world_apply splice` with three place edits anchored on
   those landmarks.
4. Splice, open in the chat's world pane and in any other tab, picks up the
   new revision within a second. The rover, crabs and turbine appear, drawn in
   the same cel and ink style, with colliders that match what you see.
5. The person walks there and finds them. The run still plays to SCENE
   RESTORED, because the checker refuses any edit that blocks it.
6. "Undo the turbine" goes back one edit. Nothing in `splice-game.coax`
   changes, and nothing is deployed.

### Slice 0: Splice's edit layer

**Status: done, 2026-11-12 (#13404).** The commits, in order:

| Commit | What landed |
|---|---|
| `a4c1c7ddbe`, `46f7f36d21` | Splice's own folder and context. It is `verse/splice/`: the repository root is for projects with a book |
| `f0a0759359` | The content tables, and the game reading every landmark from them |
| `53505717f7` | The colliders as World entities, the edit layer as typed edits, and the check |
| `3efca3f506` | The walk replay, the fixture layer and the mutant check |
| `@@DRAW@@` | The retained draw (a mesh for each kind, an instance for each placed piece), the dev server's fixture load, and the docs |

What was built differently from the text below, and why:

- **Five tables, not three.** `pieces`, `landmarks` and `kept_clear` are as planned. `looks`
  holds a piece's three colours, because a content table has eight columns at most. `areas`
  says where a piece may stand at all, which is how "past the map's edge" is refused.
- **30 walls, not 44.** `WALLS` is the list's size. The layout has 30 boxes; all of them, the
  rubble, the boss wall, the banned bot and the rover at the rail stop are World entities.
- **The walk replay uses the engine's steering service** (`aethyr/ai/steer.coax`) with the
  game's own step and collision. The protocol bots speak a room's wire, and Splice has no room.
  The agent driver's `walk-to` goes straight at its target, so it would refuse a crate a player
  walks round. `coax drive` isn't wired to Splice's page yet.
- **The fixture layer is typed edits in Coax** (`verse/splice/layer-fixture.coax`), loaded with
  `?layer=fixture` on the dev server only. A layer file needs the wire shape of slice 2.
- **The host could already draw instances.** `coax/web/host.js` passes a draw's instance count
  and first instance through, and binds every store a page declares. The new work was Splice's
  two stores and its vertex stages (`verse/splice/gpu/splice.coax`); the host didn't change.
- **Not done here:** the Unreal host draws no placed pieces (its face passes the stores but
  draws two). The developer notes are `verse/splice/README.md`, "Edit layer".

New work in Splice and the engine. The rule is "never hand-roll", and the AAA
program (#13257) has already built most of what this needs. The section
"Checked against the AAA program" below says which issue each part follows.

- **Splice gets its own folder and context first.** Its code moves from
  `apps/openagents-web/splice*` to `verse/splice/` (realms without a book live under `verse/`), as Everglade's moved to
  `everglade/` (P1.2, #13268). It also gets a row in `contexts.txt`
  (`splice folders: verse/splice/ may load: engine realm-authoring toolchain`), so
  `layer-check` (P0.7, DDD.6) judges it. The page stays at `/worlds/splice`.
- **The catalogue and the landmarks are content, not code** (P4.5, #13294).
  Splice gets an `aethyr.content.v1` document, read and validated by
  `aethyr/content/tables.coax` and edited in Aethyrforge's table editor
  (`aethyr/forge/content-edit.coax`). It has three tables:
  - `pieces`: one row a placeable piece (rover, crab-bot, turbine, palm, dome
    house, crate, arch…). Each row holds a stable content ID (P3.3, #13286),
    its display name, the kit piece it draws, its tints from Splice's palette,
    and its footprint.
  - `landmarks`: the rover, stash crate, checkpoint post, dance floor, dome
    village, rail marker and so on. Each is a `cm` place with a radius. The
    constants in `splice-game.coax` are read from this table instead.
  - `kept_clear`: the places the run needs: the scanner gate and its
    corridor, the rail track, every pickup's reach, the rubble, the boss wall
    and chamber, the catwalk ramps, the rover's and crate's use range, and the
    checkpoint post.
  Edits and saves name pieces by content ID; names are for people and agents
  to read.
- **The kit pieces stay engine code** in `aethyr/kit/`. A row names which kit
  piece to draw. A `#test` measures every piece's triangles and checks its
  row's footprint against them, so colliders match what you see ("no
  invisible blockers").
- **Placed pieces are entities in Aethyr's World** (P2.1, #13274,
  `aethyr/core/world.coax`): a transform, a kind (the content ID) and a
  collider. Collision goes through the World's spatial index
  (`aethyr/core/spatial.coax`) and the shared physics queries, not through a
  longer wall list.
  - Splice's 44 hand-written walls (`g_layout`, scanned linearly by
    `g_push`) move into the same collider set as part of this slice. That
    follows G6 step 6 and P2.6 (#13279): one broadphase behind the queries,
    and no linear scans.
  - A placed machine is scenery: it has no AI and no hit points.
- **Drawn as a retained scene** (P6.1, #13303): each piece kind's cel mesh is
  written once and kept on the GPU, and the layer draws instances with their
  transforms. An edit changes instances; it doesn't rewrite the designed
  world's buffer. If Splice's host can't draw cel instances yet, that support
  is built here, as Splice's part of P6.1, and not worked around ("no
  shortcut phases").
- **Deterministic and replayable.** The layer is applied in revision order
  from the edit log (`aethyr/forge/edit.coax`). `world_digest` covers it, so a
  replay with the same edits gives the same digest a step at a time (book
  chapter 5).
- **The run still plays.**
  - A `#test` plays the whole run with a full layer: a fixture with a piece
    next to every kept-clear zone.
  - Each save replays the walk from the start to the boss chamber against the
    proposed layer, using the protocol bots and agent clock that already exist
    (P5.2, #13296; `coax drive`, `clock until`), not a separate walker. It
    refuses the edit if the walk stops.
  - A mutant check removes each kept-clear zone in turn and expects the walk
    replay to refuse.

### Slice 0b: one placement planner

- WorldBuilder's planner already works out where things go: an anchor, a side
  as the camera sees it, footprints kept apart, and a seeded spread. Today it
  assumes WorldBuilder's pack catalogue (`WbCat`).
- Move the placement part to `aethyr/forge/place.coax`, in the
  realm-authoring context, with a catalogue interface that the Quaternius pack
  and Splice's `pieces` table both fill. It loads nothing from `apps/` (P1.4,
  #13270). It has two consumers from the day it lands (P1.6, #13272), and
  WorldBuilder's copy is deleted in the same change.
- Splice's `landmarks` are its named anchors (`A_NAMED`), and its
  `kept_clear` zones are places the planner never spreads into.

### Slice 1: a world has an id and a document

- **API:**
  - `POST /api/worldbuilder/worlds` makes a world and answers its id and an
    edit key.
  - `GET /api/worldbuilder/worlds/ID` answers its `WB1` text and its revision.
  - `POST .../ID/edits` takes edit lines with a base revision. It answers 409
    on a stale revision, and refuses the whole batch if one line fails.
- **No sign-in still works:** the edit key stays in the browser beside today's
  slot save, and the page writes both. A logged-in visitor's worlds are also
  listed under their account.
- **The URL:** `/create-world/ID` opens a world and `/create-world` makes a new
  one. A world that has only a slot save gets an id the first time it is
  shared or opened by an agent.
- **The server checks with the page's code:** it carries edits out by running
  the shared planner and the world's own checker as a native tool, so the
  server and the page cannot disagree. The route's rules live in
  `coax/web/api/world-policy.coax`, `#authority` and mutation-tested (DDD.10,
  #13321), and `INVARIANTS.md` gains a line for them. The client is never
  trusted for placement (P0's rule: the server decides).
- **Each saved edit is a domain event:** a `proto/registry.json` row of kind
  `event` owned by realm-authoring (DDD.5, DDD.8), written with the outbox
  rule, so the page, the CLI and later a room all read one stream.
- **The migration** takes the next free number, and the unique-number check
  passes (P5.7, #13301). It ships in the one ordered release (P5.5, #13299),
  run by the coordinator.
- **Splice is the first world with an id:** `splice`, made once by migration.
  Only an admin may write it: the owner's own Coder (with the agent token) or
  an admin session in the hosted chat. Everyone who opens `/worlds/splice`
  sees the layer. Its checker is Splice's: the kit catalogue, the kept-clear
  zones and the walk replay from slice 0.
- **ORIGINAL** clears the layer and brings back the designed world, as
  Everglade's ORIGINAL does, and UNDO after it brings the layer back.

### Slice 2: one wire shape for an edit

- Write `proto/world/edit-v1.schema.json` from the fields
  `worldbuilder.c` already asks Gemini for. Generate one codec that
  `worldbuilder.c`, the page and the CLI all use (the protocol review's rule:
  `proto/` is the one source).
- A golden fixture covers every op.

### Slice 3: CLI verbs and MCP tools

- **Verbs:**
  - `openagents world new`, `show ID`, `apply ID FILE.jsonl` (or lines on
    stdin), `undo`, `redo`, `shot ID` and `open ID`.
  - `show` answers what an agent needs to aim: each thing's id, catalogue name,
    position, size and yaw; the ground's layers; the catalogue's names; and the
    last tapped spot, so "here" means what the person tapped.
  - For Splice, `show` also answers the named landmarks, the kept-clear zones
    and where the player stands, so "by me" works as it does on the desktop.
  - Each verb is an MCP tool, `world_*` beside `realm_*` in
    `apps/openagents-cli/mcp.coax`.
- **Coder** gets the tools by default, as it does the realm tools, so Claude
  Code, Codex, Antigravity and Microcoder in `/chat` can all use them.
- **Cost:** edits that come from an agent never call our Gemini planner. They
  cost us nothing, so they don't count against the $50 a day. The person's own
  engine pays for the words. The anti-flood limit and the 4,096-thing cap
  still apply, and the page still shows both.

### Slice 4: the page follows outside edits

- While a world with an id is open, the page re-reads its revision once a
  second (myrealm's pattern). It applies new edits through `ForgeScene`, so
  UNDO works on them too.
- Each edit animates in as the Aethyrforge spec describes ("A thing is placed:
  a ring of light, it rises out of the ground"). That spec's arrival table is
  shared engine work, not WorldBuilder-only code.
- This also makes two tabs, or a person and an agent, work on one world.
- In Splice, a new revision spawns and despawns the layer's entities at the
  World's flush, and adds or removes their instances and colliders. The
  player's position, inventory, Heat and the run's state are kept, and a piece
  is never placed on the player: the checker treats the player's spot as a
  kept-clear zone for that edit.

### Slice 5: the world in `/chat`

- **A world card.** When a turn's tool result names a world, the transcript
  shows a card with its name, a frame from `world shot`, the thing count, and
  OPEN. The hosted chat and the local agents use the same card.
- **A world pane.** OPEN puts the world beside the conversation, and you keep
  talking to the same agent.
  - The pane is the world's own page in a frame beside the chat: Splice's page
    for `splice`, WorldBuilder and its 12.2 MB pack for a WorldBuilder world.
    The chat's code never loads a world module. In `contexts.txt`, chat may
    load only coder, identity, protocol and toolchain, so `layer-check` would
    refuse that load anyway. `/chat`'s own budget (chat-web, 168,000 bytes)
    doesn't grow.
  - The card is built from a protocol shape (`proto/world/card-v1`: id, name,
    revision, thing count, a frame's URL), which chat is allowed to load.
  - The page and the pane talk only through `postMessage`, with typed
    messages from the same proto: "here" from a tap, and the revision now
    shown.
  - A local runner's pane opens the dev server's world (`127.0.0.1:9471`) so
    the owner can watch an agent work against a local build first.
  - On a phone the pane is a tab above the composer.
  - A tap in the pane sets "here" and passes it to the agent's next turn, so
    "put a tree here" works.
- **Starting from chat:** "make me a world" in an agent's conversation runs
  `world new`, the card appears, and the pane opens.

### Slice 6: the hosted chat (no Coder)

- When no Coder is paired, the hosted chat can reach the same tools. A
  world-building turn is chosen by the central typed semantic selector (the
  Decisions API, Clef first), never by keyword matching, per the workspace's
  routing rule.
- Its words-to-edits step can be WorldBuilder's existing Gemini call. Such
  turns count against the $50 a day, and the card says so when the cap stops
  them.

### Slice 7: the other world kinds, on the same tools

- Once `world_*` works, the same shape covers:
  - made realms, which already have `realm_*` and gain a pane in `/chat`;
  - Everglade city edits, which stay admin-only and reuse E11's codec;
  - the desktop's scene commands.
- In the longer run, the `/forge` screen in the Aethyrforge spec has WorldBuilder's
  object layer as its canvas. That one screen then appears in `/chat`'s pane,
  in Aethyrforge desktop's window and at `/forge`.

## Checked against the AAA program

The AAA audit (`2026-11-11-everglade-aethyr-aaa-audit.md`) and its tracker
(#13257, issues #13258-#13306 and #13346) set the rules for engine and game
work. On 2026-11-12, 31 of those 50 issues are closed and 19 are open. The DDD
tracker (#13311) adds the context rules. This is how each relevant one
applies to the plan, and what changed because of it.

| AAA or DDD issue | State | What it asks | In this plan |
|---|---|---|---|
| P0.7 layer-check, DDD.6 contexts | Closed | No upward loads; each context loads only what `contexts.txt` allows | **Changed.** The chat never loads a world module; the pane is the world's own page, and the card is a proto shape. Splice gets a context. `place.coax` loads nothing from `apps/` |
| P1.2 game code leaves `apps/openagents-web` | Open | One game module in its own folder | **Changed.** Splice moves to `verse/splice/` before its edit layer lands |
| P1.4 Aethyr layers | Open | Engine code sits in its layer folders | `place.coax` is realm-authoring (`aethyr/forge/`); kit pieces stay in `aethyr/kit/` |
| P1.6 every engine module has a consumer | Closed | No unused modules | `place.coax` has two consumers on the day it lands; WorldBuilder's copy is deleted |
| P2.1 Aethyr World | Closed | Every entity lives in one World with columns, systems and a spatial index | **Changed.** Placed pieces are World entities, not draw calls added to `g_world` |
| P2.6 a broadphase behind the queries | Open | No linear collision scans | **Changed.** Pieces and Splice's 44 walls collide through the spatial index; `WALLS` doesn't grow |
| P2.7 one world of cells | Open | Cells, tiles and paged packs | Not needed for Splice's size. Edits carry world coordinates, so a layer splits by cell when Splice becomes cells |
| P3.3 stable content IDs | Closed | IDs on the wire and in saves | **Changed.** Edits and saves name pieces by content ID, not by name |
| P4.5 content tables, edited in Aethyrforge | Closed | Content is validated data that designers edit without a programmer | **Changed.** The catalogue, landmarks and kept-clear zones are `aethyr.content.v1` tables, not Coax constants |
| P5.2 protocol bots | Closed | Bots drive the game headless | The walk replay on save uses the existing bots and agent clock |
| P5.5 one release; P5.7 migration numbers | Closed | Ordered release behind a canary; unique migration numbers | The world document's migration and API ship that way, run by the coordinator |
| P6.1 retained scene | Open | Meshes stay on the GPU; transforms are sent | **Changed.** The layer is per-kind meshes drawn as instances; an edit doesn't rewrite the world buffer |
| DDD.5, DDD.8 registry rows and domain events | Closed | Every wire shape names its owning context; events are registry rows with the outbox rule | A saved edit is an `event` row owned by realm-authoring |
| DDD.10 rules in `*-policy.coax` | Closed | API rules in Coax policy files | `world-policy.coax`, `#authority`, mutation-tested |
| Determinism and replay (graded A) | Kept | Same inputs give the same digest | The layer is applied in revision order and `world_digest` covers it |
| P0 server trust (P0.1-P0.4) | Closed | The server decides; the client is never trusted | The server runs the checker; writes need an admin or the owner's agent key |

What doesn't apply: Splice has no room yet, so the room-only items (P2.2-P2.5,
P2.9, P3.6, P5.1, P5.3) don't constrain the first demo. If Splice gets a room,
its layer loads with the room's content at start (P4.5's rule), and a new
revision reaches players as a content revision.

What this costs: slice 0 grows from medium to large. Moving Splice's walls
into the spatial index and drawing a retained instanced layer are real engine
adoption, not just a list of extra things to draw. The AAA rule "build the
real thing directly, no stopgap" makes that the right trade.

## Order and size

| Slice | Delivers | Size |
|---|---|---|
| 0 | Splice's folder and context; its content tables (pieces, landmarks, kept-clear); layer entities in the World with colliders in the spatial index; its 44 walls moved there too; retained instanced drawing; the run test and walk replay | Large, all in Coax with `#test`s, no deploy |
| 0b | One placement planner in `aethyr/forge/place.coax` | Small to medium |
| 1 | World id and document, fenced edits; `splice` first, admin writes | API + migration: medium |
| 2 | `proto/world/edit-v1` and the codec | Small |
| 3 | `openagents world` + `world_*` MCP | Medium, mirrors `realm_*` |
| 4 | Live follow: Splice writes its world again on a new revision | Small for Splice |
| 5 | Card + pane in `/chat` | Medium |
| 6 | Hosted chat route | Small once 3 exists |
| 7 | WorldBuilder worlds, realms, Everglade and the desktop on the same tools | Large, and follows E11 |

Slices 0-4 make the Splice demo work on the local dev server, with the owner's
Coder and Claude Code in `/chat`, before anything is deployed. Slice 5 puts
Splice beside the conversation. WorldBuilder's worlds get ids in slice 7; they
reuse everything Splice needed except the edit layer.

**Before the first API deploy,** slice 0 can be tried alone: the fixture layer,
loaded with `?layer=fixture` on the dev server, shows the pieces in Splice with
nothing on the server. A layer file written by an agent's `world apply --local`
needs slice 2's wire shape.

## Risks and open questions

- **An edit must never break Splice's run.** The kept-clear zones plus the
  walk replay on save are the guard, and the run test with a full layer keeps
  them honest. A zone the list misses would let an edit block the run, so a
  mutant test removes each zone in turn and expects the walk replay to catch
  it.
- **The look.** Splice is cel and ink. WorldBuilder's Quaternius models are
  not, so Splice's catalogue is its own kit pieces only. Quaternius models in
  Splice would need the cel pass to draw them and are not in the first demo.

- **The edit key is a secret in a URL.** Anyone who has an edit key can edit
  the world. A shared link must be the view link, not the edit link, and the
  page must show which is which.
- **Abuse without sign-in.** Agent edits skip the model's cost, but they can
  still fill the server with worlds. That needs a cap on worlds per address
  each day, shown on the page like the other limits.
- **Two planners.** The server must carry out edits with the same Coax as the
  page (slice 1), or the server and the page will disagree about where a thing
  stands.
- **Deploys.** Slices 1, 3 and 6 need an API deploy. The coordinator runs those
  serially from a green main. Nothing here gives a web page shell access.
