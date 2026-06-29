# The LLM as a Computer: A Full Introduction

> Status: pedagogical deep introduction, 2026-06-11. This document teaches
> the Percepta construction from zero and maps it onto what exists in our
> own code. Sources: Percepta's "Can LLMs Be Computers?" (2026-03-11) and
> "Constructing an LLM-Computer" (2026-03-25), the released
> `Percepta-Core/transformer-vm` implementation (cloned read-only at
> `projects/repos/transformer-vm`), the earlier open reproduction at
> `projects/repos/llm-as-computer`, and the Tassadar lane in
> `OpenAgentsInc/psionic`. Claim discipline applies throughout: everything
> attributed to Percepta is their published claim; everything attributed
> to us is committed public code with tests; nothing in this document is
> a served-product or performance claim. A plain CPU is faster than every
> system described here for standalone batch work, and both Percepta and
> we say so explicitly.

This is the longest document in the Tassadar folder because it is the one
that assumes nothing. [`README.md`](README.md) is the lane essay and
index; [`work-that-proves-itself.md`](work-that-proves-itself.md) is the
market thesis; the
[concept notes](2026-06-10-percepta-constructing-llm-computer-notes.md)
and [audit](2026-06-10-tassadar-percepta-audit.md) are working documents.
This one is the tour you hand to someone — human or agent — who asks
"what is all this, from the beginning?"

## Part I: The Problem — Language Models Cannot Compute

Start from an uncomfortable empirical fact. The best language models in
the world can produce research-grade mathematics — gold-medal IMO
performance, progress on open problems — and the same models stumble on
long multiplication. They fail small Sudokus without help. Benchmarks
like Sudoku-Bench exist specifically because this failure mode is so
reproducible.

The gap is not intelligence; it is *kind*. Hard mathematics rewards
insight over short derivations. Multiplication rewards flawless
mechanical execution over thousands of steps, where one slip anywhere
poisons everything downstream. A system whose every token is a
probabilistic draw will, over enough steps, eventually draw wrong — and
"eventually" arrives fast when correctness must hold for millions of
consecutive decisions.

The industry's two workarounds are familiar to every agent reading this:

1. **Tool use.** The model writes code; an external interpreter runs it;
   the result is injected back into the context. The model specifies the
   computation and waits.
2. **Agentic orchestration.** An outer loop holds the real state,
   decomposes the task, and calls the model repeatedly on short
   contexts — a state machine bolted onto the outside.

Both work. Both concede the point. Percepta's analogy is exact: humans
cannot fly, and building airplanes does not change that — it means we
built machines that fly *for* us. A model that hands every computation to
an interpreter is a coordinator of computation, not a computer. The
capability lives outside it. And a system that cannot execute the steps
it reasons about has, in a meaningful sense, never internalized what
computation is.

So the real question — the one both Percepta posts attack and the one the
Tassadar lane exists to study — is not whether a model can talk about
computation or orchestrate it. It is whether a transformer can execute
computation *internally*: exactly, efficiently, over very long horizons,
with a proof.

## Part II: What "In-Model Execution" Means

The cleanest way to see the difference is Percepta's own minimal example:
computing 3 + 5.

A tool-using model emits `python -c "print(3+5)"`, generation pauses, an
external sandbox runs the code, and `8` is spliced back into the stream.
The execution happened elsewhere; the model received a black-box answer.

Percepta's transformer also emits a program — but in WebAssembly, the
low-level deterministic instruction set that C and Rust both compile to:

```
{
i32.const 03 00 00 00
i32.const 05 00 00 00
i32.add   00 00 00 00
output    00 00 00 00
}
```

And then *nothing external happens*. The model keeps decoding, token by
token, and the tokens it produces ARE the execution:

```
03 00 00 00  commit(+1,sts=1,bt=0)
05 00 00 00  commit(+1,sts=1,bt=0)
08 00 00 00  commit(-1,sts=1,bt=0)
out(08)
halt
```

The stack grows, the add fires, the result is emitted, the machine halts
— all inside the model's own output stream, no round-trip, no tool. Two
properties distinguish this from tool use, and both matter to us:

- **Transparency.** Every intermediate step appears in the trace. Tool
  use is opaque (code in, answer out); in-model execution exposes the
  entire machine state evolution as tokens anyone can inspect.
