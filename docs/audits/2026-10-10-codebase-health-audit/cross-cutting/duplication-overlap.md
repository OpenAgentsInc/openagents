# Duplication & overlapping implementations

**Scope:** Repo-wide. This section covers parallel implementations of the same concept across crates. **Health grade: C.** Audit date 2026-10-10, snapshot commit `3168c986aa11e18a8bd30f52c270f609e49b3815`.

Duplication is the largest structural drag on the openagents Rust workspace. The workspace has 182 packages and about 2.34M lines of tracked Rust outside `crates/psionic`. A 10-line sliding-window scan found 4,167 shared code windows in the top 40 crate pairs alone. The worst pairs copy code from crates they already depend on: `coder-new/src/ui/models.rs` is byte-identical to `coder-demo-ui/src/ui/models.rs`. The pattern shows up at three levels:

1. **Basic utilities are rewritten per file.** Examples: 60 `unix_now`, 98 `hex`, 23 `write_atomic` variants, 16 private canonical-JSON functions, Hinnant civil-date math in 46 files, and 76 manual UTF-8 boundary loops.
2. **Wire contracts are restated per crate.** The chat-router enums exist in three copies, the System One types in four or five crates, and six crates hand-roll their own NIP-42 relay clients.
3. **Whole product loops run in parallel.** These are coder-one, microcoder-loop, the deprecated microluna, and five separate `claude -p --output-format` launchers.

Most copies still behave the same, so the main risk is drift. One copy has already drifted into a gap. `coder::router::redact` keeps its own secret-shape rules and misses the API-key shapes that `secret-screen` catches, and it runs on every chat turn in coder-worker.

The codebase already has shared homes for most of this: codex-transport, microcoder-loop, secret-screen, bitcoin-amount, route-contract::digest, oa-copy and markdown-stream. Most of the work is moving callers onto them. The rest is one small new leaf utility crate (time, hex, atomic write, clip, escape, random id, paths).

## Measurements

| Metric | Value |
|---|---|
| Workspace packages | 182 (184 crate dirs; psionic and openagents-mobile excluded) |
| Tracked `.rs` files / Rust LOC outside psionic | 4,461 / about 2,339,780 |
| Cross-crate duplicate 10-line windows (top 40 pairs) | 4,167 |
| Worst pairs | coder-demo-ui<->coder-new 1,278; coder-new<->coder-ui 832; kev<->laya 255; boat<->coder-cloud 221; receipts<->tenancy 103; coder<->gateway 103 |
| `fn unix_now(` / `fn now_ms(` | 60 / 20 |
| `duration_since(UNIX_EPOCH)` | 384 sites in 334 files |
| Hinnant civil-date constants (719468/146097) | 46 files |
| `fn hex(` / `format!("{:02x}")` / `fn sha256_hex(` | 98 / 347 sites in 297 files / 15 |
| `format!("sha256:{")` | 179 sites in 100 files |
| Private canonical-JSON functions / JCS implementations | 16 / 2 |
| `write_atomic` / `atomic_write` variants | 23 |
| `while !x.is_char_boundary` loops / `floor_char_boundary` uses | 76 in 69 files / 3 |
| HTML escape functions | about 14 |
| Retry: RetryPolicy-like structs / backoff fns / Retry-After parsers | 4 / 3 / 3 |
| Hand-rolled NIP-42 relay clients (bypassing nostr-transport) | 6 |
| `claude ... --output-format` argv builders | 6 sites (5 outside claude_agent_sdk); claude_agent_sdk has 1 dependent |
| Files referencing `total_cost_usd` | 44 (includes tests and fixtures) |
| OpenRouter base URL literals | 7 files |
| Files reading `CODEX_HOME` | 15 |
| `".openagents"` path literals | 75 in 64 files |
| `secp256k1::rand::` used as general RNG | 243 uses in 141 files |
| `[workspace.dependencies]` | only iroh, iroh-relay, iroh-mdns-address-lookup |
| sha2 manifest versions | 0.10 in 72 manifests, 0.11 in 13 (both in Cargo.lock) |
| getrandom manifest versions | 0.3 in 14 own crates, 0.4 in 2 |
| Cargo.lock | 1,567 packages, 144 with multiple versions (base64 x4, secp256k1 x4, rand x3) |
| Coder-family LOC | coder 262,296; coder-one 132,534 (0 dependents, last commit 2026-10-03); coder-new 46,026; microcoder 21,429; microcoder-loop 12,966; microluna 5,080 (deprecated; only dependent coder-one) |
| System One servers | kev 6,484; laya (serve.rs shares 122 long lines with kev); lev 13,573 (0 dependents, last commit 2026-09-24) |

## Strengths

