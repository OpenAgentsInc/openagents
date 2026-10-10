# Psionic: eval, research, data

Scope: `crates/psionic/crates/{psionic-eval, psionic-research, psionic-data, psionic-environments, psionic-optimize, psionic-datastream}` (section PS2). Audit date 2026-10-10, snapshot `3168c986aa`.

**Health grade: D**

These six crates hold about 266k lines of Rust. Nearly all of it came in one bulk import (`737f94a17c`, 2026-10-07, "Import Psionic serving path into crates/psionic"), and none of the six crates has changed since. Most of the code is a frozen copy of the upstream Tassadar research program: 188 of 227 src files in psionic-eval and 132 of 141 in psionic-research are Tassadar. The import commit says research lanes stay upstream. These crates were pulled in anyway, because psionic-serve, psionic-provider and psionic-router link them through Tassadar publication modules that the `psionic-openai-server` binary never uses.

The code hard-codes 504 repo-relative fixture paths, and 491 of them do not exist. All 11 referenced `docs/` paths and all 56 `scripts/` paths are missing too. As a result, about 307 "matches committed truth" golden tests and most report-writing examples cannot pass. No CI job builds or tests `crates/psionic`.

The code also has systemic smells. Hundreds of helpers are copy-pasted (`stable_digest` 447 times, `repo_root` about 308 times, `read_json` about 300 times). Serialization failures are handled in three different ways. Roughly 396 non-test panic sites exist even though the workspace lints deny them. psionic-eval exposes 206 modules through glob re-exports. The legal-benchmark lane is better engineered than the rest, but its write tool follows symlinks out of the workspace root. psionic-datastream and psionic-optimize are the healthiest crates, though each is a single large file.

## Measurements

| Metric | Value |
|---|---|
| LOC (tracked .rs, src+examples+bins) | eval 162,211 (227 src / 194 examples); research 49,757 (141 src / 140 examples / 2 bins); data 37,406 (55 files); environments 8,486 (7); optimize 4,622 (1); datastream 3,818 (1). Total about 266k |
| Tassadar share | eval 188/227 src (118,903 LOC); research 132/141 (38,929); data 18/43 (11,138); environments 4/7 (3,827) |
| Largest files | eval/src/tassadar.rs 5,729; eval/src/legal_benchmark_provider.rs 3,474; data/src/tassadar.rs 3,416; data/src/lib.rs 3,396; environments/src/tassadar.rs 3,325; eval/src/full.rs 2,655; research/src/lib.rs 2,633 |
| `#[test]` count | eval 751, research 392, data 139, environments 18, optimize 16, datastream 34. About 307 are committed-truth golden tests |
| Fixture paths referenced / missing | 504 / 491 (473 under `fixtures/tassadar`). Verifier re-measured 505 / 492 |
| `docs/` / `scripts/` refs missing | 11 of 11 / 56 of 56 |
| Non-test expect/unwrap/panic/unreachable | eval 197, research 113, data 57, environments 29, optimize 0, datastream 0 |
| Duplicated helpers | `fn stable_digest` 447; `fn repo_root` about 308; `read_json`/`read_repo_json` about 296-300; `fn write_json` 25 |
| `unwrap_or_default` digests | about 296-299 |
| `#[allow]` | 42 `too_many_arguments`, 4 `expect_used`, 3 `panic` |
| Dated version literals / long (200+ char) string literals | 82 / 605 (about 184 KB of prose in code) |
| psionic-eval `full.rs` | 206 `#[path]` mods, 206 `pub use` globs. lib.rs has 19 more globs |
| TODO/FIXME, unsafe | 0, 0 |
| Git activity | 1 commit per crate (the import) |
| CI references to `crates/psionic` | 0 |
| `crates/psionic/target` | 448 MB, not gitignored |

## Strengths

