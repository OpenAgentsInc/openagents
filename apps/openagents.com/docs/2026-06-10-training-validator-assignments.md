# Training Validator Assignments

Issue: `#4676`

The training verification queue can now be bridged into Pylon validation
assignments without bypassing the existing Pylon assignment rail.

## Dispatch Contract

- Validator assignments use `jobKind: "validation"`.
- Validators must advertise `capability.public.training_verification`.
- Assignment selection is dispatcher-controlled through
  `selection.public.training_validator.dispatcher_controlled`.
- Validator payment is marked `payable_pending_settlement`, but the bridge
  keeps `blocker.training_validator.operator_spend_approval_required` attached
  until an operator explicitly approves the small-sats settlement.
- Forum autopublish remains disabled for validator assignments.

## Self-Validation Guard

The bridge refuses to create an assignment when:

- the validator Pylon ref equals the worker Pylon ref; or
- the challenge contribution ref contains the validator Pylon ref.

Those blockers make the worker validating its own contribution structurally
impossible at the assignment-request boundary.

## Rejection Quorum

Verified verdicts can resolve with one validator. Freivalds/Merkle rejections
require two distinct validator Pylon refs before consensus is projected as
`consensus_rejected`. Duplicate verdicts from the same validator remain
`quorum_pending`.

Other verification classes still accept a single rejecting validator verdict,
because those checks are deterministic recompute, exact replay, statistical
threshold, or seeded replication failures rather than probabilistic Freivalds
spot checks.

## No-Spend Smoke

Run from `apps/openagents.com/workers/api`:

```sh
bun run smoke:training-validator:no-spend
```

The smoke covers assignment construction, Pylon public projection, the
self-validation blockers, and the Freivalds rejection quorum without creating a
wallet payment.

## Live Paid Closeout (2026-06-11)

The first paid weak-device validator closeout ran end to end on
production: validator Pylon `pylon.4f4ef3d029e57674be98` (fresh home,
registered with `capability.public.training_verification`) claimed
`training.verification.challenge.8a74a531-8b0d-4392-a49d-ede5179f23f7`,
independently re-executed the `freivalds_merkle` class over the #4675
worker contribution, closed out through assignment
`assignment.public.training_validator.recheck_20260611053500`, and was
paid 30 sats with public settled receipt
`receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500`.
The self-validation guard was also exercised live: a dispatch attempt
with validator == worker was blocked before any request reached the
Worker. Full ref bundle and named gaps:
`docs/2026-06-11-training-validator-paid-closeout-evidence.md`.

Live tooling added for this lane:

- `scripts/training-validator-live-verify.ts` — validator-side local
  re-execution and independent verdict (no network, no secrets).
- `scripts/training-validator-live-dispatch.ts` — operator dispatch that
  routes through `buildTrainingValidatorAssignmentRequest`, so the
  no-self-validation guard is enforced on the real dispatch path.
