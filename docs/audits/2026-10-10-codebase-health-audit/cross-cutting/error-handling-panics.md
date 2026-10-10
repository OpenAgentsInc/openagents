# Error handling & panics

**Scope:** Error handling and panics across all Rust code in the repo (non-test code where possible), quantified per crate. **Health grade: C.** Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`.

Error handling across the roughly 2.2M non-test Rust LOC is uneven. Very little code panics on purpose. There are about 26 non-test `panic!` sites outside `docs/`, the workspace denies `todo!`/`unimplemented!`/`dbg!`, no crate depends on `anyhow`, and most `expect()` messages name the invariant they rely on. The main weakness is that errors lose information as they travel up the stack. 2,476 public functions return `Result<_, String>`, and only 123 `Error` enums exist outside psionic. As a result, tests assert on error substrings, callers branch on error text, and the payment path cannot tell "definitely not paid" from "unknown". About 1,900 non-`writeln!` `let _ =` discards exist, and some of them drop writes that matter for security or money: session revocation, buyer-ledger phases, paid-call records, and agent journal entries. The riskiest sites sit at live boundaries: Spark wallet error flattening, gateway session revocation, poisonable mutexes in server state and `Drop` impls, and detached background tasks that nothing supervises. Logging uses `eprintln!` nearly everywhere, so an error that is logged and then ignored cannot be found later.

## Measurements

| Metric (non-test unless noted) | Value |
|---|---|
| Tracked `.rs` files / total LOC | 6,704 / ~3.59M |
| Non-test LOC (excl. `tests/`, `*tests.rs`, `#[cfg(test)]` code and modules) | ~2.20M |
| `.unwrap()` / `.expect(` | 924 / 1,976 |
| `panic!(` | 29 (26 outside `docs/`) |
| `unreachable!(` | 143 (63 in psionic) |
| `todo!` / `unimplemented!` | 0 real (oa-copy hits are string literals) |
| `let _ =` | 2,893 (~1,900 excluding `writeln!`/`write!`) |
| Largest `let _ =` groups | `fs::remove_dir_all` 243, `remove_file` 149+41, sender/tx/reply `.send` 170+, `fs::write` 55, `store.append` 47, `child.wait` 44+13, `child.kill` 40 |
| `.ok();` statement discards | 174 |
| `Result<_, String>` occurrences / `pub fn`s returning it | 6,056 / 2,476 |
| `Box<dyn Error>` | 96 (72 in psionic-train, mostly bins) |
| Crates depending on `anyhow` / manifests with `thiserror` | 0 / 43 |
| `lock()`/`read()`/`write()` + `.unwrap`/`.expect` | 205-210 |
| Poison recovery via `into_inner` | ~295 |
| `eprintln!` in non-test crate code | 1,324 |
| Crates depending on `tracing` | 2 (jev, claude_agent_sdk) |
| Statement-form detached `tokio::spawn` / `thread::spawn` | 108 / 146 |
| Error-substring assertions in tests | 958 |
| `pub enum *Error` | 1,209 (961 files in psionic, 123 outside) |
| Other error-type suffixes | `*Refusal` 78, `*Failure` 18, `*Fault` 16, `*Trouble` 9 |
| Files defining own `stable_json_digest`/`stable_digest`/`digest_json` | 912 (psionic-train 250, eval 195, research 137, runtime 86; includes test files) |
| Copy-pasted canonical-JSON writers with `expect("a key serializes")` | 11 |
| Largest error-relevant files | gateway `serve.rs` 6,008 lines, `accounts.rs` 2,709, `relay_worker.rs` 2,156 |

**Per crate, non-test `unwrap`/`expect`:**

| Crate | unwrap / expect | Non-test LOC |
|---|---|---|
| psionic-train | 1 / 436 | 320k |
| verse-world | 207 / 13 | 47k |
| psionic-eval | 0 / 170 | |
| coder | 74 / 62 | 147k |
| coder-working-computer | 80 / 17 (34 lock-unwraps in ungated fakes) | 6.3k |
| psionic-research | 0 / 97 | |
| psionic-serve | 0 / 97 | |
| gateway | 42 / 39 | 32k |
| coder-one | 13 / 66 | |
| tenancy | 35 / 35 | |
| voyager | 0 / 44 | |
| lev | 0 / 43 | |
| nostr | 0 / 38 | |
| oa-auth | 0 / 30 (feature-gated fake) | |

