# OpenAgents Zero-Tech-Debt Caller Inventory

Date: 2026-06-04

Source issue: <https://github.com/OpenAgentsInc/openagents/issues/18>

Source audit:
`docs/2026-06-04-openagents-broader-effect-refactor-audit.md`

Purpose: identify the compatibility paths that must not be preserved by
default while OpenAgents moves toward the fully Effect-native end state. This
inventory is based on direct `rg` caller evidence gathered on 2026-06-04.

Executable guardrail: `bun run check:architecture`, backed by
`scripts/check-zero-debt-architecture.mjs`, enforces the current budgets for
the debt categories listed below. The production-source scan excludes test
files and `*.test-support.ts` Vitest helper modules so conformance fixtures do
not weaken production Worker budgets.

## Caller Evidence Commands

These commands were used to build this pass:

```sh
rg -n "Effect\.promise|Effect\.runPromise|runPromise\(" workers packages apps/web/src --glob '!**/*.test.ts'
rg -n "makeD1SyncOutboxRepository|SyncOutboxRepository" workers/api/src packages --glob '!**/*.test.ts'
rg -n "from './registry'|export \* from './registry'|export type .* from './registry'" apps/web/src/ui --glob '!registry.ts'
rg -n "legacyAgentRunUuid|legacyAgentRunIdFromUuid|legacyChatRouter|fallbackThreadId|fallbackBackend|projectRef|projects\?\.|teamProjectRouteRef|teamProjectChatRouter|teamChatRouter" apps/web/src workers/api/src packages --glob '!**/*.test.ts'
rg -n "message\.includes|credential-shaped|does not match|providerCallbackErrorStatus" workers/api/src --glob '!**/*.test.ts'
rg -n "return (methodNotAllowed|unauthorized|forbidden|notFound|badRequest)\(|Effect\.succeed\((methodNotAllowed|unauthorized|forbidden|notFound|badRequest)\(" workers/api/src --glob '!**/*.test.ts'
rg -n "S\.NullOr|null \| undefined|: string \| null|: .* \| null" apps/web/src/domain apps/web/src/page/loggedIn workers/api/src packages --glob '!**/*.test.ts'
rg -n "env\.(GITHUB_CLIENT_ID|GITHUB_CLIENT_SECRET|OPENAGENTS_ADMIN_API_TOKEN|OPENAGENTS_APP_URL|OPENAUTH_CLIENT_ID|OPENAUTH_ISSUER_URL|RESEND_API_KEY|RESEND_FROM_EMAIL|RESEND_REPLY_TO_EMAIL|SHC_CONTROL_API_BEARER_TOKEN|SHC_CONTROL_API_URL|SHC_DISPATCH_MODE|SHC_RUNNER_CALLBACK_TOKEN)" workers/api/src --glob '!**/*.test.ts'
```

Summary counts from this pass:

- `Effect.promise`: `workers/api/src/omni-routes.ts` 17,
  `provider-account-routes.ts` 10, `team-chat-routes.ts` 2,
  `worker-routes.ts` 3, `sync-routes.ts` 3, `index.ts` 14,
  `thread-file-routes.ts` 15, `thread-access.ts` 4.
- `Effect.runPromise` below production boundaries:
  `workers/api/src/index.ts` 4, `sync-routes.ts` 2,
  `thread-access.ts` 1, `onboarding/repository.ts` 1.
- Parent logged-in command definitions:
  `apps/web/src/page/loggedIn/update.ts` 19.
- Separate DOM command definitions:
  `apps/web/src/page/loggedIn/commands/dom.ts` 4.

## Deleted In This Pass

| Deleted path                                            | Evidence                 | Reason | Guardrail                                                                                                                                                                                 |
| ------------------------------------------------------- | ------------------------ | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Browser `/chat` parser alias in `apps/web/src/route.ts` | `rg -n "legacyChatRouter | /chat  | chatRouter\(" apps/web/src workers/api/src docs`showed no generated browser links to`/chat`; app links use `chatRouter()`, which maps root, while Worker keeps a tested `/chat` redirect. | Production compatibility belongs at the Worker redirect boundary. The browser should not preserve the old route alias. | Added `apps/web/src/route.test.ts` asserting `urlToAppRoute('/chat')` returns `NotFoundRoute`. Worker redirect remains covered by `workers/api/src/admin-access.test.ts`. |

