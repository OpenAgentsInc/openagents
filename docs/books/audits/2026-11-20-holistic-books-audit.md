# Holistic Books Audit (All 8 Books + First 6 Chapters)

- **Date:** 2026-11-20
- **Author:** Gemini 4 Neon (`antigravity`)
- **Snapshot:** `origin/main` at `9241da2df8`
- **Scope:** Cross-book health check across all **8 books** (`coax`, `metaverse-engine`, `everglade`, `psionic`, `coder`, `bitcoin`, `nostr`, `openagents`), reviewing `scripts/check-books.sh`, `psionic/tests/book.py`, `docs/books/README.md`, every `PLAN.md`/`SUMMARY.md`/`chapters.json`, and the **first 6 chapters** of each book.

---

## 1. Executive Summary & Library Scorecard

The 8-book architecture (*Bounded → Deterministic → Checkable → Payable → Ownable*) is one of the strongest intellectual assets in the repository. Two books—**`bitcoin/book`** (100/125 implemented) and **`coax/book`** (strongest onboarding)—are already close to publication grade, and **`psionic/book`**'s opening 6 chapters are the gold standard for the four-rule teaching contract.

However, rapid product consolidation over the last 10 days (retiring standalone apps `#13856`, promoting `coder-desktop` to `apps/openagents-desktop`, externalizing verification artifacts `#13840`, folding in GameBuilder `#13854`, shipping Breez Spark wallet on macOS `#13899`–`#13901`, and landing Antigravity + local Metal Inkling 2 Mini) has left **five systemic book problems** that need attention:

| Book | Chapters (`chapters.json`) | Status Breakdown | First-6 Health | Biggest Issue |
| --- | --- | --- | --- | --- |
| **Coax** (`coax/book`) | 47 | 21 impl, 10 partly, 16 planned | **Strong** (ch01–04 all impl, all 4 rules) | Not in `book.py`; no `PLAN.md`; `ch03-common-concepts.md` is 1,440 lines (too dense for chapter 3) |
| **Bitcoin** (`bitcoin/book`) | 125 | **100 impl**, 3 partly, 22 planned | **Gold standard** (ch01–06 all impl, all 4 rules) | Blocked by global `book.py` capability-claim crash; 22 Lightning/later chapters planned |
| **Psionic** (`psionic/book`) | 271 | 142 impl, 105 partly, 17 moved, 7 draft | **Gold standard** (ch01–06 all impl, all 4 rules) | 105 chapters stuck `partly implemented`; 17 `moved` tombstones; massive 271-chapter surface |
| **Nostr** (`nostr/book`) | 104 | **88 impl**, 1 partly, 15 moved | **Jarring opening** (starts at **Ch 14**, skips 15 & 18) | First chapter in `SUMMARY.md` is `14. Shared secrets with ECDH` after moving crypto to Bitcoin |
| **Metaverse Engine** (`metaverse-engine/book`) | 103 | 14 impl, 57 partly, 32 draft | **Mixed** (ch01–06 impl, but 3/6 lack expected transcripts) | Still called `aethyr` in `book.py` & `routes.json`; 57 `partly implemented` |
| **Coder** (`coder/book`) | 43 | 11 impl, 18 partly, 12 `written`, 2 draft | **Drifted** (ch03 & ch006 partly; ch005 lacks transcript) | Undocumented status `written`; `PLAN.md` disagrees with `chapters.json`; misses Antigravity, local Metal, `/goal`, Desktop daemon |
| **Everglade** (`everglade/book`) | **No `chapters.json`** (20 in SUMMARY) | Unregistered | **Weak** (essays: 5/6 lack transcripts, 2/6 lack broken cases) | Completely outside `book.py` & `chapters.json`; no `PLAN.md` |
| **OpenAgents** (`openagents/book`) | 78 | **0 impl**, 3 partly, 72 draft, 3 planned | **Stale & broken links** (0–2 code blocks; dead paths) | Cites deleted paths (`apps/openagents-desktop/chat_screen.coax`, `apps/openagents-ios/OpenAgents/Wallet`); caught mid-pivot between iOS-first and Desktop-first |

---

## 2. Systemic Issues Across All Books (Fix Generally)

### 2.1 `book.py` and `scripts/check-books.sh --check` are currently broken for every book
Running `python3 psionic/tests/book.py --book <ANY_BOOK>` currently crashes at line 127 (`AssertionError: reviewed capability claims`) before checking a single chapter:
- **3 missing evidence files** externalized out of git by `#13840` (`f7d036f4ff`):
  - `docs/verification/coax-quantity-psionic-compatibility-2026-10-31.json`
  - `docs/verification/coax-quantity-derived-merged-2026-10-31.json`
  - `docs/verification/coax-spatial-merged-2026-10-31.json`
