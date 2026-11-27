# WorkOS authentication and a Coax SDK

Status: **decided, 2026-11-26** (master tracking: [#14103](https://github.com/OpenAgentsInc/openagents/issues/14103);
updated decisions in [decisions.md](decisions.md)). Part of [OpenAgents authentication](README.md).
Reference clones live outside this repo at `~/work/projects/repos/workos/`.

This document specifies how OpenAgents adopts [WorkOS](https://github.com/workos) for web,
mobile (iOS, Android), desktop (macOS, Linux), and CLI authentication using our own
**Custom Coax UI** and an in-repo **Coax WorkOS SDK** (`coax/lib/workos/` +
`tools/oagen-coax/`). It audits the current `openagents` codebase, examines 18 cloned
WorkOS repositories, reconciles WorkOS against [decisions.md](decisions.md), and records
the locked architecture and phased build plan (`#14097`–`#14103`).

## Locked architecture (decided 2026-11-26)

Every design choice below was locked on 2026-11-26:

| Decision | Locked choice |
| --- | --- |
| **Identity spine** | **Hybrid `acct_` spine (Option A):** WorkOS handles human login, social OAuth, Enterprise SSO, and Optional Organizations (`user_01...`, mapped via `external_id = acct_...` and `auth.workos_identities`). Our opaque `acct_...` in Cloud SQL Postgres stays canonical for all product tables, Nostr-only accounts (D8), self-custodial wallets & NIP-VAULT backups (D4–D5), Lightning Addresses (D6), and sat-capped agent grants (`#13211`). |
| **Custom Coax UI & methods** | **Custom Coax UI everywhere** (no Hosted AuthKit login redirect): **Email Magic Auth** (6-digit code sent by WorkOS, entered inline), **Email + Password** (login, registration, and password reset mediated through WorkOS so we store zero password hashes locally — updated 2026-11-26 per owner decision to retain existing email+password support), **Google / GitHub / Apple OAuth** (satisfies iOS Guideline 4.8 natively; reverses D1), **Enterprise SSO** (domain detection via `GET /connections?domain=...`), **Nostr**, and **first-party Coax WebAuthn Passkeys** (`passkeys.c` + `passkey-verify.coax`, since WorkOS has no headless passkey API and this preserves NIP-VAULT PRF). **No phone/SMS (D3 stays).** |
| **Sessions & tokens** | **Option A1 — OpenAgents opaque sessions everywhere:** `oa-api` mediates WorkOS logins and mints `__Host-oa_session` (`oas_`) on web and device-bound `oa_at_`/`oa_rt_` (`OA-Device-Proof`) on iOS, macOS, and Android. Instant revocation, zero JWT revocation lag, one unified `auth_resolve` middleware. |
| **Linking & assurance** | **Auto-link only on `email_verified=true`** (with audit row + multi-channel security notice; never auto-link unverified emails). All WorkOS logins enter at **`L1`** — wallet backups, spend limits, key removal, and agent grants still require our own Passkey/Nostr `L2`/`L2-fresh` step-up. |
| **Native OAuth/SSO & CLI** | **Native PKCE + `oa-api` exchange:** iOS & macOS use `ASWebAuthenticationSession` (`openagents://auth/callback`, non-ephemeral); Android uses Custom Tabs + verified App Link. Apps verify `state` in Coax `flow.coax` and POST `(code, code_verifier, redirect_uri)` + `OA-Device-Key` to `oa-api`, which exchanges with WorkOS (`client_secret` + `code_verifier`), verifies the RS256 JWT via Coax JWKS + token-confusion guard, and mints `oa_at_`/`oa_rt_`. CLI uses WorkOS **RFC 8628 device flow** (or NIP-98 key). |
| **Organizations** | **Optional Organizations:** `org_id = null` for individual users; Custom Coax UI shows an org picker only on `403 organization_selection_required` or B2B workspace switching. |
| **Data mirroring & backups** | **Pure Coax/C Events API poller (`GET /events`) + verified webhook handler inside `oa-api`** (zero Node sidecar), mirroring WorkOS data into a `workos` Postgres schema (`_raw jsonb`, generated columns, tombstones, `*_live` views — `sync-engine` pattern), covered by our 6-hourly `age`-encrypted locked dumps. |
| **Radar, Actions & lifecycle** | Pass Radar context (`ip_address`, `user_agent`, `radar_auth_attempt_id`); propagate freeze/logout-everywhere (`POST /user_management/sessions/revoke`) and 30-day `auth.purge_due()` deletes (`DELETE /user_management/users/{id}`); wire synchronous **AuthKit Actions** (`authentication` + `user_registration` HMAC verify ±30s & signed verdicts) in v1 so WorkOS blocks frozen accounts and enforces invite gates before creating users. |
| **Coax SDK & `oagen` emitter** | **In-repo `oagen` Coax emitter (`tools/oagen-coax/`)** generating checked-in Sans-I/O Coax files into `coax/lib/workos/gen/` covering all **269 OpenAPI endpoints** (zero Node in normal builds). Generated models use **borrowed `Jv` views + typed `*_get_<field>` presence getters + array/list iterators + open enums + direct-to-buffer request encoders** (matching `coax/proto.md`). Hand-written `#authority` core helpers live alongside in `coax/lib/workos/`, mutation-tested (zero survivors) and verified offline against `workos/emulate`. |
| **Rollout** | Cut over `openagents.com` (web, iOS, Android, macOS desktop, CLI) behind `OA_WORKOS_ENABLED` first; leave `gamebuilder.org` on existing first-party Resend email codes until Phase 5 (`#14102`). |

## Executive summary

1. **Coax already has almost every cryptographic primitive a WorkOS SDK needs.**
   `coax/lib/crypto/` and `coax/lib/encoding/` already ship SHA-256, HMAC-SHA256,
   AES-256-GCM, XChaCha20-Poly1305, PBKDF2-HMAC-SHA256, HKDF-SHA256, standard base64,
   unpadded base64url decode, and **2048–4096-bit RSA PKCS#1 v1.5 SHA-256 verification with
   fixed exponent `e = 65537`** (`rsa_verify.coax`) — an exact match for WorkOS JWKS
   (`kty: "RSA"`, `alg: "RS256"`, `e: "AQAB"`). Only ~200–300 lines of pure Coax helpers
   (`base64url_encode`, RFC 3986 percent-encoding, strict JWT/JWKS validation) are missing.
2. **Build the Coax SDK Sans-I/O (`#authority`, effect-free) with thin platform adapters.**
   `coax/lib/net/http.coax` works on macOS/Linux tools, but iOS TLS is unqualified in
   `coax/lib/net/README.md`, Android uses JNI `oa_https` (`apps/shared/host/android/app-login.c`),
   and browsers use `fetch` (`coax/web/http-requests.js`). Following OpenAgents' established
   pattern in `apps/shared/app-login-flow.coax` and `coax/web/api/*-policy.coax`, all
   protocol state machines, URL builders, JSON codecs, JWT/JWKS verification, PKCE, session
   sealing, retry policy, and webhook/action verification should live in pure `#authority`
   Coax (`coax/lib/workos/`) compiled to C and Wasm, with thin I/O adapters per platform.
3. **Hand-write the core Auth SDK (~20 endpoints + helpers) instead of starting with an
   `oagen` Coax emitter.** WorkOS's OpenAPI spec has 269 endpoints (47,513 YAML lines) and
   its official SDKs are generated via `oagen` + `oagen-emitters` (2,450–4,480 LOC TS per
   language emitter). Crucially, in every official SDK (`workos-rust`, `workos-ios`,
   `workos-android`, `workos-go`, `workos-node`), **every security-critical auth component
   is hand-written (`@oagen-ignore-file`)**: PKCE, `PublicClient`, JWKS + token-confusion
   checks, session sealing, webhooks, AuthKit Actions, retries, and error normalization.
   Hand-writing the ~20 endpoints we actually need keeps Coax's zero-heap, fixed-buffer
   discipline intact; an `oagen` emitter can wait until we need broad enterprise CRUD.
4. **Recommended integration architecture: Hybrid `acct_` Spine (Option A).** Keep our
   opaque `acct_...` in Cloud SQL Postgres as the canonical internal identity for Nostr keys,
   self-custodial wallets, Lightning Addresses, agent grants, and app data. Treat WorkOS as
   the federated human auth/enterprise-SSO/org-RBAC engine, linked via `external_id = acct_...`
   and mirrored locally with `workos/sync-engine` (so our locked backups and SQL joins keep
   working). This avoids locking Nostr-only accounts, Bitcoin wallets, or agent spend caps
   into WorkOS, while letting us adopt WorkOS AuthKit cleanly across web, mobile, and desktop.
5. **High-value pitfalls caught in WorkOS's own source code:**
   - **Public clients must omit `client_secret` from JSON** on `/user_management/authenticate`
     (WorkOS rejects `"client_secret": ""` with HTTP 422 — explicitly worked around in
     `workos-android/PublicClient.kt` and `workos-ios/PublicClient.swift`).
   - **JWT token-confusion guard is mandatory:** WorkOS signs user access tokens, widget
     tokens, and agent tokens with the same environment JWKS key. Human session verification
     must require `sid` non-empty, `auth_time` present, `sub_profile` absent or `"user"`,
     and `sub_type` absent (`workos-rust/src/helpers/session.rs`).
   - **Refresh token rotation + single-flight + transient-error preservation:** WorkOS rotates
     refresh tokens on each refresh. Concurrent refreshes must be coalesced, and `408`/`429`/
     `5xx`/offline errors must *never* discard the stored refresh token (`authkit-js`).
   - **Apple App Store Guideline 4.8:** Enabling Google, GitHub, or other social login inside
     WorkOS AuthKit for iOS requires shipping **Sign in with Apple** in the same release
     (WorkOS supports `AppleOAuth` natively, or mobile can stay email/passkey/SSO-only).
   - **Offline E2E testing is solved:** `workos/emulate` implements 211/269 OpenAPI endpoints
     locally in memory with real JWKS, seeded users/orgs, signed webhooks, and error hooks —
     allowing full Coax SDK integration tests with zero network or WorkOS account.

---

## 1. Current state of OpenAgents auth and Coax

Any WorkOS adoption is **not greenfield anymore**: since the 2026-11-11 design in
[README.md](README.md) and [build-order.md](build-order.md), substantial first-party auth
code is already built and running across our backend and native apps:

| Surface | What is built today | Key files in `openagents` |
| --- | --- | --- |
| Backend API (`oa-api`) | Opaque sessions (`__Host-oa_session`, `oa_at_`, `oa_rt_`), email 6-digit code + same-browser link (Resend), Nostr NIP-98/07/46 + phone QR, WebAuthn passkeys (`#13215`), P-256 device-bound refresh `OA-Device-Proof` (`#13216`), agent accounts & `oa_key_` grants (`#13211`), 72h delayed recovery & freeze/merge/export/delete (`#13214`), multi-site origin allowlist (`OA_API_SITE_ORIGINS` for `gamebuilder.org`), push security notices (`#13233`) | `coax/web/api/auth.c`, `auth-policy.coax`, `auth-seal.coax`, `auth-wire.coax`, `passkeys.c`, `passkey-policy.coax`, `passkey-verify.coax`, `auth-device.c`, `auth-device-policy.coax`, `agents.c`, `agent-policy.coax`, `auth-recover.c`, `auth-life.c` |
| Web (`openagents-web`) | Custom GPU-rendered Coax `/login` and `/login/email` compiled to Wasm (`wasm32-wasip1`), browser `key.js` for NIP-07/WebAuthn, `host.js` + `http-requests.js` | `apps/openagents-web/login.coax`, `apps/openagents-web/login-face.c`, `coax/web/http-requests.js` |
| Shared App Login | Pure effect-free Coax login state machine and GPU form card | `apps/shared/app-login.coax`, `apps/shared/app-login-flow.coax`, `apps/shared/app/login/{flow,service,card,bridge}.coax`, `apps/shared/login/screen.coax` |
| iOS & macOS (`openagents-ios`, `openagents-desktop`) | Shared Swift `AppAuth` + Secure Enclave `DeviceKeys` + `KitPasskey`. iOS stores tokens in Keychain (`AfterFirstUnlockThisDeviceOnly`); macOS stores tokens and enclave blob in `0600` files under `~/.openagents/desktop/` to avoid repeated Keychain prompts on rebuilt dev binaries | `apps/shared/host/apple/AppAuth.swift`, `apps/shared/host/apple/DeviceKey.swift`, `apps/shared/native/NativeLogin.swift`, `apps/openagents-desktop/Account.swift` |
| Android | JNI C adapter driving `app-login-flow.coax`, `oa_https`, Keystore AES-256-GCM encrypted `0600` files, Hardware Keystore P-256 device key | `apps/shared/host/android/app-login.c`, `apps/shared/host/android/app-login-unit.coax`, `apps/shared/host/android/device-key.c` |
| Backups & DB Safety | Postgres 18 roles (`oa_api` has no DDL/DROP), DDL/TRUNCATE event triggers, 6-hourly `auth` + nightly full `age`-encrypted dumps to locked GCS bucket, monthly automated restore drill (`#13194`) | `coax/web/api/db/`, `coax/web/api/backup/backup.coax`, [data-and-backups.md](data-and-backups.md) |
| Reference prior art (`zeron`) | Zeron's Cloudflare edge verifies WorkOS AuthKit JWTs via remote JWKS (`jose`) while its iOS app uses system web auth sheet + org picker | `docs/zeron/README.md`, `docs/zeron/architecture.md`, `docs/audits/2026-11-20-mobile-ui-gap-analysis-zeron.md` |

The previous WorkOS integration (`docs/auth/2026-10-20-workos-source-of-truth.md` in git
history) was removed in `#13207` (`migrations/0161_auth_drop_old_login.sql`), and decision
[D7](decisions.md#d7-retire-workos) marked the old WorkOS environments and five Secret
Manager secrets for deletion. Re-adopting WorkOS means explicitly updating the relevant
owner decisions rather than silently drifting from [decisions.md](decisions.md).

---

## 2. Survey of cloned WorkOS repositories

We inspected all 72 active repositories in `https://github.com/workos` and shallow-cloned
the 18 directly relevant ones into `~/work/projects/repos/workos/`:

| Repo | Language | Purpose & key takeaways for OpenAgents |
| --- | --- | --- |
| `openapi-spec` | YAML / TS | Authoritative OpenAPI 3.x spec (`spec/open-api-spec.yaml`, 1.72 MB, 47,513 lines, **269 endpoints**) plus `oagen.config.ts` and generation policy (`src/policy/`). |
| `oagen` | TypeScript | WorkOS's OpenAPI-to-IR SDK generator framework (`ApiSpec` IR → `GeneratedFile[]`, spec diffing, compat checks). |
| `oagen-emitters` | TypeScript | Official language emitters (`rust`, `go`, `ios`, `kotlin`, `node`, `python`, `ruby`, `php`, `dotnet`). Shows the exact boundary between generated CRUD resources and hand-written helpers. |
| `workos-rust` | Rust | Closest systems-language reference to Coax. Hand-written `src/helpers/`: `pkce.rs`, `authkit.rs`, `public_client.rs`, `jwks.rs`, `session.rs`, `webhook_verification.rs`, `actions.rs`, plus `client.rs` (retry/backoff/path validation) and `error.rs`. |
| `workos-node` | TypeScript | Most mature canonical SDK (`@workos-inc/node` v8–12+); covers User Management, sealed sessions (`iron-webcrypto`), AuthKit, webhooks, SSO, Pipes, Vault, Events, Actions. |
| `workos-go` | Go | Clean standard-library HTTP client, pagination, webhook verification, and User Management patterns. |
| `workos-ios` | Swift | Official Apple SDK (iOS 16+ / macOS 13+). `PublicClient` (PKCE, no API key), `SessionSealing` (`wos2.` PBKDF2-SHA256 600k + HKDF-SHA256 + AES-256-GCM), `SessionTokenVerification`, `JWKSHelpers`, device auth. |
| `workos-android` | Kotlin | Official Android SDK (API 26+). `PublicClient` (PKCE, no API key) — explicitly documents that sending an empty `client_secret` triggers HTTP 422 on `/user_management/authenticate`. |
| `authkit-session` | TypeScript | Framework-agnostic server-side AuthKit session toolkit: `AuthKitCore` (JWKS verify, `iss` check, single-flight refresh with 429 retry, PKCE state cookie check), `ironWebcryptoEncryption` (`aes-256-cbc` + `hmac-sha256` `~2` seals). |
| `authkit-js` | TypeScript | Browser/SPA public-client SDK: PKCE, `useCookie` (`credentials: "include"`) vs memory/storage refresh tokens, cross-tab locking, transient (`408`/`429`/`5xx`) vs terminal refresh error handling. |
| `authkit-electron` | TypeScript | Official desktop public-client SDK: custom-scheme deep links (`scheme://callback`), single-instance lock for Windows/Linux, `safeStorage` persistence, main-process refresh token isolation, `system-browser` vs `window` ceremony. |
| `workos-custom-ui-authkit-example` | TypeScript | Complete headless/custom-UI login backend (Hono + `@workos-inc/node`) with no Hosted AuthKit redirect: Magic Auth (`createMagicAuth` + `authenticateWithMagicAuth`), domain SSO check, Google OAuth, multi-org `organization_selection_required`, impersonation initiate route, sealed cookies, CSRF. |
| `electron-authkit-example` | TypeScript | End-to-end desktop deep-link OAuth callback reference. |
| `expo-authkit-example` | TypeScript | End-to-end mobile OAuth PKCE callback reference. |
| `emulate` | TypeScript | Local in-memory WorkOS API emulator implementing **211 of 269** OpenAPI endpoints (including AuthKit, User Management, JWKS, Organizations, RBAC, Webhooks, Events, Agent Auth) with seed files and error hooks. Zero network, zero WorkOS account. |
| `sync-engine` | TypeScript | Official Postgres/MySQL/MongoDB/ClickHouse mirror for WorkOS (`@workos/sync-postgres`): REST backfill + Events API cursor tail + reconciliation into `workos.*` tables with `_raw jsonb`, generated columns, tombstones, and `*_live` views. |
| `auth.md` | TypeScript | WorkOS's open reference protocol for **agentic registration**: discoverable `AUTH.md`, ID-JAG assertions (`draft-ietf-oauth-identity-assertion-authz-grant`), RFC 8628 claim ceremony, and RFC 7523 JWT-bearer token exchange. |
| `cli` | TypeScript | WorkOS CLI showing real-world RFC 8628 device authorization (`src/lib/device-auth.ts`) and auth status handling. |

---

## 3. WorkOS protocol and wire surface

### 3.1 Endpoints actually needed for authentication

While `openapi-spec/spec/open-api-spec.yaml` declares 269 endpoints across 25+ products
(SSO, Directory Sync, FGA, Audit Logs, Vault, Radar, Pipes, Widgets, Feature Flags, Agents),
end-to-end web, mobile, desktop, and CLI authentication needs only a **small, stable core
of ~18–22 endpoints** against `https://api.workos.com` (configurable `base_url` so tests
point at `http://127.0.0.1:4100` for `workos/emulate`):

| Group | Method & Path | Client Mode | Purpose |
| --- | --- | --- | --- |
| **AuthKit / OAuth** | `GET /user_management/authorize` | Browser URL (no HTTP call) | Start AuthKit, Google/GitHub/Apple OAuth, or Enterprise SSO redirect (`response_type=code`, PKCE `S256`, `state`, `provider`, `connection_id`, `organization_id`, `domain_hint`, `login_hint`, `screen_hint`) |
| | `POST /user_management/authenticate` | Public **or** Confidential | Core token endpoint (all grant types below) |
| | `POST /user_management/authorize/device` | Public | Start RFC 8628 device authorization flow (CLI / headless) |
| | `GET /user_management/sessions/logout` | Browser URL | End hosted AuthKit session (`session_id`, optional `return_to`) |
| | `POST /user_management/sessions/revoke` | Confidential | Server-side session revocation (`session_id`) |
| **JWKS** | `GET /sso/jwks/{client_id}` | Public (no auth header) | Fetch RS256 public keys for offline JWT access-token verification |
| **Custom UI (optional)** | `POST /user_management/magic_auth` | Confidential | Create & email 6-digit Magic Auth code (for headless `/login`) |
| | `GET /connections` | Confidential | Domain-based Enterprise SSO discovery (`?domain=...`) |
| **Users & Orgs** | `GET /user_management/users/{id}` | Confidential | Fetch user profile |
| | `GET /user_management/users/external_id/{external_id}` | Confidential | Look up WorkOS user by our `acct_...` id |
| | `PUT /user_management/users/{id}` | Confidential | Set `external_id`, `metadata`, `email_verified`, etc. |
| | `DELETE /user_management/users/{id}` | Confidential | Propagate account deletion (Apple 5.1.1(v) / Google Play) |
| | `GET /user_management/users/{id}/sessions` | Confidential | List active sessions for `/settings/sessions` |
| | `GET /user_management/users/{id}/identities` | Confidential | List linked OAuth/SSO identities |
| | `GET /user_management/organization_memberships` | Confidential | List org memberships & roles for a user |
| | `GET /organizations/{id}` | Confidential | Read org details |
| **Sync & Webhooks** | `GET /events` | Confidential | Cursor-based Events API polling (mirroring / revocation catch-up) |
| | Inbound webhook + AuthKit Action handlers | Server inbound | Verify `WorkOS-Signature` HMAC-SHA256; sign Action verdicts |
| **Agent Auth (optional)** | `/agents/blueprints`, `/agents/instances`, `/agents/sessions`, `/agents/credentials/validate` | Confidential | WorkOS's native agent identity/session APIs (see §5.5 & §7) |

### 3.2 `POST /user_management/authenticate` grant types

From `openapi-spec/spec/open-api-spec.yaml` (L14085–14760), one endpoint handles all
authentication exchanges, discriminated by `grant_type`:

| `grant_type` | Required fields | Confidential vs Public |
| --- | --- | --- |
| `authorization_code` | `client_id`, `code`, plus either `client_secret` (confidential) or `code_verifier` (PKCE public; both allowed on BFF) | Both — **public clients MUST omit `client_secret`** |
| `refresh_token` | `client_id`, `refresh_token`, optional `organization_id`, plus `client_secret` only for confidential clients | Both — **public clients MUST omit `client_secret`** |
| `urn:ietf:params:oauth:grant-type:device_code` | `client_id`, `device_code` | Public (poll on `authorization_pending`; add +5s on `slow_down`) |
| `urn:workos:oauth:grant-type:magic-auth:code` | `client_id`, `client_secret`, `email`, `code` | Confidential only (custom UI backend) |
| `urn:workos:oauth:grant-type:organization-selection` | `client_id`, `client_secret`, `pending_authentication_token`, `organization_id` | Confidential (multi-org follow-up after 403 `organization_selection_required`) |
| `password`, `email-verification:code`, `mfa-totp`, `email-completion`, `radar-email-challenge:code`, `radar-sms-challenge:code` | `client_id`, `client_secret`, plus grant-specific fields | Confidential only (custom UI; **`password` enabled** per 2026-11-26 owner update; `radar-sms` disabled to match D3) |

**Critical public-client trap (verified in `workos-android/PublicClient.kt`):**
Sending `"client_secret": ""` in a PKCE code exchange or public refresh request causes
WorkOS to return **HTTP 422**. Public-client serializers must omit the `client_secret`
key entirely rather than serializing an empty string.

### 3.3 Authentication headers, error shapes, pagination, and retries

Verified across `workos-rust/src/client.rs` and `workos-rust/src/error.rs`:

- **Auth header:** Confidential API calls send `Authorization: Bearer <WORKOS_API_KEY>`
  (for `/user_management/authenticate`, the API key is also sent as `client_secret` in the
  JSON body). Public clients send **no** `Authorization` header.
- **Headers:** `Content-Type: application/json`, `Accept: application/json`,
  `User-Agent: workos-coax/<version>`, optional `Idempotency-Key` (deduplicated server-side
  on `POST /audit_logs/events`).
- **Dual error JSON schema:** WorkOS returns two error shapes:
  - Management APIs: `{"code": "user_not_found", "message": "User not found", ...}`
  - OAuth / AuthKit endpoints: `{"error": "invalid_grant", "error_description": "...", ...}`
  - Multi-step auth errors (HTTP 403) also include `pending_authentication_token`,
    `organizations`, or `connection_ids` (for `organization_selection_required` and
    `sso_required`).
  - Always capture the `x-request-id` and `Retry-After` response headers.
- **Cursor pagination:** List endpoints accept `limit` (1–100), `before`, `after`, `order`
  (`asc`/`desc`), returning `{"object": "list", "data": [...], "list_metadata": {"before": ..., "after": ...}}`.
- **Safe retry policy:** Auto-retry only HTTP-safe methods (`GET`, `HEAD`, `OPTIONS`) or
  requests with `Idempotency-Key` on HTTP `429` or `5xx` (respecting integer `Retry-After`,
  else exponential backoff `100ms * 2^attempt` capped at `5000ms` with equal jitter). Never
  blindly auto-retry non-idempotent `POST /user_management/authenticate` with a single-use
  authorization code or rotated refresh token without explicit caller handling!
- **Path traversal guard:** Reject empty segments and `.`, `..`, `%2e`, `%2e%2e`
  (case-insensitive) in interpolated path IDs before constructing request URLs.

### 3.4 Cryptographic contracts

Every crypto contract below was read directly from WorkOS's reference implementations:

1. **PKCE (`S256` only, RFC 7636 — `workos-rust/src/helpers/pkce.rs`):**
   - `code_verifier`: 43..=128 characters (default 43 = 32 random bytes base64url no-pad).
   - `code_challenge`: `base64url_nopad(SHA-256(code_verifier))`, `code_challenge_method = "S256"`.
   - `state`: 32 random bytes base64url no-pad, compared in constant time on callback.
2. **Access Token JWT Verification (`workos-rust/src/helpers/session.rs` + `jwks.rs`, `authkit-session/src/core/AuthKitCore.ts`):**
   - Fetch `GET {base_url}/sso/jwks/{client_id}`, cache with 10-minute TTL, coalesce
     concurrent fetches (single-flight), and force-refresh once when encountering an unknown
     `kid` while preserving the last known-good cache on refresh failure.
   - Verify header: `alg == "RS256"`, `kid` non-empty.
   - Match **exactly one** JWK by `kid` (duplicate `kid` fails closed); require
     `kty == "RSA"`, `alg` absent or `"RS256"`, `use` absent or `"sig"`, `key_ops` absent
     or containing `"verify"`, and `e == "AQAB"` (`65537`).
   - Verify RS256 signature over `<b64url_header>.<b64url_payload>` using decoded modulus `n`.
   - Validate claims: `sub` non-empty, `exp` present (zero leeway; surface `needs_refresh`
     when expired *after* signature verification passes so callers refresh only genuinely
     signed tokens!), `nbf` (if present) `<= now`, `iss` (when configured).
   - **Mandatory token-confusion check:** Because WorkOS signs user tokens, widget tokens,
     and agent tokens with the same environment JWKS key, reject any JWT as a human session
     unless:
     - `sid` is non-empty,
     - `auth_time` is present,
     - `sub_profile` is absent or `"user"`, and
     - `sub_type` is absent.
   - **Cookie metadata binding:** If user/impersonator metadata is stored alongside the JWT,
     bind `cookie.user.id == claims.sub` and `cookie.impersonator == claims.impersonator`,
     and never use unverified cookie fields (like `email`) for authorization.
3. **Sealed Session Cookies (note cross-SDK format divergence!):**
   - `authkit-session` / `workos-node` use `iron-webcrypto` (`AES-256-CBC` + `HMAC-SHA256`,
     PBKDF2 1 iteration, `~2` suffix).
   - `workos-rust` uses `AES-256-GCM` (`nonce(12) || ciphertext || tag(16)`, standard base64,
     64-char hex 32-byte key).
   - `workos-ios` uses `wos2.` (`"wos2." + base64(salt(16) || nonce(12) || ct || tag(16))`,
     AAD `"wos2."`, master key via PBKDF2-HMAC-SHA256 600,000 iterations cached per process,
     per-seal key via HKDF-SHA256 over salt with info `"wos2.aes-256-gcm"`).
   - **Takeaway for Coax:** WorkOS itself does *not* require a single sealed-cookie wire
     format because cookies are only read by your own backend. Our Coax backend can use
     AES-256-GCM (matching `workos-rust` or `wos2.`) or our existing `auth-seal.coax`
     XChaCha20-Poly1305 format — **or** avoid storing WorkOS tokens in cookies altogether by
     keeping OpenAgents' opaque `__Host-oa_session` cookie and storing the encrypted WorkOS
     refresh token server-side in Postgres (see §6).
4. **Webhook Signature Verification (`WorkOS-Signature` — `workos-rust/src/helpers/webhook_verification.rs`):**
   - Header format: `t=<unix_ms>, v1=<hex_sig>`.
   - Reject empty headers, malformed pairs, or **any unknown key** (e.g. `v2=`) to prevent
     silent downgrade attacks.
   - Timestamp within tolerance (default ±180s for webhooks, ±30s for AuthKit Actions).
   - Compute `HMAC-SHA256(secret, "<timestamp>.<raw_body>")`, lowercase hex, constant-time
     comparison.
5. **AuthKit Actions (`workos-rust/src/helpers/actions.rs`):**
   - Inbound request verified identically to webhooks (±30s tolerance).
   - Outbound response is `{"object": "authentication_action_response" | "user_registration_action_response", "payload": {"timestamp": <ms>, "verdict": "Allow" | "Deny", "error_message"?: "..."}, "signature": "<hmac_hex>"}`,
     signed over `"<timestamp>.<JSON(payload)>"`.
   - Lets our Coax API enforce realm invite gates, email blocks, or account-freeze checks
     synchronously before WorkOS issues a token.

---

## 4. Coax readiness audit: what exists vs what to write

We audited `coax/lib/crypto/`, `coax/lib/encoding/`, `coax/lib/json/`, and `coax/lib/net/`
directly against WorkOS's requirements:

| Capability | Needed by WorkOS SDK | Status in `openagents` | Exact Coax module / gap |
| --- | --- | --- | --- |
| SHA-256 | PKCE `S256`, JWT signing input hash, token digests | **Ready** | `coax/lib/crypto/sha256.coax` (`sha256_hash_checked`) |
| HMAC-SHA256 | Webhook verify, AuthKit Actions sign/verify, email lookup | **Ready** | `coax/lib/crypto/hmac.coax` (`hmac_sha256_checked`) |
| RSA RS256 verify | WorkOS JWKS access-token JWT verification (`kty=RSA`, `e=65537`, 2048–4096-bit) | **Ready** | `coax/lib/crypto/rsa_verify.coax` (`rsa_verify_sha256` accepts 256..=512-byte moduli with fixed `e = 65537` — exact match for WorkOS JWKS!) |
| AES-256-GCM | Sealed sessions (`workos-rust` / `wos2.` compatible) & Vault local crypto | **Ready** | `coax/lib/crypto/aes_gcm.coax` (`aes256_gcm_seal`, `aes256_gcm_open`, zeroizes expanded key) |
| XChaCha20-Poly1305 | Encrypting WorkOS refresh tokens at rest in Postgres | **Ready** | `coax/lib/crypto/xchacha20_poly1305.coax`, `coax/web/api/auth-seal.coax` |
| PBKDF2-SHA256 & HKDF-SHA256 | `wos2.` master key + per-seal key derivation | **Ready** | `coax/lib/crypto/pbkdf2.coax`, `coax/lib/crypto/hkdf.coax` |
| Base64 & Base64url | Standard base64 (cookies), unpadded base64url (JWT, JWK `n`/`e`, PKCE) | **Mostly ready** | `coax/lib/encoding/base64.coax` (std encode/decode), `coax/lib/encoding/fi-base64url.coax` (canonical unpadded decode). **Missing:** ~40-line `base64url_encode` (no pad) |
| Hex & Constant-time eq & Zeroize | Webhook sigs, key parsing, secret scrubbing | **Ready** | `coax/lib/encoding/hex.coax`, `coax/lib/crypto/bytes.coax`, `coax/lib/crypto/zeroize.coax` |
| JSON parse & write | Request/response payloads, JWKS, JWT header/claims, webhooks | **Ready (with JWT caveat)** | `coax/lib/json/json.coax` (`jv_parse`, `jo_*`). **Caveat:** `jv_parse` does not reject duplicate object keys (`jv_get` returns first); our JWT header/claims reader must check key uniqueness to prevent duplicate-claim smuggling (`exp`/`sub`/`sid`/`alg`)! |
| URL parsing & RFC 3986 percent-encoding | Authorization URLs, logout URLs, query strings, path segments | **Partial** | `coax/lib/net/uri.coax` parses bounded `https://`/`http://`/`wss://` URLs (max 2048 bytes). **Missing:** ~80-line RFC 3986 percent-encoder + query builder + dot-segment path guard |
| HTTP/1.1 + TLS client | Calling `https://api.workos.com` | **Native macOS/Linux only** | `coax/lib/net/http.coax` works for CLI & macOS/Linux native (`Network.framework` / OpenSSL 3; `Connection: close`, 1 MiB body max, synchronous). **Gaps:** iOS TLS runtime unqualified (`coax/lib/net/README.md`), Android uses JNI `oa_https`, browsers use `fetch`, `coax/web/api` needs pooled/non-blocking or bounded timeout C transport |
| Random bytes / wall clock | PKCE verifier, `state`, GCM nonces, JWT `exp`/`nbf`, webhook skew | **Host effect** | Passed explicitly as parameters into pure `#authority` Coax functions (matching `auth-seal.coax` and `app-login-flow.coax`) |

### Crucial architectural lesson from the audit

Coax has **no heap, no globals, caller-owned bounded buffers, and strict effect isolation**
(`#authority`, `#test`, `#forall`, `coax mutate`). Meanwhile, HTTP transport differs across
every target we ship (`coax/web/api` C server, browser Wasm `fetch`, iOS/macOS `URLSession`,
Android JNI `oa_https`, CLI `http.coax`).

Therefore, **our WorkOS Coax SDK MUST be Sans-I/O (effect-free)** — exactly like
`apps/shared/app-login-flow.coax` and `coax/web/api/*-policy.coax`. Every Coax function
takes caller-owned buffers, explicit timestamps/entropy, and returns either:
- a bounded HTTP request descriptor (method, URL, headers, body) to execute, or
- a platform action (open auth URL in system browser, persist/clear tokens, schedule retry), or
- a verified decision/struct (verified JWT claims, webhook event, unsealed session).

---

## 5. Proposed Coax SDK design (`coax/lib/workos/`)

### 5.1 Hand-written Coax vs `oagen` Coax emitter

We inspected `oagen`, `oagen-emitters`, and `openapi-spec/oagen.config.ts`:

| Dimension | Hand-written `coax/lib/workos/` (Phase 1–2) | Custom `oagen` Coax emitter |
| --- | --- | --- |
| Upfront effort | ~2,000–2,800 LOC of pure `#authority` Coax + tests (1–2 weeks) | ~2,500–4,500 LOC TypeScript emitter (cf. `oagen-emitters/src/ios`: 2,450 LOC; `rust`: 3,740 LOC; `go`: 4,480 LOC) **plus** the same ~1,500 LOC of hand-written Coax crypto/auth helpers (`@oagen-ignore-file`) |
| Coax memory model fit | **Native fit:** fixed-capacity structs sized to real auth payloads (e.g. JWT ≤ 8 KiB, JWKS ≤ 32 KiB, User ≤ 4 KiB) | **Poor fit for 269 endpoints:** arbitrary nested OpenAPI schemas (FGA, Directory Sync custom attributes, Audit Logs) require sizing heuristics or dynamic allocation Coax does not have |
| Verification bar | 100% `#test` + `#forall` + `coax mutate` zero surviving mutants | Generated code rarely hits zero-survivor mutation cleanly without custom generator work |
| Toolchain dependency | Pure Coax (zero Node/npm in build, honoring `coax/web.md`) | Pulls Node 22 + npm/pnpm into SDK regeneration workflow |
| When it wins | **Now** — auth, sessions, PKCE, JWKS, custom UI, webhooks, events, orgs (~20 endpoints) | **Later** — only if we ever adopt 50+ enterprise CRUD endpoints (SCIM, FGA, Vault, Radar admin) |

**Recommendation:** Start with a **lean, hand-written `#authority` Coax SDK** in
`coax/lib/workos/`, pinned to a recorded `openapi-spec` version and checked against
`workos/emulate`. Keep the JSON wire codecs cleanly separated in `wire.coax` so a future
schema-driven codegen pass (either an `oagen` Coax emitter or a descriptor generator like
`coax/proto.md`) can replace or extend `wire.coax` without touching `pkce.coax`, `jwt.coax`,
`seal.coax`, `webhook.coax`, or `flow.coax`.

### 5.2 Module layout

```text
coax/lib/workos/
  README.md          # Contract, buffer ceilings, targets, verification commands
  url.coax           # #authority: RFC 3986 percent-encoding, path-segment traversal guard,
                     #   AuthKit/SSO authorize URL, logout URL, JWKS URL builders
  pkce.coax          # #authority: base64url_encode (unpadded), PKCE S256 verifier/challenge,
                     #   state generation & constant-time compare
  jwt.coax           # #authority: strict duplicate-key-free JWT parser, JWKS parser/matcher,
                     #   RS256 verification (via rsa_verify.coax), exp/nbf/iss checks,
                     #   mandatory user-session token-confusion guard (sid/auth_time/sub_profile/sub_type)
  seal.coax          # #authority: AES-256-GCM session seal/unseal (wos2. PBKDF2+HKDF or raw 32B key)
                     #   with explicit nonce/salt input & key-schedule zeroization
  webhook.coax       # #authority: WorkOS-Signature header parse (rejects unknown components),
                     #   HMAC-SHA256 verify (±180s), AuthKit Actions verify (±30s) & verdict signer
  error.coax         # #authority: dual error parser ({code,message} + {error,error_description}),
                     #   pending_authentication_token / sso_required extraction, Retry-After parser
  retry.coax         # #authority: safe-method / Idempotency-Key retry policy, bounded backoff + jitter
  wire.coax          # #authority: bounded JSON encoders & decoders for the ~20 core endpoints
                     #   (strict PublicClient vs ConfidentialClient serializer: omits client_secret on public!)
  flow.coax          # #authority: Sans-I/O client session state machine (PKCE login, callback verify,
                     #   proactive refresh + single-flight lock bit, transient vs terminal error handling,
                     #   RFC 8628 device-code polling, logout)
  adapters/
    http_native.coax # Native CLI/macOS/Linux adapter over coax/lib/net/http.coax
    c_unit.coax      # #export_c boundary compiled into coax/web/api (same pattern as auth-seal.coax)
    app_unit.coax    # #export_c boundary compiled into Apple Swift & Android JNI app hosts
  tests/
    vectors.coax     # RFC 7636, RFC 7515/7517, WorkOS webhook/action/session vectors
    emulate_e2e.sh   # Spins up workos/emulate locally on 127.0.0.1 and exercises full flows
```

### 5.3 Concrete Coax interface sketch (Sans-I/O pattern)

Following `apps/shared/app-login-flow.coax` and `coax/LIBRARIES.md`:

- **Bounded sizes:** `WOS_URL_MAX :: 2048`, `WOS_JWT_MAX :: 8192`, `WOS_JWKS_KEYS_MAX :: 16`,
  `WOS_BODY_MAX :: 65536`, `WOS_TOKEN_MAX :: 4096`. Every procedure takes caller-owned
  mutable slices (`[] u8`, `[] WosFlow`) and returns explicit status/length (`-1` or typed
  error code on refusal; never partial output on failure).
- **Zero secret leaks:** Secrets (`api_key`, `refresh_token`, `code_verifier`, `seal_key`)
  live in caller-allocated workspaces and are scrubbed with `coax/lib/crypto/zeroize.coax`
  when a flow ends or errors.
- **Public vs Confidential separation at the API boundary:**
  - `wos_public_*` procedures accept `client_id` only and have **no `api_key`/`client_secret`
    parameter**, making it impossible for mobile/desktop/web client code to accidentally
    bundle or transmit an API key — and guaranteeing `client_secret` is omitted from JSON.
  - `wos_server_*` procedures accept `client_id` + `api_key` and compile only into server
    binaries (`coax/web/api`) or operator tools.
- **Single-flight JWKS & refresh flags:** `WosJwksCache` tracks `fetched_at_ms`, `ttl_ms`
  (600,000), `refresh_in_flight`, and `unknown_kid_retried` so the host knows when to
  coalesce concurrent JWKS fetches and when to fail closed. `WosClientFlow` similarly sets
  `refreshing` so concurrent 401/expiry triggers share one refresh request instead of
  burning a rotated refresh token twice.

---

## 6. Per-platform authentication flows

| Platform | Client Type | Interactive Ceremony | Redirect / Callback | Token Storage | Refresh & Revocation |
| --- | --- | --- | --- | --- | --- |
| **Web (`openagents.com`, `gamebuilder.org`)** | **Confidential BFF** (recommended) | Either Hosted AuthKit redirect (`GET /user_management/authorize`) **or** our custom Coax `/login` calling `createMagicAuth` + `authenticateWithMagicAuth` (server-side) | `https://openagents.com/api/auth/workos/callback` (standard HTTPS; `state` bound to `__Host-oa_wos_pkce` HttpOnly cookie, constant-time compared before unseal) | HttpOnly `Secure` `SameSite=Lax` `__Host-` cookie only (either opaque `oas_` backed by Postgres, or sealed cookie — never `localStorage` or JS memory) | Server refreshes transparently; instant revocation if backed by Postgres session row; `GET /user_management/sessions/logout` clears hosted IdP session |
| **iOS (`apps/openagents-ios`)** | **Public Client (PKCE)** — `client_id` only | `ASWebAuthenticationSession` (`prefersEphemeralWebBrowserSession = false` so iCloud passkeys and SSO cookies work) | Custom scheme `openagents://auth/callback` (captured natively by `ASWebAuthenticationSession` without global scheme collision) or Universal Link | Keychain (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, not synced), matching `AppAuth.swift` | Single-flight `POST /user_management/authenticate` (`grant_type=refresh_token`, no `client_secret`) via `flow.coax`; preserve refresh token on `408`/`429`/`5xx` |
| **Android (`apps/shared/host/android`)** | **Public Client (PKCE)** — `client_id` only | Android Custom Tabs (`androidx.browser` / intent) | Verified App Link `https://openagents.com/app/callback` (preferred) or custom scheme `openagents://auth/callback` | Android Keystore AES-256-GCM encrypted `0600` file (reusing `app-login.c`'s existing Keystore seal!) | Same `flow.coax` compiled via `app_unit.coax` over existing JNI `oa_https` |
| **Desktop — macOS (`apps/openagents-desktop`)** | **Public Client (PKCE)** — `client_id` only | **`ASWebAuthenticationSession`** (native macOS 13+ AppKit, no scheme squatting!) — fallback: system browser + `openagents://auth/callback` | `ASWebAuthenticationSession` callback scheme `openagents` (macOS handles return directly to the calling window) | `~/.openagents/desktop/workos-login.json` (`0600`, atomic rename) in dev/unsigned builds to avoid Keychain prompts per `AppAuth.swift`; optional Keychain in notarized release builds | Same `flow.coax` shared across iOS, macOS, and Android! |
| **Desktop — Linux / Windows (future)** | **Public Client (PKCE)** — `client_id` only | System browser (`xdg-open` / `ShellExecute`) | **RFC 8252 loopback** `http://127.0.0.1:<ephemeral-port>/callback` (avoids OS protocol-registration / single-instance-lock complexity of `authkit-electron`) or custom scheme + single-instance lock | `0600` file under `~/.openagents/desktop/` or OS secret service / DPAPI | Same `flow.coax` |
| **CLI (`apps/openagents-cli`)** | **Public Client (Device Flow)** | RFC 8628 `POST /user_management/authorize/device` → prints `verification_uri_complete` + QR + `user_code` | None (headless/SSH friendly) | `~/.openagents/cli/` (`0600`, `OPENAGENTS_KEYCHAIN=1` opt-in) | Poll `device_code` (`authorization_pending` / `slow_down` +5s), then refresh token; or link a Nostr key after login and keep NIP-98 per-request |

### Multi-site web note (`gamebuilder.org`)

`OA_API_SITE_ORIGINS` currently lets `gamebuilder.org` log people in to the same OpenAgents
account by proxying `/api/auth/*` to `oa-api` with host-only `__Host-` cookies
([sessions-and-tokens.md](sessions-and-tokens.md#a-second-first-party-site)). With WorkOS:
- Register each first-party site's callback URI (`https://openagents.com/api/auth/workos/callback`,
  `https://gamebuilder.org/api/auth/workos/callback`) in the WorkOS Dashboard.
- Because both sites share one WorkOS environment (`client_id`) and one AuthKit domain, a
  person already logged into `openagents.com` gets near-silent SSO when clicking "Log in" on
  `gamebuilder.org`, while each origin keeps its own host-only `__Host-` session cookie.

### Apple App Store Guideline 4.8 warning

As documented in [identity-and-methods.md](identity-and-methods.md#oauth-connections-only)
and [decisions.md](decisions.md#d1-offer-oauth-login-github-google-at-all):
- If WorkOS AuthKit presents **Google, GitHub, or any third-party social login** inside our
  iOS app (including inside `ASWebAuthenticationSession`), Apple Guideline 4.8 requires
  offering **Sign in with Apple** in the same release.
- Two clean ways to comply:
  1. Enable **Apple OAuth** in WorkOS AuthKit whenever social login is enabled (WorkOS
     handles Apple's private-relay emails and OIDC validation natively), **or**
  2. Keep iOS on **Email Magic Auth + Passkeys + Enterprise SSO + Nostr** (using either
     custom UI or an AuthKit configuration without consumer social buttons), preserving the
     first-party/enterprise exemption.

---

## 7. Integration architectures & reconciling with `docs/auth/`

Adopting WorkOS touches several deliberate decisions from 2026-11-11 ([decisions.md](decisions.md)).
Three architectural options represent the real design space:

### Option A — Hybrid `acct_` Spine (Recommended)

Keep OpenAgents' opaque `acct_...` in Cloud SQL Postgres as the **sole internal primary key**
for every product table (realms, chats, wallets, Lightning Addresses, Nostr keys, agent
grants, billing/credit). WorkOS becomes an **identity & enterprise-auth provider** attached
to `acct_...`:

```text
  Web / iOS / Android / Desktop
        │  WorkOS AuthKit (PKCE / BFF) or Custom UI (Magic Auth)
        ▼
  WorkOS (auth, passkeys, OAuth, Enterprise SSO, orgs/RBAC, Radar)
        │  verified RS256 JWT (via Coax JWKS) + external_id = acct_...
        │  + workos/sync-engine (Events API → workos.* tables in Postgres)
        ▼
  oa-api (C + Coax workos SDK)
        ├── Maps workos_user_id ↔ acct_... (auth.workos_identities)
        ├── Keeps Nostr keys (NIP-98/07/46), Nostr-only accounts (D8),
        │   self-custodial wallets & L2-fresh spend step-up, Lightning Addresses,
        │   and agent accounts (oa_key_ / NIP-OA / sat spend caps)
        └── Session choice (sub-option):
              A1. Exchange WorkOS login → OpenAgents opaque session (oas_ / oa_at_ / oa_rt_
                  + OA-Device-Proof). Keeps instant server-side revocation & hardware device
                  binding; stores encrypted WorkOS refresh token server-side if needed.
              A2. Accept WorkOS RS256 access JWTs directly on oa-api (verified in Coax via
                  cached JWKS; Zeron pattern), with short access-token TTL (5 min) +
                  workos/sync-engine / webhook revocation cache for fast kill.
```

- **Pros:** Preserves every OpenAgents differentiator (Nostr-only accounts, self-custodial
  Bitcoin wallets, Lightning Addresses, agent sat-cap grants, locked Postgres backups of
  the full identity graph); keeps a clean exit path (removing WorkOS only swaps one
  authenticator table without migrating `acct_...` foreign keys); works with `sync-engine`
  so SQL joins and our 6-hourly `age`-encrypted dumps always hold a complete local copy.
- **Cons:** Two identity records to keep aligned (`acct_...` ↔ `user_01...`, mitigated cleanly
  by setting WorkOS `external_id = acct_...` and running `sync-engine`).

### Option B — Full WorkOS AuthKit Direct (Zeron-style)

Use WorkOS `user_01...` directly as the primary user ID across web (sealed session cookie),
mobile, and desktop (PublicClient PKCE sending WorkOS RS256 JWTs directly to `oa-api`).

- **Pros:** Simplest human-auth plumbing; zero token-exchange hop on mobile/desktop; matches
  Zeron's edge JWT verification pattern (`docs/zeron/architecture.md`).
- **Cons:** Directly breaks or complicates Nostr-only accounts (D8: WorkOS users require an
  email or IdP identity), removes `OA-Device-Proof` Secure Enclave refresh-token binding,
  introduces up to access-token-TTL revocation lag unless every request checks a revocation
  list, and couples every product table's foreign key to WorkOS IDs (higher vendor lock-in).

### Option C — Headless Custom UI on WorkOS APIs

Keep our existing GPU/DOM Coax login UI (`apps/openagents-web/login.coax`,
`apps/shared/app/login/flow.coax`) with **no Hosted AuthKit redirect**, replacing Resend +
our own code table with WorkOS `createMagicAuth` + `authenticateWithMagicAuth` (and
`getAuthorizationUrl` only for Enterprise SSO / OAuth), modeled on
`workos-custom-ui-authkit-example`.

- **Pros:** Zero UX disruption to our Coax-rendered login screens; no hosted third-party
  login redirect; keeps iOS/Android fully native without `ASWebAuthenticationSession` for
  email-code login.
- **Cons:** Gives up Hosted AuthKit's turnkey passkeys, MFA enrollment UI, org switcher,
  and Radar UI challenges (we'd have to build custom UI for every WorkOS challenge state
  like `mfa_enrollment`, `email_verification_required`, `organization_selection_required`),
  and mobile cannot call confidential `magic_auth` directly (must proxy through `oa-api`,
  which means `oa-api` stays in the login critical path — fine if paired with Option A1).

### Reconciliation with 2026-11-11 owner decisions (`decisions.md`)

| Decision / Principle | Status under Option A (Hybrid `acct_` Spine) | Action needed if adopting WorkOS |
| --- | --- | --- |
| **Own the user table & backups** (Principle 1, [data-and-backups.md](data-and-backups.md)) | WorkOS holds human login credentials/emails, which **does** reintroduce a third-party auth vendor (note the 2026-10-30 WorkOS CRM/sales incident discussed in [README.md](README.md#why-were-not-using-workos)). Running `workos/sync-engine` mirrors `workos.users`, `organizations`, `memberships`, `sessions`, and `authentication.*` events into Cloud SQL so our locked dumps and SQL joins stay complete. | Explicit owner trade-off: accept WorkOS as auth vendor in exchange for Enterprise SSO, AuthKit, Orgs/RBAC, Radar, and reduced auth maintenance. |
| **D1: No OAuth login / no Sign in with Apple** | Optional in WorkOS. If we enable Google/GitHub login in WorkOS for web+iOS, **D1 changes** and we must enable Sign in with Apple in WorkOS for iOS Guideline 4.8. | Decide whether to enable social OAuth + Apple OAuth, or keep OAuth as connections (via WorkOS Pipes or existing `auth.connections`). |
| **D2 & D8: Email + Nostr launch, Nostr-only accounts** | Preserved intact under Option A: Nostr login (`POST /api/auth/nostr`) continues creating/authenticating `acct_...` directly with no WorkOS user required. | Keep `coax/web/api/auth.c` Nostr routes alongside WorkOS. |
| **D3: No phone numbers, ever** | Preserved: disable Phone/SMS auth, SMS MFA, and Radar SMS challenges in the WorkOS Dashboard. | Enforce in WorkOS env config + verify in Coax SDK (`wire.coax` omits SMS grant types). |
| **D4–D6: Self-custodial wallets & Lightning Addresses** | Preserved intact: wallet seed backups (NIP-VAULT) and Lightning Addresses stay bound to `acct_...` and require L2-fresh step-up (passkey or Nostr key) — never delegated to WorkOS. | Map WorkOS `authentication_method` (`Passkey`, `SSO`, `MagicAuth`) to our `L1`/`L2` assurance levels and `auth_time` to `L2-fresh` (≤ 5 min). |
| **D7: Retire WorkOS** | Reversed (or replaced by fresh staging/prod WorkOS environments + new Secret Manager secrets). | Owner approval to reverse D7. |
| **Sessions: opaque vs JWT** | Under **Option A1**, web/app sessions stay opaque and instantaneously revocable (with `OA-Device-Proof` preserved on native). Under **Option A2**, `oa-api` accepts short-lived (5-min) WorkOS RS256 JWTs + checks a `revoked_sessions` set populated from WorkOS webhooks/events. | Choose **A1** (strongest revocation + keeps existing `AppAuth.swift` / `OA-Device-Proof`) vs **A2** (direct Zeron-style WorkOS PKCE on mobile/desktop). |
| **Agents (`#13211`)** | OpenAgents' agents have sat spend caps, NIP-OA owner attestation, and `oa_key_` scoped grants. WorkOS now ships `/agents/*` (Blueprints, Instances, Sessions) and `auth.md` (ID-JAG / RFC 7523 for third-party services). | Keep our local `auth.agent_grants` for OpenAgents' own spend-capped agents; optionally evaluate WorkOS `auth.md` / `/agents/*` later for external enterprise agent federation. |

---

## 8. Verification strategy (including offline E2E with `workos/emulate`)

Every piece of `coax/lib/workos/` can follow Coax's existing high-assurance verification stack:

1. **Pure Coax compile-time & mutation gate (`#authority`):**
   - `coax/bin/coax test coax/lib/workos/<file>.coax --library`
   - `coax/bin/coax mutate coax/lib/workos/<file>.coax --library` — **zero surviving mutants**
     on `pkce.coax`, `jwt.coax`, `seal.coax`, `webhook.coax`, `url.coax`, `error.coax`,
     `retry.coax`, and `flow.coax`.
   - Test vectors imported verbatim from RFC 7636 (PKCE), RFC 7515/7517 (JWS/JWKS),
     `workos-rust` (`webhook_verification.rs`, `actions.rs`, `session.rs` compat seals,
     `client.rs` path traversal), and `workos-ios` (`SessionSealing.swift` `wos2.`).
   - Explicit adversarial tests for: duplicate JSON keys in JWT header/payload, `alg: "none"`
     or `HS256` downgrade, duplicate `kid` in JWKS, widget/agent JWT presented as a user
     session (token confusion), unknown `v2=` webhook component, empty `client_secret`
     omission on public clients, and refresh preservation on HTTP 429/503.
2. **Native C ABI + Sanitizer + Wasm gates:**
   - Compile `c_unit.coax` and `app_unit.coax` with `coax lib` and run ASan/UBSan C harnesses
     (matching `coax/tests/library_recipes.py` and `coax/web/api/test.sh`).
   - Compile pure modules to `wasm32-wasip1` (`coax/web/wasm.sh`) with denied host effects to
     guarantee zero host leakage.
3. **Offline end-to-end tests against `workos/emulate`:**
   - Run `workos/emulate` locally on `127.0.0.1:4100` (Docker or binary) with a checked-in
     seed file (users, orgs, memberships, roles, webhook endpoints).
   - Exercise the real `coax/lib/workos/` native adapter and `coax/web/api` routes against
     `emulate` for: PKCE authorize → code exchange → JWKS fetch + RS256 verify → refresh
     rotation → session revoke → webhook signature delivery → Events API cursor tail →
     device authorization polling → injected 429/500 error hooks.
   - Zero WorkOS credentials, zero network, fully reproducible in CI and local dev.
4. **Platform smoke tests:**
   - Web: headless Chrome (`coax/web/tool.sh`) against staging + `emulate`.
   - iOS / macOS: Swift unit tests (mock transport + real `workos/emulate`) plus Simulator /
     Mac `ASWebAuthenticationSession` smoke test.
   - Android: JNI harness against `emulate` + emulator Custom Tabs callback check.

---

## 9. Phased build plan & GitHub tracking

Master tracking issue: **[#14103](https://github.com/OpenAgentsInc/openagents/issues/14103)**.

| Phase | Issue | Scope | Exit criteria |
| --- | --- | --- | --- |
| **0. Env & Secret Manager** | [#14097](https://github.com/OpenAgentsInc/openagents/issues/14097) | Provision staging/prod WorkOS envs; enable Magic Auth, Email + Password, Google/GitHub/Apple OAuth (4.8), Enterprise SSO, Optional Orgs; disable Phone/SMS & Radar SMS (D3); register web/Apple/Android redirect URIs; configure Radar & Actions secrets in Secret Manager | Staging env serves JWKS; secrets bound only to runtime API account |
| **1. Pure `#authority` primitives** | [#14098](https://github.com/OpenAgentsInc/openagents/issues/14098) | Hand-written `#authority` Coax core in `coax/lib/workos/`: `pkce.coax`, `url.coax`, `webhook.coax` (incl. AuthKit Actions), `seal.coax` (`wos2.` + `workos-rust` compat), `jwt.coax` (duplicate-key guard + RS256 JWKS + token-confusion guard), `error.coax`, `retry.coax` | All RFC & WorkOS vectors pass; `coax mutate` **zero surviving mutants**; ASan/UBSan C unit & Wasm build clean |
| **2. `oagen` emitter, 269 endpoints & `emulate` E2E** | [#14099](https://github.com/OpenAgentsInc/openagents/issues/14099) | In-repo `tools/oagen-coax/` emitter generating checked-in Sans-I/O borrowed-`Jv` Coax files into `coax/lib/workos/gen/` for all 269 endpoints (zero Node in normal builds; omits `client_secret` on public ops); Sans-I/O `flow.coax`; `adapters/{http_native,c_unit,app_unit}.coax`; offline `tests/emulate_e2e.sh` | All 269 generated endpoints compile cleanly; `flow.coax` zero surviving mutants; full offline E2E passes against `workos/emulate` |
| **3. `oa-api`, Custom Coax web `/login`, Mirror & Actions** | [#14100](https://github.com/OpenAgentsInc/openagents/issues/14100) | `auth.workos_identities` + `workos.*` mirror schema; Option A1 routes behind `OA_WORKOS_ENABLED` (Magic Auth, OAuth/SSO, `/api/auth/workos/exchange`, multi-org, `email_verified=true` auto-link at `L1`, Radar forwarding, webhooks, synchronous AuthKit Actions, Coax/C `GET /events` poller, freeze/purge propagation); Custom Coax web `/login` UI (`gamebuilder.org` untouched) | Staging web login, Actions, webhooks, Events mirror pass in headless Chrome & `test.sh`; Nostr/Passkey/wallet/agent/`gamebuilder.org` checks stay green |
| **4. Native iOS, macOS, Android & CLI** | [#14101](https://github.com/OpenAgentsInc/openagents/issues/14101) | Wire `app_unit.coax` + `flow.coax` into `AppAuth.swift` (`ASWebAuthenticationSession` on iOS/macOS + Keychain / `0600` file), Android `app-login.c` (Custom Tabs + App Link + Keystore), and `openagents-cli` (RFC 8628 device flow + NIP-98) with `OA-Device-Proof` preserved | Magic Auth, OAuth/SSO PKCE exchange, org picker, refresh, transient 429/offline preservation, and logout verified on iOS Simulator, macOS, Android emulator, and CLI |
| **5. Prod cutover & `gamebuilder.org` follow-up** | [#14102](https://github.com/OpenAgentsInc/openagents/issues/14102) | Enable `OA_WORKOS_ENABLED=1` on production `openagents.com`; verify monthly restore drill includes `workos.*`; B2B org switcher & Enterprise SSO polish; follow-up cutover of `gamebuilder.org` & retirement of legacy first-party login-code routes (Resend stays for security notices) | Production `openagents.com` green across all platforms; locked backup drill passes; `gamebuilder.org` migrated |

## 10. Resolved owner decisions (2026-11-26)

All six decisions from the original exploration were resolved on 2026-11-26 (and reflected
in [decisions.md](decisions.md)):

1. **D7 reversed:** Adopt WorkOS for human auth on `openagents.com` (fresh staging/prod
   environments, `#14097`), mitigated by the Hybrid `acct_` spine and pure Coax/C `workos.*`
   Events mirror into Cloud SQL + locked backups.
2. **Architecture:** **Option A1** (Hybrid `acct_` spine + OpenAgents opaque sessions
   everywhere with `OA-Device-Proof` and instant revocation; auto-link only on
   `email_verified=true`; all WorkOS logins at `L1`).
3. **UI:** **Custom Coax UI** everywhere (Magic Auth inline + direct OAuth/SSO redirects +
   optional org picker + Nostr + first-party Coax WebAuthn Passkeys).
4. **D1 reversed:** Offer Google, GitHub, and **Sign in with Apple** OAuth (satisfying iOS
   Guideline 4.8 natively) plus Enterprise SSO.
5. **D3 unchanged:** No phone numbers or SMS anywhere (disabled in WorkOS Dashboard).
6. **Nostr, wallets, agents & rollout:** Nostr-only accounts (D8), self-custodial wallet
   `L2-fresh` step-up (D4–D6), and sat-capped agent grants (`#13211`) stay first-class in
   our `auth` schema. Build the in-repo `oagen` Coax emitter (`tools/oagen-coax/`) for all
   269 endpoints alongside `#authority` helpers (`#14098`, `#14099`), wire Radar + AuthKit
   Actions + freeze/delete propagation in v1 (`#14100`), and cut over `openagents.com` first
   before migrating `gamebuilder.org` (`#14102`).
