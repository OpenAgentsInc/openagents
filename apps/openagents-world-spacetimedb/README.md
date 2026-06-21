# openagents-world SpacetimeDB Module

This is the minimal SpacetimeDB module for the OpenAgents world projection.
It is deployed to the self-hosted database named `openagents-world` at
`https://spacetime.openagents.com`.

The module is a projection and interaction layer only. It does not own
settlement, payout, training truth, product promises, receipt validation,
wallet state, private prompts, private repositories, or provider credentials.

## Tables

Public projection tables:

- `training_run`
- `run_entity`
- `world_edge`
- `proof_ref`
- `settlement_ref`
- `world_event`
- `projection_cursor`
- `bridge_health`

Public interaction tables:

- `world_region`
- `pylon_station`
- `agent_avatar`
- `avatar_position`
- `avatar_position_near`
- `avatar_position_far`
- `pylon_attention`
- `local_chat_message`
- `chat_bubble`
- `local_emote`
- `agent_intent`

Private authority tables:

- `module_owner`
- `service_identity`

Only allowlisted service identities may call projection reducers. The `init`
reducer stores the publishing identity as owner and service identity. Future
bridge identities can be added with `authorize_service_identity`.

Interaction tables are public because they drive the shared world UI. They are
still bounded interaction state, not product truth. Browser identities may only
join or leave a region, update their own derived avatar position, focus a pylon
station, send short local or pylon messages, emit an emote, and set their own
ephemeral intent. Browser reducers cannot create run truth, pylon truth, proof
refs, receipt refs, settlement refs, or product claims.

Service-only interaction reducers:

- `upsert_world_region`
- `upsert_pylon_station_from_projection`
- `ensure_pylon_agent_avatar`
- `record_system_world_message`
- `expire_interaction_rows`

Browser/user interaction reducers:

- `join_region`
- `leave_region`
- `set_avatar_position`
- `focus_pylon`
- `clear_pylon_focus`
- `send_local_message`
- `send_pylon_message`
- `send_emote`
- `set_agent_intent`

The starter Street region is a bounded multiplayer chunk even when the visible
road is rendered as a longer repeated/streamed scene. The current Tassadar
region contract is `x=-160..160`, `y=0..40`, and `z=-160..160`, with local
origin `(0, 0, 0)`, road direction `(0, 0, 1)`, and the starter pylon site at
offset `(24, 0, 0)` from the local origin. `world_region` also records adjacent
Street chunk refs for future traversal:
`region.run.tassadar.executor.20260615.street.prev` and
`region.run.tassadar.executor.20260615.street.next`. Position updates are
throttled to at most 10 Hz per avatar and reject jumps above the MVP movement
limit. The bounds, Street metadata, proximity radius, position cadence, and
stale-avatar TTL are published through `world_region`; reducers validate
station, avatar, chat, and emote writes against an existing region row. Local
messages are plain text, capped at 280 characters, rate-limited to one message
per avatar per second, marked `moderation_state="visible"`, and paired with
short-lived chat-bubble rows. `expire_interaction_rows` is a service reducer
that removes stale avatar positions using the region TTL and removes expired
attention, message, bubble, emote, and intent rows.

`avatar_position` remains the compatibility high-resolution public presence
table. #5892 adds two feed-specific public tables with the same row shape:
`avatar_position_near` updates on every accepted position write, and
`avatar_position_far` updates at most once per avatar per second. Clients can
keep using the single region-filtered compatibility table until row churn
requires split queries, then subscribe to the near table inside the local
window and the far table outside it.

The proximity subscription shape for clients is deliberately simple:

- read `world_region` by `region_ref` for bounds, Street metadata, proximity
  radius, and TTL;
- subscribe to `pylon_station` rows with the same `region_ref`;
- subscribe to `avatar_position` rows with the same `region_ref`, then join to
  `agent_avatar` by `avatar_ref` for the compatibility single-feed path;
- when split presence is enabled, subscribe to `avatar_position_near` for the
  local high-resolution window and `avatar_position_far` outside that window,
  joining each feed to `agent_avatar` by `avatar_ref`;
