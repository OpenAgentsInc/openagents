# Laravel (`laravel/laravel` + `laravel/framework` 13.x) to Coax Web Framework Audit

- **Date:** 2026-11-26
- **Audited Upstream Repos (cloned locally):**
  - `https://github.com/laravel/laravel` — application skeleton, branch `13.x`, commit `f32d7019cbd0ab0c378b3154c551d0c60c553ebc` (`/tmp/laravel-review`, ~40 files)
  - `https://github.com/laravel/framework` — full runtime framework (`Illuminate\*`), branch `13.x`, commit `321caf77d6c037577434e177c3bf280abb2b3513` (`/tmp/laravel-framework-review`, 38 namespaces, 1,759 PHP files, 279,381 LOC)
- **Target System:** Coax Native Web — proposed rename to **Coax Web** (`coax/web/`, `coax/web/api/`, `apps/openagents-web/`, `components/backend/dom.coax`) serving `openagents.com` (and first-party sites such as `gamebuilder.org`)
- **Companion Audit:** [`docs/audits/2026-11-20-openagents-com-live-surface-audit.md`](2026-11-20-openagents-com-live-surface-audit.md) (the live surface inventory and zero-base plan for `openagents.com`)

---

## 1. Executive Summary & Verdict

We cloned and audited **both** repositories that make up modern Laravel 13:

1. **`laravel/laravel` (~40 files)**: the application skeleton — conventions, single-entrypoint bootstrapping, database-first defaults, testing configuration, `artisan` CLI surface, and agentic scaffolding (`AGENTS.md` / Boost).
2. **`laravel/framework` (38 namespaces, 1,759 files, 279,381 LOC)**: the runtime library (`Illuminate\*`) — routing, middleware pipeline, database/Eloquent, queues/batches/middleware, cache/SWR/locks/rate-limiting, outbound HTTP client, structured context logging, deferred callbacks, validation/JSON Schema, scheduling, signed URLs, and test fakes.

### Measured baseline of `coax/web/api/` (why this audit matters)

Static inspection of `coax/web/api/` (`70` `.c` files / `23,732` LOC, `123` `.coax` files / `18,771` LOC, `48` `.py` files) alongside `apps/openagents-web/` (`384` entries, `657`-line `build.sh`) exposes concrete, measurable pain points that Laravel's architecture solves cleanly:

| Observed Friction in Coax Web Today | Measured Footprint | Laravel Pattern That Solves It |
| --- | --- | --- |
| Ad-hoc `getenv()` calls scattered across handlers | **143 calls across 34 C files** (`drops.c`: 15, `inference.c`: 12, `npc.c`: 11, `realms.c`: 10, `auth.c`: 6, `api.c`: 6) | Typed `config/*.php` + `.env.example` + boot-time config validation |
| Copy-pasted raw `libcurl` HTTP client code | **25 `curl_easy_init()` sites across 14 C files** (`drops.c`: 6, `inference.c`: 4, `npc.c`: 2, `email.c`: 1, `jobs.c`: 1, `mesh.c`: 1, ...) | `Illuminate\Http\Client` (bounded client, retry, `Http::fake()`, `preventStrayRequests()`) |
| Unstructured `stderr` printf logging with no request correlation | **318 `api_log()` calls across 35 C files** | `Illuminate\Log\Context` (structured request/job context propagation + hidden secrets) |
| Fragile `strstr()` JSON response scraping | **20 occurrences across 8 C files** (`drops.c`: 8, `villa.c`: 5, `email.c`: 1, `auth.c`: 1, ...) | Unified `*-wire.coax` / `JsonSchema` typed codecs & validation |
| Direct SQL with no transactional side-effect boundary | **551 `PQexec*()` calls across 43 C files** (`auth.c`: 59, `player.c`: 58, `realms.c`: 33, `worlds.c`: 31, ...) | `DB::afterCommit()` / `ShouldDispatchAfterCommit` transactional outbox |
| Synchronous retry sleeps inside HTTP request workers | `email.c` runs up to 4× `nanosleep()` + 20 s HTTP timeouts inside 1 of 4 `OA_API_WORKERS` | Postgres `jobs` / `job_batches` / `failed_jobs` + `Queue\Middleware` (`WithoutOverlapping`, `ThrottlesExceptions`, `RateLimited`) |
| Routing fractured across 4 disconnected sources | `auth.c` `RULES[]`, `api.c` 40-line `ROUTE()` cascade, `site-routes.coax`, `lb.sh` (already drifted; `/api/openapi.json` is 404) | `bootstrap/app.php` + declarative `routes/*.php` + `/up` + JSON error negotiation |
| Fragmented scripts & slow integration suite | 9+ shell scripts; `test.sh` is 632 lines, spawns ~12 fake Python sidecar ports, takes ~16 min (`py:721s`) | `artisan` + `composer run dev` + `phpunit.xml` test env matrix + `Http/Mail/Queue::fake()` + `travel()` fake clock |

### Where Coax Web already beats Laravel (do NOT regress)

1. **Verified `#authority` domain policies (`*-policy.coax`)**: Pure decision modules proven with `#test`, `#forall`, and `coax mutate` with zero surviving mutants (`coax/web/api/COAX.md`). Laravel has nothing comparable.
2. **Zero-bundler, sub-150 ms hot-swap runtime**: `coax/web/serve.c` + `host.js` + `wasm.sh` hot-swaps compiled Wasm logic, WGSL shaders, and content-hashed asset packs while preserving live tab state (`state-keep.js`) with zero npm, Vite, or `node_modules`.
3. **Unified GPU + DOM rendering**: WebGPU worlds (`apps/openagents-web/`) plus a tiny binary DOM diff backend (`components/backend/dom.coax` → `dom-host.js`) sharing one Coax component tree and Markdown parser (`components/core/`).
4. **Modern cryptographic auth**: Passwordless email codes, NIP-98 Nostr signatures, passkeys, phone QR, device keys, L1/L2/L2-fresh step-up, 72-hour recovery, and scoped agent grants (`auth.c`, `auth-policy.coax`).
5. **Immutable migration integrity & DDL guards**: `migrations.sha256` checksum locks, canonical generated `schema.sql`, `oa.allow_ddl` trigger guard, and non-DDL runtime Postgres roles.
6. **Minimal native footprint & agent-first text twins**: Tiny distroless C+Coax Cloud Run binary, bounded request memory (`REQ_MAX`), `api --rules`/`--limits`, `/llms.txt`, and `index.md` twins.

