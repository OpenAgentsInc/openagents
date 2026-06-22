# Cloudflare World Production Release Receipt

Date: 2026-06-22
Issue: #5972
Tracker: #5959

## Release Target

- Production Worker: `openagents-world`
- Production URL: `https://openagents-world.openagents.workers.dev`
- Production version ID: `76bd5d07-765c-4ea3-881e-8e9e2b7a5495`
- Production D1 database: `openagents-world`
- Production D1 ID: `022446b6-ca60-4045-9126-7519ad0c1df7`
- Production queue: `openagents-world-bridge`
- Staging D1 database: `openagents-world-staging`
- Staging D1 ID: `99fdeed0-e3fa-4a70-87f5-7c5aef984cce`
- Staging queue: `openagents-world-bridge-staging`

## Live Checks

- `GET /health`: returned `ok: true`, `envName: "production"`,
  schema `openagents.world_contract.v1`, and service version `0.1.0`.
- `GET /version`: returned Worker/contract/schema version metadata.
- `GET /connect?region=region.run.tassadar.executor.20260615.street`:
  returned the WebSocket URL for the starter region and a server-approved
  subscription plan.
- `POST /bridge/ingest`: accepted public-safe rows for source
  `source.public.p13.smoke`, cursor `cursor.p13.smoke.1`, and persisted
  deterministic projection rows without private payloads.
- D1 projection row count after ingest:
  `bridge_health=1`, `projection_cursor=1`, `training_run=1`,
  `world_region=1`.

## Client Smokes

- Web smoke imported `startTassadarCloudflareWorldSubscription`, connected to
  production with a browser-like WebSocket, joined the starter region, moved the
  local avatar, and sent local chat. Result: `summaryCount=9`,
  `regionCount=1`, `avatarCount=2`, `positionCount=2`, `chatCount=1`.
- Desktop smoke imported `subscribeCloudflareWorld` and
  `publishActiveVerseLocalPose`, started two independent desktop subscriptions,
  and moved the second desktop avatar. Result: subscriber A received
  `maxAgents=2`, subscriber B received `maxAgents=1`, both connected through
  `https://openagents-world.openagents.workers.dev`.
- The smoke uncovered and fixed the release-critical command fanout gap: region
  command deltas now broadcast to every hibernatable WebSocket in the Durable
  Object, not only the command sender.

## Test Gate

- `bun run --cwd apps/openagents-world typecheck`
- `bun run --cwd apps/openagents-world test`
- `bun run typecheck:world-contract`
- `bun run test:world-contract`
- `bun run typecheck:world-client`
- `bun run test:world-client`
- `bun run --cwd apps/autopilot-desktop test tests/chat-world-subscriptions.test.ts tests/chat-world-cloudflare.test.ts tests/chat-world-visualization.test.ts`
- `bun run --cwd apps/openagents.com/apps/web test src/scene/tassadarCloudflareWorld.test.ts`
- `bun run --cwd apps/openagents.com check:agent-doc-links`
- `bun run check:deploy`

## Historical Infrastructure Archive

The old self-hosted SpacetimeDB production VM path is no longer active:

- GCE instance `spacetimedb-world-1` in project `openagentsgemini`,
  zone `us-central1-a`: `TERMINATED`.
- Instance labels: `openagents_state=historical_archived`,
  `openagents_replaced_by=cloudflare_world`,
  `openagents_cutover_issue=5972`.
- Disks `spacetimedb-world-1` and `spacetimedb-world-data-1`: retained as
  historical archives and labeled with the same cutover metadata.
- Firewall rule `oa-allow-spacetimedb-world-http-https`: disabled.
- Static address `spacetimedb-world-ip` remains attached to the terminated
  historical instance until a separate DNS/address cleanup decision releases
  the old name and address.

## Old Backend Diff

No committed one-shot SpacetimeDB diff helper remains after P11. The final
release gate used the public bridge ingest sample and D1 row counts above as
the durable cutover sample; the old VM was archived immediately after Cloudflare
production smokes passed so no active SpacetimeDB serving path remains.
