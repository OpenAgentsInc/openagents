# Payments, identity, tenancy

**Scope:** `crates/{tenancy, pay-ledger, pay-host, x402, wallet, spark-wallet, bitcoin-amount, receipts, xp-ledger, commercial-accounts, commercial-spend, oa-auth, oa-seal, oa-tokens, secret-screen, private-fs, capability, push-gateway}` [PAY]
**Health grade:** C
**Audit date:** 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`

The area is about 122.6k Rust LOC across 240 files and 18 crates, with 780 tests, and it is written with care. Money is integer msat, and SQLite CHECK constraints back that up. Split rules are digest-pinned and immutable, and settlement runs inside IMMEDIATE transactions. Replay keys are consumed exactly once with O_EXCL. Private files are opened with O_NOFOLLOW and checked for owner and mode. Secrets are fingerprinted and never echoed, and OAuth state is compared in constant time. The Bitcoin-only rail policy holds in code: every x402 facilitator path refuses any asset other than BTC, and spark-wallet refuses token and conversion payments. The only policy leftovers are a reserved `Method::Cashu` slot and an upstream "tempo" string in a spec test vector. Stripe card references in tenancy billing are allowed under the owner's 2026-10-09 decision ("card for credits and Pro").

The problems are structural and operational:

1. **Capacity cliffs.** The tenancy file stores have hard caps that will stop live money flows. The billing book refuses provider events after 4,096 and saves after 8,192 grants. The money ledger refuses to append or open past 16 MiB. Nothing compacts any of these stores.
2. **Hand-rolled HTTP parser.** The production pay front uses an HTTP/1.1 parser with no header bounds, one thread per connection, and last-wins Content-Length.
3. **Racy spending cap.** The buyer daily cap is check-then-act across processes.
4. **Inverted layering.** tenancy depends on gym, and commercial-spend depends on coder.
5. **Copied primitives.** Canonicalizers, AEAD sealing, NIP-98 and file custody code exist in several copies.
6. **Opaque errors.** Error types hide why a money-path operation failed.
7. **Thin tests.** Several money-moving modules have few tests.

## Measurements

| Metric | Value |
|---|---|
| Crates / files / LOC | 18 / 240 `.rs` / 122,579 |
| Tests (`#[test]`/`#[tokio::test]`) | 780 |
| `.unwrap()` | 4,213 (mostly in tests) |
| Files > 1,000 lines | 25 |
| Largest files | tenancy `accounts.rs` 2,876; tenancy `billing.rs` 2,625; pay-ledger `shared.rs` 2,408; x402 `native.rs` 2,154; tenancy `money/funding_tests.rs` 2,076; tenancy `money.rs` 1,773 |
| `unsafe` sites vs `SAFETY` comments | 73 vs 35 (x402 `outcome.rs`: 16 unsafe, 0 SAFETY) |
| Error opacity | commercial-spend 215 `Error::Denied` in 2,012 src LOC; pay-ledger 256 `Error::Invalid("...")` (reviewer said 281); tenancy 168 `Result<_, String>` fns and 12 distinct Trouble/Refusal/Error enums |
| `pay_ledger::Ledger` | 24 `impl` blocks in 17 files; 93 `CREATE ` matches; no schema version |
| Duplication | 7 `fn canonicalize` in scope (16 repo-wide); 3 AEAD seal implementations; NIP-98 kind 27235 in 4+ crates; O_NOFOLLOW in 83 non-psionic files |
| TODO/FIXME | 1 |
| Crates with no dependents | pay-host (deployed binary only) |
| Manifest hygiene | x402, wallet, bitcoin-amount outside workspace inheritance; license `"CC-0"` (invalid SPDX); two sha2 versions (0.10.9, 0.11.0) in `Cargo.lock` |

Per crate (LOC / tests / unwrap / commits):

| Crate | LOC | Tests | unwrap | Commits |
|---|---|---|---|---|
| tenancy | 39,023 | 239 | 1,526 | 76 |
| pay-ledger | 17,247 | 123 | 971 | 37 |
| x402 | 13,145 | 76 | 475 | 17 |
| receipts | 9,549 | 80 | 65 | 23 |
| oa-auth | 9,426 | 44 | 186 | 15 |
| push-gateway | 4,730 | 17 | 127 | 3 |
| xp-ledger | 4,589 | 41 | 80 | 14 |
| capability | 4,180 | 36 | 125 | 10 |
| wallet | 3,999 | 16 | 94 | 102 |
| commercial-spend | 3,418 | 7 | 145 | 2 |
| spark-wallet | 3,309 | 35 | 6 | 2 |
| pay-host | 2,692 | 26 | 247 | 4 |
| oa-tokens | 2,699 | 10 | 1 | 4 |
| commercial-accounts | 2,441 | 6 | 127 | 4 |
| secret-screen | 680 | 7 | - | - |
| private-fs | 545 | 3 | - | - |
| bitcoin-amount | 483 | 9 | - | - |
| oa-seal | 424 | 5 | - | - |

## Strengths

