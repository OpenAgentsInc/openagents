# Pluralis → Pylon Adaptation Roadmap

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-12

Status: planning document. Nothing here is a capability claim or product
copy; every item ships behind the product-promises registry discipline and
the issue tracker. Authored by Fable (model `claude-fable-5`; authorship
verified against the live session transcript before commit, per the
provenance protocol established after the 2026-06-12 model-fallback
incident). Companion documents: the buildout plan
(`2026-06-10-psion-full-pipeline-buildout-plan.md`), the training program
status (`2026-06-10-training-program-status.md`), the Tassadar research
directive (`../tassadar/RESEARCH_PLAN.md`), the Pluralis lane README
(workspace `projects/pluralis/README.md`), and the forum analysis this
roadmap operationalizes (ASI reading-group thread, post `6197bd1b`,
2026-06-12).

## 0. Rules of Engagement

1. `projects/pluralis/repos/*` (agora, node0, AsyncPP, AsyncMesh) is
   **read-only reference code**. We port ideas, not code. Owned
   implementations are Rust, in psionic (execution truth) or the monorepo
   (dispatch, verification, receipts, funnel, payments), per the workspace
   repo boundary.
2. The W3 standing order binds every item below: **no public gradients
   into the main optimizer, ever**. Robust-aggregation / sparse-averaging
   decentralized training is a side experiment with canary evals, not the
   run. Nothing in this roadmap relaxes that; several items exist to make
   the side experiment honest.
3. The frame, from the forum analysis: Pluralis bounds **influence**
   (topology, sparsity, compression decide what a stranger's write can
   touch); our program bounds **admission** (verification classes decide
   what a stranger may sell). Influence bounds complement admission
   classes. They never replace them. Every adaptation below imports an
   influence-bound or lifecycle mechanism *underneath* an existing
   verification class, never instead of one.
