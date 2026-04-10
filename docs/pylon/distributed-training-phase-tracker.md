# Pylon Distributed Training Phase Tracker

This document is the release-facing phase tracker for the distributed-training
MVP described in `docs/pylon/distributed-training-mvp-roadmap.md`.

It exists to make the roadmap executable:

- every implementation issue belongs to a named phase
- every phase has an explicit exit gate
- MVP launch is blocked until Phases 0 through 5 are complete
- Apple support is blocked until Phase 6 is complete

This tracker is a control document, not a second roadmap. If implementation
details drift, the roadmap remains the spec and this file is updated to reflect
the current phase status against that spec.

## Launch Gates

- MVP launch is blocked until every issue mapped into Phases 0 through 5 is
  closed and the full rehearsal matrix in
  `docs/pylon/PYLON_DISTRIBUTED_TRAINING_REHEARSAL_MATRIX.md` and
  `scripts/release/check-pylon-distributed-training-mvp.sh` is passing.
- Apple-capable public claims are blocked until every issue mapped into Phase 6
  is closed and the Apple rehearsal matrix in `openagents#4251` is passing.
- Features outside the frozen Phase 0 contracts do not enter the MVP by
  opportunistic implementation unless they are required to satisfy an exit
  criterion.

## Current Status

Status snapshot as of `2026-04-10`:

| Phase | Status | Open blockers | Gate |
| --- | --- | --- | --- |
| Phase 0 Contract Freeze | complete | none | satisfied |
| Phase 1 Psionic Under Pylon Supervision | complete | none | satisfied |
| Phase 2 Admitted Multi-Node Runtime | complete | none | satisfied |
| Phase 3 Nexus Windows And Validator Loop | complete | none | satisfied |
| Phase 4 TRN Publication And Reputation | complete | none | satisfied |
| Phase 5 Dress Rehearsal And Launch | complete | none | satisfied |
| Phase 6 Apple Silicon And Metal Support | in progress | `openagents#4248`, `openagents#4249`, `openagents#4250`, `openagents#4251` | blocked |

## Phase Ownership

| Phase | Primary owner repo | Main contributing repos |
| --- | --- | --- |
| Phase 0 | `openagents` | `psionic` |
| Phase 1 | `openagents` and `psionic` | `workspace` tracker only |
| Phase 2 | `psionic` and `openagents` | `workspace` tracker only |
| Phase 3 | `openagents` | `psionic` |
| Phase 4 | `openagents` | `psionic` |
| Phase 5 | `openagents` | `psionic`, `workspace` tracker only |
| Phase 6 | `psionic` and `openagents` | `workspace` tracker only |

## Phase 0 Contract Freeze

Exit gate:

- the sealed-window contract, topology limits, manifest schema, digest policy,
  validator policy, failure ownership rules, reputation policy, timers,
  observability contract, refusal taxonomy, and drift tests are all frozen in
  code and docs

Mapped issues:

- `openagents#4207` closed
- `openagents#4208` closed
- `openagents#4209` closed
- `openagents#4210` closed
- `openagents#4211` closed
- `openagents#4212` closed
- `openagents#4213` closed
- `openagents#4214` closed
- `openagents#4215` closed
- `openagents#4216` closed
- `openagents#4217` closed
- `openagents#4218` closed
- `openagents#4219` closed
- `openagents#4244` closed
- `openagents#4245` closed
- `openagents#4246` closed

Current assessment:

- complete

## Phase 1 Psionic Under Pylon Supervision

Exit gate:

- `Pylon` can launch one machine-stable `psionic-train` process, supervise it,
  publish the admitted capability truthfully, and send the minimum coordination
  traffic needed for a proving slice

Mapped issues:

- `psionic#902` closed
- `psionic#903` closed
- `openagents#4220` closed
- `openagents#4221` closed
- `openagents#4222` closed
- `openagents#4223` closed

Current assessment:

- complete

## Phase 2 Admitted Multi-Node Runtime

Exit gate:

- the admitted runtime can hold leases and membership, write and restore
  checkpoints, move artifacts durably, and survive restart without losing the
  canonical run state

Mapped issues:

- `psionic#904` closed
- `psionic#905` closed
- `psionic#906` closed
- `psionic#907` closed
- `openagents#4224` closed
- `openagents#4229` closed
- `openagents#4230` closed
- `openagents#4237` closed

Current assessment:

- complete

## Phase 3 Nexus Windows And Validator Loop

Exit gate:

- `Nexus` can plan, activate, seal, validate, reconcile, and close one live
  training window with real validator evidence and deterministic accepted
  outcomes

Mapped issues:

- `psionic#908` closed
- `openagents#4231` closed
- `openagents#4232` closed
- `openagents#4233` closed

Current assessment:

- complete

## Phase 4 TRN Publication And Reputation

Exit gate:

- node, window, verdict, artifact, and closeout truth is published
  authoritatively to TRN and scheduler preference consumes the emitted
  reputation labels

Mapped issues:

- `openagents#4225` closed
- `openagents#4234` closed
- `openagents#4235` closed
- `openagents#4239` closed
- `openagents#4240` closed
- `openagents#4241` closed
- `openagents#4242` closed
- `openagents#4243` closed

Current assessment:

- complete

## Phase 5 Dress Rehearsal And Launch

Exit gate:

- the end-to-end rehearsal matrix passes
- timeout, reconciliation, closeout, reputation, and restart-replay coverage
  are locked in CI
- operator and summary surfaces are sufficient to inspect the live system

Mapped issues:

- `psionic#909` closed
- `psionic#910` closed
- `psionic#911` closed
- `openagents#4226` closed
- `openagents#4227` closed
- `openagents#4228` closed
- `openagents#4236` closed
- `openagents#4238` closed
- `openagents#4247` closed

Current assessment:

- complete

## Phase 6 Apple Silicon And Metal Support

Exit gate:

- Apple workers can participate under the same manifest and policy contracts
- scheduler and publication surfaces keep backend families explicit
- the Apple rehearsal matrix passes before any dual-backend claim is made

Mapped issues:

- `psionic#912` closed
- `psionic#913` closed
- `openagents#4248` closed
- `openagents#4249` closed
- `openagents#4250` closed
- `openagents#4251` open

Current assessment:

- runtime support is in place in `psionic`
- `Pylon` now has admitted Apple capability detection, manifest acceptance, and
  node-record publication coverage
- `Nexus` now enforces backend-homogeneous worker and validator matching across
  shared Apple and CUDA training networks
- the publication path now keeps backend-family and environment identity
  explicit across shared Apple and CUDA TRN network, window, receipt, verdict,
  and artifact events without introducing new MVP kinds
- the remaining launch blocker is the Apple rehearsal issue

## Required Tracker Hygiene

- update this document when a phase-mapped issue opens, closes, or moves phase
- update `workspace#9` when a phase gate meaningfully changes
- do not mark a phase complete until the phase exit gate is satisfied, even if
  the linked issues are technically closed
