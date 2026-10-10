# Retail, town, workbench & misc crates

**Scope [MSC]:** `crates/{retail-cloud, retail-service, retail-qualify, bunny-rules, townsfolk, town-clock, compute-workbench, contribution-service, contribution-workbench, workbench, workbench-session}`, plus `mc-bridge/`, `os/` and `packages/coder-defaults`.
**Health grade: B**
**Audit date:** 2026-10-10. **Snapshot:** `3168c986aa11e18a8bd30f52c270f609e49b3815`.

This area has about 46.5k lines of Rust in 11 workspace crates. It also includes mc-bridge (a separate nightly workspace, 1.1k LOC), os/ (about 8.4k lines of Nix and shell) and packages/coder-defaults, which holds data only. Every crate is live. Each one has workspace dependents, ships a binary that `deploy/retail/build.sh` builds, or is mounted by terminal-app or verse. The one exception is contribution-service, which has no dependents: it is an operator-only binary still waiting on an owner step (NEEDS_OWNER.md:1436). The code is very young. Almost all commits fall between 2026-10-06 and 2026-10-09, mc-bridge was last touched 2026-09-21, and most crates have 2 to 8 commits. Safety is handled carefully. Gates fail closed, plans are bound to digests, private files use O_NOFOLLOW and mode 0600, and the HTTP edge zeroes secrets and rejects requests that carry an Origin header. There are more than 270 tests, and every `docs/` link in the area resolves.

The weak spots are mainly concurrency and scale in the retail stack (retail-cloud, retail-service, retail-qualify, compute-workbench; about 26k LOC, all switched off until the owner confirms). One global store mutex stays held during Lightning wallet and Boat network calls in both the HTTP path and the worker, so the service effectively handles one customer at a time. Journal reads are N+1 queries over the full history with no index on account, and they run on every worker tick. The compute account view silently shows only the oldest 64 executions. Through retail-qualify, the production retail-service binary links the qualification harness, the simulator and terminal UI crates. Some code is duplicated across crates: private-file ownership checks are reimplemented in many places and the copies have drifted, and pane text bounding and digest formatting are also copied. Two god functions remain (Service::call and contribution verify). Errors are erased on purpose but never logged. mc-bridge and the os/ shell tests are untested or not wired into anything that runs. The grade is B: the intent and test discipline are strong, and the layering and scalability debt can be fixed before paid availability opens.

## Measurements

| Unit | Rust LOC (incl. tests) | Largest files | `#[test]` | Dependents / binary |
|---|---|---|---|---|
| retail-cloud | 11,546 (36 files) | environment.rs 1,587; boat.rs 891; fake.rs 688 | 78 | 5 |
| retail-service | 7,043 | tests.rs 2,256; lib.rs 922; package.rs 739 | 39 | 2; bin `retail-service` |
| retail-qualify | 4,084 | sim.rs 1,015 | 13 | 3; bin `retail-qualify` |
| bunny-rules | 5,013 | game.rs 2,149 | 48 | 1 (bunny-web) |
| townsfolk | 4,830 | files.rs 739 | 23 | 3 |
| town-clock | 705 | lib.rs | 12 (see Refuted) | 4 |
| compute-workbench | 3,379 | tests/retail.rs 1,046; retail/mod.rs 974; host.rs 797 | 16 | 4; bin `retail-client` (`client` feature) |
| contribution-service | 3,737 | tests.rs 1,532; evaluate.rs 802 | 19 | 0; bin `contribution-service` |
| contribution-workbench | 2,692 | | 12 | 3 |
| workbench | 2,306 (+959-line JSON fixture) | | 13 | 13 |
| workbench-session | 1,194 | | 11 | 2 |
| mc-bridge | 1,078 (single main.rs) | | 0 | separate nightly workspace |
| os/ | about 8.4k lines, 55 files | bin/screen-record 1,224; modules/coderos/desktop.nix 1,126 | 9 manual-only `os/tests/*.sh` | |
| packages/coder-defaults | 6 data files | | | |

Other metrics:

- TODO/FIXME: 0 in every crate. The 11 hits in os/ are `mktemp XXXXXX` patterns.
- `#[allow]`: 2, at retail-cloud environment.rs:1065 and :1136.
- Non-test unwrap/expect counts are low. Most of the raw counts (compute-workbench 51, contribution-workbench 25, townsfolk 25) are inside inline `#[cfg(test)]` modules.
- `unsafe libc::geteuid`: 71 calls in 38 files repo-wide, 13 of them in this area.
- `map_err(|_|` error erasures: retail-service 66, contribution-service 94, retail-cloud 18, compute-workbench 16.
- Logging statements: 1 in retail-service and 1 in contribution-service.
- Longest functions: `Service::call` about 453 lines (retail-service lib.rs:384); `evaluate::verify` 369 (contribution-service evaluate.rs:361); worker `advance` 226 (retail-service worker.rs:107); `sim::route` 142 (retail-qualify).
- SQLite in retail-cloud: 16 `CREATE TABLE IF NOT EXISTS`, 0 `CREATE INDEX`, no `user_version`, no migrations.
- `civil_from_days` copies repo-wide: 56 files.

