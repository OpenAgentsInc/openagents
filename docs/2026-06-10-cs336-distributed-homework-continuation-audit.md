# CS336 Distributed Homework Continuation Audit

Date: 2026-06-10

Registry version at audit time: `2026-06-10.4`

Status: historical review of the April 2026 CS336 distributed-homework
system (Episodes 222–224 era) from git history — almost all of it was
deleted in the 2026-06-09 Bun rebuild (`f5919c766`) — plus what would be
needed to continue that work on the current stack. Companion to the
training epic audit
(`2026-06-10-pylon-training-compute-modes-promise-audit.md`, issues
#4664–#4671).

## What CS336 Is And Why We Used It

Stanford CS336 "Language Modeling from Scratch" is a full-stack curriculum:
BPE tokenizer → Transformer → Adam → training loop (A1), FlashAttention /
DDP / FSDP / sharded optimizers (A2), scaling laws (A3), pretraining data
(A4), alignment (A5). The workspace tracks the complete course repo set in
the `projects/cs336/` lane (assignments, lectures, leaderboards, notes).

Episode 224 ("Distributed Training 101", `docs/transcripts/224.md`) states
the product use directly: the Pylon network had just crossed 1,300+
registered pylons and ~1M sats paid for being online; that day the network
switched from paying-for-presence to paying-for-work, and the work was the
CS336 assignments — "we're going to be going through the assignments...
we're not using PyTorch, we're using Psionic. We've already gone ahead and
ported all of the Assignment 1 code into Psionic." Pylons received "little
pieces of homework" and got paid Bitcoin for completing them. CS336 was the
curriculum that turned the idle-compute network into a distributed-training
network, on the DiLoCo-class model (local work, periodic sync) discussed in
the same episode.

## What Was Built (April 2026, From Git History)

### Psionic side (still alive, in the psionic repo)

- CS336 A1 fully ported to Rust: tokenizer, transformer, optimizer, training
  loop, with a packaged bounded demo lane —
  `psion_cs336_a1_demo_v1` (work class `small_model_local_training`,
  request/output schemas
  `psion.cs336_a1_demo_automatic_execution_{request,outputs}.v1`, a fixed
  tiny corpus, four-step budget, one accepted checkpoint) wired into the
  zero-touch `psionic-train` manifest path specifically so Pylon and the
  control plane could dispatch it (`psionic/docs/PSION_CS336_A1_DEMO_LANE.md`,
  `PSION_CS336_A1_FULL_PORT_MATRIX.md`, `PSION_CS336_A1_REFERENCE_LANE.md`).
- CS336 A2 bounded coverage: FlashAttention, DDP, FSDP wrapper /
  after-backward / gather-full-params, and sharded-optimizer adapter rows
  mapped one-to-one against Stanford's `assignment2-systems/tests/adapters.py`
  with checked-in proof bundles (`PSION_CS336_A2_FULL_PORT_MATRIX.md`).
  Explicitly bounded reference evidence, not full transport-backed parity.
- A separate A1-derived minimal distributed LM lane
  (`PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE.md`).

### Control plane (deleted with the rebuild; lived in `apps/nexus-control`)

The Rust Nexus control plane carried the whole homework economy. Route
surface at rev `d1a6a9dc4` (`apps/nexus-control/src/lib.rs` ~line 7050+):

- `/api/training/nodes/admission`, `/api/training/heartbeats` — node intake.
- `/api/training/leases/claim` — pylons claimed homework leases; lease
  priority preferred admin-dispatched homework, then auto-launched hosted
  CS336 starter runs when the backlog was empty (tests:
  `default_pylon_lease_claim_prefers_admin_dispatched_homework_before_auto_starter`,
  `..._auto_launches_hosted_cs336_starter_work`).
- `/api/training/windows/plan` → `/{window_id}/activate` → `/{window_id}/seal`
  → `/{window_id}/reconcile` — the training-window lifecycle (the DiLoCo-ish
  unit of merged progress).
- `/api/training/validator-challenges/claim`, `/{challenge_id}/retry`,
  `/{challenge_id}/finalize` — the verification loop (next section).
- `/v1/admin/training/demo-runs/cs336-a1/launch` — the Episode 224 operator
  demo-launch path (exposed publicly as
  `POST https://openagents.com/admin/training/demo-runs/cs336-a1/launch`).
- An automatic CS336 homework dispatcher (`cbf617ca4`) later fanned out
  across all compatible online pylons (`50125608d`,
  `cs336_homework_auto_dispatch_cycle_targets_all_compatible_online_pylons`),
  with a documented operator runbook
  (`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`): npm
  installs Pylon → stays online → hosted dispatch → claim → closeout →
  validation accepts → treasury pays → wallet balance increases.
- Kernel authority objects for training (`docs/kernel/compute-training-authority.md`):
  checkpoint-family policy, validator policy, benchmark package, training
  policy registries, plus run/artifact/adapter-window/adapter-contribution
  records — with the owner split Psionic = execution truth, kernel-core =
  typed contracts, nexus-control = durable authority and receipts.

### The verification layer — what the "commit" machinery was for

This is the part the word "commit endpoint" points at. There was no HTTP
route literally named `/commit`; the commit step was the **commitment
protocol inside homework result submission**, and the endpoints that
consumed it were the validator-challenge routes. The machinery lived in
`crates/openagents-validator-service` (rev `d1a6a9dc4`, 1,253 lines,
deleted in the rebuild):

- Protocol `openagents.validator.gpu_freivalds_merkle.v1`: when a pylon
  submitted homework results, the result matrices were **Merkle-committed**
  — `MerkleCommittedMatrix { matrix_id, row_count, column_count, row_root,
  field_modulus }` — inside the `ExecutionProofBundle` (type imported from
  `psionic_runtime`) that traveled with the closeout. Committing first means
  the worker is bound to its claimed computation before anyone checks it; it
  cannot adjust results after seeing which rows get challenged.
- The control plane minted `ValidatorChallengeContext` records keyed by
  `proof_bundle_digest` and `request_digest`, queued them, and validator
  workers claimed them via `/api/training/validator-challenges/claim` with
  leases, retries, and timeouts (`Queued → Leased → Retrying → Verified /
  Rejected / TimedOut`).
- Verification was **Freivalds' algorithm over the committed matrices**:
  the validator checks A·B = C probabilistically with random vectors in
  O(n²) per round instead of recomputing the O(n³) multiplication, using
  Merkle row openings (`MerkleRowOpening` with sibling hashes) to verify
  that the rows used in the check belong to the committed roots. Typed
  failure codes covered every honest outcome: `DimensionMismatch`,
  `FieldMismatch`, `RowOpeningMissing`, `MerkleProofInvalid`,
  `FreivaldsMismatch`, `LeaseExpired`, `RetryBudgetExhausted`.
- Verdicts finalized via `/{challenge_id}/finalize` fed the closeout →
  payout chain.

**What we were trying to do:** pay strangers Bitcoin for training compute
without recomputing their work and without trusting them. Commit-then-
challenge makes cheating detectable at ~1000× less cost than the work
itself: a worker that fabricates results fails Freivalds; a worker that
commits honestly gets paid. Two design refs sit behind it — the `CommitLLM`
reference (commit-and-audit inference verification: Freivalds-style matrix
checks, trace commitments, CPU verifier flow) and the Catalini "Some Simple
Economics of AGI" verification-gap thesis that Episodes 213/214 cite as the
basis for the risk market: the gap between produced and *verifiably*
produced agent output is the economic bottleneck. The validator service was
our concrete machinery for closing that gap on training work. It also
explains the Episode 224 line about weak devices: "if you can't do
meaningful gradient descent work then you may just be getting assigned
validator work" — verification was itself paid homework for low-end
machines.

One operational compromise is worth remembering: homework-dispatch windows
were validated with the **aggregate challenge only** (per-contribution
sample challenges skipped — `9a4494992`, `9eda4b045`), and aggregate-only
validation was made defensible for payout. That was the pragmatic
throughput call that let payouts flow; a continuation should revisit
whether per-contribution sampling returns.

Two adjacent "commit" concepts also existed and should not be confused with
the above: the window `seal` step (window-level commit of merged
contributions), and psionic-distributed's **cluster commit authority**
(coordinator/term records naming which node may commit merged updates,
hashed into execution facts).

## What Survives Today

- **Psionic**: everything — the A1/A2 ports, demo lane, reference lanes, and
  the `psionic-train` manifest path are intact in the psionic repo, plus the
  newer Psion actual-pretraining lane (`./TRAIN`).
- **The workspace reference lane**: `projects/cs336/` with all course repos
  and notes.
- **This monorepo**: effectively nothing. The Bun rebuild removed
  nexus-control, the validator service, the deprecated Rust pylon, and all
  homework dispatch code. The current worker has zero CS336 code;
  `https://openagents.com/training/runs/run.cs336.a1.demo` returns 200 only
  because the SPA catch-all serves the app shell — there is no training-run
  projection behind it. Current `pylon-stats` payout totals (2,323 sats)
  count only the current-era D1 records; the April-era >1M sats lived in old
  Nexus records and did not carry over.
- **The old Nexus is deprecated**: per workspace policy, continuation must
  not route through `nexus.openagents.com` or revive nexus-control; the
  current `openagents.com` Worker surface is the authority.
- **Registry linkage**: `pylon.first_real_model_training_run.v1` (red) and
  `pylon.compute_revenue_modes.v1` (red) are the promises this work feeds;
  `docs/promises/source-set.md` still cites #4413 ("Prove public-style CS336
  Pylon earning end to end", 2026-04-21) as prior-proof source material.

## The Full Pipeline: Porting All Of CS336 Into Psionic

The point of CS336 was never one demo lane. The course is a complete
LM-pipeline curriculum — data → tokenizer → architecture → training →
systems → scaling → post-training — and the program is to port **all of
it** into Psionic as owned Rust, then run it as paid, verified homework on
the Pylon network. That does three things at once: builds our own
full-stack training pipeline from scratch (no PyTorch dependency anywhere),
gives the network a graded curriculum of real work, and turns the course's
own measurement structure (its leaderboards) into our public receipts
layer. The structural insight from reviewing the course repos
(`projects/cs336/repos/`): **Assignment 3's hosted training API is
literally our product architecture** — Stanford runs a FastAPI dispatcher
where students submit training configs by API key and fit scaling laws to
the results; that is exactly "worker dispatches training runs to Pylons,
public dashboard shows results." We are building the industrial version of
the course's own teaching infrastructure.

Repo boundary for everything below: Psionic-side ports happen in the
psionic repo (its own issue tracker); the openagents monorepo owns
dispatch, verification, receipts, leaderboards, and public projections.
The reference adapters in `projects/cs336/repos/*/tests/adapters.py` are
the conformance bars — port behavior against them, never vendor course
code.

### A1 — Basics (tokenizer, transformer, optimizer, training loop)

**Reference surface:** 21 adapters in `assignment1-basics/tests/adapters.py`
— linear/embedding/swiglu/RoPE/attention/transformer-block/transformer-LM/
rmsnorm/silu (model), get_batch/softmax/cross-entropy/gradient-clipping/
AdamW/cosine-schedule/save+load-checkpoint (training), tokenizer +
train_bpe (tokenizer). Data: TinyStories + OpenWebText sample.

**Psionic status: complete as a bounded reference lane** — all 21 rows
green since 2026-04-02 (`PSION_CS336_A1_FULL_PORT_MATRIX.md`; surfaces in
`psionic-models/src/cs336_a1_reference_stack.rs`,
`psionic-train/src/cs336_a1_reference_training.rs`,
`psionic-data/src/cs336_a1_bpe.rs`), packaged for dispatch as
`psion_cs336_a1_demo_v1`. The honest gap the matrix itself named — the
reference trainer was a tiny finite-difference trainer, not real
backprop — narrowed on 2026-06-10: `psionic#1114`
(`psion_cs336_a1_real_gradient_reference_v1`) lands hand-derived
analytic backprop for the A1 architecture shape, gradient-checked
against central differences in f64; single-head tiny config only, with
RoPE/multi-head backward, batching, and scale still open. Psionic's newer actual-pretraining lane (`./TRAIN`) owns real
training; the A1 lane's continuation is a **leaderboard-class run** — train
the A1 model on TinyStories/OWT shards across contributor devices with real
gradients, matching the course's own measure (validation loss under a
compute budget).

