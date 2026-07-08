# Effect Usage Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-28

Issue: #6970

Scope: `apps/openagents.com`, `apps/pylon`, and `packages/*`.

This audit compares the current repository against the root Effect workspace
invariant, ADR-0002, and the local `effect-solutions` guidance for
`services-and-layers`, `error-handling`, `data-modeling`, `testing`, `config`,
and `basics`.

## Executive Summary

OpenAgents has a real Effect foundation, not a superficial import pattern. The
strongest code uses `Effect`, `Schema`, `Context.Service`, `Layer`,
`Stream`, `Schedule`, and typed errors in the places where the repository has
recently invested: Probe runtime contracts, selected Pylon runtimes, browser
subscription lifecycles, world/client contracts, and some
`apps/openagents.com` Worker modules.

The gap is consistency. Older and fast-moving authority paths still use raw
`async` functions, `Promise`, direct env reads, unchecked `JSON.parse`, D1 row
casts, `catch {}` fallbacks, and hand-injected dependency objects. That does not
mean those paths are broken today, but it does mean the repo has not yet fully
realized ADR-0002's "everything effectful" direction. The most important
improvement is not more Effect syntax. It is standardizing service boundaries,
schema decoding, config loading, and tests so the effect model is visible at
the same places where authority, payment, routing, public claims, and local
executor state are decided.

## Reference Rules Used

- Root `INVARIANTS.md` requires new production TypeScript to use Bun and Effect,
  and requires external boundaries to be modeled with typed structures or
  Effect Schema.
- `docs/adr/0002-adopt-effect-as-the-core-runtime-model.md` chooses Effect,
  Effect Schema, and Foldkit on Bun/Cloudflare Workers, with explicit services,
  errors, commands, and schemas.
- `effect-solutions services-and-layers` recommends `Context.Service` contracts,
  `Layer` implementations, service methods returning `Effect`, and providing
  layers at entry points.
- `effect-solutions error-handling` recommends `Schema.TaggedErrorClass`, typed
  error channels, `catchTag`/patterned recovery, and reserved use of defects.
- `effect-solutions data-modeling` recommends schema-backed records, variants,
  brands, JSON serialization, and fail-closed boundary decoding.
- `effect-solutions config` recommends `Config`, `Config.redacted`, schema
  validation, config services, and test layers instead of raw env plumbing.
- `effect-solutions testing` recommends Effect-aware tests, per-test layers, and
  deterministic clock/random/service injection.
- `effect-solutions basics` recommends `Effect.gen`, named `Effect.fn`, retry,
  timeout, logging, and spans at effect boundaries.

## Current Strengths

1. Some service boundaries are excellent. `packages/probe/packages/runtime/src/llm/openrouter.ts:166`
   defines `OpenRouterClient` as a `Context.Service`, returns typed `Effect`
   methods, reads credentials through `Config.redacted`, maps missing config to
   a tagged auth error, applies timeout, and retries with `Schedule` at
   `packages/probe/packages/runtime/src/llm/openrouter.ts:386`.

2. Error modeling is already idiomatic in important modules. Examples include
   `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts:53`,
   `packages/probe/packages/runtime/src/blueprint/action-submission.ts:61`,
   `packages/provider-account-schema/src/runtime.ts:79`, and
   `apps/openagents.com/workers/api/src/first-batch-payment-policies.ts:44`.

3. Effect Schema is the default in many shared contracts. `packages/world-contract`,
   `packages/world-client`, `apps/openagents.com/packages/sync-schema`, and
   `packages/probe/packages/runtime/src/blueprint/contracts.ts` expose branded
   and structured schema types that can be reused across clients, Workers, and
   tests.

4. Browser lifecycles have good Effect resource handling. `apps/openagents.com/apps/web/src/subscriptions.ts:366`
   and `apps/openagents.com/apps/web/src/subscriptions.ts:963` use
   `Stream.callback` plus `Effect.acquireRelease` to bind WebSocket/SSE lifetime
   to Foldkit subscription lifetime.

5. Recent Pylon runtimes are moving toward Effect service injection. `apps/pylon/src/openagents-native-runtime.ts:52`
   and `apps/pylon/src/openagents-native-runtime.ts:59` model the language model
   and toolkit as `Context.Service`s, with test layers at
   `apps/pylon/src/openagents-native-runtime.ts:106`.

## Audit Findings By Dimension

### 1. Services And Layers

Current state:

- Strong examples exist in `OpenRouterClient`,
  `OpenAgentsNativeLanguageModel`, `OpenAgentsNativeToolkit`, ATIF redaction,
  and UI icon service layers.
