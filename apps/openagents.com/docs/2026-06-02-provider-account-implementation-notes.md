# Provider Account Implementation Notes

Date: 2026-06-02

This note tracks the implementation slice started from GitHub issue #1 and the
ChatGPT/Codex account connection audit.

## Boundary

OpenAuth remains first-party OpenAgents identity. ChatGPT/Codex account
connection state belongs to ProviderAccountService in `workers/api`.

D1 stores provider-account metadata, public connection attempt state, stable
secret references, short-lived grant references, and events. It must not store
raw ChatGPT/Codex credential payloads or OpenCode runtime auth payloads.

## First Package

`packages/provider-account-schema` owns the shared Effect Schema types and the
Vortex-derived secret-reference policy:

- branded refs and ids;
- provider account status and health enums;
- connection attempt status/source/method enums;
- auth grant status enums;
- public provider-account response models;
- secret-material detection;
- public secret-reference validation.

The package accepts refs such as `codex-auth://...`, `secret://...`,
`vault://...`, `gcp-secret://...`, `cloud-secret://...`, and
`provider-account://...`. It rejects raw credential-shaped material such as
OpenAI API keys, bearer tokens, OAuth token JSON fields, private keys,
`auth.json`, and `OPENCODE_AUTH_CONTENT`.

## First D1 Tables

Migration `0009_provider_accounts.sql` adds:

- `provider_accounts`;
- `provider_account_connection_attempts`;
- `provider_account_auth_grants`;
- `provider_account_events`;
- `runner_sessions`.

Column names intentionally avoid raw token names such as `access_token`,
`refresh_token`, `id_token`, `code_verifier`, `device_code`, and `auth_json`.
Future sync/API projections should use explicit public projection functions
from these rows rather than returning rows directly.

## Device Login First Slice

GitHub issue #2 adds the first browser-visible ChatGPT/Codex connection
surface in `workers/api`:

- `GET /api/provider-accounts`;
- `POST /api/provider-accounts/chatgpt-codex/device-login/start`;
- `GET /api/provider-accounts/chatgpt-codex/device-login/:attemptId`;
- `POST /api/provider-accounts/:providerAccountRef/disconnect`;
- a signed-in homepage panel listing ChatGPT/Codex accounts and active device
  login attempts.

The Worker-native device start call uses the OpenCode-observed Codex client id
and OpenAI's public device-code ceremony endpoint. The Worker persists only
public ceremony fields in D1: verification URL, user code, expiry, attempt id,
provider account ref, status, and health.

The OpenAI `device_auth_id` returned by the start call is not stored in D1,
browser state, sync state, or public docs/API responses. It is stored only in
Worker KV under `provider-device-login:<attemptId>` with an expiry-aligned TTL,
because the OpenCode headless flow requires it to poll
`/api/accounts/deviceauth/token` after the human completes the browser device
flow.

The browser `Refresh status` action now polls OpenAI through the Worker. On
success, the Worker exchanges the returned authorization code for OAuth tokens,
stores the OpenCode auth JSON under a private KV key
`provider-auth:<providerAccountRef>`, deletes the transient device-login key,
and records only `codex-auth://provider-account_...` in D1. Pending attempts
created before this KV storage existed cannot be completed; a refresh will mark
those stale attempts failed so the user can start a new connection.

This means issue #2 can start and display a real device login ceremony, list
multiple accounts, project pending attempts as expired, complete a current
device login from the browser refresh action, and disconnect accounts with grant
revocation.

## Broker Callback Slice

GitHub issue #3 adds broker/service callbacks for the device-login attempts
started by issue #2:

- `POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/connected`;
- `POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/failed`;
- `POST /api/provider-accounts/:providerAccountRef/health`.

These routes intentionally do not accept browser-session auth. They require a
programmatic-agent bearer token, so the first SHC broker can be represented as a
normal OpenAgents service actor rather than a global static operator secret.
The actor id is recorded on provider-account events.

The connected callback is idempotent for already-connected attempts, rejects
stale pending attempts, rejects provider-account-ref mismatches, stores only a
stable secret ref such as `codex-auth://provider-account_...`, marks the
account `connected` / `healthy`, clears public ceremony fields on the attempt,
and writes a `login_connected` event. The failed callback records
`failed`, `denied`, or `expired` attempt state, maps account state to `denied`
or `expired`, and writes the corresponding event. The health callback lets SHC
or a runner mark `healthy`, `unhealthy`, or `requires_reauth`; a revoked Codex
token should map to `requires_reauth`.

