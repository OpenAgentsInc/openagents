# Apple Plus NVIDIA Mixed-Cluster Experiment Audit

> Status: audit for GitHub issue `#3662`, written on 2026-03-15 after
> reviewing `crates/psionic/docs/TRAIN_SYSTEM.md`,
> `crates/psionic/docs/ARCHITECTURE_EXPLAINER_CLUSTER_BRINGUP_RUNBOOK.md`,
> `docs/audits/2026-03-15-psionic-architecture-explainer-simulated-cluster-addendum.md`,
> `crates/psionic/psionic-train/src/adapter_reference_program.rs`, and
> `crates/psionic/psionic-train/src/open_adapter.rs`.

## Why This Audit Exists

The repo now has:

- a real single-host Apple adapter operator lane
- a bounded non-Apple open adapter backend
- a simulated-cluster reference program that already exercises both the Apple
  family and the first open-backend family under one control-plane vocabulary

What was still underspecified before this audit was the first honest
heterogeneous experiment shape.

The mixed Apple plus NVIDIA path is not just "same run, different boxes."
Apple and NVIDIA participate for different reasons and through different
backend families.

This audit freezes the first truthful mixed-backend experiment boundary.

## The First Honest Mixed Topology

The first realistic heterogeneous experiment should be a mixed-role topology:

- Apple host:
  - cluster coordinator
  - authority-facing run owner
  - Apple runtime-validation host
  - final `.fmadapter` export and attach host
  - optional local Apple executor
- NVIDIA host:
  - first concrete non-Apple open-backend executor
  - artifact contributor for the mixed cluster rehearsal
  - throughput-oriented open-backend batch execution node

This is not a symmetric Apple-training story.

The NVIDIA participant is not there to pretend it is running the Apple
Foundation Models backend. It is there as the first truthful non-Apple worker
family under the same cluster, artifact, validator, and replay vocabulary.

## Canonical Backend Split

The mixed experiment should keep the backend boundary explicit:

- Apple adapter lane backend:
  - `apple.foundation_models.adapter_train`
- first non-Apple mixed participant backend:
  - `open_adapter_backend.cuda.gpt_oss_lm_head`

That second backend label is important because it names the first concrete
heterogeneous target instead of hiding behind generic "open backend" language.

The first mixed experiment is therefore:

- Apple-specific on final runtime validation and final export
- NVIDIA-specific on the first concrete non-Apple contributor target
- shared only at the cluster, worker, artifact, validator, and receipt layers

## What Is Already Implemented Enough To Support This Definition

The repo already has enough landed work to define the mixed experiment
honestly:

- `#3661` runbook now documents the heterogeneous Apple-plus-NVIDIA bring-up
  path and warns against overclaiming Apple-valid mixed execution
- `#3660` simulated-cluster addendum now includes the heterogeneous proxy via
  the open-backend reference harness and explicitly states that this is not yet
  evidence of mixed Apple-valid acceleration
- `psionic-train` already exposes both the Apple reference family and the
  open-backend reference family under the same decentralized-adapter reference
  program
- `psionic-train` already exposes the CUDA backend label for the first open
  participant target

## Role And Receipt Rules

The first heterogeneous experiment must preserve backend truth in every
operator-facing receipt.

That means:

- worker registration must keep backend labels explicit
- contributor selection must remain capability-aware rather than manually
  overridden
- artifact manifests must preserve which backend family produced the artifact
- validator and replay posture must explain backend-specific disagreement
  instead of collapsing everything into generic worker failure

The first mixed experiment only counts if an operator can answer:

- which worker was Apple-specific?
- which worker was the NVIDIA open-backend participant?
- which artifacts came from which backend family?
- which phase still required the Apple host even after the NVIDIA participant
  contributed?

## Scheduling Guidance For Uneven Hardware

The first mixed experiment should assume uneven throughput and uneven
responsibility.

Apple host characteristics:

- required for final bridge validation
- required for final `.fmadapter` export or attach
- likely lower-throughput than a strong NVIDIA host on pure open-backend batch
  work

NVIDIA host characteristics:

- useful for open-backend throughput-oriented work
- useful for stressing artifact movement and replay posture under mixed
  hardware
- not a substitute for final Apple runtime validation

Operator scheduling rule:

- treat the Apple host as the correctness anchor
- treat the NVIDIA host as the first concrete high-throughput non-Apple
  participant
- do not flatten them into one fake "GPU pool"

## Compatibility Boundary

The compatibility boundary for the first mixed experiment is strict.

Apple-specific:

- live Foundation Models runtime behavior
- final `.fmadapter` asset loading
- final attach or detach validation
- final acceptance of Apple runtime-smoke truth

Open-backend-specific:

- the first non-Apple contributor target
- the first CUDA or NVIDIA participant backend label
- open adapter artifact production and batch execution semantics

Shared across both:

- cluster admission and topology truth
- contributor selection vocabulary
- worker heartbeats and assignment receipts
- artifact staging and manifest lineage
- validator dispositions
- replay or quarantine posture

## What This Issue Can Honestly Close On

This issue can close once the mixed experiment is defined clearly enough that:

- the runbook names the Apple-plus-NVIDIA topology
- the simulated-cluster addendum includes the heterogeneous proxy
- the canonical train spec points at the mixed experiment boundary
- the open adapter backend is explicitly documented as the first NVIDIA or CUDA
  participant target

This issue does not require pretending that live mixed Apple-valid training is
already implemented.

## Remaining Follow-On Beyond This Audit

Still later:

- real mixed-device execution and scheduling behavior under live load
- real artifact interchange under non-simulated multi-host execution
- proof that a mixed Apple plus NVIDIA topology can shorten a real end-to-end
  run rather than only enrich cluster posture

That remaining work is execution work, not naming work. This audit closes the
naming and boundary problem first.
