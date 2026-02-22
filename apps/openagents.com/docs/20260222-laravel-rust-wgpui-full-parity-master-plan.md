# Laravel to Rust/WGPUI Full Parity Master Plan (openagents.com)

Date: 2026-02-22  
Status: Proposed program plan (execution backlog not yet created)  
Owner: openagents.com platform

## 1) Goal

Port or explicitly retire 100% of currently active Laravel web app functionality in `apps/openagents.com` to Rust services (`service`) plus Rust/WGPUI web UI (`web-shell`), with production running Rust-only once parity is verified.

Chat/thread scope decision for this plan:
- End-state chat is Codex app-server protocol only.
- No parallel Vercel-style chat/thread authority lane is allowed.
- First-pass chat access requires a user ChatGPT account for Codex-backed flows.
- A future OpenAgents-native non-Codex chat lane is out of scope for this plan and must be proposed separately if revisited.

This plan covers:
- API parity
- Web route/page parity
- Codex app-server chat consolidation (replace Vercel chat stack)
- Streaming/event protocol parity (Khala WS delivery)
- Data/storage parity
- Ops/command parity
- Rollout and retirement of Laravel serving path

## 2) Scope Baseline (as of 2026-02-22)

Measured from current repository state in `apps/openagents.com`:

- API manifest coverage target: 77 endpoints (`tests/Feature/Api/V1/ApiRouteCoverageManifestTest.php`)
- Laravel HTTP controllers: 32 files (`app/Http/Controllers/`)
- Laravel services: 13 files (`app/Services/`)
- Laravel models: 17 files (`app/Models/`)
- Laravel migrations: 25 files (`database/migrations/`)
- React/Inertia pages: 18 TSX files (`resources/js/pages/`)
- React components: 102 TSX files (`resources/js/components/`)
- Laravel feature tests: 61 files (`tests/Feature/`)
- Laravel unit tests: 7 files (`tests/Unit/`)

Current Rust control-service/web-shell already ships key foundations (auth/session/control/route-split), but does not yet cover all Laravel domain surfaces.
Current Laravel chat surface still includes Vercel AI SDK frontend + Laravel AI backend protocol lanes; this plan retires that stack in favor of Codex app-server protocol only.

## 3) Parity Definition (non-negotiable)

Parity is complete only when all conditions hold:

1. All API routes in the Laravel API manifest are accounted for in Rust via either (a) parity-preserved implementation or (b) explicit retirement/deprecation with approved migration behavior; Vercel-style chat lanes are retired, not reimplemented.
2. All active web routes/pages (`/`, `/feed`, `/login`, `/settings/*`, `/l402/*`, `/admin`, `/openapi.json`) are served by Rust and preserve user-visible behavior, deep-link redirects, and error-state UX; legacy `/aui` is deleted and not recreated.
3. Khala WebSocket replay/live subscriptions preserve protocol semantics for chat/autopilot/codex worker delivery, including finish/error/tool events and replay/resume ordering guarantees.
4. Data writes/reads are backed by Rust-owned persistence logic and schema migration paths.
5. Internal and operations-critical endpoints (runtime secret fetch, lightning ops control plane, smoke validation lane, webhook ingestion, token issuance) are ported and verified.
6. Operator workflows currently done via Artisan commands have Rust equivalents.
7. Staging and production canary runs show no critical regressions, and rollback runbooks are validated.
8. Laravel is removed from production serving path and left only as archived legacy reference (if retained at all).
9. Cookie/session behavior parity is proven for guest + authenticated flows, including cookie attributes (`SameSite`, `Secure`, domain/path scope) and session upgrade behavior.
10. Rate limiting and throttles preserve Laravel semantics (keying strategy, window sizes, and violation responses).
11. Static asset hosting parity is proven (content hashing, `Cache-Control`/ETag semantics, compression, and service-worker update behavior).
12. Production chat code paths do not depend on Vercel AI SDK protocol semantics or Laravel AI package adapters; Codex app-server protocol is the single chat/thread authority lane.
13. First-pass Codex chat access policy is explicit and enforced (ChatGPT account required).
14. Final cleanup removes remaining PHP and TypeScript implementation code from active product paths so runtime/product implementation is Rust-only.
15. Web routing parity is implemented as a single-shell architecture: non-API GET requests resolve to one Rust-hosted shell entry (`index.html`), and internal navigation is handled in-app via route state + browser history (`pushState`/`popstate`) without full-page reloads.

## 4) Program Strategy

Use domain-sliced migration with strict contract verification:

1. Freeze baseline contracts and fixtures.
2. Port storage/domain behavior in Rust first.
3. Port APIs next, behind route split.
4. Port WGPUI pages after API domains are stable.
5. Run dual-read/dual-run parity checks in staging.
6. Execute canary, then full cutover.
7. Decommission Laravel serving path.
8. Use expand/migrate/contract database rollout rules with explicit mixed-version support while Laravel and Rust run concurrently.
9. Require data invariants/checksum validation after backfills and before route-group cutovers.
10. Consolidate chat/thread behavior onto Codex app-server protocol and remove duplicate Vercel/Laravel-AI chat execution paths from production code.
11. Final gate: eradicate remaining PHP and TypeScript implementation lanes from active product paths.

