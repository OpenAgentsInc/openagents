# Unsafe, async, and concurrency

Scope: unsafe code, async and runtime hygiene, locking, channels, task supervision and global state across all Rust in the repo, psionic included. Snapshot commit `3168c986aa11e18a8bd30f52c270f609e49b3815`, audit date 2026-10-10.

**Health grade: C**

For a tree this size, concurrency hygiene is better than expected. A scan of every file that imports `std::sync::Mutex`/`RwLock` and also uses `.await` turned up 21 places where a guard looked like it might be held across an await. All 21 were false positives: the code clones the value and drops the guard before awaiting. No `lazy_static` or `once_cell` is left, there is no real `static mut`, and the workspace denies `unsafe_op_in_unsafe_fn`. The FFI surfaces (rust-native, openagents-mobile, coder-mobile) check for null, carry `# Safety` docs and wrap their work in `catch_unwind`. Most process-wide caches are size-limited.

The problems sit in a few places:

1. The psionic inference stack has a real soundness bug in the macOS vDSP RMS-norm path. Separately, the 26k-line CUDA `lib.rs` contains 198 `unsafe` blocks and not one SAFETY comment.
2. Several live services spawn tasks and drop the handles. In the gateway, a job that panics stays `running` until the next restart. In pay-host, async handlers run SQLite queries under a `std::sync::Mutex` that a 250 ms ingestion loop also takes.
3. In coder-cloud, a released user credential can stay armed after its turn ends.
4. Backpressure and lock-poisoning handling vary from crate to crate. There are 38 production unbounded channels, including the inference request queues. 433 lock-unwrap sites panic on a poisoned lock while 296 recover from it, and 17 crates or modules keep their own `fn lock<T>` helper.
5. Low-level Unix file-ownership and locking code is written by hand against `libc` in 21+ crates, while the shared `private-fs` crate only covers Windows.
6. About 270 non-test code paths build a Tokio runtime per call, often next to a new reqwest client per call.

The audit found no deadlocks and no guards held across `.await`. It found one memory-safety bug and a handful of reliability issues in live paths. Around them is a large amount of unsafe and async boilerplate, with no enforced convention for documenting unsafe code or supervising tasks. Every number comes from `rg`/`git grep` on HEAD. Nothing was compiled.

## Measurements

| Metric | Value |
|---|---|
| Tracked `.rs` files | 6,704 |
| Unsafe sites (`unsafe {`, `unsafe fn`, `unsafe impl`, `unsafe extern`) | 1,151 |
| `// SAFETY` / `# Safety` markers | 480 |
| Largest unsafe/SAFETY gaps by crate | psionic-backend-cuda 253/0 (198 `unsafe {` in one 26,043-line lib.rs); rust-native 117/25 (mostly `# Safety` on the fn instead); coder-boundary 57/40; openagents-mobile 52/16; coder-mobile 32/14; gateway 28/14; psionic-serve 24/3; x402 19/0; openagents-web 16/1; psionic-backend-metal 10/0; commercial-spend 8/0; retail-service 8/0 |
| `unsafe impl Send/Sync` | 10 (Windows handle wrappers, plugin-outline Arena) |
| `static mut` | 0 real (one safe fn that returns `&'static mut`) |
| `lazy_static` / once_cell `Lazy` / `LazyLock` | 0 / 0 / 35 |
| Globals with interior mutability (static Mutex/RwLock/Atomic/OnceLock<Mutex>) | ~202 non-test |
| `libc::geteuid` | 71 calls, 38 files, 21 crates |
| `libc::flock` vs std `File::lock`/`try_lock` | 10 sites in 8 files vs 82 |
| Crates using `libc::` directly | 52 |
| Tokio runtime constructions | 273 non-test lines (114 in psionic-serve openai_http.rs, mostly in-file test helpers) |
| `fn runtime() ->` helper copies | 14 |
| `block_on` / `block_in_place` | 813 lines / 1 (openagents-chat client.rs) |
| `unbounded_channel` | 48 total, 38 non-test |
| `tokio::spawn`/`task::spawn` (non-test) | 233; 105 fire-and-forget statements (coder-host 29, gateway 9, coder 9) |
| Files using JoinSet/TaskTracker | 18 |
| Detached `thread::spawn` statements | 174 non-test (338 thread spawns total) |
| Lock + `.unwrap()/.expect()` (non-test) vs poison recovery via `into_inner` | 433 vs 296 |
| Local `fn lock<T>` helpers | 17 (coder-connect 5, coder-host 4) |
| `std::thread::sleep` | 370 non-test, 342 in tests; `tokio::time::sleep` in tests: 231 |
| Tests forced to `multi_thread` flavor | 294 |
| Workspace lints | `unsafe_op_in_unsafe_fn = deny`; clippy `dbg_macro`/`todo`/`unimplemented = deny`; no `undocumented_unsafe_blocks` |
| Std guard held across `.await` (heuristic) | 21 candidates, all false positives |
| Nested lock acquisition (heuristic) | 2 candidates, neither a lock-ordering hazard |

