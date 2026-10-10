# Lints, build config, compile time

**Scope:** Repo-wide [X-LINT]: lint opt-in, `#[allow]` use, edition/rust-version consistency, build scripts, profile overrides, features, compile-time hotspots, rustfmt and cargo-deny coverage and enforcement.
**Health grade:** C
**Snapshot:** commit `3168c986aa11e18a8bd30f52c270f609e49b3815`, audit date 2026-10-10.

The build and lint foundation is sound. The toolchain is pinned to 1.97.1, rustfmt uses style_edition 2024, and 174 of 182 root-workspace crates set `[lints] workspace = true`. deny.toml has per-version license exceptions and an advisory waiver with an expiry date. Every profile override has a comment explaining it. Several of these controls, however, either do nothing or are never enforced. `cargo deny --offline --locked check sources licenses bans` fails at HEAD on all three checks: 32 source-not-allowed errors (the 7 git sources are not in an allow-git list), 15 license errors and 1 wildcard error, plus 115 duplicate-version warnings. Nothing runs the gate automatically. There is no CI workflow, the pre-commit hook only checks file size, and `verify-rust.sh` treats a missing cargo-deny as "partial" and leaves `deps` out of its default phases. The mobile app, psionic, mc-bridge, the doc/deploy crates and 6 more lockfiles are never checked at all.

Lint hygiene has a lot of no-op noise. There are 449 `#[allow]` attributes (503 counting `cfg_attr` forms) and no `#[expect]`. About 151 of the allows name pedantic or restriction lints that the root workspace never enables, so they suppress nothing and make it harder to see which allows still matter. The root lint baseline is 3 clippy denies plus 1 rustc deny. A few very large crates drive compile time. coder has 262K LOC, and its build script watches `.git/index`, so it reruns on every `git add`; verse-host does the same. The Verse game links coder and gym through its default features. openagents-cli pulls in 873 packages, including three Lightning stacks and four secp256k1 versions. The 26 hand-maintained dev `opt-level=3` overrides already miss 7 verse crates.

## Measurements

| Metric | Value |
|---|---|
| Root workspace packages | 182 (184 dirs under `crates/`; psionic and openagents-mobile are separate workspaces) |
| Rust LOC outside psionic | 2,339,780 |
| Largest crates (LOC) | coder 262,296; coder-one 132,534; gym 110,396; verse-world 93,199; openagents-cli 89,408; nostr 84,833; gateway 72,380; verse 72,307 |
| Largest files (lines) | coder/src/runtime.rs 9,576; coder/src/bin/coder-worker.rs 7,065; openagents-desktop/src/shell.rs 6,756 |
| Crates without `[lints] workspace = true` | 8: bitcoin-amount, oa-copy, openagents-ui, rust-native, wallet, x402, plus the openagents-mobile and psionic workspaces |
| Crates with a hard-coded edition | 9 |
| Crates missing rust-version | 5: paper-mono, spark-wallet, wallet, x402, psionic |
| `#[allow]` attributes | 449 (503 including `cfg_attr`); 0 `#[expect]`; 1 with a reason |
| Top allows | too_many_arguments 196; dead_code 60 (103 including `cfg_attr`); cast_precision_loss 50; too_many_lines 41 |
| Allows naming lints that are off by default | ~151 |
| Crate-level `#![allow]` | 28, all in test support or test modules except openagents-desktop's `cfg_attr` for menubar/appmenu |
| `unsafe` sites vs `SAFETY` comments | 793 vs 476 |
| `libc::flock` / `libc::geteuid` / `fs2` | 10 sites in 8 files / 71 calls in 38 files / 4 crates |
| External dependency declarations | 1,330 across 152 crates; only 3 (iroh*) use `[workspace.dependencies]` |
| Crates declared with conflicting requirements | 22 (e.g. sha2 0.10 x71 vs 0.11 x13) |
| Cargo.lock | 1,567 packages; 146 names with more than one version |
| Transitive packages | openagents-cli 873 (117 in-workspace); verse 640; openagents-desktop 636; coder 505 (57 in-workspace) |
| Dev profile `opt-level=3` overrides | 26 |
| Build scripts | 9 (4 are git stamps) |
| Non-default features | 121; never enabled anywhere: verse-bake/gpu, bunny-web/autoplay |
| Tracked Cargo.lock files | 14 |
| cargo-deny at HEAD (offline) | sources 32 errors; licenses 15 errors; bans 1 error + 115 duplicate warnings |
| CI workflows | 0 |

## Strengths