Transport doctrine for this plan:
1. Khala live delivery is WS-only per `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`.
2. No new SSE live lanes are allowed (`docs/plans/active/rust-migration-invariant-gates.md`, `INV-03`).
3. Command/mutation flows stay on authenticated HTTP APIs; subscriptions/replay stay on Khala WS (`docs/sync/ROADMAP.md`).
4. Codex control/event contracts follow `docs/protocol/codex-worker-control-v1.md` and `docs/protocol/codex-worker-events-v1.md`, aligned to the Codex app-server protocol subset audited in `docs/audits/2026-02-22-codex-app-server-parity-audit.md`.

## 5) Phase Plan

### Phase 0: Program Guardrails
Outcome: parity program has hard gates, measurable scope, and CI visibility.

### Phase 1: Contracts, Storage, and Shared Infrastructure
Outcome: Rust has canonical contracts, middleware semantics, and schema plan for all domains.

### Phase 2: Auth and Identity Surface
Outcome: login/session/token/profile behavior fully Rust-backed.

### Phase 3: Codex Chat Consolidation Surface
Outcome: Codex app-server is the only chat/thread execution contract for web chat surfaces; legacy Vercel chat endpoints are migrated or retired.

### Phase 4: Autopilot Surface
Outcome: autopilot CRUD/threads/live-delivery/policy behavior in Rust.

### Phase 5: Runtime Integration Surface
Outcome: runtime tools/skills/codex-worker/internal-runtime APIs in Rust.

### Phase 6: L402 and Payments Surface
Outcome: L402 + agent-payments + control-plane behavior in Rust.

### Phase 7: Social, Integrations, and Webhooks
Outcome: shouts/whispers/feed/integrations/webhook delivery behavior in Rust.

### Phase 8: WGPUI Web Experience Parity
Outcome: all routed pages and critical UI flows run in Rust/WGPUI.

### Phase 9: Ops, Cutover, and Laravel Retirement
Outcome: production Rust-only, audited, and stable.

## 6) GitHub Issue Backlog (execution order)

Each item below is intended to become one GitHub issue.

### Phase 0 issues

1. **OA-WEBPARITY-001 - Program charter and parity checklist freeze**
Description: Create a canonical parity checklist doc enumerating routes, pages, commands, and test targets; define pass/fail criteria and ownership.

2. **OA-WEBPARITY-002 - Baseline manifest export for routes/pages/commands**
Description: Generate machine-readable manifests from Laravel for API routes, web routes, page entries, and Artisan commands to lock migration scope.

3. **OA-WEBPARITY-003 - Contract capture harness for JSON + Khala WS**
Description: Build a harness to capture golden request/response fixtures plus Khala WS frame/replay transcripts for later Rust conformance checks.

4. **OA-WEBPARITY-004 - Parity CI scoreboard and required checks**
Description: Add CI job(s) that report per-domain parity status (pass/fail/drift) and block merges on contract regressions in migrated domains.

### Phase 1 issues

5. **OA-WEBPARITY-005 - Canonical Rust API envelope and error code matrix**
Description: Define and implement shared Rust response envelope/error semantics matching Laravel behavior where required.

6. **OA-WEBPARITY-006 - Middleware parity: auth, throttles, admin, runtime-internal**
Description: Port middleware semantics from Laravel (`auth`, throttles, admin, runtime.internal, WorkOS session checks) into reusable Rust layers.

7. **OA-WEBPARITY-007 - OpenAPI generation pipeline from Rust handlers**
Description: Produce `openapi.json` directly from Rust route contracts, including security scheme parity (session/cookie/bearer), key endpoint examples, and deprecated endpoint visibility, then enforce schema checks in CI.

8. **OA-WEBPARITY-008 - Rust persistence for user/auth/token domain**
Description: Implement Rust data access and storage semantics for users, session state, personal access tokens, and related identity fields.

9. **OA-WEBPARITY-009 - Rust persistence for Codex thread/read-model domain**
Description: Implement Rust repositories/models needed for Codex thread projections and read paths, and remove active dependency on legacy Vercel conversation persistence as chat authority.

10. **OA-WEBPARITY-010 - Rust persistence for autopilot/l402/integrations/social domain**
Description: Implement Rust data access for autopilot, L402, integrations, comms projections, shouts, whispers, and supporting tables.

11. **OA-WEBPARITY-011 - Idempotent migration/backfill plan for Rust ownership**
Description: Add migration/backfill scripts for any schema and data transitions required for Rust-first operation and safe rollback.

12. **OA-WEBPARITY-012 - Seed fixtures for deterministic cross-stack parity tests**
Description: Create deterministic fixtures and seeded state used by both Laravel baseline tests and Rust parity tests.

