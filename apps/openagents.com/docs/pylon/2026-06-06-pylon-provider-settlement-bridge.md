# Pylon Provider Assignment And Settlement Bridge

Date: 2026-06-06

Status: implemented contract note for issue #339 / `OPENAGENTS-RUST-006`.

## Purpose

OpenAgents product surface now has a schema-first bridge for Pylon provider assignments and
settlement projections.

The implementation lives in
`workers/api/src/pylon-settlement-bridge.ts`.

This is a contract and projection layer only. It does not spend bitcoin,
dispatch payout, mutate payout targets, charge buyers, settle provider work,
or operate a live wallet.

## Bridge Model

`OpenAgentsPylonSettlementBridgeRecord` records:

- provider assignment refs;
- provider job refs;
- capability snapshot refs;
- wallet readiness summary refs;
- buyer payment evidence refs;
- accepted-work refs;
- reward-intent refs;
- payout-eligibility refs;
- payout-dispatch refs;
- payout-confirmation refs;
- payout-verification refs;
- settlement refs;
- blocker refs;
- caveat refs;
- evidence refs;
- operator diagnostic refs; and
- workroom refs.

The bridge keeps these states separate:

- assignment;
- capability snapshot;
- wallet readiness checked;
- buyer payment evidence;
- accepted work;
- provider reward intent;
- payout eligibility;
- payout dispatch;
- payout confirmation;
- payout verification; and
- settlement.

That separation matters because evidence that a buyer paid is not the same as
accepted work, accepted work is not the same as a provider reward intent, and
a reward intent is not a bitcoin payout or settlement.

## Authority Boundary

The default authority block is
`OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY`.

It explicitly denies:

- live wallet spend;
- payout dispatch;
- payout-target mutation;
- buyer-charge mutation; and
- settlement mutation.

`openAgentsPylonSettlementBridgeCanMutateSettlement` returns false for records
using that authority block. The bridge can report what happened elsewhere; it
cannot make a payout happen.

## Bitcoin Terminology

Product and doc copy should say bitcoin. If an amount needs base-unit
clarification, it may also clarify the denomination as sats. This bridge does
not store raw invoice material, raw payment proofs, preimages, or private
channel state.

## Projection And Redaction

Public and agent projections can show safe assignment, job, capability,
accepted-work, reward-intent, and public settlement refs. They hide buyer
payment evidence, wallet-readiness refs, payout dispatch refs, payout
confirmation refs, payout verification refs, workroom refs, provider-private
refs, and operator diagnostics.

Customer projections can show more work context but still hide private payment
and provider internals. Operator/private projections can show safe internal
refs, but still reject raw secret, wallet, and payment material.

The contract rejects refs containing:

- private customer data;
- wallet material;
- raw bitcoin payment material;
- raw invoices;
- payment IDs, payment proofs, preimages, and hashes;
- payout addresses, payout destinations, and payout targets;
- private channel state and channel monitor material;
- provider secrets, provider payloads, and provider tokens;
- private repo material;
- raw logs, raw payloads, raw prompts, raw source archives, and raw emails; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Conformance Fixtures

`OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES` contains two safe
fixtures:

- a settled provider job bridge with full evidence refs; and
- a buyer-payment-only bridge proving that payment evidence alone does not
  allow accepted-work, reward, payout, or settlement claims.

The Rust/native conformance issue can mirror these fixtures when validating
`oa-node`, `oa-workroomd`, Probe, Psionic, and Pylon-side payload generation.

## Tests

`workers/api/src/pylon-settlement-bridge.test.ts` covers:

- schema/projection decoding;
- no-spend, no-dispatch, and no-settlement-mutation authority;
- buyer payment, accepted work, reward intent, payout eligibility, payout
  dispatch, payout confirmation, payout verification, and settlement
  separation;
- public redaction of provider, wallet-readiness, payment, payout, workroom,
  and operator diagnostic refs; and
- unsafe wallet, invoice, preimage, payout target, private channel, provider
  secret, private repo, raw payload, and raw timestamp rejection.
