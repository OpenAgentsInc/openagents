# Ben OTEC Exa Enrichment Smoke Runbook

Date: 2026-06-05

Purpose: prove the canonical OTEC/SWAC/floating datacenter order can be enriched
with current public evidence before Adjutant starts the sitebuilder run.

Authoritative customer request:

```text
Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.
```

## Policy Boundary

- Preserve the exact order request as the customer intent.
- Use only public evidence from explicit source refs and bounded public
  web/topic search after the Adjutant assignment exists.
- Do not infer Ben's private identity, private repos, private social accounts,
  email, DMs, OAuth-only data, or provider-account grants from the name "Ben".
- Do not paste secrets, cookies, bearer tokens, Exa payloads, or local `.env`
  values into logs, issues, task packets, or public/customer projections.
- Public/customer projections may expose only approved public-safe summaries and
  source URLs.

## Operator Inputs

Set these local shell variables or keep them as browser-console constants. Do
not print secret values.

```sh
APP_ORIGIN="https://openagents.com"
SITE_ID="site_project_otec"
ORDER_ID="software_order_otec"
TASK_PACKET_PATH="docs/autopilot-tasks/adjutant-otec.md"
TASK_PACKET_COMMIT="<pushed commit sha containing the task packet>"
```

For production, verify that `EXA_API_KEY` is configured without printing it.
Use the admin UI or deployment environment inspector. Do not echo the key.

## Browser Session Commands

These commands are intended for an authenticated core-operator browser session
on `https://openagents.com/admin`. Browser `fetch` automatically carries the
operator session cookie.

### 1. Create Or Select Assignment

```js
const assignmentResponse = await fetch(
  `/api/operator/adjutant/sites/${SITE_ID}/assign`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      assignmentKind: 'site_generation',
      objective:
        'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
      taskSpecPath: TASK_PACKET_PATH,
      visibility: 'public',
    }),
  },
)
const { assignment } = await assignmentResponse.json()
const ASSIGNMENT_ID = assignment.id
```

If an assignment already exists, open:

```text
GET /api/operator/adjutant/assignments/:assignmentId
```

and confirm the objective still preserves the exact order request.

### 1a. Confirm Research Policy

Every assignment has a deterministic effective research policy. Read it before
planning or launching:

```js
const policyResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/research-policy`,
)
const { policy } = await policyResponse.json()
```

Expected OTEC Site-generation default:

```json
{
  "effectiveMode": "research_required",
  "source": "default_assignment_kind",
  "customerSafeStatus": "research_required"
}
```

If an operator intentionally bypasses or overrides research, record the bounded
policy explicitly. Do not put private operator notes, raw Exa payloads,
provider-account refs, OAuth data, callback URLs, or secrets in `reason`,
`customerSafeSummary`, or `sourceAuthorityRef`.

```js
await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/research-policy`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      policyMode: 'research_bypassed_by_operator',
      reason:
        'Customer supplied enough public source context in the submitted order.',
      customerSafeSummary:
        'The operator approved this assignment using existing customer-provided public context.',
      sourceAuthorityRef: `order:${ORDER_ID}`,
    }),
  },
)
```

Policy modes are:

- `research_required`
- `research_optional`
- `research_not_applicable`
- `research_bypassed_by_operator`

Current preflight surfaces this as a `research_policy` check. For
`research_required` assignments, preflight and launch also add a blocking
`research_required_gate` check until an approved research brief or explicit
operator bypass receipt is present.

### 2. Attach Explicit Public Source Refs

Attach only public refs selected by the operator/customer. Example public repo
and topic refs:

```js
await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/source-refs`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      kind: 'github_repository',
      label: 'OpenAgents OpenAgents Autopilot repository',
      status: 'public_safe',
      url: 'https://github.com/OpenAgentsInc/openagents',
    }),
  },
)

await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/source-refs`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      kind: 'generic_url',
      label: 'Public OTEC/SWAC reference selected for smoke',
      status: 'approved',
      url: '<public source URL>',
    }),
  },
)
```

Do not add private repository, private social, email, OAuth-only, or guessed
person-profile refs.

### 3. Create Enrichment Plan

```js
const planResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/plan`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      freshnessMaxAgeHours: 24,
      numResults: 4,
      operatorNotes:
        'Ground the OTEC/SWAC/floating datacenter Site in public evidence only.',
    }),
  },
)
const { plan } = await planResponse.json()
```

Confirm the plan has topic/repository tasks and no private identity inference.

### 4. Run Enrichment

For overnight or automatic fulfillment, prefer the durable queued path:

```js
const enqueueResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/enqueue`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      freshnessMaxAgeHours: 24,
      numResults: 4,
      requestBudget: 4,
      triggerKind: 'operator_requested',
    }),
  },
)
const { job } = await enqueueResponse.json()
```

