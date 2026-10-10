# Mobile (Rust + iOS/Android shells)

Scope: `crates/openagents-mobile` (its own Cargo workspace), `bins/{openagents-ios, openagents-android, coder-ios, coder-android, openagents-mockup-ios}`, `swift/`. Audit date 2026-10-10, snapshot `3168c986aa`.

**Health grade: B**

The Rust core in `crates/openagents-mobile` (about 29.2k lines in 55 files, about 231 tests) is well built. Rust owns the state, and each host renders the views Rust sends and returns activations. The FFI code is careful: non-test code has no `unwrap`, `expect` or `panic`, every C export runs inside `catch_unwind` and checks input sizes, and the Android JNI layer records panic locations, enforces one thread per handle and keeps its size limits as named constants. The weaknesses are structural. `app.rs` is a god object (about 45 fields on `App`, about 90 `Request` variants, `App::open` and `App::call` at 354 and 319 lines), and `wallet.rs` and `account_link.rs` are the next-largest files. The crate is excluded from the root workspace and the repo has no CI workflows, so none of its tests run automatically. That includes the 57 `coder_tab` tests, which are in practice the main regression suite for the shared `openagents-chat-app` crate (that crate has only 6 of its own). Fifteen two-line modules only re-export `openagents-chat-app`, and a shared crate reads a fixture out of this crate's directory, so the layering runs backwards. On the host side, iOS shares Coder's Swift files through relative paths, while Android keeps forked copies that have drifted. Several money and secret paths need tightening: the over-1,000-sat approval check lives only in the host (with a fail-open branch on Android), the world key crosses the bridge as hex on many requests, seeds and mnemonics are never wiped, and the preview gate hides UI without removing the request handlers from release builds. About 82 MB of verification captures sit in git, 145 of them with absolute `/Users/` paths.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC (`crates/openagents-mobile`) | 29,230 in 55 `.rs` files |
| Cargo.lock | 10,651 lines, 943 packages; still carries secp256k1 x4, reqwest x2, jni x2 |
| Largest Rust files | `coder_tab_tests.rs` 3,797; `wallet.rs` 3,751 (about 2,056 non-test); `app.rs` 2,564; `account_link.rs` 1,893 (no inline tests); `spend.rs` 1,329; `tests.rs` 1,251; `account.rs` 1,157; `trainer.rs` 1,062 |
| Test-only lines | about 7.8k |
| `#[test]` / `#[tokio::test]` | about 231, of which 13 are `#[ignore]` network tests |
| Non-test `unwrap`/`expect`/`panic`/`unreachable` | 0 |
| `#[allow]` | 3 (`verse.rs:83`, `android.rs:139`, `trainer.rs:482`) |
| TODO/FIXME | 0 |
| `let _ =` / `.ok();` discards | `app.rs` 19, `wallet.rs` 13, `account_link.rs` 9, `playtest.rs` 6 |
| `Result<_, String>` sites | about 79 (`wallet.rs` 21, `wallet_fixture.rs` 16, `trainer.rs` 11, `payees.rs` 8); typed error enums: 1 (`SettleError`) |
| `std::thread::spawn` in `wallet.rs` | 14 |
| Re-export-only shim modules | 15 (2 lines each) |
| C exports | 14 (`lib.rs` 8, `verse.rs` 4, `studio.rs` 1, `preview.rs` 1), matching 14 hand-written declarations in `OpenAgentsMobile.h` |
| JNI exports | 33 extern fns in `android.rs` and `android/*` |
| `App` / `Request` / `Packet` | about 45 fields / about 90 variants / about 35 fields |
| Longest functions | `App::open` 354 (`app.rs:901`), `App::call` 319 (`app.rs:1506`), `Poller::pass` about 259 (`account_link.rs:1448`), `Wallet::screen` about 197 (`wallet.rs:1486`), `App::packet` 167 (`app.rs:2182`) |
| Commits touching the crate | 221; last on 2026-10-09 |
| Swift (openagents-ios + coder-ios) | 21.5k LOC; largest `NativeTranscriptPainter.swift` 2,302, `VerseTab.swift` 1,394, `NativeChat.swift` 1,313, `WalletTab.swift` 1,277, `MobileBridge.swift` 1,111 |
| Kotlin (both Android hosts) | 15.3k LOC; largest `TranscriptPainter.kt` 1,260, `MainActivity.kt` 1,236; `!!` 20; catch-all catches 18 |
| Tracked verification artifacts | 738 files, about 82 MB (407 png, 166 log, 101 json, 1 mp4); 145 contain `/Users/` |
| `swift/` | 4 tracked files, 338 Swift LOC (lev-bridge only) |
| `openagents-mockup-ios` | 48 Swift files, 7.1 MB |
| CI workflows | 0 |
| Lockfile drift vs root (tokio, serde, rustls, url) | none |