## Completed Config Boundary

Issue #23 moved the migrated secret/config fields from raw `Env` reads into
`workers/api/src/config.ts`:

- `GITHUB_CLIENT_ID`
- `GITHUB_CLIENT_SECRET`
- `OPENAGENTS_ADMIN_API_TOKEN`
- `OPENAGENTS_APP_URL`
- `OPENAUTH_CLIENT_ID`
- `OPENAUTH_ISSUER_URL`
- `RESEND_API_KEY`
- `RESEND_FROM_EMAIL`
- `RESEND_REPLY_TO_EMAIL`
- `SHC_CONTROL_API_BEARER_TOKEN`
- `SHC_CONTROL_API_URL`
- `SHC_DISPATCH_MODE`
- `SHC_RUNNER_CALLBACK_TOKEN`

Current caller evidence:

```sh
rg -n "env\.(GITHUB_CLIENT_ID|GITHUB_CLIENT_SECRET|OPENAGENTS_ADMIN_API_TOKEN|OPENAGENTS_APP_URL|OPENAUTH_CLIENT_ID|OPENAUTH_ISSUER_URL|RESEND_API_KEY|RESEND_FROM_EMAIL|RESEND_REPLY_TO_EMAIL|SHC_CONTROL_API_BEARER_TOKEN|SHC_CONTROL_API_URL|SHC_DISPATCH_MODE|SHC_RUNNER_CALLBACK_TOKEN)" workers/api/src --glob '!**/*.test.ts'
```

The command is expected to report no production dot-property reads. The config
boundary reads by typed key through `workers/api/src/config.ts`, and the
architecture guardrail `direct migrated Worker config Env reads` enforces a
zero budget outside that boundary.

## Runtime Capability Boundary

Issue #24 moved production Worker runtime capability reads behind
`workers/api/src/runtime.ts`. Route and handler modules now use:

- `scheduleBackgroundWork` / `OpenAgentsWorkerContext` for background work
  instead of direct `ctx.waitUntil`.
- `RunnerEventsQueue` for Schema-encoded queue producer payloads.
- `OpenAgentsSyncRoomNotifications` / `syncRoomNotifications` for `SYNC_ROOM`
  access from typed sync scopes.
- `openAgentsDatabase` and `syncOutboxStoreLayer` for D1 binding access.

The architecture guardrail `direct Worker runtime capability access` now has a
zero budget for direct production use of `ctx.waitUntil`, `env.OPENAGENTS_DB`,
`env.SYNC_ROOM`, `env.RUNNER_EVENTS`, `this.env.OPENAGENTS_DB`, and
`scopeIdFromName` outside `workers/api/src/runtime.ts`.

## Final Issue #44 State

Issue #44 completed the final same-audit deletion/tightening pass that was not
blocked by active callers or upstream package peer metadata.

Deleted in #44:

- Removed the local Foldkit `LoginRoute` / `loginRouter` parser surface. The
  real public login surface is root plus the `/login/github` document
  navigation.
- Deleted `apps/web/src/page/loggedOut/page/login.ts` and
  `apps/web/src/page/loggedOut/page/login.story.test.ts`.
- Removed the logged-out `loginModel`, `GotLoginMessage`, simulated
  email/password auth command, and dead save-session command/message path.
- Changed public "Log in" header links to root.
- Changed Worker `/login` handling to redirect to root instead of serving the
  app shell.
- Changed Worker `/chat` handling from a root redirect alias to not found.

Guardrails tightened in #44:

- `Worker throw new Error calls`: budget tightened from 19 to the current 12.
- `raw Env parameter annotations`: budget tightened from 161 to the current 160.
- `Worker Response return surfaces`: budget tightened from 81 to the current 80.
- Added a zero budget for deleted local login route symbols:
  `LoginRoute`, `loginRouter`, `StartupRedirectToLogin`, `RedirectToLogin`,
  `SimulateAuthRequest`, `SaveSession`, `SucceededSaveSession`, and
  `FailedSaveSession`.
