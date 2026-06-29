# Model Lab Model Artifact Contract

Status: implemented for issue #380 / `OPENAGENTS-LAB-001`.

## Purpose

Model Lab needs to remember model and adapter work products without pretending
OpenAgents product surface can train, install, route, deploy, pay out, settle, or upgrade public
claims from the artifact record itself. The `ModelArtifact` contract records
identity, source refs, digest evidence, storage refs, provider refs, training
run refs, eval/benchmark refs, rights caveats, safety redaction, readiness,
and rollback posture as read-only evidence.

Implementation:

- `workers/api/src/omni-model-lab-model-artifact.ts`
- `workers/api/src/omni-model-lab-model-artifact.test.ts`

## Artifact Record

The record carries:

- artifact identity and kind;
- model family ref;
- provider, source, storage, training run, retained failure, eval,
  benchmark, adapter validation, promotion gate, and Model Lab loop refs;
- digest records with evidence refs and explicit no-raw-weight-copy posture;
- rights and license caveats;
- safety review and redaction policy refs;
- rollback refs and prior artifact refs;
- state and storage state; and
- read-only authority.

Projection timestamps use friendly labels and do not expose raw ISO strings.

## Authority Boundaries

Model artifacts cannot:

- start model training;
- install adapters;
- copy raw weights;
- promote runtime behavior;
- mutate routing;
- mutate payouts;
- mutate settlement; or
- upgrade public claims.

Those actions require separate server-authoritative workflows, approvals, and
receipts.

## Validation Rules

Artifacts require source refs, digest evidence, redaction policy refs, and
safe artifact refs. Validated, review-ready, and approved artifacts require
eval refs and safety review refs. Approved artifacts additionally require
benchmark refs, promotion gate refs, ready or verified rollback posture,
rollback refs, and prior artifact refs.

Redistribution can only be marked allowed when rights are open or
redistributable and license refs exist. Unknown rights cannot allow training
reuse.

## Projection Audiences

Supported audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public/customer/agent projections redact private artifact, digest, benchmark,
eval, model, provider, promotion gate, retained failure, rollback, safety,
source, storage, and training run refs as appropriate. Operator and team
projections can retain the safe ref set, but all projections reject raw
weights, provider payloads, prompts, private datasets, secrets, payment or
wallet material, private repositories, raw logs, source archives, and raw
timestamps.

## Tests

Coverage includes:

- reviewed artifact projection;
- readiness, rollback, storage, and rights labels;
- source, digest, eval, safety, benchmark, rights, and rollback validation;
- public redaction; and
- hard false training, adapter-install, raw-weight-copy, runtime-promotion,
  routing, payout, settlement, and public-claim mutation authority.
