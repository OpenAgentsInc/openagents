# Scripts, deploy, and repo hygiene

**Scope:** the non-crate top level: `scripts/`, `deploy/`, `Dockerfile`, `migrations/`, `vendor/`, `training/`, `tests/`, the data directories at the top level (artifacts, scratch, sources, questions, quests, recipes, patterns, worlds, programs, methods, capabilities, qa), ignore rules, and gating.
**Health grade:** C
**Snapshot:** commit `3168c986aa11e18a8bd30f52c270f609e49b3815`, audit date 2026-10-10.

The individual scripts are in decent shape: almost every shell script uses `set -e` or `set -euo pipefail`, shellcheck finds little, the Dockerfiles are multi-stage and run as non-root, and `Cargo.lock` resolves with `--locked --offline`. The weak areas are repo hygiene and gating. HEAD tracks 1.82 GB, 1.13 GB of it under `bench/`. 704 MiB of that is in files under 1 MB, which the 1 MB pre-commit guard never checks. The pack is 8.2 GiB. CI is absent by policy, the pre-commit hook is opt-in per clone, and gate records are git-ignored, so no check is enforced anywhere. The changed-scope gate has two defects. It sends nested-workspace edits to cargo as non-member packages, which cargo rejects. It also misses about 30 crates that compile in top-level files. Production `promote` reads its launcher from the working tree. The `cargo-deny` phase starts failing on 2026-10-20.

## Measurements

| Metric | Value |
|---|---|
| Tracked files / bytes at HEAD | 38,825 / 1,822 MB |
| By directory | bench 1,132 MB (21,894 files), assets 271 MB, crates 187 MB, docs 136 MB, bins 85 MB, scripts 2.6 MB, vendor 1.1 MB, training 0.8 MB, deploy 0.28 MB |
| bench files at or under 1 MB | 21,686 files, 704 MiB |
| Files over 1 MB at HEAD | 268 (bench 208, assets 46, docs 7, crates 7) |
| Git object store | size-pack 8.20 GiB in 21 packs |
| Blobs no longer at HEAD | 195,789 blobs, 3,660 MiB (largest: `bench/wow/2026-10-03/*.mp4` at 54 MiB, trace tarballs at 49 MiB) |
| Duplicate blob entries at HEAD | 6,595 (73.5 MB) |
| Tracked files matching ignore rules | 602 |
| Tracked `.pyc` / `.DS_Store` | 603 / 1 |
| `.gitignore` files | 185 |
| Untracked, unignored `crates/psionic/target` | 448 MB |
| Root `target/` | 70 GB, no `.dockerignore` |
| Top-level entries | 45 |
| `scripts/` | 242 tracked files (140 .py, 89 .sh, 3 .ps1, 3 .mjs, 2 .pyc), 57,494 LOC; blender + unreal = 31,353 LOC; 84 files directly in `scripts/` |
| Script test files | 31, of which 6 run in `verify-rust.sh` |
| `deploy/` | 81 files, 8,333 LOC, 22 entries, 13 indexed in `deploy/README.md` |
| Shellcheck (warning) | ~27 findings in 15 files, plus 1 parse error that disables checking of `scripts/release/coder.sh` |
| Release script duplication | `coder.sh` and `terminal.sh` share 603 identical lines and 23 functions |
| wasm build scripts with copied lock check | 6 |
| Tracked files containing `/Users/christopherdavid` | 411 |
| Workspace | 182 packages, 83 binary targets |
| Standalone workspaces outside the gate | `crates/psionic`, `crates/openagents-mobile`, `mc-bridge` (nightly-2026-08-03), `deploy/iroh-relay/live-test`, `vendor/boltz-client` |
| CI | `.github/` holds only `ISSUE_TEMPLATE/playtest-report.yml`; workflows removed by policy (latest restore commit `489dfbbf35`, 2026-07-16) |
| `check-dependencies.sh` | hard-fails from 2026-10-20 |

## Strengths

