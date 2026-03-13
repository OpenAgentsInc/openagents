# 2026-03-13 Prime Relation And Psionic Adaptation Audit

> Historical note: this audit is a point-in-time snapshot from 2026-03-13.
> Current product and architecture authority lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, and the active `crates/psionic/docs/*` plans.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/PROVIDER_INTEGRATION.md`
- `crates/psionic/docs/CONFORMANCE_HARNESS.md`
- `crates/psionic/docs/BACKENDS.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `crates/psionic/psionic-serve/src/openai_http.rs`
- `crates/psionic/psionic-provider/src/lib.rs`
- `crates/psionic/psionic-cluster/src/lib.rs`
- `crates/psionic/psionic-apple-fm/src/lib.rs`
- `~/code/prime/README.md`
- `~/code/prime/pyproject.toml`
- `~/code/prime/packages/prime/README.md`
- `~/code/prime/packages/prime/pyproject.toml`
- `~/code/prime/packages/prime/src/prime_cli/main.py`
- `~/code/prime/packages/prime/src/prime_cli/core/config.py`
- `~/code/prime/packages/prime/src/prime_cli/core/client.py`
- `~/code/prime/packages/prime/src/prime_cli/api/inference.py`
- `~/code/prime/packages/prime/src/prime_cli/commands/inference.py`
- `~/code/prime/packages/prime/src/prime_cli/commands/sandbox.py`
- `~/code/prime/packages/prime-sandboxes/README.md`
- `~/code/prime/packages/prime-sandboxes/src/prime_sandboxes/models.py`
- `~/code/prime/packages/prime-sandboxes/src/prime_sandboxes/sandbox.py`
- `~/code/prime/packages/prime-sandboxes/src/prime_sandboxes/rpc_command_session.py`
- `~/code/prime/packages/prime-evals/README.md`
- `~/code/prime/packages/prime-evals/src/prime_evals/evals.py`
- `~/code/prime/packages/prime-evals/src/prime_evals/core/client.py`
- `~/code/prime/packages/prime-mcp-server/README.md`
- `~/code/prime/packages/prime-mcp-server/src/prime_mcp/mcp.py`
- `~/code/prime/packages/prime-mcp-server/src/prime_mcp/client.py`
- `~/code/prime/packages/prime-mcp-server/src/prime_mcp/core/client.py`
- `~/code/prime/packages/prime-mcp-server/src/prime_mcp/tools/availability.py`
- `~/code/prime/packages/prime-mcp-server/src/prime_mcp/tools/pods.py`
- `~/code/prime/packages/prime-tunnel/README.md`
- `~/code/prime/packages/prime-tunnel/src/prime_tunnel/tunnel.py`
- `~/code/prime/packages/prime-tunnel/src/prime_tunnel/core/client.py`
- `~/code/pi/README.md`
- `~/code/pi/protocol/README.md`
- `~/code/pi/protocol/Cargo.toml`
- `~/code/pi/prime-iroh/README.md`
- `~/code/pi/prime-iroh/Cargo.toml`
- `~/code/pi/pccl/README.md`
- `~/code/pi/prime-diloco/README.md`
- `~/code/pi/prime-rl/README.md`
- `~/code/pi/verifiers/README.md`
- `~/code/pi/prime-vllm/README.md`
- `~/code/pi/prime-pipeline/README.md`
- `~/code/pi/toploc/README.md`
- `~/code/pi/toploc-validator/README.md`
- `~/code/pi/gpu-challenge/README.md`
- `~/code/pi/threadpark/README.md`
- `~/code/pi/threadpool/README.md`
- `~/code/pi/pi-quant/README.md`
- `~/code/pi/datasetstream/README.md`
- `~/code/pi/smart-contracts/README.md`

## Executive Summary

`prime` is not a peer to Psionic in the stack. It is an external hosted control
plane and operator SDK for renting/managing remote compute, remote sandboxes,
inference endpoints, tunnels, and evaluations. Psionic is our Rust-native local
and cluster execution substrate with explicit backend truth, delivery proofs,
validation references, and provider receipts.

So the right relationship is:

- Prime is adjacent infrastructure and operator UX research.
- Psionic is execution truth inside OpenAgents.
- Autopilot/Desktop is the product shell that may optionally talk to hosted
  infrastructure later.

The practical conclusion is:

1. do not pull Prime into the core Psionic runtime path,
2. do not let Prime-hosted execution blur the MVP promise that Autopilot lives
   on your machine and earns from your machine,
3. do adapt several Prime ideas at the contract and operator-surface level,
   especially for future bounded `psionic.sandbox_execution`, evaluation
   artifact handling, and MCP/operator control surfaces,
4. if we ever integrate Prime directly, do it as an explicit optional app-layer
   integration, not as hidden runtime dependency or provider-truth shortcut.

After reading the broader `~/code/pi` mirror, there is a second conclusion that
matters even more than the `prime` SDK itself:

- the PI org has a real native protocol/comms/proof stack,
- some of those repos are much closer to our future Psionic cluster and
  evidence ambitions than `prime` is,
- if the goal is to build much of that stack in Rust, we should treat PI as a
  multi-repo reference corpus, not as a package to integrate.

## What Prime Actually Is

After reading the local `~/code/prime` tree, Prime is best understood as a
small Python monorepo for one hosted platform:

- `prime`
  - Typer CLI and Python SDK for account, pods, environments, sandboxes,
    inference, images, secrets, teams, and deployments.
- `prime-sandboxes`
  - lightweight typed SDK for remote code-execution sandboxes, including file
    transfer, command execution, background jobs, port exposure, and SSH
    sessions.
- `prime-evals`
  - evaluation artifact upload client with batching, retries, environment
    resolution, and finalization.
- `prime-tunnel`
  - tunnel registration plus local `frpc` process supervision for exposing
    local services.
- `prime-mcp-server`
  - a thin MCP wrapper over Prime compute-management APIs.

The important architectural point is that Prime is mostly control plane and
service access, not a local inference runtime. Even its inference surface is a
minimal OpenAI-compatible HTTP client over a hosted endpoint. The real product
center is:

- account + team auth via API key
- centralized config in `~/.prime/config.json`
- remote compute lifecycle
- remote sandbox lifecycle
- remote operator access
- remote evaluation publishing

That makes Prime much closer to:

- a hosted GPU marketplace + SDK
- a RunPod-style operator CLI
- a hosted sandbox product

than to:

- Ollama
- llama.cpp
- Psionic
- our local provider runtime

## What Psionic Actually Is

Psionic is the opposite end of the stack:

- Rust-native engine subtree under `crates/psionic/*`
- explicit runtime/backend/model/compiler/serve/provider layering
- local and cluster execution truth
- explicit backend readiness and validation posture
- served product contracts for `psionic.embeddings` and
  `psionic.text_generation`
- provider-facing receipts, delivery proofs, execution topology, and bounded
  future `psionic.sandbox_execution`

The important Psionic themes that keep showing up in source and docs are:

- deterministic and inspectable execution plans
- truthful backend selection and health
- explicit validation references
- provider evidence and receipts
- clear separation between reusable execution substrate and app UX

So Prime and Psionic are not substitutes. Prime is external hosted compute
operations. Psionic is our execution kernel and evidence layer.

## What Changes After Reading `~/code/pi`

The broader Prime Intellect mirror changes the framing materially.

Looking only at `prime` suggests:

- hosted cloud operator tooling
- remote sandboxes
- remote inference
- MCP wrapper

Looking at the full PI org reveals a much larger vertical stack:

- hosted operator tooling: `prime`
- decentralized protocol/control plane: `protocol`
- native P2P transport wrapper: `prime-iroh`
- fault-tolerant collectives: `pccl`
- decentralized training/recovery: `prime-diloco`
- async RL training + environments: `prime-rl`, `verifiers`
- distributed inference experiments: `prime-vllm`, `prime-pipeline`
- verification/proof systems: `toploc`, `toploc-validator`, `gpu-challenge`
- low-level native systems libs: `threadpark`, `threadpool`, `pi-quant`

That means the right updated reading is:

- `prime` is still mostly operator/control-plane UX research for us,
- but the broader PI stack includes several reference tracks that are directly
  relevant to future OpenAgents Rust work,
- especially cluster transport/orchestration, proof/evidence systems, and
  environment/eval substrate design.

It also matters that the PI stack is not uniformly Python. Some of the most
relevant repos are already native:

- `protocol` is a Rust workspace with `discovery`, `worker`, `validator`,
  `shared`, `orchestrator`, and `p2p` crates.
- `prime-iroh` is a Rust library with Python bindings.
- `pccl` is a C++/C/Python collective communications stack.

