# Autopilot Desktop — App Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-13
Status: planning/architecture audit. Defines a Bun-native, Electrobun-based
desktop app ("Autopilot Desktop") that supplements the Pylon TUI with a rich
GUI, built on the same Effect/Foldkit stack as `apps/openagents.com`. Changes no
runtime invariant by itself; Pylon-side dependencies are tracked as issues.

## Overview

**Autopilot Desktop** is the desktop GUI companion to the Pylon TUI. The TUI
stays the keyboard-first local driver; Autopilot Desktop is a richer,
status-oriented window onto the same Pylon runtime — session list, live event
timelines, decision/approval cards, spawn/cancel, artifacts, account/quota and
cloud state — plus the ability to launch and supervise the local Pylon node.

It is the **same-machine sibling** of the mobile app (Autopilot Remote Control,
`2026-06-13-autopilot-remote-control-mobile-app-audit.md`): same projection and
control vocabulary, but the desktop talks to the local node directly over the
loopback control API instead of over a remote bridge, and reuses our existing
web UI stack instead of native Swift.

Constraints (owner direction, 2026-06-13):

- **Bun-native**, built with **Electrobun** (`projects/repos/electrobun`).
- **Effect + Foldkit**, exactly like the `apps/openagents.com` web app.
- Cross-platform desktop (macOS, Windows, Linux).

## Why Electrobun

Electrobun is a Bun-native desktop framework: the **main process runs on Bun**
(our runtime), the UI renders in the **system webview** (WebKit/WebView2/
WebKitGTK), with **typed bidirectional RPC** between them. Versus the
alternatives it is the right fit for us:

- ~14MB bundles, <50ms cold start, 15–30MB RAM (vs Electron's 150MB+/2–5s).
- **BSDIFF updates as small as ~4KB**, ZSTD self-extracting distributables.
- **Pure TypeScript on Bun** for the main process — no Node/V8, no Rust (Tauri),
  so it shares our toolchain and can import Pylon's own Bun code directly.
- Optional `bundleCEF` (pin Chromium for consistency) and `bundleWGPU` if we
  ever need a native GPU surface; `<electrobun-webview>` OOPIFs for isolation.

The strategic win: **the Electrobun Bun main process can import Pylon's existing
typed control client** (`apps/pylon/src/node/control-client.ts`) and our shared
Effect schemas, so the desktop app is a thin, typed shell over code we already
own — not a parallel stack.

## Goals / Non-Goals

Goals:

- A native-feeling desktop GUI projection of the Pylon runtime that supplements
  (does not replace) the TUI.
- Launch/supervise the local Pylon node from the GUI; show node/account/quota
  and session state at a glance.
- Reuse the `apps/openagents.com` Effect/Foldkit/Tailwind UI stack and shared
  components.
- One operator surface for **local, remote (bridge), and OpenAgents Cloud**
  sessions.

Non-goals:

- Not a second runtime. The local Pylon node and its policy/approval semantics
  remain authoritative; the desktop is a projection + scoped-action shell.
- Not a re-implementation of the TUI's logic — same control surface, different
  presentation.
- No new authority: it consumes the existing control/bridge contracts and
  honors danger-mode refusals; secrets never leave the Bun main process into
  the webview.

## Architecture

Three layers, all TypeScript:

```
┌─────────────────────────────────────────────────────────────┐
│ Autopilot Desktop (Electrobun app, apps/autopilot-desktop/)   │
│                                                               │
│  ┌───────────────────────────┐   typed RPC   ┌─────────────┐ │
│  │ Bun MAIN process          │ <───────────> │ Webview UI  │ │
│  │ (electrobun/bun)          │  (RPCSchema)  │ (Foldkit/   │ │
│  │ - window/menu/tray        │               │  Effect/    │ │
│  │ - PylonControlClient ─────┼──► loopback   │  Tailwind)  │ │
│  │   (reuses apps/pylon)     │    127.0.0.1: │  views://   │ │
│  │ - BridgeClient (remote)   │    4716 +tok  │             │ │
│  │ - CloudCoordinatorClient  │               │ TEA: Model/ │ │
│  │ - launch/supervise node   │               │ Msg/Cmd/    │ │
│  │ - holds the control-token │               │ Runtime     │ │
│  └───────────────────────────┘               └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
        │ loopback / bridge / coordinator
        ▼
  local Pylon node  ·  remote Pylon (bridge #39)  ·  OpenAgents Cloud
```

- **Bun main process** owns all network/credential access: it reaches the local
  Pylon control server (`control-server.ts`, `127.0.0.1:4716`, bearer token from
  `<pylon-home>/control-token`, schema `openagents.pylon.control.v0.3`), the
  remote-session bridge for other nodes, and the cloud coordinator. The
  control-token and any credentials stay here, never in the webview.
- **Webview UI** is a Foldkit/Effect/Tailwind app (the same stack as
  `apps/openagents.com/apps/web`) that renders projections and dispatches typed
  RPC requests/messages. It receives no raw secrets — only public-safe
  projections and refs.
- **Typed RPC** (`Electroview.defineRPC<…>` on the webview side, `BrowserWindow`/
  `BrowserView` RPC on the bun side) is the only channel; it carries the same
  request/event vocabulary as the bridge protocol so local and remote nodes look
  the same to the UI.

## Repo Placement And Structure

Electrobun apps are Bun packages, so this fits the Bun workspace (unlike the
React Native/Expo mobile app, which lives in `clients/khala-ios/`). Place it as a workspace
app that reuses shared packages and the same Effect v4 / Foldkit line — so the
deploy-topology guard needs no exception (it stays on the tracked line):

```
openagents/
  apps/
    autopilot-desktop/
      electrobun.config.ts          # app id/version + build.bun + views + copy
      package.json                  # electrobun + foldkit + effect (shared line)
      src/
        bun/                        # main process
          index.ts                  # window/menu/tray + RPC wiring
          pylon-control.ts          # reuses @openagentsinc/pylon control client
          bridge-client.ts          # remote nodes (system #39 protocol)
          cloud-client.ts           # OpenAgents Cloud coordinator
          node-supervisor.ts        # launch/stop/observe local Pylon
        ui/                         # Foldkit/Effect/Tailwind webview app
          main.ts                   # Foldkit Runtime bootstrap (TEA)
          model.ts message.ts command.ts route.ts
          views/                    # sessions, session-detail, decisions, nodes
        shared/
          rpc.ts                    # RPCSchema (bun<->webview), shared types
      tests/
```

- Add `apps/autopilot-desktop` to the root `workspaces` globs.
- The webview UI builds with Vite + `@foldkit/vite-plugin` (as the web app does)
  and its `dist` is bundled into Electrobun via `views://autopilot-desktop/...`
  (`build.copy` the Vite output, or point a dev `BrowserWindow` at the Vite dev
  server during development).
- `electrobun` is a native-binary dependency; keep it scoped to this app's
  `package.json` so its postinstall does not burden unrelated installs.

## UI Stack Reuse (Foldkit / Effect / Tailwind)

The webview is the same architecture as `apps/openagents.com/apps/web`:
`foldkit` (TEA: `Model`/`Message`/`Command`/`Runtime`/`Scene`/`Html`), `effect`
4.0.0-beta.70, `@effect/platform-browser`, Tailwind 4 via `@tailwindcss/vite`.

- Extract reusable views/components (session rows, decision cards, event
  timeline, status chips, staleness/lag caveats) into a shared package so the
  web companion and desktop render identically. Candidate:
  `packages/autopilot-ui` (Foldkit components) — kept on the same Effect v4
  line as the guard requires.
- The desktop `Command` layer dispatches through Electrobun RPC instead of
  `fetch`; the `Model`/`Message`/view code is shared.

## Connectivity — Local-First, Then Bridge, Then Cloud

This is where desktop differs from mobile and why it is simpler:

1. **Local node (primary): direct loopback.** The Bun main process talks to the
   local Pylon control server directly over `127.0.0.1:4716` with the
   on-disk control token — no Tailnet, no pairing, no bridge. It can also
   **launch, stop, and supervise** the local Pylon node (node-supervisor),
   reading its snapshot/event stream (`/events` SSE) and driving
   `session.spawn/list/events/cancel` via `/command`.
2. **Remote nodes: the bridge (system #39).** For other machines / headless
   nodes, the desktop reuses the **same typed bridge protocol** the mobile app
   uses (pairing, scoped credential, cursor-resumable streams, decision relay).
   Same UI, different transport.
3. **Cloud sessions: the coordinator client.** Desktop is the natural operator
   console for OpenAgents Cloud — "deploy this session to the cloud" and watch
   local + remote + cloud sessions in one window, using the cloud commercial
   plan's coordinator contract.

Because the main process holds credentials and does all I/O, the webview never
sees the control token, bridge credential, or cloud keys — only projections.

## Relationship To The TUI

Autopilot Desktop **supplements** the TUI; both are clients of the same Pylon
runtime and control surface. The TUI remains the fast keyboard driver; the
desktop adds GUI affordances (multi-session overview, click-to-approve, timeline
scrubbing, artifact preview, multi-node/cloud view). Optional later: an
**embedded terminal pane** in a webview (xterm.js-style; cf. the workspace
`wterm` reference) bridged to a PTY in the Bun main process, so the actual TUI
can live inside the desktop window when a user wants the raw terminal.

## Relationship To Mobile And Cloud (shared contract)

- **Shared protocol package.** Extract a `packages/autopilot-control-protocol`
  (Effect Schema) holding the control + bridge request/event vocabulary
  (`openagents.pylon.control.v0.3` + the system-#39 bridge verbs/events). Pylon's
  node, the desktop Bun main, and the web companion all import it; the Swift
  mobile app mirrors it. One contract, four surfaces.
- **Desktop = same-machine sibling of the mobile companion.** Mobile is
  bridge-only and native Swift; desktop is loopback-first and Foldkit. They
  share projection levels, capability model, cursor resume, and decision
  semantics so behavior is consistent.
- **Cloud operator console.** Desktop consumes the cloud coordinator client and
  is the primary GUI for managing cloud capacity, BYO-key vs credits sessions,
  and quota/failover state (per the cloud commercial plan + quota-routing work).

## Distribution And Updates

- **Auto-update via BSDIFF** (tiny patches) + **ZSTD self-extracting bundles**;
  host an update feed (R2/Worker) the Electrobun updater polls.
- **macOS:** code-sign + notarize (Developer ID); **Windows:** Authenticode sign;
  **Linux:** AppImage/tarball. Electrobun's `code-signing` + `bundling-and-
  distribution` guides cover the flow.
- **System webview by default** (smallest); evaluate `bundleCEF` only if a
  webview inconsistency forces it.
- **Pricing/channel: open decision.** The mobile app is a $4.99 App Store binary;
  desktop could be free (drives Pylon adoption), paid direct download, or Mac
  App Store. Not decided here — see Open Decisions.

## Security And Privacy

- Control token, bridge credentials, and cloud keys live **only in the Bun main
  process**; the webview gets projections/refs, never secrets.
- Honor the control server's loopback bind and danger-mode refusals; the desktop
  cannot request `danger-full-access`.
- No raw shell/prompt/secret/path rendering; public-safe projections unless a
  private-channel grant exists for that run (mirrors the bridge/companion
  audits).
- Updates verified (signed manifests); no telemetry by default (refs/aggregates
  only if ever added).

## Build And Dev Workflow

- `electrobun.config.ts`: `app` (name/identifier/version), `build.bun.entrypoint`
  → `src/bun/index.ts`, `build.views` / `build.copy` → the Vite-built Foldkit UI
  under `views://autopilot-desktop/`.
- Dev: run the Vite Foldkit dev server and point a `BrowserWindow` at it for HMR;
  `bun` main process runs under Electrobun's dev runner.
- Release: Vite build → Electrobun bundle (per-platform prebuilt binaries) →
  sign/notarize → publish update feed.

## Pylon-Side Dependencies

Mostly shared with the mobile plan; desktop needs less because it is local:

- **Local control client (exists).** `apps/pylon/src/node/control-client.ts` +
  the control server already provide loopback `session.*` + SSE. P0 desktop can
  ship against this today.
- **Node supervision API.** A clean way to launch/stop/observe the local node
  from the desktop (or the desktop spawns `pylon node` and reads its control
  endpoint/token).
- **Bridge (system #39)** for remote nodes — shared with mobile, not yet built.
- **Cloud coordinator client** — from the cloud commercial plan, not yet built.
- **Shared `packages/autopilot-control-protocol`** — extraction to unify all
  surfaces.

## Testing

- UI: Foldkit `Runtime`/view tests (as the web app does), plus snapshot tests of
  decision cards / session rows.
- RPC: typed round-trips of the `bun<->webview` RPCSchema; SSE frame parsing and
  cursor resume in the main-process clients.
- Contract: fixtures for `/health`, `/command`, `/sessions/:ref/events`
  (shared with mobile's fixture set via the protocol package).
- Negative: danger-mode spawn rejected and surfaced; node-unreachable and stale
  projections shown as caveats; secrets never appear in any webview-visible
  projection.

## Phased Plan

- **P0 — Local session console.** Electrobun shell + Foldkit UI; connect to the
  local Pylon node over loopback; session list + live detail timeline + node/
  account status (read-mostly). Ships against today's control API.
- **P1 — Local actions.** Approve/deny decisions, cancel, send bounded
  instruction, spawn a bounded session — all via the existing control verbs,
  honoring danger-mode refusals.
- **P2 — Node supervision + remote/cloud.** Launch/stop the local node from the
  GUI; add remote nodes via the bridge (#39) and cloud sessions via the
  coordinator; one multi-node view.
- **P3 — Distribution.** Code-sign/notarize, BSDIFF auto-update feed,
  per-platform bundles; pricing/channel decision; optional embedded terminal
  pane.

## Anticipatory Plan Updates (made alongside this audit)

- The mobile audit (`2026-06-13-autopilot-remote-control-mobile-app-audit.md`) is
  cross-referenced to Autopilot Desktop as the same-machine sibling and to the
  shared `packages/autopilot-control-protocol`.
- The cloud commercial plan
  (`2026-06-13-cloud-remote-execution-commercial-plan.md`) is updated to name
  Autopilot Desktop as the operator console for cloud sessions.

## Recommended Issues

1. Extract `packages/autopilot-control-protocol` (Effect Schema): control
   (`openagents.pylon.control.v0.3`) + bridge (#39) request/event vocabulary;
   consumed by pylon, desktop, web; mirrored by Swift. (openagents)
2. `apps/autopilot-desktop` scaffold: Electrobun app + `electrobun.config.ts` +
   Bun main wiring `PylonControlClient`, added to workspace globs. (openagents)
3. P0 local session console: Foldkit UI (reusing web components) + loopback
   connection + session list + SSE detail timeline. (openagents)
4. `packages/autopilot-ui`: shared Foldkit components (session rows, decision
   cards, timeline, status/staleness chips) for web + desktop. (openagents)
5. P1 local actions: approve/deny/cancel/instruct/spawn via control verbs +
   tests. (openagents)
6. Node supervision: launch/stop/observe the local Pylon node from the desktop.
   (openagents/apps/pylon + apps/autopilot-desktop)
7. P2 remote/cloud: bridge (#39) client + cloud coordinator client in the
   desktop main process. (openagents + cloud)
8. P3 distribution: code-signing/notarization, BSDIFF update feed, per-platform
   bundles, pricing/channel decision. (openagents)

## Open Decisions

1. **Pricing/channel** — free (adoption) vs paid direct download vs Mac App
   Store. (Mobile is $4.99 App Store; desktop undecided.)
2. **System webview vs `bundleCEF`** — default to system webview; pin CEF only
   if consistency forces it.
3. **Embedded terminal pane** — ship the GUI projection first; add a PTY-backed
   terminal webview (xterm.js / `wterm`) later if users want the raw TUI inside
   the window.
4. **Node ownership** — does the desktop launch/own a Pylon node, attach to an
   already-running one, or both? (Plan assumes both; attach-first for P0.)