- The toolchain is pinned exactly (`rust-toolchain.toml` channel 1.97.1, minimal profile, clippy and rustfmt), `rust-version = 1.97.1` is set at workspace level, and `rustfmt.toml` pins style_edition 2024. Formatting is deterministic.
- Workspace lints are widely adopted: 174 of 182 root members use `[lints] workspace = true`. The baseline denies `dbg_macro`, `todo`, `unimplemented` and `unsafe_op_in_unsafe_fn`.
- deny.toml is strict and documented. It uses per-version license exceptions instead of graph-wide allows, sets unmaintained/unsound = all and unused-ignored-advisory = deny, and has an advisory waiver that expires (enforced by `scripts/check-dependencies.sh:7`).
- Every profile override and root Cargo.toml decision has a comment with the measured reason (e.g. sha2 is 18x slower unoptimized, Cargo.toml:42-46), so each one can be re-measured.
- The git-stamp build scripts (`crates/coder/build.rs`) prefer exact stamps passed by the install script through env vars and fall back to git. Their comments explain the limits.
- `verify-rust.sh` records pass/skip/partial per phase instead of a bare pass, and `--changed` scopes it to changed crates, which suits a 2.3M-LOC repo.
- Each excluded workspace (openagents-mobile, psionic, mc-bridge) documents why it is separate: an sqlite link conflict, CUDA/Metal, and nightly azalea.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-LINT-01 | High | build | The cargo-deny gate fails at HEAD: 7 git sources not allowed, 15 license errors, 1 wildcard error | M |
| X-LINT-02 | High | build | coder and verse-host build scripts watch `.git/index`, so every `git add` rebuilds the biggest crates | S |
| X-LINT-03 | Medium | build | Nothing enforces fmt or deny automatically: no CI, size-only pre-commit hook, deps phase is opt-in | S |
| X-LINT-04 | Medium | maintainability | ~150 `#[allow]` entries name lints the root workspace never enables, so they do nothing | M |
| X-LINT-05 | Medium | architecture | The Verse game links coder (262K) and gym (110K) through default features | L |
| X-LINT-06 | Medium | build | Dev `opt-level=3` overrides miss 7 verse crates and don't reach the mobile workspace | S |
| X-LINT-07 | Medium | build | Three Lightning stacks and four secp256k1 versions in one graph | L |
| X-LINT-08 | Medium | build | Only 3 external deps use `[workspace.dependencies]`; sha2 is split 0.10/0.11 | M |
| X-LINT-09 | Medium | policy | 6 root crates bypass workspace lints; wallet and x402 have no lints at all | S |
| X-LINT-10 | Medium | duplication | Three file-locking mechanisms and 71 hand-rolled unsafe `geteuid` calls | S |
| X-LINT-11 | Medium | policy | openagents-mobile, mc-bridge and docs runner crates are outside the deny policy; a runner uses an absolute path | S |
| X-LINT-12 | Medium | build | The dependency gate will hard-fail on 2026-10-20 (paste advisory review date) | S |
| X-LINT-13 | Low | maintainability | 196 `allow(clippy::too_many_arguments)`: long parameter lists instead of structs | M |
| X-LINT-14 | Low | dead-code | openagents-web cloud effects/hosts modules (1,093+ lines) parked behind `allow(dead_code)` | S |
| X-LINT-15 | Low | build | Root workspace uses resolver "2"; MSRV-aware resolver 3 is not enabled | S |
| X-LINT-16 | Low | dead-code | Orphan `check-cfg(kani)` and the never-enabled verse-bake/gpu feature | S |
| X-LINT-17 | Low | build | Debug-info cost handled with env vars in mobile scripts; no explicit dev debug or release profile | S |
| X-LINT-18 | Low | repo-hygiene | Invalid SPDX license id "CC-0" in three crates | S |
| X-LINT-19 | Low | repo-hygiene | Redundant workspace member entry; a non-build module named build.rs | S |
| X-LINT-20 | Low | build | The EVM alloy stack reaches openagents-cli through vendored boltz-client and Breez Spark | M |

### X-LINT-01 The dependency policy gate (cargo-deny) fails at HEAD: 7 git sources not allowed, 15 license errors, 1 wildcard error

**Severity:** High · **Category:** build · **Effort:** M

**Locations:**
- [deny.toml](../../../../deny.toml) (deny.toml:55-58, deny.toml:20-46)
- [scripts/check-dependencies.sh](../../../../scripts/check-dependencies.sh) (check-dependencies.sh:19)
- [Cargo.lock](../../../../Cargo.lock)
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml)
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml)
- [docs/dependencies.md](../../../../docs/dependencies.md) (dependencies.md:282-288)

