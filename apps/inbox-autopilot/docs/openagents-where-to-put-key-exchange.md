# Where to Put OAuth / Key Exchange in OpenAgents

This doc looks at the [OpenAgents](https://github.com/OpenAgentsInc/openagents) monorepo (~/code/openagents) and recommends where to implement **OAuth key exchange** (e.g. Google/Gmail: redirect → callback with `?code=` → exchange for tokens → store). It applies to any flow where a user authorizes a third-party (Google, etc.) and your backend must securely store and later use refresh/access tokens.

---

## Short answer

**Put key exchange in the Laravel app (`apps/openagents.com`).** Use the existing **integrations** surface: same auth (session/Sanctum), same `UserIntegration` model and encrypted storage, same settings/integrations UI pattern, and the same internal API the runtime already uses to fetch secrets. A separate “Google service” is possible but adds complexity without a clear win unless you need a different security or scaling boundary.

---

## What exists today

### 1. Laravel app (`apps/openagents.com`)

- **Auth:** Email-code (magic link) login; WorkOS session validated on settings. Sanctum for API.
- **Integrations:** User-scoped third-party credentials.
  - **Model:** `UserIntegration` — `user_id`, `provider` (e.g. `resend`), `status`, `encrypted_secret`, `secret_fingerprint`, `secret_last4`, `metadata`, `connected_at`, `disconnected_at`. Unique on `(user_id, provider)`.
  - **Lifecycle:** `IntegrationSecretLifecycleService` — `upsertResend()`, `revokeResend()`, audit. Resend: user pastes API key → encrypted and stored.
  - **Settings UI:** `settings/integrations` — IntegrationController, Inertia page; connect/disconnect/test Resend.
  - **Runtime secret fetch:** `POST /api/internal/runtime/integrations/secrets/fetch` (middleware: `runtime.internal` — HMAC-signed, nonce + body hash). Request: `user_id`, `provider`, `integration_id`, `run_id`, `tool_call_id`. Response: `secret` (decrypted), `scope`, `cache_ttl_ms`. **Allowed `provider` today: `resend` only** (validated in `RuntimeSecretController`).
- **Khala:** Laravel is the auth authority; `KhalaTokenController` mints Khala JWTs for the authenticated user (no third-party OAuth).

### 2. Other apps in the monorepo

- **runtime** (Elixir): Agent execution, tools, skills. Calls Laravel’s internal secret-fetch API when a tool needs a secret; `secrets_ref` in manifests points to `provider: laravel`, `key_id: intsec_*`. It does **not** do OAuth or token storage.
- **autopilot-desktop**, **mobile**, **lightning-ops**, **lightning-wallet-executor**: Clients or specialized services; none own “user integrations” or “OAuth callback for web users.”

So: **Laravel is already the place that owns user identity, session, integrations, and the runtime’s source of secrets.**

---

## What “key exchange” needs

For OAuth (e.g. Google):

1. **Redirect:** User clicks “Connect Gmail” → backend (or frontend with backend-supplied URL) sends user to Google consent URL (`client_id`, `redirect_uri`, `scope`, `state`).
2. **Callback:** Google redirects to your `redirect_uri` with `?code=...&state=...`. **Only a backend** should exchange `code` for tokens (client secret must not be in the browser).
3. **Exchange:** Backend calls Google token endpoint with `code`, `client_id`, `client_secret`, `redirect_uri` → receives `access_token`, `refresh_token` (optional but needed for long-lived access).
4. **Storage:** Store refresh token (and optionally access token) securely, associated with the **current user**. Use it later to get access tokens for Gmail API (watch, read, send).

So the component that handles the **callback and exchange** must:

- Be a **server** that holds the client secret.
- Know **which user** is connecting (e.g. from `state` that encodes a signed user/session reference, or from session cookie if callback is a normal browser redirect to your app).
- **Persist** the tokens in a user-scoped, encrypted store that the runtime (or other backend) can read when it needs to call Gmail.

---

## Option A: Laravel (openagents.com) — recommended

**Where:** `apps/openagents.com`

**Where exactly:**

- **Routes:** Add under the same auth/settings surface as Resend, e.g.:
  - `GET /settings/integrations/google/redirect` — build Google auth URL, put signed `state` (e.g. user id or session id), redirect user to Google.
  - `GET /settings/integrations/google/callback` — validate `state`, exchange `code` for tokens, store in `UserIntegration` (provider `google` or `gmail`), redirect back to `settings/integrations` with success/error.
- **Controller:** Either extend `IntegrationController` (e.g. `redirectGoogle()`, `callbackGoogle()`) or add a small `GoogleOAuthController` (or `GmailConnectController`) that only does redirect + callback; then call into `IntegrationSecretLifecycleService` to upsert the integration.
- **Lifecycle:** Add `IntegrationSecretLifecycleService::upsertGoogle()` (or `upsertGmail()`) that:
  - Takes the user and the OAuth token response (refresh_token, access_token, expires_at).
  - Serializes what’s needed for Gmail (e.g. JSON with refresh_token and maybe client_id if you ever need it) and stores it in `UserIntegration` with `provider = 'google'` (or `gmail`), `integration_id` e.g. `gmail.primary`, `status = 'active'`, `encrypted_secret` = encrypted JSON, `connected_at` = now, etc.
- **Config:** Store Google OAuth client id and client secret in Laravel config/env (e.g. `config('services.google.client_id')`), never in the frontend.
- **Runtime:** In `RuntimeSecretController::fetch()`, allow `provider` in `['resend', 'google']` (or `gmail`). Runtime (or tools) then request secrets with `provider: 'google'`, `integration_id: 'gmail.primary'` and receive the decrypted token payload to call Gmail API (or a small token-service that uses the refresh token to return an access token).

**Why this fits:**

- User is already authenticated in Laravel (session or Sanctum); callback is a normal GET to your domain, so session cookie identifies the user (or you encode a signed user ref in `state` and re-associate in callback).
- Same storage and security model as Resend: one row per (user, provider), encrypted at rest, audit trail via `UserIntegrationAudit`.
- Settings/integrations UI already exists; “Connect Gmail” is another card and a link to `settings/integrations/google/redirect`.
- Runtime already fetches secrets from Laravel; you only add a provider and optionally a small contract for “what the decrypted payload looks like” (e.g. `refresh_token` + `access_token` + `expires_at` or a single refresh_token).

**Possible schema detail:** `UserIntegration` today has a single `encrypted_secret` (string). For OAuth you can store a JSON blob there, e.g. `{"refresh_token":"...","access_token":"...","expires_at":...}` and decrypt in the same way; or add a `secret_kind` column later if you want to distinguish “api_key” vs “oauth_tokens”. Not required for an initial version.

---

## Option B: Dedicated Google (or “integrations”) service

**What:** A separate service (new app in the monorepo or separate repo) that:

- Exposes “redirect” and “callback” endpoints.
- Holds the Google client id/secret.
- Exchanges the code and stores tokens in its own DB or in a shared store.
- Exposes an API for “get token for user X” that Laravel or the runtime calls.

**Pros:**

- Isolates OAuth and client secret in one service; Laravel never sees the raw tokens if you design it that way.
- Could, in theory, serve multiple clients (web, mobile, desktop) with one token store.

**Cons:**

- **Identity:** The callback must know “which OpenAgents user” is connecting. That usually means the redirect URL (or `state`) is generated by Laravel (or another app that knows the user) and includes a signed token or opaque id that the Google service can send back to Laravel to resolve user_id, or the Google service must have its own user table and a linking step. So you still have a tight coupling to Laravel (or a shared identity store).
- **Secret consumption:** Today the **runtime** fetches secrets from **Laravel**. If tokens live in a “Google service,” either (1) the runtime fetches from that service (new client, new auth, new config in runtime and possibly in Laravel for “where to get Google tokens”) or (2) the Google service pushes/syncs tokens to Laravel (duplication and sync complexity) or (3) Laravel proxies secret fetch to the Google service (Laravel still in the path; you’ve added a network hop and another service to run).
- **Operational cost:** Another service to deploy, monitor, and secure. For “we need Gmail tokens per user and the runtime already gets secrets from Laravel,” the benefit is small unless you have a strong requirement (e.g. strict isolation of Google credentials from the main app).

**Verdict:** Only consider a dedicated service if you have a clear requirement (compliance, multi-tenant isolation, or multiple independent consumers that can’t go through Laravel). For “OpenAgents web app + runtime need Gmail,” Laravel is the simpler and consistent place.

---

## Option C: Key exchange in runtime (Elixir)

**What:** Runtime exposes “redirect” and “callback” and stores tokens itself (or in a DB it shares with Laravel).

**Why not:**

- Runtime today does **not** own user identity or session. It receives `user_id` (and run context) when it calls Laravel for secrets; it doesn’t implement login or session. So the OAuth callback would have to identify the user by something (e.g. a token in `state`) that Laravel issued — and then you’re still doing the “who is this user?” dance and likely storing the result in Laravel anyway so that the existing secret-fetch path works. That leaves the runtime doing OAuth for no real gain and duplicates “where do we store user-scoped secrets?” in two places.
- Runtime is optimized for agent execution and tool calls; adding HTTP redirect/callback and user association is a poor fit.

**Verdict:** Don’t put key exchange in the runtime.

---

## Recommendation summary

| Concern | Recommendation |
|--------|-----------------|
| **Where to implement redirect + callback + code exchange** | Laravel (`apps/openagents.com`). |
| **Where to store refresh/access tokens** | Same as Resend: `UserIntegration` (provider `google` or `gmail`), `encrypted_secret` (e.g. JSON with refresh_token). |
| **Where to put client id/secret** | Laravel config/env only (e.g. `config('services.google')`). |
| **Settings UI** | Same integrations page; add “Connect Gmail” that hits the new redirect route. |
| **Runtime / tools needing tokens** | No change to architecture: runtime keeps calling Laravel’s internal secret-fetch API; add `google` (or `gmail`) to the allowed `provider` list and return the decrypted token payload (or a minimal token object). |
| **Dedicated “Google service”** | Only if you have a concrete need to isolate OAuth or serve non-Laravel clients; otherwise adds complexity. |

**Concrete steps in Laravel:**

1. Add `config/services.google` (client_id, client_secret, redirect_uri).
2. Add routes: e.g. `GET settings/integrations/google/redirect` and `GET settings/integrations/google/callback`, both under auth + WorkOS session middleware.
3. Implement redirect (build Google URL, set signed `state` with user/session ref) and callback (validate state, exchange code, call `IntegrationSecretLifecycleService::upsertGoogle()`).
4. Extend `IntegrationSecretLifecycleService` to upsert a `UserIntegration` with provider `google` (or `gmail`), storing encrypted JSON of refresh_token (and optionally access_token/expires_at).
5. In `RuntimeSecretController::fetch()`, allow `provider` in `['resend', 'google']`; return the decrypted secret (same shape as today: string payload; runtime/tools can parse JSON if needed).
6. Optionally add a `disconnectGoogle()` lifecycle method and a “Disconnect” button on the integrations page.

This keeps key exchange, secret storage, and secret consumption in one place (Laravel) and reuses the existing integration and runtime-secret pattern.