- Nostr protocol logic is centralized. No crate outside `crates/nostr` and `crates/nostr-relay` computes event IDs or signs events itself.
- Extraction has worked when it was done. codex-transport was pulled out of microluna and re-exports the old paths. microcoder-loop was split out with no dependency on `crates/coder` (issue #9879). `crates/coder/src/task/usage.rs:47` re-exports `microcoder_loop::usage` instead of copying it.
- secret-screen was consolidated out of `gym-leaderboard::scrub` and is used by 8 crates. bitcoin-amount is a well-documented single home for amount display (BIP 177, no floats).
- `route-contract::digest` already provides a typed `Digest` plus `canonical`/`digest_of`, and 33 files use it. The consolidation target for canonical JSON exists.
- Deliberate duplication is often fenced with tests. For example, `crates/coder/src/router.rs:2179` pins `Screen::ALL` against `nostr::cj_conversation`, and the jargon list in `openagents-chat-app/src/eval_cards.rs` is held equal by a coder test.
- markdown-stream is shared by openagents-web, rust-native and coder-new, and oa-copy holds the machine-talk guard in one place used by 11 crates.
- Doc comments are excellent. Almost every duplicate states where it came from (for example "reimplemented from the public terminal renderer" or "this crate does not depend on coder, so..."), so consolidation is easy to plan.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-DUP-01 | medium | security | Chat-router secret redaction is a weaker copy of secret-screen and misses API-key shapes on the live worker path | S |
| X-DUP-02 | medium | duplication | Chat-router wire vocabulary is restated in coder, openagents-chat and nostr::cj_conversation | M |
| X-DUP-03 | medium | duplication | 16 private sorted-key canonical-JSON copies instead of route-contract::digest; 2 JCS implementations | M |
| X-DUP-04 | medium | duplication | System One request/question/refusal types re-declared in jev, kev, laya, lev | L |
| X-DUP-05 | medium | duplication | coder-new copies code from coder-demo-ui and coder-ui/demo, which it already depends on | L |
| X-DUP-06 | medium | architecture | Parallel coding loops and five hand-built Claude CLI launchers | L |
| X-DUP-07 | medium | duplication | Six hand-rolled NIP-42 relay WebSocket clients bypass nostr-transport | L |
| X-DUP-08 | medium | duplication | Time helpers rewritten everywhere: 60 unix_now, 20 now_ms, civil-date math in 46 files | M |
| X-DUP-09 | medium | build | No workspace dependency table; sha2 0.10 and 0.11 both used by first-party crates | M |
| X-DUP-10 | medium | duplication | Five Markdown renderers with different link-safety policies | L |
| X-DUP-11 | medium | duplication | Jargon BANNED list and matcher copied into three crates with different matching rules | S |
| X-DUP-12 | medium | duplication | Codex login and ~/.openagents path resolution re-implemented across crates | M |
| X-DUP-13 | medium | duplication | Several RetryPolicy types and ad-hoc backoff functions | M |
| X-DUP-14 | medium | duplication | Usage-log and open-lane quota modules copied between coder relay, gateway and eval-runner | M |
| X-DUP-15 | medium | duplication | Copy-pasted JSON stores across coder-environment-* crates; identical test HTTP harness in boat/coder-cloud | S |
| X-DUP-16 | low | duplication | Hex and sha256-hex helpers duplicated about 100 times; one `hex` actually hashes | M |
| X-DUP-17 | low | duplication | 23 write_atomic variants with inconsistent durability, permissions and temp naming | M |
| X-DUP-18 | low | duplication | OpenRouter base URL and key check duplicated across crates | S |
| X-DUP-19 | low | duplication | plugin-* WASM guests copy read_some/clip helpers that belong in plugin-pdk::guest | S |
| X-DUP-20 | low | maintainability | 76 hand-rolled UTF-8 boundary loops; floor_char_boundary used 3 times | S |
| X-DUP-21 | low | duplication | exact_msat money parsing copied between pay-host and openagents-desktop | S |
| X-DUP-22 | low | duplication | HTML escaping and random-ID helpers scattered | S |
| X-DUP-23 | low | maintainability | terminal-app ships a binary named openagents-terminal, while the crate openagents-terminal is a different product | S |
| X-DUP-24 | low | duplication | coder-history devin/opencode SQLite mirrors duplicate their bookkeeping | M |

### X-DUP-01 Chat-router secret redaction is a weaker parallel copy of secret-screen and misses API-key shapes on the live worker path

**Severity:** medium · **Category:** security · **Effort:** S

**Locations:**
- [router.rs:1623](../../../../crates/coder/src/router.rs), [router.rs:1629](../../../../crates/coder/src/router.rs), [router.rs:1658](../../../../crates/coder/src/router.rs) (`crates/coder/src/router.rs`)
- [coder-worker.rs:3004](../../../../crates/coder/src/bin/coder-worker.rs), [coder-worker.rs:3573](../../../../crates/coder/src/bin/coder-worker.rs)
- [secret-screen lib.rs:55](../../../../crates/secret-screen/src/lib.rs), [lib.rs:85](../../../../crates/secret-screen/src/lib.rs), [lib.rs:171](../../../../crates/secret-screen/src/lib.rs)
- [crates/coder/Cargo.toml:71](../../../../crates/coder/Cargo.toml)

**Evidence:**
- `router::redact` (router.rs:1629) is documented as replacing "bounded secret shapes": nsec, 64-hex, BOLT11, LNURL, Lightning address and on-chain address. Its private `secret_shape` (router.rs:1658-1693) recognizes only those shapes.
- coder already depends on secret-screen (Cargo.toml:71), but never calls it here. secret-screen has rules for `sk-ant-` (lib.rs:55), `AKIA`/`ASIA` (lib.rs:85) and `Bearer` (lib.rs:110), and a test for `ghp_` (lib.rs:533).
- coder-worker passes `router::redact(&turn.message)` into seam requests at 6 sites: lines 3004, 3020, 3036, 3049, 3067 and 3573.

**Impact:** Anthropic, GitHub, AWS or bearer tokens pasted into chat reach the personalize, ground and CLI seam model calls unredacted. Those seam calls are extra hops beyond the main turn. The two detectors will also keep drifting apart.

**Suggested action:**
1. Add the payment shapes from router.rs:1658-1693 to secret-screen as a named rule group.
2. Make `coder::router::redact` call `secret_screen::redact` and then apply the `MESSAGE_CHARS` cut. Delete `secret_shape`.
3. Check that secret-screen's `[redacted:kind]` markers are acceptable to the seams, or map them to `REDACTED`.
4. Verify: add a router test where a message containing an `sk-ant-` key, a `ghp_` PAT and a bolt11 invoice comes back with all three redacted.

Severity was lowered from high during verification. The raw message already reaches the worker and its main model turn, so this gap adds exposure only on the seam calls. It is still a broken documented promise.

### X-DUP-02 Chat-router wire vocabulary (Screen/RunsOn/Offer and Surface/Computer/Engine/Project/Context) is restated in coder, openagents-chat and nostr::cj_conversation

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- coder: [router.rs:563](../../../../crates/coder/src/router.rs), router.rs:1311, router.rs:1346, router.rs:1369, router.rs:1434, router.rs:2174
- openagents-chat: [router.rs:45](../../../../crates/openagents-chat/src/router.rs), router.rs:628, router.rs:695, router.rs:753
- nostr: [cj_conversation.rs:55](../../../../crates/nostr/src/cj_conversation.rs), cj_conversation.rs:120, cj_conversation.rs:164, cj_conversation.rs:252

**Evidence:**
- Screen, RunsOn and Offer are declared in all three files: coder 1369/1346/1434, openagents-chat 628/695/753, nostr 55/164/252.
- Effect is declared only in coder (1311) and nostr (120).
- Surface, Computer, Engine/EngineState, Project, RunEnding, CoderRun and Context are declared in coder and openagents-chat, but not in nostr.
- Some layering is deliberate. coder re-exports nostr's Engine as `CodingEngine` and Plan as `DispatchPlan` (router.rs:65,69), and converts its Offer through `Offer::cj()` (router.rs:1500). The identical enums, however, are not shared.
- The coder test at router.rs:2174-2179 checks that every coder Screen word parses in nostr's Screen and that the lengths match, which pins the Screen word sets equal. Nothing guards the coder/openagents-chat pair.

**Impact:** Adding or renaming a screen, offer or surface word takes two or three coordinated edits. The phone and the worker can disagree on the Surface, Computer and Context words, and no cross-crate test covers them.

**Suggested action:**
1. Move the pure wire enums (Screen, Effect, RunsOn, and the Surface/Computer/EngineState/RunEnding words) into `nostr::cj_conversation`.
2. `pub use` them from `coder::router` and `openagents_chat::router`.
3. Keep side-specific policy as free functions in each crate: the phone's READ_ONLY table and the worker's route policy.
4. Surface, Computer and Context are not yet in the protocol module. Either add them there, or add a round-trip test between coder and openagents-chat that covers every variant's word.
5. Verify: `rg 'enum (Screen|RunsOn|Offer)\b' crates` matches only `cj_conversation.rs`.

### X-DUP-03 About 16 private copies of the sorted-key canonical-JSON function across tenancy, gym, receipts and others, instead of route-contract::digest

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- Target: [route-contract digest.rs:68](../../../../crates/route-contract/src/digest.rs)
- tenancy: [accounts.rs:1945](../../../../crates/tenancy/src/accounts.rs), [backend.rs:576](../../../../crates/tenancy/src/backend.rs), [billing.rs:2190](../../../../crates/tenancy/src/billing.rs), [sessions.rs:1353](../../../../crates/tenancy/src/sessions.rs), [manifest.rs:315](../../../../crates/tenancy/src/manifest.rs)
- receipts: [execution.rs:468](../../../../crates/receipts/src/execution.rs), [export.rs:983](../../../../crates/receipts/src/export.rs)
- gym: [jobs.rs:1178](../../../../crates/gym/src/jobs.rs), [ab.rs:2030](../../../../crates/gym/src/ab.rs), [suite.rs:1177](../../../../crates/gym/src/suite.rs), [gate.rs:2205](../../../../crates/gym/src/gate.rs), [admission.rs:2113](../../../../crates/gym/src/admission.rs)
- [coder-scheduler catalog.rs:474](../../../../crates/coder-scheduler/src/catalog.rs), [lev suite.rs:92](../../../../crates/lev/src/suite.rs), [nostr decision.rs:1771](../../../../crates/nostr/src/decision.rs)
- JCS: [nostr contracts/json.rs:70](../../../../crates/nostr/src/contracts/json.rs), [x402 payment_scheme.rs:53](../../../../crates/x402/src/payment_scheme.rs)

**Evidence:**
- `route-contract::digest::canonical` (digest.rs:64-101) sorts keys by UTF-8 bytes and uses serde_json number encoding.
- `rg 'fn canonicali[sz]e\w*\('` outside psionic finds 16 private value canonicalizers: 5 in tenancy, 5 in gym, 2 in receipts, plus coder-scheduler, lev and nostr/decision. The 17th match is an unrelated method.
- There are two separate JCS implementations:
  - `nostr::contracts::json::jcs` (json.rs:70) is RFC 8785 and errors on non-finite numbers.
  - `x402::payment_scheme::jcs` (line 53) sorts keys by UTF-16, but formats numbers with `Value::to_string`, so it is not full RFC 8785 number formatting.

**Impact:** Cross-crate digest agreement depends on hand-kept copies staying byte-identical, and each new receipt type adds another copy. x402's JCS is incomplete and can disagree with nostr's on float formatting.

**Suggested action:**
1. Make `route-contract::digest` the single home for the sorted scheme, exposing `canonical` plus a `canonical_string` helper.
2. Replace the 16 private bodies, starting with the 5 in tenancy.
3. Make x402 call `nostr::contracts::json::jcs`, or move `jcs` into the digest module, and delete x402's copy.
4. Add golden-vector tests for both schemes that cover nested objects, non-ASCII keys and floats.
5. Verify: `rg 'fn canonicalize\(' crates` returns nothing outside the digest home.

Two schemes are justified in principle: JCS is a protocol requirement, and x402 uses it for base64url headers, not for `sha256:` digests. The real secondary problem is the incomplete duplicate JCS.

### X-DUP-04 System One (`POST /v1/systemone`) request/question/refusal types re-declared in jev, kev, laya, lev (and psionic Clef)

**Severity:** medium · **Category:** duplication · **Effort:** L

**Locations:**
- [jev questions.rs:197](../../../../crates/jev/src/questions.rs), [kev api.rs:21](../../../../crates/kev/src/api.rs), [lev api.rs:26](../../../../crates/lev/src/api.rs)
- [kev serve.rs](../../../../crates/kev/src/serve.rs), [laya serve.rs](../../../../crates/laya/src/serve.rs)
- [kev error.rs](../../../../crates/kev/src/error.rs), [laya error.rs](../../../../crates/laya/src/error.rs)

**Evidence:**
- Each crate declares the request and question types differently:
  - kev's Question uses `rename_all=lowercase` with `Noul{instructions: Value, criteria: Option<IndexMap<String,Value>>}` (kev/api.rs:19-30).
  - lev has `SystemOneRequest{model: Option<String>, extensions: Extensions}` plus `MIN/MAX_SCORE_LEVELS` (lev/api.rs:20-40).
  - jev has its own `Noul{instructions: Option<Entry>}` with `NoulCriteria` (questions.rs:126,197).
- `kev/serve.rs` (1,029 lines) and `laya/serve.rs` (773 lines) are parallel axum servers.
- `kev/error.rs` and `laya/error.rs` (218 vs 202 lines) differ by 82 diff lines. kev has an extra Memory budget variant and different refusal texts. Both doc comments say they share codes with `gym::eval::classify`.
- kev, laya and lev have no workspace dependents. Last commits: lev 2026-09-24, laya 2026-10-06, kev 2026-10-07.

**Impact:** A contract change has to land in four or five places. The refusal codes that `gym::eval::classify` depends on stay consistent only because of comments.

**Suggested action:**
1. Create a serde-only `crates/systemone-contract` holding the request, Question variants, answer and refusal envelope, RefusalCode, and score-level limits.
2. Have jev, kev, laya and lev use it. Keep model-specific errors (kev's Memory budget, laya's `head_max_len`) as extension codes.
3. Factor the shared axum request and refusal plumbing out of kev's and laya's `serve.rs` behind a trait.
4. Decide separately whether lev (0 dependents, idle since 2026-09-24) stays in the default workspace build.
5. Verify: add serde round-trip tests in systemone-contract, and confirm `gym::eval::classify` matches on the shared RefusalCode instead of string literals.

### X-DUP-05 coder-new copies code from coder-demo-ui and coder-ui/demo, both of which it already depends on

**Severity:** medium · **Category:** duplication · **Effort:** L

**Locations:**
- [coder-new/src/ui/models.rs](../../../../crates/coder-new/src/ui/models.rs), [coder-demo-ui/src/ui/models.rs](../../../../crates/coder-demo-ui/src/ui/models.rs)
- [coder-demo-ui/src/ui.rs:3](../../../../crates/coder-demo-ui/src/ui.rs)
- [coder-new/src/models.rs](../../../../crates/coder-new/src/models.rs), [coder-ui/src/demo/models.rs](../../../../crates/coder-ui/src/demo/models.rs)
- [coder-new/Cargo.toml:27](../../../../crates/coder-new/Cargo.toml), Cargo.toml:28; [coder-new/src/ui.rs:52](../../../../crates/coder-new/src/ui.rs)

**Evidence:**
- `diff crates/coder-new/src/ui/models.rs crates/coder-demo-ui/src/ui/models.rs` prints nothing. The two 358-line files are byte-identical.
- `coder-new/src/models.rs` and `coder-ui/src/demo/models.rs` differ by 9 diff lines.
- coder-new already depends on both coder-ui and coder-demo-ui (Cargo.toml:27-28), and `ui.rs:52` calls `coder_demo_ui::render`.
- Window scan: coder-demo-ui<->coder-new has 1,278 shared windows, and coder-new<->coder-ui has 832.

**Impact:** Fixes to the model picker, plugin manager and settings forms have to be repeated. The web components catalog built from coder-ui can drift from the terminal it mirrors.

**Suggested action:**
1. In `coder-demo-ui/src/ui.rs:3`, change `mod models;` to `pub mod models;`. It is currently private, so a re-export will not compile until this changes.
2. Replace `coder-new/src/ui/models.rs` with a re-export of `coder_demo_ui::ui::models`.
3. Consolidate the `models.rs` pair, then the plugins, settings and tools copies, one screen at a time. coder-ui should own the portable state machines, and coder-new should keep only the live adapters.
4. Verify: re-run the 10-line window scan after each screen and track the coder-new pair counts toward zero.

### X-DUP-06 Parallel coding loops (coder-one, microcoder-loop, deprecated microluna) and five hand-built Claude CLI launchers

**Severity:** medium · **Category:** architecture · **Effort:** L

**Locations:**
- [coder-one ask/executor.rs:197](../../../../crates/coder-one/src/ask/executor.rs)
- [microcoder-loop claude.rs:245](../../../../crates/microcoder-loop/src/claude.rs)
- [coder-new claude_print.rs:116](../../../../crates/coder-new/src/claude_print.rs)
- [coder-delegate delegate.rs:2219](../../../../crates/coder-delegate/src/delegate.rs)
- [openagents-cli eval_engine.rs:222](../../../../crates/openagents-cli/src/eval_engine.rs)
- [claude_agent_sdk options.rs:587](../../../../crates/claude_agent_sdk/src/options.rs)
- [microluna/README.md](../../../../crates/microluna/README.md), [coder-one/Cargo.toml](../../../../crates/coder-one/Cargo.toml)

**Evidence:**
- `rg '"--output-format"'` outside psionic finds argv construction at all six sites listed above.
- claude_agent_sdk is a dependency of microcoder only.
- coder-one has 132,534 tracked Rust lines and no workspace dependents. Its last commit was 2026-10-03, and it is microluna's only dependent.
- `microluna/README.md` says microluna was deprecated on 2026-09-28 and that microcoder-loop is Coder's loop.
- 44 files reference `total_cost_usd`, though many of them are tests and fixtures.

**Impact:** A Claude CLI flag change or stream-json parsing fix lands in one launcher and not the others. coder-one, plus the deprecated microluna, keeps 132k LOC in every workspace build with no in-workspace caller.

**Suggested action:**
1. Remove coder-one (and with it microluna) from default workspace builds, either with a `default-members` list that omits them or by excluding them as `crates/psionic` is. Record in coder-one's README what it is kept for.
2. Make claude_agent_sdk the only place that builds `claude -p --output-format stream-json` argv and parses result events. Expose a typed result that carries `total_cost_usd` and usage.
3. Migrate microcoder-loop, coder-delegate, coder-new and openagents-cli to it.
4. Verify: `rg '"--output-format"' crates` matches only claude_agent_sdk, and `cargo build` at the workspace root no longer compiles coder-one.

### X-DUP-07 Six hand-rolled NIP-42 relay WebSocket clients bypass nostr-transport

**Severity:** medium · **Category:** duplication · **Effort:** L

**Locations:**
- Target: [nostr-transport lib.rs:44](../../../../crates/nostr-transport/src/lib.rs)
- [coder relay.rs:360](../../../../crates/coder/src/relay.rs)
- [gateway relay_worker.rs:2087](../../../../crates/gateway/src/relay_worker.rs), [gateway advertise.rs:237](../../../../crates/gateway/src/advertise.rs)
- [voyager guild.rs](../../../../crates/voyager/src/guild.rs), [gym-leaderboard relay.rs](../../../../crates/gym-leaderboard/src/relay.rs)
- [verse-net net/relay.rs:149](../../../../crates/verse-net/src/net/relay.rs)

**Evidence:**
- nostr-transport exposes `connect`, `connect_as`, `connect_open`, `send`, `next`, `close` and `with_frame_budget` (lib.rs:44-186), and 13 manifests declare it.
- Each of the six files below handles `"AUTH"` itself and has 0 nostr_transport references:

| File | Lines | Connection |
|---|---|---|
| coder/relay.rs | 1,559 | plain `connect_async` at :360 |
| gateway/relay_worker.rs | 2,156 | plain `connect_async` at :2087 |
| gateway/advertise.rs | 313 | plain `connect_async` at :237 |
| voyager/guild.rs | 339 | own handling |
| gym-leaderboard/relay.rs | 179 | own handling |
| verse-net/net/relay.rs | 251 | custom rustls connector and `WebSocketConfig` at :149 |

**Impact:** AUTH handling, frame bounds and reconnect behavior are implemented six times. The transport bounds that nostr-transport enforces do not apply to the plain `connect_async` callers.

**Suggested action:**
1. Port the short one-shot clients to `nostr_transport::connect_as` first: gym-leaderboard/relay.rs, gateway/advertise.rs and voyager/guild.rs.
2. Add a TLS-connector option and REQ/CLOSE helpers to nostr-transport.
3. Migrate verse-net's native worker and the long-lived coder and gateway relay workers.
4. Verify: `rg -l '"AUTH"' crates` matches only the nostr, nostr-relay and nostr-transport crates.

### X-DUP-08 Time helpers re-written everywhere: 60 unix_now, 20 now_ms, Hinnant civil-date math in 46 files

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder relay.rs:1294](../../../../crates/coder/src/relay.rs), [coder runstate.rs:1022](../../../../crates/coder/src/runstate.rs), [coder relay/usage.rs:394](../../../../crates/coder/src/relay/usage.rs)
- [gateway decision_usage.rs:116](../../../../crates/gateway/src/decision_usage.rs)
- [x402 payment_scheme.rs:79](../../../../crates/x402/src/payment_scheme.rs)
- [tenancy quota.rs:757](../../../../crates/tenancy/src/quota.rs)
- [coder/Cargo.toml:64](../../../../crates/coder/Cargo.toml)