13. **OA-WEBPARITY-013 - Route-split domain controls and rollback matrix**
Description: Extend route-split controls for per-domain rollout, with explicit emergency rollback mappings per route group.

### Phase 2 issues

14. **OA-WEBPARITY-014 - Port `/api/auth/email`, `/api/auth/verify`, and `/api/auth/register`**
Description: Complete Rust parity for JSON auth bootstrapping, including challenge handling, throttles, and environment gates.

15. **OA-WEBPARITY-015 - Port web auth routes (`/login`, `/logout`, `/internal/test-login`)**
Description: Recreate login/logout/test-login route behavior in Rust, including signed URL logic for local/testing workflows.

16. **OA-WEBPARITY-016 - Port identity/token APIs (`/api/me`, `/api/tokens*`, `/api/khala/token`, `/api/sync/token`)**
Description: Implement Rust handlers with full policy/ability/expiration semantics, codex access/account-linkage signals needed for first-pass ChatGPT-gated chat, and backward-compatible payload contracts.

17. **OA-WEBPARITY-017 - Port profile API routes (`GET/PATCH/DELETE /api/settings/profile`)**
Description: Move profile read/update/delete logic and validation semantics to Rust with matching auth checks and audit side effects.

18. **OA-WEBPARITY-018 - Auth observability and compatibility contract parity**
Description: Mirror Laravel auth telemetry/audit behavior and align with control-service compatibility headers and rejection semantics.

### Phase 3 issues

19. **OA-WEBPARITY-019 - Remove Vercel AI SDK web chat lane and adopt Codex app-server client contract**
Description: Replace Vercel AI SDK-driven chat wiring for `/` with Codex app-server contract handling and explicit authenticated-user gating, and delete `/aui` route/page with no replacement.

20. **OA-WEBPARITY-020 - Replace Laravel AI/Vercel protocol backend lane with Codex command bootstrap**
Description: Remove Laravel AI protocol bridging and route chat command bootstrap through Codex worker control/app-server contract semantics with Khala WS delivery.

21. **OA-WEBPARITY-021 - Retire legacy Vercel conversation APIs (`/api/chats*`) from active product path**
Description: Deprecate or remove `/api/chats*` endpoints as standalone authority, provide migration-safe behavior, and move active thread lifecycle to Codex thread methods.

22. **OA-WEBPARITY-022 - Migrate chat history/read UX to Codex thread/read primitives**
Description: Rebuild history and transcript views using Codex thread/read semantics instead of legacy `messages/runs/run_events` REST shapes.

23. **OA-WEBPARITY-023 - Sunset legacy `/api/chat*` stream aliases with codex-only compatibility policy**
Description: Replace legacy stream alias behavior with codex-only migration responses/bridges and enforce no new Vercel-protocol stream behavior.

24. **OA-WEBPARITY-024 - Remove duplicate chat orchestrators and enforce single Codex chat authority**
Description: Eliminate production-path usage of Laravel `RunOrchestrator`/Vercel-protocol chat orchestration for web chat and keep Codex app-server as the only chat execution lane.

### Phase 4 issues

25. **OA-WEBPARITY-025 - Port autopilot CRUD APIs (`GET/POST/PATCH /api/autopilots*`)**
Description: Implement autopilot create/read/update/list handlers with validation and scoping semantics identical to Laravel.

26. **OA-WEBPARITY-026 - Port autopilot thread APIs (`GET/POST /api/autopilots/{autopilot}/threads`)**
Description: Implement autopilot thread listing/creation and associated ownership constraints in Rust.

27. **OA-WEBPARITY-027 - Port autopilot stream API (`POST /api/autopilots/{autopilot}/stream`)**
Description: Port autopilot command bootstrap endpoint and move live autopilot delivery to Khala WS replay/live topics with policy enforcement.

28. **OA-WEBPARITY-028 - Port autopilot prompt-context + tool resolver/runtime bindings**
Description: Rebuild prompt context construction and tool resolution/runtime binding behavior in Rust, including policy profiles.

29. **OA-WEBPARITY-029 - Port settings autopilot profile updates (`PATCH /settings/autopilot`)**
Description: Move autopilot profile settings behavior to Rust route handlers and persistence layer.

### Phase 5 issues

30. **OA-WEBPARITY-030 - Port runtime tools execute API (`POST /api/runtime/tools/execute`)**
Description: Implement typed runtime tool invocation with policy checks, replay modes, and deterministic receipts.

31. **OA-WEBPARITY-031 - Port runtime skill registry APIs (tool-spec + skill-spec)**
Description: Port list/store/publish/release handlers for runtime skill/tool specs with schema and version validation.

32. **OA-WEBPARITY-032 - Port runtime codex worker APIs (`/api/runtime/codex/workers*`)**
Description: Implement worker lifecycle endpoints (index/create/show/requests/events/stop) in Rust and align worker live updates with Khala WS replay/live parity.

33. **OA-WEBPARITY-033 - Port internal runtime secret fetch API**
Description: Port `POST /api/internal/runtime/integrations/secrets/fetch` with signature/auth middleware parity and secure auditing.

