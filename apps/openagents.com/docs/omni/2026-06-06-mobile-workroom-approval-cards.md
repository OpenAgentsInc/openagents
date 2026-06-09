# Mobile Workroom Projection And Approval Cards

Status: implemented for issue #372 / `OPENAGENTS-LATE-012`.

## Purpose

Mobile and compact agent surfaces need a small workroom view that shows what is
happening, what needs approval, what evidence supports the request, and what is
still blocked. This contract defines read-only workroom projections and
approval cards for CRM sends, coding writes, runner launches, payments,
provider actions, public claims, and legal-sensitive actions.

Implementation:

- `workers/api/src/omni-mobile-workroom-approval-cards.ts`
- `workers/api/src/omni-mobile-workroom-approval-cards.test.ts`

## Compact Workroom

The compact workroom record carries:

- status and status ref;
- work kind;
- title ref;
- active outcome refs;
- artifact refs;
- receipt refs;
- blocker refs;
- site refs;
- source refs;
- provider state refs; and
- wallet state refs.

Projection labels use friendly time strings such as `5 minutes ago`, not raw
timestamps. Summary counts expose pending, blocked, expired, and critical
approval cards plus artifact, receipt, and evidence counts.

## Approval Cards

Approval cards support these action kinds:

- `crm_send`;
- `coding_write`;
- `runner_launch`;
- `payment`;
- `provider_action`;
- `public_claim`; and
- `legal_sensitive`.

Each card records action kind, risk level, approval requirement, state,
evidence refs, artifact refs, receipt refs, source refs, idempotency ref,
expiry, server-authority caveats, and optional approval/execution receipts.

High and critical cards require evidence refs and an explicit approval
requirement. Runner launches, payments, provider actions, public claims, and
legal-sensitive cards always require approval. Approved or executed cards
require approval receipts, executed cards require execution receipts, expired
cards require an expiry timestamp, and blocked cards require blocked reason
refs.

## Expiry Labels

Card expiry is projected as:

- `not_expiring` when no expiry exists;
- `active` with a label such as `Expires in 1 hour`; or
- `expired` with a label such as `Expired 10 minutes ago`.

Raw ISO timestamps remain in records only and are not exposed in projections.

## Authority Boundaries

The projection is read-only. It cannot:

- approve cards;
- execute actions;
- send notifications;
- spend wallets;
- mutate provider accounts;
- launch runners; or
- upgrade public claims.

Those actions require separate server-authoritative routes, scoped grants,
idempotency, receipts, and approval policy.

## Audience Redaction

Supported projection audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public and agent projections redact private approval, idempotency, provider,
wallet, receipt, title, artifact, source, status, blocker, site, and
server-authority refs as appropriate. The contract rejects raw email, provider,
wallet, payment, private repo, runner log, secret, and raw timestamp material
before projection.

## Tests

Coverage includes:

- compact mobile projection validation;
- pending, blocked, expired, and critical approval counts;
- risk/evidence and mandatory approval requirements;
- expiry display labels;
- approval, execution, blocked, and expired state requirements;
- public and agent redaction; and
- hard false approval, execution, notification, payment, provider, public
  claim, and runner-launch mutation authority.
