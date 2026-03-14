# 2026-03-13 Full Rust Inference Engine Gap Audit

> Historical note: this is a point-in-time architecture audit written on
> 2026-03-13. Current product authority still lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, and the retained MVP implementation scope. This document
> is a longer-horizon engine audit, not a statement that all of this should be
> pulled into the current MVP immediately.

## Intent

This audit answers a narrower question than the earlier Prime ecosystem audit:

> what is still missing for OpenAgents to honestly claim a full Rust-native
> inference engine, and what GitHub issue program would finish that from the
> current Psionic state?

It also resolves the earlier Prime question more precisely:

- Prime does have inference-serving repos.
- Prime does not really have one single Rust-native inference engine of its own.
- Prime's serving stack is mostly distributed serving around existing engines
  and runtimes such as `vLLM`, plus research pipeline code.
- OpenAgents has already rebuilt some of the useful serving and topology ideas
  in Rust.
- OpenAgents has not yet finished the full engine-complete Rust-native stack.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md`
- `docs/plans/prime-ecosystem-compute-integration-spec.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/ROADMAP_CLUSTER.md`
- `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- `crates/psionic/docs/INFERENCE_ENGINE.md`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `crates/psionic/psionic-serve/src/openai_http.rs`
- `crates/psionic/psionic-provider/src/lib.rs`
- `~/code/pi/README.md`
- `~/code/pi/prime-vllm/README.md`
- `~/code/pi/prime-pipeline/README.md`
- `~/code/pi/sglang/README.md`
- prior performance and gap audits under `docs/audits/`

## Executive Summary

OpenAgents is no longer at "we should build a Rust inference engine someday."
Psionic already is a real Rust inference substrate:

- it owns tensor, IR, compiler, runtime, models, serving, provider, cluster,
  and backend crates
- it has a real shipped CUDA GPT-OSS inference lane
- it has a real native Metal GPT-OSS lane, even if that lane is not yet fully
  closed
- it has real clustered serving ideas already landed: replica routing,
  pipeline, layer sharding, tensor sharding, manifest truth, and cache truth

But that is still not the same as a full Rust-native inference engine.

Today the honest description is:

- Psionic has a strong Rust-native inference substrate.
- Psionic has one real high-value decoder family execution path pushed much
  further than the rest: GPT-OSS.
- Psionic does not yet have the generality, API completeness, scheduler shape,
  backend closure, and productized runtime behavior needed to call the whole
  system "full" in the same sense people mean when they talk about
  `vLLM`/`SGLang`/`llama.cpp`-class inference runtimes.

The biggest remaining gaps are:

- real multi-family executed decoder support, not just metadata support
- a real batching and scheduler layer instead of mostly single-request local
  decode
- closure of the native Metal path without `llama.cpp` proxy ambiguity
- broader Psionic-native HTTP/API surface beyond the current GPT-OSS chat lane
- adapter support, interoperability, and cluster/local serving parity
- production-grade validation, replay, and acceptance gates

## What Prime Actually Has

Prime's inference story is valuable, but it is important to read it correctly.

### What exists in Prime

- `prime-vllm`
  - distributed inference around `vLLM`
  - public-network pipeline transport
  - sharding and intermediate transfer
- `prime-pipeline`
  - research pipeline inference sandbox
  - stage-boundary experiments and benchmarking
- `sglang`
  - scheduler and serving-runtime reference material
- `cloud-lora`
  - adapter-hosting and adapter-product ideas

### What that means architecturally

The most useful Prime lessons are:

- stage and shard identity must be explicit
- remote transport and handoff truth must be explicit
- cache and prefix reuse must be explicit
- operators need benchmark and placement evidence
- the serving plane and the engine plane should be separable

The least useful thing to copy directly is the actual implementation stack:

- Python runtimes
- `vLLM`-specific internals
- one-off research pipeline code

For OpenAgents, the right move is still:

- take the serving and topology ideas
- rebuild them in Psionic-owned Rust
- finish the engine-complete substrate ourselves

## What OpenAgents Already Has

OpenAgents has already pulled in more of the important Prime serving ideas than
it may look like at first glance.

