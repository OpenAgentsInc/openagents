# Coding On Autopilot Continuation Decisions

Date: 2026-06-06

Status: implemented contract note for GitHub issue #315 / `OPENAGENTS-068`.

## Purpose

Continuation decisions record why Autopilot should continue, pause, retry an
account, ask for context, run tests, prepare review, or stop between turns.
They connect the Coding on Autopilot mission layer to Blueprint Program Type,
Program Signature, Module Version, Program Run, constraints, evidence, receipts,
and guardrails.

The implementation lives in
`workers/api/src/coding-autopilot-continuation-decisions.ts`.

## Record Shape

`CodingAutopilotContinuationDecisionRecord` captures:

- mission ref;
- workroom refs;
- Program Run ref;
- Program Type id;
- Program Signature id;
- Module Version id;
- selected Blueprint continuation action;
- mapped Coding on Autopilot queue action kind;
- confidence and confidence bucket;
- constraints;
- guardrail state;
- evidence refs;
- receipt refs;
- source-authority refs;
- rejected alternative refs;
- risk refs;
- customer explanation ref.

## Evidence-Only Rule

The record is only accepted if the source Blueprint continuation decision is
evidence-only. The projection carries:

- `evidenceOnly: true`;
- `directEffectPermitted: false`;
- `actionSubmissionRequiredForDirectEffects: true`.

If a Blueprint decision has write-authority flags, the conversion fails.
Continuation decisions cannot directly deploy, email, create pull requests,
mutate source facts, spend money, post publicly, or upgrade public claims.

## Projection Rules

Public projections hide Program Run, workroom, and source-authority refs.

Customer projections can see safe Program Run and workroom refs, but not
source-authority internals.

Team projections also hide source-authority internals.

Operator projections can see source-authority refs, but still reject provider
tokens, raw runner logs, raw patches/source archives, private repo refs,
customer emails, payment material, wallet material, private keys, and raw
timestamps.

## Tests

`workers/api/src/coding-autopilot-continuation-decisions.test.ts` covers:

- public/customer/operator projection splits;
- confidence buckets;
- Blueprint action to queue-action mapping;
- evidence-only conversion enforcement;
- no raw timestamps in projections;
- unsafe provider, runner, private repo, and customer ref rejection.
