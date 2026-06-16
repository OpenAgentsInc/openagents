# Private Project Workspace API And Invite Fanout Audit

Date: 2026-06-16
Status: docs audit for `https://github.com/OpenAgentsInc/openagents/issues/5156`.
Scope: operator API for creating private project workspaces, sending multiple
transactional invite emails, and optionally copying the operator on those emails.

## Requirement

An operator should be able to ask an agent to create a named private project
workspace and invite known recipients through API calls only. The operator must
be able to distinguish an internal teammate from an external partner or future
client in the request, send all invitations programmatically, and choose whether
the operator receives copies of the invite emails. For API-driven setup, operator
copies should default to enabled.

## Current State

- `team_workspace_invites` exists and supports pending invite creation, refresh,
  expiry, acceptance, and membership promotion.
- `POST /api/operator/team-workspace-invites` exists for creating or refreshing a
  single invite to an existing active team and optional project.
- The invite email uses the existing Resend-backed `EmailService` ledger when
  `sendEmail` is true and email config is present.
- `POST /api/workspaces` can create a `private_team` prefilled workspace row, but
  it expects `privateTeamId` and `privateProjectId` to already exist.
- The private workspace runbook currently requires operator SQL to create/select
  the `teams` and `team_projects` rows before invites can be sent.

## Gaps

- There is no single API that creates or selects the private team/project from a
  project name.
- There is no batch invite fanout API; operators must call the invite endpoint
  once per recipient.
- Invite requests model membership `role`, but not the recipient's business
  relationship to the workspace, such as `internal_team_member`,
  `external_partner`, or future `client`.
- The email rendering/sending model does not yet expose `cc`, `bcc`, or a
  separate operator-copy mode.
- The low-level invite route returns `acceptUrl`, which is useful for manual
  fallback but should not be part of normal API-first responses.

## Recommended Endpoint

Add an operator-authenticated orchestration endpoint:

```http
POST /api/operator/private-project-workspaces
```

Request shape:

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

Defaults:

- `sendEmail` defaults to `true`.
- `copyOperator` defaults to `true` for API-created invite emails.
- Per-invite `copyOperator` overrides the request-level default.
- `participantKind` is stored as operator/private metadata and must not appear
  in public or holder projections unless a future UI deliberately exposes a safe
  role label.

Response shape should include stable refs and statuses only:

```json
{
  "team": { "id": "<team-ref>", "status": "active" },
  "project": { "id": "<project-ref>", "status": "active" },
  "workspace": { "id": "<workspace-ref>", "accessMode": "private_team" },
  "invitations": [
    {
      "inviteId": "<invite-ref>",
      "role": "admin",
      "participantKind": "internal_team_member",
      "status": "pending",
      "email": { "status": "accepted", "copyStatus": "accepted" }
    }
  ]
}
```

Normal responses should not include raw accept tokens or accept URLs. If a manual
fallback needs the accept URL, require an explicit private operator flag and keep
the value out of transcripts, issue comments, public docs, and logs.

## Operator Copy Behavior

Use one of two safe modes:

- **BCC mode:** send one invite email to the recipient with the operator as BCC.
  This keeps the operator copied without exposing the operator copy recipient to
  the invitee or exposing one invitee to another.
- **Ledger copy mode:** send the recipient's invite normally, then send a
  separate operator copy email containing the safe invite summary and delivery
  refs. This is cleaner for auditing because the operator copy can have its own
  `email_messages` / `email_deliveries` rows.

Do not visibly CC customers, teammates, partners, or multiple invitees on a
private workspace invite.

## Implementation Slices

1. Add schema validation for the new endpoint, including bounded slugs, project
   labels, roles, participant kinds, copy defaults, and per-invite overrides.
2. Add idempotent team/project creation or selection, using stable generated refs
   when slugs are omitted.
3. Optionally create the matching `private_team` prefilled workspace row.
4. Reuse the existing invite creation/refresh semantics for each recipient, but
   execute them under one operator transaction where D1 constraints allow it.
5. Extend `EmailService` / `RenderedEmail` with BCC or add a separate operator
   copy send method with explicit ledger rows.
6. Update the transcript-safe checker/runbook so it can verify the project,
   invite fanout, invite email delivery, copy delivery, and acceptance without
   printing addresses or tokens.

## Tests Needed

- Project/team/workspace creation from a single operator API call.
- Existing team/project reuse without duplicate rows.
- Batch invites with mixed roles and participant kinds.
- Default `sendEmail: true` and default `copyOperator: true`.
- Request-level and per-invite copy opt-out.
- Invitee email does not expose other recipients or operator-copy recipients.
- Resend/config failure keeps invite refs and returns a clear email blocker.
- Idempotent retry refreshes or reuses invites without uncontrolled duplicate
  emails.
- Public and holder projections omit raw emails, accept tokens, accept URLs,
  participant metadata, and operator-only delivery refs.

## Roadmap Link

This is the API-first replacement for the current runbook's manual SQL setup
path. It is the next workspace machinery item before scaling private legal,
agency, e-commerce, investor, or other project workspaces beyond hand-built
operator setup.
