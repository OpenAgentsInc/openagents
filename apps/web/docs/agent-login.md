# Agent login (service accounts + API keys) — design plan

This doc proposes an **agent signup/login** path that does **not** rely on WorkOS AuthKit (GitHub OAuth), while still fitting cleanly into the existing OpenAgents stack:

- `apps/web` (TanStack Start UI + server routes)
- Convex (data + actions/mutations/queries)
- `apps/api` (Rust API worker, internal-key gated, proxies to runtime)
- `apps/openclaw-runtime` (runtime, service-token gated)

The goal is to give autonomous agents a first-class way to authenticate and act as a “principal” without a human OAuth session.

## Problem statement

### Current state
- Web app identity is **WorkOS AuthKit only**.
- Convex auth is derived from WorkOS access tokens (client + SSR), so the “user id” used across the system is effectively the WorkOS `user.id`.
- Many internal actions (Convex → API worker) are gated by `OA_INTERNAL_KEY`, which is **not** an agent/user credential. It’s a service secret.

### What breaks for agents
- OAuth + GitHub login is not a usable primitive for headless agents.
- We need **key-based** auth (service accounts), with scopes, rotation, revocation, and rate limiting.
- We likely need a higher-level abstraction than “WorkOS user” to represent “who is calling” across UI/Convex/API/runtime.

## Design goals

- **Agent-first auth**: API keys + scopes + rotation, no OAuth.
- **Unified principals**: one abstraction that covers both humans and agents.
- **Least privilege**: keys have explicit scopes (OpenClaw, chat, read-only, etc.).
- **Revocation and rotation**: keys can be revoked; sessions expire.
- **No secrets in git**: keys are one-time returned and stored locally (gitignored).
- **Minimal disruption**: human flows continue to use WorkOS unchanged initially.

## Proposed abstraction: “Principal”

Introduce a shared “principal” concept used by server routes, Convex actions, and API worker auth layers.

```ts
type Principal =
  | { kind: "human"; id: string; workosUserId: string }
  | { kind: "agent"; id: string; agentUserId: string; ownerWorkosUserId?: string | null };
```

Key idea: **the system routes and authorizes based on `Principal`**, not on “WorkOS user” directly.

## Two viable implementation paths

### Option 1 (recommended): agents authenticate to `apps/api` (Rust) directly

Agents use an API key to call the Rust worker. The Rust worker:

1. validates the agent API key
2. maps it to `agentUserId`
3. performs any Convex reads/writes server-to-server (using internal control creds)
4. calls `openclaw-runtime` using `OPENAGENTS_SERVICE_TOKEN` (already in place)

Pros:
- Doesn’t require Convex “end-user auth” changes right away.
- Keeps agent credentials out of the browser entirely (by default).
- Centralizes abuse controls (rate limits, scopes) at the API worker.

Cons:
- Agent can’t directly use Convex from the browser/client without additional work (but that’s usually fine for headless agents).

### Option 2: agents get a Convex-auth token (custom auth)

Agents exchange API keys for a short-lived signed token that Convex can treat as identity (subject = `agent:<id>`).

Pros:
- Agents can use Convex client APIs directly (useful for “agent UI” or local tools).

Cons:
- Requires implementing and maintaining a robust custom-auth token pipeline.
- More foot-guns around token leakage in browsers.

We can do Option 1 first, then add Option 2 if/when needed.

## Data model in Convex (draft)

Create agent-specific tables in `apps/web/convex/schema.ts`:

- `agent_users`
  - `_id`
  - `handle` (human-readable name, optional)
  - `createdAt`
  - `ownerWorkosUserId` (optional; if agents are created by a logged-in human)
  - `status` (`active` / `disabled`)

- `agent_api_keys`
  - `_id`
  - `agentUserId` (ref -> `agent_users`)
  - `keyId` (public identifier, e.g. `oak_live_...`)
  - `keyHash` (hash/HMAC of the secret key material)
  - `createdAt`
  - `lastUsedAt`
  - `revokedAt`
  - `scopes` (array of strings)
  - `description` (optional)

### Key storage and verification (important)
Never store raw API keys in Convex.

Preferred approach:
- Generate key material server-side.
- Store `keyHash = HMAC-SHA256(server_secret, key_material)` (or `SHA256(salt + key)` with a server secret).
- Compare hashes in constant time.

This implies we need a server secret available to whichever component verifies keys:
- If verification happens in `apps/api` (Rust): store the HMAC secret as a Wrangler secret on that worker.
- If verification happens in Convex actions: store the secret in Convex env vars.

