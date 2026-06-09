# OpenAgents ChatGPT/Codex Account Connection Audit

Date: 2026-06-02

Status: architecture audit for the Cloudflare-only `openagents` plan.

Scope:

- inspect how Vortex connects ChatGPT/Codex accounts today;
- compare that to what OpenCode expects from provider auth, console auth, CLI
  account auth, and server auth;
- define how OpenAgents OpenAuth should relate to both without conflating
  first-party identity with third-party provider credentials;
- recommend the Cloudflare-only implementation shape for `openagents`.

## Executive Summary

Keep three authority planes separate.

```text
OpenAuth
  -> OpenAgents human, workspace, service, and runner identity
  -> sign-in, session, actor, membership, authorization

OpenAgents Provider Accounts
  -> ChatGPT/Codex provider-account connection state
  -> device-login ceremony
  -> server-side secret reference
  -> scoped runner grants

OpenCode Runtime
  -> SHC/GCloud coding-agent execution process
  -> provider credential materialization for one run
  -> local/server auth for the OpenCode HTTP control plane
```

The Vortex implementation already points in the right direction. It does not
store raw ChatGPT/Codex credential material in Convex or browser state. It
stores public account status, a stable `providerAccountRef`, a public
device-code ceremony, and a server-side `secretRef`. Workroom launch then
issues a short-lived auth grant. A Cloud runner resolves that grant through a
bearer-protected endpoint and receives a provider-secret reference, not raw
tokens.

OpenCode expects a different shape. Its normal provider auth is local runtime
credential config in `~/.local/share/opencode/auth.json`, or equivalent
`OPENCODE_AUTH_CONTENT`. Its OpenAI Codex plugin can run ChatGPT Pro/Plus
browser OAuth or headless device auth and then stores OAuth refresh/access
material for the `openai` provider. OpenCode's hosted console auth is OpenAuth
for OpenCode accounts/workspaces. Its CLI `account login` device flow logs a
machine into the OpenCode console/account service. Its server mode auth is
HTTP Basic Auth controlled by `OPENCODE_SERVER_PASSWORD`.

Those OpenCode auth systems are useful, but none of them replace OpenAgents
Provider Accounts.

The correct target is:

```text
OpenAgents OpenAuth identity
  authenticates the actor
  authorizes account connection and grant issuance

OpenAgents ProviderAccountService
  owns ChatGPT/Codex account metadata
  owns the device-login broker contract
  owns secret-reference policy
  owns grant issuance/resolution/revocation

SHC OpenCode runner
  receives only a run-scoped grant
  resolves it through OpenAgents
  materializes an OpenCode-compatible provider auth payload
  starts OpenCode/Codex for the run
  clears local credential material after the run
```

Do not put ChatGPT/Codex credential storage into the OpenAuth issuer. Do not
use OpenCode console account login as the product's provider-account fleet.
Do not expose an OpenCode server as the product auth boundary. The product
boundary is OpenAgents, not OpenCode.

## Local Sources Inspected

Vortex:

- `../vortex/components/settings/workspace-settings.tsx`
- `../vortex/app/api/provider-accounts/chatgpt-codex/device-login/start/route.ts`
- `../vortex/app/api/provider-accounts/chatgpt-codex/device-login/status/route.ts`
- `../vortex/app/api/provider-accounts/chatgpt-codex/grants/resolve/route.ts`
- `../vortex/convex/providerAccounts.ts`
- `../vortex/convex/schema.ts`
- `../vortex/convex/lib/providerAccountPolicy.ts`
- `../vortex/server/providerAccountPolicy.test.ts`
- `../vortex/lib/autopilot/codex-vm-workroom.ts`
- `../vortex/components/autopilot/prototype.tsx`
- `../vortex/docs/chatgpt-codex-provider-accounts.md`
- `../vortex/docs/2026-06-02-workos-to-openauth-cloudflare-worker-audit.md`

OpenCode:

- `../projects/repos/opencode/packages/console/function/src/auth.ts`
- `../projects/repos/opencode/packages/console/app/src/context/auth.ts`
- `../projects/repos/opencode/packages/console/core/src/schema/auth.sql.ts`
- `../projects/repos/opencode/packages/console/core/src/schema/provider.sql.ts`
- `../projects/repos/opencode/packages/console/core/src/provider.ts`
- `../projects/repos/opencode/packages/opencode/src/auth/index.ts`
- `../projects/repos/opencode/packages/opencode/src/provider/auth.ts`
- `../projects/repos/opencode/packages/plugin/src/index.ts`
- `../projects/repos/opencode/packages/opencode/src/plugin/openai/codex.ts`
- `../projects/repos/opencode/packages/opencode/src/account/account.ts`
- `../projects/repos/opencode/packages/opencode/src/account/repo.ts`
- `../projects/repos/opencode/packages/core/src/account/sql.ts`
- `../projects/repos/opencode/packages/opencode/src/server/auth.ts`
- `../projects/repos/opencode/packages/opencode/src/server/routes/instance/httpapi/middleware/authorization.ts`
- `../projects/repos/opencode/packages/web/src/content/docs/providers.mdx`
- `../projects/repos/opencode/packages/web/src/content/docs/cli.mdx`
- `../projects/repos/opencode/SECURITY.md`

OpenAgents Autopilot:

- `README.md`
- `AGENTS.md`
- `docs/2026-06-02-cloudflare-only-openagents-sync-audit.md`
- `workers/api/src/index.ts`
- `workers/api/migrations/0001_openagents_sync.sql`
- `packages/sync-schema/src/index.ts`
- `packages/sync-worker/src/index.ts`

## What Vortex Built