34. **OA-WEBPARITY-034 - Port runtime shadow routing and driver override behavior**
Description: Recreate runtime canary/shadow routing and override logic currently validated by Laravel runtime mode tests.

35. **OA-WEBPARITY-035 - Port sync-token v1 aliases and control-plane handshake**
Description: Ensure `/api/v1/sync/token` and related compatibility controls stay aligned with client expectations during migration.

### Phase 6 issues

36. **OA-WEBPARITY-036 - Port L402 primitives crate (challenge parser, Bolt11, credential cache)**
Description: Implement Rust-native L402 primitives and policy enforcement used by API/tools and payment workflows.

37. **OA-WEBPARITY-037 - Port L402 read APIs (`wallet`, `transactions`, `settlements`, `deployments`)**
Description: Implement Rust handlers for L402 operational read endpoints with same payload fields and auth rules.

38. **OA-WEBPARITY-038 - Port L402 paywall lifecycle APIs (`POST/PATCH/DELETE /api/l402/paywalls*`)**
Description: Implement admin-guarded paywall create/update/delete with validation, audit, and persistence parity.

39. **OA-WEBPARITY-039 - Port Agent Payments APIs + compatibility aliases**
Description: Port `/api/agent-payments/*` and `/api/agents/me/*`/`/api/payments/*` alias endpoints with identical behavior.

40. **OA-WEBPARITY-040 - Port internal lightning ops control-plane APIs**
Description: Port `/api/internal/lightning-ops/control-plane/query|mutation` request routing and response contract behavior.

41. **OA-WEBPARITY-041 - Port Spark/LND invoice payer adapters and spend controls**
Description: Port invoice payment adapter implementations and enforce max-spend/budget controls consistent with Laravel behavior.

42. **OA-WEBPARITY-042 - Port L402 web page data backends for `/l402/*`**
Description: Provide Rust page data/query backends used by wallet, transaction detail, paywalls, settlements, and deployments screens.

### Phase 7 issues

43. **OA-WEBPARITY-043 - Port shouts APIs (`GET /api/shouts`, `GET /api/shouts/zones`, `POST /api/shouts`)**
Description: Recreate public + authenticated shout behavior, validation, and feed shaping in Rust.

44. **OA-WEBPARITY-044 - Port whispers APIs (`GET/POST /api/whispers`, `PATCH /api/whispers/{id}/read`)**
Description: Implement whisper storage/listing/read-mark behavior with ownership and notification semantics.

45. **OA-WEBPARITY-045 - Port integrations APIs for Resend and Google**
Description: Port settings integrations routes (`resend` upsert/disconnect/test and Google redirect/callback/disconnect) with secrets lifecycle/audit parity.

46. **OA-WEBPARITY-046 - Port webhook ingest pipeline (`POST /api/webhooks/resend`)**
Description: Port inbound webhook verification, idempotent processing, projection updates, replay handling, and retry/failure persistence semantics for delivery projections.

47. **OA-WEBPARITY-047 - Port feed page backend and public discoverability path**
Description: Port `GET /feed` backend data shaping and pagination/filter semantics used by the public feed UI.

### Phase 8 issues

48. **OA-WEBPARITY-048 - WGPUI route shell parity for all web routes**
Description: Implement Rust/WGPUI route handling for all current web surfaces with matching auth-gate and redirect behavior, using a single-shell route model (no per-page HTML responses for non-API routes).
Acceptance criteria:
- Rust service routes all non-API page GETs to shared shell entrypoint (`index.html`) while preserving API/static route behavior.
- Web-shell runtime boots once per document load and internal route changes do not reinitialize the GPU/runtime singleton.
- Internal link navigation intercepts same-origin app routes and dispatches app `Navigate` intent instead of triggering full document navigation.
- App navigation updates browser history with `history.pushState(...)` so URL stays canonical for deep links/share links.
- Browser `popstate` is handled and mapped back into app route state (`location.pathname` -> `Navigate`) for back/forward parity.
- Hard refresh/direct URL load remains supported by server-side shell fallback + client-side route parse.
- Route-shell parity tests assert no full reload on internal navigation (except explicit hard refresh or external-link transitions).

49. **OA-WEBPARITY-049 - Port core chat UI page (`/`) to WGPUI**
Description: Rebuild the home/chat flow in WGPUI on Codex app-server protocol only (no Vercel AI SDK lane), preserving quick prompts and login gating behavior; do not recreate `/aui`.

50. **OA-WEBPARITY-050 - Port settings UI pages (`/settings/profile`, `/settings/autopilot`, `/settings/integrations`)**
Description: Recreate settings pages in WGPUI with form validation, async state handling, and integration action UX parity.

51. **OA-WEBPARITY-051 - Port L402 UI pages (`/l402`, `/l402/transactions/*`, `/l402/paywalls`, `/l402/settlements`, `/l402/deployments`)**
Description: Port full L402 page suite and table/detail interactions to WGPUI.