**Evidence:** `cargo +1.97.1 deny --offline --locked check sources` (cargo-deny 0.20.2) reports 32 `error[source-not-allowed]`. `check licenses bans` reports 11 `error[unlicensed]`, 4 `error[rejected]`, 1 `error[wildcard]` and 115 `warning[duplicate]`, plus 12 `warning[no-license-field]` and 11 `warning[unlicensed]`. deny.toml `[sources]` sets `unknown-git = "deny"` and has only `allow-registry`, with no `allow-git`. The verifier re-ran cargo-deny at HEAD and got the same counts.

**Impact:** The release dependency gate cannot pass, so in practice it is off. Unreviewed git sources and licenses, including copyleft MPL-2.0, reach the shipped wallet and payment binaries and nothing reports them.

**Suggested action:**
1. Add `[sources] allow-git = [...]` to deny.toml, listing each reviewed repo exactly: breez/spark-sdk, moneydevkit/rust-lightning, moneydevkit/ldk-node, moneydevkit/bitcoin-payment-instructions, lightningdevkit/rust-lightning, lightsparkdev/frost, arik-so/rust-musig2. Give each one a comment.
2. Add `[[licenses.clarify]]` entries for the unlicensed Breez, Spark and vendored crates.
3. Review the MITNFA, MIT-0 and MPL-2.0 crates, then add per-version `exceptions` for them.
4. Fix the wildcard version in the vendored crate.
5. Verify: `./scripts/check-dependencies.sh` exits 0. Update docs/dependencies.md.

### X-LINT-02 coder (262K LOC) and verse-host build scripts watch .git/index, so every `git add` re-runs them and recompiles coder/verse and their dependents

**Severity:** High · **Category:** build · **Effort:** S

**Locations:**
- [crates/coder/build.rs](../../../../crates/coder/build.rs) (build.rs:29-38)
- [crates/coder-new/build.rs](../../../../crates/coder-new/build.rs)
- [crates/retail-service/build.rs](../../../../crates/retail-service/build.rs)
- [crates/verse-host/build.rs](../../../../crates/verse-host/build.rs) (build.rs:28-41)

**Evidence:** crates/coder/build.rs:29 loops `for path in ["HEAD", "index", "packed-refs"]` and calls `watch(...)` on each, and it also watches the branch ref. verse-host/build.rs:34 loops `for name in ["index", "packed-refs"]` and also emits `rerun-if-changed` for HEAD and the branch ref. There are 9 build.rs files in total.

**Impact:** Many agents stage files constantly in this checkout. Each `git add` makes the next build re-run the script and recompile coder, the largest crate in the repo, and everything that depends on it (verse, the CLI, desktop). The script's only output is a version string. Commits also trigger a rebuild, because HEAD and the branch ref are watched.

**Suggested action:**
1. Stop watching `index`. Take the dirty flag from `CODER_BUILD_DIRTY` (`scripts/install-coder.sh` already sets it), or report "unknown" in dev builds.
2. Move the stamp into a small leaf crate, e.g. `crates/build-stamp` with `pub const COMMIT: &str = env!(..)`, and make coder, coder-new, retail-service and verse-host depend on it. After a commit only that crate reruns, and the four copied scripts become one.
3. Verify: run `cargo build -p coder`, then `git add` any file, then build again. coder should not recompile. After a commit, only build-stamp and the link steps should rerun.

### X-LINT-03 No automated enforcement of fmt or deny: no CI, the pre-commit hook only checks file size, and the deps phase is opt-in and skippable

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations:**
- [.githooks/pre-commit](../../../../.githooks/pre-commit) (pre-commit:1-4)
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh) (verify-rust.sh:24, verify-rust.sh:393-399)

**Evidence:** `git ls-files .github` lists only `ISSUE_TEMPLATE/playtest-report.yml`. The pre-commit hook execs `scripts/dev/check-large-files.sh` and nothing else. verify-rust.sh:396-398 records `cargo-deny is not installed, so this is a partial gate` as a skip, not a failure.

**Impact:** Many agents commit to main, and policy drift such as the failing deny gate (X-LINT-01) goes unnoticed.

**Suggested action:**
1. Keep this cheap, since the owner has chosen lean verification with no clippy or gates by default. Add a push-to-main check (GitHub Actions or a Cloud Build trigger) that runs only `cargo fmt --all --check` and `cargo deny --locked check sources licenses bans`. Neither compiles anything.
2. Under `--release`, make a missing cargo-deny a failure in verify-rust.sh.
3. Do not add a clippy CI requirement unless the owner asks for one.
4. Verify: push a commit with a formatting error and confirm the check fails.