### Bottom-line verdict

- **Pull/adapt ~20% of Laravel's ideas** — specifically the **skeleton conventions** from `laravel/laravel` and **~12 small, high-leverage runtime primitives** from `laravel/framework` (declarative routing, typed config, Postgres queues + queue middleware, transactional `afterCommit`, `Context` structured logging, `defer()` post-response hooks, unified outbound HTTP client with fakes, SWR cache + atomic locks, signed URLs, cursor pagination, unified `coax web` CLI, and agentic introspection).
- **Reject ~80% of `laravel/framework`'s LOC** — Eloquent ActiveRecord (~40k+ LOC), IoC Container/Reflection/Facades, Blade/Vite/npm/Tailwind, PHP Collections, password-first auth, and Redis-centric layers.

---

## 2. Naming Assessment: "Coax Native Web" → "Coax Web"

`coax/web.md` recorded the 2026-11-13 rename from **Coaxial** to **Coax Native Web** (as the browser target of **Coax Native**). Re-evaluating this name as the framework owns both the browser runtime (`apps/openagents-web/`, `gamebuilder/web/`) and the backend HTTP/API/queue server (`coax/web/api/`):

| Criterion | Coax Native Web | **Coax Web** (Recommended) |
| --- | --- | --- |
| Clarity to outsiders | Internally contradictory ("Native" + "Web") | Immediately legible: Coax's full-stack web framework |
| Repo & CLI alignment | Path is `coax/web/`, not `coax/native-web/` | Exact 1:1 match with `coax/web/` and `coax web <cmd>` |
| Full-stack scope | Sounds client/browser-only | Naturally covers Wasm/WebGPU/DOM frontend **and** C/Coax HTTP API, queues, migrations, and deploy |
| Family symmetry | Coax Native / Coax Native Web | **Coax Native** (macOS, iOS, Linux desktop) + **Coax Web** (browsers + web APIs), both sharing Coax + `components/` |
| ABI / contract risk | Zero if internal names stay | **Zero**: keep `coaxial_*` Wasm exports, `/coaxial/live`, `/coaxial/code`, and GCP resource names untouched |

**Recommendation:** Adopt **Coax Web** as the canonical public and documentation name, ship `coax web <subcommand>` as the unified CLI entrypoint, and leave all existing `coaxial_*` Wasm ABI exports and GCP resource IDs unchanged.

---

## 3. Complete Inventory of Both Upstream Repositories

### 3.1 `laravel/laravel` (`13.x` @ `f32d7019`, ~40 files)

| Layer | Files | Key Convention / Mechanism |
| --- | --- | --- |
| **Front Controller & Boot** | `public/index.php` (20 L), `bootstrap/app.php` (21 L), `bootstrap/providers.php` (7 L), `artisan` (18 L) | Single fluent app manifest (`withRouting(web, commands, health: '/up')`, `withMiddleware`, `withExceptions`). Pre-boot maintenance check in `public/index.php`. Automatic JSON error rendering for `api` & `api/*` (`f32d701`). |
| **App Surface** | `app/Http/Controllers/Controller.php`, `app/Models/User.php` (32 L), `app/Providers/AppServiceProvider.php` (24 L) | Ultra-lean skeleton (Laravel 11–13 stripped 20+ boilerplate middleware/kernel files out of `app/`). Models use PHP 8 attributes (`#[Fillable]`, `#[Hidden]`). |
| **Routing** | `routes/web.php`, `routes/console.php` | Declarative route files split by transport (`web`, `api`, `console`), plus built-in `/up` liveness. |
| **Configuration** | `config/{app,auth,cache,database,filesystems,logging,mail,queue,services,session}.php`, `.env.example` | All runtime knobs live in typed config files backed by `env('KEY', default)`. Key rotation via `APP_PREVIOUS_KEYS`. |
| **Zero-Infra DB Defaults** | `0001_01_01_000000_create_users_table.php`, `...000001_create_cache_table.php`, `...000002_create_jobs_table.php` | Sessions, cache + atomic locks (`cache`, `cache_locks`), and background queues + batches + dead-letters (`jobs`, `job_batches`, `failed_jobs`) all default to the primary SQL database — zero Redis/SQS needed. |
| **Testing & DX** | `phpunit.xml`, `tests/Unit/`, `tests/Feature/`, `UserFactory.php`, `DatabaseSeeder.php`, `composer.json` (`setup`, `dev`, `test`) | Isolated test env overrides (`MAIL_MAILER=array`, `QUEUE_CONNECTION=sync`, `CACHE_STORE=array`, `BCRYPT_ROUNDS=4`). One-command `composer run setup` and multiplexed `composer run dev` (`@laravel/multiplex` + `laravel/pail`). |
| **Frontend Assets** | `vite.config.js`, `package.json`, `resources/views/welcome.blade.php`, `resources/css/app.css` | Blade SSR + Vite 8 + Tailwind CSS v4 + self-hosted fonts. |
| **Agentic Development** | `AGENTS.md`, `CLAUDE.md`, `README.md` (Boost) | Ships explicit agent bootstrap guidelines and promotes `laravel/boost` (15+ MCP tools & skills). |

### 3.2 Full 38-Namespace Audit of `laravel/framework` (`13.x` @ `321caf77`, 1,759 files, 279,381 LOC)

Every namespace in `src/Illuminate/` was measured and evaluated against Coax Web's invariants:

| Namespace | Files | LOC | What It Contains | Verdict for Coax Web |
| --- | ---: | ---: | --- | --- |
| `Database` | 255 | 61,709 | Eloquent ORM (~40k+ LOC), Query/Schema Builder, `ManagesTransactions` (`afterCommit`/`afterRollBack`), Migrations, Seeding | **SPLIT:** **PULL** `afterCommit()`/`afterRollBack()` hooks & `prohibitDestructiveCommands`; **KEEP** Coax SQL migrations + `schema.sql`; **REJECT** Eloquent ActiveRecord ORM (consider sqlc-style typed SQL codegen later) |
| `Foundation` | 271 | 37,467 | `Application`, `Configuration/*`, `Exceptions/Handler` (throttling, `dontReport`, `context()`), `Http/Middleware/*` (`PreventRequestsDuringMaintenance`, `ValidatePostSize`, `TrustProxies`, `TrustHosts`, `InvokeDeferredCallbacks`, `PreventStaleDatabaseReads`), `Testing` | **ADAPT:** Pull declarative boot config, front-gate maintenance middleware, `InvokeDeferredCallbacks`, exception dedup/throttling, and testing ergonomics; reject PHP container lifecycle |
| `Support` | 118 | 22,008 | `Defer` (`DeferredCallbackCollection`), `Lottery`, `Sleep`, `Benchmark`, `Uri`, `Carbon`/time-travel, `Str`, `Number`, `Arr`, `Once`, `Env`, `Fluent` | **PULL SELECTED:** `Defer` (post-response callbacks), `Lottery` (deterministic/testable sampling for error/sweep throttling), `Sleep`/fake clock for tests; skip PHP utility bloat |
| `Queue` | 131 | 15,620 | `DatabaseQueue` (`FOR UPDATE SKIP LOCKED`), `Worker` (graceful drain, memory/time limits), `Middleware` (`WithoutOverlapping`, `RateLimited`, `ThrottlesExceptions`, `Skip`), `ShouldBeUnique`, `FailoverQueue`, `FailedJobProvider` | **PULL & ADAPT (P0):** Core Postgres queue + `WithoutOverlapping`/`ThrottlesExceptions`/`RateLimited` as `#authority` `queue-policy.coax` + `oa-api --work` |
| `Validation` | 82 | 13,925 | Declarative validation rules, `FormRequest`, typed error bags (`422` field errors) | **ADAPT:** Unify input validation inside `*-wire.coax` codecs so every route returns consistent structured field errors — eliminates the 20 `strstr` JSON hacks |
| `Http` | 69 | 11,702 | `Request`/`Response`/`JsonResponse`/`StreamedResponse`, `Http\Client` (`PendingRequest`, `Factory`, `Pool`, retry, `Http::fake()`, `preventStrayRequests()`), `JsonResource` | **PULL `Http\Client` (P0):** Replace 25 raw `curl_easy_init()` call sites across 14 C files with one bounded outbound HTTP client + in-process test fakes |
| `Routing` | 64 | 11,580 | `Router`, `CompiledRouteCollection`, `UrlGenerator` (`signedRoute`, `temporarySignedRoute`), `ValidateSignature`, `ThrottleRequests`, `ResourceRegistrar` | **PULL & ADAPT (P0):** Single compiled Coax route table + stateless HMAC `temporarySignedRoute` + declarative per-route throttle |
| `Cache` | 66 | 10,573 | `Repository::flexible()` (SWR), `Lock` (atomic lease locks with `owner`/`refresh`/`block`), `RateLimiter`, `MemoizedStore` (per-request cache), `FailoverStore` | **PULL (P1):** Postgres `sys.cache` + `sys.locks` + SWR `flexible()` + per-request memoization + named rate limiters |
| `Console` | 90 | 9,748 | `Artisan` command registry, `Scheduling\Event` (`withoutOverlapping`, `onOneServer`, `evenInMaintenanceMode`, `pingOnFailure`), `Prohibitable`, signals | **PULL & ADAPT (P1):** `coax web <cmd>` unified CLI + declarative `routes/console.coax` scheduler with overlap locks & `Prohibitable` prod guards |
| `View` | 54 | 8,992 | Blade compiler, directives, view components | **REJECT:** Coax Web uses `components/backend/dom.coax` + WebGPU + Markdown twins (pure Coax HTML builder only where SSR text is required) |
| `Collections` | 11 | 8,557 | `Collection`, `LazyCollection` | **REJECT:** PHP workaround for weak arrays; Coax slices & explicit loops are faster and zero-allocation |
| `Testing` | 30 | 6,855 | `TestResponse` fluent assertions, `AssertableJson`, `InteractsWithTime` (`travel()`), `RefreshDatabase` | **PULL & ADAPT (P1):** Fast Coax feature-test assertions, JSON path assertions, fake clock `travel()`, savepoint/template DB isolation |
| `Mail` | 40 | 6,644 | `Mailer`, `Mailable`, drivers (`log`, `array`, `resend`, `smtp`, `failover`), `Mail::fake()` | **PULL (P1):** Driver split (`log`, `capture`, `resend`) + Coax mail templates + async delivery via `queue.jobs` |
| `Auth` | 49 | 6,252 | Guards, UserProviders, password resets, `Gate`/`Policy` | **KEEP COAX:** Coax Web's NIP-98/email-code/passkey/L2-fresh/agent-scope auth + `#authority` `*-policy.coax` is already far stronger |
| `Contracts` | 156 | 5,996 | PHP interface definitions for IoC decoupling | **REJECT:** AOT Coax doesn't need 156 runtime interfaces; use small explicit struct/function tables |
| `Filesystem` | 12 | 3,788 | `Storage` disk abstraction (`local`, `s3`/GCS), `temporaryUrl`, `Storage::fake()` | **ADAPT (P2):** Thin `local` vs `gcs` object-store adapter for `drops.c`, `realms.c`, `verse_packs.c` + in-memory test fake (replaces `GCS_PORT` Python sidecar) |
| `Bus` | 19 | 3,669 | `Dispatcher`, `Batch`/`PendingBatch`, `Chain` | **ADAPT (P1):** Lightweight Postgres `job_batches` & sequential `job_chain` on top of `queue.jobs` |
| `Broadcasting` | 27 | 3,534 | Pusher/Ably/Reverb WebSocket channels | **REJECT:** Coax Web already has native WebSockets (`serve.c`, `room.c`, `voice-host/ws.h`) and Nostr relays |
| `Redis` | 16 | 3,406 | phpredis/Predis cluster client | **REJECT:** Keep zero-Redis Postgres-first footprint on GCP |
| `Session` | 17 | 3,124 | Cookie/DB/file session handlers | **KEEP COAX:** `__Host-oa_session` with daily rotation & idle/abs expiry in `auth.c` is already stronger |
| `Pagination` | 18 | 3,020 | `Paginator`, `CursorPaginator` (opaque base64 keyset cursor) | **PULL `CursorPaginator` (P2):** Standardize keyset cursor pagination across chat history, audit logs, and lists |
| `Container` | 24 | 2,898 | Runtime reflection DI container | **REJECT:** Explicit compile-time wiring in Coax |
| `Notifications` | 26 | 2,811 | Multi-channel notifications (mail, push, database, Slack) | **ADAPT (P2):** Unify security notices (`auth.c` notices + push + email) behind one `notify` dispatch over `queue.jobs` |
| `Process` | 15 | 2,633 | Subprocess runner, `Pool`, `Process::fake()`, `preventStrayProcesses()` | **ADAPT (P2):** Clean bounded subprocess helper + fakes for `coax/web` dev/build tools & sandboxed script runners |
| `Log` | 12 | 2,195 | `LogManager`, `Context\Repository` (`add`, `addHidden`, `scope`, `dehydrate`/`hydrate`) | **PULL `Context` (P0):** Structured JSON logging + automatic request/job context propagation + hidden-secret redaction |
| `JsonSchema` | 18 | 1,989 | New in Laravel 12/13: typed JSON Schema AST (`object`, `array`, `string`, `anyOf`, `ValidationRules`) | **ADAPT (P1):** Generate JSON Schema + OpenAPI + validation directly from `*-wire.coax` / `proto/` |
| `Translation` | 11 | 1,853 | i18n message catalogs | **REJECT (YAGNI):** Not needed for `openagents.com` |
| `Events` | 7 | 1,640 | Event dispatcher + `ShouldDispatchAfterCommit` | **ADAPT (P1):** Keep explicit function calls (no opaque observer magic), but adopt `afterCommit` outbox dispatch |
| `Image` | 21 | 1,402 | New in Laravel 13: image manipulation manager | **REJECT (YAGNI):** Asset pipeline (`assets/registry/`, `og*.ts`) handles media at build time |
| `Hashing` | 6 | 673 | bcrypt/Argon2 password hashing | **KEEP COAX:** Passwordless auth + `coax/lib/crypto/` |
| `Cookie` | 5 | 620 | Cookie jar + cookie encryption | **KEEP COAX:** `auth.c` `__Host-` hardened cookies + `auth-seal.coax` |
| `Encryption` | 5 | 519 | AES-CBC + `previousKeys` rotation | **KEEP COAX:** `auth-seal.coax` AEAD with versioned `1:<hex>,2:<hex>` rotation is already superior (standardize across all secrets) |
| `Pipeline` | 3 | 464 | Composable middleware stage runner (`send()->through()->then()`) | **ADAPT (P1):** Fixed compile-time middleware pipeline array in `oa-api` (maintenance → origin → trace/context → auth → rate-limit → handler → defer) |
| `Concurrency` | 6 | 454 | `Concurrency::run()` / `defer()` | **ADAPT:** Adopt `defer()` post-response hooks; Coax/C already controls processes/threads explicitly |
| `Reflection` | 3 | 441 | Runtime PHP reflection helpers | **REJECT:** Zero runtime reflection in Coax |
| `Config` | 1 | 305 | Dot-notation config repository | **PULL (P0):** Typed `config/app.coax` struct (no stringly-typed dot lookups) |
| `Conditionable` | 2 | 181 | Fluent `->when()` trait | **REJECT:** PHP fluent-API syntactic sugar |
| `Macroable` | 1 | 134 | Runtime monkey-patching methods onto classes | **REJECT:** Actively harmful to static analysis & `#authority` verification |

