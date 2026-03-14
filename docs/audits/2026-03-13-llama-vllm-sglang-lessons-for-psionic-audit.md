# 2026-03-13 llama.cpp, vLLM, and SGLang Lessons For Psionic

> Historical note: this is a point-in-time architecture audit written on
> 2026-03-13. Current product authority still lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, and the retained MVP implementation scope. This document
> is a longer-horizon Psionic inference audit, not a statement that all of this
> should be pulled into the current MVP immediately.

## Intent

This audit answers a narrower follow-up question:

> if OpenAgents already learned a lot from `llama.cpp`, what should Psionic now
> learn separately from `llama.cpp`, `vLLM`, and `SGLang`?

The wrong framing is:

> which one of the three should we copy?

The right framing is:

> these three sit at different layers of the serving stack, so what should
> Psionic borrow from each one?

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-13-rust-inference-engine-gap-audit.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `crates/psionic/psionic-serve/src/openai_http.rs`
- `/Users/christopherdavid/code/llama.cpp/README.md`
- `/Users/christopherdavid/code/llama.cpp/grammars/README.md`
- `/Users/christopherdavid/code/vllm/README.md`
- `/Users/christopherdavid/code/vllm/docs/features/structured_outputs.md`
- `/Users/christopherdavid/code/vllm/docs/features/tool_calling.md`
- `/Users/christopherdavid/code/vllm/docs/features/disagg_prefill.md`
- `/Users/christopherdavid/code/vllm/docs/features/automatic_prefix_caching.md`
- `/Users/christopherdavid/code/vllm/docs/usage/security.md`
- `/Users/christopherdavid/code/sglang/README.md`
- `/Users/christopherdavid/code/sglang/sgl-model-gateway/README.md`
- `/Users/christopherdavid/code/sglang/docs/advanced_features/hicache_design.md`
- `/Users/christopherdavid/code/sglang/docs/advanced_features/pd_disaggregation.md`
- `/Users/christopherdavid/code/sglang/docs/advanced_features/sgl_model_gateway.md`

## Executive Summary

The three projects are not three interchangeable versions of the same thing.

They are best understood as:

- `llama.cpp`
  - the best reference for portable local inference, GGUF pragmatism,
    quantization breadth, hybrid CPU/GPU execution, and small-footprint serving
- `vLLM`
  - the best reference for the core serving engine: continuous batching,
    paged-KV/block memory management, prefix caching, distributed serving
    topology, and generic high-throughput API serving
- `SGLang`
  - the best reference for the higher-level serving runtime: cache-aware agent
    workloads, structured outputs, reasoning/tool parsers, multi-turn response
    state, router-level privacy boundaries, MCP loops, and prefill/decode-aware
    control planes

So the right answer for Psionic is not:

- replace `llama.cpp` with `SGLang`
- or replace `SGLang` with `vLLM`

It is:

- keep `llama.cpp` as the local engine, format, and backend-discipline oracle
- take `vLLM` as the primary reference for core scheduler and KV-cache design
- take `SGLang` as the primary reference for agent-serving semantics, cache-
  aware routing, and structured output/tool infrastructure

That split is much more useful than trying to pick one winner.

## The Real Difference Between The Three

## 1. llama.cpp is a local systems engine first

The important `llama.cpp` pattern is not "it has a server."

The important pattern is:

- one portable engine
- many backends
- aggressive quantization support
- one honest model format boundary
- one deployable local binary story

Its strongest lessons for Psionic are:

- GGUF pragmatism
- hybrid CPU/GPU offload for oversized models
- portable backend discipline across CPU, Metal, CUDA, Vulkan, HIP, and SYCL
- grammar-constrained and JSON-schema-constrained output as a local primitive
- embeddings, reranking, and lightweight OpenAI-compatible serving in the same
  local runtime

The main thing `llama.cpp` is not is a modern multi-tenant cloud scheduler.
It does support parallel decoding and a server, but its center of gravity is
still local execution and low-level backend performance.

## 2. vLLM is a serving engine first

The important `vLLM` pattern is:

- high-throughput generic serving
- continuous batching
- explicit KV memory management through PagedAttention
- automatic prefix caching
- tensor/pipeline/data/expert parallelism
- OpenAI-compatible serving as a product surface

`vLLM` is the clearest reference for the middle of the serving stack:

- how requests are admitted
- how decode and prefill interact
- how KV memory is block-managed
- how distributed serving topology is expressed
- how one generic server supports many model families

Its structured outputs, tool calling, and even tool-server hooks through the
Responses API matter, but those still feel like features added onto a serving
engine.

That is useful for Psionic because Psionic's biggest remaining engine gap is
still in the scheduler/KV/distributed-serving layer.

## 3. SGLang is a serving runtime first

The important `SGLang` pattern is:

- cache-aware runtime semantics
- structured generation and parser infrastructure
- prefill/decode disaggregation as a first-class deployment shape
- router-level history, responses, and privacy boundaries
- MCP-aware tool loops
- model-aware routing policies

In the local repo, the split is especially visible:

- `SGLang` root runtime emphasizes RadixAttention, zero-overhead CPU
  scheduling, PD disaggregation, continuous batching, paged attention,
  quantization, multi-LoRA, and broad distributed topologies
- `sgl-model-gateway` adds a dedicated control/data plane with multi-model
  routing, reasoning parsers, tool-call parsers, `/v1/responses`,
  `/v1/conversations`, MCP, rate limiting, retries, circuit breakers, history
  storage, and cache-aware policies

That makes `SGLang` the most relevant reference for OpenAgents-style agent and
tool workloads.

It is the clearest example of:

- how an inference engine grows into an agent-serving runtime
- how structured outputs stop being a side feature and become a core runtime
  concern
- how response history and tool loops move into the router boundary

## What Blog-Post Comparisons Usually Miss

The simplistic internet version is:

- `SGLang` = agentic and structured
- `vLLM` = throughput
- `llama.cpp` = local

That is directionally true, but it hides the more useful truth:

- `vLLM` now also has structured outputs, tool calling, prefix caching, and
  disaggregated prefill
- `SGLang` now also has continuous batching, paged attention, TP/PP/EP/DP,
  multi-LoRA, and serious distributed deployment machinery
- `llama.cpp` now also has OpenAI-compatible serving, parallel decoding,
  embeddings, reranking, grammars, and JSON-schema-to-grammar conversion

So the difference is no longer raw feature presence.

The difference is emphasis:

- `llama.cpp` emphasizes portability and local systems truth
- `vLLM` emphasizes serving-engine throughput and memory management
- `SGLang` emphasizes serving-runtime semantics for complex multi-turn and tool
  workloads

## What Psionic Should Take From Each

## From llama.cpp

Psionic should continue taking:

- GGUF ingestion discipline and model-family pragmatism
- quantization breadth as a first-class product feature
- backend-specific kernel discipline and performance honesty
- hybrid residency and offload truth for models larger than one device budget
- lightweight local serving surfaces
- grammar and JSON-schema constrained generation as local primitives

Psionic should not copy:

- the exact `ggml` execution model
- the exact C/C++ architecture
- the assumption that the local single-node server is the final serving shape

## From vLLM

Psionic should take:

- continuous batching and mixed prefill/decode scheduler design
- paged/block KV manager design
- automatic prefix caching under one generic runtime
- disaggregated prefill/decode transfer seams
- distributed serving topology for TP/PP/DP/EP
- one generic server that fronts many model families and APIs

Psionic should not copy:

- the Python/PyTorch-centered execution stack
- the exact connector and deployment ecosystem
- the habit of treating higher-level agent semantics as mostly an add-on

## From SGLang

Psionic should take:

- RadixAttention-style cache-aware request semantics
- HiCache-style tiered KV story across device, host, and distributed storage
- router-level cache-aware and policy-aware request placement
- structured outputs as a first-class runtime capability
- reasoning-parser and tool-parser infrastructure
- `/v1/responses` and conversation-oriented multi-turn runtime semantics
- MCP-aware tool execution loops and router-local privacy boundaries
- multi-model routing, retries, queuing, and circuit-breaker operator posture

Psionic should not copy:

- the Python serving core
- exact model-specific parser inventories
- the exact router product shape when OpenAgents needs a more explicit kernel /
  Psionic split

## The Best Split For OpenAgents

If we map these directly onto Psionic and OpenAgents, the clean owner split is:

- `llama.cpp`
  - reference for low-level engine behavior, artifact compatibility, local
    serving truth, and backend performance oracles
- `vLLM`
  - reference for the Psionic runtime and scheduler layer
- `SGLang`
  - reference for the Psionic serve/router layer and the eventual agent-serving
    control plane around it

That is the most useful architecture rule to keep in mind:

> use `llama.cpp` to shape the bottom of the stack, `vLLM` to shape the middle,
> and `SGLang` to shape the top.

## What This Means For Current Psionic Gaps

The current Psionic gap audit already says the biggest missing pieces are:

- broader real executed model-family support
- continuous batching and scheduler closure
- broader HTTP/API surface
- adapter support
- stronger cache and routing semantics

This three-project comparison sharpens that:

- the scheduler and KV story is mostly a `vLLM` lesson
- the structured-output, tool, response, and router story is mostly an
  `SGLang` lesson
- the model-format, quantization, and local portability story remains mostly a
  `llama.cpp` lesson

## GitHub Issue Program

The cleanest issue program is not "pick one engine and port it."

It is one Psionic program that draws from the right reference at the right
layer.

### Core Engine And Runtime

1. **Psionic Architecture: codify the three-source reference split for inference**
   Document and freeze the intended split: `llama.cpp` for local/runtime truth, `vLLM` for scheduler/KV/distributed serving, and `SGLang` for structured runtime and routing semantics.

2. **Psionic Models: finish GGUF, tokenizer, quantization, and artifact-tooling parity for the intended decoder families**
   Push the `llama.cpp` lesson further so model ingestion, quantization truth, and artifact packaging stay first-class instead of becoming an afterthought behind one successful model family.

