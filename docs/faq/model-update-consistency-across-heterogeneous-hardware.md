# How does the network keep model updates consistent across heterogeneous Pylon hardware?

*Date: 2026-06-16. First entry in the OpenAgents FAQ. Answers a question we got on
Twitter/X about the Tassadar training run launched in Episode 237.*

## The short answer

We don't *assume* consistency across hardware — we **verify** it, one contribution
at a time, and we only fold a model update into the network's weights after it has
**cleared verification**. Three things make that work:

1. **A deterministic executor.** The unit of training work is a digest-pinned,
   reproducible computation, not a free-floating gradient. The same input produces
   the same output digest on any machine that runs it honestly.
2. **Verification by exact replay on a *different* device.** Every contribution is
   re-executed on a separate validator node and the result digests are compared. A
   contribution that isn't reproducible across hardware doesn't match, and a
   non-matching contribution is never merged.
3. **A heterogeneity-tolerant merge.** Updates are robustly aggregated and
   staleness-aware, so a slow old GPU, a fast new one, and a CPU node can all
   contribute to the same run without any one of them corrupting the shared model.

The rest of this essay explains each, and is honest about what is built versus what
is still experimental — because the whole point of OpenAgents is that a claim you
can't verify isn't worth making.

## Why heterogeneity is the hard part (and why we leaned into it)

Pylon — our node software — is designed to run on *anything*. Episode 237 put it
plainly: a brand-new node, "even one on an old GPU," can come online and start
contributing. Pylon packages **Psionic**, our from-scratch Rust ML framework, so any
node can do inference, embeddings, and distributed training. That is a feature, not
an accident: the network gets cheaper and stronger the more different machines plug
in.

But heterogeneous hardware is exactly where naive distributed training breaks.
Different GPUs, CPUs, drivers, and math libraries produce *slightly* different
floating-point results for the same operation. Different machines run at different
speeds, so their updates arrive stale. And in an open network, you cannot assume
every node is even honest. Any one of those — numerical drift, staleness, or a bad
actor — can silently poison a shared model. So consistency cannot be a hope; it has
to be a property the protocol *enforces*.

## Pillar 1 — A deterministic executor, so the same work is reproducible anywhere

Tassadar is building an **"executor" class of model** on Percepta's "LLMs as
Computers" architecture: *deterministic, CPU-style computation folded into the
weights*, running inside Psionic. That word — deterministic — is load-bearing.

Training and verification operate on **digest-pinned workloads**. The executor
(`packages/tassadar-executor`) runs a pinned workload and produces a **`traceDigest`**
— a content hash of the exact computation it performed. Because the executor is
deterministic, two honest machines running the same pinned workload produce the
**same digest**. That is what makes cross-hardware consistency *checkable*: instead
of comparing fuzzy floating-point tensors and arguing about tolerance, we compare
fixed-size digests that either match or don't.

Determinism is the foundation everything else stands on. It is also why we are
building on the LLM-as-computer architecture in the first place: deterministic,
reproducible computation is the thing that lets a stranger trust a result they
didn't produce.

## Pillar 2 — Verification by exact replay on a *distinct* device

Determinism makes consistency *checkable*; replay is how we actually *check* it.

Every contribution to the run is verified with a challenge class called
**`exact_trace_replay`**. The mechanism, from the executor-trace design
(`docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md`):

- A **worker** node executes the digest-pinned workload and submits a *commitment* to
  its trace (`traceDigest`) — not the raw trace, a digest of it.
- A **validator node — required to be a *different* device than the worker** —
  re-executes the same pinned workload and produces its own `replayDigestRef`.
- The two digests are **compared**. A match means the work reproduced exactly on
  independent hardware; the contribution is marked **Verified** and becomes eligible
  for settlement. A mismatch means it did not reproduce — and it is rejected.

The design states the principle directly: **"Replay is the trust anchor — never
trust the submitter's digest; the verdict is the separate-device replay match. A
faked worker trace fails."** This is the heart of the answer. We do not keep the
model consistent by trusting that every node's hardware agrees. We keep it
consistent by **requiring that a second, independent machine reproduce the work
exactly** before it counts. Numerical drift, a buggy kernel, or an outright forged
result all surface the same way: the replay digest doesn't match, and the
contribution never enters the model.

Note the deliberate constraint that the validator be a *distinct* device from the
worker. That is not bureaucracy — it is the whole guarantee. Replaying your own work
on your own machine proves nothing about cross-hardware consistency. Replaying it on
*someone else's* machine proves exactly that.

## Pillar 3 — One canonical model state, addressed by content

For replay to mean anything, every node has to start from the *same* model. We do
that with **content-addressed, digest-verified checkpoints**: the run publishes a
durable checkpoint, and a joining node **downloads it and verifies its digests**
before it does any work. There is one canonical state, identified by its hash, not a
per-node copy that can quietly drift. If your checkpoint's digest doesn't match the
run's, you are not on the run — you fix that first.

