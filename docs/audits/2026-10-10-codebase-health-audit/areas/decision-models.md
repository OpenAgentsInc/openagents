# Decision models & knowledge

Scope: [DM] `crates/{jev, jev-hosted, kev, lev, laya, microluna, briefing-lab, knowledge, memory-stream, brainstorm-client, oak}`, `packages/jev-probe`, and the top-level `knowledge/` corpus. Snapshot commit `3168c986aa`, audited 2026-10-10.

**Health grade: B**

This area holds about 79K lines of Rust in 11 crates, plus `packages/jev-probe` and the 322-file `knowledge/` corpus. Code inside each crate is disciplined. Tests are dense (jev 200, knowledge 139, lev 125). There are 3 TODOs (two are intentional template placeholders) and one `#[allow]`. Outside tests, unwrap/expect is rare and is mostly invariant expects. Secrets are redacted, I/O is bounded, refusals are typed, and module docs explain intent. The workspace semantic-routing rule is mostly honored. The codebase KB is embedding-only, knowledge search is hybrid BM25 plus cosine with an absolute similarity floor, and the string matching in jev and oak classifies error codes, not user intent. Two lexical paths remain: the BM25-only fallback and briefing-lab's term overlap.

The problems are structural:

- **Repeated infrastructure.** kev and laya carry copies of the same serve stack. oak runs its own HTTP and retry stack next to the jev SDK it already depends on. Refusal codes are bare strings in several crates.
- **Layering creep.** jev, the System One client, also carries about 2.7K lines of OpenAgents account client code. lev depends on the 110K-line gym crate for a few calibration types. knowledge's own module name shadows the `workbench` crate it depends on.
- **Reliability bugs on live or near-live paths.** The bundled knowledge cache is never refreshed and is not seeded for microcoder, gym or verse. The shared embedding cache is written non-atomically and loses updates. microluna's read-only classifier lets some writing commands run in parallel. Several conformance tests report a pass when they skipped.
- **Dormant code still in the build.** microluna (deprecated), briefing-lab and the lev sweep binaries all compile with the workspace.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC (incl. tests) | ~79.3K: jev 16,689; knowledge 14,020; lev 13,899; briefing-lab 7,592; kev 6,889 (196 files, 174 fixtures, 756K); oak 6,244; microluna 5,080; laya 4,216; brainstorm-client 2,270; jev-hosted 2,078; memory-stream 378 |
| packages/jev-probe | 31 files, 60 LOC of fixture code |
| knowledge/ corpus | 322 files, 1.3 MB; 223 admitted entries, 75 candidates; kinds: product 110, method 106, slip 33, tool 25, edge-case 21, environment 3 |
| Test fns | jev 200, knowledge 139, lev 125, briefing-lab 71, microluna 61, kev 47, oak 31, laya 22, brainstorm-client 22, jev-hosted 20, memory-stream 5 (see Refuted); 6 `#[ignore]` total |
| unwrap/expect/panic! (mostly tests) | knowledge 334/13/4; briefing-lab 285/4/0; oak 173/6/5; kev 120/104/26; microluna 106/0/2; lev 58/253/18; jev 62/12/57; jev-hosted 79/17/6 |
| Non-test library expects | lev bridge.rs 8 (mutex/join), lev adapter.rs 6, oak lib.rs 6, brainstorm-client 4 |
| TODO/FIXME; `#[allow]` | 3 (2 intentional placeholders); 1 (laya encode.rs:271 `too_many_arguments`) |
| Approx. pub items | jev 438, knowledge 352, lev 266, kev 110, microluna 108, laya 107 |
| Workspace dependents | jev 21, knowledge 10, jev-hosted 9, memory-stream 2, brainstorm-client 2, microluna 1 (coder-one); kev, lev, laya, briefing-lab, oak 0 |
| Git activity (commits, last) | jev 46 (10-09), knowledge 46 (10-09), lev 39 (09-24), microluna 37 (09-30; deprecated 09-28), kev 35 (10-07), oak 19 (09-30), jev-hosted 13, briefing-lab 5 (all 10-02..10-03), laya 4, memory-stream 2, brainstorm-client 1 |
| Largest files | jev tests/client.rs 2,385; oak src/lib.rs 2,338; microluna tools.rs 1,427; jev-hosted lib.rs 1,402; kev tests/refusals.rs 1,326; briefing-lab explicit_structure.rs 1,253; jev client.rs 1,248; lev policy.rs 1,227 |
| Longest functions | microluna session.rs `run_watched` ~305; briefing-lab lib.rs `assemble_internal` ~305 (637-942); oak main.rs `call` ~182, `run` ~170 |
| `SystemOneRequest` definitions | 4 (jev, kev, laya, lev); deliberate for lev, but kev and laya share copied serve plumbing |
| Unused dependencies | 5: microluna base64, libc; kev tower-http; lev tower-http; oak sha2 |
| CI | none (`.github` holds only ISSUE_TEMPLATE) |