- Money is integer msat everywhere. The pay-ledger schema enforces invariants in SQL: `CHECK(received_msat <= price_msat)` and `CHECK(lsp_fee_msat = price_msat - received_msat)` ([schema.sql:14-17](../../../../crates/pay-ledger/src/schema.sql)). Settlement validates inputs before any arithmetic ([lib.rs:822-824](../../../../crates/pay-ledger/src/lib.rs)).
- Split rules are versioned, digest-pinned and immutable (`Ledger::load_rule`, [lib.rs:401-430](../../../../crates/pay-ledger/src/lib.rs)). Settlement and payout state changes run in IMMEDIATE transactions.
- The x402 replay store gives cross-process exactly-once consumption through `File::create_new` / O_EXCL ([replay.rs:71-86](../../../../crates/x402/src/replay.rs)) and deliberately stores no preimage or invoice.
- The Bitcoin-only policy is enforced in code. The facilitator refuses any asset other than BTC ([facilitator.rs:56](../../../../crates/x402/src/facilitator.rs)). spark-wallet refuses token and conversion payments ([spark.rs:260, 820](../../../../crates/spark-wallet/src/spark.rs)). The router docs state the EVM/Solana/Tempo exclusion ([router.rs:31-35](../../../../crates/x402/src/router.rs)).
- Auth hygiene is good. `return_to` rejects open redirects, including encoded slash and backslash forms ([flow.rs](../../../../crates/oa-auth/src/flow.rs)). OAuth state is compared with `subtle::ConstantTimeEq` (flow.rs:154). BYOK header errors never echo key material ([front.rs:560-575](../../../../crates/x402/src/front.rs)).
- oa-seal is a well-designed rotating keyring: named keys, AES-256-GCM with associated data, and zeroize on drop. It should become the single sealing primitive.
- bitcoin-amount is a clean amount formatter with no dependencies and no floats, and it forbids unsafe code.
- Tenancy's Postgres backend applies embedded, ordered migrations under an advisory lock and records them in `schema_migrations` ([db.rs:41-55](../../../../crates/tenancy/src/db.rs)).
- Nearly every file has module-level docs that explain its intent, invariants and non-goals.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| PAY-01 | high | performance | Billing book and money ledger hit hard size caps with no compaction; live Stripe and funding processing will stop | L |
| PAY-02 | high | security | Production pay front runs on a hand-rolled HTTP/1.1 parser with no header limits and a thread per connection | M |
| PAY-03 | high | error-handling | File-lock stores never reclaim stale locks; a crashed writer blocks all writes permanently | S |
| PAY-04 | medium | concurrency | Buyer daily spending cap is check-then-act and can be overspent by concurrent purchases | M |
| PAY-05 | medium | architecture | Payment and identity crates depend on product god-crates (coder, gym) | M |
| PAY-06 | medium | architecture | tenancy is a 39k-LOC god crate | XL |
| PAY-07 | medium | duplication | Canonical-JSON digest function copied 7 times in scope (16 repo-wide) | S |
| PAY-08 | medium | duplication | Owner-private file custody code duplicated instead of using private-fs | M |
| PAY-09 | medium | security | Three AEAD sealing implementations; GitHub tokens sealed with a key that cannot be rotated | M |
| PAY-10 | medium | error-handling | Opaque error types hide the cause of money-path failures | M |
| PAY-11 | medium | testing | Money-moving controllers and the wallet RPC are thinly tested | M |
| PAY-12 | medium | architecture | Two parallel Lightning wallet stacks with separate seeds and storage workarounds | L |
| PAY-13 | medium | architecture | x402 mixes the payment protocol with a generic gateway and BYOK key handling | L |
| PAY-14 | medium | duplication | NIP-98 HTTP auth implemented separately in several crates | S |
| PAY-15 | medium | security | Postgres hard-codes NoTls; per-thread transaction model is fragile | M |
| PAY-16 | low | security | Wallet seed creation is not durable; resident socket briefly has umask permissions | S |
| PAY-17 | low | security | x402 replay store lacks a directory fsync and uses default permissions | S |
| PAY-18 | low | architecture | pay-host queries pay-ledger's SQLite schema with raw SQL | M |
| PAY-19 | low | build | pay-host enables serde_json `arbitrary_precision`, which unifies into any build that links it | S |
| PAY-20 | low | performance | pay-host serializes SSE subscribers through a std Mutex and polls SQLite every 250 ms | M |
| PAY-21 | low | policy | Reserved Cashu payment method remains after Cashu was ruled out | S |
| PAY-22 | low | build | Manifests bypass workspace inheritance and lints; invalid license; two sha2 versions | S |
| PAY-23 | low | maintainability | pay-ledger schema spread across many CREATE statements with ad hoc migrations | M |
| PAY-24 | low | security | secret-screen has no rules for payment credentials (Stripe, BIP39, xprv) | S |
| PAY-25 | low | maintainability | receipts mixes funnel, pilot and policy modules with receipt contracts; capability and oa-tokens are misplaced | M |

### PAY-01 Billing book and money ledger hit hard size caps with no compaction; live Stripe and funding processing will stop

**Severity:** high · **Category:** performance · **Effort:** L

**Locations:**
- [billing.rs:47-54](../../../../crates/tenancy/src/billing.rs), billing.rs:973-979, billing.rs:1872-1881, billing.rs:2153-2182
- [money.rs:22](../../../../crates/tenancy/src/money.rs), money.rs:943-951, money.rs:1050
- [gateway billing.rs:114](../../../../crates/gateway/src/billing.rs), [gateway serve.rs:289](../../../../crates/gateway/src/serve.rs)
- [db/docs.rs:484-561](../../../../crates/tenancy/src/db/docs.rs)

**Evidence:**
- **Caps.** `EVENTS_MAX=4096` and `GRANTS_MAX=8192` are set at billing.rs:53-54. `receive()` returns `Refusal::EventsBounded` once `events.len() >= EVENTS_MAX` (973-974). Grants are inserted without a check (941, 1044, 1630), but validation refuses more than `GRANTS_MAX` (1877).
- **Full rewrites.** `save()` pretty-prints the whole store (`to_string_pretty`, 2160) and refuses past 16 MiB. On every new revision it also writes a full archived copy to `billing-history/<digest>.json` (2164-2170).
- **Money log.** `MAX_LOG` is 16 MiB (money.rs:22). The ledger refuses to open past it (950) and refuses appends past it (1050).
- **Deployment.** The gateway opens one global ledger (serve.rs:289) and one billing directory (billing.rs:114). Postgres layouts exist only for ACCOUNTS, SESSIONS and KEYS (docs.rs:484, 522, 560).
- No compaction or rotation code exists in billing.rs or money.rs.

**Impact:**
- After about 4k provider events, every Stripe webhook is refused.
- After 8k grants, saves fail.
- After 16 MiB of money log, funding mutations fail and the ledger will not reopen.
- Archive disk use grows with history size times revision count.

**Suggested action:**
1. Move billing and money into the tenancy Postgres layer as row tables (events, grants, invoices, subscriptions, money entries), as `db/docs.rs` already does for accounts, sessions and keys. Delete the whole-document rewrite.
2. Until that lands:
   - Archive concluded events older than N days into a sealed period file, and keep only an id set for dedup.
   - Make grants an append-only log with a checkpointed digest.
   - Have `money::Ledger` checkpoint its state plus the head digest and start a new segment instead of refusing.
3. Add a test that inserts 5,000 events and 9,000 grants and asserts that `receive` and `save` succeed.
4. Add a gateway metric and alert at 80% of any cap that remains.

