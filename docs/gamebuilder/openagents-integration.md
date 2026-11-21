# How GameBuilder should use OpenAgents

Written 2026-11-16 against openagents `19ff5d7715` and Coax `coax0 0.52.0`.
Status: findings and a plan. The owner answered the first round of questions
on 2026-11-16; the answers are under [Decisions](#decisions) and the plan
below follows them. Stage 0 (pinning) is built.

GameBuilder is meant to let people make games on iPhone, iPad and Mac. Today
it is a box-placing level editor. This document records exactly what it takes
from the openagents checkout now, what openagents already has that it should
take next, how the dependency itself should work, and a staged plan.

## Decisions

The owner's answers, 2026-11-16.

| Question | Decision |
| --- | --- |
| Why a separate repo? | GameBuilder is proprietary. The engine and Coax are open source and keep improving in openagents. The interface, editing model, game templates and business logic stay here |
| May GameBuilder agents change openagents? | Yes, whenever needed |
| Pinning | A lock file and a pinned worktree (left to the agent; built as stage 0) |
| Interface | The GPU component kit now, in place of SwiftUI |
| What is a game on the server? | A Metaverse world, until there is a reason to change |
| WorldBuilder | Probably deprecated, or it reuses GameBuilder's code. GameBuilder is the one being developed |
| First game type | A team arena brawler played in first person with a free camera, with a roster of distinct characters. See [The first game type](#the-first-game-type) |
| How creators write rules | Unknown. Start simple |
| Accounts | The OpenAgents account, the same login as the OpenAgents apps. Built 2026-11-16: an email code through openagents' `AppAuth.swift` |
| Who edits the official games | Anyone plays Scrappers Arena and SPLICEBOTS; only approved owners edit them, associated by hand. `chris@openagents.com` owns all three (`coax/games.coax`) |
| Money | Postponed |

Two consequences follow from the first row:

- **The dividing line is "engine or product".** A camera move, a gizmo, a
  weapon kernel or a renderer feature is engine work and goes to openagents,
  where it is public. A game template, the touch editing flow, the roster
  editor and anything about accounts, sharing or pricing is product and
  stays here.
- **The licence is proprietary.** `LICENSE` was Apache-2.0 by mistake; the
  owner never chose it. It is now an all-rights-reserved notice.

## Summary

1. **GameBuilder uses a small corner of what is available.** It takes the Coax
   compiler, the editor viewport, the System One codec and the NIP-98 signer.
   The engine also has an undoable edit log, a save-file format, asset packs,
   a play loop, a walking character, a follow camera, a sandboxed scripting
   language and a publish path. GameBuilder has none of these yet and should
   not write its own.
2. **The dependency was unpinned.** The build read whatever commit and
   compiler binary happened to be in `../openagents`. This is fixed: see
   [How the dependency works](#how-the-dependency-works).
3. **The two stand-ins named in `AGENTS.md` have real replacements.** The
   SwiftUI toolbar should become the shared GPU component kit, drawn by the
   shared Apple host (`apps/shared/host/apple/ui-host.m`). The hand-written
   Metal shader should become Coax, emitted with `coax metal`. Everglade's
   Mac Workshop already works this way and is the model to copy.
4. **The closest existing product is WorldBuilder** (`apps/openagents-web/worldbuilder.coax`),
   a local-only world editor on the same viewport, with undo, a terrain
   brush, chat-driven placement and "walk into it". GameBuilder should be
   the native sibling of WorldBuilder and share its Coax, not a parallel
   design.
5. **Three things need changes inside openagents**, which the owner has
   allowed: orbit, pan, dolly and focus in `viewport.coax`; a world kind for
   GameBuilder games on `/api/worlds`; and a way to consume the shared Apple
   host from outside the monorepo.

## What GameBuilder takes today

Measured by following every `#load` from `coax/editor.coax` (check loads
excluded).

| What | From | How it is reached |
| --- | --- | --- |
| The compiler | `coax/bin/coax` (ignored build output in that checkout) | `scripts/env.sh`, `OPENAGENTS_ROOT` |
| Editor viewport: picking, fly camera, snapped move, turn and size drags | `metaverse-engine/editor/viewport.coax` | `#load "../../openagents/..."` in `coax/editor.coax` |
| System One request and answer codec | `coax/lib/systemone/answers.coax` | `#load` in `coax/decide.coax` |
| NIP-98 signer | `coax/lib/nostr/nip98_sign.coax` | `#load` in `coax/decide.coax` |
| App Store Connect tool | `apps/openagents-ios/tools/app_store.py` | `scripts/release.sh` |
| Release script pattern | `apps/everglade-mobile/ios/release.sh` | copied by hand |
| The decision service | `POST https://openagents.com/api/v1/systemone` | at run time |

Those three `#load` lines pull in **75 openagents files, about 17,600
lines**:

| Folder | Files | Why |
| --- | --- | --- |
| `coax/lib/crypto` | 36 | secp256k1 and SHA-256 for the signer |
| `coax/lib/nostr`, `coax/tests/nostr` | 20 | event signing, JSON, fixtures |
| `metaverse-engine/physics` | 6 | `query_ray` and the colliders |
| `metaverse-engine/editor` | 4 | `viewport`, `world-edit`, `content-edit` |
| `metaverse-engine/core` | 3 | spaces, angle |
| others | 6 | JSON, quantity, content tables, System One |

Of the viewport, GameBuilder calls 19 `vp_*` procedures and reads one
`WorldEdit`. It does not use the viewport's play, pause and step clock, its
simulate mode or its terrain brush.

What GameBuilder writes itself, in 1,270 lines of Coax and 680 of Swift and
Metal:

- the scene: a fixed array of 64 `Actor` boxes, with no undo and no save;
- orbit, pan, dolly and focus;
- input bindings for mouse, keys and touch;
- the camera matrix and one instance per box;
- a Metal shader for lit boxes, the grid and selection edges;
- a SwiftUI toolbar;
- Decide: the state it sends, the six questions and carrying out the answer.

## Problems with how it was pulled in

As found before stage 0. The first three are fixed by the pin.

**Nothing is pinned.** A build uses the working tree of `../openagents`,
whatever branch and commit it is on. Build 1 went to TestFlight on
2026-11-16 and nothing records which openagents commit it contained.
The compiler binary is separate again: `coax/bin/coax` is ignored build
output, so it can be older than the sources beside it. `AGENTS.md` already
warns about this.

**The checkout is shared and often dirty.** While writing this, the main
`openagents` checkout had 54 uncommitted files under `apps/openagents-web`
from another agent. None are in GameBuilder's closure today. Once GameBuilder
loads more of the engine, a release built from that checkout could ship
someone's half-finished work.

**The paths are brittle.** `#load "../../openagents/..."` is written into
the Coax sources, so the checkout must sit at exactly `../openagents`.
`OPENAGENTS_ROOT` moves only the compiler: a different root finds a different
`coax` but the sources still load from the sibling folder. The two can
disagree.

**There is no package system to lean on.** `coax/LIBRARIES.md` says it is
not a registry. Loads resolve relative to the loading file. Compilers are
published per source fingerprint (`coax/published.sh`, GitHub releases
tagged `coax-build-<16hex>`), but only so that `coax/build.sh` can fetch the
one matching its own tree. In practice, one openagents commit fixes both the
compiler and the libraries, and that is the unit to pin.

**Engine API changes arrive unannounced.** The engine is moving fast (#13257,
the AAA program; `consumers.txt` marks `editor/`, `render/` and `host/` as
still being consolidated). Anything GameBuilder loads can change shape on any
pull. The compiler does have the tools to notice: `coax deps FILE --modules`
prints a content hash per file and per module, and `coax compat OLD NEW`
compares published surfaces.

## How the dependency works

**Built as stage 0 on 2026-11-16.** The proposal was: pin one openagents
commit in this repo, and build releases from a clean worktree at that
commit. What exists now:

- `openagents.lock` holds the commit (`19ff5d7715`, the one the app was
  already building against).
- `scripts/pin.sh sync` makes a git worktree at `../openagents-gamebuilder`
  at that commit and builds its compiler. `pin.sh bump` moves the lock.
- `vendor/openagents` (ignored) links to the tree in use. The compiler and
  every `#load` go through it, so they cannot come from different trees.
- `scripts/release.sh` refuses unless the tree is at the locked commit, its
  compiler was built from that tree's sources, and no loaded file is changed
  or untracked. It writes the commit into the app as `GBOpenAgentsCommit`.

The original reasoning follows.

- Add `openagents.lock`: one line, the openagents commit. A second line can
  hold the compiler fingerprint (`coax/provenance.sh fingerprint`).
- `scripts/env.sh` compares `git -C "$OPENAGENTS_ROOT" rev-parse HEAD` to
  the lock. Day-to-day builds warn on a mismatch. `scripts/release.sh`
  refuses, and also refuses if any file in the load closure is modified
  (`coax deps coax/editor.coax` lists the closure).
- `release.sh` records the openagents commit in the archive, for example as
  an Info.plist key, so a TestFlight build can be traced.
- Releases build from a dedicated worktree (`git worktree add
  ../openagents-gamebuilder <commit>` then `coax/build.sh` there), never
  from the shared main checkout. That also follows the workspace rule that
  main checkouts stay clean.
- Bumping the lock is a normal change: pull, run `scripts/build.sh`, fix
  what moved, commit the new lock with the fixes.

**Make the path one setting.** Either a symlink `vendor/openagents` that
`env.sh` creates from `OPENAGENTS_ROOT`, with every `#load` going through
it, or a compiler load-root flag if Coax has or gains one (I did not find
one). Then the compiler and the sources always come from the same tree.

**Alternatives considered:**

| Option | For | Against |
| --- | --- | --- |
| Lock file plus sibling checkout (recommended) | Smallest change; matches how everything already works; lets an agent fix the engine and the app side by side | Still needs a second checkout |
| Git submodule | Pin is native to git; a fresh clone just works | The workspace says child repos are "intentionally not submodules"; openagents is large; compiler still has to be built |
| Vendor the 75-file closure into this repo | Fully self-contained; small | A fork that drifts; directly against "reuse before writing"; closure grows about tenfold once the engine is adopted |
| Move GameBuilder into `openagents/apps/` | No dependency problem at all; `layer-check` and `consumers.txt` cover it | The owner chose a standalone repo; see question 1 |

**Add GameBuilder to the engine's consumer list.** `consumers.txt` exists so
no engine module is orphaned, and `editor/` is waiting on adopters (#13294).
An out-of-repo consumer is invisible to `layer-check --consumers`, so a
module only GameBuilder uses could be deleted as unused. This needs either a
line in `consumers.txt` naming GameBuilder, or a small in-repo check file
that loads what GameBuilder loads. That is an openagents change.

## What to adopt from the engine

All paths are under `openagents/`. "Seen" means I or a survey agent read the
file; maturity claims come from the files' own headers and check counts.

### The document: replace the `Actor` array

| Need | Use | Notes |
| --- | --- | --- |
| Undo, redo, grouped edits | `metaverse-engine/editor/edit.coax` (`EditorScene`, `editor_scene_place/move/rotate/undo/redo`) | 1,445 lines, 20 checks |
| Typed edits | `editor/world-edit.coax` (`world.edit.v1`: spawn, despawn, move, rotate, scale, set column, terrain) | Already loaded. `we_scene_apply` carries a `WorldEdit` out on the `EditorScene` log |
| Save file | `editor/world-doc.coax` (`world.doc.v1`: `world_save`, `world_load`, with a digest) | Saves a `core/world.coax` World |
| The world itself | `core/world.coax`, `core/entity.coax` | Entity store, component columns, ordered systems, `world_digest` for replays |
| "Put three trees beside the house" | `editor/place.coax` | A placement planner; pairs naturally with Decide |
| Authored vs running state | `editor/authority.coax` | Play never writes back into the level |

Today a viewport drag ends as a `WorldEdit` that `ed_apply` writes into the
`Actor` array. The change is to apply it with `we_scene_apply` instead.
Undo, redo and a save file then come from the engine. Decide's edits go
through the same door, so a suggested edit becomes undoable for free.
WorldBuilder (`apps/openagents-web/worldbuilder/worldbuilder-world.coax`)
does exactly this and should be read first.

### Play: press Play and walk around

| Need | Use |
| --- | --- |
| Play, pause, step, simulate | `editor/viewport.coax` already has it (`VT_PLAY`, the fixed clock, `vp_hands`) |
| Frame loop and input | `core/loop.coax`, `core/input.coax` (keys, touch and pads to one `Intent`) |
| Walking, jumping | `move/walk.coax`, `move/jump.coax`, `move/stance.coax` |
| Collision | `physics/query.coax` (already loaded), `physics/collide.coax` |
| Camera in play | `cam/follow.coax`, `cam/shot.coax` |
| Character | `character/character.coax` |
| Things to do | `action/` (carry, vitals, sensors, break), `world/interact.coax` |
| NPCs | `ai/steer.coax`, `ai/npc-plan.coax`, `talk/` |
| Game data | `content/tables.coax`, edited through `editor/content-edit.coax` |
| Sound | `audio/` |

### Rules: how a creator makes it a game

The engine has a user scripting language: `#script` restricted Coax, compiled
to sandboxed wasm, run by a fuel-metered interpreter with rollback
(`coax/script-profile.md`, `metaverse-engine/script/`). Scripts are stored as
content-table rows, so editing a script is an undoable edit like any other.
Compiling is a server job today (`editor/shell-scripts.coax`).

For a phone app, typed code is the wrong first surface. Two fits:

- **Decide writes rules.** GameBuilder already asks a decision model typed
  questions about the scene. The same pattern can pick from a fixed list of
  rule templates ("touching this ends the level", "collect all of these to
  win") that are stored as script rows.
- **Scripts later**, for creators who want them, and on the Mac first.

### Assets: more than boxes

| Need | Use |
| --- | --- |
| Import a glb, PNG or sound | `editor/pack-import.coax` (every asset must declare a licence) |
| Asset packs | `editor/pack.coax` (VPK1), `pack-seal.coax`, `page-pack.coax` |
| Asset identity | `core/assets.coax` (an asset is the SHA-256 of its bytes) |
| Ready-made props, land, bodies | `metaverse-engine/kit/`, `metaverse-engine/arch/` (parametric buildings and flora) |
| Terrain | `world/terrain-tile.coax`, `physics/ground*.coax`, and the viewport's land brush (`vp_dab`) |
| Export | `export/glb-write.coax` |

### Drawing: replace `Shaders.metal`

There are two levels, and GameBuilder should take them in order.

1. **Shaders written in Coax.** `coax metal FILE --vertex V --fragment F`
   emits Metal from Coax. Everglade's Workshop does this
   (`everglade/workshop/gpu/temple.coax`, `apps/everglade-desktop/Workshop.metal`).
   Moving the box, grid and selection shading into Coax removes the last
   logic that lives in a host-only file, and makes it checkable.
2. **The engine's renderer.** The contract is `metaverse-engine/host/README.md`
   (`aethyr-host-v1`): the Coax side emits a frame plan
   (`render/graph/frame-plan.coax`, passes for background, shadow, scene,
   particles, bloom, post and HUD) and the host executes it. A Metal host
   exists (`metaverse-engine/host/apple/PageHost.m`), used by
   `apps/everglade-desktop`, `apps/everglade-mobile` and
   `apps/metaverse-engine-desktop`. It expects a full realm "page": eight
   shader pairs and a VPK pack, built per realm by `metaverse/native/build.sh`.
   It now rests on one frame graph (#13303, closed) and one C host core and
   pack reader (#13304, closed). Adopting it is still the largest piece:
   GameBuilder's level would be drawn as a page, with its own face, its
   emitted shader pairs and a pack.

   **Adopted for the website's nine game types (issue 2, steps 4 and 5).**
   A level of the meadow's four types, the village, crates, the platformer,
   racing or drift is a page in the Mac and iOS apps: `coax/world.coax`
   loads the website's scene (`web/hud.coax`) into the core, `host/page/world-face.c`
   is the page's face over it, `scripts/world-page.sh` emits the seven shader
   pairs from the engine's own Coax sources and packs the village, and
   `host/Sources/WorldPage.swift` opens `PageHost.m` on them. The editor and
   Play are both that picture: the level held still under the editor's
   camera, or the scene's world running. Arena, Adventure, Open Field and
   SPLICEBOTS are still `Shaders.metal`'s; moving them is what is left of
   this section.

   The grass and the blend are the engine's too. The page host has a grass
   pass and a per-draw blend as `coax/web/host.js` has them (openagents
   #13654, pinned since 2026-11-17), so the app's meadow has its blades and
   the character's ghost its blend. The scene hands both over through the
   face (`coaxial_grass_instances`, `coaxial_grass_vertices_v1`,
   `coaxial_draw_blend_v1`).

### The interface: replace the SwiftUI toolbar

`components/gpu-components.md` (2026-11-15) is the plan of record for native
apps: every component is drawn by the shared display-list kit on Metal, and
the platform supplies only the active text field, accessibility and system
sheets. Note that `coax/native.md`, which this repo's README links, still
describes the older design.

- Controls: `components/kit/controls.coax` (`ctx_button`, `ctx_toggle`,
  `ctx_slider`, `ctx_tabs`, `ctx_list`, `ctx_panel`), `kit/frame.coax`,
  `components/app/`.
- Skin: `components/skin/mono.coax`, the look `AGENTS.md` already asks for.
- Host: `apps/shared/host/apple/ui-host.m`, with the contract in
  `apps/shared/host/linux/ui-host.h`. The app fills a `UiApp` of callbacks
  and the host draws the vertices Coax returns.
- 3D under the UI: `ui_layer` in `ui-host-apple.h` lets the app draw its own
  Metal pass, with depth, beneath the components. This is how the viewport
  and the toolbar share one view.
- Accessibility and text entry: `apps/shared/native/KitAccess.swift`,
  `KitEditor.swift`.
- The editor's panels as data: `editor/shell-model.coax` (`editor.model.v1`)
  describes outliner, inspector and toolbar as a plain model. The full
  `editor/shell.coax` is DOM-backed and web only, but the model is reusable.

Adopting this means `host/Sources` shrinks to an app entry point plus the
shared host, and App.swift's toolbar moves into Coax. It is the largest
single change here, and it has one blocker: these files are compiled
straight from paths inside the monorepo. There is no Swift package. See
question 4.

Known gaps in the kit: images, affine transforms and blur. Physical-phone
checks of the kit are still open on the openagents side.

### Sharing a game

The live path is `openagents.com`'s `/api/worlds/ID` with `?lane=dev|live`
and `/events` (`coax/web/api/worlds.c`, `editor/world-live.coax`,
`world-follow.coax`). Publishing copies the dev lane to live. It accepts
NIP-98, which GameBuilder already signs with a device key for Decide, so the
identity is in place.

Two blockers on the server: `worlds.c` knows only the kinds `realm` and
`splice-bots`, and making realms is gated by a creator grant in production.
A GameBuilder game needs its own kind, or to be a `realm`. See question 5.

## What GameBuilder should keep owning

- The product: what a creator sees, the touch-first editing model, the
  templates and the flow from empty level to shared game.
- Input bindings for touch and for mouse and keys, which are a product
  choice per app.
- Decide: the state, the questions and how answers become edits.
- The thin host: windowing, the Keychain key, the one HTTPS request,
  signing and release.

Everything else should come from the engine or go into it.

## What belongs upstream in openagents

These are things GameBuilder has written or will need that every editor
should share. This repo may not edit openagents, so each needs the owner's
go-ahead or an issue there.

How a person's request becomes an engine pull request from their own fork
is designed in [engine-upgrades.md](engine-upgrades.md).

1. **Orbit, pan, dolly, focus and the eye floor** from `coax/editor.coax`
   into `metaverse-engine/editor/viewport.coax`. `AGENTS.md` already says so.
   About 200 lines with their checks.
2. **Touch bindings for the viewport** (one finger orbits or edits, two pan,
   pinch dollies). WorldBuilder and the web editor will want the same on a
   phone.
3. **A world kind for GameBuilder games** on `/api/worlds`.
4. **A consumable Apple host**: a folder or package boundary around
   `apps/shared/host/apple` and `apps/shared/native` that an outside app can
   compile without reaching into `apps/openagents-ios`.
5. **A `consumers.txt` line** for GameBuilder.
6. **The App Store Connect tool** (`apps/openagents-ios/tools/app_store.py`)
   somewhere app-neutral. `release.sh` currently has to rewrite "OpenAgents"
   to "GameBuilder" in its output.
7. **A decision "next edit" library.** If WorldBuilder wants Decide, the
   scene-to-request and answer-to-`WorldEdit` code in `coax/decide.coax`
   belongs beside `editor/place.coax`.

## Proposed order

Each stage ships on its own and leaves the app working.

| Stage | Change | Needs openagents change? |
| --- | --- | --- |
| 0 | **Done.** Pin: `openagents.lock`, `scripts/pin.sh`, the pinned worktree, the commit recorded in the build, one path setting | No |
| 1 | **Done.** Document: `EditorScene` and `we_scene_apply` in place of the `Actor` array; undo and redo; save and reopen with `world-doc`; several levels on disk | No |
| 2 | **First playable built** (`coax/play.coax`, 2026-11-16): a 3-a-side first-person arena with bots on the engine's collision queries, its own walker and bots for now. Remaining: adopt the engine's World, walker, camera and steering. Play: the viewport's play tool, `core/loop`, `move/walk`, a first-person camera, spawn points; Play and Stop in the toolbar. One character walking the arena, no combat yet | No |
| 2b | Combat: a primary attack, health, a bot opponent, win and lose. See [The first game type](#the-first-game-type) | Likely (missing kernels go upstream) |
| 3 | Shaders in Coax through `coax metal`; delete the hand-written shading | No |
| 4 | Upstream the camera moves and touch bindings; delete them here | Yes |
| 5 | GPU component kit and the shared Apple host in place of SwiftUI. Decided: do this now, so it moves ahead of stages 3 and 4 | Yes (item 4 above) |
| 6 | Assets: kit props, glb import, terrain brush | No |
| 7 | Rules: Decide picks rule templates stored as script rows; scripts later | Maybe (script compile is a server job) |
| 8 | Share: publish to a live lane, open someone else's game | Yes (world kind, creator grant) |
| 9 | The engine renderer through the frame plan and `PageHost` | Yes: #13303 and #13304 are closed. Not started |

Order after the decisions: 0 (done), then 1 and 5 next, then 2 and 2b.
Stage 1 matters most: every later stage assumes the level is an engine World
and not a private array. Stage 5 is next because every screen added before
it would be SwiftUI that has to be rewritten.

## The first game type

The owner's brief: a team arena brawler in the style of the mobile brawler
genre, but played in first person with a camera you can move, and with a
roster of characters that have their own name for what they are.

GameBuilder ships its own game here: its own name, characters, art, maps
and sounds. The genre and its mechanics are the reference, not any one
game's content.

What a creator makes in this template:

- **An arena**: walls, cover, spawn points per team, and objectives. This is
  what the level editor already edits, plus markers.
- **A roster**: each character has health, speed, a primary attack, a
  charged special and a look.
- **A mode**: the win rule. Start with one (most knockouts in a time limit,
  or hold a point).

What the engine has for it, and what looks missing:

| Need | Engine | State |
| --- | --- | --- |
| First-person camera | `cam/follow.coax` has first- and third-person shots | Present |
| Walk, jump, sprint | `move/walk.coax`, `jump.coax`, `stance.coax` | Present |
| Health, knockouts | `action/vitals.coax` | Present |
| Aimed attacks, projectiles | `action/archery.coax`, `bow.coax`, `area-aim.coax`, `target.coax`, `telegraph.coax` | Present for bows and area aim; a general gun or projectile kernel needs checking |
| Abilities with charge | `action/deed.coax` (the deed bar), `action/bot-battle.coax` (cards and a battery) | Close; needs reading |
| Hit reactions | `action/react.coax` | Present |
| Bot opponents | `ai/steer.coax`, `ai/plan.coax`, `ai/stimulus.coax`, `ai/pilot.coax` | Present |
| Line of sight | `physics/visibility.coax` | Present |
| Characters and animation | `character/character.coax`, `anim/` | Present; a roster of new characters is content work |
| Touch controls for a first-person game | `core/input.coax` takes touch and pads | Twin-stick touch layout needs checking |
| Real-time multiplayer | `net/` (predict, replication, rooms), `session/` | Present, but server-side rooms; the largest piece by far |

Suggested cut for a first playable: one arena, two characters, bots only,
one mode, on device with no network. Multiplayer comes after that works,
because it needs rooms on the server and an account.

Each "needs checking" row is a read of the named files before stage 2b,
and any missing kernel is engine work for openagents.

## Shared infrastructure for every game

The arena template, SPLICEBOTS and Everglade (`docs/splice-bots-port.md`,
`docs/scrappers-multiplayer.md`) need the same five
things. Status on 2026-11-16:

| Piece | Used by | Status |
| --- | --- | --- |
| The pin at the tip of openagents `main` | All | **Done.** Bumped 78 commits to `42092fc2e9`; nothing GameBuilder loads broke |
| A realm built as a native library from the pinned tree | SPLICEBOTS, Everglade | **Proved.** `metaverse/native/build.sh splice-bots` emits the realm as C in 15 seconds (7.9 MB of C); it compiles for the iOS simulator in 9 more, to a 1.9 MB library with the host face's 52 exports. Not yet wired into the app's build |
| The engine's page host and renderer in the app, in place of the box shader | All | Not started. The largest piece: `metaverse-engine/host/apple/PageHost.m`, the frame plan, emitted Metal shader pairs, the pack reader |
| The room running on a device, and memory and nearby transports | Scrappers multiplayer, Everglade offline, any game alone | Planned (`docs/scrappers-multiplayer.md`). Engine work, in openagents |
| A games library screen and shared touch controls | All | Not started. Waits on the page host: a library with one game in it is not a library |

The order that unblocks the most: the page host first, then the device room.

## Risks

- **Engine churn.** Modules GameBuilder adopts can change or be deleted.
  The lock makes that a deliberate bump instead of a surprise.
- **Size and compile time.** The closure is 17,600 lines now, and
  `editor.coax` already raises `#run_steps` to 100,000,000 to build the
  secp256k1 table at compile time. Adopting World, walk and camera will
  multiply the closure. Measure the generated C size and the build time at
  stage 1, and consider `coax c --split`.
- **Fixed capacities.** The engine takes its storage whole from the realm
  and allocates nothing in a step. GameBuilder's `Editor` is one struct the
  Swift host allocates; a World's storage is much larger and its size has to
  be chosen. This affects memory on older phones.
- **App Review.** User scripts run in an interpreter on data the app
  downloads. Apple permits interpreted code that does not change the app's
  primary purpose, and games made in a game-making app are that purpose,
  but shared games with scripts are worth a deliberate read of guideline
  2.5.2 before stage 8.
- **Shared content.** Publishing other people's games brings moderation,
  reporting and asset licences. The pack format already requires a licence
  per asset, which helps.

## Things I could not confirm

- Whether `ui_layer`'s depth pass is enough for the whole editor viewport,
  or whether the engine page host is needed sooner.
- Whether the frame plan can be used without the full realm page setup.
- Whether the script compiler can run on the device or only on the server.
- How large an engine World's storage is at the caps GameBuilder would pick.
- Any physical iPhone or iPad evidence for the GPU component kit.

## Open questions

2. **What the characters are called.** The brief says not to call them what
   the genre's best-known game does. A name is needed before the roster
   editor has labels.
3. **Third person as well?** The brief says first person with a movable
   camera. Should a creator be able to pick third person or top-down per
   game? The engine has all three shots.
4. **Multiplayer in the first release**, or bots only first?
5. **Team size and modes** for the first template.
6. **How a Metaverse world opens a GameBuilder game.** A game stored as a
   Metaverse world should be playable from the web. That means the game's
   rules have to run in the web engine too, so they must live in engine
   modules or script rows, not in this repo's proprietary Coax. This decides
   how much of the template can stay proprietary.
7. **Which account.** Sign in with an OpenAgents account in the app
   (`apps/shared/host/apple/AppAuth.swift` exists for the OpenAgents app),
   and link the device's Nostr key to it?
