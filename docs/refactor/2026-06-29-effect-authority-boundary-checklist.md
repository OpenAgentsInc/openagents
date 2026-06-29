# Effect Authority Boundary Checklist

Date: 2026-06-29

Issue: #7009

This checklist applies to code that decides or mutates authority state:
payment, settlement, assignment lifecycle, public proof, product promises,
auth, routing, provider selection, token accounting, trace ingest, and
owner-local executor state.

## Boundary Rule

Raw async or platform code is allowed only at a thin edge whose job is to call
the platform and immediately map the result into a typed boundary:

- CLI, Worker, test, and script entrypoints may bridge a final program with
  `Effect.runPromise`.
- Platform adapters may call `fetch`, filesystem APIs, process APIs, D1,
  Durable Objects, or environment variables only when they translate results
  into an Effect service, Effect Schema decoder, typed config service, or
  tagged domain error.
- Domain code that decides authority state should return
  `Effect<Success, DomainError, R>` and should not hide expected failures in
  `Promise`, `throw`, broad `catch {}`, unchecked JSON casts, or raw env reads.
- Fail-soft paths are allowed, but they must preserve a typed, redacted
  diagnostic before returning a public-safe fallback.

## Review Checklist

- Service boundary: authority orchestration is exposed as an Effect service or
  Effect-returning function; dependencies are supplied through `Context` and
  `Layer` where the surrounding code already uses that pattern.
- Error boundary: expected failures use tagged domain errors, not untyped
  `throw`, string refs alone, or swallowed exceptions.
- Data boundary: external JSON, D1 rows, local state files, WebSocket frames,
  queue payloads, and request bodies decode from `unknown` with Effect Schema
  or a named typed boundary helper before domain logic sees them.
- Config boundary: secrets and flags come from a config service or typed
  adapter. Direct `process.env` and `Bun.env` reads stay at entry/config edges.
- HTTP/provider boundary: raw `fetch` is wrapped with timeout, retry policy
  where appropriate, status mapping, and redacted error payloads.
- Run boundary: `Effect.runPromise` appears at CLI, Worker, script, or test
  edges only. Inner modules return Effects and let callers provide layers.
- Resource boundary: subprocesses, WebSockets, leases, locks, and long-lived
  streams have explicit cleanup through `Scope`, `acquireRelease`, or an
  equivalent local wrapper.
- Public-safety boundary: public proofs and counters are derived from exact,
  typed rows or receipts, never from raw provider payloads, raw event chunks,
  private prompts, local paths, or secrets.

## Local Reference Examples

- Worker runtime layering:
  `apps/openagents.com/workers/api/src/runtime.ts` composes request, Worker env,
  queues, execution context, and outbox dependencies through services and
  layers.
- Provider-account tagged errors:
  `apps/openagents.com/workers/api/src/provider-account-errors.ts` models
  expected account failures as serializable tagged errors.
- OpenRouter retry/config/error discipline:
  `packages/probe/packages/runtime/src/llm/openrouter.ts` reads redacted config,
  wraps provider calls in typed errors, and applies timeout/retry policy.
- World contracts:
  `packages/world-contract/src/index.ts` keeps shared public-safe rows,
  commands, cursors, and refs schema-first.
- ATIF redaction service shape:
  `packages/atif/src/redaction.ts` exposes redaction as an Effect service so
  trace cleanup is injectable and testable.

## Report-Only Guard

Run the current migration inventory scan with:

```sh
bun run scan:effect-authority-boundaries
```

The scanner covers declared authority roots including
`apps/openagents.com/workers/api/src`, `apps/pylon/src`,
`apps/openagents-world/src`, and shared authority-bearing packages. It reports
raw `JSON.parse` near casts or manual narrowing, direct `process.env` or
`Bun.env`, bare `catch {}`, raw `fetch`, and suspicious `Effect.runPromise`
bridges away from declared edges.

The scanner is intentionally report-only. Existing matches are migration
inventory for follow-up issues, not immediate failures. Intentional raw edges
belong in `scripts/effect-authority-boundary-allowlist.mjs` with a narrow match
and a reason explaining why the edge is allowed to remain raw.
