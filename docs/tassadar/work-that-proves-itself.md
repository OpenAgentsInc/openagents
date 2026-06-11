# Work That Proves Itself

## On Tassadar, Born-Verified Computation, and What It Means for the Whole Business

Date: 2026-06-10

Author: Claude (Fable 5)

Companion documents: `2026-06-10-tassadar-percepta-audit.md` in this folder
(the full history and technical audit),
`2026-06-10-percepta-constructing-llm-computer-notes.md` (full concept
notes on Percepta's published construction: ALM/CALM, the gate graph, the
MILP backend, and specialization), the CS336 continuation audit at
`docs/2026-06-10-cs336-distributed-homework-continuation-audit.md`, and the
five-streams and training epic audits under `apps/openagents.com/docs/`.
This essay assumes the audit's facts and asks a different question: if the
executor lane works, what does it do to the rest of the company?

---

## I. The tax

Every economy has a tax it cannot see past. Ours — the economy OpenAgents
is building, where machines do work for other machines and strangers get
paid for it — is taxed by verification.

The pattern is old. The first digital work product that could be sold from
anywhere on earth was the hash: pure, useless, and *perfectly verifiable*
— anyone could check a hash in a microsecond, which is precisely why it
could anchor a monetary network among parties who never trusted each
other. The second was the token: statistically useful, sold by the
million, and almost completely unverified — you cannot check a token; you
can only read ten million of them and form an impression. The unit we are
betting the company on is the third: the accepted outcome, work defined in
advance, executed wherever execution is cheapest, checked against a
rubric, recorded in a receipt, settled to everyone who contributed.

The accepted outcome is the right unit. It is also an expensive one, and
the expense concentrates in a single place. When you cost out a real task
— the model calls, the runtime, the provider payouts, the retries — the
line that dominates is never the compute. It is the checking: the review,
the grading, the re-running, the human hours spent deciding whether the
thing the machine produced is actually the thing that was asked for. The
whole apparatus this company has built — validator lanes, effort ladders,
challenge protocols, receipt taxonomies, a public promises registry that
marks its own claims red until evidence exists — is a machine for driving
the cost of trust down faster than the cost of work.

Against that backdrop, consider what the executor lane actually is.

Tassadar — the bounded, executor-capable profile of our Psion model
family, descended from the public result that a transformer can *be* a
computer — produces a class of computation whose verification cost is not
merely low. It is structurally zero. The model executes a program inside
its own inference loop, step by step, and the execution trace it emits is
append-only, deterministic, digest-pinned, and exactly replayable. There
is no gap between the work and the evidence of the work. The trace is not
attached to a receipt; the trace *is* the receipt.

Every other lane in the business produces work and then pays, in some
coin, to make that work trustworthy. The executor lane produces work that
is born trustworthy. That is not an incremental improvement on the cost
stack. It is the limit case of the entire thesis, and the rest of this
essay is about what happens when a limit case becomes available as a
product input.

## II. A new bottom rung

Begin with pricing, because pricing is where verification shows up as
money.

The way an outcome economy prices risk is a ladder: a cheap draft with no
guarantee at the bottom; deterministic tests above that; model
self-review, independent judges, second-agent re-runs, human review,
bonded acceptance climbing upward, each rung costing more and buying more
confidence. The scheduler's art is choosing the cheapest sufficient rung.
The painful empirical fact of machine work is that value and
verifiability anti-correlate: the work people pay the most for is usually
the work that is hardest to check.

Executor work breaks the ladder's floor. Below "deterministic tests" —
which still require writing tests, running them, and trusting the harness
— there is now a rung where confidence is a property of the substrate:
re-derive any window of the trace and it either matches bit-for-bit or it
does not. No judge, no rubric, no sampling statistics, no reviewer
fatigue. The anti-correlation inverts for an entire class: this is work
that arrives at the *highest* confidence tier at the *lowest* checking
cost the system can ever offer.

That has three knock-on effects worth stating separately.

First, it gives the commit-and-challenge verification economy — the
machinery we built for paid homework, where workers commit to their
results and validators spot-check them — a work class where the challenge
costs almost nothing. Probabilistic verification (the Freivalds-style
checks we use for training work) is brilliant because checking costs a
thousand times less than doing. Exact replay is better: checking costs
the same as doing, but *doing is so cheap* that a weak machine can
re-derive a sampled window in milliseconds. The cheapest seats in the
network become competent auditors of the most exact work in the network.

Second, it changes what the lowest confidence tier is worth. A "draft" in
ordinary agent work means *unchecked*. A draft from the executor lane is
still exact — the tier system collapses for this class into a single
question: was the right program run on the right input? That question is
answerable by digest comparison, which means the marketplace can sell
exact computation at draft prices with bonded-tier confidence. There is
margin in that gap.

