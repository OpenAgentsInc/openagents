# Psionic: backends, compiler, core, cluster

**Scope:** the compute substrate of the imported Psionic workspace: `crates/psionic/crates/{psionic-backend-cuda, psionic-backend-metal, psionic-backend-cpu, psionic-backend-tests, psionic-compiler, psionic-ir, psionic-core, psionic-array, psionic-nn, psionic-transformer, psionic-cluster, psionic-net, psionic-collectives, psionic-sandbox, psionic-adapters, psionic-observe}` plus `crates/psionic/{Cargo.toml, README.md, fixtures}`. Snapshot commit `3168c986aa`, audit date 2026-10-10. The uncommitted psionic-backend-cuda files belong to another agent, so all figures here use the HEAD versions.

**Health grade: C**

These 16 crates hold about 178k lines of Rust. They came in with the 2026-10-07 import (`737f94a17c`), and no commit has touched any of them since, so this is upstream code with no owner here yet. Line by line the code is careful. It has zero `.unwrap()` calls, errors are typed with thiserror, the workspace lints deny unwrap, expect and panic, and psionic-net signs its envelopes, keeps a replay window and redacts admission tokens in Debug output. The problems are structural:

- Four god files hold about 60k lines between them.
- The CUDA FFI has 198 `unsafe {}` blocks and no SAFETY comments.
- About 44k lines (a quarter of the area) are Tassadar research code. All of it is linked into the serving binary, because psionic-array depends on both GPU backends and psionic-transformer depends on psionic-array.

The most urgent issues:

- Nothing builds, tests or lints this workspace, and the deny-lints are already violated in code that is not test code.
- 87 of the 90 fixture paths the code references are missing, so many tests fail before anyone changes a line.
- The psionic-net transport and relay tasks die on any single send error.
- The relay accepts registrations that nobody has authenticated.
- The node's secret key is written as plaintext JSON with default file permissions.
- The sandbox runner can deadlock on its own output pipes.
- The 448 MB `target/` directory is not gitignored.

## Measurements

| Metric | Value |
|---|---|
| In-scope Rust LOC (git ls-files, HEAD) | ~178k across 16 crates |
| Per crate | compiler 26,599 (47 files); cuda 26,422 (2 files); ir 25,802 (22); cluster 24,339 (18); metal 16,177 (1); sandbox 15,027 (31); net 10,308 (2); transformer 6,315; array 6,230; nn 5,542; core 5,514; collectives 3,858; cpu 3,227; adapters 1,707; observe 529; backend-tests 469 |
| Largest files | cuda/lib.rs 26,043 at HEAD (26,301 in the dirty working copy); metal/lib.rs 16,177; net/lib.rs 10,029; ir/autodiff.rs 8,372; ir/lib.rs 8,141; cluster/ordered_state.rs 7,422; array/lib.rs 6,137; compiler/lib.rs 4,631 |
| CUDA C sources | quantized_matvec.cu 10,496 lines; the stub .c, kept in sync by hand, is 1,902 lines |
| Tassadar research code (`src/tassadar_*`, re-measured) | compiler 40 files / 21,671 LOC (82% of the crate); ir 19 / 9,275; sandbox 14 / 9,794; transformer 11 / 2,561; cluster 2 / 659. About 43.9k LOC in total |
| `#[test]` count | compiler 172, cluster 130, cuda 123 (92 are `_when_available` and pass vacuously with no GPU), ir 109, sandbox 63, metal 52, core 38, net 38, transformer 33, array 28, nn 26, cpu 20, collectives 19, adapters 9, observe 6, backend-tests 0 (harness only) |
| `.unwrap()` / TODO / FIXME | 0 / 0 / 0 |
| `.expect(` | 822 in total; about 95 sit outside `#[cfg(test)]` (structured_control 31, ir/lib 18, cuda 9) |
| `panic!` / `unreachable!` | cluster 92, net 62, mostly in tests |
| `#[allow]` | cuda 78, cluster 24, compiler 21 |
| `unsafe` | cuda: 198 blocks plus 55 unsafe fn and extern items, 0 SAFETY comments; metal: 10 |
| Glob re-exports (`pub use x::*`) | compiler 40, ir 20, sandbox 19, transformer 15, cluster 12 |
| Copy-pasted helpers (across `crates/psionic/crates`) | about 771 free-function copies of `stable_digest`; `fn repo_root(` 479; `fn read_json` 271 |
| Fixtures | 208 tracked files (4.6 MB), but the README says 23; 87 of the 90 referenced paths are missing |
| Cargo.lock | 683 packages, 60 with more than one version (metal 0.29/0.32/0.33; wasmparser 0.228/0.244/0.245; thiserror 1/2; rand 0.8/0.9) |
| Commits in scope since import | 0 |
| CI coverage | none (no `.github/workflows`, no hook) |
| `crates/psionic/target/` | 448 MB, not gitignored |

## Strengths

