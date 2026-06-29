# Autopilot Task: Adjutant Site Fulfillment Launch Runbook

Status: active launch runbook packet for the first Adjutant Site fulfillment.

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `agent_adjutant`

Team: `team_openagents_core`

Project: `project_adjutant`

Canonical first smoke: Ben OTEC/SWAC floating datacenter Site, when production
data permits.

## Production Target

```text
softwareOrderId: <ben-otec-software-order-id>
siteId: <site_project_otec>
site slug: otec
site title: OTEC Floating Datacenter
target URL: https://sites.openagents.com/otec
agentId: agent_adjutant
projectId: project_adjutant
teamId: team_openagents_core
visibility: public after review
```

Safe production links:

- Admin overview: `https://openagents.com/admin`
- Customer order surface: `https://openagents.com/order`
- Public Adjutant route: `https://openagents.com/adjutant`
- Canonical public Adjutant route: `https://openagents.com/agents/adjutant`
- Public Site target: `https://sites.openagents.com/otec`
- Public activity API: `https://openagents.com/api/public/adjutant/activity`

## Local Verification Gate

Run these from the repo root before production launch:

```sh
bun run --cwd workers/api test \
  src/adjutant-assignments.test.ts \
  src/operator-adjutant-routes.test.ts \
  src/adjutant-run-lifecycle.test.ts \
  src/customer-order-routes.test.ts \
  src/adjutant-public-activity.test.ts \
  src/operator-sites-routes.test.ts \
  src/sites.test.ts
```

```sh
bun run --cwd apps/web test \
  src/route.test.ts \
  src/main.test.ts \
  src/docs-blog-route.test.ts \
  src/page/loggedIn/view.scene.test.ts \
  src/page/loggedIn/update.test.ts
```

```sh
bun run typecheck
bun run check:architecture
git diff --check
```

## Required Operator Material

Use an authenticated admin browser session for production calls. If issuing API
calls from the terminal, put the browser session cookie or approved operator
token in an ignored local shell variable and do not print it:

```sh
export OPENAGENTS_BASE_URL="https://openagents.com"
export OPENAGENTS_ADMIN_COOKIE="<admin-session-cookie>"
export ADJUTANT_SOFTWARE_ORDER_ID="<ben-otec-software-order-id>"
export ADJUTANT_SITE_ID="<site_project_otec>"
export ADJUTANT_ASSIGNMENT_ID="<adjutant_assignment_id>"
export ADJUTANT_TASK_PACKET_PATH="docs/autopilot-tasks/2026-06-05-adjutant-ben-otec.md"
export ADJUTANT_TASK_PACKET_COMMIT="$(git rev-parse HEAD)"
```

If a browser session is not available to the shell, perform the same steps
through `https://openagents.com/admin` and use these commands only as the exact
payload reference.

## Launch Flow

1. Create or confirm the Site for the order.

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/operator/sites" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  --data-binary @- <<'JSON'
{
  "softwareOrderId": "<ben-otec-software-order-id>",
  "slug": "otec",
  "title": "OTEC Floating Datacenter",
  "accessMode": "public",
  "visibility": "public",
  "teamId": "team_openagents_core",
  "projectId": "project_adjutant"
}
JSON
```

2. Assign the Site to Adjutant.

Operator Adjutant endpoints accept either an authenticated admin browser
session cookie or the ignored local `OPENAGENTS_ADMIN_API_TOKEN` as a bearer
token. Prefer the bearer token for CLI launches and never print or commit the
token value.

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/operator/adjutant/sites/$ADJUTANT_SITE_ID/assign" \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  --data-binary @- <<'JSON'
{
  "assignmentKind": "site_generation",
  "objective": "Build the public OTEC floating datacenter Site for Ben's software order.",
  "visibility": "public",
  "teamId": "team_openagents_core",
  "projectId": "project_adjutant",
  "agentId": "agent_adjutant"
}
JSON
```

3. Write the assignment task packet from
   `docs/autopilot-tasks/adjutant-site-task-template.md`, commit it, and push
   `main`.

```sh
git status --short
git add "$ADJUTANT_TASK_PACKET_PATH"
git commit -m "Add Ben OTEC Adjutant launch packet"
git push origin main
export ADJUTANT_TASK_PACKET_COMMIT="$(git rev-parse HEAD)"
```

The packet must include assignment ID, software order ID, Site ID, goal ID,
target URL, output contract, safety rules, and acceptance criteria. It must not
include secrets, provider grants, callback tokens, OAuth material, raw runner
payloads, private prompts, or shell transcripts.

