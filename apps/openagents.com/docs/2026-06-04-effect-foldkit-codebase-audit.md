# OpenAgents Autopilot Effect/Foldkit Codebase Audit

Date: 2026-06-04

Scope: `apps/web`, `workers/api`, and shared `packages/*` in
`openagents`, with emphasis on Effect usage, Foldkit architecture,
runtime boundaries, schema/data modeling, routing, testability, and maintainable
module boundaries.

This is a code-quality and refactor audit only. It does not change production
contracts or invariants.

## References Consulted

- Workspace and repo guidance:
  - `/Users/christopherdavid/work/AGENTS.md`
  - `/Users/christopherdavid/work/INVARIANTS.md`
  - `openagents/AGENTS.md`
  - `openagents/INVARIANTS.md`
- Local Effect guidance:
  - `effect-solutions show basics`
  - `effect-solutions show services-and-layers`
  - `effect-solutions show data-modeling`
  - `effect-solutions show error-handling`
  - `effect-solutions show config`
  - `effect-solutions show testing`
- Official Effect documentation:
  - `https://effect.website/docs/requirements-management/services/`
  - `https://effect.website/docs/schema/introduction/`
  - `https://effect.website/docs/getting-started/using-generators/`
- Local Foldkit references:
  - `../projects/repos/foldkit/packages/foldkit/src`
  - `../projects/repos/foldkit/examples/*`
- Local Cloudflare/Effect reference:
  - `../projects/repos/effect-cf/README.md`
  - `../projects/repos/effect-cf/packages/effect-cf/src/Worker.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/src/DurableObject.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/src/D1.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/src/R2.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/src/Kv.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/src/Queue.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/src/QueueBinding.ts`
  - `../projects/repos/effect-cf/packages/effect-cf/examples/*`

## Executive Summary

OpenAgents product surface already has a strong foundation: TypeScript strictness is high,
`@effect/language-service` is enabled, the Foldkit app keeps the main runtime
boundary in `entry.ts`, domain schemas exist, and several packages already use
Effect Schema classes and branded identifiers. The largest opportunity is not
to "add more Effect" mechanically. It is to move effectful systems into typed
services, make errors explicit, and keep Foldkit update functions as small,
pure state machines.

The highest leverage refactors are:

1. Continue splitting the Worker `index.ts` into domain services,
   repositories, route modules, and small request/session boundaries. Route
   ownership and thread-file storage are split; several domain handlers still
   remain in `index.ts`.
2. Convert Worker request handling from Promise-shaped helpers wrapped in
   `Effect.promise` into typed `Effect<Response, AppError, Services>` programs.
3. Extract browser API command boilerplate and domain helper clusters from
   `loggedIn/update.ts` into reusable request/decode and state modules.
4. Centralize runner event, sync patch, token usage, and provider payload
   decoding in shared Effect Schema modules instead of repeating unknown JSON
   traversal.
5. Replace user-facing keyword/regex selection for file intent with explicit
   selection state or a typed semantic selector, in line with workspace routing
   guidance.
6. Align the `packages/sync-client` state shape with the app's scoped sync
   model before it becomes a widely used abstraction.
7. Split the UI registry entrypoint into component-family modules, then
   continue moving implementation families out of the backing registry.
8. Model public/authenticated routing, route authorization, sidebar source
   authority, and sync subscription planning as typed workflows. The
   2026-06-04 root-route and stale-thread production defects were not caused by
   missing Effect syntax, but they are exactly the kind of cross-boundary state
   drift that proper Effect services, Schema-backed ADTs, and one route
   hydration workflow should prevent.

`effect-cf` is now installed in the Worker package, not only referenced:
OpenAgents product surface has been aligned from Effect `4.0.0-beta.66` to `4.0.0-beta.70`, matching
the current `effect-cf@0.13.1` peer range. The thread-file Worker slice uses
typed `effect-cf` D1 and R2 binding services, and token leaderboards now read
through a typed Effect service backed by the `OpenAgentsDatabase` binding. The
audit's recommended incremental adoption pattern is implemented; a full
Worker-wide `Worker.make`, Durable Object, KV, queue, and
`WorkerContext.waitUntil` migration remains a future architecture direction,
not a same-audit requirement, because this audit explicitly says not to drop
`effect-cf` into the whole Worker at once.

## What Is Already Strong

- `tsconfig.base.json` is appropriately strict: `strict`,
  `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noUnusedLocals`,
  `noImplicitOverride`, `isolatedModules`, `verbatimModuleSyntax`, and the
  Effect language service plugin are enabled.
- ESLint forbids the common escape hatches: explicit `any` and type assertions
  are blocked.
- `apps/web/src/main.ts` and `apps/web/src/entry.ts` preserve the expected
  Foldkit split, with `Runtime.run(...)` contained at the application entry.
- `packages/sync-schema`, `packages/provider-account-schema`, and related
  packages already use Effect Schema, branded IDs, and redaction-oriented tests.
- The app has a meaningful test suite across Foldkit update tests, scene tests,
  schema tests, and Worker tests.
- `m(...)`, `ts(...)`, and `r(...)` conventions are present across the app and
  should be preserved.

## High Priority Findings

### 1. Worker request handling is Promise-shaped instead of Effect-shaped

`workers/api/src/index.ts` imports `Effect` and `Schema`, but most business
logic remains `async`/`Promise` code. The router eventually wraps large
handlers with `Effect.promise`, and the default Worker boundary runs
`Effect.runPromise(routeRequest(...))`.

This loses the main benefits of Effect:

- dependencies are not visible in the `R` channel;
- expected errors are not visible in the `E` channel;
- services cannot be replaced cleanly in tests;
- retry, timeout, logging, spans, interruption, and resource management remain
  ad hoc;
- route-level code grows around environment objects instead of typed services.

Recommended target:

- Keep `fetch(request, env, ctx)` as the only Promise boundary.
- Build a per-request `WorkerAppLayer` from Cloudflare `Env`.
- Define service tags such as `AuthService`, `TeamRepository`,
  `TeamChatRepository`, `ThreadFileRepository`, `OmniRunService`,
  `ProviderAccountService`, `SyncNotifier`, `BillingService`, `ClockService`,
  and `IdGenerator`.
- Give service methods `R = never` after construction. Dependencies should be
  supplied by layers, not threaded through every method.
- Convert route handlers to
  `Effect.Effect<Response, AppRouteError, AuthService | TeamRepository | ...>`.
- Convert all errors to a response in one boundary function.

The first migration does not need to rewrite every route. Start with one
vertical slice, for example thread files or provider accounts, and use it as
the service pattern for the rest of the Worker.

`effect-cf` gives OpenAgents product surface a concrete version of this target. Its `Worker.make`
and `DurableObject.make` entrypoints build one managed Effect runtime per
Cloudflare boundary, provide request/event context as services, expose
`WorkerContext.waitUntil`, and support per-event layers. Its binding modules
provide typed services for D1, R2, KV, queues, queue consumers, service
bindings, and Durable Object RPC. That shape maps cleanly to OpenAgents product surface's current
bindings:

- `OPENAGENTS_DB` should become a D1 service, with SQL moved into repository
  layers. Once `@effect/sql-d1` is introduced, repository code should prefer
  Effect SQL over hand-threaded `env.OPENAGENTS_DB.prepare(...)`.
- `ARTIFACTS` should become an R2 service with typed `R2OperationError`
  style failures and `Option` for missing objects.
- `RUNNER_EVENTS` should become a typed queue producer so enqueue payloads are
  schema-encoded before leaving the Worker.
- `SYNC_ROOM` and any future Durable Object namespace should be accessed
  through service tags, not directly from route handlers.
- `ctx.waitUntil(...)` calls should be wrapped behind a Worker context service
  so background sync publication and billing jobs are observable and testable.

Do not drop `effect-cf` into the whole Worker at once. Adopt it at a vertical
slice boundary after the corresponding service and error model exists. That
keeps the migration auditable and avoids coupling the entire Worker rewrite to
one package install.

### 2. `workers/api/src/index.ts` is an omnibus module

At 7,977 lines, `workers/api/src/index.ts` is carrying too many ownership
boundaries:

- request routing;
- OpenAuth storage;
- session and cookie handling;
- GitHub OAuth and provider-account flows;
- team chat reads/writes;
- team autopilot request construction;
- token usage and credit accounting integration;
- R2-backed thread files;
- sync publication;
- admin/operator endpoints.

This makes localized changes risky because unrelated concepts share helper
functions and hidden assumptions. The current shape also makes it harder to
use Effect services: every domain wants its own repository and error model, but
the single file encourages more helper functions around a global `Env`.

Recommended module map:

- `workers/api/src/http/router.ts`: route table and route parameter decoding.
- `workers/api/src/http/response.ts`: typed error to `Response` conversion.
- `workers/api/src/auth/session.ts`: session lookup, cookies, authorization.
- `workers/api/src/auth/openauth-storage.ts`: storage adapter only.
- `workers/api/src/provider-accounts/service.ts`: account connection flows.
- `workers/api/src/team-chat/repository.ts`: D1 row mapping and SQL.
- `workers/api/src/team-chat/service.ts`: business rules and sync publication.
- `workers/api/src/thread-files/repository.ts`: D1/R2 file persistence.
- `workers/api/src/thread-files/routes.ts`: upload/download/detail endpoints.
- `workers/api/src/omni-runs/service.ts`: run projection and event ingestion.
- `workers/api/src/bootstrap.ts`: `Env` to `Layer` construction.