## Strengths

- **Retail fails closed throughout.** Paid availability stays off until the owner review and a funded qualification are done ([lib.rs:10-12](../../../../crates/retail-cloud/src/lib.rs)). `Gate::open` in environment.rs checks the plan digest and the qualification receipt. retail-qualify labels fake and simulated receipts so they can never count (qualify.rs:239, 382).
- **Private state is handled consistently, even though the code is duplicated.** The journal is created with mode 0600 and `create_new`, `secure_delete` is on, a busy timeout is set, and a custody fence is re-checked inside every immediate transaction ([journal.rs:42-136](../../../../crates/retail-cloud/src/journal.rs)). retail-service and contribution-service both use O_NOFOLLOW and nlink==1 checks.
- **The HTTP edge is conservative.** It has a 32-slot semaphore that returns 429 when full and a body limit. It rejects requests that carry an Origin header, sets no-store, runs work in `spawn_blocking`, zeroes the customer bearer secret after use, and maps typed refusals to stable error codes ([http.rs](../../../../crates/retail-service/src/http.rs)).
- **Worker health checks for staleness.** The last tick must be ok and newer than `poll_seconds*3+30` ([package.rs:312-319](../../../../crates/retail-service/src/package.rs)), and operator status exposes this.
- **External collaborators are traits.** Providers, wallets, sandboxes and task owners are traits, so the fakes in retail-cloud/src/fake.rs and the HTTP-level Boat simulator in retail-qualify/src/sim.rs exercise the real adapters. The script that runs on the sandbox has its own test ([owner_script.rs](../../../../crates/retail-cloud/tests/owner_script.rs)).
- **Test coverage is highest where money moves.** retail-cloud has 78 tests in 12 integration files covering offer, reserve, provision, dispatch, meter, cancel, settle, recover, retain, topup, environment and authority. retail-service/tests/process.rs adds process-level tests.
- **workbench and workbench-session are small contracts with few dependencies.** They use serde only, plus coder-pty for the session crate. Non-test code has no unwraps, uses `deny_unknown_fields` and enforces explicit bounds (TOKEN_MAX, SUMMARY_MAX, LIST_MAX). Both ship a contract fixture and tests.
- **bunny-rules, town-clock and townsfolk are pure and deterministic.** The caller passes in the time and the seed, so they are wasm-friendly and give the same results on every device.
- **Docs links are accurate.** Every `docs/*.md` path cited by these crates exists, and the crate-level docs explain purpose, invariants and issue numbers.
- **os/ has real checks.** It wraps its bin/ scripts with `writeShellApplication`, which runs shellcheck and sets strict mode. flake.nix has evaluation checks (`extension-points`, `coder-host-stands-down`) that test generated config line by line.
- **mc-bridge documents its unusual setup.** Its Cargo.toml explains why it is a separate nightly workspace and gives the reason for each pinned pre-release crypto crate.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| MSC-01 | Medium | concurrency | Global store mutex is held during wallet and Boat network calls, so customers are served one at a time | L |
| MSC-02 | Medium | correctness | Compute account view silently drops everything after the 64 oldest executions | S |
| MSC-03 | Medium | architecture | Production retail-service links the qualification harness, simulator, fakes and terminal UI; there is also a dev-dependency cycle | M |
| MSC-04 | Medium | performance | `Journal::all_funded` is an N+1 full-history scan with no account index, run every tick | M |
| MSC-05 | Medium | maintainability | Money journal has no schema versioning or migrations | S |
| MSC-06 | Medium | duplication | Private file and directory checks are reimplemented in at least 5 crates, and some copies have drifted | M |
| MSC-07 | Medium | error-handling | Worker and startup failures are erased and never logged | S |
| MSC-08 | Low | concurrency | BoatAdapter persistence can lose updates; it uses a fixed tmp name and never prunes intents | S |
| MSC-09 | Low | maintainability | God functions: `Service::call`, worker `advance`, contribution `verify` | M |
| MSC-10 | Low | duplication | Pane detail sanitizing and truncation is copied about 5 times | S |
| MSC-11 | Low | maintainability | Digests are passed as strings in two formats | S |
| MSC-12 | Low | testing | os/ script tests are manual-only; 8 bin scripts have no test | M |
| MSC-13 | Low | repo-hygiene | mc-bridge is an untested, unchecked nightly-only workspace | M |
| MSC-14 | Low | architecture | About 30k LOC of gated-off retail and contribution code has no plan for keeping it current | S |
| MSC-15 | Low | security | townsfolk builds file paths from unvalidated villager ids; its dependency docs are stale | S |
| MSC-16 | Low | security | Operator endpoint does not zero its bearer secret | S |
| MSC-17 | Low | duplication | `civil_from_days` date math is copied again (56 copies repo-wide) | M |
| MSC-18 | Low | performance | bunny-rules re-parses a level on every `garden()` call | S |
| MSC-19 | Low | maintainability | Small hygiene items: stale clippy allow, a bool authority flag, misleading package.json, dependency versions not centralized | M |

### MSC-01 Global store mutex held across Lightning wallet and Boat network calls