## Strengths

- Tests are dense and named for the behavior they check. jev has 200 test fns. `kev/tests/refusals.rs` and `lev/tests/conformance.rs` run the real jev client against local servers. Keep this cross-implementation conformance.
- `lev/src/api.rs:1-7` explains why the System One contract is implemented separately in several places: independent implementations can each be wrong, and conformance tests catch the differences. The duplication is deliberate and written down.
- Secrets are handled carefully. `jev/src/transport.rs` `redact()` masks credential headers. The Debug impls on `OpenRouterTransport` and `RelayExchange` hide keys. knowledge `search.rs:382-401` refuses key files that group or others can read. jev-hosted `decision_key` creates the key atomically (`create_new`, `0o600`, `hard_link`).
- I/O is bounded. oak `exchange_async` caps responses at `MAX_RESPONSE_BYTES`, brainstorm-client has `bounded_json`, briefing-lab caps `MAX_FILE`/`MAX_BYTES`/`MAX_FILES`, and kev and laya check request shape in `Admission` before evaluating.
- The semantic-routing rule is mostly honored. `knowledge/src/codebase.rs:28-29` says no keyword ranking chooses what is read. Search is hybrid BM25 plus cosine with an absolute `MIN_SEMANTIC_SIMILARITY` floor (`search.rs:38-42`). Decisions go through the typed System One contract.
- There is almost no TODO or `#[allow]` debt. Every crate inherits workspace lints, and module docs cite their design docs.
- `jev/src/doors.rs:1-40` documents the failover rules, and they are unit-tested, including the difference between a door hitting its own limit and a question being refused.
- memory-stream is a dependency-free, wasm-safe arithmetic crate shared by coder and townsfolk, which is the right boundary.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| DM-01 | High | correctness | Bundled knowledge cache never refreshed; only seeded by the `kb` CLI | M |
| DM-02 | Medium | concurrency | Embeddings cache written non-atomically, loses concurrent updates; query cache unbounded | S |
| DM-03 | Medium | duplication | kev and laya carry byte-identical serve infrastructure; Clef is a third System One server | L |
| DM-04 | Medium | testing | Weight- and helper-backed tests pass when they skip | S |
| DM-05 | Medium | maintainability | Refusal codes copied as bare strings; oak disagrees with itself on `quota_exhausted` | M |
| DM-06 | Medium | duplication | oak keeps a second HTTP/retry stack beside jev, plus a third blocking retry loop | M |
| DM-07 | Medium | architecture | jev bundles ~2.7K LOC of OpenAgents account client code via `#[path]` | M |
| DM-08 | Medium | security | Permissive CORS on laya's unauthenticated localhost server; 5 unused deps | S |
| DM-09 | Low | dead-code | briefing-lab is dormant: 305-line function, boxed errors, own stoplist | M |
| DM-10 | Low | policy | Lexical-only retrieval has no written model-boundary exception | S |
| DM-11 | Low | build | `CARGO_MANIFEST_DIR` paths used as runtime defaults in shipped code | S |
| DM-12 | Low | maintainability | knowledge CLI exits 2 on `--help`; two entry points disagree on usage codes | S |
| DM-13 | Low | concurrency | microluna `reads_only` treats some writing commands as reads | S |
| DM-14 | Low | architecture | lev depends on all of gym (~110K LOC) for calibration types | M |
| DM-15 | Low | error-handling | Poisoned lane mutex in lev's helper pool turns later calls into panics | S |
| DM-16 | Low | architecture | knowledge's `workbench` module shadows the `workbench` crate it depends on | S |
| DM-17 | Low | dead-code | Deprecated microluna still built and wired into coder-one and scripts | M |
| DM-18 | Low | repo-hygiene | lev ships six research binaries next to its server | S |
| DM-19 | Low | security | lev adapter parser pre-allocates from an untrusted record count | S |
| DM-20 | Low | docs | knowledge::search docs omit the gateway embedding provider | S |
| DM-21 | Low | testing | jev-probe program digest never verified; same digest reused in a fixture | S |
| DM-22 | Low | security | jev has two credential-header lists; jev-hosted reads its key without a mode check | S |

### DM-01 The bundled knowledge cache is never refreshed and is only seeded by the `kb` CLI, so agent retrieval reads stale or empty entries

Severity: High · Category: correctness · Effort: M

