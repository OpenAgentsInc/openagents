# API keys + agent signup (chat-first OpenAgents) — Spec

Date: 2026-02-02

We want **all relevant OpenAgents functionality** (including Managed OpenClaw control) accessible via **API keys**, not only via WorkOS web auth.

This spec defines:
- a single API key model (user-scoped tokens)
- how agents/bots can “sign up” quickly (one HTTP call)
- how API keys authorize requests to `/api/openclaw/*` and other services
- optional Nostr integrations (including where NIP-42 could fit later)

---

## 0) Current state (important context)

### A) Rust API already has API key concepts
Repo: `apps/api/` (Rust worker at `https://openagents.com/api/*`)

Existing key patterns:
- **Control-plane** key (query/header `x-api-key` / `api_key`) used for `/projects`, `/tokens`, `/nostr/verify`, etc.
- **Social API keys** stored in D1 for Moltbook parity (`Authorization: Bearer <api_key>`, `x-moltbook-api-key`, etc.).
- Moltbook Developers flow exists for short-lived **identity tokens**:
  - `POST /agents/me/identity-token`
  - `POST /agents/verify-identity`
  Doc: `apps/api/docs/moltbook-developers.md`

### B) Convex already has a “user-scoped API token” implementation in legacy app
Legacy location:
- `apps/website-old2/convex/apiTokens.ts`

It implements:
- token generation (32 bytes)
- token hashing (SHA-256)
- storage in `api_tokens` table
- create/list/revoke/resolve/touch operations

This is the model we will standardize on for **OpenAgents API keys**.

### C) New main app is chat-first
Main app:
- `apps/web/`
- chat endpoint: `/chat`

Web auth is WorkOS, but API key users should be able to operate without WorkOS.

---

## 1) Goal

1) Provide a **single API key** (“OpenAgents API token”) usable for:
- calling OpenAgents APIs programmatically
- calling managed OpenClaw control endpoints (`/api/openclaw/*`)
- agent/bot usage that doesn’t want interactive WorkOS login

2) Provide a **one-shot signup** endpoint for agents:
- “give me an identity + token now” (no UI)

3) Preserve existing social/moltbook API keys for their existing domain, but do not force them as the general auth mechanism.

---

## 2) Canonical API key model

### 2.1 Token semantics
- A token is an opaque string, e.g. 64 hex chars.
- Store only a hash in DB (SHA-256).
- Tokens are user-scoped (belong to an OpenAgents “principal”).

### 2.2 Principal types
We support two principal types:
- **human user** (WorkOS-backed account)
- **agent user** (headless principal created by API)

### 2.3 Token table
In Convex schema (new main app):
- `apps/web/convex/schema.ts`

Add (port from `apps/website-old2/convex/schema.ts`):
- `api_tokens` table
  - `user_id` (string)
  - `token_hash` (string)
  - `name` (string)
  - `created_at` (number)
  - `last_used_at` (optional number)
  - `expires_at` (optional number)

Add indexes:
- `by_user_id`
- `by_token_hash`

---

## 3) API key auth: how requests authenticate

### 3.1 Header
All API-key authenticated requests use:
- `Authorization: Bearer <OPENAGENTS_API_TOKEN>`

(Optionally accept `x-api-key: <token>` for compatibility, but **prefer Authorization**.)

### 3.2 Introspection / resolution
Where verification happens:
- Rust API (`apps/api`) resolves the token by calling Convex via a control endpoint.

Implementation:
- Add a Convex HTTP endpoint in `apps/web/convex/http.ts` that supports:
  - `POST /control/auth/resolve-token`
  - Request: `{ token: "..." }`
  - Response: `{ ok: true, data: { user_id: string, tokenId: string, tokenHash: string } }`

Convex handler should call the ported function from:
- `apps/web/convex/apiTokens.ts` (ported from old2)

Also update `last_used_at` on successful resolve.

---

## 4) “Agent quick signup” endpoint

We need a public endpoint that returns a principal identity + API key in one call.

### 4.1 Endpoint
Add in Rust API (`apps/api/src/lib.rs`) under a new namespace:
- `POST /auth/agent/register`

Request:
```json
{
  "name": "optional display name",
  "metadata": {"...": "..."},
  "nostr": {
    "pubkey": "optional hex pubkey",
    "nip98": "optional NIP-98 Authorization value"
  }
}
```

Response:
```json
{
  "ok": true,
  "data": {
    "user_id": "oa_agent_...",
    "api_token": "oa_tok_...",
    "created": true
  }
}
```

### 4.2 What it does
- Creates an **agent principal** in Convex (a row in a `users` table or a new `principals` table).
- Issues an API token (`api_tokens`) via Convex internal mutation.

### 4.3 Abuse controls
- rate limit by IP (basic)
- optional CAPTCHA later
- optionally require a “registration key” header in beta:
  - `X-OA-Register-Key: <secret>`

---

## 5) Apply API key auth to Managed OpenClaw control

Currently chat tools call `/api/openclaw/*` using beta internal headers (`X-OA-Internal-Key`, `X-OA-User-Id`).

We want:
- API key callers can call these endpoints directly

### 5.1 Rust API change
In `apps/api`, for all `/openclaw/*` endpoints:
- Accept `Authorization: Bearer <oa_api_token>`
- Resolve token → `user_id`
- Use that user_id for instance operations

Keep internal headers as a fallback for server-side calls from `apps/web` during beta.

Auth precedence:
1) If internal headers present and valid → use them
2) Else if Authorization Bearer present → resolve token via Convex
3) Else 401

---

## 6) Nostr integration (where NIP-42 fits)

### 6.1 HTTP identity proofs (today)
- For HTTP, NIP-98 already exists in the codebase (`/nostr/verify` in control plane).
- We can allow `/auth/agent/register` to optionally accept NIP-98 proof of a pubkey.

### 6.2 NIP-42 (relay auth) (later)
NIP-42 is for authenticating to a **Nostr relay**, not HTTP.

Where it can fit:
- If we run a private relay (e.g. Nexus) that requires NIP-42:
  - We can allow “agent signup” by posting a signed AUTH event to the relay
  - The relay issues a one-time code / token to exchange for an OpenAgents API token

This is optional and not required for MVP.

---

## 7) Coding agent implementation plan (filepaths)

### 7.1 Port Convex token model into apps/web
From legacy:
- `apps/website-old2/convex/apiTokens.ts`
- `apps/website-old2/convex/schema.ts` (api_tokens table)

To new main app:
- `apps/web/convex/schema.ts`
- `apps/web/convex/apiTokens.ts` (port)
- `apps/web/convex/http.ts` + `apps/web/convex/control_http.ts`
  - add `/control/auth/resolve-token`

### 7.2 Rust API
- Add `/auth/agent/register`
- Add token auth resolver helper in `apps/api/src/lib.rs`:
  - parse `Authorization: Bearer`
  - call Convex control endpoint to resolve token

### 7.3 Web chat
- Keep `apps/web/src/routes/api/chat.ts` using internal headers for now.
- Later: allow users to create/manage API tokens from the UI.

---

## 8) Definition of done

- A bot can call `POST https://openagents.com/api/auth/agent/register` and receive `{user_id, api_token}`.
- That `api_token` works with `Authorization: Bearer ...` on:
  - `https://openagents.com/api/openclaw/*`
- Tokens are stored hashed; last_used_at updates.
- Token revocation works.