Keep `index.ts` as the Cloudflare entrypoint and nothing more.

### 3. Unknown JSON parsing is duplicated across browser and Worker code

The browser and Worker both contain hand-written "unknown to maybe record"
helpers. Examples:

- `apps/web/src/page/loggedIn/update.ts` has `recordFromUnknown`,
  `textFromUnknown`, `numberFromUnknown`, `nested`, `firstNumber`, `firstText`,
  `parseJsonRecord`, `embeddedRecord`, token usage parsing, and runner event
  payload extraction.
- `workers/api/src/index.ts` has `isRecord`, `safeJsonRecord`, `nestedUnknown`,
  `optionalNestedString`, `optionalString`, `optionalInteger`, status parsers,
  provider response parsers, and similar token/run projection helpers.
- `workers/api/src/token-usage.ts` has additional JSON parse and payload
  extraction logic.

This is the place where Effect Schema should carry more of the load. Repeated
manual traversal tends to create subtly different behavior between the UI,
Worker, and tests.

Recommended target:

- Put runner event, token usage, provider account, and sync payload decoders in
  shared packages.
- Prefer `Schema.Class`, `Schema.TaggedClass`, branded IDs, and
  `Schema.decodeUnknownEffect`.
- Convert API DTOs once at the boundary, then move internal app state to typed
  domain records.
- Keep lossy or compatibility parsing in one explicitly named module, for
  example `packages/runner-event-schema/src/compat.ts`.

### 4. `loggedIn/update.ts` mixes pure update logic, commands, parsers, and DOM

`apps/web/src/page/loggedIn/update.ts` is 3,004 lines and does much more than
transition model state:

- model transitions;
- command construction;
- API request/response decoding;
- token usage projection;
- sync patch projection;
- browser URL mutation;
- DOM downloads;
- file upload form access;
- polling and timeout behavior.

Foldkit update functions stay easiest to reason about when the update layer is
a pure, compact state machine and commands are built by small effectful
modules. The current file is still testable, but it has grown into the main
browser application service layer.

Recommended split:

- `apps/web/src/page/loggedIn/state.ts`: pure model transitions.
- `apps/web/src/page/loggedIn/commands/api.ts`: request/decode/error helpers.
- `apps/web/src/page/loggedIn/commands/dom.ts`: URL and download commands.
- `apps/web/src/page/loggedIn/team-chat/update.ts`: team chat message handling.
- `apps/web/src/page/loggedIn/thread-files/update.ts`: files workflow.
- `apps/web/src/page/loggedIn/billing/update.ts`: billing and credit actions.
- `apps/web/src/page/loggedIn/providers/update.ts`: provider connection flows.
- `apps/web/src/page/loggedIn/sync/projection.ts`: sync and cursor mapping.

The split should preserve current message names where possible. That limits
test churn and keeps the Foldkit model recognizable.

### 5. Browser API commands repeat request/decode/error boilerplate

The browser update file repeats the same command pattern across provider login,
billing, team chat, sync snapshots, file lists, file detail, file upload,
file download, and run fetches:

1. create a request;
2. call `fetch`;
3. parse JSON or blob;
4. check status;
5. decode with Effect Schema;
6. map all failures to a message.

This should be consolidated before more endpoints are added.

Recommended helper shape:

```ts
const requestJson = <Schema extends S.Top>(options: {
  readonly name: string
  readonly request: RequestInfo | URL
  readonly init?: RequestInit
  readonly schema: Schema
}) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () => fetch(options.request, options.init),
      catch: cause => new ApiNetworkError({ cause }),
    })

    const payload = yield* decodeJsonResponse(response)
    return yield* S.decodeUnknownEffect(options.schema)(payload)
  }).pipe(Effect.withSpan(options.name), Effect.timeout('30 seconds'))
```

Mutations should generally not retry automatically. Idempotent polling and
snapshot reads can opt into a retry schedule.

### 6. Error models are mostly thrown generic `Error`

The Worker frequently throws `new Error(...)` inside helpers for GitHub,
provider account, storage, D1 reload, billing, and grant flows. That is normal
for Promise code, but it is weak once Effect is available.

Recommended target:

- Use `Data.TaggedError` or `Schema.TaggedErrorClass` for expected errors.
- Give each domain a small error union:
  - `AuthError`
  - `ProviderAccountError`
  - `TeamChatError`
  - `ThreadFileError`
  - `OmniRunError`
  - `BillingError`
- Convert those errors to HTTP responses centrally.
- Keep defects for impossible programmer errors.

Typed errors will also make tests clearer: tests can assert the domain error
instead of matching response strings after the route boundary.

### 7. Time and identity generation are not consistently injected

Some modules already accept `now` and `makeUuid` options, which is a good
pattern. The main Worker file and sync code still call `new Date()`,
`Date.now()`, and `crypto.randomUUID()` directly in many places.

Recommended target:

- Standardize on an `IdGenerator` service for UUIDs.
- Standardize on Effect clock APIs or a small `ClockService` for timestamps.
- Keep test layers deterministic.
- Use direct platform calls only in service live layers.

This will make token accounting, sync cursor publication, file upload
timestamps, and run event tests less brittle.

### Incident addendum: root routing and stale thread links

The 2026-06-04 production issues around `/`, stale `/t/:id` sidebar links, and
forbidden thread sync requests expose a specific architectural gap. They would
not be prevented merely by wrapping more functions in `Effect.gen`. They would
be largely preventable by using Effect and Foldkit in the stricter form this
audit recommends: one typed route/session/sync workflow, one source of truth
for sidebar thread rows, and explicit domain errors before HTTP or UI
conversion.

The root redirect bug happened because `/` was simultaneously being used as a
public homepage route and the authenticated chat route. The route parser was
allowed to treat the same path as the protected `ChatRoute`, then logged-out
startup correctly redirected protected routes to `/login`. TypeScript accepted
that because the route union modeled "what route did the parser produce", not
"what is the product policy for this URL under this auth state".

Recommended target:

- Keep `HomeRoute` as the only parsed representation of `/`.
- Add a pure `resolveStartupRoute(authState, parsedRoute)` domain function
  whose return type is an ADT such as `PublicHome | AuthenticatedShell |
PublicRoute | Redirect`.
- Make authenticated root-to-chat behavior an explicit branch in that resolver,
  not an artifact of parser order.
- Keep a regression table with at least these cases:
  - logged-out `/` => public early access homepage;
  - authenticated `/` => chat shell;
  - logged-out app routes => `/login`;
  - unknown public paths => `/`;
  - `/login/github` => document navigation.

The stale thread bug happened because several effects independently interpreted
the same `/t/:id` route:

- auth bootstrap could seed sidebar thread rows before live workspace sync
  loaded;
- workspace sync could later replace that list and remove stale rows;
- route entry unconditionally requested `thread:<id>` snapshot/stream;
- fallback detail fetch then requested `/api/omni/agent-runs/:id`;
- Worker sync authorization and agent-run detail resolution were separate
  checks that could disagree for stale IDs.

That shape creates fan-out before authority is established. The desired Effect
shape is a single `OpenThreadRoute` workflow:

1. Resolve the route ID through a `ThreadAccessService`.
2. Return `AuthorizedThread` with canonical run ID, route ID, owner/team scope,
   sync scopes, and sidebar item projection, or return a typed
   `ThreadRouteError`.
3. Derive snapshot commands, WebSocket subscriptions, file loading, and fallback
   detail fetches only from `AuthorizedThread`.
4. Convert `ThreadRouteError` once at the boundary: redirect home for dead
   hard-loads, show a small unavailable state for in-app navigation if needed,
   and never start a sync stream for an unauthorized scope.

That service should be shared conceptually between Worker and browser
boundaries even if the implementations differ:

- Worker: `ThreadAccessService.resolveForUser(userId, routeId)` returns an
  Effect with errors such as `ThreadNotFound`, `ThreadForbidden`, and
  `ThreadArchived`. Route handlers and sync authorization consume the same
  resolver.
- Browser: `ThreadRouteProjection` is a Schema-backed model state such as
  `Idle | Resolving | Authorized | Unavailable`, and sync subscription
  dependencies are computed only from `Authorized`.
- Sidebar: thread rows should come from the live workspace sync collection, not
  from auth bootstrap. If bootstrap ever needs initial sidebar rows again,
  encode them as `UnverifiedBootstrapThread` and require a sync or access
  confirmation before rendering clickable `/t/:id` links.

Concretely, add these refactor tasks:

- Extract `apps/web/src/routing/startup.ts` for the public/authenticated route
  policy and move the current root special case there.
- Extract `apps/web/src/page/loggedIn/thread-route.ts` for route hydration,
  with an ADT that owns `Resolving`, `Authorized`, and `Unavailable` states.
