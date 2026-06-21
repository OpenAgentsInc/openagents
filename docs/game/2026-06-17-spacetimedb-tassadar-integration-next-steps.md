# SpacetimeDB To Tassadar Integration Next Steps

Date: 2026-06-17
Status: Phase 0, Phase 1, Phase 2, base ops hardening, MVP interaction schema, station/avatar projection, station/avatar rendering, browser movement/attention, and local chatter implemented

## Current Boundary

The current `/tassadar` page is already live-data backed. Keep this path as the
source of truth:

- `GET /api/public/tassadar-run-summary`
- `apps/openagents.com/apps/web/src/scene/tassadarRunSnapshot.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts`
- `@openagentsinc/three-effect` `oa-training-run`

SpacetimeDB should not replace that authority. The first connection should be a
world-state projection and subscription layer fed from the existing public
Worker/D1 authority. If SpacetimeDB is down, `/tassadar` must still render from
`/api/public/tassadar-run-summary`.

## Phase 0: Minimal Module

Issue #5236 published a minimal `openagents-world` module from
`apps/openagents-world-spacetimedb` on 2026-06-17. It contains only the rows
needed to mirror the canonical Tassadar run:

| Table | Purpose |
| --- | --- |
| `training_run` | Canonical run id, state, generated timestamp, staleness contract, source URL. |
| `run_entity` | Public-ref entities already visible in the scene: pylons, replay refs, trace refs, receipt refs, and the run node. |
| `world_edge` | Source-backed relationships between real entities. |
| `proof_ref` | Dereferenceable proof, challenge, receipt, or trace links. |
| `settlement_ref` | Public settlement refs only, including simulation or real movement metadata. |
| `world_event` | Append-only timestamped events derived from public refs or projection transitions. |
| `projection_cursor` | Bridge cursor for the last public summary payload projected into SpacetimeDB. |
| `bridge_health` | Bridge heartbeat, source URL, last success, and last failure summary. |

Use service-only reducers for authority projection rows:

- `upsert_training_run`
- `upsert_run_entity`
- `upsert_world_edge`
- `upsert_proof_ref`
- `upsert_settlement_ref`
- `append_world_event`
- `record_projection_cursor`
- `record_bridge_health`
- `record_bridge_success`
- `record_bridge_failure`

Browser/user reducers should be limited to explicit interaction state such as
selection or presence after those rows are modeled. They must not create proof,
settlement, receipt, pylon, or training truth.

## Phase 1: Bridge From The Worker Projection

Issue #5237 implemented the operator bridge on 2026-06-17. It:

1. Reads `https://openagents.com/api/public/tassadar-run-summary`.
2. Validates the payload with the same structural assumptions used by
   `tassadarRunSnapshot.ts`.
3. Transforms the summary into the minimal SpacetimeDB tables above.
4. Calls service-only reducers on `openagents-world`.
5. Stores the source URL, `generatedAt`, projection staleness contract, and
   public proof refs on the rows it writes.

The bridge should be idempotent. Replaying the same public summary should
update existing rows without inventing new events. A new `world_event` row
requires a real source ref or a timestamped projection transition.

The live bridge planned 182 reducer calls for canonical run
`run.tassadar.executor.20260615` and projected these live row counts after an
apply and replay:

| Table | Count |
| --- | ---: |
| `training_run` | 1 |
| `run_entity` | 16 |
| `world_edge` | 16 |
| `proof_ref` | 58 |
| `settlement_ref` | 1 |
| `world_event` | 17 |
| `projection_cursor` | 1 |
| `bridge_health` | 1 |

`run_entity` and `proof_ref` counts are intentionally de-duplicated by primary
key. The replay check left `world_event` at 17 rows.

## Phase 2: Browser Subscription Adapter

Issue #5238 added the browser client behind explicit page attributes:

```text
data-spacetime-world-url="https://spacetime.openagents.com"
data-spacetime-database="openagents-world"
```

Do not put private tokens in the browser. Browser access should be anonymous or
public-row scoped until a modeled OpenAgents identity mapping exists.

The first web adapter does not rewrite the scene. It:

- keeps `tassadarRunElement.ts` fetching `/api/public/tassadar-run-summary` for
  the base snapshot;
- subscribes to `training_run`, `run_entity`, `world_edge`, `proof_ref`,
  `settlement_ref`, and `world_event` rows only when the flag is enabled;
- converts row callbacks into the same public-summary shape that
  `tassadarRunSnapshot.ts` already builds;
- falls back to the Worker summary without blocking page startup if the
  SpacetimeDB connection fails;
- applies live updates only when they carry public refs or timestamped projection
  transitions.

Implementation paths:

- `apps/openagents.com/apps/web/src/scene/tassadarSpacetimeWorld.ts`
- `apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings/`
- `apps/openagents.com/apps/web/src/scene/tassadarRunSnapshot.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts`

The visual contract from the live-page audit still applies: every moving dot,
pulse, flow, beam, burst, or counter roll must be backed by a public ref or a
timestamped live state transition. Counts can color labels or status, but counts
alone cannot create fake traffic or spatial nodes.

## Phase 3: Inspector And Presence

Issue #5239 completed the base GCP operations hardening before this phase:

- uptime check for `https://spacetime.openagents.com/v1/identity` expecting
  `405`;
- enabled Cloud Monitoring policies for identity uptime failure, Nginx 5xx
  spikes, and `spacetimedb.service` restart loops;
- boot and data disk snapshots named in the admin runbook;
- `/stdb` moved to dedicated persistent disk `spacetimedb-world-data-1`.

The remaining ops follow-up is notification delivery, not signal definition:
the project had no Cloud Monitoring notification channels when #5239 ran, so
the policies currently create Monitoring incidents without external paging.

Issue #5261 added the first presence/gameplay schema without changing the
authority boundary. The new public interaction tables are:

| Table | Purpose |
| --- | --- |
| `world_region` | Public run-space envelope: bounds, Street metadata, proximity radius, avatar update cadence, stale-position TTL, and adjacent chunk refs. |
| `pylon_station` | In-world station for a public pylon ref during a run. |
| `agent_avatar` | Public avatar identity row for a guest, human, pylon agent, or service agent. |
| `avatar_position` | Latest bounded position, yaw/pitch, movement mode, and freshness for one avatar. |
| `pylon_attention` | Short-lived signal that an avatar is approaching, nearby, looking, inspecting, or talking to a pylon. |
| `local_chat_message` | Public-safe plain-text spatial chat row with radius, channel, TTL, and moderation state. |
| `chat_bubble` | Short-lived display row for a message bubble anchored to an avatar or target entity. |
| `local_emote` | Short-lived non-verbal world signal such as wave, ping, point, confused, or working. |
| `agent_intent` | Ephemeral public activity hint for an avatar. |

The new browser reducers are limited to interaction state:
`join_region`, `leave_region`, `set_avatar_position`, `focus_pylon`,
`clear_pylon_focus`, `send_local_message`, `send_pylon_message`, `send_emote`,
and `set_agent_intent`. The new service-only reducers are
`upsert_world_region`, `upsert_pylon_station_from_projection`,
`ensure_pylon_agent_avatar`, `record_system_world_message`, and
`expire_interaction_rows`.

These rows do not create proof, settlement, receipt, pylon, or training truth.

Issue #5272 added the first explicit region/proximity contract. The Tassadar
bridge now writes `world_region` before projecting pylon stations. Browser
movement reducers validate against that region row, reject impossible jumps,
and use the region's update interval. Stale avatar expiry uses the
`world_region.stale_avatar_position_ms` value, with the previous 20 second
constant only as a compatibility fallback for older rows. Local chat and emotes
also require an existing region row, so a browser identity cannot mint
arbitrary spatial namespaces.

Issue #5890 expands that first region into the actual Verse/Street starter
chunk instead of the early tiny diagram box. The default Tassadar row now uses
`x=-160..160`, `y=0..40`, and `z=-160..160`, road direction `(0, 0, 1)`, local
origin `(0, 0, 0)`, starter pylon site offset `(24, 0, 0)`, and previous/next
Street chunk refs. The road can stay visually continuous in Three.js, but
server reducers and browser/desktop movement validation use these bounded
chunk coordinates until cross-region traversal is implemented.

