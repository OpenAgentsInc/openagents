# Psionic: training

**Section:** PS1 · **Scope:** `crates/psionic/crates/psionic-train` (417k lines, the largest crate in the repo) · **Health grade: D**

Audit date 2026-10-10, snapshot commit `3168c986aa11e18a8bd30f52c270f609e49b3815`.

psionic-train has 417k Rust lines in 666 tracked files: 389k in `src` (363 files, 97 of them bins), 24k in 203 examples, and one 3.7k-line integration test. All of it arrived in a single import commit (`737f94a17c`, 2026-10-07), and nobody has changed it since. It is in the repo only because psionic-serve, the OpenAI-compatible model server that Pylon runs, depends on it. The server uses about a dozen of its types: the Gemma E4B adapter checkpoint types, the Qwen3.8 LM-head adapter loader, the Psion route and acceptance types, and a few Tassadar reports. Because of that, the whole crate is compiled into the serving binary, directly and again through psionic-research. `lib.rs` declares 361 flat modules and re-exports each one with `pub use x::*`. The result is 5,528 top-level public items, 1,410 of them constants.

The import did not bring in most of the files the crate refers to. 621 of about 730 fixture, docs and scripts path strings point at files that do not exist, and there is no `scripts/` directory at all. No CI runs the crate's tests, so the 1,387 tests cannot be green and nobody would notice. Library code finds the repo at run time from the compile-time `CARGO_MANIFEST_DIR` and runs `bash` on scripts that are gone. The CLI's dirty-tree check covers the whole monorepo. Six networked modules sign messages with ed25519 keys derived from public strings, and their verifiers trust the public key carried in the message, so the signatures prove nothing. This is latent today rather than live. A large share of the code is dead. Worth keeping: typed per-module errors, the schema-version-plus-digest validation habit, BTreeMap ordering for deterministic digests, and careful adapter loading. The individual files are reasonably disciplined, but the crate as a whole is mostly unverified, partly dead, and should not be attached to the serving path.

## Measurements

| Metric | Value |
|---|---|
| Size | 417,177 Rust lines, 666 tracked files |
| `src` | 389,257 lines (incl. `src/bin`: 6,866 lines over 97 bins) |
| Examples | 24,192 lines over 203 files |
| Integration test | `tests/psionic_train_cli.rs`: 3,728 lines |
| README | 3 lines |
| `lib.rs` | 1,716 lines; 361 module declarations; 361 `pub use x::*` |
| Public surface | 5,528 top-level pub items (1,410 pub consts) |
| Largest files | `parameter_golf_single_h100_training.rs` 8,736; `psion_reference_pilot.rs` 7,203; `parameter_golf_distributed_8xh100_train_step.rs` 5,849; `parameter_golf_reference.rs` 5,504; `examples/psion_actual_pretraining_operator.rs` 5,213; `apple_adapter.rs` 5,138; `qwen_legal_adapter_sft.rs` 4,786; `model_io.rs` 3,687 |
| Functions (brace heuristic) | 9,791 total; 303 over 150 lines; 63 over 300 lines; largest `run_with_args` 1,813 lines |
| Module families | `psion_*` 101 files / 103.6k; `parameter_golf_*` 34 / 49.9k; `tassadar*` 47 / 27.2k; `qwen*` 18 / 27.2k; `*_contract*` 35 / 26.1k; `compiled_agent_*` 11 / 14.3k |
| Tests | 1,387 `#[test]`, 400 `#[cfg(test)]` blocks, 0 `#[ignore]`; no CI workflow (`.github` has no `workflows/`) |
| Panic-prone (non-test lib) | 449 `.expect(` (195 on serde serialization in digest helpers), 18 `panic!`/`unreachable!`, 1 `.unwrap()` |
| Panic-prone (whole crate) | 1,824 expect, 77 unwrap, 27 `panic!` |
| Allow attributes | 62 (reviewer: 45 `too_many_arguments`; verifier re-measured 50 across src + examples), 5 `dead_code`, 7 expect/panic allows on test modules |
| Duplication | ~189 file-local `stable_digest` helpers (~800 across psionic); 176 clone/clear-digest patterns; 176-177 repo_root/workspace_root helpers |
| Env / processes (non-test lib) | 68 `CARGO_MANIFEST_DIR` uses (43 files); 22 `Command::new`; 40 `env::var` reads (14 files) |
| Path references | ~730 distinct fixtures/docs/scripts literals, ~621 missing; 480 of 534 fixture-path consts missing; 86 into nonexistent `scripts/`; 173 into `docs/` |
| Dead code | 20 modules (15,890 lines) unreferenced outside their file; 62 modules (56,510 lines) reportedly reached only by their own bin/example (this aggregate is unreliable, see PS1-06) |
| Dependents | psionic-serve, psionic-research |
| Git activity | 1 commit (the import) |

## Strengths

