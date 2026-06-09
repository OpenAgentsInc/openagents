# Probe Benchmark Docs

Date: 2026-06-08

This folder tracks how Probe should participate in public benchmark execution,
continual improvement, and optimizer-driven promotion. The docs here are about
architecture and execution plans. They are not public benchmark claims.

## Current Reading Order

1. `plan.md`
   Execution source of truth for converting the benchmark architecture into
   GitHub issues. It defines the cross-repo ownership model, the 19-issue
   series, labels, milestones, public claim rules, final tracking deliverable,
   and the priority order for opening work.

2. `2026-06-08-workspace-benchmark-systems-audit.md`
   Inventory of the benchmark systems across Probe, private Cloud source
   material, public benchmark-cloud target architecture, Psionic,
   Pylon, OpenAgents product surface, and historical repos. Start here when deciding which repo owns
   which part of the benchmark apparatus.

3. `2026-06-08-probe-gepa-benchmark-system-closeout-audit.md`
   Closeout audit for the implemented `plan.md` issue series. It summarizes
   how Probe, public benchmark-cloud, Psionic, OpenAgents product surface/Pylon, and Artanis now fit
   together, what is done, what remains live-gated, what was tested, and the
   next milestone for real SHC/Harbor and Pylon-distributed runs.

4. `2026-06-08-omni-continual-learning-training-loop.md`
   Big-picture bridge from the root-level Omni docs and OpenAgents transcript
   arc into Probe's benchmark plans. It explains why continual benchmark
   learning serves Coding on Autopilot, accepted outcomes, route scorecards,
   public proof, Pylon provider work, and Psionic model training.

5. `canaries/20260608151057/`
   First tracked live Probe GEPA Terminal-Bench 2 canary evidence bundle. It
   records an unpaid OpenAgents product surface Pylon assignment lifecycle through offer, accept,
   progress, artifact/proof submission, operator accepted-work closeout, a
   schema-backed Probe closeout bundle, and a Psionic import request. It is
   initial retained evidence only, not a public benchmark score, paid work, or
   production promotion.

6. `2026-06-08-artanis-gepa-benchmark-pylon-focus.md`
   Artanis refocus audit. It compares the current Artanis mission, public
   Forum readback, posting runbook, and proof trail against the Probe
   benchmark docs, then defines how Artanis should become the public overseer
   for Probe GEPA coding-agent benchmark campaigns through Pylons while
   leaving execution, scoring, payment, training, and promotion authority in
   the owning systems.

7. `2026-06-08-probe-continual-benchmark-learning-apparatus.md`
   End-state plan for Probe's continual improvement loop. It defines how Probe,
   public benchmark-cloud, Psionic, Pylon, and OpenAgents product surface should turn
   benchmark failures into prompt, Blueprint, tool-menu, loop-policy, and LoRA
   candidates with explicit promotion gates.

8. `2026-06-08-pylon-gepa-coding-agent-benchmark-run.md`
   First executable optimizer plan. It narrows the initial benchmark-climbing
   work to a GEPA-only text-candidate campaign, using Pylon as the parallel
   rollout engine across retained Terminal-Bench failures, validation splits,
   and frozen holdout tasks.

9. `../probe-benchmark-contracts.md`
   Implementation note for the first Probe runtime contract slice from issue
   #182. It records the benchmark assignment, run, closeout, decision trace,
   candidate, evidence-only promotion decision schemas, and normalized
   closeout bundle writer now exported by the Bun/Effect runtime package.

10. `../probe-retained-terminal-bench-fixtures.md`
   Implementation note for the retained Terminal-Bench fixture package from
   issue #184. It records the public-ref-only failure-family fixtures that
   Probe and GEPA Stage 0/1 can load without hidden task material or private
   Harbor traces.

## Current Decisions

- `plan.md` is now the issue-creation and execution-order source of truth.
  The older audits explain rationale and boundaries; they do not supersede
  the ordered cross-repo issue series in `plan.md`.
- The benchmark apparatus should be public OpenAgents infrastructure.
- The private `cloud` repo is source material and backfill, not the desired
  long-term benchmark authority.
- Public Benchmark Cloud should be rebuilt or moved into `openagents`, with
  public docs, contracts, scripts, fixtures, and eventually stable protocol
  surfaces where needed.
- Probe should be the coding-agent runtime and evidence emitter. It should not
  become the benchmark product, scorer, public-claim authority, or promotion
  authority.
- The benchmark loop exists to improve accepted coding outcomes in Coding on
  Autopilot. Benchmark wins are useful only when they create better workrooms,
  route scorecards, guidance decisions, artifacts, and review outcomes.
- Terminal-Bench 2 through Harbor on the SHC box is the first live coding
  benchmark lane.
