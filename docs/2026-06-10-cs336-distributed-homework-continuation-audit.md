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

### Candidate issue set (not filed — sequence after epic 3's #4669)

1. `training: worker-side training-run and window authority (runs, windows, leases, public projections)`
2. `training: commit-and-challenge verification v2 (Merkle commitments in closeouts, D1 challenge queue, Freivalds verifier)` — port the `f5919c766^` validator-service contract
3. `training: CS336 A1 homework job kind through the dispatcher (#4639 pattern) with paid closeouts`
4. `training: validator work as paid Pylon assignments (weak-device lane)`
5. `training: public run page projection replacing the dead /training/runs SPA shell`

These slot between #4669 (boundary) and #4670 (bounded remote Qwen run) or
immediately after the epic, and would let the network re-run the Episode
224 story — paid homework with verification — on the current honest stack.

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
  `docs/PSION_CS336_A1_FULL_PORT_MATRIX.md`,
  `docs/PSION_CS336_A2_FULL_PORT_MATRIX.md`, CS336 commit history
- Workspace: `projects/cs336/README.md` and lane notes
- Live: `GET /api/public/pylon-stats`,
  `https://openagents.com/training/runs/run.cs336.a1.demo` (SPA shell only)
- `docs/promises/source-set.md` (#4413), registry 2026-06-10.4
