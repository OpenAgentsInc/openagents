# OpenAgents product surface Homepage Pylon v0.2.5 Live Stats Plan

Date: 2026-06-08

Status: implementation plan.

Related audit:

- `docs/pylon/2026-06-08-deprecated-laravel-homepage-live-pylon-count-audit.md`

## Implementation Progress

- 2026-06-08: Issue #518 added version-bearing Pylon registration and
  heartbeat state. OpenAgents product surface now stores `client_version`,
  `client_protocol_version`, latest heartbeat status/resource mode, and latest
  public health/load/capacity refs on `pylon_api_registrations` for future
  v0.2.5+ stats projection work.
- 2026-06-08: Issue #519 replaced the production
  `GET /api/public/pylon-stats` snapshot path with an OpenAgents product surface-owned projection
  from Pylon API registrations. The endpoint now counts only active
  v0.2.5+ registrations, separates online, seen, wallet-ready, and
  assignment-ready states, and keeps Nexus payout fields as nullable
  compatibility fields until a separate receipt aggregate exists.
- 2026-06-08: Issue #520 wired the logged-out homepage to load the public
  Pylon stats endpoint and render a compact v0.2.5+ stats strip. Homepage copy
  is limited to online, seen-in-24h, wallet-ready, version-floor, and
  freshness signals.

## Goal

Show live Pylon stats on the public `openagents.com` homepage for the new
Pylon v0.2.5+ line, using OpenAgents product surface-owned Pylon API state instead of rebuilding the
old Laravel/Nexus stats setup.

The first homepage surface should answer a narrow public question:

- how many v0.2.5+ Pylons are online now;
- how many v0.2.5+ Pylons have checked in recently;
- how many online Pylons are wallet-ready or assignment-ready; and
- when the public projection was last refreshed.

It must not imply that an online Pylon is eligible for paid work, has accepted
work, has been paid, or is settled.

## Current OpenAgents product surface State

OpenAgents product surface already has two relevant pieces.

First, the old-style public stats endpoint exists:

- `workers/api/src/public-pylon-stats.ts`
- `workers/api/src/public-pylon-stats-routes.ts`
- `workers/api/src/public-pylon-stats.test.ts`
- route: `GET /api/public/pylon-stats`

That endpoint currently fetches `https://nexus.openagents.com/api/stats` and
maps Nexus fields such as `pylons_online_now`, `pylons_seen_24h`, and
`recent_pylons`.

Second, the newer OpenAgents product surface Pylon API exists:

- `workers/api/src/pylon-api.ts`
- `workers/api/src/pylon-api-routes.ts`
- `workers/api/src/pylon-api-routes.test.ts`
- `workers/api/migrations/0123_pylon_agent_api.sql`
- `workers/api/migrations/0134_pylon_api_assignment_leases.sql`

The new API already records:

- `POST /api/pylons/register`
- `POST /api/pylons/{pylonRef}/heartbeat`
- `POST /api/pylons/{pylonRef}/wallet-readiness`
- `POST /api/pylons/{pylonRef}/payout-target-admission`
- assignment accept/progress/artifact/payment/settlement events

The D1 tables already include:

- `pylon_api_registrations.latest_heartbeat_at`
- `pylon_api_registrations.status`
- `pylon_api_registrations.resource_mode`
- `pylon_api_registrations.wallet_ready`
- `pylon_api_registrations.public_projection_json`
- `pylon_api_events.event_kind`
- `pylon_api_events.status`
- `pylon_api_events.public_projection_json`

The public web app also already knows how to load `GET /api/public/pylon-stats`
for `/artanis`:

- `apps/web/src/page/loggedOut/update.ts`
- `apps/web/src/page/loggedOut/model.ts`
- `apps/web/src/page/loggedOut/page/publicAgent.ts`

The root logged-out homepage currently does not load Pylon stats.

## Proposed Source Of Truth

Use OpenAgents product surface's Pylon API state as the public stats authority for v0.2.5+.

Do not fetch Nexus for homepage live counts. Nexus/Treasury/Pylon receipt
surfaces can remain separate evidence paths for payout and settlement, but the
homepage online count should come from OpenAgents product surface-accepted registration and heartbeat
records.

The public projection should be read-only and generated from:

- active `pylon_api_registrations`;
- recent heartbeat material from `pylon_api_registrations.latest_heartbeat_at`;
- wallet readiness from `pylon_api_registrations.wallet_ready`;
- assignment readiness from `pylon_api_assignments` only as a separate count,
  never as payout evidence;
- public-safe status/version/resource-mode fields materialized from the latest
  accepted registration and heartbeat.

## Required Schema Additions

The current Pylon API accepts public-safe refs for capabilities, health, load,
and capacity, but it does not store a typed client version field. For a v0.2.5+
homepage count, add typed version state rather than inferring version from free
text refs.

Add a D1 migration to extend `pylon_api_registrations`:

- `client_version TEXT`
- `client_protocol_version TEXT`
- `latest_heartbeat_status TEXT`
- `latest_resource_mode TEXT`
- `latest_health_refs_json TEXT NOT NULL DEFAULT '[]'`
- `latest_load_refs_json TEXT NOT NULL DEFAULT '[]'`
- `latest_capacity_refs_json TEXT NOT NULL DEFAULT '[]'`

Extend request schemas in `workers/api/src/pylon-api.ts`:

- `PylonApiRegistrationRequest.clientVersion`
- `PylonApiRegistrationRequest.clientProtocolVersion`
- `PylonApiHeartbeatRequest.clientVersion`
- `PylonApiHeartbeatRequest.clientProtocolVersion`

Version parsing should be a typed helper with bounded semantics:

- accept plain semver like `0.2.5`;
- accept known Pylon labels like `pylon-v0.2.5` or
  `openagents.pylon@0.2.5`;
- normalize into `{ major, minor, patch, label }`;
- reject unknown prose and private material.

This is deterministic parsing of a bounded version field, not user-intent
routing.

## Public Projection Contract

Replace the Nexus-derived shape behind `GET /api/public/pylon-stats` with an
OpenAgents product surface projection, while keeping short-term compatibility fields until the
frontend and Artanis report can be updated.

New canonical fields:

- `available`
- `status`
- `error`
- `sourceUrl`
- `asOfUnixMs`
- `asOfLabel`
- `minimumClientVersion`
- `pylonsOnlineNow`
- `pylonsSeen24h`
- `pylonsRegisteredTotal`
- `pylonsWalletReadyNow`
- `pylonsAssignmentReadyNow`
- `pylonsByResourceMode`
- `pylonsByClientVersion`
- `recentPylons`
- `caveatRefs`
- `sourceRefs`

Compatibility fields can remain for one deploy cycle:

- `pylonSessionsOnlineNow`
- `sellablePylonsOnlineNow`
- `nexusPayoutSatsPaidTotal`
- `nexusAcceptedWorkPayoutSatsPaidTotal`
- `nexusAcceptedWorkPayoutSatsPaid24h`
- training contributor counts

For the OpenAgents product surface-backed projection:

- `sourceUrl` should become `https://openagents.com/api/public/pylon-stats`;
- payout fields should be `null` unless a separate public-safe receipt
  aggregator explicitly supplies them;
- `sellablePylonsOnlineNow` should not be used as an eligibility claim unless
  backed by separate admission/policy evidence. Current public copy should use
  `pylonsWalletReadyNow` and `pylonsAssignmentReadyNow` for the OpenAgents product surface-backed
  homepage/report stats.

## Online Count Semantics

Use explicit windows:

- online now: active registration, v0.2.5+ normalized version, heartbeat within
  the last five minutes, and latest heartbeat status in an online/ready family;
- seen in 24h: active registration, v0.2.5+ normalized version, heartbeat
  within 24 hours;
- wallet ready now: online now plus `wallet_ready = 1`;
- assignment ready now: online now plus wallet ready plus no explicit blocked
  registration status; do not include payout target claims unless admission
  evidence is present.

Keep these states separate:

- online;
- wallet-ready;
- assignment-ready;
- payout-target admitted;
- assigned;
- accepted work;
- paid;
- settled.

The homepage may show online, seen, wallet-ready, and assignment-ready. It
should not show paid or settled counts until a separate public receipt
projection is intentionally added.

## Backend Implementation Steps

1. Add the Pylon API version/status migration.

2. Extend `PylonApiRegistrationRecord` and `PylonApiHeartbeatRequest` in
   `workers/api/src/pylon-api.ts`.