- The first optimizer run should be GEPA-only over text artifacts: Probe
  prompts, Blueprint usage, Program Signature playbooks, tool-menu policy,
  failure-family playbooks, and closeout instructions.
- LoRA, DPO, GRPO, and Qwen3.6 adapter work should come after the GEPA lane
  creates clean traces, candidate diffs, verifier outcomes, and split-aware
  evidence.
- Pylon should provide distributed rollout capacity for GEPA first, with
  explicit worker capability envelopes and signed receipts. Later Pylon may
  provide distributed training capacity for Psionic/Qwen/LoRA work, but GEPA
  itself is distributed benchmark-driven optimization over text artifacts, not
  distributed neural-network training.
- The 2026-06-08 OpenAgents product surface #502 smoke proved the generic live Pylon assignment
  lease path. The tracked canary in `canaries/20260608151057/` is the first
  Probe GEPA Terminal-Bench 2 unpaid smoke evidence using that lifecycle, with
  public-safe assignment, progress, artifact, proof, closeout, route scorecard,
  and Psionic import refs.
- OpenAgents product surface should remain the release gate and projection surface for public and
  private benchmark evidence.
- Artanis should be the public-safe overseer and campaign narrator for Probe
  benchmark learning, not the runtime, scorer, optimizer, payment authority,
  or promotion authority.
- The existing Artanis Forum can carry public campaign summaries. Public
  Forum reads use `/api/forum` and exact topic/post APIs. Posting as Artanis
  follows the OpenAgents product surface local operator runbook
  `openagents/docs/forum/2026-06-07-artanis-forum-posting-runbook.md`
  and requires the dedicated Artanis/operator credential path. Probe may
  prepare public-safe copy or reply under its own registered-agent identity,
  but it must not post as Artanis or invoke the Artanis bridge.
- The Pylon release freeze remains active for broad download, earning, payout,
  settlement, and release-promotion claims until the remaining OpenAgents product surface/Nexus
  gates close.

## Issue Series Source Of Truth

`plan.md` defines the executable work as 19 GitHub issues across the owning
repos. Do not put every issue in Probe.

Ownership:

- `probe`: runtime, benchmark assignment intake, candidate execution, closeout
  evidence, local fixtures, selected signatures, and tool menus.
- `openagents`: public Benchmark Cloud contracts, split manifests,
  artifact/proof contracts, Terminal-Bench runner lane, and Pylon benchmark
  package surfaces where applicable.
- `openagents`: Artanis projection, OpenAgents product surface/Pylon assignment lease
  adaptation, release gates, and public claim boundaries.
- `psionic`: GEPA coordinator, candidate frontier, candidate manifests,
  reflection/proposal jobs, and later LoRA/Qwen training path.
- `pylon` or `openagents`: worker capability envelopes, assignment receipt
  schema, benchmark-capable worker admission, and artifact/proof submission.

Open issues in the order specified by `plan.md`:

1. Probe closeout foundation.
2. Public Benchmark Cloud contracts.
3. GEPA candidate optimization.
4. Pylon work slices and paid-work path.
5. Stage 0 and Stage 1 campaign execution.
6. Artanis and public projection.
7. Route scorecards and product impact.

The priority unlock sequence is:

1. Probe closeout bundle.
2. Public Benchmark Cloud split manifest.
3. GEPA candidate manifest.
4. Pylon metric-call assignment type.
5. Stage 0 smoke.
6. Stage 1 retained-failure sprint.

The required tracking deliverable after issue creation is a grouped summary of
issue links by epic, owner repo, dependencies, Pylon work-slice issues, paid
work prerequisites, direct Probe-performance issues, and claim-boundary or
projection-only issues.

## Public Claim Boundaries

Do not call retained fixture improvements public benchmark scores. Do not call
validation-split GEPA improvements frozen holdout performance. Do not publish
"Probe beats Terminal-Bench" from retained, validation, local smoke, or
optimizer-accepted evidence.

Every claim should name:

- dataset and version;
- split;
- task selector;
- agent slug;
- model/backend;
- Probe commit;
- candidate hash;
- retry and timeout policy;
- verifier or scorer result;
- cost and duration;
- artifact availability;
- redaction state;
- whether the evidence is retained, validation, frozen holdout, or live public
  claim evidence.

## Maintenance Rule

When adding a benchmark doc to this folder, update this README with its purpose,
status, and reading-order position. If the new doc changes ownership,
promotion gates, public claim boundaries, or the immediate implementation
sequence, update the older docs instead of leaving contradictory plans in the
folder. If an execution sequence conflicts with `plan.md`, treat `plan.md` as
the source of truth and update the older doc.
