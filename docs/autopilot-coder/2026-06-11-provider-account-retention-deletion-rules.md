# Provider Account Retention And Deletion Rules

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

## Scope

This is the Pack B retention/deletion record for provider-account credentials,
account leases, account-health telemetry, provider-routing decisions, policy
snapshots, reconnect state, debug/support records, artifacts, and receipts.

It supports #4829 under the #4824 Pack B parent and is an input to #4771
provider-peer closeout.

## Data Classes

Every Pack B provider-account data class must declare:

- retention class
- deletion behavior
- projection invalidation behavior
- audit ref when a ref-only audit trail is retained

The typed declaration lives in
`apps/openagents.com/workers/api/src/provider-account-retention-policy.ts`.

## Deletion Behavior

- User deletion deletes or redacts provider-account state, invalidates active
  account leases, tombstones redacted account refs, and keeps only deletion
  receipt refs plus retained audit refs.
- Team deletion applies the same invalidation behavior for team-scoped account
  state, policy snapshots, telemetry aggregates, artifacts, and receipts.
- Provider-account deletion invalidates credential-boundary projections,
  telemetry projections, reconnect state, and active account leases tied to
  that account.
- Credential revocation invalidates dependent leases immediately, produces
  typed dependent blockers, emits provider-account cache invalidation refs, and
  creates reconnect action refs when the account can be reconnected.
- Retention expiry invalidates affected projections and cache entries without
  creating lease blockers unless the expired data is a live credential or
  account authority ref.

## Tombstones And Receipts

Tombstones, deletion receipts, retained audit refs, artifact refs, and receipt
refs are evidence handles only. They must not contain raw credentials, OAuth
material, raw prompts, transcripts, shell output, private repo data, raw
provider responses, wallet/payment material, customer-private data, or local
paths.

## Projection Invalidation

Deletion and retention events may invalidate:

- credential-boundary projections
- account-health telemetry projections
- provider-routing projections
- policy snapshots
- reconnect state
- debug/support bundles
- artifact projections
- receipt projections

Public and agent-readable views should treat stale projections as evidence,
not fresh authority. Dependent runs must cite the new typed blockers or
reconnect action refs before claiming provider-account readiness.