- Errors are typed, not strings. Each module has its own `thiserror` enum with structured variants, for example `TrainingSessionError` at `src/lib.rs:754` and `RewardLedgerContractError`.
- Artifacts carry a schema version, a self-digest and a `validate()` check. The Qwen3.8 adapter loader (`src/qwen38_training_adapter.rs:1016-1067`) checks the artifact digest, lineage metadata, rank, alpha bits and parameter count before it accepts an adapter. Keep these checks when the loader is moved out of the crate.
- Digests are deterministic. Serialized maps use `BTreeMap`. The crate has only 4 `HashMap` uses, all in `parameter_golf_reference.rs`.
- The live buy-mode HTTP dispatcher fails closed. It does nothing unless `PSIONIC_BUY_MODE_HTTP_ARM=armed` is set, and its errors never include the bearer token or the endpoint URL (`src/coordinator_http_buymode_dispatch.rs:1-16`).
- The CPU budget is applied at process start. The one production `unsafe env::set_var` has a SAFETY comment explaining that it runs before any threads exist (`src/main.rs:113-130`).
- Most tests write scratch output to `tempfile` directories. The workspace lint profile denies unwrap, expect, panic and print, so the standard is already defined even though this crate does not meet it.
- The canonical contract modules have negative tests. For example, `reward_ledger_contract.rs:530-545` checks that `validate()` rejects a mutated contract.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| PS1-01 | High | architecture | Serving binary links the entire 417k-line training crate for about a dozen types | L |
| PS1-02 | High | testing | ~621 of ~730 fixture/docs/scripts references dangle; many test modules cannot pass | L |
| PS1-03 | Medium | security | ed25519 signatures prove nothing: keys come from public strings, verifiers trust the embedded key | M |
| PS1-04 | Medium | error-handling | Run-time repo lookup via compile-time `CARGO_MANIFEST_DIR`; `bash` on missing scripts | M |
| PS1-05 | Medium | correctness | Dirty-tree check covers the whole monorepo; `crates/psionic/target` not ignored | S |
| PS1-06 | Medium | dead-code | Large dead or single-consumer module set, unchanged since import | M |
| PS1-07 | Medium | architecture | Flat 361-module crate with glob re-exports and session logic in `lib.rs` | L |
| PS1-08 | Medium | maintainability | 8.7k-line training file and ~1,000-line functions | L |
| PS1-09 | Medium | duplication | Digest and repo-root helpers copied hundreds of times | M |
| PS1-10 | Medium | error-handling | Workspace denies expect/panic, but non-test lib code has hundreds of `.expect` | M |
| PS1-11 | Medium | error-handling | CLI classifies errors by substring, maps lane errors to BadConfig, compiles a 5.2k-line example into the binary | M |
| PS1-12 | Medium | security | Hand-rolled TCP framing: one byte per read, unbounded buffer, peer-chosen allocation | S |
| PS1-13 | Medium | concurrency | Tests mutate process env in parallel; a test writes fixtures into the source tree | S |
| PS1-14 | Low | maintainability | Contract documents as Rust builders plus 97 near-identical writer bins | M |
| PS1-15 | Low | duplication | Example and test copies of the same benchmark package builder | S |
| PS1-16 | Low | dead-code | Feature-gated legacy Apple toolkit module that no build compiles | S |
| PS1-17 | Low | docs | Stale references and placeholder README | S |
| PS1-18 | Low | build | Inline dependency versions bypass the workspace | S |
| PS1-19 | Low | docs | `crates/psionic/README.md` says 23 fixture files; 211 exist | S |

### PS1-01 Serving binary links the entire 417k-line training crate for about a dozen types

**Severity:** High · **Category:** architecture · **Effort:** L

