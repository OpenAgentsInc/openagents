# Gateway, routing, inference

**Scope:** `crates/{gateway, route-contract, inference, openrouter, model-access, codex-transport, claude_agent_sdk, acp-client}` · **Health grade: C** · Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`.

The area is about 122k lines of Rust across 8 crates. `gateway` accounts for 72,380 of them (88 files) and is the main problem. Its Cargo description is still "the keyed HTTP gateway in front of decision doors", but the crate now also holds Stripe card funding, subscriptions, payout earnings, referrals, SSO, a skills directory, server-rendered dashboards and a playground, the public OpenAI-compatible inference API and x402 pay-per-request. It pulls in `gym` (110k LOC) for one module. `serve.rs` is 6,008 lines, has changed in 67 commits in the last month, and contains functions of 489, 409 and 329 lines. Several reliability problems sit on live paths. Receipt writes run a blocking fsync on the async runtime under one global mutex and fail silently. The public-inference key book drops spend-persistence errors. CORS uses a fail-open deny-list. There is no structured logging. Mutex poisoning is handled three different ways. The money handlers in `earnings.rs` and `funding.rs` are the weakest code: untyped `serde_json` round-trips with unwraps, and lines rustfmt cannot format.

The leaf crates (`inference`, `route-contract`, `acp-client`, `codex-transport`, `claude_agent_sdk`, `model-access`, `openrouter`) are in much better shape. They are well documented, have almost no unwraps outside tests, zero TODO/FIXME markers, careful secret redaction and explicit network timeouts. Their main problem is duplication across crates: three SSE parsers (one quadratic and CRLF-blind), four redacted-key types, two Codex price tables, two `usd_micros` parsers and two process-group killers. There are also stale docs and dependency version skew. Test volume is high, but security- and money-critical modules such as `sso.rs` and `inference_x402.rs` have no direct tests.

## Measurements

| Metric | Value |
|---|---|
| LOC (tracked `.rs`) | gateway 72,380 (88 files; src ~44k, tests ~28k), inference 23,796 (62), route-contract 7,338 (22), claude_agent_sdk 7,011 (13), acp-client 4,554 (10), model-access 2,573 (5), openrouter 2,491 (3), codex-transport 2,275 (6) |
| Largest files | gateway/tests/serve.rs 6,328; gateway/src/serve.rs 6,008; accounts.rs 2,709; classify.rs 2,416; relay_worker.rs 2,156; openrouter/src/lib.rs 2,098 (~1,430 non-test) |
| Longest functions (serve.rs) | review_phase ~489, classify_run 409, admitted 329, ServeState::open_with ~255, authenticate_session 254, unit_result 224, money_hold 205 |
| Gateway LOC by subsystem | decision admission/accounts/config 17,993; billing/funding/earnings/money 12,234; dashboard/skills/sso/team/usage 7,626; inference API 5,708 |
| Test fns | gateway 394 (292 integration), inference 149, claude_agent_sdk 58, route-contract 51, acp-client 46, codex-transport 33, openrouter 32, model-access 23 |
| Non-test unwrap/expect (gateway hot spots) | relay_worker 17, earnings 17, billing 10, funding 8, team_reports 5; other crates 0-4 each |
| TODO/FIXME | 0 in all 8 crates |
| `#[allow]` | gateway 11 (all `clippy::result_large_err`), inference 2 |
| `Err(r) => return r` boilerplate | 267 in gateway (accounts.rs 88) |
| `json!` in serve.rs | 144 |
| Gateway src lines > 140 chars | 261 |
| Logging | ~60 `eprintln!` in gateway (58 verified); 0 `tracing` in gateway and inference |
| Commits | gateway 136 (since 2026-01-08), claude_agent_sdk 31, inference 18, route-contract 17, acp-client 16, openrouter 13, model-access 9, codex-transport 6; serve.rs 67 since 2026-09-10 |
| Dependents | route-contract 14, model-access 10, codex-transport 8, acp-client 7, openrouter 6, inference 2, gateway 2 (dev-deps only), claude_agent_sdk 1 (microcoder) |
| Cargo.lock duplicates | sha2 0.10/0.11, base64 x4, reqwest 0.12/0.13, hmac 0.12/0.13, tokio-tungstenite x3 |
| CI | No `.github/workflows` (no fmt or clippy gate) |

## Strengths