- Added a zero budget for reintroducing the Worker `/chat` redirect-to-root
  alias.
- Added deleted-file guardrails for the local login demo implementation and
  story test.

Explicit remaining exceptions after #44:

- `SyncOutboxRepository` / `makeD1SyncOutboxRepository` remains because current
  Worker callers still use the Promise facade in the Durable Object sync path,
  Omni run publication, and sync notification publication.
- Thread-file route dependency adapters remain the only
  `Effect.promise(() => dependencies.*)` route budget.
- Raw `Env` parameter annotations and Worker `Response` return surfaces remain
  migration budgets until route signatures and HTTP mapping are fully
  Effect-native.
- `Effect.runPromise` remains only on the named allowlist in
  `scripts/check-zero-debt-architecture.mjs`.
- Legacy run ID normalization remains until persisted data no longer requires
  canonicalizing old `agent_run_*` / UUID identifiers.
- The Foldkit Effect beta 66 topology exception remains blocked by package
  metadata; see
  `docs/2026-06-04-openagents-effect-dependency-upgrade-tracker.md`.

## Worker Request Boundary

Issue #25 moved the top-level fetch route composer to a request-scoped Effect
boundary:

- `WorkerRequestLayer` composes `Request`, URL, `Env`, `ExecutionContext`,
  `effect-cf` request/environment/context services, background scheduling,
  sync-room notifications, and the sync outbox D1 layer.
- `makeWorkerRouteRequest` now consumes `OpenAgentsWorkerRequest` from the
  Effect context instead of accepting `(request, env, ctx)` at invocation.
- The public Worker export remains stable and provides the layer from
  `fetch(request, env, ctx)`.

Temporary boundary exceptions:

- Route-group dependencies still accept `(request, env, ctx)` until issue #31
  converts route signatures to Effect-native services.
- Exact routes and app-shell/thread fallbacks still contain Promise adapters
  until issues #31 and #32 move response mapping and route dependencies out of
  `workers/api/src/index.ts` and `workers/api/src/worker-routes.ts`.
- Three non-entrypoint `Effect.runPromise` bridges remain in
  `workers/api/src/index.ts` for thread-file artifact excerpts, token
  leaderboard reads, and thread route access bundles. Delete them when their
  owning services are consumed through request layers and final compatibility
  issue #44 removes the temporary bridge allowlist.

## Public Khala Chat Served-Token Bridge

Date: 2026-06-26

`workers/api/src/khala-chat-routes.ts` has one named `Effect.runPromise`
bridge. Current caller evidence:

```sh
rg -n "Effect\\.runPromise\\(|recordPublicKhalaChatServedTokens|recordServedTokens" workers/api/src/khala-chat-routes.ts workers/api/src/public-khala-chat-served-tokens.ts
```

The caller is the public `/api/khala/chat` SSE path in
`workers/api/src/khala-chat-routes.ts`. It records served-token usage after the
provider stream drains and before emitting the terminal `meta` / `done` frames.
The route callback is Promise-shaped because Web Streams
`ReadableStream.start` is the current boundary, while the shared served-token
recorder helper in `public-khala-chat-served-tokens.ts` is Effect-shaped so the
inference gateway can `yield*` equivalent recorder work.

Replacement: move the public chat stream/finalize path to an Effect Stream
program end-to-end so the route finalizer can `yield*` the Effect-shaped
served-token recorder without a Web Streams Promise bridge.

Deletion condition: remove the `Effect.runPromise` call from
`khala-chat-routes.ts`, then delete its allowlist entry from
`scripts/check-zero-debt-architecture.mjs`.

Guardrail: `bun run check:architecture` budgets exactly one bridge for this
file and fails if the count grows or the entry is removed without deleting the
bridge.

## Historical Kept Compatibility Paths From Initial Pass

The table below records the original caller inventory from the zero-debt pass.
Later issue sections above supersede rows that have since been completed or
deleted.

