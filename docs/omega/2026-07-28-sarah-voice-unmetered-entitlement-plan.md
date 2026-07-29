# Sarah voice unmetered entitlement plan

Date: 2026-07-28

Status: One bounded staging owner exception enabled. Future Nostr allowlist proposed.

Issue: [#9272](https://github.com/OpenAgentsInc/openagents/issues/9272)

## Purpose

This document defines a possible next phase for Sarah voice.
It does not grant an entitlement.
It does not change the current credit rules.
Normal credits remain the default.

## Current staging owner exception

The service has one smaller exception for the current staging test.
This exception is not the Nostr public key allowlist in this plan.
It does not enable public or production unmetered voice.

The exception has these server checks:

1. `SARAH_STAGING_OWNER_VOICE_ENTITLEMENT_ENABLED` is on.
2. `OPENAGENTS_APP_URL` identifies the staging Cloud Run service.
3. The request has a valid OpenAgents session.
4. The session user is the canonical primary owner user.
5. The user is active.
6. The database entitlement is active and not expired.

The service derives the owner user from the configured primary admin email.
The client does not send an owner allowlist value.
The client does not send a GitHub header or an entitlement flag.
A linked Nostr device and a GitHub or email session resolve to the same
canonical user before this check.

The first eligible request creates this exact server record:

- entitlement reference: `sarah_voice_entitlement:staging_owner_v1`
- environment: `staging`
- duration: seven days from the first eligible request
- activation actor: `operator:staging_owner_voice_entitlement`
- activation source: `cloudrun:staging_owner_account_match`

The unique user constraint prevents a second record.
An expired or revoked record does not become active again automatically.
The audit table records the activation.
The route log contains only the entitlement reference and a user digest.

An eligible voice session has a zero credit hold.
The service records all provider token usage and the calculated credit cost.
It does not write a payment row and does not debit the user balance.
The response has `reservedCreditMsat: 0`.

All other voice controls continue to apply.
The maximum staging session duration is 300 seconds.
The one-session account limit, device binding, ticket expiry, rate limits,
provider safety controls, tool allowlist, and confirmation controls remain.
An account that is not the canonical owner uses the normal credit path.
The service returns `402 {"error":"insufficient_credit"}` when that account
does not have enough available credit.

### Staging kill and revocation

Set `SARAH_STAGING_OWNER_VOICE_ENTITLEMENT_ENABLED` to `false` and deploy the
staging service to stop new exception sessions.
Do not add this variable to the production environment.

An authorized database operator can revoke the record with this transaction.
The command uses only the fixed entitlement reference.
It does not need a user identifier.

```sql
BEGIN;

WITH revoked AS (
  UPDATE sarah_voice_credit_entitlements
  SET state = 'revoked',
      revoked_at = CURRENT_TIMESTAMP::text,
      revocation_actor_ref = '<authorized-operator-ref>',
      revocation_reason = '<bounded-reason>',
      updated_at = CURRENT_TIMESTAMP::text
  WHERE entitlement_ref = 'sarah_voice_entitlement:staging_owner_v1'
    AND environment = 'staging'
    AND state = 'active'
  RETURNING entitlement_ref
)
INSERT INTO sarah_voice_credit_entitlement_audit (
  event_ref, entitlement_ref, action, actor_ref, reason, source, occurred_at
) SELECT
  'sarah_voice_entitlement:staging_owner_v1:revoked',
  entitlement_ref,
  'revoked',
  '<authorized-operator-ref>',
  '<bounded-reason>',
  'operator:cloud_sql',
  CURRENT_TIMESTAMP::text
FROM revoked
ON CONFLICT (event_ref) DO NOTHING;

COMMIT;
```

Revocation blocks a new session immediately.
An active session can continue only until its normal 300-second limit.
The future allowlist design below has a stricter active-session kill control.

The next phase can let an operator approve a Nostr public key for unmetered
voice.
The word `unmetered` has one narrow meaning.
It bypasses the credit hold and debit for an approved voice session.
It does not bypass any other control.

## Required server record

The service must keep the allowlist in Cloud SQL.
Do not keep the allowlist on a client.
Source code must not contain a personal public key.

Each record must contain these fields:

- an entitlement reference
- a 64-character lowercase hexadecimal public key
- the canonical OpenAgents user reference
- an optional approved device reference
- an optional approved device generation
- an activation time
- an optional expiry time
- a state of `pending`, `active`, `expired`, or `revoked`
- an audit reason
- an actor reference for the operator
- a source or approval reference
- a creation time and an update time
- a revocation time and a revocation reason when applicable

The database must reject a key that is not canonical.
The database must prevent two active records for the same key and product.
The service must verify the active user and the live Nostr identity link.
It must not link accounts by email, display name, NIP-05 name, or relay
membership.

An account-link operation needs separate authority.
It must use a recent authenticated account session and a fresh one-use NIP-98
proof.
It must reject a public key that belongs to a different account.

## Admission rules

The service must check the entitlement after authentication.
It must check these items for each session:

1. The NIP-98 proof is valid and fresh.
2. The challenge is valid and unused.
3. The canonical public key has one active entitlement.
4. The entitlement user equals the authenticated user.
5. The device and generation match when the record limits a device.
6. The activation time has passed.
7. The expiry time has not passed.
8. The entitlement and the user are not revoked.
9. The default-off `unmetered voice enabled` switch is on.

An unmetered session still has all normal limits.
Authentication is always required.
The service keeps the account concurrency limit.
It keeps challenge and session rate limits.
It keeps abuse detection and provider safety checks.
It keeps the ticket expiry and the maximum session lifetime.

It keeps the editor tool allowlist and the confirmation handshake.
It also keeps exact provider usage records for cost and abuse review.

The entitlement grants no account, admin, payment, wallet, deployment, release,
or external-action authority.
It does not grant a different model or provider.

## Kill switch and revocation

The service needs an `unmetered voice enabled` environment switch.
Its default value must be off, and it must refuse admission when it is off.
The service also needs a shared kill state in Cloud SQL.
Each active bridge must check this state at intervals of 30 seconds or less.
An operator must be able to disable this path without a code release.

The operator action must disable the shared state and drain all current
unmetered sessions.
Each active bridge must close no later than 30 seconds after the operator
disables the shared state.

Revocation must take effect for a new session immediately.
The service must also close an active session when its entitlement becomes
revoked or expired.
The close process must record the final provider usage.
It must release any old credit hold if the session changed from a credit mode.

An operator action must write an append-only audit event.
The event must contain bounded references and typed reasons.
It must not contain the NIP-98 token, signature, challenge, bearer token,
ticket, audio, transcript, provider frame, editor text, or tool result.

## Budget and abuse limits

Unmetered does not mean unlimited provider spend.
The next phase needs account, device, session, daily provider-token, and daily
cost limits.
It also needs one service-wide daily cost limit.
The service must fail closed when a limit is absent, invalid, or reached.

The operator must approve the budget and threat model before activation.
The implementation needs a migration, store tests, route tests, settlement
tests, and an operator rollback test.
This proposal is not implementation or release authority.