**Locations:**
- [psionic-serve/Cargo.toml](../../../../crates/psionic/crates/psionic-serve/Cargo.toml) (Cargo.toml:55)
- [psionic-serve/src/gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (gguf.rs:34)
- [psionic-serve/src/qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs) (qwen35.rs:36)
- [psionic-serve/src/psion_capability_matrix.rs](../../../../crates/psionic/crates/psionic-serve/src/psion_capability_matrix.rs)
- [psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs)
- [psionic-research/Cargo.toml](../../../../crates/psionic/crates/psionic-research/Cargo.toml)
- [psionic-train/Cargo.toml](../../../../crates/psionic/crates/psionic-train/Cargo.toml)
- [crates/psionic/README.md](../../../../crates/psionic/README.md)

**Evidence:** `psionic-serve/Cargo.toml:55` declares `psionic-train = { path = "../psionic-train" }`. `gguf.rs:34-37` imports `GemmaE4bCudaAdapterCheckpoint`, `GemmaE4bCudaAdapterExportedArtifact` and `GemmaE4bServedBaseModelBinding`. Nine serve source files import `psionic_train`: gguf, qwen35, tassadar, tassadar_article_transformer_minimal_frontier, psion_capability_matrix, psion_capability_withdrawal, psion_generic_load_and_generate, psion_served_output_claim_posture and psion_served_evidence. psionic-serve also depends on psionic-research (around Cargo.toml line 51), which itself declares `psionic-train`, and 7+ serve `tassadar_*` modules import `psionic_research`. `crates/psionic/README.md` describes the imported code as "mostly training and evaluation code that psionic-serve links".

**Impact:** Every `psionic-openai-server` build compiles about 417k lines of training, research and ledger code. Compile time and the security-review surface of the server grow with this crate.

**Suggested action:**
1. Move the Gemma E4B adapter checkpoint and binding types, plus `load_qwen38_lm_head_adapter_safetensors`, into a small crate (for example `psionic-adapter-artifacts`). Move the Psion route and acceptance types used by `psion_capability_*` and `psion_served_*` into psionic-eval or a `psion-contracts` crate. Re-export them from psionic-train so other callers keep working.
2. Put serve's `tassadar_*` and `psion_capability_*` publication modules behind a non-default `research` feature. That feature must also gate the psionic-research dependency, because psionic-research depends on psionic-train. Removing only the direct edge is not enough.
3. Remove psionic-train from serve's `[dependencies]`.
4. Verify: `cargo tree -p psionic-serve -e normal --no-default-features | grep psionic-train` prints nothing.

### PS1-02 Import left ~621 of ~730 fixture/docs/scripts references dangling, so many test modules cannot pass

**Severity:** High · **Category:** testing · **Effort:** L

**Locations:**
- [tassadar_default_train_rehearsal.rs](../../../../crates/psionic/crates/psionic-train/src/tassadar_default_train_rehearsal.rs) (tassadar_default_train_rehearsal.rs:33, tassadar_default_train_rehearsal.rs:622)
- [psion_plugin_argument_construction_benchmark.rs](../../../../crates/psionic/crates/psionic-train/src/psion_plugin_argument_construction_benchmark.rs) (psion_plugin_argument_construction_benchmark.rs:139)
- [crates/psionic/README.md](../../../../crates/psionic/README.md)
- [crates/psionic/fixtures](../../../../crates/psionic/fixtures)

**Evidence:** Re-measured: psionic-train contains 730 distinct `fixtures/|docs/|scripts/` literals, and 621 of them do not exist under `crates/psionic`. 86 of them point into `scripts/`, and `crates/psionic/scripts` does not exist. `tassadar_default_train_rehearsal.rs:622-632` reads `fixtures/tassadar/operator/tassadar_default_train_rehearsal_bundle_v1.json` with `.expect("bundle fixture")`, but that directory contains only `tassadar_default_train_lane_contract_v1.json`. `crates/psionic/fixtures` has 211 files. There is no `.github/workflows` directory.

**Impact:** `cargo test -p psionic-train` cannot pass, so its ~1,387 tests protect nothing. A real regression looks the same as an expected missing-fixture failure.

**Suggested action:**
1. Add a test that enumerates every `*_FIXTURE_PATH` / `*_REF` const and asserts that the file exists relative to `crates/psionic`.
2. For each failing module, do one of three things: import the fixture from `OpenAgentsInc/psionic@02e0bc85`, delete the tests that read it, or delete the module if it is dead (see PS1-06).
3. Record each decision in `crates/psionic/README.md`.
4. Add `cargo test -p psionic-train --lib` to whichever lane runs psionic tests (`scripts/pylon-psionic.sh` or a new lane).
5. Verify: the existence test and `cargo test -p psionic-train --lib` both pass.

### PS1-03 ed25519 signatures prove nothing: keys come from public strings and verifiers trust the key carried in the message

**Severity:** Medium · **Category:** security · **Effort:** M

**Locations:**
- [coordinator_live_buymode_dispatch.rs](../../../../crates/psionic/crates/psionic-train/src/coordinator_live_buymode_dispatch.rs) (coordinator_live_buymode_dispatch.rs:673, :697, :727)
- [qwen_legal_pylon_dispatch.rs](../../../../crates/psionic/crates/psionic-train/src/qwen_legal_pylon_dispatch.rs) (qwen_legal_pylon_dispatch.rs:1024, :1050)
- [qwen_legal_pylon_training_job.rs](../../../../crates/psionic/crates/psionic-train/src/qwen_legal_pylon_training_job.rs) (qwen_legal_pylon_training_job.rs:1055, :1841)
- [bin/qwen_legal_pylon_worker_server.rs](../../../../crates/psionic/crates/psionic-train/src/bin/qwen_legal_pylon_worker_server.rs)

**Evidence:**
- **Keys come from public strings.**
  - `coordinator_live_buymode_dispatch.rs:727-739`: `scheduler_signing_key` and `worker_signing_key` call `deterministic_signing_key`, which is SHA-256 of `"coordinator-buy-mode-scheduler|{run_id}"`.
  - `qwen_legal_pylon_dispatch.rs:1024` uses SHA-256 of `"qwen-legal-pylon-dispatch|{run_id}"`.
  - `qwen_legal_pylon_training_job.rs:1841` uses SHA-256 of `"qwen-legal-pylon-worker|{worker_id}|{job_id}"`.
- **Verifiers trust the key in the message.**
  - `verify_signed_envelope` decodes `envelope.scheduler_pubkey_hex` (around buymode line 688).
  - `verify_worker_verdict_receipt` uses `receipt.worker_pubkey_hex` (around line 716).
  - `verify_worker_receipt_signature` (around pylon_dispatch line 1060) and `training_job:1055` use `receipt.worker_pubkey`.
  - None of them compares the key against an expected key.
- **Runnable today.** `bin/qwen_legal_pylon_worker_server` binds a TCP address chosen by the caller and serves signed envelopes that are verified this way.

**Impact:** Anyone can forge a valid envelope or receipt in two ways: sign with any key and embed that key's pubkey, or derive the "secret" from a known run or worker id. The risk is currently contained: `coordinator_live_buymode_dispatch` has no references outside `lib.rs`, the Tailnet and Production lanes require explicit arming, and the worker keys are labelled protocol-smoke. The worker-server bin, however, can be run today.

**Suggested action:**
1. Make every `verify_*` function take `expected: &VerifyingKey` (or the admitted-worker registry) and reject a message whose embedded pubkey does not match.
2. Move `deterministic_signing_key`, `scheduler_signing_key`, `worker_signing_key` and `deterministic_worker_signing_key` under `#[cfg(test)]` or a `smoke-keys` feature. Production paths should load keys from operator key material (for example `signed_node_identity_contract`).
3. Add a negative test: a receipt signed by a fresh random key that carries its own pubkey must be rejected.
4. Until steps 1-2 land, make the Tailnet and Production constructors return an error.

### PS1-04 Library code finds the repo at run time from the compile-time CARGO_MANIFEST_DIR and calls bash on scripts that no longer exist

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [psion_plugin_argument_construction_benchmark.rs](../../../../crates/psionic/crates/psionic-train/src/psion_plugin_argument_construction_benchmark.rs) (psion_plugin_argument_construction_benchmark.rs:139, :675)
- [parameter_golf_submission_pr.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_submission_pr.rs) (parameter_golf_submission_pr.rs:1411, :1564, :1594, :1835)

**Evidence:** In `psion_plugin_argument_construction_benchmark.rs:675-680`, `fn repo_root()` is `env!("CARGO_MANIFEST_DIR").parent().parent().expect(...)`. Its result feeds the public function `psion_plugin_argument_construction_benchmark_bundle_path()` at :139. `parameter_golf_submission_pr.rs:1564` and `:1594` run `Command::new("bash").arg(repo_root().join("scripts/check-parameter-golf-record-folder-compatibility.sh"))` and a replay script, but `crates/psionic/scripts` does not exist. Line `:1411` runs `python3 train_gpt.py`. 176 files define repo_root/workspace_root helpers.

**Impact:** Public path APIs return paths from the build machine, which are wrong on any deployed host. The parameter-golf verifier steps always fail, though with a typed `CommandFailed` error rather than a panic. The `.expect` on `parent()` cannot actually fire for a crate directory, so the risk of a panic in shipped code is low.

**Suggested action:**
1. In non-test code, replace `repo_root()` with an explicit `root: &Path` argument (or a `PsionicTrainPaths` struct). Bins fill it from a flag or from `PSIONIC_TRAIN_RUNTIME_ROOT`, which `main.rs` already supports.
2. Delete or port the two `bash` verifier calls in `parameter_golf_submission_pr.rs`. If they are kept, check that the script exists first and return a `ScriptMissing` error when it does not.
3. Limit `CARGO_MANIFEST_DIR` to `#[cfg(test)]` code.
4. Verify: `rg 'CARGO_MANIFEST_DIR' crates/psionic/crates/psionic-train/src` only matches inside test modules.

### PS1-05 psionic-train dirty-tree check covers the whole monorepo and crates/psionic/target is not ignored

**Severity:** Medium · **Category:** correctness · **Effort:** S

**Locations:**
- [psionic-train/src/main.rs](../../../../crates/psionic/crates/psionic-train/src/main.rs) (main.rs:1201, :1251, :1320)
- [tests/psionic_train_cli.rs](../../../../crates/psionic/crates/psionic-train/tests/psionic_train_cli.rs) (psionic_train_cli.rs:41)
- [.gitignore](../../../../.gitignore) (.gitignore:1)

**Evidence:** `dirty_tree_posture` (main.rs:1320-1338) runs `git status --porcelain` in `repo_root`. Unless `--allow-dirty-tree` is passed, it fails with "dirty working trees are refused by default". `repo_root` resolves to `crates/psionic`: main.rs:1251-1258 checks for `Cargo.toml`, `crates/psionic-train/src/main.rs` and `train_runtime.rs`, which all exist there. But `git status` run from that directory reports on the entire openagents repo. `git check-ignore crates/psionic/target` finds no matching rule. `.gitignore` lists `/target` and `/crates/openagents-mobile/target/` but not `crates/psionic/target`, and the session git status shows `?? crates/psionic/target/`.

**Impact:** Any unrelated dirty or untracked file anywhere in the monorepo, including psionic's own build directory, makes manifest mode refuse to run without `--allow-dirty-tree`. Revision attestation also records the openagents HEAD, not an identity for the psionic tree.

**Suggested action:**
1. Add `/crates/psionic/target/` to the root `.gitignore`.
2. Limit the check to psionic: run `git status --porcelain -- .` from `crates/psionic`.
3. Attest `git rev-parse HEAD:crates/psionic` alongside HEAD, and update the CLI test helper `git_head()` (`tests/psionic_train_cli.rs:41`) to match.
4. Verify: with an unrelated dirty file elsewhere in the repo, manifest mode runs without `--allow-dirty-tree`.

### PS1-06 Large dead or single-consumer module set with no changes since import

**Severity:** Medium · **Category:** dead-code · **Effort:** M

**Locations:**
- [rl_run_service.rs](../../../../crates/psionic/crates/psionic-train/src/rl_run_service.rs)
- [benchmarking.rs](../../../../crates/psionic/crates/psionic-train/src/benchmarking.rs)
- [reliability.rs](../../../../crates/psionic/crates/psionic-train/src/reliability.rs)
- [security_posture.rs](../../../../crates/psionic/crates/psionic-train/src/security_posture.rs)
- [curriculum.rs](../../../../crates/psionic/crates/psionic-train/src/curriculum.rs)
- [coordinator_m8_head_to_head.rs](../../../../crates/psionic/crates/psionic-train/src/coordinator_m8_head_to_head.rs)
- [coordinator_live_buymode_dispatch.rs](../../../../crates/psionic/crates/psionic-train/src/coordinator_live_buymode_dispatch.rs)
- [lib.rs](../../../../crates/psionic/crates/psionic-train/src/lib.rs)

**Evidence:** The verifier scanned every `.rs` file under `crates/psionic/crates` for each module's top-level pub names. These modules are referenced only in their own file and in `lib.rs`: rl_run_service (2,103 lines), benchmarking (1,725), reliability (1,237), security_posture (1,047), curriculum (824), coordinator_m8_head_to_head (1,242) and coordinator_live_buymode_dispatch. `git log -- crates/psionic/crates/psionic-train` shows only `737f94a17c`. One correction to the reviewer's list: `psion_reference_pilot` was listed as used only by a fixture writer, but at least 4 examples reference it, including `examples/psion_actual_pretraining_operator.rs`, which `main.rs` compiles into the psionic-train binary via `#[path]`.

**Impact:** Unreferenced modules cost compile time and review time. They also carry latent security surface, such as the buy-mode dispatch with forgeable signatures (PS1-03).

**Suggested action:**
1. Delete the zero-reference modules and their `mod` and `pub use` lines in `lib.rs`. Their history remains upstream.
2. For modules used only by a single fixture-writer bin or example, decide per family: put them behind a feature (for example `lanes-swarm`) or delete them together with the bin. Do not treat `psion_reference_pilot` as dead, because the main CLI reaches it.
3. Re-run the reference scan after each pass, and confirm that `cargo check -p psionic-serve -p psionic-research` still passes.

### PS1-07 Flat 361-module crate with glob re-exports and session logic living in lib.rs

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [lib.rs](../../../../crates/psionic/crates/psionic-train/src/lib.rs) (lib.rs:1, :401, :754, :961)

**Evidence:** `lib.rs` is 1,716 lines. It has 362 `mod` declarations and 361 `pub use x::*;` lines, and it also contains the `TrainingSessionError` / `TrainingSessionState` logic.

**Impact:** Every public item becomes part of the crate's API, new names can collide through the globs, and the code does not show which lane owns which module.

**Suggested action:**
1. Move the session and recovery code out of `lib.rs` into `src/session.rs`.
2. Group modules into family directories (`psion/`, `tassadar/`, `parameter_golf/`, `qwen_legal/`, `compiled_agent/`, `cs336/`, `swarm/`, `coordinator/`) with explicit re-exports.
3. Re-export only the stable substrate at the crate root, by name.
4. Update the imports in psionic-serve and psionic-research.
5. Verify: `rg -c 'pub use .*::\*;' src/lib.rs` returns 0 (or close to it), and dependent crates still build.

### PS1-08 Huge files and functions: 8.7k-line training file and ~1,000-line functions

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [parameter_golf_single_h100_training.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_single_h100_training.rs) (parameter_golf_single_h100_training.rs:2123)
- [parameter_golf_distributed_8xh100_train_step.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_distributed_8xh100_train_step.rs) (parameter_golf_distributed_8xh100_train_step.rs:3695)
- [examples/psion_actual_pretraining_operator.rs](../../../../crates/psionic/crates/psionic-train/examples/psion_actual_pretraining_operator.rs) (psion_actual_pretraining_operator.rs:426)

**Evidence:** `run_with_args` starts at `examples/psion_actual_pretraining_operator.rs:426` (1,813 lines by the brace heuristic). `build_parameter_golf_single_h100_training_report_inner` starts at :2123 in an 8,736-line file. `execute_parameter_golf_distributed_8xh100_train_step` starts at :3695. The verifier counted 50 `#[allow(clippy::too_many_arguments)]` across src and examples; the reviewer had counted 45.

**Impact:** Training and report builders close to 1,000 lines long cannot be unit-tested piece by piece or reviewed for ordering and numeric bugs.

**Suggested action:**
1. Split the single-H100 builder into stage functions with stage structs: `load_inputs`, `build_graphs`, `run_steps`, `evaluate`, `assemble_report`.
2. Split the 8xH100 train step the same way: `bootstrap`, `exchange`, `apply`, `receipt`.
3. Replace `too_many_arguments` sites with parameter structs. Start with the training paths only.
4. Verify: report outputs and digests are byte-identical before and after for the existing tests, and no function in the touched files exceeds ~300 lines.

### PS1-09 Digest and repo-root helpers copied hundreds of times

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [swarm_trusted_lan.rs](../../../../crates/psionic/crates/psionic-train/src/swarm_trusted_lan.rs) (swarm_trusted_lan.rs:248)
- [reward_ledger_contract.rs](../../../../crates/psionic/crates/psionic-train/src/reward_ledger_contract.rs) (reward_ledger_contract.rs:509)

**Evidence:** 189 psionic-train src files define a top-level `fn stable_digest`, and 176 files define repo_root/workspace_root helpers.

**Impact:** The copies drift apart in how they handle errors. Changing digest canonicalization would take hundreds of coordinated edits.

**Suggested action:**
1. Add `psionic_core::digest::stable_json_digest<T: Serialize>(prefix, value) -> Result<String, serde_json::Error>`.
2. Pin one known output digest in a test before migrating, so fixture digests stay byte-identical.
3. Codemod the copies one module family at a time, replacing each local copy with the shared helper.
4. Verify: `rg -l 'fn stable_digest' crates/psionic/crates/psionic-train/src | wc -l` falls to 0, and the pinned digest test still passes.

### PS1-10 Workspace lints deny expect/panic, but non-test library code has hundreds of .expect calls

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml) (Cargo.toml:41)
- [adapter_window.rs](../../../../crates/psionic/crates/psionic-train/src/adapter_window.rs) (adapter_window.rs:608)
- [psion_google_single_node_visualization.rs](../../../../crates/psionic/crates/psionic-train/src/psion_google_single_node_visualization.rs) (psion_google_single_node_visualization.rs:588)

