# Pylon Provider Job Lifecycle Records

Date: 2026-06-06

Status: implemented contract note for GitHub issue #321 / `OPENAGENTS-074`.

## Purpose

Provider jobs need a lifecycle that can talk about work moving forward without
collapsing accepted work, reward intent, payout dispatch, confirmation,
verification, and settlement into the same claim.

The implementation lives in `workers/api/src/pylon-provider-job-lifecycle.ts`.

## Stages

The v1 stage model is:

- `offered`;
- `assigned`;
- `running`;
- `artifact_produced`;
- `accepted`;
- `reward_intent_recorded`;
- `payout_dispatched`;
- `payout_confirmed`;
- `payout_verified`;
- `settled`;
- `blocked`;
- `failed`;
- `cancelled`.

The model deliberately avoids saying a provider has been paid merely because a
buyer payment, an acceptance, or a reward intent exists.

## Required Evidence

Lifecycle advancement requires the corresponding safe refs:

- `artifact_produced` requires artifact refs;
- `accepted` requires acceptance refs;
- `reward_intent_recorded` requires reward-intent refs;
- `payout_dispatched` requires payout-dispatch refs;
- `payout_confirmed` requires payout-confirmation refs;
- `payout_verified` requires payout-verification refs;
- `settled` requires settlement refs and payout-verification refs;
- `blocked` requires blocker refs.

## Projection Rules

Public projection can show public-safe job, offer, assignment, run, artifact,
acceptance, reward-intent, caveat, blocker, and evidence refs. It hides
workroom refs, buyer payment evidence, payout refs, and settlement refs.

Customer and team projections can see more work context where the surrounding
product surface authorizes it, but they still do not receive buyer payment
evidence or private provider settlement internals.

Operator projection can see safe buyer-payment, payout-dispatch, confirmation,
verification, and settlement refs. It still rejects raw secrets, raw payment
material, raw wallet material, raw runner logs, provider tokens, and customer
private data.

## Claim Flags

The projection exposes separate booleans:

- `acceptedWorkClaimAllowed`;
- `rewardIntentClaimAllowed`;
- `payoutDispatchClaimAllowed`;
- `settlementClaimAllowed`.

These are intentionally separate so UI copy, public proof, and future
accounting receipts cannot treat accepted work as settled provider payout.

## Tests

`workers/api/src/pylon-provider-job-lifecycle.test.ts` covers:

- settled lifecycle projection;
- public/operator projection splits;
- accepted/reward/payout/settlement claim separation;
- required refs for advanced stages;
- private provider redaction;
- raw timestamp omission;
- rejection of raw payout targets, payment IDs, wallet refs, invoices,
  preimages, provider tokens, runner logs, and customer refs.