- Errors are typed with `thiserror` throughout. Report builders return `Result` with specific variants, for example `TassadarClrsWasmBridgeReportError` (psionic-eval/src/tassadar_clrs_wasm_bridge.rs:250-268).
- psionic-datastream and psionic-optimize have zero non-test panic sites and use domain-separated digest prefixes (psionic-optimize/src/lib.rs:16-30). `DatastreamManifest::validate_payload` (psionic-datastream/src/lib.rs:758-802) checks total length, the object digest, and every chunk's digest and bounds.
- The legal-benchmark lane keeps secrets out of requests. They carry `<secret_ref:...>` placeholders and redacted headers, and `ReqwestBlockingHttpTransport` refuses unresolved refs before any network I/O (legal_benchmark_provider.rs:1713-1722). A test covers this (line 2901).
- The legal tool workspace confines reads and edits to its root with canonicalize plus `starts_with` (legal_benchmark_tools.rs:1775-1794). It rejects absolute paths and `.`/`..` components, and it only runs shell commands when a sandbox backend (line 419) and podman (line 570) are attached.
- Output is deterministic: BTreeMap/BTreeSet for stable serialization, digest-bearing receipts, and explicit refusal kinds instead of silent fallbacks.
- Some heavy dependencies are already feature-gated: psionic-research's `burn-import` feature and binary use `required-features`, and psionic-eval's `full` feature makes 16 psionic deps optional.
- No TODO/FIXME markers and no `unsafe` code.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| PS2-01 | high | testing | 491 of 504 hard-coded fixture paths are missing, so about 307 golden tests and most examples cannot pass | M |
| PS2-02 | high | security | Legal-benchmark write tool follows existing or dangling symlinks out of the workspace root | S |
| PS2-03 | medium | architecture | psionic-research and eval's Tassadar half are linked into psionic-serve only through Tassadar modules the server never uses | L |
| PS2-04 | medium | duplication | 447 copy-pasted `stable_digest` helpers; about 299 silently hash empty bytes on failure | M |
| PS2-05 | medium | duplication | About 308 duplicated `repo_root()` helpers with a stale expect message; examples write into the source tree | M |
| PS2-06 | medium | error-handling | Workspace denies expect/unwrap/panic, but hundreds of non-test sites exist, so clippy is never run | M |
| PS2-07 | medium | build | No CI builds or tests anything under `crates/psionic` | S |
| PS2-08 | medium | architecture | psionic-eval exposes 206+19 modules through `pub use *` globs and 206 `#[path]` declarations | L |
| PS2-09 | medium | architecture | Tassadar-specific locators are hard-wired into the generic datastream transport | M |
| PS2-10 | medium | architecture | psionic-train hard-depends on psionic-eval, so eval is on the live serving path | M |
| PS2-11 | low | dead-code | Dead no-default-features surface duplicates four benchmark types in lib.rs | S |
| PS2-12 | low | dead-code | Legal-benchmark reqwest transport refuses secret refs, and no resolver exists in the monorepo | M |
| PS2-13 | low | maintainability | datastream and optimize are single large files; environments has few tests | M |
| PS2-14 | low | dead-code | psionic-optimize (4.6k LOC) has one consumer module and is a non-optional eval dep | S |
| PS2-15 | low | docs | Stale or broken references to upstream docs, scripts and the standalone psionic repo | S |
| PS2-16 | low | maintainability | Hard-coded dated version strings and large prose literals in report builders | S |
| PS2-17 | low | maintainability | Legal-tool failure helpers take 9 positional args; 42 `too_many_arguments` allows | S |
| PS2-18 | low | error-handling | Reqwest transport swallows builder errors, times with SystemTime, and records all response headers | S |
| PS2-19 | low | build | `tempfile` is a production dep of psionic-eval for one non-test use; burn deps pinned inline | S |
| PS2-20 | low | repo-hygiene | `crates/psionic/target` (448 MB) is not gitignored | S |

### PS2-01 491 of 504 fixture paths hard-coded in these crates do not exist, so about 307 golden tests and most examples are broken by construction

Severity: high · Category: testing · Effort: M

Locations:
- [tassadar_article_abi.rs:167](../../../../crates/psionic/crates/psionic-eval/src/tassadar_article_abi.rs)
- [tassadar_article_abi.rs:374-380](../../../../crates/psionic/crates/psionic-eval/src/tassadar_article_abi.rs)
- [compiled_agent_eval.rs:7-15](../../../../crates/psionic/crates/psionic-eval/src/compiled_agent_eval.rs)
- [README.md:16](../../../../crates/psionic/README.md)
- [fixtures/tassadar](../../../../crates/psionic/fixtures/tassadar)

