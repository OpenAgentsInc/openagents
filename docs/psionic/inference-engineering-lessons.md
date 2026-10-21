# Inference Engineering: lessons for DeepSeek and Pylon

Written 2026-10-21. Repository baseline:
`af238b40188f7ecd5e36ae70e322493605e5f807`.
These are proposed adaptations, not new performance or qualification results.

**Recommendation:** finish qualification of the pinned serial V4.1 runtime,
measure complete useful answers, then improve prefill in three separate steps:
bounded chunks, genuinely batched computation, and overlap between stages.
After that, investigate local prefix reuse and short speculative verification.
Continuous batching, expert parallelism, and prefill/decode disaggregation
belong to a later workload with demonstrated demand.

The [completion checklist](deepseek-v41-completion.md) remains authoritative
for #12369. This document adds reasoning and experiments to it and the
[implementation roadmap](implementation-roadmap.md); it does not replace
their gates or add every optimization as a launch prerequisite.

## Source and reading scope

Source: Philip Kiely, *Inference Engineering*, Baseten Books, copyright 2026,
ISBN `979-8-9943597-2-3`. Read the supplied 259-page PDF, focusing on the
sections below. Its SHA-256 is
`b297183c38ffc7ffa30100e1aab0af3b1c20b49778939ee976522fe39ac0df89`.
The PDF's metadata records a modification date of 2026-01-29. It supplies
general principles, not documentation for our later V4.1 checkpoint.

Page references below use **printed page numbers**; add two for the PDF
viewer's page number. Figures 1.4/1.5, the parallelism table on page 143,
and Figure 5.17 were also checked visually. The PDF and extracted book text
are not copied into the repository.

| Reference | Book sections and printed pages | Application here |
| --- | --- | --- |
| B1 | 1.1–1.4, pp. 26–37 | Workload, model choice, latency, and quality baselines |
| B2 | 2.4–2.5, pp. 61–69 | Arithmetic intensity, memory traffic, attention |
| B3 | 3.3 and 3.5, pp. 83–91 | Whole-machine constraints and local/cloud tradeoffs |
| B4 | 4.1.1–4.1.3, pp. 98–101; 4.5, pp. 112–115 | Kernel choice, fusion, benchmarking, profiling |
| B5 | 5.1, pp. 120–129 | Precision choices and quality evaluation |
| B6 | 5.2, pp. 129–136 | Speculation costs, acceptance, and workload fit |
| B7 | 5.3, pp. 136–141 | Prefix reuse, storage tiers, routing, long context |
| B8 | 5.4–5.5, pp. 142–151 | Parallelism and conditional disaggregation |
| B9 | 7.1–7.3, pp. 179–198 | Build dependencies, admission, cold starts, reliability |
| B10 | 7.4–7.5, pp. 199–207 | Deployment, cost, observability, and client latency |

## What we already have

Keep the distinction between implemented mechanisms and accepted model results:

- **Execution:** native Rust model loading, packed FP4/FP8 CUDA weights,
  device-resident stage state, captured per-token graphs, and sampling on
  the last stage. The current prefill enqueues that graph once per prompt
  token; it is not a matrix-matrix prefill implementation. See
  [`V41Stage`](../../crates/psionic/crates/psionic-backend-cuda/src/v41_stage.rs).
- **Distribution:** typed stage messages, direct authenticated links,
  bounded receiver handoff, one admitted generation, cancellation barriers,
  generation fencing, and group readiness. Extend
  [`stage_group`](../../crates/pylon/src/stage_group.rs),
  [`serve`](../../crates/psionic/crates/psionic-v41-stage/src/serve.rs), and
  [`stage-link`](../../crates/stage-link/src/lib.rs).
- **Deployment:** stage-only shard selection, cloud placement, an expiring
  spend guard, boot readiness, and build bundles. #12410 now carries the
  verified bundle hash through qualification and launch; #12411 fixes rapid
  VM-name reuse in settlement; #12412 makes raw-text decoding incremental.
  Their SM120 integration checks remain separate evidence. Do not schedule
  the old audit defects as new implementation work.