## Strengths

- The workspace denies `unsafe_op_in_unsafe_fn` ([Cargo.toml](../../../../Cargo.toml) `[workspace.lints.rust]`), so even inside an `unsafe fn` the unsafe operations need their own `unsafe` block.
- No `std::sync` guard is held across `.await`. All 21 candidates clone under the lock and release it before awaiting, for example [byo.rs:652](../../../../crates/openagents-web/src/cloud/byo.rs) and [paid.rs:587](../../../../crates/pylon/tests/paid.rs).
- The FFI exports are written defensively. [layout/ffi.rs](../../../../crates/rust-native/src/layout/ffi.rs) checks for null, caps lengths (`len > 256`), wraps work in `catch_unwind` and has a `# Safety` section on each export. openagents-mobile has 14 exports and 15 `catch_unwind` sites; coder-mobile has 8 of each.
- Most process-wide caches are size-limited and document it: coder-cloud release.rs `MAX_OFFERS=1024`, oa-auth repos.rs `BUDGETS` capped at 4096, coder-boundary snapshot/observe.rs `REUSED_MAX`.
- [private-fs/src/lib.rs:1-25](../../../../crates/private-fs/src/lib.rs) documents the Windows equivalent of the Unix owner-only file rules and tests the SDDL string. It is a good base to extend to Unix.
- The coder-host control socket caps concurrent connections with a `Semaphore` and checks the peer uid before reading a byte ([control/mod.rs:84-104](../../../../crates/coder-host/src/control/mod.rs)). The tailnet listener also takes a permit per connection ([tailnet.rs:368-376](../../../../crates/coder-host/src/tailnet.rs)).
- Git worktree cleanup is kept off runtime workers on purpose. `Drop` hands cleanup to a named OS thread, `close()` uses `spawn_blocking`, and comments explain why ([worktree.rs:71-160](../../../../crates/coder/src/worktree.rs)).
- The coder-reach Acceptor wraps every handshake in `tokio::time::timeout` ([channel.rs:731](../../../../crates/coder-reach/src/channel.rs)).
- No `lazy_static` or `once_cell` remains; globals use std `LazyLock`/`OnceLock`.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-CONC-01 | medium | security | Unsound vDSP RMS-norm on macOS: safe fn reads `weight` out of bounds; `&mut`/`&` raw pointers alias | S |
| X-CONC-02 | medium | maintainability | psionic-backend-cuda: 198 unsafe blocks, no SAFETY comments; bounds checks live in distant wrappers | L |
| X-CONC-03 | medium | concurrency | Gateway jobs spawned without supervision; a panicking job stays `running` and leaks its cancel entry | M |
| X-CONC-04 | medium | performance | pay-host runs blocking SQLite under a std Mutex in async handlers, contended by a 250 ms ingestion loop | M |
| X-CONC-05 | medium | security | Released user credentials can stay armed in a global map; arm precedes fallible steps, disarm is not RAII | S |
| X-CONC-06 | medium | concurrency | Unbounded queues with no backpressure on inference workers and network frame pumps | M |
| X-CONC-07 | medium | concurrency | coder-host listeners spin on accept errors; direct/WebSocket listeners have no connection limit | S |
| X-CONC-08 | medium | duplication | Hand-rolled libc ownership/locking code duplicated across 21 crates, mostly without SAFETY comments | L |
| X-CONC-09 | low | performance | Tokio runtimes and HTTP clients built per call in sync wrappers (14 `fn runtime()` copies) | M |
| X-CONC-10 | low | concurrency | openagents-chat `Client::collect` uses `block_in_place`, which panics on current-thread runtimes | M |
| X-CONC-11 | low | duplication | Inconsistent lock-poisoning policy; 17 duplicated `fn lock<T>` helpers | M |
| X-CONC-12 | low | build | No lint enforces documented unsafe; x402 and wallet carry no lints at all | M |
| X-CONC-13 | low | concurrency | In-process `static Mutex<()>` guards on-disk read-modify-write state in coder task modules | M |
| X-CONC-14 | low | error-handling | Fire-and-forget tasks and detached threads; long-lived loops die silently | M |
| X-CONC-15 | low | maintainability | plugin-outline guest arena: safe fn returns aliased `&'static mut`; bump pointer never resets | S |
| X-CONC-16 | low | testing | Tests depend on wall-clock sleeps, including `thread::sleep` inside runtime hooks | L |

### X-CONC-01 Unsound vDSP RMS-norm on macOS: a safe fn reads `weight` out of bounds, and &mut/& raw pointers alias

