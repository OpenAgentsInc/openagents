# Psionic: serving, runtime, models

**Scope [PS3]:** `crates/psionic/crates/{psionic-serve, psionic-runtime, psionic-models, psionic-router, psionic-provider, psionic-catalog, psionic-apple-fm}`. Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`. Several psionic-serve and psionic-backend-cuda files were dirty in the working tree (another agent's uncommitted work), so this review uses the committed HEAD versions.

**Health grade: D**

These seven crates hold about 368.5k lines of Rust in 572 tracked `.rs` files. They were imported in one commit from `OpenAgentsInc/psionic` on 2026-10-07 (`737f94a17c`). Line-level hygiene is good. The workspace lints deny `unwrap`, `expect`, `panic`, printing and `exit`. There is almost no TODO debt and errors are typed with `thiserror`. The server binds to loopback by default, the coordination store is bounded, HTML output is escaped and Ollama digests are checked strictly.

The structure is poor. About 130k lines in these crates are Tassadar "article / post_article / publication" research code, which `psionic-openai-server` never calls. To support that code and some `psion_*` evidence modules, `psionic-serve` links `psionic-train`, `psionic-eval` and `psionic-research`, about 630k lines in total. The import kept the tests but dropped their fixtures: 316 of the 351 fixture files the tests reference are missing, so tests that read fixtures fail every time. Nothing builds or tests `crates/psionic` automatically. The hot serving files are very large: `openai_http.rs` (25.3k lines, with two parallel OpenAI servers), `gguf.rs` (20.5k), `qwen35.rs` (17.1k) and `gpt_oss.rs` (15.5k, with one 2,654-line function and a file-wide `expect_used` allow). Risks on the live path:

- Unbounded worker queues.
- A shared-key check on the internal Gemma4 pipeline that allows every request when no key is set.
- An unchecked Accelerate vDSP call that can read out of bounds on a malformed GGUF.
- Allocations sized from counts in the GGUF file.
- Dozens of undocumented `PSIONIC_*` environment variables.

## Measurements

| Metric | Value |
|---|---|
| Tracked `.rs` LOC at HEAD | serve 146,994 (135 files: 66 src, 57 examples, 12 tests); runtime 97,595 (167 files, 68 examples); models 71,847 (82); router 16,630 (52); provider 21,448 (117); catalog 7,265; apple-fm 6,770. **Total 368,549** |
| Tassadar code | runtime 87 of 99 src files / 70,320 LOC; models 47 of 76 / 21,682; serve 25 of 66 / 16,556; router 27 of 30 / 11,784; provider 112 of 113 / 10,512. About 130k LOC. `git ls-files crates/psionic \| grep -c tassadar` = 1274 |
| Largest files | serve `openai_http.rs` 25,289; `gguf.rs` 20,498; `qwen35.rs` 17,144 (HEAD); `gpt_oss.rs` 15,459; models `lib.rs` 15,427; runtime `lib.rs` 14,927; serve `lib.rs` 14,491; runtime `tassadar.rs` 13,233; provider `lib.rs` 10,878 |
| Largest functions (HEAD) | `gpt_oss.rs` `forward_step_with_output_mode` 2,654; `qwen35.rs` `forward_token_fused` 1,600; `gguf.rs` `forward_local_step_with_layer_caches` 1,131; models `build_gguf_decoder_tensor_layout` 1,108; `openai_http.rs` `handle_generic_responses` 498 |
| `#[test]` counts | serve 478, runtime 495, models 386, router 101, provider 197, catalog 32, apple-fm 49. `qwen35.rs` has 5 and `gpt_oss.rs` has 22 |
| `expect`/`unwrap` (all code) | serve 624, runtime 714, models 506, router 189, provider 153. Before the first `cfg(test)`: serve 103, runtime 55, router 8. `gpt_oss.rs` has 23 `expect("checked above")` |
| `#[allow]` | serve 30, runtime 29, models 10 |
| `unsafe` mentions | serve 25, runtime 8, models 4, catalog 2. Serve and catalog have 3 `SAFETY` comments between them |
| TODO/FIXME | 1 |
| Glob re-exports (`pub use x::*`) in `lib.rs` | serve 54, runtime 98, models 75, router 27, provider 112 (total 366) |
| `PSIONIC_*` env vars | 63 distinct identifiers at HEAD. None appears in `docs/psionic` or `crates/psionic/README.md` |
| Fixture references | 415 distinct `fixtures/...` literals, 351 of them file paths. 316 of those files are missing, 300 of them under `fixtures/tassadar`. About 130 test and helper functions have "committed" in the name |
| Activity since import | 2 commits touched serve and models. Runtime, router, provider, catalog and apple-fm have only the import commit |
| Build and CI | `crates/psionic/target` (448 MB) is not git-ignored. The repo has no `.github/workflows`. `scripts/verify-changed.py` does not cover psionic. No `Cargo.toml` outside `crates/psionic` depends on psionic |

## Strengths