Vortex has a provider-account subsystem for ChatGPT/Codex accounts. It is
currently implemented with WorkOS/AuthKit for the browser session, Convex for
the product ledger, and a Codex App Server WebSocket broker for the actual
ChatGPT/Codex device-login state.

The browser Settings card reads:

```text
Connect ChatGPT accounts that Codex VM workrooms can use.
Credentials stay on the broker; Vortex stores only account status.
```

That sentence is the key boundary.

### Vortex Account Model

The relevant Convex tables are:

```text
providerAccounts
providerAccountAuthGrants
providerAccountEvents
```

`providerAccounts` stores:

- Vortex `userId`;
- optional organization/project scope;
- provider literal `chatgpt_codex`;
- auth mode such as `chatgpt_device_code`;
- status and health;
- stable `providerAccountRef`;
- optional `secretRef`;
- account label and plan type;
- public login ceremony fields such as `loginRef`, `verificationUrl`,
  `userCode`, and `expiresAt`;
- public metadata JSON.

It does not store raw access tokens, refresh tokens, device auth IDs, PKCE
verifiers, API keys, or an `auth.json` payload.

`providerAccountAuthGrants` stores:

- provider-account id/ref;
- user/workspace/thread/workroom scoping fields;
- a stable `grantRef`;
- grant status;
- requested action;
- expiry;
- `providerSecretRef`.

The important nuance is that `providerSecretRef` is still a reference. The
grant is not a raw ChatGPT token.

`providerAccountEvents` stores audit history for login starts, login
connections, disconnections, grant issuance, grant revocation, and grant
failure.

### Vortex Device Login Start

Normal browser use calls:

```text
POST /api/provider-accounts/chatgpt-codex/device-login/start
```

The route supports two modes:

```text
browser user
  -> WorkOS/AuthKit session
  -> Convex auth token
  -> providerAccounts.startChatGptCodexDeviceLogin

operator bridge
  -> Authorization: Bearer VORTEX_PROVIDER_ACCOUNT_OPERATOR_TOKEN
  -> target user email in body
  -> providerAccounts.operatorStartChatGptCodexDeviceLogin
```

When a Codex App Server WebSocket URL is configured, the route opens a
WebSocket and speaks JSON-RPC:

```text
initialize
initialized
account/login/start { type: "chatgptDeviceCode" }
```

The broker result is normalized to public device-login fields:

- `loginRef`;
- `verificationUrl`;
- `userCode`.

Vortex then stores those fields in Convex with a short pending-login TTL.

If the broker is not configured, Vortex can write a placeholder pending
account. That placeholder is useful for UI and account-fleet plumbing, but it
is not a completed provider login and must not authorize a runner.

### Vortex Device Login Status

The status endpoint is operator-only:

```text
POST /api/provider-accounts/chatgpt-codex/device-login/status
Authorization: Bearer VORTEX_PROVIDER_ACCOUNT_OPERATOR_TOKEN
```

It calls the Codex App Server:

```text
initialize
initialized
account/read
```

If the broker reports `account.type === "chatgpt"`, Vortex records the account
as connected and stores:

```text
codex-auth://<providerAccountRef>
```

as the server-side secret reference.

The route has an explicit `refreshToken` option, but the default is a
non-mutating account read so the operator probe does not consume or rotate
credential material right before a run needs it.

### Vortex Secret Policy

`convex/lib/providerAccountPolicy.ts` has the policy that matters most for the
migration.

Accepted public secret-reference prefixes include:

```text
secret://
vault://
gcp-secret://
cloud-secret://
provider-account://
codex-auth://
```

Rejected credential-shaped values include:

- OpenAI-style API keys;
- bearer tokens;
- JSON fields named like `access_token`, `refresh_token`, `id_token`,
  `code_verifier`, or `device_code`;
- `OPENAI_API_KEY`;
- `CODEX_ACCESS_TOKEN`;
- `auth.json`;
- private keys;
- JWT-looking strings.

`server/providerAccountPolicy.test.ts` covers the important regressions:

- stable secret refs are accepted;
- raw API keys, bearer tokens, token JSON, and `auth.json` references are
  rejected;
- public metadata fails closed if it contains credential-shaped material.

This policy should be ported almost directly to `openagents` as Effect
Schema plus typed errors.

### Vortex Grant Flow

The workroom launch path is:

```text
select connected chatgpt_codex provider account
  -> issueSessionAuthGrant
  -> POST /api/workrooms/start with providerAccountRef + authGrantRef
  -> runner resolves grant
  -> runner materializes VM-side auth
```

The selected account must have:

- provider `chatgpt_codex`;
- `hasSecretRef`;
- `status === "connected"`;
- `publicStatus === "connected"`.

`issueSessionAuthGrant` creates a `codex-auth-grant` ref with a two-hour TTL.
If a thread is involved, it also records an authority-boundary receipt that the
workroom received a scoped provider-account grant reference.

The runner resolves the grant through:

```text
POST /api/provider-accounts/chatgpt-codex/grants/resolve
Authorization: Bearer VORTEX_CLOUD_RUNNER_GRANT_TOKEN
```

The route returns:

- `grantRef`;
- provider literal `chatgpt_codex`;
- `providerAccountRef`;
- `providerSecretRef`;
- requested action;
- expiry;
- status.

It does not return a ChatGPT refresh token or `auth.json`.

### Vortex Disconnect

Disconnect clears `secretRef`, moves health to `requires_reauth`, marks the
account disconnected, revokes issued grants, and writes a provider-account
event.

New workroom grants cannot be issued from a disconnected account.

### Vortex Strengths

Vortex got these right:

- provider-account authority is separate from product sign-in;
- browser state sees public ceremony/status only;
- raw credentials are behind a broker/secret reference;
- grants are short-lived and scoped to a runner action;
- workroom launch is blocked if no connected account exists;
- secret-shaped material is rejected in provider-account metadata;
- provider-account events and receipts create an authority audit trail.

### Vortex Weaknesses

The current Vortex implementation is still a bridge:

- browser identity depends on WorkOS/AuthKit;
- product authority depends on Convex;
- status recording depends on an operator bearer token rather than a fully
  modeled service actor;
- the Codex broker is out-of-band;
- connection attempts, secret refs, and runner grants are not yet a
  Cloudflare-native D1/DO/Queue workflow;
- the user status UI relies on Convex live queries rather than OpenAgents
  Sync;
- the broker/ref-to-secret semantics are not fully represented as a typed
  provider-account adapter package.

Those are implementation maturity issues, not a reason to throw away the
boundary.

## What OpenCode Expects

OpenCode has several auth surfaces. They sound similar but serve different
purposes.

### OpenCode Console Auth

`packages/console/function/src/auth.ts` is a Cloudflare Worker using
OpenAuth's `issuer`.

It configures:

- OpenAuth subjects for `account` and `user`;
- GitHub provider;
- Google OIDC provider;
- Cloudflare KV storage for issuer state;
- database-backed account/workspace/user linking.

On successful GitHub/Google login, it:

- resolves a verified email;
- finds or creates an OpenCode `account`;
- links provider identity and email identity in `AuthTable`;
- ensures the account has a workspace;
- returns an OpenAuth `account` subject.

OpenCode's app then uses a Solid Start session cookie to store the current
OpenCode account and derive an actor. This is first-party product identity for
OpenCode's console. It is not ChatGPT provider credential storage.

The closest OpenAgents equivalent is:

```text
auth.openagents.com
  -> OpenAuth issuer
  -> GitHub / Google / future email code provider
  -> OpenAgents account subject
  -> D1 account/user/workspace rows
  -> OpenAgents session cookie or token
```

### OpenCode Provider Auth

OpenCode provider credentials are handled by `packages/opencode/src/auth`
and `packages/opencode/src/provider/auth`.

The local auth service reads and writes:

```text
~/.local/share/opencode/auth.json
```

It can also read:

```text
OPENCODE_AUTH_CONTENT
```

The stored auth types include:

```text
api
  -> key
  -> metadata

oauth
  -> refresh
  -> access
  -> expires
  -> optional accountId / enterpriseUrl

wellknown
  -> key
  -> token
```

The provider auth service discovers plugin auth hooks, asks the selected
plugin to authorize, and then writes either API-key auth or OAuth auth to the
local OpenCode auth service.

The OpenCode HTTP API has provider auth endpoints:

```text
GET  /provider/auth
POST /provider/{id}/oauth/authorize
POST /provider/{id}/oauth/callback
```

These endpoints are for configuring a running OpenCode instance's provider
credentials. They are not a multi-tenant product provider-account ledger.

### OpenCode OpenAI Codex Plugin

`packages/opencode/src/plugin/openai/codex.ts` is the closest OpenCode code to
the Vortex ChatGPT/Codex provider-account connection.

It defines an OpenAI provider auth hook with three methods:

```text
ChatGPT Pro/Plus (browser)
ChatGPT Pro/Plus (headless)
Manually enter API Key
```

The browser method:

- starts a local HTTP callback server;
- generates PKCE verifier/challenge;
- builds an OpenAI authorization URL;
- exchanges the callback code for tokens;
- extracts a ChatGPT account id from token claims when present;
- stores OAuth material through OpenCode's auth service.

The headless method:

- requests a ChatGPT/Codex device user code from `auth.openai.com`;
- tells the user to enter that code at the Codex device URL;
- polls the device token endpoint;
- exchanges the resulting authorization code for OAuth tokens;
- stores refresh/access/expiry/account id through OpenCode's auth service.

The plugin loader then:

- returns a dummy API key to satisfy provider plumbing;
- refreshes expired access tokens using the refresh token;
- persists refreshed OAuth material back into OpenCode auth;
- injects `Authorization: Bearer <access>` into OpenAI/Codex requests;
- injects `ChatGPT-Account-Id` if available;
- routes OpenAI responses/chat-completion requests to the Codex API endpoint.

This is very useful for runtime materialization. It is not sufficient as the
OpenAgents provider-account service because it stores raw OAuth material in
the local OpenCode auth store.

### OpenCode CLI Account Login

`packages/opencode/src/account/account.ts` and
`packages/opencode/src/cli/cmd/account.ts` implement a device flow for logging
an OpenCode CLI/runtime into an OpenCode account service, defaulting to:

```text
https://console.opencode.ai
```

The service calls:

```text
POST /auth/device/code
POST /auth/device/token
GET  /api/user
GET  /api/orgs
```

It persists account access/refresh tokens in a local SQLite table. This is
OpenCode console/account auth. It is not ChatGPT/Codex provider auth.

For OpenAgents, this can inspire service-device auth for SHC machines, but it
should not become the way we connect user ChatGPT accounts.

### OpenCode Server Auth

OpenCode server mode is opt-in. The security doc says server mode should be
protected with:

```text
OPENCODE_SERVER_PASSWORD
```

`packages/opencode/src/server/auth.ts` implements Basic Auth credentials from
`OPENCODE_SERVER_PASSWORD` and optional `OPENCODE_SERVER_USERNAME`.

The HTTP API middleware allows access when Basic Auth matches. If no password
is configured, the OpenCode server can run unauthenticated.

For OpenAgents, this means:

- every SHC/GCloud OpenCode server must have a per-run or per-runner server
  password;