Evidence: The verifier re-extracted the unique `"fixtures/..."` literals from the six crates' src and found 505, of which 492 are missing on disk. `fixtures/tassadar` tracks only 5 files. `article_abi_closure_report_matches_committed_truth` (tassadar_article_abi.rs:374) calls `std::fs::read(tassadar_article_abi_closure_report_path())?`, which returns NotFound. Correction to the reviewer: `crates/psionic/fixtures` tracks 208 files (107 psion, 21 clef, 17 apple_adapter, ...), not 23. The 23 in README.md:16 is stale.

Impact: `cargo test -p psionic-eval` (and research/data) cannot pass. Real regressions are hidden among hundreds of expected failures.

Suggested action:
1. Preferred: delete the Tassadar report modules and their golden tests from the monorepo copy. Coordinate this with PS2-03/PS2-10 so the non-Tassadar eval API that psionic-train uses stays intact.
2. Otherwise, gate every committed-truth test behind an `upstream-fixtures` feature with `#[cfg_attr(not(feature = "upstream-fixtures"), ignore)]`.
3. Add a test that lists every fixture ref that does not resolve, so new missing paths fail loudly.
4. Verify that `cargo test --manifest-path crates/psionic/Cargo.toml -p psionic-eval -p psionic-research -p psionic-data` passes on a clean checkout.

### PS2-02 Legal-benchmark write tool follows existing or dangling symlinks out of the workspace root

Severity: high · Category: security · Effort: S

Locations:
- [legal_benchmark_tools.rs:771-803](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_tools.rs)
- [legal_benchmark_tools.rs:1796-1826](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_tools.rs)
- [legal_benchmark_tools.rs:570-592](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_tools.rs)

Evidence: `resolve_new_or_existing_path` canonicalizes only the parent or nearest existing ancestor, checks it with `starts_with(canonical_root)`, and then returns the raw `candidate` (line 1825). The write path checks `resolved.exists() && !overwrite` and then calls `fs::write(resolved)` (line 803). Both `exists()` and `fs::write` follow symlinks. An existing link combined with `overwrite=true` therefore escapes the root. A dangling link has `exists() == false`, so it escapes even without overwrite. `grep symlink` in the file finds nothing.

Impact: A model tool call can write outside the per-run workspace. The shell tool operates in the same workspace, so the model can create the link itself. No in-repo caller runs this harness live today, but the tool exists to execute untrusted model actions.

Suggested action:
1. In `resolve_new_or_existing_path`, refuse with `PathEscape` when `fs::symlink_metadata(&candidate)` reports `is_symlink()`.
2. Write via `OpenOptions::new().write(true).create(true).truncate(true)`, using `create_new(!overwrite)` and, on Unix, `custom_flags(libc::O_NOFOLLOW)`.
3. Add two tests that each expect `PathEscape`: an overwrite through an existing link to a file outside the root, and a write through a dangling link that points outside the root.

### PS2-03 psionic-research and psionic-eval's Tassadar half are linked into psionic-serve only via Tassadar modules the server binary never uses

Severity: medium · Category: architecture · Effort: L

Locations:
- [psionic-serve/Cargo.toml:46](../../../../crates/psionic/crates/psionic-serve/Cargo.toml), Cargo.toml:51, Cargo.toml:55
- [psionic-train/Cargo.toml:40](../../../../crates/psionic/crates/psionic-train/Cargo.toml)
- [psionic-provider/Cargo.toml:20-22](../../../../crates/psionic/crates/psionic-provider/Cargo.toml)
- [psionic-provider/src/lib.rs:243](../../../../crates/psionic/crates/psionic-provider/src/lib.rs), lib.rs:712-790
- [psionic-router/Cargo.toml:19](../../../../crates/psionic/crates/psionic-router/Cargo.toml)