Severity: medium · Category: security · Effort: S

**Locations:** [gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) at gguf.rs:16144, gguf.rs:16150, gguf.rs:16211, gguf.rs:16223 and gguf.rs:1945 (an example caller).

**Evidence:** `fn rms_norm_in_place(values: &mut [f32], weight: &[f32], epsilon: f32)` (16144) calls `vDSP_vmul(values.as_ptr(), 1, weight.as_ptr(), 1, values.as_mut_ptr(), 1, values.len())` and never checks that `weight.len() >= values.len()`. `per_head_rms_norm_in_place` (16211-16235) has the same problem with `head.len()`. On non-macOS targets the code uses `zip`, which quietly stops at the shorter slice. Both functions also take `x.as_ptr()` and `x.as_mut_ptr()` from the same `&mut [f32]`. There are more than 30 callers, for example 1945 `rms_norm_in_place(attention_out.as_mut_slice(), norm.as_slice(), ..)`. Nothing at model load checks that the norm tensor lengths match the hidden width.

**Impact:** If a GGUF file is malformed or mismatched so that a norm tensor is shorter than the hidden size or head_dim, safe Rust reads past the end of a heap buffer on macOS, and Linux quietly computes a different result. That is undefined behavior that a model file can trigger. The operator chooses which model to load, and the out-of-bounds access is a read, not a write.

**Suggested action:**
1. Before the cfg split, add `assert!(weight.len() >= values.len())` to `rms_norm_in_place` and `assert!(weight.len() >= head_dim)` to `per_head_rms_norm_in_place`.
2. The better fix: check norm tensor lengths once at model load and return a load error on mismatch.
3. At each vDSP call, take `let p = values.as_mut_ptr();` once and pass `p.cast_const()` and `p`, so the shared and mutable pointers no longer come from separate borrows.
4. Add `// SAFETY:` comments that state the length invariant.
5. Verify with a unit test that passes a short `weight` and expects a panic (or a load error), and run it on macOS.

### X-CONC-02 psionic-backend-cuda: 198 unsafe blocks with no SAFETY comments; bounds invariants live only in distant wrappers

Severity: medium · Category: maintainability · Effort: L

**Locations:** [psionic-backend-cuda lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs) at lib.rs:696, lib.rs:9424, lib.rs:11422 and lib.rs:17061; [psionic Cargo.toml](../../../../crates/psionic/Cargo.toml) at Cargo.toml:9.

**Evidence:** At HEAD the file is 26,043 lines with 198 `unsafe {` blocks and 0 occurrences of `SAFETY`. `pub(super) fn write_bytes_at_offset` inside `mod platform` (9424) computes `self.inner.device_ptr.cast::<u8>().add(byte_offset)` and calls cudaMemcpy without a bounds check (11422-11440). The check `byte_offset.saturating_add(bytes.len()) > self.byte_len` exists only in the public `CudaBuffer` wrapper (701). `unsafe fn load_symbol` (17061) copies a function pointer out of a libloading `Symbol` and never explains why its lifetime is safe. The psionic lints (Cargo.toml:9-10) turn on pedantic but not `undocumented_unsafe_blocks`.

**Impact:** The invariants these unsafe blocks rely on are never written down and live thousands of lines away. A refactor or a new `pub(super)` caller could skip the bounds check and silently corrupt GPU memory.

**Suggested action:**
1. Move both `mod platform` blocks (9424, 17199) into their own files.
2. Make `PlatformBuffer::{write,read}_bytes_at_offset` repeat the bounds check, or turn them into `unsafe fn` with a `# Safety` section.
3. Add `// SAFETY:` comments, starting with the memcpy, free and destroy sites, then `load_symbol`.
4. Add `undocumented_unsafe_blocks = "warn"` to `[workspace.lints.clippy]` in `crates/psionic/Cargo.toml`.
5. Verify by checking that `rg -c 'SAFETY' lib.rs` goes up from 0, and that the clippy warning count for the crate goes down release over release.

### X-CONC-03 Gateway jobs are spawned without supervision; a panicking job stays 'running' forever and leaks its cancel entry

Severity: medium · Category: concurrency · Effort: M

**Locations:** [gateway jobs.rs](../../../../crates/gateway/src/jobs.rs) at jobs.rs:276, jobs.rs:484, jobs.rs:497, jobs.rs:527, jobs.rs:609, jobs.rs:675 and jobs.rs:714.

**Evidence:** `tokio::spawn(run(state.clone(), job.clone()));` at 276, and the resume spawns at 714/728, all drop their handles. `run()` inserts into `job_cancels` (484-488), and only the normal completion path removes the entry (`state.job_cancels.lock().await.remove(&id)` at 609). `run()` sets RUNNING with `let _ = write_status(..)` (497), which discards any error, and spawns `ledger_writer` over an unbounded channel (526-527). A RUNNING job becomes FAILED only during startup recovery (675, cause `gateway_restart`). `job_cancels` is a tokio Mutex, so it cannot be poisoned.

