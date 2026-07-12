# AC-1 owner-capacity charge-disposition receipt

Issue: [#8719](https://github.com/OpenAgentsInc/openagents/issues/8719)  
Parent: [#8547](https://github.com/OpenAgentsInc/openagents/issues/8547)

## Outcome

An Agent Computer can no longer accept an owner-subscription-capacity usage
receipt merely because the usage endpoint returned HTTP 200. For the exact
Codex and Claude owner-capacity lane/provider pairs, the consumer now requires
both:

- `tokenChargeMetered: false`; and
- `tokenChargeSkippedReason: owner_subscription_capacity`.

A metered receipt, an omitted reason, or another reason returns the stable
public-safe failure `owner_capacity_charge_disposition_invalid`. The Codex
workroom turn maps that to
`codex.owner_capacity_charge_disposition_invalid`, so it cannot emit a
successful turn or accepted closeout after an unexpected customer charge.
The result includes no bearer, provider credential, or raw server response.

Hosted provider-capacity receipts are outside this no-charge contract and
retain their normal metered-success behavior.

## Verification

- `bun test tests/agent-computer-turn-runner.test.ts`: 53 passed, 0 failed,
  183 assertions.
- `bun run typecheck` in `apps/pylon`: passed.
- The corpus covers successful Codex owner capacity, unexpectedly metered
  Codex, missing-reason Codex, mismatched-reason Claude, ordinary hosted
  metering, and public-safe failure projection.
- `git diff --check`: passed before publication.

This closes the unexpected-overbilling failure-mode slice. It does not claim
the separate broker grant-authority proof or #8547's live Firecracker/mobile
acceptance.
