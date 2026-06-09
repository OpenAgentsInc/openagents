# Slack Team Message Ingestion Audit

Date: 2026-06-06

Status: architecture audit. This document does not add a live Slack app,
OAuth route, event endpoint, database migration, data export, Slack write, or
provider-token store.

## Request

OpenAgents users can already opt into GitHub and ChatGPT/Codex connection
flows. The next connector should let them authorize an OpenAgents Slack bot that
can ingest messages from their team and make those messages useful inside OpenAgents product surface
workrooms.

## Executive Summary

Slack should be a team-scoped connector installation, not a first-party login
identity and not a generic runner provider account.

The correct product shape is:

```text
OpenAgents signed-in user
  -> selects an OpenAgents product surface team or project
  -> starts Slack OAuth v2 installation
  -> Slack workspace admin/member authorizes the OpenAgents app
  -> OpenAgents product surface stores Slack installation metadata plus a server-side secret ref
  -> selected Slack channels become source-authorized connector feeds
  -> Events API messages create Slack source refs and extracted spans
  -> workrooms retrieve bounded Slack context through the source bundle layer
  -> explicit Slack mentions, slash commands, or OpenAgents product surface UI actions create runs
```

The MVP should be event-first and channel-opt-in:

- install one Slack app into one workspace;
- bind the Slack workspace to one OpenAgents product surface team;
- prioritize private channels the connected user is already part of and will
  add the bot to;
- choose explicit public or private channels to monitor;
- require the bot to be present in each selected channel before ingesting;
- ingest live message events into team/private source records;
- create workroom context bundles from Slack refs, not from hidden prompt
  stuffing; and
- launch Autopilot only from explicit triggers such as `@autopilot`, a slash
  command, a message shortcut, or an OpenAgents product surface UI action.

Do not promise "all team messages" across the whole workspace in the first
implementation. The MVP promise should instead be "all selected private
channels I am a member of and have added the bot to," plus selected public
channels. A normal Slack bot can only reliably ingest conversations where the
app is authorized and has access. Broad historical imports are also constrained
by Slack Web API rate limits. As of this audit, Slack says new commercially
distributed, non-Marketplace apps are limited to one `conversations.history`
request per minute with small page sizes for history/replies. That makes
narrow per-channel backfill acceptable and full-workspace archive import the
wrong MVP.

## Sources Reviewed

Local workspace and OpenAgents product surface sources:

- `/Users/christopherdavid/work/AGENTS.md`
- `/Users/christopherdavid/work/INVARIANTS.md`
- `/Users/christopherdavid/work/docs/omni/README.md`
- `/Users/christopherdavid/work/docs/omni/vortex-business-workrooms-synthesis.md`
- `/Users/christopherdavid/work/docs/omni/vortex-knowledge-data-workbench-synthesis.md`
- `openagents/AGENTS.md`
- `openagents/INVARIANTS.md`
- `openagents/docs/2026-06-02-chatgpt-codex-account-connection-opencode-openauth-audit.md`
- `openagents/docs/2026-06-02-provider-account-implementation-notes.md`
- `openagents/docs/2026-06-03-team-project-rooms.md`
- `openagents/docs/2026-06-03-team-room-shared-history-autopilot-audit.md`
- `openagents/docs/2026-06-06-openagents-data-classification-policy-v2.md`
- `openagents/docs/2026-06-06-redaction-regression-suite.md`
- `openagents/docs/2026-06-06-audit-export-contracts.md`
- `openagents/docs/omni/2026-06-06-knowledge-source-bundle-and-span-model.md`
- `openagents/docs/omni/2026-06-05-data-classification-and-trust-v1.md`
- `openagents/packages/provider-account-schema/src/index.ts`
- `openagents/workers/api/migrations/0009_provider_accounts.sql`
- `openagents/workers/api/migrations/0011_github_write_connections.sql`
- `openagents/workers/api/src/provider-account-routes.ts`
- `openagents/workers/api/src/github-write-connections.ts`
- `openagents/apps/web/src/page/loggedIn/page/settings.ts`

Slack platform sources:

- Slack OAuth installation:
  <https://docs.slack.dev/authentication/installing-with-oauth>
- Slack `oauth.v2.access`:
  <https://docs.slack.dev/reference/methods/oauth.v2.access/>
- Slack scopes:
  <https://docs.slack.dev/reference/scopes/>
- Slack Events API:
  <https://docs.slack.dev/apis/events-api/>
- Slack request verification:
  <https://docs.slack.dev/authentication/verifying-requests-from-slack/>
- Slack app manifests:
  <https://docs.slack.dev/reference/app-manifest/>
