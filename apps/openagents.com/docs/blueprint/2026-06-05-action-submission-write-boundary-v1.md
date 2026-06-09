# Blueprint Action Submission Write Boundary v1

Issue: OPENAGENTS-BP-008 / #228

This note records the typed Action Submission model. The source of truth is
`workers/api/src/blueprint/schemas/action-submission.ts`.

## Purpose

Program Runs are evidence-only. Any direct effect has to become an Action
Submission before it can move toward execution.

The v1 model covers these write-sensitive actions:

- deploy;
- send email;
- create pull request;
- source writeback;
- public claim upgrade;
- payment;
- legal-sensitive action.

## Required Path

Action Submissions move through proposal, dry run, approval, execution, receipt,
and failure states. Execution is allowed only when:

- status is approved;
- approval state is approved;
- approval receipt exists;
- approver ref exists;
- required dry run is complete;
- no execution receipt already exists;
- no failure ref exists.

## Current Limits

This issue defines the typed write boundary and state guards. D1 persistence,
operator approval routes, executor services, failure receipts, and UI controls
are separate roadmap issues.