### PAY-02 Production pay front runs on a hand-rolled HTTP/1.1 parser with no header limits and a thread per connection

**Severity:** high · **Category:** security · **Effort:** M

**Locations:**
- [server.rs:326-366](../../../../crates/x402/src/server.rs), server.rs:405-440
- [openagents-cli pay.rs:937](../../../../crates/openagents-cli/src/pay.rs)
- [openagents-pay-front.service:22](../../../../deploy/systemd/openagents-pay-front.service)

**Evidence:**
- **No header limits.** `read_request` calls `reader.read_line` with no per-line cap, no total cap and no limit on header count. Only the body is capped (`MAX_BODY` 8 MiB).
- **Last-wins Content-Length.** A duplicate `content-length` header silently overwrites `length`, so the last one wins.
- **Unbounded threads.** `serve_with` calls `std::thread::spawn` for every accepted connection with no bound. It sets a 30 s per-read timeout and busy-polls `accept` with a 50 ms sleep.
- **It is the production path.** `pay.rs:937` calls `openagents_x402::server::serve_with`, and the systemd unit runs `ExecStart=openagents --json pay serve --routes ...`.
- `crates/x402/tests/front.rs` (8 tests) exercises the front end to end, but no test covers the parser's limits.

**Impact:**
- A remote client can exhaust memory (endless header lines) or threads on the live payment front.
- If a fronting proxy honors the first Content-Length, this opens a request-smuggling path.

**Suggested action:**
1. Replace `serve_with` with hyper/axum on a bounded tokio runtime, keeping the `Fn(&Request) -> Response` adapter. Both are already in the tree via pay-host and oa-auth.
2. If the hand-rolled loop must stay:
   - Use `reader.take()` to cap the request line and each header at 8 KiB, and total headers at 64 KiB or 100 lines.
   - Reject duplicate or conflicting Content-Length with 400.
   - Bound concurrent connections with a counter or semaphore.
   - Enforce a total-request deadline.
3. Verify with parser unit tests in `server.rs` for an oversize header, a duplicate Content-Length and a slow client.

### PAY-03 File-lock stores never reclaim stale locks; a crashed writer blocks all writes permanently

**Severity:** high · **Category:** error-handling · **Effort:** S

**Locations:**
- [accounts.rs:868-903](../../../../crates/tenancy/src/accounts.rs)
- [sessions.rs:1214-1240](../../../../crates/tenancy/src/sessions.rs)
- [keys.rs:347-395](../../../../crates/tenancy/src/keys.rs)
- [skills.rs:1256-1280](../../../../crates/tenancy/src/skills.rs)
- [quota.rs:327-350](../../../../crates/tenancy/src/quota.rs)
- [billing.rs:2035-2070](../../../../crates/tenancy/src/billing.rs) (the correct pattern, for reference)

**Evidence:**
- **The broken pattern.** accounts, sessions, keys, skills and quota take their lock with `OpenOptions::create_new` on a lock file and write `pid {}` into it (accounts.rs:890, sessions.rs:1232, skills.rs:1265, quota.rs:335). On `AlreadyExists` they sleep 10 ms and retry, and finally return `Locked`.
- **No recovery.** The lock file is removed only in `Drop`. The pid is never read back, and nothing handles a stale lock.
- **The correct pattern already exists.** `BillingLock::acquire` opens a persistent 0600 O_NOFOLLOW file and calls `file.try_lock()`, which the OS releases when the process dies. money.rs:943 does the same.

**Impact:**
- A SIGKILL, OOM or power loss during a write leaves skills and quota unwritable until an operator deletes the lock file.
- The same holds for accounts, sessions and keys on file-backed deployments. In production those three go through the Postgres `Held` path instead (accounts.rs:870-877).

**Suggested action:**
1. Port the BillingLock pattern (persistent private lock file, `File::try_lock`, `same_file` recheck) to accounts, sessions, keys, skills and quota, ideally as one shared `tenancy::store` lock helper.
2. Add a test that spawns a child process, takes the lock, kills the child with SIGKILL, and asserts that the parent acquires the lock within the retry budget.

### PAY-04 Buyer daily spending cap is check-then-act and can be overspent by concurrent purchases

**Severity:** medium · **Category:** concurrency · **Effort:** M

**Locations:**
- [policy.rs:193-223](../../../../crates/x402/src/policy.rs), policy.rs:254-330, policy.rs:461-470
- [openagents-cli x402.rs:813-825](../../../../crates/openagents-cli/src/x402.rs)

**Evidence:**
- **The race.** `admit()` in openagents-cli `x402.rs` calls `open_ledger().spent_since(now - DAY_SECS)` without a lock, then calls the pure `Policy::admit(.., spent)`. The payment follows, and only then do `append` or `record_once` take `mutation_lock`. Two concurrent purchases can read the same `spent` value, and both pass `would_be <= cap`.
- **Inconsistent file options.** `append()` (policy.rs:289-305) opens the ledger with plain create+append, without `mode(0o600)` or O_NOFOLLOW. `record_once` uses both.

**Impact:** Under parallel agents, the user-set daily cap is not a hard limit. The overshoot is bounded by concurrency × the per-call `max_msat` ceiling, which still holds.

**Suggested action:**
1. Add `Ledger::reserve(policy, limits, provider, amount_msat, fee_cap_msat, payment_hash)`. Under `mutation_lock`, it recomputes the 24h spend including pending reservations, checks the cap, and appends a `phase: "reserved"` entry, all as one atomic step.
2. Finalize or release the reservation after the payment.
3. Switch the `x402.rs` and `plugin_purchase.rs` callers to `reserve`.
4. Give `append()` the same 0600/O_NOFOLLOW options as `record_once`.
5. Add a test with two threads that each pay 60% of the cap, and assert that exactly one is admitted.

### PAY-05 Payment and identity crates depend on product god-crates (coder, gym)

**Severity:** medium · **Category:** architecture · **Effort:** M

**Locations:**
- [commercial-spend Cargo.toml:11](../../../../crates/commercial-spend/Cargo.toml)
- [commercial-spend wallet.rs:15](../../../../crates/commercial-spend/src/wallet.rs), wallet.rs:99
- [commercial-accounts Cargo.toml:11-25](../../../../crates/commercial-accounts/Cargo.toml)
- [tenancy Cargo.toml:12](../../../../crates/tenancy/Cargo.toml)
- [tenancy admission.rs](../../../../crates/tenancy/src/admission.rs)
- [gym Cargo.toml:28](../../../../crates/gym/Cargo.toml)

