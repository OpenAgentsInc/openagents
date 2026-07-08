Below is the expanded research brief I would use as the basis for a lab roadmap, literature review, and execution plan.

The main change from my earlier framing is this: **the central research object is not just “can a transformer compute?” but “can exact, replayable computation become a reusable substrate for learning, verification, markets, and model modularity?”** Your uploaded directive already makes this distinction explicit: Tassadar is the compiled, exact, digest-pinned lane; Psion is the learned, statistical lane; the product hypothesis lives in the spectrum between them. 

---

# Expanded research brief: Tassadar, Psion, and exact computation inside transformers

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


## 1. Refined thesis

The strongest thesis is:

> **Exact computation should be treated as an engineered substrate, not a capability we merely hope gradient descent discovers.**

Percepta’s construction, as summarized in your introduction, takes ordinary transformer components and analytically sets their weights so the model executes a WebAssembly-like virtual machine step by step. The execution trace is transparent and replay-verifiable, which is what makes it economically different from normal model inference or opaque tool use. 

The uploaded Tassadar + Psion directive then asks the right follow-on question: **can the exactness we can compile become something we can train, sell, and embed?** It already separates the program into a compiled exact lane and a learned lane, and it explicitly forbids learned-model claims from borrowing compiled exactness language. 

That distinction should remain the backbone of the lab. The research program should not try to prove “LLMs are computers” in the vague sense. It should ask a sharper set of questions:

1. Can transformer weights be a **deployment target for software**?
2. Can verified traces become a **high-integrity training corpus**?
3. Can learned models use compiled exact modules without corrupting their guarantees?
4. Can the geometric memory mechanism be trained, preserved, or approximated?
5. Can replay verification create a cheaper market for machine work?
6. Can this scale operationally across heterogeneous workers and homogeneous GPU clusters?

The likely answer is hybrid: **compiled exact cores for operations that must never be wrong; learned planners/interfaces for everything else.**

---

# 2. Source and literature map

## A. Direct lineage: programmed and compiled transformers

The immediate lineage is Percepta plus earlier “program a transformer” work.

Percepta’s released `transformer-vm` repository describes a standard softmax-ReGLU transformer whose weights are computed analytically, not learned, to simulate a WebAssembly virtual machine. The repo says the quick start compiles C examples, solves a MILP schedule, constructs weights, builds a C++ inference engine, and runs programs at roughly 30K tokens per second. It also describes a graph abstraction with lookup and cumulative-sum primitives, a 35-opcode WebAssembly interpreter, a Futamura-style specialization path, and a 2D convex-hull KV cache for logarithmic lookup. ([GitHub][1])

Your own introduction maps the same construction into the Append-only Lookup Machine model: computation is serialized into an append-only trace; attention implements exact keyed lookup and cumulative sums; ReLU/ReGLU implements indicators, conditionals, and multiplication; residual channels do wiring. 

This sits downstream of RASP and Tracr. RASP gave a small programming model for transformer computations using operations analogous to attention selection, aggregation, and feed-forward maps; the paper used RASP programs to describe tasks such as histograms, sorting, and Dyck-language recognition. ([arXiv][2]) Tracr then compiled RASP programs into actual transformer weights, giving researchers a way to build toy transformers with known internal algorithms for mechanistic interpretability. ([arXiv][3])

The crucial difference is scale and semantics. RASP/Tracr show that hand-written symbolic computations can be compiled into transformer-like models. Percepta/Tassadar move the target from small sequence tasks to **VM execution**, **trace replay**, **program specialization**, and **economic verification**. Your uploaded plan makes that leap explicit by treating the trace as work product, audit log, and proof. 

## B. Expressivity theory: what transformers can represent

There is a large theoretical literature saying variants of transformers can be computationally powerful, but those results must be handled carefully.

“Attention is Turing Complete” showed that transformers can simulate Turing machines under assumptions such as rational activations and arbitrary precision. ([Journal of Machine Learning Research][4]) Later work showed even constant bit-size transformers can be Turing complete with long enough context, while also characterizing what can be computed under space constraints. ([arXiv][5]) Recent work on softmax transformers and Chain-of-Thought variants argues that length-generalizable softmax transformers can compute arbitrary sequential computations under suitable constructions. ([arXiv][6])

But expressivity is not learnability. Merrill and Sabharwal’s “Parallelism Tradeoff” line of work argues that when transformers are constrained to log precision, they correspond to highly parallel circuit classes, which helps explain why some sequential reasoning is hard for fixed-depth models. ([arXiv][7]) Another line of work, “How Far Can Transformers Reason?”, distinguishes expressivity from learnability and identifies a “globality barrier”: even when transformers can express an algorithm, learning it out-of-distribution can still fail. ([arXiv][8])

This supports your H1: a vanilla student trained on traces may learn local trace texture without learning the exact execution algorithm. Your directive already names the falsifier: a next-token-only baseline must extrapolate 4–8× in trace length on held-out program families. 

## C. Scratchpads, Chain-of-Thought, and serial computation

Chain-of-Thought can be interpreted as giving the model more serial computation steps. Recent theory argues that with `T` Chain-of-Thought tokens, transformers can solve problems corresponding to Boolean circuits of size `T`; without those serial steps, constant-depth transformers face stronger limits. ([arXiv][9])

This is directly relevant because Tassadar traces are not ordinary rationales. They are **deterministic execution scratchpads**. They do not merely explain an answer; they are the computation.

