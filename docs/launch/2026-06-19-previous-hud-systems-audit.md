# Previous HUD / Pane / Hotbar / Game-UI Systems Audit

**Date:** 2026-06-19
**Repo:** `OpenAgentsInc/openagents` (worktree off `origin/main`)
**Purpose:** Inform the current Autopilot desktop UI rebuild. The owner has
zero-based the `apps/autopilot-desktop` surface to a black screen + text bar and
wants to rebuild the HUD drawing on OpenAgents' prior pane/hotbar/HUD systems.
This audit catalogs every prior system across repos and history, extracts the
reusable design patterns, and recommends what to revive vs. leave behind.

> Scope honesty up front: this is a documentation audit. No external repo code
> was vendored into this repo. Repos were cloned to a throwaway `/tmp/hud-audit/`
> dir only to read them. Several private repos were not clonable in this session
> (noted in §6).

---

## 1. Executive summary

OpenAgents has built **at least five distinct generations** of a "draggable
panes + hotbar + sci-fi HUD" UI, in two language families, over ~2 years:

| # | System | Repo | Lang | Era | What it did | Status |
|---|--------|------|------|-----|-------------|--------|
| 1 | **AutoDev HUD / pane store** | `openagents` (Laravel/Inertia web app, pre-monorepo) | TS/React + Laravel | Jul–Aug 2024 (ep. 103–119) | First draggable/resizable "HUD panes": chat panes, diff panes, multiplayer; `store/HUD.ts` pane model; "we're building an operating system" | Pruned (web app retired) |
| 2 | **Commander pane/hotbar/hand-tracking app** | `OpenAgentsInc/commander` (public) + `commander-prev`/`commander-old` | TS/React, Electron (electron-forge), Zustand, R3F | May–Jun 2025 (ep. 170, 178) | StarCraft-style HUD: full pane manager, bottom hotbar with numbered slots + Cmd-K, MediaPipe **hand-tracking pinch-to-drag panes**, NIP-90 compute/wallet/coder panes | Archived (public, last commit 2025-06-02) |
| 3 | **`v4` desktop** | `OpenAgentsInc/v4` (private) + `v4-prev` | TS/React, Next.js + Convex | Mar–May 2025 | The "little earlier version with some panes you could move around" (ep. 170, 00:42) — predecessor experiment to Commander | Archived (private) |
| 4 | **WGPUI + `hud` crate (Rust GPU HUD)** | `openagents` Rust era (now in `backroom/archive/openagents/wgpui/`) | **Rust** (wgpu, cosmic-text, Taffy) | ~Feb–Apr 2026 (ep. 199–214) | "Own the pixel" GPU UI lib (`wgpui`) + an Arwes-style sci-fi `hud` widget crate (frames, backgrounds, meters, status lights, command palette, hotbar, draggable panes, headless screenshot capture) | Pruned from active tree → archived in `backroom`; some pieces lived in active `openagents` history through Apr 2026 |
| 5 | **Autopilot desktop Foldkit nav shell** (CURRENT, pre-zero-base) | `openagents` `apps/autopilot-desktop` | TS, Bun/Electrobun + Effect + Foldkit | May–Jun 2026 (ep. 228+) | Cmd-K command palette + grouped nav + keyboard layer + pane registry (`nav.ts`/`keyboard.ts`); composer/chat/sessions/network/training panes | The surface being rebuilt now |

The recurring, durable idea across all five: **a screen of movable/selectable
"panes," a bottom or side launcher, a keyboard/command layer, and a sci-fi HUD
aesthetic** — explicitly framed as "an agent operating system" (ep. 117, 119)
and a "StarCraft-style HUD" for "commanding groups of agents" (ep. 170, 199).

**Top reusable patterns** (detail in §4):
- Pane model as data: `{id, type, x, y, w, h, isActive, content}` + a store of
  open panes + a typed `PaneManager` switch on `type`. (Systems 1, 2, 5)
- Bottom **hotbar** with numbered slots (1–9) mapped to hotkeys, plus a Cmd-K
  command palette slot. (Systems 2, 4)
- The **nav-registry seam** (System 5): a single registry drives sidebar +
  palette + keyboard so new panes plug in with one entry — already the right
  abstraction; keep it.
- WGPUI's Arwes-style HUD component vocabulary (frames, corners, status lights,
  meters, text-decipher) for the sci-fi look. (System 4)
