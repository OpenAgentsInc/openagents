# Documentation drift & prior-audit follow-up

Scope: `docs/` (including `docs/audits`), `README.md`, `AGENTS.md`, `INVARIANTS.md`, `NEEDS_OWNER.md`, crate READMEs, the workspace-level `AGENTS.md`, and the status of the 2026-09-19 audit remediation register. Snapshot `3168c986aa`, audit date 2026-10-10.

**Health grade: C**

Most documentation is well kept within each file. All 182 workspace crates have a crate-level `//!` doc. About 77% of public items have `///` docs. The crate READMEs that exist cite real tests, and historical documents carry "Status: historical" or "Removed" banners. Of the 1,299 test names cited in `INVARIANTS.md`, all but about 12 still exist. The problems are in the indexes and navigation docs, and they come from growth. Since the 2026-09-19 audit (`1843fa6c18`), 4,990 commits took the repo from 10 crates and 365 doc files to 184 crate directories, 3,691 doc files (138 MB) and about 2.34M lines of Rust. The indexes have not kept up and now claim more than they deliver:

- `README.md` says `AGENTS.md` has the full crate list, but `AGENTS.md` leaves out 92 of 184 crates, including the flagship apps.
- `docs/catalog.md` calls itself complete, but it leaves out several hundred live docs.
- `docs/audits/README.md` does not list the newest audits.
- The master roadmap never mentions the V1 Launch board.

The workspace-level `AGENTS.md`, which every agent here loads as `CLAUDE.md`, routes openagents work to paths that were deleted on 2026-09-18 or never existed.

All 19 remediation issues from the 2026-09-19 audit are closed. Some items have slipped back since then:

- **Stale register:** the register text still says A17, A18 and #9427 are open.
- **A24 bytecode:** 603 `.pyc` files are tracked again.
- **A02 process tree:** the new `microcoder-loop` executor does not kill the whole process tree when a deadline hits.
- **A23 package policy:** six crates skip workspace lint inheritance, and three declare the invalid license `CC-0`.

## Measurements