**Severity:** Medium · **Category:** concurrency · **Effort:** L

**Locations:**
- [lib.rs:89](../../../../crates/retail-service/src/lib.rs), lib.rs:388, lib.rs:437, lib.rs:727 (retail-service)
- [worker.rs:29](../../../../crates/retail-service/src/worker.rs), worker.rs:31
- [http.rs:38](../../../../crates/retail-service/src/http.rs)
- [boat.rs:259](../../../../crates/retail-cloud/src/boat.rs)
- [deploy/retail/Caddyfile](../../../../deploy/retail/Caddyfile)

**Evidence:** The store is declared as `store: Mutex<Store>` (lib.rs:89). `call` takes `self.lock()?` at lib.rs:388 and keeps it through `topup::request_top_up(&mut store.ledger, &*self.wallet, ..)` (lib.rs:437) and `dispatch::observe(&store.journal, &*self.backend, ..)` (lib.rs:727). BoatAdapter calls `self.runtime.block_on(self.client.read_text(..))` (boat.rs:259). The worker's `tick` locks at worker.rs:29 and keeps the lock through `topup::reconcile(.., &*self.wallet, ..)` (worker.rs:31), up to STEP_MAX `advance` calls, and `retain::worker_step` with the backend (worker.rs:98). The HTTP layer has a 32-permit semaphore that returns 429 "busy" (http.rs:38-40, 66-75). deploy/retail/Caddyfile reverse-proxies a public origin to 127.0.0.1:9042, so this is a shared service with many customers.

**Impact:** One slow Boat or wallet round-trip blocks every customer's Account, Capacity, Progress and Cancel call, and each worker tick blocks all requests while it runs. Once paid availability opens, this is the main throughput limit. The service is gated closed today. The coarse lock is partly a deliberate safety simplification: docs/cloud/retail-service.md:120-122 describe fencing descriptors "before effects and after blocking collaborator calls".

**Suggested action:**
1. Split each operation into three phases. (a) Under the lock: authenticate, read the funded record and write the journal intent. (b) Release the lock and do the remote wallet or Boat I/O. (c) Lock again, re-run the custody fence (`store.check`) and record the observation idempotently.
2. Start with the read-only observations (`dispatch::observe` for Progress, `recover::observe`) and `topup::reconcile`.
3. Change `Worker::tick` to take and release the lock per execution instead of once per tick.
4. Verify: add a retail-service test with a Backend whose `observe` sleeps for 2s, and assert that a concurrent Account call returns in under 100ms.
5. Finish this before the MSC-08 fix becomes reachable, because narrowing the lock exposes that race.

### MSC-02 The compute account view silently drops every receipt after the 64 oldest executions

**Severity:** Medium · **Category:** correctness · **Effort:** S

**Locations:**
- [host.rs:42](../../../../crates/compute-workbench/src/host.rs), host.rs:46, host.rs:180, host.rs:352 (compute-workbench)
- [offer.rs:307](../../../../crates/retail-cloud/src/offer.rs), offer.rs:315

**Evidence:** `read_observing` iterates `journal.all_funded()?.into_iter().filter(|f| f.account == principal.account).take(64)` (host.rs:42-46). `all_funded` is documented as "Every funded request, oldest first" and runs `ORDER BY confirmed_at, execution` (offer.rs:307, 315), so the view keeps the 64 oldest. Top-ups use `.take(64)` (host.rs:180). `mount_products` takes `account.receipts.iter().take(16)` (host.rs:352).

**Impact:** After an account's 65th funded execution, new executions, holds and settlement receipts never appear in the desktop or Verse compute panes, and nothing tells the user the list was cut. The money state itself is still correct.

**Suggested action:**
1. Add `Journal::funded_for_account(account, limit, before: Option<cursor>)` to crates/retail-cloud/src/offer.rs, using `WHERE account=? ORDER BY confirmed_at DESC, execution DESC LIMIT ?`.
2. Add `CREATE INDEX IF NOT EXISTS funded_account ON funded(account, confirmed_at)` to the journal.rs SCHEMA.
3. Use the new query in `read_observing`, and add `truncated: bool` to `Account` so the pane can show "showing the newest 64".
4. Apply the same change to `ledger.top_ups`.
5. Verify: add a host.rs test that funds 70 executions and asserts that the newest one is present and the flag is set.

### MSC-03 The production retail-service binary links the qualification harness, simulator, fakes and terminal UI crates, and there is a dev-dependency cycle

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:**
- [retail-service/Cargo.toml:21](../../../../crates/retail-service/Cargo.toml)
- [retail-qualify/Cargo.toml:17](../../../../crates/retail-qualify/Cargo.toml), [retail-qualify/src/lib.rs:11](../../../../crates/retail-qualify/src/lib.rs)
- [compute-workbench/Cargo.toml:11](../../../../crates/compute-workbench/Cargo.toml), Cargo.toml:37
- [retail-cloud/src/lib.rs:40](../../../../crates/retail-cloud/src/lib.rs)
- [deploy/retail/build.sh:22](../../../../deploy/retail/build.sh)

