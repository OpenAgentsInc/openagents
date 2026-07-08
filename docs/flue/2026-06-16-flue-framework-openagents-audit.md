# Flue Framework Audit For OpenAgents

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-16  
Status: architecture audit, no implementation yet  
Scope: Flue as a possible agent harness and third-party connector layer for
OpenAgents product, private workspaces, operator workflows, and launch/business
operations.

## Executive Summary

Flue is worth a serious pilot for OpenAgents, but not as a replacement for the
current `openagents.com` Worker, payment authority, workspace authority, or
Effect/Foldkit product architecture.

The best fit is a separate connector/agent sidecar: a Flue Worker that receives
verified Slack, GitHub, Linear, Notion, Resend, Stripe, Shopify, Twilio,
WhatsApp, Zendesk, and similar provider events, dispatches them into durable
agent sessions, and calls back into OpenAgents through narrow internal APIs.
That gives us a programmable agent harness for third-party work without moving
our trusted system of record out of the current OpenAgents Worker.

Yes, Flue can help with Slack/GitHub and related third-party connections. The
important nuance is that Flue channels are inbound verified-event adapters, not
universal outbound clients. Outbound API calls still use each provider's normal
SDK in application-owned code. That is the right boundary for us: the model can
select bounded content such as a reply body, while trusted OpenAgents code binds
the workspace, repository, issue, payment account, project, team, and
credential.

Recommended posture:

1. Do not use Flue for the immediate private-workspace invite or Stripe launch
   gate. Those are already implemented in, or belong directly inside, the
   `openagents.com` Worker because they are auth, team, payment, and ledger
   authority.
2. Start a small `apps/flue-connectors` pilot as a separate Cloudflare Worker
   after the current private-workspace and Stripe work is stable.
3. First pilot: GitHub issue/comment/PR webhook ingress for support,
   Autopilot work-order writeback, work-order triage, and project workspace
   GitHub events. Bind Octokit tools only to the repository and issue/PR selected
   by trusted code.
4. Second pilot: Slack app-mention and slash-command ingress for private
   project/team workspaces, plus a bound `replyInThread` tool. Dispatch into
   a project-scoped Flue agent, but keep project membership, invite state, and
   billing checks in OpenAgents.
5. Treat Stripe and Resend Flue channels as optional workflow triggers only.
   Payment crediting, checkout, webhook verification, email ledgers, auth OTP,
   and product lifecycle send authority should stay in the current Worker.

## Sources Reviewed

- Flue AI-guided starter skill: <https://flueframework.com/start.md>
- Flue homepage: <https://flueframework.com/>
- Getting Started / quickstart: <https://flueframework.com/docs/getting-started/quickstart/>
- Why Flue: <https://flueframework.com/docs/introduction/why-flue/>
- Durable Agents: <https://flueframework.com/docs/concepts/durable-execution/>
- Channels guide: <https://flueframework.com/docs/guide/channels/>
- Ecosystem overview: <https://flueframework.com/docs/ecosystem/>
- Slack channel guide: <https://flueframework.com/docs/ecosystem/channels/slack/>
- GitHub channel guide: <https://flueframework.com/docs/ecosystem/channels/github/>
- Cloudflare deployment guide:
  <https://flueframework.com/docs/ecosystem/deploy/cloudflare/>
- OpenAgents local context:
  - `docs/blitz/forge/2026-06-16-workspace-seeding-invite-engagement.md`
  - `docs/blitz/forge/2026-06-16-business-slack-connect-intake.md`
  - `docs/auth/2026-06-16-login-and-auth-audit.md`
  - `apps/openagents.com/INVARIANTS.md`
  - `apps/openagents.com/workers/api/src/prefilled-workspace.ts`
  - `apps/openagents.com/workers/api/src/billing.ts`
  - `apps/openagents.com/workers/api/src/index.ts`

## What Flue Is

Flue is a TypeScript agent framework built around a harness, not a prompt-only
SDK. Its own positioning is "durable AI agents and workflows" with sessions,
tools, skills, subagents, sandboxes, MCP servers, observability, channels, and
deploy targets such as Node and Cloudflare.

The useful primitives for OpenAgents are:

- **Agents:** continuing, stateful sessions that can receive prompts or
  asynchronous `dispatch(...)` inputs over time.
- **Durable execution:** on Cloudflare, Flue generates Durable Object-backed
  agents whose accepted inputs enter a durable per-instance queue and whose
  session history is persisted in SQLite.
