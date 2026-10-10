# Giant files, functions, and module structure

**Scope:** All tracked, non-generated Rust in the repo: the main workspace plus the separate `crates/psionic` workspace. **Health grade: C.** Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`.

Size is the codebase's biggest structural problem. The repo holds about 3.59M lines of tracked, non-generated Rust in 6,701 files. The main workspace has 2.36M lines in 4,541 files, and `crates/psionic` has 1.23M lines in 2,159 files. 89 files are over 3,000 lines, 38 are over 5,000 and 12 are over 10,000. All 12 of the files over 10,000 lines are in psionic, where `lib.rs` and model files have turned into catch-alls. psionic also has 173 functions over 300 lines. The worst is `forward_step_with_output_mode` in GPT-OSS at 2,654 lines.

Per crate, the main workspace is in better shape: it has 184 small-to-medium crates. It still has 14 files over 5,000 lines and 180 functions over 300 lines (35 of them over 500). Most of those large files are also the files that change most often: `verse/src/app.rs` has 106 commits since 2026-09-10, `openagents-desktop/src/shell.rs` 96, `coder-worker.rs` 81, `coder-mobile/src/verse_app.rs` 76, `gateway/src/serve.rs` 67 and `coder/src/runtime.rs` 53.

The same few patterns cause most of the size:

- large inline `mod tests {}` blocks
- one huge impl block per type
- UI and app god-structs
- enums with 98 to 171 variants that are matched in about 10 parallel places
- production code that pulls in example or other-crate files with `#[path]`
- psionic `lib.rs` files that mix glob re-exports with 10k lines of unrelated types

No lint stops this growth. Most of the fixes below are mechanical moves with low risk. The most valuable ones are the busy files in coder, gateway, verse and desktop.

## Measurements

| Metric | Value |
|---|---|
| Tracked non-generated `.rs` | 6,701 files, ~3.59M lines |
| Main workspace / psionic | 2,358,644 lines in 4,541 files / 1,230,103 lines in 2,159 files |
| Files >3k lines | 89 (psionic 45, rest 44) |
| Files >5k lines | 38 (rest 14) |
| Files >10k lines | 12 (all psionic) |
| Functions >300 lines | psionic 173, rest 180 (rest >500: 35) |
| Inline test modules inside files >3k lines | ~114,945 lines |
| `#[path]` attributes (non-psionic) | 289 in 194 files; 49 cross-crate (31 include `coder-control/src/tests/relay.rs`) |
| Tassadar research lane | 1,269 files, ~388,654 lines, 217 `tassadar_post_article_*` files (reviewer figures) |
| Unsafe blocks in psionic | 266, with 8 `SAFETY` comments (cuda `lib.rs`: 198 unsafe, 0 SAFETY) |
| `clippy::too_many_lines` mentions (main) | 41, 33 of them in coder-one; inert because pedantic is off. psionic allows it globally |
| `too_many_arguments` allows | 206 main, 267 psionic |
| Duplicated helpers | `fn hex` 97 copies, `fn unix_now` 60 (6 in gateway), `civil_from_days` 5 |

**Largest crates by LOC:**

| Crate | Lines |
|---|---|
| psionic-train | 417,177 (664 files, 362 glob re-exports in `lib.rs`) |
| coder | 262,296 |
| psionic-eval | 162,211 |
| psionic-serve | 146,994 |
| coder-one | 132,534 |
| gym | 110,396 |

**Largest functions:**

| Function | Location | Lines |
|---|---|---|
| `forward_step_with_output_mode` | `gpt_oss.rs` | 2,654 |
| `run_with_args` | `psion_actual_pretraining_operator.rs` | 1,813 |
| `forward_token_fused` | `qwen35.rs` | 1,600 |
| `lean_loop` | coder-one `micro/lean.rs` | 1,398 |
| `forward_local_step_with_layer_caches` | `gguf.rs` | 1,131 |
| `suite_loop` | coder-one `micro.rs` | 1,131 |
| `canonical_supervision_scenarios` | `compiled_agent_receipts.rs` | 1,123 |
| `build_gguf_decoder_tensor_layout` | psionic-models `lib.rs` | 1,108 |
| `Photo::new` | verse-pbr `gpu.rs` | 1,104 |
| `draw_frame` | verse-pbr imported | 1,002 |

**God types:**

| Type | Size |
|---|---|
| `TassadarEnvironmentError` | 171 variants (135 `Missing*`) |
| verse `App` | 102 fields |
| coder-access `Operation` | 98 variants |
| openagents-mobile `Request` | 90 variants |
| terminal-core `KeyCode` | 87 variants |
| nostr-relay `Statements` | 85 fields |
| desktop chat `Panel` | 68 fields |

The largest single match is `name()` at `protocol.rs:1227`, with 98 arms.

### Top 40 largest files at HEAD