**Impact:** If anything in the run path panics, the job reports `running` until the gateway restarts. Pollers wait forever, webhooks never fire, and the cancel map entry leaks. No concrete panic path inside `run()` was traced, so this is a robustness gap, not an observed failure.

**Suggested action:**
1. Spawn jobs through a supervisor that awaits the `JoinHandle` and, on `JoinError::is_panic`, writes FAILED with cause `internal_panic`.
2. Create a `Drop` guard at the top of `run()` that removes the `job_cancels` entry on every exit path.
3. Replace `let _ = write_status(..)` with a logged error.
4. Verify with a test that uses a panicking classify stub and asserts the job reaches FAILED without a restart and leaves `job_cancels` empty.

### X-CONC-04 pay-host runs blocking SQLite calls under a std Mutex inside async handlers, contended by a 250 ms ingestion loop

Severity: medium · Category: performance · Effort: M

**Locations:** [pay-host main.rs](../../../../crates/pay-host/src/main.rs) at main.rs:74; [pay-host lib.rs](../../../../crates/pay-host/src/lib.rs) at lib.rs:191, lib.rs:611, lib.rs:620 and lib.rs:654.

**Evidence:** `pub type SharedStore = Arc<Mutex<Store>>` wraps a rusqlite `Connection` (186-191). The async `stats`, `snapshot` and `stream` handlers call `store.lock()` and run SQLite queries plus a route-map layout directly on Tokio workers. Each SSE stream polls `s.since(cursor)` and sleeps 250 ms between polls. main.rs:74 starts the ingestion loop with `spawn_blocking`, `std::thread::sleep(250ms)` and `ingestion.lock()`, and drops the handle. Ingestion errors are logged, but if ingestion panics it poisons the lock and every handler then returns 500 through `.map_err(internal)`. lib.rs:624 is one very long unformatted line.

**Impact:** Handlers block runtime workers whenever ingestion holds the lock. N SSE viewers mean N SQLite polls every 250 ms. A panic in ingestion silently stops the feed and poisons the store. This is a small flow-visualizer service and no outage has been observed.

**Suggested action:**
1. Have ingestion publish new sequence numbers through a tokio `watch` or `broadcast`, and make SSE await that instead of polling.
2. Run handler queries in `spawn_blocking`, or give readers their own read-only WAL connection.
3. Keep the ingestion `JoinHandle` and exit or restart if it panics.
4. Recover poisoned locks with `into_inner`.
5. Run `rustfmt` on lib.rs.
6. Verify by load-testing with several SSE clients and confirming the number of SQLite queries no longer scales with the number of viewers.

### X-CONC-05 Released user credentials can stay armed in a global map: arm happens before fallible steps, disarm is not RAII

Severity: medium · Category: security · Effort: S

**Locations:** [coder-cloud release.rs](../../../../crates/coder-cloud/src/release.rs) at release.rs:35, release.rs:105 and release.rs:123; [coder-cloud operator.rs](../../../../crates/coder-cloud/src/operator.rs) at operator.rs:447, operator.rs:459, operator.rs:1568 and operator.rs:1580.

**Evidence:** `static TURNS: Mutex<BTreeMap<String, Arc<Credentials>>>` has no size cap, unlike `OFFERS` with `MAX_OFFERS=1024`. operator.rs:1568/1576 call `release::arm(&job, ..)`. After that, `self.scope(..)?`, `accepted.validate().map_err(..)?` and `write(&path, ..)?` (1580-1590) can each return early without disarming. In the resume path, `if !crate::claude_task::resume(..) { return Ok(()); }` (447-449) also skips disarm, and so does a panic in `self.drive` (459). The module doc (release.rs:16) promises: "When the turn ends the driver disarms the job".

**Impact:** A released credential can outlive the turn it was released for, and `release::turn` (123-128) can then merge it into a later turn. That breaks the one-release-one-turn rule (BYO-05).

**Suggested action:**
1. Have `arm()` return an `Armed(String)` guard whose `Drop` calls `disarm`, and move the guard into `drive`.
2. If that is not practical, move the `arm()` calls after the last fallible step (after `write` at 1590), and add an explicit disarm on the resume early-return.
3. Add tests: a `validate()` failure after arm leaves `armed(job) == false`, and so does a panicking `drive`.

### X-CONC-06 Unbounded queues with no backpressure on inference workers and network frame pumps

Severity: medium · Category: concurrency · Effort: M