Locations:
- [lib.rs:157-190](../../../../crates/knowledge/src/lib.rs) (`crates/knowledge/src/lib.rs`)
- [build.rs:1-27](../../../../crates/knowledge/build.rs) (`crates/knowledge/build.rs`)
- [cli.rs:292-294](../../../../crates/knowledge/src/cli.rs) (`crates/knowledge/src/cli.rs`)
- [main.rs:461-468](../../../../crates/microcoder/src/main.rs) (`crates/microcoder/src/main.rs`)
- [remote.rs:394-401](../../../../crates/knowledge/src/remote.rs) (`crates/knowledge/src/remote.rs`)
- [runs.rs:90](../../../../crates/gym/src/runs.rs) (`crates/gym/src/runs.rs`)
- [replay.rs:446](../../../../crates/verse/src/replay.rs) (`crates/verse/src/replay.rs`)

Evidence: `build.rs` embeds every `knowledge/*.md` except README as `BUNDLED`. `seed_bundled` (lib.rs:172-190) opens files with `create_new(true)` and skips `AlreadyExists`, so a revised bundled entry never replaces an existing copy. Its only caller is cli.rs:294, and only when `o.bundled` is set. microcoder (main.rs:461) passes `knowledge::default_dir()` to `knowledge::remote::load`, which calls `Base::load(dir)` without seeding (remote.rs:401). gym runs.rs:90 and verse replay.rs:446 also read `default_dir()` directly. When `HOME` is unset, `default_dir` falls back to the CWD-relative `.openagents/knowledge/entries` (lib.rs:166). No test calls `seed_bundled`. Not overwriting is documented as intentional ("without replacing local edits"), but nothing tells an untouched stale copy apart from a user edit.

Impact: On a fresh machine, microcoder's `--kb` retrieval quietly runs against an empty knowledge base until someone runs `kb`. Everywhere else it runs against whatever version was seeded first, so revised or withdrawn entries keep being served. The Microcoder loop backs Coder's terminal (per the microluna README).

Suggested action:
1. Add `knowledge::ensure_seeded(dir)`. It writes a manifest of `{name: sha256}` for the bundled files next to the cache.
2. If a cached file's digest equals a previously bundled digest (the user never edited it), replace it with the current bundled version. Leave edited files alone.
3. Call it from `remote::load` (or `Base::load`) when `dir == default_dir()`, and remove the seeding call from `cli::parse`.
4. When `HOME` is unset, return an error instead of falling back to a CWD-relative path.
5. Verify with tests: a fresh dir gets seeded, a revised bundled entry replaces an untouched copy, and a user-edited copy survives.

### DM-02 The embeddings cache is shared across processes, written non-atomically, and loses concurrent updates; the per-process query cache is unbounded

Severity: Medium · Category: concurrency · Effort: S

Locations:
- [search.rs:806](../../../../crates/knowledge/src/search.rs), search.rs:830-835, search.rs:960-968, search.rs:984-996 (`crates/knowledge/src/search.rs`)

Evidence: `Retriever::new` reads the cache once. A parse failure silently becomes an empty cache (`.ok()...unwrap_or_default()`, 831-835). `save()` does `let _ = std::fs::write(path, text)` (994): no temp file, no rename, no lock, and errors are ignored. Each process writes back its whole in-memory map, so parallel runs overwrite each other's additions. `queries` only ever gets inserts (971-974).

Impact: Lost updates mean paid embedding calls are repeated. A torn write wipes the whole cache on the next load. The query map grows for the life of a long-running worker.

Suggested action:
1. In `save()`, re-read the file and merge entries by model and digest.
2. Write to `path.with_extension("json.tmp.<pid>")`, then rename (the same pattern `codebase.rs` uses).
3. Log parse and write failures instead of discarding them.
4. Cap `queries` at about 256 entries with FIFO eviction.
5. Verify with a test where two `Retriever`s save disjoint digests to the same path and both sets survive.

### DM-03 kev and laya carry byte-identical serve infrastructure, and psionic Clef adds a third local System One server

Severity: Medium · Category: duplication · Effort: L

Locations:
- [kev serve.rs:155-200](../../../../crates/kev/src/serve.rs), kev serve.rs:621-690 (`crates/kev/src/serve.rs`)
- [laya serve.rs:97-142](../../../../crates/laya/src/serve.rs), laya serve.rs:472-543, laya serve.rs:656-664 (`crates/laya/src/serve.rs`)

Evidence: `diff` of kev serve.rs:155-200 against laya serve.rs:97-142 shows they are identical (`host_memory_budget`, meminfo/vm_stat probes). The admission, slot, permit and refusal helpers have the same shapes in both files. The copies have started to drift: laya still layers `CorsLayer::permissive()` (663), while kev declares tower-http (Cargo.toml:11, 29) but never uses it.

Impact: Fixes to admission, memory budgeting or the refusal envelope have to be ported by hand, and the copies are already diverging.