## Proposed endpoints (draft)

### Agent signup / provisioning

We need a safe way to create agent identities. There are two sub-modes:

1) **Human-created agent (safe default)**
- Logged-in human uses UI to “Create agent key”.
- Ownership is recorded (`ownerWorkosUserId`).

2) **Self-serve agent signup (public)**
- Anonymous call to create an agent principal and receive a key.
- Must include anti-abuse (invite code, rate limits, captcha, payment, etc.).

Suggested API worker endpoints:

- `POST /agent/signup`
  - creates `agent_user` + initial key
  - returns `agentUserId` and **one-time** `apiKey`

- `POST /agent/keys`
  - authenticated (human or agent)
  - creates a new key for an agent user

- `POST /agent/session`
  - optional, for browser-like clients
  - exchanges API key for a short-lived session token/cookie

- `POST /agent/keys/:keyId/revoke`

### Authentication on API worker

Accept one of:
- `Authorization: Agent <api_key>`
- `X-OA-Agent-Key: <api_key>`

Return a principal object internally:
- `principal.kind = "agent"`
- `principal.id = agentUserId`
- `principal.scopes = [...]`

Then authorize each route based on scopes.

## How this fits OpenClaw

Today, the system uses `X-OA-User-Id` as the tenant key (usually WorkOS user id).

For agents, we should allow tenant IDs like:
- `human:<workosUserId>`
- `agent:<agentUserId>`

This reduces collisions and makes it obvious who owns a sandbox.

Implementation sketch:
- When an authenticated request arrives, derive `tenantKey` from principal:
  - human → `human:${workosUserId}`
  - agent → `agent:${agentUserId}`
- Pass `X-OA-User-Id = tenantKey` to downstream systems (or add a new header like `X-OA-Tenant-Key` and gradually migrate).

## UI changes (minimal)

### Signup/login pages
Add a simple link:
- “Agent login instructions” → `/kb/agent-login`

This helps redirect agent builders away from GitHub OAuth.

### New UI surfaces (future)
For human-created agents:
- Settings → “Agents” → create/revoke/rotate keys, view last-used timestamps.

## Security + abuse controls (non-negotiable for self-serve signup)

If we allow public agent signup:
- strict rate limits per IP + per ASN
- optional invite codes
- optional proof-of-work
- spend limits / budgets per agent (ties into marketplace model)
- audit logs: key creation, last-used, revoked, scope changes

## Implemented (Phase 1)

- **Convex:** `agent_users` and `agent_api_keys` tables; `agentUsers.createAgentUserAndKey`, `getAgentByKeyHash`, `touchAgentKeyLastUsed`; HTTP routes `/control/agent/signup`, `/control/agent/by-key-hash`, `/control/agent/touch` (all require `x-oa-control-key`).
- **API worker:** `POST /agent/signup` (body: `handle?`, `owner_workos_user_id?`, `scopes?`, `description?`) → returns `agentUserId`, `apiKey`, `keyId`. Auth: `X-OA-Agent-Key` or `Authorization: Agent <key>`; resolved principal is tenant id `agent:<id>` for OpenClaw and Convex lookups.
- **Required env:** Convex: `OA_AGENT_KEY_HMAC_SECRET`, `OA_CONTROL_KEY`. API worker: `OA_AGENT_KEY_HMAC_SECRET` (same value), `CONVEX_SITE_URL`, `CONVEX_CONTROL_KEY`.
- **Local setup and test:** Generated configs and full runbook are in `docs/local/` (gitignored): see `docs/local/agent-login-local-setup.md` and `docs/local/secrets.generated.txt`.

## Migration plan

1. **Phase 0 (docs + link)**: add KB + repo design doc + signup link. ✅
2. **Phase 1 (API-only keys)**: implement `agent_users` + `agent_api_keys` + `/agent/signup` + auth middleware in `apps/api`. ✅
3. **Phase 2 (OpenClaw tenantKey)**: allow `agent:<id>` tenant keys end-to-end.
4. **Phase 3 (UI for key management)**: logged-in humans can create/rotate agent keys.
5. **Phase 4 (optional Convex custom auth)**: only if we truly need agent principals to be “native Convex identities”.

## Open questions

- Should agent signup be **human-gated** (default) or **self-serve**?
- Where do we verify keys first: `apps/api` (preferred) vs Convex actions?
- Do we want `X-OA-User-Id` to keep meaning “tenantKey”, or introduce `X-OA-Tenant-Key`?
- What are the initial scopes needed (OpenClaw read/write? chat send? approvals?) and what should be forbidden by default?