**Evidence:**
- commercial-spend depends on coder unconditionally (`default-features=false`) to use `coder::customer::plugins::Offer` and `coder::customer::Store::shared_plugin_approval` (wallet.rs:15, 99).
- commercial-accounts pulls in coder and plugin under `merchant-commissions`.
- tenancy depends on gym, and gym enables serde_json `preserve_order` (gym Cargo.toml:28).
- coder and gym together are about 373k tracked `.rs` lines.

**Impact:**
- Changes in coder or gym recompile the money controller and the identity layer, and can break them.
- gym's `preserve_order` setting leaks into tenancy's digest code, which the canonicalizers then have to defend against.

**Suggested action:**
1. Move the `Offer`/`CommissionSource` types that payments need into a small leaf crate (or `receipts::purchase`). Have coder convert into them, and drop the coder dependency from commercial-spend and commercial-accounts.
2. Move tenancy::admission's gym bridge into gym or a `tenancy-admission` crate, so tenancy core has no gym dependency.
3. Verify that `cargo tree -p tenancy | grep gym` and `cargo tree -p commercial-spend | grep -w coder` both return nothing.

### PAY-06 tenancy is a 39k-LOC god crate spanning registry, identity, billing, money, skills moderation and training

**Severity:** medium · **Category:** architecture · **Effort:** XL

**Locations:**
- [tenancy Cargo.toml:6](../../../../crates/tenancy/Cargo.toml)
- [tenancy src/bin](../../../../crates/tenancy/src/bin)
- Lock implementations: [accounts.rs:868](../../../../crates/tenancy/src/accounts.rs), [billing.rs:2035](../../../../crates/tenancy/src/billing.rs), [keys.rs:347](../../../../crates/tenancy/src/keys.rs), [quota.rs:327](../../../../crates/tenancy/src/quota.rs), [sessions.rs:1214](../../../../crates/tenancy/src/sessions.rs), [skills.rs:1256](../../../../crates/tenancy/src/skills.rs)

**Evidence:**
- The crate description reads "The tenant-to-artifact registry...", but the crate holds accounts, billing, money, sessions, skills, training, quota and more.
- It ships 7 bins: skills-moderate, tenant-db, tenant-keys, tenant-money, tenant-referrals, tenant-train and tenant-usage.
- There are six separate `fn acquire` lock implementations (accounts, billing, keys, quota, sessions, skills), and they have already diverged: billing uses `try_lock`, while the others use `create_new` lock files (see PAY-03).

**Impact:**
- Change coupling is high and compiles are long.
- Durability fixes land in one store but not the others, as the lock divergence already shows.

**Suggested action:**
1. Extract one `tenancy::store` module that owns lock acquire/check (OS `try_lock`), the size bound, the history archive and the canonical digest.
2. Port the six stores to it. Verify that one `fn acquire` remains.
3. Split the crate along module seams:
   - tenancy-core
   - identity (accounts, workspaces, sessions, keys, sso)
   - billing (billing, prepaid, quota, money)
   - skills-directory
   - tenant-training

### PAY-07 Canonical-JSON digest function copied 7 times in scope (16 repo-wide) beside an existing JCS implementation

**Severity:** medium · **Category:** duplication · **Effort:** S

**Locations:**
- tenancy: [accounts.rs:1945](../../../../crates/tenancy/src/accounts.rs), [backend.rs:576](../../../../crates/tenancy/src/backend.rs), [billing.rs:2190](../../../../crates/tenancy/src/billing.rs), [manifest.rs:315](../../../../crates/tenancy/src/manifest.rs), [sessions.rs:1353](../../../../crates/tenancy/src/sessions.rs)
- receipts: [execution.rs:468](../../../../crates/receipts/src/execution.rs), [export.rs:983](../../../../crates/receipts/src/export.rs)
- x402: [native.rs:18](../../../../crates/x402/src/native.rs)

**Evidence:**
- Outside psionic, `git grep 'fn canonicalize('` returns 16 hits, 7 of them in this scope.
- The doc comment in export.rs says "the same canonicalization every digest in this workspace shares", but the sharing is done by copying.
- x402 uses `nostr::contracts::jcs` instead (native.rs:18, 167, 810).

**Impact:** Digests only agree across stores and receipts as long as 16 copies stay identical.

**Suggested action:**
1. Create one canonicalizer, either a tiny `canonical-json` leaf crate or an export from `nostr::contracts`, that reproduces the current copies' bytes exactly.
2. Build golden vectors from current store fixtures. Do not switch existing stores to RFC 8785 JCS until those vectors prove byte-identity, because JCS number and escape rules may differ.
3. Replace the 7 in-scope copies, then the other 9. Verify with `git grep 'fn canonicalize('`, which should return one definition.

### PAY-08 Owner-private file custody code duplicated across payment crates instead of using private-fs

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [commercial-spend private.rs:11-60](../../../../crates/commercial-spend/src/private.rs)
- [commercial-accounts lib.rs:80-110](../../../../crates/commercial-accounts/src/lib.rs)
- [tenancy private_fs.rs](../../../../crates/tenancy/src/private_fs.rs)
- [private-fs lib.rs:1-20](../../../../crates/private-fs/src/lib.rs)
- [x402 policy.rs:254-280](../../../../crates/x402/src/policy.rs)

**Evidence:**
- commercial-spend `private.rs` has `Held{path,directory,file,..}`, `fn private(m, directory)` (uid==geteuid, mode&0o077==0, nlink==1) and `fn same(a,b)` (dev/ino).
- commercial-accounts lib.rs:80-98 has nearly the same code, line for line. One copy carries a SAFETY comment and the other does not.
- tenancy has its own 130-line `private_fs` module, while the `private-fs` crate's docs describe only the Windows equivalents.
- x402 `policy.rs` repeats the O_NOFOLLOW+flock pattern inline.

**Impact:**
- Custody-hardening fixes have to be applied by hand in several places.
- Some copies make unsafe libc calls without SAFETY comments.

