# Chat and world building: audit and proposal

Status: proposal, 2026-11-12. Nothing in it is built yet.

This doc covers Aethyrforge, the other places where a world is edited, and the
new WorldBuilder at `/create-world`. It then proposes how `/chat` and its agents
(Claude Code, Codex, Antigravity and Microcoder through your Coder, plus the
hosted chat) can build worlds.

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
- **Proposal:**
  1. Give a WorldBuilder world an id and a fenced document.
  2. Make its edit line one wire shape.
  3. Expose it as `openagents world ...` CLI verbs and MCP tools, which Coder
     gets by default.
  4. Make the page follow outside edits.
  5. Give `/chat` a world pane and a world card.

  After that, "build me a fishing village" works from any agent in `/chat`,
  and the world builds itself beside the conversation.

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

The chat zero-base audit (`2026-11-11-chat-zero-base-audit.md`) already notes
that the conversation is written five times: web, desktop, the project agent,
Aethyrforge and Coder. WorldBuilder's box is a sixth input line, on the kit
(#13226).

## Proposal: building a world from `/chat`

The rule is the desktop's: **the agent turns words into typed edits; the world
checks them, saves them fenced by version, and shows them.** Our server's model
is one optional source of edits, not the only one.

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
  `worldbuilder-plan.coax` as a native tool, so the server and the page cannot
  disagree. The rule is `#authority`, and `INVARIANTS.md` gains a line for it.

### Slice 2: one wire shape for an edit

- Write `proto/worldbuilder/edit-v1.schema.json` from the fields
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

### Slice 5: the world in `/chat`

- **A world card.** When a turn's tool result names a world, the transcript
  shows a card with its name, a frame from `world shot`, the thing count, and
  OPEN. The hosted chat and the local agents use the same card.
- **A world pane.** OPEN puts the world beside the conversation, and you keep
  talking to the same agent.
  - The pane loads WorldBuilder's module and its 12.2 MB pack only when it
    opens, so `/chat`'s own budget (chat-web, 168,000 bytes) doesn't grow.
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

## Order and size

| Slice | Delivers | Size |
|---|---|---|
| 1 | Id, document, fenced edits | API + migration + page save: medium |
| 2 | `proto/worldbuilder/edit-v1` and the codec | Small |
| 3 | `openagents world` + `world_*` MCP | Medium, mirrors `realm_*` |
| 4 | Live follow + arrivals | Medium (arrivals are engine work) |
| 5 | Card + pane in `/chat` | Medium |
| 6 | Hosted chat route | Small once 3 exists |
| 7 | Realms, Everglade, desktop on one shape | Large, and follows E11 |

Slices 1-3 make the first demo possible: Claude Code in `/chat` builds a world
you then open at `/create-world/ID`. Slices 4-5 make the world build itself
beside the chat.

## Risks and open questions

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
