# Coder environment & infra crates

**Scope [CE]:** `crates/{coder-cloud, coder-project, coder-boundary, coder-history, coder-connect, coder-working-computer, coder-environment, coder-environment-build, coder-environment-operator, coder-environment-setup, coder-environment-verify, coder-hands, coder-hands-measure, coder-reach, coder-control, coder-ssh, coder-sync, coder-link}`
**Snapshot:** commit `3168c986aa11e18a8bd30f52c270f609e49b3815`, audit date 2026-10-10.
**Health grade: C**

This area has 18 crates, about 103k lines of Rust in 220 files, and 745 tests. Most individual modules are well built. Failures are typed (`StoreError`, `CallError` definite/unknown, `Refusal`), unsafe code is rare and mostly documented, production code has only 3 `unreachable!` calls and no TODO/FIXME, private files are opened owner-only without following symlinks, and evidence is redacted before it is truncated. The main problem is growth by copying and adapting code. There are five near-identical leased JSON record stores (plus a sixth in coder-cloud), and the three newest (setup, build, verify) have lost the symlink and identity hardening that the original has. Boat and GCE each have their own provider stack with a separate gcloud runner and separate label constants. Hand-written `openat`/`O_NOFOLLOW` private-file code repeats across crates, and the devin and opencode history mirrors are near-identical twins. Some security-relevant logic lives in Python and shell embedded in Rust strings, and the tar-restore symlink check there lets a `..` symlink escape the workspace. Two live-path risks stand out: GCE pool provisioning makes blocking gcloud/ssh calls with no timeout, and pool SSH uploads credentials with host-key checking turned off. None of the environment, working-computer, sync, or control invariants appear in `INVARIANTS.md`.

## Measurements

| Metric | Value |
|---|---|
| Crates / `.rs` files / LOC | 18 / 220 / 103,482 (git ls-files) |
| Tests (`#[test]`/`#[tokio::test]`) | 745 total; 9 `#[ignore]` |
| `unwrap`/`expect` lines | 813 overall; ~120 in production code (transition.rs 26, pub `fake` modules 60) |
| `panic!` / production `unreachable!` | 94 (almost all in tests) / 3 |
| Unsafe lines | 106 (mostly in coder-boundary `windows/` and `snapshot/observe.rs`); ~18 blocks with no SAFETY comment |
| `#[allow]` / `std::thread::sleep` / `Command::new` / `block_on` | 24 / 43 / 96 / 16 (11 in coder-sync) |
| TODO/FIXME | 0 |
| `pub` items | 2,148 |
| Duplicated leased record stores | 5 (191-305 lines each) + 1 in `coder-cloud/src/lib.rs` |
| Duplicated shell-quote helpers | 5 |
| Crates with README | 7 of 18 |
| `INVARIANTS.md` references | 0 each for coder-environment*, coder-working-computer, coder-reach, coder-ssh, coder-sync, coder-control, coder-link, coder-project |
| Crates with no in-repo Rust dependents | coder-control (binary used by coder-host tests), coder-hands-measure (standalone tool, 1 commit) |

Per-crate LOC / tests / tests per KLOC:

| Crate | LOC | Tests | /KLOC |
|---|---:|---:|---:|
| coder-cloud | 11,429 | 69 | 6.0 |
| coder-project | 8,914 | 71 | 8.0 |
| coder-boundary | 8,374 | 108 | 12.9 |
| coder-history | 8,256 | 61 | 7.4 |
| coder-connect | 8,042 | 52 | 6.5 |
| coder-working-computer | 7,639 | 29 | 3.8 |
| coder-environment | 6,887 | 31 | 4.5 |
| coder-environment-operator | 5,903 | 22 | 3.7 |
| coder-hands | 5,385 | 90 | 16.7 |
| coder-reach | 5,305 | 50 | 9.4 |
| coder-environment-setup | 4,985 | 22 | 4.4 |
| coder-control | 4,057 | 16 | 3.9 |
| coder-environment-verify | 3,580 | 15 | 4.2 |
| coder-ssh | 3,459 | 33 | 9.5 |
| coder-sync | 3,149 | 18 | 5.7 |
| coder-environment-build | 3,003 | 22 | 7.3 |
| coder-hands-measure | 2,980 | 24 | 8.1 |
| coder-link | 2,135 | 12 | 5.6 |

Largest files: `coder-working-computer/src/gce.rs` 1,991; `coder-cloud/src/operator.rs` 1,841; `coder-history/src/host/tests.rs` 1,620; `coder-environment-verify/src/service.rs` 1,573; `coder-history/src/host/catalog.rs` 1,446; `coder-project/src/controller.rs` 1,360.