4. Record and validate the pushed packet ref on the assignment.

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/operator/adjutant/assignments/$ADJUTANT_ASSIGNMENT_ID/task-packet" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  --data-binary @- <<JSON
{
  "commitSha": "$ADJUTANT_TASK_PACKET_COMMIT",
  "taskSpecPath": "$ADJUTANT_TASK_PACKET_PATH",
  "operatorNotes": "First Adjutant Site fulfillment smoke for Ben OTEC."
}
JSON
```

5. Preflight Adjutant with the public Sites launch checklist.

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/operator/adjutant/assignments/$ADJUTANT_ASSIGNMENT_ID/preflight" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  --data-binary @- <<'JSON'
{
  "includeCallbackLag": true,
  "launchChecklist": {
    "sourceReviewed": true,
    "buildReviewed": true,
    "audienceReviewed": true,
    "secretsReviewed": true,
    "urlReviewed": true
  }
}
JSON
```

Do not launch until every blocking check is resolved. Provider reconnect,
GitHub writeback, SHC health, callback config, migration state, packet ref,
Site/order existence, and public launch checklist failures are launch blockers.

6. Launch the Adjutant run.

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/operator/adjutant/assignments/$ADJUTANT_ASSIGNMENT_ID/launch" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  --data-binary @- <<'JSON'
{
  "includeCallbackLag": true,
  "runnerBackend": "shc_vm",
  "launchChecklist": {
    "sourceReviewed": true,
    "buildReviewed": true,
    "audienceReviewed": true,
    "secretsReviewed": true,
    "urlReviewed": true
  }
}
JSON
```

7. Monitor operator, customer, and public projections.

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/operator/adjutant/assignments/$ADJUTANT_ASSIGNMENT_ID" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json"
```

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/customer-orders/$ADJUTANT_SOFTWARE_ORDER_ID" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json"
```

```sh
curl -sS "$OPENAGENTS_BASE_URL/api/public/adjutant/activity" \
  -H "Accept: application/json"
```

8. Confirm the runner saved a reviewable Site version.

The operator review payload must show:

- `assignment.currentRunId` linked to the launched run;
- `review.order.id = $ADJUTANT_SOFTWARE_ORDER_ID`;
- `review.site.id = $ADJUTANT_SITE_ID`;
- at least one `review.versions[]` row with
  `sourceKind = autopilot_generated`, `buildStatus = saved`, and
  `createdByRunId` set;
- `review.usageReceipts[]` entries for generation and any artifact storage;
- customer-safe `usageReceipts[]` in the customer order projection;
- only sanitized public activity at `/api/public/adjutant/activity`.

9. Deploy only after operator review.

```sh
export ADJUTANT_SITE_VERSION_ID="<site_version_id>"
curl -sS "$OPENAGENTS_BASE_URL/api/operator/sites/$ADJUTANT_SITE_ID/versions/$ADJUTANT_SITE_VERSION_ID/deploy" \
  -H "Cookie: $OPENAGENTS_ADMIN_COOKIE" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  --data-binary @- <<'JSON'
{
  "confirm": true,
  "launchChecklist": {
    "sourceReviewed": true,
    "buildReviewed": true,
    "audienceReviewed": true,
    "secretsReviewed": true,
    "urlReviewed": true
  }
}
JSON
```

10. Verify the launch.

```sh
curl -I https://sites.openagents.com/otec
curl -sS "$OPENAGENTS_BASE_URL/api/public/adjutant/activity" \
  -H "Accept: application/json"
```

Browser checks:

- `https://openagents.com/admin` shows the Adjutant assignment, review panel,
  saved version, active deployment, and usage receipts.
- `https://openagents.com/order` shows the customer-safe Adjutant status, Site
  URL, and usage summary for Ben when logged in as the customer.
- `https://openagents.com/adjutant` and
  `https://openagents.com/agents/adjutant` show only safe public progress.
- `https://sites.openagents.com/otec` serves the reviewed Site.

## Recovery

- If preflight blocks, resolve the named check and run preflight again.
- If callbacks lag, use the operator callback retry path by run ID before
  manually inspecting SHC state.
- If the run stops without a deployable version, request an Adjutant adjustment
  against the same assignment and durable goal instead of launching an
  unrelated duplicate.
- If a saved version is unsafe, reject or replace it through the Sites review
  lifecycle; do not deploy to the public Site target.

## Closeout

Record the launch closeout in the relevant issue or operator note:

- assignment ID;
- goal ID;
- run ID;
- Site ID;
- saved version ID;
- deployment ID and URL;
- test commands run;
- customer/public projection links checked;
- blockers or follow-up adjustments.
