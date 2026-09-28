# Tassadar and Percepta: complete code and documentation history

**This document records history.** Tassadar was retired on 2026-07-08, and no
Tassadar code remains in this repository. This file pulls together, in one
place, every piece of code and documentation related to Tassadar or Percepta
that ever existed in `OpenAgentsInc/openagents`, along with the matching work
in the sibling `OpenAgentsInc/psionic` repository. It does not describe
current direction. For that, see the [master roadmap](../roadmap.md).

For a proposal on which Tassadar patterns to bring back into the current
Gym, MVP, and Verse/XP plans, see
[What to bring back from Tassadar](../roadmap/2026-09-28-tassadar-revival.md).

Surveyed 2026-09-28 from git history:

- openagents at `3943b45f62`: 352 commits whose messages mention
  Tassadar/Percepta, and about 605 paths ever touched.
- psionic at `02e0bc85`: 255 commits by message, 336 by path, and about 2,955
  tracked files that still mention it.

Most of the openagents material is deleted. Every path and hash below can be
recovered with `git show <deleting-commit>^:<path>`. The deleting commits are
listed in [How to recover deleted material](#how-to-recover-deleted-material).

Private material also exists in `alpha`: the live Tassadar roadmap
`alpha/tassadar/tassadar-llm-as-computer-roadmap.md` and the paper corpus
under `alpha/tassadar/`. Psionic's roadmap bridge and `AGENTS.md` point to
these files. They are not summarized here, following the rule that separates
public and private material.

---

## Summary

**Percepta** is the research lab behind two posts. **"Can LLMs Be
Computers?"** (Christos Tzamos et al., 2026-03-11) showed that a program can
be *compiled*, not trained, into the weights of an ordinary transformer. The
model then runs the program exactly, one token at a time, for millions of
steps. **"Constructing an LLM-Computer"** (2026-03-25) explained how, and
released the open-source `Percepta-Core/transformer-vm`. The key ingredients
are:

- an append-only execution trace
- the five-primitive *Append-only Lookup Machine* (ALM)
- 2D parabolic-key hard-max attention as exact memory
- convex-hull (HullKVCache) decoding in O(log t)
- a MILP gate scheduler
- the first Futamura projection, which bakes a fixed program into the FFN

**Tassadar** was the OpenAgents program that built on this work. The name
covered several things over time:

1. psionic's executor-class model/runtime lane, the "exact, compiled" lane;
2. the Rust ALM compiler campaign;
3. the public, Bitcoin-paid network run `run.tassadar.executor.20260615`
   ("the Tassadar Run");
4. a family of product promises, Worker routes, Pylon verbs and 3D/Verse
   surfaces around (3).

The unifying thesis was *"a computation that is exact is a computation that
can be verified by replay"*: the trace is the receipt.

The program went through six phases:

| When | Where | What happened | Outcome |
|---|---|---|---|
| 2026-03-15 → 03-16 (~22 h) | openagents `crates/psionic` (in-tree) | Executor substrate phases 1–9D; trained-executor phases 1–11; learned 4x4 promotion phases 12–18 | Compiled executors exact (Sudoku-v0 and Hungarian-v0 4x4, 8/8 traces). Learned executors never passed the in-tree gate (best 7188 bps first-32, 0/2 exact). Psionic extracted to its own repo at `feb4d45007` two hours after the last commit. |
| 2026-03-17 → 04-02 | psionic | ~219 commits: module-scale Wasm, article parity, Rust-only closeout, Turing-completeness closeout, article-equivalence final audit (TAS-186), post-article plugin platform (TAS-187…226), Psion executor lane (PSION-0001…0808), `TRAIN_TASSADAR` | Bounded green claims: 24/24 article lines, Percepta closeout `green_bounded`. Heavy caveat: the fast "HullCache" route is a Rust interpreter over fixture rule tables, not transformer inference. |
| 2026-03-17 → 03-25 | openagents | Autopilot desktop "Tassadar Lab" replay pane over pinned psionic | Deleted in the Bun rebuild, `f5919c7669` (2026-06-09). |
| 2026-06-10 → 06-13 | both | Docs folder `docs/tassadar/`; Rust ALM executor-compiler E1–E6 (psionic #1098–#1114); TS executor with Rust↔TS digest parity; first live Pylon closeout with a Worker replay validator; trace factory; W3 student sweep | `compute.tassadar_executor_poc.v1` green on 06-10. W3 result: only "frozen exact core + learned interface" works (H2 supported; H1 learned-exactness fails). |
| 2026-06-14 → 07-04 | openagents | The Tassadar Run: run authority, admission, self-serve submit/validate, pairing, real Bitcoin settlement over Spark, auto-stream 5+5 sats, settled feed, `/run` and `/tassadar` 3D pages, Verse run board, Blueprint module steps, module marketplace, Percepta receipts | Peak: 5 independent contributors paid, 1,020 real sats, 11–12 accepted traces. The run re-verified one trivial program (`loop_sum_v1`) and "constructed no new capability". World-first claims stayed red. |
| 2026-07-08 → 09-18 | openagents | Retirement by owner mandate (`5f87fd63a2`): focus on Khala Code and business. Banners (`97b600f4d7`), then prune (`e1fbd1c185`, −337k lines, archived to backroom `a56fd270`), TS deletion (`d613b8ea22`, `fae80bde79`, 08-28), "Nuke" of satellites (`dabc08102f`, 09-18) | No Tassadar code remains in openagents. psionic still carries all of it at HEAD (not feature-gated), untouched since `4f36914f` (2026-06-23). |

**Claims set against the evidence.** The public messaging went further than
the repositories' own audits:

- **Transcript 216** says "we've more or less reproduced the Percepta paper".
  The in-tree audits call article parity "unearned".
- **Psionic's later closeouts** do go green, but only within tight bounds:
  - one tiny canonical model (hidden size 8, 52 KB of weights);
  - a direct Rust interpreter route whose link to the weights rests on parity
    certificates;
  - two CPU classes;
  - no arbitrary C/Wasm;
  - served universality suppressed.
- **Weights-shaped execution** reached 1.7k–9.3k steps/s, versus about 10M
  steps/s on the CPU reference.
- **The Tassadar Run** proved the verify-by-replay-and-pay mechanism end to
  end, at canary scale.

---

## Master timeline

| Date | Repo | Commit(s) | Event |
|---|---|---|---|
| 2026-03-11 | — | — | Percepta publishes "Can LLMs Be Computers?" |
| 2026-03-15 | oa | `1943b0125a` | Percepta adaptation audit: plan an executor-class lane in Psionic |
| 2026-03-15 | oa | `145cc2ac8d` | Phase 1: CPU reference executor fixture, `tassadar.wasm.core_i32.v1` |
| 2026-03-16 | oa | `6459ee3d13` … `66762cb36b` | HullCache fast path, served `psionic.executor_trace`, sparse top-k, planner route, compiled-weight path |
| 2026-03-16 | oa | `9696a427ab` | Trained-executor gap audit: "an honest executor substrate … not yet a trained in-model executor" |
| 2026-03-16 | oa | `3090ccf707` | First learned run: 0/2 exact, 15 bps |
| 2026-03-16 | oa | `7fb1983bc3`, `52eced170b` | Compiled Sudoku-v0 and Hungarian-v0 bundles: 8/8 exact, 32/32 refusals |
| 2026-03-16 | oa | `fa80adcf81` | Attention adapter family declared saturated (7188 bps first-32 ceiling) |
| 2026-03-16 | oa | `feb4d45007` | Psionic extracted; openagents consumes it via pinned git deps |
| 2026-03-16 | ps | `38cb0184` | "Close the 4x4 promotion gate" (`sudoku_v0_promotion_v3`, first-32 exactness) |
| 2026-03-17 | ps | `7ea89dbf`, `4c4f9ac6`, `1cf1c79a` | `ROADMAP_TASSADAR.md`; article-parity closeout "green" |
| 2026-03-17 | oa | `1cafa430ec` … `77174ad7c1` | Autopilot desktop Tassadar Lab pane |
| 2026-03-18 | ps | — | Rust-only article closeout; TAS-084…102 queue closed |
| 2026-03-19 | ps | `92e97913` | Turing-completeness closeout: theory and operator green, served suppressed |
| 2026-03-21 | ps | `9d1d5201` | TAS-186 article-equivalence final audit: 24/24 lines, claim allowed (bounded) |
| 2026-03-21/22 | ps | TAS-187…226 | Post-article universality rebase, plugin platform, starter plugins |
| 2026-03-25 | oa | `dc0f35c28f` | PR #4008: defer Tassadar Lab load (freeze fix) |
| 2026-03-26 | oa | `815263b70e` | Transcript 216: "more or less reproduced the Percepta paper" |
| 2026-03-30 | ps | `f03acff7`, `24291b45` | Psion executor lane; Percepta closeout `green_bounded`; roadmaps frozen as subordinate |
| 2026-04-02 | ps | `39608477`, `5f55a3b8` | Default train lane plus `./TRAIN_TASSADAR` launcher |
| 2026-06-08/09 | oa | `86625916df`, `f5919c7669` | March audits and desktop lab retired in the Bun rebuild |
| 2026-06-10 | oa | `833e5b9c46`, `abb3f2f932` | Cross-repo Tassadar/Percepta audit; `docs/tassadar/` created |
| 2026-06-10 | ps | `bd74e5e6` … `e9531d8a` | ALM executor-compiler E1–E6, bounded check (found 2 scheduler bugs), trace-replay class |
| 2026-06-10 | oa | `3704ba785a`, `7bf1f01c47`, `43d64fb8ae` | `@openagents/tassadar-executor`; first live Pylon closeout; PoC promise green |
| 2026-06-11 | oa | `5e15d1a65e`, `19ec46e4f6`, `f54c9b6a95` | RESEARCH_PLAN (H1–H6, W1–W5); trace-factory contract freeze; W3 100M corpus |
| 2026-06-12/13 | ps/oa | `7497713e`, `a1734d7204` | W3 student sweep: H1 and H2 supported, H3 falsified |
| 2026-06-14 | oa | `3f20f3d3e2` … `cdb44315e9` | Run authority for `run.tassadar.executor.20260615` (#5006–#5010) |
| 2026-06-15 | oa | `602e83e0b3`, `c8bcb888bc`, `20a354b326` | Launch (Episode 237): self-serve submit/verdict, pairing, Pylon verbs |
| 2026-06-16 | oa | `4344028a03`, `2341dd06df`, `0d344194cf` | Pairing live; first independent pair (5-sat receipt, later found to be simulated); `/run` page |
| 2026-06-17 | oa | `89b718fa1c`, `ffbcb76f62` | First real Bitcoin settlement (1,000 sats over Spark, #5232); auto-stream 5+5 sats |
| 2026-06-17/18 | oa | `17cd637240`, `7ba06168f7` | Run gap audit: "training" means construction, not gradient descent; the run runs one program forever |
| 2026-06-18 | oa | `5af4784f67`, `698abde52d`, `dc27cf007a`, `cb31d3c521` | Blueprint Tassadar module steps; module marketplace; adversarial verification market |
| 2026-06-18 | oa | `3be46e4edf`, `e738443e13` | Episode 238 "The Tassadar Run is Live"; Pylon v1.0.0 |
| 2026-06-19 | oa | `15f614f46f` | Registry: 5 contributors, 1,020 real sats |
| 2026-06-20 | oa | `c61088825f`, `8bfef42537`, `549570a273` | Percepta executor spec; architecture receipts; evolution-loop promise green |
| 2026-06-23 | ps | `4f36914f` | CPU-transform training receipt (0/2 exact). Last psionic Tassadar commit. |
| 2026-06-24 | oa | `db0494b0e7` | Khala × Blueprint × Tassadar marketplace fusion essay |
| 2026-06-27 | oa | `9036dcdbcf` | Artanis hand-off audit: Percepta "~60% built" |
| 2026-06-28 | oa | `b4ebdeb98a` | CPU-transform fixture receipts published (#6952) |
| 2026-07-04 | oa | `d084548b95` | TanStack Start port of `/tassadar` |
| 2026-07-08 | oa | `5f87fd63a2`, `97b600f4d7`, `e1fbd1c185`, `9bf6be5191` | Retirement mandate; banners; prune; executor package restored for the Worker build |
| 2026-08-28 | oa | `d613b8ea22`, `fae80bde79` | All TypeScript apps and packages deleted, including the remaining Tassadar code |
| 2026-09-18 | oa | `dabc08102f` | "Nuke": remaining satellite docs, `docs/RETIRED.md` and the retirement audit deleted |

(oa = openagents, ps = psionic. Psionic hashes for the March 15–16 commits
differ from openagents hashes because the history was rewritten on
extraction. Subjects and dates match.)

---

## How to recover deleted material

| Material | Deleted in | Recover with |
|---|---|---|
| In-tree `crates/psionic/**` (March Tassadar code, 556 MB of fixtures) | `feb4d45007` (2026-03-16) | `git show feb4d45007^:crates/psionic/<path>`, or the psionic repo itself |
| March audits `docs/audits/*tassadar*`, `docs/tassadar-lab.md`, desktop lab pane | `f5919c7669` (2026-06-09) | `git show f5919c7669^:docs/deprecated/audits/<file>` (moved there in `86625916df`) |
| `docs/tassadar/` (21 files), `docs/training/`, `docs/sakana/`, Pylon Tassadar modules, replay packages | `e1fbd1c185` (2026-07-08) | `git show e1fbd1c185^:<path>`; also backroom `openagents-prune-20260708-tassadar-psionic` @ `a56fd270` |
| `packages/tassadar-executor`, Worker Tassadar routes, web `/run` and `/tassadar`, all of `apps/pylon` | `d613b8ea22`, `fae80bde79` (2026-08-28) | `git show d613b8ea22^:apps/openagents.com/workers/api/src/<file>`, `git show fae80bde79^:packages/tassadar-executor/<file>` |
| Satellites (`docs/launch/`, `docs/artanis/`, `docs/khala/`, `docs/research/`, `docs/RETIRED.md`, retirement audit) | `dabc08102f` (2026-09-18) | `git show dabc08102f^:<path>` |

## Still live today

- **openagents:** only docs.
  - [glossary](../glossary.md) (Tassadar marked "Historical")
  - [Psionic and Pylon](../psionic-and-pylon.md)
  - [transcript roadmap](2026-09-25-transcript-roadmap.md)
  - [Verse](../verse/README.md) and [game](../game/README.md) READMEs (the Verse look derives from the Tassadar Run Board)
  - transcripts 203, 216, 220, 236–238, 240, 241, 243, 274, 275
- **psionic:** everything is present and compiles by default.
  - ~394k lines of `tassadar_*` Rust across 18 crates
  - 491 examples
  - 80 `check-tassadar-*.sh` scripts
  - `fixtures/tassadar/` (1,293 files, 737 MB)
  - 202 docs
  - `./TRAIN_TASSADAR`
  - the `psionic-tassadar-student` crate

## Known discrepancies across sources

- **Hull decode baseline.** Percepta's figure is cited as 31,037 vs 316 tok/s
  in the construction notes, but as 31,037 vs 702 in `RESEARCH_PLAN.md`.
- **4x4 learned promotion gate.** It stayed red in the openagents in-tree
  history (last in-tree result 6875/7188 bps first-32, 0/2 exact). psionic
  later closed it (`38cb0184`, `sudoku_v0_promotion_v3`). That result was
  first-32-token exactness, not full-trace exactness.
- **Payment per window.** The economics doc recommended 1 sat per verified
  window. The shipped code paid 5 sats to the worker plus 5 to the validator.
- **First "paid" pair.** The 5-sat receipt from 2026-06-16 was later
  reclassified as a simulation (`realBitcoinMoved:false`). The real totals
  are 1,005 and then 1,020 sats.
- **Retirement method.** The retirement audit said "never delete or move a
  file". The prune commit less than an hour later deleted the docs and code.
- **Psionic's `ROADMAP_TASSADAR_INDEX.md`** still says PTAS-003 article
  closure is "red", while the committed acceptance report is all green.
  `ROADMAP_TASSADAR_TAS_SYNC.md` has no rows for TAS-084 and TAS-085.

---

The five parts below are the detailed surveys:

- **Part I:** openagents from March to April 2026, including the in-tree
  psionic crates and the desktop lab.
- **Part II:** openagents docs from June 2026 on.
- **Part III:** openagents code from June 2026 on.
- **Part IV:** psionic code and fixtures.
- **Part V:** psionic docs.

Headings inside each part are nested one level down.


---

## Part I — openagents, March–April 2026: the in-tree era

Source: git history of `/Users/christopherdavid/work/openagents`. Commit bodies for
this work are empty, so the detail below comes from the in-tree docs, READMEs and
fixture JSON at the relevant commits. The last in-tree Psionic snapshot is
`feb4d45007^` (= `876300aa1a`). The end-of-April snapshot is `3c8f4a51c7`.

### 0. What happened, briefly

- All the in-tree Tassadar engineering happened in a roughly 22-hour burst:
  **2026-03-15 23:03 CDT to 2026-03-16 20:57 CDT**. That is 49 commits whose messages
  mention Tassadar or Percepta, plus 4 earlier audit/doc commits. Almost all were
  authored by Christopher David. Much of the late-March-16 work landed through the
  merge of branch `codex/tassadar-continue-20260316` (`c4c37b607d`, 20:57).
- **Two hours later Psionic left the repo.** Commit `feb4d45007` (2026-03-16 23:01,
  "repo: consume psionic via pinned git deps") deleted `crates/psionic/` (787 files,
  about 36.9M deleted lines, most of it fixture JSON). openagents then consumed
  `github.com/OpenAgentsInc/psionic` through pinned git revs. The first pin was
  `c61159d7…`. The Tassadar lab commit bumped it to `60abf060…`.
- On 2026-03-17, four commits added the Autopilot desktop **Tassadar Lab** pane on
  top of the external psionic crates. A freeze fix followed on 2026-03-25
  (PR #4008).
- The audits (`docs/audits/*tassadar*`, plus the Percepta adaptation audit) and the
  lab pane stayed in openagents through 2026-04-30. They were retired in June:
  - moved to `docs/deprecated/audits/` in `86625916df` (2026-06-08)
  - the lab app was moved under `apps/deprecated/` in `eb4b0fa4f1` (2026-06-08)
  - everything was deleted in the "Rebuild openagents as Bun Effect workspace"
    commit `f5919c7669` (2026-06-09)

### 1. Origin: the Percepta adaptation audit (2026-03-15)

- `docs/audits/2026-03-15-can-llms-be-computers-psionic-adaptation-audit.md`
  - Added in `1943b0125a` ("docs: add Psionic in-model execution audit",
    2026-03-15 23:03).
  - Updated in `8f44479c85`, `f6e05e4582`, `1c261a19bd` and `d40fe27fe2`.
  - About 6,100 words.
  - Its input was Percepta / Field Notes, "Can LLMs Be Computers?" (published
    2026-03-11), plus user-relayed statements from the author:
    - the weights are handcrafted, with a correctness proof tied to the
      WebAssembly spec
    - the aim is inner computational ability for LLMs, not replacing CPUs
    - native CPU execution is still orders of magnitude faster
    - learned or grown circuits (SUBLEQ-style) are a future step
- **Thesis.** Treat the article as a new *executor-class model/runtime lane* inside
  Psionic, not as a faster path for ordinary serving. The lane should be:
  - WebAssembly-first
  - CPU-reference-first
  - proof-bearing
  - benchmarked against a direct CPU Wasm runner
  - kept out of MVP compute-market scope
  - kept away from GPT-OSS and Apple FM retrofits
- **Ownership map.**

  | Crate | Owns |
  | --- | --- |
  | `psionic-models` | executor descriptors and trace ABI |
  | `psionic-core`, `psionic-ir` | hard-max and sparse attention semantics |
  | `psionic-runtime` | hull cache, proof bundles, decode identity |
  | `psionic-eval`, `psionic-environments` | Sudoku, Hungarian, arithmetic and micro-Wasm packages |
  | `psionic-serve` | `psionic.executor_trace` |
  | `psionic-router` | later planner/executor hybrid |
  | `psionic-research` | variant sweeps |

- **Issue spine.**
  - Phase 0: #3743 (closed)
  - Phase 1: #3744 (closed)
  - Phases 2–6: #3745–#3749
  - Umbrellas: #3741 and #3742
  - Related PLIB issues: #3707, #3709, #3710, #3713, #3714, #3720, #3723, #3732,
    #3735, #3736
  - The codename "Tassadar" was attached to the landed Phase 1 substrate.

### 2. Timeline, first spine: executor substrate (Phases 1–9D)

All of these commits touch `crates/psionic/*`.

| Date/time (CDT) | Commit | Phase | What landed |
|---|---|---|---|
| 03-15 23:31 | `145cc2ac8d` | 1 | CPU reference executor fixture. Adds `psionic-runtime/src/tassadar.rs` (2,037 lines) and `psionic-models/src/tassadar.rs` (401 lines). Profile `tassadar.wasm.core_i32.v1`. Opcodes: `I32Const, LocalGet, LocalSet, I32Add, I32Sub, I32Mul, I32Load, I32Store, BrIf, Output, Return`. `TassadarExecutorFixture` uses `WeightFormat::ProgrammaticFixture`, with exact parity against a direct CPU runner. |
| 03-15 23:43 | `eca02fa193` | 2 | Digest-bound program artifacts and executor compatibility descriptors (#3745). |
| 03-15 23:56 | `cba26e9b4d` | 3 | Benchmark and environment packages: `psionic-environments/src/tassadar.rs` (971 lines) and `psionic-eval/src/tassadar.rs` (690 lines) (#3746). |
| 03-16 00:03 | `1a9b988212` | 4 | Executor trace proof bundles (`tassadar.executor_trace.proof.v1`) and runtime-manifest lineage (#3747). |
| 03-16 00:17 | `6459ee3d13` | 5 | `HullCache` fast path (`tassadar.decode.hull_cache.v1`). Adds an exact CPU / reference-linear / hull three-way equivalence check, typed refusal for backward branches, and speedup plus CPU-gap reporting (#3748). |
| 03-16 00:26 | `41155fe07a` | 6 | Runtime capability report and direct/fallback/refused decode-selection diagnostics (#3749). |
| 03-16 00:45 | `57746794be` | 7A | `psionic-serve/src/tassadar.rs`: the `psionic.executor_trace` served product, with request/stream/terminal contract, typed refusals and evidence bundles. |
| 03-16 00:58 | `0f14d6719e` | 7B | `core_i32_v2` profile, capped at 128 instructions and 512 steps. Adds "article-class" cases `MicroWasmKernel`, `SudokuClass`, `HungarianMatching`. These were later admitted to be placeholders (see §3). |
| 03-16 01:08 | `04957c4f8e` | 8A | `psionic-research` executor-variant research family with sweep records. |
| 03-16 01:18 | `7124a8adce` | 8B | Sparse-top-k decode (`tassadar.decode.sparse_top_k.v1`), with explicit fallback on unsupported shapes. |
| 03-16 01:27 | `fe868f3a34` | 9A | `psionic.planner_executor_route` hybrid planner route in `psionic-serve`. |
| 03-16 01:38 | `239bac7560` | 9B | "Small-executor training" in `psionic-train/src/tassadar.rs`. |
| 03-16 01:56 | `b8e41ddfa0` | 9C | Compiled-weight path: `TassadarCompiledProgramExecutor` with program-specialized artifacts, plus larger-2D-head exploration in research. |
| 03-16 02:04 | `66762cb36b` | 9D | Learned-plus-compiled / "circuit" research family (research-only proxy surfaces). |

Two Phase 9 items were later downgraded by the reality audit in §3:

- **Phase 9B.** Only three scalar arithmetic kernels (`add`, `sub`, `mul`) were
  trained, from `BinaryOp` events in the validation corpus. Everything else still
  ran through handwritten Rust interpreter logic.
- **Phase 9C.** `TassadarCompiledProgramExecutor::execute` delegated back to the
  imperative runtime through `execute_tassadar_executor_request(...)`.

### 3. The "reality" audit and the second spine (trained-executor Phases 1–11)

- `docs/audits/2026-03-16-tassadar-trained-executor-gap-audit.md` was added in
  `9696a427ab` (02:15) and linked to issues in `d7eab9dd13`. About 3,400 words.
- **Verdict:** *"`Tassadar` is currently an honest executor substrate and simulator
  stack, not yet a trained in-model executor."*
  - All four runners are imperative Rust state machines over
    stack/locals/memory/pc: `TassadarCpuReferenceRunner`, `TassadarFixtureRunner`,
    `TassadarHullCacheRunner`, `TassadarSparseTopKRunner`.
  - The "weights" were fixture metadata.
  - The benchmark cases were placeholders:
    - `sudoku_class` was "sum-based exact completion for two missing values in a
      tiny 4x4 Sudoku-style instance"
    - `hungarian_matching` was a fixed 2x2 branch choice
  - Scale was far below the article: 128 instructions and 512 steps, against the
    article's million-step traces.
- **Epic #3776 and sub-issues #3777–#3787.** Everything below landed 03-16 between
  02:37 and 08:50.

| Phase | Issue | Commit | Result |
|---|---|---|---|
| 1 | #3777 | `63f1899baf` | `tassadar.wasm.sudoku_v0_search.v1` profile. A real 4x4 backtracking search program, exact on the CPU reference and outside the hull/sparse validated subset. |
| 2 | #3778 | `c88fbe8cd6` | Real split-aware 4x4 Sudoku-v0 corpus: 8 cases (train a–d, validation a–b, test a–b) with exact CPU traces. Replaced the fake `SudokuClass`. |
| 3 | #3779 | `0363dff6cb` | Fixed executor token vocabulary, deterministic program+trace tokenization, dataset manifests, packing plans (`psionic-data/src/tassadar.rs`). Dataset key `oa.tassadar.sudoku_v0.sequence@train-v0`. |
| 4 | #3780 | `32534efa0b` | First neural executor family `psionic-models/src/tassadar_executor_transformer.rs`. A "lookup-style" model with 2D lookup-head geometry claims, next-token logits, and a claim boundary of next-token-only. |
| 5 | #3781 | `749ab9be33` | Teacher-forced next-token training (`psionic-train`) plus exact-trace, final-output and halt eval (`psionic-eval/src/tassadar_executor_eval.rs`). |
| 6 | #3782 | `9664719edc` | Neural linear-decode benchmark against the CPU reference (`tassadar_executor_benchmark.rs`). |
| 7 | #3783 | `3090ccf707` | First persisted run at `fixtures/tassadar/runs/sudoku_v0_reference_run_v0`. Result: 0/2 validation exact traces, 15 bps aggregate target exactness. |
| 8 | #3784 | `3715283886` | Telemetry, exactness curve, divergence report and failure samples. All 8 cases diverge at target token 0, with per-case exactness of 9–16 bps. |
| 9 | #3785 | `0f4cb16f00` | `postmortem.json`, `next_run_plan.json`, and the human postmortem doc. |
| 10 | #3786 | `5826f0f4b8` | Neural hull-cache decode over real model-KV. Hull matches linear on 8/8 cases with 0 fallbacks. Throughput goes from **21,860 to 42,172 target tok/s** (about 1.93x) over a 4,096-token window. Exactness stays 0/8. |
| 11 | #3787 | `039d6db3e0` | 9x9 scale plan (`sudoku_9x9_scale_plan_v0/scale_plan.json`), described in the bullets below. |

Details of the Phase 11 9x9 plan:

- Profile `tassadar.wasm.sudoku_9x9_search.v1`.
- Only 4 cases: 2 train, 1 validation, 1 test, each with 53–55 givens.
- The program is 11,666 instructions.
- Prompt: 51,536 tokens.
- Targets: **4.84M–5.28M tokens per case**.
- The gate stayed closed: 4x4 first-target 0/2, exact-trace 0/2.
- 9x9 training was never promoted.

Findings from the first-run postmortem:

- Only 1 training step, 1 epoch and 1,024 supervised target tokens.
- Epoch mean loss 5,720 milli-loss.
- 4x4 target lengths ran from **114,913 to 205,350 tokens**.
- Baseline failure: the reference first token is `<step>` but the model predicted
  `<byte_50>`.

### 4. Third spine: the learned 4x4 promotion campaign (Phases 12–18)

The promotion gate, written in Phase 14:

- `first_target_exactness_bps = 10000`
- `first_32_token_exactness_bps > 9000`
- at least 1 exact validation trace

The gate is enforced by `scripts/release/check-psionic-tassadar-4x4-promotion-gate.sh`.

Issues:

| Phase | Issue |
| --- | --- |
| 13 | #3813 |
| 14 | #3814 |
| 15 | #3815 |
| 16 (honest 9x9 run) | #3816, never unblocked |
| 17 | #3817 |
| 18 | #3818 |

The window is the 2 validation cases of 4x4 Sudoku-v0 throughout.

#### Lookup-family runs

| Run (fixture dir) | Commit | Result |
|---|---|---|
| `sudoku_v0_boundary_v1` (Phase 12) | `c4f732b877` | See details below the table. |
| `sudoku_v0_trainable_surface_ablation_v1` (Phase 13) | `55bb4e1a76` | Compared four trainable surfaces. `output_head_only` 7431 / 10000 / 2500 / 5000 bps (aggregate / first / first-8 / first-32). `+token_embeddings` 7242 aggregate. `+embeddings` 7240 aggregate. **`output_head_embeddings_and_small_learned_mixer` 7439 / 10000 / 3750 / 5625**, the only winner. All four: 0/2 exact, divergence at index 1. |
| `sudoku_v0_promotion_v1` (Phase 14) | `9fc9b98e7e` | See details below the table. |
| `sudoku_v0_promotion_v2` | `ae059cc2f5` | Teacher-forced-only schedule. Best checkpoint `epoch_0008`, with exactly the same ceiling (6875 first-32, 0 exact). The predicted divergence token changed to `<byte_00>`. Conclusion: schedule churn is exhausted. |

Details of the Phase 12 boundary run (`sudoku_v0_boundary_v1`):

- Uses a boundary curriculum (first 1/2/4/8/16/32 tokens, then full trace), with
  checkpoints selected by boundary metrics.
- Selected checkpoint `epoch_0005`: aggregate **7431 bps**, first-target
  **10000**, first-8 2500, first-32 5000, 0/2 exact.
- Divergence moved from index 0 to 1: the reference wanted `<step_index>` and the
  model predicted `<step>`.
- Key finding: full-trace supervision at `epoch_0006` pushed first-32 to 6250 and
  aggregate to 7640, but *destroyed* token-0 (first-target fell to 0). That is why
  boundary-aware checkpoint selection became mandatory.

Details of the Phase 14 promotion run (`sudoku_v0_promotion_v1`):

- Run with `cargo run -p psionic-train --example tassadar_promotion_training_run`,
  taking about 659 s.
- Bundle digest `61d72a53…`.
- Best checkpoint `epoch_0006` at stage `prompt_to_first_16_tokens`: 10000
  first-target, 7500 first-8, **6875 first-32**, aggregate 7641, **0/2 exact**.
- `promotion_gate_report.json` says `passed=false`.
- Greedy-rollout refinement regressed first-target from 10000 to 0.
- Live progress logging was added in `5ac5697f3b`.

#### Executor-attention family

Model: `psionic-models/src/tassadar_executor_attention.rs`, a layered full-prefix
causal attention transformer with fixed 2D heads and hard-max attention. It was
evaluated on a bounded window of 256 prompt tokens and 32 target tokens, with
claim boundary `research_windowed_decode_only`.

| Run | Commit | First / first-8 / first-32 (bps), exact | Notes |
|---|---|---|---|
| Phase 15 seeded, `architecture_comparison_v1` | `b559ba5806` | 0 / 0 / 0, 0 exact; 1,333 tok/s, hull falls back to linear | Lookup baseline in the same window: 10000 / 6250 / 6563 at 32,000 tok/s. Report: `candidate_more_exact=false`, `candidate_closer_to_article_fidelity=true`. |
| `attention_training_v1` + comparison `v2` | `3e48b699dd` | 0 / 6250 / 6563, 0 exact | Loss 5.71 → 4.36. Parity with lookup on suffix, but still fails token 0. |
| `attention_boundary_v1` | `cc18e7de48` | 10000 / 1250 / 313 | Destructive fix through the shared output head. Preserved as a negative result. |
| `attention_boundary_v2` + comparison `v4` | `cc18e7de48` | 10000 / 7500 / 6875, 0 exact | Relative-target output-bias adapter. **First time attention beat lookup** (`candidate_more_exact=true`). |
| `attention_boundary_v3`, `v4` + comparison `v6` | `e8e076dddb` | Flat at 10000 / 7500 / 6875 | Hidden-state projection adapter, then a larger step size. Diagnosis: the model emits `<step> <byte_00>` and skips `<step_index>`. |
| `attention_boundary_v5` + comparison `v7` | `c13cf35a64` | **10000 / 8750 / 7188**, 0 exact | Previous-token-conditioned transition adapter. Divergence moved to **token 6**: the reference wants `<pc>` and the model predicts `<byte_00>`. **Best learned result in the whole program.** |
| `attention_boundary_v6` + comparison `v8` | `50c8fdb86e` | 10000 / 8750 / 7188 | Joint transition + projection fine-tune. Plateau. |
| `attention_boundary_v7`, `v8`, `v9` + comparisons `v9`–`v11` | `fa80adcf81` | Unchanged | Added a trace-schema adapter, then per-position bias, then 64x per-position gain. **All 32/32 checkpoints share one validation signature** (token 6, `<pc>`→`<byte_00>`, 7188 bps, 0/2). Declared "saturated rather than merely under-tuned". |

Notes on the fixtures:

- Only comparison directories v1, v2, v4, v6, v7, v8, v10 and v11 exist under
  `fixtures/tassadar/runs`. The v9 name appears only in the audit text.
- The next step the audit recommended (trainable hidden-state dynamics / a learned
  state-transition path) was not attempted in-tree.

#### Compiled / proof-backed lanes (the only exact results)

**Phase 17**, `7fb1983bc3`, fixture `sudoku_v0_compiled_executor_v0`:

- Built with `cargo run -p psionic-research --example tassadar_compiled_executor_bundle`.
- Covers 8 per-case deployments, each carrying program artifact, compiled weight
  artifact, runtime contract, compile-evidence bundle and descriptor.
- **8/8 exact trace, output and halt against the CPU reference (10000 bps).**
- **32/32 exact typed refusals** across four mismatch classes: wrong program
  digest, wrong Wasm profile, wrong trace ABI, and inconsistent digest binding.
- Posture `eval_only`. Bundle digest `2a9fcad9…`.
- Claim: "bounded compiled/proof-backed … not arbitrary-program closure".

**Phase 18**, `52eced170b`, fixture `hungarian_v0_compiled_executor_v0`:

- New profile `tassadar.wasm.hungarian_v0_matching.v1` with an `i32.lt` opcode.
- A real bounded min-cost perfect matching over **4x4 cost matrices**, with an
  8-case split corpus and a validator-owned benchmark and environment package.
- **8/8 exact, 32/32 exact refusals.**
- `hungarian_lane_status_report.json`: learned lane `not_done`, compiled lane
  `exact`.
- Caveat stated in the audit: the program uses bounded exhaustive assignment
  search, not the article's Hungarian algorithm path.
- Bundle digest `f97fdff6…`.

### 5. Crates, modules and paths (all under `crates/psionic/`, deleted from openagents by `feb4d45007`)

- **runtime:** `psionic-runtime/src/tassadar.rs`, 6,416 lines at extraction.
  Holds the profiles `core_i32.v1/v2`, `sudoku_v0_search.v1`,
  `sudoku_9x9_search.v1` and `hungarian_v0_matching.v1`; the decode modes
  `reference_linear`, `hull_cache` and `sparse_top_k`; and the trace ABI
  `tassadar.trace.v1`.
- **models:**
  - `psionic-models/src/{tassadar.rs, tassadar_sequence.rs, tassadar_executor_transformer.rs, tassadar_executor_attention.rs}`
- **data and environments:**
  - `psionic-data/src/tassadar.rs`
  - `psionic-environments/src/tassadar.rs`
- **eval:**
  - `psionic-eval/src/{tassadar.rs, tassadar_sequence.rs, tassadar_executor_eval.rs, tassadar_executor_benchmark.rs, tassadar_executor_hull_benchmark.rs, tassadar_executor_architecture_comparison.rs, tassadar_compiled_executor.rs, tassadar_hungarian_compiled_executor.rs}`
- **train:**
  - `psionic-train/src/{tassadar.rs, tassadar_sequence.rs, tassadar_executor_training.rs, tassadar_executor_run.rs, tassadar_executor_telemetry.rs, tassadar_executor_postmortem.rs, tassadar_executor_hull_benchmark.rs, tassadar_executor_scale_plan.rs, tassadar_executor_promotion.rs}`
  - 8 examples: `tassadar_reference_training_{run,telemetry,postmortem,hull_benchmark}`, `tassadar_sudoku_9x9_scale_plan`, `tassadar_boundary_training_run`, `tassadar_promotion_training_run`, `tassadar_promotion_v2_training_run`
- **research:**
  - `psionic-research/src/{tassadar_surface_ablation.rs, tassadar_architecture_comparison.rs, tassadar_attention_training.rs, tassadar_compiled_executor_bundle.rs, tassadar_hungarian_compiled_executor_bundle.rs}`
  - 8 matching examples
- **serve:** `psionic-serve/src/tassadar.rs`
- **fixtures:** `fixtures/tassadar/runs/`
  - 26 run directories, 328 files, **about 556 MB**
  - The largest files are `checkpoint_state.json` files of about 85 MB each
    (`promotion_v1`, `promotion_v2`, and the ablation mixer and embeddings
    surfaces).
  - `docs/audits/2026-03-16-psionic-extraction-audit.md` (`bfb2a375c6`) singled
    out "Tassadar checkpoint state files" as the main fixture weight and
    recommended LFS or a release bucket.
- **Psionic docs updated alongside:** `crates/psionic/README.md` ("Tassadar Executor
  Lane" section), `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/TRAIN_SYSTEM.md`,
  `docs/RESEARCH_EXPERIMENT_REFERENCE.md`.
- **Outside the psionic crates:** `scripts/release/check-psionic-tassadar-4x4-promotion-gate.sh`
  (added in `9fc9b98e7e`, removed in `feb4d45007`).

### 6. The Autopilot desktop Tassadar Lab pane

| Commit | Date | Change |
|---|---|---|
| `1cafa430ec` | 03-17 14:28 | "replay-first Tassadar lab pane". Adds `apps/autopilot-desktop/src/panes/tassadar_lab.rs` (895 lines), `tassadar_lab_control.rs` and `docs/tassadar-lab.md`. Bumps the psionic pin from `c61159d7` to `60abf060`. |
| `2ee586a518` | 03-17 14:42 | Live Tassadar lab sessions (article sessions and hybrid workflows). |
| `eefb356e79` | 03-17 15:05 | Controls and persistence, plus `autopilotctl tassadar …` CLI and desktop-control (about 1,700 lines). |
| `77174ad7c1` | 03-17 15:17 | Wider run explorer (replay families). |
| `446ec5d3cb` / merge `dc0f35c28f` | 03-25 | Lab load deferred until the pane opens. PR #4008, "Fix Tassadar startup load causing post-open UI freeze", authored by BigDaddy and merged by bensilone. |
| `081a843f76` | 04-19 | App renamed `apps/autopilot-desktop` → `apps/autopilot-deprecated`. At 04-30 the pane was 1,042 lines and the control file 758 lines. |
| `eb4b0fa4f1` / `f5919c7669` | 06-08 / 06-09 | Moved to `apps/deprecated/`, then deleted. |

- **Architecture.**
  - The pane is a WGPUI shell over `psionic_serve::LocalTassadarLabService`.
  - It uses the `TassadarLabRequest`, `TassadarLabSnapshot` and `TassadarLabReplayCatalogEntry` types.
  - It also uses `TassadarArticleExecutorSessionRequest`, `TassadarArticleHybridWorkflowRequest` and the planner routing policy and budget.
  - Psionic owns replay bundles, proof identity and lineage. OpenAgents owns only playback, selection and persistence, stored at
    `~/.openagents/logs/autopilot/tassadar-lab.json`.
- **Replay families (`TassadarLabReplayFamily`):** ArticleSessions, HybridWorkflows,
  CompiledClosure, Acceptance, LearnedPromotion, Learned9x9Fit, LearnedHorizon,
  ArchitectureComparison. Several of these ("compiled article closure",
  "acceptance report", "learned 9x9 fit", "learned horizon policy") name artifacts
  that exist only in the *external* psionic repo after extraction. They are not in
  openagents history.
- **Views and controls:**
  - Views: Overview, Trace, Program, Evidence.
  - Source modes: Artifact Explorer, Article Session, Hybrid Workflow.
  - Keyboard: Space plays or pauses. Arrow keys step cases and updates. `1`–`4` select the view, `5`–`7` the source, `8`/`9` the family.
  - `autopilotctl tassadar {status, play, pause, reset, refresh, view, source, family, case, update, readable-log, token, fact, speed, window}`.

### 7. Key docs

All docs in `docs/audits/` were moved to `docs/deprecated/audits/` in `86625916df`
(06-08) and deleted in `f5919c7669` (06-09). Each entry gives the commit that added
it.

- **`2026-03-15-can-llms-be-computers-psionic-adaptation-audit.md`** (`1943b0125a`):
  the Percepta reading and the Phase 0–9 plan.
- **`2026-03-16-tassadar-trained-executor-gap-audit.md`** (`9696a427ab`): says the
  substrate is not a trained executor, with a gap table and the #3776–#3787 spine.
- **`…-first-run-postmortem.md`** (`0f4cb16f00`): the first run collapses at
  token 0, then the Phase 10/11 follow-ups.
- **`…-phase-12-boundary-audit.md`** (`c4f732b877`): token 0 cleared, divergence
  at index 1, full-trace training regresses the boundary.
- **`…-phase-13-trainable-surface-audit.md`** (`55bb4e1a76`): 4-surface ablation,
  the mixer wins modestly.
- **`…-phase-14-blocker-audit.md`** (`9fc9b98e7e`): the promotion gate is red at
  6875 first-32 and 0 exact. Later edits append the attention saturation summary.
- **`…-phase-15-executor-attention-audit.md`** (`b559ba5806`): seeded attention
  family at 0 bps, closer to the article in shape but worse.
- **`…-phase-17-compiled-executor-audit.md`** (`7fb1983bc3`): compiled Sudoku-v0,
  8/8 exact and 32/32 refusals.
- **`…-phase-18-hungarian-audit.md`** (`52eced170b`): compiled Hungarian-v0 on 4x4,
  8/8 exact; learned lane `not_done`.
- **`…-promotion-v2-teacher-forced-audit.md`** (`ae059cc2f5`): schedule-only churn
  hits the same ceiling.
- **`…-trained-attention-follow-on-audit.md`** (`3e48b699dd`): trained attention
  at 6563 first-32, but fails token 0.
- **`…-attention-boundary-adapter-audit.md`** (`cc18e7de48`): boundary v1
  (destructive) and v2 (first attention win over lookup).
- **`…-step-index-blocker-audit.md`** (`e8e076dddb`): v3/v4 flat, and the
  `<step_index>` skip is diagnosed.
- **`…-transition-adapter-audit.md`** (`c13cf35a64`): v5 moves divergence to
  token 6 (`<pc>`), giving the best ceiling of 7188.
- **`…-joint-adapter-audit.md`** (`50c8fdb86e`): v6 plateau.
- **`…-structural-adapter-saturation-audit.md`** (`fa80adcf81`): v7–v9, 32/32
  identical checkpoints, adapter family saturated.
- **`docs/tassadar-lab.md`** (`1cafa430ec`, updated through `77174ad7c1`): the
  lab pane manual. Deleted in `f5919c7669`.
- **Peripheral mentions:**
  - `docs/audits/2026-03-16-psionic-extraction-audit.md` (`bfb2a375c6`):
    fixture weight.
  - `docs/audits/2026-03-16-attnres-port-into-psionic-and-wgpui-audit.md`
    (`5ba323491a`): cites `tassadar_executor_transformer.rs` as a reference.
  - `docs/compiled-agent-contributor-beta.md` (`6266dbf5bb`, 03-29): says the
    contributor beta must not "require Tassadar to participate".
  - Transcripts:
    - `docs/transcripts/216.md` (`815263b70e`, 03-26) quotes Chris: "we've more
      or less um, reproduced the Percepta paper… point your coding agent at
      Psionic". It also says Psion will be "an executor model".
    - `docs/transcripts/220.md` (`8ea1161821`, 04-06) mentions "this Percepta
      post".

### 8. Outcome, as of 2026-04-30

- **Exact results in the in-tree era were compiled/proof-backed only.**
  - Sudoku-v0 on 4x4: 8/8 exact trace, 32/32 exact refusals.
  - Hungarian-v0 on 4x4 cost matrices: 8/8 exact trace, 32/32 exact refusals.
  - Both are `eval_only` and bound to matched programs.
  - The 9C gap audit says an earlier version of the compiled path delegated back to
    the imperative runtime. The Phase 17/18 artifacts add per-deployment compiled
    weight bundles and compile evidence.
- **The learned executor never passed its gate.**
  - Best learned result: executor-attention `boundary_v5`, at 10000 / 8750 /
    **7188** bps (first / first-8 / first-32) with **0/2 exact traces**.
  - It diverges at token 6, where the model predicts `<byte_00>` instead of `<pc>`.
  - The bounded adapter family was declared saturated.
  - Phase 14 (#3814) stayed red. Phase 16 (the 9x9 run, #3816) stayed blocked.
    Learned Hungarian stayed `not_done`.
- **Speed result:** the neural hull-cache decode ran about 1.93x faster than linear
  on model-KV (42,172 vs 21,860 tok/s), with explicit caveats that this was not a
  correctness result.
- **All of it left the repo the same night.** Psionic was extracted to its own repo
  at `feb4d45007`, about two hours after the final Tassadar commit. After that,
  openagents only held:
  - the audits
  - the desktop Tassadar Lab, which read replay and live data from pinned external
    `psionic-serve`
  - later marketing and podcast references
- **Public claims ran ahead of the audits.** The Episode 216 transcript says the
  team "more or less reproduced the Percepta paper". The in-repo audits
  consistently say article parity is "unearned".
- **No Tassadar or Percepta commits** landed in openagents between 2026-03-26 and
  2026-04-30 beyond the 04-19 app rename. Retirement came in June 2026:
  `86625916df`, `eb4b0fa4f1`, `f5919c7669`.


---

## Part II — openagents documentation, June 2026 onward

Scope: every Tassadar/Percepta document in `OpenAgentsInc/openagents` from 2026-06-01 on. Most of it is now deleted. It was reconstructed with `git show <deletion>^:<path>`. The core folder `docs/tassadar/` was added between 2026-06-10 and 2026-06-21, bannered "RETIRED FOR NOW" in `97b600f4d7` (2026-07-08 15:55), and physically deleted in `e1fbd1c185` (2026-07-08 16:43). That second commit archived it to backroom as `openagents-prune-20260708-tassadar-psionic` at backroom commit `a56fd270`. A second wave of satellite docs (`docs/launch/`, `docs/artanis/`, `docs/khala/`, `docs/game/`, `docs/research/`, `docs/RETIRED.md` and the retirement audit itself) survived until the repo-wide "Nuke" commit `dabc08102f` (2026-09-18). The implementation lived mostly in the sibling `psionic` repo (psionic #1098 to #1114). This repo held the TypeScript executor (`packages/tassadar-executor`), the replay validator, settlement, projections and the docs.

---

### 1. What Percepta built, and what "Tassadar" meant

#### Percepta's construction, as the docs describe it

The docs use two Percepta Research posts as their source:

- **"Can LLMs Be Computers?"** (Christos Tzamos et al., 2026-03-11).
- **"Constructing an LLM-Computer"** (2026-03-25). This post came with the open-source `Percepta-Core/transformer-vm` repo (Apache-2.0), cloned read-only at `projects/repos/transformer-vm`.

The claim: take a small exact program and *compile* it into the weights of a completely standard transformer, with no training. The model then executes the program exactly, token by token, for millions of steps. The notes describe the model as "boring on purpose": vanilla PyTorch, d_model=36, 18 heads of exactly 2 dimensions each, 7 layers, a gated FFN and no custom kernels. "The only thing that makes it special is the weights."

The mechanism, as encapsulated in `docs/tassadar/2026-06-10-percepta-constructing-llm-computer-notes.md` (`b94cbd91d2`):

- **Computation as an append-only trace.** The prompt holds the program (Wasm bytecode). The generated tokens are the machine state's history: instruction pointer, stack ops, memory commits and control flow. Each instruction takes at most 5 tokens. Nothing is edited, so the trace is replayable and audit-native by construction.
- **ALM (Append-only Lookup Machine).** Five primitives, each realized *exactly* by a transformer component:
  - keyed read/write, via attention
  - cumulative sum, via uniform-key attention times (t+1)
  - product, via a ReGLU FFN (`a·b = a·ReLU(b) − a·ReLU(−b)`)
  - conditional, via ReLU step functions (`1[z≥0] = ReLU(z+1) − ReLU(z)`)
  - linear combination, via residual-stream wiring

  The defining restriction is that writes come before reads. With it, ALM is Turing complete. **CALM** is a language over ALM, and it compiles to a **gate graph** built from only LookUp gates (attention) and ReGLU gates (FFN).
- **2D parabolic-key attention is exact memory.** A key `k` becomes the point `(2k, −k²)` and a query becomes the direction `(q, 1)`. The score `2qk − k² = −(k−q)² + q²` is uniquely maximized at `k = q`. A small position-dependent perturbation gives latest-write-wins semantics. The construction also carries over to real softmax, with exponentially small error.
- **Hull decode (HullKVCache).** In 2D, a hard-max query is a convex-hull "supporting point" query, answerable in O(log t) rather than O(t). The published benchmark on one 9,580-line trace was 31,037 tok/s vs 316 tok/s (about 98x), or 1.3 s vs 258.9 s on CPU. Note that `RESEARCH_PLAN.md` cites "31,037 against 702" for the same head-to-head, so the docs disagree on the baseline figure.
- **Demos.**
  - A 10x10 Hungarian min-cost matching: 439,194 trace tokens at 33,583 tok/s.
  - Arto Inkala's "world's hardest Sudoku": about 3M tokens in under 3 minutes.
- **Compiler backend.** A MILP places gates into four-phase layers (attention, materialize, FFN, materialize) under precedence, type and co-location constraints. It minimizes peak liveness, which sets d_model. Slot reuse is interval coloring, with stale-slot subtraction because the residual stream is additive. The weights are then emitted analytically.
- **Two execution modes.**
  - A universal 35-opcode Wasm interpreter in weights, with a lowering pass for MUL, DIV and the bitwise ops.
  - The **first Futamura projection** ("programs into weights"), which bakes a fixed N-instruction program into 2N shared ReGLU step-function neurons. The d_ffn cost grows as O(N).
- **Percepta's own caveats**, which the docs preserve: it is "orders of magnitude slower than a conventional computer", it covers only a Wasm subset, and memory grows with token count.
- **Percepta's announced directions**:
  - weight-programming toolkits
  - formal verification of transformer logic
  - faster constructions
  - "injecting programmatic logic into the training loop of LLMs", i.e. the hybrid
- **The transformer-vm reference review** (`b82286f965`, in the audit) found a full program-to-weights compiler in the repo. It includes:
  - the `graph/core.py` five-primitive IR
  - `wasm/interpreter.py`
  - `scheduler/milp.py`
  - an analytic `model/weights.py`
  - `hull2d_cht.h` with O(log n) insert and query
  - a C++ engine at about 30K tok/s
  - a C to wasm32 toolchain
  - a triple-reference harness (`wasm-eval` / `wasm-reference` / `wasm-run`)

  The docs also track `projects/repos/llm-as-computer`, an independent 55-opcode reproduction, and its "Free Computer" essay.

#### What "Tassadar" meant

**Tassadar** began (March 2026, in this repo, `0363dff6c` "psionic: add Tassadar sequence dataset contracts") as psionic's **executor-capable bounded Psion profile**: a transformer that executes programs inside its own inference loop. The Protoss naming family is Psionic, Psion, Pylon, Artanis and Khala. The naming rule (psionic `PSION_EXECUTOR_PROGRAM.md`) was strict:

- **Tassadar = compiled, exact lane.** Its claims are proofs.
- **Psion = learned lane.** Its claims are bounded statistics and "must never borrow Tassadar's exactness language."

From June 2026 on, "Tassadar" also named:

1. the ALM compiler campaign in psionic;
2. the public paid network run `run.tassadar.executor.20260615`, called "the Tassadar run";
3. the product-promise family around both (`compute.tassadar_executor_poc.v1`, `artanis.tassadar_evolution_loop.v1`, `models.tassadar_percepta_executor.v1`, `training.decentralized_training_launch.v1`, and the `claims.world_first_*` pair).

The unifying thesis, in the docs' words: "a computation that is exact is a computation that can be verified by replay". In the executor lane the trace *is* the receipt, and a validator's verdict is a digest comparison, "the cheapest verification grade that can exist."

---

### 2. Doc inventory

Status key:
- **del-J** = deleted in `e1fbd1c185` (2026-07-08), archived to backroom `a56fd270`
- **del-N** = deleted in the "Nuke" `dabc08102f` (2026-09-18)
- **del-W1** = deleted in `d613b8ea22` (2026-08-28, "delete TypeScript product roots")

All `docs/tassadar/*` and `docs/training/*` files carried the RETIRED banner from `97b600f4d7` before deletion.

#### Core folder `docs/tassadar/` (all del-J)

| Path | Added | Commit | One-line summary |
|---|---|---|---|
| `2026-06-10-tassadar-percepta-audit.md` (first at `docs/`, `833e5b9c46`) | 06-10 | `abb3f2f932` (relocated) | Full history across both repos; Percepta lineage; transformer-vm review (`b82286f965`); Tassadar x CS336 "compiled vs trained" (`fc36323eb5`). |
| `work-that-proves-itself.md` | 06-10 | `abb3f2f932` | Fable business essay: verification is the economy's tax; the exact lane is "a new bottom rung"; "software that lives in weights"; kill conditions. |
| `README.md` | 06-10 | `278fc8cf1b` | "Tassadar: The LLM as a Computer" lane essay and index; what was built (E1 to E6-numeric) and what was not. |
| `2026-06-10-percepta-constructing-llm-computer-notes.md` | 06-10 | `b94cbd91d2` | Concept-by-concept encapsulation of the construction post, mapped to Tassadar. |
| `2026-06-10-psionic-alm-compiler-design-speculation.md` | 06-10 | `d60d5aa5c5` | Rust-native ALM compiler design and phasing table, executed as psionic #1098 to #1114. |
| `2026-06-11-llm-computer-full-introduction.md` | 06-10 | `8671afc246` | "Assume-nothing" 10-part tutorial on both posts, a transformer-vm tour and the Rust file map. |
| `RESEARCH_PLAN.md` | 06-11 (rev 06-18) | `5e15d1a65e` | Unified Tassadar + Psion directive: thesis, hypotheses H1 to H6, workstreams W1 to W5, kill conditions, standing orders. |
| `2026-06-11-chatgpt-pro-analysis.md` | 06-11 | `f18a58a701` | External brief: "verified trace factory plus student sweep", a 14-day plan; Fable commentary adopting first-divergence metrics and the trace_record schema. |
| `2026-06-11-chatgpt-pro-analysis-2.md` | 06-11 | `640d98ac11` | Second brief: literature map (RASP/Tracr, NTM/DNC, Faith and Fate), 7 critiques, 11 avenues; commentary on what was adopted and what was declined. |
| `2026-06-11-tassadar-plugin-marketplace-audit.md` | 06-11 | `600fc56496` | "The Store We Built Twice": 2024 agent store, then Blueprint, then replay-verifiable modules; shelf tiers E/D/S/N; "the store is built last." |
| `2026-06-11-coding-agent-primitive-wedge.md` | 06-11 | `f825f589cb` | The first good is the coding agent (Autopilot), not exact modules; the "Tier E shelf is real but nearly empty." |
| `2026-06-11-autopilot-agentic-labor-market.md` | 06-11 | `b3bd080557` | The "Orrery moment": idle agents as supply, the issue backlog as demand, labor-market clearing rails. |
| `2026-06-11-tassadar-capability-envelope-pylon-consumer-evidence.md` | 06-11 | `20c0447ecd` | #4750: Pylon declares `capability.tassadar_poc.numeric_model_executor` only after a digest-verified self-test (80-step loop_sum). |
| `2026-06-14-w3-student-program-report.md` | 06-13 | `a1734d7204` | W3 sweep on a 103.6M-token verified corpus: H1 and H2 supported, H3 falsified. |
| `2026-06-15-executor-trace-contributor-completion-design.md` | 06-15 | `7b2e8876dc` | Why contributors could claim work but not finish it (admin-only submit; closeout needs two devices). |
| `2026-06-16-verified-work-payment-economics.md` | 06-16 | `f558da0008` | 50k-sat recognition rewards; recommended 1 sat per verified window; validator was unpaid. |
| `2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md` | 06-17 | `17cd637240` | Corrected audit: "training" means construction, not gradient descent; the run executes one program forever; tracks S/C/V/E/H. |
| `2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md` | 06-19 | `b7ba64e45f` | Kernel-optimization work unit plus throughput-parity protocol; the promise stays red. |
| `2026-06-20-tassadar-percepta-executor-model-spec.md` | 06-20 | `c61088825f` | `models.tassadar_percepta_executor.v1` spec: exact substrate, learned interface and product rails as three separate lanes. |
| `2026-06-20-tassadar-percepta-architecture-receipt.md` | 06-20 | `8bfef42537` | Public architecture-receipt route; clears one blocker only. |
| `2026-06-21-tassadar-cpu-transform-training-receipt-surface.md` | 06-20 | `c9d32a1942` | Receipt route that deliberately reports every "real training" gate as false. |

#### Satellites

| Path | Added | Commit | Status | Summary |
|---|---|---|---|---|
| `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md` | 06-10 | `16475f4c71` | del-N | The executor lane is "poised" for Pylon v0.3; five inclusion items; §5 makes executor-trace Artanis's first autonomous work class. |
| `docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md` | 06-10 | `5f31491afd` | del-N | How Artanis ticks; proposes the evolution-loop promise. |
| `docs/artanis/2026-06-10-artanis-pylon-tassadar-full-status-audit.md` | 06-10 | `78f21dd628` | del-N | Forum, registry and doc sweep after a 69-commit evening; first autonomous spans. |
| `apps/openagents.com/docs/2026-06-10-tassadar-executor-trace-homework-internal.md` | 06-10 | `57b6170494` | del-W1 | Executor-trace homework wiring. |
| `apps/openagents.com/docs/2026-06-11-tassadar-trace-factory-contract-freeze-evidence.md` | 06-11 | `5446980e95` | del-W1 | W2 contract freeze (#4748): corpus stats, replay from a clean checkout. |
| `packages/tassadar-executor/README.md` | 06-10 | `3704ba785a` | removed `fae80bde79` 08-28 | TS ALM numeric executor; claim boundary "no softmax, no learning, no serving." |
| `docs/launch/JUNE15_LAUNCH_PLAN.md` | ~06-14 | (launch folder) | del-N | "JUNE 15 LAUNCH PLAN: The Tassadar Run"; the launch happened, then pivoted to install stability and a built-in Gemini agent. |
| `docs/launch/2026-06-17-tassadar-live-page-accuracy-audit.md` | 06-17 | `4fe8683387` | del-N | `/tassadar` route honesty (#5186 to #5189): no fallback visuals, typed settlement rows. |
| `docs/launch/2026-06-17-tassadar-training-run-visual-language.md` | 06-17 | `b75d1169da` | del-N | Visual primitives for the run. |
| `docs/launch/2026-06-18-autopilot-tassadar-chat-blueprint-audit.md` | 06-18 | `2bfaad8ae6` | del-N | Default post-onboarding chat = a Blueprint program run with Tassadar modules as steps. |
| `docs/launch/2026-06-18-blueprint-tassadar-chat-delegation.md` | 06-18 | `1c84faadeb` | del-N | Delegation brief for EPIC #5449 (#5450 to #5456). |
| `docs/launch/2026-06-18-world-firsts-verification.md`, `2026-06-20-llm-computer-training-run-definition.md`, `2026-06-20-world-first-llm-computer-evidence-pack.md`, `vertex-fleet/claims.world_first_public_llm_computer_training_run.v1.md` | 06-18 to 06-20 | (launch folder) | del-N | Prior-art review (Percepta, Tracr, Gensyn, Bittensor, and others); sense (A) vs sense (B) of "training run"; qualified wording. |
| `docs/launch/2026-06-20-verse-hands-off-pylon-tassadar-audit.md` | 06-20 | `1b9cf28ea9` | del-N | Desktop starts on Chat/Verse; Tassadar state in the world. |
| `docs/game/2026-06-17-spacetimedb-tassadar-integration-next-steps.md` | 06-17 | `8150933493` | del `3ee0785f51` 06-22 | SpacetimeDB world projection of the run. |
| `docs/game/2026-06-17-tassadar-wasd-mouselook-controller-plan.md` | 06-17 | `232341bd4c` | del-N (POSTPONED) | Walkable `/tassadar` (three-effect#1). |
| `docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md` | 06-17 | `a8967720f8` | del-N | Use this repo as the next "study" corpus. |
| `docs/sakana/tassadar-fugu-exploration.md`, `tassadar-run-integration.md` | 06-22 | `c0b6e17475`, `45319aeb54` | del-J | Sakana Fugu/Conductor coordinator over the paid pool; five verification classes. |
| `docs/research/tmax/tassadar-tmax-exploration.md` | 06-22 | `e37d989f21` | del-N | TMAX terminal-RL recipe as verified, paid, watchable work. |
| `docs/khala/2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md` | 06-24 | `db0494b0e7` | del-N | "Khala = Blueprint program execution x Tassadar verification floor x marketplace consumption." |
| `docs/training/2026-06-27-tassadar-training-and-percepta-status-audit.md` | 06-27 | `9036dcdbcf` | del-J | Artanis hand-off: live run numbers; Percepta "about 60% built"; P0/P1 fan-out list. |
| `docs/fable/2026-07-04-ts-6-start-khala-tassadar-route-slice.md` | 07-04 | `d084548b95` | del-J | TanStack Start port of `/khala`, `/tassadar`, `/gym`. |
| `docs/promises/2026-06-29-world-first-claims-7027-audit.md` | 06-29 | (promises) | del-N | Both world-first claims stay red; allowed and refused wording. |
| `docs/fable/2026-07-08-repo-docs-direction-cleanup-audit.md` | 07-08 | `5f87fd63a2` | del-N | The retirement mandate and taxonomy (see §4). |
| `docs/RETIRED.md` | 07-08 | `97b600f4d7` | del-N | Retirement ledger; updated to "archived" in `e1fbd1c185`. |

Other in-window mentions, not Tassadar-titled: `docs/promises/registry.md` and `source-set.md`, `docs/launch/JUNE16` to `JUNE19_ROADMAP.md`, the blog post `tassadar-run-is-live` in `apps/openagents.com/apps/web/src/page/blog.ts` (dated June 16), `docs/autopilot-coder/2026-06-14-the-second-engine...essay.md` ("Verification is the moat, and Tassadar is the deepest part of it"), and `docs/fable/2026-07-07-what-openagents-is-essay-and-talking-points.md` (Tassadar as "our research bet: verification by re-execution"). Pre-window material was already gone before June 2026: `docs/tassadar-lab.md` (added `1cafa430ec` 03-17, removed in the Bun rebuild `f5919c7669` 06-09) and the March 2026 `docs/audits/*tassadar*` postmortems.

#### Still live at HEAD

`git grep -il 'tassadar\|percepta'` returns:

- **`docs/glossary.md`** marks Tassadar "Historical": "the training/construction program and worker–validator/run-board narrative in episodes 236–240; the current Verse reimplements part of the spatial presentation idea without carrying over a live training or payment network."
- **`docs/psionic-and-pylon.md`** describes psionic's still-existing `psionic-tassadar-student` crate, `docs/ROADMAP_TASSADAR.md`, `TRAIN_TASSADAR`, and "Tassadar executor lane: the exact-computation 'LLM as computer' executor substrate." It also records that Tassadar "ran on [Pylons] as an indefinite Bitcoin-paid distributed training run (episode 237)."
- **`docs/history/2026-09-25-transcript-roadmap.md`** groups "Pylon, Psionic, Tassadar" as the broad compute platform, deferred to Phase 5 and outside this repo.
- **`docs/game/README.md`** and **`docs/verse/README.md`** say the Verse's look derives from the walkable "Tassadar Run Board." The game log cites `0d344194cf` (06-16, `/run` live 3D page) and `522077d84e` (06-20, "Render Tassadar training in the Verse").
- **`docs/transcripts/README.md`** plus transcripts 203, 216, 220, 236, 237, 238, 240, 241, 243, 274 and 275.

**How the transcripts reference it:**

- **203** is the first on-air aside ("I think that's Tassadar").
- **216**: "Psion is also going to be an executor model. We've more or less reproduced the Percepta paper," plus pausing markets to focus on paid decentralized training and "this Percepta executor stuff."
- **220**: the "Percepta post" as something "the other labs wouldn't be able to do."
- **236 ("Tassadar")**: the "Percepta Executor Class model... CPU computation transformed," with support added to Pylon v0.3 alongside Bitcoin payments.
- **237** (21 hits): Autopilot 1.0 plus Tassadar as "an indefinite distributed training run that pays its contributors Bitcoin for verified work... building a new 'executor' class of model on @PerceptaAI's 'LLMs as Computers'."
- **238 ("The Training Run Begins")**: a whiteboard reading "THE TASSADAR RUN IS LIVE", two "world firsts", and "WTF is LLM-computer?": "Defined by AI lab Percepta... Programs are compiled into transformer weights... No gradient descent! (Optional add-on later)."
- **240**: the Verse "Tassadar Run Board": 11 pylons, 12 verified items, 1,020 sats paid.
- **241** and **243**: passing Protoss-naming and self-improvement references.
- **274** and **275** (September): Tassadar listed among spec'd services to be "folded into the OpenAgents product suite", and "the Tassadar idea... distributed training" as a future CLI capability.

---

### 3. How the thesis evolved

#### Stage 0 (pre-window, March 2026): the trained executor fails

The first Tassadar ladder tried to *train* exact execution. It ran next-token training on execution traces, boundary curricula, transition adapters and neural hull decode, and ended in documented blockers: the step-index boundary blocker, the joint-adapter plateau and adapter saturation. The 06-10 audit reads the commit titles as "try to *train* exact execution, hit the plateau honestly, and pivot weight production toward trace-bound/compiled routes," consistent with "Faith and Fate" (Dziri et al. 2023). What survived was a bounded `TRAIN_TASSADAR` lane (`tassadar-article-transformer-trace-bound-trained-v0`). Psionic's `PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md` (2026-03-30) recorded a `green_bounded` closeout with a HullKVCache at ≥1.69x over reference-linear and a ≤2.55x gap to CPU, plus the explicit limitation `arbitrary_c_or_wasm_not_claimed`. The audit calls that "the honest shape of the 'reproduced Percepta' claim." The desktop "Tassadar Lab" replay pane from March was removed in the Bun rebuild.

#### Stage 1 (06-10 to 06-11): the construction paradigm, owned in Rust

On 2026-06-10 the docs folder was created (audit, essay, concept notes, compiler design). The same day, a 17-issue campaign (psionic #1098 to #1114) built an integer-exact **executor-compiler** in Rust:

- **E1**: ALM gate-graph IR plus an exact evaluator.
- **E2**: a four-phase list scheduler with interval coloring and explicit stale-slot subtraction.
- **E2b / E2c**: parabolic-key geometric reads that *refuse* near-misses, and a Li Chao hull fast path. On a 2,000-step workload the linear baseline exceeded 1M comparisons, while hull visits stayed an order of magnitude lower.
- **E5**: a Futamura specializer (v2, shared 2N indicators).
- **E3**: a branch-capable interpreter for psionic's 12-opcode i32 window, cross-validated against the production CPU runner.
- **E6-numeric**: a digest-pinned f64 coefficient artifact running inside a checked 2^53 window.
- **#1106**: an exact trace-replay verifier.
- **#1107**: a 5-leg differential harness over 400 generated graphs.

The headline composite fact: a real backward-branch program, baked by Futamura projection into static gates and serialized as a JSON coefficient file, reproduces the CPU runner exactly across five agreeing legs. The narrative centerpiece is "The part worth reading twice": on its first run the harness caught **two real scheduler bugs** (cumsum reordering across layers, and same-step writes resolved in schedule order instead of program order). The docs present this as proof that "a system that cannot catch its own errors cannot price trust."

The same day `compute.tassadar_executor_poc.v1` went **green**. A real Pylon ran the pinned workload, the production worker replayed it as a separate validator device (Verified, and Rejected on tamper), and one operator-funded closeout settled over Lightning (transition receipt `promise_transition_99b561e9…`).

The docs stated the gaps plainly: no trained model, hard-max only (no softmax), no dense `W_Q/W_K/W_V` checkpoints, a 12-opcode window against Percepta's 35, no MILP, no serving. "A CPU is faster" became a standing order.

`work-that-proves-itself.md` framed the business stakes:

- verification is the machine economy's tax;
- exact replay is a rung below "deterministic tests", which inverts the usual anti-correlation between value and verifiability for this work class;
- weak devices become first-class validators;
- the exact lane is a noise-free control group for the fuzzy lanes;
- "software that lives in weights" becomes a new artifact class for the existing marketplace rails.

The Tassadar x CS336 section drew three ways "training" touches the lane. Compiled-exact needs no training. Learned-exact plateaus. The hybrid, "CPU compute added to the weights of models", requires owning architecture and training loop, which is what the CS336 port provides.

#### Stage 2 (06-11 to 06-14): research program and the student verdict

`RESEARCH_PLAN.md` (`5e15d1a65e`) turned the material into a directive. The compound question: "Can the exactness we can compile become something we can train, sell, and embed?"

Hypotheses:

| Hypothesis | Claim |
|---|---|
| H1 | Pure learned exactness fails |
| H2 | Frozen exact cores plus learned control succeed |
| H3 | 2D geometry is trainable only with help |
| H4 | Programs-in-weights becomes a module system |
| H5 | Verified-trace distillation is the best data |
| H6 | Born-verified work clears at better margins |

Workstreams:

| Workstream | Scope |
|---|---|
| W1 (gates everything) | Substrate: window ladder toward 35 opcodes, dense materialization, MILP, softmax bounds |
| W2 | Trace factory: contract freeze, compact binary traces, tiered validation, "never train from unverified artifacts" |
| W3 | Student program |
| W4 | Hybrid products: capability envelopes, module library, evolution loop |
| W5 (added later) | Quarantined public gradient windows |

Both ChatGPT Pro analyses fed this plan. The first recommended "massively distribute trace generation and verification; centralize synchronous GPU training." The second added a literature map and critiques. The Fable commentary adopted first-divergence metrics and the trace_record schema, and declined a parallel module ABI and cluster-topology work.

The **W3 report** (06-14) ran the sweep on `corpus.tassadar_trace.v0_2.w3_100m`: 103,573,600 verified tokens, 12,548 records, 6 families.

| Baseline | Setup | Result | Verdict |
|---|---|---|---|
| A | Next-token distillation | 0.0 pass@1, every record diverged at step 0 | H1 supported |
| C | Analytic lookup initialization | Lookup accuracy 1.0, still 0.0 rollout | H3 falsified for this setup |
| D | Frozen analytic executor plus learned interface | 1.0 pass@1, 1.0 replay acceptance, 1.0 digest match on 748 records | H2 supported |

The docs labeled this "Psion student evidence, not Tassadar proof evidence."

#### Stage 3 (06-11 onward): compiled modules and the marketplace

The marketplace audit ("The Store We Built Twice") traced three generations:

1. the 2024 agent store and paid WASM plugins with a 60/20/20 split (episodes 048 to 102);
2. Blueprint, the DSPy-descended typed programs;
3. replay-verifiable compiled modules, "goods that carry their own evidence."

It proposed shelf tiers E (exact), D, S and N, with "replay before purchase clears" and revenue splits decomposed from traces, under the rule that "the store is built last." The wedge essay then conceded that the first good is the coding agent (Autopilot), not modules, because the Tier E shelf was "real but nearly empty." The labor-market essay added idle agents (the Orrery moment) as supply.

#### Stage 4 (06-15 to 06-27): the Tassadar Run

`JUNE15_LAUNCH_PLAN.md` launched the **public run `run.tassadar.executor.20260615`** as "the Percepta Executor Class model direction from Episode 236." On launch day the plan itself pivoted, by owner directive, to install stability and a keyless built-in Gemini agent.

**Contributor completion design (06-15).** The funnel reached 3 devices and 5 leases but 0 verified and 0 paid. The causes:

- contributors were told to run an operator-only command;
- every training write except lease claim was admin-only;
- closeout inherently needs two distinct devices (`validatorDeviceRef != pylonDeviceRef`).

**Payment economics (06-16).**

- A one-time 50,000-sat recognition reward each to the first worker (Trigger) and the first validator (Orrery).
- A recommended symbolic 1 sat per verified window, since the fixture is a trivial CPU loop.
- The validator earned nothing per window at that point. The shipped code later settled at 5+5 sats (worker plus validator).

**First real payment (06-16 to 06-18).** The first independent pairing went Verified, and `training.decentralized_training_launch.v1` went green for a bounded scope: one 1,000-sat real-Bitcoin canary over the Spark rail. The RESEARCH_PLAN 06-18 revision counts two paid contributors and 1,005 real sats. Settlement auto-streamed on a Verified pair (#5309, #5310, #5311), and a self-serve window producer followed (#5396). The plan also recorded a 75-sat hygiene-lane settlement, verified by review rather than replay.

**World firsts.** Two claims were vetted to qualified wording. The second was "first **public, open-contributor LLM-computer training run** — the paradigm **defined by Percepta**." `2026-06-20-llm-computer-training-run-definition.md` separated sense (A), gradient training, which the run does not do, from sense (B), executor construction and exact-trace verification, which it does. It explicitly refused "we invented the LLM-computer" and "we trained a model." Both claims stayed **red** through the 06-29 audit (#7027).

**The corrected gap audit (06-17/18).** This is the pivotal self-correction. It withdrew its first version's gradient-descent framing: "training / building the Tassadar model = constructing, verifying, composing, and paying for real compiled capability weight-modules... NOT minimizing a loss over weights." Its verdict: the live workload is a genuine pipeline output (`loop_sum_v1`, a backward-branch loop summing to 15 over 100 steps), but "the run constructs no new capability." It runs one program forever, as sparse scalar-lane coefficients, with no variety, no composition, no marketplace and no pricing of construction. Artanis could dispatch only `dispatch_executor_trace` or `no_action`, at most 4 times a day, for that same fixture. The construction pipeline was "~6/7 phases landed" in psionic. Real settlement was off by default behind `OPENAGENTS_REAL_SETTLEMENT_GATE`, with ceilings of 100k sats per payout and 1M sats per day.

**The 06-27 hand-off audit** (`9036dcdbcf`) wrote the state down for Artanis:

- run `active`
- 12 accepted traces
- 5 distinct contributor devices (2 required)
- 1,020 settled sats across 5 receipts

It judged Percepta "roughly 60% built": about 4K lines of TypeScript executor (including an 851-line linked-dense-module runtime) plus psionic E1 to E5, with E4 MILP, E6 served artifacts, softmax, corpus diversity and construction pricing still missing. It kept the separate Psion `./TRAIN` pretraining lane distinct (last canary: 12 optimizer steps, 3,992 tokens). Its P0 list was a second compiled program in the loop, pricing construction, and a live linked module, to be fanned out through Artanis's new `read_repo_file` and plan-only `dispatch_codex_task` tools. Model-direction receipts from 06-20/21 (`models.tassadar_percepta_executor.v1`) deliberately reported every real-training gate as false.

#### Stage 5 (06-18 to 07-04): Blueprint and Khala fusion, and the Verse

The Autopilot chat audit and delegation (EPIC #5449) redefined the post-onboarding default: the chat is a Blueprint program run, with Tassadar verified-computation modules as steps inside it. The 06-24 Khala fusion essay recast Khala (the OpenAI-compatible `openagents/khala` endpoint) as the marketplace's demand side: "Khala = Blueprint program execution × the Tassadar verification floor × capability-marketplace consumption." Khala would shop for Tier E modules instead of hallucinating numbers, and pay module authors from the trace.

Side explorations followed:
- Sakana Fugu/Conductor as a coordinator over the paid pool, with five verification classes (`exact_trace_replay` at sample rate 1.0);
- the TMAX RL recipe as verified work;
- repo "machine studying";
- the walkable `/tassadar` run board, which became the Verse's visual origin.

The last Tassadar-titled work was the TS-6 TanStack Start port of the `/tassadar` route (07-04).

#### Stage 6 (07-08): retirement

See §4. After 07-08 Tassadar appears only as history: in the glossary, the psionic-and-pylon reference, the 09-25 transcript roadmap, and brief mentions in episodes 274 and 275 of ideas to be "folded into" the product suite.

---

### 4. The retirement decision

**Authority.** `docs/fable/2026-07-08-repo-docs-direction-cleanup-audit.md` (`5f87fd63a2`, co-authored by Claude Fable 5) is the "audit + execution prescription (owner mandate)." It followed `MASTER_ROADMAP` rev 6 (`5ea343c583`, same day, "Effect Native FULL CONVERSION, ASAP").

**What the owner mandated (§1):**

- "Focus everything on Khala Code and business-facing efforts": the mobile MVP, Sarah/outbound sales, credits and payments, the sales landing, and the Effect Native conversion.
- "Deprioritize/deprecate the Tassadar/Psionic program — the LLM-computer research, executor/training/gym/inference lanes, and the compute-market framing built around them. **Retired for now, revived only by explicit owner decision after the company is cashflow-positive.**"
- Postpone everything that is neither Khala Code nor business-facing.
- Stop point-in-time audits from "masquerading as current direction."

**Mechanism.** A four-label taxonomy: RETIRED FOR NOW, SUPERSEDED, POSTPONED, HISTORICAL. §4 named the retired surface:

- `docs/tassadar/` (21 files)
- `training/` (20)
- `gym/` (8)
- `inference/` (36)
- `gepa`, `sakana` (12), `benchmarks`, `stress-testing`, `confidental-compute`
- `agi`, `asi`
- `tokens`, `traces`, `unit`, `systems`, `verification`, `proof`, `apm`
- four `docs/fable/` members, including the TS-6 route slice

`docs/game/` was only **postponed**; the Verse direction was retained. The audit's own guardrails said "**Never delete or move a file**", banners only. Tassadar/Psionic code (about 563 TS files, mostly `apps/pylon`) was declared out of scope for a separate owner-gated lane.

**What actually happened:**

1. **`97b600f4d7` (15:55).** The banner went onto each file ("RETIRED FOR NOW… earliest reconsideration: after cashflow-positive… do not route new work, issues, or copy from this document"). The commit created `docs/RETIRED.md` with one row per directory, revival condition "Owner decision; earliest after cashflow-positive."
2. **`e1fbd1c185` (16:43, "Prune retired Tassadar Psionic surfaces").** Less than an hour later this commit went past the audit's no-delete rule. It physically deleted all of `docs/tassadar/`, `docs/training/`, `docs/sakana/` and the other retired directories. It also removed large parts of the code:
   - the Pylon Tassadar capability, trace client and CPU-transform modules;
   - the Psionic connector, install, vLLM proxy, serving and training-cockpit modules;
   - the proof-replay routes, replay-clip jobs and the `replay-r1` spike.

   Small `archived-tassadar-*.ts` stubs replaced them. `RETIRED.md` status changed to "archived," pointing at backroom `openagents-prune-20260708-tassadar-psionic` @ `a56fd270`. `MASTER_ROADMAP` "Retired programs (rev 6.1; execution status rev 6.2)" was updated to match.
3. **`dabc08102f` "Nuke" (2026-09-18).** The remaining satellites went, including `docs/RETIRED.md` and the cleanup audit itself. The TS executor package and the homework docs had already been deleted on 2026-08-28 (`fae80bde79`, `d613b8ea22`).

**Why.** The retirement docs give priority and focus as the stated reason, not a technical refutation: the company is to concentrate on Khala Code and revenue until cashflow-positive. The program's own documents supply the substantive context, and a reader can connect the two:

- The executor lane had proven the verify-by-replay-and-settle mechanism, but at canary scale: 1,020 sats and 12 traces.
- It "constructs no new capability" and re-verified one trivial program.
- No external demand had been shown (`proof.demand_provenance.v1` planned). The first kill condition, "'Just use a CPU' wins everywhere", was untested either way.
- Both world-first claims stayed red.
- The wedge essay had already conceded that the coding agent, not exact modules, was the first sellable good.

**What survives.** The ideas persist only as history in this repo: the glossary's "Historical" entry and the Verse's run-board aesthetic. Psionic's own Tassadar crates and roadmaps (`psionic-tassadar-student`, `ROADMAP_TASSADAR.md`, `TRAIN_TASSADAR`) still live in the psionic repo. The full document set is recoverable from backroom `a56fd270` or from `git show e1fbd1c185^:docs/tassadar/<file>` and `git show dabc08102f^:<path>` for the later satellites.


---

## Part III — openagents code, June 2026 onward

Scope: code only (docs are covered separately). Source: `git log -i --grep='tassadar\|percepta' --since=2026-06-01` (303 commits), plus path-based logs over `*tassadar*` / `*percepta*`, and file reads at specific commits. The repo is `OpenAgentsInc/openagents`. Commit hashes are short, and dates are author dates.

**State today (checkout at `3943b45f62`, 2026-09-28; `origin/main` gives the same result):** there is no live Tassadar code. `git grep -il tassadar -- ':!docs'` returns 0 files. The only 15 hits are docs (transcripts, `docs/glossary.md`, `docs/psionic-and-pylon.md`, `docs/verse/README.md`, `docs/game/README.md`, `docs/history/…`). `packages/tassadar-executor` no longer exists, and neither do `packages/` or `apps/`: the repo is now a Rust-only tree (`crates/`, `bins/`, …). The Tassadar code was removed in two stages:

1. **2026-07-08 prune** (`e1fbd1c185`). It removed the Pylon executor/client, the replay packages and the executor package. The package was restored the same day (`9bf6be5191`), and the Worker-side run, settlement and corpus code stayed live.
2. **2026-08-28 TypeScript deletion.** `d613b8ea22` "delete TypeScript product roots (Wave 1, #266)" removed `apps/openagents.com`, `apps/pylon`, `apps/forum` and `apps/forge-git-service`. `fae80bde79` "delete the TypeScript package graph (Wave 3, #268)" removed `packages/`, including `packages/tassadar-executor`. Both commits say that git history is the archive.

---

### Dated milestone timeline

| Date | Milestone | Evidence |
|---|---|---|
| 2026-06-10 | `@openagents/tassadar-executor` created with byte-for-byte Rust↔TS trace-digest parity (#4689, epic #4687) | `3704ba785a` |
| 2026-06-10 | `compute.tassadar_executor_poc.v1` promise added as yellow (registry 2026-06-10.7) | `0c83a21a8c` |
| 2026-06-10 | **First live Tassadar executor-trace closeout on a real Pylon.** `pylon.7a41439039d360162e84` ran `assignment.tassadar-poc.live-m1`. Trace digest `f2995c4e…932a5b` matched the psionic Rust fixture: 80 steps, halted, closeout receipt `assignment.closeout.7e7ebbf204c7b7688d07af55` | `7bf1f01c47` |
| 2026-06-10 | The production Worker acted as the separate-device validator. `exact_trace_replay` produced a Verified challenge, and a tampered digest produced a Rejected one. One operator-funded paid closeout settled over real Lightning (receipts: payer balance 2173→1173, receiver 0→980) | `7f73ca254b`, `933a41c1d3` |
| 2026-06-10 | `compute.tassadar_executor_poc.v1` flipped **green** (registry 2026-06-10.12, transition receipt `promise_transition_99b561e9…`) | `43d64fb8ae` |
| 2026-06-10 | `artanis.tassadar_evolution_loop.v1` added as yellow (registry 2026-06-10.13) | `5f31491afd` |
| 2026-06-10 | Pylon 0.3.0-rc2 cut; it declares the Tassadar executor capability by default (#4711) | `63ff6037d6` |
| 2026-06-11 | Trace-factory contract freeze. Local pilot corpus: 314 records, 3,476,576 trace tokens (#4748) | `19ec46e4f6` |
| 2026-06-11 | W3 100M-token corpus snapshot and student harness (#4749); TassadarCapabilityEnvelope (#4750) | `f54c9b6a95`, `20c0447ecd` |
| 2026-06-14 | Run authority: `run.tassadar.executor.20260615` seeded as active (#5006), then admission and work window (#5007), exact_trace_replay tie-in (#5008), settlement (#5009) and corpus (#5010) | `3f20f3d3e2` … `cdb44315e9` |
| 2026-06-15 | Launch day (Episode 237). Self-serve contributor backend landed: submit/verdict routes (#5052), pairing (#5053), Pylon verbs (#5054) | `602e83e0b3`, `c8bcb888bc`, `20a354b326` |
| 2026-06-16 | `TASSADAR_TRACE_PAIRING` enabled in prod. **First independent cross-owner worker↔validator pairing** was Verified (challenge `59ba1f30`) with a settled receipt of 5 sats. `training.monday_decentralized_training_launch.v1` flipped **green** (registry 2026-06-16.6). The 5 sats were later identified as a *simulation* receipt (`realBitcoinMoved:false`) | `4344028a03`, `2341dd06df` |
| 2026-06-16 | `/run` live 3D page; blog post `/blog/tassadar-run-is-live` | `0d344194cf`, `5477c50f30` |
| 2026-06-17 | **First real Bitcoin settlement:** 1,000 sats native over Spark (`spark_treasury`, `realBitcoinMoved:true`) to an independent contributor, closing #5232 (registry 2026-06-17.5). The first attempt failed closed (`payout_intent_not_found`) and was fixed in `ef6afeef5d`. The promise was renamed to `training.decentralized_training_launch.v1` in `bef33c98ce` | `89b718fa1c`, `15097fc2ee` |
| 2026-06-17 | Auto-stream real settlement: 5 sats to the worker plus 5 sats to the validator per Verified pair, under a daily cap (#5309/#5310). Live settled feed over the sync engine (#5311) | `ffbcb76f62`, `e5481f54e1` |
| 2026-06-18 | Second real settlement (5 sats, self-serve), bringing the real total to **1,005 sats** (the 5-sat simulation row is excluded; the earlier aggregate read 1,010) | `9d64310375`, `e252c671d9`, `1b28d595b6` |
| 2026-06-18 | Pylon **v1.0.0** stable on npm. It fixes `npx` install, which had failed because the old 0.2.5 launcher predated the Tassadar earning path | `e738443e13` |
| 2026-06-19 | Registry 2026-06-19.7: **5 distinct independent contributors paid real Bitcoin, 1,020 sats** (a 1,000-sat canary plus 4×5 sats), qualifiedContributorCount 5, acceptedTraceCount 11 | `15f614f46f` (copy) |
| 2026-06-20 | Percepta executor spec (`c61088825f`), architecture receipts (#5570), CPU-transform receipt status. `artanis.tassadar_evolution_loop.v1` flipped green (registry 2026-06-20.47) | `8bfef42537`, `c9d32a1942`, `549570a273` |
| 2026-06-28 | CPU-transform fixture receipts published (#6952) | `b4ebdeb98a` |
| 2026-07-05 | Training-runs UI deprecated "for now" (#8415) | `cb284086fb` |
| 2026-07-08 | **Prune** of Tassadar/Psionic surfaces (276 files, about −337k lines); executor package restored the same day | `e1fbd1c185`, `9bf6be5191` |
| 2026-08-28 | All TypeScript apps and packages deleted (the remaining Tassadar code went with them) | `d613b8ea22`, `fae80bde79` |

---

### 1. Executor package: `packages/tassadar-executor`

**Created in `3704ba785a` (2026-06-10, #4689).** This is a TypeScript executor for psionic's `TassadarAlmNumericModel` v1 format. The model format has:
- sparse wiring rows and gated neurons;
- hard-max parabolic attention with latest-write tie-breaking and exact-match refusal;
- accumulator sums and end-of-step writes;
- a 2^53 exactness window with typed refusals.

The trace digest is `sha256('tassadar_alm_trace|' + graphDigest + i64-LE rows)`, computed exactly as the Rust code does. The load-bearing test reproduces the Rust-exported digest byte-for-byte on `fixtures/tassadar-poc-loop-sum-v1.json`, a backward-branch loop program exported from psionic commit `3b7a3941`. Exact-replay verdicts (full replay, and window spot-checks that name the tampered step) mirror psionic #1106.

Files:
- `src/numeric-executor.ts`
- `src/replay.ts`
- `src/replay-cli.ts`
- `src/index.ts`

**Later additions:**
- **Publishability.** `16475f4c71` (2026-06-10) made the package publishable, so the packed Pylon could resolve it; this fixed the release-gate install smoke. `63ff6037d6` exported shared job-kind and capability constants for the Pylon.
- **Capability envelope (W4.1, #4750, `20c0447ecd`, 2026-06-11).** Added `src/capability-envelope.ts`, which is a dependency-free decoder for psionic's provider-facing `TassadarCapabilityEnvelope`. Also added `src/self-test.ts` (a digest-pinned self-test) and `src/lane.ts`.
- **Compiled program corpus.** `93b4ecbbfa` (2026-06-18) added `src/compiled-program-corpus.ts` and `fixtures/tassadar-compiled-program-corpus-v1.json`, dispatched by the Artanis tick. `36d7b7ee6e` rerouted it to the W1.1 corpus, and `0dc7a7fe40` updated it to E4.
- **Dense weight module.** `d3c5993629` added `src/dense-weight-module-runtime.ts` and `fixtures/tassadar-dense-weight-module-v1.json` (about 19.8k lines).
- **Linked dense module.** `dc27cf007a` added `src/linked-dense-module-runtime.ts` and `fixtures/tassadar-linked-dense-module-v1.json` (about 230k lines).
- **Composition verification.** `e2b900dbb6` extended the linked runtime by about 392 lines so that composition must verify.
- **Fixture de-duplication.** `6c8bd0afa8` (2026-06-18, #5334/#5336) removed the duplicate fixtures: the `.ts` twins now import the canonical `.json`, cutting about 265k lines. The commit message lists 23 of 23 package tests passing and credits @orrery-agent. This was the first funded "hygiene lane" pass.
- **Kernel optimization (2026-06-20, "vertex-fleet").** This is the `compute.agentic_kernel_optimization_at_scale.v1` work: `src/kernel-optimization-parity.ts`, a throughput-parity verifier with op-provenance, delivered-kernel, trace-binding and cross-graph output-equivalence gates (`955da19042`, `f64a7bc758`, `aa402f589c`, `e72a36eae2`, `8053d9bccf`, `9acc283ad5`, `5f94df1994`). Dispatch receipts in `src/kernel-optimization-dispatch.ts` followed on 2026-06-28 (`1ada0e4a18`, #6897).

**Lifecycle:** the package was deleted by the prune `e1fbd1c185` (2026-07-08). `9bf6be5191` restored it verbatim the same day because the prune left 13 runtime imports in `workers/api`, so `bun build src/cloudrun/server.ts` failed. Ambient type shims (`archived-tassadar-modules.d.ts`) had hidden the breakage from `tsc`. That commit was primarily a Khala 502 incident fix (OpenRouter 402 overflow). The package then survived toolchain migrations (`8d2ec44c34`, `6bbe34bee4`, `799f51d17a`, 2026-07-14) until it was finally deleted in `fae80bde79` (2026-08-28).

### 2. Pylon (`apps/pylon`)

- **Job kind and dispatch.** `73cf420156` (2026-06-10, #4690) added the `tassadar_executor_trace` value to `PylonApiAssignmentJobKind` in `workers/api/src/pylon-api.ts`. It also added `workers/api/scripts/tassadar-poc-dispatch.ts`, an operator-gated assignment that goes through `POST /api/operator/pylons/assignments` with the digest-pinned workload embedded (`unpaid_smoke` by default).
- **Execution.** `a409d5d0c4` (#4691) made `apps/pylon/src/assignment.ts` execute these assignments through the shared executor. The computed digest goes into the artifact, proof and result refs. A digest mismatch or a typed refusal becomes a rejected closeout, never a silent accept. Tests: `tests/tassadar-assignment.test.ts`.
- **Live fixes.** `7bf1f01c47` made three fixes from the live run:
  - the gate also matches the `tassadar_executor_trace_homework` kind;
  - `seed_writes` travels as `initialChannelWrites`, because the projection scanner forbids "seed" keys;
  - each assignment is safety-isolated in the poll path.
- **Default capability.** `2babfb939b` made the executor capability a Pylon default in `bootstrap.ts` and `state.ts`. `63ff6037d6` (0.3.0-rc2, #4711) auto-declares `capability.tassadar_poc.numeric_model_executor` on go-online and adds an executor leg to `scripts/packaged-live-network-smoke.ts`.
- **Receipted capability (#4750, `20c0447ecd`).** Added `src/tassadar-capability.ts`. On go-online the Pylon runs a real self-test, and only a verified replay declares the capability, together with `receipt.tassadar_executor.self_test.v1.<digest16>`. If the self-test fails, the Pylon reports `blocker.pylon.tassadar_executor_self_test_failed`. The Worker side of this check is in section 3.
- **Contributor verbs (#5054, `20a354b326`, 2026-06-15).** Added `src/tassadar-trace-client.ts` with two verbs:
  - `pylon training submit-trace` runs the workload locally and posts to `/api/training/leases/{leaseRef}/trace-submission`;
  - `pylon training validate` replays and posts to `/replay-verdict`.

  Both take an agent token. The background assignment worker stayed off by default (`PYLON_ASSIGNMENT_WORKER`).
- **Auto validation (#5121, `fb66fb69f4`, 2026-06-16).** Added `pylon training validate --auto [--watch --interval-ms --max-iterations --run-ref]`, which discovers work through `GET /api/training/contributions/next-unpaired`.
- **Codex and Claude work classes.** These touch Tassadar only indirectly. `a8ede030ff` (#4719) and `5256203d34` (#4720) added the Claude-agent executor gate and task dispatch. `87db2dbf5f` (CX3, #4790) added the `codex_agent_task` work class. Both run on the same assignment rails that the Tassadar PoC proved. `982c33f521` and `1cc0e9ba03` (2026-06-27, #6354) fixed capability-ref refresh for these classes; they matched the grep only through context.
- **CPU-transform training (#6952, `b4ebdeb98a`, 2026-06-28).** Added `src/tassadar-cpu-transform-training.ts` and a CLI catalog entry.
- **Pruned on 2026-07-08.** The prune deleted `tassadar-capability.ts`, `tassadar-trace-client.ts`, `tassadar-cpu-transform-training.ts`, the psionic-qwen, serving and training-cockpit files, and their tests. `apps/pylon/src/index.ts` lost about 300 lines. They were replaced by the stub `src/archived-tassadar-executor.ts`: the capability ref becomes `…numeric_model_executor.archived`, and `executeTassadarNumericModel` throws "Tassadar executor assignments are archived in backroom." All of `apps/pylon` was deleted on 2026-08-28.

### 3. Worker API (`apps/openagents.com/workers/api`)

#### PoC-era code (June 10–11)

- **Replay validator.** `src/tassadar-replay-validator.ts` provides `POST /api/operator/tassadar/replay` (admin; `7f73ca254b`, #4692). The production Worker re-executes the workload itself and gives a verdict on the claimed digest. The Tailnet validator devices were unreachable, so the Worker served as an honest, bounded v1 validator. It later gained dense-module and composition checks (`d3c5993629`, `e2b900dbb6`).
- **Homework wiring.** `src/tassadar-executor-trace-homework.ts` (`57b6170494`) contains the homework payload builders. It later held `tassadarExecutorTraceVerificationChallengeRequest`, which is used for run-tied challenges.
- **Artanis closeout verifier.** `src/artanis-administrator-tick.ts` reads the full 64-hex digest from artifact refs (`abd2b7ce29`). It also dispatches the compiled corpus and dense module (`93b4ecbbfa`, `36d7b7ee6e`, `d3c5993629`), and its eligibility pre-filter requires the receipted capability.
- **Capability admission.** `src/tassadar-capability-admission.ts` (#4750) enforces the receipted executor claim on the Worker side:
  - unreceipted registration claims are refused with `refusal.public.pylon_capability.tassadar_executor_unreceipted`;
  - the dispatch gate blocks with `blocker.public.pylon_dispatch.tassadar_capability_unreceipted`.
- **Trace factory (#4748, `19ec46e4f6`).** The directory `src/tassadar-trace-factory/` holds:
  - `trace-record.ts`: trace_record v0.1, `trace_token.v0.1` limb encoding and a TTRC binary container;
  - `validation-policy.ts`: the Tier 0–3 validator ladder, quarantine before admission, and the rule "never train from unverified";
  - `training-split-policy.ts`: held-out families, and training on short programs while evaluating on long ones (2×, 4×, 8×);
  - `projection-rebuild.ts`: counters rebuild on validation transitions, never on registration (case law #4744–#4746);
  - `tick-closure.ts`, `record-factory.ts` and `workload-families.ts` (six families).

  Scripts: `scripts/tassadar-trace-factory-generate.ts` and `-replay.ts`. The manifest is `corpus/tassadar-trace-corpus.v0_1.manifest.json` (314 records, 3,476,576 tokens, 100% passing Tier 0 and Tier 1).
- **W3 (#4749, `f54c9b6a95`).** Added `corpus/tassadar-trace-corpus.v0_2.w3_100m.manifest.json`, `scripts/tassadar-trace-factory-generate-w3.ts`, `scripts/tassadar-w3-student-prep.ts`, and `tassadar-trace-factory/w3-student-report.ts` (eval-report schema and projection).
- **Held-out guard (#6419, 2026-06-28).** `4f7ac7261b` added an isolated GG partition and was reverted in `8d120580bd`. `3c64aa5084` then added held-out shard validation in `generalization-guard.ts`, which checks checksums, family drift, and record and token counts.

#### Run authority and the launch steps (June 14–18)

**Run state (#5006, `3f20f3d3e2`).**
- `POST /api/training/runs/{ref}/activate|seal|reconcile` (admin) moves a run through planned→active→sealed→reconciled.
- A public-safe launch manifest was added in a new `training_runs.manifest_json` column. Its fields: objective, `workloadFamily=executor-trace`, `verifierPolicy=exact_trace_replay`, admissionRule, paymentMode, settlementState, spendCapSats, statusUrl, abortRule, artifact digest refs and blockers.
- Code: `src/training-run-window-authority.ts` and `src/training-run-window-routes.ts`.

**Admission (#5007, `1c3ee09ca8`).** `POST /api/training/runs/{ref}/admit`. `src/tassadar-run-admission.ts` combines three gates: the receipted executor capability, an independent-contributor check, and the #4852 host-RAM device admission gate.

**Closeout to verification (#5008, `65e52c1bf1`).** `POST /api/training/runs/{ref}/executor-trace-closeout` creates a run- and window-tied `exact_trace_replay` challenge. If the validator is not a distinct device, the route returns 400.

**Settlement (#5009, `987cf677a4`).** `src/tassadar-run-settlement.ts` and `POST /api/training/runs/{runRef}/settlement-receipt` (admin). The payout chain is intent → attempt → reconciliation → `settlement_recorded` receipt, under the run's spend cap plus a per-payout cap. `publicTrainingRunSummary` sums `providerConfirmedSettledPayoutSats`, which had been hardcoded to 0. `d7cfa7a454` fixed a missing `nexus_payout_target_approvals` foreign-key row.

**Corpus on the run (#5010, `cdb44315e9`).** The summary gains a `corpus` block: the count of Verified `exact_trace_replay` closed-tick traces.

**Self-serve contribution (#5052, `602e83e0b3`).**
- `POST /api/training/leases/{leaseRef}/trace-submission` (requireAgent; the lease must belong to the caller's Pylon).
- `POST /api/training/leases/{leaseRef}/replay-verdict` (requireAgent; the validator must be a distinct device).
- Code: `src/tassadar-trace-contribution-authority.ts` and `-routes.ts`.

**Pairing (#5053, `c8bcb888bc`).** `src/tassadar-trace-pairing.ts` runs on the Artanis tick and is gated by `TASSADAR_TRACE_PAIRING=1`, which was enabled in prod on 2026-06-16 (`4344028a03`). `resolveValidatorCandidates()` is intentionally empty so the server never fabricates a validator digest.

**Auto-discovery (#5121).** `GET /api/training/contributions/next-unpaired?validatorDeviceRef=…` (`fb66fb69f4`), plus `cf969c0ee8` and `3ba873ff64` (digest-ref normalization).

**Open-window producer (#5396, `db51f3d370`).** A cron keeps a pool of 2 claimable `auto_starter` windows on the run.

**Public reads.**
- `GET /api/public/tassadar-run-summary` (#5114, `f5ba42bbb3`), in `src/public-tassadar-run-summary-routes.ts`.
- `GET /api/public/training/runs/{ref}` (`878d9775a8`).
- Settlement rows (`b76fa68558`).
- `GET /api/training/runs/{ref}/settlements` and the public alias `/api/public/training/runs/{ref}/settlements` (`2be196c286`, `cd37bbaede`, #5403). `summary.settlement.reconciledState` is computed on read.
- `2f7c857f49` added migration `0209` to clear stale manifest blockers.

#### Real Bitcoin settlement (#5232) and auto-stream

**Owner gate (`3af6232e3c`).** `src/tassadar-run-settlement-gate.ts` reads the typed, fail-closed `OPENAGENTS_REAL_SETTLEMENT_GATE`, which carries a per-payout cap, a single recipient and a single run. `spark_treasury` stamps `moneyMovement:'real_bitcoin'`.

**Ledger migrations (`cd22e92c4e`).** `0203_nexus_payout_adapter_kind_spark_treasury.sql` and `0204_…fk_repair.sql`. The `INSERT OR IGNORE` against the old CHECK constraint had been silently dropping the Spark rows.

**Wiring (`89b718fa1c`).** The real branch calls `makeTreasuryPaymentAuthority` and `makeSparkTreasuryPayoutAdapter`. It is receipt-first and idempotent, with at most one dispatch per window and recipient.

**Intent persistence fix (`ef6afeef5d`).** The first real attempt failed closed. `createPayoutIntent` in `nexus-treasury-payout-ledger.ts` now verifies that the intent was persisted.

**Auto-stream (`ffbcb76f62`, #5309/#5310).** `src/tassadar-auto-settlement.ts` exports `autoSettleVerifiedPair`, which pays 5 sats to the worker and 5 sats to the validator on each Verified pair. It adds `maxDailyPayoutSats` and `runScopedStreaming`, is fail-soft, and runs from `routeReplayVerdict`. `fd0ef77d68` (#5359) fixed the fail-soft typing.

**Payout target resolution.** `daf29aa7e3` added an owner fallback for the payout target (lease device ref vs pylonRef). `24cb8f30a2` resolved the validator-leg target by device ref (#5394).

**Real-only totals.** `e252c671d9` and `1b28d595b6` made the total count only `real_bitcoin` movement (1,005, not 1,010) through a single resolver.

**Construction settlement simulation.** `174d1e2ff9` added a simulation in `tassadar-auto-settlement.ts`.

**Reuse by other lanes.** Other lanes use the same gate and rail:
- the hygiene lane: `POST /api/hygiene-lane/settlement-receipt` (`c773f49b0e`, `3f636c133a`, #5372);
- firm-up escrow (`e9581e34ae`, #5459);
- referrals (`2c83afd4f9`, RL-1 #5458);
- the Khala M3 verified-work Spark payout (`eba893fd13`, #6023; `5844245c7c`, #6067), which reuses `resolveTassadarSettlementAdapter` and `decideTassadarDailyBudget`.

**Settled feed (#5311, `e5481f54e1`).** `src/tassadar-settled-feed-sync.ts` publishes public-safe settled events to the sync scope `public-settled-feed:tassadar`, guarded against raw payment strings (spark1, bc1, lnbc, preimages, 64-hex). The homepage "Live Settled Feed" panel consumes it. Later changes:
- a public `GET /api/public/settled-feed`;
- a khala-sync dual-write and then a full cutover (`8a1f5c858e`, `9f46cff7e7`, KS-6.4, #8414);
- `1165ab90f9` (#8515, 2026-07-07) moved training-domain writes and the `tassadar-run-summary` and first-real-settlement reads off the dead D1 onto the Postgres adapter; `4ea6575252` handled the remaining D1 reads.

#### Research, market and model surfaces (June 18–29)

- **Adversarial verification market.** `src/tassadar-adversarial-verification-market.ts` (`cb31d3c521`), wired into `artanis-work-directions.ts`.
- **Compiled module marketplace.** `src/tassadar-compiled-module-marketplace.ts` serves `GET /api/public/tassadar/compiled-module-marketplace` (`dc27cf007a`). `src/tassadar-module-library.ts` adds demand ranking and price signals (`7c9a819a32`).
- **Gradient-window promotion gate.** `src/tassadar-gradient-window-regime.ts` (`657cd92703`), with a promise edit. On 2026-06-20 the vertex-fleet `training.public_gradient_windows.v1` work added these files, all `src/tassadar-gradient-window-*`:
  - `-intake` (`a5a8e38864`)
  - `-promotion-receipt` (`ba4ab4a902`)
  - `-quarantine-record` (`3529fd2104`)
  - `-promotion-lineage` (`388ced8c25`)
  - `-promotion-receipt-verify` (`915382bf47`)
  - `-promotion-receipt-feed` (`a61116a1eb`)
  - `-quarantine-record-verify` (`ed3c1a1b5d`)
  - `-quarantine-record-feed` (`bdc4e3839e`)
- **Percepta executor spec (`c61088825f`, 2026-06-20).** A spec doc was added, and only the model-spec blocker on `models.tassadar_percepta_executor.v1` was cleared. The architecture-receipt and CPU-transform-receipt blockers stayed.
- **Architecture receipts (#5570, `8bfef42537`).** `src/tassadar-percepta-architecture-receipts(-routes).ts` serves `GET /api/public/models/tassadar-percepta-executor/architecture-receipts`. `07e56bf038` (#7219) aligned it with the CPU fixture receipt.
- **CPU-transform training receipts.** `src/tassadar-percepta-cpu-transform-training-receipts(-routes).ts` serves `GET /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`. It started as gate status only (`c9d32a1942`, PR #5886 merged as `8dd265f612`, #5528). With #6952 (`b4ebdeb98a`, #6891) it gained one bounded Pylon CPU-transform fixture receipt. The fixture receipt, assignment, accepted work, verdict and fixture artifact are all true, while real settlement and green gate satisfaction stay false.
- **Distillation dataset receipt.** `src/artanis-distillation-dataset-receipt.ts` serves `GET /api/public/artanis/tassadar-distillation-dataset` (`2277068fe6`). It cleared the last blocker on `artanis.tassadar_evolution_loop.v1`.
- **Proof replays.** `src/public-proof-replay-routes.ts` serves `GET /api/public/proof-replays` and `GET /api/public/tassadar-replays/first-real-settlement` (`e48ae2d293`, `cf2e1cec9c`). `replay-clip-job(s|-routes).ts` came from the replay-clip epic #5346. `44a58cac63` serves the replay routes directly from the Worker.

#### DB migrations (all under `workers/api/migrations/`)

- `0158_forum_tassadar_psionic_seed.sql`: seeds the Forum Research category and the `tassadar` and `psionic` forums (`b86bc227c7`, 2026-06-10).
- `0185_training_run_manifest_tassadar_run.sql`: adds `manifest_json` and seeds `run.tassadar.executor.20260615` as active, with `paymentMode: operator_approved_small_sats`, `spendCapSats: 100000` and `settlementState: pending`.
- `0186_tassadar_executor_window.sql`: seeds the window `training.window.tassadar.executor.20260615.w1`.
- `0188_training_trace_contributions.sql`: creates the table `training_trace_contributions` (pending, paired and so on) with lease and run-state indexes.
- `0203` and `0204`: add `spark_treasury` as a payout adapter kind and repair the foreign keys.
- `0209_tassadar_manifest_clear_stale_blockers.sql`.

### 4. Product-promise registry (`workers/api/src/product-promises.ts`)

- **`compute.tassadar_executor_poc.v1`.** Added as yellow in `0c83a21a8c`, blockers cleared in `933a41c1d3`, flipped green in `43d64fb8ae` (all 2026-06-10). It was still green at the final TypeScript commit. Its `unsafeCopy` forbids claims of a served product, CPU-performance parity, or general LLM-computer capability.
- **`artanis.tassadar_evolution_loop.v1`.** Added as yellow in `5f31491afd` (2026-06-10) with four blockers. The distillation receipt landed in `2277068fe6`, and it flipped green in `549570a273` (2026-06-20.47, where the owner delegated per-flip sign-off).
- **`training.monday_decentralized_training_launch.v1`.** Flipped red→green in `2341dd06df` (2026-06-16.6) and renamed to `training.decentralized_training_launch.v1` in `bef33c98ce` (2026-06-17.6). The real-settlement evidence was added in `15097fc2ee` (2026-06-17.5). The copy then went from "two contributors / 1,005 sats" (`9d64310375`) to "five / 1,020 sats" (2026-06-19.7; `15f614f46f`). It was still green at the final commit.
- **`pylon.install_without_wallet_knowledge.v1`.** Flipped green in `6587039d61` (2026-06-16.7).
- **`models.tassadar_percepta_executor.v1`.** Stayed **planned** at the end. A misspelled twin, `models.tasadar_percepta_executor.v1`, is marked withdrawn.
- **`training.public_gradient_windows.v1`.** Planned.
- **`training.public_distributed_training_run.v1`.** Stayed red on network-scale criteria.
- **Research and marketplace promises.** `e2115120c8` (2026-06-20.49) postponed seven of them to planned.
- **Prune.** Registry 2026-07-08.1 (in `e1fbd1c185`) replaced the Tassadar evidence refs with `backroom:openagents-prune-20260708-tassadar-psionic@a56fd270` and flipped no promise state.

### 5. Web UI (`apps/openagents.com/apps/web`, then `apps/start`)

- **Living-run adapter.** `src/scene/tassadarRunSnapshot.ts` (`2da2be3c8c`, #5113, epic #5112) maps the public summary onto three-effect's `TrainingRunVisualizationSnapshot` and shows honest zeros for an idle run.
- **`/run` page (#5118, `0d344194cf`, 2026-06-16).** `page/run.ts` plus the self-fetching `oa-tassadar-run` element in `scene/tassadarRunElement.ts`. `abf3743b3b` (#5115/#5116) added proof entities.
- **`/tassadar` live route.** `4ef34fd89f` and `753ad90f85` (2026-06-17), with a smoke test in `scripts/tassadar-live-page-smoke.mjs` (`8e72bd79bb`). About 20 fixes followed on 2026-06-17: walk mode, WASD, pointer-lock mouselook, the HUD, promise gates (`8ca070e95d`), Pylon metric reconciliation (`728119f235`), evidence-bound motion (`8d76b0b0cb`, `5456ecff79`), proof-node lanes and drawer links.
- **SpacetimeDB.**
  - `apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs` and `tassadar-summary-transform.mjs`, plus `src/lib.rs` tables (`bd2ecb8332`).
  - Generated web bindings in `scene/spacetimeWorldBindings/*`: training_run, run_entity, proof_ref, settlement_ref, bridge_health (`7ecfc007cd`).
  - `scene/tassadarSpacetimeWorld.ts`, which renders Pylon world stations and agents (#5262 `b12da06d2b`, #5263 `6e182b0b08`), avatar movement (#5264 `6d91e28e36`) and chat bubbles (#5265 `e1a33cf28e`).
  - The world later moved to Cloudflare (`184c4c4be2`, `3ee0785f51`, `d1722ccab7`, 2026-06-22).
- **Replay theater.**
  - `/tassadar/replay/{replaySlug}` (`TassadarReplayRoute`), handled by `scene/tassadarProofReplayElement.ts` (`cf2e1cec9c`), with a social-share cut (`f2a7a660d5`).
  - `packages/proof-replay` and `packages/replay-clips`.
  - A headless render spike, R-1 `a90b1cf28e` (#5347), under `apps/web/spike/replay-r1`. It produced a real 1280×720 PNG, but from the 2.5D DOM projection, not WebGL.
- **Settled feed on the homepage.** Built in `e5481f54e1`; `e4a603644c` added the "Settled (24h)" row.
- **Old scene retired.** `c6b00594cd` (2026-06-21) deleted `tassadarRunElement.ts` (−1,491 lines) and its test, simplifying `/run`.
- **`/tassadar` info page (#6121, `c7859c5d8a`, 2026-06-23).** A persistent-scene camera pose, `page/loggedOut/page/tassadar.ts` (Protoss-themed), a "Join the Tassadar training run" landing CTA, and a "Copy Agent Instructions" button, tightened in `62079891dd` to the order status → preflight → claim.
- **TanStack Start port.** TS-6 `d084548b95` (#8348, 2026-07-04) added `apps/start/src/routes/tassadar.tsx`. `cb284086fb` (#8415) deprecated the training-runs routes. EN-4 `acac7e0ff3` (#8573, 2026-07-08) converted `/tassadar` to the Effect Native DOM renderer. `044d07cca2` (#9325, 2026-08-05) converted it back to plain React (`-tassadar-page.tsx`).
- **Replay oracle test.** `5ff25918d2` (#8634, 2026-07-09) is a 6-line edit to `apps/web/src/main.test.ts` that keeps the retired replay oracle's test expectation in place.
- **Final removal.** The whole web app was deleted in `d613b8ea22`.

### 6. Desktop (`apps/autopilot-desktop`)

- `49d761d976` (2026-06-18): shows Tassadar chat receipts (`ui/model.ts`, `ui/view.ts`).
- `2846ab5223`: a desktop replay-scene fix, in `src/shared/proof-replays.ts`.
- `522077d84e` (2026-06-20): renders Tassadar training in the Verse (`src/shared/verse-training-visualization.ts`).
- `76fee98d61`: routes Verse chat to Tassadar (`src/bun/verse-turn.ts`).
- `eb90f7538a` (2026-06-21): the Verse Tassadar bulletin board (`shared/verse-bulletin-board.ts`).
- `7885c6023f`: the status and Pylon-balance HUD (`shared/verse-run-hud.ts`, `bun/training-runs.ts`), plus related HUD visibility and refresh fixes (`96b20eb3e8`, `1e7d9af0a5`).
- Prune: `e1fbd1c185` removed desktop `proof-replays.ts` logic (−386 lines) and its test.

### 7. Blueprint (inside the Worker)

- **Registry.** `5af4784f67` (2026-06-18) added `src/blueprint/repositories/tassadar-module-registry.ts` and `GET /api/blueprint/tassadar-modules` in `blueprint-routes.ts`. It also touched the contract export and Probe runtime `blueprint/contracts.ts`.
- **Module steps.** `698abde52d` added `src/blueprint/services/tassadar-module-step.ts`, typed tool-scope bindings for dense and linked module steps. It is an evidence-only bridge that runs the fixture-bound runtimes and returns public-safe exact-replay receipts. It also added schemas in `blueprint/schemas/program.ts` and the Probe `tool-menu.ts`.
- **Prune.** The prune reduced the registry, module step, `replay-module.ts` and `replay-signatures.ts` fixtures to archived stubs. `473300f4b5` (2026-07-08) repaired a dangling provenance ref in the archived replay fixture.

### 8. Forum

- `b86bc227c7` (2026-06-10) with migration 0158: the Research category plus the Tassadar and Psionic forums, as idempotent `INSERT OR IGNORE` seeds.

### 9. The 2026-07-08 prune and what remained

**`e1fbd1c185` "Prune retired Tassadar Psionic surfaces"** changed 276 files, adding 789 lines and deleting 337,581. The material was archived to backroom `openagents-prune-20260708-tassadar-psionic` at `a56fd270`, and `docs/RETIRED.md` records it as retired until the company is cash-flow positive.

Removed:
- `packages/tassadar-executor`, `packages/proof-replay` and `packages/replay-clips`;
- the Pylon Tassadar, psionic-qwen, serving, training-cockpit and Khala M6/M7 files and tests;
- the Worker scripts `tassadar-poc-dispatch.ts` and `tassadar-trace-factory-generate` / `-replay.ts`;
- `replay-clip-job*`;
- the web `spike/replay-r1`;
- the tests for the replay element, module library and Blueprint module;
- many `docs/*` directories.

Stubbed: `archived-tassadar-modules.d.ts`, `apps/pylon/src/archived-tassadar-executor.ts`, `tassadar-module-library.ts` and the Blueprint module files.

**`9bf6be5191`** restored `packages/tassadar-executor` (36 files, about 272k lines, mostly fixtures) because the Worker monolith still imported it.

**What stayed live after July 8 and until August 28:**
- the Worker run authority and the settlement, auto-stream and gate code;
- the trace-contribution and pairing routes;
- the trace factory and corpus manifests;
- the Percepta receipt routes and the gradient-window modules;
- the settled-feed sync;
- the public run summary;
- migrations 0158, 0185, 0186 and 0209;
- the Start `/tassadar` page.

At `d613b8ea22~1`, 298 non-docs files still mentioned Tassadar. However, the Pylon client verbs were gone, so new contributors could no longer run the executor.

**2026-08-28:** `d613b8ea22` and `fae80bde79` deleted everything that remained. `packages/tassadar-executor` does not exist today.


---

## Part IV — psionic code and fixtures

Scope: code, examples, scripts, and fixtures in `/Users/christopherdavid/work/psionic`. Docs under `docs/` are covered elsewhere. Everything here is read-only survey work at HEAD `02e0bc85` (2026-09-13). Every count comes from `git ls-files` or `git grep` at HEAD.

### 1. Volume at a glance

- About 2,955 tracked files mention Tassadar or Percepta. By top-level area: `crates/` 1,336, `fixtures/` 1,330, `docs/` 202, `scripts/` 82, plus `TRAIN_TASSADAR`, `README.md`, `AGENTS.md`, `Cargo.lock` and `.gitignore`.
- Rust in files with "tassadar" in the name comes to about 394k lines across 18 crates. There are no `tests/` directories; tests are inline `#[cfg(test)]` modules.
- `fixtures/tassadar/` holds 1,293 tracked files and takes 737 MB on disk (`runs/` 662 MB, `reports/` 63 MB).
- There are 80 `scripts/check-tassadar-*.sh` gate scripts plus `scripts/scaffold-tassadar-capability-free-starter-plugin.py`.
- One Tassadar-specific crate exists: `crates/psionic-tassadar-student`, added in June 2026.

Tassadar files per crate (src modules / examples / approximate LOC):

| Crate | src | examples | LOC | Role |
|---|---:|---:|---:|---|
| psionic-eval | 188 | 178 | 121.7k | Benchmarks, gates, closeout audits, report builders |
| psionic-runtime | 87 | 65 | 71.4k | Executor, trace ABI, Wasm profiles, runners, proof bundles, plugin runtime |
| psionic-research | 132 | 136 | 41.1k | Research-family sweeps, architecture/frontier studies |
| psionic-train | 47 | 40 | 28.3k | Executor-transformer training, promotion, default train lane, CPU-transform receipt |
| psionic-compiler | 40 | 5 | 21.7k | Wasm module lowering, symbolic compile, ALM backend (June) |
| psionic-models | 47 | 1 | 21.7k | Executor/article Transformer families, sequence tokenization |
| psionic-serve | 25 | 21 | 18.3k | Served executor product, OpenAI-HTTP plugin tool loop, publication verdicts |
| psionic-router | 27 | 22 | 12.1k | Planner/route policy, evidence routing, delegation benchmark |
| psionic-data | 18 | 1 | 11.2k | Sequence dataset contracts, corpora, plugin trace corpus |
| psionic-provider | 112 | 4 | 10.6k | Capability/publication envelopes, accepted-outcome bindings |
| psionic-sandbox | 14 | 11 | 10.0k | Plugin capability/charter/world-mount boundaries, import policy |
| psionic-ir | 19 | 1 | 9.3k | Normalized Wasm module IR, symbolic IR, ALM gate-graph IR |
| psionic-tassadar-student | 11 | 0 (2 bins) | 5.1k | W3 four-baseline student sweep, H1 hybrid gate |
| psionic-environments | 4 | 0 | 3.8k | Benchmark environment bundles |
| psionic-catalog | 3 | 3 | 3.7k | Catalog entries for executor artifacts |
| psionic-transformer | 11 | 0 | 2.6k | "Post-article" contract types anchoring the canonical route |
| psionic-cluster | 2 | 2 | 0.7k | Cluster attachment for executor evidence |
| psionic-apple-fm | 2 | 1 | 0.7k | Apple FM plugin session pilot |

Crates with no Tassadar files: the backends, `psionic-core`, `psionic-nn`, the MLX crates and others.

Most of the 491 `examples/tassadar_*` binaries are fixture generators: `cargo run -p <crate> --example tassadar_<x>` writes the matching JSON under `fixtures/tassadar/{runs,reports}`. Each check script re-runs a generator and diffs its output against the committed fixture.

### 2. Is Tassadar still present at HEAD? Retired? Feature-gated?

The code is still present and live. Nothing is feature-gated:

- The workspace is `members = ["crates/psionic-*"]`.
- No `Cargo.toml` defines a Tassadar feature.
- Every `pub mod tassadar_*` in the `lib.rs` files is unconditional (87 in runtime, 132 in research, 112 in provider, and so on). All of it compiles with the default build.
- `README.md` still documents the "Tassadar Training Shortcut" (`./TRAIN_TASSADAR`) and the "Tassadar Executor Lane".
- `AGENTS.md` still points at the private paper corpus: `~/code/alpha/tassadar/tassadar-research/papers/` and `.../can-llms-be-computers/papers/`.

Activity dropped off after March 2026. The last Tassadar-touching commits are:

- 2026-06-18: a burst on the ALM compiler and the student crate.
- 2026-06-23: `4f36914f`, the CPU-transform receipt.

From 2026-06-23 to HEAD (2026-09-13) no commit mentions Tassadar. Recent psionic work is other lanes, for example qwen38 speed gates.

### 3. History and relationship to openagents

psionic was extracted from openagents with rewritten history.

- psionic's root commit is `4c3cff99` (2026-03-09), "Rename mox crate subtree to psionic". This is a subtree-split style extraction: commit subjects and dates are kept, but hashes differ.
- Example of the rewrite: psionic `51de0b83` "psionic: land Tassadar phase 1 executor fixture" (2026-03-15) is `145cc2ac8d` in openagents.
- Of openagents' 44 Tassadar-mentioning commits up to 2026-03-16, 40 appear in psionic with the same date and subject. The other 4 are openagents-only: two docs audits, an issue-spine link, and `desktop: add replay-first Tassadar lab pane` (2026-03-17).
- In those first-day psionic commits, paths were `psionic-runtime/src/tassadar.rs` at the repo root, not under `crates/`. psionic `ddfd8945` "repo: align layout with extraction audit" (2026-03-16) moved them under `crates/`. As a result, path-based `git log -- crates/...` undercounts early history.
- openagents `feb4d45007` "repo: consume psionic via pinned git deps" (2026-03-16) removed in-tree `crates/psionic`. openagents has no `crates/psionic` today.
- openagents `833e5b9c46` (2026-06-10) is a cross-repo "Audit Tassadar and Percepta history" doc.

### 4. Timeline

Commits whose message mentions tassadar/percepta: 255 in total.

| Date | Commits | Notes |
|---|---:|---|
| 03-15 | 3 | Phases 1–3 |
| 03-16 | 43 | |
| 03-17 | 58 | |
| 03-18 | 58 | |
| 03-19 | 57 | |
| 03-20 | 4 | |
| 03-21 | 5 | |
| 04-02 | 3 | |
| 06-10 | 13 | |
| 06-12 | 1 | |
| 06-13 | 1 | |
| 06-18 | 8 | |
| 06-23 | 1 | |

A path-based count gives 336 commits touching Tassadar paths, including 32 on 03-22 and a few on 03-24/28/30 whose messages do not say "tassadar". The Psion executor lane (§12) landed about 45 commits on 2026-03-30 that the message grep misses entirely.

Key milestones:

- **03-15 · Phases 1–3.** `51de0b83` phase 1 executor fixture (runtime `tassadar.rs` 2,037 lines, models `tassadar.rs`). `0732c711` phase 2 artifacts. `8a7b9de8` phase 3 packages.
- **03-16 · Core executor stack, first training, 4x4 promotion.**
  - Executor surface: `3682ba67` proof bundles; `ce3a91f6` phase 5 fast path (HullCache); `0a813997` runtime diagnostics; `56f49b79` served executor surface; `62738c9f` article-class coverage.
  - Research, decode and routing: `2351630b` research family; `fe7dfc98` sparse-top-k decode; `df7c4522` planner routing.
  - Training: `372c5765` small-executor training; `d4679750` compiled-weight path; `e9befd04`/`5b502757` Sudoku-v0 profile and corpus; `d6a68794` sequence dataset contracts; `4abecfae` executor transformer family; `a271503f` next-token training/eval.
  - Runs: `b24f8a31` phase 7 reference run; `f9cf45eb` first-run postmortem; `22e6d06f` neural hull decode; `c4c5460e` 9x9 scale plan; `3c009833`/`9debba5a` phases 12–13 curriculum and ablations; `02b17e09` phase 14 promotion tooling; `bf7be9c2` compiled executor bundle; `38cb0184` "close the 4x4 promotion gate".
- **03-17 · Module-scale Wasm.** `7ef3a37a` compile pipeline matrix; `aa8257de` Wasm conformance harness; `92f8bd5d` module trace ABI v2; `14c10584` module-scale workload suite; `f4f81810` module-state architecture report.
- **03-18 · Specialization and audits.** `9b4b7a59` module-aware specialization; `218d8e44` Wasm route capability matrix; `0f363adf` recurrent fast-path baseline; `af5e4a0d` workload capability frontiers; `f2d99104` precision/attention robustness audits.
- **03-19 · Universality and closeout.** `4ed6e006` hybrid process controller; `87e5996b` effective-unbounded claim audit; `a4dda4e7` full core-Wasm public gate; `fa8a068e` relaxed-SIMD ladder; `7833316e` shared-state concurrency verdicts; `27a15732` general internal-compute red-team; `5b95f5d4` pre-closeout universality audit; `92e97913` Turing-completeness closeout audit; `ae1a5992` "Make Tassadar docs Rust-first"; `56d83ac2` article-equivalence blocker matrix.
- **03-20 – 03-21 · Post-article plugin/platform tranche.** Mostly `docs:` commits that also carry code/fixtures: `49a1d193` plugin system audit, `aa1ed35b` platform threat model, `e6b0d45a` anti-drift stability, `c0d18736` control-plane proof, `fcd3cd0a` machine closure bundle.
- **03-22 – 03-30 · Work the message grep misses.** Article Transformer weights, the trace-bound trained model, and Psion executor lane packets. The safetensors carry mtime 2026-03-26; the Psion packets were committed 2026-03-30.
- **04-02 · Default train lane.** `39608477` freeze default train lane contract; `5f55a3b8` operator launcher (`TRAIN_TASSADAR`); `4da8d69f` rehearsal bundle.
- **06-10 · ALM executor-compiler series E1–E6** (§6). `bd74e5e6` E1 gate-graph IR; `d6652ee3` E2 feasible-first backend; `30cd1797`/`bb36df6f` E3 stack-ISA and branch-capable interpreters; `2285a92f` E5 first-Futamura specializer; `3c19bf35` E2b geometric attention; `9d929459` E2c Li Chao hull; `7874a431` E6-numeric; `2da78db3` bounded differential harness (found two scheduler bugs); `e9531d8a` trace-replay verification class.
- **06-12 – 06-13.** `d12a7b38` fixes 37 residual psionic-train failures; `7497713e` publishes the W3 student sweep artifacts.
- **06-18.** `217078e7` numeric program corpus; `ec2bfe9c` E4 ALM scheduler (MILP schedule, v2); `df4bb227` dense weight module; `d992add8` W1.1 window and softmax bounds; `bd1bea24`/`4cc09d32`/`c36d056d` linked dense module fixture, evidence and tests; `8e74fc2d` H1 frozen-core hybrid gate.
- **06-23.** `4f36914f` dereferenceable CPU-transform training receipt for `models.tassadar_percepta_executor.v1` (#1140, openagents EPIC DE-5 #5528).

### 5. psionic-runtime: executor, trace ABI, Wasm profiles, runners, proofs

**`crates/psionic-runtime/src/tassadar.rs`** (13,233 lines) is the core. It defines:

- `TASSADAR_TRACE_ABI_VERSION = 1`, an append-only trace ABI (`tassadar.trace.v1`) and opcode vocabulary `tassadar.opcodes.v1`.
- Four runner IDs: `tassadar.cpu_reference.v1`, `tassadar.fixture_runner.v1`, `tassadar.hull_cache_runner.v1` and `tassadar.sparse_top_k_runner.v1`. A research-only `tassadar.hierarchical_hull_candidate.v0` exists too.
- `TassadarClaimClass`: `CompiledExact`, `CompiledArticleClass`, `LearnedBounded`, `LearnedArticleClass`, `ResearchOnly`. This vocabulary appears in every fixture.
- Decode modes `reference_linear`, `hull_cache` and `sparse_top_k`, with matching caches (linear-scan KV, hull-support, sparse-top-k). There are selection diagnostics for direct versus fallback, and typed `TassadarExactnessRefusalReport` / mismatch classes.
- Nine Wasm profiles, all i32-only:
  - `core_i32.v1` and `v2`
  - `core_i32_w1_1.v1`
  - `article_i32_compute.v1`
  - `sudoku_v0_search.v1`
  - `hungarian_v0_matching.v1`
  - `hungarian_10x10_matching.v1`
  - `sudoku_9x9_search.v1`
  - `sudoku_9x9_hard_search.v1`
- Instruction coverage reports, a trace-ABI decision report, long-horizon trace evidence and a million-step decode benchmark builder.
- `TassadarFixtureWeights`, described in the code as "handcrafted/programmatic rule tables backing the Phase 1 fixture lane".

**What the HullCache runner is.** Reading `execute_program_hull_cache_summary` (around line 9232) shows that the HullCache runner is a direct Rust interpreter:

- It keeps an explicit stack, locals and memory.
- On every step it looks up the opcode's fixture rule and tracks last-write step indices.
- It does no neural forward pass.

This matters for reading the throughput numbers later: the "fast route" figures measure this interpreter, not Transformer inference.

**Other runtime modules** (87 in total):

| Area | Modules |
|---|---|
| Module-scale Wasm | `tassadar_module_execution` (2.8k), `tassadar_module_trace_abi_v2`, `tassadar_memory_abi_v2`, `tassadar_call_frames`, `tassadar_call_frame_resume`, `tassadar_structured_control`, `tassadar_module_linker`, `tassadar_linked_program_bundle`, `tassadar_wasm_conformance`, `tassadar_frozen_core_wasm` |
| Wasm proposal profiles | `simd`, `relaxed_simd`, `memory64`, `multi_memory`, `exception`, `threads_research`, `component_linking`, `float_semantics` |
| Process and continuation | `tassadar_execution_checkpoint`, `tassadar_spill_tape_store_profile`, `tassadar_preemptive_jobs`, `tassadar_process_object_family`, `tassadar_installed_process_lifecycle`, `tassadar_session_process_profile`, `tassadar_hybrid_process_controller`, `tassadar_universal_machine_simulation`, `tassadar_tcm_v1_runtime_contract` (the `TCM.v1` Turing-completeness substrate) |
| Effects | `tassadar_effect_taxonomy`, `tassadar_effect_safe_resume`, `tassadar_effectful_replay_audit_receipts`, `tassadar_virtual_fs_mount_profile`, `tassadar_simulator_effect_profile` |
| Article lane | `tassadar_article_abi`, `tassadar_article_trace_schema`, `tassadar_article_transformer_forward_pass`, `tassadar_article_fast_route_throughput`, `tassadar_article_hard_sudoku_benchmark`, `tassadar_article_cpu_reproducibility`, `tassadar_article_runtime_closeout`, `tassadar_direct_model_weight_execution_proof` |
| Post-article plugin platform (§9) | `tassadar_post_article_starter_plugin_runtime` (4.3k), `..._starter_plugin_catalog_bundle`, `..._plugin_packet_abi_and_rust_pdk`, `..._plugin_runtime_api_and_engine_abstraction`, `..._plugin_invocation_receipts_and_replay_classes`, `..._plugin_world_mount_envelope_compiler_and_admissibility`, `..._weighted_plugin_controller_trace_and_refusal_aware_model_loop`, `..._canonical_computational_model_statement` |

### 6. psionic-ir and psionic-compiler: Wasm lowering, symbolic compile, ALM backend

**The March layer:**

- `psionic-ir/src/tassadar_wasm_module.rs` (2.4k): normalized Wasm module IR with encode/parse. The runtime imports it.
- `tassadar_symbolic.rs`: bounded symbolic IR.
- Also `tassadar_module_manifest`, `tassadar_generalized_abi`, `tassadar_mixed_trajectory`, `tassadar_sparse_rule`, `tassadar_numeric_encoding`, `tassadar_universal_substrate_model`, `tassadar_scratchpad` and `tassadar_state_design`.
- On the compiler side: `tassadar_wasm_module.rs` (2.6k) lowers Wasm text/modules into Tassadar programs, alongside `tassadar_structured_control`, `tassadar_symbolic`, `tassadar_generalized_abi` and `tassadar_locality_scratchpad`.
- Committed sources and binaries sit in `fixtures/tassadar/sources/` and `fixtures/tassadar/wasm/` (35 `.wasm` files): article kernels (arithmetic accumulator, branch dispatch, bump allocator, state-machine router), CLRS shortest-path, Hungarian 10x10, Sudoku 9x9, the TAS-177/178 refusal cases, and module suites (checksum, memcpy, parsing, vm-style).

**The June ALM "executor-compiler" series** is a separate, cleaner construction built around an "Append-only Lookup Machine":

- **E1** `psionic-ir/src/tassadar_alm_graph.rs` (`tassadar.alm_gate_graph.v1`):
  - Five primitive families: input; const/linear; ReGLU product; keyed channel write/read (latest write wins); cumsum accumulator.
  - Exact i64 evaluator with typed refusals.
  - Limits: 65,536 gates, 256 channels, 1,048,576 steps.
- **E2 / E4** `tassadar_alm_backend.rs`: schedules gates into attention, FFN (ReGLU `value*max(gate,0)`) and wiring rows. The default scheduler is `tassadar_alm_backend_e4_milp_schedule` v2; the legacy feasible-first list scheduler is kept as v1.
- **E2b** `tassadar_alm_geometric.rs`: keyed writes become parabolic points `(2k, -k^2)`, and reads take a hard-max over them. A near-miss argmax is a typed `MissingKey` refusal.
- **E2c** `tassadar_alm_hull.rs`: a Li Chao tree convex-hull-trick fast path, the ALM analog of HullKVCache. On a 2,000-step chain, linear scan takes more than 1M comparisons and hull node visits are over 10x fewer. This is asserted as deterministic counts, not wall clock.
- **E3** `tassadar_alm_stack_isa.rs` and `tassadar_alm_wasm_interpreter.rs` (`tassadar.alm_wasm_interpreter.v1`): a universal interpreter graph that is branch-capable over the Tassadar i32 window.
- **E5** `tassadar_alm_specializer.rs`: first-Futamura specialization of the interpreter on a fixed program.
- **E6-numeric** `tassadar_alm_numeric.rs`: re-encodes the compiled bundle as f64 coefficient arrays (`TassadarAlmNumericModel`). The exactness window `|v| ≤ 2^53` is checked at runtime.
- **Weight modules:** `tassadar_alm_dense_module.rs` and `tassadar_alm_linked_dense_module.rs` produce dense weight modules and linked, composable weight banks "consumed by OpenAgents marketplace rails".
- **Verification:**
  - `tassadar_alm_trace_replay.rs` defines an exact trace-replay verification class.
  - `tassadar_alm_bounded_check.rs` is a differential harness over 400 seeded graphs, up to five legs, requiring zero failures. Its first run found a real cumsum reordering bug: outputs `[2,8,12]` versus `[6,12,12]`. It was fixed in `2da78db3`.
  - `tassadar_alm_softmax_bounds.rs` (W1.1) adds window and softmax bounds.
- **Committed June fixtures:**
  - `tassadar-compiled-program-corpus-v1.json`: 5 programs, `tassadar_alm.numeric_program_corpus.v1`.
  - `tassadar-dense-weight-module-v1.json`: `loop_sum_v1`, backward-branch sum, 80 steps, halted.
  - `tassadar-linked-dense-module-v1.json`: `mul_add_memory_roundtrip`, 12 steps, 2 conformance cases, 3 marketplace refs.

### 7. psionic-models, psionic-transformer, psionic-data

**psionic-models** (47 modules):

- `tassadar.rs`: executor descriptors and the fixture-model lineage.
- `tassadar_executor_transformer.rs`: the first neural executor family. Its claim boundary is `NextTokenOnly` or `GreedyDecodeUnvalidated`.
- `tassadar_executor_attention.rs`: an attention-candidate family.
- `tassadar_sequence.rs`: trace tokenization, vocabulary size 303.
- `tassadar_article_transformer.rs`: the canonical "Attention Is All You Need" encoder-decoder.
- Also `tassadar_decompilable_executor`, `tassadar_module_state_executor`, `tassadar_rust_article_profile` and `tassadar_workload_frontier`.

**Model artifacts** in `fixtures/tassadar/models/`:

- `paper_faithful_v0` (13 KB), `trace_bound_v0` (52 KB) and `trace_bound_trained_v0` (52 KB safetensors, 88 tensors), plus v1 descriptors and lineage contracts.
- The canonical `tassadar-article-transformer-trace-bound-trained-v0` config: vocabulary 303; hidden size 8; FFN 16; 2 heads; 2 encoder and 2 decoder layers; 16,384 max positions.

**psionic-transformer** holds 11 `tassadar_post_article_*_contract.rs` files, for example canonical machine identity lock, fast-route legitimacy and carrier binding, and anti-drift closeout. They are contract types that make `psionic-transformer` the "canonical architecture anchor crate" named in the closeout reports.

**psionic-data** (18 modules):

- `tassadar.rs` (3.4k): sequence dataset contracts, e.g. `oa.tassadar.sudoku_v0.sequence@train-v0`.
- `tassadar_multi_plugin_trace_corpus`, `tassadar_article_interpreter_breadth_{envelope,suite}`, `tassadar_compiled_distillation`, `tassadar_weak_supervision`, `tassadar_program_family_frontier` and `tassadar_kernel_module_scaling`.

### 8. psionic-train and the training runs

**psionic-train** (47 src modules, 40 examples) covers:

- Executor training, runs and telemetry: `tassadar_executor_training`, `_run`, `_telemetry`.
- Postmortem, promotion and 9x9 work: `_postmortem`, `_promotion`, `_9x9_reference_run`, `_9x9_promotion`, `_scale_plan`.
- Hungarian runs: `_hungarian_learned_run`, `_hungarian_10x10_article_run`.
- Hull benchmarking: `_hull_benchmark`.
- Article Transformer: `tassadar_article_transformer_training` and `_weight_production`.
- Supervision and curriculum studies: supervision density, subroutine, receipt, weak and no-hint self-supervision, and the shared-depth and module-state curricula.
- `tassadar_default_train_lane`, `_rehearsal` and `tassadar_train_launcher`, driven by `./TRAIN_TASSADAR` → `cargo run -p psionic-train --example tassadar_train_operator`.
- `tassadar_cpu_transform_training_receipt` (June 23).

**Headline results** from `fixtures/tassadar/runs/` (100 run directories: 27 `sudoku_v0_*`, plus 9x9, Hungarian, article and about 60 `tassadar_*` family runs):

- **First reference run** `sudoku_v0_reference_run_v0` (postmortem, 03-16). Three critical findings:
  - `prompt_trace_boundary_collapse`: all 8 cases diverge at the first target token, with exactness of 15–16 bps.
  - The budget was far too small: 1 step, 1 epoch, 1,024 supervised tokens.
  - Trace length versus curriculum mismatch: targets of 114,913–205,350 tokens per case.
- **The 4x4 lane recovered through iteration.** The iterations were `attention_boundary_v1..v9`, `architecture_comparison_v1..v12`, and the supervision and trainable-surface ablations. Then `sudoku_v0_promotion_v3` (committed 03-17) passed with:
  - trainable surface `relative_target_output_bias_plus_trace_schema_bias`;
  - first-target and first-32-token exactness of 10000 bps;
  - 2 exact trace cases (gate required ≥1).
  - This is prompt-to-first-32-tokens exactness, not full-trace exactness.
- **9x9 did not promote.** `sudoku_9x9_v0_reference_run_v0` has claim class `learned_bounded`, output-head-only training, and `passed=false`. The full sequence overflows context by up to 4,811,021 tokens, and there are 0 full-trace exact cases.
- **Hungarian 10x10 learned** (`hungarian_10x10_v0_learned_article_executor_v0`) is `learned_article_class` and passed. The caveats are explicit in the report:
  - It uses an alternate `hungarian_assignment_frontier` trace family with 169 target tokens and `final_outputs_only` reconstruction.
  - It was optimized on the same fixed benchmark corpus: "benchmark-corpus exactness closure rather than a held-out generalization claim".
- **Article Transformer training** (`tassadar_article_transformer_training_v1`) is a tiny overfit check:
  - hidden 4, 104 trainable scalars, 64 steps;
  - training loss 2.03 → 0.74, train exact 2/2;
  - held-out loss 6.64, held-out exact 0/2.
- **CPU-transform receipt** (`fixtures/tassadar/operator/tassadar_percepta_cpu_transform_training_receipt_v1.json`, June 23):
  - Promise `promise:models.tassadar_percepta_executor.v1` stays `planned`.
  - `is_real_pylon_assignment=false` and `green_gate_satisfied=false`.
  - The commit notes the bounded baseline reports 0/2 exact traces.
  - It exists to feed openagents' `/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts` blocker.

**Compiled executors** (`*_compiled_executor_v0`). These execute exactly but far slower than the CPU reference:

| Workload | Exact traces | Compiled steps/s | CPU reference steps/s | Ratio |
|---|---:|---:|---:|---:|
| sudoku_9x9 | 4/4 | 9,321 | 12.46M | 0.00075 |
| hungarian_10x10 | 4/4 | 1,758 | 10.47M | — |

### 9. psionic-serve, psionic-router, psionic-sandbox, psionic-provider: product and plugin surfaces

- **psionic-serve:**
  - `tassadar.rs` (8.0k) is the dedicated served product (`psionic.executor_trace`, `psionic.article_executor_session`).
  - `tassadar_article_transformer_minimal_frontier` (1.7k).
  - `openai_http/tassadar_post_article_router_plugin_tool_loop_pilot.rs`: an OpenAI-compatible tool-loop pilot.
  - Plus `tassadar_direct_model_weight_execution_proof`, `tassadar_rust_only_article_{reproduction,acceptance_gate_v2}`, `tassadar_execution_unit_registration`, and several publication-verdict modules.
- **psionic-router:** `tassadar_route` (1.6k), `tassadar_evidence_routing`, `tassadar_planner_policy`, `tassadar_composite_routing`, `tassadar_delegation_benchmark`, `tassadar_counterfactual_route_quality`, `tassadar_self_installation_gate`, `tassadar_world_mount_compatibility` and `tassadar_negative_invocation`.
- **psionic-sandbox:** the post-article plugin boundaries (charter authority, capability boundary, world-mount envelope compiler, conformance harness, packet ABI and Rust PDK, runtime API and engine abstraction, weighted controller trace), plus `tassadar_import_policy_matrix`, `tassadar_threads_scheduler_boundary` and `tassadar_virtual_fs_mount_boundary`.
- **psionic-provider:** 112 small modules (10.6k LOC total). They are publication, capability and economic envelopes such as `tassadar_accepted_outcome_binding`, `tassadar_exact_compute_market`, `tassadar_cost_per_correct_job` and `tassadar_broad_general_compute_economic_bridge`, plus mirrors of most eval gates.
- **Starter plugins:** `text_url_extract`, `text_stats`, `http_fetch_text`, `html_extract_readable`, `feed_rss_atom_parse`, with a catalog, tool bridge, workflow controller and an Apple FM plugin session pilot. Each has a run bundle in `fixtures/tassadar/runs/tassadar_post_article_plugin_*_v1`. A capability-free starter-plugin scaffold lives in `fixtures/tassadar/scaffolds/`.

### 10. psionic-eval and psionic-research: gates, audits, research families

**psionic-eval** has 188 modules and 178 examples, the biggest single chunk of code (121.7k LOC). Nearly every module is one gate or report generator:

- Article fast route: architecture selection, exactness, implementation, throughput floor.
- Article equivalence: acceptance gate, blocker matrix, claim checker, final audit.
- Article transformer: forward-pass closure, generalization gate, minimal frontier, reference-linear exactness, weight lineage and production.
- Article benchmarks and reproducibility: hard-Sudoku closure, Hungarian demo parity, frontend compiler envelope, cross-machine reproducibility, route minimality, KV/activation discipline, single-run no-spill closure.
- Core Wasm: frozen core-Wasm closure and the full core-Wasm public acceptance gate.
- Broad internal compute and the Turing-completeness closeout.
- Earlier families: `tassadar_compiled_executor`, `tassadar_compiled_kernel_suite`, `tassadar_architecture_bakeoff`, `tassadar_clrs_wasm_bridge`.

**psionic-research** has 132 modules and 136 examples (41.1k LOC). These are research families such as the architecture bakeoff, learnability gap, trace-family comparison, program-family frontier, verifier-guided search, search-native executor, weak supervision, workload capability frontier summaries and effective-unbounded compute claims. Most are tagged `research_only`.

`fixtures/tassadar/reports/` holds 399 files: about 82 `tassadar_article_*`, 70 `tassadar_post_article_*` and 104 `*_summary.json`.

### 11. Closeout claims and numbers (fixtures/tassadar/reports and runs)

**Article-class fast route** (`runs/article_fast_route_throughput_v1`, HullCache runner, `throughput_floor_green=true`):

| Workload | Steps | Exactness (bps) | Steps/s |
|---|---:|---:|---:|
| Hungarian 10x10 demo | 48,727 | 10000 | 13.4M |
| 9x9 Sudoku demo | 6,823 | 10000 | 5.0M |
| Long-loop kernel | 1,048,575 | 10000 | 1.67M |
| Long-loop kernel | 2,097,151 | 10000 | 1.67M |
| State-machine kernel | 1,230,779 | 10000 | 1.58M |
| State-machine kernel | 2,461,547 | 10000 | 1.58M |

The internal floor is 250k steps/s. Reminder from §5: this is the direct Rust HullCache interpreter.

**Million-step benchmark** (`runs/million_step_loop_benchmark_v0`):

- 1,048,575 steps at 1.04M steps/s on the CPU reference, 10000 bps.
- The serialized trace is 236 MB.
- In this bundle, `hull_cache` and `sparse_top_k` fall back to `reference_linear` (control flow unsupported / validation unsupported).

**Hard Sudoku** (`runs/article_hard_sudoku_benchmark_v1`): the 9x9 test case and the named Arto Inkala puzzle (21 givens) are both exact, with matching behavior digests between CPU reference and fast route. They run in 0.054 s and 0.0001 s against a 180 s ceiling.

**Article-equivalence final audit** (`tassadar_article_equivalence_final_audit_summary.json`, TAS-186):

- 24/24 article lines matched.
- Mechanistic, behavioral and operational verdicts are all green; `public_article_equivalence_claim_allowed=true`.
- Canonical model: `trace-bound-trained-v0`. Canonical route: `tassadar.article_route.direct_hull_cache_runtime.v1`.
- Machines: `host_cpu_aarch64` and `host_cpu_x86_64`. Open issue: TAS-R1.
- The blocker matrix records 7 blockers, 0 open.
- Its non-implications are explicit: not arbitrary C/Wasm, and not a "generic public interpreter-in-weights claim".

**Direct model-weight execution proof** (v3): 3 canonical workloads, all direct, fallback-free and with zero external calls, on the `reference_linear` route. It is justified by a fixture-to-Transformer parity certificate plus the weight-lineage contract. Its own claim boundary says it does not claim fast-route closure or benchmark parity.

**Rust-only article closeout** reports `green=true` with the claim: "reproduces the full Rust-only Percepta article claim end to end on the committed canonical article workloads". It lists 5 exclusions: no arbitrary Rust/Wasm, no non-CPU backend, and only two CPU classes.

**Turing-completeness closeout:**

- Status `TheoryGreenOperatorGreenServedSuppressed`, under declared `TCM.v1` semantics via bounded slices, persisted continuation and spill/tape.
- 4 portability envelopes and 4 refusal boundaries.
- Served universality is blocked by 3 conditions, two of which lie outside psionic: kernel policy authority and Nexus accepted-outcome closure.
- The post-article rebased version says the same: theory and operator green, `served_green=false`, `plugin_publication_allowed=false`.

**Bounded weighted plugin platform closeout:** `operator_green_served_suppressed`, with `plugin_capability_claim_allowed=true`, `weighted_plugin_control_allowed=true` and `plugin_publication_allowed=false`.

**Full core-Wasm public acceptance gate:** 4 requirements green (including differential execution parity), 3 suppressed (cross-machine replay, served publication, target-feature coverage), 0 failed. Status: `suppressed`.

### 12. Psion executor lane (fixtures/psion/executor)

- **Footprint:** 46 JSON packets, 46 `crates/psionic-train/src/psion_executor_*` modules and 46 examples. Almost all landed 2026-03-30 in commits whose messages do not mention "tassadar", e.g. `85485e73` "Admit local 4080 executor profile" and `a95dcaf2` "trained-v1 replacement report".
- **What it does:** runs the canonical `trace-bound-trained-v0` model as a Psion "executor" training/eval program. There are local 4080 CUDA and Mac MLX profiles, decision-grade runs, checkpoint/eval/recovery packets, ablations (optimizer, scheduler, batch accumulation, supervision density), mixture policy, and a trained-v1 promotion/replacement.
- **`psion_executor_percepta_closeout_status_v1.json`:** `percepta_closeout_status = green_bounded` over a 3-workload closeout trio.
  - The HullCache versus reference-linear speedup is at least 1.69x; the benchmark packet shows a maximum of 3,258x.
  - The remaining gap versus the CPU reference is at most 2.55x.
  - `research_branch_status=research_only`; the next epic is `PSION-EPIC-8`.
- **`psion_executor_baseline_truth_v1`:** exactness, held-out and adversarial suites are all green for trained-v0, with 3 committed report reproductions.
- **The 4080 decision-grade packet** actually reuses an open-adapter gpt-oss LM-head CUDA run (47,104 steps, 571 s) as its evidence. It is infrastructure evidence, not executor-model training.

### 13. W3 student sweep and H1 (psionic-tassadar-student, June 2026)

**The crate** (openagents#4749) is 5.1k LOC:

- Deterministic f32 math and a hand-derived backprop transformer (`model.rs`, `tensor.rs`).
- A CPU budget guard (one core by default).
- Two binaries, `tassadar-student-train` and `tassadar-student-eval`.
- Evaluation is by "first divergence behind replay — never perplexity".

**The sweep** (`fixtures/tassadar/w3_student_sweep_20260612/README.md`) ran on corpus `corpus.tassadar_trace.v0_2.w3_100m`:

| Baseline | pass@1 | Replay acceptance | Median divergence step | Notes |
|---|---:|---:|---:|---|
| A next-token | 0.0 | 0.0 | 0 | |
| B auxiliary-state | 0.0 | 0.0 | 0 | |
| C lookup-analytic init | 0.0 | 0.0 | 0 | |
| D frozen analytic executor + learned interface | 1.0 | 1.0 | 512 | p90 4096, median valid prefix 10,240 tokens |

Conclusions recorded in the README:

- **H1 supported:** pure next-token trace learning does not produce replay-safe rollouts.
- **H2 supported:** a frozen executor plus a learned interface is the only working route.
- **H3 falsified:** analytic lookup initialization learns the auxiliary but still diverges at step 0.

`hybrid.rs` (`8e74fc2d`) adds the H1 frozen-core hybrid validation gate over the retained baseline D artifacts.

### 14. Scripts and operator entry points

- **`scripts/check-tassadar-*.sh`** (80 scripts):
  - Promotion gates: 4x4 and 9x9.
  - About 25 `article-*` gates: fast-route, equivalence, benchmark, frontend, interpreter, transformer, reproducibility.
  - About 45 `post-article-*` gates: plugins, universality, control-plane proof, machine closure, anti-drift.
  - Plus `acceptance`, `promotion-policy`, `public-disclosure`, `default-train-lane`/`rehearsal`, `minimal-universal-substrate-gate`, `multi-plugin-trace-corpus`, `compiled-article-closure`, and the `rust-only-article-{reproduction,acceptance-v2,closeout-audit}` trio.
- **`TRAIN_TASSADAR`:** repo-root launcher for the default train lane. Its operator artifacts are in `fixtures/tassadar/operator/`: the lane contract, checker receipts, promotion evidence, rehearsal bundle, launch manifest, current run status and retained summary.
- **Other scripts:** a few non-Tassadar scripts mention it incidentally (tailnet matrix, first-swarm, coordinator gcloud job).

### 15. Caveats

1. The published throughput numbers (millions of steps/s) and the "direct HullCache" canonical route come from a Rust interpreter with fixture rule tables, not from running Transformer weights.
2. The canonical trained model is very small (hidden size 8, a 52 KB safetensors file). Its link to exact execution rests on parity certificates and lineage contracts.
3. The truly weights-shaped paths are much slower (compiled executors at 1.7k–9.3k steps/s) or still bounded research:
   - the June ALM numeric and dense modules;
   - the June student sweep, where only the frozen-executor hybrid succeeds.
4. Learned executor claims are narrow: 4x4 first-32-token exactness, Hungarian final outputs only on the training corpus, and 9x9 failed.
5. Served and public universality and plugin publication are explicitly suppressed in every closeout.


---

## Part V — psionic documentation

Repo: `/Users/christopherdavid/work/psionic`, HEAD `02e0bc85` (2026-09-13). This survey only reads documentation. `git grep -il 'tassadar\|percepta' -- docs` matches **202 files**: 91 top-level/`docs/research` docs and 111 under `docs/audits/`.

**Deleted docs:** none. `git log --no-renames --diff-filter=D` finds no deleted doc whose name contains tassadar or percepta. The only tassadar-named deletions (46 paths in `ddfd8945`, 2026-03-16, "repo: align layout with extraction audit") are code files moved into `crates/`. The only doc renames are the `TASSION_*` files, renamed to `PSION_PLUGIN_*` / `PSION_PROGRAM_MAP.md` in `c6c619b6` (2026-03-22, "Rename plugin convergence lane to psion plugin").

**Chronology in one line:** The Percepta blog post "Can LLMs Be Computers?" appeared 2026-03-11. The research memo came 2026-03-16, followed by the phase-by-phase executor build (Phases 1–16, March 13–16). The PTAS/TAS roadmap started 2026-03-17 (`7ea89dbf`). Article-parity closeout came 2026-03-17 (`4c4f9ac6`, `1cf1c79a`), the Turing-completeness closeout 2026-03-19 (`92e97913`), the article-equivalence final audit TAS-186 on 2026-03-21 (`9d1d5201`), and the post-article/plugin wave through TAS-226 on 2026-03-21/22. The Psion-executor ("PSION-0xxx") program followed on 2026-03-30, and the TRAIN_TASSADAR launcher on 2026-04-02. The ALM executor-compiler wave (E1–E6, W1.x, C4/C5) ran 2026-06-10/18, and the CPU-transform training receipt landed 2026-06-23 (`4f36914f`).

---

### 1. Roadmap structure and status

#### 1.1 The three roadmap docs

- **`docs/ROADMAP_TASSADAR.md`** (1019 lines, 144 commits; created `7ea89dbf` 2026-03-17, last touched `24291b45` 2026-03-30 "Mark stale roadmap docs subordinate").
  - It began as a 471-line full roadmap with **Epics 0–6** and **PTAS-numbered issues**: PTAS-001..004, 101..105, 201..205, 301..305, 401..405, 501..505 and 601..605.
  - The epics were: 0 Governance & Acceptance; 1 Wasm Substrate Closure; 2 Fast Decode Closure; 3 Compiled/Proof-Backed Article Closure; 4 Learned Executor Substrate Redesign; 5 Learned Long-Trace Closure; 6 Compile-To-Weights & Hybrid Systems.
  - Its original "Article Target" was: stable Wasm from a canonical compile pipeline, executed inside the executor with no external interpreter; exact CPU parity, trace proofs and lineage; an exact fast decode on long traces; compiled 9x9 Sudoku, 10x10 Hungarian and kernel suites; and learned-lane claims kept separate. "Arbitrary C" would become honest only after coverage, receipts and acceptance runners said so.
  - The original status matrix marked "Full article-parity Wasm compute claim" as `planned`.
  - **Current form:** a "Roadmap Bridge". It states that the live roadmap moved outside the repo, to `/Users/christopherdavid/code/alpha/tassadar/tassadar-llm-as-computer-roadmap.md` (private alpha). The in-repo file is kept because many issues link to it. It is subordinate to `PSION_EXECUTOR_PROGRAM.md`.
  - The body is a long running list of "the repo now also has one…" bullets covering each landed tranche. It ends with a "Still unclaimed" list and a **dependency spine**:
    1. TAS-103–106 real program execution
    2. TAS-107–110 resumable / effect-safe execution
    3. TAS-111–112 portable / publishable execution
    4. TAS-113–114 frozen core-Wasm closure
    5. TAS-115–124 numeric and proposal-family widening
    6. TAS-125–136 process and internal-platform closure
    7. TAS-137–140 learned / hybrid broad-compute comparison
    8. TAS-141–150 public-claim, economic and governance closure
    9. TAS-151–156 universal substrate and Turing-completeness closeout
  - An issue-state note records TAS-103 through TAS-226 as implemented. It also says later TAS-204 weighted-controller work stays separate from the orchestration wave.
- **`docs/ROADMAP_TASSADAR_TAS_SYNC.md`** (250 lines, last `e5f81c69` 2026-03-22). It maps each public `TAS-*` issue in `OpenAgentsInc/psionic` to its landed code and artifact evidence.
  - It has 235 rows and **every row says `implemented`**: TAS-001/#65 … TAS-226, plus letter sub-items (167A, 169A, 171A/B/C, 184A, 185A, 188A, 203A) and the optional `TAS-R1`/#312 (a research-only minimal-size frontier of six reduced article-Transformer candidates).
  - Noted gap: **TAS-084 and TAS-085 have no sync rows**, although the Index lists them (Rust source canon; profile completeness matrix). TAS-214 and TAS-221 are listed out of numeric order.
- **`docs/ROADMAP_TASSADAR_INDEX.md`** (162 lines, ~95 table rows). It maps each landed phase to five things: canonical artifact root, supporting audit, primary validator, and current claim boundary.
  - It covers PTAS-003 (acceptance gate), PTAS-101 (Wasm instruction coverage), then TAS-084 onward through the numeric, proposal-family and post-article tranches.
  - It names the operator entrypoints `TASSADAR_WASM_RUNBOOK.md` and `TASSADAR_RUST_ONLY_ARTICLE_RUNBOOK.md`, and the latest flow status audit `2026-03-18-tassadar-wasm-flow-status-audit.md`.
  - **Stale-wording finding:** the PTAS-003 row still says "article-class and final article closure remain red". The committed `fixtures/tassadar/reports/tassadar_acceptance_report.json` has `compiled_article_class`, `learned_article_class` and `article_closure` all `passed=true`, and `article_parity_language_allowed=true`.

#### 1.2 Earlier "Phase" numbering (pre-roadmap)

`docs/ARCHITECTURE.md` (§"Tassadar Executor-Class Lane", about 650 mentions) records the build as numbered phase bars:

- Phase 1: CPU reference fixture plus parity harness.
- Phase 2: digest-bound program artifacts.
- Phase 3: environment bundle plus benchmark suite.
- Phase 4: trace artifacts and proof bundles.
- Phase 5: exact `HullCache` fast path on the validated acyclic subset.
- Phase 6: capability reports and decode selection.
- Phase 7A–D: served `psionic.executor_trace`, the `article_executor_session`, the `core_i32_v2` profile with article-class benchmarks, the trace ABI decision, and the workload capability matrix.
- Phases 10–16: learned Sudoku-v0 runs, boundary and trainable-surface ablations, attention-transformer family, 4x4 promotion, 9x9 reference run.

Each phase has an audit in `docs/audits/2026-03-16-*`.

#### 1.3 Psion-executor issue spine

`docs/PSION_EXECUTOR_*` uses `PSION-0001`/#700 … `PSION-0808`/#783, all dated 2026-03-30. For example, PSION-0705/#774 is the Percepta closeout status, PSION-0807/#782 the trained-v1 promotion and PSION-0808/#783 the trained-v1 replacement report. `PSION_EXECUTOR_PROGRAM.md` links a workspace-level `docs/ROADMAP_PSION.md` umbrella roadmap. The program lists many more follow-on docs than exist in this grep set; for example, MLX parity, 4080 remote launch and ownership docs mention neither Tassadar nor Percepta.

#### 1.4 ALM executor-compiler phases (June 2026)

- **Phases:** E1 gate-graph IR, E2 feasible-first backend, E2b geometric attention leg, E2c Li Chao hull fast path, E3 stack-ISA and branch-capable window interpreter, E4 scheduler, E5/E5b first-Futamura specializer, E6-numeric materialization.
- **Later items:** W1.1 Wasm window ladder, W1.2 dense module, W1.4 softmax bounds, C4/#5324, C5/#5325 linked dense module.
- **Issue references:** psionic issues #1098–#1108 and OpenAgents issues #5324/#5325/#5528.
- **Status and design:** each doc is marked `implemented_early` or `implemented`. The cross-repo design sketch is `openagents/docs/tassadar/2026-06-10-psionic-alm-compiler-design-speculation.md`.

---

### 2. What "article parity" / Percepta closeout means, and the final claimed status

"The article" is Percepta's post "Can LLMs Be Computers?" (2026-03-11). Its claim is a transformer executing a WebAssembly-style interpreter trace in-model, using 2D hard-max attention heads and a convex-hull (HullKVCache) lookup for fast exact decode. Showcase workloads are hard Sudoku and 10x10 Hungarian matching, with million-step traces. The repo's word for this is **"article-shaped"**, and closure has four successively stronger levels.

1. **Article parity (2026-03-17).** `docs/audits/2026-03-17-tassadar-article-parity-closeout-audit.md` says: "Final article-parity closure is green."
   - The verdict is subordinate to `tassadar_acceptance_report.json`, with `article_parity_language_allowed=true`, and compiled-article-class, fast-path-declared-workload-exact, learned-article-class and article_closure all passing.
   - The evidence is compiled Sudoku-9x9 and Hungarian-10x10 executors, a compiled kernel suite, the neural hull benchmark, and a learned Hungarian-10x10 bundle.
   - Boundary: the learned lane is scoped to benchmark-corpus exactness on the committed Hungarian bundle.
2. **Rust-only article closeout (TAS-084..088, 2026-03-18).** The frontend is rooted in committed Rust fixtures under `fixtures/tassadar/sources/*.rs`. A bounded ABI covers scalar `i32` and pointer-length `i32` heap inputs. Canonical reproducers exist for `hungarian_10x10_test_a` and Sudoku-9x9, with direct no-tool proof receipts.
   - The served profile is `tassadar.internal_compute.article_closeout.v1`.
   - The audited runtime is the exact reference-linear CPU lane. Faster families are explicitly not "the default public route".
3. **Article equivalence (TAS-157..186 plus TAS-R1, 2026-03-19..21).** This rebuilt the article on an owned paper-faithful Transformer (`psionic-transformer`) and ran a seven-category blocker matrix (TAS-157) and an acceptance gate (TAS-158).
   - Model work covered the stack boundary, attention and mask, blocks, the encoder-decoder model, training closure, forward-pass closure, the trace vocabulary, the artifact descriptor, and weight lineage and production.
   - Gates covered reference-linear exactness, generalization, evaluation independence, representation invariance, fast-route implementation, exactness and throughput floor, Hungarian demo parity (TAS-180), hard-Sudoku closure including the named Arto Inkala puzzle (TAS-181), the demo/benchmark gate (TAS-182), single-run no-spill million-step closure (TAS-183), interpreter ownership (TAS-184), KV/activation discipline (TAS-184A), cross-machine reproducibility (TAS-185) and route minimality (TAS-185A).
   - **TAS-186 final audit** (`docs/audits/2026-03-21-tassadar-article-equivalence-final-audit.md`; fixture `tassadar_article_equivalence_final_audit_report.json`) claims:
     - 24 article lines, all matched to closed blockers;
     - mechanistic, behavioral and operational verdicts all green;
     - `public_article_equivalence_claim_allowed=true` and `article_equivalence_green=true`.
   - The claim is fixed to one canonical model (`tassadar-article-transformer-trace-bound-trained-v0`), its weight artifact, the direct deterministic `HullCache` route (`tassadar.article_route.direct_hull_cache_runtime.v1`), and a two-class CPU machine matrix (`host_cpu_x86_64`, `host_cpu_aarch64`).
4. **Psion-executor Percepta closeout status (PSION-0705/#774, 2026-03-30, `f03acff7`).** Source: `docs/PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md`.
   - It is one typed packet with verdict `red | partial | green_bounded`. The final verdict is **`green_bounded`**.
   - Workload truth (frozen trio `long_loop_kernel`, `sudoku_v0_test_a`, `hungarian_matching`) is green. Fast-path truth (HullKVCache) is green. Route-replacement truth (Mac export inspection and carrier binding) is green. The 2D-head research branch is `research_only`.
   - Minimum HullKVCache speedup over reference_linear is **1.69x**. The maximum remaining gap versus direct CPU is **2.55x**, meaning it is still slower than native CPU.
   - Remaining limitations are `arbitrary_c_or_wasm_not_claimed`, `research_branch_remains_research_only`, and `trained_v1_candidate_promotion_moves_to_psion_epic_8`.
   - Packet digest: `9856bfc3…1185`.
   - **Follow-on:** `PSION_EXECUTOR_TRAINED_V1_PROMOTION.md` (PSION-0807) and `…_REPLACEMENT_REPORT.md` (PSION-0808) promote `tassadar-article-transformer-trace-bound-trained-v1` on the same route.
     - Bounded claim status: `green_bounded_replacement_ready`.
     - It has 10 preserved gates and 7 improved metrics.
     - Deltas: HullCache speedup +0.059, CPU gap −0.063, exactness +8 bps, held-out +1 bp.
     - It is explicitly "does not widen the executor lane".

---

### 3. Grouped inventory

#### 3.1 Roadmaps, indexes, program maps

- `docs/ROADMAP_TASSADAR.md`: roadmap bridge, posture summary, dependency spine.
- `docs/ROADMAP_TASSADAR_INDEX.md`: map from phase to artifact, validator and claim boundary.
- `docs/ROADMAP_TASSADAR_TAS_SYNC.md`: map from TAS issue to implementation evidence (235 rows, all implemented).
- `docs/ROADMAP.md`: full-library roadmap. Tassadar appears as an `implemented_early`, library-owned executor-class reference lane (64 mentions).
- `docs/ARCHITECTURE.md`: system spec. It has the §Tassadar Executor-Class Lane phase bars and the status vocabulary.
- `docs/WORKSPACE_MAP.md`: crate map; incidental mentions.
- `docs/PSION_EXECUTOR_PROGRAM.md`: the naming rule. Psion is the umbrella; Tassadar is the executor-capable bounded Psion profile and route family, separate from the generic compact-decoder Psion lane.
- `docs/PSION_PROGRAM_MAP.md`, `docs/PSION_ACCEPTANCE_MATRIX.md`: generic Psion lane docs that point executor closure to Tassadar.
- `docs/research/tassadar.md` (2026-03-16): literature map around the Percepta post. It covers Turing-completeness theory, RASP/Tracr/ALTA compile-to-weights, NTM/NAR/CLRS, and efficient attention. It recommends keeping compiled and learned lanes separate, richer trace supervision, parallel-algorithm traces, recurrence, sparse baselines and broader evaluations.

#### 3.2 Operator runbooks, governance, disclosure

- `docs/TASSADAR_WASM_RUNBOOK.md`: canonical operator guide for the bounded Wasm flow (334 mentions).
- `docs/TASSADAR_RUST_ONLY_ARTICLE_RUNBOOK.md`: one-command Rust-only article reproduction.
- `docs/TASSADAR_PUBLIC_DISCLOSURE_FLOW.md`: checklist for moving alpha/private Tassadar material into public psionic without over-claiming.
- `docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md`: canonical owned-Transformer boundary. It records the bounded article-equivalence verdict on the HullCache route (275 mentions).

#### 3.3 Train lane (TRAIN_TASSADAR)

- `docs/TASSADAR_DEFAULT_TRAIN_LANE.md` (2026-04-02): "train Tassadar" means the bounded trace-bound article-transformer weight-production lane that produces `…trained-v0`.
  - It runs on a CPU reference writer, and the checkers are `check-tassadar-default-train-lane.sh` and `check-tassadar-acceptance.sh`.
  - It is explicitly not the 4x4 promotion, the 9x9 reference, Hungarian learned, or the 4080 lanes.
- `docs/TASSADAR_DEFAULT_TRAIN_REHEARSAL.md`: bounded rehearsal bundle for the default lane.
- `docs/TASSADAR_TRAIN_LAUNCHER.md`: `./TRAIN_TASSADAR start|dry-run|status`.
  - Lanes: `…trace_bound_trained_v0` (default), `tassadar_hungarian_10x10_article_learned_v0`, `tassadar_sudoku_v0_promotion_v3`.
  - It uses the same run-root layout (manifests/status/retained_summary) as the Psion actual lane.
- `docs/TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT.md` (2026-06-23, `4f36914f`, psionic#1140, OpenAgents EPIC DE-5 #5528): a dereferenceable local CPU training receipt for the `models.tassadar_percepta_executor.v1` product promise.
  - Honest result: the 1-epoch reference model replays **0/2** validation traces exactly.
  - The Pylon assignment, accepted-work, settlement and green-transition gates stay unsatisfied, so the promise stays `planned`.
- `docs/TRAIN_SYSTEM.md`: train-system spec that documents the default Tassadar contract and launcher (§ around line 1529).

**Relationship to psion `./TRAIN`:** the repo-root `TRAIN` defaults to lane `actual_pretraining` (generic Psion pretraining), with `reference_pilot` and `cs336_a1_demo` also available. Tassadar has its own `TRAIN_TASSADAR` script, which calls the `tassadar_train_operator` example. The two share only the operator run-root pattern and the psionic-train substrate. The workspace `./TRAIN` / "train Psion" lane is **not** Tassadar. `PSION_PLUGIN_PROGRAM_MAP.md` makes the plugin-conditioned Psion x Tassadar convergence subordinate to the actual-pretraining continuation `pretrain -> general_sft -> agentic_sft`.

#### 3.4 Psion executor lane (PSION-0xxx; all 2026-03-30)

- **Program and governance:** `PSION_EXECUTOR_PROGRAM` (umbrella split), `…_ACCEPTANCE_PROFILE`, `…_ARTIFACT_NAMING` (phase one keeps live `tassadar-*` artifact ids), `PSION_ARTIFACT_ID_MIGRATION_DECISION` (`do_not_migrate`).
- **Baselines and evaluations:** `…_BASELINE`, `…_BASELINE_TRUTH` (trained-v0 truth packet), `…_EVAL_PACKS` (frozen frequent and promotion packs), `…_FORMATTING_AUDIT`, `…_ARTICLE_CLOSEOUT_SET` (frozen trio), `…_TRACE_NATIVE_METRICS`, `…_HULL_CACHE_BENCHMARK` (HullKVCache versus reference_linear, with a promotion-block rule).
- **Data and curriculum:** `…_CANONICAL_MIXTURE_V0`, `…_CURRICULUM_BOUNDARIES`.
- **Ablations:** `…_OPTIMIZER_ABLATION`, `…_SCHEDULER_ABLATION`, `…_BATCH_ACCUMULATION_ABLATION`, `…_SUPERVISION_DENSITY_ABLATION`, `…_TRACE_FAMILY_WEIGHTING_ABLATION`.
- **Local cluster:** `…_LOCAL_PROFILE_REFERENCE`, `…_MLX_SMOKE_RUN`, `…_LOCAL_CLUSTER_RUN_REGISTRATION`, `…_LOCAL_CLUSTER_LEDGER`, `…_LOCAL_CLUSTER_ROUNDTRIP` (Mac to 4080 to Mac), `…_UNIFIED_THROUGHPUT_REPORTING`.
- **Closeout and promotion:** `…_RESEARCH_BRANCH` (research-only 2D-head branch), `…_PERCEPTA_CLOSEOUT_STATUS` (`green_bounded`), `…_TRAINED_V1_PROMOTION`, `…_TRAINED_V1_REPLACEMENT_REPORT`.
- **Family serve:** `PSION_FAMILY_SERVE_VOCABULARY`, `PSION_GENERIC_LOAD_AND_GENERATE`. Both keep the executor route distinct from generic Psion serving.

#### 3.5 Starter plugins, orchestration, and the Psion plugin convergence (2026-03-22)

- **Starter plugins:** `TASSADAR_STARTER_PLUGIN_CATALOG`, `…_RUNTIME` (runtime-owned `plugin.text.url_extract`, `plugin.http.fetch_text`, `plugin.html.extract_readable`, `plugin.feed.rss_atom_parse`, text stats), `…_TOOL_BRIDGE`, `…_WORKFLOW_CONTROLLER` (deterministic host controller), `…_AUTHORING`, `…_USER_AUTHORING_WAVE`.
- **Plugin loops, orchestration and traces:**
  - `TASSADAR_ROUTER_PLUGIN_TOOL_LOOP`: router-owned served tool loop.
  - `TASSADAR_APPLE_FM_PLUGIN_SESSION`: local Apple FM controller lane.
  - `TASSADAR_MULTI_PLUGIN_TRACE_CORPUS`: TAS-226.
  - `TASSADAR_MULTI_PLUGIN_ORCHESTRATION_WAVE`: TAS-221 umbrella closeout.
- **Psion plugin docs:**
  - `PSION_PLUGIN_PROGRAM_MAP`: the proposed Psion x Tassadar plugin-conditioned training map.
  - `PSION_PLUGIN_CLAIM_BOUNDARY_AND_CAPABILITY_POSTURE`: the combined claim boundary.
  - `PSION_PLUGIN_GUEST_ARTIFACT_DIRECTION`: product direction for guest-artifact plugin support.
  - `PSION_PLUGIN_TRACE_DERIVATION`: derivation of training data from plugin-runtime traces.
  - `PSION_PLUGIN_TRAINING_RECORD_SCHEMA`: the plugin-training record format.

#### 3.6 ALM executor-compiler (program-to-weights; June 2026)

- `TASSADAR_ALM_GRAPH` (E1, #1098): Append-only Lookup Machine gate-graph IR with exact evaluator.
  - It has five gate families: Input, Const/Linear, ReGlu, ChannelWrite/Read (2D lookup head) and CumSum (uniform-key head).
  - Semantics are checked i64 only; it emits no weights.
- `TASSADAR_ALM_BACKEND` (E2/E4): feasible-first scheduler, slot allocator and compiled bundle.
- `TASSADAR_ALM_GEOMETRIC` (E2b): keyed reads realized as geometric attention.
- `TASSADAR_ALM_HULL` (E2c): Li Chao hull fast path.
- `TASSADAR_ALM_STACK_ISA` (E3 bounded): universal stack-ISA interpreter.
- `TASSADAR_ALM_WASM_INTERPRETER` (E3 full): branch-capable i32 window interpreter.
- `TASSADAR_ALM_SPECIALIZER` (E5/E5b): first Futamura projection.
- `TASSADAR_ALM_NUMERIC` (E6): f64 re-encoding with window refusal.
- `TASSADAR_ALM_TRACE_REPLAY` (#1106): exact trace-replay verification class.
- `TASSADAR_ALM_BOUNDED_CHECK` (#1107): differential harness; it found two scheduler bugs.
- `TASSADAR_SYMBOLIC_ALM_BRIDGE` (#1105): bounded symbolic IR lowered to ALM.
- `TASSADAR_WASM_WINDOW_ALIGNMENT` (#1108): opcode map from psionic `core_i32_v2` (11 opcodes, 12 in the article profile) to Percepta's `transformer-vm` (36 dispatch opcodes plus lowering). It is doc-only.
- `TASSADAR_WASM_WINDOW_LADDER` (W1.1, C4/#5324): opcode-window ladder.
- `TASSADAR_ALM_DENSE_MODULE` (W1.2): dense loadable weight module.
- `TASSADAR_ALM_SOFTMAX_BOUNDS` (W1.4): analytic bound from hard-max to softmax.
  - With n=1024, d=1 and beta=32, non-winner mass is below 1.4e-11.
  - No softmax execution.
- `TASSADAR_ALM_LINKED_DENSE_MODULE` (C5/#5325): two digest-pinned dense banks (`mul_add_v1`, `memory_roundtrip_v1`) linked with exact projected replay. OpenAgents consumes it as the first compiled-weight-module listing, with no purchase authority.

#### 3.7 Incidental mentions (one or two hits)

- **Compiled agent:** `COMPILED_AGENT_LEARNING_RECEIPTS`, `COMPILED_AGENT_PHASE_SIX`, `COMPILED_AGENT_TAILNET_FIRST_PILOT`, `COMPILED_AGENT_XTRAIN`.
- **Khala coordinator:** `COORDINATOR_EVOLUTION_TRAINING`, `KHALA_M6_M7_COORDINATOR_PLAN` (June 2026).
- **Fable-authored June 2026 docs:** `PSIONIC_PHILOX_RNG`, `PSIONIC_TERNARY_TQ_FORMATS`, `PSION_SPARTA_CANARY`.
- **Other:** `RESEARCH_EXPERIMENT_REFERENCE`, `ROADMAP_PARAMETERGOLF`, `turboquant.md`.

#### 3.8 `docs/audits/` (111 files), grouped by date and family

**2026-03-16: first learned runs, phases 12–16, extraction (9)**
- `2026-03-16-tassadar-first-run-postmortem.md`: first Sudoku-v0 learned run.
  - Every case diverged at token 0, validation exact-trace was 0/2, and case exactness was 9–16 bps.
  - Phase 10 doubled hull decode to 42k tok/s, but model quality was unchanged.
- `2026-03-16-tassadar-phase-12-boundary-audit.md`
- `2026-03-16-tassadar-phase-13-trainable-surface-audit.md`
- `2026-03-16-tassadar-phase-14-blocker-audit.md`
- `2026-03-16-tassadar-phase-14-promotion-green-audit.md`
- `2026-03-16-tassadar-phase-16-9x9-reference-run-audit.md`
- `2026-03-16-tassadar-promotion-v2-teacher-forced-audit.md`
- `2026-03-16-psionic-extraction-audit.md`, `2026-03-16-attnres-port-into-psionic-and-wgpui-audit.md`: incidental.

**2026-03-17: article parity (4)**
- `2026-03-17-tassadar-article-parity-closeout-audit.md`: "Final article-parity closure is green."
- `2026-03-17-tassadar-compiled-weight-widening-audit.md`
- `2026-03-17-tassadar-economic-kernel-workload-audit.md`
- `2026-03-17-tassadar-learned-article-closure-audit.md`

**2026-03-18: Rust-only closeout and TAS-102 queue close (4)**
- `2026-03-18-tassadar-article-cpu-reproducibility-audit.md`
- `2026-03-18-tassadar-post-tas-102-final-audit.md`: closes TAS-084..102.
  - The open `tassadar` issue queue was empty.
  - The served profile was still `article_closeout.v1`, with 2 implemented and 6 planned profiles.
- `2026-03-18-tassadar-rust-only-article-closeout-audit.md`
- `2026-03-18-tassadar-wasm-flow-status-audit.md`

**2026-03-19: fast route, universality, TC closeout (13)**
- `2026-03-19-tassadar-article-frontend-compiler-envelope.md` (TAS-176)
- `2026-03-19-tassadar-article-interpreter-breadth-suite-gate.md` (TAS-179A)
- `2026-03-19-tassadar-effective-unbounded-compute-claim-audit.md`
- `2026-03-19-tassadar-fast-route-exactness.md`
- `2026-03-19-tassadar-fast-route-implementation.md`
- `2026-03-19-tassadar-fast-route-throughput-floor.md` (TAS-175)
- `2026-03-19-tassadar-general-internal-compute-red-team-audit.md`
- `2026-03-19-tassadar-hard-sudoku-benchmark-closure.md` (TAS-181)
- `2026-03-19-tassadar-hungarian-article-demo-parity.md` (TAS-180)
- `2026-03-19-tassadar-pre-closeout-universality-audit.md`
- `2026-03-19-tassadar-tcm-v1-substrate-audit.md`
- `2026-03-19-tassadar-turing-completeness-closeout-audit.md` (TAS-156): TC "for theory and operator use" under `TCM.v1`, with served/public universality suppressed.
- `2026-03-19-tassadar-universality-verdict-split-audit.md`: theory green, operator green, served false.

**2026-03-20: article-equivalence build-out on the owned Transformer (24)**
- **Transformer stack and model:**
  - `2026-03-20-tassadar-existing-substrate-inventory.md` (TAS-159)
  - `2026-03-20-tassadar-canonical-transformer-stack-boundary.md` (TAS-160)
  - `2026-03-20-tassadar-attention-primitive-mask-closure.md` (TAS-161)
  - `2026-03-20-tassadar-transformer-block-closure.md` (TAS-162)
  - `2026-03-20-tassadar-article-transformer-model-closure.md` (TAS-163)
  - `2026-03-20-tassadar-article-transformer-training-closure.md` (TAS-164)
  - `2026-03-20-tassadar-article-transformer-forward-pass-closure.md` (TAS-165)
  - `2026-03-20-tassadar-owned-transformer-stack-audit.md` (TAS-166)
- **Trace vocabulary, artifacts and weights:**
  - `2026-03-20-tassadar-article-trace-vocabulary-binding.md` (TAS-167)
  - `2026-03-20-tassadar-article-representation-invariance.md` (TAS-167A)
  - `2026-03-20-tassadar-article-transformer-artifact-descriptor.md` (TAS-168)
  - `2026-03-20-tassadar-article-transformer-weight-lineage-contract.md` (TAS-169A)
  - `2026-03-20-tassadar-article-transformer-weight-production-run.md`
  - `2026-03-20-tassadar-article-fixture-transformer-parity.md` (TAS-170)
- **Exactness, generalization and independence gates:**
  - `2026-03-20-tassadar-transformer-direct-proof-closure.md` (TAS-171)
  - `2026-03-20-tassadar-reference-linear-transformer-exactness-gate.md` (TAS-171A)
  - `2026-03-20-tassadar-transformer-generalization-gate.md`
  - `2026-03-20-tassadar-article-evaluation-independence-audit.md`
- **Blocker matrix, acceptance and closure gates:**
  - `2026-03-20-tassadar-article-demo-benchmark-equivalence-gate.md` (TAS-182)
  - `2026-03-20-tassadar-article-single-run-no-spill-closure.md` (TAS-183)
  - `2026-03-20-tassadar-article-equivalence-blocker-matrix.md` (TAS-157)
  - `2026-03-20-tassadar-article-equivalence-acceptance-gate.md` (TAS-158)
- **Plugin system and TC rebase:**
  - `2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md` (134 mentions): the big planning audit for the plugin plane.
  - `2026-03-20-tassadar-post-article-turing-completeness-audit.md` (128 mentions): the rebase plan.

**2026-03-21: final article audit, post-article machine, plugin platform (38)**
- **Article closure:**
  - `2026-03-21-tassadar-article-equivalence-final-audit.md` (TAS-186)
  - `2026-03-21-tassadar-article-interpreter-ownership-gate.md` (TAS-184)
  - `2026-03-21-tassadar-article-kv-activation-discipline-audit.md` (TAS-184A)
  - `2026-03-21-tassadar-article-cross-machine-reproducibility-matrix.md` (TAS-185)
  - `2026-03-21-tassadar-article-route-minimality-audit.md` (TAS-185A)
- **Post-article universality rebase:**
  - `…-post-article-universality-bridge-contract.md` (TAS-187)
  - `…-post-article-canonical-route-semantic-preservation-audit.md` (TAS-188)
  - `…-post-article-control-plane-decision-provenance-proof.md` (TAS-188A)
  - `…-post-article-carrier-split-contract.md` (TAS-189)
  - `…-post-article-universal-machine-proof-rebinding.md` (TAS-190)
  - `…-post-article-universality-witness-suite-reissue.md` (TAS-191)
  - `…-post-article-canonical-route-universal-substrate-gate.md` (TAS-192)
  - `…-post-article-universality-portability-minimality-matrix.md` (TAS-193)
  - `…-post-article-rebased-universality-verdict-split.md` (TAS-194)
  - `…-post-article-turing-completeness-closeout-audit.md` (TAS-196)
- **Plugin plane:**
  - `…-post-article-plugin-capability-boundary.md` (TAS-195)
  - `…-post-article-plugin-charter-authority-boundary.md` (TAS-197)
  - `…-post-article-plugin-manifest-identity-contract.md` (TAS-198)
  - `…-post-article-plugin-packet-abi-and-rust-pdk.md` (TAS-199)
  - `…-post-article-plugin-runtime-api-and-engine-abstraction.md` (TAS-200)
  - `…-post-article-plugin-invocation-receipts-and-replay-classes.md` (TAS-201)
  - `…-post-article-plugin-world-mount-envelope-compiler-and-admissibility.md` (TAS-202)
  - `…-post-article-plugin-conformance-sandbox-and-benchmark-harness.md` (TAS-203)
  - `…-post-article-plugin-result-binding-schema-stability-and-composition.md` (TAS-203A)
  - `…-post-article-weighted-plugin-controller-trace-and-refusal-aware-model-loop.md` (TAS-204)
  - `…-post-article-plugin-authority-promotion-publication-and-trust-tier-gate.md` (TAS-205)
  - `…-post-article-bounded-weighted-plugin-platform-closeout-audit.md` (TAS-206)
- **Machine-closure contracts (TAS-207..215):**
  - `…-post-article-canonical-machine-identity-lock.md`
  - `…-post-article-canonical-computational-model-statement.md`
  - `…-post-article-execution-semantics-proof-transport-audit.md`
  - `…-post-article-continuation-non-computationality-contract.md`
  - `…-post-article-fast-route-legitimacy-and-carrier-binding-contract.md`
  - `…-post-article-equivalent-choice-neutrality-and-admissibility-contract.md`
  - `…-post-article-downward-non-influence-and-served-conformance.md`
  - `…-post-article-anti-drift-stability-closeout.md` (TAS-214)
  - `…-post-article-canonical-machine-closure-bundle.md` (TAS-215)
- **Starter catalog and orchestration:**
  - `2026-03-21-tassadar-post-article-starter-plugin-catalog.md`
  - `2026-03-21-multi-plugin-real-run-orchestration-audit.md`

(All `…` prefixes above are `2026-03-21-tassadar`.)

**2026-03-22: starter plugins, orchestration wave, full-state audits (16)**
- **Plugin implementations:**
  - `2026-03-22-tassadar-post-article-plugin-text-url-extract.md`
  - `2026-03-22-tassadar-post-article-plugin-http-fetch-text.md`
  - `2026-03-22-tassadar-post-article-plugin-html-extract-readable.md`
  - `2026-03-22-tassadar-post-article-plugin-feed-rss-atom-parse.md`
  - `2026-03-22-tassadar-post-article-plugin-text-stats-and-user-authoring-path.md`
- **Bridge, controller and sessions:**
  - `2026-03-22-tassadar-post-article-starter-plugin-tool-bridge.md`
  - `2026-03-22-tassadar-post-article-starter-plugin-workflow-controller.md`
  - `2026-03-22-tassadar-post-article-router-plugin-tool-loop.md`
  - `2026-03-22-tassadar-post-article-apple-fm-plugin-session.md`
- **Traces and waves:**
  - `2026-03-22-tassadar-multi-plugin-trace-corpus.md`
  - `2026-03-22-tassadar-real-run-multi-plugin-orchestration-wave.md`
  - `2026-03-22-tassadar-starter-plugin-user-authoring-wave.md`
- **State audits:**
  - `2026-03-22-tassadar-plugin-runtime-collision-and-continuation-audit.md`
  - `2026-03-22-tassadar-full-plugin-system-state-audit.md`
  - `2026-03-22-psion-full-system-state-and-operations-audit.md`, `2026-03-22-psion-training-system-full-state-audit.md`: Psion-wide, with incidental Tassadar mentions.

**Later (3)**
- `2026-03-24-psionic-turboquant-integration-audit.md` (incidental)
- `2026-04-02-mesh-llm-vs-psionic-and-project-product-surfaces-audit.md` (incidental)
- `2026-06-18-cutile-rust-gpu-concurrency-adaptation-audit.md`: proposes a Psionic launch-contract lane so the public `/tassadar`, `/api/public/tassadar-run-summary` and proof-replay surfaces, and the promise `compute.tassadar_executor_poc.v1`, can distinguish CPU versus GPU evidence.

---

### 4. Key findings, results and claim boundaries

#### 4.1 What is claimed (green, bounded)

- **Article parity (2026-03-17) and article equivalence (TAS-186, 2026-03-21).** Both are green.
  - Envelope: one owned-Transformer canonical model and weights, one direct deterministic `HullCache` route, and the declared CPU machine matrix. The canonical workloads are committed Rust fixtures such as hungarian_10x10_test_a, Sudoku-9x9 including Arto Inkala, and long loops, with million-step single-run no-spill.
  - `public_article_equivalence_claim_allowed=true` and 24/24 article lines are matched.
- **Percepta closeout `green_bounded` (PSION-0705).** HullKVCache is at least 1.69x faster than reference-linear but still up to 2.55x slower than direct CPU. `trained-v1` replacement is `green_bounded_replacement_ready`.
- **Bounded Turing completeness** (TAS-156, rebased to the canonical route in TAS-196). It is green for **theory and operator** use under `TCM.v1` semantics, which cover bounded slices, persisted continuation, spill/tape extension, and witness machines (two-register and single-tape).
- **Bounded weighted plugin platform** (TAS-206). It is an operator/internal-only statement, backed by real starter plugins and a multi-plugin orchestration wave (TAS-221/226).
- **Numeric widening.** Exact scalar `f32` and mixed `i32`/`f32` profiles are published for the cpu-reference envelope only.
- **Proposal-family widening.** Exceptions, memory64, multi-memory, component linking and SIMD are named profiles on the cpu-reference envelope; they are not served by default.
- **ALM compiler.** Integer-exact analytical program-to-weights over bounded windows, with exact trace replay, dense and linked modules, and a softmax approximation certificate.

#### 4.2 What is explicitly not claimed (repeated across docs)

- Arbitrary C execution and arbitrary Wasm ingress or execution.
- Full frozen core-Wasm closure. TAS-114 verdict: `not_closed`, `served_publication_allowed=false`.
- Broad internal computation as a generally served capability.
- Served/public Turing-complete support or universality (`served_green=false`). Authority-bearing served closure lives outside standalone psionic.
- "The Percepta article headline in its broadest frontend/runtime reading".
- Treating the faster route as the default public article route. The audited closeout runtime is reference-linear CPU.
- Planner-mediated or hybrid canonical routes, resumed or stochastic execution, and generic interpreter-in-weights claims outside the envelope.
- Backend-invariant float exactness, full `f64`, and relaxed-SIMD promotion; relaxed SIMD and threads are research-only.
- Learned generality. The learned lane is scoped to benchmark-corpus exactness. The first learned run failed badly (0/2), and the June CPU-transform receipt model still replays 0/2 exactly.
- **Product and settlement:** there is no trained Tassadar product model, accepted Pylon work, real settlement, hosted inference or CPU outperformance. `models.tassadar_percepta_executor.v1` stays `planned`, and the ALM linked module carries no purchase authority.

#### 4.3 Documentation-hygiene observations

- The live roadmap is private: `~/code/alpha/tassadar/tassadar-llm-as-computer-roadmap.md`. The public bridge defers tranche definitions there, so the public repo alone does not show sequencing after TAS-102.
- `ROADMAP_TASSADAR_INDEX.md` has a stale PTAS-003 claim-boundary cell ("final article closure remain red"), contradicted by the committed acceptance report (all green).
- `ROADMAP_TASSADAR_TAS_SYNC.md` has no rows for TAS-084/TAS-085, even though the Index cites them. It has not been updated since 2026-03-22, so it does not cover the PSION-executor or ALM June work.
- All Tassadar roadmap docs were frozen on 2026-03-30 as "subordinate" (`24291b45`). Later work (April TRAIN_TASSADAR; June ALM, receipts, cuTile) is documented only in standalone docs, `TRAIN_SYSTEM.md` and `ARCHITECTURE.md`, not in the roadmap, Index or TAS sync.
- Audits are consistently "subordinate to" machine-readable fixtures under `fixtures/tassadar/reports/`. The docs treat those fixtures plus the `scripts/check-tassadar-*.sh` checkers as the authority, and the prose as observational.