4. Registry discipline: no new promise is created by this document. Items
   land as issues against the existing rails (#4673 run/window authority,
   #4674 verification classes, #4676 validator lane, #4681 device
   benchmarks, #4748 W2 contract freeze) and surface in existing promises
   only when receipts exist.

## 1. What Pluralis Has That Pylon Lacks (the inventory)

Read from the four repos, most adaptable first:

- **A staged contributor join lifecycle** (agora
  `docs/agora-system/startup-sequence.md`): authorization → state
  download → join queue (pacing, not waitlist) → Sync Phase 1 (invisible
  to trainers, receives averaged state at `weight=0`, ~400 steps) → Sync
  Phase 2 (processes real batches, optimizer warms up, still `weight=0`,
  ~100 steps) → active. The phase is a published DHT record
  (`sync_phase`) that routers filter on.
- **Staleness as a first-class quantity**: `max_allowed_stale` (node0
  config: 5 steps) triggers sync-mode re-entry; a lagged worker is
  re-integrated through the same two-phase ramp rather than rejected or
  trusted.
- **Failure semantics for collective ops** (agora `fault-tolerance.md`):
  tensors chunked with per-chunk timeouts; a failing sender is banned for
  the round, not retried; partial all-reduce results are preserved; a
  peer failure is fatal only if an entire pipeline stage empties; new
  joins are blocked during the steps around an averaging round so nobody
  downloads half-updated state.
- **Sparse state averaging (SPARTA)**: same-stage peers average
  `sparse_avg: 0.05` of parameters per round (`average_state_every: 5`),
  with a partitioned index selector rotating through ~1/p random
  partitions so coverage is complete over time and no single round moves
  more than a sliver (AsyncMesh `sparta/sparta.py`).
- **A presence/compute incentive split**: presence points accrue from
  Sync Phase 1 (the peer is reporting to DHT); compute points accrue from
  Phase 2 (real batches); sync-phase samples do not count toward the
  per-stage `target_batch_size`.
- **Memory and admission discipline** (`memory-communication.md`): Adam
  moments offloaded to host RAM (24 GB GPU + 80 GB RAM contributor
  shape); BF16 autocast with FP32 master params/grads; hard admission
  gate on BF16 tensor-core capability (compute capability ≥ 8.0 — T4/V100
  excluded *with a stated reason*: they would emulate BF16 slower than
  FP32).
- **Compression at every boundary**: PowerSGD rank-64 gradient averaging
  (node0), subspace-compressed pipeline-stage boundaries
  (`compression_rate: 100`), so activations survive residential WAN
  links.
- **Asynchrony with correction, not denial** (AsyncPP, ICML 2025):
  weight stashing and Nesterov-corrected optimizers treat gradient delay
  as a measured quantity to correct for.

What Pluralis does **not** have, which is why this is an adaptation and
not an adoption: acceptance. No per-contribution verification, no
settlement, no receipts, no adversarial audit, no economics gate. Their
trust is HuggingFace-token identity plus influence bounds; their points
are leaderboard-only and unverified. Everything we import lands on rails
where contributions are accepted and paid, which changes several designs
(noted inline).

## 2. The Roadmap

Phases key to the model ladder (R0 done → R1 operator devices → R2
network rung → R3+). Each item names its owner per the repo boundary and
the rail it extends. No phase starts before the prior rung's closeout
receipt, per the ladder rules.

### P0 — Contracts and ledger entries (now; no hardware required)

- **P0.1 Pylon join-lifecycle state machine.** Adopt the six-step staged
  join as typed Pylon contributor states: `registered → qualified →
  state_synced → warmup → active`, plus `lagged → sync_reentry` on the
  back edge. Map them onto the capacity funnel's existing reason-code
  taxonomy so the funnel stops being a binary dark/ready surface and
  becomes the ladder a device climbs — today the funnel reason-codes the
  dark side richly and says almost nothing about the bright side.
  Owner: monorepo (funnel + Pylon client states). Rail: the funnel
  (#4629 lineage) and run/window authority (#4673).
- **P0.2 Staleness fields in the W2 day-0 contract.** Already proposed
  publicly (forum post `6197bd1b`, against #4748): window-seal records
  carry (a) the staleness distribution of merged contributions
  (steps-behind per contribution), (b) contributor-churn events within
  the window, (c) verification overhead as a fraction of window cost,
  published per ladder rung. Add `max_allowed_stale` as a per-run config
  field in the #4673 run authority so the sync-reentry trigger is a
  contract value, not a convention.
  Owner: monorepo. Rail: #4748, #4673.
- **P0.3 Derisking-ledger entries** (one line each, with the reason, so
  "we considered it" is on the record per the buildout plan's §6 rule):
  SPARTA-class sparse averaging (side experiment only, W3 rule);
  PowerSGD rank compression (blocked on the verification-compatibility
  question, P3.2); subspace-compressed stage boundaries (R3+-conditional,
  P3.1); AsyncPP delay-corrected optimizers (enters only via the ablation
  manifest if a side experiment earns it).
  Owner: psionic (the ledger lives with the ablation system).

### P1 — R1 rung mechanics (operator devices; days-scale runs)

- **P1.1 The shadow-window ramp** (the Sync Phase 1/2 adaptation). A
  joining device does not enter a paid merged window on day one. It runs:
  *Phase 1 analogue* — download the durable checkpoint, verify digests,
  replay a sealed window locally, produce receipts that are checked but
  never merged ("shadow window"); *Phase 2 analogue* — run a live window
  whose outputs are verified and receipted but excluded from the merge
  (`weight=0` in our terms), warming scheduler trust and (for gradient
  work classes, later) optimizer-state continuity. Only then: active,
  merged, paid at full class rate. Pluralis's numbers (400 + 100 steps)
  are their tuning, not ours — the R1 deliverable is *measuring* the ramp
  length that actually reduces post-join divergence, which is itself an
  ablation cell.
  Owner: psionic (window mechanics) + monorepo (dispatcher policy).
  Rail: #4673 window lifecycle.
- **P1.2 Snapshot-lags-live checkpoint publication.** Pluralis publishes
  periodic S3 snapshots and lets joiners sync forward; they never chase
  live state. We already require a window to be *sealed only when its
  checkpoint digest is durably stored* (buildout plan §7). Add the
  complementary rule: joiners bootstrap from the last durable seal and
  catch up via the shadow-window ramp — never from any in-flight state.
  Snapshot cadence becomes a run-authority config.
  Owner: monorepo (seal/lifecycle), psionic (resume drills already
  exist). Rail: #4673.
- **P1.3 The join-blocking window.** No state download and no join
  transition while a merge/seal operation is in flight, so nobody
  bootstraps from half-updated state. This is a dispatcher scheduling
  rule, cheap at R1 scale and load-bearing at R2.
  Owner: monorepo (dispatcher, #4639-pattern). Rail: #4673.
- **P1.4 Reasoned hardware admission gates.** Adopt the
  exclusion-with-stated-reason pattern (T4/V100: "would emulate BF16
  slower than FP32") into the device-capability dataset: every device
  class admitted to a work class carries the measured reason; every
  exclusion carries one too, surfaced through the funnel reason codes.
  Sustained-vs-burst thermal behavior (already planned in #4681) and
  host-RAM headroom for optimizer offload join the qualification probe —
  Pluralis's 24 GB GPU + 80 GB RAM contributor shape says host RAM is a
  binding constraint our benchmarks do not yet measure.
  Owner: monorepo (#4681) + psionic (preflight qualification, which
  already does GPU/memory/thermal checks in the actual-pretraining lane).

### P2 — R2 rung mechanics (contributor devices; paid verified windows)

- **P2.1 Collective-op failure semantics.** Port the agora rules into
  `psionic-collectives` and the dispatcher lease policy: chunk transfers
  with per-chunk timeouts; ban-for-round instead of retry (failures
  logged, training continues; persistent failers leave the swarm);
  preserve partial results; and translate "fatal only if a stage empties"
  into "a window aborts only when no warm standby can be promoted" —
  which is exactly the shard-preloaded standby-Pylon dispatcher feature
  the buildout plan §7 already names. Volunteer churn stops being an
  incident and becomes a priced, receipted event.
  Owner: psionic (collectives) + monorepo (leases, standby promotion).
  Rail: #4673, #4674.
- **P2.2 Staleness-priced acceptance.** Every contribution carries
  `steps_behind` (P0.2 made it a contract field; P2 makes it
  load-bearing): the verification classes gain a staleness dimension,
  and a contribution beyond `max_allowed_stale` routes to `sync_reentry`
  — re-ramped through P1.1's shadow window — rather than being either
  rejected (wasting a willing device) or merged (importing divergence).
  This is the AsyncPP lesson applied to dispatch rather than to the
  optimizer: measure the delay, respond to it, never pretend it is zero.
  Owner: monorepo (#4674 classes) + psionic (window merge rules).
- **P2.3 Presence/compute receipt split.** Adopt the two-tier incentive
  shape with the difference made explicit: Pluralis presence points are
  unverified leaderboard decoration; our presence receipts settle money,
  so they must be (a) bounded (a capped availability floor per §3.5 of
  the buildout plan — floors keep a fleet alive and enrolled, not rich),
  (b) verified by liveness/qualification probes (the #4681 instrument,
  paid), and (c) Sybil-priced — presence pay per identity, not per
  process, mirroring Pluralis's one-token-many-GPUs aggregation rule.
  Compute receipts remain what they are: verified work closeouts at the
  class rate. Shadow-window work (P1.1) pays presence-tier, not
  compute-tier, exactly as sync-phase samples do not count toward
  `target_batch_size`.
  Owner: monorepo (payments + funnel). Rail: #4674, #4676, #4681.
- **P2.4 The SPARTA canary.** The W3-sanctioned side experiment, run by
  the book: a non-main model at R1/R2 scale trained with sparse
  partition-rotating averaging (their tuning — 5% per round, every 5
  steps, random rotating partitions — as the starting grid), against the
  synchronous baseline, with canary evals and the eval schema the
  research plan already fixed (first divergence, never perplexity alone).
  Entry criteria: P0.3 ledger entry filed, harness before claim. Kill
  criteria: written before the run (see §4). If the canary holds, sparse
  averaging becomes a candidate for *replica* synchronization inside
  trusted window groups — still never public gradients into the main
  optimizer.
  Owner: psionic. Rail: ablation system
  (`training.ablation_system.v1`, planned).

### P3 — Conditional and research-gated (R3+, or never)

- **P3.1 Pipeline-stage-sharded windows.** Pluralis's deepest property —
  no contributor ever holds the whole model — only becomes relevant when
  a ladder rung exceeds what a single contributor device class can hold
  (R3's ~1B Psion does not; this is an R4-class question). If and when:
  stage ownership, subspace-compressed boundaries, and per-stage SPARTA
  groups are the reference design, and the unextractability property is
  itself a product feature worth a registry promise — but not before the
  R2 economics gate has cleared against the rented-cluster comparator
  twice. Until then this item exists to prevent anyone (including us)
  from starting it early.
  Owner: psionic, hypothetically. Rail: none yet, deliberately.
- **P3.2 The compression/verification compatibility question.** A named
  research question, not a work item: does PowerSGD-class low-rank
  gradient compression compose with Freivalds checks over
  Merkle-committed matrices — can a validator verify a *compressed*
  contribution without decompressing to full rank? If yes, compressed
  gradient work classes open to strangers at WAN-friendly bandwidth. If
  no, compression stays strictly inside the trust boundary (operator
  devices), and that boundary is stated in the verification map. Either
  answer is worth one short doc with receipts.
  Owner: psionic research + #4674's class definitions.

## 3. What We Deliberately Do Not Adapt

- **HuggingFace-token identity.** We have registered agent/contributor
  identity with claims, receipts, and settlement. Importing token-based
  identity would be a downgrade with extra steps. (The one idea kept:
  many processes, one identity, one aggregated score — see P2.3.)
- **Unverified incentive points.** Anything that settles money rides a
  verification class. Full stop. Pluralis can afford decorative points;
  a network that pays sats cannot.
- **Hivemind/libp2p DHT discovery.** Pylon registration, the funnel, and
  the dispatcher already provide discovery with authority and receipts.
  Decentralized discovery becomes interesting only if the platform stops
  being the dispatcher of record; if that day comes, the workspace's
  `iroh` reference lane is the Rust-native starting point, not a
  Hivemind port.
- **Trust-by-architecture as a substitute for acceptance.** The whole
  point of the synthesis: influence bounds *underneath* admission
  classes, never instead of them.
- **Any Python/Hivemind code.** Read-only reference, per the workspace
  manifest rules.

## 4. Falsifiers and Kill Conditions (written before the work)

- **Shadow-window ramp (P1.1):** if R1 measurement shows the Phase-2
  analogue does not reduce post-join divergence or dispute rates versus
  bootstrap-and-merge, the ramp collapses to Phase 1 only (verify the
  joiner can replay a sealed window; skip the warm-up window), and the
  doc records the negative result.
- **Staleness pricing (P2.2):** if per-contribution staleness accounting
  generates more dispute/adjudication overhead than the divergence it
  prevents at R2 scale, simplify to a binary fresh/stale gate at
  `max_allowed_stale` and say so in the seal-record schema rev.
- **SPARTA canary (P2.4):** kill if the canary's first-divergence and
  held-out-family evals degrade beyond the pre-registered bound versus
  the synchronous baseline, or if the communication savings are
  immaterial at our actual fleet scale (the honest possibility that 5%
  sparse averaging solves a problem only 8B-at-WAN-scale has). A killed
  canary still produces the ledger entry and the receipts.
- **The roadmap itself:** if the R2 economics gate fails twice (the
  buildout plan's own rule), the network-training thesis pauses and this
  roadmap's P2/P3 items pause with it — the join lifecycle, funnel
  ladder, and failure semantics (P0.1, P1.x, P2.1) survive, because
  they price availability and churn for *any* distributed work class,
  not just training.

## 5. Sequencing Summary

```
now ──────────► R1 (operator devices) ──────► R2 (network rung) ──► R3+
P0.1 funnel ladder ─ P1.1 shadow-window ramp ─ P2.1 failure semantics
P0.2 staleness contract ─ P1.2 snapshot/seal rule ─ P2.2 staleness pricing
P0.3 ledger entries ─ P1.3 join-blocking window ─ P2.3 presence/compute split
                      P1.4 admission gates ─────── P2.4 SPARTA canary
                                                    P3.x stays gated
```

Dependencies: P0 items are pure contract/docs work and start now. P1
gates on the #4673/#4674 rails landing. P2 gates on R1's closeout receipt
and real contributor devices existing (the funnel currently reports the
fleet dark; this roadmap does not pretend otherwise). P3 gates on R2
economics, twice.

**Implementation status** (same day, 2026-06-12): the contract/code scope
of P0–P2 landed on main within hours of filing. openagents: #4848
(`8f9bdd040` join-lifecycle ladder + funnel projection), #4849
(`25e07afdd` seal staleness/churn/overhead metadata + `maxAllowedStale`,
migration 0174), #4850+#4851 (`31368c250` bootstrap-from-durable-seal
grants + seal-in-flight join barrier, migration 0175), #4852 (`41d8c858f`
reasoned admission gates + host-RAM/thermal probe schema), #4853
(`244d35aca` staleness-priced acceptance — no reject arm exists by type;
over-stale routes to sync_reentry), #4854 (`9a4150273` presence/compute
receipt tiers, per-identity-per-day Sybil-priced cap). psionic: #1124 +
#1128 (`5b5cf6a2` derisking ledger + PowerSGD×Freivalds answer: composes
with the algebra, not the provenance), #1125 (`a2b1d269` shadow-window
ramp with type-level merge exclusion via the `MergeEligibleReceipt`
proof token), #1126 (`e8869eca` collective failure semantics: chunked
timeouts, ban-for-round, partial preservation, standby-gated abort),
#1127 (`a48843a8` SPARTA canary harness: standing order enforced in
code, pre-registration digest-pins the grid, toy artifacts cannot
decide the canary by typed rule). All 12 child issues closed same day.
**Tracker closure (2026-06-13):** master issue #4855 closed on its own
done-condition (every child closed, roadmap updated). The remaining
hardware/settlement-gated evidence is NOT lost by that closure — it is
owned where it belongs: the `training.*` registry promises carry the
R1/R2 rung gates (`training.model_ladder.v1` blockers
r1_full_rehearsal_missing / rung_economics_gate_format_missing,
`training.marathon_operations.v1` blockers, and the seal/staleness/
overhead fields' first real values), and each child issue's closing
comment records its specific unclaimed bullet. When R1 runs on operator
devices, those receipts flip registry blockers — no tracking issue
needed in between.
Every hardware/settlement-gated
acceptance bullet (live R1/R2 receipts, real devices, settled payments)
remains open and is recorded per-issue — none are claimed. Side finding:
psionic#1129 (68 pre-existing psionic-train test failures from
cwd-relative fixture paths, surfaced by the #1125 merge triage). P3.1
remains deliberately unfiled.

**Issues of record** (filed 2026-06-12; master tracking issue
openagents#4855): P0.1 funnel join-lifecycle
ladder — openagents#4848; P0.2 staleness contract fields — #4849; P0.3
derisking-ledger entries — psionic#1124; P1.1 shadow-window ramp —
psionic#1125; P1.2 bootstrap-from-durable-seal rule — #4850; P1.3
join-blocking window — #4851; P1.4 reasoned admission gates + host-RAM
probe — #4852; P2.1 collective-op failure semantics — psionic#1126; P2.2
staleness-priced acceptance — #4853; P2.3 presence/compute receipt
split — #4854; P2.4 SPARTA canary — psionic#1127; P3.2
compression/verification compatibility question — psionic#1128. P3.1
remains deliberately unfiled, per its own text.

The one-sentence version: adopt Pluralis's *lifecycle* (join ramp,
staleness, failure semantics) wholesale because it prices availability
and churn — things our receipts want to pay for anyway; adopt their
*optimizer-side* machinery (sparse averaging, compression, async
correction) only through the side-experiment door the research plan
already built; and adopt their *topology* (stage sharding) only when a
ladder rung forces it. Receipts or it did not happen.

## 6. The next tracker: W5 — public training windows (what #4855 set up but did not fly)

#4855 is a **preflight hardening** tracker, not a decentralized-training
tracker. It closed `completed` on its own done-condition ("every child
closed, roadmap updated") — it ported Pluralis's *lifecycle* (typed
contributor states, staleness, durable seals, shadow-window join ramp,
reasoned hardware admission, collective-op failure semantics,
presence/compute receipt split, SPARTA canary) and deliberately kept the
standing order intact: **no public gradients into the canonical
optimizer.** It built the runway; it did not fly the plane.

What it did **not** add — and what real decentralized model training
needs — is the **public model-update layer**:

```
accepted training window  (checkpoint + shard + config + seed + delta digest
                           + loss stats + verification refs + acceptance + receipt)
quarantine optimizer      (canonical → quarantine → promoted; promote, never submit)
gradient verification     (Tier 0 hash · Tier 1 recompute · Tier 2 replicate ·
                           Tier 3 stats · Tier 4 canary · Tier 5 downstream)
robust aggregation        (untrusted updates never aggregated pre-quarantine)
checkpoint lineage        (+ rollback; which windows produced which checkpoint)
dataset shard authority   (bind windows to verified-trace shards)
bandwidth-aware topology  (windowed/local-SGD, not global all-reduce)
device tiers              (validator / adapter / dense / canonical roles)
staged payout             (pending → provisional → accepted → settled → clawback)
canary-eval promotion     (first-divergence metrics, not loss)
registry promise          (training.public_gradient_windows.v1, red/planned)
```

That layer is specified as **W5** in
[`../tassadar/RESEARCH_PLAN.md`](../tassadar/RESEARCH_PLAN.md) §5. The next
master tracker to file is therefore **not** a reopen of #4855 but a new
one — *"Public Training Windows: quarantine, verify, promote, settle"* —
gated behind a W3 student checkpoint worth training in public. The
distinction to hold in copy: **#4855 made decentralized work safer at the
edges; W5 is the lane where public updates can advance a shared model, and
it earns the canonical checkpoint gate by gate.**

— Fable, for the training program. 2026-06-12. W5 lane appended 2026-06-15.