- Slack Web API rate limits:
  <https://docs.slack.dev/apis/web-api/rate-limits/>
- Slack `conversations.history`:
  <https://docs.slack.dev/reference/methods/conversations.history/>
- Slack `conversations.replies`:
  <https://docs.slack.dev/reference/methods/conversations.replies/>
- Slack message events:
  <https://docs.slack.dev/reference/events/message.channels/>
- Slack token rotation:
  <https://docs.slack.dev/authentication/using-token-rotation/>

## Existing OpenAgents product surface Shape

OpenAgents product surface already has the right primitives for this connector, but they should be
composed carefully.

### First-Party Identity Stays OpenAgents

The ChatGPT/Codex provider-account audit separates three planes:

- OpenAgents human/workspace identity;
- third-party provider account connection state; and
- runtime grants for one bounded action.

Slack should follow the same separation. A Slack install does not replace the
OpenAgents session. It only proves that an actor authorized a Slack workspace
connector and gave OpenAgents product surface a bot token for Slack API calls.

### Slack Is Not The Same As ChatGPT/Codex Provider Accounts

`provider_accounts` currently models ChatGPT/Codex account fleets for agent
runtime execution. It has provider enums, provider-account secret refs, and
auth grants built around "runner can resolve this grant for one run."

Slack has a different authority shape:

- the install is usually workspace-scoped, not user-account-fleet scoped;
- the useful resource is a workspace/channel/message graph;
- the primary ingest path is Slack pushing event callbacks to OpenAgents product surface;
- backfill reads are connector reads, not model-provider execution grants; and
- Slack writes are collaboration side effects that require their own approval
  policy.

Therefore Slack should start with dedicated connector tables such as
`slack_installations`, `slack_channel_links`, and `slack_message_sources`.
Later, if OpenAgents product surface generalizes provider connections into a typed connector-account
registry, Slack can move behind that interface. Do not wedge Slack into the
`chatgpt_codex` provider-account enum.

### GitHub Write Connections Are The Closer Precedent

`github_write_connections` is a better structural precedent:

- one OAuth attempt with an expiry;
- one durable connection ref;
- scopes stored as JSON;
- a secret ref instead of raw token material;
- status and health fields;
- short-lived grants only when a runner needs a GitHub write action.

Slack should reuse those ideas while adding workspace/channel mapping and event
ingestion.

### Team Rooms And Source Authority Are The Product Layer

The team-room docs say shared team chat is durable, scoped by team/project, and
can launch child Autopilot workrooms from explicit `@autopilot` invocations.
The Omni docs say connector reads should become source refs with account,
timestamp, and scope. The knowledge-source bundle contract already separates
source records, extracted spans, generated summaries, rights, redaction, and
projection audiences.

Slack messages should flow into that layer:

```text
Slack workspace/channel/message
  -> Slack connector event
  -> normalized source record
  -> extracted message/thread span
  -> optional retrieval index chunk
  -> bounded workroom context bundle
  -> generated summary/proposal
  -> human-approved business update or Autopilot run
```

Slack message text must not become invisible product truth.

## Slack Platform Facts That Shape The Design

### OAuth v2 Is The Install Path

OpenAgents product surface should use Slack OAuth v2. The browser starts an install by redirecting
to Slack with:

- `client_id`;
- `scope` for bot scopes;
- optional `user_scope` only if user token features are deliberately added;
- `redirect_uri`; and
- one-time `state`.

The callback exchanges `code` through `oauth.v2.access`. OpenAgents product surface stores the
returned workspace/app metadata and a secret ref for bot token material. The
callback must consume `code` and `state`, then redirect to a clean first-party
settings URL to satisfy OpenAgents product surface's clean-public-URL invariant.

### Events API Is The Live Ingest Path

Slack sends URL verification and event callbacks to one request URL. OpenAgents product surface must
verify Slack signatures before processing. The event handler should acknowledge
quickly and then queue work. It should not do slow retrieval, embedding,
workroom launch, or Slack writes inside the request acknowledgment path.

The event callback path should be idempotent by Slack `event_id` plus team/app
scope. Retries must not duplicate source records or launch duplicate workrooms.

### Bot Access Is Not Full Workspace Surveillance

A Slack bot should be treated as a scoped connector:

- public-channel reads depend on app scopes and channel access;
- private-channel reads require private-channel scopes and the app being
  added to each selected private channel;
- direct messages and multi-person DMs require their own scopes and should be
  excluded from MVP unless the product explicitly needs them;
- files need explicit file-read handling;
- Slack Connect channels may include external organizations and should default
  to a more restrictive data policy.