**Locations:** [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) at openai_http.rs:3934 and openai_http.rs:4573; [coder-host link.rs](../../../../crates/coder-host/src/client/link.rs) at link.rs:149; [coder-host guest.rs](../../../../crates/coder-host/src/client/guest.rs) at guest.rs:72; [coder-connect transport.rs](../../../../crates/coder-connect/src/transport.rs) at transport.rs:209; [gateway relay_worker.rs](../../../../crates/gateway/src/relay_worker.rs) at relay_worker.rs:730; [openagents-chat basic_link.rs](../../../../crates/openagents-chat/src/basic_link.rs) at basic_link.rs:173.

**Evidence:** There are 38 non-test `unbounded_channel` lines. `OpenAiCompatWorker::spawn` (3934) and the GPT-OSS worker spawn (4573) each use `mpsc::unbounded_channel()`, drained by a single named OS thread. openai_http.rs has no `TOO_MANY_REQUESTS`, `Semaphore` or queue-depth limit. In coder-host link.rs, inbound frames go into an unbounded channel (149), while outbound uses `mpsc::channel::<ToHost>(256)` (151).

**Impact:** A burst of completion requests queues without limit and holds prompt memory, so clients time out instead of being told to back off. A fast remote peer can grow client memory without limit while the consumer is stalled.

**Suggested action:**
1. In openai_http.rs, switch to `mpsc::channel(N)` with a configurable N (default about 64), and return 429 with `Retry-After` when `try_send` reports Full.
2. In link.rs:149 and guest.rs:72, use a bounded channel (e.g. 1024) and close the link on overflow.
3. Check the remaining sites (transport.rs, relay_worker.rs, basic_link.rs) and bound each one or document why it is safe unbounded.
4. Add a CI ratchet on the non-test `unbounded_channel` count, currently 38.
5. Verify with a load test that floods the completion endpoint and checks for 429s and flat memory.

### X-CONC-07 coder-host network listeners spin on accept errors, and direct/WebSocket listeners have no connection limit

Severity: medium · Category: concurrency · Effort: S

**Locations:** [direct.rs](../../../../crates/coder-host/src/serve/direct.rs) at direct.rs:34; [websocket.rs](../../../../crates/coder-host/src/serve/websocket.rs) at websocket.rs:51; [control/mod.rs](../../../../crates/coder-host/src/control/mod.rs) at mod.rs:87; [tailnet.rs](../../../../crates/coder-host/src/tailnet.rs) at tailnet.rs:372.

**Evidence:** All four loops use `let Ok((stream, _)) = listener.accept().await else { continue; };` with no backoff. direct.rs:41 and websocket.rs:55 spawn one task per connection with no `Semaphore`. By contrast, control/mod.rs:84 has `Semaphore::new(CONNECTIONS)` and tailnet.rs:368-375 calls `permits.try_acquire_owned()`.

**Impact:** EMFILE or any other persistent accept error makes the loop spin a core. The direct and WebSocket ports accept unlimited connections that are still waiting to finish their handshake.

**Suggested action:**
1. Add one `accept_loop(listener, Arc<Semaphore>, handler)` helper in coder-host that backs off exponentially on `Err` (50 ms up to 1 s) and gives each connection an owned permit.
2. Use it at all four sites, and add `DIRECT_CONNECTIONS` and `WEBSOCKET_CONNECTIONS` constants.
3. Verify with a test that lowers the fd limit (or injects accept errors) and confirms the loop backs off, and a test that confirms connections past the limit are refused.

### X-CONC-08 Hand-rolled libc file-ownership and locking code duplicated across 21 crates, mostly without SAFETY comments

Severity: medium · Category: duplication · Effort: L

**Locations:** [retail-service store.rs](../../../../crates/retail-service/src/store.rs) at store.rs:35; [x402 outcome.rs](../../../../crates/x402/src/outcome.rs) at outcome.rs:329; [openagents-web cloud/effects.rs](../../../../crates/openagents-web/src/cloud/effects.rs) at effects.rs:502; [commercial-spend lib.rs](../../../../crates/commercial-spend/src/lib.rs) at lib.rs:186; [private-fs lib.rs](../../../../crates/private-fs/src/lib.rs) at lib.rs:1.

**Evidence:** There are 71 `libc::geteuid` calls in 38 files across 21 crates, and 10 `libc::flock` sites. The usual shape is `m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0` (retail-service store.rs:35). Unsafe sites against SAFETY markers: x402 19/0, commercial-spend 8/0, retail-service 8/0, openagents-web 16/1. private-fs/src/lib.rs:1 describes itself as "Owner-only files and directories on Windows" and only translates the Unix rule into Windows terms.

**Impact:** A security-relevant owner-only check exists in about 70 slightly different copies. Fixing one copy does not fix the others.