- Change `syncScopesForModel` so it cannot derive `thread:<id>` from the raw
  route alone; it should require authorized thread state.
- Add a Worker `ThreadAccessService` around `findAuthorizedAgentRunBundle`,
  `resolveAgentRunId`, and team autopilot thread lookup. Use it from
  `/t/:id`, `/api/sync/thread/:id/*`, and `/api/omni/agent-runs/:id`.
- Add one `RouteAccessError -> Response` mapper so 403/404/redirect behavior is
  consistent across hard loads, sync snapshots, streams, and detail APIs.
- Remove `AuthBootstrap.missions` from the long-term public bootstrap contract,
  or rename/retype it to make its unverified nature explicit.
- Promote the added `main.test.ts` and sidebar scene coverage into the permanent
  deploy gate, because route/auth policy is production-critical.

The key lesson is that Effect structure helps most when it narrows fan-out. The
app should not let a route change independently trigger sync, detail fetches,
sidebar projection, URL mutation, and file loading until a typed policy has
resolved what that route means for the current authenticated user.

## Medium Priority Findings

### 8. `Effect.provide` is repeated inside browser commands

`apps/web/src/command.ts` provides `BrowserKeyValueStore.layerLocalStorage`
inside `SaveSession` and `ClearSession`.

Foldkit command definitions may need self-contained effects, so this is not a
blocking defect. Still, Effect service guidance favors providing layers once
near the application boundary. At minimum, hoist the layer to a named constant
and consider a `SessionStore` command module so tests can substitute storage
without reworking command internals.

### 9. Sync client package state is not scoped like app sync state

`packages/sync-client/src/index.ts` has a flat `collections` record. The app
model in `apps/web/src/page/loggedIn/model.ts` already uses
`collectionByScope`, and the update logic applies patches by scope.

If `packages/sync-client` is intended to become the reusable abstraction, it
should match the app-level state shape now:

- `collectionsByScope: Record<SyncScope, SyncCollections>`;
- tests for same `collection` and `id` in two scopes;
- cursor state per scope;
- explicit handling for missing scopes.

If the package is obsolete, remove or archive it to avoid future callers using
the wrong state model.

### 10. WebSocket subscription decoding should use schema results, not sync throws

`apps/web/src/subscriptions.ts` parses WebSocket payloads with `JSON.parse` and
`S.decodeUnknownSync` inside an event callback. The surrounding callback catches
errors, so this is not currently crash-prone. The smell is that validation,
transport failure, and app messages are all represented as strings.

Recommended target:

- Define a shared `ServerSyncMessage` schema union.
- Decode with `Schema.decodeUnknownEither` or an Effect decoder.
- Send a structured failure message that includes message kind, parse failure,
  or cursor gap reason.
- Consider explicit reconnect/backoff state instead of relying on polling as
  an implicit fallback.

### 11. Internal model state still carries boundary nulls

External API schemas legitimately use `S.NullOr(...)`, especially under
`apps/web/src/domain/session.ts`. The smell starts when null-heavy DTOs become
long-lived internal model state.

Recommended target:

- Keep nulls at API boundaries.
- Convert to internal model types using `Option` or tagged ADTs.
- Give ambiguous fields names that describe their semantics, for example
  `ThreadFileOwnership`, `MaybeProviderIdentity`, or `RunLifecycle`.

This aligns with the repo guidance that model fields should prefer `Option`
over null or undefined.

### 12. User-facing keyword/regex selection is close to the workspace invariant

The explicit `@autopilot` command parser is a bounded command affordance and
can remain deterministic. The riskier area is file intent detection based on
prompt text, such as PDF/file/many-file keyword and regex checks in the team
autopilot path.

Workspace guidance prohibits ad hoc string or keyword matching for
user-facing intent routing, retrieval routing, and tool selection. File
selection should therefore move toward one of these shapes:

- explicit selected file IDs from UI state;
- an Effect Schema request type that states file intent directly;
- a central semantic selector based on embeddings/cosine similarity;
- a structured query planner with an explicit output schema.

Do not add more regex heuristics in this path.

### 13. Server route parsing is a long regex chain

The Worker route section is a long sequence of URL checks and regex matches.
There is already a more structured sync path parser; most routes should move
in that direction.

Recommended target:

- Use a typed route table or Hono route modules.
- Decode path parameters once.
- Keep route handlers as small calls into services.
- Keep response creation centralized.

This will reduce accidental route-order bugs and make authorization checks
auditable by route group.

### 14. `apps/web/src/ui/index.ts` is too large for a design-system registry

`apps/web/src/ui/index.ts` is 6,943 lines. It appears to be an intentional
local Foldkit/Tailwind UI registry, but it is now too large to scan quickly.

Recommended split:

- `ui/layout.ts`
- `ui/forms.ts`
- `ui/navigation.ts`
- `ui/feedback.ts`
- `ui/data-display.ts`
- `ui/page-examples.ts`
- `ui/index.ts` as a barrel export

Keep the public API stable while moving implementation families into separate
files. This should be a mechanical refactor with snapshot/scene coverage.

## Suggested Refactor Plan

### Phase 1: Low-risk extraction

- Done 2026-06-04: aligned OpenAgents product surface Effect dependencies from `4.0.0-beta.66` to
  `4.0.0-beta.70`, matching the current `effect-cf@0.13.1` peer dependency
  line and unblocking direct evaluation of `effect-cf` in Worker slices.
- Done 2026-06-04: extracted browser `requestJson`, `requestBlob`, and
  `decodeJsonResponse` helpers from `loggedIn/update.ts` into
  `apps/web/src/page/loggedIn/commands/api.ts`.
- Done 2026-06-04: extracted sync and runner-event projection helpers into
  `apps/web/src/page/loggedIn/sync/projection.ts`.
- Done 2026-06-04: extracted logged-in DOM command helpers into
  `apps/web/src/page/loggedIn/commands/dom.ts`.
- Done 2026-06-04: hoisted repeated local-storage layer provisioning into the
  named `apps/web/src/commands/session.ts` command module.
- Done 2026-06-04: added focused helper coverage in
  `apps/web/src/page/loggedIn/commands/api.test.ts` for successful JSON
  decoding, failed API error-message preservation, and blob downloads.
- Done 2026-06-04: added focused projection coverage in
  `apps/web/src/page/loggedIn/sync/projection.test.ts` for token usage
  projection, legacy route ID normalization, sync snapshot/patch application,
  and run reconstruction from sync collections.
- Done 2026-06-04: added focused session command coverage in
  `apps/web/src/commands/session.test.ts` for saving and clearing the encoded
  auth session through the configured key-value store effects.

Expected payoff: smaller update file, less repeated failure handling, and a
clearer place to add timeouts/spans.

### Phase 2: Shared schemas and scoped sync

- Done 2026-06-04: moved duplicated runner event and token usage parsing into
  shared schemas.
- Done 2026-06-04: moved token usage compatibility decoding into
  `packages/sync-schema/src/token-usage.ts` and updated browser and Worker
  consumers to use the shared decoder.
- Done 2026-06-04: moved runner-event callback payload normalization into
  `packages/sync-schema/src/runner-event.ts` and updated Worker event
  construction to consume it.
- Done 2026-06-04: moved OpenAI provider response payload decoders into
  `packages/provider-account-schema` and updated the Worker provider account
  service to consume the shared decoders.
- Done 2026-06-04: fixed `packages/sync-client` so scope is part of the state
  model through `collectionsByScope` and `cursorsByScope`.
- Done 2026-06-04: added regression tests for multi-scope sync patches in
  `packages/sync-client/src/index.test.ts` and wired `packages/sync-client`
  into the root package test suite.

Expected payoff: browser and Worker projections stop drifting.

### Phase 3: Worker service vertical slice

- Done 2026-06-04: chose thread files as the first Worker service vertical
  slice and introduced a `ThreadFileRepository` Effect service for the
  download visibility write path.
- Done 2026-06-04: moved thread-file row/public types, object-key/checksum
  helpers, read/list/insert/detail SQL helpers, and message-reference helpers
  from `workers/api/src/index.ts` into `workers/api/src/thread-files.ts`.
- Done 2026-06-04: expanded `ThreadFileRepository` with read/list/insert,
  detail, and reference methods and wired the public thread-file API routes to
  use those service methods.
- Done 2026-06-04: converted the team files route handler to an
  `Effect<Response>` program with local repository-error-to-response mapping.
- Done 2026-06-04: converted the thread-file detail/PATCH route handler to an
  `Effect<Response>` program and reused a shared thread-file repository error
  response mapper.
- Done 2026-06-04: converted the thread-file download route handler to an
  `Effect<Response>` program with repository lookup/authorization and R2 fetch
  modeled inside the route program.
- Done 2026-06-04: converted the thread-file list/upload route handler to an
  `Effect<Response>` program and removed the old Promise-to-repository runner
  from the public thread-file route path.
