# Order Triage Operator Runbook

Issue: `OPENAGENTS-P0-001`

The order triage queue gives operators a durable first-batch view before
overnight Adjutant or Sites runs. It stores internal launch priority separately
from the customer-safe order projection.

## Classifications

Use exactly one classification per active triage record:

- `runnable_site`: website work that can become a Sites assignment after a
  compatibility check.
- `runnable_general_autopilot`: non-Sites work that can become a general
  Adjutant assignment.
- `needs_clarification`: priority work that still needs a scoped first slice or
  customer/operator clarification.
- `smoke_or_test`: pipeline probes that must not be launched as customer work.
- `legal_sensitive_policy_review`: legal-sensitive work that requires explicit
  human policy review before any automated fulfillment run.
- `unavailable_or_declined`: requests OpenAgents cannot currently fulfill.

Only `runnable_site` and `runnable_general_autopilot` can be reported as
`overnightLaunchEligible`, and the record must also have
`firstBatchEligible: true`.

## Operator API

List the current queue:

```text
GET /api/operator/orders/triage?limit=100
```

Update or create a triage record:

```text
PATCH /api/operator/orders/{softwareOrderId}/triage
```

Body:

```json
{
  "classification": "runnable_site",
  "operatorPriority": 10,
  "firstBatchEligible": true,
  "holdReason": null,
  "nextAction": "Run existing-project compatibility check, create a Site assignment, then prepare the first saved version for review.",
  "customerSafeStatus": "scoping",
  "customerSafeSummary": "OpenAgents is preparing this website order for the first overnight Sites batch.",
  "orderStatus": "scoping"
}
```

The route requires an OpenAgents admin browser session. It does not expose
provider account refs, auth grants, device-login state, or secrets.

## First-Batch Assignment Creation

After triage is reviewed, create first-batch assignments from typed triage
state instead of prompt keyword matching:

```text
POST /api/operator/orders/triage/first-batch/assign
```

Dry-run body:

```json
{
  "dryRun": true,
  "limit": 25
}
```

Targeted real body:

```json
{
  "softwareOrderIds": [
    "software_order_..."
  ]
}
```

Behavior:

- `runnable_site` creates or reuses the linked `site_projects` row and creates
  a `site_generation` Adjutant assignment in `preflight_pending`.
- `runnable_general_autopilot` creates a `general_order_fulfillment`
  assignment in `preflight_pending`.
- Existing active assignments are treated as `already_assigned`; the route does
  not create duplicates.
- `needs_clarification`, `smoke_or_test`, `legal_sensitive_policy_review`, and
  `unavailable_or_declined` remain held and do not create assignments.
- The route writes a durable `order_triage_events` receipt for created, held,
  and already-assigned decisions.
- Assignment creation also writes an `adjutant.first_batch_assignment_prepared`
  event. No Autopilot launch, build, saved Site version, deployment, provider
  lease, or ChatGPT/Codex run starts from this endpoint.

The response is operator-safe and includes decision, order ID, site ID when
present, assignment ID when present, receipt ID, classification, next action,
and customer-safe status/summary. It must not include provider account refs,
auth grants, secret refs, raw runner payloads, or private triage notes.

## Reusable Order Fulfillment Prepare

Issue `OPENAGENTS-006` adds the reusable single-order prepare endpoint for orders
outside the first-batch-only flow:

```text
POST /api/operator/orders/:softwareOrderId/fulfillment/prepare
```

Body:

```json
{
  "dryRun": false
}
```

Behavior:

- `runnable_site` creates or reuses the linked `site_projects` row and creates
  a `site_generation` Adjutant assignment in `preflight_pending`.
- `runnable_general_autopilot` creates or reuses a
  `general_order_fulfillment` assignment in `preflight_pending`.
- Existing active assignments return `already_assigned` and do not create
  duplicates.
- Held classes remain held with a typed reason and no assignment creation.
- Unlike the first-batch endpoint, this path is not gated on
  `firstBatchEligible`; it relies on the typed triage classification.
- No Autopilot launch, task packet generation, provider lease, saved Site
  version, deployment, or email send is started by this endpoint.

Durable events use non-first-batch event names:

- `order_fulfillment.prepare_assignment_created`
- `order_fulfillment.prepare_already_assigned`
- `order_fulfillment.prepare_held`
- `adjutant.order_fulfillment_prepared`

The response is operator-safe and carries the same decision shape as the
first-batch assignment endpoint. It must not include provider account refs,
auth grants, secret refs, raw runner payloads, private operator notes, callback
tokens, or raw Exa payloads.

## GitHub PR-Style Order Authority

General codebase orders can produce branches, commits, diffs, pull requests,
or notes through `order_fulfillment_artifacts`. Creating an external GitHub
branch, push, or pull request is a write action and must be authorized before a
runner or operator executor performs it.

Use the authority model in `workers/api/src/github-writeback-authority.ts`:

- Every external write requires an explicit approval receipt for the order.
- Public repository PRs may use `openagents_fork` only for
  `open_fork_pull_request` and only after explicit approval.
- Private repository PRs must use `customer_grant`, backed by a healthy
  `github_write_connections` row and a fresh `github_write_auth_grants` row
  tied to the assignment/run.
- Expired, used, revoked, mismatched, missing-scope, missing-secret, or
  missing-connection grants block the write.
- Blocked decisions must create a customer-safe input-needed or unavailable
  explanation. They must not attempt a branch, push, or PR.

Persist every decision in `order_github_write_authority_receipts` before the
write executor continues. Successful PR, branch, commit, or diff artifacts
should include the same secret-safe authority metadata in
`order_fulfillment_artifacts.metadata_json` so the customer and team can see
why the artifact is valid without exposing provider grants or token refs.

