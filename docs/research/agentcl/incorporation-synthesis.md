# AgentCL Incorporation Synthesis

How much of the AgentCL evaluation framework OpenAgents should adopt, where it
maps onto our systems, and a sequenced, issue-sized roadmap.

- Source paper: `AgentCL: Toward Rigorous Evaluation of Continual Learning in
  Language Agents`, Shu et al. (Ohio State, Johns Hopkins, Intuit AI Research),
  `arXiv:2606.02461v2`, 2 Jun 2026.
- Companion notes in this directory: `README.md`, `source.md`,
  `paper-summary.md`, `openagents-implications.md`.
- This doc was written after reading the full primary-source PDF (26 pages,
  SHA-256 `ae411b3e…56eb`), not only our notes.

## 1. What AgentCL is (plain terms)

AgentCL is an **evaluation framework**, not a memory architecture. Its thesis:
you cannot make a credible claim that an agent "learns from prior work" unless
your benchmark (a) controls how tasks in the stream relate to each other and
(b) reports plasticity, stability, and generalization as **separate** numbers.

Two construction ideas:

- **Controlled task streams.** A *naive stream* draws tasks from the same broad
  domain with no guaranteed reuse relationship. A *compositional stream*
  deliberately places earlier "source" tasks (sub-solutions, evidence,
  workflows) before later "complex" tasks that can profitably reuse them. The
  compositional relationship is a *controlled source→target relation*, not just
  "same topic."

- **Two-pass protocol + held-out set.** For each task `i` the harness measures:
  - `Bi` memoryless baseline,
  - `Fi` first pass (memory read+write allowed),
  - `Si` second pass over the same task with memory frozen (read-only),
  - `Hj` held-out task after memory is frozen.

  From these it defines three gains:
  - **Plasticity Gain** `PG = Fi - Bi` — did earlier experience help later
    in-stream tasks?
  - **Stability Gain** `SG = Si - Fi` — did the benefit *persist* after later
    tasks kept writing to memory?
  - **Generalization Gain** `GG = Hj - Bj` — does the built memory help on
    unseen tasks from a different source?

Key empirical findings (the part that should change our behavior):

- Naive streams have **weak discriminative power** — methods look similar.
  Compositional streams spread methods far apart (e.g. CodeEval-Pro complex-task
  accuracy std-dev 9.4/8.8 compositional vs 3.0/1.9 naive; BrowseComp+ 14.9/16.0
  vs 2.3/5.7). If you only run naive/long-history evals, you cannot tell good
  memory from bad memory.
- **Plasticity is real but stability/generalization are not solved.** Top
  methods (ExpRAG/ReMem/MemProbe) hit large PG (+17.7/+13.5/+21.9 on coding;
  MemProbe +40 on deep research) yet flat-or-negative SG (+0.0/-2.0/-2.1) and
  held-out GG *below* the memoryless ReAct baseline. Memoryless ReAct stays
  strongest on held-out coding (72.5). Memory that helps in-stream can *degrade*
  unrelated tasks.