- A large-file guard exists and works. [`scripts/dev/check-large-files.sh`](../../../../scripts/dev/check-large-files.sh) runs from [`.githooks/pre-commit`](../../../../.githooks/pre-commit), and over the last 200 commits it found no new file over 1 MB. It pairs with [`scripts/bench-artifacts.py`](../../../../scripts/bench-artifacts.py), which moves captures to a content-addressed bucket and leaves a sha256 manifest plus a `.gitignore` block in git.
- Shell hygiene is strong. 83 of 89 `.sh` files use `set -e` or `set -euo pipefail`. Most open with a usage block and issue references. Shellcheck at warning level finds about 27 issues across all deploy and scripts shell files.
- The Dockerfiles follow good practice. Root, `deploy/staging` and `deploy/boat-template` are multi-stage builds on debian:13-slim that run as uid 10001, pin rust:1.97.1, strip binaries, and build with `--locked` (except boat-template). `.gcloudignore` keeps the relay upload to what the Dockerfile copies.
- The deploy pipeline is well structured. [`scripts/deploy/web.sh`](../../../../scripts/deploy/web.sh) `stage` builds from a `git archive` of a ref, deploys by digest, and stops when the staging smoke fails. `render.py` writes Secret Manager references and never secret values. The systemd units are hardened, and the env templates hold no credentials.
- The vendored boltz-client is documented. [`vendor/boltz-client/VENDORED.md`](../../../../vendor/boltz-client/VENDORED.md) records the upstream revision, the 404 that forced vendoring, what was left out, the license, and when to remove it.
- `Cargo.lock` is in sync: `cargo metadata --locked --offline` exits 0. Every `-p` or `--bin` reference in scripts and deploy names a real package or binary; psionic-serve goes through `--manifest-path`.
- The changed-scope gate is designed well. `verify-rust.sh` scopes daily runs, makes the full gate opt-in with `--release`, records each phase, and has its own tests (`scripts/tests/test_gate_*.py`).
- The relay's deploy files are checked at compile time. `crates/nostr-relay/tests/deployment_static.rs` pulls `deploy/` files in with `include_str!`, so tests catch drift in the relay's units and config.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| OPS-01 | High | repo-hygiene | 1.82 GB tracked at HEAD and an 8.2 GiB pack; the 1 MB guard misses 704 MiB of bench output under the threshold | L |
| OPS-02 | High | build | `verify-changed.py` maps nested-workspace edits to non-member packages, which breaks `verify-rust.sh --changed` | S |
| OPS-03 | Medium | testing | About 30 crates compile in top-level files that the scoped gate treats as uncovered or maps to the wrong crate | M |
| OPS-04 | Medium | build | Nothing is enforced: CI is banned, the hook is opt-in, gate records are git-ignored | M |
| OPS-05 | Medium | security | Production `promote` reads its launcher from the working tree and keeps a second copy of the `render.py` config | M |
| OPS-06 | Medium | repo-hygiene | `.gitignore` gaps: a 448 MB untracked target, 603 tracked `.pyc` files, 602 tracked files matching ignore rules | S |
| OPS-07 | Medium | build | No `.dockerignore`, gcloudignore uploads untracked targets, poor layer caching, boat-template built without `--locked` | M |
| OPS-08 | Medium | security | The `cargo-deny` gate fails from 2026-10-20 and covers only the root workspace | S |
| OPS-09 | Medium | duplication | Release, signing and wasm build scripts are heavily copy-pasted | M |
| OPS-10 | Medium | testing | Most script tests run in no gate, including the one that guards a live payment timer | S |
| OPS-11 | Medium | policy | The repo-local `NEEDS_OWNER.md` contradicts AGENTS.md, which sends owner steps to the workspace file | S |
| OPS-12 | Medium | repo-hygiene | Evidence screenshots and logs are committed under `bins/` and `docs/`, and blanket allowlist globs exempt them from the guard | M |
| OPS-13 | Low | maintainability | Build scripts hard-code per-agent target directories and the owner's home layout | S |
| OPS-14 | Low | docs | `deploy/` is poorly indexed, and the staging launcher falls back to `/tmp` for WEB_STATE where production requires it | S |
| OPS-15 | Low | architecture | Root `migrations/` serves only nostr-relay, and two migration runners use the same `schema_migrations` table name with incompatible schemas | M |
| OPS-16 | Low | architecture | 45 top-level entries, many of them data directories with one consumer | M |
| OPS-17 | Low | build | Standalone sub-workspaces and vendored extras sit outside every check | S |
| OPS-18 | Low | maintainability | Shellcheck cannot parse the 902-line release script, and no gate runs lint | S |
| OPS-19 | Low | security | Committed benchmark output contains private keys and owner paths, which will trip secret scanners | S |
| OPS-20 | Low | docs | The top-level policy files are too large to review and use very long lines | M |

### OPS-01 Repository bloat: 1.82 GB tracked at HEAD, 8.2 GiB pack, and the 1 MB guard misses 704 MiB of sub-threshold bench output

**Severity:** High · **Category:** repo-hygiene · **Effort:** L

**Locations:**
- [`bench/terminal-bench/traces`](../../../../bench/terminal-bench/traces)
- [`bench/terminal-bench/microcoder-runs`](../../../../bench/terminal-bench/microcoder-runs)
- [`scripts/dev/check-large-files.sh`](../../../../scripts/dev/check-large-files.sh), check-large-files.sh:14-15
- [`scripts/dev/large-files-allowlist.txt`](../../../../scripts/dev/large-files-allowlist.txt), large-files-allowlist.txt:1-6

**Evidence:** `bench/` has 21,894 tracked files totalling 1,131.9 MB. 21,686 of them are 1 MB or smaller and add up to 704.4 MiB. `git count-objects -vH` reports size-pack 8.20 GiB in 21 packs. `check-large-files.sh` refuses only single files larger than `LARGE_FILE_LIMIT` (default 1048576). The allowlist header says bench output "does not belong here" and should go through `bench-artifacts.py push`, but nothing enforces this for small files. The verifier re-measured the bench totals and the pack size exactly. The history-blob and duplicate-blob figures were not re-verified, but they are consistent with the measured pack size.

