# Building Our Own LLM-Computer: An ALM-Class Compiler In Psionic

Date: 2026-06-10

Status: **speculative design**. Nothing here is implemented, scheduled, or
claimed. This doc takes the construction Percepta published
(`2026-06-10-percepta-constructing-llm-computer-notes.md` in this folder)
and asks: what would our own version look like, built Rust-native into
Psionic, on the Tassadar surfaces that already exist? Implementation, if
it happens, belongs in the psionic repo and its own tracker; this doc is
the cross-repo design sketch. The Tassadar disclosure flow governs
anything that ever becomes public claim copy.

## The Question, Precisely

Percepta's pipeline is: **ALM** (a five-primitive abstract machine) →
**CALM** (a language over it) → **gate graph** (LookUp/ReGLU DAG) →
**MILP scheduler** (gates → layers, values → residual slots) →
**analytical weights** → a standard transformer that executes programs,
with a Futamura specializer that bakes a fixed program into the FFN.

Psionic's Tassadar lane already has large parts of the *destination* —
an executor transformer family, a hull-cache decode path, a frozen trace
ABI, served products, conformance discipline — but it reached them by
**hand construction**: the interpreter logic lives in carefully built
module families, not in artifacts emitted by a general compiler from a
small IR. What Percepta demonstrated is the *factory*. The question is
what the factory looks like in our stack.

## What Psionic Already Has (The Anchors)

Grounded against the current crate tree (49 crates):

- **`psionic-compiler`** — already self-described as "lowering and
  scheduling boundaries for Psionic," with 28 Tassadar modules: the
  article ABI (`tassadar_article_abi.rs`), the Wasm module surface
  (`tassadar_wasm_module.rs`), structured control
  (`tassadar_structured_control.rs`), trap/exception profiles, float
  semantics (`tassadar_float_semantics.rs`), the mixed numeric ladder,
  the universal-machine encoding
  (`tassadar_universal_machine_encoding.rs`), the module manifest /
  catalog / linker / overlap-resolution family, the internal-compute
  package manager, and the plugin packet ABI + Rust PDK. **This is the
  natural home for the IR and the scheduler.**