- No `.unwrap()` in any of the 16 crates. Every crate opts into `[lints] workspace = true`, which denies `unwrap_used`, `expect_used`, `panic`, `todo`, `dbg_macro`, `print_stdout` and `await_holding_lock` ([Cargo.toml](../../../../crates/psionic/Cargo.toml)).
- Errors are typed with thiserror enums nearly everywhere outside psionic-net and psionic-sandbox. Only 21 functions in scope return `Result<_, String>`.
- psionic-net authenticates its wire envelopes with ed25519 and a sliding replay window per peer (lib.rs:2353-2411, verified at 4485). It also redacts admission tokens in Debug output (lib.rs:144-146).
- The CUDA backend builds on machines without the CUDA toolkit. build.rs falls back to a C stub, and the runtime loads libcudart and libcublas through libloading, so Mac and CPU builds keep working. Readiness is reported explicitly (`CudaBackendState::Unavailable`).
- Metal and CUDA keep their FFI inside per-OS `mod platform` modules, with matching stubs for other targets, so cross-platform builds compile.
- The WASM structured-control parser returns typed errors for a malformed `end` and for frames left open (tassadar_structured_control.rs:1024-1103).
- One shared conformance harness, psionic-backend-tests, is a dev-dependency of the cpu, cuda and metal backends.
- The sandbox canonicalizes the workspace entrypoint and rejects paths that escape it (execution.rs:621-629). It also runs jobs with `env_clear()` and enforces timeouts and kill-after.
- In the CUDA backend, Drop impls clean up allocator pooling, cuBLASLt plan contexts and CUDA graph lifetimes (cuda lib.rs:9703-9802, 16313-16400).

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| PS4-01 | high | build | No CI, build, test or lint gate covers crates/psionic, and the deny-lints are already violated | M |
| PS4-02 | high | testing | 87 of the 90 referenced fixtures are missing, so the tests that compare against committed fixtures fail before any change | M |
| PS4-03 | high | repo-hygiene | crates/psionic/target (448 MB) is not gitignored | S |
| PS4-04 | medium | error-handling | psionic-net transport and relay tasks stop for good on any single send or recv error | S |
| PS4-05 | medium | security | The relay accepts registrations without authentication, they overwrite each other, and its state never shrinks | M |
| PS4-06 | medium | security | The ed25519 secret key and the network state are written as plaintext JSON, without an atomic rename, with default permissions | S |
| PS4-07 | medium | concurrency | The sandbox runner can deadlock on piped stdout/stderr, and the container path strips the env podman needs | S |
| PS4-08 | medium | architecture | Inverted layering: psionic-array depends on both GPU backends, and psionic-transformer pulls them in for one CPU matmul | M |
| PS4-09 | medium | dead-code | About 43k LOC of Tassadar research code sits inside the compiler, ir, sandbox, transformer and cluster crates | L |
| PS4-10 | medium | maintainability | The 26k-line CUDA lib.rs has 198 unsafe blocks and zero SAFETY comments | L |
| PS4-11 | medium | build | The CUDA kernel ABI is kept by hand in three files with no check that they match | M |
| PS4-12 | medium | build | The CUDA build script ignores env changes, builds for one GPU arch only and hardcodes toolkit paths | S |
| PS4-13 | medium | duplication | GGML block dequantizers exist three times, although psionic-core already has a ggml_quantization module | M |
| PS4-14 | medium | duplication | Hundreds of copy-pasted `stable_digest` helpers silently hash empty bytes when serialization fails | M |
| PS4-15 | medium | maintainability | net lib.rs and cluster ordered_state.rs are god modules, and cluster glob re-exports all of psionic-net | M |
| PS4-16 | medium | testing | 92 of the 123 CUDA tests pass vacuously when no GPU is present | S |
| PS4-17 | medium | performance | The CPU backend's dense matmul is a naive single-threaded triple loop | S |
| PS4-18 | medium | error-handling | Code outside tests panics on poisoned mutexes inside Drop and on graphs that were never validated | M |
| PS4-19 | medium | architecture | psionic-serve itself uses Tassadar code, so the extraction must include serve | M |
| PS4-20 | low | performance | Hardcoded 100 ms hello and 75 ms ping intervals, and oversized datagrams dropped without a trace | S |
| PS4-21 | low | performance | Reference attention builds a fresh ArrayContext graph for every (batch, head) | S |
| PS4-22 | low | correctness | The CUDA host-fallback profiler builds JSON by hand with format! and no escaping | S |
| PS4-23 | low | maintainability | The Metal backend embeds about 2,000 lines of MSL in a Rust string and compiles it at run time | S |
| PS4-24 | low | docs | The workspace README and the per-crate READMEs are stale or empty | S |
| PS4-25 | low | build | Dependency hygiene: versions pinned per crate, a feature flag that does nothing, unexpected_cfgs allowed | S |
| PS4-26 | low | architecture | parameter_golf training kernels and a host-fallback path live in the generic CUDA backend | M |

### PS4-01 No CI, build, test or lint gate covers crates/psionic; the configured deny-lints are already violated

**Severity:** high · **Category:** build · **Effort:** M

**Locations:**
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml), lines 41-43
- [Cargo.toml](../../../../Cargo.toml), lines 4-11
- [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs), lines 11238-11248 and 16346-16386
- [psionic-ir/src/lib.rs](../../../../crates/psionic/crates/psionic-ir/src/lib.rs), lines 4046-4058
- [psionic-compiler/src/tassadar_structured_control.rs](../../../../crates/psionic/crates/psionic-compiler/src/tassadar_structured_control.rs), lines 1-1256
- [scripts/pylon-psionic.sh](../../../../scripts/pylon-psionic.sh), line 84

**Evidence:**
- `.github` contains only ISSUE_TEMPLATE. There is no workflows directory.
- The root Cargo.toml excludes `crates/psionic` (lines 4-11). The only script that builds the workspace is pylon-psionic.sh:84 (`cargo build --release --manifest-path .../crates/psionic/Cargo.toml`), and that is a deploy step, not a test or lint gate.
- The workspace sets `unwrap_used`, `expect_used` and `panic` to `"deny"`, yet code outside tests still violates them:
  - HEAD cuda lib.rs calls `.expect("cuda allocator pool mutex should not be poisoned")` at about lines 11238 and 11245, and again in the Drop impl at about 16357 and 16381. The crate attributes allow `expect_used` only under `cfg(test)`.
  - psionic-ir lib.rs:4050 and 4057 call `.expect("all built-in graph ops must have registered schemas")` with no allow attribute. The file's tests start at line 6920.
  - tassadar_structured_control.rs has 31 `.expect(` calls before its `#[cfg(test)]` at line 1257. 30 of them are `expect("root frame")`.
- `git log 737f94a17c..HEAD` shows no commits touching the in-scope crates.

**Impact:** psionic-openai-server links these crates, so a regression only shows up when someone builds by hand on a GPU box. The deny-lints look like a guarantee, but nothing runs clippy to enforce them.

