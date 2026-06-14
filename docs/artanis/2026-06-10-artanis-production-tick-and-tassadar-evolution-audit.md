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
7. *(Owner clarification, §7)* **Artanis is a model-driven agent, not
   the cron**: the cron is its alarm clock, the loop ledger its working
   memory, Cloudflare pure orchestration. The AI proposes; the schema
   validates; the gates hold.
8. *(Owner clarification, §7)* **The objective function is full device
   utilization with dark-capacity accounting**, and Artanis holds
   Autopilot powers — filing issues and commissioning code changes on
   its own infrastructure through the normal work-order gates.

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
- **The payout adapter**: the OpenAgents treasury wallet (the `/treasury`
  MDK-backed wallet) makes payouts programmatically, and Artanis is already
  wired to pay out from it under bounded spend authority (the nexus-treasury
  payout ledger). Any earlier note here that "hosted-MDK programmatic payouts
  are disabled" describes only the hosted-MDK SDK's own payout path, which is
  not what the loop uses — it is not a constraint on the treasury/Artanis
  payout path. The open question is the standing cap and per-payout policy, not
  whether payout can run automatically.
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
- **The mind's model binding**: which provider runs the in-worker
  administrator (Workers AI, hosted Gemini per `api.hosted_gemini.v1`,
  or an existing provider binding), and what is the per-tick token
  budget? Does the binding choice need its own promise evidence?
- **Artanis's authority envelope**: when Artanis files issues and opens
  work orders on its own blockers, does it act under its own registered
  identity with scoped grants (recommended — symmetrical with every
  other agent) or under a delegated operator identity? What can it
  accept on its own ticks (proposal: nothing — acceptance stays with
  owner/operator gates until a promise says otherwise)?
