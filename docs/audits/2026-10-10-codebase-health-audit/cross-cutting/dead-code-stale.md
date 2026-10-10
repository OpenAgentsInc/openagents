# Dead code, TODOs, and stale artifacts

**Scope:** The whole repo. Covers `#[allow(dead_code)]` sites, pub items with no outside references, TODO/FIXME/XXX/HACK markers, commented-out code, orphan modules, deprecated code that still compiles, references to retired repos and products, and tracked build or bench artifacts.
**Snapshot:** commit `3168c986aa11e18a8bd30f52c270f609e49b3815`, audit date 2026-10-10.
**Grade: B**

Outside `crates/psionic`, there is very little dead code for a 2.34M-line Rust workspace with 182 packages and 6,704 tracked `.rs` files:

- About one real TODO in production code.
- One commented-out line of code (an import).
- No orphan modules.
- No package without a consumer, a binary or a cdylib target.
- About 120 non-psionic pub declarations that nothing else in the repo references.

Retired products show up only as domain words, not as live integrations. Most of the cost outside psionic is code kept after features were reset or paused:

- dependencies left unused by the 2026-10-08 Cloud reset;
- a dead Cloud effects journal and an archived pilot copy compiled into openagents-web;
- the paused Grid ball/blocks feature, still compiled, with its tests `#[ignore]`d;
- the deprecated microluna crate, still in the default build;
- a live sales-outbox posting path whose suppress and cancel controls have no callers or tests.

The biggest problem is `crates/psionic`, imported on 2026-10-07 as the serving path. It is 1.23M lines. `psionic-serve` links the training, eval and research crates to use about 52 symbols. About 1,000 of its ~1,150 fixture path literals point at files that were never imported. No script or CI lane covers it. Repo hygiene adds about 858 MB of tracked terminal-bench traces and an un-ignored `crates/psionic/target/`. The core workspace alone would grade A-; psionic and the reset leftovers bring it down to B.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC, root workspace | 2,339,780 (182 packages; largest: coder 262k, coder-one 133k, gym 110k, verse-world 93k, openagents-cli 89k) |
| Rust LOC, `crates/psionic` | 1,230,091 (30 crates; psionic-train 417k, psionic-eval 162k, psionic-serve 147k) |
| Tassadar `.rs` lines (psionic) | 388,654 |
| Tracked files | 38,825 (bench 21,894; crates 9,092; docs 3,691; bins 1,032) |
| TODO / FIXME / XXX / HACK (outside vendor) | 36 / 0 / 0 / 1, nearly all in strings or test fixtures. Real TODOs: 1 (`code-highlight` `open_code.rs:217`) |
| Commented-out code | 0 multi-line blocks; 1 commented `use` (`openagents-web` `pages/chat.rs:21`) |
| `#[allow(dead_code)]` | 119 sites (101 non-psionic, including `cfg_attr`; 17 crate-level, all in tests/support) |
| `#[expect(dead_code)]` | 1 |
| `#[allow(unused_*)]` | 16, almost all platform `cfg_attr` |
| `#[deprecated]` | 0 |
| Orphan `.rs` in crate `src` | 0. 1 cross-crate `include!`d test file; 8 uncompiled `.rs` files under docs/audits, bench, bins, quests |
| Pub items declared but never referenced (name appears once in the repo) | 504 of 65,046. psionic 382 (train 172, serve 47, runtime 31); coder 18; claude_agent_sdk 10 |
| Unused normal dependencies (heuristic) | 41 hits. Verified: openagents-web ×5, verse-zone-water ×5. Candidates: coder-one ×2, coder-delegate `reqwest`, retail-qualify `rusqlite` |
| Ignored tests | 174 non-psionic (9 for the disabled Grid ball/blocks); 16 psionic |
| psionic `"fixtures/..."` literals | ~1,150 unique; ~1,000 missing at HEAD (reviewer: 1,003/1,152; verifier: 1,037/1,167); 693 Tassadar |
| psionic-serve → train/eval/research | 52 distinct symbols |
| Files containing `/Users/christopherdavid` | 411 total; 14 lines in psionic `.rs` source |
| `bench/terminal-bench/traces` | 14,744 tracked files, 857.6 MB, 631 runs, 0 with bucket manifests |
| Tracked bench files >1 MB | 208 |
| Git pack size | 8.20 GiB |
| Oldest last-touch of any crate | 2026-09-21 (every crate changed within the last 3 weeks) |

## Strengths