**`pub fn` returning `Result<_, String>` by crate:** coder 319, verse-world 251, coder-one 176, verse-engine 106, verse 86, gym 82, knowledge 67, everglade 64, tenancy 56, coder-new 54, gateway 42.

**Non-`writeln!` `let _ =` by crate:** coder 268, coder-one 175, jev 137 (mostly doc-test noise), openagents-desktop 109, coder-host 78, openagents-cli 72.

## Strengths

- Almost no deliberate panics in production code: about 26 non-test `panic!` sites outside `docs/`. The workspace denies `clippy::todo`, `clippy::unimplemented` and `clippy::dbg_macro` ([Cargo.toml](../../../../Cargo.toml) lines 37-40).
- No `anyhow` in library APIs (zero crates depend on it). Where typed errors exist they use `thiserror` (43 manifests). [pay-ledger/src/lib.rs](../../../../crates/pay-ledger/src/lib.rs) lines 39-60 is a good model: `Invalid(&'static str)`, `Conflict`, `Insufficient{available_msat}`, `NoRule`.
- Lock-poison recovery is used where it matters most: [pylon/src/paid.rs](../../../../crates/pylon/src/paid.rs) (lines 257, 542, 578, 608, 679, 768, 789) and [spark-wallet/src/spark.rs](../../../../crates/spark-wallet/src/spark.rs) line 510 use `unwrap_or_else(|e| e.into_inner())`. There are about 295 such sites repo-wide.
- Most `expect()` calls state the invariant they rely on (e.g. "validated validator manifests always carry challenge_id", "HMAC accepts 32 bytes"), so a panic message points at the broken assumption.
- Test doubles are feature-gated properly in oa-auth (`#[cfg(feature = "fake")] pub mod fake;` at [oa-auth/src/lib.rs](../../../../crates/oa-auth/src/lib.rs) lines 23-24). Other crates should copy this pattern.
- Some code handles uncertain outcomes honestly. `plugin_purchase.rs:871-873` refuses with "do not pay again" when the ledger phase cannot be written after delivery. Gateway jobs recovery (`jobs.rs:645-652`) records missing item outcomes as `unknown` instead of guessing.
- nostr-relay writes structured JSON log lines (`server.rs:2480-2493`) instead of free-text `eprintln!`.
- Lease and epoch code often uses checked arithmetic (verse-world `realm.rs:404-410` uses `checked_add` with clear error messages).

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-ERR-01 | high | error-handling | Spark wallet flattens SDK errors to String; CLI says "Nothing was paid" on network/storage failures and retries with a fresh idempotency key | M |
| X-ERR-02 | high | concurrency | Poisonable `std::Mutex` with `lock().expect/unwrap` on shared server state, including inside `Drop` | M |
| X-ERR-03 | medium | security | Member removal swallows the session-revocation error its doc promises | S |
| X-ERR-04 | medium | error-handling | Buyer ledger phase writes and paid-call records discarded on CLI payment paths | S |
| X-ERR-05 | medium | concurrency | Detached long-running tasks are never supervised; errors and panics vanish | M |
| X-ERR-06 | medium | maintainability | ~2,476 `pub fn`s return `Result<_, String>`; callers and tests match on text | XL |
| X-ERR-07 | medium | error-handling | Gateway batch jobs rebuild caller identity from untyped JSON with silent defaults and drop write errors | M |
| X-ERR-08 | medium | observability | No structured logging: `eprintln!` everywhere, no `tracing` in server crates | L |
| X-ERR-09 | medium | duplication | ~912 files with copy-pasted digest helpers; 11 identical canonical-JSON writers with their own `expect()` | L |
| X-ERR-10 | medium | error-handling | Lookups that are only safe because of code elsewhere are unwrapped repeatedly in realm leasing and the sales pipeline | M |
| X-ERR-11 | medium | error-handling | Agent journal appends silently discarded in the coder task host | S |
| X-ERR-12 | low | error-handling | Earnings handlers rely on the route-mount invariant via `unwrap()` | S |
| X-ERR-13 | low | repo-hygiene | Test fakes compiled into production libraries without cfg gates | S |
| X-ERR-14 | low | build | Workspace lints don't flag unwrap/expect, discarded must_use, or lossy casts | M |
| X-ERR-15 | low | architecture | psionic: non-test expects re-assert validation invariants | L |
| X-ERR-16 | low | error-handling | Failures turned into "nothing found" on payout-destination and file-read paths | S |
| X-ERR-17 | low | maintainability | Two distinct gateway `Trouble` enums | S |