**Evidence:** Outside psionic there are:
- 60 `fn unix_now(` definitions in 60 files and 20 `fn now_ms(` definitions.
- The Hinnant constant `719_468` in 46 files.
- A separate rfc3339 helper in x402 (payment_scheme.rs:78-80).

coder already depends on jiff 0.2.37 (Cargo.toml:64).

**Impact:** Date bugs, such as epoch signedness or panicking instead of defaulting on a clock error, get fixed one copy at a time. Functions with the same name have different return types.

**Suggested action:**
1. Create a leaf crate `crates/oa-time`, either std-only or jiff-backed, with `unix_secs`, `unix_millis`, `rfc3339_secs`, `parse_rfc3339` and `utc_day`. Test it against jiff.
2. Migrate coder (9 copies) and tenancy (5 copies) first, then the rest.
3. Verify: `rg -l '719_?468' crates` matches only oa-time, and `rg 'fn unix_now\(' crates` trends to zero.

### X-DUP-09 No workspace dependency table: sha2 0.10 and 0.11 both used by first-party crates

**Severity:** medium · **Category:** build · **Effort:** M

**Locations:** [Cargo.toml:15](../../../../Cargo.toml), [Cargo.lock](../../../../Cargo.lock)

**Evidence:**
- `[workspace.dependencies]` (Cargo.toml:15-21) holds only iroh, iroh-relay and iroh-mdns-address-lookup.
- 72 crate manifests declare sha2 `"0.10"` and 13 declare `"0.11"`.
- Cargo.lock contains both sha2 0.10.9 and 0.11.0, and 1,567 packages in total.

