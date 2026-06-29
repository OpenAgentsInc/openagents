# Artanis Real Small-Bitcoin Assignment Smoke Evidence

Date: 2026-06-07
Issue: #438

## Summary

Issue #438 retained an Artanis-administered Pylon assignment smoke through the
OpenAgents product surface/Nexus authority path. The smoke moved 0.00001000 bitcoin (1,000
satoshis) from the OpenAgents treasury test wallet lane to a separate Pylon
edge test wallet lane through the MDK agent-wallet adapter boundary.

This is not a Pylon v0.2 release. It is the missing release-gate evidence item
for an Artanis-administered accepted-work assignment with real bitcoin
settlement evidence.

## Public Receipt

Public receipt page:

```text
https://openagents.com/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221
```

Public receipt API:

```text
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221
```

The public API should report:

- `movementMode: "real_bitcoin"`;
- `realBitcoinMoved: true`;
- `receiptKind: "settlement_recorded"`;
- `payoutMovement.terminalSettlementClaimAllowed: true`;
- `settlement.providerRef: "provider.public.mdk_agent_wallet"`;
- `settlement.stateLabel: "Settled"`.

## Retained Evidence Refs

Public-safe assignment and work refs:

- `smoke.public.issue_438.artanis_real_assignment.issue_438_artanis_1780822221`
- `assignment.public.issue_438.issue_438_artanis_1780822221`
- `accepted_work.public.issue_438_artanis_pylon_assignment`
- `artifact.public.issue_438.artanis_assignment_proof_manifest`
- `proof.public.issue_438.pylon_assignment.accepted_work`

Payment authority refs:

- `approval.public.issue_438.issue_438_artanis_1780822221`
- `payout_intent.issue_438.issue_438_artanis_1780822221`
- `payout_attempt.issue_438.issue_438_artanis_1780822221`
- `reconciliation.issue_438.issue_438_artanis_1780822221`
- `receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221`

Readiness and idempotency refs:

- `wallet_readiness.public.issue_438.treasury.minimum_satisfied`
- `wallet_readiness.public.issue_438.pylon.receive_ready`
- `idempotency.public.issue_438.intent.insert_or_ignore`
- `idempotency.public.issue_438.payment_authority.no_duplicate_spend`

Forum update evidence:

- `forum.public.artanis.nexus_pylon.release_gate_pass.issue_438_artanis_1780822221`

That Forum ref is a retained public-safe update intent for the Artanis
Nexus/Pylon Forum bridge. It is sufficient for the release gate evidence item;
it does not claim a human-facing Pylon release announcement.

## Storage Boundary

The live D1 ledger now contains public-safe rows for:

- payout target approval;
- payout intent;
- payout attempt;
- reconciliation event;
- intent, dispatch, confirmation, verification, and settlement receipts;
- the `artanis_real_assignment` release-gate evidence row.

The private operator command artifacts remain in an ignored local smoke
artifact directory. They are not tracked and are not needed to view the public
receipt.

## Redaction Rules

Do not commit, expose in public receipts, or put in issue comments:

- raw invoice;
- raw payment hash;
- preimage;
- mnemonic;
- wallet config;
- wallet home path;
- exact wallet balance;
- private payout target;
- provider access token;
- webhook secret;
- raw command stdout or stderr;
- customer data;
- operator-only notes.

The public evidence uses redacted refs only. Payment proof still is not
authority by itself; the smoke is accepted because it is linked to approval,
spend cap, wallet readiness, idempotency, dispatch, reconciliation, and
settlement receipt evidence.

## Release-Gate Impact

`workers/api/src/artanis-real-small-bitcoin-assignment-smoke.ts` retains the
typed smoke evidence and public/operator projection boundary.

`workers/api/src/pylon-v02-openagents-release-gate.ts` now marks
`artanis_real_small_bitcoin_assignment` as passed for the current OpenAgents product surface/Nexus
gate.

With #434 and #438 complete, the typed Pylon v0.2 OpenAgents product surface/Nexus release gate can
classify the evidence set as ready for operator release review. It still does
not publish a release, spend bitcoin, settle payouts, or upgrade public claims
by itself.