- **Workflows:** finite jobs with a result. These are useful for batch or
  explicit one-shot operations, but they are not the same as continuing agent
  sessions.
- **Tools:** typed operations exposed to an agent. Flue's docs emphasize the
  correct security model: model-selected parameters are not an authorization
  boundary.
- **Channels:** provider webhook ingress. A channel verifies provider requests,
  parses native provider payloads, and calls application handlers.
- **MCP server connections:** a way to connect authenticated tools and services
  through MCP, with the same boundary requirement that credentials and account
  selection stay in trusted code.
- **Sandboxes:** virtual or remote execution environments for agents that need
  filesystem/shell execution.
- **Observability:** OpenTelemetry, Braintrust, Sentry, or custom observers for
  agent runs and workflows.

The docs are explicit that several pages are "AI-generated, awaiting review."
That does not disqualify the framework, but it means we should pilot against the
installed package and generated code before betting production launch gates on
it.

## Why It Matters For OpenAgents

OpenAgents has several converging needs:

- third-party event ingress from Slack, GitHub, Stripe, Resend, Linear, Notion,
  Shopify, Twilio, WhatsApp, Zendesk, and similar systems;
- project/private-workspace agents that can keep context across human and
  provider events;
- operator workflows that should be durable enough to survive disconnects and
  deployment interruptions;
- AI-native runbooks and skills that can be packaged, reused, and iterated on;
- connector code that should not be hand-stitched for every provider;
- strict authority boundaries around private workspace membership, payment,
  settlement, GitHub writeback, email, and public projection.

Flue is interesting because it can own the generic agent harness and provider
ingress mechanics while OpenAgents keeps business authority. That matches the
current architecture better than a monolithic "agent platform rewrite."

## Current OpenAgents Fit

OpenAgents already has several relevant systems:

- Email sign-in uses OpenAuth `CodeProvider` with direct Resend transport for
  auth OTP. Product lifecycle email and Resend webhooks exist behind the Worker
  email/ledger flow.
- GitHub login and GitHub write connection/grant flows exist in the Worker.
- Billing has Stripe checkout/return/setup-intent/webhook routes plus credit
  ledger application.
- Prefilled workspaces have explicit `public_safe` versus `private_team` access
  modes and private-team membership checks.
- Business signup has a Slack Connect opt-in, but Slack invite completion is
  currently manual/external.
- Product and runtime invariants require secrets, provider credentials, raw
  prompts, private repo content, wallet material, and private customer data to
  stay out of docs, fixtures, logs, and public projections.

Because these systems already exist, Flue should not be introduced as a new
source of truth. It should be introduced, if at all, as an event/agent
coordination plane that calls into those systems through typed, least-authority
OpenAgents APIs.

## Connector Boundary

Flue's channel model is the strongest reason to pilot it.

The channel guide draws a clean line:

- Channel package owns request authentication/signature verification, protocol
  handshakes, parsing, and discovered routes.
- Application code owns provider SDK clients, outbound credentials, OAuth
  install/token storage, token rotation, agent tools, authorization policy,
  delivery deduplication, and business persistence.

That boundary maps directly to OpenAgents' invariants.

For OpenAgents, a Flue connector must never expose a generic "call Slack",
"call GitHub", "charge Stripe", or "send email" tool to a model. It should
expose tools like:

- `reply_in_bound_slack_thread({ text })`
- `comment_on_bound_github_issue({ body })`
- `summarize_bound_workspace_status({ format })`
- `draft_bound_project_update({ audience })`

The trusted application chooses the Slack workspace/channel/thread, GitHub
owner/repo/issue, workspace/team/project, user, Stripe customer, and credential.
The model chooses only bounded content fields.

## Slack Assessment

Flue has a first-party Slack channel. The guide says the blueprint installs
`@flue/slack` plus Slack's official `@slack/web-api` SDK, publishes routes for
events, interactions, and slash commands when callbacks are enabled, and dispatches
verified Slack events into agent sessions.

Supported Slack surfaces:

- `/channels/slack/events`
- `/channels/slack/interactions`
- `/channels/slack/commands`

The useful pattern is a thread-scoped agent:

- Slack `app_mention` arrives.
- The channel verifies the signing secret and parses the native Slack event.
- Trusted code maps `{ teamId, channelId, threadTs }` to a Flue conversation key.
- The handler dispatches a sanitized input to the agent.
- The agent can use a bound `replyInThread` tool where the channel/thread is
  fixed by trusted code.

Benefits for OpenAgents:

- Converts the current Slack Connect manual handoff into an event-driven path.
- Gives every private project/team workspace an optional Slack companion without
  building a separate agent session system from scratch.
- Supports app mentions, slash commands, and interactions for operator commands
  like "summarize workspace", "draft update", "prepare demo call", or "open
  a follow-up task."
- Keeps short-lived Slack capabilities (`trigger_id`, `response_url`, modal
  response URLs) in immediate trusted request handling rather than durable
  session history.

Risks and boundaries:

- Slack OAuth installation storage, workspace authorization, Socket Mode, and
  token rotation remain app-owned. Flue does not solve those.
- Channel packages are stateless and do not deduplicate deliveries. We must
  claim Slack `event_id` in D1 before dispatch when duplicate admission matters.
- Slack workspace/channel IDs may be sensitive. They can be stored as internal
  refs, but should not be projected publicly or treated as authorization by
  themselves.
- This should not block the near-term workspace invite/email/payment tasks.

Verdict: strong pilot candidate.

## GitHub Assessment

Flue has a first-party GitHub channel. The guide says the blueprint installs
`@flue/github` plus `@octokit/rest`, creates a channel module with a named
`channel` and `client`, verifies webhook deliveries using `GITHUB_WEBHOOK_SECRET`,
and uses Octokit for outbound comments.

The useful pattern is an issue/PR-scoped agent:

- GitHub webhook arrives at `/channels/github/webhook`.
- The channel verifies the webhook secret and parses the native
  `@octokit/webhooks-types` payload.
- Trusted code branches on `delivery.name` and `delivery.payload.action`.
- Trusted code binds the repository and issue/PR number into a conversation key.
- The agent can use a bound `commentOnIssue` tool where owner/repo/issue are
  fixed by trusted code.

Benefits for OpenAgents:

- Better GitHub issue/comment/PR event ingress for Autopilot work orders,
  strict bug reports, release candidate feedback, and code-runner support.
- Agent sessions can remain attached to a specific issue or PR across comments.
- Octokit stays in app code, which matches our GitHub writeback invariants.
- Could reduce custom glue around comment triage, runbook drafting, PR status
  summaries, CI failure triage, and issue-to-workspace routing.

Risks and boundaries:

- GitHub expects a `2xx` response quickly; the handler should admit durable work
  and return rather than doing slow work inline.
- Flue does not deduplicate `deliveryId`; D1 must claim it before dispatch if
  duplicate effects are unacceptable.
- Existing GitHub OAuth/write-grant code in OpenAgents remains the authority.
  Flue should consume a narrow internal grant or app installation token selected
  by trusted code, not store broad user tokens in model context.
- Do not use Flue to bypass strict issue intake policy. Loose reports remain
  Forum-first unless they satisfy the strict bug form.

Verdict: immediate first pilot candidate. GitHub has the clearest near-term
OpenAgents value because it directly connects issues, PRs, work orders,
writeback commentary, strict bug follow-up, and project evidence without waiting
on Slack workspace install/acceptance flow.

## Resend And Email Assessment

Flue lists Resend as a first-party ecosystem channel, but OpenAgents already has
direct Resend auth OTP and product lifecycle email/webhook logic.

Recommended use:

- Do not move auth OTP to Flue.
- Do not move invite-email send authority to Flue.
- Do not move email deliverability ledger or webhook truth to Flue.
- Consider a Flue Resend channel only for non-authoritative follow-up workflows,
  such as "delivery failed, draft an operator note" or "summarize lifecycle
  email health" after the Worker ledger has already accepted the event.

Email is too close to authentication, private workspace invites, customer
communications, and legal/commercial trust to hand to a new agent harness before
the core route is proven.

Verdict: possible later trigger source, not a launch blocker and not a source of
truth.

## Stripe Assessment

Flue lists Stripe as a first-party ecosystem channel. OpenAgents already has
Stripe checkout, return, setup-intent, webhook, and credit-ledger application
paths.

Recommended use:

- Keep Stripe webhook verification, checkout state, credit application,
  customer mapping, ledger writes, chargeback handling, and payment projection
  in the current Worker.
- If Flue is used at all, use it only after a trusted payment event has been
  accepted by OpenAgents. Example: dispatch "workspace credited" to a project
  concierge agent so it can draft onboarding next steps.
- Never expose `create_checkout`, `issue_refund`, `apply_credit`, or
  `change_subscription` as model-callable tools unless the tool is a typed
  request for human/operator approval and the actual mutation remains in
  OpenAgents authority.