**Suggested action:**
1. Add a Linux CI job without the CUDA toolkit (build.rs falls back to the stub). It should run `cargo check --workspace --manifest-path crates/psionic/Cargo.toml` and `cargo test -p psionic-core -p psionic-ir -p psionic-backend-cpu -p psionic-net -p psionic-cluster`.
2. Run `cargo clippy --workspace --manifest-path crates/psionic/Cargo.toml` once and fix the expects outside tests:
   - Use `lock().unwrap_or_else(PoisonError::into_inner)` for the mutexes.
   - Return a typed error when `frame_stack.last_mut()` is empty.
   - Where an expect really is an invariant, add a scoped `#[allow(clippy::expect_used, reason = ...)]`.
3. Make the job required on main. To verify, clippy should exit 0 in CI.

The verifier lowered this from critical to high. The whole monorepo lacks workflows, so this is not specific to psionic, and this code is not the live revenue path.

### PS4-02 The import left 87 of 90 referenced fixtures missing, so the committed-truth tests fail by construction

**Severity:** high · **Category:** testing · **Effort:** M

**Locations:**
- [crates/psionic/README.md](../../../../crates/psionic/README.md), lines 14-17
- [psionic-core/src/philox.rs](../../../../crates/psionic/crates/psionic-core/src/philox.rs), lines 60 and 249-254
- [psionic-core/src/ternary.rs](../../../../crates/psionic/crates/psionic-core/src/ternary.rs), line 60

**Evidence:**
- The in-scope crates contain 90 distinct `"fixtures/..."` string literals, and 87 of those paths do not exist on disk.
- Only two of the missing paths are outside Tassadar: `fixtures/rng/philox4x32_reference_vectors.json` and `fixtures/quant/ternary_tq_formats_v1.json`. Neither directory exists.
- philox.rs:252 calls `fs::read_to_string(&path).expect("philox fixture must exist")`.
- The README says fixtures/ "holds only the 23 files those crates embed with include_str!".

**Impact:** `cargo test` cannot pass on psionic-core or on any crate that carries Tassadar code, so real regressions hide among the expected failures. The philox and ternary tests guard live RNG and quantization behaviour.

**Suggested action:**
1. Restore `fixtures/rng/philox4x32_reference_vectors.json` and `fixtures/quant/ternary_tq_formats_v1.json` from upstream psionic@02e0bc85.
2. Mark the Tassadar tests that compare against report fixtures `#[ignore = "fixture not imported"]`, or restore those reports.
3. Add a test asserting that every `*_FIXTURE_PATH` const in psionic-core resolves to a file that exists.
4. To verify, `cargo test -p psionic-core` should pass, and a re-run of the fixture-literal scan should show 0 missing paths outside ignored tests.

### PS4-03 crates/psionic/target (448 MB) is not gitignored

**Severity:** high · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [.gitignore](../../../../.gitignore), lines 1-6
- [crates/psionic/README.md](../../../../crates/psionic/README.md), lines 20-27

**Evidence:**
- The root .gitignore has `/target` and `/crates/openagents-mobile/target/`, but no entry for psionic.
- There is no crates/psionic/.gitignore.
- `git check-ignore -v crates/psionic/target/x` prints nothing.
- `du -sh crates/psionic/target` reports 448M.

**Impact:** a `git add -A` by any agent working in this checkout would commit hundreds of MB of build output.

**Suggested action:**
1. Add `/crates/psionic/target/` to the root .gitignore, next to the openagents-mobile entry.
2. To verify, `git check-ignore -v crates/psionic/target/x` should print the matching rule, and `git status` should no longer list `crates/psionic/target/`.

### PS4-04 psionic-net transport and relay tasks die permanently on any single send or recv error

**Severity:** medium · **Category:** error-handling · **Effort:** S

**Locations:** [psionic-net/src/lib.rs](../../../../crates/psionic/crates/psionic-net/src/lib.rs), lines 6171-6207, 7476-7586 and 7593-7612

**Evidence:**
- `run_transport` applies `?` to `send_hello_to_seed_peers`, `send_ping_to_discovered_peers`, `handle_transport_command`, `handle_incoming_message` and `received.map_err(|error| error.to_string())?`.
- `send_encoded_datagram` turns any `send_to` error into an `Err(String)`.
- `run_relay_server` applies `.await?` to each `send_transport_datagram`.
- When either function returns, its task ends.

**Impact:** one unroutable seed peer (EHOSTUNREACH or ENETUNREACH from `send_to`) stops all hello, ping and inbound handling on the node, yet the node handle still looks alive. On the relay side, one bad send takes the relay down for every session.

**Suggested action:**
1. Treat a failed send to one peer, and a failure to decode or handle one datagram, as non-fatal: log it, count it in SharedState and keep looping. Return Err only for fatal socket errors.
2. Replace `Result<(), String>` with a thiserror `TransportError`.
3. Report task liveness in health through `JoinHandle::is_finished`.
4. To verify, add a test that seeds `192.0.2.1:9` alongside a live peer and asserts that the live peer still completes hello and ping.

The verifier lowered this from high to medium. This transport is used by psionic-cluster and the psionic-mesh-lane binary, not the Pylon psionic-openai-server path. Also, on Linux an unconnected UDP socket does not deliver ICMP errors to `recv_from`, so the send branch is the likelier failure.

### PS4-05 The relay server accepts unauthenticated registrations that overwrite each other, and its state grows without bound

**Severity:** medium · **Category:** security · **Effort:** M

**Locations:** [psionic-net/src/lib.rs](../../../../crates/psionic/crates/psionic-net/src/lib.rs), lines 7491-7572 and 4104-4120

**Evidence:**
- For both RelayRegister and RelayForward, the relay runs `.entry(session_key).or_default().insert(sender_node_id/source_node_id, RelayRegistration { remote_addr })`. It uses the node id the datagram claims and checks no signature.
- Nothing ever evicts entries from `registrations`.
- `ClusterRelayServer::spawn` (4104-4120) is public API. In this repo only the tests in psionic-cluster/tests/local_cluster_transport.rs:943 and 1051 spawn it.