| # | File | Lines |
|---|---|---|
| 1 | [psionic-backend-cuda/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs) | 26,043 |
| 2 | [psionic-serve/src/openai_http.rs](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) | 25,289 |
| 3 | [psionic-serve/src/gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) | 20,498 |
| 4 | [psionic-serve/src/qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs) | 17,144 |
| 5 | [psionic-backend-metal/src/lib.rs](../../../../crates/psionic/crates/psionic-backend-metal/src/lib.rs) | 16,177 |
| 6 | [psionic-serve/src/gpt_oss.rs](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) | 15,459 |
| 7 | [psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs) | 15,427 |
| 8 | [psionic-runtime/src/lib.rs](../../../../crates/psionic/crates/psionic-runtime/src/lib.rs) | 14,927 |
| 9 | [psionic-serve/src/lib.rs](../../../../crates/psionic/crates/psionic-serve/src/lib.rs) | 14,491 |
| 10 | [psionic-runtime/src/tassadar.rs](../../../../crates/psionic/crates/psionic-runtime/src/tassadar.rs) | 13,233 |
| 11 | [psionic-provider/src/lib.rs](../../../../crates/psionic/crates/psionic-provider/src/lib.rs) | 10,878 |
| 12 | [psionic-net/src/lib.rs](../../../../crates/psionic/crates/psionic-net/src/lib.rs) | 10,029 |
| 13 | [coder/src/runtime.rs](../../../../crates/coder/src/runtime.rs) | 9,576 |
| 14 | [psionic-train/src/parameter_golf_single_h100_training.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_single_h100_training.rs) | 8,736 |
| 15 | [psionic-ir/src/autodiff.rs](../../../../crates/psionic/crates/psionic-ir/src/autodiff.rs) | 8,372 |
| 16 | [psionic-ir/src/lib.rs](../../../../crates/psionic/crates/psionic-ir/src/lib.rs) | 8,141 |
| 17 | [psionic-serve/src/tassadar.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar.rs) | 7,976 |
| 18 | [psionic-cluster/src/ordered_state.rs](../../../../crates/psionic/crates/psionic-cluster/src/ordered_state.rs) | 7,422 |
| 19 | [psionic-train/src/psion_reference_pilot.rs](../../../../crates/psionic/crates/psionic-train/src/psion_reference_pilot.rs) | 7,203 |
| 20 | [coder/src/bin/coder-worker.rs](../../../../crates/coder/src/bin/coder-worker.rs) | 7,065 |
| 21 | [openagents-desktop/src/shell.rs](../../../../crates/openagents-desktop/src/shell.rs) | 6,756 |
| 22 | [coder-mobile/src/verse_app.rs](../../../../crates/coder-mobile/src/verse_app.rs) | 6,685 |
| 23 | [gateway/tests/serve.rs](../../../../crates/gateway/tests/serve.rs) | 6,328 |
| 24 | [psionic-array/src/lib.rs](../../../../crates/psionic/crates/psionic-array/src/lib.rs) | 6,137 |
| 25 | [gateway/src/serve.rs](../../../../crates/gateway/src/serve.rs) | 6,008 |
| 26 | [verse/src/app.rs](../../../../crates/verse/src/app.rs) | 5,913 |
| 27 | [psionic-train/src/parameter_golf_distributed_8xh100_train_step.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_distributed_8xh100_train_step.rs) | 5,849 |
| 28 | [psionic-eval/src/tassadar.rs](../../../../crates/psionic/crates/psionic-eval/src/tassadar.rs) | 5,729 |
| 29 | [openagents-desktop/src/chat.rs](../../../../crates/openagents-desktop/src/chat.rs) | 5,673 |
| 30 | [openagents-chat-app/src/coder_tab.rs](../../../../crates/openagents-chat-app/src/coder_tab.rs) | 5,589 |
| 31 | [verse-pbr/src/pbr/gpu.rs](../../../../crates/verse-pbr/src/pbr/gpu.rs) | 5,586 |
| 32 | [gym/src/gate.rs](../../../../crates/gym/src/gate.rs) | 5,533 |
| 33 | [psionic-train/src/parameter_golf_reference.rs](../../../../crates/psionic/crates/psionic-train/src/parameter_golf_reference.rs) | 5,504 |
| 34 | [coder/src/task/autostart.rs](../../../../crates/coder/src/task/autostart.rs) | 5,381 |
| 35 | [verse-world/src/play.rs](../../../../crates/verse-world/src/play.rs) | 5,366 |
| 36 | [psionic-train/examples/psion_actual_pretraining_operator.rs](../../../../crates/psionic/crates/psionic-train/examples/psion_actual_pretraining_operator.rs) | 5,213 |
| 37 | [psionic-train/src/apple_adapter.rs](../../../../crates/psionic/crates/psionic-train/src/apple_adapter.rs) | 5,138 |
| 38 | [coder-one/src/micro.rs](../../../../crates/coder-one/src/micro.rs) | 5,035 |
| 39 | [psionic-models/src/parameter_golf.rs](../../../../crates/psionic/crates/psionic-models/src/parameter_golf.rs) | 4,888 |
| 40 | [gym/src/bin/gym.rs](../../../../crates/gym/src/bin/gym.rs) | 4,808 |

## Strengths

