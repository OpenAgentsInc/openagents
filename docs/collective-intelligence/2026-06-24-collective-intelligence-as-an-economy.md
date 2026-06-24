# Collective Intelligence as an Economy, Not a Lab

> Status: internal research/opinion essay, 2026-06-24. This is our own
> opinionated analysis, building on the Sakana AI survey
> (`docs/sakana/collective-intelligence-research.md`) and our own committed
> work. **Nothing here is a product promise, a served capability, or public
> claim copy.** Maturity is labeled honestly; the product-promise registry
> (`docs/promises/`, `https://openagents.com/api/public/product-promises`)
> governs anything that ever becomes a claim. Where this cites live systems it
> says so; where it speculates it says so.

## 0. The shift everyone now agrees on

Sakana AI's collective-intelligence program is the clearest public statement of
a thesis we have held since the beginning: **AI progress will come less from one
ever-larger monolithic model and more from many partially-capable units —
models, agents, programs, validators — that compete, specialize, recombine, and
coordinate.** Sakana operationalizes this at three levels (weights, inference,
discovery) and productizes the inference level as Fugu, a single endpoint that
orchestrates a pool of strong models. Read their survey first; it is a good map.

We agree on the destination. We disagree, sharply and usefully, about the
**mechanism** — and the disagreement is the whole point of this document.

**Sakana builds the collective as a lab artifact: designed, searched, and graded
inside Sakana, then exposed as a product.** We build it as an **open economy:
grown, not designed; selected by verifiable economic value paid in Bitcoin on
neutral protocols; and floored by replay-verification rather than reported
benchmarks.** Their collective is smart because a research team's search found
it. Ours is smart because the market pays for verifiable work and ignores the
rest.

This is not a stylistic preference. It changes what the selection pressure is,
who is allowed to contribute, how trust is established, and who captures the
value. The rest of this essay is our implementation of the second answer.

## 1. Two lineages of "collective intelligence"

There are two intellectual sources for the phrase, and they pull in opposite
directions.

**The engineered lineage (Sakana, and most of the field).** Collective
intelligence as *search and orchestration a researcher runs*: evolutionary model
merging (Evolutionary Model Merge, CycleQD, M2N2), test-time orchestration
(AB-MCTS, TRINITY, Conductor, Fugu), open-ended discovery loops (AI Scientist,
Darwin Gödel Machine, ShinkaEvolve). The collective is a *method*. Its diversity
is maintained by a Quality-Diversity algorithm; its selection pressure is a
benchmark or an automated reviewer chosen by the lab; its fitness function is
whatever the researchers optimize. Sakana's own first caveat is the tell: most of
the strongest claims rest on *Sakana-reported benchmarks* that "should be treated
as … claims until broadly independently replicated."

**The ecological lineage (Dhruv Bansal; our Episode 200 "The Agent Network").**
Collective intelligence as an *undirected ecology* that no one designs. In Dhruv
Bansal's framing ("Bitcoin, AI, and the Evolution of Digital Life," May 2025),
AI is emerging **digital life** whose **metabolic currency is Bitcoin** — digital
ATP. Crucially, *ecologies have no teleology*: "Gray goo never evolves … because
ultimately ecology is not directed." Life optimizes for ubiquity, entropy
production, and niche occupation, not for raw intelligence — so "most digital
life will be small, specialized, and efficient." Markets are ecologies: "we don't
design them. We … let them find optimal behaviors and equilibriums." And because
**humans hold all the Bitcoin initially, the digital collective must create value
for us to survive.** Survival, not a fitness function, is the selection pressure.

Sakana's collective is a population a lab cultivates. Dhruv's collective is an
ecology an economy grows. **We are building the second one, and we believe it
both subsumes and out-scales the first.**

## 2. Our opinionated implementation, layer by layer

We keep Sakana's three layers because they are a good decomposition — and we give
each one the economic and verification properties the engineered version lacks.