**Evidence:** retail-service has a normal dependency on retail-qualify (Cargo.toml:21). Its non-test code uses it in only two places: lib.rs:26 (`launch::{self, Gate}`) and package.rs:14, 222, 474 (`bindings::BOAT_API`, `qualify::RECEIPT_SCHEMA`). retail-qualify publicly exports `acceptance`, `harness` and `sim` (lib.rs:11-17). It depends on compute-workbench with the `host` feature (Cargo.toml:17), and `host = [pay-ledger, retail-cloud, terminal-core]` (compute-workbench Cargo.toml:11). compute-workbench dev-depends on retail-service and retail-qualify (Cargo.toml:37-38). `pub mod fake` lives in non-test retail-cloud code (lib.rs:40). deploy/retail/build.sh builds `-p retail-service -p retail-qualify ...` in release mode.

**Impact:** The deployed money service carries simulator, fake and terminal code. That means a larger surface to audit, slower release builds, and production code that is able to construct fakes. The dev-dependency cycle makes the build graph harder to follow.

**Suggested action:**
1. Move `launch.rs`, `QualificationReceipt`/`RECEIPT_SCHEMA` and `bindings::BOAT_API` into a new `retail-launch` crate, or behind a default `gate` feature of retail-qualify. Make retail-service depend only on that.
2. Put retail-qualify's `harness`, `sim` and `acceptance` behind a `simulation` feature that its binary and tests enable.
3. Put `retail_cloud::fake` behind a `fake` feature that is enabled only from dev-dependencies.
4. Split compute-workbench's `host` feature into `ledger` (read/read_observing) and `mount` (terminal-core).
5. Verify: `cargo tree -p retail-service -e normal | rg 'terminal-core|coder-vt'` returns nothing.

### MSC-04 Journal::all_funded is an N+1 full-history scan with no index on account, run every worker tick and on every listing

**Severity:** Medium · **Category:** performance · **Effort:** M

**Locations:**
- [offer.rs:312](../../../../crates/retail-cloud/src/offer.rs)
- [journal.rs:14](../../../../crates/retail-cloud/src/journal.rs)
- [worker.rs:92](../../../../crates/retail-service/src/worker.rs)
- [lib.rs:699](../../../../crates/retail-service/src/lib.rs) (retail-service)
- [retain.rs:411](../../../../crates/retail-cloud/src/retain.rs)

**Evidence:** `all_funded` selects every execution id and then calls `self.funded(id)` once per id (offer.rs:312-321). The `funded` table (journal.rs:14-26) has PRIMARY KEY `offer` and UNIQUE `request`/`execution`, which create automatic indexes, so each per-id lookup is indexed. Nothing indexes `account`, and `rg` finds 0 `CREATE INDEX` in crates/retail-cloud/src. The function is called on every worker tick (worker.rs:92-97) and in `retain::worker_step` (retain.rs:411). The Executions listing calls it too: it loads every account's records, filters in Rust, sorts and truncates to STEP_MAX (lib.rs:699-708). All of these calls run under the global store lock (MSC-01).

**Impact:** The cost of each tick and each listing grows linearly with the lifetime number of executions across all customers, and that work happens inside the critical section.

**Suggested action:**
1. Replace the N+1 with a single `SELECT offer,request,execution,account,... FROM funded ORDER BY confirmed_at, execution` and decode the rows in one pass.
2. Add `CREATE INDEX IF NOT EXISTS funded_account ON funded(account, execution)`.
3. Move the Executions filter into SQL: `WHERE account=? AND execution>? ORDER BY execution LIMIT ?`.
4. Have the worker and retain walk only non-terminal executions (for example with `WHERE NOT EXISTS` against the cleanup and settle tables) instead of the full history.
5. Verify: existing retail-cloud and retail-service tests pass, and a test with many settled executions shows the tick touching only the open ones.

### MSC-05 The money journal has no schema versioning or migrations

**Severity:** Medium · **Category:** maintainability · **Effort:** S

**Locations:**
- [lib.rs:86](../../../../crates/retail-cloud/src/lib.rs) (retail-cloud)
- [journal.rs:14](../../../../crates/retail-cloud/src/journal.rs), journal.rs:82

**Evidence:** `EXTRA_SCHEMAS` lists 9 module SCHEMA strings (lib.rs:86-96), and journal.rs has its own SCHEMA as well. `initialize` runs them with `execute_batch` (journal.rs:82-87). Together they contain 16 `CREATE TABLE IF NOT EXISTS` statements. `rg` finds no `user_version` or `ALTER TABLE` in retail-cloud, retail-service or contribution-service.

**Impact:** A column or constraint change silently has no effect on existing journal files, and a backup cannot tell which schema generation it holds. Adding versioning is cheapest before the first paid launch.

**Suggested action:**
1. Add `PRAGMA user_version` handling to `Journal::initialize`, driven by an ordered `MIGRATIONS: &[&str]` list. v1 is today's concatenated schema. Apply migrations inside an immediate transaction.
2. Refuse to open a journal whose version is newer than the binary.
3. Record the schema version in backup manifests.
4. Verify: add a test that migrates a checked-in v1 fixture and a test that refuses a future version.