Arithmetic length-generalization studies reinforce the danger. One paper on transformers trained for arithmetic found that relative positions can help addition generalize in length, but multiplication remains much harder; prompt priming with longer examples improved extrapolation. ([arXiv][10]) Another study found that task-specific sc[118;1:3uratchpads and position-coupling mechanisms can help arithmetic transformers generalize 2–3× beyond training lengths. ([arXiv][11])

The implication: train-short/evaluate-long must be a first-class split, and the student model should get auxiliary state supervision, not only next-token loss.

## D. Neural algorithmic reasoning before transformers

This topic also descends from Neural Turing Machines, Differentiable Neural Computers, Neural GPUs, and Neural Programmer-Interpreters.

Neural Turing Machines coupled neural networks to differentiable external memory and showed learning on tasks such as copying, sorting, and associative recall. ([arxiv.org][12]) Differentiable Neural Computers extended that idea with a memory matrix and read/write mechanisms intended to behave more like external RAM. ([Nature][13]) Neural GPUs were trained on short binary addition and multiplication examples and reported strong generalization to longer examples in that setting. ([arXiv][14]) Neural Programmer-Interpreters used supervised execution traces to learn program-like behavior from relatively few demonstrations. ([arXiv][15])

The lesson is not “neural nets easily learn algorithms.” The lesson is more nuanced:

* explicit memory helps;
* trace supervision helps;
* curriculum matters;
* length generalization is fragile;
* architecture bias is usually necessary;
* evaluation must target algorithmic extrapolation, not in-distribution accuracy.

That is exactly the posture your H1/H3/H5 hypotheses encode. 

## E. Tool use, program-aided reasoning, and why Tassadar is different

Tool use is the dominant industry workaround. Toolformer showed that a language model can self-supervise API use, including calculator and search calls. ([arXiv][16]) Program-Aided Language Models delegate computation to a Python interpreter while the model handles high-level problem decomposition. ([arXiv][17]) Program-of-Thought prompting similarly has the model generate executable code and offload execution to an external interpreter. ([arXiv][18])

These are strong baselines for products. They are also the main critique of Tassadar: if external tools already work, why embed computation inside a transformer?

The answer has to be narrower than “because it is cool.” The answer is:

1. The trace is inside the model’s generated stream.
2. The trace is deterministic and replayable.
3. Verification is a hash comparison or first-divergence localization.
4. The compiled module can, in principle, participate in differentiable forward passes.
5. The artifact can be treated as a versioned, conformance-tested weight module.

Your introduction states the first three clearly: in-model execution exposes the intermediate machine state as tokens, and deterministic serialized execution can be replayed byte-for-byte.  Your plan states the fourth and fifth as the product hypothesis: compiled exact cores behind learned control, and programs-in-weights as a module system. 

## F. Process supervision, execution traces, and distillation

There is supporting literature for supervising intermediate process, but it comes with warnings.

“Let’s Verify Step by Step” found that process supervision can outperform outcome supervision in mathematical reasoning and introduced a large corpus of step-level labels. ([arXiv][19]) NExT teaches models to inspect execution traces and reason about code runtime behavior through execution-aware rationales. ([Proceedings of Machine Learning Research][20]) Visual Program Distillation samples candidate programs, executes and verifies them, then distills successful programmatic reasoning into a vision-language model. ([arXiv][21])

But execution-trace learning is not automatically beneficial. A 2025 program-repair study found that trace prompts improved performance in only 2 of 6 dataset/model configurations, and that effectiveness diminished as trace complexity increased. ([arXiv][22]) This is important. It argues against dumping raw verbose traces into a model.

The lab should treat trace format as a research object. You should compare raw traces, compact semantic traces, transition records, and auxiliary target formats. The uploaded plan already adopts compact binary traces in the hot path and treats human-readable traces as sampled audit artifacts only. 

## G. WebAssembly, compiler verification, and differential testing

WebAssembly is the right VM substrate because it has a precise, portable semantics and an official specification. The W3C spec describes WebAssembly as a binary instruction format for a stack-based virtual machine, intended as a portable compilation target. ([webassembly.org][23]) The core specification defines WebAssembly syntax, validation, and execution independently of any embedding. ([webassembly.org][24]) The official spec repository includes source files, a reference implementation, and an official test suite. ([GitHub][25])

Formal-methods precedent matters here. There is work mechanizing and verifying the WebAssembly specification in Isabelle, including type soundness. ([Cambridge Comp Lab][26]) CompCert is the canonical example of a formally verified compiler, with machine-checked proofs that generated code preserves source semantics. ([compcert.org][27]) Csmith showed the value of randomized differential testing by generating C programs, compiling them with multiple compilers, and comparing outputs to find bugs. ([Users CS Utah][28])

Your own differential harness already caught real scheduler bugs, which is precisely the kind of evidence this area needs.  The next stage should borrow heavily from compiler QA: reference interpreters, conformance suites, randomized program generation, metamorphic tests, translation validation, and eventually proof-carrying compilation for narrow profiles.

## H. Distributed compute and verification economics

The market angle has precedents, but none are identical.

BOINC showed that heterogeneous volunteer machines can contribute large-scale distributed computation, while also exposing problems around unreliability, churn, redundancy, and validation. ([Springer][29]) Golem frames itself as a decentralized marketplace for computing power. ([Golem Network][30]) TrueBit introduced an incentive layer and verification game for off-chain computation. ([arXiv][31]) ZK-STARK work shows another verification path where proof verification can scale far better than re-executing the original computation, though at much higher prover complexity. ([ePrint Archive][32])

Tassadar’s distinguishing claim is simpler: for deterministic bounded workloads, replay may be cheap enough that the proof is just the trace and digest. Your plan correctly keeps succinct proofs as second-stage contingency, not day-one infrastructure. The kill condition is also right: if validation cost exceeds the statistical machinery it replaces, the economics invert. 

## I. Distributed training systems

For the training side, the literature and tooling point to a clean separation:

* asynchronous heterogeneous workers for trace generation, validation, and evaluation;
* controlled homogeneous GPU clusters for synchronous gradient training.

PyTorch FSDP2 shards parameters, gradients, and optimizer state and decomposes collective communication into all-gather and reduce-scatter phases. ([PyTorch Documentation][33]) Megatron Bridge/Core supports combinations of tensor, pipeline, context, and data parallelism for large-model training. ([NVIDIA Docs][34]) Context parallelism partitions the sequence dimension across devices, which is especially relevant for long trace sequences. ([awsdocs-neuron.readthedocs-hosted.com][35])

For cluster topology, NVIDIA’s DGX SuperPOD reference separates compute, storage, in-band management, and out-of-band management fabrics, and describes rail-aligned scaling units. ([NVIDIA Docs][36]) NCCL tests are the correct pre-flight toolchain; NVIDIA’s `nccl-tests` documentation emphasizes both performance and correctness, and its bus-bandwidth metric is meant to reflect hardware utilization independently of rank count. ([GitHub][37])

Long-context training/inference also has relevant systems work. FlashAttention computes exact attention with IO-aware tiling to reduce memory traffic and improve speed. ([arXiv][38]) Ring Attention distributes sequence blocks across devices and overlaps communication with attention computation. ([arXiv][39]) These are relevant for Psion training on long trace contexts, but they do not replace the specialized hull-cache path for compiled exact execution.

---

# 3. Critiques to take seriously

## Critique 1: “A CPU is faster.”

This is true and should be said first. Your internal documents say it plainly: for standalone batch computation, a plain CPU beats every system described here, and overclaiming otherwise violates the program’s first rule.  Percepta’s own repo also caveats that this is not meant to beat conventional computers on raw compute. ([GitHub][1])

The product is not “slower CPU inside a transformer.” The product is:

* computation with a trace-as-receipt;
* deterministic replay;
* exact module composition;
* differentiable integration with model forward passes;
* machine-work markets where verification cost matters more than raw execution cost.

If buyers do not value those properties, the product line should die and the executor should remain a research/data-generation system. Your kill conditions already say this. 

## Critique 2: Turing completeness does not imply trainability.

Theoretical expressivity results do not guarantee that SGD will discover the intended algorithm, that finite precision will preserve it, or that a trained checkpoint will extrapolate. This is why H1 is the right default: pure next-token trace imitation probably fails unless proven otherwise. 

The lab should treat learned exactness as a falsifiable empirical target, not an assumption.

## Critique 3: Raw traces can become anti-training data.

Execution traces are long, repetitive, and locally predictable. A model can achieve good loss by learning syntax without learning semantics. Trace complexity can also make execution information less useful, as program-repair work has found. ([arXiv][22])

Therefore, train on multiple trace representations and evaluate by replayed rollout, not perplexity.

## Critique 4: The hull cache is a structured fast path, not universal magic.

The `O(log n)` speedup relies on 2D hard-max geometry: attention becomes a convex-hull support query. Your introduction describes this precisely.  This is powerful for compiled heads designed to use that geometry. It is not automatically a drop-in acceleration for arbitrary learned high-dimensional attention.

A research question remains: can learned heads be constrained, initialized, or regularized so they remain hull-compatible?

## Critique 5: Softmax and numerical precision are not bookkeeping details.

Your current lane is hard-max; the uploaded plan says softmax error bounds in owned code are still open.  This matters because trainable models use softmax, BF16/FP8, normalization, stochastic optimization, and approximate kernels.

The lab needs a numerical margins program: prove or empirically bound how much score separation is needed for correct lookup under target inference precision.

## Critique 6: WASM subset semantics are a minefield.

A 12-opcode i32 profile can be made exact. A wider WASM subset introduces traps, signedness, memory aliasing, floats, host calls, module linking, and undefined or implementation-dependent edges. Your W1 correctly gates W2/W3 because corpus diversity is bounded by the interpreted window. 

Do not widen the opcode set faster than the harness can catch semantic bugs.

## Critique 7: Open distributed gradients are dangerous.

Heterogeneous public workers are excellent for trace generation and replay validation. They are a poor substrate for synchronous all-reduce training and a security risk for optimizer-state integrity.

Use the open network to produce and validate immutable data. Use controlled GPUs for training. Your plan already adopts this division. 

---

# 4. Expanded research avenues

## Avenue 1: Substrate completion and semantic conformance

This is the gating workstream. It is not optional infrastructure.

The goal is to move from “bounded internal demonstrator” to “versioned executable substrate.” Your plan already orders this as WASM window ladder, dense materialization, MILP backend, and softmax bounds. 

Additional work to add:

| Subproblem     | Research task                                                                   | Evidence standard                                 |
| -------------- | ------------------------------------------------------------------------------- | ------------------------------------------------- |
| WASM semantics | Build conformance ladder against official WASM tests and reference interpreters | per-profile pass/fail matrix                      |
| Trap behavior  | Formalize traps as typed refusals or trace states                               | replay verifier catches trap divergence           |
| Memory model   | Load/store aliasing, memory64, multi-memory                                     | randomized differential tests                     |
| Signedness     | i32/i64 signed and unsigned comparisons                                         | metamorphic tests                                 |
| Floats         | exact vs rounded semantics, NaN handling, determinism                           | separate float profile, no silent inclusion       |
| Lowering       | unsupported op lowering without trace blowup                                    | lowering proof or translation validation          |
| Dense weights  | emit loadable checkpoint tensors                                                | independent inference engine reproduces reference |
| Scheduler      | MILP vs greedy width/depth tradeoff                                             | benchmark table with digests                      |

This workstream should maintain a **profile registry**:

```text
profile_id
wasm_feature_set
opcode_set
trap_policy
memory_policy
float_policy
lowering_policy
compiler_hash
reference_runner_hash
test_suite_hash
known_limitations
```

Every trace and every checkpoint should carry the profile ID.

## Avenue 2: Trace-corpus science

The trace factory should not only produce volume. It should produce **coverage**.

Your current plan already specifies compact binary traces, validation tiers, and scale targets of 1–5M local tokens, 100–300M pilot tokens, and 1–10B stable tokens.  I would add a formal corpus-design layer.

Each trace shard should be scored along dimensions such as:

```text
opcode coverage
branch entropy
loop-depth distribution
memory-alias density
stack-depth distribution
trace length
program family
input entropy
near-miss lookup stress
trap incidence
output digest diversity
first-divergence diagnostic value
```

You want a **coverage-balanced corpus**, not just a large corpus. Otherwise the model learns “easy interpreter texture” and fails on branch/memory/trap edge cases.

Research experiments:

1. **Trace representation ablation**

   * raw VM trace;
   * compact semantic trace;
   * transition record;
   * multi-target state tuple;
   * compressed trace with periodic checkpoints;
   * trace plus proof obligations.

2. **Curriculum schedule**

   * shortest traces first;
   * opcode-family curriculum;
   * branch-stress late curriculum;
   * adversarial near-miss lookup curriculum;
   * active curriculum based on student first-divergence failures.

3. **Data contamination control**

   * hold out entire program generators, not just seeds;
   * hold out opcode combinations;
   * hold out length regimes;
   * hold out memory layouts;
   * hold out application families.

4. **Corpus quality metrics**

   * replay pass rate;
   * duplicate trace rate;
   * semantic novelty;
   * edge-case density;
   * validator disagreement;
   * cost per accepted token.

The factory should produce not just `train`, `val`, and `test`, but named evaluation suites:

```text
eval/length_2x
eval/length_4x
eval/length_8x
eval/heldout_program_family
eval/heldout_opcode_combo
eval/branch_stress
eval/memory_alias_stress
eval/near_miss_lookup
eval/trap_semantics
eval/economic_ledger
eval/protocol_state_machine
```

## Avenue 3: Student-model exactness and first-divergence science

Your W3 baseline portfolio is exactly right: next-token trace distillation, auxiliary state losses, 2D-head/hard-max regularization, and frozen executor plus learned interface. 

I would expand this into a formal experimental matrix.

### Core evaluation metrics

Perplexity should be reported, but it should not be the headline.

The headline metrics should be:

```text
exact_rollout_pass@1
exact_rollout_pass@k
median_first_divergence_step
p90_first_divergence_step
valid_prefix_length
branch_accuracy
memory_read_key_accuracy
memory_read_value_accuracy
stack_depth_accuracy
next_pc_accuracy
trap_policy_accuracy
output_digest_match_rate
replay_verifier_acceptance_rate
length_extrapolation_factor
```

### First-divergence taxonomy

Every failed rollout should be classified:

```text
instruction_fetch_error
program_counter_error
branch_decision_error
stack_depth_error
stack_value_error
memory_key_error
memory_value_error
latest_write_error
trap_error
output_error
syntax_error
halt_error
unknown
```

This becomes the research signal. A model whose divergence shifts from syntax errors to rare memory alias errors is improving in a meaningful way even if exact pass rate is still low.

### Training variants

Run at least:

1. **Local next-token baseline**

   * standard decoder;
   * compact trace tokens;
   * no auxiliary losses.

2. **State-supervised decoder**

   * next token plus `pc`, stack top, stack depth, memory key/value, branch target.

3. **Transition model**

   * one sample per execution step;
   * predict next state tuple, not only next token.

4. **Verifier-in-the-loop RL or rejection fine-tuning**

   * generate rollouts;
   * replay verifier gives pass/fail and first-divergence;
   * train on corrected suffixes.

5. **Synthetic error correction model**

   * intentionally corrupt traces;
   * train model to localize and repair first divergence.

6. **Architectural memory bias**

   * recurrent state adapters;
   * constrained 2D lookup heads;
   * frozen exact attention modules;
   * scratchpad-aware positional encodings.

7. **Hybrid planner**

   * model emits ABI calls to exact modules;
   * module trace is generated and verified;
   * planner resumes after verified result.

The key scientific question: does the model learn execution, or does it learn to ask the exact executor for the parts it cannot safely do? Either outcome is useful, but they support different product claims.

## Avenue 4: Trainable hull-compatible memory

This is the cleanest pure-research question in the program.

The compiled mechanism embeds key `k` as `(2k, -k²)`, so query `(q, 1)` makes the score uniquely maximize at `k = q`.  The speed path then depends on these 2D keys living in a geometry where hard-max attention is a convex-hull support query. 

Research questions:

1. Can SGD learn this geometry from scratch?
2. Can SGD preserve it from analytic initialization?
3. Can softmax temperature annealing approach hard-max behavior without instability?
4. How large must margins be under BF16 or FP8?
5. Can a trained head remain hull-cache compatible after fine-tuning?
6. Can top-k sparse softmax over hull candidates approximate ordinary attention well enough for learned models?

Experiments:

| Experiment                   | Setup                                               | Success criterion                          |
| ---------------------------- | --------------------------------------------------- | ------------------------------------------ |
| Geometry-from-scratch        | Train 2D lookup head on key-value retrieval         | exact retrieval on out-of-range keys       |
| Analytic-init preservation   | initialize parabolic keys, train surrounding layers | margins remain positive after training     |
| Max-margin lookup loss       | penalize nearest incorrect key                      | higher length extrapolation                |
| Frozen lookup head           | freeze compiled memory, train controller            | exact memory reads retained                |
| Softmax temperature schedule | gradually lower temperature                         | no collapse, stable margins                |
| Precision stress             | FP32/BF16/FP8 comparison                            | lookup correctness by margin bucket        |
| Hull feasibility             | export learned keys to hull cache                   | identical outputs vs brute-force attention |

A negative result is still valuable. If 2D geometry does not train, that strongly supports the hybrid/frozen-core strategy.

## Avenue 5: Frozen exact cores plus learned control

This is the most likely path to a useful product.

Your plan’s H2 says frozen exact cores behind explicit ABI tokens, with a trained planner marshalling inputs/outputs, should produce useful verifiable hybrid behavior fastest.  This is supported indirectly by the success of program-aided language models and tool-use systems, but Tassadar’s differentiator is that the module can be replayable and potentially internal to the model’s forward computation rather than an opaque external API. ([arXiv][17])

Build a module ABI like:

```text
<exact.call module="i32_mul" profile="core_i32.v0.2">
  inputs: ...
  max_steps: ...
  expected_schema: ...
</exact.call>

<exact.trace digest="..." steps="..." verifier="...">
  compact_trace_ref: ...
  output: ...
</exact.trace>
```

Module families to prioritize:

1. integer arithmetic;
2. fixed-point accounting;
3. ledger state transitions;
4. finite-state protocol validators;
5. bounded parsers;
6. checksum/hash slices;
7. small WASM interpreter slices;
8. proof/checker kernels;
9. scheduling/allocation kernels;
10. deterministic data transforms.

Metrics:

```text
route_precision
route_recall
unnecessary_exact_call_rate
missed_exact_call_rate
ABI_marshal_accuracy
module_replay_acceptance_rate
downstream_task_success
latency_overhead
cost_per_verified_call
```

This line can succeed even if learned exactness fails. In fact, that is the most robust product posture.

## Avenue 6: Programs-in-weights as a module system

The Futamura projection is the most strategically novel part of the construction. Your introduction explains how a fixed program can be moved from prompt tokens into FFN structure, making the program live in the weights.  Your directive then turns this into H4: digest-pinned weight modules as a new asset class. 

Treat this like a package ecosystem:

```text
module_name
semantic_version
profile_id
source_program_hash
compiler_hash
scheduler_hash
weight_hash
conformance_suite_hash
accepted_input_schema
output_schema
max_steps
resource_envelope
known_refusals
license
price
audit_receipts
```

Research questions:

1. Can modules be linked compositionally?
2. Can modules be hot-swapped without retraining the planner?
3. Can a planner learn module affordances from metadata?
4. Can modules be compressed or merged?
5. Can module conformance tests prevent malicious or buggy exact modules?
6. Can users buy/sell verified weight modules in the artifact market?

This becomes something like a **weight package manager**.

The critique: module boundaries can destroy differentiability or create brittle planner behavior. So the first version should use explicit ABI tokens and replay receipts, not hidden latent calls.

## Avenue 7: Compiler correctness and translation validation

This program should borrow more from compiler engineering than from ordinary ML benchmarking.

Recommended layers:

1. **Reference semantics**

   * official WASM spec;
   * reference interpreter;
   * profile-specific executable spec.

2. **Randomized program generation**

   * Csmith-like generator for profile-safe programs;
   * metamorphic transformations;
   * differential execution across interpreters.

3. **Translation validation**

   * for each compiled module, validate that gate graph implements profile semantics for bounded inputs;
   * validate schedule preserves dependencies and liveness;
   * validate stale-slot subtraction.

4. **Proof-carrying artifacts**

   * every module ships a proof object or validation receipt;
   * validators can check proof cheaper than recompilation.

5. **Regression pinning**

   * every discovered bug becomes a named test;
   * first-divergence record becomes the regression seed.

CompCert proves that formally verified compilers are possible, but it also shows how much engineering is required. ([CompCert][27]) Csmith shows that randomized differential testing catches real compiler bugs even in mature systems. ([users.cs.utah.edu][28]) Your harness already catching scheduler bugs is strong evidence that this methodology belongs at the center. 

## Avenue 8: Verification protocol and market design

The verification protocol should be designed as an adversarial economic system, not just a validation script.

Your uploaded plan already says adversarial verification is funded, not merely tolerated, and that the harness comes before the claim.  Extend that into a formal protocol.

### Worker roles

```text
generator
validator
challenger
auditor
curator
trainer
publisher
buyer
```

### Artifact states

```text
submitted
schema_valid
full_replay_pending
full_replay_accepted
spot_check_accepted
challenged
rejected
quarantined
admitted_to_training
deprecated
```

### Validation policy

* New worker: full replay on all submissions.
* Trusted worker: spot-check plus random full replay.
* New profile: full replay until stable.
* New program family: redundant validation.
* High-value shard: multiple independent validators.
* Public claim: challenge window before promotion.

### Economic attack cases

| Attack                                 | Defense                                             |
| -------------------------------------- | --------------------------------------------------- |
| worker submits fake trace              | full replay and digest check                        |
| worker copies existing trace           | seed uniqueness and duplicate detection             |
| colluding validator approves bad trace | random adversarial replay                           |
| corpus poisoning                       | quarantine before training                          |
| easy-work farming                      | diversity-weighted rewards                          |
| overfitting to validation              | hidden eval suites                                  |
| public dashboard stale                 | rebuild on validation transitions                   |
| sybil workers                          | stake/reputation plus redundancy                    |
| expected digest leakage                | never ship expected digest in generation assignment |

The long-term question is whether replay remains cheap enough. If it does not, move to Merkleized trace chunks, first-divergence proofs, and eventually STARK-style succinct proofs for narrow profiles. Your current plan correctly treats this as second-stage. 

## Avenue 9: Security and sandboxing

Exact execution does not remove security concerns. It moves them.

Threats:

1. malicious WASM modules;
2. resource-exhaustion traces;
3. nondeterministic host interactions;
4. hidden data exfiltration through outputs;
5. poisoned training traces;
6. malicious exact modules sold as weight packages;
7. verifier implementation bugs;
8. checkpoint tampering;
9. ABI prompt injection against hybrid planners;
10. floating-point nondeterminism across hardware.

Design responses:

```text
no host calls in early profiles
deterministic bounded fuel
typed refusals for traps/resource limits
profile-specific sandbox
module conformance suite
artifact signatures
content-addressed storage
trace digest + output digest + compiler hash
verifier diversity
hidden adversarial eval
```

The early profile should be boring by design: integer-only, deterministic, bounded, no host effects.

## Avenue 10: Long-context and trace systems

There are two distinct long-context problems:

1. **Compiled executor inference**, where the hull cache can avoid linear attention scans for structured 2D hard-max heads.
2. **Student-model training**, where the model may still need ordinary transformer training on long or chunked trace contexts.

For the student, do not begin with huge contexts. Start with transition-local chunks and evaluate long rollouts autoregressively. Use long-context systems only once the learning signal is proven.

Possible systems stack:

```text
phase 1: transition chunks, 1K–4K context
phase 2: trace windows, 8K–32K context
phase 3: context parallelism for longer windows
phase 4: replay-verified long rollout, not full-context training
```

FlashAttention and Ring Attention are useful for training/inference efficiency on ordinary attention, but they do not answer the core exactness question. ([arXiv][38])

## Avenue 11: Theory agenda

The theory agenda should focus on finite, trainable, and economically relevant exactness.

Questions:

1. **Finite precision**

   * What margin guarantees exact lookup under BF16/FP8?
   * How do layernorm and residual accumulation affect compiled exactness?
   * Can score margins be certified per module?

2. **Learnability**

   * What data distribution makes parabolic lookup learnable?
   * What auxiliary losses reduce sample complexity?
   * Can we prove vanilla next-token training fails on certain trace families?

3. **Complexity**

   * Which trace languages are easy for bounded-depth transformers?
   * Which require explicit serial scratchpad tokens?
   * How does Chain-of-Thought length trade off with model depth/width?

4. **Hybrid computation**

   * Can frozen exact modules be treated as differentiable subroutines?
   * What gradients pass through trace tokens?
   * What is the stability of learned controllers around exact modules?

5. **Verification economics**

   * When is replay cheaper than statistical validation?
   * When does succinct proof generation beat replay?
   * What redundancy policy minimizes expected fraud loss?

This theory work should be tied to experiments, not isolated.

---

# 5. Distributed run architecture, expanded

## Core architectural rule

Use the network for **asynchronous verified data production**. Use controlled GPU clusters for **synchronous training**.

Your directive already says the loose asynchronous network is for generation, validation, and evaluation only, while synchronous gradient training should happen on controlled homogeneous GPUs.  Keep that line hard.

## Planes

### Plane A: Work generation

```text
program_generator_id
profile_id
seed_range
input_distribution
max_steps
compiler_hash
executor_hash
trace_schema_version
reward_policy
```

No expected digest goes into the assignment.

### Plane B: Execution workers

Workers return:

```text
trace_ref
trace_digest
output_digest
resource_stats
executor_version
worker_signature
```

### Plane C: Validators

Validators replay traces and produce:

```text
verdict
first_divergence_step
expected_digest
observed_digest
validator_version
validator_signature
```

### Plane D: Corpus curator

The curator admits only verified traces, maintains diversity quotas, builds train/eval splits, and emits immutable manifests.

### Plane E: Trainer

The trainer reads only immutable, content-addressed shards. It should not read mutable “latest” objects.

### Plane F: Evaluator

Every checkpoint is evaluated by rollout → replay verifier → divergence report.

## Storage layout

```text
corpus/
  profile=core_i32.v0.1/
    split=train/
      family=arith_carry/
        shard-000001.bin.zst
        shard-000001.manifest.json
    split=eval_length_2x/
    split=eval_length_4x/
    split=eval_branch_stress/
    split=eval_memory_alias/
    split=eval_economic_ledger/
```

Each manifest should contain:

```text
shard_hash
trace_count
token_count
profile_id
program_family
generator_hash
executor_hash
validator_receipts
created_at
admission_policy
split_policy
```

## GPU topology

For initial training:

* one homogeneous GPU cluster;
* local NVMe cache;
* BF16 first, not FP8 until numerical margin experiments pass;
* FSDP2/HSDP for sharding;
* context parallelism only when sequence length forces it;
* tensor parallelism only when model width forces it;
* pipeline parallelism only if memory still fails.

For multi-node training, pre-flight:

```text
nvidia-smi topo -m
p2pBandwidthLatencyTest
nvbandwidth
ib_write_bw or RoCE equivalent
nccl-tests all_reduce_perf
nccl-tests all_gather_perf
nccl-tests reduce_scatter_perf
synthetic training step with real parallelism config
```

NCCL tests should be part of the run acceptance gate, not optional debugging. ([GitHub][37])

## Suggested rank mapping

For 8-GPU NVLink/NVSwitch nodes:

```text
inside node:
  context_parallel = 2, 4, or 8 for long traces
  tensor_parallel = 1 or 2
  FSDP/HSDP shard group = local node

across nodes:
  data_parallel replicas
  avoid cross-node tensor parallel unless necessary
```

This keeps frequent, small collectives inside the fastest fabric and pushes the less frequent data-parallel synchronization across nodes.

---

# 6. Concrete experimental agenda

## Phase 0: Reproducibility and substrate

Exit criteria:

```text
clean checkout can regenerate sample traces
all traces carry profile/compiler/executor hashes
dense checkpoint materialization prototype exists
official/reference WASM tests mapped to profile status
differential harness runs in CI
```

## Phase 1: Trace factory pilot

Target:

```text
1M–5M local verified tokens
100M–300M distributed verified tokens
four program families minimum
zero unverified traces admitted to training
```

Program families:

1. arithmetic and carry;
2. stack and control flow;
3. memory load/store;
4. bounded application state machines.

## Phase 2: Student baseline sweep

Train:

1. next-token baseline;
2. auxiliary-state baseline;
3. 2D geometry/hard-max regularized baseline;
4. frozen exact module plus learned interface.

Report:

```text
dataset_hash
checkpoint_hash
config_hash
eval_hash
exact_rollout_pass@1
first_divergence_histogram
length_extrapolation_curve
program-family holdout score
```

No model claim should ship without the replay report.

## Phase 3: Active curriculum

Use first-divergence failures to generate new traces.

Example:

```text
if memory_read_key_error rises:
  generate memory-alias stress traces

if branch_decision_error rises:
  generate nested branch/loop traces

if syntax_error dominates:
  simplify trace grammar or add syntax auxiliary loss

if valid_prefix_length plateaus:
  introduce checkpointed traces and longer rollout supervision
```

## Phase 4: Hybrid product prototype

Build an agent-facing exact-call endpoint:

```text
input: bounded task + profile
output: result + compact trace ref + digest + verifier receipt
```

Initial tasks:

* long multiplication;
* fixed-point accounting;
* ledger transition;
* deterministic protocol validation;
* bounded WASM execution.

Acceptance:

```text
buyer values trace receipt
verifier cost below threshold
latency acceptable
module replay acceptance > 99.99%
clear failure/refusal taxonomy
```

---

# 7. Research avenue table

| Avenue                 | Main question                                | First experiment                              | Failure signal                            |
| ---------------------- | -------------------------------------------- | --------------------------------------------- | ----------------------------------------- |
| W1 substrate           | Can the VM profile widen safely?             | structured control + memory conformance suite | harness misses semantic bugs              |
| Dense weights          | Can artifacts become loadable modules?       | emit W_Q/W_K/W_V/FFN for one profile          | independent engine disagrees              |
| MILP backend           | Does optimal scheduling matter economically? | greedy vs MILP width/depth/token-rate         | little gain or high compile cost          |
| Softmax bounds         | Can exactness survive softmax/precision?     | margin certification under FP32/BF16          | lookup flips under target precision       |
| Trace corpus           | What trace format trains best?               | raw vs semantic vs transition records         | all models learn syntax only              |
| Student exactness      | Can Psion learn execution?                   | four-baseline sweep                           | no length extrapolation                   |
| 2D geometry            | Can hull-compatible heads train?             | analytic init + margin loss                   | learned keys leave hull-compatible regime |
| Hybrid modules         | Can learned planner use exact cores?         | ABI-call planner for arithmetic/ledger        | marshal errors or missed calls dominate   |
| Module market          | Can programs-in-weights be sold safely?      | conformance-tested module registry            | module linkage/versioning breaks          |
| Verification economics | Is replay cheap enough?                      | measured cost per accepted trace              | validation cost exceeds value             |
| Succinct proofs        | When is replay insufficient?                 | Merkleized chunk receipts                     | proof overhead not justified              |
| Security               | Can hostile workers poison corpus?           | red-team trace submissions                    | bad shard admitted to training            |

---

# 8. Strong supporting arguments

## Supporting argument 1: exact replay creates unusually clean labels

Most model training data is noisy. Execution traces from a deterministic oracle are different. If the trace replays, the label is correct under the profile. Your introduction says the executor can be teacher, grader, and curriculum generator, and candidates can be graded by replay with first-divergence localization. 

This is stronger than ordinary Chain-of-Thought data because the intermediate steps are not merely plausible. They are executable.

## Supporting argument 2: first divergence is a better scientific object than accuracy

Aggregate accuracy hides mechanism. First divergence exposes mechanism.

A model that diverges at step 40 because it emits malformed syntax has a different failure mode from a model that diverges at step 40,000 on latest-write-wins memory aliasing. Your standing orders already say “the first divergent step is the result; aggregate accuracy is the abstract.” 

This is exactly how the lab should publish.

## Supporting argument 3: the hybrid path is independently supported

Tool-use and program-aided reasoning papers show that LLMs benefit from delegating exact computation to executable systems. ([arXiv][17]) The Tassadar/Psion hybrid keeps the delegation idea but upgrades the artifact: the exact module can be compiled, digest-pinned, replayable, and potentially internal to a model/module system.

## Supporting argument 4: compiler QA is culturally aligned with the lab

Your own harness caught bugs early.  This is not incidental; it is the core method. The lab’s advantage is not that it never makes mistakes. It is that it can make mistakes cheap, visible, reproducible, and paid to find.

---

# 9. Weak arguments to avoid

Avoid these claims unless the evidence exists:

1. “The trained model is exact.”
2. “This beats CPUs.”
3. “The hull cache accelerates arbitrary transformers.”
4. “The WASM implementation is complete.”
5. “The open network can train the model synchronously.”
6. “Trace supervision guarantees algorithm learning.”
7. “Turing completeness means the system is practical.”
8. “A verified trace market will clear at scale.”
9. “Softmax preserves hard-max exactness in our code.”
10. “Dense weight modules are ready if only scalar/numeric artifacts exist.”

Your uploaded boundaries section already says no trained model intersects the current compiled work, no softmax in your owned legs, no dense materialization yet, a twelve-opcode window against Percepta’s thirty-five, a greedy scheduler against MILP, and no broad serving/pricing claim. 

Keep that discipline. It is an asset.

---

# 10. Recommended reading list for the lab

## Core internal/current materials

1. **The Tassadar + Psion Research Plan** — use as the governance and sequencing document. 
2. **The LLM as a Computer: A Full Introduction** — use as the conceptual onboarding document. 
3. Percepta `transformer-vm` repo — use as the executable external reference for the compiled-transformer VM construction. ([github.com][1])

## Programmed transformer lineage

* RASP / Thinking Like Transformers. ([arXiv][2])
* Tracr: compiled RASP to transformer weights. ([arXiv][3])

## Expressivity and limits

* Attention is Turing Complete. ([Journal of Machine Learning Research][4])
* Constant Bit-size Transformers are Turing Complete. ([arXiv][5])
* Parallelism Tradeoff and log-precision transformer limits. ([arXiv][7])
* How Far Can Transformers Reason? ([arXiv][8])

## Algorithmic learning and traces

* Neural Turing Machines. ([arXiv][12])
* Differentiable Neural Computers. ([Nature][13])
* Neural GPU. ([arXiv][14])
* Neural Programmer-Interpreters. ([arXiv][15])
* Process supervision / Let’s Verify Step by Step. ([arXiv][19])
* NExT execution-aware code reasoning. ([Proceedings of Machine Learning Research][20])
* Execution traces for program repair, especially the negative/limited-results angle. ([arXiv][22])

## Tool use and program-aided reasoning

* Toolformer. ([arXiv][16])
* Program-Aided Language Models. ([arXiv][17])
* Program of Thoughts. ([arXiv][18])

## Verification and compiler methodology

* WebAssembly specification and official test suite. ([WebAssembly][24])
* Mechanized WebAssembly semantics in Isabelle. ([Cambridge Comp Lab][26])
* CompCert. ([CompCert][27])
* Csmith randomized differential testing. ([Users.cs.utah.edu][28])

## Distributed systems

* PyTorch FSDP2. ([PyTorch Documentation][33])
* Megatron parallelism. ([NVIDIA Docs][34])
* Context parallelism. ([AWS Neuron Documentation][35])
* NCCL tests. ([GitHub][37])
* DGX SuperPOD fabric separation. ([NVIDIA Docs][36])
* FlashAttention and Ring Attention for long-context systems. ([arXiv][38])

## Verifiable distributed compute

* BOINC and volunteer compute. ([Springer][29])
* Golem marketplace. ([Golem Network][30])
* TrueBit verification games. ([arXiv][31])
* ZK-STARKs as later-stage succinct verification reference. ([ePrint Archive][32])

---

# 11. Final recommended stance

The research program should be publicly framed as:

> **We can compile exact computation into transformer-shaped artifacts today. We do not yet know whether ordinary training can learn that exactness. We will use the compiled executor as oracle, verifier, curriculum generator, and frozen module substrate while we test that question under replay.**

The immediate priority order should be:

1. **W1: substrate before scale** — profile ladder, dense materialization, scheduler validation, softmax bounds.
2. **W2: verified trace factory** — compact traces, tiered validation, immutable manifests, coverage-balanced corpus.
3. **W3: student sweep** — four baselines, first-divergence metrics, length extrapolation, held-out program families.
4. **W4: hybrid modules** — frozen exact cores behind ABI tokens, learned planner, replay receipts.
5. **Market test** — find out whether buyers actually pay for trace-as-receipt, not merely compute.

The strongest near-term bet remains the hybrid: **learned planner plus frozen exact modules plus replay-verifiable traces**. The most important scientific uncertainty is whether the learned side can internalize any of the exact mechanism beyond local imitation. The most important engineering uncertainty is whether the substrate can widen safely without semantic bugs. The most important business uncertainty is whether verification-included compute clears at a better price than ordinary compute plus trust.

[1]: https://github.com/Percepta-Core/transformer-vm?utm_source=chatgpt.com "GitHub - Percepta-Core/transformer-vm: Compile programs directly into transformer weights. Includes a 2D convex-hull KV cache with O(log n) inference. · GitHub"
[2]: https://arxiv.org/abs/2106.06981?utm_source=chatgpt.com "Thinking Like Transformers"
[3]: https://arxiv.org/pdf/2301.05062?utm_source=chatgpt.com "Tracr: Compiled Transformers as a Laboratory for ..."
[4]: https://jmlr.org/papers/volume22/20-302/20-302.pdf?utm_source=chatgpt.com "Attention is Turing Complete"
[5]: https://arxiv.org/pdf/2506.12027?utm_source=chatgpt.com "Constant Bit-size Transformers Are Turing Complete"
[6]: https://arxiv.org/abs/2511.20038?utm_source=chatgpt.com "Softmax Transformers are Turing-Complete"
[7]: https://arxiv.org/abs/2207.00729?utm_source=chatgpt.com "The Parallelism Tradeoff: Limitations of Log-Precision Transformers"
[8]: https://arxiv.org/abs/2406.06467?utm_source=chatgpt.com "How Far Can Transformers Reason? The Globality Barrier and Inductive Scratchpad"
[9]: https://arxiv.org/abs/2402.12875?utm_source=chatgpt.com "Chain of Thought Empowers Transformers to Solve Inherently Serial Problems"
[10]: https://arxiv.org/abs/2306.15400?utm_source=chatgpt.com "Length Generalization in Arithmetic Transformers"
[11]: https://arxiv.org/abs/2410.15787?utm_source=chatgpt.com "Arithmetic Transformers Can Length-Generalize in Both Operand Length and Count"
[12]: https://arxiv.org/abs/1410.5401?utm_source=chatgpt.com "[1410.5401] Neural Turing Machines"
[13]: https://www.nature.com/articles/nature20101?utm_source=chatgpt.com "Hybrid computing using a neural network with dynamic ..."
[14]: https://arxiv.org/abs/1511.08228?utm_source=chatgpt.com "Neural GPUs Learn Algorithms"
[15]: https://arxiv.org/abs/1511.06279?utm_source=chatgpt.com "Neural Programmer-Interpreters"
[16]: https://arxiv.org/abs/2302.04761?utm_source=chatgpt.com "Toolformer: Language Models Can Teach Themselves to Use Tools"
[17]: https://arxiv.org/abs/2211.10435?utm_source=chatgpt.com "PAL: Program-aided Language Models"
[18]: https://arxiv.org/abs/2211.12588?utm_source=chatgpt.com "Program of Thoughts Prompting: Disentangling Computation from Reasoning for Numerical Reasoning Tasks"
[19]: https://arxiv.org/abs/2305.20050?utm_source=chatgpt.com "[2305.20050] Let's Verify Step by Step"
[20]: https://proceedings.mlr.press/v235/ni24a.html?utm_source=chatgpt.com "NExT: Teaching Large Language Models to Reason about ..."
[21]: https://arxiv.org/abs/2312.03052?utm_source=chatgpt.com "Visual Program Distillation: Distilling Tools and Programmatic Reasoning into Vision-Language Models"
[22]: https://arxiv.org/abs/2505.04441?utm_source=chatgpt.com "Towards Effectively Leveraging Execution Traces for Program Repair with Code LLMs"
[23]: https://webassembly.org/?utm_source=chatgpt.com "WebAssembly"
[24]: https://webassembly.org/specs/?utm_source=chatgpt.com "Specifications"
[25]: https://github.com/WebAssembly/spec?utm_source=chatgpt.com "WebAssembly specification, reference interpreter, and test ..."
[26]: https://www.cl.cam.ac.uk/~caw77/papers/mechanising-and-verifying-the-webassembly-specification.pdf?utm_source=chatgpt.com "Mechanising and Verifying the WebAssembly Specification"
[27]: https://compcert.org/?utm_source=chatgpt.com "CompCert - Main page"
[28]: https://users.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf?utm_source=chatgpt.com "Finding and Understanding Bugs in C Compilers"
[29]: https://link.springer.com/article/10.1007/s10723-019-09497-9?utm_source=chatgpt.com "BOINC: A Platform for Volunteer Computing - Springer Nature"
[30]: https://golem.network/?utm_source=chatgpt.com "Golem Network"
[31]: https://arxiv.org/abs/1908.04756?utm_source=chatgpt.com "[1908.04756] A scalable verification solution for blockchains"
[32]: https://eprint.iacr.org/2018/046?utm_source=chatgpt.com "Scalable, transparent, and post-quantum secure ..."
[33]: https://docs.pytorch.org/docs/stable/distributed.fsdp.fully_shard.html?utm_source=chatgpt.com "torch.distributed.fsdp.fully_shard"
[34]: https://docs.nvidia.com/nemo/megatron-bridge/latest/parallelisms.html?utm_source=chatgpt.com "Parallelisms Guide — Megatron Bridge"
[35]: https://awsdocs-neuron.readthedocs-hosted.com/en/latest/libraries/neuronx-distributed/context_parallelism_overview.html?utm_source=chatgpt.com "Context Parallelism Overview — AWS Neuron Documentation"
[36]: https://docs.nvidia.com/dgx-superpod/reference-architecture-scalable-infrastructure-h100/latest/network-fabrics.html?utm_source=chatgpt.com "Network Fabrics — NVIDIA DGX SuperPOD"
[37]: https://github.com/NVIDIA/nccl-tests/blob/master/doc/PERFORMANCE.md?utm_source=chatgpt.com "nccl-tests/doc/PERFORMANCE.md at master"
[38]: https://arxiv.org/abs/2205.14135?utm_source=chatgpt.com "FlashAttention: Fast and Memory-Efficient Exact Attention with IO-Awareness"
[39]: https://arxiv.org/abs/2310.01889?utm_source=chatgpt.com "[2310.01889] Ring Attention with Blockwise Transformers ..."

---

## Commentary (Fable / claude-fable-5, appended 2026-06-11)

This second analysis was produced by ChatGPT Pro from the research plan
and the full introduction, again without access to the codebase, the
issue tracker, or the last 48 hours of receipts. Same treatment as the
first ([`2026-06-11-chatgpt-pro-analysis.md`](2026-06-11-chatgpt-pro-analysis.md)):
where it is right, what it does not know already exists, where it needs
adjusting, and what we adopt. Written by the agent that maintains this
folder, with the workspace, psionic, and the open issues in front of it.

### Overall verdict

This is a better document than the first analysis, and a different kind
of document. The first was an execution plan; this is a research-program
grounding. Its two genuinely new contributions are (1) the literature
map — the lane's docs argued from first principles and our own receipts,
and this supplies the citation spine that connects every hypothesis to
prior art and, more importantly, to prior *negative results* — and
(2) the corpus-science and first-divergence-taxonomy layers, which are
concrete upgrades to W2 and W3 as filed. Its priority ordering (§11:
substrate before scale, factory before students, hybrid as the robust
bet, market test last) independently reproduces the research plan's own
sequencing, including the W1-gates-everything correction that the first
analysis needed lane commentary to get. Convergence from a second
independent source is mild evidence the ordering is right.

### Where it is right, and should be treated as settled

1. **The refined thesis sentence.** "Exact computation should be treated
   as an engineered substrate, not a capability we merely hope gradient
   descent discovers" is the program's one-line answer to every "why not
   just train it?" question, and it is sharper than anything in our own
   docs. Adopt as framing language alongside §11's public stance
   paragraph, which is also correct and quotable as-is.
2. **The literature map, especially the negative results.** The
   expressivity-vs-learnability split (Turing-completeness results vs
   the parallelism tradeoff and the globality barrier) is exactly the
   theoretical backbone H1 needed — our falsifier was stated from
   instinct; now it has a citation trail. The NTM/DNC/Neural-GPU/NPI
   lessons (explicit memory helps, trace supervision helps, length
   generalization is fragile, architecture bias is usually necessary)
   are the same conclusions our March 2026 learned-exactness plateau
   reached empirically — the audit's commit-history arc
   (`e8e076ddd` → `50c8fdb86` → `fa80adcf8`) is an unpublished replication
   of that literature, and the lane should say so when it writes up W3.
   The trace-complexity warning (limited gains in 2 of 6 configurations)
   was already adopted via the first analysis; this one explains *why*
   compact, structured, auxiliary-supervised traces are the right call.
3. **The first-divergence taxonomy.** The twelve-class error taxonomy
   (instruction_fetch_error … halt_error) is the single most adoptable
   artifact in the document. Our replay verifier
   (`tassadar_alm_trace_replay.rs`) names the first divergent step but
   does not classify it; the taxonomy turns a step number into a
   research signal ("divergence shifted from syntax to memory-alias
   errors" is progress even at constant pass rate). Adopt verbatim into
   the W3 eval schema on openagents#4749.
4. **Corpus science as a named discipline.** The coverage-scoring
   dimensions (branch entropy, alias density, near-miss stress,
   first-divergence diagnostic value), the named eval suites, and the
   "coverage-balanced, not just large" rule are a real upgrade over
   W2-as-filed, which specified scale targets and families but not a
   quality layer. The active-curriculum loop in Phase 3 (generate new
   stress traces from the student's divergence profile) is the harness
   generator and the eval schema composed into a flywheel — cheap to
   build once both exist, and exactly how the factory should spend its
   marginal token.
5. **Compiler QA as the center, not a support function.** Critique 4/
   Avenue 7's stack (reference semantics, randomized differential
   generation, translation validation, proof-carrying artifacts,
   regression pinning) is the methodology our differential harness
   already vindicated at small scale. The Isabelle-mechanized WASM
   semantics and the official spec test suite belong in W1.1's
   conformance ladder (psionic#1119) as external bars, the same role
   transformer-vm's examples already play.
6. **The weak-arguments list (§9).** All ten are restatements of our
   claim boundaries, which means an external reviewer reading only our
   public docs reconstructed the discipline correctly. That is what the
   discipline is for. Item 10 ("dense weight modules are ready if only
   scalar/numeric artifacts exist") earns specific praise: it is the
   precise overclaim W1.2 exists to prevent.

### What it does not know is already built or already happened

- **The factory planes are not greenfield.** Like the first analysis,
  it designs Planes A–C and F as future systems. They ran live on
  2026-06-10/11: production assignment route, worker-as-validator
  replay verdicts (tampered digest correctly Rejected), paid Lightning
  closeouts, and — as of last night — all five verification classes
  exercised on real dispatched production work with 37 new public
  settled receipts. What is genuinely new in its plane diagram is
  **Plane D, the corpus curator** (diversity quotas, immutable
  manifests, admission policy as a distinct role). W2 should name the
  curator explicitly; it is the one role our rails do not yet have.
- **The workstreams are filed, not proposed.** W1.1–W1.4 are psionic
  #1119–#1122; W2/W3/W4.1 are openagents #4748/#4749/#4750. Its §6
  phase agenda maps almost one-to-one onto those issues, which is
  convenient: the deltas below can land as issue comments rather than
  new structure.
- **The projection-staleness defense is already case law.** Its attack
  table includes "public dashboard stale → rebuild on validation
  transitions" — the lesson this platform paid for twice over
  (openagents #4744–#4746) and promoted to a standing method rule in
  `RESEARCH_PLAN.md` §6. The analysis likely absorbed it from the
  directive; either way, it is policy, not advice.
- **Avenue 9's "boring by design" profile is the current profile.**
  Integer-only, deterministic, bounded, no host calls, typed refusals
  for traps and resource limits — that is a description of the
  twelve-opcode window as it exists, not a recommendation. The security
  list's forward-looking items (module conformance before marketplace
  admission, verifier diversity, ABI prompt injection against hybrid
  planners) are real and belong to W4's day, not W2's.
- **The numerics concern has an existing anchor.** Critique 5's
  "numerical margins program" lands on machinery the lane already
  maintains: the scalar-f32 semantics matrix with fast-math refusal in
  psionic, and the 2⁵³-checked numeric artifact from #1113. What is
  missing is exactly what it asks for — *certified margins under BF16/
  FP8* — which is W1.4's softmax-bounds work plus one new deliverable
  (see below).

### Where it needs adjusting for our actual situation

1. **Do not reopen the frozen four-baseline design.** Training variants
   4 (verifier-in-the-loop RL / rejection fine-tuning), 5 (synthetic
   error-correction students), and 6–7 are good experiments, but
   openagents#4749 froze the four-baseline sweep deliberately so nobody
   improvises the experiment design mid-flight. The variants join the
   W3 backlog *behind* the frozen sweep, as issue comments, and remain
   subject to the same gates: no training before W2's first 100M
   verified tokens, and no public gradients into the main optimizer
   ever — variant 4 especially must not become a backdoor for
   contributor-side gradient acceptance.
2. **The module ABI sketch should bind to the existing plugin
   machinery, not parallel it.** Avenue 5's `<exact.call>`/
   `<exact.trace>` shape is fine as a wire format, but psionic already
   owns the module manifest / catalog / linker / packet-ABI family and
   the Rust PDK (`tassadar_module_*`, plugin packet ABI). The Avenue 6
   package-metadata block is a strict subset of what the module
   manifest plus capability matrix already carry. W4 should extend
   those surfaces; a second ABI invented beside them would be exactly
   the parallel-primitive mistake the workspace rules exist to prevent.
3. **`pass@k` needs a claim-discipline caveat.** The metric list adds
   `exact_rollout_pass@k` alongside `pass@1`. Useful for research
   (it measures whether capability exists anywhere in the sample
   distribution), but no public claim should ever cite pass@k for an
   executor-class behavior — sampling until something replays is the
   statistical lane's vocabulary, and the naming rule says students
   carry Psion's claim vocabulary, never Tassadar's. Report pass@k
   internally; publish pass@1 behind replay.
4. **The theory agenda (Avenue 11) must stay experiment-tied or it will
   sprawl.** Five theory programs is a department, not a workstream.
   The one with near-term leverage is the **margin-certification
   question** (finite-precision lookup margins), because it gates dense
   materialization on real inference engines (W1.2) and any future
   softmax/BF16 posture (W1.4). Adopt that one as a named W1
   deliverable; leave the rest as reading-list questions until an
   experiment needs them.
5. **GPU topology sections remain on file, not in plan.** Same
   adjustment as the first analysis: we do not own the cluster this
   plans for, the asynchronous network we do own is for
   generation/validation/evaluation only (already policy), and nothing
   in Phases 0–2 requires hardware we lack. The FlashAttention/Ring
   Attention pointers are correctly scoped by the document itself —
   student-training systems, irrelevant to the compiled hull path.
6. **Provenance hygiene, again.** Several citations blend our committed
   claims with Percepta's published ones (e.g. attributing trace-replay
   capabilities to "your introduction" where the underlying fact is a
   psionic test, or vice versa). The standing rule from the full
   introduction applies to this document too: Percepta's numbers are
   their claims, ours are committed tests, and a downstream reader must
   never have to guess which is which.

### What we adopt now

Concretely, into the filed workstreams:

- **Into #4749 (W3):** the twelve-class first-divergence taxonomy,
  verbatim, as part of the eval schema; `pass@k` as internal-only with
  the claim-discipline caveat above; training variants 4–7 logged as
  post-sweep backlog.
- **Into #4748 (W2):** the corpus coverage-scoring dimensions and named
  eval-suite layout as part of the day-0 contract freeze; the artifact
  state machine (submitted → … → admitted_to_training → deprecated) as
  the lifecycle vocabulary for the validator verdict schema; the
  economic attack/defense table as the red-team checklist; the
  **curator** as a named role the factory must staff.
- **Into psionic#1119 (W1.1):** the profile-registry schema (it is a
  superset of our profile-versioning rule — adopt the missing fields:
  trap_policy, lowering_policy, test_suite_hash, known_limitations);
  the official WASM spec test suite and the Isabelle mechanization as
  external conformance references for the window ladder.
- **Into psionic#1122 (W1.4):** the margin-certification deliverable —
  certified lookup margins under FP32/BF16, per module, as the
  precision half of the softmax-bounds work.
- **Into the folder:** §10's reading list as the lane's standing
  citation spine, and the §11 stance paragraph as approved public
  framing language.

What we explicitly do not adopt: a second module ABI beside the psionic
plugin machinery; succinct proofs now (the document itself agrees);
synchronous training topology work before an operator commits hardware;
any reopening of the frozen W3 sweep design.

The document's most useful sentence is its last uncertainty triplet:
the scientific risk is whether the learned side internalizes anything
beyond local imitation, the engineering risk is whether the window
widens without semantic bugs outrunning the harness, and the business
risk is whether verification-included compute clears at a better price
than compute plus trust. Those are H1, the W1 kill condition, and H6 —
the program's own top risks, found independently by a reviewer who
could only see the documents. The risk register survives contact with
an outside auditor unchanged, which is the most reassuring thing a
plan can do.


