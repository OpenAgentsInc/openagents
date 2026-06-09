# Investor Ops Workroom Template

Date: 2026-06-06

Status: implemented contract note for issue #342 / `OPENAGENTS-BIZ-002`.

## Purpose

OpenAgents product surface now has a schema-first investor ops workroom contract for investor prep,
data-room tasks, deck/video work orders, follow-up queues, decision receipts,
and accepted outcome refs.

The implementation lives in
`workers/api/src/omni-investor-ops-workrooms.ts`.

This is a contract and projection layer only. It does not send outreach,
publish decks or videos, upload private data-room material, mutate investor
records, or mutate accepted outcomes.

## Template Model

`OmniInvestorOpsTemplate` records:

- template ref;
- version ref;
- approval policy refs;
- data-room policy refs;
- closeout requirement refs;
- evidence requirement refs;
- proof policy refs;
- required artifact refs; and
- caveat refs.

The default fixture is `OMNI_INVESTOR_OPS_TEMPLATE_FIXTURE`.

## Workroom Model

`OmniInvestorOpsWorkroomRecord` records:

- investor refs;
- contact refs;
- source refs;
- prep packet refs;
- data-room task refs;
- deck work-order refs;
- video work-order refs;
- follow-up refs;
- decision receipt refs;
- acceptance refs;
- closeout refs;
- evidence refs;
- blocker refs;
- caveat refs; and
- operator diagnostic refs.

The state model keeps these steps separate:

- intake;
- prep packet ready;
- data-room task ready;
- creative work order ready;
- follow-up queued;
- decision receipt recorded;
- accepted outcome recorded;
- closed; and
- blocked.

That separation matters because prep is not a data-room update, a deck/video
work order is not publication, follow-up queueing is not outreach, a decision
receipt is not accepted outcome mutation, and closeout is not settlement.

## Authority Boundary

The default authority block is
`OMNI_INVESTOR_OPS_CONTRACT_ONLY_AUTHORITY`.

It explicitly denies:

- outreach send;
- deck or video publish;
- data-room upload;
- investor-record mutation; and
- accepted-outcome mutation.

`omniInvestorOpsAuthorityIsContractOnly` returns true only for records using
that full deny block.

## Projection And Redaction

Public and agent projections hide investor, contact, data-room, deck/video,
follow-up, decision, acceptance, source, workroom, and operator diagnostic refs.

Customer/team/operator projections can show safe refs appropriate to that
audience. Operator/private projections can show safe internal diagnostics.

The contract rejects refs containing:

- investor emails, investor names, contact emails, contact names, and phone
  values;
- raw or private data-room material;
- raw deck or video assets;
- provider accounts, grants, payloads, or tokens;
- private repo refs;
- raw source payloads;
- raw runner logs;
- secrets, bearer tokens, OAuth material, cookies, and API keys;
- wallet/payment material; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/omni-investor-ops-workrooms.test.ts` covers:

- schema/projection decoding;
- friendly time projection;
- contract-only authority;
- no outreach, creative publish, data-room mutation, investor mutation, or
  accepted-outcome mutation authority;
- public redaction of investor/contact/data-room/deck/video/follow-up/decision
  refs;
- prep, data-room, creative, follow-up, decision, accepted outcome, and
  closeout separation; and
- unsafe investor, contact, data-room, deck/video, provider, private repo,
  wallet/payment, raw log, and timestamp rejection.
