# Tassadar Training Run + Percepta Implementation — Status Audit for Artanis

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-27
Author: audit for Artanis hand-off (autonomy epic #6359)
Scope: public-safe. No secrets, no private `alpha/` strategy verbatim.

This document exists so Artanis can pick up the Tassadar training run and the
Percepta executor implementation, understand exactly where they stand right now,
and autonomously fan out new work and research with his own tools
(`read_repo_file`, `dispatch_codex_task`) under the autonomy epic #6359.

Every claim below is grounded in a real file, commit, issue, or live endpoint.
Where a memory or prior assumption is restated, it is reconfirmed from the repo,
not taken on faith.

---

## 0. TL;DR

- The Tassadar run `run.tassadar.executor.20260615` is **live and active right
  now** (verified 2026-06-27 against the public endpoint): `state: active`,
  `acceptedTraceCount: 12`, `observedDistinctContributorDevices: 5` (required 2),
  `settlement.reconciledState: settling`, `settledPayoutSats: 1020`,
  `settledReceiptCount: 5`.
- The launch gate / first independent pairing went **green ~2026-06-16** and is
  real: an independent worker↔validator pairing produced a `Verified` verdict and
  auto-streamed real Bitcoin. This is confirmed, not just remembered.
- **This is not gradient-descent training.** Tassadar's run is an
  exact-execution + verification-by-replay economy. The honest gap is that it
  verifies and pays for **one fixed compiled program forever** rather than a
  growing, composable, constructed capability corpus.
- The separate **actual-pretraining** lane (psionic `./TRAIN`, default lane) is
  real Rust pretraining at bounded scale, but its **dispatch/verification/payment
  rails** are still mostly red/planned. Do not conflate it with the Tassadar
  executor run; they are different lanes.
- **Percepta** is roughly 60% built in our repo: a real ~4K-line TypeScript
  exact-replay executor (`packages/tassadar-executor`) plus a Rust ALM compiler
  in `psionic` (phases E1–E5 landed). Missing: MILP optimal scheduling (E4),
  served compiled artifacts (E6), the softmax path, corpus diversity, and
  construction pricing.
- **Artanis is now newly capable**: as of commit `96427f8a22` he has a
  tool-calling loop with `read_repo_file` and a plan-only `dispatch_codex_task`
  that emits exact `pylon khala request` commands for the Khala→Pylon→Codex
  burndown loop. That is the lever to fan this work out.

---

## 1. Where the Tassadar training run is NOW

### 1.1 The live run

Run ID: **`run.tassadar.executor.20260615`**

Live state, verified 2026-06-27 against
`GET https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615`:

| Field | Value |
|---|---|
| `run.state` | `active` |
| `manifest.objective` | "Grow the Tassadar verified-trace corpus via paid executor-trace work, verified by exact replay." |
| `manifest.verifierPolicy` | `exact_trace_replay` |
| `manifest.workloadFamily` | `executor-trace` |
| `manifest.paymentMode` | `operator_approved_small_sats` |
| `manifest.spendCapSats` | `100000` |
| `corpus.acceptedTraceCount` | `12` |
| `realGradient.deviceRequirement.observedDistinctContributorDevices` | `5` (required `2`) |
| `settlement.reconciledState` | `settling` |
| `settlement.settledPayoutSats` | `1020` |
| `settlement.settledReceiptCount` | `5` |

Note the manifest carries `settlementState: "live"` as a *static* owner
launch-gate field seeded once in migration `0185`. The live truth is
`summary.settlement.reconciledState` / `settledPayoutSats`. The endpoint itself
ships a `manifestSettlementStateNote` warning not to read the static field as
live status — a good honesty signal Artanis should respect.

Public surfaces for the run:
- `GET /api/public/training/runs/run.tassadar.executor.20260615` — summary
- `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements` —
  enumerable per-run settled rows (real `realBitcoinMoved: true` rows, sim rows
  excluded from the real total)
- WebSocket `public-settled-feed:tassadar` — live settled-event broadcast
  (commit `e5481f54e1`, 2026-06-17)
- `GET /api/public/artanis/admin-ticks` — Artanis admin-tick monitor (dispatches
  the fixed workload at most 4×/day)

### 1.2 What is running / blocked / done

**Running (live loop):**
- Worker executes the one compiled program → submits a trace digest commitment →
  an independent validator replays the pinned workload → digests are compared
  byte-for-byte → on a `Verified` verdict, both legs auto-stream settle (5+5
  sats per verified replay).
- Artanis autonomous dispatch tick dispatches the one fixed workload at most 4×
  per day (`artanis-administrator-tick.ts`, dispatching `tassadarPocLoopSumFixture`).
- Self-serve claim producer keeps auto-starter windows openly claimable
  (commit `db51f3d370`, 2026-06-18).

**Done:**
- First independent pairing settled real Bitcoin (see §1.3).
- 5 settled receipts, 1020 settled sats, 12 accepted traces, 5 distinct
  contributor devices observed against a required 2 — i.e., the independent-device
  bar is met.
- `compute.tassadar_executor_poc.v1` registry promise went green 2026-06-10.

**Blocked / not yet done (the real gap, see §1.4):**
- No new capability is ever constructed — the run re-verifies **one** program
  (`loop_sum_v1`: sum to 15 over 100 steps) forever.
- No corpus diversity, no dense composable weight modules in the live loop, no
  pricing for *constructing* or *composing* a module (only for re-executing the
  fixed one).

Key code surfaces (in `apps/openagents.com/workers/api/src/`):
- `tassadar-replay-validator.ts` — `runTassadarReplayValidation`
- `training-verification.ts` — `verifyExactTraceReplay`
- `tassadar-auto-settlement.ts` — `autoSettleVerifiedPair`
- `tassadar-settled-feed-sync.ts` — public settled feed broadcast
- Executor itself: `packages/tassadar-executor/src/numeric-executor.ts`
  (`executeTassadarNumericModel`)

### 1.3 Launch gate / first-pairing / reward state (reconfirmed)

The prior belief — "first independent Orrery-worker↔Whitefang-validator pairing
went green ~2026-06-16, settled" — is **confirmed correct, with nuance**:

- Commit `4d3aad5428` (2026-06-16): "independent validator Verified; gate now
  blocked only on independent worker".
- Registry promise flipped green 2026-06-16 (commit `2341dd06df`) for **bounded
  scope only**: one real 1,000-sat Bitcoin canary settlement to an independent
  contributor.
- Promise renamed `training.monday_decentralized_training_launch.v1` →
  `training.decentralized_training_launch.v1` (commit `bef33c98ce`).
- Settlement receipt with `realBitcoinMoved: true`, `state: settled`, documented
  in `docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md`.
- Worker leg + validator-leg payout resolution fixed in commit `24cb8f30a2`
  (2026-06-18). First auto-stream sequence captured in
  `docs/launch/2026-06-19-autostream-settlement-visibility-capture.md`:
  `trace_submitted → verification_verified → real_bitcoin_moved →
  settlement_recorded`.

Reward / recognition state (public-safe): the three first-pairing participants
(worker, validator, dispatch operator) are owed recognition rewards; some payouts
were held because recipient nodes were offline. The per-window rate in the live
loop is 5 sats worker + 5 sats validator per Verified replay. Treat exact
recognition amounts and revshare as **owner-gated** — do not invent numbers.

**Honest caveat carried in `docs/tassadar/RESEARCH_PLAN.md` (§3.4, §4):** the
auto-stream capture carries an `operator_approval.tassadar.autostream.worker`
source-ref, so the *mechanism* is proven, but a fully hands-off external pair
with zero operator involvement is still flagged as a future first-to-land.

### 1.4 The actual-training gap (real vs. simulated/stubbed)

Per `docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`:

The gap is **not** "real Bitcoin vs. fake Bitcoin" — both the settlements and the
traces are real. The gap is paradigmatic:

**What is real:**
- The compiled program is a genuine artifact from the owned psionic ALM pipeline
  (`TassadarProgram → tassadar_alm_wasm_interpreter → compile_tassadar_alm_graph
  → materialize_tassadar_alm_numeric → digest-pinned JSON`), not hand-coded.
- The exact-replay verification (re-execute pinned workload, compare digests).
- Real Bitcoin settlement on the Spark treasury rail.

**What is missing (construction, not gradients):**
1. One program forever — only `loop_sum_v1`, no corpus variety.
2. No dense loadable weight-modules in the live loop (sparse scalar-lane
   coefficients, not W_Q/W_K/W_V/FFN checkpoint blocks).
3. No live composition/linking (psionic `tassadar_module_linker.rs` +
   `tassadar_cross_profile_link_compatibility.rs` exist, but no live linked unit).
4. No pricing of construction — settlement pays for *re-executing and matching*
   one workload, never for *constructing a new module* or *composing modules*.
5. Corpus diversity gates on psionic W1 substrate (wider Wasm window 12→35
   opcodes, MILP scheduler E4, dense materialization, softmax error bounds).

A read-only receipt surface deliberately reports all "real training" gates as
false: `GET /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`
(`docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md`).
This is intentional honesty, not a bug.

### 1.5 The other lane: actual-pretraining (do not conflate)

There is a **separate** real pretraining lane that is NOT the Tassadar executor
run. The current top operator command is `./TRAIN` (workspace root), which
fetches `psionic` `origin/main`, materializes a clean worktree, and runs the
psionic `./TRAIN` dispatcher. Default lane is **`actual_pretraining`**
(`reference_pilot` is explicitly demoted to an escape hatch:
`./TRAIN --lane reference_pilot`).

- Model: `psion-compact-decoder-internal-v1`; data `psion_corpus_tokenized@v1`.
- Last clean canary run (per `psionic/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md`):
  run `psion-actual-pretraining-tri-host-actual-prodcanary-metal-mainseed-20260413t183100Z`,
  tri-host (2 Macs Metal + 1 RTX 4080 CUDA), 12 optimizer steps, 3,992 cumulative
  train tokens, ~94 tok/s mean, recovery/resume drill passed bit-exact.
- Per `docs/training/2026-06-20-training-full-pipeline-program-status.md`: the
  full-pipeline promise stays **planned**, blocker
  `blocker.product_promises.training_pipeline_rails_incomplete`; 12 issues
  (#4673–#4684) define the rebuilding rails. The training **stack** is ahead of
  the training **network** — the dispatch/verification/payment/projection rails
  are still being rebuilt in public, mostly red.
- SFT lane `psion_instruct_sft_v1` exists at fixture scale (8 steps, 93 trainable
  tokens, resume bit-exact) but paid SFT dispatch and preference rollout are open
  (`docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md`).

**Implication for Artanis:** "Tassadar run" = the live executor-trace
verification-by-replay economy. "Psion pretraining" = the bounded Rust gradient
lane whose network rails are still red. Keep them distinct.

---

## 2. Percepta implementation status

Percepta is the external thesis: compile (Wasm-subset) programs into transformer
weights and execute them **exactly**, with every intermediate step appearing in
the trace ("the trace is the receipt"), verifiable by replay. OpenAgents is
executing a **bounded, honest subset** of that thesis.

### 2.1 What exists (built and tested)

**Layer 1 — Design (complete):** `docs/tassadar/` carries the full design set:
`2026-06-10-percepta-constructing-llm-computer-notes.md`,
`2026-06-10-tassadar-percepta-audit.md` (history + posture, incl. the March 2026
failed *learned*-exact ladder),
`2026-06-10-psionic-alm-compiler-design-speculation.md` (the port target),
`2026-06-20-tassadar-percepta-executor-model-spec.md` (three-lane split),
`2026-06-20-tassadar-percepta-architecture-receipt.md` (public route binding).

**Layer 2 — TypeScript executor (real code):**
`packages/tassadar-executor/src/` — ~4K lines of implementation plus ~2.3K lines
of tests across the files: `numeric-executor.ts` (exact-integer ALM execution,
hard-max parabolic attention lookup, cumsum, ReGLU), `replay.ts` (full-trace +
window replay verdicts with typed rejections), `linked-dense-module-runtime.ts`
(851 lines — composed specialized executors), `dense-weight-module-runtime.ts`,
`capability-envelope.ts` (W4.1 self-test receipts → capability matrix),
`kernel-optimization-dispatch.ts` / `kernel-optimization-parity.ts` (market
work-requests + parity/speedup settlement claims), `compiled-program-corpus.ts`,
plus `*.test.ts` covering fixture parity, byte-for-byte digest matching, forged-pin
rejection, and capability-matrix schema. **Claim boundary:** faithful
re-execution of digest-pinned compiled bundles only — no softmax, no learning,
no serving claim, no perf-vs-CPU claim.

**Layer 3 — psionic Rust compiler (sibling repo, referenced):** ALM compiler
campaign phases **E1–E5 landed** (psionic #1098–#1114): `TassadarAlmGraph` IR +
exact evaluator (E1), backend list-scheduler + interval coloring (E2), universal
stack ISA with specializable channel (E3), Futamura specializer at IR level (E5),
plus geometric attention, Li Chao hull fast path, differential check harness, and
numeric materialization. **Open:** E4 (MILP optimal scheduling) and E6 (served
compiled artifacts).

### 2.2 What's missing / stubbed (gap to the thesis)

| Capability | Percepta thesis | Our honest posture |
|---|---|---|
| Arbitrary program execution | Wasm interpreter in weights | bounded workloads only — `arbitrary_c_or_wasm_not_claimed` |
| Soft-exact execution | softmax carry-over proven | hard-max only; soft path pending |
| Training-free weights | analytic, zero training | yes for the compiled-exact lane |
| Served to users | n/a (research) | served only under disclosure gates (`served_publication_allowed=false`) |
| Optimal scheduling | MILP | E4 open |
| Served artifacts / marketplace | n/a | E6 open; no live artifact wrapper |

The deeper missing piece is the **market structure**: verification-by-replay
should price *constructing* and *composing* modules and let weak devices (Apple
Silicon, consumer desktops) earn by executing compiled programs at full speed.
Today the loop only pays for re-executing one fixed program.

### 2.3 References available to port from (read-only)

- `projects/repos/transformer-vm/` — Percepta's own compiler (Python+PyTorch):
  five-primitive IR (`graph/core.py`), 35-opcode Wasm subset
  (`wasm/interpreter.py`), MILP scheduler (`scheduler/milp.py`), analytical
  weight emitter (`model/weights.py`), O(log n) hull cache
  (`attention/hull2d_cht.h`), C++ inference engine, end-to-end C toolchain.
  Examples: Hungarian matcher, Sudoku, Collatz, Fibonacci. **Highest-leverage
  port target: the IR + MILP scheduler + analytical weight emitter pipeline.**
- `projects/repos/llm-as-computer/` — independent validation with a 55-opcode
  stack-machine ISA (Mojo backend 67–126M steps/s). **Use as a cross-conformance
  bar:** run the same example programs through our compiler; any trace divergence
  is a precise bug report on one side or the other.

---

## 3. What can be delegated to Artanis

Artanis gained real agency in commits `fbe8307e78` (capabilities audit),
`96427f8a22` ("Give Artanis real agency: tool-calling loop + repo-read + Codex
dispatch plan"), and `b2cba05c2c` (Gemini function-calling so the tool loop fires
on the Khala lane). His tools (defined in
`apps/openagents.com/workers/api/src/artanis-operator-tools.ts`):

- **`read_repo_file(path)`** (#6365): reads UTF-8 files from the **public**
  `OpenAgentsInc/openagents` repo (branch `main`) via raw.githubusercontent.com.
  Path-safety enforced (`isSafeArtanisRepoPath`); secret/wallet/auth paths denied;
  256 KB cap; returns honest "(file not found)" on miss. Runs free (no gate).
- **`dispatch_codex_task(objective, issue, filePaths, verify, branch)`** (#6366):
  **plan-only**. Returns the exact public-safe `pylon khala request` command that
  *would* run the Khala→Pylon→Codex burndown loop, pending owner approval
  (`pylon_job_dispatch` gate in `artanis-approval-gates`). All fields pass
  `DISPATCH_UNSAFE_PATTERN` (rejects `@`, local paths, tokens, mnemonics, payout,
  etc.). Output is the bounded `pylon khala request --workflow codex_agent_task
  --repo OpenAgentsInc/openagents --branch <b> --commit <sha> [--verify ...]`
  string plus `pylon assignment run-no-spend` guidance and parallel-concurrency
  notes (`OPENAGENTS_PYLON_CODEX_CONCURRENCY`).

The end-to-end flow (the Khala→Pylon→Codex runbook) is documented in
`CLAUDE.md` ("Khala -> Pylon -> Codex Coding Delegation Runbook") and
`apps/pylon/docs/khala-burndown-runbook.md`.

Concretely, Artanis can autonomously, within bounded read+plan authority:

1. **Read the live state himself** — pull this audit, `docs/tassadar/RESEARCH_PLAN.md`,
   and the gap audit via `read_repo_file`; poll the public run endpoint and the
   `cpu-transform-training-receipts` surface to know exactly which gates are red.
2. **Fan out the corpus-diversity work** — plan `dispatch_codex_task` jobs that
   add new compiled-program fixtures to `packages/tassadar-executor` and the
   replay validator, each verified by `bun run --cwd apps/openagents.com/workers/api
   test -- <file>`.
3. **Run cross-conformance benchmarks** — dispatch a task that runs the
   `llm-as-computer` / `transformer-vm` example programs through our executor and
   reports trace divergences as issues.
4. **Direct research** — use the inference-engineering book and `docs/tassadar/`
   as the recurring "what's next" source (this is named explicitly in epic #6359),
   and open issues for each new research thread.
5. **Keep the burndown loop full** — the #6355 parallel loop is already CLOSED as
   a runner; Artanis's job under #6359 is to *drive* it: select work, plan
   dispatch, verify closeouts, merge non-spend code, refill.

Money and destructive actions stay owner-gated. Spend, deploy, wallet, and
settlement actions escalate through `artanis-approval-gates`; everything read/plan
runs free.

---

## 4. New work + research to fan out (prioritized)

Each item is shaped so Artanis can dispatch it. Open issues are referenced where
they already exist; new ones are marked **[file]**.

**P0 — make the live run construct something new (close the §1.4 gap):**
1. **[file] Second compiled program in the live loop.** Add a distinct workload
   beyond `loop_sum_v1` to the Tassadar dispatch fixture + replay validator, with
   its own digest pin and tests. Verify: `tassadar-executor` + replay-validator
   test suites. This is the single highest-signal move: it proves the corpus can
   grow, not just repeat.
2. **[file] Price construction, not just re-execution.** Extend
   `tassadar-auto-settlement.ts` so a *newly constructed/verified* module earns a
   distinct (owner-gated) settlement event vs. a re-execution. Keep amounts
   owner-gated; ship the mechanism + tests first.
3. **[file] Live linked-module unit.** Wire one composed/linked dense module
   (`linked-dense-module-runtime.ts`) into a dispatchable workload so composition
   is demonstrated end-to-end, not just unit-tested.

**P1 — Percepta substrate (psionic, study-then-port):**
4. **[file] Port the MILP scheduler (E4)** from `transformer-vm/scheduler/milp.py`
   into psionic, with formal liveness/slot-reuse validation (named open in the
   research plan).
5. **[file] Cross-conformance harness.** Run `llm-as-computer` and `transformer-vm`
   example programs through our executor; file each trace divergence as a bug.
6. **[file] Widen the Wasm window** (12→35 opcodes) toward the reference ISA;
   gate corpus diversity on it.

**P2 — pretraining network rails (separate lane, already tracked):**
7. Advance the open training-pipeline rails (#4673–#4684): paid dispatch,
   verification classes, multi-rung economics — these unblock
   `blocker.product_promises.training_pipeline_rails_incomplete`.
8. Paid SFT dispatch + preference rollout for `psion_instruct_sft_v1`
   (`docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md`).

**P3 — Artanis autonomy plumbing (epic #6359, OPEN):**
9. #6365 (`read_repo_file`, OPEN) and #6366 (`dispatch_codex_task`, OPEN) —
   finish/verify the tool wiring so the autonomous loop is real, not plan-only,
   within the documented authority boundary.

When filing, Artanis should set each issue's body to a public-safe objective + the
exact verify command, so it maps one-to-one to a `dispatch_codex_task` plan.

---

## 5. Risks / unknowns / owner-gated items

- **Owner-gated money.** Real settlement amounts, recognition rewards, and
  revshare for the first-pairing participants are owner decisions. Do not invent
  or auto-spend. The live spend cap is 100,000 sats on this run; settlement is
  `operator_approved_small_sats`.
- **Held payouts.** Some first-pairing recognition payouts are held because
  recipient nodes were offline — a node-availability/operational risk, not a
  protocol failure.
- **Hands-off pairing not yet proven.** The auto-stream capture still carries an
  `operator_approval.tassadar.autostream.worker` source-ref. A fully zero-operator
  external pair has not yet landed (research plan §3.4).
- **Don't over-claim Percepta.** The repo deliberately enforces
  `arbitrary_c_or_wasm_not_claimed`, `served_publication_allowed=false`, and a
  receipt surface that reports real-training gates as false. Artanis must preserve
  these honesty boundaries in any public copy or dispatch.
- **Two lanes, easy to conflate.** The Tassadar executor run (live, verification-
  by-replay) and the psion actual-pretraining lane (bounded gradient training,
  red network rails) are different. Status claims must name which.
- **psionic is a sibling repo and read-only here.** Compiler phases E1–E5 are
  cited from psionic docs/commits; E4/E6 work lands in `psionic`, not this repo.
  Artanis's `read_repo_file` only sees the public `openagents` repo, so
  psionic-side state must be confirmed through committed receipts/docs surfaced
  into `openagents`.
- **`alpha/tassadar/` is private.** It holds deeper strategy and market-impact
  research; this audit extracts only public-safe status. Do not copy it verbatim
  into public surfaces.

---

## Appendix — key paths, commits, endpoints

Docs:
- `docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`
- `docs/tassadar/RESEARCH_PLAN.md`
- `docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md`
- `docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md`
- `docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md`
- `docs/tassadar/2026-06-10-percepta-constructing-llm-computer-notes.md`
- `docs/tassadar/2026-06-10-tassadar-percepta-audit.md`
- `docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md`
- `docs/launch/2026-06-19-autostream-settlement-visibility-capture.md`
- `docs/training/2026-06-20-training-full-pipeline-program-status.md`
- `docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md`
- `psionic/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md` (sibling, read-only)
- `psionic/docs/TRAIN_SYSTEM.md` (sibling, read-only)

Code:
- `packages/tassadar-executor/src/` (TS executor + tests)
- `apps/openagents.com/workers/api/src/tassadar-replay-validator.ts`
- `apps/openagents.com/workers/api/src/training-verification.ts`
- `apps/openagents.com/workers/api/src/tassadar-auto-settlement.ts`
- `apps/openagents.com/workers/api/src/tassadar-settled-feed-sync.ts`
- `apps/openagents.com/workers/api/src/artanis-operator-tools.ts`

Commits: `4d3aad5428` (independent validator Verified, 2026-06-16),
`2341dd06df` (registry green, bounded), `bef33c98ce` (promise rename),
`24cb8f30a2` (validator-leg payout fix), `e5481f54e1` (settled feed),
`db51f3d370` (self-serve windows), `96427f8a22` (Artanis tool loop),
`b2cba05c2c` (Gemini function-calling).

Issues: #6359 (EPIC, OPEN), #6365 (OPEN), #6366 (OPEN), #6355 (CLOSED),
#6357 (CLOSED), #4673–#4684 (training-pipeline rails).

Endpoints (verified live 2026-06-27):
- `GET /api/public/training/runs/run.tassadar.executor.20260615`
- `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements`
- `GET /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`
- `GET /api/public/models/tassadar-percepta-executor/architecture-receipts`
- `GET /api/public/artanis/admin-ticks`