The intended subscription shape is:

- `world_region` for the selected run region envelope;
- `pylon_station` filtered by `region_ref`;
- `avatar_position` filtered by `region_ref`, joined to `agent_avatar`;
- `pylon_attention` joined through active-region `pylon_station` rows;
- `chat_bubble` joined through active-region `local_chat_message` rows;
- `agent_intent` joined through active-region `avatar_position` rows;
- `local_chat_message` and `local_emote` filtered by `region_ref`;
- proof/run/settlement details still read by selected public refs, not by
  proximity or animation.

Issue #5891 implements that desktop query contract and adds module indexes for
the active run/region filters and join columns. It also introduces a bounded
visible/nearby pylon/avatar target candidate mapper for the Verse tab-cycling
path, so normal targeting does not enumerate off-screen or off-region users.

Issue #5262 now derives pylon stations and one pylon-agent avatar per visible
public leaderboard pylon ref from the existing Tassadar summary bridge. The
bridge dry-run against `https://openagents.com/api/public/tassadar-run-summary`
plans 194 reducer calls, including 6 `upsert_pylon_station_from_projection`
calls and 6 `ensure_pylon_agent_avatar` calls. After publishing the schema and
applying/replaying the bridge, live counts stayed stable at 17 `world_event`
rows and added 6 `pylon_station`, 6 `agent_avatar`, and 6 `avatar_position`
rows.

Issue #5263 subscribes to those public interaction rows in the browser adapter
and renders stations plus pylon-agent avatars on `/tassadar`. The scene still
fetches `/api/public/tassadar-run-summary` first and continues rendering from
that Worker/D1 summary when SpacetimeDB is disabled, unreachable, or returns no
usable world rows. When SpacetimeDB is connected, `pylon_station` rows become
compact station entities next to the public pylon lane, and `agent_avatar` rows
with matching `avatar_position` rows become pylon-agent entities at their
bounded world coordinates. Selection remains proof-safe: station and avatar
entity IDs are resolved back to their public `home_pylon_ref`, then the existing
public proof/receipt inspector decides whether a dereferenceable settlement
receipt or pylon evidence link exists.

Issue #5264 makes the browser an interaction writer without changing proof
authority. When SpacetimeDB connects, `/tassadar` calls `join_region` with a
session-local public display name, tracks the existing WASD/mouselook controls
with a small bounded local avatar state, and calls `set_avatar_position` at a
250 ms client throttle or slower. Idle connected viewers send a 5 second
keepalive so they remain visible; disconnected or stale non-service avatars are
still removed by the module's 20 second stale-position TTL when
`expire_interaction_rows` runs. The browser also emits `focus_pylon` at a
1 second throttle when the local avatar is near, looking at, or inspecting a
pylon station, and clears focus when it leaves the station.

The scene now subscribes to `pylon_attention` and maps guest/human/service
avatars with current `avatar_position` rows in the run region, so a second
browser session appears as a row-backed avatar. Pylon stations show compact
`+N` visitor labels when attention rows exist. If SpacetimeDB is disabled or
unreachable, the WASD/mouselook scene still works locally from the Worker/D1
summary and no multiplayer reducers are called.

Issue #5265 adds public-safe local and pylon-targeted chatter over the same
interaction boundary. The module now rejects chat writes from the same avatar
inside a 1 second window, keeps body text plain, caps bodies at 280 characters,
stores `moderation_state="visible"`, expires message rows after 90 seconds, and
expires bubble rows after 8 seconds. The browser sanitizes form input before
calling `send_local_message` or `send_pylon_message`, subscribes to
`local_chat_message` and `chat_bubble`, renders row-backed bubble entities over
speakers and pylon stations, and shows a compact nearby transcript HUD. Pylon
messages render at both the speaker and station from the same public message and
bubble rows.

