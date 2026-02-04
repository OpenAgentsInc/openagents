# Decision: keep the Rust API for the initial OpenClaw release

## Executive summary

It is **not advisable** to remove the Rust API (`apps/api`) dependency for the initial version. While it looks like “one less service,” removing it would push the same responsibilities into `apps/web` or Convex, reintroduce known failure modes ("Only HTML" server-function errors), and weaken the auth/billing boundary. The Rust API already exists, is wired end-to-end, and is the canonical `/api/*` surface. The simplest, lowest‑risk path is to keep it as a thin proxy and avoid re‑platforming the same logic elsewhere.

## Current dependency map (what already relies on `apps/api`)

- Hatchery provisioning and status:
  - Browser → Convex actions → `apps/api` `/openclaw/instance` → Convex HTTP routes.
- OpenClaw runtime controls:
  - `apps/web` server routes call `apps/api` for runtime status/devices/approve/restart/backup/billing.
- OpenClaw tools and sessions:
  - `apps/api` proxies to `apps/openclaw-runtime` for tools + sessions endpoints.
- OpenClaw WebChat:
  - `apps/api` provides `/api/openclaw/chat` streaming proxy over runtime `/v1/responses`.

These are already implemented and documented in:
- `apps/web/docs/openclaw-on-openagents-com.md`
- `apps/web/docs/openclaw-hatchery-architecture.md`
- `apps/web/docs/cloudflare-agents-sdk-openagents-com.md`

## Option A: remove the Rust API for the initial version (what would change)

To remove `apps/api` from the critical path, we would need to move its responsibilities into either `apps/web` or Convex:

1. **Hatchery provisioning path**
   - Move `apps/api`’s Convex-bridge logic into Convex actions directly (no HTTP hop), or
   - Move it into `apps/web` server routes with a Convex control key.

2. **Runtime control and tools**
   - `apps/web` (server) or Convex actions would call `apps/openclaw-runtime` directly, carrying `OPENAGENTS_SERVICE_TOKEN`.

3. **Streaming chat**
   - `apps/web` server routes would need to proxy SSE from runtime `/v1/responses` directly (no `/api/openclaw/chat`).

4. **Auth + billing boundary**
   - Anything handled today by `apps/api` (auth token verification, user scoping, billing/limits) must be recreated in `apps/web` or Convex.

5. **Routing constraint**
   - `/api/*` is already owned by `apps/api`. Removing it from the path means re‑routing or duplicating endpoints under a different path, which breaks existing client assumptions and docs.

## Pros of removing the Rust API

- Fewer moving pieces to deploy in the very short term.
- One less hop in the request chain (small latency improvement).
- Slightly simpler local debugging if everything runs in `apps/web` + Convex.

## Cons and risks

- **Reintroduces known failure modes**: the "Only HTML requests" issue was a driver for the Convex → Rust API path; moving logic back into `apps/web` re‑opens that risk.
- **Security boundary gets weaker**: `apps/api` isolates internal keys (Convex control key, runtime service token). Moving these into `apps/web` or Convex increases blast radius and secret sprawl.
- **More total code change**: removing the dependency is not just deletion; it requires rebuilding routes, auth, and streaming proxy logic.
- **Loss of canonical API surface**: external clients and future integrations expect `/api/*` to be stable and Rust‑owned. Removing it now creates churn we will have to undo.
- **Docs and operational tooling already assume `apps/api`**: removing it means large doc churn and a high‑risk migration while we still need to ship milestones.

## Recommendation

**Keep the Rust API dependency for the initial release.** It is already functioning, aligns with the existing routing constraints, and provides the cleanest, most secure boundary for auth/billing and service tokens. The lowest‑risk approach is to keep `apps/api` thin and focused (proxy + auth + billing), and avoid re‑implementing its responsibilities in `apps/web` or Convex.

## If we must de‑risk further without removing `apps/api`

If the goal is simplification rather than elimination, the safe alternative is:

- Keep `apps/api` in place but keep it minimal:
  - Strict request validation and auth.
  - Thin proxy to Convex and `apps/openclaw-runtime`.
- Avoid adding new business logic to `apps/api` unless it is auth/billing or routing.

This preserves the boundary while keeping operational complexity small.

## “Only if forced” fallback plan (not recommended)

If we ever **must** remove `apps/api` temporarily (e.g., severe outage), the least bad path is:

1. Move Hatchery provisioning and status into Convex actions directly.
2. Add direct runtime proxying in `apps/web` server routes for tools/chat.
3. Gate access using WorkOS auth + internal server secrets.
4. Keep `/api/*` route reserved for re‑introducing the Rust API later.

This should be treated as a short‑lived contingency path, not the default architecture.