## Strengths

- FFI discipline: every C export (`lib.rs:101-243`, `studio.rs:123`, `verse.rs:252`) checks for null pointers and input sizes and runs inside `catch_unwind`. Non-test code has zero `unwrap`, `expect` or `panic`.
- The Android JNI bridge is careful: `guarded()` records the panic location (`android.rs:84-94`), size limits are named constants (`android.rs:20-32`), main-thread and worker-thread rules are enforced (`android/exports.rs:51-63`), and UTF-16 length is checked before allocating.
- Rust owns state and the hosts are thin, so most product logic is testable on a desktop machine. `android.rs` compiles under `cfg(test)`, so the JNI glue has host tests.
- Network tests are `#[ignore]`d with a reason (e.g. `wallet.rs:3595`, `tests.rs:301`); live-host smokes read `OPENAGENTS_TEST_*` variables rather than hard-coded values.
- Secret hygiene by design: `Seed` has no `Debug`, the device key leaves only through an explicit Account reveal, and chat uploads are screened for credential shapes (`account_link.rs` `upload_messages`).
- Money safety: the spend ledger reserves amount plus fee ceiling before paying, derives an idempotency key from the request ID, and re-checks the grant on Approve (`spend.rs:832-880`).
- Debug fixtures (`chat_fixture`, `gym_fixture`, `wallet_fixture`) are compiled out of release builds (`lib.rs:30-61`); debug-only launch overrides are filtered by `cfg!(debug_assertions)`.
- Link previews are bounded: https only, at most 3 redirects, separate page and image time limits, size limits, at most 3 concurrent reads (`link_fetch.rs`).
- No lockfile drift against the root for shared core dependencies.
- `Cargo.toml` comments explain why each dependency exists, including why the crate is its own workspace.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| MOB-01 | high | testing | Nothing runs the mobile crate's tests automatically, though they are the main regression suite for shared chat-app code | M |
| MOB-02 | medium | security | Spend approval above 1,000 sats relies on the host for user presence; Rust `approve()` accepts any waiting request | S |
| MOB-03 | medium | maintainability | `app.rs` is a god object: ~90 `Request` variants, 300+ line `open()`/`call()` | L |
| MOB-04 | medium | duplication | Android hosts keep drifted forks of shared Kotlin files; iOS shares via relative paths into coder-ios | L |
| MOB-05 | medium | security | Preview release gate only hides UI; Tailnet, Trainer and display-name requests are still compiled and answered | M |
| MOB-06 | medium | security | World key crosses the FFI as hex on every trainer/report/gym request; seeds never wiped | M |
| MOB-07 | medium | concurrency | A thread per wallet event/action and a new Tokio runtime per relay call | M |
| MOB-08 | medium | error-handling | Persistence failures silently ignored, writes not atomic, duplicate `keep()` | M |
| MOB-09 | medium | error-handling | iOS C bridge: literal size limits, no output cap, no panic diagnostics, hand-written header | M |
| MOB-10 | medium | concurrency | Android change-wait loop dies on any throwable; app handles tied to one replaceable executor thread | S |
| MOB-11 | medium | security | Account HTTP client falls back to a default client (redirects, no timeouts) and reads unbounded bodies | S |
| MOB-12 | medium | repo-hygiene | About 82 MB of verification captures in git, 145 files with `/Users/` paths | M |
| MOB-13 | low | error-handling | Stringly-typed errors: `Result<_, String>` mixes on-screen copy with error identity | L |
| MOB-14 | low | maintainability | `wallet.rs` and `account_link.rs` are god modules with manual field resets | L |
| MOB-15 | low | architecture | Shared crate reads mobile's fixtures; 15 two-line shim modules keep old paths alive | M |
| MOB-16 | low | performance | iOS host decodes each packet twice on the main thread; `attachImage` applies packets differently | S |
| MOB-17 | low | security | Link previews auto-fetch agent-chosen URLs with only hostname-based private-host blocking | S |
| MOB-18 | low | duplication | Duplicate Verse surface config structs with matching `#[allow(dead_code)]` fields | S |
| MOB-19 | low | docs | Stale comments: "four surfaces" listing six; "Chat / Code switch" | S |
| MOB-20 | low | dead-code | `openagents-mockup-ios` is an unmaintained design fork | S |
| MOB-21 | low | repo-hygiene | Top-level `swift/` holds only the unrelated lev-bridge macOS tool | S |
| MOB-22 | low | maintainability | `Launch` mixes release config with debug-only fixture knobs | S |
| MOB-23 | low | duplication | Small helpers duplicated: `now()`, `hex()`, QR-module rendering | S |
| MOB-24 | low | maintainability | Host god files and silent no-op keystore failures in `MobileBridge.kt` | L |