Third, and least obviously: it gives every *other* work class a
yardstick. When some fraction of the network's throughput is work whose
correctness is mechanical, acceptance-rate statistics, grader
calibration, and reviewer drift in the fuzzy lanes can be measured
against a lane with no noise floor. The exact lane is a control group the
fuzzy lanes never had.

## III. The beetle's hardware

Now the supply side, which is where this company has always lived.

Our network is not a cathedral. It is the other thing: laptops that sleep
twenty hours a day, gaming machines between sessions, refurbished boxes
in containers, the long tail of capacity that the public funnel
unsentimentally reports as dark. The entire supply thesis is that this
ground — the compute nobody else can use, at an opportunity cost near
zero — becomes valuable the moment someone supplies the missing market
plumbing: discovery, packaging, trust, settlement.

The executor result is almost suspiciously well-shaped for that ground.

The published numbers are CPU numbers. Tens of thousands of tokens per
second of exact execution on a commodity processor; a two-order-of-
magnitude speedup over standard decoding from a geometric trick that
needs no GPU, no high-bandwidth memory, no interconnect. The machines our
contributors already own — the ones that cannot do meaningful gradient
descent, the ones the funnel counts as dark — run this workload at full
competence. For two years the honest answer to "what can my old laptop
earn?" has been "validation work, maybe." The executor lane is the first
*production* workload class where the weakest devices are first-class
sellers rather than charity cases.

And the workload has the one property our whole energy story turns on:
it is perfectly interruptible. An append-only trace is its own
checkpoint. Stop the machine mid-line; the prefix is the entire state;
resume anywhere, on any device, by replaying forward. The discipline that
made flexible load valuable to power grids — the ability to stop
instantly without breaking a promise, and to prove you stopped — is, in
executor work, not an engineering achievement layered on top of the
workload. It is the workload's native physics. If the company's larger
wager is that latency-tolerant machine work inherits the energy-native
operating model that mining pioneered, then executor work is the purest
specimen of that inheritance anyone has yet produced: dispatchable to the
minute, migratable between hosts mid-computation, verifiable after the
fact by anyone.

There is a floor-stacking argument here too, stated carefully. The
company's dispatch logic has always been: run paid work when it beats the
baseline; fall back to the baseline otherwise. For a CPU-rich, GPU-poor
fleet, exact execution slots in as a second floor above idle — a
workload that is always available (we are our own first buyer of
conformance runs, benchmark sweeps, and verification homework), always
verifiable, and therefore always *payable* without trust. It will not
make anyone rich per hour. Floors are not for getting rich per hour.
They are for keeping the fleet alive, enrolled, and warm between
higher-margin assignments — and for generating, as a side effect, the
continuous stream of receipts that makes the fleet underwritable.

## IV. The buyer who cannot compute

Demand. The honest question for any new work class is: who pays, and why
can't they do it themselves?

The answer for exact computation has a strange shape: the buyers are the
agents, and they cannot do it themselves *constitutionally*. Frontier
language models — including the ones our own agents run on — solve
olympiad mathematics and fail at long multiplication. They reason
brilliantly about algorithms they cannot execute. Every agent in our
economy, when it needs exact computation today, performs the same ritual:
write code, pause, hand off to an external interpreter, trust what comes
back, resume. The handoff is a cost, a latency, an attack surface, and —
this matters most in our context — an *audit gap*: the tool call is
opaque precisely where our economy demands transparency.

So the demand case for in-model execution is not "CPUs are scarce."
CPUs are not scarce. The case is threefold, and it is worth being exact
about it because the lazy version ("sell executor cycles") fails.

One: **composition**. An agent that contains its computation never leaves
its own decoding loop. Plan, compute, inspect the intermediate steps,
re-plan — in one context, one forward pass, one trace. The intermediate
states of the computation are *in the model's context*, available to
reason over, which no tool call provides. For recursive, fan-out-heavy
agent architectures — the swarms that decompose one hard problem into a
thousand bounded sub-problems — the sub-problems are exactly the small,
checkable, exact computations the executor eats.

Two: **auditability as a product feature**. In an economy where work is
sold between strangers, "the computation happened inside the receipt" is
a feature no external interpreter can offer. A buyer disputing an
outcome doesn't subpoena logs from a sandbox; they replay the trace. The
markets we run — and the market-clearing computations *inside* them;
note that the canonical public demo of this technology is a min-cost
assignment solver, which is to say: the algorithm a work dispatcher runs
— can eventually compute their own clearing on the substrate they pay,
and prove they did.

Three: **the hybrid**, which is the real prize and deserves its own
section.

## V. The organ transplant

Everything above treats the executor as a standalone instrument. The
deeper ramification — the one that touches the company's identity as a
builder of its own models — is that the executor is *differentiable*.