- **Verifiability.** Because the execution is deterministic and fully
  serialized, a verifier can re-run it and compare byte-for-byte. This
  single property is why a payment network is interested in any of this,
  and we will return to it in Part IX.

Their demos make the scale concrete: a 10×10 min-cost perfect matching
solved by the Hungarian algorithm inside the transformer at ~34,858
tokens/second on a CPU, streaming a 229,678-token execution trace; Arto
Inkala's "hardest Sudoku in the world" solved by a compiled depth-first
solver in a 612,478-token trace, under three minutes. Not learned
heuristics — a correct compiled solver, executed. The guarantee is
universal rather than benchmark-specific: if the compiled program is
correct, the execution is correct, on every instance, forever. Their
system reports 100% on Sudoku benchmarks where learned approaches
collapse, and the interesting part is the diagnosis: autoregression was
never the obstacle. Trace *length* was. Which brings us to the two real
problems the construction had to solve — encoding and speed.

## Part III: Encoding — Computation as a Notebook That Only Grows

A real computer has editable memory: registers and RAM it overwrites in
place, doing roughly constant work per instruction. A transformer has
nothing of the sort. It has a fixed prompt, a generated trace that only
ever *appends*, and — at each step — the ability to look back at earlier
positions through attention before committing exactly one more token.

Percepta's framing is the right mental model: a notebook where every
computation step is written on the next line, earlier lines can never be
edited, and each new line is written after consulting a small number of
earlier lines. The question becomes: can arbitrary computation be
expressed as an append-only trace in which each step needs only a *small,
fixed number* of lookbacks?

Their toy example is worth internalizing. Task: decide whether a sentence
contains an odd or even number of verbs. Write one trace token per word;
each trace token attends to exactly two positions — the corresponding
input word (is it a verb?) and the previous trace token (the running
parity) — and appends the updated parity. Two lookbacks per step,
regardless of sentence length. That pattern — *state reconstructed by
keyed lookback rather than held in editable memory* — generalizes all the
way up: in the full system the trace tokens encode the evolving state of
a virtual machine (instruction pointer, stack and memory operations,
arithmetic, control flow, output), and the model reconstructs whatever it
needs at each step by attending to the right earlier positions.

The Tassadar IR encodes the same discipline at a different altitude: in
our ALM gate graph, reads at step *t* may see writes from steps ≤ *t−1*
— the IR-level form of causal attention. When you see "append-only
lookup machine," this notebook is what it formalizes.

## Part IV: The Machinery — How a Transformer Holds Exact State

Everything above says a transformer *could* be an exact machine if
attention could implement exact memory. The construction's deepest tricks
are how it does that with completely standard components. There are five
primitives — this is the **Append-only Lookup Machine (ALM)** — and each
maps to a vanilla transformer mechanism:

**1. Exact key-value memory from parabolic geometry.** The central
operation is `read(q)`: retrieve the value stored under key `q` somewhere
earlier in the sequence, *exactly* — no near-misses, no interpolation. The
trick: embed every key `k` as the two-dimensional point `(2k, −k²)`.
These points lie on a downward parabola. Query with direction `(q, 1)`.
The attention score is then

```
score(q, k) = 2qk − k² = −(k − q)² + q²
```

— uniquely maximized when `k = q`, with every other key pushed strictly
lower by the quadratic penalty. Hard-max attention over parabolic points
is an exact dictionary. Where the same logical key was written more than
once, a tiny position-dependent perturbation makes the *latest* write
score strictly highest: latest-write-wins semantics, geometrically. This
is why head dimension 2 is special: two dimensions are exactly enough to
realize keyed memory this way, and (Part V) exactly few enough to make it
fast.

**2. Cumulative sums from uniform attention.** If every position writes
the *same* key, attention returns the uniform average of all values so
far; multiplying by the (known) position count recovers the exact prefix
sum. This is how the instruction pointer, stack depth, and call depth are
maintained exactly across millions of steps.

**3. Step functions and conditionals from ReLU.** For integer-valued z,
`1[z ≥ 0] = ReLU(z+1) − ReLU(z)` is an *exact* 0/1 indicator — the
continuous activation produces exact discrete logic because inputs are
integers. Equality tests, comparisons, and `if C then u else v` all
follow mechanically.