### MOB-01 Nothing runs the mobile crate's tests automatically, even though they are the main regression suite for shared chat-app code

Severity: high · Category: testing · Effort: M

Locations:
- [Cargo.toml](../../../../Cargo.toml) (`Cargo.toml:4-6`)
- [coder_tab_tests.rs](../../../../crates/openagents-mobile/src/coder_tab_tests.rs)
- [coder_tab.rs](../../../../crates/openagents-chat-app/src/coder_tab.rs)
- [openagents-chat-app lib.rs](../../../../crates/openagents-chat-app/src/lib.rs) (`lib.rs:34`)

Evidence: The root `Cargo.toml` excludes `crates/openagents-mobile` (lines 4-6, "Its own workspace, for Breez's SQLite"). `.github` holds only `ISSUE_TEMPLATE`; there are no workflows. `coder_tab_tests.rs` has 57 test functions, while `openagents-chat-app/src/coder_tab.rs` has 6. The mobile tests exercise chat-app types through the 2-line re-export shims. (Verifier re-counted 57 vs 6.)

Impact: A change to `openagents-chat-app` can pass root-workspace tests and still break 57 phone tests that only run when someone remembers `--manifest-path`.

Suggested action:
1. Move tests that only use `openagents_chat_app` types (`CoderTab`, `Chats`, `coder_list::Store`) into `crates/openagents-chat-app/src/coder_tab/tests.rs`, which runs in the root workspace. Keep `App`, wallet and FFI tests in mobile.
2. Add `scripts/check-mobile.sh` running `cargo test --locked --manifest-path crates/openagents-mobile/Cargo.toml`.
3. Call it from the iOS and Android release scripts before they archive.
4. Verify: `cargo test -p openagents-chat-app` in the root workspace reports the moved tests; the release scripts fail when a mobile test is broken on purpose.

### MOB-02 Spend approval above 1,000 sats relies on the host for the user-presence check; Rust's approve() accepts any waiting request

Severity: medium · Category: security · Effort: S

Locations:
- [spend.rs](../../../../crates/openagents-mobile/src/spend.rs) (`spend.rs:41-42`, `spend.rs:832-888`, `spend.rs:1196`)
- [AgentPayments.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/AgentPayments.kt) (`AgentPayments.kt:111-132`)
- [AgentPayments.swift](../../../../bins/openagents-ios/host/App/AgentPayments.swift) (`AgentPayments.swift:207-222`)

Evidence: Rust only sets `authenticate: w.request.amount_msat > AUTHENTICATE_ABOVE_MSAT` on the view (`spend.rs:1196`). `approve(&self, request, node, transport)` takes no proof of authentication; it re-reserves against the grant and pays. `AgentPayments.kt:123-127` still calls `bridge.spend(op, ...)` when `createConfirmDeviceCredentialIntent` returns null in the non-trust branch, and uses the deprecated `KeyguardManager` API under `@Suppress("DEPRECATION")`. iOS correctly gates on `LAContext`.

Impact: A host bug, or a future host, could approve payments above 1,000 sats with no user-presence check. The Android null-intent branch is fail-open. In practice it is close to unreachable: it follows an `isDeviceSecure` check, and the intent is essentially only null on insecure devices.

Suggested action:
1. In `AgentPayments.kt`, treat a null intent as failure and refuse in both branches.
2. Migrate to `androidx.biometric.BiometricPrompt` with `DEVICE_CREDENTIAL`.
3. Optionally, as defense in depth (not a closed hole, since Rust still trusts the host), add `authenticated: bool` to `spend_approve` and have `approve()` require it when `amount_msat > AUTHENTICATE_ABOVE_MSAT`.
4. Verify: a spend test that calls approve over the threshold without `authenticated` is refused; a host unit test shows null intent does not call `bridge.spend`.

### MOB-03 app.rs is a god object: Request has 90 variants, and open()/call() are 300+ line functions

Severity: medium · Category: maintainability · Effort: L

Locations:
- [app.rs](../../../../crates/openagents-mobile/src/app.rs) (`app.rs:174-614` Request, `app.rs:831-894` App, `app.rs:901-1255` open, `app.rs:1295-1413` respond, `app.rs:1506-1825` call, `app.rs:2182-2349` packet)

Evidence: `Request` starts at line 174 with about 90 variants. `respond()` is a chain of `if let Request::X = request { return ... }`. The trainer error JSON `{"schema":"openagents.trainer.v1","error":...}` is built three times (around 1322, 1343, 1368). The file is 2,564 lines.

Impact: Every feature touches one 2.5k-line file, causing merge conflicts between concurrent agents. Hosts must know which schema each op returns. No defect shown.