- **Qualification:** a direct-runtime harness, full-vocabulary comparison,
  negative controls, and a frozen holdout definition exist under #12405.
  Final numerical policy/calibration and model acceptance remain pending.
  Plausible answers and closed infrastructure issues do not pass G1/G2.
- **Measured baseline:** the older one-machine run reports 35.589 s to the
  head's first token for a 1,072-token prompt, versus about 29.7 decode
  tokens/s on short prompts. Cross-machine receipts include both required
  placements and faults, but predate the latest fixes. Those results are
  historical measurements of an unqualified runtime, not current guarantees.

The current four/eight-stage CUDA admission bound is **2,872 prompt tokens**,
with an 8,192-position default context and a separate output budget. It is
limited by encoded state size, not the checkpoint's advertised context.
The W14 receipt's 3,022-token label is disputed. The 19,989-token oracle
prompt remains outside this unchunked profile. See the
[budget tests](../../crates/psionic/crates/psionic-v41-stage/tests/budget.rs),
[one-machine receipt](verification/deepseek-v41-g5-one-quad-2026-10-21.json),
and [cross-machine receipt](verification/deepseek-v41-g6-g7-cross-machine-2026-10-21.json).

That operational bound does not waive G1's required long-context coverage.
For final acceptance, either support and qualify the checklist's full scope
or explicitly approve a narrower release and revise its gates, policy, and
frozen corpus prospectively. Missing checks remain pending in the meantime.
W14's existing ceilings are a 600 s request deadline, a 75 s step limit,
and 4,096 output tokens. Those are operational limits, not evidence of an
interactive user experience or a latency target for a future optimization.

## 1. Measure useful answers, not just generated tokens

The book separates inference time from end-to-end latency and distinguishes
per-user token rate from aggregate throughput. It also treats a completed
tool call as the useful unit, rather than its first token. Adopt that
distinction directly. [B1, B10]

Our [`GroupEngine`](../../crates/pylon/src/group_serve.rs) records its first
received token, collects text and parsed answer pieces, waits for completion,
and returns a `Generation`. Early tokens may be private reasoning. Therefore
the logged first-token time is neither buyer-visible streaming latency nor
time to the first useful final answer.

Extend the existing measurement path with these distinct observations:

| Observation | Start and end |
| --- | --- |
| Admission wait | Buyer request sent to group admission; keep discovery retries separate |
| Internal first token | Group admission to first sampled token at the head |
| First final-answer text | Admission to the first parsed content token, excluding reasoning |
| Buyer completion | Buyer request sent to a complete, valid final response |
| Decode rate | Generated tokens after the first divided by decode time; identify reasoning, final, and EOS counts |
| Recovery interval | Fault detection to readiness and a successful subsequent request |

For the current buffered delivery, buyer completion is the product latency.
If streaming is later added, carry final-answer pieces through the existing
engine/provider path, preserve reasoning separation and backpressure, and
measure first text at the buyer. An internal stream alone does not implement
that feature.

Keep failed, cancelled, oversized, and output-capped requests in outcome
counts. Report complete-answer latency alongside failure rates; reporting
only successful requests can hide overload. Use sample counts and repeated
runs before drawing percentile conclusions; a dozen prompts cannot establish
a stable p99. Reuse `StepProfile` and the W14 harness rather than introducing
a second benchmark service.

## 2. Profile our actual prefill before assuming the usual bottleneck

The book's typical compute-bound prefill depends on processing many prompt
tokens together, amortizing weight reads across matrix-matrix work. Our CUDA
path instead replays a token-at-a-time graph during prefill. Removing CPU
round trips was valuable, but does not create that arithmetic intensity.
The book's compute/memory classification is a hypothesis to test for each
operation, not a description we can assign from the phase name. [B2, B4]

Use a bounded Nsight Systems capture for an isolated stage, then a selected
hot kernel capture if needed. Separate weight reads, attention/cache access,
Engram host-memory reads, graph launches, device synchronization, host copies,
codec work, and link waits. Stage elapsed time is not a pure GPU kernel timer;
record which timing domain each number uses.

Then make three independently reviewable changes:

1. **Bounded chunked prefill.** Make prompt length independent of one wire
   message's token count. Derive the chunk limit from every boundary's
   encoded bytes, including candidates, selections, KV rows, pre-mix, and
   metadata. Begin without overlap, retaining the existing token arithmetic.
2. **Batched computation within a stage.** Replace suitable repeated
   projections with batched GEMM; group MoE work only where routing makes
   it compatible. Keep causal attention, compressor completion, index reuse,
   and Engram history ordered. Measure padding, workspace, and effective
   weight traffic. Merely increasing `max_step_tokens` is not this change.
3. **Overlap dependent prompt chunks across stages.** After a chunk's state
   reaches stage 1, stage 0 can work on the next chunk if all state and
   buffer dependencies permit it. Bound chunks in flight by bytes and memory,
   not only by count. Keep one user generation admitted throughout.

Chunking requires a defined mapping between the front's logical generation
step and the workers' chunk sequence and absolute positions. An intermediate
prefill chunk must not sample an output token or appear as a completed decode
step. Cancel/fence must stop and drain every affected chunk before admission
opens again. Extend the existing versioned state contract rather than adding
an unrelated transport.

Test chunk boundaries around compression/window transitions, odd positions,
and Engram history, plus cancellation between chunks, duplicates, stale
frames, and link failure. Requalify each changed arithmetic or overlap
schedule. Chunking removes a wire obstacle; reaching 20k also requires a
larger context allocation, measured memory, and corresponding qualification.

## 3. Keep pipeline stages for capacity; measure latency separately

The book favors tensor parallelism for low single-request latency on suitable
interconnects and warns about pipeline utilization. That warning applies:
our serial decode visits all stages before producing the next token. Four
GPUs do not automatically provide four times the token rate. [B3, B8]

For our present schedule, a useful accounting model is:

```text
decode-token latency ≈ sum(stage time) + sum(handoff time) + head/return work
```

Only an implemented concurrent pipeline can use the slowest stage as its
steady-state throughput model; fill/drain and dependencies still affect
individual latency. The current split buys enough combined device memory
and practical links. It is a valid capacity strategy even when it does not
accelerate a single request.

Extend W3/placement measurements with actual weight, workspace, cache,
candidate/indexer, Engram, and in-flight-chunk memory. Count shared host
mappings correctly. MoE's active weights help estimate per-token traffic;
total resident weights still determine capacity. Use legal state boundaries
and measured stage cost, not even layer counts alone.

Compare one-machine, four-single, and two-dual placements on the same pinned
profile. Separate actual cross-machine bytes from logical hop bytes, and
report encode/copy/transfer/decode time. Consider within-node tensor
parallelism only after measuring the specific machine's peer links and
collectives. Expert parallelism needs measured dispatch **and return/combine**
costs and expert skew; the book's throughput argument does not justify
expert routing across arbitrary Pylons or a WAN.

## 4. Treat reusable state as more than a KV cache

The strongest cache opportunity for personal use is repeated system/tool
context and conversation prefixes. Start with one warm group's local prefix
reuse, before remote cache storage or global cache routing. Keep stable
prompt content before changing content where semantics permit it. [B7]

For V4.1, a usable prefix snapshot must include the state needed to resume
every stage: window/compressed KV, pending compressor groups, index keys and
selections, candidate state, position, Engram history, and the last committed
token or residual/pre-mix needed at the chosen resume point. A boundary message
is a handoff of selected state, not a complete durable snapshot of a group.
Restoring only generic K/V tensors is insufficient.

Bind reuse to exact rendered token IDs, checkpoint, tokenizer/template/mode,
verified runtime bundle, precision, cache layout, stage manifest, and access
scope. Keep cache snapshots immutable or copy-on-write; a later request must
not mutate a prefix used by another. Generation changes invalidate live
ownership; any reuse across replacement must be explicitly restored and
verified, never inferred from a cache advertisement.

Compare a verified hit, a miss, and a cold reset on identical continuations.
Measure saved prefill against lookup, restoration, copying, eviction, and
memory costs. When replicas eventually exist, choose by estimated completion
time including queue delay, restore cost, and uncached suffix work. A hot
cache on a busy group can lose to a cold idle group.