- The workspace clippy lints deny `todo`, `unimplemented` and `dbg_macro` (`Cargo.toml` `[workspace.lints.clippy]`). The one real TODO in product code documents an accepted residual and its cost.
- There is essentially no commented-out code: zero multi-line blocks and one commented `use` line.
- No module is an orphan. Every `.rs` under `crates/*/src` is reached through `mod`, `#[path]` or `include!`, including the `include!`-based test splits such as `crates/coder/src/task/sales.rs:1641` → `funnel_tests.rs`.
- No crate is dead. All 182 packages have a normal dependent, a binary or a cdylib target, and all were touched within 3 weeks.
- Most `#[allow(dead_code)]` sites are narrow `cfg_attr(not(target_os = ...), allow(dead_code))` attributes. Crate-level allows appear only in tests/support modules.
- Deprecations are written down where readers will find them:
  - `crates/microluna/README.md` explains why the deprecated crate stays.
  - `docs/verse/ruins-source-parity.md` opens with a "Removed." banner.
  - `docs/coder-earn.md` is marked as a historical survey.
- Outside psionic, retired products (treasury, nexus, blueprint, lyra, control) do not appear as live integrations. The remaining mentions are domain vocabulary, such as the `sov` profile's `treasury` field and pay-host `treasury_only`.
- A large-file pre-commit guard is in place (`.githooks/pre-commit` → `scripts/dev/check-large-files.sh`). It comes with a bucket-backed artifact flow (`scripts/bench-artifacts.py`) and a written size analysis (`docs/repo/size.md`).
- Ignored tests almost always give a reason, for example `#[ignore = "needs Docker"]`.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-DEAD-01 | High | architecture | psionic-serve compiles ~1M lines of training/Tassadar research code to use ~52 symbols | XL |
| X-DEAD-02 | Medium | dead-code | ~1,000 of ~1,150 psionic fixture paths point at files that were never imported; nothing verifies psionic | L |
| X-DEAD-03 | Medium | build | Unused dependencies left by the Cloud reset and in verse-zone-water | S |
| X-DEAD-04 | Medium | dead-code | openagents-web compiles a dead Cloud effects journal and an archived pilot copy under `#[allow(dead_code)]` | S |
| X-DEAD-05 | Medium | dead-code | Disabled Grid ball/blocks: ~2.6k lines still compiled, 9 tests `#[ignore]`d instead of feature-gated | M |
| X-DEAD-06 | Medium | testing | Public-reply suppress/cancel kill switches have no callers and no tests | S |
| X-DEAD-07 | Medium | repo-hygiene | 858 MB / 14,744 terminal-bench trace files still tracked despite the #11110 bucket policy | M |
| X-DEAD-08 | Medium | repo-hygiene | `crates/psionic/target` is not gitignored | S |
| X-DEAD-09 | Low | dead-code | Deprecated microluna crate is still in the default workspace build through coder-one | M |
| X-DEAD-10 | Low | maintainability | Owner home-directory paths and retired workspace paths are hard-coded in psionic defaults | S |
| X-DEAD-11 | Low | dead-code | ~500 pub items that nothing else references escape rustc's dead_code lint | M |
| X-DEAD-12 | Low | maintainability | `#[allow(dead_code)]` used where `_` fields or `#[expect]` would check themselves; one allow is stale | S |
| X-DEAD-13 | Low | duplication | A test fixture is shared across crates by `include!` of `receipts/tests/support/service_sale.rs` | S |
| X-DEAD-14 | Low | docs | Comments and tests still reference removed code and retired systems | S |
| X-DEAD-15 | Low | dead-code | "Nothing calls this yet" hooks and one commented-out import have no tracking issue | S |
| X-DEAD-16 | Low | repo-hygiene | Unreferenced scripts and uncompiled `.rs` files in docs/bench will rot | S |

### X-DEAD-01 psionic-serve compiles ~1M lines of training/Tassadar research code to use ~52 symbols

**Severity:** High · **Category:** architecture · **Effort:** XL

**Locations**
- [crates/psionic/README.md](../../../../crates/psionic/README.md): README.md:12, README.md:24
- [crates/psionic/crates/psionic-serve/Cargo.toml](../../../../crates/psionic/crates/psionic-serve/Cargo.toml): Cargo.toml:46, :51, :55
- [crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs): tassadar_article_transformer_minimal_frontier.rs:334

**Evidence**
- `psionic-serve/Cargo.toml` `[dependencies]` lists `psionic-eval` (line 46), `psionic-research` (51) and `psionic-train` (55).
- psionic-train is 417k lines and has 46 `tassadar_*` bins.
- psionic-serve has its own `tassadar_*.rs` modules.
- README:12-18 says "The rest of the upstream repository (training programs, research lanes, reports, and its other crates) was not imported". README:24-25 says the subtree is "about a million lines, mostly training and evaluation code that psionic-serve links". The README contradicts itself; it does not hide the linkage.