- Hand-tracking pinch-to-drag as a "wow" layer, not the primary input. (System 2)

---

## 2. Method & date→repo mapping

Sources used:
1. **`docs/transcripts/`** — episode theme guide (`README.md`) + grep for
   `hud|hand-track|hotbar|pane|wgpui|gesture|command palette|overlay`. Hits:
   `038, 117, 119, 123, 127, 131, 170, 174, 176, 177, 179, 185, 189, 199, 200,
   206, 208, 214, 228`. Transcript headers carry `Upload date:` which anchors
   each system to a repo era.
2. **`backroom/`** (`/Users/christopherdavid/work/backroom/`) — the pruned-code
   archive. The Rust WGPUI + `hud` crates live at
   `backroom/archive/openagents/wgpui/`.
3. **This repo** — `apps/autopilot-desktop/src/ui/` (current shell),
   `git log --all` for `wgpui|hotbar|pane`.
4. **External repos** cloned read-only to `/tmp/hud-audit/`: `commander`,
   `v4`, `v4-prev`, `commander-prev`, `commander-old`, `ruinsofatlantis`.

**Date → repo/system map** (transcript `Upload date` + commit dates):

| Episode(s) | Date | Maps to system |
|---|---|---|
| 103–117 | Jun–Aug 2024 | System 1 — AutoDev HUD panes in the Laravel web app ("extend our HUD UI to show diffs", ep.117) |
| 119 | 2024-08-12 | System 1 — public launch: "we got panes, HUD panes. All of them are draggable, resizable… open multiple panes" |
| 170 | 2025-05-06 | System 2 — Commander reveal: "StarCraft HUD and key bindings for groups of agents"; "earlier version with some panes" = System 3 (`v4`) |
| 178 | 2025-05-24 | System 2 — hand-tracking demo ("pinch one of the panes and move it") |
| 199–200 | 2025-12-23 | System 4-era framing — "StarCraft style HUD UI… give you a cool sci-fi HUD" (Autopilot foundations) |
| 214 | 2026 | "pull that [hand tracking] back into the current app… pinch a pane and move it" — explicit intent to revive System 2's gesture layer |
| 228+ | 2026-05/06 | System 5 — current Foldkit `apps/autopilot-desktop` |

---

## 3. Per-system detail

### System 1 — AutoDev HUD panes (Laravel web app, TS/React, 2024)

- **Where:** the original `openagents.com` Laravel/Inertia web app (pre-Rust,
  pre-monorepo). Code is not in the current tree; lives only in transcript
  evidence + the old web-app history.
- **What it did** (ep. 117, 119, transcripts are quote-grade-adjacent):
  - A `store/HUD.ts` (zustand-style) held the list of open panes and pane
    geometry; `Pane.tsx` rendered a draggable/resizable window with a title
    bar + close (X) button; `HUD.tsx` rendered all panes from the store.
  - Pane **types**: `chat`, `diff` (React diff viewer), with the explicit
    pattern "register a new pane `type`, render it in `HUD.tsx`."
  - Cmd/Ctrl-click to **open multiple panes** (multiple simultaneous chats);
    panes draggable + resizable so users "configure your chat workspace."
  - Initial pane geometry templated off a "chat pane style initial" definition
    (default size, ~90% max height, constrained width).
- **Framing:** ep.117/119 is where "we're building an operating system, an
  **agent operating system**" is first said out loud, prompted directly by the
  pane/HUD UI.
- **Status:** pruned with the Laravel web app. Pattern survived into System 2.

### System 2 — Commander (TS/React + Electron, 2025) — the richest prior HUD

- **Where:** `OpenAgentsInc/commander` (public, last commit 2025-06-02). Cloned
  read-only to `/tmp/hud-audit/commander`. Introduced ep.170; hand-tracking
  shown ep.178. Predecessors: `commander-prev`, `commander-old` (Turbo
  monorepos), `commander-no` (private).
- **Stack:** Electron (electron-forge), React, Zustand, `@use-gesture/react`,
  `@react-three/fiber` + `drei` + `rapier`, MediaPipe (`@mediapipe/hands`,
  `camera_utils`), Effect, Vite.