- **A sibling-test convention already exists.** `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` is used 120 times, for example at `crates/coder/src/task/agent_plan.rs:1252` and `crates/coder-mobile/src/verse_app.rs:4201-4211`. Moving large inline test blocks out is therefore a mechanical change that follows an accepted pattern.
- **psionic builds as its own Cargo workspace.** The root `Cargo.toml` exclude list (lines 4-11) keeps CUDA/Metal builds and their lint policy out of the product workspace.
- **Workspace lints cover the main workspace.** They deny `dbg_macro`, `todo`, `unimplemented` and `unsafe_op_in_unsafe_fn`. Only 8 `crates/*/Cargo.toml` files lack `[lints] workspace = true`.
- **The size problem is concentrated.** The main workspace has 184 focused crates, many under 1k lines, so the oversized files are a few hotspots rather than a general pattern.
- **Long functions usually have good comments.** The comments explain why the code does what it does, for example the pipeline-ordering comments in `nostr-relay` `admit_inner` and the doc comments on coder-one `suite_loop`. That makes it safer to extract phases.
- **No orphaned `.rs` files.** Every psionic file is reachable from a `mod` declaration or is an auto-discovered bin.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-SIZE-01 | High | maintainability | coder `runtime.rs`: 9.6k lines, ~3,800-line `impl Runtime`, 4.2k lines of inline tests | M |
| X-SIZE-02 | High | architecture | coder-worker bin holds 3.6k lines of logic plus 3.1k lines of tests that no library can reuse | M |
| X-SIZE-03 | High | architecture | gateway `serve.rs` (6k lines) mixes routing, auth, money holds and the classify pipeline | L |
| X-SIZE-04 | Medium | maintainability | GptOss CUDA forward step is one 2,654-line function nested 17 levels deep | L |
| X-SIZE-05 | Medium | architecture | `gguf.rs` (20.5k lines) holds five services, with duplicated Metal/CUDA Gemma4 step code | L |
| X-SIZE-06 | Medium | architecture | `openai_http.rs` (25.3k lines) mixes API, console HTML, llama.cpp proxy and GPT-OSS state; half of it is tests | L |
| X-SIZE-07 | Medium | maintainability | CUDA/Metal `lib.rs` embed ~7.8k lines of FFI platform code; CUDA has 198 unsafe blocks and 0 SAFETY comments | L |
| X-SIZE-08 | Medium | architecture | psionic `lib.rs` files are both glob re-export facades and 10k+ lines of types | XL |
| X-SIZE-09 | Medium | architecture | psionic-serve depends on the 417k-line psionic-train for a handful of types | M |
| X-SIZE-10 | Medium | architecture | psionic-train binary compiles a 5.2k-line example via `#[path]` and toggles a global flag around it | M |
| X-SIZE-11 | Medium | duplication | 750-line benchmark-package fixture builder is duplicated in `src` and `examples` | S |
| X-SIZE-12 | Medium | architecture | The ~389k-line Tassadar research lane is compiled into the runtime/serve/provider crates | L |
| X-SIZE-13 | Medium | maintainability | coder-one `suite_loop` (1,131 lines) and `lean_loop` (1,398 lines) are near-parallel giant loops | M |
| X-SIZE-14 | Medium | maintainability | UI god-structs: verse `App` (102 fields), desktop `Panel` (68 fields); `shell.rs` is 60% tests | L |
| X-SIZE-15 | Medium | maintainability | coder-access `Operation` has 98 variants re-enumerated in ~10 parallel matches | M |
| X-SIZE-16 | Medium | build | A test relay fixture is compiled into other crates via 31 cross-crate `#[path]` includes | M |
| X-SIZE-17 | Medium | maintainability | ~115k lines of inline `mod tests {}` sit inside the largest production files | M |
| X-SIZE-18 | Medium | maintainability | verse-pbr `Photo::new` is a 1,104-line constructor; `encode_neon` is 638 lines | M |
| X-SIZE-19 | Medium | maintainability | Correctness-critical long functions: nostr-relay `admit_inner` (637), gym finance rebuild (852) | M |
| X-SIZE-20 | Low | maintainability | `TassadarEnvironmentError` has 171 variants, 135 of them unit `Missing*` | S |
| X-SIZE-21 | Low | build | Size lints are off: psionic allows `too_many_lines`; the main-workspace allows are inert | S |
| X-SIZE-22 | Low | testing | A 967-line test helper in openagents-cli is driven by four booleans | S |
| X-SIZE-23 | Low | repo-hygiene | 448 MB `crates/psionic/target` is not gitignored; root members repeats coderbench | S |
| X-SIZE-24 | Low | duplication | Copied helpers (`hex`, `unix_now`, `civil_from_days`) show shared utility modules are missing | M |
| X-SIZE-25 | Low | maintainability | Large tick/sweep functions: verse-world play tick (622), coder autostart sweep (411), gym `gate.rs` (5.5k) | M |

### X-SIZE-01 coder runtime.rs: one 9.6k-line file with a ~3,800-line impl Runtime and 4.2k lines of inline tests

**Severity:** High · **Category:** maintainability · **Effort:** M

**Locations:**
- [runtime.rs:892](../../../../crates/coder/src/runtime.rs) (`impl Runtime`)
- [runtime.rs:3290](../../../../crates/coder/src/runtime.rs)
- [runtime.rs:4288](../../../../crates/coder/src/runtime.rs)
- [runtime.rs:5411](../../../../crates/coder/src/runtime.rs) (`mod tests`)

**Evidence:** The file is 9,576 lines at HEAD. `impl Runtime` starts at :892 and `mod tests` at :5411. Production code also contains a test-only `#[cfg(test)] fn cancel_from` wrapper, with the comment "this wrapper remains the tests' entry point".

**Impact:** This is the core runtime orchestrator and changes often (53 commits since 2026-09-10). Merge conflicts are likely, and the admission and budget logic can't be reviewed on its own.

**Suggested action:**
1. Turn the file into a `runtime/` directory: `mod.rs`, `admit.rs`, `select.rs`, `effects.rs`, `runstate.rs`, `budget.rs` and `steps/{decide,review,verify,check,delegate,ask}.rs`. Give each file its own `impl Runtime` block.
2. Move the tests into `runtime/tests/*.rs`, grouped the same way, using the `#[path]` sibling convention.
3. Delete the `cancel_from` shim and have the tests call `cancel_pending` directly.
4. Verify with `cargo test -p coder runtime::`. Each step should be a pure move with no behavior diff.

### X-SIZE-02 coder-worker binary holds 3.6k lines of business logic plus 3.1k lines of tests that no library can reuse

**Severity:** High · **Category:** architecture · **Effort:** M

**Locations:**
- [coder-worker.rs:461](../../../../crates/coder/src/bin/coder-worker.rs) (local `fn hex`)
- [coder-worker.rs:2800](../../../../crates/coder/src/bin/coder-worker.rs) (`async fn routed`)
- [coder-worker.rs:3934](../../../../crates/coder/src/bin/coder-worker.rs) (tests)

**Evidence:** The file is 7,065 lines at HEAD and is a bin target. `async fn routed` sits under `#[allow(clippy::too_many_arguments)]` at about :2800, and the file defines its own `fn hex` at :461.

**Impact:** Logic inside a bin target can't be reused or tested from the library. This file changes often (81 commits since 2026-09-10).

