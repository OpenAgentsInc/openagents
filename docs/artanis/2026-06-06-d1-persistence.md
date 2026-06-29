# Artanis D1 Persistence

Issue #403 / `ARTANIS-017` adds the first durable Artanis persistence family in
OpenAgents product surface.

The implementation lives in:

- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/src/artanis-persistence.ts`
- `workers/api/src/artanis-persistence.test.ts`

## Stored Record Families

The migration creates these `artanis_*` tables:

- `artanis_runtime_snapshots`
- `artanis_loop_records`
- `artanis_loop_ticks`
- `artanis_approval_gates`
- `artanis_health_snapshots`
- `artanis_work_routing_proposals`
- `artanis_forum_publication_intents`

Every row carries a stable `record_ref`, `idempotency_key`, public projection
JSON, original contract record JSON, content hash, state, created/updated
timestamps, and optional closeout fields. Loop records also enforce one active
loop per agent and scope.

## Repository Boundary

`artanis-persistence.ts` persists the existing Artanis contract records instead
of inventing a second data model. It stores:

- `ArtanisRuntimeRecord`
- `ArtanisLoopRecord`
- `ArtanisLoopTickRecord`
- `ArtanisApprovalGateRecord`
- `ArtanisHealthSnapshotRecord`
- `ArtanisWorkRoutingProposalRecord`
- `ArtanisForumPublicationIntentRecord`

Writes are idempotent. A retry with the same idempotency key and identical
record/projection content returns an idempotent receipt. Reusing an
idempotency key or stable record ref with different content is rejected as a
conflict.

Loop ticks can be closed out idempotently with public-safe closeout receipt
refs. A conflicting closeout retry is rejected.

## Authority Boundary

Persistence is evidence, not execution authority.

Persisted records and write receipts explicitly set `executableAuthority` to
`false`. Saving an approved gate does not dispatch work. Saving a ready Forum
intent does not post it. Saving a work-routing proposal does not mutate a
provider, spend money, or settle a payout.

The next issue, #404, can build a scheduled tick runner on top of these rows,
but it still needs explicit approval gates for risky actions.

## Redaction

The repository derives public projections through the already implemented
Artanis projection contracts before persisting. Public projections reject or
redact private evidence, provider credentials, runner data, wallet material,
raw payment material, raw logs, private repos, customer data, and raw
timestamps according to the underlying contract.

Tests cover migration coverage, insert/retry behavior, duplicate suppression,
conflict rejection, closeout idempotency, projection reads, and the
non-authority boundary.