### Already landed in Psionic

- Rust-owned tensor, runtime, and backend stack
- served-artifact identity and cache invalidation contracts
- sharded-model manifests and pre-shard artifact handling
- clustered replica routing
- clustered pipeline-parallel execution
- clustered layer-sharded execution
- clustered tensor-sharded execution
- prefix-cache and KV-compatibility truth in clustered receipts
- local runtime observability, loaded-model state, and capability envelopes
- real CUDA GPT-OSS execution through the Psionic-owned server path

### Already true but narrower than it sounds

- `psionic-models` can classify and load GGUF metadata for Llama, Qwen,
  Mistral, and GPT-OSS families
- `psionic-serve` has real local text-generation and embeddings services
- the shipped OpenAI-compatible server is still a narrow GPT-OSS server
- the strongest real executed path is still GPT-OSS-first

That distinction matters. The repo already has a serious engine substrate, but
it is still not a general finished inference runtime.

## The Real Missing Pieces

## 1. Executed model-family coverage is still narrower than loader coverage

Psionic can load much more model metadata than it can fully serve today.

The cleanest example:

- `psionic-models` supports GGUF decoder-family classification for `llama`,
  `qwen`, `mistral`, and `gpt_oss`
- the actually productized real-GGUF execution path is still centered on
  `gpt_oss`
- the generic model-backed text-generation services are still anchored to the
  earlier `ArtifactWordDecoder` path rather than a generic real-GGUF decoder
  runtime

So one core gap is:

> Psionic has a broader model-ingress layer than it has a broader real executed
> decoder engine.

## 2. The local serving scheduler is still intentionally modest

The runtime contract already knows how to describe:

- `single_request_only`
- `caller_static_batch`
- `scheduler_static_batch`
- `continuous_batch`

But the current conformance/evidence contract is explicit that local text
generation is still:

- `single_request_only`
- direct caller backpressure
- one active request
- no internal shared scheduler queue

That is a major reason the current system is not yet a full
`vLLM`/`SGLang`-class engine.

What is missing is not just "more speed." It is:

- continuous batching
- mixed prefill/decode scheduling
- fairness and starvation rules
- cancellation behavior under shared scheduling
- queue admission and backpressure truth

## 3. The shipped HTTP surface is still too narrow

The current OpenAI-compatible server in `psionic-serve/src/openai_http.rs`
ships:

- `/health`
- `/v1/models`
- `/v1/chat/completions`

and that server is GPT-OSS-specific.

It does not yet represent a general Psionic-native serving plane for:

- generic multi-family text generation
- `/v1/embeddings`
- `/v1/responses`
- structured outputs
- constrained decoding
- tool-calling
- logprobs/top-logprobs
- multi-model routing under one generic runtime

That is a serving-surface gap, not just a kernel gap.

## 4. Native Metal is still an open completion track

The current repo still keeps an explicit open Apple-native queue in the Psionic
roadmap:

- `#3270`
- `#3268`
- `#3269`
- `#3271`
- `#3272`
- `#3261`
- `#3262`

The reason is straightforward:

- proxy mode against `llama.cpp` still exists
- the roadmap still says the shipped native Metal GPT-OSS path needs closure on
  device KV, remaining host-owned decode work, readback/wait removal, bounded
  logits, and parity/perf evidence

So even though Metal is no longer just an idea, it is still not engine-complete
enough to count as a closed backend.

## 5. CPU, CUDA, and backend truth are unevenly productized

CUDA has the strongest real decoder path today.

But a full engine means more than "one NVIDIA path is good":

- the CPU GGUF GPT-OSS service exists, but the shipped GPT-OSS OpenAI server
  still explicitly says the CPU backend is not implemented there
- the generic CUDA decoder substrate is still too tied to the current GPT-OSS
  success path
- AMD still remains future work for served execution

So the backend story is still:

- one strong path
- one partial native path
- one partially surfaced CPU path
- one future backend

That is not yet a complete backend matrix.

## 6. Cache and memory systems are only partly productized

Psionic already has real cache truth:

- paged-KV policy types
- prefix-cache accounting
- cache invalidation policy
- served-artifact identity

