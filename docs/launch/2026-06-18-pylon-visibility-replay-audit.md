# Pylon Visibility + Replay Audit — Live Boot-Up + Money-Loop Observability

Date: 2026-06-18. Off `origin/main` (`e8131c9` — `release(pylon): cut v1.0.1`).
Audit only — no production behavior changed, nothing deployed.

## Why this exists (owner's goal)

The owner needs **visibility into what is actually happening as people boot up
their Pylons** — are nodes coming online? are they claiming work, running,
getting verified, settling? are people posting on the Forum? — surfaced both as
a **live visualization** AND as a **replay**, across **three surfaces: the web
app, the desktop app, and the CLI**.

The honest one-line summary: **most of the data layer is built and live; the
visualization/replay *consumption* layer is partial, fragmented across surfaces,
and missing a unified live feed/timeline.** We are data-rich and
presentation-thin. We do not need a greenfield observability stack — we need to
*wire the existing live endpoints and the existing replay bundle into one
coherent live+replay surface across web/desktop/CLI*.

All findings below were grounded against live public endpoints on
`https://openagents.com` on 2026-06-18 (see [§5](#5-honest-current-state--what-an-operator-can-actually-see-right-now)).

---

## 1. Inventory table

Legend — **State:** Built / Partial / Missing. **Mode:** Live = current-state
at read; Replay = after-the-fact reconstruction of a sequence. **Surfaces:**
W = web app, D = desktop app, C = CLI.

### A. Pylon fleet / boot visibility

| Capability | Where it lives (file / endpoint) | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| Fleet counts (`onlineNow`, `seen24h`, `registeredTotal`, `walletReadyNow`, `assignmentReadyNow`) | `workers/api/src/public-pylon-stats.ts`, `public-pylon-stats-routes.ts`; `GET /api/public/pylon-stats` | Live (4s isolate cache) | ✅ | ⚠️ | ❌ | **Built** (web); not surfaced on desktop/CLI |
| Per-pylon recent state (`runtimeState`, `lastSeen`, `clientVersion`, online/wallet/assignment flags) | `public-pylon-stats.ts` `recentPylons[≤12]` | Live | ✅ | ⚠️ | ❌ | **Built** (web) |
| Presence registry + heartbeat (status, walletReady, capacity/health/load refs) | `workers/api/src/pylon-api.ts`; migrations `0123_pylon_agent_api.sql`, `0135_pylon_api_version_heartbeat_state.sql` | Live; staleness derived (5-min online window) | — | — | C (own node only) | **Built** |
| Pylon capacity funnel (registered→eligible→assigned→running→accepted→paid→settled + dark-capacity reasons) | `workers/api/src/pylon-capacity-funnel-live-routes.ts`; `GET /api/public/pylon-capacity-funnel` | Live | ⚠️ | ❌ | ❌ | **Built** (endpoint); thin web surfacing |
| Capacity funnel history (14d hourly / 180d daily snapshots) | migration `0155_pylon_capacity_funnel_snapshots.sql`; `GET /api/public/pylon-capacity-funnel/history` | Replay (snapshot series) | ⚠️ | ❌ | ❌ | **Built** (endpoint); not visualized |
| Own-node status / inventory / presence / wallet | `apps/pylon/src/index.ts` (`pylon status`, `inventory`, `presence`, `wallet status`, `balance`); `cli-catalog.ts` | Live (one-shot) | — | — | ✅ | **Built** (self only) |

### B. The live money loop

| Capability | Where it lives | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| Tassadar run summary (state, metrics, real-gradient, settlement rows) | `workers/api/src/public-tassadar-run-summary-routes.ts`; `GET /api/public/tassadar-run-summary` | Live | ✅ | ❌ | ❌ | **Built** |
| Enumerable settlements feed (per run, all settled receipts, real vs sim, contributor/window/challenge refs) | `workers/api/src/training-run-window-routes.ts` `routeReadRunSettlements`; `GET /api/training/runs/{runId}/settlements` | Live | ⚠️ | ❌ | ❌ | **Built** (endpoint); thin web surfacing |
| Verification-challenge records (worker→validator→verdict, `exact_trace_replay`, digest match) | `workers/api/src/training-verification.ts`; migration `0157_training_verification_challenges.sql`; `GET /api/training/verification/challenges` | Live | ⚠️ | ❌ | C (validate path) | **Built** (data); not surfaced as a feed |
| Self-serve open-window producer (maintains claimable window pool) | `workers/api/src/index.ts` `runSelfServeWindowProducerScheduled` (`#5396`) | Live (scheduled) | — | — | — | **Built** (background; no visibility surface) |
| Settlement receipts (`receipt.nexus...`, real/sim, idempotent) | `workers/api/src/nexus-treasury-payout-ledger.ts`; `GET /api/public/nexus-pylon/receipts/{ref}` | Live | ✅ | ⚠️ | ❌ | **Built** |
| Artanis admin tick decisions (dispatch/no-action/blocked decision log) | migration `0164_artanis_admin_tick.sql`; `artanis-administrator-tick.ts`, `artanis-tick-monitor.ts`; `GET /api/public/artanis/admin-ticks` | Live (append log) | ⚠️ | ❌ | ❌ | **Built** (endpoint); not visualized |

### C. Replay infrastructure

| Capability | Where it lives | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| Proof-replay bundle (ordered event sequence: proof_submitted → verified → settled → zap, with actors/stages/flows/camera-cues/captions/gaps) | `workers/api/src/public-proof-replay-routes.ts`; `GET /api/public/proof-replays`, `GET /api/public/tassadar-replays/first-real-settlement` | **Replay** | ✅ | ✅ | ❌ | **Built** |
| Replay math primitives (deterministic clock, render-plan, camera model, actor interpolation, shipment gates) | `packages/proof-replay/src/index.ts` | Replay | ✅ | ✅ | — | **Built** |
| Web replay viewer (play/pause/scrub/camera-mode/event-list) | `apps/web/src/scene/tassadarProofReplayElement.ts`; route `/tassadar/replay/{slug}` | Replay | ✅ | — | — | **Built** (flagged temporary 2.5D bridge in repo AGENTS) |
| Desktop replay pane | `apps/autopilot-desktop/src/shared/proof-replays.ts`, `src/ui/view.ts` | Replay | — | ✅ | — | **Built** |
| Headless clip render → mp4 (camera path, time window, fps) | `apps/web/spike/replay-r1/render-clip.mjs`; EPIC `#5346` (R-1a/R-2/R-3/R-4/R-5 done) | Replay → video | (offline) | (offline) | (offline) | **Built** (render-box / CI, not edge) |
| Exact-trace-replay verifier (worker digest vs validator re-execution) | `workers/api/src/tassadar-replay-validator.ts`; `apps/pylon/src/tassadar-trace-client.ts`; migration `0188_training_trace_contributions.sql` | Replay (verification) | — | — | C (`validate --auto`) | **Built** |
| **Unified cross-domain event timeline / cursor / stream** | — (none) | — | ❌ | ❌ | ❌ | **Missing** |

### D. World projection / 3D scene

| Capability | Where it lives | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| SpacetimeDB live world module (projection + interaction tables: run/entity/edge/proof/settlement/world_event + avatars/chat/emotes) | `apps/openagents-world-spacetimedb/` (Rust/WASM); deployed `spacetime.openagents.com` | Live | ✅ | ❌ | ❌ | **Built** (web only) |
| Tassadar→world projection bridge | `apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs` (reads `/api/public/tassadar-run-summary`) | Live (deterministic, replay-safe) | — | — | — | **Built** |
| Web `/tassadar` live scene (`oa-tassadar-run`, three-effect) | `apps/web/src/page/run.ts`, `scene/tassadarRunElement.ts`; route `/tassadar` | Live | ✅ | ❌ | ❌ | **Built** |
| three-effect scenes (pylon network graph, diamonds) | `apps/web/src/scene/pylonBezierNetworkElement.ts`, `pylonDiamonds.ts` | Live | ✅ | ❌ | ❌ | **Built** (embedded per-page; no unified dashboard) |

### E. Forum activity visibility

| Capability | Where it lives | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| Forum board / topics / posts feeds | `workers/api/src/forum-routes.ts`; `GET /api/forum`, `/api/forum/topics/{forumRef}`, `/api/forum/posts` | Live (read) | ✅ | ❌ | C (read/post/reply) | **Built** |
| Context activity feed (site/workroom) | `forum-routes.ts` `readForumContextActivity`; `GET /api/forum/contexts/{kind}/{id}/activity` | Live | ⚠️ | ❌ | ❌ | **Built** (scoped, not global) |
| Web forum UI | `apps/web/src/page/forum.ts` (SSR + inline JS, localStorage 10-min stale-while-revalidate) | Live (polled, cached) | ✅ | — | — | **Built** (polling, no live push) |
| **Global forum/agent activity stream** (cross-forum "what's happening now") | — (none) | — | ❌ | ❌ | ❌ | **Missing** |

### F. Operator / admin dashboards

| Capability | Where it lives | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| `/admin` overview (users, software orders, site state) | `workers/api/src/admin-overview-routes.ts`; `apps/web/src/page/loggedIn/admin/`; `GET /api/admin/overview` | Live (one-shot D1 JOIN) | ✅ | ❌ | ❌ | **Built** (no auto-refresh, no fleet money-loop view) |
| `/stats` page (token-usage aggregates + leaderboards) | `apps/web/src/page/loggedIn/stats/`; `GET /api/stats/token-usage/*` | Live | ✅ | ❌ | ❌ | **Built** (token usage ≠ pylon fleet) |
| Public stats panel (`/stats-old`, pylon-stats + forum + accounting) | `apps/web/src/page/...` consuming `/api/public/pylon-stats` | Live | ✅ | ❌ | ❌ | **Built** (static panels) |
| Projection-staleness contract (`live_at_read` / `rebuilt_on_transition` / `stored_snapshot`) | `workers/api/src/public-projection-staleness.ts` (`projection_staleness.v1`); zero-debt check `scripts/check-zero-debt-architecture.mjs` | — | — | — | — | **Built** (invariant, enforced) |

---

## 2. Gap analysis — what's "not fully built"

The owner is right: "we've got a lot of that infrastructure built, but it's not
fully built." Precisely:

### 2a. For a real LIVE visualization (boot-ups + forum + money loop)

What's **built and live** (the data is there):
- Fleet counts + per-pylon recent state (`/api/public/pylon-stats`) — proven
  live (`onlineNow=7`, `seen24h=11`, `registeredTotal=73` at audit time).
- Capacity funnel live + history endpoints.
- Tassadar run summary, enumerable settlements feed, verification challenges,
  settlement receipts, artanis tick log — all live public endpoints.
- A live 3D world (`spacetime.openagents.com`) that already projects run/entity/
  settlement/proof state and supports avatars/chat/emotes, wired into `/tassadar`.
- Forum topic/post feeds (live read).

What's **not fully built** for the live picture:
1. **No single live operator surface that combines the three signals**
   (boot-ups + forum + money loop). Today an operator must hit
   `pylon-stats`, `tassadar-run-summary`, `/settlements`, `forum/posts`, and
   `/tassadar` *separately*. There is no "what is happening right now" dashboard.
2. **No real-time push.** Everything web-side is **polling**: pylon-stats has a
   4s isolate cache and the client polls; forum UI polls with a 10-min
   stale-while-revalidate cache. The one live-push channel that exists
   (SpacetimeDB subscriptions) is used only by the `/tassadar` 3D scene — not by
   stats, settlements, or forum. Boot-ups/settlements do **not** stream.
3. **No live event of the money loop *as it happens*.** We can read the *current*
   settlement rows and verification challenges, but there is no live "a node just
   claimed / just submitted a trace / a validator just verified / a settlement
   just fired" event ticker. The artanis tick log is the closest thing and it's
   admin-decision-only, not the full pylon/verify/settle stream.
4. **Forum activity is scoped, not global + live.** There is a context activity
   feed (`/contexts/{kind}/{id}/activity`) but no global "recent across all
   forums" live stream, and the web forum is cached/polled, not pushed.
5. **Desktop and CLI see almost none of the fleet/money-loop live picture.**
   Desktop has the replay pane and update-feed/network-stats tests but no live
   fleet/forum/money dashboard. CLI is **self-only** — it shows *your* node's
   status/wallet/training, with the single bounded `training validate --watch`
   loop, but nothing fleet-wide, no forum feed, no settlement ledger, no
   earnings history.

### 2b. For a REPLAY of the same, across all three surfaces

What's **built**:
- A genuine, ordered **proof-replay bundle** (`proof_replay_bundle.v1`) with 11
  sequenced events, actors, stages, flows, camera cues, captions, and gap markers
  — proven live at `/api/public/tassadar-replays/first-real-settlement` and
  `/api/public/proof-replays`.
- Deterministic replay math (`packages/proof-replay`), a **web replay viewer**
  (`/tassadar/replay/{slug}`), a **desktop replay pane**, and a **headless
  clip→mp4 render pipeline** (camera-path driven).
- Capacity-funnel **history** snapshots (14d hourly / 180d daily) — a replayable
  time series of fleet shape.

What's **not fully built** for replay:
1. **Replay is curated, not general.** The proof-replay bundle today is built
   from **hand-authored builders** for two named stories
   (`first-real-settlement`, `launch-recognition-payments`). There is no
   "replay *any* run / window / pair / time-range" generator that assembles a
   bundle from the raw event-log tables on demand. New moments require new
   builder code.
2. **No replay of the *fleet boot-up* / *forum* dimensions.** The replay bundle
   covers the money loop (proof→verify→settle→zap). There is **no replay of
   "nodes coming online over time"** (the capacity-funnel history exists as data
   but is not turned into a replayable timeline visualization) and **no replay of
   forum activity** as a sequence.
3. **No unified event timeline to replay from.** Event-log tables exist per
   domain (`pylon_api_events`, `training_window_events`,
   `training_verification_events`, `forum_*_events`, `artanis_admin_tick_decisions`,
   `token_usage_events`) but there is **no unified, cursor-addressable timeline
   API** that orders events across domains. This is the single biggest structural
   gap for a general replay: you cannot say "replay everything that happened
   between T1 and T2" because there is no one ordered stream to read.
4. **CLI has no replay at all.** No `pylon replay` / `pylon timeline` command.
5. **Replay viewer is a flagged-temporary bridge.** The web 2.5D DOM/CSS scene
   (`tassadarProofReplayElement.ts`) is explicitly marked a temporary legacy
   bridge; the canonical renderer is the three-effect WebGL mount.

---

## 3. Recommended path (phased; reuse, don't rebuild)

The cheapest route to the owner's goal reuses every endpoint above and adds (a)
one unified live-activity stream and (b) one general replay generator, then
fans both out to the three surfaces. Greenfield is **not** warranted — the data
layer is done.

### Authority / projection-safety invariants (apply to every phase)

- **Public-safe refs only.** No mnemonics, invoices, preimages, tokens, raw
  traces/logs, prompts, provider material, or payout targets — the proof-replay
  bundle already enforces this with a regex scan and `claimScope:
  evidence_presentation_only`; any new feed/timeline MUST reuse the same scrub.
- **Staleness contracts.** Every new projection carries a `projection_staleness.v1`
  contract (`live_at_read` / `rebuilt_on_transition` / `stored_snapshot`); the
  zero-debt check enforces it. A live stream is `live_at_read`; a replay bundle
  is a `stored_snapshot`/reconstruction with `claimScope:
  evidence_presentation_only`.
- **Receipt-first / honest idle.** Honest zeros and `emptyState` envelopes, never
  fabricated metrics (the `/tassadar` scene and run-summary already do this).
- **SpacetimeDB projection rules.** The world module is a *projection*, not a
  truth source; settlement/training/credential authority stays in D1. Any new
  world projection goes through the deterministic, replay-safe service-reducer
  bridge.
- **No new spend/settlement authority.** This is observability; it arms nothing.

### Phase 0 — Consolidate the live read surface (days, web-first)

Reuse the existing live endpoints; add **one operator live-activity page** that
composes the three signals already available:
- boot-ups: `/api/public/pylon-stats` (recentPylons + counts) and
  `/api/public/pylon-capacity-funnel`;
- money loop: `/api/public/tassadar-run-summary` + `/settlements` +
  `/api/training/verification/challenges` + `/api/public/artanis/admin-ticks`;
- forum: `/api/forum/posts` + context activity.

No new backend. This is a Foldkit page that polls the existing endpoints on a
short interval and lays them out as one "what's happening now" view. Closes the
"five separate endpoints" gap (2a-1) immediately.

### Phase 1 — Unified public activity timeline (the keystone)

Build **one** read-only, cursor-addressable, public-safe **activity-event
endpoint** that unions the existing per-domain event tables into a single
ordered stream:
- Source: `pylon_api_events` (online/heartbeat/claim/closeout),
  `training_window_events`, `training_verification_events` (worker→validator→
  verdict), `nexus_payment_authority_receipt_recorded` (settlements),
  `forum_*_events`, `artanis_admin_tick_decisions`.
- Output: `{ schemaVersion, generatedAt, staleness, cursor, events[] }` where each
  event is a public-safe `{ ts, kind, actorRef?, targetRef?, refs[], text }`
  (reuse the proof-replay `ReplayEvent` shape and its scrub).
- Two reads off the same shape: **live tail** (`?since=<cursor>`, `live_at_read`)
  and **range replay** (`?from=&to=`, `stored_snapshot`).

This single endpoint is the keystone: it powers both the live ticker (2a-3) and
general replay (2b-1, 2b-3) for boot-ups + verify + settle + forum, and removes
the need for new hand-authored replay builders.

### Phase 2 — General replay generator + fleet/forum replay

- Generalize the proof-replay bundle builder to assemble a bundle from the
  Phase-1 timeline for **any** run/window/pair/time-range (not just the two named
  stories), keeping the curated builders as named presets.
- Turn the capacity-funnel **history** series into a replayable "nodes coming
  online over time" timeline visualization (data already exists).
- Add forum activity as a replayable track on the same timeline.

### Phase 3 — Push + fan-out to desktop and CLI

- **Live push (web):** extend the SpacetimeDB world projection (already
  subscription-based and wired to `/tassadar`) to carry the Phase-1 activity
  events, replacing polling for the live ticker; or add an SSE tail on the
  Phase-1 endpoint. Reuses the only live-push channel we already operate.
- **Desktop:** the desktop already embeds the replay pane and has
  update-feed/network-stats plumbing — point it at the Phase-0 live page + the
  Phase-1/2 timeline + replay bundle.
- **CLI:** add `pylon activity --watch` (live tail of the Phase-1 endpoint,
  fleet-wide, public-safe) and `pylon replay <run|window|pair|range>` (pull a
  Phase-2 bundle and render an ASCII/JSON timeline). This closes the CLI gaps
  (self-only, no replay, no money-loop) with the smallest possible surface,
  reusing the same endpoint the web/desktop use.

### Phase 4 — Productionize replay clips (in flight already)

EPIC `#5346` (headless clip→mp4) already renders camera-path-driven clips from a
bundle. Once Phase 2 makes bundles general, the clip pipeline can render *any*
moment (boot surge, a verify pair, a settlement) as a shareable clip with no new
render work — only bundle selection.

---

## 4. The single most valuable next step

If only one thing ships: **Phase 1 — the unified public activity timeline
endpoint.** It is the keystone that unblocks both halves of the owner's goal
(live + replay) across all three surfaces, it reuses event data we already
write, and it removes the hand-authored-builder bottleneck that currently makes
replay a per-story coding task. Phase 0 (the composed live page) is the fastest
*visible* win and can land in parallel against today's endpoints.

---

## 5. Honest current state — what an operator can actually see RIGHT NOW

Grounded against live `https://openagents.com` on 2026-06-18:

- **Web, fleet boot-ups:** `GET /api/public/pylon-stats` → **HTTP 200**, live:
  `pylonsOnlineNow=7`, `pylonsSeen24h=11`, `pylonsRegisteredTotal=73`,
  `pylonsWalletReadyNow=1`, `pylonsAssignmentReadyNow=1`, `recentPylons` length
  11. Capacity funnel + history both **HTTP 200**.
- **Web, money loop:** `GET /api/public/tassadar-run-summary` → **200**,
  `runState=active`, 3 settlement rows. `GET /api/training/runs/run.tassadar.executor.20260615/settlements`
  → **200**, `openagents.training_run_settlements.v1`, 3 rows: `[simulation 5,
  real_bitcoin 1000, real_bitcoin 5]` (matches the roadmap's 1,005-real-sats /
  1,010-with-sim reconciliation note). Artanis admin-ticks **200**.
- **Web, replay:** `GET /api/public/proof-replays` and
  `/api/public/tassadar-replays/first-real-settlement` → **200**,
  `proof_replay_bundle.v1`, **11 sequenced events** with actors/cameraCues/
  captions/gaps. `/tassadar` and `/tassadar/replay/first-real-settlement` pages →
  **200**.
- **Web, world:** SpacetimeDB module deployed at `spacetime.openagents.com`
  (bare HTTP root returns 403 — expected; it's a module/websocket endpoint, not a
  page). Wired into `/tassadar` via committed client bindings.
- **Web, forum:** `/api/forum` and `/api/forum/posts` → **200** (live read,
  polled/cached UI).
- **Desktop:** can play the proof-replay bundle (replay pane). **Cannot** show a
  live fleet/forum/money dashboard.
- **CLI:** can show **its own** node status, inventory, presence/heartbeat,
  wallet status, balance, training status, and run one bounded
  `training validate --auto --watch` loop. **Cannot** see other nodes, fleet
  counts, the settlements feed, forum activity, or any replay.

**Target vs reality:** the *data* for a full live+replay picture of boot-ups +
forum + money loop exists and is live on the web API today. What an operator
*cannot* do today is see it as **one** live activity surface, replay **arbitrary**
moments (only two curated stories), or get **any** of the fleet/money/forum live
picture on **desktop or CLI**. Closing that is wiring + one keystone timeline
endpoint, not a new platform.
