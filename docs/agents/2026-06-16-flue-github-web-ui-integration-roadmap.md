# Flue GitHub Connector And Web UI Integration Roadmap

Date: 2026-06-16
Status: implementation roadmap, no implementation yet
Scope: GitHub-first Flue connector integration tied into the
`apps/openagents.com` Worker and web UI.

## Decision

Start with GitHub, make it visible in the OpenAgents web UI early, and keep
OpenAgents as the authority.

The first production-shaped integration should not be a hidden webhook demo. It
should make `/settings/connections` show a concrete GitHub connector state,
then prove that a GitHub issue or PR event can become a durable, public-safe
OpenAgents connector event. Only after that should the agent post back to
GitHub, and only through a bound writeback path selected by trusted
OpenAgents code.

## Target Outcome

A signed-in user or operator can open `Settings > Connections` and see:

- GitHub identity and repository-write status from the existing OpenAgents
  GitHub connection flow.
- The default repository already selected for onboarding/work orders.
- GitHub connector installation or watch status for selected repositories.
- Recent webhook delivery health and typed blockers.
- A link from a watched GitHub issue/PR to the corresponding OpenAgents
  workroom, workspace, or work-order event stream.

An initial internal repo can then prove the loop:

```text
GitHub issue or PR event
  -> apps/flue-connectors GitHub channel verifies the webhook
  -> delivery id is claimed for idempotency
  -> provider payload is reduced to a sanitized event envelope
  -> OpenAgents Worker records connector event and maps it to a repo/work order
  -> web UI shows the event in Settings and the related workroom/workspace
  -> agent drafts a response
  -> approved/bound writeback posts to the fixed GitHub issue or PR
```

## Non-Goals

- Do not replace the `openagents.com` Worker with Flue.
- Do not move login, team membership, workspace access, billing, email,
  public-promise, settlement, payout, or accepted-work authority into Flue.
- Do not expose generic model-callable GitHub tools.
- Do not let Flue store GitHub OAuth tokens, broad installation credentials,
  private repo content, raw webhook bodies, raw prompts, or raw runner logs in
  durable agent history.
- Do not use GitHub webhook ingestion to bypass the strict GitHub issue intake
  policy. Loose reports remain Forum-first unless they satisfy the strict bug
  form.

## Existing OpenAgents Anchors

Use these rather than creating parallel surfaces:

- Web UI settings route:
  `apps/openagents.com/apps/web/src/page/loggedIn/page/settings.ts`
  already renders `SettingsSection(connections)`, provider account status, and
  the default GitHub repository picker.
- Existing GitHub write connection routes:
  `/auth/github/write/start`, `/api/github-write/connections`,
  `/api/github-write/connections/:connectionRef/disconnect`, and
  `/api/github-write/grants/resolve`.
- Existing GitHub write authority:
  `apps/openagents.com/workers/api/src/github-write-connections.ts` stores
  connection metadata, secret refs, scoped grants, and grant expiry.
- Existing repository/work-order model:
  `apps/openagents.com/workers/api/src/omni-runs.ts` and
  `apps/openagents.com/workers/api/src/autopilot-work-request.ts`.
- Existing workspace access boundary:
  `apps/openagents.com/workers/api/src/prefilled-workspace.ts` and
  `apps/openagents.com/workers/api/src/prefilled-workspace-routes.ts`.
- Invariant authority:
  `apps/openagents.com/INVARIANTS.md`, especially GitHub writeback authority,
  public projection staleness, provider-account credential boundaries, and
  private workspace access.

## Architecture

OpenAgents owns identity, repo mapping, workspace mapping, GitHub write
authority, UI projections, and public-safe event storage. Flue owns only
provider ingress and the agent harness for connector sessions.

