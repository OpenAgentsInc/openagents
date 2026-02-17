# Autopilot API Plan (Laravel `apps/openagents.com`)

Status: Active plan (implementation-ready)
Date: 2026-02-17
Owner: OpenAgents

## 1. Scope and intent

This document replaces the earlier conceptual draft with an implementation plan tied to the current code in `apps/openagents.com`.

Goals:

1. Make Autopilot a first-class API resource (not just a hardcoded agent prompt).
2. Reuse the existing Laravel chat/run/receipt pipeline (`threads`, `runs`, `messages`, `run_events`) instead of creating a second execution stack.
3. Keep EP212 Lightning/L402 behavior intact and explicitly cover remaining gaps from `docs/plans/active/lightning/212-demo-plan.md`.

## 2. Current reality (codebase truth)

### 2.1 Canonical API surface is `/api/*` (not `/api/v1/*`)

Current routes and tests are `/api/*`:

- Routes: `apps/openagents.com/routes/api.php`
- Route manifest test: `apps/openagents.com/tests/Feature/Api/V1/ApiRouteCoverageManifestTest.php`
- OpenAPI generation test: `apps/openagents.com/tests/Feature/Api/V1/OpenApiGenerationTest.php`

`apps/openagents.com/public/openapi.json` is stale and still exposes `/api/v1/*` paths plus reduced tag/path coverage. We treat this as a publishing drift issue, not runtime truth.

### 2.2 Autopilot runtime today

The runtime is currently a single built-in agent persona:

- `apps/openagents.com/app/AI/Agents/AutopilotAgent.php`
- Tool registry: `apps/openagents.com/app/AI/Tools/ToolRegistry.php`

Current tools in Laravel:

1. `get_time`
2. `echo`
3. `lightning_l402_fetch`
4. `lightning_l402_approve`

### 2.3 Persistence model already in place

Chat execution persistence is already production-usable:

- Threads: `threads`
- Runs: `runs`
- Messages: `messages`
- Receipts/events: `run_events`

References:

- Migrations:
  - `apps/openagents.com/database/migrations/2026_02_15_000001_create_threads_table.php`
  - `apps/openagents.com/database/migrations/2026_02_15_000002_create_runs_table.php`
  - `apps/openagents.com/database/migrations/2026_02_15_000003_create_messages_table.php`
  - `apps/openagents.com/database/migrations/2026_02_15_000004_create_run_events_table.php`
- Orchestration and event writes: `apps/openagents.com/app/AI/RunOrchestrator.php`
- API controller: `apps/openagents.com/app/Http/Controllers/Api/ChatController.php`

Legacy `agent_conversations` tables still exist as compatibility/storage for `laravel/ai` conversation memory and import backfill:

- `apps/openagents.com/database/migrations/2026_01_11_000001_create_agent_conversations_table.php`

### 2.4 EP212-related pieces already implemented in Laravel

- L402 fetch + approval gating:
  - `apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`
  - `apps/openagents.com/app/AI/Tools/LightningL402ApproveTool.php`
- L402 client/policy/cache/allowlist:
  - `apps/openagents.com/app/Lightning/L402/L402Client.php`
  - `apps/openagents.com/config/lightning.php`
- Pending approvals:
  - `apps/openagents.com/database/migrations/2026_02_16_120000_create_l402_pending_approvals_table.php`
- Credential cache:
  - `apps/openagents.com/database/migrations/2026_02_16_000001_create_l402_credentials_table.php`
- Spark wallet ownership per user:
  - `apps/openagents.com/database/migrations/2026_02_17_000001_create_user_spark_wallets_table.php`
  - `apps/openagents.com/app/Http/Controllers/Api/AgentPaymentsController.php`
- L402 observability API endpoints:
  - `apps/openagents.com/app/Http/Controllers/Api/L402Controller.php`

## 3. Locked architecture decisions

1. Do not create a second message/run system for Autopilot.
2. Model Autopilot as owner-scoped durable entities and attach them to existing thread/run/event rows.
3. Keep existing `/api/chats/*` routes for backward compatibility; add Autopilot-first routes in parallel.
4. Keep deterministic receipt/event flow in `run_events` as the source of truth for replay/observability.
5. Keep `/api/*` as canonical route prefix and regenerate published OpenAPI from current code.