- The workspace lint policy in `crates/psionic/Cargo.toml` is strict. It denies `unwrap_used`, `expect_used`, `panic`, `print_stdout`/`print_stderr`, `exit`, `todo`, `dbg_macro` and `await_holding_lock`. Tests opt out through `cfg_attr(test, allow(...))`. Keep this policy and stop adding file-wide allows.
- There is almost no TODO/FIXME debt (1 in 368k lines). Errors are `thiserror` enums, and HTTP errors map to `OpenAiCompatHttpError` instead of generic 500 responses.
- Network defaults are safe. `OpenAiCompatConfig::new` and `psionic-openai-server` bind `127.0.0.1` (`openai_http.rs:1011`, `psionic-openai-server.rs:184`). The README run line uses `--mesh-coordination disabled`.
- The mesh coordination store is bounded: `MESH_COORDINATION_MAX_ITEMS=500`, `MAX_BODY_BYTES=4096` and TTL pruning (`openai_http.rs:151-155`, `1896-1904`). A test covers the cap (`openai_http.rs:14098-14118`).
- The management console escapes HTML (`html_escape`, `openai_http.rs:5917`), and the inline JSON bootstrap neutralizes `</` (`openai_http.rs:5938-5940`).
- `psionic-catalog` validates Ollama digests strictly (sha256, exactly 64 hex characters, lowercased; `lib.rs:586-605`). It checks blob digests on read, and when mmap fails it falls back to a buffered read and records the reason.
- Hardware isolation is designed correctly. Pylon calls `psionic-openai-server` over HTTP instead of linking it, so CUDA faults stay out of the provider.
- Accelerator stacks sit behind features (`csm-cuda`, `medpsy-cuda`, `qwen38-vision-cuda`). When `nvcc` is missing, the CUDA build falls back to a stub loaded at runtime, so Mac and CPU builds still work.
- `openai_http.rs` has 135 unit tests covering the HTTP contract, tool calling, the coordination store and the proxy modes. No other serving surface is tested as well.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| PS3-01 | High | architecture | The serving library links ~630k lines of training, eval and research code it does not need | L |
| PS3-02 | High | testing | Fixture-reading tests fail: 316 of 351 referenced fixture files were not imported | M |
| PS3-03 | Medium | dead-code | About 130k lines of Tassadar research code sit in runtime, router, provider, models and serve, unused by the server | XL |
| PS3-04 | Medium | build | No automated check covers `crates/psionic` | M |
| PS3-05 | Medium | security | Out-of-bounds read: Accelerate `vDSP_vmul` uses `values.len()` without checking `weight.len()` | S |
| PS3-06 | Medium | security | GGUF parser sizes allocations from untrusted counts | S |
| PS3-07 | Medium | concurrency | Unbounded worker queues with no admission control, timeouts or 429/503 backpressure | M |
| PS3-08 | Medium | security | Unauthenticated internal and management mutation routes | S |
| PS3-09 | Medium | maintainability | `openai_http.rs` is a 25k-line god file holding two parallel OpenAI servers | L |
| PS3-10 | Medium | maintainability | Forward-pass functions thousands of lines long, with few tests, in `qwen35.rs` and `gpt_oss.rs` | L |
| PS3-11 | Medium | error-handling | `gpt_oss.rs` allows `expect_used` file-wide and enforces invariants with 23 `expect("checked above")` calls | M |
| PS3-12 | Medium | architecture | `psionic-models/src/lib.rs` (15.4k lines) mixes descriptors, GGUF parsing, tokenizers, templates and MedPsy policy | L |
| PS3-13 | Low | maintainability | 63 undocumented `PSIONIC_*` env vars, some read per layer or per token | M |
| PS3-14 | Low | maintainability | Serving library ships hard-coded benchmark numbers as "evidence" | M |
| PS3-15 | Low | security | Fetch-text plugin URL allowlist is a string-prefix check, and redirects skip it (code currently unreachable) | S |
| PS3-16 | Low | maintainability | 366 glob re-exports make the crate APIs unbounded | M |
| PS3-17 | Low | duplication | `rms_norm` and related kernels are copied across serve modules | M |
| PS3-18 | Low | build | `psionic-models` always links candle and moshi | M |
| PS3-19 | Low | repo-hygiene | `crates/psionic/target` (448 MB) is not git-ignored | S |
| PS3-20 | Low | maintainability | Four server binaries parse arguments by hand, duplicating defaults from the config structs | S |

### PS3-01 The serving library links ~630k lines of training, eval and research code it does not need

**Severity:** High · **Category:** architecture · **Effort:** L