Suggested action:
1. Nest `Request` into per-surface enums (Wallet, Spend, Tailnet, Trainer, Account), keeping existing `op` names via serde.
2. Move each surface's handler bodies into its own module.
3. Replace `respond()`'s if-chain with one `Reply` enum serialized in one place, plus a `trainer_error()` helper.
4. Verify: add a serde round-trip test over every op name the hosts send (grep both `MobileBridge` files for op strings) and keep it green through the refactor.

### MOB-04 The Android hosts keep forked copies of shared Kotlin files that have drifted, while iOS shares them through relative paths into coder-ios

Severity: medium · Category: duplication · Effort: L

Locations:
- [openagents-android PinchAdmission.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/PinchAdmission.kt)
- [coder-android PinchAdmission.kt](../../../../bins/coder-android/host/app/src/main/java/com/openagents/coder/PinchAdmission.kt)
- [QRScanner.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/QRScanner.kt)
- [VerseSurface.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/VerseSurface.kt)
- [project.yml](../../../../bins/openagents-ios/host/project.yml) (`project.yml:16-36`)

Evidence: Line counts, Coder vs OpenAgents: PinchAdmission 22 vs 60, QRScanner 115 vs 101, GymPanel 201 vs 219, VerseSurface 577 vs 431, NativeRenderer 232 vs 497. The iOS `project.yml` compiles 10 files from `../../coder-ios/host/App` (NativeView, NativeChat, NativeTranscriptPainter, QRScanner, QRDecoder, SecretInputField, TerminalKeyboard, PinchAdmission, DeviceMotion, GymBoard).

Impact: Fixes land in one Android app and not the other. On iOS, one product folder is a hidden source dependency of another (deliberate and working today; the main cost is the Android drift).

Suggested action:
1. Extract a Gradle library module (e.g. `bins/android-shared/native`) with PinchAdmission, QRScanner and NativeRenderer, starting from the newer 60-line PinchAdmission.
2. Make both Android apps depend on it and delete the forks.
3. On iOS, move the 10 shared files into a local Swift package referenced by both `project.yml` files.
4. Verify: both Android apps and both iOS apps build; `find bins -name PinchAdmission.kt` returns one file.

### MOB-05 The preview release gate only hides the UI: Tailnet, Trainer and display-name requests are compiled into release builds and still answered

Severity: medium · Category: security · Effort: M

Locations:
- [preview.rs](../../../../crates/openagents-mobile/src/preview.rs) (`preview.rs:1-29`)
- [app.rs](../../../../crates/openagents-mobile/src/app.rs) (`app.rs:1077`, `app.rs:1305-1374`)
- [Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) (`Cargo.toml:80-82`)

Evidence: `preview.rs:5` says "Release builds and normal debug builds hide them; their code stays." `preview::ON` is checked only at `app.rs:1077` and `2273`, `verse.rs:71/216/232`, `playtest.rs:50` and the Android exports. `respond()` handles `SetDisplayName`, `Trainer`, `TrainerProfile`, `TrainerExport` and `TrainerLink` with no `ON` check. `ts_control`, `ts_control_serde` and `ts_keys` are unconditional dependencies.

Impact: Release binaries carry and answer code paths that have not been release-reviewed.

Suggested action:
1. Add one early guard in `respond()`/`call()` that refuses preview-only ops when `!preview::ON`.
2. Add a test that each preview-only op is refused with the gate off.
3. Longer term, make `preview` a Cargo feature with optional `ts_*` dependencies and `cfg`-gated tailnet/trainer modules.
4. Verify: `cargo tree --manifest-path crates/openagents-mobile/Cargo.toml` without the feature shows no `ts_*` crates.

### MOB-06 The world key crosses the FFI as hex on every trainer, report and gym request, and seeds are never wiped

Severity: medium · Category: security · Effort: M

Locations:
- [MobileBridge.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/MobileBridge.kt)
- [MobileBridge.swift](../../../../bins/openagents-ios/host/App/MobileBridge.swift) (`MobileBridge.swift:518-931`)
- [app.rs](../../../../crates/openagents-mobile/src/app.rs) (`app.rs:297-352`, `app.rs:883`, `app.rs:1310-1374`)
- [wallet.rs](../../../../crates/openagents-mobile/src/wallet.rs) (`wallet.rs:787-792`)

Evidence: `Request` variants carry `world_secret_hex: String` (`app.rs:297, 308, 315, 322, 338, 345, 352, 505`), although `App` already holds `world: Option<SecretKey>` (`app.rs:883`, set at 1789). `MobileBridge.kt` has 7 occurrences of `world_secret_hex` and `MobileBridge.swift` has 8 (gym_world, trainer, trainer_profile, trainer_export, trainer_link, report_send, feedback_send, reports). `wallet.rs:788-792` clones the mnemonic then calls `drop(mnemonic)` without zeroing. There is no `zeroize` dependency.

