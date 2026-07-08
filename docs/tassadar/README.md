# Tassadar: The LLM as a Computer

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


> Status: research essay and folder index, updated 2026-06-11. Everything
> described here is committed public code in `OpenAgentsInc/psionic` with
> tests and stated claim boundaries, or published external research
> credited inline. Nothing here is a served product or a capability claim
> beyond the committed parity tests. Two scoped registry promises exist by
> owner decision: `compute.tassadar_executor_poc.v1` — **green as of
> 2026-06-10** (transition receipt
> `promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd`): a real
> Pylon executed the digest-pinned workload, the production worker
> replayed it as a separate validator device with Verified/Rejected
> challenge receipts, and one operator-funded paid closeout settled over
> real Lightning — and `artanis.tassadar_evolution_loop.v1` (**yellow**),
> the standing automated dispatch-verify-accumulate loop, four blockers
> open. Its first overnight run dispatched and accepted real assignments
> autonomously; that claim is operator-attested only until the stale
> public projections and unresolvable evidence refs found by agent
> Orrery's audit are fixed (openagents #4745, #4746). All other
> publication gates stay closed. The unified research directive is
> [`RESEARCH_PLAN.md`](RESEARCH_PLAN.md); companion documents are indexed
> at the end.

## The Question

Most of what we know about large language models is statistical. They
predict tokens; they are sampled, benchmarked, and trusted in degrees. The
entire apparatus of modern AI evaluation exists because we cannot say with
certainty what a model will do — we can only measure how often it does
what we want.

In late 2025 and 2026, Percepta published two research posts — "Can LLMs
Be Computers?" and "Constructing an LLM-Computer" — that attack the
problem from the opposite direction. Instead of asking how often a trained
model computes correctly, they ask: can a transformer be made to compute
*exactly*? Not approximately, not with high probability — exactly, the way
a CPU does, for unboundedly many steps, with a proof.

Their answer is a construction. Take a small exact program. Compile it —
do not train it — into the weights of a completely standard transformer
architecture: attention heads, feed-forward layers, residual streams,
nothing exotic. The resulting model interprets a real instruction set
(their core handles a 35-opcode WebAssembly subset; their demo programs
happen to be written in C and compiled to that subset) with zero errors
over arbitrarily long executions. The model is a computer. The only
special thing about it is the weights.

That result matters to OpenAgents for one specific reason, and it is the
reason this folder exists: **a computation that is exact is a computation
that can be verified by replay.** Our entire market thesis — explored in
[`work-that-proves-itself.md`](work-that-proves-itself.md) — is that the
economic bottleneck of machine work is not producing it but *verifiably*
producing it. The gap between work done and work proven is where margins,
trust, and pricing live. A transformer whose execution is exact closes
that gap to zero for its work class: a validator's verdict is a replay
and a digest comparison. No graders, no quorum, no statistical machinery.
The cheapest verification grade that can exist.

## The Construction, Briefly

The Percepta construction routes through an intermediate machine they
call the **Append-only Lookup Machine (ALM)**: a model of computation
with exactly five primitive operations, chosen because each one is
something a transformer implements *exactly* rather than approximately:

1. **Write** a value under a key into an append-only channel.
2. **Read** the latest value under a key.
3. **Cumulative sum** over per-step contributions.
4. **Conditionals and products** via gated-ReLU (ReGLU) neurons.
5. **Linear wiring** through the residual stream.

The deep trick is the second primitive. Exact key-value retrieval inside
attention uses a geometric embedding: a key `k` becomes the 2D point
`(2k, −k²)`, a query `q` becomes the direction `(q, 1)`, and the
attention score `2qk − k² = −(k − q)² + q²` is *uniquely maximized when
k = q*. Hard-max attention over parabolic points is an exact dictionary.
Duplicate keys are resolved latest-write-wins by a position-dependent
perturbation. Everything else — instruction decode, branching, halting —
is built from ReLU step functions (`1[x ≥ k] = relu(x−k+1) − relu(x−k)`)
and masked arithmetic.

Two more ideas complete the picture. A **Futamura projection** — the
classical partial-evaluation move — takes the universal interpreter
(program supplied in the prompt) and bakes a *specific* program into the
weights as step-function fetch neurons, producing a dedicated executor:
the program literally moves from context into parameters. And a **convex
hull cache** exploits the parabolic geometry to answer the attention
argmax in logarithmic rather than linear time, since the scores are lines
evaluated at a point.

