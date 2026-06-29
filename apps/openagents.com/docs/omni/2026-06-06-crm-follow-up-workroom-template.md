# CRM Follow-Up Workroom Template

Date: 2026-06-06

Status: implemented contract note for issue #341 / `OPENAGENTS-BIZ-001`.

## Purpose

OpenAgents product surface now has a schema-first CRM follow-up workroom contract for business
follow-up work that should share the Omni workroom model.

The implementation lives in
`workers/api/src/omni-crm-follow-up-workrooms.ts`.

This is a contract and projection layer only. It does not send email, mutate
CRM records, update relationship memory, execute external follow-up, or settle
accepted outcomes.

## Template Model

`OmniCrmFollowUpTemplate` records:

- template ref;
- version ref;
- approval policy refs;
- closeout requirement refs;
- evidence requirement refs;
- proof policy refs;
- required artifact refs; and
- caveat refs.

The default fixture is `OMNI_CRM_FOLLOW_UP_TEMPLATE_FIXTURE`.

## Workroom Model

`OmniCrmFollowUpWorkroomRecord` records:

- workroom ref;
- template ref;
- contact refs;
- company refs;
- source refs;
- prep packet refs;
- draft message refs;
- approval refs;
- send request refs;
- email receipt refs;
- relationship memory refs;
- closeout refs;
- evidence refs;
- blocker refs;
- caveat refs; and
- operator diagnostic refs.

The state model keeps these steps separate:

- intake;
- prep packet ready;
- draft prepared;
- approval requested;
- approval recorded;
- send prepared;
- email receipt recorded;
- relationship memory recorded;
- closed; and
- blocked.

That separation matters because a draft is not approval, approval is not an
email send, an email receipt is not relationship-memory mutation, and closeout
is not settlement.

## Authority Boundary

The default authority block is
`OMNI_CRM_FOLLOW_UP_CONTRACT_ONLY_AUTHORITY`.

It explicitly denies:

- email send;
- CRM mutation;
- external follow-up;
- relationship-memory mutation; and
- accepted-outcome settlement.

`omniCrmFollowUpAuthorityIsContractOnly` returns true only for records using
that full deny block.

## Projection And Redaction

Public and agent projections hide CRM operational refs, including contact,
company, source, draft, approval, send request, email receipt, relationship
memory, closeout, workroom, and operator diagnostic refs.

Customer/team/operator projections can show safe refs appropriate to that
audience. Operator/private projections can show safe internal email receipt,
send request, relationship memory, and diagnostic refs.

The contract rejects refs containing:

- raw emails or email bodies;
- contact emails, contact names, customer names, customer emails, or phone
  values;
- private contact data;
- provider accounts, grants, payloads, or tokens;
- raw source payloads;
- raw runner logs;
- secrets, bearer tokens, OAuth material, cookies, and API keys;
- wallet/payment material;
- private repo refs; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/omni-crm-follow-up-workrooms.test.ts` covers:

- schema/projection decoding;
- friendly time projection;
- contract-only authority;
- no email-send/CRM/relationship-memory mutation authority;
- public redaction of contact/company/source/draft/receipt/memory/workroom
  refs;
- draft, approval, send, receipt, closeout, and relationship-memory separation;
  and
- unsafe email, private contact, customer, provider, source, secret, wallet,
  payment, private repo, raw log, and timestamp rejection.