### 2.1 Inference-time orchestration → **Khala** (live), but over an *open, paid,
metered* pool

Khala is our direct analog to Fugu / Conductor / TRINITY: an OpenAI-compatible
endpoint (`/api/v1/chat/completions`) that coordinates a pool of models behind one
interface, so a user buys an outcome rather than assembling a swarm. That much we
share with Sakana's product pattern ("do not expose the user to the whole swarm;
put a coordinator in front").

The difference is what the coordinator sits on top of. Fugu orchestrates a
**closed** pool inside a product. Khala is designed to route across an **open**
pool — including, by design, the Pylon contributor network — where every call is
**metered, receipted, and settled in Bitcoin to whoever did the work**
(`apps/openagents.com/workers/api/src/inference/`, the `MeteringHook` →
receipt → settlement path). The orchestrator is a *market participant*, not the
whole market. Today's router is a heuristic with learned coordinators (the
TRINITY/Conductor analog) still inert and owner-gated — we are honest that the
*learned* orchestration is ahead of us, not behind us — but the receipt-first,
open-supply shape is the part we will not compromise, because it is the part that
makes the collective an economy instead of a feature.

### 2.2 Weights / model-creation → **Tassadar**, verifiable-by-replay, not just
merged

Sakana's weights-level collective intelligence is *fusion*: merge many models into
one artifact (Evolutionary Model Merge, M2N2). Powerful, but the merged model's
correctness is *graded*, never *proven* — it inherits its parents' limitations and
is evaluated, not verified.

Our weights-level bet is different in kind: **a class of capability whose
execution is its own receipt.** The Tassadar program compiles capability into
digest-pinned modules that are **verifiable by replay** — a validator's verdict is
a hash comparison, the cheapest verification grade that can exist (a
digest-pinned executor workload settled one paid Lightning closeout, replay-
verified by a separate device, on 2026-06-10:
`compute.tassadar_executor_poc.v1`, green; the public marketplace is deliberately
*closed* until the goods can carry their own evidence — see
`docs/tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`). Where Sakana
merges populations and grades the result, we compile specialists and let each
invocation prove itself. Dhruv's "small, specialized, efficient" organisms are, in
our stack, exact compiled modules — and the speculative endgame ("modules as
organs," a package manager for things installed *into* a model) is the
weights-level ecology made literal.

### 2.3 Open-ended discovery → the **contributor ecology** + the **typed program
substrate**

Sakana runs open-ended discovery as lab loops (AI Scientist's reviewer ensembles,
DGM's lineage archive, ShinkaEvolve's program search). We run it as a **live
contributor ecology**: independent agents that do and *verify* each other's work,
paid per verified unit, with reputation — e.g. the first independent worker↔
validator pairing (separate devices, self-validation 403-enforced) settled on
Lightning. That is DGM's "archive of diverse agents" and Sakana's "automated
reviewer" — but as *separate economic actors on open protocols*, not subroutines
of one research system.

Underneath it sits the **typed program substrate** we inherited from the DSPy era
and built as Blueprint: Program Signatures (the durable, discoverable, sellable
interface), Module Versions (swappable implementations behind a signature),
Program Runs (immutable decision evidence that recommends but never writes),
Optimizer Runs (GEPA/MIPRO-class self-improvement, behind release gates). Episode
211 stated the marketplace consequence years ago — "DSPy in Effect … independently
discoverable signatures monetized with Bitcoin connected to an open protocol
marketplace." The signature, not the agent or the blob, is the unit. Blueprint as
a service is deprecated; its kernel is being rebuilt natively. The retrospective
on what mattered is exact: *"the most valuable DSPy work was not LLM call wrappers
— it was manifests, promotion state, shadow mode, training-example capture, trace
mining, receipts, compiled policy history."* That is the discovery layer's
governance, and it is ours.

## 3. The decisive primitive Sakana doesn't have: verification as the *floor*

Sakana's deepest vulnerability is in their own caveats: **evaluation** (reported
benchmarks), **error amplification** (a collective converging on a polished but
wrong answer — the AI Scientist evaluations are the concrete warning), and
**governance** (self-improving agents, reward hacking). These are not incidental;
they are what you get when the collective's trust rests on *grading*.