Longest functions: working-computer `transition::apply` 540 lines; coder-cloud operator `effect` 289 and `execute` 235; coder-project `observe::capture` 268; `discovery::scan` 264; setup `transition::apply` 248; verify `service::start` 233.

## Strengths

- Failure modes are typed and classified carefully in the newer crates. `CallError {definite, message}` separates GCE refusals from transport loss ([gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:473-495). `StoreError` has Fence/Immutable/Busy variants for revision-fenced, history-preserving commits ([store.rs](../../../../crates/coder-working-computer/src/store.rs) store.rs:16-56).
- Where owner-only, symlink-refusing file access exists, it is done properly: an `openat` walk with `O_NOFOLLOW|O_DIRECTORY`, uid/mode/nlink checks, and before/after identity stamps to catch races ([operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:213-312, [observe.rs](../../../../crates/coder-project/src/observe.rs) observe.rs:80-122).
- Unsafe code is small and mostly documented. `coder-boundary/src/windows/*` (37 blocks), `coder-history/src/host/confined.rs` (15), `coder-connect/src/store.rs` and coder-ssh all carry SAFETY comments.
- Production code has very few panics: 3 `unreachable!` and 0 TODO/FIXME. Every crate inherits the clippy `todo`/`unimplemented`/`dbg_macro` denials through `[lints] workspace = true`.
- Credentials are handled deliberately. `Credentials` derives no `Debug` (coder-cloud/src/runtime.rs:7-11). Released BYO credentials stay in memory, are scoped per turn and checked per device (coder-cloud/src/release.rs). The GitHub token goes only to api.github.com and is redacted in `Debug` (coder-environment-operator/src/studio/github.rs:72-110). askpass zeroes its buffers (coder-ssh/src/askpass.rs:58).
- Designs are deterministic and injectable. coder-link is a pure state machine with `Clock`/`Connector` traits, and the provider adapters have in-memory fakes, so tests run the real command wrapper end to end.
- Docs are current. All 11 `docs/` paths cited from source exist, module docs state the design invariants (gce.rs:1-43, boat.rs:1-31, release.rs:1-23), and the coder-connect, coder-reach and coder-link READMEs describe scope and boundaries.
- No crate in scope declares an unused `[dependencies]` entry, and every crate opts into the workspace lints.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| CE-01 | medium | duplication | Five copy-pasted leased JSON stores; the newer copies lost the symlink and identity checks | M |
| CE-02 | medium | security | Pool SSH turns off host-key verification on the channel that uploads credentials | M |
| CE-03 | medium | concurrency | Blocking gcloud/ssh calls with no timeout inside async provision | M |
| CE-04 | medium | security | Embedded Python tar restore lets a `..` symlink escape the workspace | S |
| CE-05 | medium | architecture | Separate Boat and GCE provider stacks with duplicated gcloud runners and labels | L |
| CE-06 | medium | architecture | coder-working-computer depends on all of coder-cloud for a handful of helpers | M |
| CE-07 | medium | duplication | Hand-written `openat`/`O_NOFOLLOW`/`geteuid` code repeated; private-fs covers Windows only | M |
| CE-08 | medium | duplication | devin.rs and opencode.rs are ~650-line twins | M |
| CE-09 | medium | error-handling | String-typed errors; exact-message matching drives control flow | M |
| CE-10 | medium | maintainability | 540-line `apply()` state machine with `unwrap`/`unreachable!` | M |
| CE-11 | medium | policy | Environment, computer, pool, sync and control invariants missing from INVARIANTS.md | S |
| CE-12 | medium | error-handling | coder-sync silently resets corrupt settings; temp file name is per-pid only | S |
| CE-13 | low | maintainability | Unsafe blocks without SAFETY comments; no lint enforces them | S |
| CE-14 | low | build | Test fakes that run local `sh` are compiled into production builds | S |
| CE-15 | low | error-handling | Results discarded in background loops | S |
| CE-16 | low | security | Released-credential state kept in globals and cleared only on normal return | S |
| CE-17 | low | concurrency | Re-locking in the setup service can panic (TOCTOU) | S |
| CE-18 | low | maintainability | Security-relevant shell/Python embedded as unformatted string literals | M |
| CE-19 | low | duplication | Five shell-quote helpers with different semantics | S |
| CE-20 | low | security | Pool SSH key dir created with default umask; hard-coded GCP project default | S |
| CE-21 | low | build | Unused openagents-web dev-dependencies on four in-scope crates | S |
| CE-22 | low | maintainability | Very large files and functions in operator and adapters | M |
| CE-23 | low | docs | Poor naming and discoverability: `Kind2`, overlapping connectivity crates, missing READMEs | S |
| CE-24 | low | duplication | Small helpers duplicated: clock, sha256-hex, price table | S |

### CE-01 Five copy-pasted leased JSON record stores, and the newer copies have dropped the symlink and identity checks

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder-environment/src/store.rs](../../../../crates/coder-environment/src/store.rs) store.rs:197, store.rs:222, store.rs:243
- [coder-environment-setup/src/store.rs](../../../../crates/coder-environment-setup/src/store.rs) store.rs:174, store.rs:191
- [coder-environment-build/src/store.rs](../../../../crates/coder-environment-build/src/store.rs) store.rs:158
- [coder-environment-verify/src/store.rs](../../../../crates/coder-environment-verify/src/store.rs) store.rs:165
- [coder-cloud/src/lib.rs](../../../../crates/coder-cloud/src/lib.rs) lib.rs:206

**Evidence:** `coder-environment/src/store.rs` calls `regular_or_missing` on both the record and its `.writing` temp file (203-205), opens through `symlink_metadata` + `is_file` (222-223), and refuses with "Environment identity mismatch." (243). The setup, build and verify stores create the temp file with `with_extension("writing")` and no symlink check (setup 174, build 158, verify 165), and read with a plain `File::open(path)` (setup 191, build 175, verify 182). None of the three contains `regular_or_missing`, `protect_dir` or an identity check. `coder-cloud/src/lib.rs` has a sixth `Store`/`Lease` that uses `String` errors.

**Impact:** Hardening added to one store is missing from three others, and every later fix has to be made in 5-6 places. Exploiting the gap needs write access inside the user's own 0700 store directory (setup store.rs:107-110 creates it with `DirBuilder` mode `0o700`). This is therefore drift and duplication, not a live vulnerability. The verifier lowered the severity from high to medium for this reason.

**Suggested action:**
1. Add a generic `coder_environment::store::RecordStore<T: Record>` that contains `lease`, `write_atomic`, `read_record`, `protect_dir` and `regular_or_missing` once, with the full hardening.
2. Reduce the setup, build and verify `store.rs` files to thin type aliases over it. Later, move the coder-cloud `Store`/`Lease` onto it as well.
3. Write one shared test suite covering a symlinked record, a symlinked `.writing` temp, an id/file-name mismatch, and directory mode 0700. Instantiate it for every record type.
4. Verify: the suite passes for all record types, and `grep -n 'File::open(path)' crates/coder-environment-*/src/store.rs` returns nothing.

### CE-02 Pool SSH disables host-key verification on the channel used to upload credentials

**Severity:** medium · **Category:** security · **Effort:** M

**Locations:**
- [coder-cloud/src/pool.rs](../../../../crates/coder-cloud/src/pool.rs) pool.rs:337
- [coder-cloud/src/gce_backend.rs](../../../../crates/coder-cloud/src/gce_backend.rs) gce_backend.rs:51
- [coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:637

**Evidence:** `pool::ssh` passes `StrictHostKeyChecking=no` and `UserKnownHostsFile=/dev/null` (pool.rs:340-344). When `OA_POOL_SSH=internal` is set, it connects directly to `host.address` instead of going through the IAP ProxyCommand (pool.rs:357-359). The credential upload in coder-working-computer (gce.rs:637) reuses `coder_cloud::pool::ssh`.

**Impact:** In internal mode, any machine that answers on the VPC address receives the uploaded credentials. The default IAP path binds the tunnel to the named instance through Google identity, which limits the risk, but the host key is never pinned. The verifier lowered the severity from high to medium because the unauthenticated direct path is opt-in.

**Suggested action:**
1. After instance creation, read the host keys from guest attributes (`gcloud compute instances get-guest-attributes NAME --query-path=hostkeys/`) and write them to a per-host known_hosts file under the 0700 cloud home.
2. Pass `-o StrictHostKeyChecking=yes -o UserKnownHostsFile=<file>`.
3. In internal mode, fail closed when no key is pinned.
4. Add a unit test that checks the ssh `Command` args, and add an INVARIANTS.md row (see CE-11).
5. Verify: the test fails if `StrictHostKeyChecking=no` comes back, and a provision run against a host with a mismatched key is refused.

### CE-03 Synchronous gcloud and ssh calls with no timeout inside async provision (up to ~18 minutes of blocking, unbounded on a hung gcloud)

**Severity:** medium · **Category:** concurrency · **Effort:** M

**Locations:**
- [coder-cloud/src/pool.rs](../../../../crates/coder-cloud/src/pool.rs) pool.rs:184, pool.rs:739, pool.rs:745
- [coder-cloud/src/gce_backend.rs](../../../../crates/coder-cloud/src/gce_backend.rs) gce_backend.rs:13, gce_backend.rs:144, gce_backend.rs:173
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:504

**Evidence:** `pool::gcloud` calls std `Command::output()` with no timeout (184-198). `wait_ready` and `wait_ssh` loop on `std::thread::sleep` (739, 750). The `Transport` trait's `granted`/`hosts`/`start` are synchronous (gce_backend.rs:13-15) and are called from `async fn provision` (144, 160, 173). operator.rs:504-508 runs `drive` on a new current_thread runtime for each call, using `block_on`.

**Impact:** Each drive has its own current_thread runtime, so no shared executor is starved. The job's cancellation and pause polling is still frozen for the whole host boot, and a hung gcloud (for example one waiting on a reauth prompt) blocks the job thread forever. The verifier lowered the severity from high to medium for this reason.

**Suggested action:**
1. Make `Transport::granted`/`hosts`/`start` async.
2. Port `pool::gcloud` and `ssh_output` to `tokio::process` with `kill_on_drop(true)` and `tokio::time::timeout`, following the `Gcloud::call` pattern at coder-working-computer/src/gce.rs:473.
3. In `wait_ready`/`wait_ssh`, replace `thread::sleep` with `tokio::time::sleep`.
4. Verify: add a test with a fake `Transport` whose `start()` never returns, and assert that cancellation ends `provision`.

### CE-04 Embedded Python tar restore lets a `..` symlink escape the workspace, and the rule is untested

**Severity:** medium · **Category:** security · **Effort:** S

**Locations:**
- [coder-cloud/src/workspace.rs](../../../../crates/coder-cloud/src/workspace.rs) workspace.rs:330
- [coder-cloud/tests/workspace.rs](../../../../crates/coder-cloud/tests/workspace.rs) workspace.rs:155

**Evidence:** workspace.rs:330 checks `if m.linkname.startswith('/') or posixpath.normpath(posixpath.join(posixpath.dirname(m.name),m.linkname)).startswith('../')`. For a member `a` with linkname `..`, `normpath('..') == '..'`. That string does not start with `'../'`, so the check passes and line 331 runs `q.symlink_to('..')`.

**Impact:** A crafted repository archive can plant a symlink from the workspace to its parent job directory on the remote host, which gets around the "Unsafe workspace symlink" guard.

**Suggested action:**
1. Change the check to `n = normpath(...); if n == '..' or n.startswith('../') or linkname.startswith('/')`. Alternatively, resolve the target against the workspace root and require containment with `os.path.commonpath`.
2. Add a test in `tests/workspace.rs` with members `a -> '..'`, `a/b -> '../..'` and `c -> '/etc'`, each expected to raise `SystemExit('Unsafe workspace symlink')`.
3. Verify: the new test fails on the current code and passes after the fix.

### CE-05 Two parallel Boat and GCE provider stacks with duplicated gcloud runners and label constants

**Severity:** medium · **Category:** architecture · **Effort:** L

**Locations:**
- [coder-cloud/src/pool.rs](../../../../crates/coder-cloud/src/pool.rs) pool.rs:184, pool.rs:23
- [coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:62, gce.rs:473

**Evidence:** pool.rs:184-198 is a synchronous gcloud runner with no timeout that keeps the last 4 stderr lines. gce.rs:473 is a separate async runner with a timeout. The label strings are defined twice, in pool.rs:23 and gce.rs:62-64.

**Impact:** Fixes reach only one stack, as the timeout gap in CE-03 already shows, and the label-isolation constants can drift apart.

**Suggested action:**
1. Move one async `Gcloud::call(args, timeout)`, the `not_found`/`definite` classifiers and the label constants into a shared leaf module or crate (e.g. `coder-gce`).
2. Use it from both pool.rs and gce.rs, and delete `pool::gcloud`.
3. Verify: `rg 'fn gcloud|Command::new\("gcloud"\)' crates/coder-cloud crates/coder-working-computer` finds exactly one definition, and each label constant is defined once.

### CE-06 coder-working-computer pulls in all of coder-cloud for Credentials and three pool helpers

**Severity:** medium · **Category:** architecture · **Effort:** M

**Locations:**
- [coder-working-computer/Cargo.toml](../../../../crates/coder-working-computer/Cargo.toml) Cargo.toml:12
- [coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:54, gce.rs:470, gce.rs:627, gce.rs:637
- [coder-working-computer/src/boat.rs](../../../../crates/coder-working-computer/src/boat.rs) boat.rs:41

**Evidence:** Apart from doc mentions, coder-working-computer/src uses coder_cloud only for `runtime::Credentials` (gce.rs:54, boat.rs:41) and `pool::{ensure_key, Host, ssh}` (gce.rs:470, 627, 637).

**Impact:** The dependency points the wrong way between layers, and every environment crate takes longer to build.

**Suggested action:**
1. Move `Credentials` and the pool ssh/key helpers into a leaf crate (this can be the same crate as CE-05), and re-export them from coder-cloud so existing callers keep working.
2. Remove `coder-cloud` from coder-working-computer/Cargo.toml.
3. Verify: `cargo tree -p coder-working-computer | grep coder-cloud` returns nothing, and the crate's tests pass.

### CE-07 Hand-written openat/O_NOFOLLOW/geteuid private-file code repeated across crates, while private-fs covers only Windows

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:252
- [coder-project/src/observe.rs](../../../../crates/coder-project/src/observe.rs) observe.rs:80
- [coder-connect/src/store.rs](../../../../crates/coder-connect/src/store.rs) store.rs:41
- [private-fs/Cargo.toml](../../../../crates/private-fs/Cargo.toml)

**Evidence:** operator.rs and observe.rs each implement their own `openat` walk, and `geteuid` unsafe calls are repeated. The reviewer counted at least 6 crates in scope (about 80 files repo-wide). The verifier did not re-measure the full count but found the pattern consistent with the code it checked.

**Impact:** Security-critical traversal logic is rewritten in several places with slightly different rules.

**Suggested action:**
1. Add a unix implementation to private-fs (`open_private`, `is_private`, `euid`) that carries the full rule set from operator.rs:213-312 (O_NOFOLLOW walk, uid/mode/nlink checks, before/after identity stamps).
2. Migrate operator.rs and observe.rs first, then coder-connect/src/store.rs and the other crates in scope.
3. Verify: one private-fs test suite covers symlink, wrong-owner, wrong-mode and hard-link cases, and `rg 'libc::geteuid' crates/coder-*` shrinks to zero.

### CE-08 coder-history devin.rs and opencode.rs are ~650-line twins

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder-history/src/devin.rs](../../../../crates/coder-history/src/devin.rs) devin.rs:636
- [coder-history/src/opencode.rs](../../../../crates/coder-history/src/opencode.rs) opencode.rs:636

**Evidence:** devin.rs has 659 lines and opencode.rs has 658. Both define the same set of helpers at matching line offsets.

**Impact:** Every mirror fix has to be made twice.

**Suggested action:**
1. Extract a `coder_history::mirror` module with an `Engine` trait that captures the per-engine differences.
2. Reduce devin.rs and opencode.rs to `Engine` impls.
3. Verify: the existing history tests pass unchanged, and the two files together come to well under the current ~1,300 lines.

### CE-09 String-typed errors and exact-message matching drive control flow

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [coder-cloud/src/lib.rs](../../../../crates/coder-cloud/src/lib.rs) lib.rs:24, lib.rs:237
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:485
- [coder-control/src/lib.rs](../../../../crates/coder-control/src/lib.rs) lib.rs:22

**Evidence:** operator.rs:485 checks `error == "Another process is using this remote job."`, and that string literal is produced at lib.rs:237. coder-cloud and coder-control both define `Result<T> = Result<T, String>`.

**Impact:** Rewording the message silently turns off the lease retry.

**Suggested action:**
1. Add a `CloudError` enum with a `Busy` variant returned by `Store::lease`, and match on the variant in operator.rs.
2. Convert the rest of coder-cloud's `Result<T, String>` to the new enum step by step, keeping `Display` text for user-facing messages.
3. Verify: add a test that takes the lease twice and checks that the retry path runs, with no string comparison anywhere in operator.rs.

### CE-10 540-line state-machine apply() with unchecked unwrap/expect

**Severity:** medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [coder-working-computer/src/transition.rs](../../../../crates/coder-working-computer/src/transition.rs) transition.rs:241
- [coder-working-computer/src/decide.rs](../../../../crates/coder-working-computer/src/decide.rs) decide.rs:131
- [coder-environment-setup/src/transition.rs](../../../../crates/coder-environment-setup/src/transition.rs) transition.rs:299

**Evidence:** `apply` starts at line 241 and its closing brace is about 540 lines later. transition.rs holds 26 production unwrap/expect lines. decide.rs:131 has `unreachable!("only restored boots restore")`, and setup transition.rs:299 has a bare `_ => unreachable!()`.

**Impact:** If someone reorders a precondition check, a case that should be a refusal becomes a panic.

**Suggested action:**
1. Split `apply()` into one function per `Command`.
2. Replace `unwrap`/`expect`/`unreachable!` with accessors that return `Refusal`.
3. Add a proptest that generates arbitrary command sequences and asserts that `apply` never panics. Do the same for the setup transition.
4. Verify: the proptest passes, and `rg 'unwrap\(\)|expect\(|unreachable!' crates/coder-working-computer/src/transition.rs` returns only test code.

### CE-11 Environment, computer, pool, sync and control invariants missing from INVARIANTS.md

**Severity:** medium · **Category:** policy · **Effort:** S

**Locations:**
- [INVARIANTS.md](../../../../INVARIANTS.md)
- [coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:1
- [coder-cloud/src/release.rs](../../../../crates/coder-cloud/src/release.rs) release.rs:1

**Evidence:** `grep -c` for `coder-working-computer|coder-environment|coder-sync|coder-ssh` in INVARIANTS.md returns 0. The guarantees are written down only in module doc comments (gce.rs:1-43, release.rs:1-23).

**Impact:** These guarantees can be relaxed without the INVARIANTS.md review that workspace policy requires.

**Suggested action:**
1. Add INVARIANTS.md rows for: GCE instances created with no service account, credentials kept on tmpfs and passed only over stdin, commands applied at most once, BYO credentials released per turn, and sync uploads screened. Each row must name its enforcing test.
2. Add the pinned host key from CE-02 when that lands.
3. Verify: every new row points to a test that exists and passes.

### CE-12 coder-sync silently resets corrupt settings and uses a per-pid temp name

**Severity:** medium · **Category:** error-handling · **Effort:** S

**Locations:**
- [coder-sync/src/lib.rs](../../../../crates/coder-sync/src/lib.rs) lib.rs:95, lib.rs:124

**Evidence:** `Settings::load` uses `.ok().and_then(from_str.ok()).unwrap_or_default()` (95-100), and its doc comment says an unreadable file means sync is off. The temp file is named `.{FILE}.{pid}.tmp` (124). coder-sync also has 11 `block_on` calls.

**Impact:** A corrupt `sync.json` loses the `kept_here` list, and the next `store()` writes the empty list back. Two threads in the same process can collide on the temp file.

**Suggested action:**
1. Make `load` return an error, and move a corrupt file aside to `sync.json.corrupt` instead of overwriting it.
2. Add a per-call suffix (counter or random) to the temp file name.
3. Share one lazily built runtime and client across the blocking helpers.
4. Verify: add tests where a corrupt file keeps `kept_here` recoverable in `.corrupt`, and where concurrent `store()` calls from two threads both succeed.

### CE-13 Unsafe blocks without SAFETY comments in boundary snapshot, and no lint to enforce them

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:**
- [coder-boundary/src/snapshot/observe.rs](../../../../crates/coder-boundary/src/snapshot/observe.rs) observe.rs:677
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:204
- [Cargo.toml](../../../../Cargo.toml) Cargo.toml:37

**Evidence:** snapshot/observe.rs has 14 lines containing `unsafe` and only 1 SAFETY comment near them. `[workspace.lints.clippy]` denies only `dbg_macro`/`todo`/`unimplemented` (Cargo.toml:37-40). About 18 unsafe blocks in scope have no SAFETY comment.

**Impact:** Later edits can easily break the fd-ownership rules in the boundary walker. The verifier found no soundness bug and lowered the severity to low.

**Suggested action:**
1. Add SAFETY comments to the observe.rs blocks and to operator.rs:204.
2. Set `clippy::undocumented_unsafe_blocks = "warn"` for these crates, or for the whole workspace.
3. Verify: `cargo clippy -p coder-boundary -p coder-cloud` reports no `undocumented_unsafe_blocks` warnings.

### CE-14 Test fakes that run local `sh` are compiled into production builds

**Severity:** low · **Category:** build · **Effort:** S

**Locations:**
- [coder-working-computer/src/provider.rs](../../../../crates/coder-working-computer/src/provider.rs) provider.rs:394
- [coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:1606
- [coder-working-computer/Cargo.toml](../../../../crates/coder-working-computer/Cargo.toml)

**Evidence:** `pub mod fake` at provider.rs:394 and gce.rs:1606 has no `cfg` gate, and Cargo.toml has no `[features]` section. These fake modules account for about 60 of the ~120 production unwrap/expect lines.

**Impact:** Test-only code ships in production builds and enlarges the public API. No runtime path reaches the fakes, so the verifier lowered the severity to low.

**Suggested action:**
1. Add a `test-support` feature.
2. Gate both modules with `#[cfg(any(test, feature = "test-support"))]`.
3. Enable the feature in consumers' `[dev-dependencies]`.
4. Verify: `cargo build -p coder-working-computer --release` succeeds without the fakes, and the consumer tests still build.

### CE-15 Discarded results in background loops hide failures

**Severity:** low · **Category:** error-handling · **Effort:** S

**Locations:**
- [coder-environment-operator/src/lib.rs](../../../../crates/coder-environment-operator/src/lib.rs) lib.rs:489
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:517

**Evidence:** lib.rs:489 has `let _ = owners.setup.resume(&id, now_ms()).await;`. operator.rs:517 has `let _ = lease.save(record);` right after the unresolved error is set.

**Impact:** Failed resumes and failed saves leave no trace.

**Suggested action:**
1. Log the error or record it through the activity log.
2. When the save in operator.rs fails, return a combined error.
3. Verify: a test with a failing fake store shows the error in the returned value or in the activity log.

### CE-16 Released-credential state in process-global statics, cleared only on normal return

**Severity:** low · **Category:** security · **Effort:** S

**Locations:**
- [coder-cloud/src/release.rs](../../../../crates/coder-cloud/src/release.rs) release.rs:44
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:457

**Evidence:** `OFFERS` and `TURNS` are process-global statics (release.rs:44-45). `disarm` runs only after `drive` returns (operator.rs:457-459), so a panic skips it.

**Impact:** If `drive` panics, the credentials stay armed beyond the single turn they were released for.

**Suggested action:**
1. Replace the explicit `disarm` call with an RAII guard that disarms on `Drop`.
2. Give each `TURNS` entry an expiry.
3. Verify: add a test where `drive` panics under `catch_unwind` and assert that nothing is still armed afterwards.

### CE-17 Lock re-locking in setup service can panic

**Severity:** low · **Category:** concurrency · **Effort:** S

**Locations:**
- [coder-environment-setup/src/service.rs](../../../../crates/coder-environment-setup/src/service.rs) service.rs:986, service.rs:988

**Evidence:** The match guard takes the lock to check `contains_key`, then `lengths()` runs, then the code takes the lock again and indexes `[&s.id]` (988). That index panics if the recorder was removed in between.

**Impact:** This is a TOCTOU panic, and the poisoned lock then makes later calls fail as well.

**Suggested action:**
1. Take the lock once, clone the dir out of the guard, and drop the guard before calling `lengths()`.
2. Use `.get()` instead of indexing, and lock in a poison-tolerant way (`unwrap_or_else(PoisonError::into_inner)`).
3. Verify: add a test that removes the recorder concurrently and asserts there is no panic.

### CE-18 Security-relevant shell and Python embedded as unformatted string literals

**Severity:** low · **Category:** maintainability · **Effort:** M

**Locations:**
- [coder-cloud/src/workspace.rs](../../../../crates/coder-cloud/src/workspace.rs) workspace.rs:330, workspace.rs:333

**Evidence:** The Python is written as one-liners, for example `def git(*args,**kw):return subprocess.run(...)` at workspace.rs:333. The tar symlink check from CE-04 is embedded the same way.

**Impact:** No linter or direct test covers these scripts. CE-04 is a bug that slipped through as a result.

**Suggested action:**
1. Move each script to its own `.py`/`.sh` file loaded with `include_str!`.
2. Run ruff/shellcheck on those files from the check scripts.
3. Verify: the check scripts flag a deliberately introduced lint error in the moved files.

### CE-19 Five shell-quote helpers with different semantics

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:**
- [coder-ssh/src/ssh.rs](../../../../crates/coder-ssh/src/ssh.rs) ssh.rs:317
- [coder-environment-verify/src/plan.rs](../../../../crates/coder-environment-verify/src/plan.rs) plan.rs:434
- [coder-environment-setup/src/source.rs](../../../../crates/coder-environment-setup/src/source.rs) source.rs:239
- [coder-environment-build/src/sanitize.rs](../../../../crates/coder-environment-build/src/sanitize.rs) sanitize.rs:74
- [boat/src/follow.rs](../../../../crates/boat/src/follow.rs) follow.rs:64

**Evidence:** Four of the helpers use the strict single-quote form. `boat::shell_quote` also rewrites a leading `~/` to `"$HOME"/`.

**Impact:** The same path can be quoted two different ways depending on which helper is called.

**Suggested action:**
1. Put a strict `quote` and a home-expanding `quote_home` in one module.
2. Point all five call sites at it.
3. Verify: one table test covers quotes, spaces, `~/` and empty strings, and `rg 'fn shell_quote|fn quote' crates/coder-* crates/boat` finds only the shared module.

### CE-20 Pool SSH key directory created with default umask; hard-coded GCP project default

**Severity:** low · **Category:** security · **Effort:** S

**Locations:**
- [coder-cloud/src/pool.rs](../../../../crates/coder-cloud/src/pool.rs) pool.rs:33, pool.rs:308

**Evidence:** pool.rs:308 uses `create_dir_all(dir)`. `project()` falls back to `"openagentsgemini"` (33-38).

**Impact:** The key directory's mode depends on the process umask, and the company GCP project is used silently when nothing is configured.

**Suggested action:**
1. Create the directory with `DirBuilder::new().recursive(true).mode(0o700)`, and check the mode of a directory that already exists.
2. Require the project to be configured explicitly, with a clear error when it is not.
3. Verify: a test under umask 022 asserts the directory mode is 0700, and a test with no project configured asserts the error.

### CE-21 Unused dev-dependencies in openagents-web on four scope crates

**Severity:** low · **Category:** build · **Effort:** S

**Locations:**
- [openagents-web/Cargo.toml](../../../../crates/openagents-web/Cargo.toml) Cargo.toml:58, Cargo.toml:60, Cargo.toml:61, Cargo.toml:68

**Evidence:** `rg 'coder_project|coder_environment_build|coder_environment_setup|coder_environment_verify' crates/openagents-web` returns 0 matches.

**Impact:** Test builds compile large dependency trees that nothing uses.

**Suggested action:**
1. Delete the four lines.
2. Verify: `cargo test -p openagents-web --no-run` still builds.

### CE-22 God files and functions in operator and adapters

**Severity:** low · **Category:** maintainability · **Effort:** M

**Locations:**
- [coder-working-computer/src/gce.rs](../../../../crates/coder-working-computer/src/gce.rs) gce.rs:1
- [coder-cloud/src/operator.rs](../../../../crates/coder-cloud/src/operator.rs) operator.rs:1304

**Evidence:** gce.rs is about 1,991 lines (it contains the fake module at 1606 and `cfg(test)` at 1989), and operator.rs is about 1,841 lines. The operator's `effect` function is 289 lines and `execute` is 235.

**Impact:** These files are hard to review.

**Suggested action:**
1. Split each file into submodules by concern (e.g. gce: runner, instance lifecycle, credentials upload, fake, tests; operator: private-file access, lease/retry, effect, execute). Make each split a pure move.
2. Verify: the diff contains moves only (`git diff --color-moved`), and the tests pass unchanged.

### CE-23 Naming and discoverability: Kind2, overlapping connectivity crates, missing READMEs

**Severity:** low · **Category:** docs · **Effort:** S

**Locations:**
- [coder-environment-build/src/service.rs](../../../../crates/coder-environment-build/src/service.rs) service.rs:153

**Evidence:** `Kind2` is used 18 times. 11 of the 18 crates have no README. The connectivity crates (coder-connect, coder-reach, coder-ssh, coder-link) overlap in scope and have no index that says which one owns what.

**Impact:** Work gets routed to the wrong crate.

**Suggested action:**
1. Rename `Kind2` to `BuildStep`.
2. Add a short crate index (one line of ownership per crate) and READMEs for the 11 crates that lack one, starting with the connectivity crates.
3. Verify: `rg Kind2 crates/` returns nothing, and every crate in scope has a README.md.

### CE-24 Small helpers duplicated: clock, sha256-hex, price table

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:**
- [coder-cloud/src/lib.rs](../../../../crates/coder-cloud/src/lib.rs) lib.rs:186
- [coder-sync/src/claude_session.rs](../../../../crates/coder-sync/src/claude_session.rs) claude_session.rs:261

**Evidence:** Small helpers such as `now_ms` and sha256-hex, and a price table, are defined in more than one crate. The verifier did not re-check this exhaustively.

**Impact:** Small definitional drift between crates.

**Suggested action:**
1. Keep one `hex_sha256` and one `now_ms` in a shared leaf module.
2. Move the price table into a single place and record its source and date.
3. Verify: `rg 'fn now_ms|fn hex_sha256' crates/coder-*` finds one definition each.