### MSC-06 Private file and directory ownership checks are reimplemented in at least 5 crates, and some copies have drifted

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [store.rs:26](../../../../crates/retail-service/src/store.rs), store.rs:42, store.rs:63 (retail-service)
- [package.rs:166](../../../../crates/retail-service/src/package.rs)
- [lib.rs:90](../../../../crates/contribution-service/src/lib.rs), lib.rs:96 (contribution-service)
- [boat.rs:850](../../../../crates/retail-cloud/src/boat.rs)
- [private-fs/src/lib.rs:1](../../../../crates/private-fs/src/lib.rs)

**Evidence:** retail-service store.rs has `private_dir`, `check_path` and `private_regular` (lines 26-75), and package.rs:166 has `private_stamp`. contribution-service has its own `private_regular` (lib.rs:96-108). retail-cloud has `save_private` at boat.rs:850. The copies have drifted. retail-service's `check_path` rejects `..` and propagates metadata errors other than NotFound (store.rs:42-61). contribution-service's ancestor walk uses `symlink_metadata(&prefix).is_ok_and(|m| m.is_symlink())` (lib.rs:90), which ignores errors. Outside psionic, the repo has 71 `libc::geteuid` occurrences in 38 files. private-fs only does anything on Windows ("On every other platform the crate is empty", lib.rs:24).

**Impact:** Each hardening fix has to be applied in many places, and the copies keep drifting apart. The repeated `unsafe` geteuid calls add noise to any audit.

**Suggested action:**
1. Add a Unix module to crates/private-fs containing: a safe `euid()`; `check_no_symlink_ancestors(path)`, which rejects `..` and propagates errors other than NotFound; `is_private_regular(&Metadata)`; `create_private_dir`; `open_private` (O_NOFOLLOW|O_NONBLOCK); and `atomic_write_private`.
2. Port retail-service store.rs, contribution-service lib.rs, retail-cloud boat.rs/journal.rs, retail-qualify bound.rs and compute-workbench retail/private.rs to the new module. Keep each crate's own error mapping.
3. Move the existing symlink and shared-directory tests into private-fs.
4. Verify: `rg 'libc::geteuid' crates/{retail-*,contribution-*,compute-workbench}` returns only private-fs, and all moved tests pass.

### MSC-07 Worker and startup failures are erased and never logged

**Severity:** Medium · **Category:** error-handling · **Effort:** S

**Locations:**
- [worker.rs:434](../../../../crates/retail-service/src/worker.rs), worker.rs:66, worker.rs:79
- [bin/retail-service.rs:166](../../../../crates/retail-service/src/bin/retail-service.rs)
- [http.rs:57](../../../../crates/retail-service/src/http.rs)

**Evidence:** The worker loop keeps only `(now, result.as_ref().is_ok_and(|r| r.failed.is_empty()))` (worker.rs:434-438). `failed` holds only ids or "environments" and throws the error away (`Err(_) => { report.failed.push(id.clone()) }`, worker.rs:79-82). retail-service has 1 `eprintln` in total, contribution-service has 1, and neither uses tracing or log. At startup, every failure becomes "retail runtime configuration or custody is unavailable" (bin/retail-service.rs:166). `spawn_blocking` join errors become Unavailable (http.rs:57, 112).

**Impact:** When settlement, teardown or top-up reconciliation fails, the operator sees only `last_ok: false`, with no execution id and no cause.

**Suggested action:**
1. Keep hiding the details from customers.
2. In worker.rs, change `Err(_)` to capture `(id, error.to_string())` and write it to stderr from the worker thread.
3. Keep a bounded ring of the last 32 failures in `package::Operating`, and expose it through `/operator/status`.
4. In bin/retail-service.rs, `eprintln` the underlying error before mapping it.
5. Verify: add a test where a failing backend's id and cause appear in `operator_status`.

### MSC-08 BoatAdapter state persistence can lose updates

**Severity:** Low · **Category:** concurrency · **Effort:** S

**Locations:**
- [boat.rs:225](../../../../crates/retail-cloud/src/boat.rs), boat.rs:203, boat.rs:850

**Evidence:** `update` clones the index, calls `drop(index)` and then `self.save(&copy)` (boat.rs:225-233), so two concurrent saves can land on disk out of order. `save_private` always writes to `path.with_extension("json.tmp")`, removing and recreating it if it already exists (boat.rs:856-875). `remember_creation` inserts into `intents`, and nothing ever removes entries: every reference to `intents` is at boat.rs:60-223 or 407, and none is a remove or retain. `remember_creation` does keep its lock while saving.

**Impact:** The bug is latent today because retail-service serializes every backend call behind its store mutex. Once MSC-01 narrows that lock, it becomes a real race that can lose deletions. intents.json grows without bound and is rewritten in full on every create.

**Suggested action:**
1. In `update`, keep the index guard alive across `self.save(&index)`, as `remember_creation` already does.
2. Use a unique tmp file name (`json.tmp.<pid>.<counter>`).
3. Prune intents older than `READY_DEADLINE_SECS` once the create resolves.
4. Verify: add a test with 2 threads making 100 updates each, and assert that the on-disk index matches memory. (`save_private` already fsyncs the parent directory, at boat.rs:878-884.)