**Homework kinds:** BPE corpus-shard training, tokenization throughput
jobs, bounded training windows (already packaged), then real A1 training
windows.

### A2 — Systems (kernels, profiling, distributed training)

**Reference surface:** FlashAttention2 autograd functions (PyTorch +
Triton), DDP (+ after-backward hook), FSDP (wrapper, after-backward
reduce-scatter, gather-full-params), sharded optimizer, plus the handout's
profiling/benchmarking work.

**Psionic status: bounded reference coverage, explicitly not full parity**
(`PSION_CS336_A2_FULL_PORT_MATRIX.md`: no row missing-tracked, but bounded
evidence only; the earlier full-green claim was retired when Stanford's
Spring 2026 FSDP surface changed — a useful precedent for tracking
upstream adapter drift). The continuation is where the owned Rust stack
gets real: attention kernels in the owned backends
(`psionic-backend-metal`, `psionic-backend-cuda`, CPU), DDP/FSDP over real
transport (`psionic-distributed`, `psionic-collectives` — which already
carry the cluster commit-authority model), and a profiling harness whose
output is receipt-shaped (per-device tokens/sec, memory, step-time
histograms).

**Homework kinds:** kernel benchmark jobs (run the attention benchmark on
your device, return receipted numbers — this is also how the network learns
the answer to "what can my machine earn?", which Episode 224 promised),
multi-device throughput probes, gradient all-reduce window tests. Hardware
diversity is the asset: the same benchmark across M1/M2/M5 Macs and
consumer NVIDIA/AMD cards is a dataset no lab has.