---

## 4. Deep Dive: The 14 High-Value Capabilities to Pull / Adapt into Coax Web

The original 10 patterns from `laravel/laravel` plus 4 major runtime primitives discovered inside `laravel/framework` (`Http\Client`, `Log\Context`, `DB::afterCommit` + `Defer`, and `Queue\Middleware` + Signed URLs + Cursor Pagination):

### 4.1 Single Declarative Route Manifest (`routes/api.coax` + `routes/web.coax`) — **P0**

**From:** `laravel/laravel` `bootstrap/app.php` + `routes/*.php`; `laravel/framework` `Routing\CompiledRouteCollection` + `Foundation\Exceptions\Handler`.

**Current Coax Web pain:** Adding one endpoint requires editing 3–4 places in lockstep (`auth.c` `RULES[]`, `api.c` 40-line order-sensitive `ROUTE(...)` cascade, `site-routes.coax`, `lb.sh`), causing documented production drift (`/verse`, `/projects/metaverse`, `/forge`, `/worlds/splice`) and leaving `/api/openapi.json` returning 404.

**Coax Web design:**
- One `#authority` route table (`routes/api.coax` and `routes/web.coax`) declaring `(method, pattern, access, scope, rate_limit, origin_policy, handler, req_wire, res_wire, note)`.
- **Compile-time `#test` invariants:** zero duplicate `(method, pattern)`, zero prefix shadowing, every handler bound to an explicit `AccessRule`, zero redirect loops.
- **Single-source generation:** emits `oa-api` C dispatch + auth gate, `serve.c` site routes, `lb.sh` URL-map diff checks, and live `GET /api/openapi.json` + `GET /up`.
- **Consistent JSON error contract (`f32d701`):** Every `/api` or `/api/*` request (or `Accept: application/json`) always returns structured JSON errors (400/401/403/404/405/422/429/500/503) — never static `404.html`.

---

### 4.2 Unified Outbound HTTP Client with Bounded Retries & Test Fakes — **P0** *(from `laravel/framework`)*

**From:** `Illuminate\Http\Client` (`PendingRequest`, `Factory`, `Pool`, `Http::fake()`, `Http::preventStrayRequests()`).

**Current Coax Web pain:** Measured **25 separate `curl_easy_init()` call sites across 14 C files** (`drops.c`: 6, `inference.c`: 4, `npc.c`: 2, `verse_packs.c`: 2, `villa.c`: 2, `chat.c`, `email.c`, `jobs.c`, `lnaddr.c`, `mesh.c`, etc.). Each file re-invents its own `buf` struct, write callback, timeout flags, error mapping, and `strstr` JSON scraping — and `test.sh` has to spawn ~12 fake Python HTTP servers (`RESEND_PORT`, `TOKENINFO_PORT`, `GCS_PORT`, `NPC_PORT`, `INFER_PORT`, `SPARK_PORT`, ...) to test them.

