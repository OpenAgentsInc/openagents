# Callbacks (Internal Observability)

Callbacks are an **internal observability** mechanism for module and model execution.

They are distinct from replay logs:
- Callbacks: in-process events (Layer A).
- REPLAY.jsonl: session recording (Layer B) and exportable publication format (Layer C).

See:
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0017-telemetry-trace-contract.md` (Layer A/B/C separation)
- `docs/execution/REPLAY.md` (replay event format)

## Requirements (Normative)

- Callback failures MUST NOT crash execution.
- Callbacks SHOULD be non-blocking (use queues/channels if needed).
- Callback events MAY include full, unredacted data (Layer A).

## Implementation Pointers

In the active TypeScript/Effect stack, callback-like observability typically maps to:
- Effect `Logger` / structured logs
- tracing spans (OpenTelemetry-style)
- in-memory event streams used by the UI/HUD

If a Rust-era doc refers to `crates/dsrs/src/callbacks.rs`, treat it as historical; the *contract* in ADR-0017 still applies.

