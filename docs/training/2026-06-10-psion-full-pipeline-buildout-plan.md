# Psion Full Training-Pipeline Buildout Plan

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-10 (revised same day with the business frame, §3)

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
run as paid, verified work. ("Homework" was always a tongue-in-cheek CS336
reference — we are doing the course's homework — and it stays scoped to those
CS336 lanes. Everywhere else this plan calls the dispatched units what they
are: real paid work.) What it does not give us is the
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
   be wired into every stage, not just committed training matrices.

One structural advantage is worth stating once, without hype: the playbook
team needed a fixed 384-H100 Slurm reservation and a spare node. Our
architecture is built around the opposite assumption — elastic, untrusted,
heterogeneous capacity with verification instead of trust. Where the
playbook says "reserve a spare node with the dataset preloaded," we say
"the funnel reason-codes dark capacity and the dispatcher routes around
it." That substitution — receipts for reservations — is the whole company
thesis applied to training. Section 3 spells out that thesis and what the
pipeline is *for* in business terms; the workstreams after it are the
engineering.

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
  ladder in §10), multilingual-optional, math/code-leaning, trained
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
deviation goes through the derisking ladder in §5.

## 3. The Business Frame: What The Pipeline Is For

The engineering sections that follow make more sense with the strategic
context stated once, plainly. None of this is new doctrine — it is the
through-line of the public episode corpus (`docs/transcripts/` 178, 201,
202, 216, 220, 224, 228, 230, 234), the Tassadar essays
(`docs/tassadar/`), and the promises registry itself — applied to the
question "why operate a training pipeline at all."

### 3.1 The unit of account

The economic history this company is betting on runs: **hash → token →
accepted outcome**. Bitcoin priced the hash — pure, useless, perfectly
verifiable work, sellable from anywhere there is power and a thin
connection. The AI era priced the token — statistically useful, sold by
the million, and almost completely unverified. The unit we are building
for is the accepted outcome: work defined in advance, executed wherever
execution is cheapest, verified against a rubric, recorded in a receipt,
settled to everyone who contributed. The north-star metric that prices
the whole conversion chain — energy, compute, model, orchestration,
verification, settlement — is **accepted outcomes per kilowatt-hour**.

The training pipeline is that thesis pointed at ourselves. Every stage in
§4–§8 is deliberately shaped as accepted outcomes: a filtered data shard,
an ablation cell, a sealed training window, a checkpoint eval, a graded
rollout — each defined in advance, executed on heterogeneous supply,
verified by a named class (§11), receipted, and paid. The pipeline is
simultaneously a model factory and the **demand engine for the network**:
it is the standing, internally-generated workload that gives contributor
machines real work to clear while external demand is still being
discovered. That dual role is why it deserves investment beyond what any
single model checkpoint would justify.

### 3.2 Supply: compute fracking, and why the pipeline runs edge-first

The supply thesis (episode 201) is that enormous compute already exists
but is *stranded* — missing discovery, packaging, trust, settlement, or
operability, not missing silicon. The named formations run from ~5.5 GW
of installed Apple Silicon and the world's sleeping gaming PCs, through
idle prepaid AI-subscription capacity, prosumer boxes, and miner-adjacent
sites, up to underused institutional capacity. "Fracking" that supply
means injecting the missing plumbing: streaming money, receipts, routing,
reputation.

The training pipeline is the work-class generator that makes the fracking
real, and its stages map onto the supply funnel by opportunity cost:

- **Zero-floor edge devices** (the machines that cannot do meaningful
  gradient descent) get the CPU-bound stages: A4 data refinery shards,
  reward grading, deterministic verification, validator work, and —
  uniquely — Tassadar exact-replay auditing, where the weakest machine is
  a fully competent auditor of the most exact computation in the system.
- **Mid-funnel consumer GPUs and Apple Silicon** get ablation cells, A3
  sweep cells, rollout generation, and (as the ladder climbs) real
  pretraining windows.