The MVP should optimize for the user workflow: "I am in these private channels,
I will add the bot there, ingest those channels." The settings surface should
let the user/admin choose private channels they can authorize, show whether the
bot is present in each one, and make the active/paused/backfill state visible.

### Historical Backfill Must Be Narrow

Live events are the reliable path. Historical import through
`conversations.history` and thread import through `conversations.replies` should
be explicit, narrow, and backgrounded.

Design consequences:

- channel setup can optionally import "last 24 hours" or "last 100 messages";
- large imports need a queued rate-limit-aware worker;
- history progress should be visible and cancellable;
- failed or partial backfill must not block live events; and
- "full workspace archive" should be deferred to an Enterprise Grid/compliance
  review path, not bundled into the normal bot install.

### Slack Writes Need A Separate Policy

Message ingestion does not imply write authority. Posting a reply, status,
approval prompt, or generated answer to Slack requires `chat:write` and should
be modeled as a Slack side effect with a receipt. Slack button clicks and slash
commands are useful, but they do not bypass OpenAgents product surface approval, acceptance, or
source-authority rules.

## Recommended Slack App Manifest Shape

The first app should be one first-party OpenAgents Slack app, installed into
customer workspaces. Recommended MVP surfaces:

- OAuth redirect URL:
  `https://openagents.com/api/slack/oauth/callback`
- Events request URL:
  `https://openagents.com/api/slack/events`
- Interactivity URL:
  `https://openagents.com/api/slack/interactivity`
- Slash command:
  `/autopilot`
- App mentions:
  `@Autopilot`
- Optional app home for connection status and help.

Recommended MVP bot scopes:

| Scope | Purpose | MVP posture |
| --- | --- | --- |
| `app_mentions:read` | Receive explicit bot mentions. | Required. |
| `chat:write` | Post status, answer-back, and approval/result links. | Required once Slack replies are enabled. |
| `commands` | Receive `/autopilot` slash commands. | Required if slash command ships. |
| `channels:read` | List public channels for channel opt-in. | Required. |
| `channels:history` | Read public channel messages where authorized. | Required for public-channel ingest/backfill. |
| `groups:read` | List private channels visible to the app. | Required for MVP private-channel ingest. |
| `groups:history` | Read private-channel messages where the app has been added. | Required for MVP private-channel ingest. |
| `users:read` | Resolve Slack user display metadata. | Useful for provenance, keep minimal projection. |
| `team:read` | Resolve workspace metadata. | Useful for install records. |
| `files:read` | Ingest files linked in selected channels. | Phase 2, not MVP if text-only. |
| `reactions:read` | Use reactions as lightweight approval/vote signals. | Later feature. |
| `im:history` / `mpim:history` | DM or group-DM ingestion. | Defer; high privacy risk. |

Avoid requesting admin, organization, audit, or discovery scopes in the first
connector. Those scopes change the product from "user-authorized workroom
context" into enterprise compliance ingestion.

## OAuth And Connection Flow

### Browser Start

Route:

```text
POST /api/slack/install/start
```

Authenticated request body:

```json
{
  "openagentsTeamId": "team_openagents_core",
  "openagentsProjectId": "project_artanis",
  "redirectAfter": "/settings/integrations"
}
```

Server behavior:

1. Require OpenAgents browser session.
2. Check the actor can manage connectors for the target team/project.
3. Create a one-time `slack_oauth_attempts` row with an expiry.
4. Store `state` as a high-entropy opaque value, not as encoded private state.
5. Redirect to Slack's OAuth URL.

### OAuth Callback

Route:

```text
GET /api/slack/oauth/callback?code=...&state=...
```

Server behavior:

1. Validate the one-time state attempt.
2. Exchange `code` through Slack `oauth.v2.access`.
3. Validate expected app/client/team shape.
4. Store only installation metadata in D1.
5. Store token material behind `slack-installation://...` or a private KV/vault
   key.
6. Mark the attempt connected or failed with redacted reason text.
7. Redirect to a clean settings URL.

Public URL policy:

```text
/settings/integrations
```

No `?slack=connected`, OAuth code, state, token, or Slack error should remain
in the public app URL.

### Disconnect

Route:

```text
POST /api/slack/installations/:installationRef/disconnect
```

Server behavior:

1. Require connector admin on the OpenAgents product surface team.
2. Mark the installation disconnected.
3. Delete or revoke server-side token material where possible.
4. Stop event/backfill processing for that installation.
5. Preserve safe audit receipts and deletion/retention-sensitive tombstones.

## Event Ingestion Flow

### Event Endpoint