Expected result: HTTP 202 with a durable `job.id`, queued enrichment run ID,
and `enrichment.latestJob`. Repeating the enqueue while the job is `queued` or
`running` returns HTTP 200 with `duplicate: true` and the existing job instead
of creating another run. The Worker queue consumer executes the same
policy-safe Exa task path as the manual run endpoint and updates the job to
`succeeded` or `failed`.

Manual enrichment remains available as an explicit operator control-plane
action:

```js
const runResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/run`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      freshnessMaxAgeHours: 24,
      numResults: 4,
      requestBudget: 4,
    }),
  },
)
const enrichmentRun = await runResponse.json()
```

Expected result: HTTP 202 with `runId`, source cards, and a needs-review
research brief. If the response is a typed blocker, use the failure table below.

### 5. Review Source Cards

Open:

```text
GET /api/operator/adjutant/assignments/:assignmentId
```

For every proposed card, approve only public-safe sources:

```js
await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/source-cards/${SOURCE_ID}/review`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      publicSafe: true,
      reviewStatus: 'approved',
    }),
  },
)
```

Reject questionable sources:

```js
await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/source-cards/${SOURCE_ID}/review`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      rejectedReason: 'Not public-safe or not relevant to the OTEC smoke.',
      reviewStatus: 'rejected',
    }),
  },
)
```

### 6. Approve Research Brief

After source-card review regenerates the brief, approve it:

```js
await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/enrichment/briefs/${BRIEF_ID}/review`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      reviewReason: 'Approved public evidence for OTEC smoke launch.',
      status: 'approved',
    }),
  },
)
```

Expected review state:

- `enrichment.status` is `approved`;
- `researchBrief.status` is `approved`;
- source cards are approved or rejected, with no proposed cards remaining.
- if a task packet was already generated from older or missing research, the
  assignment detail and preflight surfaces report
  `taskPacketFreshness.status: stale`.

### 7. Generate Task Packet

```js
const packetResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/task-packet`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      commitSha: TASK_PACKET_COMMIT,
      operatorNotes:
        'Use the approved research brief as source context. Keep claims careful.',
      taskSpecPath: TASK_PACKET_PATH,
    }),
  },
)
const { packet, taskPacketFreshness } = await packetResponse.json()
```

Confirm the packet contains:

- `## Approved Research Brief`;
- `researchBriefId`;
- concise grounded facts;
- approved source URLs only.

Confirm freshness after generation:

- `taskPacketFreshness.status` is `current`;
- `taskPacketFreshness.researchBriefId` is the approved brief ID;
- `taskPacketFreshness.taskSpecPath` is `TASK_PACKET_PATH`.

If an operator reviews a stale packet and intentionally keeps it for this pass,
record the bounded reason before preflight. Do not include private operator
notes, raw Exa payloads, provider-account refs, OAuth data, callback URLs, or
secrets in either field.

```js
await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/task-packet/keep-current`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      reason:
        'Operator reviewed the approved brief and confirmed the current packet is sufficient.',
      customerSafeSummary:
        'The operator confirmed the current task packet already includes the approved public research context needed for this pass.',
    }),
  },
)
```

### 8. Preflight

```js
const preflightResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/preflight`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      launchChecklist: {
        audienceReviewed: true,
        buildReviewed: true,
        secretsReviewed: true,
        sourceReviewed: true,
        urlReviewed: true,
      },
    }),
  },
)
const preflight = await preflightResponse.json()
```

Required checks before launch:

- `exa_enrichment: ok`;
- `research_required_gate: ok`;
- `research_brief: ok`;
- `research_review: ok`;
- `task_packet_freshness: ok`;
- `task_packet: ok`;
- `commit_sha: ok`;
- `sites_launch_checklist: ok`;
- provider, GitHub, SHC, and callback checks are `ok`.

### 9. Launch Adjutant/Sitebuilder

```js
const launchResponse = await fetch(
  `/api/operator/adjutant/assignments/${ASSIGNMENT_ID}/launch`,
  {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      launchChecklist: {
        audienceReviewed: true,
        buildReviewed: true,
        secretsReviewed: true,
        sourceReviewed: true,
        urlReviewed: true,
      },
    }),
  },
)
const launch = await launchResponse.json()
```

