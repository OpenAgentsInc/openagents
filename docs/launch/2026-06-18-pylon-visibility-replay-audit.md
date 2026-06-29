# Pylon Visibility + Replay Audit — Live Boot-Up + Money-Loop Observability

Date: 2026-06-18. Off `origin/main` (`e8131c9` — `release(pylon): cut v1.0.1`).
Audit only — no production behavior changed, nothing deployed.

Roadmap refresh: later on 2026-06-18, after reading the full `docs/launch/`
folder, this document was expanded from a gap audit into the launch roadmap for
bringing Pylon visibility/replay fully live. The companion launch docs update a
few baselines: stable `npx @openagentsinc/pylon` is now on the v1.0 line, the
evidence pack is live enough for the honest-scoped video, product promises are
served from source as `2026-06-19.1`, and the remaining launch/product blockers
are visibility/replay productization, broad self-serve earning scale,
Windows/WSL coverage, Spark-helper auto-start/readiness, and owner-signed claim
upgrades.

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
| Proof-replay bundle (ordered event sequence: proof_submitted → verified → settled → zap, with actors/stages/flows/camera-cues/captions/gaps) | `workers/api/src/public-proof-replay-routes.ts`; `GET /api/public/proof-replays`, `GET /api/public/tassadar-replays/first-real-settlement`; `apps/pylon/src/public-activity-cli.ts` (`pylon replay`) | **Replay** | ✅ | ✅ | ✅ | **Built** (CLI text/JSON, no video) |
| Replay math primitives (deterministic clock, render-plan, camera model, actor interpolation, shipment gates) | `packages/proof-replay/src/index.ts` | Replay | ✅ | ✅ | — | **Built** |
| Web replay viewer (play/pause/scrub/camera-mode/event-list) | `apps/web/src/scene/tassadarProofReplayElement.ts`; route `/tassadar/replay/{slug}` | Replay | ✅ | — | — | **Built** (flagged temporary 2.5D bridge in repo AGENTS) |
| Desktop replay pane | `apps/autopilot-desktop/src/shared/proof-replays.ts`, `src/ui/view.ts` | Replay | — | ✅ | — | **Built** |
| Headless clip render → mp4 (camera path, time window, fps) | `apps/web/spike/replay-r1/render-clip.mjs`; EPIC `#5346` (R-1a/R-2/R-3/R-4/R-5 done) | Replay → video | (offline) | (offline) | (offline) | **Built** (render-box / CI, not edge) |
| Exact-trace-replay verifier (worker digest vs validator re-execution) | `workers/api/src/tassadar-replay-validator.ts`; `apps/pylon/src/tassadar-trace-client.ts`; migration `0188_training_trace_contributions.sql` | Replay (verification) | — | — | C (`validate --auto`) | **Built** |
| **Unified cross-domain event timeline / cursor / stream** | `packages/public-activity-timeline`; `workers/api/src/public-activity-timeline-routes.ts`; `GET /api/public/activity-timeline`; `GET /api/public/activity-timeline/stream`; `apps/web/src/page/activity.ts` (`/activity`); `apps/pylon/src/public-activity-cli.ts` (`activity`, `timeline`) | Live + Replay source | ✅ | ❌ | ✅ | **Built** (endpoint + CLI + SSE + web page; desktop pending) |

### D. World projection / 3D scene

| Capability | Where it lives | Mode | W | D | C | State |
| --- | --- | --- | --- | --- | --- | --- |
| SpacetimeDB live world module (projection + interaction tables: run/entity/edge/proof/settlement/world_event + avatars/chat/emotes) | the deleted legacy world module (Rust/WASM); deployed `spacetime.openagents.com` | Live | ✅ | ❌ | ❌ | **Built** (web only) |
| Tassadar→world projection bridge | the deleted legacy bridge scripts (reads `/api/public/tassadar-run-summary`) | Live (deterministic, replay-safe) | — | — | — | **Built** |
| Public activity timeline→world event bridge | the deleted legacy bridge scripts (reads `/api/public/activity-timeline`) | Live (deterministic, replay-safe) | — | — | — | **Built** |
| Web `/tassadar` route | `apps/web/src/page/run.ts`; route `/tassadar` | Retired guardrail | ✅ | ❌ | ❌ | **Retired** (plain route now points people to the Verse surface and public summary API) |
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
1. **Single live operator surface is web-only so far.** The `/activity` page now
   combines boot-ups, forum activity, and the money loop over the public
   activity timeline, but desktop still lacks the same "what is happening right
   now" dashboard.
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
4. **CLI replay is emerging, not complete across surfaces.** `pylon timeline`
   and `pylon replay` now expose bounded public activity and generated replay
   tracks for agents, but desktop/web generated-bundle controls and production
   replay clips still remain follow-on work.
5. **Replay viewer is a flagged-temporary bridge.** The web 2.5D DOM/CSS scene
   (`tassadarProofReplayElement.ts`) is explicitly marked a temporary legacy
   bridge; the canonical renderer is the three-effect WebGL mount.

---

## 3. Roadmap to fully live (reuse, don't rebuild)

"Fully live" means a new contributor boots Pylon, claims work, submits/verifies,
settles, and posts/discusses in the Forum, while an operator can see the same
sequence **live** and **replay it later** from the web app, Autopilot Desktop,
and the CLI. The finished system must also generate directed replay clips from
the same evidence without adding settlement, payout, deployment, or public-claim
authority.

Greenfield is not warranted. The launch folder says the hard data paths are
already in place: stable Pylon install (`@openagentsinc/pylon@1.0.0`), public run
summary, reconciled settlement rows, public receipts, pylon-stats, capacity
funnel + history, proof-replay bundles, SpacetimeDB world projection, Forum
feeds, and a render-box clip pipeline. The remaining work is status cleanup,
one keystone timeline API, **agent/CLI retrievability first**, product surfaces
after that, and proof gates.

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
- **Evidence-bound motion.** If it moves, pulses, flows, or bursts, it must name
  the public refs that caused it. The `/tassadar` motion-policy work from the
  June 17 audits is the baseline, not an optional design preference.

### Phase 0 — Normalize evidence and status (same-day polish)

Close the small evidence-surface gaps before building new presentation on top:

- Add or alias the public settlements route so both the evidence pack and the
  operator UI can cite a stable public URL. Implemented on 2026-06-18: the
  canonical agent-facing route is now
  `/api/public/training/runs/{runRef}/settlements`, with the legacy
  non-`/public/` route retained for compatibility.
