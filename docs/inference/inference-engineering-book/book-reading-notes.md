# Inference Engineering - Reading Notes For Khala

Status: notes from `~/Downloads/Inference Engineering.pdf`, cross-read against
the local `docs/inference/` corpus. These are paraphrased notes, not copied book
text.

## Overall Frame

The book splits inference engineering into three layers:

- **Runtime:** make one model deployment fast and efficient.
- **Infrastructure:** scale, route, queue, and fail over across replicas,
  regions, and providers.
- **Tooling:** give operators enough abstraction to ship safely without hiding
  the controls needed for production.

Khala already spans all three. The gateway, provider adapters, model catalog,
MeteringHook, receipts, batch-job rails, Verse projection, Pylon supply plan,
and M8 evidence manifests are the beginnings of an inference platform. The
gap is not a missing slogan. The gap is measured production behavior across the
whole request lifecycle.

## Chapter 1 - Prerequisites

The useful prerequisite is a concrete definition of "best" for the product.
For Khala, "best" cannot be just cheapest tokens or fastest streaming. The
Khala docs already name cost per accepted outcome as the north-star, and the
book supports that framing: latency, throughput, unit economics, and quality
are tradeoffs that must be specified per application.

Relevant Khala implications:

- Every Khala lane should declare its latency budget and optimization target:
  chat, coding artifact generation, batch embeddings, and future voice/video
  should not share one performance goal.
- Evals come before optimization. The M8 executed crossy-road runner is the
  right shape because it measures behavior, not source patterns.
- Model selection needs product-specific evals. General benchmark scores are
  only a shortlist mechanism for `openagents/khala-*`.
- TTFT, inter-token latency, perceived TPS, total TPS, total response time, and
  P50/P90/P99 should be tracked separately. For agentic or tool calls, total
  response time and accepted outcome matter more than token streaming speed.

## Chapter 2 - Models And Bottlenecks

The book's most useful model-level distinction is:

- **Prefill** constructs the KV cache and largely determines time to first
  token. It is generally compute-bound.
- **Decode** generates later tokens and largely determines per-user token rate.
  It is generally memory-bandwidth-bound.

Relevant Khala implications:

- Long coding prompts are prefill-heavy. Khala-code should preserve telemetry
  for input sequence length and cache hits, not only total tokens.
- Faster decode work will not fix slow TTFT on huge codebase contexts.
- Prefix caching, chunked prefill, and cache-aware routing are probably higher
  leverage for Autopilot/Khala than low-level kernel work in the near term.
- Any Pylon fabric claim should identify whether it improves TTFT, ITL/TPS,
  total throughput, cost, or verification rate. "Faster" is not enough.

## Chapter 3 - Hardware

The hardware chapter mostly reinforces why Khala should avoid pretending every
GPU is fungible. GPU memory bandwidth, GPU memory size, interconnect, CPU-GPU
bandwidth, and cold-start weight loading all change the viable deployment.

Relevant Khala implications:

- The Pylon capability heartbeat should eventually describe usable memory,
  bandwidth class, interconnect posture, engine support, model residency, and
  cold-start posture, not just "GPU online."
- Small models may fit better on fractional modern GPUs or cheaper managed
  lanes than on full high-end cards. This supports starting Pylon with whole
  small models before shard-WAN.
- Large-model sharding depends on topology. The shard-WAN docs already capture
  this through topology receipts; the book reinforces that interconnect is the
  limiting variable.

## Chapter 4 - Software

The book treats vLLM, SGLang, and TensorRT-LLM as the main production engines.
The practical reading for OpenAgents is to use proven engines for model serving
unless Psionic is explicitly owning a lower-level execution research boundary.

Relevant Khala implications:

- Use vLLM for broad model support and quick serving experiments.
- Look at SGLang for Kimi/Qwen/DeepSeek/MoE heavy lanes and for code-oriented
  speculative decoding experiments.
- Look at TensorRT-LLM for NVIDIA-only, high-performance, carefully configured
  lanes when the model and hardware are stable enough to justify the work.
- Treat NVIDIA Dynamo as an orchestration pattern to study for KV-aware
  routing and prefill/decode disaggregation, not a dependency to add before
  traffic justifies it.
- Benchmark with realistic Khala traffic: real input/output lengths, streaming
  settings, temperature/reasoning settings, code prompts, verifier runs, and
  cacheable session prefixes.

## Chapter 5 - Techniques

### Quantization

Quantization can improve both prefill and decode but can also damage output
quality. The book's conservative production advice maps directly to Khala:
quantization should be proven against product evals, not assumed safe from a
throughput number.

Khala relevance:

- Start with lower-risk weights and activation quantization before aggressive
  KV cache or attention quantization.
- Use the executed Khala evals and acceptance receipts to compare original and
  quantized outputs.