Route:

```text
POST /api/slack/events
```

Handler contract:

1. Read raw request body exactly once.
2. Verify Slack timestamp and `X-Slack-Signature` using the signing secret.
3. Reject stale timestamps to reduce replay risk.
4. Answer URL verification challenges.
5. Dedupe `event_id`.
6. Persist a minimal pending event envelope.
7. Queue normalization in Cloudflare Queues or a Durable Object.
8. Return quickly.

Do not store raw event payloads as normal D1 records. If raw payload retention
is temporarily needed for operator debugging, put it behind a private,
short-TTL object ref and exclude it from public/customer/team exports.

### Normalization Worker

The queued worker should:

1. Load the Slack installation by `team_id`, `enterprise_id`, and app id.
2. Verify the installation is connected and healthy.
3. Check channel allowlist and project/team mapping.
4. Normalize message identity:
   - Slack workspace/team id;
   - channel id;
   - message timestamp;
   - thread timestamp;
   - event id;
   - Slack user/bot refs;
   - subtype such as edit/delete/bot message;
   - permalink if safely available; and
   - content digest.
5. Classify message content as `team`, `private`, or stricter.
6. Create source refs and extracted spans.
7. Update retrieval index enqueue state when eligible.
8. Detect explicit Autopilot triggers through a bounded parser only after the
   Slack event route is already selected.
9. Create a workroom proposal or child run only when the trigger and
   authorization checks pass.

### Source Ref Shape

Slack source refs should be stable, non-secret, and dereferenceable only through
authorized OpenAgents product surface code:

```text
slack://team/<slack-team-id>/channel/<channel-id>/message/<message-ts>
slack://team/<slack-team-id>/channel/<channel-id>/thread/<thread-ts>
slack://team/<slack-team-id>/file/<file-id>
```

Those refs are not public-safe by default. Public projections should show only
generic counts, source type, and redaction state unless a human explicitly
publishes a redacted proof artifact.

### Message Edits And Deletes

Slack message events include subtypes for changed and deleted messages. OpenAgents product surface
should treat them as source lifecycle events:

- edited message: create a new source version, preserve digest lineage, and
  mark old chunks stale;
- deleted message: mark source as deletion/retention-sensitive, hide normal
  projections, and stop retrieval use unless legal retention policy requires
  otherwise;
- bot message: default exclude to avoid feedback loops unless the bot message
  is an explicit OpenAgents receipt.

## Proposed Data Model

The following D1 tables keep Slack separate from provider-account runner
credentials while matching OpenAgents product surface's existing audit style.

### `slack_oauth_attempts`

Purpose: one-time install ceremony state.

Fields:

- `id`
- `user_id`
- `openagents_team_id`
- `openagents_project_id`
- `state`
- `requested_scopes_json`
- `requested_user_scopes_json`
- `status`: `pending | connected | expired | denied | failed`
- `redirect_after`
- `expires_at`
- `completed_at`
- `failed_at`
- `failure_reason`
- `created_at`
- `updated_at`

### `slack_installations`

Purpose: durable workspace app install metadata.

Fields:

- `id`
- `installation_ref`
- `openagents_team_id`
- `openagents_project_id`
- `installed_by_user_id`
- `slack_team_id`
- `slack_enterprise_id`
- `slack_team_name`
- `slack_app_id`
- `slack_bot_user_id`
- `secret_ref`
- `scopes_json`
- `user_scopes_json`
- `status`: `connected | disconnected | unhealthy`
- `health`: `healthy | unhealthy | requires_reauth`
- `token_rotation_enabled`
- `connected_at`
- `disconnected_at`
- `last_event_at`
- `last_status_at`
- `metadata_json`
- `created_at`
- `updated_at`
- `deleted_at`

`secret_ref` should look like:

```text
slack-installation://slack_installation_...
```

D1 must not store bot tokens, user tokens, refresh tokens, signing secrets, raw
OAuth responses, raw event bodies, or Slack file download URLs.

### `slack_channel_links`

Purpose: map Slack channels to OpenAgents product surface teams/projects and ingestion policy.

Fields:

- `id`
- `installation_id`
- `openagents_team_id`
- `openagents_project_id`
- `slack_channel_id`
- `slack_channel_name`
- `channel_kind`: `public | private | slack_connect | dm | mpim`
- `ingest_status`: `paused | active | backfill_pending | failed`
- `ingest_policy_ref`
- `retention_policy_refs_json`
- `redaction_policy_refs_json`
- `created_by_user_id`
- `created_at`
- `updated_at`

