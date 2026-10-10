# Workspace policy conformance

**Scope:** a repo-wide check of the workspace rules. These are semantic (not keyword) routing, Bitcoin-only payment rails, no Chat/Code mode switches, no machine talk or dead controls in user-visible text, and INVARIANTS.md claims compared with what is actually enforced.
**Health grade: C**. Audit date 2026-10-10, snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`.

Conformance is uneven. The paths the owner has reviewed closely hold up. The chat router (`crates/coder/src/router.rs`) routes only on typed Jev Choice/Noul readings, and the product knowledge base retrieves by embedding and cosine similarity. Bitcoin-only is enforced in code: the Spark wallet refuses token payments, x402 serves only `lnbtc`, mainnet is fixed in code, and the payments docs were brought in line on 2026-10-09. Thirteen product crates run the oa-copy machine-talk guard, and every allowlist entry gives a reason. Every one of the 381 rows in INVARIANTS.md names a check.

The violations are in less-watched paths. The crew agent (`crates/coder/src/task/agent_host*.rs`) still routes the owner's free text by substring. One of those routes runs a real merge: any message containing "merge" plus a common phrase such as "for me" or "do it" merges her waiting change. A "remember ..." prefix silently turns a work request into a note. The coder-one ask picks a Gym retrieval probe by keyword, even though a Jev question set sits a few lines away. Web and wasm surfaces outside the guard, and the crew agent's replies, show words AGENTS.md bans ("admitted", "retained", "admission", "context digest"). The website's /stats page and the desktop show "sats", which breaks the BIP 177 amounts rule. INVARIANTS.md has drifted: 11 named tests were deleted without their rows being updated, and nothing checks the citations. The file has grown to about 378 KB, with rows up to 7,072 characters. In short, the scaffolding is good but only partly enforced.

## Measurements

| Metric | Value |
|---|---|
| Tracked files / crates | 38,825 / 184 |
| oa-copy guard | 639 LOC, 54 terms, 11 phrases; 13 product crates depend on it |
| User-facing crates without the guard | coder-browser-web (2,453 LOC), everglade-web (3,809), coder-cloud-web (845), coder-chat-web (990), openagents-connect (5,950), openagents-terminal (9,463), rust-native-desktop (15,471), openagents-cli (89,408) |
| Files scanned by the guard in `crates/coder` | 2 (`cli.rs`, `task/cli.rs`) |
| Separate jargon lexicons | 4 (oa-copy `TERMS`, chat-app `eval_cards::BANNED`, desktop `words.rs`, coder-new `PLUMBING`) |
| `const *_WORDS/CUES/PHRASES/MARKERS/VERBS: &[&str]` outside oa-copy | 28 |
| Confirmed ad hoc routing of user text | 4 functions on the crew path (`asks_where`, `asks_to_merge`, `remembered`, `proposed_preference`) plus coder-one `asks_strategy` |
| INVARIANTS.md | 649 lines, ~378 KB, 43 sections, 381 table rows (median 724 chars, max 7,072, 46 rows > 2,000 chars) |
| Commits touching INVARIANTS.md since 2026-09-10 | 248 |
| Test-function names cited in INVARIANTS.md | 1,438; 11 no longer exist (removed in d65db8f209, 3f7ec6f9c6, 95d447a4b5, abdebf0ba6, e2247fd810, 7b8cfce37c, 0831075e84, ddab7b9a6e) |
| Automated check of ledger citations | none |
| Per-crate INVARIANTS.md files | 0 |
| USDC / EVM / Solana / Tempo code paths in crates | 0 (1 reserved, unconstructed Cashu enum variant) |
| `bitcoin-amount` users | openagents-cli, spark-wallet, openagents-mobile (not openagents-web or openagents-desktop) |

## Strengths

- The chat router is typed end to end. The `router.rs` module doc says "Nothing here is keyword matching: every routing reading is a Choice answer's argmax or a Noul's probability", and INVARIANTS.md rows 153 and 231 pin this with named tests.
- Product knowledge retrieval is semantic. `crates/coder/src/product_kb.rs` embeds the message and ranks by cosine similarity. `crates/knowledge/src/search.rs` combines BM25 with cosine under an absolute floor (`MIN_SEMANTIC_SIMILARITY = 0.4`).
- Bitcoin-only is enforced in code. `crates/spark-wallet/src/spark.rs:260` refuses any prepared payment that carries a `token_identifier` or a conversion. The x402 router speaks only `lnbtc`, and the wallet's mainnet network is fixed in code (INVARIANTS rows 328, 339). `docs/payments/agent-payments.md` lists the rejected rails.
- The oa-copy guard is a real, reusable mechanism. It scans source string literals and skips logs, panics and test code. It also checks rendered HTML (`openagents-web` `copy_guard.rs::assert_plain` runs on 61 pages) and keeps per-route and per-file allowlists, each with a written reason. It catches block elements nested inside `<p>`.
- Every INVARIANTS.md row names a check, and most name exact test functions with their crates. This is what makes the drift below detectable by a script.
- The phone's amounts go through the shared `crates/bitcoin-amount` formatter (INVARIANTS row 336). The remaining "sat" strings there are fee-rate units (sat/vB).

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-ROUTE-01 | High | policy | Crew agent runs a real merge when the owner's text matches a substring | M |
| X-ROUTE-02 | High | policy | "Remember ..." prefix and "never"/"always" substrings reroute crew requests | M |
| X-ROUTE-03 | Medium | policy | Machine talk ships in unguarded web and wasm surfaces | S |
| X-ROUTE-04 | Medium | policy | Coder crate's copy guard covers 2 files; crew replies carry jargon | S |
| X-ROUTE-05 | Medium | policy | Web /stats and desktop say "sats" with their own formatters | S |
| X-ROUTE-06 | Medium | testing | INVARIANTS.md cites deleted tests and nothing checks the citations | M |
| X-ROUTE-07 | Medium | docs | INVARIANTS.md has become an unreviewable change log | L |
| X-ROUTE-08 | Medium | error-handling | Outcomes classified by matching message text | M |
| X-ROUTE-09 | Low | policy | /app/purchases shows digests, Debug-formatted enums and msat | S |
| X-ROUTE-10 | Low | duplication | Separate banned-word lexicons drift apart | S |
| X-ROUTE-11 | Low | policy | coder-one ask picks a retrieval probe by keyword | S |
| X-ROUTE-12 | Low | policy | Component catalog models a Chat/Code mode switch and "(soon)" dead options | S |
| X-ROUTE-13 | Low | policy | Desktop Verse/Grid page is a placeholder on Windows in preview builds | S |
| X-ROUTE-14 | Low | testing | Copy guard skips two shipped scripts | S |
| X-ROUTE-15 | Low | docs | Workspace guide sends agents to openagents files that don't exist | S |

### X-ROUTE-01 Crew agent runs a real merge when the owner's text matches a substring

**Severity:** High · **Category:** policy · **Effort:** M

**Locations:**
- [agent_host_merge.rs](../../../../crates/coder/src/task/agent_host_merge.rs), agent_host_merge.rs:25-63, agent_host_merge.rs:66-104
- [agent_host.rs](../../../../crates/coder/src/task/agent_host.rs), agent_host.rs:2113-2119

**Evidence:** `asks_to_merge()` lowercases the text. It returns true when the text contains "merge", contains none of 4 negations, and contains any of 14 phrases, among them "for me", "do it", "do that" and "instead of me". `merge_station()` then calls `self.merge_own(store, name)`, reached from agent_host.rs:2116-2118 "with no model call". "Could you explain this merge conflict for me?" contains "merge" and "for me", so it returns true and triggers `merge_own`. `asks_where()` is `contains("merge station") && any of ["where","how do i","how to"]`.

**Impact:** A message that only mentions merging can merge her waiting change into the owner's branch. This is the keyword routing of user intent that the workspace rule forbids.

**Suggested action:**
1. Replace `asks_where` and `asks_to_merge` with one Jev Choice over `{merge_station.where, merge_station.merge_own, none}`, called from `merge_station()` in agent_host_merge.rs.
2. Act on `merge_own` only above a probability threshold, and add a confirm step before calling `merge_own`.
3. Delete both substring helpers.
4. Verify with regression tests: "explain this merge conflict for me" must not merge, and "please merge your change" must reach the confirm step.

### X-ROUTE-02 "Remember ..." prefix and "never"/"always" substrings reroute crew requests

**Severity:** High · **Category:** policy · **Effort:** M

**Locations:**
- [agent_memory.rs](../../../../crates/coder/src/task/agent_memory.rs), agent_memory.rs:411-441
- [agent_host.rs](../../../../crates/coder/src/task/agent_host.rs), agent_host.rs:2120-2145

**Evidence:** `remembered()` returns the rest of any request that starts with "remember that ", "remember " or "note that ". agent_host.rs:2122-2136 then stores it as a note, replies "I'll remember that (memory entry {id}).", and returns before any model call. So "Remember to run the tests and fix the login bug" is filed as a note and no work happens. `proposed_preference()` files any request of 400 bytes or fewer that contains "always ", "never ", "i prefer ", "from now on" or "in the future" as an Agent-authored preference "The owner said: ...". This second path does not stop the request, which still reaches the model, but it does add a junk preference.

**Impact:** Work requests that start with "remember" are silently dropped. Ordinary sentences fill the preference store with noise, and an internal memory id is shown to the user.

**Suggested action:**
1. Route note, preference and work through one typed Jev Choice on the crew request, next to the merge-station choice from X-ROUTE-01.
2. Keep deterministic parsing only to extract the note body after the note route has been chosen.
3. Reply "Got it, I'll remember that." without the id.
4. Verify with tests: "Remember to fix X" must start work, and "I never got the email" must not add a preference.

### X-ROUTE-03 Machine talk ships in unguarded web and wasm surfaces

**Severity:** Medium (lowered from high: wording on a few error and notice paths, not behavior) · **Category:** policy · **Effort:** S

**Locations:**
- [coder-browser-web/src/mount.rs](../../../../crates/coder-browser-web/src/mount.rs), mount.rs:526, mount.rs:1686, mount.rs:1833, mount.rs:2094
- [everglade-web/src/terminal.rs](../../../../crates/everglade-web/src/terminal.rs), terminal.rs:60
- [coder-cloud-web/src/browser.rs](../../../../crates/coder-cloud-web/src/browser.rs), browser.rs:53

**Evidence:** All six strings are present verbatim, for example "...no input was retained or replayed.", "... context digest {}", "... of {} retained turns; read-only.", "The admitted native workbench is unavailable...", "The admitted host terminal is unavailable..." and "The workspace needs fresh server admission...". "retained", "admitted", "admission" and "digest" are all in `oa_copy::TERMS`. None of coder-browser-web, everglade-web, coder-cloud-web or coder-chat-web lists oa-copy in its Cargo.toml (0 matches each), while 13 other crates do.

**Impact:** People see words banned by #11031 in the browser terminal and workbench, and nothing fails when more are added.

**Suggested action:**
1. Add `oa-copy` as a dev-dependency to coder-browser-web, everglade-web, coder-cloud-web and coder-chat-web.
2. Add a `copy_guard_tests.rs` to each that calls `oa_copy::scan_dir_allowing`, modelled on [crates/coder-ui/src/copy_guard_tests.rs](../../../../crates/coder-ui/src/copy_guard_tests.rs).
3. Rewrite the six strings in plain words. Put the thread/generation/digest detail line behind a developer-only toggle.
4. Verify: the new guard tests fail before the rewrite and pass after it (`cargo test -p <crate> copy_guard`).

### X-ROUTE-04 Coder crate's copy guard covers 2 files; crew replies carry jargon

**Severity:** Medium · **Category:** policy · **Effort:** S

**Locations:**
- [coder/src/copy_guard_tests.rs](../../../../crates/coder/src/copy_guard_tests.rs), copy_guard_tests.rs:1-15
- [agent_host.rs](../../../../crates/coder/src/task/agent_host.rs), agent_host.rs:2012, agent_host.rs:2088, agent_host.rs:2100-2103
- [agent_coder.rs](../../../../crates/coder/src/task/agent_coder.rs), agent_coder.rs:723

**Evidence:** `FILES` lists only `cli.rs` and `task/cli.rs`. Crew replies the owner reads include "The original admission was revoked; no queued work starts." ("admission" is an oa-copy term), "Private customer disclosure is unavailable for this crew request.", "The current sales charter refuses this request.", "Private customer model disclosure is unavailable." and "(memory entry {id})".

**Impact:** Nothing checks the crew agent's conversation for machine talk, so jargon regressions land unnoticed.

**Suggested action:**
1. Add `task/agent_host.rs`, `task/agent_host_merge.rs`, `task/agent_coder.rs` and `task/agent_memory.rs` to `FILES`, or move the reply strings into a guarded `task/agent_copy.rs`.
2. Rewrite the hits in plain words.
3. Verify: `cargo test -p coder copy_guard` passes with the extended file list.

### X-ROUTE-05 Web /stats and desktop say "sats" with their own formatters

**Severity:** Medium · **Category:** policy · **Effort:** S

**Locations:**
- [openagents-web/src/pages/stats.rs](../../../../crates/openagents-web/src/pages/stats.rs), stats.rs:83-98
- [openagents-desktop/src/route_live.rs](../../../../crates/openagents-desktop/src/route_live.rs), route_live.rs:752-758
- [docs/breez/amounts.md](../../../../docs/breez/amounts.md), amounts.md:9-33

**Evidence:** amounts.md:11 says "One formatter. crates/bitcoin-amount shows and reads every amount", and amounts.md:33 says "No 'sat' in product copy." In stats.rs, `Msat::sats()` (line 84) renders "1,234 sats" and line 98 has its own `grouped()`. route_live.rs:754 renders "{} sats received · {} sats paid out · {} calls". Only bitcoin-amount itself, openagents-mobile, openagents-cli and spark-wallet depend on bitcoin-amount.

**Impact:** The public /stats page and the desktop break the documented amounts rule and duplicate the formatting code.

**Suggested action:**
1. Add `bitcoin-amount` to openagents-web and openagents-desktop.
2. Replace `Msat::sats()`/`grouped()` in stats.rs and the format string at route_live.rs:754 with `bitcoin_amount` `Format::Bip177` output.
3. Add "sat" and "sats" to `oa_copy::TERMS` so regressions fail the guard. Allowlist fee-rate units (sat/vB) where needed.
4. Verify: the openagents-web copy guard and the desktop tests pass, and `rg '" sats' crates/openagents-web crates/openagents-desktop` returns nothing.

### X-ROUTE-06 INVARIANTS.md cites deleted tests and nothing checks the citations

**Severity:** Medium (lowered: documentation drift, not a runtime defect) · **Category:** testing · **Effort:** M

**Locations:**
- [INVARIANTS.md](../../../../INVARIANTS.md), INVARIANTS.md:49, INVARIANTS.md:140, INVARIANTS.md:152, INVARIANTS.md:160-166, INVARIANTS.md:523, INVARIANTS.md:617

**Evidence:** The verifier spot-checked 6 of the 11 named tests: `a_question_must_end_with_the_visitor`, `the_judge_answers_first_and_the_model_follows_its_opener`, `a_refusal_carries_its_code_and_wait`, `the_catalog_plugins_have_their_real_statuses`, `a_toggle_allows_and_removes_a_provider_and_keeps_the_settings_valid` and `the_primary_is_space_bunny_whenever_openrouter_is_reachable`. Each appears once in INVARIANTS.md, and none has a `fn NAME` definition under `crates/`, `bins/` or `tests/`. Only one missing path was confirmed: `crates/openagents-mobile/src/gym/tests.rs` (row at line 523). `gym.rs` is a single file.

**Impact:** Rows that say "checked by X" point at tests that no longer exist, and the drift goes unnoticed.

**Suggested action:**
1. Add a ledger check, either as `crates/coder/tests/invariants_ledger.rs` or as a `scripts/` check in the docs gate. It parses backticked snake_case names in the Checked-by column and asserts that each has a `fn NAME` under `crates/`, `bins/` or `tests/`, with a `(manual)` escape.
2. Re-point the 11 rows to their replacement tests (for example, the openagents-web chat tests for row 140).
3. Fix the `gym/tests.rs` path to [crates/openagents-mobile/src/gym.rs](../../../../crates/openagents-mobile/src/gym.rs).
4. Verify: the ledger check fails on the current file, passes after the re-pointing, and fails again if a cited test is renamed.

### X-ROUTE-07 INVARIANTS.md has become an unreviewable change log

**Severity:** Medium · **Category:** docs · **Effort:** L

**Locations:**
- [INVARIANTS.md](../../../../INVARIANTS.md), INVARIANTS.md:1-12, INVARIANTS.md:38, INVARIANTS.md:209

**Evidence:** The file is 649 lines and 377,886 bytes (about 378 KB). Of its 381 table rows, the longest is 7,072 characters and 46 exceed 2,000. 248 commits touched the file between 2026-09-10 and the audit date.

**Impact:** Reviewers cannot see what an invariant currently says, and this single file causes constant merge conflicts.

**Suggested action:**
1. Split the file by area (for example `docs/invariants/{coder,chat,wallet,pay,verse}.md`) and keep INVARIANTS.md as an index.
2. Cut each row down to the current rule plus an issue link. Move "Reinterpreted on" history to git.
3. Add a row-length cap to the ledger check from X-ROUTE-06.
4. Verify: the ledger check passes on the split files, and no row exceeds the cap.

### X-ROUTE-08 Outcomes classified by matching message text

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [sales/remote.rs](../../../../crates/coder/src/task/sales/remote.rs), remote.rs:1098-1104
- [agent_coder.rs](../../../../crates/coder/src/task/agent_coder.rs), agent_coder.rs:531-536

**Evidence:** remote.rs:1098 maps an error whose `message.contains("idempotency conflict")` to `Code::Conflict`, and one containing "revision conflict" to `Code::Stale`. agent_coder.rs:531 and :536 branch on `said.starts_with("The host refuses")` and `said.starts_with("The owner rejected")`.

**Impact:** A copy fix to those sentences silently changes how conflicts and refusals are handled.

**Suggested action:**
1. Make `store.apply()` return a typed error enum (`Conflict`, `Stale`, `Refused`), and match on it in remote.rs.
2. Give the host command result a typed `Outcome { Ran, Refused, Rejected }`, and have agent_coder.rs match on that instead of on the text.
3. Verify: add tests that reword the human-readable messages and assert the classification is unchanged.

### X-ROUTE-09 /app/purchases shows digests, Debug-formatted enums and msat

**Severity:** Low (lowered: the route serves data only when `config.customer` is set, which is the local customer store on this computer, not a public page) · **Category:** policy · **Effort:** S

**Locations:**
- [openagents-web/src/purchases.rs](../../../../crates/openagents-web/src/purchases.rs), purchases.rs:23-27, purchases.rs:52-67, purchases.rs:75-91, purchases.rs:101-160
- [openagents-web/src/copy_guard.rs](../../../../crates/openagents-web/src/copy_guard.rs), copy_guard.rs:104

**Evidence:** The page shows a "LOCAL / READ ONLY" eyebrow, "Quote ID" and "Approval ID" rows, "Payer {node} on {network}" and "Recovery: checked". `msat()` renders `format!("{value} msat")`. The list view prints `{phase:?}` (line 84), although `phase_text()` at line 52 already gives plain wording. copy_guard.rs:104 allows "digest" in purchases.rs.

**Impact:** The purchase record reads like a ledger dump and shows msat, which breaks the BIP 177 amounts rule.

**Suggested action:**
1. Use `phase_text(phase)` in `render_list` at purchases.rs:84.
2. Format amounts through `bitcoin_amount` (shares the dependency added in X-ROUTE-05).
3. Move the Quote/Approval/Payer/Recovery rows into a `<pre>` "Details for support" block, then drop `("purchases.rs", ["digest"])` from `ALLOW_IN`.
4. Verify: the openagents-web copy guard passes without the allowlist entry.

### X-ROUTE-10 Separate banned-word lexicons drift apart

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [oa-copy/src/lib.rs](../../../../crates/oa-copy/src/lib.rs), lib.rs:17-87
- [openagents-chat-app/src/eval_cards.rs](../../../../crates/openagents-chat-app/src/eval_cards.rs), eval_cards.rs:23-75
- [openagents-desktop/src/words.rs](../../../../crates/openagents-desktop/src/words.rs), words.rs:18
- [coder-new/src/copy_guard_tests.rs](../../../../crates/coder-new/src/copy_guard_tests.rs), copy_guard_tests.rs:34-41

**Evidence:** oa-copy `TERMS` (machine talk: retained, admission, digest...) and chat-app `eval_cards::BANNED` (label words: npub, relay, eval, judge, sats, btc, host, workspace...) cover different sets on purpose. The desktop `words.rs:18` `BANNED` starts with the same entries as `eval_cards::BANNED`, so it is a hand-kept copy. coder-new `PLUMBING` is a small, separate list of error-plumbing words.

**Impact:** The same string can pass on one surface and fail on another, and each new term has to be added in several places.

**Suggested action:**
1. Move the label-word list into oa-copy as a named set (for example `oa_copy::LABEL_WORDS`) next to `TERMS`.
2. Have both eval_cards.rs and openagents-desktop/src/words.rs use it, and delete the desktop copy. Leave `PLUMBING` local.
3. Verify: `cargo test -p openagents-chat-app -p openagents-desktop` passes and `rg 'const BANNED' crates/openagents-desktop` returns nothing.

### X-ROUTE-11 coder-one ask picks a retrieval probe by keyword

**Severity:** Low (internal Gym question tool; only adds or skips one read-only probe) · **Category:** policy · **Effort:** S

**Locations:**
- [coder-one/src/ask/gather.rs](../../../../crates/coder-one/src/ask/gather.rs), gather.rs:366-381
- [coder-one/src/ask/mod.rs](../../../../crates/coder-one/src/ask/mod.rs), mod.rs:1009-1013, mod.rs:1062-1068

**Evidence:** `asks_strategy()` lowercases the question and matches `[fable, strategy, strategies, fingerprint, moves, phase, winners do]`. A hit adds the `runs moves --cached` Gym probe, and the comment says "Neither asks Jev anything." Nearby, the "marks" decision uses a Jev Noul gated by `ASK_GATHER_YES`, and it falls back to `names_marks()` only when Jev gives no answer.

**Impact:** Whether retrieval runs depends on wording, which goes against the workspace routing rule. The cost is bounded: one extra cached probe, or one missed.

**Suggested action:**
1. Add a "strategy" Noul to the existing `ask_reasons` Jev question set in ask/mod.rs, and gate the moves probe on `ASK_GATHER_YES` the same way `about_marks` is gated.
2. Keep `asks_strategy` only as the fallback when Jev gives no answer.
3. Verify: add paraphrase tests in gather.rs (a strategy question with none of the keywords still triggers the probe when Jev says yes).

### X-ROUTE-12 Component catalog models a Chat/Code mode switch and "(soon)" dead options

**Severity:** Low · **Category:** policy · **Effort:** S

**Locations:**
- [openagents-ui/src/catalog/forms.rs](../../../../crates/openagents-ui/src/catalog/forms.rs), forms.rs:85, forms.rs:115-118, forms.rs:137
- [openagents-ui/src/catalog/overlays.rs](../../../../crates/openagents-ui/src/catalog/overlays.rs), overlays.rs:86

**Evidence:** The SegmentedControl specimen has the options "Chat" and "Code" plus a disabled "Voice", with `aria_label("Mode")`. The catalog also shows the disabled options "Team (soon)", "Asia Pacific (soon)" and "Ultra (soon)". This was added in b1d488861e on 2026-10-08 and is served at `/ui` (openagents-web lib.rs:248).

**Impact:** The shared catalog demonstrates the mode switch and placeholder controls the owner has banned.

**Suggested action:**
1. Change the specimen to a neutral switch such as Day/Week/Month, and remove the "(soon)" options.
2. Add "(soon)" and "coming soon" to `oa_copy::PHRASES`.
3. Verify: the openagents-web copy guard over `/ui` passes, and `rg '\(soon\)' crates/openagents-ui` returns nothing.

### X-ROUTE-13 Desktop Verse/Grid page is a placeholder on Windows in preview builds

**Severity:** Low · **Category:** policy · **Effort:** S

**Locations:**
- [openagents-desktop/src/chrome.rs](../../../../crates/openagents-desktop/src/chrome.rs), chrome.rs:703-712, chrome.rs:1404-1418

**Evidence:** When `cfg!(windows)`, `Page::Grid` shows only "Playable Verse is not yet available on Windows." The "Verse" sidebar entry is added only when `state.preview` is set (chrome.rs:703), so normal builds do not show it.

**Impact:** Preview users on Windows reach a dead page.

**Suggested action:**
1. Change chrome.rs:703 to `if state.preview && !cfg!(windows)`.
2. Delete the Windows placeholder string.
3. Verify: the desktop crate builds and its tests pass, and `rg 'not yet available on Windows' crates/openagents-desktop` returns nothing.

### X-ROUTE-14 Copy guard skips two shipped scripts

**Severity:** Low · **Category:** testing · **Effort:** S

**Locations:**
- [openagents-web/src/copy_guard.rs](../../../../crates/openagents-web/src/copy_guard.rs), copy_guard.rs:115-125
- [oa-copy/src/lib.rs](../../../../crates/oa-copy/src/lib.rs), lib.rs:140-166

**Evidence:** The script list names chat.js, chat-start.js, components-start.js, flow.js and everglade.js. `static/` also ships a.js and webmcp.js, which are not scanned.

**Impact:** Jargon added to a.js or webmcp.js would pass the guard.

**Suggested action:**
1. In `site_sources_have_no_machine_talk`, scan every `static/*.js` except `*.test.js` and `vendor/`, instead of the hard-coded list.
2. Verify: the test enumerates a.js and webmcp.js, and a planted banned term in either fails it.

### X-ROUTE-15 Workspace guide sends agents to openagents files that don't exist

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [docs/cloud/README.md](../../../../docs/cloud/README.md) (as reported), plus the workspace root `CLAUDE.md`/`AGENTS.md` (outside this repo)

**Evidence:** Running `ls` at HEAD shows that `docs/MVP.md`, `docs/OWNERSHIP.md`, `docs/headless-compute.md`, `docs/kernel/README.md`, `docs/cloud/INVARIANTS.md`, `docs/cloud/MIGRATION.md`, `apps/` and `crates/oa-node` do not exist. The workspace CLAUDE.md names each of them as a starting point.

**Impact:** Agents look for start docs and app paths that do not exist.

**Suggested action:**
1. In the root workspace repo (not this repo), point CLAUDE.md's openagents sections at `crates/openagents-web`, `crates/openagents-mobile` and `crates/openagents-desktop`, with INVARIANTS.md as the ledger.
2. Drop the `oa-node` and `docs/cloud/INVARIANTS.md` / `MIGRATION.md` references.
3. Verify: every openagents path named in the workspace CLAUDE.md resolves with `ls`.

## Refuted during verification

- **"x402 router keeps a Cashu method slot after the owner closed Cashu."** Rejected. `docs/payments/agent-payments.md:14` records Cashu as "not now, maybe later", not as rejected. The variant is a documented reserved slot, and nothing constructs `Method::Cashu`, so nothing can advertise it.
- **"Breez docs still frame USDC/USDT receive as off only 'at first'."** Not a policy violation. `docs/breez/README.md:102-103` does say "off at first", and `stablecoin-receive.md` is a 2026-09-28 assessment written before the decision. The authoritative 2026-10-09 decision in `docs/payments/agent-payments.md` is explicit, and no code path exists. This belongs in a general docs-staleness cleanup.
- **"INVARIANTS.md cites 8 paths that are missing."** Mostly wrong. `tests/pty.rs` (coder-pty, openagents-terminal), `tests/threads.rs` (coder-host), `tests/tool_groups.rs` (openagents-chat), `first_run.rs` (openagents-mobile/src) and `nips/openagents/NIP-MV.md#shared-bodies` (heading at line 346) all exist as crate-relative paths or anchors, and `/connect` is a slash command, not a path. Only `crates/openagents-mobile/src/gym/tests.rs` is truly missing.
- **"INVARIANTS.md is about 80 KB."** Wrong. It measures 377,886 bytes (about 378 KB).
- **"The web copy guard misses attribute text."** Mostly covered already. `site_sources_have_no_machine_talk` scans every Rust string literal, which includes static attribute text written in Rust. Only the two unscanned scripts (X-ROUTE-14) are a real gap.
