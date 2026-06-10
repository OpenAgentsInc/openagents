# Artanis In Production: The Tick Model And The Tassadar Evolution Loop

Date: 2026-06-10

Registry version at audit time: `2026-06-10.12`. Companion to the
autonomous-loop contract (`2026-06-06-autonomous-loop-contract.md`), the
executor-loop candidate note
(`2026-06-10-executor-trace-loop-candidate.md`), the v0.3 readiness
audit (`docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md`
§5), and the Tassadar essay (`docs/tassadar/README.md`).

This audit answers two owner questions: **how does Artanis actually run
in production** (what ticks, who ticks it, what the spine is), and
**how does Artanis learn from what comes in and build up the Tassadar
model over time**. Decisions, open questions, and a proposed product
promise are at the end.

## 1. What ticks today — the honest baseline

The production worker already has a cron trigger (`* * * * *`,
deployed). Its `scheduled` handler runs
`runArtanisScheduledTickScheduled(db, config.artanis.scheduledRunnerEnabled, …)`
alongside billing sweeps and notification dispatch. The Artanis leg is
gated by the `ARTANIS_SCHEDULED_RUNNER_ENABLED` flag and, when enabled,
executes a **status-projection tick**: load allowed context refs
(public pylon stats, model-lab contracts, persisted state, operator
steering, runner backend), persist a loop record and tick record under
the loop contract's rules (idempotency keys, one active loop per
scope), save a runtime snapshot, queue one Forum publication intent,
and record a health snapshot. It proposes no real work; its work
proposal and approval gate are example-shaped placeholders, and the
loop contract independently denies every risky action kind.

So the honest sentence is: **the heartbeat exists and is deployed; the
hands do not act yet.** That is the right starting posture — it means
"making Artanis genuinely run" is a matter of wiring real actions into
an already-tested spine, not of inventing a scheduler.

## 2. The production tick model: who supplies the motive force

Three candidate answers to "what makes Artanis go," and the
recommendation is **all three, in layers**, because each maps to a
different part of the existing type system.

### 2.1 Layer one: Cloudflare cron as the spine (exists)

The worker cron is the only component that must never sleep. But a
Workers isolate cannot *compute* — CPU and wall-clock limits make it a
bookkeeper, not a body. The right production shape for the cron tick is
therefore pure orchestration, every step already representable in the
shipped vocabulary:

1. claim the scoped loop (idempotent; duplicate ticks suppressed);
2. read what changed since the last tick: new closeouts, verification
   verdicts, settlement events, health/staleness signals;
3. advance state machines mechanically: accept closeouts whose digest
   predicate passed, create replay challenges for unverified closeouts,
   mark stale assignments, record blockers;
4. **dispatch new work as assignments** — never execute it;
5. queue copy-gated Forum publication intents and the public status
   projection;
6. close the tick with receipts and schedule the next pass.

Everything in steps 1–6 is sub-second D1 and HTTP work — exactly what a
worker is for. The minute cadence is almost certainly too hot for real
operation (every tick writes ledger rows); a production loop wants the
cron as a *clock* with the runner deciding internally whether a tick is
due (e.g. act every N minutes, project status hourly), which the
schedule-suffix idempotency design already supports.

### 2.2 Layer two: the Pylon fleet as the body (proven)

All actual computation — executor workloads, replay validation, dataset
generation, training steps, benchmark reruns — runs as **assignments to
Pylons**, dispatched by the cron tick through the same
`/api/operator/pylons/assignments` machinery the green PoC used. This
is the deep answer to "provide compute to advance Artanis": advancing
Artanis *is* doing Pylon work, because every heavy thing Artanis wants
done is a dispatchable, receipt-bearing, verifiable assignment. The
fleet is Artanis's body; contributors who run Pylons are quite
literally supplying Artanis's motive force, and the payment-mode ladder
prices it. Weak devices validate (replay verdicts); strong devices
execute and train. Nothing new is required here — the PoC proved the
full round trip including settlement.

### 2.3 Layer three: agents as the hands (formalize what already happens)

The PoC was one registered agent performing Artanis ticks by hand, and
the sibling agent advancing the v0.3 cluster is doing the same in
parallel. The honest production model embraces this: **tick actions
that need judgment, incident response, or approval-satisfying authority
are claimable by registered agents**, through the same assignment/claim
rails — Artanis as a job board for its own operation. Concretely:

- the cron tick records blockers it cannot clear (a gate needs
  approval, a refusal needs diagnosis, copy needs review) as **open
  tick actions** with idempotency keys;
- registered agents (Fable-class operators today; outside contributors
  eventually) claim them exactly like assignments, do the work, and
  post receipts that the next cron tick consumes;
