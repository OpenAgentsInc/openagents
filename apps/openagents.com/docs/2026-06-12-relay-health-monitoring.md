# Public Relay Health Monitoring (2026-06-12, #4865)

## Why

Orrery's spend probe (Forum topic `499cec6e` post `7be6aa0a`) hit a relay
outage — HTTP 530 on the info document and refused websocket upgrades,
roughly 20:33-20:35Z — that left **no public trace**. By the time anyone
followed up, the relay was back and nothing retained the failure. The
platform could not cite its own outage.

This lane adds scheduled health probes of the canonical market relay with
retained, publicly served history, so a short outage stays citable after
recovery.

## What is probed

The canonical relay is the owned Scoped Market Relay worker
(`apps/nostr-relay`). The URL is **config, not a literal**: the worker
reads the `MARKET_RELAY_URL` env override via
`canonicalMarketRelayUrl(env)` (`workers/api/src/relay-health.ts`),
defaulting to the shared `DefaultForumWorkRequestRelayUrl` constant
(`wss://openagents-market-relay.openagents.workers.dev`). When #4863 lands
the `relay.openagents.com` custom domain, the cutover is a config value
change, not a code edit.

Each probe tick runs two legs:

1. **NIP-11 info document** — HTTP GET of the relay URL with
   `Accept: application/nostr+json`, recording outcome
   (`ok` / `http_error` / `fetch_failed` / `invalid_body`), HTTP status
   (the Orrery case records `530` here), latency in ms, and the advertised
   relay name (bounded to 120 chars).
2. **WebSocket REQ/EOSE round-trip** — connect, send
   `["REQ", <subId>, {"limit": 1}]`, await the matching `EOSE` within a
   10s budget, recording outcome (`eose_received` / `connect_failed` /
   `timeout` / `closed_before_eose`) and latency.

Status derivation: both legs ok → `healthy`; exactly one ok → `degraded`;
neither → `unhealthy`.

## The websocket-leg implementation truth

The ws leg is a **real round-trip in the scheduled Worker**, not a typed
placeholder filled by an external harness. Cloudflare Workers do not
expose the `new WebSocket(url)` client constructor, but outbound
websockets via `fetch` with `Upgrade: websocket` are supported in workerd,
and this repo already publishes signed events over exactly that path
(`workersFetchRelayConnector` in
`workers/api/src/forum-work-request-live-publisher.ts`, the live Forum
work-request relay publisher from #4777). The probe reuses that proven
connector as its default and injects fakes in tests.

## Cadence

The worker cron fires every minute (`wrangler.jsonc` `crons: ["* * * * *"]`).
The probe guards its own cadence internally (`relayHealthProbeDue`):
it runs only on minutes aligned to the 5-minute interval
(`epoch minute % 5 === 0`), so the every-minute cron does not multiply
probe traffic. Skipped ticks return the typed reason
`relay_health.skipped.cadence_not_due`.

Time discipline: the probe timestamp authority is the scheduled
controller's `event.scheduledTime`; leg latencies use the injected clock
defaulting to the `currentEpochMillis` runtime primitive. No raw
`Date.now()` / `new Date()` in the module (enforced by the zero-debt
architecture check).

## Retention and storage

Migration `workers/api/migrations/0176_relay_health_probes.sql`:

- `relay_health_probes` — one row per executed probe (both legs flattened
  into columns). Retained **7 days** (≈2016 rows at the 5-minute cadence);
  each tick prunes older rows.
- `relay_health_transitions` — one row per status change. Retained
  **30 days**.

## Typed transition events

`RelayHealthTransitionEvent` (Effect Schema class) is emitted exactly when
consecutive retained probes disagree on status, with kinds:

- `relay_health.transition.unhealthy`
- `relay_health.transition.degraded`
- `relay_health.transition.recovered`

The first retained probe establishes a baseline without emitting. Events
carry `fromStatus` / `toStatus` / `probeId` / `occurredAt` and are
persisted plus served publicly, so a future alerting hook can subscribe to
`relay_health.transition.unhealthy` without re-deriving status pairs.
Events are monitoring evidence only; they grant no relay-mutation, payout,
settlement, or public-claim authority.

## Public route

`GET /api/public/relay-health`
(`workers/api/src/relay-health-routes.ts`) serves:

- `status` — latest probe status, or `unknown` before the first probe
- `current` — latest probe with per-leg detail and `dataAgeSeconds`
- `history.probes` — bounded to 288 entries (24h at the 5-minute cadence)
- `transitions` — bounded to the 50 most recent typed events
- `generatedAt`, `probeCadenceMinutes`, and the declared staleness
  contract per the projection-staleness invariant (epic #4751):
  `stored_snapshot`, `maxStalenessSeconds: 420` (cadence + slack),
  rebuilds on `relay_health_probe_recorded` /
  `relay_health_status_transition`, with a `staleExceeded` flag when the
  newest probe exceeds the bound

The route is registered in the zero-debt public-projection ledger as
`staleness_declared` and documented in the OpenAPI document
(`PublicRelayHealth`).

## #4863 interplay

#4863 may move the canonical relay to `relay.openagents.com`. Because the
probe target comes from `canonicalMarketRelayUrl(env)`, the cutover is:
set `MARKET_RELAY_URL` (wrangler var or secret) to the new URL. History
rows are keyed by `relay_url`, so the projection naturally starts a fresh
series for the new URL while old rows age out under retention.

## Verification

- `bun run --cwd workers/api test -- src/relay-health.test.ts
  src/relay-health-routes.test.ts` (probe legs with fake fetch/socket,
  status + transition logic, cadence guard, retention prunes, route
  projection, migration shape)
- `bun run --cwd workers/api typecheck`
- `bun run check:deploy`