### A3 — Scaling (the structural jackpot)

**Reference surface:** `assignment3-scaling` is a *training API client
exercise* — the course hosts a FastAPI + dispatcher + database service
(`hyperturing.stanford.edu:8000`, server side shipped in the repo for
non-students); students submit (model-size, data, compute) configs under an
API-key budget, get loss results back, and fit scaling laws (IsoFLOP
curves) to pick the best model under a fixed budget.

**Psionic status:** the analysis core landed 2026-06-10
(`psionic#1103`, lane `psion_cs336_a3_scaling_reference_v1`: IsoFLOP sweep
planner + Chinchilla approach-2 fit with synthetic-recovery proof). The
big work is not the math — it is
that **our worker training-run authority becomes the training API**, and
the Pylon network replaces Stanford's cluster. Scaling sweeps are
embarrassingly parallel small runs at varied (N, D) — the single
best-matched workload for a heterogeneous volunteer network: every device
trains a different tiny config; the curve fit happens in Psionic; the
output (a fitted scaling law and a predicted-best config) is a public
artifact.

**Homework kinds:** sweep cells (one small training run per assignment),
loss-curve reporting, validator replication of sampled cells. Public
dashboard = an IsoFLOP plot built from receipts.

### A4 — Data (Common Crawl → pretraining data)