Suggested action:
1. Extract a `systemone-serve` crate containing the axum skeleton, `Admission`, `VariantSlots`/`Permit`, host memory probes, refusal helpers and `BODY_LIMIT`, generic over a `trait DecisionBackend`.
2. Keep each model's own `api.rs`.
3. Port kev and laya to the new crate. Verify with the existing kev `tests/refusals.rs` and laya `tests/conformance.rs`, run with artifacts present (see DM-04).
4. Decide whether laya (4 commits) should be archived now that Clef serves the same route.

### DM-04 Weight- and helper-backed tests report a pass when they actually skip

Severity: Medium · Category: testing · Effort: S

Locations:
- [laya conformance.rs:51-55](../../../../crates/laya/tests/conformance.rs), conformance.rs:107-111
- [kev tests/serve.rs:93](../../../../crates/kev/tests/serve.rs), serve.rs:140, serve.rs:159, serve.rs:179
- [kev tests/encode.rs:22](../../../../crates/kev/tests/encode.rs), encode.rs:64, encode.rs:96
- [lev tests/pool.rs:45-52](../../../../crates/lev/tests/pool.rs), pool.rs:98-105

Evidence: Each of these tests prints a "skipping" message with `eprintln!` and then `return`s when an artifact or helper is missing. `laya/tests` contains only `conformance.rs`, and both of its tests skip unless `LAYA_BUNDLE_DIR` is set. None of the three crates uses `#[ignore]`, and the repo has no CI (`.github` holds only ISSUE_TEMPLATE).

Impact: `cargo test -p laya` / `-p kev` / `-p lev` report green even though the model-parity checks never ran.

Suggested action:
1. Mark these tests `#[ignore = "needs LAYA_BUNDLE_DIR / artifact bundle / lev-bridge"]`.
2. Add `scripts/decision-model-conformance.sh`, which sets up the artifacts and runs the tests with `--ignored`. As an alternative, make the tests panic instead of returning when `OPENAGENTS_REQUIRE_ARTIFACTS=1`.
3. Record the date and digest of the last real run in docs/kev and docs/laya.
4. Verify: without artifacts, `cargo test` lists the tests as ignored rather than passed. With the script, they run and pass.

### DM-05 Refusal-code vocabulary is copied as bare strings across jev, jev-hosted and oak, and oak disagrees with itself about quota_exhausted

Severity: Medium · Category: maintainability · Effort: M

Locations:
- [jev nip_dec.rs:355-393](../../../../crates/jev/src/nip_dec.rs)
- [jev doors.rs:165-178](../../../../crates/jev/src/doors.rs)
- [jev-hosted lib.rs:439-456](../../../../crates/jev-hosted/src/lib.rs), lib.rs:950-956
- [oak lib.rs:41-84](../../../../crates/oak/src/lib.rs)
- [oak main.rs:452-463](../../../../crates/oak/src/main.rs), main.rs:858-879

Evidence: oak defines `UNAVAILABLE_CODES` and `UNRETRYABLE_UNAVAILABLE` as consts (lib.rs:71-84), then lists the same codes again inline at main.rs:459-460, 873-874 and 876-878. Within oak, main.rs:871 treats `quota_exhausted` as Refused, while the exit-code path at main.rs:454 maps any 429 to `EXIT_UNAVAILABLE`, and jev nip_dec maps `quota_exhausted` to 429. jev doors.rs keeps its own `DOOR_OWN_CODES` list, and `jev_hosted::unavailable` keeps another (lib.rs:452).

Impact: Adding or changing a code means editing several places, and oak already gives different outcomes for `quota_exhausted` depending on the code path.

Suggested action:
1. Add `#[non_exhaustive] enum RefusalCode { ..., Other(String) }` in `jev::nip_dec`, with `http_status()`, `is_unavailable()`, `is_retryable()` and `fails_over()`.
2. Replace oak's inline match arms (main.rs:459-460, 871-878) with the enum methods, and decide on a single class for `quota_exhausted`.
3. Have `doors::fails_over` and `jev_hosted::unavailable` call the enum methods.
4. Verify with one table-driven test that lists every code and its class.

### DM-06 oak keeps a second HTTP and retry stack beside jev, plus a third blocking retry loop

Severity: Medium · Category: duplication · Effort: M

Locations:
- [oak lib.rs:376-384](../../../../crates/oak/src/lib.rs), lib.rs:483-590
- [oak main.rs:855-912](../../../../crates/oak/src/main.rs)
- [jev client.rs:280-287](../../../../crates/jev/src/client.rs), client.rs:1152-1162

