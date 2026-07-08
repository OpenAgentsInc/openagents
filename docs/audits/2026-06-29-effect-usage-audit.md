# Effect usage audit and improvement recommendations

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Issue: #6970

Date: 2026-06-29

Scope: `apps/openagents.com`, `apps/pylon`, and shared `packages/*`.

Reference material used:

- `effect-solutions list`
- `effect-solutions show services-and-layers error-handling data-modeling testing config basics`
- `docs/adr/0002-adopt-effect-as-the-core-runtime-model.md`
- Root `INVARIANTS.md` and `apps/openagents.com/INVARIANTS.md`

## Executive summary

OpenAgents has adopted Effect broadly but unevenly. A repo scan found more than
1,700 TypeScript files importing `effect`, and the strongest areas are the
Cloudflare Worker runtime composition, shared contract packages, and newer
provider/runtime modules. The best examples already match the
`effect-solutions` guidance: dependency tags via `Context.Service`, environment
composition via `Layer`, serializable domain errors via `Schema.TaggedError`,
boundary models via `Schema.Class` and branded primitives, redacted config, and
bounded retry/timeout around external calls.

The gaps are mostly consistency and depth, not absence. Several authority and
executor paths still expose raw `Promise` functions, direct `process.env` or
`Bun.env` reads, `JSON.parse(... ) as T`, swallowed `catch {}`, untyped casts at
HTTP/D1/CLI boundaries, and manual concurrency/resource handling. This makes
some newer code very Effect-native while older CLI, storage, and route code is
Effect-adjacent: it imports Effect or wraps a final call, but the meaningful
failure, dependency, and config boundaries remain outside the typed Effect
model.

The highest-value next step is not a rewrite. Create a small set of repo-level
guardrails and migration targets: an Effect boundary checklist for authority
paths, a typed JSON/config helper package, a service/layer target for Pylon
network and environment dependencies, and test helpers based on `@effect/vitest`
or equivalent layer-injection patterns.

## 1. Services and Layers

Current state:

- Good Worker runtime composition exists. `apps/openagents.com/workers/api/src/runtime.ts:26`
  defines `OpenAgentsWorkerRequest` as a `Context.Service`, `runtime.ts:47`
  defines `RunnerEventsQueueLayer`, and `runtime.ts:99` composes request,
  native request, execution context, worker environment, sync notifications, and
  outbox store in `WorkerRequestLayer`.
- `apps/openagents.com/workers/api/src/config.ts:648` defines
  `OpenAgentsWorkerConfig` as a service with a `Layer.effect` constructor at
  `config.ts:652`.
- Newer Pylon runtime code follows the same shape:
  `apps/pylon/src/openagents-native-runtime.ts:52` and `:59` define
  `OpenAgentsNativeLanguageModel` and `OpenAgentsNativeToolkit` service tags,
  with test layers at `:106`, `:125`, and `:134`.
- Shared redaction also uses service/layer shape:
  `packages/atif/src/redaction.ts:45` defines `TraceRedactorShape` with
  Effect-returning methods.

Best-practice gap:

`effect-solutions` recommends service methods return `Effect`, dependencies be
modeled via service tags, and layers be provided once near the entry point.
The Worker has a clear direction here, but older domain services still mix
Effect imports with raw async APIs. For example,
`apps/openagents.com/workers/api/src/provider-account-service.ts:69` and `:82`
export async functions returning `Promise`, and they call repository methods
directly. This limits typed error channels and makes test substitution depend
on ad hoc object injection instead of layer composition.

Recommendation:

High impact. Promote authority-path services such as provider accounts, Pylon
assignments, token usage, and payment adapters to Effect service APIs first at
their orchestration boundary. Do not rewrite every helper. Start by adding
service facades whose public methods return `Effect<Success, DomainError>`,
then keep existing Promise repositories as private adapters until they can be
replaced.

## 2. Error handling

Current state:

- Strong positive examples exist. `apps/openagents.com/workers/api/src/provider-account-errors.ts:3`
  through `:145` defines provider-account error variants with
  `Schema.TaggedErrorClass` and a union schema.