- Module docs state contracts precisely: the admission sequence in `gateway/src/serve.rs:1-24`, router steps in `inference/src/router.rs:1-35`, custody rules in `codex-transport/src/codex.rs:1-40`. Keep this standard.
- Routing follows the workspace semantic-routing policy: `inference/src/router.rs:15` routes `openagents/auto` through a typed `ClassJudge`; no ad hoc string-intent matching was found.
- Secrets are handled carefully. Key types redact Debug/Display; codex-transport refuses to refresh or rewrite `~/.codex/auth.json` and marks the process non-dumpable (`codex.rs:219-381`); `gateway/src/inference_byok.rs` uses `Zeroizing` for opened provider keys.
- Webhook HMAC verification is constant-time (`billing.rs:489-501`, `mac.verify_slice`); SSO pins RS256 to reviewed keys with no JWKS fetch (`sso.rs:37-55`).
- `inference` upstream HTTP sets explicit connect, headers and quiet-stream timeouts (`inference/src/upstream/http.rs:15-23, 55, 177`); fallback happens only before the first token.
- Money is integer (micro-dollars, msat) across inference and gateway billing; `usd_micros` parses decimal text without floats (`router.rs:501-526`).
- `inference/src/sse.rs` is a correct WHATWG SSE decoder (CR, LF, CRLF, split UTF-8) and should become the shared one.
- `route-contract` is a clean pure-data crate (serde, serde_json, sha2, workbench) with versioned schemas, canonical-JSON digests and JSON fixtures, used by 14 crates.
- `claude_agent_sdk` tracks upstream parity explicitly (`PARITY.md` + `scripts/check-claude-sdk-parity.sh`) and has no non-test panics or unwraps.
- High test volume with recorded upstream fixtures (`inference/fixtures/upstream`, `acp-client/fixtures`).
- Zero TODO/FIXME; workspace lints deny `dbg!`, `todo!` and `unimplemented!`.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| GW-01 | high | error-handling | Receipt writes fsync on the async runtime under one global lock; failures silently dropped | M |
| GW-02 | high | architecture | gateway is a 72k-LOC god crate spanning six product domains | XL |
| GW-03 | medium | error-handling | Public inference key book drops spend-persistence errors and rewrites the whole file per charge | M |
| GW-04 | medium | security | CORS fail-open deny-list gives 5 credentialed GET routes `ACAO: *` | S |
| GW-05 | medium | maintainability | serve.rs is a 6,008-line god file with 200-490 line functions | L |
| GW-06 | medium | maintainability | No structured logging or observability | M |
| GW-07 | medium | error-handling | Earnings handlers round-trip untyped JSON with unwraps | M |
| GW-08 | medium | concurrency | Three mutex-poisoning policies, one silently disables metering | S |
| GW-09 | medium | build | No formatting gate; unformatted funding.rs logic leaks internal codes | S |
| GW-10 | medium | duplication | Three SSE parsers; the Codex one is quadratic and CRLF-blind | M |
| GW-11 | medium | concurrency | No graceful shutdown; detached tasks and Drop-spawned settlements lost on SIGTERM | M |
| GW-12 | medium | testing | Security- and money-critical modules have no direct tests | M |
| GW-13 | medium | architecture | ~20 JSON/JSONL state files with silent read failures, plus ambient global Postgres | L |
| GW-14 | medium | architecture | 35 public modules, only classify and relay_worker used externally | M |
| GW-15 | low | duplication | 267 early-return matches and 5 error-envelope builders | M |
| GW-16 | low | duplication | Four redacted API-key types, none zeroizing | S |
| GW-17 | low | duplication | Codex price tables duplicated; `-pro` handling differs | S |
| GW-18 | low | duplication | Small helpers duplicated (time, hex, digest, usd_micros) | S |
| GW-19 | low | repo-hygiene | acp-client compiles replay fixtures and test helpers into production | S |
| GW-20 | low | duplication | Two process-group spawn/kill implementations | S |
| GW-21 | low | docs | Stale docs and identifiers from renames | S |
| GW-22 | low | build | Dependency version skew and hand-rolled encoders | S |
| GW-23 | low | maintainability | Parallel string matches with `unreachable!()` in Stripe read path | S |
| GW-24 | low | maintainability | Test scaffolding and lint workarounds in production code | S |
| GW-25 | low | maintainability | openrouter is a 2,098-line single file | S |

### GW-01 Receipt writes fsync on the async runtime under one global lock, and failures are silently dropped

**Severity:** high · **Category:** error-handling · **Effort:** M

