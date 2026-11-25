# Metaverse Viewer & GameBuilder Right-Side Pane in OpenAgents Desktop

**Status:** Specification — Ready for Implementation (2026-11-24)  
**Scope:** `apps/openagents-desktop/` (Coax page, C bridge, macOS Metal / Linux Vulkan / Windows Vulkan hosts, build & packaging), `gamebuilder/` (`core/`, `cli/`, `shared/`, `web/`, `worlds/`), `metaverse-engine/` (`host/`, `content/`, `editor/`, `import/`, `render/`, `anim/`), `metaverse/gpu/`, `coax/lib/agent/desktop/`, `proto/`, `assets/registry/collections/`, `contexts.txt`.  
**Related & Prior Specs:**
- Extends and operationalizes [2026-11-20-gamebuilder-3d-worlds-audit.md](2026-11-20-gamebuilder-3d-worlds-audit.md)
- Follows the right-side expandable pane pattern established by `apps/openagents-desktop/page/filepane.coax` (#13760, #13881, #13992) and [ios-simulator-spec.md](ios-simulator-spec.md) (#13811)
- Honors the 2026-11-20 app-consolidation decision in [../audits/2026-11-20-gamebuilder-monorepo-migration-audit.md](../audits/2026-11-20-gamebuilder-monorepo-migration-audit.md)
- Conforms to [../../components/rendering.md](../../components/rendering.md), [openagents-v1-spec.md](openagents-v1-spec.md), [steer-ple-from-chat.md](steer-ple-from-chat.md), [../protocol/environments-claude-and-world-editing.md](../protocol/environments-claude-and-world-editing.md), and [../../gamebuilder/INVARIANTS.md](../../gamebuilder/INVARIANTS.md)

---

## Table of Contents

1. [Product Vision, Goals & Hard Invariants](#1-product-vision-goals--hard-invariants)
2. [Current Codebase Baseline & Concrete Gaps](#2-current-codebase-baseline--concrete-gaps)
3. [Unified Right-Side Pane UX, Chrome & Shell Integration](#3-unified-right-side-pane-ux-chrome--shell-integration)
4. [Auto-Opening & 3D Trigger Routing Matrix](#4-auto-opening--3d-trigger-routing-matrix)
5. [Metaverse Viewer Mode (3D Models, Scenes & Realms)](#5-metaverse-viewer-mode-3d-models-scenes--realms)
6. [GameBuilder Interface Inside the Pane](#6-gamebuilder-interface-inside-the-pane)
7. [Rendering Engine, Context Audit & Cross-Platform Host Architecture](#7-rendering-engine-context-audit--cross-platform-host-architecture)
8. [Coax Page State, C-ABI Bridge, Flags & Proto Contracts](#8-coax-page-state-c-abi-bridge-flags--proto-contracts)
9. [Agent Tooling, Daemon Integration & Security / Confinement](#9-agent-tooling-daemon-integration--security--confinement)
10. [File-by-File Implementation Map](#10-file-by-file-implementation-map)
11. [Phased Delivery Plan, Verification Matrix & Risk Register](#11-phased-delivery-plan-verification-matrix--risk-register)

---

## 1. Product Vision, Goals & Hard Invariants

### 1.1 Why One Expandable Right-Side Pane Serves Both Metaverse Viewer and GameBuilder

OpenAgents Desktop already established an ergonomic, Antigravity-style expandable right-side pane for source files and previews (`apps/openagents-desktop/page/filepane.coax`, #13992): a split pane on the right edge of the conversation column with a unified multi-tab strip, a `+` quick-open button, an **Expand / Restore (`⛶`)** button that takes over the entire main region beside the left sidebar (`window_w - left`), a **Hide (`◫`)** button that collapses the pane while preserving open tabs and state, and a compact Row 2 breadcrumb and action bar.

This specification defines its 3D counterpart: the **Metaverse Viewer & GameBuilder Pane** (`page/metaversepane.coax`). Whenever a user or an agent opens, references, generates, or edits any 3D model, 3D scene, realm, or GameBuilder project, it opens directly inside this right-side pane—just like opening a code file opens the File Viewer—and expands (`⛶`) into a full-canvas 3D inspector or full **GameBuilder** creation and play studio without ever leaving OpenAgents Desktop or spawning a separate application.

Combining both surfaces into **one right-side pane with mode-aware tabs** fulfills three converging architectural decisions:

1. **One Desktop App (2026-11-20 consolidation):** Per [../audits/2026-11-20-gamebuilder-monorepo-migration-audit.md](../audits/2026-11-20-gamebuilder-monorepo-migration-audit.md), standalone `GameBuilder.app` (`gamebuilder/host/`), `Everglade.app`, and `MetaverseEngine.app` were retired. GameBuilder's ~56,429 LOC Coax domain core (`gamebuilder/core/`, `shared/`, `cli/`, `web/`, `worlds/`) lives directly in the monorepo, and **OpenAgents Desktop IS its native macOS/Linux/Windows host**.
2. **One 3D Viewport Lifecycle:** Inspecting a `.glb` asset, exploring a `.ple-scene.json` facility, following a live `openagents.com/realms/...` world, building a level in GameBuilder, and play-testing a Scrappers Arena or Whispering Spring world all share the exact same camera controls, GPU memory/thermal budget, overlay hole-cutting (`cd_over_rects`), multi-tab strip, Expand/Restore geometry, and chat round-trip mechanics.
3. **Zero Friction Between Chat and 3D Work:** In split mode (`520pt` default, draggable `360–1080pt`), the user chats on the left while watching agents build, inspect, or test a live 3D world on the right—clicking 3D entities to reference them in the composer and seeing live edits land in real time. One click on **`⛶` Expand** turns the pane into a widescreen 3D studio / playable game viewport beside the left chat sidebar; one click on **`⛶` Restore** (or `Esc`) snaps back to split chat+3D.

### 1.2 Goals

1. **Instant 3D auto-open:** Any 3D model (`.glb`, `.gltf`, `.obj`, `.stl`, `.usdz`, `.gbm`), engine pack (`.pack`/VPK1–3, `RBA1`), scene (`.ple-scene.json`, `.ifc`, `anim.scene.v1`, `.aimport`, `.splat`/`.ply`), realm URL, or GameBuilder file (`.world`, `.game`) opened from chat, attachments, tool output (`OA-SHOW` / `TK_WORLD`), File Explorer, Finder, drag-and-drop, CLI, or `openagents://` routes automatically to the right-side Metaverse Viewer pane.
2. **Identical File-Viewer ergonomics:** Two-row Coax chrome (`38pt` tab strip + `30pt` breadcrumb/mode bar), multi-tab switching across mixed 3D models/scenes/games, `+` quick-open / New Game, **`⛶` Expand/Restore** to full main area beside sidebar, **`◫` Hide** preserving open tabs, camera state, and running GameBuilder sessions, and smooth `app_presence` spring transitions.
3. **Full GameBuilder Studio inside the pane:** Complete native UI for all 12 `GT_*` game types, 33 `GK_*` entity kinds, `Select/Move/Rotate/Scale` (`Q/W/E/R`) gizmos, 128-step Undo/Redo, `panel.coax` Palette/Inspector/Publish/Owners, `picker.coax` New Game card grid, live 60/120 Hz **Play** mode (Scrappers 4 match modes + all 9 web worlds + SPLICEBOTS + standalone worlds), deterministic `play-test`, `agents.coax` multi-agent panel (`claims.coax`, `changes.coax`, `agent-replay.coax`), and Nostr `lobby.coax`/`party.coax`—with **every single mutation** flowing through `gamebuilder/core/commands.coax`.
4. **Live bidirectional Chat ↔ 3D round-trip:** Bundled `gamebuilder` CLI and auto-registered `gamebuilder mcp --attach` connect agent sessions (Microcoder, Claude Code, Inkling, external MCP) straight into the live open desktop pane over an owner-only Unix socket; typed `TK_WORLD` transcript cards show before/after JPEGs; clicking entities or points in the 3D viewport injects structured selection context (`world.turn.v1`) into the chat composer.
5. **Unified Metaverse Engine rendering:** Retire ad-hoc Swift Metal viewports (`PleSceneMetalView`) onto `metaverse-engine/`, fix the single-instance global-context blockers (`phCurrent`, `activeRealm`, `static RealmHost H`), adopt `RenderRegistry` budgets, and design for macOS Metal, Linux Wayland/Vulkan, and Windows Win32/Vulkan parity.
6. **Zero regression to Inkling-only v1.0 release:** Every new capability is strictly feature-flagged (`DC_FLAG_METAVERSE`, `DC_FLAG_GAMEBUILDER`, `DC_FLAG_SCENES`) and stubbed out of `--minimal` builds (`page/v1-stubs.coax`).

### 1.3 Non-Goals

- Shipping a standalone `GameBuilder.app`, `Everglade.app`, or `MetaverseEngine.app`.
- Building a full DCC sculpting/UV/rigging/material-node suite (Blender remains the offline DCC; GameBuilder + Metaverse Viewer cover inspection, assembly, placement, terrain, tuning, play, and agent authoring).
- Introducing third-party shader compilers (`glslang`, `naga`, `tint`, `Dawn`, `wgpu`) or third-party C++/Swift 3D game engines (`SceneKit`, `RealityKit`, `Filament`, `Three.js`). All production rendering uses owned Coax shaders compiled through `coax metal`, `coax spirv`, and `coax wgsl`.
- Running untrusted remote realms inside the host app's Metal/Vulkan device without process or cross-origin sandbox isolation.
- Supporting tvOS/visionOS or mobile iOS multi-view in this phase (iOS single-pane reuse is noted as a downstream consumer of the same `aethyr-host-v2` work).

### 1.4 Hard Architectural Invariants

Every implementation PR derived from this spec MUST preserve these non-negotiable invariants:

| # | Invariant | Source of Truth | Enforcement |
|---|---|---|---|
| **I-1** | **One Command Table (`GB_COMMANDS`):** Every GameBuilder feature is a row in `gamebuilder/core/commands.coax` (currently 120 verbs). Desktop toolbar buttons, menus, shortcuts, drawer controls (`GB_CONTROLS`), CLI, MCP, HTTP, and Agents pane all dispatch through `ed_command` / `gb_run`. Zero desktop-only state mutations outside the table. | `gamebuilder/INVARIANTS.md` | `gamebuilder/tools/commands-check.coax`, `commands-golden.coax`, `docs-rows.coax` |
| **I-2** | **Coax Owns Logic; Swift/ObjC/C Owns Effects Only:** Coax owns layout, tab management, chrome, routing policy, camera math, picking, undo/redo, editing, gameplay, play-testing, sync, publish, claims, wire codecs, and feature-flag `#authority` rules. Swift/ObjC/C only executes Metal/Vulkan presentation, input forwarding, sockets, file I/O, audio output, and OS dialogs. | `apps/INVARIANTS.md`, `gamebuilder/INVARIANTS.md` | Firm-reason file headers + `coax check` |
| **I-3** | **Acyclic Bounded Contexts (`contexts.txt`):** `components/` NEVER loads `metaverse-engine/` or app modules. `apps/openagents-desktop/page/` (in `coder`) NEVER directly `#load`s `gamebuilder/` (which loads `coder`). GameBuilder core and Metaverse viewer engines compile as separate relocatable `.o` libraries (`gamebuilder-core.o`, `metaverse-viewer.o`) behind narrow C headers—matching `rh-flythrough.o` and `pay-wallet.o` in `build.sh`. | `contexts.txt`, `components/rendering.md` | `coax/web/tool.sh layer-check` |
| **I-4** | **Revision Truthfulness:** The UI MUST display the exact revision currently rendered in the viewport and NEVER conflate chat completion, HTTP `202` proposal, saved world revision (`AuthoringEffect`), and rendered preview (`PreviewFrame.rendered_target`). `"Saved at rev N · preview unavailable"` is always explicit. | `docs/protocol/environments-claude-and-world-editing.md` §8.3 | `#test` state-machine assertions |
| **I-5** | **Minimal v1.0 Release Isolation:** New flags default to `false` in `coax/lib/agent/desktop/flags.coax`. `--minimal` builds (`app-v1.coax` + `page/v1-stubs.coax`) MUST omit all Metaverse/GameBuilder code, Swift files, shaders, and bundled `gamebuilder` resources. | `docs/desktop/openagents-v1-spec.md` | `build.sh --minimal` smoke check |
| **I-6** | **Bounded Assets & Zero Git Bloat:** No binary asset `> 512 KiB` in git. GameBuilder packs (`scrappers.gbm`, `village.pack`, `splice-bots-jam*.pack`) hydrate via `assets/registry/collections/gamebuilder.lock.json` (`assets-sync --group gamebuilder`). Every runtime 3D loader enforces strict byte/node/vertex/index caps and rejects external network URIs. | `gamebuilder/INVARIANTS.md`, `coax/web/prepush.coax` | `prepush` + `import-container.coax` `#test` |
| **I-7** | **Single Shared GPU & Thermal Budget:** Right-pane 3D viewports and inline `OA-SHOW-LIVE` cards share one aggregate host VRAM/frame budget (`RenderRegistry`). Hidden, occluded, scrolled-offscreen, or backgrounded views stop rendering immediately (`0 Hz`). Previews are muted unless explicitly playing/unmuted. | `components/rendering.md`, `apps/INVARIANTS.md` | `RenderRegistry` admission + telemetry |
| **I-8** | **Human Supremacy & Confinement:** `claims.coax` agent region locks NEVER block the human user. Local attach sockets are `0600` in a `0700` directory with `getpeereid`/`SO_PEERCRED` peer UID verification. Concurrent agent runs over the same target obey `#authority` `ar_bindings_conflict` leases. | `gamebuilder/core/claims.coax`, `agents-catalog.coax` | `#authority` mutation tests |
| **I-9** | **Proto-First Contracts:** Every new cross-boundary IPC, wire, or persisted schema gets a versioned JSON Schema under `proto/`, a `proto/registry.json` row, and one Coax codec. | `docs/INVARIANTS-CROSS-CUTTING.md` | `proto` registry validation |

---

## 2. Current Codebase Baseline & Concrete Gaps

### 2.1 Desktop Right-Side Pane Infrastructure (`apps/openagents-desktop/`)

The desktop shell already provides almost every building block needed for an expandable right-side pane:

- **Expandable File Viewer (`page/filepane.coax`):** Implements the exact two-row Antigravity chrome (`CP_FP_BAR1_H = 38pt` tab strip with `+` quick-open `CP_FP_ADD_TAB`, `⛶` Expand/Restore `CP_FP_EXPAND`, `◫` Hide-preserving-tabs `CP_FP_HIDE`; `CP_FP_BAR2_H = 30pt` clickable breadcrumbs + 24pt inline buttons), 4 MB virtualized viewer, structural code folding, Markdown preview, collapsible File Explorer (`CP_FT_TREE_W = 196pt`), and monochrome-pass RGB exemption (`fp_color_u0..u1`, `fp_color_o0..o1` in `page/host-api.coax:77-120`).
- **Existing 3D Hook in File Pane:** `CP_FP_SCENE :: 4` already exists in `filepane.coax` (gated on `DC_FLAG_SCENES :: 26`), reporting `coder_page_file_scene_rect` → `cd_file_scene_rect` so `Window+Files.swift:syncScenePanelFrame()` mounts `PleScenePanelView` over the body rect (converting Coax top-left logical points to AppKit bottom-left points).
- **Shell Layout & Priority (`page/layout/layout.coax`, 111 LOC):**
  - `cp_side_pane` resolves right-pane priority top-to-bottom: `forge` → `side_chat` → `preview` → `term` → `sim` → `filepane` → `review` → `todos`/`pane` (sub-agent transcript).
  - `app_shell_layout` manages split width (`CP_PANE = 520`, min `CP_PANE_LEAST = 360`, max `CP_PANE_MOST = 1080`, min conversation `CP_PANE_MAIN = 456`) with keyed `app_presence` spring slide (`coder.shell.pane`) and narrow-window `APP_PANE_FULL`.
  - User-expanded override (`layout.coax:87-94`): when `cp_fp_shown(s) && s[0].fp[0].expanded`, the right pane takes `pane_x = left`, `pane_w = window_w - left`, `col_w = 0`, drawing the sidebar beside it and disabling the drag-resize handle (`input.coax:345`).
- **Live Inline 3D Card & Overlay Cutouts (`page/shows.coax`, `page/overlay.coax`, `Window+Files.swift:485-582`):**
  - Tool outputs emit `OA-SHOW:`, `OA-SHOW-CAPTION:`, `OA-SHOW-OPEN:`, `OA-SHOW-LIVE: on`.
  - `coder_page_live_scene` reports `[card_x, card_y, card_w, card_h, clip_x, clip_y, clip_w, clip_h]` + path.
  - `coder_page_over_rects` exports up to 8 merged overlay rectangles (menus, popovers, dialogs, command palette, toasts); `Window+Files.swift:pageOverRects(in:)` feeds them to `PleScenePanelView.setHoles(...)`, which masks a `CAShapeLayer` with even-odd fill rule and returns `nil` in `hitTest(_:)` so Coax popovers float cleanly above native Metal layers (#13817).
- **Agents Page Placeholder (`AgentsPage.swift`, `coax/lib/agent/desktop/agents-catalog.coax`):**
  - Already defines a built-in `"game-builder"` `AgentDefinition` (`kind = "creator"`, `max_concurrency = 1`), **but** today it is wired to `runtime = "coax-metal:SceneView"`, `input_contract = "ple.scene.edit.v1"`, and `AgentsPage.swift:launchGameBuilder` simply opens `fixtures/ple-data-hall-sample.ple-scene.json` in `PleScenePanelView`—not `gamebuilder/core/`.
- **Single-Instance Control & Deep Links (`Activation.swift`, `Info.plist`):**
  - Owner-verified `0600` Unix socket at `~/.openagents/desktop/activate.sock` (`getpeereid` UID check) and `openagents://` URL scheme (`open`, `chat`, `settings`, `new`). `Info.plist` currently registers only `.coax` and generic source/plain-text UTTypes.

### 2.2 GameBuilder Package Inventory (`gamebuilder/`)

GameBuilder (~56,429 LOC Coax) is complete at the domain, CLI, MCP, HTTP, web, and worlds layers, with only its native desktop shell removed during consolidation:

| Area | Files / LOC | What Already Exists |
|---|---:|---|
| `gamebuilder/core/` | 49 `.coax` files | Full `Editor` (`editor.coax`: 128-step undo/redo, Q/W/E/R gizmos, orbit/pan/dolly/fly camera), **120-verb `GB_COMMANDS`** (`commands.coax`), **73-control `GB_CONTROLS`** (`controls.coax`), `level.coax` (`world.doc.v1` + `aethyr.content.v1`), `games.coax` (`Scrappers Arena`, `SPLICEBOTS`, owners), `palette.coax` (11 `FIELDS`), `panel.coax` (GPU Component Kit palette/inspector/publish), `picker.coax` (12-type New Game cards), `world.coax` (bridge to `web/hud.coax` for 9 web game types), `play.coax` & `modes.coax` (Scrappers 4 modes, 3 classes, 10 fighters), `playtest.coax` (headless match & NavGrid walk), `farm.coax`/`model.coax`/`rave.coax`/`scene.coax`/`her.coax` (SPLICEBOTS 5 routes + 65-joint chars), `meshes.coax` (`.gbm`), `agent.coax`/`agents.coax`/`agent-timeline.coax`/`agent-replay.coax`/`claims.coax`/`changes.coax`/`pictures.coax`/`decide.coax`/`code-agent.coax`, `mcp.coax`, `api.coax`, `attach.coax`, `publish.coax`, `sync.coax`, `lobby.coax`, `party.coax`, `net.coax`, `triage.coax`/`engine-*.coax`. |
| `gamebuilder/shared/` | 5 `.coax` files | `kinds.coax` (`GK_KINDS = 33`), `buildings.coax` (5 0 A.D. village buildings), `game-types.coax` (`GT_TYPES = 12`, `GT_LEVEL_THINGS = 512`, `GT_LEVEL_SHAPES = 64`, `GT_LEVEL_FILE = 262144`), `loop.coax`, `plays.coax`. |
| `gamebuilder/cli/` | 9 `.coax` files | Compiles standalone native binary `gamebuilder/build/cli/gamebuilder` (built automatically by `apps/openagents-cli/build.sh`, not yet bundled by `apps/openagents-desktop/build.sh`). Currently the **only** native binary compiling `gamebuilder/core/`. |
| `gamebuilder/web/` | 31 `.coax` files | Full WebGPU interactive builder (`hud.coax` 5,963 LOC, `page.coax` 3,197 LOC, `world-{village,crates,platformer,racing,drift}.coax`, `gpu/hud.coax` stylized HDR+grass+bloom shaders) + static showcase site. |
| `gamebuilder/worlds/` | 5 worlds | Playable `fated-dragon` (`dragon.gbd1`), `splicebots`, `whispering-spring` (`grove.wsg`, 5 `.glb` models, WebAudio, multiplayer room), `scrappers-arena` (`scrappers.coax` + `gpu/scrappers-arena.coax`), and `ice-cream-man` (`icecream-face.c`). |
| `gamebuilder/host/` | **Deleted** | Intentionally removed on 2026-11-20; replaced by OpenAgents Desktop's pane specified in this document. |

### 2.3 Metaverse Engine & 3D Rendering Paths

Four rendering paths exist today across the monorepo:

1. **WebGPU Browser Host (`coax/web/host.js`, `render.js`, `realm-frame.js`):** Reference host with full `coaxial_frame_plan_v1`, `RenderRegistry`, `host-pcm.coax` audio, and sandboxed cross-origin `realm-frame.v1` iframes.
2. **Apple Metal `AEPageHost` (`metaverse-engine/host/apple/PageHost.m`, 1,481 LOC) & `MFRealm` (`metaverse-engine/host/metal/MetalRealm.m`, 602 LOC):**
   - `AEPageHost` renders offscreen to private BGRA8 `MTLTexture` (`screen`) and composites via `-present:format:`, runs `microduck.coax` (`RBA1` + `view-menu-v1`) and `web/hud.coax` natively. **Blocked for multi-view** by `static __weak AEPageHost *phCurrent` (line 135), `static EngineCorePack pack` (line 650), and missing `RenderRegistry` + water/character/cull/sort passes.
   - `MFRealm` uses `RenderRegistry` + `HostLifecycle` + `HostTransport` + device-loss recovery over `CAMetalLayer`, **also blocked for multi-view** by `static __weak MFRealm *activeRealm` (line 77; `mf_open` refuses >1 realm).
3. **Linux Vulkan Realm Host (`metaverse/gpu/vk-realm.c`, 1,120 LOC):** Runs all 16 `realms.json` realms natively on Wayland/KMS/headless with `RenderRegistry` and `coax spirv`, but holds a process-global `static RealmHost H` (line 116). Windows (`apps/shared/host/windows/`) currently has Vulkan 2D component UI (`ui-host.c`, `vk-load.c`) with no 3D realm host yet.
4. **Ad-Hoc Desktop Metal Views (`apps/openagents-desktop/SceneView.swift`, `RhFlythroughView.swift`):** Inline MSL shaders on separate `CAMetalLayer`s outside `metaverse-engine/`. `SceneModel.swift` also shells out to `tools/ple-ifc-to-scene.py` instead of calling the native Coax parser `metaverse-engine/import/ple-ifc.coax`.

### 2.4 Concrete Gaps to Close

| # | Gap | Impact | Closed In |
|---|---|---|---|
| **G-1** | No dedicated expandable 3D right-side pane (`page/metaversepane.coax`); `CP_FP_SCENE` in `filepane.coax` only handles `.ple-scene.json`/`.ifc` without 3D controls, tabs, or GameBuilder UI | 3D files fail or open as binary/JSON text; GameBuilder has no desktop UI | Sections 3, 4, 8 (Phase 1) |
| **G-2** | No generic standalone 3D model viewer (`metaverse-engine/content/model-view.coax` does not exist; `pi_glb` & `gltf-scene.coax` drop PBR textures; `.obj`/`.stl`/`.usdz` have no runtime loader) | Dropping or clicking a textured `.glb`/`.gltf`/`.obj`/`.stl`/`.usdz` model cannot inspect it natively | Section 5.1 (Phase 1) |
| **G-3** | `apps/openagents-desktop/` does not compile, link, or bundle `gamebuilder/core/` or `gamebuilder/build/cli/gamebuilder`; `AgentsPage.swift` opens a PLE fixture instead | GameBuilder is unreachable from Desktop except via manual terminal CLI | Sections 6, 8, 9 (Phases 0, 3) |
| **G-4** | `PageHost.m` (`phCurrent`), `MetalRealm.m` (`activeRealm`), `vk-realm.c` (`static H`), and `aethyr-host-v1` C imports carry no host context handle | Cannot safely host a right-pane 3D viewport + inline `OA-SHOW-LIVE` card + multi-tab cached scenes in one process | Section 7.2 (Phase 2) |
| **G-5** | `composition-v1` (`UcPlan`) cannot borrow external Metal/Vulkan scene textures | Requires `ui_child` native subviews + `cd_over_rects` `CAShapeLayer` holes until `composition-v2` lands | Section 7.4 (Phase 1–3 bridge, Phase 4 native) |
| **G-6** | `OA-SHOW-LIVE` & `cp_show_scene` hardcode `.ple-scene.json`; no `TK_WORLD` typed transcript cards for `gamebuilder` | Agents building 3D models or GameBuilder levels only emit raw JSON/text tool rows | Sections 4.2, 6.6 (Phases 0, 1) |
| **G-7** | `contexts.txt` would cycle if `apps/openagents-desktop/` (in `coder`) directly `#load`ed `gamebuilder/` (which loads `coder`) | Breaks `layer-check` unless separated at the relocatable `.o` link boundary | Section 8.1 (Phase 0) |
| **G-8** | Windows desktop has Vulkan 2D UI (`apps/shared/host/windows/`) but no 3D Vulkan realm/model host | Windows parity requires reusing `metaverse/gpu/` over Win32 Vulkan | Section 7.6 (Phase 4) |

---

## 3. Unified Right-Side Pane UX, Chrome & Shell Integration

### 3.1 Responsive Shell States (Mirroring `page/filepane.coax` #13992)

The Metaverse Viewer & GameBuilder pane participates in the exact same 3-region responsive shell (`apps/shared/app/view/shell.coax` + `page/layout/layout.coax`) as the File Viewer:

#### State A: Split Mode (Default — `pw = 520pt`, draggable `360..1080pt`)
```
┌──────────────┬────────────────────────────────┬──────────────────────────────────────────────────┐
│ Left Sidebar │  Conversation Column (≥456pt)  │ [GLB duck.glb ×][GB Arena ×] [+]      [⛶] [◫]   │ ← Row 1 (38pt)
│ (272pt,      │                                ├──────────────────────────────────────────────────┤
│ collapsible) │  User: "Add a jump pad by the  │ Scrappers › Level 1  [Inspect|Build|Play] rev 14 │ ← Row 2 (30pt)
│              │  east ramp and test it"        ├──────────────────────────────────────────────────┤
│  Chats       │                                │ [↶][↷] [Q][W][E][R] [Snap 50cm] [+Add] [F] [⋮]  │ ← Sub-bar (Build only, 30pt)
│  Pinned      │  ┌──────────────────────────┐  ├──────────────────────────────────────────────────┤
│  Recent      │  │ TK_WORLD: Added pad #14  │  │                                                  │
│              │  │ [Before JPG] [After JPG] │  │                                                  │
│              │  │ [Show me]  [Undo this]   │  │              Live 3D Metal / Vulkan              │
│              │  └──────────────────────────┘  │                     Viewport                     │
│              │                                │         (Orbit / Fly / Gizmos / Live Play)       │
│              │  ┌──────────────────────────┐  │                                                  │
│              │  │ + @  Ask or steer…   [↑] │  │  [◧ Inspector]              [FPS · 12.4k tris]  │
│              │  └──────────────────────────┘  │                                                  │
└──────────────┴────────────────────────────────┴──────────────────────────────────────────────────┘
                                                ▲ Draggable split divider (cp_edge == 2, 4pt hit)
```

#### State B: Expanded Mode (`⛶` — `expanded == true`, Full Main Area Beside Sidebar)
Clicking **`⛶`** (or pressing `⇧⌘E` / `F11` when focused) sets `s[0].mv[0].expanded = true`. Exactly like `cp_fp_shown(s) && s[0].fp[0].expanded` in `layout.coax:87-94`, the pane takes `pane_x = left`, `pane_w = window_w - left`, `col_w = 0`. The left chat sidebar stays accessible (or can be collapsed via `⌘B` for a true edge-to-edge canvas), and the drag-resize edge is disabled.

```
┌──────────────┬───────────────────────────────────────────────────────────────────────────────────┐
│ Left Sidebar │ [GB Scrappers Arena ×][GLB mech.glb ×][PLE hall.ple-scene.json ×] [+]   [◧] [⛶][◫]│ ← Row 1 (38pt)
│ (272pt,      ├───────────────────────────────────────────────────────────────────────────────────┤
│ ⌘B toggle)   │ Scrappers Arena › Arena 1 (rev 14 · saved)  [Inspect|Build|Play] [▶ Test] [Share] │ ← Row 2 (30pt)
│              ├────────────────────────────────────────────────────────────────┬──────────────────┤
│              │ [↶][↷] │ [Q] [W] [E] [R] │ [Snap 50cm] [F] [Del] [Decide]      │ [Build][Levels]  │ ← Studio Drawer
│              ├────────────────────────────────────────────────────────────────┤ [Agents][Share]  │   (280pt, toggle)
│              │                                                                │                  │
│              │                                                                │  ADD PALETTE     │
│              │                  Widescreen 120 Hz Metal / Vulkan              │  [Block] [Start] │
│              │                   3D Studio / Playable Viewport                │  [Rival] [Pad]   │
│              │                                                                │  [Goal]  [Ball]  │
│              │                                                                │                  │
│              │                                                                │  INSPECTOR       │
│              │                                                                │  Kind:   pad     │
│              │                                                                │  East:  - 450 +  │
│              │                                                                │  Up:    -   0 +  │
│              │                                                                │  North: - 120 +  │
│              │  ┌──────────────────────────────────────────────────────────┐  │  Turn:  -  90 +  │
│              │  │ [1 Block][2 Pad][3 Goal][4 Tree][5 Water][6 Pack] [Snap] │  │  Scale: - 100 +  │ ← Optional Hotbar
│              │  └──────────────────────────────────────────────────────────┘  │  W/H/D: 200×40…  │   (Build expanded)
└──────────────┴────────────────────────────────────────────────────────────────┴──────────────────┘
```

#### State C: Narrow Window Mode (`APP_PANE_FULL`)
When `window_w < CP_PANE_LEAST + CP_PANE_MAIN` (`816pt` at `1.0×` zoom), `app_shell_layout` returns `APP_PANE_FULL`. The pane fills `[0, window_w]` with Row 1 pushed below `cp_bar(s)` (`38pt`) to clear macOS traffic lights and the left titlebar island (`titlebar.coax`). Clicking **`◫` Hide** returns immediately to the full-width conversation without losing any open 3D tabs or running GameBuilder state.

### 3.2 Row 1 (`CP_MV_BAR1_H = 38.0pt`): Unified Multi-Tab Strip

Row 1 aligns flush with the window title bar (`top = px > 0 ? 0.0 : cp_bar(s)`), mirroring `cp_fp_draw_bar1` (`filepane.coax:483-547`):

- **Left-to-right controls:**
  1. **Tab pills** (`max CP_MV_TABS = 12` open tabs; pills shrink gracefully with `app_fit`, minimum `56pt`, horizontal overflow chevron menu when exceeded). Each tab pill contains:
     - **Colored 16×16 RGB type badge** (rendered inside the monochrome-exempt primitive range so badges remain crisp color even under the Noir monochrome desktop skin):
       - `GLB` / `3D` (teal `#2DD4BF`): `.glb`, `.gltf`, `.obj`, `.stl`, `.usdz`, `.gbm`
       - `ASM` (cyan `#38BDF8`): `RBA1` rigid assemblies / Microduck
       - `VPK` (indigo `#818CF8`): `.pack`, VPK1/2/3, `.aimport`
       - `PLE` (amber `#F59E0B`): `.ple-scene.json`, `.ifc`
       - `SPL` (violet `#A78BFA`): `.splat`, `.ply` Gaussian splats
       - `RLM` (emerald `#34D399`): live Metaverse realms (`openagents.com/realms/...`)
       - `GB` (coral/orange `#FB7185` or brand accent `#F97316`): GameBuilder game/level tabs
     - **Short tab title** (filename or Game/Level name; italic when preview/ephemeral tab).
     - **Status dot:** unsaved dot (`●`) for dirty GameBuilder levels, pulsing live dot (`●` green) when an agent is actively editing or a live realm is streaming, or amber dot when revision is stale/unavailable.
     - **Close button (`×`)** on active or hovered tab (prompts save / confirms stopping active Play session if dirty).
  2. **`+` Quick-Open / New button (`24pt`, `CP_MV_ADD_TAB`):** Opens a compact popover menu with:
     - *Open 3D File…* (triggers `NSOpenPanel` / platform picker filtered to supported 3D formats)
     - *New GameBuilder Game…* (opens the `picker.coax` 12-template card grid in a tab)
     - *Open GameBuilder Game* (submenu listing local + synced games: `Scrappers Arena`, `SPLICEBOTS`, user games)
     - *Open Realm URL…* (connects `world-follow.coax` / realm viewer)
     - *Recent 3D Scenes* (MRU list of up to 8 paths/games)
- **Right-aligned controls (fixed right-to-left, 24pt icon wells, 4pt gap):**
  - **`◫` Hide (`CP_MV_HIDE`):** Collapses the pane (`mv.hidden = true`) while preserving all open tabs, cameras, undo stacks, and GameBuilder sessions. Pauses GPU rendering immediately (0 Hz). Re-opening via the corner icon, `⌥⌘M`, or any 3D trigger restores the exact active tab.
  - **`⛶` Expand / Restore (`CP_MV_EXPAND`):** Toggles `mv.expanded` between split right pane and full main area beside the left sidebar (`window_w - left`). Tooltip switches `"Expand 3D viewer (⇧⌘E)"` / `"Restore split view (⇧⌘E)"`.
  - **`◧` Studio / Inspector Drawer toggle (`CP_MV_DRAWER`):** Toggles the collapsible side drawer (Scene Hierarchy & Properties in Viewer mode; Build Palette / Inspector / Levels / Agents / Publish / Lobby in GameBuilder mode).

### 3.3 Row 2 (`CP_MV_BAR2_H = 30.0pt`): Context, Breadcrumb & Mode Bar

Row 2 mirrors `cp_fp_draw_bar2` (`filepane.coax:550-655`) and adapts cleanly to the active tab's kind:

- **Viewer Tab Row 2:**
  - **Left:** Segmented clickable breadcrumbs (`… > models > [GLB] quail.glb`; clicking the filename copies path; clicking a parent folder opens File Explorer at that folder) + **Revision/Format pill** (e.g. `glTF 2.0 · 14.2k tris` or `rev 27 · live` or `rev 27 · preview unavailable`).
  - **Right (24pt compact controls):**
    - Camera mode toggle: `Orbit | Fly` (`CP_MV_CAM_MODE`)
    - Camera Preset menu (`Iso`, `Front`, `Top`, `Side`, `Under`, `Focus F`) (`CP_MV_CAM_MENU`)
    - Shading / View Menu (`metaverse.engine.view-menu.v1`: `Lit`, `Normals`, `Wireframe`, `Grid`, `Bounds`, `Z-Slice`, `Diff`, `Stats`) (`CP_MV_VIEW_MENU`)
    - **Camera Snapshot → Chat (`CP_MV_SNAPSHOT`):** Captures PNG from the active viewport + structured camera/selection metadata and attaches directly to the chat composer (`cd_attach`).
    - `↻` Reload (`CP_MV_RELOAD`) & `⋮` More (`Copy Path`, `Reveal in Finder`, `Open in External App`, `Export PNG…`, `Open as GameBuilder Asset…`).
- **GameBuilder Tab Row 2:**
  - **Left:** Clickable `Game › Level` breadcrumb (opens quick game/level switcher popover) + **Mode Segmented Pill**:
    - **`Inspect`** (read-only orbit/fly + claims/changes review + playtest path overlay)
    - **`Build`** (active Q/W/E/R gizmos, snapping, Palette & Inspector drawer, `world.coax` held still)
    - **`Play`** (live 60/120 Hz simulation/gameplay via `play.coax` or `world.coax`, mouse/keyboard routed to game loop, audio unmuted if enabled)
  - **Center-Right:** Revision & Save pill (`rev 14 · saved` / `unsaved` / `Agent editing…` with one-click `Stop`) — strictly obeying Invariant I-4.
  - **Right (24pt compact controls):**
    - `▶ Test` (`CP_GB_PLAYTEST`): runs deterministic headless `play-test` (`GT_TEST_MATCH` or `GT_TEST_WALK`) and overlays path + stuck-bot pins.
    - `Share` (`CP_GB_PUBLISH_TAB`): jumps to the Publish/Share drawer tab (shows active 6-char share code pill if already published).
    - `Agents` (`CP_GB_AGENTS_TAB`): toggles the Agents sub-tab with unread/running badge count.
    - `⋮` More (`Duplicate Level`, `Make My Own Copy`, `Copy Attach Socket Command`, `Open Level File in File Viewer`, `Export Snapshot to Chat`).

### 3.4 Collapsible Studio / Inspector Drawer

- **Width:** `CP_MV_DRAWER_W_SPLIT = 240pt` (only auto-expanded in split mode when `pw >= 580pt`; overlay-docked or collapsed by default at `520pt` so the 3D viewport always keeps at least `320pt` width) and `CP_MV_DRAWER_W_EXPANDED = 280pt` in expanded mode (`⛶`), where it opens automatically for GameBuilder tabs.
- **Position:** Docked on the **right edge** inside the pane (keeping the 3D viewport flush with the conversation column on the left so eye travel from chat → 3D scene is minimal).
- **Rendered natively in Coax GPU UI** (`components/kit/` + `page/metaversepane.coax`), never SwiftUI or AppKit `NSStackView`:
  - **Viewer Mode tabs inside drawer:** `Tree` (scene nodes / rigid-assembly parts / PLE layers / skeleton bones / animation clips) and `Inspect` (selected part transform, dimensions, material/shader, mesh stats, asset SHA-256 & licence provenance, slice/explode sliders).
  - **GameBuilder Mode tabs inside drawer:** `Build` (`panel.coax` Add Palette + 11-field Inspector), `Levels` (Games/Levels/New Game/Owners), `Agents` (`agents.coax` Builder/Playtester/Code agents + Claims + Changes + Replay scrubber), `Share` (`publish.coax` versions/rollback/feedback/moderation), and `Lobby` (`lobby.coax`/`party.coax` Nostr global chat & 4-player party).
- **Monochrome Exemption:** Generalize `CoderPage.fp_color_u0..u1` / `fp_color_o0..o1` (`page/host-api.coax:77-120`) to a shared pane color-exempt range `pane_color_u0..u1` / `pane_color_o0..o1` (supporting up to 2 ranges if needed, or active right pane sets it once per frame) so tab badges, claim region color swatches, agent state dots, and diff highlights render in full RGB while the rest of the desktop stays monochrome Noir.

### 3.5 Right-Side Pane Priority, Yielding & Resize Rules (`page/layout/layout.coax`)

Insert `cp_mv_shown(s)` directly into `cp_side_pane` alongside `cp_fp_shown(s)` using **explicit front-most ownership** (matching `fp[0].front`, `sim[0].front`, `term[0].front`):

1. **Updated `cp_side_pane` priority (`layout.coax:13-32`):**
   ```
   forge → side_chat → preview → term → sim → metaverse → filepane → review → todos/pane
   ```
   - Whenever **any** pane is explicitly opened or clicked (`mv_front`, `fp_front`, `sim_front`, `term_front`, `preview_front`, `forge_front`), it sets its own `front = true` and yields the other right panes (`front = false`, leaving their tabs and state intact in memory—never destroying tabs when switching from a `.coax` file to a `.glb` model and back!).
   - Switching between the File Viewer and the Metaverse Viewer is instant (1 click on their corner icons, or `⌥⌘F` / `⌥⌘M`, or clicking a `.coax` vs `.glb` link in chat).
2. **Expanded Mode Check (`layout.coax:87-94`):**
   - Extend the user-expanded full-main-area branch to:
     ```coax
     if (cp_fp_shown(s) && s[0].fp[0].expanded) || (cp_mv_shown(s) && s[0].mv[0].expanded) {
       ...
     }
     ```
   - `cp_edge(s, ux)` in `page/input.coax:343-345` disables edge `2` whenever `cp_mv_shown(s) && s[0].mv[0].expanded`, and enables drag-resizing (`360..1080pt`) when docked in split mode.
3. **Corner Bar (`page/layout/conversation.coax:648-690` `cp_corners`):**
   - Add a quiet `24pt` Metaverse / 3D icon button (`coder.mv.toggle`, `ICON_CUBE` / `ICON_WORLD`, action `CP_MV_TOGGLE`) beside `coder.ft.toggle` (`ICON_SIDEBAR_RIGHT`), gated by `cp_flag(s, DC_FLAG_METAVERSE) || cp_flag(s, DC_FLAG_GAMEBUILDER) || cp_flag(s, DC_FLAG_SCENES)`.
   - Shows `EL_S_CURRENT` when shown, and a subtle active indicator dot when hidden with open 3D/GameBuilder tabs or a running Play/Agent session.

### 3.6 Entry Points & Discoverability

| Entry Point | Behavior |
|---|---|
| **Top-right Corner Icon (`coder.mv.toggle`)** | Toggles pane visibility. If no tabs are open yet, opens the Metaverse/GameBuilder Home view (recent 3D files + `picker.coax` 12-type GameBuilder launcher + drop zone). |
| **Chat Click (`CP_PATH`, `CP_CARGO`, `CP_SHOW`, `TK_WORLD`)** | Clicking any 3D file path, attachment, `OA-SHOW` card, or `TK_WORLD` card opens/focuses that tab in the Metaverse pane. |
| **File Explorer (`filetree.coax`)** | Clicking a 3D/GameBuilder file in the File Explorer opens it in the Metaverse pane (while non-3D files continue opening in the File Viewer). Right-click / `⋮` offers *"Open as Text/JSON in File Viewer"* for `.ple-scene.json`, `.gltf`, `.world`, `.game`, `.ifc`. |
| **Command Palette (`⌘K`, `palette.coax`)** | Adds `Show/Hide Metaverse Viewer` (`⌥⌘M`), `Open 3D Scene or Model…`, `Expand/Restore 3D Pane` (`⇧⌘E`), `Open GameBuilder` (`⌥⌘G`), `GameBuilder: New Game…`, `GameBuilder: Play / Stop`, `GameBuilder: Run Play-Test`. |
| **macOS Menu Bar (`Window+Menus.swift`)** | `File > Open 3D Scene or Model…` (replaces PLE-only `Open 3D Scene…`); `View > Metaverse Viewer` (`⌥⌘M`), `View > GameBuilder` (`⌥⌘G`), `View > Expand Right Pane` (`⇧⌘E`); contextual `GameBuilder` menu (Undo/Redo/Duplicate/New Game/New Code Agent `⇧⌘K` driven directly from `GB_CONTROLS` `GB_PLACE_MENU`). |
| **Agents Page (`AgentsPage.swift`)** | Clicking **Launch** on the `"game-builder"` agent card opens the real GameBuilder tab inside the Metaverse pane (replacing the PLE fixture placeholder) and acquires a non-overlapping `TargetBinding` lease. |
| **Drag-and-Drop onto Window** | Dropping any supported 3D file (`.glb`, `.gltf`, `.obj`, `.stl`, `.usdz`, `.pack`, `.gbm`, `.world`, `.game`, `.ple-scene.json`, `.ifc`, `.splat`, `.ply`) onto the right half (or onto the active Metaverse pane) opens it directly in a new tab; dropping onto the composer still attaches it to the chat draft (with a click-to-preview badge). |
| **Finder / OS Open & CLI** | Double-clicking registered 3D/GameBuilder files in Finder, running `openagents --scene PATH` / `openagents --gamebuilder [GAME]`, or opening `openagents://metaverse/...` or `openagents://gamebuilder/...` routes through `Activation.swift` into the running window. |

### 3.7 Keyboard Focus, Shortcuts & Escape Contract

3D navigation (`WASD`, `Q/W/E/R`, `F`, `Space`, `Delete`) MUST NEVER collide with typing in the chat composer or editing inspector numeric fields:

- **Focus Targets (`CP_KB_*`):**
  - `CP_KB_COMPOSER (1)` / `CP_KB_SEARCH (2)` / `CP_KB_FILE (3)` / `CP_KB_TERM (4)` / `CP_KB_FIND (5)` / `CP_KB_FT_FILTER (6)` / `CP_KB_PROV (7)` — unchanged.
  - **`CP_KB_MV (8)`:** Viewport has keyboard focus (acquired on pointer-down inside the 3D viewport rect, or via `⌥⌘M` when opening in Play/Build mode). Receives `WASD`/`Space`/`Shift` (Fly / Play), `Q/W/E/R` (Select/Move/Rotate/Scale), `F` (Focus), `G` (Grid snap toggle), `1..9` (Hotbar / Picker), `Backspace`/`Delete` (Delete selected entity in Build mode), `⌘Z`/`⇧⌘Z`/`⌘C`/`⌘V`/`⌘D` (routed through `ed_command` when `CP_KB_MV` is focused).
  - **`CP_KB_GB_PANEL (9)`:** A text/numeric field inside the GameBuilder drawer (rename game/level, inspector value, agent composer, lobby chat, publish title) has keyboard focus. Standard OS text editing applies; single-letter viewport hotkeys (`Q/W/E/R/WASD`) are suppressed.
- **Explicit Escape (`Esc`) Unwind Stack:**
  1. If a Coax overlay (menu, popover, dialog, command palette) is open → close overlay.
  2. Else if in GameBuilder **Play** mode with pointer lock / active match → exit Play mode (`stop` command) and return to Build mode (releasing pointer lock).
  3. Else if dragging a gizmo or placing a shape → cancel active drag without mutating undo history.
  4. Else if `CP_KB_MV` has an entity selection → clear selection.
  5. Else if `mv.expanded == true` → restore to split mode (`mv.expanded = false`) and return keyboard focus to `CP_KB_COMPOSER`.
  6. Else → return keyboard focus to `CP_KB_COMPOSER`.
- **Never Steal Focus on Auto-Open:** When an agent auto-opens a 3D model or GameBuilder level while the user is typing in the composer, the pane slides open and renders live, **without** taking `CP_KB_COMPOSER` away from the user.

### 3.8 Overlay Cutouts & Native View Z-Order

In Phases 1–3, the 3D Metal/Vulkan surface mounts as a native child view (`ui_child(..., above=1)` on macOS/Linux/Windows) aligned every frame to `cd_mv_rect(page, out[4])` (matching `Window+Files.swift:syncScenePanelFrame()`).

- Every frame after `coder_page_frame`, the host queries `cd_over_rects(page, r, 8)` (which covers dropdown menus, the `+` quick-open popover, `⋮` actions menu, view-menu popovers, Command Palette `⌘K`, settings, dialogs, and bottom toasts) and applies them via `MetaverseViewportView.setHoles(_:)` (even-odd `CAShapeLayer` mask + `hitTest` pass-through returning `nil` inside holes).
- **Important constraint:** Coax-drawn pane chrome (Row 1, Row 2, Build sub-toolbar, Studio Drawer, expanded Hotbar) lives **outside** `cd_mv_rect` (the body rect strictly excludes Row 1, Row 2, active sub-bar, and open Studio Drawer width), so Coax chrome never fights the 3D view for z-order or input. Only transient overlays (menus/popovers/palette/dialogs/toasts) overlap `cd_mv_rect` and use `cd_over_rects` cutouts—until Phase 4 moves 3D scene output directly into `composition-v2` (`UcPlan` external textures, §7.4).

---

## 4. Auto-Opening & 3D Trigger Routing Matrix

### 4.1 Complete Format, Scheme & Source Routing Table

When `CoderWindow.openNamedFile(_:)` (or its Linux/Windows equivalent in `host-main.c`) receives a file path or URI, a **pure Coax classifier** (`coax/lib/agent/desktop/metaverse-route.coax`, `#authority`) determines the target pane, tab kind, and engine adapter—so macOS, Linux, and Windows never diverge on routing rules:

| Trigger / Extension / Scheme | Tab Kind (`CP_MV_KIND_*`) | Badge | Default Mode | Engine Loader / Adapter | Notes & Fallbacks |
|---|---|---|---|---|---|
| `.glb` | `CP_MV_KIND_MODEL` | `GLB` | `Inspect` | `metaverse-engine/content/model-view.coax` (static PBR + morph/skin detection → `gltf-scene.coax`) | Bounded container (`ai_glb`); zero external network URIs. |
| `.gltf` | `CP_MV_KIND_MODEL` | `GLB` | `Inspect` | `model-view.coax` / `gltf-scene.coax` | Resolves relative sibling `.bin`/textures **only inside the file's directory subtree** (path-traversal rejected). |
| `.obj`, `.stl` | `CP_MV_KIND_MODEL` | `3D` | `Inspect` | `model-view.coax` (`mv_load_obj`, `mv_load_stl`) | Phase 1 pure-Coax bounded parsers (`.mtl` optional sibling read within dir). |
| `.usdz`, `.usd`, `.usda`, `.usdc` | `CP_MV_KIND_MODEL` | `USD` | `Inspect` | macOS: Apple `ModelIO` (`MDLAsset`) → bounded vertex/material buffer into `model-view.coax` Metal renderer; Linux/Win: explicit unsupported-format card | Keeps single Metaverse renderer (no `SceneKit` view); strictly sandboxed local decode. |
| `.gbm` (`GBM1`) | `CP_MV_KIND_MODEL` | `GBM` | `Inspect` | `gamebuilder/core/meshes.coax` (`ed_meshes_open`) | Displays mesh selector list (up to 32 meshes) + joint metric gizmos. |
| `.pack`, `.pack.gz` (VPK1/2/3) | `CP_MV_KIND_PACK` | `VPK` | `Inspect` | `pack.coax` / `page-pack.coax`; if `RBA1` catalog present → `assembly.coax` (`microduck`) | Shows pack manifest, entry list, licences, SHA-256, and interactive 3D mesh/rig/assembly preview. |
| `.ple-scene.json` | `CP_MV_KIND_PLE` | `PLE` | `Inspect` | `metaverse-engine/import/ple-scene.coax` → Metaverse Engine instanced adapter | Retires standalone `PleSceneMetalView`; preserves layers, clashes, Z-slice, diff, 4 camera presets. |
| `.ifc` | `CP_MV_KIND_PLE` | `IFC` | `Inspect` | Native Coax `metaverse-engine/import/ple-ifc.coax` (retires `tools/ple-ifc-to-scene.py` subprocess) | Zero Python dependency; instant load with MEP layer tree & clash inspection. |
| `.splat`, `.ply` (3DGS / mesh) | `CP_MV_KIND_SPLAT` or `MODEL` | `SPL` / `3D` | `Inspect` | Header sniff: 3DGS properties (`f_dc_0`, `opacity`, `scale_0`) → Splat renderer; triangle faces → `model-view.coax` | Native Metal/Vulkan splat sorting in Phase 2/4 (bounded ≤ 1,048,576 splats). |
| `.anim-scene.json` (`anim.scene.v1`) | `CP_MV_KIND_SCENE` | `ANM` | `Inspect` | `metaverse-engine/anim/scene-document.coax` | Includes playback timeline scrubber, camera flight selector, `realistic`/`cel` style toggle. |
| `*.aimport/` (directory w/ `manifest.json`) | `CP_MV_KIND_PACK` | `UE` | `Inspect` | `metaverse-engine/import/unreal/` manifest browser + `.glb`/heightmap viewer | Inspects Unreal Engine 5.9 imported level bundles deterministically. |
| `*.world` (`world.doc.v1`), `*.game` (`aethyr.content.v1 game 1`) | `CP_MV_KIND_GAMEBUILDER` | `GB` | `Build` | `gamebuilder/core/level.coax` + `games.coax` + `editor.coax` / `world.coax` | Full GameBuilder studio tab; opens parent game context if `.world` belongs to a `.game`. |
| `gamebuilder/worlds/{fated-dragon,splicebots,whispering-spring,scrappers-arena,ice-cream-man}` | `CP_MV_KIND_GAMEBUILDER` | `GB` | `Play` | Native compiled world face via `AEPageHost` / `vk-realm` | Opens directly in Play/Inspect mode with world controls and audio toggle. |
| `openagents://metaverse?...` / `https://openagents.com/realms/SLUG` | `CP_MV_KIND_REALM` | `RLM` | `Inspect` | `world-follow.coax` + `AuthoringBinding` (trusted) or sandboxed viewer / `PreviewFrame` (untrusted) | Respects trust boundary (§9.4) and revision truthfulness (§5.5). |
| `openagents://gamebuilder?...` / `https://gamebuilder.org/g/CODE` | `CP_MV_KIND_GAMEBUILDER` | `GB` | `Play` (shared) / `Build` (owned) | `gamebuilder/core/publish.coax` (`GET /api/play/CODE`) or local game open | Non-owned shared games open in Play/Inspect with `"Make my own copy"` remix banner (`ed_may_edit`). |

> **Flag Gating Fallback:** If `DC_FLAG_METAVERSE` (and `DC_FLAG_SCENES`) are **off** (e.g., `--minimal` v1.0 release), text-based 3D formats (`.ple-scene.json`, `.gltf`, `.world`, `.game`, `.obj`, `.ifc`) continue opening cleanly in the standard text File Viewer (`page/filepane.coax`), and binary formats (`.glb`, `.pack`, `.gbm`) show the standard File Viewer binary/hex notice with *"Open in External App"*.

### 4.2 Chat, Tool Output & Agent Auto-Open Policy

Four distinct paths from the conversation trigger the Metaverse Viewer & GameBuilder pane:

1. **Generalized `OA-SHOW` Markers (`page/shows.coax`):**
   - Extend `cp_show_scene(path)` (currently hardcoded to `.ple-scene.json` at `shows.coax:120`) to call the shared Coax classifier `mv_route_is_3d(path)` covering every format in §4.1.
   - Add two optional metadata lines (backward-compatible with existing `OA-SHOW:` parsers):
     ```text
     OA-SHOW: /abs/path/preview.jpg
     OA-SHOW-CAPTION: Added east jump pad (rev 14)
     OA-SHOW-OPEN: /abs/path/level-1.world
     OA-SHOW-LIVE: on
     OA-SHOW-KIND: gamebuilder        # optional hint: model | scene | ple | realm | gamebuilder
     OA-SHOW-REV: 14                  # optional authoritative revision shown on card & handoff
     ```
   - Clicking an `OA-SHOW` card whose `OA-SHOW-OPEN` resolves to a 3D format opens (or focuses) that tab in the Metaverse pane instead of `filepane.coax`.
2. **Typed `TK_WORLD` Transcript Cards (GameBuilder & World Edits):**
   - Mirrors `cp_sim_cmd` in `conversation.coax:65-71` (which detects `oasim` tool calls and renders human-readable verb rows that bring the Simulator pane forward on click).
   - A Coax recognizer `cp_world_cmd(text, output)` detects:
     - `gamebuilder ...` CLI commands and `gamebuilder` MCP tool calls (`add`, `apply-edits`, `play-test`, `publish`, `new-game`, `open-level`, `decide`, `picture`, `pictures`).
     - `ple-run ...` commands and `world.edit.v1` / `realm_*` MCP calls.
   - Instead of raw JSON blobs, the activity group renders a compact **World Edit Card** (`TK_WORLD`) showing:
     - Human summary: e.g. `GameBuilder · Added jump pad at (450, 0, 120) · rev 14` or `Play-Test · 10-7 Knockouts (0 stuck bots)`.
     - Side-by-side **Before / After JPEG thumbnails** (when `pictures.coax` emitted picture paths or `gamebuilder.pictures.v1` references) decoded via `cp_image_request`.
     - Inline action chips: **`Open in 3D`** (focuses right pane), **`Show me`** (highlights agent changes via `changes.coax` & aims camera), and **`Undo`** (invokes `undo-turn` via `ed_command`).
3. **Auto-Open Etiquette (Respecting Human Control):**
   - **Rule A (Explicit Human Click):** Clicking any 3D file link (`CP_PATH`), attachment (`CP_CARGO`), File Explorer row, `OA-SHOW` card, `TK_WORLD` card, or dropping a 3D file ALWAYS opens the Metaverse pane (`mv.hidden = false`, `mv.closed_by_person = false`, `mv.front = true`).
   - **Rule B (Agent-Driven Auto-Open):** When a running agent emits a new 3D file artifact (`Write`/`Edit` creating or modifying a `.glb`/`.gltf`/`.obj`/`.stl`/`.ple-scene.json`/`.ifc`/`.world`/`.game`), executes a mutating `gamebuilder` / `ple-run` / `realm_*` command, or outputs `OA-SHOW-LIVE: on` / `TK_WORLD`:
     - If `mv.closed_by_person == false` (the user did not manually hide/close the pane in this chat) AND no higher-priority modal/settings overlay is active, the pane **automatically opens in split mode** (`520pt`) on the produced/edited 3D target and live-reloads on subsequent revisions—mirroring `simpane.coax`'s `closed_by_person` etiquette.
     - If the user clicks `◫` Hide (`mv.closed_by_person = true`), subsequent agent turns update the hidden tab state and light the corner button's activity dot **without** popping the pane back open until the user clicks the corner icon or a card.
     - Auto-open NEVER steals `CP_KB_COMPOSER` keyboard focus and NEVER forces `mv.expanded = true`.
4. **File Modification Live-Reload:**
   - Any open file-backed tab watches its `(mtime, size)` or `FSEvents`/`inotify`/`ReadDirectoryChangesW` (debounced 150 ms) and reloads seamlessly **preserving the user's current camera pose** (unless the user clicks `Focus F`), displaying a brief subtle `"Reloaded"` status pulse in Row 2.

### 4.3 Live Inline Card (`OA-SHOW-LIVE`) ↔ Right-Pane Handoff

When an `OA-SHOW-LIVE: on` card is visible in the conversation transcript **and** the user clicks its `"Open in Pane"` button (or clicks the card):

1. The current inline card's camera pose (`eye`, `at`, `fov`, `layer_mask`, `z_slice`) is copied into the newly opened/focused right-pane tab so there is **zero visual jump**.
2. **GPU Deduplication Rule:** If the exact same `(path, rev)` is simultaneously displayed in the right-side Metaverse pane and visible as an inline `OA-SHOW-LIVE` card in the scrolled transcript, the inline card **automatically pauses its live 3D view and falls back to its static `OA-SHOW` JPEG image** with a subtle pill `"Live in right pane ↗"` (clicking it focuses the right pane). This guarantees we never burn double VRAM/GPU cycles rendering the identical scene twice on screen simultaneously.

### 4.4 `Info.plist` & `Activation.swift` Deep-Link Extensions

- **New `openagents://` Routes (`Activation.swift` + `coax/lib/agent/desktop/activation.coax`):**
  - `openagents://metaverse?path=/abs/path/model.glb&expand=1` — opens a local 3D model/scene in the Metaverse pane (optional `expand=1`, `camera=iso|front|top|side`, `mode=inspect`).
  - `openagents://metaverse/realm/<slug>?lane=dev&rev=14` — opens a realm tab with optional lane and revision.
  - `openagents://gamebuilder` — opens the Metaverse pane to the GameBuilder home / last active game.
  - `openagents://gamebuilder/game/<game_id>?level=<level_id>&mode=build|play&expand=1` — opens a specific local/synced GameBuilder game & level.
  - `openagents://gamebuilder/play/<share_code>` — fetches and opens a published `gamebuilder.org/g/<CODE>` game in Play mode.
- **Single-Instance Socket (`~/.openagents/desktop/activate.sock`):**
  - Extend `dispatchCommand(op:obj:)` with `"metaverse"` (`{"op":"metaverse","path":"...","expand":false}`) and `"gamebuilder"` (`{"op":"gamebuilder","game":"...","level":"...","mode":"build","expand":true}`) so CLI invocations (`openagents --scene foo.glb`, `openagents --gamebuilder`) activate the running window rather than spawning a second app instance.
- **`Info.plist` Document Types (Full Builds / `UTExportedTypeDeclarations`):**
  - Register `com.openagents.gamebuilder-world` (`.world`), `com.openagents.gamebuilder-game` (`.game`), `com.openagents.gamebuilder-mesh` (`.gbm`), `com.openagents.metaverse-pack` (`.pack`), `com.openagents.ple-scene` (`.ple-scene.json`) as `Owner`, and `model/gltf-binary` (`.glb`), `model/gltf+json` (`.gltf`), `public.3d-content` (`.obj`, `.stl`, `.usdz`), `.ifc` as `Alternate` viewer handlers.

---

## 5. Metaverse Viewer Mode (3D Models, Scenes & Realms)

When a tab is in **Viewer Mode** (`.glb`, `.gltf`, `.obj`, `.stl`, `.usdz`, `.gbm`, `.pack`, `RBA1`, `.ple-scene.json`, `.ifc`, `.splat`/`.ply`, `anim.scene.v1`, `.aimport`, or a Realm), the pane acts as a fast, native, GPU-accelerated 3D inspector built directly on `metaverse-engine/`.

### 5.1 New Generic 3D Model Viewer (`metaverse-engine/content/model-view.coax`)

As audited in §2.4 (Gap G-2), `metaverse-engine/content/model-view.coax` does not yet exist—today `pack-import.coax:pi_glb` and `anim/gltf-scene.coax` drop PBR textures and refuse non-unit node scales or arbitrary static multi-material scenes. To make the right-side Metaverse Viewer genuinely work for **any** 3D model a user or agent drops in, this spec introduces `metaverse-engine/content/model-view.coax` (plus `metaverse-engine/gpu/model-view-shader.coax` compiled via `coax metal` / `coax spirv` / `coax wgsl`):

- **Supported Formats & Hard Bounds (pure Coax, zero network I/O):**
  - **glTF 2.0 / `.glb` (`mv_load_glb`, `mv_load_gltf`):**
    - Reuses `anim/import-container.coax` (`ai_glb`) and `anim/gltf-access.coax` (`ga_read_*`).
    - Supports **textured PBR metallic-roughness** (`baseColorTexture` PNG/JPEG decoded via `metaverse-engine/render/host/host-image.coax` `htm_decode`, `baseColorFactor`, `metallicRoughnessTexture`, `metallicFactor`, `roughnessFactor`, `normalTexture`, `emissiveTexture`, `alphaMode` OPAQUE/MASK/BLEND, `doubleSided`).
    - Supports full node hierarchy (`TRS` and `matrix` transforms, non-unit scales, up to `1,024` nodes, `128` materials, `256` primitives/parts).
    - If skins/animations are present, delegates skeletal playback to `anim/gltf-scene.coax` (`skin8` + sparse morphs + animation clip selector with play/pause/scrub/loop bar).
    - Hard bounds: max `128 MiB` file, `2,000,000` vertices, `6,000,000` indices, `64` embedded textures (clamped/downsampled to `2048×2048` max per texture), total decoded VRAM capped by `RenderRegistry` sub-budget (§7.5). Any `uri` with `http://`, `https://`, `file://`, or `..` traversal is **hard-rejected** with a clear diagnostic message.
  - **Wavefront `.obj` + optional `.mtl` (`mv_load_obj`):** Pure Coax streaming parser for `v`, `vn`, `vt`, `f` (triangles, quads, convex n-gon fan triangulation), `o`/`g` part groups, `usemtl`/`mtllib` (local sibling directory only). Auto-generates flat/smooth normals when `vn` is absent.
  - **Binary & ASCII `.stl` (`mv_load_stl`):** Pure Coax parser computing face normals, welded bounding box, manifold/triangle count, and default CAD studio matcap shading.
  - **`.usdz` / `.usd` (macOS Adapter):** Thin Swift/ObjC host bridge using Apple's `ModelIO` (`MDLAsset`, restricted to local file URL, `preserveTopology = false`, triangulated) extracting positions, normals, UVs, baseColor/metallic/roughness textures, and node names into the exact same `MvModel` struct rendered by `model-view-shader.coax`—so USDZ models get identical camera, wireframe, grid, Z-slice, picking, and cutout support without embedding `SceneKit`.
  - **`.gbm` (`GBM1`) & `.pack` (VPK1/2/3):** Direct adapters over `gamebuilder/core/meshes.coax` and `metaverse-engine/editor/pack.coax` / `page-pack.coax`, showing every contained mesh, texture layer, sound (click to audition), rig (`VRG1`), and licence header.
  - **`RBA1` Rigid Assemblies (`microduck.coax` convergence):** When a VPK1 contains an `RBA1` catalog line, `model-view.coax` mounts `anim/assembly.coax` (`assembly_admit`, `assembly_pose`, `assembly_palette`, `assembly_pick`) and surfaces the Joint Pose sliders, Explode slider, Joint Axis overlay, and optional Servo Lab playback (`SLV1`/`SLR1`) right in the Inspector drawer.
  - **PLE / IFC Convergence (`ple-scene.coax` + `ple-ifc.coax`):** Moves `.ple-scene.json` and `.ifc` rendering onto the Metaverse Engine instanced box/cylinder pass using the native Coax `ple-ifc.coax` parser—eliminating `tools/ple-ifc-to-scene.py` and retiring standalone `PleSceneMetalView` while retaining 100% of existing PLE features (9 semantic layers, translucent architecture, clash spheres, Z-clip slice plane, 4 camera presets, and before/after visual diff overlay).
  - **Gaussian Splats (`.splat` / 3DGS `.ply`):** Header-sniffed into a dedicated Splat viewport pass (Phase 2/4: native Metal/Vulkan compute radix sort replacing WebGPU's 121-pass fragment bitonic sort, capped at `1,048,576` splats within the host `RenderRegistry` budget).

### 5.2 Camera & Navigation Controls

Every Viewer tab uses the shared Coax camera math in `metaverse-engine/editor/viewport.coax` + `metaverse-engine/shot/{blend,closeup}.coax`:

| Input | Orbit Mode (Default) | Fly Mode |
|---|---|---|
| **Left-drag** | Orbit around pivot (`yaw`, `pitch` clamped `±89°`) | Look around (yaw/pitch) |
| **Middle-drag / `Shift`+Left-drag / Two-finger drag** | Pan pivot in screen plane | Strafe / vertical pan |
| **Scroll wheel / Trackpad pinch / `Option`+drag** | Dolly zoom (exponential distance scaling, never clips past `0.05 × radius`) | Adjust fly speed (`0.5×`–`8×`) |
| **`W` `A` `S` `D` / `Q` `E`** (when `CP_KB_MV` focused) | Auto-nudge pan or hold right-mouse for temporary fly | Fly forward/back/left/right/down/up (`Shift` boost) |
| **Double-click geometry** | Smooth `camera_blend` (`0.28s` smootherstep) focusing clicked part | Same |
| **`F` key / Focus button** | Frame selected part (or whole scene bounds if nothing selected) | Same |
| **Camera Presets (`1..5` in Viewer)** | `Iso`, `Front (+Z)`, `Right (+X)`, `Top (+Y)`, `Under (-Y)` via `camera_blend` | Same |

- **Auto-Framing on Open:** Every loaded model/scene computes its axis-aligned bounding box (`min`, `max`, `center`, `radius`) and initializes an isometric 3/4 view at distance `2.4 × radius` with Y-up ground grid snapped to `min.y` (or explicit world origin for scenes/levels).
- **Per-Tab Camera Persistence:** Switching between Tab 1 (`quail.glb`) and Tab 2 (`Scrappers Arena`) preserves each tab's exact `eye`, `at`, `fov`, `mode`, shading options, and slice plane.

### 5.3 Shading, Inspection Overlays & `view-menu-v1`

Row 2's View Menu button (`CP_MV_VIEW_MENU`) projects `metaverse.engine.view-menu.v1` (`docs/protocol/view-menu-v1.md`) directly into a Coax popover menu (never an unstyled AppKit popup):

- **Shading Mode:** `Lit (PBR / Stylized)` | `Studio Clay / Matcap` | `Normals` | `Unlit Albedo` | `Wireframe` | `Lit + Wireframe`
- **Lighting Rig:** `Studio Neutral` (default 3-point hemisphere + key + rim) | `Daylight Sun` (`atmosphere.coax`) | `Dark Lab` | `Turntable Light`
- **Scene Overlays:**
  - `Ground Grid` (analytical `fwidth` metric grid with origin X/Y/Z axes)
  - `Bounding Box & Dimensions` (displays exact `W × H × D` in meters/centimeters)
  - `Clip / Z-Slice Plane` (interactive slider along Y or Z axis for inspecting interiors, CAD/IFC floors, or MEP clashes)
  - `Visual Diff Overlay` (for PLE/IFC comparisons and `world.edit.v1` revision diffs: added green, moved amber, removed translucent red)
  - `Stats HUD` (compact bottom-right translucent pill: `FPS · Frame ms · Verts · Tris · Draws · VRAM MB`)
- **Animation / Simulation Bar (auto-shown at viewport bottom only when applicable):**
  - For animated `.gltf`/`.glb` or `anim.scene.v1`: Play/Pause, Clip selector dropdown, timeline scrubber, speed (`0.25×`..`2×`), Loop toggle.
  - For `RBA1` rigid assemblies: Joint angle sliders + Explode factor (`0%..100%`) + Axis gizmos.

### 5.4 Scene Hierarchy / Part Tree & Inspector Drawer

When the drawer (`◧`) is open in Viewer Mode:
- **`Tree` tab:** Collapsible tree of scene nodes, mesh primitives, PLE layers, rigid-assembly parts, or VPK pack entries.
  - Each row has an eye visibility toggle, highlight-on-hover, and click-to-select (syncs bidirectionally with raycast clicking inside the 3D viewport).
  - Filter input at top (`CP_KB_GB_PANEL`) for quickly finding a part in large assemblies (e.g., `"rack"`, `"duct"`, `"wheel"`).
- **`Inspect` tab:**
  - Selected node/part name, path, world transform (`pos`, `rot`, `scale`), local & world bounds.
  - Material summary (shader, base color swatch, metallic/roughness, texture resolution).
  - Asset provenance block: file size, format version, SHA-256 prefix, and closed-enum licence (`PIP_LICENCE` when present in VPK/`world.assets.v1`).
  - **`Attach Selection to Chat`** button: appends a structured selection chip (`path`, `part_name`, `bounds`, `camera`) into the conversation composer.

### 5.5 Live Realm Follow, Revision Truthfulness & Snapshot-to-Chat

When viewing a live Metaverse realm (`openagents://metaverse/realm/<slug>` or `world.doc.v1` backed by a server):
- **Live Follow (`metaverse-engine/editor/world-follow.coax`):** Polls or receives server nudges (`dev 1s / live 5s / idle 30s`, inbox 614, `409` stale reopen) and applies incremental `world.state.v1` updates without losing the user's orbit camera.
- **Explicit Revision Truthfulness (`environments-claude-and-world-editing.md` §8.3):**
  - Row 2 displays a persistent status pill distinguishing four non-conflatable states:
    1. `rev 27 · live` (native viewport rendered authoritative revision 27)
    2. `saved rev 28 · rendering rev 27…` (mutation saved, native/preview catch-up in flight)
    3. `saved rev 28 · preview unavailable` (never silently draws rev 27 labeled as 28)
    4. `proposal pending (202)` (agent proposed an edit not yet committed)
- **Snapshot-to-Chat (`CP_MV_SNAPSHOT`):**
  - Captures the current offscreen Metal/Vulkan framebuffer to a bounded PNG (`≤ 1280px` long edge, `≤ 512 KiB`, saved under `~/.openagents/desktop/snapshots/mv-<timestamp>.png`) and attaches it to the chat composer via `cd_attach`, alongside a compact JSON camera/selection note so the agent knows *exactly* what the user is looking at.

---

## 6. GameBuilder Interface Inside the Pane

### 6.1 Architectural Coupling: Strict Adherence to `GB_COMMANDS` & `GB_CONTROLS`

GameBuilder is **not** re-implemented in `apps/openagents-desktop/`. Instead, the Metaverse Pane hosts `gamebuilder/core/` directly as a native Coax library (`gamebuilder-core.o`, §8.1), obeying `gamebuilder/INVARIANTS.md` unconditionally:

- **Single Command Table (`gamebuilder/core/commands.coax`, `GB_N = 120` verbs):** Every user click, keyboard shortcut, gizmo commit, inspector stepper, level operation, play/stop toggle, agent dispatch, publish action, and lobby message routes through `ed_command` / `gb_run` (`{"ok":true,...}` or `{"ok":false,"error":CODE,"why":WORDS}`).
- **Single Declarative Control Table (`gamebuilder/core/controls.coax`, `GB_CONTROL_N = 73` controls across 9 `GB_PLACE_*` places):** Desktop toolbar strips (`toolbar.`), native macOS/Linux/Windows menus (`menu.`), levels menus (`levels.`), read-only remix banner (`banner.`), panel actions (`panel.`), edit menu (`edit.`), agents drawer (`agents.`), play bar (`play.`), and lobby (`lobby.`) read their enabled states, shortcuts, and labels from `ed_control_*` / `GB_CONTROLS` so the desktop pane can never drift from CLI, MCP, HTTP, or `commands-check.coax`.
- **Single Design System Adaptation (`docs/gamebuilder/claude-ui-spec/tokens.css`):** GameBuilder's monochrome Geist / Geist Mono design tokens (`--df-chrome-bar-h: 36px`, `--df-row-h: 32px`, neutral monochrome ladders with semantic `danger`/`success`/`warning`/`info` accents) match OpenAgents Desktop's Noir monochrome skin natively—meaning GameBuilder panels feel 100% native inside the desktop right pane without visual jarring.

### 6.2 Three Workspace Modes Inside a GameBuilder Tab

Every GameBuilder tab exposes a 3-way segmented mode switch in Row 2: **`Inspect`** | **`Build`** | **`Play`**.

| Mode | Viewport Behavior | Sub-Bar & Drawer | Audio | Keyboard (`CP_KB_MV`) |
|---|---|---|---|---|
| **`Inspect`** | Orbit/Fly camera, read-only geometry, shows `claims.coax` ground rectangles, `changes.coax` agent highlights, and `playtest.coax` bot path overlays. | No Build sub-toolbar; Drawer defaults to `Agents` or `Levels`. | Muted | `WASD`/Arrows fly, `F` focus, `1..5` presets. |
| **`Build`** *(default for owned levels)* | Active raycast picking + `Select/Move/Rotate/Scale` gizmos + snap grid; `world.coax` holds the 9 web worlds still while editing; shows selection wireframe (`LOOK_SELECTED`), claim zones, and change outlines. | Row 3 Build Sub-Toolbar (`30pt`) visible; Drawer defaults to `Build` (`panel.coax` Palette + Inspector); optional bottom **Hotbar** in Expanded (`⛶`) mode. | Muted | `Q/W/E/R` tools, `G` snap, `F` focus, `Delete`, `⌘Z`/`⇧⌘Z`/`⌘C`/`⌘V`/`⌘D`, `1..9` hotbar. |
| **`Play`** | Live 60/120 Hz simulation (`play.coax` for Scrappers/Field/SPLICEBOTS or `world.coax` → `web/hud.coax` for the 9 web worlds). Optional one-click **`⛶` Expand** hint pill for widescreen play. | Row 3 Play Bar (`30pt`): Score/Timer/Mode selector (`Knockouts`/`Last Stand`/`Scrapball`/`Hoops`), Class picker (`Bolt`/`Boulder`/`Lumen`), Bot count (`1v1`..`5v5`), Audio mute/unmute, `■ Stop (Esc)`. Drawer auto-collapses during Play to maximize viewport. | Unmuted (on explicit Play; mute icon in Play Bar) | Game controls (`WASD`, mouse aim/shoot/click, `Space`, `Shift`, `1..3` abilities, `Esc` stops Play). |

### 6.3 Game & Level Lifecycle (`picker.coax`, `games.coax`, `level.coax`)

1. **New Game Picker (`gamebuilder/core/picker.coax`):**
   - Triggered from `+` → *New GameBuilder Game…*, the empty-state Metaverse Home view, `⌘K` → `GameBuilder: New Game…`, or `Levels` drawer → `+ New Game`.
   - Renders `EdPicker` natively inside the pane body (responsive 2 columns in split mode `520pt`, 3–4 columns in expanded mode `⛶`) showing all **`GT_TYPES = 12`** starter templates with `1..9` hotkeys and JPEG preview cards (`ed_picker_pic_rect`):
     - **App-origin templates (`GT_ON_WEB = false`):**
       1. `arena` (`GT_ARENA`) — Scrappers Arena (4 modes: Knockouts, Last Stand, Scrapball, Hoops; 3 fighter classes)
       2. `adventure` (`GT_ADVENTURE`) — SPLICEBOTS Adventure (5 route maps, Director + Raiders, digger/bag/travel)
       3. `field` (`GT_FIELD`) — Open Field sandbox
     - **Shared / Web-publishable templates (`GT_ON_WEB = true`, rendered via `world.coax` → `web/hud.coax`):**
       4. `third-person` — 3D meadow third-person world
       5. `first-person` — First-person exploration/shooter world
       6. `top-down` — Top-down action world
       7. `side-scroller` — 2.5D side-scrolling world
       8. `village` — 0 A.D. village builder (`village.pack`: `britons_house`, `gauls_house`, `market`, `mossy_rock`, `rock`)
       9. `crates` — Sokoban/physics crate puzzle (3 starter levels)
       10. `platformer` — Jump pads, spikes, movers, lifts, shards, checkpoints
       11. `racing` — Track, boost pads, rings, cones, laps
       12. `drift` — Arcade drift circuit
   - Picking a template executes `new-game` via `ed_command`, creates `game-N.game` + starter `level-N.world` (`gt_starter`), opens the new tab in `Build` mode, and focuses the starter level.
2. **Shipped Official Games & Ownership (`games.coax`):**
   - `Scrappers Arena` (`scrappers-arena`) and `SPLICEBOTS` (`splicebots`) appear in the `Levels` switcher immediately.
   - Ownership is enforced strictly via `ed_may_edit`: non-owners viewing a shipped or shared (`gamebuilder.org/g/CODE`) game see a clean Row 3 **Read-Only Banner** (`GB_PLACE_BANNER`): `"Viewing official/shared game · Read-only"` with a primary **`Make my own copy`** button (`copy-level` / remix `--from CODE`) that forks an editable local copy in one click.
3. **Level & Game Management:**
   - Rename, duplicate, move level between games, set owners (`owners` / `set-owners`), and double-press-to-confirm delete (preserving `panel.coax`'s accidental-delete guard) all live in the `Levels` drawer tab and dispatch `rename-level`, `copy-level`, `move-level`, `delete-level`, `rename-game`, `delete-game`.

### 6.4 Build Mode: Gizmos, Palette, Hotbar, Inspector & Three Render Pipelines

#### Row 3 Build Sub-Toolbar (`CP_GB_BAR3_H = 30.0pt`, driven by `GB_PLACE_TOOLBAR`)
```
[↶ Undo] [↷ Redo] │ [Q Select] [W Move] [E Rotate] [R Scale] │ [⊞ Snap: 50cm ▾] │ [+ Block] [F Focus] [⌫ Del] │ [✦ Decide] │ [◎ Show Changes (3)]
```
- **Gizmos & Snapping (`metaverse-engine/editor/viewport.coax` + `gamebuilder/core/editor.coax`):**
  - Left-click picks entities (`vp_ray` → `ed_pick`); Shift-click multi-selects (or single entity depending on command support); dragging gizmo handles translates on `[10, 25, 50, 100]` cm grid, rotates in `15°` steps (`-179..180°`), or scales in `5%` steps (`25..400%`).
  - Every committed drag writes a single atomic undo step (`128`-step / `256`-op undo ring in `editor.coax`) and bumps the level revision.
- **Build Drawer Tab (`gamebuilder/core/panel.coax`):**
  - **Add Palette:** Shows only the entity kinds valid for the active game's template (filtered from the 33 `GK_KINDS`: `block`, `start`, `rival`, `tree`, `water`, `path`, `pack`, `goal`, `rival-goal`, `hoop`, `rival-hoop`, `ball`, `target`, `moving-target`, `enemy`, `pad`, `spikes`, `shard`, `checkpoint`, `wall`, `crate-goal`, `boost`, `cone`, `track`, `ring`, `britons_house`, `gauls_house`, `market`, `mossy_rock`, `rock`, `mover`, `lift`, `gate`). Clicking a tile dispatches `add` (or enters click-to-place mode on the ground plane).
  - **Selection Inspector (`FIELDS = 11` from `palette.coax`):**
    - `FIELD_KIND` (kind dropdown), `East` (cm), `Up` (cm), `North` (cm), `Turn` (deg), `Scale` (%), `Width` (cm), `Height` (cm), `Depth` (cm), plus conditional `FIELD_GO` (`0–5000 cm`) and `FIELD_EVERY` (`1000–60000 ms`) for moving targets/movers/lifts.
    - Each numeric field supports `-` / `+` steppers (`PN_STEP`), click-drag scrubbing, and direct keyboard numeric entry (`CP_KB_GB_PANEL`), committing through `set-field`.
  - **World Tunables (`web/props-panel.coax`):** For the 9 web templates, an expandable *"World Settings"* section exposes `metaverse-engine/state/world-tunables.coax` parameters (gravity, sun angle, fog/atmosphere, grass density) via `set` / `gravity`.
- **Optional Bottom Hotbar (Expanded Mode `⛶`):**
  - Mirrors `gamebuilder/web/page.coax` (`components/kit/hotbar.coax`): 9 customizable quick-place slots (`1..9` keys, Shift-drag to reorder, `snap`/`free` toggle) anchored cleanly at the bottom center of the viewport when `mv.expanded == true` and `mode == Build` (excluded from `cd_mv_rect` or rendered via `composition-v2` in Phase 4).
- **Three Unified 3D Rendering Pipelines Inside the Viewport:**
  1. **Instance-Stream Box/Mesh Pipeline (`editor.coax` + `play.coax` + `meshes.coax` `.gbm`):** Renders `arena`, `adventure`, `field`, ground grid (`GROUND_HALF = 200m`), placed shapes (`INSTANCES_MAX` × 20-float instances with `LOOK_*`/`TINT_*` and `.gbm` mesh IDs `0..32`), selection wireframes, claim ground boxes, agent change outlines/ghosts, playtest path bars, and live Scrappers fighters/shots/sparks/packs/goals/hoops/ball using a Coax-compiled Metal/Vulkan shader (`gamebuilder/core/gpu/gb-instances.coax` replacing the deleted Swift `Shaders.metal`).
  2. **SPLICEBOTS UE World & Character Pipeline (`farm.coax` + `model.coax` + `rave.coax` + `scene.coax` + `her.coax`):** Streams `splice-bots-jam.pack` (5 route maps: `L_SolarFarm`, `L_SolarFarm_GreekQuarter`, `L_Plain`, `L_Ring`, `L_Lab`, up to `5,120` draws) and `splice-bots-jam-chars.pack` (`SjcCast`, 65-joint blended bone palettes for the Director + up to 64 crowd/placed raiders) via `metaverse-engine` native host passes.
  3. **Metaverse Engine Stylized Page Pipeline (`world.coax` → `web/hud.coax` + `web/gpu/hud.coax`):** Renders all 9 website game types (`third-person`..`drift`) with full stylized HDR, shadows, 480×480 swaying grass, sunrise reveal, CPU-skinned character (`web/ghost.coax`), 0 A.D. VPK1 village meshes (`village.pack`), rigid-body physics (`physics/sim/bodies.coax`), and `world_lines` overlays—plus all 5 standalone playable worlds (`fated-dragon`, `splicebots`, `whispering-spring`, `scrappers-arena`, `ice-cream-man`)—using the exact same Coax shaders compiled via `coax metal` / `coax spirv`.

### 6.5 Play Mode & Deterministic Play-Testing (`play.coax`, `modes.coax`, `playtest.coax`)

- **Live Play Mode:**
  - Clicking **`Play`** (or pressing `⌘P` when `CP_KB_MV` is focused) dispatches `play` (`ed_command`), snapshots the current level state (so gameplay never corrupts the saved Build layout), transitions smoothly into the gameplay camera (`cam/chase.coax` / `cam/kinds.coax`), and steps `play_step` (`1/60s` fixed physics accumulator interpolated to `60/120 Hz` display refresh) or `world.coax`'s live web-world rules.
  - For **Scrappers Arena**, the Row 3 Play Bar lets the user switch between all 4 match modes (`Knockouts` first to 10 / 150s, `Last Stand` 60s rounds / best of 7, `Scrapball` first to 3 goals, `Hoops` first to 5 hoops), choose fighter class (`Bolt`, `Boulder`, `Lumen`), and pick bot team sizes (`1v1`..`5v5`).
  - Clicking **`Stop`** (or pressing `Esc`) dispatches `stop`, restores the Build camera and exact pre-play level state, and mutes gameplay audio.
- **Deterministic Headless Play-Test (`playtest.coax`):**
  - Clicking **`▶ Test`** runs `play-test` asynchronously (off the UI thread) using a deterministic seed:
    - `GT_TEST_MATCH`: full bot-vs-bot `play_step` simulation returning score, duration, kills/goals, stuck-bot positions, and path traces (`gamebuilder.playtest.v1`).
    - `GT_TEST_WALK`: `metaverse-engine/world/nav/grid.coax` (`NavGrid`, up to `16,384` cells ≥ 0.5m, 4 m/s walk-through) verifying reachability from `start` to objectives/goals/checkpoints.
  - Results appear as a compact summary card in the drawer + **3D path overlays (`PIC_SEGS`) and glowing red stuck-bot pins directly in the viewport**. Clicking any issue row smoothly focuses the camera (`camera_blend`) right onto the stuck spot, and a one-click **`Ask Chat to Fix`** button attaches the play-test report + stuck coordinates + viewport screenshot into the chat composer!

### 6.6 Multi-Agent Collaboration, Chat Round-Trip & Replay Timeline

This is the centerpiece integration connecting OpenAgents Desktop's chat column with GameBuilder's native pane:

1. **Live Attach Socket (`~/.openagents/gamebuilder/attach.sock`):**
   - Whenever a GameBuilder tab is open in OpenAgents Desktop, the desktop host binds an owner-only Unix socket (`0600` in `~/.openagents/gamebuilder/` `0700`, verifying `getpeereid`/`SO_PEERCRED` == current UID) wired directly into `gamebuilder/core/attach.coax` & `attached.coax`.
   - Any external or daemon-spawned `gamebuilder --attach ...` or `gamebuilder mcp --attach` client connects to this socket, verifies `gb_row_digest` (command table compatibility), and executes any of the 120 `GB_COMMANDS` **directly against the live open level**.
   - Every committed agent mutation:
     - Updates the 3D viewport on the very next frame (zero manual reload).
     - Records a `changes.coax` mark (`CH_MARKS`) so changed/added entities glow with an outline and the Row 3 **`◎ Show Changes (N)`** / **`Dismiss`** controls light up (`show-changes` / `dismiss-changes`).
     - Respects `claims.coax` colored ground rectangles (agents claim regions before editing so parallel Builder agents never collide; **human edits always override claims**).
     - Generates offscreen before/after JPEG pictures (`pictures.coax`, `PIC_BOXES` or `PIC_PAGE`) saved to `level-N.pictures` and surfaced in both the Agents timeline and the Desktop chat's `TK_WORLD` card.
2. **Bidirectional Viewport → Chat Context (`world.turn.v1`):**
   - Selecting 1+ entities in the GameBuilder viewport shows a subtle floating pill / Inspector action **`Reference in Chat`** (or `Option`-click any point/entity in the viewport).
   - This attaches a compact context chip to the Desktop chat composer (using `page/attach.coax` + `proto/desktop/metaverse-selection-v1.json`) carrying:
     ```json
     {
       "schema": "desktop.metaverse.selection.v1",
       "kind": "gamebuilder",
       "game": "scrappers-arena",
       "level": "level-1",
       "rev": 14,
       "selected_ids": ["shape-22"],
       "hit_cm": [450, 0, 120],
       "camera": {"eye": [12.4, 8.2, 15.0], "at": [4.5, 0.0, 1.2]}
     }
     ```
   - The user can simply type *"move this ramp 3m east and add a jump pad next to it"* without ever typing coordinates or entity IDs.
3. **In-Pane Agents Drawer Tab (`gamebuilder/core/agents.coax`, `agent-timeline.coax`, `agent-replay.coax`):**
   - Renders GameBuilder's native agent manager directly inside the Studio Drawer (`This game` / `All games` filter):
     - **`+ New Agent` sheet:** Launch a **Builder**, **Playtester**, or **Code** agent (`code-agent.coax` creating git worktrees in `~/gamebuilder-code/REPO-ID` for engine/game code changes via `⇧⌘K`).
     - **Budget & State Meter:** Displays the 9 canonical states (`working`, `waiting`, `queued`, `paused`, `done`, `stopped`, `failed`, `offline`, `unknown`) and cost/turn bar (`"Turn 6 of 20 · $0.42 of $2.00"`).
     - **Event Timeline & Replay Scrubber (`ATL_CARDS = 96`, `agent-replay.coax`):** Expandable tool calls, before/after JPEG cards with `Show me` & `Undo this`, and a timeline scrubber slider that lets the user visually scrub through every revision an agent made in the live 3D viewport!
     - **`✦ Decide` One-Click Suggestion (`decide.coax`):** Sends a NIP-98 signed next-edit suggestion request and highlights the proposed edit before committing.
     - **`Kept on your account` switch (`agents-account`):** Controls cloud sync of agent records/events.
4. **Upgrading `AgentsPage.swift` & `coax/lib/agent/desktop/agents-catalog.coax`:**
   - Update the built-in `"game-builder"` `ArDefRecord` in `agents-catalog.coax:50-63`:
     - `runtime`: `"coax-metaverse:gamebuilder-core"` (replacing `"coax-metal:SceneView"`)
     - `input_contract`: `"world.edit.v1"` (replacing `"ple.scene.edit.v1"`)
     - `output_contract`: `"gamebuilder.level.v1"` (replacing `"ple.scene.document.v1"`)
   - Update `AgentsPage.swift:launchGameBuilder` so clicking **Launch** acquires a non-overlapping `TargetBinding` lease (`ar_bindings_conflict` over `game_id/level_id`) and opens the live GameBuilder tab inside the right-side Metaverse pane instead of `fixtures/ple-data-hall-sample.ple-scene.json`.

### 6.7 Publishing, Version Rollback, Account Sync & Global Lobby

- **Share / Publish Drawer Tab (`gamebuilder/core/publish.coax`):**
  - Displays current publish status, 6-char lookalike-free share URL (`https://gamebuilder.org/g/CODE`, `PUB_ALPHABET`) with one-click Copy Link & Open in Browser.
  - **`Publish`** button validates limits (`≤ 256 KB` level, `≤ 16` games, `≤ 128` versions, `≤ 48B` name), uploads SHA-256 content-addressed version (`publish`), and updates the live pointer.
  - **Version History & Rollback:** Lists recent versions with one-click **`Set Live`** rollback (`set-live`), **`Unpublish`**, and **`Delete from web`** (`remove-published`).
  - **Player Feedback Moderation:** Lists submitted player feedback with `Hide` (`feedback-hide`), `Block` (`block`), and `Report` (`report`), plus server-counted play/remix stats (`gamebuilder.plays.v1`).
- **Account & Sync (`sync.coax`, `cli/account.coax`):**
  - Connects to the user's OpenAgents account (reusing Desktop's signed-in session or browser-code L1 login `gamebuilder.org/login/cli` → `~/.openagents/gamebuilder/cli-login-PROFILE` `0600`) for bidirectional sync of games, levels, agents, events, pictures, and publish state.
- **Global Lobby & Party Drawer Tab (`lobby.coax`, `party.coax`, `net.coax`):**
  - Connects to `wss://relay.openagents.com` (NIP-42 AUTH kind `22242`, NIP-38 presence kind `30315`, kind `42` global lobby chat with 280B/no-links/12-word filter, NIP-56 report kind `1984`) and kind `25910` (`PT_KIND`) party signaling (up to 4 players, `mm=1` quick match) launching live multiplayer Scrappers matches over `wss://openagents.com/api/room/relay/scrappers/` right inside the Expanded (`⛶`) pane.

---

## 7. Rendering Engine, Context Audit & Cross-Platform Host Architecture

### 7.1 End-to-End Architecture Diagram

```
┌─────────────────────────────────────────── OpenAgents Desktop Process ───────────────────────────────────────────┐
│                                                                                                                  │
│  ┌───────────────────────── Coax UI Page (gallery.o, coder context) ─────────────────────────┐                  │
│  │  page/layout/layout.coax        page/metaversepane.coax         page/shows.coax            │                  │
│  │  • Split / Expanded ⛶ / Full    • Row 1 tabs + badges + ◫       • OA-SHOW-LIVE cards       │                  │
│  │  • cp_side_pane priority        • Row 2 breadcrumbs + modes     • TK_WORLD cards           │                  │
│  │  • app_presence spring          • Build/Play sub-bars           • cp_over_rects (≤8 holes) │                  │
│  │  • pane_color_* RGB exempt      • Studio/Inspector drawer UI                               │                  │
│  └───────────────┬─────────────────────────────────────────┬──────────────────────────────────┘                  │
│                  │ C-ABI (host.h: cd_mv_*, cd_gb_*)        │ C-ABI (cd_mv_rect, cd_over_rects, cd_live_scene)    │
│  ┌───────────────▼─────────────────────────────────────────▼──────────────────────────────────┐                  │
│  │  Platform Host Layer (Swift/ObjC on macOS; C11 on Linux Wayland & Windows Win32)           │                  │
│  │  • Window+Metaverse.swift / mv-host-linux.c / mv-host-win.c                                │                  │
│  │  • MetaverseViewportView (shared MTLDevice / VkDevice, single RenderRegistry VRAM ledger)  │                  │
│  │  • Overlay holes (Phase 1-3: CAShapeLayer mask; Phase 4: composition-v2 external texture)  │                  │
│  │  • File watchers, OS open/save dialogs, audio output (CoreAudio / PulseAudio / WASAPI)     │                  │
│  │  • Attach socket server: ~/.openagents/gamebuilder/attach.sock (0600, peer UID verified)   │                  │
│  └──────────────────────────┬────────────────────────────────────────────┬────────────────────┘                  │
│                             │ C-ABI (mv_host.h)                          │ C-ABI (gb_host.h)                     │
│  ┌──────────────────────────▼───────────────────────┐  ┌─────────────────▼────────────────────────────────────┐  │
│  │  metaverse-viewer.o (engine/realm context)       │  │  gamebuilder-core.o (gamebuilder context)            │  │
│  │  • metaverse-engine/content/model-view.coax      │  │  • gamebuilder/core/app.coax (120 GB_COMMANDS,       │  │
│  │  • metaverse-engine/anim/{gltf-scene,assembly}   │  │    73 GB_CONTROLS, editor, play, modes, playtest,    │  │
│  │  • metaverse-engine/import/{ple-scene,ple-ifc}   │  │    panel, picker, world->web/hud, farm/model/rave,   │  │
│  │  • metaverse-engine/editor/{viewport,world-follow│  │    agents, claims, changes, pictures, publish,       │  │
│  │    pack,page-pack}                               │  │    sync, lobby, party, attach, mcp)                  │  │
│  │  • Coax shaders -> .metallib / .spv              │  │  • Coax shaders -> gb-instances + web/gpu/hud        │  │
│  └──────────────────────────┬───────────────────────┘  └─────────────────┬────────────────────────────────────┘  │
│                             └──────────────────────┬─────────────────────┘                                       │
│                                                    ▼                                                             │
│                ┌───────────────────────────────────────────────────────────────────────┐                         │
│                │  Unified Context-Safe Native GPU Engine Host (aethyr-host-v2)         │                         │
│                │  • macOS: AEPageHost v2 (no phCurrent, RenderRegistry, full FP passes)│                         │
│                │  • Linux: vk-realm v2 (per-instance RealmHost*, Wayland/Vulkan)       │                         │
│                │  • Windows: vk-realm v2 over apps/shared/host/windows/vk-load.c       │                         │
│                └───────────────────────────────────────────────────────────────────────┘                         │
└─────────────────────────────────────────────────────▲────────────────────────────────────────────────────────────┘
                                                      │ ~/.openagents/gamebuilder/attach.sock (0600, gb_row_digest)
                                  ┌───────────────────┴───────────────────┐
                                  │  Bundled gamebuilder CLI & MCP server │
                                  │  (Contents/Resources/gamebuilder)     │
                                  │  ◄── Daemon workers (inky, mcworker,  │
                                  │      claudeworker, agyworker)         │
                                  └───────────────────────────────────────┘
```

### 7.2 Solving the Multi-Instance Context Blocker (`aethyr-host-v2`)

As established in the Metaverse Engine audit (§2.3, Gap G-4), running a right-pane 3D viewport alongside an inline `OA-SHOW-LIVE` card and cached background tabs in one process is **impossible today** because of three process-global singletons:

1. `metaverse-engine/host/apple/PageHost.m:135`: `static __weak AEPageHost *phCurrent;`, line 650 `static EngineCorePack pack;`, line 1307 static frame timing counters.
2. `metaverse-engine/host/metal/MetalRealm.m:77,457`: `static __weak MFRealm *activeRealm;` (`mf_open` explicitly rejects >1 instance).
3. `metaverse/gpu/vk-realm.c:116`: `static RealmHost H;` (process-global Vulkan host struct).

**Root cause:** The `aethyr-host-v1` C import ABI (`coaxial_socket_*`, `coaxial_http_*`, `guard_*`) passes no instance context pointer, forcing native hosts to stash the active host in a static global—which breaks async HTTP/WebSocket callbacks and concurrent frames.

**Required Engineering Fix (Phase 2 — `aethyr-host-v2`):**
1. **Context-Carrying C Import ABI (`aethyr-host-v2`):**
   - Introduce `typedef struct AethyrHostCtx AethyrHostCtx;` passed as the first parameter to all v2 host imports (`coaxial_v2_http_*(AethyrHostCtx *ctx, ...)`, `coaxial_v2_socket_*(ctx, ...)`, `coaxial_v2_guard_*(ctx, ...)`, `coaxial_v2_audio_*(ctx, ...)`), and stored on the producer's `state` struct during `coaxial_init_v2(state, ctx)`.
   - Keep `aethyr-host-v1` working untouched for browser Wasm (which is already isolated per Wasm instance) and legacy single-realm CLI binaries; native desktop/mobile hosts require `aethyr-host-v2`.
2. **Refactor `AEPageHost` (`metaverse-engine/host/apple/PageHost.m`):**
   - Delete `static __weak AEPageHost *phCurrent`, `static EngineCorePack pack`, and static timing accumulators; move all of them to ivars on `AEPageHost`.
   - Accept an injected shared `id<MTLDevice>`, `id<MTLCommandQueue>`, and shared `RenderRegistry` coordinator (`AEGpuBudget`) in `-initWithDevice:queue:budget:` so multiple `AEPageHost` instances share one Metal device and one aggregate VRAM ledger.
   - Adopt `metaverse-engine/render/registry.coax` (`RenderRegistry`) inside `AEPageHost` (closing the parity gap with `MetalRealm.m`, `vk-realm.c`, and WebGPU).
   - Wire the missing `ENGINE_FP_HOST_*` passes (water, character pass, instance culling, particle sort) into `PageHost.m` and consolidate `MetalRealm.m`'s GPU device-loss recovery (`mf_recover`) onto the single Apple host so macOS has **one** production Metal realm/viewer host instead of two diverging ones.
3. **Refactor `vk-realm.c` (`metaverse/gpu/vk-realm.c`):**
   - Replace `static RealmHost H` with heap-allocated `RealmHost *h` passed into every Vulkan pass/callback and sharing `VkDevice` + `VkQueue` + `RenderRegistry` with `apps/shared/host/vulkan-ui.c`.

### 7.3 Converging Fragmented Desktop 3D Views

- **`PleSceneMetalView` (`SceneView.swift` / `ScenePanelView.swift`) → Retired in Phase 1/2:**
  - Today `SceneView.swift` maintains its own `MTLDevice`, inline MSL shaders, and `PleSceneLoader` (which spawns `python3 tools/ple-ifc-to-scene.py`).
  - Moving PLE/IFC onto `metaverse-engine/import/ple-scene.coax` + `ple-ifc.coax` + a Coax-emitted instanced box/cylinder/clash shader (`metaverse-engine/gpu/ple-shader.coax`) eliminates Python subprocesses, eliminates duplicated Metal boilerplate, and gives PLE/IFC scenes automatic multi-tab, Expand (`⛶`), Snapshot-to-Chat, and Linux/Windows Vulkan support.
- **`RhFlythroughView.swift` (Gym Lean RH Workbench):**
  - Remains inside Gym Workbench (separate feature surface), but shares the window's single `MTLDevice` and registers its buffers/textures with the shared `AEGpuBudget` ledger so switching between Gym and Metaverse Viewer never leaks or double-allocates VRAM.

### 7.4 Composition & Overlay Strategy (Phase 1–3 vs Phase 4 `composition-v2`)

| Phase | Mechanism | Pros / Constraints |
|---|---|---|
| **Phases 1–3 (Immediate, Zero Blocker)** | Native `MetaverseViewportView` (`CAMetalLayer` / Vulkan child surface) mounted via `ui_child(..., above=1)` over `cd_mv_rect`, receiving up to 8 merged overlay cutouts via `cd_over_rects` → even-odd `CAShapeLayer` mask + `hitTest` pass-through (exact pattern proven by #13817 in `Window+Files.swift:455-570`). | Ships immediately on macOS Metal with 120 Hz zero-copy presentation and full overlay menu/palette compatibility. Constraint: Coax cannot apply `prim_backdrop` frosted glass blur *over* the 3D viewport (menus over 3D fall back to opaque `th.menu` fill). |
| **Phase 4 (`composition-v2` External Scene Output)** | Extend `coax/native/gpu-composition.coax` + `proto/components/composition-v2.json` with `PRIM_EXT_TEXTURE` (external opaque texture ID bound by the host from `AEPageHost.screen` / `VkImage` into `UcPlan`). | 3D viewport renders to an offscreen BGRA8 texture; `UiHost` composites it directly inside the single window render pass! Eliminates all `CAShapeLayer` hole masks, enables true frosted glass popovers, rounded viewport corners, and Coax HUD overlays (expanded Hotbar, floating pills) directly over 3D pixels at zero extra copy. |

> **Layering Guard (`components/rendering.md`):** `PRIM_EXT_TEXTURE` carries only an integer texture handle + rect inside `components/`; `components/` still NEVER `#load`s `metaverse-engine/` or `gamebuilder/`.

### 7.5 Shared GPU Memory, Frame-Rate & Thermal Budgets

Following `components/rendering.md` and `apps/INVARIANTS.md`:

- **Single Aggregate VRAM Budget:**
  - Desktop host ceiling: **`512 MiB`** total across all open Metaverse/GameBuilder tabs + inline `OA-SHOW-LIVE` cards (configurable lower on 8–16 GB machines: `min(512 MiB, 10% of system RAM)`).
  - Active focused right-pane tab receives up to **`384 MiB`**; inactive background tabs retain lightweight Coax CPU scene/camera state while evicting GPU textures/buffers in LRU order when aggregate VRAM exceeds `75%` of budget (re-uploading transparently on tab switch).
  - Active inline `OA-SHOW-LIVE` card receives up to **`96 MiB`** (and drops to `0 MiB` when deduplicated against the right pane per §4.3).
- **Dirty-Gated Frame Rate & Zero Idle Burn:**
  - **GameBuilder `Play` mode / animated clip / active camera orbit:** native refresh (`60 Hz` standard, up to `120 Hz` on ProMotion when focused and on AC power; capped at `60 Hz` on battery).
  - **Static Viewer / idle Build mode:** **`0 Hz` (event-driven dirty redraw)**—re-renders only on pointer move/drag, camera blend tick, file reload, agent mutation, or resize!
  - **Hidden (`◫`), occluded by Settings/another pane, scrolled offscreen, or window minimized/inactive:** immediately pauses tick & GPU submission (`0 Hz`) and releases transient MSAA/bloom render targets after `10 s` idle.
- **Audio Policy:** Muted by default in all Viewer, Inspect, and Build modes; unmuted only when entering GameBuilder `Play` mode or clicking an explicit audio button, routed through `metaverse-engine/audio/host-pcm.coax` (48 kHz, 1024-frame stereo blocks).

### 7.6 Cross-Platform Parity (macOS, Linux, Windows & Future iOS)

| Platform | UI Surface | 3D Viewport Surface | Shaders | Audio | Status / Target |
|---|---|---|---|---|---|
| **macOS (Apple Silicon)** | `apps/shared/host/apple/ui-host.m` (Metal) | `AEPageHost v2` (Metal, shared `MTLDevice`) | `coax metal` → `.metallib` | CoreAudio (`host-pcm.coax`) | **Primary target (Phases 0–4)** |
| **Linux (Wayland)** | `apps/shared/host/linux/ui-host.c` + `vulkan-ui.c` | `metaverse/gpu/vk-realm.c` v2 (shared `VkDevice`, subsurface / `composition-v2`) | `coax spirv` → `.spv` | PipeWire / PulseAudio (`host-pcm.coax`) | **Phase 4 parity** (`host-main.c`) |
| **Windows (Win32)** | `apps/shared/host/windows/ui-host.c` + `vk-load.c` + `vulkan-ui.c` | Same `vk-realm.c` v2 over `VK_KHR_win32_surface` / offscreen `VkImage` | `coax spirv` → `.spv` | WASAPI (`host-pcm.coax`) | **Phase 4 parity** (closes Gap G-8) |
| **iOS (`apps/openagents-ios`)** | `apps/shared/host/apple/ui-host.m` (Metal) | Single-instance `AEPageHost v2` modal/sheet (reuses exact `model-view.coax` + `gamebuilder-core`) | `coax metal` | AVAudioEngine (`host-pcm.coax`) | Downstream consumer (out of scope for v1 desktop PRs) |

> **Known SPIR-V Shader Note (`docs/metaverse/native-gpu.md:709`):** All new Viewer/GameBuilder shaders (`model-view-shader.coax`, `ple-shader.coax`, `gb-instances.coax`, `web/gpu/hud.coax`) MUST stay strictly inside the common lowering subset of `coax metal`, `coax spirv`, and `coax wgsl` (verified in CI by compiling all three targets on every shader edit).

### 7.7 Asset Hydration & Bundling

- **Out-of-Git Large Assets (`assets/registry/collections/gamebuilder.lock.json`):**
  - `gamebuilder/assets/scrappers.gbm` (1.87 MB), `gamebuilder/web/assets/village.pack` (1.84 MB), and `splicebots.lock.json` packs (`splice-bots-jam.pack` 61 MB, `splice-bots-jam-chars.pack`) stay out of git and hydrate to `~/.openagents/assets/sha256/` via `coax/web/tool.sh assets-sync --group gamebuilder`.
  - **First-Open Graceful Hydration:** If a user opens `Scrappers Arena`, `Village`, or `SPLICEBOTS` before assets are hydrated on disk, the Metaverse pane renders a clean Coax notice card inside the viewport: `"Download GameBuilder Starter Packs (3.7 MB, SHA-256 verified)"` (or `61 MB` on-demand for SPLICEBOTS) with a progress bar and Retry/Cancel—identical in pattern to Inkling's download card (`cp_local_onboarding`)—and never crashes or shows blank black screens.
- **`apps/openagents-desktop/build.sh` Updates:**
  - Under `--full` (`minimal != "1"`):
    - Compiles `metaverse-viewer.o` and `gamebuilder-core.o` (`ld -r` exported symbols only).
    - Emits and compiles `model-view.metallib`, `ple-scene.metallib`, `gb-instances.metallib`, `gb-hud.metallib` into `$res/Metaverse/`.
    - Runs `gamebuilder/cli/build.sh` and bundles `$res/gamebuilder` beside `$res/oasim`, `$res/ple-run`, `$res/lean-run`.
  - Under `--minimal` (`minimal == "1"`):
    - Strips `$res/Metaverse/`, `$res/gamebuilder`, and compiles `page/v1-stubs.coax` stubs so v1.0 release size and behavior are 100% unchanged.

---

## 8. Coax Page State, C-ABI Bridge, Flags & Proto Contracts

### 8.1 Avoiding `contexts.txt` Circularity

A critical constraint discovered by inspecting `contexts.txt`:
- Line 22: `coder` owns `apps/openagents-desktop/` and `may load: psionic nostr protocol toolchain realm-seat zeron-kit`.
- Line 37: `gamebuilder` owns `gamebuilder/` and `may load: engine realm-authoring psionic protocol coder chat nostr toolchain zeron-kit splice`.

Because `gamebuilder` already depends on `coder`, directly `#load`ing `gamebuilder/` from `apps/openagents-desktop/page/` would create a **forbidden cycle** rejected by `coax/web/tool.sh layer-check`.

**Clean, Idiomatic Solution (already used by `build.sh:92-106` for `rh-flythrough.o`, `pay-wallet.o`, and `lab-sign.o`):**

1. `apps/openagents-desktop/page/metaversepane.coax` stays inside `coder` and loads **only** `coder`/`protocol`/`toolchain`/`zeron-kit` (pure UI state, tab list, breadcrumbs, drawer chrome, hit targets, and host asks).
2. Add two narrow bridge entrypoints compiled as **separate relocatable `.o` libraries** via `coax c ... --library` + `ld -r`:
   - `metaverse-engine/content/desktop-viewer-bridge.coax` (lives in `engine` context → compiles to `metaverse-viewer.o`, exporting `_oa_mv_*` C ABI declared in `apps/openagents-desktop/mv_host.h`).
   - `gamebuilder/core/desktop-bridge.coax` (lives in `gamebuilder` context → compiles to `gamebuilder-core.o`, exporting `_oa_gb_*` C ABI declared in `apps/openagents-desktop/gb_host.h`).
3. `apps/openagents-desktop/Window+Metaverse.swift` (and `host-main.c` on Linux/Windows) links `gallery.o`, `metaverse-viewer.o`, and `gamebuilder-core.o` together and passes compact structs/strings across the narrow C headers—zero `contexts.txt` cycles, zero bloat in `gallery.o`, and instant omission in `--minimal` builds.

### 8.2 Feature Flags (`coax/lib/agent/desktop/flags.coax`)

Add two new flags to the `#authority` registry (`DC_FLAG_COUNT` bumped from `28` to `30`, both `default_on = false` for the Inkling-only v1.0 release):

```coax
DC_FLAG_SCENES      :: 26;   // existing: PLE/IFC 3D scenes & OA-SHOW-LIVE cards
DC_FLAG_RICH        :: 27;   // existing
DC_FLAG_METAVERSE   :: 28;   // new: "metaverse" — Expandable right-side Metaverse 3D Viewer pane (.glb/.gltf/.obj/.stl/.usdz/.pack/RBA1/realms)
DC_FLAG_GAMEBUILDER :: 29;   // new: "gamebuilder" — GameBuilder creation/play/agents/publish studio inside the Metaverse pane + bundled CLI/MCP
DC_FLAG_COUNT       :: 30;
```

- **Flag Composition Rules (`#authority`):**
  - `experimentalFeatures` / `--experimental` / `OA_EXPERIMENTAL=1` turns on `DC_FLAGS_ALL` (including both `metaverse` and `gamebuilder`).
  - Individual testing: `--flag metaverse` enables Viewer Mode alone (and subsumes `scenes` routing into the Metaverse pane); `--flag gamebuilder` enables GameBuilder tabs (automatically implying Metaverse pane chrome visibility even if `metaverse` wasn't listed separately: `cp_mv_enabled(s) := cp_flag(s, DC_FLAG_METAVERSE) || cp_flag(s, DC_FLAG_GAMEBUILDER) || cp_flag(s, DC_FLAG_SCENES)`).
  - When `--minimal` builds `app-v1.coax`, `page/v1-stubs.coax` provides zero-cost stubs for `CpMetaversePane`, `cp_mv_*`, and `coder_page_mv_*` / `coder_page_gb_*` so `carrier.c` and `host.c` link cleanly without `#ifdef` soup.

### 8.3 Coax Page Structs (`apps/openagents-desktop/page/metaversepane.coax`)

```coax
CP_MV_TABS      :: 12;
CP_MV_PATH      :: 1024;
CP_MV_TITLE     :: 96;
CP_MV_STATUS    :: 128;
CP_MV_BAR1_H    :: 38.0f32;   // matches CP_FP_BAR1_H
CP_MV_BAR2_H    :: 30.0f32;   // matches CP_FP_BAR2_H
CP_MV_BAR3_H    :: 30.0f32;   // Build/Play sub-toolbar or Read-only banner
CP_MV_DRAWER_W  :: 260.0f32;  // collapsible Studio / Inspector drawer
CP_MV_TREE_MAX  :: 256;       // visible hierarchy rows pushed from host
CP_MV_FIELDS    :: 11;        // mirrors gamebuilder/core/palette.coax FIELDS

// Tab kinds
CP_MV_KIND_HOME        :: 0;  // empty/launcher view (recent 3D + New Game picker)
CP_MV_KIND_MODEL       :: 1;  // .glb, .gltf, .obj, .stl, .usdz, .gbm
CP_MV_KIND_PACK        :: 2;  // .pack (VPK1/2/3), RBA1 rigid assembly, .aimport
CP_MV_KIND_PLE         :: 3;  // .ple-scene.json, .ifc
CP_MV_KIND_SPLAT       :: 4;  // .splat, 3DGS .ply
CP_MV_KIND_SCENE       :: 5;  // anim.scene.v1
CP_MV_KIND_REALM       :: 6;  // live openagents.com/realms/... or world.doc.v1
CP_MV_KIND_GAMEBUILDER :: 7;  // GameBuilder game/level or standalone world

// Active mode within a tab
CP_MV_MODE_INSPECT :: 0;
CP_MV_MODE_BUILD   :: 1;
CP_MV_MODE_PLAY    :: 2;

// Drawer sub-tabs
CP_MV_DRW_TREE    :: 0;  // Viewer: Hierarchy / Layers / Parts
CP_MV_DRW_INSPECT :: 1;  // Viewer: Properties & Provenance
CP_MV_DRW_BUILD   :: 2;  // GameBuilder: Palette + 11-field Inspector
CP_MV_DRW_LEVELS  :: 3;  // GameBuilder: Games, Levels, New Game, Owners
CP_MV_DRW_AGENTS  :: 4;  // GameBuilder: Agents, Claims, Changes, Replay
CP_MV_DRW_SHARE   :: 5;  // GameBuilder: Publish, Versions, Rollback, Feedback
CP_MV_DRW_LOBBY   :: 6;  // GameBuilder: Nostr Global Lobby & Party

// Revision truthfulness state (Invariant I-4)
CP_MV_REV_SYNCED      :: 0;  // rendered_rev == saved_rev
CP_MV_REV_RENDERING   :: 1;  // saved_rev > rendered_rev, render in flight
CP_MV_REV_UNAVAILABLE :: 2;  // saved_rev valid, preview unavailable
CP_MV_REV_PROPOSAL    :: 3;  // HTTP 202 proposal pending

CpMvTab :: struct {
  used: bool;
  kind: int;                       // CP_MV_KIND_*
  mode: int;                       // CP_MV_MODE_*
  path: [CP_MV_PATH] u8; path_n: int;       // filesystem path, realm URL, or "gb:<game_id>/<level_id>"
  title: [CP_MV_TITLE] u8; title_n: int;    // short tab pill label
  game_id: [64] u8; game_id_n: int;
  level_id: [64] u8; level_id_n: int;
  badge: [4] u8; badge_n: int;              // "GLB", "VPK", "PLE", "RLM", "GB", "SPL"
  saved_rev: int;
  rendered_rev: int;
  rev_state: int;                           // CP_MV_REV_*
  dirty: bool;
  read_only: bool;                          // !ed_may_edit (shows "Make my own copy" banner)
  agent_active: bool;                       // an attached/panel agent is editing this tab
  change_count: int;                        // changes.coax CH_MARKS count
  playing: bool;
  muted: bool;
  has_anim: bool;                           // shows animation scrubber bar
  has_assembly: bool;                       // shows RBA1 joint/explode controls
  cam_fly: bool;                            // false=Orbit, true=Fly
  shading: int;                             // 0 Lit, 1 Clay, 2 Normals, 3 Unlit, 4 Wire, 5 Lit+Wire
  overlay_mask: int;                        // bit0 Grid, bit1 Bounds, bit2 Slice, bit3 Diff, bit4 Stats, bit5 Claims
  z_slice: f32;                             // 0.0..1.0 normalized slice plane
  stats: [CP_MV_STATUS] u8; stats_n: int;   // e.g. "60 fps · 14.2k tris · 28 MB"
  err: [256] u8; err_n: int;                // non-empty on bounded load/parse refusal
}

CpGbUiState :: struct {
  tool: int;                 // 0 Select(Q), 1 Move(W), 2 Rotate(E), 3 Scale(R)
  snap_idx: int;             // 0: off, 1: 10cm, 2: 25cm, 3: 50cm, 4: 100cm
  can_undo: bool;
  can_redo: bool;
  template_id: int;          // GT_* (0..11)
  match_mode: int;           // 0 Knockouts, 1 Last Stand, 2 Scrapball, 3 Hoops
  fighter_class: int;        // 0 Bolt, 1 Boulder, 2 Lumen
  bot_team_size: int;        // 1..5
  sel_count: int;
  sel_kind: int;             // GK_* (-1 none)
  sel_fields: [CP_MV_FIELDS] int;
  sel_label: [64] u8; sel_label_n: int;
  share_code: [12] u8; share_code_n: int;
  attach_connected: int;     // count of attached MCP/CLI clients
  picker_open: bool;         // New Game 12-template card grid active
  hotbar_open: bool;         // bottom quick-place hotbar in expanded Build mode
  playtest_running: bool;
  playtest_summary: [128] u8; playtest_summary_n: int;
  playtest_stuck: int;       // number of stuck-bot pins
  hydrating: bool;           // true while assets-sync downloads scrappers.gbm / village.pack
  hydrate_pct: int;
}

CpMetaversePane :: struct {
  open: bool;
  front: bool;
  hidden: bool;              // collapsed via ◫ preserving tabs & GameBuilder sessions
  expanded: bool;            // expanded via ⛶ across main area beside left sidebar
  closed_by_person: bool;    // suppresses automatic agent pop-open until person reopens
  drawer_open: bool;
  drawer_tab: int;           // CP_MV_DRW_*
  drawer_scroll: f32; drawer_content: f32; drawer_view: f32;
  add_menu: bool;            // '+' quick-open popover state
  view_menu: bool;           // Row 2 View/Shading popover state
  cam_menu: bool;            // Row 2 Camera preset popover state
  more_menu: bool;           // Row 2 ⋮ popover state
  tabs: [CP_MV_TABS] CpMvTab;
  ntabs: int;
  active: int;               // index in tabs[0..<ntabs], -1 if Home
  vp_x: f32; vp_y: f32; vp_w: f32; vp_h: f32;   // 3D viewport body rect (top-left logical points)
  gb: [1] CpGbUiState;
  // Pending host command payload (populated before setting s[0].ask = CP_ASK_GB_CMD)
  pending_cmd: [512] u8; pending_cmd_n: int;
  pending_arg: [512] u8; pending_arg_n: int;
  pending_tab: int;
}
```

### 8.4 Exact Non-Colliding Constant Allocations

Audited directly against `origin/main`'s `apps/openagents-desktop/page/*.coax` (highest existing: `CP_ASK_FLAWED_RESET = 197`, UI action `CP_FP_EXT_LAST = 459`, `CP_PC_FORGE_ISSUES = 38`, `CP_KB_PROV = 7`, `CP_SCR_LEAN = 10`):

| Category | Allocated Range | Constants |
|---|---|---|
| **Host Asks (`CP_ASK_*`)** | `200..215` | `CP_ASK_MV_OPEN_DIALOG :: 200`, `CP_ASK_MV_LOAD_TAB :: 201`, `CP_ASK_MV_CLOSE_TAB :: 202`, `CP_ASK_MV_RELOAD :: 203`, `CP_ASK_MV_SNAPSHOT :: 204`, `CP_ASK_MV_ATTACH_SEL :: 205`, `CP_ASK_MV_CAM_ACTION :: 206`, `CP_ASK_MV_VIEW_OPT :: 207`, `CP_ASK_MV_EXTERNAL :: 208`, `CP_ASK_MV_OPEN_AS_TEXT :: 209`, `CP_ASK_MV_HYDRATE :: 210`, `CP_ASK_GB_CMD :: 211` *(dispatches any `GB_COMMANDS` JSON/verb)*, `CP_ASK_GB_MODE :: 212`, `CP_ASK_GB_NEW_GAME :: 213`, `CP_ASK_GB_FOCUS_ISSUE :: 214`, `CP_ASK_GB_NEW_CODE_AGENT :: 215` |
| **UI Action IDs** | `470..509` | `CP_MV_TOGGLE :: 470`, `CP_MV_HIDE :: 471`, `CP_MV_EXPAND :: 472`, `CP_MV_DRAWER :: 473`, `CP_MV_ADD_TAB :: 474`, `CP_MV_TAB :: 475`, `CP_MV_TAB_CLOSE :: 476`, `CP_MV_MODE :: 477`, `CP_MV_CAM_MODE :: 478`, `CP_MV_CAM_MENU :: 479`, `CP_MV_CAM_PICK :: 480`, `CP_MV_VIEW_MENU :: 481`, `CP_MV_VIEW_PICK :: 482`, `CP_MV_SNAPSHOT :: 483`, `CP_MV_RELOAD :: 484`, `CP_MV_MORE_MENU :: 485`, `CP_MV_MORE_PICK :: 486`, `CP_MV_DRAWER_TAB :: 487`, `CP_MV_TREE_ROW :: 488`, `CP_MV_TREE_EYE :: 489`, `CP_MV_ATTACH_SEL :: 490`, `CP_MV_ADD_PICK :: 491`, `CP_MV_HYDRATE :: 492`, `CP_GB_CTRL :: 495` *(generic `GB_CONTROLS` index dispatch)*, `CP_GB_TOOL :: 496`, `CP_GB_SNAP :: 497`, `CP_GB_PALETTE_ADD :: 498`, `CP_GB_FIELD_STEP :: 499`, `CP_GB_PICKER_CARD :: 500`, `CP_GB_PLAYTEST :: 501`, `CP_GB_PLAYTEST_PIN :: 502`, `CP_GB_PUBLISH_ACT :: 503`, `CP_GB_AGENT_ACT :: 504`, `CP_GB_REPLAY_SCRUB :: 505`, `CP_GB_LOBBY_ACT :: 506`, `CP_GB_HOTBAR_SLOT :: 507`, `CP_GB_REMIX_COPY :: 508`, `CP_WORLD_CARD_ACT :: 509` |
| **Palette Commands (`CP_PC_*`)** | `39..45` | `CP_PC_METAVERSE :: 39`, `CP_PC_MV_OPEN :: 40`, `CP_PC_MV_EXPAND :: 41`, `CP_PC_GAMEBUILDER :: 42`, `CP_PC_GB_NEW :: 43`, `CP_PC_GB_PLAY :: 44`, `CP_PC_GB_PLAYTEST :: 45` |
| **Keyboard Focus (`CP_KB_*`)** | `8..9` | `CP_KB_MV :: 8` *(3D viewport / Play / QWER)*, `CP_KB_GB_PANEL :: 9` *(Studio drawer text/numeric inputs)* |
| **Scroll Targets (`CP_SCR_*`)** | `11..13` | `CP_SCR_MV_VIEW :: 11`, `CP_SCR_MV_TREE :: 12`, `CP_SCR_GB_PANEL :: 13` |

### 8.5 Complete C-ABI Bridge Surface (`host.h`, `mv_host.h`, `gb_host.h`)

#### Additions to `apps/openagents-desktop/host.h` (`cd_mv_*` & `cd_gb_*`)
```c
// Metaverse Viewer & GameBuilder right-side pane (page/metaversepane.coax)
int  cd_mv_open_tab(void *page, int kind, const char *path, const char *title,
                    const char *game_id, const char *level_id, int mode, int auto_opened);
void cd_mv_show(void *page, int open, int expand);
void cd_mv_hide(void *page);
int  cd_mv_shown(void *page);
int  cd_mv_expanded(void *page);
int  cd_mv_hidden(void *page);
int  cd_mv_active_tab(void *page);
int  cd_mv_tab_path(void *page, int idx, char *out, int cap);
int  cd_mv_tab_kind(void *page, int idx);
int  cd_mv_tab_mode(void *page, int idx);
// Reports viewport body rect [x, y, w, h] in top-left logical points; returns 1 when visible.
int  cd_mv_rect(void *page, float out[4]);
// Host pushes updated state back to active tab chrome (revision truthfulness, stats, errors, dirty).
void cd_mv_tab_status(void *page, int idx, int saved_rev, int rendered_rev, int rev_state,
                      int dirty, int read_only, int agent_active, int change_count,
                      const char *stats, const char *err);
void cd_mv_set_tree(void *page, int idx, const char *tsv_rows);
int  cd_mv_pending_cmd(void *page, char *cmd_out, int cmd_cap, char *arg_out, int arg_cap);
// GameBuilder toolbar/inspector/agents sync (host pushes snapshot from gamebuilder-core.o)
void cd_gb_sync_ui(void *page, int tool, int snap_idx, int can_undo, int can_redo,
                   int template_id, int playing, int sel_count, int sel_kind,
                   const int sel_fields[11], const char *sel_label,
                   const char *share_code, int attach_clients);
void cd_gb_playtest_status(void *page, int running, int stuck_count, const char *summary);
void cd_gb_hydrate_progress(void *page, int active, int pct);
// Input forwarding when native viewport or host receives key/menu chords
int  cd_mv_focus_viewport(void *page);
```

#### Separate Relocatable Engine Bridge Headers (`mv_host.h` & `gb_host.h`)
- **`apps/openagents-desktop/mv_host.h`** (backed by `metaverse-viewer.o`):
  - `oa_mv_create`, `oa_mv_destroy`, `oa_mv_load_file`, `oa_mv_load_memory`, `oa_mv_set_viewport`, `oa_mv_pointer`, `oa_mv_scroll`, `oa_mv_key`, `oa_mv_camera_preset`, `oa_mv_set_shading`, `oa_mv_set_overlay`, `oa_mv_set_slice`, `oa_mv_pick`, `oa_mv_step_and_encode`, `oa_mv_snapshot_png`, `oa_mv_selection_json`, `oa_mv_tree_tsv`.
- **`apps/openagents-desktop/gb_host.h`** (backed by `gamebuilder-core.o`):
  - `oa_gb_create`, `oa_gb_destroy`, `oa_gb_command_json` *(single choke point to `ed_command` / `gb_run`)*, `oa_gb_control_press`, `oa_gb_attach_request` *(serves `~/.openagents/gamebuilder/attach.sock`)*, `oa_gb_pointer`, `oa_gb_key`, `oa_gb_step`, `oa_gb_render_frame`, `oa_gb_ui_snapshot`, `oa_gb_selection_turn_json`, `oa_gb_take_pictures`.

### 8.6 Versioned `proto/` Contracts & Registry Rows

Register all new contracts in `proto/registry.json` with a single Coax codec each:

1. **`proto/desktop/metaverse-route-v1.json` (`desktop.metaverse.route.v1`):** Canonical routing decision emitted by `coax/lib/agent/desktop/metaverse-route.coax` (`target_pane`, `tab_kind`, `default_mode`, `badge`, `read_only`, `requires_flag`).
2. **`proto/desktop/metaverse-selection-v1.json` (`desktop.metaverse.selection.v1`):** Structured 3D selection / picked-point / camera payload attached from the Metaverse or GameBuilder viewport to the Desktop chat composer (convertible into `world.turn.v1`).
3. **`proto/desktop/metaverse-tab-state-v1.json` (`desktop.metaverse.tab-state.v1`):** Optional persisted workspace session snapshot (open 3D tabs, camera poses, expanded/drawer preference per workspace folder).
4. **`proto/verse/aethyr-host-v2.json` (`aethyr.host.v2`):** Context-carrying native host import/export ABI contract enabling multi-instance `AEPageHost v2` and `vk-realm v2`.
5. **`proto/components/composition-v2.json` (`components.composition.v2`, Phase 4):** Extends `composition-v1` with `PRIM_EXT_TEXTURE` for zero-copy external 3D scene composition inside `UcPlan`.
6. **Reuse unchanged:** `world.edit.v1`, `world.state.v1`, `world.doc.v1`, `world.turn.v1`, `world.assets.v1`, `metaverse.engine.rigid-assembly.v1`, `metaverse.engine.view-menu.v1`, `ple-scene.v1`, `gamebuilder.*` (all 40+ existing schemas), `realm-frame.v1`, and `openagents.verse.{authoring,preview}.v1alpha1`.

---

## 9. Agent Tooling, Daemon Integration & Security / Confinement

### 9.1 Bundled `gamebuilder` CLI, Shim & Auto-Registered MCP in Daemon Workers

To guarantee that every agent running inside OpenAgents Desktop—**Inkling**, **Microcoder**, **Claude Code**, **Antigravity ACP**, and external terminal sessions—can immediately inspect, build, play-test, and photograph 3D worlds without manual setup:

1. **Bundled Helper Binary (`Contents/Resources/gamebuilder`):**
   - `apps/openagents-desktop/build.sh` (`--full`) compiles `gamebuilder/cli/build.sh` and copies `gamebuilder/build/cli/gamebuilder` into `$res/gamebuilder` (right beside `$res/oasim`, `$res/ple-run`, `$res/lean-run`, `$res/tester-cli`).
   - Settings > General > Command-Line Tools offers an optional one-click `~/.local/bin/gamebuilder` symlink shim (matching the existing `openagents` CLI shim in `Activation.swift`).
2. **Per-Session MCP Auto-Registration (`coder/tui/host-runs.coax`, `claudeworker.coax`, `mcworker.coax`, `inky.coax`):**
   - When `DC_FLAG_GAMEBUILDER` is active (or a GameBuilder tab is open in the workspace), the worker spawner automatically registers a local stdio MCP server:
     ```json
     {
       "mcpServers": {
         "gamebuilder": {
           "command": "<Bundle>/Contents/Resources/gamebuilder",
           "args": ["mcp", "--attach", "--as-agent", "<session_id>"]
         }
       }
     }
     ```
   - `gamebuilder mcp` reads directly from `GB_COMMANDS` (`GB_MCP` surface mask) and exposes typed JSON-RPC 2.0 tools (`list-games`, `list-levels`, `read-state`, `add`, `apply-edits`, `set-field`, `delete`, `undo-turn`, `claim`, `release`, `play-test`, `pictures`, `picture`, `publish`, etc.).
   - If the Desktop pane has an open GameBuilder tab, `--attach` routes transparently into the live app over `~/.openagents/gamebuilder/attach.sock` so edits animate live in the right pane. If the Desktop app is closed (because daemon runs outlive the app), `gamebuilder mcp` falls back seamlessly to headless file-backed editing against `--dir` so agent work never fails when the window closes—and when the user reopens Desktop, the updated revision and `level-N.pictures` appear immediately!
3. **Metaverse Viewer Agent Tools:**
   - Agents producing `.glb`, `.gltf`, `.obj`, `.stl`, `.ple-scene.json`, `.ifc`, or `.pack` files automatically surface in the right pane via §4.2 auto-open rules and `OA-SHOW` / `OA-SHOW-LIVE` markers.
   - `ple-run` (`apps/openagents-desktop/ple-run.coax`) continues emitting `OA-SHOW:` / `OA-SHOW-OPEN:` / `OA-SHOW-LIVE: on` lines, now routed directly into the unified Metaverse Viewer pane.

### 9.2 Owner-Only Socket Security & Table Digest Pinning

- **Attach Socket (`~/.openagents/gamebuilder/attach.sock`):**
  - Directory `~/.openagents/gamebuilder/` created mode `0700`; socket file `chmod 0600` (identical to `~/.openagents/desktop/activate.sock` in `Activation.swift:40-68` and `~/.openagents/sim/simhost.sock`).
  - Every `accept()` call verifies `getpeereid(client, &euid, &egid) == 0 && euid == geteuid()` on macOS (and `SO_PEERCRED` `cred.uid == geteuid()` on Linux; named pipe ACL restricted to current user SID on Windows). Unmatched peers are closed immediately without reading bytes.
- **Command-Table Version Pinning (`gb_row_digest`):**
  - Every `--attach` handshake verifies `X-GameBuilder-Table` / `gb_row_digest` (`gamebuilder/core/attach.coax`). A stale external `gamebuilder` binary whose command table hash disagrees with the running desktop app receives a structured `bad-request` upgrade error rather than silently misinterpreting command arguments.

### 9.3 Approval Levels, Target Leases & Human Supremacy

1. **Four Approval Tiers (`GB_APPROVALS` in `gamebuilder/core/commands.coax`):**
   - `none`: read-only queries (`list-levels`, `read-state`, `pictures`, `play-test`) run automatically.
   - `owner`: level/game mutations (`add`, `apply-edits`, `set-field`, `delete`) require `ed_may_edit` ownership and respect active `claims.coax` locks (other agents are rejected with `claimed`; **human edits ALWAYS succeed**).
   - `confirm`: destructive or public actions (`delete-game`, `delete-level`, `publish`, `unpublish`, `remove-published`, `set-owners`) surface an interactive Desktop confirmation card (`page/permit.coax` / `CoderDialog`) before executing when invoked by an agent.
   - `person`: account/spend/social identity actions (`login`, `logout`, `lobby-say`, `block`, `report`, `decide` spend past budget) require explicit human trigger and are refused for autonomous background agents.
2. **Non-Overlapping `TargetBinding` Leases (`coax/lib/agent/desktop/agents-catalog.coax`):**
   - Before spawning an autonomous GameBuilder or Playtester agent from `AgentsPage.swift` or the in-pane Agents drawer, the host checks `#authority` `ar_bindings_conflict` across active `TargetBinding` leases (`game_id + level_id`) unless both agents coordinate explicitly through non-overlapping `claims.coax` regions.
3. **One-Click Human Interrupt & Rollback:**
   - Whenever an attached or drawer agent is mutating the active tab, Row 2 displays a pulsing badge `"Agent editing…"` with a one-click **`Stop`** button (cancels worker / revokes session lease) and **`Undo Agent Turn`** (`undo-turn` command).

### 9.4 Bounded Untrusted Input & Untrusted Realm Confinement

- **Hard Resource Ceilings on All Local 3D Imports (`#authority` checks before GPU allocation):**
  - Max file size: `128 MiB` (`.glb`/`.pack`), `256 KiB` (`.world`/`.game`), `16 MiB` (`.ple-scene.json`/`.ifc`/`.obj`/`.stl`).
  - Max geometry: `2,000,000` vertices, `6,000,000` indices, `1,024` nodes, `128` materials, `64` textures (≤ `2048×2048` each), `1,048,576` Gaussian splats.
  - **Zero Network Fetch on Local Model Open:** `.gltf` external `uri` fields MUST be relative paths resolving strictly inside the `.gltf` file's parent directory (after symlink/canonicalization check); `http:`, `https:`, `data:` oversize, or `..` traversal are rejected deterministically.
  - **Asset Licence Enforcement:** Importing any asset into a GameBuilder/Metaverse VPK pack enforces `world.assets.v1` closed-enum `PIP_LICENCE` (`"CC0 1.0 Universal"`, `"CC BY 4.0"`, `"CC BY-SA 4.0"`) and SHA-256 verification.
- **Untrusted / Remote Authored Realms (`components/rendering.md`):**
  - First-party and locally owned worlds/models run natively in `AEPageHost v2` / `vk-realm v2`.
  - Untrusted third-party remote realms NEVER execute untrusted Wasm/shaders inside OpenAgents Desktop's main Metal/Vulkan process: they either render via bounded server-side `PreviewFrame` images (`openagents.verse.preview.v1alpha1`, ≤ `2048×2048`, ≤ `512 KiB`, ≤ `1 fps`, `8 MiB/min` budget) or inside a sandboxed cross-origin `WKWebView` / helper process speaking strictly `realm-frame.v1` (`I=1`, `O=1`, opaque origin).

---

## 10. File-by-File Implementation Map

### New Files

| Path | Context (`contexts.txt`) | Purpose |
|---|---|---|
| `coax/lib/agent/desktop/metaverse-route.coax` | `coder` (`#authority`) | Canonical 3D file/URI/MIME router (`mv_route_classify`, `mv_route_is_3d`, flag gating) shared by macOS/Linux/Windows. |
| `apps/openagents-desktop/page/metaversepane.coax` | `coder` | Expandable right-side Metaverse Viewer & GameBuilder Coax pane (Row 1 tabs/badges/`+`/`⛶`/`◫`, Row 2 breadcrumbs/modes/revision pill, Row 3 Build/Play/Read-only sub-bars, Studio/Inspector drawer, `CpMetaversePane` state, C exports). |
| `apps/openagents-desktop/page/checks/metaversepane-checks.coax` | `coder` | Coax `#test` suite covering split/expanded/narrow geometry, tab lifecycle, `closed_by_person` auto-open, `Esc` unwind, `TK_WORLD` parsing, revision truthfulness states, and `pane_color_*` exemption. |
| `apps/openagents-desktop/mv_host.h` | C header | Narrow C-ABI header for `metaverse-viewer.o` (`oa_mv_*`). |
| `apps/openagents-desktop/gb_host.h` | C header | Narrow C-ABI header for `gamebuilder-core.o` (`oa_gb_*`). |
| `apps/openagents-desktop/Window+Metaverse.swift` | macOS Host | AppKit/Metal integration for `CP_ASK_MV_*` & `CP_ASK_GB_*`, `MetaverseViewportView`, `cd_over_rects` hole sync, file watching, ModelIO `.usdz` bridge, Snapshot-to-Chat, `~/.openagents/gamebuilder/attach.sock` server. |
| `apps/openagents-desktop/MetaverseViewportView.swift` | macOS Host | Unified `CAMetalLayer` / `AEPageHost v2` viewport view with `setHoles(_:)`, orbit/fly/gizmo/Play input forwarding, shared `MTLDevice` & `AEGpuBudget`, and headless PNG readback (`--shot-metaverse`). |
| `metaverse-engine/content/model-view.coax` | `engine` | Bounded standalone 3D model inspector for textured `.glb`/`.gltf`, `.obj`+`.mtl`, `.stl`, `.gbm`, VPK1/2/3, and `RBA1` assemblies. |
| `metaverse-engine/content/desktop-viewer-bridge.coax` | `engine` | Relocatable library entry (`metaverse-viewer.o`) exposing `oa_mv_*` over `model-view.coax`, `gltf-scene.coax`, `assembly.coax`, `ple-scene.coax`, `ple-ifc.coax`, `world-follow.coax`. |
| `metaverse-engine/gpu/model-view-shader.coax` | `engine` | Single-source Coax PBR/Clay/Normals/Wireframe/Grid/Slice shader lowering cleanly to `coax metal`, `coax spirv`, and `coax wgsl`. |
| `metaverse-engine/gpu/ple-shader.coax` | `engine` | Single-source Coax instanced box/cylinder/clash/diff shader replacing inline MSL in `SceneView.swift`. |
| `metaverse-engine/host/aethyr-host-v2.h` | `engine` | Context-carrying C import/export ABI (`AethyrHostCtx*`) eliminating `phCurrent` / `activeRealm` / `static H`. |
| `gamebuilder/core/desktop-bridge.coax` | `gamebuilder` | Relocatable library entry (`gamebuilder-core.o`) exposing `oa_gb_*` strictly through `ed_command` / `GB_COMMANDS` / `GB_CONTROLS` / `attach.coax`. |
| `gamebuilder/core/gpu/gb-instances.coax` | `gamebuilder` | Single-source Coax shader for GameBuilder's instance-stream box/mesh/`.gbm` pipeline (replaces deleted Swift `Shaders.metal`). |
| `proto/desktop/metaverse-route-v1.json` | `protocol` | JSON Schema for `desktop.metaverse.route.v1`. |
| `proto/desktop/metaverse-selection-v1.json` | `protocol` | JSON Schema for `desktop.metaverse.selection.v1` (viewport-to-chat selection chips). |
| `proto/desktop/metaverse-tab-state-v1.json` | `protocol` | JSON Schema for `desktop.metaverse.tab-state.v1`. |
| `proto/verse/aethyr-host-v2.json` | `protocol` | Contract for context-carrying multi-instance native host ABI. |
| `proto/components/composition-v2.json` | `protocol` | Phase 4 external scene texture composition contract (`PRIM_EXT_TEXTURE`). |

### Modified Existing Files

| Path | Key Changes |
|---|---|
| `coax/lib/agent/desktop/flags.coax` | Add `DC_FLAG_METAVERSE :: 28`, `DC_FLAG_GAMEBUILDER :: 29`, bump `DC_FLAG_COUNT :: 30`, update `#test` assertions. |
| `coax/lib/agent/desktop/agents-catalog.coax` | Point `"game-builder"` `ArDefRecord` to `coax-metaverse:gamebuilder-core`, `world.edit.v1`, `gamebuilder.level.v1`. |
| `apps/openagents-desktop/page.coax` & `app.coax` | `#load "page/metaversepane.coax"` in full builds; wire `cp_mv_*` into `cp_init`, `cp_press`, `cp_key`, `cp_scroll_under`. |
| `apps/openagents-desktop/page/v1-stubs.coax` | Add zero-cost `--minimal` stubs for `CpMetaversePane`, `cp_mv_*`, `coder_page_mv_*`, `coder_page_gb_*`. |
| `apps/openagents-desktop/page/state.coax` | Embed `mv: [1] CpMetaversePane` in `CoderPage`; generalize `fp_color_*` → `pane_color_*`. |
| `apps/openagents-desktop/page/layout/layout.coax` | Add `cp_mv_shown(s)` to `cp_side_pane`, `pane_any`, and the user-expanded `expanded` full-main-area branch. |
| `apps/openagents-desktop/page/layout/conversation.coax` | Add Metaverse/3D corner toggle button in `cp_corners`; add `TK_WORLD` card rendering in `cp_act_row` / `cp_row_act`. |
| `apps/openagents-desktop/page/shows.coax` | Generalize `cp_show_scene` → `mv_route_is_3d`; support `OA-SHOW-KIND:` and `OA-SHOW-REV:`; deduplicate inline live card when same target is open in right pane. |
| `apps/openagents-desktop/page/input.coax` | Update `cp_edge`, `cp_press`, `cp_scroll_under`, `cp_key`, and `Esc` unwind stack for `CP_KB_MV` / `CP_KB_GB_PANEL` / `CP_SCR_MV_*`. |
| `apps/openagents-desktop/page/palette.coax` | Add `CP_PC_METAVERSE..CP_PC_GB_PLAYTEST` (`39..45`). |
| `apps/openagents-desktop/page/filetree.coax` & `filepane.coax` | Route 3D file clicks from File Explorer to Metaverse pane when flagged on; retain *"Open as Text"* fallback; retire `CP_FP_SCENE` once Phase 1 migration completes. |
| `apps/openagents-desktop/host.h` & `host.c` | Expose `cd_mv_*` and `cd_gb_*` C bridge functions. |
| `apps/openagents-desktop/Window.swift`, `Window+Files.swift`, `Window+Menus.swift`, `Window+Palette.swift`, `AppDelegate.swift`, `Activation.swift`, `Info.plist` | Route 3D files & `openagents://metaverse` / `openagents://gamebuilder` URLs to `Window+Metaverse.swift`; add menu items, drag-drop routing, and UTTypes; sync `MetaverseViewportView` frame + holes every frame. |
| `apps/openagents-desktop/AgentsPage.swift` | Replace PLE fixture launcher in `launchGameBuilder` with live GameBuilder tab + `TargetBinding` lease. |
| `apps/openagents-desktop/build.sh` | Under `--full`: compile `metaverse-viewer.o`, `gamebuilder-core.o`, emit `.metallib` shaders, bundle `Contents/Resources/gamebuilder`; under `--minimal`: strip cleanly. |
| `metaverse-engine/host/apple/PageHost.m` | Remove `phCurrent`, static `pack`, static counters; adopt `aethyr-host-v2`, shared `MTLDevice`/`RenderRegistry`, device-loss recovery, and missing `ENGINE_FP_HOST_*` passes. |
| `metaverse/gpu/vk-realm.c` & `apps/openagents-desktop/host-main.c` | Phase 4: remove global `static RealmHost H`; wire Linux Wayland & Windows Win32 Vulkan Metaverse/GameBuilder viewport. |
| `coder/tui/host-runs.coax`, `claudeworker.coax`, `mcworker.coax`, `inky.coax` | Auto-register bundled `gamebuilder mcp --attach` when `DC_FLAG_GAMEBUILDER` is enabled. |
| `proto/registry.json` | Register the 5 new schema rows (§8.6). |

---

## 11. Phased Delivery Plan, Verification Matrix & Risk Register

### 11.1 Phased Delivery Plan (Strict Vertical Slices)

Each phase ships behind feature flags (`DC_FLAG_METAVERSE`, `DC_FLAG_GAMEBUILDER`), passes `coax/web/tool.sh layer-check`, keeps `build.sh --minimal` green, and lands with automated tests + headless screenshots before the next phase starts.

#### Phase 0 — Foundations, Flags, Context-Safe Build & `TK_WORLD` Cards
- **Scope:**
  - Add `DC_FLAG_METAVERSE (28)` & `DC_FLAG_GAMEBUILDER (29)` in `flags.coax` + `page/v1-stubs.coax`.
  - Land `coax/lib/agent/desktop/metaverse-route.coax` (`#authority`) + `proto/desktop/metaverse-route-v1.json`.
  - Bundle `Contents/Resources/gamebuilder` in `build.sh --full` and auto-register `gamebuilder mcp --attach` in daemon worker launches.
  - Land typed `TK_WORLD` transcript cards in `conversation.coax` showing `gamebuilder` command summaries + before/after JPEG pictures (`pictures.coax`).
- **Done Criteria:** `build.sh --minimal` unchanged; `build.sh --full` bundles `gamebuilder`; running `gamebuilder add ...` from chat renders a clean `TK_WORLD` card with before/after thumbnails; `layer-check` passes with zero cycles.

#### Phase 1 — Expandable Right-Side Metaverse Viewer Pane & Universal 3D Model Inspector
- **Scope:**
  - Land `page/metaversepane.coax` (Row 1 tabs/badges/`+`/`⛶`/`◫`, Row 2 breadcrumbs/camera/shading/Snapshot-to-Chat, collapsible Tree/Inspect drawer, split/expanded/narrow layout, `pane_color_*` RGB exemption, corner toggle, Command Palette, menus, drag-and-drop, `Activation.swift`, `Info.plist`).
  - Land `metaverse-engine/content/model-view.coax`, `desktop-viewer-bridge.coax`, `model-view-shader.coax`, `ple-shader.coax` (`metaverse-viewer.o`), and `Window+Metaverse.swift` / `MetaverseViewportView.swift`.
  - Support `.glb`, `.gltf`, `.obj`, `.stl`, `.usdz` (ModelIO adapter), `.gbm`, `.pack` (VPK1/2/3), `.ple-scene.json`, and native Coax `.ifc` (`ple-ifc.coax`, retiring Python `ple-ifc-to-scene.py` and standalone `PleSceneMetalView`).
  - Generalize `OA-SHOW` auto-open & `closed_by_person` etiquette.
- **Done Criteria:** Clicking/dropping any `.glb`, `.obj`, `.stl`, `.usdz`, `.ple-scene.json`, or `.ifc` auto-opens the right-side Metaverse pane; `⛶` Expand fills `window_w - left`; `◫` Hide preserves tabs & drops GPU to `0 Hz`; menus/Command Palette cut out cleanly via `cd_over_rects`; Snapshot-to-Chat attaches PNG + selection JSON; headless `--shot-metaverse` matches golden images.

#### Phase 2 — Multi-Instance Context Audit (`aethyr-host-v2`) & Rich Scene / Realm / Assembly Views
- **Scope:**
  - Land `aethyr-host-v2.h` and refactor `metaverse-engine/host/apple/PageHost.m` (delete `phCurrent` & static `pack`, adopt `RenderRegistry` + shared `AEGpuBudget`, consolidate `MetalRealm.m` recovery, wire missing `ENGINE_FP_HOST_*` passes).
  - Enable `RBA1` rigid assembly / Microduck joint/explode/servo inspection, animated `gltf-scene.coax` clip scrubbing, `anim.scene.v1`, `.aimport` Unreal bundles, and live `world-follow.coax` realms with explicit revision-truthfulness pills (`CP_MV_REV_*`).
  - Enable multi-view coexistence + deduplication between right-pane Metaverse tab and inline `OA-SHOW-LIVE` cards.
- **Done Criteria:** Two live 3D views (e.g., right-pane `.glb` + inline `OA-SHOW-LIVE` card, or two active cached tabs) run concurrently with zero cross-talk, verified under `RenderRegistry` VRAM cap; inline card deduplicates cleanly when opened into the right pane.

#### Phase 3 — Full GameBuilder Studio Inside the Pane & Live Chat Round-Trip
- **Scope:**
  - Land `gamebuilder/core/desktop-bridge.coax` + `gamebuilder/core/gpu/gb-instances.coax` (`gamebuilder-core.o`) + `gb_host.h`.
  - Wire all 3 GameBuilder render pipelines (instance-stream `.gbm`, SPLICEBOTS `farm`/`model`/`rave`, and `world.coax` → `web/hud.coax` + all 5 standalone worlds `fated-dragon`/`splicebots`/`whispering-spring`/`scrappers-arena`/`ice-cream-man`).
  - Wire `Inspect | Build | Play` modes, Row 3 Build toolbar (`GB_CONTROLS`), `panel.coax` Palette & 11-field Inspector, `picker.coax` 12-type New Game grid, expanded Hotbar, live 60/120 Hz Play (Scrappers 4 modes + 9 web worlds + CoreAudio `host-pcm.coax`), deterministic `play-test` with stuck-bot pins, and graceful on-demand `assets-sync --group gamebuilder` hydration UI.
  - Bind `~/.openagents/gamebuilder/attach.sock` (`0600`, peer UID verified, `gb_row_digest`) so Chat/MCP agents edit the live open level with `changes.coax` glow + `Show me`/`Dismiss` and bidirectional `desktop.metaverse.selection.v1` / `world.turn.v1` viewport-to-chat chips.
  - Upgrade `AgentsPage.swift` & `agents-catalog.coax` to launch the real GameBuilder pane with `TargetBinding` leases.
- **Done Criteria:** User can create any of the 12 game types, edit with Q/W/E/R + Palette + Inspector, expand `⛶` and play Scrappers or Village at 60/120 Hz, ask Desktop chat to add/modify entities and watch them appear live with before/after `TK_WORLD` cards, click an entity to reference it in chat, and run `play-test` to jump to stuck bots. `gamebuilder/scripts/check.sh` and `commands-check.coax` pass 100%.

#### Phase 4 — Multi-Agent Studio, Publish/Lobby, `composition-v2` & Linux/Windows Vulkan Parity
- **Scope:**
  - Wire remaining GameBuilder Studio drawer tabs: `Agents` (`agents.coax`, Builder/Playtester/Code agents, `claims.coax`, `agent-replay.coax` timeline scrubber, `decide.coax`), `Share` (`publish.coax` versions/rollback/feedback/moderation, `sync.coax`), and `Lobby` (`lobby.coax`/`party.coax`/`net.coax` Nostr relay & multiplayer Scrappers).
  - Land `proto/components/composition-v2.json` (`PRIM_EXT_TEXTURE`) in `gpu-composition.coax` and `ui-host.m` / `vulkan-ui.c` so 3D scenes composite directly into `UcPlan` (retiring `CAShapeLayer` hole masks and unlocking frosted glass overlays over 3D).
  - Refactor `metaverse/gpu/vk-realm.c` (remove global `static RealmHost H`) and wire identical Metaverse Viewer & GameBuilder pane into Linux Wayland (`host-main.c`) and Windows Win32 (`apps/shared/host/windows/`).
  - Add native Metal/Vulkan compute radix sort for `.splat` / 3DGS `.ply` captures.
- **Done Criteria:** Feature parity across macOS Metal, Linux Wayland/Vulkan, and Windows Win32/Vulkan; zero `CAShapeLayer` hole masks needed; all Coax shaders compile cleanly via `coax metal`, `coax spirv`, and `coax wgsl`.

#### Phase 5 — Flag Graduation & Release Promotion
- Evaluate crash-free telemetry, VRAM/thermal metrics, and `--minimal` binary impact before proposing any flag promotion from `default_on = false` to `true` in a post-v1.0 release.

---

### 11.2 Verification & Test Plan

| Layer | Suite / Command | What It Verifies |
|---|---|---|
| **Coax Unit & Layout Tests** | `coax check apps/openagents-desktop/page/checks/metaversepane-checks.coax` | Split/Expanded/Narrow pane rects; Row 1/2/3 geometry; 12-tab open/switch/close/dirty prompts; `closed_by_person` auto-open rules; `Esc` unwind stack; `CP_KB_MV` vs `CP_KB_COMPOSER` isolation; revision truthfulness states (`CP_MV_REV_*`); `pane_color_*` ranges; `TK_WORLD` parser. |
| **Authority & Policy Tests** | `coax check coax/lib/agent/desktop/flags.coax coax/lib/agent/desktop/metaverse-route.coax` + mutation check | All 30 flags sequential/default-off; every extension/URI routes deterministically; path traversal (`..`, `http://`) hard-rejected; non-overlapping `TargetBinding` lease conflict detection. |
| **GameBuilder Drift & Golden Tests** | `gamebuilder/scripts/check.sh` (`commands-check`, `commands-golden`, `docs-rows`, `attach-check`) | All 120 `GB_COMMANDS` and 73 `GB_CONTROLS` remain synchronized across Desktop bridge, CLI, MCP, HTTP, and docs; zero silent plumbing exports. |
| **Shader Triple-Target CI** | `coax metal`, `coax spirv`, `coax wgsl` on `model-view-shader.coax`, `ple-shader.coax`, `gb-instances.coax`, `web/gpu/hud.coax` | Guarantees cross-platform shader compilation without third-party compilers or SPIR-V regressions. |
| **Bounded Loader Fuzz / Limits** | `coax check metaverse-engine/content/model-view.coax` | Truncated GLB, oversized vertex/index counts, malformed OBJ/STL, cyclic node graphs, external URI injection, and out-of-range VPK offsets all fail cleanly with descriptive errors and zero traps. |
| **Headless Visual Regression** | `OpenAgents --shot-metaverse fixtures/duck.glb --out ...` & `--shot-gamebuilder scrappers-arena --expand --out ...` | Offscreen Metal/Vulkan rendering + composited Coax chrome (following `docs/desktop/macos-drive-and-screenshots.md`) verifying split view, expanded view, overlay menu over 3D viewport, Build gizmos, and dark/light themes. |
| **Live Integration Smoke** | End-to-end test: launch Desktop → `gamebuilder --attach add block` → verify `cd_mv_tab_status` rev bump + `TK_WORLD` card + `undo-turn` | Confirms socket auth (`0600` + UID), `gb_row_digest`, live viewport refresh, `changes.coax` mark, and rollback end-to-end. |
| **Minimal Build Gate** | `OA_RELEASE_MINIMAL=1 apps/openagents-desktop/build.sh` | Verifies v1.0 `--minimal` build links cleanly via `page/v1-stubs.coax` with zero Metaverse/GameBuilder symbols or bundled resources. |
| **Bounded Context Gate** | `coax/web/tool.sh layer-check` | Verifies zero cyclic `#load`s between `coder`, `gamebuilder`, `engine`, and `components`. |

---

### 11.3 Failure Modes & Recovery Matrix

| Failure Mode | User-Visible Behavior | Recovery |
|---|---|---|
| **Corrupt / Oversized 3D File** | Tab stays open with a clean Coax error state inside the viewport body naming exact reason (e.g., `"mesh exceeds 2M vertex limit"` or `"external URI rejected"`) + *"Open in External App"* / *"Open as Text"* buttons. | Never crashes host; frees partial allocations immediately. |
| **Missing Hydrated Pack (`scrappers.gbm` / `village.pack` / `splicebots`)** | Coax hydration card inside viewport showing required pack size & SHA-256 verification status with **`Download Pack`** button (`CP_ASK_MV_HYDRATE`). | Runs `assets-sync --group gamebuilder`; auto-loads tab on completion. |
| **GPU Device Reset / Allocation Refused (`RenderRegistry`)** | Evicts inactive background tabs first; if still over budget or device lost, shows `"3D viewport paused (GPU memory/reset)"` with one-click **`Reload Viewport`** (using `AEPageHost v2` recovery from `MetalRealm.m`). | Chat window and unsaved GameBuilder level data remain intact on CPU; zero data loss. |
| **Stale Revision / Realm Offline** | Row 2 pill switches explicitly to `saved rev N · preview unavailable` or `offline · showing rev M` (Invariant I-4). | `↻` Reload or `world-follow.coax` automatic reconnect + `409` reopen. |
| **Attach Socket Disconnected / Version Mismatch** | `gamebuilder --attach` prints actionable error (or falls back to `--dir` in headless daemon mode); Desktop Row 2 drops the attached-agent indicator. | Re-attaching reconnects cleanly; no orphaned locks (`claims.coax` leases expire on disconnect). |
| **Dirty GameBuilder Level on Tab Close / Quit** | Prompts standard `CoderDialog` (`Don't Save` / `Cancel` / `Save`) matching `Window+Files.swift:askToSave`. | Never discards unsaved level edits silently. |

---

### 11.4 Open Design Decisions (With Recommended Defaults)

1. **Unified Right Pane vs Separate Corner Toggle for File Viewer and Metaverse Viewer:**
   - *Recommendation (adopted in §3.5):* Keep **two dedicated panes** (`page/filepane.coax` for code/text/docs and `page/metaversepane.coax` for 3D models/scenes/realms/GameBuilder) sharing the exact same right-side slot, two-row chrome language, and `⛶` Expand / `◫` Hide behavior, with instant 1-click front-most switching via their adjacent corner buttons. *Why:* Prevents 3D tabs from crowding open code tabs while keeping `filepane.coax` lean and untouched for `--minimal` v1.0 builds.
2. **Default Auto-Open Behavior When Agents Generate 3D Files:**
   - *Recommendation (adopted in §4.2):* Auto-open in **split mode** (`520pt`) unless the user clicked `◫` Hide (`closed_by_person == true`), and never auto-expand (`⛶`) or steal keyboard focus from the composer.
3. **GameBuilder Data Storage Directory:**
   - *Recommendation:* Share `~/.openagents/gamebuilder/` (matching `gamebuilder/cli/main.coax` default data directory) for user games/levels/login/attach socket so Desktop pane, `gamebuilder` CLI, and daemon MCP sessions operate on the exact same local library with zero sync mismatch.
4. **Expanded Hotbar in Build Mode:**
   - *Recommendation:* Show the 9-slot bottom quick-place Hotbar by default only when `mv.expanded == true` and `mode == Build` (toggleable in `⋮` More), keeping split mode clean and uncluttered.