## 4. Finalized data model (v1)

### 4.1 New tables

#### `autopilots`

- `id` string(36) primary (UUIDv7)
- `owner_user_id` foreignId indexed (`users.id`)
- `handle` string(64) unique indexed
- `display_name` string(120)
- `avatar` string(255) nullable
- `status` string(16) indexed (`active|disabled|archived`)
- `tagline` string(255) nullable
- `created_at`, `updated_at`

Indexes:

- unique(`handle`)
- index(`owner_user_id`, `updated_at`)

#### `autopilot_profiles` (1:1)

- `autopilot_id` string(36) primary (fk -> `autopilots.id`)
- `owner_display_name` string(120)
- `persona_summary` text nullable
- `autopilot_voice` string(64) nullable
- `principles` json nullable
- `preferences` json nullable
- `onboarding_answers` json
- `schema_version` unsignedSmallInteger default 1
- `created_at`, `updated_at`

#### `autopilot_policies` (1:1)

- `autopilot_id` string(36) primary (fk -> `autopilots.id`)
- `model_provider` string(64) nullable
- `model` string(128) nullable
- `tool_allowlist` json
- `tool_denylist` json
- `l402_require_approval` boolean default true
- `l402_max_spend_msats_per_call` unsignedBigInteger nullable
- `l402_max_spend_msats_per_day` unsignedBigInteger nullable
- `l402_allowed_hosts` json
- `data_policy` json nullable
- `created_at`, `updated_at`

#### `autopilot_runtime_bindings` (optional but included in v1 schema)

- `id` bigIncrements
- `autopilot_id` string(36) indexed (fk -> `autopilots.id`)
- `runtime_type` string(32) indexed (`laravel|web_worker|desktop|external`)
- `runtime_ref` string(255) nullable
- `is_primary` boolean default true
- `last_seen_at` timestamp nullable
- `meta` json nullable
- `created_at`, `updated_at`

Indexes:

- index(`autopilot_id`, `runtime_type`)
- index(`autopilot_id`, `is_primary`)

### 4.2 Existing table changes

#### `threads`

Add:

- `autopilot_id` string(36) nullable indexed

#### `runs`

Add:

- `autopilot_id` string(36) nullable indexed

#### `run_events`

Add:

- `autopilot_id` string(36) nullable indexed

Additional index:

- index(`autopilot_id`, `type`, `id`)

Reason: current L402 and run queries are user-scoped. Multi-autopilot support requires fast autopilot-scoped filtering without expensive joins in every dashboard/API query.

## 5. Finalized API surface (Autopilot v1)

All under Sanctum auth and `/api/*`.

### 5.1 Autopilot resources

1. `POST /api/autopilots`
2. `GET /api/autopilots`
3. `GET /api/autopilots/{autopilot}` (`id` or `handle`)
4. `PATCH /api/autopilots/{autopilot}`
5. `POST /api/autopilots/{autopilot}/threads`
6. `GET /api/autopilots/{autopilot}/threads`

### 5.2 Streaming aliases

1. `POST /api/autopilots/{autopilot}/stream`

Behavior:

- Accepts the same `messages` payload shape as `POST /api/chats/{conversationId}/stream`.
- If `threadId` is omitted, create a thread for the autopilot and stream against it.
- Internally route through the existing `RunOrchestrator` so event semantics stay identical.

### 5.3 L402 autopilot scoping

Keep existing L402 routes and add optional filter:

- `GET /api/l402/wallet?autopilot=<id-or-handle>`
- `GET /api/l402/transactions?autopilot=<id-or-handle>`
- `GET /api/l402/paywalls?autopilot=<id-or-handle>`
- `GET /api/l402/settlements?autopilot=<id-or-handle>`
- `GET /api/l402/deployments?autopilot=<id-or-handle>`

Default stays current user-wide behavior if filter is not supplied.

## 6. Contract normalization decisions

### 6.1 L402 input units

Current mismatch:

- Laravel tool input: `maxSpendSats` (`apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`)
- Worker/Effect contracts: `maxSpendMsats` (`apps/autopilot-worker/src/tools.ts`, `packages/lightning-effect/src/contracts/l402.ts`)

Decision:

- New Autopilot API accepts `maxSpendMsats` as canonical.
- Keep `maxSpendSats` as backward-compatible alias for Laravel-only callers during migration.

### 6.2 Approval flag naming

Current mismatch:

- Laravel: `approvalRequired`
- Worker contracts: `requireApproval`

Decision:

- Canonical API field: `requireApproval`
- Laravel tool layer accepts both temporarily and normalizes internally.

### 6.3 Receipt field continuity

Keep existing receipt payload keys in `run_events` (`l402_fetch_receipt`) for compatibility with current L402 pages and tests, then layer normalized response DTOs in controllers.

## 7. Implementation sequence

### Phase A: OpenAPI and route baseline

1. Add Autopilot route/controller skeletons under `apps/openagents.com/app/Http/Controllers/Api`.
2. Add OpenAPI request/response factories in `apps/openagents.com/app/OpenApi/*`.
3. Regenerate published spec:
   - `php artisan openapi:generate --output=public/openapi.json`
4. Add parity test that compares generated spec to committed `public/openapi.json`.

### Phase B: Migrations and models

1. Add migrations for the 4 new Autopilot tables.
2. Add autopilot_id columns/indexes to `threads`, `runs`, `run_events`.
3. Add Eloquent models + relations.

### Phase C: Services and controllers

1. Create `AutopilotService` for create/update flows with transaction boundaries.
2. Implement `AutopilotController` CRUD + thread APIs.
3. Implement `AutopilotStreamController` aliasing to existing stream/orchestrator path.

### Phase D: Orchestrator and receipt propagation

1. Thread lookup resolves and carries `autopilot_id`.
2. `RunOrchestrator` writes `autopilot_id` onto `runs` + `run_events`.
3. L402 controller queries support optional autopilot filter.

### Phase E: EP212 alignment items in Laravel surface

1. Add explicit OpenAgents EP212 presets in `config/lightning.php`:
   - `ep212_openagents_premium`
   - `ep212_openagents_expensive`
2. Keep `sats4ai` preset as-is.
3. Add chat-visible wallet balance snapshot hook (currently wallet balance is on `/l402` pages, not in the chat stream UI).

## 8. EP212 review and implementation checklist

Source: `docs/plans/active/lightning/212-demo-plan.md`

### Implemented in current codebase

1. Approval-gated paid fetch flow (`lightning_l402_fetch` -> `lightning_l402_approve`).
2. Host allowlist + spend-cap policy enforcement.
3. Receipt persistence and L402 telemetry endpoints/pages.
4. Server-side payer backends including Spark wallet executor path (`spark_wallet`).
5. Programmatic API endpoints for wallet lifecycle and payments.

### Must be completed in this Autopilot API track

1. Autopilot-scoped data model and APIs (multi-agent ownership, not just per-user aggregation).
2. Canonical L402 input/output contract normalization (`msats`, `requireApproval`).
3. Stable presets for both EP212 domains (`sats4ai.com` and `l402.openagents.com`).
4. Chat-adjacent wallet balance visibility (so EP212 "live wallet" requirement is satisfiable in the same UX surface).

## 9. Verification and release gates

Required verification for this plan:

1. `cd apps/openagents.com && php artisan test`
2. `cd apps/openagents.com && npm run types && npm run build`
3. `cd apps/openagents.com && php artisan openapi:generate --output=public/openapi.json`
4. `cd apps/lightning-ops && npm run smoke:ep212-full-flow -- --json --mode mock`
5. `cd apps/lightning-ops && npm run smoke:ep212-full-flow -- --json --mode live` (release rehearsal gate)

Targeted tests to add/update:

1. Autopilot CRUD + ownership scoping tests.
2. Autopilot stream alias tests.
3. L402 API autopilot-filter tests.
4. OpenAPI published spec parity test.

## 10. Definition of done

Autopilot API v1 is done when all conditions are true:

1. Autopilot resources exist as first-class rows with finalized schema above.
2. Streaming can be invoked by autopilot handle/id and persists autopilot-scoped run artifacts.
3. L402 endpoints can return per-autopilot metrics/receipts.
4. EP212-required endpoints and guardrails are callable through Autopilot APIs without manual route-specific hacks.
5. `public/openapi.json` matches generated spec and reflects the real `/api/*` surface.