**Impact:** Every clone, fetch, sandbox fork and build-context upload pays for gigabytes of benchmark output. The guard does not cover the main way the repo grows.

**Suggested action:**
1. Push the run directories under `bench/terminal-bench/{traces,experiments,microcoder-runs}` with `scripts/bench-artifacts.py push`. Then `git rm --cached` them and keep only `summary.json` and the manifest in git.
2. Add an aggregate budget to `check-large-files.sh`: refuse when the staged additions under `bench/` total more than 5 MB or more than 200 new files, unless `BENCH_BUDGET_OK=1` is set. Cover both limits with a test in `scripts/tests`.
3. Rewriting history needs owner sign-off. Until then, document `git clone --filter=blob:limit=1m` in the README.
4. To verify, run `git ls-files bench | wc -l` and `git ls-files -z bench | xargs -0 du -ch | tail -1` and check that both fall sharply. Staging 201 small files under `bench/` should make the hook refuse.

### OPS-02 verify-changed.py maps nested-workspace edits to cargo packages that are not members, which breaks `verify-rust.sh --changed`

**Severity:** High · **Category:** build · **Effort:** S

**Locations:**
- [`scripts/verify-changed.py`](../../../../scripts/verify-changed.py), verify-changed.py:60-68 and verify-changed.py:71-80
- [`scripts/verify-rust.sh`](../../../../scripts/verify-rust.sh), verify-rust.sh:142-163
- [`Cargo.toml`](../../../../Cargo.toml), Cargo.toml:2-11 and Cargo.toml:131

**Evidence:** Running `resolve()` on psionic, mobile, vendor and migrations paths returned `{'crates': ['openagents-mobile','psionic'], 'uncovered': ['vendor/boltz-client/...', 'migrations/0001_store.sql']}`. `package_of()` takes the first `name` line it finds. `crates/psionic/Cargo.toml` contains only a `[workspace]` table, so the function falls back to the directory name. Both crates are in the root `exclude` list (Cargo.toml:4-9). verify-rust.sh:163 turns them into `-p psionic -p openagents-mobile`, and cargo rejects those because they are not members. `vendor/` is also excluded but is patched in through `[patch."https://github.com/breez/boltz-client"]` (Cargo.toml:131). Even so, vendor paths land in `uncovered`. `scripts/tests/test_gate_scope.py` never mentions psionic, mobile or vendor. The verifier reproduced the `resolve()` output.

**Impact:** The dev gate errors out whenever a diff touches psionic or mobile, which the current dirty tree does. Changes to vendored patches are never tested.

**Suggested action:**
1. In `verify-changed.py`, parse the root `Cargo.toml` with `tomllib`. Route paths under `[workspace].exclude` entries to a separate `nested` list of `{manifest}`.
2. Map `vendor/` (a patched path) to `workspace=True`, or to the names of the patched packages.
3. Make `package_of()` require a `[package]` table and stop falling back to the directory name.
4. In `verify-rust.sh`, run fmt, clippy and test with `--manifest-path` for each nested entry.
5. Add psionic, mobile and vendor cases to `test_gate_scope.py`. To verify, edit a file under `crates/psionic` and check that `verify-rust.sh --changed` runs against `crates/psionic/Cargo.toml` and does not error.

### OPS-03 About 30 crates compile in top-level files that the scoped gate treats as uncovered, or maps to the wrong crate

**Severity:** Medium · **Category:** testing · **Effort:** M

**Locations:**
- [`scripts/verify-changed.py`](../../../../scripts/verify-changed.py), verify-changed.py:27-33
- [`crates/nostr-relay/src/store/migration.rs`](../../../../crates/nostr-relay/src/store/migration.rs), migration.rs:37
- [`crates/boat-template/src/main.rs`](../../../../crates/boat-template/src/main.rs), main.rs:671

**Evidence:** `DATA_DIRS` maps only `programs/`, `questions/`, `capabilities/` and `sources/`, and maps all four to `coder`. Resolving every `include_str!`/`include_bytes!` path in `crates/` (psionic excluded) shows 30 crates that include files from top-level directories:
- `migrations/`: nostr-relay
- `tests/`: nostr and nostr-relay
- `nips/`: 4 crates
- `deploy/`: 4 crates, including nostr-relay and retail-service
- `scripts/`: 5 crates, including boat-template and openagents-web
- `assets/`: 8 verse crates
- `questions/`: 4 crates (coder, coder-one, coder-project, openagents-cli), though `DATA_DIRS` maps it only to coder
- `programs/`: coder-delegate and coder-project, not coder

`resolve()` put `migrations/0001_store.sql` in `uncovered`. The reviewer estimated about 40 crates. The verifier counted 30 through include macros and noted that runtime filesystem reads, such as gym recipes, come on top of that.

**Impact:** Editing a migration, a NIP fixture, an embedded script or a questions file either runs no Rust tests for the crates that use it or runs the wrong crate. Severity is medium because the gate is advisory and a `--release` run covers the whole workspace.