- **`psionic-models`** — the executor model family:
  `tassadar_executor_transformer.rs`, `tassadar_executor_attention.rs`
  (with per-layer hull posture: direct / fallback-only / refused — i.e.
  the capability honesty is already in the type system),
  `TassadarExecutorAttentionWeightBundle` (a weight container the
  compiler backend can target), `tassadar_article_transformer.rs`,
  `tassadar_call_frames.rs`, `tassadar_conditional_masking_executor.rs`,
  `tassadar_decompilable_executor.rs` (decompilability — the inverse
  direction Percepta doesn't have), and
  `tassadar_broad_family_specialization.rs` (a specialization surface
  already exists in bounded form).
- **`psionic-eval` / `psionic-runtime`** — the triple-parity discipline
  (CPU reference / reference-linear / hull-cache), the frozen trace ABI,
  proof bundles, and the article-class benchmark suite (MicroWasm,
  BranchHeavy, MemoryHeavy, LongLoop, Sudoku, HungarianMatching).
- **`psionic-serve` / `psionic-provider`** — the served
  `psionic.executor_trace` product and the provider-facing
  `TassadarCapabilityEnvelope`, ready to carry whatever the compiler
  emits.
- **The claim machinery** — TCM.v1 substrate model, the workload
  capability matrix, the disclosure flow, `served_publication_allowed`
  gates. Any compiler output inherits this posture for free.

What psionic does **not** have, and Percepta does: a small typed IR with
exactly five primitives; a general analytical weight *emitter* from that
IR; an optimizing layer/slot scheduler; and a mechanical
program-to-specialized-weights path exposed as one command.

## Proposed Architecture

Working name for the lane inside psionic: the **executor compiler**
(no new codename — Tassadar already names the profile; this is its
factory). Five components, mapped to crates.

### 1. The IR: `AlmGraph` in `psionic-compiler`

A typed DAG with exactly the five ALM primitives, as a Rust enum — and
nothing else, because the smallness of the primitive set is the entire
trick:

```rust
pub enum AlmGate {
    /// write_c(k, v) / read_c(q) — keyed channel memory (attention).
    LookUp { channel: ChannelId, query: ValueRef },
    /// Exact prefix sum over a channel (attention, uniform keys).
    CumSum { channel: ChannelId, value: ValueRef },
    /// a * ReLU(b) pairs composing products, steps, conditionals (FFN).
    ReGlu { a: ValueRef, b: ValueRef },
    /// Residual-stream wiring; free, no gate consumed.
    Linear { terms: Vec<(i64, ValueRef)>, bias: i64 },
    /// Token-embedding input.
    Input { field: InputFieldId },
}
```

Plus the derived-op library as *lowerings onto* these (equality,
comparison, select, integer multiply via the two-ReLU identity, latest-
write tie-breaking perturbations) — mirroring how
`tassadar_mixed_numeric_ladder.rs` and the lowering posture already
work. The append-only channel restriction (writes before reads) becomes
a **type-level or validation-pass invariant**, checked at graph
construction — this is exactly the kind of bounded contract the
workspace's invariant discipline wants modeled, and the ALM is small
enough to *actually* model: a bounded model checker over a toy ALM
(channels, cursor, a few values) verifying that scheduling preserves
read-after-write would be a real formal artifact, not aspiration.
Percepta lists "formally verifying the logic a transformer implements"
as an open direction; our INVARIANTS culture is positioned to get there
first on the IR level.

We likely skip CALM-the-surface-language initially. Rust builder APIs
over `AlmGraph` (the way `tassadar_structured_control.rs` already
expresses control constructs) give us authoring without designing a DSL;
a `.calm`-class text format can come later if module authors outside the
repo need it — which is a marketplace question, not a compiler question.

### 2. The frontend: Wasm → AlmGraph

Psionic already owns a Wasm semantic window (`core_i32_v2`,
`tassadar_wasm_module.rs`, structured control, trap/exception profiles,
the scalar-f32 semantics matrix with explicit refusals). The frontend
work is a translation pass from that window onto `AlmGraph`: instruction
fetch as LookUp against the program channel, the operand stack and
locals as channels with latest-write semantics, the instruction
pointer/stack depth/call depth as CumSums (`tassadar_call_frames.rs`
already models frames), arithmetic through the ReGLU lowerings.
Alignment task from the audit stands: diff our opcode window against
transformer-vm's 35-op core + lowering set, converge or document. The
refusal surfaces stay: anything outside the window refuses at the
frontend, not at runtime.

### 3. The backend: scheduler + slot allocator + weight emitter

The MILP is the most directly portable piece, and the problem shape is
small (the Wasm interpreter graph is thousands of gates, not millions):

- **Phase 1 — feasible-first.** A deterministic list scheduler over the
  4-phase layer structure (attention / persist / FFN / persist) with
  precedence + type constraints, then greedy interval-coloring for slot
  reuse with stale-slot subtraction emitted automatically. This gets a
  *correct* d_model, not a minimal one, with no solver dependency — and
  correctness is checkable against the evaluator (component 4).
- **Phase 2 — optimal.** Swap in a real MILP for peak-liveness
  minimization. Rust options: `good_lp` over HiGHS/CBC, `russcip`, or
  microlp for the small case; the objective and constraints are exactly
  Percepta's (precedence, type compatibility, co-location, minimize
  peak simultaneously-live values). Keep phase 1 as the always-available
  fallback and as the solver's correctness cross-check.
- **Emitter.** Schedule + slot map + head packing →
  `TassadarExecutorAttentionWeightBundle` rows: 2D lookup heads with
  parabolic key/query projections, uniform-average cumsum heads, ReGLU
  FFN banks, residual wiring, stale-slot subtractions. Digest-pin the
  artifact through the existing digest-bound program artifact path
  (phase-2 Tassadar bar) so every compiled model is
  content-addressed from birth. The per-layer hull posture
  (direct/fallback/refused) is *computed by the compiler* — lookup heads
  it emits are hull-direct by construction; anything else is declared
  honestly in the descriptor.

### 4. The evaluator: exact-arithmetic reference without weights

Port the `wasm-eval` idea: an `AlmGraph` interpreter in pure integer
arithmetic (`psionic-eval`), no tensors, no weights. This becomes the
fourth leg of the parity harness — **graph-evaluator / CPU-reference /
reference-linear / hull-cache**, all required to agree — and the
cheapest CI gate for compiler changes. It is also the conformance
instrument for cross-validating against transformer-vm's reference
traces (same C examples, same expected output), giving the compiler an
external bar the day it produces its first weights.

### 5. The specializer: programs into weight modules

The Futamura step is mechanical once 1–3 exist: for a fixed N-instruction
program, emit the 2N shared ReGLU step-function neurons
(`1[cursor ≥ i]`), rewrite instruction-fetch LookUps into linear
combinations over them, drop the program channel, and emit a specialized
bundle (d_ffn grows O(N), d_model barely moves). Here psionic is
*already ahead of the post* on packaging: the
module manifest / catalog / linker / overlap-resolution family and the
internal-compute **package manager** in `psionic-compiler` are exactly
the plumbing a library of compiled weight-modules needs, and
`tassadar_broad_family_specialization.rs` plus the plugin packet
ABI + Rust PDK give the admission and receipt shape. The marketplace
implication from the audit (compiled, digest-pinned, conformance-tested
weight modules as tradeable artifacts) would be realized as: specializer
output → module manifest → catalog entry → NIP-DS-class listing on the
OpenAgents side, with replay-verification before purchase clears.
Module *linking* (composing several specialized banks in one model) is
the research question Percepta hasn't published on — and
`tassadar_cross_profile_link_compatibility.rs` +
`tassadar_module_linker.rs` show psionic has been preparing for it.

