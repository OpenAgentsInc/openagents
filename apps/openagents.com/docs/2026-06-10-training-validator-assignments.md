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