**Impact:** Every build of `psionic-openai-server`, the network-facing model server on the CUDA box, compiles about a million lines of research code that serving never uses. Builds are slower, the server's code surface is larger, and the "not imported" line misleads readers.

**Suggested action**
1. Move the few types serving needs (`PsionRouteKind`, `PsionRouteClassEvaluationReceipt`, the qwen38 lm-head adapter loader) out of psionic-train into `psionic-models` or a small `psionic-contracts` crate.
2. Put `psionic-serve/src/tassadar*.rs` and the `psion_*` evidence/report modules behind a non-default `research` feature, or delete them.
3. Remove `psionic-train`, `psionic-eval` and `psionic-research` from psionic-serve `[dependencies]`.
4. Delete the psionic-train bins and modules that serving does not reach (`tassadar_*`, `qwen_legal_*`, `cs336_*`, `parameter_golf_*`).
5. Make README:12-18 match README:24-25 and the code that is actually present.
6. Verify:
   - `cargo tree --manifest-path crates/psionic/Cargo.toml -p psionic-serve -e normal | grep psionic-train` prints nothing.
   - A CUDA build succeeds on the 4080 box.

### X-DEAD-02 ~1,000 of ~1,150 psionic fixture paths point at files that were never imported; nothing verifies psionic

**Severity:** Medium · **Category:** dead-code · **Effort:** L

**Locations**
- [crates/psionic/README.md](../../../../crates/psionic/README.md): README.md:15
- [crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs](../../../../crates/psionic/crates/psionic-serve/src/tassadar_article_transformer_minimal_frontier.rs): tassadar_article_transformer_minimal_frontier.rs:334
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh)

**Evidence**
- `git grep -o '"fixtures/[^"]*"' HEAD -- crates/psionic` finds 1,167 unique literals. 1,037 of them are not in `git ls-tree HEAD crates/psionic/fixtures`. The reviewer's stricter count was 1,003 of 1,152.
- Example: `minimal_frontier.rs:334` reads `TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF`. `fixtures/tassadar/reports/tassadar_article_fast_route_throughput_floor_report.json` does not exist at HEAD.
- README:15 says `fixtures/` holds "only the 23 files" that are embedded. `git ls-files crates/psionic/fixtures` returns 208.
- The repo has no `.github/workflows`, and neither `scripts/verify-rust.sh` nor `scripts/verify-changed.py` mentions psionic.

**Impact:** Much of the imported code would fail at run time on a missing file. Nothing runs psionic tests, so the breakage stays hidden, and agents cannot tell live code from stranded code. The subtree is excluded from the root workspace, and the readers of the missing fixtures are research and report paths, not the serving hot path.

**Suggested action**
1. Generate a list of every missing fixture literal and the module that references it.
2. Delete those modules or put them behind the `research` feature from X-DEAD-01.
3. Add a psionic-serve test that checks every `*_REF` constant reachable from serving resolves to an existing file.
4. Add a CPU lane, `cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-serve`, to `scripts/verify-changed.py` for changes under `crates/psionic`.
5. Correct the README fixture count from 23 to the real number.
6. Verify: rerun the grep/ls-tree diff and confirm the number of missing literals reachable from serving is 0.

### X-DEAD-03 Unused dependencies left by the Cloud reset and in verse-zone-water

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations**
- [crates/openagents-web/Cargo.toml](../../../../crates/openagents-web/Cargo.toml): Cargo.toml:23, :25, :26, :30, :53
- [crates/verse-zone-water/Cargo.toml](../../../../crates/verse-zone-water/Cargo.toml)
- [crates/verse-zone-water/src/lib.rs](../../../../crates/verse-zone-water/src/lib.rs): lib.rs:5
- [crates/coder-one/Cargo.toml](../../../../crates/coder-one/Cargo.toml): Cargo.toml:13, :18
- [crates/coder-delegate/Cargo.toml](../../../../crates/coder-delegate/Cargo.toml): Cargo.toml:25

**Evidence**
- openagents-web: `git grep -lw` finds no Rust use of `coder_demo_ui`, `compute_workbench`, `route_contract`, `receipts` or `workbench`. `receipts` appears only in legal `.md` files and `workbench` only in a Dockerfile comment. The crate has no `build.rs`.
- verse-zone-water/src: `verse_content`, `verse_core`, `verse_gfx`, `verse_world` and `verse_zone_grove` each have 0 hits. `verse_zone_everglade`, `verse_pbr`, `physics` and `glam` are used. `verse-content` is pulled in with `features = ["compiler"]`.
- The coder-one, coder-delegate (`reqwest`) and retail-qualify (`rusqlite`) candidates were not re-checked.