But a full engine still needs more than schema truth:

- shared scheduler ownership of KV residency
- real eviction and spill policy under load
- tiered memory posture where honest
- cross-request prefix-cache reuse under shared batching
- consistent local-versus-cluster cache behavior

Right now the capability/evidence layer is ahead of the fully productized
runtime behavior.

## 7. Adapter support is still intentionally missing

Psionic is explicit today that adapter-bearing Ollama manifests are refused.

That was the correct short-term decision. It is still a missing feature for a
full engine.

If OpenAgents wants a complete Rust-native serving runtime, it eventually needs:

- adapter import
- adapter identity
- adapter merge/unmerge
- hosted adapter serving
- truthful compatibility and memory-policy reporting

This is one of the clearest places where `cloud-lora` remains reference
material, not something we have rebuilt yet.

## 8. The productized API and operator surface is behind the runtime substrate

The underlying crates already know a lot about:

- loaded models
- observability
- receipts
- capability envelopes
- backend health
- residency

But the operator-facing serving product is still narrower than the substrate.

That means a full engine still needs:

- one generic multi-model server
- operator-facing route and runtime inventory
- clearer admission, warm, unload, and scheduling control
- cluster-aware serving control that matches the runtime truth

## 9. Validation is strong in places, but still not a full engine acceptance program

Psionic already has:

- parity contracts
- benchmark gates
- explicit capability truth
- seed/replay support for supported sampling

But a full engine still needs a broader acceptance program:

- backend-by-backend completion criteria
- one non-GPT-OSS family proven end to end
- local and clustered scheduler parity tests
- fault injection on cancellation, restart, and cache invalidation
- API-surface conformance beyond today's narrow request schema

## What We Should Take From Prime Specifically

The right Prime lesson is not "vendor `prime-vllm`."

The right Prime lesson is:

- keep stage/shard/router/cache truth explicit
- separate the control plane from the engine plane
- make distributed serving first-class, not an afterthought
- keep benchmark and topology evidence attached to runtime behavior

The missing work now is mostly not "learn more Prime." It is:

- finish the general engine core
- finish the serving scheduler
- finish backend completion
- finish the generic API and model matrix

## GitHub Issue Program

The cleanest way to finish the full Rust-native inference engine from the
current state is to treat it as one explicit issue set.

Some of these map directly onto already-open roadmap items, especially the
Apple Metal queue. Those existing issues should be treated as part of this
program, not separate from it.

The current model-specific performance umbrellas such as `#3345` and `#3360`
still matter, but they are not the same thing as full engine completion. They
improve one heavy workload on one backend. The issue set below is the broader
program required to finish the general Rust-native inference runtime.

### Core Engine Completion

1. **Psionic Inference: define the full-engine completion contract and backend claim matrix**
   Define exactly what "full Rust-native inference engine" means in Psionic terms: supported model families, API surfaces, batching posture, backend claims, cluster claims, and the minimum validation needed before any backend or family is advertised as complete.

2. **Psionic Models: promote GGUF decoder-family support from metadata into real executed Llama, Qwen, and Mistral runtimes**
   Turn the current broader GGUF loader coverage into real decoder execution support so the engine is not effectively GPT-OSS-only for serious model serving.

3. **Psionic Serve: build one generic multi-family decoder execution path over the shared runtime**
   Stop keeping the strongest real decoder path inside a GPT-OSS-specialized serving lane and move toward one reusable decoder-serving substrate with family-specific kernels and layouts behind it.

4. **Psionic Serve: ship a generic Psionic-native server for models, chat, responses, and embeddings**
   Replace the current narrow GPT-OSS server shape with one generic server that can front the real Psionic runtime across text generation, embeddings, and model inventory without being model-family-specific.

5. **Psionic Serve: expose the real CPU GGUF decoder lane through the shipped server and validation matrix**
   Close the gap where a CPU GGUF GPT-OSS service exists in the codebase but the shipped GPT-OSS OpenAI server still treats CPU as unimplemented.

