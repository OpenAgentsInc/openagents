# Spatial / HUD / Agentic-MMORPG: bringing the archive's WOW factor into Forge & Autopilot

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-16
Status: Private blitz direction doc. Synthesizes ~200 episodes of HUD / Commander /
hand-tracking / MMORPG / gamified-UI ideas from `openagents/docs/transcripts/` and
maps them onto **today's** real product entities (Pylon nodes, the Tassadar run,
the Forge software factory, contributors earning Bitcoin) using what we now actually
have to build them: `@openagentsinc/three-effect` and the `/animations` gallery.
Goal: a visual identity that makes us *unmistakable* — and, unlike the 2024 demos,
backed by **real, receipted data** instead of mockups.

## 0. Thesis — the UI bet was right; only now can we cash it

The transcripts contain a single, stubborn UI thesis across the whole series:
**replace the chat box with a spatial, game-like operator surface** — a HUD you
command a fleet from, not a textarea you wait at. It kept showing up because it kept
being too early. Three things changed that make it shippable *and* substantive now:

1. **We have the rendering substrate.** `@openagentsinc/three-effect` is a real,
   Effect-owned Three.js runtime (26 primitive modules, Foldkit custom elements), and
   `/animations` already proves 16 live scenes (constellations, bezier networks, flow
   fields, instanced fields, glow/bloom). The "cool sci-fi HUD" is no longer a mock.
2. **We have real entities to render.** Pylon presence, the live Tassadar run
   (`run.tassadar.executor.20260615`), Forge work-orders/runs, verification verdicts,
   Lightning settlements. The old demos visualized *nothing*; ours can visualize the
   actual network.
3. **We have a reason for it to be more than eye-candy.** Episode 237's thesis is the
   **accepted outcome** + the **receipt**. A spatial HUD that lets you *click any
   glowing thing and dereference its proof* turns WOW factor into the product's core
   argument: "show the work, the verifier, and the settlement." The most viral artifact
   we can produce is a *visible* record of an agent earning Bitcoin for verified work.

So: this is not "add a 3D skin." It's "make the clearing layer legible and alive."

## 1. The archive idea bank (what the series envisioned)

Distilled from the transcript-mining pass; episode refs are pointers, not quote-grade.

### A. The HUD (panes, canvas, work-state)
- **Draggable / resizable panes**, command-click to spawn many, configure your own
  workspace (`119`); **infinite canvas** you pan/zoom, chats as panes (`208`).
- **Diffs rendered *in* the HUD** — never leave the interface to check GitHub (`117`).
- **Parallel multi-agent panes** — 4 issues solved at once, tab between contexts (`123`).
- **Live cost/credit burn in the header** — "the price of thought" in real time (`119`).
- **Event-log sidebar + identity/wallet pane** (Nostr + Lightning) (`208`).

### B. Commander / StarCraft fleet command
- **"Less like VS Code, more like StarCraft"; "Tony Stark does not use Microsoft
  products."** A futuristic dashboard to command a *fleet* without debugging detail
  (`170`).
- **Hotkeys + macros + group keybindings for agent swarms**; press a key to spawn an
  agent, select two and press a key to pay (`170`,`171`).
- **"You allocate gold budgets to agents that do the execution. You're playing
  StarCraft."** Operator-as-commander, capital allocation as gameplay (`200`).
- **Floating AR windows** you move around (`170`).

### C. Hologram / Jarvis / hand & voice
- **Manipulate holograms like Tony Stark / Jarvis** (`170`,`179`).
- **Hand tracking via MediaPipe** — pinch a pane and drag it; subtle finger gestures;
  hand + voice combined (`176`,`185`,`214`). "I could pinch one of the panes and move
  it around the screen" (`214`).

### D. Payment / economic visualization
- **Live sats counters, payment streaming, agents with Bitcoin balances**; invoice
  pop-ups; 2-sat job pulses every 12s (`171`,`214`).

### E. Agentic MMO / world
- **Agents as MMO NPCs; guilds, reputation, multiple players sharing a persistent 3D
  space**; **Joules** as in-world compute currency (`116`).
- **Decentralized agent MMO over Nostr** — regions, "shouting," per-region chat
  between humans and agents (`177`).
- **Guilds (EverQuest), coalitions that form, split work, split payments, and
  persist as teams** (`200`).
- **"The introductory product shouldn't be a gamified HUD — it should be an actual
  game that starts overlapping the real world"** (Ruins of Atlantis) (`189`).

### F. Gamification mechanics
- **AAPM — Agent Actions Per Minute — as a StarCraft-style metric, with a
  leaderboard** (`185`).
- **Public agent trace chain** — show, on the website, exactly what the agent did
  (`228`). Transparent collective learning vs. black-box labs (`116`).

### G. Aesthetic
- **Evangelion-style UI** (the look people freaked out about) (`214`); duotone /
  dark, red-green-only accents (`117`). Aligns with our **dark-only DESIGN** default.

## 2. What we have to build it today