- **Always-on, higher-duty-cycle capacity** (miner-adjacent compute
  islands, dedicated operator hardware) anchors the long marathon runs
  that need continuity.

One economic caveat from the corpus disciplines all of it: electricity
arbitrage alone is nearly worthless when job revenue dwarfs power cost.
The margin must come from converting capacity *nobody else can monetize
at all* into work that clears — which is exactly why the funnel runs
edge-first, where the opportunity floor is genuinely zero and paying
slightly more than zero buys access to everything.

### 3.3 The capital point: good-enough hardware and latency tolerance

The demand-side taxonomy (the inference-shift argument) splits the market
into training (cathedral workload, not contested here at frontier scale),
answer inference (human waiting; token speed is the product; bounded by
human attention), and **agentic/batch work** — machines doing tasks over
time, where latency can be traded for capacity and cost. The market
already prices this: frontier labs sell batch APIs at ~50% discounts for
24-hour turnaround.

Training work is the limiting case of latency-tolerant work, and that
has a capital-structure consequence the corpus has begun to quantify:
when revenue is latency-tolerant, the *optimal hardware point shifts
down*. Modeled facility economics show consumer-tier GPUs reaching
payback several times faster than frontier datacenter parts on
latency-tolerant token work — because the two-year depreciation clock on
frontier hardware is the dominant cost, and cheap hardware amortizes
against revenue that frontier hardware cannot proportionally exceed.
(Modeled, not measured; the registry's energy promise
`energy.flexible_load_proof.v1` stays planned until operator proof
exists.) The pipeline's design choices — DiLoCo-class infrequent sync,
windows as the unit of progress, checkpoint-on-interrupt, WSD schedules
that tolerate elastic capacity — are all downstream of this one fact:
**we design the workload for the supply, instead of buying the supply the
workload assumes.** That is also the strategic answer to "why not rent a
cluster": a rented cluster would produce a model; it would not produce a
network that knows how to train one.

### 3.4 Verification economics: the cost stack this pipeline attacks

The honest cost model for accepted outcomes says the dominant term is
never compute — it is checking: review, grading, retries, the human
minutes spent deciding whether the thing produced is the thing asked for.
The company's standing strategy against that term has six compounding
moves, and the training pipeline implements or feeds every one:

1. **Amortize** — solve once, reuse forever. Every verified pipeline
   artifact (a deduped corpus, a fitted scaling law, an ablation verdict,
   a trained grader) is paid for once and amortized across all future
   runs. The ablation ledger (§4) is market memory for training
   decisions.
2. **Decompose** — small units are cheap to verify. The pipeline is
   already decomposed by construction: shards, cells, windows, rollouts.
   This is why training work can be verified at ~1000× less than the cost
   of redoing it.
