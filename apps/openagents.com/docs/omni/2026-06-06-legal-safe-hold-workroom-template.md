# Legal Safe-Hold Workroom Template

Date: 2026-06-06

Status: implemented contract note for issue #344 / `OPENAGENTS-BIZ-004`.

## Purpose

OpenAgents product surface now has a schema-first legal safe-hold workroom contract for
legal-sensitive orders that must be held, scoped, source-backed, and routed to
human/legal review before anything operational happens.

The implementation lives in
`workers/api/src/omni-legal-safe-hold-workrooms.ts`.

This is a contract and projection layer only. It does not execute legal work,
send anything externally, file anything, claim legal advice, settle payment, or
upgrade a private/legal projection into a public projection.

## Template Model

`OmniLegalSafeHoldTemplate` records:

- template ref;
- version ref;
- approval policy refs;
- hold policy refs;
- scoping requirement refs;
- source requirement refs;
- legal review requirement refs;
- release policy refs;
- closeout requirement refs;
- evidence requirement refs;
- proof policy refs;
- required artifact refs; and
- caveat refs.

The default fixture is `OMNI_LEGAL_SAFE_HOLD_TEMPLATE_FIXTURE`.

## Workroom Model

`OmniLegalSafeHoldWorkroomRecord` records:

- client refs;
- matter refs;
- jurisdiction refs;
- source refs;
- scoping refs;
- legal-review refs;
- hold refs;
- release refs;
- decline refs;
- closeout refs;
- evidence refs;
- blocker refs;
- caveat refs; and
- operator diagnostic refs.

The state model keeps these steps separate:

- intake;
- safe hold recorded;
- scoping recorded;
- source-backed summary ready;
- legal review requested;
- legal review recorded;
- released;
- declined;
- closed; and
- blocked.

That separation matters because a hold is not scoping, scoping is not a
source-backed summary, a source-backed summary is not legal review, legal
review is not release, release is not filing or external send, and closeout is
not payment settlement.

## Authority Boundary

The default authority block is
`OMNI_LEGAL_SAFE_HOLD_CONTRACT_ONLY_AUTHORITY`.

It explicitly denies:

- automatic execution;
- external send;
- filing;
- legal advice claims;
- payment settlement; and
- public projection upgrade.

`omniLegalSafeHoldAuthorityIsContractOnly` returns true only for records using
that full deny block.

## Projection And Redaction

Public and agent projections hide client, matter, jurisdiction, source,
scoping, legal-review, hold, release, decline, closeout, workroom, and operator
diagnostic refs.

Customer/team/operator projections can show safe refs appropriate to that
audience. Operator/private projections can show safe hold, legal-review,
release, decline, closeout, and diagnostic refs.

The contract rejects refs containing:

- legal-sensitive data;
- client identity;
- matter data;
- privileged or confidential refs;
- raw legal documents;
- raw filing material;
- raw source payloads;
- provider accounts, grants, payloads, or tokens;
- private repo refs;
- raw runner logs;
- secrets, bearer tokens, OAuth material, cookies, and API keys;
- wallet/payment material; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/omni-legal-safe-hold-workrooms.test.ts` covers:

- schema/projection decoding;
- friendly time projection;
- contract-only authority;
- no automatic execution, external send, filing, legal advice claim, payment
  settlement, or public projection upgrade authority;
- public redaction of client/matter/source/review/hold/release/decline/
  closeout/workroom refs;
- hold, scoping, source summary, legal review, release, decline, and closeout
  separation; and
- unsafe client, matter, confidential, privileged, raw document, provider,
  private repo, wallet/payment, raw log, and timestamp rejection.