**Evidence:** `crates/psionic/Cargo.toml:41-43` sets `unwrap_used`, `expect_used` and `panic` to `"deny"`. `adapter_window.rs:607-608` sets `contribution.execution = Some(receipt);` and then calls `.expect("execution inserted")`. `psion_google_single_node_visualization.rs:588` calls `self.shared.lock().expect("writer shared state should lock")`. Non-test library code has 449 `.expect(` calls, 195 of them on serialization in digest helpers.

**Impact:** The crate cannot pass its own clippy profile, so in practice clippy is not enforced. A poisoned mutex or a broken bookkeeping invariant aborts a run.

**Suggested action:**
1. Make the shared digest helper from PS1-09 return a `Result`. That removes most of the expects.
2. In `adapter_window.rs`, replace set-then-expect with `Ok(contribution.execution.insert(receipt))`; `Option::insert` returns `&mut T`.
3. Map mutex poisoning to a typed error.
4. Run `cargo clippy -p psionic-train --lib` in a dedicated lane. Verify that it passes with no new `allow`s.

### PS1-11 CLI classifies errors by substring, maps lane errors to BadConfig, and compiles a 5.2k-line example into the main binary

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [psionic-train/src/main.rs](../../../../crates/psionic/crates/psionic-train/src/main.rs) (main.rs:44, :132, :1863)