**Suggested action:**
1. Generate the map instead of maintaining it by hand. Add `scripts/dev/include-map.py`, which resolves `include_str!`/`include_bytes!` paths per crate and writes `scripts/dev/include-map.json`, mapping each path prefix to its packages.
2. Have `verify-changed.py` load that file and fall back to `DATA_DIRS`.
3. Add tests asserting that `migrations/0001_store.sql` maps to nostr-relay, that `scripts/cloud/boat-fork-ready.sh` maps to boat-template, and that `questions/` maps to all 4 consumers.
4. To verify, `resolve()` on those paths should return the expected packages and leave `uncovered` empty.

### OPS-04 No enforced gate: CI is banned, the pre-commit hook is opt-in, and gate records are git-ignored

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:**
- [`AGENTS.md`](../../../../AGENTS.md), AGENTS.md:182-183
- [`.githooks/pre-commit`](../../../../.githooks/pre-commit)
- [`.github/ISSUE_TEMPLATE/playtest-report.yml`](../../../../.github/ISSUE_TEMPLATE/playtest-report.yml)
- [`scripts/verify-rust.sh`](../../../../scripts/verify-rust.sh), verify-rust.sh:16 and verify-rust.sh:37
- [`.gitignore`](../../../../.gitignore), .gitignore:6

**Evidence:** AGENTS.md:182 says "No GitHub workflows or GitHub-billed automation. Required checks run manually on a contributor machine or on non-GitHub infrastructure." `.github/` contains only `ISSUE_TEMPLATE`. verify-rust.sh:37 sets `record_dir=".coder/verification"`, and .gitignore:6 ignores `/.coder/`. The pre-commit hook takes effect only after `git config core.hooksPath .githooks`, which each clone must run itself.

**Impact:** Nothing stops a failing, unformatted or oversized commit from reaching main. Problems show up only at release or deploy time. Severity is medium because the absence of CI is an explicit, documented owner policy. What can be fixed is the opt-in hook and the lack of a gate off GitHub.

**Suggested action:**
1. Keep the no-GitHub-Actions policy. Add `scripts/dev/post-push-gate.sh`, which runs `check-large-files.sh LAST_GOOD..origin/main` and `verify-rust.sh --changed=LAST_GOOD`, uploads the record to a GCS bucket, and opens an issue on failure.
2. Run it from a systemd timer on CoderOS or from a Cloud Scheduler job.
3. Have `scripts/coderdev` and the Boat fork-ready script set `core.hooksPath=.githooks`.
4. To verify, push a commit with a deliberate fmt error to a scratch ref the gate watches. An issue should open and a record should appear in the bucket.

### OPS-05 Production promote reads its launcher from the working tree and duplicates production config already in render.py

**Severity:** Medium · **Category:** security · **Effort:** M

**Locations:**
- [`scripts/deploy/web.sh`](../../../../scripts/deploy/web.sh), web.sh:51, web.sh:197-215, web.sh:236-280
- [`deploy/production/render.py`](../../../../deploy/production/render.py), render.py:40 and render.py:97-103

**Evidence:** Line 51 sets `ROOT=$(git rev-parse --show-toplevel)`. `promote` passes `"$ROOT/deploy/production/web.sh"` into a heredoc, which runs `launcher = open(launcher_path).read()` and replaces `args[1]` ("web launcher refreshed from deploy/production/web.sh"). The heredoc hard-codes `openagentsgemini-web-chats-prod`, `openagentsgemini-web-analytics-prod`, `ENVIRONMENTS_MODEL google/gemini-3.8-flash`, `boat-api-key` and `THIRD_PARTY_ANALYTICS`, and render.py:40/97/102/103 defines the same values. `promote` checks only that the digest exists in Artifact Registry. At line 197 a failed smoke only prints "do not promote" and does not stop the promote.

**Impact:** An uncommitted edit in the operator's checkout becomes the production entrypoint, and the two copies of the production config can drift apart. Severity is medium because only the operator's own checkout is exposed. This is a trusted operator path, not an external attack surface.

**Suggested action:**
1. Have `stage` write `{sha, digest, smoke}` to `$STATE/staged-<digest>.json`.
2. Have `promote` read the launcher with `git show <sha>:deploy/production/web.sh`. It should refuse a digest that has no passing record unless `--force` is given.
3. Replace the constants in the heredoc with a call into `deploy/production/render.py`.
4. Add `scripts/tests/test_production_render.py`. To verify, promoting with a dirty `deploy/production/web.sh` should still deploy the committed launcher, and promoting an unstaged digest should be refused.

### OPS-06 .gitignore gaps: a 448 MB untracked target, 603 tracked .pyc files, and 602 tracked files that match ignore rules

**Severity:** Medium · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [`.gitignore`](../../../../.gitignore), .gitignore:1-12
- [`scripts/blender/__pycache__/kit.cpython-313.pyc`](../../../../scripts/blender/__pycache__/kit.cpython-313.pyc)
- [`scripts/tests/__pycache__/test_pay_reconcile_unit.cpython-313.pyc`](../../../../scripts/tests/__pycache__/test_pay_reconcile_unit.cpython-313.pyc)
- `crates/psionic/target` (untracked)