52. **OA-WEBPARITY-052 - Port admin Codex worker page (`/admin`)**
Description: Recreate admin worker controls, WS delivery visibility, and action safety semantics in WGPUI.

53. **OA-WEBPARITY-053 - Port remaining web utility surfaces (`/openapi.json`, `/api/smoke/stream`, nav wiring)**
Description: Ensure utility endpoints/pages and shell navigation/state integration are fully Rust-owned, with smoke validation aligned to Khala WS delivery and no SSE live transport.

54. **OA-WEBPARITY-054 - Accessibility, mobile fidelity, and performance parity signoff**
Description: Validate keyboard shortcuts, focus traps, responsive layout, scroll restoration, deep-link/login-gate redirects, 401/429/500 error UX, and performance budgets for migrated WGPUI pages before cutover.

### Phase 9 issues

55. **OA-WEBPARITY-055 - Port operator commands from Artisan to Rust CLI**
Description: Provide Rust CLI replacements for `demo:l402`, `khala:import-chat`, `ops:test-login-link`, `runtime:tools:invoke-api`, and `ops:create-api-token`.

56. **OA-WEBPARITY-056 - Build full parity regression suite (API, UI, stream, internal)**
Description: Build and enforce an end-to-end parity test lane that validates Rust behavior, codex-only chat authority, and explicit retirement behavior for legacy Vercel chat endpoints.

57. **OA-WEBPARITY-057 - Staging dual-run and shadow diff report**
Description: Run Rust and Laravel in staging dual-run mode, produce structured diff report, and resolve all critical mismatches.

58. **OA-WEBPARITY-058 - Production canary + rollback drill execution**
Description: Execute phased production canary using route-split controls with validated rollback drill and SLO guardrails.

59. **OA-WEBPARITY-059 - Production flip to Rust-only route target**
Description: Set Rust as default for all web/API route groups, lock write paths to Rust handlers, and remove fallback reliance.

60. **OA-WEBPARITY-060 - Retire Laravel serving path and archive legacy lane**
Description: Remove Laravel from active serving topology, archive remaining legacy files/docs/runbooks, and update ownership docs.

61. **OA-WEBPARITY-061 - Post-cutover architecture audit and hardening**
Description: Produce final audit of parity completion, outstanding debt, and SLO/observability hardening tasks after steady state.

### Cross-cutting addendum issues (required for full parity)

62. **OA-WEBPARITY-062 - Non-HTTP behavior parity harness (cookies, CORS/preflight, cache headers, throttles)**
Description: Add contract tests for cookie attribute semantics, API preflight behavior, endpoint caching headers (including `openapi.json` and list endpoints), rate-limit key/window behavior, and WS handshake/auth compatibility.

63. **OA-WEBPARITY-063 - Auth/session edge-case matrix parity**
Description: Add parity tests and fixes for refresh rotation race conditions, revoke/logout-other-devices WS subscription impact, and codex-only auth gating during legacy guest-session endpoint retirement.

64. **OA-WEBPARITY-064 - Webhook signature/idempotency/replay parity**
Description: Enforce signature verification parity, idempotency-key handling, replay safety for duplicate deliveries, and explicit retry-state transitions.

65. **OA-WEBPARITY-065 - Static asset host and service worker delivery parity**
Description: Verify content-hashed asset delivery, immutable/short-lived cache policies, compression (`gzip`/`brotli`), and service worker update/rollback behavior.

66. **OA-WEBPARITY-066 - Queue/scheduler/listener parity and Laravel scheduler shutdown**
Description: Port queued jobs, scheduled tasks, event listeners, and notification side effects to Rust-owned runtime behavior, then disable Laravel scheduler/worker paths in production.

67. **OA-WEBPARITY-067 - Mixed-version deploy safety, rollback, and backfill invariants**
Description: Define mixed-version deploy rules, expand/migrate/contract rollback paths, and data invariant checksums/count checks required before and after each cutover.

68. **OA-WEBPARITY-068 - Eradicate remaining PHP/TypeScript implementation lanes (Rust-only terminal gate)**
Description: Remove or archive all remaining PHP and TypeScript implementation code in active product paths so shipped runtime/product implementation is Rust-only.

## 7) Endpoint Coverage Map (Laravel API manifest -> issue mapping)

This map ensures every manifest endpoint has an owning issue.
Legacy `/stream` endpoints listed below are compatibility scope only; canonical live delivery must migrate to Khala WS replay/live subscriptions.
Legacy Vercel chat endpoints (`/api/chat*`, `/api/chats*`) are migration/deprecation scope and must not remain as a separate production chat authority lane.

### Auth and identity

