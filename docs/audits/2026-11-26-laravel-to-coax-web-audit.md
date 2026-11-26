# Laravel (`laravel/laravel` 13.x) to Coax Web Framework Audit

- **Date:** 2026-11-26
- **Audited Upstream Repo:** `https://github.com/laravel/laravel` (cloned locally to `/tmp/laravel-review`, branch `13.x`, commit `f32d7019cbd0ab0c378b3154c551d0c60c553ebc`, requiring `laravel/framework ^13.17`, PHP `^8.3`)
- **Target System:** Coax Native Web — proposed rename to **Coax Web** (`coax/web/`, `coax/web/api/`, `apps/openagents-web/`, `components/backend/dom.coax`) serving `openagents.com` (and first-party sites such as `gamebuilder.org`)
- **Companion Audit:** [`docs/audits/2026-11-20-openagents-com-live-surface-audit.md`](2026-11-20-openagents-com-live-surface-audit.md) (the live surface inventory and zero-base plan for `openagents.com`)

---

## 1. Executive Summary & Verdict

### First, a crucial scope distinction

`https://github.com/laravel/laravel` is **not** the Laravel runtime library (that lives in `laravel/framework`). It is the **application skeleton** — ~40 concise files that codify Laravel's opinionated project structure, single-entrypoint bootstrapping, database-backed zero-infrastructure defaults, testing ergonomics, developer CLI (`artisan`), and agentic development hooks (`AGENTS.md` / Laravel Boost).

That distinction is actually **good news** for us. We do not want PHP's runtime, Composer's dependency tree, Eloquent's ActiveRecord magic, or Vite/npm's JavaScript toolchain. What makes Laravel extraordinarily productive for both humans and AI coding agents is its **skeleton, conventions, declarative wiring, unified CLI, and zero-infra Postgres/DB-backed primitives** — almost all of which translate cleanly into compile-time checked, `#authority`-verified Coax.

### Where Coax Web already beats Laravel

Before importing anything, we must protect the strengths where Coax Web is already structurally ahead of Laravel:

1. **Verified `#authority` domain policies (`*-policy.coax`)**: Pure decision modules checked with `#test`, `#forall`, and `coax mutate` with zero surviving mutants (`coax/web/api/COAX.md`). Laravel has nothing equivalent out of the box.
2. **Zero-bundler, sub-150 ms hot-swap runtime**: `coax/web/serve.c` + `host.js` + `wasm.sh` swaps compiled Wasm logic, WGSL shaders, and content-hashed asset packs into a running browser tab while preserving live state (`coax/web/state-keep.js`), with zero `node_modules`, npm, or Vite.
3. **Unified GPU + DOM rendering architecture**: WebGPU worlds (`apps/openagents-web/`) plus a minimal binary DOM diff applier (`components/backend/dom.coax` → `dom-host.js`) sharing one Coax component tree and Markdown parser (`components/core/`).
4. **Modern cryptographic auth**: Passwordless email codes, NIP-98 Nostr signatures, passkeys, phone QR, device keys, L1/L2/L2-fresh step-up, 72-hour recovery, and scoped agent capability grants (`coax/web/api/auth.c`, `auth-policy.coax`).
5. **Immutable migration integrity & schema snapshots**: `coax/web/api/migrations/` with `migrations.sha256` immutability checks and generated `coax/web/api/schema.sql`, plus `DROP`/`TRUNCATE` DDL guards (`oa.allow_ddl`) and non-DDL runtime DB roles.
6. **Native C + Coax footprint**: Tiny distroless Cloud Run binary (`oa-api`), bounded request memory (`REQ_MAX`), `api --rules` / `api --limits` introspection, and first-class agent text twins (`/llms.txt`, `/index.md`).

### Where Coax Web currently hurts (and where Laravel shines)

Inspecting `coax/web/`, `coax/web/api/`, and `apps/openagents-web/` side by side with `laravel/laravel` reveals concrete structural friction that slows iteration and causes production drift:

1. **Fragmented routing across four disconnected surfaces**: API route auth rules live in `coax/web/api/auth.c` (`RULES[]`), handler dispatch is a 40-line order-sensitive `ROUTE(...)` cascade in `coax/web/api/api.c` (with fragile comments like `// before the admin routes`, `// before the rooms`), dev redirects live in `coax/web/site-routes.coax`, and production LB rules live in `coax/web/api/lb.sh`. As documented in the 2026-11-20 live surface audit, these have already drifted (`/verse`, `/projects/metaverse`, `/forge`, `/worlds/splice`), and `/api/openapi.json` returns 404.
2. **Flat directory sprawl**: `apps/openagents-web/` has 384 entries driven by a 657-line handwritten `build.sh`; `coax/web/api/` has 303 files sitting flat in one directory (`.c`, `.coax`, `.py`, `.sh`, `.ts`, `.conf`, `schema.sql`), including monolithic C files (`auth.c` at 125 KB, `player.c` at 109 KB, `worlds.c` at 91 KB).
3. **No durable async job queue**: `coax/web/api/jobs.c` only supports cron-triggered Cloud Scheduler endpoints (`heartbeat`, `auth-sweep`, `service-watch`). Meanwhile `coax/web/api/email.c` runs a 4-attempt `nanosleep()` exponential backoff loop **synchronously inside an HTTP worker process**, pinning one of only four `OA_API_WORKERS` processes and its Postgres connection whenever Resend throttles or errors.
4. **Scattered configuration via ad-hoc `getenv()`**: Over 25 `OA_*` environment variables are read lazily across C files (`api.c`, `auth.c`, `email.c`, `jobs.c`, etc.) with no unified boot-time config schema, no `.env.example`, and no fail-fast config check before accepting traffic.
5. **Fragmented developer CLI**: Common tasks are spread across `coax/web/serve.sh`, `coax/web/tool.sh`, `coax/web/deploy.sh`, `coax/web/release.sh`, `coax/web/rollback.sh`, `coax/web/api/test.sh --serve`, `coax/web/api/admin.sh`, `coax/web/api/schedules.sh`, and `apps/openagents-web/build.sh`. There is no single `artisan`-style entrypoint or one-command multiplexed local dev stack (`composer run dev`).
6. **Slow, sprawling integration test harness**: `coax/web/api/test.sh` is 632 lines and takes ~16 minutes across 35+ Python `test_*.py` scripts and Coax tests, with no lightweight transaction-isolated feature-test runner or reusable entity factories.
7. **Public lock/cache ad-hoc plumbing**: Release locking currently writes a public `/release.lock` object in the served GCS bucket (exposing hostname, PID, and username per the 2026-11-20 audit), and there is no general Postgres-backed cache/lock primitive.