| Metric | Value |
|---|---|
| Tracked files / `.git` size | 38,825 / 9.7 GB |
| Workspace packages / crate dirs | 182 / 184 (psionic and openagents-mobile are excluded workspaces) |
| Rust lines outside `crates/psionic` | ~2,339,780 |
| At 2026-09-19 audit (`1843fa6c18`) | 10 crates, 365 docs files, 4,990 commits ago |
| `docs/` | 3,691 files, 138 MB (png 51.5 MB, json 30.7 MB, md 21.8 MB); desktop 33M, audits 23M, verse 18M, coder 13M |
| `docs/audits` | 1,334 files; `2026-10-03-independent-efficiency` = 1,306 files / 22 MB (474 log, 472 json, 212 patch, 105 md, 17 py, 7 rs, 2 tgz) |
| `AGENTS.md` | 883 lines / 58 KB |
| `INVARIANTS.md` | 649 lines / 378 KB; 381 table rows; 46 rows > 2,000 chars; longest line 7,072 chars |
| `README.md` | 390 lines |
| `NEEDS_OWNER.md` | 2,177 lines / 146 sections; every issue named in its headings is closed |
| Relative markdown links (non-bench/vendor/psionic) | 14,124; 480 unresolved in 91 files; 53 real after excluding site routes, fixtures, audits, NIP numbers |
| Backticked repo paths not resolving | 838 (mostly historical and external-repo surveys) |
| `INVARIANTS.md` test citations | 1,299 cited; 17 not defined as Rust fns; 12 appear nowhere outside docs |
| Crates without README | 127 / 182 |
| Crates missing from `AGENTS.md` "## Crates" | 92 / 184 |
| Crates missing from README repo map | 119 / 184 |
| Large crates with no README and no AGENTS/README mention | openagents-ui 21.8k LOC, terminal-core 19.5k, coder-cloud 11.4k, oa-auth 9.4k |
| `docs/catalog.md` coverage | 528 docs linked; 337 of 850 live docs uncatalogued |
| Public-item `///` coverage | 35,133 / 45,376 = 77.4%; lowest: coder-ui 14%, boat 35%, nostr-relay 37%, verse-engine 40%, coder-new 43%, pay-ledger 46% |
| Crates with `//!` crate doc | 182 / 182 |
| Zero-dependent library crates with no bin | 14, all intentional cdylib web/plugin leaves |
| Tracked `.pyc` / `.DS_Store` | 603 (601 under `bench/`, 2 under `scripts/`) / 1 |
| Members without `lints.workspace` | 6 |
| Crates declaring license `CC-0` | 3 |
| 2026-09-19 remediation issues closed | 19 / 19 (#9415–#9433, plus #9413, #9402, #9404) |
| Open GitHub issues | 61 |

## Strengths

- All 182 workspace crates open with a `//!` crate doc, and 77% of public items carry `///` docs. Enforce this with lints so it does not erode.
- `INVARIANTS.md` names the tests that check each invariant. About 1,287 of the 1,299 cited identifiers still resolve to real functions.
- Historical documents are clearly labeled. For example, `docs/coder-earn.md`, `docs/psionic-and-pylon.md`, `docs/text-optimization.md` and `docs/delegation-brief.md` start with "Status: historical…", and `docs/verse/ruins-source-parity.md` has a `> **Removed.**` banner.
- `docs/documentation.md` sets a sound policy: one job per document, retained evidence is not rewritten, and references are checked on change. The drift comes from not following it at the index layer. The policy itself is fine.
- The 2026-09-19 tooling fixes held:
  - `rust-toolchain.toml` pins 1.97.1.
  - `deny.toml` uses `unmaintained='all'` with one owned, dated ignore (RUSTSEC-2024-0436, to be reviewed by 2026-10-20).
  - `.gitignore` covers `swift/lev-bridge/.build` and training bytecode.
- Existing crate READMEs are accurate. Every apparently broken test path in them turned out to be a correct cross-crate reference.
- Every script named in `AGENTS.md`, `README.md`, `docs/verification.md`, `docs/project-board.md` and `docs/dependencies.md` exists.
- Of the 52 distinct `openagents <subcommand>` forms used in live docs, only 3 do not match an implemented command.
- There are no dead library crates. The 14 zero-dependent ones are intentional cdylib build roots.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-DOC-01 | medium | docs | Workspace `AGENTS.md` routes openagents work to deleted or never-existing paths | S |
| X-DOC-02 | medium | docs | README says `AGENTS.md` has the full crate list; it leaves out 92 of 184 crates | M |
| X-DOC-03 | medium | concurrency | A02-class regression: microcoder-loop Bounded env relies on `kill_on_drop` only; secret scrub is suffix-only | M |
| X-DOC-04 | medium | testing | `INVARIANTS.md` "Checked by" cites about 12 tests that no longer exist | S |
| X-DOC-05 | medium | maintainability | `INVARIANTS.md` is a 378 KB table with 7 KB single-line rows | L |
| X-DOC-06 | medium | docs | Two owner-action queues (workspace vs repo `NEEDS_OWNER.md`) | S |
| X-DOC-07 | low | docs | Docs indexes claim a completeness they lack | M |
| X-DOC-08 | low | docs | 2026-09-19 remediation register still says closed items are open | S |
| X-DOC-09 | low | repo-hygiene | A24 regression: 603 tracked `.pyc` files | S |
| X-DOC-10 | low | build | A23 partly regressed: 6 crates skip lint inheritance, 3 declare `CC-0` | S |
| X-DOC-11 | low | docs | Theme docs still describe the amber palette that Coder Noir replaced | S |
| X-DOC-12 | low | docs | Roadmap omits the V1 Launch board; migration tracker is two weeks old | S |
| X-DOC-13 | low | repo-hygiene | 22 MB of raw evidence in one audit folder | M |
| X-DOC-14 | low | docs | Broken links in live docs, crate READMEs and NIPs | S |
| X-DOC-15 | low | docs | Weak public-API docs in key crates; no `missing_docs` lint | M |
| X-DOC-16 | low | docs | Historical surveys at the `docs/` root; machine-local paths in `docs/` Cargo manifests | S |
| X-DOC-17 | low | policy | Language policy does not cover the web JS and Blender/Unreal Python in the tree | S |

### X-DOC-01 Workspace AGENTS.md routes openagents work to deleted or never-existing paths

Severity: medium · Category: docs · Effort: S

Locations:
- Workspace `AGENTS.md` (outside this repo, in `AtlantisPleb/workspace`): [AGENTS.md](../../../../../AGENTS.md), lines 88, 91, 817, 1182, 1191, 1193 and 1957.
- [docs/cloud/README.md](../../../../docs/cloud/README.md), line 161.

Evidence:
- `ls apps` fails in openagents because the directory does not exist.
- Workspace `AGENTS.md:88-91` names `openagents/apps/openagents.com`, `apps/openagents-mobile`, `apps/openagents-desktop` and `packages/ui` as the product homes. The real crates are `crates/openagents-web`, `crates/openagents-mobile`, `crates/openagents-desktop` and `crates/openagents-ui`.
- Line 1182 routes Cloud work to `crates/oa-node`, line 1191 to `openagents/docs/cloud/MIGRATION.md`, and line 1193 to issue #8591. Line 1957 repeats this.
- `docs/cloud/MIGRATION.md` and `docs/cloud/INVARIANTS.md` do not exist. `docs/cloud/README.md:161` says the `oa-*` crates and the old `docs/cloud` were removed in `dabc08102f` on 2026-09-18.
- `openagents/AGENTS.md:3` says "Product code in this workspace is Rust. Do not add TypeScript.", but the workspace file tells agents to consult Effect guidance.

Impact: Every session starts with wrong routing hints. That wastes turns and invites files in the wrong place. The workspace precedence rule (follow the child `AGENTS.md` first) limits the damage.

Suggested action:
1. In the `AtlantisPleb/workspace` `AGENTS.md`, replace `apps/*` and `packages/ui` with `crates/openagents-web`, `crates/openagents-mobile`, `crates/openagents-desktop`, `crates/openagents-terminal`, `crates/openagents-ui` and `crates/coder-ui`.
2. Replace lines 1182–1193 and the "OpenAgents Cloud ownership (2026-07-09)" section with pointers to `crates/coder-cloud`, `crates/coder-environment*`, `crates/boat` and `docs/cloud/README.md`.
3. State that the Effect guidance does not apply inside openagents. Point to `openagents/AGENTS.md` for the crate map.
4. Add a check script that extracts every `openagents/...` path from the workspace `AGENTS.md` and fails on any path that does not exist. To verify the fix, run the script and confirm it reports zero missing paths. Commit and push the workspace repo.

### X-DOC-02 README says AGENTS.md has the full crate list, but AGENTS.md leaves out 92 of 184 crates, including the flagship apps

Severity: medium · Category: docs · Effort: M

Locations: [README.md](../../../../README.md) README.md:328; [AGENTS.md](../../../../AGENTS.md) AGENTS.md:224; [crates/openagents-web](../../../../crates/openagents-web), [crates/openagents-desktop](../../../../crates/openagents-desktop), [crates/openagents-terminal](../../../../crates/openagents-terminal), [crates/openagents-ui](../../../../crates/openagents-ui).

Evidence: `README.md:328` says "AGENTS.md has the full crate list." Checking each `ls crates` entry against `AGENTS.md` finds 92 of 184 crates that are never mentioned. `grep -c openagents-web AGENTS.md` returns 0, and so does `grep -c openagents-desktop AGENTS.md`.

Impact: Agents treat `AGENTS.md` as their contract. Large crates such as oa-auth, pay-ledger, coder-cloud and the main apps have no orientation entry.

Suggested action:
1. Add `scripts/dev/crate-map.sh`. It should run `cargo metadata --no-deps` and write each crate's name, path and first `//!` paragraph into `docs/repo/crates.md`.
2. Change `README.md:328` to link `docs/repo/crates.md`.
3. Keep the hand-written entries in `AGENTS.md` only for crates whose contract agents must read first, and say so in the section header.
4. Make the script fail when a directory under `crates/` is missing from the generated file. To verify, run it in CI or in the docs check, and confirm that `grep -c openagents-web docs/repo/crates.md` returns at least 1.

### X-DOC-03 A02 class regression: microcoder-loop Bounded env relies on kill_on_drop only, so a host deadline may not reap grandchildren; secret scrub is suffix-only

Severity: medium · Category: concurrency · Effort: M

Locations: [crates/microcoder-loop/src/env.rs](../../../../crates/microcoder-loop/src/env.rs) env.rs:137, env.rs:157, env.rs:213, env.rs:219, env.rs:247; [crates/coder/src/delegate_door/microcoder.rs](../../../../crates/coder/src/delegate_door/microcoder.rs) microcoder.rs:602; [docs/audits/2026-09-19-codebase-audit/remediation.md](../../2026-09-19-codebase-audit/remediation.md) remediation.md:41.

Evidence:
- The `execute()` path that `Bounded::run` uses (env.rs:213–264) spawns with only `.kill_on_drop(true)` (l.219). It sets no process group and never calls `killpg`.
- When `tokio::time::timeout` expires, it returns "[the host stopped the command at its deadline]" and relies on drop to kill the direct child.
- The read future waits for EOF on stdout and stderr. A background grandchild that keeps a pipe open therefore runs until the deadline and then survives it.
- `microcoder-loop/Cargo.toml` has no `supervise` dependency.
- The Docker variant guards against this with an in-container `timeout -k`. Bounded has no equivalent.
- coder's delegate door builds Bounded at microcoder.rs:602.
- The env scrub (l.157) removes only names ending in `_API_KEY`, `_TOKEN` or `_SECRET`, so `AWS_SECRET_ACCESS_KEY` and `DATABASE_URL` pass through.

Impact: A command the model starts that times out, such as a server or a cargo build, can keep running and hold ports and locks after the loop reports it stopped. A02 fixed this same class of defect in the live Coder loop.

Suggested action:
1. In `crates/microcoder-loop/src/env.rs`, route `execute()` for Bounded through `supervise::spawn`, or set `process_group(0)` and `killpg` the group on timeout.
2. Add a test that runs `sleep 30 & echo $! > pid; wait` with a 1 s deadline and asserts that the pid is gone afterwards.
3. Replace the suffix scrub with an env allowlist, or reuse `crates/secret-screen`. Add a test that sets `AWS_SECRET_ACCESS_KEY` and asserts it is absent in the child.
4. Verify with `cargo test -p microcoder-loop`.

### X-DOC-04 INVARIANTS.md 'Checked by' column cites about 12 tests that no longer exist

Severity: medium · Category: testing · Effort: S

Locations: [INVARIANTS.md](../../../../INVARIANTS.md), lines 49, 140, 152, 161, 163, 376, 523 and 617.

Evidence:
- `git grep -l` for each of these names returns only `INVARIANTS.md`:
  - `a_toggle_allows_and_removes_a_provider_and_keeps_the_settings_valid`
  - `a_question_must_end_with_the_visitor`
  - `the_judge_answers_first_and_the_model_follows_its_opener`
  - `the_primary_is_space_bunny_whenever_openrouter_is_reachable`
  - `a_refusal_carries_its_code_and_wait`
  - `a_wrong_answer_report_carries_the_exchange`
- `the_catalog_plugins_have_their_real_statuses` appears only in `INVARIANTS.md` and a docs verification record.
- `INVARIANTS.md:523` cites `crates/openagents-mobile/src/gym/tests.rs`, which does not exist. The file is `crates/openagents-chat-app/src/gym/tests.rs`.

Impact: These rows claim test coverage that does not exist. Either the invariants are unguarded or the rows are stale.

Suggested action:
1. For each row, either restore or rename the test and update the citation, or mark the row relaxed or reinterpreted and name the commit that removed the test (for example `ddab7b9a6e` for l.161).
2. Fix the path on l.523.
3. Add `scripts/dev/invariants-tests.sh`. It should fail when a backticked snake_case name has no matching `fn name`, with an allowlist for identifiers that are not tests.
4. Verify that the script exits 0 on main.

### X-DOC-05 INVARIANTS.md is a 378 KB table with 7 KB single-line rows, a merge and review hotspot

Severity: medium · Category: maintainability · Effort: L

Locations: [INVARIANTS.md](../../../../INVARIANTS.md), lines 49, 152 and 161.

Evidence: `wc -c` gives 377,886 bytes. The longest line is 7,072 characters, and 46 lines are over 2,000 characters.

Impact: Any edit shows the whole row as changed in a diff. Concurrent edits conflict, and stale citations (X-DOC-04) are easy to miss inside the long rows.

Suggested action:
1. Split the file by owner area into `docs/invariants/*.md`. Give each invariant a stable ID (`INV-AREA-NNN`) and its own heading with Statement, History, Model boundary and Checked by fields.
2. Keep `INVARIANTS.md` as an index with one line per invariant that links to the full entry.
3. Point the X-DOC-04 checker at the new files.
4. To verify, confirm that no line in `INVARIANTS.md` exceeds about 300 characters (`awk 'length>300' INVARIANTS.md` is empty) and that the invariant count is unchanged.

### X-DOC-06 Two owner-action queues: AGENTS.md routes to the workspace NEEDS_OWNER.md while the repo keeps its own 2,177-line copy

Severity: medium · Category: docs · Effort: S

Locations:
- [AGENTS.md](../../../../AGENTS.md) AGENTS.md:130
- [NEEDS_OWNER.md](../../../../NEEDS_OWNER.md) NEEDS_OWNER.md:1
- Workspace [NEEDS_OWNER.md](../../../../../NEEDS_OWNER.md)
- [scripts/grid-soak.sh](../../../../scripts/grid-soak.sh) grid-soak.sh:36
- [scripts/qualification/later-markets.sh](../../../../scripts/qualification/later-markets.sh) later-markets.sh:17

Evidence:
- `AGENTS.md:130` says "Put those steps in the workspace `NEEDS_OWNER.md`".
- `openagents/NEEDS_OWNER.md` is still tracked. It has 2,177 lines and 146 `## ` sections, and it has had 192 commits since 2026-10-04.
- The workspace copy has 6,664 lines.
- Repo scripts point at the repo copy (grid-soak.sh:36, later-markets.sh:17).
- Entries have no checkbox or done marker.

Impact: The owner reads the workspace repo, so steps written only to the openagents copy can be missed. The file also grows without bound.

Suggested action:
1. Merge the 146 sections into the workspace `NEEDS_OWNER.md`, then commit and push `AtlantisPleb/workspace`.
2. Replace `openagents/NEEDS_OWNER.md` with a one-line pointer, and update the comments in the two scripts.
3. Give each entry a `- [ ]` checkbox, a date and its issue, and move checked entries to an archive section.
4. To verify, confirm that `wc -l openagents/NEEDS_OWNER.md` is under 10 and that `git grep -n NEEDS_OWNER scripts/` resolves to the workspace path.

### X-DOC-07 Docs indexes claim completeness they lack: catalog misses about 322 docs, docs/README does not link many top-level dirs, audits index misses recent audits

Severity: low · Category: docs · Effort: M

Locations: [docs/catalog.md](../../../../docs/catalog.md) catalog.md:3; [docs/README.md](../../../../docs/README.md) README.md:72; [docs/audits/README.md](../../README.md); [docs/documentation.md](../../../../docs/documentation.md) documentation.md:35.

Evidence:
- `docs/catalog.md:3` says "A complete path inventory of Markdown documents under `docs/`, excluding `docs/transcripts/`". It has 528 unique `.md` links, but there are 850 tracked `.md` files outside audits, history and transcripts.
- `docs/README.md` does not link `desktop/`, `mobile/`, `web/`, `nostr/`, `release/`, `launch/`, `repo/`, `qa/`, `promises/`, `psionic/`, `worktrees/`, `dependencies.md` or `project-board.md`.
- `docs/audits/README.md` has no match for 2026-10-04, 2026-10-05 or receipts, although `docs/audits` contains `2026-10-04-verse-engine-audit.md`, `2026-10-05-grid-multiplayer-audit.md` and `receipts/`.
- `docs/documentation.md:35` (rule 5) requires updating the index and catalog on change.

Impact: Readers who trust the index and catalog miss current guidance, and nothing checks the "complete" claim automatically.

Suggested action:
1. Add `scripts/dev/docs-index-check.sh`. It should diff `git ls-files 'docs/*.md'` (minus transcripts) against the catalog links, and check that every top-level `docs/` entry is linked from `docs/README.md`.
2. Backfill the missing entries, starting with the domain READMEs.
3. Add rows to `docs/audits/README.md` for 2026-10-04, 2026-10-05 and `receipts`.
4. Drop the word "complete" from `catalog.md:3`, or run the script as part of the docs-change check. To verify, the script should exit 0.

### X-DOC-08 2026-09-19 remediation register is stale: it says A17/A18/#9427 are open, but #9427 and #9413 are closed

Severity: low · Category: docs · Effort: S

Locations: [remediation.md](../../2026-09-19-codebase-audit/remediation.md) remediation.md:39, remediation.md:89; [docs/audits/2026-09-19-codebase-audit/README.md](../../2026-09-19-codebase-audit/README.md) README.md:60.

Evidence: `remediation.md:39-40` says "A17 and A18 remain open", and line 89 says "#9427 and the #9413 blockers remain open". `gh issue view` shows #9427 closed at 2026-09-20T12:07:23Z and #9413 closed at 2026-09-20T17:48:22Z.

Impact: Anyone who uses the register to judge whether delegation is safe gets an outdated answer.

Suggested action:
1. Prepend a dated "Status at `<rev>`" table to `remediation.md`, with one row per item A01–A25: the closing issue, the closing commit, and a pointer to the code or test that holds the fix.
2. Mark A24 as "regressed outside training/" (X-DOC-09), A02 as "new microcoder-loop path not on supervise" (X-DOC-03), and A23 as "partly regressed" (X-DOC-10).
3. Leave the original text unchanged below the table, as the retained-evidence policy requires.
4. To verify, check that every issue the table marks closed shows CLOSED in `gh issue view`.

### X-DOC-09 A24 regression: 603 Python bytecode files are tracked because the ignore rule covers only training/

Severity: low · Category: repo-hygiene · Effort: S

Locations:
- [.gitignore](../../../../.gitignore) .gitignore:10, .gitignore:11
- [scripts/blender/__pycache__/kit.cpython-313.pyc](../../../../scripts/blender/__pycache__/kit.cpython-313.pyc)
- [scripts/tests/__pycache__/test_pay_reconcile_unit.cpython-313.pyc](../../../../scripts/tests/__pycache__/test_pay_reconcile_unit.cpython-313.pyc)
- [docs/verse/captures/.DS_Store](../../../../docs/verse/captures/.DS_Store)

Evidence:
- `git ls-files | grep -cE '\.py[co]$'` returns 603.
- All of them are under `bench/` except `scripts/blender/__pycache__/kit.cpython-313.pyc` and `scripts/tests/__pycache__/test_pay_reconcile_unit.cpython-313.pyc`.
- `.gitignore:10-11` covers only `/training/**/__pycache__/` and `/training/**/*.py[co]`.
- `docs/verse/captures/.DS_Store` is tracked even though `.gitignore:41` lists `.DS_Store`.

Impact: Machine-specific bytecode keeps landing in commits.

Suggested action:
1. Change the `.gitignore` rules to global `__pycache__/` and `*.py[co]` patterns.
2. Run `git rm --cached` on the two `scripts/` `.pyc` files and on `docs/verse/captures/.DS_Store`.
3. Decide whether the 601 bench `.pyc` files are evidence. Either remove them, or keep them under a commented negated ignore rule.
4. To verify, confirm that `git ls-files | grep -E '\.py[co]$|\.DS_Store$'` lists only the paths you chose to keep.

### X-DOC-10 A23 package policy partly regressed: 6 workspace crates skip lint inheritance and 3 declare the invalid license 'CC-0'

Severity: low · Category: build · Effort: S

Locations:
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml): lines 4 and 5
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml): line 5
- [crates/bitcoin-amount/Cargo.toml](../../../../crates/bitcoin-amount/Cargo.toml): lines 6 and 12
- [crates/rust-native/Cargo.toml](../../../../crates/rust-native/Cargo.toml): line 36
- [crates/oa-copy/Cargo.toml](../../../../crates/oa-copy/Cargo.toml)
- [crates/openagents-ui/Cargo.toml](../../../../crates/openagents-ui/Cargo.toml)
- [Cargo.toml](../../../../Cargo.toml): line 30
- [LICENSE](../../../../LICENSE): line 1

