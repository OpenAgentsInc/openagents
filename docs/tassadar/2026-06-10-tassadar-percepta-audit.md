# Tassadar And Percepta Audit

Date: 2026-06-10

Status: full history audit of Tassadar across the `openagents` monorepo and
the `psionic` repo commit histories, including all Percepta-related
material, and how the lane relates to Psion, Psionic, Pylon, and Autopilot.

Disclosure posture: this audit follows the rules in psionic's
`docs/TASSADAR_PUBLIC_DISCLOSURE_FLOW.md` — public names only, claims
bounded to committed evidence, dependency markers preserved, refusal
language kept. The live Tassadar roadmap is private (the psionic repo's own
public bridge doc says so); this audit cites only what the two public repos
already record.

## What Tassadar Is

Tassadar is the **executor-capable bounded Psion profile** inside Psionic:
a transformer whose weights are compiled and/or trained so that it
**executes programs exactly inside its own inference loop** — every
instruction fetch and memory read is an attention operation, with no
external interpreter and no tool use. The transformer is the computer.

The naming rule (psionic `docs/PSION_EXECUTOR_PROGRAM.md`, the
`PSION-0001`/`#700` contract) keeps the lanes from being flattened:

- `Psion` is the umbrella learned-model family inside `psionic`.
- The generic compact-decoder `Psion` is a separate learned lane with its
  own route, refusal, serving, and training contracts (and is *not*
  exactness-claiming).
- `Tassadar` names the executor-capable bounded `Psion` profile and route
  family — currently the bounded article-transformer route and artifact
  family.

The name sits in the StarCraft Protoss family with Psionic, Psion, Pylon,
and Artanis.

## The Percepta Lineage

Percepta is the research origin. Their post "Can LLMs Be Computers?"
(Christos Tzamos et al., percepta.ai, 2026-03-11) demonstrates a computer
built inside a transformer: arbitrary C code compiles (via WebAssembly) to
tokens the model itself executes reliably for millions of steps, with no
external interpreter and no tool round-trip. The mechanism, in their own
structure:

- **A modern RAM computer, not a theoretical reduction.** A Wasm
  interpreter is implemented inside the transformer weights; each
  instruction maps to at most 5 tokens. Universality results existed for
  years (their citations and ours agree); the contribution is *practical*
  execution efficiency.
- **Computation as an append-only trace.** The prompt is the input, the
  generated tokens are the machine state's history (instruction pointer,
  stack ops, memory commits, control flow), and each step reconstructs
  state by attending back to a small fixed number of earlier positions.
  Nothing is ever edited; the notebook only grows — which is precisely
  what makes the trace audit-friendly.
- **The key unlock — exponentially fast attention.** Standard decoding
  pays Θ(t) per step against a growing prefix (quadratic total). They
  restrict *lookup* heads to dimension 2, which turns hard-max attention
  into the classic computational-geometry "supporting point" query: keys
  are points in the plane (memory addresses encoded as parabolic keys
  k_j = (2j, −j²), so query (i, 1) argmaxes exactly at j = i), and the
  furthest-in-direction point lives on the convex hull, retrievable in
  O(log t). Their HullKVCache-vs-KVCache benchmark on one 9,580-line
  trace: **31,037 tok/s vs 316 tok/s (~98×), 1.3s vs 258.9s**, on CPU.
- **The demos.** A 10×10 min-cost perfect matching solved by the Hungarian
  algorithm inside the model (439,194 trace tokens at 33,583 tok/s); Arto
  Inkala's "world's hardest Sudoku" solved in under 3 minutes
  (2,999,038 tokens) — with the pointed observation that their fully
  autoregressive system gets 100% on Sudoku benchmarks where learned
  approaches fail, so the real bottleneck was never autoregression, it was
  the cost of long traces.
- **The model is boring on purpose.** A vanilla PyTorch transformer —
  d_model=36, n_heads=18 (exactly 2D per head), 7 layers, standard
  `nn.MultiheadAttention`, gated FFN, no custom kernels. "The only thing
  that makes it special is the weights."