- Many large Pylon and Worker flows still pass dependency records or raw
  functions instead of services. For example,
  `apps/openagents.com/workers/api/src/first-batch-payment-policies.ts:33`
  takes a runtime object with `makePolicyId` and `nowIso`; this is testable, but
  not composable as a layer. `apps/pylon/src/bootstrap.ts:124` defaults to
  `process.env` directly, again testable by parameter but outside a config/env
  service.
- Local executor and workspace code still contains long `async` orchestration
  functions, for example `apps/pylon/src/workspace-materializer.ts:902`, with
  lock acquisition, polling, Git execution, and cleanup modeled manually.

Best-practice gap:

Service methods should return `Effect` and hide their own dependencies behind
layers. Dependency records are acceptable as transitional seams, but they do not
compose as well as `Layer`s, and they make it harder to provide once at the
entry point.

Recommendations:

- High: Define first-class services for Pylon workspace materialization,
  assignment execution, local state, and account registry operations. Start with
  service interfaces and test layers before moving implementation.
- High: In `apps/openagents.com`, introduce Worker-side service tags for D1,
  request auth/session, public projection reads, token accounting, and trace
  ingest. Provide Cloudflare bindings once in the Worker entry layer.
- Medium: Replace ad hoc runtime dependency records with services as modules are
  touched. Keep pure helper functions pure; only effectful boundaries need
  services.
- Low: Normalize service tag names to a unique path-like convention, following
  the OpenRouter example more consistently.

### 2. Error Handling

Current state:

- Typed errors are common in newer Effect modules:
  `apps/openagents.com/workers/api/src/first-batch-payment-policies.ts:44`,
  `packages/probe/packages/runtime/src/llm/openrouter.ts:77`, and multiple
  Probe blueprint modules.
- Some legacy helpers still swallow parse or storage failures. `apps/openagents.com/workers/api/src/json-boundary.ts:92`
  returns `undefined` on malformed JSON, and
  `apps/pylon/src/workspace-materializer.ts:987` returns `null` for an invalid
  lease record. In some read-side helpers this is fine; in authority paths it
  hides whether data was absent, malformed, or unreadable.
- `apps/pylon/src/workspace-materializer.ts:811` uses a bare `catch {}` while
  reading lock-owner JSON and treats all failures as "not live." This is the
  class of pattern issue #6970 explicitly asked to flag.

Best-practice gap:

Expected failures should stay in the typed error channel. Defects and lossy
fallbacks should be reserved for truly optional, best-effort projections.

Recommendations:

- High: Ban bare `catch {}` in authority paths. Replace with typed
  `Schema.TaggedErrorClass` values carrying operation and public-safe reason
  refs.
- High: Split "not found", "malformed", and "storage failed" in local Pylon
  state, lease records, D1 reads, and public proof closeout checks.
- Medium: Wrap low-level `unknown` exceptions in module-specific tagged errors
  with `S.Defect`, as `FirstBatchPaymentPolicyStorageError` already does at
  `apps/openagents.com/workers/api/src/first-batch-payment-policies.ts:44`.
- Low: Prefer `Effect.catchTag` or `Match` when callers need different
  recovery behavior per error tag.

### 3. Effect Schema And Data Modeling

Current state:

- Shared packages use Effect Schema well. `apps/openagents.com/workers/api/src/json-boundary.ts:75`
  has a small schema-backed JSON boundary helper, and many contract packages use
  branded IDs and schema classes.
- D1 and SQLite row reads often cast directly after query execution. Examples:
  `apps/pylon/src/spark-bun-storage.ts:631`,
  `apps/pylon/src/spark-bun-storage.ts:892`, and the mirrored
  `apps/openagents.com/services/mdk-treasury/src/spark-bun-storage.ts:631`.
- Pylon local state reads use generic casts after `JSON.parse`, for example
  `apps/pylon/src/state.ts:180`.
- Some command/public safety checks are regex and shape checks rather than
  schemas. `apps/pylon/src/khala-requester.ts:320` validates verification args
  with regexes and then emits a public command ref. That is bounded and useful,
  but it should become a named schema contract because this is a public
  delegation boundary.

Best-practice gap:

All external boundaries should decode from `unknown` with schema before domain
logic sees values. Raw JSON plus casts are the highest recurring schema debt.

Recommendations:

- High: Add schema decoders for Pylon local state files, active assignment run
  files, workspace leases, and Khala git checkout workspace payloads.
- High: Add row schemas for authority-bearing D1/SQLite rows before converting
  snake_case storage shapes to domain records.