- `POST api/auth/register` -> OA-WEBPARITY-014
- `POST api/auth/email` -> OA-WEBPARITY-014
- `POST api/auth/verify` -> OA-WEBPARITY-014
- `GET api/me` -> OA-WEBPARITY-016
- `GET api/tokens` -> OA-WEBPARITY-016
- `POST api/tokens` -> OA-WEBPARITY-016
- `DELETE api/tokens/current` -> OA-WEBPARITY-016
- `DELETE api/tokens/{tokenId}` -> OA-WEBPARITY-016
- `DELETE api/tokens` -> OA-WEBPARITY-016
- `POST api/khala/token` -> OA-WEBPARITY-016
- `POST api/sync/token` -> OA-WEBPARITY-016
- `GET api/settings/profile` -> OA-WEBPARITY-017
- `PATCH api/settings/profile` -> OA-WEBPARITY-017
- `DELETE api/settings/profile` -> OA-WEBPARITY-017

### Chat consolidation (legacy Vercel chat -> Codex app-server)

- `GET api/chat/guest-session` -> OA-WEBPARITY-019 and OA-WEBPARITY-021
- `POST api/chat` -> OA-WEBPARITY-020
- `POST api/chat/stream` -> OA-WEBPARITY-023
- `GET api/chats` -> OA-WEBPARITY-021
- `POST api/chats` -> OA-WEBPARITY-021
- `GET api/chats/{conversationId}` -> OA-WEBPARITY-021
- `GET api/chats/{conversationId}/messages` -> OA-WEBPARITY-022
- `GET api/chats/{conversationId}/runs` -> OA-WEBPARITY-022
- `GET api/chats/{conversationId}/runs/{runId}/events` -> OA-WEBPARITY-022
- `POST api/chats/{conversationId}/stream` -> OA-WEBPARITY-023

### Runtime tools, skills, and codex workers

- `POST api/runtime/tools/execute` -> OA-WEBPARITY-030
- `GET api/runtime/skills/tool-specs` -> OA-WEBPARITY-031
- `POST api/runtime/skills/tool-specs` -> OA-WEBPARITY-031
- `GET api/runtime/skills/skill-specs` -> OA-WEBPARITY-031
- `POST api/runtime/skills/skill-specs` -> OA-WEBPARITY-031
- `POST api/runtime/skills/skill-specs/{skillId}/{version}/publish` -> OA-WEBPARITY-031
- `GET api/runtime/skills/releases/{skillId}/{version}` -> OA-WEBPARITY-031
- `GET api/runtime/codex/workers` -> OA-WEBPARITY-032
- `POST api/runtime/codex/workers` -> OA-WEBPARITY-032
- `GET api/runtime/codex/workers/{workerId}` -> OA-WEBPARITY-032
- `GET api/runtime/codex/workers/{workerId}/stream` -> OA-WEBPARITY-032
- `POST api/runtime/codex/workers/{workerId}/requests` -> OA-WEBPARITY-032
- `POST api/runtime/codex/workers/{workerId}/events` -> OA-WEBPARITY-032
- `POST api/runtime/codex/workers/{workerId}/stop` -> OA-WEBPARITY-032
- `POST api/internal/runtime/integrations/secrets/fetch` -> OA-WEBPARITY-033

### Autopilot

- `GET api/autopilots` -> OA-WEBPARITY-025
- `POST api/autopilots` -> OA-WEBPARITY-025
- `GET api/autopilots/{autopilot}` -> OA-WEBPARITY-025
- `PATCH api/autopilots/{autopilot}` -> OA-WEBPARITY-025
- `GET api/autopilots/{autopilot}/threads` -> OA-WEBPARITY-026
- `POST api/autopilots/{autopilot}/threads` -> OA-WEBPARITY-026
- `POST api/autopilots/{autopilot}/stream` -> OA-WEBPARITY-027

### Social and messaging

- `GET api/shouts` -> OA-WEBPARITY-043
- `GET api/shouts/zones` -> OA-WEBPARITY-043
- `POST api/shouts` -> OA-WEBPARITY-043
- `GET api/whispers` -> OA-WEBPARITY-044
- `POST api/whispers` -> OA-WEBPARITY-044
- `PATCH api/whispers/{id}/read` -> OA-WEBPARITY-044

### L402 and payments

- `GET api/l402/wallet` -> OA-WEBPARITY-037
- `GET api/l402/transactions` -> OA-WEBPARITY-037
- `GET api/l402/transactions/{eventId}` -> OA-WEBPARITY-037
- `GET api/l402/paywalls` -> OA-WEBPARITY-037
- `POST api/l402/paywalls` -> OA-WEBPARITY-038
- `PATCH api/l402/paywalls/{paywallId}` -> OA-WEBPARITY-038
- `DELETE api/l402/paywalls/{paywallId}` -> OA-WEBPARITY-038
- `GET api/l402/settlements` -> OA-WEBPARITY-037
- `GET api/l402/deployments` -> OA-WEBPARITY-037
- `GET api/agent-payments/wallet` -> OA-WEBPARITY-039
- `POST api/agent-payments/wallet` -> OA-WEBPARITY-039
- `GET api/agent-payments/balance` -> OA-WEBPARITY-039
- `POST api/agent-payments/invoice` -> OA-WEBPARITY-039
- `POST api/agent-payments/pay` -> OA-WEBPARITY-039
- `POST api/agent-payments/send-spark` -> OA-WEBPARITY-039
- `GET api/agents/me/wallet` -> OA-WEBPARITY-039
- `POST api/agents/me/wallet` -> OA-WEBPARITY-039
- `GET api/agents/me/balance` -> OA-WEBPARITY-039
- `POST api/payments/invoice` -> OA-WEBPARITY-039
- `POST api/payments/pay` -> OA-WEBPARITY-039
- `POST api/payments/send-spark` -> OA-WEBPARITY-039
- `POST api/internal/lightning-ops/control-plane/query` -> OA-WEBPARITY-040
- `POST api/internal/lightning-ops/control-plane/mutation` -> OA-WEBPARITY-040