### Bottom-line recommendation

**Pull the architectural conventions and zero-infra Postgres primitives from `laravel/laravel`; reject PHP/OOP runtime magic, ActiveRecord ORM, and Node/Vite asset bundling.** Pair this work with the `openagents.com` zero-base reset and formalize the framework name as **Coax Web**.

---

## 2. Naming Assessment: "Coax Native Web" → "Coax Web"

`coax/web.md` recorded the 2026-11-13 rename from **Coaxial** to **Coax Native Web** (as the browser target of **Coax Native**). Re-evaluating this name as the framework grows to own both browser pages (`apps/openagents-web/`, `gamebuilder.org`) and the HTTP API / backend (`coax/web/api/`):

| Criterion | Coax Native Web | **Coax Web** (Recommended) |
| --- | --- | --- |
| Clarity to outsiders | Internally paradoxical ("Native" + "Web") | Immediately clear: Coax's full-stack web framework (browser + server) |
| Repo & CLI alignment | Path is `coax/web/`, not `coax/native-web/` | Exact 1:1 match with `coax/web/` and proposed `coax web <cmd>` CLI |
| Full-stack scope | Sounds client/browser-only | Naturally covers Wasm/WebGPU/DOM frontend **and** C/Coax HTTP API, queues, migrations, and deploy |
| Family symmetry | Coax Native / Coax Native Web | **Coax Native** (macOS, iOS, Linux desktop) + **Coax Web** (browsers + web APIs), both sharing Coax + `components/` |
| ABI / contract risk | Zero if internal names stay | **Zero**: keep `coaxial_*` Wasm exports, `/coaxial/live`, `/coaxial/code`, and GCP resource names unchanged (same policy as 2026-11-13) |

**Recommendation:** Adopt **Coax Web** as the canonical public and documentation name, introduce `coax web <subcommand>` as the unified CLI entrypoint, and leave all existing `coaxial_*` Wasm ABI exports and GCP resource IDs untouched.

---

## 3. Complete Inventory of `laravel/laravel` (`13.x` @ `f32d7019`)

Every substantive file in the cloned repository was audited. Below is what `laravel/laravel` actually ships and why each piece exists:

| Layer | Files in `laravel/laravel` | Key Mechanism / Convention |
| --- | --- | --- |
| **Front Controller & Boot** | `public/index.php` (20 L), `bootstrap/app.php` (21 L), `bootstrap/providers.php` (7 L), `artisan` (18 L) | Single fluent application manifest (`Application::configure()->withRouting(web, commands, health: '/up')->withMiddleware()->withExceptions()`). Pre-boot maintenance check in `public/index.php`. Automatic JSON error rendering for `api` and `api/*` (`f32d701`). |
| **Application Code** | `app/Http/Controllers/Controller.php`, `app/Models/User.php` (32 L), `app/Providers/AppServiceProvider.php` (24 L) | Minimal thin skeleton (Laravel 11–13 stripped 20+ boilerplate middleware/kernel files out of `app/`). Models use PHP 8 attributes (`#[Fillable]`, `#[Hidden]`) and typed `casts()`. |
| **Routing** | `routes/web.php` (7 L), `routes/console.php` (8 L) | Dedicated declarative route files split by transport (`web`, `api` when added, `console` for CLI & scheduled tasks). Built-in `/up` health route. |
| **Configuration** | `config/{app,auth,cache,database,filesystems,logging,mail,queue,services,session}.php` (10 files), `.env.example` (65 L) | Every runtime knob lives in one typed config file backed by `env('KEY', default)`. Supports key rotation via `APP_PREVIOUS_KEYS`. Separate `.env.example` documents every variable with safe local defaults. |
| **Database-First Zero-Infra Defaults** | `database/migrations/0001_01_01_000000_create_users_table.php`, `...000001_create_cache_table.php`, `...000002_create_jobs_table.php` | Laravel 11–13 defaults sessions, cache + atomic locks (`cache`, `cache_locks`), and background queues + batches + dead-letters (`jobs`, `job_batches`, `failed_jobs`) to the **primary SQL database** — zero Redis/SQS required until scale demands it. |
| **Testing & Seeding** | `phpunit.xml` (36 L), `tests/TestCase.php`, `tests/Unit/`, `tests/Feature/`, `database/factories/UserFactory.php`, `database/seeders/DatabaseSeeder.php` | Clean `Unit` vs `Feature` split. `phpunit.xml` pins safe, fast test drivers (`CACHE_STORE=array`, `MAIL_MAILER=array`, `QUEUE_CONNECTION=sync`, `SESSION_DRIVER=array`, `BCRYPT_ROUNDS=4`, in-memory DB). Factories produce composable test states (`->unverified()`). |
| **Dev Ergonomics & DX** | `composer.json` scripts (`setup`, `dev`, `test`), `package.json`, `vite.config.js` | Single-command onboarding (`composer run setup`), single-command multiplexed dev stack (`composer run dev` → `php artisan dev` / `@laravel/multiplex` running server + queue worker + `laravel/pail` log tailer + Vite), code formatter (`laravel/pint`), REPL (`laravel/tinker`). |
| **Frontend Assets** | `resources/views/welcome.blade.php`, `resources/css/app.css`, `resources/js/app.js` | Blade SSR templates + Vite 8 + Tailwind CSS v4 (`@tailwindcss/vite`) + self-hosted fonts (`laravel-vite-plugin/fonts`). |
| **Agentic Development** | `AGENTS.md`, `CLAUDE.md`, `README.md` (Boost section) | Ships explicit agent bootstrap instructions in the repo root and promotes `laravel/boost` (15+ MCP tools & skills for route/schema/log/docs introspection). |

