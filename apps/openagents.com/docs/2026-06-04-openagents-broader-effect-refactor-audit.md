# OpenAgents Autopilot Broader Effect Refactor Audit

Date: 2026-06-04

Scope: the full `openagents` codebase, including `apps/web`,
`workers/api`, `packages/*`, package dependency topology, route/runtime
boundaries, tests, schema packages, and Foldkit UI architecture.

Baseline: this audit rereads and builds on
`docs/2026-06-04-effect-foldkit-codebase-audit.md`. The earlier audit is now a
history of an implemented recommendation set, excluding incident-response work.
This document looks at the next refactor and upgrade frontier: what should be
brought further in line with Effect, Foldkit, and Cloudflare-native best
practices after the first audit's recommendations landed.

This is a planning and code-quality audit only. It does not change production
contracts or invariants.

## References Consulted

- Workspace and repo guidance:
  - `/Users/christopherdavid/work/AGENTS.md`
  - `/Users/christopherdavid/work/INVARIANTS.md`
  - `openagents/AGENTS.md`
  - `openagents/INVARIANTS.md`
- Prior OpenAgents product surface audit:
  - `docs/2026-06-04-effect-foldkit-codebase-audit.md`
- Zero-tech-debt caller inventory:
  - `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- Effect topology guardrail:
  - `scripts/check-effect-topology.mjs`
  - `bun run check:effect-topology`
- Zero-debt architecture guardrail:
  - `scripts/check-zero-debt-architecture.mjs`
  - `bun run check:architecture`
- Local Effect guidance:
  - `effect-solutions show services-and-layers error-handling data-modeling testing config`
- Official Effect documentation:
  - `https://effect.website/docs/requirements-management/services/`
  - `https://effect.website/docs/schema/introduction/`
  - `https://effect.website/docs/error-management/expected-errors/`
  - `https://effect.website/docs/configuration/`
  - `https://effect.website/docs/resource-management/introduction/`
  - `https://effect.website/docs/getting-started/using-generators/`
- Local Cloudflare/Effect reference:
  - `../projects/repos/effect-cf/README.md`
- Package metadata checked on 2026-06-04:
  - `npm view effect-cf version peerDependencies --json`
  - `npm view foldkit version peerDependencies dependencies --json`
  - `npm view @effect/vitest version peerDependencies --json`
  - `bun pm why effect`
  - `bun pm why foldkit`

## Executive Summary

OpenAgents product surface is no longer in the "Effect adoption seed" phase. The first audit drove
real extraction: shared schemas, scoped sync, route modules, thread access,
thread-file `effect-cf` bindings, token leaderboard service boundaries, and
many runtime/time/ID seams now exist. The next wave is not "add Effect imports"
or "split files for their own sake." The next wave is to make the remaining
large route, provider, sync, and projection modules expose typed
`Effect<Success, Error, Requirements>` contracts all the way to their nearest
runtime boundary.

The highest leverage next refactors are:

1. Resolve the Effect version topology. The repo currently has
   `effect@4.0.0-beta.70` for OpenAgents product surface/effect-cf and `effect@4.0.0-beta.66` through
   Foldkit/devtools/plugin peer lines. This is the most important upgrade
   audit item because duplicate Effect runtimes can quietly undermine service,
   schema, and layer identity assumptions.
2. Convert route modules that currently return `Effect.promise(() =>
dependencyPromise(...))` into route modules whose dependencies already
   return `Effect<Response, DomainError, Services>`.
3. Turn provider-account and GitHub-write flows from generic thrown-error
   repositories into typed service/repository layers with serializable
   `Schema.TaggedErrorClass` unions.
4. Move sync-worker and sync-route persistence from Promise repositories into
   Effect services backed by `effect-cf` D1/Durable Object boundaries.
5. Split the remaining Foldkit logged-in update and chat timeline projection
   surfaces by domain, while preserving one model, one message union, and pure
   update semantics.
6. Keep reducing boundary `null` and unknown JSON in long-lived browser model
   state by converting API DTOs into Option/tagged internal state at the
   boundary.
7. Finish UI registry decomposition so `ui/registry.ts` disappears as an
   implementation bucket instead of remaining a 6,000-line compatibility
   module.
8. Add architecture tests for version topology, route Promise adapters, and
   forbidden runtime primitives so the codebase does not regress toward the old
   shape.
9. Treat compatibility paths as temporary debt: search current callers, delete
   caller-free aliases, wrappers, fallbacks, and mode flags, and keep only the
   product/runtime surface OpenAgents product surface would have if it had been Effect-native from
   day one.

## 100% Effect Compliance End State

The intended end state is simple: OpenAgents product surface has one coherent Effect runtime and one
coherent product architecture. Cloudflare, D1, Durable Objects, queues, sync,
provider accounts, runner dispatch, billing, and browser commands are all
modeled as typed Effect services, while Foldkit remains the pure Elm-style UI
runtime that turns facts into model updates and commands.

"Fully Effect-native" should mean all of the following, not just that files
import `Effect`:

- One Effect dependency line across the repo. The current Foldkit beta 66
  exception is documented only because the package ecosystem currently forces
  it; once Foldkit aligns, the exception should be deleted rather than made
  permanent.
- Worker entrypoints are the only Promise boundary. Route groups, repositories,
  sync stores, Omni lifecycle handlers, provider-account operations, billing
  flows, and GitHub-write flows should return
  `Effect.Effect<Success, DomainError, DomainServices>`.
- `Effect.runPromise` is allowed at entrypoints, test harnesses, and temporary
  compatibility facades only. It should not appear inside route modules or
  domain services once their callers are converted.
- `Effect.promise(() => dependencies.somePromise(...))` is a migration smell in
  route modules. The dependency itself should become an Effect service method.
- Expected failures are values, not thrown English strings. Domain failures
  should be represented with `Schema.TaggedErrorClass` or `Data.TaggedError`
  unions, then mapped exhaustively at route boundaries.
- Service methods should have no remaining dependency requirements after layer
  construction. Dependencies belong in `Layer` composition, not in ad hoc
  method plumbing or raw parameter bags.
- Service implementations should use `Effect.fn("Service.method")`,
  structured spans, and Effect logging so workflows are observable without
  leaking secrets.
- Resource lifetime should be explicit. Durable Object stubs, queues, request
  context, WebSocket/session streams, background scheduling, and external
  provider clients should be modeled through services, scoped resources, or
  boundary layers rather than ambient globals.
- Business services should not accept raw Cloudflare `Env`. `Env` should be
  consumed by binding/config layers, with branded config values and
  `Redacted` secrets exposed through an `OpenAgentsWorkerConfig` service.
- Domain services should return domain values or domain errors, never
  `Response`. Route groups own HTTP mapping.
- Time, IDs, and randomness should come from injected services such as
  `Clock`, `Random`, or an OpenAgents product surface UUID service. Business logic should not call
  `new Date`, `Date.now`, `crypto.randomUUID`, or `Math.random` directly.
- JSON and unknown inputs should be decoded at named boundaries with Schema.
  Raw `JSON.parse`, `decodeUnknownSync`, and manual nested `unknown` traversal
  should be treated as temporary compatibility code unless they are isolated in
  a boundary decoder with tests.
- API DTO `null` values may exist at the network boundary, but long-lived
  browser model state should use `Option` or tagged state for product meaning.
- Foldkit update functions stay pure. Effects live in commands,
  subscriptions, services, or boundary layers, not in update transitions.
- No compatibility path survives without a current caller. Route aliases,
  facade exports, mode flags, fallback branches, Promise wrappers, and old DTO
  shims should be searched with `rg`, deleted when unused, and documented only
  when there is a real current dependency.

This target is stricter than "make the current code a little cleaner." It says
the future code should look as if the intended Effect/Foldkit architecture had
existed from the start.

## Zero-Tech-Debt Refactor Rules

Apply the following rules to each refactor slice:

1. State the intended end state before editing. For example: "sync routes
   consume `SyncOutboxStore` and map `SyncRouteError` to HTTP once."
2. Search real callers before preserving compatibility. If a wrapper, route
   alias, prop, mode flag, fallback, or facade export has no current caller,
   delete it.
3. Reshape around the final product surface. Prefer one clear service, route
   group, command module, or projection module over historical mode switches.
4. Move shared rules to one owner. Feature flags, permissions, route gating,
   URL state, request naming, command naming, redaction policy, and response
   mapping should not be duplicated across pages or hidden in render modules.
5. Verify both the intended flow and the deleted assumption. Tests should cover
   the new behavior and prove removed aliases, wrappers, or fallbacks are not
   required by navigation, permissions, persisted state, or sync contracts.
6. Prefer product-intent names over implementation-history names. A module
   called `provider-accounts/service.ts` or `run-timeline/projection.ts` is
   better than a "compat", "legacy", "registry", or "helpers" bucket.

These rules should make the next refactor wave more deletion-oriented. The
goal is not to invent a generic framework for OpenAgents product surface; it is to remove accidental
complexity until the product architecture and the Effect architecture describe
the same thing.

## Full Compliance Scorecard