```text
GitHub webhook
  -> Flue GitHub channel verifies signature
  -> Sidecar claims delivery id
  -> Sidecar reduces payload to sanitized connector envelope
  -> OpenAgents internal connector API records the event
  -> OpenAgents maps repo/issue/PR to work order, workspace, or support context
  -> OpenAgents exposes public-safe connector projections to the web UI
  -> Flue issue/PR agent receives sanitized input
  -> Agent can call only bound tools
  -> Bound tool calls OpenAgents internal writeback API
  -> OpenAgents uses selected GitHub authority to post or block
```

Cloudflare deployment should keep this as a separate Worker at first:

```text
apps/flue-connectors/
  .flue/
    agents/
    channels/
    skills/
  src/
    internal-client.ts
    redaction.ts
  wrangler.jsonc
  README.md
```

Use Cloudflare Worker service bindings or a signed internal HTTP route for
sidecar-to-OpenAgents calls. Current Cloudflare docs describe service bindings
and Worker RPC as a way for one Worker to call another without exposing a public
URL. If the sidecar gets Durable Object-backed Flue agents, its `wrangler.jsonc`
must carry top-level Durable Object migrations. D1 bindings and local
development should follow the existing Wrangler/D1 pattern; local secrets belong
in `.dev.vars` and must not be committed.

## Data Model

Prefer OpenAgents-owned records for anything the web UI or authority logic reads.
The sidecar may keep minimal ingress state for idempotency and Flue runtime
state, but it is not the source of truth for product state.

OpenAgents Worker records:

- `connector_integrations`: provider, owner user/team/project ref, status,
  created/updated refs, disabled state.
- `github_connector_repositories`: provider repo ref, owner/name, visibility
  class, default branch, selected installation/connection ref, workspace or
  work-order routing ref, enabled state.
- `connector_events`: provider, delivery id, event kind, repo ref, issue/PR ref,
  sanitized title/body excerpt, mapped OpenAgents subject refs, idempotency ref,
  generatedAt, staleness contract, blockers.
- `connector_agent_sessions`: connector event refs mapped to Flue session ids,
  sanitized state only.
- `connector_writeback_requests`: draft, approval, posted, blocked, failed; fixed
  destination refs; authority receipt refs; provider response summary only.

Sidecar records:

- `github_webhook_deliveries`: delivery id, event type, action, receivedAt,
  accepted/rejected/duplicate status.
- `flue_dispatches`: delivery id, agent session id, dispatch status, error
  summary.

## Web UI Shape

Do not add a new top-level GitHub app first. Extend the existing
`Settings > Connections` page.

Initial panels:

- `GitHub account`: signed-in GitHub identity, or email-only account blocker.
- `Repository writeback`: existing GitHub write connection status and
  connect/disconnect actions.
- `Default repository`: existing picker/manual owner-repo save flow.
- `GitHub connector`: installation/watch status, webhook health, watched
  repositories, latest delivery, blockers.
- `Provider account pool`: existing provider-account pool panel remains below or
  beside these cards.

Follow-on UI:

- Workroom/workspace event strip for connector events mapped to that context.
- Issue/PR detail drawer with event summary, source GitHub URL, generatedAt,
  staleness contract, and blocker refs.
- Draft writeback review row with approve/post/cancel only after the bound
  writeback API exists.

Implementation should follow the current Foldkit pattern: schema-backed model
fields, message facts, commands for API calls, `SettingsSection(connections)`,
Fireball icon catalog, Tailwind utility classes, and no raw History API calls.

## Issue Order

Open and work these in order. The order is the dependency graph.

### FGH-00 - Add The GitHub Connector Invariant

Owner area: `apps/openagents.com/INVARIANTS.md`

Scope:

- Add a short invariant for Flue connector ingress.
- State that provider ingress is evidence only.
- State that GitHub destination, credential, repo, issue/PR, workspace, and
  writeback authority are selected by OpenAgents, not by the model.
- State that connector projections carry `generatedAt` and the shared staleness
  contract.

Acceptance:

- Invariant names the first tests that will enforce it.
- No production behavior changes.

### FGH-01 - Define The OpenAgents GitHub Connector Projection Contract

Owner area: `apps/openagents.com/workers/api/src`

Scope:

- Add schema types for GitHub connector status and recent connector events.
- Define the UI-facing response shape for
  `GET /api/connectors/github/status`.
- Include `generatedAt`, staleness contract, connection status, watched repo
  summaries, recent deliveries, and typed blockers.
- Exclude raw webhook payloads, tokens, private repo content, raw comments beyond
  bounded excerpts, raw prompts, and provider response bodies.

Acceptance:

- Unit tests reject secret-shaped and raw payload fields.
- Contract can render the Settings panel without special cases.
- Empty state is explicit: disconnected, uninstalled, no watched repositories,
  or unavailable.

### FGH-02 - Surface Existing GitHub Write Connection In Settings

Owner area: `apps/openagents.com/apps/web/src/page/loggedIn/page/settings.ts`

Scope:

- Make `Settings > Connections` clearly show existing GitHub write connection
  state from `/api/github-write/connections` or bootstrap data.
- Add a visible connect action that uses `/auth/github/write/start`.
- Keep disconnect wired through the existing disconnect route.
- Keep email-only accounts blocked with a typed explanation: no GitHub identity
  is available to connect.

Acceptance:

- User can see whether GitHub writeback is connected, disconnected, unhealthy,
  or requires reauth.
- No new GitHub token is exposed to the browser.
- UI tests cover connected, disconnected, pending, and email-only states.

### FGH-03 - Add The GitHub Connector Status Endpoint

Owner area: `apps/openagents.com/workers/api/src`

Scope:

- Implement `GET /api/connectors/github/status`.
- Require browser session.
- Return only the signed-in user's or active team's connector state.
- Join existing GitHub write connection state where useful, but do not make
  write connection presence equivalent to webhook installation.

Acceptance:

- Unauthenticated returns `401`.
- Email-only accounts can read a status response with GitHub identity blocker.
- Connected GitHub users see write connection state and empty connector install
  state.
- Response carries `generatedAt` and staleness.

### FGH-04 - Add The Settings GitHub Connector Panel

Owner area: `apps/openagents.com/apps/web/src/page/loggedIn`

Scope:

- Add model state, messages, commands, and view for the new status endpoint.
- Load the status when entering `/settings/connections`.
- Render status, watched repos, recent deliveries, and blockers.
- Keep it passive in this issue: no install or writeback controls yet.

Acceptance:

- Settings route shows GitHub connector state from the endpoint.
- Loading/failure/empty states are stable and do not resize controls.
- Scene or route tests cover the panel.

### FGH-05 - Define Internal Connector Event API

Owner area: `apps/openagents.com/workers/api/src`

Scope:

- Add an internal-only API or service-binding RPC method for connector event
  admission.
- Start with GitHub events: `ping`, `issues`, `issue_comment`,
  `pull_request`, and `pull_request_review_comment`.
- Require a service actor, service binding, or signed internal token. Browser
  sessions are not accepted for this route.
- Claim idempotency by provider delivery id.
- Store sanitized connector event records.

Acceptance:

- Invalid actor fails closed.
- Duplicate delivery id does not create a second event.
- Unsupported event types are recorded as ignored or rejected without dispatch.
- Raw payload storage is blocked by tests.

### FGH-06 - Scaffold `apps/flue-connectors`

Owner area: new `apps/flue-connectors`

Scope:

- Add the isolated Flue Cloudflare Worker skeleton.
- Add health route and internal smoke route only.
- Add Wrangler config, D1 binding for sidecar delivery idempotency, and Flue
  Durable Object migration config if generated by Flue.
- Add `.dev.vars.example` with names only.
- Document local dev and deploy commands.

Acceptance:

- Builds and runs locally.
- No provider credentials committed.
- No imports from `apps/openagents.com` runtime internals.
- Sidecar can call a no-op OpenAgents internal connector API in local smoke.

### FGH-07 - Add GitHub Webhook Verification And Dedupe In Flue

Owner area: `apps/flue-connectors`

Scope:

- Add `@flue/github` channel.
- Configure `/channels/github/webhook`.
- Verify `GITHUB_WEBHOOK_SECRET`.
- Handle `ping` with fast `2xx`.
- Claim `deliveryId` before dispatch.
- Reduce provider payloads into sanitized envelopes before crossing to
  OpenAgents.

Acceptance:

- Valid signature accepted.
- Invalid signature rejected.
- Duplicate delivery id is not dispatched twice.
- `ping` does not create an agent session.
- Tests include issue comment and PR review comment payload fixtures with raw
  body redaction.

### FGH-08 - Map GitHub Events To OpenAgents Subjects

Owner area: `apps/openagents.com/workers/api/src`

Scope:

- Map `{ owner, repo, issue/PR number }` to known repository refs, work-order
  refs, project refs, or support contexts.
- Start with explicit watched repository mappings, not heuristic prompt or
  keyword routing.
- Return typed blockers for unknown repo, unsupported private repo, missing
  membership, missing write authority, and disabled integration.

Acceptance:

- Mapping uses structured repo refs and stored mappings.
- No ad hoc keyword routing.
- Unknown repo is visible as a blocker in Settings.
- Private repo event cannot reach a workspace projection without membership
  authorization.

### FGH-09 - Dispatch Issue/PR-Scoped Flue Agent Sessions

Owner area: `apps/flue-connectors`

Scope:

- Create an issue/PR-scoped Flue agent key from trusted event refs.
- Dispatch only sanitized event envelopes.
- Persist only event refs and bounded excerpts in Flue history.
- Add a read-only OpenAgents status tool that fetches safe work-order or
  workspace summaries through the internal API.

Acceptance:

- Agent session id is deterministic for repo plus issue/PR thread.
- Model context excludes raw webhook body, tokens, private repo content, and raw
  comments beyond approved excerpts.
- Read-only status tool cannot choose arbitrary workspace, repo, or issue.

### FGH-10 - Show GitHub Connector Events In Workroom/Workspace UI

Owner area: `apps/openagents.com/apps/web/src`

Scope:

- Add connector event projection to the relevant workroom, team chat, or
  workspace surface.
- Start with read-only events: source, event kind, issue/PR link, short excerpt,
  generatedAt, and blockers.
- Do not expose writeback buttons yet.

Acceptance:

- A GitHub issue comment can appear as a public-safe connector event in the
  mapped OpenAgents context.
- User can navigate from Settings recent deliveries to the mapped context.
- Private-team events require private-team membership.

### FGH-11 - Add Bound Draft Writeback

Owner area: `apps/openagents.com/workers/api/src` and `apps/flue-connectors`

Scope:

- Add bound tool shape:
  `draft_comment_on_bound_github_issue({ body })`.
- Destination repo/issue/PR is fixed by trusted OpenAgents event mapping.
- Store the draft in `connector_writeback_requests`.
- Do not post to GitHub in this issue.

Acceptance:

- Agent cannot choose owner, repo, issue, PR, token, or GitHub API method.
- Draft is visible in operator/web UI with source event refs.
- Secret and raw payload checks run on draft metadata.

### FGH-12 - Add Human Approval And GitHub Comment Post

Owner area: `apps/openagents.com/workers/api/src`

Scope:

- Add approve/post/cancel endpoints for a draft writeback request.
- Use existing GitHub write connection/grant authority or an explicit GitHub App
  installation token selected by OpenAgents.
- Post only to the fixed destination.
- Store provider response summary and public-safe GitHub URL.

Acceptance:

- Missing approval blocks post.
- Missing/expired/unusable GitHub authority blocks post.
- Duplicate approval does not post twice.
- Posted URL appears in the related workroom/workspace event.

### FGH-13 - Add GitHub Connector Install/Watch Controls

Owner area: `apps/openagents.com/apps/web/src` and
`apps/openagents.com/workers/api/src`

Scope:

- Add Settings controls to watch/unwatch repositories for connector events.
- For the pilot, allow operator-enabled watched repositories first.
- Then add self-serve GitHub App installation if needed for customer repos.
- Keep repository mapping explicit: selected repo, optional project/workspace,
  event types, enabled state.

