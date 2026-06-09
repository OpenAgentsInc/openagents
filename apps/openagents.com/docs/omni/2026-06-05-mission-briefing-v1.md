# Omni Mission Briefing v1

Date: 2026-06-05

Status: implemented for issue #213.

## Purpose

Mission Briefing gives returning customers and operators a customer-safe
summary of a workroom without requiring anyone to read private run logs.

The first implementation is Sites-oriented but intentionally reusable for
coding, adjustment, business, and future legal-sensitive workrooms.

## Projection Shape

`buildOmniMissionBriefing` projects:

- what changed;
- what was built;
- what is blocked;
- what needs review;
- what was emailed;
- what happens next.

Each item carries only:

- section kind;
- customer-safe ref;
- customer-safe summary ref;
- status;
- friendly display time.

The briefing does not project raw ISO timestamps. It transforms time into
friendly labels such as `Just now`, `15 minutes ago`, `1 hour ago`,
`Yesterday`, or `3 days ago`.

## Inputs

The projection composes already-modeled customer-safe records:

- `omni_workrooms`;
- `omni_evidence_bundles`;
- `omni_workroom_lifecycle_decisions`;
- workroom email refs.

Evidence and lifecycle decisions are passed through their customer-safe
projection functions before briefing sections are assembled.

## Section Rules

Changed:

- research briefs;
- source commits;
- generated source;
- diffs.

Built:

- workroom artifact refs;
- deployment URLs;
- screenshots;
- test reports;
- receipts.

Blocked:

- workroom blocker refs.

Review:

- lifecycle decision receipts and customer-safe explanation refs.

Email:

- workroom email refs.

Next action:

- acceptance recorded;
- revision queue;
- rejection review;
- clear blocker;
- review latest work;
- complete;
- work in progress.

## Guardrails

The projection drops refs and summary refs that contain raw provider payload
markers, raw run logs, raw emails, customer-private email addresses, auth
material, payment/wallet material, secret-like strings, or raw ISO timestamps.

Mission Briefing does not:

- fetch private logs;
- infer payment settlement;
- mark payout eligibility;
- deploy a Site;
- send email;
- mutate order or workroom state.