**Impact:** The web server and the zone crate build and link crates they do not use, including the verse-content compiler. That costs build time and widens the dependency surface that `docs/dependencies.md` otherwise reviews.

**Suggested action**
1. Remove `coder-demo-ui`, `compute-workbench`, `receipts`, `route-contract` and `workbench` from openagents-web `[dependencies]`.
2. Remove `verse-content`, `verse-core`, `verse-gfx`, `verse-world` and `verse-zone-grove` from verse-zone-water.
3. Grep each coder-one, coder-delegate and retail-qualify candidate, and remove the ones confirmed unused.
4. Add `cargo machete` to `scripts/check-dependencies.sh`, with an ignore list for the intentional wasm `getrandom` entries.
5. Verify with `cargo check -p openagents-web -p verse-zone-water -p coder-one`.

### X-DEAD-04 openagents-web compiles a dead Cloud effects journal and an archived pilot copy under #[allow(dead_code)]

**Severity:** Medium · **Category:** dead-code · **Effort:** S

**Locations**
- [crates/openagents-web/src/cloud/mod.rs](../../../../crates/openagents-web/src/cloud/mod.rs): mod.rs:12, :14, :16
- [crates/openagents-web/src/cloud/effects.rs](../../../../crates/openagents-web/src/cloud/effects.rs)
- [crates/openagents-web/src/pilot.rs](../../../../crates/openagents-web/src/pilot.rs): pilot.rs:283, :396
- [crates/openagents-web/src/pilot/archived.rs](../../../../crates/openagents-web/src/pilot/archived.rs)

**Evidence**
- `cloud/mod.rs:12-17` reads: "Host bindings and their request journal wait for `/environments` ... nothing calls them until it lands." It then declares `#[allow(dead_code)] mod effects;` and `#[allow(dead_code)] pub mod hosts;`.
- `effects.rs` is 1,093 lines and `hosts.rs` is 657.
- `pilot.rs:396` declares `mod archived;` and line 283 imports `ARCHIVED_INSTALL` and `ARCHIVED_OFFER`. These are kept only for the record, behind a route that returns 404.

**Impact:** Dead code ships in the production web binary. `hosts` is partly live, so the module-wide allow on it will hide any new dead code added there.

**Suggested action**
1. Either wire `cloud/effects.rs` into `/environments`, or delete it and restore it from git when it is needed.
2. Replace `#[allow(dead_code)] pub mod hosts` with item-level `#[expect(dead_code, reason = "...")]` on only the unused items.
3. Move the archived pilot HTML to `docs/sales/archive/` as markdown. Then delete `pilot/archived.rs`, its import, the tests that assert on it, and its copy_guard exception.
4. Verify that `cargo build -p openagents-web` produces no dead_code warnings and that `cargo test -p openagents-web` passes.

### X-DEAD-05 Disabled Grid ball/blocks: ~2.6k lines still compiled, 9 tests #[ignore]d instead of feature-gated

**Severity:** Medium · **Category:** dead-code · **Effort:** M

**Locations**
- [crates/verse/src/ball.rs](../../../../crates/verse/src/ball.rs), [crates/verse/src/blocks.rs](../../../../crates/verse/src/blocks.rs), [crates/verse/src/shared.rs](../../../../crates/verse/src/shared.rs)
- [crates/verse/src/runtime.rs](../../../../crates/verse/src/runtime.rs): runtime.rs:233
- [crates/verse/src/session.rs](../../../../crates/verse/src/session.rs), [crates/verse/src/spectator.rs](../../../../crates/verse/src/spectator.rs), [crates/verse/src/zones/tests.rs](../../../../crates/verse/src/zones/tests.rs)
- [crates/coder-mobile/src/verse_app.rs](../../../../crates/coder-mobile/src/verse_app.rs), [crates/coder-mobile/src/verse_surface.rs](../../../../crates/coder-mobile/src/verse_surface.rs), [crates/coder-mobile/src/bare_bodies_tests.rs](../../../../crates/coder-mobile/src/bare_bodies_tests.rs)

**Evidence**
- `ball.rs`, `blocks.rs` and `shared.rs` are 964 + 681 + 969 = 2,614 lines.
- `runtime.rs:233-235` says: "The ball, the blocks (cubes and dominoes), and the pedestal are off for now (owner, 2026-10-01)".
- `git grep -c "the Grid's ball"` finds 9 ignore markers: runtime.rs (2), session.rs (2), spectator.rs, zones/tests.rs, and coder-mobile's verse_app.rs, verse_surface.rs and bare_bodies_tests.rs.