**Evidence:** `git check-ignore -v crates/psionic/target` exits 1, so the directory is not ignored, and `du` reports 448M. The root `.gitignore` anchors `/target` and ignores `__pycache__` only under `/training`. `git ls-files -ci --exclude-standard | wc -l` gives 602. `git ls-files '*.pyc' | wc -l` gives 603, 2 of them under `scripts/`. The verifier re-measured every number exactly.

**Impact:** `git add -A` would stage hundreds of MB of build output, and tracked bytecode adds noise to diffs.

**Suggested action:**
1. Add unanchored `target/`, `__pycache__/`, `*.py[co]`, `.venv/`, `.pytest_cache/`, `node_modules/` and `.DS_Store` to the root `.gitignore`.
2. Run `git ls-files -ci --exclude-standard -z | xargs -0 git rm --cached`, and remove the 2 `.pyc` files under `scripts/` as well.
3. Add a check to the pre-commit hook that refuses staged paths that match ignore rules.
4. To verify, `git check-ignore crates/psionic/target` should exit 0, and `git ls-files -ci --exclude-standard` and `git ls-files '*.pyc'` should both print nothing.

### OPS-07 Container build contexts and Dockerfiles: no .dockerignore, gcloudignore uploads untracked targets, uncached layers, unlocked boat-template build

**Severity:** Medium · **Category:** build · **Effort:** M

**Locations:**
- [`Dockerfile`](../../../../Dockerfile), Dockerfile:1-15
- [`.gcloudignore`](../../../../.gcloudignore), .gcloudignore:5-13
- [`deploy/staging/stack.gcloudignore`](../../../../deploy/staging/stack.gcloudignore), stack.gcloudignore:6-14
- [`deploy/boat-template/Dockerfile`](../../../../deploy/boat-template/Dockerfile), Dockerfile:14-17

**Evidence:** The repo has no `.dockerignore`, and the root `target/` is 70 GB. `.gcloudignore` starts with `/*` and re-includes `!/crates/` without excluding target directories and without including `.gitignore`, so the untracked `crates/psionic/target` (448 MB) gets uploaded. `stack.gcloudignore` has the same gap. The root Dockerfile builds only `-p nostr-relay`, but it does so after `COPY crates ./crates`, so an edit to any crate invalidates the dependency layer. Base images are pinned by tag only. boat-template/Dockerfile:14 says "No --locked: main's Cargo.lock is sometimes stale".

**Impact:** Uploads are slow, cache reuse is poor, and the boat-template build is not reproducible.

**Suggested action:**
1. Add `crates/**/target/` (or `**/target/`) after `!/crates/` in both gcloudignore files.
2. Add a root `.dockerignore` with `**/target`, `bench`, `assets`, `bins` and `docs`.
3. Move the root Dockerfile to `deploy/nostr-relay/Dockerfile` and pass it with `-f`. Add BuildKit cache mounts or cargo-chef so the dependency layer survives source edits.
4. Pin base images by digest and restore `--locked` in boat-template.
5. To verify, `gcloud meta list-files-for-upload` should list no `target/` paths, and a second relay build after an edit outside nostr-relay should reuse the dependency layer.

### OPS-08 The cargo-deny gate is set to fail on 2026-10-20 and covers only the root workspace

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [`scripts/check-dependencies.sh`](../../../../scripts/check-dependencies.sh), check-dependencies.sh:7-10 and check-dependencies.sh:13-18
- [`scripts/verify-rust.sh`](../../../../scripts/verify-rust.sh), verify-rust.sh:393-399

**Evidence:** check-dependencies.sh:7 exits 1 on or after 20261020 with "RUSTSEC-2024-0436 requires a new maintainer review". `cargo deny` runs only against the root manifest. verify-rust.sh:398 records a skip ("partial gate") when cargo-deny is not installed. The deadline is deliberate and forces a review, but the fixed date still needs action within 10 days.

**Impact:** From 2026-10-20 the deps phase fails until the review is redone. The nested workspaces (mobile wallet, psionic, mc-bridge) get no advisory scan.

**Suggested action:**
1. Before 2026-10-20, redo the `paste` review in `docs/dependencies.md` and move the date, or drop `paste`.
2. Run `cargo deny` with `--manifest-path` over `crates/openagents-mobile/Cargo.toml`, `crates/psionic/Cargo.toml` and `mc-bridge/Cargo.toml` in a loop.
3. Under `--release`, fail instead of skipping when cargo-deny is missing.
4. To verify, `scripts/check-dependencies.sh` should pass with the date faked to 2026-10-21 and should print one result per manifest.

