# ARC Claim Vocabulary
Status: canonical claim vocabulary for the ARC subtree
Date: 2026-03-15
Audience: coding agents and human maintainers working in `crates/arc/*`,
`crates/psionic/*`, and `docs/*`.

## Purpose

This document freezes the claim vocabulary used by the ARC subtree roadmap,
acceptance artifacts, and future benchmark reports.

The goal is to stop three kinds of drift:

- claim drift between the roadmap, spec, and acceptance artifacts
- overclaiming from one-off demos or partial ports
- vague "ARC progress" language that hides which layer is actually green

This document is intentionally compact. It is the canonical definition of the
five ARC subtree claims; other ARC docs may summarize them, but they should not
silently redefine them.

## Canonical Claims

The ARC subtree uses exactly five progressively stronger claims:

1. `contracts-real`
2. `benchmark-real`
3. `solver-real`
4. `interactive-real`
5. `research-credible`

These claims are ordered. A later claim does not erase the requirements of an
earlier one.

## Claim Definitions

### `contracts-real`

This means the shared ARC contracts are frozen enough that multiple crates can
build against one stable owner for:

- task, grid, action, frame, scorecard, and recording schemas
- canonicalization and object or relation views
- budget, result, and refusal envelopes
- dataset manifests and split metadata

This claim is invalid if ARC crates still duplicate owner types, rely on
unstable task IDs or hashes, or leave refusal behavior implicit.

### `benchmark-real`

This means the benchmark runtime is truthful enough that score claims come from
the ARC subtree itself rather than from ad hoc external glue.

Minimum meaning:

- deterministic local engine parity exists
- compatibility-server and REST behavior are typed and tested across
  `Offline`, `Online`, and `Competition`
- exact-match static scoring and versioned interactive scoring are computed by
  `arc-benchmark`
- recordings, checkpoints, replay bundles, scorecard lifecycle, and
  session-affine online runs survive restart and resume

This claim is invalid if score is still computed outside `arc-benchmark`,
hidden environment assumptions are required, or score-policy versioning drifts.

### `solver-real`

This means the static ARC solver is more than prompt wrapping or a one-off
search anecdote.

Minimum meaning:

- the typed DSL and interpreter exist
- verifier and falsifier logic exist
- at least one symbolic lane and one non-symbolic lane exist
- traces, budgets, candidate identity, and attempt policy are explicit
- portfolio results are reported against internal hidden holdout rather than
  only public-facing examples

This claim is invalid if there is no replay, no verifier, no attempt-policy
truth, or no hidden-holdout reporting.

### `interactive-real`

This means ARC-AGI-3 work is honest about game-client, action, and scorecard
truth rather than collapsing those concepts into prompt text.

Minimum meaning:

- the agent trait and runner exist
- local and remote game-client parity exists
- action budgets, resets, terminal transitions, and refusal semantics are
  explicit
- recordings, scorecards, and trajectories are replayable
- baseline agents are reproducible

This claim is invalid if interactive runs still depend on ad hoc game loops,
opaque action semantics, or non-replayable score behavior.

### `research-credible`

This means ARC model work is honest rather than aspirational.

Minimum meaning:

- evaluator parity exists before model claims
- small model train/eval loops run on Psionic substrate
- search-guide and trace-derived learning loops are real
- HRM-class claims remain gated on explicit Psionic readiness
- metrics, checkpoints, and failure semantics are auditable

This claim is invalid if large-scale training anecdotes substitute for bounded
parity fixtures or if ARC-specific substrate is silently hidden inside Psionic.

## Required Artifact Families

Each claim must map to at least one concrete artifact family.

| Claim | Minimum artifact family |
| --- | --- |
| `contracts-real` | schema fixtures and deterministic serialization fixtures |
| `benchmark-real` | exact-match and interactive score parity harnesses plus replay fixtures |
| `solver-real` | trace-bundle corpus, determinism fixtures, and hidden-holdout reports |
| `interactive-real` | replayable trajectory packs plus local-vs-remote parity fixtures |
| `research-credible` | checkpoint artifacts, train/eval logs, and bounded parity summaries |

Code without a matching artifact family is not enough to justify the claim.

## Usage Rules

- New ARC docs SHOULD reference these exact claim names.
- New acceptance matrices MUST use these exact claim names or explicitly state
  why they are narrower than this vocabulary.
- Later ARC docs MAY summarize the claims, but they MUST NOT redefine them.
- If a future change needs a sixth claim or a rename, this document MUST be
  updated in the same change that introduces it.