### X-LINT-04 ~150 #[allow] entries name pedantic or restriction lints the root workspace never enables, so they do nothing

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:37-40)
- [crates/pylon/src/field/tests.rs](../../../../crates/pylon/src/field/tests.rs) (tests.rs:1)
- [crates/pylon/Cargo.toml](../../../../crates/pylon/Cargo.toml) (Cargo.toml:44-45)
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml) (Cargo.toml:10)

**Evidence:** Root `[workspace.lints.clippy]` contains only `dbg_macro`, `todo` and `unimplemented`. No root crate has a crate-level `#![warn/deny(clippy::pedantic|unwrap_used)]`, and there is no clippy.toml. The only pedantic or unwrap_used configuration is in crates/psionic/Cargo.toml:10/41. Recounted allows: cast_precision_loss 50, too_many_lines 41, trivially_copy_pass_by_ref 16, unwrap_used 13, unnecessary_wraps 10, expect_used 7. There are no `#[expect(` attributes. pylon/src/field/tests.rs:1 has `#![allow(clippy::unwrap_used, clippy::expect_used)]`, but pylon only has `[lints] workspace = true`. Counting `cfg_attr` forms, the verifier found 503 allow attributes in total.

**Impact:** Readers wrongly assume these lints are enforced (for example, that pylon production code has no unwrap). The no-op allows also hide the allows that suppress real warnings.

**Suggested action:**
1. Delete the no-op allows (cast_*, too_many_lines, trivially_copy_pass_by_ref, unnecessary_wraps, unused_async) in one scripted pass.
2. Where no-unwrap is actually wanted (pylon, nostr, x402, wallet, pay-ledger), add a root clippy.toml with `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`, and put `#![deny(clippy::unwrap_used, clippy::expect_used)]` in each of those crates' lib.rs.
3. Add `allow_attributes_without_reason = "warn"` to `[workspace.lints.clippy]` so new suppressions are written as `#[expect(lint, reason = ..)]`.
4. Verify: `rg -c 'allow\(clippy::(cast_precision_loss|too_many_lines|trivially_copy_pass_by_ref|unnecessary_wraps|unused_async)' crates --glob '!crates/psionic/**'` returns nothing.

### X-LINT-05 The verse game links the 262K-LOC coder crate and the 110K-LOC gym through default features

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [crates/verse/Cargo.toml](../../../../crates/verse/Cargo.toml) (Cargo.toml:10-15)

**Evidence:** `default = ["desktop"]`, and `desktop` includes `"model-host"` (dep:coder, dep:coder-delegate), `"replay-host"` (dep:gym, dep:knowledge), `"terminal"` (workbench, compute-workbench, contribution-workbench, ...) and `"panels"`. The reviewer measured 640 transitive packages for verse; the verifier did not re-measure that figure.

**Impact:** Every default Verse build compiles and links most of the product, and any edit to coder invalidates the game build.

**Suggested action:**
1. Move `model-host`, `replay-host` and `terminal` out of `desktop` into an opt-in `agent-host` feature.
2. Enable `agent-host` from openagents-desktop and from the verse-play alias.
3. Track `cargo tree -p verse --prefix none | sort -u | wc -l` as the regression metric. Verify it drops, and that `cargo tree -p verse -i coder` reports nothing with default features.

### X-LINT-06 The hand-maintained dev opt-level=3 overrides already miss 7 verse crates and don't apply to the mobile workspace

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:44-115)
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) (Cargo.toml:122)

**Evidence:** These 7 crates have a Cargo.toml but no profile override: verse-game, verse-lagrange, verse-private, verse-water-spells, verse-zone-coast, verse-zone-crypt, verse-zone-water. crates/openagents-mobile/Cargo.toml has no `[profile]` section, and a separate workspace does not inherit the root profiles.

**Impact:** Frame-time measurements depend on which zone is loaded, and the list goes stale every time a verse crate is added.

**Suggested action:**
1. Add `[profile.dev.package."*"] opt-level = 2` for third-party dependencies, and keep explicit overrides only for workspace crates.
2. Alternatively, add a check to `scripts/verify-changed.py` that every `crates/verse*` and physics member has an override.
3. Mirror the profile in crates/openagents-mobile/Cargo.toml, with a comment.
4. Verify: a script that lists `crates/verse*` members and diffs them against `[profile.dev.package.*]` keys reports no gaps.

### X-LINT-07 Three separate Lightning stacks and four secp256k1 versions in one binary graph

**Severity:** Medium · **Category:** build · **Effort:** L