### OPS-09 Release, signing and wasm build scripts are heavily copy-pasted

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [`scripts/release/coder.sh`](../../../../scripts/release/coder.sh)
- [`scripts/release/terminal.sh`](../../../../scripts/release/terminal.sh)
- [`scripts/build-coder-browser-web.sh`](../../../../scripts/build-coder-browser-web.sh)
- [`scripts/build-coder-chat-web.sh`](../../../../scripts/build-coder-chat-web.sh)
- [`scripts/build-coder-cloud-web.sh`](../../../../scripts/build-coder-cloud-web.sh)
- [`scripts/build-coder-components-web.sh`](../../../../scripts/build-coder-components-web.sh)
- [`scripts/build-bunny-web.sh`](../../../../scripts/build-bunny-web.sh)
- [`scripts/build-everglade-web.sh`](../../../../scripts/build-everglade-web.sh)

**Evidence:** difflib finds 603 matching lines and 23 shared functions between `coder.sh` (902 lines) and `terminal.sh` (793 lines). All 6 `build-*-web.sh` scripts contain the same `name = "wasm-bindgen"` check against `Cargo.lock`.

**Impact:** Signing and notarization fixes have to be made in several places, and the copies drift apart.

**Suggested action:**
1. Extract the shared functions into `scripts/release/lib.sh` and source it from `coder.sh` and `terminal.sh`.
2. Extract `scripts/lib/wasm.sh` with `check_wasm_bindgen` and `build_wasm`, and reduce each `build-*-web.sh` to a thin wrapper that sets an explicit profile.
3. To verify, the duplicate-line count between the two release scripts should drop to near zero, `git grep -c 'name = "wasm-bindgen"' scripts` should match only `scripts/lib/wasm.sh`, and a dry-run release of each app should produce the same artifacts as before.

### OPS-10 Most script test files never run in any gate, including one that guards a live payment timer

**Severity:** Medium · **Category:** testing · **Effort:** S

**Locations:**
- [`scripts/verify-rust.sh`](../../../../scripts/verify-rust.sh), verify-rust.sh:375-383
- [`scripts/tests/test_pay_reconcile_unit.py`](../../../../scripts/tests/test_pay_reconcile_unit.py)
- [`scripts/bench/test-verse-delayed-route.py`](../../../../scripts/bench/test-verse-delayed-route.py)
- [`scripts/bench/tests/test_verse_delayed_route.py`](../../../../scripts/bench/tests/test_verse_delayed_route.py)

**Evidence:** `verify-rust.sh` runs only `test_gate_*.py`, `test_verification_phase.py`, `test_fetch_kev_artifacts.py`, `test_check_coder_delegation_run.py` and `test_backup_media.py`. No `.sh` or `.py` file other than the test itself refers to `test_pay_reconcile_unit`. There are two separate verse delayed-route test files. Of the 31 script test files, 6 run in the gate; the verifier did not recount that split.

**Impact:** Regressions in the payment timer policy and in the release packaging and publishing scripts go unnoticed.

**Suggested action:**
1. Rename the test files to the `test_*.py` pattern.
2. Add a `script-tests` phase to `verify-rust.sh` that runs `python3 -B -m unittest discover -s scripts -p 'test_*.py'`.
3. Merge the two verse delayed-route tests into one.
4. To verify, the new phase's record should list every file from `git ls-files 'scripts/**/test_*.py'`, including `test_pay_reconcile_unit.py`.

### OPS-11 A repo-local NEEDS_OWNER.md contradicts AGENTS.md, which points owner steps to the workspace file

**Severity:** Medium · **Category:** policy · **Effort:** S

**Locations:**
- [`NEEDS_OWNER.md`](../../../../NEEDS_OWNER.md)
- [`AGENTS.md`](../../../../AGENTS.md), AGENTS.md:126-131

**Evidence:** AGENTS.md:129 says "Put those steps in the workspace `NEEDS_OWNER.md`". Even so, `NEEDS_OWNER.md` is tracked in this repo: 2,177 lines, 131,606 bytes and 146 `## ` sections, last changed in `8f13d35cbb` (CMP-01). The workspace contract says the owner reads the workspace copy on GitHub.

**Impact:** Owner steps recorded in this file may never reach the owner.

**Suggested action:**
1. Merge the open sections into the workspace `NEEDS_OWNER.md` and push it to `AtlantisPleb/workspace` main.
2. Replace this repo's file with a one-line pointer, or delete it and add it to `.gitignore`.
3. To verify, every open section of the local file should appear in the workspace copy on GitHub, and the local file should contain no actionable entries.

### OPS-12 Evidence screenshots and logs are committed under bins/ and docs/, with blanket allowlist globs

**Severity:** Medium · **Category:** repo-hygiene · **Effort:** M

**Locations:**
- [`scripts/dev/large-files-allowlist.txt`](../../../../scripts/dev/large-files-allowlist.txt), large-files-allowlist.txt:9-10
- [`bins/coder-ios/verification`](../../../../bins/coder-ios/verification)
- [`bins/openagents-ios/verification`](../../../../bins/openagents-ios/verification)

**Evidence:** Lines 9-10 of the allowlist are `assets/*` and `bins/*`. The file's header says `*` also matches `/`, so a file of any size anywhere under `bins/` passes the guard. The verifier did not re-measure the per-directory MB figures.