**Suggested action:**
1. Grow `crates/private-fs` into the cross-platform custody crate, and move `tenancy/src/private_fs.rs` (the Unix code) into it.
2. Add `Held::open(path, immutable)`, `is_private(meta, dir)`, `same_file`, `create_private_dir` and `open_nofollow`.
3. Replace the copies in commercial-spend, commercial-accounts, wallet, x402 and tenancy.
4. Verify with `git grep -c geteuid -- crates/commercial-* crates/wallet crates/x402 crates/tenancy`, which should drop to the private-fs uses only.

### PAY-09 Three separate AEAD sealing implementations; GitHub tokens sealed with a key that cannot be rotated

**Severity:** medium · **Category:** security · **Effort:** M

**Locations:**
- [oa-auth repos.rs:38](../../../../crates/oa-auth/src/repos.rs), repos.rs:945-973
- [oa-auth config.rs:142-199](../../../../crates/oa-auth/src/config.rs)
- [push-gateway server/store.rs:14](../../../../crates/push-gateway/src/server/store.rs), store.rs:112-150

**Evidence:**
- oa-auth `repos.rs` builds `Aes256Gcm` from `credentials.token_key()`, a single `[u8;32]`, and emits `v1.<b64>` (repos.rs:973). The output records no key id.
- push-gateway seals with ring `LessSafeKey` AES_256_GCM (store.rs:14, 147).
- Only gateway and openagents-web depend on oa-seal.

**Impact:**
- The `token_encryption_key` that protects GitHub repo-scope tokens cannot be rotated without losing every stored token.
- Each store needs its own compromise response.

**Suggested action:**
1. Migrate oa-auth `repos.rs` (and the `repos/installed.rs` consumers) to `oa_seal::Keyring::{seal, open, is_current}`, using the existing associated-data string as the AAD.
2. Accept legacy `v1.` values on read and reseal them under the keyring.
3. Make the same change in push-gateway `store.rs`.
4. Add a rotation test: seal under k1, add k2 as current, open the value, and assert that it is resealed.

### PAY-10 Opaque error types hide the cause of money-path failures

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [commercial-spend lib.rs:28-49](../../../../crates/commercial-spend/src/lib.rs)
- [pay-ledger lib.rs](../../../../crates/pay-ledger/src/lib.rs)
- [pay-host lib.rs:600-602](../../../../crates/pay-host/src/lib.rs)

**Evidence:**
- **commercial-spend.** Its src has 215 `Error::Denied` occurrences, all rendering one fixed message. The `Io`, `Ledger`, `Json`, `Accounts` and `Wallet` variants have fixed `#[error]` strings that drop the source. Debug is derived, so `{:?}` still shows it.
- **pay-ledger.** Its src has 256 `Error::Invalid("...")` sites.
- **pay-host.** `fn internal(_: impl Display) -> StatusCode` discards the error before returning 500.

**Impact:**
- Logs that format errors with Display cannot show why a purchase, settlement or payout was refused.
- Callers cannot branch on the cause.

**Suggested action:**
1. In commercial-spend, change `Denied` to `Denied(DenyReason)`, with reasons such as `NotAbsolute`, `NotPrivate`, `CustodyChanged`, `InvoiceMalformed` and `ConversionMismatch`. Keep the coarse user-facing text.
2. Add `: {0}` (or `#[source]`) to commercial-spend's wrapping variants.
3. In pay-host, log the error inside `internal()` with `tracing::error!` or `eprintln!`.
4. In pay-ledger, split `Invalid` messages that name several fields into single-field messages.
5. Verify with `git grep -c 'Error::Denied\b'`, which should return 0 bare uses.

### PAY-11 Money-moving controllers and the wallet RPC are thinly tested

**Severity:** medium · **Category:** testing · **Effort:** M

**Locations:**
- [crates/commercial-spend](../../../../crates/commercial-spend)
- [crates/commercial-accounts](../../../../crates/commercial-accounts)
- [wallet resident.rs](../../../../crates/wallet/src/resident.rs)
- [wallet ldk.rs](../../../../crates/wallet/src/ldk.rs)
- [x402 facilitator.rs](../../../../crates/x402/src/facilitator.rs)

**Evidence:** Test attribute counts:

| Module | Tests | Size |
|---|---|---|
| commercial-spend | 7 | |
| commercial-accounts | 6 | |
| wallet `resident.rs` | 3 | 1,141 lines |
| wallet `ldk.rs` | 4 | |
| x402 `facilitator.rs` | 2 | 471 lines |
| x402 `server.rs` | 1 | |

x402 also has integration tests: `tests/front.rs` (8) and `tests/router.rs` (5).

**Impact:** Funding, refunds and invoice payment have the least regression protection.

**Suggested action:**
1. commercial-spend: test refund/reversal plans, protocol framing bounds, and wallet intent construction (conversion mismatch, foreign invoice).
2. wallet `resident.rs`: test framing, the financial mutex serializing concurrent Pay calls, and stale-socket takeover in `Server::bind`.
3. x402 `facilitator.rs`: add one test per `errorReason` (expired, hash mismatch, amount mismatch, non-BTC asset, network mismatch), using nostr's `test-invoice` feature.

### PAY-12 Two parallel Lightning wallet stacks with separate seeds, traits and storage workarounds

**Severity:** medium · **Category:** architecture · **Effort:** L

**Locations:**
- [spark-wallet Cargo.toml:10-12](../../../../crates/spark-wallet/Cargo.toml)
- [spark-wallet store.rs](../../../../crates/spark-wallet/src/store.rs)
- [openagents-pay-payouts.service:23-28](../../../../deploy/systemd/openagents-pay-payouts.service)
- [openagents-pay-front.service:21](../../../../deploy/systemd/openagents-pay-front.service)

**Evidence:**
- spark-wallet `store.rs` is 1,162 lines.
- The Cargo.toml comment says ldk-node's libsqlite3-sys 0.28 conflicts with the 0.30 that Breez needs.
- The payouts service runs `pay payouts --spark-home` with a separate Spark seed (`openagents-pay-seed fetch spark`), while the front requires the LDK wallet's `control.sock`.

**Impact:**
- There are two seeds and two backup procedures.
- The hand-maintained storage shim has to track Breez's schema on every SDK bump.