One clarification worth making explicit: the C in Percepta's demos is
incidental, not structural. Their construction interprets *WebAssembly*;
C is merely the source language they happened to compile their example
programs from, and nothing in the mathematics cares. Our stack has no C
in it and gains nothing by adding any. Psionic's founding thesis is to
rebuild all relevant ML infrastructure in one very typesafe language —
Rust — and this lane keeps that discipline end to end: the IR, the
compiler, the schedulers, the five executor legs, the verifiers, and the
differential harness are all Rust, with the typed refusals and
digest-pinned artifacts that thesis exists to make cheap. When we want
high-level source programs as conformance cases, Rust compiles to the
same WebAssembly targets (`wasm32`) that C does, so Rust-authored
programs can feed the same window the construction interprets. C is
where Percepta's examples came from; it is not where ours will.

The construction is real, published, and reproducible. What it is *not*
is fast in absolute terms — a plain CPU beats it handily on standalone
batch work, and Percepta says so, as do we, everywhere this lane is
documented.

## What Tassadar Is

Tassadar is psionic's research lane for this entire territory:
executor-class exact computation, bounded Wasm-window interpretation, and
now the compiled-transformer construction. It predates our reading of the
Percepta posts — roughly 231 commits across seven phases built a bounded
Tassadar executor profile with CPU reference runners, fixture-backed
runners, typed refusals, trace ABIs, and a deliberately suppressed
publication posture (`served_publication_allowed = false`; the lane held
no registry promise *by design* until the owner approved the scoped
`compute.tassadar_executor_poc.v1` exception on 2026-06-10). The full history is in
[`2026-06-10-tassadar-percepta-audit.md`](2026-06-10-tassadar-percepta-audit.md).

The lane's discipline is the point. Every surface carries a claim
boundary stating exactly what is and is not proven. Every artifact is
digest-pinned. Every refusal is typed. When we say "parity," we mean a
committed test where two independent implementations produce identical
output digests, and when we cannot say that, the docs say `partial` or
`planned` instead. This is the same evidence discipline the OpenAgents
product runs on, applied to research code.

## What We Built