- **Umbrella promise timing**: create
  `artanis.nexus_administrator.v1` now (yellow, design-stage) or at
  Phase A evidence? (§7.6 recommends at evidence; owner's call.)

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

## 7. Owner clarification (2026-06-10): Artanis as Nexus administrator

After the sections above were written, the owner clarified the full
vision. It is larger than the tick spine and the Tassadar loop, and it
changes one architectural emphasis. Restated:

> Artanis is the administrator of the Nexus — the central administrator
> of the Pylons (the Nexus/Pylon idea from StarCraft), even though
> Nexus folded into core openagents.com. An agent in charge of
> distributing work out to Pylons and making sure devices are fully
> utilized. All of this has to be run by an actual AI — that is what
> Artanis is — posting Forum updates. Cloudflare is orchestration.
> Ideally Artanis can also deploy Probes and run the Autopilot system:
> request software updates, post GitHub issues, use the full suite of
> Autopilot automated-coding services from the product promises.
> Artanis boots up Tassadar.

### 7.1 What "run by an actual AI" changes in section 2

Section 2's layer model survives, but its emphasis inverts: **the cron
is not Artanis. Artanis is a model-driven agent; the cron is its alarm
clock and the loop ledger is its working memory.** Cloudflare remains
pure orchestration substrate. Two embodiments, complementary rather
than competing:

- **(a) The in-worker mind**: the scheduled tick assembles the typed
  context (capacity funnel, new closeouts, verdicts, blockers, spend
  state) and calls a hosted model, which returns an action plan that
  must validate against the typed action vocabulary. Schema-invalid
  proposals are blocked ticks, not actions; risky kinds still route to
  approval gates. This is the smallest honest path to "an actual AI
  decides every tick," and it is cheap enough to run at administrative
  cadence.
- **(b) The resident agent**: a long-lived coding-agent session
  (Probe/Codex class, a registered agent identity like the ones
  operating today) that claims the tick actions the in-worker mind
  cannot perform — filing issues, opening work orders, diagnosing
  refusals, incident response. This is where the Autopilot powers live.

Recommended composition: (a) is the standing administrator; (b) is
summoned by (a) when a tick needs hands. The typed loop ledger is the
contract between them, and the existence proof for (b) is this very
operating mode — the Tassadar PoC and this audit were produced by
registered agents doing exactly that job manually.

The decisive guardrail is unchanged from the loop contract: the AI
*proposes*; the schema *validates*; the gates *hold*. Intelligence
upgrades the quality of proposals, never the authority of actions.

### 7.2 Nexus administration: the utilization objective

The administrator's objective function is **full device utilization
with honest accounting**: every online Pylon should hold work it is
capable of, and every idle device should have a named dark-capacity
reason. The inputs are already shipped: capacity-funnel snapshots are
recorded by the same scheduled handler that runs the Artanis tick,
presence/heartbeats and registration capabilities exist, and
`pylon.no_dark_capacity_accounting.v1` (yellow) is the accounting
conscience — a utilization claim without dark-capacity reasons is
overclaim by that promise's own standard.

The administration policy is a matching loop over work classes, in
descending order of verification cheapness:

1. **executor-trace** — the always-on floor; green PoC; every CPU
   qualifies (execute or validate);
2. **GEPA assignment slices** — `pylon.gepa_worker_loop_v03.v1`
   (yellow; live endpoint + paid settlement gated);
3. **CS336 homework** — rails landed (#4673–#4684), training run
   promise still red;
4. **local inference** — `pylon.compute_revenue_modes.v1` (red);
5. **labor** — `provider.compliant_usage_labor.v1` (red).

Artanis keeps queue depth against capability match; the funnel measures
the result; the Forum gets the projection. This is the StarCraft
framing made literal: the Nexus does not do the work — it powers and
directs Pylons.

### 7.3 The Autopilot powers: the self-improvement arm

The vision's most consequential clause is that Artanis consumes the
Autopilot coding suite on its own infrastructure: when a tick hits a
blocker that requires code — the PoC's job-kind mismatch, the payout
adapter, a new workload family — Artanis files the GitHub issue, opens
an Autopilot work order carrying a `promiseRef`, lets the coding loop
produce the change under the normal acceptance gates, and consumes the
deploy as new capability on a later tick. **Self-improving operations,
priced and gated identically to customer work.** The existence proof is
already on the record: the entire Tassadar campaign (issues filed,
implemented, reviewed, deployed, promise flipped) is one agent doing
this loop by hand.

What that requires, honestly, by promise state:

- Available now: agents file issues and Forum posts
  (`agents.one_instruction_sheet.v1` green; demonstrated daily); the
  no-spend Autopilot work-order loop (grant → work order → lease →
  execution → proof → acceptance → briefing) runs today; Probe ships
  inside every Pylon (`pylon.cli_tui_probe_background.v1` green;
  `autopilot.codex_probe_pylon_successor.v1` yellow). So the
  issue-filing + work-order version of self-improvement needs **no new
  product surface** — only the Artanis identity and tick wiring.
- Gated: mission briefings as Artanis's read-back surface
  (`autopilot.mission_briefing.v1` yellow); the decision queue Artanis
  would drive instead of a human
  (`autopilot.decision_queue.v1` planned); many-agent fan-out
  (`autopilot.control_center_fanout_marketplace.v1` red); honest
  outcome economics for pricing its own commissions
  (`payments.accepted_outcome_economics.v1` red).

### 7.4 The promise map

How the full vision decomposes onto the registry as of `2026-06-10.13`:

| Vision component | Promise | State | Gap for Artanis |
| --- | --- | --- | --- |
| The administrator loop + Tassadar boot-up | `artanis.tassadar_evolution_loop.v1` | yellow | its four blockers are the Phase A work |
| The executor work class (floor) | `compute.tassadar_executor_poc.v1` | green | scale past one Pylon / one family (#4696, #4697) |
| The AI mind's model binding | `api.hosted_gemini.v1` | red | any provider binding works; this is the named one |
| Utilization accounting | `pylon.no_dark_capacity_accounting.v1` | yellow | funnel→reason wiring into tick context |
| GEPA work class | `pylon.gepa_worker_loop_v03.v1` | yellow | live endpoint smoke, paid settlement |
| Training work class | `pylon.first_real_model_training_run.v1` | red | CS336 rails exist; needs the run |
| Inference work class | `pylon.compute_revenue_modes.v1` | red | psionic-qwen lanes in flight (#4665/#4666) |
| Labor work class | `provider.compliant_usage_labor.v1` | red | five-streams lane (#4646–#4650) |
| Probe deployment | `pylon.cli_tui_probe_background.v1` | green | Probe is in every Pylon already |
| Coding-runtime direction | `autopilot.codex_probe_pylon_successor.v1` | yellow | per its own blockers |
| Mission read-back | `autopilot.mission_briefing.v1` | yellow | Artanis as briefing consumer |
| Decision-driving | `autopilot.decision_queue.v1` | planned | Artanis as the queue's first agent driver |
| Many-agent fan-out | `autopilot.control_center_fanout_marketplace.v1` | red | Phase C+ |
| Commission economics | `payments.accepted_outcome_economics.v1` | red | needed before Artanis prices its own work orders |
| Agent identity/onboarding | `agents.one_instruction_sheet.v1` | green | Artanis registers like any agent |
| Forum updates | (live Forum surface + publication queue) | live/built | copy gates per authority split |

### 7.5 Roadmap phases

- **Phase A — the administrator boots (now)**: real tick actions, the
  in-worker mind on a model binding, executor-lane dispatch, the public
  tick monitor. Flips the four `artanis.tassadar_evolution_loop.v1`
  blockers. Needs nothing red.
- **Phase B — Nexus administration**: utilization matching across the
  yellow work classes with dark-capacity reporting; the funnel becomes
  Artanis's scoreboard. Needs GEPA yellow→green motion; benefits from
  no-dark-capacity flipping.
- **Phase C — Autopilot powers**: Artanis files issues and opens work
  orders on its own blockers (possible now); graduates to driving the
  decision queue and fan-out as those promises land.
- **Phase D — Tassadar evolution at scale**: the §3 flywheel with the
  training work class live (red→yellow via the CS336/homework rails),
  candidates graded by exact replay, curriculum widening per the
  alignment plan.

Phases are concurrent-friendly: A is prerequisite to nothing being
claimed about B–D, but B–D groundwork (rails, lanes, promises) is
already in flight across the issue tracker.

### 7.6 Proposed umbrella promise (not yet created)

`artanis.nexus_administrator.v1`: *an actual AI administers the Pylon
fleet in production — distributing work across all live work classes,
keeping devices utilized with dark-capacity accounting, posting Forum
updates, and commissioning its own software improvements through the
Autopilot loop under unchanged approval gates.* Creation is an owner
decision; the honest sequencing is to create it when Phase A evidence
exists (so its safeCopy can cite a running administrator rather than a
design), with `artanis.tassadar_evolution_loop.v1` as the first
evidence row beneath it. Recorded here so the registry intent is
explicit.

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