Evidence:
- A loop over `crates/*/Cargo.toml` finds no `[lints] workspace = true` in six crates: bitcoin-amount, oa-copy, openagents-ui, rust-native, wallet and x402. psionic and openagents-mobile are excluded workspaces and are not counted.
- wallet and x402 have no lints section at all. bitcoin-amount and rust-native use hand-written `[lints.rust]`/`[lints.clippy]` sections.
- wallet, x402, bitcoin-amount and rust-native hard-code `edition = "2024"`.
- wallet, x402 and bitcoin-amount declare `license = "CC-0"`, which is not a valid SPDX identifier. `LICENSE` is Apache 2.0.
- The workspace lints (Cargo.toml:30-40) deny `dbg_macro`, `todo` and `unimplemented`.

Impact: The payment crates wallet and x402 skip the workspace deny rules, and their license metadata contradicts the repo license.

Suggested action:
1. Add `[lints] workspace = true` to the six crates. Keep overrides that are needed, such as bitcoin-amount's `forbid(unsafe_code)`.
2. Switch `edition` and `rust-version` to `.workspace = true`.
3. Delete the `CC-0` lines or set them to `Apache-2.0`.
4. Add a check to `scripts/verify-rust.sh` that fails when a member lacks lint inheritance. To verify, run `cargo metadata --no-deps` and confirm that no package reports license `CC-0`. Also run `cargo clippy -p wallet -p x402` and confirm it passes under the workspace lints.

