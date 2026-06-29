# Psionic Qwen3.5 Live Smoke

Date: 2026-06-10

This smoke is the Pylon-owned attach test for the optional Psionic Qwen3.5
backend. It does not install Psionic, download model weights, claim paid
capacity, or claim training support.

## Prerequisites

- A live Psionic OpenAI-compatible server reachable from this machine.
- `/health` reports `execution_engine = psionic`.
- `/health` advertises `/v1/chat/completions`.
- `/v1/models` exposes an admitted `model.psionic.qwen35.0_8b.q8_0` row,
  either through the retained 0.8B digest or a public-safe artifact manifest
  ref.

Use an explicit base URL or set `PYLON_PSIONIC_BASE_URL`. The default remains
`http://127.0.0.1:8080`.

```sh
bun run smoke:psionic-qwen -- --base-url http://127.0.0.1:8080
```

The smoke performs:

1. `doctor`: checks `/health` and `/v1/models`.
2. `plainInference`: runs a bounded 0.8B chat completion.
3. `toolRoundTrip`: requires one local `echo_public_ref` tool call and one
   model continuation.

The JSON output is intentionally public-safe: transcript contents are limited
to the bounded probe text, provider payloads are not retained, local model
paths are not emitted, and receipts carry `contentRedacted: true`.

## Blockers

- `blocker.psionic_qwen35.health_unreachable`: no server is reachable.
- `blocker.psionic_qwen35.execution_engine_not_psionic`: the server is not a
  Psionic execution engine.
- `blocker.psionic_qwen35.chat_completion_endpoint_missing`: chat completions
  are not advertised.
- `blocker.psionic_qwen35.model_0_8b_missing`: the 0.8B Qwen3.5 row is not
  admitted.
- `blocker.psionic_qwen35.chat_completion_failed`: plain completion failed.
- `blocker.psionic_qwen35.tool_call_failed`: required tool round-trip failed.

On an unattached machine the smoke should fail with one of the typed blocker
refs above, not with an unstructured runtime exception.