**Suggested action:**
1. Write a decision doc on whether LDK stays as the receiver.
2. If both stacks stay, put them behind one `Wallet` trait in a leaf crate.
3. File an issue to resolve the libsqlite3-sys conflict (upgrade the MDK ldk-node fork, or align rusqlite) so that `store.rs` can be deleted.

### PAY-13 x402 crate mixes payment protocol with a generic gateway and BYOK model-key handling

**Severity:** medium · **Category:** architecture · **Effort:** L

**Locations:**
- [x402 Cargo.toml:7-15](../../../../crates/x402/Cargo.toml)
- [router.rs:25-41](../../../../crates/x402/src/router.rs)
- [front.rs](../../../../crates/x402/src/front.rs)

**Evidence:**
- The crate description reads "x402 exact Lightning over HTTP...: the embedded facilitator, its replay store, and the header codecs".
- Yet it depends on model-access, used only for BYOK (`# BYOK: an API caller's own provider keys (OpenAgents-Provider-Key, #10176)`), and `front`, `hosted` and `native` live here too.

**Impact:** A security review of the payment toll also has to cover provider-key handling and gateway routing.

**Suggested action:**
1. Limit `openagents-x402` to wire, facilitator, replay, payment_scheme, router and receipt.
2. Move `front`, `hosted` and BYOK into a `pay-front` crate. Its binary home is openagents-cli `pay serve`.
3. Move `native.rs` out of x402.
4. Verify that `model-access` no longer appears in `x402/Cargo.toml`.

### PAY-14 NIP-98 HTTP auth implemented separately in several crates instead of in nostr

**Severity:** medium · **Category:** duplication · **Effort:** S

**Locations:**
- [push-gateway nip98.rs:12-16](../../../../crates/push-gateway/src/nip98.rs)
- [nostr-relay gateway/push/transport.rs:32](../../../../crates/nostr-relay/src/gateway/push/transport.rs)
- [ext-eval blob.rs:15](../../../../crates/ext-eval/src/blob.rs)
- [verse-private auth.rs:13](../../../../crates/verse-private/src/auth.rs)

**Evidence:**
- `HTTP_AUTH_KIND = 27_235` is defined separately in push-gateway, the nostr-relay push transport and ext-eval.
- verse-private defines its own NIP-98 kind and header builder.
- push-gateway's 179-line `nip98.rs` holds the method/path/body-hash policy and `WINDOW_SECONDS=60`.
- `crates/nostr` has no nip98 module, only a NIP-96 storage flag.

**Impact:**
- Signer and verifier rules (time window, URL normalization, body hash) can drift apart.
- It also goes against the workspace rule to extend the shared Nostr implementation instead of rebuilding it.

**Suggested action:**
1. Move push-gateway `nip98.rs` into `crates/nostr` as `nostr::nip98::{sign, verify, Verified, WINDOW_SECONDS}`, with a parameter that selects path-only or full-URL matching.
2. Point push-gateway, the nostr-relay push transport, ext-eval and verse-private at it.
3. Keep push-gateway's tests as the conformance tests.
4. Verify that `git grep 27_235` returns only `crates/nostr`.

### PAY-15 Postgres connection hard-codes NoTls, and the per-thread transaction model is fragile

**Severity:** medium · **Category:** security · **Effort:** M

**Locations:**
- [db.rs:111-123](../../../../crates/tenancy/src/db.rs), db.rs:206, db.rs:520-556

**Evidence:**
- **No TLS.** `config.connect(NoTls)` is unconditional (206), even though the doc comment accepts `postgres://` URLs (111-112).
- **Thread-local transactions.** `HELD` is a `thread_local` `Vec<(String, Tx)>` (520). `Held` holds only a `String`, so it is `Send`. Its `Drop` calls `take_held(&self.name)` on whatever thread it runs on.
- **Swallowed commit errors.** `let _ = tx.commit();` discards commit errors (550-553).

**Impact:**
- A remote DSN sends account and session data in cleartext.
- If a `Held` is dropped on another thread, the transaction and its advisory lock stay in the original thread's TLS. The store lock then stays held until that thread ends.
- A failed commit loses a save that already reported success.

**Suggested action:**
1. Reject DSNs that are neither unix-socket hosts nor loopback unless TLS is configured. Add tokio-postgres-rustls for the remote case.
2. Make `Held` `!Send` with `PhantomData<*const ()>`.
3. When the commit in `Drop` fails, log it and poison or mark the store.
4. Longer term, pass `&mut Tx` explicitly instead of looking it up thread-locally.
5. Add tests: a non-loopback DSN without TLS is refused, and a compile-fail test shows `Held` is `!Send`.

### PAY-16 Wallet seed creation is not durable, and the resident socket is briefly umask-permissioned

**Severity:** low · **Category:** security · **Effort:** S

**Locations:**
- [wallet config.rs:319-359](../../../../crates/wallet/src/config.rs)
- [wallet ldk.rs:91](../../../../crates/wallet/src/ldk.rs)
- [wallet resident.rs:208-225](../../../../crates/wallet/src/resident.rs)

**Evidence:**
- **Directory mode.** `load_or_create_seed` uses `create_dir_all` with the default mode.
- **Seed write.** `write_private` does `create_new` + 0600 + `write_all`. There is no `sync_all`, no temp-and-rename and no directory fsync.
- **Seed load.** It returns `text.trim()` with no mode or owner check. An empty or partial seed is not silently accepted, because ldk.rs:91 parses it with `Mnemonic::from_str` and fails.
- **Socket.** `Server::bind` binds `control.sock` and only then chmods it to 0600 (221-224). `create_dir_all(home)` does not force 0700.

**Impact:**
- A power loss right after `x402 node init` can lose an unsynced seed while the node state persists.
- The spend-capable socket has umask permissions for a brief window.

**Suggested action:**
1. Write the seed via `seed.tmp` (0600, O_NOFOLLOW), call `sync_all`, rename it into place, and fsync the directory.
2. Create the home directory with mode 0700.
3. On load, check the uid and that `mode & 0o077 == 0`.
4. Bind the socket inside a 0700 directory or under a restrictive umask.
5. Add a test that an empty seed file is rejected at load.

### PAY-17 x402 replay store lacks a directory fsync and creates entries with default permissions

**Severity:** low · **Category:** security · **Effort:** S