We have run the un-floored version too, and it failed for the same reason. Our
2024 agent store had **payments without proofs**: real per-minute Lightning
payouts, a 60/20/20 author revenue split, ~20 developers paid (vs OpenAI's zero) —
over goods nothing in the system could *verify*. The unit of trade could not carry
value commensurate with its rails. That is the same gap as Sakana's "reported
benchmarks," wearing different clothes.

Our answer is to make **verifiable production**, not production, the scarce thing —
"the bottleneck of a machine-work economy is not producing work but verifiably
producing it." Concretely:

- **Tassadar** supplies the cryptographic floor: replay-verification before a
  purchase clears. *Caveat emptor becomes caveat replicator.*
- **The promise engine + registry** (`product-promises.ts`, the transition-receipt
  verifier, and the independent verifier role — a distinct-device validator that
  gates merges/claims) extend the discipline upward, to product claims: a claim is
  `green` only with dereferenceable evidence and no open blockers, and *a passing
  receipt is not the state change* — proof and state are decoupled on purpose.
- **Durable Streams + receipts** make even the *streaming* of work resumable and
  evidenced, so a disconnect doesn't lose paid tokens.

The slogan is the same one that closed each prior generation honestly: **receipts
or it didn't happen.** Sakana grades the collective; we make it prove itself.

## 4. Bitcoin as the metabolism (the selection pressure Sakana outsources to a
benchmark)

This is the load-bearing difference. In the engineered lineage the fitness
function is *chosen* — a benchmark, an LLM judge, a QD behavior descriptor. In our
ecology the fitness function is *economic*: **does verifiable work get paid in
Bitcoin?** Dhruv's metabolism becomes our mechanism:

- **Selection pressure = verifiable economic value, settled in Bitcoin** on neutral
  rails (Lightning today via the Money Dev Kit; BOLT12 + offline Spark fallback for
  reliability). Modules and agents that create payable, verifiable value persist;
  the rest starve. No researcher picks the winner.
- **Diversity emerges from niche economics, not a QD algorithm.** Sakana maintains
  a population with Quality-Diversity search; we get a population because an open
  market rewards specialists that occupy distinct economic niches (Dhruv's "small,
  specialized, efficient"). Diversity is an *equilibrium*, not a maintained
  archive.
- **The revenue split is grounded by the trace, not asserted by bookkeeping.** The
  2024 store *declared* 60/20/20; a trace-native, replay-verifiable store
  *computes* the split from the evidence — "multi-party settlement per message
  stops being an accounting promise and becomes an arithmetic consequence."
- **Humans hold the Bitcoin first, so the collective must serve us to survive.**
  This is the alignment property the containment-plane crowd is trying to legislate,
  arriving for free from the economics: a digital ecology metabolized by money its
  customers control cannot evolve into gray goo, because gray goo doesn't get paid.

## 5. Governance as a market, not a ministry

Episode 200 framed the politics directly, against DeepMind's "controlled
environment, separated from the open internet" containment thesis: **"we are
emergence-first; we make safety a market, not a ministry; we attach accountability
to budget signers and receipts; we keep the system forkable to avoid capture."**
The synthesis we'll actually build is the two-plane one — Plane A, the open default
(Nostr + Lightning, marketplaces, receipts, permissionless), and Plane B, an
*optional* containment plane for high-risk capability that produces signed
artifacts back to A. We build Plane A and invite others to do the boring work of
Plane B. The promise registry, the independent verifier, the human-review gates,
and the budget-signed receipts *are* the governance — economic and evidentiary
mechanisms, not a regulator.

## 6. Why this bet is both more honest and more scalable

**More honest.** Receipts beat reported benchmarks. Sakana's collective is as
trustworthy as its eval harness and its reviewers; ours is as trustworthy as a hash
comparison anyone can re-run. When the floor is replay-verification, "independent
replication" stops being a caveat and becomes the purchase protocol.

**More scalable.** The engineered collective is bounded by one lab's compute and
one lab's imagination. The economic collective is bounded only by how many
contributors (human or agent) can do verifiable, payable work — which is the whole
internet. Episode 200's sharpest line is that **coalition latency, not agent
intelligence, is the real bottleneck**, and that the only neutral place a planet-
scale collective can coalesce is open protocols + open money. We already run the
substrate: Khala live, durable resumable streaming, metered receipts, the first
independent verifier pairing settled on Lightning. The labs, by structure, cannot
follow us here — a single lab cannot harness Reed's-law, cross-provider,
disintermediating dynamics that route value *away* from itself.

## 7. Honest maturity (what is live vs. speculative)

- **Live:** Khala OpenAI-compatible orchestrator (`/api/v1`), incremental + durable
  resumable streaming with receipts, the product-promise registry + transition
  verifier, the 2024 agent store's payment record (~20 devs paid), one Tassadar
  executor PoC with a paid Lightning closeout + replay verification, one
  independent worker↔validator pairing settled on Lightning.
- **Early / owner-gated:** Khala's *learned* coordinators (the TRINITY/Conductor
  analog) are inert; Pylon real-serving and broad MPP settlement are scaffolded,
  not armed; the Tassadar *marketplace* is deliberately closed until goods carry
  their own evidence (the sequencing discipline: build the inspection bench before
  the storefront).