Evidence: `psionic-openai-server.rs` mentions tassadar 0 times. In psionic-serve/src, every `psionic_eval`/`psionic_research` import is in a `tassadar*` file. psionic-provider's uses outside tassadar files sit at lib.rs:243 and 712-790, and all of them call `build_tassadar_*` functions. However, psionic-serve also hard-depends on psionic-train (Cargo.toml:55), which serve's live `gguf.rs`/`qwen35.rs` use. psionic-train hard-depends on psionic-eval (Cargo.toml:40) and imports `psionic_eval::` from 58 non-Tassadar files. Gating serve alone therefore cannot remove psionic-eval from the serving closure. psionic-research is not a psionic-train dependency and can be dropped.

Impact: The production server compiles psionic-research (about 50k LOC) and eval's Tassadar modules for no runtime value. This inflates build time and audit scope and contradicts the import commit's statement that research lanes stay upstream.

Suggested action:
1. Add a non-default `tassadar` feature to psionic-serve, psionic-provider and psionic-router. Use it to gate their tassadar modules and the provider lib.rs Tassadar block, and make psionic-research (serve/provider) and psionic-environments (router) optional deps.
2. Separately, add a `tassadar` feature in psionic-eval that gates the 188 `tassadar_*` modules in full.rs, so psionic-train only pulls in the non-Tassadar surface (see PS2-10).
3. Verify that `cargo tree --manifest-path crates/psionic/Cargo.toml -p psionic-serve -e normal | grep -c psionic-research` returns 0 and that the server still builds.

### PS2-04 447 copy-pasted `stable_digest` helpers; ~299 silently hash empty bytes when serialization fails

Severity: medium · Category: duplication · Effort: M

Locations:
- [tassadar_article_abi.rs:342-347](../../../../crates/psionic/crates/psionic-eval/src/tassadar_article_abi.rs)
- [psionic-research/src/lib.rs:2056-2202](../../../../crates/psionic/crates/psionic-research/src/lib.rs)
- [psionic-environments/src/tassadar.rs:150](../../../../crates/psionic/crates/psionic-environments/src/tassadar.rs)
- [psion_reference_corpus.rs:1474](../../../../crates/psionic/crates/psionic-data/src/psion_reference_corpus.rs)

Evidence: `rg -c 'fn stable_digest'` finds 447 definitions. `serde_json::to_vec(value).unwrap_or_default` appears 299 times. Other copies use `expect` or `unreachable!`.

Impact: One integrity operation follows three different error policies, and any fix has to be applied hundreds of times. A silent empty-hash collision requires serde_json to fail on a `derive(Serialize)` type (non-string map keys, or a custom `Serialize` that errors), so in practice the main cost is duplication.

Suggested action:
1. Add one shared `stable_json_digest<T: Serialize>(prefix, value) -> Result<String, serde_json::Error>` in a crate all six already depend on, or in a new small crate.
2. Replace the local functions with a scripted rewrite, propagating the error with `?` where the error enum supports it.
3. Verify that digests embedded in tracked fixtures are unchanged and that `rg -c 'fn stable_digest'` returns 0 outside the shared crate.

### PS2-05 ~308 duplicated `repo_root()` helpers resolve paths via CARGO_MANIFEST_DIR with a stale expect message; examples write reports into the source tree

Severity: medium · Category: duplication · Effort: M

Locations:
- [tassadar_article_abi.rs:334-340](../../../../crates/psionic/crates/psionic-eval/src/tassadar_article_abi.rs)
- [tassadar_clrs_wasm_bridge.rs:323-357](../../../../crates/psionic/crates/psionic-eval/src/tassadar_clrs_wasm_bridge.rs)

Evidence: `rg -c 'fn repo_root'` finds 308 definitions. The sample at tassadar_article_abi.rs:334-340 is `.parent().and_then(Path::parent).expect("psionic-eval should live under <repo>/crates/psionic-eval")`. That message is stale, because the crate now lives at `crates/psionic/crates/`. There are 296 `read_json`/`read_repo_json` functions.

Impact: `CARGO_MANIFEST_DIR` is fixed at compile time, so binaries panic or misbehave when run from anywhere else, and examples leave the git tree dirty.

Suggested action:
1. Replace all copies with one `psionic_fixture_root()` that reads `PSIONIC_FIXTURE_ROOT` and falls back to `CARGO_MANIFEST_DIR`.
2. Consolidate `read_json`/`write_json` next to it.
3. Give examples an `--out` argument that defaults to `target/`.
4. Verify that `rg -c 'fn repo_root'` returns 1 and that running an example leaves `git status` clean.