**Coax Web design:**
- Build **one** `coax/web/api/http_client.c` + `#authority` `http-client-policy.coax` used by every outbound caller:
  - Mandatory connect timeout, total deadline, and `max_response_bytes` cap (no unbounded `realloc` OOMs).
  - Explicit SSRF policy (allowlisted hosts/schemes for user-influenced URLs like LNURL/Nostr).
  - Retry decision delegated to `http-client-policy.coax` (only idempotent or keyed requests, bounded jittered backoff, never inside an HTTP handler when the queue should own retries).
  - **Test-mode fake registry (`http_fake_register` + `http_prevent_stray_requests`)**: in feature tests, unmatched outbound HTTP calls fail immediately instead of hitting the network or needing a spawned Python server.

---

### 4.3 Structured Request & Job Context Logging (`Context`) — **P0** *(from `laravel/framework`)*

**From:** `Illuminate\Log\Context\Repository` (`add`, `addHidden`, `scope`, `dehydrate`, `hydrate`).

**Current Coax Web pain:** Measured **318 `api_log()` calls across 35 C files**, all emitting flat `api  <printf>` lines to `stderr` with no `request_id`, `route`, `status`, `duration_ms`, or `principal_kind` — making Cloud Run incident triage slow when 4 workers interleave logs.

**Coax Web design:**
- Maintain a tiny per-request fixed-buffer `LogContext` struct initialized at `serve_one()` entry (`request_id`, `method`, `path`, `route`, `worker_pid`) and enriched by `auth_gate()` (`principal_kind`, `account_id_hash` — **never** tokens, cookies, emails, or keys, matching Laravel's `addHidden()` separation and `auth.c`'s strict zero-secret-logging rule).
- Emit one-line structured JSON logs (on Cloud Run, Google Cloud Logging indexes JSON fields automatically!) plus a human-readable compact formatter in `coax web dev`.
- **Cross-queue propagation:** When a request enqueues a `queue.jobs` row, serialize its public `LogContext` (`request_id`, `trace_id`, `actor`) into the job metadata; when `oa-api --work` picks up the job, hydrate that context so async mail/webhook logs correlate directly back to the triggering HTTP request.

---

### 4.4 Postgres-Backed Durable Job Queue, Batches & Queue Middleware — **P0** *(deepened from `laravel/framework`)*

**From:** `laravel/laravel` `0001_01_01_000002_create_jobs_table.php`; `laravel/framework` `Queue\DatabaseQueue`, `Queue\Middleware\{WithoutOverlapping, RateLimited, ThrottlesExceptions}`, `Bus\Batch`.

**Current Coax Web pain:**
- `email.c` runs up to 4 synchronous `nanosleep()` retries + 20 s HTTP timeouts **inside an HTTP worker**, starving 1 of only 4 `OA_API_WORKERS` processes per slow Resend call.
- `jobs.c` only supports cron-triggered Cloud Scheduler endpoints, not ad-hoc async jobs, retries, batches, or dead-letter inspection.

**Coax Web design:**
- Add `queue.jobs`, `queue.job_batches`, and `queue.failed_jobs` in Postgres, claimed with `SELECT ... FOR UPDATE SKIP LOCKED LIMIT 1`.
- Encode all state transitions and Laravel's three highest-value **Queue Middleware** primitives directly in `#authority` `queue-policy.coax` (mutation-checked):
  1. **`WithoutOverlapping(key, lease_s)`**: uses `sys.locks` so two jobs for the same account, wallet backup, or realm never race.
  2. **`ThrottlesExceptions(max_errors, window_s, backoff_s)`**: circuit-breaker backoff during downstream outages (Resend, Gemini, Spark) so jobs don't burn all retries in seconds.
  3. **`RateLimited(limiter, key)`**: respects external provider rate quotas without dropping work.
- Run workers via the same binary (`oa-api --work --queues=default,mail --concurrency=4`) with graceful `SIGTERM` drain and optional `--once` mode for tests.
- **Immediate win:** Move `email_send()` onto `queue.jobs` so login/auth HTTP latency drops to <20 ms regardless of Resend availability.

---

### 4.5 Transactional `afterCommit()` Hooks & Post-Response `defer()` — **P0/P1** *(from `laravel/framework`)*

**From:** `Illuminate\Database\Concerns\ManagesTransactions` (`afterCommit`, `afterRollBack`) and `Illuminate\Support\Defer` (`InvokeDeferredCallbacks`).

**Current Coax Web pain:** Across **551 `PQexec*()` calls in 43 C files**, there is no structural guard preventing a handler from triggering external effects (sending mail, waking `oa-agent`, emitting push notices, logging analytics) before a Postgres transaction commits — or if it rolls back. And lightweight post-request work currently blocks the HTTP response flush.

**Coax Web design:**
- **`db_tx_after_commit(fn, ctx)`**: A tiny fixed-slot callback array attached to the active transaction. Queuing an external HTTP wake (`oa-agent`) or emitting a push notification only runs **after** `COMMIT` succeeds, and is discarded automatically on `ROLLBACK` (while Postgres `queue.jobs` inserts participate *inside* the same `COMMIT` as a true transactional outbox).
- **`api_defer(fn, ctx)`**: Right after `serve_one()` writes the HTTP response and closes the client socket (`Connection: close`), the worker runs any registered deferred callbacks (non-critical analytics, SWR cache refresh, lightweight housekeeping) with a strict time cap before calling `accept()` again — zero latency added to the caller, zero queue overhead.

---

### 4.6 Typed Centralized Configuration & `.env.example` (`config/app.coax`) — **P0**

**From:** `laravel/laravel` `config/*.php` + `.env.example` + `APP_PREVIOUS_KEYS`; `laravel/framework` `Config` + `Encryption\Encrypter::previousKeys()`.

**Current Coax Web pain:** **143 `getenv()` calls across 34 C files**, duplicated `env_or()` helpers, lazy request-time failures on missing secrets, and no `.env.example`.