3. Update registration and heartbeat write paths so heartbeat updates
   materialized registration fields:
   - `latest_heartbeat_at`;
   - `latest_heartbeat_status`;
   - `latest_resource_mode`;
   - `client_version`;
   - `client_protocol_version`;
   - latest health/load/capacity refs.

4. Add a small stats repository or service, for example
   `workers/api/src/public-pylon-stats-store.ts`, that queries D1 and returns
   only the public projection.

5. Refactor `workers/api/src/public-pylon-stats.ts` so the primary path builds
   from the OpenAgents product surface store. Keep the existing Nexus mapper as a temporary fallback
   fixture only if Artanis tests still need it.

6. Update `handlePublicPylonStatsApi` dependencies so the route can access D1
   instead of calling global `fetch`.

7. Done in #521: `openagents-openapi.ts` and
   `openagents-capability-manifest.ts` describe the OpenAgents product surface-backed stats source,
   canonical v0.2.5+ fields, and online-versus-payment boundary.

8. Done in #521: Artanis public report adapters read the new field names and
   stop describing the source as Nexus public stats for this homepage count.

## Frontend Implementation Steps

1. Update `apps/web/src/page/loggedOut/model.ts` `PublicPylonStats` schema with
   the new OpenAgents product surface fields while accepting compatibility fields during rollout.

2. Keep `LoadPublicPylonStats` in
   `apps/web/src/page/loggedOut/update.ts`, still fetching
   `/api/public/pylon-stats` with `cache: 'no-store'`.

3. Add homepage initial command wiring so route `_tag === 'Home'` loads public
   Pylon stats. Today only public-agent routes load it.

4. Add a compact homepage Pylon stats section in
   `apps/web/src/page/loggedOut/page/home.ts`.

   The first version should show:
   - `Online now`;
   - `Seen in 24h`;
   - `Wallet ready`;
   - `v0.2.5+ floor`;
   - a small freshness line.

5. Keep public-agent `/artanis` stats working, but rename copy away from
   "Nexus connection" once the endpoint is OpenAgents product surface-backed.

6. Avoid public UI copy that explains internal dispatch, payout, or settlement
   mechanics. Use direct labels and caveats.

## Testing Plan

Worker tests:

- update `workers/api/src/pylon-api-routes.test.ts` for version-bearing
  registration and heartbeat writes;
- add `workers/api/src/public-pylon-stats.test.ts` cases for D1-backed stats;
- prove v0.2.4 registrations do not count toward the v0.2.5+ homepage metric;
- prove stale heartbeat records do not count as online;
- prove wallet-ready and assignment-ready counts are separate from online;
- prove raw wallet, invoice, payout target, private path, provider secret, raw
  timestamp, and raw log material are rejected or absent from projection;
- preserve `405` behavior for non-GET stats requests.

Frontend tests:

- update `apps/web/src/main.test.ts` so the logged-out home route includes
  `LoadPublicPylonStats`;
- add/update a logged-out home view test showing the Pylon stats section with a
  loaded model;
- keep `/artanis` public-agent coverage passing.

Deploy checks:

- run `bun run check:deploy` after code changes;
- for UI work, run the local dev server and inspect desktop/mobile screenshots
  for the homepage stats block.

## Rollout Plan

1. Ship backend schema and write-path support first.
2. Release or update Pylon v0.2.5+ so it sends typed version-bearing
   registration and heartbeat payloads to OpenAgents product surface.
3. Done in #519: ship the OpenAgents product surface-backed `/api/public/pylon-stats` projection
   behind tests.
4. Done in #520: ship homepage UI using the existing endpoint.
5. Done in #521: leave the old Nexus mapper available only as an explicit
   compatibility fixture while Artanis/public report tests use OpenAgents product surface source
   semantics.
6. Monitor the public projection for a day before adding stronger public copy.

## Open Questions

- What exact status strings will Pylon v0.2.5+ heartbeat send for online,
  ready, paused, blocked, and shutting down?
- Should assignment-ready require payout-target admission, or should the
  homepage keep that count to wallet-ready only until payout-target admission
  has a public-safe aggregate?
- Should `pylonsRegisteredTotal` be a monotonic lower-bound counter, or simply
  the active v0.2.5+ registration count?
- Should public recent Pylons include display labels at all, or only aggregate
  counts by resource mode and version?
