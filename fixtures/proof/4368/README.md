## 4368 Proof Fixtures

This fixture corpus seeds the first local proof lanes with retained state and
request templates taken from the `#4368` debugging path instead of from
clean-room happy-path assumptions.

Current fixture set:

- `cs336_a1_stale_worker_lease_v1`
  - stale retained worker lease/window state that should not steal fresh claim
    priority once the proof fleet comes online
- `cs336_a1_stale_validator_lease_v1`
  - stale retained validator lease/window state for the same recovery class
- `cs336_a1_closeout_observe_payout_worker_v1`
  - reduced worker-side retained closeout state for the post-seal,
    payout-attention class seen in `proof.4388.run2`
- `cs336_a1_closeout_observe_payout_validator_v1`
  - reduced validator-side retained closeout state for the same payout-attention
    class
- `cs336_a1_replacement_attempt_contribution_v1`
  - contribution/reconcile template for the lease-fail -> replacement-claim ->
    seal/reconcile scenario from `#4368`

Source evidence:

- retained local proof namespaces:
  - `proof.4389.run3`
  - `proof.4389.run4`
  - `proof.4388.run1`
  - `proof.4388.run2`
- live replacement-attempt proof thread on `#4368`:
  - replacement-attempt seal fix comment:
    `https://github.com/OpenAgentsInc/openagents/issues/4368#issuecomment-4275397579`
  - live accepted closeout confirmation:
    `https://github.com/OpenAgentsInc/openagents/issues/4368#issuecomment-4275436962`

The `.template.json` files intentionally stay small and redact local machine
paths. The proof harness substitutes only the pieces that must match the live
namespace being exercised, such as `__PROOF_NETWORK_ID__`.