- For Pylon lanes, publish the quantization mode in the receipt. A model
  served as FP8/MXFP8/NVFP4 is not the same product as an unqualified model id.

### Speculative Decoding

The book's speculation chapter matches the local speculative-decoding primer
and the shard-WAN docs. Speculation mainly improves decode speed, especially at
low batch sizes where spare compute exists. It is not a universal win.

Khala relevance:

- Code tasks are a strong fit for n-gram or lookahead speculation because
  generated code often repeats syntax and prompt context.
- EAGLE is interesting for future learned/Psionic lanes but needs target-model
  hidden-state data and training work.
- Speculation should be dynamically disabled when batches are large enough that
  the extra verification work hurts throughput.
- For shard-WAN, speculation is more than a local speed trick: it is a WAN
  latency-hiding strategy and belongs in the Psionic receipt mode.

### Caching

Prefix caching is one of the highest-leverage near-term items for Khala. The
book's key operational rule is that novel tokens should appear as late as
possible in the prompt so the shared prefix remains cacheable.

Khala relevance:

- Keep long stable instructions, tool schemas, acceptance contracts, and
  provider/system policy blocks before user-specific tokens.
- Deterministically order tool schemas and acceptance-contract text.
- Pass Fireworks session affinity or OpenAI `user` where appropriate.
- Record cached input tokens from provider usage/headers when available.
- Route session/codebase/account follow-up turns to the same cache-warm lane
  unless health/cost/privacy constraints override it.

### Parallelism And Disaggregation

Tensor parallelism is usually best within one node for latency; expert
parallelism is useful for throughput on MoE models; pipeline parallelism is
mostly for multi-node settings. Disaggregation is powerful but should wait for
large traffic, large models, and prefill-heavy workloads.

Khala relevance:

- The Psionic shard-WAN plan is a deliberate exception to normal pipeline
  parallelism advice because it combines topology selection, direct return,
  speculative decode, and receipts to hide WAN latency.
- Do not put disaggregation on the critical path for Khala MVP. Watch for the
  workload trigger: high-volume, large-model, long-context coding traffic.
- If/when disaggregation arrives, track prefill queue size and decode KV cache
  pressure explicitly.

## Chapter 6 - Modalities

The modality chapter matters because `docs/inference/` already frames Khala as
one primitive inside a broader Agent Cloud. Voice, embeddings, image, and video
have different latency and batching shapes.

Relevant Khala implications:

- Embeddings and bulk document processing should prefer async/batch economics,
  not interactive chat semantics.
- Voice and live UIs need bidirectional streaming or WebSockets, not plain
  request/response.
- Image/video lanes are compute-bound and should be scaled independently from
  LLM decode-heavy lanes.
- Future Cloud primitives should each define their own metric contract instead
  of inheriting Khala chat metrics.

## Chapter 7 - Production

The production chapter is the most immediately relevant part of the book.

Key points for Khala:

- Containers and engine versions must be pinned. Pylon worker auto-updates
  should remain signed/pinned and operator-consented.
- Autoscaling should combine traffic and utilization signals. Token counts,
  sequence lengths, and cache hit rates explain load better than request count
  alone.
- Continuous batching trades latency for throughput. Khala needs different
  batch/concurrency targets for chat, code generation, verifier runs, and batch
  jobs.
- Cold starts include GPU allocation, image load, model weight load, and engine
  startup/compile. Pylon receipts should eventually say whether a request was
  warm or cold.
- Routing needs request-level data: sequence length, prefix/cache key, LoRA or
  adapter needs, provider health, queue depth, and account tier.
- Queues are first-class. When capacity is saturated, route to the right queue
  and surface honest status rather than blocking the Cloudflare edge.
- Scale-to-zero is acceptable for dev, batch, and predictable offline jobs. It
  is a bad fit for latency-sensitive unscheduled Khala traffic unless cold
  starts are bounded and queued.
- Multi-cloud management maps to Khala's Vertex, Fireworks, partner, and Pylon
  supply mix. The control plane should make global placement decisions, while
  workload planes continue serving if the control plane is impaired.
- Observability should include request volume, request/response token sizes,
  status codes, TTFT, ITL/TPS, end-to-end latency percentiles, active and
  starting replicas, CPU/GPU utilization, GPU memory, and queue depth.
- Client code matters. The existing 524 postmortem is exactly the book's point:
  interactive calls stream, detached work becomes async jobs, and live UIs use a
  durable connection.

## Overall Relevance To Khala

Khala's most valuable next work is not low-level GPU optimization. It is making
the gateway and receipts production-grade enough that later low-level serving
work can be evaluated honestly:

- measure what matters;
- preserve prompt/cache shape;
- route by typed capability and observed state;
- execute verification;
- stream or queue appropriately;
- only optimize engines once the workload shape and quality gates are clear.
