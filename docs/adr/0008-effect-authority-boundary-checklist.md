# ADR 0008: Effect Authority-Boundary Checklist

## Status

Accepted.

## Context

OpenAgents authority paths decide payment, settlement, assignment, public proof,
product-promise, auth, and routing state. Those paths need typed failure,
validated inputs, and explicit runtime dependencies instead of incidental
platform calls hidden inside business logic.

The Effect usage audits from 2026-06-28 and 2026-06-29 found good local
examples, but the repository still depends on review discipline to prevent new
raw `JSON.parse`, direct environment reads, bare `catch {}`, raw `fetch`, and
incidental `Effect.runPromise` bridges from entering authority code.

## Decision

Authority-bearing TypeScript should default to functions that return
`Effect<Success, DomainError, R>` or pure typed values. Raw async/platform code is
allowed only at named entry edges and adapters, where it immediately converts
platform data into schema-validated values, typed service dependencies, or
domain errors.

Use this checklist when changing authority paths:

- Runtime entrypoints may call `Effect.runPromise`, read Worker bindings, or
  adapt HTTP/platform requests. Deeper modules should return `Effect` values and
  leave execution to the entrypoint.
- External JSON must be decoded with Effect Schema or a local typed boundary
  helper before it affects auth, routing, payment, settlement, proof, or public
  claims. `JSON.parse(...) as Type` is migration inventory unless it is isolated
  behind an explicitly documented compatibility boundary.
- Environment and binding reads belong in runtime layering, config loaders, or
  deployment scripts. Authority logic should receive configuration through typed
  services or arguments.
- Network calls should go through provider/domain clients that model retry,
  timeout, response decoding, and tagged errors. A raw `fetch` in authority code
  must be an adapter edge with an allowlist reason.
- Empty `catch {}` blocks are not acceptable in authority logic. Expected
  failures should become typed errors, diagnostic refs, or deliberately ignored
  cleanup errors with a comment and allowlist entry.
- Public projection and product-promise code must preserve public-safe data
  boundaries: private prompts, raw provider payloads, wallet material, local
  paths, and private repo content stay out of public traces and counters.

## Local Examples To Follow

- Worker runtime layering in `apps/openagents.com/workers/api` keeps request
  adapters and deployment composition near the Worker edge.
- Provider-account code uses tagged errors and typed account state instead of
  throwing unstructured platform exceptions.
- OpenRouter integration code demonstrates retry/config/error discipline around
  provider calls rather than scattering raw fetches through call sites.
- `packages/world-contract` centralizes public-safe world command, row, delta,
  and cursor schemas with Effect Schema.
- ATIF redaction service code models scrubbed trace shape before trace data can
  become public-safe evidence.

## Guardrail

Run the report-only scanner:

```sh
bun run scan:effect-authority-boundaries
```

The scanner inventories suspicious raw boundary operations in declared authority
directories. It does not fail the build yet; existing findings are migration
inventory for follow-up issues. Intentional raw edges must be listed in
`scripts/effect-authority-boundary-allowlist.ts` with a reason explaining the
boundary and why a typed Effect adapter is not the right shape there yet.

## Consequences

This ADR does not require one large migration. It creates a common checklist and
an actionable local report so future work can migrate module-by-module while
keeping new authority code inside the intended Effect model.