- **Pane system** (`src/types/pane.ts`, `src/stores/panes/`):
  - `Pane = {id, type, title, x, y, width, height, isActive?, dismissable?,
    headerMenus?, content?}`. ~25 pane **types**: `chat`, `chats`, `diff`,
    `nip28_channel`, `nip90_dashboard`, `sell_compute`, `dvm_job_history`,
    `nip90_*`, `wallet`, `agent_chat`, `coder`, swebench panes, etc.
  - Store split into **one file per action**: `addPane`, `removePane`,
    `bringPaneToFront`, `setActivePane`, `updatePanePosition`,
    `updatePaneSize`, `togglePane`, plus ~20 `open<Thing>Pane` openers
    (`openChatPane`, `openWalletPane`, `openSellComputePane`, …). Utils:
    `calculatePanePosition` (cascade/offset placement),
    `ensurePaneIsVisible` (clamp into viewport).
  - `PaneManager.tsx` is a `switch` on `pane.type` → component;
    `Pane.tsx` owns chrome: title bar, drag, and **8-handle resize**
    (`topleft/top/topright/right/bottomright/bottom/bottomleft/left`) via
    `useDrag` from `@use-gesture/react`, using a `memo` ref for resize-start
    state. z-order via `bringPaneToFront`.
- **Hotbar** (`src/components/hud/Hotbar.tsx`, `HotbarItem.tsx`):
  - Fixed bottom-center bar (`fixed bottom-4 left-1/2 -translate-x-1/2`),
    rounded, `backdrop-blur`, high z-index. Numbered **slots** (`slotNumber`)
    each a toggle for a pane (Coder, Sell-Compute, Wallet, DVM history, Agent
    Chat, Previous Chats, SWE-bench, Hand-tracking). Each slot gated by a
    **feature flag** (`useFeatureFlag(Feature.CODER_PANE)` etc.).
- **Hand-tracking** (`src/components/hands/`): MediaPipe Hands landmarks →
  `handPoseRecognition.ts` classifies poses (`FIST`, `FLAT_HAND`, `OPEN_HAND`,
  `PINCH_CLOSED`); `HandTracking.tsx` + `useHandTracking.ts` stream landmarks;
  `HomePage.tsx` implements **pinch-to-drag**: on `PINCH_CLOSED`, hit-test from
  topmost pane down, set `draggingPaneId`, then `updatePanePosition` follows the
  `pinchMidpoint`. R3F scene (`ThreeScene`, `InteractiveHandScene`) renders the
  hand overlay. This is the "pinch a pane and move it" demo from ep.178/214.
- **Status:** archived public repo; the canonical, most complete prior HUD.

### System 3 — `v4` desktop (TS/React, Next.js + Convex, 2025)

- **Where:** `OpenAgentsInc/v4` (private; last commit 2025-05-28), `v4-prev`.
  Cloned read-only. This is the "little earlier version with some panes you
  could move around" referenced in ep.170 (00:42) — the immediate predecessor
  experiment before Commander. Next.js + Convex app shell; less mature pane
  system than Commander. Treated here as lineage context, not a primary source.
- **Status:** archived/private.

### System 4 — WGPUI + `hud` crate (Rust GPU HUD, 2026)

- **Where:** now archived at `backroom/archive/openagents/wgpui/` (two crates:
  `wgpui/` core + `hud/` widgets). Lived in active `openagents` Rust history;
  `git log --all` shows WGPUI HUD work through ~Apr 2026 (e.g. `de953e968
  Replace Nexus homepage with WGPUI HUD scene`, 2026-04-11) before the Rust
  codebase was deprecated (`8d375f2e5 chore: deprecate and remove Rust
  codebase`) and the repo rebuilt as a Bun/Effect workspace (`f5919c766`).
- **`wgpui` core** (`backroom/.../wgpui/wgpui/`): "GPU-accelerated UI rendering
  library for Rust. Own the pixel." wgpu (WebGPU/WebGL/Vulkan/Metal/DX12) +
  **cosmic-text** glyph atlas + **SDF** rounded-corner/border quads + **Taffy**
  (CSS flexbox) layout + a Bloomberg-inspired dark theme. Explicitly *not* a
  reactive framework — a primitive scene/renderer (`Scene`, `Quad`, `TextRun`).
  Docs: `ARCHITECTURE.md`, `TEXT.md`, `THEME.md`, `LAYOUT.md`, `RENDERING.md`.