**Coax Web design:**
- One immutable `AppConfig` struct loaded once at boot from env (and `.env` in `local` mode only), validated by `#authority` `config-policy.coax` (`production` refuses `OA_LOCAL_SERVE`, `mail.driver=log`, empty `seal_keys`, or wildcard `site_origins`).
- Standardize `auth.c`'s versioned key rotation (`1:<hex>,2:<hex>`) across all signing/sealing secrets in `config/app.coax`.
- Ship `oa-api --config-check` (gated in `deploy.sh` before traffic shift) and a zero-secret `.env.example`.

---

### 4.7 Postgres Cache, SWR (`Cache::flexible`), Atomic Locks & Named Rate Limiters — **P1** *(deepened from `laravel/framework`)*

**From:** `Illuminate\Cache` (`Repository::flexible`, `Lock`, `RateLimiter`, `MemoizedStore`).

**Current Coax Web pain:** `release.lock` is stored in the **public** GCS web bucket (exposing hostname, PID, and user at `https://openagents.com/release.lock`), rate limits are hardcoded in `auth.c`'s `LIMITS[]` enum, and external reads (Google `tokeninfo`, Nostr relay snapshots, `/status`) lack a shared cache/SWR primitive.

**Coax Web design:**
- Postgres `sys.cache` (`key`, `value BYTEA`, `created_at`, `expires_at`) and `sys.locks` (`key`, `owner`, `expires_at`), governed by `#authority` `lock-policy.coax`.
- **`cache_flexible(key, fresh_s, stale_s, compute)`**: returns stale bytes immediately during `[fresh_s, stale_s]` and refreshes asynchronously via `api_defer()` under a `sys.locks` lease (prevents thundering herds).
- **Per-request memoization** (cleared at `serve_one()` end) avoids redundant Postgres round trips for flags/account lookups within one request.
- **Named rate limiters** referenced declaratively from `routes/api.coax` (`Limit::perMinute(N)->by(account|ip|key)`).
- Move `release.lock` out of the public GCS bucket into `sys.locks` or a private ops bucket.

---

### 4.8 Unified Developer CLI (`coax web <cmd>`), Multiplexed Dev & `Prohibitable` Prod Guards — **P1**

**From:** `laravel/laravel` `artisan`, `composer run setup` / `dev`; `laravel/framework` `Console\Scheduling\Event` (`withoutOverlapping`, `onOneServer`, `evenInMaintenanceMode`, `pingOnFailure`), `Console\Prohibitable`.

**Current Coax Web pain:** 9+ separate scripts (`serve.sh`, `tool.sh`, `deploy.sh`, `release.sh`, `rollback.sh`, `test.sh --serve`, `admin.sh`, `schedules.sh`, `build.sh`).

**Coax Web design:**
Single CLI entrypoint `coax web <command>`:
- `setup` (verify LLVM/Postgres/assets, copy `.env.example`, migrate)
- `dev [APP]` (multiplexed static/Wasm hot-reload + local Postgres + `oa-api` + queue worker + structured logs in one terminal)
- `build [APP]` (replaces 657-line handwritten `apps/openagents-web/build.sh` with a declarative page manifest in `app.coax`)
- `routes [--json]` (unified web + API route table)
- `migrate [--status|--check|--fresh]` (with **`Prohibitable` guard**: refuses `--fresh` / destructive commands when `APP_ENV=production`)
- `work [--queue=...]` / `schedule:list | schedule:run` (with `sys.locks` overlap protection & failure pings)
- `test [--unit|--feature|--only ...]`
- `make:<page|route|policy|wire|migration|job>`
- `release | rollback | down | up | doctor`

---

### 4.9 Convention-Over-Configuration Coax Web App Skeleton — **P1**

**From:** `laravel/laravel` canonical folder structure (`app/`, `config/`, `database/`, `public/`, `routes/`, `tests/`).

**Current Coax Web pain:** `apps/openagents-web/` has 384 largely flat entries; `coax/web/api/` mixes 303 framework + `openagents.com`-specific files in one directory (including monolithic `auth.c` 125 KB, `player.c` 109 KB, `worlds.c` 91 KB).

**Coax Web design:** Separate generic framework (`coax/web/`) from each application (`apps/openagents-web/`, `gamebuilder/web/`):

```text
apps/openagents-web/
├── app.coax                  # Single app manifest (pages, routes, config, jobs)
├── .env.example              # Safe local dev defaults (zero secrets)
├── AGENTS.md                 # Concise agent guide pointing to `coax web inspect`
├── config/
│   ├── app.coax              # Typed env config + boot-time prod assertions
│   └── testing.coax          # Deterministic test overrides (fake mail/http/clock)
├── routes/
│   ├── web.coax              # Pages, redirects, /.well-known/*, /llms.txt
│   ├── api.coax              # Declarative API routes, auth, scopes, rate limits
│   └── console.coax          # Scheduled tasks & admin CLI commands
├── app/
│   ├── pages/                # One subfolder per page (entry.coax, face.c, *.wgsl, index.md)
│   ├── policies/             # Pure #authority *-policy.coax (0 surviving mutants)
│   ├── wire/                 # *-wire.coax codecs, validation & JSON Schema generation
│   ├── handlers/             # Thin C/Coax HTTP adapters (no domain logic)
│   ├── mail/                 # Typed Coax mail templates
│   └── jobs/                 # Async queue jobs + scheduled tasks
├── database/
│   ├── migrations/           # Immutable NNNN_*.sql + migrations.sha256
│   ├── schema.sql            # Canonical generated Postgres snapshot
│   ├── factories/            # Composable Coax test data builders
│   └── seeders/              # Local dev seeders
├── public/                   # Static root assets (favicon.svg, mark.svg, robots.txt)
└── tests/
    ├── unit/                 # Instant compile-time coax test + coax mutate
    └── feature/              # Savepoint/template-isolated HTTP + Postgres tests
```

---

### 4.10 Fixed Compile-Time Middleware Pipeline & Front-Gate Maintenance Mode — **P1/P2**

**From:** `Illuminate\Pipeline\Pipeline`, `Foundation\Http\Middleware\{PreventRequestsDuringMaintenance, ValidatePostSize, TrustProxies, TrustHosts}`.