### `/animations` gallery (16 live three-effect scenes)
Constellation (proximity-graph nodes), bezier networks/graphs (ring/spiral/web +
travelling pulses), instanced field (22×22 wave), flow field (1600 advected points),
tube flow (pulses along a pipeline), particles/starfield, grid floor (infinite
scrolling spatial context), glow knot + wobble sphere (fresnel/bloom), shader
gradient, blob-track instancing (rings + hot cores + connector splines + coordinate
sprites), light beams. **These are already 80% of an agentic-network aesthetic** —
they just aren't wired to real data yet.

### `@openagentsinc/three-effect` (the substrate)
Core: cameras (persp/ortho, bounds-fit), **controls** (Orbit / Map / Fly /
FirstPerson / PointerLock / Transform), **instancing** (`createInstancedMesh`),
shader materials, **scroll + htmlOverlay world-to-screen projection**, interaction
(raycasting, cursor, intersection visibility), **post-processing** (EffectComposer,
render/output/**bloom**), scene-graph (LOD, billboarding, edges, outlines, wide
lines, decals), particles/points, noise/fBm/surface-sampling, text geometry + font
loading, environment/PMREM/sky. Foldkit elements: `oa-training-run` (lifecycle
graph, contributor dots, flow, loss curves, **emits `node-selected`**),
`oa-bezier-nodes` (draggable), `oa-moksha`, `oa-spinning-cube`.

## 3. The mapping — old vision → today's real entities (the WOW surfaces)

This is the heart of the doc. Each surface renders **real, dereferenceable data**.

| Archive idea | Today's surface (proposed) | Real data it renders | Receipt click-through |
|---|---|---|---|
| MMO world + agents-as-inhabitants (`116`,`177`) · "living run" | **Tassadar "living run"** — extend `oa-training-run` into a live 3D world | Pylon nodes as instanced entities; worker↔validator **pairs**; executor traces as work-pulses; `exact_trace_replay` as a **beam between the two distinct devices**; settlements as payment particles | each entity → its verification challenge + settlement receipt |
| StarCraft fleet command (`170`,`200`) · Forge software factory | **Forge factory HUD** (#5088 signal→deploy) | the pipeline as a spatial production line; automations as units staffing stages; accepted outcomes flowing; throughput / cycle-time / pass-rate live | each "shipped" → Verification Report + Delivery Receipt |
| Draggable HUD panes + cost burn (`119`,`208`) | **Autopilot cockpit HUD** (#5087) over a 3D backdrop | live runs, event-log, identity/wallet pane, **credit burn in header**, the **public trace chain** (`228`) | each pane → its run/receipt |
| Network constellation + payment beams (`116`,`171`,`214`) | **Fleet/network minimap** | Pylon **presence** as a live constellation (we already have `constellation.ts`); Lightning settlements as **flow beams** (`tubeFlow`/`lightBeams`) | a node → its earnings + status |
| Guilds / coalitions / reputation (`200`) | **Guild / coalition layer** | human+agent teams; "verified-work count" as reputation; coalitions that form/split/persist | a guild → its members' receipts |
| AAPM + leaderboard (`185`) | **Throughput leaderboard** | accepted outcomes / week (the Linear 50–70 fixes/wk benchmark), per contributor/agent | a rank → that contributor's receipts |
| Hand / gesture (`176`,`214`) | **Gesture flourish** for the cockpit + keynotes | pinch-drag panes via MediaPipe | — (interaction, not data) |
| Evangelion aesthetic (`214`) | the **visual language** across all of the above | dark-only, bloom/glow, mono labels, red/green accents | — |

**The single highest-WOW, highest-leverage artifact:** the **Tassadar living run**.
It is the literal picture of Episode 237's promise — independent nodes doing verified
work and getting paid in Bitcoin — and it is the most viral thing we can ship because
it recruits humans ("your machine could be in there") and agents ("there's real money
here") at once. Build this first.

## 4. Gap list — what three-effect is missing, and what to add

(From the inspection pass. Add these to the **shared `three-effect`** package, not
one-off in the app — per the workspace "three-effect-first" rule — so every surface
reuses them.)

**P0 — needed for the living run + factory HUD:**
- **Crisp 3D text / labels** — port a Troika-style SDF text primitive into
  three-effect (today only `blobTracking` fakes labels with canvas sprites; HUDs need
  real, billboarded, outlined labels at scale). *(Tracks the existing "port drei/troika
  text into three-effect" memory.)*
- **Scalable entity pool** — an instanced agent/node entity layer (spawn/despawn,
  per-entity color/status, LOD) for hundreds–thousands of Pylons without per-mesh cost.
- **HUD-overlay compositing** — formalize the `htmlOverlay` world-to-screen projection
  into a reusable "2D HUD layer pinned over the 3D scene" (safe-area math, responsive
  scale) so panes/labels/readouts sit cleanly over the world.
- **Payment-flow effects** — particle-burst + animated beam factories for
  settlements/work-pulses (we have `tubeFlow`/`lightBeams`/`flowField` as starting
  points; package them as data-driven `flowBeam(from,to,rate)` + `payoutBurst(at)`).
- **Live-presence binding** — a thin adapter that maps a stream of real entities
  (Pylon presence, run state) → entity-pool updates with interpolation, so scenes are
  *data-bound*, not scripted.

**P1 — needed for the fleet minimap + cockpit:**
- **Camera-mode presets** with smooth transitions (orbit overview ↔ fly-through ↔
  focus-on-entity) over the existing controls primitives.
- **Minimap / radar** via render-to-texture + a camera-relative compass.
- **Node-link graph layout engine** (force-directed / circular / hierarchical) —
  `trainingRunView` hardcodes its layout; generalize it.
- **Draggable 3D panes / 3D UI widgets** (buttons, toggles) for the canvas HUD.
- **Time-series chart builders** (line/area/bars) for throughput/cycle-time/loss.

**P2 — flourish:**
- **Hand/gesture input adapter** (MediaPipe → pointer/gesture events) for pinch-drag.
- **Post-processing chains** (bloom + FXAA + subtle chromatic aberration) as a named
  "Evangelion" preset for instant brand look.

## 5. Build proposals (prioritized; file under existing epics, don't reorder)

These extend the Forge blitz ROADMAP (Epic B cockpit/#5087, factory dashboard/#5088,
metrics/#5090; Epic G terminal-agent-systems/#5107) and the `/animations` surface.
All `[NEW]`; do **not** reorder issue-backed items.

- **[NEW] P0 — Tassadar "living run" view.** Extend `oa-training-run` into a
  data-bound 3D world fed by real run state + Pylon presence; nodes, worker↔validator
  pairs, verification beams, settlement bursts, verified-work/loss readouts; every
  entity dereferences to its receipt. Ships on the public run page + `openagents.com`
  hero. *(Extends #5090 metrics + the run surface; needs the P0 three-effect prims.)*
- **[NEW] P0 — Forge factory HUD.** Render #5088's signal→deploy pipeline as a
  StarCraft-style spatial production line (automations as units, accepted outcomes
  flowing, live throughput/cycle/pass), each shipped outcome clicking to its receipt.
  *(Extends #5088; pairs with the Linear software-factory reference doc.)*
- **[NEW] P1 — Network constellation + payment beams.** Pylon presence as a live
  constellation (`constellation.ts` data-bound) with Lightning-settlement flow beams;
  a "command your fleet" overview/minimap. *(Extends #5087/#5088.)*
- **[NEW] P1 — Cockpit HUD panes over 3D.** Bring the old draggable-pane / infinite-
  canvas HUD into `/autopilot` (#5087): event-log, identity/wallet pane, credit-burn
  header, and the public agent **trace chain** (`228`) — over a subtle 3D backdrop.
- **[NEW] P2 — Gamification layer.** Throughput **leaderboard** (accepted outcomes/wk;
  AAPM reframed honestly), **guilds/coalitions** (human+agent teams), reputation =
  verified-work count. *(Extends #5107 + the labor market.)*
- **[NEW] P2 — Gesture flourish.** MediaPipe pinch-drag for cockpit panes — a keynote/
  demo "wow," opt-in, not load-bearing.
- **[NEW] three-effect additions** — land the P0/P1 gap-list primitives in the shared
  package first, then consume them across the surfaces above.

## 6. Guardrails (so WOW stays honest)

- **Three-effect-first.** Build these as reusable primitives in
  `@openagentsinc/three-effect`, consumed by `openagents.com` on `@openagentsinc/ui`;
  don't fork bespoke Three code per surface. (Workspace memory: three-effect first;
  Foldkit owns structure.)
- **Dark-only DESIGN.** The Evangelion/duotone language fits the default; no light-mode
  variant needed for these surfaces.
- **Receipt-first / no fake activity.** Every glowing thing must dereference to a real
  receipt; **never animate seeded/demo data as if it were live** (same rule as the
  software-factory reference doc + `compliance-guardrails.md`). A pretty dashboard of
  invented numbers is exactly the overclaiming the 237 thesis forbids. Honest
  real-vs-seeded labeling for any pre-launch demo.
- **Substance over skin.** The point of the spatial surface is *legibility of the
  clearing layer* — define-verify-record-settle made visible — not decoration. If a
  3D view doesn't help someone trust the work, cut it.

## 7. Sources

- Transcript theme guide + episodes: `openagents/docs/transcripts/README.md`;
  HUD/3D `111`,`116`,`117`,`119`,`123`,`208`; Commander/hand `170`,`171`,`175`,`176`,
  `177`,`179`,`185`; MMORPG `188`–`192`; later HUD/Autopilot `193`–`228`,`214`; launch
  `237`.
- Substrate: `@openagentsinc/three-effect` (`/Users/christopherdavid/work/three-effect`);
  `openagents/apps/openagents.com/apps/web/src/page/animations.ts` + `scene/animations/`.
- Product alignment: `docs/blitz/forge/2026-06-16-software-factory-reference-and-forge-mapping.md`,
  `docs/blitz/ROADMAP.md` (Epics B/G), `docs/blitz/compliance-guardrails.md`.