## What We Would Do Differently (Not Just Port)

1. **Invariant-first.** Model the ALM and the scheduler's correctness
   conditions formally (bounded checker over read-after-write, slot
   lifetime disjointness, stale-subtraction completeness) before
   optimizing. Percepta ships caveats; we ship caveats plus a checked
   model. This is the lane where our formal-verification culture has an
   actually-bounded state space to bite on.
2. **Decompilability as a first-class output.** Psionic already has
   `tassadar_decompilable_executor.rs`. A compiler whose artifacts carry
   their own gate-graph provenance can *prove* what logic a weight
   bundle implements by reverse-mapping — the audit story for a
   weight-module marketplace, and something the Percepta release does
   not attempt.
3. **Refusal-complete frontend.** Their subset-plus-lowering is implicit
   in code; ours is an explicit semantic window with typed refusals
   (float semantics matrix, trap profiles). Every unsupported construct
   refuses with a reason at compile time.
4. **Receipts at every stage.** Compile receipts (graph digest, schedule
   digest, bundle digest, parity verdicts) flow into the existing proof
   bundle / capability matrix machinery, so a compiled artifact's claim
   posture is machine-derived, never asserted.
5. **Hard-max and softmax tracked as postures.** The construction
   carries to softmax with exponentially small error; the capability
   matrix should track hard-max-exact vs softmax-approximate as separate
   verdicts per workload rather than picking one globally.

## Phasing (Speculative, Disclosure-Gated)

| Phase | Deliverable | Proof |
|---|---|---|
| E1 | `AlmGraph` IR + validation passes + exact evaluator | evaluator passes article-workload fixtures; ALM invariant model checked |
| E2 | Backend phase 1 (list scheduler + interval coloring + emitter) for one tiny workload | 4-leg parity on that workload; digest-pinned bundle |
| E3 | Wasm frontend over `core_i32_v2` window | transformer-vm example set cross-validated trace-for-trace |
| E4 | MILP optimal scheduling | d_model parity-or-better vs phase 1 on the interpreter graph; solver verdict cross-checked |
| E5 | Specializer + module manifest integration | one specialized bundle, replay-verified, cataloged |
| E6 | Served compiled artifacts via `psionic.executor_trace`; provider envelope rows | capability matrix rows derived from compile receipts |

Every phase lands behind the existing publication gates; none of it
creates public claim copy until the disclosure flow approves, and no
OpenAgents registry promise exists or should exist before E3 at minimum.

## Open Questions (Honest List)

- **Solver dependency policy.** Does psionic want a MILP solver in its
  dependency tree at all, or does the phase-1 heuristic + a checked
  bound suffice indefinitely? (The graphs are small; optimality may be
  vanity.)
- **CALM-or-not.** Builder APIs vs a surface language is really a
  question about who authors modules. If the marketplace lane matures,
  external authors need *something* — but it could be "write Rust
  against the PDK" rather than a new DSL.
- **Numeric envelope.** The IR is exact-integer; the f32 semantics
  matrix governs anything beyond. Does the compiler refuse floats
  entirely at first (cleanest) or admit the bounded scalar-f32 window?
  Refuse first, admit by evidence.
- **Hybrid hooks.** Where does the emitter leave seams for the
  differentiable-hybrid future (reserved head ranges inside a larger
  model rather than a standalone executor)? Design the bundle format so
  embedding-into-host is a layout decision, not a rewrite — but build
  nothing hybrid until the CS336 pretraining lane can actually train
  the host.
- **Memory growth.** Percepta names token-growing memory as a caveat;
  psionic's locality scratchpad work
  (`tassadar_locality_scratchpad.rs`) is the existing surface to point
  at it. Out of scope for E1–E6; named so it is not forgotten.

## Pointers

- Concept notes this doc builds on:
  `2026-06-10-percepta-constructing-llm-computer-notes.md`
- Audit (lane history, current psionic state):
  `2026-06-10-tassadar-percepta-audit.md`
- Business ramifications: `work-that-proves-itself.md`
- Reference implementation (read-only; port ideas, never vendor):
  `projects/repos/transformer-vm`