- Done 2026-06-04: added broader fake-D1 service tests for team listing,
  insert/reload, and detail/reference assembly. Kept plain Vitest because the
  current `@effect/vitest@0.29.0` peer range is `effect ^3.21.0`, while OpenAgents product surface
  is pinned to `effect 4.0.0-beta.70`.
- Done 2026-06-04: installed `effect-cf@0.13.1` and
  `@effect/sql-d1@4.0.0-beta.70`, added typed `OpenAgentsDatabase` and
  `ThreadFileArtifacts` binding services, and moved the thread-file live D1/R2
  storage boundary onto `effect-cf` layers.

Expected payoff: a concrete template for the rest of the Worker migration.

### Phase 4: Worker module split

- Done 2026-06-04: moved the public thread-file route group out of
  `workers/api/src/index.ts` into `workers/api/src/thread-file-routes.ts` with
  dependency-injected session, response, authorization, and sync helpers.
- Done 2026-06-04: moved shared HTTP response helpers into
  `workers/api/src/http/responses.ts` and reused them from `index.ts` and the
  thread-file route module.
- Done 2026-06-04: moved `/api/health` and `/api/sync/...` route parsing and
  handlers into `workers/api/src/sync-routes.ts` with injected session,
  authorization, and refreshed-cookie boundaries.
- Done 2026-06-04: moved provider account and GitHub write disconnect route
  matching into `workers/api/src/provider-account-routes.ts` with injected
  handler functions.
- Done 2026-06-04: moved Autopilot, billing, and Omni run/deployment route
  matching into `workers/api/src/omni-routes.ts` with injected handler
  functions.
- Done 2026-06-04: moved team chat route matching into
  `workers/api/src/team-chat-routes.ts` with injected handler functions.
- Done 2026-06-04: added `workers/api/src/http/router.ts` and moved static
  exact route matching in `routeRequest` to a typed exact-route table.
- Done 2026-06-04: moved final Worker route composition into
  `workers/api/src/worker-routes.ts`, leaving `index.ts` to construct route
  dependencies and delegate the request boundary.
- Done 2026-06-04: centralized shared response conversion in
  `workers/api/src/http/responses.ts`; route-local authorization now enters
  route modules through explicit injected policies.
- Done 2026-06-04: replaced the main inline regex chain with route modules,
  a typed exact-route table, and one Worker route composer.
- Done 2026-06-04: reduced `index.ts` routing ownership to bootstrap and
  Cloudflare boundary wiring. Domain handler/service extraction remains the
  long-term architecture direction from the findings, but Phase 4 route
  ownership is complete.

Expected payoff: lower blast radius for production Worker changes.

### Phase 5: Intent and file-selection cleanup

- Done 2026-06-04: preserved exact `@autopilot` command parsing as a bounded
  parser and added backend parser tests for leading, trailing, standalone-line,
  and non-command text cases.
- Done 2026-06-04: replaced prompt keyword file selection with explicit
  authorized file IDs from request/metadata, added a repo invariant, and added
  tests showing prompt wording alone selects no files.

Expected payoff: compliance with workspace semantic-routing guidance and fewer
surprising run contexts.

### Phase 6: Route, session, and sync consistency

- Done 2026-06-04: added `apps/web/src/routing/startup.ts`, a route policy
  module that resolves parsed routes against auth state. Root remains
  `HomeRoute` at the parser layer, and authenticated root-to-chat behavior is
  an explicit policy branch.
- Done 2026-06-04: added `workers/api/src/thread-access.ts` with
  `ThreadAccessService` as an Effect service and uses from product thread
  pages, sync authorization, and agent-run detail APIs.
- Done 2026-06-04: added
  `apps/web/src/page/loggedIn/thread-route.ts` so browser thread route
  hydration is represented as `Idle | Resolving | Authorized | Unavailable`
  instead of deriving all behavior from a raw `ThreadRoute`.
- Done 2026-06-04: changed `syncScopesForModel` to derive `thread:<id>` only
  from authorized thread-route state.
- Done 2026-06-04: sidebar thread rows remain sourced from workspace sync; the
  stale bootstrap thread path is rejected through `ThreadAccessService` and
  covered by tests.
- Done 2026-06-04: `apps/web/src/main.test.ts`, logged-in scene coverage,
  route policy tests, and subscription tests are included in
  `bun run check:deploy`.

Expected payoff: root/home auth semantics, sidebar thread rows, sync streams,
and agent-run detail fetches stop drifting across separate boundaries.

## Testing Recommendations

- Keep Foldkit update tests for pure model transitions.
- Use `@effect/vitest` once services/layers are introduced.
- Add tests around typed error values before HTTP response conversion.
- Add schema tests for every compatibility decoder that accepts unknown JSON.
- Add sync tests for same collection/id across two scopes.
- Add route tests at the module level once route groups leave `index.ts`.
- Done 2026-06-04: added route-policy tests that cross product path and auth
  state, especially root, login, unknown public paths, and authenticated app
  paths.
- Done 2026-06-04: added thread-access service tests for personal runs, team
  autopilot threads, archived/deleted runs, forbidden team runs, and stale
  bootstrap IDs.
- Done 2026-06-04: added subscription tests proving no `thread:<id>` stream is
  opened before thread access has been authorized.

Docs-only note: no production behavior was changed by this audit.

## Implementation Notes

### 2026-06-04 browser API command extraction

The first audit implementation slice extracted the repeated browser command
request/decode/error pattern into `apps/web/src/page/loggedIn/commands/api.ts`.
`loggedIn/update.ts` still owns Foldkit messages and state transitions, but
provider login, billing, chat, sync snapshot, thread file, upload/download,
run launch, and run polling commands now call shared `requestJson` or
`requestBlob` helpers.

The helper keeps API error text preservation centralized through a typed
`ChatApiHttpError`, keeps network/DOM failures wrapped as `ChatApiError`, and
leaves mutation retry policy unchanged. File upload retains the existing
30-second abort behavior and now clears the abort timer when the request
finishes.

No production contract or invariant changed. Phase 1 is complete.

### 2026-06-04 browser sync projection extraction

The next Phase 1 implementation slice moved the browser's sync and runner-event
projection helpers into `apps/web/src/page/loggedIn/sync/projection.ts`.
`loggedIn/update.ts` now imports run ID display normalization, token-aware
event projection, launch/run response projection, sync snapshot URLs, sync
snapshot application, sync patch application, and sync-collection run
reconstruction from that module.

The extraction preserves the current lossy unknown-JSON compatibility parsing
behavior intentionally. Phase 2 should replace those compatibility helpers with
shared Effect Schema decoders so browser and Worker projections stop drifting.

Focused tests now cover token usage projection from runner payloads, legacy
`agent_run_<hex>` route normalization, scoped sync snapshot/patch application,
pending mutation removal, and run reconstruction from sync collections.

No production contract or invariant changed. Remaining Phase 1 work at this
point was to move DOM/local-storage command helpers out of `loggedIn/update.ts`
and `apps/web/src/command.ts`.

### 2026-06-04 browser DOM and session command extraction

The final Phase 1 implementation slice moved logged-in DOM side-effect
commands from `loggedIn/update.ts` into
`apps/web/src/page/loggedIn/commands/dom.ts`: timeline scroll, composer focus,
thread URL replacement, and account-menu outside-click installation. The
commands remain re-exported from `loggedIn/update.ts` for existing tests and
callers.

The same slice moved session persistence commands from
`apps/web/src/command.ts` into `apps/web/src/commands/session.ts`.
`apps/web/src/command.ts` remains the stable public command barrel for existing
call sites, while the session module owns the named `sessionStoreLayer` and the
`withSessionStore` provider boundary for `SaveSession` and `ClearSession`.

Focused tests now execute the store-level session effects against
`KeyValueStore.layerMemory`, asserting that session save writes the encoded auth
session and session clear removes it. The Foldkit commands continue to provide
the named live browser `sessionStoreLayer` at runtime.

No production contract or invariant changed. Phase 1 is complete.

### 2026-06-04 scoped sync-client state

The first Phase 2 implementation slice updated `packages/sync-client` from a
flat `collections` record to `collectionsByScope`, paired with
`cursorsByScope`. `applySyncPatch` now applies collection changes only under
the patch scope, while `collectionsForScope` and `cursorForScope` make missing
scope behavior explicit.

Regression tests cover the same collection and entity ID appearing in separate
workspace and thread scopes, scoped patch merging, scoped delete behavior,
pending mutation removal, and default empty scope reads. `packages/sync-client`
now has a package-local `test` script and is included in the root
`test:packages` command.

No production contract or invariant changed. Phase 2 is complete.

### 2026-06-04 shared token usage decoder

The next Phase 2 implementation slice moved the cross-runtime token usage
compatibility parser into `packages/sync-schema/src/token-usage.ts`.
`workers/api/src/token-usage.ts` now delegates `tokenUsageFromEvent` to the
shared JSON-boundary decoder while preserving its existing
`extractAutopilotTokenUsage` export for Worker tests and callers.

The browser sync projection module now uses the same decoder for event token
totals, providers, and models instead of carrying a second unknown-JSON
traversal implementation. This makes UI run metadata and Worker leaderboard
accounting interpret OpenCode, OpenAI-compatible, Codex JSONL, Anthropic cache,
and Gemini usage payloads through the same compatibility rules.