- risk classes are unchanged: a contributed tick can *satisfy* an
  approval requirement only with the authority its agent actually
  holds; `wallet_spend` still terminates at the owner.

This answers "enable other people to contribute Artanis ticks" without
a new trust model: contributing a tick is doing a claimable, bounded,
receipt-bearing piece of operational work, identical in shape to every
other work class in the market. It also makes Artanis honest about its
own labor: today's hand-driven operation becomes visible, priced, and
auditable instead of ambient.

### 2.4 The decision

**Cron = clock and bookkeeper. Fleet = computation. Agents = judgment
and gated authority. Owner = spend.** No single-runner architecture; no
new infrastructure. The alternative spines considered and deferred:
Durable Objects (stronger serialization than the loop ledger needs —
D1 + idempotency keys already give one-active-loop semantics), Queues
(useful later for fan-out of per-assignment follow-ups; not needed for
the spine), and an external scheduler (rejected — it would put the
heartbeat outside the audited surface).

## 3. The Tassadar evolution loop: how Artanis builds the model

The owner's directive: Artanis should learn from what comes in and
evolve the Tassadar model over time. The remarkable fact is that the
pieces compose into a closed flywheel where the exact executor plays
**three roles at once — teacher, grader, and curriculum generator** —
and every stage maps onto an existing surface.

### Stage 0 — accumulate verified traces (running now, by hand)

Dispatch executor workloads; replay-verify; keep the receipts. Each
verified closeout is a digest-pinned, exactly-correct execution trace.
The PoC produced the first ones. Cron-tick automation makes this a
standing corpus that grows with fleet idle time.

### Stage 1 — generate and curate the curriculum

Two sources, both mechanical:

- **Committed workloads**: the psionic fixture families (stack ISA, the
  branch-capable window programs, the symbolic examples).
- **Generated workloads**: psionic's bounded differential harness
  already *generates* unlimited small ALM graphs with known-correct
  traces (it found two real compiler bugs that way). The same seeded
  generator, dispatched as assignments, mints arbitrarily much fresh
  curriculum — programs nobody wrote, with oracle traces nobody had to
  check. Artanis's `dataset_curation` template kind is the binding
  surface; psionic's `tassadar_compiled_distillation` lane
  (psionic-data) is the conversion machinery from verified traces to
  training datasets.

### Stage 2 — train candidates

The `lora_finetuning_training` template kind routes curated datasets
into training runs — through psionic's training lanes and the same
homework rails the CS336 program built (#4675–#4678): tokenizer steps,
real-gradient steps, distributed windows, paid closeouts. The training
objective at this stage is bounded and honest: **train models toward
executor behavior on the committed workload families** — the
direction psionic's Tassadar lane has always pointed (its universality
program and distillation surfaces predate this audit). This is the
compiled→trained spectrum from the essay made operational: the
compiled executor supplies infinite clean supervision; training chases
it.

### Stage 3 — evaluate by exact replay (the special part)

A trained candidate's output trace is graded by the **same
`exact_trace_replay` verdict machinery that grades Pylon work**: run
the candidate on a workload, diff its trace against the oracle's,
verdict Verified/Rejected with the exact first divergent step. No
graders, no rubric, no judge model — and the rejection signal is
uniquely rich (the precise step where the model's computation departs
from correct). `benchmark_eval_rerun` and `regression_analysis`
templates own the cadence; weak-device validators can run the grading.

### Stage 4 — promote and widen

Candidates that clear eval bars promote through psionic's capability
envelope with eval evidence (never past the disclosure gates); the
curriculum widens along the alignment plan already written
(straight-line → branchy window → `core_i32_v3` widenings), and each
widening becomes a new curriculum stage whose oracle is, again, the
exact executor. Every stage's receipts land in the tick ledger, so
**the loop's own history is the context the next tick loads** — which
is the precise, bounded sense in which Artanis "learns": dispatch
policy, workload mix, pylon selection, and spend pacing evolve from
accumulated receipts (mechanical/parameterized policy first;
LLM-judgment policy later, behind approval gates, as its own decision).

### Why this loop is credible rather than aspirational