- **MemProbe** (the paper's diagnostic method) stores three typed views per
  experience — `interaction` (trajectory + final response), `insight`
  (distilled pattern / failure mode), `skill` (procedure or reusable snippet) —
  retrieves top-k semantically, solves with that as *reference context*, then
  consolidates with a syntactic check + LLM judge. Ablations show all three
  views matter on compositional streams; an oracle judge barely beats the real
  judge (memory construction is not the bottleneck — *what to abstract/ignore*
  is).
- **Retrieval hit-rate is not the metric.** A case study shows a topically
  similar but semantically wrong memory actively hurting. Memory must be
  reference, not authority, and the solver must be able to reject it.

Scope boundary the paper draws: it studies **non-parametric memory** only
(retrieval/notes/skills), explicitly *not* parameter-update or RL continual
learning — though it says using AgentCL to diagnose training-based approaches is
interesting future work.

## 2. Why it's relevant to OpenAgents

We are accumulating exactly the surfaces where "the agent got better from prior
work" is becoming a product claim, and we have almost no apparatus to separate
plasticity from stability from harmlessness. AgentCL is a ready-made vocabulary
and protocol for that.

Component-by-component:

- **Khala (collective-intelligence orchestrator)** — `clients/khala-cli/`,
  `apps/openagents.com/workers/api/src/inference/`,
  `docs/khala/`. Khala already exposes an OpenAI-compatible endpoint and has a
  benchmark-harness book (`docs/khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`)
  and a telemetry schema (`openagents.khala.telemetry.v1`). This is the natural
  home for the two-pass protocol: run a candidate memory/coordinator config as
  pass-1 (read+write), then pass-2 (frozen), then a held-out stream.

- **The gym / benchmark ladder** — `docs/gym/`,
  `apps/openagents.com/workers/api/src/inference/gym/`, the decision-grade
  honesty gate **#6309**, and the MirrorCode integration epic **#6376**. The gym
  already has typed run/report schemas (`openagents.gym.run_progress.v1`,
  `openagents.gym.terminal_bench_comparison_report.v1`) and a cost-per-accepted-
  outcome honesty gate. AgentCL is the missing *axis*: the gym today scores
  one-shot capability; it does not score *continual* reuse. Adding a
  compositional-stream environment + PG/SG/GG to the ladder is a direct fit and
  reinforces #6309's "don't claim more than you measured" stance.

- **MirrorCode (#6377–#6379, under epic #6376)** — `apps/openagents.com/scripts/
  mirrorcode/`. MirrorCode points an external eval framework at Khala over the
  OpenAI-compatible API with hard "no training/RAG on tasks, public tasks only"
  constraints. That constraint is essentially AgentCL's *held-out* discipline.
  AgentCL gives MirrorCode the language to say *why* the no-RAG rule matters
  (held-out GG) and a structured place to report it.

- **Tassadar training run + trace homework** — `packages/tassadar-executor/`,
  `apps/openagents.com/workers/api/src/tassadar-executor-trace-homework.ts`,
  `tassadar-trace-factory/`, `docs/training/`. This is the one area the paper
  *explicitly excludes* (parameter-update CL). That is itself useful: it tells
  us the AgentCL gains are the wrong metric for Tassadar weight training, but
  the *stream construction* idea (source tasks before complex tasks; held-out
  rows never used to build the corpus) directly improves how we build and
  partition the trace corpus (`tassadar-trace-corpus.v0_*.manifest.json`). Today
  the corpus risks the naive-stream pathology: lots of adjacent traces, no
  controlled source→target relation, no isolated held-out split.

- **Gym/benchmark ladder vs. agent memory** — Pylon's TAS memory
  (`apps/pylon/src/tas/session-memory.ts`, `team-memory.ts`, `repo-memory.ts`)
  and the omni retrieval/selection hooks
  (`omni-retrieval-trace-context.ts`, `omni-market-memory-hooks.ts`) are exactly
  the "non-parametric memory" the paper evaluates. We currently have these
  systems with *no* PG/SG/GG measurement. Right now we cannot answer "does
  repo-memory actually help, persist, and stay harmless?" — only "does it
  retrieve something."

- **Agent traces + reputation economy** — `trace-store-d1.ts`,
  `atif-trace-schema.ts`, migrations `0228`–`0239`,
  `tassadar-trace-factory/trace-record.ts`. AgentCL's MemProbe three-view split
  (`interaction`/`insight`/`skill`) and its "was this memory used / ignored /
  contradicted / harmful" provenance map cleanly onto trace records and could
  become a reputation signal: a trace's economic value should rise when it is
  *reused and helps* (measurable PG contribution) and fall when it is retrieved
  but rejected or harmful. This is a principled basis for trace pricing beyond
  raw token counts.

- **Pylon fleet / per-account fleet** — `apps/pylon/`,
  `docs/ops/2026-06-27-artanis-as-a-service-multi-tenant-codex-fleet-enablement.md`.
  The fleet is the execution substrate that would *run* a compositional stream
  cheaply (parallel devices), and the per-account fleet raises a real AgentCL
  question: is memory per-account, per-repo, or shared? Cross-account memory
  bleed is precisely the "harmful on unrelated tasks" failure naive streams are
  designed to detect.

- **Artanis autonomous loop (#6359) + signal→backlog→fleet** —
  `artanis-mind.ts`, `artanis-scheduled-runner.ts`, `docs/artanis/`. Artanis is
  itself a continual learner: it ingests signal, updates a backlog, dispatches
  the fleet, and is supposed to improve. AgentCL gives Artanis a self-check: a
  periodic held-out task family that Artanis's accumulated context is *never*
  allowed to train on, so we can detect when its loop is drifting (negative SG)
  rather than improving. This makes the loop honest rather than self-reinforcing.

- **Blueprint/DSPy + GEPA prompt optimization** — `packages/blueprint-contracts/`,
  `apps/qa-runner/src/failure-learning-gepa.ts`, `distiller.ts`,
  `apps/pylon/src/gepa-capability.ts`. GEPA-style optimization is "learn better
  prompts/skills from failures" — a form of non-parametric continual learning.
  AgentCL's two-pass split is the right acceptance test for GEPA-distilled
  skills: a distilled skill should produce positive PG on a compositional stream
  *and* non-negative SG/GG before it is promoted into the live skill set. This
  turns GEPA's `skill-candidate` promotion into a measured gate.

## 3. To what extent to incorporate

### Adopt now (cheap, high leverage, shippable)

- **The vocabulary and claim discipline.** Stop reporting a single "memory
  improved accuracy" number. Whenever we describe Khala/Pylon memory, trace
  reuse, GEPA skills, or Artanis self-improvement, separate plasticity (helped
  later in-stream) from stability (persisted) from generalization (held-out).
  Wire this into `docs/promises/` language so a "continually learns" promise
  requires PG/SG/GG evidence, mirroring the #6309 honesty gate. Near-zero code.
- **A typed `agentcl_eval.v0` contract** for memory experiments (baseline,
  first pass, frozen second pass, held-out pass, three gains) living next to the
  existing gym schemas. This is the spine everything else hangs off.
- **Held-out discipline for the Tassadar trace corpus and MirrorCode.** Add an
  isolated held-out split to the trace-corpus manifest that is never used to
  build memory/training context, referenced by checksum only. MirrorCode already
  enforces the spirit of this; formalize it as the GG set.

### Experiment (worth a bounded spike, not yet load-bearing)

- **One compositional-stream gym environment** over our own repo/docs: small
  source tasks (lint fix, fixture test, single-helper change) that later complex
  issues genuinely reuse, plus a naive stream and a held-out stream. Run our
  *existing* memory systems (Pylon TAS memory, omni retrieval) through the
  two-pass harness to get our first real PG/SG/GG numbers. Expect, per the
  paper, that we will find positive PG but weak SG/GG — which is the point.
- **MemProbe-style three-view memory record** (`interaction`/`insight`/`skill`)
  with consolidation status and `reuse_mode: reference_only`, prototyped on top
  of the existing trace schema. Evaluate it against current TAS memory on the
  compositional environment.
- **GEPA skill-promotion gate**: require a distilled skill candidate to clear
  PG>0 and SG/GG≥0 on the compositional environment before promotion.

### Watch (track upstream; do not build yet)

- **The AgentCL HuggingFace dataset** (`osunlp/AgentCL`) and the related
  benchmarks it builds on (CodeEval-Pro / BigCodeBench-Lite-Pro, BrowseComp+,
  Continual Learning Bench, Evo-Memory, SWE-Bench-CL). If/when we want an
  external, citable continual-learning number for Khala, run the public splits
  through MirrorCode rather than reimplementing the harness. No vendoring.
- **Applying AgentCL's diagnostic lens to training-based CL** (the paper's own
  "future work"). This is exactly the Tassadar weight-training question. Watch
  whether the authors or follow-ups publish a parametric variant before we
  invest.

### Skip / does not fit (be honest)

- **Adopting AgentCL's exact task domains as our product benchmark.** Their
  streams (MMLU-Pro, BabyAI, ScienceWorld) are research instruments, not our
  market work. We borrow the *protocol*, not the datasets, for internal product
  claims.
- **Treating PG/SG/GG as the metric for Tassadar parameter training.** The
  paper deliberately excludes parametric CL; its gains assume frozen-weights
  non-parametric memory. Using them to grade weight updates would be a category
  error. Use them only for our non-parametric memory/skill/trace layers.
- **Building a full memory architecture off this paper.** AgentCL offers no
  solved memory design — only a measurement methodology and the finding that
  stability/generalization are open. Anyone proposing "adopt MemProbe as our
  memory system" is over-reading it; MemProbe is a *diagnostic probe*.

## 4. Concrete roadmap items

Each item is issue-sized, with owning system, priority, and dependencies.
Priority: **NOW** (this cycle), **SOON** (next), **LATER** (after deps land).
Owning repo is `openagents` unless noted.

1. **`agentcl_eval.v0` contract.** Add a typed schema for a memory experiment:
   `{ baseline, first_pass, frozen_second_pass, held_out_pass, plasticity_gain,
   stability_gain, generalization_gain, stream_kind: naive|compositional }`.
   Place alongside `openagents.gym.*` schemas in the gym/inference area.
   - System: gym schemas. **NOW.** Dep: none. Unblocks #2–#6.

2. **Promise-language gate for "continual learning" claims.** Extend the
   `docs/promises/` honesty rules so any claim that an agent "learns from prior
   work / gets better over time" must cite PG/SG/GG from an `agentcl_eval.v0`
   run. Fold into the #6309 decision-grade honesty gate.
   - System: docs/promises + gym honesty gate (#6309). **NOW.** Dep: #1.

3. **Held-out split for the Tassadar trace corpus.** Add an isolated held-out
   partition to `tassadar-trace-corpus.v0_*.manifest.json` that the trace-
   homework loop (`tassadar-executor-trace-homework.ts`) and trace-factory
   replay are forbidden to use for memory/context construction; reference by
   checksum only.
   - System: Tassadar trace factory / training. **NOW.** Dep: none (independent
     of #1; format-only).

4. **Formalize MirrorCode's no-RAG rule as the GG set.** Document in
   `apps/openagents.com/scripts/mirrorcode/README.md` that MirrorCode public
   tasks are the held-out generalization set for Khala memory, and emit the GG
   field in the MirrorCode result JSON.
   - System: MirrorCode (#6377–#6379, epic #6376). **SOON.** Dep: #1; coordinate
     with in-flight MirrorCode phases.

5. **Compositional-stream gym environment (spike).** Build one small
   `agentcl-repo-reuse` environment: ~10–20 source tasks (single-helper /
   fixture / lint changes) that later complex tasks genuinely compose, plus a
   matched naive stream and a held-out stream from a different package. Register
   it in the gym environment registry next to `khala-code` / `terminal-bench`.
   - System: gym (`inference/gym/`), runs on Pylon fleet via harbor-dispatch.
     **SOON.** Dep: #1. Related: #6309, #6376.

6. **Two-pass harness runner.** Add a runner that executes any registered
   memory config over a stream as pass-1 (read+write), pass-2 (frozen), and a
   held-out pass, emitting `agentcl_eval.v0`. Reuse the gym harbor-dispatch and
   trace-archive plumbing (`harbor-dispatch.ts`,
   `harbor-full-trace-archive-routes.ts`).
   - System: gym + Pylon fleet. **SOON.** Dep: #1, #5.

7. **First measurement pass on existing memory.** Run Pylon TAS memory
   (`session/team/repo-memory.ts`) and omni retrieval
   (`omni-retrieval-trace-context.ts`) through #6 on #5. Publish the first
   honest PG/SG/GG for OpenAgents memory.
   - System: Pylon memory + gym. **SOON.** Dep: #5, #6.

8. **MemProbe-style three-view trace record + provenance.** Extend the trace
   schema (`atif-trace-schema.ts` / `trace-record.ts`) with `memory_kind`
   (`interaction|insight|skill`), `consolidation_status`
   (`valid|uncertain|invalid`), `relationship_ref`, `reuse_mode:
   reference_only`, and a post-hoc `reuse_outcome` (`used|ignored|contradicted|
   harmful`). Select via the existing typed semantic selector, deterministic
   parsing only for bounded id/enum fields (workspace invariant).
   - System: trace store + omni hooks. **LATER.** Dep: #6 (need measurement to
     justify), #7.

9. **Reputation signal from reuse outcome.** Feed `reuse_outcome` and per-trace
   PG contribution into the multi-earning ledger
   (`pylon-multi-earning-node.ts` / `apps/pylon/src/multi-earning-ledger.ts`) so
   a trace's value reflects measured, harmless reuse rather than token volume.
   - System: reputation economy. **LATER.** Dep: #8. Owner decision required
     (changes payout basis).

10. **GEPA skill-promotion gate.** Gate `skill-candidate` promotion in
    `failure-learning-gepa.ts` / `distiller.ts` on PG>0 and SG/GG≥0 from #6.
    - System: qa-runner GEPA + Pylon `gepa-capability.ts`. **LATER.** Dep: #5,
      #6.

11. **Artanis held-out drift check.** Give the Artanis loop (#6359) a periodic
    held-out task family its accumulated context never trains on, surfaced as a
    GG metric on the admin-ticks surface, to detect self-reinforcing drift.
    - System: Artanis (`artanis-scheduled-runner.ts`). **LATER.** Dep: #6;
      coordinate with #6359.

12. **Per-account memory isolation test.** Use the naive/held-out streams from
    #5 across two accounts in the per-account fleet to verify one account's
    memory does not degrade another's tasks (cross-account harmlessness).
    - System: Pylon per-account fleet. **LATER.** Dep: #5, #6; coordinate with
      the multi-tenant codex-fleet enablement doc.

## 5. Risks, open questions, owner decisions

- **Construction cost / honesty.** A *good* compositional stream is expensive to
  build — the source→target relation must be real, not "same topic," or we
  reproduce the naive-stream weakness we are trying to avoid. The paper's own
  BrowseComp+ subtasks were synthesized with privileged access to parent
  answers; we must build ours without leaking answers into the source tasks.
  Risk: a sloppy stream produces flattering-but-meaningless PG. Mitigation: keep
  #5 small and hand-audited first.
- **We will probably look bad first.** Per the paper, expect positive PG but
  weak/negative SG and below-baseline GG from our current memory. That is the
  honest result and the reason to do this — but it must not be spun. Owner
  decision: are we willing to publish a first PG/SG/GG that shows memoryless
  baselines beating our memory on held-out work? (Recommended: yes, internally,
  gated like #6309.)
- **Reputation/payout basis change (#9).** Tying trace value to measured reuse
  outcome changes the economic incentive surface and could be gamed (farming
  "reused" traces). Owner decision required before #9; keep it experiment-only
  until the measurement (#7) is trusted.
- **Scope creep into a memory architecture.** The paper does not give us one.
  We must resist "implement MemProbe as our memory" framing; #8 is a *record
  shape + provenance* change for measurement, not a new memory engine.
- **Tassadar boundary.** Confirm with the training owners that PG/SG/GG are used
  only for the non-parametric trace/memory layer, not as a weight-training
  metric. #3 (held-out corpus split) is safe and valuable regardless.
- **No vendoring.** The AgentCL dataset and reference methods stay external; if
  we want a citable external number we run public splits through MirrorCode.
  This respects the workspace no-vendor rule.

## 6. At-a-glance table

| AgentCL idea | OpenAgents target | Verdict | Suggested issue |
| --- | --- | --- | --- |
| PG/SG/GG vocabulary + claim discipline | `docs/promises/`, #6309 honesty gate | adopt now | #2 |
| Typed memory-experiment contract | gym/inference schemas | adopt now | #1 |
| Held-out set never used to build memory | Tassadar trace corpus; MirrorCode | adopt now | #3, #4 |
| Two-pass (baseline/F/frozen-S/held-out) protocol | Khala + gym runner | experiment | #6 |
| Compositional vs naive stream construction | new gym environment | experiment | #5 |
| First real measurement of our memory | Pylon TAS + omni retrieval | experiment | #7 |
| MemProbe 3-view record (interaction/insight/skill) + provenance | trace store / ATIF schema | experiment | #8 |
| Retrieval-as-reference, log used/ignored/harmful | omni hooks, trace record | experiment | #8 |
| Skill promotion gated on PG/SG/GG | GEPA (`failure-learning-gepa.ts`) | experiment | #10 |
| Held-out drift check for a self-improving loop | Artanis loop #6359 | experiment | #11 |
| Cross-account memory harmlessness | per-account Pylon fleet | experiment | #12 |
| Reuse-outcome → trace value | reputation / earning ledger | watch (owner decision) | #9 |
| AgentCL dataset + reference methods | external via MirrorCode | watch | (#4) |
| AgentCL gains for parametric/weight CL | Tassadar weight training | skip (category error) | — |
| Adopt AgentCL's research task domains as product benchmark | product claims | skip | — |
| Adopt MemProbe as our memory architecture | memory engine | skip (over-read) | — |

## Bottom line

AgentCL is a measurement discipline, not a feature. **Adopt the discipline now**
(vocabulary, typed `agentcl_eval.v0` contract, held-out splits) because it is
cheap and directly strengthens the #6309 honesty posture. **Experiment** with
one compositional gym environment and a two-pass runner to get our first honest
PG/SG/GG on existing memory. **Watch** the dataset and the parametric-CL
direction. **Skip** using these gains to grade Tassadar weight training or
treating MemProbe as a memory architecture.
