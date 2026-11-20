# GameBuilder and 3D worlds in OpenAgents Desktop

Written 2026-11-20 against openagents `78585b901a` and GameBuilder `ea3efb0`.
Status: audit and proposal. Nothing here is built yet.

This audit covers two things:

1. Folding GameBuilder (`~/work/gamebuilder`) into this repository so that
   OpenAgents Desktop can run the `gamebuilder` command line and its MCP
   tools in a chat.
2. Showing 3D worlds inside chats: a world opened in the side pane or full
   screen, models shown inline, and tool messages that draw a glTF or a level
   in a small live world.

`apps/openagents-desktop` is the main integration surface. Coder Desktop
(`apps/coder-desktop`) gets the same components for free if they live in
`components/app/`.

## Summary

Most of the pieces exist. What is missing is the glue between them.

- GameBuilder already exposes every feature as a row of one command table,
  with the same rows on a CLI, MCP (`gamebuilder mcp`), HTTP
  (`gamebuilder serve`) and a socket into the running app (`--attach`). Its
  results are typed JSON, and it already renders before and after pictures
  of each edit. A chat can show rich results without scraping prose.
- The session record (`coax/lib/agent/session/session.coax`) and the tool row
  component (`components/app/task.coax`) give every tool call a kind, an
  input and an output. A new tool kind and a new body for the open row are
  enough to add world cards.
- The shared pane (`components/app/pane.coax`) already docks right on wide
  windows and goes full width on narrow ones. That is the side pane and full
  screen view.
