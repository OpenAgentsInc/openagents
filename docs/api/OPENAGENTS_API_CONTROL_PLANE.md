# OpenAgents Control Plane API (Spec)

This document describes the intended public control-plane API exposed under:

- `https://openagents.com/api`

Canonical ADR:
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0026-openagents-control-plane-api.md`

## Goals

- Provide a single public API base for control-plane entities (orgs/projects/repos/issues/tokens).
- Keep Khala internal; expose only via the Cloudflare Worker boundary.
- Use API tokens (`Authorization: Bearer <token>`) for authentication.

## Endpoints (Relative To `/api`)

Auth:
- `POST /register`
- `GET|POST|DELETE /tokens`

Entities:
- `GET|POST /organizations`
- `GET|POST /projects`
- `GET|POST|PATCH|DELETE /issues`
- `GET|POST|DELETE /repos`

Nostr helper endpoints:
- `GET /nostr`
- `POST /nostr/verify` (NIP-98 HTTP auth verification)

## Auth

- Client: `Authorization: Bearer <api_token>`
- Worker injects internal control key when proxying to Khala control endpoints.

## Compatibility

- Paths are stable under `/api/*` even if internal implementation changes.
- Schema evolution should be additive where possible; breaking changes require an ADR and versioning strategy.