Impact: Key material is copied into many immutable host strings and heap buffers, widening exposure through crash dumps.

Suggested action:
1. Hand the world key to Rust once; remove `world_secret_hex` from Trainer*, ReportSend, FeedbackSend and Reports and use `self.world`.
2. Add `zeroize` to the Seed and mnemonic holders.
3. Verify: `rg world_secret_hex bins/` drops to the launch and Verse-create sites only; existing trainer and report tests pass.

### MOB-07 A thread per wallet event and action, and a new Tokio runtime per relay call, while App owns a multi-thread runtime

Severity: medium · Category: concurrency · Effort: M

Locations:
- [wallet.rs](../../../../crates/openagents-mobile/src/wallet.rs) (`wallet.rs:788-826`)
- [payees.rs](../../../../crates/openagents-mobile/src/payees.rs) (`payees.rs:246-256`)
- [trainer.rs](../../../../crates/openagents-mobile/src/trainer.rs) (`trainer.rs:180-190`)
- [spend.rs](../../../../crates/openagents-mobile/src/spend.rs) (`spend.rs:881`)

Evidence: `wallet.rs` has 14 `std::thread::spawn` calls. The subscribe callback (821-825) spawns an OS thread per event, `std::thread::spawn(move || read(&shared, &home, generation, false))`, with no coalescing. `payees.rs:246` (`runtime()`) and `trainer.rs:180` (`RelayPublish`) build a new current-thread runtime per query or publish. `spend.rs:881` spawns a thread per approval.

Impact: Bursts of wallet events produce unbounded concurrent reads that race on state and files, and waste battery.

Suggested action:
1. Coalesce wallet refreshes with an `AtomicBool` dirty flag and a single worker.
2. Pass the app's Tokio `Handle` to `Wallet`, payees and `RelayPublish` instead of building runtimes.
3. Verify: a test firing 100 fake events causes at most 2 reads.

### MOB-08 Persistence failures are silently ignored, writes are not atomic, and account_link has two identical keep() functions

Severity: medium · Category: error-handling · Effort: M

Locations:
- [wallet.rs](../../../../crates/openagents-mobile/src/wallet.rs) (`wallet.rs:244-248`, `wallet.rs:681`, `wallet.rs:1465-1469`)
- [account_link.rs](../../../../crates/openagents-mobile/src/account_link.rs) (`account_link.rs:596-605`, `account_link.rs:1707-1716`)

Evidence: `write_json` drops serialization and IO errors (`let _ = std::fs::write(...)`). The words-saved marker (681) and the trust file (1467) are written with `let _ =`, and `acknowledge()` then sets `trust_acknowledged = true` regardless. `Link::keep` and `Poller::keep` are identical apart from `lock()` vs `lock(&self.state)`.

Impact: A failed write of `WORDS_SAVED_FILE` brings the backup card back. A partial JSON write loses history after a crash.

Suggested action:
1. Add `write_atomic` (write `.tmp`, fsync, rename) for the wallet JSON files and markers.
2. Surface failures of user-confirmed writes through `shared.error`, and only set `trust_acknowledged` on success.
3. Collapse the two `keep()` functions into one free function.
4. Verify: a test with a read-only home directory shows the error surfaced and the flag unchanged.

### MOB-09 The iOS C bridge hard-codes size limits, has no output cap, keeps no panic diagnostics, and its header is hand-written

Severity: medium · Category: error-handling · Effort: M

Locations:
- [lib.rs](../../../../crates/openagents-mobile/src/lib.rs) (`lib.rs:103`, `lib.rs:130`, `lib.rs:190`)
- [android.rs](../../../../crates/openagents-mobile/src/android.rs) (`android.rs:21-32`, `android.rs:53-86`)
- [OpenAgentsMobile.h](../../../../bins/openagents-ios/host/App/OpenAgentsMobile.h)

Evidence: `lib.rs` uses `16 * 1024`, `128 * 1024`, `1024` and `96` as literals. `android.rs` has `MAX_CONFIG_BYTES`, `MAX_REQUEST_BYTES` and `MAX_PACKET_BYTES` (2 MiB, checked at line 101) plus `LAST_PANIC`. The 14 `no_mangle` exports match the 14 hand-written declarations in `OpenAgentsMobile.h`, but nothing checks that they stay in sync.

Impact: iOS failures carry no cause, limits can drift between platforms, and an ABI change goes undetected.

Suggested action:
1. Move the limits into a shared `bridge.rs` used by both `lib.rs` and `android.rs`, including an output cap for iOS.
2. Add an `openagents_mobile_last_error` export that reuses the panic hook.
3. Generate the header with cbindgen, or add a test comparing the header's function names to the exports.
4. Verify: renaming an export fails the test.