Evidence: The doc on `Transport` (lib.rs:376-380) says it serves "the routes the SDK does not model: POST /v1/classify and ... GET /v1/models". jev gained `models.rs` on 2026-09-18 and `classify.rs` on 2026-09-22 (oak lib.rs is from 2026-09-21), and `BlockingClient` now exposes `list_models` (1152) and `classify` (1161). `Transport` runs its own tokio runtime and its own `exchange_async` with sleep and backoff (531, 572, 587). main.rs:858-905 wraps `jev::BlockingClient` in yet another retry loop, using `std::thread::sleep` and its own 5 s backoff cap. That stated reason is only partly stale: oak needs the unparsed `/v1/models` document, and jev keeps that `pub(crate)`.

Impact: One service has three retry policies, so Retry-After or idempotency fixes made in jev never reach oak or oak-mcp.

Suggested action:
1. Route oak's classify through `jev::BlockingClient::classify`.
2. Make jev's existing raw models call (client.rs:445, `list_models -> RawResponse`, currently `pub(crate)`) public as a raw variant, and use it for oak's `/v1/models` passthrough.
3. Delete `Transport::exchange`/`exchange_async`.
4. Replace the main.rs:858-905 loop with `jev::RetryPolicy`, adding a `max_retry_after` field if needed.
5. Verify that oak's existing tests pass and that `rg 'thread::sleep|exchange_async' crates/oak` finds nothing.

### DM-07 jev mixes the System One client with about 2.7K LOC of OpenAgents account, team, referral and card client code, using #[path] module attributes

Severity: Medium · Category: architecture · Effort: M

Locations:
- [jev Cargo.toml:6](../../../../crates/jev/Cargo.toml)
- [jev account.rs:30-50](../../../../crates/jev/src/account.rs)

Evidence: The Cargo description reads "Typed async client for TypeSafe AI's System One API". `account.rs` declares six submodules with `#[path = "account_*.rs"]` (30-48). `wc -l crates/jev/src/account*.rs` totals 2,713 lines.

Impact: Every change to OpenAgents account routes recompiles all 21 jev dependents. The SDK's scope is blurred, and the `#[path]` attributes hide the module tree.

Suggested action:
1. Move the files to `crates/jev/src/account/{statement,card,management,policy,referrals,team}.rs` and drop the `#[path]` attributes.
2. Split them into an `openagents-account-client` crate that uses jev's transport, and migrate callers.
3. Verify that `cargo check --workspace` passes and that `rg '#\[path' crates/jev` finds nothing.

### DM-08 Unused dependencies, and permissive CORS on laya's unauthenticated localhost model server

Severity: Medium · Category: security · Effort: S

Locations:
- [laya serve.rs:663](../../../../crates/laya/src/serve.rs)
- [laya bin/laya_serve.rs:44](../../../../crates/laya/src/bin/laya_serve.rs)
- [kev Cargo.toml:11, 29](../../../../crates/kev/Cargo.toml)
- [lev Cargo.toml:11, 31](../../../../crates/lev/Cargo.toml)
- [microluna Cargo.toml:11-12](../../../../crates/microluna/Cargo.toml)
- [oak Cargo.toml:30](../../../../crates/oak/Cargo.toml)

Evidence: laya's router layers `tower_http::cors::CorsLayer::permissive()` (663). The server binds 127.0.0.1 by default (laya_serve.rs:44) and has no auth. Permissive CORS answers the preflight for a JSON POST from any origin. `rg` finds no use of `tower_http` in kev or lev source, no use of `base64` or `libc` in microluna src or tests, and no use of `sha2`/`Sha256` in oak src or tests.

Impact: Any web page the operator visits can drive the local model server. The unused crates slow builds.

Suggested action:
1. Remove `CorsLayer` from laya's router, or gate it behind an explicit `--allow-origin` flag.
2. Remove tower-http from kev and lev (both the dependency and its entry in the `serve` feature list), base64 and libc from microluna, and sha2 from oak.
3. Verify with `cargo udeps` on the build host. Also check that a cross-origin preflight to laya no longer gets `Access-Control-Allow-Origin`.

### DM-09 briefing-lab is a dormant experiment with a 305-line function, boxed errors and a duplicated stoplist

Severity: Low · Category: dead-code · Effort: M

Locations:
- [briefing-lab lib.rs:17](../../../../crates/briefing-lab/src/lib.rs), lib.rs:228-273, lib.rs:637-942

Evidence: `git log` shows 5 commits (4 on 2026-10-02, 1 on 10-03). Line 17 is `pub type Result<T> = ...Box<dyn std::error::Error>`. `terms()` hard-codes its own stoplist (228-273). `assemble_internal` runs about 305 lines (637-942).

Impact: Its file-relevance code drifts away from the #11210 deterministic-first path.

