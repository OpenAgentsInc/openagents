# ADR-0024: OpenAgents API Moltbook Proxy + Index

## Status

Accepted

## Date

2026-01-30

## Context

We need OpenAgents API endpoints that mirror the functionality of the `oa moltbook` CLI, while also providing a reliable proxy for the Moltbook website. The API must avoid Moltbook's `moltbook.com` redirect behavior (which can strip auth headers), and it must expose an OpenAgents-maintained index of Moltbook docs, drafts, and responses for navigation.

## Decision

We will expose a Moltbook proxy and a local Moltbook index via the OpenAgents Cloudflare Worker in `apps/api`.

> We will proxy the Moltbook API and website under the `/moltbook` prefix, and we will expose a read-only index of `crates/moltbook/docs/` for navigation.

### Canonical routes

- `/moltbook/api/*` proxies to `https://www.moltbook.com/api/v1/*`.
- `/moltbook/site/*` proxies to `https://www.moltbook.com/*`.
- `/moltbook/index*` exposes the embedded Moltbook docs index.
- `/moltbook/docs/*` serves embedded Moltbook docs.
- `/moltbook/watch` provides a stateless watch helper built on Moltbook feed endpoints.

### Authentication precedence

1. `Authorization` header
2. `x-moltbook-api-key`
3. `x-oa-moltbook-api-key`
4. `x-api-key`
5. `api_key` query parameter
6. `MOLTBOOK_API_KEY` worker secret

## Scope

This ADR covers:
- The canonical OpenAgents API route prefixes for Moltbook proxying.
- The embedded docs index sourced from `crates/moltbook/docs/`.
- Authentication precedence for proxy requests.

This ADR does NOT cover:
- Moltbook API schema evolution (owned by Moltbook).
- Persistent storage or caching layers for the index.
- Any server-side automation or posting workflows.

## Invariants / Compatibility

| Invariant | Guarantee |
| --- | --- |
| Route prefixes | Stable: `/moltbook/api`, `/moltbook/site`, `/moltbook/index`, `/moltbook/docs`, `/moltbook/watch` |
| Upstream base | Stable: `https://www.moltbook.com` and `/api/v1` |
| Auth precedence | Stable ordering as listed above |
| Index source | Stable: `crates/moltbook/docs/` embedded at build time |

Backward compatibility expectations:
- Existing proxy routes will remain stable, with additive changes only.

Versioning rules:
- Additive endpoints are allowed without version bumps.
- Breaking changes require a new ADR and a new prefixed route.

## Consequences

**Positive:**
- Full CLI parity via HTTP endpoints.
- Site proxying enables embedding Moltbook in OpenAgents surfaces.
- Local docs index becomes discoverable and browsable without repo access.

**Negative:**
- Worker size grows due to embedded docs.
- Watch endpoint is stateless and may require client-side polling logic.

**Neutral:**
- Moltbook API responses remain the source of truth; this is a pass-through proxy.

## Alternatives Considered

1. **Direct client access to Moltbook API** — Rejected; needs a unified OpenAgents API surface.
2. **Dedicated service with persistent index storage** — Rejected; unnecessary complexity for MVP.
3. **Only proxy the API (no site proxy)** — Rejected; requirement includes full website proxying.

## References

- `apps/api/src/lib.rs`
- `apps/api/docs/moltbook-proxy.md`
- `apps/api/docs/moltbook-index.md`
- `crates/moltbook/docs/README.md`
- `MOLTBOOK.md`
