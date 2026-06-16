# Workspace Seeding, Invite Links, And Engagement

Date: 2026-06-16
Scope: #5093, Epic C / prefilled workspaces.

## What Shipped

The prefilled workspace primitive now covers the operator seed-to-invite loop:

- operators create public-safe project workspaces with `POST /api/workspaces`;
- operator responses include a personal invite URL at `/workspaces/{workspaceId}`;
- the first signed-in holder who opens an unbound `invited` workspace claims it;
- holder reads record first view and revisit count;
- `POST /api/workspaces/{workspaceId}/engagement` with
  `{ "event": "first_run" }` records the first starter-run handoff;
- operator reads expose engagement for inspection, while holder projections keep
  operator-only holder bindings and invite metadata out.

## Public-Safe Boundary

Seeded memory, starter workflows, and intro receipts remain public-source-only.
Workspace engagement records are workflow telemetry: invited time, first view,
first claim, first run, last view, and revisit count. They grant no workspace
authority, spend, payout, settlement, provider mutation, delivery, or public
claim authority.

The invite URL is not an auth grant. It points to the branded logged-out
workspace shell; session issuance still happens through `/login`, and holder
access is enforced by the Worker route before returning the seeded workspace
projection.

## Operator Flow

1. Research the prospect from public sources only.
2. Choose a vertical template or provide equivalent public-source seed material.
3. Create the workspace as `status: "invited"`.
4. Send the returned `inviteUrl`.
5. Inspect engagement through operator `GET /api/workspaces/{workspaceId}`.

Live customer data, private files, connected accounts, publication, spend, and
delivery remain blocked until the holder connects accounts and approves the
specific next action.