- **`hud` crate** (`backroom/.../wgpui/hud/`): "Arwes-style sci-fi HUD
  components for WGPUI," white-on-black. Component families:
  - **Frames:** `FrameCorners` (brackets), `FrameLines`, `FrameOctagon`,
    `FrameCircle`, `FrameHeader`, `FrameUnderline`.
  - **Backgrounds:** `DotGridBackground`, `GridLinesBackground`,
    `MovingLinesBackground`.
  - **Text FX:** `TextSequence` (char reveal), `TextDecipher` (scramble).
  - **Effects:** `Illuminator` (mouse-follow glow).
  - **Interactive/forms:** `HudButton`, `TextInput`, `Checkbox`, `Toggle`,
    `Select`.
  - **Data:** `List`, `Table`, `CodeBlock`, `Card`.
  - **Nav:** `Tabs`, `Breadcrumbs`, `Nav`, `Pagination`.
  - **Feedback:** `Alert`, `Progress`, `Loading`, `Modal`, `Tooltip`.
  - **Indicators:** `StatusLight` (LED), `Meter` (gauge w/ thresholds),
    `Separator`, `Figure`.
  - **Animation:** `HudAnimator` state machine + `AnimatorManager`
    (parallel/stagger/sequence orchestration) + 30+ easing curves. Demo bin:
    `hud/src/bin/hud_demo.rs`.