### MSC-09 God functions: Service::call (about 453 lines), worker advance (226) and contribution verify (369)

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [lib.rs:384](../../../../crates/retail-service/src/lib.rs) (retail-service)
- [worker.rs:107](../../../../crates/retail-service/src/worker.rs)
- [evaluate.rs:361](../../../../crates/contribution-service/src/evaluate.rs)

**Evidence:** `Service::call` runs from lib.rs:384 to 836 as one match with inline `json!` responses, plus a separate `need` match (lib.rs:390-398). `advance` runs from worker.rs:107 to 332. `evaluate::verify` runs from 361 to 729. Each function is linear rather than tangled, and test coverage is good.

**Impact:** Adding a request type means editing several places in one function. The response shapes are untyped JSON, which the compute-workbench client parses on its own.

**Suggested action:**
1. Add `impl Request { fn need(&self) -> Need }` in types.rs.
2. Move each match arm into a method in a new handlers.rs.
3. Replace the `json!` responses with Serialize response structs shared with compute-workbench's default-feature types.
4. Split `verify` into `verify_frozen`, `verify_current`, `verify_corpus`, `verify_trials` and `verify_consent`.
5. Verify: the existing retail-service, compute-workbench and contribution-service tests pass unchanged.

### MSC-10 Pane detail sanitizing and truncation is copied about 5 times with different magic limits

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [compute-workbench/src/host.rs:304](../../../../crates/compute-workbench/src/host.rs)
- [contribution-workbench/src/host.rs:741](../../../../crates/contribution-workbench/src/host.rs), [lib.rs:112](../../../../crates/contribution-workbench/src/lib.rs), [quests.rs:46](../../../../crates/contribution-workbench/src/quests.rs), quests.rs:62
- [workbench/src/pane.rs:268](../../../../crates/workbench/src/pane.rs), [workbench/src/lib.rs:34](../../../../crates/workbench/src/lib.rs)

**Evidence:** workbench rejects detail longer than `SUMMARY_MAX` (2048) or containing any control character other than `\n` (pane.rs:268-269). compute-workbench host.rs:304-310 re-implements that filter with a hard-coded 2048. contribution-workbench host.rs:741 truncates at 2048 without filtering control characters. contribution-workbench lib.rs:112-123 uses 2048/1980, and quests.rs uses 200 per line (line 46), 2048/2000 (lines 62-67) and `clean()` (line 73).

**Impact:** The copies match the validator today. If `SUMMARY_MAX` or the control-character rule changes, the producers will drift and emit detail that workbench rejects, and the pane will show Missing.

**Suggested action:**
1. Add `pub fn bounded_detail(text: &str) -> String` to crates/workbench/src/pane.rs, built on `SUMMARY_MAX` and the validator's own predicate.
2. Replace the copies in compute-workbench and contribution-workbench.
3. Verify: add a test (property-style or with crafted inputs) that `bounded_detail(x)` always passes `PaneDescriptor::check`.

### MSC-11 Digests are passed around as strings in two different formats

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [retail-cloud/src/lib.rs:100](../../../../crates/retail-cloud/src/lib.rs)
- [material.rs:49](../../../../crates/retail-cloud/src/material.rs)
- [contract.rs:88](../../../../crates/retail-cloud/src/contract.rs)
- [environment.rs:135](../../../../crates/retail-cloud/src/environment.rs)
- [contribution-workbench/src/host.rs:770](../../../../crates/contribution-workbench/src/host.rs)

**Evidence:** `sha256_hex` returns bare hex (lib.rs:100-105), and `CustomerSecret::digest()` returns that format (material.rs:49). contract.rs:88 and environment.rs:135 return `digest_of(..).to_string()`, which includes a prefix. contribution-workbench strips the prefix by hand with `.trim_start_matches("sha256:")` (host.rs:770-773).

**Impact:** If the two formats are mixed, the values compare unequal. That fails closed, but the resulting refusals are confusing, and the types give no help in catching the mix-up. No mismatch bug is known.

**Suggested action:**
1. Return `route_contract::Digest` from contract.rs:88 and environment.rs:135, and convert to String only at the serde boundary.
2. Add `Digest::hex(&self) -> &str` and use it at contribution-workbench host.rs:770.
3. Keep material.rs's bare hex (it is a secret digest stored in the journal), but rename it `secret_digest_hex` so the format is explicit. `sha256_hex` has 149 call sites repo-wide, so leave its visibility as it is.
4. Verify: `rg 'trim_start_matches\("sha256:"\)' crates/` returns no hits in this area.

### MSC-12 The os/ script tests are manual-only and several bin scripts have no test

**Severity:** Low · **Category:** testing · **Effort:** M

**Locations:**
- [os/tests/screen-record.sh:5](../../../../os/tests/screen-record.sh)
- [os/flake.nix:111](../../../../os/flake.nix)
- [os/bin/coder-update](../../../../os/bin/coder-update)
- [os/bin/oa-workspace](../../../../os/bin/oa-workspace)