### X-ERR-01 Spark wallet pay paths flatten SDK errors to String; the CLI reports 'Nothing was paid' for network/storage failures and retries with a fresh idempotency key

**Severity:** high · **Category:** error-handling · **Effort:** M

**Locations:**
- [spark.rs:506-536](../../../../crates/spark-wallet/src/spark.rs)
- [spark.rs:424-443](../../../../crates/spark-wallet/src/spark.rs)
- [spark.rs:748-789](../../../../crates/spark-wallet/src/spark.rs)
- [x402_spark.rs:27-45](../../../../crates/openagents-cli/src/x402_spark.rs)
- [wallet.rs:388-389](../../../../crates/openagents-cli/src/wallet.rs)
- [spend.rs:965-970](../../../../crates/openagents-mobile/src/spend.rs)

**Evidence:** `SparkNode::pay -> Result<Paid, String>` uses `.map_err(|error| describe("pay", &error.to_string()))`, and `describe()` renders "The payment did not go through ({detail})." `failure()` at spark.rs:783-789 chooses `InsufficientFunds` via `detail.to_ascii_lowercase().contains("insufficient")`. x402_spark.rs:44: `AgentPayFailure::Failed(message) => format!("{message} Nothing was paid.")`. x402_spark.rs:28 and wallet.rs:388 both create `let key = uuid::Uuid::new_v4().to_string();` on every run. spend.rs:969 maps `Failed` to `Refusal::PaymentFailed`.

**Impact:** An SDK error that arrives after the transfer was actually sent (a network or storage error) is reported as a definite non-payment. A manual retry then uses a fresh idempotency key, so the same payment can go out twice. The text match on "insufficient" breaks if the SDK changes its error wording. Two things must both happen for a double payment: an indeterminate SDK error after the send, and a manual retry by the user. No automatic retry loop was found.

**Suggested action:**
1. Add a typed `WalletError` / `AgentPayFailure::Indeterminate` built by matching `breez_sdk_spark::SdkError` variants instead of calling `to_string()`.
2. Map `NetworkError`/`StorageError` during send to `Indeterminate`. Map insufficient funds from its variant, not from text.
3. In `x402_spark.rs` `refused()` and `wallet.rs`, print "Nothing was paid" only for definite refusals. On `Indeterminate`, persist the idempotency key, reuse it on retry, and point the user to `openagents wallet history`.
4. In mobile `spend.rs`, keep the request pending on `Indeterminate` instead of refusing.
5. Verify: add fake-Node tests for each `SdkError` variant that assert the mapped failure, the user-facing text, and that the stored key is reused on retry.

### X-ERR-02 Poisonable std::Mutex with lock().expect/unwrap on shared server state, including inside Drop impls

**Severity:** high · **Category:** concurrency · **Effort:** M

**Locations:**
- [admission.rs:274-300](../../../../crates/verse-world/src/service/net/admission.rs)
- [admission.rs:402-411](../../../../crates/verse-world/src/service/net/admission.rs)
- [relay_worker.rs:37](../../../../crates/gateway/src/relay_worker.rs)
- [team_reports.rs:105-125](../../../../crates/gateway/src/team_reports.rs)
- [psionic-backend-cuda lib.rs:16345-16380](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs)

**Evidence:** In admission.rs, `Slot::received/delivered/authenticate` call `self.limits.0.lock().unwrap()` and then `s.clients.get_mut(&self.serial).unwrap()`. `impl Drop for Slot` also locks with `unwrap` and runs `s.stats.active -= 1`. relay_worker.rs imports `std::sync::Mutex` and has 16 lock calls of the form `.expect("jobs"|"running"|"seen"|"quota")`. In team_reports.rs, `impl Drop for ProgressGuard` calls `.lock().expect("team progress")`. The CUDA allocator's `Drop` at HEAD lib.rs:16356/16380 has `.expect("cuda allocator pool mutex should not be poisoned")`.

**Impact:** A panic while one of these locks is held poisons it, and every later connection or job then panics too. A `Drop` that panics while the thread is already unwinding aborts the process.

