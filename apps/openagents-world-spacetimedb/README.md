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

- `pylon_station`
- `agent_avatar`
- `avatar_position`
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

The MVP world bounds are intentionally small and flat:
`x=-8..8`, `y=0..4`, and `z=-6..6`. Position updates are throttled to at most
10 Hz per avatar and reject jumps above the MVP movement limit. Local messages
are plain text, capped at 280 characters, marked `moderation_state="visible"`,
and paired with short-lived chat-bubble rows. `expire_interaction_rows` is a
service reducer that removes stale avatar positions and expired attention,
message, bubble, emote, and intent rows.

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
and records `bridge_health` with `record_bridge_success`. Replaying the same
summary is deterministic, and `append_world_event` ignores existing event refs
so live replay does not create duplicate events.