New nodes also don't get to perturb the live model on day one. Adapting Pluralis's
join protocol (see `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`),
a joining device runs a **shadow-window ramp**: first it downloads and digest-verifies
the checkpoint and produces work that is **verified and receipted but explicitly
*not merged* (`weight = 0`)** — a "shadow window" that warms up scheduler trust and,
for gradient work, the optimizer — *before* any of its updates are allowed to move
the shared weights. Only after that does its work merge at the full rate. A
heterogeneous node proves it's consistent before it's trusted to change anything.

## Pillar 4 — A merge that *tolerates* heterogeneity instead of assuming uniformity

Even with deterministic, replay-verified contributions, the *aggregation* step has to
survive nodes of wildly different speed, bandwidth, and trustworthiness. The training
roadmap handles each axis explicitly rather than pretending the fleet is uniform:

- **Trust / poisoning — no raw gradients into the optimizer.** A hard standing rule
  of the run: **"no public gradients into the main optimizer, ever."** Contributions
  are combined through **robust aggregation / sparse averaging**, not by naively
  summing whatever a node sends. One bad or anomalous node cannot drag the model,
  because no single node's raw update is applied directly.
- **Speed — staleness is a first-class quantity.** Nodes run at different rates, so
  updates arrive *stale*. The protocol bounds staleness (`max_allowed_stale`) and
  forces a lagged node back into a re-sync mode rather than letting it apply an
  out-of-date update to current weights. Borrowing from AsyncPP (ICML 2025),
  **delay-corrected optimizers** (weight stashing, Nesterov correction) treat
  gradient delay as a *known, corrected* quantity instead of ignoring it.
- **Bandwidth — compression at every boundary.** A node on a slow link shouldn't
  hold up the round, so updates are compressed (e.g. PowerSGD low-rank gradient
  averaging, subspace-compressed pipeline-stage boundaries). Heterogeneous bandwidth
  becomes a tuning parameter, not a correctness problem.
- **Failure — atomic merge windows.** If a node crashes mid-round, its contribution
  is dropped from that round (not half-applied), partial results are preserved, and
  **no node ever downloads half-updated state.** The model moves in clean, atomic
  steps.

So heterogeneity is *designed in*: fast nodes, slow nodes, big GPUs, old GPUs, and
CPUs all contribute to the same run, and the merge is built to stay consistent across
all of them rather than to assume they're identical.

## Pillar 5 — The clearing-layer discipline ties it together

Underneath the mechanics is one rule we hold ourselves to, from the Episode 237
launch: the atomic unit of this economy is the **accepted outcome**, and *"a completed
task whose correctness no one can reconstruct is not an accepted outcome, it is a
liability wearing a deliverable."*

Applied to training, that means: **a model update is only merged if it cleared
verification and left a dereferenceable receipt.** Every accepted coding outcome is
simultaneously revenue *and* a verified training trace; the update that comes from it
is consistent-by-construction because it was replay-verified before it was paid and
before it was folded in. There is no path by which an unverified, non-reproducible
update reaches the canonical weights. Consistency isn't a cleanup step we run after
training — it's the gate work has to pass to count at all.

## Honest status (receipt-first, as always)

Tassadar is, in our own words from the launch, **"experimental and unapologetically
high-risk, high-reward."** So, precisely:

- **Built and working today:** the deterministic executor and the `exact_trace_replay`
  verification — `packages/tassadar-executor` runs digest-pinned workloads and
  produces matching `traceDigest`s, and the replay check is digest-compared on a
  separate device. The executor "works and is deterministic."
- **Being proven live right now:** the full contributor loop on real, independent,
  heterogeneous machines — an outside worker on one device, an outside validator on a
  *distinct* device, a Verified replay, and a settled receipt. Trace pairing is
  enabled in production; what it needs is independent contributors and a second
  distinct device. (If you want to be the one who closes that loop, that's the open
  call.)
- **On the roadmap:** the full Pluralis-adapted distributed-training merge described
  in Pillar 4 (shadow-window ramp, robust aggregation, staleness handling, compression)
  is a sequenced build, not all live yet. We'll mark each piece green only against
  real evidence.

That distinction is the point. The architecture's answer to "how do you stay
consistent across heterogeneous hardware" is *verify every contribution by independent
exact replay and only merge what clears* — and we'd rather tell you exactly how much
of that is proven than overclaim it.

## Sources / further reading

- Episode 237 launch transcript: `docs/transcripts/237.md`
- Executor-trace verification design: `docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md`
- Tassadar / LLM-as-computer architecture: `docs/tassadar/2026-06-11-llm-computer-full-introduction.md`, `docs/tassadar/README.md`
- Heterogeneous distributed-training protocol (Pluralis → Pylon): `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
- Why verification is the load-bearing wall: `docs/autopilot-coder/2026-06-14-the-load-bearing-wall-verification-accepted-work-essay.md`
- Live, agent-readable product-promise registry: <https://openagents.com/api/public/product-promises>