### Webhooks and utility

- `POST api/webhooks/resend` -> OA-WEBPARITY-046
- `GET api/smoke/stream` -> OA-WEBPARITY-053

## 8) Web Route/Page Coverage Map

- `GET /` -> OA-WEBPARITY-049
- `GET /feed` -> OA-WEBPARITY-047 and OA-WEBPARITY-048
- `GET /aui` -> retired (delete route/page; no Rust/WGPUI replacement)
- `GET /openapi.json` -> OA-WEBPARITY-007 and OA-WEBPARITY-053
- `GET /login` -> OA-WEBPARITY-015
- `POST /login/email` -> OA-WEBPARITY-015
- `POST /login/verify` -> OA-WEBPARITY-015
- `POST /logout` -> OA-WEBPARITY-015
- `GET /internal/test-login` (local/testing) -> OA-WEBPARITY-015
- `GET /settings/profile` -> OA-WEBPARITY-050
- `GET /settings/autopilot` -> OA-WEBPARITY-050
- `GET /settings/integrations` -> OA-WEBPARITY-050
- `PATCH /settings/profile` -> OA-WEBPARITY-050
- `PATCH /settings/autopilot` -> OA-WEBPARITY-029 and OA-WEBPARITY-050
- `POST /settings/integrations/resend` -> OA-WEBPARITY-045 and OA-WEBPARITY-050
- `DELETE /settings/integrations/resend` -> OA-WEBPARITY-045 and OA-WEBPARITY-050
- `POST /settings/integrations/resend/test` -> OA-WEBPARITY-045 and OA-WEBPARITY-050
- `GET /settings/integrations/google/redirect` -> OA-WEBPARITY-045 and OA-WEBPARITY-050
- `GET /settings/integrations/google/callback` -> OA-WEBPARITY-045 and OA-WEBPARITY-050
- `DELETE /settings/integrations/google` -> OA-WEBPARITY-045 and OA-WEBPARITY-050
- `GET /l402` -> OA-WEBPARITY-051
- `GET /l402/transactions` -> OA-WEBPARITY-051
- `GET /l402/transactions/{eventId}` -> OA-WEBPARITY-051
- `GET /l402/paywalls` -> OA-WEBPARITY-051
- `GET /l402/settlements` -> OA-WEBPARITY-051
- `GET /l402/deployments` -> OA-WEBPARITY-051
- `GET /admin` -> OA-WEBPARITY-052

## 9) Data/Schema Coverage Map

Primary table groups that must be Rust-owned by end state:

- Identity/auth: `users`, `personal_access_tokens` and related auth/session metadata -> OA-WEBPARITY-008, OA-WEBPARITY-011
- Codex thread/read-model runtime data (Rust-owned chat authority) -> OA-WEBPARITY-009, OA-WEBPARITY-020, OA-WEBPARITY-022
- Legacy Vercel conversation tables and adapters (`threads`, `runs`, `messages`, `run_events`, `agent_conversation_*`) retired or isolated from active product chat path -> OA-WEBPARITY-021, OA-WEBPARITY-024
- Autopilot: autopilot profile/policy/runtime-binding tables -> OA-WEBPARITY-010, OA-WEBPARITY-025..029
- L402 and payments: credentials, pending approvals, paywalls, control-plane tables, spark wallets -> OA-WEBPARITY-010, OA-WEBPARITY-036..041
- Social: `shouts`, `whispers` -> OA-WEBPARITY-010, OA-WEBPARITY-043..044
- Integrations/comms: integration secrets/audits/webhook events/delivery projections -> OA-WEBPARITY-010, OA-WEBPARITY-045..046

## 10) Rust Web Routing Implementation Contract

This section defines the required technical implementation for Rust-side web routing parity.

Current code anchors (for issue execution context):
- Non-API GET shell routing in control service: `apps/openagents.com/service/src/lib.rs:232`, `apps/openagents.com/service/src/lib.rs:272`, `apps/openagents.com/service/src/lib.rs:596`, `apps/openagents.com/service/src/lib.rs:630`.
- Web-shell boot + singleton init + route render path: `apps/openagents.com/web-shell/src/lib.rs:542`, `apps/openagents.com/web-shell/src/lib.rs:598`, `apps/openagents.com/web-shell/src/lib.rs:798`, `apps/openagents.com/web-shell/src/lib.rs:2855`.
- Immutable hashed asset-serving context for reload behavior: `apps/openagents.com/service/src/lib.rs:45`, `apps/openagents.com/service/src/lib.rs:584`.