**Evidence:** os/tests/screen-record.sh says "Run it by hand" (lines 5-7). The flake.nix checks (lines 47-233) reference only the .nix stub files, not the 9 .sh tests. Of the 16 os/bin scripts, 8 have no matching test: android-emulator, beep-sound, camera-overlay, camera-toggle, coder-host-install, coder-open-url, coder-update (245 lines) and oa-workspace (358 lines).

**Impact:** Regressions in the recording, dictation and update scripts are caught only when someone tests by hand on a CoderOS host, and coder-update runs unattended.

**Suggested action:**
1. Add to os/flake.nix: `checks.${system}.scripts = pkgs.runCommand "coderos-script-tests" { nativeBuildInputs = [ pkgs.bash pkgs.coreutils pkgs.jq ]; } ''for t in ${./tests}/*.sh; do bash "$t"; done; touch $out''`.
2. Add os/tests/coder-update.sh with stubbed `git` and `cargo`, following the stub style of screen-record.sh.
3. Verify: `nix flake check ./os` runs the script tests, and fails when one test is broken on purpose.

### MSC-13 mc-bridge is an untested, unchecked nightly-only workspace

**Severity:** Low · **Category:** repo-hygiene · **Effort:** M

**Locations:**
- [mc-bridge/Cargo.toml:1](../../../../mc-bridge/Cargo.toml)
- [mc-bridge/rust-toolchain.toml:2](../../../../mc-bridge/rust-toolchain.toml)
- [scripts/build-mc-bridge.sh](../../../../scripts/build-mc-bridge.sh)

**Evidence:** mc-bridge is a separate `[workspace]` pinned to nightly-2026-08-03, with 0 `#[test]` in mc-bridge/src. It was last committed on 2026-09-21, and only scripts/build-mc-bridge.sh builds it. Cargo.toml documents that azalea forces both the release-candidate crypto pins (`pkcs8 =0.11.0-rc.9` and others) and the separate nightly workspace.

**Impact:** A nightly or azalea change can break it without anyone noticing until someone runs a Voyager episode. The pinned release-candidate crypto crates receive no patches.

**Suggested action:**
1. Decide whether the Voyager Minecraft lane is still active. docs/minecraft/voyager-runbook.md still references it.
2. If it is active, add unit tests for the line-protocol parse and serialize code, plus a periodic `cargo +nightly check --manifest-path mc-bridge/Cargo.toml`.
3. If it is not active, move it to backroom.

### MSC-14 About 30k LOC of retail and contribution code is gated off, with no plan for keeping it current

**Severity:** Low · **Category:** architecture · **Effort:** S

**Locations:**
- [retail-cloud/src/lib.rs:10](../../../../crates/retail-cloud/src/lib.rs)
- NEEDS_OWNER.md:1270 and NEEDS_OWNER.md:1436 (workspace root)