Suggested action:
1. Pick one: fold its git-blob index and symbol extraction into the #11210 implementation (with thiserror errors, a split `assemble_internal`, and the `knowledge::search` stopwords), or move the crate to `backroom`.
2. Record the decision in the #11210 issue or the crate README.

### DM-10 Lexical-only retrieval paths have no written model-boundary exception

Severity: Low · Category: policy · Effort: S

Locations:
- [knowledge search.rs:1-9](../../../../crates/knowledge/src/search.rs), search.rs:812-824
- [briefing-lab lib.rs:228-273](../../../../crates/briefing-lab/src/lib.rs)

Evidence: The module doc says that with no embedder, or when the embeddings call fails, "the ranking is BM25 alone, and the result says why". microcoder uses `Retriever::lexical(base, why)` for private inputs and `--kb-lexical` (main.rs:469-471). briefing-lab uses only term overlap. The result already records the reason in its `why`/`missing` field.

Impact: The workspace rule bans keyword routing for retrieval. This degraded mode is documented in code but is not recorded as an invariant exception, and nothing tracks how often it happens. It affects ranking, not intent routing, so the severity is low.

Suggested action:
1. Add an `INVARIANTS.md` entry that names BM25-only as a degraded retrieval mode and points to the existing `why` field as its record.
2. Have microcoder and gym count lexical-only retrievals in their run summaries.

### DM-11 Compile-time CARGO_MANIFEST_DIR paths are used as runtime defaults in shipped code

Severity: Low · Category: build · Effort: S

Locations:
- [knowledge product.rs:60-75](../../../../crates/knowledge/src/product.rs)
- [coder product_kb.rs:175-181](../../../../crates/coder/src/product_kb.rs)
- [lev bridge.rs:589-600](../../../../crates/lev/src/bridge.rs)

Evidence: `product::default_dir` falls back to `repository().join("knowledge")`, where `repository()` is `env!("CARGO_MANIFEST_DIR")/../..` (product.rs:63-75). `ProductKnowledge::from_env` uses both. The lev bridge looks for `swift/lev-bridge/.build/release/lev-bridge` under the build checkout.

Impact: If the env var is missing, a relocated binary looks for the build machine's checkout.

Suggested action:
1. In release builds, require `OPENAGENTS_PRODUCT_KNOWLEDGE` and `LEV_BRIDGE_BIN` and fail with a clear error when they are missing. Alternatively, embed the product corpus the way `build.rs` does for coding entries.
2. Verify by running a release binary from a temporary directory with the env vars unset. It should produce the typed error, not a path from the build machine.

### DM-12 knowledge CLI exits with status 2 for --help and disagrees with result() on usage exit codes

Severity: Low · Category: maintainability · Effort: S

Locations:
- [knowledge cli.rs:286](../../../../crates/knowledge/src/cli.rs), cli.rs:292-294, cli.rs:302-310, cli.rs:313-325

Evidence: `-h | --help => return Err(USAGE)` (286). `main()` maps every `Err` to exit 2 (307), while `result()` maps the same parse errors to 64 (324). When `o.bundled` is set (the default dir only), `parse()` seeds the cache as a side effect (292-294).

Impact: Wrappers cannot tell help apart from misuse, and the two entry points return different codes.

Suggested action:
1. Return `Parsed::Help`, which exits 0.
2. Use 64 as the single usage exit code in both entry points.
3. Move seeding out of `parse` (see DM-01).
4. Verify with CLI tests: `--help` exits 0 and a bad flag exits 64 through both paths.

### DM-13 microluna's reads_only classifier treats some writing commands as reads

Severity: Low · Category: concurrency · Effort: S

Locations:
- [microluna tools.rs:118-156](../../../../crates/microluna/src/tools.rs)
- [microluna session.rs:634-660](../../../../crates/microluna/src/session.rs)

Evidence: Commands are split on `|`, `;`, `&` and newline, and each piece is approved if its first word is on an allowlist. So `find . -delete`, `find . -exec rm {} \;`, `sort -o out in`, `sed -n -i ...` and `cat $(touch x)` all count as reads. When `parallel_tools` is on, session.rs:636-639 runs turns made only of reads with `join_all`, and 655-659 skip `finish::listing` for reads.

Impact: Writes in a parallel turn can race with each other, and the finish rule misses edits made this way.

Suggested action:
1. Leave `reads_only` unchanged. The crate is deprecated and kept only to reproduce evidence, so changing the classifier would change the behavior it reproduces.
2. Default `parallel_tools` to false for any new run.
3. List the known misclassifications in the microluna README.

### DM-14 lev depends on the whole gym crate (about 110K LOC) for calibration types

Severity: Low · Category: architecture · Effort: M

