# Internal GitHub writer refusal boundary

- Date: 2026-08-03
- Status: implemented, not activated
- Owner: Omega All Work cutover (`OpenAgentsInc/omega#225`)
- Contract: `openagents.all_work_boundary.v1`

## Outcome

OpenAgents now has one shared policy for internal GitHub issue creation,
internal progress comments, and Repository Work Claim comments. The policy
keeps current `legacy_github` behavior before cutover. After the canonical
writer becomes `native_omega`, guarded internal writers refuse before they call
GitHub and direct the caller to create or update canonical Work through Omega.

This change does not activate the cutover. GitHub remains the current claim
ledger until an authorized `work.cutover.execute` activation is recorded and
the remaining installed acceptance gates pass.

## State resolution

Owner-local tools resolve the durable cutover ledger at
`$OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT/all-work/work-cutover.v1.json`. Remote or
hosted processes receive the writer value through
`OPENAGENTS_INTERNAL_WORK_WRITER=legacy_github|native_omega`.

- No signal means `legacy_github`, which preserves preactivation operation.
- A malformed signal or ledger fails closed.
- Conflicting configuration and ledger values fail closed.
- `native_omega` refuses `internal_issue_create`, `internal_issue_comment`, and
  `internal_claim_comment` before the external mutation seam.

The environment value is configuration transport only. It cannot activate the
canonical ledger or grant command authority.

## Guarded writers

The source audit covers these retained internal writer paths:

- Khala lag-profile issue creation.
- QA Observer issue creation and follow-up comments.
- The retained Electron Git/GitHub host's issue-creation operation.
- Pylon standing-task recreation and backlog replenishment.
- Artanis unsupported-request issue creation, including injected opener seams.
- Marching-orders delivered-progress comments.
- Release-feedback follow-up issue creation.

The TypeScript CLI and Pylon shell helper apply the same policy to shell
writers. Each high-level TypeScript path also checks before its concrete GitHub
adapter so dependency injection cannot bypass the fence.

## Retained GitHub boundaries

GitHub remains repository, pull-request, check, review, merge, release, source,
and provenance transport. Existing issue URLs remain dereferenceable.

Two bounded comment paths are intentionally not claim writers:

- A signed agent-definition run can post one idempotent completion callback to
  the exact source issue or pull request. It cannot create an issue.
- A backlog-faucet listing can post its idempotent lifecycle linkage to the
  exact source issue. It cannot claim ownership or create another issue.
- Release-candidate feedback can acknowledge the exact linked source thread.
  New internal follow-up issue creation is fenced and must route to Omega after
  activation.

The strict public bug form is also retained. Its verified webhook transport
must create an untrusted pending candidate through the strict-bug candidate
authority; it does not create canonical Work and grants no command authority.

## Remaining #225 gates

- Install the signature-verifying GitHub transport for strict bug candidates.
- Prove the packaged two-client journey against one owner-local authority.
- Activate only after source reconciliation and capture the activation receipt.
- Prove rollback against the native high-water cursor.
- Run the single deferred final build and installed acceptance gate.

Until those gates pass, `legacy_github` remains the canonical writer and this
policy is an inactive cutover fence.