- the OpenCode server should be reachable only through the runner/control
  network, not as the product's public web auth;
- the OpenAgents Worker should authenticate users/runners independently before
  any OpenCode server call;
- OpenCode Basic Auth is a runtime control-plane guard, not user identity.

## Comparison Matrix

| Surface                    | Vortex Today                                         | OpenCode Expectation                                                        | OpenAgents Target                                                         |
| -------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Human sign-in              | WorkOS/AuthKit                                       | OpenAuth issuer for OpenCode console                                        | OpenAuth issuer for OpenAgents                                            |
| User/account ledger        | Convex `users`, orgs, projects                       | OpenCode DB account/workspace/user tables                                   | D1 account/user/workspace/member tables                                   |
| ChatGPT account connection | Vortex provider-account routes plus broker           | OpenAI Codex plugin writes local provider OAuth                             | OpenAgents ProviderAccountService owns metadata, broker, refs, grants     |
| Credential material        | broker/secret ref, not Convex/browser                | `auth.json` or `OPENCODE_AUTH_CONTENT` can contain raw provider credentials | secret store/broker only; D1 stores refs only                             |
| Runtime grant              | `providerAccountAuthGrants` resolved by runner token | not a native OpenCode concept                                               | first-class D1 grant, short TTL, one action/run                           |
| Runner provider auth       | GCP/SHC materializes from secret ref                 | OpenCode expects provider auth JSON/env/file                                | grant resolver emits OpenCode-compatible auth material only inside runner |
| OpenCode server access     | Vortex launches lower-level workroom API             | Basic Auth via `OPENCODE_SERVER_PASSWORD`                                   | per-run runner/server token, never public product auth                    |
| Realtime state             | Convex live queries                                  | OpenCode local/TUI state and HTTP API                                       | OpenAgents Sync over DO WebSocket + D1 outbox                             |

## The Main Conceptual Trap

The word "account" means four different things here.

```text
OpenAgents account
  -> first-party user/account subject from OpenAuth

ChatGPT/Codex provider account
  -> user's third-party ChatGPT subscription/auth material

OpenCode console account
  -> account at console.opencode.ai or our equivalent console service

OpenCode local auth account/provider entry
  -> credential entry in auth.json / OPENCODE_AUTH_CONTENT
```

The migration must keep those identities linked but not collapsed.

The OpenAgents account may own several ChatGPT/Codex provider accounts. A
single ChatGPT/Codex provider account may produce many short-lived runner
grants. A runner grant may materialize one OpenCode local auth entry for one
run. The OpenCode server may additionally have Basic Auth so our Worker can
control it. None of that makes the ChatGPT account an OpenAuth identity.

## Recommended Target Architecture

### Authority Plane 1: OpenAuth

OpenAuth should authenticate and issue OpenAgents actors.

Recommended deployment:

```text
auth.openagents.com
  -> Cloudflare Worker
  -> OpenAuth issuer
  -> GitHub / Google / future email-code providers
  -> Cloudflare KV for issuer state
  -> D1 for OpenAgents account identity mapping
```

OpenCode used a separate Cloudflare Worker for OpenAuth at `auth.<domain>` and
linked it to KV plus the database. That shape is good.

For `openagents`, start with a separate Worker package in the same Bun
workspace:

```text
workers/auth/
  OpenAuth issuer
  provider callbacks
  subject schemas
  session/token issuance

packages/auth-schema/
  Effect Schema for subjects, sessions, actor claims

packages/auth-worker/
  D1 repositories and OpenAuth integration helpers
```

Do not start by creating a separate GitHub repo unless we need independent
deploy cadence across multiple products immediately. A separate service
boundary is required; a separate repository is optional. The safer first move
is one `openagents` monorepo with separate `workers/auth` and
`workers/api` deployments. Extract `openagentsinc/auth` later only when the
API has stabilized and multiple repos are consuming it.

OpenAuth should own:

- provider login callbacks;
- subject creation;
- session cookies or access tokens;
- actor claims;
- account identity mapping;
- workspace membership lookup;
- service-account and runner-token minting policy.

OpenAuth should not own:

- ChatGPT refresh tokens;
- OpenCode `auth.json`;
- provider-account grants;
- runner artifact storage;
- OpenAgents Sync state.

### Authority Plane 2: OpenAgents Provider Accounts

Provider accounts should live in the API/control worker domain, not the
OpenAuth issuer.

Recommended deployment:

```text
workers/api/
  /api/provider-accounts/*
  /api/sync/*
  /api/runners/*
  /api/workrooms/*

packages/provider-account-schema/
  Effect Schema for provider accounts, connection attempts, grants, events

packages/provider-account-worker/
  D1 repositories
  secret-reference policy
  broker adapter interfaces
  grant issuance/resolution
```

The ProviderAccountService should authenticate every request through OpenAuth
actor claims or a dedicated service/runner token minted from that identity
system.

It should own:

- provider-account records;
- connection attempts;
- device-login ceremony fields;
- broker callbacks;
- account health and capacity;
- public metadata;
- auth grants;
- grant resolution;
- revocation;
- audit events and receipts.

It should not be a generic OAuth provider. It is a product-specific authority
for third-party provider accounts used by OpenAgents workrooms.

### Authority Plane 3: OpenCode Runtime

OpenCode should run as the core agent runtime on SHC first, GCloud second.

Runtime sequence:

```text
1. User starts an OpenAgents workroom.
2. API verifies OpenAuth actor and workspace authorization.
3. API issues provider-account grant for selected ChatGPT/Codex account.
4. API dispatches SHC OpenCode runner.
5. Runner resolves grant with service/runner auth.
6. ProviderAccountService returns a secret reference or encrypted sealed secret.
7. Runner materializes an OpenCode-compatible provider auth payload.
8. Runner launches OpenCode with that payload.
9. Runner streams events back to OpenAgents.
10. Runner destroys local credential material at closeout.
```