- Add a focused public verification-challenge read route (or a documented
  `focusRef` route) so `training.verification.challenge.*` can be dereferenced
  directly, not only as embedded run-summary data. Implemented on 2026-06-18:
  `/api/public/training/verification-challenges/{challengeRef}` is the public
  challenge dereference path.
- Reconcile every public aggregate that can still show simulation rows as real
  settled sats. The run summary is reconciled at 1,005 real sats; any remaining
  `/api/public/pylon-stats` 1,010-style aggregate must exclude
  `realBitcoinMoved:false` rows before it is used in the live dashboard.
- Label the remaining V1.0 gates plainly in the UI: install is now stable and
  one auto-stream visibility sequence is captured, but broad "anybody installs
  on any platform and automatically earns" copy is still not proven; the two
  world-first claims remain RED/qualified pending owner-signed upgrades.
- Add smoke coverage for the public routes above, the real-vs-simulation
  reconciliation, and the absence of private material.

Exit: a skeptic can dereference run, settlement, receipt, verification, promise,
and install refs from one evidence pack without discovering URL drift.

### Phase 1 — Ship the unified public activity timeline (keystone)

Build **one** read-only, cursor-addressable, public-safe timeline endpoint:
`GET /api/public/activity-timeline`.

Contract:

- `schemaVersion: "openagents.public_activity_timeline.v1"`.
- Envelope: `{ generatedAt, staleness, nextCursor, sourceLag, events[] }`.
- Cursor: stable monotonic key such as
  `{ts}:{sourceKind}:{eventRef}`; support `?since=<cursor>`, `?limit=`, and
  bounded `?from=&to=` range reads.
- Event shape:
  `{ eventRef, ts, kind, actorRef?, targetRef?, runRef?, windowRef?, refs[],
  amountSats?, realBitcoinMoved?, state?, sourceKind, sourceRefs, text,
  caveatRefs[] }`.
- Event kinds should be typed and finite: `pylon_registered`,
  `pylon_heartbeat`, `wallet_ready`, `assignment_ready`, `window_opened`,
  `work_claimed`, `trace_submitted`, `verification_queued`,
  `verification_verified`, `verification_rejected`, `settlement_recorded`,
  `real_bitcoin_moved`, `forum_topic_created`, `forum_posted`,
  `artanis_tick`, `capacity_snapshot`, and `projection_gap`.

Initial sources:

- `pylon_api_events` and registration/heartbeat state for boot and readiness.
- `training_window_events`, lease/claim rows, trace-contribution rows, and
  verification events for claim→submit→replay→verdict.
- Settlement receipt rows, not payout intent rows, for paid/settled events.
- Forum topic/post/context activity rows for public activity.
- Artanis admin-tick decisions for dispatch/no-action/blocked context.
- Capacity-funnel snapshots for aggregate fleet-shape events.

Rules:

- If a source table lacks enough public-safe detail, emit a
  `projection_gap` event with blocker refs instead of guessing.
- Never infer `real_bitcoin_moved` from amount or state; only a receipt with
  `realBitcoinMoved:true` may emit it.
- Attach `projection_staleness.v1` and source lag. A stale source must be visible
  in the envelope, not hidden behind a fresh timeline timestamp.

Exit: the timeline can answer "what happened since cursor X?" and "what happened
between time A and B?" across boot-ups, work, verification, settlement, Forum,
and operator ticks using one public-safe schema. It is not done until it has a
documented machine-readable contract, example `curl` commands, and fixture JSON
that agents can consume without scraping UI.

### Phase 2 — Agent/CLI programmatic access first

Before building the live UI, make the data easy for agents and scripts to fetch,
page, filter, replay, and cite. This is the first acceptance surface for the
timeline.

API/docs:

- Publish agent-readable endpoint docs for
  `/api/public/activity-timeline`, including cursor semantics, supported
  filters, staleness fields, event-kind enum, example responses, and private-
  material redaction guarantees.
- Add an endpoint index or manifest entry so agents can discover the timeline,
  settlement, verification, receipt, proof-replay, and product-promise routes
  without reading launch prose.
- Provide "one-screen verification" recipes mirroring the evidence pack:
  `curl` timeline tail, bounded range replay, per-run settlement rows, one
  verification challenge, one receipt, and product-promise states.
- Add JSON fixtures for empty, active, stale, replay-range, simulation-only, and
  real-Bitcoin event sequences.

CLI:

- `pylon activity --watch [--json] [--since CURSOR] [--filter work,verify,...]`
  tails public-safe fleet activity.
- `pylon timeline --from ... --to ... --json` fetches bounded historical events.
- `pylon replay --run ...|--window ...|--range ... --format text|json` fetches
  generated replay bundles and prints an ASCII/JSON event track.
- `pylon receipts --run ... --json` summarizes settlement rows and links receipt
  URLs.
- `pylon evidence-pack --run ... --json` prints the machine-readable refs an
  agent needs to verify the run, receipts, promises, and current blockers.
- Keep own-node commands (`status`, `presence`, `wallet status`) separate from
  public fleet commands so users do not confuse self state with network state.

Agent acceptance:

- A fresh coding agent can answer, from JSON only, "who came online, who claimed,
  what got verified/rejected, what settled, what was simulation, what Forum
  posts happened, and what is still blocked?"
- The CLI and endpoint docs must make every displayed event dereferenceable via
  public URLs or explicit blocker refs.
- No browser, DOM, canvas, localStorage, or visual route may be required for the
  agent to reconstruct the evidence sequence.

Exit: programmatic retrieval is the source of truth for the UI build, not a
follow-on convenience after the UI.

### Phase 3 — Web live-activity surface (after programmatic access)

Ship a Foldkit page that consumes the Phase-1/2 programmatic contract. Candidate
routes: `/activity`, `/tassadar/activity`, or an admin-linked "Pylon Live" page.

Required panes:

- Fleet: online/seen/registered, wallet-ready, assignment-ready, capacity funnel,
  recent pylons, dark-capacity reasons, and stale-source labels.
- Money loop: run state, open windows, latest claims/submissions/verdicts,
  settlement receipt rows, real vs simulation split, autonomous-settlement gate
  status, and Artanis tick decisions.
- Forum: latest public product-promise, release-candidate, and run-context posts
  with public author refs and timestamps.
- Timeline: a single reverse-chronological event list with filters for
  `boot`, `work`, `verify`, `settle`, `forum`, and `operator`.
- Proof drawer: selected event shows source refs, public URLs, caveats,
  staleness, and any product-promise blockers.
- World link: selected timeline events can focus `/tassadar` entities when the
  world projection has matching refs.

Start with 3-5 second polling. Do not wait for push to get the operator view
online, but do not let the page consume private or undocumented shapes.