**Impact:** Verification captures under `bins/` skip the size guard entirely.

**Suggested action:**
1. Narrow the allowlist to the paths and file types that actually ship, such as `bins/*/host/**` and fonts.
2. Move the `*/verification` captures to the bench-artifacts bucket and keep only manifests in git.
3. To verify, staging a 2 MB file under `bins/coder-ios/verification/` should make the pre-commit hook refuse.

### OPS-13 Build scripts hard-code per-agent target directories and the owner's home layout

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [`scripts/build-coder-browser-web.sh`](../../../../scripts/build-coder-browser-web.sh), build-coder-browser-web.sh:11
- [`scripts/build-coder-chat-web.sh`](../../../../scripts/build-coder-chat-web.sh), build-coder-chat-web.sh:10
- [`scripts/build-coder-cloud-web.sh`](../../../../scripts/build-coder-cloud-web.sh), build-coder-cloud-web.sh:11
- [`scripts/build-coder-components-web.sh`](../../../../scripts/build-coder-components-web.sh), build-coder-components-web.sh:11
- [`scripts/cloud/build-coder-runtime.sh`](../../../../scripts/cloud/build-coder-runtime.sh), build-coder-runtime.sh:6
- [`scripts/coderdev`](../../../../scripts/coderdev), coderdev:28
- [`scripts/deploy/web.sh`](../../../../scripts/deploy/web.sh), web.sh:49

**Evidence:** 7 scripts default to `$HOME/work/openagents-target-agent{0,1,3}`. web.sh:49 sets `SA_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}`. 411 tracked files contain `/Users/christopherdavid`. These are defaults that can be overridden, so severity is low.

**Impact:** Contributors get stray target directories outside their checkout, and agent slot names leak into shared tooling.

**Suggested action:**
1. Add `scripts/lib/env.sh` that defines `OA_TARGET_ROOT` (default `$repo/target`) and `OA_SECRETS_DIR`.
2. Source it from these scripts and replace the `agentN` literals.
3. To verify, `git grep 'openagents-target-agent[0-9]' scripts` should print nothing.

### OPS-14 deploy/ is poorly indexed, and the staging launcher defaults WEB_STATE to /tmp where production requires it

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [`deploy/README.md`](../../../../deploy/README.md)
- [`deploy/staging/web.sh`](../../../../deploy/staging/web.sh), web.sh:13
- [`deploy/production/web.sh`](../../../../deploy/production/web.sh), web.sh:16-20

**Evidence:** staging/web.sh:13 sets `byo=${WEB_STATE:-/tmp}/byo`, while production/web.sh:16 uses `${WEB_STATE:?WEB_STATE is unset}`. `deploy/` has 22 to 23 entries, `deploy/README.md` indexes 13 of them (count not re-verified), and `deploy/production/README.md` does not exist.

**Impact:** A misconfigured staging deploy silently keeps sealed BYO keys on ephemeral `/tmp`, so staging is a less faithful rehearsal of production.

**Suggested action:**
1. Use `${WEB_STATE:?}` in `deploy/staging/web.sh`.
2. Add `deploy/production/README.md`, and give every `deploy/` entry a row in `deploy/README.md`.
3. Enforce the index with `scripts/tests/test_deploy_index.py`, which fails when an entry has no row. To verify, run the test and check that starting staging without WEB_STATE exits with an error.

### OPS-15 Root migrations/ belongs to nostr-relay alone, and both migration runners share an incompatible `schema_migrations` table name

**Severity:** Low · **Category:** architecture · **Effort:** M

**Locations:**
- [`crates/nostr-relay/src/store/migration.rs`](../../../../crates/nostr-relay/src/store/migration.rs), migration.rs:8-26
- [`crates/tenancy/src/db.rs`](../../../../crates/tenancy/src/db.rs), db.rs:279-302
- [`Dockerfile`](../../../../Dockerfile), Dockerfile:10

**Evidence:** The relay creates `schema_migrations(version bigint, name UNIQUE, sha256 NOT NULL with a CHECK)`. tenancy runs `CREATE TABLE IF NOT EXISTS public.schema_migrations(version integer, name, applied_at)`, inserts only `(version, name)`, and skips any version that already has a row. Today the two run against separate databases.

**Impact:** The hazard is latent. If both ever share a database, tenancy's insert fails on `sha256 NOT NULL`, or tenancy silently skips its own versions.

**Suggested action:**
1. Rename the tables to `nostr_relay_schema_migrations` and `tenancy_schema_migrations`, with a guarded rename that runs only if the old table exists.
2. Move `migrations/` to `crates/nostr-relay/migrations`, and update the include paths, the Dockerfile `COPY` lines and the gcloudignore entries.
3. To verify, run both migration runners against one scratch Postgres database. Both should apply cleanly, and `cargo test -p nostr-relay -p tenancy` should pass.

### OPS-16 45 top-level entries, many of them single-consumer data directories

**Severity:** Low · **Category:** architecture · **Effort:** M