| ID      | Kept path                                                                                                                                      | Current callers from `rg`                                                                                                                                                                                                         | Intended replacement                                                                                                                                         | Deletion condition                                                                                                                                             | Owning issue / guardrail                            |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| ZTD-001 | `makeOmniRoutes` Promise dependency adapters in `workers/api/src/omni-routes.ts`                                                               | Imported and wired in `workers/api/src/index.ts:90` and `workers/api/src/index.ts:3091`; route function passed to `makeWorkerRouteRequest` at `workers/api/src/index.ts:3204`.                                                    | Omni route dependencies return typed Effect programs or domain values plus route mappers.                                                                    | Delete `Effect.promise(() => dependencies.*)` adapters after Omni services and route signatures are Effect-native.                                             | Issues #29, #30, #31; architecture budget from #20. |
| ZTD-002 | `makeProviderAccountRoutes` Promise dependency adapters in `workers/api/src/provider-account-routes.ts`                                        | Imported at `workers/api/src/index.ts:109`; wired at `workers/api/src/index.ts:3034`; passed to `makeWorkerRouteRequest` at `workers/api/src/index.ts:3205`.                                                                      | Provider-account route dependencies consume provider services returning typed Effect values/errors.                                                          | Delete adapters after provider-account route mappers consume `ProviderAccountError` directly and route signatures become Effect-native.                        | Issues #27, #31.                                    |
| ZTD-003 | `makeTeamChatRoutes` Promise dependency adapters in `workers/api/src/team-chat-routes.ts`                                                      | Imported at `workers/api/src/index.ts:137`; wired at `workers/api/src/index.ts:3116`; passed to `makeWorkerRouteRequest` at `workers/api/src/index.ts:3207`.                                                                      | Team chat route dependency becomes an Effect service call with route error mapping.                                                                          | Delete adapters after route dependency signatures convert.                                                                                                     | Issue #31.                                          |
| ZTD-005 | `makeWorkerRouteRequest` app-shell, asset, and thread-page Promise fallbacks in `workers/api/src/worker-routes.ts`                             | Constructed once in `workers/api/src/index.ts`; now consumes `OpenAgentsWorkerRequest` through `WorkerRequestLayer`, while `handleAppShellPage`, `handleThreadPage`, and `env.ASSETS.fetch` remain Promise fallbacks.             | Worker runtime and route dependencies are Effect services; asset and app-shell behavior is explicit route-layer mapping.                                     | Delete Promise fallbacks after route dependencies and asset/app-shell handling are Effect-native.                                                              | Issues #25, #31, #32.                               |
| ZTD-006 | Exact route Promise adapters in `workers/api/src/index.ts` for home, login, auth, session, admin sync notify, registration, and service routes | `exactRoutes` at `workers/api/src/index.ts:3120`; consumed by `routeExact` through `makeWorkerRouteRequest`.                                                                                                                      | Exact route handlers return Effect values and route errors through service-owned modules.                                                                    | Delete `Effect.promise` wrappers from `exactRoutes` after each handler moves behind an Effect route/service boundary.                                          | Issues #23, #24, #25, #31, #32.                     |
| ZTD-007 | `SyncOutboxRepository` Promise facade and `makeD1SyncOutboxRepository` in `packages/sync-worker/src/index.ts`                                  | Still used by the SyncRoom Durable Object replay path, Omni run sync publication, and team sync notification publication; sync routes now consume `SyncOutboxStore` through `syncOutboxStoreLayer`.                               | `SyncOutboxStore` now exists as the D1-backed Effect service with typed errors, stored-payload boundary decoders, and an effect-cf-compatible layer builder. | Keep the one-call `Effect.runPromise` facade only while listed callers remain; delete it after all callers use `SyncOutboxStore` directly.                     | Issues #21, #24, #41.                               |
| ZTD-008 | `ThreadAccessService.runPromise` bridge in `workers/api/src/thread-access.ts`                                                                  | `rg` shows production `Effect.runPromise` at `workers/api/src/thread-access.ts:196`; thread/file route callers use thread access results.                                                                                         | Route access dependencies stay in Effect until route mappers convert errors to HTTP.                                                                         | Delete bridge after thread-file routes consume `ThreadAccessService` directly through layers.                                                                  | Issues #20, #31, #32.                               |
| ZTD-009 | Thread-file route Promise adapters and direct response helpers in `workers/api/src/thread-file-routes.ts`                                      | Passed as `routeThreadFileRequest` into `makeWorkerRouteRequest`; `rg` shows 15 `Effect.promise` adapters and many direct `unauthorized`, `forbidden`, `notFound`, and `methodNotAllowed` returns.                                | Thread-file service methods return typed values/errors; route mapper owns HTTP.                                                                              | Delete adapters and response helpers after thread-file route/service split.                                                                                    | Issues #31, #32, #41.                               |
| ZTD-010 | UI registry compatibility facade in `apps/web/src/ui/registry.ts` and `ui/index.ts`                                                            | `ui/index.ts` still exports `./registry`; `ui/layout.ts`, `ui/data-display.ts`, `ui/shared.ts`, `ui/navigation.ts`, and `ui/page-examples.ts` import from `./registry`.                                                           | Final component families live in `ui/layout.ts`, `ui/workroom.ts`, `ui/public.ts`, `ui/data-display.ts`, `ui/feedback.ts`, `ui/forms.ts`, and `ui/index.ts`. | Delete `ui/registry.ts` after callers move to final modules or the stable barrel.                                                                              | Issue #38; UI registry budget from #20.             |
| ZTD-011 | Parent logged-in command matrix in `apps/web/src/page/loggedIn/update.ts`                                                                      | `rg -c "Command.define"` reports 19 definitions in the parent update; handlers in the same file return commands such as `LoadTeamChatMessages`, `LaunchAutopilotRun`, and `FetchAutopilotRun`.                                    | Domain command modules under providers, billing, thread files, team chat, onboarding, and runs; parent update dispatches and composes results.               | Delete parent-owned request command definitions after domain command modules own them.                                                                         | Issues #34, #35; parent-update budget from #20.     |
| ZTD-013 | Generic Worker response helpers returned from domain-heavy modules                                                                             | `rg` shows direct helper returns in `index.ts`, `omni-handlers.ts`, `thread-file-routes.ts`, `billing-routes.ts`, `provider-account-*-routes.ts`, `sync-routes.ts`, and `onboarding/routes.ts`.                                   | Domain services return typed values/errors; route modules map those errors once.                                                                             | Delete helper returns from service/domain modules after route-group error mappers exist.                                                                       | Issue #32; response-return guardrail from #20.      |
| ZTD-014 | Project workroom feature gate and duplicated project routing branches                                                                          | `PROJECT_WORKROOMS_ENABLED` in `apps/web/src/page/loggedIn/model.ts`; branches in `model.ts`, `view.ts`, `chatState.ts`, `page/chat.ts`, route definitions, and Worker team/project APIs.                                         | One typed feature/permission/route gating owner; views consume the central decision.                                                                         | Remove duplicated project visibility branches after centralized gates exist.                                                                                   | Issue #39.                                          |
| ZTD-015 | Legacy run ID route aliases and stored ID normalization                                                                                        | `legacyAgentRunIdFromUuid` used by `workers/api/src/thread-access.ts` and `workers/api/src/omni-handlers.ts`; browser `legacyAgentRunUuid` in `apps/web/src/page/loggedIn/sync/projection.ts`; tests assert legacy normalization. | One route/run identity boundary that maps persisted IDs and public route IDs explicitly.                                                                     | Delete alias functions after persisted data no longer requires legacy `agent_run_*` IDs or a migration proves all callers use canonical IDs.                   | Issues #29, #36, #37, #41.                          |
| ZTD-016 | Worker `/chat` redirect compatibility route                                                                                                    | `workers/api/src/index.ts:3127` exact route redirects `/chat` to `/`; covered by `workers/api/src/admin-access.test.ts`.                                                                                                          | Canonical personal chat is `/`; old `/chat` is accepted only as a Worker redirect.                                                                           | Delete the Worker redirect only after product policy says old external `/chat` links no longer need a clean redirect.                                          | Issue #39 or final deletion issue #44.              |
| ZTD-017 | Logged-out local login route model/view                                                                                                        | `apps/web/src/page/loggedOut/model.ts`, `message.ts`, `update.ts`, and `view.ts` import `./page/login`; `apps/web/src/route.ts` still defines `LoginRoute`; Worker has `/login` exact route.                                      | One public login surface at root/Worker auth boundary, with logged-out product routes redirecting through startup policy.                                    | Delete the logged-out local login submodel after `/login` and logged-out startup policy no longer reference it and tests are moved to the final login surface. | Issue #39 or final deletion issue #44.              |
| ZTD-018 | Long-lived browser model DTO nulls                                                                                                             | `apps/web/src/domain/session.ts` exposes many `S.NullOr` fields; logged-in update/message carries `teamId: S.NullOr(S.String)`; views branch on optional team/project/provider metadata.                                          | DTO-to-model conversion functions that convert boundary nulls into `Option` or tagged state.                                                                 | Delete null-preserving model paths after chat timeline and thread-file models convert.                                                                         | Issue #37.                                          |
| ZTD-019 | Raw JSON and unknown sync/provider/run payload parsing                                                                                         | `packages/sync-worker/src/index.ts` has `parseJsonOrUndefined`; Worker provider/Omni code parses metadata/payload JSON; token usage parsing works from nullable payload JSON.                                                     | Named Schema boundary decoders with typed decode errors.                                                                                                     | Delete raw parsing helpers after boundary decoders cover stored payloads and callbacks.                                                                        | Issue #41.                                          |
| ZTD-020 | Raw time/ID primitives in business logic                                                                                                       | `packages/sync-worker/src/index.ts` has `systemSyncWorkerRuntime.nowIso = () => new Date().toISOString()`; additional raw time/ID use will be enumerated by issue #40.                                                            | `Clock`, `Random`, `Effect.uuid`, or OpenAgents-owned runtime services injected through layers.                                                                   | Delete raw primitive calls after deterministic services exist.                                                                                                 | Issue #40; runtime guardrail from #20.              |