Regression coverage for those payload forms now lives in
`packages/sync-schema/src/token-usage.test.ts`, and `packages/sync-schema` is
included in the root package test command.

No production contract or invariant changed. The remaining runner-event and
provider payload pieces were completed in the following implementation slices.

### 2026-06-04 shared runner event normalizer

The next Phase 2 implementation slice moved raw runner callback payload
normalization into `packages/sync-schema/src/runner-event.ts`.
`eventFromRunnerPayload` in `workers/api/src/omni-runs.ts` now delegates
sequence, type, summary, status, source, external event ID, and artifact
reference compatibility handling to the shared package while preserving the
Worker-side credential-shaped payload rejection in the existing event storage
boundary.

Regression coverage now lives in
`packages/sync-schema/src/runner-event.test.ts` for complete runner payloads,
partial/defaulted payloads, and non-record payload rejection. The Worker keeps
the same stored `OmniEventRecord` shape and callback response behavior.

No production contract or invariant changed. The remaining provider response
payload decoder piece was completed in the following implementation slice.

### 2026-06-04 shared OpenAI provider response decoders

The final Phase 2 implementation slice moved OpenAI device-code, device-token,
and OAuth token response payload schemas from
`workers/api/src/provider-accounts.ts` into
`packages/provider-account-schema/src/index.ts`. The Worker provider account
service now consumes `decodeOpenAiDeviceCodeResponse`,
`decodeOpenAiDeviceTokenResponse`, and `decodeOpenAiOAuthTokenResponse` from
the shared package, leaving external provider payload validation beside the
provider account public schemas and secret-material policy.

Regression coverage now verifies device-code response trimming, device-token
response decoding, OAuth token response decoding, and rejection of empty token
fields in `packages/provider-account-schema/src/index.test.ts`.

No production contract or invariant changed. Phase 2 is complete.

### 2026-06-04 thread-file repository service seed

The first Phase 3 implementation slice chose thread files as the Worker
vertical slice and added `workers/api/src/thread-files.ts`. The new module
defines a `ThreadFileRepository` Effect service, a typed
`ThreadFileRepositoryError`, a D1-backed live layer, and a
`setThreadFileDownloadEnabled` program.

`handleThreadFileApi` now delegates its PATCH download-visibility write to
that service layer instead of issuing the SQL update inline. The route still
owns authentication, authorization, response shaping, and the final detail
reload for now; subsequent Phase 3 slices should move the rest of the
thread-file read/detail/upload/download flow behind the same service boundary
and then convert route handlers from Promise-shaped functions to typed Effect
programs.

Focused tests in `workers/api/src/thread-file-service.test.ts` cover the D1
repository binding values and service substitution through an injected Layer.

No production contract or invariant changed. Phase 3 is in progress.

### 2026-06-04 thread-file repository extraction

The next Phase 3 implementation slice moved thread-file-owned types and D1
helpers out of `workers/api/src/index.ts` and into
`workers/api/src/thread-files.ts`: row/public file types, object-key and
checksum helpers, file read/list/insert helpers, read/manage authorization
helpers, file-detail assembly, and team message-reference persistence/listing.

Authorization still receives the existing team-membership role reader from the
Worker entrypoint, so the extracted module does not take ownership of the
broader team membership model. Existing thread-file message-reference tests now
target the new module directly.

No production contract or invariant changed. Remaining Phase 3 work is to move
thread-file route handlers to typed Effect programs.

### 2026-06-04 team files Effect route handler

The next Phase 3 implementation slice converted
`handleTeamFilesApi` from an `async` Promise-shaped handler to an
`Effect<Response>` program. The route now performs session lookup, membership
authorization, and `ThreadFileRepository.listTeam` inside one Effect program,
provides the live D1 repository layer at the route boundary, and maps
`ThreadFileRepositoryError` to a JSON 500 response locally for this slice.

The router now returns that Effect directly for `/api/teams/:teamId/files`
instead of wrapping the handler in `Effect.promise`.

No production contract or invariant changed. Remaining Phase 3 work is to
convert the other thread-file handlers to typed Effect programs and centralize
the slice's error-to-response conversion.

### 2026-06-04 thread-file detail route Effect conversion

The next Phase 3 implementation slice converted `handleThreadFileApi` from an
`async` Promise-shaped route handler to an `Effect<Response>` program. The route
now performs session lookup, file lookup, read/manage authorization, PATCH
download visibility updates, post-write reload, and detail assembly inside one
Effect program with the live `ThreadFileRepository` layer provided at the route
boundary.

The slice also introduced a shared local `threadFileRepositoryErrorResponse`
mapper and switched both the team-files route and the thread-file detail route
to use it. The router now returns the thread-file detail Effect directly
instead of wrapping it in `Effect.promise`.

No production contract or invariant changed. Later Phase 3 work added broader
service tests and adopted `effect-cf` for this slice's D1/R2 binding boundary.

### 2026-06-04 thread-file list/upload route Effect conversion

The next Phase 3 implementation slice converted `handleThreadFilesApi` from an
`async` Promise-shaped handler to an `Effect<Response>` program. The route now
performs session lookup, GET list query parsing, team membership authorization,
personal/team repository listing, POST form parsing, file validation, byte
reading, checksum calculation, R2 artifact write, repository insert, and
team-file sync publish inside one Effect program.

The router now returns the `/api/thread-files` route Effect directly. With the
team files, list/upload, detail/PATCH, and download routes converted, the old
`runThreadFileRepository` Promise bridge is no longer needed in `index.ts` and
has been removed.

No production contract or invariant changed. Later Phase 3 work added broader
service tests and adopted `effect-cf` for this slice's D1/R2 binding boundary.

### 2026-06-04 thread-file download route Effect conversion

The next Phase 3 implementation slice converted `handleThreadFileDownloadApi`
from an `async` Promise-shaped handler to an `Effect<Response>` program. The
route now performs session lookup, repository file lookup, read authorization,
download-visibility enforcement, R2 object lookup, and response construction
inside one Effect program with the live `ThreadFileRepository` layer provided
at the route boundary.

The router now returns the download route Effect directly instead of wrapping
the handler in `Effect.promise`. The initial conversion left R2 as an explicit
`Effect.promise` boundary; the follow-up `effect-cf` slice replaces it with the
typed `ThreadFileArtifacts` R2 binding service.

No production contract or invariant changed. Later Phase 3 work added broader
service tests and adopted `effect-cf` for this slice's D1/R2 binding boundary.

### 2026-06-04 thread-file service tests and effect-cf boundary

The final Phase 3 implementation slice expanded
`workers/api/src/thread-file-service.test.ts` with fake-D1 coverage for the
broader thread-file repository flow: team file listing, file insert plus reload,
and detail/reference assembly. The repository tests now cover both D1 write
bindings and read-side public projection behavior through the Effect service.

The planned `@effect/vitest` adoption is intentionally deferred. The current
published `@effect/vitest@0.29.0` declares `effect ^3.21.0`, while OpenAgents product surface is
pinned to `effect 4.0.0-beta.70` to match Foldkit and the `effect-cf@0.13.1`
peer line. Pulling `@effect/vitest` into this repo now would create a
test-framework peer mismatch, so the Phase 3 tests stay on the existing Vitest
harness while still exercising Effect layers directly.

`effect-cf` was evaluated against the completed thread-file service boundary
and then adopted for the slice. `workers/api/src/bindings.ts` defines
`OpenAgentsDatabase` with `D1.Service` for `OPENAGENTS_DB` and
`ThreadFileArtifacts` with `R2.Tag` for `ARTIFACTS`. `ThreadFileRepository`
now exposes `effectCfLayer()`, which is provided from `OpenAgentsDatabase`, and
`thread-file-routes.ts` provides one per-request storage layer from
`WorkerEnvironment`, the D1 tag, the repository service, and the R2 tag.

The upload route now writes artifacts through `ThreadFileArtifacts.put`, and
the download route reads artifacts through `ThreadFileArtifacts.get` with
`Option`-based missing-object handling. R2 operation failures are converted to
explicit thread-file artifact error responses at the route boundary.

Because the `effect-cf` root export includes Worker entrypoint modules that
import Cloudflare's virtual `cloudflare:workers` module, the Worker Vitest
configuration now aliases that virtual module to a small test-only stub and
forces `effect-cf` through Vite's transform pipeline. A new repository test
proves the live thread-file repository can be provided from the
`WorkerEnvironment` plus the typed `OpenAgentsDatabase` D1 binding.

No production contract or invariant changed. Phase 3 is complete.

### 2026-06-04 shared Worker response helpers

The next Phase 4 implementation slice moved common HTTP response constructors
out of `workers/api/src/index.ts` and into
`workers/api/src/http/responses.ts`: redirects, method-not-allowed responses,
server errors, unauthorized/forbidden responses, and no-store JSON responses.

`index.ts` now imports those helpers, and `thread-file-routes.ts` imports the
same response boundary directly instead of receiving basic response helpers via
dependency injection. The thread-file route factory still injects session,
membership, and sync dependencies to avoid a runtime import cycle.

