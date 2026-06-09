# Coding On Autopilot Artifacts

Date: 2026-06-06

Status: implemented contract note for GitHub issue #316 / `OPENAGENTS-069`.

## Purpose

Coding on Autopilot needs durable artifact records for the customer handoff and
mission briefing loop. This issue models refs and projections only. It does not
create raw artifact storage.

The implementation lives in `workers/api/src/coding-autopilot-artifacts.ts`.

## Artifact Kinds

The v1 model supports:

- `diff_summary`;
- `patch_ref`;
- `test_run`;
- `build_log_summary`;
- `preview_url`;
- `pr_draft`;
- `pr_url`;
- `rollback_note`;
- `screenshot_ref`;
- `redaction_report`;
- `fulfillment_receipt`;
- `customer_note`.

## Record Shape

`CodingAutopilotArtifactRecord` stores:

- artifact kind;
- mission and workroom refs;
- source refs;
- evidence refs;
- visibility;
- status;
- public-safe flag;
- authority receipt refs;
- summary ref;
- caveat refs;
- retention caveat refs;
- created/updated/archive timestamps for backend truth.

Statuses are `draft`, `ready`, `blocked`, `failed`, `superseded`, and
`archived`.

Visibility is `private`, `team`, `customer`, or `public`.

## Safety Rules

Public visibility requires `publicSafe: true`.

Ready artifacts require either evidence refs or authority receipt refs.

`pr_draft` and `pr_url` artifacts require authority receipt refs so PR
writeback is never shown without a writeback authority trail.

The model rejects raw build logs, raw runner logs, raw patches, source
archives, provider tokens, source/private repo material, customer emails,
payment material, wallet material, and raw timestamps in projections.

## Projection Rules

Public projections show only public artifacts and hide workroom refs.

Customer projections show customer and public artifacts.

Team projections show team, customer, and public artifacts.

Operator projections can see private artifacts as refs, but still reject raw
secret, provider, payment, wallet, customer, source archive, raw patch, and raw
log material.

## Tests

`workers/api/src/coding-autopilot-artifacts.test.ts` covers:

- public/customer/team visibility;
- every required artifact kind;
- readiness and PR authority receipt guardrails;
- public-safe visibility guardrails;
- no raw timestamps in projections;
- unsafe raw log, raw patch, source archive, provider token, and customer ref
  rejection.