**Suggested action:**
1. Switch these mutexes to `parking_lot::Mutex`, or use `lock().unwrap_or_else(PoisonError::into_inner)`.
2. In the `Drop` impls (`Slot`, `ProgressGuard`, CUDA buffer), always recover from poison and use `saturating_sub` for `active`.
3. Replace `clients.get_mut(&self.serial).unwrap()` with `if let`.
4. Verify: add an admission test that panics while holding the lock (in a spawned thread) and then shows that new slots can still be admitted and dropped without a panic.

### X-ERR-03 Workspace member removal swallows the session-revocation error its doc promises

**Severity:** medium · **Category:** security · **Effort:** S

**Locations:**
- [accounts.rs:1874-1925](../../../../crates/gateway/src/accounts.rs)
- [accounts.rs:472-490](../../../../crates/gateway/src/accounts.rs)

**Evidence:** The doc says "the member's sessions end in the same call". The code is `if let Ok(sessions) = sessions_store(&state) { sessions.mutate(|book, access, now| { book.revoke_all(...); ... Ok(()) }).ok(); }`, and the handler always answers 200. `record()` at 472-490 also ends in `.ok();`, and its doc says that is intentional.

**Impact:** If the sessions store fails, a removed member keeps live sessions while the API reports success. The workspace membership itself is still revoked, so the exposure is limited to account-level (non-workspace) actions.

**Suggested action:**
1. In `member_remove`, return 503 `sessions_unavailable` (after the membership revoke) when `sessions_store` or `mutate` fails. Alternatively, do both revokes in one store transaction.
2. Leave `record()`'s documented `.ok()` in place, but log the failure.
3. Verify: add a gateway test with a failing sessions store and assert the DELETE does not return a silent 200.

### X-ERR-04 Buyer payment-ledger writes and paid-call records discarded on CLI payment paths

**Severity:** medium · **Category:** error-handling · **Effort:** S

**Locations:**
- [plugin_purchase.rs:871-884](../../../../crates/openagents-cli/src/plugin_purchase.rs)
- [pay_plugin.rs:651-665](../../../../crates/openagents-cli/src/pay_plugin.rs)
- [x402.rs:878-883](../../../../crates/openagents-cli/src/x402.rs)
- [pay_payout.rs:282](../../../../crates/openagents-cli/src/pay_payout.rs)

**Evidence:** plugin_purchase.rs:880 has `let _ = ledger.set_phase(&proof.payment_hash, "delivery_failed");`, while the `http_200` branch at 871-873 returns "needs reconciliation; do not pay again" when the same write fails. pay_plugin.rs:662-664 has `if let Ok(mut ledger) = self.0.lock() { let _ = ledger.record_call(&record); }` and `at: i64::try_from(usage.at).unwrap_or(i64::MAX)`. `x402::set_phase` only calls `eprintln!` on failure. pay_payout.rs:282 has `i64::try_from(row.fee_sats).unwrap_or(0) * 1000`.

**Impact:** The local buyer ledger can still say "paid" after a delivery failed, and paid calls can drop out of the call log, so reconciliation and refunds rely on data that was silently lost. These are local bookkeeping records, not fund movement, and the user still sees the `delivery_failed` result.

**Suggested action:**
1. At plugin_purchase.rs:880, return the same reconciliation error as the `http_200` branch when `set_phase` fails.
2. In pay_plugin.rs `on_call`, recover the lock with `into_inner` and log or surface `record_call` failures.
3. Replace `unwrap_or(i64::MAX)` and `unwrap_or(0) * 1000` with checked conversions and checked multiplication that return errors.
4. Verify: add tests with a read-only ledger and assert the reconciliation error is returned and the call-record failure is reported.

### X-ERR-05 Detached long-running tasks are never supervised; their errors and panics vanish

**Severity:** medium · **Category:** concurrency · **Effort:** M

**Locations:**
- [coder-host serve/mod.rs:342-349](../../../../crates/coder-host/src/serve/mod.rs)
- [coder-host serve/mod.rs:580-583](../../../../crates/coder-host/src/serve/mod.rs)
- [nostr-relay gateway/server.rs:660-667](../../../../crates/nostr-relay/src/gateway/server.rs)
- [openagents-web pages/chat.rs:1369-1383](../../../../crates/openagents-web/src/pages/chat.rs)