OpenCode-compatible provider auth can be materialized as:

```text
OPENCODE_AUTH_CONTENT=<redacted provider auth JSON>
```

or as an ephemeral `auth.json` in an isolated per-run `OPENCODE_HOME` /
`XDG_DATA_HOME`. Prefer `OPENCODE_AUTH_CONTENT` when possible because the
credential never needs to be written to a durable home directory. If a file is
required, write it into a per-run encrypted/ephemeral directory and remove it
after closeout.

OpenCode server mode should be protected with a minted per-run
`OPENCODE_SERVER_PASSWORD`. That password is for the control plane's access to
the OpenCode process, not for the user's public session.

## How Much OpenCode Code Can We Reuse?

### Reuse Directly

OpenCode's runtime expects provider auth in a well-known shape. We should reuse
that expectation.

Useful direct runtime compatibility:

- `OPENCODE_AUTH_CONTENT`;
- `auth.json` provider entries when a file is needed;
- provider ids such as `openai`;
- OAuth auth shape with refresh/access/expires/account id;
- plugin loader behavior that injects bearer auth and `ChatGPT-Account-Id`;
- OpenCode HTTP API provider auth endpoints for local debugging;
- OpenCode server Basic Auth for protected control-plane access.

### Reuse As Reference, Not As Product Authority

The OpenAI Codex plugin contains the logic we care about:

- browser PKCE login;
- headless device login;
- token exchange;
- token refresh;
- account id extraction;
- Codex endpoint request adaptation.

But direct Worker import is not clean because the plugin module also imports
Node-specific pieces for the browser callback server and OS/user-agent
behavior. The headless method is mostly fetch/poll/token-exchange logic, but
the module as a whole is not a Cloudflare Worker-native adapter.

Recommended reuse:

```text
Use OpenCode's Codex plugin as the protocol reference.
Port the headless device-login/token-refresh pieces into an Effect service.
Keep the service API OpenAgents-specific.
Emit OpenCode-compatible auth only at runner materialization time.
```

Alternative for the first SHC slice:

```text
Run an OpenCode/Codex broker on SHC.
Have OpenAgents ProviderAccountService request device-login start/status
from that broker.
Store only codex-auth:// refs in D1.
Let SHC materialize local OpenCode auth from the broker ref at run time.
```

This is closest to Vortex's Codex App Server WebSocket bridge and should be
fastest to operational parity.

### Do Not Reuse As-Is

Do not reuse these as OpenAgents product authority:

- OpenCode console `AuthTable` as our account table;
- OpenCode console `ProviderTable.credentials` as our provider-account store;
- OpenCode CLI account login as ChatGPT provider-account login;
- OpenCode server Basic Auth as browser/user auth;
- OpenCode local `auth.json` as the central multi-tenant secret store.

Those are valid in OpenCode's product/runtime boundary. They are too broad or
too local for OpenAgents provider-account authority.

## Cloudflare-Only Data Model

The D1 model should preserve the Vortex semantics but make actors, scopes,
connection attempts, grants, and secret references first-class.

### Identity Tables

```sql
CREATE TABLE accounts (
  id TEXT PRIMARY KEY,
  primary_email TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE auth_identities (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  provider_subject TEXT NOT NULL,
  email TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  UNIQUE(provider, provider_subject)
);

CREATE TABLE workspaces (
  id TEXT PRIMARY KEY,
  slug TEXT,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE workspace_memberships (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  role TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(workspace_id, account_id)
);
```

These are OpenAgents records derived from OpenAuth, not ChatGPT records.

### Provider Account Tables

```sql
CREATE TABLE provider_accounts (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  workspace_id TEXT,
  provider TEXT NOT NULL,
  auth_mode TEXT NOT NULL,
  status TEXT NOT NULL,
  health TEXT NOT NULL,
  provider_account_ref TEXT NOT NULL UNIQUE,
  secret_ref TEXT,
  account_label TEXT,
  plan_type TEXT,
  connected_at TEXT,
  disconnected_at TEXT,
  denied_at TEXT,
  last_status_at TEXT NOT NULL,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX provider_accounts_account_provider_idx
  ON provider_accounts(account_id, provider);

CREATE INDEX provider_accounts_workspace_provider_idx
  ON provider_accounts(workspace_id, provider);
```

`secret_ref` must be a stable reference, not credential JSON.

### Connection Attempt Tables

Separate attempts from accounts. Vortex currently stores pending ceremony
fields directly on `providerAccounts`; that works but makes retries and
history harder.

```sql
CREATE TABLE provider_account_connection_attempts (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  workspace_id TEXT,
  provider TEXT NOT NULL,
  method TEXT NOT NULL,
  source TEXT NOT NULL,
  login_ref TEXT,
  verification_url TEXT,
  user_code TEXT,
  status TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  completed_at TEXT,
  failed_at TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX provider_connection_attempts_account_idx
  ON provider_account_connection_attempts(account_id, created_at);

CREATE INDEX provider_connection_attempts_provider_account_idx
  ON provider_account_connection_attempts(provider_account_id, created_at);
```

This lets OpenAgents show current pending code, previous denied/expired
attempts, and broker failures without mutating the account row for every
attempt detail.

### Grant Tables

