# Training Program Status: Where We Are, And How The Promises Carry It

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-10 (registry `2026-06-10.9`)

Status: status essay. Every claim below is bounded to committed code,
retained runs, filed issues, or registry records. Nothing here upgrades any
promise. Companion documents: the buildout plan
(`2026-06-10-psion-full-pipeline-buildout-plan.md` in this folder), the
CS336 continuation audit
(`../2026-06-10-cs336-distributed-homework-continuation-audit.md`), and the
green-velocity extrapolation
(`../promises/2026-06-10-green-velocity-extrapolation.md`).

## The shape of the moment

In April 2026 this company ran a real distributed-training economy for a
few weeks: 1,300+ registered Pylons, about a million sats paid, CS336
assignment work dispatched, Merkle-committed results verified by Freivalds
challenges, payouts settled. The June 9 rebuild deleted the control plane
that ran it. The Psionic side — the actual ML stack — survived intact and
kept growing. So the honest one-sentence status is:

> **The training stack is ahead of the training network.** Psionic can
> tokenize, train, eval, checkpoint, and verify on owned Rust top to
> bottom at bounded scale; the monorepo's dispatch, verification, payment,
> and projection rails for that work are being rebuilt on the current
> Worker stack, in public, behind twelve filed issues and twenty-one
> registry promises that are mostly red and planned — on purpose.

Today the live network is dark: the public capacity funnel reports 37
registered Pylons, all 37 reason-coded dark, zero assignment-ready. The
registry says so out loud. That is the baseline every estimate in the
velocity doc starts from.

## What actually exists, by layer

**Psionic (execution truth — alive and compounding).**