| Area                    | Current Shape                                                           | Perfect Compliance Target                                                                                            |
| ----------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Effect dependency graph | Beta 70 for OpenAgents product surface/effect-cf and beta 66 through Foldkit peers           | One Effect line, with the current Foldkit exception deleted as soon as the ecosystem allows it                       |
| Worker boundary         | `fetch` plus route modules that still adapt Promise handlers            | One Worker request runtime/layer; route dependencies are Effect programs                                             |
| Service ownership       | Some service tags exist, but large modules still own mixed concerns     | Every durable domain has a service tag, live layer, test layer, typed methods, and no hidden dependency requirements |
| Errors                  | Generic `throw new Error` and string-message classification still exist | Expected failures are tagged errors with exhaustive mappers                                                          |
| Config and secrets      | Raw `Env` flows into business modules                                   | Typed config service with branded values and `Redacted` secrets                                                      |
| Cloudflare bindings     | Some `effect-cf` binding services exist                                 | D1, DO, queues, waitUntil/background work, and Worker runtime are Effect layer concerns                              |
| JSON/schema             | Shared schemas exist, but compatibility parsing remains scattered       | Named boundary decoders with Schema, tests, and no raw parsing in domain logic                                       |
| HTTP mapping            | Business handlers often return `Response` directly                      | Domain services return values/errors; route groups map to HTTP once                                                  |
| Foldkit update          | One pure architecture, but large parent command matrix remains          | Parent dispatcher plus domain command/transition modules                                                             |
| Browser model           | DTO `null`s still persist in long-lived state                           | DTO nulls stop at boundaries; internal state uses Option/tagged ADTs                                                 |
| UI registry             | `ui/registry.ts` remains a large implementation bucket                  | Registry compatibility removed after callers move to final modules                                                   |
| Observability           | Browser command spans are stronger than Worker service spans            | `Effect.fn`, spans, structured logs, and redaction policy across Worker workflows                                    |
| Tests                   | Behavior tests exist, architecture tests are still partial              | Budget and invariant tests enforce the final shape and deletion of legacy paths                                      |

## Deletion Candidates And Caller Checks

The next implementation wave should create a caller inventory before preserving
any compatibility surface. These are the main classes of code that should be
deleted as soon as the current callers are gone:

- Promise facades around Effect services. Examples include repository methods
  or route dependencies that exist only so a Promise-shaped handler can survive
  one more refactor step.
- `Effect.runPromise` bridges below the Worker entrypoint. A bridge is
  acceptable only while converting callers; after route groups consume Effect
  dependencies directly, the bridge should be removed in the same slice.
- Route aliases and app-shell fallbacks without current navigation callers.
  Root, login, team chat, project chat, thread, and file routes should each
  have one product-intent owner rather than historical aliases spread across
  `worker-routes`.
- Response helpers embedded in service modules. `unauthorized`, `forbidden`,
  `notFound`, `badRequest`, and similar helpers belong in route response
  mappers once a domain service owns the failure values.
- UI registry compatibility exports. `ui/registry.ts` is now deleted; keep new
  component families in `ui/layout.ts`, `ui/workroom.ts`, `ui/public.ts`,
  `ui/data-display.ts`, or another specific module instead of recreating a
  compatibility bucket.
- Logged-in parent update compatibility shims. Domain command modules should
  own command creation; parent update should dispatch messages and compose
  results, not preserve every old helper name.
- Browser DTO shims that keep `null` semantics alive after decode. Once a
  model conversion function exists, downstream render modules should stop
  importing the raw DTO shape.
- String-based error classifiers. When a typed error union exists, old
  `error.message.includes(...)` classifiers should be deleted immediately.
- Feature flags or visibility gates duplicated in view code. If a product area
  such as projects is disabled, the rule should live in one feature-gate owner
  and views should consume that decision.
- Test allowlists that only preserve historical debt. Budgets are useful, but
  every allowed exception should have a named owner and a planned deletion
  condition.

The audit should be read as a deletion plan as much as an extraction plan.
Every new service split should ask: which wrappers can now disappear?

## Current State Snapshot

The codebase is meaningfully smaller and better segmented than the original
audit described, but several large files still represent future ownership
boundaries:

- `apps/web/src/ui/registry.ts`: 6,078 lines
- `workers/api/src/index.ts`: 3,513 lines
- `apps/web/src/page/loggedIn/update.ts`: 2,282 lines
- `apps/web/src/page/loggedIn/page/chat.ts`: 2,131 lines
- `workers/api/src/omni-runs.ts`: 2,048 lines
- `workers/api/src/omni-handlers.ts`: 1,737 lines
- `workers/api/src/github-write-connections.ts`: 1,393 lines
- `workers/api/src/billing.ts`: 859 lines
- `apps/web/src/page/loggedIn/model.ts`: 853 lines

`workers/api/src/provider-accounts.ts` is now a five-line compatibility barrel;
provider-account implementation ownership moved to the issue #26 modules.
`workers/api/src/github-write-connections.ts` grew during issue #28 because it
now owns typed GitHub-write errors and Effect service wrappers. The next
GitHub-write deletion point is route-signature extraction, not another
monolith-preserving wrapper.

The current TypeScript surface is about 152 `.ts` files and 57,809 lines under
`apps`, `workers`, and `packages`. The broad direction is good: there are
service tags, schema packages, route modules, focused tests, and stricter
invariants. The remaining work is about depth, not novelty.

## Priority Findings

### 1. Effect version topology is split between beta 70 and beta 66

OpenAgents product surface's workspace packages and Worker are on `effect@4.0.0-beta.70`.
`effect-cf@0.13.1` requires `effect ^4.0.0-beta.70`, matching that line. But
`foldkit@0.102.1`, `@foldkit/devtools-mcp@0.9.0`, and
`@foldkit/vite-plugin@0.7.0` pull `effect@4.0.0-beta.66` and
`@effect/platform-browser@4.0.0-beta.66`. `bun pm why effect` reports both
Effect versions in the install graph.

The latest npm `foldkit` checked during this audit was `0.104.0`, and it still
declares `effect` / `@effect/platform-browser` peer dependencies at
`4.0.0-beta.66`. So this is not a simple "bump Foldkit today" task.

Risk:

- Effect service tags and layer identity are runtime-sensitive.
- Schema values crossing package boundaries can become harder to reason about
  if two Effect versions are present.
- Foldkit app code imports `effect@4.0.0-beta.70` while Foldkit internals expect
  beta 66.
- This can become a subtle class/tag/instance boundary issue long before it
  appears as a failing typecheck.

Recommendation:

- Add a dependency-topology check script that records the current exception
  explicitly: OpenAgents product surface/effect-cf may use beta 70 and Foldkit may pull beta 66 until
  a compatible Foldkit release exists.
- Once Foldkit supports beta 70, upgrade Foldkit, `@foldkit/devtools-mcp`,
  `@foldkit/vite-plugin`, and `@effect/platform-browser` together in one
  version-alignment PR.
- Do not add new shared service/schema abstractions that rely on crossing
  Foldkit-internal Effect identities until the version split is gone.
- Keep this as a release-gate concern: future agents should not treat duplicate
  Effect versions as harmless metadata churn.

The same check showed `@effect/vitest@0.29.0` still peers on `effect ^3.21.0`
and `vitest ^3.2.0`. OpenAgents product surface should continue using plain Vitest plus
`Effect.runPromise` for Effect 4 beta service tests until that peer line
changes.

### 2. Route modules still adapt Promise handlers instead of owning Effect programs

The first audit split route ownership out of `workers/api/src/index.ts`, which
was the right first step. The next issue is that several route modules still
mostly adapt Promise-shaped handlers into Effect:

- Issue #31 converted `workers/api/src/omni-routes.ts`,
  `workers/api/src/provider-account-routes.ts`,
  `workers/api/src/team-chat-routes.ts`, and
  `workers/api/src/worker-routes.ts` dependencies to `RouteEffect` programs.
  These route groups now use `workers/api/src/http/route-effects.ts` for one
  typed route dependency error and response mapper.
- `workers/api/src/sync-routes.ts` contains an `Effect.promise(async () => ...)`
  shell that calls `Effect.runPromise` internally for stream and mutation
  handlers.
- `workers/api/src/thread-file-routes.ts` still has the remaining route
  dependency `Effect.promise` adapters and is the current route-adapter budget
  owner.

The old control flow is no longer true for Omni, provider-account, team-chat,
or Worker fallback routing. Those route groups can now see dependency effects
and route dependency errors. The remaining work is to convert sync/thread-file
routes and then push the Effect boundary deeper into the handlers themselves.

Recommendation:

- Change route dependencies from `(...args) => Promise<Response>` to
  `(...args) => Effect.Effect<Response, DomainRouteError, DomainServices>`.
- Keep `fetch(request, env, ctx)` as the only unavoidable Promise boundary.
- Compose a `WorkerRequestLayer` once per request from `Env`, `Request`, and
  `ExecutionContext`.
- Convert each route group to a domain error union plus one
  `DomainRouteError -> Response` mapper.
- Eliminate nested `Effect.runPromise` calls inside route modules.

Suggested first target: `sync-routes`. It is small enough to finish, uses D1
and Durable Object surfaces that map well to `effect-cf`, and already has a
typed `RequestDecodeError`.

Implementation status:

- Issue #31 added `RouteDependencyError`, `RouteEffect`, `routeEffect`, and
  `routeEffectOrResponse` in `workers/api/src/http/route-effects.ts`.
- `omni-routes`, `provider-account-routes`, `team-chat-routes`, and
  `worker-routes` now accept Effect-returning dependencies and no longer wrap
  dependency calls with `Effect.promise`.
- `workers/api/src/index.ts` owns the temporary Promise-to-Effect adapters for
  still-Promise handlers, with named operations.
- `scripts/check-zero-debt-architecture.mjs` reduced the route dependency
  adapter budget from 39 to 8 and the Worker `Response` surface budget from 115
  to 82.

### 3. `effect-cf` adoption should move from bindings to runtime boundaries

OpenAgents product surface currently uses `effect-cf` for concrete binding services such as
`OpenAgentsDatabase` and `ThreadFileArtifacts`. That is good, but the Worker is
not yet an `effect-cf` Worker boundary. The local `effect-cf` README shows the
intended direction: Cloudflare services modeled as `Context`, `Layer`, and
`Effect`, with runtime boundaries at Worker and Durable Object entrypoints.

Recommendation:

- Keep the existing Worker export stable while proving one smaller boundary
  first.
