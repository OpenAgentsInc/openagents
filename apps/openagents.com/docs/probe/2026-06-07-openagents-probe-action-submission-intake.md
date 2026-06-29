# OpenAgents product surface Probe Action Submission Intake

Issue #497 adds the OpenAgents product surface intake path for Probe Action Submission proposals.

The route is:

- `POST /api/blueprint/action-submissions`
- `GET /api/blueprint/action-submissions`

Probe submits `ProbeBlueprintActionSubmissionProposal` records for external
write-side effects. OpenAgents product surface normalizes accepted proposals into
`BlueprintActionSubmission` records in D1. The POST route is runner-callback
authorized, with admin API token fallback for operator-owned verification. The
GET route is operator-read only.

The intake route is proposal only. Accepted records are stored with:

- `status: "pending_approval"`
- `approvalState: "pending"`
- `directExecution: false`
- `directProgramRunExecutionAllowed: false`
- `programRunAuthorityBoundary: "evidence_only"`
- `modelConfidenceBypassDisabled: true`
- `contentRedacted: true`

The route maps Probe external effects into OpenAgents product surface action kinds:

- `create_pull_request` -> `create_pull_request`
- `deploy` -> `deploy`
- `send_email` -> `send_email`
- `post_public_claim` -> `public_claim_upgrade`
- `spend_money` -> `payment`
- `legal_sensitive_commitment` -> `legal_sensitive_action`
- `mutate_source_backed_business_fact` -> `source_writeback`

Probe-local effects such as sandbox reads, sandbox file edits, and local
evidence records are rejected. They remain local Probe tools and must not become
OpenAgents product surface Action Submissions.

OpenAgents product surface requires a Program Run ref, approval policy ref, evidence refs, summary
ref, and redacted receipt refs. Proposal metadata can preserve Probe actor,
assignment, Program Type, Program Signature, Module Version, input snapshot,
effect kind, and redacted typed intent, but the public route response only
returns the canonical Action Submission record. The raw body is checked before
schema decoding so unknown fields cannot smuggle raw emails, callback URLs or
tokens, provider payloads, payment material, private repo contents, wallet
material, keys, mnemonics, raw customer data, or raw prompts.

This route does not execute PR creation, deploys, email sends, payments, public
claim upgrades, legal-sensitive commitments, or source-backed mutations. Those
effects require a later reviewed executor path with approval and execution
receipts.
