# Percepta's "Constructing an LLM-Computer": Full Concept Notes

Date: 2026-06-10

Source: https://www.percepta.ai/blog/constructing-llm-computer
(Percepta Research Team, 2026-03-25) — the companion to "Can LLMs Be
Computers?" (2026-03-11). This is the construction post: it explains the
full program-to-weights pipeline and releases the complete implementation
(`Percepta-Core/transformer-vm`, cloned read-only at
`projects/repos/transformer-vm`), including an in-browser demo that
compiles arbitrary C and executes it inside a transformer (C-to-Wasm
compiler ~50 MB + model weights ~1.5 MB, fully client-side). A formal
theory write-up is announced as forthcoming.

This doc encapsulates every concept in the post and maps each to the
Tassadar lane. Status: reference notes; nothing here is an OpenAgents
capability claim.

## The Pipeline At A Glance

Two phases, deliberately separated:

**Build the model (once):**
1. Define the abstract machine (ALM) — the primitive operations a
   transformer can realize exactly.
2. Build a programming language (CALM) over those primitives.
3. Write a WebAssembly interpreter in CALM.
4. Schedule the interpreter's gate graph into layers via a MILP solver and
   emit weight matrices analytically.

**Run a program (at inference):**
1. Compile C → WebAssembly.
2. Feed the Wasm bytecode as input tokens (the program prefix).
3. The weights execute the program step by step; decoding is accelerated
   by the HullKVCache from the first post.

## Contribution 1: ALM and CALM — A Programming Language For Transformers

### The abstract machine (ALM)

The **Append-only Lookup Machine** formalizes exact integer computation in
transformer-native terms. Three abstraction levels: the machine (ALM), the
language (CALM — "Code for Append-only Lookup Machines"), and the
compiled intermediate representation (the gate graph).

Five primitives, each chosen because a transformer component implements it
*exactly*:

| Primitive | Form | Transformer realization |
|---|---|---|
| Read/Write | `write_c(k, v)`, `read_c(q)` on a channel c | attention (LookUp gate) |
| Cumulative sum | `cumsum_c(v)` | attention (uniform keys) + one multiply |
| Product | `a · b` | gated ReLU (ReGLU gate) in the FFN |
| Conditional | `if a then b else c` | ReGLU step functions |
| Linear combination | `c₁·x + c₂·y` | residual-stream wiring (free — no gate) |