**Locations:**
- [replay.rs:52-90](../../../../crates/x402/src/replay.rs), replay.rs:104-121
- [nostr x402.rs:156](../../../../crates/nostr/src/x402.rs)

**Evidence:**
- **No directory fsync.** `insert` does `File::create_new` + `write_all` + `sync_all`, but never fsyncs the directory.
- **Default permissions.** `open` uses `create_dir_all` with the default mode, and entry files are created with the umask mode.
- **Zero-byte entries.** On a write error the file is removed. A crash between create and write, however, leaves a 0-byte entry that `get` fails to parse and `sweep` skips forever. This fails closed: the entry still blocks reuse.
- **Keys are safe today.** Keys are built as `{network}:{payment_hash}` (nostr/x402.rs:156), so path traversal is not reachable from the wire.

**Impact:**
- Exactly-once consumption under power loss is weaker than documented.
- The store does not follow the area's 0700/0600 custody rules.

**Suggested action:**
1. fsync the directory after each successful insert.
2. Create the directory 0700 and entry files 0600.
3. As defense in depth, validate keys against the `<network>:<64 hex>` shape in `path()`.
4. In `sweep`, remove or report zero-length or unparseable entries older than the maximum invoice lifetime.

### PAY-18 pay-host queries pay-ledger's SQLite schema with raw SQL

**Severity:** low · **Category:** architecture · **Effort:** M

**Locations:**
- [pay-host ingest.rs:182-297](../../../../crates/pay-host/src/ingest.rs)
- [pay-host Cargo.toml:21-24](../../../../crates/pay-host/Cargo.toml)
- [pay-host tests/ledger.rs:2-3](../../../../crates/pay-host/tests/ledger.rs)

**Evidence:**
- `ingest.rs` prepares SELECTs over call, settlement, share and bonus. It also runs a payout/payout_item UNION bonus_payout_item/payable_share/settlement join.
- pay-ledger is only a dev-dependency.
- `pay-host/tests/ledger.rs` builds a real `pay_ledger::Ledger`, so schema drift is caught when pay-host's tests run.

**Impact:** A pay-ledger refactor can break the public `/flow` feed, and the break surfaces only in pay-host's tests, not pay-ledger's.

**Suggested action:**
1. Add a typed read-only export API to pay-ledger (`open_read_only(..).flow_since(cursor)`), make pay-host depend on it normally, and delete the SQL in `ingest.rs`.
2. Add `PRAGMA user_version` and assert it on open (see PAY-23).

### PAY-19 pay-host enables serde_json arbitrary_precision, which unifies into any build linking it

**Severity:** low · **Category:** build · **Effort:** S

**Locations:**
- [pay-host Cargo.toml:18](../../../../crates/pay-host/Cargo.toml)
- [pay-host lib.rs:24-60](../../../../crates/pay-host/src/lib.rs)

**Evidence:**
- pay-host sets `serde_json = { version = "1", features = ["arbitrary_precision"] }` so that `Sats` can serialize decimal sats by parsing a `serde_json::Number`. Despite its name, `Sats` stores msat (lib.rs:24-26).
- Cargo feature unification means a workspace build behaves differently from a per-crate build.
- All in-tree writers serialize integers, so digests differ only for externally authored numbers such as `1.50` or `1e3`.

**Impact:**
- Digests may not reproduce between `cargo test -p X` and workspace builds. The probability is low.
- The type name is misleading.

**Suggested action:**
1. Serialize `Sats` through a `RawValue` (the serde_json `raw_value` feature), or emit integer msat, and drop `arbitrary_precision`.
2. Rename `Sats` to something like `PublicSats`, or document what it holds.
3. Verify with `cargo tree -e features -i serde_json`.

### PAY-20 pay-host serializes all SSE subscribers through a std Mutex and polls SQLite every 250 ms

**Severity:** low · **Category:** performance · **Effort:** M

**Locations:**
- [pay-host lib.rs:191](../../../../crates/pay-host/src/lib.rs), lib.rs:600-670

**Evidence:**
- `SharedStore = Arc<Mutex<Store>>` (191) is a std mutex, and async handlers lock it (614, 621, 656).
- Each stream loop sleeps 250 ms between polls (669).
- A poisoned lock maps to 500 through `internal()`, which discards the error.

**Impact:**
- Each viewer of the public flow page adds its own polling cost, and tokio workers block on the mutex.
- A panic in one handler poisons every endpoint.

**Suggested action:**
1. Run ingest on one task that publishes to `tokio::sync::broadcast` or `watch`, and serve SSE from that channel.
2. Move SQLite calls to `spawn_blocking`.
3. Recover from poison with `lock().unwrap_or_else(|e| e.into_inner())`.
4. Cache the snapshot topology per sync tick.

### PAY-21 Reserved Cashu payment method remains after Cashu was ruled out

**Severity:** low · **Category:** policy · **Effort:** S

**Locations:**
- [router.rs:35](../../../../crates/x402/src/router.rs), router.rs:66-80

**Evidence:**
- The router declares `/// Cashu NUT-24. Reserved: no adapter yet. Cashu,` and `Self::Cashu => "cashu"` (router.rs:67-68, 79).
- The module doc on line 35 says "Cashu is not planned for now (owner, 2026-10-09)".

**Impact:** A typed slot for a rejected rail shows up in exhaustive matches and in method id lists.

**Suggested action:**
1. Delete `Method::Cashu` and its `id()` arm. In its place, leave a comment that Taproot Assets over Lightning is the planned addition.
2. Annotate the payment_scheme "tempo" test vector as an upstream literal, not a supported method.
3. Verify that `git grep -i cashu crates/x402/src` returns only the policy doc line.

### PAY-22 Manifests bypass workspace inheritance and lints; invalid license string; split sha2 versions

**Severity:** low · **Category:** build · **Effort:** S

**Locations:**
- [x402 Cargo.toml:1-6](../../../../crates/x402/Cargo.toml)
- [wallet Cargo.toml:1-6](../../../../crates/wallet/Cargo.toml)
- [bitcoin-amount Cargo.toml:1-15](../../../../crates/bitcoin-amount/Cargo.toml)
- [push-gateway Cargo.toml:37](../../../../crates/push-gateway/Cargo.toml)
- [xp-ledger Cargo.toml:13](../../../../crates/xp-ledger/Cargo.toml), Cargo.toml:20
- [workspace Cargo.toml:38-40](../../../../Cargo.toml)
- [Cargo.lock:13664-13676](../../../../Cargo.lock)