**Suggested action:**
1. Add a `#[cfg(unix)]` module to `crates/private-fs` that exposes `is_private(&Metadata)`, `euid()` and no-follow open helpers.
2. Implement it with std (`File::try_lock`) or rustix. rustix is only a transitive dependency today, so adopt it only after a `docs/dependencies.md` review.
3. Migrate x402, openagents-web/cloud, commercial-spend, retail-service and pay-ledger first.
4. Add a CI check that `libc::geteuid|libc::flock` appears only in private-fs, and ratchet the count down to zero outside it.

### X-CONC-09 Tokio runtimes and HTTP clients built per call in sync wrappers (14 copies of `fn runtime()`)

Severity: low · Category: performance · Effort: M

**Locations:** [coder-sync lib.rs](../../../../crates/coder-sync/src/lib.rs) at lib.rs:328, lib.rs:530 and lib.rs:548; [openagents-cli screen.rs](../../../../crates/openagents-cli/src/screen.rs) at screen.rs:370; [coder worktree.rs](../../../../crates/coder/src/worktree.rs) at worktree.rs:197 and worktree.rs:208; [coder branch_checkout.rs](../../../../crates/coder/src/branch_checkout.rs) at branch_checkout.rs:28.

**Evidence:** In coder-sync, `client()` (328-335) builds a new `reqwest::Client` on every call, and `choice_now`/`choose_now` each build a new current_thread runtime (530-552). openagents-cli screen.rs:370 builds a runtime for every control-socket call. In worktree.rs, `locked` (197) and `mutate` (208) each build their own runtime. `git grep 'fn runtime() ->'` returns 14 matches. The headline figure of 273 non-test runtime constructions is inflated: 114 of them are in-file test helpers in openai_http.rs.

**Impact:** Every call pays for TLS and connection setup with no keep-alive, on top of building a runtime. Calling one of these wrappers from inside a runtime panics. The 14 helper copies drift apart.

**Suggested action:**
1. In coder-sync, add `static HTTP: OnceLock<reqwest::Client>` and one shared `OnceLock<Runtime>` bridge that returns an error when `Handle::try_current()` is Ok.
2. Replace the 14 `fn runtime()` helpers with that bridge.
3. Keep worktree.rs's async git supervision. Its doc says the lock is taken off the caller's runtime so cancellation cannot release it in the middle of a subprocess. Change only the runtime handling, so one transaction reuses one runtime instead of building one per step.
4. Verify with `git grep -c 'fn runtime() ->'` dropping to 1, and a test showing the bridge returns an error, rather than panicking, when called inside a runtime.

### X-CONC-10 openagents-chat Client::collect uses block_in_place, which panics on current-thread runtimes

Severity: low · Category: concurrency · Effort: M

**Locations:** [client.rs](../../../../crates/openagents-chat/src/client.rs) at client.rs:1106 and client.rs:1133; [client_tests.rs](../../../../crates/openagents-chat/src/client_tests.rs) at client_tests.rs:1; [openagents-cli main.rs](../../../../crates/openagents-cli/src/main.rs) at main.rs:499.

**Evidence:** `pub async fn collect` runs `tokio::task::block_in_place(|| crate::thread::collect(id, |command| handle.block_on(self.apply(command))))` (1106-1111). `observe()` falls through to `collect` (1133). 21 of the 27 tokio tests in client_tests.rs use `multi_thread`. This is the only non-test `block_in_place` in the repo. Today's only CLI caller (chat.rs:400/424) runs under openagents-cli's multi_thread runtime (main.rs:499).

**Impact:** No current production caller crashes, but the trap is armed: any future current_thread caller, or a test using the default `#[tokio::test]`, will panic in `collect`/`observe`.

**Suggested action:**
1. Make `thread::collect` take an async page fetcher so `Client::collect` can await `self.apply` directly.
2. Until then, add a `# Panics` doc section and a `debug_assert!` on `RuntimeFlavor::MultiThread`.
3. Verify by adding a current_thread `#[tokio::test]` that calls `collect` and passes.

### X-CONC-11 Lock-poisoning policy is inconsistent, with 17 duplicated `fn lock<T>` helpers

Severity: low · Category: duplication · Effort: M

**Locations:** [coder-connect client.rs](../../../../crates/coder-connect/src/client.rs) at client.rs:350; [coder-host authority.rs](../../../../crates/coder-host/src/authority.rs) at authority.rs:393; [coder-cloud release.rs](../../../../crates/coder-cloud/src/release.rs) at release.rs:37; [gateway relay_worker.rs](../../../../crates/gateway/src/relay_worker.rs) at relay_worker.rs:1004.

**Evidence:** `git grep 'fn lock<'` finds 17 helpers: coder-connect 5, coder-host 4, coder-computers 2, openagents-mobile 2, and one each in coder-cloud, openagents-chat, retail-cloud and retail-qualify. The gateway has 16 non-test `.lock().expect/unwrap` sites, for example relay_worker.rs:1004 `self.running.lock().expect("running")`. Repo-wide there are 433 non-test lock-unwrap sites and 296 sites that recover with `into_inner`.

