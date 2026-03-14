# Psionic Product-Class Acceptance Matrices

> Status: canonical `PSI-256` / `#3561` closure doc, updated 2026-03-14 after
> landing the runnable category harness in
> `scripts/release/check-psionic-product-class-acceptance.sh`.

Psionic is trying to become three different things at once:

- a portable local engine
- a high-throughput serving engine
- a structured or agentic serving runtime

Those product classes overlap, but they are not the same claim. One benchmark
headline cannot honestly prove all three.

This document splits Psionic acceptance into three explicit categories, names
the supported envelopes and refusal boundaries for each one, and points at the
canonical runner for each category.

This is a `crates/psionic/*` authority document only. It does not widen current
MVP product scope in `docs/MVP.md`, and it does not move ownership boundaries
out of `docs/OWNERSHIP.md`.

## Canonical Runner

Run the full category split from the repo root:

```bash
scripts/release/check-psionic-product-class-acceptance.sh
```

Targeted entrypoints:

```bash
scripts/release/check-psionic-product-class-acceptance.sh --local-portability-only
scripts/release/check-psionic-product-class-acceptance.sh --throughput-serving-only
scripts/release/check-psionic-product-class-acceptance.sh --structured-agent-only
scripts/release/check-psionic-product-class-acceptance.sh --apple-silicon
scripts/release/check-psionic-product-class-acceptance.sh --linux-nvidia
scripts/release/check-psionic-product-class-acceptance.sh --include-local-throughput-benchmark --local-throughput-json-out /tmp/psionic-local-bench
```

The canonical runner is intentionally host-aware:

- default runs stay host-agnostic and only exercise portable or synthetic
  evidence
- `--apple-silicon` widens the local-portability category into real Metal
  execution and parity checks
- `--linux-nvidia` widens the local-portability category into real CUDA
  execution checks
- `--include-local-throughput-benchmark` widens the throughput category into
  the exact local GPT-OSS benchmark script when the required model and binaries
  exist

## Why This Split Exists

Three green categories mean three different things:

| Product class | What a green result means | What it does not mean |
| --- | --- | --- |
| Local portability | Psionic can execute or explicitly refuse the supported local backend/product envelopes honestly, with machine-checkable validation references and local backend truth | It does not prove sustained serving throughput, clustered topology behavior, or agent-runtime semantics |
| High-throughput serving | Psionic can sustain queueing, cache, routing, and clustered planner behavior within the claimed serving envelopes, with benchmark or topology receipts where those claims matter | It does not prove host portability breadth or structured-agent completion |
| Structured or agentic serving | Psionic can produce machine-checkable structured outputs, tool flows, response-state continuation, and refusal behavior for the supported generic server surfaces | It does not prove high tok/s on every backend or that every local hardware lane is shipped |

If a future issue tries to close one of these classes with evidence from another,
that issue is now wrong by definition and should update this document before it
claims completion.

## Matrix 1: Local Portability

### Goal

Prove that Psionic can act as a truthful local execution substrate across the
currently claimed host and backend envelopes, or refuse explicitly when a lane
is outside the shipped support matrix.

### Primary authorities

- `HARDWARE_VALIDATION_MATRIX.md`
- `CONFORMANCE_HARNESS.md`
- local backend truth surfaces in `psionic-serve` and `psionic-provider`

### Green means

- the minimum shipped hardware validation rows are still honest
- local backend truth headers and validation references stay machine-checkable
- refusal-only lanes still refuse explicitly instead of silently downgrading
- the local semantics cutover harness remains usable as a correctness oracle for
  basic list/show/generate/embed behavior

### Supported envelope

- CPU reference embeddings and text generation on ordinary `x86_64` and
  `aarch64` hosts
- Apple Silicon Metal embeddings and dense text generation on supported Apple
  GPU families when `--apple-silicon` is run
- Apple Silicon Metal GPT-OSS validation on supported Apple GPU families when
  `--apple-silicon` is run
- Linux NVIDIA CUDA embeddings validation when `--linux-nvidia` is run
- explicit refusal rows for unavailable Metal or CUDA lanes on unsupported hosts

### Refusal expectations

- off-platform Metal must serialize explicit refusal validation
- unavailable CUDA must serialize explicit refusal validation
- local backend truth must stay explicit even for proxy or degraded control
  surfaces
- no local portability claim may rely on cluster-only topology evidence

### Canonical evidence hooks