- Move sync Durable Object access, queue producer access, and `ctx.waitUntil`
  into typed services before attempting `Worker.make` for the full app.
- Create an `OpenAgentsWorkerContext` service for background work scheduling
  and sync notification publication.
- Model `RUNNER_EVENTS` as a typed queue producer service with Schema-encoded
  payloads before enqueue.
- Model `SYNC_ROOM` through a service that turns a `SyncScope` into a stream
  request. Do not construct Durable Object IDs directly in route handlers.
- Only after those services exist, migrate the Worker entrypoint to an
  `effect-cf` `Worker.make` style boundary.

The goal is a Worker runtime whose dependency graph is visible in layer
composition, not a mechanical replacement of `fetch` syntax.

Implementation status:

- Issue #24 introduced `workers/api/src/runtime.ts` as the current Worker
  runtime capability boundary.
- Background scheduling now goes through `scheduleBackgroundWork` /
  `OpenAgentsWorkerContext` instead of direct `ctx.waitUntil` in route and
  handler modules.
- `RUNNER_EVENTS` is modeled as `RunnerEventsQueue`, an `effect-cf` queue
  service with a Schema-encoded `RunnerEventQueueMessage` envelope.
- `SYNC_ROOM` access is modeled as `OpenAgentsSyncRoomNotifications`; route
  modules pass typed `SyncScope` values and no longer construct Durable Object
  IDs. The service uses Cloudflare's name-based `getByName` path.
- D1 binding reads in production Worker modules now pass through
  `openAgentsDatabase` or `syncOutboxStoreLayer`, leaving direct binding access
  inside the runtime boundary.
- `scripts/check-zero-debt-architecture.mjs` enforces a zero budget for direct
  production `ctx.waitUntil`, `OPENAGENTS_DB`, `SYNC_ROOM`, `RUNNER_EVENTS`,
  and `scopeIdFromName` usage outside `workers/api/src/runtime.ts`.

### 4. Provider-account core is split and route errors are typed

Issue #26 split the former `workers/api/src/provider-accounts.ts` monolith into
product-intent modules:

- `provider-account-domain.ts`: provider-account records, public projections,
  row mappers, redaction-preserving metadata helpers, normalization, and
  credential-material rejection.
- `provider-account-errors.ts`: `ProviderAccountError`, implemented with
  `Schema.TaggedErrorClass`, plus helpers for preserving existing public
  messages while carrying typed `_tag` values.
- `provider-account-client.ts`: the OpenAI/Codex device login and OAuth polling
  client plus `OpenAiCodexProviderClient`.
- `provider-account-repository.ts`: D1 persistence and
  `ProviderAccountRepositoryService` / D1 layer builders.
- `provider-account-service.ts`: lifecycle flows for device login start,
  refresh, connected/failed callbacks, health updates, grant issue/resolve,
  runner authorization materialization, and disconnect.
- `provider-accounts.ts`: compatibility barrel for current callers.

Issue #27 then deleted the provider-account route string classifiers. Browser
and service route modules now use `provider-account-route-errors.ts`, which
maps `ProviderAccountError['_tag']` values through an exhaustive status table
and keeps redaction as output sanitization only. The zero-debt architecture
guard enforces `0/0` provider-account message-substring classifiers.

The remaining provider-account debt is now concentrated in compatibility
callers: route dependencies still use Promise adapters until issue #31 moves
them to typed Effect programs, and the `provider-accounts.ts` barrel stays in
place until the final compatibility deletion pass.

Risk:

- Promise route adapters still hide the intended Effect service dependency
  graph.
- The compatibility barrel makes migration easier but should not become the
  permanent ownership boundary.

Recommendation:

- Complete issue #31 by changing provider-account route dependencies from
  Promise-returning handlers to Effect service programs.
- Keep redaction at the route boundary, but do not use redacted strings as
  control-flow input.
- Keep lowering the generic Worker `throw new Error` budget as other domains
  move to typed errors.

### 5. Sync-worker has an Effect store with a remaining Promise facade

`packages/sync-worker/src/index.ts` is a shared package and now exposes
`SyncOutboxStore` as the Effect-native D1 outbox service. Issue #21 moved the
repository methods to Effect values, added typed sync errors, added stored JSON
boundary decoders, and retained `SyncOutboxRepository` /
`makeD1SyncOutboxRepository` as a one-call Promise facade for existing Worker
callers.

Because sync is a core cross-boundary subsystem, the next sync step is deleting
that facade by converting the remaining SyncRoom, notification, and Omni
callers to consume `SyncOutboxStore` directly.

Recommendation:

- Move sync notification, SyncRoom replay, and Omni callers to the store once
  the route conversion pattern is stable.
- Keep the Promise API only as the documented compatibility facade and reduce
  its architecture budget to zero after callers are gone.

### 6. Worker config and secrets are raw `Env`, not a typed config service

`Env` is a useful Cloudflare binding type, but it currently carries business
configuration and secrets directly into many modules:

- OpenAuth client/issuer/app URL
- GitHub client ID/secret
- admin API token
- SHC dispatch URLs and bearer tokens
- runner callback token
- email configuration

Effect's config model and `Redacted` type are designed to make sensitive
configuration explicit and safe in logs. Cloudflare Workers do not have to read
from `process.env` to benefit from this: OpenAgents product surface can build a typed config layer
from Cloudflare `Env` at the Worker boundary.

Recommendation:

- Add `workers/api/src/config.ts` with `OpenAgentsWorkerConfig` as a
  `Context.Service`.
- Convert required values to branded URL/token/email/newtype schemas at
  startup.
- Represent sensitive values with `Redacted`.
- Keep direct `Env` use in binding layers only; business services should depend
  on config services.
- Add tests that build the config layer from minimal, full, and invalid env
  shapes.

This should be done before deeper `effect-cf` Worker boundary migration so the
event runtime can fail fast with typed config errors.

Implementation status:

- Implemented `OpenAgentsWorkerConfig` in `workers/api/src/config.ts` as the
  Worker-boundary typed config service.
- Decoded app URL, OpenAuth issuer/client config, GitHub client config, admin
  token, agent registration secret, Resend config, SHC dispatch config, and
  runner callback token into branded values.
- Represented secret-bearing values with `Redacted`.
- Moved migrated config field reads out of Worker business modules. The only
  production source file allowed to read those raw fields is `config.ts`.
- Added `direct migrated Worker config Env reads` to
  `scripts/check-zero-debt-architecture.mjs` with a zero budget.
- Added focused tests for minimal config, full config, missing config,
  malformed URLs, malformed secrets, malformed email values, malformed SHC
  mode, and sync-boundary config caching.

### 7. Omni remains split by route ownership but not by service ownership

The old audit's route split is complete. Issue #29 added the first
service-owned Omni lifecycle layer, so the remaining pressure is now the
compatibility source still living in `workers/api/src/omni-runs.ts` and the
Promise-shaped orchestration still living in `workers/api/src/omni-handlers.ts`.
Those files still own a broad set of concerns:

- run and deployment assignment construction;
- SHC dispatch;
- runner event normalization and storage;
- callback ingestion;
- public/operator projection;
- billing-aware run updates;
- GitHub write grant and provider grant selection;
- deployment APIs.

Recommendation:

- Split Omni into service domains:
  - `omni/assignments.ts`
  - `omni/runner-events.ts`
  - `omni/run-repository.ts`
  - `omni/deployment-repository.ts`
  - `omni/dispatch-service.ts`
  - `omni/operator-service.ts`
  - `omni/public-service.ts`
- Move SHC dispatch to a service that returns typed dispatch errors instead of
  generic thrown failures.
- Move runner callback validation into Schema-backed request types.
- Move event storage behind `OmniRunStore` service methods with typed storage
  errors.
- Add spans around launch, dispatch, callback ingestion, event persistence,
  billing debit, and sync notification.

This keeps the route extraction benefits while making run lifecycle behavior
auditable as one typed workflow.

Implementation status:

- Issue #29 added `workers/api/src/omni/assignments.ts`,
  `runner-events.ts`, `run-repository.ts`, `deployment-repository.ts`,
  `dispatch-service.ts`, `operator-service.ts`, and `public-service.ts`.
- The new services expose Effect methods with typed Omni errors for assignment,
  repository, dispatch, Schema decode, billing/debit, and public projection
  failures.
- Runner callback event validation now has a Schema-backed service entrypoint
  via `decodeOmniRunnerEvent`, and issue #30 moved callback ingestion through
  that service before events are persisted.
- Billing gates and container usage debits are represented by
  `OmniOperatorService` calls that convert insufficient credits or ledger
  failures into `OmniBillingError`.
- `workers/api/src/omni-services.test.ts` covers the service split alongside
  the existing `omni-runs` behavior tests.
- Issue #30 deepened these seams by replacing generic SHC dispatch failures
  with typed dispatch categories, moving callback ingestion through the
  runner-event service in the route, routing event persistence through Omni
  repositories, adding service spans, and removing route-local `console.error`
  handling for those workflows.
- `scripts/check-zero-debt-architecture.mjs` now names the temporary
  `omni-handlers.ts` `Effect.runPromise` bridge required by the still
  Promise-shaped route handler. Issue #31 owns deleting that allowance when
  route dependencies become Effect programs.

### 8. Foldkit logged-in update is still a command matrix

`apps/web/src/page/loggedIn/update.ts` has improved since the first audit, but
it remains 2,282 lines and owns a large command matrix. It has many
`Effect.gen` command definitions with similar success/failure handling. This
is not a correctness bug; it is maintainability pressure.

Recommendation:

- Keep one top-level `Message` union and one pure `update`.
- Extract command definitions by domain:
  - `providers/commands.ts`
  - `billing/commands.ts`
  - `thread-files/commands.ts`
  - `team-chat/commands.ts`
  - `onboarding/commands.ts`
  - `runs/commands.ts`