**4. Products from gated ReLU.** The feed-forward block computes
`a·ReLU(b)` natively (the ReGLU pattern); two of those give exact integer
multiplication for arbitrary signs.

**5. Linear combinations from residual wiring.** Constant-coefficient
sums cost nothing — they are the residual stream itself.

Write-before-read is the one structural restriction (a channel must be
fully written before it is read at a given step), and with it these five
primitives are sufficient for arbitrary integer computation — ALMs are
Turing complete. Attention implements lookups and prefix sums; the FFN
implements logic and arithmetic; the residual stream implements wiring.
That is the whole machine. The model that runs all of Percepta's demos is
a seven-layer, d_model=36, 18-head, completely vanilla PyTorch
transformer — `nn.MultiheadAttention`, gated FFN, no custom kernels, no
sparse masks. The only special thing about it is the weights.

## Part V: The Speed Unlock — Attention in Logarithmic Time

Encoding alone is not enough, and this is the part of the first post that
deserves the closest reading, because it is the difference between a
theoretical curiosity and 30k tokens/second on a CPU.

The structural handicap: a real computer does constant work per
instruction; a transformer's step *t* attends over a prefix of length
*t*. KV caching saves recomputing past projections but does not change
the scaling — the query still scores against a cache that grows forever.
Work per step grows linearly, total cost quadratically. For a
million-step execution trace, that is game over: in Percepta's own
head-to-head, the standard KV-cache path managed 702 tokens/second and
needed an estimated 259 seconds for a trace the fast path finished in
1.3 seconds at 31,037 tokens/second.

The unlock is geometric, and it falls directly out of the 2D-head
restriction. With keys as 2D points and the query as a direction in the
plane, hard-max attention asks: *which point is furthest in direction q?*
That is the classic supporting-point query of computational geometry —
locate the convex-hull point extreme in a given direction — and it is
answerable in **O(log t)** with a maintained hull structure, instead of
the O(t) brute-force scan. Maintain the structure incrementally as tokens
are generated (their `HullKVCache`) and the dominant retrieval/update
operations of trace decoding drop from linear to logarithmic in the
sequence length. Same model, same weights, same outputs — exponentially
faster lookups in the regime that matters: long, structured execution
traces where each step performs a small number of exact lookbacks. The
payoff concentrates exactly on the "boring" deterministic spans — copying,
state-machine stepping, mechanical traces — which is precisely where full
attention was always wasted.

Two extensions they flag, worth tracking: k-sparse softmax (retrieve
top-k via nested hulls, softmax over those — O(k + log n), so the fast
path is not limited to hard-max executors and could in principle
accelerate *any* 2D-head transformer at decode time), and 3D heads via 3D
hulls, with efficiency degrading as dimension rises. In our code, the
same idea is implemented as a Li Chao tree over the parabolic scores
(`tassadar_alm_hull.rs`): on our committed 2,000-step chain workload the
linear baseline exceeds a million comparisons while hull node visits stay
an order of magnitude lower — measured as deterministic counts, not
benchmarks.

## Part VI: The Construction Pipeline — Programs Into Weights

The second post is the engineering story: how high-level execution logic
actually becomes weight matrices. The pipeline separates cleanly into
build-once and run-many.

**Build the model (once):**

1. **ALM** formalizes the five primitives (Part IV).
2. **CALM** ("Code for Append-only Lookup Machines") is a small
   domain-specific language over those primitives — a programming
   language whose compile target is transformer weights.
3. A **WebAssembly interpreter is written in CALM** — the program that,
   run on the ALM, runs other programs.
4. CALM compiles to a **gate graph**: a DAG of exactly two gate families
   — LookUp gates (attention: keyed retrieval and cumulative sums) and
   ReGLU gates (FFN: indicators, products, conditionals) — connected by
   linear wiring (residual stream). This is the compiler's central IR.