Default MVP should allow `private` and `public`, with private-channel ingest as
the priority. A private channel is eligible only when the Slack app has the
private-channel scopes, the bot has been added to that specific channel, and an
OpenAgents product surface connector admin has linked it to the team/project. `dm` and `mpim` should
be blocked until there is a separate privacy decision.

### `slack_message_sources`

Purpose: normalized Slack message source records.

Fields:

- `id`
- `source_ref`
- `installation_id`
- `channel_link_id`
- `openagents_team_id`
- `openagents_project_id`
- `slack_team_id`
- `slack_channel_id`
- `slack_message_ts`
- `slack_thread_ts`
- `slack_event_id`
- `slack_user_ref`
- `message_digest_ref`
- `content_storage_ref`
- `source_version`
- `source_state`: `active | edited | deleted | hidden | blocked`
- `data_classification`
- `trust_tier`
- `retention_policy_refs_json`
- `redaction_policy_refs_json`
- `rights_refs_json`
- `created_at`
- `updated_at`

`content_storage_ref` points to bounded private message content or chunked text
storage. Projection code should never return it directly to public/customer
audiences. Team projection may show snippets only when policy allows it.

### `slack_ingest_events`

Purpose: idempotent processing ledger.

Fields:

- `id`
- `slack_event_id`
- `installation_id`
- `event_type`
- `event_subtype`
- `status`: `queued | processed | ignored | failed`
- `dedupe_key`
- `source_ref`
- `failure_summary`
- `created_at`
- `updated_at`

This is the first place to check before launching a workroom from Slack.

### `slack_backfill_jobs`

Purpose: explicit, narrow history import jobs.

Fields:

- `id`
- `installation_id`
- `channel_link_id`
- `requested_by_user_id`
- `oldest_ts`
- `latest_ts`
- `cursor_ref`
- `status`: `queued | running | rate_limited | completed | failed | canceled`
- `message_limit`
- `messages_processed`
- `rate_limit_reset_at`
- `created_at`
- `updated_at`

Backfill jobs should be disabled by default until live ingest works.

## Public Projection Rules

Slack data starts as at least `team` classification and often `private`.

Projection rules:

- Public: no Slack message text, channel names, user names, timestamps, raw
  source refs, file refs, or Slack workspace identity.
- Customer: no Slack message text unless the customer is a member of the same
  authorized OpenAgents product surface team and an explicit customer-safe export exists.
- Agent: only bounded context refs selected by the retrieval planner; no broad
  channel transcript.
- Team: can see team-safe snippets, channel labels, author display names, and
  source links when policy permits.
- Operator: can see safe diagnostic refs, event state, health, and redacted
  failure summaries.
- Private/internal: can process token-backed connector reads and private
  content under retention policy.

This should consume `OmniDataPolicyEnvelope` rather than defining new audience
semantics.

## Retrieval And Context Policy

Slack ingestion must respect the workspace invariant against ad hoc keyword
routing. The route into Slack is deterministic because the event endpoint is a
Slack connector route. But selecting Slack messages for an Autopilot workroom
should use one of:

- explicit message/thread/channel refs;
- channel opt-in plus recent-window policy;
- a typed retrieval planner;
- embedding search with cosine similarity; or
- a modeled parser for bounded fields such as channel ids, message ids, dates,
  user ids, and exact enum values.

Do not scan arbitrary user prompts for "Slack", "channel", or teammate names
and silently pull Slack messages.

Recommended context bundle:

```json
{
  "kind": "slack_context_bundle",
  "openagentsTeamId": "team_openagents_core",
  "openagentsProjectId": "project_artanis",
  "selectedSourceRefs": [
    "slack://team/T.../channel/C.../thread/1717620000.000000"
  ],
  "excludedSourceRefs": [],
  "selectionReasonRefs": [
    "explicit_slack_mention_thread",
    "channel_allowlist.project_artanis"
  ],
  "redactionPolicyRefs": [
    "redaction.team_safe_slack_context"
  ],
  "retentionPolicyRefs": [
    "retention.slack_team_default"
  ]
}
```

Workrooms should cite source refs and extracted spans. Generated summaries
should remain generated artifacts, not source records.

## Slack-Triggered Autopilot

### Supported Triggers

MVP trigger options:

- `@Autopilot summarize this thread`
- `/autopilot create a workroom from this channel`
- message shortcut: "Send to OpenAgents"
- OpenAgents product surface UI action: "Use Slack thread as context"

Trigger parser rules:

- only run after the incoming request is already verified as Slack;
- require exact mention, slash command, or Slack shortcut payload;
- map Slack channel to an authorized OpenAgents product surface team/project;
- check channel link is active;
- require the Slack actor to map to an OpenAgents product surface member or require a channel-level
  policy that allows non-member requests to create proposals only;