## Inventory Maintenance Rules

- Any issue that deletes one of the paths above must update this document in
  the same commit.
- Any issue that introduces a temporary facade must add a row with current
  callers, final replacement, deletion condition, and guardrail.
- Once a category is complete, replace its migration budget with a
  zero-allowed architecture test and move its row to the deleted section.
- Do not add new compatibility rows for paths with no current callers. Delete
  those paths instead.

## Addendum: Forward-Work Enforcement

Date: 2026-06-04

Answer to "do agents now know the best practices?": yes, with two caveats.
The repo-level `AGENTS.md` now points future agents to this audit before they
touch the zero-tech-debt surfaces, and `bun run check:architecture` enforces
the hard rules through `scripts/check-zero-debt-architecture.mjs`. The caveats
are that remaining migration budgets still exist for active callers, and any
new best practice that cannot be mechanically checked must be added to
`AGENTS.md`, this audit, or `INVARIANTS.md` in the same change that depends on
it.

This audit is therefore a forward contract, not only a cleanup record. New
work must either use the end-state pattern immediately or update this document
with current callers, final replacement, deletion condition, owning issue, and
an executable guardrail. Hidden compatibility paths are not allowed.

Current hard-enforced cleanup categories:

| Cleanup category | Forward rule | Enforcement |
| --- | --- | --- |
| Route dependency Promise adapters | Do not add `Effect.promise(() => dependencies.*)` adapters. New route dependencies should be typed Effect services or typed route dependencies already behind a migration row. | `route dependency Effect.promise adapters` budget in `check:architecture`. |
| `Effect.runPromise` bridges | Keep bridges only on the named temporary allowlist. New bridges are not allowed unless the audit records why they are temporary and how they will be deleted. | `Effect.runPromise temporary bridge allowlist`. |
| Generic Worker thrown errors | Model expected failures as tagged errors and map them at route boundaries. | `Worker throw new Error calls` budget. |
| Provider-account and GitHub-write classification | Map tagged domain errors, not English `message.includes(...)` strings. | `provider-account string error classifiers` and `GitHub-write string error classifiers`, both zero budget. |
| Stored JSON and external payloads | Decode through named Schema boundary decoders. Do not put raw `JSON.parse` in domain logic. | `raw JSON.parse outside json-boundary`, zero budget. |
| Time, UUID, and randomness | Use runtime primitive helpers, injected services, `Clock`, `Random`, or `Effect.uuid`; do not call raw primitives in business logic. | `raw time/id/random primitives`, zero budget outside boundary files. |
| Worker config | Consume migrated settings through `OpenAgentsWorkerConfig`; do not read migrated secret/config Env properties directly. | `direct migrated Worker config Env reads`, zero budget. |
| Worker runtime capabilities | Use runtime services for background work, D1, queues, and sync room access. Do not directly reach for `ctx.waitUntil`, runtime Env bindings, or `scopeIdFromName` outside the boundary. | `direct Worker runtime capability access`, zero budget. |
| Worker logging | Use redacted Effect observability helpers, not raw Worker `console.*`. | `raw Worker console logging`, zero budget. |
| Service/domain HTTP mapping | Service and domain modules return typed values/errors; HTTP helpers stay in route/HTTP modules. | `service/domain HTTP response helper usage`, zero budget. |
| Worker Response surfaces and raw Env annotations | Do not grow route/domain response annotations or raw Env parameter annotations. When cleanup removes one, lower the budget in the same commit. | `Worker Response return surfaces` and `raw Env parameter annotations` budgets. |
| Browser model/policy conversions | Converted browser domains branch on tagged state or `Option`; permission decisions go through `product-policy.ts`. | `converted browser domain raw null branches` and `direct browser Core Team permission checks`, both zero budget. |
| Project workroom visibility | Keep project workroom visibility centralized in `product-policy.ts`; do not reintroduce local feature flags. | `legacy project workroom flag`, zero budget. |
| Deleted login and route aliases | Do not restore local login route symbols, simulated auth, deleted login demo files, or the Worker `/chat` redirect-to-root alias. | `deleted local login route symbols`, `legacy Worker /chat redirect alias`, and deleted-file checks. |
| Logged-in parent update | Keep `loggedIn/update.ts` as a dispatcher. Domain command definitions and direct model mutations belong in domain modules. | `loggedIn/update.ts` line, `Command.define`, and `evo` checks. |
| Chat rendering/projection split | Keep chat rendering focused; runner payload parsing belongs in typed projection modules. | `loggedIn/page/chat.ts` line budget and `JSON.parse` zero budget. |
| UI registry compatibility | UI implementations live in typed family modules and stable barrels. Do not recreate `apps/web/src/ui/registry.ts`. | Deleted-file guardrail for `apps/web/src/ui/registry.ts`. |