Verdict: do not use for core billing now. Later, use as a post-ledger workflow
trigger if helpful.

## Other Third-Party Channels

The ecosystem list includes Discord, Facebook/Messenger, Google Chat, Intercom,
Linear, Microsoft Teams, Notion, Salesforce Marketing Cloud, Shopify, Telegram,
Twilio, WhatsApp, and Zendesk.

Highest-value OpenAgents candidates:

- **Linear:** product/ops issue mirroring if a customer or investor workspace
  expects Linear-native updates.
- **Notion:** private workspace knowledge intake/export for teams already using
  Notion, with strict document-scope authorization.
- **Zendesk/Intercom:** support-ticket ingestion into project-specific agents.
- **Shopify:** e-commerce vertical workspace triggers around products, orders,
  returns, and customer-service workflows.
- **Twilio/WhatsApp:** high-touch business communication channels where the
  agent drafts but trusted code owns destination and send approval.
- **Microsoft Teams/Google Chat:** enterprise customer workspaces where Slack is
  not the collaboration surface.

In every case, inbound verification can be Flue-owned, but OAuth/install state,
destination binding, dedupe, private data policy, and outbound calls remain
OpenAgents-owned.

## MCP Assessment

Flue can connect MCP servers and expose MCP tools as ordinary agent tools. This
is useful for internal/operator tools and third-party systems that already speak
MCP.

OpenAgents should apply the same boundary:

- The app chooses which MCP server, credentials, workspace/project scope, and
  tool set are available.
- The model does not get to select arbitrary credentials, account IDs, repo URLs,
  payment targets, or workspace IDs.
- MCP tool results that contain private data should be summarized or stored in
  private project state, never public projections.

This could be useful for internal operator workflows, but should not be the
first production pilot. GitHub exercises the clearest immediate product value;
Slack follows once the connector boundary is proven.

## Cloudflare Deployment Fit

Flue supports Cloudflare Workers. The Cloudflare guide says Flue uses
Cloudflare's Agents SDK Durable Object base/lifecycle, generates Durable Object
classes and bindings, requires ordered Durable Object migrations, uses
`nodejs_compat`, and deploys via generated Wrangler config under `dist`.

This is compatible with OpenAgents' Cloudflare direction, but it is not a tiny
drop-in:

- Generated Durable Object classes and migrations become deployment state.
- Flue wants source layout discipline (`.flue`, `src`, or root; do not mix).
- Local development uses `flue dev --target cloudflare`, not `flue run`.
- Cloudflare auth must happen before Flue admission; original request headers,
  cookies, query, URL, and body are not preserved after the durable boundary
  unless trusted code carries non-secret correlation into input/storage.
- Secrets belong in `.dev.vars` locally and Wrangler secrets in production.
- Static asset Workers need `assets.run_worker_first` for application/Flue
  prefixes.

Because `apps/openagents.com` already has a large Worker with strict deploy
guards and Effect/Foldkit boundaries, the safest pilot is a separate Worker:

```text
apps/flue-connectors/
  .flue/
    agents/
    channels/
    skills/
  wrangler.jsonc
  package.json
  README.md
```

That keeps Flue's generated build/deploy/migration topology away from the main
product Worker until we have evidence. The sidecar can call internal OpenAgents
APIs over a signed service-token path or D1-bound internal interface, depending
on where it deploys.

## Proposed OpenAgents Architecture

```text
Provider webhook
  -> Flue channel verifies signature
  -> Flue handler claims provider delivery id in app-owned durable storage
  -> Trusted code maps provider event to OpenAgents project/team/workspace refs
  -> Trusted code authorizes membership/account/install state through OpenAgents
  -> dispatch(project-agent, sanitized input)
  -> Agent reasons with project instructions/skills
  -> Agent uses only bound tools
  -> Bound tool calls internal OpenAgents API or provider SDK with trusted refs
  -> OpenAgents Worker remains authority for workspace, payment, email, and projection
```

This architecture gives us:

- durable conversational continuity per Slack thread, GitHub issue, ticket, or
  private workspace;
- tested provider signature parsing through first-party Flue channels;
- a unified harness for skills, tools, subagents, and observability;
- no migration of payment/workspace/email/GitHub authority out of OpenAgents.

## Specific Pilot Plan

### Pilot 0: Isolated Connector Worker Skeleton

Goal: create the Flue sidecar without touching `openagents.com` authority or
deployment topology.

Build:

- `apps/flue-connectors` Cloudflare target.
- Flue source layout, generated Wrangler config, and Durable Object migration
  history documented.
