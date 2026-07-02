# Khala Code QA Metrics Bridge

Date: 2026-07-02
Status: implemented for Khala Code desktop preview and packaged Electrobun runs.

## Contract

Khala Code webview metrics are recorded in two places:

- The renderer keeps the local `window.__khalaCode.qaMetrics()` snapshot for
  interactive debugging.
- Every `pushQaMetricSample(...)` call also publishes the same sample to the
  Bun host through the schema-first `qaMetricSample(sample)` RPC request.

The Bun host owns the real-run authoritative buffer exposed by
`qaMetrics()`. That buffer is capped, cloned on write/read, and evaluated
against the shared `khalaCodeQaMetricBudgets` data before it is returned.

The bridge uses the same RPC schema path in browser preview and packaged
Electrobun:

- browser preview: renderer calls `/rpc/qaMetricSample`; `/rpc/qaMetrics`
  reads the host snapshot.
- browser preview: `/rpc/events` chat-turn SSE is decoded through the shared
  chat event schema and applied to the renderer, so `sse.event_to_ui_ms` and
  `turn_start.first_event_ms` can be sampled in Mode D, not only in packaged
  native RPC.
- packaged app: renderer calls the native Electrobun `qaMetricSample` request;
  `qaMetrics` reads the same host snapshot.

`qaMetricSample` is classified as read-only-safe preview telemetry. It is
allowed while preview mutation RPC methods are blocked.

## Covered Samples

The existing renderer call sites now flow through the bridge automatically,
including:

- `thread_switch.rpc_ms`
- `thread_switch.optimistic_render_ms`
- `thread_switch.full_render_ms`
- `thread_switch.hydrated_render_ms`
- `startup.interactive_ms`
- `turn_start.latency_ms`
- `turn_start.first_event_ms`
- `first_render.ms`
- `panel.open_ms`
- `composer.keystroke_echo_ms`
- `sse.event_to_ui_ms`
- `transcript.scroll_dropped_frames_pct`
- `app_server.spawn_ready_ms`
- `cache.hit`

The thread-switch path is the first live lag assertion target: selecting a
Codex thread records the real switch sample locally and publishes it to the
host, where `qaMetrics()` can observe it.

## Verification

Pinned issue verify:

```bash
bun run --cwd clients/khala-code-desktop verify
```

Focused bridge coverage:

```bash
bun test clients/khala-code-desktop/tests/preview-bridge.test.ts clients/khala-code-desktop/tests/rpc-schema.test.ts clients/khala-code-desktop/tests/app-shell.test.ts
```

The preview bridge fixture posts a thread-switch sample through
`qaMetricSample`, then proves it is returned by `qaMetrics`. The app-shell
test keeps the renderer publish path, host recorder, preview policy, and RPC
schema wired together.
