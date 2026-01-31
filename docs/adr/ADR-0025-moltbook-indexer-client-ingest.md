# ADR-0025: Moltbook Indexer Client Ingest Endpoint

## Status

Accepted

## Date

2026-01-31

## Context

The Autopilot desktop Moltbook pane fetches posts on-demand. To keep the OpenAgents Moltbook index current, we need a lightweight way for clients to upsert freshly fetched posts without waiting for the cron ingestion cycle.

## Decision

We will expose a client ingest endpoint on the Moltbook indexer Worker that accepts raw post objects and upserts them into R2/D1, reusing the existing normalization and secret-scanning logic.

> We will add `POST /api/indexer/v1/ingest/posts` to accept a JSON payload containing a `posts` array (and optional `source` string). The indexer will ignore duplicates, store raw JSON in R2, normalize into D1, and enqueue comment ingestion when `comment_count > 0`.

## Scope

This ADR covers:
- The new `POST /api/indexer/v1/ingest/posts` endpoint.
- The ingestion behavior for client-supplied posts (idempotent upsert).

This ADR does NOT cover:
- Client authentication beyond existing optional indexer auth.
- Comment ingestion payloads (comments remain fetched by the indexer queue).

## Invariants / Compatibility

| Invariant | Guarantee |
| --- | --- |
| Endpoint path | Stable: `/api/indexer/v1/ingest/posts` |
| Payload shape | Stable: JSON object with `posts` array |
| Idempotency | Duplicate `id` values are ignored |
| Storage | Raw JSON in R2, normalized in D1, signals derived |

Backward compatibility expectations:
- Additive only. Existing endpoints remain unchanged.

## Consequences

**Positive:**
- Indexer stays current when clients already fetched posts.
- Avoids unnecessary double-fetching from Moltbook for recent posts.

**Negative:**
- Larger surface area on the indexer API.

**Neutral:**
- Comment ingestion remains queue-based.

## Alternatives Considered

1. **Call `/ingest` from clients** — Rejected; redundant fetches and slower propagation.
2. **Add a queue-only endpoint** — Rejected; more moving parts without clear benefit.

## References

- `apps/indexer/src/index.ts`
- `apps/indexer/README.md`
- `apps/autopilot-desktop/src/main.rs`