### MOB-10 The Android change-wait loop dies permanently on any throwable, and app handles are tied to one executor thread that is replaced if a task throws

Severity: medium · Category: concurrency · Effort: S

Locations:
- [MobileBridge.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/MobileBridge.kt) (`MobileBridge.kt:25`, `MobileBridge.kt:204-215`)
- [exports.rs](../../../../crates/openagents-mobile/src/android/exports.rs) (`exports.rs:38-41`)

Evidence: `catch (problem: Throwable) { break }` at `MobileBridge.kt:207` has no log and no restart. `APPS` and `VERSES` are `thread_local!` (`exports.rs:38-41`). `worker` is a `newSingleThreadExecutor` used via `worker.execute` at lines 58, 449, 517, 535 and 614; an exception in an `execute` task kills the thread, and the replacement thread sees an empty map.

Impact: The UI stops refreshing, or every Rust handle becomes unreachable, until the process restarts.

Suggested action:
1. Log and back off in the wait loop instead of breaking.
2. Wrap each worker task in `runCatching` with logging, or move `APPS` into a process-wide `Mutex` map.
3. Verify: a host test that throws inside a worker task, then issues another call on the same handle, succeeds.

### MOB-11 The account HTTP client silently falls back to defaults that follow redirects with no timeouts, and reads unbounded bodies

Severity: medium · Category: security · Effort: S

Locations:
- [account_link.rs](../../../../crates/openagents-mobile/src/account_link.rs) (`account_link.rs:60`, `account_link.rs:229-239`, `account_link.rs:258-279`)

Evidence: `.build().unwrap_or_default()` falls back to a default `reqwest::Client`, which follows up to 10 redirects and has no timeout, while bearer tokens are attached per request. `response.bytes().await` has no cap, although `MAX_REPLY_BYTES = 16 KiB` (line 60) is applied elsewhere (line 782).

Impact: Client initialization failure fails open (rare in practice), and the phone buffers whatever body size the server sends.

Suggested action:
1. Make `Https::new` return a `Result`, or fail closed.
2. Read bodies through a bounded reader with an explicit cap.
3. Verify: a test against a local server returning an oversized body gets an error rather than the full body.

### MOB-12 About 82 MB of verification captures in git, with absolute /Users/ paths in 145 files

Severity: medium · Category: repo-hygiene · Effort: M

Locations:
- [bins/openagents-ios/verification](../../../../bins/openagents-ios/verification)
- [bins/coder-ios/verification](../../../../bins/coder-ios/verification)
- [bins/openagents-android/verification](../../../../bins/openagents-android/verification)
- [bins/openagents-mockup-ios/verification](../../../../bins/openagents-mockup-ios/verification)

Evidence: `git ls-files 'bins/*/verification/*'` lists 738 files totalling 81,976 KB by du; 145 contain `/Users/`.

Impact: The clone grows with every build, and local paths leak into public history.

Suggested action:
1. Move PNG, MP4, log and gz captures to Git LFS or R2; keep README files plus checksums in git.
2. Add a check script that fails on large files or `/Users/` in `verification/`.
3. Rewrite home paths to `~` in the files that stay.
4. Verify: `git ls-files 'bins/*/verification/*' | xargs grep -l /Users/` returns nothing.

### MOB-13 Stringly-typed errors throughout: Result<_, String> mixes on-screen copy with error identity

Severity: low · Category: error-handling · Effort: L

Locations:
- [wallet.rs](../../../../crates/openagents-mobile/src/wallet.rs) (`wallet.rs:787-792`)
- [payees.rs](../../../../crates/openagents-mobile/src/payees.rs) (`payees.rs:246-250`)
- [account_link.rs](../../../../crates/openagents-mobile/src/account_link.rs) (`account_link.rs:258-279`)

Evidence: Errors are user copy produced by `map_err(|_| ...)`, e.g. "The wallet's folder could not be created on this phone." (`wallet.rs:790`) and "Couldn't reach openagents.com. Check your connection." (`account_link.rs:274, 279`). The source error is discarded.

Impact: No field diagnostics, callers cannot tell retryable from permanent errors, and tests assert on copy.

Suggested action:
1. Start with `wallet.rs`: a thiserror `WalletError` carrying the source, with a `message()` that returns the current copy.
2. Map to a string only at the packet boundary.
3. Verify: existing wallet tests pass unchanged on copy; new tests match on variants.

### MOB-14 wallet.rs (3,751 lines) and account_link.rs (1,893 lines) are god modules with manual field resets

Severity: low · Category: maintainability · Effort: L