**Impact:** anyone who knows a relay_id and a session_tag can take over another node's relay or NAT-introduction address. That allows denial of service and lets the attacker observe traffic, though the payloads stay signed. Registrations under random session keys can also exhaust the relay's memory.

**Suggested action:**
1. Have the claimed node sign RelayRegister and RelayForward with its ed25519 key over (relay_id, session_tag, target, counter). The relay verifies the signature before inserting.
2. Refuse to overwrite a registration from a different address unless the counter is newer.
3. Add a TTL and caps per session and in total.
4. To verify, add tests showing that a spoofed registration is refused and that expired entries are evicted.

The verifier lowered this from high to medium because no production binary in this repo spawns the relay. It remains a risk for any future deployment.

### PS4-06 The node's ed25519 secret key and network state are written non-atomically in plaintext JSON with default permissions

**Severity:** medium · **Category:** security · **Effort:** S

**Locations:**
- [psionic-net/src/lib.rs](../../../../crates/psionic/crates/psionic-net/src/lib.rs), lines 6062-6093 and 3652
- [psionic-serve/src/bin/psionic-mesh-lane.rs](../../../../crates/psionic/crates/psionic-serve/src/bin/psionic-mesh-lane.rs), line 281

**Evidence:**
- The FileBacked identity path builds `PersistedNodeIdentityRecord { auth_secret_key_hex: Some(hex::encode(signing_key.to_bytes())), ... }` and calls `fs::write(path, encoded)` (6093). This runs on every start, because the epoch increases each time.
- psionic-net never uses `set_permissions` or `PermissionsExt`.
- Network state is written with a bare `fs::write` at 3652.
- psionic-mesh-lane stores the identity at `state_dir.join("node.identity.json")` and runs as a systemd or launchd service.

**Impact:** the signing key gets the permissions the umask allows, often 0644. A crash in the middle of a write can corrupt the identity, and the node then has to take a new identity on its next start.

**Suggested action:**
1. Add `write_private_atomic(path, bytes)`. It creates a temp file with mode 0o600 on unix, calls `sync_all`, renames the temp file over the target, and fsyncs the parent directory.
2. Use it for the identity file. Use an atomic variant without the permission change for network state, the operator manifest and cluster ordered_state.
3. To verify, add a unix test asserting that the identity file has mode 0600 after a write.

The verifier lowered this from high to medium. The mesh lane is not part of the deployed Pylon serving path, and the key sits in a state directory owned by the operator. The verifier did not re-open operator_manifest.rs:120 or ordered_state.rs:3852.

### PS4-07 The sandbox subprocess runner can deadlock on piped stdout/stderr, and the container path strips the env podman needs

**Severity:** medium · **Category:** concurrency · **Effort:** S

**Locations:** [psionic-sandbox/src/execution.rs](../../../../crates/psionic/crates/psionic-sandbox/src/execution.rs), lines 221-232, 256-302 and 522-580

**Evidence:**
- stdout and stderr are `Stdio::piped()`. The runner then loops on `child.try_wait()` with `thread::sleep(10ms)`, and calls `wait_with_output()` only after the child exits or is killed. Nothing reads the pipes while the job runs.
- `env_clear()`, the request env, HOME and TMPDIR are applied to the `podman` client command itself.
- `prepare_container_command` passes no `--env` arguments, unlike podman.rs:285.
- On timeout, `child.kill()` kills the podman client, not the container.
- Nothing outside psionic-sandbox in this repo calls `execute_sandbox_job` (execution.rs:173).

**Impact:**
- A job that writes more than about 64 KiB blocks on the full pipe and is reported as TimedOut.
- Container jobs lose their env and may leave orphaned containers behind.
- The bug is latent today because nothing in the repo calls this code.

**Suggested action:**
1. Read both pipes on reader threads from spawn onward, with a byte cap per stream.
2. For containers, keep PATH and XDG_RUNTIME_DIR for podman and pass the request env as `--env K=V`. Name each container `--name psionic-<job>`, and run `podman rm -f` on timeout.
3. To verify, add a test where the job prints 1 MiB and must finish with CleanExit.

The verifier lowered this from high to medium because nothing outside the crate calls the function.

### PS4-08 Inverted layering: psionic-array depends on both GPU backends, and psionic-transformer pulls them in for one CPU matmul

**Severity:** medium · **Category:** architecture · **Effort:** M

**Locations:**
- [psionic-array/Cargo.toml](../../../../crates/psionic/crates/psionic-array/Cargo.toml), lines 17-18
- [psionic-transformer/Cargo.toml](../../../../crates/psionic/crates/psionic-transformer/Cargo.toml), lines 16-18
- [psionic-transformer/src/attention.rs](../../../../crates/psionic/crates/psionic-transformer/src/attention.rs), lines 1 and 724

**Evidence:**
- psionic-array lists psionic-backend-cuda and psionic-backend-metal in `[dependencies]` unconditionally.
- psionic-transformer uses psionic_array in exactly one place, `ArrayContext::cpu()` at attention.rs:724.
- psionic-transformer also declares `psionic-nn = { ..., default-features = false }`, but psionic-nn defines no features.

**Impact:** any consumer of the transformer or array crates that only needs the CPU still compiles the CUDA build script and about 42k LOC of backend code, and the layers cannot be separated.

**Suggested action:**
1. In psionic-array, add the features `cuda = ["dep:psionic-backend-cuda"]` and `metal = ["dep:psionic-backend-metal"]`, both off by default, and put `ArrayContext::cuda` and `ArrayContext::metal` behind them.
2. Replace psionic-transformer's `matmul_with_array` with a plain matmul over contiguous buffers, and remove the psionic-array dependency.
3. Enable the new features in psionic-serve and psionic-train.
4. To verify, `cargo tree -p psionic-transformer | grep backend` should print nothing.

