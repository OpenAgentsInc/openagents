# Khala Disaggregation And Dynamo Study

Date: 2026-06-23

Issue: OpenAgentsInc/openagents#6092

Status: P2 study. No production code, dependency adoption, or routing behavior
change.

## Recommendation

Do not add prefill/decode disaggregation to the Khala MVP path yet.

Disaggregation becomes worth a design spike only after Khala has measured,
high-volume, large-model traffic where long-context prefill dominates request
time after prefix caching has already been applied. Current Khala evidence does
not prove that trigger. The right next action is to keep collecting the telemetry
needed to recognize that trigger and use NVIDIA Dynamo as an architecture
reference, not as a dependency.

This keeps the serving stack honest: prefix caching, streaming/async split,
executed verification, provider/engine benchmarking, and Pylon proven-engine
evidence all have stronger near-term signal than adding a new distributed
serving topology before traffic justifies it.

## Trigger Threshold

Open a production design spike for Khala disaggregation only when a rolling
seven-day production window shows all of the following for one Khala route or
model class:

| Gate | Threshold |
| --- | --- |
| Traffic volume | At least 10,000 completed large-model requests for the same route or model class. |
| Long-context shape | P90 post-cache prompt length is at least 32,000 input tokens, or P75 is at least 16,000 input tokens. |
| Prefill dominance | P90 measured prefill or TTFT contribution is at least 60 percent of total wall-clock time after cache hits are accounted for. |
| Decode pressure is not the only bottleneck | P90 inter-token latency / decode throughput remains acceptable enough that optimizing only decode would not explain the slow end-to-end path. |
| Cache already used | Prefix-cache hit rate is at least 50 percent for the stable prefix, so the remaining prefill cost is not simply a prompt-layout bug. |
| Queue signal | Prefill queue wait or prefill-worker saturation is measured, not inferred from total latency. |
| Product relevance | The affected requests include accepted-outcome or verification-relevant workloads, not only exploratory traffic. |

If these gates are not met, disaggregation stays out of the critical path.

The numeric thresholds are intentionally conservative. They are not a product
promise. They are the minimum evidence needed before spending design and
implementation effort on a topology whose benefits appear only at meaningful
scale.

## Metrics Required First

The P0 telemetry schema already gives Khala a public-safe place to record most
request lifecycle facts, but disaggregation needs a few specific measurements to
be decision-grade:

- post-cache input tokens;
- cacheable-prefix tokens and cache-hit tokens;
- TTFT split into prefill, routing, provider, and gateway overhead where the
  provider or worker exposes it;
- prefill queue wait and queue depth;
- decode KV-cache pressure, including resident KV bytes, eviction/offload count,
  and offload latency;
- active prefill workers and active decode workers by lane;
- generated tokens, inter-token latency, and perceived TPS;
- request class: interactive stream, async job, verifier run, or batch;
- route, provider, served model, region, fallback reason, and cache-affinity hash;
- executed verifier result, scalar reward, and cost-per-accepted-outcome.

Until these fields are measured, a disaggregation report would mostly restate
beliefs about long context. That is too weak for a production topology decision.

## Conditional Design

If the trigger threshold is met, the first design should stay conditional and
route-scoped.

1. Classify candidate requests by post-cache input length, request class, route,
   and model family.
2. Keep normal monolithic serving as the default lane.
3. Route only the long-context class to a disaggregated lane when prefill queue
   pressure is high and decode KV-cache pressure has capacity.
4. Emit a receipt-mode field that says whether a request used monolithic,
   disaggregated-prefill, or fallback service.
5. Compare against the monolithic lane on accepted outcome per sat, TTFT,
   total wall-clock, queue wait, and verification rate.

The first successful slice should be a shadow or canary lane, not a broad routing
change. A request that cannot produce a clear receipt for its split topology
must fall back to monolithic service or return an honest blocker.

## Dynamo Decision

NVIDIA Dynamo is useful to study for architecture patterns:

- KV-block management;
- KV-aware routing;
- separating prefill and decode workers;
- scheduler pressure signals;
- routing around KV-cache locality and offload cost.

Khala should not adopt Dynamo as a dependency now. The current product path
needs stable request telemetry and proof-bearing receipts more than a new
orchestration runtime. Dynamo remains a reference until measured Khala traffic
hits the trigger threshold above and a design spike proves it is a better fit
than smaller additions to the existing gateway, provider adapters, Pylon
capability receipts, or Psionic serving seams.

## Relationship To Existing Work

- Prefix caching (#6084) should reduce avoidable prefill before disaggregation is
  considered.
- The streaming/async split (#6086) keeps long work out of synchronous edge
  deadlines and gives detached work a receipt path.
- The telemetry scorecard (#6083) is the schema home for the fields that would
  prove the trigger.
- Provider/engine benchmark work (#6088) and Pylon proven-engine work (#6089)
  should establish realistic sequence shapes before infrastructure is added.
- Psionic shard-WAN remains a deliberate research and execution boundary for
  large-model distributed serving. This study does not promote that path to
  product routing without the same measured evidence and receipt gates.

## Closeout

#6092 is resolved by this study because it records:

- the measured traffic/context threshold for reopening disaggregation;
- the prefill-queue and KV-pressure metrics required first;
- a reference-only Dynamo decision;
- a clear "not yet" recommendation for Khala MVP production routing.

No production code is required to close this issue.
