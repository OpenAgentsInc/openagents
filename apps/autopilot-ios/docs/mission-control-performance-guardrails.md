# Mission Control Performance Guardrails (iOS WGPUI)

Status: active (`OA-IOS-WGPUI-CODEX-017`)

This document defines the runtime defaults and operator-selectable tradeoffs used to keep Mission Control responsive on mobile under high Codex event rates.

## Guardrails

1. Coalesced bridge flush cadence is capped to `100-250ms`.
2. Event and timeline stores are bounded by retention profiles.
3. Mission overview event rows render payload previews only.
4. Event Inspector performs expensive payload pretty-format only after explicit expand.
5. Thread timelines and overview tapes are viewport-virtualized in WGPUI.

## Retention Profiles

Profiles are selectable from the Mission header (`Compact` / `Balanced` / `Extended`).

- `Compact`
  - cadence: `100ms`
  - max events: `512`
  - max per-thread timeline entries: `160`
  - target: battery-sensitive / long-running monitoring
- `Balanced` (default)
  - cadence: `160ms`
  - max events: `1024`
  - max per-thread timeline entries: `320`
  - target: general operator usage
- `Extended`
  - cadence: `240ms`
  - max events: `2048`
  - max per-thread timeline entries: `640`
  - target: richer short-term debugging context

## Tradeoffs

1. Lower cadence and smaller rings reduce CPU/memory pressure and battery usage, but shorten in-memory investigation history.
2. Larger rings preserve more local history, but can increase per-flush work.
3. Payload previews in overview keep scanning fast; full payload expansion is deliberate and scoped to inspector workflow.

## Operator Flows (Watchlists, Alerts, Continuity)

1. Watchlist the active lane from Mission overview:
  - `Watch active` toggles the current `(worker_id, thread_id)` in the watchlist.
  - Lane watchlist state is deterministic (normalized, deduped, sorted by worker/thread id).
2. Scope Mission overview to watchlist lanes:
  - `WL on/off` toggles watchlist-only mode.
  - If watchlist is empty, Mission falls back to showing all lanes (no empty-screen trap).
3. Change timeline ordering:
  - `New/Old` toggles newest-first vs oldest-first in the overview fold.
4. Configure alert predicates:
  - `Err on/off`: emits `mission/alert/error_event` when an ingested event has error severity.
  - `Stk on/off`: emits `mission/alert/stuck_turn` when a turn crosses `events_since_turn_started >= 40`.
  - `Rec on/off`: emits `mission/alert/reconnect_storm` when `>= 3` of the last `6` reconnect samples are non-live.
5. Cross-device continuity:
  - Preferences persist locally in iOS defaults and are also synced to worker metadata key `autopilot_ios_mission_control`.
  - On worker summary sync, the newest valid remote snapshot (`updated_at`) is adopted and applied to the Rust mission store.
  - Preferences include watchlist, watchlist-only, order, filter, pin-critical, and alert rules.

## Failure Modes and Recovery Behavior

1. No active lane for watch action:
  - `Watch active` does not mutate state and surfaces `"No active lane available to watchlist."`.
2. Remote sync failure:
  - Local preference state still applies immediately and persists locally.
  - Remote upsert failure records lifecycle event `mission_preferences_sync_failed ...`; stream/replay lane stays intact.
3. Remote snapshot incompatibility:
  - Snapshot is ignored when schema version does not match current `schema_version`.
4. Concurrent device edits:
  - Last-writer-wins by parsed `updated_at` timestamp (string fallback comparison if parsing fails).
5. Replay/watermark safety:
  - Preference sync is out-of-band metadata; it does not alter Khala resume checkpoints or mission replay dedupe.

## Verification

Recommended checks:

```bash
cargo test -p wgpui --features ios mission_density_tests -- --nocapture
./scripts/local-ci.sh ios-codex-wgpui
```

Benchmark smoke evidence is emitted by:

- `mission_density_tests::fold_benchmark_smoke_for_high_event_density`