**Suggested action:**
1. Move everything except `main()` and option parsing into `crates/coder/src/worker/`.
2. Split `Job::routed` into separate select, run, judge and publish steps that share a context struct.
3. Move the tests to `worker/tests/`.
4. Replace the local `hex` with the shared helper from X-SIZE-24.
5. Verify with `cargo test -p coder worker`, and check that the bin is under about 300 lines.

### X-SIZE-03 gateway serve.rs (6k lines) mixes routing, auth, money holds and the entire classify pipeline

**Severity:** High · **Category:** architecture · **Effort:** L

**Locations:**
- [serve.rs:1964](../../../../crates/gateway/src/serve.rs)
- [serve.rs:2755](../../../../crates/gateway/src/serve.rs)
- [serve.rs:4045](../../../../crates/gateway/src/serve.rs)
- [serve.rs:5724](../../../../crates/gateway/src/serve.rs) (`civil_from_days`)
- [serve.rs:5743](../../../../crates/gateway/src/serve.rs) (`unix_now`)
- [accounts.rs:493](../../../../crates/gateway/src/accounts.rs)
- [funding.rs:777](../../../../crates/gateway/src/funding.rs)

**Evidence:** The file is 6,008 lines at HEAD. `unix_now` is defined in several places:
- `serve.rs:5743` (`pub(crate)`)
- `accounts.rs:493` (`pub(crate)`)
- `funding.rs:777`
- `bin/billing-sandbox.rs:92`
- `tests/common/mod.rs:47`
- `tests/relay_worker.rs:45`

That puts two `pub(crate)` copies in the same crate. `civil_from_days` is at `serve.rs:5724`. The companion `tests/serve.rs` is 6,328 lines.

**Impact:** The money hold/settle code sits next to classify orchestration in a file that changes often (67 commits since 2026-09-10), which makes billing changes hard to review in isolation.