**Impact:** Two hashing stacks are compiled, builds take longer, and sha2 types cannot be shared across crate APIs.

**Suggested action:**
1. Add `[workspace.dependencies]` entries for the common set: serde, serde_json, tokio, sha2, hex, base64, getrandom, secp256k1, reqwest, thiserror, tokio-tungstenite and indexmap.
2. Convert the manifests to `workspace = true`.
3. Move the 13 sha2 0.11 users back to 0.10. Third-party crates probably keep 0.10 in the lock anyway, so standardizing on 0.11 would not remove it.
4. Add a cargo-deny `multiple-versions` warning for crates that first-party manifests declare.
5. Verify: `rg 'sha2 = "0.11"' crates` returns nothing, and `cargo tree -d -e normal | rg sha2` shows a single version from first-party paths.

### X-DUP-10 Five Markdown renderers with divergent link-safety policy; coder-ui is an explicit reimplementation

**Severity:** medium · **Category:** duplication · **Effort:** L

**Locations:**
- [coder-terminal/src/markdown.rs](../../../../crates/coder-terminal/src/markdown.rs)
- [coder-ui components/markdown.rs:1](../../../../crates/coder-ui/src/components/markdown.rs)
- [coder-terminal/Cargo.toml:13](../../../../crates/coder-terminal/Cargo.toml)
- [rust-native markdown.rs:213](../../../../crates/rust-native/src/markdown.rs)
- [openagents-web markdown.rs:1](../../../../crates/openagents-web/src/markdown.rs), markdown.rs:128