- The Metaverse Engine already has the hard parts of a scene view: the Apple
  page host (`metaverse-engine/host/apple/PageHost.m`), the editor viewport
  (`metaverse-engine/editor/viewport.coax`), the world follower
  (`metaverse-engine/editor/world-follow.coax`), a bounded glTF/GLB reader
  (`metaverse-engine/anim/import-container.coax`, `gltf-access.coax`,
  `gltf-scene.coax`) and a model inspection viewer (Microduck,
  `apps/openagents-web/microduck.coax`, #13639).
- The design for embedding worlds in UI is already written:
  `components/rendering.md` ("Several inline worlds", "scene view") and
  `docs/protocol/environments-claude-and-world-editing.md` §5.4 and §8
  (authoring binding, preview frames, follow the saved revision).

The gaps:

- The page host draws one page at a time (`phCurrent`). Several live worlds
  in one window need the context audit that `components/rendering.md` asks
  for.
- Desktop has no world view at all. `apps/openagents-desktop/home/app.coax`
  emits a `coax.native.world.open.v1` action (`na_world`, action 13), but the
  desktop hosts don't handle it, and the iOS `NativeWorldHost.swift` that did
  was deleted in #13645.
- The engine reads glTF for characters (skins, morphs, clips), not as a
  general static model viewer. Inline glTF needs a small static-mesh path.
- GameBuilder is proprietary and this repository is open source. Folding it
  in is an owner decision first (see [Decisions](#decisions-for-the-owner)).

Recommendation: build in four phases. Phase 1 ships typed world cards with
still pictures and needs no new renderer. Phase 2 adds one live world in the
side pane and full screen. Phase 3 adds live inline views and glTF. Phase 4
lets edits made in the view flow back into the conversation.

## What exists today

### GameBuilder

| Piece | Where (GameBuilder repo) | What it gives the desktop |
| --- | --- | --- |
| Command table | `coax/commands.coax` | One row per feature: verb, effect, approval, argument kinds and bounds, result. Everything else is made from it |
| CLI | `cli/main.coax`, `docs/cli.md` | `gamebuilder` and `gb`: levels, games, edits (`apply-edits`, `add`, `set-field`, `delete`), `play-test`, `pictures`, `picture`, publish, agents |
| MCP | `coax/mcp.coax` | `gamebuilder mcp` on stdio, about 52 tools named `gb_*`, schemas from the rows, `readOnlyHint` and `destructiveHint` set. Each session is an attached agent with its own record |
| HTTP | `coax/api.coax` | `gamebuilder serve`: 127.0.0.1 only, OpenAPI 3.1, `POST /mcp` |
| Attach | `coax/attach.coax` | A 0600 Unix socket into the running app. An edit lands as one undo step and stays highlighted |
| Skill | `gamebuilder skill --install` | A Claude skill made from the table. Already installed for this account |
| Level as data | `coax/agent.coax` | `world.state.v1`, `gamebuilder.level.v1` with a revision, `world.edit.v1` batches |
| Pictures | `coax/pictures.coax` | `gamebuilder.picture.v1`: JPEGs of the level before and after each agent edit and each play-test, drawn by the level's own renderer |
| Play-test | `coax/playtest.coax` | `gamebuilder.playtest.v1`: headless match with bots, score, stuck bots and where |
| Engine page in the app | `host/page/world-face.c`, `coax/world.coax` | The engine's page host draws the level, as any realm is drawn |
| Linux host | `host/linux/main.c` | Wayland and Vulkan, on `metaverse/gpu` |
| Pin | `openagents.lock`, `scripts/pin.sh` | Builds against one openagents commit |

The hosted service already exists on openagents.com
(`coax/web/api/gamebuilder-agent-*.coax`, `proto/coder/gamebuilder-agent-v1.json`),
and Coder Desktop already runs `gamebuilder` through the skill
(`apps/coder-desktop/README.md`, "List my GameBuilder levels").

### OpenAgents Desktop

- Two shells (`apps/openagents-desktop/README.md`). The recovery shell is the
  Zeron-fork GPU chat (`zeron/screen.coax`, drawn on the shared AppKit/Metal
  and Wayland/Vulkan hosts). The native client (`native/`) is the Coax Native
  app shared with iPhone, with `coder host serve` owning Claude processes.
- Tool calls arrive as session parts: `P_TOOL` with a kind from
  `coax/lib/agent/session/kinds.coax` (`TK_EXEC`, `TK_MCP`, `TK_TOOL`, …).
  MCP calls keep their full name, such as `mcp__gamebuilder__gb_apply_edits`.
- `components/app/task.coax` draws a tool row (`AppTool`: title, argument,
  state, input, output, and a `diff` body). It has a precedent for a typed
  body: the unified diff card.
- `components/app/pane.coax` is the inspector pane: docked right at 320 to
  900 px on wide windows (`APP_PANE_SIDE`), full width on narrow ones
  (`APP_PANE_FULL`).

### Metaverse Engine and protocols

- `PageHost.m` draws a realm on Metal from its face, shaders and VPK1 pack.
  It runs one page at a time (`phCurrent`).
- `components/rendering.md` defines a scene view (a camera onto an
  engine-owned world, not a second engine) and the rules for several inline
  worlds: separate lifetimes, offscreen targets composited with UI, one
  shared memory budget, input only after an explicit click, muted by
  default, paused offscreen.
- `docs/protocol/environments-claude-and-world-editing.md` defines
  `AuthoringTarget`, `AuthoringBinding`, `AuthoringEffect`, `PreviewRequest`
  and `PreviewFrame`. A frame names the revision it actually drew. Its §8.3
  "Milestone 2A" is exactly the side-pane preview this audit proposes.
- `apps/openagents-cli/realm-world.coax` gives the same `world.edit.v1`
  commands for realms on openagents.com (`realm world-place`, …).
- Model viewing: `metaverse-engine/anim/import-container.coax` reads `.gltf`
  and `.glb` within bounds and treats them as data. The Microduck viewer
  (`apps/openagents-web/microduck.coax`, `docs/protocol/rigid-assembly-v1.md`,
  `docs/protocol/view-menu-v1.md`) shows orbit, picking, per-part matrices
  and an inspection menu on web and native.
- Stills: the engine's camera shot modules (`metaverse-engine/shot/`,
  `metaverse-engine/cam/shot.coax`) frame a scene for a picture, and
  GameBuilder's picture renderer (`coax/pictures.coax`) already draws a
  level to a JPEG on an editor of its own, without a visible window.

## Part 1: fold GameBuilder into this repository

### Proposed layout

| Path | From GameBuilder | Notes |
| --- | --- | --- |
| `apps/gamebuilder/core/` | `coax/` | The editor core, command table, MCP, API, agents |
| `apps/gamebuilder/cli/` | `cli/` | Installs `gamebuilder` and `gb` as today |
| `apps/gamebuilder/shared/` | `shared/` | Game types and kinds. Over time, the general parts move to `metaverse-engine/` (GameBuilder's own `AGENTS.md` already says so) |
| `apps/gamebuilder/host/` | `host/`, `project.yml` | The standalone Mac, iOS and Linux apps keep building |
| `apps/gamebuilder/web/` | `web/` | gamebuilder.org, deployed as today |
| `apps/gamebuilder/docs/` or `docs/gamebuilder/` | `docs/` | Keep the history; the business docs may need to stay private |

What changes when it lives here:

- Drop `openagents.lock`, `scripts/pin.sh` and the `vendor/openagents` link.
  `#load` paths point into the same tree. Engine fixes and GameBuilder fixes
  land in one commit instead of a push, a bump and a fix.
- Keep the command table as the single source of truth. The desktop reads
  `gamebuilder commands` (`gamebuilder.commands.v1`) to know which tools are
  reads, which change a world and which need approval.
- Keep the "every 3D scene uses the engine" rule. The move makes it easier:
  the hand-written `Shaders.metal` passes that GameBuilder lists as debt can
  be replaced by the engine renderer in the same change.
- Bring the checks along: `scripts/check.sh`, `cli/check.sh`,
  `tools/game-types-check.coax` and the command golden tests become targets
  under the repository's normal check flow.
- Move history with `git subtree add` or a filtered import, so blame and the
  issue references in comments survive.

### Install the CLI with the desktop

The desktop should not depend on someone having run
`scripts/install-cli.sh` in another checkout.

1. Build `gamebuilder` with the desktop and ship it inside the app bundle
   (`Contents/Helpers/gamebuilder`) and the Linux package.
2. On first use, link it to `~/.local/bin/gamebuilder` only if nothing else
   is there, the same way the current installer treats `gb`.
3. When the desktop's resident service starts a Claude session, register
   the MCP server for that session (`gamebuilder mcp`, or
   `gamebuilder mcp --attach` when the GameBuilder app is open). Each session
   is then an attached agent with its own record, which GameBuilder's Agents
   pane already shows.
4. Keep the skill installed (`gamebuilder skill --install --global`) so that
   plain `gamebuilder ...` Bash calls also work.
5. Show the CLI's version and table digest in the desktop's settings, from
   `gamebuilder version`.

Login stays the person's. The skill already tells agents never to log in for
the person. The desktop can show a "Log in to GameBuilder" button that opens
the existing browser flow.

## Part 2: 3D worlds in chats

### Four views of one world

Every view below is a scene view in the sense of `components/rendering.md`:
a camera onto one world the engine owns. They differ only in size, input and
how often they draw.

| View | Where | Draws | Input | When |
| --- | --- | --- | --- | --- |
| Still | Inside a tool row | One picture of a named revision | None | Default for every world result. Costs nothing after it loads |
| Inline live | Inside a tool row, about 360×220 | At most 30 frames a second while visible, paused offscreen | Orbit after a click; Esc gives the pointer back | A glTF or level the person clicks to explore |
| Side pane | `components/app/pane.coax`, docked right | Full rate | Full editor viewport: fly, orbit, select, gizmos | "Open in side pane" on any world card, or when Claude opens a level |
| Full screen | Pane in full mode, or its own window | Full rate | Editor or play | "Full screen" on the pane, or Play |

Rules that hold for all four:

- A view shows the revision it actually drew. A card for revision 12 never
  silently shows revision 14. When the world has moved on, the card says
  "newer version available" and the pane follows the newest revision.
- Only one view owns the pointer and keys at a time. Scrolling the transcript
  over an inline view scrolls the transcript.
- Views are muted unless they are full screen.
- Only the focused view draws at full rate. Inline views drop to stills when
  more than one is on screen, until the multi-view work in phase 3 lands.

### Tool message components

Add one tool kind and a small set of typed bodies. The body is chosen from
the tool's result JSON, never from the model's prose.

1. Add `TK_WORLD` to `coax/lib/agent/session/kinds.coax`. The engine layer
   sets it for `mcp__gamebuilder__gb_*` calls, for Bash calls whose command
   is `gamebuilder` or `gb`, and for the `realm_*` tools. Recognition is by
   the exact tool name or the parsed command word, after the call is already
   routed, which keeps it inside the workspace's routing rule.
2. Add a `world` body to `AppTool` beside `diff`, filled from a parsed
   result record, so `components/app/` still loads no engine code.
3. The bodies:

| Card | From | Shows | Actions |
| --- | --- | --- | --- |
| World edit | `apply-edits`, `add`, `set-field`, `delete`, `realm world-*` | The result's `summary` ("added 2 blocks, moved 1"), before and after pictures side by side, the revision, refusals in words | Open, Undo (`agent-undo` or `undo-turn`), Show changes |
| Level | `read-level`, `new-level`, `list-levels` | A still of the level, its name, game type and counts | Open, Play |
| Play-test | `play-test` | The play-test picture, score, knockouts, stuck bots marked on the picture | Open at a stuck bot, Run again |
| Model | A `.gltf` or `.glb` path or artifact in a tool result, or a `show-model` tool | A still, then a live inline view on click: orbit, zoom, part list, bounds | Open in side pane, Full screen |
| Engine change | `triage`, `engine-ready`, `engine-mine` | Which module would change and the pull request state | Open PR |

Phase 1 needs no renderer on the desktop for the first three cards:
GameBuilder already writes the pictures (`pictures LEVEL`,
`picture LEVEL REVISION before|after|playtest`). For realms on
openagents.com, use the headless shot path and the `PreviewFrame` record
from the environments protocol.

### The desktop's world view host

Build one host object that owns a scene view, and use it for all four views.

- **Input:** a target (`AuthoringTarget`: authority, world, lane) or a model
  artifact, plus a camera and a size.
- **Output:** frames drawn to an offscreen color and depth target that the
  component compositor places in the view's rectangle (the
  "external scene output" capability in `components/rendering.md`).
- **Mac:** start from `PageHost.m`. For phase 2, one live view at a time is
  enough and fits `phCurrent` as it is. For phase 3, do the context audit
  that `components/rendering.md` asks for: move every static per-page value
  into an explicit context, version the callback ABI, and keep
  `aethyr-host-v1` working through an adapter.
- **Linux:** the Vulkan twin on `metaverse/gpu`, as GameBuilder's Linux host
  already uses.
- **Follow edits:** for a world on openagents.com, `world-follow.coax`
  applies only the next revision and reloads on a gap. For a local
  GameBuilder level, watch the level file's revision, or ask through the
  attach socket when the GameBuilder app has the level open. Keep the
  camera and selection across revisions, as the follower already does.
- **Handle the existing action:** implement `coax.native.world.open.v1`
  (action 13 from `na_world`) on the desktop hosts by opening the side pane,
  so Home's existing world links work too.

### glTF inline

The engine reads glTF today for characters. A general model card needs a
static path:

1. Add `metaverse-engine/content/model-view.coax`: take the bounded
   container from `import-container.coax` and `gltf-access.coax`, and build
   static meshes, node transforms and base-color materials. Skins and clips
   come later through the existing `gltf-scene.coax` path.
2. Draw with the engine renderer and the orbit camera from
   `metaverse-engine/cam/shot.coax`. Reuse the Microduck viewer's picking and
   inspection menu (`assembly-pick.coax`, `view-menu-v1`) for the part list.
3. Bound everything: file size, vertex and triangle count, texture size,
   node depth. Refuse external URIs and anything that is not data. The
   container module already says it never makes a network request; keep it
   that way.
4. Frame the model on load from its bounds, on a neutral ground and sky, so
   any model looks reasonable without settings.
5. Put the same viewer on web as a GPU island, so a shared chat on
   openagents.com shows the same card.

### Edits from the view back into the chat

When the person moves a block in the side pane, Claude should know.

- An edit made in the view is a `world.edit.v1` batch attributed to the
  person, sent through the same command (`apply-edits` or the realm write),
  with the revision it was made from.
- The conversation gets a compact part that says what changed, in the same
  words as a tool card ("you moved block 4"). The next turn's context carries
  the selection and the last picked point through `world.turn.v1`, as §8.1 of
  the environments protocol describes.
- Claims (`coax/claims.coax`) keep the person and an agent from editing the
  same things at the same time. Show a claimed area in the view.

## Phased plan

| Phase | Builds | Done when |
| --- | --- | --- |
| 0. Move | GameBuilder in `apps/gamebuilder/`, pin removed, checks wired, history kept | `gamebuilder` builds from this tree, its checks pass, and the standalone apps still build |
| 1. Cards | Bundled CLI and per-session MCP; `TK_WORLD`; world edit, level and play-test cards with pictures | In the desktop, "make me an arena with ramps" shows edit cards with before and after pictures, and Undo on a card takes the edit back |
| 2. Pane | World view host, one live view; side pane and full screen; follows revisions; action 13 handled | "Open" on a card shows the level live in the pane; an edit by Claude appears in the pane within a second at the new revision |
| 3. Inline | Page host context audit; several views with one budget; static glTF model view; model card on desktop and web | Three inline views in one transcript draw correctly, scrolling never steers them, and a `.glb` from a tool result opens as an orbitable model |
| 4. Round trip | Edits from the pane as attributed batches; turn context; claims shown | Moving a block in the pane shows a "you moved" part, and Claude's next turn refers to it correctly |

Each phase adds proto records where a new shape crosses a module or the
wire (`proto/`, one codec and a registry row), as `components/rendering.md`
requires.

## Decisions for the owner

1. **License.** GameBuilder is proprietary (its `LICENSE` and the
   2026-11-16 decision in its `docs/openagents-integration.md`). This
   repository is open source. Folding it in either makes GameBuilder open
   source, or needs a private subtree or a separately licensed folder.
   The business docs in `docs/business/` probably should not move either
   way.
2. **Which desktop shell first.** The Zeron GPU shell already draws
   everything on Metal and Vulkan, so compositing a world into it is
   simplest. The native client is where the product is going. This audit
   assumes the native client, with the world view host shared so the Zeron
   shell can use it too.
3. **Standalone GameBuilder app.** Keep shipping it, or make GameBuilder a
   mode of the desktop app. The proposal above keeps it; the desktop then
   talks to it through `--attach` when it is open.

## Risks

- **Memory and heat.** Several live worlds can take all the GPU memory. The
  single shared budget and the "stills unless focused" rule are what keep
  this safe; build them before allowing more than one live view.
- **Showing the wrong revision.** A card must never imply an edit rendered
  when it didn't. "Saved, picture unavailable" is a valid state.
- **Untrusted realms.** Realms we don't own stay in their sandboxed frames
  on web (`realm-frame.v1`). Trusted inline views and untrusted realms must
  not share a device or a texture.
- **Two tool surfaces.** GameBuilder's `gb_*` tools and the realm `realm_*`
  tools both edit worlds with `world.edit.v1`. Keep one card for both, keyed
  by the result shape, so the chat does not grow two ways to show an edit.