**Impact:** Ignored tests never run, so the paused feature will break without anyone noticing before the owner turns it back on. It also adds compile time to the hot `verse` crate.

**Suggested action**
1. Put `ball.rs`, `blocks.rs` and the shared-bodies path behind a `grid-bodies` feature in `verse`, and forward the feature from coder-mobile.
2. Replace the 9 `#[ignore]` attributes with `#[cfg(feature = "grid-bodies")]`.
3. Run `cargo test -p verse --features grid-bodies` in verify-changed whenever those files change.
4. If the owner does not plan to restore the feature, delete the modules instead.
5. Verify that `cargo test -p verse` and `cargo test -p verse --features grid-bodies` both pass, and that `git grep "the Grid's ball"` returns no `#[ignore]` hits.

### X-DEAD-06 Public-reply suppress/cancel kill switches have no callers and no tests

**Severity:** Medium · **Category:** testing · **Effort:** S

**Locations**
- [crates/coder/src/task/sales/outbox/public_reply.rs](../../../../crates/coder/src/task/sales/outbox/public_reply.rs): public_reply.rs:458, :616, :654
- [NEEDS_OWNER.md](../../../../NEEDS_OWNER.md): NEEDS_OWNER.md:2012

**Evidence**
- `git grep -nw` finds `public_reply_suppress` and `public_reply_cancel` only at their declarations.
- `public_reply_dispatch_live` appears only at its declaration and in `NEEDS_OWNER.md:2017`.
- `NEEDS_OWNER.md:2012-2025` documents that live posting is deliberately gated on an owner-recorded live grant and a dedicated agent GitHub account. The live path waiting for a grant is intended; the gap is that the safety controls have never been run.

**Impact:** Once the owner records a live grant, the company will post publicly while relying on suppress and cancel controls that have never been exercised.

**Suggested action**
1. Before any live grant is recorded, add tests in `crates/coder/src/task/sales/agents/tests.rs`:
   - Suppress an account, then assert that its Proposed and Approved replies become Invalidated.
   - Cancel a reply, then assert that a later `dispatch_fixture` refuses it.
2. Wire operator commands for suppress and cancel next to the live dispatch command, and link #10881.
3. Verify that `cargo test -p coder public_reply` passes and that `git grep -nw public_reply_suppress` shows at least one caller besides the declaration.

### X-DEAD-07 858 MB / 14,744 terminal-bench trace files still tracked despite the #11110 bucket policy

**Severity:** Medium · **Category:** repo-hygiene · **Effort:** M

**Locations**
- [bench/terminal-bench/traces](../../../../bench/terminal-bench/traces)
- [docs/repo/size.md](../../../../docs/repo/size.md): size.md:13, :74, :150
- [scripts/dev/large-files-allowlist.txt](../../../../scripts/dev/large-files-allowlist.txt): large-files-allowlist.txt:51

**Evidence**
- `git ls-files bench/terminal-bench/traces | wc -l` returns 14,744 files.
- They total 857.6 MB by stat across 631 run directories (the reviewer measured 817.8 MB). None of the runs has a `bench-artifacts.json` manifest.
- `docs/repo/size.md:13` says "Bench captures ... are no longer in the tree (#11110)". The same file lists traces at 0.86 GB (:74) and names traces as content that belongs in the bucket (:150).
- `large-files-allowlist.txt:51-53` grandfathers the trace files.

**Impact:** Clones are slow, greps over the tree are noisy, and the docs claim a cleanup that has not happened for traces.

**Suggested action**
1. Run `scripts/bench-artifacts.py push` for each `bench/terminal-bench/traces/<run>`.
2. `git rm --cached` the payloads, keeping each run's manifest and `.gitignore`.
3. Remove the matching entries from `scripts/dev/large-files-allowlist.txt`.
4. Correct `size.md:13` so it does not claim traces are out of the tree until steps 1-3 are done.
5. Verify:
   - `git ls-files bench/terminal-bench/traces | wc -l` drops to about the 631 manifests plus ignore files.
   - Restoring one run recreates its payload.

### X-DEAD-08 crates/psionic/target is not gitignored

**Severity:** Medium · **Category:** repo-hygiene · **Effort:** S

**Locations**
- [.gitignore](../../../../.gitignore): .gitignore:1, :2

