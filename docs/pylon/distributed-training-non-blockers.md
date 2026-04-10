# Pylon Distributed Training Non-Blockers

This document is the deferred-scope register for the admitted-node distributed
training MVP.

Its job is narrow:

- make the explicitly deferred scope visible
- keep that scope out of MVP delivery unless an exit criterion requires it
- give later roadmap work a clean place to pick those items back up

This is not a backlog of "nice to have" ideas. It is a scope-control document.
If an item is listed here, it is not an MVP blocker for the first honest
admitted-node run.

## Rule

Features outside the frozen Phase 0 contracts do not enter the MVP by
opportunistic implementation unless they are required to satisfy an exit
criterion.

If a team believes a non-blocker has become blocking, the burden is to update
the roadmap and phase tracker explicitly rather than quietly folding the work
into MVP delivery.

## Deferred Scope Buckets

### Multi-Backend Portability Beyond The First Honest Shape

These are intentionally out of MVP scope:

- mixed CUDA plus Apple windows
- backend-portable checkpoint artifacts across heterogeneous active windows
- any scheduler mode that weakens the backend-homogeneous window rule

Why deferred:

- the first honest contract is one backend-homogeneous window at a time
- portability work should not destabilize the MVP acceptance, replay, and
  artifact contracts

Future home:

- follow-on work after Phase 6 when the Apple rehearsal matrix is already
  passing for backend-homogeneous windows

### Stronger Window Seals And External Anchoring

These are intentionally out of MVP scope:

- threshold-signed window seals
- Bitcoin anchoring for windows, randomness, or closeouts
- stronger external notarization layers beyond the signed TRN trail

Why deferred:

- the MVP needs a defensible admitted-node audit trail, not a fully anchored
  adversarial finality system

Future home:

- post-MVP trust-hardening work after the admitted-node loop is already live

### Permissionless Admission And Market Economics

These are intentionally out of MVP scope:

- permissionless admission
- stake or bond mechanics
- open validator markets
- automatic slashing economics

Why deferred:

- the MVP assumes admitted nodes, admitted builds, and authority-driven
  scheduling rather than open economic admission
- introducing stake and slashing mechanics before the control plane is stable
  would distort delivery priority and failure analysis

Future home:

- later market design work after the admitted-node runtime is operationally
  stable and observable

### Blockchain-Level Consensus And Hostile-Network Verifiability

These are intentionally out of MVP scope:

- blockchain-level consensus or finality
- generalized hostile-network verifiability
- any public claim that the first admitted-node system is trustless against a
  fully adversarial validator or provider set

Why deferred:

- the current MVP target is a technically defensible admitted-node system with
  receipts, replay, labels, and audit trails
- hostile-network guarantees require a different verification and incentive
  posture than the one frozen for the first admitted run

Future home:

- separate roadmap work after the admitted-node control loop, evidence model,
  and provider runtime are proven in production

## Operational Guardrails

- Do not add issue dependencies from this document into the MVP phase tracker
  unless the roadmap itself changes.
- Do not use "this would be easy" as a reason to widen MVP scope.
- Do not present deferred work as implied by the current launch claims.
- When a deferred item is revisited, create a new issue in the appropriate repo
  and link it from the later roadmap phase or follow-on audit that actually
  owns it.

## Current Deferred Set

The current explicit non-blockers for the first honest admitted-node run are:

- mixed CUDA plus Apple windows
- threshold-signed window seals
- Bitcoin anchoring for windows or randomness
- permissionless admission
- stake or bond mechanics
- fully open validator markets
- automatic slashing economics
- blockchain-level consensus or finality
- generalized hostile-network verifiability