---

## 4. Decision Matrix: Pull, Adapt, Keep, or Reject

| Capability / Pattern | Laravel (`laravel/laravel`) | Coax Web Today | Verdict | Priority |
| --- | --- | --- | --- | --- |
| **Single declarative route manifest + OpenAPI/rule generation** | `bootstrap/app.php` + `routes/*.php` | Split across `auth.c` `RULES[]`, `api.c` `ROUTE()`, `site-routes.coax`, `lb.sh` (drifted) | **PULL & ADAPT** | **P0** |
| **Unified JSON error negotiation & `/up` health** | `withRouting(health: '/up')`, `shouldRenderJsonWhen(api/*)` | `/api/playtest/health` exists; inconsistent 404/405/503 across LB/API/static | **PULL** | **P0** |
| **Typed centralized config + `.env.example` + boot validation** | `config/*.php` + `.env.example` + `APP_PREVIOUS_KEYS` | 25+ ad-hoc `getenv()` calls scattered across C files; late failure | **PULL** | **P0** |
| **Postgres-backed job queue, batches & failed jobs** | `jobs`, `job_batches`, `failed_jobs` + `queue:work` | Only cron (`jobs.c`); `email.c` sleeps synchronously inside HTTP worker! | **PULL & ADAPT** | **P0** |
| **Unified CLI (`coax web <cmd>`) + one-command multiplexed dev** | `artisan` + `composer run dev` / `setup` / `pail` | 9+ separate shell scripts (`serve.sh`, `tool.sh`, `test.sh --serve`, `release.sh`, ...) | **PULL & ADAPT** | **P1** |
| **Convention-over-configuration app skeleton** | `app/`, `routes/`, `config/`, `database/`, `public/`, `tests/` | 384 flat files in `apps/openagents-web/`, 303 flat files in `coax/web/api/` | **PULL & ADAPT** | **P1** |
| **Postgres-backed cache & atomic lease locks** | `cache` + `cache_locks` tables | Ad-hoc GCS `/release.lock` (publicly readable!), no general cache/lock table | **PULL** | **P1** |
| **Mailer driver abstraction (`log`, `array`, `resend`)** | `config/mail.php` (`log`, `array`, `resend`, `smtp`) | `email.c` hardwires libcurl → Resend (`OA_RESEND_URL` fake in tests) | **PULL** | **P1** |
| **Maintenance mode with bypass & `Retry-After`** | `public/index.php` maintenance gate + `artisan down` | `/api/admin/flags` flag in `rooms.c`; not unified at front gate | **PULL** | **P2** |
| **Isolated fast Feature tests, Factories & Test Env Matrix** | `phpunit.xml`, `tests/Unit` vs `Feature`, `UserFactory` | 632-line `test.sh` (~16 min), 35+ mixed Python/Coax scripts | **PULL & ADAPT** | **P1** |
| **Agentic framework introspection (`Boost`-style MCP/CLI)** | `AGENTS.md` + `laravel/boost` (15+ tools) | `api --rules`/`--limits`, `/llms.txt`, `index.md` twins — strong start, not unified | **PULL & ADAPT** | **P1** |
| **Database migrations & schema integrity** | PHP `Blueprint` migrations (`up`/`down`) | `NNNN_*.sql` + `migrations.sha256` + `schema.sql` + DDL guard | **KEEP COAX** (already stronger; add `coax web make:migration`) | — |
| **Authentication & identity** | Email/password `users` + `password_reset_tokens` | Passwordless email, NIP-98, passkeys, QR, L1/L2-fresh, agent grants | **KEEP COAX** (far ahead) | — |
| **Domain business rules** | Controllers / Services / Policies (runtime PHP) | `#authority` `*-policy.coax` + `#forall` + `coax mutate` (0 survivors) | **KEEP COAX** (far ahead) | — |
| **Hot reload & browser rendering** | Vite 8 + Blade + Tailwind CSS v4 + npm | `serve.c` + `wasm.sh` + WebGPU + `components/backend/dom.coax` (zero npm) | **KEEP COAX** (far ahead) | — |
| **Eloquent ActiveRecord ORM** | `Model` magic attributes, lazy relations | Explicit SQL (`PQexecParams`) + pure Coax structs | **REJECT** (build typed SQL codegen instead) | — |
| **Runtime Service Container / Reflection / Facades** | `Application` IoC container + package auto-discovery | Explicit AOT compile-time wiring in Coax/C | **REJECT** | — |
| **Blade SSR templates / npm / Vite / Tailwind** | `resources/views/*.blade.php`, `package.json`, `vite.config.js` | Coax `components/` (DOM + GPU) + Markdown twins | **REJECT** | — |

---

## 5. Deep Dive: The 10 High-Value Capabilities to Pull / Adapt

### 5.1 Single Declarative Route Manifest (`routes/api.coax` + `routes/web.coax`)

**What Laravel gets right:** `bootstrap/app.php` + `routes/web.php` + `routes/api.php` declare every route, its middleware, its health endpoint (`/up`), and JSON exception behavior in one inspectable place. `php artisan route:list` prints the exact live table.

