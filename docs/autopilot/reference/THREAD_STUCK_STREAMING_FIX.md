# Thread Stuck in "Streaming": Root Cause + Definitive Fix (2026-02-10)

## Symptom

Some threads could get stuck showing `status: "streaming"` indefinitely:

- Khala had a `run` and assistant `message` in `streaming`.
- The UI treated the thread as busy forever (because it keys off any `message.status === "streaming"`).

This was user-visible as the chat spinner / “…” never settling.

## Root Cause

In `apps/web/src/effuse-host/autopilot.ts`, the stream pipeline used a JavaScript `try/catch/finally` *inside* `Effect.gen(...)` to guarantee finalization (`finalizeRunInKhala`) and telemetry (`run.finished`).

In this codebase’s Effect version/usage, **JS `try/catch/finally` does not reliably run for Effect failures** (e.g. `Effect.fail(...)`, fiber interruption, or failures occurring before the JS block is entered). As a result, the “finalize” logic could be skipped, leaving the run/message in `streaming`.

Additionally, several pre-stream steps ran *outside* the old “finalize” try/finally boundary (Khala snapshot load, bootstrap/DSE setup, RLM-lite recap work), meaning early failures could bypass finalization entirely.

## Fix (Worker)

Implemented an explicit “always finalize” control flow using `Effect.exit(...)` rather than JS `finally`:

- Wrap the stream program in a single `streamProgram`.
- `exit = yield* Effect.exit(streamProgram)`
- On `Exit.failure`, compute `Cause.pretty(...)`, best-effort append an error part, finalize the run as `error` (or `canceled` on abort).
- On `Exit.success`, finalize the run as `final` (or `canceled` on abort).
- Always emit `run.finished` (best-effort).

Hard timeouts were added around:

- Khala query/mutation calls (to avoid “hang forever”).
- Model streaming (to avoid never-settling streams).
- RLM-lite pre-summary (bounded so it can’t block finalization).

## Fix (Khala Guardrail)

Added a safety net for any residual cases (e.g. Worker termination, provider stalls):

- New internal mutation `finalizeStaleRuns` finalizes `streaming` runs older than a cutoff, and patches any assistant message still marked `streaming`.
- New index `runs.by_status_updatedAtMs` supports efficient stale-run scanning.
- A cron runs the sweeper every minute.

Files:

- `apps/web/khala/schema.ts` (index)
- `apps/web/khala/autopilot/messages.ts` (internal mutation)
- `apps/web/khala/crons.ts` (cron schedule)

## Verification

- Worker unit test coverage: `apps/web/tests/worker/chat-streaming-khala.test.ts` includes a regression test asserting that a Khala snapshot failure still finalizes the run (`error`) rather than leaving it stuck.
- E2E coverage: `packages/effuse-test/src/suites/apps-web.ts` includes a prod chat send test that asserts the home chat status settles (`ready` or `error`) and never silently stalls.

## Operational Notes

- Correlate by Worker request id first (`x-oa-request-id` header, `oa_req=<id>` in logs).
- Khala logs can confirm whether `finalizeRun` ran for a given `runId` / `threadId`.