3. **Psionic Runtime: add continuous batching and mixed prefill/decode scheduling**
   Rebuild the strongest `vLLM` and `SGLang` scheduler lesson in Rust so local and clustered serving stop being effectively `single_request_only`.

4. **Psionic Runtime: add a real paged/block KV manager and request-owned KV accounting**
   Make KV memory a real runtime subsystem rather than just capability and evidence truth.

5. **Psionic Runtime: add automatic prefix caching with explicit tenancy and invalidation boundaries**
   Borrow the strongest `vLLM` APC lesson while keeping OpenAgents-specific cache truth and served-artifact identity intact.

6. **Psionic Runtime: add disaggregated prefill/decode execution and KV transfer seams**
   Rebuild the useful `vLLM` and `SGLang` PD idea so TTFT and ITL can be tuned separately where that is actually beneficial.

7. **Psionic Runtime: add tiered KV residency and a HiCache-class device/host/distributed cache design**
   Take the core `SGLang` HiCache lesson and turn Psionic's current cache schemas into a true hierarchical runtime.

8. **Psionic Cluster: add scheduler-grade TP/PP/DP/EP planning under one truthful serve topology**
   Use `vLLM` and `SGLang` as references for how serving topology should be exposed, planned, and benchmarked.

### Serve And Router Layer

9. **Psionic Serve: ship a generic multi-family server instead of a GPT-OSS-specific OpenAI lane**
   Apply the strongest `vLLM` lesson: one generic serving plane must front many model families and request types.

10. **Psionic Serve: add `/v1/embeddings`, `/v1/responses`, and broader multi-endpoint product coverage**
    Bring Psionic closer to the richer serving surfaces that both `vLLM` and `SGLang` now expose.

11. **Psionic Serve: add structured outputs for choice, regex, JSON schema, grammar, and tagged structure**
    Treat constrained generation as a first-class runtime capability, drawing from `vLLM` and `SGLang`, while preserving `llama.cpp`-style local grammar fallback.

12. **Psionic Serve: add reasoning-parser and tool-parser layers for served model families**
    Rebuild the `SGLang` parser lesson so tool use and reasoning are handled as explicit runtime outputs rather than ad hoc prompt conventions.

13. **Psionic Serve: add named, auto, required, and none tool-calling modes with truthful parser contracts**
    Borrow the best current `vLLM` tool-calling contract shape while keeping the higher-level runtime parser seam Psionic-owned.

14. **Psionic Serve: add conversation and response-state contracts for multi-turn agent loops**
    Rebuild the strongest `SGLang` router lesson: agentic `/v1/responses` style flows need explicit state ownership, not just repeated chat-completions calls.

15. **Psionic Router: add cache-aware, policy-aware, and power-of-two load balancing for multi-model fleets**
    Use `SGLang` gateway and `vLLM` serving lessons to define how OpenAgents should route across warm models, PD nodes, and model-specific policies.

16. **Psionic Router: add reliability primitives for retries, rate limiting, queues, circuit breakers, and worker health**
    Adopt the operator posture that `SGLang` gateway makes explicit instead of leaving those concerns outside the serving plane.

17. **Psionic Router: add MCP-aware tool loops and router-local privacy boundaries**
    This is the most specifically `SGLang` lesson for OpenAgents: tools, histories, and multi-model loops need a control boundary that is not the model worker itself.

### Local And Portable Execution

18. **Psionic Local: keep the portable local lane first-class across CPU, Metal, CUDA, and hybrid offload**
    Continue the `llama.cpp` lesson that local execution, weird hardware, and partial offload matter enough to shape the architecture.

19. **Psionic Local: add grammar and JSON-schema constrained decoding as universal local fallback capabilities**
    Make sure structured generation survives even on the smallest, most local serving path.

20. **Psionic Adapters: add LoRA and adapter batching, routing, and hosted serving**
    This is where `SGLang` multi-LoRA and `llama.cpp` adapter ecosystem lessons both matter.

### Validation And Completion

21. **Psionic Benchmarks: define three separate acceptance matrices for local, high-throughput serving, and agentic structured workloads**
    Stop mixing all benchmark claims together. `llama.cpp`-style local truth, `vLLM`-style serving throughput, and `SGLang`-style agent runtime behavior are different completion targets.

22. **Psionic Pilot: validate one end-to-end agent workload with structured outputs, tool calls, response state, and cache reuse**
    The final proof should not just be raw text generation. It should demonstrate the exact workload class that makes `SGLang` relevant to OpenAgents in the first place.

## Bottom Line

If Psionic only keeps learning from `llama.cpp`, it will become a better local
engine but still underinvest in the serving scheduler and agent runtime layers.

If Psionic only learns from `vLLM`, it will improve the serving engine but may
still treat structured outputs, tools, and response-stateful agent loops as
secondary concerns.

If Psionic only learns from `SGLang`, it risks skipping the harder engine-core
discipline around generic scheduler and KV memory design.

The right move is the layered one:

- `llama.cpp` for the bottom
- `vLLM` for the middle
- `SGLang` for the top

That is the most honest way to finish Psionic into a full Rust-native inference
and agent-serving stack.