Issue #5273 tightens the `/tassadar` world binding around those rows. The
browser adapter now subscribes to `world_region` rows and includes the region
envelope in the public-summary shape. The first valid region row supplies the
first-person movement/controller bounds; the static fallback remains only for
Worker-summary or older projection cases where no region row is available.
That fallback now matches the starter Street contract from issue #5890, so the
client does not clamp locally to the obsolete `x=-8..8`, `z=-6..6` diagram
while the server accepts the larger chunk.
Station entities still require real `pylon_station` rows, and avatar entities
now require both `agent_avatar` and matching `avatar_position` rows. Crowded
station/avatar positions are separated with the shared `three-effect`
`SpatialHashGrid` and minimum-distance layout helper, then chat bubbles follow
the adjusted row-backed anchor positions. The `three-effect` renderer used by
the page also resolves node/entity hits through the shared hit-target registry
for both unlocked clicks and pointer-locked center-reticle clicks.

Issue #5274 moves lifecycle/status categories fully out of world geography. The
primary canvas keeps only the real run node and row/ref-backed world entities;
registered/qualified/state-synced/active/sync-reentry counters render as a
compact text HUD legend. Loss/chart chrome stays hidden until real public loss
data is intentionally wired and product-ready, and the page still emits no
transfer dots, beams, or bursts without evidence-backed source refs.

This phase still does not render payout motion before row-backed settlement
evidence exists, and it does not store private prompts, private runtime logs,
wallet material, provider payloads, or fixture chatter.

After the base projection works, add subscriptions that are useful only when an
entity is selected:

- selected entity `proof_ref` rows;
- selected entity `settlement_ref` rows;
- recent `world_event` rows for the selected entity;
- optional public presence rows for viewers in the same run region.

Keep inspector links dereferenceable through existing public OpenAgents routes.
Do not move proof or receipt rendering to SpacetimeDB-only URLs.

## Tests And Smoke

Required checks before treating `/tassadar` as connected:

- Unit-test the bridge transform from public summary payload to SpacetimeDB
  rows.
- Unit-test that replaying the same summary is idempotent and does not append
  duplicate `world_event` rows.
- Add web adapter coverage for Worker-summary fallback when SpacetimeDB is
  unavailable. Done in issue #5238.
- Run the existing `/tassadar` smoke. Done in issue #5238 against the current
  production route.
- Re-run the existing `/tassadar` smoke after station/avatar rendering. Done in
  issue #5263 against the deployed production route and hashed
  `/assets/index-D3gMYS5a.js` bundle.
- Add web coverage for bounded movement integration, SpacetimeDB-unavailable
  fallback, guest-avatar row mapping, and pylon-attention row mapping. Done in
  issue #5264.
- Add web coverage for chat sanitization, chat row mapping, pylon-targeted
  speaker/station bubble rendering, and fallback with no fixture chatter. Done
  in issue #5265.
- Add web coverage for `world_region` row mapping, row-backed avatar gating,
  shared minimum-distance station/avatar layout, region-bounded movement
  handoff, mouselook debug handoff, and proof-safe selection drawer behavior.
  Done in issue #5273 through focused web scene unit tests. The existing
  dependency-free live smoke remains HTTP/API/asset-level; pixel-level WebGL,
  WASD, mouselook, and canvas selection smoke still requires a browser runner.
- Add web coverage proving lifecycle/status categories remain HUD text and do
  not reappear as spatial node/entity IDs. Done in issue #5274.
- Build the SpacetimeDB WASM module after reducer-boundary changes. Done in
  issue #5265.
- Probe `https://spacetime.openagents.com/v1/database/openagents-world/subscribe`
  and expect `426` from a non-WebSocket curl. Done in issue #5238.
- Manually verify the browser scene still contains no unbacked motion.

## Acceptance Criteria

- `openagents-world` is published on the self-hosted VM.
- The bridge projects canonical run `run.tassadar.executor.20260615`.
- `/tassadar` renders from `/api/public/tassadar-run-summary` when
  SpacetimeDB is down or disabled.
- When enabled, SpacetimeDB subscriptions only add row-backed updates and do
  not introduce anonymous animation.
- Selected entity proof, trace, challenge, receipt, and settlement links still
  resolve through existing public OpenAgents routes.
- The authority boundary remains documented: Worker/D1 public projections own
  Tassadar truth; SpacetimeDB owns live world projection and interaction state.