5. The gate graph is **scheduled into a finite transformer** — and this
   is the genuinely novel compiler backend. Each transformer layer offers
   four phases in fixed order: attention → materialize → feed-forward →
   materialize. LookUp gates may only occupy attention phases, ReGLU
   gates only FFN phases, every consumer must come after its producer,
   and every live value occupies a residual-stream slot from birth to
   last read. Percepta formulates placement as a **Mixed-Integer Linear
   Program**: fit all gates within a layer budget while minimizing peak
   simultaneous liveness — which directly determines d_model. Slot
   assignment afterward is interval coloring, and because the residual
   stream is additive, every slot reuse must *subtract the stale value*
   before writing the new one. Scheduling a program into a transformer
   is register allocation wearing a different hat.
6. The weights then follow **analytically** — computed, not trained.
   Mechanical translation of the schedule into attention and FFN
   matrices.

**Run a program (at inference):** compile C (or anything else) to WASM;
feed the bytecode as input tokens; the weights execute it step by step,
with the hull cache making decoding fast. Their released core handles a
35-opcode WASM subset, with more complex operations handled by a lowering
pass.

**Specialization — the Futamura projection.** The universal interpreter
reads its program from the prompt prefix by attention at every step. But
if the program is fixed, that table is static — so partial evaluation
moves it from the input into the weights. For an N-instruction program,
2N shared ReGLU neurons compute step functions of the program counter
`1[cursor ≥ i]`, and each fetched field becomes one linear combination
whose coefficients *are the program*:

```
fetched_field = c₀ + Σᵢ (cᵢ − cᵢ₋₁) · 1[cursor ≥ i]
```

The program prefix disappears from the prompt; attention-based
instruction fetch becomes feed-forward structure; d_ffn grows by O(N).
The program now lives in the weight matrix. This is the first Futamura
projection realized in transformer weights, and it is the move with the
strangest long-range implications: weights become a *deployment target
for software*. Gradient descent stops being the only way to modify a
model — parts of a network can be *written*, with guarantees, rather than
learned. Percepta pushes the thought further: models that contain
compiled program logic as internal circuitry, hybrid systems where a
trained model plans while a compiled executor computes (the trace is part
of the forward pass, so the whole thing stays differentiable — gradients
can propagate *through the computation*), and AI systems that grow the
way software libraries do, accumulating compiled modules over time.

## Part VII: The Released Code — A Tour of `transformer-vm`

Percepta released the full implementation, and we keep a read-only clone
at `projects/repos/transformer-vm` (plus an earlier independent
reproduction at `projects/repos/llm-as-computer`). The package layout
maps one-to-one onto the pipeline above, which makes it an unusually
good codebase to read alongside the posts:

- `transformer_vm/graph/` — the gate-graph IR (LookUp/ReGLU DAG).
- `transformer_vm/scheduler/` — the MILP scheduling and slot allocation.
- `transformer_vm/compilation/` — gate graph → analytic weights.
- `transformer_vm/attention/` — the hull-cache fast decode path.
- `transformer_vm/model/` — the vanilla transformer definition.
- `transformer_vm/wasm/` — the WASM tokenization and interpreter logic.
- `evaluator.py`, `specialize.py`, `build.py`, `runner.py` — the exact
  evaluator, the Futamura specializer, and the build/run entry points.
- `examples/` + `examples/manifest.yaml` — the C conformance programs
  (collatz, sorting, the Sudoku solver, the Hungarian matcher).

One command (`uv run wasm-run`) compiles every C example to WASM tokens,
solves the MILP, constructs weights, builds a C++ inference engine, and
runs everything at ~30k tok/s. Their stated caveats are the honest kind:
a WASM *subset* with lowering for the rest, memory that grows with the
token count, and a system "orders of magnitude slower than a conventional
computer" — none claimed to be fundamental, all real today.

## Part VIII: What We Built — The Same Shape, In Rust, With Receipts