6. **Psionic Serve: add multi-model load, warm, unload, and route selection under one runtime**
   Move beyond one loaded worker per process and make model inventory, residency, warm state, and explicit routing first-class operator concepts.

7. **Psionic Runtime: add shared static batching and continuous batching for text generation**
   Upgrade the current local text-generation posture from `single_request_only` into a real scheduler that can batch compatible requests truthfully.

8. **Psionic Runtime: add mixed prefill/decode queueing, fairness, cancellation, and backpressure semantics**
   Define how the runtime behaves once multiple live requests share the engine, including queue discipline, starvation control, abort behavior, and observability.

9. **Psionic Runtime: productize paged-KV residency, eviction, spill, and tiered-memory policy for served decode**
   Move from policy schema and accounting truth into a fully productized residency system that can support long-lived shared serving under pressure.

10. **Psionic Serve: productize prefix-cache reuse and invalidation across local and clustered serving**
    Make shared prefix reuse a first-class serving optimization with explicit compatibility and invalidation behavior under local, replica, pipeline, and sharded lanes.

11. **Psionic Metal: retire proxy ambiguity and finish the native Apple decode path**
    Absorb the existing open Metal queue `#3270`, `#3268`, `#3269`, `#3271`, `#3272`, `#3261`, and `#3262` into one completion track whose end state is a fully native, fully benchmarked, no-proxy-needed Metal decoder lane.

12. **Psionic CUDA: separate the reusable CUDA decoder substrate from the current GPT-OSS-specific success path**
    Keep the GPT-OSS wins, but extract the reusable runtime, kernel, and scheduler pieces so future families are not blocked on one hand-tuned path.

13. **Psionic AMD: either ship a truthful served decoder path or keep AMD explicitly non-claimable**
    Resolve the current half-state by either landing real served execution or making the non-support boundary permanent and explicit.

14. **Psionic API: add logprobs, top-logprobs, and token-probability outputs**
    Close the gap between the current minimal request schema and the richer probability surfaces expected from a modern inference server.

15. **Psionic API: add constrained decoding, structured outputs, and tool-calling**
    Make structured generation a first-class runtime capability instead of leaving the server limited to plain chat-completions text output.

16. **Psionic Adapters: add LoRA and adapter import, merge/unmerge, and hosted serving**
    Replace the current explicit refusal posture with truthful Rust-native adapter handling and serving policy.

17. **Model IO: add Rust-native checkpoint, tokenizer, template, and runtime-format interoperability**
    Ensure the engine can ingest and emit real-world model artifacts without trapping Psionic in one narrow artifact path.

18. **Psionic Cluster: unify local and clustered serving scheduler semantics**
    Close the remaining gap between what the local runtime can do and what the clustered runtime claims, especially around cache truth, warm routing, shard handoff, and operator observability.

### Production Completion

19. **Psionic Reliability: ship subprocess runtime mode, restart recovery, and serving fault injection**
    The isolation contract exists today, but a full engine should also have a productized crash boundary, restart story, and tests for cancellation, cache corruption, and runtime restarts.

20. **Psionic Reproducibility: add deterministic replay and receipt-level decode reproducibility guarantees**
    Extend the current seeded sampling and identity work into stronger cross-backend replay, request reproduction, and evidence guarantees.

21. **Psionic Benchmarks: define backend-by-backend acceptance thresholds and regression gates**
    Replace ad hoc performance proof with a stable completion matrix for latency, throughput, warmup, prompt-cache-hit, recovery, and memory behavior per backend and model class.

22. **Inference Pilot: prove one non-GPT-OSS family end to end through the generic Psionic server**
    Run a real completion pilot on at least one additional family such as Llama, Qwen, or Mistral so "full engine" no longer really means "GPT-OSS plus plans."

## Recommended Reading Of Current State

If we compress the whole repo state into one sentence, it is this:

> Psionic has already rebuilt enough of the engine and distributed-serving core
> to justify continuing in Rust, but it still needs one more explicit program
> to become a genuinely full inference engine instead of a strong GPT-OSS-first
> inference substrate.

That is a good place to be.

It is much better than "we should import Prime."

But it still means the next work is not vague research. It is a concrete engine
completion program.