- **Pane/hotbar/palette in WGPUI era** (from `git log --all` in this repo):
  draggable **hotbar** (`Add draggable hotbar handle with small triangle icon`,
  `fix/hotbar-drag-handle` #4023), **command palette as a hotbar slot**
  (`Add K hotbar slot to open command palette`, `Render K shortcut badge`,
  `keep command palette above all pane layers`), numbered hotkeys
  (`map hotkey 1 to codex`), pane splitting (`Mission Control pane split into
  focused hotbar panes` #3448), **headless WGPUI screenshot capture**
  (`Add headless WGPUI capture binaries`, `Add offscreen WGPUI render and
  readback`, `Add pane screenshot capture to desktop control`), and a
  `wgpui::viz` shared training-visualization layer.
- **Status:** archived in `backroom`; not in the active tree. The single
  richest *visual* reference for the sci-fi HUD look and the Rust "own the
  pixel" approach.

### System 5 — Current Autopilot desktop Foldkit nav shell (TS, 2026)

- **Where:** `openagents` `apps/autopilot-desktop/src/ui/` — Bun/Electrobun main
  process + webview, Effect + Foldkit, typed bun↔webview RPC. This is the
  surface the owner just zero-based.
- **What already exists** (the abstractions worth keeping even after the
  black-screen reset):
  - **Pane registry seam** (`nav.ts`): `NAV_GROUPS` (Chat, Code, Supervise,
    Explore, Settings) → each group has `destinations: NavDestination[]`
    (`{pane: PaneId, label, keywords}`). A **single registry** drives the
    grouped sidebar, the **Cmd-K palette** ("Go to…" + action commands), and
    j/k navigation. Adding a pane = add a `PaneId` literal + `paneView` case +
    one `NavDestination` entry. Central files (`model/update/view/message.ts`)
    know nothing about which panes exist.
  - **Keyboard layer** (`keyboard.ts`): pure, unit-testable. Raw `KeyEvent` →
    `KeyIntent` (`open-palette`, `palette-move`, `palette-run`,
    `navigate-group`, `navigate-pane`, `submit-turn`); the reducer re-dispatches
    an existing `Message` — shortcuts never invent a new control verb.
  - **Anti-clutter rule** (referenced as "audit §5.2" in `nav.ts`/`keyboard.ts`,
    documented in `docs/launch/2026-06-19-autopilot-desktop-coding-agent-audit.md`
    §5.2): a new system never gets a new top-level sidebar button — it joins a
    group's `destinations` or becomes a palette command; the primary sidebar
    stays ~5 groups.
  - Panes already built: `chat` (Blueprint), `composer` (the coding loop,
    #5355), `swarm`, `sessions`, `session-detail`, `spawn`, `decisions`,
    `autonomous-loop`, `nodes`/accounts, `network` (immersive Pylon scene),
    `training` + `training-fullscreen`, `builtin-agent`, `settings`,
    `onboarding`.
- **Status:** active; being rebuilt from a black screen + text bar.

---

## 4. Design patterns worth reusing

1. **Pane-as-data + typed PaneManager (Systems 1/2/5).** A pane is a plain
   record `{id, type, x, y, w, h, isActive, content}`; a store holds the open
   set; a single `switch(type)` renders the body. This has survived three
   rewrites — it is the durable core. System 5's closed-union `PaneId` + the
   registry seam is the cleanest expression of it.

2. **One-file-per-action store (System 2).** Commander's
   `stores/panes/actions/*` (one opener per pane type + a handful of geometry
   mutations) kept the pane store readable at ~25 types. Mirror this discipline,
   but route every opener through the System-5 registry rather than hand-wiring.

3. **Hotbar with numbered slots + a palette slot (Systems 2/4).** Bottom-center,
   blurred, high-z, numbered 1–9 hotkeys, each slot a pane toggle, with a
   dedicated **Cmd-K / K** slot for the command palette. WGPUI even made the
   hotbar itself draggable. This is the "StarCraft command bar" the owner keeps
   invoking (ep.170/199).

4. **Command palette + pure keyboard intent layer (System 5).** Already the
   right model: keyboard → intent → existing Message. Keep `keyboard.ts` pure
   and unit-tested; keep the palette reading from the same registry as the
   sidebar/hotbar so there is one source of truth.

5. **Arwes-style sci-fi HUD vocabulary (System 4).** Corner-bracket frames,
   dot/grid/moving-line backgrounds, `StatusLight`, `Meter`, `TextDecipher`,
   enter/exit animators with stagger orchestration, white-on-black. This is the
   look that made the old HUDs feel "futuristic"/"StarCraft." Port the
   *aesthetic and component taxonomy*, even though the Rust impl itself is
   archived.

6. **8-handle drag-resize via gesture lib (System 2).** `@use-gesture/react`
   `useDrag` per edge/corner with a memo-ref for resize-start state, plus
   `calculatePanePosition` (cascade) + `ensurePaneIsVisible` (clamp) — a
   complete, proven free-floating window model if free panes return.

7. **Headless WGPUI screenshot capture (System 4).** Offscreen render + readback
   + artifact manifests gave reviewable pane screenshots in CI/control. Valuable
   for launch-video/review artifacts regardless of renderer choice; the current
   `docs/wgpui/HEADLESS_CAPTURE.md` lineage (per workspace CLAUDE.md) tracks
   this.

8. **Feature-flagged slots (System 2).** Every hotbar slot / pane gated by a
   flag — let the HUD ship incrementally without clutter.

9. **Hand-tracking pinch-to-drag as a wow layer (System 2).** MediaPipe →
   pose classification → pinch hit-test → `updatePanePosition`. Ep.214 states
   explicit intent to revive it. Treat as an optional overlay, never primary
   input.

---

## 5. Visual / screenshot references

- **Commander demo GIF:** `/tmp/hud-audit/commander/images/demo.gif` (clone-only
  path; shows the live pane+hotbar+hand-tracking HUD). Re-clone from
  `OpenAgentsInc/commander` to view — not vendored here.
- **WGPUI HUD demo binary:** `backroom/archive/openagents/wgpui/hud/src/bin/hud_demo.rs`
  (run from a backroom checkout to see the Arwes-style components live).
- **WGPUI/HUD docs (visual specs):**
  `backroom/archive/openagents/wgpui/wgpui/docs/{ARCHITECTURE,TEXT,THEME,LAYOUT,RENDERING}.md`
  and `backroom/archive/openagents/wgpui/hud/docs/{README,getting-started,theming}.md`.
- No PNG/JPG screenshots of the old HUDs are checked into `backroom`'s wgpui
  archive (only source + docs). The current repo's headless-capture tooling is
  the path to regenerate pane screenshots if needed.
- **Transcript visual narration** (closest to "what it looked like"): ep.119
  (panes launch), ep.117 (HUD diff panes), ep.170 (Commander reveal), ep.178
  (hand-tracking pinch), ep.199–200 (sci-fi HUD framing).

---

## 6. Honest scope — what was accessible

- **Cloned & read (read-only, `/tmp/hud-audit/`):** `commander` ✅ (full pane
  store, hotbar, hands — the richest source), `v4` ✅, `v4-prev` ✅,
  `commander-prev` ✅, `commander-old` ✅, `ruinsofatlantis` ✅ (Rust game; not a
  HUD/pane source — skipped beyond confirming).
- **Archive read in place:** `backroom/archive/openagents/wgpui/` (Rust `wgpui`
  + `hud` crates) ✅.
- **In-repo read:** `apps/autopilot-desktop/src/ui/` (current shell) ✅;
  `git log --all` for HUD/WGPUI history ✅.
- **Not separately cloned (private, lower-value lineage):** `commander-no`,
  `v4a/v4c/v4t/v4tno`, the `v5*` series, `archon`, `dashboard`. These are
  earlier/parallel desktop experiments; Commander + the backroom WGPUI archive
  cover the design space they explored. Note these honestly as un-audited rather
  than assumed empty.
- **No external code was vendored into this repo.**

---

## 7. Recommendations for the current Autopilot rebuild

Starting point: black screen + text bar in `apps/autopilot-desktop`.

**Revive (high confidence):**
1. **Keep System 5's registry seam as the spine.** Do not rebuild navigation
   from scratch — `nav.ts`'s `NAV_GROUPS` + `keyboard.ts`'s pure intent layer is
   already the best version of the pattern OpenAgents has shipped. Build the HUD
   *on top of* it.
2. **Add a hotbar as a second launcher alongside the palette.** Bottom-center,
   numbered 1–9 → pane toggles, plus the existing Cmd-K. Drive the hotbar slots
   from the SAME registry as the sidebar/palette (one source of truth, per the
   §5.2 anti-clutter rule). This is the StarCraft "command bar" the owner keeps
   asking for (ep.170/199) and it grows naturally from the text-bar shell.
3. **Adopt the Arwes/WGPUI HUD vocabulary for the look.** Corner-bracket frames,
   dot-grid background, `StatusLight`/`Meter` for node/training state, subtle
   `TextDecipher`/enter animations. Per workspace UI guidance, build 3D/visual
   layers on `@openagentsinc/three-effect` first (the `network`/training scenes
   already do); reserve raw GPU work for genuinely perf-critical surfaces.
4. **Pane-as-data, typed switch.** Keep the closed `PaneId` union + `paneView`
   switch. If/when free-floating panes return, port Commander's
   `calculatePanePosition` (cascade) + `ensurePaneIsVisible` (clamp) +
   `@use-gesture/react` 8-handle resize wholesale — it is proven.

**Revive (medium confidence / later):**
5. **Hand-tracking pinch-to-drag** as an optional, flagged overlay (Commander's
   MediaPipe → pose → pinch hit-test pipeline). Ep.214 explicitly wants it back.
   It is a demo/wow feature, not the daily driver — gate it behind a flag.
6. **Headless pane screenshot capture** for launch-video/review artifacts (the
   WGPUI capture lineage), regardless of renderer.

**Leave behind:**
- The Rust `wgpui`/`hud` crates as *implementation* — the active stack is
  Bun/Effect/Foldkit + three-effect; reviving a Rust GPU UI lib would re-open a
  deprecated lane. Mine the *taxonomy and aesthetic*, not the code.
- Free-floating, fully draggable/overlapping windows as the DEFAULT layout. The
  §5.2 anti-clutter learning (System 5) is that unconstrained pane sprawl hurt
  usability; prefer grouped/destination panes with the hotbar+palette as the
  primary controls, and offer free panes only where they earn it.
- Commander's NIP-90/DVM/wallet-specific pane menagerie verbatim — those map to
  a different product era; reuse the *mechanism*, re-pick the *panes* from the
  current `NAV_GROUPS`.

**One-line thesis for the rebuild:** the durable OpenAgents HUD = *typed
pane-as-data + a registry-driven sidebar/palette/hotbar trio + an Arwes-style
sci-fi skin*, with hand-tracking as an optional flourish. System 5 already owns
the first two; the rebuild's job is to lay the hotbar + sci-fi skin (and,
optionally, gestures) over it — built up from the text bar, not bolted onto a
window-sprawl.

---

## Appendix: key paths

- Transcripts: `docs/transcripts/{117,119,170,178,199,200,214,228}.md`,
  guide `docs/transcripts/README.md`.
- Current shell: `apps/autopilot-desktop/src/ui/{nav,keyboard,commands,view,model}.ts`.
- Existing related audit (§5.2 anchor):
  `docs/launch/2026-06-19-autopilot-desktop-coding-agent-audit.md`.
- Backroom Rust HUD: `backroom/archive/openagents/wgpui/{wgpui,hud}/`.
- Commander (re-clone to view): `OpenAgentsInc/commander` →
  `src/{types/pane.ts, stores/panes/, panes/Pane.tsx, panes/PaneManager.tsx,
  components/hud/Hotbar.tsx, components/hands/}`, `images/demo.gif`.