- `scripts/release/check-psionic-product-class-acceptance.sh --local-portability-only`
- `scripts/release/check-psionic-product-class-acceptance.sh --local-portability-only --apple-silicon`
- `scripts/release/check-psionic-product-class-acceptance.sh --local-portability-only --linux-nvidia`

## Matrix 2: High-Throughput Serving

### Goal

Prove that Psionic can act as a truthful serving engine under queueing, cache,
and clustered-topology pressure, without pretending that portability or agentic
surface correctness is the same thing as throughput closure.

### Primary authorities

- `TOPOLOGY_ACCEPTANCE_MATRIX.md`
- `CLUSTER_VALIDATION_RUNBOOK.md`
- `crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh`
- `crates/psionic/scripts/benchmark-cluster-gates.sh`

### Green means

- local scheduling, queue posture, and prefix-cache reuse remain explicit
- clustered serving topology claims remain executable and benchmark-gated
- throughput or planner-budget claims emit typed receipts instead of ad hoc
  timing notes
- local exact GPT-OSS throughput claims, when published, are tied to the
  dedicated benchmark script rather than to unrelated structured or portability
  checks

### Supported envelope

- local continuous-batching scheduler, local prefix-cache reuse, and explicit
  scheduling headers on the generic server
- clustered remote whole-request, replica-routed, pipeline-sharded,
  layer-sharded, and tensor-sharded planner behavior under the current
  `PSI-255` matrix
- cluster planner benchmark receipts for the current synthetic CUDA topology
  classes
- exact local GPT-OSS benchmark evidence only when
  `--include-local-throughput-benchmark` is run on a host with the required
  model and comparison binaries

### Refusal expectations

- cluster planner or topology claims must refuse or downgrade explicitly when
  topology, cache, or backend constraints are violated
- Apple proxy-mode GPT-OSS benchmarks are not portability or shipped native
  throughput claims
- one local benchmark result must not be used to close cluster topology truth
- one cluster planner benchmark must not be used to close structured-agent
  serving claims

### Canonical evidence hooks

- `scripts/release/check-psionic-product-class-acceptance.sh --throughput-serving-only`
- `scripts/release/check-psionic-product-class-acceptance.sh --throughput-serving-only --include-local-throughput-benchmark --local-throughput-json-out /tmp/psionic-local-bench`

## Matrix 3: Structured Or Agentic Serving

### Goal

Prove that Psionic can act as a structured-serving and agent-runtime gateway
with explicit, machine-checkable semantics for structured outputs, tools,
response-state continuation, route visibility, and refusal behavior.

### Primary authorities

- `LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
- `CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- generic server and router tests in `psionic-serve` and `psionic-router`

### Green means

- structured outputs remain explicit and machine-checkable
- tool-calling modes and tool-call validation remain explicit
- response-state continuation and restart-safe local persistence remain explicit
- router-owned multi-step tool-loop boundaries stay outside app-local glue
- refusal behavior for unsupported JSON-schema features, invalid tool arguments,
  unsupported reasoning requests, and bad response-state references stays green

### Supported envelope

- generic server structured outputs for grammar, JSON schema subset, and other
  explicitly supported structured contracts
- generic server tool-calling modes and streaming tool-call envelopes
- `/v1/responses` conversation state, restart-safe local persistence, and
  router-owned tool loops
- route headers and router-owned visibility controls on the current served fleet
  substrate

### Refusal expectations

- unsupported JSON-schema features must refuse explicitly
- unsupported reasoning requests for a model family must refuse explicitly
- invalid tool arguments must refuse explicitly
- invalid or unknown response-state references must refuse explicitly
- agent-runtime completion must not be claimed from raw token throughput alone

### Canonical evidence hooks

- `scripts/release/check-psionic-product-class-acceptance.sh --structured-agent-only`
- `STRUCTURED_AGENT_WEATHER_PILOT.md`

## Claim Discipline Rules

These rules are now part of the acceptance contract:

- do not claim Psionic portability closure from throughput receipts alone
- do not claim Psionic serving completion from the hardware validation matrix
  alone
- do not claim structured-agent completion from local or clustered throughput
  benchmarks alone
- do not claim a new backend lane as shipped until the relevant local
  portability envelope is green
- do not claim a new serving topology as shipped until the throughput-serving
  category includes it explicitly
- do not claim an agent-runtime surface as shipped until the structured-agent
  category includes it explicitly

## Update Rule

If a future issue adds:

- a new host or backend envelope
- a new clustered serving topology
- a new structured or agent-runtime surface

then it must update this document, update the canonical runner, and add or
extend the relevant category evidence before claiming closure.
