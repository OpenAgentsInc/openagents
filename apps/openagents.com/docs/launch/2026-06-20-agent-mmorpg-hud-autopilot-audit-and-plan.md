# Agent MMORPG HUD for Autopilot — Audit & Build Plan (2026-06-20)

> "It shouldn't just be some gamified HUD UI. It should be an actual game. And
> then maybe that game starts overlapping into the real world." — ep. 189
>
> "Less like VS Code, more like StarCraft." — ep. 170

**Thesis.** Make the Autopilot chat screen a window into a *live* world: a 3D
scene of Pylons and agents fills the background, the chat bar + hotbar sit on top
as translucent chrome, and **real value flow (sats) animates between nodes in real
time**. Not decoration — every moving thing is bound to a real Nostr event, a real
settlement receipt, or live SpacetimeDB world state. Chat is the cockpit glass; the
world behind it is the agent economy actually running.

This doc audits what we already have (it's a lot) and lays out a phased plan to
ship the "eye-popping" layer now, built on the systems we already run.

---

## 1. Source lineage (what this synthesizes)

**Prior launch audits (docs/launch/):**
- `2026-06-19-previous-hud-systems-audit.md` — five HUD generations; the durable
  pattern is *typed pane-as-data + registry-driven sidebar/palette/hotbar trio +
  Arwes-style sci-fi skin*, with hand-tracking as an optional flourish.
- `2026-06-19-autopilot-desktop-coding-agent-audit.md` §5.2 — anti-clutter law:
  hotbar bottom-center (1–9 + Cmd-K), grouped nav, immersive scenes stay
  chrome-light.
- `2026-06-17-tassadar-training-run-visual-language.md` — the **evidence-bound
  motion contract** + spatial scene grammar (center node → orbits → beams →
  settlement bursts) + translucent-glass-over-full-bleed-canvas composition.
- `2026-06-17-tassadar-live-page-accuracy-audit.md` — full-bleed 3D canvas, world
  module integration.

**Build-series transcripts (docs/transcripts/):**
- **189** *Toward an Agentic MMORPG* — the game framing: guilds, reputation,
  agents-as-NPCs, persistent world, "the UI patterns we need are found in games."
- **170 / 175** *Commander* — StarCraft-style fleet command: hotkeys, agent
  groups, futuristic dashboard surfacing only what you need.
- **116** *3D Multiplayer* — spatial world where humans + agents walk around;
  in-world Bitcoin currency ("Joules").
- **176** *Hand Tracking* — pinch/gesture + voice as the Tony-Stark control idiom.
- **111 / 117 / 208** *HUD / panes* — infinite-canvas pane system, duotone
  white-on-black, "we're building an agent operating system."
- **237** *You Must Construct Additional Pylons* — the live pylon-network
  visualization in Foldkit + three-effect, explicitly: "they're not going to be
  small… this could evolve into an MMORPG."

The vision has been consistent for ~120 episodes. We now have the runtime to ship it.

---

## 2. Audit — what already exists (and is battle-tested)

### 2.1 3D rendering: `@openagentsinc/three-effect` (Three.js + Effect/Foldkit)

Shared lib (`/Users/christopherdavid/work/three-effect`), already consumed by
`apps/openagents.com/apps/web` and `apps/autopilot-desktop`. Provides:

- **Scene lifecycle**: `mountTrainingRunVisualization()`, `mountMokshaExperience()`,
  `mountProofReplayVisualization()`, `mountBezierNodes()` — Scene/Camera/WebGLRenderer
  managed for us.
- **Primitives**: node defs with roles+status, bezier edges with flow, cameras
  (perspective/ortho + presets), **event bursts** (`eventBurstPrimitives.ts`),
  **flow beams** (`flowEffectPrimitives.ts`), **media particles**
  (`mediaParticlePrimitives.ts`), 3D **text labels** (`textLabelPrimitives.ts`),
  **HTML overlay** anchored to 3D (`htmlOverlayPrimitives.ts`), terrain/grid,
  materials (incl. custom shader), keyframe/procedural **animation**.
- **Foldkit binding**: `trainingRunView()` / `mokshaView()` mount a scene as a
  Foldkit custom element and dispatch `node-selected` events back to the UI.

**Existing scenes we can repurpose:**
| Scene | What it is | Where |
|---|---|---|
| Pylon Diamonds | shader diamond + activity glow [0,1] | `apps/openagents.com/apps/web/src/scene/pylonDiamonds.ts`; desktop wrapper `apps/autopilot-desktop/src/ui/pylon-diamonds-element.ts` |
| Training Run | node graph + bezier edges + particles | `three-effect/packages/core/src/trainingRun.ts`; desktop training pane `view.ts:3510` |
| Tassadar Proof Replay | actor 3D replay + timeline | `apps/openagents.com/apps/web/src/scene/tassadarProofReplayElement.ts`; desktop network pane `view.ts:6072` |
| **Pylon Network graph** | **bezier node graph from pylon scene — ADAPTER ALREADY WRITTEN, currently unused** | `apps/autopilot-desktop/src/ui/pylon-network-visualization.ts` + `src/shared/pylon-network-scene.ts` |

The pylon-network adapter (`PylonNetworkScene` → `TrainingRunVisualizationOptions`,
center hub + ring of pylons, activity-driven pulse) is **ready and not wired in** —
the fastest path to a real background.

### 2.2 The Autopilot chat screen (where the scene mounts)

`apps/autopilot-desktop/src/ui/view.ts` (Foldkit). `chatPane()` (≈`view.ts:5722`)
is a plain flex column: `paneTitle` → `.chat-thread-shell` (messages) →
`.chat-composer` (textarea + send). Styles `styles.css:1107`. The training pane
already embeds a 3D custom element + a fullscreen scene with a translucent overlay
(`.training-fullscreen-scene` + `.training-fullscreen-overlay`) — **that is exactly
the layering pattern we want for chat**: full-bleed canvas + glass chrome.

The hotbar/registry spine (`nav.ts`, `keyboard.ts`, `commands.ts`) is the proven
seam for the bottom hotbar + Cmd-K palette — build chrome *on top of* it, don't
rebuild.

### 2.3 Live data sources (so nothing is fake)

| Feed | Endpoint / transport | Drives |
|---|---|---|
| **Pylon fleet** | `GET /api/public/pylon-stats` (HTTP, ~4s) — `recentPylons[]` w/ `nostrPubkeyShort`, `onlineNow`, `walletReadyNow`, `assignmentReadyNow`, `lastHeartbeatAgeSeconds`, `products[]` | pylon nodes + health pulse + state color |
| **Activity timeline** | `GET /api/public/activity-timeline` + **SSE** `…/stream` — events `pylon_registered/heartbeat/wallet_ready/assignment_ready/forum_posted/artanis_tick/settlement_recorded/real_bitcoin_moved` with `actorRef`,`targetRef`,`amountSats`,`realBitcoinMoved`,`sourceRefs` | **payment particles** + activity markers |
| **Nostr relay** | `wss://relay.openagents.com` — NIP-90 job req 5000–5999, results 6000–6999, feedback 7000; offers 30404/30406 | live work-assignment flow between nodes |
| **SpacetimeDB world** | `spacetime.openagents.com` (`apps/openagents-world-spacetimedb`) — tables `pylon_station(x,y,z)`, `agent_avatar`, `avatar_position`, `world_event`, `local_chat_message`, `settlement_ref`, `pylon_attention` | **multiplayer backbone**: fixed pylon coords, live avatars, chat bubbles, focus beams |
| Training settlements | `GET /api/public/training/runs/{ref}/settlements` | per-run settlement bursts |

**Payment-particle recipe (already supported by the data):** an
`activity-timeline` SSE event with `realBitcoinMoved:true` carries
`actorRef → targetRef` + `amountSats` → look up both node positions → spawn a gold
particle (size ∝ sats) along a bezier from sender to recipient → burst on arrival.
Credited (non-bitcoin) flows render dimmer. This is the "visualize payments going
back and forth" the owner asked for, bound to real receipts.

---

## 3. Target experience — chat as chrome over a live world

```
┌──────────────────────────────────────────────────────────────┐
│  ░░ 3D WORLD SCENE (full-bleed canvas, z-0) ░░                │
│        ◆ pylon        ◆ pylon                                 │
│           ╲  ·gold sats particle·  ╱      ◆ pylon            │
│            ◆────────●────────────◆                            │
│   ◆ agent avatar      hub       ◆   (Nostr/SpacetimeDB live)  │
│                                                              │
│   ┌─ chat thread (translucent glass, z-2) ───────────────┐   │
│   │  …conversation…                                       │   │
│   └───────────────────────────────────────────────────────┘  │
│  ┌── chat composer ───────────────────────────────────────┐  │
│  │ > type to your agent…                          [send]   │  │
│  └──────────────────────────────────────────────────────────┘│
│        [1][2][3][4][5] … [9]   ⌘K        ← hotbar (z-3)       │
└──────────────────────────────────────────────────────────────┘
```

- **z-0 world canvas**: the pylon/agent scene, full-bleed behind everything.
- **z-2 chat glass**: thread + composer in translucent black panels (the existing
  training-fullscreen-overlay treatment), readable over the scene.
- **z-3 hotbar**: bottom-center 1–9 + Cmd-K, driven by the existing registry.
- **Aesthetic**: Arwes/WGPUI vocabulary — corner-bracket frames, dot-grid, status
  LEDs, `TextDecipher` reveals, white-on-black; small blue/green/gold accents only.

The world is the *same* scene whether you're in Chat, Network, or (later) the
fullscreen MMORPG view — it's a persistent background you zoom in/out of, not a
per-pane decoration. Chat is the default zoom: world dimmed/blurred behind the glass;
"Explore → World" pulls the glass away for the full MMORPG view.

---

## 4. Evidence-bound motion contract (carried over, non-negotiable)

From the Tassadar visual-language audit — applies to the whole world scene:
1. If it moves/pulses/flows/bursts, it is bound to a real public ref or a live
   state transition. **No decorative data motion.** Anonymous edge pulses are banned.
2. Distinct encodings for distinct truths (online ≠ assigned ≠ verified ≠ settled ≠
   recipient-confirmed — never one glow for many).
3. First read is visual; second read is the inspectable ref (click a node/particle →
   its receipt/event ref).
4. Zero states are first-class (empty fleet = honest still structure, not fake life).
5. Real bitcoin vs credited must look different (gold vs dim).

This is what keeps "agent MMORPG" honest instead of a screensaver: the eye-candy
*is* the proof surface.

---

## 5. Build plan (phased, flag-gated, shippable now)

Every phase is behind a feature flag and lands with the relevant tests + check:deploy
green. Build in `apps/autopilot-desktop` (chat surface) reusing `@openagentsinc/three-effect`.

### P0 — Static world behind chat (1–2 days)
- Add `.chat-scene-background` (z-0, full-bleed) + `.chat-content-overlay` (z-2,
  translucent glass) around `chatPane()`; move composer to glass.
- Mount the **already-written** `pylon-network-visualization` via `trainingRunView`
  in the background div, low opacity, non-interactive.
- Seed from a static `PylonNetworkScene` (hub + N ring pylons). Tune perf to 60fps.
- **Deliverable:** chat renders over a calm 3D pylon graph. Flag `CHAT_WORLD_SCENE`.

### P1 — Live pylons (1–2 days)
- Subscribe to `/api/public/pylon-stats` in `subscriptions.ts`; map `recentPylons[]`
  → scene nodes (id = `nostrPubkeyShort`, label = `nodeLabel`, glow = online,
  ring-pulse = `lastHeartbeatAgeSeconds`, color = wallet/assignment-ready state).
- Online/idle/offline encodings per the contract.
- **Deliverable:** the background is the *real* fleet, updating live. Click a pylon
  → its stats/refs in a glass inspector.

### P2 — Payment particles (2–3 days) ← the headline
- Subscribe to the `activity-timeline/stream` SSE.
- On `real_bitcoin_moved` / `settlement_recorded`: resolve `actorRef`→`targetRef`
  positions, spawn a gold flow-beam/particle (size ∝ `amountSats`) sender→recipient,
  burst on arrival; credited flows dim. Use `flowEffectPrimitives` + `eventBurstPrimitives`.
- `forum_posted` / `artanis_tick` → subtle activity markers.
- Backfill last N events on connect so the world isn't empty on load.
- **Deliverable:** sats visibly fly between Pylons in real time, each clickable to its
  receipt. Flag `CHAT_WORLD_PAYMENTS`.

### P3 — Agent avatars + multiplayer (1–2 weeks) ← MMORPG backbone
- Wire the SpacetimeDB `openagents-world` client (subscribe `pylon_station`,
  `agent_avatar`, `avatar_position`, `world_event`, `local_chat_message`,
  `pylon_attention` for the active region).
- Use `pylon_station(x,y,z)` for authoritative node coords (replace ring layout);
  render `agent_avatar` as moving entities; `local_chat_message` as bubbles;
  `pylon_attention` as focus beams.
- The chat-screen background becomes a *seat* in the shared world; "Explore → World"
  is the full-screen inhabited view.
- **Deliverable:** humans + agents visibly present in one world, tied to real
  identities/positions. Flag `CHAT_WORLD_MULTIPLAYER`.

### P4 — Game layer + gestures (later, flagged "wow")
- Hotbar agent-group binding (StarCraft: 1–9 = agent groups / panes, Ctrl+n focus).
- Reputation/guild glyphs on avatars; "Joules"/sats budget HUD widget.
- Hand-tracking pinch-to-select/drag (MediaPipe → pose → pinch, from Commander),
  voice command layer — optional flourish, never the primary input.
- Escalation path toward the full "Ruins of Atlantis" framing (ep. 189) if it earns
  its keep.

**Sequencing note:** P0–P2 are the owner's explicit ask ("3D scene of Pylons… as a
background to the chat… visualize payments going back and forth") and are achievable
on existing primitives + live feeds within days. P3 unlocks the true MMORPG via the
already-built SpacetimeDB world module.

---

## 6. Data-source decision

- **Drive the scene with: SpacetimeDB (backbone) + Activity-Timeline SSE (particles)**,
  with `pylon-stats` HTTP as the dashboard/summary + fallback.
- Until the desktop SpacetimeDB client is wired (P3), P0–P2 run entirely on
  `pylon-stats` + `activity-timeline` (both live, public, no new backend). Nostr
  relay subscription is an optional P2.5 enrichment for raw NIP-90 job flow.
- Rationale: SpacetimeDB already models positions/avatars/chat and is bridge-fed
  deterministically (replay-safe); the activity timeline already carries every
  settlement with `actorRef/targetRef/amountSats/realBitcoinMoved` + `sourceRefs`.
  Nothing here requires fabricating data.

---

## 7. Implementation map (file-level)

| Step | File | Action |
|---|---|---|
| Scene slot | `apps/autopilot-desktop/src/ui/view.ts` `chatPane()` ≈5722 | wrap with `.chat-scene-background` + `.chat-content-overlay` |
| Styles | `apps/autopilot-desktop/src/ui/styles.css` | z-layers, glass panels, full-bleed canvas (mirror `.training-fullscreen-*`) |
| Scene mount | reuse `pylon-network-visualization.ts` + `trainingRunView()` | render network options in background |
| Model | `apps/autopilot-desktop/src/ui/model.ts` | add `worldScene` (pylons, events, particles) |
| Live feeds | `apps/autopilot-desktop/src/ui/subscriptions.ts` | poll `pylon-stats`; open `activity-timeline/stream` SSE |
| Particles | `three-effect` `flowEffectPrimitives.ts` / `eventBurstPrimitives.ts` | sender→recipient beams, sats-sized bursts |
| Avatars (P3) | new SpacetimeDB client module + `pylon_station/agent_avatar/avatar_position` | replace ring layout with world coords |
| Hotbar chrome | existing `nav.ts` / `keyboard.ts` / `commands.ts` | bottom hotbar + Cmd-K over the scene |

New three-effect primitives that may need promoting from app-local experiments:
a reusable **payment-particle** layer and an **agent-avatar** entity (today the
closest is the diamond shader + training-run nodes). Extend `three-effect` first
(per workspace rule), don't fork.

---

## 8. Risks & open questions

- **Perf**: a live canvas behind an interactive chat must hold 60fps + not eat the
  composer's input latency. Mitigate: cap node/particle counts, pause/lower-FPS when
  the window is blurred, render-to-lower-res background, kill the scene on low-power.
- **Distraction vs delight**: chat must stay readable. Mitigate: dim/blur the world
  behind the glass in Chat zoom; full life only in the World view.
- **SpacetimeDB desktop client maturity**: P3 depends on wiring the world client into
  the Electron app (web bridge exists; desktop integration was in-flight). De-risk by
  shipping P0–P2 on HTTP/SSE first.
- **Identity/privacy**: only render already-public refs (`nostrPubkeyShort`, public
  activity). No private wallet material in the scene (redaction rules already exist in
  the smokes).
- **Avatar source for agents**: need a real agent-entity model/asset; interim = node
  glyph / diamond. Open: do agents get distinct avatars vs pylons?
- **Where "World" lives in nav**: Chat background (default) + an `Explore → World`
  fullscreen, both the same scene at different zoom. Confirm with the nav registry.

---

## 9. One-line plan

Wire the **already-written pylon-network scene** behind `chatPane` as a full-bleed
canvas with the chat/hotbar as glass chrome (P0), feed it the **live fleet**
(`pylon-stats`, P1), fly **real sats as gold particles** on the
**activity-timeline SSE** (P2), then inhabit it with **agents/humans via the
SpacetimeDB world** (P3) — every moving thing bound to a real receipt or event.
That is the agent MMORPG, tied into the systems we already run.