- create a proposal or workroom with source refs, not raw Slack text in title;
  and
- post a Slack acknowledgement only when write scope exists.

### Slack User Mapping

Slack user identity should be mapped separately from OpenAgents identity:

```text
slack_user_ref -> optional openagents_user_id
```

Mapping options:

- self-link from OpenAgents product surface settings;
- email-domain match only as an untrusted candidate;
- team admin confirmation;
- per-command fallback to "external Slack actor created proposal."

Do not grant OpenAgents team permissions merely because a Slack email address
matches a user email. Slack is a connector source, not the identity authority.

### Answer Back To Slack

Slack answer-back should be explicit and receipt-backed:

```text
workroom completed
  -> generate team-safe result summary
  -> classify projection
  -> if Slack reply policy allows
  -> post to channel/thread
  -> store slack_write_receipt
```

If classification blocks the summary, post only a safe link to the OpenAgents product surface
workroom or ask a human to approve a redacted answer.

## Feature Possibilities

### Team Knowledge

- Channel and thread source chips inside OpenAgents product surface workrooms.
- "Ask from this Slack thread" actions.
- Source-backed summaries of selected channels.
- Weekly project digests from opt-in channels.
- Conflict/stale-memory warnings when Slack says one thing and docs/repos say
  another.
- Slack thread-to-source-bundle export for audits and handoffs.

### Project Operations

- Turn Slack decisions into proposed project decisions with source refs.
- Create tasks from explicitly tagged Slack messages.
- Detect blockers only in opted-in channels and stage workroom proposals.
- Attach Slack context to Artanis/project workrooms.
- Notify Slack when a workroom is blocked, waiting for review, or complete.

### Coding Autopilot

- Launch a coding workroom from a Slack thread plus GitHub repo context.
- Post compact run cards back to Slack with status, branch, preview, tests, and
  review link.
- Let a team approve "continue", "request revision", or "open PR" through
  Slack interactive buttons after OpenAgents product surface verifies the actor and policy.
- Use Slack discussion as mission briefing context without including it in
  public proof bundles.

### CRM And Investor Ops

- Extract follow-up candidates from investor/customer channels.
- Stage draft replies or email follow-ups from Slack threads.
- Create relationship-memory candidates with source refs.
- Keep outbound email and CRM mutation behind existing approval gates.
- Produce meeting or channel prep packets for an upcoming call.

### Support

- Ingest a customer Slack Connect channel into a support workroom.
- Build issue timelines from selected threads.
- Draft customer-safe responses with cited Slack source refs.
- Escalate engineering workrooms from support threads.
- Track acceptance, revision, and closeout receipts.

### Approvals And Receipts

- Slack button for "approve draft", "request revision", or "open workroom".
- Per-button action receipts tied to Slack user, OpenAgents product surface user mapping, team, and
  source refs.
- Approval reminders in Slack for pending workroom decisions.
- Policy-blocked replies when Slack actor cannot be mapped to an authorized
  OpenAgents product surface member.

### Public Proof And Data Packages

- Redacted Slack-derived evidence counts in proof bundles.
- Private/team data package export for a project Slack channel.
- No public Slack source refs unless a human creates a redacted public artifact.
- Data rights manifest for Slack-derived packages.

## Security And Compliance Boundaries

### Request Verification

Every Slack request route must verify:

- `X-Slack-Request-Timestamp`;
- `X-Slack-Signature`;
- signing secret from Workers secret binding or secret store;
- raw body HMAC; and
- replay window.

Test fixtures should cover valid signature, bad signature, stale timestamp,
body mutation, and missing headers.

### Secret Storage

Never store these in D1, docs, logs, public projections, issue comments, or
commit messages:

- Slack bot tokens;
- Slack user tokens;
- Slack refresh tokens;
- OAuth code/state after callback processing;
- Slack signing secret;
- raw OAuth responses;
- raw event payloads;
- Slack file private download URLs; or
- full private Slack message archives.

Secret refs are allowed when they use an explicit safe prefix such as:

```text
slack-installation://...
secret://...
vault://...
cloud-secret://...
```

`packages/provider-account-schema` currently scans for provider secret
material but is ChatGPT/GitHub oriented. A Slack implementation should add
Slack token-shaped fixtures to the shared redaction regression suite.

### Data Retention

Slack sources need retention policy from day one:

- default channel retention, for example 30 or 90 days for raw text;
- digest/source refs can survive longer than content if policy allows;
- deleted Slack messages become deletion/retention-sensitive;
- disconnected installations stop new ingest immediately;
- export requires explicit export refs; and
- legal-sensitive workrooms require separate policy.

### Slack Connect

Slack Connect channels can include external organizations. Treat them as a
separate channel kind with a stricter default:

- no automatic public/customer projection;
- no broad retrieval unless explicitly selected;
- more conservative answer-back;
- visible "external participants present" caveat; and
- separate admin opt-in.

### Enterprise Grid And Compliance APIs

Enterprise Grid support is later work:

- org-wide installs may produce enterprise ids and multiple workspaces;
- channel ids need enterprise-aware identity;
- admin/audit/discovery APIs may require enterprise agreements and review;
- retention and legal hold behavior must be designed before full archive
  ingestion.

The MVP should not depend on enterprise-only APIs.

## Cloudflare/OpenAgents product surface Implementation Sketch

### Worker Bindings

Likely bindings:

- `SLACK_CLIENT_ID`
- `SLACK_CLIENT_SECRET`
- `SLACK_SIGNING_SECRET`
- `SLACK_TOKEN_STORAGE` KV or secret-store backed storage
- `SLACK_INGEST_QUEUE`
- D1 database
- optional R2 bucket for bounded private Slack source content

### Routes

New routes:

```text
POST /api/slack/install/start
GET  /api/slack/oauth/callback
GET  /api/slack/installations
POST /api/slack/installations/:installationRef/disconnect
GET  /api/slack/installations/:installationRef/channels
POST /api/slack/installations/:installationRef/channels/:channelId/connect
POST /api/slack/installations/:installationRef/channels/:channelId/pause
POST /api/slack/events
POST /api/slack/interactivity
POST /api/slack/commands/autopilot
```

Keep route implementations in `workers/api`. Browser/Foldkit code should only
call typed API commands and render public projections.

### Services

Suggested Worker modules:

- `slack-installations.ts`
- `slack-oauth-routes.ts`
- `slack-signature.ts`
- `slack-events.ts`
- `slack-source-records.ts`
- `slack-channel-links.ts`
- `slack-backfill.ts`
- `slack-workroom-triggers.ts`
- `slack-write-receipts.ts`

Use Effect services/layers consistent with the repo, and add schemas at every
external boundary.

### UI

Add Slack to the settings workspace page as an integration family:

- "Connect Slack";
- connected workspace name;
- bot user status;
- channel count;
- health/reconnect state;
- "Manage channels";
- "Pause ingest";
- "Disconnect".

Team/project rooms should show Slack as source context, not as a replacement
for native OpenAgents product surface chat:

- source chips for selected Slack threads;
- retrieval trace rows showing selected/excluded Slack source refs;
- workroom cards launched from Slack;
- optional answer-back status.

No user-facing product copy should explain token storage, event ingestion,
signature verification, or internal routing. Keep that in docs/operator views.

## MVP Roadmap

### Phase 0: Slack App Setup

- Register OpenAgents Slack app.
- Add OAuth redirect, Events URL, Interactivity URL, and slash command.
- Store client id/secret/signing secret in Worker secrets.
- Document required scopes.

### Phase 1: Installation Ledger

- Add D1 migrations for OAuth attempts and installations.
- Implement OAuth start/callback with clean redirect.
- Store token material behind secret refs.
- Add settings projection and disconnect.
- Test callback state, scope projection, and secret redaction.

### Phase 2: Channel Opt-In

- List public and private channels visible to the bot token.
- Add `slack_channel_links`.
- Require team/project connector-admin permission.
- Prioritize the installer's private channels and show which ones still need
  the bot added.
- Show active/paused/failed channel state.
- Block DM/MPIM by policy.

### Phase 3: Live Event Ingest

- Implement request verification.
- Handle URL verification.
- Dedupe events.
- Queue normalization.
- Subscribe to and process both `message.channels` and `message.groups`.
- Store Slack source records and extracted spans, with private-channel records
  classified as private/team by default.
- Add edit/delete lifecycle handling.

### Phase 4: Workroom Triggers

- Support `@Autopilot` and `/autopilot`.
- Map Slack channel to OpenAgents product surface team/project.
- Create workroom proposal/run with Slack source refs.
- Post safe acknowledgement to Slack.
- Show run in OpenAgents product surface team/project room.

### Phase 5: Narrow Backfill

- Add explicit backfill jobs for selected public and private channels.
- Prioritize selected private channels the user is already part of and has
  added the bot to.
- Respect Slack rate limits and cursors.
- Show progress and partial failure.
- Add cancellation and replay.