- Extract pure state transitions by domain:
  - `billing/update.ts`
  - `providers/update.ts`
  - `thread-files/update.ts`
  - `team-chat/update.ts`
  - `onboarding/update.ts`
- Keep the parent update as a dispatcher that composes domain transition
  results and returns commands.
- Add a small architecture test that forbids new request commands in the parent
  update file unless they are explicitly allowlisted.

Foldkit's Elm-style architecture remains the anchor: messages are facts, model
is source of truth, and side effects stay in commands.

### 9. Chat timeline projection should be its own typed view-model module

`apps/web/src/page/loggedIn/page/chat.ts` is 2,131 lines. It combines view
layout, team message projection, run event decoding, failure summarization,
artifact display, reconnect-card handling, composer form layout, and side-panel
data sections. Recent production fixes landed in this file because run
projection logic lives beside rendering.

Recommendation:

- Extract `apps/web/src/page/loggedIn/run-timeline/projection.ts`.
- Introduce typed view-model ADTs:
  - `TimelineTextPart`
  - `TimelineToolPart`
  - `TimelineReconnectPart`
  - `TimelineArtifactPart`
  - `TimelineRunSummary`
- Move runner event/failure compatibility parsing into a shared package if the
  Worker also needs the same interpretation.
- Keep `page/chat.ts` focused on rendering already-projected messages.
- Add golden projection tests for noisy runner failures, token usage events,
  artifact events, missing artifacts, reconnect-required states, and partial
  streaming text.

This is directly connected to the recent ChatGPT reconnect fix: user-facing
error projection needs a domain-owned projection module, not scattered string
handling in a page renderer.

### 10. Browser model state still preserves DTO nulls in long-lived state

API response schemas correctly use `S.NullOr` for external payloads. The
problem is that many of those nullable DTO fields are stored long-term in the
browser model and then interpreted in views:

- thread file ownership fields;
- team chat author avatar/GitHub fields;
- run summary durations and external IDs;
- provider/account connection metadata;
- billing/onboarding/bootstrap fields.

Recommendation:

- Keep API DTO schemas in `domain/session.ts` and response modules.
- Add conversion functions from DTOs to internal model records.
- Use `Option` or tagged ADTs in the internal model for meaningful states:
  - `RunDuration = Unknown | KnownSeconds`
  - `ProviderIdentity = Anonymous | GitHubIdentity | AccountLabel`
  - `ThreadFileOwnership = Personal | Team`
  - `AgentRunExternalRef = Missing | Present`
- Avoid migrating everything at once. Start with the chat timeline and
  thread-file detail models because they currently carry the most branching.

This aligns with repo guidance to prefer Option/tagged state in the model while
keeping nulls at external boundaries.

### 11. UI registry decomposition is complete

Issue #38 removed the former `apps/web/src/ui/registry.ts` implementation
bucket. UI implementations now live in family modules instead of a 6,000-line
catch-all file.

Recommendation:

- Keep application-shell primitives in `ui/layout.ts`.
- Keep workroom/sidebar/timeline primitives in `ui/workroom.ts`.
- Keep marketing/public-page components in `ui/public.ts`.
- Keep badge/list/media/grid primitives in `ui/data-display.ts`.
- Keep `ui/index.ts` as the stable export barrel.
- Keep `ui/registry.ts` deleted. The zero-debt architecture check now fails if
  the deleted registry file returns.

### 12. Request/response error boundaries should stop returning `Response` early

Many Worker handlers return `Response` directly from inside business logic:
`unauthorized()`, `forbidden()`, `methodNotAllowed()`, JSON 400s, and 404s.
This makes control flow easy to follow locally, but it prevents domain services
from exposing typed errors.

Recommendation:

- Route modules may still convert errors to `Response`.
- Domain services should return domain values or domain errors, never
  `Response`.
- Add per-route error unions for request method, auth, bad body, forbidden,
  not found, storage, dispatch, and config errors.
- Keep one conversion function per route group.
- For product routes, preserve the existing redirect/no-store policy in the
  mapper instead of inside services.

This is the practical way to get Effect's typed error channel into OpenAgents product surface
without making every route unreadable.

### 13. Observability is present in browser commands but thin in Worker services

`apps/web/src/page/loggedIn/commands/api.ts` already wraps request effects with
`Effect.withSpan(options.name)`. Worker services do not yet have the same
consistent span/log shape. Several route modules still use `console.error`
with string names.

Recommendation:

- Wrap domain service methods with `Effect.fn("Service.method")`.
- Add spans around Worker workflows: auth verification, provider callback,
  grant issue/resolve, run launch, SHC dispatch, event ingestion, token
  leaderboard read, sync publication, and file upload/download.
- Replace route-local `console.error` with Effect logging in services and a
  redacted error boundary in routes.
- Keep secret-bearing values as `Redacted` or explicitly redacted strings
  before logging.

### 14. Architecture tests should guard the next refactor wave

The repo already has policy tests for navigation, icons, root routing, sync
subscription, thread access, and file selection. The next high-value tests are
architecture tests that prevent regression toward the old code shape.

Recommendation:

- Add a package topology test:
  - report all installed `effect` versions;
  - allow the current Foldkit beta 66 exception explicitly;
  - fail when unexpected additional Effect versions appear.
- Add a Worker route architecture test:
  - count `Effect.promise(() => dependencies.` adapters in route modules;
  - set a decreasing budget as route groups are converted.
- Add a generic-error budget:
  - count `throw new Error` in production Worker modules;
  - exclude test fixtures and intentional defect boundaries;
  - reduce the budget as provider/Omni/sync services get typed errors.
- Add a UI registry deletion guard:
  - fail if `ui/registry.ts` returns after the decomposition.
- Add a browser parent-update budget:
  - fail if new API command definitions are added directly to
    `loggedIn/update.ts`.

These are not style tests. They are guardrails for the architectural direction
the repo has already chosen.

## Suggested Refactor Roadmap

### Phase 0: Caller inventory and deletion map

- List every remaining Promise facade, compatibility export, route alias,
  generic response helper in service code, parent-update helper, and UI
  registry export that exists only for old callers.
- Maintain the tracked caller inventory in
  `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`.
- Use `rg` to prove current callers before deciding to keep any of them.
- For each kept compatibility path, record:
  - owner module;
  - exact current callers;
  - final Effect-native replacement;
  - deletion condition;
  - test that will fail if the old path is reintroduced after deletion.
- Delete caller-free paths before adding new abstractions.
- Rename modules toward product intent while doing the deletion, not afterward.

Expected payoff: the repo stops polishing legacy shapes and starts converging
on the code that should exist.

### Phase A: Dependency and architecture guardrails

- Add the Effect-version topology test with the current Foldkit beta 66
  exception documented.
- Add line/budget tests for route Promise adapters, `throw new Error` in
  production Worker modules, `ui/registry.ts`, and parent logged-in command
  definitions.
- Record `@effect/vitest` as intentionally deferred while it peers on Effect 3.
- Add direct guardrails for:
  - nested `Effect.runPromise` below entrypoints;
  - raw `JSON.parse` in domain modules;
  - `Date.now`, `new Date`, `crypto.randomUUID`, and `Math.random` in business
    logic;
  - raw Cloudflare `Env` parameters outside binding/config layers;
  - `Response` return types in domain service modules.
- Maintain those guardrails through `bun run check:architecture`, which is
  backed by `scripts/check-zero-debt-architecture.mjs` and included in
  `bun run check:deploy`.

Expected payoff: future work cannot silently reintroduce the old shape.

### Phase B: Sync as the next Effect-native service package

- Convert `packages/sync-worker` to expose `SyncOutboxStore` as a service.
- Add typed sync storage errors.
- Add an `effect-cf` D1-backed layer.
- Convert `workers/api/src/sync-routes.ts` to consume Effect services instead
  of wrapping an async block and nested `Effect.runPromise`. Completed by
  issue #22.

Expected payoff: one core cross-boundary system becomes service-first.

### Phase C: Provider-account typed errors and service split

- Introduce provider-account error unions.
- Replace string-message status classification in browser/service callback
  routes with exhaustive error matching.
- Split repository, service, OpenAI client, and route response conversion.
- Keep existing route behavior stable while changing internal error flow.

Expected payoff: safer auth-material lifecycle and clearer callback behavior.

### Phase D: Route dependency signatures

- Convert `omni-routes`, `provider-account-routes`, `team-chat-routes`, and
  `worker-routes` dependencies from Promise handlers to Effect programs.
- Move route-local response conversion to group mappers.
- Remove nested `Effect.runPromise` from route modules.

Expected payoff: route composition can see errors and dependencies.

### Phase E: Browser projection and update decomposition

- Extract chat run timeline projection from `page/chat.ts`.
- Split logged-in commands and pure transitions by domain.
- Convert DTO null-heavy state to internal Option/tagged records starting with
  thread files and chat timeline.

Expected payoff: smaller Foldkit modules and fewer production fixes in page
renderer files.

### Phase F: UI registry and design-system cleanup

- Keep implementation families out of the deleted `ui/registry.ts`.
- Preserve the deleted-registry guard in the architecture check.
- Keep `ui/index.ts` as the stable public export.

Expected payoff: the local design system becomes navigable and easier to
evolve without touching unrelated UI surfaces.

### Phase G: Worker runtime boundary

- Add config, queue, Durable Object, and waitUntil services.
- Compose one `WorkerRequestLayer`.
- Evaluate moving the Worker entrypoint itself to `effect-cf` `Worker.make`
  once services and route errors are already Effect-native.

Expected payoff: Cloudflare runtime, bindings, background work, and route
services all live in one typed dependency graph.

### Phase H: Final compatibility deletion pass

