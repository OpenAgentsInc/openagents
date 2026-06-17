# SpacetimeDB To Tassadar Integration Next Steps

Date: 2026-06-17
Status: Phase 0, Phase 1, Phase 2, base ops hardening, and MVP interaction schema implemented; station/avatar projection next

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
`upsert_pylon_station_from_projection`, `ensure_pylon_agent_avatar`,
`record_system_world_message`, and `expire_interaction_rows`.

These rows do not create proof, settlement, receipt, pylon, or training truth.
The next step is issue #5262: derive pylon stations and one pylon-agent avatar
per public pylon ref from the existing Tassadar summary bridge.

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