- No provider credentials committed.
- A no-op health route and one internal-only smoke route.
- Shared redaction/idempotency helpers before provider channels land.

Acceptance:

- Builds and dry-runs with Flue's Cloudflare target.
- Does not import or modify `apps/openagents.com` runtime authority.
- No login, workspace, payment, email, settlement, or public-promise authority
  is present.
- The next change can add GitHub without changing the skeleton's authority
  boundary.

### Pilot 1: GitHub Issue/PR Agent

Goal: prove Flue can connect GitHub webhook events to bounded agent sessions
and post back only with trusted repo/issue authority.

Build:

- GitHub channel for `issue_comment` and `pull_request_review_comment`.
- Delivery-id dedupe.
- Mapping from repository/issue to OpenAgents work order or support context.
- Bound `comment_on_bound_issue({ body })` tool.
- Optional read-only tool to fetch OpenAgents work-order status.

Acceptance:

- Valid GitHub signature accepted; invalid rejected.
- `ping` acknowledged without dispatch.
- Duplicate delivery ID not repeated.
- Agent cannot select arbitrary owner/repo/issue.
- Existing GitHub write grant policy is not weakened.

### Pilot 2: Slack Private Workspace Companion

Goal: prove Flue can connect a provider event to a private project workspace
without leaking private data or broadening authority, after GitHub has proven
the sidecar, dedupe, bound-tool, and redaction pattern.

Build:

- Slack channel with events only: `app_mention` and, optionally, one slash
  command.
- D1 table or internal OpenAgents API for delivery-id claim.
- Workspace lookup by Slack installation/channel/thread mapping.
- Agent instance id derived from project/workspace plus Slack thread.
- Bound `reply_in_thread({ text })` tool.
- Bound read-only `get_workspace_brief()` tool returning a redacted private
  workspace projection authorized by OpenAgents.

Acceptance:

- Valid Slack signature accepted; invalid rejected.
- Duplicate Slack `event_id` does not double-dispatch or double-post.
- Model cannot choose channel, workspace, team, user, token, or provider method.
- Private workspace data is never logged, stored in public docs, or returned to
  unauthorized channels.
- Operator can disable the integration for a workspace.

### Pilot 3: Connector-Triggered Project Updates

Goal: after GitHub/Slack work, evaluate non-authoritative Resend/Stripe events
as triggers.

Build:

- Dispatch from OpenAgents Worker to Flue after trusted ledger events, not
  directly from raw billing/auth webhooks.
- Agent drafts a project update or operator checklist.
- Human/operator approval required before external send or account mutation.

Acceptance:

- Payment and email ledgers remain Worker-owned.
- Agent output is a draft or recommendation unless a separate approval path
  authorizes send/mutation.

## Security And Privacy Rules

These rules should be part of any Flue pilot before code lands:

- No provider secrets in model context, logs, dispatch input, durable session
  history, docs, fixtures, or issue comments.
- No raw webhook bodies in durable agent history.
- No Slack `response_url`, Slack `trigger_id`, modal response URLs, OAuth tokens,
  GitHub tokens, Stripe secrets, Resend keys, session cookies, wallet material,
  or private repo content in agent context.
- Provider delivery IDs should be stored for dedupe/tracing, but they do not
  authorize any action by themselves.
- Workspace/project/team membership must be checked in OpenAgents before dispatch
  or before any bound tool reads private material.
- Outbound tools must bind destination and credential in trusted code.
- Public projections from Flue-derived work must carry `generatedAt` and the same
  staleness/public-safety contracts as existing OpenAgents projections.
- If an agent is uncertain whether an external effect happened, it must not
  replay the effect blindly. Use idempotency keys and app-owned ledgers.
- Do not use Flue to work around existing strict GitHub issue, product-promise,
  payment, settlement, or private-team access policy.

## Observability

Flue's observability integrations are attractive for agent run debugging. The
homepage/docs list OpenTelemetry, Braintrust, Sentry, and custom observers.

Recommended path:

- Start with structured internal event logs and Cloudflare logs.
- Add OpenTelemetry only after the first connector has useful volume.
- Do not send private workspace material, provider tokens, raw prompts, raw
  webhook bodies, or private repository content to external observability tools.
- Treat traces as operational metadata, not product promises or acceptance
  receipts.

## Benefits

### Product Benefits

- Faster third-party connector rollout.
- A single pattern for Slack/GitHub/ticket/chat events entering project agents.
- Better private-workspace experience: work happens where the customer already
  talks, without making OpenAgents a generic chat app clone.