- `apps/openagents.com/workers/api/src/runtime.ts:51` models
  `SyncRoomNotificationError` with a `Schema.TaggedErrorClass` and maps
  Durable Object notification failures into it at `runtime.ts:149`.
- `packages/probe/packages/runtime/src/llm/openrouter.ts:77` through `:106`
  models OpenRouter auth, rate-limit, upstream, and timeout failures as tagged
  errors and keeps secrets out of the error payload.
- `apps/pylon/src/codex-agent-executor.ts:1274` through `:1288` and `:1337`
  through `:1363` are intentional fail-soft paths, but both use broad
  `catch {}` blocks that erase the reason.

Best-practice gap:

`effect-solutions` recommends expected failures live in the error channel and
be recoverable with tag-aware handling. Broad `catch {}` and `Promise` APIs
erase whether a failure is expected, transient, invalid input, auth, or defect.
This is especially risky in code like Codex assignment execution, PR publishing,
wallet/presence, and D1 projection code, where a swallowed error can become a
public-safe but low-information closeout ref.

Recommendation:

High impact. Create a "fail-soft but observed" pattern: broad recovery can still
return public-safe fallback refs, but it should first map unknown causes into a
typed private diagnostic such as `PylonCodexPrTitleGenerationFailed` or
`PylonPullRequestPublicationFailed`, with a redacted reason and operation field.
Use `Schema.TaggedErrorClass` for expected errors and reserve defects for truly
unexpected cases.

## 3. Effect Schema and data modeling

Current state:

- Shared contract packages are strongly schema-first. `packages/world-contract/src/index.ts:10`
  through `:50` brands refs, timestamps, cursors, sequences, and source refs.
  The same file uses `Schema.Class` for contract records beginning at
  `index.ts:201`.
- Worker boundary helpers exist in `apps/openagents.com/workers/api/src/json-boundary.ts`.
  `recordFromUnknown` at `:11`, `stringArrayFromUnknown` at `:24`, and
  `parseJsonWithSchema` at `:75` are useful centralization points.
- Pylon native runtime decodes assignment data with Schema in
  `apps/pylon/src/openagents-native-runtime.ts:94` through `:103`.
- Manual casts are still common at IO boundaries. Examples include
  `apps/pylon/src/codex-agent-executor.ts:293` through `:299`,
  `apps/pylon/src/codex-agent-executor.ts:1243` through `:1259`, and
  `packages/world-client/src/index.ts:283` through `:311`, where JSON is parsed
  and then narrowed by hand.

Best-practice gap:

The repo has a good schema vocabulary but does not consistently use it at every
external boundary. `JSON.parse` followed by `as Type` makes malformed payloads
look like valid domain values until later checks fail, and it makes fail-closed
behavior inconsistent.

Recommendation:

High impact. Make `json-boundary.ts` or a shared package the standard JSON
boundary surface: `parseJsonEffect(schema, text, operation)` and
`readRequestJsonEffect(schema, request, operation)` should return
`Effect<A, JsonBoundaryError | Schema.SchemaError>`. Then migrate route bodies,
D1 JSON columns, Pylon local state files, and WebSocket frames to that helper
when those files are touched.

## 4. Effectfulness and authority paths

Current state:

- The Worker runtime and newer provider code contain good Effect boundaries.
  For example, `packages/probe/packages/runtime/src/llm/openrouter.ts:388`
  through `:403` wraps the SDK call in `Effect.tryPromise`, validates the
  result, applies timeout, and retries with a `Schedule`.
- Several Pylon paths remain direct async code with direct `fetch`, `readFile`,
  `JSON.parse`, and environment access. `apps/pylon/src/codex-agent-executor.ts:251`
  through `:270` runs local commands with `Bun.spawn` and a manual timeout;
  `apps/pylon/src/codex-agent-executor.ts:1315` and `:1328` read `Bun.env`
  directly in the PR closeout path.
- `apps/openagents.com/workers/api/src/provider-account-service.ts:69`
  through `:80` uses `Promise.all` in a domain service that otherwise imports
  Effect dependencies.

