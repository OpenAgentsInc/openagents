# Private Workspace Same-Day Setup Runbook

Date: 2026-06-16
Scope: same-day private team/project workspace setup, invite delivery,
acceptance verification, and demo walkthrough.

This runbook is for operator use only. It covers the current production path for
setting up a new private project: create or select the private team/project D1
rows, send a team/project invite, verify the transactional email ledger, confirm
acceptance, and walk the teammate through what they can access.

## Safety Rules

- Do not paste recipient emails, raw invite accept URLs, invite tokens, private
  workspace material, provider request/response bodies, or partner-specific
  source text into transcripts, issue comments, public docs, or public demos.
- Keep raw invite responses in a private shell only. The response contains an
  `acceptUrl`; treat it as bearer-like invite material.
- When invite emails are copied to the operator, use BCC or a separate operator
  ledger/copy email. Do not visibly CC private recipient lists to customers,
  teammates, partners, or other invitees.
- Use `/demo/legal` only for generalized, public-safe legal concepts. Private
  package material belongs only in the gated workspace until a separate
  generalization pass approves broader copy.
- The checker script is read-only and transcript-safe. It prints readiness
  statuses and counts, not private identifiers or email addresses.
- Project-scoped invites still create an active **team** membership. `projectId`
  scopes the invite target/redirect and private workspace gate, but there is no
  separate per-project membership table yet. For confidential partner work, use
  a separate private team per partner/project group.

## What Gets Created

The setup path creates or uses these records:

- `teams`: one private team boundary. This is the main access boundary.
- `team_projects`: one project inside the team, used for the project chat route
  and private workspace/project refs.
- `team_workspace_invites`: one pending invite per recipient/team/project/email.
  Refreshed invites rotate the token and expiry.
- `email_messages` / `email_deliveries`: transactional invite email ledger rows
  when `sendEmail` is true and Resend config is present.
- `team_memberships`: created or reactivated as `active` when the invitee accepts
  with the invited email address.

The invitee gets:

- a transactional email with subject `Your private OpenAgents workspace invite`;
- an `Accept invite` button/link;
- the workspace label you provide;
- the invite expiry;
- instruction to sign in with the invited email address;
- after acceptance, a redirect to the team/project chat if `projectId` was
  provided, otherwise the team chat.

The invitee does **not** get raw private source material from the invite alone.
Private package material appears only after it is seeded into gated team/project
surfaces or private workspace rows.

## API-First Project + Invite Path

Use this path first. It replaces the manual SQL setup for normal same-day
operator work:

- creates or reuses the private team/project from the project name and slugs;
- creates or reuses a `private_team` prefilled workspace unless disabled;
- sends multiple role-tagged invites in one request;
- stores private participant kind metadata on each invite;
- defaults invite email sending to on;
- defaults operator email copies to on through separate ledger-copy emails;
- does not return raw accept URLs unless explicitly requested.

```http
POST /api/operator/private-project-workspaces
```

Example request shape, with placeholder addresses only:

```json
{
  "project": {
    "name": "Private project workspace",
    "slug": "optional-stable-slug",
    "teamName": "Optional team label",
    "teamSlug": "optional-team-slug"
  },
  "workspace": {
    "createPrefilledWorkspace": true,
    "workspaceLabel": "Private project workspace"
  },
  "email": {
    "copyOperator": true,
    "copyMode": "bcc_or_ledger_copy"
  },
  "invitations": [
    {
      "email": "<team-member-email>",
      "role": "admin",
      "participantKind": "internal_team_member",
      "sendEmail": true,
      "copyOperator": true
    },
    {
      "email": "<external-partner-email>",
      "role": "member",
      "participantKind": "external_partner"
    }
  ]
}
```

API defaults:

- `sendEmail: true` for each invite unless explicitly disabled;
- `copyOperator: true` for API-created invite emails unless explicitly disabled
  at the request or per-invite level;
- operator copy is sent to `email.operatorCopyEmail` when present, otherwise the
  configured Resend reply-to address; the current implementation uses a
  separate ledger-backed copy email rather than visible CC, and the operator
  copy redacts the invite token / accept URL so it is audit-only;
- no raw accept URLs or invite tokens in normal responses unless the operator
  passes `"includeAcceptUrls": true`;
- idempotent project/team selection and invite refresh so repeating the same
  request does not create duplicate teams or uncontrolled duplicate emails;
- safe response fields only: team/project/workspace refs, invite refs, invite
  status, email status, and redacted delivery refs.

If the API is unavailable or a manual backfill is needed, use the fallback path
below.

## Inputs