No production contract or invariant changed. Later Phase 4 slices completed
route table extraction and Worker route composition.

### 2026-06-04 typed exact route table and Worker route composer

The final Phase 4 implementation slices added `workers/api/src/http/router.ts`
with typed exact-route entries and replaced the repeated static
`url.pathname === ...` checks in `routeRequest` with one `exactRoutes` table.
Static product, auth, GitHub write, admin sync notify, and programmatic-agent
routes now use the table before delegated route modules run. Route composition
then moved into `workers/api/src/worker-routes.ts`, which owns matching order,
the `/t/:threadId` product page route, `/api/*` sync fallback, asset fallback,
and app-shell fallback.

API regex matching now lives in route modules for team chat, thread files,
provider accounts, sync, and Omni surfaces. The entry module constructs
handlers and route dependencies, then delegates to the Worker route composer.

No production contract or invariant changed. Phase 4 route ownership is
complete.

### 2026-06-04 team chat route module split

The next Phase 4 implementation slice moved team chat route matching out of
`workers/api/src/index.ts` and into `workers/api/src/team-chat-routes.ts`. The
module now owns both `/api/teams/:teamId/chat/messages` and
`/api/teams/:teamId/projects/:projectId/chat/messages` matching, including
malformed path-parameter responses, while delegating to the existing team chat
handler through an injected function.

No production contract or invariant changed. Later Phase 4 slices completed
route table extraction and Worker route composition.

### 2026-06-04 Omni route module split

The next Phase 4 implementation slice moved Autopilot fleet/token leaderboard,
billing, Omni operator, agent-run, and deployment route matching out of
`workers/api/src/index.ts` and into `workers/api/src/omni-routes.ts`. The new
module owns the route matching for `/api/autopilot/...`, `/api/billing/...`,
and `/api/omni/...` run/deployment surfaces while delegating to the existing
handlers through injected functions.

This removes another large regex-heavy route group from `routeRequest` without
changing handler behavior. The admin sync notify route remains in `index.ts`
for now because it is an operator utility route rather than part of the Omni
run/deployment surface.

No production contract or invariant changed. Later Phase 4 slices completed
route table extraction and Worker route composition.

### 2026-06-04 provider account route module split

The next Phase 4 implementation slice moved provider account route matching out
of `workers/api/src/index.ts` and into
`workers/api/src/provider-account-routes.ts`. The module now owns the
`/api/provider-accounts` list route, ChatGPT Codex device-login route matching,
provider grant/health/disconnect routes, and the GitHub write connection
disconnect route.

The route module currently injects existing handler functions from `index.ts`
so behavior stays unchanged while the route chain shrinks. This is an
intermediate Phase 4 step toward domain route modules with local services and
centralized authorization/response conversion.

No production contract or invariant changed. Later Phase 4 slices completed
route table extraction and Worker route composition.

### 2026-06-04 sync route module split

The next Phase 4 implementation slice moved `/api/health` and `/api/sync/...`
routing out of `workers/api/src/index.ts` and into
`workers/api/src/sync-routes.ts`. The new module owns sync path parsing,
snapshot handling, Durable Object stream forwarding, mutation command decoding,
scope mismatch responses, and health responses.

`index.ts` now injects only the session lookup, refreshed-cookie appender, and
sync authorization policy into `makeSyncRoutes`. This keeps agent-run/team
authorization internals in the current Worker module for now while removing
another route group and its regex parser from the main route chain.

No production contract or invariant changed. Later Phase 4 slices completed
route table extraction and Worker route composition.

### 2026-06-04 thread-file route module split

The first Phase 4 implementation slice moved the public thread-file route group
out of `workers/api/src/index.ts` and into
`workers/api/src/thread-file-routes.ts`. The new module owns the
`/api/thread-files`, `/api/thread-files/:fileId`,
`/api/thread-files/:fileId/download`, and `/api/teams/:teamId/files` route
matching and handlers.

The split avoids a runtime import cycle by using a `makeThreadFileRoutes`
factory. `index.ts` injects the existing session, response, membership, and
sync helpers, while the new module owns route-local parsing, repository error
response mapping, thread-file authorization calls, repository layer provision,
and R2 artifact boundaries. `routeRequest` now delegates this route group to a
single `threadFileRoutes.routeThreadFileRequest(...)` call.

No production contract or invariant changed. Later Phase 4 slices completed
route table extraction and Worker route composition.

### 2026-06-04 explicit Autopilot file selection invariant

The Phase 5 file-selection cleanup removed prompt-keyword inference from
`selectedTeamFileIdsForAutopilotPrompt`. Team Autopilot context now includes
uploaded files only when their explicit file IDs are provided by the request or
stored message metadata and those IDs are present in the authorized file list.
Prompt wording such as "pdf", "file", or "attachment" no longer selects hidden
context files by itself.

`workers/api/src/team-autopilot.test.ts` now covers the explicit-ID path and a
negative prompt-wording case. `INVARIANTS.md` records the new repo-level
invariant so future route or UI work keeps file context selection explicit
unless a typed semantic selector is deliberately modeled and tested.

Production behavior changed intentionally: Team Autopilot no longer infers
selected files from prompt keywords. This narrows hidden dispatch context and
aligns the implementation with the workspace semantic-routing guidance.
`teamAutopilotPromptFromBody` is now exported and covered by backend tests for
the bounded `@autopilot` parser forms. Phase 5 is complete.

### 2026-06-04 thread-file route service wiring

The next Phase 3 implementation slice expanded `ThreadFileRepository` with
service methods for file insert, personal/team listing, read-by-ID, detail
assembly, message-reference persistence/listing, and download visibility
updates. The public thread-file API route handlers now run those service
methods through the D1-backed live layer instead of calling SQL helpers
directly for list, upload insert, detail read/reload, download read, and
download-visibility writes.

The extracted SQL helpers remain available for the team-autopilot context path
and tests. Later Phase 3 slices converted the public thread-file route handlers
to typed `Effect<Response>` programs, centralized the slice's repository and
artifact error-to-response handling, and moved the live D1/R2 binding boundary
onto `effect-cf` services.

No production contract or invariant changed. Phase 3 is complete.

### 2026-06-04 Effect beta 70 and effect-cf audit update

OpenAgents product surface now pins `effect`, `@effect/platform-browser`, and workspace package
Effect dependencies to `4.0.0-beta.70`. This matches the current
`effect-cf@0.13.1` peer dependency line and means Worker slices can evaluate
`effect-cf` directly without a separate Effect-version migration first.

The audit recommendation is now partially implemented rather than only
architectural. `effect-cf` has been installed for the Worker package and the
thread-file vertical slice uses typed D1/R2 binding services. The remaining
`effect-cf` migration is to extend the same service/layer pattern to other
Worker domains, then adopt typed KV, queue, Durable Object, and
`WorkerContext.waitUntil` boundaries as those domains leave `index.ts`.

### 2026-06-04 continuation: UI, logged-in state, and Worker cookie boundaries

The next continuation slice reduced additional audit pressure without changing
public behavior.

`apps/web/src/ui/index.ts` is now a six-line barrel that exports component
families from `ui/shared.ts`, `ui/forms.ts`, `ui/layout.ts`,
`ui/navigation.ts`, `ui/data-display.ts`, and `ui/page-examples.ts`. The
existing implementation moved to `ui/registry.ts` so current imports keep
working while future slices can move implementation families out behind the
new category boundaries.

`apps/web/src/page/loggedIn/chatState.ts` now owns logged-in team chat and
thread-file state helpers: personal/team thread IDs, file scope keys, route
request derivation, team chat room keys, sync collection decoding, team sync
application, and thread-file replacement helpers. `loggedIn/update.ts`
preserves its exported helper surface for existing callers but no longer owns
that domain cluster.

`workers/api/src/auth-cookies.ts` now owns session cookie constants, parsing,
serialization, expiry, refreshed-session cookie appending, and session-cookie
clearing. `workers/api/src/index.ts` imports and re-exports the existing
session-cookie test surface while shedding the inline cookie implementation.

No production contract or invariant changed. Remaining audit work still
includes moving more Worker domain handlers and persistence services out of
`index.ts`, extending `effect-cf` beyond the thread-file slice, splitting the
`ui/registry.ts` implementation itself, tightening remaining unknown-JSON
decoders, and continuing to shrink the Foldkit update modules by domain.

### 2026-06-04 continuation: decoder and boundary extraction

The next continuation slice closed more of the remaining concrete audit
recommendations.

`apps/web/src/subscriptions.ts` now decodes WebSocket frames through the shared
`ServerMessage` schema from `packages/sync-schema` using
`Schema.fromJsonString(...)` and `Schema.decodeUnknownExit(...)`. The callback
no longer performs raw `JSON.parse` plus synchronous schema throws. Focused
tests cover valid sync patches, cursor gaps, and invalid frame failures.