### Phase 6: Retrieval And Digest Features

- Embed/chunk Slack source spans behind the typed retrieval planner.
- Add retrieval trace UI.
- Add project digests and CRM/support/investor workroom templates.
- Add source-backed Slack summaries with approval gates.

### Phase 7: Enterprise Review

- Decide whether to support Enterprise Grid org installs.
- Evaluate admin/compliance/discovery APIs separately.
- Add legal/retention policy before broad archive ingestion.

## Test Plan

Minimum tests before shipping:

- OAuth start requires authenticated team connector admin.
- OAuth callback rejects missing, stale, reused, and mismatched state.
- OAuth callback stores installation metadata without token material.
- Callback redirects to a clean URL with no OAuth result query.
- Public/team/operator installation projections omit secret refs as required.
- Slack signature verifier accepts official-style fixtures.
- Slack signature verifier rejects stale timestamp, bad signature, and mutated
  body.
- URL verification returns the challenge only after signature verification.
- Event idempotency prevents duplicate source records.
- Channel allowlist blocks unconfigured channels.
- Private channels ingest when `groups:history` is granted, the bot is added,
  and the channel is linked to the OpenAgents product surface team/project.
- Private channels do not ingest when the bot is missing or the channel is not
  linked, even if the workspace installation exists.
- Slack Connect/DM channels obey policy.
- Message edit creates a new source version and stales old retrieval chunks.
- Message delete hides or blocks source projections.
- `@Autopilot` exact trigger creates one proposal/run and never duplicates on
  retry.
- Slack actor mapping cannot grant OpenAgents product surface team permissions by email alone.
- Slack writeback requires policy and records a receipt.
- Redaction regression fixtures include Slack token patterns and raw Slack
  webhook/file payloads.
- Backfill respects cursor, rate-limit, cancellation, and partial failure.

## Risks

### Over-Ingesting Private Communication

Slack feels like a team memory source, but it also contains private employee,
customer, legal, and HR material. The connector should default to explicit
channel opt-in, visible status, retention policy, and source refs. The MVP
should prioritize the user's opted-in private channels, but it must not make
workspace-wide private-channel ingest the default.

### Backfill Expectations

Users may expect "connect Slack" to import years of history. Current Slack API
limits and privacy posture make that a poor first promise. Position historical
import as narrow and queued.

### Identity Confusion

Slack users are not automatically OpenAgents product surface users. Slack commands can create
proposals, but authority to mutate OpenAgents product surface state should require OpenAgents
membership mapping and policy checks.

### Public Projection Leakage

Slack messages must never leak into public proof, generated Sites, issue
comments, public claim pages, or exported artifacts without explicit redaction
and approval.

### Feedback Loops

If OpenAgents product surface ingests bot messages and posts summaries back to Slack, it can ingest
its own output. The normalizer should mark OpenAgents bot messages as receipts
or exclude them from retrieval unless explicitly needed.

## Open Questions

- Should Slack be one app for all customers or separate customer-specific apps
  for higher-trust deployments?
- Should MVP require Slack workspace admin install, or allow member install
  with narrower scopes?
- Which OpenAgents product surface role can connect Slack for a team: owner, admin, project manager,
  or a new connector-admin role?
- What is the fastest user flow for adding the bot to all private channels the
  installer is part of without pretending OpenAgents product surface can see private channels before
  the bot is invited?
- How long should raw Slack message content be retained after source refs and
  embeddings exist?
- Should deleted Slack messages remove embeddings immediately, or mark them
  inactive until a retention job sweeps?
- Should Slack messages enter `team_chat_messages`, or remain connector source
  records only? Recommendation: connector records only, with projections into
  rooms when selected.
- Should `files:read` ship in MVP or wait until text-only channel ingest is
  stable?
- Should Slack approval buttons be accepted authority or only a prompt to open
  OpenAgents product surface? Recommendation: buttons create proposals until actor mapping and
  approval receipts are hardened.

## Recommendation

Ship Slack in this order:

1. OAuth install and installation ledger.
2. Private-channel-first channel opt-in and health projection.
3. Verified Events API ingestion into Slack source records for selected private
   and public channels.
4. `@Autopilot` or `/autopilot` trigger that creates a workroom proposal from
   explicit Slack source refs.
5. Safe answer-back with receipts.
6. Narrow per-channel backfill, prioritizing the selected private channels.
7. Retrieval, digests, CRM/support/project ops features.

This matches OpenAgents product surface's existing direction: provider secrets stay behind server
refs, user-facing URLs stay clean, source authority remains explicit, and
workrooms operate on bounded context rather than hidden transcript stuffing.