```sql
CREATE TABLE provider_account_auth_grants (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  workspace_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  provider TEXT NOT NULL,
  provider_account_ref TEXT NOT NULL,
  provider_secret_ref TEXT NOT NULL,
  grant_ref TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL,
  requested_action TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  revoked_at TEXT,
  failed_at TEXT
);

CREATE INDEX provider_grants_account_created_idx
  ON provider_account_auth_grants(account_id, created_at);

CREATE INDEX provider_grants_runner_session_idx
  ON provider_account_auth_grants(runner_session_id);

CREATE INDEX provider_grants_status_expiry_idx
  ON provider_account_auth_grants(status, expires_at);
```

Grants should be:

- short-lived;
- scoped to one action/run where possible;
- idempotently resolvable;
- marked used when a runner consumes them;
- revoked on account disconnect;
- excluded from OpenAgents Sync except for public status/projection.

### Event Tables

```sql
CREATE TABLE provider_account_events (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT,
  auth_grant_id TEXT,
  account_id TEXT NOT NULL,
  workspace_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  kind TEXT NOT NULL,
  summary TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  target_ref TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX provider_account_events_account_created_idx
  ON provider_account_events(account_id, created_at);

CREATE INDEX provider_account_events_target_idx
  ON provider_account_events(target_ref);
```

Events must pass the same secret-material detector as Vortex.

### Runner Session Tables

```sql
CREATE TABLE runner_sessions (
  id TEXT PRIMARY KEY,
  runner_id TEXT NOT NULL,
  lane TEXT NOT NULL,
  backend TEXT NOT NULL,
  status TEXT NOT NULL,
  workspace_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  provider_account_ref TEXT,
  active_auth_grant_ref TEXT,
  opencode_server_url TEXT,
  opencode_server_auth_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT
);

CREATE INDEX runner_sessions_thread_idx
  ON runner_sessions(thread_id, created_at);
```

`opencode_server_auth_ref` should be a reference to the per-run Basic Auth
secret, not the password.

## Cloudflare Runtime Design

### Workers

Use two deployed Workers:

```text
auth.openagents.com
  workers/auth
  OpenAuth issuer
  login callbacks
  subject/session endpoints

openagents.com
  workers/api
  product API
  provider-account routes
  sync routes
  runner routes
  static assets
```

The current `workers/api` route split already distinguishes `/api/*` from
browser routes. The new auth worker should be added beside it rather than
mixed into every API handler.

### D1

D1 is authoritative for product metadata:

- account identity mapping;
- workspace membership;
- provider-account metadata;
- connection attempt records;
- public refs;
- grants;
- events;
- runner sessions;
- sync outbox.

D1 is not a credential vault.

### KV

KV is appropriate for OpenAuth issuer state, matching OpenCode's
CloudflareStorage usage. Do not use KV as the source of truth for
provider-account metadata. KV is also not the right place for raw long-lived
ChatGPT refresh tokens.

### Secrets

Credential material should be stored behind a secret reference. Concrete
options:

- SHC broker-managed encrypted account home;
- Cloudflare Secrets Store when available for this path;
- encrypted R2 object with KMS/wrapping-key discipline;
- external KMS-backed vault if Cloudflare-native secret storage is not
  sufficient.

The invariant is more important than the first backend:

```text
D1 stores refs.
OpenAgents Sync streams refs/status only.
Runner materializes raw credentials only after grant resolution.
Raw credentials are never committed, logged, synced, or rendered.
```

### Durable Objects

Durable Objects should coordinate:

- live connection attempt state;
- polling and fanout for account connection status;
- active workroom sync rooms;
- runner heartbeats and grant-use transitions.

Do not keep raw provider tokens in hibernating WebSocket attachments or
Durable Object public state.

### Queues

Queues should handle:

- broker callbacks;
- account health checks;
- grant expiry cleanup;
- runner event ingest;
- redaction jobs.

### Workflows

Workflows should handle:

- long-running device-login repair flows;
- run closeout;
- approval wait/resume;
- account health recheck schedules;
- migration/backfill jobs.

## API Shape

### Browser/User Routes

```text
GET  /api/provider-accounts
POST /api/provider-accounts/chatgpt-codex/device-login/start
GET  /api/provider-accounts/chatgpt-codex/device-login/:attemptId
POST /api/provider-accounts/:providerAccountRef/disconnect
POST /api/provider-accounts/:providerAccountRef/grants
```

Browser routes authenticate with OpenAuth. They return only public metadata
and public ceremony fields.

### Broker/Operator Routes

The operator bridge should become service-actor auth, not a static
environment bearer token.

```text
POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/connected
POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/failed
POST /api/provider-accounts/:providerAccountRef/health
```

The broker authenticates as a service actor minted by OpenAuth or by a
runner/service-token issuer owned by OpenAuth.

### Runner Routes

```text
POST /api/provider-accounts/chatgpt-codex/grants/resolve
POST /api/provider-accounts/chatgpt-codex/grants/:grantRef/used
POST /api/runners/:runnerSessionId/events
POST /api/runners/:runnerSessionId/complete
```

Runner routes authenticate with service/runner credentials. Runner credentials
should be scoped to a runner session or SHC node, not reusable global static
tokens.

## Sync Projection Rules

OpenAgents Sync should stream provider-account public state, not credential
state.

Allowed collections:

```text
provider_accounts_public
provider_connection_attempts_public
provider_account_events_public
provider_account_grants_public
runner_sessions_public
```

Forbidden fields in sync patches:

- access token;
- refresh token;
- id token;
- device auth id;
- code verifier;
- authorization code;
- API key;
- Basic Auth password;
- `OPENCODE_AUTH_CONTENT`;
- `auth.json`;
- raw secret-store payloads.

Every provider-account sync projection should be produced by an explicit
projection function that runs the Vortex-derived secret detector before
appending to `sync_changes`.

