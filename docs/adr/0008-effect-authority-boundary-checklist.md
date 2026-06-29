# ADR-0008: Effect Authority Boundary Checklist

Date: 2026-06-29

Status: Accepted

## Context

The Effect usage audits from 2026-06-28 and 2026-06-29 found strong local
examples, but authority paths still rely on review discipline to avoid fresh raw
platform code. This checklist applies when code decides payment, settlement,
assignment lifecycle, public proof, product-promise, auth, routing, or owner
local executor state.

## Decision

Authority code should return `Effect<Success, DomainError, R>` once it is past a
thin platform edge. Raw async/platform code is allowed only at entry adapters
that immediately map untyped inputs into Effect services, Schema decoders, or
tagged domain errors.

Use this checklist for new or touched authority code:

- Decode external JSON, D1 rows, WebSocket frames, CLI input, and local state
  files from `unknown` with Effect Schema or a named boundary helper before
  domain logic uses the value.
- Read config and secrets through a config service or redacted `Config` layer.
  Direct `process.env`, `Bun.env`, and Cloudflare `Env` reads belong at
  bootstrap or Worker binding edges only.
- Model expected failures with `Schema.TaggedErrorClass` or equivalent tagged
  domain errors. A fail-soft projection can return a public-safe fallback, but
  the swallowed cause must first become an observed private diagnostic.
- Wrap external HTTP, provider SDKs, storage calls, subprocesses, and Durable
  Object calls in service methods that return `Effect`, with timeout/retry where
  the domain requires it.
- Use `Effect.runPromise` only at CLI, Worker, test, or explicit adapter edges.
  Do not bridge out in the middle of assignment, payment, proof, product-promise,
  auth, or routing decisions.
- Manage long-lived resources with `Scope`, `acquireRelease`, scoped fibers, or
  explicit service lifetimes instead of open-ended Promises and manual cleanup.

Promoted examples from the audits:

- Worker runtime layering: `apps/openagents.com/workers/api/src/runtime.ts`
  composes request, environment, execution context, queue, and notification
  services through layers.
- Provider-account tagged errors:
  `apps/openagents.com/workers/api/src/provider-account-errors.ts` keeps
  account failures in serializable typed variants.
- OpenRouter retry/config/error discipline:
  `packages/probe/packages/runtime/src/llm/openrouter.ts` keeps credentials
  redacted, provider failures tagged, and network calls bounded by timeout and
  retry.
- World contract schemas: `packages/world-contract/src/index.ts` uses branded
  refs and Schema classes for shared public world rows and commands.
- ATIF redaction service shape: `packages/atif/src/redaction.ts` exposes
  Effect-returning redaction methods behind a service contract.

## Guardrail

Run the report-only inventory with:

```sh
bun run check:effect-authority-boundaries
```

The scanner covers declared authority-bearing directories, including
`apps/openagents.com/workers/api`, `apps/pylon/src`, and shared packages. It
reports raw `JSON.parse`, direct `process.env` / `Bun.env`, bare `catch {}`,
raw `fetch`, and suspicious `Effect.runPromise` bridges with file and line
output. Existing findings are migration inventory, not deploy blockers.

Intentional raw edges must be recorded in
`scripts/effect-authority-boundary-allowlist.json` with a rationale explaining
why the edge is raw and what typed Effect or Schema boundary contains it.