### PS2-06 Workspace denies expect_used/unwrap_used/panic, yet hundreds of non-test sites exist, so clippy is never run on these crates

Severity: medium · Category: error-handling · Effort: M

Locations:
- [crates/psionic/Cargo.toml:5-45](../../../../crates/psionic/Cargo.toml)
- [tassadar_clrs_wasm_bridge.rs:274-276](../../../../crates/psionic/crates/psionic-eval/src/tassadar_clrs_wasm_bridge.rs), line 356
- [psionic-environments/src/tassadar.rs:150](../../../../crates/psionic/crates/psionic-environments/src/tassadar.rs)
- [psion_reference_corpus.rs:847-1070](../../../../crates/psionic/crates/psionic-data/src/psion_reference_corpus.rs)

Evidence: The reviewer's scan of non-test code finds eval 197, research 113, data 57 and environments 29 sites. The 308 `repo_root` expects alone, all outside test modules, independently confirm that the deny is not enforced.

Impact: The lint policy is not enforced, and library code panics in places where typed error variants already exist.

Suggested action:
1. Run clippy for datastream, optimize, data and environments on coderos with a separate target dir.
2. Convert serialization expects to `?` where the error enum has `#[from] serde_json::Error`, and replace lock expects with `unwrap_or_else(PoisonError::into_inner)`.
3. Enforce the deny in CI (PS2-07), crate by crate as each one goes clean.

### PS2-07 No CI builds or tests anything under crates/psionic

Severity: medium · Category: build · Effort: S

Locations:
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml)
- [.github](../../../../.github)

Evidence: `git grep -n psionic -- .github` returns nothing.

Impact: The broken golden tests, the lint violations and the symlink bug all went unnoticed. Changes to the live dependencies psionic-data and psionic-datastream land untested.

Suggested action:
1. Add `.github/workflows/psionic.yml`, triggered on paths `crates/psionic/**`, that runs `cargo test` and `cargo clippy` for `-p psionic-datastream -p psionic-optimize -p psionic-data -p psionic-environments`.
2. Add eval and research once PS2-01 is resolved.
3. Verify by opening a PR that touches `crates/psionic/` and checking that the job runs.

### PS2-08 psionic-eval exposes 206+19 modules through `pub use *` globs, plus 206 #[path] declarations in full.rs

Severity: medium · Category: architecture · Effort: L

Locations:
- [psionic-eval/src/lib.rs:13-58](../../../../crates/psionic/crates/psionic-eval/src/lib.rs)
- [psionic-eval/src/full.rs](../../../../crates/psionic/crates/psionic-eval/src/full.rs)

Evidence: full.rs has 206 `#[path` attributes and 206 `pub use ...::*;` lines. lib.rs has 19 more glob re-exports.

Impact: The result is one huge flat namespace. Glob imports can shadow each other silently, and there is no visible public contract.

Suggested action:
1. Move the 206 modules into `src/tassadar/mod.rs` as normal submodules without `#[path]`.
2. Move full.rs's own types into `src/benchmark.rs`.
3. Replace the globs with namespaced modules and a curated re-export list, built from what consumers actually import: `rg -o 'psionic_eval::\{?[A-Za-z_, ]+'` across train, serve and provider.
4. Verify that the workspace builds and that `rg -c 'pub use .*::\*;' crates/psionic/crates/psionic-eval/src` drops to near zero.

### PS2-09 Tassadar-specific locators hard-wired into the generic datastream transport (~12 near-identical constructors and locator methods)

Severity: medium · Category: architecture · Effort: M

Locations:
- [psionic-datastream/src/lib.rs:195-318](../../../../crates/psionic/crates/psionic-datastream/src/lib.rs), lib.rs:844-1040, lib.rs:1088-1640

Evidence: `rg 'pub fn tassadar_.*_locator'` finds 12 functions and `pub struct Tassadar.*Locator` finds 12 structs (24 combined).

Impact: Hundreds of lines of boilerplate tie a serving-path crate to a research program.