**Locations:**
- [psionic-serve/Cargo.toml](../../../../crates/psionic/crates/psionic-serve/Cargo.toml)
- [gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (gguf.rs:34-37)
- [qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs) (qwen35.rs:36)
- [psion_capability_withdrawal.rs](../../../../crates/psionic/crates/psionic-serve/src/psion_capability_withdrawal.rs)
- [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (openai_http.rs:103-105)
- [crates/psionic/README.md](../../../../crates/psionic/README.md) (README.md:24)

**Evidence:** At HEAD, the `[dependencies]` of `psionic-serve` include `psionic-train` (417,177 LOC), `psionic-eval` (162,211) and `psionic-research` (49,757). README line 24 says the workspace is "about a million lines, mostly training and evaluation code that psionic-serve links". Outside the `tassadar_*` modules, serve `src` uses these crates in only a few places:
- `gguf.rs:34-37` uses `GemmaE4bCudaAdapterCheckpoint`, `ExportedArtifact` and `ServedBaseModelBinding`.
- `qwen35.rs:36` uses `load_qwen38_lm_head_adapter_safetensors`.
- The `psion_*` evidence modules use them: `psion_capability_withdrawal.rs` (11 references), and `psion_capability_matrix`, `psion_generic_load_and_generate`, `psion_served_evidence` and `psion_served_output_claim_posture` (one each).
- The examples and tests `parameter_golf_*`, `xtrain_*` and `promoted_parameter_golf_*` use them.

`openai_http.rs` reaches Tassadar only through `mod tassadar_post_article_router_plugin_tool_loop_pilot` (103-105).

**Impact:** Every `psionic-openai-server` build compiles ~630k lines of unrelated code, including the CUDA release build that `scripts/pylon-psionic.sh:84` runs. Build time, link time and supply-chain surface all grow, and a compile break in research code blocks the server build.

**Suggested action:**
1. Move the two adapter loaders that serving needs (the `GemmaE4bCudaAdapter*` types and `load_qwen38_lm_head_adapter_safetensors`) from `psionic-train` into `psionic-models` or `psionic-adapters`.
2. Delete the following, or put them behind a non-default `research` feature: the 25 serve `tassadar_*` modules, the `psion_*` evidence/withdrawal/claim modules, `openai_http/tassadar_post_article_router_plugin_tool_loop_pilot.rs`, and the parameter_golf/xtrain examples and tests.
3. Make `psionic-train`, `psionic-eval` and `psionic-research` optional dependencies under that feature.
4. Verify: `cargo tree --manifest-path crates/psionic/Cargo.toml -p psionic-serve -e normal | grep -c psionic-train` returns 0. Record `cargo build --timings` for the `psionic-openai-server` bin before and after.

### PS3-02 Fixture-reading tests fail: 316 of 351 referenced fixture files were not imported

**Severity:** High · **Category:** testing · **Effort:** M

**Locations:**
- [psion_plugin_guest_artifact_invocation.rs](../../../../crates/psionic/crates/psionic-runtime/src/psion_plugin_guest_artifact_invocation.rs) (psion_plugin_guest_artifact_invocation.rs:25-26, 1101-1107)
- [crates/psionic/fixtures](../../../../crates/psionic/fixtures)

**Evidence:** The seven crates reference 415 distinct `"fixtures/..."` literals, 351 of them file paths. 316 of those 351 files do not exist under `crates/psionic/fixtures`, and 300 of the missing files are under `fixtures/tassadar`. Example: `committed_guest_artifact_invocation_fixture_validates` (1101-1107) reads `PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF = fixtures/psion/plugins/guest_artifact/psion_plugin_guest_artifact_invocation_v1.json`, and that file is missing. Some of the constants are output paths that writers create, but read tests like this one fail.

**Impact:** `cargo test` over these crates cannot pass, so real serving regressions are hidden among failures everyone expects.

**Suggested action:**
1. After the prune (PS3-01/PS3-03), list the remaining failures: `cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-runtime -p psionic-serve -p psionic-models --no-fail-fast 2>&1 | grep FAILED`.
2. Copy the needed fixtures from upstream at the import commit, or regenerate them with the matching example.
3. Delete the tests for modules that were removed.
4. Add a test asserting that every exported `*_FIXTURE_PATH`/`*_REF` constant that a test reads points to an existing file. Verify that the command in step 1 reports no failures.

### PS3-03 About 130k lines of Tassadar research code sit in runtime, router, provider, models and serve, unused by the server

**Severity:** Medium · **Category:** dead-code · **Effort:** XL

**Locations:**
- [psionic-runtime/src/lib.rs](../../../../crates/psionic/crates/psionic-runtime/src/lib.rs) (lib.rs:1-15)
- [psionic-runtime/src/tassadar.rs](../../../../crates/psionic/crates/psionic-runtime/src/tassadar.rs)
- [psionic-router/src/lib.rs](../../../../crates/psionic/crates/psionic-router/src/lib.rs)
- [psionic-provider/src/lib.rs](../../../../crates/psionic/crates/psionic-provider/src/lib.rs)
- [psionic-serve/Cargo.toml](../../../../crates/psionic/crates/psionic-serve/Cargo.toml)

**Evidence:** Tassadar share of each crate's `src` at HEAD:

| Crate | Files | LOC |
|---|---|---|
| runtime | 87 of 99 | 70,320 |
| router | 27 of 30 | 11,784 |
| provider | 112 of 113 | 10,512 |
| models | 47 of 76 | 21,682 |
| serve | 25 of 66 | 16,556 |

Since the import, runtime, router and provider have no commits besides `737f94a17c`. Nothing outside `crates/psionic` depends on a psionic crate. Inside the psionic workspace the code does have consumers. Files that reference `psionic_runtime` Tassadar items: eval 56, research 17, models 13, serve 12, provider 11, train 9, compiler 8.

**Impact:** About a third of the area is unmaintained research scaffolding. It adds noise to searches and reviews, pulls in heavy dependencies (wasmi, wasm-encoder, scraper, roxmltree, blocking reqwest) and hides the small runtime core that serving uses. This is maintenance cost, not a live defect.

**Suggested action:**
1. Prune Tassadar from the monorepo copy; upstream `OpenAgentsInc/psionic` keeps it. Do it as one coordinated change: delete the `tassadar*` modules in runtime, router, models and serve together with their dependents in `psionic-eval`, `psionic-research`, `psionic-train` and `psionic-compiler`. Where one of those crates exists only for research, remove it from the monorepo workspace members.
2. Shrink `psionic-provider` to the `CapabilityEnvelope`, `ExecutionReceipt` and `ReceiptStatus` types used by 7 serve tests, or move those types into `psionic-serve/tests/support`.
3. Remove the deps that become unused (`wasmi`, `wasm-encoder`, `wasmparser`, `scraper`, `roxmltree`) from `psionic-runtime/Cargo.toml`.
4. Record the prune in `crates/psionic/README.md`.
5. Verify: `cargo build --bin psionic-openai-server` and `cargo test -p psionic-serve` both pass.

### PS3-04 No automated check covers crates/psionic

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:4-10)
- [scripts/verify-changed.py](../../../../scripts/verify-changed.py)
- [scripts/pylon-psionic.sh](../../../../scripts/pylon-psionic.sh) (pylon-psionic.sh:84)
- [AGENTS.md](../../../../AGENTS.md) (AGENTS.md:812-816)

**Evidence:** The root `Cargo.toml` excludes `crates/psionic` (lines 4-10), so cargo commands run from the root never touch it. The repo has no `.github/workflows`; `.github` holds only `ISSUE_TEMPLATE/playtest-report.yml`. `scripts/verify-changed.py` never mentions psionic. The only automated build is the release build in `scripts/pylon-psionic.sh:84` when Pylon starts. Everything else is manual instructions in AGENTS.md and the README. The repo has no CI by design, so the gap that matters is the repo's own `verify-changed` gate.

**Impact:** Changes to the model server that Pylon publishes get no test or lint signal. This is why the broken fixture tests and the growing lint exceptions went unnoticed.

**Suggested action:**
1. Teach `scripts/verify-changed.py` to map a change under `crates/psionic/crates/<crate>` to `cargo test --manifest-path crates/psionic/Cargo.toml -p <crate>`: CPU only, with a persistent target dir per the lean-verification policy.
2. Have it also run `cargo check --manifest-path crates/psionic/Cargo.toml --bin psionic-openai-server`.
3. Optional: add a smoke test that starts `psionic-openai-server` on a tiny GGUF and calls `GET /health` and `/v1/models`.
4. Verify: touch a psionic-serve file, run `verify-changed.py`, and confirm the psionic commands run.

### PS3-05 Out-of-bounds read: Accelerate vDSP_vmul uses values.len() without checking weight.len()

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (gguf.rs:16145-16179, 16211-16245, 1821-1828, 14977-14982)

**Evidence:** On macOS, `rms_norm_in_place` calls `vDSP_vmul(values.as_ptr(),1, weight.as_ptr(),1, values.as_mut_ptr(),1, values.len())` without checking the length of `weight`. `per_head_rms_norm_in_place` does the same with `head.len()`. The non-macOS branch uses `zip()`, which stops at the shorter slice. `load_dense_vector` (14977) never checks the tensor's length against `hidden_size` or `head_dim`, so each norm vector's length comes straight from the GGUF.

**Impact:** On macOS, a malformed or mismatched GGUF norm tensor causes an out-of-bounds read, which is undefined behavior, inside the server. On Linux, where production runs on the 4080, the same mismatch produces wrong output instead of a load error. The trigger is a malformed model file that the operator chose to load.

**Suggested action:**
1. Validate norm vector lengths once at load time. In the layer builder around gguf.rs:2191, check that each `load_dense_vector`/`load_optional_dense_vector` result (`attention_norm`, `attention_query_norm`, etc.) has length `hidden_size` or `head_dim`. Otherwise return `ModelLoadError::InvalidTensorShape`.
2. In `rms_norm_in_place` and `per_head_rms_norm_in_place`, add a `debug_assert` or an early return before the `unsafe` block.
3. Use a single `as_mut_ptr()` for both the input and output pointers, and add `SAFETY` comments.
4. Verify: add a unit test that loads a layer with a short norm weight and expects a load error.

### PS3-06 GGUF parser sizes allocations from untrusted counts

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs) (lib.rs:1620-1631, 1856-1863, 2129-2150)