**Evidence:**
- `coder-ui/components/markdown.rs:1` says "Selectable Markdown rows, reimplemented from the public terminal renderer", even though coder-terminal depends on coder-ui (Cargo.toml:13).
- The link policies differ:
  - rust-native's `opens()` admits only `https://` (markdown.rs:210-217).
  - openagents-web keeps http, https, mailto, site paths and anchors (doc lines 5-7). Its options also omit `ENABLE_TASKLISTS` (:128).
- markdown_stream is used by rust-native and openagents-web, but not by coder-terminal.

**Impact:** The same reply renders and links differently on each surface, and a link-safety fix has to be applied several times.

**Suggested action:**
1. Extract one shared `link_policy(destination)` and call it from every renderer.
2. Make coder-ui and coder-terminal share one block-tree parse. `rust-native::markdown` is a candidate base.
3. Verify: add a cross-renderer fixture test for `javascript:`, `http:`, `mailto:` and site paths that asserts the same decision from every renderer.

### X-DUP-11 Jargon BANNED word list and matcher copied into three crates with different matching rules

**Severity:** medium · **Category:** duplication · **Effort:** S

**Locations:**
- [coder router/gym.rs:907](../../../../crates/coder/src/router/gym.rs), gym.rs:966
- [openagents-chat-app eval_cards.rs:30](../../../../crates/openagents-chat-app/src/eval_cards.rs)
- [openagents-desktop words.rs:18](../../../../crates/openagents-desktop/src/words.rs)
- [oa-copy lib.rs:18](../../../../crates/oa-copy/src/lib.rs)

**Evidence:**
- `pub const BANNED` appears in exactly 3 files with the same leading words. coder's doc says "The phone keeps the same list ...; a test holds them equal".
- The matchers differ:
  - coder has `jargon_all` (gym.rs:966) and `jargon` (gym.rs:993).
  - desktop has its own `jargon` (words.rs:90) that matches whole words plus a plural s.
  - eval_cards documents exact whole-word matching.
- oa-copy has a separate `TERMS` list (lib.rs:18) for the related machine-talk rule.

**Impact:** One product rule lives in three lists with up to three matchers. A word added on one surface can still leak on another.

**Suggested action:**
1. Move BANNED, NAMES and one plural-aware matcher into oa-copy as an `oa_copy::jargon` module.
2. Re-export it from coder, openagents-chat-app and openagents-desktop, and delete the local lists and matchers.
3. Delete the equality test, which becomes redundant.
4. Verify: `rg 'pub const BANNED' crates` matches only oa-copy.