Locations:
- [wallet.rs](../../../../crates/openagents-mobile/src/wallet.rs) (`wallet.rs:697-747`)
- [account_link.rs](../../../../crates/openagents-mobile/src/account_link.rs) (`account_link.rs:1448-1706`)

Evidence: `forget()` resets `Shared`'s fields one at a time; `Poller::pass` spans about 1448-1706.

Impact: A field added to `Shared` but missed in `forget()` leaks the old wallet's state after a restore. No observed bug.

Suggested action:
1. Split `Shared` into a `Default`-able per-wallet `Session` and have `forget()` call `std::mem::take`.
2. Split both files into submodules.
3. Verify: a test that `forget()` leaves `Session::default()`.

### MOB-15 A shared crate depends on mobile's fixtures directory, and 15 two-line shim modules keep old paths alive

Severity: low · Category: architecture · Effort: M

Locations:
- [openagents-chat-app lib.rs](../../../../crates/openagents-chat-app/src/lib.rs) (`lib.rs:34`)
- [openagents-mobile lib.rs](../../../../crates/openagents-mobile/src/lib.rs) (`lib.rs:18-66`)
- [gym.rs](../../../../crates/openagents-mobile/src/gym.rs)

Evidence: `include_str!("../../openagents-mobile/fixtures/gym-report.json")` at chat-app `lib.rs:34`. The 15 shims (wake, transcripts, router, outbox, hosted, gym, coder_tab, chats, first_run, eval_cards, coder_list, cli_run, chat_invites, basic_coder, basic_chats) are 2 lines each. 21 Markdown references under docs and crates still point at these shim paths.

Impact: Layering runs backwards, and doc readers land on empty files.

Suggested action:
1. Move `gym-report.json` to `crates/openagents-chat-app/fixtures`; repoint `include_str!` and the test writer.
2. Delete the shims and import `openagents_chat_app::X` directly.
3. Update the 21 doc links.
4. Verify: `rg 'openagents-mobile/fixtures' crates/openagents-chat-app` is empty; both workspaces build and test.

### MOB-16 The iOS host decodes each packet twice on the main thread, and attachImage applies packets differently

Severity: low · Category: performance · Effort: S

Locations:
- [MobileBridge.swift](../../../../bins/openagents-ios/host/App/MobileBridge.swift) (`MobileBridge.swift:989-1012`, `MobileBridge.swift:1022-1046`, `MobileBridge.swift:1072-1092`)

Evidence: `call()` hands raw `Data` to `DispatchQueue.main`, where `send()` runs `JSONDecoder.decode(AppPacket)` and also `JSONSerialization.jsonObject` to read `computers`. `attachImage`'s reply skips `settleLink`, `coder_go`, the terminal reset and `open_url`. `attachImage` is gated by `attachmentsEnabled` and chat is currently text only (#10093), so the divergence is latent.

Impact: Main-thread work scales with packet size.

Suggested action:
1. Decode off the main thread.
2. Add `computers` as a typed `AppPacket` field.
3. Route both paths through one `@MainActor apply(_:)`.
4. Verify: Instruments shows no JSON decoding on the main thread during a long transcript.

### MOB-17 Link previews auto-fetch URLs chosen by agents, with only a hostname-based block on private hosts

Severity: low · Category: security · Effort: S

Locations:
- [link_fetch.rs](../../../../crates/openagents-mobile/src/link_fetch.rs) (`link_fetch.rs:22-52`, `link_fetch.rs:54-67`)

Evidence: `allowed()` rejects IP literals and names ending in `.local`, `.localhost`, `.internal`, `.lan` or `.home.arpa`, but not DNS names that resolve to private addresses. `client()` builds a new `Client` per read.

Impact: A small LAN-probing and IP-tracking risk.

Suggested action:
1. Add a `dns_resolver` that rejects private, loopback and link-local results.
2. Build the client once.
3. Verify: a test with a resolver stub mapping a public-looking name to 192.168.x.x is refused.

### MOB-18 Duplicate Verse surface config structs with matching #[allow(dead_code)] fields

Severity: low · Category: duplication · Effort: S

Locations:
- [verse.rs](../../../../crates/openagents-mobile/src/verse.rs) (`verse.rs:75-100`)
- [android.rs](../../../../crates/openagents-mobile/src/android.rs) (`android.rs:130-150`)

Evidence: Both are `deny_unknown_fields` structs with the same `computer_hud` doc and `#[allow(dead_code)]`, plus `width`, `height`, `scale` and `world_secret_hex`.

Impact: A new host field must be added in two places.

Suggested action:
1. Use a single `SurfaceConfig` from `verse.rs` in `android.rs`.
2. Replace the dead bool with `serde::de::IgnoredAny`.
3. Verify: `#[allow]` count drops by 2; Android host tests pass.