So for part of this stack, the task is not “port Python to Rust.” It is
“design a clean OpenAgents-native Rust equivalent, using PI’s existing native
and mixed-language systems as reference only.”

## Prime In Relation To OpenAgents

The relation is easiest to understand by stack position.

### 1. Prime is not our kernel

Prime should not be treated as a replacement for:

- `psionic-runtime`
- `psionic-serve`
- `psionic-provider`
- `psionic-cluster`

Those crates exist to make execution, capability claims, and receipts truthful
inside our own system. Prime does not provide that level of runtime-owned
evidence. It provides centralized account-backed APIs.

### 2. Prime is not our protocol truth

OpenAgents MVP truth is anchored in:

- desktop-first local execution
- Nostr/Nexus participation
- explicit wallet and payout state
- replay-safe state continuity

Prime’s trust model is centralized API auth and team context. That is fine for a
hosted cloud product, but it is not the same thing as our provider-market and
wallet truth.

### 3. Prime could become an optional external capacity provider

If we ever want “rent extra remote capacity” or “launch a remote sandbox/GPU
from Autopilot,” Prime is a plausible integration target. But that belongs at
the app orchestration layer as an explicit remote-capacity feature, not as the
default execution substrate.

In other words:

- Prime can be an optional upstream infrastructure provider.
- It should not become invisible plumbing underneath Psionic’s claims.

### 4. Prime cluster availability is not Psionic cluster execution

Prime does have multi-node and cluster-related surfaces, but they are about
availability discovery and pod procurement. That is materially different from
what `psionic-cluster` owns:

- execution topology
- shard placement
- delivery evidence
- ordered cluster state
- trusted-LAN admission and identity

So Prime is not a reference for Psionic cluster execution truth. At most it is a
reference for operator-facing capacity discovery.

### 5. Prime is useful as product and contract research

Prime has several mature operator-facing concepts that we can reuse without
adopting the hosted platform itself:

- typed sandbox lifecycle
- explicit command/file/background-job surfaces
- short-lived remote access sessions
- adaptive evaluation artifact uploads
- MCP wrapping for operator tools
- thin, composable package boundaries

This is the highest-value reason to study it.

## Where Prime Fits The MVP And Where It Does Not

`docs/MVP.md` makes the current product promise clear: Autopilot is your agent
on your computer, and the earn loop is about turning your own machine online.

That means Prime is a mixed fit.

### Fits the MVP indirectly

- operator tooling patterns
- future overflow capacity or remote execution options
- evaluation and evidence publication patterns
- future explicit sandbox product design

### Does not fit the MVP as the default path

- replacing local execution with hosted sandboxes
- replacing local provider truth with hosted account truth
- hiding remote execution behind a “local” provider claim
- making the MVP dependent on third-party centralized infrastructure for the
  core seller loop

If we integrated Prime naively, we would risk weakening the core story from
"your machine earns" into "our app forwards work to someone else’s cloud."
That is a product regression for the current MVP.

## What Prime Does Well Enough To Adapt

### 1. Explicit sandbox contract shape

Prime’s strongest reusable idea is not the cloud backend. It is the contract
shape around bounded remote execution.

`prime-sandboxes` already models:

- stable sandbox statuses
- explicit timeout and termination states
- environment vars vs secrets
- labels and filters
- background job handles and polling
- file upload/download
- port exposure records
- SSH session metadata
- structured error typing for OOM, timeout, image pull failure, and not-running
  states

This maps well to a future OpenAgents bounded execution product because Psionic
already has:

- `SANDBOX_EXECUTION_PRODUCT_ID = "psionic.sandbox_execution"`
- `SandboxExecutionCapabilityProfile`
- `SandboxExecutionEvidence`
- provider-facing sandbox capability envelopes and receipts

Recommended adaptation:

- use Prime’s sandbox lifecycle vocabulary as design input for the next Psionic
  sandbox contract review,
- port only the generic lifecycle and evidence ideas,
- keep them inside `psionic-runtime` and `psionic-provider` if they are reusable
  execution truth,
- keep product UX and orchestration in `apps/autopilot-desktop`.

What is worth stealing specifically:

- explicit “running vs terminated vs timeout vs image pull failed vs OOM”
  semantics
- separate access/session records instead of opaque connection strings
- first-class background-job identity instead of treating long-running work as
  “just command output”
- explicit port exposure models instead of ad hoc URLs