**Locations:**
- [Cargo.lock](../../../../Cargo.lock)
- [deny.toml](../../../../deny.toml) (deny.toml:63-68)

**Evidence:** Cargo.lock contains lightning 0.1.13/0.2.5/0.3.0+git; lightning-invoice 0.33.3/0.34.0+git/0.34.1 (x2); secp256k1 0.27.0/0.29.1/0.30.0/0.31.1; tokio-tungstenite 0.28/0.29/0.30; axum 0.7.9/0.8.9; reqwest 0.12.28/0.13.5; syn 1/2/3. In total 146 names have duplicate versions. deny.toml sets `multiple-versions = "warn"`.

**Impact:** Longer clean builds, larger binaries, and key types that cannot be passed between the Spark wallet and the LDK wallet.

**Suggested action:**
1. Record each duplicate Lightning and secp256k1 stack in docs/dependencies.md, with an owner and an exit condition.
2. Add `[bans] skip` entries for the reviewed duplicates, and work toward `multiple-versions = "deny"`.
3. Converge the repo's own dependency choices on one tokio-tungstenite and one reqwest.
4. Verify: `grep -c '^name = "secp256k1"' Cargo.lock` and the cargo-deny duplicate warning count go down over time.

### X-LINT-08 Only 3 external dependencies use [workspace.dependencies]; sha2 is split 0.10/0.11

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:15-22)
- [Cargo.lock](../../../../Cargo.lock)

**Evidence:** `[workspace.dependencies]` contains only iroh, iroh-relay and iroh-mdns-address-lookup. Cargo.lock has 2 `name = "sha2"` entries. The reviewer counted 1,330 external dependency declarations over 152 crates and 22 crates declared with conflicting requirements (sha2 0.10 x71 vs 0.11 x13); the verifier did not re-measure the per-manifest counts.

**Impact:** Version bumps and security patches have to be repeated by hand in 150+ manifests, and the same hash primitive is compiled twice.

**Suggested action:**
1. Use a scripted rewrite to hoist serde, serde_json, tokio, tempfile, sha2, hmac, base64, reqwest, libc and getrandom into `[workspace.dependencies]`, with members using `.workspace = true`.
2. Pick one sha2 line.
3. Verify: `grep -c '^name = "sha2"' Cargo.lock` reports 1.

### X-LINT-09 6 root crates bypass workspace lints with copied or absent tables; wallet and x402 (payment code with unsafe libc) have no lints

**Severity:** Medium · **Category:** policy · **Effort:** S

**Locations:**
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml)
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml)
- [crates/oa-copy/Cargo.toml](../../../../crates/oa-copy/Cargo.toml)
- [crates/openagents-ui/Cargo.toml](../../../../crates/openagents-ui/Cargo.toml)
- [crates/bitcoin-amount/Cargo.toml](../../../../crates/bitcoin-amount/Cargo.toml) (Cargo.toml:10-18)
- [crates/rust-native/Cargo.toml](../../../../crates/rust-native/Cargo.toml) (Cargo.toml:36-42)

**Evidence:** wallet, x402, oa-copy and openagents-ui have no `[lints]` table. x402/src contains 19 `unsafe` occurrences. rust-native copies only `unsafe_op_in_unsafe_fn` and the 3 clippy denies, not `unexpected_cfgs`, and has 92 unsafe blocks, fns and impls. wallet and x402 hard-code `edition = "2024"`.

**Impact:** The shared unsafe and dbg/todo policy does not apply to the crates that handle money, and the copied tables drift from the workspace baseline.

**Suggested action:**
1. Add `[lints]\nworkspace = true` to wallet, x402, oa-copy and openagents-ui.
2. Replace the copied tables in rust-native and bitcoin-amount with `workspace = true`. Move bitcoin-amount's `unsafe_code = "forbid"` to `#![forbid(unsafe_code)]` in its lib.rs.
3. Switch hard-coded `edition` and `rust-version` to `edition.workspace = true` and `rust-version.workspace = true`.
4. Verify: `rg -L '^\[lints\]' crates/*/Cargo.toml` lists only the separate workspaces, and `cargo check -p wallet -p x402 -p rust-native` still passes.

### X-LINT-10 Three file-locking mechanisms (libc::flock, fs2, std File::try_lock) and 71 hand-rolled unsafe geteuid calls

**Severity:** Medium · **Category:** duplication · **Effort:** S

