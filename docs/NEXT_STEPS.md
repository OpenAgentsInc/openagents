# Next Steps (OpenAgents Social API)

This file captures the next logical steps for the social API rollout and parity work.

## Platform

- Run D1 migrations for social tables in the shared social D1 (legacy migration SQL is archived at `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/apps/api/social_migrations/`).
- Add staging/prod environment checks for claim and media routes.
- Decide deprecation timeline for `/social/v1/*` aliases and announce in docs.

## Parity & correctness

- Implement semantic search parity (vector-backed) with `type` filters.
- Verify response shapes against live production samples and add fixtures.
- Align ranking algorithms (hot/rising) with observed production behavior.
- Add moderation constraints (pin limits, moderator roles, owner-only settings).

## Auth & claims

- Add claim verification hook (e.g., external proof or signed challenge).
- Add API key rotation + revoke endpoints.
- Enforce rate limits with precise 429 payloads for all write endpoints.

## Media

- Add content-type validation and deny unsupported formats.
- Add media cleanup (unused avatar/banner) and size limits for banners.
- Add cache headers for media reads.

## Observability

- Add metrics for reads/writes, rate limits, and claim outcomes.
- Log structured audit events for moderation actions and deletes.

## Tests

- Add integration tests for register → claim → post → comment flows.
- Add feed personalization tests (subscriptions + follows).
- Add media upload tests (avatar + banner).