- Remove remaining Promise facades after route dependencies are Effect-native.
- Keep `ui/registry.ts` deleted after callers moved to final UI modules and
  `ui/index.ts`.
- Delete old DTO shims after model conversion functions are in place.
- Delete string-message error classifiers after tagged error mappers cover the
  domain.
- Delete topology exceptions after Foldkit aligns with OpenAgents product surface's Effect version.
- Replace budget tests with stronger zero-allowed tests when each category is
  complete.

Expected payoff: the audit's "fully Effect-native" target becomes enforced by
the repository rather than remembered by agents.

## GitHub Issue Backlog For Full Implementation

The following list is intended to be copied into GitHub as implementation
issues. It covers every recommendation in this audit, including the
zero-tech-debt deletion work. These are not live GitHub issues yet.

### Issue 1: Create the OpenAgents product surface zero-tech-debt caller inventory

Body:

Build the caller inventory required before preserving any compatibility path.
This issue should identify every Promise facade, route alias, compatibility
export, fallback branch, old DTO shim, UI registry export, parent-update helper,
feature gate duplicate, and route response helper that exists only because of
historical callers.

The tracked inventory for this audit lives at
`docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`.

Acceptance criteria:

- Add a tracked inventory document under `docs/` or a generated report under a
  checked-in architecture test fixture.
- For each kept compatibility path, record the owner module, current callers,
  intended Effect-native replacement, deletion condition, and guardrail test.
- Delete compatibility paths that have no current callers.
- Use `rg`-based evidence for caller checks; do not preserve a path by
  assumption.
- Keep the inventory updated as later issues delete facades.

### Issue 2: Add Effect dependency topology guardrails

Body:

Add an architecture check that reports all installed `effect` versions and
fails for any unexpected Effect runtime split. The current Foldkit beta 66
exception may remain only as an explicit temporary exception while Foldkit's
published peer dependencies require it.

Implemented guardrail: `bun run check:effect-topology`, backed by
`scripts/check-effect-topology.mjs`, and included in `bun run check:deploy`.

Acceptance criteria:

- The check reports the installed Effect versions and the packages that pull
  them in.
- `effect@4.0.0-beta.70` remains the OpenAgents product surface/effect-cf line.
- The Foldkit beta 66 exception is named, documented, and isolated.
- Any third Effect line fails the check.
- The issue records `@effect/vitest` as deferred while it peers on Effect 3.
- The check is included in the normal deploy or architecture test command.

### Issue 3: Add zero-tech-debt architecture tests for forbidden legacy shapes

Body:

Add guardrail tests that prevent OpenAgents product surface from drifting back toward Promise-first,
throw-first, route-local, or raw-runtime patterns while the refactor progresses.

Implemented guardrail: `bun run check:architecture`, backed by
`scripts/check-zero-debt-architecture.mjs`, and included in
`bun run check:deploy`.

Acceptance criteria:

- Add a budget test for `Effect.promise(() => dependencies.` in route modules.
- Add a guardrail for nested `Effect.runPromise` below entrypoints, tests, and
  explicitly named temporary compatibility facades.
- Add a budget test for `throw new Error` in production Worker modules.
- Add guardrails for raw `JSON.parse` in domain modules.
- Add guardrails for `Date.now`, `new Date`, `crypto.randomUUID`, and
  `Math.random` in business logic.
- Add guardrails for raw Cloudflare `Env` parameters outside binding/config
  layers.
- Add guardrails for `Response` return types in domain service modules.
- Add UI registry and parent logged-in update line/definition budgets.

### Issue 4: Convert sync-worker to an Effect-native SyncOutboxStore service

Body:

Refactor `packages/sync-worker` so the sync outbox is a proper Effect service
instead of a Promise repository. This should be the first shared package to
fully demonstrate OpenAgents product surface's service/layer/error standard.

Acceptance criteria:

- Add `SyncOutboxStore` as a `Context.Service`.
- Convert repository methods to return
  `Effect.Effect<Success, SyncOutboxError, never>` after layer construction.
- Add typed errors for storage failure, sequence allocation failure, payload
  decode failure, scope mismatch, accepted/rejected mutation conflicts, and
  missing snapshot/change states.
- Decode stored payload JSON with Schema or a named boundary decoder.
- Provide an `effect-cf` D1-backed live layer for Worker use.
- Provide focused tests for success, decode failure, scope mismatch, and
  sequence allocation failure.
- Keep a temporary Promise facade only if current callers require it, and list
  its deletion condition in the caller inventory.

### Issue 5: Convert sync-routes to consume Effect services directly

Body:

Update `workers/api/src/sync-routes.ts` so it consumes `SyncOutboxStore` and
other Effect services directly instead of wrapping async blocks and invoking
nested `Effect.runPromise`.

Acceptance criteria:

- Route dependencies are Effect programs, not Promise handlers.
- Nested `Effect.runPromise` is removed from `sync-routes`.
- Request decoding errors, auth errors, sync errors, and storage errors are
  represented in a route error union.
- One mapper converts the sync route error union to HTTP responses.
- Existing sync route behavior and subscription policy tests keep passing.
- Any old Promise compatibility facade has a documented deletion condition or
  is deleted.

### Issue 6: Add a typed OpenAgentsWorkerConfig service

Body:

Move business configuration and secrets out of raw Cloudflare `Env` parameters
and into a typed Effect config service built at the Worker boundary.

Acceptance criteria:

- Add `workers/api/src/config.ts` with `OpenAgentsWorkerConfig` as a
  `Context.Service`.
- Decode required URLs, tokens, issuer/client IDs, email configuration, SHC
  dispatch settings, runner callback tokens, and admin tokens with Schema or
  branded constructors.
- Represent secret-bearing values with `Redacted`.
- Direct `Env` access remains only in binding/config layers.
- Business services consume `OpenAgentsWorkerConfig`, not raw `Env`.
- Tests cover minimal valid env, full valid env, missing required values, and
  malformed secret/URL values.

Implementation note:

- Completed by adding the typed `OpenAgentsWorkerConfig` service, branded
  config values, redacted secrets, root Worker wiring, and a zero-budget
  architecture guardrail for direct migrated config field reads outside
  `workers/api/src/config.ts`.

### Issue 7: Model Cloudflare runtime capabilities as Effect services

Body:

Create typed services for Cloudflare runtime capabilities that are currently
ambient or route-local: `ctx.waitUntil`, queue producers, Durable Object stubs,
request context, D1 access, and sync notification publication.

Acceptance criteria:

- Add an `OpenAgentsWorkerContext` or equivalent service for request-scoped
  background work scheduling.
- Model `RUNNER_EVENTS` as a typed queue producer service with Schema-encoded
  payloads before enqueue.
- Model `SYNC_ROOM` Durable Object access through a service that accepts a
  typed `SyncScope` and hides Durable Object ID construction from routes.
- Keep D1 binding access behind service/layer construction.
- Add tests for queue payload encoding, sync scope routing, and background work
  scheduling behavior where feasible.
- Route modules stop constructing these runtime objects directly once the
  services exist.

Implementation note:

- Completed by adding `workers/api/src/runtime.ts`,
  `workers/api/src/runtime.test.ts`, typed queue and sync-room services,
  runtime helpers for D1 and `waitUntil`, production caller migration, and a
  zero-budget architecture guardrail for direct Worker runtime capability
  access outside the runtime boundary.

### Issue 8: Migrate the Worker entrypoint toward an effect-cf Worker boundary

Body:

After route dependencies, config, queues, Durable Objects, and D1 bindings are
modeled as services, migrate the Worker runtime boundary toward the
`effect-cf` `Worker.make` style entrypoint.

Acceptance criteria:

- Compose one `WorkerRequestLayer` from `Env`, `Request`, and
  `ExecutionContext`.
- Keep the public Worker export stable during migration.
- The route composer receives Effect services through layers instead of
  parameter bags.
- The entrypoint is the primary `Effect.runPromise` boundary.
- Existing deploy checks and route policy tests pass.
- Document any temporary boundary exception and its deletion condition.

Implementation note:

- Completed by adding `OpenAgentsWorkerRequest` and `WorkerRequestLayer` to
  `workers/api/src/runtime.ts`.
- `WorkerRequestLayer` composes the request, URL, `Env`, `ExecutionContext`,
  `effect-cf` request/environment/context services, background scheduling,
  sync-room notification access, and sync outbox D1 layer for one request.
- `workers/api/src/worker-routes.ts` now reads the request triple from
  `OpenAgentsWorkerRequest` instead of being invoked as
  `routeRequest(request, env, ctx)`.
- The public Worker export remains stable: `fetch(request, env, ctx)` still
  exists, but it now provides `WorkerRequestLayer` and runs one
  `workerFetchProgram`.
- The top-level fetch recovery path now uses `Effect.catchCause` inside the
  program instead of a Promise-side `try/catch`.
- Temporary boundary exceptions remain for route-group adapters that still
  accept `(request, env, ctx)`, exact-route Promise wrappers, app-shell/thread
  Promise fallbacks, the scheduled handler, and three service bridge
  `Effect.runPromise` calls in `workers/api/src/index.ts`. Delete those when
  issues #31 and #32 convert route dependencies and response mapping to
  Effect-native services, and when the final deletion pass in #44 removes the
  remaining compatibility bridges.

### Issue 9: Split provider accounts into Effect schema, errors, repository, service, client, and routes

Body:

Break up `workers/api/src/provider-accounts.ts` into product-intent modules
with typed service ownership and no generic thrown expected failures.

Status:

- Implemented in issue #26. The compatibility barrel remains by design for
  current callers; deletion/migration is tracked by issues #27, #31, and #44.

Acceptance criteria:

- Add provider-account schema/domain record modules.
- Add a `ProviderAccountError` union using `Schema.TaggedErrorClass` or
  `Data.TaggedError`.