### X-DUP-12 Codex login and ~/.openagents path resolution re-implemented across crates

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- Target: [codex-transport codex.rs:53](../../../../crates/codex-transport/src/codex.rs)
- [microcoder-loop account.rs](../../../../crates/microcoder-loop/src/account.rs), [coder-delegate delegate.rs](../../../../crates/coder-delegate/src/delegate.rs), [coder-cloud boat_backend.rs](../../../../crates/coder-cloud/src/boat_backend.rs), [openagents-desktop model.rs](../../../../crates/openagents-desktop/src/model.rs)
- [model-access store.rs:67](../../../../crates/model-access/src/store.rs), [jev-hosted lib.rs:429](../../../../crates/jev-hosted/src/lib.rs), [coder-delegate credentials.rs:157](../../../../crates/coder-delegate/src/credentials.rs)
- [openagents-cli ext_eval.rs:267](../../../../crates/openagents-cli/src/ext_eval.rs), [ext-eval child.rs:75](../../../../crates/ext-eval/src/child.rs)

**Evidence:**
- 15 files read `"CODEX_HOME"`. Outside codex-transport and tests, they are:
  - coder-new: fleet.rs, codex_usage.rs
  - coder-one: capture.rs, capabilities.rs
  - coder-cloud: boat_backend.rs
  - microcoder-loop: account.rs
  - openagents-desktop: model.rs
  - coder: delegate_door.rs, task/shadow.rs
  - openagents-cli: chat_boat.rs
  - coder-delegate: delegate.rs
- Three `pub fn openagents_dir()` copies exist: model-access store.rs:67, coder-delegate credentials.rs:157 and jev-hosted lib.rs:429.
- `OPENAGENTS_HOME` is read only by openagents-cli ext_eval.rs:267, but ext-eval child.rs:75 sets it for child processes.

**Impact:** Crates can disagree about where credentials live. `OPENAGENTS_HOME` looks supported but most crates ignore it.

**Suggested action:**
1. Route every Codex home lookup through `codex_transport::Login::home()` / `default_path()`.
2. Add one `openagents_home()` to a leaf crate that checks `OPENAGENTS_HOME` first, then `HOME/.openagents`.
3. Replace the three `openagents_dir` copies and the literal `".openagents"` joins (75 literals in 64 files).
4. Verify: `rg '"CODEX_HOME"' crates` matches only codex-transport and tests, and a test sets `OPENAGENTS_HOME` and checks that model-access, coder-delegate and jev-hosted all resolve to it.

### X-DUP-13 Retry/backoff logic re-implemented: several RetryPolicy types and ad-hoc backoff functions

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [jev retry.rs:45](../../../../crates/jev/src/retry.rs)
- [boat client.rs:52](../../../../crates/boat/src/client.rs)
- [openagents-chat client.rs:3561](../../../../crates/openagents-chat/src/client.rs)
- [coder-delegate git_fetch.rs:79](../../../../crates/coder-delegate/src/git_fetch.rs)
- [nostr-relay gateway/push/mod.rs:65](../../../../crates/nostr-relay/src/gateway/push/mod.rs)

**Evidence:**
- `jev::RetryPolicy` (retry.rs:45) and `boat::RetryPolicy` (client.rs:52) are separate types. boat's doubles `base_delay` and adds jitter.
- openagents-chat's backoff (client.rs:3561) doubles from `OFFLINE_FIRST` with no jitter.
- coder-delegate's git_fetch.rs:79 takes its jitter from `SystemTime` subsec_nanos.
- The workspace has 4 RetryPolicy-like structs, 3 backoff functions and 3 Retry-After parsers.

**Impact:** Jitter and Retry-After handling are inconsistent. openagents-chat clients have no jitter, so they all reconnect at the same moments after an outage.

**Suggested action:**
1. Extract jev's RetryPolicy delay computation and a single Retry-After parser into a transport-free helper.
2. Use it from boat, openagents-chat (which gains jitter) and coder-delegate.
3. Verify: unit-test the delay sequence bounds and Retry-After parsing (seconds and HTTP-date). `rg 'struct RetryPolicy' crates` should match one definition.

### X-DUP-14 Usage-log and open-lane quota modules copied between coder relay, gateway and eval-runner

**Severity:** medium · **Category:** duplication · **Effort:** M

**Locations:**
- [coder relay/usage.rs:1](../../../../crates/coder/src/relay/usage.rs)
- [gateway decision_usage.rs:1](../../../../crates/gateway/src/decision_usage.rs)
- [eval-runner usage.rs:1](../../../../crates/eval-runner/src/usage.rs)
- [gateway open_quota.rs:9](../../../../crates/gateway/src/open_quota.rs)

**Evidence:**
- All three usage modules open with the same doc ("...usage log: one JSON line per job, so usage stats are one command away") and the #10120 decision text.
- open_quota.rs:9-11 says: "This is the chat worker's design (coder::relay::quota; this crate does not depend on coder, so the same design is kept here)".

**Impact:** Changing the record schema or rotation takes three edits, and each copy has its own date math.

**Suggested action:**
1. Create a small `crates/usage-log` with a generic daily JSONL writer and reader, plus the open-lane brake.
2. Keep the record types local to each worker.
3. Verify: re-run the window scan on the coder<->gateway pair (103 windows) and confirm the usage and quota windows are gone.

### X-DUP-15 Copy-pasted JSON stores across coder-environment-* crates and an identical test HTTP harness in boat/coder-cloud

**Severity:** medium · **Category:** duplication · **Effort:** S

**Locations:**
- [coder-environment store.rs](../../../../crates/coder-environment/src/store.rs), [coder-environment-build store.rs](../../../../crates/coder-environment-build/src/store.rs), [coder-environment-verify store.rs](../../../../crates/coder-environment-verify/src/store.rs), [coder-environment-setup store.rs](../../../../crates/coder-environment-setup/src/store.rs), [coder-working-computer store.rs](../../../../crates/coder-working-computer/src/store.rs)
- [boat tests/support.rs:138](../../../../crates/boat/tests/support.rs), [coder-cloud tests/support.rs:138](../../../../crates/coder-cloud/tests/support.rs)

**Evidence:**
- The five `store.rs` files total 1,122 lines. The build-vs-verify diff is 67 lines, all noun or type swaps (`BuildJob` vs `VerifyJob`, "build job" vs "verify job").
- All four sibling crates depend on coder-environment.
- `boat/tests/support.rs` and `coder-cloud/tests/support.rs` are both 325 lines. They differ only at line 138, where the accept timeout is 500ms in one and 30s in the other.

