# Autopilot Trace Retrieval (Khala + Worker Logs)

> Legacy scope: this trace retrieval contract is for the former `apps/web` + Khala stack.
> For current web/runtime trace workflows, see `apps/openagents.com/README.md` and runtime docs in `apps/openagents-runtime/docs/`.

This document describes what trace data is persisted for a thread, what is **not** persisted in Khala, and how an agent can fetch traces programmatically.

## TL;DR

- Use `api.autopilot.traces.getThreadTraceBundle` to fetch a thread trace in one call.
- For headless end-to-end run + trace retrieval with a fixed test user, use `docs/autopilot/admin/AUTOPILOT_ADMIN_TEST_USER_TRIGGER.md`.
- Khala stores canonical chat state (`messages`, `messageParts`, `runs`) and related artifacts (`receipts`, `feature requests`, `blueprint`).
- Some runtime failures are still only in Worker telemetry logs (not Khala rows).

## What Is Persisted In Khala

For a thread, the persisted trace sources are:

- `messages`: canonical chat messages (`user` / `assistant`) and status.
- `messageParts`: streamed wire parts (`text-delta`, `finish`, `error`, `dse.signature`, `dse.tool`, etc).
- `runs`: run lifecycle state (`streaming` / `final` / `error` / `canceled`) + cancel flag.
- `blueprints`: current Blueprint JSON state for the thread.
- `receipts`: recorded receipts (`model` / `tool` / `dse.predict`) when produced.
- `autopilotFeatureRequests`: normalized capability/upgrade requests detected post-bootstrap.
- Optional per-run DSE state (if requested):
  - `dseBlobs`
  - `dseVarSpace`

## What Is NOT Fully Persisted In Khala (Current Gaps)

These are currently telemetry/log-only unless they also produce a persisted `messagePart`/row:

- Worker telemetry events (`run.started`, `run.finished`, warning/error logs) emitted via `TelemetryService`.
- Failures before run creation (for example `create_run_failed`), since no run/message exists yet.
- Internal warnings where code logs and continues (for example transient append/retry warnings) without emitting a persisted part.
- Raw provider request/response payloads (OpenRouter / Workers AI HTTP details) are not stored in Khala.
- Reasoning wire parts are intentionally filtered (`reasoning-*` parts are ignored in stream persistence).

For production correlation of these gaps, use request-id based Worker/Khala logs (see `docs/autopilot/testing/PROD_E2E_TESTING.md`).

## One-Call Thread Trace API

Public Khala query:

- Function: `autopilot/traces:getThreadTraceBundle`
- Source: `apps/web/khala/autopilot/traces.ts`

Arguments:

- `threadId` (required)
- `maxMessages` (optional, default `400`)
- `maxParts` (optional, default `8000`)
- `maxRuns` (optional, default `200`)
- `maxReceipts` (optional, default `2000`)
- `maxFeatureRequests` (optional, default `500`)
- `includeDseState` (optional, default `false`)
- `maxDseRowsPerRun` (optional, default `200`)

Response includes:

- `thread`, `blueprint`
- `messages`, `parts`, `runs`, `receipts`, `featureRequests`
- `dseBlobs`, `dseVars` (only when `includeDseState=true`)
- `summary` counts for fast sanity checks

## Programmatic Retrieval Examples

### 1) TypeScript (Khala client)

```ts
import { KhalaHttpClient } from "khala/browser";
import { api } from "../khala/_generated/api";

const client = new KhalaHttpClient(process.env.VITE_KHALA_URL!);
client.setAuth(process.env.KHALA_AUTH_TOKEN!); // owner auth required

const bundle = await client.query(api.autopilot.traces.getThreadTraceBundle, {
  threadId: "thread_123",
  includeDseState: true,
});

console.log(bundle.summary);
```

### 2) CLI (Khala run)

```bash
cd apps/web
npx khala run autopilot/traces:getThreadTraceBundle \
  '{"threadId":"thread_123","includeDseState":true}'
```

## Suggested Retrieval Flow For Agents

Given a `threadId`:

1. Query `getThreadTraceBundle` with bounded limits.
2. Inspect `summary` first (quick completeness check).
3. Group `parts` by `runId`, then sort by `seq` for deterministic replay.
4. Join `runs` with `messages` (`assistantMessageId` / `runId`) to get terminal status + final text.
5. If investigating DSE execution internals, set `includeDseState=true`.
6. If a gap remains (for example missing pre-run failure), pivot to Worker logs by `x-oa-request-id`.