The current generic Autopilot launch path still requires a customer GitHub
write connection when a GitHub work order is present. Do not relax that path
until the order-specific PR executor consumes authority receipts directly.

### Public Repository Fork PR Receipts

For public repositories, a runner or operator-side GitHub workflow can create a
pull request from an OpenAgents-controlled fork and then hand OpenAgents product surface a PR
receipt. Record it through the public-fork fulfillment path in
`workers/api/src/github-pr-fulfillment.ts`.

That path:

- uses `open_fork_pull_request` and authority mode `openagents_fork`;
- still requires explicit customer/operator approval;
- writes the authority receipt before publishing a PR artifact;
- records fork, branch, target branch, commit, PR URL, tests summary, and
  authority metadata in `order_fulfillment_artifacts`;
- marks the order delivered when the review-ready PR artifact is saved;
- leaves unapproved or blocked attempts as customer-safe notes artifacts
  instead of attempting a GitHub write.

The scheduled review-ready artifact email reconciler will pick up the public
PR artifact through the existing `customer_review_ready` fulfillment artifact
path.

### Private Repository Customer-Grant PR Receipts

For private repositories, use the customer-grant fulfillment path in
`workers/api/src/github-pr-fulfillment.ts`. This path is for a runner or
operator-side GitHub workflow that has already created a PR using a
customer-approved write grant and needs to record the review-ready artifact in
OpenAgents product surface.

That path:

- uses `open_pull_request` and authority mode `customer_grant`;
- requires explicit approval, a healthy `github_write_connections` row,
  repo/workflow scopes, a secret-backed connection, and a fresh issued
  `github_write_auth_grants` row tied to the assignment/run;
- blocks missing connection, missing scope, missing secret, expired grant,
  used grant, revoked grant, mismatched grant, or missing grant before a PR
  artifact can be published;
- records branch, target branch, commit, PR URL, tests summary, permission
  receipt, and authority metadata when the customer grant is valid;
- marks the order delivered when the review-ready private PR artifact is
  saved;
- leaves blocked attempts as customer-safe notes artifacts and moves the order
  to `needs_customer_input` rather than attempting a private-repo write.

The review-ready artifact email reconciler uses the same fulfillment artifact
path as public fork PRs, so private PR review emails remain ledger-backed and
idempotent.

## First-Batch No-Payment Policy

Before launching a runnable first-batch Adjutant assignment, apply an explicit
no-payment policy record:

```text
POST /api/operator/orders/triage/first-batch/payment-policy
```

Body:

```json
{
  "softwareOrderIds": [
    "software_order_..."
  ],
  "policyMode": "public_beta_free",
  "reason": "First submitted-order batch is covered by the OpenAgents public beta free-slice policy.",
  "customerSafeSummary": "This first-batch OpenAgents run is covered by a public beta free slice. No customer charge is being recorded for this launch."
}
```

`policyMode` can be `public_beta_free` or `operator_grant`. The route is
idempotent per active software order and records:

- policy ID and mode;
- software order ID;
- current assignment ID when present;
- current Site ID when present;
- applying operator user ID;
- operator reason;
- customer-safe no-charge summary; and
- created/updated timestamps.

The route rejects customer-safe policy copy that implies paid settlement,
Lightning, MDK, L402, wallet, provider payout, invoice, or payment-ID activity.
First-batch launch preflight then includes a `first_batch_payment_policy`
check. Runnable first-batch orders are blocked at launch until this explicit
policy exists; non-first-batch assignments do not require it. Generation usage
receipts for this path remain zero-charge `public_beta_free` receipts and carry
the first-batch policy ID and mode in team receipt details.

## First-Batch Overnight Monitor

Operators can inspect all first-batch runnable and held orders from one
read-only surface:

```text
GET /api/operator/orders/triage/first-batch/monitor?limit=100
```

The monitor returns each first-batch record with:

- customer-safe order title, repository ref, status, and summary;
- triage classification, eligibility, hold reason, and next action;
- Site project ID, slug, title, and status when present;
- Adjutant assignment ID, kind, status, goal ID, current run ID, task packet
  path, and commit SHA when present;
- active provider-account lease ref and public provider account ref when a
  lease exists;
- first-batch no-payment policy requirement, status, policy mode, policy ID,
  customer-safe summary, and applying operator when present;
- latest failover receipt and account state action;
- latest runner callback/event, callback status, and callback lag;
- current blocker and next action; and
- safe operator commands for dashboard, lease explanation, failover history,
  assignment review, and first-batch assignment creation.

Monitor states are:

- `not_yet_assigned`
- `preflight_ready`
- `queued`
- `running`
- `blocked`
- `waiting_for_input`
- `review_ready`
- `deployed`
- `delivered`
- `held`
- `failed`

The monitor is operator-only. It may expose public provider account refs and
lease refs, but it must not expose provider tokens, auth JSON, auth grant refs,
secret refs, callback bearer tokens, raw runner payloads, raw provider
responses, private operator notes, billing secrets, payment IDs, wallet refs,
or provider settlement payloads.

## Seeded First Batch

The migration seeds triage records for the seven known submitted orders:

- Chefgroep site remake: `runnable_site`, first-batch eligible.
- Ben OTEC site: `runnable_site`, first-batch eligible.
- OpenAgents marketing: priority, but `needs_clarification` until the first
  slice is scoped.
- Uplink MVP, AIBTC, and OpenAgents product surface smoke orders: `smoke_or_test`, not launchable.
- Minnesota legal/lawsuit request: `legal_sensitive_policy_review`, not
  launchable.

The public customer order response adds a `triage` projection with only
`status`, `summary`, and `nextAction`.