- Add D1 repository service/layer methods for account, grant, device attempt,
  health, and public projection persistence.
- Add lifecycle service methods for connect, reconnect, disconnect, grant
  issue/resolve, health, and runner authorization flows.
- Add an OpenAI/Codex provider client service for device/OAuth interactions.
- Keep route modules limited to request decode, auth, service call, redaction,
  and response mapping.
- Preserve existing redaction and credential-safety behavior.

### Issue 10: Replace provider-account string error classification with tagged errors

Body:

Delete provider-account callback status classification based on English error
message text and replace it with exhaustive matching on typed error values.

Status:

- Implemented in issue #27. Provider-account route modules now use
  `provider-account-route-errors.ts`, and
  `scripts/check-zero-debt-architecture.mjs` enforces a zero budget for
  provider-account route message-substring classifiers.

Acceptance criteria:

- Replace helpers that inspect `error.message` for strings such as "expired",
  "does not match", or "credential-shaped".
- Add tagged errors for grant expiration, runner mismatch, credential material
  rejection, account not connected, device login attempt expiration, and device
  login attempt mismatch.
- Route mappers use exhaustive matching on the error union.
- Tests assert typed error to HTTP status mapping directly.
- Old string classifiers are deleted, not kept as fallbacks.

### Issue 11: Convert GitHub write connection flows to Effect services and typed errors

Body:

Move GitHub write grant, connection, callback, token, and repository access
flows into Effect services with typed errors and redacted config.

Status:

- Implemented in issue #28. `github-write-connections.ts` now exposes
  `GitHubWriteError`, typed repository/lifecycle Effect services, callback
  account and permission validators, and typed failures for grant expiry,
  runner mismatch, callback mismatch, GitHub API failure, token storage
  failure, permission failure, reload failure, and missing connection.
- `github-write-route-errors.ts` maps typed GitHub-write errors to HTTP status
  codes. `scripts/check-zero-debt-architecture.mjs` enforces a zero budget for
  GitHub-write route message-substring classifiers.
- The shared provider-account redactor now recognizes GitHub token prefixes
  such as `gho_...` before route errors are serialized.

Acceptance criteria:

- Add a GitHub write connection service/repository boundary.
- Use typed errors for missing connection, expired grant, callback mismatch,
  GitHub API failure, token storage failure, and permission failure.
- Consume `OpenAgentsWorkerConfig` for GitHub secrets.
- Remove generic thrown expected failures from GitHub write flows.
- Add response mappers in route modules only.
- Add tests for grant issue, callback success, callback mismatch, expired
  grant, and missing connection projection.

### Issue 12: Split Omni run lifecycle into focused Effect services

Status: implemented. The service-owned modules and focused tests now exist;
remaining deeper SHC dispatch and callback-ingestion work is tracked by issue 13.

Body:

Refactor Omni run lifecycle code out of broad mixed-concern modules into
service-owned domains for assignments, run storage, deployment storage, runner
events, dispatch, operator projection, and public projection.

Acceptance criteria:

- Add `omni/assignments.ts`, `omni/runner-events.ts`,
  `omni/run-repository.ts`, `omni/deployment-repository.ts`,
  `omni/dispatch-service.ts`, `omni/operator-service.ts`, and
  `omni/public-service.ts` or equivalent product-intent modules.
- Each service exposes Effect methods with typed domain errors.
- Runner callback validation is Schema-backed.
- Billing-aware run updates are represented as service calls with typed
  billing/debit errors.
- Public/operator projections do not reach directly into persistence details.
- Existing run launch, callback, projection, and deployment tests keep passing
  or are replaced with equivalent behavior coverage.

### Issue 13: Add typed SHC dispatch and runner event ingestion services

Status: implemented. SHC dispatch now emits typed dispatch failure categories,
runner callbacks are Schema-decoded before persistence, and the route-local
console error handling for these workflows has been removed. The temporary
`omni-handlers.ts` `Effect.runPromise` bridge is explicitly budgeted until
issue 14 converts route dependencies to Effect programs.

Body:

Extract SHC dispatch and runner event ingestion into observable Effect services
with typed request/response schemas and failure values.

Acceptance criteria:

- SHC dispatch returns typed errors for unavailable endpoint, rejected request,
  malformed response, missing credentials, timeout, and transport failure.
- Runner event ingestion decodes all inbound callback payloads with Schema.
- Event persistence is behind an `OmniRunStore` or equivalent service.
- Add spans around launch, dispatch, callback ingestion, event persistence,
  billing debit, and sync notification.
- Remove route-local `console.error` handling for these workflows.
- Tests cover successful dispatch, provider reconnect-required failure,
  malformed callback, and storage failure.

### Issue 14: Convert route dependency signatures to Effect programs

Status: implemented. The four listed route groups now consume `RouteEffect`
dependencies and the route dependency adapter budget has been reduced to the
remaining `thread-file-routes.ts` adapters.

Body:

Convert route group dependency signatures from Promise handlers to Effect
programs so route composition can see dependencies and typed errors.

Acceptance criteria:

- Convert `omni-routes`, `provider-account-routes`, `team-chat-routes`, and
  `worker-routes` dependencies from `(...args) => Promise<Response>` to
  `(...args) => Effect.Effect<Response, DomainRouteError, DomainServices>` or
  to domain values plus route mappers where appropriate.
- Remove `Effect.promise(() => dependencies.*)` adapters as each dependency is
  converted.
- Remove nested `Effect.runPromise` from route modules.
- Each route group has one route error union and one response mapper.
- Architecture budgets are reduced after each route group conversion.

### Issue 15: Move HTTP response mapping out of domain services

Body:

Ensure domain services return domain values/errors instead of `Response`.
Route modules should own HTTP status, headers, redirects, no-store policy, and
JSON serialization.

Status:

- Removed the identified service/domain HTTP mapping leak by moving
  `RouteAccessError -> Response` mapping out of `workers/api/src/thread-access.ts`
  into `workers/api/src/http/route-access-response.ts`.
- `ThreadAccessService` now exposes typed authorization values/errors only;
  API JSON and product redirect/no-store behavior are owned by the HTTP mapper.
- Added representative HTTP mapper tests for forbidden/not-found API responses
  and product redirect/no-store behavior.
- Added a zero-budget architecture guard that fails if service/domain modules
  start using HTTP response helpers such as `noStoreJsonResponse`,
  `redirectResponse`, `methodNotAllowed`, `forbidden`, `unauthorized`, or
  `serverError`.
- Lowered the broad Worker `Response` surface budget after removing the
  `thread-access.ts` mapper. Remaining `Response` references are route modules,
  route facades, cookie/transport boundaries, or response parsers, not
  service-owned HTTP error mapping.

Acceptance criteria:

- Identify all production Worker service/domain modules returning `Response`.
- Replace early `unauthorized`, `forbidden`, `notFound`, `badRequest`, and
  `methodNotAllowed` returns in services with typed domain errors.
- Add route-group mappers for method, auth, bad body, forbidden, not found,
  storage, dispatch, config, and product policy errors.
- Preserve current redirect and no-store behavior in route mappers.
- Add tests for representative mapper behavior.
- Add or tighten architecture tests so new service-level `Response` returns
  fail.

### Issue 16: Establish Worker observability and redaction standards

Body:

Apply a consistent Effect observability standard across Worker services,
matching the browser command span discipline and avoiding secret leakage.

Status:

- Added `workers/api/src/observability.ts` as the Worker observability boundary:
  it builds redacted structured log entries, emits Effect-backed route error
  and warning logs, and provides a named `observedPromise` bridge for legacy
  async handlers that need spans before the route stack is fully Effect-native.
- Replaced production Worker `console.error` / `console.warn` calls in
  `index.ts`, provider-account browser routes, and provider-account service
  routes with redacted Effect logging.
- Added spans for auth verification/code exchange, provider callbacks, grant
  issue/resolve, provider auth-material reads, token leaderboard reads, sync
  publication, and thread file upload/detail/download route effects. Existing
  Omni spans continue to cover run launch, SHC dispatch, and runner event
  ingestion.
- Enabled Worker observability and traces in `workers/api/wrangler.jsonc`,
  matching Cloudflare's current logs/traces sampling guidance.
- Added redaction tests proving bearer tokens, OpenCode auth JSON, `auth.json`,
  and GitHub tokens are removed before log entries are emitted.
- Added a zero-budget architecture check for production Worker raw console
  logging so future route-local logs must use the redacted Effect helper.

Acceptance criteria:

- Wrap domain service methods with `Effect.fn("Service.method")` where useful.
- Add spans for auth verification, provider callback, grant issue/resolve, run
  launch, SHC dispatch, event ingestion, token leaderboard read, sync
  publication, and file upload/download.
- Replace route-local `console.error` with Effect logging in services and
  redacted route boundaries.
- Secret-bearing values are `Redacted` or explicitly redacted before logging.
- Add tests or log-shape assertions where feasible for redaction-sensitive
  paths.

### Issue 17: Split logged-in Foldkit commands by domain

Body:

Reduce `apps/web/src/page/loggedIn/update.ts` by extracting command
definitions into domain modules while preserving one top-level `Message` union
and pure update semantics.

Status:

- Extracted all 14 API/request command definitions out of
  `apps/web/src/page/loggedIn/update.ts` into focused domain command modules:
  `providers/commands.ts`, `billing/commands.ts`, `runs/commands.ts`,
  `team-chat/commands.ts`, `sync/commands.ts`, and
  `thread-files/commands.ts`.
- Preserved the single top-level `Message` union in `message.ts` and kept
  parent `update.ts` as the pure dispatcher plus initial command composition.
- Commands remain PascalCase `Command.define` constants and continue to catch
  errors into the same `Failed*` messages.
- `update.ts` re-exports the command constants that tests and views already
  imported from it, preserving the public module surface without keeping
  implementations there.