## How OpenAuth Should Relate

OpenAuth answers:

```text
Who is this OpenAgents actor?
Which account/workspace/service does it represent?
Which roles and scopes can it claim?
```

ProviderAccountService answers:

```text
Which ChatGPT/Codex provider accounts has this OpenAgents actor connected?
Which provider-account refs are healthy and usable?
Can this actor issue a grant for this workroom?
Which server-side secret ref backs this grant?
```

OpenCode runtime answers:

```text
Given a grant and a workroom, can I run the agent?
Can I turn the secret ref into OpenCode-compatible runtime auth?
Can I stream events, artifacts, and closeout status back?
```

OpenAuth should be called before provider-account operations, but it should
not store provider credentials. The provider-account service should record the
OpenAuth actor id in account/grant/event rows for auditability.

OpenAuth should also issue or validate service actors:

```text
actor:account:acc_...
actor:user:user_...
actor:workspace-member:...
actor:runner:shc-node-...
actor:broker:codex-login-broker-...
```

That replaces today's static operator and cloud-runner bearer tokens.

## How OpenCode Should Relate

OpenCode should be treated as the core execution engine, not the product
authority.

### Provider Auth Materialization

When a runner resolves a provider-account grant, it should receive enough
authority to materialize an OpenCode provider auth payload locally.

The payload shape is OpenCode-compatible, but the source of truth remains
OpenAgents:

```json
{
  "openai": {
    "type": "oauth",
    "refresh": "<redacted>",
    "access": "<redacted-or-empty-if-refresh-first>",
    "expires": 0,
    "accountId": "<chatgpt-account-id-if-known>"
  }
}
```

This JSON must not be stored in D1, sync changes, logs, docs, or browser state.
It is a transient runtime payload.

OpenCode then uses its own provider/plugin machinery to refresh access tokens,
set authorization headers, and call the Codex API. That is the code we want to
reuse.

### OpenCode Server Control

If the runner starts `opencode serve` or `opencode web`, the control plane
must set:

```text
OPENCODE_SERVER_PASSWORD=<per-run secret>
OPENCODE_SERVER_USERNAME=opencode or openagents-runner
```

The OpenAgents Worker should store only:

```text
opencode_server_auth_ref
```

and the runner should expose the server only over the SHC control network or a
short-lived tunnel authorized by OpenAgents.

### OpenCode Console Account

Do not require `opencode account login` to `console.opencode.ai` for every
OpenAgents run. That flow is for OpenCode's own console/account service. It
may be useful if we later run a first-party OpenAgents fork of the OpenCode
console, but it is not needed for ChatGPT/Codex provider auth.

## Migration Plan From Vortex

### Phase 0: Preserve The Boundary

Before changing code, declare these invariants in the new implementation:

- provider-account metadata may contain refs and public status only;
- raw credential material cannot enter D1;
- raw credential material cannot enter OpenAgents Sync;
- raw credential material cannot enter logs, receipts, artifacts, docs, or
  screenshots;
- a workroom runner needs a short-lived grant before materializing provider
  auth;
- disconnect revokes outstanding grants;
- runner grant resolution is authenticated as a service/runner actor.

### Phase 1: Port Policy

Move the Vortex secret-material detector into an Effect-first package:

```text
packages/provider-account-schema/
  PublicSecretRef
  ProviderAccountRef
  ProviderAccountStatus
  ProviderAccountHealth
  ProviderAccountEvent
  ProviderAccountGrant
  containsProviderSecretMaterial
  sanitizeProviderAccountText
  requirePublicSecretReference
```

Use branded Effect Schema types rather than plain strings.

### Phase 2: Add OpenAuth Worker

Add:

```text
workers/auth/
packages/auth-schema/
packages/auth-worker/
```

OpenCode's console auth worker is a strong template:

- OpenAuth issuer;
- GitHub provider;
- Google OIDC provider;
- Cloudflare KV storage;
- D1 account identity mapping;
- account subject returned on success.

For OpenAgents, remove the non-production email restriction from OpenCode's
template and implement OpenAgents account/workspace bootstrapping.

### Phase 3: Add Provider Account D1 Tables

Apply D1 migrations for:

- provider accounts;
- connection attempts;
- auth grants;
- provider account events;
- runner sessions.

Do not backfill any raw secret material. Backfill only public metadata and
refs.

### Phase 4: Implement Connection Start

Recreate Vortex:

```text
POST /api/provider-accounts/chatgpt-codex/device-login/start
```

but authenticate with OpenAuth instead of WorkOS/AuthKit.

The first implementation can call the SHC broker just as Vortex calls Codex
App Server. The more Cloudflare-native implementation can port OpenCode's
headless device-code protocol into an Effect service if the secret backend is
ready.

### Phase 5: Implement Broker Status

Replace the operator token with a service actor.

The broker should call:

```text
POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/connected
```

with:

- account label/email if public;
- plan type if known;
- provider account ref;
- server-side secret ref;
- broker id;
- no raw credential material.

### Phase 6: Implement Grant Issue/Resolve

Browser/API:

```text
POST /api/provider-accounts/:providerAccountRef/grants
```

Runner:

```text
POST /api/provider-accounts/chatgpt-codex/grants/resolve
```

Resolution should mark the grant `used` or record a runner-session use event.
It should return a sealed secret or secret ref to the runner, not to the
browser.

### Phase 7: Materialize OpenCode Runtime Auth

Add an SHC runner adapter:

```text
resolve grant
  -> fetch/decrypt secret payload inside runner
  -> create OPENCODE_AUTH_CONTENT
  -> set OPENCODE_SERVER_PASSWORD
  -> start OpenCode
  -> stream events to OpenAgents
  -> wipe auth material at closeout
```