Forward implementation guidelines for agents:

- Start from the typed end state: `Context.Service` contracts, `Layer`
  implementations, named `Effect.fn` operations, Schema DTOs, and
  `Schema.TaggedErrorClass` error variants.
- Keep external systems at explicit boundaries: config, runtime capabilities,
  JSON decoding, provider HTTP clients, email providers, sync rooms, queues,
  D1, R2, and browser navigation each need a named boundary service or helper.
- Keep HTTP mapping at the route edge. If a lower-level module wants to return
  `Response`, first decide whether it is actually a route module; otherwise
  return a typed value/error and add or reuse a route mapper.
- Do not add compatibility for old URLs, IDs, auth flows, UI registries, or
  provider callback shapes without caller evidence. If there are no current
  callers, delete the compatibility path instead.
- When an existing migration budget decreases, tighten
  `scripts/check-zero-debt-architecture.mjs` in the same commit. Do not leave
  stale headroom.
- When a rule cannot be enforced by the architecture script, add it to
  `AGENTS.md` for agent discovery. If the rule is product/security behavior
  rather than implementation style, add or update `INVARIANTS.md` and its
  regression test.
- Run `bun run check:architecture` before pushing cleanup or forward work that
  touches these categories. For deploy-bound changes, use
  `bun run --cwd workers/api deploy`, which runs the architecture check,
  rebuilds web assets, and deploys the Worker.
