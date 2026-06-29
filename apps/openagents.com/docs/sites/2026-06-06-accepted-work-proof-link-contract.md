# Accepted-Work Proof Links For Sites

Issue #358 / `OPENAGENTS-L-010` adds the receipt-link contract that lets
Site/order/public proof surfaces point at accepted provider work and
Pylon/Nexus/Treasury settlement evidence without becoming payout authority.

The implementation lives in
`workers/api/src/pylon-accepted-work-proof-links.ts`.

## Purpose

Sites need a public-safe way to say:

- this order or Site version produced accepted work;
- a provider job is associated with that accepted work;
- reward intent, payout eligibility, dispatch, confirmation, verification, or
  settlement evidence exists; and
- the user-facing Site/order/proof page can link to the relevant safe receipt
  refs.

The proof link is only a projection. It cannot accept work, charge a buyer,
dispatch a payout, expose a payout target, mutate provider eligibility, settle
a payment, or release a Site revision.

## Consumer Surfaces

The contract supports these consumer surfaces:

- `site_order`;
- `customer_dashboard`;
- `public_proof`;
- `operator_receipt`; and
- `agent_api`.

Each projection includes safe refs for the Site, order, version, public proof,
receipt link, provider job, accepted work, payout SLO, payout row, settlement
bridge, settlement evidence, caveats, and source records.

## Claim Separation

The projection keeps these states separate:

- accepted work;
- reward intent;
- payout eligibility;
- payout dispatch;
- payout confirmation;
- payout verification; and
- settled.

`settlementClaimAllowed` becomes true only when the link is in `settled` state
and includes settlement refs, settlement evidence refs, and payout verification
refs. Earlier states can be shown as useful progress, but they do not imply
settlement.

## Redaction

Public, customer, team, and agent projections redact private provider, job,
payout, SLO, settlement, Site, order, version, link, caveat, evidence, and
source refs according to audience.

All projections reject raw payout targets, wallet material, private channel
state, raw bitcoin payment material, invoices, preimages, provider secrets,
credentials, private repo refs, customer data, raw logs, and raw timestamps.

## Tests

`workers/api/src/pylon-accepted-work-proof-links.test.ts` covers:

- settled Site/order proof-link projection;
- read-only authority;
- public ref redaction;
- accepted-work, reward, eligibility, dispatch, verification, and settlement
  separation;
- required provider job and state evidence refs; and
- unsafe payout, wallet, payment, invoice, credential, channel, provider,
  customer, and timestamp material rejection.
