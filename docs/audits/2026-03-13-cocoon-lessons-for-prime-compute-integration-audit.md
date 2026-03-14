# 2026-03-13 Cocoon Lessons For Prime Compute Integration Audit

## Intent

This audit answers a narrower question than the existing Prime-focused docs:

> while OpenAgents widens its compute stack around the Prime ecosystem plan,
> what should it learn from `~/code/cocoon` and `~/code/cocoon-contracts`,
> what should it adapt, and what should it explicitly avoid copying?

The answer is not "turn OpenAgents into Cocoon."

The useful answer is:

- Cocoon is not the right reference for Prime-style clustered inference topology.
- Cocoon is a very strong reference for attested runtime packaging, artifact
  truth, operator health tooling, rollback-resistant economic sequencing, and
  adversarial testing of compute-market state transitions.

That is exactly where the current Prime integration plan still needs concrete
implementation discipline.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/plans/prime-ecosystem-compute-integration-spec.md`
- `docs/plans/compute-market-full-implementation-plan.md`
- `docs/kernel/economy-kernel.md`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-provider-substrate/src/{sandbox.rs,sandbox_execution.rs}`
- `apps/autopilot-desktop/src/kernel_control.rs`

Cocoon sources reviewed:

- `~/code/cocoon/README.md`
- `~/code/cocoon/docs/{README.md,architecture.md,deployment.md,ra-tls.md,seal-keys.md,smart-contracts.md,tdx-and-images.md,gpu.md}`
- `~/code/cocoon/scripts/dist-worker/README.md`
- `~/code/cocoon/spec/spec-worker/worker-config.json`
- `~/code/cocoon/spec/spec-proxy/proxy-config.json`
- `~/code/cocoon/tee/cocoon/*`

Cocoon contracts sources reviewed:

- `~/code/cocoon-contracts/README.md`
- `~/code/cocoon-contracts/wrappers/{CocoonRoot.ts,CocoonProxy.ts}`
- `~/code/cocoon-contracts/tests/{Integration.spec.ts,CocoonRoot.spec.ts,CocoonClient.spec.ts}`

## Executive Summary

The current OpenAgents Prime plan is directionally right. It already points at
the correct long-term owners:

- `apps/autopilot-desktop` for operator and product control
- `crates/psionic/*` for reusable execution substrate
- `openagents-kernel-*` plus `apps/nexus-control` for economic truth

What Cocoon adds is a more operational answer to a question our current plan
still treats too abstractly:

> what exact evidence should bind runtime identity, artifact identity,
> configuration identity, session identity, and payout state together?

OpenAgents already has some of the schema slots:

- `ComputeCapabilityEnvelope`
- `ComputeEnvironmentBinding`
- `ComputeProofPosture`
- `DeliveryProof`

It also already records launch lots with
`attestation_posture: "desktop.local.best_effort"` and currently does not
require attestation for launch products. That is a truthful MVP choice, but it
means the Prime/Psionic plan still needs a concrete path from "best effort
local provenance" to "machine-legible evidence with policy meaning."

Cocoon is the best local reference for that path.

## Current OpenAgents Gap, Precisely

The repo already models proof and environment concerns in the kernel:

- `crates/openagents-kernel-core/src/compute.rs` carries execution kind,
  topology kind, proof posture, environment binding, attestation posture, cost
  proof posture, and delivery proof fields.
- `apps/autopilot-desktop/src/kernel_control.rs` already publishes compute
  products and capacity lots into the kernel and records delivery proofs when
  jobs complete.
- `crates/openagents-provider-substrate/src/sandbox.rs` and
  `crates/openagents-provider-substrate/src/sandbox_execution.rs` already model
  sandbox profiles, runtime digests, artifacts, and execution evidence.

The gap is not "missing nouns." The gap is that most of those nouns are still
filled with local, app-generated, best-effort evidence rather than a stable,
portable, policy-meaningful evidence package.

That is where Cocoon is useful.

## What Cocoon Gets Right That OpenAgents Should Adapt

### 1. Runtime identity is a package, not a string

Cocoon treats runtime identity as a digest-bound package:

- image components are reproducible and measured
- static config is measured separately from runtime variables
- model identity is tracked as `model@commit:verity_hash`
- the root registry carries allowed image hashes, model hashes, and code hashes

The strongest concrete examples are:

- `~/code/cocoon/docs/tdx-and-images.md`
- `~/code/cocoon/docs/gpu.md`
- `~/code/cocoon/spec/spec-worker/worker-config.json`
- `~/code/cocoon-contracts/wrappers/CocoonRoot.ts`

OpenAgents should adapt this as:

- a Psionic-owned runtime/environment manifest, not just ad hoc metadata
- digest-bound artifact identity for model shards, sandbox images, eval bundles,
  and environment packages
- a first-class split between measured/static config and non-security-critical
  runtime vars
- product identity that can say more than "gpt_oss" or "apple_fm"; it should
  carry artifact lineage and environment lineage

Recommended owner split:

- `crates/psionic/*`: build manifests, artifact manifests, environment package
  manifests, runtime digests
