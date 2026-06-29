# Coding On Autopilot Mission Records

Date: 2026-06-06

Status: implemented contract note for GitHub issue #313 / `OPENAGENTS-066`.

## Purpose

Coding on Autopilot needs a durable mission object instead of relying on
sidebar rows, thread state, or inferred assignment state. A mission is the
user-facing container for long-running coding or Site work.

The first implementation lives in
`workers/api/src/coding-autopilot-missions.ts`.

## Mission Record

`CodingAutopilotMissionRecord` stores stable refs for:

- mission identity and work kind;
- status;
- objective stack refs;
- owner, customer, and team refs;
- workroom refs;
- assignment refs;
- route-scorecard refs;
- account-lease refs;
- budget refs;
- blocker refs;
- next-order refs;
- latest Mission Briefing ref;
- artifact refs.

The record may contain raw ISO timestamps because it is backend truth. The
projection layer renders those timestamps as friendly labels before anything is
shown to customers or public users.

## Projection Rules

Mission projections support `public`, `customer`, `team`, and `operator`
audiences.

Public projections show mission status, safe objective refs, safe blockers,
safe next-order refs, safe artifact refs, and the latest briefing ref. They do
not show customer, team, workroom, assignment, route-scorecard, account-lease,
budget, or owner refs.

Customer projections may show customer and workroom refs, but not route
scorecards, account leases, provider-account refs, budgets, private repo refs,
raw runner logs, or raw timestamps.

Team projections can see team refs and route-scorecard refs, while account
lease and budget refs remain operator-only.

Operator projections can see account-lease and budget refs, but still reject
raw secrets, raw runner logs, raw patches/source archives, provider tokens,
payment material, wallet material, customer emails, and private keys.

## Blueprint Briefing Boundary

The mission record links to the existing Blueprint continuation Mission
Briefing by ref only. It does not copy briefing contents or private workroom
truth into the mission object. The briefing renderer remains the source for
briefing sections, and future mission UI should fetch or compose that
projection through the established briefing contract.

## Tests

`workers/api/src/coding-autopilot-missions.test.ts` covers:

- public/customer/team/operator projections;
- status labels;
- friendly time labels with no raw timestamps in projections;
- audience redaction for account leases, route scorecards, and budgets;
- fail-closed rejection of raw runner logs, provider-account refs, private repo
  refs, customer emails, and secret-shaped refs.