**Evidence:** `classify_operator_error` (main.rs:1863-1892) lowercases the error's Display text and matches substrings such as "requires --", "launch refused preflight admission", "checkpoint manifest ref drifted" and "failed-upload drill". `main.rs` has 12 uses of `BadConfig.exit_code()`, and `run_legal_ft_passthrough` (132+) maps every `Err` to BadConfig. Lines 44-46 compile the example into the binary with `#[allow(dead_code)] #[path = "../examples/psion_actual_pretraining_operator.rs"] mod ...`.

**Impact:** Rewording an error message changes the exit code, which callers depend on. Runtime failures are reported as bad configuration. Production logic is hidden under `examples/`.

**Suggested action:**
1. Give each lane error type a `refusal_class()` method and delete `classify_operator_error`.
2. Replace the passthrough functions with one generic helper that uses `refusal_class()`.
3. Move the operator example into `src/psion_actual_pretraining_operator/` and remove the `#[path]` include.
4. Verify: `tests/psionic_train_cli.rs` exit-code assertions still pass, and `rg 'to_lowercase' src/main.rs` no longer appears in error classification.

### PS1-12 Hand-rolled TCP framing: one byte per read, unbounded line buffer, peer-chosen allocation size

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [swarm_first_live_runtime.rs](../../../../crates/psionic/crates/psionic-train/src/swarm_first_live_runtime.rs) (swarm_first_live_runtime.rs:1946)
- [psion_google_two_node_swarm_runtime.rs](../../../../crates/psionic/crates/psionic-train/src/psion_google_two_node_swarm_runtime.rs) (psion_google_two_node_swarm_runtime.rs:1478)
- [parameter_golf_distributed_8xh100_train_step.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_distributed_8xh100_train_step.rs) (parameter_golf_distributed_8xh100_train_step.rs:979)