Suggested action:
1. Collapse them into one generic `checkpoint_locator(&self, family_prefix)` that returns a `CheckpointLocator`.
2. Move the Tassadar family prefixes behind a `tassadar` feature or into the Tassadar crates.
3. Add a JSON round-trip test that checks the serialized form matches the old one.

### PS2-10 psionic-train hard-depends on psionic-eval, so eval is on the live serving path regardless of serve/provider gating

Severity: medium · Category: architecture · Effort: M

Locations:
- [psionic-serve/Cargo.toml:55](../../../../crates/psionic/crates/psionic-serve/Cargo.toml)
- [psionic-train/Cargo.toml:39-40](../../../../crates/psionic/crates/psionic-train/Cargo.toml)

Evidence: psionic-serve depends on psionic-train as a non-optional dependency, and serve's live `gguf.rs` and `qwen35.rs` import `psionic_train::`. psionic-train depends on psionic-eval and psionic-environments. 75 train src files import `psionic_eval::`, 58 of them outside Tassadar code.

Impact: Any fix for eval's breakage or size has to keep the non-Tassadar eval API that train uses stable. Deleting eval wholesale would break serving.

Suggested action:
1. Before removing any Tassadar code, list the exact eval items train uses: `rg -o 'psionic_eval::[A-Za-z_]+' crates/psionic/crates/psionic-train/src | sort -u`.
2. Keep those items outside the new `tassadar` feature in psionic-eval, and gate everything else.
3. Verify that `cargo check -p psionic-serve` passes with eval's `tassadar` feature off.

### PS2-11 Dead no-default-features surface duplicates four benchmark types in lib.rs

Severity: low · Category: dead-code · Effort: S

Locations:
- [psionic-eval/src/lib.rs:60-137](../../../../crates/psionic/crates/psionic-eval/src/lib.rs)
- [psionic-eval/Cargo.toml:16-34](../../../../crates/psionic/crates/psionic-eval/Cargo.toml)

Evidence: lib.rs:61-115 has six `#[cfg(not(feature = "full"))]` blocks. The default features are `["full"]`, and no Cargo.toml in `crates/psionic` sets `default-features = false` for psionic-eval.

Impact: The duplicate contract can drift from the real one, and the lightweight build is never exercised. No runtime impact.

Suggested action:
1. Delete the `cfg(not(full))` blocks and the `full` feature, making its deps non-optional. Alternatively, move the four types into a `benchmark_contract.rs` used by both configurations.
2. Verify with `cargo check -p psionic-eval` (and with `--no-default-features` if the feature is kept).

### PS2-12 Legal-benchmark reqwest transport refuses secret refs and no resolver exists in the monorepo

Severity: low · Category: dead-code · Effort: M

Locations:
- [legal_benchmark_provider.rs:614-642](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_provider.rs)
- [legal_benchmark_provider.rs:1713-1722](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_provider.rs)

Evidence: Routes emit `Bearer <secret_ref:...>` headers (lines 624, 658, 689, 720). `ReqwestBlockingHttpTransport::send` refuses any header containing `<secret_ref:` (line 1717). Outside this file, `rg 'secret_ref:'` finds nothing. `ProviderHttpTransport` is a trait, so a caller could supply a resolving transport, but none exists in this repo.

Impact: With the shipped transport, the hosted OpenAI, Vertex and Anthropic routes cannot authenticate. The refusal itself is a deliberate fail-closed design.

Suggested action:
1. Add a resolver hook (a trait parameter) that substitutes refs just before send and never stores resolved values in recorded requests.
2. Add a test with a fake resolver that asserts the recorded request still contains only the placeholder.
3. Alternatively, delete the hosted routes if hosted legal runs live elsewhere.

### PS2-13 Monolithic single-file crates: datastream (3,818) and optimize (4,622), with low test density in environments

Severity: low · Category: maintainability · Effort: M

Locations:
- [psionic-datastream/src/lib.rs](../../../../crates/psionic/crates/psionic-datastream/src/lib.rs)
- [psionic-optimize/src/lib.rs](../../../../crates/psionic/crates/psionic-optimize/src/lib.rs)
- [psionic-environments/src/tassadar.rs](../../../../crates/psionic/crates/psionic-environments/src/tassadar.rs)
- [psionic-environments/src/runtime_service.rs](../../../../crates/psionic/crates/psionic-environments/src/runtime_service.rs)