- subscribe to `pylon_attention` by joining active-region `pylon_station`
  rows to attention rows by `pylon_ref`;
- subscribe to `chat_bubble` by joining active-region `local_chat_message`
  rows to bubble rows by `message_ref`;
- subscribe to `agent_intent` by joining active-region `avatar_position` rows
  to intent rows by `avatar_ref`;
- subscribe to `local_chat_message` and `local_emote` directly by active
  `region_ref`;
- read `world_event`, `proof_ref`, and `settlement_ref` by selected public
  entity/run refs, not as anonymous motion.

The module carries btree indexes for the active subscription filters and joins:
`world_event.run_ref`, `pylon_station.region_ref`,
`avatar_position.region_ref`, `avatar_position_near.region_ref`,
`avatar_position_far.region_ref`, `pylon_attention.pylon_ref`,
`local_chat_message.region_ref`, `chat_bubble.message_ref`, and
`local_emote.region_ref`. `agent_avatar.avatar_ref`,
`avatar_position.avatar_ref`, `avatar_position_near.avatar_ref`,
`avatar_position_far.avatar_ref`, `pylon_station.pylon_ref`,
`local_chat_message.message_ref`, and `agent_intent.avatar_ref` are primary
keys and therefore already indexed for the join side of the region-scoped
subscriptions.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build --manifest-path apps/openagents-world-spacetimedb/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

The WASM artifact is:

```text
apps/openagents-world-spacetimedb/target/wasm32-unknown-unknown/release/openagents_world.wasm
```

Publish through the VM-local SpacetimeDB CLI as documented in
`docs/game/2026-06-17-spacetimedb-admin-runbook.md`.

## Client Bindings

The `/tassadar` browser adapter uses generated TypeScript bindings from this
module. They live in:

```text
apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings
```

Regenerate those bindings after public table or reducer schema changes:

```bash
~/.local/bin/spacetime generate \
  --lang typescript \
  --out-dir apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings \
  --module-path apps/openagents-world-spacetimedb
```

## Tassadar Bridge

The operator bridge projects the public Worker summary into the
`openagents-world` module. It reads only already-public data from:

```text
https://openagents.com/api/public/tassadar-run-summary
```

Dry-run the transform without writing rows:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs
```

Run the transform tests:

```bash
bun test apps/openagents-world-spacetimedb/scripts/tassadar-summary-transform.test.mjs
```

Apply the projection through IAP SSH and the VM-local SpacetimeDB CLI:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs --apply-vm
```

The bridge writes the public projection tables through service-only reducers
and records `bridge_health` with `record_bridge_success`. It also projects one
`pylon_station`, `agent_avatar`, and `avatar_position` row for each visible
public leaderboard pylon ref. Replaying the same summary is deterministic, and
`append_world_event` ignores existing event refs so live replay does not create
duplicate events.

## Public Activity Timeline Bridge

The public activity bridge projects the agent-readable activity timeline into
`world_event` rows only. It reads the same public-safe envelope served at:

```text
https://openagents.com/api/public/activity-timeline?limit=50
```

Dry-run the transform without writing rows:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-activity-timeline.mjs
```

Resume from a cursor or inspect the exact reducer call plan:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-activity-timeline.mjs \
  --since "2026-06-18T18:00:08.000Z:projection_gap:event.public.gap.1" \
  --json
```

Run the bridge checks:

```bash
bun test \
  apps/openagents-world-spacetimedb/scripts/tassadar-summary-transform.test.mjs \
  apps/openagents-world-spacetimedb/scripts/activity-timeline-transform.test.mjs
```

Apply the projection through IAP SSH and the VM-local SpacetimeDB CLI:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-activity-timeline.mjs \
  --apply-vm
```

This bridge preserves the timeline event cursor, public source refs, blocker
refs, caveat refs, source lag status, generated time, and expiry in each
`world_event.summary`. The Worker/D1 timeline remains the source projection;
SpacetimeDB is only the replay-safe live world projection. Replaying the same
timeline envelope is deterministic, and existing `world_event` refs are ignored
by the reducer so bridge retries do not create duplicate motion.