- `crates/openagents-kernel-core`: carry manifest refs and digest slots in
  `ComputeEnvironmentBinding`, `ComputeCapabilityEnvelope`, and `DeliveryProof`
- `apps/nexus-control`: policy allowlists and read models for accepted manifests

This is more important than any specific TDX detail. The reusable lesson is the
manifest discipline.

### 2. Session identity should be bound to proof claims

Cocoon's RA-TLS layer is valuable because it does not treat transport identity
and attestation identity as separate concerns. The session certificate embeds
claims, the claims are bound to the session public key, and both sides verify
that bundle during connection setup.

Relevant sources:

- `~/code/cocoon/docs/ra-tls.md`
- `~/code/cocoon/tee/cocoon/router.cpp`
- `~/code/cocoon/tee/cocoon/tdx.cpp`

OpenAgents should adapt the pattern, not the exact TDX implementation:

- `psionic-net` sessions should carry a signed session-claims bundle
- the claims bundle should bind to the peer public key or session key material
- the claims bundle should reference runtime/environment/artifact digests
- cluster admission and sandbox attach flows should evaluate policy from that
  same bundle

This should support multiple proof levels:

- `none`
- `desktop_local_best_effort`
- `software_signed`
- `hardware_attested`
- `challenge_eligible`

That maps cleanly to the Prime plan's future `psionic-net`, `psionic-proof`,
and validator work, and it fits the existing kernel proof-posture taxonomy much
better than a binary "TEE or nothing" model.

### 3. Operator tooling must expose trust state directly

Cocoon is unusually concrete about bring-up and operator visibility:

- `cocoon-launch` supports local, fake-chain, real-chain, and production modes
- `/stats`, `/jsonstats`, and `/perf` expose instance truth
- `health-client` exposes service health, logs, GPU state, and TDX status

Relevant sources:

- `~/code/cocoon/docs/deployment.md`
- `~/code/cocoon/scripts/dist-worker/README.md`
- `~/code/cocoon/tee/cocoon/health-client.cpp`

This is directly aligned with the Prime plan section that says desktop control
and `autopilotctl` must remain the operator seed.

OpenAgents should adapt this as:

- richer `autopilotctl` commands for artifact manifests, topology, proof state,
  sandbox jobs, and challenge status
- stable health/readout endpoints in the app-owned control plane
- layered test modes for local-only, simulated-attestation, and
  authority-connected compute
- packaged verification flows for future clustered/sandbox products, not just
  local runtime status

The important lesson is not "write a separate health-client binary." The lesson
is that trust state, artifact state, and service health need to be inspectable
without reading logs by hand.

### 4. Policy and registry objects need explicit versioning

Cocoon's root contract and config wrappers are blunt but useful:

- they version parameters
- they separate registry state from instance state
- they keep allowlists for images, models, proxies, and code

Relevant sources:

- `~/code/cocoon/docs/smart-contracts.md`
- `~/code/cocoon-contracts/wrappers/CocoonRoot.ts`
- `~/code/cocoon-contracts/tests/CocoonRoot.spec.ts`

OpenAgents should not copy the blockchain part. It should copy the versioning
discipline:

- versioned attestation profiles
- versioned environment package policies
- versioned allowed-manifest registries
- explicit linkage from compute product / lot / delivery proof to the policy
  bundle and manifest version actually evaluated

This belongs in:

- `crates/openagents-kernel-core`
- `crates/openagents-kernel-proto`
- `apps/nexus-control`

The existing `PolicyContext` in desktop kernel mutations is a start, but it is
not yet the same thing as a durable manifest/policy registry.

### 5. Rollback-resistant payout state matters before full financial expansion

Cocoon's payment design is over-coupled to TON for OpenAgents' needs, but it
solves one operational problem clearly:

> a compute marketplace cannot trust local mutable state alone when payout,
> stake, refund, and close-out state can be replayed or rolled back.

Relevant sources:

- `~/code/cocoon/docs/smart-contracts.md`
- `~/code/cocoon-contracts/wrappers/CocoonProxy.ts`
- `~/code/cocoon-contracts/tests/Integration.spec.ts`

OpenAgents should adapt this into kernel receipts and wallet-linked settlement,
not smart contracts:

- explicit receipt families for reserve, deliver, accept, reject, withdraw,
  timeout, and close
- periodic commitment of economically important provider state into durable
  authority receipts
- replay-safe links between payout references and delivery proofs
- provider-side close semantics that make "I went offline" different from
  "I abandoned an open obligation"

This is already implicit in the compute-market plan and kernel docs. Cocoon is
useful because it forces the uncomfortable part into concrete lifecycle rules.

### 6. Economic state machines need adversarial tests, not just happy-path demos

The `cocoon-contracts` repo is valuable partly because the tests are willing to
encode ugly cases and known bugs.

Examples:

- `tests/Integration.spec.ts` models realistic lifecycle usage rather than only
  trivial unit calls
- `tests/CocoonClient.spec.ts` explicitly documents a known stake-direction bug
  (`[BUG #4]`)
- the test suite checks refund/excess behaviors and close semantics repeatedly