Locations:
- [lev Cargo.toml:14-17](../../../../crates/lev/Cargo.toml)
- [lev admission.rs:35-36](../../../../crates/lev/src/admission.rs)
- [lev serve.rs:57-58](../../../../crates/lev/src/serve.rs)
- [lev observation.rs:11-12](../../../../crates/lev/src/observation.rs)

Evidence: Non-test code uses `gym::calibrate::{EstimatorConfig, Mismatch, Record}`, `gym::row::DoorIdentity` and `gym::store::Store`. The sweep binaries also use `gym::suite` and `gym::gate`. gym's `.rs` files total 110,396 lines, and gym depends on jev-hosted, knowledge, pay-ledger, receipts and atif.

Impact: lev-serve rebuilds whenever gym or anything in its dependency tree changes. lev has no library dependents and is not deployed, so the impact is limited.

Suggested action:
1. Move `gym::calibrate` and `row::DoorIdentity` (and the slice of `store` that `observation.rs` needs) into a small `calibration` crate.
2. Keep gym only as a dev-dependency, or as a dependency of a separate `lev-lab` crate for the sweep binaries.
3. Verify that `cargo tree -p lev -e normal` no longer lists gym.

### DM-15 A poisoned lane mutex in lev's helper pool turns later calls into panics

Severity: Low · Category: error-handling · Effort: S

Locations:
- [lev bridge.rs:671](../../../../crates/lev/src/bridge.rs), bridge.rs:783, bridge.rs:279, bridge.rs:370

Evidence: `Lane::with` calls `lock().expect` (671), and `Pool::decide` calls `handle.join().expect` (783). No panic path in `Bridge::decide` is known; the only expects are on lock, pipe-take and join. This is defensive hardening.

Impact: A panic inside a lane would leave that lane broken until the process restarts.

Suggested action:
1. Use `lock().unwrap_or_else(PoisonError::into_inner)` and reset the slot to `None`.
2. Map a join error to `Refusal(BridgeError)` for that lane's positions.
3. Verify with a unit test that panics inside a lane closure and then checks that the next call returns a refusal or recovers, without panicking.

### DM-16 The knowledge crate depends on the workbench contract crate, and its own `workbench` module shadows that crate

Severity: Low · Category: architecture · Effort: S

Locations:
- [knowledge Cargo.toml:10-11](../../../../crates/knowledge/Cargo.toml)
- [knowledge workbench.rs:432-451](../../../../crates/knowledge/src/workbench.rs)
- [knowledge prospective.rs:576-584](../../../../crates/knowledge/src/prospective.rs)
- [knowledge product.rs:172](../../../../crates/knowledge/src/product.rs)

Evidence: knowledge declares `pub mod workbench` and implements `::workbench::pane::PaneAdapter` in both workbench.rs:435 and prospective.rs:580, which forces code to disambiguate `::workbench` from `crate::workbench`. The workbench crate is a roughly 2.3K-line contract crate with no path dependencies, so the dependency itself is acceptable.

Impact: The colliding name confuses readers and tooling.

Suggested action:
1. Rename `knowledge::workbench`, for example to `harvest_session`.
2. Keep the `PaneAdapter` impls.
3. Verify that `rg '::workbench::' crates/knowledge` matches only references to the external crate.

### DM-17 The deprecated microluna crate is still built and wired into coder-one and scripts

Severity: Low · Category: dead-code · Effort: M

Locations:
- [microluna README.md:1-25](../../../../crates/microluna/README.md)
- [coder-one Cargo.toml:17](../../../../crates/coder-one/Cargo.toml)
- [scripts/ember.sh](../../../../scripts/ember.sh)
- [scripts/fire-loop.sh](../../../../scripts/fire-loop.sh)

Evidence: The README marks the crate deprecated as of 2026-09-28, and the last commit was 2026-09-30. coder-one depends on it, and both scripts reference it. The README gives a documented reason to keep it (reproducing evidence).

Impact: Every workspace build compiles code that is frozen.

Suggested action:
1. Put coder-one's microluna dependency behind a non-default `microluna` feature.
2. Tag the commit that reproduces the evidence, and point the scripts at that tag.
3. Verify that `cargo build -p coder-one` no longer compiles microluna.

### DM-18 lev ships six research binaries next to its server binary

Severity: Low · Category: repo-hygiene · Effort: S

Locations:
- [lev Cargo.toml:38-76](../../../../crates/lev/Cargo.toml)
- [crates/lev/src/bin](../../../../crates/lev/src/bin)

Evidence: lev has 9 `[[bin]]` entries totalling 3,345 lines, and only lev-serve has `required-features`. measure, band, the seed/order/calibration sweeps and render all build on every `cargo build -p lev`. lev-serve is referenced in `training/lev-adapter/README.md`.