Evidence: `wc -l` gives 3,818 and 4,622 lines. optimize has 16 tests. environments has 7 tests in lib.rs, 4 in tassadar.rs and 2 in runtime_service.rs.

Impact: These are live-path dependencies with thin test coverage. File size on its own is a weak smell. The missing tests are the part worth acting on.

Suggested action:
1. Add state-transition tests for `EnvironmentRuntimeService` and error-path tests for `DatastreamManifest::validate_payload` (length mismatch, bad object digest, bad chunk digest, out-of-bounds chunk).
2. Then, as pure moves with no logic changes, split optimize along its prefix constants and datastream into manifest, bindings, locators and transfer modules.
3. Verify that the test counts stay the same and the tests pass before and after each move.

### PS2-14 psionic-optimize is a 4.6k-LOC crate with a single consumer module and is a non-optional psionic-eval dep

Severity: low · Category: dead-code · Effort: S

Locations:
- [psionic-optimize/src/lib.rs](../../../../crates/psionic/crates/psionic-optimize/src/lib.rs)
- [psionic-eval/Cargo.toml:50](../../../../crates/psionic/crates/psionic-eval/Cargo.toml)
- [compiled_agent_module_optimization_proof.rs](../../../../crates/psionic/crates/psionic-eval/src/compiled_agent_module_optimization_proof.rs)

Evidence: `psionic_optimize::` appears only in compiled_agent_module_optimization_proof.rs (4 hits). `psionic-optimize = { path = "../psionic-optimize" }` at Cargo.toml:50 is psionic-eval's only non-optional psionic dependency.

Impact: The crate adds weight to the eval (and therefore serving) dependency closure for one module.

Suggested action:
1. Make it optional under `full` or a new `compiled-agent` feature and gate the proof module behind it. If the optimizer lives elsewhere, remove the crate.
2. Verify with `cargo tree -p psionic-serve -e normal | grep -c psionic-optimize` returning 0.

### PS2-15 Stale or broken references to upstream docs, scripts and the 'standalone psionic' repo

Severity: low · Category: docs · Effort: S

Locations:
- [compiled_agent_eval.rs:9-13](../../../../crates/psionic/crates/psionic-eval/src/compiled_agent_eval.rs)
- [tassadar_article_abi.rs:338](../../../../crates/psionic/crates/psionic-eval/src/tassadar_article_abi.rs)
- [crates/psionic/README.md:16](../../../../crates/psionic/README.md)

Evidence: The `repo_root` expect message names a path that no longer exists. README.md:16 says `fixtures/` holds 23 files, but 208 are tracked. The reviewer reports 11 missing `docs/` refs and 56 missing `scripts/` refs; the verifier did not re-measure these.

Impact: Readers cannot tell what is live and what is frozen, and generated reports embed dead links.

Suggested action:
1. Fix the fixture count in README.md:16.
2. Add a live/frozen status line and a list of consumers to each crate's README.
3. Rewrite `docs/` and `scripts/` references as pinned upstream URLs, or drop them.

### PS2-16 Hard-coded dated version strings and large prose literals inside report builders

Severity: low · Category: maintainability · Effort: S

Locations:
- [tassadar_clrs_wasm_bridge.rs:289-307](../../../../crates/psionic/crates/psionic-eval/src/tassadar_clrs_wasm_bridge.rs)

Evidence: The reviewer counts 82 dated `YYYY.MM.DD` literals and 605 string literals of 200+ characters. Not re-measured.

Impact: Edits produce noisy diffs, and any prose edit changes golden digests.

Suggested action:
1. If the Tassadar reports are kept, hoist one `const` version per report.
2. Move long claim-boundary prose into `include_str!` files.
3. This finding is moot if the Tassadar removal in PS2-01/PS2-03 happens.

### PS2-17 Legal-tool failure helpers take 9 positional args; 42 clippy::too_many_arguments allows

Severity: low · Category: maintainability · Effort: S

Locations:
- [legal_benchmark_tools.rs:1541-1551](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_tools.rs)
- [legal_benchmark_tools.rs:759-769](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_tools.rs)