**Current Coax Web pain:** Cross-cutting request checks (maintenance flags in `rooms.c`, origin checks, body size checks, `X-Forwarded-For` proxy trust, auth gate, response headers) are interleaved ad-hoc across `api.c`, `auth.c`, and individual route handlers.

**Coax Web design:**
- Define an explicit, zero-allocation **ordered middleware stage chain** in `oa-api` before dispatch:
  1. `trust_proxies` (strictly trust only GCP LB hop for client IP / `X-Forwarded-Proto`)
  2. `validate_size` (enforce per-route `REQ_MAX` before buffering large bodies)
  3. `log_context` (mint `request_id`, start timer, emit structured access log on exit)
  4. `maintenance_gate` (immediate `503` + `Retry-After` + bypass for `/up`, admins, and signed bypass cookie)
  5. `origin_and_csrf` (centralized `ap_site` / `auth_same_site` check)
  6. `auth_and_rate_limit` (single table lookup from `routes/api.coax`)
  7. `dispatch_handler` → `flush_response` → `run_deferred_callbacks`

---

### 4.11 Mail & Object-Storage Driver Abstractions — **P1/P2**

**From:** `Illuminate\Mail` (`log`, `array`, `resend`) and `Illuminate\Filesystem` (`local`, `s3`/GCS, `Storage::fake()`).

**Coax Web design:**
- **Mail:** `log` (dev console; forbidden in staging/prod), `capture` (test table/memory — kills `RESEND_PORT` Python fake), `resend` (async via `queue.jobs`). Render messages in pure Coax (`app/mail/*.coax`).
- **Storage:** Thin `local` vs `gcs` object-store adapter for `drops.c`, `realms.c`, and `verse_packs.c`, plus an in-memory/tempdir test driver (kills `GCS_PORT` and `METAVERSE_GCS_PORT` Python fakes in `test.sh`).

---

### 4.12 Stateless Signed URLs & Keyset Cursor Pagination — **P2** *(from `laravel/framework`)*

**From:** `Illuminate\Routing\UrlGenerator::temporarySignedRoute()` + `ValidateSignature`, and `Illuminate\Pagination\CursorPaginator`.

**Coax Web design:**
- **Stateless HMAC-SHA256 Signed URLs (`signed_url_make` / `signed_url_verify` in `#authority` Coax):** Uses the active versioned sealing key + expiry timestamp + constant-time HMAC check. Ideal for stateless unsubscribe links, temporary asset download URLs, and preview links without creating DB rows. (Keep single-use DB tokens where revocation/one-time spend is required, such as login/recovery links.)
- **Keyset Cursor Pagination (`cursor-wire.coax`):** Standardize opaque base64 `(created_at, id)` cursors on chat history, audit logs, project listings, and event streams — O(1) Postgres index scans, immune to duplicates/skips during live writes.

---

### 4.13 Fast Isolated Feature Testing, Factories, Fakes & Time Travel — **P1** *(deepened from `laravel/framework`)*

**From:** `laravel/laravel` `phpunit.xml` + `UserFactory`; `laravel/framework` `Testing\{TestResponse, Fluent\AssertableJson, Concerns\InteractsWithTime}`, `Http::fake()`, `Queue::fake()`, `Mail::fake()`, `Lottery::forceResultWithSequence()`.

**Current Coax Web pain:** `coax/web/api/test.sh` is 632 lines, spins up ~12 fake Python sidecar ports, and takes ~16 minutes (`py:721s`), whereas existing Coax API tests (`coax/web/api/tests/*.coax`) already run in 1–6 seconds each.

**Coax Web design:**
1. **Two-tier pyramid:** `coax web test --unit` (pure `#test` + `#forall` + `coax mutate`, zero DB) and `coax web test --feature` (HTTP + Postgres using `BEGIN ... ROLLBACK` or `CREATE DATABASE ... TEMPLATE` isolation).
2. **`config/testing.coax`:** Switches on `mail.driver=capture`, `queue.driver=sync`, `storage.driver=local`, `http.prevent_stray=true`, and an **injectable test clock (`clock_travel_ms`)** so 5-minute L2-fresh windows, 24-hour wallet limit delays, and 72-hour account recovery tests run in **milliseconds** instead of waiting or mutating SQL timestamps by hand.
3. **Coax Factories (`database/factories/`)** + fluent Coax JSON/HTTP test assertions (`assert_status`, `assert_json_path`, `assert_mail_sent`, `assert_job_pushed`).
4. **Target:** Cut full API CI time from ~16 minutes to **<60 seconds** as retired routes and `test_*.py` suites drop off during the zero-base reset.

---

### 4.14 First-Class Agentic Introspection (The "Laravel Boost" Pattern) — **P1**

**From:** `laravel/laravel` `AGENTS.md`/`CLAUDE.md` + `laravel/boost` (15+ MCP tools); `laravel/framework` `JsonSchema`.

**Coax Web design:** Build native, zero-dependency agent introspection directly into `coax web`:
- `coax web routes --json` — full route + auth + rate-limit + wire-schema graph.
- `coax web schema --json` — parsed tables, columns, indexes, and constraints from `schema.sql`.
- `coax web policies --json` — every `#authority` policy, `#test`/`#forall` counts, and mutation status.
- `coax web config --schema` — every typed config key, type, default, and prod guard (zero secret values).
- `coax web logs --tail N --level warn` — filtered structured JSON logs from `coax web dev`.
- Ship a concise `AGENTS.md` in every Coax Web app skeleton so coding agents (`coder/`, OpenAgents Desktop, Antigravity) discover the entire app contract in one command.

---

## 5. What to Explicitly REJECT from Laravel (and Why)

Roughly **80% of `laravel/framework`'s 279,381 LOC** should **not** be ported to Coax Web:

1. **Eloquent ActiveRecord ORM (`Database/Eloquent`, ~40k+ LOC)**: Hides SQL behind magic properties, enables N+1 lazy-loading bugs, couples persistence to domain rules, and violates Coax's explicit data-flow and `#authority` purity. *If `PQexecParams` boilerplate in C becomes painful, build a compile-time typed SQL query generator (sqlc-style `.sql` → Coax/C structs), never ActiveRecord.*
2. **Runtime Service Container, Reflection, Facades & `Macroable` (`Container`, `Reflection`, `Macroable`, `Contracts` — ~9.5k LOC)**: Breaks static call-graph analysis (`coax deps`, `coax mutate`) and adds runtime indirection to an AOT compiled systems language. Use explicit struct wiring in `app.coax`.
3. **Blade SSR Templates, Vite 8, npm & Tailwind CSS (`View` — 8,992 LOC + JS toolchain)**: Violates `coax/web.md`'s zero-npm, zero-bundler invariant and regresses from `serve.c`'s 118–135 ms Wasm hot-swap. Keep `components/backend/dom.coax` + WebGPU islands + pure Coax HTML/Markdown emitters for text routes.
4. **PHP `Collections` (8,557 LOC) & `Conditionable` (181 LOC)**: Workarounds for PHP array ergonomics; Coax slices and direct loops are faster and allocation-free.
5. **Password-First Auth & Legacy Session/Encryption/Hashing (`Auth`, `Session`, `Hashing`, `Encryption`, `Cookie` — ~11k LOC)**: Coax Web's passwordless/NIP-98/passkey/L2-fresh/agent-scope auth and `auth-seal.coax` AEAD are already strictly more modern and formally verified.
6. **`Broadcasting` (3,534 LOC) & `Redis` (3,406 LOC)**: Redundant with Coax Web's native WebSockets, Nostr relays, and zero-Redis Postgres architecture.
7. **`Translation` (1,853 LOC) & `Image` (1,402 LOC)**: YAGNI for `openagents.com`'s zero-base scope.
8. **SQLite-by-Default for Production API**: Keep Postgres as the sole production store (`FOR UPDATE SKIP LOCKED`, advisory migration locks, `jsonb`, `oa.allow_ddl` DDL role separation).

---

## 6. Phased Implementation Plan for Coax Web & `openagents.com`

Sequenced to land directly on top of the **`openagents.com` zero-base reset** (`docs/audits/2026-11-20-openagents-com-live-surface-audit.md`), avoiding refactor work on retired routes:

### Phase 0 — Naming & Contract Lock (1–2 days)
- Approve the rename **Coax Native Web → Coax Web** across `coax/web.md`, `apps/openagents-web/README.md`, and `docs/REPO-MAP.md` (keeping `coaxial_*` Wasm exports and GCP resource names unchanged).
- Lock the canonical Coax Web app skeleton (Section 4.9) for both `openagents.com` and `gamebuilder.org`.

### Phase 1 — Single Source of Truth: Routes, Config, Context & HTTP Client (2 weeks)
- Build `#authority` `routes/api.coax` + `routes/web.coax` generating `oa-api` dispatch/`RULES[]`, `serve.c` site routes, `lb.sh` parity checks, `GET /up`, and live `GET /api/openapi.json`.
- Build `config/app.coax` + `.env.example` + `oa-api --config-check` (eliminating 143 scattered `getenv()` calls).
- Introduce `LogContext` structured JSON logging (upgrading 318 `api_log()` calls) and the ordered compile-time middleware pipeline.
- Build the unified bounded outbound `http_client` + `http-client-policy.coax` + `http_fake` / `prevent_stray_requests` (consolidating 25 `curl_easy_init()` call sites across 14 C files and eliminating fragile `strstr` JSON scraping in favor of `*-wire.coax` codecs).

### Phase 2 — Postgres Zero-Infra Primitives & Transactional Safety (2 weeks)
- Add Postgres migrations for `queue.jobs`, `queue.job_batches`, `queue.failed_jobs`, `sys.cache`, and `sys.locks`.
- Implement `#authority` `queue-policy.coax` (including `WithoutOverlapping`, `ThrottlesExceptions`, `RateLimited`), `lock-policy.coax`, and `oa-api --work`.
- Add `db_tx_after_commit()` and post-response `api_defer()` hooks, plus `cache_flexible()` (SWR) and per-request memoization.
- Move `email.c` delivery onto `queue.jobs` with `log` / `capture` / `resend` drivers; move `release.lock` out of the public GCS bucket; wire front-gate maintenance mode (`503` + `Retry-After` + bypass).

### Phase 3 — Unified `coax web` CLI, Declarative Build & Fast Test Runner (2 weeks)
- Ship `coax web {setup,dev,build,routes,migrate,work,schedule,test,make,release,rollback,down,up,doctor}` with `Prohibitable` production guards and multiplexed `coax web dev`.
- Replace the 657-line handwritten `apps/openagents-web/build.sh` with a declarative page manifest (`app.coax`) driven by `coax web build`.
- Ship `config/testing.coax`, injectable test clock (`clock_travel_ms`), Coax factories (`database/factories/`), storage/mail/http fakes, and savepoint/template-isolated Coax feature tests (targeting <60 s full API CI as legacy `test_*.py` suites retire).

### Phase 4 — Framework/App Split, Polish & Agentic Introspection (2 weeks)
- Separate generic Coax Web framework (`coax/web/`) from `openagents.com` (`apps/openagents-web/`) and validate against `gamebuilder.org` as the second consumer.
- Add stateless HMAC `signed_url` helper and `cursor-wire.coax` keyset pagination for history/audit/list routes.
- Ship `coax web inspect` / MCP tools (`routes --json`, `schema --json`, `policies --json`, `config --schema`, `logs --tail`) and the skeleton `AGENTS.md`.

---

## 7. Owner Decisions Requested

1. **Rename:** Confirm renaming **Coax Native Web → Coax Web** (preserving `coaxial_*` Wasm ABI/exports unchanged).
2. **Phase 1–2 priority order:** Confirm landing Routes + Config + `LogContext` + unified `http_client` + Postgres `queue.jobs`/`sys.locks`/`afterCommit` directly on the zero-based `openagents.com` surface.
3. **Second reference app:** Confirm validating the extracted Coax Web skeleton against both `openagents.com` and `gamebuilder.org`.
4. **Typed SQL layer (post-Phase 4):** Decide whether to add a small compile-time typed SQL query generator (sqlc-style `.sql` → Coax/C structs) to replace the bulk of the 551 manual `PQexec*()` calls across 43 C files while keeping zero ORM magic.