**Locations:**
- [crates/x402/src/policy.rs](../../../../crates/x402/src/policy.rs)
- [crates/x402/src/front.rs](../../../../crates/x402/src/front.rs)
- [crates/commercial-spend/src/lib.rs](../../../../crates/commercial-spend/src/lib.rs)
- [crates/gateway/Cargo.toml](../../../../crates/gateway/Cargo.toml)
- [crates/retail-service/Cargo.toml](../../../../crates/retail-service/Cargo.toml)
- [crates/contribution-service/Cargo.toml](../../../../crates/contribution-service/Cargo.toml)
- [crates/compute-workbench/Cargo.toml](../../../../crates/compute-workbench/Cargo.toml)
- [crates/background/src/run.rs](../../../../crates/background/src/run.rs) (run.rs:169)

**Evidence:** `libc::flock` appears 10 times in 8 files, 4 of them in x402. `fs2` is declared in 4 manifests. background/src/run.rs:169 uses `file.try_lock()`. `libc::geteuid` is called 71 times across 38 files.

**Impact:** Unnecessary unsafe in payment and ledger code, an unmaintained dependency, and inconsistent lock semantics across crates.

**Suggested action:**
1. Replace `libc::flock` and `fs2` with `std::fs::File::lock()` / `try_lock()`, and drop `fs2` from the 4 manifests.
2. Add one safe `current_euid()` helper in a shared crate and route the geteuid call sites through it.
3. Verify: `rg 'libc::flock|fs2::' crates --glob '!crates/psionic/**'` is empty, and `rg -c 'libc::geteuid'` returns only the helper.

### X-LINT-11 openagents-mobile, mc-bridge and the docs runner crates are outside the deny policy; a docs runner uses an absolute machine path

**Severity:** Medium · **Category:** policy · **Effort:** S

**Locations:**
- [scripts/check-dependencies.sh](../../../../scripts/check-dependencies.sh) (check-dependencies.sh:19)
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) (Cargo.toml:122-126)
- [docs/decision-models/2026-09-20-state-budget-choice/original-runner/Cargo.toml](../../../../docs/decision-models/2026-09-20-state-budget-choice/original-runner/Cargo.toml) (Cargo.toml:6)

**Evidence:** check-dependencies.sh:19 runs only against the root workspace. There are 14 tracked Cargo.lock files. original-runner/Cargo.toml:6 reads `coder = { path = "/Users/christopherdavid/work/openagents/crates/coder" }`.

**Impact:** The dependency graph of the shipped mobile wallet never gets a source, license or advisory review. The docs runner builds on only one machine.

**Suggested action:**
1. Make check-dependencies.sh loop over `--manifest-path crates/openagents-mobile/Cargo.toml` and mc-bridge with the same deny.toml.
2. Delete the docs runner manifests, or switch them to relative paths with an empty `[workspace]`.
3. Verify: `rg '/Users/' --glob 'Cargo.toml'` is empty, and the script output shows a cargo-deny run for each workspace.

### X-LINT-12 The dependency gate will hard-fail on 2026-10-20 because of the paste advisory review date

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations:**
- [scripts/check-dependencies.sh](../../../../scripts/check-dependencies.sh) (check-dependencies.sh:7-17)
- [deny.toml](../../../../deny.toml) (deny.toml:12-16)

**Evidence:** check-dependencies.sh:7 exits 1 when the date is on or after 20261020. `cargo tree -i paste` at HEAD shows paste 1.0.15 coming in through kev and laya (candle 0.11, tokenizers 0.23), and also through alloy-primitives via the vendored boltz-client, breez-sdk-spark and openagents-spark, reaching openagents-cli.

**Impact:** In 10 days every release gate will fail for a reason unrelated to the change being verified.

