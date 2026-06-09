# ADR-0004: Nexus Relay Core Versus Training Demo Control Boundary

- Status: Accepted
- Date: 2026-04-16
- Owners: runtime + infra + contracts-docs
- Supersedes: none
- Related: `docs/deploy/NEXUS_HOTFIX_LANE.md`, `docs/deploy/NEXUS_GCP_RUNBOOK.md`, `apps/nexus-relay/src/durable.rs`, `apps/nexus-control/src/lib.rs`

## Context

The April 15 Nexus iteration-speed audit showed the hotfix lane problem at two
different levels:

1. the build and deploy path was too slow because Cloud Build image production
   was still the required hotfix lane
2. the deploy unit was too large because routine `nexus-relay` fixes still
   compiled training-class code through `nexus-control`

The first problem is handled by the warm-builder and binary-first deploy lane.
The second problem needs a boundary decision before more refactors land.

The pre-extraction wiring was:

- `apps/nexus-relay/src/durable.rs` owns the durable relay shell, public relay
  homepage, websocket upgrade path, health surface, and upstream relay proxy
- that relay shell still merges `nexus_control::build_api_router(...)` directly
  into the same process
- `apps/nexus-control/src/lib.rs` owns the in-process authority API, including
  treasury, provider presence, training status, and admin launch routes
- `apps/nexus-control/Cargo.toml` depended directly on
  `psionic-train`
- that direct dependency is used for a narrow machine-lane contract surface:
  lane ids plus `PsionicTrainLaneContract::for_lane(...)`

The expensive consequence was visible in the pre-extraction build graph:

- `cargo tree -p nexus-relay` included `psionic-train`
- that pulls additional training-class crates such as `psionic-cluster`,
  `psionic-eval`, `psionic-router`, and the heavier Psionic runtime graph into
  routine relay hotfix builds
- the current relay hotfix loop therefore pays for training runtime compile
  cost even when the fix is in relay shell or authority code

That is the wrong boundary.

## Decision

The deployed Nexus runtime stays one combined process for now, but the code and
build boundary must split into two explicit layers.

### 1. Relay core stays narrow

`apps/nexus-relay` is the relay-core layer. It owns:

- durable upstream relay supervision
- public relay shell routes
- websocket proxy and NIP-11 relay identity behavior
- local health surface
- release packaging and VM activation target

Relay core must not take direct responsibility for:

- CS336 demo launch semantics
- training bootstrap artifact publication
- machine-lane contract definitions
- training scheduler policy details

### 2. Training and demo control remain in `nexus-control`

`apps/nexus-control` remains the owner of:

- admin launch routes
- training summary and run-detail authority surfaces
- training scheduler and registry mutation behavior
- treasury and provider-presence authority behavior

The service boundary does not move yet. `nexus-control` is still allowed to run
in-process with `nexus-relay`.

### 3. The first extraction is a contract seam, not a service split

The first required extraction is:

- move the canonical Psionic lane ids and lane-contract table used by Nexus out
  of `psionic-train`
- host that surface in a small local contract crate inside this repo
- make `nexus-control` depend on that contract crate instead of the full
  `psionic-train` runtime crate

This first cut is intentionally narrow. It does not move all training logic out
of `nexus-control`. It removes the specific training-runtime dependency that
makes routine relay hotfixes behave like training-system rebuilds.

### 4. Service decomposition is explicitly later work

After the contract extraction lands and the hotfix lane is proven stable, later
work may split fast-changing training and demo-control surfaces into a separate
deployed unit if the combined process still causes operational drag.

That question is deferred.

## First Extraction Target

The smallest first extraction that materially reduces the relay hotfix build
graph is:

- a local crate that exports the canonical lane ids for:
  - actual pretraining
  - CS336 A1 demo
  - Apple windowed training
- the canonical lane-contract mapping:
  - `lane_id`
  - `release_id`
  - `environment_ref`
  - `backend_family`
  - `topology_class`
  - `minimum_machine_class`

The initial Nexus callsites that should use that crate are:

- environment-ref to backend-family mapping in `nexus-control`
- CS336 demo run launch
- CS336 demo bootstrap artifact publication
- CS336 registry/environment package materialization

## Consequences

1. The hotfix lane no longer waits on a broad runtime rewrite before getting a
   materially smaller relay build graph.
2. `nexus-relay` remains operationally simple while the code boundary becomes
   explicit.
3. Future review can reject new `psionic-train` imports in Nexus hotfix code
   unless there is a clear reason to widen the boundary again.
4. `apps/pylon` and other training-oriented surfaces may continue using the
   full `psionic-train` runtime where that dependency is actually justified.