**Evidence:**
- x402, wallet and bitcoin-amount hard-code `version` and `edition`, and use `license = "CC-0"`, which is not valid SPDX (the correct id is `CC0-1.0`).
- x402 and wallet have no `[lints] workspace = true`, so the workspace denial of `dbg_macro`, `todo` and `unimplemented` (Cargo.toml:38-40) does not apply to them.
- x402 and push-gateway use sha2 0.11.0, so `Cargo.lock` carries both 0.10.9 and 0.11.0.
- xp-ledger lists sha2 in both `[dependencies]` and `[dev-dependencies]`.
- `[workspace.package]` has no `license` field.

**Impact:**
- Lint policy is weakest in the payment protocol and the wallet.
- sha2 is built twice.
- License tooling sees an invalid id.

**Suggested action:**
1. Use `version`/`edition`/`rust-version`/`publish.workspace = true` in the three manifests.
2. Add `license = "CC0-1.0"` to `[workspace.package]` and inherit it.
3. Add `[lints] workspace = true` to x402 and wallet.
4. Pin sha2 once in `[workspace.dependencies]`.
5. Drop xp-ledger's duplicate dev-dependency.
6. Verify that `cargo tree -d | grep sha2` shows one version.

### PAY-23 pay-ledger schema is spread over many CREATE statements and 24 impl blocks with ad hoc migrations

**Severity:** low · **Category:** maintainability · **Effort:** M

**Locations:**
- [pay-ledger lib.rs:404-425](../../../../crates/pay-ledger/src/lib.rs)
- [pay-ledger payout.rs:84](../../../../crates/pay-ledger/src/payout.rs)
- [pay-ledger tests/migration.rs](../../../../crates/pay-ledger/tests/migration.rs)

**Evidence:**
- `initialize` runs `schema.sql`, `payout::create_table`, `compute.sql`, `earnings::TABLES`, `adjustment::TABLES` and others.
- Migrations are ad hoc: a `pragma_table_info` probe plus `ALTER TABLE settlement ADD COLUMN release_id` (lib.rs:418-423), and a payout_v2 rename (payout.rs:84).
- There is no `user_version`.
- `impl Ledger` appears 24 times across 17 files, and src contains 93 `CREATE ` matches.

**Impact:**
- Every column change needs another hand-written probe.
- Read-only consumers cannot check schema compatibility.

**Suggested action:**
1. Add numbered migrations under `crates/pay-ledger/migrations`, applied under `PRAGMA user_version`, mirroring `tenancy::db::MIGRATIONS`.
2. Fold the current `CREATE IF NOT EXISTS` sets into v1 and the probes into v2.
3. Assert the version in read-only opens.
4. Keep `tests/migration.rs` as the upgrade test.

### PAY-24 secret-screen lacks credential shapes for the payment stack (Stripe, BIP39, xprv, macaroons)

**Severity:** low · **Category:** security · **Effort:** S

**Locations:**
- [secret-screen lib.rs:27-128](../../../../crates/secret-screen/src/lib.rs)

**Evidence:**
- The existing rules are private-key, claude-oauth-env, anthropic-key, openai-key, openagents-key, github-token, slack-token, aws-key, google-key, nostr-secret, jwt, bearer, home-directory and email.
- `git grep 'sk_live|whsec|xprv' crates/secret-screen` returns nothing.

**Impact:** A leaked Stripe key or wallet mnemonic in traces or memory passes the screen.

**Suggested action:**
1. Add these rules:
   - `stripe-key`: `\b(sk|rk)_(live|test)_[A-Za-z0-9]{16,}` and `whsec_[A-Za-z0-9]{16,}`
   - `bip32-key`: `\b[xt]prv[1-9A-HJ-NP-Za-km-z]{100,}`
   - `bip39-phrase`: 12 or 24 consecutive BIP39 English words
2. Add a test for each new rule, in the style of the existing shape tests.

### PAY-25 receipts crate mixes funnel, pilot and policy modules with receipt contracts; capability and oa-tokens are misplaced in this group

**Severity:** low · **Category:** maintainability · **Effort:** M

**Locations:**
- [receipts lib.rs:29-48](../../../../crates/receipts/src/lib.rs)
- [capability lib.rs:1-5](../../../../crates/capability/src/lib.rs)
- [oa-tokens Cargo.toml:7](../../../../crates/oa-tokens/Cargo.toml)

**Evidence:**
- receipts exports `brainstorm_pilot`, `sales_funnel`, `team_policy`, `feedback`, `service_sale`, `shared_spend`, `funding_units` and `purchase` alongside `execution`, `export` and `validate`.
- capability is an agent capability contract, and oa-tokens is "The OpenAgents design token table". Neither is a payments crate.

**Impact:** Money unit types sit behind a grab-bag API, which makes them more expensive to review.

**Suggested action:**
1. Move `funding_units` and `purchase` into a `pay-types` leaf crate.
2. Move `brainstorm_pilot` and `sales_funnel` next to their consumers.
3. Re-home capability and oa-tokens in the Coder and UI audit sections.

## Refuted during verification

The verifier rejected no findings outright. It did correct several claims, so future readers should not raise them again:

- **PAY-03:** billing is not affected by the stale-lock problem. `BillingLock` already uses OS `try_lock` (billing.rs:2053).
- **PAY-08:** a duplicate custody copy in `wallet/custody.rs:91-105` was not confirmed, so it was dropped.
- **PAY-10:** pay-ledger has 256 `Error::Invalid` sites, not 281. Derived Debug keeps error sources, so the opacity affects only Display-based logs.
- **PAY-11:** x402 has 13 more tests than the unit counts suggest (`tests/front.rs` 8, `tests/router.rs` 5).
- **PAY-16:** an empty or partial seed fails loudly at `Mnemonic::from_str`. It is not silently accepted.
- **PAY-17:** replay-key path traversal is not reachable, because keys are built from validated fields. A 0-byte entry fails closed.
- **PAY-18:** schema drift is not invisible to CI, because pay-host's integration tests run against a real pay-ledger schema.
- **PAY-23:** the actual counts are 24 `impl Ledger` blocks in 17 files and 93 `CREATE` matches, not 26 in 19 and about 68.