This is a strong lesson for OpenAgents because the Prime plan widens into:

- clustered delivery
- sandbox jobs
- challenge flows
- collateral and claim flows
- more explicit inventory and settlement state

All of those are economic state machines. They need:

- realistic lifecycle tests
- replay tests
- partial failure tests
- rollback tests
- offline/timeout/close tests

The strongest thing to copy from Cocoon here is not the contract language. It
is the willingness to treat these flows as failure-prone protocol systems.

## What OpenAgents Should Not Copy

### 1. Do not copy Cocoon's network topology

Cocoon is fundamentally:

- client
- proxy
- worker

That is not the right canonical topology for the Prime/Psionic plan, which is
moving toward:

- desktop-owned operator surface
- Psionic peer transport
- clustered execution
- sandbox execution
- kernel-linked economic authority

Proxy concentration may make sense for Cocoon's confidential inference model.
It is not the right mental model for OpenAgents' broader compute market.

### 2. Do not block the compute plan on TDX, SGX, or H100-class hardware

Cocoon is intentionally built around confidential-compute hardware and high-end
NVIDIA constraints.

That is incompatible with the retained MVP promise in `docs/MVP.md`, which is
desktop-first and already supports Apple FM and GPT-OSS local lanes.

OpenAgents should support hardware-attested lanes later, but only as one proof
posture among several. It should not redefine the product around hardware that
most providers do not have.

### 3. Do not move authority out of the kernel and into a chain-shaped registry

Cocoon's root/proxy/client/worker contracts are informative, but OpenAgents
already has a better owner split for this repo:

- execution truth in Psionic
- economic truth in kernel/Nexus
- operator truth in desktop control

The right adaptation is durable receipts and read models, not TON-style
contracts or chain-bound governance.

### 4. Do not create a separate hidden operator world

Cocoon has dedicated external tooling because it is running VMs and host-side
attestation helpers. OpenAgents should resist creating a second operator stack
that bypasses desktop control.

If a proof, topology, artifact, or challenge status matters, it should be
available through the same app-owned control plane the product already uses.

## Recommended Changes To The Prime / Compute Backlog

### Highest priority

1. Add a Psionic runtime/environment manifest model.
   It should cover artifact digests, static config digest, runtime-vars digest,
   runtime engine identity, and optional hardware-attestation evidence refs.

2. Extend kernel compute objects to carry those refs explicitly.
   `ComputeEnvironmentBinding`, `ComputeCapabilityEnvelope`, and
   `DeliveryProof` should gain durable manifest linkage rather than relying on
   loose metadata.

3. Add session-claims bundles in `psionic-net`.
   Bind peer identity, runtime manifest, and proof posture to session keys.

4. Extend desktop control and `autopilotctl` to expose:
   artifact manifest identity, proof posture, topology identity, sandbox job
   status, validator/challenge state, and policy bundle linkage.

### Next priority

5. Add a durable manifest/policy registry in the kernel authority layer.
   This is the OpenAgents equivalent of Cocoon's root registry, but it should
   live in receipts and read models instead of a chain contract.

6. Add settlement/close lifecycle receipts for compute obligations.
   This should make delivery, acceptance, timeout, withdrawal, and close-out
   explicit and replay-safe.

7. Add simulated-attestation and partial-failure test harnesses.
   The repo should be able to exercise these flows without requiring special
   hardware, just as Cocoon supports local/fake-chain/test modes.

### Lower priority but still important

8. Add hardware-attested proof adapters later.
   TDX-like or GPU-attested lanes are valuable future proof postures, but they
   should plug into the generic manifest/session/proof model rather than define
   it.

## Ownership Map For The Adaptation

- `apps/autopilot-desktop`
  - own operator presentation, `autopilotctl`, health views, and user-facing
    trust/readiness state
- `crates/openagents-provider-substrate`
  - remain the narrow descriptor layer; do not absorb deeper attestation or
    cluster runtime ownership
- `crates/psionic/*`
  - own runtime manifests, artifact manifests, session claims, transport proof
    hooks, sandbox runtime, and optional hardware-attestation adapters
- `crates/openagents-kernel-core` and `crates/openagents-kernel-proto`
  - own policy-facing proof taxonomy, manifest refs, and delivery/settlement
    object shapes
- `apps/nexus-control`
  - own accepted policy registries, read models, and durable receipt mutation
    for proof-sensitive compute state

## Bottom Line

The most useful way to read Cocoon against the current Prime integration plan
is this:

- Prime gives OpenAgents the right expansion target for transport, clustered
  execution, sandboxing, evals, and validators.
- Cocoon gives OpenAgents the right implementation instincts for trust
  packaging, artifact truth, operator visibility, and rollback-resistant market
  state.

So the thing to copy from Cocoon is not its chain stack or its proxy topology.

The thing to copy is its insistence that compute claims become concrete through:

- manifest digests
- policy allowlists
- session-bound proof claims
- inspectable operator health
- explicit close/withdraw semantics
- adversarial lifecycle tests

That is the missing discipline that can make the Prime/Psionic plan land as a
truthful compute market instead of a large but loosely evidenced execution
system.