Operator sanity checks now use the same provider-auth class as the runner
boundary instead of only proving grant resolution. After reading the private
Codex auth cache, the Worker performs an OAuth refresh probe. If the provider
returns a replacement token set, the Worker stores the rotated cache back into
private KV and clears stale D1 blockers such as `reauth_required_reason`,
cooldown, low-credit, and recent failure class. If the refresh token is expired,
reused, revoked, invalid, or otherwise normalized as `token_invalidated`, the
health event records only the redacted class, marks the account
`requires_reauth`, and the fleet selector excludes that account from future
leases until it is reconnected. Raw auth JSON, access tokens, refresh tokens,
provider response bodies, and secret refs remain out of D1 and issue-safe
outputs.

Callback bodies must contain only safe public metadata: account label/email,
plan type, stable secret ref, public reason text, and status/health values.
The service rejects credential-shaped text in labels, reasons, and refs before
it can reach D1 or event metadata.

## Grant Issue And Resolve Slice

GitHub issue #4 adds the SHC/OpenCode grant boundary:

- `POST /api/provider-accounts/:providerAccountRef/grants`;
- `POST /api/provider-accounts/chatgpt-codex/grants/resolve`.

Grant issue is a browser-session route. It requires the provider account to be
`connected` and `healthy`, have a stable secret ref, and belong to the signed-in
user. It returns only public grant metadata: `grantRef`, status, expiry,
provider account ref, and optional action/run scoping fields. It never returns
`providerSecretRef` to the browser.

Grant resolve is a service route. It requires a programmatic-agent bearer token,
checks that the grant is `issued`, unexpired, not revoked, not already used,
and matches the requested provider account / runner session when supplied. It
marks the grant `used`, writes an `auth_grant_used` event, and returns the
stable provider secret ref plus a redacted OpenCode materialization plan:

```json
{
  "provider": "openai",
  "authRef": "codex-auth://provider-account_...",
  "authContentEnv": "OPENCODE_AUTH_CONTENT",
  "homeIsolation": "per-run-opencode-home",
  "serverPassword": "runner-generated",
  "scrubAfterCloseout": true
}
```

The plan is a contract, not credential material. SHC must resolve/decrypt the
secret ref inside the runner, construct `OPENCODE_AUTH_CONTENT` or an isolated
`auth.json` locally, start OpenCode/Codex, and scrub the material at closeout.

## Public Projection And Redaction Slice

GitHub issue #5 adds the explicit safe projection boundary for provider-account
sync/API state.

Allowed public sync collections are centralized in
`PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS`:

- `provider_accounts_public`;
- `provider_connection_attempts_public`;
- `provider_account_events_public`;
- `provider_account_grants_public`;
- `runner_sessions_public`.

The worker projection functions now run every public value through
`assertProviderAccountPublicProjection` before returning or preparing it for a
future sync patch:

- `toPublicProviderAccount`;
- `toPublicProviderConnectionAttempt`;
- `toPublicProviderAccountGrant`;
- `toPublicProviderAccountEvent`.

Those projections deliberately omit raw rows, `secretRef`,
`providerSecretRef`, OpenCode auth payloads, `device_auth_id`,
`OPENCODE_AUTH_CONTENT`, and `auth.json` contents. Event projection also checks
stored `metadataJson`, `sourceRefsJson`, and `evidenceRefsJson` before exposing
the public event shell.

`packages/provider-account-schema` owns the shared secret scanner and redactor:

- `containsProviderSecretMaterial`;
- `assertNoProviderSecretMaterial`;
- `redactProviderAccountSecretMaterial`;
- `redactProviderAccountLogValue`.

The scanner covers OpenAI API keys, bearer headers, OAuth token field names,
OpenCode `{"openai":{"type":"oauth",...}}` auth JSON, `OPENCODE_AUTH_CONTENT`,
`auth.json`, private keys, and JWT-shaped values. Provider-account route error
messages now pass through the redactor before being returned to callers.

Tests now cover:

- credential-shaped text detection, including representative OpenCode auth
  JSON;
- redaction of authorization headers and escaped OpenCode auth JSON in logs;
- migration-policy rejection of raw credential column names;
- public projection rejection for account, connection attempt, grant, and event
  values that contain credential-shaped material;
- public account/grant API shape omitting secret refs.