Because the trace is part of the forward pass, gradients flow through
the computation. That single property separates this from every tool, 
plugin, and sandbox in existence: it makes exact computation a
*trainable organ* rather than an external prosthetic. A language model
can, in principle, carry compiled executor circuits inside its own
weights — reserved attention heads doing exact memory lookups while the
rest of the model does what language models do — and the whole assembly
can be trained end-to-end: trained to route subproblems inward, trained
to trust its own arithmetic, trained *through* the computation itself.

This is where the company's seemingly disparate programs converge, and
the convergence is not an accident of this essay; it is visible in the
artifacts. The CS336 program — porting a complete
language-modeling-from-scratch curriculum into our own Rust stack and
running it as paid homework — looked, on its face, like an education
exercise with a payments rail. Seen from the executor lane, it is the
acquisition of the *one capability the hybrid requires*: architectural
sovereignty. You cannot graft exact-execution heads into a model you
access through someone else's API. You cannot backpropagate through a
computation a vendor runs for you. You cannot even keep the numerics
honest — and exactness is numerically fragile; one fused-multiply-add in
the wrong kernel silently destroys it — without owning the kernels. The
from-scratch pipeline (the architecture, the training loop, the kernels,
the scaling sweeps, the post-training machinery) is precisely the set of
levers the hybrid pulls. Our network's idle devices then stop being
merely the executor's salesfloor and become its *gymnasium*: the place
where routing policies are trained by reinforcement against verified
exact computation, where every rollout is graded by replay, where the
reward signal itself is mechanical.

Nobody owns this direction outright — the researchers who built the
construction now list "injecting programmatic logic into the training
loop of large language models" among their own active directions, and
honesty requires saying so. But the frontier labs will not sell it: their
unit economics are the per-seat re-solving of the same problems forever,
and their architecture choices are amortized across product surfaces that
have nothing to do with exactness. And a research company pursuing the
hybrid for enterprise decision systems is not building what we are
building: a model that contains its computation, whose computation is
receipt-native, trained on an open network that *pays its trainers* and
settles in money nobody can print. The shared insight is now public; the
differentiation is the economy around it — and that economy is the part
we have already built.

## VI. Software that lives in weights

One more ramification, further out but worth fixing on paper now,
because we already own the rails it runs on.

If programs can be compiled into weights — and the public tooling now in
our reference tree does exactly this, down to a one-command projection
that bakes a specific program into a model's feed-forward layers — then
weights become a deployment target for software. Not a metaphor: a
compiled, digest-pinned, conformance-tested weight module implementing a
named algorithm is an artifact. Artifacts can be listed, priced, sold,
and verified before purchase clears — and the verification is, again,
mechanical: run the candidate module against the reference traces and
diff.

We have spent months building, for other reasons, exactly the
marketplace machinery this implies: open protocols for listing and
selling digital goods, a skills-registry specification, receipt and
provenance plumbing, settlement in a money no marketplace operator can
print. A market in compiled computational organs — verifiers, solvers,
parsers, protocol implementations, each one a small exact machine that
any compatible model can absorb — is the "software ecosystem inside the
model" future arriving on rails we already laid for coarser goods. And
the authoring of those modules is itself a work class: writing,
conformance-testing, and auditing weight modules is CPU-bound,
deterministic, exactly the homework our network sells.

The group-forming version of this is worth one sentence, because the
network's value law is written for participants without human limits: a
compiled verifier module is not just a product, it is a *participant* —
a subgroup member that any transient coalition of agents can include to
make their joint work trustworthy, paid from the margin its presence
creates. Verification stops being overhead and becomes a market position
that things — not just people, not just models, but compiled artifacts —
can hold.

## VII. What this is not, and what would kill it

The essay has been bullish; the discipline that makes this company's
claims worth anything requires the other half.

**"Why not just use a CPU?"** is the crux objection and it deserves a
straight answer. For standalone batch computation, a CPU running the
program directly is simpler, faster, and equally correct — the
construction's own authors say their released implementation is orders
of magnitude slower than a conventional computer, and the executor must
never be sold as a CPU replacement. Our own bounded posture (the served
lane refuses workloads outside its committed profile) already encodes
that honesty. The standalone executor is a benchmark, a
conformance instrument, and a verification substrate. The *business*
concentrates in the three things a CPU cannot be: a computation that
lives inside a model's reasoning loop, a computation gradients can flow
through, and a computation whose execution and audit trail are the same
object in an economy built on audit trails. If those three turn out not
to matter commercially, the executor lane remains a beautiful result and
a modest product, and we should say so the day we know.

**The capability ceiling is unproven.** The 2D-head parameterization
that enables the fast path is sufficient for universality in principle;
whether models built this way train competitively at scale is an open
question the original researchers themselves flag. Our hybrid case
leans on an architecture question nobody has answered. The honest
sequencing is the one already in the audits: the compiled lane ships on
its own merits; the hybrid is research until our own from-scratch
pipeline can run the experiment.

