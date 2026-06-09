# Deprecated Laravel Homepage Live Pylon Count Audit

Date: 2026-06-08

Source repo: `/Users/christopherdavid/work/deprecated/openagents.com`

Destination context: OpenAgents product surface docs. This is an audit of the previous Laravel /
Inertia `openagents.com` homepage implementation that showed live Pylon counts.
It is evidence for future OpenAgents product surface migration work, not a production contract change.

## Location

The old site is the deprecated Laravel/Inertia clone:

- repo: `deprecated/openagents.com`
- branch inspected: `main`
- current note: the repo had unrelated local edits in auth files during this
  audit, so the implementation was treated as read-only evidence.

The homepage Pylon stats implementation was not in the active `openagents/`
Rust repo. It lived in the Laravel app and its React page:

- `app/Http/Controllers/HomeController.php`
- `app/Services/PublicStats.php`
- `app/Services/NexusStats.php`
- `app/Services/PublicStatsBroadcaster.php`
- `resources/js/pages/welcome.tsx`
- `resources/js/lib/public-stats.ts`
- `resources/js/lib/homepage-pylon-stats-display.ts`
- `routes/web.php`
- `routes/console.php`
- `database/migrations/0001_01_01_000004_create_observed_pylons_table.php`
- `database/migrations/0001_01_01_000005_create_public_stat_counters_table.php`

Related browser and installer telemetry lived in:

- `app/Http/Controllers/PublicTelemetryEventController.php`
- `app/Models/PublicTelemetryEvent.php`
- `resources/js/lib/public-telemetry.ts`
- `database/migrations/0001_01_01_000003_create_public_telemetry_events_table.php`

## Relevant History

The live Pylon-count homepage landed in a short sequence:

- `0acc6eb` on 2026-04-07: added homepage Pylon stats, the
  `observed_pylons` table, and a persistent public counter table.
- `7cd3ed4` on 2026-04-08: added `GET /live/pylon-stats` and browser polling.
- `de2d05b` on 2026-04-08: added public stats broadcasting over Reverb.
- `e524d96` on 2026-04-08: trimmed the Reverb event payload.
- `431a050` on 2026-04-09: moved the homepage to a durable local Nexus
  snapshot instead of fetching Nexus directly on every homepage request.
- `80910b0`, `312fe8a`, and `0ba3498` on 2026-04-26: reduced live stats load,
  trimmed broadcast payloads, and made live stats read from the local snapshot.
- `9c0cda7` on 2026-04-28: rejected stale Nexus recovery-proxy stats.

Later Pylon work added admin, account-linking, Pylon Codex, and capability
surfaces, but the homepage count path above is the core public implementation.

## Request Flow

Initial page load:

1. `GET /` routes to `HomeController::__invoke`.
2. Non-admin users render Inertia page `welcome`.
3. `HomeController` passes `pylonStats` from `PublicStats::homePageInitialProps`.
4. `PublicStats` delegates to `NexusStats::homePageInitialProps`, which reads
   the local Nexus stats snapshot through `localStatsPageProps`.
5. `welcome.tsx` initializes display state from `pylonStats` and renders the
   four homepage stat cells: `Pylons total`, `Seen in last 24h`, `Online now`,
   and `Payouts paid`.

Live browser update:

1. `routes/web.php` exposes `GET /live/pylon-stats` as
   `home.pylon-stats`.
2. `HomeController::stats` returns `PublicStats::homePageProps()` as JSON with
   `Cache-Control: no-store, no-cache, must-revalidate, max-age=0`.
3. `welcome.tsx` imports the Wayfinder action for `HomeController.stats`, calls
   `fetch(homePylonStats.url(), { cache: 'no-store' })`, and refreshes every
   three seconds.
4. If Reverb is configured, the page also subscribes to channel
   `public.stats` and event `PublicStatsUpdated`.
5. `PublicStatsBroadcaster` sends a small payload containing only `home` and
   `publishedAtUnixMs` when the public stats snapshot changes.
6. `routes/console.php` owns `public-stats:broadcast`, with `--watch`,
   `--interval`, and `--force` options for the Laravel Cloud/Reverb runtime.

Display-state smoothing:

- `homepage-pylon-stats-display.ts` keeps the last live stats visible for a
  nine-second grace window before allowing an unavailable/stale response to
  degrade the homepage badge.
- The homepage status label is intentionally coarse: `Pylon stats` when the
  state is live/stale and `Pylon stats unavailable` when unavailable.