**Impact:** Modules disagree on whether a poisoned lock should be recovered or should panic, and the helper copies drift apart. No poisoning incident or panic cascade has been observed.

**Suggested action:**
1. Add one `LockExt::lock_or_recover()` (plus read/write variants) to a small shared crate, and replace the 17 copies.
2. In long-running services such as gateway relay_worker, replace `.lock().expect` with recovery plus a logged warning.
3. Add a ratchet on the non-test `.lock().unwrap()|.expect(` count.
4. Verify with `git grep 'fn lock<'` returning only the shared definition.

### X-CONC-12 No lint enforces documented unsafe; x402 and wallet carry no lints at all

Severity: low · Category: build · Effort: M

**Locations:** [Cargo.toml](../../../../Cargo.toml) at Cargo.toml:1; [x402 Cargo.toml](../../../../crates/x402/Cargo.toml); [wallet Cargo.toml](../../../../crates/wallet/Cargo.toml); [rust-native Cargo.toml](../../../../crates/rust-native/Cargo.toml) at Cargo.toml:36; [openagents-mobile Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) at Cargo.toml:124; [psionic Cargo.toml](../../../../crates/psionic/Cargo.toml) at Cargo.toml:9.

**Evidence:** `[workspace.lints]` contains only `unsafe_op_in_unsafe_fn = deny` plus `dbg_macro`/`todo`/`unimplemented`; there is no `undocumented_unsafe_blocks`. rust-native (36-42) and bitcoin-amount have local `[lints]` sections that mirror the workspace set, and bitcoin-amount also sets `unsafe_code = "forbid"`. openagents-mobile is excluded from the workspace and mirrors the lints locally (124-131). Only x402 (19 unsafe sites), wallet, oa-copy and openagents-ui have no `[lints]` section at all.

**Impact:** Undocumented unsafe can keep growing with nothing to catch it, and x402, an FFI-heavy payment crate, does not even get `unsafe_op_in_unsafe_fn`.

**Suggested action:**
1. Add `[lints] workspace = true` to x402, wallet, oa-copy and openagents-ui.
2. Add `undocumented_unsafe_blocks = "warn"` to the root `[workspace.lints.clippy]`, to `crates/psionic/Cargo.toml`, and to the mirrored local lint blocks in rust-native and openagents-mobile.
3. Until clippy runs are cheap enough to gate on, enforce this with an rg ratchet on unsafe sites minus SAFETY markers per crate.
4. Verify that `cargo clippy -p x402` reports `unsafe_op_in_unsafe_fn` violations, if there are any, after the change.

### X-CONC-13 In-process static Mutex<()> guards on-disk read-modify-write state in coder task modules

Severity: low · Category: concurrency · Effort: M

**Locations:** [agent_jobs.rs](../../../../crates/coder/src/task/agent_jobs.rs) at agent_jobs.rs:51; [agent_plan.rs](../../../../crates/coder/src/task/agent_plan.rs) at agent_plan.rs:1083; [agent_consolidate.rs](../../../../crates/coder/src/task/agent_consolidate.rs) at agent_consolidate.rs:50; [publish.rs](../../../../crates/coder/src/task/publish.rs) at publish.rs:55; [studio_git.rs](../../../../crates/coder/src/task/studio_git.rs) at studio_git.rs:94.

**Evidence:** agent_jobs.rs:47-51 says it "Serializes every read-modify-write of a jobs file in this process" using `static WRITE: Mutex<()>`. agent_plan, agent_consolidate, publish ("One publication at a time on this computer") and studio_git ("One merge at a time on this computer") use similar process-local locks. The publish and studio_git comments promise exclusivity per computer, but a static Mutex only gives exclusivity within one process.

**Impact:** If two processes touch the same store, updates can be lost. Examples would be overlapping host generations, or a CLI running next to the host. No concrete cross-process writer was traced.

**Suggested action:**
1. In publish.rs and studio_git.rs, whose comments promise per-computer exclusivity, use the existing file-lock pattern (`landing_lock` in issue_run.rs at 1146).
2. For agent_jobs and agent_plan, either add a sibling lockfile or record "single host process owns these files" as an invariant in the host.
3. Verify with a test that runs two processes (or two independent lock holders on the same path) and checks they serialize.

### X-CONC-14 Fire-and-forget tasks and detached threads; long-lived loops die silently

Severity: low · Category: error-handling · Effort: M

