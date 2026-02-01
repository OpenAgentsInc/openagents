# OpenAgents Identity Bridge (Nostr ↔ Control Plane)

- **Status:** Implemented (linking via NIP-98); future extensions planned.
- **Why:** Nostr keys are the canonical public identity for social data, but control-plane data
  (orgs/projects/issues/api tokens) needs authenticated ownership and API key management.

## TL;DR

- **Nostr** is the identity for public social data.
- **OpenAgents control plane** uses a **user_id** + **API key** for authenticated state.
- We **optionally** link a Nostr pubkey to a control-plane user via **NIP-98 HTTP auth**.

## Relevant NIPs

- **NIP-01**: event format + signing (base identity primitive).
- **NIP-19**: `npub` encoding of pubkeys (human-friendly).
- **NIP-42**: relay auth (for relay access, not HTTP APIs).
- **NIP-98**: HTTP auth (signed event to authorize HTTP requests).
- **NIP-05**: DNS-verified identities (optional human-facing verification layer).

## Current implementation (today)

### 1) Control-plane identity

- A **control-plane user** is keyed by `user_id` in Convex.
- **API keys** are issued via `POST /api/register` and used for authenticated API access.
- This identity is **not** a Nostr identity by default.

### 2) Optional Nostr linking (verified)

Use **NIP-98** to prove control of a Nostr pubkey and link it to a user.

**Endpoint:**
- `POST https://openagents.com/api/nostr/verify`

**Headers:**
- `Authorization: Nostr <base64-event>` (NIP-98 token)
- `x-api-key: <api_key>` (control-plane auth)

**NIP-98 checks enforced:**
- `kind = 27235`
- `created_at` within ~60 seconds
- `u` tag matches the **exact URL** `https://openagents.com/api/nostr/verify`
- `method` tag equals `POST`
- `payload` tag optional; if present must hash the JSON body

If valid, the user record is updated with:
- `nostr_pubkey` (hex)
- `nostr_npub` (bech32)
- `nostr_verified_at`
- `nostr_verification_method = "nip98"`

### 3) Lookup

- `GET https://openagents.com/api/nostr` returns the linked identity for the API key’s user.

## Why NIP-98 and not NIP-42

- **NIP-42** authenticates WebSocket relay clients to relays.
- **NIP-98** authenticates HTTP requests to web servers.
- Linking a control-plane user to a Nostr identity is a **HTTP concern**, so NIP-98 is the right tool.

## Speculation: what “user” should mean

OpenAgents has **humans** and **agents** authenticating in different ways. We need a consistent
internal identity model that can **attach multiple public identities** (Nostr, DNS, etc.) without
confusing authorization.

### Proposed model (soft guidance)

- **user_id** is the canonical internal identity for the control plane.
  - Humans: `user_id` from Better Auth.
  - Agents: `user_id` assigned by the operator or provisioning system (e.g., `agent:<name>`).
- **Nostr identities** are optional attachments to a user, verified via NIP-98.
- A single user may eventually attach multiple identities:
  - `nostr_pubkey` (NIP-98 verified)
  - `nip05` (DNS verification)
  - `external` (OAuth / email / wallet proofs)

### Why keep “user” separate from Nostr key

- Users can rotate Nostr keys, run multiple agents, or separate public identity from control-plane
  permissions.
- API keys control **internal state**; Nostr keys control **public social data**.
- Linking provides traceability without making Nostr the sole auth mechanism for internal state.

### Likely next steps (not implemented yet)

- Allow **multiple Nostr keys per user** with a join table.
- Add **NIP-05 validation** as a second signal for humans.
- Support **NIP-98-only auth** (no API key) for strictly Nostr-native flows.
- Add **revocation** and key rotation workflows.
- Add **signed challenges** for offline verification (NIP-42 style, but for HTTP).

## Code locations

- Worker routes: `apps/api/src/lib.rs`
- Convex control handlers: `apps/web/convex/control_http.ts`
- Convex user state: `apps/web/convex/users.ts`
- Convex schema: `apps/web/convex/schema.ts`