**Impact:** Store durability and limit fixes have to be repeated five times, and the test harness copies have already drifted.

**Suggested action:**
1. Add a generic `coder_environment::store::JsonRecordStore<T>` parameterized by noun and limit, and replace the five copies.
2. Move the fake HTTP server into a `publish = false` dev crate with a configurable accept timeout, and use it from boat and coder-cloud.
3. Verify: the existing store tests pass against the generic store, and the boat<->coder-cloud window count (221) drops.

### X-DUP-16 Hex and sha256-hex helpers duplicated about 100 times; one `hex` actually hashes

**Severity:** low · **Category:** duplication · **Effort:** M

**Locations:**
- [coder-one checks/behavior.rs:855](../../../../crates/coder-one/src/checks/behavior.rs)
- [coder-reach lib.rs:166](../../../../crates/coder-reach/src/lib.rs)
- [openagents-chat client.rs:3578](../../../../crates/openagents-chat/src/client.rs), [openagents-chat api.rs:191](../../../../crates/openagents-chat/src/api.rs)

**Evidence:**
- Outside psionic there are 98 `fn hex(` definitions in 98 files and 15 `fn sha256_hex(` definitions.
- `fn hex(bytes)` at coder-one behavior.rs:855 returns the hex of the input's SHA-256 digest, not the hex of the input.
- openagents-chat has both a `hex` (client.rs:3578) and a separate `digest_hex` (api.rs:191).

**Impact:** Review noise and misleading names. Allocating one `format!` per byte in digest paths is a minor cost.

**Suggested action:**
1. Add `hex` to `[workspace.dependencies]` (see X-DUP-09) and mechanically replace the `fn hex(bytes)` bodies with `hex::encode`.
2. Put `sha256_hex` and a `sha256:`-tagged helper next to `route-contract::digest`.
3. Rename coder-one behavior.rs:855 to `sha256_hex`.
4. Verify: `rg 'fn hex\(' crates` trends to zero.

### X-DUP-17 23 write_atomic variants with inconsistent durability, permissions and temp naming

**Severity:** low · **Category:** duplication · **Effort:** M

**Locations:**
- [openagents-chat api.rs:182](../../../../crates/openagents-chat/src/api.rs)
- [coder-service fsx.rs:150](../../../../crates/coder-service/src/fsx.rs)
- [coder-history devin.rs:636](../../../../crates/coder-history/src/devin.rs), [coder-history opencode.rs:636](../../../../crates/coder-history/src/opencode.rs)
- [eval-runner lib.rs:103](../../../../crates/eval-runner/src/lib.rs)

**Evidence:**
- `rg 'fn (write_atomic|atomic_write)\w*\('` outside psionic finds 23 definitions.
- openagents-chat's api.rs:182-189 writes to `path.with_extension("tmp")` with a plain `fs::write` and then renames, with no fsync and default permissions.
- The `write_atomic` bodies in coder-history's devin.rs and opencode.rs are byte-identical.

**Impact:** Durability and file permissions depend on which copy a crate happened to pick.

**Suggested action:**
1. Promote `coder-service::fsx::atomic_write` into a shared leaf crate and add an fsync option. It already uses a unique temp name, `create_new`, `O_NOFOLLOW` and an explicit mode.
2. Replace the copies, starting with coder-history and openagents-chat.
3. Verify: `rg 'fn (write_atomic|atomic_write)' crates` matches only the leaf crate. A test should check that the file mode is preserved and that no `.tmp` file is left behind after an error.

### X-DUP-18 OpenRouter base URL and key-check duplicated across crates

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:**
- [openrouter lib.rs:48](../../../../crates/openrouter/src/lib.rs)
- [coder-new provider.rs:83](../../../../crates/coder-new/src/provider.rs), [coder-new model_catalog.rs:14](../../../../crates/coder-new/src/model_catalog.rs)
- [microluna openrouter.rs:23](../../../../crates/microluna/src/openrouter.rs)
- [model-access check.rs:30](../../../../crates/model-access/src/check.rs)

**Evidence:**
- 7 files define `"https://openrouter.ai/api/v1"`.
- coder-new's provider.rs:83-95 performs its own `GET {base}/key`.
- model-access's check.rs:30 is a transport-free, multi-provider request builder that hardcodes `https://openrouter.ai/api/v1/key`.

**Impact:** An endpoint change has to be found and made in several crates.

**Suggested action:**
1. Add `check_key()` and `list_models()` to the openrouter crate and use them from coder-new.
2. Have model-access's request builder reference `openrouter::BASE_URL`.
3. Leave the coder-ui fixture strings and the deprecated microluna alone.
4. Verify: `rg 'openrouter.ai/api/v1' crates` matches only the openrouter crate, coder-ui fixtures and microluna.

### X-DUP-19 plugin-* WASM guests copy read_some/clip helpers that belong in plugin-pdk::guest

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:**
- Target: [plugin-pdk guest.rs](../../../../crates/plugin-pdk/src/guest.rs)
- [plugin-action-items lib.rs:400](../../../../crates/plugin-action-items/src/lib.rs)
- [plugin-release-notes lib.rs:255](../../../../crates/plugin-release-notes/src/lib.rs)
- [plugin-explain-error lib.rs:309](../../../../crates/plugin-explain-error/src/lib.rs)
- [plugin-dependency-check lib.rs:393](../../../../crates/plugin-dependency-check/src/lib.rs)

**Evidence:** `fn read_some(` is defined in exactly 4 plugin crates. plugin-pdk already has a 693-line `guest.rs` module behind the `guest` feature (lib.rs:10).

**Impact:** Every new plugin copies the chunked-read loop.

**Suggested action:**
1. Move `read_some` and `clip` into `plugin-pdk/src/guest.rs` as public helpers with unit tests.
2. Delete the 4 copies.
3. Verify: `rg 'fn read_some\(' crates` matches only plugin-pdk, and the four plugins still build for the wasm target.

### X-DUP-20 76 hand-rolled UTF-8 boundary loops; str::floor_char_boundary used 3 times

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:** [Cargo.toml:28](../../../../Cargo.toml), [coder claim.rs:720](../../../../crates/coder/src/claim.rs)

**Evidence:** `rg 'while !\w+\.is_char_boundary'` finds 76 sites in 69 files, and `floor_char_boundary` appears 3 times. coder's claim.rs:720 is a typical clip: a byte max, a boundary walk-back, and an appended ellipsis.