**Evidence:** coder-host pushes `direct::listen`, `relay::serve`, `presence_loop`, `summary_loop`, `spend_wake_loop` and `cj::serve` into `tasks`. The only thing that ever touches them is `stop()`, which calls `task.abort()`. nostr-relay has `let _ = handle_socket(stream, peer, state).await;` and `completed = connections.join_next() => { let _ = completed; }`. In chat.rs, `spawn_answer` runs `tokio::spawn(async move { answer(...).await; })`, and `answer()` calls `.expect("dispatch owns pending request")`.

**Impact:** If a subsystem such as spend wake or the relay exits early or panics, it stays dead and nothing reports it, while the host keeps serving. Relay connection errors and panics leave no trace. A panic in the chat answer task leaves the conversation stuck in pending. No failure has been demonstrated; this is a gap in resilience and observability.

**Suggested action:**
1. In coder-host, hold `tasks` in a `JoinSet` and `select!` on it. Log each exit reason, then restart the task or exit non-zero.
2. In nostr-relay, log `Err` from `handle_socket` at debug level, and log `JoinError::is_panic` at error level.
3. In chat.rs, make `answer()` return `Result` and mark the message failed on `Err` or `JoinError`.
4. Verify: add a test where a supervised loop returns early or panics, and assert the exit is logged and handled. Add a chat test where `answer` fails and the message ends as failed, not pending.

### X-ERR-06 Stringly-typed errors at crate boundaries: ~2,476 pub fns return Result<_, String>, so callers and tests match on text

**Severity:** medium · **Category:** maintainability · **Effort:** XL

**Locations:**
- [coder-new fleet.rs:540-564](../../../../crates/coder-new/src/fleet.rs)
- [coder task/agent.rs:1035-1044](../../../../crates/coder/src/task/agent.rs)
- [gateway serve.rs:60-76](../../../../crates/gateway/src/serve.rs)
- [gateway relay_worker.rs:290-297](../../../../crates/gateway/src/relay_worker.rs)

**Evidence:** fleet.rs:560 has `Err(error) if error.contains("already exists") => last = error`, which matches on the text returned by `coder::branch_checkout::add`. `Store::append` returns `Result<(), String>`. Gateway serve.rs defines `Trouble::Money(String)`, and relay_worker.rs defines `Trouble::Config(String)` and `Ledger(String)`. Across the repo, 958 test assertions match on error substrings.

**Impact:** Editing an error message can silently change control flow, as with the fleet.rs retry. Error wording is tied to program logic.

**Suggested action:**
1. Add `thiserror` enums, starting with `coder::branch_checkout` (`BranchError::AlreadyExists`), and make fleet.rs:560 match the variant.
2. Do the same for `Store::append`, then for gateway `Trouble::Money/Config/Ledger`. Keep the human-readable text in `Display`.
3. Add a CI ratchet that fails if the count of `pub fn .* -> Result<.*, String>` grows.
4. Verify: fleet retry tests pass after the error message text is changed, and the ratchet count goes down with each migrated crate.

### X-ERR-07 Gateway batch jobs rebuild caller identity from untyped JSON with silent defaults and drop status, result and ledger write errors

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [jobs.rs:493-512](../../../../crates/gateway/src/jobs.rs)
- [jobs.rs:620-643](../../../../crates/gateway/src/jobs.rs)
- [jobs.rs:993-995](../../../../crates/gateway/src/jobs.rs)

**Evidence:** The job runner reads `key: manifest["key"].as_str().unwrap_or_default()` and `scopes: serde_json::from_value(manifest["scopes"].clone()).unwrap_or_default()`, and discards status writes with `let _ = write_status(...)`. `ledger_writer` returns silently when open, `writeln` or `sync_data` fails. `write_result` is `let _ = fs::write(...)`. The reader is `fn read_json(path) -> Option<Value>`.

**Impact:** A corrupt manifest runs the job with an empty key and default scopes. When the ledger writer dies, items go missing and nothing reports it.

**Suggested action:**
1. Deserialize into a typed `JobManifest` struct and fail the job as `manifest_invalid` on a parse error.
2. Make `read_json` return `Result<Option<Value>>`.
3. Report `ledger_writer` failure back through a oneshot so `run()` marks the job failed.
4. Log `write_status`/`write_result` errors.
5. Verify: add tests with a corrupt manifest (job fails as `manifest_invalid`) and with an unwritable ledger path (job is marked failed).

### X-ERR-08 No structured logging: eprintln! everywhere and no tracing in server crates

