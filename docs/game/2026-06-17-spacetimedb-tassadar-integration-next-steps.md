# SpacetimeDB To Tassadar Integration Next Steps

Date: 2026-06-17
Status: implementation plan

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

Browser/user reducers should be limited to explicit interaction state such as
selection or presence after those rows are modeled. They must not create proof,
settlement, receipt, pylon, or training truth.

## Phase 1: Bridge From The Worker Projection

Create an operator/server-side bridge. It should:

1. Read `https://openagents.com/api/public/tassadar-run-summary`.
2. Validate the payload with the same structural assumptions used by
   `tassadarRunSnapshot.ts`.
3. Transform the summary into the minimal SpacetimeDB tables above.
4. Call service-only reducers on `openagents-world`.
5. Store the source URL, `generatedAt`, projection staleness contract, and
   public proof refs on the rows it writes.

The bridge should be idempotent. Replaying the same public summary should
update existing rows without inventing new events. A new `world_event` row
requires a real source ref or a timestamped projection transition.

## Phase 2: Browser Subscription Adapter

Add the browser client behind an explicit feature flag or page attribute, for
example:

```text
TASSADAR_SPACETIME_WORLD_URL=https://spacetime.openagents.com
TASSADAR_SPACETIME_DATABASE=openagents-world
```

Do not put private tokens in the browser. Browser access should be anonymous or
public-row scoped until a modeled OpenAgents identity mapping exists.

The first web adapter should not rewrite the scene. It should:

- keep `tassadarRunElement.ts` fetching `/api/public/tassadar-run-summary` for
  the base snapshot;
- subscribe to `training_run`, `run_entity`, `world_edge`, `proof_ref`,
  `settlement_ref`, and `world_event` rows only when the flag is enabled;
- convert row callbacks into the same `TrainingRunVisualizationSnapshot` shape
  that `tassadarRunSnapshot.ts` already builds;
- fall back to the Worker summary without blocking page startup if the
  SpacetimeDB connection fails;
- apply live updates only when they carry public refs or timestamped projection
  transitions.

The visual contract from the live-page audit still applies: every moving dot,
pulse, flow, beam, burst, or counter roll must be backed by a public ref or a
timestamped live state transition. Counts can color labels or status, but counts
alone cannot create fake traffic or spatial nodes.

## Phase 3: Inspector And Presence

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
  unavailable.
- Run the existing `/tassadar` smoke.
- Probe `https://spacetime.openagents.com/v1/database/openagents-world/subscribe`
  and expect `426` from a non-WebSocket curl.
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