- Tightened `scripts/check-zero-debt-architecture.mjs`: parent logged-in
  update is now capped at 1,349 lines and `Command.define` count is capped at
  `0`, so new request commands must go into domain command modules.

Acceptance criteria:

- Extract command definitions into modules such as `providers/commands.ts`,
  `billing/commands.ts`, `thread-files/commands.ts`, `team-chat/commands.ts`,
  `onboarding/commands.ts`, and `runs/commands.ts`.
- Commands remain `Command.define` PascalCase constants.
- Commands catch errors and return `Failed*` messages rather than crashing the
  app.
- Parent update imports domain commands instead of defining new API commands
  inline.
- Add or tighten an architecture test that forbids new request commands in the
  parent update file unless explicitly allowlisted.

### Issue 18: Split logged-in Foldkit pure transitions by domain

Body:

Move pure state transitions out of the large parent logged-in update module
into domain update modules while preserving the Foldkit architecture.

Status:

- Extracted pure transition modules for billing, providers, thread files,
  team chat, sync/run projection updates, run state, and session/chrome shell
  messages.
- Moved the onboarding transition module into the `onboarding/` domain folder,
  preserving the existing onboarding command behavior while giving the parent
  dispatcher a domain-owned transition entry point.
- Replaced `apps/web/src/page/loggedIn/update.ts` with an exhaustive message
  dispatcher that delegates to domain transition functions and keeps only
  initial command composition plus public re-exports.
- Preserved the single top-level `Message` union and existing verb/fact-style
  message names; no new side effects were introduced into update functions.
- Representative update/story coverage already spans onboarding, provider
  connection, billing-related command flow, team chat posting, run launch and
  polling, sync projection, and thread-file transitions; those tests now cover
  the extracted modules through the dispatcher.
- Tightened `scripts/check-zero-debt-architecture.mjs`: parent logged-in
  update is capped at 228 lines, `Command.define` remains capped at `0`, and
  direct `evo(...)` mutations in the parent update are capped at `0`.

Acceptance criteria:

- Extract pure transition modules for billing, providers, thread files, team
  chat, onboarding, and run state.
- Parent update remains a dispatcher that composes domain transition results
  and returned commands.
- Messages continue to describe facts and remain verb-first/past-tense where
  applicable.
- No Foldkit side effects move into update functions.
- Update/story tests cover representative domain transitions.

### Issue 19: Extract chat run timeline projection into a typed view-model module

Body:

Move run event, failure, artifact, reconnect, and streaming text projection
out of `apps/web/src/page/loggedIn/page/chat.ts` into a typed projection module.

Status:

- Added `apps/web/src/page/loggedIn/run-timeline/projection.ts` and moved the
  run event, failure, artifact, reconnect, token usage, and streaming text
  projection logic out of `page/chat.ts`.
- Introduced exported run-timeline view-model types for text, tool,
  reconnect, artifact, summary, part, and message projections while keeping
  the renderer on the existing UI timeline component contract.
- Moved the projection tests into
  `apps/web/src/page/loggedIn/run-timeline/projection.test.ts` so they exercise
  the typed projection module directly.
- Added explicit golden cases for token usage summaries, primary
  missing-artifact failures, and partial streaming text replacement; retained
  existing coverage for noisy runner failures, artifact events, reconnect
  required states, closeout plumbing suppression, and visible assistant text.
- Kept compatibility payload parsing local to the browser projection module
  for now. No Worker-side caller uses this exact interpretation yet, so no
  shared package was created.
- Tightened `scripts/check-zero-debt-architecture.mjs`: `page/chat.ts` is
  capped at 606 lines and raw `JSON.parse(...)` in that renderer is capped at
  `0`.

Acceptance criteria:

- Add `apps/web/src/page/loggedIn/run-timeline/projection.ts` or equivalent.
- Introduce typed view-model ADTs for text, tool, reconnect, artifact, and run
  summary parts.
- Keep `page/chat.ts` focused on rendering already-projected messages.
- Add golden projection tests for noisy runner failures, token usage events,
  artifact events, missing artifacts, reconnect-required states, and partial
  streaming text.
- Share projection compatibility parsing with the Worker only through a shared
  package if both sides truly need the same interpretation.

### Issue 20: Convert long-lived browser DTO nulls to Option or tagged state

Body:

Keep API `null` values at network boundaries, but convert them into meaningful
internal state before they enter long-lived Foldkit model records and render
branches.

Status:

- Added model-boundary conversion helpers for the first target browser
  domains while preserving API DTO schemas with `null` at the network
  boundary.
- Introduced `RunDuration = Unknown | KnownSeconds`,
  `AgentRunExternalRef = Missing | Present`, and
  `ThreadFileOwnership = Personal | Team` tagged states.
- Converted long-lived `ChatRunEvent` optional values to `Option<string>` and
  `ChatRunMetadata.externalRunId` to `externalRunRef`.
- Converted thread-file API records/details/references into internal
  ownership-tagged records before storing them in `threadFilesByScope` and
  `threadFileDetailsById`.
- Updated run timeline projection, sync ingestion, team-run cards, and the
  team file-detail guard to branch on `Option` or tagged model state instead
  of raw DTO `null`.
- Added `apps/web/src/page/loggedIn/model-boundary.test.ts` covering present,
  missing, unknown, and malformed boundary cases for external refs, durations,
  optional strings, and thread-file ownership/detail conversion.
- Tightened `scripts/check-zero-debt-architecture.mjs` with a zero budget for
  raw DTO-null branches in converted browser domains.

Acceptance criteria:

- Add DTO-to-model conversion functions for the first target domains: chat run
  timeline and thread-file detail state.
- Introduce tagged states such as `RunDuration = Unknown | KnownSeconds`,
  `ProviderIdentity = Anonymous | GitHubIdentity | AccountLabel`,
  `ThreadFileOwnership = Personal | Team`, and
  `AgentRunExternalRef = Missing | Present` where appropriate.
- Use `Option` for optional values without product-state meaning.
- Views stop branching on raw DTO `null` for converted domains.
- Add model conversion tests for present, missing, unknown, and malformed
  boundary cases.

### Issue 21: Finish UI registry decomposition and delete the registry implementation bucket

Body:

Complete the extraction of implementation families out of
`apps/web/src/ui/registry.ts` so the local design system is navigable and the
registry stops being a 6,000-line compatibility bucket.

Implementation status: complete in issue #38. The implementation bucket was
deleted, `ui/index.ts` remains the stable barrel, and the architecture guard now
fails if `ui/registry.ts` returns.

Acceptance criteria:

- Move app-shell primitives to `ui/layout.ts`.
- Move workroom, sidebar, and timeline primitives to `ui/workroom.ts`.
- Move marketing/public-page components to `ui/public.ts`.
- Move badge, list, media, and grid primitives to `ui/data-display.ts`.
- Keep `ui/index.ts` as the stable public export.
- Move callers to final modules or `ui/index.ts`.
- Keep `ui/registry.ts` deleted now that no implementation remains.

### Issue 22: Centralize feature gates, permissions, route gating, and command naming rules

Body:

Move shared product rules out of view components and duplicated route branches
into one typed owner so disabled product areas, permission checks, route gates,
URL state, and command names are enforced consistently.

Implementation status: complete in issue #39. `apps/web/src/product-policy.ts`
now owns browser feature flags, project-workroom visibility, Core Team
permission gates, logged-in workroom eligibility, route gating, route-to-product
intent names, and command-to-product-intent names. Startup routing uses that
policy to redirect disabled project workroom URLs back to the personal chat
shell. Logged-in model/view/chat-state/update/subscription code consumes the
policy helpers instead of duplicating project and permission checks. The
zero-debt architecture check now fails if direct browser Core Team checks or
the old project-workroom flag return outside the policy owner.

Acceptance criteria:

- Add a typed feature/permission/route gating module or service for browser and
  Worker-owned decisions as appropriate.
- Remove duplicated visibility checks from view components for disabled
  product areas such as projects.
- Keep clean public URL invariants enforced at the route boundary.
- Document how browser commands and route names map to product intent.
- Add tests for disabled product areas, route gating, permission denial, and
  clean redirect behavior.

### Issue 23: Replace raw time, ID, and randomness primitives in business logic

Body:

Inject time, UUID, and randomness through Effect services or OpenAgents product surface-owned
service wrappers so business logic becomes deterministic and testable.

Acceptance criteria:

- Identify production business logic calling `new Date`, `Date.now`,
  `crypto.randomUUID`, or `Math.random`.
- Replace those calls with `Clock`, `Random`, `Effect.uuid`, or an
  OpenAgents product surface-specific UUID service.
- Keep browser DOM/runtime setup exceptions documented if they are truly
  boundary-only.
- Add tests using deterministic clocks/UUIDs for at least provider grants,
  sync sequencing where relevant, and Omni run/event timestamps.
- Tighten the architecture guardrail budget after conversion.

Implementation note, 2026-06-04:

- Production business logic now consumes runtime primitive boundaries instead of
  raw `new Date`, `Date.now`, `crypto.randomUUID`, or `Math.random` calls.
- The remaining raw primitive calls are documented runtime edges:
  `workers/api/src/runtime-primitives.ts`,
  `packages/sync-worker/src/runtime-primitives.ts`, and
  `apps/web/src/time-format.ts`.
- Provider grants, sync sequencing, and Omni queued run/event creation have
  deterministic runtime tests.
- `scripts/check-zero-debt-architecture.mjs` now budgets raw time, UUID, and
  randomness primitives at zero outside the documented boundary files.

### Issue 24: Replace raw JSON and unknown compatibility parsing with Schema boundary decoders

Body:

Move raw JSON parsing and manual `unknown` traversal into named boundary
decoders backed by Schema and tests.

Acceptance criteria:

- Identify raw `JSON.parse`, `decodeUnknownSync`, and manual nested `unknown`
  traversal in production modules.
- Keep parsing only in named boundary decoder modules.
- Decode sync payloads, runner callback payloads, provider callbacks, stored
  projection payloads, and file/artifact metadata with Schema.
- Return typed decode errors instead of generic thrown failures.
- Add tests for valid payloads, malformed JSON, schema mismatch, and backward
  compatibility cases that must remain.

Implementation note, 2026-06-04:

- Raw `JSON.parse` is now confined to named boundary modules:
  `workers/api/src/json-boundary.ts`,
  `packages/sync-worker/src/json-boundary.ts`,
  `packages/sync-schema/src/json-boundary.ts`, and
  `apps/web/src/json-boundary.ts`.
- Worker request bodies, JWT claims, stored Omni assignment/event fields,
  GitHub-write scopes, Resend responses, sync-worker stored payloads,
  token-usage compatibility payloads, and browser run-timeline payloads now
  decode through those boundaries.
- Direct runtime `decodeUnknownSync` calls in Worker and browser modules now go
  through boundary decoder helpers; provider-account schema package exports
  remain the public schema decoder surface for provider API responses.
- Boundary tests cover valid JSON, malformed JSON, schema mismatch, JWT claim
  decoding, mixed string-array compatibility, embedded runner-log JSON, and
  stored projection nested paths.
- `scripts/check-zero-debt-architecture.mjs` now budgets raw JSON parsing at
  zero outside the named boundary decoder files.

### Issue 25: Add service-layer test fixtures using plain Vitest

Body:

Build reusable service test layers and fixtures for Effect 4 beta service tests
while `@effect/vitest` remains incompatible with the repo's Effect line.

Acceptance criteria:

- Add test layers for config, D1 repository fakes, provider-account client
  fakes, SHC dispatch fakes, sync store fakes, and queue producer fakes.
- Tests use plain Vitest plus `Effect.runPromise` at the test boundary.
- No test helper imports `@effect/vitest` while its peer dependencies target
  Effect 3.
- Document the future migration path to `@effect/vitest` once compatible.
- Use these fixtures in at least sync and provider-account service tests.

Implementation note, 2026-06-04:

- Added reusable plain-Vitest service fixtures in
  `workers/api/src/test/service-fixtures.ts` and
  `packages/sync-worker/src/test-fixtures.ts`.
- The fixtures cover Worker config layers, D1 binding fakes,
  provider-account repository/client/lifecycle service layers, SHC dispatch
  fakes, sync store fakes, and runner-event queue producer fakes.
- `packages/sync-worker/src/index.test.ts` now exercises the sync store fixture
  layer, and `workers/api/src/provider-accounts.test.ts` now exercises the
  provider-account lifecycle fixture layer.
- `workers/api/src/test/service-fixtures.test.ts` smoke-tests config, D1, SHC
  dispatch, and queue fixture layers.
- The future `@effect/vitest` migration path is documented in
  `docs/2026-06-04-openagents-effect-test-fixtures.md`.

### Issue 26: Track Foldkit, effect-cf, and @effect/vitest alignment upgrades

Body:

Create a dependency upgrade issue that tracks the package ecosystem work needed
to remove temporary topology exceptions and test-runner limitations.

Acceptance criteria:

- Track the current installed versions of `effect`, `effect-cf`, `foldkit`,
  `@foldkit/devtools-mcp`, `@foldkit/vite-plugin`,
  `@effect/platform-browser`, and `@effect/vitest`.
- Re-check npm metadata before any upgrade attempt.
- Upgrade Foldkit/devtools/plugin/platform-browser together once their peer
  dependencies align with OpenAgents product surface's Effect line.
- Remove the Foldkit beta 66 exception from the topology test after alignment.
- Revisit `@effect/vitest` only when it peers on the repo's Effect major/beta
  line.
- Run full typecheck, deploy checks, and representative Foldkit tests after
  dependency alignment.

Implementation note, 2026-06-04:

- Added `docs/2026-06-04-openagents-effect-dependency-upgrade-tracker.md` as the
  current dependency upgrade tracker for this issue.
- Added `bun run check:effect-upgrade-metadata`, backed by
  `scripts/check-effect-upgrade-metadata.mjs`, to force a fresh `npm view`
  metadata check before any future upgrade attempt.
- Updated `scripts/check-effect-topology.mjs` to print the tracked installed
  versions explicitly.
- Rechecked npm metadata on 2026-06-04. The latest published Foldkit family
  still peers on Effect beta 66, `effect-cf@0.13.1` still matches OpenAgents product surface's beta
  70 line, and `@effect/vitest@beta` now exists but peers on beta 78 rather
  than OpenAgents product surface's current beta 70 line. No dependency bump is safe in this issue;
  the temporary topology exception remains intentional until the ecosystem
  aligns.

### Issue 27: Final compatibility deletion and zero-allowed architecture pass

Body:

After the service, route, browser, UI, and dependency work lands, perform a
final deletion pass that removes temporary migration paths and tightens
architecture budgets to zero where possible.

Acceptance criteria:

- Delete remaining Promise facades after callers consume Effect services.
- Keep `ui/registry.ts` deleted after callers moved to final UI modules and
  `ui/index.ts`.
- Delete old DTO shims after model conversion functions are complete.
- Delete string-message error classifiers after tagged mappers cover the
  domains.
- Delete route aliases, fallback branches, and old mode flags without current
  callers.
- Delete topology exceptions after dependency alignment.
- Replace migration budgets with zero-allowed guardrails for categories that
  are complete.
- Update this audit or its successor with the final state and any explicit
  remaining exceptions.

Implementation note, 2026-06-04:

- Deleted the local Foldkit `LoginRoute` / `loginRouter` client route, the
  old logged-out simulated email/password login module, and its story test.
- Removed the logged-out login submodel/message/update path and the dead
  `SaveSession` command/message path that only supported the simulated auth
  demo.
- Changed public "Log in" header links to root and kept `/login/github` as the
  real auth document navigation.
- Changed Worker `/login` to redirect to root and changed the legacy Worker
  `/chat` alias from root redirect to not found.
- Tightened `scripts/check-zero-debt-architecture.mjs` budgets to the current
  counts for Worker generic throws, raw `Env` annotations, and Worker
  `Response` return surfaces.
- Added zero-allowed guardrails for deleted local login symbols, the deleted
  Worker `/chat` redirect alias, and the deleted local login demo files.
- Updated `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md` with the
  final #44 state and explicit remaining exceptions.
- Remaining exceptions are caller-backed or package-metadata-blocked:
  `SyncOutboxRepository` Promise facade callers still exist, thread-file route
  adapters remain the only route dependency Promise budget, route signatures
  still carry raw `Env` / `Response` migration budgets, legacy run ID
  normalization still protects persisted IDs, and the Foldkit Effect beta 66
  exception remains blocked by the npm metadata recorded in
  `docs/2026-06-04-openagents-effect-dependency-upgrade-tracker.md`.

## Upgrade Notes

- `effect-cf@0.13.1` currently matches OpenAgents product surface's `effect@4.0.0-beta.70` line.
  Extend it by slice; do not rewrite the full Worker in one pass.
- `foldkit@0.102.1` is installed and latest checked `foldkit@0.104.0` still
  peers on `effect@4.0.0-beta.66`. Treat Foldkit/Effect alignment as a tracked
  dependency issue rather than a same-day upgrade.
- `@effect/vitest@0.29.0` still peers on Effect 3. Keep plain Vitest for Effect
  4 beta service tests until that changes.
- The existing `Effect` beta 70 choice is justified by `effect-cf`; the risk is
  not beta 70 itself, but the mixed beta 66/beta 70 graph.
- `bun run check:effect-topology` is the executable guardrail for the current
  state. It allows only the OpenAgents product surface/effect-cf beta 70 line plus the temporary
  Foldkit beta 66 exception, fails on any third Effect runtime line, and records
  `@effect/vitest` as deferred while its published peer line still targets
  Effect 3.

## What Not To Do

- Do not flatten the app into one "Effect everything" rewrite.
- Do not move Foldkit side effects into update functions.
- Do not keep a compatibility path merely because it is already there. Search
  callers first; delete it when no current caller exists.
- Do not make old mode flags, route aliases, facade exports, or fallback
  branches nicer when the intended product surface no longer needs them.
- Do not weaken route, URL, icon, or file-selection invariants to make
  refactors simpler.
- Do not replace explicit file selection with prompt keyword or regex
  inference.
- Do not add more route modules that simply wrap Promise handlers in
  `Effect.promise` and call the work done.
- Do not hide business behavior in route response helpers. Put domain behavior
  in services and HTTP mapping in route mappers.
- Do not preserve raw DTO `null`, untyped JSON parsing, or English error
  strings beyond their boundary modules.
- Do not invent a generic framework for one feature. Extract only the service,
  layer, projection, or command module that makes the final shape coherent.
- Do not add `@effect/vitest` until its peer dependencies match the repo's
  Effect major/beta line.

## Closing Assessment

OpenAgents product surface is now in a better position than the original audit found. The main
remaining work is less about extraction and more about authority: which module
owns the domain state, which service owns the side effect, which error union
owns the failure, and which runtime boundary provides the layer. The next
successful refactor wave should leave fewer Promise adapters, fewer generic
errors, fewer null-heavy internal states, fewer giant renderer/update modules,
fewer compatibility facades, and one explicit plan for resolving the split
Effect dependency graph.

If those changes are made incrementally, OpenAgents product surface can keep its Foldkit UI
discipline while making the Worker side much closer to Effect's service,
schema, error, config, and resource-management model. The final standard should
be deletion-backed: when the intended Effect-native path exists, the old path is
removed, not preserved as a permanent artifact of the migration.