Record the returned `runId` and confirm launch selector metadata includes
`researchBriefId`. Do not copy raw session cookies or provider payloads into
tracking issues.

### 10. Save Version And Deploy After Review

When the runner returns reviewable artifacts, save the version:

```text
POST /api/operator/sites/:siteId/versions
```

Deploy only after operator review:

```text
POST /api/operator/sites/:siteId/versions/:versionId/deploy
```

If the deployed Site is unsafe:

```text
POST /api/operator/sites/:siteId/deployments/:deploymentId/disable
POST /api/operator/sites/:siteId/deployments/:deploymentId/rollback
```

### 11. Inspect Public Proof Closeout

The public-safe OTEC proof closeout API is:

```text
GET /api/public/proof/otec
```

The response is a no-store JSON projection for the canonical production OTEC
order (`software_order_c34f3a52d60b41d699b71525365b6ee5`). It can expose:

- customer-safe order, Site, assignment, version, deployment, compatibility,
  build-validation, research, and receipt status;
- Site URL only when the active deployment is recorded as `active`;
- approved research/source counts without raw Exa payloads;
- public receipt refs, evidence refs, claim-state labels, caveats, and next
  action.

It must not expose private operator notes, provider account refs, auth grants,
callback tokens, raw runner payloads, raw Exa payloads, private source data, or
secret-shaped material. If the order is missing, the API returns
`public_otec_proof_not_found`. If any projected text becomes unsafe, it fails
closed with `public_otec_proof_unsafe`.

## Manual Production Smoke Checklist

- Assignment objective preserves the exact OTEC order text.
- Explicit source refs are public-safe and not guessed from "Ben".
- Plan has no private identity inference or private repo/social refs.
- Exa run completes or returns a typed blocker.
- Source cards are reviewed; rejected/internal-only cards are not projected.
- Approved research brief exists and has `approvedAt`.
- Task packet includes the approved brief and source URLs.
- Preflight reports `exa_enrichment`, `research_required_gate`,
  `research_brief`, and `research_review`.
- Launch selector includes `researchBriefId`.
- Customer/public surfaces show only approved summaries/source URLs.
- `GET /api/public/proof/otec` returns the current public-safe closeout state.
- Issue/run notes include run IDs and statuses, not secrets.

## Failure And Rollback Handling

| Failure                     | Expected Response                                   | Operator Action                                                                    |
| --------------------------- | --------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Missing `EXA_API_KEY`       | `exa_unconfigured`                                  | Configure Exa or skip refresh only if an already approved brief exists.            |
| Budget exhausted            | `exa_budget_exhausted`                              | Reduce `requestBudget`, wait for the daily window, or explicitly raise env budget. |
| Duplicate active enrichment | `adjutant_enrichment_already_running`               | Wait for the active run to complete or inspect assignment review.                  |
| Provider failure            | failed query plus `failed` or `partial_failure` run | Review available cards, then `refresh` after provider recovery.                    |
| Stale/rejected brief        | `research_review: warning`                          | Refresh enrichment or approve a regenerated brief before launch.                   |
| Required research missing   | `research_required_gate: blocked`                   | Queue/refresh Exa enrichment, approve the brief, or record an explicit bypass.     |
| Required research active    | `research_required_gate: blocked` with job/run ID   | Wait for the job/run, then review source cards and approve the brief.              |
| Required research failed    | `research_required_gate: blocked` with redacted code | Refresh research or record an explicit operator bypass with bounded reason.        |
| Rejected source card        | card hidden from public-safe context                | Regenerate/reapprove brief from remaining approved cards.                          |
| Task packet missing         | `task_packet_ref_missing`                           | Push the packet path at the referenced commit and retry.                           |
| Launch blocked              | `adjutant_launch_blocked`                           | Resolve the named blocked check before retrying launch.                            |
| Bad saved Site version      | no deploy until review                              | Reject/adjust the version; do not activate.                                        |
| Bad deployment              | disable or rollback deployment routes               | Record the rollback event and request an adjustment.                               |

## Verification Evidence To Record

When closing the smoke, record:

- assignment ID;
- Exa enrichment run ID;
- research brief ID;
- task packet path and commit SHA;
- preflight status and failed/warning check names, if any;
- Adjutant run ID;
- Site version ID;
- deployment ID or reason deployment was skipped.
- public proof closeout URL and claim state.

Do not record secret values, raw cookies, API keys, or full provider payloads.