### X-DOC-11 Contradictory theme docs: AGENTS.md, docs/README.md and architecture.md still describe the amber palette that Coder Noir replaced

Severity: low · Category: docs · Effort: S

Locations:
- [AGENTS.md](../../../../AGENTS.md): AGENTS.md:695, AGENTS.md:700
- [docs/README.md](../../../../docs/README.md): README.md:66
- [docs/coder/rust-native/architecture.md](../../../../docs/coder/rust-native/architecture.md): architecture.md:228
- [crates/coder-ui/src/coder_noir.rs](../../../../crates/coder-ui/src/coder_noir.rs): coder_noir.rs:1
- [crates/verse/src/app.rs](../../../../crates/verse/src/app.rs): app.rs:270

Evidence:
- `AGENTS.md:695-700` says "a Tron-style city drawn in amber lines… a palette test protects its amber geometry".
- `docs/README.md:66` says "Coder's amber palette".
- `architecture.md:228` says "The amber theme preserves current product identity."
- The code says otherwise. `coder_noir.rs:1` reads "Coder Noir: Coder's neutral accents over Superlogical's "Static Noir" palette", and `app.rs:270` keeps amber only as the `VERSE_PLAZA_LEGACY` comparison path.

Impact: Agents may protect or reintroduce the old palette.