**Locations:**
- [`Cargo.toml`](../../../../Cargo.toml), Cargo.toml:2-3
- [`recipes/README.md`](../../../../recipes/README.md)
- [`methods/cosine-distance.json`](../../../../methods/cosine-distance.json)

**Evidence:** The members list in `Cargo.toml` is `"crates/coderbench","crates/*"`, so the first entry is redundant. Only coder-one includes `methods/`. These two points were re-checked; the full ownership map was not.

**Impact:** Ownership of the top-level data is implicit, which is why the gate needs hand-kept path lists (see OPS-03).

**Suggested action:**
1. Move data directories with a single consumer under the crate that owns them, starting with `methods/` into coder-one, and update the include paths.
2. Drop the redundant `"crates/coderbench"` member entry.
3. To verify, `cargo metadata --locked --offline` should still succeed and the top-level entry count should go down.

### OPS-17 Standalone sub-workspaces and vendored extras sit outside every check

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [`mc-bridge/rust-toolchain.toml`](../../../../mc-bridge/rust-toolchain.toml)
- [`deploy/iroh-relay/live-test/Cargo.toml`](../../../../deploy/iroh-relay/live-test/Cargo.toml)
- [`vendor/boltz-client/VENDORED.md`](../../../../vendor/boltz-client/VENDORED.md)

**Evidence:** These are separate workspaces, and `verify-rust.sh` never compiles them. `mc-bridge` pins nightly-2026-08-03. This was not checked in depth, but it is consistent with the phase list in `verify-rust.sh`.

**Impact:** They break silently over time.

**Suggested action:**
1. Add an opt-in `satellites` phase to `verify-rust.sh` that runs `cargo check --locked --manifest-path <manifest>` for each one.
2. To verify, the phase record should show one result per satellite manifest.

### OPS-18 Shellcheck cannot parse the 902-line release script, and lint is not part of any gate

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [`scripts/release/coder.sh`](../../../../scripts/release/coder.sh), coder.sh:258-264

**Evidence:** `shellcheck -S error scripts/release/coder.sh` reports SC1124, SC1073 and SC1072 at line 259. The cause is a `# shellcheck disable=SC2086` comment placed between `case` items. The verifier reproduced this with the local shellcheck.

**Impact:** The release signing script gets no static analysis at all.

**Suggested action:**
1. Move the directive above the `case` statement, or turn `$_files` into an array.
2. Add a `lint-scripts` phase that runs `shellcheck -S warning` over the tracked `.sh` files.
3. To verify, `shellcheck -S error scripts/release/coder.sh` should exit 0.

### OPS-19 Private keys and owner paths in committed benchmark output will trip secret scanners

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:**
- [`bench/terminal-bench/microcoder-runs/coderos-4080-tb21/openssl-selfsigned-cert-1790465361591/events.jsonl`](../../../../bench/terminal-bench/microcoder-runs/coderos-4080-tb21/openssl-selfsigned-cert-1790465361591/events.jsonl)
- [`bench/terminal-bench/leaderboard-reviewed-redactions.json`](../../../../bench/terminal-bench/leaderboard-reviewed-redactions.json)

**Evidence:** `git grep -l 'BEGIN [A-Z ]*PRIVATE KEY' -- bench` returns 5 files, all produced by the openssl benchmark task. 411 tracked files contain `/Users/christopherdavid`.

**Impact:** Secret scanners will raise false alarms, and the owner's home paths are published.

**Suggested action:**
1. In the trace publisher, redact PEM key bodies and replace `$HOME` with `~`.
2. Re-run the publisher over the affected files, or drop them as part of OPS-01.
3. To verify, `git grep -l 'BEGIN [A-Z ]*PRIVATE KEY' -- bench` should print nothing.

### OPS-20 Top-level policy files are too large to review and are written with very long lines

**Severity:** Low · **Category:** docs · **Effort:** M

**Locations:**
- [`INVARIANTS.md`](../../../../INVARIANTS.md)
- [`AGENTS.md`](../../../../AGENTS.md)

**Evidence:** `INVARIANTS.md` is 377,886 bytes in 649 lines, and 46 of those lines are over 2,000 characters. `AGENTS.md` is 58,376 bytes in 883 lines.

**Impact:** Diffs to the invariants are hard to review.

**Suggested action:**
1. Reflow `INVARIANTS.md` to one sentence per line.
2. Split it by area into linked files, keeping `INVARIANTS.md` as the index.
3. To verify, `awk 'length > 400' INVARIANTS.md | wc -l` should return 0.

## Refuted during verification

- **Licensed `crates/openagents-web/kit` leaking through the build context (OPS-07):** that path does not exist in this checkout, so the claim was dropped.
- **`GITHUB_OAUTH_JSON` divergence between the staging and production launchers (OPS-14):** under `set -u` an unset variable already causes an error, so this is moot. The real divergence is the WEB_STATE default.
- **About 40 crates compiling in top-level files (OPS-03):** the verified count through include macros is 30. Runtime filesystem reads come on top of that.