## 5. Preserve precision and quality as explicit execution choices

The book's useful lesson is component-specific precision and application
evaluation, not a blanket preference for one low-bit format. Our original
checkpoint already has packed FP4/FP8 weights and scales; preserve those
semantics before proposing another quantization. Treat residual, activation,
cache, and wire formats as separate choices. [B5]

Keep G1 v2's same-mode full-distribution and margin-aware checks, exact
structural checks, and G2's executable task assertions. Apply them to new
GEMM shapes, fusion, chunks, cache formats, and speculation. An algorithm
described as lossless does not promise bit-identical floating-point results
after a hardware or reduction-order change. Conversely, a different valid
answer is not by itself a numerical failure.
The book's discussion of evaluation noise does not derive arithmetic
tolerances; a deterministic oracle rerun neither sets them to zero nor
permits relaxing them after observing candidate failures.

Do not adopt the book's blanket dismissal of integer formats or its generic
precision speedup estimates as acceptance rules. Psionic already serves
quantized GGUF families. A supported artifact/kernel combination earns its
status through measured quality, memory, and end-to-end performance.

Keep the R1-Distill-Qwen-1.5B short-greedy profile as an independently
qualified low-resource option. The book's model-selection advice supports
using a smaller model when it passes a task's requirements; it does not make
the distill equivalent to V4.1 or qualify longer/sampled R1 reasoning. [B1]

## 6. Use speculation only after verification and rollback exist

Serial personal serving is a plausible fit for speculation because it does
not consume compute with large concurrent batches. But acceptance rate is
only one term in its value. [B6]

```text
committed tokens/s = committed tokens /
  (draft + target verification + sampling + rollback + transport time)
```

First implement and qualify a short fixed-length target verification path.
Rollback must restore all stage state listed above, including partially
completed compressor groups and parser/constraint state. Expose only
committed tokens. Test accept-all, reject-first, partial acceptance, EOS,
cancellation, and stage loss. Greedy checks come first; sampled speculation
requires the correct acceptance/resampling algorithm and distribution tests.

N-gram drafts are a reasonable later code-editing experiment without new
trained weights. EAGLE/MTP require compatible artifacts and integration;
the existing Qwen speculation work is a reference, not V4.1 support.
Benchmark speculation off against short fixed lengths at the same memory
budget. Do not turn token prediction into a feature merely by adding an
acceptance threshold.

## 7. Operate a whole group as the deployable unit

Apply the book's reproducible dependencies, cold-start breakdown, and
deployment observability to our existing cloud launcher and supervisor.
Kubernetes is not required to use those ideas. [B9, B10]

Record capacity wait, boot, build/library verification, stage shard transfer,
hashing, weight load, Engram preparation, graph warmup, all-stage readiness,
and first complete answer separately. Extend the current bundle pins and
ready markers. Caching a compiled worker helps startup; it does not remove
the cost of loading hundreds of gigabytes or qualifying its target.

For a group update, replace or drain a complete compatible group. Do not
canary arbitrary stages from two numerically different bundles in one
generation. If there is no capacity for a second group, use a planned
drain-and-restart and report the interruption. Warm standby and blue-green
deployment require explicit additional GPU/host-memory reservations.

Keep interrupted requests visibly failed after delivered output; readiness
recovery is not continuation of a partially generated answer. Retain the
existing stop barrier and repeat fault tests after changing chunking,
snapshots, or speculation. Connection reuse and same-zone stage locality
are useful; quantify their savings before replacing transport.

Report complete valid answers within the deadline per paid group-hour and
per measured cloud cost. Include load time, idle time, and failed/preempted
attempts; itemize compute, storage, and transfer charges, and track engineering
cost separately. Preserve the distinction between spend reservations,
estimated settlement, and provider billing. The book's hardware tables and
traffic thresholds are not a current cloud catalog or a purchasing rule.

Scale-to-zero can suit explicit personal sessions if the cold wait is shown.
It cannot promise an interactive first response before a full group loads.
For future multiuser service, scale whole compatible groups and bound
admission by token work and state memory, not request count alone.

## Prioritized adaptations and existing issue ownership