**Evidence:** Lines 1-2 of `.gitignore` are `/target` and `/crates/openagents-mobile/target/`. `git check-ignore -v crates/psionic/target` exits 1, meaning the directory is not ignored, and `git status` shows `?? crates/psionic/target/`.

**Impact:** Any agent running a broad `git add` would stage the psionic build output. Most of those files are under 1 MB, so the large-file hook would not stop them.

**Suggested action**
1. Add `/crates/psionic/target/` after `.gitignore` line 2.
2. Verify that `git check-ignore -v crates/psionic/target` prints the new rule and that `git status` no longer lists the directory.

### X-DEAD-09 Deprecated microluna crate is still in the default workspace build through coder-one

**Severity:** Low · **Category:** dead-code · **Effort:** M

**Locations**
- [crates/microluna/Cargo.toml](../../../../crates/microluna/Cargo.toml): Cargo.toml:6
- [crates/coder-one/src/accept/minitask.rs](../../../../crates/coder-one/src/accept/minitask.rs): minitask.rs:14
- [crates/coder-one/src/accept/mod.rs](../../../../crates/coder-one/src/accept/mod.rs): mod.rs:450, :1317
- [Cargo.toml](../../../../Cargo.toml): Cargo.toml:2

**Evidence**
- microluna's description reads "Deprecated on 2026-09-28; Microcoder replaced it."
- coder-one is its only dependent (`git grep -ln microluna -- 'crates/*/Cargo.toml'`).
- coder-one uses `microluna::run`, `Recorder` and `Workspace` in `minitask.rs`, and `microluna::Evidence` and `Brief` in `accept/mod.rs` (lines 119, 450, 1023, 1044, 1317).
- The root `Cargo.toml` sets `members` but no `default-members`.
- The crate is kept on purpose so recorded evidence can be reproduced, which is why this is rated Low.

**Impact:** Deprecated code keeps costing maintenance and compile time, and coder-one's acceptance path still depends on its types.

**Suggested action**
1. Tag the last commit whose recorded evidence used microluna, and point the README reproduction steps at that tag.
2. Move `Evidence` and `Brief` into coder-one.
3. Remove the minitask path and the microluna crate. If that is not possible yet, at least add `default-members` that leaves out microluna, coderbench and the bench-only crates.
4. Verify that `cargo tree -i microluna` returns nothing and that `cargo test -p coder-one` passes.

### X-DEAD-10 Owner home-directory paths and retired workspace paths are hard-coded in psionic defaults

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations**
- [crates/psionic/crates/psionic-models/src/lib.rs](../../../../crates/psionic/crates/psionic-models/src/lib.rs): lib.rs:10219
- [crates/psionic/crates/psionic-serve/src/gguf.rs](../../../../crates/psionic/crates/psionic-serve/src/gguf.rs): gguf.rs:14966
- [crates/psionic/crates/psionic-eval/examples/qwen35_legal_mlx_lora_harvey_no_cheat_suite.rs](../../../../crates/psionic/crates/psionic-eval/examples/qwen35_legal_mlx_lora_harvey_no_cheat_suite.rs): qwen35_legal_mlx_lora_harvey_no_cheat_suite.rs:23
- [crates/psionic/crates/psionic-eval/examples/qwen35_legal_mlx_lora_harvey_mfn_slice.rs](../../../../crates/psionic/crates/psionic-eval/examples/qwen35_legal_mlx_lora_harvey_mfn_slice.rs): qwen35_legal_mlx_lora_harvey_mfn_slice.rs:20
- [crates/psionic/crates/psionic-serve/src/psion_rvllm_direct_engine_comparator.rs](../../../../crates/psionic/crates/psionic-serve/src/psion_rvllm_direct_engine_comparator.rs): psion_rvllm_direct_engine_comparator.rs:91

**Evidence:** `git grep '/Users/christopherdavid' HEAD -- crates/psionic/crates '*.rs'` returns 14 lines. Examples:
- `lib.rs:10219`: `/Users/christopherdavid/models/qwen3.5/Qwen3.5-27B-Q4_K_M.gguf`
- `gguf.rs:14966`: `/Users/christopherdavid/code/llama.cpp/build/bin/llama-server`
- Both harvey examples: `/Users/christopherdavid/work/competition/repos/harvey-labs/tasks`
- `psion_rvllm_direct_engine_comparator.rs:91`: `competition/repos/rvllm/...`, which is the retired `competition/` lane

**Impact:** These defaults fail without a clear error on CoderOS, the 4080 box and every other machine, and they expose the owner's filesystem layout in a public repo.