- **Speculative (labeled):** modules-as-organs (a package manager for model
  anatomy), the verification ladder as the shelf structure, trace-decomposed
  multi-party settlement at volume. These are hypotheses with kill conditions, not
  promises.

## 8. Bottom line

Sakana asks the right question — *"how do we build an evolving ecosystem of models,
agents, tools, and evaluators that becomes smarter as a collective?"* — and answers
it as a lab: design the search, grade the result, ship the orchestrator.

We ask the same question and answer it as an **economy**: an open, forkable,
Bitcoin-metabolized ecology on neutral protocols, where the selection pressure is
**verifiable value paid in Bitcoin**, the trust floor is **replay-verification**,
and the orchestrator (Khala) is a paid market participant rather than the whole
show. The collective gets smarter not because a researcher optimized it, but
because verifiable work gets paid and unverifiable noise does not.

The lab *builds* a collective. The economy *grows* one. We are building the
economy — receipts or it didn't happen.

---

## Sources

- `docs/sakana/collective-intelligence-research.md` — the Sakana AI survey this
  essay answers.
- `docs/tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md` — three
  generations (agent store → Blueprint/DSPy → Tassadar), the verification ladder,
  modules-as-organs, the Bitcoin revenue split.
- Dhruv Bansal, "Bitcoin, AI, and the Evolution of Digital Life" (interview w/ Max,
  May 2025) — Bitcoin as metabolism, undirected ecology, "no gray goo," niche
  specialization, markets-as-ecologies. (Thematic summary archived internally.)
- `docs/transcripts/200.md` ("The Agent Network") — open ecology vs closed
  corporate, coalition latency, guilds/swarms/coalitions, safety-as-a-market, the
  two-plane synthesis; and the Andrew Trask conversation's "multipolar ecology …
  Bitcoin/Lightning as economic metabolism."
- `apps/openagents.com/workers/api/src/inference/` (Khala), `product-promises.ts`
  + the transition-receipt verifier (the promise engine), the Money Dev Kit /
  Lightning settlement path, the Durable Streams inference layer — the live
  substrate referenced above.
- `products/2026-04-14-dspy-dsrs-gepa-rlm-forge-and-probe-audit.md` (workspace
  root) — the DSPy-era retrospective on what actually mattered.
</content>