Suggested action:
1. Rewrite `AGENTS.md:695-700` to describe the Noir/neutral plaza, and name the palette test that exists today.
2. Change `docs/README.md:66` and `architecture.md:228` to say Coder Noir.
3. Review the output of `git grep -n -i amber -- AGENTS.md docs/README.md docs/coder`. Keep only mentions that are explicitly historical or legacy.

### X-DOC-12 Planning docs disagree on what is current: master roadmap omits the V1 Launch board, and the migration tracker is two weeks old

Severity: low · Category: docs · Effort: S

Locations: [docs/roadmap.md](../../../../docs/roadmap.md) roadmap.md:3; [docs/coder/migration-status.md](../../../../docs/coder/migration-status.md) migration-status.md:3; [AGENTS.md](../../../../AGENTS.md) AGENTS.md:134; [docs/README.md](../../../../docs/README.md) README.md:72.

Evidence:
- `docs/roadmap.md:3` says "Updated October 5, 2026… This is the single cross-project roadmap", but `grep -c 'projects/22\|V1 Launch'` on it returns 0.
- `AGENTS.md:134` requires keeping the V1 Launch board (project 22) current.
- `docs/coder/migration-status.md:3` says "Updated September 26, 2026", yet `docs/README.md:72` says "The migration tracker owns…" issue claims.