**Suggested action**
1. Replace each default with an env var (`PSIONIC_MODEL_PATH`, `LLAMA_SERVER_BIN`, `HARVEY_TASKS_ROOT`), and skip with a clear message when the variable is unset.
2. Change `competition/repos` to `projects/repos`, or delete the harvey and rvllm comparator code as part of the X-DEAD-01 cleanup.
3. Verify that `git grep -n '/Users/christopherdavid\|competition/repos' -- crates/psionic '*.rs'` returns nothing.

### X-DEAD-11 ~500 pub items that nothing else references escape rustc's dead_code lint

**Severity:** Low · **Category:** dead-code · **Effort:** M

**Locations**
- [crates/coder/src/router/bank.rs](../../../../crates/coder/src/router/bank.rs): bank.rs:547
- [crates/coder/src/task/capacity.rs](../../../../crates/coder/src/task/capacity.rs): capacity.rs:86
- [crates/jev/src/client.rs](../../../../crates/jev/src/client.rs): client.rs:1179
- [crates/boat/src/lib.rs](../../../../crates/boat/src/lib.rs): lib.rs:32
- [crates/nostr/src/cap.rs](../../../../crates/nostr/src/cap.rs): cap.rs:1045
- [crates/tenancy/src/billing.rs](../../../../crates/tenancy/src/billing.rs): billing.rs:709
- [crates/tenancy/src/workspaces.rs](../../../../crates/tenancy/src/workspaces.rs): workspaces.rs:875

**Evidence:** The reviewer's token scan found 504 of 65,046 pub declarations whose name occurs only once in the repo: 382 in psionic and about 120 elsewhere (coder 18, claude_agent_sdk 10, boat 6, jev 5, nostr 4, tenancy 2). The full scan was not re-run during verification.

**Impact:** rustc never warns about unused pub items in library crates, so these build up with no callers and no tests.

**Suggested action**
1. For each non-psionic item, delete it or narrow it to `pub(crate)`. Keep the claude_agent_sdk items only if `scripts/check-claude-sdk-parity.sh` needs them.
2. Handle the psionic items as part of the X-DEAD-01 research-lane cleanup.
3. Verify that `cargo check --workspace` passes with no new dead_code warnings from the narrowed items, then delete whatever they surface.

### X-DEAD-12 #[allow(dead_code)] used where `_` fields or #[expect] would check themselves; one allow is stale

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations**
- [crates/coder-compositor/src/state.rs](../../../../crates/coder-compositor/src/state.rs): state.rs:99, :103
- [crates/verse-pbr/src/water/ocean.rs](../../../../crates/verse-pbr/src/water/ocean.rs): ocean.rs:547

**Evidence**
- `coder-compositor/state.rs` puts bare allows on Wayland keep-alive fields such as `decoration_state` and `text_input_state`.
- `ocean.rs:547` allows dead_code on `Inline(Option<Synthesis>)`, but that variant is constructed at :590 and matched at :690 and :952, so the allow is stale.
- `expect(dead_code` appears only once in the repo, in `openagents-web/src/promises.rs`.

**Impact:** Stale allows stay in place indefinitely and can hide real dead code added later.

**Suggested action**
1. Rename the keep-alive fields to `_decoration_state`, `_text_input_state` and so on, and remove their allows.
2. Remove the allow at `ocean.rs:547`.
3. Convert the remaining allow(dead_code) attributes that are not `cfg_attr` to `#[expect(dead_code, reason = "...")]`.
4. Verify that `cargo check -p coder-compositor -p verse-pbr` is clean with no unfulfilled-expectation warnings.

### X-DEAD-13 A test fixture is shared across crates by include! of receipts/tests/support/service_sale.rs

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations**
- [crates/receipts/tests/support/service_sale.rs](../../../../crates/receipts/tests/support/service_sale.rs): service_sale.rs:1
- [crates/coder/src/task/sales/partners/tests.rs](../../../../crates/coder/src/task/sales/partners/tests.rs): tests.rs:9
- [crates/gym/src/sales_finance_tests.rs](../../../../crates/gym/src/sales_finance_tests.rs): sales_finance_tests.rs:9
- [crates/openagents-cli/tests/sales_service.rs](../../../../crates/openagents-cli/tests/sales_service.rs): sales_service.rs:14

**Evidence:** `crates/receipts/tests` contains only `support/service_sale.rs`. `git grep service_sale.rs` returns 7 references from other crates, all of which paste the file in textually with `include!`.

**Impact:** No crate type-checks the fixture on its own, so a receipts API change breaks three other crates instead of receipts.