Evidence: `failure_execution(` is called 28 times, with the positional tail `Vec::new(), 0, 0, None, None` visible at lines 763-768. The six crates carry 42 `too_many_arguments` allows in total.

Impact: The `bytes_read`/`bytes_written` pair is easy to swap by mistake.

Suggested action:
1. Introduce a `ToolIoStats` struct with `Default` and pass it as a single argument.
2. Remove the `too_many_arguments` allow on `failure_execution` and confirm clippy passes.

### PS2-18 ReqwestBlockingHttpTransport swallows builder errors, times with SystemTime, and records all response headers

Severity: low · Category: error-handling · Effort: S

Locations:
- [legal_benchmark_provider.rs:1701-1710](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_provider.rs)
- [legal_benchmark_provider.rs:1748-1766](../../../../crates/psionic/crates/psionic-eval/src/legal_benchmark_provider.rs)

Evidence: The builder fallback is `Err(_) => Self { client: reqwest::blocking::Client::new() }`. Timing uses `SystemTime::now()` and `.elapsed()...unwrap_or(0)`. All response headers are collected into a BTreeMap.

Impact: Latencies can read 0 ms, construction errors are hidden, and every response header ends up in persisted artifacts.

Suggested action:
1. Remove `impl Default` and have a fallible constructor return the builder error.
2. Time requests with `std::time::Instant`.
3. Record only an allow-list of response headers, and add a test asserting that unlisted headers are dropped.

### PS2-19 tempfile is a production dependency of psionic-eval for one non-test use; burn deps pinned inline

Severity: low · Category: build · Effort: S

Locations:
- [psionic-eval/Cargo.toml:61](../../../../crates/psionic/crates/psionic-eval/Cargo.toml)
- [tassadar_article_interpreter_ownership_gate.rs:10](../../../../crates/psionic/crates/psionic-eval/src/tassadar_article_interpreter_ownership_gate.rs)
- [psionic-research/Cargo.toml](../../../../crates/psionic/crates/psionic-research/Cargo.toml)

Evidence: `tempfile = "3"` is listed under `[dependencies]` at Cargo.toml:61. The only non-test use is ownership_gate.rs:10 (`use tempfile::tempdir;`), which comes before that file's `#[cfg(test)]` at line 1838. The use in legal_benchmark_extraction.rs (line 1068) is inside `#[cfg(test)]` (line 1038).

Impact: Minor dependency bloat.

Suggested action:
1. Replace the single non-test use in the ownership gate, then move `tempfile` to `[dev-dependencies]`.
2. Move the burn and zip versions into `[workspace.dependencies]`.
3. Verify with `cargo check -p psionic-eval` and `cargo test -p psionic-eval --no-run`.

### PS2-20 crates/psionic/target (448 MB) is not gitignored

Severity: low · Category: repo-hygiene · Effort: S

Locations:
- [crates/psionic/target](../../../../crates/psionic/target)
- [.gitignore:1](../../../../.gitignore)

Evidence: The root `.gitignore` has `/target`, which only matches the repo root. `git check-ignore -v crates/psionic/target` prints nothing, and `git status` shows `?? crates/psionic/target/`.

Impact: A broad `git add` would commit 448 MB of build artifacts.

Suggested action:
1. Add `/crates/psionic/target/` to the root `.gitignore`, next to the existing `/crates/openagents-mobile/target/` line.
2. Verify that `git check-ignore -v crates/psionic/target` prints the new rule.

## Corrections made during verification

The verifier rejected no findings outright, but it corrected these reviewer claims. Do not re-raise them in their original form:

- "The import brought only 23 fixture files": wrong. 208 files are tracked under `crates/psionic/fixtures`, and the 23 in README.md:16 is stale (PS2-01, PS2-15).
- "About 160k LOC can be removed from the serving closure by gating serve": overstated. serve -> psionic-train -> psionic-eval is a hard live dependency, so only psionic-research and eval's Tassadar modules (behind a feature) can be removed (PS2-03, PS2-10).
- "tempfile has two production uses": only one. The legal_benchmark_extraction use is test-only (PS2-19).
- The silent-collision risk from `unwrap_or_default` digests is mostly theoretical for derive-serialized structs. The real issue is duplication (PS2-04).
