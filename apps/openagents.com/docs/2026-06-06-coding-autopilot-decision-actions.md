# Coding On Autopilot Decision Actions

Date: 2026-06-06

Status: implemented contract note for GitHub issue #314 / `OPENAGENTS-067`.

## Purpose

Coding on Autopilot missions need visible next actions. A returning operator,
customer, or agent should be able to see whether the next action is to
continue, steer, provide context, rerun tests, retry an account, stop, approve
a PR draft, request customer input, create a follow-up mission, or mark work
unavailable.

The implementation lives in
`workers/api/src/coding-autopilot-decision-actions.ts`.

## Contract

`CodingAutopilotDecisionActionRecord` captures:

- action kind;
- action status;
- mission ref;
- workroom refs;
- Program Run ref;
- assignment refs;
- route refs;
- account-lease refs;
- source-authority refs;
- action-submission refs;
- prerequisite refs;
- blocked reason refs;
- evidence and receipt refs;
- customer next-action ref;
- safe summary ref.

Statuses are `draft`, `recommended`, `available`, `blocked`, `completed`, and
`cancelled`.

## No Direct Effects

Decision actions are not execution authority. Every projected action carries:

- `directEffectPermitted: false`;
- `actionSubmissionRequired: true`.

That means a queue item cannot itself continue a run, deploy a Site, send an
email, create or approve a PR, rotate a provider account, post publicly, spend
money, or mutate customer state. It can only point to the next authorized
submission/approval path.

## Projection Rules

Public projections hide workroom, assignment, route, account-lease,
source-authority, and action-submission refs.

Customer projections can show customer-safe workroom and assignment refs but
hide account leases, route scorecards, source authority internals, and
action-submission refs.

Team projections can see route refs, but not account leases or source
authority refs.

Operator projections can see account-lease, route, source-authority, and
action-submission refs, but still reject raw secrets, raw runner logs, provider
tokens, private repo refs, customer emails, payment material, wallet material,
and raw timestamps.

## Tests

`workers/api/src/coding-autopilot-decision-actions.test.ts` covers:

- continue and retry-account projections;
- every planned action kind and status;
- the direct-effect guard;
- no raw timestamps in projections;
- public/customer/team/operator ref redaction;
- fail-closed rejection for provider, runner, token, private repo, and customer
  material.