**Evidence:** retail-cloud says paid availability stays off until the owner confirms (lib.rs:10-12). The retail contract review is at NEEDS_OWNER.md:1270, and contribution-service qualification (#10729) is at NEEDS_OWNER.md:1436. contribution-service has no reverse dependencies. The gating itself is intentional and documented.

**Impact:** Every change to a shared crate has to keep this dormant code compiling and its tests passing.

**Suggested action:**
1. Track the blocking owner decisions in one issue on project 22.
2. Until those decisions land, limit changes in these crates to fixes and the refactors listed above.
3. If the v2 contract is chosen, delete the paths that only v1 uses.

### MSC-15 townsfolk builds file paths from villager ids it has not validated, and its dependency docs are stale

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:**
- [files.rs:81](../../../../crates/townsfolk/src/files.rs), files.rs:86, files.rs:381
- [lib.rs:30](../../../../crates/townsfolk/src/lib.rs), lib.rs:661
- [Cargo.toml:5](../../../../crates/townsfolk/Cargo.toml)

**Evidence:** `npc_path` and `proposal_path` both do `root.join(DIR).join(format!("{id}.json"))` (files.rs:81-88). `propose` reads `dir.npc(id)` through `check()` and writes to `proposal_path(id)` (files.rs:381). A validator, `good_id`, exists at lib.rs:661, but `Dir` does not call it. lib.rs:30 says the crate depends on "serde, SHA-256, the town clock, and the world tree only", and the Cargo.toml description says "Serde and SHA-256 only". The crate also depends on memory-stream and serde_json.

**Impact:** An id such as `../x` reads root/x.json and writes root/x.json, outside the proposals directory. Only a local operator can supply such an id.

**Suggested action:**
1. Make `npc_path` and `proposal_path` return `Result`, and reject any id for which `!good_id(id)`.
2. Verify: add a test that `propose(dir, "../x", ..)` returns an error.
3. Correct the dependency text in lib.rs:30-31 and in the Cargo.toml description.

### MSC-16 The operator endpoint does not zero its bearer secret the way the customer endpoint does

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:**
- [http.rs:51](../../../../crates/retail-service/src/http.rs), http.rs:93

**Evidence:** `operator` builds `secret` as a String and moves it into the closure without zeroing it (http.rs:51-57). `call` zeroes its copy with `bytes.fill(0)` (http.rs:104-106). Both handlers duplicate the same header parsing.

**Impact:** The operator credential is left in freed heap memory, and the duplicated parsing makes further inconsistencies like this one more likely.

**Suggested action:**
1. Add `fn bearer(headers: &HeaderMap) -> zeroize::Zeroizing<String>` to http.rs and use it in both handlers.
2. Remove the manual `fill(0)`.
3. Verify: the existing http tests pass, and `rg 'fill\(0\)' crates/retail-service/src/http.rs` returns nothing.

### MSC-17 civil_from_days date math copied again (56 copies repo-wide)

**Severity:** Low · **Category:** duplication · **Effort:** M

**Locations:**
- [environment.rs:352](../../../../crates/retail-cloud/src/environment.rs), environment.rs:367

**Evidence:** `day_label` hand-rolls Hinnant's civil_from_days algorithm (environment.rs:366-376). Outside psionic and vendor, `rg` finds `719_?468` in 56 files.

**Impact:** Calendar logic is duplicated all over the repo, including in billing-period code.

**Suggested action:**
1. Add a tested `civil` module (`days_to_ymd`, `ymd_to_days`, `month_name`) to an existing crate with no dependencies, such as route-contract.
2. Switch retail-cloud to it first, then replace the other copies as those files are touched.
3. Verify: the `rg '719_?468'` count goes down over time, and the module's tests cover leap years and the epoch.

### MSC-18 bunny-rules re-parses a level file on every garden() call, and game.rs is 2.1k lines

**Severity:** Low · **Category:** performance · **Effort:** S

**Locations:**
- [bunny-rules/src/level.rs:59](../../../../crates/bunny-rules/src/level.rs)
- [bunny-web/src/app.rs:598](../../../../crates/bunny-web/src/app.rs), app.rs:629, app.rs:756

**Evidence:** `garden(number)` runs `parse(GARDENS[number - 1]).expect(..)` every time it is called (level.rs:59-61). bunny-web calls `level::garden(n).name` just to read the name, at app.rs:598, 629 and 756. game.rs is 2,149 lines.

**Impact:** The menu and board code paths parse a whole level just to get its name. The cost is small, but the work is wasted.

**Suggested action:**
1. In level.rs, add `static PARSED: LazyLock<Vec<Garden>>` and `pub fn name(n) -> &'static str`.
2. Use `name(n)` at app.rs:598, 629 and 756.
3. Optionally, move the farmer AI out of game.rs into src/farmer.rs.
4. Verify: the 48 bunny-rules tests still pass.

### MSC-19 Small hygiene issues: a stale clippy allow, a bool authority flag, a misleading package.json and dependency versions that are not centralized

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [environment.rs:1065](../../../../crates/retail-cloud/src/environment.rs), environment.rs:1136
- [packages/coder-defaults/package.json:1](../../../../packages/coder-defaults/package.json)
- [Cargo.toml:14](../../../../Cargo.toml)

**Evidence:** `#[allow(clippy::too_many_arguments)]` is on `offer`, which has 7 parameters (environment.rs:1065-1073), below the lint threshold, so the allow is stale. `confirm` has 8 parameters including `spend: bool` (environment.rs:1136-1145), so its allow is legitimate. packages/coder-defaults/package.json is an OpenAgents package descriptor (`"v": 1`, `"package": "<hash>:coder-defaults"`) with no npm name or version. The root `[workspace.dependencies]` lists only iroh crates (Cargo.toml:14-22).

**Impact:** Each item is minor, but together they mislead readers and tooling. The bool lets a caller grant spend authority without proving it.

**Suggested action:**
1. Remove the `allow` on `offer`.
2. Change `confirm`'s `spend: bool` to a proof type that only the authority checks can construct.
3. Add a README to packages/coder-defaults stating that package.json is not an npm manifest. Renaming the file would require updating its consumers.
4. Move serde, serde_json, sha2, hex, rusqlite and thiserror into `[workspace.dependencies]`, starting with the crates in this area.
5. Verify: `cargo clippy -p retail-cloud` is clean without the removed allow.

## Refuted during verification

- **"town-clock has no tests, though five crates depend on its time math."** This is wrong. crates/town-clock/src/lib.rs has `mod tests` at line 493 with 12 `#[test]` functions at HEAD, including `the_pace_gives_daylight_most_of_the_hour`, `two_devices_at_one_instant_agree` and `the_epoch_is_midnight_of_day_zero`. Only a small doc error remains: lib.rs:44 says daylight takes 42 minutes, but PACE gives 42.5. That is too minor to be a finding.
- **"contribution-service compares money and time values through lossy casts and string errors."** Every cited cast is inside a check that fails closed. At lib.rs:652, a negative difference in `invoice.expiry_seconds() != (expires_at - created_at) as u64` wraps to about 1.8e19, which can never equal a real invoice expiry, so the request is refused. evaluate.rs:379 requires `frozen_event.created_at as i64 == frozen.terms.committed_at`, which forces committed_at to be a non-negative signed-event timestamp before the later `as u64` uses (lines 432, 470 and 523). Line 461 does the same for evaluated_at. Negative timestamps are refused, so no wrapped value is ever compared. What remains is generic lint advice.