The verifier lowered this from high to medium. The real consumers (serve and train) need the backends anyway, so the cost is mostly architecture and compile time.

### PS4-09 About 43k LOC of Tassadar research code is embedded in the compiler, IR, sandbox, transformer and cluster crates

**Severity:** medium · **Category:** dead-code · **Effort:** L

**Locations:**
- [psionic-compiler/src/lib.rs](../../../../crates/psionic/crates/psionic-compiler/src/lib.rs), lines 3-40
- [psionic-ir/src/lib.rs](../../../../crates/psionic/crates/psionic-ir/src/lib.rs), lines 3-22
- [psionic-cluster/src/lib.rs](../../../../crates/psionic/crates/psionic-cluster/src/lib.rs), lines 24-25
- [psionic-sandbox/src/tassadar_import_policy_matrix.rs](../../../../crates/psionic/crates/psionic-sandbox/src/tassadar_import_policy_matrix.rs), lines 443-448
- [psionic-serve/src/tassadar.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar.rs)

**Evidence:**
- Re-measured `src/tassadar_*` (files / Tassadar LOC / crate LOC):

  | Crate | Files | Tassadar LOC | Crate LOC |
  |---|---|---|---|
  | compiler | 40 | 21,671 | 26,599 |
  | ir | 19 | 9,275 | 25,802 |
  | sandbox | 14 | 9,794 | 15,027 |
  | transformer | 11 | 2,561 | 6,315 |
  | cluster | 2 | 659 | 24,339 |

  The total is about 43.9k LOC.
- cluster lib.rs:24-25 glob re-exports two Tassadar modules.
- psionic-serve has many Tassadar modules of its own (see PS4-19).

**Impact:** the serving binary compiles and links research scaffolding. This slows builds, widens the public API and buries the real compiler and IR. The same code accounts for most of the missing-fixture failures in PS4-02.

**Suggested action:**
1. Create a `psionic-tassadar` crate, or a default-off `tassadar` feature, to hold every `tassadar_*` module from compiler, ir, sandbox, transformer and cluster.
2. Put psionic-serve's `tassadar*.rs` modules and its Tassadar openai_http route behind the same feature (PS4-19).
3. Once only the Tassadar crate uses wasmparser, wasm-encoder and wat, remove them from psionic-ir and psionic-compiler.
4. Track this as one issue with a checklist per crate. To verify, `cargo tree -p psionic-serve --no-default-features` should show no wasm* crates.

The verifier lowered this to medium because it is maintenance weight, not a reliability defect. The reviewer's file counts were too high and have been corrected above.

### PS4-10 The 26k-line psionic-backend-cuda/src/lib.rs god file has 198 unsafe blocks and zero SAFETY comments

**Severity:** medium · **Category:** maintainability · **Effort:** L

**Locations:** [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs), lines 4-18, 9423-17198, 17199-18928 and 18929

**Evidence:**
- At HEAD the file has 26,043 lines. The Linux `mod platform` starts at line 9424, the stub for other platforms at 17199, and `mod tests` at 18930.
- `grep -c 'unsafe {'` returns 198. `grep -c SAFETY` returns 0.
- The crate-level allow covers `too_many_arguments`, `result_large_err` and `manual_is_multiple_of`.

**Impact:** the file is hard to review, and agents working at the same time collide in it (it has uncommitted changes right now). The FFI invariants are not written down anywhere.

**Suggested action:**
1. Split the file mechanically into `ffi/`, `allocator.rs`, `submission.rs`, `graph_exec.rs`, `host_fallback.rs`, `validate.rs`, `kernels/*.rs` and `tests/`, with no behaviour changes.
2. Enable `clippy::undocumented_unsafe_blocks` in `ffi` and add SAFETY comments. In particular, explain why `PlatformBufferInner::drop` can return pooled device pointers to the pool without a stream sync.
3. Agree on timing with the agent that currently has uncommitted changes in this file.
4. To verify, no file in the crate should exceed about 3k lines, and clippy should pass with the new lint.

### PS4-11 The CUDA kernel ABI is maintained by hand in three places (.cu, stub .c, Rust extern table) with no parity check

**Severity:** medium · **Category:** build · **Effort:** M

**Locations:**
- [psionic-backend-cuda/build.rs](../../../../crates/psionic/crates/psionic-backend-cuda/build.rs), lines 8-21
- [kernels/quantized_matvec_stub.c](../../../../crates/psionic/crates/psionic-backend-cuda/src/kernels/quantized_matvec_stub.c)
- [kernels/quantized_matvec.cu](../../../../crates/psionic/crates/psionic-backend-cuda/src/kernels/quantized_matvec.cu)

**Evidence:**
- At HEAD the .cu file defines 90 distinct `int psionic_cuda_*(` functions.
- The stub .c is 1,902 lines.
- The Rust side names 106 distinct `fn psionic_cuda_*` symbols.
- When `find_nvcc()` returns None, build.rs (14-20) compiles the stub silently and emits no `cargo:warning`.

**Impact:** if the signatures drift apart, the result is undefined behaviour. A CUDA box without nvcc on PATH ships stub kernels and nothing at build time says so.

**Suggested action:**
1. Emit `cargo:warning=psionic-backend-cuda: nvcc not found, linking stub kernels`.
2. Make `PSIONIC_REQUIRE_NVCC=1` a hard error when nvcc is missing, and set it in scripts/pylon-psionic.sh.
3. Add a unit test that parses the three files and asserts that their symbol sets match, or generate the stub from a shared `abi.h`.
4. To verify, the parity test passes, and `PSIONIC_REQUIRE_NVCC=1 cargo build` fails on a machine without nvcc.

### PS4-12 The CUDA build script ignores env changes, pins the build to one host GPU arch, and hardcodes toolkit paths

**Severity:** medium · **Category:** build · **Effort:** S

**Locations:** [psionic-backend-cuda/build.rs](../../../../crates/psionic/crates/psionic-backend-cuda/build.rs), lines 9-11, 24-39, 55-70 and 72-98

