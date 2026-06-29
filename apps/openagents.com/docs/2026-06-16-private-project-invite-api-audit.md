# Private Project Workspace API And Invite Fanout Audit

Date: 2026-06-16
Status: implemented for `https://github.com/OpenAgentsInc/openagents/issues/5156`.
Scope: operator API for creating private project workspaces, sending multiple
transactional invite emails, and optionally copying the operator on those emails.

## Requirement

An operator should be able to ask an agent to create a named private project
workspace and invite known recipients through API calls only. The operator must
be able to distinguish an internal teammate from an external partner or future
client in the request, send all invitations programmatically, and choose whether
the operator receives copies of the invite emails. For API-driven setup, operator
copies should default to enabled.

## Implemented State

- `team_workspace_invites` exists and supports pending invite creation, refresh,
  expiry, acceptance, and membership promotion.
- `POST /api/operator/team-workspace-invites` exists for creating or refreshing a
  single invite to an existing active team and optional project.
- The invite email uses the existing Resend-backed `EmailService` ledger when
  `sendEmail` is true and email config is present.
- `POST /api/workspaces` can still create a lower-level `private_team` prefilled
  workspace row when the operator already has private team/project refs.
- `POST /api/operator/private-project-workspaces` now orchestrates the normal
  API-first setup: private team/project create-or-select, optional private
  prefilled workspace create-or-reuse, batch invite fanout, and operator email
  copies by default.

## Original Gaps

- The old setup path had no single API that created or selected the private
  team/project from a project name.
- The old setup path had no batch invite fanout API; operators had to call the
  invite endpoint once per recipient.
- The old invite request modeled membership `role`, but not the recipient's
  private business relationship to the workspace, such as
  `internal_team_member`, `external_partner`, or future `client`.
- The old route did not expose operator-copy controls.
- The low-level invite route returns `acceptUrl`, which remains useful for
  manual fallback but is not part of normal API-first responses.

## Implemented Endpoint

The operator-authenticated orchestration endpoint is:

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

Implemented defaults:

- `sendEmail` defaults to `true`.
- `copyOperator` defaults to `true` for API-created invite emails.
- Per-invite `copyOperator` overrides the request-level default.
- Operator copy is sent as a separate ledger-backed email to
  `email.operatorCopyEmail` when supplied, otherwise the configured Resend
  reply-to address. Operator copies redact the live accept URL / invite token so
  the copy is audit-only and cannot be used to accept the invite.
- `participantKind` is stored as operator/private metadata and must not appear
  in public or holder projections unless a future UI deliberately exposes a safe
  role label.

Response shape includes stable refs and statuses only:

```json
{
  "team": { "id": "<team-ref>", "slug": "<team-slug>", "status": "active" },
  "project": {
    "id": "<project-ref>",
    "slug": "<project-slug>",
    "status": "active",
    "teamId": "<team-ref>"
  },
  "workspace": {
    "id": "<workspace-ref>",
    "accessMode": "private_team",
    "status": "invited"
  },
  "invitations": [
    {
      "index": 0,
      "participantKind": "internal_team_member",
      "invite": {
        "id": "<invite-ref>",
        "role": "admin",
        "status": "pending"
      },
      "email": { "status": "accepted" },
      "copy": { "status": "accepted" }
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
  separate operator copy of the invite email. This is cleaner for auditing
  because the operator copy has its own `email_messages` / `email_deliveries`
  rows. The operator copy must redact the invite token and accept URL.

Do not visibly CC customers, teammates, partners, or multiple invitees on a
private workspace invite.

## Implemented Slices

1. Added schema validation for the new endpoint, including bounded slugs, project
   labels, roles, participant kinds, copy defaults, and per-invite overrides.
2. Added idempotent team/project creation or selection using slugs and generated
   refs.
3. Reuses an existing private workspace by private team/project target, or
   creates the matching `private_team` prefilled workspace row when absent.
4. Reuses the existing invite creation/refresh semantics for each recipient.
5. Sends operator copies as separate ledger-backed emails; no visible CC is
   used.
6. Updated the runbook so the API-first path is primary and the SQL path is
   fallback-only.

## Regression Coverage

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