3. **Downshift** — route verification to zero-floor supply. Validator
   work as paid assignments for weak devices (#4676) migrates dollars of
   review into cents of replay.
4. **Price** — classify outcomes by how cheaply and how soon correctness
   can be known, and route to the cheapest sufficient effort rung.
   Training work classes are the best-behaved in the catalog: most are
   deterministic or probabilistically checkable, which is why they clear
   at the bottom of the effort ladder.
5. **Incentivize** — pay whoever cheapens verification. Grader authors
   and verifier improvements earn from the margin they create. The
   pipeline supplies both the training data for graders (its own traces)
   and the compute to train them.
6. **Eliminate** — the Tassadar limit case: work whose execution trace
   *is* the receipt, verification cost structurally zero. Research-gated,
   one scoped PoC promise (`compute.tassadar_executor_poc.v1`), and the
   asymptote the other five moves bend toward.

The metric that decides whether this strategy works is **review cost per
accepted outcome trending down while work value trends up**. The training
pipeline is where that curve is easiest to bend first, because its work
classes start cheap to verify — which makes it the proving ground for the
verification economy the rest of the product needs.

### 3.5 Energy: training as dispatchable load

The energy corpus (episodes and the public registry's energy promise)
establishes that grids can absorb tens of gigawatts of new load *if the
load can credibly curtail during rare stress hours* — and that mining
proved the operating model: interruptible, mobile, indifferent to where
its buyer lives. The strategic question is what else can run that way.

Training work on this pipeline is built to qualify. A sealed-window
architecture with checkpoint-on-interrupt is curtailment-compatible by
construction: stop the window, the prefix is the state, resume anywhere.
The dispatch rule for any compatible watt — run accepted work if it beats
the floor; else mine if positive; else curtail if grid value beats both;
else idle — slots training windows in as a second floor above idle for
CPU-rich, GPU-poor fleets: always available (we are our own first buyer
of conformance runs, sweeps, and verification work), always
verifiable, therefore always payable without trust. Floors are not for
getting rich per hour; they are for keeping a fleet alive, enrolled, and
warm between higher-margin assignments — and for generating the
continuous receipt stream that makes the fleet underwritable. The
long-run artifact this enables is the one the energy promise names: a
live curtailment event handled cleanly — load shed on schedule, windows
checkpointed and resumed, receipt published. The pipeline should be
engineered so that artifact is producible on demand.

### 3.6 Demand honesty: who buys, and the first-buyer rule

The standing discipline: internal demand proves plumbing, not markets.
**No external dollar, no demand claim.** Applied here:

- The pipeline's internal demand (our own ablations, sweeps, corpus work,
  grader training) is real work with real payouts, and it is labeled as
  plumbing proof in every public surface. It solves the cold-start
  problem that killed the 2023 provider-market attempt ("we were the only
  buyer") without pretending to be a market.
- The pipeline's *sellable outputs* are where external demand can attach,
  and they are more numerous than "a model": receipted datasets and
  data-quality work (the data market, #4643–#4645); fitted scaling-law
  reports and the device-capability dataset (#4681) as public-good
  artifacts that build buyer trust; small specialized models served
  cheaply on Psionic (the episode-220 thesis: continually-improved,
  laser-focused small models on retail compute, fed by paid contributor
  data); fine-tuning and post-training as a service on the same rails;
  and eventually digest-pinned weight modules verified by replay before
  purchase clears.
- Every revenue-bearing claim carries provenance: internal vs external,
  modeled vs measured vs settled. The registry already enforces this
  shape; the pipeline inherits it.

### 3.7 The moat question, stated honestly

Incumbents have internalized the heterogeneous-compute insight; what they
have not built — and structurally cannot build without disintermediating
their own take rate — is an **open market** where placement, payment, and
verification do not route through the operator. The network-economics
argument (episode 230): group-forming networks scale with possible
subgroups (2^n), which has always been theoretical for humans because of
cognitive limits — but agents are not Dunbar-limited. An open protocol
network is the only topology where agent-to-agent subgroups can form,
transact, and settle without the platform's permission; a closed fleet
captures pairwise value at best.

For the training pipeline specifically, the compounding asset is **market
memory**: acceptance records, route scorecards across devices, grader
calibration histories, per-window economics, the derisking ledger. A
closed lab re-derives this privately and amortizes it across one fleet;
an open network that *pays its trainers and validators* accumulates it as
a shared, receipt-backed asset that any participant can build on. That is
a future moat, not a present one — it exists only when the receipts
accumulate — which is precisely why the plan's sequencing (§12) front-
loads the receipt-generating rails over model scale.

### 3.8 Opportunities unlocked, and the macro posture

Concrete option value the pipeline creates beyond its rungs, in rough
order of nearness: the public ablation/scaling-law artifacts (cheap,
trust-building, no one else's incentive to publish); the data-market
quality machinery (A4 stages are the same machinery a redacted trace
bundle needs before sale); the device-capability dataset across
heterogeneous consumer hardware (a dataset no lab has); specialized small
models as products; verification-as-a-service (graders and validator
quorums priced per check); training/fine-tuning as a service on owned
rails; and the research lanes — verified distributed training at
internet-grade trust, and the Tassadar hybrid (exact cores inside trained
hosts), both of which require exactly the architectural sovereignty this
pipeline builds.

The macro posture is the same one the company holds generally: built so
that **either AI-capex future is survivable**. If centralized capex keeps
compounding, the pipeline rides the cost-down curve of good-enough
hardware and sells specialized work the cathedrals don't price. If the
financing bubble pops, the bust produces this system's inputs —
distressed compute needing utilization, burned buyers demanding outcome
pricing and receipts, operators needing a second lane — and a training
network that pays for verified work becomes the salvage market. Capacity
is financed from demonstrated margin, never ahead of it; no modeled
economics presented as proven; no accepted work, no revenue claim.

### 3.9 Business-level falsifiers

The plan should be falsifiable in business terms, not just engineering
terms. What would tell us the strategy (not merely a rung) is failing:

1. External buyers refusing outcome-priced training/data/model work in
   favor of commodity alternatives, after R2-scale receipts exist.
2. Verification cost per accepted training outcome staying flat as work
   value rises — the six moves failing to bend the curve in the easiest
   domain they will ever get.
3. Contributor retention failing at unsubsidized payout levels — the
   supply thesis holds only if wildcatters stay once novelty fades.
4. Per-rung economics gates (§10) failing twice at the same rung — the
   ladder stalling is information, and the honest response is recorded in
   the registry, not papered over.

## 4. Workstream 1 — The Ablation System (the missing organ)

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

- Ablation runs as a dispatched work kind: small fixed-budget training runs are
  exactly the A3-sweep shape (#4679) — embarrassingly parallel, cheap per
  cell, verifiable by seeded replication. The ablation system and the
  scaling-sweep system are one system with two report formats.
- A public ablation ledger page (extends the leaderboard work, #4683):
  what we tested, what won, what the current baseline is. This is the
  playbook's lab notebook made into a public receipt surface — and it is
  the cheapest "we actually do this" proof the training promises can cite.
  Per §3.8, it is also a public-good artifact in its own right.

## 5. Workstream 2 — The Data Program (beyond the A4 port)

The A4 refinery port (#4680) gives us filters, dedup, and PII masking. The
playbook makes clear that is maybe a third of a data program. The other
two-thirds:

**Corpus acquisition and scale honesty (psionic + monorepo).** Psion's
current corpus is a frozen bounded mixture (`psion_corpus_tokenized@v1`).
A real ladder run needs tokens in the 10B–1T range depending on rung.
Plan: build the refinery as a *pipeline over public crawl-class sources*
(FineWeb-class English web, Stack-class code, FineMath-class math — all
permissively available), processed as paid CPU work on the long-tail
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
retrains). This is move 5 from §3.4 in miniature: contributors earn from
the measured margin their data creates.

## 6. Workstream 3 — Architecture & Hyperparameter Derisking

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
  longer run*, which compounds with the window-based dispatch model.
- **Deferred explicitly:** MoE (load-balancing machinery, all-to-all
  traffic — the worst collective for weak interconnects, per the
  playbook's own measurements), Muon-class optimizers (promising,
  under-recipe'd), hybrid SSM blocks, QK-norm (playbook: hurts
  long-context). Each gets a one-line entry in the derisking ledger with
  the reason, so "we considered it" is on the record. Four QVAC-derived
  entries join the ledger via psionic#1118 (Vulkan backend, dynamic
  tiling, KV-cache quantization, BitNet-b1.58 QAT — the last enters only
  through the ablation manifest, at R2+; see the QVAC analysis in this
  folder).
- **Tassadar hybrid hooks:** the only architecture ask from the exact
  lane is that the weight-bundle/layout formats leave seams for reserved
  exact heads later (`docs/tassadar/` E-phases). Design-time
  consideration, zero pretraining-time cost, no hybrid work before the
  ladder can actually train a host model.

Hyperparameter scaling comes from Workstream 1's machinery: LR/batch
sweeps are ablation cells; fitting the LR-vs-compute power law across the
ladder rungs is the A3 lane (#4679) doing double duty.

## 7. Workstream 4 — The Marathon (operating a long run on a volunteer network)

The playbook's chapter 7 is a field manual for multi-week runs. Translating
its checklist onto our stack produces mostly *monorepo* work, because the
marathon is an orchestration problem:

**Preflight (psionic has most of this; wire it into dispatch).** Psionic's
actual-lane already does hardware qualification (GPU/memory/thermal/ECC
checks), checkpoint/resume drills, and run-shape admission. The playbook
adds: stress-test before launch (they found 2 throttling GPUs; we have
device benchmark assignments, #4681 — same instrument, paid), automate evals
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

**Curtailment as a feature, not a failure (per §3.5).** The same
machinery that survives volunteer churn — checkpoint-on-interrupt, window
rewind, standby promotion — is what makes training load *dispatchable* in
the energy sense. The marathon workstream should treat "shed N% of the
fleet on schedule and resume cleanly, with receipts" as a drill it can
run, because that drill is the future evidence for the registry's energy
promise.

## 8. Workstream 5 — Post-Training (the program after pretraining)

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
   below pretraining. Filed as psionic#1117, with QVAC's
   `llama-finetune-lora` (masked loss, checkpoint/resume, schedulers,
   proven on consumer/mobile GPUs) as the read-only external reference.
   *Hybrid reasoning modes are explicitly deferred* —
   the playbook documents them as hard (paired data, separate reward
   shaping, joint training instability); a 100M–1B Psion has no business
   there yet.
3. **Preference optimization:** DPO first (psionic's Bradley–Terry lane
   exists), APO-class variants by ablation later. Preference *data
   generation is inference, which the network sells*: strong-vs-weak pair
   generation and on-policy grading are rollout work — the same kind
   5050-class work A5 already defines.
4. **RL/RLVR:** GRPO with verifier rewards is the network-native choice —
   rollouts are paid inference work, reward checking is deterministic
   CPU work, and only the update step needs the training boundary
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

Strategically (§3.6, §3.8), post-training is the stage closest to
external revenue: specialized small models, fine-tuning as a service, and
grader training are all post-training products, and they reuse the same
rails the pretraining rungs build.

## 9. Workstream 6 — Infrastructure Truth (measure, don't assume)

The playbook's infrastructure chapter is one long argument for measuring
your actual hardware instead of trusting specs (their H100s delivered
72–77% of peak in kernels and ~30% MFU end-to-end). Our fleet is radically
more heterogeneous, which makes measurement *more* load-bearing, and we
already decided to sell it as a product:

- **Device-capability dataset (#4681) is the roofline program.** Benchmark
  assignments measure per-device matmul throughput, memory bandwidth,
  attention-kernel performance, and sustained-vs-burst thermals
  (playbook: one throttling GPU collapsed a 14-node collective — on a
  volunteer network, throttle detection must be continuous, and it
  already has a home in the funnel's reason codes). Per §3.3, this
  dataset is also how the network answers "what can my machine earn?"
  honestly, and per §3.8 it is a public asset no lab has an incentive to
  build.
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
  the training-work default (#4674). A second path opened by the QVAC
  review: integer/ternary serving formats can make *inference* outputs
  bit-reproducible across heterogeneous backends (Tether's BitNet lane
  demonstrates this on Vulkan-vs-CPU), which would let quantized
  inference work ride deterministic_recompute instead of statistical
  checks — psionic#1115 ports the formats with parity fixtures and
  determinism receipts, and psionic#1116 gives every seeded work class a
  counter-based RNG with the same cross-device property.

## 10. The Model Ladder (sizing the program honestly)

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

Each rung also carries an **economics gate**, not just an engineering
gate, per §3: measured cost per accepted training outcome (all-in,
including verification and settlement overhead), contributor payout per
device-hour against the relevant opportunity floor, and verification
overhead as a fraction of work cost — published with provenance labels
(modeled vs measured vs settled). R2's gate is the important one: it is
the first time "the network trains a model" must clear against "we rent a
small cluster," and the comparison is run honestly against that fallback
comparator, not against a vacuum. A rung whose economics fail twice is
information (§3.9), and the honest response is recorded in the registry.

## 11. Verification Map (every stage gets a class)

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
| Executor work | Tassadar traces | exact_trace_replay (#4684) |

The aggregate-only-validation compromise from the April era is re-decided
per class here, in writing, when #4674 lands — not inherited silently.

This table is §3.4 made operational: the rows near the bottom of the
effort ladder (deterministic, replayable) are routed to the cheapest
supply, and the one row with structurally zero verification cost
(exact_trace_replay) is the floor the rest of the system is being bent
toward. Two QVAC-derived upgrades are in flight for this table: a
ternary serving lane with determinism receipts (psionic#1115) would move
quantized-inference rows from seeded_replication down to
deterministic_recompute, and the Philox port (psionic#1116) hardens
every seeded row by making seed → output reproducible across device
classes by construction.

## 12. Sequencing — The Unified Roadmap

This section is the single roadmap view. It binds four layers that are
maintained in different places: **phases** (below) gate on receipts;
**promises** (registry `2026-06-10.10`: the red tier-1 claims, the nine
planned program records, the yellow Tassadar PoC) are the public claim
ledger for each phase; **issues** are the work units (monorepo
#4673–#4684 for rails and lanes; psionic #1115–#1118 for the QVAC-derived
ports, plus the psionic-side asks below); and the **velocity forecast**
(`../promises/2026-06-10-green-velocity-extrapolation.md`) is the
modeled calendar, re-measured weekly. External references (the smol
playbook in `psionic/docs/smol/`, the QVAC analysis in this folder) feed
workstreams through issues — never directly into promises. One explicit
scope decision is recorded here so the roadmap stays unified: **image
generation is out of scope at this time** (owner decision 2026-06-10; at
least not locally via the QVAC-class infrastructure), so no image/video
work kind appears in any phase.

Phases, each gated by receipts, layered onto the existing issue set:

1. **Foundations (now → rails landed):** #4673 (run/window authority),
   #4674 (verification classes), #4675 (A1 homework), #4677 (public run
   pages) — unchanged from the CS336 audit, they are this plan's
   skeleton. Psionic side: ablation harness + eval-suite reproduction
   gate (§4, gate zero).
2. **R1 rung (operator-scale full rehearsal):** corpus v2 through the
   refinery; 3–5 architecture ablations off the priority list; WSD
   confirmation; one marathon with public run page, checkpoint-seal
   discipline, and a restart-policy receipt; SFT + DPO close the arc.
   Everything dispatched through the rails even when all devices are
   operator-owned — dispatch *is* the rehearsal.
3. **R2 rung (network pretraining):** contributor windows with
   freivalds_merkle verification and paid closeouts (#4676 weak-device
   validator lane live); A3 sweeps (#4679) and A4 data-refinery work (#4680)
   running as standing work classes; device-capability dataset (#4681)
   public. This rung is the honest green path for
   `pylon.first_real_model_training_run.v1`, and the first rung whose
   economics gate compares network training against a rented-cluster
   fallback.
4. **R3 rung + post-training in anger:** instruct Psion with the full
   post-training arc (#4682 generalized beyond legal), preference-data
   and rollout generation as standing inference work, leaderboards (#4683)
   spanning every stage — and the first rung where external-facing
   products (specialized models, fine-tuning, grader services) are
   realistic, under the no-external-dollar-no-demand-claim rule.
5. **Continuous:** Tassadar exact-replay work (#4684) runs alongside
   from the moment #4674 lands — it needs no training capability and is
   the always-available, verification-perfect floor workload; the
   `compute.tassadar_executor_poc.v1` promise resolves on its own track.

**Filed (2026-06-10):** monorepo rails and lanes #4673–#4684 (from the
CS336 audit); psionic #1115 (ternary determinism receipts), #1116
(Philox seeded-work RNG), #1117 (instruct SFT lane — covers the "chat
template + instruct SFT lane" ask below), #1118 (QVAC derisking-ledger
entries); device-taxonomy and external-comparator spec input recorded on
#4681.

**Still proposed (to file after review, monorepo side unless noted):**
ablation-ledger projection + dispatched work kind; durable checkpoint-seal
storage in the window lifecycle; corpus-provenance receipts on the data
rails; decontamination receipts; standby-Pylon dispatcher feature;
vibe-test artifact in post-training closeouts; per-rung economics-gate
report format with provenance labels; the scheduled curtailment drill
(§7). Psionic side (its tracker): ablation manifest/harness, eval-suite
reproduction gate, WSD schedule, intra-document masking +
embedding-weight-decay deltas, GRPO length-penalty reward shaping, remote
checkpoint backup target.

## 13. What This Plan Does Not Claim

- No rung above R1 is scheduled against a date; rungs are gated by the
  previous rung's receipts and measured economics, not by ambition.
- Nothing here moves any registry promise. `pylon.first_real_model_training_run.v1`
  and `pylon.compute_revenue_modes.v1` stay red until the R2 evidence
  exists; new rung promises are written before their runs and transition
  only on receipts.
- Internal demand is plumbing proof, not market proof. No external
  dollar, no demand claim — and every revenue-bearing number this program
  publishes carries internal/external and modeled/measured/settled
  provenance.
- The hardware-economics arguments in §3.3 are modeled directional
  results, not measured operator proof; the registry's energy promise
  stays planned until the live evidence exists.
- The smol playbook's numbers (MFU, bandwidths, mixture ratios) are
  *their* measurements on *their* hardware; we cite them as priors to be
  replaced by our own receipts, never as our claims.
- Tassadar/hybrid work stays behind its own disclosure flow; this plan
  touches it only at the bundle-format seam and the #4684 work class.
- The **decentralized-optimizer / public-gradient lane** is not specified
  here. Public devices in this plan do generation, validation, and
  evaluation only; letting public Pylons contribute model updates that can
  advance a shared checkpoint (accepted training windows, quarantine
  optimizer, gradient verification ladder, canary-gated promotion, staged
  payout) is its own workstream — **W5** in
  [`../tassadar/RESEARCH_PLAN.md`](../tassadar/RESEARCH_PLAN.md) §5, gated
  behind a W3 student checkpoint and the #4855 lifecycle substrate, owing
  `training.public_gradient_windows.v1` (red/planned) when it produces
  evidence. No public gradient enters the canonical optimizer until it
  passes quarantine, verification, canary, and promotion gates.

## Sources

- `psionic/docs/smol/` chapters 1–10 (psionic `ee915fcb`), from
  `the-smol-training-playbook-the-secrets-to-building-world-class-llms.pdf`
- `docs/2026-06-10-cs336-distributed-homework-continuation-audit.md` and
  issues #4664–#4671, #4673–#4684
- `docs/tassadar/README.md`, `work-that-proves-itself.md`,
  `2026-06-10-tassadar-percepta-audit.md`
- `docs/training/2026-06-10-qvac-edge-stack-analysis.md` (external
  reference absorption; psionic #1115–#1118)
- Episode transcripts (`docs/transcripts/`): 178 (Swarm Inference), 201
  (Fracking Apple Silicon), 202 (Recursive Language Models), 216, 220,
  224 (Distributed Training 101), 228 (Free Autopilot), 230 (Calling All
  Agents), 234 (Product Promises)
- `psionic/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md`, `TRAIN_SYSTEM.md`,
  CS336 port matrices, `psionic-train`/`-data`/`-distributed`/
  `-collectives`/`-eval` module surfaces
- Product-promises registry `2026-06-10.7`
  (`apps/openagents.com/workers/api/src/product-promises.ts`), including
  `energy.flexible_load_proof.v1` and `compute.tassadar_executor_poc.v1`
- Internal strategy syntheses in the root workspace (energy/compute
  economics, verification-cost strategy, network-economics framing),
  represented here only through their public-corpus sources above