**Evidence:**
- build.rs emits three `rerun-if-changed` lines and no `rerun-if-env-changed`.
- The arch comes from CUDAARCHS, from PSI_CUDA_ARCH, or from running `nvidia-smi` at build time, and is passed as a single `-arch=sm_XX`. If all three are absent, no `-arch` is passed at all.
- The link search paths are hardcoded to `/opt/cuda/lib64` and `/usr/local/cuda/lib64`.

**Impact:**
- A stale stub build survives a toolkit install.
- Binaries do not run on GPU generations older than the build host's.
- Nix hosts depend on the wrapper script to find the toolkit.

**Suggested action:**
1. Add `cargo:rerun-if-env-changed` for NVCC, CUDAARCHS, PSI_CUDA_ARCH and CUDA_PATH.
2. Accept a list in CUDAARCHS and emit one `-gencode` per arch. Default to a documented set, for example 80;86;89.
3. Derive the lib64 path from nvcc's parent directory or from CUDA_PATH.
4. To verify, changing CUDAARCHS triggers a rebuild, and `cuobjdump --list-elf` on the built library lists every configured arch.

The verifier corrected one point: nvcc's `-arch=sm_XX` shorthand also embeds compute_XX PTX, so newer GPUs can JIT the kernels. The real gaps are older GPU generations and the case where no arch is passed.

### PS4-13 GGML block dequantizers are copied three times (backend-cpu, nn, models) while psionic-core already owns ggml_quantization

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [psionic-backend-cpu/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cpu/src/lib.rs), line 1922
- [psionic-nn/src/quantized.rs](../../../../crates/psionic/crates/psionic-nn/src/quantized.rs), line 1140
- [psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs), line 10012
- [psionic-core/src/ggml_quantization.rs](../../../../crates/psionic/crates/psionic-core/src/ggml_quantization.rs), lines 35-119

**Evidence:**
- `fn decode_q4_k_scale_min(index: usize, packed: &[u8]) -> (u8, u8)` is defined at all three of the first three locations.
- psionic-core already exposes `pub fn decode_q3_k_block`, `decode_iq3_s_block` and `decode_iq4_xs_block`.

**Impact:** a fix to a quant format has to land three times. If the copies drift, the CPU backend and model loading silently produce different numbers.

**Suggested action:**
1. Move the q4_0, q4_k, q6_k, q8_0 and mxfp4 block decoders into psionic-core/src/ggml_quantization.rs, in the style of the existing decoders.
2. Delete the three copies and point their callers at psionic-core.
3. Add golden-vector tests for each format.
4. To verify, `git grep -c 'fn decode_q4_k_scale_min'` returns exactly one match.

### PS4-14 Hundreds of copy-pasted `stable_digest` helpers silently hash empty bytes when serialization fails

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:** [psionic-sandbox/src/tassadar_import_policy_matrix.rs](../../../../crates/psionic/crates/psionic-sandbox/src/tassadar_import_policy_matrix.rs), lines 443-448 (a representative copy)

**Evidence:**
- The same helper is copied over and over: `fn stable_digest<T: Serialize>(prefix, value) { ...; hasher.update(serde_json::to_vec(value).unwrap_or_default()); ... }`.
- Across crates/psionic/crates there are 654 exact copies of that signature, 96 `where`-clause variants and 21 `impl Serialize` variants, about 771 free functions in all.
- The audit scope alone has 121 `fn stable_digest` definitions, counting methods.
- `fn repo_root(` appears 479 times and `fn read_json` 271 times.

**Impact:** if serialization ever fails, identity and receipt digests collapse to H(prefix). No single place defines what canonical JSON means.

**Suggested action:**
1. Add `psionic_core::digest::{stable_digest, canonical_json_bytes}`. Either return a Result, or panic with a documented invariant.
2. Replace the copies with an ast-grep or sed pass, one crate per commit.
3. Move `repo_root` and `read_json` into a shared test-support crate.
4. To verify, `git grep -c 'fn stable_digest'` drops to one definition plus documented exceptions.

The verifier corrected the reviewer's counts. For derived Serialize types failure is rare, so the main risk is the duplication itself.

### PS4-15 psionic-net/src/lib.rs (10k lines) and cluster ordered_state.rs (7.4k) are god modules, and cluster glob re-exports all of psionic-net

**Severity:** medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [psionic-net/src/lib.rs](../../../../crates/psionic/crates/psionic-net/src/lib.rs)
- [psionic-cluster/src/ordered_state.rs](../../../../crates/psionic/crates/psionic-cluster/src/ordered_state.rs)
- [psionic-cluster/src/lib.rs](../../../../crates/psionic/crates/psionic-cluster/src/lib.rs), lines 15-26

**Evidence:**
- `wc -l` gives 10,029 lines for net lib.rs and 7,422 for ordered_state.rs.
- cluster lib.rs:15-26 has 12 `pub use x::*` lines, including `pub use psionic_net::*` at line 19.

**Impact:** security-relevant code sits next to bookkeeping code in the same files, and every item in psionic-net becomes part of cluster's public API.

**Suggested action:**
1. Split net into `identity`, `trust`, `wire`, `transport` and `relay` modules.
2. Split ordered_state into `membership`, `leadership`, `recovery`, `commands` and `artifacts`.
3. Replace `pub use psionic_net::*` with explicit re-exports. Remove the glob first and let the compiler errors in serve and train show which items they need.
4. To verify, the workspace builds and `git grep 'pub use psionic_net::\*'` returns nothing.

### PS4-16 92 of 123 CUDA tests pass vacuously when no device is present

**Severity:** medium · **Category:** testing · **Effort:** S

**Locations:** [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs), line 18929

**Evidence:** HEAD lib.rs has 123 `#[test]` functions. 92 of them are named `*_when_available`, and they return Ok when `selected_device()` is None. None of them is marked `#[ignore]`, and no CI runs them.

**Impact:** kernel regressions in the CUDA serving lane go unnoticed, and the test count overstates real coverage.