## Data Source

The Laravel app did not calculate live Pylon presence from its own browser
traffic. It read a public Nexus stats endpoint:

- default URL: `https://nexus.openagents.com/api/stats`
- config key: `services.nexus.stats_url`
- env var: `NEXUS_STATS_URL`

`NexusStats::fetchNexusStats` made an HTTP JSON request to that endpoint with
short connect/request timeouts, two retry attempts, and forced IPv4 resolution.
After `9c0cda7`, it rejected responses marked by the recovery proxy as served
or stale through `X-Nexus-Recovery-Proxy-Cache`.

The mapped Nexus fields that drove the homepage were:

- `pylons_online_now` -> `pylonsOnlineNow`
- `pylons_seen_24h` -> `pylonsSeen24h`
- `recent_pylons` and `recent_pylon_diagnostics` -> observed identity records
  and fallback total calculations
- payout fields such as `nexus_payout_sats_paid_total`

The TypeScript surface for homepage stats was `HomePagePylonStats` in
`resources/js/lib/public-stats.ts`.

## Count Semantics

`Online now` was the direct live Nexus value:

- source key: `pylons_online_now`
- PHP mapped field: `pylonsOnlineNow`
- React stat id: `pylons-online`
- display label: `Online now`

`Pylons total` was a durable lower-bound style value, not just the current live
online count. `NexusStats` combined available Nexus stats with locally retained
identity observations:

- recent Nexus `recent_pylons` rows were normalized by `nostr_pubkey_short`
  into an `identity_key`.
- recent `recent_pylon_diagnostics` rows could also produce observations.
- observations were upserted into `observed_pylons` with `first_seen_at` and
  `last_seen_at`.
- `observed_pylons` had a unique `identity_key`, so repeat sightings updated
  the same row.
- `public_stat_counters` held monotonic max counters such as
  `pylons_total_lower_bound` and `pylons_online_peak`.

This means the homepage could show:

- a current live count from Nexus;
- a 24-hour seen count from Nexus;
- a retained total/lower-bound count from local observed identities and
  maximum counters.

## Snapshot And Caching Behavior

The later implementation intentionally avoided making every homepage request a
fresh Nexus request.

`NexusStats` used:

- short in-process cache: `public-stats.nexus`, 3 seconds;
- last-good cache: `public-stats.nexus.last-good`, 3600 seconds;
- metadata cache: `public-stats.nexus.meta`, 86400 seconds;
- durable database snapshot table key: `public_stat_snapshots` / `nexus`;
- minimum refresh interval: 15 seconds;
- stale threshold: 30 seconds.

`homePageInitialProps`, `homePageProps`, and `PublicStats::broadcastSnapshot`
eventually read the local snapshot. The `public-stats:broadcast` console command
called `NexusStats::refreshLocalSnapshot` before broadcasting, so the watch
process became the normal refresh driver.

## Telemetry Coupling

The homepage also emitted public telemetry, but that telemetry did not power
the live Pylon number.

`welcome.tsx` emitted:

- `homepage_viewed`
- `homepage_copy_agent_instructions_clicked`
- `homepage_copy_agent_instructions_succeeded`
- `homepage_copy_agent_instructions_failed`
- `homepage_instructions_preview_opened`
- outbound social link click events

The public telemetry endpoint accepted website and installer events, capped
properties at 16 KB, stored them in `public_telemetry_events`, and cleared the
public telemetry cache. `PublicStats` used those records for install/version
stats on the stats page.

## Migration Notes For OpenAgents product surface

OpenAgents product surface should not copy the old Laravel shape blindly. The useful migration
shape is:

- keep Nexus as the source of truth for live fleet presence;
- keep the public homepage projection small and public-safe;
- maintain a durable local last-good snapshot so the homepage does not hammer
  Nexus;
- preserve distinct semantics for `online now`, `seen in 24h`, `total observed`,
  and `peak online`;
- treat installer/homepage telemetry as funnel analytics, not fleet authority;
- avoid private node identifiers in public projection; the old public count used
  short identity keys only for local dedupe/storage, not as a public list.

For OpenAgents product surface, this belongs behind the existing public projection and Pylon claim
boundaries in `INVARIANTS.md`. A future code port should add regression tests
for:

- Nexus snapshot mapping;
- stale/recovery-cache rejection;
- last-good fallback behavior;
- no-store JSON endpoint behavior;
- public broadcast payload minimization;
- public-safe redaction of node identity, wallet, payment, and raw runtime data.