**Suggested action**
1. Move the file to `crates/receipts/src/test_support/service_sale.rs` behind a `test-support` feature.
2. Add `receipts` with `features = ["test-support"]` to the dev-dependencies of coder, gym and openagents-cli.
3. Replace each `include!` with `use receipts::test_support::service_sale::*`.
4. Verify that `git grep 'service_sale.rs'` returns no `include!` hits and that `cargo test -p coder -p gym -p openagents-cli` passes.

### X-DEAD-14 Comments and tests still reference removed code and retired systems

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations**
- [crates/knowledge/src/codebase.rs](../../../../crates/knowledge/src/codebase.rs): codebase.rs:791
- [crates/verse-world/src/rules.rs](../../../../crates/verse-world/src/rules.rs): rules.rs:2
- [crates/psionic/crates/psionic-train/src/qwen_legal_pylon_training_job.rs](../../../../crates/psionic/crates/psionic-train/src/qwen_legal_pylon_training_job.rs): qwen_legal_pylon_training_job.rs:21, :58, :225

**Evidence**
- `codebase.rs:791` uses `crates/verse-ruins/vendor/...` as a test path, but `crates/verse-ruins` no longer exists.
- `rules.rs:2` says "The retained Ruins adapter is a parity reference", though the adapter has been removed.
- `qwen_legal_pylon_training_job.rs` mentions the retired systems three times:
  - :21: "Pylon legal training Treasury/Nexus payable batch schema"
  - :58: "settlement state returned by Treasury or Nexus"
  - :225: "Payout authorization id generated by Psionic for Treasury/Nexus"

**Impact:** These comments point readers and agents at systems that have been retired or removed.

**Suggested action**
1. Change the knowledge test path to a vendored path that exists.
2. Reword `rules.rs:2` as "Ported from the removed Ruins adapter (95c938edd4^)".
3. Delete the `qwen_legal_*` modules as part of the X-DEAD-01 cleanup, or reword those comments to name the openagents.com Worker / MDK bridge.
4. Verify that `git grep -n 'verse-ruins\|Treasury/Nexus\|Treasury or Nexus'` returns nothing in `.rs` files.

### X-DEAD-15 "Nothing calls this yet" hooks and one commented-out import have no tracking issue

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations**
- [crates/verse-zone-everglade/src/zones/everglade/mod.rs](../../../../crates/verse-zone-everglade/src/zones/everglade/mod.rs): mod.rs:1265
- [crates/openagents-web/src/pages/chat.rs](../../../../crates/openagents-web/src/pages/chat.rs): chat.rs:19

**Evidence**
- everglade `mod.rs:1265`: "The sales floor rings it when the payment ledger records a settled deal; nothing calls this yet."
- `chat.rs:19-21`: "// Disabled until the composer's context, model and voice controls do something ... // use openagents_ui::shell::{ComposerAction, ModelPickerTrigger};"

**Impact:** Neither has an issue link, so nobody owns wiring them up or removing them.

**Suggested action**
1. Delete the commented `use` at `chat.rs:21`.
2. Add a tracking issue in project 22 to the `ring_agora_bell` doc comment, or remove the hook.
3. Verify that `cargo check -p openagents-web -p verse-zone-everglade` passes.

### X-DEAD-16 Unreferenced scripts and uncompiled .rs files in docs/bench will rot

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations**
- [scripts/dev/issue-board.test.sh](../../../../scripts/dev/issue-board.test.sh)
- [docs/audits/2026-09-19-codebase-audit/reproduce.rs](../../../../docs/audits/2026-09-19-codebase-audit/reproduce.rs)

**Evidence:** According to the reviewer, four scripts are referenced nowhere, and eight `.rs` files under docs/audits, bench, bins and quests sit outside any Cargo target. This was not re-measured during verification.

**Impact:** These files break without anyone noticing when the APIs they use change.

**Suggested action**
1. Wire `issue-board.test.sh` into a test runner, or delete it.
2. In each audit README that ships a `.rs` file, record the commit the file compiles against.
3. Verify by re-running the reference scan; every remaining script should have a caller or a documented manual-use note.

## Refuted during verification

- **"The one real TODO causes an O(lines²) clone per streamed chunk in code-highlight, so long fenced outputs get slower over time."** Rejected. The TODO at `crates/code-highlight/.../open_code.rs:217-219` goes on to say that it only copies precomputed style spans, that the expensive syntect parse/highlight is already O(N) in total, and that the cost "is tracked as an accepted residual, not a regression." It is a documented trade-off, not neglected debt.
- **"openagents-web README:220 points at a missing file."** Rejected. `src/pilot.rs` exists, declares `mod archived` and imports both archived constants, so the README pointer is accurate. It was dropped from X-DEAD-04 and X-DEAD-14.