### 2. Retry and idempotency discipline around remote operations

Prime’s sandbox and tunnel clients distinguish between:

- idempotent operations that can retry on timeout
- non-idempotent operations that should not blindly retry after timeouts

That discipline is worth adapting anywhere OpenAgents talks to remote control
planes or future external providers. This is not a unique invention, but Prime
applies it cleanly and consistently enough to copy.

Best fit in our codebase:

- app-owned remote integrations
- future external capacity providers
- any remote evidence upload path

Not a reason to touch local Psionic execution.

### 3. MCP as a thin operator wrapper

`prime-mcp-server` is thin, but the thinness is the lesson. It does not try to
rebuild the product. It exposes a narrow set of high-value tools over MCP with
good tool descriptions and safety guidance.

OpenAgents already has an app-owned control plane and `autopilotctl`. That
makes Prime’s MCP shape highly relevant.

Recommended adaptation:

- add a thin OpenAgents MCP server that wraps `autopilotctl` and/or the app
  control plane,
- expose operator-safe actions such as:
  - local runtime status
  - provider online/offline
  - wait-for-ready flows
  - packaged verification hooks
  - benchmark/conformance status
- keep it outside `crates/psionic/*`
- keep it as app/tooling surface, probably next to `apps/autopilot-desktop` or
  a thin tools crate

This is one of the better near-term adaptations because it improves testing and
agent control without changing the MVP product story.

### 4. Evaluation artifact publishing patterns

`prime-evals` has a simple but useful shape:

- create an evaluation record
- resolve/validate environment identity
- push samples in adaptive batches
- retry rate-limited uploads
- finalize with metrics

We should not copy the product semantics directly, but the flow is relevant to
our growing collection of:

- conformance reports
- performance benchmark artifacts
- hardware validation evidence
- packaged verification outputs

Recommended adaptation:

- define an OpenAgents evidence-upload contract for benchmark/conformance
  artifacts,
- use adaptive batching and explicit finalization semantics,
- treat evidence as first-class productized artifacts rather than loose logs.

Best fit:

- docs and tooling around `psionic` validation
- future Nexus-side evidence upload or hosted validation dashboards

### 5. Thin package boundaries around operator tasks

Prime’s package split is directionally good:

- a main CLI
- thin focused SDKs
- MCP wrapper
- tunnel wrapper
- evals wrapper

The exact implementation is not something to copy literally because the local
tree duplicates config/client code across several packages. But the product
boundary idea is useful:

- keep operator surfaces narrow
- do not force one giant dependency tree onto every tool
- split heavy product code from lightweight SDK wrappers

OpenAgents can apply that lesson when deciding whether a feature belongs in:

- the desktop app
- `autopilotctl`
- a thin SDK/client crate
- a future MCP bridge

### 6. Explicit remote access/session modeling

Prime’s sandboxes and tunnels do not treat remote access as magic. They model:

- tunnel identity
- expiry
- host/port/url
- binding/auth tokens
- SSH sessions with TTL and gateway metadata

That is useful for any future OpenAgents remote-control or delegated-execution
surface. We should adapt the explicit session metadata pattern, not the `frpc`
dependency or Prime control plane.

This matters most if we later add:

- remote provider administration
- remote artifact browsers
- remote sandbox attach/inspect flows
- hosted Autopilot helper services

## What The Broader PI Stack Is Worth Rebuilding In Rust

Given the explicit goal is to build much of their stack in Rust rather than use
their code directly, the broader PI repos should be treated as reference/oracle
repos only. The useful question is not “should we integrate this?” but “which
parts are worth rebuilding in our own stack, and where do they belong?”

### 1. `protocol` + `prime-iroh` + `pccl`: highest-value cluster/control-plane references

This is the most important PI cluster for OpenAgents.

Why it matters:

- `protocol` already expresses a decentralized compute stack in Rust with
  discovery, worker, validator, orchestrator, and p2p crates.
- `prime-iroh` shows one way to factor internet-grade reliable P2P transport
  into a native core with higher-level bindings.
- `pccl` is a serious fault-tolerant collective communications system with
  shared-state sync, dynamic join/leave, and bandwidth-aware topology work.

This is the closest PI material to our future needs in:

- `psionic-cluster`
- `psionic-runtime`
- `psionic-provider`

What to rebuild, not reuse:

- cluster admission and identity model
- peer discovery and topology dissemination
- explicit execution-lane and placement policy
- fault-tolerant collectives and state sync
- bandwidth-aware topology planning
- internet-facing versus trusted-LAN posture distinctions

What not to copy literally:

- PI’s exact validator/orchestrator/economic assumptions
- any coupling to their hosted platform or EVM contracts
- Python wrapper shape around native transport

The correct OpenAgents move is to keep building our own Rust-native cluster and
evidence layer, while using these repos as reference checks for semantics and
failure handling.

### 2. `prime-vllm` + `prime-pipeline`: reference for distributed inference coordination, not runtime truth

These repos are useful because they show how PI thinks about:

- model sharding
- rank/world-size configuration
- pipelined inference over public networks
- benchmark decomposition into startup/prefill/decode
- explicit comms hooks across stages

They are not good direct foundations for OpenAgents because they remain wrapped
around Python inference stacks such as vLLM and GPT-Fast-derived code.

Recommended reading of their value:

- very useful as coordination and benchmarking references
- not acceptable as execution truth for Psionic
- worth mining for sharding UX, benchmark taxonomy, and stage-boundary design

Best ownership fit if rebuilt:

- `psionic-cluster`
- `psionic-runtime`
- maybe future cluster benchmark tooling

### 3. `prime-diloco`: strong reference for elastic distributed training and live recovery

`prime-diloco` is less relevant to the current MVP than cluster inference, but
its design ideas are still strong:

- elastic device mesh
- heartbeat-driven membership changes
- live checkpoint recovery from peers
- asynchronous checkpoint staging and upload
- low-communication sync strategy

This does not belong in the current Autopilot MVP path. It matters later if
OpenAgents wants:

- large-scale distributed training
- elastic multi-node job control
- resumable long-running compute markets for training-class work

The right takeaway is:

- not a near-term product fit,
- but a high-value semantic reference for future Rust-native elastic training or
  long-running cluster job orchestration.

### 4. `verifiers` + `prime-rl`: strong reference for an environment/eval substrate

This is the most compelling PI application-layer reference after the cluster
stack.

What `verifiers` gets right conceptually:

- environment = dataset + harness + rubric
- self-contained task modules
- explicit local and hosted evaluation flow
- integration between task definitions, training, and evals

What `prime-rl` adds:

- a clean separation between orchestrator, trainer, inference, and eval roles
- async/off-policy training semantics
- modular configs and example-driven workflow

For OpenAgents, the opportunity is not “build PRIME-RL in Rust now.” The better
reading is:

- build a Rust-native or mixed Rust/app-layer environment and benchmark
  substrate over time,
- keep the concept of explicit environment definitions, harnesses, and rubrics,
- connect that to buyer-side jobs, local agent benchmark flows, and future
  skill/plugin testing.

Best fit if rebuilt:

- likely new crates or app/tooling layers, not `psionic-runtime`
- maybe future `openagents-envs` / `openagents-evals` style surfaces

This is strategically relevant, but below cluster/proof work for the current
MVP.

### 5. `toploc` + `toploc-validator` + `gpu-challenge`: high-strategic-value proof and validation references

This PI cluster aligns unusually well with OpenAgents’ emphasis on truthful
execution and receipts.

Why it matters:

- `toploc` is about compact activation-derived proof material for verifiable
  inference.
- `toploc-validator` turns that into an operational validation service.
- `gpu-challenge` shows a different verification shape for outsourced GPU work
  using Freivalds-style checking plus Merkle commitments.

For OpenAgents this is more important than it might seem, because we already
care about:

- truthful provider receipts
- delivery proofs
- validation references
- future market trust hardening

These repos suggest a real future track:

- OpenAgents-native proof/evidence services in Rust
- explicit verification lanes for higher-trust remote or clustered execution
- evidence formats that go beyond “job said it finished”

This should remain a future bounded track, not something we promise in MVP, but
it is one of the strongest PI areas to study.

### 6. `threadpark` + `threadpool` + `pi-quant` + `datasetstream`: useful low-level implementation references

These repos matter because they show PI building native infrastructure instead
of only stitching Python tools together.

Useful takeaways:

- `threadpark`: portable park/unpark semantics
- `threadpool`: low-overhead native scheduling primitives
- `pi-quant`: SIMD-aware multithreaded quant/dequant kernels
- `datasetstream`: thin data-plane shape for token streaming

For OpenAgents these are not first-order product surfaces, but they are useful
when we need:

- fast host-side helper kernels
- queueing/execution runtime utilities
- CPU quant/dequant and staging paths
- high-throughput local or cluster data feeds

The right approach is still Rust-first. These repos are evidence that the
underlying problems are worth solving natively, not a reason to add C/C++/FFI
sprawl by default.

## What The Broader PI Stack Still Does Not Fit

### 1. `smart-contracts` is not a near-term fit for our economic model

PI’s protocol contracts are EVM-first and staking/slashing oriented. Our MVP is
desktop-first, Nostr/Nexus, and Bitcoin/Lightning anchored.

So even if the contract system is interesting, it should not drive near-term
OpenAgents architecture.

Relevant as:

- market/economy research
- incentive-mechanism reference

Not relevant as:

- direct foundation for MVP payout or provider identity

### 2. The training-heavy PI stack is strategically relevant but not MVP-central

Repos like:

- `prime-rl`
- `prime-diloco`
- `OpenRLHF`
- `torchtune`
- `DeepSpeed`

are important if the goal becomes large-scale training or post-training
infrastructure. They are not the core of “Autopilot prints Bitcoin” today.

The risk is roadmap drift: building a research/training platform instead of a
truthful local provider product.

## What We Should Not Adapt

### 1. Do not make Prime a hidden Psionic backend

Psionic’s value is that execution truth, backend truth, and receipts stay
inside our own reusable Rust substrate. Prime-hosted execution would be a
different product lane with different trust and evidence properties.

If we ever use Prime compute, it must be explicit in:

- capability reporting
- UI wording
- receipts/evidence
- product configuration

### 2. Do not weaken the desktop-first MVP with cloud-default behavior

The MVP is not “rent a pod.” It is “turn your machine into an earner.”

So we should not:

- default Autopilot to remote sandboxes
- quietly offload jobs to Prime
- use hosted inference as the implicit fallback for local runtime failure

Any remote capacity feature has to be explicit and secondary.

### 3. Do not copy Prime’s package-internal duplication

The local Prime tree repeats very similar `Config`, `APIClient`, async client,
and user-agent code across packages. Their package boundaries are useful; their
internal duplication is not.

If we borrow the package split idea, we should implement shared primitives once
in Rust rather than clone the same client stack into multiple crates.

### 4. Do not put Prime-shaped logic into the wrong crates

`docs/OWNERSHIP.md` rules matter here:

- no Prime/cloud orchestration in `crates/wgpui`
- no app-specific Prime workflow in reusable Psionic crates
- no app-to-cloud product logic hidden inside provider/runtime crates unless it
  is truly generic execution contract data

Concrete boundary:

- reusable execution facts belong in `psionic-runtime` / `psionic-provider`
- optional hosted-provider integration belongs in app/tooling layers

The same logic applies to broader PI-inspired work:

- cluster/runtime/evidence semantics belong in reusable execution crates
- environment/eval workflows belong in app or dedicated non-engine crates
- market-specific orchestration must not be smuggled into generic runtime code

### 5. Do not confuse Prime’s centralized team/account model with our market model

Prime’s auth, billing, and team context are centralized SaaS concepts. They may
be useful for one integration, but they are not substitutes for:

- Nostr identity
- Spark wallet state
- provider payout truth
- our marketplace receipts

Treat those as external integration credentials, not as foundational OpenAgents
identity.

## Recommended Adaptation Sequence

The best path is to adapt the highest-signal ideas in order of MVP fit.

### 1. Add an OpenAgents MCP/operator wrapper

Near-term value:

- high
- low architectural risk
- no change to execution truth

Likely home:

- app/tooling surface near `apps/autopilot-desktop`

### 2. Do a Psionic sandbox-contract review using Prime as one reference

Near-term value:

- medium-high
- useful future-proofing
- aligns with already-landed `psionic.sandbox_execution` capability/evidence
  types

Likely home:

- `psionic-runtime`
- `psionic-provider`
- supporting docs under `crates/psionic/docs/`

### 3. Add evidence-upload/productization patterns for benchmarks and conformance

Near-term value:

- medium
- improves rigor around validation artifacts

Likely home:

- tooling/docs first
- maybe later Nexus-side artifact ingestion

### 4. Consider an explicit optional external-capacity adapter

Near-term value:

- medium, but only after MVP local path is strong

Likely home:

- app orchestration layer
- never as hidden fallback in Psionic