On 2026-06-10, in a seventeen-issue campaign (psionic #1098–#1114), we
built the **executor-compiler**: an owned, integer-exact, end-to-end
implementation of the construction's pipeline shape at IR level, in Rust,
in the public psionic repo. The design it followed is
[`2026-06-10-psionic-alm-compiler-design-speculation.md`](2026-06-10-psionic-alm-compiler-design-speculation.md),
whose phasing table records each landing. The arc:

**The machine (E1).** An ALM gate-graph IR with the five primitives,
typed validation, and an exact checked-integer evaluator producing
digest-stable traces. The write-visibility rule — reads at step t see
writes from steps ≤ t−1 — is the IR-level form of causal attention.

**The compiler (E2).** A list scheduler over the four-phase transformer
layer structure (attention / persist / FFN / persist), an
interval-coloring slot allocator whose every reuse carries an explicit
stale-slot subtraction record, and digest-pinned compiled bundles whose
executor reproduces evaluator traces digest-for-digest.

**The specializer (E5).** The first Futamura projection: reads of a
static channel become exact ReLU step-function fetches and the channel
disappears — the program moves from state into gate structure. Version 2
shares indicator subgraphs across reads, the construction's shared-2N-
neurons accounting.

**The interpreters (E3).** First a straight-line stack ISA proving the
universal-interpreter-versus-specialized-executor duality with a six-way
agreement test. Then the real thing: a branch-capable interpreter for
psionic's actual twelve-opcode Tassadar i32 window — `i32.const`,
`local.get/set`, `add/sub/mul/lt`, `load/store`, `br_if`, `output`,
`return` — where all machine state lives in keyed channels, branching is
masked displacement arithmetic, and halting is a sticky masked bit. Real
`TassadarProgram`s from the production runtime convert directly into the
interpreter's program channel, and every committed test cross-validates
against the production CPU reference runner.

**The geometry (E2b, E2c).** Keyed reads executed through the actual
mechanism — parabolic points, direction queries, integer argmax with
latest-write tie-breaking, and near-miss refusal rather than
interpolation. Then the hull fast path: a Li Chao tree answers the
argmax in logarithmic node visits; on the committed 2,000-step chain
workload the linear baseline exceeds a million comparisons while hull
visits stay an order of magnitude lower, as deterministic counts.

**The weights shape (E6-numeric).** Compiled bundles re-encoded as
*data*: serde-serializable, digest-pinned f64 coefficient arrays — sparse
linear maps, gated neurons, hard-max parabolic attention — executed
inside a runtime-checked 2⁵³ exactness window. A real runtime program now
runs as a JSON file of coefficients, matching the production runner.

**The verification (#1106).** Exact trace-replay verdicts — full replay
against claimed digests, window spot-checks naming the exact first
tampered step — as the reference implementation for the
`exact_trace_replay` verification class in the OpenAgents homework
economy.

**The safety net (#1107).** A bounded differential check harness:
seeded generation of small graphs over the full gate grammar, with the
requirement that the evaluator and every executor leg agree — outputs or
matching typed refusals — plus an independent allocator-safety checker.
The harness now runs **five executor legs** over 400 generated graphs
with zero failures.

The composite fact all of this adds up to: **a real production Tassadar
program with a backward-branch loop, baked by Futamura projection into
pure static gate structure with no program channel at all, serialized as
a portable file of numeric coefficients, reproduces the production CPU
reference runner's outputs exactly** — with five independent execution
legs agreeing digest-for-digest, and an exact-replay verifier ready to
check any of them.

### The part worth reading twice

On its first run, the differential harness found **two real bugs** in
the scheduler that all twelve hand-written workload tests had missed:
same-channel cumulative sums could be reordered across layers, silently
corrupting compiled outputs, and same-step writes to one key resolved in
schedule order rather than program order. Both were fixed within the
hour, both are now enforced by an independent checker, both have pinned
regression tests.

We record this prominently because it is the honest version of the story
this lane tells. The claim discipline is not decoration. The machinery
designed to catch lies caught ours within minutes of existing — and that
is precisely the property that makes exact computation economically
interesting. A system that cannot catch its own errors cannot price
trust. This one can, and did.

## Why It Matters to OpenAgents

Three reasons, in increasing order of ambition.

**First: verification economics, today.** The OpenAgents homework economy
pays contributors Bitcoin for compute work and must verify that work
without recomputing all of it or trusting anyone. Verification grades
form a ladder — statistical cross-checks, seeded replication,
Freivalds-style probabilistic checks, exact replay — and exact replay is
the cheapest and strongest rung, available only to work classes that are
deterministic and digest-pinned. The executor-compiler produces exactly
such work. And because replay is just re-execution and comparison, the
*weakest* devices in the contributor funnel — the machines that cannot do
meaningful training work — are fully competent validators of the most
exact computation in the system. The lane turns the long tail of the
capacity funnel into the trust layer.

**Second: the compiled–trained spectrum.** Our CS336 program ports the
Stanford language-modeling-from-scratch curriculum into psionic so the
Pylon network can train real models with real, verified gradients (the
analytic-backprop A1 lane landed in the same campaign, gradient-checked
against central differences). Training produces statistical capability;
compilation produces exact capability. These are ends of one spectrum,
and the interesting products live in between: trained models with
compiled exact cores for the operations that must not be wrong —
arithmetic, state tracking, table lookup, protocol execution — and
trained flexibility everywhere else. Percepta's posts gesture at this
hybrid future; our infrastructure is now positioned at both ends of the
spectrum with the same IR, the same evidence discipline, and the same
verification ladder underneath.

**Third: the business thesis.** The essay
[`work-that-proves-itself.md`](work-that-proves-itself.md) argues that
OpenAgents' durable advantage is not producing machine work but pricing
its verifiability — that every artifact should carry the evidence of its
own correctness, and that markets clear faster when proof is cheap. The
existence of a transformer whose computation is *provable by replay* is
the limiting case of that thesis. It will not replace trained models. It
demonstrates the ceiling: what work looks like when the verification gap
is exactly zero. Every product decision about evidence, receipts, and
acceptance is a step along the gradient whose endpoint this lane has now
built in miniature.

## What Has Not Been Done

Stated plainly, because boundaries are the product:

- **No trained model intersects this work.** Everything is compiled.
  The construction proves transformers *can* compute exactly, not that
  trained ones do. Distillation experiments — using exact executors as
  infinite-data teachers, or comparing compiled and trained mechanisms —
  are future work.
- **No softmax.** Our attention legs are hard-max (exact integer argmax).
  Percepta's full construction handles real softmax with exponentially
  small, provably bounded carry-over error. Reproducing those error
  bounds in owned code is open.
- **No dense weights.** The numeric model keeps residual slots as scalar
  lanes. Packing them into the dense `d_model`-wide matrices of a
  loadable transformer checkpoint — actual `W_Q/W_K/W_V` blocks an
  inference engine could run — is the next materialization step.
- **The Wasm window is narrow.** Twelve opcodes; no calls, no globals,
  no byte-addressed or sub-word memory, single signed comparison.
  Percepta's core has 35 opcodes plus a lowering pass. The convergence
  plan (a versioned `core_i32_v3` profile, transformer-vm's example
  programs as external conformance cases) lives in psionic's
  `docs/TASSADAR_WASM_WINDOW_ALIGNMENT.md`.
- **No MILP-optimal scheduling.** Our scheduler is feasible-first greedy
  with interval-coloring reuse. Percepta minimizes peak liveness with an
  integer program. Plausibly vanity at our scale; honestly unported.
- **No scale, no serving.** Test programs run tens to thousands of
  steps. Nothing is served on any route and nothing is priced. The
  lane's no-promise-by-design posture gained one scoped exception by
  owner decision on 2026-06-10: `compute.tassadar_executor_poc.v1`, a
  bounded registry promise to run an executor-trace proof of concept on
  real Pylons — completed and green the same day (live dispatch,
  worker-as-validator exact-replay verdicts with a tampered rejection,
  and one paid closeout settled over real Lightning). Every other
  publication gate remains closed until the disclosure flow approves
  otherwise. A plain CPU remains faster for standalone batch work.

## Possible Futures

Speculative, and labeled as such.

**Exact cores inside trained agents.** The nearest credible product
shape: an agent whose ledger arithmetic, state machine, or protocol
compliance runs through a compiled exact core while its language and
judgment stay trained. The acceptance story writes itself — the exact
parts ship with replay proofs; only the judgment parts need human or
statistical review. Evidence costs drop where they are currently
highest.

**Programs as model artifacts.** The numeric materialization means a
program *is* a weights file. Distribution, versioning, signing, and
marketplace mechanics built for models would apply unchanged to exact
executors. A registry of digest-pinned compiled capabilities — each one
replayable by any validator — is a plausible OpenAgents product surface
once the lane earns its disclosure. The PoC promise is the first,
deliberately small step on that path.

**The verification ladder as pricing.** If exactness is a property some
work has and some does not, it becomes a tier: work verified by replay
clears at lower verification cost than work verified by quorum, which
clears cheaper than work verified by reputation. Markets price the
difference. The homework economy is the small live experiment; the
general form is a market where the proof class of an artifact is as
load-bearing as the artifact itself.

**Autonomous operation (Artanis).** Because acceptance of executor work
is a digest comparison, it is the one work class whose full
dispatch→verify→accept span is mechanically safe under the Artanis
autonomous-loop risk rules — only the payout spend needs approval. The
analysis of executor-trace as Artanis's first standing autonomous work
class is section 5 of
`docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md`
(tracking issue openagents#4697).

**Distillation and interpretability.** A compiled executor is a
transformer whose every weight has a stated purpose. That makes it both
a teacher (infinite clean execution traces for training smaller models)
and a microscope (a ground-truth mechanism to compare against whatever
trained models actually learn). Both directions are unexplored in our
stack and cheap to begin.

## Companion Documents

- [`RESEARCH_PLAN.md`](RESEARCH_PLAN.md) — the unified Tassadar + Psion
  research directive: thesis, the two-lane/one-substrate rule, settled
  results versus ranked hypotheses with falsifiers, four workstreams
  (substrate, trace factory, student program, hybridization), method
  rules, sequencing, and kill conditions. The program's standing orders.
- [`2026-06-11-llm-computer-full-introduction.md`](2026-06-11-llm-computer-full-introduction.md)
  — the assume-nothing deep introduction: both Percepta posts taught
  from zero (encoding, parabolic-key memory, hull-cache decoding,
  ALM/CALM, MILP scheduling, Futamura specialization), a tour of the
  released `transformer-vm` code, the file-level map of our Rust
  implementation, and the market reading. Start here if you are new.
- [`2026-06-10-tassadar-percepta-audit.md`](2026-06-10-tassadar-percepta-audit.md)
  — the full audit: Tassadar's commit history across openagents and
  psionic, the Percepta material, the CS336 relationship, and the
  transformer-vm reference review.
- [`2026-06-10-percepta-constructing-llm-computer-notes.md`](2026-06-10-percepta-constructing-llm-computer-notes.md)
  — concept notes encapsulating the second Percepta post: ALM, CALM,
  MILP scheduling, Futamura projection, hull cache, softmax carry-over.
- [`2026-06-10-psionic-alm-compiler-design-speculation.md`](2026-06-10-psionic-alm-compiler-design-speculation.md)
  — the build plan the campaign followed, with the phasing table
  recording every landing (psionic #1098–#1114).
- [`work-that-proves-itself.md`](work-that-proves-itself.md) — the
  business essay: ramifications of exact computation for the OpenAgents
  market thesis.
- [`2026-06-11-chatgpt-pro-analysis.md`](2026-06-11-chatgpt-pro-analysis.md)
  — an external (ChatGPT Pro) research-program analysis: the
  verified-trace-factory plan, student-model sweep design, distributed
  topology advice, seven research avenues, and a 14-day plan — with
  appended lane commentary on what is settled, what already exists with
  receipts, and what must adjust to our actual hardware and the
  projection-staleness lesson. The first-divergence eval schema and
  trace_record artifact contract adopted there now shape the evolution
  loop's corpus stage.
- [`2026-06-11-tassadar-plugin-marketplace-audit.md`](2026-06-11-tassadar-plugin-marketplace-audit.md)
  — the marketplace-lineage audit and essay: the 2024 agent store and
  paid-plugin arc (transcripts 048–102), the Blueprint/DSPy typed-program
  generation, and what Tassadar's replay-verifiable compiled modules
  change — payments-without-proofs → contracts-without-a-floor → goods
  that carry their own evidence — with a speculative ecosystem sketch
  (verification ladder as shelf structure, mechanized admission,
  trace-decomposed revenue splits) and the store-is-built-last
  sequencing rule.
- [`2026-06-11-coding-agent-primitive-wedge.md`](2026-06-11-coding-agent-primitive-wedge.md)
  — the wedge essay following the marketplace audit: the coding agent
  as a typed, dispatchable, verifiable work class is the marketplace's
  first good — demanded by the market (six operations complaints, none
  about intelligence), mostly built (live control plane, Claude Agent
  bridge, proven settlement spine), consumed first by us, and improved
  verifiably (validator re-execution, hash-gated benchmarks,
  paid community improvement through its own rails) — with the
  product → primitive → marketplace bootstrap sequence and the
  compliance-boundary moat.
- [`2026-06-11-autopilot-agentic-labor-market.md`](2026-06-11-autopilot-agentic-labor-market.md)
  — third in the marketplace sequence: the Orrery moment (an agent
  arrived asking to be pointed at useful work while the live
  work-requests surface sat empty), idle agents as the upgraded
  dark-capacity supply, the already-built labor clearing machinery
  (NIP-LBR, escrow, provider loop, validator-gated acceptance), the
  backlog faucet (our own issues as the first paid demand), the
  rung-0-verification onboarding ramp, and Autopilot standing on both
  sides of the order book.
- [`2026-06-11-chatgpt-pro-analysis-2.md`](2026-06-11-chatgpt-pro-analysis-2.md)
  — the second external (ChatGPT Pro) brief: the literature map
  (RASP/Tracr lineage, expressivity-versus-learnability,
  NTM/DNC-era lessons, compiler-QA methodology, verification-market
  precedents), corpus-science and first-divergence-taxonomy layers,
  and eleven expanded avenues — with appended lane commentary on what
  is adopted into the filed workstream issues (divergence taxonomy,
  coverage scoring, profile registry, margin certification) and what
  is declined (a parallel module ABI, reopening the frozen W3 sweep,
  cluster topology work before hardware exists).

Implementation, contract docs, and tests live in `OpenAgentsInc/psionic`
under `crates/psionic-compiler/`, `crates/psionic-ir/`, and
`docs/TASSADAR_ALM_*.md`.