Prepare these in a private shell:

```sh
export OPENAGENTS_BASE_URL="https://openagents.com"
export OPENAGENTS_ADMIN_API_TOKEN="..."
export PRIVATE_TEAM_ID="..."
export PRIVATE_TEAM_SLUG="..."
export PRIVATE_TEAM_NAME="..."
export PRIVATE_PROJECT_ID="..."
export PRIVATE_PROJECT_SLUG="..."
export PRIVATE_PROJECT_NAME="..."
```

Keep the teammate recipient email out of shell history when possible. Use a
temporary private file or stdin for the invite request body, and delete it after
the invite is created.

Use stable ids and slugs containing only lowercase letters, numbers, `_`, and
`-`. Keep display names safe for the app; they may be visible to invited
members. For the copy-paste SQL path below, avoid single quotes in display names
or escape them manually before running the file.

## 1. Create Or Select The Private Team/Project

If the team/project already exists, skip to preflight. If not, create both rows
with a reviewed SQL file. This is now a fallback-only operator step; the API
path above should be used for normal project creation and invite fanout.

From `apps/openagents.com/workers/api`:

```sh
NOW="$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")"

cat > /tmp/private-project-setup.sql <<SQL
INSERT INTO teams
  (id, name, slug, kind, plan, owner_user_id, status, created_at, updated_at,
   archived_at)
VALUES (
  '${PRIVATE_TEAM_ID}',
  '${PRIVATE_TEAM_NAME}',
  '${PRIVATE_TEAM_SLUG}',
  'organization',
  'team',
  NULL,
  'active',
  '${NOW}',
  '${NOW}',
  NULL
)
ON CONFLICT(id) DO UPDATE SET
  name = excluded.name,
  slug = excluded.slug,
  kind = excluded.kind,
  plan = excluded.plan,
  status = 'active',
  archived_at = NULL,
  updated_at = excluded.updated_at;

INSERT INTO team_projects
  (id, team_id, slug, name, description, status, metadata_json, created_at,
   updated_at, archived_at)
VALUES (
  '${PRIVATE_PROJECT_ID}',
  '${PRIVATE_TEAM_ID}',
  '${PRIVATE_PROJECT_SLUG}',
  '${PRIVATE_PROJECT_NAME}',
  'Private project workspace.',
  'active',
  '{"source":"operator_private_project_runbook"}',
  '${NOW}',
  '${NOW}',
  NULL
)
ON CONFLICT(id) DO UPDATE SET
  team_id = excluded.team_id,
  slug = excluded.slug,
  name = excluded.name,
  description = excluded.description,
  status = 'active',
  metadata_json = excluded.metadata_json,
  archived_at = NULL,
  updated_at = excluded.updated_at;
SQL

bunx wrangler d1 execute openagents-autopilot \
  --remote \
  --file /tmp/private-project-setup.sql
```

Before running that SQL:

- review `/tmp/private-project-setup.sql` locally;
- do not paste it into public transcripts if the display names are private;
- reuse the existing ids instead of creating new ids when the team/project
  already exists;
- do not set `owner_user_id` unless you are intentionally binding an existing
  user id as owner.

After running it, delete the temporary SQL file if it contains private display
names:

```sh
rm -f /tmp/private-project-setup.sql
```

## 2. Preflight

From `apps/openagents.com`:

```sh
node scripts/private-workspace-setup-check.mjs \
  --team-id "$PRIVATE_TEAM_ID" \
  --project-id "$PRIVATE_PROJECT_ID" \
  --session-ready unknown \
  --live-config \
  --live-d1 \
  --remote
```

Required readiness:

- `operator_admin_token: ready`
- `email_config: ready`
- `team_exists: ready`
- `project_exists: ready`
- `browser_session` can be `manual` before the teammate signs in; confirm it
  before the call.

If the project is not required for the call, omit `--project-id`; the invite will
be team-scoped.

## 3. Create Or Refresh The Invite

Send one operator invite for the private team/project:

```sh
curl -fsS -X POST "$OPENAGENTS_BASE_URL/api/operator/team-workspace-invites" \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "content-type: application/json" \
  --data @/tmp/private-workspace-invite.json
```

Request body shape:

```json
{
  "teamId": "<private team id>",
  "projectId": "<private project id or null>",
  "email": "<recipient email>",
  "role": "member",
  "workspaceLabel": "Private project workspace",
  "sendEmail": true
}
```

Roles:

- `viewer`: can join with read-oriented access where the UI/API distinguishes
  role. Use for observers.
- `member`: default for teammates expected to collaborate.
- `admin`: use only when the invitee should help manage the team/project.