The defining restriction (and the machine's name): **writes to a channel
must happen before reads from it** — computation as an append-only
history, never edited. This small primitive set is sufficient for
arbitrary integer computation; **ALMs are Turing complete**.

### How each primitive is represented

**Exact keyed memory via parabolic keys.** Embed key k as the 2D point
`k ↦ (2k, −k²)` — all keys lie on a downward parabola. Query with
direction `(q, 1)`; the attention score is

```
score(q, k) = 2qk − k² = −(k − q)² + q²
```

uniquely maximized at `k = q`, with every other key pushed strictly lower
by the quadratic penalty. This is *why 2D attention is special*: two
dimensions realize exact keyed memory geometrically while still
supporting the O(log t) convex-hull decode path.

**Latest-write semantics.** Hard-max attention averages tied keys, so a
small position-dependent perturbation is added to each key: among entries
with the same logical key, the latest written scores strictly highest.
(Mutable memory on an immutable tape, via tie-breaking.)

**Hard-max is convenient, not essential.** Standard softmax approximates
hard-max when scores are scaled by a large constant; the construction
carries over with **exponentially small approximation error**. This
matters: the result is not an artifact of an exotic attention variant.

**Exact prefix sums via uniform averages.** If every position writes the
same key, the head returns the uniform average of all values; multiplying
by (t+1) — the position is known from the embedding — recovers the exact
cumulative sum. This is how the instruction pointer, stack depth, and
call-stack depth are maintained exactly over millions of steps.

**Discrete logic from ReLU on integers.** For integer z:

```
1[z ≥ 0] = ReLU(z + 1) − ReLU(z)
```

Because inputs are integers, continuous activations yield *exact* 0/1
indicators; equality tests (`1[x = c]` as a difference of two step
functions), comparisons, and selection
(`if C then u else v = u·1[C] + v·(1 − 1[C])`) all follow mechanically.

**Products from gated ReLU.** The FFN computes `a · ReLU(b)` natively
(the ReGLU pattern); for arbitrary integers,
`a·b = a·ReLU(b) − a·ReLU(−b)`.

### The gate graph

A CALM program compiles to a DAG of exactly two gate families — **LookUp
gates** (attention: keyed retrieval, cumulative sums) and **ReGLU gates**
(FFN: step functions, indicators, products, conditionals) — connected by
linear wiring through the residual stream. This DAG is the compiler's key
intermediate representation: it describes exactly what happens at each
token position during autoregressive generation.

## Contribution 2: Gate Graphs → Weights Via Integer Programming

A gate graph is exact but is not yet a model: it must be embedded into
finite depth and finite width. Four questions: which operations in which
layer; which values in which residual slot; how many layers; how wide.

### The scheduling problem

Each transformer layer offers **four phases in fixed order**:

1. Attention (LookUp gates read from the sequence)
2. Materialization (attention outputs written to the residual stream)
3. Feed-forward (ReGLU gates compute local nonlinear logic)
4. Materialization (FFN outputs written to the residual stream)

This is simultaneously a **scheduling problem and a register-allocation
problem**: depth is driven by precedence constraints; width (d_model) is
driven by value lifetimes and peak simultaneous liveness.

### The MILP

Decision variables assign each operation to a layer and phase.
Constraints: **precedence** (consumers strictly after producers), **type
compatibility** (LookUp only in attention phases, ReGLU only in FFN
phases), **co-location** (operations sharing intermediates pinned to the
same layer). Objective: fit within a layer budget while **minimizing the
peak number of simultaneously live values** — the quantity that directly
sets d_model.

With the schedule fixed, every value has a lifetime (birth at compute,
death at last read); slot assignment becomes **interval coloring**, with
slots reused across non-overlapping lifetimes. One transformer-specific
subtlety: the residual stream is *additive*, so **reusing a slot requires
subtracting the stale value** before writing the new one.

Output: a completely standard transformer — multi-head attention with 2D
heads plus a gated FFN — whose weights follow mechanically from the
schedule, slot assignment, and head packing.

## Contribution 3: Programs → Weights (Interpreter And Specialization)

### The universal interpreter

A WebAssembly interpreter written in CALM, embedded in weights. At
inference, arbitrary C compiles to Wasm, is tokenized into the prompt,
and the transformer executes step by step. The correctness guarantee is
stated in decoding terms: **the next execution token is always scored
strictly higher than everything else**, so greedy decoding selects it —
determinism as a property of the score landscape.

### Baking programs into weights (partial evaluation)

When the program is fixed in advance, classical **partial evaluation**
(the first Futamura projection) moves the static instruction table from
the input prefix into the weights:

- For a program with N instructions, the specializer builds **2N shared
  ReGLU neurons** computing step functions of the program counter,
  `1[cursor ≥ i]` for each instruction index i.
- Each fetched instruction field then becomes one linear combination
  whose coefficients *are* the program:

```
fetched_field = c₀ + Σᵢ (cᵢ − cᵢ₋₁) · 1[cursor ≥ i]
```

- Cost shape: d_ffn grows **O(N)**; d_model grows only by the handful of
  fetched fields that must be materialized.

What changes versus the universal model: the program prefix disappears
from the prompt, and attention-based instruction fetch is replaced by the
ReGLU step-function lookup. The interpreter logic and token-by-token
execution are unchanged — only *where the static instruction table lives*
moves (input prefix → feed-forward weights). One can also skip Wasm
entirely and hand-write an adder, sorter, or Sudoku solver directly in
CALM and compile it to weights.

## Percepta's Own Caveats (Preserved Verbatim In Substance)

- The released construction is **complete but simplified** and not
  over-optimized; it is **orders of magnitude slower than a conventional
  computer**.
- It implements a **subset of WebAssembly**, handling more complex
  operations via lowering.
- **Memory requirements grow with the number of tokens.**
- They do not expect these to be fundamental blockers; each is
  improvable.

## Percepta's Announced Directions

1. Robust toolkits for **programming transformer weights**.
2. **Formally verifying** the logic a transformer implements.
3. Faster constructions and specialized attention mechanisms.
4. **Injecting programmatic logic into the training loop of large
   language models.**

Plus a formal theory write-up, forthcoming.

## Mapping To Tassadar

What the post confirms, sharpens, or changes for our lane (see the audit
in this folder for the lane itself):

- **ALM is the missing name for the substrate.** Psionic's executor lane
  has been building against "the article construction"; ALM/CALM gives
  the abstraction a clean three-level shape (machine / language / gate
  graph) worth mirroring in any Rust port: the gate-graph IR is the
  durable interface, and psionic's TCM.v1 substrate-model work is the
  same object approached from the universality side.
- **The append-only restriction is the receipt property.** "Writes
  before reads, history never edited" is what makes traces replayable
  and audit-native — the property the whole OpenAgents verification
  economy prices. It is now formalized as the *defining constraint* of
  the computation model, not an implementation detail.
- **Softmax carry-over closes an objection.** The construction surviving
  softmax with exponentially small error means the exact lane is not
  hostage to hard-max attention; psionic's capability matrix can track
  hard-max vs softmax-approximate postures as a refinement, not a fork.
- **The MILP framing upgrades the port target.** What we described in
  the audit as "gate-to-layer assignment" is precisely
  scheduling-plus-register-allocation with interval-coloring slot reuse
  and stale-slot subtraction. A Rust port should adopt that compiler
  vocabulary directly (and can swap pulp for any MILP/CP-SAT binding).
- **Specialization economics are now explicit.** O(N) d_ffn growth per
  baked program, tiny d_model growth — which makes the "compiled
  weight-module marketplace" idea costable: a module's size is linear in
  its instruction count, and modules are independent ReGLU banks, which
  is friendly to digest-pinning and composition audits.
- **Their direction #2 (formal verification of transformer logic) is our
  invariant discipline pointed at weights** — a natural collaboration or
  benchmark surface; their direction #4 (programmatic logic in the
  training loop) is the hybrid thesis from our essay, now explicitly on
  the originators' roadmap. The differentiation we can honestly claim is
  not the hybrid idea itself but the open-network execution economy
  around it: paid contributors, receipts, settlement, and from-scratch
  architectural sovereignty (the CS336 program). Track their formal
  write-up when it lands.
- **"Orders of magnitude slower than a conventional computer" is their
  sentence, not ours** — it confirms the audit's and essay's standing
  answer to "why not just use a CPU": the standalone executor is a
  substrate and benchmark; the value concentrates in composition,
  differentiability, and audit-native traces.

## Pointers

- Audit (history, psionic state, relations): 
  `2026-06-10-tassadar-percepta-audit.md`
- Business ramifications essay: `work-that-proves-itself.md`
- Reference implementation: `projects/repos/transformer-vm` (read-only;
  port ideas, never vendor)
- First post notes: covered inside the audit's Percepta lineage section

---

## Addendum (2026-06-11)

These notes were written from the second Percepta post. Both posts have
since been read in full and taught from zero — with the repo tour, our
implementation map, and the market reading — in
[`2026-06-11-llm-computer-full-introduction.md`](2026-06-11-llm-computer-full-introduction.md);
the program built on these concepts is directed by
[`RESEARCH_PLAN.md`](RESEARCH_PLAN.md). These notes remain the compact
per-concept reference for the construction itself.