`workers/api/src/json-boundary.ts` now owns record guards, nested unknown
lookup, optional primitive coercion, safe JSON-record parsing, JSON body
reading, and request selector merging. `workers/api/src/index.ts` imports that
boundary instead of defining another ad hoc parsing surface inline.

`workers/api/src/auth/openauth-storage.ts` now owns the OpenAuth D1
`StorageAdapter`, matching the audit's requested `auth/openauth-storage.ts`
module split. The Worker entrypoint now imports that adapter rather than
carrying storage SQL inline.

The UI registry split now includes real implementation movement, not only
barrel re-exports. Shared UI primitives live in `apps/web/src/ui/primitives.ts`,
form controls live in `apps/web/src/ui/forms.ts`, and the alert feedback
component lives in `apps/web/src/ui/feedback.ts`. `ui/registry.ts` remains as
a compatibility export surface while implementation families continue moving
out behind stable category modules.

No production contract or invariant changed. Remaining audit work is now
concentrated in the broad domain-service migration: moving team chat,
provider-account, billing, auth/session, sync-notifier, and Omni handlers out
of `workers/api/src/index.ts`; extending `effect-cf` and typed service layers
to those domains; finishing UI family implementation splits; and standardizing
time/ID generation behind injectable services.

### 2026-06-04 continuation: team-chat repository and table display split

The next continuation slice moved team-chat persistence and row projection out
of `workers/api/src/index.ts` into `workers/api/src/team-chat.ts`. The module
now owns team chat message/run-summary types, public message projection,
message list/read/insert/update SQL helpers, agent-run message lookup, and
Autopilot launch-error response extraction. `index.ts` imports that boundary
instead of carrying the D1 queries inline.

The team-chat repository helpers now accept optional injected clock and UUID
generators while preserving live defaults, and the team-chat route uses named
message/thread ID helpers instead of allocating UUIDs inline. Focused tests in
`workers/api/src/team-chat.test.ts` cover strict run-summary metadata
projection, deterministic ID construction, insert/reload bindings, and
run-summary metadata merge behavior with deterministic time/ID inputs.

The UI implementation split also moved `tableList` into
`apps/web/src/ui/data-display.ts`. `ui/index.ts` continues to expose the
component through the category barrel, while `ui/registry.ts` sheds another
implementation family.

No production contract or invariant changed. Remaining audit work is still
the broad domain-service migration: moving provider-account, billing,
auth/session, sync-notifier, and Omni handlers out of `workers/api/src/index.ts`;
extending `effect-cf` and typed service layers beyond thread files; finishing
the UI family implementation split; and continuing to standardize direct
time/ID generation behind injectable services or Effect layers.

### 2026-06-04 continuation: sync notifier boundary

The next continuation slice moved sync notification and publication helpers out
of `workers/api/src/index.ts` into `workers/api/src/sync-notifier.ts`. The new
module owns agent-run sync scope projection, Durable Object scope notification,
team chat sync publication, team thread-file sync publication, and agent-run
scope notification after billing, launch, event, and status transitions.

`index.ts` re-exports the existing sync publication helpers for compatibility,
but the focused team sync publication test now imports the notifier module
directly. This completes the concrete `SyncNotifier` module split called out in
the audit while preserving the current outbox and Durable Object notification
contracts.

No production contract or invariant changed. Remaining audit work is still
provider-account, billing, auth/session, and Omni handler extraction from the
Worker entrypoint; broader `effect-cf` service/layer adoption beyond thread
files; finishing the UI implementation-family split; and moving remaining
direct time/ID generation behind injectable services or Effect layers.

### 2026-06-04 continuation: team repository boundary

The next continuation slice moved team-owned read models and D1 helpers out of
`workers/api/src/index.ts` into `workers/api/src/team-repository.ts`. The new
module owns team roles, membership/project/team public read types, project
agent metadata decoding, active team membership lookup, active project lookup,
and user team listing with member/project hydration.

This completes another concrete Worker domain-repository split from the audit
module map. Focused coverage in `workers/api/src/team-repository.test.ts`
verifies strict project-agent metadata decoding and incomplete metadata
rejection, while existing team chat, team sync, and Team Autopilot tests cover
the call sites that consume the imported repository helpers.

No production contract or invariant changed. Remaining audit work is now
provider-account, billing, auth/session, and Omni handler extraction from the
Worker entrypoint; broader `effect-cf` service/layer adoption beyond thread
files; finishing the UI implementation-family split; and moving remaining
direct time/ID generation behind injectable services or Effect layers.

### 2026-06-04 continuation: operator target repository boundary

The next continuation slice moved Omni operator target-user lookup out of
`workers/api/src/index.ts` into `workers/api/src/operator-targets.ts`. The new
module owns operator target projection, user-id lookup, email/login/GitHub
identity lookup, and selector fallback handling through an explicitly supplied
default identity.

`index.ts` now keeps only the default admin identity choice and calls the
repository helper from Omni operator routes. Focused coverage in
`workers/api/src/operator-targets.test.ts` verifies user-id lookup projection,
identity normalization, and default-identity fallback binding behavior.

No production contract or invariant changed. Remaining audit work is now
provider-account, billing, auth/session, and remaining Omni handler extraction
from the Worker entrypoint; broader `effect-cf` service/layer adoption beyond
thread files; finishing the UI implementation-family split; and moving
remaining direct time/ID generation behind injectable services or Effect
layers.

### 2026-06-04 continuation: provider launch helper boundary

The next continuation slice moved provider-account launch selection and
launch-block explanation helpers out of `workers/api/src/index.ts` into
`workers/api/src/provider-launch.ts`. The new module owns connected account
selection, requested-account matching, reconnect reason projection, latest
provider health-summary lookup, and the user-facing launch-block message used
by user and operator Autopilot launch paths.

Focused coverage in `workers/api/src/provider-launch.test.ts` verifies
connected account selection, requested account matching, reconnect reason
priority, health-summary lookup bindings, and launch-block message assembly.

No production contract or invariant changed. Remaining audit work is now
billing, auth/session, provider-account route/service extraction, remaining
Omni handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving remaining direct time/ID generation behind injectable
services or Effect layers.

### 2026-06-04 continuation: billing API handler boundary

The next continuation slice moved browser billing API handlers out of
`workers/api/src/index.ts` into `workers/api/src/billing-routes.ts`. The new
factory owns summary, coupon redemption, and placeholder checkout handlers
while receiving browser-session lookup and refreshed-cookie response handling
as injected boundaries from the Worker entrypoint.

Focused coverage in `workers/api/src/billing-routes.test.ts` verifies
unauthorized responses, coupon-code-required responses with refreshed session
cookies, and placeholder checkout payloads.

No production contract or invariant changed. Remaining audit work is now
auth/session, provider-account route/service extraction, operator billing and
remaining Omni handler extraction from the Worker entrypoint, broader
`effect-cf` service/layer adoption beyond thread files, finishing the UI
implementation family split, and moving remaining direct time/ID generation
behind injectable services or Effect layers.

### 2026-06-04 continuation: auth/session boundary

The next continuation slice extracted the reusable browser session boundary
from the Worker entrypoint into `workers/api/src/auth/session.ts`. The new
module owns the route-facing operations that were previously ad hoc helpers in
`index.ts`: require a verified browser session and persist its user, then append
refreshed session cookies when OpenAuth verification returned new tokens.
`index.ts` still owns concrete OpenAuth verification and GitHub user upsert
details, but route modules now consume the same small boundary shape.

Focused coverage in `workers/api/src/auth/session.test.ts` verifies both the
failed verification path and the successful persist-and-return path. Existing
Worker tests continue to cover cookie refresh behavior through integrated
routes.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving remaining direct time/ID generation behind injectable
services or Effect layers.

### 2026-06-04 continuation: billing runtime time/ID boundary

The next continuation slice moved billing time and ID generation behind an
injectable runtime boundary. `workers/api/src/billing.ts` now exports
`BillingRuntime` and `systemBillingRuntime`, and billing operations accept that
runtime as an optional dependency while preserving the existing production
signatures. Direct `new Date()` and `crypto.randomUUID()` allocation in the
billing module is now isolated to the default runtime implementation rather
than spread through account creation, manual credits, coupon redemption,
notifications, Codex debits, and container-usage debits.

Focused billing and billing-route tests still pass with the default runtime,
and the new shape is ready to be supplied by a future `ClockService` /
`IdGenerator` Effect layer without changing route contracts.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving the remaining direct time/ID generation in
provider-account, GitHub-write, thread-file, token-usage, sync-worker, and
OpenAuth storage modules behind injectable services or Effect layers.

### 2026-06-04 continuation: token-usage runtime timestamp boundary

The next continuation slice moved token-usage leaderboard timestamp generation
behind an injectable runtime boundary. `workers/api/src/token-usage.ts` now
exports `TokenUsageRuntime` and `systemTokenUsageRuntime`, and
`readAutopilotTokenLeaderboards` accepts the runtime as an optional dependency
while preserving existing route callers.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving the remaining direct time/ID generation in
provider-account, GitHub-write, thread-file, sync-worker, and OpenAuth storage
modules behind injectable services or Effect layers.