1. Service routing model:
   - All non-API/non-static browser page routes resolve to the same shell entrypoint.
   - API routes retain normal handler dispatch and must never be swallowed by shell fallback.
2. Shell lifetime model:
   - WASM/WGPUI runtime initializes once per document load.
   - The app/runtime singleton remains alive across internal route transitions.
3. In-app route transition model:
   - Route changes are state transitions in Rust app state, followed by in-place rerender.
   - Internal app links must be intercepted and translated to app-level `Navigate` intents.
4. Browser history parity model:
   - Programmatic navigation calls `history.pushState(...)`.
   - Browser back/forward events (`popstate`) are translated into route state updates.
5. Reload semantics:
   - Full reload occurs only on hard refresh/direct initial load or explicit external navigation.
   - Internal route transitions do not tear down and remount the shell.
6. Known implementation gap at time of plan update:
   - `pushState`/`popstate` wiring is not yet treated as complete and remains mandatory scope under OA-WEBPARITY-048.

## 11) Verification Gates Per Issue

Each implementation issue should include acceptance criteria with these minimum checks:

1. Route contract parity:
   - Request/response status and JSON structure match baseline fixtures.
   - CORS preflight and cache header behavior matches baseline where applicable.
2. Auth/policy parity:
   - Unauth/forbidden/admin-only/internal-only behavior matches baseline.
   - Cookie/session attribute behavior and scope matches baseline.
   - Codex web chat access enforces first-pass account policy (authenticated ChatGPT-backed Codex access required).
3. Khala WS parity (if live update domain):
   - Replay/resume behavior, `(topic, seq)` ordering, and duplicate handling match captured transcripts.
   - Session revoke/logout flows terminate or block WS subscriptions with baseline-equivalent behavior.
4. Rust quality gates:
   - `cargo check --workspace --all-targets`
   - Relevant Rust unit/integration tests
5. Repo quality gates:
   - `./scripts/local-ci.sh changed`
   - Additional domain-specific harnesses as required
   - Production chat paths have no Vercel AI SDK/Laravel AI protocol dependencies (code/search gate).
6. Webhook parity (if webhook domain):
   - Signature verification, idempotency, replay, and retry-state transitions are validated.
7. Migration safety (if schema/data domain):
   - Mixed-version compatibility, rollback path, and invariants/checksum checks are validated.
8. Rollout safety:
   - Route-split off-switch documented for domain issues that affect production traffic.

## 12) Risks and Mitigations

1. Risk: hidden behavior in Laravel middleware/services causes silent regressions.
Mitigation: freeze contract fixtures first and require parity tests before route flips.

2. Risk: Khala WS replay/live behavior mismatch breaks clients.
Mitigation: WS transcript-based regression tests (replay + resume + stale-cursor) and canary-only rollout for live update domains.

3. Risk: auth/token semantics drift during mixed mode.
Mitigation: shared token/session conformance tests and migration period with dual validation.

4. Risk: L402/payments regressions impact real spend paths.
Mitigation: strict budgeted canary, deterministic fake-payer tests, and explicit rollback criteria.

5. Risk: UI parity lags backend parity.
Mitigation: keep route-split per domain/page and require page-level signoff before global cutover.

6. Risk: hidden async side effects (queues/scheduler/listeners) diverge after partial cutover.
Mitigation: parity-test async effects and do not disable Laravel scheduler/worker lanes until Rust async parity evidence is green.

7. Risk: static asset cache/service-worker mismatch causes stale clients after deploy.
Mitigation: enforce hashed asset + cache policy + SW update regression checks before production rollout.

8. Risk: split-brain chat behavior if Vercel-style lane remains partially active.
Mitigation: codex-only chat authority gate, explicit deprecation of `/api/chat*` + `/api/chats*`, and CI checks that block Vercel/Laravel-AI chat path reintroduction.

9. Risk: web navigation appears functional but silently reloads the full document, causing state loss and runtime churn.
Mitigation: require route-shell tests that assert `pushState`/`popstate` wiring and verify no full-page reload on same-origin internal navigation.

## 13) Exit Criteria for Full Program

Program is complete when:

1. OA-WEBPARITY-001 through OA-WEBPARITY-068 are closed.
2. API manifest coverage reports 77/77 endpoints accounted for (ported, codex-consolidated, or explicitly retired with approved migration behavior).
3. All active web routes/pages are Rust/WGPUI-owned and validated on desktop and mobile browsers (and iOS host app where applicable).
4. Production traffic runs Rust-only with stable SLOs through post-cutover observation window.
5. Laravel serving path is retired and documentation reflects Rust-first ownership.
6. Web chat/thread execution is codex app-server protocol only (no separate Vercel-style thread/chat implementation in production paths).