Privately capture these response fields:

- `invite.id`
- `invite.status`
- `email.status`
- `email.emailMessageId`
- `acceptUrl`

Do not paste `acceptUrl` into chat, issues, or transcripts.

What the route does:

- validates the active team exists;
- validates `projectId` belongs to that active team when provided;
- creates or refreshes the pending invite for that recipient/team/project target;
- sends the invite email unless `sendEmail` is false;
- records the email attempt on the invite when the email service returns a
  message id;
- returns a safe invite projection plus the raw `acceptUrl`.

## 4. Verify Invite And Email Ledger

Rerun the checker with the safe ids from the response:

```sh
node scripts/private-workspace-setup-check.mjs \
  --team-id "$PRIVATE_TEAM_ID" \
  --project-id "$PRIVATE_PROJECT_ID" \
  --invite-id "$INVITE_ID" \
  --email-message-id "$EMAIL_MESSAGE_ID" \
  --session-ready unknown \
  --live-config \
  --live-d1 \
  --remote
```

Expected before acceptance:

- `invite_status: ready` with invite status `pending`
- `email_ledger: ready` with accepted delivery, or a clear blocker/fallback
- no raw email address, accept URL, token, or provider body printed

## 5. Teammate Accepts

The teammate must sign in with the invited email address and open the invite
from the transactional email. If using the raw `acceptUrl` fallback, paste it
only into the teammate's browser or a private channel.

Acceptance behavior:

- `GET /api/team-workspace-invites/accept?token=...` redirects after success.
- A logged-out `GET` click redirects to `/login/email` with the invite accept
  path as the return target, then retries the same invite after sign-in.
- A signed-in browser using the wrong account is sent through `/auth/logout`
  first, then returned to the same invite link and email-login flow.
- `POST /api/team-workspace-invites/accept` returns JSON for scripted checks.
- A signed-in user with a different email still gets `403` on the scripted
  `POST` accept API; browser `GET` links use the recovery redirect above.
- An expired invite gets `410`.
- An already accepted invite is idempotent for the same accepted user.
- A successful accept creates/reactivates `team_memberships` for the team with
  the invited role.

Successful acceptance redirects to:

```text
/teams/<team id>/projects/<project id>/chat
```

For team-scoped invites, it redirects to:

```text
/teams/<team id>/chat
```

## 6. Verify Acceptance

After acceptance:

```sh
node scripts/private-workspace-setup-check.mjs \
  --team-id "$PRIVATE_TEAM_ID" \
  --project-id "$PRIVATE_PROJECT_ID" \
  --invite-id "$INVITE_ID" \
  --email-message-id "$EMAIL_MESSAGE_ID" \
  --session-ready yes \
  --live-config \
  --live-d1 \
  --remote
```

Expected:

- `browser_session: ready`
- `invite_status: ready` with invite status `accepted`
- `team_exists: ready`
- `project_exists: ready`
- `email_ledger: ready` or documented manual fallback

At this point the teammate should be able to:

- open the team chat route;
- open the project chat route when `projectId` was included;
- view private prefilled workspace projections only when those rows are
  configured as `private_team` and scoped to the accepted team/project;
- participate according to their role and whatever UI/API-specific role checks
  exist for that surface.

They should not be able to:

- see raw invite tokens or invite hashes;
- accept the invite while signed in as a different email;
- view private material in public demos or public-safe projections;
- access other partner teams unless they were separately invited there.

## Email Failure Fallback

If Resend config or delivery fails:

1. Create or refresh the invite with `"sendEmail": false`.
2. Keep the returned `acceptUrl` private.
3. During the call, have the teammate sign in with the invited email address.
4. Paste the `acceptUrl` directly into the teammate browser, or send it through a
   private channel outside transcripts.
5. Rerun the checker with `--invite-id`, `--session-ready yes`, and `--live-d1`
   to confirm acceptance.

If acceptance returns `403`, the teammate is signed in as the wrong email. Sign
out, sign in with the invited email, and retry. If acceptance returns `410`,
create a fresh invite.

## Demo Call Walkthrough

Use this sequence:

1. Show the generalized public demo only for orientation.
2. Switch to the private team/project workspace after acceptance.
3. Confirm the teammate is an active member and can see the project workspace.
4. Walk the private material only inside the gated workspace.
5. Keep payment and credit collection as a separate Stripe/checkout step unless
   a verified payment receipt already exists.

Private workspace seeding of partner-specific source material is the next H4
step. Until H4 is complete, do not seed private package content through public
fixtures, public docs, or `/demo/legal`.