### 2026-06-04 continuation: sync-worker runtime timestamp boundary

The next continuation slice moved sync outbox timestamps behind an injectable
runtime boundary. `packages/sync-worker/src/index.ts` now exports
`SyncWorkerRuntime` and `systemSyncWorkerRuntime`, and
`makeD1SyncOutboxRepository` accepts the runtime as an optional dependency for
change, accepted-mutation, and rejected-mutation timestamps.

Focused coverage in `packages/sync-worker/src/index.test.ts` now asserts
deterministic timestamps for appended changes and rejected mutations through
the injected runtime.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving the remaining direct time/ID generation in
provider-account, GitHub-write, thread-file, and OpenAuth storage modules
behind injectable services or Effect layers.

### 2026-06-04 continuation: OpenAuth storage runtime boundary

The next continuation slice moved OpenAuth D1 storage time reads behind an
injectable runtime boundary. `workers/api/src/auth/openauth-storage.ts` now
exports `OpenAuthStorageRuntime` and `systemOpenAuthStorageRuntime`, and
`makeD1Storage` accepts the runtime as an optional dependency for expiry checks,
scan filtering, and `updated_at` timestamps.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving the remaining direct time/ID generation in
provider-account, GitHub-write, and thread-file modules behind injectable
services or Effect layers.

### 2026-06-04 continuation: thread-file runtime and route ID boundary

The next continuation slice moved thread-file repository timestamps and
message-reference IDs behind an injectable `ThreadFileRuntime`, and moved upload
route file-ID generation behind an optional `makeThreadFileId` route
dependency. The remaining `new Date()` and `crypto.randomUUID()` calls in
thread-file modules are now default runtime/dependency implementations rather
than direct business-logic allocations.

Focused coverage in `workers/api/src/thread-file-service.test.ts` now supplies
the runtime through `makeD1ThreadFileRepository`, and the thread-file route and
storage tests continue to pass.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, finishing the UI implementation
family split, and moving the remaining direct time/ID generation in
provider-account and GitHub-write modules behind injectable services or Effect
layers.

### 2026-06-04 continuation: provider, GitHub-write, Omni, and Worker runtime boundaries

The next continuation slice moved the remaining provider-account,
GitHub-write, Omni-run, and Worker-entrypoint time/ID allocations behind named
runtime defaults. `workers/api/src/provider-accounts.ts` now exports
`ProviderAccountRuntime` and `systemProviderAccountRuntime`;
`workers/api/src/github-write-connections.ts` now exports `GitHubWriteRuntime`
and `systemGitHubWriteRuntime`; `workers/api/src/omni-runs.ts` now exports
`OmniRunRuntime` and `systemOmniRunRuntime`; and `workers/api/src/index.ts`
uses a single local Worker runtime object for its residual timestamp and UUID
needs.

Focused Worker coverage passed for provider accounts, GitHub-write
connections, Omni runs, and auth/admin access. The remaining raw `new Date()`
and `crypto.randomUUID()` call sites in these modules are default runtime
implementations or pure date arithmetic/formatting helpers, not scattered
business-logic allocations.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, broader `effect-cf`
service/layer adoption beyond thread files, and finishing the UI
implementation family split.

### 2026-06-04 continuation: UI feedback/data-display implementation split

The next continuation slice moved two remaining UI implementation families out
of the backing registry file. `emptyState` now lives in
`apps/web/src/ui/feedback.ts`, and `statsTimeline` now lives in
`apps/web/src/ui/data-display.ts`; `registry.ts` no longer owns those
implementations. `page-examples.ts` now re-exports `statsTimeline` from the
data-display family instead of the registry.

Focused web typecheck and UI coverage/view tests passed.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, operator billing and remaining Omni
handler extraction from the Worker entrypoint, and broader `effect-cf`
service/layer adoption beyond thread files.

### 2026-06-04 continuation: operator billing handler extraction

The next continuation slice moved the operator manual-credit endpoint out of
the Worker entrypoint and into `workers/api/src/operator-billing-routes.ts`.
The new handler factory owns method validation, admin authorization handling,
selector parsing, positive-credit validation, idempotency-key construction, and
manual billing credit application, while receiving admin-token verification and
operator target lookup as injected boundaries.

Focused Worker typecheck and billing/admin route tests passed.

No production contract or invariant changed. Remaining audit work is now
provider-account route/service extraction, remaining Omni handler extraction
from the Worker entrypoint, and broader `effect-cf` service/layer adoption
beyond thread files.

### 2026-06-04 continuation: provider-account browser handler extraction

The next continuation slice moved the browser provider-account list and
disconnect handlers out of the Worker entrypoint and into
`workers/api/src/provider-account-browser-routes.ts`. The new factory owns the
GET list endpoint and POST disconnect endpoint while receiving browser-session,
refreshed-cookie, and provider auth-secret key boundaries from `index.ts`.

Focused Worker typecheck and provider-account tests passed.

No production contract or invariant changed. Remaining audit work is now the
provider-account device-login/grant callback handler extraction, remaining Omni
handler extraction from the Worker entrypoint, and broader `effect-cf`
service/layer adoption beyond thread files.

### 2026-06-04 continuation: provider-account browser device/grant extraction

The next continuation slice moved additional browser provider-account handlers
out of the Worker entrypoint and into
`workers/api/src/provider-account-browser-routes.ts`: ChatGPT/Codex device-login
start, device-login status refresh, and session-scoped provider grant issue.
The module now owns browser-facing provider-account list, disconnect,
device-login start/status, and grant issue endpoints. KV helpers for pending
device login and connected auth storage are injected from `index.ts`.

Focused Worker typecheck and provider-account tests passed.

No production contract or invariant changed. Remaining audit work is now
provider-account service callback/grant-resolve handler extraction, remaining
Omni handler extraction from the Worker entrypoint, and broader `effect-cf`
service/layer adoption beyond thread files.

### 2026-06-04 continuation: provider-account service callback extraction

The next continuation slice moved the remaining provider-account service
callbacks out of the Worker entrypoint and into
`workers/api/src/provider-account-service-routes.ts`: device-login connected,
device-login failed, provider-account health, and provider-account grant
resolve. The new factory owns service-actor authorization, callback payload
validation, provider account repository calls, redacted callback error
responses, and optional Codex auth-material inclusion while receiving
programmatic-agent authorization and connected-auth material lookup from
`index.ts`.

Focused Worker typecheck and provider-account tests passed.

No production contract or invariant changed. Remaining audit work is now
remaining Omni handler extraction from the Worker entrypoint and broader
`effect-cf` service/layer adoption beyond thread files. Incident-response
work remains intentionally excluded from this continuation.

### 2026-06-04 continuation: token usage effect-cf service boundary

The next continuation slice extended `effect-cf` adoption beyond thread files.
`workers/api/src/token-usage.ts` now exports a typed
`TokenUsageLeaderboards` Effect service with both direct D1 and `effect-cf`
layer constructors, backed by the existing `OpenAgentsDatabase` binding.
`workers/api/src/index.ts` now reads token leaderboards for authenticated page
context and `/api/autopilot/token-leaderboards` through that service layer
instead of calling the D1 reader directly.

Focused Worker typecheck plus token-usage and provider-account tests passed.

No production contract or invariant changed. Remaining audit work is now
remaining Omni handler extraction from the Worker entrypoint. Incident-response
work remains intentionally excluded from this continuation.

### 2026-06-04 continuation: Omni handler extraction complete

The final non-incident continuation slice moved the remaining Omni and
Autopilot handler implementations out of the Worker entrypoint and into
`workers/api/src/omni-handlers.ts`. The new handler factory owns Autopilot
fleet/token leaderboard APIs, operator fleet/team-chat/agent-run/deployment
APIs, browser mission/deployment APIs, runner event ingestion handlers, mission
launch assembly, SHC dispatch logging, provider-account grant selection, GitHub
write grant selection, and run-summary construction. `workers/api/src/index.ts`
now constructs the factory with explicit boundaries for browser session,
programmatic actor auth, admin auth, token leaderboards, team-chat posting,
sync access, and billing-aware run storage, then passes the extracted handlers
into `makeOmniRoutes`.

No production contract or invariant changed. With incident-response work
explicitly excluded per operator direction, the audit's remaining
recommendations are implemented.

### 2026-06-04 final non-incident recommendation reconciliation

The final reconciliation pass audited the original recommendation checklist
against the current codebase rather than relying only on the latest remaining
work note. Phase 6 is now explicitly marked complete: route startup policy
lives in `apps/web/src/routing/startup.ts`; thread route state lives in
`apps/web/src/page/loggedIn/thread-route.ts`; `syncScopesForModel` requires
authorized thread-route state before opening `thread:<id>` streams;
`ThreadAccessService` centralizes product page, sync, and agent-run detail
authorization; and the relevant startup, subscription, main, scene, sync
access, and thread-access tests are present and wired into the deploy gate.

The executive summary was also reconciled so it no longer describes the
intentionally incremental `effect-cf` rollout as unfinished same-audit work.
With incident-response work excluded per operator direction, all audit
recommendations are now recorded as implemented.