- **9 changed test files** whose hashes drifted from the reviewed capability claims manifest (`coax/tests/{generics,semantic_profiles,reflection,modules,quantity,quantity_derived,spatial,spatial_045}.py`, `psionic/tests/run_group_kernels.sh`).

**Fix:** Re-review and refresh the capability claims manifest (or point the 3 externalized verification JSONs to their GCS/manifest record from `#13840`) so `scripts/check-books.sh --check` and `book.py` are green again and can gate CI/pre-push.

### 2.2 Incomplete book registry & naming drift
- `psionic/tests/book.py` only checks `{aethyr, bitcoin, coder, nostr, openagents, psionic}`:
  - **`coax`** and **`everglade`** are missing from `book.py`.
  - **`metaverse-engine`** is still addressed as `aethyr` in `book.py` and `docs/books/routes.json` (`aethyr-core-replay`).
- **`everglade/book`** has no `chapters.json` and no `PLAN.md`, making it invisible to automated status tracking.

### 2.3 Status vocabulary drift (`chapters.json` vs. `docs/books/README.md` vs. `PLAN.md`)
- `docs/books/README.md` defines four statuses (`implemented`, `partly implemented`, `design draft`, `planned`), plus `moved` in `psionic`/`nostr`.
- `coder/book/chapters.json` introduces a 5th undocumented status (`written`: 12 chapters), while `coder/book/PLAN.md` still labels chapters 1–8 `partly implemented` even though `chapters.json` marks chapters 1, 2, 4, and 5 `implemented`.
- **Fix:** Enforce one canonical status enum in `book.py` (`implemented | partly implemented | design draft | planned | moved`), and generate or check `PLAN.md`/`SUMMARY.md` status badges directly from `chapters.json` so they can never drift.

### 2.4 Four-rule compliance gaps in the first 6 chapters
`docs/books/README.md` states that a teaching chapter requires all four elements: (1) hand calculation/trace, (2) runnable source + exact expected transcript, (3) broken case + error/refusal, (4) independent check.
- **Passing cleanly:** `coax` (ch01–04), `bitcoin` (ch01–06), `psionic` (ch01–06), `nostr` (ch014–020, though starting at 14).
- **Missing expected transcripts in first 6:** `metaverse-engine` (`ch002`, `ch004`, `ch006`), `coder` (`ch005`), `everglade` (5 of 6).
- **Missing runnable code in first 6:** `openagents` (`ch001`, `ch003`, `ch004`, `ch005` have 0 code blocks).

---

## 3. What Deserves Specific Attention (Prioritized by Leverage)

### Priority 1 — Re-ground `openagents/book` to the real Desktop + iOS + Wallet architecture
`openagents/book` has **0 of 78 chapters implemented** and is caught mid-rewrite across three pivots (Nov 10 Episode 289 → Nov 13 Coax Native → Nov 15 iOS GPU → Nov 19 Desktop consolidation):
- **Broken references to delete/fix immediately:**
  - `openagents/book/ch001-a-text-input-on-the-gpu.md:5` cites `apps/openagents-desktop/chat_screen.coax` (**deleted** during Desktop consolidation).
  - `openagents/book/ch001-a-text-input-on-the-gpu.md:70-77` still describes `UITextField` and `UIAccessibilityElement` (iOS-only) inside a chapter whose top paragraph was patched to say *"grounded in the primary desktop app"*.
  - `openagents/book/PLAN.md:37` cites `apps/openagents-ios/OpenAgents/Wallet` (**deleted** in `269227265a`), whereas the real Spark wallet now lives in `apps/shared/host/apple/{SparkWallet,WalletKeychain,BIP39,BreezKey}.swift`, `apps/openagents-desktop/Window+Wallet{,RPC}.swift`, and `pay/wallet{,-backup,-rpc}.coax` (`#13899`–`#13901`).
  - `docs/books/README.md:16` still describes `openagents/book` as *"The iPhone app on compiled Coax... then desktop"*, contradicting `ch001`'s Desktop-first statement.
- **Streamlining opportunity:** Collapse the 72 prose design drafts in `openagents/book` into a tighter, real-code spine anchored on what actually ships today:
  - **Part I (Chapters 1–6):** The shared GPU app (`apps/openagents-desktop` + `apps/openagents-ios` over `components/`), resident `coder-host` daemon, and `Window+Wallet` / `pay/wallet*.coax`.
  - **Part II (Chapters 7–14):** The Episode 289 composable general agent (`Agents` page `#13894`, Thread Proof/ATIF `#13895`, Capability Authoring & Two-Arm Eval Loop `#13896`, Nostr Identity & Lightning Spend Approvals `#13897`).