- Medium: Promote existing regex/public-safety validators into named schemas
  with tests, especially for verification commands and public refs.
- Low: Prefer branded schema IDs for recurring refs (`assignmentRef`,
  `workspaceRef`, `pylonRef`, `tokenUsageEventId`) to avoid string mixups.

### 4. Effectfulness

Current state:

- Newer modules intentionally return `Effect` values: `apps/openagents.com/workers/api/src/first-batch-payment-policies.ts:257`,
  `packages/world-client/src/index.ts`, Probe blueprint validators, and
  OpenRouter client.
- Many authority paths still bridge out through `Effect.runPromise` or plain
  `async` before orchestration is complete. `apps/pylon/src/openagents-native-runtime.ts:176`
  runs an Effect inside `Effect.promise`, which loses interruption and typed
  error composition across that sub-boundary. `apps/pylon/src/workspace-materializer.ts:874`
  models acquisition/release with `try/finally` rather than `Effect.acquireRelease`.
- Direct `fetch` and env reads are still common in CLI and Worker code. Some are
  entry edges, but many are internal enough to deserve services.

Best-practice gap:

Effect boundaries should be pushed inward until the program's orchestration is
one composable Effect, with `runPromise` only at CLI, Worker, or test edges.

Recommendations:

- High: For Pylon assignment execution, workspace checkout, and Codex/Claude
  runners, move the top-level orchestration from `async` functions into
  `Effect.gen` and expose one Promise bridge at the CLI command edge.
- High: Convert Worker authority route handlers to return `Effect<Response, E,
  R>` internally, then run once at the Worker boundary.
- Medium: Treat direct `fetch` as an `HttpClient` or local service dependency in
  code that needs retries, typed errors, or test injection.
- Low: Name important effectful functions with `Effect.fn` for trace spans and
  call-site visibility.

### 5. Concurrency And Resources

Current state:

- Good resource modeling exists in browser subscriptions via
  `Effect.acquireRelease`, and Pylon node runtime uses scoped fibers in
  `apps/pylon/src/index.ts:1076` and related node modules.
- `OpenRouterClientLive` uses timeout and bounded retry with `Schedule` at
  `packages/probe/packages/runtime/src/llm/openrouter.ts:386`.
- Workspace cache locking is hand-rolled with `setInterval`, `sleep`, polling,
  and `try/finally` in `apps/pylon/src/workspace-materializer.ts:828`.
- Repository-level work often uses plain loops and `Promise` sequencing where
  `Effect.forEach` with explicit concurrency would make intent auditable.

Best-practice gap:

Resources that need cleanup should be acquired in `Scope`. Concurrency should
declare limits, interruption behavior, and retry/backoff policies explicitly.

Recommendations:

- High: Rewrite Pylon workspace locks and active-run registration as scoped
  resources, using `Effect.acquireRelease` for lock directories, heartbeats, and
  temporary worktrees.
- Medium: Use `Effect.forEach(..., { concurrency })` in batch/projection
  rebuilds and assignment fanout code so concurrency is not hidden in ad hoc
  promises.
- Medium: Centralize retry schedules for external APIs, Git operations, D1
  transient failures, and public projection sync rather than duplicating
  timeouts.
- Low: Add interruption tests for scoped resources that currently depend on
  `finally`.

### 6. Config

Current state:

- `packages/probe/packages/runtime/src/llm/openrouter.ts:200` is the best
  example: `Config.redacted`, defaults, safe caps, fail-closed missing key, and
  a live layer separate from mocks.
- Pylon bootstrap and runtime code commonly defaults function parameters to
  `process.env` or `Bun.env`; see `apps/pylon/src/bootstrap.ts:124`,
  `apps/pylon/src/bootstrap.ts:210`, and `apps/pylon/src/dev-loop.ts:411`.
- Several CLI paths pass env maps manually. This keeps tests possible but leaves
  config validation spread across modules.

Best-practice gap:

Config should be loaded as an Effect service/layer with redacted secrets and
schema validation, not read directly by business logic.

Recommendations:

- High: Create `PylonConfig`, `PylonPathsConfig`, and `OpenAgentsWorkerConfig`
  services using `Config` and schema validation. Keep existing pure helpers, but
  move env access into layers.
- High: Use `Config.redacted` for provider keys, tokens, wallet-adjacent
  settings, and API credentials everywhere they enter Effect code.
- Medium: Add test layers for common Pylon and Worker config profiles instead of
  passing raw env records through deep call chains.
- Low: Document allowed raw-env exceptions: CLI entry parsing, test harnesses,
  and compatibility wrappers.

### 7. Testing

Current state:

- The repo has broad deterministic test coverage and `check:deploy` is a real
  gate.
- Many tests run effects through `Effect.runPromise`, for example
  `packages/probe/packages/runtime/src/llm/openrouter.test.ts:39` and
  `apps/pylon/tests/control-protocol.test.ts:174`. This is fine at the edge, but
  not as expressive as `@effect/vitest` for effect services.
- Test layers exist in some modules, such as `OpenAgentsNativeTestLanguageModelLayer`
  and `OpenRouterClientMock`. These are good models.
- There is little visible use of `TestClock`, `TestRandom`, suite/per-test
  `Layer` patterns, or interruption assertions for resource-heavy code.

Best-practice gap:

Effect-aware tests should assert typed errors, layer substitution, time, retry,
and interruption directly.

Recommendations:

- High: Introduce `@effect/vitest` for new Effect services and migrate critical
  service tests as those modules are touched.
- Medium: Add TestClock-based tests for retry/backoff, stale lock expiry, stream
  reconnects, and lease expiry.
- Medium: Use per-test layers for stateful services by default; use suite layers
  only for expensive shared resources.
- Low: Add `Effect.runPromiseExit` assertions for expected typed failures rather
  than relying on thrown exceptions.

### 8. Consistency

Current state:

- The repo has multiple Effect styles: modern v4 `Context.Service` and `Layer`,
  pure helper plus `Effect` wrapper modules, Foldkit `Command`/`Subscription`
  effects, raw `async` CLI code, and duplicated imported runtime code under
  Pylon/Probe.
- `apps/openagents.com` and `packages/probe` have the clearest pattern library.
  Pylon has the widest mix because it combines local CLI, filesystem state,
  Nostr identity, wallet-adjacent code, local executor orchestration, and new
  Effect services.

Best-practice gap:

ADR-0002 is correct, but contributors need a small set of repository-native
patterns that show exactly when to choose pure helpers, schema classes, service
tags, config layers, and Promise bridges.

Recommendations:

- High: Add a short "Effect implementation patterns" doc under `docs/adr/` or
  `docs/refactor/` with approved examples from OpenRouter, subscriptions,
  FirstBatchPaymentPolicy, and Probe blueprint contracts.
- High: Add an architecture check that flags new raw `JSON.parse` plus `as T`,
  bare `catch {}`, and `process.env`/`Bun.env` outside allowlisted entry files.
- Medium: Track a module-by-module migration list for Pylon executor/workspace
  code and Worker authority routes.
- Low: Normalize imports and naming (`Schema as S`, `Context.Service`,
  `Layer.*`) across new modules.

## Prioritized Top 10 Improvements

1. Create schema decoders for Pylon local state, active assignment runs,
   workspace leases, and Khala workspace payloads.
2. Replace bare `catch {}` and lossy parse fallbacks in authority paths with
   typed tagged errors.
3. Introduce Pylon workspace and assignment-executor services with live/test
   layers.
4. Move Pylon workspace lock acquisition, heartbeat, and cleanup into
   `Effect.acquireRelease` under `Scope`.
5. Add Worker-side D1/auth/token/trace services so route internals compose as
   `Effect<Response, E, R>`.
6. Add row schemas for D1/SQLite authority tables before domain conversion.
7. Centralize Pylon and Worker config as `Config` services with redacted secrets
   and test layers.
8. Adopt `@effect/vitest` for new Effect services and add TestClock coverage for
   retries, stale locks, lease expiry, and streams.
9. Add a repository guard for new raw `JSON.parse` casts, direct env reads, and
   bare catches outside allowlisted edges.
10. Publish a concise OpenAgents Effect pattern guide with approved examples and
    migration rules.

## Suggested Migration Order

1. Pylon workspace/materializer state: high leverage, active in Khala coding
   delegation, and currently contains the clearest manual resource patterns.
2. Pylon local state/config: many small raw JSON/env reads can be moved behind
   schemas and config layers incrementally.
3. Worker D1 authority routes: add row schemas and D1 service tags around
   payment, token, promise, and proof rows as files are touched.
4. Tests: add Effect-aware tests alongside each service migration rather than a
   separate test-only sweep.
5. Guardrails: after the first migrations establish allowlists, enforce the new
   rules in architecture checks.

## Verification Notes

This audit is documentation-only. It is grounded in live source examples from
the repository and intentionally does not modify runtime code.

Expected verification for this issue:

- `docs/audits/2026-06-28-effect-usage-audit.md` exists.
- The audit cites concrete files and paths.
- `bun run --cwd apps/openagents.com check:deploy` passes.
