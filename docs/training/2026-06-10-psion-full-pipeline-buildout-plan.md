# Psion Full Training-Pipeline Buildout Plan

Date: 2026-06-10

Status: planning document. Nothing in here is a capability claim or product
copy; every stage below ships behind the product-promises registry
discipline (claims stay red/planned until receipts exist). This plan extends
the CS336 continuation program
(`docs/2026-06-10-cs336-distributed-homework-continuation-audit.md`, issues
#4673–#4684) from "port the course" to "operate the full pipeline," using
Hugging Face's Smol Training Playbook — now chaptered into
`psionic/docs/smol/` (psionic commit `ee915fcb`) — as the reference for what
a complete, honest LLM training operation actually requires.

Repo boundary, per workspace rules: **Psionic owns execution truth**
(model, data, training, eval, backends, distributed); **this monorepo owns
dispatch, verification, receipts, leaderboards, payments, and public
projections**. Each workstream below names which side owns what. Psionic
work items go to the psionic tracker; monorepo work items extend the
#4673–#4684 set.

## 1. Why This Plan Exists

The CS336 program gives us the curriculum: A1–A5 ported into owned Rust and
run as paid, verified homework. What it does not give us is the
*operational* shape of a real training organization — the part the smol
playbook documents bluntly: 100+ ablations consuming 161k of 437k total GPU
hours, a full restart at 1T tokens from a tensor-parallel seeding bug,
dataloader and storage failures that only appear at scale, multi-stage data
curricula, a post-training arc (mid-training → SFT → preference
optimization → RL), and infrastructure measurement at every layer.

The playbook's three meta-lessons map directly onto what we already
practice and what we still lack:

1. **"Never change anything unless you've tested that it helps"**
   (derisking) — this is our evidence/receipt discipline applied to
   training decisions. We have the receipt machinery; we do not yet have an
   ablation *system* that produces those receipts routinely.
2. **"By far the most influential aspect of LLM training is data
   curation"** — our A4 refinery port is the seed; we do not yet have a
   corpus program, mixture ablations, or a multi-stage curriculum.
3. **"Expect scale to break things"** — the playbook's marathon chapter is
   a checklist of failure modes (throughput collapse, loss spikes, seeding
   bugs). Our network adds new ones (volunteer churn, heterogeneous
   numerics). The verification economy is our native answer, but it has to
   be wired into every stage, not just homework matrices.

One structural advantage is worth stating once, without hype: the playbook
team needed a fixed 384-H100 Slurm reservation and a spare node. Our
architecture is built around the opposite assumption — elastic, untrusted,
heterogeneous capacity with verification instead of trust. Where the
playbook says "reserve a spare node with the dataset preloaded," we say
"the funnel reason-codes dark capacity and the dispatcher routes around
it." That substitution — receipts for reservations — is the whole company
thesis applied to training, and it is also why every stage below has a
verification class attached.

And one constraint stated equally bluntly: SmolLM3 is 3B params on 11T
tokens at ~30% MFU for a month on 384 H100s. Our retained
source-of-truth distributed run is a tri-host, 12-step, 3,992-token
rehearsal at 2.74 effective tokens/sec
(`psionic/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md`). The plan respects
that gap: every phase is sized to hardware we actually have, and the
model ladder below never skips a rung.

## 2. The Training Compass, Answered For Us

The playbook demands a written answer to *why train* before anything else.
Ours, on the record:

- **Why.** All three of the playbook's legitimate reasons apply, but the
  binding one is strategic: the Pylon network needs a graded supply of
  real, verifiable training work, and OpenAgents needs architectural
  sovereignty — you cannot graft Tassadar executor heads into a model you
  call over an API, and you cannot backprop through a vendor's forward
  pass (`docs/tassadar/work-that-proves-itself.md`). Research (exact-core
  hybrids, verified distributed training) and production (small
  specialized models served by Psionic) both hang off that.
- **What.** The Psion compact-decoder family: small dense models (the
  ladder in §9), multilingual-optional, math/code-leaning, trained
  entirely on the owned Rust stack. Explicitly *not* exactness-claiming —
  Tassadar is where exactness lives, and `PSION_EXECUTOR_PROGRAM.md` is
  the wall between the lanes.
- **What not.** No MoE until the dense ladder earns it (the playbook's own
  advice for small teams on tight timelines); no from-API distillation as
  a substitute for owned pretraining; no public capability copy without
  registry receipts.

The compass also supplies our baseline rule: start from a proven
architecture (Llama-3.2-class dense decoder: GQA, SwiGLU, tied embeddings,
RMSNorm), not a novel one. Psionic's A1 stack already is that shape. Every
deviation goes through the derisking ladder in §3.

## 3. Workstream 1 — The Ablation System (the missing organ)

The playbook's single strongest structural claim is that ablations are not
overhead, they are ~40% of total cost, and the teams that win are the ones
that iterate fastest. We have no ablation system today — psionic has a
"bounded ablation" tool in the baseline bundle, but no harness that runs a
config matrix, evaluates checkpoints against a fixed suite, and emits
comparable receipts.

**Psionic side (owns the harness):**

- An ablation manifest format: baseline config + a single named delta
  (architecture / data / optimizer), enforcing the playbook's
  one-change-at-a-time rule mechanically — a manifest with two deltas is
  refused, the same typed-refusal posture the rest of the stack uses.
- Parameter-count accounting per the playbook: when a delta changes param
  count (tied/untied, GQA ratios), the manifest must carry the
  compensating adjustment so comparisons stay fair.
- An ablation eval suite in `psionic-eval`, modeled on the playbook's
  criteria (monotonicity, low noise, above-random early, ranking
  consistency): start with cloze-formulation ports of HellaSwag / ARC /
  OpenBookQA / CommonsenseQA-class tasks at ~1k questions each, with
  character-length-normalized log-prob scoring. **Gate zero, before any
  ablation runs:** reproduce published scores for at least one open
  reference model through our harness, exactly the playbook's "validate
  your evaluation suite" rule. That reproduction receipt is the first
  artifact this workstream ships.
- Ablation receipts: config digest, data digest, eval results, loss
  curves, verdict — flowing into the same proof-bundle machinery the
  Tassadar lane uses. The derisking ledger ("current baseline + every
  tested delta + verdict") becomes a committed doc, not tribal memory.

**Monorepo side (owns dispatch + the ledger projection):**

- Ablation runs as a homework kind: small fixed-budget training runs are
  exactly the A3-sweep shape (#4679) — embarrassingly parallel, cheap per
  cell, verifiable by seeded replication. The ablation system and the
  scaling-sweep system are one system with two report formats.
- A public ablation ledger page (extends the leaderboard work, #4683):
  what we tested, what won, what the current baseline is. This is the
  playbook's lab notebook made into a public receipt surface — and it is
  the cheapest "we actually do this" proof the training promises can cite.

## 4. Workstream 2 — The Data Program (beyond the A4 port)

The A4 refinery port (#4680) gives us filters, dedup, and PII masking. The
playbook makes clear that is maybe a third of a data program. The other
two-thirds:

**Corpus acquisition and scale honesty (psionic + monorepo).** Psion's
current corpus is a frozen bounded mixture (`psion_corpus_tokenized@v1`).
A real ladder run needs tokens in the 10B–1T range depending on rung.
Plan: build the refinery as a *pipeline over public crawl-class sources*
(FineWeb-class English web, Stack-class code, FineMath-class math — all
permissively available), processed as paid CPU homework on the long-tail
fleet (WARC extraction, language ID, quality classification, MinHash dedup
— the playbook's own stages, the course's own adapter surface). The
monorepo dispatches shards and records receipts; psionic owns the
deterministic transforms. Every shard carries provenance + transform
digests so the corpus itself is replay-auditable — a corpus with receipts
is also a sellable artifact on the data-market rails (#4643–#4645).

**Mixture design and curriculum (psionic owns, ablation system executes).**
Adopt the playbook's method wholesale:

- Run data ablations at (or near) target model scale, not proxy scale —
  data conclusions don't transfer the way architecture ones do.
- Multi-stage curriculum as the default: abundant/lower-quality early,
  high-quality math/code/reasoning reserved for the decay phase. The
  playbook's annealing-ablation trick (checkpoint at late stage-1, 40%
  baseline + 60% candidate for a bounded token budget) becomes a first-
  class run type in the ablation manifest format.
- Manual mixtures over automated ones (DoReMi-class methods underperformed
  careful ablations in the playbook's hands; we adopt their conclusion
  until our own ablations say otherwise).
- Repetition budgets tracked per source (their ~5-epoch harm threshold as
  the default cap, revisited by ablation).

**Eval-delta payment (monorepo, the novel part).** The course's A4
leaderboard design — train a fixed reference model on contributed data,
pay on measured eval delta — is the data market's pricing mechanism: pay
for measured quality, not volume. This was already sketched in #4680; the
playbook's annealing methodology is exactly how to run it cheaply
(bounded annealing runs from a shared checkpoint instead of full
retrains).

## 5. Workstream 3 — Architecture & Hyperparameter Derisking

Bounded, because the playbook's advice is to *not* be clever here:

- **Baseline:** A1-stack dense decoder, GQA (4-group), tied embeddings,
  SwiGLU, AdamW (β₁ 0.9, β₂ 0.95, wd 0.1, clip 1.0). All already in
  psionic or trivially parameterized.
- **Deltas worth one ablation each, in priority order:** intra-document
  masking (cheap, playbook says crucial for long-context later); no weight
  decay on embeddings (OLMo-2 stability result); NoPE/RoPE hybrid
  (long-context foundation; pairs with doc masking); WSD schedule vs
  cosine (see below); batch-size selection by throughput.
- **WSD (warmup-stable-decay) is strategically important for us, not just
  convenient.** A volunteer network cannot promise a fixed total token
  budget at launch. WSD's property — extend the stable phase indefinitely,
  decay when you decide you're done, match cosine performance — is the
  schedule shape that fits elastic capacity. Adopt as default after one
  confirming ablation; this also keeps every long run *resumable into a
  longer run*, which compounds with the homework-window model.
- **Deferred explicitly:** MoE (load-balancing machinery, all-to-all
  traffic — the worst collective for weak interconnects, per the
  playbook's own measurements), Muon-class optimizers (promising,
  under-recipe'd), hybrid SSM blocks, QK-norm (playbook: hurts
  long-context). Each gets a one-line entry in the derisking ledger with
  the reason, so "we considered it" is on the record.
- **Tassadar hybrid hooks:** the only architecture ask from the exact
  lane is that the weight-bundle/layout formats leave seams for reserved
  exact heads later (`docs/tassadar/` E-phases). Design-time
  consideration, zero pretraining-time cost, no hybrid work before the
  ladder can actually train a host model.

Hyperparameter scaling comes from Workstream 1's machinery: LR/batch
sweeps are ablation cells; fitting the LR-vs-compute power law across the
ladder rungs is the A3 lane (#4679) doing double duty.

## 6. Workstream 4 — The Marathon (operating a long run on a volunteer network)

The playbook's chapter 7 is a field manual for multi-week runs. Translating
its checklist onto our stack produces mostly *monorepo* work, because the
marathon is an orchestration problem:

**Preflight (psionic has most of this; wire it into dispatch).** Psionic's
actual-lane already does hardware qualification (GPU/memory/thermal/ECC
checks), checkpoint/resume drills, and run-shape admission. The playbook
adds: stress-test before launch (they found 2 throttling GPUs; we have
device benchmark homework #4681 — same instrument, paid), automate evals
on every checkpoint (psionic has checkpoint-eval decisions; the monorepo
needs the projection), and config sanity gates.

**Monitoring (monorepo owns the public surface).** The playbook's metric
set — throughput, loss, grad norm, downstream evals vs a reference run,
hardware health — maps onto: per-window throughput receipts, the capacity
funnel's dark-capacity taxonomy (already live), and **public run pages**
(#4677) showing loss curves and checkpoint evals. Their strongest
operational insight transfers directly: *downstream evals against a prior
run's checkpoints caught the TP bug before the loss curve did*. Our
equivalent reference trajectory is each ladder rung's retained eval
series; every new run is monitored against the last rung's curve as a
matter of course.

**Loss-spike and restart policy (psionic decides, monorepo records).**
Psionic's continue/hold/restart checkpoint-decision logic already encodes
the playbook's triage. Add their two playbook moves: batch-skip recovery
(rewind one window, skip the offending shards — windows make this natural)
and a written restart criterion (restart only when the run would plateau
below target; record the decision as a receipt, the way they narrated
their 1T-token restart). Volunteer-network spike sources get named
explicitly: a malicious or numerically-divergent contribution is a *spike
source the playbook doesn't have*, and our defense is the verification
layer (Freivalds/recompute checks reject the contribution before it enters
the merge) plus window-level rewind when something slips through.

**Checkpoint discipline (both sides).** Playbook rule: one local
checkpoint, durable copies offsite, auto-resume always. Psionic has local
+ backup copies and resume drills; the missing piece is **remote durable
checkpoint storage with content-addressed digests** (R2 via the worker, or
operator S3) and the rule that a window is only *sealed* when its
checkpoint digest is durably stored. Checkpoint lineage is already part of
psionic's evidence family; the monorepo's window lifecycle (#4673
plan/activate/seal/reconcile) should carry the checkpoint digest in the
seal record.

**The spare-node strategy, translated.** Their fix for node failure was a
hot spare with the dataset preloaded. Ours is structural: shard-preloaded
standby Pylons are just contributors whose data-feed lease is warm — the
dispatcher (#4639-pattern) keeps N standbys per running window and the
funnel prices their idleness. This is a dispatcher feature, not new
infrastructure.

## 7. Workstream 5 — Post-Training (the program after pretraining)

The playbook's post-training arc is mid-training → SFT → preference
optimization → (optional) RL → merging, with evals and "vibe tests"
throughout. Psionic already holds bounded versions of most stages (legal
SFT/DPO/GRPO CLI lanes, A5 alignment math, #4682); what's missing is the
*sequence* and the data:

1. **Mid-training (continued pretraining):** the playbook's result —
   reasoning-heavy continued pretraining nearly tripled math performance —
   says this stage earns its tokens. Ours doubles as the curriculum's
   stage-2/3 anyway (same machinery, same annealing ablations).
2. **SFT:** extend the legal SFT lane to a general instruct lane: a chat
   template (owned, versioned, with generation-masking — the playbook
   flags template bugs as silent killers), an instruct corpus assembled
   under the same provenance discipline as pretraining data, LR ~10×
   below pretraining. *Hybrid reasoning modes are explicitly deferred* —
   the playbook documents them as hard (paired data, separate reward
   shaping, joint training instability); a 100M–1B Psion has no business
   there yet.
3. **Preference optimization:** DPO first (psionic's Bradley–Terry lane
   exists), APO-class variants by ablation later. Preference *data
   generation is inference, which the network sells*: strong-vs-weak pair
   generation and on-policy grading are rollout homework — the same kind
   5050-class work A5 already defines.
4. **RL/RLVR:** GRPO with verifier rewards is the network-native choice —
   rollouts are paid inference homework, reward checking is deterministic
   CPU homework, and only the update step needs the training boundary
   (#4682's framing, confirmed by the playbook). One playbook warning
   adopted now: length-reward hacking is real; bring their overlong-
   completion penalty into the GRPO lane's reward shaping from day one.
   On-policy distillation (their "GRPO quality at 1/10 compute" note) is
   the cheap alternative to evaluate once any stronger teacher checkpoint
   exists in-family.
5. **Vibe testing as a stage gate:** the playbook caught a corpus bug via
   manual personas that evals missed. Institutionalize: every
   post-training closeout includes a small structured manual-interaction
   transcript as an artifact, reviewed before any promise transition.

Post-training evals extend the Workstream-1 suite with IFEval-class
instruction following, GSM8K/MATH-class math, and decontamination
(n-gram) receipts against everything we train on — decontamination is a
*data-program* deliverable that post-training consumes.

## 8. Workstream 6 — Infrastructure Truth (measure, don't assume)

The playbook's infrastructure chapter is one long argument for measuring
your actual hardware instead of trusting specs (their H100s delivered
72–77% of peak in kernels and ~30% MFU end-to-end). Our fleet is radically
more heterogeneous, which makes measurement *more* load-bearing, and we
already decided to sell it as a product:

- **Device-capability dataset (#4681) is the roofline program.** Benchmark
  homework measures per-device matmul throughput, memory bandwidth,
  attention-kernel performance, and sustained-vs-burst thermals
  (playbook: one throttling GPU collapsed a 14-node collective — on a
  volunteer network, throttle detection must be continuous, and it
  already has a home in the funnel's reason codes).
- **Collective reality:** their measurements (all-reduce scales, all-to-all
  collapses across nodes) transfer as design rules: DiLoCo-class
  infrequent synchronization is not a compromise for us, it is the only
  shape that fits internet-grade links; anything all-to-all-shaped (MoE
  expert parallel) stays deferred; `psionic-collectives`' quantized
  collective benchmarking becomes part of the capability dataset.
- **MFU honesty in public copy:** every public throughput claim states
  effective tokens/sec including communication and verification overhead
  — the 2.74 tok/s tri-host number is the template: unflattering and
  exact. Verification overhead is itself a measured, published number
  (the cost of trust is part of our cost model, and pricing it is the
  thesis).
- **Numerics policy:** the playbook trains BF16 and never FP16; our
  cross-device gradient work has a harder problem (Metal vs CUDA vs CPU
  non-determinism). Psionic's scalar-f32/SIMD-profile discipline already
  exists for the exact lane; the training lane needs a stated *tolerance
  contract* per verification class instead — Freivalds checks over
  committed matrices absorb backend numerics differences by checking in a
  field, which is precisely why that class (not bitwise comparison) is
  the training-work default (#4674).

## 9. The Model Ladder (sizing the program honestly)

The playbook's cadence lesson: teams that train every 2–3 months compound;
teams that train yearly don't. The ladder gives us that cadence, each rung
a full pipeline rehearsal at a scale we can actually complete:

| Rung | Model | Tokens (order) | Hardware reality | What it proves |
|---|---|---|---|---|
| R0 (exists) | tri-host bringup | ~4k | 2 Macs + 1 RTX 4080 | dispatch/checkpoint/receipt mechanics (done) |
| R1 | ~30–50M Psion | 1–5B | operator-owned devices, days | full pipeline end-to-end: data→ablations→marathon→SFT→evals, all receipted |
| R2 | ~125–200M Psion | 10–50B | operator + early contributor Pylons | first *network* pretraining with paid verified windows (the `pylon.first_real_model_training_run.v1` target, run honestly) |
| R3 | ~1B Psion | 100B+ | depends on R2's measured economics | first generally-useful model; post-training arc in anger |
| R4 | ~3B class | 1T+ | only if R3 economics close | smol-playbook scale; not promised, priced by receipts |

Rules of the ladder: no rung starts before the previous rung's closeout
receipt; every rung re-runs the *whole* pipeline (the rehearsal is the
point); scaling-law fits from each rung (A3 machinery) size the next; and
the registry promise for each rung is written *before* the run with
safeCopy/unsafeCopy bounds, transitioning only on receipts — the same
discipline as `compute.tassadar_executor_poc.v1`.

## 10. Verification Map (every stage gets a class)

Extending the CS336 audit's table to the full pipeline (#4674's pluggable
classes):

| Pipeline stage | Work | Verification class |
|---|---|---|
| Data refinery | extraction/filter/dedup shards | deterministic_recompute (digest spot-checks) |
| Tokenizer | BPE train/encode | deterministic_recompute |
| Ablation cells / A3 sweeps | small training runs | seeded_replication (sampled cells) |
| Pretraining windows | gradient/matrix work | freivalds_merkle (committed matrices) |
| Checkpoint eval | benchmark scoring | deterministic_recompute |
| Device benchmarks | throughput probes | statistical_cross_check (same-class devices) |
| Rollout generation (PO/RL) | inference | seeded_replication + reward-distribution checks |
| Reward grading | verifier scoring | deterministic_recompute |
| Executor homework | Tassadar traces | exact_trace_replay (#4684) |

The aggregate-only-validation compromise from the April era is re-decided
per class here, in writing, when #4674 lands — not inherited silently.

## 11. Sequencing

Phases, each gated by receipts, layered onto the existing issue set:

1. **Foundations (now → rails landed):** #4673 (run/window authority),
   #4674 (verification classes), #4675 (A1 homework), #4677 (public run
   pages) — unchanged from the CS336 audit, they are this plan's
   skeleton. Psionic side: ablation harness + eval-suite reproduction
   gate (§3, gate zero).
2. **R1 rung (operator-scale full rehearsal):** corpus v2 through the
   refinery; 3–5 architecture ablations off the priority list; WSD
   confirmation; one marathon with public run page, checkpoint-seal
   discipline, and a restart-policy receipt; SFT + DPO close the arc.
   Everything dispatched through the rails even when all devices are
   operator-owned — dispatch *is* the rehearsal.
3. **R2 rung (network pretraining):** contributor windows with
   freivalds_merkle verification and paid closeouts (#4676 weak-device
   validator lane live); A3 sweeps (#4679) and A4 data homework (#4680)
   running as standing work classes; device-capability dataset (#4681)
   public. This rung is the honest green path for
   `pylon.first_real_model_training_run.v1`.
4. **R3 rung + post-training in anger:** instruct Psion with the full
   post-training arc (#4682 generalized beyond legal), preference-data
   and rollout homework as standing inference work, leaderboards (#4683)
   spanning every stage.
5. **Continuous:** Tassadar exact-replay homework (#4684) runs alongside
   from the moment #4674 lands — it needs no training capability and is
   the always-available, verification-perfect floor workload; the
   `compute.tassadar_executor_poc.v1` promise resolves on its own track.

Proposed new issues (to file after review, monorepo side unless noted):
ablation-ledger projection + homework kind; durable checkpoint-seal
storage in the window lifecycle; corpus-provenance receipts on the data
rails; decontamination receipts; standby-Pylon dispatcher feature;
vibe-test artifact in post-training closeouts. Psionic side (its tracker):
ablation manifest/harness, eval-suite reproduction gate, WSD schedule,
intra-document masking + embedding-weight-decay deltas, chat template +
instruct SFT lane, GRPO length-penalty reward shaping, remote checkpoint
backup target.

## 12. What This Plan Does Not Claim

- No rung above R1 is scheduled against a date; rungs are gated by the
  previous rung's receipts and measured economics, not by ambition.
- Nothing here moves any registry promise. `pylon.first_real_model_training_run.v1`
  and `pylon.compute_revenue_modes.v1` stay red until the R2 evidence
  exists; new rung promises are written before their runs and transition
  only on receipts.
- The smol playbook's numbers (MFU, bandwidths, mixture ratios) are
  *their* measurements on *their* hardware; we cite them as priors to be
  replaced by our own receipts, never as our claims.
- Tassadar/hybrid work stays behind its own disclosure flow; this plan
  touches it only at the bundle-format seam and the #4684 homework class.

## Sources

- `psionic/docs/smol/` chapters 1–10 (psionic `ee915fcb`), from
  `the-smol-training-playbook-the-secrets-to-building-world-class-llms.pdf`
- `docs/2026-06-10-cs336-distributed-homework-continuation-audit.md` and
  issues #4664–#4671, #4673–#4684
- `docs/tassadar/README.md`, `work-that-proves-itself.md`,
  `2026-06-10-tassadar-percepta-audit.md`
- `psionic/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md`, `TRAIN_SYSTEM.md`,
  CS336 port matrices, `psionic-train`/`-data`/`-distributed`/
  `-collectives`/`-eval` module surfaces
- Product-promises registry `2026-06-10.7`
  (`apps/openagents.com/workers/api/src/product-promises.ts`)
