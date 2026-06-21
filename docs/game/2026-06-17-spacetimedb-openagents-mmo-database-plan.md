# SpacetimeDB Direction for the OpenAgents MMO Database

Date: 2026-06-17
Status: Initial architecture note after reading the `projects/spacetime` lane.
Sources:
- `/Users/christopherdavid/work/projects/spacetime/README.md`
- `/Users/christopherdavid/work/projects/spacetime/repos/SpacetimeDB`
- `/Users/christopherdavid/work/projects/spacetime/repos/BitCraftPublic`
- `/Users/christopherdavid/work/projects/spacetime/repos/spacetimedb-minecraft`
- `/Users/christopherdavid/work/projects/spacetime/repos/spacetimedb-typescript-sdk`

## Conclusion

SpacetimeDB is a strong fit as the reference model for an OpenAgents MMO
database because it combines:

- relational state;
- server-side mutation logic through reducers;
- scheduled server agents;
- generated typed clients;
- live row subscriptions with local client caches.

That is almost exactly the shape needed for an OpenAgents world: real pylons,
agents, contributors, runs, proof edges, settlements, regions, and teams rendered
as persistent, subscribable world entities.

The important product boundary is that the MMO database should not become the
authority for OpenAgents money, training, payout, proof, or public claims. It
should start as a live world-state projection and interaction layer. Existing
OpenAgents authority surfaces continue to decide whether a run, receipt,
settlement, task, or claim is real. The MMO database makes those facts visible,
inspectable, and spatial.

## What SpacetimeDB Teaches

SpacetimeDB modules define tables and reducers inside the database. Clients
connect over WebSocket, subscribe to table/query results, receive initial rows,
and then receive inserts, updates, and deletes in real time. Client SDKs keep a
local cache, which maps directly to Three.js/Foldkit entity pools.

BitCraft shows the high-end version of this pattern:

- gameplay state is mostly tables;
- mutations are reducers;
- scheduled tables drive periodic agents;
- region modules and a global module separate local world state from global
  coordination;
- player/entity/location/component state is split by access pattern rather than
  packed into one large record.

The Minecraft example shows a useful bridge pattern: a legacy or external
runtime can proxy protocol events into SpacetimeDB reducers and subscribe to
database updates, while SpacetimeDB stays the authoritative state-sync layer for
that world.

For OpenAgents, the analog is not a Minecraft proxy. It is an OpenAgents
authority bridge that projects verified product facts into the world database.

## Proposed OpenAgents Role

Publish an `openagents-world` SpacetimeDB database/module as the MMO state
layer.

It should own:

- spatial placement of world entities;
- live presence and region occupancy;
- display state for pylons, agents, contributors, teams, and runs;
- recent world events derived from verified OpenAgents events;
- user interaction state such as selected entities, chat, region entry, and
  viewport-local subscriptions.

It should not own:

- settlement or payout authority;
- training-run truth;
- product-promise status;
- receipt validation;
- Pylon assignment authority;
- wallet secrets;
- provider credentials;
- private prompts, private repos, or raw customer data.

## First Schema Shape

The first module should be intentionally small and additive. Suggested tables:

| Table | Purpose | Authority |
| --- | --- | --- |
| `world_region` | Bounded MMO regions, shards, or rooms, including local coordinate bounds and region adjacency metadata. | OpenAgents world module |
| `actor_identity` | Public world identity for a human, agent, pylon, or service. | OpenAgents auth/identity bridge |
| `presence_session` | Connected viewer/operator sessions. | SpacetimeDB lifecycle reducers |
| `pylon_node` | Public pylon projection: id, status, capability summary, freshness. | Pylon/public projection bridge |
| `agent_actor` | Agent display identity and current activity summary. | OpenAgents/agent authority bridge |
| `training_run` | Public run projection with canonical run id and freshness. | OpenAgents training projection bridge |
| `run_entity` | Entity rows inside a run world: worker, verifier, executor, artifact, proof. | Training projection bridge |
| `world_edge` | Spatial relationship between two real entities. | Derived from authority bridge rows |
| `proof_ref` | Dereferenceable proof or registry link for an entity or edge. | Existing proof/registry authority |
| `settlement_ref` | Dereferenceable settlement or payout reference, never raw wallet data. | Existing settlement authority |
| `world_event` | Append-only visible events with timestamps and source refs. | Existing event/projection bridge |
| `team_state` | Guild/team/coalition projection for humans and agents. | OpenAgents world module plus authority refs |
| `reputation_snapshot` | Derived visible reputation, e.g. verified-work counts. | Derived from receipts/proof refs |

This mirrors BitCraft's split between entity state, location state, global state,
and scheduled agents, but with OpenAgents domain objects.

For the first Verse/Street deployment, `world_region` is not a decorative
viewport hint. It is the server contract for reducer validation and client
movement clamps. The Tassadar starter chunk uses local Cartesian bounds
`x=-160..160`, `y=0..40`, and `z=-160..160`; road direction `(0, 0, 1)`;
local origin `(0, 0, 0)`; starter pylon site offset `(24, 0, 0)`; and typed
previous/next Street region refs reserved for future chunk traversal. The road
may render as visually continuous or repeated beyond the chunk, but avatar,
station, chat, and emote writes stay inside registered region rows.

## Reducer Boundary

Reducers are the only mutation path in SpacetimeDB, so the OpenAgents module
should use reducers as a hard claim boundary.

Service-only reducers:

- `upsert_pylon_projection`
- `upsert_training_run_projection`
- `upsert_run_entity`
- `upsert_world_edge`
- `append_authority_event`
- `upsert_proof_ref`
- `upsert_settlement_ref`
- `expire_stale_projection`

User/client reducers:

- `join_region`
- `leave_region`
- `set_display_position`
- `select_entity`
- `send_region_message`
- `follow_entity`

The rule: user reducers may update game/user interaction state, but they may not
create proof, payout, run, training, or pylon truth. Those rows come only from
service identities that bridge from existing OpenAgents authority.

## Subscription Model

The client should never subscribe to everything. SpacetimeDB's docs call out
subscription lifetime grouping; OpenAgents should use that directly:

- **Global lifetime:** regions, legends, public registry metadata, current user
  identity, feature flags.
- **Region lifetime:** nearby entities, region messages, region events, world
  edges within the viewport.
- **Selected entity lifetime:** proof refs, settlement refs, full event history,
  challenge/receipt links.
- **Run lifetime:** `training_run`, `run_entity`, `world_edge`, and recent
  `world_event` rows for one canonical run id.

For `/tassadar`, the page should subscribe only to the Tassadar run and the
world entities needed to render that run. The Three.js scene should update from
row insert/update/delete callbacks, not from scripted fake activity.

The first Verse/Street desktop subscription scope now follows this model:
station, position, chat, and emote rows are filtered by active `region_ref`;
avatar profiles, pylon attention, chat bubbles, and agent intents are joined
through active-region position/station/message rows instead of subscribed as
global tables. Tab targeting should consume bounded visible/nearby pylon and
avatar candidates, leaving proof, settlement, and training detail rows to the
selected-entity lifetime.

## Scheduled Agents

BitCraft uses schedule tables for periodic agents. OpenAgents should use the
same pattern, but only for non-authority world maintenance:

- expire stale presence sessions;
- mark stale projections as stale rather than deleting them;
- compact old non-critical world events;
- rebuild derived reputation snapshots from receipt/proof refs;
- update region occupancy counters;
- emit heartbeat rows for module health.

Scheduled reducers must not fabricate work, payouts, training progress, or
network traffic. If there is no source event, there is no animated work event.

## Identity and Authorization

SpacetimeDB supports OIDC authentication and server-issued reconnect tokens. For
OpenAgents, the official world should avoid anonymous authority:

- OpenAgents users authenticate through the existing OpenAgents identity surface.
- A bridge service maps approved OpenAgents identities to SpacetimeDB identities.
- Service reducers require service identities or module-owner/admin roles.
- Public clients receive only public rows or view-filtered subsets.
- Private/internal data stays outside the MMO database unless explicitly modeled
  as safe public projection.

This should be written down before any production module is deployed, because
SpacetimeDB modules are exposed to the internet and authorization lives in module
logic.

## How This Drives `/tassadar`

The page should eventually become a SpacetimeDB-backed world client:

1. Connect to `openagents-world`.
2. Subscribe to the canonical Tassadar run query set.
3. Render `pylon_node`, `run_entity`, and `world_edge` rows as real Three.js
   entities.
4. Show a small HUD legend for status categories.
5. Open an inspector by subscribing to the selected entity's `proof_ref`,
   `settlement_ref`, and `world_event` rows.
6. Animate only row-backed events or explicit user focus transitions.

This turns the current "living run" idea into a real database/client contract.
The visual rule remains unchanged: every meaningful glow, line, pulse, badge, or
event must resolve to a row with a source reference.

## Prototype Plan

The first self-hosted deployment is live at
`https://spacetime.openagents.com`. The initial `openagents-world` module
source lives in `apps/openagents-world-spacetimedb/`. Its admin runbook is
`docs/game/2026-06-17-spacetimedb-admin-runbook.md`, and the current
`/tassadar` connection plan is
`docs/game/2026-06-17-spacetimedb-tassadar-integration-next-steps.md`.

P0:

- Keep the published `openagents-world` SpacetimeDB module prototype in the
  dedicated `apps/openagents-world-spacetimedb/` app.
- Model only `training_run`, `run_entity`, `world_edge`, `proof_ref`,
  `settlement_ref`, `world_event`, `projection_cursor`, and `bridge_health`
  until the bridge proves the row contract.
- Write a bridge script that replays one existing public Tassadar projection into
  the module.
- Generate TypeScript bindings and build a small dev-only client adapter that
  maps row callbacks to a Three.js entity store.

P1:

- Add `pylon_node`, `presence_session`, `actor_identity`, and selected-entity
  inspector subscriptions.
- Add stale projection semantics and scheduled expiry.
- Add service-identity authorization checks.

P2:

- Add regions, teams/guilds, reputation snapshots, and chat.
- Evaluate whether the official world should be self-hosted or deployed to
  SpacetimeDB Maincloud.
- Define migration discipline. SpacetimeDB supports additive changes more
  comfortably than arbitrary schema migration, so early schema changes should
  remain small and resettable until the world contract stabilizes.

## Guardrails

- Do not vendor BitCraft or SpacetimeDB code into OpenAgents by default.
- Treat the `projects/spacetime` repos as reference material.
- Keep OpenAgents settlement, receipt, and training authority outside the world
  module.
- Do not put secrets, private prompts, private repository data, or wallet
  material in SpacetimeDB rows.
- Label simulated, replayed, stale, and live world state distinctly.
- Prefer subscriptions and typed generated bindings over polling.
- Keep visual motion tied to a real row, a real timestamped replay event, or a
  user interaction.