**Severity:** medium · **Category:** observability · **Effort:** L

**Locations:**
- [gateway/Cargo.toml](../../../../crates/gateway/Cargo.toml)
- [nostr-relay/Cargo.toml](../../../../crates/nostr-relay/Cargo.toml)
- [x402.rs:878-883](../../../../crates/openagents-cli/src/x402.rs)
- [Cargo.toml:37-40](../../../../Cargo.toml)

**Evidence:** Only `crates/jev/Cargo.toml` and `crates/claude_agent_sdk/Cargo.toml` declare `tracing`. The gateway, nostr-relay, tenancy and coder-host manifests have no logging crate. `x402::set_phase` logs with `eprintln!("x402: ledger: {error}")`. There are 1,324 `eprintln!` sites in non-test crate code.

**Impact:** When an error is logged and execution continues, nobody can filter, correlate or alert on it in production.

**Suggested action:**
1. Add `tracing` and `tracing-subscriber` (JSON output) as workspace dependencies.
2. Initialize them in the gateway, nostr-relay, coder-host and openagents-web binaries.
3. Migrate server `eprintln!` calls, starting with gateway and nostr-relay.
4. Set `clippy::print_stderr = "warn"` in those crates.
5. Verify: those binaries emit JSON log lines with level and target, and the `eprintln!` count in the migrated crates is zero or allow-listed.

### X-ERR-09 About 912 files with copy-pasted digest helpers and 11 identical canonical-JSON writers, each with its own expect()

**Severity:** medium · **Category:** duplication · **Effort:** L

**Locations:**
- [receipts export.rs:993-1011](../../../../crates/receipts/src/export.rs)
- [tenancy billing.rs:2200-2218](../../../../crates/tenancy/src/billing.rs)
- [gym admission.rs:2123](../../../../crates/gym/src/admission.rs)
- [nostr decision.rs:1781](../../../../crates/nostr/src/decision.rs)
- [coder-scheduler catalog.rs:484](../../../../crates/coder-scheduler/src/catalog.rs)

**Evidence:** `git grep -lE 'fn (stable_json_digest|stable_digest|digest_json)' HEAD -- crates` returns 912 files (test files included). `expect("a key serializes")` appears in 11 files.

**Impact:** Receipt and signature canonicalization can drift apart between crates, and each copy adds its own panic site.

**Suggested action:**
1. Create `crates/canonical-json` with fallible `canonical()` and `sha256_hex()` functions.
2. Replace the 11 key-sorted writers first, and add a cross-crate golden-fixture test that pins the byte output.
3. Then replace the psionic helpers crate by crate.
4. Verify: the golden fixture passes for every migrated crate, and the `git grep` count above goes down.

### X-ERR-10 Lookups whose success is guaranteed only by code elsewhere are unwrapped repeatedly in realm leasing and sales pipeline

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [realm.rs:411-552](../../../../crates/verse-world/src/service/realm.rs)
- [realm.rs:632-638](../../../../crates/verse-world/src/service/realm.rs)
- [sales.rs:494-497](../../../../crates/coder/src/task/sales.rs)
- [sales.rs:1236-1552](../../../../crates/coder/src/task/sales.rs)
- [billing.rs:1192](../../../../crates/tenancy/src/billing.rs)

**Evidence:** realm.rs calls `self.games.get_mut(&...).unwrap()` at 411, 431, 508, 552, 632 and 638, and `self.manifest.instances.get_mut(...).unwrap()` at 432, 509 and 532. These assume the two maps stay in sync. sales.rs has 5 `leads.get_mut(..).unwrap()` calls and 20 unwraps in total in lines 1236-1552. Line 496 has `normalized.split_once(':').unwrap().0`. billing.rs:1192 has `self.invoices.get_mut(&invoice_id).unwrap()`.

**Impact:** If a reloaded manifest and the game map get out of sync, the realm authority panics instead of returning an error.

**Suggested action:**
1. In realm.rs, merge `games` and `manifest.instances` into one map, or add a helper that returns `Result` for the pair.
2. In sales.rs, fetch the lead once with `ok_or(...)?` before the match, and handle the `split_once` miss.
3. In billing.rs, bind the `&mut` returned by `entry().or_insert_with`.
4. Verify: add a realm test with a manifest instance that has no game and assert an error is returned, not a panic.

### X-ERR-11 Agent journal appends silently discarded in the coder task host