### Priority 2 — Fix `nostr/book`'s jarring "starts at Chapter 14" opening
Because chapters 1–13, 15, and 18 moved to `bitcoin/book`, `nostr/book/SUMMARY.md` opens with `14. Shared secrets with ECDH`, then jumps `16 → 17 → 19 → 20`.
- **Two clean fixes (pick one):**
  1. **Add a crisp Chapter 0 ("Prerequisites from Bitcoin From Scratch")** that bridges readers cleanly from Bitcoin Parts I–V into ECDH/NIP-19/NIP-44, and **renumber Nostr cleanly `1..89`** with a one-table legacy-number map in `PLAN.md` (just as `openagents/book/PLAN.md` already does!).
  2. Or keep the numbers for link stability, but add a short **Part 0: Cryptography (in Bitcoin From Scratch)** section in `SUMMARY.md` with direct links to the exact Bitcoin chapters so the table of contents reads sequentially instead of looking broken.

### Priority 3 — Update `coder/book` to match the real harness you actually ship
`coder/book` (43 chapters) is well-written, but its `PLAN.md` still reflects the Nov 1 proposal:
- **Sync statuses:** Replace the undocumented `written` status in `chapters.json` and align `PLAN.md` with `chapters.json`.
- **Finish the opening 6:** Upgrade `ch003` and `ch006` from `partly implemented` to `implemented`, and add an exact expected transcript to `ch005`.
- **Update Part II (Engines) & Part III (Many tasks):** Document **Antigravity (`agyworker.coax`, `#13831`–`#13839`)**, **local Metal inference (Inkling 2 Mini / `llama-server` / Psionic Metal)**, **Git-backed per-turn checkpoints (`#13872`)**, **universal permission/plan cards (`#13873`)**, **`/goal` verification turns (`649c31f297`)**, and **ATIF-v1.8 canonical traces (`#13830`)**.
- **Move `psionic/lib/harness/`:** `coder/book/PLAN.md` notes that chapters 1–8's code still lives in `psionic/lib/harness/` waiting to move to `coder/`.

### Priority 4 — Bring `everglade/book` into the real book system (and align with GameBuilder)
`everglade/book/README.md` was already thoughtfully updated to use **GameBuilder** (`gamebuilder new-game`, `world.edit.v1`), which aligns with `#13854` and Episode 297.
- Add `everglade/book/chapters.json` and `everglade/book/PLAN.md`, and register `everglade` in `psionic/tests/book.py`.
- Upgrade its first 6 chapters from short essays (133–265 lines, mostly missing expected transcripts and broken cases) into full 4-rule teaching chapters backed by real `gamebuilder` CLI commands and exact expected output.

### Priority 5 — Trim & tighten `coax/book` and `metaverse-engine/book`
- **`coax/book`:** Register in `book.py`, add a `PLAN.md`, and consider splitting `ch03-common-concepts.md` (**1,440 lines**) into two focused chapters (e.g., *Values, Types & Control Flow* and *Memory, Slices & Fixed Buffers*) so new readers don't hit a wall on Chapter 3.
- **`metaverse-engine/book`:** Rename the `aethyr` CLI/route key to `metaverse-engine` (or keep `aethyr` as an alias), add expected transcripts to `ch002`, `ch004`, and `ch006`, and burn down the 57 `partly implemented` chapters along the 4 completion milestones in `PLAN.md`.

### Priority 6 — Rationalize `psionic/book`'s 271-chapter sprawl
`psionic/book` is massive (271 entries: 142 implemented, 105 partly implemented, 17 moved, 7 draft). Its first 6 chapters are exemplary, but 105 `partly implemented` chapters + 17 `moved` stubs make it hard for a reader to tell what is core vs. experimental model target.
- Clearly split the **Core ML Spine** (Parts I–X: tensors, autograd, attention, quantization, Metal/CUDA kernels, serving, training) from a **Model Targets Appendix** (Granite, Parakeet, Whisper, GPT-OSS, Inkling 2 Mini Part XXVII) so adding new models doesn't inflate the main book past 300 chapters.

---

## 4. Recommended Order of Work

1. **Unblock the checker first:** Repair the 3 externalized `#13840` evidence paths + 9 drifted test hashes so `scripts/check-books.sh --check` and `book.py` pass across all books, and register `coax` + `everglade` in `book.py`.
2. **Fix broken links & drift in `openagents/book` and `coder/book`:** Remove dead paths (`chat_screen.coax`, `apps/openagents-ios/OpenAgents/Wallet`), align Desktop-first + Spark Wallet + Antigravity + ATIF reality, and eliminate the undocumented `written` status.
3. **Fix `nostr/book`'s opening** (add Part 0 / renumber cleanly so readers don't start at Chapter 14).
4. **Upgrade `everglade/book` + `metaverse-engine/book` first 6 chapters** to 100% four-rule compliance (runnable code + exact expected transcripts + broken cases + `everglade/book/chapters.json`).
5. **Split `coax/book/ch03`** (1,440 lines) and separate `psionic/book`'s Model Targets Appendix from the core ML spine.
