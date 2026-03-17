# ADR-0003: Compute Market Ownership And Authority Split

- Status: Accepted
- Date: 2026-03-13
- Owners: desktop + runtime + contracts-docs
- Supersedes: none
- Related: `ADR-0001-spacetime-domain-authority-matrix.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, `docs/plans/compute-market-full-implementation-plan.md`, `docs/plans/prime-ecosystem-compute-integration-spec.md`

## Context

OpenAgents now has three different compute truths in active use:

1. the product truth in `apps/autopilot-desktop`
2. the reusable execution truth in `OpenAgentsInc/psionic` and
   `crates/openagents-provider-substrate`
3. the economic truth in `openagents-kernel-*` plus `apps/nexus-control`

That split is already visible in the retained tree:

- `apps/autopilot-desktop/src/local_inference_runtime.rs` owns the app-level
  local-runtime seam and operator experience
- `apps/autopilot-desktop/src/kernel_control.rs` already bridges provider
  execution back into kernel compute-market objects
- `apps/autopilot-desktop/src/desktop_control.rs` and `autopilotctl.rs` already
  provide an app-owned compute control plane
- `crates/openagents-provider-substrate` already owns provider health, product
  derivation, inventory controls, and sandbox profile summaries
- `OpenAgentsInc/psionic` already own reusable execution substrate, clustered
  execution evidence, and provider-facing proof/export shapes
- `crates/openagents-kernel-core/src/compute.rs` and `apps/nexus-control` already
  own canonical compute-market objects, receipts, snapshots, and authority
  mutation behavior

The new compute program intentionally widens this stack using Prime/PI-inspired
patterns:

- wider-network transport
- clustered execution
- bounded sandbox execution
- proof and challenge infrastructure
- environment and eval infrastructure
- later training-class execution

Without an explicit ownership split, implementation pressure will push the repo
toward one of three bad outcomes:

- app-owned orchestration swallowing reusable compute substrate
- Psionic or provider-substrate swallowing market authority and settlement truth
- validator/environment/eval logic landing ad hoc in whichever crate needed it
  first

## Decision

Compute ownership is split by function, not by which layer happens to touch the
feature first.

### 1) `apps/autopilot-desktop` owns product and operator truth

`apps/autopilot-desktop` owns:

- Mission Control, Provider Control, and buyer/provider product behavior
- the app-owned local-runtime seam
- desktop-controlled provider orchestration
- app-owned execution snapshots and human-facing inventory presentation
- the app-owned control plane in `desktop_control.rs`
- the thin operator CLI in `autopilotctl.rs`

`apps/autopilot-desktop` must not own:

- reusable clustered transport or cluster state machines
- reusable sandbox runtimes
- reusable proof-generation or validator implementations
- canonical market settlement, index, or claim logic

### 2) `crates/openagents-provider-substrate` owns narrow reusable provider semantics

`crates/openagents-provider-substrate` owns:

- provider backend identity and health models
- provider lifecycle helpers
- launch and future compute-product derivation from detected runtime state
- reusable provider inventory control primitives
- provider-facing sandbox profile summaries and execution-class descriptors

`crates/openagents-provider-substrate` must not own:

- the long-term sandbox runtime engine
- cluster networking or ordered-state logic
- proof-generation engines
- market settlement, procurement, or authority behavior
- pane UX or app-specific orchestration

Short-term exception:

- existing local sandbox execution code may remain here while a Psionic-owned
  sandbox runtime is being extracted, but the long-term owner of executable
  sandbox runtime is Psionic rather than provider-substrate

### 3) `OpenAgentsInc/psionic` own reusable compute execution substrate

`OpenAgentsInc/psionic` own:

- local inference and embeddings execution
- clustered transport, ordered execution state, and topology planning
- artifact residency and staging logic for execution
- sandbox execution runtime and evidence, once extracted
- execution-proof bundle assembly
- clustered delivery evidence and settlement linkage inputs
- later training-class execution substrate

`OpenAgentsInc/psionic` must not own:

- pane UX
- wallet or payout UX
- canonical market authority
- buyer-side procurement UX
- final settlement, collateral draw, or risk claim authority

### 4) `openagents-kernel-*` plus `apps/nexus-control` own economic truth

`openagents-kernel-core`, `openagents-kernel-proto`, and `apps/nexus-control`
own:

- canonical compute product, lot, instrument, delivery, and index objects
- authority mutation and receipt contracts
- read-model and snapshot publication for compute-market truth
- policy gating for market-level behavior
- validator-result acceptance and adjudication authority
- settlement, collateral, and claim consequences

These layers must not delegate economic authority to:

- Psionic ordered execution state
- desktop-local caches
- Nostr or transport-visible gossip
- operator judgment without a canonical mutation and receipt path

### 5) Planned validator services own proof and challenge execution, not settlement authority

When introduced, validator services own:

- challenge execution
- proof verification workloads
- adjudication inputs and supporting evidence

Validator services do not own:

- final market settlement authority
- canonical collateral or claim resolution authority
- buyer/provider UX

Validator output must terminate in kernel-owned objects and receipts.

### 6) Planned environment/eval services own reusable environment and evaluation infrastructure

When introduced, environment/eval services own:

- environment package descriptors
- dataset, harness, and rubric registries
- evaluation-run lifecycle helpers
- synthetic-data generation and verification pipeline helpers

They do not own:

- final compute settlement authority
- provider-side product UX
- reusable low-level execution runtimes that belong in Psionic

Environment/eval artifacts may be inputs to compute products and delivery
proofs, but they are not themselves the economic authority.

## Authority Split

The compute program has two authoritative planes and they must not be conflated.

### Execution-truth authority

Execution-truth authority lives in Psionic-owned substrate and covers:

- cluster membership and topology facts
- runtime readiness
- artifact residency
- realized execution topology
- runtime evidence and proof-bundle assembly

### Money-truth authority

Money-truth authority lives in kernel-owned substrate and covers:

- advertised products and lots
- accepted obligations
- accepted delivery proofs
- challenge outcomes once adjudicated
- settlement, collateral, and claim consequences
- index publication and correction

Execution truth may inform money truth.

Execution truth does not replace money truth.

## App Control Surface Rule

The existing app-owned compute control plane is retained as a first-class owner.

This means:

- `desktop_control.rs` remains the primary app-owned machine interface for the
  running desktop
- `autopilotctl` remains the thin CLI over that control plane
- future headless, MCP, cluster, or sandbox operator surfaces should extend or
  wrap this truth where appropriate rather than creating a silent second
  operator world

Exception:

- dedicated validator and authority services may expose service-native control
  surfaces for service operations, but those surfaces must not redefine
  app-owned product state or kernel-owned economic state

## Review Invariants

1. Desktop owns product behavior and operator UX, not reusable execution
   substrate.
2. Provider-substrate owns narrow reusable provider semantics, not full runtime
   engines.
3. Psionic owns reusable compute execution substrate, not canonical market
   settlement.
4. Kernel plus Nexus own market authority, not Psionic ordered state.
5. Validator services may execute challenges, but canonical economic outcomes
   remain authority mutations with receipts.
6. Environment and eval services may supply inputs, evidence, and registries,
   but compute-market obligations still terminate in kernel-owned truth.
7. New compute surfaces must be reviewed against this ADR before new crate or
   app boundaries are accepted.

## Consequences

- The repo now has a stable answer for where Prime/PI-inspired compute features
  should land.
- Future compute PRs can be rejected for owner drift without re-litigating the
  whole architecture.
- Psionic can expand aggressively without becoming a stealth settlement layer.
- Desktop can remain the visible product wedge without swallowing the reusable
  compute stack.
- Validator and environment/eval work now have explicit landing zones before
  they exist as crates or services.