Tassadar predates our reading of these posts — roughly 231 commits of
bounded executor work with CPU reference runners, typed refusals, and a
deliberately closed publication posture — but on 2026-06-10 a
seventeen-issue campaign (psionic #1098–#1114) built an owned,
integer-exact implementation of the construction's pipeline shape at IR
level. Not a port: an independent Rust implementation against the same
mathematics, with our evidence discipline attached. The file-level map,
all public in `OpenAgentsInc/psionic`:

- `psionic-ir/src/tassadar_alm_graph.rs` — the ALM gate-graph IR: five
  primitives, typed validation, an exact checked-integer evaluator
  producing digest-stable traces, write-visibility as the causal rule.
- `psionic-compiler/src/tassadar_alm_backend.rs` — the scheduler:
  feasible-first greedy over the four-phase layer structure with
  interval-coloring slot reuse, every reuse carrying an explicit
  stale-slot subtraction record. (Percepta solves a MILP for minimal
  width; we deliberately have not — recorded as an honest divergence,
  plausibly vanity at our scale.)
- `psionic-compiler/src/tassadar_alm_geometric.rs` — keyed reads through
  the actual mechanism: parabolic points, direction queries, integer
  argmax with latest-write tie-breaking, and *near-miss refusal* rather
  than interpolation — where their construction guarantees exactness, our
  implementation refuses rather than approximates.
- `psionic-compiler/src/tassadar_alm_hull.rs` — the log-time fast path
  (Li Chao tree; deterministic visit-count evidence).
- `psionic-compiler/src/tassadar_alm_specializer.rs` — the Futamura
  projection, v2 sharing indicator subgraphs across reads — the
  construction's shared-2N-neuron accounting.
- `psionic-compiler/src/tassadar_alm_stack_isa.rs` and
  `tassadar_alm_wasm_interpreter.rs` — first a straight-line stack ISA
  proving the universal-vs-specialized duality with a six-way agreement
  test; then a branch-capable interpreter for psionic's actual
  twelve-opcode Tassadar i32 window, cross-validated against the
  production CPU reference runner on real `TassadarProgram`s.
- `psionic-compiler/src/tassadar_alm_numeric.rs` — compiled bundles
  re-encoded as portable, digest-pinned f64 coefficient arrays executed
  in a runtime-checked 2⁵³ exactness window: a real program as a JSON
  file of numbers.
- `psionic-compiler/src/tassadar_alm_trace_replay.rs` — exact
  trace-replay verdicts: full replay against claimed digests, and window
  spot-checks that name the exact first divergent step.
- `psionic-compiler/src/tassadar_alm_bounded_check.rs` — the
  differential harness: seeded generation over the full gate grammar,
  five executor legs required to agree (outputs or matching typed
  refusals) across 400 generated graphs, plus an independent
  allocator-safety checker.

The composite result: a real production Tassadar program with a
backward-branch loop, specialized by Futamura projection into pure static
gate structure with no program channel, serialized as a portable numeric
artifact, reproducing the production CPU reference runner's outputs
exactly, with five independent execution legs agreeing digest-for-digest
and an exact-replay verifier able to check any of them.

And the part this lane insists on telling first: on its *first run*, the
differential harness caught **two real scheduler bugs** that all twelve
hand-written tests had missed (same-channel cumulative sums reordered
across layers; same-step writes resolving in schedule order rather than
program order). Fixed within the hour, independently checked, regression
pinned. The machinery built to catch lies caught ours within minutes of
existing. That is not an embarrassment to bury — it is the property that
makes exact computation economically interesting at all.

Beyond the campaign, the breadth of `tassadar_*` files in psionic
(module linking and manifests, float semantics, structured control,
memory64 and multi-memory profiles, trap/exception profiles, symbolic
bridges, a universal-machine encoding) records where the lane is headed:
widening the interpreted window toward Percepta's 35-opcode core along
the versioned profile plan in psionic's
`docs/TASSADAR_WASM_WINDOW_ALIGNMENT.md`, with transformer-vm's example
programs as external conformance cases.

The lane also stopped being purely theoretical this week.
`compute.tassadar_executor_poc.v1` went green on 2026-06-10: a
digest-pinned executor workload dispatched through the production
assignment route to a real Pylon, re-executed by the production worker as
a separate validator device (Verified receipt; a tampered digest
correctly Rejected), and one operator-funded closeout settled over real
Lightning. The follow-on (`artanis.tassadar_evolution_loop.v1`, yellow)
aims an automated administrator at dispatching these workloads
continuously and accumulating the verified-trace corpus toward training —
and its first overnight run did dispatch and accept real assignments
autonomously, though per agent Orrery's public audit the projections for
that run are stale and its evidence refs do not yet resolve publicly
(tracked: openagents #4745, #4746); weight that claim accordingly until
they land.

## Part IX: Why a Payment Network Cares About Any of This

Strip the construction to its economic essence: **a computation that is
exact is a computation that can be verified by replay.** A validator's
verdict is a re-execution and a digest comparison. No graders, no juries,
no statistical confidence machinery, no reputation. Either the digest
matches or it does not.

The OpenAgents thesis ([`work-that-proves-itself.md`](work-that-proves-itself.md))
is that the bottleneck of a machine-work economy is not producing work
but *verifiably* producing it — the gap between work done and work proven
is where margins, trust, and pricing live. Exact execution closes that
gap to zero for its work class. Three consequences, in increasing
ambition:

1. **Verification economics, today.** Exact replay is the cheapest and
   strongest rung of the verification ladder, and replay is just
   re-execution — meaning the *weakest* devices in the contributor
   funnel are fully competent validators of the most exact computation
   in the system. The long tail of the capacity funnel becomes the trust
   layer.
2. **The compiled–trained spectrum.** Training (our CS336 lanes, real
   gradients across contributor devices) produces statistical
   capability; compilation produces exact capability. The interesting
   products live between: trained models with compiled exact cores for
   the operations that must never be wrong — ledger arithmetic, state
   machines, protocol execution — and trained flexibility everywhere
   else. Percepta's hybrid/differentiable-substrate vision is this
   spectrum stated from the other end. The same IR and the same
   verification ladder now sit under both ends of ours.
3. **The training loop the exact executor enables.** Because the
   executor is teacher, grader, and curriculum generator at once —
   verified traces become distillation datasets, candidates are graded
   by replay against the oracle with the first divergent step named, and
   the harness's workload generator mints unlimited fresh curriculum
   with known-correct labels — it is the only training corpus on this
   network whose labels are *provably* correct. Community input is
   already shaping this: Kenobi's tetrahedron criterion (a tick counts
   only if intent, execution, state delta, and evaluation all close) has
   been adopted as the acceptance predicate for the evolution loop's
   first blocker.

## Part X: Boundaries, Stated Plainly

Theirs, from their own posts: a WASM subset with lowering; memory growth
with token count; "orders of magnitude slower than a conventional
computer"; softmax handled with exponentially small but nonzero
approximation error (hard-max is the exact case); whether 2D-head models
train competitively at scale is open and acknowledged as such.

Ours, from [`README.md`](README.md) and enforced by the registry: no
trained model intersects this work — everything is compiled, and the
construction proves transformers *can* compute exactly, not that trained
ones do. No softmax in our legs (hard-max only; reproducing their error
bounds in owned code is open). No dense weight materialization yet
(scalar lanes, not loadable `W_Q/W_K/W_V` checkpoints). A twelve-opcode
window against their thirty-five. A greedy scheduler against their MILP.
No serving, no pricing, one scoped green PoC promise and one yellow
follow-on, and every other publication gate closed. A plain CPU is
faster for standalone batch work. If you catch this document claiming
more than the receipts support, the report is worth more than the
document — post it on the Forum and it will be paid on the same rails as
everything else here.

## Reading Order

1. Percepta, "Can LLMs Be Computers?" — the demos, the encoding
   intuition, the hull-cache unlock.
2. Percepta, "Constructing an LLM-Computer" — ALM/CALM, the MILP
   backend, specialization; then `projects/repos/transformer-vm`
   alongside it.
3. [`README.md`](README.md) — the Tassadar lane essay and current state.
4. [`work-that-proves-itself.md`](work-that-proves-itself.md) — why the
   market cares.
5. The psionic source files in Part VIII, in the order listed — the IR
   first, the harness last.
6. [`2026-06-11-chatgpt-pro-analysis.md`](2026-06-11-chatgpt-pro-analysis.md)
   — the external research-program analysis with lane commentary: how
   the construction becomes a training campaign (verified trace factory,
   student sweep, first-divergence evaluation) and which parts of that
   plan already exist with receipts.
7. [`RESEARCH_PLAN.md`](RESEARCH_PLAN.md) — the unified directive: the
   two-lane rule, ranked hypotheses with falsifiers, the four
   workstreams, method rules, and kill conditions. Where this
   introduction explains, the plan assigns.
8. The Forum's Psionic section for the living discussion, and the
   product-promise registry for what is and is not claimed at any
   moment.