Impact: Experiment code builds together with the serving crate.

Suggested action:
1. Move the sweep, measure, band and render tools to `examples/` or to a `lev-lab` crate.
2. Keep lev-serve, lev-policy and lev-adapter-check as binaries.
3. Update any README or script that invokes the moved tools.

### DM-19 lev adapter parser pre-allocates from an untrusted record count

Severity: Low · Category: security · Effort: S

Locations:
- [lev adapter.rs:220-236](../../../../crates/lev/src/adapter.rs)

Evidence: `Vec::with_capacity(count as usize)` at line 236 uses the `count` read from the header (228) before validating it. A `Record` has three fields (u32, u64, u64), about 24 bytes.

Impact: A corrupt file can request about 100 GB, which can abort the process instead of returning a typed error. macOS overcommit may defer the failure.

Suggested action:
1. Cap the allocation with `.min(bytes.len() / ALIGNMENT)`, or reject the file when `count * ALIGNMENT > bytes.len()`.
2. Verify with a test that feeds a header with `count = u32::MAX` and expects a typed error.

### DM-20 Stale embedding-provider docs in knowledge::search

Severity: Low · Category: docs · Effort: S

Locations:
- [knowledge search.rs:11-14](../../../../crates/knowledge/src/search.rs), search.rs:361-362, search.rs:384-388
- [knowledge lib.rs:12-13](../../../../crates/knowledge/src/lib.rs)

Evidence: The module doc names only OpenAI, OpenRouter and Vertex. `EMBEDDINGS_CHOICES` is "vertex or gateway", and "gateway" appears 37 times in search.rs, but the module doc never mentions it. lib.rs says embeddings come "from crates/openrouter".

Impact: The docs misstate which provider receives the data.

Suggested action:
1. Rewrite the module doc as an ordered provider-selection list that includes the gateway and BYOK.
2. Fix the lib.rs line.
3. Name the OpenAI key file path in the `key_from_file` error message.

### DM-21 jev-probe program digest is never verified, and the same digest is reused for a different program in a fixture

Severity: Low · Category: testing · Effort: S

Locations:
- [packages/jev-probe/package.json:11](../../../../packages/jev-probe/package.json)
- [openagents-cli screen.rs:1386](../../../../crates/openagents-cli/src/screen.rs)

Evidence: `rg` finds the digest `c5d0ebed...` only in package.json and in a screen.rs fixture, where it labels the program `explain-error`.

Impact: An edit to `programs/jev-probe.json` silently invalidates the pinned digest.

Suggested action:
1. Add a test that canonicalizes each `packages/*/programs/*.json` and checks it against the digest in its `package.json`.
2. Use an obviously fake digest in the screen.rs fixture.
3. Verify that editing the program JSON fails the test.

### DM-22 jev keeps two credential-header lists, and jev-hosted's decision key is read without a permission check

Severity: Low · Category: security · Effort: S

Locations:
- [jev transport.rs:69-79](../../../../crates/jev/src/transport.rs), transport.rs:110-112
- [jev-hosted lib.rs:468-472](../../../../crates/jev-hosted/src/lib.rs)
- [knowledge search.rs:390-401](../../../../crates/knowledge/src/search.rs)

Evidence: The local-only guard lists authorization, proxy-authorization, x-api-key, api-key and cookie. `redact()` uses `KEYS` and `OPAQUE`, which also include set-cookie. `decision_key` reads an existing key file without checking its mode, while knowledge's `key_from_file` refuses files where `mode & 0o077` is nonzero.

Impact: The two header lists can drift apart, and a world-readable key is used without warning.

Suggested action:
1. Back both lists with a single `is_credential_header()`.
2. Add the same mode check to `decision_key`.
3. Verify with unit tests: every header name is classified the same way by both paths, and a key file with mode 0644 is refused.

## Refuted during verification

- **"memory-stream has no tests despite NaN and infinity edge-case logic."** `crates/memory-stream/src/lib.rs` has 5 `#[test]` functions (lines 285-356). They cover round-trip and newest-kept, recency halving and negative spans, `min_max` mapping including flat columns, equal term weighting, and drop-oldest eviction. At most, NaN and infinity inputs to `clamp_importance` and `min_max` are untested, which is too minor to be a finding.
- **"Door failover depends on substring matches against third-party error prose."** `DOOR_LIMIT_MESSAGES` is documented in place (doors.rs:190-194) as a fixed table of known protocol errors for doors (Ollama, llama.cpp) that refuse with a bare 400 and no code, so no structured alternative exists. The jev-hosted half of the claim is also wrong. `UNREACHABLE` is a constant in the same crate, used both to build the message (lib.rs:588-774) and to match it (441), so rewording the constant cannot break the check.