**Evidence:**
- In the metadata array arm, `let length = version.read_count(reader)?; let mut values = Vec::with_capacity(length);` (1858-1859). `read_count` (1620-1631) only checks that the count fits in `usize`.
- The tensor info loop calls `Vec::with_capacity(dimension_count as usize)` on a raw `u32` (2149).
- `tensor_count` and `metadata_kv_count` are unbounded loop counts (2129-2130).

**Impact:** A corrupt or crafted GGUF aborts the process with a capacity overflow or allocation failure instead of returning a `ModelLoadError`.

**Suggested action:**
1. Cap preallocation at `length.min(remaining_bytes / min_element_size)`, or drop `with_capacity`.
2. Reject `dimension_count > 4` with `artifact_format_error`.
3. Reject a `tensor_count` or `metadata_kv_count` larger than the remaining bytes divided by the minimum record size.
4. Verify: add tests that feed headers with huge counts and assert `Err(ModelLoadError)` without aborting.

### PS3-07 Unbounded worker queues with no admission control, timeouts or 429/503 backpressure

**Severity:** Medium · **Category:** concurrency · **Effort:** M

**Locations:**
- [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (openai_http.rs:3934-3940, 4573-4576, 3882-3923)

**Evidence:** `mpsc::unbounded_channel()` at lines 3935 and 4573 feeds a single `std::thread` named `psionic-openai-worker`. A grep for `TimeoutLayer`, `ConcurrencyLimit`, `TOO_MANY`, `queue_depth` and `max_queue` finds nothing. `SERVICE_UNAVAILABLE` appears only on the llama.cpp proxy path (4806, 4867).

**Impact:** Under load, requests queue in memory without limit behind one thread. Requests whose clients have disconnected still run, and the server can neither shed load nor report that it is saturated.

**Suggested action:**
1. Replace the unbounded channel with `mpsc::channel(N)`, taking N from a new `OpenAiCompatConfig` field and a `--max-queue` flag.
2. Call `try_send` in the handlers and return 503 with `Retry-After` when the queue is full.
3. Report queue depth in `/health` and `/psionic/management/status`.
4. Before running a command, skip it if its response oneshot `is_closed()`.
5. Verify: add a test that fills the queue and asserts a 503.

### PS3-08 Unauthenticated internal and management mutation routes

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (openai_http.rs:1221-1225, 8710-8730, 3882-3923, 1619-1624, 1011-1016)

**Evidence:**
- `require_distributed_gemma4_internal_access` starts with `let Some(expected_key) = distributed_gemma4_shared_key_from_env() else { return Ok(()); };`, so it allows every request when no key is set. The comparison `provided == Some(expected_key.as_str())` is not constant-time, and a bad key gets 400 BadRequest instead of 401.
- The `/psionic/internal/gemma4/pipeline/{step,reset}` routes and the coordination POST and redact routes share the `/v1` router, which has no auth layer.
- The redact handler trusts a self-reported `actor` field (1619-1624).
- Mesh coordination is enabled by default (`OpenAiCompatConfig::new`, 1015).

Mitigation today: `scripts/pylon-psionic.sh:138` runs with `--host 127.0.0.1 --mesh-coordination disabled`.

**Impact:** A worker started with a non-loopback `--host` and no key env var accepts pipeline step and reset calls from the network. On a non-loopback node with coordination enabled, anyone can post or redact entries under any actor name.

**Suggested action:**
1. Fail closed: when the worker role is set and no key is configured, `OpenAiCompatServer::from_config` returns a Config error.
2. Compare keys in constant time on sha256 digests (`sha2` is already a dependency) and return 401 on mismatch.
3. Nest `/psionic/internal/*` and the coordination POST and redact routes in a sub-Router behind an auth middleware.
4. Refuse a non-loopback `--host` when no management/auth key is configured.
5. Verify: add tests for the unset-key, wrong-key and right-key cases.

### PS3-09 openai_http.rs is a 25k-line god file holding two parallel OpenAI servers

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (openai_http.rs:943-949, 3882-3923, 5937, 9181)

**Evidence:** 25,289 lines with about 647 `fn` definitions. The GptOss router (943-949) and the generic router (3882-3923) each implement health, models and chat. Largest items:

| Item | Lines | Starts at |
|---|---|---|
| `mesh_management_console_html` | 453 | 5937 |
| `handle_generic_responses` | 498 | 9181 |
| Generic worker spawn | 441 | 3934 |
| `handle_generic_chat_completions` | 363 | |

The file has 135 tests.

**Impact:** The file is hard to navigate, the two servers drift apart on the same `/v1` contract, and agents working in parallel collide on one file.

**Suggested action:**
1. In one mechanical commit with no logic changes, split the file into an `openai_http/` directory: `types.rs`, `generic_server.rs`, `gpt_oss_server.rs`, `worker.rs`, `mesh_management.rs`, `coordination_store.rs`, `console.rs` (HTML loaded with `include_str!`), `llama_cpp_proxy.rs`, `gemma4_distributed.rs`, `responses.rs` and `tests/`.
2. In a follow-up, fold `GptOssOpenAiCompatServer` into `OpenAiCompatServer` as one more generation-service variant.
3. Verify: all 135 tests still pass after each step.

### PS3-10 Forward-pass functions thousands of lines long, with few tests, in qwen35.rs and gpt_oss.rs

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [gpt_oss.rs](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (gpt_oss.rs:6519)
- [qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs) (qwen35.rs:8449)
- [gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (gguf.rs:4927, 6060)
- [psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs) (lib.rs:6395)

**Evidence:** Function lengths at HEAD, measured by brace matching:

| File | Function | Lines |
|---|---|---|
| `gpt_oss.rs` | `forward_step_with_output_mode` | 2,654 |
| `gpt_oss.rs` | `encode_cuda_forward_step_submission` | 958 |
| `gpt_oss.rs` | `run_metal_generation_request` | 698 |
| `qwen35.rs` | `forward_token_fused` | 1,600 |
| `qwen35.rs` | `generate_inner` | 871 |
| `qwen35.rs` | `forward_full_attention_device` | 851 |
| `gguf.rs` | `forward_local_step_with_layer_caches` | 1,131 |
| `gguf.rs` | `forward_step_with_layer_caches` | 914 |
| models `lib.rs` | `build_gguf_decoder_tensor_layout` | 1,108 |

`qwen35.rs` (17,144 lines at HEAD) has 5 `#[test]`. `gpt_oss.rs` has 22.

**Impact:** The production Qwen3.5 decode path is the least tested and hardest to read code in the area. Numerical regressions show up only in manual benchmarks.

**Suggested action:**
1. Add golden-logit parity tests for the qwen35 CPU path. Use a tiny synthetic GGUF with random weights and compare `forward_token_fused` against an unfused reference.
2. With those tests in place, split `forward_step_with_output_mode` and `forward_token_fused` into per-stage functions (embed, attention, FFN/MoE, head) that take typed plan structs.
3. Split `build_gguf_decoder_tensor_layout` by `GgufDecoderFamily`.
4. Coordinate with the agent currently editing `qwen35.rs` (uncommitted changes in the working tree).
5. Verify: the parity tests pass before and after each split.

### PS3-11 gpt_oss.rs allows expect_used file-wide and enforces invariants with 23 `expect("checked above")` calls

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [gpt_oss.rs](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (gpt_oss.rs:1-11, 6519, 10086)
- [psionic-serve/src/lib.rs](../../../../crates/psionic/crates/psionic-serve/src/lib.rs) (lib.rs:3-8)

**Evidence:** Lines 1-11 hold an inner `#![allow(... clippy::expect_used ...)]`, and the first `#[cfg(test)]` is at line 12965, so the allow covers all production code. The 23 `.expect("checked above")` calls are inside `forward_step_with_output_mode`. `forward_step_with_device_attention` carries `#[allow(dead_code)]` (10086). `lib.rs:3-8` allows `too_many_arguments` and `large_enum_variant` for the whole crate.

**Impact:** A panic in the single worker thread takes down every loaded model, and the file-wide allow hides any new `expect`.

**Suggested action:**
1. Remove `clippy::expect_used` from the inner allow in `gpt_oss.rs`.
2. Replace the `Option` juggling with a typed context struct built once with `let-else` that returns `Err`.
3. Delete `forward_step_with_device_attention` (10086) if nothing calls it.
4. Verify: `cargo clippy --manifest-path crates/psionic/Cargo.toml -p psionic-serve` passes with no new allows.

### PS3-12 psionic-models/src/lib.rs (15.4k lines) mixes descriptors, GGUF parsing, tokenizers, templates and MedPsy policy

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs) (lib.rs:1620, 1840-2160, 6395, 10146)

**Evidence:** 15,427 lines with 91 top-level public structs and enums. The GGUF reader spans roughly lines 1620-2160. `build_gguf_decoder_tensor_layout` (1,108 lines) starts at 6395, and tests start at 10146. The crate root has 75 `pub use x::*` glob re-exports, some of them for research modules.

**Impact:** The GGUF parser, which handles untrusted input, is buried in a catch-all file, and the public API has no clear boundary.

**Suggested action:**
1. Extract `gguf/{reader,metadata,tensor_types,layout}.rs`, `tokenizer.rs`, `prompt_template.rs`, `weights.rs`, `descriptors.rs` and `medpsy.rs`.
2. Reduce `lib.rs` to `mod` declarations plus explicit `pub use` lists.
3. Move the research modules behind a `research` feature, or delete them.
4. Verify: `cargo test -p psionic-models` and `cargo check -p psionic-serve` pass, and `lib.rs` contains no code beyond module declarations.

### PS3-13 63 undocumented PSIONIC_* env vars, some read per layer or per token

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs) (qwen35.rs:6772-6774, 8191, 8273)
- [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (openai_http.rs:888-891)

**Evidence:** The crates use 63 distinct `PSIONIC_*` identifiers at HEAD, and none appears in `docs/psionic` or `crates/psionic/README.md`. `qwen35_fused_qkv_rms_norm_enabled()` (6772) calls `env::var_os` each time; its callers are at 6806, 6880, 10074 and 10112. `PSIONIC_QWEN35_DEBUG_ATTENTION` is read at 8191 and `PSIONIC_QWEN38_DEBUG_NONFINITE` at 8273. The truthy parse at `openai_http.rs:888-891` is one-off code, while most flags only check `is_some`/`is_none`.

**Impact:** Operators cannot discover these settings, and the rules for what counts as "on" vary from flag to flag. The per-call env lookups cost microseconds against a GPU decode step, so performance impact is minor.

**Suggested action:**
1. Add `psionic-serve/src/env_flags.rs` with a `PsionicServeFlags` struct read once through `LazyLock`, plus a single truthy parser. Replace the `env::var` calls inside the kernels with it.
2. Promote the production settings (`LLAMA_SERVER_BIN`, `GEMMA4_DISTRIBUTED_*`, `OPENAI_INCLUDE_DEBUG_FIELDS`) to CLI or config fields.
3. List the debug flags in a table in `docs/psionic/README.md`.
4. Verify: `git grep -n 'env::var' crates/psionic/crates/psionic-serve/src` returns matches only in `env_flags.rs` and the bin entry points.

### PS3-14 Serving library ships hard-coded benchmark numbers as "evidence"

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [psion_rvllm_memory_pool.rs](../../../../crates/psionic/crates/psionic-serve/src/psion_rvllm_memory_pool.rs) (psion_rvllm_memory_pool.rs:4-7, 98-112)
- [gpt_oss.rs](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (gpt_oss.rs:5425)

**Evidence:** `psion_rvllm_memory_pool.rs` hard-codes `p50_step_latency_ms` 21.4 → 18.6 and `tokens_per_second` 53.1 → 61.0 (lines 102-110). `PSION_RVLLM_MEMORY_POOL_DOC_PATH = docs/PSION_RVLLM_MEMORY_POOL.md` exists neither at the repo root nor under `crates/psionic/docs`. Serve `src` has 15 `psion_rvllm_*` files. Execution code does use `select_psion_rvllm_attention_backend` (`gpt_oss.rs:5425`, `6627`).

**Impact:** The literals read like measured results, and doc paths that point at missing files mislead readers and agents. These packets are internal and never served to users.

**Suggested action:**
1. Move `PsionRvllmAttentionBackend`, its selector and the FA3/cuBLASLt constants into `psionic-serve/src/cuda_policy.rs`.
2. Delete the packet and benchmark builders and their examples.
3. Remove `*_DOC_PATH` constants that point at missing files, or replace them with upstream links pinned to the import commit.
4. Verify: `git grep -n 'psion_rvllm' crates/psionic/crates/psionic-serve/src` shows only `cuda_policy.rs` and its callers.

### PS3-15 Fetch-text plugin URL allowlist is a string-prefix check, and redirects skip it (code currently unreachable)

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:**
- [tassadar_post_article_starter_plugin_runtime.rs](../../../../crates/psionic/crates/psionic-runtime/src/tassadar_post_article_starter_plugin_runtime.rs) (tassadar_post_article_starter_plugin_runtime.rs:1622, 2794-2799, 2832-2840)

**Evidence:** `url_allowed` is `allowlisted_url_prefixes.iter().any(|prefix| url.starts_with(prefix))`. `invoke_live_fetch` follows redirects with `redirect::Policy::limited(...)` and does not check each hop against the allowlist. `git grep` finds no caller of `FetchTextRuntimeConfig::live` (1622) anywhere in `crates/psionic`.

**Impact:** If this code is ever wired up, it allows SSRF and allowlist bypass. Today it is dead code.

**Suggested action:**
1. Delete it as part of the Tassadar prune (PS3-03).
2. If it is kept, parse each URL and compare scheme, host and port exactly, with a path match that stops at a segment boundary. Re-run `url_allowed` on every hop with a custom redirect `Policy`, and block private and link-local IPs.
3. Verify with tests for `example.com.evil.net`, URLs with userinfo, and a redirect to `127.0.0.1`.

### PS3-16 366 glob re-exports make the crate APIs unbounded

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [psionic-serve/src/lib.rs](../../../../crates/psionic/crates/psionic-serve/src/lib.rs)
- [psionic-runtime/src/lib.rs](../../../../crates/psionic/crates/psionic-runtime/src/lib.rs)
- [psionic-provider/src/lib.rs](../../../../crates/psionic/crates/psionic-provider/src/lib.rs)
- [psionic-router/src/lib.rs](../../../../crates/psionic/crates/psionic-router/src/lib.rs)
- [psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs)

**Evidence:** Matches for `^pub use .*::\*;` in each `lib.rs` at HEAD: serve 54, runtime 98, models 75, router 27, provider 112, total 366.

**Impact:** Every helper becomes crate-root API, and name collisions between glob imports resolve silently. Most of this goes away with the prune.

**Suggested action:**
1. After the prune, collect the items downstream crates actually import, per crate and starting with psionic-runtime: `git grep -ho 'psionic_runtime::[A-Za-z_]*' crates/psionic/crates | sort -u`.
2. Replace the globs with explicit `pub use` lists, and make everything else `pub(crate)`.
3. Verify: the psionic workspace builds and `grep -c 'pub use .*::\*;'` on each `lib.rs` returns 0.

### PS3-17 rms_norm and related kernels are copied across serve modules

**Severity:** Low · **Category:** duplication · **Effort:** M

**Locations:**
- [gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (gguf.rs:16123, 16181, 15115)
- [gpt_oss.rs](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (gpt_oss.rs:14215, 14264)
- [qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs) (qwen35.rs:16605, 16615)

**Evidence:**

| Function | Copies |
|---|---|
| `fn rms_norm(input, weight, epsilon)` | `gguf.rs:16123`, `gpt_oss.rs:14215`, `qwen35.rs:16605` |
| `per_head_rms_norm` | `gguf.rs:16181`, `qwen35.rs:16615` |
| `softmax_selected` | `gguf.rs:15115`, `gpt_oss.rs:14264` |

The `gguf.rs` copy uses vDSP on macOS and a scalar sum on other platforms. Every copy uses `zip`, which silently truncates when lengths differ. The softmax in `runtime/lib.rs` works on `SampleToken` with a different signature, so it is not a duplicate.

**Impact:** Each fix has to be made in several places, and numeric results differ between Mac and Linux.

**Suggested action:**
1. Create `psionic-serve/src/cpu_math.rs` with length-checked `rms_norm`, `rms_norm_in_place`, `per_head_rms_norm`, `softmax_selected` and `argmax`, plus one `cfg`-gated macOS fast path.
2. Replace the copies in `gguf.rs`, `gpt_oss.rs` and `qwen35.rs`. Coordinate on `qwen35.rs`, which has uncommitted work.
3. Verify: add a vDSP-vs-scalar parity test, and confirm with `git grep 'fn rms_norm'` that only one definition remains.

### PS3-18 psionic-models always links candle and moshi

**Severity:** Low · **Category:** build · **Effort:** M

**Locations:**
- [psionic-models/Cargo.toml](../../../../crates/psionic/crates/psionic-models/Cargo.toml) (Cargo.toml:16-30)
- [csm.rs](../../../../crates/psionic/crates/psionic-models/src/csm.rs)
- [medpsy_qwen3.rs](../../../../crates/psionic/crates/psionic-models/src/medpsy_qwen3.rs)
- [qwen38_vision_runtime.rs](../../../../crates/psionic/crates/psionic-models/src/qwen38_vision_runtime.rs)

**Evidence:** `candle-core` 0.9.2, `candle-nn`, `candle-transformers` and `moshi` 0.6.4 are unconditional dependencies. The crate features only toggle their cuda/metal sub-features. Only `csm.rs`, `medpsy_qwen3.rs` and `qwen38_vision_runtime.rs` reference candle.

**Impact:** Every build compiles a second ML stack that the GGUF serving path never uses.

**Suggested action:**
1. Make `candle*` and `moshi` optional behind new features `csm`, `medpsy` and `qwen38-vision`, and have the existing `*-cuda`/`*-metal` features enable them.
2. Put the three modules behind those features with `cfg`. In serve, do the same for `csm_speech.rs` and the `psionic-csm-speech-server` bin.
3. Verify: `cargo tree -p psionic-serve --no-default-features | grep -c candle` returns 0.

### PS3-19 crates/psionic/target (448 MB) is not git-ignored

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [crates/psionic/target](../../../../crates/psionic/target)
- [.gitignore](../../../../.gitignore)

**Evidence:** `git check-ignore -v crates/psionic/target` exits 1, and `du -sh` reports 448M. `crates/psionic` has no `.gitignore`.

**Impact:** A single `git add -A` could commit hundreds of MB of build output.

**Suggested action:**
1. Create `crates/psionic/.gitignore` containing `/target/`.
2. Verify: `git check-ignore crates/psionic/target` prints the path.

### PS3-20 Four server binaries parse arguments by hand, duplicating defaults from the config structs

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [psionic-openai-server.rs](../../../../crates/psionic/crates/psionic-serve/src/bin/psionic-openai-server.rs) (psionic-openai-server.rs:148-272)
- [openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (openai_http.rs:1008-1016)

**Evidence:** At HEAD, `psionic-openai-server.rs` parses flags with manual `match argument.as_str()` loops at lines 148 and 193 (`"--host"` at 197). `usage()` at 270 contains a 612-character line (272). The defaults (host `127.0.0.1`, `mesh_coordination_enabled = true`) are repeated in `OpenAiCompatConfig::new` (1011-1015). `src/bin` has four binaries, and clap is not a psionic workspace dependency.

**Impact:** Help text and flags drift apart, and new settings get added as env vars instead of flags.

**Suggested action:**
1. Add `clap` (derive) to the psionic workspace dependencies.
2. Define a shared `#[derive(Args)] ServeArgs` that converts into `OpenAiCompatConfig`, with all defaults in one `Default` impl.
3. Port the existing parse tests to `try_parse_from`.
4. Verify: the parse tests pass and `--help` shows every flag the old `usage()` listed.

## Verification corrections

The verifier rejected no findings outright. It corrected these reviewer measurements and severities:

- **qwen35 sizes.** The reviewer's figures (17,182 lines, `forward_token_fused` 1,614) came from the dirty working tree. HEAD values are 17,144 and 1,600.
- **Missing fixtures under `fixtures/tassadar`.** 300, not 286.
- **`PSIONIC_*` count.** 63 distinct identifiers, not 58.
- **Glob re-exports.** The total is 366, not the 466 in the reviewer's title.
- **"No consumer" for Tassadar (PS3-03).** Wrong: eval, research, train and compiler import `psionic_runtime` Tassadar items, so the prune has to cascade across crates.
- **Downgraded findings.**
  - PS3-05: macOS-only UB, triggered by a malformed model file the operator chose.
  - PS3-13: the per-token env lookups cost almost nothing.
  - PS3-14: internal packets, not user-facing.
  - PS3-15: no caller exists.
  - PS3-04: the repo has no CI by design, so the fix targets `verify-changed.py`.