Impact: Agents get conflicting answers about current priority, and the claims tracker they are sent to is stale.

Suggested action:
1. Add a V1 Launch section to `docs/roadmap.md` that links project 22 with its date and scope, and update the date in the header.
2. Either mark `migration-status.md` "historical as of 2026-09-29" or bring it up to date.
3. Move issue-claim ownership in `docs/README.md:72` to the project boards that `AGENTS.md:134` mandates.
4. To verify, confirm that `grep -c 'projects/22' docs/roadmap.md` returns at least 1.

### X-DOC-13 docs/audits holds 22 MB of raw evidence in one audit (1,306 files of logs, patches and tarballs)

Severity: low · Category: repo-hygiene · Effort: M

Locations:
- [docs/audits/2026-10-03-independent-efficiency](../../2026-10-03-independent-efficiency)
- [native-evidence.tgz](../../2026-10-03-independent-efficiency/jev-native-pilot/native-evidence.tgz)
- [docs/audits/receipts](../../receipts)
- [.gitignore](../../../../.gitignore): .gitignore:43
- [docs/documentation.md](../../../../docs/documentation.md): documentation.md:29

Evidence:
- `du` puts `docs/audits` at 24M, of which `2026-10-03-independent-efficiency` is 22M across 1,306 tracked files (474 log, 472 json, 212 patch, 105 md, 17 py).
- `native-evidence.tgz` is 3,404,315 bytes, and `docs/` as a whole is 138M.
- `.gitignore:43-44` sends bench captures to `gs://openagents-bench-artifacts` (#11110, added 2026-10-09 in `8b03a43cdf`), but that rule does not cover `docs/`.
- `docs/audits/receipts` holds three 2026-10-05 grid files and has no index entry.

Impact: Clone size and grep noise grow with every evidence run.

Suggested action:
1. Going forward, extend the #11110 bucket policy to `docs/` evidence folders over about 1 MB or 100 files. Upload `.log`, `.patch` and `.tgz` files with `scripts/bench-artifacts.py`, and commit the manifest plus summaries.
2. Amend rule 3 in `docs/documentation.md` to allow external bytes addressed by a manifest.
3. Move `docs/audits/receipts` into a `2026-10-05-grid-multiplayer-audit/` folder and fix the inbound links.
4. To verify, run `du -sh docs/audits` on the next evidence-heavy audit and confirm that only manifests and summaries were committed.

### X-DOC-14 Broken links in live docs, crate READMEs and NIPs: moved Everglade code, missing coder-one files, Buzz paths, wrong relative depth

Severity: low · Category: docs · Effort: S

Locations:
- [docs/verse/workshop-agent.md](../../../../docs/verse/workshop-agent.md): workshop-agent.md:201
- [docs/terminal-bench/2026-09-22-luna-jevprobe-upgrade.md](../../../../docs/terminal-bench/2026-09-22-luna-jevprobe-upgrade.md): luna-jevprobe-upgrade.md:55
- [crates/gateway/tests](../../../../crates/gateway/tests)
- [crates/lev/manifests/README.md](../../../../crates/lev/manifests/README.md): README.md:17
- [nips/block/NIP-FI.md](../../../../nips/block/NIP-FI.md): NIP-FI.md:193
- [nips/block/NIP-MP.md](../../../../nips/block/NIP-MP.md): NIP-MP.md:21
- [docs/verse/agent-trainer-leveling.md](../../../../docs/verse/agent-trainer-leveling.md): agent-trainer-leveling.md:146

Evidence:
- `workshop-agent.md:201` links `crates/verse/src/zones/everglade/studio.rs`, which does not exist.
- `luna-jevprobe-upgrade.md:55-56` links `crates/coder-one/src/judge.rs` and `delegate.rs`. The crate exists, but neither file does.
- `crates/gateway/tests/decision_offer_budgets.rs` does not exist.
- `crates/lev/manifests/README.md:17` links `../../docs/lev/revocation.md`. That resolves under `crates/docs/`, but the file is at `docs/lev/revocation.md`.
- `NIP-FI.md:193` links `../../crates/buzz-auth/…`, and `NIP-MP.md:21` links `../../VISION_PROJECTS.md`.
- `agent-trainer-leveling.md:146` documents `openagents quests`. `quests` is dispatched only inside `world::run` (`crates/openagents-cli/src/world.rs:508`), which `main.rs:375` maps to `openagents verse`.

Impact: Readers who follow runbooks and published NIPs hit dead links.

Suggested action:
1. Fix each link to point at the current file. For removed code, use a permalink pinned to a commit.
2. Use `../../../docs/lev/revocation.md` in the lev READMEs.
3. In the NIPs, link the upstream Buzz repo at a pinned commit.
4. Change the command in `agent-trainer-leveling.md` to `openagents verse quests`.
5. Add `scripts/dev/check-md-links.py`. It should resolve relative links per file and skip site-absolute routes, fixtures and upstream NIP numbers. To verify, it should report 0 unresolved links outside its allowlist. The current count is about 53.

### X-DOC-15 Weak public-API docs in key crates (coder-ui 14%, nostr-relay 37%, pay-ledger 46%) and no missing_docs lint anywhere

Severity: low · Category: docs · Effort: M

Locations: [crates/coder-ui/src](../../../../crates/coder-ui/src); [crates/nostr-relay/src](../../../../crates/nostr-relay/src); [crates/pay-ledger/src](../../../../crates/pay-ledger/src); [crates/boat/src](../../../../crates/boat/src); [Cargo.toml](../../../../Cargo.toml) Cargo.toml:30.

Evidence:
- `git grep -l missing_docs` over the `Cargo.toml` files and `lib.rs` files (excluding psionic) returns nothing.
- `[workspace.lints]` at Cargo.toml:30-40 sets only `unsafe_op_in_unsafe_fn`, `unexpected_cfgs`, `linker_messages`, `dbg_macro`, `todo` and `unimplemented`.
- The per-crate coverage ratios come from the reviewer's scan of pub items and were not re-measured.

Impact: Callers of the money, relay and SDK APIs have to read the implementations to learn their invariants.

Suggested action:
1. Add `#![warn(missing_docs)]` to the `lib.rs` of pay-ledger, nostr-relay, boat and coder-ui.
2. Document their pub items until `cargo check -p <crate>` produces no `missing_docs` warnings.
3. Then consider a workspace-level `missing_docs = "warn"` with per-crate allows, and re-run the pub-item scan to track the coverage ratio.

### X-DOC-16 Historical surveys sit at the docs/ root, and docs/ Cargo manifests use machine-local absolute paths

Severity: low · Category: docs · Effort: S

Locations:
- Root docs, line 3 of each: [docs/coder-earn.md](../../../../docs/coder-earn.md), [docs/psionic-and-pylon.md](../../../../docs/psionic-and-pylon.md), [docs/text-optimization.md](../../../../docs/text-optimization.md), [docs/delegation-brief.md](../../../../docs/delegation-brief.md)
- [docs/documentation.md](../../../../docs/documentation.md): documentation.md:29
- [original-runner/Cargo.toml](../../../../docs/decision-models/2026-09-20-state-budget-choice/original-runner/Cargo.toml): Cargo.toml:6
- [recovery-runner/Cargo.toml](../../../../docs/decision-models/2026-09-20-state-budget-choice/recovery-runner/Cargo.toml)

Evidence:
- The four root docs start with "Status: historical…" or "Status: retained September 20 handoff".
- `docs/documentation.md:29` says to move superseded surveys to `history/`, which holds only 5 files.
- `docs/` tracks 4 `Cargo.toml` manifests and 28 `.rs` files.
- `original-runner/Cargo.toml:6-7` uses `path = "/Users/christopherdavid/work/openagents/crates/coder"`, and does the same for jev.
- 147 files under `docs/` contain `/Users/christopherdavid`.

Impact: The historical surveys look like current guides, and the reproduction programs cannot build on any other machine.

Suggested action:
1. `git mv` the four docs into `docs/history/`, and leave stubs where they are widely linked.
2. Change the runner manifests to relative paths and add an empty `[workspace]` table. To verify, run `cargo check --manifest-path docs/decision-models/2026-09-20-state-budget-choice/original-runner/Cargo.toml` from a clean checkout.
3. Add a "pinned to `<rev>`; not compiled by the workspace" header to the `docs/**/*.rs` reproductions.

### X-DOC-17 Language-policy text in AGENTS.md does not cover the hand-written web JS and Blender/Unreal Python in the tree

Severity: low · Category: policy · Effort: S

Locations: [AGENTS.md](../../../../AGENTS.md) AGENTS.md:3, AGENTS.md:19; [crates/openagents-web/static](../../../../crates/openagents-web/static); [scripts/blender](../../../../scripts/blender); [scripts/unreal](../../../../scripts/unreal).

Evidence: `AGENTS.md:19-21` allows only "Retained Python training and acceptance tooling and shell orchestration" as exceptions to Rust. Outside bench, vendor and psionic, the tree tracks:
- 37 `.js` files, 11 of them in `crates/openagents-web/static` (possibly including vendored files)
- 67 `.py` files in `scripts/blender`
- 8 `.py` files in `scripts/unreal`

Impact: Agents cannot tell whether adding to `static/*.js` or `scripts/blender` is allowed.

Suggested action:
1. Add a "Non-Rust exceptions" table to `AGENTS.md`. It should name `crates/openagents-web/static`, `crates/openagents-ui/static`, `scripts/blender`, `scripts/unreal` and the plugin eval fixtures, each with its reason.
2. Add a check to `scripts/verify-rust.sh` that fails on `.js`, `.py` or `.ts` files outside those globs. To verify, run it on main and confirm that it passes, then add a stray `.js` file and confirm that it fails.
