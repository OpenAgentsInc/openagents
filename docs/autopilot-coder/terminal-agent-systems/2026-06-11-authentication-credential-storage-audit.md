# Authentication And Credential Storage Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #34 from the Bun/Effect terminal-agent systems list. It captures
how a new terminal coding agent should handle login, credential resolution,
secure storage, refresh, revocation, and auth-dependent cache invalidation.

The important boundary is that credentials are not settings. Settings may point
to credential sources, but the runtime must resolve, store, refresh, redact,
and revoke credentials through a dedicated auth service.

## Target

Build an auth system that supports:

- Interactive login.
- Headless credentials.
- Managed hosted sessions.
- External provider accounts.
- Per-connector OAuth.
- Account-pool leasing.
- Local secure storage.
- CI and test credentials.
- Logout and revocation.

The system should be usable by the native agent loop, delegated agent adapters,
connector transports, paid work-order execution, and future provider peers.

## User-Visible Capability

Users should be able to:

- Log in from the terminal.
- Use API keys or bearer tokens in CI.
- Use external provider credentials without mixing them with interactive login.
- See which account or provider is active without exposing secrets.
- Log out and clear local credentials.
- Reconnect expired providers.
- Authorize connector-specific OAuth flows.

Operators should be able to:

- Lease provider accounts to runs.
- Enforce team policy around allowed providers.
- Rotate or revoke credentials.
- Verify that public artifacts do not contain raw tokens.

## Credential Sources

Use a typed source model:

```ts
type CredentialSourceKind =
  | "environment"
  | "file_descriptor"
  | "secure_store"
  | "helper_command"
  | "managed_session"
  | "connector_oauth"
  | "account_pool"
  | "test_fixture"

interface CredentialSourceRef {
  readonly kind: CredentialSourceKind
  readonly provider: string
  readonly scope?: string
  readonly workspaceBound: boolean
  readonly interactiveAllowed: boolean
  readonly priority: number
}

interface CredentialEnvelope {
  readonly subjectId: string
  readonly provider: string
  readonly source: CredentialSourceRef
  readonly scopes: readonly string[]
  readonly expiresAt?: string
  readonly redactedPreview: string
  readonly storageRef?: string
}
```

The durable runtime should persist credential references, not credential
payloads. Raw secrets belong in a secure store or ephemeral process memory.

## Resolution Rules

Credential resolution must be deterministic and context-aware.

Recommended precedence:

1. Test fixtures in test mode.
2. Explicit headless credentials from environment or file descriptor.
3. Managed session credentials for hosted or delegated execution.
4. Account-pool lease credentials.
5. Helper-command credentials after workspace trust.
6. Secure-store credentials from interactive login.
7. Provider-specific fallback credentials.

Important constraints:

- Managed sessions should not accidentally use the user's personal local
  credentials.
- CI should fail clearly when required credentials are absent.
- Helper commands should win over secure-store credentials when configured.
- Helper commands from workspace settings must not execute before trust.
- Account-pool leases should be scoped to a run and expire explicitly.

## Secure Storage

Implement a `SecureStore` service with platform-specific backends.

```ts
interface SecureStore {
  readonly read: Effect.Effect<StoredCredentialBundle | null, SecureStoreError>
  readonly write: (
    bundle: StoredCredentialBundle,
  ) => Effect.Effect<void, SecureStoreError>
  readonly delete: Effect.Effect<void, SecureStoreError>
}
```

Backend expectations:

- Use OS-backed secure storage where available.
- Use a fallback only when the platform has no better local option.
- Keep fallback storage clearly labeled as weaker.
- Cache reads briefly to avoid startup stalls.
- Deduplicate concurrent reads.
- Serve stale cached data on transient secure-store read failure when no
  explicit invalidation happened.
- Clear cache generations on write and delete.
- Avoid putting raw secrets in command arguments where process monitors can
  read them.

The store should use stable namespacing so multiple config homes or profiles do
not collide.

## Interactive OAuth

Interactive OAuth should use a standard authorization-code flow with PKCE.

Required properties:

- Generate a state value and validate it on callback.
- Use a local callback listener where possible.
- Allow a manual redirect fallback.
- Time out waiting for user completion.
- Exchange code for tokens with a short request timeout.
- Persist only after token exchange succeeds.
- Fetch account metadata separately when needed.
- Redirect or close the browser flow cleanly on success or error.

The local callback listener should only accept the expected path and expected
state. Any missing code or state mismatch should fail the login.

## Token Refresh

The refresh service should:

- Check expiry before requests.
- Refresh in advance of hard expiry.
- Preserve old refresh tokens if the server does not rotate them.
- Fetch profile or account metadata only when necessary.
- Emit structured success and failure events.
- Treat invalid-grant-style failures as credential invalidation.
- Avoid repeated refresh storms through memoization.

Refresh must invalidate auth-dependent caches:

- User profile.
- Policy limits.
- Feature flags.
- Connector inventories.
- Tool schema caches.
- Remote managed settings.
- Account-pool state.

The cache invalidation path should be explicit on login, logout, refresh, and
account switch.

## Helper Commands

Some enterprises need helper commands to mint short-lived credentials.

Rules:

- Helper commands are settings references, not secrets.
- Commands from workspace sources require workspace trust.
- Commands should have a bounded timeout.
- Stdout must produce exactly the expected credential shape.
- Failures should be visible to the user but not leak stderr into public logs.
- Stale-while-revalidate is acceptable after one successful helper result.
- A cold helper failure should not silently fall back to an unrelated personal
  account.

## Cloud Provider Refresh

Provider-specific refresh commands should follow the same trust model:

- Probe existing credentials first.
- Run interactive refresh only when needed.
- Bound refresh time.
- Stream user-visible progress where it helps.
- Cache successful short-lived credentials to their natural TTL.
- Clear provider SDK caches after refresh.

This should be a plugin or provider module behind `CredentialResolver`, not
hard-coded throughout the runtime.

## Connector OAuth

Connector-specific OAuth needs per-server identity.

Use a key derived from:

- Connector name.
- URL or authority.
- Relevant auth metadata.
- Static headers that affect identity, redacted where logged.

The key prevents one connector's token from being reused for another connector
with the same display name.

Connector OAuth should support:

- Metadata discovery.
- Configured metadata URLs with HTTPS enforcement.
- PKCE and state validation.
- Browser-open and manual callback modes.
- Refresh with retry for transient failures.
- Invalid-grant invalidation.
- Best-effort server-side revocation.
- Always clearing local tokens on logout or clear-auth.

Revocation should attempt refresh token first, then access token, and should
still clear local storage if the remote revocation endpoint is absent or
fails.

## Redaction

Redaction should be centralized.

Never log:

- Access tokens.
- Refresh tokens.
- API keys.
- Authorization codes.
- PKCE verifiers.
- State and nonce values.
- Raw credential helper output.
- File descriptor contents.
- Secure-store payloads.

URLs used in auth logs should redact sensitive query parameters before logging.
Public artifacts should show only provider, account identity, scopes, expiry,
and a redacted preview.

## Effect Services

Recommended service split:

- `AuthResolver`: chooses a credential source for a request.
- `SecureStore`: persists local credential bundles.
- `OAuthClient`: implements OAuth flows and refresh.
- `ConnectorAuth`: handles per-connector credential lifecycles.
- `AccountLeaseService`: leases provider accounts to runs.
- `AuthCache`: memoizes auth-dependent reads.
- `AuthRedactor`: produces safe logs and public projections.
- `LogoutService`: revokes, clears, flushes, and invalidates.
- `TrustPolicy`: gates helper and workspace-defined auth behavior.

Every service should use typed Effect errors and scoped resources for local
listeners, sockets, and subprocesses.

## Safety Rules

- Credentials are never durable runtime events.
- Public run records store credential references only.
- Managed sessions do not read personal secure-store credentials.
- Workspace-defined credential helpers require trust.
- Logout flushes telemetry before clearing account identity.
- Account switches clear signed message blocks and provider caches.
- Revocation failures do not prevent local credential deletion.
- Secure-store fallback use is visible in diagnostics.

## Tests

Minimum coverage:

- Credential precedence by session type.
- Managed session isolation from personal credentials.
- Workspace-trust gate for helper commands.
- Helper command TTL and stale-while-revalidate behavior.
- OAuth state mismatch rejection.
- Local callback success and manual callback fallback.
- Token refresh rotation and invalid-grant invalidation.
- Logout cache clearing.
- Connector credential key uniqueness.
- Revocation fallback and local delete.
- Redaction of URLs, logs, and public projections.

## OpenAgents Translation Notes

Checked the open OpenAgents issue list on 2026-06-11.

Related live roadmap issues:

- #4766 covers the account-pool dashboard: connected accounts, lease load,
  cooldowns, reset timers, and reconnect nudges.
- #4771 covers provider peer connect flows.
- #4770 covers team budgets and per-mission caps.
- #4773 covers API parity.
- #4786 is the Autopilot MVP ladder epic.

No open issue explicitly names the secure credential store, token refresh
service, or connector-OAuth lifecycle. Those are prerequisites for #4766 and
#4771, but should not be described as live until implemented and verified.

Recommended OpenAgents shape:

- Introduce `CredentialRef`, `AccountLease`, and `AuthProviderConnection`
  records.
- Keep raw tokens out of D1-visible mission and artifact tables.
- Attach account leases to missions and work orders.
- Expose reconnect state through the account-pool dashboard.
- Make provider-peer support depend on a ToS and credential-boundary review.

## Decision

Build this as a core runtime service before adding more provider peers. The
credential system is the trust root for delegated agents, account pools,
payment-backed work orders, connector access, and remote execution.
