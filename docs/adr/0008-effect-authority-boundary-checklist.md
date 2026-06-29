---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, INVARIANTS.md, apps/openagents.com/AGENTS.md, apps/openagents.com/INVARIANTS.md, docs/audits/2026-06-28-effect-usage-audit.md, docs/audits/2026-06-29-effect-usage-audit.md
informed: OpenAgents contributors and agents
---

# Effect authority-boundary checklist

## Context and Problem Statement

OpenAgents already has strong Effect examples, but authority paths still rely
on review discipline to keep platform and async code out of payment,
settlement, assignment, proof, product-promise, auth, routing, and executor
decisions. The Effect usage audits found recurring raw `JSON.parse`, direct
`process.env` or `Bun.env`, bare `catch {}`, raw `fetch`, and incidental
`Effect.runPromise` bridges.

## Decision Drivers

* Keep authority decisions inside typed Effect programs.
* Decode external data with Effect Schema before domain logic consumes it.
* Make expected failures visible as domain errors instead of defects, strings,
  swallowed exceptions, or untyped Promise rejections.
* Permit raw platform APIs only at named edges with a written reason.

## Decision Outcome

Authority-bearing code must follow this checklist when it is new or touched:

* Return `Effect<Success, DomainError, R>` from code that decides payment,
  settlement, assignment lifecycle, public proof, product-promise state, auth,
  routing, or executor closeout state.
* Use `Context.Service` and `Layer` for platform clients, config, clock, random,
  storage, queues, and provider SDKs when those dependencies affect authority.
* Decode request bodies, D1/SQLite JSON columns, local state files, WebSocket
  frames, and provider payloads with Effect Schema or a named JSON boundary
  helper before converting to domain records.
* Model expected failures with tagged domain errors. Fail-soft code may return
  public-safe diagnostics, but it should not use bare `catch {}` to erase the
  reason before a private diagnostic or typed error exists.
* Wrap external calls in typed clients that own timeout, retry, redaction, and
  error mapping. Raw `fetch` is allowed only at thin entry adapters or scripts
  that immediately map responses into a typed boundary.
* Read `process.env`, `Bun.env`, and Cloudflare bindings only at config or
  platform edges, then pass values through services/layers.
* Use `Effect.runPromise` only at process, Worker, script, or test entry edges.
  Internal orchestration should compose Effects instead of bridging out and
  back in.

Raw async/platform code is allowed when all of the following are true:

* The file is a named entry edge, platform adapter, or temporary migration edge.
* The raw call does not decide an authority outcome before typed decoding and
  error mapping.
* The edge is covered by an allowlist reason or an issue-linked migration note.
* The public result cannot expose secrets, raw prompts, wallet material,
  private repo data, provider payloads, or raw shell output.

Raw async/platform code is not allowed inside the domain step that grants,
settles, routes, accepts, promotes, pays, publishes, or closes out authority.

## Local Examples to Prefer

* Worker runtime layering: `apps/openagents.com/workers/api/src/runtime.ts`
  composes request, execution context, Worker env, queue, sync notification,
  and outbox services through Effect layers.
* Provider-account tagged errors:
  `apps/openagents.com/workers/api/src/provider-account-errors.ts` models
  expected provider-account failures as serializable tagged errors instead of
  string classifiers.
* OpenRouter retry/config/error discipline:
  `packages/probe/packages/runtime/src/llm/openrouter.ts` uses a service,
  redacted config, typed auth/rate-limit/upstream/timeout errors, timeout, and
  retry around the provider boundary.
* World contracts: `packages/world-contract/src/index.ts` keeps public-safe
  world refs, commands, deltas, cursors, and projection rows schema-first.
* ATIF redaction shape: `packages/atif/src/redaction.ts` exposes trace
  redaction as an Effect-returning service so owner-only raw material is
  scrubbed before public-safe summaries.

## Report-Only Guard

`bun run --cwd apps/openagents.com check:effect-authority-boundaries` runs a
report-only scanner across declared authority directories:

* `apps/openagents.com/workers/api/src`
* `apps/openagents.com/workers/api/scripts`
* `apps/pylon/src`
* shared authority packages including ATIF, world contracts/client,
  provider-account schema, Probe runtime, NIP-90, and agent runtime schemas

The scanner prints actionable file/line inventory for raw JSON parsing, direct
env reads, bare catches, raw fetches, and `Effect.runPromise` bridges. Existing
findings are migration inventory, not failures. The scanner exits 0 until a
future ADR or invariant promotes specific categories from report-only to a
budgeted failing guard.

## Consequences

* Good, because follow-up issues can migrate one module at a time using concrete
  file/line output.
* Good, because raw edges remain possible where the repo intentionally needs a
  process, Worker, operator-script, or platform adapter boundary.
* Bad, because the first scan will be noisy until older authority paths move
  behind typed services and schema decoders.

## Confirmation

Run:

```sh
bun run --cwd apps/openagents.com check:effect-authority-boundaries
bun run --cwd apps/openagents.com check:architecture
```

Also confirm no GitHub-hosted CI workflow is added:

```sh
bun run --cwd apps/openagents.com check:no-github-actions
```

## More Information

* `docs/audits/2026-06-28-effect-usage-audit.md`
* `docs/audits/2026-06-29-effect-usage-audit.md`
* `docs/adr/0002-adopt-effect-as-the-core-runtime-model.md`
* `INVARIANTS.md` ("Effect Workspace Boundary")
* `apps/openagents.com/INVARIANTS.md`