This could eventually support Prime or other external providers behind one
OpenAgents-owned abstraction, but it should not happen before the local seller
loop is already solid.

## If We Intend To Rebuild Much Of PI’s Stack In Rust

If the real ambition is a broader Rust-native OpenAgents stack inspired by PI,
the priority order should be different from the narrower `prime`-integration
story above.

### 1. Cluster/control-plane track first

Reference repos:

- `protocol`
- `prime-iroh`
- `pccl`

Target home:

- `psionic-cluster`
- `psionic-runtime`
- `psionic-provider`

Reason:

- this is the closest match to our reusable execution substrate
- it strengthens clustered execution, topology truth, and future market
  hardening

### 2. Proof/evidence track second

Reference repos:

- `toploc`
- `toploc-validator`
- `gpu-challenge`

Target home:

- likely new proof/evidence crates plus provider/runtime integration

Reason:

- it compounds the value of truthful receipts and verifiable execution
- it gives us a differentiated trust path instead of commodity inference hosting

### 3. Environment/eval substrate third

Reference repos:

- `verifiers`
- `prime-rl`

Target home:

- app/tooling layers or new dedicated crates

Reason:

- useful for buyer jobs, benchmarks, skills, and agent task evaluation
- strategically valuable, but not as foundational as cluster/proof work

### 4. Distributed inference semantics fourth

Reference repos:

- `prime-vllm`
- `prime-pipeline`

Target home:

- `psionic-cluster`
- cluster benchmarks and planning docs

Reason:

- valuable once our own clustered runtime surface is ready
- still downstream of the transport/topology substrate

### 5. Elastic training later

Reference repos:

- `prime-diloco`

Reason:

- important if OpenAgents later broadens into training markets
- not the right next move for the current desktop/provider MVP

## Suggested Concrete Follow-Up Work

### 1. Write a focused `psionic.sandbox_execution` contract audit

Goal:

- compare current Psionic sandbox capability/evidence types against Prime
  sandbox lifecycle concepts,
- decide which missing fields are truly reusable execution truth,
- keep app UX and remote orchestration out of the engine crates.

### 2. Add an OpenAgents MCP server over `autopilotctl`

Goal:

- make local runtime, provider, and packaged verification flows agent-friendly,
- mirror the “thin wrapper over existing control plane” approach Prime uses.

### 3. Define an evidence artifact schema

Goal:

- standardize how Psionic conformance/perf/validation artifacts are serialized,
  uploaded, and finalized,
- borrow batching/retry/finalization lessons from `prime-evals`.

### 4. If remote capacity becomes important, design it as a named lane

Goal:

- make “hosted remote capacity” explicit in UX and receipts,
- avoid violating the MVP desktop/local promise,
- support Prime as one possible provider instead of hard-wiring Prime into the
  core stack.

### 5. Write a PI-stack Rust rebuild roadmap

Goal:

- separate PI-inspired rebuild work into
  - cluster/control plane
  - proof/evidence
  - environment/eval
  - distributed inference
  - training
- assign each stream to the correct crate/app layer
- keep MVP work from getting swallowed by long-horizon infrastructure ambition

## Bottom Line

Prime is useful to us, but mostly as an external control-plane reference and a
source of contract ideas, not as a runtime foundation.

The broader PI org is more important than `prime` alone. It contains real
native protocol, communications, verification, and training infrastructure that
is worth studying as reference material for OpenAgents-owned Rust systems.

The correct takeaway is:

- Prime is a hosted compute/sandbox operator product.
- Psionic is our execution substrate and evidence layer.
- OpenAgents should borrow Prime’s operator-surface and sandbox-contract ideas
  where they improve clarity.
- OpenAgents should not outsource or blur its local execution truth to Prime.
- PI’s broader native stack is worth treating as reference material for future
  Rust-native OpenAgents cluster, proof, and environment/eval subsystems.
- We should build our own versions, not wire their code into our core path.

If we adapt anything soon, the best bets are:

- a thin MCP/operator wrapper over our existing control plane,
- a sharper future `psionic.sandbox_execution` contract,
- stronger evidence artifact/productization for conformance and benchmarks.
- If we go broader than that, the next real PI-inspired Rust tracks are
  `protocol`/`prime-iroh`/`pccl` first, then `toploc`/`gpu-challenge`, then
  `verifiers`-style environment and eval substrate work.
