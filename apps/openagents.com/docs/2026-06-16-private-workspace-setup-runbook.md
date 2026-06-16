# Private Workspace Same-Day Setup Runbook

Date: 2026-06-16
Scope: issue #5155, same-day private team/project workspace setup and invite
verification for a design-partner demo call.

This runbook is for operator use only. It assumes the private team/project
exists and that the invite flow, invite email, and private prefilled workspace
hardening are already on `main`.

## Safety Rules

- Do not paste recipient emails, raw invite accept URLs, invite tokens, private
  workspace material, provider request/response bodies, or partner-specific
  source text into transcripts, issue comments, public docs, or public demos.
- Keep raw invite responses in a private shell only. The response contains an
  `acceptUrl`; treat it as bearer-like invite material.
- Use `/demo/legal` only for generalized, public-safe legal concepts. Private
  package material belongs only in the gated workspace until a separate
  generalization pass approves broader copy.
- The checker script is read-only and transcript-safe. It prints readiness
  statuses and counts, not private identifiers or email addresses.

## Inputs

Prepare these in a private shell:

```sh
export OPENAGENTS_BASE_URL="https://openagents.com"
export OPENAGENTS_ADMIN_API_TOKEN="..."
export PRIVATE_TEAM_ID="..."
export PRIVATE_PROJECT_ID="..."
```

Keep the teammate recipient email out of shell history when possible. Use a
temporary private file or stdin for the invite request body, and delete it after
the invite is created.

## 1. Preflight

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

## 2. Create Or Refresh The Invite

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

Privately capture these response fields:

- `invite.id`
- `invite.status`
- `email.status`
- `email.emailMessageId`
- `acceptUrl`

Do not paste `acceptUrl` into chat, issues, or transcripts.

## 3. Verify Invite And Email Ledger

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

## 4. Teammate Accepts

The teammate must sign in with the invited email address and open the invite
from the transactional email. If using the raw `acceptUrl` fallback, paste it
only into the teammate's browser or a private channel.

Successful acceptance redirects to:

```text
/teams/<team id>/projects/<project id>/chat
```

For team-scoped invites, it redirects to:

```text
/teams/<team id>/chat
```

## 5. Verify Acceptance

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
