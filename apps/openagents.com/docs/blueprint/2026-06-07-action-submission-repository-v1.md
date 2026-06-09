# Blueprint Action Submission Repository v1

Issue #497 adds the first OpenAgents product surface-owned Action Submission persistence slice. The
source of truth is:

- `workers/api/src/blueprint/schemas/action-submission.ts`
- `workers/api/src/blueprint/repositories/action-submissions.ts`
- `workers/api/migrations/0132_blueprint_action_submissions.sql`

Action Submissions are the write-side proposal boundary for Program Runs. They
can cite external effects that need review, but the proposal intake path cannot
execute those effects.

The v1 record stores:

- action kind;
- approval policy ref;
- Program Run ref;
- proposed effect ref;
- evidence refs;
- context pack refs;
- source authority refs;
- summary ref;
- tool refs;
- redacted receipt refs;
- dry-run requirement state;
- approval state;
- execution and failure receipt refs, which are always null at proposal intake;
- explicit direct-execution guardrail flags.

Probe proposals land as `pending_approval` with `approvalState: "pending"`.
They are approval-gated immediately and `blueprintActionSubmissionCanExecute`
returns false until a separate reviewed path records approval and required dry
run receipts. The repository rejects missing evidence refs and private or
execution-shaped material in refs or metadata.

This repository intentionally does not mutate Program Runs, create PRs, deploy,
send emails, spend money, publish claims, or write source-backed business facts.