**Suggested action:**
1. Before 2026-10-20, re-review RUSTSEC-2024-0436.
2. Add the alloy/boltz/Breez path to the reason at deny.toml:15 and in docs/dependencies.md (see X-LINT-20).
3. Record a new review date and update the date check in check-dependencies.sh.
4. Verify: run `./scripts/check-dependencies.sh` with the system date (or the script's date input) set past 2026-10-20, and confirm the advisory step passes.

### X-LINT-13 196 #[allow(clippy::too_many_arguments)]: telescoping APIs instead of parameter structs

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [crates/verse/src/render.rs](../../../../crates/verse/src/render.rs)

**Evidence:** There are 196 `allow(..too_many_arguments` entries (re-measured).

**Impact:** Arguments of the same type are easy to swap by mistake, and the number of function variants keeps growing. No defect has been shown; this is refactoring debt.

**Suggested action:**
1. Start with crates/verse/src/render.rs: add a `CaptureRequest` struct and collapse the `capture*` variants into one function that takes it.
2. Then refactor coder-one/micro.rs and rust-native/layout/rows.rs the same way.
3. Verify: the `too_many_arguments` allow count goes down in each touched file and the touched crates' tests pass.

### X-LINT-14 Dead code parked behind #[allow(dead_code)]: openagents-web cloud effects/hosts modules (1,093+ lines)

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:**
- [crates/openagents-web/src/cloud/mod.rs](../../../../crates/openagents-web/src/cloud/mod.rs) (mod.rs:12-17)
- [crates/openagents-web/src/cloud/effects.rs](../../../../crates/openagents-web/src/cloud/effects.rs)
- [crates/openagents-desktop/src/platform/windows.rs](../../../../crates/openagents-desktop/src/platform/windows.rs) (windows.rs:152-159)
- [crates/openagents-desktop/src/platform/linux.rs](../../../../crates/openagents-desktop/src/platform/linux.rs) (linux.rs:282-297)

**Evidence:** mod.rs:12-17 has the comment "nothing calls them until it lands" followed by `#[allow(dead_code)] mod effects;` (1,093 lines, 2 `libc::flock` calls) and `#[allow(dead_code)] pub mod hosts;`. openagents-desktop has 5 `cfg_attr(not(test), allow(dead_code))` sites. Counting `cfg_attr` forms, there are 103 dead_code allows repo-wide (the reviewer counted 60 without them).

**Impact:** Unreachable code still compiles, still takes review time, and carries unsafe that nothing exercises. The parking is intentional and documented, which is why this is rated low.

**Suggested action:**
1. Put cloud/effects.rs and hosts.rs behind an off-by-default `environments` feature, or delete them and restore them from git when /environments lands.
2. Wire the desktop test-only functions into the uninstall path, or delete them.
3. Verify: `rg 'allow\(dead_code\)' crates/openagents-web/src/cloud crates/openagents-desktop/src/platform` is empty and both crates build.

### X-LINT-15 Root workspace uses resolver "2"; MSRV-aware resolver 3 is not enabled

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:13, Cargo.toml:24-28)

**Evidence:** The root is a virtual workspace that pins edition 2024 and rust-version 1.97.1 but sets `resolver = "2"`.

**Impact:** `cargo update` can choose dependency versions whose MSRV is newer than the pinned toolchain.

**Suggested action:**
1. Set `resolver = "3"` in the root Cargo.toml.
2. crates/openagents-mobile needs no change: it is a non-virtual workspace whose root package is edition 2024, so it already defaults to resolver 3.
3. Verify: `cargo metadata --format-version 1 | jq .resolve` succeeds and `cargo update --dry-run` proposes no versions above rust-version 1.97.1.

### X-LINT-16 Orphan check-cfg(kani) and the never-enabled verse-bake/gpu feature

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:32)
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml) (Cargo.toml:126)
- [crates/verse-bake/src/main.rs](../../../../crates/verse-bake/src/main.rs) (main.rs:241-248)
- [crates/verse-bake/src/tests.rs](../../../../crates/verse-bake/src/tests.rs) (tests.rs:218)

**Evidence:** Outside psionic, `rg 'cfg\(kani'` matches only the two Cargo.toml declarations. No script, toml or yml enables verse-bake's `gpu` feature. The feature list at verify-rust.sh:26 is kev/serve, lev/serve, gym/tui, jev/blocking and oak/mcp-http. The reviewer also reported bunny-web/autoplay as never enabled; the verifier did not check it separately.

**Impact:** Dead configuration, and feature-gated code that rots because nothing compiles it.

**Suggested action:**
1. Remove `check-cfg = ['cfg(kani)']` from both manifests.
2. Either add verse-bake/gpu (and bunny-web/autoplay) to the feature list in verify-rust.sh, or delete those features and the code behind them.
3. Verify: `cargo check -p verse-bake --features gpu` runs in the verify script, or `rg 'feature = "gpu"' crates/verse-bake` is empty.

### X-LINT-17 Debug-info cost is handled ad hoc with env vars in mobile build scripts; no explicit dev debug or release profile

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:33-35)
- [scripts/build-openagents-android.sh](../../../../scripts/build-openagents-android.sh) (build-openagents-android.sh:101)
- [scripts/build-coder-android.sh](../../../../scripts/build-coder-android.sh) (build-coder-android.sh:60)
- [scripts/build-coder-mobile.sh](../../../../scripts/build-coder-mobile.sh) (build-coder-mobile.sh:31)
- [scripts/mobile-probe.py](../../../../scripts/mobile-probe.py) (mobile-probe.py:39)