**Suggested action:**
1. When `PSIONIC_REQUIRE_CUDA=1` is set, the tests fail if no device is found.
2. Run the suite with that variable set on coderos-4080 before shipping a psionic-openai-server build.
3. Do the same for Metal with an equivalent variable.
4. To verify, `PSIONIC_REQUIRE_CUDA=1 cargo test -p psionic-backend-cuda` fails on a Mac and passes on coderos-4080.

### PS4-17 The CPU backend's dense matmul is a naive single-threaded triple loop with column-strided access

**Severity:** medium · **Category:** performance · **Effort:** S

**Locations:**
- [psionic-backend-cpu/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cpu/src/lib.rs), lines 500-538
- [psionic-backend-cpu/Cargo.toml](../../../../crates/psionic/crates/psionic-backend-cpu/Cargo.toml)

**Evidence:**
- The loop is `for row { for col { for inner { left[row*k+inner] * right[inner*n+col] } } }`. The right-hand matrix is read down its columns.
- backend-cpu depends on neither rayon nor matrixmultiply.
- psionic-serve uses CpuBackend (serve lib.rs:108, 5330, 5528).

**Impact:** graph matmul on the CPU lane is cache-hostile and uses one core.

**Suggested action:**
1. Switch to i-k-j loop order or a transposed right-hand matrix, and block over k.
2. Parallelize over rows with rayon (already a workspace dependency), or call the matrixmultiply crate.
3. Keep psionic-backend-tests as the correctness oracle. To verify, the conformance tests pass, and a micro-benchmark on a 512x512 matmul shows the speedup.

### PS4-18 Non-test panics on poisoned mutexes inside Drop and on unvalidated graphs

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs), lines 16346-16386
- [psionic-ir/src/lib.rs](../../../../crates/psionic/crates/psionic-ir/src/lib.rs), lines 4046-4058
- [psionic-ir/src/autodiff.rs](../../../../crates/psionic/crates/psionic-ir/src/autodiff.rs)

**Evidence:**
- `impl Drop for PlatformBufferInner` calls `.lock().expect("cuda allocator pool mutex should not be poisoned")` twice. A panic inside Drop during an unwind aborts the process.
- `schema_for_op_kind` does a `find(...).expect(...)`.
- autodiff.rs indexes `inputs()[` 253 times without checking the length.

**Impact:** a poisoned lock turns into a process abort, and a malformed graph causes a panic instead of an error.

**Suggested action:**
1. In the Drop impls, use `lock().unwrap_or_else(PoisonError::into_inner)`.
2. Add `Graph::validate()` and run it on deserialization via `#[serde(try_from)]`. Add an `input(node, n)?` helper and use it in place of raw indexing in autodiff.
3. Make the schema lookup an exhaustive match, with a test that covers every OpKind.
4. To verify, clippy `expect_used` passes for both files outside tests, and a test that deserializes a malformed graph gets back an Err.

This overlaps PS4-01. Keep this finding as the concrete list of fixes.

### PS4-19 Tassadar code is wired into psionic-serve itself, not just examples, so the extraction plan must include serve

**Severity:** medium · **Category:** architecture · **Effort:** M

**Locations:**
- [psionic-serve/src/tassadar.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar.rs)
- [psionic-serve/src/openai_http/tassadar_post_article_router_plugin_tool_loop_pilot.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http/tassadar_post_article_router_plugin_tool_loop_pilot.rs)
- [psionic-serve/src/lib.rs](../../../../crates/psionic/crates/psionic-serve/src/lib.rs)

**Evidence:**
- `git grep -l tassadar -- psionic-serve/src` lists lib.rs, openai_http.rs, tassadar.rs, psion_served_evidence.rs, psion_family_serve_vocabulary.rs and several `tassadar_article_*_publication*.rs` modules.
- This contradicts the reviewer's claim that serve used Tassadar only from examples and one openai_http submodule.

**Impact:** gating Tassadar out of compiler, ir, sandbox and transformer will not shrink psionic-openai-server unless serve's own modules and routes are gated as well.

**Suggested action:**
1. Add a default-off `tassadar` feature to psionic-serve.
2. Put its `tassadar*.rs` modules, the openai_http pilot route and the references in psion_served_evidence behind that feature. Enable it only for research binaries.
3. To verify, `cargo build -p psionic-serve --bin psionic-openai-server -v | grep -c tassadar` returns 0.

### PS4-20 Fixed 100 ms hello and 75 ms ping intervals, with JSON-over-UDP envelopes and silent drops of oversized datagrams

**Severity:** low · **Category:** performance · **Effort:** S

**Locations:** [psionic-net/src/lib.rs](../../../../crates/psionic/crates/psionic-net/src/lib.rs), lines 44-46 and 6179-6185

**Evidence:**
- `const HELLO_INTERVAL = 100ms; const PING_INTERVAL = 75ms; const MAX_DATAGRAM_BYTES = 8 * 1024;` are hardcoded, and no config can change them.
- The relay drops datagrams it cannot decode with `Err(_) => continue` (7487-7490).

**Impact:** the chatter wastes bandwidth on WAN meshes, and oversized relayed envelopes disappear without a log entry.

**Suggested action:**
1. Move the intervals into TransportConfig. Default to about 1-2 s in production and keep the fast values for tests.
2. Check the size at send time and return a typed error when a datagram is too large.
3. Count dropped inbound datagrams and expose the count in health.
4. To verify, a test that sends an oversized envelope gets the typed error and sees the drop counter go up.

### PS4-21 Reference attention builds and evaluates a fresh ArrayContext graph per (batch, head)

**Severity:** low · **Category:** performance · **Effort:** S

**Locations:** [psionic-transformer/src/attention.rs](../../../../crates/psionic/crates/psionic-transformer/src/attention.rs), lines 716-760

**Evidence:** `matmul_with_array` creates a new `ArrayContext::cpu()` (line 724) on every call, and it is called inside the per-head loop.