**Severity:** medium · **Category:** error-handling · **Effort:** S

**Locations:**
- [agent_host.rs](../../../../crates/coder/src/task/agent_host.rs)
- [agent_host_plan.rs](../../../../crates/coder/src/task/agent_host_plan.rs)
- [agent_lifecycle.rs](../../../../crates/coder/src/task/agent_lifecycle.rs)
- [agent.rs:1039-1044](../../../../crates/coder/src/task/agent.rs)
- [microcoder repository.rs](../../../../crates/microcoder/src/repository.rs)

**Evidence:** There are 45 `let _ = store.append` sites in `coder/src/task`: agent_host.rs 22, agent_host_plan.rs 6, agent_lifecycle.rs 4, agent_host_merge.rs 3, and others. microcoder has 19 `let _ = host.append` sites. `Store::append` runs `check_agent_copy` (a privacy check) before writing and returns `Result<(), String>`.

**Impact:** Journal entries can disappear without any signal, and a privacy refusal looks the same as success.

**Suggested action:**
1. Add a typed append error, `{PrivacyRefused, Io}`.
2. Add one `journal()` helper that logs and counts failures, and replace the discards with it.
3. Verify: a test that triggers `check_agent_copy` refusal shows it was logged and counted, and `git grep 'let _ = store.append' -- crates/coder` returns nothing.

### X-ERR-12 Earnings handlers depend on the route-mount invariant via unwrap()

**Severity:** low · **Category:** error-handling · **Effort:** S

**Locations:**
- [earnings.rs:255](../../../../crates/gateway/src/earnings.rs), [earnings.rs:291](../../../../crates/gateway/src/earnings.rs), [earnings.rs:405](../../../../crates/gateway/src/earnings.rs), [earnings.rs:450](../../../../crates/gateway/src/earnings.rs), [earnings.rs:787](../../../../crates/gateway/src/earnings.rs)
- [serve.rs:757-758](../../../../crates/gateway/src/serve.rs)

**Evidence:** `state.earnings.as_ref().unwrap().lock().await` appears at 5 sites. The routes are mounted only when `state.config.earnings.is_some()` (serve.rs:757), and serve.rs:301 builds `state.earnings` from the same config.

**Impact:** This is safe today. A future refactor that mounts the routes unconditionally would turn those requests into panics.

**Suggested action:**
1. Give only the earnings sub-router a non-optional `EarningsState` as its axum `State`, which removes the `as_ref().unwrap()` calls.
2. Verify: the earnings handlers compile with no `Option` access, and the existing earnings tests pass.

### X-ERR-13 Test fakes compiled into production libraries without cfg gates

**Severity:** low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [provider.rs:391-394](../../../../crates/coder-working-computer/src/provider.rs)
- [gce.rs:1606](../../../../crates/coder-working-computer/src/gce.rs)
- [retail-cloud lib.rs:40](../../../../crates/retail-cloud/src/lib.rs)
- [codex-transport lib.rs:22](../../../../crates/codex-transport/src/lib.rs)
- [openagents-desktop lib.rs:65](../../../../crates/openagents-desktop/src/lib.rs)
- [coder claim.rs:1365](../../../../crates/coder/src/claim.rs)
- [oa-auth lib.rs:23-24](../../../../crates/oa-auth/src/lib.rs) (the correct pattern)

**Evidence:** provider.rs has "/// An in-memory provider for tests" followed by an ungated `pub mod fake {`. gce.rs:1606 is the same. coder-working-computer is a dependency of openagents-web (Cargo.toml:62), yet the fakes are used only from coder-environment-operator `*tests.rs` files. oa-auth gates its fake with `#[cfg(feature = "fake")] pub mod fake;`. The exception is openagents-desktop: `shell.rs:133` uses `fake::FakeHost` from the non-test `answer_as` capture path.

**Impact:** Test code that panics freely ships in production binaries and inflates unwrap counts in audits.

**Suggested action:**
1. Gate the fakes with `#[cfg(any(test, feature = "fake"))]` and add a `fake` feature that downstream dev-dependencies (coder-environment-operator) enable.
2. Put openagents-desktop's fake behind a non-test feature (for example `capture`), because `answer_as` uses it outside tests. A plain `cfg(test)` would break that caller.
3. Verify: `cargo build -p openagents-web` succeeds and its dependency graph no longer includes the fake modules, and the tests that use them still pass with the feature enabled.