Best-practice gap:

ADR-0002 says effectful work should be explicit, typed, and testable. Direct
async code is acceptable at thin edge adapters, but the current boundary is not
always thin. Some domain decisions, spend gates, PR publication fallbacks, and
public closeout refs are derived inside untyped async functions.

Recommendation:

High impact. Adopt a repo convention: edge adapters may be raw async only when
their sole job is to call a platform API and immediately map the result into an
Effect service method. Any code that decides authority, payment state,
public-proof state, assignment lifecycle, or product-promise status should
return an `Effect` with a typed error channel.

## 5. Concurrency and resources

Current state:

- There are targeted good examples. `apps/openagents.com/workers/api/src/inference/gym/paid-run.ts:399`
  through `:408` meters paid runs with `Effect.forEach(..., { concurrency: 1 })`,
  making ordering and duplicate-risk explicit.
- OpenRouter applies bounded retry and timeout with `Schedule.exponential` in
  `packages/probe/packages/runtime/src/llm/openrouter.ts:386` through `:403`.
- World client socket handling is still mostly manual. `packages/world-client/src/index.ts:218`
  through `:264` creates a Promise around WebSocket events and cleanup. It
  removes some listeners on open/error, but long-lived message listeners,
  pending command rejection, interruption, close semantics, and reconnect
  lifecycle are outside `Scope` or `acquireRelease`.
- Pylon command execution uses manual timers at
  `apps/pylon/src/codex-agent-executor.ts:251` through `:270`.

Best-practice gap:

`effect-solutions` recommends `Effect.fn` for traced effects, `.pipe()` for
cross-cutting timeout/retry instrumentation, and resource lifetime management
through Effect constructs. Manual Promises make interruption and cleanup harder
to reason about, especially for WebSockets, subprocesses, Durable Object calls,
and local assignment runners.

Recommendation:

Medium impact. Migrate long-lived resources in small steps: WebSocket clients,
subprocess runners, file locks, and assignment leases should expose scoped
Effect APIs that guarantee cleanup on interruption. Add an internal pattern doc
for `acquireRelease`/`Scope` usage in Pylon and world-client before attempting a
large conversion.

## 6. Config

Current state:

- Worker config is centralized and uses brands plus `Redacted` secrets. Examples
  include `apps/openagents.com/workers/api/src/config.ts:500` for `WorkerSecret`,
  `:519` through `:523` for redacted email config, and `:640` through `:653` for
  a tagged config error and config service layer.
- The config file still accepts a large structural env type beginning at
  `config.ts:4` and performs much validation manually, for example
  `requiredString` at `config.ts:664` and email validation at `:690`.
- Pylon code frequently reads direct environment values or defaults from
  `process.env` and `Bun.env`, including
  `apps/pylon/src/codex-agent-executor.ts:1315`, `:1328`, and similar presence,
  wallet, and requester paths found in the scan.

Best-practice gap:

`effect-solutions` recommends `Config`, `Config.schema`, redacted secret values,
and config services/layers so tests can provide config without mutating process
environment. The Worker has a partial equivalent, but Pylon and Probe still
lean on direct env reads.

Recommendation:

High impact. Keep the Worker config service but gradually replace manual
validators with reusable `Config.schema`-style parsers where practical. For
Pylon, introduce a `PylonRuntimeConfig` service that owns environment reads,
redacts secrets, and exposes typed flags. The first migration targets should be
Codex assignment execution, presence heartbeat, wallet, Khala requester, and PR
publication because they affect public proof and owner-local execution.

## 7. Testing

Current state:

- Tests often use `Effect.runPromise` manually inside Vitest. Examples include
  `apps/openagents.com/workers/api/src/thread-file-service.test.ts:1` and the
  `Effect.runPromise` style in `apps/openagents.com/workers/api/src/artanis-administrator-labor-tick.test.ts`.
- Test doubles for D1 and services are hand-built, such as the scripted D1 in
  `thread-file-service.test.ts:97` through `:158`.