Every arrow in it has already carried real traffic at least once:
dispatch→execute→closeout (PoC milestone 1), closeout→verdict
(milestone 2), verdict→settlement (milestone 3), traces→datasets
(psionic's distillation lane exists), datasets→training (the CS336
rails and the real-gradient A1 lane exist), training→eval-by-oracle
(psionic's eval surfaces + the replay class). What does not exist is
the *automation connecting the arrows* — which is exactly what the
tick model in §2 is for.

## 4. Decisions (made or proposed here)

1. **Spine = worker cron**, with the runner deciding tick cadence
   internally; no Durable Object or external scheduler for the spine.
2. **The cron tick never computes**; all computation is dispatched as
   assignments. The tick is bookkeeping + dispatch + state advance.
3. **Agent-contributed ticks become first-class**: blockers the cron
   cannot clear are exposed as claimable tick actions on the existing
   assignment rails, with unchanged risk classes.
4. **Tassadar evolution runs as the template chain**
   `dataset_curation → lora_finetuning_training → benchmark_eval_rerun
   → regression_analysis`, with the exact executor as teacher, grader,
   and curriculum generator, and the bounded harness's generator as the
   curriculum mint.
5. **Policy learning starts mechanical** (thresholds, pacing, mix from
   receipts); any LLM-driven steering is a separate, approval-gated
   decision later.
6. **A scoped product promise** (below) makes the automated run
   publicly monitorable, consistent with the registry discipline.

## 5. Unanswered questions

- **Cadence and cost**: what tick interval balances D1 ledger growth,
  cron cost, and responsiveness? (Proposal: act-tick every 15 minutes,
  status-tick hourly; needs measurement.) Related: tick-ledger
  retention/compaction policy for D1.
- **Funding the paid samples**: the loop wants a steady trickle of paid
  closeouts to keep settlement honest. Who sets the standing cap, and
  is the owner's spend-enable per-epoch or per-amount?
- **Agent identity for contributed ticks**: does a contributed tick run
  under the contributing agent's authority (current posture) or under a
  delegated Artanis sub-identity with narrower scope? Registration
  ownership (the PoC's `pylon_api_forbidden` residual) is the same
  question from the other side.
- **The payout adapter**: hosted-MDK programmatic payouts are disabled
  upstream and the agent-wallet adapter is unregistered in the payment
  authority. Which gets fixed first? (The loop's paid stage depends on
  one of them; the local bridge pattern does not automate.)
- **Candidate model architecture**: what do Stage-2 candidates actually
  train — a small from-scratch transformer (Psion-adjacent), a LoRA on
  an existing open model, or both as separate eval rows? Out of this
  audit's scope; belongs to psionic.
- **Monitor surface shape**: public page rendering the tick ledger
  (loops, ticks, receipts, blockers) — does it extend the existing
  public-report projection or stand alone? The authority-split doc
  suggests extending.
- **Queue adoption point**: at what fan-out (assignments per tick) do
  Workers Queues earn their place for follow-up scheduling?

## 6. Proposed product promise

`artanis.tassadar_evolution_loop.v1` (Pylon/Artanis product area,
yellow at creation):

- **claim**: a standing automated Artanis run advances the Tassadar
  executor lane in production — dispatching digest-pinned executor
  work to Pylons, verifying it by exact replay, accumulating the
  verified-trace corpus toward Tassadar model training, and publishing
  monitorable per-tick receipts on a public surface.
- **blockers at creation**: scheduled-runner enablement with real
  (non-placeholder) tick actions; a sustained autonomous streak
  (proposal: ≥10 consecutive unattended ticks with receipts) including
  executor dispatch and replay verdicts; a public monitor surface for
  the tick ledger; the first curated distillation-dataset receipt from
  verified traces.
- **unsafeCopy**: no claim that a trained model executes exactly, no
  claim of ungated autonomy (spend and publication stay gated), no
  general LLM-computer or earning claims; the Tassadar disclosure
  boundaries extend unchanged.
- **verification**: the tick ledger itself — receipts per tick, with
  the monitor page as the public check.

This is the "automated run people can monitor and push" the owner
asked for: monitorable because every tick emits receipts to a public
projection, and pushable because contributing compute (run a Pylon) or
contributing ticks (claim an open action) are both standing,
first-class ways for anyone to advance it.

## Source refs

- `apps/openagents.com/workers/api/src/artanis-scheduled-runner.ts`
  (status tick), `index.ts` `scheduled` handler + `wrangler.jsonc` cron,
  `config.ts` `ARTANIS_SCHEDULED_RUNNER_ENABLED`
- `docs/artanis/2026-06-06-autonomous-loop-contract.md`,
  `2026-06-06-continual-learning-job-templates.md`,
  `2026-06-06-operator-approval-gates.md`,
  `2026-06-08-artanis-public-report-authority-split.md`,
  `2026-06-08-artanis-gepa-network-launch-status-audit.md`
- `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md` §5,
  `docs/tassadar/README.md`
- psionic: `tassadar_compiled_distillation` (psionic-data),
  `tassadar_alm_bounded_check` (the curriculum generator),
  `tassadar_alm_trace_replay` (the grader),
  `cs336_a1_real_gradient_reference` (training rails)
- Issues: #4687 (PoC epic), #4696 (v0.3 inclusion), #4697 (executor
  loop), #4676 (validator lane), #4675–#4678 (training rails)