**Current Coax Web pain:**
- Adding a single endpoint today requires editing **at least three places** in lockstep:
  1. `RULES[]` in `coax/web/api/auth.c` (access level + scope)
  2. The 40-line `ROUTE(...)` cascade in `coax/web/api/api.c` (where placing `admin_route` before `flags_route` or `room_route` before `chat_route` silently shadows routes)
  3. `coax/web/api/lb.sh` and `coax/web/site-routes.coax` (if it's a web path or redirect)
  4. `proto/verse/api-routes-v1alpha1.json` (which is never served live — `/openapi.json` and `/api/openapi.json` are 404).

**Coax Web design:**
- Define **one** `#authority` route table per app (`routes/api.coax` and `routes/web.coax`):
  ```coax
  // Conceptual Coax Web route declaration
  RouteSpec :: struct {
    method:     HttpMethod;     // GET, POST, PUT, DELETE
    pattern:    string;         // "/api/auth/sessions/:id/remove"
    access:     AccessRule;     // PUBLIC, GUEST, ACCOUNT, HUMAN, L2, L2_FRESH, SCOPE, ADMIN
    scope:      AgentScope;     // NONE, PROFILE_READ, WALLET_SPEND, ...
    rate_limit: RateLimitId;    // NONE, EMAIL_IP, KEY_LOGIN, ...
    origin:     OriginPolicy;   // SAME_SITE, LISTED_FIRST_PARTY, ANY_CLIENT
    handler:    HandlerId;      // compile-time enum -> C/Coax function table
    req_schema: string;         // wire codec / proto message name
    res_schema: string;
    note:       string;
  }
  ```
- **Compile-time invariants (`#test` in `routes/api.coax`):**
  - Zero duplicate `(method, pattern)` entries.
  - Zero ambiguous prefix shadowing (static routes cannot be shadowed by dynamic `:param` or prefix routes).
  - Every `HandlerId` has an explicit `AccessRule` (impossible to ship an unguarded route).
  - Zero redirect loops in `routes/web.coax` (generalizing the excellent `#test "a moved address never leads to another moved address"` already in `coax/web/site-routes.coax`).
- **Single-source generation:** From `routes/api.coax` + `routes/web.coax`, generate:
  1. The `oa-api` C dispatch table and auth gate (`RULES[]` eliminated as a separate hand-maintained list).
  2. The `serve.c` dev-server route/redirect table AND the GCP LB URL-map spec (eliminating `lb.sh` ↔ `site-routes.coax` drift forever).
  3. Live `/api/openapi.json` and `/api/capabilities` JSON manifests (and `coax web routes`).
- **Standardized Health & Error Negotiation:** Add a canonical lightweight `GET /up` liveness route (plus keep `/api/playtest/health` for deep DB+build diagnostics), and enforce Laravel 13's `f32d701` rule: any request to `/api` or `/api/*` (or with `Accept: application/json`) **always** returns structured JSON errors (400/401/403/404/405/429/500/503) — never HTML `404.html` from the bucket.

---

### 5.2 Postgres-Backed Durable Job Queue, Batches & Failed Jobs (`queue-policy.coax` + `api --work`)

**What Laravel gets right:** `0001_01_01_000002_create_jobs_table.php` gives every app durable async background jobs (`jobs`), fan-out batch tracking (`job_batches`), and dead-letter storage (`failed_jobs`) directly in the primary SQL database — zero Redis, RabbitMQ, or SQS required.

**Current Coax Web pain:**
- `coax/web/api/email.c` lines 88–93 execute up to 4 synchronous `nanosleep()` retries (0.5 s + 1 s + 2 s + HTTP timeouts up to 20 s each!) **inside the HTTP request handler**. Because `api.c` runs `OA_API_WORKERS=4` forked processes, 4 simultaneous slow Resend calls completely stall the entire API instance!
- `coax/web/api/jobs.c` only supports Cloud Scheduler HTTP cron ticks (`heartbeat`, `auth-sweep`, `service-watch`). There is no general async outbox/queue for email delivery, Nostr relay publishing, push notifications, webhook calls, or long-running account exports/deletions.

**Coax Web design:**
- Add one migration creating `queue.jobs`, `queue.job_batches`, and `queue.failed_jobs` in Postgres (using `BIGSERIAL`, `JSONB` or typed `BYTEA` proto payloads, `queue TEXT`, `idempotency_key TEXT UNIQUE`, `attempts SMALLINT`, `max_attempts SMALLINT`, `available_at TIMESTAMPTZ`, `reserved_at TIMESTAMPTZ`, `reserved_by TEXT`).
- Claim jobs atomically using Postgres `SELECT ... FROM queue.jobs WHERE queue = $1 AND available_at <= now() AND (reserved_at IS NULL OR reserved_at < now() - $retry_after) ORDER BY id FOR UPDATE SKIP LOCKED LIMIT 1`.
- Put every state transition (enqueue validation, retry backoff calculation, dead-letter threshold, batch completion/cancellation) in a `#authority` `queue-policy.coax` verified with `coax mutate`.
- Run workers using the **same compiled binary** (`oa-api --work --queues=default,mail --concurrency=4`), either as a companion Cloud Run worker/sidecar or a forked worker pool inside `oa-api` in low-traffic/local dev mode.
- **Immediate win:** Refactor `email_send()` (`coax/web/api/email.c`) to enqueue a durable `mail.send` job inside the same Postgres transaction as the auth/account write (transactional outbox!). HTTP login responses drop to <20 ms regardless of Resend latency, and failed sends are inspectable/retryable in `queue.failed_jobs`.

---

### 5.3 Typed Centralized Configuration & `.env.example` (`config.coax`)

**What Laravel gets right:** All configuration flows through `config/*.php` with explicit defaults, documented environment variables in `.env.example`, and multi-key rotation (`APP_PREVIOUS_KEYS`). Application code reads typed config, never raw `getenv()` scattered across 20 files.

**Current Coax Web pain:**
- `coax/web/api/` scatters `getenv("OA_*")` and duplicated `static const char *env_or(...)` helpers across `api.c`, `auth.c`, `email.c`, `jobs.c`, etc.
- Missing secrets fail lazily at request time (e.g., `OA_RESEND_API_KEY` only returns 503 when an email is actually sent) instead of failing fast at container startup.
- Local onboarding requires reverse-engineering 632 lines of `coax/web/api/test.sh` to see which env vars exist.

**Coax Web design:**
- Introduce `config/app.coax` defining a single immutable `AppConfig` struct populated once at boot from environment variables (and `.env` in dev mode only, never in prod):
  - `app`: `env` (`local | staging | production | test`), `origin`, `site_origins[]`, `maintenance_driver`, `log_level`
  - `db`: `url`, `workers`, `idle_timeout_ms`, `allow_migrate`
  - `auth`: `seal_keys[]` (preserving `auth.c`'s versioned key rotation), `lookup_peppers[]`, `admin_pubkeys[]`, `session_idle_days`, `session_abs_days`
  - `mail`: `driver` (`log | array | resend`), `resend_api_key`, `resend_url`, `allowlist`, `from_address`
  - `queue`: `driver` (`sync | postgres`), `retry_after_sec`, `max_attempts`
  - `storage`: `driver` (`local | gcs`), `site_bucket`, `drops_bucket`
- Put config validation rules in `#authority` `config-policy.coax` (e.g. production **must** reject `OA_LOCAL_SERVE`, `mail.driver=log`, empty `seal_keys`, or wildcard `site_origins`).
- Ship `oa-api --config-check` (run in `deploy.sh` before traffic shift) and a checked-in `.env.example` with safe local defaults.

---

### 5.4 Unified Developer CLI (`coax web <cmd>`) & One-Command Dev Stack

**What Laravel gets right:** One memorable command (`php artisan ...`) discovers everything an engineer or agent can do. `composer run setup` boots a fresh checkout in one step; `composer run dev` starts the web server, queue worker, live log tailer (`laravel/pail`), and asset watcher in one terminal.

**Current Coax Web pain:**
- Developers and agents must remember 10+ distinct scripts and flag combinations:
  - `coax/web/serve.sh apps/openagents-web`
  - `coax/web/api/test.sh --serve http://127.0.0.1:8800`
  - `coax/web/tool.sh coax/web/api/migration-check.coax --write`
  - `coax/web/release.sh` / `rollback.sh` / `deploy.sh`
  - `coax/web/api/admin.sh`, `schedules.sh`, `lb.sh`
  - `apps/openagents-web/build.sh`

**Coax Web design:**
Create a single thin dispatcher `coax web <command>` (invocable via `coax/bin/coax web ...` or `coax/web/cli.sh` until integrated into `coax0`):

| Command | Replaces / Wraps | Behavior |
| --- | --- | --- |
| `coax web setup` | Manual Homebrew/LLVM/Postgres/assets setup | Verifies LLVM/wasi-libc/Postgres, copies `.env.example` if absent, syncs assets, runs migrations |
| `coax web dev [APP]` | `serve.sh` + `test.sh --serve` + logs | Multiplexes static/Wasm hot-reload server + local Postgres + `oa-api` + queue worker + structured log stream in one terminal |
| `coax web build [APP]` | `apps/openagents-web/build.sh` | Incremental parallel Wasm/WGSL/API build with timing table and budget checks |
| `coax web routes [--json]` | `api --rules` + `site-routes.coax` + `lb.sh` | Prints unified web + API route table (method, path, access, scope, limit, handler) |
| `coax web migrate [--status\|--check\|--fresh]` | `api --migrate-only` + `migration-check.coax` | Runs migrations, updates `migrations.sha256` & `schema.sql`, reports applied/pending |
| `coax web work [--queue=...]` | *(new)* | Runs Postgres job queue worker (`--once` for tests/cron) |
| `coax web schedule:list \| schedule:run` | `schedules.conf` / `schedules.sh` / `jobs.c` | Lists and executes scheduled jobs locally or syncs Cloud Scheduler |
| `coax web test [--unit\|--feature\|--only ...]` | `coax test` + `coax/web/api/test.sh` | Runs fast unit/policy/mutation tests or isolated feature tests |
| `coax web make:<page\|route\|policy\|migration\|job>` | Copy-pasting boilerplate files | Scaffolds canonical Coax Web files with matching `#test` stubs and registers them cleanly |
| `coax web release \| rollback` | `release.sh` / `rollback.sh` | Staging → smoke/golden diff → promote / rollback |
| `coax web down \| up` | Ad-hoc `/api/admin/flags` | Toggles maintenance mode with optional bypass token and `Retry-After` |
| `coax web doctor` | Manual debugging | Validates toolchain, config schema, route parity, migration checksums, LB drift |

---

### 5.5 Convention-Over-Configuration Coax Web App Skeleton

**What Laravel gets right:** Every Laravel project in the world has the exact same directory tree (`app/`, `bootstrap/`, `config/`, `database/`, `public/`, `resources/`, `routes/`, `storage/`, `tests/`). Both humans and coding agents know where any file belongs in <1 second without grepping 384 flat files.

**Current Coax Web pain:**
- `apps/openagents-web/` has 384 entries where live pages, faces, WGSL shaders, headers, and data files sit flat alongside subdirectories (`README.md` even has a 30-line section explaining what sits flat vs in subfolders and why).
- `coax/web/api/` mixes 303 framework + `openagents.com`-specific files in a single flat folder — meaning **the Coax Web framework (`serve.c`, `api.c` HTTP kernel, `auth` primitives, `dom-host.js`, `host.js`) is tangled together with `openagents.com` application code (`everglade`, `villa.c`, `makers.c`, `anime.c`, `worldbuilder.c`)**.

**Coax Web design:**
Separate **the Coax Web framework** (`coax/web/framework/` or `coax/web/`) from **a Coax Web application** (`apps/openagents-web/`, `gamebuilder/web/`). As `openagents.com` zero-bases (`docs/audits/2026-11-20-openagents-com-live-surface-audit.md`), adopt this canonical skeleton:

```text
apps/openagents-web/
├── app.coax                  # Single manifest (name, routes, config, jobs, pages)
├── .env.example              # Safe local dev defaults (zero secrets)
├── config/
│   └── app.coax              # Typed env config + production boot assertions
├── routes/
│   ├── web.coax              # Pages, redirects, /.well-known/*, /llms.txt
│   ├── api.coax              # Declarative API routes, auth rules, scopes, limits
│   └── console.coax          # Scheduled tasks & admin CLI commands
├── app/
│   ├── pages/                # One subfolder per page (entry.coax, face.c, *.wgsl, index.md)
│   ├── policies/             # Pure #authority *-policy.coax (0-mutant verified)
│   ├── wire/                 # *-wire.coax codecs & proto bindings
│   ├── handlers/             # Thin C/Coax HTTP adapters (no domain rules)
│   └── jobs/                 # Async queue jobs + scheduled tasks
├── database/
│   ├── migrations/           # Immutable NNNN_*.sql + migrations.sha256
│   ├── schema.sql            # Canonical generated Postgres snapshot
│   ├── factories/            # Composable Coax test data builders
│   └── seeders/              # Local dev seeders
├── public/                   # Static root assets (favicon.svg, mark.svg, robots.txt)
└── tests/
    ├── unit/                 # Fast compile-time coax test + coax mutate
    └── feature/              # Transaction-isolated HTTP + Postgres tests
```

And critically: **replace the 657-line handwritten `apps/openagents-web/build.sh`** with a declarative page manifest (`app.coax` / `pages.json`) consumed by `coax web build` so adding a page never requires editing a 657-line bash script with manual PID tracking!

---

### 5.6 Postgres-Backed Cache & Distributed Locks (`cache` + `cache_locks`)

**What Laravel gets right:** `0001_01_01_000001_create_cache_table.php` provides both a TTL key-value cache (`cache`) and atomic lease-based distributed locks (`cache_locks` with `key`, `owner`, `expiration`) using only Postgres — zero Redis required.

**Current Coax Web pain:**
- `coax/web/api/api.c` wisely refuses in-memory per-worker state ("Nothing a request decides lives in a worker's memory"), which is great for correctness across 4 workers × 4 Cloud Run instances.
- However, there is no shared cache/lock primitive. Release locking uses a GCS object `gs://openagentsgemini-openagents-web/release.lock` that is **publicly served at `https://openagents.com/release.lock`** (leaking hostname, PID, and user!), and external calls like Google's `tokeninfo` endpoint (`jobs.c`) or Nostr relay snapshots lack a clean shared cache layer.

**Coax Web design:**
- Add `sys.cache` (`key TEXT PRIMARY KEY`, `value BYTEA`, `expires_at BIGINT`) and `sys.locks` (`key TEXT PRIMARY KEY`, `owner TEXT`, `expires_at BIGINT`) with a `#authority` `lock-policy.coax` governing atomic acquire/renew/release/stale-takeover.
- Sweep expired rows in the existing `auth-sweep` (renamed `sys-sweep`) scheduled job.
- Move `release.lock` out of the public web bucket (either to a private ops bucket or `sys.locks`), eliminating the public info leak noted in the 2026-11-20 audit.

---

### 5.7 Mailer & Notification Driver Abstraction (`log` | `array` | `resend`)

**What Laravel gets right:** `config/mail.php` decouples *composing* an email from *delivering* it. Local dev uses `MAIL_MAILER=log` (printing emails/codes straight to the console with zero external API keys), tests use `MAIL_MAILER=array` (capturing sent messages in memory for deterministic assertions without spinning up a fake HTTP server), and production uses `resend` / `ses` / `postmark`.

**Current Coax Web pain:**
- `coax/web/api/email.c` tightly couples `email_send` to libcurl + Resend HTTP JSON parsing (`strstr(b.data, "\"id\"")`), requiring `test.sh` to spawn a fake Python HTTP server (`RESEND_PORT`) just to test email login flows.
- Email composition mixes raw JSON construction inside SQL (`email.begin`) and C strings.

**Coax Web design:**
- Define a small Coax `MailMessage` struct (`to`, `subject`, `text_body`, `html_body`, `idempotency_key`, `list_unsubscribe`) rendered by pure Coax templates (`app/mail/*.coax` with `#test` snapshot checks).
- Support three drivers selected by `config.mail.driver`:
  - `log`: writes masked/redacted or dev-visible OTP code cleanly to `coax web dev` console (keeping `auth.c`'s strict rule: **never** log secrets in `staging` or `production`).
  - `capture` (test): appends to `sys.mail_capture` table (or test inbox) so feature tests assert exact recipients, subjects, and OTP links in <1 ms with zero HTTP sockets.
  - `resend`: dispatches asynchronously via the new `queue.jobs` worker (`mail.send`).

---

### 5.8 Maintenance Mode & Graceful Front-Gate Control

**What Laravel gets right:** `public/index.php` checks for `storage/framework/maintenance.php` *before* booting the rest of the app, returning a fast 503 with `Retry-After` while allowing admins/operators through via a secret bypass cookie (`php artisan down --secret=... --retry=60`).

**Current Coax Web pain:**
- Maintenance mode currently lives behind `/api/admin/flags` (`rooms.c`), evaluated *after* request parsing and scattered across route families, with no standardized bypass cookie or unified static+API 503 response.

**Coax Web design:**
- Check a cached maintenance flag right at the front of `auth_gate()` in `oa-api` (and optionally in `serve.c` / LB):
  - Bypass allowed for: `/up`, `/api/playtest/health`, `/api/admin/flags`, and authenticated admins or a signed `OA-Maintenance-Bypass` cookie.
  - Everyone else immediately gets `503 Service Unavailable` + `Retry-After: 60` + clean JSON (`{"error":"maintenance","retry_after":60}`) on `/api/*`.
  - Controlled via `coax web down --retry=60` / `coax web up`.

---

### 5.9 Fast Feature-Test Ergonomics, Factories & Test Env Matrix

**What Laravel gets right:**
- Clear separation of `tests/Unit` (pure, instant, no I/O) and `tests/Feature` (full HTTP request/response against a test database using `RefreshDatabase` transaction rollbacks).
- `phpunit.xml` declares a single deterministic test environment (`CACHE_STORE=array`, `MAIL_MAILER=array`, `QUEUE_CONNECTION=sync`, `BCRYPT_ROUNDS=4`).
- `UserFactory` (`User::factory()->unverified()->create()`) lets tests construct valid domain fixtures in one readable line without duplicating SQL inserts across 40 test files.

**Current Coax Web pain:**
- `coax/web/api/test.sh` is 632 lines, spawns ~12 fake sidecar ports (`NPC_PORT`, `RESEND_PORT`, `TOKENINFO_PORT`, `GCS_PORT`, `RELAY_PORT`, `SPARK_PORT`, ...), runs 35+ Python `test_*.py` files plus Coax tests, and takes ~16 minutes (`API_PARTS` hardcodes `py:721` seconds!).
- Notice that `coax/web/api/tests/*.coax` tests are already **5–20× faster** than the legacy Python tests (`auth_checks:4s`, `account_settings_checks:1s`, `wallet_checks:6s` vs `py:721s`)!

**Coax Web design:**
1. **Standardize the two-tier test pyramid**:
   - **Unit (`coax web test --unit`)**: Pure `#test`, `#forall`, and `coax mutate` across `app/policies/` and `app/wire/` — runs in seconds, zero DB, zero sockets.
   - **Feature (`coax web test --feature`)**: Coax HTTP feature tests against one persistent template Postgres database where each test case runs inside a `BEGIN ... ROLLBACK` (or `CREATE DATABASE ... TEMPLATE oa_test_template`) instead of booting new clusters.
2. **Test environment matrix (`config/testing.coax`)**: Automatically sets `mail.driver=capture`, `queue.driver=sync`, `cache.driver=memory_or_pg`, deterministic test clock, and in-process mock adapters — eliminating half of the 12 fake Python sidecars in `test.sh`.
3. **Coax Test Factories (`database/factories/`)**: Small reusable builders (`factory_account()`, `factory_session(L2)`, `factory_agent(SCOPE_WALLET_SPEND)`) so feature tests stay concise and resilient to schema evolution.
4. **Retire legacy `test_*.py` as routes zero-base**: Per the 2026-11-20 audit, many of those 721 seconds test retired Metaverse/playtest/demo routes. Moving kept routes to Coax feature tests will cut full API CI time from ~16 minutes to under 60 seconds.

---

### 5.10 First-Class Agentic Framework Introspection (The "Laravel Boost" Pattern)

**What Laravel gets right:** Look at `laravel/laravel`'s `README.md`, `AGENTS.md`, and `CLAUDE.md` — Laravel 13 explicitly designs its skeleton for AI coding agents, promoting `laravel/boost` (15+ structured tools & skills: route inspector, DB schema inspector, log reader, doc search, interactive evaluator) so agents never guess conventions.

**Why this matters doubly for OpenAgents:** We *build* AI coding agents (`coder/`, `apps/openagents-desktop/`, `coax/lib/inky/`). Coax Web should be the **gold standard** of an agent-native web framework. We already have strong building blocks (`api --rules`, `api --limits`, `/llms.txt`, `/index.md` twins, `coax/web/tool.sh`, `.agents/skills/`).

**Coax Web design:**
Bundle a tiny, zero-dependency **`coax web inspect` / Coax Web MCP toolset** directly with the framework:
- `coax web routes --json` — complete machine-readable route + auth + rate-limit + wire-schema graph.
- `coax web schema --json` — tables, columns, indexes, and foreign keys parsed from `schema.sql`.
- `coax web policies --json` — all `#authority` policy modules, their `#test` / `#forall` counts, and mutation-check status.
- `coax web config --schema` — every typed config key, type, default, and prod requirement (never secret values).
- `coax web logs --tail N --level warn` — structured JSON logs from the local dev stack.
- Ship a concise `AGENTS.md` template in every Coax Web app skeleton pointing agents straight to `coax web doctor`, `coax web routes --json`, and `coax web test`.

---

## 6. What to Explicitly REJECT from Laravel (and Why)

Pulling the wrong half of Laravel would destroy Coax Web's architectural advantages. Do **not** adopt:

1. **Eloquent ActiveRecord ORM & Dynamic Magic (`__get`, lazy loading, mutators)**
   - *Why reject:* Violates Coax's explicit data-flow, `#authority` purity, and bounded-memory guarantees. ActiveRecord hides SQL queries behind property access, creates N+1 bugs, and couples domain logic to database state.
   - *Coax Web alternative:* Keep domain decisions in pure `*-policy.coax` functions. If C `PQexecParams` boilerplate becomes painful, build a **compile-time typed SQL query generator** (inspired by `sqlc`, emitting typed Coax/C structs from `.sql` files) — never an ActiveRecord ORM.
2. **Runtime IoC Service Container, Reflection & Facades**
   - *Why reject:* Coax is an ahead-of-time compiled systems language. Runtime container lookups, string-keyed bindings, and `composer.json` auto-discovery add indirection, hide dependencies from the compiler, and break static call-graph analysis (`coax deps`, `coax mutate`).
   - *Coax Web alternative:* Explicit struct wiring in `app.coax` — checked at compile time with zero runtime cost.
3. **Blade SSR Templates, Vite 8, npm, and Tailwind CSS Build Pipelines**
   - *Why reject:* Directly violates `coax/web.md`'s foundational constraint ("no npm, no bundler, no JavaScript framework"). Introducing Node/Vite into Coax Web would add megabytes of `node_modules`, supply-chain churn, and slower rebuilds than `serve.c`'s 118–135 ms Wasm hot swap.
   - *Coax Web alternative:* Keep `components/backend/dom.coax` (binary DOM diffing) + WebGPU islands + `components/core/` Markdown twins. For routes that genuinely need plain server-rendered HTML (SEO landing, `/memorial/*`, no-WebGPU fallback), emit HTML directly from a pure Coax builder with auto-escaping — zero template runtime, zero Node.
4. **Password-First Auth Scaffolding (`users.password`, bcrypt, `password_reset_tokens`)**
   - *Why reject:* Coax Web's auth architecture (`docs/auth/`, `auth-policy.coax`) is already strictly superior: passwordless email codes, NIP-98 cryptographic identity, passkeys, phone QR, L1/L2-fresh step-up, recovery, and scoped agent grants.
5. **SQLite-by-Default for Production Web APIs**
   - *Why reject:* `oa-api` runs multi-instance on Cloud Run (`max 4` instances × `4` workers) and depends on Postgres `FOR UPDATE SKIP LOCKED`, advisory migration locks, `jsonb`, and strict DDL roles (`oa.allow_ddl`). Keep Postgres as the primary database.
6. **Heavy Third-Party Package Ecosystem / Auto-Discovery**
   - *Why reject:* Laravel's `vendor/` tree pulls dozens of Symfony/PHP packages. Coax Web's security posture, 6-minute self-bootstrapped compiler Dockerfile, and minimal distroless container depend on owning the stack.

---

## 7. Phased Implementation Plan for Coax Web & `openagents.com`

This roadmap is deliberately sequenced to ride alongside the **`openagents.com` zero-base reset** (`docs/audits/2026-11-20-openagents-com-live-surface-audit.md`), avoiding wasted refactor work on routes slated for removal:

### Phase 0 — Naming & Contract Lock (1–2 days)
- Approve the public rename **Coax Native Web → Coax Web** in `coax/web.md`, `apps/openagents-web/README.md`, and `docs/REPO-MAP.md` (preserving `coaxial_*` Wasm ABI exports and GCP resource names unchanged).
- Agree on the canonical Coax Web app skeleton (Section 5.5) for both `openagents.com` (post-zero-base) and `gamebuilder.org`.

### Phase 1 — Single Source of Truth: Routes & Config (1–2 weeks)
- Build `routes/api.coax` and `routes/web.coax` (`#authority`, `#test`-checked for zero duplicates, zero shadowing, zero redirect loops).
- Generate `oa-api` dispatch + `RULES[]`, `serve.c` site routes, LB URL-map diff checks (`lb.sh`), and live `GET /api/openapi.json` + `GET /up` from that single table.
- Introduce `config/app.coax` + `.env.example` + `oa-api --config-check` (fail-fast production validation) and remove scattered `getenv()` calls from handlers.

### Phase 2 — Postgres-Backed Zero-Infra Primitives (1–2 weeks)
- Add migration for `queue.jobs`, `queue.job_batches`, `queue.failed_jobs`, `sys.cache`, and `sys.locks`.
- Implement `#authority` `queue-policy.coax` and `lock-policy.coax` (zero surviving mutants) + `oa-api --work` worker mode.
- Move `email.c` delivery + retry loops onto `queue.jobs` (unblocking HTTP workers immediately) and add `log` / `capture` / `resend` mail drivers.
- Move `release.lock` out of the public GCS bucket into `sys.locks` or a private ops bucket.
- Add unified front-gate maintenance mode (`503` + `Retry-After` + admin/cookie bypass).

### Phase 3 — Unified `coax web` CLI & Fast Test Runner (1–2 weeks)
- Ship `coax web {setup,dev,build,routes,migrate,work,schedule,test,make,release,rollback,down,up,doctor}` wrapping existing battle-tested scripts behind one consistent interface.
- Build `coax web dev` multiplexed local runner (hot-reload static/Wasm server + Postgres + `oa-api` + queue worker + structured logs in one command).
- Replace the 657-line `apps/openagents-web/build.sh` with a declarative page manifest (`app.coax`) driven by `coax web build`.
- Introduce `config/testing.coax`, transaction/template-isolated Coax feature tests, and reusable Coax factories (`database/factories/`), targeting <60 s full feature CI as legacy Python suites retire.

### Phase 4 — Framework/App Split & Agentic Introspection (2 weeks)
- Separate generic Coax Web framework runtime (`coax/web/`) from `openagents.com`-specific routes/policies (`apps/openagents-web/`), enabling `gamebuilder.org` and future sites to consume Coax Web cleanly.
- Expose `coax web inspect` / MCP tools (`routes --json`, `schema --json`, `policies --json`, `config --schema`, `logs --tail`) and ship a lean `AGENTS.md` template for every Coax Web app.

---

## 8. Owner Decisions Requested

1. **Rename:** Confirm renaming **Coax Native Web → Coax Web** (keeping `coaxial_*` Wasm ABI/exports unchanged).
2. **Sequence with zero-base:** Confirm landing Phase 1 (declarative `routes/` + `config/`) and Phase 2 (Postgres `queue.jobs` + `sys.locks` + async `email.c`) directly onto the zero-based `openagents.com` surface rather than retrofitting retired routes.
3. **Second consumer:** Confirm targeting **both** `openagents.com` and `gamebuilder.org` as the two reference consumers of the extracted Coax Web skeleton so framework boundaries stay honest.
4. **Typed SQL layer (future):** After Phases 1–3, decide whether to add a small compile-time typed SQL query generator (sqlc-style `.sql` → Coax/C structs) to further shrink `coax/web/api/*.c` handler boilerplate while keeping zero ORM magic.
