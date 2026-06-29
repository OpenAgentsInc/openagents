# Support And Project Ops Workroom Templates

Date: 2026-06-06

Status: implemented contract note for issue #343 / `OPENAGENTS-BIZ-003`.

## Purpose

OpenAgents product surface now has schema-first support and project ops workroom contracts for
customer issue handling, project status work, escalation tracking, decisions,
risks, status reports, and closeout receipts.

The implementation lives in
`workers/api/src/omni-support-project-ops-workrooms.ts`.

This is a contract and projection layer only. It does not send support
responses, mutate customer records, mutate external project-management
systems, create external escalations, or mutate accepted outcomes.

## Template Model

`OmniSupportProjectOpsTemplate` records:

- template ref;
- version ref;
- approval policy refs;
- closeout requirement refs;
- evidence requirement refs;
- proof policy refs;
- required artifact refs; and
- caveat refs.

The default fixtures are:

- `OMNI_SUPPORT_OPS_TEMPLATE_FIXTURE`; and
- `OMNI_PROJECT_OPS_TEMPLATE_FIXTURE`.

## Workroom Model

`OmniSupportProjectOpsWorkroomRecord` records:

- customer refs;
- ticket refs;
- source refs;
- issue timeline refs;
- proposed response refs;
- escalation refs;
- project task refs;
- decision refs;
- risk refs;
- status report refs;
- receipt refs;
- closeout refs;
- evidence refs;
- blocker refs;
- caveat refs; and
- operator diagnostic refs.

The state model keeps these steps separate:

- intake;
- issue timeline reconstructed;
- proposed response ready;
- escalation recorded;
- project task updated;
- decision recorded;
- risk recorded;
- status report ready;
- receipt recorded;
- closed; and
- blocked.

That separation matters because a reconstructed issue timeline is not a
customer response, a proposed response is not a sent response, an escalation
record is not external escalation, a project task update is not external system
mutation, and a receipt is not accepted-outcome settlement.

## Authority Boundary

The default authority block is
`OMNI_SUPPORT_PROJECT_OPS_CONTRACT_ONLY_AUTHORITY`.

It explicitly denies:

- support response send;
- project-management mutation;
- customer-record mutation;
- external escalation; and
- accepted-outcome mutation.

`omniSupportProjectOpsAuthorityIsContractOnly` returns true only for records
using that full deny block.

## Projection And Redaction

Public and agent projections hide customer, ticket, issue timeline, proposed
response, escalation, project task, decision, risk, status report, receipt,
source, workroom, and operator diagnostic refs.

Customer/team/operator projections can show safe refs appropriate to that
audience. Operator/private projections can show safe escalation, receipt, and
diagnostic refs.

The contract rejects refs containing:

- customer private data;
- raw support transcripts;
- raw ticket payloads;
- private ticket refs;
- proposed response raw bodies;
- provider accounts, grants, payloads, or tokens;
- private repo refs;
- raw source payloads;
- raw runner logs;
- secrets, bearer tokens, OAuth material, cookies, and API keys;
- wallet/payment material; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/omni-support-project-ops-workrooms.test.ts` covers:

- schema/projection decoding;
- friendly time projection;
- contract-only authority;
- no support-send, project-management mutation, customer-record mutation,
  external escalation, or accepted-outcome mutation authority;
- public redaction of customer/ticket/timeline/response/task/decision/risk/
  status/receipt/workroom refs;
- timeline, response, escalation, task, decision, risk, status, receipt, and
  closeout separation; and
- unsafe customer, ticket, transcript, provider, secret, wallet/payment,
  private repo, raw log, and timestamp rejection.