### X-ERR-14 Workspace lints don't flag unwrap/expect, discarded must_use, or lossy casts in boundary crates

**Severity:** low · **Category:** build · **Effort:** M

**Locations:**
- [Cargo.toml:37-40](../../../../Cargo.toml)

**Evidence:** `[workspace.lints.clippy]` denies only `dbg_macro`, `todo` and `unimplemented`.

**Impact:** New unwraps and discards on server and payment paths pass review without any lint warning. This is a process gap, not a defect.

**Suggested action:**
1. Add crate-level `[lints.clippy]` with `unwrap_used`, `expect_used` and `let_underscore_must_use` set to warn, `let_underscore_future` set to deny, and `cast_possible_wrap` set to warn. Apply it to gateway, nostr-relay, pay-ledger, spark-wallet, tenancy, receipts, oa-auth, coder-host and openagents-web.
2. Allow these lints in test code (`#![cfg_attr(test, allow(...))]` in lib.rs).
3. Run clippy for those crates in CI.
4. Verify: clippy reports the existing sites, the warning count is recorded as a baseline, and CI keeps it from growing.

### X-ERR-15 psionic: non-test expects re-assert validation invariants

**Severity:** low · **Category:** architecture · **Effort:** L

**Locations:**
- [train_validator.rs:233-258](../../../../crates/psionic/crates/psionic-train/src/train_validator.rs)

**Evidence:** `.expect("validated validator manifests always carry validator_target_work_class")` and two similar expects appear within lines 233-258 at HEAD.

**Impact:** Validation returns the same type it was given, so every consumer has to re-assert the invariants.

**Suggested action:**
1. Make `validate()` return a `ValidatedValidatorManifest` newtype whose fields are not `Option`.
2. As follow-up work, consolidate psionic error enums per crate.
3. Verify: the three expects are gone, and the code that uses the manifest compiles against the newtype.

### X-ERR-16 Failures turned into 'nothing found' on the payout-destination and file-read paths

**Severity:** low · **Category:** error-handling · **Effort:** S

**Locations:**
- [pay_payout.rs:291-316](../../../../crates/openagents-cli/src/pay_payout.rs)
- [jobs.rs:993-995](../../../../crates/gateway/src/jobs.rs)

**Evidence:** `relay_sources` has `let Ok(signer) = ... else { return sources; }` and `let _ = client.subscribe(...)`. `read_json` returns `Option`.

**Impact:** A relay outage looks the same as "no payout target", which sends debugging in the wrong direction. Payouts themselves stay safe.

**Suggested action:**
1. Make `relay_sources` return `Result<Sources, RelayError>`, and make `read_json` return `Result<Option<_>>`.
2. Verify: a test with an unreachable relay shows a relay error, not an empty source list.

### X-ERR-17 Error type naming is inconsistent: two distinct gateway `Trouble` enums

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:**
- [serve.rs:62](../../../../crates/gateway/src/serve.rs)
- [relay_worker.rs:292](../../../../crates/gateway/src/relay_worker.rs)

**Evidence:** Both files declare a `pub enum Trouble`, with different variants.

**Impact:** The two types are easy to confuse.

**Suggested action:**
1. Rename them to `ServeError` and `RelayWorkerError`.
2. Document the convention for when a type is a `Refusal` and when it is an `Error`.
3. Verify: `git grep 'enum Trouble' -- crates/gateway` returns nothing.

## Verification notes

No reviewer claims were refuted. The verifier corrected these details, and the findings above already include the corrections:
- X-ERR-01: severity lowered from critical to high. Double payment needs both an indeterminate error after the send and a manual retry. No automatic retry was found.
- X-ERR-02: relay_worker has 16 lock expects, not 17. CUDA line numbers were corrected against HEAD.
- X-ERR-09: there are 11 canonical-JSON writers, not 10. The count of 912 files includes test files.
- X-ERR-10: sales.rs has 5 lead `get_mut` unwraps, not ~20 (20 is the count of all unwraps in that range). Unverified billing ranges were dropped.
- X-ERR-11: 45 `store.append` discards in `coder/src/task`, not 47.
- X-ERR-03, X-ERR-04, X-ERR-05, X-ERR-06, X-ERR-12, X-ERR-13, X-ERR-14, X-ERR-15: severity lowered, for the reasons given in each finding.