- Strong layer-based examples exist in application code, such as the Pylon
  native runtime test layers in `apps/pylon/src/openagents-native-runtime.ts:106`
  through `:141`.

Best-practice gap:

`effect-solutions` recommends Effect-aware tests with per-test layers and
specialized testing utilities such as `TestClock` where time matters. Current
tests are serviceable but often bridge each assertion with `Effect.runPromise`,
which hides environment requirements and makes layer reuse inconsistent.

Recommendation:

Medium impact. Add a small test helper module for Effect tests that standardizes
`runEffect`, per-test layer provision, config overrides, and clock/random
control. New tests for Effect services should prefer layer injection over
process-env mutation and raw fake object threading.

## 8. Consistency against local conventions

Current state:

- The strongest local convention is schema-first public contracts. World
  contracts, provider-account errors, ATIF redaction, and OpenRouter provider
  code are good reference material.
- The weakest consistency point is old/new boundary style. Worker runtime and
  selected packages are Effect-native; many Pylon CLI and executor files remain
  idiomatic Bun/Node async code with manual parsing, env, timers, and broad
  catch blocks.
- `apps/openagents.com/AGENTS.md` explicitly says to consult
  `effect-solutions` before writing Effect code, but the repo has no automated
  check that authority paths avoid raw `Promise`, raw `JSON.parse`, or direct
  env access.

Best-practice gap:

The codebase has enough good patterns to standardize, but not enough guardrails
to keep new code from drifting. The invariant says "everything effectful"; the
implementation currently relies on review discipline to interpret where that
line falls.

Recommendation:

High impact. Add an architecture guard in a follow-up PR, initially report-only,
that lists raw `JSON.parse`, `process.env`, `Bun.env`, bare `catch {}`, and raw
`fetch` in declared authority directories. Do not fail the build immediately.
Use the report to drive targeted migrations and then promote the most sensitive
directories to hard failures.

## Prioritized top 10 improvements

1. Define an "Effect authority boundary" checklist for code that mutates or
   decides payment, settlement, assignment, public proof, product-promise, auth,
   or routing state.
2. Add shared JSON boundary helpers that parse text/request/file/D1 JSON through
   Effect Schema and return typed errors instead of `JSON.parse(... ) as T`.
3. Introduce `PylonRuntimeConfig` as an Effect service and migrate Codex
   assignment, presence, wallet, Khala requester, and PR publication flags to it.
4. Convert provider-account domain orchestration from exported async functions
   to an Effect service facade with typed provider-account error channels.
5. Replace broad `catch {}` in Codex assignment execution and PR publishing with
   fail-soft typed diagnostics that preserve a redacted cause and operation.
6. Add a scoped resource pattern for subprocesses, WebSockets, local leases, and
   file locks using `Scope`/`acquireRelease` or an equivalent local wrapper.
7. Standardize retry/timeout policy with `Schedule` for external HTTP, provider,
   Durable Object, wallet, and GitHub calls.
8. Add Effect test helpers for per-test layer injection, config override,
   redacted secret fixtures, and deterministic time/randomness.
9. Create a report-only architecture scan for raw `JSON.parse`, direct env
   reads, bare `catch {}`, raw `fetch`, and `Effect.runPromise` bridges in
   authority directories.
10. Promote the best examples to local reference docs: Worker runtime layering,
    provider-account tagged errors, OpenRouter retry/config/error discipline,
    world-contract schemas, and ATIF redaction service shape.

## Suggested migration order

1. Guardrails and docs first: checklist plus report-only scan.
2. Boundary helpers second: JSON/config/test helpers that make the right path
   easy.
3. High-risk authority paths third: Pylon Codex assignment execution, provider
   accounts, token usage, wallet/presence, and settlement/proof paths.
4. Resource lifetime fourth: world-client sockets, subprocesses, leases, and
   long-running local runners.
5. Broad cleanup last: opportunistic conversion of lower-risk CLI and display
   code as files are touched.

## Verification notes

This audit is documentation-only and intentionally makes no production code
changes. It is grounded in direct code inspection of the files cited above and
in the local Effect best-practice guides available through `effect-solutions`.