**Evidence:** `swarm_first_live_runtime.rs:1946-1975` reads one byte per call into `[0_u8; 1]` and appends to an unbounded `Vec` until it sees `'\n'`. `8xh100_train_step.rs:984-987` reads a `byte_len` supplied by the peer and allocates `vec![0_u8; byte_len]` before checking it against `value_count`.

**Impact:** Large messages are very slow to read, and a stray or hostile peer can force huge allocations and run the process out of memory.

**Suggested action:**
1. Add a shared framing module: length-prefixed frames, a `MAX_FRAME_BYTES` limit, and a `BufReader`.
2. Before allocating, check `byte_len == value_count * 4 && byte_len <= MAX`.
3. Add tests for an over-limit frame and for a stream with no newline. Both must return an error without a large allocation.

### PS1-13 Tests mutate process env in parallel and a test writes fixtures into the source tree

**Severity:** Medium · **Category:** concurrency · **Effort:** S

**Locations:**
- [parameter_golf_distributed_8xh100_train_step.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_distributed_8xh100_train_step.rs) (parameter_golf_distributed_8xh100_train_step.rs:5626)
- [parameter_golf_single_h100_visualization.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_single_h100_visualization.rs) (parameter_golf_single_h100_visualization.rs:1789)
- [tassadar_default_train_rehearsal.rs](../../../../crates/psionic/crates/psionic-train/src/tassadar_default_train_rehearsal.rs) (tassadar_default_train_rehearsal.rs:661)

