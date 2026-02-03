# Checklist — API keys + agent quick signup

This checklist implements the spec:
- `docs/api-keys-and-agent-signup-spec.md`

## Read first (required)
1) `docs/api-keys-and-agent-signup-spec.md`
2) Legacy Convex implementation to port:
   - `apps/website-old2/convex/apiTokens.ts`
   - `apps/website-old2/convex/schema.ts` (api_tokens table)
3) Rust API context:
   - `apps/api/src/lib.rs`
   - `apps/api/README.md`
   - `apps/api/docs/moltbook-developers.md` (identity-token/verify flow)

---

## Phase 0 — Design decisions to confirm (before coding)
- [ ] Token format: keep 64-hex random (32 bytes) as in old2.
- [ ] Header: canonical `Authorization: Bearer <token>` (accept `x-api-key` optionally).
- [ ] Beta registration protection: decide whether to require `X-OA-Register-Key` for `/auth/agent/register`.

---

## Phase 1 — Convex (apps/web) API token system

### 1.1 Schema
- [ ] Update `apps/web/convex/schema.ts`:
  - [ ] Add `api_tokens` table with fields:
    - `user_id`, `token_hash`, `name`, `created_at`
    - optional `last_used_at`, `expires_at`
  - [ ] Add indexes:
    - `by_user_id`
    - `by_token_hash`

### 1.2 Port functions
- [ ] Add `apps/web/convex/apiTokens.ts` (port from `apps/website-old2/convex/apiTokens.ts`)
  - [ ] `createApiToken` (user creates token)
  - [ ] `listApiTokens`
  - [ ] `revokeApiToken`
  - [ ] `resolveApiToken` (internal query: token -> {user_id, tokenHash, tokenId})
  - [ ] `updateApiTokenLastUsed` (internal mutation)
  - [ ] `issueApiTokenForUser` (internal mutation)

### 1.3 Control endpoint for Rust to resolve tokens
- [ ] Add/extend Convex HTTP endpoints in `apps/web/convex/http.ts` and handler file (whatever the template uses) to expose:
  - [ ] `POST /control/auth/resolve-token`
  - Input: `{ token: string }`
  - Output: `{ ok: true, data: { user_id, tokenHash, tokenId } }` or 401
- [ ] Gate it with a control key header similar to old patterns (`x-oa-control-key`).

---

## Phase 2 — Rust API (`apps/api`) token auth + agent signup

### 2.1 Token parsing helper
- [ ] In `apps/api/src/lib.rs`, implement helper:
  - Parse bearer token: `Authorization: Bearer <token>`
  - Optional: accept `x-api-key: <token>`

### 2.2 Convex resolve call
- [ ] Reuse existing Convex control bridge (already present in `apps/api/src/lib.rs`) to call:
  - `POST control/auth/resolve-token`

### 2.3 Endpoint: agent quick signup
- [ ] Add route:
  - [ ] `POST /auth/agent/register`
- [ ] Handler actions:
  - [ ] Create “agent principal” in Convex (user record or principal record)
  - [ ] Issue token via `issueApiTokenForUser`
  - [ ] Return `{ user_id, api_token }`
  - [ ] Rate limit + optional `X-OA-Register-Key` gating

### 2.4 Apply API token auth to OpenClaw endpoints
- [ ] For all `/openclaw/*` endpoints in Rust:
  - [ ] Auth precedence:
    1) internal headers (`X-OA-Internal-Key`, `X-OA-User-Id`) if present + valid
    2) else bearer token resolve via Convex
    3) else 401

---

## Phase 3 — “API keys everywhere” rollout
- [ ] Document auth for `/api/openclaw/*` in `apps/api/docs/`.
- [ ] Optional: add UI in `apps/web` for humans to create/revoke API tokens.

---

## Acceptance tests

### A) Agent signup
- [ ] `curl -X POST https://openagents.com/api/auth/agent/register ...` returns `api_token`.

### B) Token works for OpenClaw
- [ ] `curl -H "Authorization: Bearer <api_token>" https://openagents.com/api/openclaw/runtime/status` returns 200.

### C) Revocation
- [ ] Revoke token, repeat request should 401.