- **CS336 ports.** A1 complete as a bounded reference lane (21/21 adapter
  rows) with hand-derived analytic backprop landed 2026-06-10
  (psionic#1114) — the old finite-difference honesty gap is closing. A2 at
  bounded reference coverage (explicitly not full parity). The A3 scaling
  core (IsoFLOP planner + Chinchilla fit), A4 data-refinery deterministic
  core (PII masking, Gopher rules, exact + MinHash dedup), and A5
  alignment math (GRPO/GSPO losses, DPO, packing, eval parsers) all landed
  2026-06-10 (psionic#1101–#1103).
- **The actual-pretraining lane (R0).** A retained tri-host run — two Macs
  on Metal plus one RTX 4080 on CUDA — through the full operational loop:
  preflight hardware qualification, windows, checkpoints with backup
  copies, planned-interruption recovery, contributor continuity receipts,
  continue/hold/restart eval decisions. 3,992 train tokens at 2.74
  effective tokens per second. Tiny, unflattering, and real — the template
  for every honesty rule above it.
- **Tassadar.** The executor-compiler campaign (psionic #1098–#1114)
  produced an integer-exact ALM pipeline with five agreeing execution
  legs, a differential harness that caught two real scheduler bugs on its
  first run, and the `exact_trace_replay` verifier — the reference
  implementation for the cheapest verification class the work economy
  will have.
- **The playbook.** The Smol Training Playbook is chaptered into
  `psionic/docs/smol/` and absorbed into the buildout plan: ablation
  discipline, data curriculum, WSD schedules, marathon operations,
  post-training arc, infrastructure measurement.

**Monorepo (dispatch, verification, receipts — under active rebuild).**

- Twelve issues (#4673–#4684) define the rails: run/window authority,
  pluggable verification classes, A1 work kind with paid closeouts,
  weak-device validator lane, public run pages, then the A3/A4/A2/A5
  lanes, leaderboards, and the Tassadar exact-replay work class. Agents
  have publicly claimed #4673, #4675, #4677, #4678 on the Forum with
  scoped approaches and explicit no-overclaim boundaries.
- Pieces already landed: the Tassadar executor-trace dispatch wiring with
  a passing no-spend smoke, the `a4_eval_delta` leaderboard serving an
  honest empty state with the right blocker ref, the live capacity funnel
  with its dark-capacity taxonomy, and promise-transition receipts
  (proposed state changes mechanically checked, with a public feed).
- The April-era verification contract (Merkle-committed matrices,
  Freivalds challenges, lease/retry/timeout queues) exists as a written
  reference at the pre-rebuild revision; #4674 ports it as pluggable
  classes rather than a monolith.

**The plan layer.** The buildout plan extends the CS336 curriculum to the
six workstreams a real training organization needs — ablation system,
data program, architecture derisking, marathon operations, post-training
arc, infrastructure truth — plus the R0→R4 model ladder with engineering
*and* economics gates, and the per-stage verification map.

## How the promises carry it

As of `2026-06-10.9` the training program is represented in the registry
as three tiers, and the tiers are the roadmap:

**Tier 1 — the old launch claims, still red, now with a named path.**
`pylon.first_real_model_training_run.v1` and
`pylon.compute_revenue_modes.v1` are the April-era claims rebuilt
honestly. Their verification text now names the ladder's R2 rung —
network pretraining windows on real contributor devices, commitment-backed
verification, paid closeouts, an economics gate against a rented-cluster
comparator — as the honest green path. They do not move until that
evidence exists.

**Tier 2 — the program promises, all planned, written before the work.**
Nine records entered at `2026-06-10.8`:

| Promise | Workstream it carries | First gate |
|---|---|---|
| `training.full_pipeline_program.v1` | the umbrella | every workstream ≥ yellow + one ladder rung end-to-end |
| `training.ablation_system.v1` | derisking machinery | gate zero: reproduce a published eval score through the owned harness |
| `training.data_refinery_corpus.v1` | corpus program | paid refinery shards with provenance digests |
| `training.model_ladder.v1` | R0→R4 sequencing | R1 full rehearsal + economics-gate format |
| `training.marathon_operations.v1` | long-run operations | durable checkpoint seal; curtailment drill |
| `training.post_training_arc.v1` | SFT/PO/RL | instruct lane + vibe-test artifact |
| `training.verification_classes.v1` | the trust layer | class registry live on real dispatched work |
| `training.device_capability_dataset.v1` | infrastructure truth | first paid benchmark assignments |
| `proof.demand_provenance.v1` | honesty about buyers | internal/external split on revenue surfaces |

**Tier 3 — the scoped exception.** `compute.tassadar_executor_poc.v1`
(yellow): one bounded executor-trace proof of concept on real Pylons —
dispatch, separate-device exact replay, one paid closeout. It needs no
training capability at all and runs the moment the verification rails and
two real devices exist; it is the floor workload the rest of the program
sits above.

The dependency chain underneath all three tiers is short and explicit:
**rails (#4673/#4674) → R1 rehearsal on operator devices → real devices +
operator spend approvals → R2 network run → tier-1 promises flip.**
Everything else in the program parallelizes around that spine. The
external Psionic asks (real-gradient scale-up beyond the single-head A1
trainer, A4 adapter conformance, A2 kernels) run in the psionic tracker
alongside.

## What is genuinely blocking, named plainly

1. **Real devices.** R2, the Tassadar PoC's separate-device replay, the
   bounded two-device Qwen run (#4670), and the Windows/WSL claims all
   need hardware that is not the operator's primary machine. The funnel
   says the contributor fleet is currently dark.
2. **Operator authority.** Paid closeouts, settlement smokes, and every
   registry flip are deliberately operator-gated. Agents propose with
   receipts; operators flip. Agent velocity cannot move these alone.
3. **Psionic scale-up.** The A1 real-gradient trainer is single-head tiny
   config; RoPE/multi-head backward, batching, and scale are open before
   R1 is a *real* (if small) pretraining run rather than a mechanics
   rehearsal.
4. **Nothing else.** The design work, the contracts, the verification
   classes, the plan, and the claim discipline all exist. The remaining
   distance is execution and evidence, which is the right kind of
   remaining distance.

## The posture

The program's bet, restated from the plan: the pipeline is simultaneously
a model factory and the network's standing internal demand engine, its
work classes are the cheapest-to-verify in the whole catalog, and the
receipts it generates — run pages, ablation ledgers, device datasets,
economics gates — are the market memory that makes the open network worth
more than a closed fleet. The registry is the instrument that keeps that
bet honest: twelve of twenty-one training-program records say *planned*,
two say *red*, and the rule that none of them moves without receipts is
itself a green promise (`promises.registry.v1`).

For when the pieces are modeled to flip, see the velocity extrapolation
in `docs/promises/2026-06-10-green-velocity-extrapolation.md` — with its
caveats attached, which is the only way numbers travel around here.