**Locations:** [serve.rs:5600-5639](../../../../crates/gateway/src/serve.rs#L5600), [serve.rs:207](../../../../crates/gateway/src/serve.rs#L207), [serve.rs:5644-5660](../../../../crates/gateway/src/serve.rs#L5644), [inference_x402.rs:888-899](../../../../crates/gateway/src/inference_x402.rs#L888)

**Evidence:** serve.rs:5634-5637:
```rust
let line = serde_json::to_string(&receipt).ok()?;
let mut file = state.receipts.lock().await;
writeln!(file, "{line}").ok()?;
file.sync_all().ok()?;
```
`receipts: Mutex<std::fs::File>` (serve.rs:207). `respond()` (5658-5660) adds `x-receipt` only `if let Some(digest)`, so a lost receipt is invisible to the caller.

**Impact:** A full disk or EIO silently loses audit/billing receipts while the request succeeds. The blocking fsync runs on a tokio worker under a process-wide lock and serializes every admission.

**Suggested action:**
1. Move appends to a dedicated writer task: bounded mpsc into a `spawn_blocking` loop with group fsync; return the digest via oneshot.
2. Change `write_receipt` to return `Result<String, ReceiptError>`.
3. On `Err`, emit a `receipt_unwritten` event, count it in `/healthz`, and either refuse with 503 `receipt_unavailable` before settlement or set `x-receipt: unavailable`.
4. Apply the same to the x402 payment-receipt path (inference_x402.rs:888-899).
5. Verify: test with a read-only `receipts.jsonl` and assert the request reports the failure and the counter increments.

### GW-02 The gateway crate is a 72k-LOC god crate spanning six product domains

**Severity:** high · **Category:** architecture · **Effort:** XL

**Locations:** [lib.rs](../../../../crates/gateway/src/lib.rs), [Cargo.toml:1-12](../../../../crates/gateway/Cargo.toml), [team_reports.rs](../../../../crates/gateway/src/team_reports.rs)

**Evidence:** lib.rs declares 41 modules (35 `pub mod`). Cargo description still reads "The keyed HTTP gateway in front of decision doors". `gym = { path = "../gym" }` is used only in team_reports.rs (19 `gym::` uses, no other file). serve.rs 6,008, accounts.rs 2,709, classify.rs 2,416 lines.

**Impact:** Every edit recompiles and retests the whole crate and its heavy dependency graph; ownership of money invariants is blurred.

**Suggested action:**
1. Keep `ServeState` as a thin composition root and split along existing seams: `gateway-billing` (billing, card_funding, subscriptions, funding, money, budgets, shared_spend, purchase, commercial), `gateway-earnings`, `gateway-inference-api` (`inference_*`), `gateway-web` (dashboard, playground).
2. Move `gym::sales_evidence` into a small data crate so team_reports stops pulling in gym.
3. Each new crate exposes `routes()` over a state trait.
4. Update the Cargo description.
5. Verify: `cargo tree -p gateway` shrinks, and gateway tests pass unchanged.

### GW-03 Public inference key book drops spend-persistence errors and rewrites the whole file on every charge

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:** [inference_public.rs:297-302](../../../../crates/gateway/src/inference_public.rs#L297), [inference_public.rs:349-359](../../../../crates/gateway/src/inference_public.rs#L349), [inference_public.rs:361-377](../../../../crates/gateway/src/inference_public.rs#L361), [inference_public.rs:1037](../../../../crates/gateway/src/inference_public.rs#L1037)

**Evidence:** `Book::save` (297-302) does `to_vec_pretty` + `fs::write(tmp)` + rename with no fsync. `add_spend` (370-377) and `give_back_free` (361-368) end with `let _ = self.save();`. `take_free` (349-359) returns `self.save().is_ok()`, so a failed free-count write is already refused. `add_spend` is called under the `book` lock at line 1037.

**Impact:** In-memory spend is still updated, so caps hold within the process. A failed or non-durable save loses recorded spend on restart, and the key can then exceed its owner-set caps. Each paid request serializes and rewrites the whole book while holding the async lock.

**Suggested action:**
1. Make `add_spend` (and `give_back_free`) return `Result`; log `key_spend_unwritten` and refuse further paid requests for that key until a save succeeds, mirroring `take_free`.
2. Persist spend as an append-only journal or in the tenancy Postgres store instead of rewriting the file.
3. fsync the file and its directory after rename; run IO in `spawn_blocking`.
4. Verify: test with a read-only book directory; assert the next paid request is refused and spend survives a reload.

### GW-04 CORS uses a fail-open deny-list; 5 credentialed GET routes get Access-Control-Allow-Origin: *

**Severity:** medium · **Category:** security · **Effort:** S

**Locations:** [serve.rs:592-624](../../../../crates/gateway/src/serve.rs#L592), [serve.rs:635-698](../../../../crates/gateway/src/serve.rs#L635), [inference_public.rs:59-60](../../../../crates/gateway/src/inference_public.rs#L59), [inference_state.rs:43](../../../../crates/gateway/src/inference_state.rs#L43), [inference_status.rs:30-31](../../../../crates/gateway/src/inference_status.rs#L30)

**Evidence:** `CREDENTIALED_GET` (595-615) lacks `/v1/key`, `/v1/usage/`, `/v1/responses/`, `/v1/admin/`. `credentialed()` is a prefix match on that list; public = GET && !credentialed, so these routes get `ACAO: *` and public preflights (GET, OPTIONS, no allow-headers). There is no cookie auth in serve.rs (0 hits for "cookie"), so Authorization-bearing reads are still blocked by the preflight today.

**Impact:** Not exploitable today, but the classification is wrong by construction and any new route defaults to public. A later change to preflight headers or the addition of cookie auth would expose private usage, stored responses and admin status cross-origin.

**Suggested action:**
1. Invert to an explicit `PUBLIC_GET` allow-list, or better, attach an `Exposure` to each route in `api_routes` and build the CORS table from it.
2. Add a test that walks mounted paths and asserts every route has an explicit exposure.
3. Assert `/v1/key`, `/v1/responses/{id}` and `/v1/admin/*` return no `ACAO` for an unadmitted Origin.

### GW-05 serve.rs is a 6,008-line god file with functions of 200 to 490 lines and the most churn in the area

**Severity:** medium · **Category:** maintainability · **Effort:** L

**Locations:** [serve.rs:4045](../../../../crates/gateway/src/serve.rs#L4045), [serve.rs:2755](../../../../crates/gateway/src/serve.rs#L2755), [serve.rs:2263](../../../../crates/gateway/src/serve.rs#L2263), [serve.rs:245-499](../../../../crates/gateway/src/serve.rs#L245), [serve.rs:1207](../../../../crates/gateway/src/serve.rs#L1207)

**Evidence:** 6,008 lines; review_phase ~489, classify_run 409, admitted 329, `open_with` ~255 (starting at 245). 144 `json!` uses; 67 commits since 2026-09-10.

**Impact:** High review and merge-conflict cost on the admission path that every paid decision runs through; untyped `json!` results let field typos through.

**Suggested action:**
1. Split into `serve/{state,auth,admission,classify_run,forward,receipt}.rs`.
2. Replace `json!` result bodies with typed `UnitResult` / `ItemResult` / `Attempt` structs deriving `Serialize`.
3. Break `review_phase` into named steps; target no function over 120 lines.
4. Verify: `tests/serve.rs` passes unchanged and `x-receipt` digests are byte-identical before and after.

### GW-06 No structured logging or observability in a production HTTP gateway

**Severity:** medium · **Category:** maintainability · **Effort:** M

**Locations:** [gateway/Cargo.toml](../../../../crates/gateway/Cargo.toml), [inference/Cargo.toml](../../../../crates/inference/Cargo.toml), [inference_x402.rs:888-911](../../../../crates/gateway/src/inference_x402.rs#L888), [bin/gateway.rs:89](../../../../crates/gateway/src/bin/gateway.rs#L89)

**Evidence:** No `tracing` dependency in gateway or inference. 58 `eprintln!` in gateway/src. The silent `.ok()?` in `write_receipt` and `let _ = self.save()` emit nothing.

**Impact:** Operators cannot correlate a request across admission, forward, settlement and receipt, or alert on lost receipts and failed saves.

**Suggested action:**
1. Add `tracing` + `tracing-subscriber` (JSON output) to gateway and inference; initialize in `bin/gateway.rs` and `bin/decision-worker.rs`.
2. Add a request span (request_id, attempt, door, tenant digest) via a tower layer next to the CORS layer.
3. Replace `eprintln!` with named events (`receipt_unwritten`, `key_spend_unwritten`, `payment_receipt_unwritten`) and add `warn!` at every silent durable-write failure.
4. Verify: grep shows 0 `eprintln!` in gateway/src; a test subscriber captures `receipt_unwritten` in the GW-01 read-only test.

### GW-07 The earnings (payout) handlers round-trip through untyped JSON with unwraps and unformattable lines

**Severity:** medium · **Category:** error-handling · **Effort:** M

**Locations:** [earnings.rs:255](../../../../crates/gateway/src/earnings.rs#L255), [earnings.rs:263](../../../../crates/gateway/src/earnings.rs#L263), [earnings.rs:278-279](../../../../crates/gateway/src/earnings.rs#L278), [earnings.rs:572-587](../../../../crates/gateway/src/earnings.rs#L572), [earnings.rs:704](../../../../crates/gateway/src/earnings.rs#L704), [serve.rs:757-758](../../../../crates/gateway/src/serve.rs#L757)

**Evidence:** `state.earnings.as_ref().unwrap()` at 255, 291, 405, 450; `body["statement"]["payouts"].as_array_mut().unwrap()` and `payout["id"].as_str().unwrap()` at 278-279; 572-587 repeat the pattern for HTML. Lines 263 (372 chars), 587 (376), 704 (272), 465, 611 and 612 exceed 250 chars. Earnings routes mount only under `if state.config.earnings.is_some()` (serve.rs:757-758).

**Impact:** The `state.earnings` unwraps are guarded by conditional mounting. The `Value`-path unwraps, however, panic a payout handler on any `pay_ledger` statement schema drift, and untyped access hides that drift.

**Suggested action:**
1. Hold `Option<Arc<EarningsState { ledger, config }>>` and pass it as route-level `State` so the unwraps disappear.
2. Serialize `pay_ledger`'s typed statement with a typed `reconciliation` field instead of mutating `Value`.
3. Render HTML from typed rows.
4. Move long `format!` strings to const templates so rustfmt formats the file.
5. Verify: `cargo fmt --check` passes on earnings.rs and non-test `unwrap` count in earnings.rs is 0.

### GW-08 Three different mutex-poisoning policies, including silently disabling metering forever

**Severity:** medium · **Category:** concurrency · **Effort:** S

**Locations:** [meter/mod.rs:190-412](../../../../crates/inference/src/meter/mod.rs#L190), [measure.rs:125-131](../../../../crates/inference/src/upstream/measure.rs#L125), [relay_worker.rs](../../../../crates/gateway/src/relay_worker.rs)

**Evidence:** meter/mod.rs uses `self.inner.lock().ok()?` (199, 231, 274, 293), `if let Ok` (263) and `let Ok(..) else` (320, 412). measure.rs:126-129 recovers with `poisoned.into_inner()`. relay_worker.rs has 13 `.lock().expect(`.

**Impact:** After one panic under the Meter lock, recording, credit burn-down and alerts silently stop for the rest of the process lifetime. In relay_worker, one panic cascades into every later job.

**Suggested action:**
1. Pick one policy: `parking_lot::Mutex` (no poisoning) or a shared `lock()` helper that logs and recovers via `into_inner`.
2. Apply it at all sites; remove `.ok()?` from `Meter`.
3. Verify: unit test that panics inside a Meter lock via `catch_unwind`, then asserts later record calls still update the ledger.

### GW-09 No formatting gate, and unformatted money logic in funding.rs leaks internal codes

**Severity:** medium · **Category:** build · **Effort:** S

**Locations:** [funding.rs:722-747](../../../../crates/gateway/src/funding.rs#L722), [funding.rs:750-752](../../../../crates/gateway/src/funding.rs#L750), [funding_tests.rs](../../../../crates/gateway/src/funding_tests.rs)

**Evidence:** `.github` contains only `ISSUE_TEMPLATE`. funding.rs:722-747 is a `spawn_blocking` closure of unspaced one-liners (`let mut store=state.funding.as_ref().ok_or(...)?.lock().map_err(...)?;`). Every `String` error maps to `refused(StatusCode::CONFLICT, "funding_unavailable", ..)`. The response body includes `"production_qualification"` (`owner_required_O5_O8`).

**Impact:** Funding quote/reconcile logic is hard to review, all failure classes look the same to clients, and internal program codes appear in a public API body.

**Suggested action:**
1. Add a CI job or pre-push hook running `cargo fmt --all --check` and scoped clippy for gateway and inference.
2. Rewrite the closure as `fn funding_operation(..) -> Result<Value, FundingError>` with a typed enum mapped to 400/403/409/503.
3. Drop or rename `production_qualification` in the public body.
4. Verify: `cargo fmt --check -p gateway` passes; tests assert distinct status codes per error class.

### GW-10 Three SSE parsers, and the Codex one is quadratic and blind to CRLF framing

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:** [codex.rs:639-662](../../../../crates/codex-transport/src/codex.rs#L639), [codex.rs:776-780](../../../../crates/codex-transport/src/codex.rs#L776), [openrouter/src/lib.rs:1086-1216](../../../../crates/openrouter/src/lib.rs#L1086), [inference/src/sse.rs](../../../../crates/inference/src/sse.rs)

**Evidence:** `push_streaming` runs `while let Some(end) = find(&self.buffer, b"\n\n")` on raw bytes and only afterwards does `.replace('\r', "")`. `find` is a `windows().position()` scan from the start of the buffer on every chunk. Non-JSON data is skipped with `let Ok(event) = ... else { continue; }`. inference/src/sse.rs is a separate, correct decoder.

**Impact:** A CRLF-framed upstream would not dispatch any event until the stream ends; large multi-chunk events cost quadratic scanning; SSE bugs must be fixed in three places.

**Suggested action:**
1. Extract `inference::sse::SseDecoder` into a small `sse-codec` crate.
2. Feed codex-transport `Events` and openrouter `StreamReader` bytes through it.
3. Count and log non-JSON frames instead of skipping silently.
4. Verify: a codex test feeds `\r\n\r\n`-framed events split at every byte boundary and asserts all events dispatch in order.

### GW-11 No graceful shutdown; detached background tasks and Drop-spawned settlements are lost on SIGTERM

**Severity:** medium · **Category:** concurrency · **Effort:** M

**Locations:** [bin/gateway.rs:89](../../../../crates/gateway/src/bin/gateway.rs#L89), [serve.rs:573-579](../../../../crates/gateway/src/serve.rs#L573), [inference_public.rs:995-1003](../../../../crates/gateway/src/inference_public.rs#L995)

**Evidence:** bin/gateway.rs:89 is `axum::serve(listener, serve::router(state)).await` with no `with_graceful_shutdown` (0 hits for "graceful"). `Settler::drop` spawns `settle(&state, ticket, None, None)` via `Handle::try_current()` without tracking it.

**Impact:** On restart, in-flight paid holds and reservations are abandoned mid-settlement and left for reconciliation.

**Suggested action:**
1. Add a `CancellationToken` and `TaskTracker` to `ServeState`.
2. Keep resume-loop `JoinHandle`s in a `JoinSet` (serve.rs:573-579).
3. Use `with_graceful_shutdown` on SIGTERM / ctrl_c, then cancel and await tracked tasks with a timeout.
4. Spawn `Settler` drops through the tracker so shutdown awaits them.
5. Verify: an integration test sends shutdown during a paid request and asserts the hold is settled before exit.

### GW-12 Security- and money-critical gateway modules have no direct tests

**Severity:** medium · **Category:** testing · **Effort:** M

**Locations:** [sso.rs](../../../../crates/gateway/src/sso.rs), [inference_x402.rs](../../../../crates/gateway/src/inference_x402.rs), [inference_pylon.rs](../../../../crates/gateway/src/inference_pylon.rs), [shared_spend.rs](../../../../crates/gateway/src/shared_spend.rs), [referrals.rs](../../../../crates/gateway/src/referrals.rs)

**Evidence:** `#[test]` / `#[tokio::test]` count is 0 in sso.rs, inference_x402.rs, shared_spend.rs, referrals.rs and inference_pylon.rs; no file in `crates/gateway/tests` mentions `sso`.

**Impact:** Regressions in SSO token sign-in (algorithm confusion, audience, expiry) or x402 refunds would ship unnoticed.

**Suggested action:**
1. Add `tests/sso.rs`: a valid RS256 token issues a session and access record; HS256 and `none` are refused; wrong audience and expired tokens are refused; an existing Authorization header gets `already_signed_in`.
2. Add unit tests for inference_x402 refund-before-first-token and for shared_spend.
3. Verify: temporarily make `Rs256::verify` return true and confirm the SSO tests fail.

### GW-13 Durable state is spread over about 20 JSON/JSONL files with silent read failures, plus an ambient global Postgres binding

**Severity:** medium · **Category:** architecture · **Effort:** L

**Locations:** [jobs.rs:1020-1030](../../../../crates/gateway/src/jobs.rs#L1020), [jobs.rs:827](../../../../crates/gateway/src/jobs.rs#L827), [tenancy/src/db.rs:480-486](../../../../crates/tenancy/src/db.rs#L480)

**Evidence:** jobs.rs:1020-1026: `fs::read_to_string(path).unwrap_or_default().lines().filter_map(|line| serde_json::from_str(line).ok())` silently drops IO errors and corrupt lines; it feeds delivery/resume logic. (The tenancy global binding was not re-read during verification.)

**Impact:** Corruption is undetectable and could cause re-delivery; the gateway carries two persistence models.

**Suggested action:**
1. Return `Result` from `read_lines`; count and log skipped lines.
2. Refuse to resume a job whose ledger fails to parse.
3. Pass `Option<tenancy::db::Database>` explicitly into `ServeState` instead of a global.
4. Document a file-store migration plan, starting with `inference-keys.json` and `receipts.jsonl`.
5. Verify: a test with a truncated JSONL line asserts the job is not resumed and the skip is reported.

### GW-14 The gateway exposes 35 public modules, but external crates use only classify and relay_worker

**Severity:** medium · **Category:** architecture · **Effort:** M

**Locations:** [gateway/src/lib.rs](../../../../crates/gateway/src/lib.rs), [oak/Cargo.toml:39](../../../../crates/oak/Cargo.toml), [oak/tests/classification_contract.rs:3](../../../../crates/oak/tests/classification_contract.rs#L3), [jev-hosted/Cargo.toml:23](../../../../crates/jev-hosted/Cargo.toml), [jev-hosted/tests/hosted.rs:14](../../../../crates/jev-hosted/tests/hosted.rs#L14)

**Evidence:** 35 `pub mod` in lib.rs. External uses are only `gateway::classify` in oak/tests and `gateway::relay_worker` in jev-hosted/tests.

**Impact:** `pub` hides dead code from rustc; oak's tests compile the whole gateway graph just to parse a classify envelope.

**Suggested action:**
1. Extract classify.rs into a `classify-contract` crate for oak.
2. Make unused modules `pub(crate)` and delete whatever dead-code warnings surface.
3. Verify: `cargo build -p oak --tests` no longer builds gateway; `cargo check -p gateway` is warning-free.

### GW-15 267 hand-written early-return matches and 5 different error-envelope builders in gateway

**Severity:** low · **Category:** duplication · **Effort:** M

**Locations:** [accounts.rs:174-184](../../../../crates/gateway/src/accounts.rs#L174), [serve.rs:5691-5701](../../../../crates/gateway/src/serve.rs#L5691), [updates.rs:211-219](../../../../crates/gateway/src/updates.rs#L211), [referrals.rs:46-56](../../../../crates/gateway/src/referrals.rs#L46), [inference_rates.rs:70-80](../../../../crates/gateway/src/inference_rates.rs#L70)

**Evidence:** 267 `Err(r) => return r`-style blocks; 5 refusal builders; 11 `#[allow(clippy::result_large_err)]`. funding.rs:719 and 752 show the pattern (`if let Err(response) = current(..) { return response; }`, `crate::accounts::refused(..)`).

**Impact:** Verbose handlers and inconsistent client error shapes.

**Suggested action:**
1. Introduce `ApiError { status, code, message }` implementing `IntoResponse`.
2. Convert helpers to `Result<T, ApiError>` so `?` replaces the matches.
3. Delete the duplicate builders and the `result_large_err` allows.
4. Verify: a test asserts every refusal body has `code` and `message`.

### GW-16 Four separate redacted API-key types, none zeroizing

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:** [model-access/src/lib.rs:160-195](../../../../crates/model-access/src/lib.rs#L160), [openrouter/src/lib.rs:59-81](../../../../crates/openrouter/src/lib.rs#L59), [codex.rs:455-470](../../../../crates/codex-transport/src/codex.rs#L455), [upstream/secret.rs:13-44](../../../../crates/inference/src/upstream/secret.rs#L13)

**Evidence:** `<redacted>` appears in openrouter/src/lib.rs, codex-transport/src/codex.rs, inference/src/upstream/secret.rs and inference/src/upstream/http.rs, plus the model-access `ApiKey`.

**Impact:** Each copy redacts correctly, but redaction and zeroize improvements must be made in several places.

**Suggested action:**
1. Create one `Secret(Zeroizing<String>)` with redacted Debug/Display and no `Serialize` in a dependency-light leaf crate.
2. Replace the per-crate types with re-exports or newtypes.
3. Verify: the existing redaction tests in each crate still pass.

### GW-17 Codex price tables are duplicated; only -pro handling differs

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:** [price.rs:1-6](../../../../crates/codex-transport/src/price.rs#L1), [price.rs:53-104](../../../../crates/codex-transport/src/price.rs#L53), [price.rs:111-120](../../../../crates/codex-transport/src/price.rs#L111), [price.rs:152-161](../../../../crates/codex-transport/src/price.rs#L152), [coder-delegate/src/delegate.rs:144-170](../../../../crates/coder-delegate/src/delegate.rs#L144)

**Evidence:** Both tables hold identical rows (gpt-6-astra 10/1/50, gpt-6-sol 2/0.2/10, gpt-6.1-sol 2/0.2/10, gpt-6-luna 0.1/0.01/0.5). `codex_transport::price::rates` strips `-pro`; `coder-delegate::codex_cost` does not, so a `gpt-6-sol-pro` run is priced by the transport and returns `None` from the delegate. Neither `price::cost` nor `codex_cost` applies long-context rates (only `upper_bound` does, via `rates_for`). price.rs:3 cites the stale path `crates/coder-one/src/delegate.rs`.

**Impact:** Two places to update prices; `-pro` models get no delegate cost estimate.

**Suggested action:**
1. Delete `CODEX_PRICES` / `codex_cost` from coder-delegate and call `codex_transport::price::{rates, cost}` (add the dependency).
2. Fix the doc path in price.rs:3.
3. Verify: a test asserts identical cost for `gpt-6-sol-pro` through both callers.

### GW-18 Small helpers duplicated within the area (time, hex, digest, usd_micros)

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:** [serve.rs:5704-5748](../../../../crates/gateway/src/serve.rs#L5704), [accounts.rs:493](../../../../crates/gateway/src/accounts.rs#L493), [funding.rs:777](../../../../crates/gateway/src/funding.rs#L777), [jobs.rs:989](../../../../crates/gateway/src/jobs.rs#L989), [router.rs:501-526](../../../../crates/inference/src/router.rs#L501), [measure.rs:333-369](../../../../crates/inference/src/upstream/measure.rs#L333)

**Evidence:** The funding.rs closure uses a local `unix_now()`; copies also exist in serve.rs, accounts.rs, funding.rs and emit.rs, and there are two differing `usd_micros` parsers (router.rs and measure.rs). Not every site was re-verified.

**Impact:** Drift between copies, notably edge cases in the two money parsers.

**Suggested action:**
1. Add `gateway/src/util.rs` (`unix_now`, `now_utc`, `digest_bytes`); use the `hex` crate everywhere.
2. Keep one strict `inference::money::usd_micros`.
3. Verify: property-test the old parsers against the new one before deleting them.

### GW-19 acp-client compiles recorded test fixtures and panicking test helpers into production

**Severity:** low · **Category:** repo-hygiene · **Effort:** S

**Locations:** [acp-client/src/lib.rs:36](../../../../crates/acp-client/src/lib.rs#L36), [replay.rs](../../../../crates/acp-client/src/replay.rs)

**Evidence:** lib.rs:36 declares `pub mod replay;` unconditionally.

**Impact:** Fixture bloat and test-only panicking APIs on the production surface.

**Suggested action:**
1. Gate it: `#[cfg(any(test, feature = "test-support"))] pub mod replay;`.
2. Enable `test-support` only from dev-dependencies of consumers.
3. Verify: `cargo build -p acp-client` (no features) succeeds without replay; all dependents' tests pass.

### GW-20 Two separate process-group spawn and kill implementations for agent CLIs

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:** [acp-client/src/process.rs:65-105](../../../../crates/acp-client/src/process.rs#L65), [claude_agent_sdk/src/transport/process.rs:90-100](../../../../crates/claude_agent_sdk/src/transport/process.rs#L90), [claude_agent_sdk/src/transport/process.rs:285-295](../../../../crates/claude_agent_sdk/src/transport/process.rs#L285)

**Evidence:** One uses `killpg`, the other `kill(-group, SIGKILL)`, with different escalation. (Not re-read in depth during verification.)

**Impact:** Divergent cleanup semantics per engine.

**Suggested action:**
1. Share one SIGTERM, grace period, then SIGKILL helper.
2. Verify: a test spawns `sh -c 'sleep 100 & sleep 100'` through each path and asserts no pid in the group survives.

### GW-21 Stale docs and identifiers left from renames and adapter growth

**Severity:** low · **Category:** docs · **Effort:** S

**Locations:** [codex.rs:58-60](../../../../crates/codex-transport/src/codex.rs#L58), [price.rs:3-6](../../../../crates/codex-transport/src/price.rs#L3), [upstream/mod.rs:10-19](../../../../crates/inference/src/upstream/mod.rs#L10), [route-contract/src/lib.rs:1](../../../../crates/route-contract/src/lib.rs#L1)

**Evidence:** codex.rs:58-60 says "The client identity Microluna sends as `originator`", with `ORIGINATOR = "openagents_microluna"` sent as a header at line 583. price.rs:3 cites `crates/coder-one/src/delegate.rs`; the table is in `crates/coder-delegate/src/delegate.rs:147`.

**Impact:** Misleading docs; the upstream originator string names a retired client.

**Suggested action:**
1. Fix the price.rs path.
2. Update the `ORIGINATOR` doc and, after checking with the Codex quota owner, the value.
3. Add the missing adapters to the table in inference/src/upstream/mod.rs.
4. Drop "frozen at phase 0" from route-contract/src/lib.rs.

### GW-22 Dependency version skew and hand-rolled encoders

**Severity:** low · **Category:** build · **Effort:** S

**Locations:** [model-access/Cargo.toml:24](../../../../crates/model-access/Cargo.toml), [model-access/src/connect.rs:40-70](../../../../crates/model-access/src/connect.rs#L40), [Cargo.toml](../../../../Cargo.toml)

**Evidence:** model-access/Cargo.toml:24 has `sha2 = "0.11.0"` while the rest of the area uses sha2 0.10. Cargo.lock also carries base64 x4, reqwest 0.12/0.13, hmac 0.12/0.13, tokio-tungstenite x3.

**Impact:** Duplicate crypto crates and bespoke encoders to review.

**Suggested action:**
1. Add common deps to `[workspace.dependencies]` and use `workspace = true` in member crates.
2. Align sha2 on one version.
3. Replace the hand-rolled base64url in connect.rs with `base64` `URL_SAFE_NO_PAD`, keeping the PKCE vector test.
4. Verify: `cargo tree -d` shows fewer duplicates for these crates.

### GW-23 Parallel string matches with unreachable!() in the live Stripe read path

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:** [card_funding.rs:266-292](../../../../crates/gateway/src/card_funding.rs#L266)

**Evidence:** The second match at 280-290 ends in `_ => unreachable!()` and relies on the earlier prefix match.

**Impact:** Adding a resource to one table but not the other panics a billing handler.

**Suggested action:**
1. Replace both matches with one const `RESOURCES` table of `(resource, prefix, object)` and an `Option` lookup.
2. Verify: a unit test iterates the table and resolves every entry.

### GW-24 Test-only scaffolding and lint workarounds in production code

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:** [serve.rs:250-263](../../../../crates/gateway/src/serve.rs#L250), [funding_tests.rs](../../../../crates/gateway/src/funding_tests.rs), [Cargo.toml:40](../../../../Cargo.toml)

**Evidence:** `open_with` runs `config.check` only when one of 7 listed features is set (250-263). funding_tests.rs has 5 `unimplemented!()` while workspace Cargo.toml:40 sets `unimplemented = "deny"` with no allow in gateway.

**Impact:** Config validation depends on a list that drifts; `clippy --all-targets` would fail on the crate.

**Suggested action:**
1. Always call `config.check` in `open_with`.
2. Replace `unimplemented!()` in funding_tests.rs with `Err(WalletError::...)` and rustfmt the file.
3. Verify: `cargo clippy -p gateway --all-targets` passes.

### GW-25 openrouter is a 2,098-line single file, and inference depends on it for a few items

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:** [openrouter/src/lib.rs](../../../../crates/openrouter/src/lib.rs), [inference/src/upstream/openrouter.rs:75-87](../../../../crates/inference/src/upstream/openrouter.rs#L75), [inference/src/rates.rs:51](../../../../crates/inference/src/rates.rs#L51)

**Evidence:** inference uses `openrouter::Config::from_env`, `KEY_VAR`, `BASE_URL` (upstream/openrouter.rs:75-87) and `openrouter::default_models()` (rates.rs:51).

**Impact:** Hard-to-navigate file; inference takes on more coupling than it needs.

**Suggested action:**
1. Split openrouter into `key`, `chat`, `embeddings`, `error`, `stream` and `tools` modules.
2. For inference, either keep the dependency (`default_models` is a real use) or move key handling and the model list into a light feature/module.
3. Verify: openrouter and inference tests pass unchanged.

## Refuted during verification

No reviewer finding was rejected outright. Several claims were corrected and should not be raised again in their original form:

- **GW-17:** The claim that the two Codex price paths disagree on long-context rates is wrong. `price::cost` uses short-context rates, the same as `codex_cost`; only `upper_bound` applies long-context rates. The only real difference is `-pro` suffix handling.
- **GW-25:** inference does not depend on openrouter for just three constants. It also uses `default_models()` (rates.rs:51), so dropping the dependency outright is not a valid fix.
- **GW-07:** The earnings routes cannot be mounted without earnings configured (serve.rs:757-758), so the `state.earnings.unwrap()` calls cannot panic today. The remaining risk is schema drift in the `Value` paths.
- **GW-03:** `take_free` already refuses when a save fails. Only `add_spend` and `give_back_free` drop errors, and in-memory caps hold until restart.
- **GW-04:** CORS misclassification is not exploitable today because the public preflight omits allow-headers and the gateway has no cookie auth.
- **Counts:** lib.rs has 41 modules, 35 of them `pub` (not 40/33). There are 58 `eprintln!` calls (not about 60).