- Richer legal, e-commerce, agency, and investor workspaces through channel
  companions.
- More credible demo/investor story: OpenAgents connects business tools to
  durable project agents with clear authority boundaries.

### Engineering Benefits

- Less one-off webhook glue.
- Durable per-conversation agent sessions on Cloudflare.
- Skills and tools as first-class authored artifacts.
- Sandboxes available where agents need controlled file/shell work.
- Provider SDK outbound calls stay normal TypeScript application code.
- Strong fit with agent-readable runbooks and coding-agent-assisted setup.

### Business Benefits

- Slack/GitHub integrations are high-signal for design partners and technical
  customers.
- Legal workspaces can get private Slack/GitHub-adjacent workflow without
  generalizing partner-specific material into the broad funnel.
- Investor and agency workspace demos can show third-party events becoming
  auditable project progress.
- Easier path to channel-specific upsells: Slack companion, GitHub work-order
  bridge, Shopify commerce workspace, Zendesk support workspace, etc.

## Costs And Risks

- Flue is new and currently presented as 1.0 beta. Several docs pages are marked
  AI-generated and awaiting review.
- Adding Flue to the main Worker too early could disturb the existing
  Cloudflare deploy topology and Effect/Foldkit guardrails.
- Generated Durable Object migrations add operational state that must be
  managed carefully.
- Channel packages do not deduplicate deliveries; we must implement D1 or
  OpenAgents-owned idempotency.
- Slack/GitHub OAuth/install/token storage is still ours.
- A generic tool exposure would be dangerous. The pilot must use bound tools.
- Agent durable session history is not an appropriate storage layer for private
  system-of-record data.
- Workflow runs are finite invocations and not resumable in the same way as
  continuing agents. Use the right primitive.

## Recommendation

Adopt Flue experimentally, not foundationally, with this decision record:

- **Use Flue for:** provider event ingress, continuing project/thread/issue
  agents, bounded tools, skills, subagents, sandboxed agent jobs, and connector
  observability.
- **Do not use Flue for:** login, invite authority, private workspace membership,
  payment checkout/crediting, Stripe ledger authority, Resend auth OTP, public
  promise authority, settlement, payout, provider token storage, or product UI.
- **First production-shaped target:** GitHub issue/PR work-order bridge in a
  separate Cloudflare Worker.
- **Second target:** Slack private workspace companion after GitHub proves the
  sidecar's signature, dedupe, authorization, and bound-tool boundary.
- **Defer:** Stripe/Resend channel usage until after the current Worker has
  accepted the authoritative event.

The immediate private-workspace and payment launch work should continue in the
current OpenAgents Worker. Flue becomes a follow-on connector layer that makes
GitHub work orders and private workspaces feel live inside the systems where
teams already operate, with GitHub first and Slack second.

## Suggested Issues

If we decide to proceed, open these as implementation issues:

1. **Create isolated Flue connector Worker skeleton**
   - Add `apps/flue-connectors` with Cloudflare target, no provider credentials
     committed, generated migrations documented, and a no-op health route.
2. **Add GitHub issue/PR connector pilot**
   - Verified GitHub webhook, delivery dedupe, issue/PR-scoped agent, and bound
     issue-comment tool.
3. **Add Slack private-workspace companion pilot**
   - Verified Slack events, D1 delivery dedupe, workspace mapping, sanitized
     dispatch, and bound thread reply tool.
4. **Define OpenAgents internal connector API**
   - Minimal signed service API for workspace lookup, private projection reads,
     and append-only connector event recording.
5. **Write Flue connector security invariant**
   - Add invariant notes and tests before any private data or outbound provider
     mutation is exposed.

Do not schedule Stripe/Resend Flue work until the connector skeleton plus the
GitHub pilot, and preferably the Slack pilot, have passed signature, dedupe,
authorization, and redaction tests.

## Bottom Line

Flue gives OpenAgents a credible, TypeScript-native way to turn third-party
provider events into durable project agents. The useful path is not "replace our
Worker with Flue"; it is "let Flue handle connector ingress and agent harness
mechanics while OpenAgents remains the authority for auth, teams, workspaces,
payments, ledgers, and public claims."

That is a strong fit for GitHub first and Slack second. It is a later, more
cautious fit for Resend and Stripe. It should be piloted as an isolated
Cloudflare Worker before it is allowed anywhere near launch-critical private
workspace or billing authority.