**Reference surface:** `assignment4-data/tests/adapters.py` — HTML text
extraction from WARC bytes, language ID, PII masking (emails, phones,
IPs), NSFW + toxic-speech + quality classifiers, Gopher quality rules,
exact line dedup, MinHash document dedup. The leaderboard twist: students
train a **fixed** staff model on their filtered data — data quality is
measured by downstream eval delta, holding training constant.

**Psionic status:** the deterministic core landed 2026-06-10
(`psionic#1102`, lane `psion_cs336_a4_data_refinery_reference_v1`: PII
masking, Gopher rules, exact + MinHash dedup; HTML extraction and
model-backed classifiers remain planned). This assignment is the seed of
the owned data refinery, and it overlaps two
things already in motion: the data-market stream (epic 1: #4643–#4645 —
the redaction tool and NIP-DS sale path) and Episode 215's promise to pay
for data work. The A4 port gives the data market its quality machinery:
language ID, PII masking, quality classification, and dedup are exactly
what a redacted conversation bundle or a crawl shard needs before sale.

**Homework kinds:** the best CPU-only homework in the curriculum — WARC
extraction, filtering, classification, and MinHash dedup over crawl shards
run fine on weak devices (the machines that can't do "meaningful gradient
descent work" per Episode 224). Payment can follow the course's own
incentive design: pay per shard processed, with bonuses tied to the
measured eval delta of data trained on a fixed reference model — paying
for data *quality*, not volume.

### A5 — Alignment (SFT + reasoning RL)

**Reference surface:** `assignment5-alignment/tests/adapters.py` —
prompt/output tokenization, response log-probs, rollout rewards,
group-normalized rewards, policy-gradient loss, microbatch aggregation,
`grpo_train_step`, packed SFT dataset, batch iteration, MMLU/GSM8K response
parsing, per-instance DPO loss. GRPO-style reasoning RL on math tasks, with
an optional safety/DPO supplement.

**Psionic status:** the math core landed 2026-06-10 (`psionic#1101`,
lane `psion_cs336_a5_alignment_reference_v1`: GRPO/GSPO losses,
group-normalized rewards, aggregation, DPO, tokenization layout, bounded
eval parsers; model-coupled execution remains with the training
boundary). The network fit is strong: **GRPO rollout
generation is pure inference** — every pylon that can serve kind 5050 can
generate rollouts; reward scoring (GSM8K answer checking) is cheap
deterministic CPU work; only the policy-gradient update needs the training
boundary. The compute market built in epic 1 literally feeds the RL loop.

**Homework kinds:** rollout batches (inference), reward/eval grading
(deterministic CPU), SFT data packing, eval suites (MMLU/GSM8K passes as
receipted public evals).

### Verification Across The Pipeline

The commit-and-challenge layer generalizes per work class — this is where
the validator-service revival pays for itself across all five assignments:

| Work class | Verification |
|---|---|
| Training steps / matrix work (A1, A2, A3, A5 update step) | Merkle-committed matrices + Freivalds (the ported protocol) |
| Tokenizer / BPE / data filters / dedup / reward grading (A1, A4, A5) | Deterministic recompute spot-checks: commit the output digest, a validator re-runs a sampled shard exactly |
| Kernel/throughput benchmarks (A2) | Statistical cross-checks across same-class devices plus occasional replication |
| Rollout generation (A5) | Seeded-sample replication and reward-distribution checks |
| Scaling sweep cells (A3) | Sampled cell re-runs (cells are tiny by design) |

Deterministic-recompute classes are the cheapest to verify and the most
abundant (A4 especially) — which is exactly why weak devices can be both
workers *and* validators.

### Leaderboards As The Public Layer

Each assignment has a public leaderboard repo upstream because the course
made every stage *measurable*: A1 loss-under-budget, A2 latency/throughput,
A4 downstream eval delta, A5 reasoning accuracy. The continuation publishes
the same measures as receipt-backed leaderboards on openagents.com (the
existing tip-leaderboard pattern generalizes), so "the network is learning
to train models" stays publicly auditable stage by stage, and contributor
earnings attach to visible ranked work.

### Sequencing The Pipeline

1. **A1 homework first** — already packaged; re-attaches through the epic-3
   connector (#4664/#4669); rehearses dispatch + verification end to end.
2. **A3 sweeps second** — reuses the A1 trainer unchanged; embarrassingly
   parallel; produces the first crowd-sourced public scaling-law artifact
   while exercising many devices cheaply.
3. **A4 data third** — CPU homework for the long tail of weak devices;
   gives the epic-1 data market its quality machinery; eval-delta payment
   design.
4. **A2 systems fourth** — owned kernels and real-transport DDP/FSDP
   graduate A1/A3 work from tiny to real scale, and benchmark homework
   doubles as the public device-capability dataset.
5. **A5 alignment last** — needs inference (epic-1 compute market), reward
   grading, and the training boundary all in place; closes the loop with
   models post-trained by the network itself.

The lectures repo (17 executable `lecture_XX.py` files spanning
tokenization, resource accounting, architectures, MoE, GPUs, Triton,
parallelism, scaling laws, inference, eval, data, and alignment) is the
derived-notes program for the lane — `projects/cs336/notes/` has lecture 01
done; the remaining notes are cheap background work that keeps the ports
honest against the course's intent rather than just its test files.

## What Continuation Needs

The training epic (#4664–#4671) builds the modern skeleton; CS336 is the
curriculum and the verification layer that plug into it. Concretely:

1. **Use epic 3 as the spine, not a parallel system.** The Psionic connector
   (#4664) and the training assignment boundary (#4669) are exactly the
   re-attachment points for the packaged `psion_cs336_a1_demo_v1` lane — it
   was built for zero-touch manifest dispatch and still is. The bounded
   remote Qwen run (#4670) and a CS336 A1 homework run are the same shape:
   assignment → local Psionic execution → signed receipts → closeout →
   payment. A1 is the cheaper, already-packaged rehearsal content for the
   #4670 machinery.
2. **Re-home the training-run authority surface in the current worker.**
   Runs, windows (plan/activate/seal/reconcile), and lease claim as
   D1-backed records with public projections — the worker already has the
   assignment-lease lifecycle (proven in #4633); training windows are an
   extension of it, not a new system. Public run pages
   (`/training/runs/{runId}` projection) replace the dead SPA-shell route.
3. **Port the commit-and-challenge verification layer.** The
   `openagents-validator-service` contract at `f5919c766^` is the reference:
   Merkle-committed matrices in closeout artifacts, a D1-backed challenge
   queue with lease/retry/timeout, Freivalds verification, typed failure
   codes. Two viable homes: (a) validator work as paid Pylon assignments
   (the Episode 224 model — weak devices verify), with the worker holding
   the queue and verdicts; (b) worker-side verification in TS for small
   matrices (Freivalds is cheap field arithmetic — viable in a Worker for
   the bounded A1 scale). Start with (b) for the demo lane, graduate to (a)
   when real GPU-scale homework returns. Revisit the aggregate-only
   validation compromise explicitly when doing this.
4. **Port the homework dispatcher as a worker queue/cron.** The old
   auto-dispatch cycle (target all compatible online pylons, prefer
   dispatched homework over starter backlog, bounded manual override) maps
   cleanly onto the buy-mode dispatcher pattern already specified in #4639 —
   homework is a job kind, not a separate dispatch system.
5. **Wire payouts through the current settlement path** (MDK bridge with
   public receipts), not treasury-era code. The accepted-work receipt shape
   already exists (`receipt.nexus_pylon.settlement.*`).
6. **Honesty boundaries carried forward:** bounded A1 demo evidence is not a
   real pretraining claim; A2 coverage is bounded reference evidence, not
   full parity; aggregate-only validation must be named in any payout copy
   it gates; and the training promises' green copy runs through the
   fine-tune gate's scope-language discipline regardless of curriculum.

### Issue set — **filed on GitHub 2026-06-10 as #4673–#4684**

Monorepo-side rails (refined at filing: verification v2 became
verification-**class-pluggable**, preparing the Tassadar exact-replay
class):

1. #4673 `training: worker-side training-run and window authority`
2. #4674 `training: pluggable verification v2 (commitments, challenge queue, replay + Freivalds classes)` — ports the `f5919c766^` validator-service contract; verification classes (freivalds_merkle, deterministic_recompute, exact_trace_replay, statistical_cross_check, seeded_replication) are pluggable registrations
3. #4675 `training: CS336 A1 homework job kind with paid closeouts`
4. #4676 `training: validator work as paid Pylon assignments (weak-device lane)`
5. #4677 `training: public run pages replacing the dead /training/runs SPA shell`

Pipeline lanes (monorepo side: dispatch kinds, verification classes,
leaderboards; each flags its Psionic-repo counterpart as an external
dependency):

6. #4678 `training: A1 leaderboard-class run — real gradients across contributor devices` (Psionic ask: a real-gradient A1 lane; the bounded finite-difference trainer is honestly insufficient)
7. #4679 `training: A3 scaling-sweep homework — crowd-sourced IsoFLOP curves` (Psionic ask: scaling-law fitting + run planner)
8. #4680 `data: A4 data-refinery homework — filtering, dedup, and eval-delta payment design` (Psionic ask: A4 adapter port into psionic-data; shares quality machinery with #4643–#4645)
9. #4681 `systems: A2 benchmark homework — the public device-capability dataset` (Psionic ask, longer horizon: owned kernels, real-transport DDP/FSDP)
10. #4682 `alignment: A5 rollout and grading homework — RL fed by the compute market` (Psionic ask: SFT packing, GRPO losses, DPO)
11. #4683 `training: per-assignment receipt-backed public leaderboards`

Filed addition connecting this epic to the Tassadar plan
(`docs/tassadar/`):

12. #4684 `training: executor-trace homework — the exact-replay work class (Tassadar lane)` — bounded executor workloads dispatched through the epic-3 connector (#4664), verified by exact trace replay (the cheapest verification grade; weak devices fully competent), with a hard disclosure boundary: dispatch plumbing only, zero public Tassadar capability copy, the psionic capability envelope governs what may be dispatched.

Rails 1–5 slot between #4669 (boundary) and #4670 (bounded remote Qwen
run); lanes 6–11 follow the pipeline sequencing above (A1 → A3 → A4 → A2 →
A5); #4684 needs only #4674 plus the #4664 connector and can run early as
the always-available verification-perfect work class. Together they re-run
the Episode 224 story — paid, verified homework — and carry it through the
whole course: by the end the network has trained, profiled, scaled,
data-fed, and post-trained a model on a stack that is owned Rust top to
bottom.

## Evidence Reviewed

- Git history (this repo): `d1a6a9dc4` (Episode 224 demo launch path; full
  nexus-control route surface; `crates/openagents-validator-service/src/lib.rs`),
  `cbf617ca4` (automatic homework dispatcher + operator runbook),
  `50125608d` (fan-out dispatch), `9a4494992`/`9eda4b045` (aggregate-only
  homework validation), `f12e5d2a6`/`79820bc7d` (Episode 223 dual-host A1
  proofs), `f5919c766` (the rebuild that removed it all)
- `docs/transcripts/224.md` (Distributed Training 101), Episode 222/223
  reports at `d1a6a9dc4:docs/reports/pylon/`
- `d1a6a9dc4:docs/kernel/compute-training-authority.md`,
  `d1a6a9dc4:docs/pylon/distributed-training-launch-status.md`,
  `cbf617ca4:docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`
- Psionic repo: `docs/PSION_CS336_A1_DEMO_LANE.md`,
  `docs/PSION_CS336_A1_FULL_PORT_MATRIX.md` (21/21 adapters green, bounded),
  `docs/PSION_CS336_A2_FULL_PORT_MATRIX.md`,
  `docs/PSION_CS336_A2_REFERENCE_LANE.md`, CS336 commit history
- `projects/cs336/` lane: `README.md` (assignment arc, repo table, compute
  posture), `repos/assignment1-basics/tests/adapters.py` (21 adapters),
  `repos/assignment2-systems/tests/adapters.py` (FlashAttention/DDP/FSDP/
  sharded-optimizer), `repos/assignment3-scaling/README.md` (hosted
  training API + shipped server/dispatcher), `repos/assignment4-data/`
  (adapter set + fixed-staff-trainer leaderboard design),
  `repos/assignment5-alignment/tests/adapters.py` (SFT/GRPO/DPO/eval
  parsing), `repos/lectures/` (17 executable lecture files),
  `notes/lecture-01-overview-and-tokenization.md`
- Workspace: `projects/cs336/README.md` and lane notes
- Live: `GET /api/public/pylon-stats`,
  `https://openagents.com/training/runs/run.cs336.a1.demo` (SPA shell only)
- `docs/promises/source-set.md` (#4413), registry 2026-06-10.4
