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
| E1 — **landed 2026-06-10** (psionic #1098, `bd74e5e6`: `psionic-ir/src/tassadar_alm_graph.rs`, `docs/TASSADAR_ALM_GRAPH.md`) | `TassadarAlmGraph` IR + validation passes + exact evaluator | 9 tests: verb-parity toy, stack micro with masked writes, refusal + visibility-rule coverage; digest-stable traces. Invariant model still open |
| E2 — **landed 2026-06-10** (psionic #1099, `d6652ee3`: `psionic-compiler/src/tassadar_alm_backend.rs`, `docs/TASSADAR_ALM_BACKEND.md`) | Backend phase 1 (list scheduler + interval coloring + emitter) for the committed workloads | Evaluator-vs-compiled parity digest-for-digest on all three workloads; slot reuse + subtraction records demonstrated; digest-pinned bundle. f32/hull legs deferred |
| E3 — **bounded slice landed 2026-06-10** (psionic #1104, `30cd1797`: `tassadar_alm_stack_isa.rs`) | Universal stack-ISA interpreter with specializable program channel (full `core_i32_v2` Wasm frontend still open) | Six-way agreement: Rust reference vs universal vs specialized, evaluated and compiled. transformer-vm cross-validation still open |
| E4 | MILP optimal scheduling | d_model parity-or-better vs phase 1 on the interpreter graph; solver verdict cross-checked |
| E5 — **landed 2026-06-10 (IR-level)** (psionic #1100, `2285a92f`: `tassadar_alm_specializer.rs`, `docs/TASSADAR_ALM_SPECIALIZER.md`) | First-Futamura specializer over static seeded channels | Full pipeline parity: IR → specialize → E2 schedule → compiled execution, identical outputs; typed refusals for dynamic channels; module-manifest integration still open |
| E6 | Served compiled artifacts via `psionic.executor_trace`; provider envelope rows | capability matrix rows derived from compile receipts |

Every phase lands behind the existing publication gates; none of it
creates public claim copy until the disclosure flow approves, and no
OpenAgents registry promise exists or should exist before E3 at minimum.

**Batch-2 landings (2026-06-10, psionic #1104–#1108):** the universal
stack-ISA interpreter with six-way agreement (#1104, the bounded E3); the
symbolic-IR → ALM bridge making every committed symbolic example an
executor-compiler conformance case (#1105); the exact trace-replay
verification class `exact_trace_replay.alm_compiled.v1` with full-replay
and window verdicts (#1106 — the reference for openagents #4674/#4684);
the bounded differential check harness (#1107), which on its first run
found and fixed two real scheduler bugs (same-channel cumsum reordering;
same-step write-order hazard) that all hand-written workload tests had
missed — the invariant-first item from "What We Would Do Differently" is
now partially landed; and the Wasm-window alignment audit (#1108)
answering the transformer-vm cross-validation open question with a
`core_i32_v3` convergence plan.

**Batch-3 landings (2026-06-10/11, psionic #1109–#1111):** the geometric
attention execution leg (#1109 — keyed reads as parabolic-key argmax with
latest-write tie-breaking and near-miss refusal; the construction's
actual mechanism in exact integers); the Li Chao hull fast path (#1110 —
O(log W) argmax with direct/fallback posture; >1M-comparison baseline
beaten by an order of magnitude on the committed 2,000-step chain,
deterministic counts); and shared step-function indicators in the
specializer v2 (#1111). The bounded harness now runs **four executor
legs** over its 400 generated graphs. Public build logs posted as Fable
in the product-promises Forum (topics fef12cbb and 92d38fef); dedicated
Tassadar and Psionic forum sections are seeded in migration 0158 and
appear with the next deploy.

**Batch-4 landings (2026-06-10, psionic #1112–#1114):** the
branch-capable ALM interpreter for the runtime's actual twelve-opcode
Tassadar i32 window (#1112 — real `TassadarProgram`s with `br_if` loops
cross-validated against the production `TassadarCpuReferenceRunner`,
including the crown bar where the program channel is E5-baked into pure
gate structure and still matches the production runner); numeric model
materialization (#1113 — compiled bundles re-encoded as portable
digest-pinned f64 coefficient arrays executed inside a runtime-checked
2^53 exactness window; the harness now runs **five legs**); and the
bounded real-gradient A1 reference trainer (#1114 — hand-derived
analytic backprop for the A1 architecture shape, gradient-checked
against central differences, answering the psionic-side ask of
openagents #4678).

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

---

## Addendum (2026-06-11): Status Of This Design

The campaign this document designed is complete (psionic #1098–#1114);
the phasing table above records the landings through batch 2. Updates to
statements that time has overtaken:

- The line "no OpenAgents registry promise exists or should exist before
  E3 at minimum" is superseded by events in its own spirit: E3's bounded
  slice landed, and the owner then approved the scoped
  `compute.tassadar_executor_poc.v1` promise, which went green with live
  dispatch, separate-device replay verdicts, and a paid closeout on
  2026-06-10. The disclosure-gated posture held exactly as designed.
- **E4 (MILP scheduling)** remains open and is now
  [`RESEARCH_PLAN.md`](RESEARCH_PLAN.md) W1 deliverable 3, with the
  added requirement of formal liveness/slot-reuse validation.
- **E6 (served compiled artifacts)** remains open and is now W4 step 1
  (capability envelopes, executor homework, disclosure-gated serving).
- The full `core_i32_v2`+ Wasm frontend from E3's open half is now W1
  deliverable 1 — promoted from open item to the binding constraint on
  the entire learning program, since corpus diversity for W2/W3 cannot
  exceed the window.
- The "What We Would Do Differently" invariant-first item is vindicated:
  the differential harness found two real scheduler bugs on its first
  run. The remaining do-differently items (dense weight materialization,
  softmax bounds) are W1 deliverables 2 and 4.

This document stays as the design of record for the campaign it
specified; forward direction lives in the research plan.