**Evidence:** Around `8xh100_train_step.rs:5626`, a test calls `unsafe { env::set_var(VALIDATION_BATCH_SEQUENCES_ENV_VAR, "256") }` and `remove_var` at the end, while an adjacent test reads the default value. `visualization.rs:1789-1796` calls `set_var` and `remove_var` on `RUNPOD_POD_ID_ENV`. `tassadar_default_train_rehearsal.rs:661` calls `write_tassadar_default_train_rehearsal_fixtures(workspace_root())`.

**Impact:**
- Tests run in parallel and race on the process environment, so results are flaky.
- The fixture-writer test dirties the checkout.
- Whether sibling tests see the missing fixtures depends on test order, which hides the PS1-02 failures in some runs.

**Suggested action:**
1. Pass these config values through structs instead of reading the environment inside the logic.
2. Where an env test must remain, guard it with a static `Mutex` plus a drop guard that restores the previous value.
3. Point the fixture-writer test at `tempfile::tempdir()`.
4. Verify: `git status` is clean after `cargo test -p psionic-train --lib`, and the tests pass with `--test-threads=16` across repeated runs.

### PS1-14 Contract documents as Rust builders plus 97 near-identical writer bins

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [reward_ledger_contract.rs](../../../../crates/psionic/crates/psionic-train/src/reward_ledger_contract.rs) (reward_ledger_contract.rs:344)
- [bin/reward_ledger_contract.rs](../../../../crates/psionic/crates/psionic-train/src/bin/reward_ledger_contract.rs)

**Evidence:** 35 `*_contract*.rs` files total 26,120 lines. `src/bin` has 97 files, and `bin/reward_ledger_contract.rs` is 24 lines. `canonical_reward_ledger_contract` (line 344) is not a flat literal: it is built from other canonical contracts (network, identities, scoring, consensus, fraud).

**Impact:** Editing contract data requires a rebuild, and the bins add build time. Many builders derive from and cross-check each other, so moving them all to plain JSON would lose those derivations.

**Suggested action:**
1. Replace the one-line writer bins with a single `psionic-train fixtures write <contract-id> <out>` subcommand backed by a static `(id, fn)` table, then delete the bins.
2. Convert only the pure-literal builders (for example `canonical_supervision_scenarios`) to `include_str!` JSON.
3. Verify: the subcommand's output is byte-identical to each old bin's output for every contract id.

### PS1-15 Example and test copies of the same benchmark package builder

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [examples/psion_benchmark_package_fixtures.rs](../../../../crates/psionic/crates/psionic-train/examples/psion_benchmark_package_fixtures.rs) (psion_benchmark_package_fixtures.rs:137)
- [psion_benchmark_packages.rs](../../../../crates/psionic/crates/psionic-train/src/psion_benchmark_packages.rs) (psion_benchmark_packages.rs:2452)

**Evidence:** Both files define `fn package_contracts()` at the cited lines, and both return `Vec<PsionBenchmarkPackageContract>`.

**Impact:** The fixture writer and the test contracts can drift apart without anything failing.

**Suggested action:**
1. Move `package_contracts` into `src/psion_benchmark_packages.rs` as `#[doc(hidden)] pub fn canonical_psion_benchmark_package_contracts()`.
2. Call it from both the example and the tests.
3. Delete the two local copies.