### MOB-19 Stale comments: 'four surfaces' that lists six, and 'Chat / Code switch' after the switch became Coder / Verse

Severity: low · Category: docs · Effort: S

Locations:
- [lib.rs](../../../../crates/openagents-mobile/src/lib.rs) (`lib.rs:5-16`)
- [app.rs](../../../../crates/openagents-mobile/src/app.rs) (`app.rs:65`)
- [MobileBridge.swift](../../../../bins/openagents-ios/host/App/MobileBridge.swift) (`MobileBridge.swift:152`)
- [MobileBridge.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/MobileBridge.kt) (`MobileBridge.kt:176`)
- [shell.rs](../../../../crates/openagents-chat-app/src/coder_tab/shell.rs) (`shell.rs:52`)

Evidence: `lib.rs:5` says "The app has four surfaces" then describes Computers, Chat, Chats, Tailnet, Verse and Wallet. Three comments say "Chat / Code switch", while `shell.rs:52` says "The **Coder** / **Verse** switch".

Impact: Comments point readers at a UI concept the owner has ruled out.

Suggested action:
1. Rewrite the `lib.rs` header to match the current surfaces.
2. Replace the three "Chat / Code" comments with "Coder / Verse".
3. Verify: `rg 'Chat / Code' crates bins` returns nothing.

### MOB-20 openagents-mockup-ios is an unmaintained design fork

Severity: low · Category: dead-code · Effort: S

Locations:
- [bins/openagents-mockup-ios](../../../../bins/openagents-mockup-ios)

Evidence: Last commit 2026-10-07. It shares no code with the app and depicts a Gym-centred spec that is now behind the preview gate.

Impact: Drift, and one more project to keep building.

Suggested action:
1. Delete the directory; keep its screenshots in `docs/product`.
2. Record the decision in `docs/mobile/1.0-audit.md`.
3. Verify: `rg openagents-mockup-ios` finds only the archive note.

### MOB-21 The top-level swift/ directory holds only the unrelated lev-bridge macOS tool

Severity: low · Category: repo-hygiene · Effort: S

Locations:
- [Package.swift](../../../../swift/lev-bridge/Package.swift)
- [build-lev-bridge.sh](../../../../scripts/build-lev-bridge.sh)

Evidence: `swift/` contains only lev-bridge (4 tracked files).

Impact: The directory name is misleading. Low value.

Suggested action:
1. Move it to `tools/lev-bridge`.
2. Update the build script and `.gitignore`.
3. Verify: `scripts/build-lev-bridge.sh` still builds.

### MOB-22 The Launch struct mixes release config with debug-only fixture knobs

Severity: low · Category: maintainability · Effort: S

Locations:
- [app.rs](../../../../crates/openagents-mobile/src/app.rs) (`app.rs:47-163`)

Evidence: Several "Honored only in debug builds" fields are each gated ad hoc.

Impact: A missed gate lets a test override take effect in release builds.

Suggested action:
1. Move them into a `DebugLaunch` struct with a single `cfg`-gated accessor.
2. Verify: a release-mode test shows the accessor returns defaults.

### MOB-23 Small helpers duplicated: now(), hex(), QR-module rendering

Severity: low · Category: duplication · Effort: S

Locations:
- [wallet.rs](../../../../crates/openagents-mobile/src/wallet.rs) (`wallet.rs:250`)
- [app.rs](../../../../crates/openagents-mobile/src/app.rs) (`app.rs:2516`)
- [payees.rs](../../../../crates/openagents-mobile/src/payees.rs) (`payees.rs:387`)

Evidence: `wallet.rs:250` defines its own `now()`, and `app.rs:2516` has another.

Impact: Minor drift.

Suggested action:
1. Add `util.rs` with `unix_now`, `hex` and `qr_modules`, and replace the copies.
2. Verify: `rg 'fn now\(' crates/openagents-mobile/src` returns one definition.

### MOB-24 Host god files and silent no-op keystore failures in MobileBridge.kt

Severity: low · Category: maintainability · Effort: L

Locations:
- [MainActivity.kt](../../../../bins/openagents-android/host/app/src/main/java/com/openagents/app/MainActivity.kt)
- [NativeTranscriptPainter.swift](../../../../bins/coder-ios/host/App/NativeTranscriptPainter.swift)

Evidence: `MainActivity.kt` is 1,236 lines in OpenAgents and 619 in Coder. `NativeTranscriptPainter.swift` is 2,302 lines.

Impact: Hard to review, and silent no-op buttons go against the owner's "no dead controls" rule.

Suggested action:
1. Split the host files by tab.
2. Add a `withWorldKey` helper in `MobileBridge.kt` that sets a failure message when the keystore read fails.
3. Verify: with the keystore unavailable, tapping a world-key action shows an error instead of doing nothing.