Acceptance:

- User can see which repos are watched.
- Operator can enable the first internal repo without a customer-facing install
  flow.
- Self-serve install is behind a separate gate and does not block the first
  webhook pilot.

### FGH-14 - Add Connector Observability And Admin Inspection

Owner area: both Workers

Scope:

- Add structured event logs for accepted, rejected, duplicate, dispatched,
  draft-created, posted, and blocked states.
- Add operator-only inspection for recent connector events and failures.
- Keep raw payloads out of logs and inspection responses.

Acceptance:

- Production debugging can identify delivery id, event kind, repo ref, mapping
  result, and blocker refs.
- No private payload or token material is logged.

### FGH-15 - End-To-End GitHub Connector Smoke

Owner area: both Workers and web UI

Scope:

- Retain a smoke runbook for the internal pilot repo.
- Prove valid webhook ingestion, idempotency, OpenAgents event projection,
  Settings visibility, workroom/workspace visibility, draft creation, approval,
  and posted GitHub comment.
- Record blocked cases for invalid signature, duplicate delivery, unknown repo,
  missing write authority, and private-team unauthorized viewer.

Acceptance:

- Smoke has a no-write mode and an approved-write mode.
- Evidence refs are public-safe.
- Failures are typed blockers, not silent drops.

## First Shippable Slice

The first slice worth shipping is FGH-00 through FGH-08 plus the read-only part
of FGH-10:

1. Settings shows GitHub write connection state and connector empty state.
2. Operator enables one watched repo.
3. GitHub `issue_comment` webhook reaches Flue, verifies signature, claims the
   delivery id, and sends a sanitized event to OpenAgents.
4. OpenAgents stores the event, maps it to a repo/work-order context, and shows
   the delivery in Settings and the relevant workroom/workspace.
5. No GitHub writeback happens yet.

That proves the connector path without risking external mutation.

## Second Slice

FGH-09 through FGH-12:

1. Flue creates a durable issue/PR-scoped agent session.
2. The agent can read only a bound OpenAgents status summary.
3. The agent creates a draft comment, not a posted comment.
4. A human approves the draft in OpenAgents.
5. OpenAgents posts through the existing GitHub write authority or an explicitly
   selected installation token.

That proves bounded writeback.

## Third Slice

FGH-13 through FGH-15:

1. Add watched-repo controls.
2. Add self-serve GitHub App installation only if operator-enabled watch is not
   enough.
3. Add admin inspection and retained smoke evidence.
4. Promote from one internal repo to a small allowlisted customer/design-partner
   set.

## Rollout Gates

Do not broaden the integration until these are true:

- GitHub webhook signature tests pass.
- Delivery idempotency tests pass in both sidecar and OpenAgents admission.
- Settings projection carries `generatedAt` and the staleness contract.
- Private workspace membership is checked before any private event projection.
- Bound tools cannot choose arbitrary repo, issue, PR, token, or provider method.
- GitHub writeback has approval, idempotency, and missing-authority blockers.
- No raw webhook body, token, private repo content, raw prompt, raw shell log, or
  provider response body is stored in public-safe projections.

## Defer Until GitHub Is Proven

- Slack companion.
- Resend connector workflows.
- Stripe connector workflows.
- Generic MCP connector exposure.
- Generic repository or project routing.

Those should not start until the GitHub sidecar has passed signature,
idempotency, mapping, UI projection, bound-tool, and writeback approval tests.

## Cloudflare Notes

Checked against Cloudflare docs on 2026-06-16:

- Durable Object bindings belong in `durable_objects.bindings`; class changes
  require top-level `migrations` entries with unique tags.
- D1 bindings are configured through Wrangler and are simulated locally by
  default under `wrangler dev`.
- Worker service bindings/RPC can be used for internal Worker-to-Worker APIs
  without exposing a public URL.
- Sensitive local development values belong in `.dev.vars` or `.env`, not in
  committed Wrangler `vars`.