Exit: an operator no longer needs five tabs to answer "are people booting,
working, getting verified, getting paid, and talking?", and an agent can
reproduce the same answer from the API/CLI without seeing the page.

### Phase 4 — Live push + world projection

Once the timeline endpoint exists, add a live tail:

- First choice: SSE on `GET /api/public/activity-timeline/stream?since=...`
  because it is the least invasive way to tail the same public event shape.
- Second layer: bridge the same event shape into `openagents-world` as
  public-safe `world_event` rows so `/tassadar` can animate live events through
  the existing SpacetimeDB subscription path. Implemented on 2026-06-18 for
  deterministic `world_event` projection from
  `/api/public/activity-timeline`; visual motion consumption remains follow-on.
- Keep polling as a fallback and expose reconnect/stale state.
- Preserve the June 17 motion contract: the world may animate only events with
  `sourceRefs`, `generatedAt`, and expiry/staleness metadata.

Exit: boot/claim/verify/settle/forum events appear live without refresh, but the
Worker/D1 timeline remains the source projection and SpacetimeDB remains a
projection.

### Phase 5 — General replay generator

Generalize proof replay from hand-authored named stories to generated bundles:

- Add `GET /api/public/proof-replays?from=&to=&runRef=&windowRef=&actorRef=&kind=`
  or a sibling route that builds a `proof_replay_bundle.v1` from Phase-1 timeline
  events.
- Keep curated bundles (`first-real-settlement`,
  `launch-recognition-payments`) as named presets and regression fixtures.
- Convert boot-ups and capacity-funnel history into a "fleet boot" replay track.
- Convert Forum events into a "discussion" replay track.
- Generate actors/stages/flows/captions/camera cues from event kinds and source
  refs, then run the existing `assertProofReplayBundleShipmentGate` and source
  coverage checks.
- Store generated replay bundles as reconstructable snapshots with the input
  cursor/range/filter in the manifest, not as authority.

Exit: a caller can replay any run/window/pair/time-range without writing a new
builder.

### Phase 6 — Replay clips as a production service

The render-box pipeline already exists: Playwright/headless Chromium drives the
three-effect proof-replay scene and `ffmpeg` emits mp4. Productize it without
putting Chrome or `ffmpeg` in the Cloudflare Worker.

Build:

- A job schema: `{ replayBundleRef|timelineRange, startSecond, duration, fps,
  resolution, cameraPathRef|cameraPath, outputKind }`.
- A local/CI/Container render worker that runs `render-clip.mjs` or its
  production successor, writes an mp4 plus a render manifest, and uploads to R2.
- A Worker route that creates/list jobs and serves finished clips/manifests; it
  does not render frames.
- A camera-path DSL with explicit keyframes and simple verbs (`hold`, `orbit`,
  `follow`, `frame_actor`, `frame_settlement`) compiled into the existing
  camera-path input.
- Regression renders for one curated story and one generated timeline bundle;
  verify nonblank WebGL frames and camera-path differences.

Exit: "pick this moment, move the camera here, generate a clip" is one command
or API call, with a public-safe manifest and no interactive widget dependency.

### Phase 7 — Desktop fan-out and UI parity

Use the same timeline and replay endpoints; do not create per-surface backends.
CLI parity already starts in Phase 2. Desktop/UI work comes after the agent
contract is stable.

- Add a live activity strip or pane to the Network/Training surfaces so the app
  is not just an immersive scene when a local node is unavailable.
- Replay pane accepts generated bundles and range filters, not only curated
  slugs.
- The coding-surface yellow promise remains separate: desktop live visibility can
  ship before the full in-window coding composer or signed packaged-node CS-B1
  gate, but downloaded builds still need the packaged node/signing path for a
  normal-user "fully live" experience.
- Desktop must not introduce page-only fields that are absent from the
  public/CLI contract. Any extra UI convenience must still point back to the
  same event refs.

Exit: web, desktop, and CLI answer the same questions from the same refs, with
the CLI/agent contract remaining the canonical consumption contract.

### Phase 8 — Ops, smokes, and owned-infra automation

Make the visibility system operational:

- Add owned-infra scheduled checks for timeline freshness, source lag, SSE
  health, SpacetimeDB bridge health, render-box queue health, R2 clip
  availability, and public route status. Do not add GitHub-hosted CI.
- Add browser/canvas smokes for `/tassadar`, replay pages, and the live-activity
  page: nonblank WebGL canvas, no anonymous motion, proof drawer route returns
  200, text fits at desktop/mobile sizes.
- Add API tests for private-material redaction, staleness envelopes, cursor
  ordering, simulation exclusion, and source coverage.
- Add a runbook in `docs/DEPLOYMENT.md` or a launch sub-runbook for operating
  the timeline, stream, world bridge, and render worker.

Exit: a stale or broken visibility surface is detected as an operations problem,
not discovered by the owner manually refreshing tabs.

### Phase 9 — Product-promise and launch gates

The visibility/replay system is "fully live" only after the evidence and copy
gates agree:

- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` remains RED even after
  the #5438 auto-stream visibility capture until the remaining broad-claim
  blockers clear: scale methodology, Windows/WSL coverage for "anybody" wording,
  and Spark-helper auto-start/readiness evidence for normal contributors.
- The live activity timeline must continue to show exact event sequences without
  manual backfill.
- Generated replay bundles and render-box clips for exact sequences must be
  produced from the timeline, not hand-authored; the #5438 manifest is committed
  as local evidence while production R2 upload remains owner-gated.
- Product-promises registry, AGENTS.md/INSTALL.md, evidence pack, and launch docs
  cite the same refs and states.
- World-first claims stay only in their qualified wording until the owner-signed
  receipt-first upgrades land.
- Windows support and Spark-helper auto-start stay explicit open platform
  coverage gates for "anybody" language.

Exit: the public claim can move from "the data exists and the loop is proven in
bounded pieces" to "a normal contributor can join, be observed live, be replayed,
and produce a clip from public evidence."

### Issue series to file (epics + child issues)

File these as `roadmap` issues, in this order. Every issue body should link back
to this audit, preserve the invariants above, and state explicitly that the work
adds observation/projection only: no settlement, payout, deployment, accepted-
work, or public-claim authority. The first epic deliberately makes API/CLI/
agent retrievability the acceptance surface before any UI work starts.

#### EPIC 1 — Programmatic Pylon activity evidence spine

Body summary: Build the machine-readable public evidence spine for Pylon
visibility and replay. The outcome is a public-safe, cursor-addressable timeline
API, agent-readable docs/fixtures, and CLI commands that let agents reconstruct
boot, claim, verify, settle, Forum, and blocker state without scraping the web
UI. This epic is the prerequisite for live UI, desktop, world projection, replay
generation, and clips.

1. **Define `openagents.public_activity_timeline.v1` schema and fixtures**
   - Body summary: Specify the timeline envelope, cursor, event shape,
     finite event-kind enum, staleness/source-lag fields, caveat refs, and
     private-material redaction rules. Add JSON fixtures for empty, active,
     stale, replay-range, simulation-only, and real-Bitcoin sequences. Include
     contract tests for ordering, redaction, and simulation-vs-real fields.
   - Implementation status (2026-06-18, issue #5413): added the shared
     `@openagentsinc/public-activity-timeline` package with Effect schemas,
     cursor helpers, public-safe assertion helpers, README contract docs, and
     fixtures/tests for empty, active, stale, replay-range, simulation-only, and
     receipt-backed real-Bitcoin sequences. The package is schema-only and
     grants no settlement, payout, deployment, accepted-work, provider, wallet,
     or product-claim authority.
2. **Normalize launch evidence routes for agent dereference**
   - Body summary: Ensure public route consistency for settlements,
     verification challenges, receipts, run summary, proof replays, and product
     promises. Alias or remove drifted paths, add focused verification-challenge
     dereference, and keep simulation rows excluded from real-settled
     aggregates. Acceptance is a one-screen `curl` recipe that resolves every
     evidence ref without UI.
   - Implementation status (2026-06-18, issue #5414): normalized agent-facing
     evidence refs around the public route set. The settlements envelope now
     advertises the `/api/public/training/runs/{runRef}/settlements` alias in
     `sourceRefs`; the capability manifest lists the run summary, public
     settlements, public verification-challenge, and proof-replay endpoints;
     the product-promise registry is bumped to `2026-06-18.7` with public URL
     evidence; and the evidence pack now includes a one-screen curl recipe for
     run, settlement, verification, receipt, proof replay, promise, and install
     refs. This is dereference/discovery cleanup only and grants no settlement,
     payout, accepted-work, deployment, or claim-upgrade authority.
3. **Implement `GET /api/public/activity-timeline`**
   - Body summary: Union public-safe pylon, training, verification, settlement,
     Forum, Artanis, and capacity-funnel source events into the v1 timeline
     envelope with `?since=`, `?from=`, `?to=`, `?limit=`, and filter support.
     Emit `projection_gap` events instead of guessing when source coverage is
     incomplete.
   - Implementation status (2026-06-18, issue #5415): added
     `GET /api/public/activity-timeline` on the Worker API. The endpoint emits
     `openagents.public_activity_timeline.v1`, supports `since`, `from`, `to`,
     `limit`, `kind`, and `source` filters, reports source lag, orders events
     by stable `{ts}:{sourceKind}:{eventRef}` cursors, and returns
     `projection_gap` events with blocker refs when a source family is missing
     or unreadable. Initial live sources are Pylon registration/presence,
     training windows/leases/challenges, receipt-backed settlements, public
     Forum topic/post rows, Artanis tick decisions, and capacity-funnel
     snapshots. The capability manifest and OpenAPI catalog now advertise the
     route. Real Bitcoin movement is emitted only from settlement receipts with
     `realBitcoinMoved:true`; simulation receipts remain `settlement_recorded`
     only. This is read-only projection/retrieval and grants no settlement,
     payout, accepted-work, deployment, provider, wallet, or claim authority.
4. **Add timeline source-coverage and redaction tests**
   - Body summary: Add focused tests proving every event kind carries source
     refs or blocker refs, private material is omitted, stale source lag is
     visible, simulation settlement never emits `real_bitcoin_moved`, and cursor
     pagination is deterministic.
   - Implementation status (2026-06-18, issue #5416): hardened the live
     activity-timeline route tests so the Worker projection covers every
     non-gap event kind plus projection gaps, rejects any event without source
     refs or blocker refs, proves private source payload fields are not
     projected, marks stale source lag with explicit lag seconds/caveat refs,
     keeps simulation settlements from emitting `real_bitcoin_moved`, and
     paginates deterministically across same-timestamp cursor ties. The route
     now computes `sourceLag.lagSeconds` from source event time versus read time
     and marks stale source families instead of reporting every readable source
     as fresh. Validation passed with
     `bun run --cwd apps/openagents.com/workers/api test src/public-activity-timeline-routes.test.ts`,
     `bun run --cwd packages/public-activity-timeline test`, and
     `bun run --cwd apps/openagents.com/workers/api typecheck`, plus the
     public-projection ledger guard
     `bun run --cwd apps/openagents.com check:architecture`. This remains a
     read-only projection/test hardening change and grants no settlement,
     payout, accepted-work, deployment, provider, wallet, or claim authority.
5. **Publish agent-readable activity endpoint docs and manifest entries**
   - Body summary: Add docs and/or a public endpoint manifest entry so agents
     can discover the timeline, settlements, verification, receipt, proof-replay,
     and product-promise routes. Include example responses, event-kind meanings,
     staleness semantics, filters, and safe copy boundaries.
   - Implementation status (2026-06-18, issue #5417): added
     `docs/launch/2026-06-18-agent-activity-endpoint-guide.md` as the
     programmatic evidence-spine guide for agents. It documents
     `/api/public/activity-timeline` parameters, cursor ordering, envelope
     fields, event/source enums, source-lag stale/unavailable/projection-gap
     states, error responses, example response shape, curl recipes for tailing,
     bounded replay, gap detection, settlements, verification, receipts,
     proof-replay, and product promises, plus redaction and safe-copy
     boundaries. The capability manifest now exposes
     `docs.activityEvidence` and a `public_activity_evidence_spine` resource,
     OpenAPI timeline text calls out source lag and invalid-filter errors, and
     the shared timeline package README points implementers to the guide. This
     is discovery/documentation only and grants no settlement, payout,
     accepted-work, deployment, provider, wallet, or claim authority.
6. **Add Pylon CLI `activity`, `timeline`, `receipts`, and `evidence-pack`**
   - Body summary: Add JSON-first CLI commands over the public APIs:
     `pylon activity --watch`, `pylon timeline --from/--to`,
     `pylon receipts --run`, and `pylon evidence-pack --run`. Keep own-node
     status commands separate from public fleet commands. Acceptance: a fresh
     agent can answer the current-state questions from CLI JSON only.
   - Implementation status (2026-06-18, issue #5418): added the Pylon CLI as
     the first programmatic consumer of the public activity spine. The new
     `activity`, `timeline`, `receipts`, and `evidence-pack` commands are
     cataloged as read-only network-backed verbs, default to
     `https://openagents.com`, honor `--base-url` /
     `PYLON_OPENAGENTS_BASE_URL`, and emit JSON envelopes with
     `authority:"observation_only"`, request URLs, caveat refs, blocker refs,
     and dereferenceable receipt / challenge / evidence URLs. `activity` and
     `timeline` require every event to carry source refs or blocker refs;
     `receipts` maps public receipt refs to public receipt URLs; and
     `evidence-pack` gathers run summary, settlements, product promises,
     proof-replay, and optional verification challenge documents for agents.
     Focused tests cover filter expansion, bounded watch cursor resume,
     required args, text formatting, public-projection safety, and command
     catalog authority flags. This remains retrieval/projection only and grants
     no settlement, payout, accepted-work, deployment, provider, wallet,
     own-node status, or public-claim authority.