**Suggested action:**
1. Split the file into `serve/{mod.rs, auth.rs, money.rs, forward.rs, receipts.rs}`, and move the classify pipeline to `classify/pipeline.rs`.
2. Add one `gateway::time` module with `unix_now` and `civil_from_days`, and delete the other copies (including the copies in the test helpers, which should use the crate's time module).
3. Split `tests/serve.rs` along the same lines.
4. Verify with `cargo test -p gateway`. Running `git grep -n 'fn unix_now' crates/gateway` should then return one definition.

### X-SIZE-04 GptOss CUDA forward step is a single 2,654-line function nested 17 levels deep

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [gpt_oss.rs:6519](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (`forward_step_with_output_mode`)
- [gpt_oss.rs:5335](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (`encode_cuda_forward_step_submission`)
- [gpt_oss.rs:2867](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (`run_metal_generation_request`)
- [gpt_oss.rs:1766](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (`run_cuda_generation_request`)
- [gpt_oss.rs:10409](../../../../crates/psionic/crates/psionic-serve/src/gpt_oss.rs) (`forward_step_with_device_attention_plan`)

**Evidence:** `forward_step_with_output_mode` runs from :6519 to about :9172, where it releases `hybrid_selected4_plan` and returns `result` just before `fn forward_step`. That is about 2,654 lines. The file is 15,459 lines. The reviewer's internal counts were not re-counted: 95 `if`, 124 `perf.` updates, 56 `hybrid_selected4_plan` references and a 17-level indent. The sibling functions listed above are also very long.

**Impact:** This is the hot decode path of the GPT-OSS CUDA lane. Changing selected4 expert routing, perf accounting or KV mirroring means reasoning about a 2.6k-line function, and the hybrid path can't be unit-tested on its own.

**Suggested action:**
1. Turn the file into a `gpt_oss/` directory: `mod.rs`, `cuda/step.rs`, `cuda/submission.rs`, `cuda/hybrid_selected4.rs`, `metal.rs` and `perf.rs`. `perf.rs` holds a `StepPerf` accumulator with scoped timers.
2. Split `forward_step_with_output_mode` into per-phase functions that share a `StepCtx` struct:
   - embed
   - per-layer attention plus KV mirror
   - MoE FFN, dispatching to a `HybridSelected4Plan` strategy
   - final norm/logits, chosen by `output_mode`
   - host-KV materialization
3. Move the tests that start at :12965 into `gpt_oss/tests.rs`.
4. Verify with `cargo test -p psionic-serve gpt_oss` and a fixed-prompt logits diff before and after on a CUDA host.

### X-SIZE-05 psionic-serve gguf.rs is 20.5k lines holding five services, with duplicated Metal and CUDA Gemma4 step code

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [gguf.rs:493](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)
- [gguf.rs:892](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)
- [gguf.rs:4515](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)
- [gguf.rs:6060](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (Metal `forward_step_with_layer_caches`)
- [gguf.rs:6975](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (Metal `forward_stage_step`)
- [gguf.rs:10362](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)
- [gguf.rs:12443](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs) (`CudaGemma4ModelInner`)
- [gguf.rs:12540](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)
- [gguf.rs:13221](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)

**Evidence:** The file is 20,498 lines at HEAD. The same function names appear in the Metal impl (:6060, :6975) and in the CUDA impl (:12540, :13221). The file also holds the CPU, Qwen35-proxy and distributed services.

**Impact:** Gemma4 algorithm fixes have to be applied by hand to both the Metal and CUDA copies, so the two backends can drift apart.

**Suggested action:**
1. Create `psionic-serve/src/gguf/` with `cpu_dense.rs`, `qwen35_proxy.rs`, `gemma4/{mod.rs, metal.rs, cuda.rs, distributed.rs}` and per-backend test files. This step is a pure move.
2. Pull the backend-independent step logic into a `Gemma4Stepper<B: Gemma4Backend>` so that each backend only supplies kernels.
3. Verify with `cargo test -p psionic-serve gguf` and the Gemma4 parity tests on Metal and CUDA hosts.

### X-SIZE-06 openai_http.rs (25.3k lines) mixes the HTTP API, mesh-management console HTML, a llama.cpp proxy and GPT-OSS server state, and is half tests

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [openai_http.rs:866](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs)
- [openai_http.rs:1079](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs)
- [openai_http.rs:1627](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs)
- [openai_http.rs:4667](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs)
- [openai_http.rs:5937](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (console HTML)
- [openai_http.rs:12765](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs) (tests)

**Evidence:** The file is 25,289 lines at HEAD. An inline test module starts at :12765, so about half the file is tests. The mesh console HTML is generated inside a Rust function at :5937.

**Impact:** Every change to a route, schema or the mesh console goes through one 25k-line compile unit. HTML built inside a Rust function can't be reviewed or linted as HTML.

**Suggested action:**
1. Split the file into `openai_http/{server.rs, gpt_oss_compat.rs, generic.rs, tool_calling.rs, schema.rs, llama_cpp_proxy.rs, mesh_management/{store.rs, api.rs}}`.
2. Move the console markup to `mesh_management/console.html` and load it with `include_str!`.
3. Move the tests to `openai_http/tests/*.rs`.
4. Verify with `cargo test -p psionic-serve openai_http` and a curl smoke test of `/v1/chat/completions`.

### X-SIZE-07 The CUDA and Metal backend lib.rs files embed about 7.8k lines of FFI platform code inline; the CUDA file has 198 unsafe blocks and no SAFETY comments

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [psionic-backend-cuda/src/lib.rs:6511](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs)
- [psionic-backend-cuda/src/lib.rs:9424](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs) (`mod platform`)
- [psionic-backend-cuda/src/lib.rs:17199](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs) (`mod platform`)
- [psionic-backend-cuda/src/lib.rs:18930](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs) (`mod tests`)
- [psionic-backend-metal/src/lib.rs:7713](../../../../crates/psionic/crates/psionic-backend-metal/src/lib.rs)
- [psionic-backend-metal/src/lib.rs:13660](../../../../crates/psionic/crates/psionic-backend-metal/src/lib.rs)

**Evidence:** Re-measured at HEAD, the CUDA `lib.rs` is 26,043 lines. `mod platform {` appears at :9424 and :17199, and `mod tests {` at :18930. `grep -c 'unsafe {'` returns 198 and `grep -c SAFETY` returns 0.

**Impact:** The unsafe FFI surface is interleaved with allocator and dispatch logic, so it can't be audited on its own.

**Suggested action:**
1. Move `mod platform` into `src/platform/{linux.rs, stub.rs}`.
2. Move buffer/pool, plan cache and GEMM tuning into their own modules, and make `execute()` dispatch to `ops/*.rs` per op family.
3. Move the tests out of `lib.rs`.
4. Add `undocumented_unsafe_blocks = "warn"` to psionic's `[workspace.lints.clippy]`, and write a `SAFETY` comment for each block as it moves.
5. Apply the same layout to `psionic-backend-metal`.
6. Verify with `cargo clippy -p psionic-backend-cuda`: the `undocumented_unsafe_blocks` warning count should go to zero for the moved modules. Then run the backend tests on a CUDA host.

### X-SIZE-08 psionic lib.rs files are both a glob re-export facade and 10k+ lines of unrelated types

**Severity:** Medium · **Category:** architecture · **Effort:** XL

**Locations:**
- [psionic-runtime/src/lib.rs:222](../../../../crates/psionic/crates/psionic-runtime/src/lib.rs)
- [psionic-provider/src/lib.rs:676](../../../../crates/psionic/crates/psionic-provider/src/lib.rs)
- [psionic-models/src/lib.rs:6395](../../../../crates/psionic/crates/psionic-models/src/lib.rs)
- [psionic-train/src/lib.rs](../../../../crates/psionic/crates/psionic-train/src/lib.rs)

**Evidence:** Re-measured at HEAD, psionic-runtime `lib.rs` is 14,927 lines and psionic-provider `lib.rs` is 10,878 lines. The reviewer reports 100 to 360 `pub use x::*` glob re-exports per file (362 in psionic-train); those counts were not re-counted.

**Impact:** The namespaces are flat and the real public API is unclear. New types go into `lib.rs` by default.

**Suggested action:**
1. Start with psionic-runtime and psionic-provider. Move the types defined in `lib.rs` into domain modules, and move the tests into `src/tests/`.
2. Replace the glob re-exports with explicit `pub use` lists.
3. Verify each step with `cargo check --workspace` in `crates/psionic`. Running `wc -l` on each `lib.rs` should show a facade of a few hundred lines.

### X-SIZE-09 psionic-serve (the serving path) depends on psionic-train, a 417k-line crate, for a handful of types

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:**
- [psionic-serve/Cargo.toml:55](../../../../crates/psionic/crates/psionic-serve/Cargo.toml)
- [psion_capability_withdrawal.rs](../../../../crates/psionic/crates/psionic-serve/src/psion_capability_withdrawal.rs)
- [qwen35.rs](../../../../crates/psionic/crates/psionic-serve/src/qwen35.rs)
- [gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs)
- [tassadar.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar.rs)
- [tassadar_article_transformer_minimal_frontier.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs)

**Evidence:** `Cargo.toml:55` lists `psionic-train = { path = "../psionic-train" }` as a normal dependency. There are 20 `psionic_train` references across 9 source files:
- 11 in `psion_capability_withdrawal.rs`
- 2 in `gguf.rs`
- 1 each in `psion_capability_matrix.rs`, `psion_generic_load_and_generate.rs`, `psion_served_evidence.rs`, `psion_served_output_claim_posture.rs`, `qwen35.rs`, `tassadar.rs` and `tassadar_article_transformer_minimal_frontier.rs`

**Impact:** Every serving build compiles the whole training crate, so the layering runs the wrong way.

**Suggested action:**
1. Move the route and receipt contract types, the Tassadar gate report schema and the Qwen loader into psionic-models or a new `psionic-contracts` crate.
2. Keep psionic-train only as a dev-dependency.
3. Verify that `cargo tree -p psionic-serve -e normal | grep psionic-train` returns nothing.

### X-SIZE-10 The production psionic-train binary compiles a 5.2k-line example via #[path] and toggles a global flag around it

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:**
- [psionic-train/src/main.rs:45](../../../../crates/psionic/crates/psionic-train/src/main.rs)
- [psionic-train/src/main.rs:491](../../../../crates/psionic/crates/psionic-train/src/main.rs)
- [psion_actual_pretraining_operator.rs:426](../../../../crates/psionic/crates/psionic-train/examples/psion_actual_pretraining_operator.rs)

**Evidence:** `main.rs` contains `#[allow(dead_code)] #[path = "../examples/psion_actual_pretraining_operator.rs"] mod psion_actual_pretraining_operator;`. It calls `set_human_output_enabled(false)`, then `run_with_args`, then `set_human_output_enabled(true)`. `run_with_args` is 1,813 lines long.

**Impact:** Production code lives under `examples/`. If `run_with_args` panics, the global flag is not restored.

**Suggested action:**
1. Move the operator into `src/actual_pretraining_operator/`, with one file per subcommand, and reduce the example to a thin wrapper.
2. Replace the global flag with an `Output` parameter. At minimum, use an RAII guard that restores the flag on drop.
3. Verify that `git grep '#\[path = "../examples' crates/psionic` returns nothing and that the operator subcommands still run.

### X-SIZE-11 The 750-line benchmark-package fixture builder is duplicated between src and examples

**Severity:** Medium · **Category:** duplication · **Effort:** S

**Locations:**
- [psion_benchmark_packages.rs:2452](../../../../crates/psionic/crates/psionic-train/src/psion_benchmark_packages.rs)
- [psion_benchmark_package_fixtures.rs:137](../../../../crates/psionic/crates/psionic-train/examples/psion_benchmark_package_fixtures.rs)

**Evidence:** Both files define `fn package_contracts() -> Result<Vec<...PsionBenchmarkPackageContract>, ...>`, and both bodies open with `Ok(vec![` at the cited lines.

**Impact:** The test fixtures and the fixture generator can silently diverge.

**Suggested action:**
1. Expose a single `canonical_benchmark_package_contracts()` function (behind a feature if needed).
2. Call it from both the tests and the example.
3. Regenerate the fixtures with the example and check `git diff --exit-code`: the output should be identical byte for byte.

### X-SIZE-12 The Tassadar research lane (~389k lines) is compiled into the runtime, serve and provider crates

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [psionic-runtime/src/tassadar.rs](../../../../crates/psionic/crates/psionic-runtime/src/tassadar.rs)
- [psionic-serve/src/tassadar.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar.rs)
- [psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs)

**Evidence:** psionic-serve has Tassadar modules that import `psionic_train`. The whole tree arrived in import commit `737f94a17c`. The reviewer's totals were spot-checked but not re-counted: 1,269 files, ~388k lines and 217 `tassadar_post_article_*` files.

**Impact:** Serving builds compile research code.

**Suggested action:**
1. Ask the owner to decide whether Tassadar is still active.
2. If it is active, put it behind a default-off `tassadar` feature or move it into its own crate.
3. If it is not, archive it.
4. Verify that `cargo build -p psionic-serve` with default features compiles no `tassadar*` modules.

### X-SIZE-13 coder-one suite_loop (1,131 lines) and lean_loop (1,398 lines) are near-parallel giant async loops under inert clippy allows

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [coder-one/src/micro.rs:2615](../../../../crates/coder-one/src/micro.rs)
- [coder-one/src/micro/lean.rs:2493](../../../../crates/coder-one/src/micro/lean.rs)

**Evidence:** `micro.rs:2614` carries `#[allow(clippy::too_many_lines)]` on `async fn suite_loop(&self, prepared: &Prepared) -> Result<Looped, Vec<Ran>>`. coder-one has 33 `too_many_lines` attribute mentions, all of them inert because clippy pedantic is off (see X-SIZE-21).

**Impact:** Every policy fix has to be made in both loops.

**Suggested action:**
1. Extract a shared `LoopClock` and a requirements-group helper into `micro/common.rs`.
2. Restructure each loop as `plan_round`, `run_round` and `judge_round`.
3. Remove the inert allows as the functions shrink.
4. Verify with `cargo test -p coder-one micro`.

### X-SIZE-14 UI and app god-structs: verse App (102 fields, 723-line frame), desktop chat Panel (68 fields), shell.rs is 60% inline tests

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [verse/src/app.rs:585](../../../../crates/verse/src/app.rs) (`struct App`)
- [verse/src/app.rs:4141](../../../../crates/verse/src/app.rs) (723-line frame)
- [openagents-desktop/src/chat.rs:44](../../../../crates/openagents-desktop/src/chat.rs) (`Panel`)
- [openagents-desktop/src/shell.rs:2071](../../../../crates/openagents-desktop/src/shell.rs) (`mod tests`)

**Evidence:**
- **verse:** re-counted, `struct App` has 102 fields. `verse/src/app.rs` has 106 commits since 2026-09-10.
- **desktop shell:** in the 6,756-line `shell.rs`, `cfg(test)` modules start at :2071 (`mod tests`), :3366 (`card_fixtures`), :4002 (`image_fixtures`) and :4195.

**Impact:** These are the busiest product files, and feature work keeps piling onto a single struct.

**Suggested action:**
1. Extract sub-state structs from verse `App`: `GymHud`, `Doors`, `Studio` and `Map`. Extract the same kind from the desktop `Panel`: `CommandPalette`, `SavedChats`, `RenameDialog` and `FeedbackForm`.
2. Split the 723-line frame function into per-sub-state draw and update calls.
3. Move the `shell.rs` test modules into sibling files with `#[path]`. `card_fixtures` is `pub(super)` and shared, so it needs its own sibling file rather than deletion.
4. Verify with `cargo test -p verse` and `cargo test -p openagents-desktop`.

### X-SIZE-15 coder-access Operation has 98 variants re-enumerated in about 10 parallel match methods

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [protocol.rs:555](../../../../crates/coder-access/src/protocol.rs) (`pub enum Operation`)
- [protocol.rs:1226](../../../../crates/coder-access/src/protocol.rs) (`name()`)
- [protocol.rs:1330](../../../../crates/coder-access/src/protocol.rs)

**Evidence:** The serde-tagged `pub enum Operation` is at :555. `name()` at :1226 maps every variant to a string such as `"cloud.projects"`, which repeats the `#[serde(rename)]` already on each variant.

**Impact:** Adding an operation means editing many parallel matches, including the authorization `Right` mapping.

**Suggested action:**
1. Generate `name()`, `required()` and `validate()` from one declarative macro table. This also removes the duplication between the serde renames and `name()`. Alternatively, group the variants into sub-enums.
2. Add an exhaustive test over all operations that asserts `name()` matches the serde tag and that every operation maps to a `Right`.
3. Verify with `cargo test -p coder-access`.

### X-SIZE-16 A test relay fixture is compiled into 30 other crates via cross-crate #[path] includes

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:**
- [coder-control/src/tests/relay.rs](../../../../crates/coder-control/src/tests/relay.rs)
- [coder-host/tests/end_to_end.rs:10](../../../../crates/coder-host/tests/end_to_end.rs)
- [gym-bridge/src/tests.rs:8](../../../../crates/gym-bridge/src/tests.rs)

**Evidence:** `git grep 'coder-control/src/tests/relay.rs'` finds 31 references at HEAD.

**Impact:** The fixture is compiled separately in every crate that includes it, and the dependencies it needs are hidden in each including crate's manifest.

**Suggested action:**
1. Create a `publish = false` `test-relay` crate from the fixture.
2. Add it as a dev-dependency wherever the fixture is used, and delete the `#[path]` includes.
3. Verify that `git grep 'coder-control/src/tests/relay.rs'` returns nothing and that `cargo test --workspace` still passes.

### X-SIZE-17 About 115k lines of inline `mod tests {}` live inside the largest production files

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [openai_http.rs:12765](../../../../crates/psionic/crates/psionic-serve/src/openai_http.rs)
- [psionic-backend-cuda/src/lib.rs:18930](../../../../crates/psionic/crates/psionic-backend-cuda/src/lib.rs)
- [coder/src/runtime.rs:5411](../../../../crates/coder/src/runtime.rs)
- [openagents-desktop/src/shell.rs:2071](../../../../crates/openagents-desktop/src/shell.rs)
- [coder-worker.rs:3934](../../../../crates/coder/src/bin/coder-worker.rs)

**Evidence:** About 114,945 lines of inline tests sit inside files over 3k lines. Spot-checked test-module starts: CUDA `lib.rs` :18930, `runtime.rs` :5411, `shell.rs` :2071.

**Impact:** Inline tests roughly double the size of the busiest files.

**Suggested action:**
1. Move every inline test module over 500 lines into a sibling file with `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`, as a pure move with no edits.
2. Do the busy files first: `runtime.rs`, `coder-worker.rs`, `shell.rs`, `gateway/src/serve.rs` and `verse/src/app.rs`.
3. Verify by running each crate's `cargo test` and checking that the test count is unchanged.

This overlaps with the per-file findings above but is a useful umbrella action.

### X-SIZE-18 verse-pbr Photo::new is a 1,104-line constructor and encode_neon is 638 lines

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [verse-pbr/src/pbr/gpu.rs:1046](../../../../crates/verse-pbr/src/pbr/gpu.rs)
- [verse-pbr/src/imported/mod.rs:1809](../../../../crates/verse-pbr/src/imported/mod.rs)

**Evidence:** The line counts come from the reviewer and were not re-measured.

**Impact:** Adding one render pass means editing a giant constructor.

**Suggested action:**
1. Introduce one struct per render pass, each with its own `new` and `encode`, plus a shared `Layouts` struct for bind-group layouts.
2. Reduce `Photo::new` to composing the passes, and split `encode_neon` per pass.
3. Verify with `cargo test -p verse-pbr` and a before/after frame capture.

### X-SIZE-19 Correctness-critical long functions: nostr-relay admit_inner (637 lines) and gym sales_finance rebuild (852 lines)

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [nostr-relay/src/store/mod.rs:409](../../../../crates/nostr-relay/src/store/mod.rs)
- [gym/src/sales_finance.rs:439](../../../../crates/gym/src/sales_finance.rs)

**Evidence:** `admit_inner` runs from :409 to :1045 (confirmed). The 852-line length of `rebuild` comes from the reviewer.

**Impact:** At this length the ordering invariants are hard to review.

**Suggested action:**
1. Split `admit_inner` into read, lock, judge and write phases, with the judge phase a pure function. Keep the existing ordering comments next to each phase.
2. Split `rebuild` into named phases that return a `FinanceError` enum.
3. Verify with `cargo test -p nostr-relay` and `cargo test -p gym`, and add a unit test for the pure judge phase.

### X-SIZE-20 TassadarEnvironmentError has 171 variants, 135 of them unit `Missing*` cases

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [psionic-environments/src/tassadar.rs:2156](../../../../crates/psionic/crates/psionic-environments/src/tassadar.rs)
- [tassadar_universality_witness_suite.rs](../../../../crates/psionic/crates/psionic-environments/src/tassadar_universality_witness_suite.rs)

**Evidence:** The enum runs from :2156 to :2754 and has 171 variants (re-counted). `TassadarEnvironmentError::Missing*` variants are also referenced in `tassadar_universality_witness_suite.rs`, not only in `tassadar.rs`.

**Impact:** Each new spec field adds boilerplate.

**Suggested action:**
1. Collapse the unit variants into `MissingField { field: &'static str }` and `InvalidField { field, reason }`.
2. Add a `require()` helper.
3. Update the callers in `tassadar_universality_witness_suite.rs`.
4. Verify with `cargo test -p psionic-environments`.

This can be skipped if X-SIZE-12 archives Tassadar.

### X-SIZE-21 Size lints are off: psionic allows too_many_lines workspace-wide, and the main-workspace allows do nothing

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [crates/psionic/Cargo.toml:16](../../../../crates/psionic/Cargo.toml)
- [Cargo.toml:36](../../../../Cargo.toml)
- [coder-one/src/micro.rs:2614](../../../../crates/coder-one/src/micro.rs)

**Evidence:**
- `crates/psionic/Cargo.toml` sets `pedantic = "warn"` at :10 and `too_many_lines = "allow"` at :16.
- The root `[workspace.lints.clippy]` sets only `dbg_macro`, `todo` and `unimplemented`, and no crate enables `clippy::pedantic`.
- The main workspace has 41 `clippy::too_many_lines` mentions, 33 of them in coder-one. All of them are inert.
- There is no `clippy.toml` in the repo.

**Impact:** Nothing limits function growth.

**Suggested action:**
1. Add `too_many_lines = "warn"` to the root `[workspace.lints.clippy]`, and change psionic's setting from `allow` to `warn`.
2. Add a root `clippy.toml` with `too-many-lines-threshold = 400`.
3. With that in place, the existing allows act as a list of known offenders, so new ones need a justification.
4. Run it in CI rather than in the local loop, given the owner's lean-verification rule.
5. Verify that `cargo clippy --workspace` reports the warning on a new over-threshold function.

### X-SIZE-22 A 967-line test helper driven by four booleans in openagents-cli plugin_purchase tests

**Severity:** Low · **Category:** testing · **Effort:** S

**Locations:**
- [openagents-cli/src/plugin_purchase/tests.rs:1181](../../../../crates/openagents-cli/src/plugin_purchase/tests.rs)

**Evidence:** The helper is `fn installed_purchase_case(binary: std::ffi::OsString, mapped: bool, shared: bool, restart: bool)` at :1181.

**Impact:** When a test fails, it is hard to tell which scenario broke.

**Suggested action:**
1. Replace the booleans with a `PurchaseScenario` builder and phase helpers, giving one named test per scenario.
2. Verify with `cargo test -p openagents-cli plugin_purchase` and check that the test count is the same or higher.

### X-SIZE-23 Repo hygiene: 448 MB crates/psionic/target is not gitignored, and the root members list repeats coderbench

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [.gitignore:1](../../../../.gitignore)
- [Cargo.toml:2](../../../../Cargo.toml)

**Evidence:** `git check-ignore crates/psionic/target/x` returns nothing, `crates/psionic/.gitignore` does not exist, and `du` reports 448M. The root `members` is `["crates/coderbench","crates/*"]`, so coderbench is already covered by the glob.

**Impact:** A `git add -A` could stage build output.

**Suggested action:**
1. Add `/crates/psionic/target/` to the root `.gitignore`.
2. Simplify `members` to `["crates/*"]`.
3. Verify that `git check-ignore crates/psionic/target/x` prints the path and that `cargo metadata` lists the same members as before.

### X-SIZE-24 Copied small helpers (hex, unix_now, civil_from_days) are a symptom of missing shared utility modules

**Severity:** Low · **Category:** duplication · **Effort:** M

**Locations:**
- [gateway/src/serve.rs:5743](../../../../crates/gateway/src/serve.rs)
- [gateway/src/accounts.rs:493](../../../../crates/gateway/src/accounts.rs)
- [gateway/src/funding.rs:777](../../../../crates/gateway/src/funding.rs)
- [coder/src/bin/coder-worker.rs:461](../../../../crates/coder/src/bin/coder-worker.rs)

**Evidence:** Gateway alone has 6 `unix_now` definitions: 4 in `src`/`bin` and 2 in `tests`. The repo-wide totals (97 `fn hex`, 60 `fn unix_now`, 5 `civil_from_days`) are the reviewer's figures.

**Impact:** The copies can drift apart.

**Suggested action:**
1. Add a small shared utility crate or module for time and encoding, or use the `hex` crate.
2. Replace the copies, starting with gateway (see X-SIZE-03) and then coder.
3. Verify with `git grep -c 'fn unix_now'` and `git grep -c 'fn hex('`: both counts should drop with each pass.

### X-SIZE-25 Game and task tick/sweep functions: verse-world play tick (622 lines), coder autostart sweep (411), gym gate.rs (5.5k lines)

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [verse-world/src/play.rs:2124](../../../../crates/verse-world/src/play.rs)
- [coder/src/task/autostart.rs:1485](../../../../crates/coder/src/task/autostart.rs)
- [gym/src/gate.rs:3285](../../../../crates/gym/src/gate.rs)

**Evidence:** These figures come from the reviewer and were not re-measured. The files are 5,366, 5,381 and 5,533 lines respectively.

**Impact:** Merge conflicts concentrate in these loops, which change often.

**Suggested action:**
1. Split `tick` into ordered per-system calls.
2. Split `sweep` into named phases.
3. Split `gate.rs` into one file per judge under `gate/`.
4. Verify with `cargo test -p verse-world`, `cargo test -p coder task::autostart` and `cargo test -p gym gate`.
