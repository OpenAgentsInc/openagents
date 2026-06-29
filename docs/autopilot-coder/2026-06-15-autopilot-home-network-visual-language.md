# Autopilot home screen — pylon-network visual language

Date: 2026-06-15. Owner: product. Status: spec + iteration guide for the
fullscreen Autopilot Desktop home screen (issue #5049, ships in the Autopilot
v1.0-rc, #5046). This doc is the **source of truth for what every visual element
means** so future agents can iterate it and so the choices can be reflected in
the product-promise registry. If you change a visual mapping, update this doc
and the relevant promise copy in the same change.

## 1. Principle: one screen, all signal

The home view is **one fullscreen three-effect canvas with a stats overlay and
nothing else** — no sidebar, no static cards, no hardcoded numbers. Every pixel
is either (a) the live network visualization or (b) a live stat. If a number
isn't backed by `GET /api/public/pylon-stats`, it does not appear. We never
render a fake "online" or a placeholder count (mirrors the worker's no-guess
rule in `public-pylon-stats.ts`).

Data source: `https://openagents.com/api/public/pylon-stats` →
`PublicPylonStats` (`apps/openagents.com/workers/api/src/public-pylon-stats.ts`).
Polled on an interval (default 15s); the whole scene is a pure function of the
latest snapshot.

## 2. The center pylon — the network's heartbeat

The center element is the **homepage pylon** (`pylonDiamonds.ts`, reused
verbatim: the refraction/diamond shader with light beams). It is the single
most important object on screen and represents **the OpenAgents network as a
living whole** — not the local node. The local node is one of the lit nodes in
the graph around it (§3).

**The blue glow = live activity.** This is the headline mapping. The shader's
`lightPulse` uniform (and beam brightness) is driven by an `activityIntensity`
in `[0,1]`:

- **Idle (intensity 0):** dim, slow breathing pulse (base ~0.30). The network is
  up but no work is flowing.
- **Active (intensity → 1):** bright, faster, saturated blue pulse. Work for the
  run is being done / compute is going.

`displayedPulse = base + intensity * span`, with the existing
`sin(seconds·rate)` breathing layered on top so it never looks frozen. Brighter
**and** faster as intensity rises — motion reads as "alive," brightness reads as
"busy."

### Activity intensity definition (initial)

`activityIntensity = clamp01( w1·f(sessions) + w2·f(nip90) + w3·f(training) )`
from the live snapshot:

| Signal | Field(s) | Reads as |
| --- | --- | --- |
| Sessions running | `pylonSessionsOnlineNow` | coding/agent work in flight |
| Market settling | `nip90MarketSettlementStats.{compute,data,labor}.jobsSettled24h` / `satsSettled24h` | paid compute/data/labor flowing |
| Training progress | `trainingModelProgressContributors` | the run is advancing |

`f(x)` is a soft saturating curve (e.g. `1 - 1/(1 + x/k)`) so the glow responds
immediately to the first unit of work and asymptotes rather than clipping. Start
weights `w1=w2=w3=1/3`; tune in this doc, not inline. **Blue is reserved for
activity** — do not use blue for chrome/idle states.

## 3. The network graph — online pylons around the center

Around the center we adapt the **bezier graph already in desktop**
(`@openagentsinc/three-effect` `trainingRunView` / `bezierNodes`; the training
scene in `view.ts` uses the same component). Here it visualizes the **overall
pylon network**:

- **Nodes = pylons.** Seeded from `recentPylons[]` and the online counters
  (`pylonsOnlineNow`, `pylonsWalletReadyNow`, `pylonsAssignmentReadyNow`). When
  there are more online than discrete `recentPylons`, the remainder render as
  anonymous ambient nodes so the count reads honestly.
- **Edges = bezier curves from each pylon to the center**, expressing
  "every pylon contributes to one network."
- **Node tone:**
  - **assignment-ready / working** (`assignmentReadyNow` or active
    `runtimeState`) → **lit blue**, edge animated toward center (flow).
  - **online, idle** (`onlineNow`, not assignment-ready) → steady cool white,
    static edge.
  - **recently seen, offline now** → dim grey, no edge (present for context,
    e.g. `pylonsSeen24h`).
- **Motion along an edge = work moving** from that pylon into the run. No motion
  = connected but idle. Edge flow brightness shares the §2 activity curve.

## 4. Palette and what each color means

From `pylonDiamonds.ts` + the overlay. Colors carry meaning; don't repurpose.

| Token | Hex | Meaning |
| --- | --- | --- |
| Background | `#0c0f13` | the void; canvas base (`backgroundColor 0x0c0f13`) |
| Pylon blue-white | `#d6f6ff` / `#d8f4ff` | the network substrate (the diamond/beams) |
| Activity blue | bright cyan-blue end of the glow ramp | **work happening** (only ever activity) |
| Idle white | desaturated cool white | online but no work |
| Offline grey | low-value grey | seen-but-gone, context only |
| Blocked/withdrawn | reuse training "blocked" tone (warm/red) | a stalled/blocked signal, sparingly |

## 5. Stat overlay and the number-roll animation

Stats are an **HTML overlay** above the canvas (three-effect `htmlOverlay`
primitives or a foldkit overlay layer), grouped into a few corners/edges so the
center pylon stays clear:

- **Primary (top/center):** `pylonsOnlineNow` (the hero number), with
  `pylonSessionsOnlineNow` as the "work in flight" sub-stat.
- **Earnings:** NIP-90 `satsSettled24h` / `satsSettledTotal` (sum across
  compute/data/labor); gate on `nexusAcceptedWorkSettlementGate` /
  `earningLaunchGate` so we only show paid totals when the gate says it's allowed.
- **Run:** `trainingAssignedContributors` / `trainingAcceptedContributors` /
  `trainingModelProgressContributors`.
- **Reach:** `pylonsSeen24h`, `pylonsRegisteredTotal`,
  `pylonsWalletReadyNow`, `pylonsAssignmentReadyNow`.
- **Footer:** `asOfLabel` (honest "as of" time — the data is last-reported, not
  live-per-frame).

**Every updating number uses the homepage countdown roll** — the slot-text
digit-roll CSS from `pylonCountdownElement.ts` (scoped `.slot-text` / `.char-slot`
/ `.char-face` with a transform-based roll). When a counter changes between polls
the digits roll to the new value; this is the shared "numbers updating" language
across the homepage and Autopilot. Autopilot Desktop now scopes that structure in
`apps/autopilot-desktop/src/ui/styles.css` and renders slot characters from
`apps/autopilot-desktop/src/ui/view.ts`, so the home overlay no longer uses a
plain tabular-number placeholder.

## 6. Empty / unavailable states

- `status: "unavailable"` or `available: false` → show the network as **dormant**
  (dim center, no nodes) with the `asOfLabel`/error reason; never invent counts.
- Zero online pylons → center dim, graph empty, hero number `0` (rolled). This is
  the honest "be the first pylon" state.

## 7. How this maps to product promises

This visualization is the live, user-facing face of several promises — keep them
consistent (registry: `/api/public/product-promises`):

- **Pylon online / earning-network counters** (`gate.public.pylon.earning_network_counters.v1`,
  `definition.public.pylon_stats.online_now.v1` et al.) — the hero count + graph
  nodes are exactly these counters; the glow shows the `assignment_ready` /
  earning activity the gate governs.
- **Accepted-work settlement** (`nexusAcceptedWorkSettlementGate`) — the sats
  totals are shown only when this gate allows public paid totals.
- **Training contributors** — the run stats substantiate the decentralized
  training launch promise.

When a promise's copy or gate changes, re-check that the overlay still only
shows what the gate permits, and update §5 here.

## 8. Iteration guide (where each knob lives)

- **Glow ramp / activity weights:** §2 mapping → the scene module's
  `activityIntensity` computation + the `pylonDiamonds` `lightPulse` drive
  (`PylonDiamondsHandle.setActivity(intensity)`). Autopilot Desktop composites
  the same homepage renderer through
  `apps/autopilot-desktop/src/ui/pylon-diamonds-element.ts`; the web renderer's
  transparent-background option exists only so the shader can sit over the
  three-effect network graph without hiding it.
- **Node tones / edge flow:** §3 → the pylon-network options builder (analogous
  to `trainingRunVisualizationOptionsFromSnapshot`).
- **Stat selection / layout:** §5 → the overlay component.
- **Palette:** §4 → shared color tokens; change here first.
- **Poll cadence / empty states:** §6 → the bun-side stats fetch + model.

Keep the scene a pure function of the `PublicPylonStats` snapshot so it stays
testable headlessly (feed synthetic snapshots; assert glow intensity and node
counts) and so it can be exercised **as new pylons join the network**.
