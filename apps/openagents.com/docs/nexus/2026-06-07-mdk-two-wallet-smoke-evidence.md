# MDK Two-Wallet Smoke Evidence

Date: 2026-06-07
Related issue: #431

## Summary

OpenAgents product surface completed the first real two-wallet MDK bitcoin movement smoke for the
Nexus/Pylon payout path.

The run moved `1,000` bitcoin sats from an isolated OpenAgents treasury test
wallet to a separate isolated Pylon edge test wallet through the
`TreasuryPaymentAuthority` service and the `mdk_agent_wallet` adapter.

The run also wrote a public-safe OpenAgents product surface receipt chain into the remote D1 ledger.
The public receipt route reads those persisted records and projects only
redacted refs.

## Public Receipt

Receipt page:

`https://openagents.com/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507`

Receipt API:

`https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507`

Settlement receipt ref:

`receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507`

## Redacted Proof Refs

| Field | Public-safe ref |
| --- | --- |
| Amount | `amount.bitcoin.1000_sats` |
| Receive invoice digest | `invoice.redacted.mdk_agent_wallet.d3925764be764110143a11178b2841b5` |
| Payment digest | `payment.redacted.mdk_agent_wallet.ee263a124a8258e66f08985c68359b05` |
| Treasury wallet ref | `wallet.test.openagents_treasury` |
| Pylon wallet ref | `wallet.test.pylon_edge` |
| Provider ref | `provider.public.mdk_agent_wallet` |

## Authority Path

The smoke used the normal OpenAgents product surface authority path:

1. Create a fresh Pylon receive invoice inside the private executor boundary.
2. Store only the redacted invoice digest in the payout target approval.
3. Create a payout intent with a bounded bitcoin spend cap.
4. Reject duplicate payout-intent creation for the same idempotency key.
5. Dispatch through `TreasuryPaymentAuthority`.
6. Confirm a second dispatch with the same idempotency key returns the stored
   attempt and does not call the MDK wallet a second time.
7. Reconcile through the MDK payment history command.
8. Persist intent, attempt, reconciliation, and payment authority receipt
   records in D1.

The first duplicate-intent attempt was rejected with
`replayed_idempotency_key`. The second dispatch attempt was idempotent and did
not spend again.

## D1 Receipt Chain

The run wrote:

- one payout target approval;
- one payout intent;
- one payout attempt;
- one reconciliation event with `status = matched`;
- one `intent_created` receipt;
- one `dispatch_recorded` receipt;
- one `confirmation_recorded` receipt;
- one `verification_recorded` receipt; and
- one `settlement_recorded` receipt.

The public settlement receipt projects these public-safe fields:

- `movementMode: real_bitcoin`;
- `realBitcoinMoved: true`;
- `payoutMovement.terminalSettlementClaimAllowed: true`;
- `settlement.providerRef: provider.public.mdk_agent_wallet`; and
- `settlement.stateLabel: Settled`.

## Redaction Policy

The public and committed record set does not include:

- raw invoice;
- raw payment hash;
- preimage;
- mnemonic;
- wallet config;
- wallet home path;
- exact wallet balance;
- private payout target;
- provider access token;
- webhook secret; or
- private customer or operator data.

Private command stdout and stderr artifacts remain only in an ignored local
operator directory. They are not part of the repository, docs, GitHub issue
comments, public receipts, or D1 public projections.

## Verification

Local verification for the code path:

```sh
bun run --cwd workers/api test -- src/treasury-payment-mdk-agent-wallet-adapter.test.ts src/nexus-pylon-visibility-routes.test.ts
bun run --cwd workers/api typecheck
```

Remote D1 verification showed the settlement receipt row exists and the
matching reconciliation event has:

```text
status = matched
provider_ref = provider.public.mdk_agent_wallet
```

The public route was deployed and verified in production after Worker version
`ac23ecc4-27ea-489d-b77a-ad7122f703b2`. The receipt URL above now serves the
persisted real-bitcoin projection from the remote D1 ledger.
