# Gym, evals, benchmarks

**Scope:** `crates/{gym, gym-bridge, gym-leaderboard, verse-gym, eval-runner, ext-eval, coderbench, chat-load-bench, playtest}` plus top-level `bench/` (`bench/terminal-bench`, `bench/verse`). Snapshot `3168c986aa`, 2026-10-10.

**Health grade: C**

The Rust code in this area is careful. Module docs explain the design, production code almost never panics (about 40 unwrap/expect/panic sites outside tests in gym's 110k LOC, nearly all documented invariant expects), and test counts are high (774 in gym, 151 in ext-eval). ext-eval's sandbox fails closed, gym-bridge pins its recipes, and gym-leaderboard produces deterministic output that `check` verifies by regenerating it. The structure is the problem. `crates/gym` is a 110k-LOC crate with 73 public modules holding four products: the decision-model core (about 30k LOC), Terminal-Bench run analysis (`runs_*` about 38k, `terminal_bench*` about 9k), Coder One analytics (`coder_*` about 13k, used only by the gym binary), and private sales/finance reporting (about 5k). Every dependent compiles all of it, including gateway (which uses only `sales_evidence`) and gym-bridge (which uses one date parser). Inside the crate the same work is done several times: canonical-JSON digests with different number rules, hand-walked ATIF readers that ignore the `atif` crate, and hand-rolled CLI parsers that warn on a bad flag and keep going. eval-runner has a real lost-update/torn-write race on its persisted idempotency ledger.

Repo hygiene is the largest issue. `bench/` holds 1.13 GB of tracked blobs at HEAD, 62% of the 1.82 GB tree, and `bench/terminal-bench` alone is 19,084 files and 1.03 GB. #11110 moved verse captures to a bucket but left terminal-bench in git, grandfathered 188 trace paths in the large-file allowlist, and made the hook opt-in with no CI enforcing it. Rust code reaches into these committed traces through build-time `env!("CARGO_MANIFEST_DIR")/../../bench/...` defaults.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC (tracked `.rs`) | gym 110,396 (290 files); ext-eval 21,095; gym-leaderboard 12,057; eval-runner 6,621; coderbench 6,054; verse-gym 4,475; playtest 3,337; gym-bridge 3,239; chat-load-bench 2,660 |
| gym by family | core 30,296 / `runs_*` 38,098 / `coder_*` 13,361 / `terminal_bench*` 9,436 / `sales*` 5,275 / tui 2,216 / bin 7,481 |
| gym largest files | `gate.rs` 5,533 (prod 3,809), `bin/gym.rs` 4,808, `runs_analysis.rs` 3,189, `ab.rs` 2,985, `runs_tui.rs` 2,909 |
| gym `pub mod` in lib.rs | 73 |
| Longest functions | `sales_finance::rebuild` ~851 lines (439-1290), `runs_card_render::rows` ~604, `runs::command` ~513, `sales_weekly::rebuild` ~498 |
| Tests (`#[test]`/`tokio::test`) | gym 774, ext-eval 151, coderbench 84, gym-leaderboard 66, verse-gym 31, playtest 27, eval-runner 22, gym-bridge 15, chat-load-bench 2 |
| unwrap/expect/panic outside tests | gym ~40, eval-runner 32 (26 in `examples/trainer.rs`), chat-load-bench 5, gym-leaderboard 4, verse-gym 4, ext-eval 2, others 0 |
| unwrap incl. tests | gym 1,268 |
| `#[allow]` | gym 38 (28 `cast_precision_loss`), ext-eval 10 |
| Real TODO/FIXME | 0 |
| Stringly / untyped patterns (gym) | 260 `Result<_, String>`; 181 `.pointer(` + 418 `and_then(Value::as_*)`; 385 `assert!(text.contains(` |
| Churn since 2026-09-10 | gym 227 commits (`lib.rs` 55, `bin/gym.rs` 42); gym-bridge 1 commit total |
| Dependents of gym | 13 crates |
| Repo tree at HEAD | 1.82 GB / 38,825 files |
| `bench/` | 1.13 GB / 21,894 files |
| `bench/terminal-bench` | 1.03 GB / 19,084 files: traces 858 MB / 14,744; experiments 83 MB / 3,370; microcoder-runs 57 MB / 408; published 12.5 MB; reference 10.6 MB |
| `bench/verse` | 105 MB / 2,701 files (81 MB JSON) |
| Junk in `bench/` | 601 `.pyc`; 248 `.npy` (51 MB); 1,031 `.log` (13 MB); 526 `trajectory.atif.json` (213 MB) |
| Files with absolute `/Users/<owner>` paths | 73 (reviewer count) |
| Large-file allowlist | 238 lines, 208 under `bench/terminal-bench` (188 traces) |

## Strengths

- Production code rarely panics: about 40 unwrap/expect/panic sites outside tests in gym's 110k LOC, nearly all documented invariant expects (e.g. `crates/gym/src/jobs.rs:692` "the job was checked above"). gym-bridge and coderbench have none.
- ext-eval's sandbox fails closed: `crates/ext-eval/src/sandbox.rs:1-23` refuses with `unconfined_host` and never spawns a child when no confinement backend exists. It clears the environment (`env_clear`) and checks that paths stay inside the sandbox (`sandbox.rs:101-103`).
- gym-bridge pins its recipes: it canonicalizes program and cwd, binds them to an executable digest plus the cwd's device and inode, bounds args and env names, and runs children with `env_clear` and a fixed PATH under supervise limits (`crates/gym-bridge/src/host.rs:50-80, 648-664`). It serializes with an OS file lock (`store.rs:61-70`).
- gym's result store is receipt-chained, verifiable (`verify_chain`) and single-writer, and a test confirms that a second writer is refused (`crates/gym/src/store.rs:326-376, 1390`).
- gym-leaderboard is split by features (contract/view small enough for a phone; generate/client/publish optional, `lib.rs:20-60`). It recomputes verdicts from evidence and refuses to build when they disagree, and its output is byte-deterministic so `check` can regenerate and compare.
- Module-level docs are thorough and give the reasoning behind each design (e.g. `crates/gym/src/admission.rs:1-60`, `crates/eval-runner/src/runner.rs:1-11`).
- All Rust in scope is rustfmt-clean (except two intentional ext-eval fixtures), the crates use workspace lints, and the `#[allow]` count is small and specific.
- #11110 already started the bench-to-bucket move: `scripts/bench-artifacts.py`, sha256 manifests, `docs/repo/size.md`, and a large-file pre-commit check. The fixes below build on it.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| GY-01 | high | repo-hygiene | `bench/terminal-bench` keeps ~1 GB of run output (19k files) in the git tree | L |
| GY-02 | high | concurrency | eval-runner persists its idempotency ledger outside the lock through a shared temp file | M |
| GY-03 | high | architecture | gym is a 110k-LOC god crate holding four products; every dependent compiles all of it | L |
| GY-04 | medium | error-handling | gym CLI parser warns on unknown or malformed flags and keeps running | M |
| GY-05 | medium | duplication | Canonical JSON for digests implemented several times with different number rules | M |
| GY-06 | medium | build | Production code finds committed bench data through build-time `CARGO_MANIFEST_DIR` paths | M |
| GY-07 | medium | duplication | Three untyped ATIF readers; `runs_transcript` swallows read/parse errors | M |
| GY-08 | medium | architecture | gym-bridge pulls the whole gym crate for one date helper | S |
| GY-09 | medium | maintainability | Private sales/finance reporting lives in the benchmark crate, with 500-850-line functions | M |
| GY-10 | medium | repo-hygiene | Near-duplicate trajectory files stored twice per trial | S |
| GY-11 | medium | build | Large-file guard is opt-in, grandfathers the problem, cannot stop many small files | S |
| GY-12 | medium | repo-hygiene | `bench/verse` still carries 105 MB of dated JSON and log output | S |
| GY-13 | low | maintainability | Per-experiment Rust adapters hard-code dated bench directories | M |
| GY-14 | low | repo-hygiene | Python bytecode, numpy arrays and logs from trial sandboxes are committed | S |
| GY-15 | low | concurrency | gym store's `create_new` lock file goes stale after a crash | S |
| GY-16 | low | error-handling | eval-runner shells out to `gcloud` with no timeout | S |
| GY-17 | low | testing | `admission.rs` (tenancy's activation input) has no tests in its own crate | S |
| GY-18 | low | security | Retained traces contain absolute owner home paths | S |
| GY-19 | low | maintainability | gym-bridge code is dense (very long lines, one-letter names) | S |
| GY-20 | low | docs | Workspace members list duplicates coderbench; stale descriptions | S |
| GY-21 | low | testing | Tests pin rendered prose with substring asserts (385 in gym) | M |

### GY-01 bench/terminal-bench keeps ~1 GB of run output (19k files) in the git tree

**Severity:** high · **Category:** repo-hygiene · **Effort:** L

**Locations:**
- [bench/terminal-bench/traces](../../../../bench/terminal-bench/traces)
- [bench/terminal-bench/experiments](../../../../bench/terminal-bench/experiments)
- [bench/terminal-bench/microcoder-runs](../../../../bench/terminal-bench/microcoder-runs)
- [scripts/dev/large-files-allowlist.txt](../../../../scripts/dev/large-files-allowlist.txt)
- [docs/repo/size.md](../../../../docs/repo/size.md)
- [.gitignore](../../../../.gitignore) (.gitignore:43-55)

**Evidence:** `git ls-tree -r -l HEAD bench/terminal-bench` sums to 1,025,639,990 bytes over 19,084 files. All of `bench/` is 1,131,926,209 bytes over 21,894 files. `.gitignore` covers terminal-bench only for `/bench/terminal-bench/experiments/**/*.tar.gz`; every other #11110 rule targets `bench/verse`. 208 of the 238 allowlist lines are terminal-bench paths.

**Impact:** Every clone, worktree and agent checkout carries about 1 GB of working tree plus its pack history. The 1 MB per-blob guard does not slow growth from many small files.

**Suggested action:**
1. Extend `scripts/bench-artifacts.py push` to `bench/terminal-bench/{traces,experiments,microcoder-runs}`, leaving `bench-artifacts.json` and a small summary per run in git.
2. Add `.gitignore` rules for `/bench/terminal-bench/traces/**` (except manifests) and `/bench/terminal-bench/{experiments,microcoder-runs}/**/records/`.
3. `git rm --cached` the migrated paths and drop the 208 allowlist entries.
4. Keep `published/` and `reference/` in git.
5. Verify: `git ls-tree -r -l HEAD bench/ | awk '{s+=$4} END {print s}'` is below 150 MB.

Note: history stays in the pack after removal, so this stops future growth and shrinks the working tree; it does not shrink existing clones' history.

### GY-02 eval-runner persists its idempotency ledger outside the lock through a shared temp file: lost updates and torn writes

**Severity:** high · **Category:** concurrency · **Effort:** M

**Locations:**
- [crates/eval-runner/src/runner.rs](../../../../crates/eval-runner/src/runner.rs) (runner.rs:269-273, 392-394, 498-512, 787-801, 1517-1520)
- [crates/eval-runner/src/store.rs](../../../../crates/eval-runner/src/store.rs) (store.rs:106-109)
- [crates/eval-runner/src/lib.rs](../../../../crates/eval-runner/src/lib.rs) (lib.rs:103-118)
- [crates/eval-runner/src/bin/eval-runner.rs](../../../../crates/eval-runner/src/bin/eval-runner.rs) (eval-runner.rs:29)

**Evidence:** Each site does `let service = state.service.clone(); drop(state); save_service(&service)`. `write_atomic` always opens `path.with_extension("tmp")` with truncate and then renames it. `deliver()` runs `tokio::spawn(runner.handle(event))` per event, and the binary uses the default multi-thread `#[tokio::main]`. Two tasks on different worker threads can write out of order (an older snapshot renamed last) or interleave on `service.tmp`. The `save_service` wrapper at runner.rs:269-273 only logs failures.

**Impact:** After a restart, a claim missing from `service.json` is treated as new, so the request runs and is charged twice. That breaks the module's stated retransmission guarantee. A torn file may not load at all.

**Suggested action:**
1. Serialize writes: hold a dedicated `std::sync::Mutex<u64>` (generation) across clone-and-write and skip snapshots older than the last persisted one, or send snapshots to a single writer task.
2. In `write_atomic`, use `tempfile::NamedTempFile::new_in(parent)` and `.persist()` for a unique temp name.
3. Make `finish()` retry or alarm instead of only logging.
4. Verify with a `#[tokio::test(flavor = "multi_thread")]` that fires concurrent `execute()`s, reloads `Store`, and asserts every claim is present.

### GY-03 gym is a 110k-LOC god crate holding four products; every dependent compiles all of it

**Severity:** high · **Category:** architecture · **Effort:** L

**Locations:**
- [crates/gym/src/lib.rs](../../../../crates/gym/src/lib.rs)
- [crates/gym/Cargo.toml](../../../../crates/gym/Cargo.toml)
- [crates/gateway/Cargo.toml](../../../../crates/gateway/Cargo.toml)
- [crates/gym-bridge/src/sources.rs](../../../../crates/gym-bridge/src/sources.rs) (sources.rs:507)
- [crates/tenancy/Cargo.toml](../../../../crates/tenancy/Cargo.toml)

**Evidence:** `lib.rs` has 73 `pub mod` lines. gateway's only gym paths are 21 references to `gym::sales_evidence`. gym-bridge's only use is `gym::views::rfc3339_seconds` at sources.rs:507. tenancy uses `admission` (22), `gate` (13), `row` (6), `suite` (3), `commitment` (2), `ab`, `coverage` and `store`. None of these dependents use `runs_*`, `coder_*` or `terminal_bench*`.

**Impact:** Changes to benchmark analysis recompile production services (gateway, tenancy), and those services inherit dependencies such as knowledge, pay-ledger and jev-hosted that only the analysis modules need.

**Suggested action:**
1. Extract `gym-core`: suite, questions, gate, row, store, eval, calibrate, ab, regress, coverage, commitment, admission, jobs.
2. Extract `gym-runs`: `runs_*`, `terminal_bench*`, timeline, views, and the tui feature.
3. Move `coder_*` into a coder-analytics crate or module, and `sales_*` into a `sales-reports` crate (see GY-09).
4. Point gateway at `sales-reports` and tenancy at `gym-core`; point gym-bridge at a small time util (see GY-08).
5. Verify with `cargo tree -p gateway -e normal | grep gym` and `cargo tree -p tenancy -e normal` showing no runs/analysis dependencies.

### GY-04 gym CLI parser warns on unknown or malformed flags and keeps running a measurement

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [crates/gym/src/bin/gym.rs](../../../../crates/gym/src/bin/gym.rs) (gym.rs:362-421)

**Evidence:** `other => eprintln!("unknown flag {other}")` continues. `--blocks` and `--timeout` use `.and_then(|n| n.parse().ok())`, so a bad value silently becomes `None` and the default applies. `--door` with no value does `continue`, and a value without `=` only prints. One shared `Options` struct serves all subcommands. The reviewer counted about 36 hand-rolled parsers of this shape across the area (not re-counted).

**Impact:** A misspelled `--partition` or `--store` runs against defaults and appends valid-looking rows and receipts.

**Suggested action:**
1. Change `read_options` to return `Result<Options, String>` and exit 2 on an unknown flag, missing value or parse failure.
2. Preferably migrate to clap derive (already a workspace dependency) with per-subcommand `Args`.
3. Verify with tests asserting that `gym eval --blocks x` and `gym report --bogus` exit non-zero.

### GY-05 Canonical JSON for digests is implemented several times in gym with different number rules

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [crates/gym/src/gate.rs](../../../../crates/gym/src/gate.rs) (gate.rs:2205-2248)
- [crates/gym/src/ab.rs](../../../../crates/gym/src/ab.rs) (ab.rs:2029-2060)
- [crates/gym/src/jobs.rs](../../../../crates/gym/src/jobs.rs) (jobs.rs:1175-1207)
- [crates/route-contract/src/digest.rs](../../../../crates/route-contract/src/digest.rs) (digest.rs:68)

**Evidence:** gate.rs normalizes numbers through f64 (its comment says "30 and 30.0 write the same way"). ab.rs falls through to `other.to_string()` and jobs.rs to `serde_json::to_string(other)`, so neither normalizes. ab and gate are byte-for-byte copies except for the Number arm. jobs.rs:1175 claims "the same canonicalization the tenancy manifest's digest uses", but nothing enforces it. The reviewer counted at least 7 such functions; the admission/suite/coder_policy/store sites were not individually re-checked.

**Impact:** Copies drift on fixes. A mismatch matters where two writers digest the same document, and jobs.rs makes exactly that cross-crate claim without a test.

**Suggested action:**
1. Freeze current digests with golden tests for each site (as `gate_digest` already does).
2. Add `gym::digest::{canonical, canonical_normalized_numbers, sha256_hex}` and replace the local copies, keeping gate's float normalization as the explicitly named variant.
3. Add a test that `jobs::canonicalize` matches tenancy's manifest canonicalizer on a shared fixture containing both `30` and `30.0`.

### GY-06 Production code finds committed bench data through build-time CARGO_MANIFEST_DIR paths

**Severity:** medium · **Category:** build · **Effort:** M

**Locations:**
- [crates/gym/src/runs.rs](../../../../crates/gym/src/runs.rs) (runs.rs:85)
- [crates/gym/src/runs_card.rs](../../../../crates/gym/src/runs_card.rs) (runs_card.rs:82)
- [crates/gym/src/coder_router.rs](../../../../crates/gym/src/coder_router.rs) (coder_router.rs:59)
- [crates/gym/src/bin/gym_terminal.rs](../../../../crates/gym/src/bin/gym_terminal.rs) (gym_terminal.rs:138)
- [crates/gym/src/bin/gym/terminal_bench_cli.rs](../../../../crates/gym/src/bin/gym/terminal_bench_cli.rs) (terminal_bench_cli.rs:88)
- [crates/gym/src/questions.rs](../../../../crates/gym/src/questions.rs) (questions.rs:531)
- [crates/gym/src/gate.rs](../../../../crates/gym/src/gate.rs) (gate.rs:1863)
- [crates/coderbench/src/lib.rs](../../../../crates/coderbench/src/lib.rs) (lib.rs:1447-1461)

**Evidence:** Non-test defaults such as `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces")` (runs.rs:85) and `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench")` (terminal_bench_cli.rs:88), plus coderbench's `tasks_dir`/`goldens_dir`/`capabilities_dir`. questions.rs and gate.rs fall back to it only after env overrides. About 20 such non-test defaults exist across the area (reviewer count).

**Impact:** A binary run outside the build checkout silently resolves the builder's paths, and the code is hard-coupled to in-tree traces, which blocks the bucket migration in GY-01.

**Suggested action:**
1. Add a `repo_root()` helper that resolves an `OPENAGENTS_REPO` env / `--repo` override, then `git rev-parse --show-toplevel` from the cwd, then the compile-time path only if it exists, with a clear error otherwise.
2. Route every bench default through it.
3. Verify: `rg 'env!\("CARGO_MANIFEST_DIR"\)' crates/{gym,coderbench}/src` returns only `#[cfg(test)]` sites and the helper; run a gym binary from `/tmp` and confirm it errors clearly or honors the override.

### GY-07 Three untyped ATIF trajectory readers in gym, and runs_transcript swallows read/parse errors

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [crates/gym/src/runs_transcript.rs](../../../../crates/gym/src/runs_transcript.rs) (runs_transcript.rs:2021-2040)
- [crates/gym/src/timeline.rs](../../../../crates/gym/src/timeline.rs) (timeline.rs:659-680)
- [crates/gym/src/runs_replay.rs](../../../../crates/gym/src/runs_replay.rs) (runs_replay.rs:542-551)
- [crates/gym-bridge/src/sources.rs](../../../../crates/gym-bridge/src/sources.rs) (sources.rs:238-260)
- [crates/atif/src/document.rs](../../../../crates/atif/src/document.rs) (document.rs:120)

**Evidence:** `runs_transcript::trajectory` does `std::fs::read(path).map_err(|_| ())...` and on any error `return transcript;` (empty), then walks `.pointer("/agent/name")` and similar on a raw `Value`. The other readers were not re-opened during verification.

**Impact:** An unreadable or renamed-field trace shows as an empty transcript instead of an error, and the readers can drift from each other and from the `atif` crate.

**Suggested action:**
1. Add `atif::read(path) -> Result<Document, Error>` in the `atif` crate.
2. Rebuild the gym readers (and gym-bridge's) on it.
3. Make `runs_transcript` record a visible warning on failure instead of returning empty.
4. Add a conformance test that reads a set of sample attempts through every reader and compares results.

### GY-08 gym-bridge pulls the whole gym crate for one date helper and re-implements run parsing

**Severity:** medium · **Category:** architecture · **Effort:** S

**Locations:**
- [crates/gym-bridge/Cargo.toml](../../../../crates/gym-bridge/Cargo.toml)
- [crates/gym-bridge/src/sources.rs](../../../../crates/gym-bridge/src/sources.rs) (sources.rs:507)
- [crates/gym/src/views.rs](../../../../crates/gym/src/views.rs)

**Evidence:** `rg 'gym::' crates/gym-bridge/src` finds exactly one use, `gym::views::rfc3339_seconds` at sources.rs:507. `git log -- crates/gym-bridge` shows 1 commit.

**Impact:** verse and openagents-cli inherit gym's whole build through gym-bridge, and gym-bridge's duplicated status classifiers can drift from gym's.

**Suggested action:**
1. Drop the gym dependency and use a small shared RFC 3339 parser (or the `time` crate), or depend on an extracted `gym-runs` crate (GY-03) and reuse its readers.
2. Add a parity test over one microcoder run directory comparing gym-bridge's classification with gym's.
3. Verify with `cargo tree -p gym-bridge | grep -w gym` returning nothing (or only `gym-runs`).

### GY-09 Private sales and finance reporting lives in the benchmark crate, with 500-850-line functions

**Severity:** medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [crates/gym/src/sales_finance.rs](../../../../crates/gym/src/sales_finance.rs) (sales_finance.rs:439-1290, sales_finance.rs:1407)
- [crates/gym/src/sales_weekly.rs](../../../../crates/gym/src/sales_weekly.rs) (sales_weekly.rs:210-708)
- [crates/gym/src/sales_evidence.rs](../../../../crates/gym/src/sales_evidence.rs) (sales_evidence.rs:284)
- [crates/gateway/src/team_reports.rs](../../../../crates/gateway/src/team_reports.rs)

**Evidence:** `sales_finance::rebuild` starts at line 439 and ends near 1290, where `merge_revenue` begins. The `O_NOFOLLOW|O_NONBLOCK` open flags are duplicated at sales_evidence.rs:284 and sales_finance.rs:1407. gateway's only gym use is `sales_evidence`.

**Impact:** Money-adjacent reporting is reviewed and built as benchmark code, and its very large functions are hard to unit-test.

**Suggested action:**
1. Move `sales_*` into a `sales-reports` crate, with an alias reader for the old schema names.
2. Split `rebuild()` into load / verify / join / tally / render steps, each unit-tested.
3. Extract one `open_regular_nofollow` helper used by both call sites.
4. Verify: gateway builds against `sales-reports` without depending on gym; existing sales tests pass unchanged.

### GY-10 Near-duplicate trajectory files are stored twice per trial

**Severity:** medium · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [atrx-vep-crispr__n5F2AiC.json](../../../../bench/terminal-bench/traces/tb4--coder-one-tunable-v5--atrx-vep-crispr/atrx-vep-crispr__n5F2AiC.json)
- [atrx-vep-crispr__n5F2AiC.episode/trajectory.atif.json](../../../../bench/terminal-bench/traces/tb4--coder-one-tunable-v5--atrx-vep-crispr/atrx-vep-crispr__n5F2AiC.episode/trajectory.atif.json)

**Evidence:** 526 tracked `trajectory.atif.json` files totalling 213,502,407 bytes. The cited pair is 3,588,530 and 3,588,293 bytes.

**Impact:** About 200 MB of avoidable weight and two sources of truth for each trajectory.

**Suggested action:**
1. Keep only `trajectory.atif.json`; make tbench retention write only that file.
2. Update readers (gym runs, gym-leaderboard `tb4_delegate*`, `tests/evidence.rs`) to use it.
3. Delete the sibling `X.json` files during the GY-01 migration.
4. Verify gym-leaderboard `check` still regenerates byte-identical output.

### GY-11 The large-file guard is opt-in, grandfathers the problem, and cannot stop many small files

**Severity:** medium · **Category:** build · **Effort:** S

**Locations:**
- [scripts/dev/check-large-files.sh](../../../../scripts/dev/check-large-files.sh)
- [scripts/dev/large-files-allowlist.txt](../../../../scripts/dev/large-files-allowlist.txt)
- [.githooks/pre-commit](../../../../.githooks/pre-commit)
- [docs/repo/size.md](../../../../docs/repo/size.md)

**Evidence:** `.github` contains only `ISSUE_TEMPLATE` (no workflows). The allowlist has 238 lines, 208 of them under terminal-bench.

**Impact:** Clones without the hook installed keep committing trace output, and a per-blob size limit does not catch thousands of small files.

**Suggested action:**
1. Add an aggregate per-range check (file count and total bytes added under `bench/`) to `check-large-files.sh`.
2. Run it from a pre-merge gate or a push Action over `origin/main..HEAD`.
3. Delete the grandfathered entries after the GY-01 migration.
4. Verify by committing a synthetic batch of 500 small files under `bench/` in a scratch branch and confirming the check fails.

### GY-12 bench/verse still carries 105 MB of dated JSON and log output

**Severity:** medium · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [bench/verse](../../../../bench/verse)
- [.gitignore](../../../../.gitignore) (.gitignore:45-55)

**Evidence:** `bench/verse` is 104,531,467 bytes in 2,701 files at HEAD. The `.gitignore` rules ignore only media/gz/zip/npy.

**Impact:** About 100 MB per checkout, mostly one-off reports.

**Suggested action:**
1. Extend `bench-artifacts.py push` to large `.json`/`.log`/`.ndjson`/`.patch` files under `bench/verse/<date>/`, keeping `summary.md` and `bench-artifacts.json`.
2. Add the matching ignore rules.
3. Keep any files that code reads (check with `rg 'bench/verse' crates`).
4. Verify `bench/verse` falls below an agreed budget (e.g. 15 MB) at HEAD.

### GY-13 Per-experiment Rust adapters hard-code dated bench directories

**Severity:** low · **Category:** maintainability · **Effort:** M

**Locations:**
- [crates/gym-leaderboard/src/tb4_delegate_dev.rs](../../../../crates/gym-leaderboard/src/tb4_delegate_dev.rs) (tb4_delegate_dev.rs:37-51)
- [crates/gym-leaderboard/src/tb4_delegate.rs](../../../../crates/gym-leaderboard/src/tb4_delegate.rs) (tb4_delegate.rs:35-43)
- [crates/gym-leaderboard/src/generator.rs](../../../../crates/gym-leaderboard/src/generator.rs) (generator.rs:118)

**Evidence:** Reviewer cites const experiment paths and a hard-coded board vec at generator.rs:118. Not re-opened in detail during verification.

**Impact:** Adding or moving a board requires code changes, and tests depend on live experiment directories.

**Suggested action:**
1. Add a declarative `boards.json` registry read by the generator.
2. Parameterize the near-identical adapters over that registry.
3. Point tests at slim fixtures instead of live experiment directories; verify `check` output is unchanged.

### GY-14 Python bytecode, numpy arrays and logs from trial sandboxes are committed

**Severity:** low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [.gitignore](../../../../.gitignore) (.gitignore:43-55)
- [bench/terminal-bench/traces](../../../../bench/terminal-bench/traces)

**Evidence:** 601 tracked `.pyc` and 248 `.npy` files under `bench/`. `.npy` is ignored only under `/bench/verse`.

**Impact:** Useless build caches and arrays inflate the tree. (Subset of GY-01; `.pyc` bytes are small.)

**Suggested action:**
1. Add global `__pycache__/`, `*.pyc` and `/bench/**/*.npy` ignore rules.
2. `git rm -r --cached` the matches.
3. Make tbench retention skip `__pycache__`.
4. Verify `git ls-files | grep -cE '\.pyc$|\.npy$'` returns 0.

### GY-15 gym store's create_new lock file goes stale after a crash, unlike gym-bridge's OS lock

**Severity:** low · **Category:** concurrency · **Effort:** S

**Locations:**
- [crates/gym/src/store.rs](../../../../crates/gym/src/store.rs) (store.rs:848-903)
- [crates/gym-bridge/src/store.rs](../../../../crates/gym-bridge/src/store.rs) (store.rs:61-70)

**Evidence:** gym's store.rs documents that "a writer killed outright leaves the file behind" and uses `create_new`. gym-bridge uses `lock.try_lock()`.

**Impact:** A SIGKILLed `gym eval` blocks later appends until someone deletes the lock by hand.

**Suggested action:**
1. Take `File::try_lock` on the pid file and treat an existing but unlocked file as stale.
2. Keep the single-writer refusal test (store.rs:1390) passing.
3. Add a test that leaves a lock file without a held OS lock and asserts the next writer proceeds.

### GY-16 eval-runner shells out to gcloud with no timeout to upload blobs

**Severity:** low · **Category:** error-handling · **Effort:** S

**Locations:**
- [crates/eval-runner/src/wire.rs](../../../../crates/eval-runner/src/wire.rs) (wire.rs:124-151)

**Evidence:** `std::process::Command::new("gcloud")...output()` has no bound, and errors report only `stderr.lines().last()`.

**Impact:** A hung `gcloud` stalls the publish indefinitely.

**Suggested action:**
1. Spawn with a deadline (`supervise::Job` with `Limits`, or `wait_timeout`).
2. Include a bounded stderr tail (e.g. last 2 KB) in the error.
3. Verify with a test that substitutes a sleeping fake `gcloud` on PATH and asserts a timeout error.

### GY-17 admission.rs (tenancy's activation input) has no tests in its own crate

**Severity:** low · **Category:** testing · **Effort:** S

**Locations:**
- [crates/gym/src/admission.rs](../../../../crates/gym/src/admission.rs)
- [crates/lev/tests/admission.rs](../../../../crates/lev/tests/admission.rs)
- [crates/tenancy/src/admission.rs](../../../../crates/tenancy/src/admission.rs)

**Evidence:** No `#[cfg(test)]` in `gym/src/admission.rs`, and no file in `crates/gym/tests` mentions admission. `crates/lev/tests/admission.rs` exists. tenancy references `gym::admission` 22 times.

**Impact:** Running only gym's tests after editing the admission rules misses regressions.

**Suggested action:**
1. Add `crates/gym/tests/admission.rs` covering `Plan::decide` refusal and unverifiable cases.
2. Add a golden `Decision` digest test.
3. Verify with `cargo test -p gym --test admission`.

### GY-18 Retained traces contain absolute owner home paths

**Severity:** low · **Category:** security · **Effort:** S

**Locations:**
- [bench/terminal-bench/traces](../../../../bench/terminal-bench/traces)
- [crates/gym-leaderboard/src/scrub.rs](../../../../crates/gym-leaderboard/src/scrub.rs)

**Evidence:** Reviewer reports 73 tracked files with `/Users/<owner>` paths (not re-counted). The scrubber runs only for published bundles.

**Impact:** Publicly leaks the local username and directory layout.

**Suggested action:**
1. Scrub whatever stays in git during the GY-01 migration.
2. Add a check (in `check-large-files.sh` or a sibling) refusing `/Users/` or `/home/<name>/` in new files under `bench/`.
3. Verify `git grep -l '/Users/' -- bench/` returns nothing.

### GY-19 gym-bridge code is dense and hard to review (very long single lines, one-letter names)

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:**
- [crates/gym-bridge/src/host.rs](../../../../crates/gym-bridge/src/host.rs) (host.rs:554)
- [crates/gym-bridge/src/main.rs](../../../../crates/gym-bridge/src/main.rs) (main.rs:80)

**Evidence:** Reviewer cites a 472-character line at host.rs:554 and a 499-character line at main.rs:80 (not re-measured).

**Impact:** Process-spawning code is harder to review.

**Suggested action:**
1. Extract a `run_row` helper and a `const HELP: &str`.
2. Rename one-letter bindings in `PinnedRecipe::admit`.
3. Verify with `awk 'length > 160' crates/gym-bridge/src/*.rs` returning nothing.

### GY-20 Workspace members list duplicates coderbench; stale descriptions

**Severity:** low · **Category:** docs · **Effort:** S

**Locations:**
- [Cargo.toml](../../../../Cargo.toml) (Cargo.toml:2-3)
- [bench/terminal-bench/pyproject.toml](../../../../bench/terminal-bench/pyproject.toml) (pyproject.toml:4)
- [crates/gym/Cargo.toml](../../../../crates/gym/Cargo.toml) (Cargo.toml:7)

**Evidence:** `members = [\n    "crates/coderbench","crates/*"]`, which is redundant with the glob and oddly formatted.

**Impact:** Minor confusion.

**Suggested action:**
1. Set `members = ["crates/*"]`.
2. Update the pyproject and gym crate descriptions to match current contents.
3. Verify with `cargo metadata --no-deps` listing coderbench once.

### GY-21 Tests pin rendered prose with substring asserts (385 in gym)

**Severity:** low · **Category:** testing · **Effort:** M

**Locations:**
- [crates/gym/src/coder_asks.rs](../../../../crates/gym/src/coder_asks.rs) (coder_asks.rs:541-593)
- [crates/gym/src/runs_tui_ask.rs](../../../../crates/gym/src/runs_tui_ask.rs) (runs_tui_ask.rs:881-896)

**Evidence:** Reviewer counts 385 `assert!(x.contains("..."))` in gym (not re-counted).

**Impact:** Copy fixes break many tests, and partial asserts miss other regressions in the same output.

**Suggested action:**
1. Assert on typed models for logic.
2. Use golden or insta snapshots for rendered text, starting with `runs_card_render` and `coder_asks`.
3. Track the count (`rg -c 'assert!\(.*\.contains\(' crates/gym/src`) and require it to go down.