**Impact:** Code noise, plus mixed semantics: helpers with the same name use a byte-unit "max" in some places and a char-unit one in others.

**Suggested action:**
1. Add `clip_bytes` (built on `floor_char_boundary`) and `clip_chars` to the shared leaf crate.
2. Replace the copies, starting with coder.
3. Verify: the loop count from `rg 'while !\w+\.is_char_boundary' crates` trends to zero. Unit-test multi-byte input at the cut point.

### X-DUP-21 exact_msat money parsing copied between pay-host and openagents-desktop instead of bitcoin-amount

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:**
- [pay-host lib.rs:72](../../../../crates/pay-host/src/lib.rs)
- [openagents-desktop route_live.rs:111](../../../../crates/openagents-desktop/src/route_live.rs)
- [bitcoin-amount lib.rs:91](../../../../crates/bitcoin-amount/src/lib.rs)

**Evidence:**
- Both files define `fn exact_msat(text: &str) -> Option<u64>` with the same negative-rejection start, and both use it in a hand-written `Deserialize`.
- The desktop copy has an extra "Fractional amount too large to be exact" check (route_live.rs:100-105), so the two have already diverged.
- bitcoin-amount exposes only show and label helpers, with no exact parser.

**Impact:** There are two sets of exactness rules for money amounts, and they already differ slightly.

**Suggested action:**
1. Add `bitcoin_amount::parse_exact_msat` with property tests, plus an optional serde `ExactMsat` newtype that includes the desktop's extra check.
2. Replace both copies.
3. Verify: `rg 'fn exact_msat' crates` matches only bitcoin-amount.

### X-DUP-22 HTML escaping and random-ID helpers scattered

**Severity:** low · **Category:** duplication · **Effort:** S

**Locations:**
- [openagents-web layout.rs:27](../../../../crates/openagents-web/src/layout.rs)
- [gateway dashboard.rs:70](../../../../crates/gateway/src/dashboard.rs)
- [coder-new memory.rs:851](../../../../crates/coder-new/src/memory.rs)
- [openagents-chat client.rs:3574](../../../../crates/openagents-chat/src/client.rs)
- [coder-reach lib.rs:162](../../../../crates/coder-reach/src/lib.rs)

**Evidence:**
- openagents-web's `escape` (layout.rs:27) and gateway's `esc` (dashboard.rs:70, chained `replace` calls) are independent escapers. There are about 14 HTML escape functions in total.
- coder-new's memory.rs:851-855 falls back to timestamp bytes when getrandom fails.
- openagents-chat's `new_id` uses `secp256k1::rand::random` (client.rs:3574). `secp256k1::rand::` is used 243 times in 141 files.

**Impact:** Low. Escapers that drift apart could become a surface-specific XSS, and coder-new's ID fallback weakens uniqueness.

**Suggested action:**
1. Remove coder-new's timestamp fallback first.
2. Add `escape_html` and `random_id` (getrandom-backed, with no fallback) to the shared leaf crate, and migrate the escapers and ID generators.
3. Verify: a shared escaper test covers `<>&"'`, and `rg 'fn (esc|escape|escape_html)\(' crates` trends to one definition.

### X-DUP-23 Terminal crate naming collision: terminal-app ships a binary named openagents-terminal while crate openagents-terminal is a different product

**Severity:** low · **Category:** maintainability · **Effort:** S

**Locations:** [terminal-app/Cargo.toml](../../../../crates/terminal-app/Cargo.toml), [openagents-terminal/Cargo.toml:11](../../../../crates/openagents-terminal/Cargo.toml)

**Evidence:**
- terminal-app's `[[bin]]` is named `"openagents-terminal"` and is described as "the shared smart terminal in a native window".
- The crate openagents-terminal has no bin target and describes itself as "a full-screen chat ... drawn with coder-terminal".
- There are 12 terminal-family crate dirs. `cargo metadata` shows no bin-vs-bin collisions, so this is a crate-vs-bin naming mismatch, not a cargo collision.

**Impact:** The shared name causes confusion in docs, in issue routing and with `cargo run`.

**Suggested action:**
1. Rename the crate openagents-terminal to `openagents-chat-tui`, or rename terminal-app's bin.
2. Document the terminal crate map in one place.
3. Folding the terminal micro-crates together is optional and weakly evidenced, so do not prioritize it.
4. Verify: `cargo metadata` lists no package whose name equals another package's bin name.

### X-DUP-24 coder-history devin/opencode SQLite mirrors duplicate their bookkeeping

**Severity:** low · **Category:** duplication · **Effort:** M

**Locations:** [coder-history devin.rs:636](../../../../crates/coder-history/src/devin.rs), [coder-history opencode.rs:636](../../../../crates/coder-history/src/opencode.rs)

**Evidence:** The `write_atomic` bodies, extracted with sed, are byte-identical between devin.rs and opencode.rs. The files are 659 and 658 lines, with parallel function names.

**Impact:** A third harness mirror would add a third copy of the state, index and atomic-write logic.

**Suggested action:**
1. Extract a shared mirror module: a `SessionSource` trait plus the state, index, `write_atomic` and `touch` helpers.
2. Reduce devin.rs and opencode.rs to SQL row adapters.
3. Verify: existing coder-history tests pass, and a diff of the two adapters shows only source-specific SQL and row mapping.

## Refuted during verification

- **openagents-chat `write_atomic` races between concurrent HTTP requests.** Rejected because the Api write methods take `&mut self` (`put_offer` at api.rs:260, `stop(api: &mut Api, ...)` at :606). Access within one process is serialized, so two concurrent requests cannot share the fixed tmp name. Only the durability inconsistency is kept, under X-DUP-17.
- **kev/error.rs and laya/error.rs differ only in doc comments.** Rejected because the diff shows 82 differing lines, including a kev-only Memory budget variant and different refusal variants and texts. X-DUP-04 is kept with this corrected.
- **openagents-mobile should depend on coder-mobile for the shared Computers/Terminal action types.** Rejected because this is already done: openagents-mobile path-depends on coder-mobile (Cargo.toml:42) and imports `coder_computers` types (app.rs:12-14).
- **Count corrections.** These were re-measured: 16 private canonicalizers (not about 25), 60 `unix_now` (not 61), 72 sha2 0.10 manifests (not 73), 23 `write_atomic` variants (not 22), and 44 `total_cost_usd` files (not 17; the higher count includes fixtures).