#### EPIC 2 — General replay from the public timeline

Body summary: Turn the programmatic timeline into generated replay bundles and
CLI replay output. Curated stories remain fixtures, but arbitrary
run/window/pair/range replay must be generated from public event refs instead
of hand-authored builders.

1. **Add generated proof-replay builder from activity timeline events**
   - Body summary: Map timeline events into `proof_replay_bundle.v1` actors,
     stages, flows, captions, camera cues, source refs, and gaps. Run the
     existing proof-replay shipment gates and source-coverage assertions on the
     generated bundle.
   - Implementation status (2026-06-18, issue #5419): added
     `buildProofReplayBundleFromPublicActivityTimeline` to
     `@openagentsinc/proof-replay`. The builder accepts a public
     `openagents.public_activity_timeline.v1` envelope, validates it through the
     timeline package, maps public work/trace/verification/settlement/payment/
     forum/fleet/capacity rows into deterministic replay actors, stages,
     events, flows, captions, and camera cues, and turns `projection_gap` rows
     plus stale/unavailable source-lag entries into explicit replay gaps. It
     runs `assertProofReplayBundleShipmentGate` before returning, preserves
     source/blocker/caveat refs, and only emits `payment_zap_confirmed` from
     receipt-backed `real_bitcoin_moved` rows. Package tests now assert the
     generated bundle passes the shipment gate and render-plan source coverage
     and rejects real-bitcoin timeline rows without public receipt evidence.
     This remains observation/projection/retrieval only and grants no
     settlement, payout, accepted-work, deployment, provider, wallet, or
     public-claim authority.
2. **Expose range-filtered generated replay API**
   - Body summary: Add a public route or route mode that accepts
     `from/to/runRef/windowRef/actorRef/kind` filters and returns generated
     replay bundles with the input cursor/range/filter recorded in the manifest.
   - Implementation status (2026-06-18, issue #5420): extended
     `GET /api/public/proof-replays` with `mode=activity-timeline`. Generated
     replay requests require bounded `from` and `to` ISO timestamps and accept
     optional `runRef`, `windowRef`, `actorRef`, repeated/comma-separated
     `kind`, repeated/comma-separated `source`, `since`, and `limit` filters.
     The route builds the same public activity timeline projection used by
     `/api/public/activity-timeline`, applies run/window/actor filtering,
     generates a `proof_replay_bundle.v1` through
     `@openagentsinc/proof-replay`, and returns a `generatedFrom` manifest with
     the exact input range/filter, source-lag state, staleness contract, source
     activity URL, and observation-only caveat refs. OpenAPI and the capability
     manifest now advertise the generated mode, and the agent endpoint guide
     includes a curl recipe. Worker tests cover bounded generation, filter
     propagation, manifest fields, shipment-gated output, source refs, and the
     400 response for unbounded generated requests. This remains
     observation/projection/retrieval only and grants no settlement, payout,
     accepted-work, deployment, provider, wallet, or public-claim authority.
3. **Add fleet boot and Forum replay tracks**
   - Body summary: Convert pylon boot/readiness/capacity events and Forum
     topic/post events into replay tracks so replay is not limited to
     proof->verify->settle. Preserve caveats for aggregate snapshots and stale
     sources.
   - Implementation status (2026-06-18, issue #5421): expanded the generated
     proof-replay builder so public activity timeline fleet boot,
     heartbeat/readiness, Forum discussion, and capacity snapshot rows become
     explicit replay tracks. Fleet boot/readiness emits
     `fleet_readiness_track` flows, Forum topic/post rows emit
     `discussion_track` flows, capacity snapshots emit
     `capacity_snapshot_track` flows and preserve aggregate snapshot caveats
     and state labels, and all remain sequenced alongside work, verification,
     settlement, and payment events. Tests now cover boot/readiness + Forum +
     capacity mixed with payment events, shipment gate/render-plan source
     coverage, and rejection of private Forum text/author material before any
     replay bundle is emitted. This remains observation/projection/retrieval
     only and grants no settlement, payout, accepted-work, deployment,
     provider, wallet, or public-claim authority.
4. **Add `pylon replay` over generated bundles**
   - Body summary: Extend CLI replay to fetch generated bundles and render a
     text/JSON event track for agents. The command should be useful without
     video or web UI and should expose source refs and caveats for every event.
   - Implementation status (2026-06-18, issue #5422): added `pylon replay`
     as the CLI-first replay consumer for generated public activity bundles.
     The command calls `GET /api/public/proof-replays?mode=activity-timeline`
     with required bounded `--from`/`--to` filters and optional `--run`,
     `--window`, `--actor`, `--pair`, `--kind`/`--filter`, `--source`,
     `--since`, and `--limit` filters. It supports text output by default plus
     `--format json` / `--json`; both outputs include an
     `openagents.pylon.public_replay_event_track.v1` projection with bundle
     refs, source authority, staleness/generatedFrom metadata, event sequence,
     timestamps, source refs, caveat refs, captions, and gaps. Video/clip
     generation remains separate. Tests cover generated replay URL construction,
     JSON event-track projection, text rendering of refs/caveats/captions/gaps,
     public-projection safety, and command catalog discovery. This remains
     observation/projection/retrieval only and grants no settlement, payout,
     accepted-work, deployment, provider, wallet, or public-claim authority.

#### EPIC 3 — Live stream and SpacetimeDB projection

Body summary: Add live delivery for the same timeline contract after the
programmatic API is stable. The Worker/D1 timeline remains source projection;
SSE and SpacetimeDB are delivery/projection layers only.

1. **Add SSE tail for the public activity timeline**
   - Body summary: Implement
     `GET /api/public/activity-timeline/stream?since=...` over the same event
     shape, with reconnect, cursor resume, stale-source reporting, and polling
     fallback guidance. Add tests for reconnect and no private material.
   - Implementation status (2026-06-18, issue #5423): added the public SSE
     stream route at `GET /api/public/activity-timeline/stream`. The stream
     reuses the exact public activity timeline builder, accepts the same
     `since`, `from`, `to`, `limit`, `kind`, and `source` filters, and also
     resumes from `Last-Event-ID` when `since` is absent. It emits a metadata
     frame containing the timeline envelope's schema/version, generated time,
     range, source-lag, staleness, and next cursor, then emits one SSE frame per
     public timeline event with `id` set to the event cursor, `event` set to the
     event kind, and `data.event` set to the same event shape returned by the
     JSON endpoint. The response includes `retry: 15000`,
     `x-openagents-polling-fallback`, no-store buffering headers, manifest and
     OpenAPI discovery, and guide docs for reconnect/poll fallback. Worker tests
     cover stream framing, source-lag metadata, `Last-Event-ID` reconnect, and
     private-material redaction. This remains observation/projection/retrieval
     only and grants no settlement, payout, accepted-work, deployment, provider,
     wallet, or public-claim authority.
2. **Bridge activity events into `openagents-world` public rows**
   - Body summary: Project public activity events into SpacetimeDB `world_event`
     rows through the existing service-identity bridge. Keep D1/Worker as the
     authority and enforce public-safe refs, generated-at, and replay-safe
     deterministic projection.
   - Implementation status (2026-06-18, issue #5424): added
     `project-activity-timeline.mjs` and its shared transform for projecting
     `openagents.public_activity_timeline.v1` envelopes into service-only
     `append_world_event` reducer calls plus a `record_projection_cursor` call.
     The bridge preserves each event cursor, event ref, run ref when present,
     public source refs, blocker refs, caveat refs, source-lag status,
     generated time, and expiry in a stable JSON `world_event.summary`, and it
     derives deterministic `world_event.public_activity.*` refs so retries and
     replays are idempotent. The transform rejects private material before an
     apply plan can be emitted, validates that every world event is sourced,
     and keeps the Worker/D1 timeline as the source projection while
     SpacetimeDB remains observation/projection/retrieval only. Validation:
     `bun test deleted legacy bridge transform tests`
     and `cargo test --manifest-path deleted legacy Cargo manifest`.
3. **Render evidence-bound live activity motion on `/tassadar`**
   - Body summary: Subscribe to timeline-backed world events and animate only
     events with source refs, generated-at, and stale/expiry metadata. Preserve
     the June 17 ban on anonymous motion and add canvas/browser smoke coverage.
   - Implementation status (2026-06-18, issue #5425): extended the `/tassadar`
     SpacetimeDB subscription to include `run.public_activity_timeline`
     `world_event` rows, and extended the summary adapter to parse
     `world_event.public_activity.*` summaries that use
     `openagents.world.public_activity_event_summary.v1`, then expose only
     source-backed rows as `world.activityMotions`. The visualization adapter
     turns those rows into one-shot bursts only when the target entity already
     exists and the motion carries public source refs, generatedAt, and either
     expiry or source-lag metadata; structural/ambient motion stays static and
     beams remain empty. Tests cover positive activity bursts plus rejection of
     missing refs, missing liveness metadata, and unsafe summary text. This
     remains observation/projection/retrieval only and grants no settlement,
     payout, accepted-work, deployment, provider, wallet, or public-claim
     authority.

#### EPIC 4 — Web and desktop visibility surfaces

Body summary: Build human/operator surfaces as consumers of the programmatic
contract. Web and desktop must not invent fields absent from the public API/CLI
contract; every visible event links back to timeline refs and proof URLs.

1. **Add web live-activity page over the public timeline**
   - Body summary: Build `/activity` or `/tassadar/activity` with Fleet, Money
     Loop, Forum, Timeline, and Proof Drawer panes. Start with polling if needed,
     but consume only documented public shapes and show stale-source state.
   - Implementation status (2026-06-18, issue #5426): added `/activity` as a
     public logged-out route backed by the shared
     `@openagentsinc/public-activity-timeline` decoder. The page polls
     `/api/public/activity-timeline?limit=100`, rejects unsafe raw payloads
     before rendering, and exposes Fleet, Money Loop, Forum, Timeline, Source
     lag, and Proof Drawer sections from documented event/source-lag fields
     only. The proof drawer renders event cursor, source API, public refs,
     source refs, blocker refs, caveats, source-lag status, and the decoded
     public event JSON so an agent can reproduce the answer from API/CLI
     output. Validation: `bun run --cwd apps/openagents.com/apps/web test
     src/scene/publicActivityTimelineElement.test.ts src/main.test.ts`,
     `bun run --cwd apps/openagents.com/apps/web typecheck`, and
     `git diff --check`.
2. **Add proof drawer and filters for activity events**
   - Body summary: Add event filters (`boot`, `work`, `verify`, `settle`,
     `forum`, `operator`) and a proof drawer that shows source refs, public URLs,
     caveats, staleness, and product-promise blockers for the selected event.
   - Implementation status (2026-06-18, issue #5427): extended the `/activity`
     page timeline with `all`, `boot`, `work`, `verify`, `settle`, `forum`, and
     `operator` filter buttons while leaving source-lag warnings outside the
     filter path. Fleet, Money Loop, Forum, and Timeline rows are now selectable
     proof-opening controls; the proof drawer includes event category, kind,
     source, state, staleness, source refs, blocker refs, caveats, derived public
     URLs, product-promise blocker refs, and decoded public event JSON. Public
     URL derivation stays bounded to public route/ref patterns and private raw
     payload material is still rejected before render. Validation:
     `bun run --cwd apps/openagents.com/apps/web test
     src/scene/publicActivityTimelineElement.test.ts src/main.test.ts`,
     `bun run --cwd apps/openagents.com/apps/web typecheck`, and
     `git diff --check`.
   - Follow-up validation (2026-06-18): added the
     `smoke:activity:proof-links` script for live same-origin 2xx checks over
     concrete proof URLs from `/api/public/activity-timeline`, documented proof
     URL derivation for agents, and filtered route-template refs such as
     `{receiptRef}` plus non-public historical `receipt.*` refs out of proof
     drawers/smokes because they are route-shape docs or refs, not
     dereferenceable evidence.
3. **Add Autopilot Desktop activity pane/strip**
   - Body summary: Add a live activity pane or strip to Network/Training so
     downloaded users can see fleet/work/money/forum activity even when the
     local node is unavailable. Reuse the public timeline endpoint and keep the
     coding-surface promise separate.
   - Implementation status (2026-06-18, issue #5428): added a shared
     `PublicActivityStrip` in `@openagentsinc/autopilot-ui` and wired Autopilot
     Desktop to fetch `GET /api/public/activity-timeline?limit=20` through the
     Bun RPC host. The Bun side validates the envelope with
     `@openagentsinc/public-activity-timeline` before the webview sees it, and
     returns a typed unavailable projection on HTTP, schema, cursor, or
     private-material failures. Network now shows the strip over the Tassadar
     replay without local-node dependence, and Training shows the same public
     strip beside the existing run/replay/operator panels. The strip exposes
     event category/kind/source/state/time, source-lag warnings, source refs,
     blocker/caveat refs, and bounded public hrefs for receipt, forum, pylon,
     training, Artanis, and capacity refs while keeping coding-session state
     separate. Validation: `bun run --cwd packages/autopilot-ui test`,
     `bun test tests/public-activity-timeline.test.ts tests/cl-53-foldkit.test.ts
     tests/cl-53-sanitize.test.ts` from `apps/autopilot-desktop`, and
     `bun run --cwd apps/autopilot-desktop verify:training`.
4. **Teach desktop replay pane generated bundle filters**
   - Body summary: Let the desktop replay pane request generated replay bundles
     by run/window/pair/range, not only curated slugs. Render source refs and
     caveats consistently with web and CLI.
   - Implementation status (2026-06-18, issue #5429): Autopilot Desktop replay
     loading now accepts either a curated catalog preset or a bounded generated
     public-activity request. The Training replay pane exposes range, run,
     window, actor, pair, event-kind, source, cursor, and limit filters, then calls
     `GET /api/public/proof-replays?mode=activity-timeline&from=...&to=...`
     through the same desktop loader and shipment gate used by curated
     bundles. Generated projections fail closed without valid ISO bounds,
     preserve `generatedFrom`, source-lag, source refs, caveat refs, and bundle
     API/activity API links, and remain `public_safe` /
     `evidence_presentation_only`. Validation: `bun test
     tests/proof-replays.test.ts tests/cl-53-foldkit.test.ts
     tests/cl-53-sanitize.test.ts` from `apps/autopilot-desktop`, plus
     `bun run --cwd apps/autopilot-desktop verify:training`.

#### EPIC 5 — Replay clips production service

Body summary: Productize the existing render-box clip pipeline so an agent or
operator can choose a timeline/replay moment, supply camera direction, and get a
public-safe mp4 plus manifest. Rendering runs on owned local/CI/Container
infrastructure, never inside the Cloudflare Worker.

1. **Define replay clip job schema and manifest**
   - Body summary: Specify the render job input (`replayBundleRef` or timeline
     range, start/duration/fps/resolution, camera path, output kind) and output
     manifest (source refs, bundle ref, camera path, renderer version, sha256,
     storage URL, caveats).
2. **Promote render-box worker with R2 upload**
   - Body summary: Turn `render-clip.mjs` into a production-owned render worker
     or wrapper that writes mp4 + manifest and uploads to R2/object storage.
     Keep Chrome/ffmpeg out of the Worker and document required runtime.
   - Implementation status (2026-06-18, issue #5431): `render-job.mjs` is now
     the owned render-box wrapper around the existing headless Chrome + ffmpeg
     renderer. It validates typed clip jobs, compiles the camera-path DSL,
     writes an mp4 and `openagents.replay_clip_manifest.v1`, builds stable
     `replay-clips/<jobRef>/...` object keys, and uploads both mp4 and manifest
     to Cloudflare R2 through the S3-compatible API when the owner-provisioned
     `R2_REPLAY_CLIPS_*` environment is present. Missing bucket credentials fail
     closed with a typed owner blocker and no secret values are printed. Chrome
     and ffmpeg remain render-box-only; the Cloudflare Worker only hosts job/read
     APIs and finished refs. Validation: `bun install --frozen-lockfile` from
     repo root, `bun run --cwd apps/openagents.com/apps/web test --
     spike/replay-r1/render-job.test.mjs`, `bun run --cwd packages/replay-clips
     test`, 1-second local `render-job.mjs` smoke to `/tmp/openagents-5431-render/clip.mp4`,
     and `git diff --check`.
3. **Add Worker clip job/read API**
   - Body summary: Add routes to create/list/read clip jobs and serve finished
     manifests/clips. The Worker may trigger or serve jobs only; it must not
     render frames or run native binaries.
4. **Add camera-path DSL for agents**
   - Body summary: Add a JSON camera-path grammar with keyframes and simple
     verbs (`hold`, `orbit`, `follow`, `frame_actor`, `frame_settlement`) that
     compiles into existing camera-path input. Include examples usable by agents.
5. **Add replay clip regression renders**
   - Body summary: Add owned-infra/local render checks for one curated story and
     one generated timeline bundle. Verify nonblank WebGL frames, differing
     camera paths produce differing frames, and manifests carry source refs.
   - Implementation status (2026-06-19, issue #5434): Added
     `render-regression-smoke.mjs`, an owned-infra/local render-box smoke that
     renders the curated `first-real-settlement` replay, the same replay through
     an alternate camera path, and a generated public activity replay bundle from
     `GET /api/public/proof-replays?mode=activity-timeline`. The smoke inspects
     retained PNG frames at pixel level, requires available WebGL surface
     metadata from `render-clip.mjs`, verifies primary/alternate camera frame
     hashes differ, builds public-safe `openagents.replay_clip_manifest.v1`
     manifests, and fails closed when source refs, caveats, public HTTPS storage
     refs, sha256s, or positive frame counts are missing. Outputs stay
     inspectable under the chosen output directory and the command is explicitly
     a local/owned-infra render-box workload, not a GitHub-hosted CI or
     Cloudflare Worker workload. Validation: `bun run --cwd
     apps/openagents.com/apps/web test -- spike/replay-r1/render-regression-smoke.test.mjs`,
     1-second local smoke to `/tmp/openagents-5434-regression`, and
     `git diff --check`.

#### EPIC 6 — Operations, gates, and launch proof

Body summary: Make the visibility/replay stack operational and line it up with
product-promise gates. This epic closes the gap between "implemented" and
"fully live": freshness checks, runbooks, promise alignment, and the first
fully autonomous self-serve settlement captured by timeline/replay/clip.

1. **Add owned-infra visibility freshness checks**
   - Body summary: Add non-GitHub-hosted scheduled checks for timeline
     freshness, source lag, SSE health, SpacetimeDB bridge health, render queue
     health, R2 clip availability, and public route status. Alert on stale
     projections instead of discovering them by manual refresh.
   - Implementation status (2026-06-19, issue #5435): Added
     `apps/openagents.com/scripts/visibility-freshness-smoke.mjs`, an
     owned-infra/local Node smoke for public route status, activity timeline
     freshness, source-lag rows, SSE stream health, SpacetimeDB bridge-plan
     health, replay-clip render queue freshness, and R2 clip manifest/artifact
     availability. The smoke exits nonzero for scheduler alerting by default,
     supports `--warn-only` for manual evidence capture during known incidents,
     and reports stale source kinds or broken URLs directly in JSON. The
     deployment hub now documents the command and explicitly keeps it off
     GitHub Actions. Validation: `cd apps/openagents.com && bunx vitest run
     scripts/visibility-freshness-smoke.test.ts`, live `--warn-only` smoke
     against `https://openagents.com`, and `git diff --check`.
2. **Add browser/canvas smokes for activity and replay surfaces**
   - Body summary: Add browser-capable smokes for `/tassadar`, replay pages, and
     the live-activity page: nonblank WebGL, no anonymous motion, proof drawer
     public route returns 200, and text fits desktop/mobile viewports.
   - Implemented on 2026-06-18: `bun run --cwd apps/openagents.com
     smoke:visibility:browser` launches Chromium against `https://openagents.com`
     by default, checks `/tassadar`, `/tassadar/replay/first-real-settlement`,
     and `/activity` at desktop and mobile viewports, verifies nonblank canvases,
     source-bound replay bundle motion, proof drawer public links, and visible
     text overflow. The smoke is observation-only and can be pointed at local or
     owned infra with `--base-url`; local UI runs can proxy live public API data
     with `--api-base-url`.
3. **Document timeline/stream/world/render operations in deployment docs**
   - Body summary: Add runbook coverage for operating the timeline API, SSE
     stream, SpacetimeDB bridge, render worker, R2 outputs, failure modes, and
     rollback. Link it from `docs/DEPLOYMENT.md`.
   - Implemented on 2026-06-19:
     `docs/launch/2026-06-19-visibility-replay-operations-runbook.md` now
     documents source-of-truth boundaries, projection authority, timeline/SSE
     operations, SpacetimeDB bridge plans, render-box/R2 outputs, failure modes,
     rollback steps, and smoke commands. `docs/DEPLOYMENT.md` links to the
     runbook from the visibility operations section.
4. **Capture first fully autonomous self-serve settlement in timeline/replay**
   - Body summary: When a fresh independent contributor is auto-paid at verdict
     with no operator POST, record the evidence sequence in the timeline, produce
     a generated replay bundle, and render a clip. This is an evidence capture
     issue, not a new settlement authority issue.
   - Implementation status (2026-06-19, issue #5438): Recorded
     `docs/launch/2026-06-19-autostream-settlement-visibility-capture.md`.
     The capture documents the public activity sequence for
     `training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4`:
     `trace_submitted` at `2026-06-18T13:47:40.322Z`,
     `verification_verified` at `2026-06-18T13:47:40.412Z`, and receipt-backed
     `real_bitcoin_moved` + `settlement_recorded` at
     `2026-06-18T13:47:40.555Z`. The public receipt dereferences with
     `realBitcoinMoved: true`, `movementMode: real_bitcoin`, and
     `settlement.state: settled`; the generated replay bundle
     `proof_replay_bundle.public_activity.73e66071` includes the same sequence;
     and a local render-box clip/manifest was produced at
     `/tmp/openagents-5438-autostream.mp4` with sha256
     `aa2689c520bca782681b3cc4db9478aba5d06ac29fd7c03e29cb436b00ffda8b`;
     the manifest is committed at
     `docs/launch/2026-06-19-autostream-settlement-clip-manifest.json`.
     The capture preserves the caveat that replay source refs still include
     `operator_approval.tassadar.autostream.worker` and that R2 upload remains
     owner-gated (`needs_owner.replay_clip.r2_bucket_not_provisioned`).
5. **Align product promises and launch docs to visibility evidence**
   - Body summary: Once the autonomous sequence is captured, update the product
     promises, AGENTS/INSTALL/evidence-pack refs, and launch docs with the exact
     public refs and owner-signed upgrades where required. Keep world-firsts in
     qualified wording until their owner-signed receipt-first upgrades land.
   - Partial cleanup on 2026-06-19: `docs/promises/registry.md` no longer
     presents the scoped decentralized-training launch as a red/planned Monday
     launch. That partial cleanup pointed to live registry `2026-06-18.8`, kept
     `training.decentralized_training_launch.v1` green only for bounded
     receipt-backed launch evidence, and preserves the red/yellow gates for
     network-scale, largest-run, model-mutation, broad earning, and fully
     autonomous auto-stream settlement claims.
   - Implementation status (2026-06-19, issue #5439): Product promises now
     target registry `2026-06-19.1`, cite the #5438 timeline/replay/receipt/clip
     refs, remove the stale default-npm blocker after `@openagentsinc/pylon` was
     verified on the v1.0 line, and keep
     `pylon.consumer_compute_earns_bitcoin_self_serve.v1` RED on scale,
     Windows/WSL, and Spark-helper auto-start/readiness gates. AGENTS.md,
     INSTALL.md, the evidence pack, and this audit now use the same refs and
     states. No world-first, public-claim, settlement, payout, provider, wallet,
     deployment, or accepted-work authority was added.

---

## 4. The single most valuable next step

If only one thing ships: **Phase 1 — the unified public activity timeline
endpoint.** It is the keystone that unblocks live ticker, arbitrary replay,
desktop/CLI parity, SpacetimeDB event projection, and generated replay clips.

The first productization step after that is **Phase 2 — agent/CLI programmatic
access**. A browser UI should not be the first consumer. Agents need stable JSON,
cursoring, examples, fixtures, and CLI commands before the UI layer starts
polishing the same data. The web `/activity` page now follows that contract and
remains reproducible from API/CLI evidence; desktop parity is still pending.

---

## 5. Honest current state — what an operator can actually see RIGHT NOW

Grounded against live `https://openagents.com` on 2026-06-18:

- **Web, fleet boot-ups:** `GET /api/public/pylon-stats` → **HTTP 200**, live:
  `pylonsOnlineNow=7`, `pylonsSeen24h=11`, `pylonsRegisteredTotal=73`,
  `pylonsWalletReadyNow=1`, `pylonsAssignmentReadyNow=1`, `recentPylons` length
  11. Capacity funnel + history both **HTTP 200**.
- **Web, money loop:** `GET /api/public/tassadar-run-summary` → **200**,
  `runState=active`, 3 settlement rows. `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements`
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