### PS1-16 Feature-gated legacy Apple toolkit module that no build ever compiles

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:**
- [psionic-train/Cargo.toml](../../../../crates/psionic/crates/psionic-train/Cargo.toml) (Cargo.toml:22)
- [lib.rs](../../../../crates/psionic/crates/psionic-train/src/lib.rs) (lib.rs:38, :401)
- [apple_toolkit.rs](../../../../crates/psionic/crates/psionic-train/src/apple_toolkit.rs) (apple_toolkit.rs:1)

**Evidence:** `rg` finds `legacy-apple-toolkit-oracle` only at `Cargo.toml:22`, `lib.rs:38`, `lib.rs:401` and in a doc comment at `apple_toolkit.rs:5`.

**Impact:** The module is never compiled or tested, so it rots without anyone noticing.

**Suggested action:**
1. Delete `apple_toolkit.rs`, the feature, and the two `cfg` lines in `lib.rs`.
2. Verify: `rg legacy-apple-toolkit-oracle crates/psionic` returns nothing and the crate still builds.

### PS1-17 Stale references and placeholder README

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [psionic-train/README.md](../../../../crates/psionic/crates/psionic-train/README.md)
- [psionic-train/src/main.rs](../../../../crates/psionic/crates/psionic-train/src/main.rs) (main.rs:110)

**Evidence:** `README.md` is 3 lines and names only the import commit. Many `docs/*.md` literals point to docs that were not imported; they are part of the 621 missing paths.

**Impact:** Readers cannot tell which lanes are live and which are archival, and generated receipts reference documents that do not exist.

**Suggested action:**
1. Write a README with:
   - a map of the module families,
   - the live CLI entrypoints,
   - the items psionic-serve consumes,
   - the fixture status (from PS1-02).
2. Qualify upstream issue references as `OpenAgentsInc/psionic#N`.

### PS1-18 Cargo hygiene: inline dependency versions bypass the workspace

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [psionic-train/Cargo.toml](../../../../crates/psionic/crates/psionic-train/Cargo.toml)
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml) (Cargo.toml:97)

**Evidence:** psionic-train declares `rayon = "1.10"`, while the workspace declares `rayon = "1"` (Cargo.toml:97). `half = "2.4.1"`, `flate2 = "1.0"` and `zstd = "0.13"` are also declared inline. The CUDA and Metal backends and blocking `reqwest` are unconditional dependencies.

**Impact:** Dependency versions can drift between crates, and consumers that only need types still build the GPU and HTTP stacks. psionic-serve already depends on the CUDA and Metal backends directly, so the practical gain is for psionic-research and future slim consumers.

**Suggested action:**
1. Use `{ workspace = true }` for `rayon`.
2. Add `half`, `flate2` and `zstd` to `[workspace.dependencies]` and reference them from the crate.
3. Put `psionic-backend-cuda`, `psionic-backend-metal` and `reqwest` behind features, and have the bins enable them through `required-features`.
4. Verify: `cargo tree -p psionic-research -e normal` no longer lists the GPU backends or reqwest via psionic-train.

### PS1-19 crates/psionic/README.md misstates the imported fixture set (says 23 files, 211 exist)

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [crates/psionic/README.md](../../../../crates/psionic/README.md)
- [crates/psionic/fixtures](../../../../crates/psionic/fixtures)

**Evidence:** The README says "`fixtures/` holds only the 23 files those crates embed with `include_str!`", but `find crates/psionic/fixtures -type f | wc -l` returns 211.

**Impact:** The README is the only map of what was imported. The wrong count misleads anyone sorting out which of the 621 missing fixture references were brought in and which were not.

**Suggested action:**
1. Correct the count.
2. State which fixtures are embedded with `include_str!` and which are read at test time.
3. Add the fixture-status table proposed in PS1-02.

## Refuted during verification

- **"Money, ledger and settlement logic lives in a training crate that the serving binary links."** Rejected for three reasons:
  - `coordinator_http_buymode_dispatch.rs` is a client that does nothing unless `PSIONIC_BUY_MODE_HTTP_ARM=armed` is set. It calls the OpenAgents Worker, and its doc states that the Worker remains the spend authority. It does not create a second spend authority.
  - The reward and settlement modules are contract and evidence documents.
  - The remaining concern, lane code being linked into serve, is already covered by PS1-01 and PS1-06.
- **Claims adjusted during verification (do not re-raise in their original form):**
  - **Dirty-tree check (PS1-05).** It does not make the CLI "refuse to run in practice", because `--allow-dirty-tree` exists. Repo-root detection does not expect the old layout: `crates/psionic` satisfies it.
  - **Fixture-writer-only aggregate (PS1-06).** The 62-module, 56.5k-line figure includes at least one wrong entry: `psion_reference_pilot` is used by the main binary.
  - **Panics when shipped (PS1-04).** The `CARGO_MANIFEST_DIR` `.expect` on `parent()` cannot realistically panic.
  - **GPU dependencies (PS1-01, PS1-18).** psionic-serve does not inherit them through psionic-train, because it already depends on the CUDA and Metal backends directly.
