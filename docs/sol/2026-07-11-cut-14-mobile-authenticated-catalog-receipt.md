# CUT-14 mobile authenticated catalog receipt

- Date: 2026-07-11
- Issue: [#8694](https://github.com/OpenAgentsInc/openagents/issues/8694)
- Status: deterministic catalog/navigation core active; visible directory,
  production link/notification delivery, and physical iOS/Android receipts
  remain open
- Contract: `openagents_mobile.coding.authenticated_navigation.v1`

## Greenfield ownership

CUT-14 lands in `apps/openagents-mobile`. The older
`clients/khala-mobile` Expo app is frozen migration source and receives no new
product feature.

The mobile Sync host now creates the CUT-13 confirmed coding-catalog reader for
the exact server-verified personal scope. The host exposes one stable mobile
navigation service while replacing its hosted catalog and owner authority on
connect, disconnect, denial, and unlink. The service lists only available,
grant-eligible repositories and non-archived sessions from a live confirmed
snapshot, sorted by recent activity. Signed-out or non-live phases return no
repository/session rows and name whether cache authority is withheld or purged
after denial.

## Stable target and recovery contract

Deep links use one bounded form:

`openagents://coding/session/{sessionRef}?repository={repositoryRef}&thread={threadRef}`

Notification inputs use the same three refs under
`openagents.mobile.coding_target.v1`. Both paths decode with Effect Schema and
then resolve exact refs against the current live owner scope. Cross-owner,
stale repository/session/thread, unavailable authority, invalid catalog, and
revoked/unprojected grant states fail closed.

After a target has live authority and its thread lease opens, mobile persists
only its owner/repository/session/thread refs, source, and timestamp in the
device-local Sync SQLite store. The record contains no path, token, body, host,
or runtime handle. A real store close/reopen proves the same target restores
after process death only when the authenticated owner and confirmed catalog
still authorize it. Relinking a different owner rejects the retained target.

## Subscription fencing

Each activation receives a monotonically increasing in-process generation.
The prior thread lease closes before the next one becomes current; a lease
that opens after it was superseded closes immediately, and its late update
callback cannot enter the current projection. This is the deterministic
boundary for switching repositories/sessions without cross-scope content
leakage. Visible Home integration will bind this lease to the conversation
subscription in the next CUT-14 tranche.

## Verification

- Focused mobile coding + Sync host: 12 pass, 0 fail, 56 expectations.
- Full `@openagentsinc/openagents-mobile`: 71 pass, 0 fail, 316 expectations.
- Mobile typecheck: pass.
- Real SQLite close/reopen: pass with stable refs and no path/token material.
- Deep-link/notification, owner mismatch, revocation, stale target, non-live
  cache withholding, and concurrent activation fault cases: pass.

## Residual

CUT-14 remains open. The next tranche must render the authorized repository and
recent-session directory in the Effect Native Home surface, wire native URL
and notification delivery through the resolver, and bind selected threads to
the generation-fenced lease. Physical iOS and Android process-death/reconnect
receipts remain required and owner-deferred while the recording phone is not
available.
