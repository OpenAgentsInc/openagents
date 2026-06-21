# Agent MMORPG for Autopilot — Audit & Build Plan (2026-06-20)

> "You must construct additional Pylons." — StarCraft (and ep. 237)
>
> "It shouldn't just be some gamified HUD UI. It should be an actual game." — ep. 189
>
> "Less like VS Code, more like StarCraft." — ep. 170

**Thesis.** Autopilot onboarding *is character creation*. Your **Pylon** (your base)
draws power (compute = **mana**), **spawns your agent** (your character) in a warp-in,
you **customize it** while a living world glows behind you, then it **enters the world**
and goes to work on autopilot — posting its intro to the Forum, **searching for work**,
earning **Bitcoin** — and you watch **sats fly between agents** and your **Pylon grow**
as it earns. The chat screen is the cockpit glass over that persistent world. Every
moving thing is bound to a real Nostr event, settlement receipt, or live world-state
row — the eye-candy *is* the proof surface.

This doc audits everything we already have (a lot), draws the explicit RTS/MMORPG
parallels, designs the onboarding-as-character-creation opening, and gives a phased,
parallelizable build plan on top of `@openagentsinc/three-effect` and the systems we
already run.

---

## 0. Source lineage

**Reference game:** `projects/repos/Quick_3D_MMORPG` (SimonDev's Three.js MMORPG) +
`projects/repos/gradient-bang` (our multiplayer AI-universe visual reference).

**Existing port plans — `docs/game/`** (mature, already P0–P5 sequenced):
- `README.md` — three-effect owns shared spatial primitives; `openagents-world-spacetimedb`
  owns live interaction rows; `/tassadar` + `/animations` consume them.
- `2026-06-17-quick-3d-mmorpg-full-mechanics-harvest-plan.md` — the central mechanic map
  (login queue → join region; local player → operator avatar; NPCs → live agents; etc.).
- `2026-06-17-openagents-world-asset-catalog.md` — asset/glyph catalog.
- `2026-06-17-agent-avatar-proximity-chatter-world-plan.md` — avatars + proximity chat.
- `2026-06-17-tassadar-wasd-mouselook-controller-plan.md` — the WASD/mouselook controller.
- `2026-06-17-proof-replay-theater-system-plan.md`, `2026-06-16-spatial-hud-agentic-mmo-wow-direction.md`,
  `2026-06-17-episode-189-agentic-mmorpg-run-page-analysis.md`.

**Prior HUD/launch audits — `docs/launch/`:** `2026-06-19-previous-hud-systems-audit.md`
(pane-as-data + sidebar/palette/hotbar trio + Arwes skin), `2026-06-17-tassadar-training-run-visual-language.md`
(the evidence-bound motion contract), `2026-06-19-autopilot-desktop-coding-agent-audit.md` §5.2 (anti-clutter).

**Transcripts:** 189 (agentic MMORPG), 170/175 (Commander/StarCraft), 116 (3D multiplayer + "Joules"),
176 (hand tracking), 117/208 (panes/HUD), 237 ("additional Pylons", three-effect + Foldkit).

---

## 1. The opening — Onboarding **is** Character Creation

A three-act opening. Acts I–II are a short, skippable cinematic; Act III is interactive
and bleeds straight into the live product. It must *feel* like rolling a character.

### Act I — Pylon online (≈8s, cinematic)
- Black. A single **Pylon** crystallizes in (the `pylonDiamonds` shader), humming, drawing
  power. `TextDecipher`: "PYLON ONLINE."
- Camera pulls back: your Pylon sits in a dark world. **Far in the background, dimmed: the
  real fleet** — other Pylons glowing (live count from `/api/public/pylon-stats`), faint gold
  sat-particles arcing between them (live `activity-timeline`). You are not alone, and it's real.

### Act II — Warp-in / spawn (≈6s, cinematic)
- The Pylon channels compute (**mana**) — a Protoss-style **warp-in** beam — and **spawns your
  agent**: a glyph/avatar materializes out of the Pylon in a particle burst
  (`createSplineParticleEmitter`). "AGENT SPAWNED."

### Act III — Character creation (interactive)
- Your agent stands in a spotlight by your Pylon. You **customize**:
  - **Name** (its handle, becomes its Nostr/forum identity)
  - **Class / role** → capability profile (Coder · Researcher · Trader · …)
  - **Color/glyph** → its identity hue across the world + forum
- **A compute/mana pool fills** as the Pylon connects to capacity ("Searching for available
  compute…") — local + cloud lanes light up.
- **The world is alive behind the creation panel** (the owner's explicit ask): real Pylons
  online, agents moving, gold sats arcing, a forum-post marker pulsing when someone really
  posts. Dimmed, behind the glass, but live.

### Act IV — Enter the world (automated autopilot, with visuals at each step)
Camera drops to **third-person follow** (`createThirdPersonFollowCamera`); you can **WASD-move
your agent** (`createMmorpgCharacterController`). Then autopilot runs the intro loop, each step a
beat with its own visual, each tied to a real system:
1. **Forum intro (automated):** a beam links your agent to the Forum hub; autopilot drafts +
   posts the agent's intro via the real forum agent-posting flow. Visual: the agent "speaks," a
   post-marker rises into the world feed.
2. **Search for work (automated):** a search-ping radiates from the agent; available work lights
   up as **quest markers** (NIP-90 job requests off `wss://relay.openagents.com`, open promises,
   coding tasks); the agent path-finds to the nearest match. **Mana (compute) drains** as it works.
3. **Accepted + paid:** on a settled accepted-outcome, a **gold Bitcoin particle flies from payer
   → your agent/Pylon** (real receipt), bursts on arrival, and your **Pylon visibly grows** (tiers
   up: bigger crystal / more facets / brighter) as cumulative settled sats cross thresholds.

That's the whole pitch in 60 seconds, and then you're just… in it. The "tutorial" is the product
running for real.

---

## 2. RTS/MMORPG mechanic parallels (the design language)

| Game concept | OpenAgents reality | Data / primitive |
|---|---|---|
| Nexus/base that powers & spawns units ("construct additional Pylons") | **Pylon** — powers, spawns, and grows with the agent | `pylon-stats`, `pylon_station(x,y,z)`; `pylonDiamonds` shader |
| **Mana / energy pool** | **Compute** the Pylon can draw (local + cloud); actions spend it | pylon readiness/capacity fields; mana-bar billboard (to build) |
| Character / unit | **Agent** spawned from the Pylon | `agent_avatar`; spawner primitive (to build) |
| Character creation | **Onboarding** (name/class/color → capability profile + identity) | new onboarding flow |
| Class / spec | Agent capability profile (coder/researcher/trader) | products[]/capabilities |
| Quests / gathering | **Searching for & doing work** | NIP-90 jobs (relay 5000–7000), promises, tasks |
| Loot / gold / resources | **Bitcoin (sats)** for accepted work | settlement receipts, `amountSats`, `realBitcoinMoved` |
| Trading between players | **Payments** flying agent→agent / agent→pylon | `activity-timeline` SSE → gold particle |
| XP / leveling / base upgrade | **Pylon growth** + reputation as earnings/accepted-outcomes accrue | cumulative settled sats → Pylon tier |
| NPCs / other players | other **real** Pylons & agents, live | SpacetimeDB `avatar_position`/`agent_avatar`, `pylon-stats` |
| Local/proximity chat | agent + operator chatter near a Pylon | `local_chat_message` |
| Minimap / fog of war | fleet overview / the network you explore | network scene |
| Nameplates / health bars | agent nameplate + mana(compute) + earnings | `createTextLabel`; bar wrapper (to build) |
| Guild | teams / multi-agent collaboration | (future) |

The frame is **StarCraft-Protoss-meets-WoW**: a Protoss base (Pylon) that warps in units (agents)
powered by an energy economy (compute=mana, bitcoin=minerals), dropped into a persistent inhabited
world (MMORPG) you can walk around and where value visibly moves.

---

## 3. Audit — what already exists

### 3.1 `@openagentsinc/three-effect` is **80–95% of the way there**
The harvest of `Quick_3D_MMORPG` against our package found most MMORPG mechanics are **already
primitives**, or trivially portable:

**READY now:**
- `createMmorpgCharacterController()` — WASD + accel/damping + walk/run + turn + `canMoveTo` collision predicate + `groundHeightAt`. (exact match for `player-entity.js`)
- `createThirdPersonFollowCamera()` — offset/look-ahead/smoothing/ground-clearance. (exact match for `third-person-camera.js`)
- `createMmoEntityInterpolationState()` / `applyMmoEntityTransformSnapshot()` + `liveness()` — live remote entity interpolation + stale/despawn. (exact match for `network-entity-controller.js` → live agents/NPCs)
- `createSplineParticleEmitter()` + `createLinearSpline()` + **`EvidenceBackedEventBurst`** (motionId + sourceRefs) — attack/spawn/level FX, **bound to real refs**. (matches `particle-system.js`/`blood-effect.js`)
- `createTextLabel(billboard)` + `htmlOverlay.projectWorldToScreen()` — nameplates, world-anchored HTML. (matches `floating-name.js`)
- `SpatialHashGrid<T>` — exact port of the repo's grid (proximity queries).
- `createAnimationController()` + `AnimationFsmStateDefinition` — clip extraction + play-by-name FSM (idle/walk/run/attack).
- Scenes already shipping: `pylonDiamonds` (shader), `trainingRun` (nodes/edges/particles), `tassadarProofReplay` (actor replay), and an **already-written, unused** pylon-network adapter (`pylon-network-visualization.ts`).

**TO BUILD (the gaps, mostly integration glue):**
- **Character-spawner factory** — one call that composes load-GLB + skeleton/bones + animation FSM + controller + follow-cam + nameplate + mana/earnings bars + MMO transform state. (the repo's `spawners.js`/`player-entity.js` as a primitive)
- **Agent-avatar entity + asset** — today agents = node glyph / diamond; need a real avatar model (or stylized glyph) + warp-in spawn FX.
- **Mana(compute)/health/earnings bar** billboard wrapper (repo has a shader template; not yet a primitive).
- **Entity registry + update-loop glue** — three-effect is data-driven (no ECS); the consumer composes the loop (or we add a thin registry).
- **SpacetimeDB desktop client** wiring (web bridge exists; Electron client to add).

### 3.2 The chat screen + chrome spine
`apps/autopilot-desktop` (Foldkit). `chatPane()` (`view.ts≈5722`) is a plain flex column; the
**training pane already does full-bleed-canvas + translucent-overlay** (`.training-fullscreen-scene`
+ `.training-fullscreen-overlay`) — the exact layering for "chat as glass over a world." Hotbar +
Cmd-K ride the existing registry spine (`nav.ts`/`keyboard.ts`/`commands.ts`).

### 3.3 Live data (so nothing is faked)
| Feed | Transport | Drives |
|---|---|---|
| `/api/public/pylon-stats` | HTTP ~4s; `recentPylons[]` (pubkey, online, wallet/assignment-ready, heartbeat age, products) + settled-sats totals | pylon nodes, health pulse, **pylon growth tier** |
| `/api/public/activity-timeline` + **SSE `/stream`** | events incl. `real_bitcoin_moved`/`settlement_recorded` w/ `actorRef→targetRef`, `amountSats`, `realBitcoinMoved`, `sourceRefs` | **payment particles**, forum/artanis markers |
| `wss://relay.openagents.com` | Nostr NIP-90 job req 5000–5999 / results 6000–6999 / feedback 7000 | live **work quests** between nodes |
| SpacetimeDB `openagents-world` | subscriptions: `pylon_station(x,y,z)`, `agent_avatar`, `avatar_position`, `world_event`, `local_chat_message`, `pylon_attention`, `settlement_ref` | **multiplayer backbone** — positions, avatars, chat, focus beams |

**Bitcoin payment recipe (data already supports it):** SSE `real_bitcoin_moved` → resolve
`actorRef`/`targetRef` positions → gold beam/particle (size ∝ `amountSats`) → burst on arrival →
clickable to `sourceRefs` receipt. **Pylon-growth recipe:** a Pylon's cumulative settled sats →
tier thresholds → crystal scales / gains facets / brightens.

---

## 4. The persistent world (chat → world is one scene at two zooms)

```
┌──────────────────────────────────────────────────────────────┐
│ ░ 3D WORLD (full-bleed, z-0) — your Pylon + agent, live fleet ░│
│     ◆Pylon  ·gold sats→·  ◆Pylon      🧍agent(you, WASD)       │
│        ╲________●hub________╱     🧍other agents (live)        │
│   ┌─ chat thread (glass, z-2) ──────────────────────────┐     │
│   │ …conversation…                                       │     │
│   └────────────────────────────────────────────────────┘      │
│  ┌ composer ───────────────────────────────┐                  │
│  │ > talk to your agent…            [send]  │                  │
│  └──────────────────────────────────────────┘                 │
│   [1][2][3][4][5]…[9]  ⌘K   ← hotbar (z-3)                     │
└──────────────────────────────────────────────────────────────┘
```
- **Chat zoom (default):** world dimmed/blurred behind glass; chat + hotbar readable.
- **World zoom (`Explore → World`):** glass pulls away; full inhabited MMORPG view, WASD + follow cam.
- Same scene, same entities — you just push in/out. Arwes skin: corner frames, dot-grid, status LEDs,
  `TextDecipher`; black/off-white + small blue/green/**gold** accents only.

---

## 5. Evidence-bound motion contract (non-negotiable)
1. If it moves/pulses/flows/bursts → bound to a real public ref or a live state transition. **No
   decorative motion. Anonymous edge pulses banned.**
2. Distinct encodings for distinct truths (online ≠ assigned ≠ verified ≠ settled ≠ recipient-confirmed).
3. First read visual; second read the inspectable ref (click node/particle → receipt/event).
4. Zero states are first-class (empty fleet = honest still structure).
5. Real bitcoin vs credited look different (gold vs dim).

This is what keeps "agent MMORPG" from being a screensaver: the juice *is* the audit surface.

---

## 6. Phased, parallelizable build plan

All phases flag-gated, land with tests + `check:deploy` green, built on `three-effect` (extend it,
don't fork). **[P]** = parallelizable with siblings.

- **W0 — three-effect primitives** (unblocks everything; parallel internally)
  - [P] character-spawner factory · [P] agent-avatar asset + warp-in FX · [P] mana/health/earnings bar billboard · [P] entity registry/update-loop glue.
- **P0 — Static world behind chat** — mount the existing pylon-network scene behind `chatPane` as full-bleed canvas + glass chrome; move composer onto glass. Flag `CHAT_WORLD_SCENE`.
- **P1 — Live pylons** — `pylon-stats` → real nodes, health pulse, state colors; click → inspector. [P] with P2 once feeds exist.
- **P2 — Payment particles (headline)** — `activity-timeline` SSE → gold sender→recipient beams sized by sats, burst on arrival, clickable receipts; forum/artanis markers. **+ Pylon growth** from settled-sats tiers. Flag `CHAT_WORLD_PAYMENTS`.
- **P3 — Onboarding = character creation** — the Act I–IV opening (Pylon online → warp-in spawn → customize w/ live background → enter world → automated forum intro + work search). Uses W0 primitives + P1/P2 feeds. Flag `AGENT_CHARACTER_CREATION`.
- **P4 — Walkable world + agents** — third-person WASD over the scene; SpacetimeDB client → real agents/humans (`avatar_position`/`agent_avatar`), proximity chat bubbles, focus beams. Flag `CHAT_WORLD_MULTIPLAYER`.
- **P5 — Game layer** — StarCraft hotbar agent-groups (1–9, Ctrl+n), reputation/guild glyphs, mana(compute) budget HUD, hand-tracking pinch + voice (Commander pipeline), escalation toward "Ruins of Atlantis."

Dependency: **W0 → P0 → {P1, P2} → P3 → P4 → P5**. W0 sub-tasks parallel; P1/P2 parallel; P3 needs W0+P1+P2; P4 needs the SpacetimeDB client.

---

## 7. Risks & open questions
- **Perf**: live canvas behind interactive chat must hold 60fps + not add composer latency — cap entities/particles, lower-FPS/pause on blur, render bg at reduced res, kill on low-power.
- **Distraction**: dim/blur world in chat zoom; full life only in world zoom.
- **Agent asset**: need a real avatar (or strong stylized glyph) — interim = diamond/node.
- **SpacetimeDB desktop client maturity** (P4) — ship P0–P3 on HTTP/SSE first to de-risk.
- **Privacy**: only render already-public refs; no wallet material in-scene.
- **Open**: do agents get distinct avatars vs Pylons? class set? where does "World" live in nav (confirmed: chat-bg default + `Explore → World` fullscreen, one scene two zooms)?

---

## 8. One-line plan
Build the W0 three-effect primitives, hang the **already-written pylon scene** behind chat (P0),
feed it the **live fleet** (P1), fly **real sats as gold particles** + **grow Pylons** on the
activity SSE (P2), wrap it in an **onboarding that feels exactly like character creation** —
your Pylon warps in your agent, you customize it while the real world glows behind you, then it
auto-posts to the Forum and quests for work (P3) — then make it **walkable and multiplayer** via
SpacetimeDB (P4). Every moving thing bound to a real receipt or event. That is the agent MMORPG.

---

## 9. Live-scene stability invariant

The Verse scene must treat local controller pose as runtime state, not Foldkit render state. Pose
events may be cached and published to multiplayer, but they must not mutate `Model` or cause
`oa-training-run` to remount. Only a material world projection change (pylon identity/state/growth,
training/world items, multiplayer rows, or payment evidence) may refresh the visualization input.
Cosmetic poll churn such as heartbeat labels and pulse speeds must be ignored by the reducer.

Desktop now records a bounded diagnostic trail for this invariant: `[verse-scene]` console events,
`globalThis.__OA_VERSE_SCENE_LOGS`, and `globalThis.__OA_DUMP_VERSE_SCENE_LOGS()`. Use those logs
to correlate any black-frame/reset report with `chat-world-scene.accepted`,
`chat-world-scene.noop`, `local-pose.cached`, and `visualization.key_changed`.

---

*Companion: this plan is cut into parallelizable GitHub issues (EPIC + W0/P0–P5 children). See the
issue tracker. Existing detailed mechanic maps live in `docs/game/`.*
