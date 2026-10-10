# Nostr stack

**Scope:** `crates/nostr`, `crates/nostr-relay`, `crates/nostr-transport`, `crates/discovery`, and `nips/`. Audit date 2026-10-10, snapshot `3168c986aa`. Items from [the 2026-09-26 Nostr adoption audit](../../2026-09-26-nostr-adoption/) are repeated here only where they are still open.

**Health grade: C**

The stack is about 115.7K lines of Rust across 4 crates, plus about 2.5 MB of NIP specs. Line-level discipline is very good. Production code (code above each file's first `#[cfg(test)]`) has 9 `unwrap` and 38 `expect` in about 57K lines of `nostr`, and 0 `unwrap` and 14 `expect` in about 15K lines of `nostr-relay`. Every `expect` states its invariant. Both main crates use `#![forbid(unsafe_code)]` and keep a dependency allowlist. The relay's maps and sockets have explicit bounds, and NIP-44 is checked against the official vectors and differential oracles.

The weak points are architecture, test running and follow-through:

- `crates/nostr` describes itself as "pure Nostr protocol primitives", but it is an 84.8K-LOC crate with 37 public modules. These include product contracts such as `xp` (7.3K LOC), `eval_ext`, `market_contracts`, `pylon` and `x402`. About 60 crates depend on it, so an XP change rebuilds the whole product.
- The relay's core paths are very large functions: `admit_inner` is 637 lines and `Gateway::start` is 375. They are tested mainly through 13 Postgres suites (about 9K LOC, 16 test functions). Those suites pass silently when the database env var is missing, and the repo has no CI workflow.
- Two items from the 2026-09-26 audit are still open. NEG-OPEN skips the auth-required and REQ rate-limit checks (B05), and `RemoteSigner::cipher` encrypts with fixed nonces (B03).
- NIP-42 client handshakes are still hand-written in about 6 crates, even though `nostr-transport` exists and about 7 crates use it.
- There is a cluster of cleanup debt:
  - 4 private event signers that do the same thing
  - 14 hex helpers
  - a 424-line if-chain over magic kind numbers
  - about 1.2K lines of test-only evidence text compiled into every client
  - an empty `lane_shapes.inc` that makes 45 assertions vacuous
  - a dead `server` feature
  - wrong env-var names in the docs, which would silently disable NIP-29 for an operator who follows them

The grade is C because the structure, the way tests are run, and the open security follow-ups outweigh the careful line-level code.

## Measurements

| Metric | Value |
|---|---|
| LOC: `nostr` | 84,833 in 152 files. Largest files: `decision.rs` 2910, `execution.rs` 2567, `eval_ext.rs` 2216, `ext.rs` 2020. By module: `domain/` 33,499, `xp` 7266, `eval_ext` 4244, `contracts` 3374 |
| LOC: `nostr-relay` | 27,795 total, about 15.4K of it production. `store/mod.rs` 3480, `gateway/server.rs` 2692. 13 `*_postgres.rs` test files = 9,009 LOC |
| LOC: `nostr-transport` / `discovery` | 345 / 2,754 |
| `nips/` | official 816K (100 files), block 524K (18), openagents 1.1M (31 `NIP-*.md` files plus contracts, 16,369 lines) |
| Tests (`#[test]` / `#[tokio::test]`) | nostr 505, nostr-relay 111 (only 16 in the 13 Postgres suites; `store_postgres.rs` has 1 test in 1027 lines), nostr-transport 2, discovery 20 |
| Production unwrap / expect / panic | nostr 9/38/0, relay 0/14/0, transport 0/0/0, discovery 2/1/0 |
| unwrap including tests | nostr 1805, relay 130 |
| `#[allow]` | nostr 4, relay 5 |
| TODO / FIXME | 0 |
| Public API | nostr: 947 pub fns, 919 pub types, 37 pub modules. relay: 181 pub fns |
| Functions of 150+ lines | `admit_inner` 637, `validate_expanded_event` 424, `Gateway::start` 375, db `handle_request` 268, `open_peer_order` 262, curated `resolve` 256, `serve_upload` 255, config `validate` 244, `handle_event` 223 |
| Dependents | `nostr`: 58 Cargo.toml files. `nostr-relay`: 3 (openagents-cli, push-gateway, eval-runner). `nostr-transport`: about 7 crates, imported in about 30 `.rs` files. `discovery`: 4 |
| Churn since 2026-09-10 | 130 commits touch `crates/nostr/src` (file-touches: domain 222, lane 66, `lane_shapes.inc` 65, xp 40) |
| `Result<_, String>` in nostr | 65 (not re-counted) |
| Hex helper copies | 14 across nostr, nostr-relay and discovery |
| Relay env vars | 65 read, 13 undocumented (not re-counted) |
| CI | No `.github/workflows` directory |

## Strengths

- **Production panic discipline is excellent.** About 15K lines of relay production code contain 0 `unwrap()`, and every remaining `expect` states its invariant (for example `wire.rs:361-395`, "serializing a validated event cannot fail").
- **Unsafe code and dependencies are locked down.** `nostr` and `nostr-relay` both use `#![forbid(unsafe_code)]`. Their Cargo.toml dependency allowlists give a written reason for each entry (`ryu-js` is there for RFC 8785, and the crypto oracles are dev-dependencies only).
- **NIP-44 is well tested.** The hand-written primitives are differentially tested against RustCrypto and the pinned official vectors. The decision to keep them is recorded in `docs/nostr/crypto-primitives.md`.
- **One kind registry guards against spec drift.** `kinds/tests.rs` cross-checks `kinds.rs` against `nips/openagents/README.md`, catches kinds claimed twice, and refuses kinds that the official NIP list already assigns.
- **The relay is bounded throughout:**
  - rate maps are capped at `MAX_RATE_KEYS=100_000`, and worlds at `MAX_WORLDS`
  - `NOTIFICATION_QUEUE_CAPACITY` bounds the notification queue, and each connection has a subscription cap
  - admission statements are pipelined in one transaction (`store/mod.rs:584-660`)
  - LISTEN is active before the cursor is sampled (`server.rs:279-282`)
- **`nostr-transport` has hard limits.** It enforces deadlines of at most 120 s and frame budgets of 256 (up to 4096). It also picks the rustls provider explicitly, so feature unification cannot select a different provider and cause a panic (`lib.rs:21-34`).
- **Test signing stays out of products.** The `test-invoice` signing feature is enabled only from `[dev-dependencies]` in all 8 consumers, so it never reaches a product binary.
- **Fixtures are thorough.** `crates/nostr/fixtures/**/valid|invalid` covers decisions and evals, and the relay has deterministic wire fuzz fixtures.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| NO-01 | High | security | NEG-OPEN skips the auth-required and REQ rate-limit checks (B05 still open) | S |
| NO-02 | High | architecture | `nostr` is an 85K-LOC catch-all with product contracts, and about 60 crates depend on it | L |
| NO-03 | High | testing | Relay Postgres suites pass silently without a DB; tests are single huge functions; no CI | M |
| NO-04 | Medium | security | `RemoteSigner::cipher` uses fixed NIP-44 and NIP-04 nonces (B03 still open) | S |
| NO-05 | Medium | maintainability | Relay functions of 223-637 lines (`admit_inner`, `Gateway::start`, ...) | L |
| NO-06 | Medium | duplication | NIP-42 client handshakes are hand-written in about 6 crates | L |
| NO-07 | Medium | duplication | `RelaySigner` is the universal signer but misnamed and hex-only; 4 private `sign()` copies | M |
| NO-08 | Medium | security | Encryption APIs make 37 call sites supply their own nonces | M |
| NO-09 | Medium | security | Hand-written table-lookup AES-256-CBC in nip04 is outside the crypto review policy | M |
| NO-10 | Medium | maintainability | `validate_expanded_event` is a 424-line if-chain over magic kind numbers | M |
| NO-11 | Medium | dead-code | About 1.2K lines of test-only evidence prose ship in every client; 45 assertions are vacuous | S |
| NO-12 | Medium | docs | Docs name relay env vars that do not exist (`NOSTR_RELAY_RELAY_*`) | S |
| NO-13 | Medium | dead-code | The legacy nostr-effect import path looks dead but still runs | M |
| NO-14 | Medium | performance | One global Mutex over 11 rate maps; sweeps run under the lock; new keys refused when a map is full | M |
| NO-15 | Medium | security | No cap on authenticated identities per connection; AUTH on an open relay writes owner rows | S |
| NO-16 | Medium | architecture | Two diverging codebases: openagents nostr/nostr-relay vs immortal-core/immortal-relay | S |
| NO-17 | Low | security | `nostr-transport` `connect_as` accepts plaintext `ws://`, duplicates connect code, uses string errors | M |
| NO-18 | Low | dead-code | Several public `nostr` modules have no consumers outside the crate | S |
| NO-19 | Low | duplication | About 14 hand-written hex helpers | S |
| NO-20 | Low | maintainability | Block kinds are defined twice under different names | S |
| NO-21 | Low | docs | Dead `server` feature and stale crate docs | S |
| NO-22 | Low | build | Inconsistent dependency pins; two sha2 versions | S |
| NO-23 | Low | error-handling | Errors are strings across the protocol API | M |
| NO-24 | Low | error-handling | The relay signer `Option` is re-checked with three `expect`s after an earlier guard | S |
| NO-25 | Low | maintainability | `discovery` mixes the docs corpus with a catalog reader; an `unwrap`; kinds compared as strings | S |
| NO-26 | Low | repo-hygiene | Inconsistent test-module layout (`engram_tests.rs` loaded via `#[path]`) | S |

### NO-01 NEG-OPEN (NIP-77) still skips the auth-required and REQ rate-limit checks (prior B05 still open)

**Severity:** High · **Category:** security · **Effort:** S

**Locations:**
- [server.rs:1007-1060](../../../../crates/nostr-relay/src/gateway/server.rs)
- [server.rs:1887-1912](../../../../crates/nostr-relay/src/gateway/server.rs)
- [db.rs:167-169](../../../../crates/nostr-relay/src/gateway/db.rs)

**Evidence:**
- `handle_req` (server.rs:1887) refuses an unauthenticated socket with "auth-required: authenticate before subscribing" when `config.auth_required` is set. It also calls `context.state.rate.req_from_ip`.
- `handle_neg_open` (server.rs:1007) checks only the negentropy subscription count and `validate_and_clamp_filters`. It then calls `db.history(filters, unix_now(), SYNC_LIMIT + 1, cancel, read_pubkeys)`.
- `handle_neg_open` has no `auth_required` check and no `req_from_ip` call. It never reads `HistoryResult.complete` (db.rs:169).
- The 2026-09-26 audit flags this as B05 (README:651).

**Impact:**
- On an auth-required relay, an anonymous socket can list the ids and timestamps of events visible to an empty `read_pubkeys` set.
- Any client can run 4,097-row history queries with no rate limit.
- A truncated result can be reconciled as if it were complete.
- Private events are still filtered through `read_pubkeys`, so what leaks is public-event metadata, and the other cost is unthrottled queries.

**Suggested action:**
1. Move the `handle_req` preamble (the `auth_required` check and `req_from_ip`) into `fn admit_read(context) -> Result<(), &'static str>`.
2. Call `admit_read` from both `handle_req` and `handle_neg_open`. On refusal, send `NEG-ERR`.
3. In `handle_neg_open`, return `NEG-ERR "blocked: incomplete"` when `!stored.complete`.
4. Verify with new `gateway_postgres` cases:
   - an anonymous NEG-OPEN on an `auth_required` relay gets `NEG-ERR`
   - a NEG-OPEN past the REQ rate is refused
   - a truncated result returns `NEG-ERR`

### NO-02 `nostr` is an 85K-LOC catch-all: product contracts live in the "pure protocol" crate that about 60 crates depend on

**Severity:** High · **Category:** architecture · **Effort:** L

**Locations:**
- [lib.rs:1-49](../../../../crates/nostr/src/lib.rs)
- [Cargo.toml:6](../../../../crates/nostr/Cargo.toml)
- [xp.rs](../../../../crates/nostr/src/xp.rs)
- [eval_ext.rs](../../../../crates/nostr/src/eval_ext.rs)
- [market_contracts.rs](../../../../crates/nostr/src/market_contracts.rs)
- [pylon.rs](../../../../crates/nostr/src/pylon.rs)
- [x402.rs](../../../../crates/nostr/src/x402.rs)

**Evidence:**
- The `lib.rs` header says "Pure Nostr protocol and verification primitives".
- The crate declares 37 pub modules, including `xp`, `eval_ext`, `market_contracts`, `pylon` and `x402`.
- 58 Cargo.toml files declare a `nostr` dependency (re-measured). `nostr-relay` links all of it.

**Impact:**
- Any edit to `xp` or the market contracts rebuilds every client and the relay.
- Gamification, payment and eval schemas are versioned together with event signing.

**Suggested action:**
1. Split the crate without changing behavior. Keep in core `nostr`: `domain`, `nip04`/`nip17`/`nip19`/`nip44`, negentropy, `kinds` and the contracts JCS helpers.
2. Move the product families into new crates: `nostr-cj`, `nostr-market`, `nostr-xp` and `nostr-ext`.
3. Keep `pub use` shims in `nostr` for one release, migrate the imports, then delete the shims.
4. Verify with `cargo metadata` that `nostr-relay` no longer depends on the xp or market crates. Then confirm that an edit to `xp.rs` no longer rebuilds `nostr-relay`.

### NO-03 Relay Postgres suites silently pass when the database env var is missing; giant single-function tests; no CI

**Severity:** High · **Category:** testing · **Effort:** M

**Locations:**
- [store_postgres.rs:18-26](../../../../crates/nostr-relay/tests/store_postgres.rs)
- [gateway_postgres.rs:33-40](../../../../crates/nostr-relay/tests/gateway_postgres.rs)
- [interoperability_postgres.rs:27-33](../../../../crates/nostr-relay/tests/interoperability_postgres.rs)
- [scripts/test-postgres.sh](../../../../scripts/test-postgres.sh)

**Evidence:**
- The 13 `*_postgres.rs` files contain only 16 test functions: `gateway_postgres` has 3 tests in 2612 lines, `store_postgres` 1 in 1027 lines, and `push_postgres` 1 in 1190 lines.
- `rg` finds 29 "skipped" lines in `tests/`.
- `store_postgres.rs:18-24` returns early in two cases: when `NOSTR_RELAY_TEST_DATABASE_URL` is unset, and when `NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE != 1`.
- `.github/` contains only `ISSUE_TEMPLATE`.

**Impact:**
- Store and gateway regressions can merge while `cargo test` shows green.
- Because each suite is one large test, the first failure hides every assertion after it.

**Suggested action:**
1. Make a missing database visible. Either mark the Postgres tests `#[ignore = "requires Postgres; run scripts/test-postgres.sh"]` and have `scripts/test-postgres.sh` pass `-- --include-ignored`, or panic when `NOSTR_RELAY_TEST_REQUIRE_DB=1`.
2. Split the `gateway_postgres` contracts and the single `store_postgres` test into one `#[tokio::test]` per clause, using a shared fixture in `tests/common`.
3. Add a CI job that runs `scripts/test-postgres.sh` with a postgres service container.
4. Verify that `cargo test -p nostr-relay` without a DB now reports the tests as ignored rather than passed, and that the CI job runs the full set.

### NO-04 RemoteSigner::cipher encrypts with fixed NIP-44 and NIP-04 nonces (prior B03 still open)

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [remote_sign.rs:564-587](../../../../crates/nostr/src/domain/remote_sign.rs)
- [domain/mod.rs:252](../../../../crates/nostr/src/domain/mod.rs)

**Evidence:**
- remote_sign.rs:579 is `nip44::encrypt(text, &nip44::conversation_key(&self.user, &peer), [4_u8; 32])`.
- remote_sign.rs:584 is `nip04::encrypt(text, &self.user, &peer, [5_u8; 16])`.
- The 2026-09-26 audit flags this as B03 (README:606).
- `RemoteSigner` is re-exported publicly (domain/mod.rs:252), but `rg` finds no use outside `crates/nostr` apart from evidence text in `lane.rs`.

**Impact:** Reusing a nonce under ChaCha20 leaks the XOR of the plaintexts. The defect is latent for now because no product crate uses `RemoteSigner`.

**Suggested action:**
1. Change `cipher` to take a caller-supplied `nonce: [u8; 32]` and `iv: [u8; 16]`, as `seal_response` already does, or call an OS-RNG `encrypt_random` wrapper (see NO-08).
2. Until B03 closes, mark `RemoteSigner` `#[doc(hidden)]` or put it behind a `nip46-experimental` feature.
3. Verify with a test that two `nip44_encrypt` calls on the same input produce different ciphertexts.

### NO-05 Relay god functions: admit_inner is 637 lines; Gateway::start, handle_request and handle_event are 223-375 lines

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [store/mod.rs:409-1045](../../../../crates/nostr-relay/src/store/mod.rs)
- [server.rs:249-623](../../../../crates/nostr-relay/src/gateway/server.rs)
- [server.rs:1233-1455](../../../../crates/nostr-relay/src/gateway/server.rs)
- [db.rs:555-822](../../../../crates/nostr-relay/src/gateway/db.rs)

**Evidence:**
- `admit_inner` starts at store/mod.rs:409, and the next function starts at line 1053.
- `store/mod.rs` has no `#[cfg(test)]` module.
- `Gateway::start` spans server.rs:249-623 and contains 3 `tokio::spawn` calls.

**Impact:**
- The admission policy is the relay's security core, and it can only be tested through Postgres.
- Ordering bugs between checks are easy to introduce.
- A reviewer has to read more than 600 lines to follow it.

**Suggested action:**
1. Split `admit_inner` into these steps:
   - `prevalidate`
   - `build_read_round`
   - a pure `judge_policy(&ReadResults, &Event) -> Option<AdmissionRejection>`
   - a pure `judge_group`
   - `write_admitted`
2. Unit-test the pure judges in the same file with table-driven cases covering every `AdmissionRejection` variant.
3. Move each background loop in `Gateway::start` into its own `spawn_*` function that returns a `JoinHandle`.
4. Verify that the Postgres suites (run per NO-03) still pass unchanged, and that the new unit tests run without a database.

### NO-06 Client-side NIP-42 handshakes hand-rolled in about 6 crates even though nostr-transport is widely used

**Severity:** Medium · **Category:** duplication · **Effort:** L

**Locations:**
- [nostr-transport/src/lib.rs:38-100](../../../../crates/nostr-transport/src/lib.rs)
- [coder/src/relay.rs](../../../../crates/coder/src/relay.rs)
- [gateway/src/advertise.rs](../../../../crates/gateway/src/advertise.rs)
- [gateway/src/relay_worker.rs](../../../../crates/gateway/src/relay_worker.rs)
- [gym-leaderboard/src/relay.rs](../../../../crates/gym-leaderboard/src/relay.rs)
- [openagents-cli/src/relay.rs](../../../../crates/openagents-cli/src/relay.rs)
- [verse-net/src/net/relay.rs](../../../../crates/verse-net/src/net/relay.rs)
- [chat-load-bench/src/relay.rs](../../../../crates/chat-load-bench/src/relay.rs)

**Evidence:**
- `nostr-transport` is used by coder, coder-host, pylon, jev-hosted, openagents-mobile, chat-load-bench and push-gateway (optional), and is imported in about 30 `.rs` files.
- Separate kind-22242 handshake code still exists in:
  - `coder/src/relay.rs`
  - `gateway/src/advertise.rs` and `gateway/src/relay_worker.rs`
  - `gym-leaderboard`
  - `openagents-cli`
  - `verse-net`
  - `chat-load-bench/src/relay.rs`
- Several of these crates already use `nostr-transport` elsewhere.

**Impact:**
- A handshake, TLS-provider or frame-bound bug has to be fixed in several places.
- Bounds and the rustls provider choice differ between copies. `nostr-transport` lib.rs:21-24 documents that the wrong provider choice can panic.

**Suggested action:**
1. Add what the remaining copies need to `nostr-transport`:
   - an opt-in long-lived connection without the 120 s cap
   - handling of AUTH challenges that arrive mid-stream
2. Migrate gateway (`advertise.rs`, `relay_worker.rs`), gym-leaderboard, openagents-cli, verse-net, `chat-load-bench/relay.rs` and `coder/relay.rs` to it.
3. Track progress with `rg -l '22_?242' crates --type rust`, excluding nostr, nostr-relay and nostr-transport. The target is no matches outside those three crates.

### NO-07 RelaySigner is the universal signer but is misnamed, lives in domain/expanded.rs, and accepts only hex; 4 identical private sign() copies

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [expanded.rs:18-50](../../../../crates/nostr/src/domain/expanded.rs)
- [nip17.rs:115-134](../../../../crates/nostr/src/nip17.rs)
- [draft.rs:86](../../../../crates/nostr/src/domain/draft.rs)
- [gift_wrap.rs:74](../../../../crates/nostr/src/domain/gift_wrap.rs)
- [remote_sign.rs:118](../../../../crates/nostr/src/domain/remote_sign.rs)

**Evidence:**
- There are 424 `RelaySigner` references in 111 files outside nostr and nostr-relay.
- 29 call sites use `from_secret_hex(&secret.display_secret().to_string())`.
- Private `sign` functions exist in nip17.rs:115, draft.rs:86, gift_wrap.rs:74 and remote_sign.rs:118. The `remote_sign` copy has no `kind` parameter, so it is not byte-identical to the others.
- The nip17 copy calls both `computed_id_bytes()` and `computed_id()`, so it hashes the event twice.

**Impact:**
- Signing logic is duplicated, and nip17 hashes twice.
- Secret keys are copied into heap `String`s that are never zeroized.

**Suggested action:**
1. Move the signer to `domain/signer.rs` as `Signer`, and keep `pub type RelaySigner = Signer` for one release.
2. Add `Signer::from_secret_key(&SecretKey)`.
3. Compute the event id once by hex-encoding `computed_id_bytes()`.
4. Replace the 4 private `sign` functions with `Signer`, then replace the 29 `display_secret` round-trips with `from_secret_key`.
5. Verify that the existing domain, nip17 and gift-wrap tests pass, and that `rg 'display_secret\(\)\.to_string' crates` finds no matches.

### NO-08 Encryption APIs make every caller supply nonces (37 call sites), the root cause of the B03 bug

**Severity:** Medium · **Category:** security · **Effort:** M

**Locations:**
- [nip44.rs:21-46](../../../../crates/nostr/src/nip44.rs)
- [nip04.rs:71-97](../../../../crates/nostr/src/nip04.rs)
- [remote_sign.rs:579](../../../../crates/nostr/src/domain/remote_sign.rs)

**Evidence:**
- There are 37 `nip44::encrypt(` calls outside `crates/nostr`, and each one sources its nonce itself.
- remote_sign.rs:579 shows what goes wrong: a fixed nonce (NO-04).

**Impact:** Every new caller is another chance to reuse a nonce, and nonce randomness cannot be audited in one place.

**Suggested action:**
1. Add `nip44::encrypt_random` behind a `rand` feature, drawing the nonce from the OS RNG.
2. Rename the raw function to `encrypt_with_nonce` and mark it `#[doc(hidden)]`.
3. Migrate the 37 call sites to `encrypt_random`.
4. Add a `clippy.toml` `disallowed-methods` entry for `encrypt_with_nonce` outside tests.
5. Verify that clippy finds no `encrypt_with_nonce` calls outside tests.

### NO-09 Hand-rolled table-lookup AES-256-CBC in nip04 falls outside the documented crypto review policy

**Severity:** Medium · **Category:** security · **Effort:** M

**Locations:**
- [nip04.rs:173-306](../../../../crates/nostr/src/nip04.rs)
- [docs/nostr/crypto-primitives.md:1-12](../../../../docs/nostr/crypto-primitives.md)

**Evidence:**
- nip04.rs:257-260 indexes `SBOX[word[i] as usize]` with bytes derived from the secret.
- The file has 2 `#[test]` functions.
- `crypto-primitives.md` covers only the four NIP-44 primitives.
- `rg` finds no `nostr::nip04` import outside the crate.

**Impact:** An unreviewed cipher that does not run in constant time ships in a widely linked crate. It becomes a timing and padding risk if NIP-04 decryption is ever exposed as a service.

**Suggested action:**
1. Put `nip04` behind a non-default `nip04` feature, enabled only where it is needed.
2. Add `tests/nip04_differential.rs`, testing against the `aes` and `cbc` dev-dependencies the same way as the nip44 differential test.
3. Record the timing caveat in `crypto-primitives.md`.
4. Verify that `cargo build -p nostr` without the feature does not compile `nip04.rs`, and that the differential test passes with the feature on.

### NO-10 validate_expanded_event is a 424-line if-chain over magic kind numbers, run on every structure validation

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [expanded.rs:437-860](../../../../crates/nostr/src/domain/expanded.rs)
- [event.rs:108-110](../../../../crates/nostr/src/domain/event.rs)

**Evidence:**
- The function runs from expanded.rs:437 to 860.
- `Event::validate_structure` (event.rs:108) calls it at line 110, so it runs on every structure validation.
- Official kinds such as 22_242 appear as literals or as private consts (for example relay `auth.rs:15`).

**Impact:**
- It is hard to see which branch owns which kind, and a duplicate branch is easy to add.
- Every event pays for a long chain of comparisons. This cost is minor; the main cost is maintainability.

**Suggested action:**
1. Add `kinds/official.rs` with named constants for the official kinds.
2. Rewrite the chain as one `match event.kind`, keeping the checks that apply to all kinds before the match.
3. Verify equivalence with the domain tests and `tests/domain_fixtures.rs`.

### NO-11 About 1.2K lines of test-only NIP "evidence" prose compile into every client; SHAPES is empty and about 45 assertions are vacuous

**Severity:** Medium · **Category:** dead-code · **Effort:** S

**Locations:**
- [lane.rs:1-1240](../../../../crates/nostr/src/lane.rs)
- [lane_shapes.inc](../../../../crates/nostr/src/lane_shapes.inc)
- [lib.rs:15](../../../../crates/nostr/src/lib.rs)
- [lib.rs:32](../../../../crates/nostr/src/lib.rs)
- [wire.rs:637-645](../../../../crates/nostr-relay/src/gateway/wire.rs)

**Evidence:**
- `lane.rs` is 1743 lines, and its first `#[cfg(test)]` is at line 1241.
- `lane_shapes.inc` contains only `[]`.
- `rg` finds 45 `shape.file !=` lines. With an empty `SHAPES`, those assertions can never fail.
- The only uses outside the crate are 2 asserts in the relay's `wire.rs` tests (lines 637 and 645).
- `lib.rs` declares `pub mod block_lane` (line 15) and `pub mod lane` (line 32) without any feature gate.

**Impact:** The evidence text adds size to the wasm, mobile and desktop builds, and 45 assertions can never fail.

**Suggested action:**
1. Gate `lane` and `block_lane` behind `#[cfg(any(test, feature = "lane-evidence"))]`, and enable that feature in nostr-relay's dev-dependencies.
2. Delete `lane_shapes.inc`, `Shape` and `SHAPES`, and the `.all(|shape| shape.file != ...)` assertions.
3. Verify that `cargo test -p nostr -p nostr-relay` passes, and that a release build of a client no longer contains the evidence strings (`strings <bin> | rg <distinctive phrase>`).

### NO-12 Docs and "proven" evidence name relay env vars that do not exist (NOSTR_RELAY_RELAY_SECRET_KEY, NOSTR_RELAY_RELAY_URL)

**Severity:** Medium · **Category:** docs · **Effort:** S

**Locations:**
- [lane.rs:903](../../../../crates/nostr/src/lane.rs)
- [nip-expansion.md:38](../../../../docs/protocol/nip-expansion.md)
- [nip-expansion.md:109](../../../../docs/protocol/nip-expansion.md)
- [official-nip-ledger.md:732](../../../../docs/protocol/official-nip-ledger.md)
- [config.rs:177](../../../../crates/nostr-relay/src/gateway/config.rs)
- [config.rs:193](../../../../crates/nostr-relay/src/gateway/config.rs)

**Evidence:** `config.rs` reads `NOSTR_RELAY_URL` (line 177) and `NOSTR_RELAY_SECRET_KEY` (line 193). All 4 doc and evidence locations use the doubled names `NOSTR_RELAY_RELAY_URL` and `NOSTR_RELAY_RELAY_SECRET_KEY`.

**Impact:** An operator who follows the docs sets variables the relay ignores. That silently disables NIP-29 and NIP-86 management.

**Suggested action:**
1. Fix the variable names in the 4 locations.
2. Document the relay env vars that are currently undocumented in `docs/deployment/README.md`.
3. Add a static test that cross-checks the `NOSTR_RELAY_*` literals in `config.rs` against `docs/deployment`.
4. Verify that `rg NOSTR_RELAY_RELAY_` finds no matches.

### NO-13 The legacy nostr-effect import path looks dead but still runs startup and background code

**Severity:** Medium · **Category:** dead-code · **Effort:** M

**Locations:**
- [config.rs:203](../../../../crates/nostr-relay/src/gateway/config.rs)
- [store/mod.rs:256-298](../../../../crates/nostr-relay/src/store/mod.rs)
- [store/mod.rs:386-393](../../../../crates/nostr-relay/src/store/mod.rs)
- [store/mod.rs:1053-1160](../../../../crates/nostr-relay/src/store/mod.rs)

**Evidence:**
- `NOSTR_RELAY_IMPORT_NOSTR_EFFECT` appears only once in the repo, at config.rs:203.
- The import path still carries `AdmissionMode::Legacy` (store/mod.rs:298, used at 393), `LegacyImportReport` (256) and the `import_nostr_effect_*` methods (1053 onward).

**Impact:** Every change to `admit_inner` must preserve this rarely run code, which includes an admission mode with weaker validation.

**Suggested action:**
1. Confirm with the operator that the import ledger is complete.
2. Delete the config flag, the server loops, the store import methods, `AdmissionMode::Legacy` and their tests.
3. Keep the existing migrations. If the ledger tables are unused, add a new migration that drops them.
4. Verify that `rg -i 'nostr_effect|AdmissionMode::Legacy' crates/nostr-relay` finds no matches and that the relay suites pass.

### NO-14 The relay rate limiter is one global std Mutex over 11 HashMaps with under-lock sweeps; it fails closed on attacker-keyed maps, and Verse game logic sits inside it

**Severity:** Medium · **Category:** performance · **Effort:** M

**Locations:**
- [rate.rs:10-45](../../../../crates/nostr-relay/src/gateway/rate.rs)
- [rate.rs:305-360](../../../../crates/nostr-relay/src/gateway/rate.rs)

**Evidence:**
- `RateLimiter { inner: Arc<Mutex<State>> }` guards 11 maps plus the worlds.
- Every 10 s (`CLEANUP_INTERVAL`), `cleanup()` runs `retain` over all of the maps while holding the lock.
- `allow_string_for` refuses any new key once a map holds `MAX_RATE_KEYS = 100_000` entries. The `event_pubkey` map is keyed by pubkeys the client chooses.
- The Verse pose and world presence logic lives in the same `State`.

**Impact:**
- Lock hold times spike during each sweep.
- A flood of fresh pubkeys can make the relay refuse new legitimate pubkeys for up to one rate window. Per-IP event limits bound the flood, so it needs many source IPs, which is feasible across many IPv6 addresses.

**Suggested action:**
1. Give each map its own Mutex, or use hash-sharded state.
2. Sweep incrementally, and when a map is full, evict expired entries before refusing a new key.
3. Move the pose/world logic to `gateway/rate/pose.rs`.
4. Add a unit test that fills a map with expired entries and checks that a new key is admitted.

### NO-15 Unbounded authenticated identities per connection; open-relay AUTH writes owner rows

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [auth.rs:18-55](../../../../crates/nostr-relay/src/gateway/auth.rs)
- [server.rs:1131-1232](../../../../crates/nostr-relay/src/gateway/server.rs)

**Evidence:**
- `AuthState.authenticated` is a `HashMap<String, Option<String>>` with no cap, and `authenticated_pubkeys()` feeds the history query's `read_pubkeys`.
- `handle_auth` calls `event.validate_structure()` at about server.rs:1136, and `auth.verify` calls it again at auth.rs:58.
- The non-closed branch calls `materialize_agent_owner(..., false)` at about server.rs:1214 for any valid NIP-OA attestation.

**Impact:**
- Per-connection memory and the number of SQL parameters grow without limit.
- Self-signed clients can cause a database write for each AUTH.

**Suggested action:**
1. Add `MAX_AUTH_IDENTITIES` (for example 8) to `GatewayLimits`, and refuse AUTH past it.
2. Rate-limit `materialize_agent_owner` per IP.
3. Remove the duplicate `validate_structure` call.
4. Verify with a test that authenticates 9 keys on one socket and expects the 9th to be refused.

### NO-16 Two diverging relay and core codebases: openagents nostr/nostr-relay vs immortal-core/immortal-relay

**Severity:** Medium · **Category:** architecture · **Effort:** S

**Locations:**
- [nostr/src/lib.rs:3](../../../../crates/nostr/src/lib.rs)
- [nostr-relay/src/lib.rs:3-4](../../../../crates/nostr-relay/src/lib.rs)

**Evidence:**
- Both crate headers say they were extracted from `immortal-core`/`immortal-relay`.
- `../immortal/crates` still contains `immortal-core` and `immortal-relay` (last commit `9508af2`, 2026-09-01).
- The workspace CLAUDE.md routes the immortal relay's Rust work to the immortal repo.

**Impact:** Security fixes may need to be applied twice, and it is unclear which relay is canonical.

**Suggested action:**
1. Record which codebase is canonical in `docs/nostr/README.md` and in immortal's README and AGENTS.md.
2. Change both `lib.rs` headers to "forked from immortal at `<commit>`; diverged since".

### NO-17 nostr-transport: authenticated connect accepts plaintext ws:// to any host, duplicates connect code, and has stringly-typed errors with 2 tests

**Severity:** Low · **Category:** security · **Effort:** M

**Locations:**
- [lib.rs:18](../../../../crates/nostr-transport/src/lib.rs)
- [lib.rs:52-100](../../../../crates/nostr-transport/src/lib.rs)
- [lib.rs:119-142](../../../../crates/nostr-transport/src/lib.rs)
- [artifacts.rs:40](../../../../crates/nostr-transport/src/artifacts.rs)

**Evidence:**
- `connect_open` requires `wss://`, but `connect_as` has no scheme check.
- The `WebSocketConfig`/`timeout_at`/connect block is duplicated in the two functions.
- The crate declares `pub type Result<T> = Result<T, String>`.
- lib.rs:89 has `from_secret_hex(&secret.display_secret().to_string())`.
- artifacts.rs:40 uses the literal `3188`, although `nostr::kinds::PRIVATE_ARTIFACT` exists (kinds.rs:71).
- `lib.rs` has 2 tests and `artifacts.rs` has none.

**Impact:**
- A misconfigured URL sends the AUTH event and relay metadata in plaintext. Artifact payloads stay NIP-44 encrypted, so only metadata leaks.
- Callers cannot branch on the error type.

**Suggested action:**
1. Add a shared `open_socket(url, deadline)` used by both connect functions.
2. In `connect_as`, require `wss://` unless the host is loopback.
3. Introduce a `TransportError` enum.
4. Use `kinds::PRIVATE_ARTIFACT` instead of the literal.
5. Add loopback tests for artifact publish and fetch, and a test that a remote `ws://` URL is refused.

### NO-18 No consumers outside the crate for several public nostr modules

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:**
- [agent_persona.rs](../../../../crates/nostr/src/agent_persona.rs)
- [federated_identity.rs](../../../../crates/nostr/src/federated_identity.rs)
- [thread_window.rs](../../../../crates/nostr/src/thread_window.rs)
- [block_lane.rs](../../../../crates/nostr/src/block_lane.rs)

**Evidence:**
- `rg 'nostr::<mod>'` finds no users outside the crate for any of the four modules.
- Sizes: `agent_persona` 324 LOC, `federated_identity` 751, `thread_window` 534, `block_lane` 421.

**Impact:** About 2K LOC of public API with no product use still has to be maintained and compiled.

**Suggested action:**
1. Put these modules behind a `drafts` feature, with their tests running under it. If they are not on the implementation plan, delete them instead.
2. Verify that the workspace builds with the feature off.

### NO-19 About 14 hand-written hex encode/decode helpers across the Nostr crates

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [store/mod.rs:3314-3345](../../../../crates/nostr-relay/src/store/mod.rs)
- [wire.rs:256-295](../../../../crates/nostr-relay/src/gateway/wire.rs)
- [auth.rs:129](../../../../crates/nostr-relay/src/gateway/auth.rs)
- [push/transport.rs:461](../../../../crates/nostr-relay/src/gateway/push/transport.rs)
- [x402.rs:297](../../../../crates/nostr/src/x402.rs)
- [git_sign.rs:370](../../../../crates/nostr/src/git_sign.rs)
- [read_state_snapshot.rs:286](../../../../crates/nostr/src/read_state_snapshot.rs)
- [geocache.rs:456](../../../../crates/nostr/src/domain/geocache.rs)
- [decision.rs:1821](../../../../crates/nostr/src/decision.rs)
- [curated.rs:121](../../../../crates/discovery/src/curated.rs)

**Evidence:** `rg` finds 14 hex function definitions across nostr, nostr-relay and discovery. Some decoders return `Result<_, ()>` and others return typed errors.

**Impact:** Duplicated code with inconsistent error types.

**Suggested action:**
1. Add `crates/nostr/src/hex.rs` with `encode`, `decode_fixed<N>` and `is_lower_hex`.
2. Replace the copies in nostr, nostr-relay and discovery with calls to it.
3. Verify with `rg 'fn (to_)?hex|fn (decode|parse)_hex'` across the three crates. Only `hex.rs` should match.

### NO-20 Block kinds defined twice under different names

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [block.rs:6-13](../../../../crates/nostr/src/domain/block.rs)
- [engram.rs:37](../../../../crates/nostr/src/engram.rs)
- [agent_persona.rs:12](../../../../crates/nostr/src/agent_persona.rs)
- [channel_window.rs:12](../../../../crates/nostr/src/channel_window.rs)

**Evidence:** Three kinds have two names each:

| Kind | Names |
|---|---|
| 30_174 | `AGENT_ENGRAM_KIND`, `ENGRAM_KIND` |
| 30_175 | `AGENT_PERSONA_KIND`, `PERSONA_KIND` |
| 39_005 | `THREAD_SUMMARY_KIND`, `SUMMARY_KIND` |

**Impact:** Changing one of these kinds takes two edits, and the registry tests do not see the duplicates.

**Suggested action:**
1. Keep the definitions in `domain/block.rs` and turn the others into `pub use` aliases.
2. Verify that the `kinds/tests.rs` suite still passes.

### NO-21 Dead `server` feature and stale crate docs

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [nostr/Cargo.toml:14-15](../../../../crates/nostr/Cargo.toml)
- [nostr-relay/src/lib.rs:3-4](../../../../crates/nostr-relay/src/lib.rs)
- [nips/README.md:28](../../../../nips/README.md)

**Evidence:**
- `nostr/Cargo.toml:14-15` declares `server = []`, with a comment saying the relay enables it. Nothing uses `cfg(feature = "server")`, and `nostr-relay/Cargo.toml` does not enable it.
- The `nostr-relay` header says "minus the ... OpenAgents-lane code", yet the relay uses `nostr::lane`.
- `nips/README.md:28` says there are 30 NIPs, but `nips/openagents` has 31 `NIP-*.md` files.

**Impact:** Misleading guidance for contributors and agents.

**Suggested action:**
1. Delete the `server` feature.
2. Rewrite the relay header (see also NO-16).
3. Fix the NIP count, or assert it in the kinds README test so it cannot drift again.

### NO-22 Dependency pin inconsistency and duplicate sha2 versions

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:**
- [discovery/Cargo.toml:16](../../../../crates/discovery/Cargo.toml)
- [nostr/Cargo.toml:28](../../../../crates/nostr/Cargo.toml)
- [nostr-transport/Cargo.toml:9-17](../../../../crates/nostr-transport/Cargo.toml)

**Evidence:** `discovery` pins `sha2 = "0.10"` and `nostr` pins `sha2 = "0.11.0"`. `Cargo.lock` contains both 0.10.9 and 0.11.0.

**Impact:** Binaries that link both crates build sha2 twice.

**Suggested action:**
1. Bump `discovery` to sha2 0.11.
2. Consider `[workspace.dependencies]` for the dependencies these crates share.
3. Check `cargo tree -i sha2@0.10.9`. Other crates may also pull in 0.10, so this change alone may not remove 0.10.9 from the lock.

### NO-23 Stringly-typed errors across the protocol API

**Severity:** Low · **Category:** error-handling · **Effort:** M

**Locations:**
- [nip44.rs:21-90](../../../../crates/nostr/src/nip44.rs)
- [pylon.rs](../../../../crates/nostr/src/pylon.rs)
- [domain/block.rs](../../../../crates/nostr/src/domain/block.rs)
- [nostr-transport/src/lib.rs:18](../../../../crates/nostr-transport/src/lib.rs)

**Evidence:** nip44 `encrypt` and `decrypt` return `Result<_, String>`, and `nostr-transport` uses `Result<T, String>` throughout.

**Impact:** Callers cannot tell a MAC failure from malformed input.

**Suggested action:**
1. Introduce a `Nip44Error` enum, then per-module error enums elsewhere.
2. During the migration, add a temporary `From<_> for String` so existing callers keep compiling.
3. Track progress with `rg 'Result<[^>]*, String>' crates/nostr/src crates/nostr-transport/src`.

### NO-24 Relay signer Option re-checked with three expects after an earlier guard

**Severity:** Low · **Category:** error-handling · **Effort:** S

**Locations:**
- [store/mod.rs:808-811](../../../../crates/nostr-relay/src/store/mod.rs)
- [store/mod.rs:1003](../../../../crates/nostr-relay/src/store/mod.rs)
- [store/mod.rs:1024](../../../../crates/nostr-relay/src/store/mod.rs)
- [store/mod.rs:1030](../../../../crates/nostr-relay/src/store/mod.rs)

**Evidence:** `GroupSigningUnavailable` is returned at about line 811. `relay_signer.expect("group actions require a relay signer")` then appears at lines 1003, 1024 and 1030.

**Impact:** The guard and the `expect`s are about 200 lines apart, so a future reordering could panic the admission task.

**Suggested action:**
1. Bind `group_signer: Option<&RelaySigner>` at the guard and pass it into the write phase, so the three `expect`s disappear.
2. Fold this into the NO-05 split if that happens first.

### NO-25 discovery mixes the docs corpus with a Nostr catalog reader; minor unwraps and string-typed enums

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [curated.rs:170-180](../../../../crates/discovery/src/curated.rs)

**Evidence:**
- Line 172 calls `identity(&item.id)?`, and line 173 then calls `item.id.split_once(':').unwrap().1`, parsing the same id a second time.
- `kind` is compared to the strings `"extension"` and `"service"`.

**Impact:** The id is parsed twice, and the second parse unwraps. If `identity()` changes, that `unwrap` can panic.

**Suggested action:**
1. Have `identity()` return the split parts, and remove the second parse and its `unwrap`.
2. Make `kind` a serde enum.

### NO-26 Inconsistent test-module layout (engram_tests.rs via #[path])

**Severity:** Low · **Category:** repo-hygiene · **Effort:** S

**Locations:**
- [engram.rs:902](../../../../crates/nostr/src/engram.rs)
- [engram_tests.rs](../../../../crates/nostr/src/engram_tests.rs)

**Evidence:** engram.rs:902 loads its tests with `#[path = "engram_tests.rs"]`.

**Impact:** Minor clutter at the crate root.

**Suggested action:** Use `git mv` to move the file to `engram/tests.rs`, and replace the attribute with `mod tests;`.

## Refuted during verification

- **"nostr-transport has only 1 consumer (gym-bridge)."** This is wrong. coder, coder-host, pylon, jev-hosted, openagents-mobile, chat-load-bench and push-gateway (optional) all depend on it, and `nostr_transport` is imported in about 30 `.rs` files. NO-06 states the correct count.
- **"`discovery/curated.rs` hex decoding is buggy because it accepts only lowercase."** This is correct behavior, because Nostr requires lowercase hex. The claim was removed from NO-19.
- **"1_985 `LABEL_KIND` / `CHECK_KIND` is a duplicate kind definition."** The pylon check uses the NIP-32 label kind on purpose. The pair was removed from NO-20.
- **"nip04 has no consumers."** It is used inside the crate. Its risk is covered separately in NO-09 and it was removed from NO-18.