These are proposals for the existing owners, not newly filed or claimed issues.
Issue state below was checked on 2026-10-21; code completion and after-close
device evidence remain separate.

| Order | Work | Existing home and completion criterion |
| --- | --- | --- |
| Now | Finish the pinned numerical and task-quality baseline | [#12405](https://github.com/OpenAgentsInc/openagents/issues/12405), open; [W2 #12389](https://github.com/OpenAgentsInc/openagents/issues/12389), open. Retain missing 20k coverage explicitly. Do not replace the pending run with a book-based tolerance. |
| Now | Separate internal token timing, final text, and buyer completion; retain representative prompt IDs | W2 and W14 after-close measurements; extend `GroupEngine`, `StepProfile`, and the existing harness. Include failures, exact modes, warm/cold runs, and sample counts. Recount the disputed long prompt before using it. |
| Next, for the checklist's full context scope | Bounded non-overlapped prefill chunks | New follow-up to W11/W12 and roadmap R0/R4. Pass codec/state/position and cancellation tests, then original-weight chunk-versus-baseline qualification. Deferral requires an explicitly approved narrower release contract. |
| Next performance work | Batched stage prefill, then chunk overlap | Follow roadmap R1/R3 with the V4.1-specific state contract. Profile first; change one mechanism at a time. Retain TTFT, complete-answer latency, memory peak, and accuracy for each schedule. |
| Alongside measurements | Refine placement and cold-start accounting | [W3 #12390](https://github.com/OpenAgentsInc/openagents/issues/12390), code-complete; W12/W14 after-close checks. Extend measured inputs and repeat affected results rather than reopening the original task by default. |
| After the baseline | Local prefix snapshots, then fixed-length speculation | Roadmap R8/R9. Require complete state restoration and measured end-to-end savings; qualify each separately. |
| Final launch | Re-run the declared profile and recovery on the same qualified bundle | [W16 #12403](https://github.com/OpenAgentsInc/openagents/issues/12403), open. Satisfy the existing G1–G8 checklist; older receipts do not certify later schedules. |
| Later, demand-driven | Continuous batching, cache-aware replicas, or conditional P/D | Separate multiuser phase with measured traffic, resource, quality, and cost targets. Preserve the personal serial profile. |

Prefill/decode disaggregation should wait until avoided interference or
specialization savings exceed cache transfer, layout conversion, extra
capacity, and orchestration costs. V4.1 needs more than a generic KV tensor
to move a live request. The book's high-volume examples explain the workload
where this can pay off; model size alone is not sufficient justification.
Likewise, chunked prefill within one group is not P/D disaggregation. [B8]

## Measurement plan for the next paid GPU session

Use the existing spend guard and isolated benchmark buyer. This document
authorizes no cloud allocation and adds no production shadow traffic.

1. Identify the verified worker/head bundles, checkpoint, template/mode,
   driver/device, placement, context/output limits, and schedule. Run the
   required baseline numerical, answer, and recovery checks first.
2. Use retained inputs covering short chat, chat-mode code, thinking-mode
   code, the 1,072-token case, the 2,124-token oracle case, and the checked
   prompt/context limits. Test rejection just above the limit separately.
   A future chunked profile adds its own longer-context corpus.
3. Keep cold start, warm first execution, warm repeated requests, and any
   genuine prefix hits separate. Match seeds, modes, and output caps; do not
   merge results from changed prompts or compare reasoning tokens with
   final-answer tokens as if they were the same workload.
4. For each optimization, compare baseline, that change alone, and the
   intended combination. Retain raw timings, decoded outcome, actual token
   counts, wire bytes, host/GPU memory, sample count, and profiling overhead.
   State the measurement limits; do not infer tail SLAs from a small suite.
5. Re-run cancellation, stage kill/freeze, link loss, and successful subsequent
   generation. Validate that every discarded chunk or speculative token is
   stopped before the group advertises a free slot.

The book reading and repository inspection are the evidence for this plan.
No new model, cloud, latency, or cost benchmark was run for this document.
The cached code graph lacked these stage/cloud modules, so implementation
mapping used scoped source inspection after querying the cache.