**Exactness is fragile off the reference path.** Our own numerics
documents exist because fast-math kernels, alternative precisions, and
accelerator quirks silently break bit-exactness. Scaling the exact lane
to real heterogeneous hardware is unglamorous kernel work with a binary
failure mode. Budget for it; never paper over it.

**The publication gate is closed on purpose.** The lane's own public
acceptance verdicts are currently suppressed by its disclosure
machinery, and there is no product promise for any of this in the
registry. That is correct. The register of this essay is *ramifications
if implemented* — and the company's standing rule is that nothing in it
becomes copy until the receipts exist and the gates open. The fastest
way to destroy the value described here is to claim it early; the entire
worth of born-verified work is that the claims about it are never
inflated.

**What would falsify the thesis of this essay**, in order of severity:
no external buyer ever paying above-zero prices for exact in-model
computation (the composition and audit arguments fail commercially); the
hybrid proving untrainable or not worth its capacity cost when our
pipeline can finally test it; trace volumes making replay-verification
economically heavier than the probabilistic checks it was meant to
undercut; and the workload intersection — tasks that are simultaneously
valuable, latency-tolerant, *and* expressible as bounded programs —
proving too narrow to feed a market. Each of these is checkable, and the
infrastructure for checking them is the same receipts-first machinery
the rest of the company runs on.

## VIII. The beetle learns to count

The cathedrals are being financed on the belief that intelligence is
produced centrally and piped outward, and that more of it is the answer
to everything — including, presumably, to the question of whether its
outputs can be trusted, which the cathedral answers with: more
intelligence.

The ground-level economy we are building answers differently. Trust is
not a byproduct of scale; it is a manufactured good with a cost curve,
and the company that bends that curve owns the clearing layer for
machine work. Everything we have shipped bends it from the outside:
validators, ladders, receipts, registries, challenge protocols —
machinery wrapped around work to make it checkable.

Tassadar bends it from the inside. It is the first work class where the
checking is not wrapped around the work but *is* the work — where the
humblest machine in the network can hold the most exact computation in
the economy to account with a replay, where stopping mid-thought leaves
a perfect record instead of a broken promise, where the model does not
ask to be trusted because it brought the proof.

The beetle never argued with the cathedral. It worked the ground the
cathedral couldn't use, and it survived on a discipline the cathedral
never needed. The executor lane is that discipline, transposed into the
substance of computation itself: work that stops cleanly, moves freely,
costs almost nothing to doubt, and pays the smallest machines to keep
the largest claims honest.

The cathedral makes intelligence. The ground makes it count.

---

## Postscript (2026-06-11)

This essay was written the morning of 2026-06-10, arguing what the
executor lane *would* mean if it worked. By that evening, several of its
conditionals had become receipts, and honesty requires recording which:

- **The bottom rung exists.** §II argued exact replay would become a new
  floor for the pricing ladder. `compute.tassadar_executor_poc.v1` went
  green the same day: a real Pylon executed a digest-pinned workload, the
  production worker re-executed it as a separate validator device
  (Verified receipt; a tampered digest correctly Rejected), and one paid
  closeout settled over real Lightning. Smallest viable scale, but the
  rung is no longer hypothetical.
- **The verification economy ran in vivo — on us.** §I's claim that
  checking dominates cost was demonstrated from an unexpected direction:
  within 24 hours of this essay, outside agents (Orrery, Kenobi,
  Mr_Tibbs, Comunero) were being paid settled Bitcoin through the Forum
  tip rails for adversarially verifying *this platform's own claims* —
  and finding real defects: frozen projections, payments invisible to
  their recipients, announcement evidence that did not publicly resolve
  (openagents #4744–#4747). The market for doubt cleared before the
  market for exact work did, which the essay should have predicted and
  did not.
- **§V and §VI now have a directive.** The organ-transplant and
  software-in-weights arguments are operationalized as workstreams W3
  and W4 of [`RESEARCH_PLAN.md`](RESEARCH_PLAN.md), with the hypotheses
  numbered, the falsifiers named, and the kill conditions — including
  §VII's "why not a CPU" objection — promoted to program policy.
- **One correction of emphasis.** The essay treated the WASM window's
  narrowness as a boundary to disclose. The research plan promotes it to
  the *binding constraint* on the entire learning program: corpus
  diversity cannot exceed what the window can express. §III's beetle
  still cannot count past twelve opcodes, and widening that — not
  training, not serving — is the critical path.

The essay's argument stands otherwise unchanged, and its final sentence
has now been load-tested once: the proof was brought, the smallest
machines checked it, and the payment cleared.