- **Extensions named in the post:** k-sparse softmax over nested convex
  hulls (O(k + log n)) so the fast path is not limited to hard-max; 3D
  hulls; and the open question of how capable 2D-head models are when
  *trained* at scale ("for Turing completeness, 2D attention is all you
  need").

Their "So what is next?" section matters as much as the mechanism, because
it names the design space OpenAgents cares about: (1) the fast path can in
principle accelerate **any** transformer with 2D heads at decode time;
(2) 2D-head models as a dedicated fast path, a fast/slow hybrid inside one
system, or a **speculative-execution** model whose cheap proposals a
regular-attention model verifies; (3) the hybrid LM+executor where the
language model plans and the executor runs algorithms — and because the
trace is part of the forward pass, **the whole computation is
differentiable**: gradients propagate through execution, making it a
trainable computational substrate rather than an external tool; (4)
**programs into weights** — weight compilation as a deployment target for
software and a second way to modify models beyond gradient descent; and
(5) **growing AI systems like software** — compiled modules accumulating
inside models the way libraries accumulate in software ecosystems:
"future AI systems will not just use software; they will contain it."

The workspace also tracks the public independent validation,
`projects/repos/llm-as-computer` — a compiled transformer executor
implementing a 55-opcode stack-machine ISA modeled on Wasm's i32 subset
(their benchmarks: 1.2M steps in 17ms on a Mojo backend) — whose companion
essay "The Free Computer" makes the economic argument that compiled CPU
execution is effectively free relative to attention-driven generation.
Psionic's `HullCache`/`HullKVCache` naming is a direct lineage marker.

### The Reference In Hand: `Percepta-Core/transformer-vm`

Percepta's own implementation is now cloned at
`projects/repos/transformer-vm` (manifest-tracked, Apache-2.0), and the
repo contains substantially more than the blog post shows — it is a
complete **program-to-weights compiler architecture**, not just a demo:

- **A computation-graph DSL as the compiler IR**
  (`transformer_vm/graph/core.py`): five primitive dimension types —
  `InputDimension`, `ReGLUDimension` (`ReLU(b) * a`), `PersistDimension`
  (materialize into a residual slot), `LookUpDimension` (attention
  retrieval from token history), `CumSumDimension` (cumulative sum via
  attention averaging) — composing into a DAG, with `reglu`/`stepglu`
  helpers building all conditional logic. Programs are authored against
  this IR; the transformer is the compile target.
- **A 35-opcode Wasm machine over that IR**
  (`wasm/interpreter.py`): byte-level arithmetic with carry propagation;
  stack, memory, locals, cursor, and call depth tracked via attention
  lookups and cumulative sums. A **lowering pass**
  (`compilation/lower.py`) rewrites MUL/DIV/AND/OR/XOR/SHL/SHR into the
  supported core — a deliberately small trusted base plus lowering, the
  same shape as a real compiler backend.
- **An MILP scheduler** (`scheduler/milp.py`): gate-to-layer assignment
  posed as mixed-integer optimization that *minimizes d_model* under a
  4-phase layer structure (attention / persist / FFN / persist) —
  compiling the DAG into the smallest transformer that holds it. Weight
  construction is fully **analytical** (`model/weights.py`); nothing is
  trained.
- **Two execution modes**: the universal interpreter (program bytecode in
  the input prefix; instruction-fetch heads look opcodes up — one model
  runs any program), and the **First Futamura projection**
  (`wasm-specialize`): the program is baked into the FFN weights and the
  prefix disappears — a per-program specialized model emitted as a weights
  binary. The "programs into weights" future the blog gestures at already
  has working public tooling.
- **A triple-reference correctness harness**: `wasm-eval` runs the
  computation graph directly with exact arithmetic (no weights),
  `wasm-reference` executes the Wasm directly, and `wasm-run` runs the
  transformer — three independent routes that must agree, which is
  precisely psionic's CPU/reference-linear/hull-cache parity discipline
  arrived at independently.
- **The performance substrate**: `attention/hull2d_cht.h` — a convex-hull-
  trick 2D hull with O(log n) insert *and* query, pybind11-bound, plus a
  standard O(n) cache kept as the honest baseline; and a standalone C++
  inference engine (`model/transformer.cpp`) with BLAS/Accelerate and
  sparse head projection at ~30K tok/s.
- **A C toolchain end to end**: clang wasm32 → Wasm MVP decoder → token
  prefix, with an auto-injected C runtime (`runtime.h`); examples include
  hello (printf), long addition, Collatz, Fibonacci (sscanf/printf), the
  Hungarian matcher, and the Sudoku solver from the post.

The companion construction post, **"Constructing an LLM-Computer"**
(Percepta Research Team, 2026-03-25), has since published the full theory
behind this repo and an in-browser demo (C compiled and executed inside a
transformer entirely client-side). It names the abstractions the repo
implements: the **Append-only Lookup Machine (ALM)** — five exact integer
primitives (keyed read/write, cumulative sum, product, conditional,
linear combination), Turing complete, with the defining append-only
restriction; **CALM**, the language over it, compiling to the
LookUp/ReGLU **gate graph**; the **MILP backend** as
scheduling-plus-register-allocation (four-phase layers, precedence/type/
co-location constraints, peak-liveness minimization for d_model,
interval-coloring slot reuse, stale-slot subtraction in the additive
residual stream); the greedy-decoding correctness guarantee (the next
execution token always scores strictly highest); and the specialization
math (2N shared ReGLU step-function neurons, O(N) d_ffn growth per baked
program). It also states their caveats plainly — a Wasm subset with
lowering, memory growing with tokens, and "orders of magnitude slower
than a conventional computer" — and announces four directions: weight-
programming toolkits, **formal verification of transformer logic**,
faster constructions, and **injecting programmatic logic into the
training loop of large language models** (the hybrid, now explicitly on
the originators' roadmap). Full concept notes and the Tassadar mapping
live in `2026-06-10-percepta-constructing-llm-computer-notes.md` in this
folder.

**How we would use and learn from it** (read-only reference per workspace
policy — study and port ideas, never vendor):

1. **Conformance cross-validation for Tassadar.** Same C programs, same
   expected traces: compiling transformer-vm's example set through
   Tassadar's lane and matching traces against `wasm-reference` output
   gives Tassadar an *external* conformance bar, exactly the role
   Stanford's `adapters.py` files play for the CS336 ports. Divergences
   become precise bug reports on either side.
2. **The compiler architecture is the port target.** Psionic has compiled
   executor bundles; what transformer-vm demonstrates is the *general*
   pipeline — graph IR → MILP layer scheduling → analytical weights — that
   turns one-off constructions into a compiler. A Rust port of the
   five-primitive IR and the scheduling pass (the MILP is small; pulp →
   any Rust MILP/CP-SAT binding) is the highest-leverage study item.
3. **The Futamura specializer is the weight-module factory.** The
   marketplace implication (#6 below) needs exactly this tool: program in,
   digest-pinned specialized weights binary out. transformer-vm proves the
   tooling shape; the psionic port would emit it as a signed, replayable
   artifact.
4. **Profile alignment.** Psionic's `core_i32_v2` Wasm profile and
   transformer-vm's 35-opcode-plus-lowering core should be compared
   opcode-by-opcode; converging the semantic windows (or documenting the
   exact diff) makes cross-validation in (1) clean and keeps our refusal
   surfaces honest.
5. **The CHT hull as a benchmark bar.** Psionic's retained hull-cache
   numbers (≥1.69× over reference-linear, ≤2.55× gap to CPU) and
   transformer-vm's O(log n)-insert CHT at ~30K tok/s are different
   metrics on different stacks; porting their incremental-hull approach
   and publishing a same-workload comparison is an honest A2-class
   kernels task.
6. **Pipeline hygiene worth copying as-is**: the manifest-driven example
   suite, the auto-built C++ engine, CI on every push, and keeping the
   brute-force cache in-tree as the permanent baseline — all match the
   receipt-shaped benchmark discipline psionic already wants.

The public OpenAgents framing is on the record in two transcripts:

- Episode 216 (`docs/transcripts/216.md`): "Psion is also going to be an
  executor model. We've more or less reproduced the Percepta paper...
  CPU compute added to the weights of models," alongside the Psion naming
  reveal and the pause-markets-to-focus-on-training decision.
- Episode 220 (`docs/transcripts/220.md`): "we've talked about this
  Percepta post. There's some pretty advanced shit that we can do that the
  other labs wouldn't be able to do" — Percepta-class execution named as a
  differentiator for the small-specialized-models thesis.

Psionic's committed Percepta record is
`docs/PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md` (`PSION-0705`/`#774`,
2026-03-30): one typed bounded closeout-status packet binding workload
truth, fast-path truth, and route-replacement truth. Retained truth at that
record:

- canonical model: `tassadar-article-transformer-trace-bound-trained-v0`
- canonical route: `tassadar.article_route.direct_hull_cache_runtime.v1`
- bounded closeout status: `green_bounded` (workload, fast-path, and
  route-replacement truths each `green`; the executor-style research branch
  explicitly `research_only`)
- retained `HullKVCache` fast path: ≥1.69× over the reference-linear
  baseline, with a ≤2.55× remaining gap versus direct CPU reference
- limitations kept explicit, starting with `arbitrary_c_or_wasm_not_claimed`

That is the honest shape of the "reproduced Percepta" claim: a bounded,
digest-pinned article-workload closeout — not arbitrary-program execution.

## History In The `openagents` Repo

Tassadar was **born in this repo**. In March 2026, psionic lived in-tree at
`crates/psionic/`, and the first Tassadar commits land here:

- `0363dff6c` (2026-03-16) "psionic: add Tassadar sequence dataset
  contracts" — `psionic-data/src/tassadar.rs` (597 lines) plus eval
  surfaces; the lane's first commit.
- The first experimental ladder follows over days: executor transformer
  family (`32534efa0`), next-token training and eval (`749ab9be3`), neural
  linear decode benchmark (`9664719ed`), phase-7 reference run persistence
  (`3090ccf70`), run telemetry (`371528388`), a first-run postmortem
  (`0f4cb16f0`), neural hull decode (`5826f0f4b`), a 9x9 (Sudoku) scale
  plan (`039d6db3e`), phase 12 boundary curriculum (`c4f732b87`), phase 13
  trainable-surface ablation (`55bb4e1a7`), executor-attention comparison
  lane (`b559ba580`), phase 14 promotion tooling (`9fc9b98e7`), compiled
  executor bundle (`7fb1983bc`), Hungarian-matching executor bundle
  (`52eced170`), fine-grained progress logging (`5ac5697f3`), promotion v2
  bundle (`ae059cc2f`), and a candid sequence of training-limit evidence:
  attention boundary training improvements (`cc18e7de4`), step-index
  boundary blocker diagnosis (`e8e076ddd`), transition-conditioned boundary
  adapter (`c13cf35a6`), joint-adapter plateau (`50c8fdb86`), and adapter
  saturation evidence (`fa80adcf8`). The arc is visible in the commit
  titles alone: try to *train* exact execution, hit the plateau honestly,
  and pivot weight production toward trace-bound/compiled routes.
- **Autopilot Tassadar Lab** (2026-03-17 → 03-25): the desktop app grew a
  replay-first Tassadar lab pane (`1cafa430e`), live lab sessions
  (`2ee586a51`), finished controls and persistence (`eefb356e7`), a widened
  run explorer (`77174ad7c`), and deferred pane loading (`446ec5d3c`, PR
  #4008) — an operator UI for replaying and inspecting executor runs inside
  Autopilot. That UI lived in `apps/autopilot-desktop` (later
  `apps/deprecated/autopilot-deprecated`) and was removed with everything
  else in the Bun rebuild (`f5919c766`).

When psionic was extracted to its own repo, the Tassadar lane moved with
it. The current monorepo footprint is references only: transcripts 203
(first on-air mention, 2026-03-17 era), 216, and 220; and two doc trails —
the workspace benchmark-systems audit
(`apps/pylon/docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`)
lists "Tassadar article, Sudoku, plugin conformance, universality, and
compiled weight eval reports" among the benchmark systems, and the Apple FM
first-backend audit (`packages/probe/docs/2026-06-07-apple-fm-first-backend-audit.md`)
records that "the Tassadar Apple FM plugin-session pilot proved a useful
controller pattern" that Probe's Apple FM backend then reused, citing the
psionic plugin-session audit and
`psionic-apple-fm/src/tassadar_post_article_starter_plugin_tools.rs`.

## History And Current State In `psionic`

The psionic repo is the implementation home: ~231 Tassadar-titled commits
and 20+ `docs/TASSADAR_*` documents, plus the `PSION_EXECUTOR_*` family.
The landed structure, per `docs/ARCHITECTURE.md` ("Tassadar Executor-Class
Lane") and `docs/ROADMAP_TASSADAR_INDEX.md`:

**The phase ladder (1 → 7D, all landed with committed artifacts):**

1. CPU reference fixture + exact parity harness.
2. Digest-bound program artifacts + model/program compatibility contracts.
3. Typed environment bundle + package-driven exactness benchmark suite
   (CPU and reference-linear baselines).
4. Emitted trace artifacts, runtime-manifest lineage, proof-bundle
   integration for replay-stable executor evidence.
5. `HullCache` fast-path decode with exact CPU/reference-linear/hull-cache
   equivalence on the validated acyclic subset and **typed refusal** for
   workloads outside it.
6. Machine-legible runtime capability reports + decode-selection
   diagnostics (direct / fallback / refused).
7. A/B/C/D: a served `psionic.executor_trace` product in `psionic-serve`
   (typed contracts, pull-driven trace streaming, typed refusals, served
   evidence bundles) plus the specialized `psionic.article_executor_session`
   surface; the widened `core_i32_v2` Wasm profile with article-class
   benchmark coverage (`MicroWasmKernel`, `BranchHeavyKernel`,
   `MemoryHeavyKernel`, `LongLoopKernel`, `SudokuClass`,
   `HungarianMatching`) at exact parity; a frozen long-horizon trace
   ABI/versioning decision; and a machine-readable workload capability
   matrix that keeps runtime-exact, fallback-only, compiled-exact, bounded
   learned, and partial learned-long-horizon postures separate per
   workload family.

**The universality program:** TCM.v1 substrate model declaration
(`ac174ceb`), universal-machine proof targets (`b1a033f7`), a universality
witness suite (`04ea60bd`), a minimal universal substrate gate
(`694878e9`), a universality verdict *split* (`a189aae7`), a
Turing-completeness closeout audit (`92e97913`), and an
article-equivalence blocker matrix (`56d83ac2`) — the claim machinery that
keeps "this substrate is universal in principle" separate from "this
served route executes these committed workloads exactly."

**The Wasm lane:** a frozen core-Wasm lane with a declared semantic window,
committed closure gate, public acceptance gate, and operator runbook — with
the current closure and public-acceptance verdicts **suppressed**
(`served_publication_allowed = false`), plus a bounded scalar-`f32`
semantics matrix (canonical quiet-NaN normalization, ordered comparisons,
CPU-reference-only posture, explicit refusal on `f64`, NaN-payload
preservation, and non-CPU fast-math).

**The plugin system:** post-article plugin manifests, packet ABI, receipts,
world-mount contracts; an operator-curated starter-plugin catalog and
runtime with one registry deriving identity/schemas/refusals/replay-class/
capability posture; an authoring contract; the Apple FM plugin session
(the weighted-controller admission pattern Probe later reused); and a
hardening tranche — platform threat model, audit invariants, anti-drift
stability, control-plane proof, machine-closure bundle.

**Training:** `./TRAIN_TASSADAR` is the frozen default train lane producing
`tassadar-article-transformer-trace-bound-trained-v0` (bounded
article-weight production), with a trained-v1 replacement report and
ablation records. The serving posture remains
`tassadar.internal_compute.article_closeout.v1` — benchmarked, bounded
internal computation under named profiles with explicit refusal surfaces.

## How Tassadar Relates To Everything Else

- **Psion**: Tassadar is one profile of the Psion umbrella. The generic
  compact-decoder Psion (the `./TRAIN` pretraining lane, the Qwen work, the
  epic-3 training issues) is deliberately *not* exactness-claiming;
  Tassadar is where exactness lives, and `PSION_EXECUTOR_PROGRAM.md` is the
  wall between them.
- **Psionic**: the implementation home across `psionic-data` / `-models` /
  `-train` / `-eval` / `-serve` / `-provider` / `-runtime` / `-apple-fm`,
  with fixtures under `fixtures/tassadar/`.
- **Pylon**: the connection is the capability-envelope pattern.
  `psionic-provider` wraps the executor capability publication into a
  provider-facing `TassadarCapabilityEnvelope` and a workload
  capability-frontier receipt — the same shape Pylon's GEPA capability
  envelope follows, and the published route for a future Pylon to advertise
  executor-class capacity without overclaiming. The shared
  benchmark-systems audit already treats Tassadar eval reports as one lane
  of the same benchmark/evidence system Pylon and Probe use.
- **Probe**: consumed the Tassadar Apple FM plugin-session controller
  pattern for its Apple FM backend — the first concrete cross-lane reuse.
- **Autopilot**: hosted the (now-removed) Tassadar Lab replay UI; any
  future operator surface would be rebuilt on the current product stack.
- **alpha**: the live roadmap, tranche definitions, and terminal-contract
  language live privately; psionic's `docs/ROADMAP_TASSADAR.md` is the
  public bridge and is deliberately subordinate to it.
- **The network economy (forward-looking, bounded):** executor work is the
  **most verifiable workload class we have**. Tassadar runs are
  deterministic, digest-pinned, trace-replayable, and already emit proof
  bundles — which makes them ideal commit-and-challenge homework for the
  verification layer described in the CS336 continuation audit
  (`docs/2026-06-10-cs336-distributed-homework-continuation-audit.md`):
  validators replay traces exactly instead of needing Freivalds-style
  probabilistic checks. The "Free Computer" economics also slot into the
  compute-revenue story: exact compiled execution is a sellable compute
  product whose verification cost is near zero. None of this is claimed as
  live — it is the natural continuation seam.

## Tassadar × CS336: Compiled Versus Trained Computation

Two questions decide how Tassadar and the CS336 program
(`docs/2026-06-10-cs336-distributed-homework-continuation-audit.md`) relate:
does Tassadar require training a model at all, and is the from-scratch
pipeline CS336 builds relevant to it? Short answers: **the exact lane
requires no training whatsoever — it is compilation — and the CS336
pipeline is what makes Tassadar's most valuable form (the hybrid) possible
at all.**

### Three different ways "training" touches Tassadar

The capability matrix already separates these postures (`runtime exact`,
`compiled exact`, `bounded learned`, `partial learned-long-horizon`), and
the distinction maps onto a real research lineage:

1. **Compiled-exact: no training, ever.** The core Percepta result — and
   the academic line before it — is weight *construction*, not learning.
   RASP ("Thinking Like Transformers", Weiss et al. 2021) defined a
   programming language whose programs map onto transformer operations;
   DeepMind/ETH's **Tracr** (Lindner et al. 2023) compiles RASP programs
   directly into the weights of a standard decoder-only transformer with
   zero training, as interpretability ground truth. Theory backs the
   ceiling: attention is Turing-complete (Pérez et al., JMLR 2021), and
   looped transformers can be programmed as general computers with
   constructed weights (Giannou et al., ICML 2023). Percepta's
   `transformer-vm` (March 2026) industrialized the idea: a WebAssembly
   interpreter compiled into weight matrices — four attention heads
   (program fetch, argument fetch, stack read, stack-pointer tracking)
   suffice for a minimal executor — with 2D parabolic attention keys and a
   convex-hull KV cache making each step an O(log t) lookup, millions of
   correct steps on a CPU. Exactness is *by construction*: there is no
   loss curve because there is nothing to learn. Psionic's compiled
   executor bundles and the hull-cache runtime live in this lane.

2. **Learned-exact: tried, and it plateaus.** The March 2026 experiments
   in this repo's history are a candid record of attempting to *train* a
   transformer into exact execution: next-token training on execution
   sequences, boundary curricula, transition-conditioned adapters — ending
   in the step-index boundary blocker, the joint-adapter plateau, and
   adapter saturation evidence. That matches the published limits of
   learned algorithmic execution (e.g. "Faith and Fate", Dziri et al.
   2023: transformers trained on compositional procedures fail to
   length-generalize). The pivot in the commit history — from "train it"
   to trace-bound weight production and compiled routes — is the program
   internalizing that result. The bounded-learned lane survives
   (`TRAIN_TASSADAR` → `tassadar-article-transformer-trace-bound-trained-v0`)
   but is honestly posted as *bounded*, never as exactness-by-learning.

3. **The hybrid: compiled circuits inside a pretrained model.** Episode
   216's phrasing — "CPU compute **added to the weights of** models" — is
   the third lane and the valuable one: a general language model that
   carries a compiled executor organ, so ordinary generation can route
   into exact internal computation (no tool call, no interpreter) and
   back. This is the lane that answers "why would a lab want this": the
   model gets calculator-grade arithmetic, exact state tracking, and
   verifiable inner computation at near-zero marginal cost (the "Free
   Computer" argument), instead of hallucinating arithmetic or paying for
   tool round-trips.

### Why the CS336 pipeline is exactly what the hybrid needs

The hybrid cannot be retrofitted onto someone else's frozen checkpoint —
it requires owning the architecture and the training loop. That is what
the CS336-to-Psionic port builds, assignment by assignment:

- **A1 (architecture ownership):** grafting compiled executor heads into a
  model means controlling head shapes, dimension layout, and where
  attention is exact versus learned. The A1 stack (our transformer, our
  attention, our RoPE) is the substrate the executor circuits get
  installed into. You cannot reserve 2D parabolic heads in a model you
  call over an API.
- **A2 (numerics and kernels):** compiled exactness is precision-brittle —
  psionic already maintains a scalar-`f32` semantics matrix with canonical
  quiet-NaN normalization and explicit refusal of non-CPU fast-math
  regimes, because a fused-multiply-add or fast-math kernel silently
  breaks exact execution. A2-level ownership of attention kernels is what
  lets exactness claims survive on real GPU/Metal backends instead of
  staying CPU-reference-only. The current ≤2.55× hull-cache-to-CPU gap is
  an A2-class kernel problem.
- **A3 (scaling the organ):** the open question for the hybrid is capacity
  cost — how much width/depth does the executor organ consume, and what
  does the host model lose or gain at each size? Psionic's
  workload-hardness taxonomy (explicit depth/width/recurrent/extra-trace-
  space budgets per workload family) is already a scaling-law-shaped
  object; the A3 sweep machinery (many small runs across (N, D) on the
  Pylon network) is the natural instrument for mapping
  hybrid-vs-plain frontiers empirically.
- **A4 (trace corpora):** the bounded-learned lane trains against
  execution traces. The homework network is a trace factory — every
  verified executor run, every CS336 training window, every rollout
  produces exactly the supervised data that trace-bound training consumes,
  already redacted and receipt-backed by the A4 data machinery.
- **A5 (routing is a post-training problem):** the hard learned problem in
  the hybrid is not execution (compiled, exact) but **routing** — when
  should the model hand a subproblem to its internal executor, and when
  must it refuse? Psionic's weighted-controller admission pattern (the
  Apple FM plugin session) is an early version. Training that router is
  GRPO-shaped: reward exact-verified internal computation, penalize
  unrouted arithmetic — i.e., A5's machinery pointed at the executor
  boundary.

So: **the CS336 pipeline is not just relevant to Tassadar — it is the
precondition for Tassadar's endgame.** The compiled lane needs no training
and already works bounded; the hybrid lane needs a from-scratch pretraining
capability we fully control, and that capability is precisely what the
CS336 port program produces. The two programs also share one economy: a
Tassadar executor workload is the *cheapest verifiable homework class*
(exact trace replay, no probabilistic checking), while CS336 training
homework needs the Freivalds-class machinery — same commit-and-challenge
rail, two verification grades.

One sequencing implication for the epics: nothing in the Tassadar exact
lane waits on the training epic (#4664–#4671) — compiled executor homework
could ship on the rails alone. The hybrid waits on real-gradient
pretraining (CS336 lane 6 in the continuation audit) and is the natural
follow-on epic after both programs land.

## Implications For OpenAgents If We Implement This Fully

Reading the Percepta post end to end sharpens what full implementation
would be worth to OpenAgents specifically. None of the following is
claimed as live; each item names the seam it would land in.

**1. The trace is the receipt.** Percepta's own framing — tool use is
opaque, in-model execution is transparent, "every intermediate step
appears in the trace" — is the property OpenAgents' whole economy is
built around. An execution trace is simultaneously the work product, the
audit log, and the proof: deterministic, append-only, digest-pinnable, and
verifiable by exact replay of any sampled window. For a network that pays
strangers for work, this is the strongest verification grade attainable —
cheaper even than Freivalds, because there is nothing probabilistic left.
The commit-and-challenge layer (CS336 continuation audit) gets a work
class whose validation cost rounds to zero.

**2. CPU economics match our supply side.** 31k tok/s on a CPU, ~98× over
standard decoding, with the benefit concentrated on "boring" deterministic
spans. Our contributor fleet is exactly that hardware — Apple Silicon
laptops, consumer desktops, the "fracked compute" thesis from Episode 224.
Executor workloads make weak devices first-class *sellers*, not just
validators: the machines that can't do meaningful gradient descent can
execute compiled programs at full speed. This is the missing
high-value workload for the long tail of the capacity funnel (19/19
registered Pylons currently dark).

**3. Hallucination-free computation is a sellable product boundary.** The
post's motivation — frontier models fail at multiplication and small
Sudokus unaided — is a *permanent* gap for API-model agents, and our
agents share it. An OpenAgents agent with access to a Tassadar executor
route (or, in the hybrid future, carrying one internally) gets
calculator-grade exactness with zero tool round-trips. For the agent
labor market, that means jobs whose acceptance criteria include exact
computation (accounting checks, constraint solving, schedule
optimization, settlement math) can be both *executed* and *verified*
inside the same substrate. The Hungarian-algorithm demo is literally a
market-clearing computation — min-cost assignment is what a work
dispatcher does.

**4. Speculative execution is a market structure.** Percepta names 2D
fast-path models as speculative proposers whose tokens a regular-attention
model verifies. On an open network, proposer and verifier do not need to
be the same machine or the same owner: cheap devices propose fast-path
tokens, stronger devices verify and accept — a price spread the market can
discover. That is a *new compute-market product* (speculative decode
bandwidth) that no centralized lab has reason to build openly.

**5. Differentiability makes the hybrid trainable — and CS336 is the
lever.** Because the trace is part of the forward pass, gradients flow
through execution. That converts the executor from an external tool into
a trainable organ, and it is what makes the A5/GRPO routing program in
the CS336 section real: reward exact-verified internal computation,
backprop *through* it. No API model offers gradient access; only a
from-scratch pretraining capability (the CS336 port) can exploit this.
This is the concrete content behind Episode 220's "things we can do that
the other labs wouldn't be able to do."

**6. Programs-into-weights meets the skills marketplace.** Percepta's
"growing AI systems like software" — compiled modules accumulating inside
models like libraries — lands directly on rails OpenAgents already has:
digest-pinned program artifacts (Tassadar phase 2), the plugin
manifest/receipt system, the draft SKL (skills registry) NIP, and the
NIP-DS dataset-sale flow. A compiled weight-module — an executor circuit
implementing a named algorithm, digest-pinned, conformance-tested — is a
*tradeable artifact*: listable on the open marketplace, payable in sats,
verifiable by replay before purchase clears. Weight compilation as
"training beyond gradient descent" also creates a homework class that
needs no GPUs at all: compiling, conformance-testing, and auditing
weight-modules is CPU work. The tooling shape already exists publicly:
transformer-vm's `wasm-specialize` (First Futamura projection) takes a
program and emits a specialized weights binary — the weight-module
factory, awaiting a Rust port and a digest-pinned artifact wrapper.

**7. Honest differentiation, honestly bounded.** Percepta is a venture
company aiming at sequential decision systems in healthcare, supply
chains, and finance; their post ends in a hiring pitch. OpenAgents' angle
is orthogonal and unclaimed by them: the **open network** version — exact
execution as paid, verified, Bitcoin-settled work on commodity hardware,
with the trace-as-receipt property feeding a public proof economy. The
two can cite the same mechanism without competing for the same product.
Discipline stays mandatory: Percepta claims arbitrary C via Wasm;
psionic's committed posture is narrower (`arbitrary_c_or_wasm_not_claimed`,
publication suppressed) and our public copy keeps saying exactly what our
own fixtures prove, not what the lineage proves.

**Risks and open questions, kept explicit:** the capability ceiling of
2D-head models trained at scale is an open question Percepta itself
names; hard-max vs softmax fast paths differ and k-sparse softmax is an
approximation; exactness is numerically brittle off CPU-reference (the
psionic f32 semantics matrix exists for this reason); and the hybrid
remains a research direction everywhere — nobody has shipped it. The
bounded-claims machinery Tassadar already has is the right vehicle for
all four.

## Registry And Disclosure Posture

There is **no product promise for Tassadar** in the public registry, and
that appears deliberate: the psionic disclosure flow gates any widening of
public claims, and the current Wasm-lane public-acceptance verdicts are
suppressed (`served_publication_allowed = false`). Episodes 216/220 made
bounded public statements ("more or less reproduced the Percepta paper",
"executor model"), and the psionic repo's committed evidence supports
exactly that bounded phrasing. If Tassadar ever becomes user-facing copy or
a registry promise, the path is: disclosure-flow review → a promise record
whose safeCopy mirrors the capability matrix's per-workload postures and
whose unsafeCopy forbids arbitrary-program and universal-claims copy — the
same scope-language discipline the Qwen fine-tune gate uses.

## What A Continuation Would Look Like (Not Filed)

1. Re-attach an operator replay/inspection surface (the old Tassadar Lab's
   job) on the current stack, reading served evidence bundles.
2. Executor-trace homework: dispatch bounded article-class workloads to
   Pylons through the epic-3 connector, verified by exact trace replay —
   the cheapest verification class in the whole homework program.
   **Filed 2026-06-10 as #4684** (CS336 epic, disclosure-bounded), with
   the pluggable `exact_trace_replay` verification class landing in
   #4674.
3. A `TassadarCapabilityEnvelope` consumer in Pylon's capability reporting,
   so executor-class capacity is advertised with the same no-overclaim
   posture as GEPA.
4. A registry promise via the disclosure flow, only when publication
   suppression lifts.

All four are sequenced behind the epic-3 connector (#4664/#4669) and the
disclosure flow's own gates.

## Evidence Reviewed

- `openagents` git history: `0363dff6c` (first Tassadar commit,
  `crates/psionic/psionic-data/src/tassadar.rs`), the March 2026
  experimental ladder (`32534efa0`, `749ab9be3`, `9664719ed`, `3090ccf70`,
  `371528388`, `0f4cb16f0`, `5826f0f4b`, `039d6db3e`, `c4f732b87`,
  `55bb4e1a7`, `b559ba580`, `9fc9b98e7`, `7fb1983bc`, `52eced170`,
  `ae059cc2f`, `cc18e7de4`, `e8e076ddd`, `c13cf35a6`, `50c8fdb86`,
  `fa80adcf8`), the Autopilot Tassadar Lab series (`1cafa430e`,
  `2ee586a51`, `eefb356e7`, `77174ad7c`, `446ec5d3c`/PR #4008),
  `f5919c766` (removal of the desktop lab with the rebuild)
- `openagents` tree: `docs/transcripts/203.md`, `216.md`, `220.md`;
  `apps/pylon/docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`;
  `packages/probe/docs/2026-06-07-apple-fm-first-backend-audit.md`
- `psionic` (231 Tassadar-titled commits; 20+ docs):
  `docs/PSION_EXECUTOR_PROGRAM.md`, `docs/PSION_PROGRAM_MAP.md`,
  `docs/ARCHITECTURE.md` (executor-class lane),
  `docs/ROADMAP_TASSADAR.md` (bridge), `docs/ROADMAP_TASSADAR_INDEX.md`,
  `docs/PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md`,
  `docs/TASSADAR_DEFAULT_TRAIN_LANE.md`,
  `docs/TASSADAR_STARTER_PLUGIN_RUNTIME.md`,
  `docs/TASSADAR_PUBLIC_DISCLOSURE_FLOW.md`, plus the universality/TCM.v1
  commit series (`ac174ceb`, `b1a033f7`, `04ea60bd`, `694878e9`,
  `a189aae7`, `92e97913`, `56d83ac2`) and the plugin/hardening series
  (`49a1d193` → `fcd3cd0a`)
- `projects/repos/llm-as-computer` (public Percepta validation: README,
  ISA, benchmark refs)
- `projects/repos/transformer-vm` (Percepta-Core, Apache-2.0,
  manifest-tracked): README, `graph/core.py` (five-primitive IR),
  `wasm/interpreter.py` (35-opcode machine), `compilation/lower.py`,
  `scheduler/milp.py` (d_model-minimizing gate-to-layer MILP),
  `model/weights.py` (analytical construction), `specialize.py` (Futamura
  projection), `attention/hull2d_cht.h` + `hull_cache.py` +
  `standard_cache.py`, `model/transformer.cpp` (C++ engine), examples
  manifest (hello/addition/collatz/fibonacci/min_cost_matching/sudoku)
- Web (for the compiled-vs-trained research lineage):
  Percepta, "Can LLMs Be Computers?" (percepta.ai/blog, 2026-03-11, full
  text) and the published companion "Constructing an LLM-Computer"
  (2026-03-25, full text; concept notes in this folder);
  Lindner et al., "Tracr: Compiled Transformers as a
  Laboratory for Interpretability" (arXiv:2301.05062, DeepMind/ETH —
  RASP-to-weights compilation with zero training); Weiss et al., "Thinking
  Like Transformers" (RASP, 2021); Pérez et al., "Attention is
  Turing-Complete" (JMLR 2021); Giannou et al., "Looped Transformers as
  Programmable Computers" (ICML 2023); Dziri et al., "Faith and Fate:
  Limits of Transformers on Compositionality" (NeurIPS 2023 — the learned
  -execution length-generalization plateau)

---

## Addendum (2026-06-11): What Happened To The Continuation

The "What A Continuation Would Look Like (Not Filed)" section above is
now historical. Status of its four items:

1. **Operator replay/inspection surface** — still open; now implied by
   `RESEARCH_PLAN.md` W2's factory monitor and the evolution loop's
   public-tick-monitor blocker.
2. **Executor-trace homework** — filed as openagents #4684 with the
   `exact_trace_replay` verification class in #4674, and then *run*: the
   PoC promise `compute.tassadar_executor_poc.v1` went green 2026-06-10
   (live Pylon dispatch, separate-device replay verdicts including a
   tampered-digest rejection, one paid Lightning closeout).
3. **Capability envelope consumer** — still open; assigned to
   `RESEARCH_PLAN.md` W4 step 1.
4. **Registry promise via the disclosure flow** — happened, twice: the
   PoC promise (green) and `artanis.tassadar_evolution_loop.v1`
   (yellow, four blockers, tracking issue #4697).

Two facts discovered since the audit that bear on its conclusions: the
seventeen-issue executor-compiler campaign (psionic #1098–#1114)
completed the same day, building the IR-level pipeline this audit could
only anticipate; and the first autonomous overnight run of the loop
exposed the projection-staleness defect class (its public report and
capacity funnel did not know about its own accepted work — openagents
#4745, #4746), which `RESEARCH_PLAN.md` §6 has since made a standing
method rule. The unified program now lives in
[`RESEARCH_PLAN.md`](RESEARCH_PLAN.md); this audit remains the canonical
history through 2026-06-10.