**Impact:** the public `scaled_dot_product_attention` is slow at anything beyond toy sizes.

**Suggested action:**
1. Replace it with a direct matmul over contiguous buffers, shared with the CPU backend fix in PS4-17. Alternatively, rename it and document it as a reference implementation.
2. To verify, the existing attention tests pass, and `cargo tree -p psionic-transformer` no longer lists psionic-array (PS4-08).

### PS4-22 The CUDA host-fallback profiler hand-builds JSON with format! and no escaping

**Severity:** low · **Category:** correctness · **Effort:** S

**Locations:** [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs), lines 4855-4909

**Evidence:**
- The profiler builds each record with `format!("{{\"label\":\"{}\"...\"detail\":\"{}\"...")` and inserts raw strings.
- Labels come from `op.label()`, which returns static strings, so they are safe.
- The one free-form detail is `ExecutionOp::BackendExtension { op } => format!("{op:?}")`, and that Debug output can contain quotes.
- serde_json is only a dev-dependency of this crate.

**Impact:** fallback records for BackendExtension ops can produce invalid JSONL lines.

**Suggested action:**
1. Derive Serialize on the profile structs.
2. Move serde and serde_json to normal dependencies and write each record with `serde_json::to_string`.
3. To verify, add a test where the detail contains a quote and assert that the output line parses as JSON.

### PS4-23 The Metal backend embeds about 2,000 lines of MSL as a Rust string and compiles it at run time

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:** [psionic-backend-metal/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-metal/src/lib.rs), lines 10193, 10854 and 12877-12878

**Evidence:**
- `const EMBEDDINGS_METAL_SOURCE: &str = r"` starts at line 10854.
- `new_library_with_source(EMBEDDINGS_METAL_SOURCE, ...)` compiles it at line 10193.
- The stub platform module for other OSes is at line 12877.

**Impact:** shader editors and linters cannot work on the code, shader errors appear only at run time, and every start pays the compile cost.

**Suggested action:**
1. Move the shaders into `src/kernels/*.metal` and load them with `include_str!`.
2. Optionally, precompile a metallib in build.rs on macOS.
3. To verify, the Metal test suite passes on a Mac and lib.rs no longer contains the MSL source.

### PS4-24 The workspace README and per-crate READMEs are stale or empty

**Severity:** low · **Category:** docs · **Effort:** S

**Locations:**
- [crates/psionic/README.md](../../../../crates/psionic/README.md), lines 14-17
- [psionic-backend-cuda/README.md](../../../../crates/psionic/crates/psionic-backend-cuda/README.md), lines 1-3

**Evidence:**
- The README says fixtures/ holds "only the 23 files" embedded with include_str!, but `git ls-files crates/psionic/fixtures | wc -l` returns 208.
- All 30 crate READMEs are 3 lines long ("Imported from OpenAgentsInc/psionic at 02e0bc85...").

**Impact:** contributors cannot tell what each crate does or which fixtures matter.

**Suggested action:**
1. Rewrite README:14-17 to describe the embedded fixtures, the runtime fixtures and the clef tools separately.
2. Give each crate a README of 5-10 lines, built from its `CRATE_ROLE` const.
3. Prefix issue references that point upstream with `OpenAgentsInc/psionic#`.
4. To verify, the fixture count in the README matches `git ls-files crates/psionic/fixtures | wc -l`.

### PS4-25 Dependency hygiene: versions pinned per crate instead of through the workspace, a no-op feature flag, unexpected_cfgs allowed

**Severity:** low · **Category:** build · **Effort:** S

**Locations:**
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml), lines 5-7
- [psionic-backend-cuda/Cargo.toml](../../../../crates/psionic/crates/psionic-backend-cuda/Cargo.toml), lines 18-20
- [psionic-transformer/Cargo.toml](../../../../crates/psionic/crates/psionic-transformer/Cargo.toml), line 18

**Evidence:**
- The cuda crate pins `half = "2.4.1"`, `libloading = "0.8.9"` and `rayon = "1.10"`, while the workspace declares `rayon = "1"`.
- psionic-transformer sets `default-features = false` on psionic-nn, which has no features.
- The workspace sets `unexpected_cfgs = { level = "allow" }` and declares no `rust-version`.
- Cargo.lock has 60 packages with more than one version (not re-counted by the verifier).

**Impact:** versions drift between crates, and typos in cfg names go unnoticed.

**Suggested action:**
1. Move half, libloading, rayon, the wasm* crates, tempfile and metal into `[workspace.dependencies]`.
2. Remove the flag that does nothing.
3. Set `unexpected_cfgs` to warn and add a `rust-version`.
4. Run `cargo tree -d` and file follow-ups for the duplicate versions. To verify, `cargo check --workspace` passes with no new unexpected_cfgs warnings left unexplained.

### PS4-26 Training-specific parameter_golf kernels and a host-fallback path live inside the generic CUDA backend

**Severity:** low · **Category:** architecture · **Effort:** M

**Locations:**
- [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs), lines 4780-4812
- [kernels/quantized_matvec.cu](../../../../crates/psionic/crates/psionic-backend-cuda/src/kernels/quantized_matvec.cu)

**Evidence:**
- HEAD lib.rs has 171 matches for `parameter_golf|ParameterGolf`.
- The host-fallback profiler (line 4780 onward) records CPU fallbacks only to a file, and only when an env variable enables it.

**Impact:** training kernels are compiled into serving builds, and a silent CPU fallback can slow GPU serving without anyone noticing.

**Suggested action:**
1. Put the parameter_golf kernels and steps behind a cargo feature.
2. Report host-fallback counts in ExecutionMetrics or health.
3. Add `PSIONIC_CUDA_FORBID_HOST_FALLBACK=1` and use it in serving smoke tests.
4. To verify, a serving build without the feature has no parameter_golf symbols (`nm | grep -c parameter_golf` returns 0), and the smoke test fails if any fallback happens.

The verifier lowered this from medium to low: it is coupling and build weight, with no demonstrated defect in serving.
