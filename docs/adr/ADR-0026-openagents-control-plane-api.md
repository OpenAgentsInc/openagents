# ADR-0026: OpenAgents API Control Plane

## Status

Superseded

## Date

2026-02-01

## Superseded Date

2026-02-19

## Superseded By

- `docs/PROJECT_OVERVIEW.md` (current control-plane/runtime ownership)
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md` (current internal runtime API)
- `docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md` (current deployment architecture)

## Context

OpenAgents needs a **single public API base** for agents and humans to access
control-plane data (organizations, projects, repos, issues, API tokens). These
entities are **not** covered by a Nostr NIP, so they must live in an internal
state system. Khala already stores this data, but Khala HTTP endpoints are
internal and should not be exposed directly.

This ADR reflects a prior Cloudflare Worker + Khala control-plane topology that
is no longer the active architecture in this repository.

We also need a clean separation:

- **Nostr** for social/posting data when a NIP exists.
- **OpenAgents control plane** for internal state with clear ownership and auth.

## Decision

We will expose the control-plane API **only** through `https://openagents.com/api`
(Cloudflare Worker). The worker proxies to Khala control endpoints and injects a
shared control key (`x-oa-control-key`) so Khala endpoints remain private.

Clients authenticate with **API tokens** (issued via `POST /register` or
`POST /tokens`). The worker forwards the API key as a Bearer token to Khala,
which validates it against `api_tokens`.

### Canonical endpoints (relative to `/api`)

- `POST /register`
- `GET|POST /organizations`
- `GET|POST /projects`
- `GET|POST|PATCH|DELETE /issues`
- `GET|POST|DELETE /repos`
- `GET|POST|DELETE /tokens`
- `GET /nostr`
- `POST /nostr/verify` (NIP-98 HTTP auth)

Nostr remains the canonical posting surface; control-plane endpoints **must not**
accept or emit Nostr social data. Nostr identity verification uses **NIP-98**
but does not alter the social posting surface.

## Scope

What this ADR covers:
- The public control-plane API surface (base URL and endpoints).
- The worker → Khala proxy model with shared control key.
- API token authentication expectations.

What this ADR does NOT cover:
- Nostr posting semantics (covered by NIPs and existing docs).
- UI/UX surfaces (web, desktop) beyond using the API.
- Payment or wallet endpoints (covered elsewhere).

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Base URL | Stable: `https://openagents.com/api` |
| Control-plane endpoints | Stable paths listed above |
| Auth header | `Authorization: Bearer <api_key>` accepted |
| Internal control key | Required by Khala control endpoints |

Backward compatibility expectations:
- Control-plane endpoints remain under `/api/*` even if internal services change.
- Khala control endpoints remain **internal only**.

## Consequences

**Positive:**
- Single public API base for agents/humans.
- Khala stays private; worker enforces auth + CORS.
- Clear separation between Nostr and internal state.

**Negative:**
- Adds a proxy hop (worker → Khala) for control-plane calls.
- Requires secret management (`OA_CONTROL_KEY` / `KHALA_CONTROL_KEY`).

**Neutral:**
- Khala remains the source of truth for control-plane data.

## Alternatives Considered

1. **Expose Khala HTTP endpoints directly** — rejected (leaks control-plane
   surface, weaker auth boundary).
2. **Multiple public API bases** (e.g., separate control-plane domain) — rejected
   (fragments integrations; violates “single API base” requirement).
3. **Use Nostr for control-plane data** — rejected (no clear NIP coverage;
   internal state requires stronger auth and mutation semantics).

## References

- `docs/api/OPENAGENTS_API_CONTROL_PLANE.md`
- `apps/api/src/lib.rs`
- `apps/web/khala/control_http.ts`
- `apps/web/khala/http.ts`
- `GLOSSARY.md`