The GCloud fallback should use the same grant and event envelope. Only the
launcher backend differs.

### Phase 8: Replace Vortex UI Projection

Foldkit should receive provider account state through OpenAgents Sync:

```text
ProviderAccountsSnapshotLoaded
ProviderAccountPatchReceived
ProviderLoginStarted
ProviderLoginConnected
ProviderGrantIssued
ProviderAccountDisconnected
```

The Model should not contain raw credentials. It may contain:

- account label;
- public status;
- health;
- plan type;
- provider account ref;
- pending verification URL;
- user code;
- expiry;
- grant public status.

## Testing Requirements

### Unit Tests

- secret detector rejects raw token-shaped values;
- public refs parse and normalize;
- provider account public projection redacts forbidden fields;
- grant issue rejects disconnected/unhealthy accounts;
- disconnect revokes issued grants;
- expired pending connection attempts become expired public status;
- OpenAuth actor without workspace role cannot issue workspace-scoped grant;
- runner actor cannot resolve another runner/session's grant.

### Integration Tests

- OpenAuth sign-in subject maps to D1 account and workspace membership;
- device-login start writes account + attempt + public sync patch;
- broker connected callback writes `secret_ref`, clears pending ceremony, and
  appends sync patch;
- browser grant issuance returns only `grantRef`;
- runner grant resolve returns only secret ref/sealed secret, never public
  account payload;
- SHC OpenCode runner can boot from `OPENCODE_AUTH_CONTENT`;
- OpenCode server refuses HTTP API access without Basic Auth when password is
  set;
- run closeout wipes local auth material.

### Static/Policy Tests

- no D1 migrations include columns named `access_token`, `refresh_token`,
  `id_token`, `code_verifier`, or `auth_json` in provider-account tables;
- no sync schema exposes credential fields;
- provider-account events and receipts fail if metadata contains secret-shaped
  material;
- logs redact authorization headers and OpenCode auth payloads.

## Risks

### Risk: OpenAuth Becomes A Credential Dump

OpenAuth is tempting because it already handles OAuth-ish things. Do not put
ChatGPT/Codex credential material there. Keep OpenAuth focused on OpenAgents
identity.

### Risk: OpenCode Local Auth Becomes The Product Ledger

OpenCode's `auth.json` is local runtime credential state. It is not a
multi-tenant account ledger, does not model workspace authorization, and does
not produce OpenAgents receipts/grants.

### Risk: Static Bearer Tokens Stay Forever

Vortex currently has static operator/runner bearer tokens. In
`openagents`, those should become service actors or runner-session tokens
issued and audited through the OpenAuth/OpenAgents identity layer.

### Risk: Raw Tokens Leak Through Sync

Provider account updates will naturally be streamed to the UI. Every public
projection must be generated through a redaction/sanitization function before
entering `sync_changes`.

### Risk: Browser Device Flow Is Not Worker-Native

OpenCode's browser PKCE method starts a local HTTP server, so it is naturally
Node/Bun runtime code, not Cloudflare Worker code. Use the headless/device
method for Worker-compatible connection starts or delegate the login ceremony
to an SHC broker.

### Risk: Multiple ChatGPT Accounts Collide

The Vortex docs already warn that Codex CLI/App Server homes are effectively
single-account unless account-scoped homes are used. The SHC runner must keep
per-account and per-run homes isolated. Do not login multiple ChatGPT accounts
into the same durable OpenCode/Codex home.

### Risk: Token Refresh Races

OpenCode's Codex plugin refreshes OAuth material and writes it back to local
auth. In OpenAgents, refresh must update the broker/secret backend, not just a
runner-local file, if we expect later runs to keep working. Decide whether
refresh is:

- broker-owned and durable; or
- per-run only, requiring a fresh broker secret on next run.

Broker-owned durable refresh is better for account fleet stability.

## Recommended First Slice

Build one end-to-end account-backed run:

```text
OpenAuth user signs in
  -> starts ChatGPT/Codex device-login attempt
  -> SHC broker completes and records codex-auth:// secret ref
  -> user starts a thread workroom
  -> API issues provider-account grant
  -> SHC OpenCode runner resolves grant
  -> runner materializes OPENCODE_AUTH_CONTENT
  -> OpenCode runs one task
  -> events stream through OpenAgents Sync
  -> runner closeout wipes local auth
```

Keep GCloud as the backup runner lane using the same grant resolver and event
envelope.

Do not begin by wiring the whole OpenCode console account system. The first
slice needs OpenCode provider auth compatibility, not OpenCode console
accounts.

## Final Recommendation

Use OpenCode as core agent runtime and reuse its provider-auth expectations.
Use OpenAuth as OpenAgents identity. Keep ChatGPT/Codex provider-account
connection as an OpenAgents-owned service.

The Vortex provider-account design should be ported, not discarded:

```text
Vortex WorkOS/AuthKit
  -> OpenAuth account/session actor

Vortex Convex providerAccounts
  -> D1 provider_accounts + connection_attempts

Vortex providerAccountAuthGrants
  -> D1 short-lived provider_account_auth_grants

Vortex operator bearer bridge
  -> OpenAuth/service actor broker auth

Vortex cloud runner grant token
  -> runner-session service auth

Vortex Codex App Server WebSocket broker
  -> SHC OpenCode/Codex broker first, Worker-native Effect adapter later

OpenCode auth.json
  -> transient runtime materialization from grant, not product source of truth
```

That gives us a Cloudflare-native product authority, an Effect-first schema
and policy layer, OpenAuth identity, and OpenCode runtime compatibility without
turning any one auth mechanism into the wrong system boundary.