**Locations:** [coder-host tailnet.rs](../../../../crates/coder-host/src/tailnet.rs) at tailnet.rs:338 and tailnet.rs:348; [gateway funding.rs](../../../../crates/gateway/src/funding.rs) at funding.rs:789; [gateway subscriptions.rs](../../../../crates/gateway/src/subscriptions.rs) at subscriptions.rs:1115; [gateway inference_status.rs](../../../../crates/gateway/src/inference_status.rs) at inference_status.rs:69; [pay-host main.rs](../../../../crates/pay-host/src/main.rs) at main.rs:74.

**Evidence:** tailnet.rs:348 spawns `serve_observer`. If it returns `Err`, the code only prints "openagents host: chat history stopped" and does not restart it. At 338, a `spawn_blocking` that warms the catalog is discarded. gateway funding.rs:789, subscriptions.rs:1115 and inference_status.rs:69 spawn infinite loops and drop the handles. The funding loop does exit cleanly when its `Weak` fails to upgrade. Repo-wide there are 105 fire-and-forget spawn statements, and only 18 files use JoinSet or TaskTracker.

**Impact:** When a background loop panics it simply disappears, and the service reports healthy while the feature is dead.

**Suggested action:**
1. Add a `supervised(name, fut)` helper per service, backed by a `JoinSet` or `TaskTracker`, that logs `JoinError::is_panic` and restarts the loop with backoff.
2. Convert the gateway loops and tailnet's observer first.
3. Verify with a test that injects a panic into one supervised loop and asserts that it restarts and that a log line is emitted.

### X-CONC-15 plugin-outline guest arena: a safe fn returns aliased &'static mut, and the bump pointer never resets

Severity: low · Category: maintainability · Effort: S

**Locations:** [plugin-outline lib.rs](../../../../crates/plugin-outline/src/lib.rs) at lib.rs:22, lib.rs:29 and lib.rs:43.

**Evidence:** `unsafe impl Sync for Arena {}` (22). The safe `fn heap() -> &'static mut Vec<u8>` (29-35) hands out a fresh `&mut` on every call. `ARENA.bump.fetch_add(len, ..)` (43) never resets and advances even when the allocation fails, so once cumulative allocations pass `HEAP_BYTES` (1 MiB), every allocation returns 0.

**Impact:** Under the aliasing rules this is undefined behavior, although the practical risk is low in a single-threaded Wasm diagnostic guest. A long-lived instance stops being able to allocate after 1 MiB in total.

**Suggested action:**
1. Make `heap()` return `*mut u8` and create slices per call, scoped to that call.
2. Reset `bump` at each export entry.
3. Use `compare_exchange` so a failed allocation does not advance `bump`.
4. Verify with a test that allocates more than 1 MiB in total across several export calls and succeeds.

### X-CONC-16 Tests depend on wall-clock sleeps, including thread::sleep inside runtime hooks

Severity: low · Category: testing · Effort: L

**Locations:** [verse-world net.rs](../../../../crates/verse-world/src/service/net.rs) at net.rs:1583; [verse-world worker_delayed.rs](../../../../crates/verse-world/src/service/worker_delayed.rs) at worker_delayed.rs:111.

**Evidence:** net.rs:1570-1590 is a default (current_thread) `#[tokio::test]` whose Tick hook calls `std::thread::sleep(Duration::from_millis(80))` to force a late wake. worker_delayed.rs:111 sleeps for `stall_ms` inside a persistence inject hook. Repo-wide counts, not re-measured by the verifier: 342 `std::thread::sleep` calls and 231 `tokio::time::sleep` calls in tests.

**Impact:** These tests are slow and timing-sensitive, which makes them flaky on loaded CI hosts.

**Suggested action:**
1. Make the clock in the verse-world tick loop injectable.
2. Rewrite these tests with `#[tokio::test(start_paused = true)]` so the late-wake scenario advances fake time instead of sleeping.
3. Verify that the tests pass deterministically and that their wall-clock runtime drops.

## Refuted during verification

The verifier did not reject any finding outright. These corrections were made and should not be raised again in their original form:

- **Eight crates do not opt into workspace lints (X-CONC-12).** This overstates the gap. rust-native and bitcoin-amount have equivalent local `[lints]` sections, and bitcoin-amount also forbids `unsafe_code`. openagents-mobile is outside the workspace and mirrors the lints locally. Only x402, wallet, oa-copy and openagents-ui have no lints.
- **issue_run.rs:1100 lacks cross-process locking (X-CONC-13).** The site was removed from the finding. Its `landing_lock` (1146) already takes a file lock.
- **Replace worktree.rs's async git with `std::process::Command` (X-CONC-09).** The advice was changed. Doing that would drop the cancellation and termination supervision the code documents, so only the per-step runtime construction should change.
- **273 non-test runtime constructions (X-CONC-09).** The figure is inflated. 114 of them are in-file test helpers in psionic-serve openai_http.rs.
- **Guards held across `.await` and lock-ordering hazards.** All 21 await candidates and both nested-lock candidates were false positives.