**Evidence:** 4 scripts export `CARGO_PROFILE_DEV_DEBUG=0` together with `CARGO_INCREMENTAL=0` and a low `CARGO_BUILD_JOBS`. build-openagents-android.sh:162 builds `--manifest-path crates/openagents-mobile/Cargo.toml`. The root has no `[profile.dev]` debug setting and no `[profile.release]`.

**Impact:** Slow linking and a bloated target/ on shared hosts, and binaries that differ depending on how the build was started.

**Suggested action:**
1. Add `[profile.dev] debug = "line-tables-only"` and `[profile.dev.package."*"] debug = false` to the root Cargo.toml, and define `[profile.release]` explicitly.
2. The Android scripts build the separate mobile workspace, which a root profile does not reach. Either mirror the profile in crates/openagents-mobile/Cargo.toml before removing the env overrides from the scripts, or keep the overrides.
3. Verify: compare `du -sh target/debug` and link time for `cargo build -p coder` before and after.

### X-LINT-18 Invalid SPDX license id "CC-0" in three crates

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [crates/bitcoin-amount/Cargo.toml](../../../../crates/bitcoin-amount/Cargo.toml) (Cargo.toml:6)
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml) (Cargo.toml:5)
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml) (Cargo.toml:5)

**Evidence:** All three crates have `license = "CC-0"`. The valid SPDX id is `CC0-1.0`. rust-native declares Apache-2.0.

**Impact:** SPDX tooling misreads the field, and the intended license is unclear.

**Suggested action:**
1. If CC0 is intended, change the value to `CC0-1.0`.
2. Otherwise add `license = "Apache-2.0"` to `[workspace.package]` and `license.workspace = true` to members.
3. Verify: `rg 'license = "CC-0"' crates` is empty and cargo-deny's licenses check no longer flags these crates.

### X-LINT-19 Redundant workspace member entry and a non-build module named build.rs

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:2-3)
- [crates/gym/src/build.rs](../../../../crates/gym/src/build.rs)

**Evidence:** `members = ["crates/coderbench", "crates/*"]`, and the glob already covers coderbench. crates/gym/src/build.rs is a 1,171-line library module, not a build script.

**Impact:** Minor confusion for readers and tools.

**Suggested action:**
1. Change members to `["crates/*"]`.
2. Rename gym/src/build.rs (e.g. `suite_build.rs`) and update the `mod` declaration.
3. Verify: `cargo metadata --no-deps` lists the same member set, and `cargo check -p gym` passes.

### X-LINT-20 The EVM alloy stack is in the root dependency graph through vendored boltz-client and Breez Spark, reaching openagents-cli

**Severity:** Low · **Category:** build · **Effort:** M

**Locations:**
- [Cargo.lock](../../../../Cargo.lock)
- [vendor/boltz-client/crates/lib](../../../../vendor/boltz-client/crates/lib)
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml)

**Evidence:** `cargo tree --offline --locked --workspace --all-features -i paste` shows paste reached through alloy-primitives, alloy-consensus, alloy-network, alloy-signer-local and others. The workspace roots on that path are boltz-client (vendored), breez-sdk-spark, openagents-spark and openagents-cli. The specific feature that enables alloy has not been identified.

**Impact:** The CLI build compiles an Ethereum client stack the product does not use (payments are Bitcoin-only). This adds compile time and supply-chain surface, and it is the undocumented second reason the paste advisory waiver is still needed (X-LINT-12).

**Suggested action:**
1. Run `cargo tree -i alloy-primitives -e features` to find the boltz-client or Breez feature that enables alloy (likely EVM swap support).
2. Turn it off in the vendored boltz-client manifest or in the Breez dependency's features.
3. Record the result in docs/dependencies.md next to the paste waiver.
4. Verify: `cargo tree -p openagents-cli -i alloy-primitives` reports no path.

## Refuted during verification

- **"Candle/GEMM/Pulp belong only to the excluded psionic workspace, so the paste waiver reason is stale"** (part of X-LINT-12). Rejected: crates/kev/Cargo.toml:16-25 and crates/laya/Cargo.toml:16-24 are in the root workspace and depend on candle-core/candle-nn 0.11 and tokenizers 0.23, and `cargo tree -i paste` lists kev and laya as consumers. The reason is incomplete (the alloy path is missing), not stale. The finding stays in adjusted form.
- **X-LINT-15, mobile part:** the suggestion to also set resolver 3 in crates/openagents-mobile was dropped. That workspace's root package is edition 2024, so it already defaults to resolver 3.
- **X-LINT-03, clippy part:** the suggestion to add a nightly clippy CI job was dropped because the owner has chosen lean verification without clippy or gates by default. Severity was lowered from high to medium.
