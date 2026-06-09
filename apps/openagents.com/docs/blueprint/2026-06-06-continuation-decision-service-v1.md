# Continuation Decision Service v1

Date: 2026-06-06

Status: implemented for #272 / `OPENAGENTS-CONT-001`.

## Purpose

The continuation decision service turns a completed, interrupted, failed, or
blocked Autopilot turn into the next governed Program Signature decision.

This is the product version of "keep going" or "stop and ask for help"; it is
not a new Guidance Module class. The durable authority remains the Blueprint
Program Signature and Module Version catalog.

## Decision Kinds

The service classifies a turn into one of:

- `continue`
- `test`
- `fix`
- `summarize`
- `request_context`
- `retry_account`
- `stop`
- `escalate`
- `prepare_review`

Each decision returns:

- Program Signature id;
- candidate Module Version id where the catalog has one;
- Program Type id;
- reason;
- confidence;
- constraint refs;
- evidence refs;
- receipt refs;
- source authority refs;
- work ref and turn-result ref.

## Authority Boundary

Continuation decisions are evidence-only.

They must not directly:

- deploy;
- send email;
- create pull requests;
- mutate source-backed facts;
- spend money;
- upgrade public claims.

Any direct effect must become an approval-gated Action Submission or another
authorized product path.

## Classifier Priority

The current deterministic classifier uses this priority:

1. user-requested escalation;
2. user-requested stop;
3. missing required context;
4. provider/account failure;
5. build/runtime/test failure;
6. unverified generated changes;
7. review-ready artifacts;
8. summary needed;
9. blocked state or blockers;
10. continue.

This priority is intentionally conservative: repair, context, account retry,
and review boundaries are selected before open-ended continuation.

## Future Work

#273 will add retained fixtures from first-batch runs. #274 will project
pending decisions into a Decision Queue. #275 will render Mission Briefings
from the resulting evidence and workroom records.
