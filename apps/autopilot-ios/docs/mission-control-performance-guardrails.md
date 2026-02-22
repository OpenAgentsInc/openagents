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

## Verification

Recommended checks:

```bash
cargo test -p wgpui --features ios mission_density_tests -- --nocapture
./scripts/local-ci.sh ios-codex-wgpui
```

Benchmark smoke evidence is emitted by:

- `mission_density_tests::fold_benchmark_smoke_for_high_event_density`
