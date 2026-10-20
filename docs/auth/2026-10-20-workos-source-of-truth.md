# WorkOS as the source of truth for auth

Date: 2026-10-20. Status: W0 to W3 built, behind the `OPENAGENTS_AUTH`
switch; W4 open. Planned at repository revision `fd70af1f17`; see
[What's built](#whats-built) for the code.

Owner request:

> Keep the nostr-centricness as much as we can, but I want to reason about
> one auth object.

This plan makes one WorkOS User the auth object for a person. Nostr keys stay
the way people sign in and sign things. Our servers verify the Nostr
signature, and WorkOS holds the person, their organizations, their roles, and
their sessions. WorkOS never holds a private key.

The plan supersedes the owner direction of 2026-10-08 recorded in
[the auth README](README.md) ("own it ourselves, in Rust; no third-party auth
service in the path"). The owner confirmed decision 1 on 2026-10-20: WorkOS
is the auth object for people. The decisions as made are under
[Owner decisions](#owner-decisions).

Epics, one per phase, all on the OpenAgents board (project 19):

| Phase | Epic | Work |
| --- | --- | --- |
| W0 | [#12320](https://github.com/OpenAgentsInc/openagents/issues/12320) | Foundation: staging environment, session-bridge answer from WorkOS, client, link table, mode switch |
| W1 | [#12321](https://github.com/OpenAgentsInc/openagents/issues/12321) | Dual-write accounts, keys, workspaces, and memberships |
| W2 | [#12322](https://github.com/OpenAgentsInc/openagents/issues/12322) | Backfill existing accounts with stable account IDs |
| W3 | [#12323](https://github.com/OpenAgentsInc/openagents/issues/12323) | Cut sign-in over to WorkOS sessions |
| W4 | [#12324](https://github.com/OpenAgentsInc/openagents/issues/12324) | Retire the old session store for people |

## Summary

- **The key question.** WorkOS has no documented API that turns "we verified
  this person ourselves" into a User Management session. Every grant on
  `POST /user_management/authenticate` needs something WorkOS issued (a
  code, a password, a refresh token, or a challenge). Impersonation starts
  only from the WorkOS dashboard. Standalone Connect lets our login page
  stand in for AuthKit, but only for OAuth applications built on Connect, not
  for AuthKit's own sessions.
- **The working path.** With WorkOS's default emails turned off, our server
  can create a Magic Auth code for the User (`POST
  /user_management/magic_auth` returns the plaintext `code`) and exchange it
  at once (`grant_type=urn:workos:oauth:grant-type:magic-auth:code`). The
  result is a real WorkOS session: access token, refresh token, and `sid`.
  This uses documented calls in an order WorkOS doesn't describe, so W0
  gets WorkOS's written answer before anything ships.
- **The fallback** if WorkOS says no: keep minting our own `sess_` for a
  Nostr sign-in, bound to the WorkOS User ID. The User is still the one auth
  object; only the session record stays ours for key sign-ins.
- **What stays Nostr.** Event signing, NIP-05 names, NIP-OA owner tags,
  NIP-HOST device grants, room tokens, browser wallet grants, and agent
  keys. Each one is checked offline by a host, the relay, or `coder-sync`,
  and none of them needs WorkOS.
- **What stays ours.** `oak_` API keys (quota, door registry, receipts),
  `oa_sk_` sign-in keys for agents, the anonymous visitor lane, and the
  NIP-98 replay book.
- **Cost.** AuthKit is free up to 1 million monthly active users. Only a
  custom domain ($99 a month) is needed for passkeys.
- **Size.** About 3 to 4 weeks of focused agent work across the five
  phases, and the slowest part is waiting on app releases that store
  refresh tokens.

## Today's auth objects and where they live

### Principals

| Principal | ID or token | Created by | Stored in | Code |
| --- | --- | --- | --- | --- |
| Person (account) | `acct_<16 hex>` | First sign-in | Account service: Postgres `identity.accounts`, or `accounts.json` | `crates/tenancy/src/accounts.rs:143-157`, minted `:1359` |
| Linked credential | `nostr:<64 hex>`, `key:<16 hex>`, `github:<id>`, `sso:<64 hex>`, `apple:<sub>` | Sign-in or link | `identity.principals` | `valid_principal`, `accounts.rs:988-1008` |
| Workspace | `ws_<16 hex>`, personal or organization | Sign-up, `POST /v1/workspaces` | `workspace.workspaces` | `accounts.rs:188-212`, minted `:1421` |
| Membership and role | Member, Admin, Owner | Invitation `inv_<id>.<secret>` | `workspace.memberships`, `invitations` | `accounts.rs:93-103`, `:235-257` |
| Browser session | `sess_<64 hex>`, 8 hours, cookie `oa_cloud_session` | Every web sign-in | `identity.account_sessions` (digest only) | `tenancy/src/sessions.rs:150-193`, `gateway/src/config.rs:905` |
| App session | `sess_<64 hex>`, 30 days, no refresh | Key sign-in, device code | Same | `tenancy/src/sessions/device.rs:43` |
| Sign-in key (agents) | `oa_sk_<64 hex>`, never, 30, or 90 days | `openagents keys create` | Same book, 50 per account | `tenancy/src/sessions/keys.rs:23,34` |
| API key | `oak_<id>.<secret>`, until revoked | Settings, `openagents keys --kind api` | `identity.bearer_keys` or `keys.json` | `tenancy/src/keys.rs:3,35-67` |
| Device-code request | `dvc_…`, 600 seconds | Coder 1.0.0, CLI | `identity.device_sign_ins` | `tenancy/src/sessions/device.rs:36,53` |
| Recovery token | `rcv_<hex>`, single use | Workspace owner | `identity.recoveries` | `tenancy/src/sessions.rs:777-860` |
| Visitor (web) | `oa_visitor` cookie, 32 hex, one year | First visit | Not stored; chats keyed on it | `openagents-web/src/ask.rs:17,132-150` |
| Visitor (app) | `ovis_<visitor><hmac>` | `POST /v1/visitors` | Not stored (HMAC) | `openagents-web/src/visitor_api.rs:46-80` |
| Anonymous session | `sess_`, kind `anonymous`, budget `onb_public` | `POST /v1/sessions` with no credential | Session book | `gateway/src/accounts.rs:41,641-648` |
| Self-registered agent | `acct_` with `agent: true`, an `oak_` key | `/v1/agents/challenge`, `/register` (proof of work, claim code) | Account store | `gateway/src/agent_accounts.rs:72-78` |
| Published agent | Nostr key derived from `OPENAGENTS_WEB_AGENT_KEY_SEED`, NIP-OA owner tag | `PUT /v1/published-agents/{id}/owner` | Chat store, `published-agents/keys/{pubkey}.json` | `openagents-web/src/published_agents/owner.rs:76,362-400` |
| Device (host grant) | `openagents.host-grant.v1`, 7 rights, 30 days, revocation epoch | NIP-HOST pairing, QR | Host `access.json`; `coder-sync` `device_grants` | `coder-access/src/rights.rs:7`, `nips/openagents/NIP-HOST.md:66-101` |
| Browser (paired) | Browser grant, observe and operate, 7 days | Desktop **Settings > Devices** | Same, key in browser IndexedDB | `coder-access/src/browser.rs:21-23`, `host/nearby.rs:60-88` |
| Room access | `ort1_` + signed kind 27236 event, 300 seconds | `POST /v1/sync/room-tokens` (#12263) | Not stored | `openagents-web/src/room_tokens.rs:169-253`, `coder-access/src/room_token.rs` |
| Browser wallet grant | `openagents.spend-*.v1` | Phone approval (#12258) | Chat store, `<owner>/browser-spend.json` | `openagents-web/src/wallet_browser.rs:84-104` |
| Service | Land integrator `acct_711fc0446cf0b234`, an `oa_sk_` key | Key sign-in with proof of work (#12251) | Secret Manager `dev-openagents-app-token` | `docs/cloud/land-queue.md:239-259` |
| Enterprise SSO | `sso:<64 hex>` | Workspace OIDC (gateway only) | `identity.stores.rest` | `gateway/src/sso.rs:27-33` |
| NIP-05 name | `name@openagents.com` | `/v1/account/name` (#12254) | `Store.names` in `identity.stores.rest` | `tenancy/src/accounts/names.rs`, `openagents-web/src/profiles.rs:68-90` |

### Sign-in paths

| Path | Route | Code | State |
| --- | --- | --- | --- |
| Nostr key (NIP-98 kind 27235 over a stateless HMAC challenge) | `GET /v1/sessions/nostr/challenge`, `POST /v1/sessions/nostr` | `oa-auth/src/nostr.rs:335-344` (challenge), `:390-440` (verify), `:464-608` (session) | Built (#11993) |
| Key gate for new accounts | App Attest, Play Integrity, proof of work | `gateway/src/nostr_sign_in.rs:1-26`, `gateway/src/key_accounts.rs` | Built (#11994) |
| Website by phone QR or NIP-07 | `/login/key`, `/login/key/extension`, `/v1/web-sign-in/approve` | `openagents-web/src/web_sign_in.rs:342,534,610` | Built (#12001) |
| App key sign-in | `/v1/device/nostr`, `/v1/device/keys` | `openagents-web/src/device_nostr.rs`, `apps/openagents/crates/account/src/key_session.rs:1-32` | Built |
| Device code (RFC 8628) | `/v1/device/code`, `/v1/device/token`, `/device` | `oa-auth/src/device.rs`, `openagents-web/src/device.rs` | Built; stays for Coder 1.0.0 |
| GitHub OAuth with PKCE | `/auth/github`, `POST /v1/sessions/github` | `openagents-web/src/auth.rs:31-37`, `gateway/src/accounts.rs:821` | Old accounts only (#12005) |
| `oak_` key to session | `POST /v1/sessions`, `/sign-in` form | `gateway/src/accounts.rs:646` | Built |
| OAuth 2.1 for MCP clients | `/oauth/*` | `openagents-web/src/oauth.rs` | Built |
| Apple | `/v1/sessions/apple` | `gateway/src/accounts.rs:861` | `410` since #12005 |
| Email or magic link | None | — | Never built |

### Request authentication

| Surface | What it accepts | Code |
| --- | --- | --- |
| Gateway | `sess_`, `oa_sk_`, or `oak_` mapped to `key:<id>` | `gateway/src/accounts.rs:225,322,349` |
| Web pages | `oa_cloud_session` cookie, re-read from the account service per request | `openagents-web/src/cloud/session.rs:14,597,616,1142-1147` |
| Web `/v1` with a cookie (#12262) | Cookie rewritten to `Bearer`, same-origin only; writes need `x-openagents-csrf` | `openagents-web/src/cookie_api.rs:46-180` |
| `coder-sync` | `sess_`, `oa_sk_` (30-second cache), `ort1_` room tokens | `openagents-web/src/coder_sync.rs:1443`, `coder-sync/src/server/auth.rs:148,195-205` |
| CSRF | HMAC tickets bound to the session digest, 600 seconds; chat HMAC over the owner | `openagents-web/src/cloud/session.rs:791-1003`, `pages/chat.rs:172-230` |
| Test runs | `X-OpenAgents-Test: 1` hides and sweeps; it authenticates nothing | `openagents-web/src/test_runs.rs:1-43` |
| Gateway dashboard | Its own cookie `oa_session` holding a pasted `sess_` | `gateway/src/dashboard.rs:36` |

### Client credential stores

| Client | Identity | Session | Code |
| --- | --- | --- | --- |
| Phone | 12 words, Keychain `com.openagents.app.identity` / `words-v1`, NIP-06 key | Keychain `com.openagents.app.account` | `apps/openagents/crates/account/src/identity.rs:406,426`, `mobile/src/client_ffi/account.rs:562` |
| Desktop | Keychain `com.openagents.desktop[.dev].identity` (0600 files on Linux) | Keychain `com.openagents.desktop.account` | `account/src/desktop.rs:105-217` |
| Desktop host | Host, owner, and host-iroh keys, Keychain `com.openagents.desktop` | NIP-HOST grants in `access.json` | `coder-host/src/serve/keys.rs:17-28` |
| CLI and Coder | None | `~/.openagents/coder/account.json` (0600) or `OPENAGENTS_TOKEN` | `openagents-login/src/lib.rs:24-63,253-262` |
| Per-feature keys | Push, sharing, computers, chat, Verse | `push.json`, several `device.key` files | Listed in the [identity audit](../audits/2026-10-17-nostr-only-identity.md) |

### How many sources of truth

Nine places hold identity or session truth today:

| # | Store | Kind |
| --- | --- | --- |
| 1 | Account service: accounts, principals, sessions, keys, GitHub access | Postgres (files without a database); production and staging separate |
| 2 | Billing | `billing.json` on NFS |
| 3 | App Attest book and Nostr challenge key | `app-attest.json`, `nostr-challenge.key` on NFS |
| 4 | `coder-sync`: device host keys, grants, spent proofs | `coder-sync` Postgres |
| 5 | NIP-HOST access store and host keys | Each computer's `access.json` and Keychain |
| 6 | App Keychain items | Phone and desktop |
| 7 | CLI login | `account.json` or `OPENAGENTS_TOKEN` |
| 8 | Per-feature device keys | Files on each machine |
| 9 | Operator secrets | Secret Manager, `~/work/.secrets` |

Stores 1 to 4 are authoritative. Of those, only store 1 decides *who a person
is*; that is the store this plan moves to WorkOS. Store 4 and store 5 decide
*what a device may do*, and they stay Nostr-native. The web server holds no
account data of its own; it forwards proof to the account service
(`openagents-web/src/web_sign_in.rs:287`).

## The target model

### One WorkOS User per person

| Concept | Today | Target |
| --- | --- | --- |
| Person | `acct_<16 hex>` | WorkOS User `user_…` with `external_id = acct_<16 hex>`. The account ID never changes, and every store keeps keying on it. |
| Email | Optional, GitHub only | Required by WorkOS and unique. Key-only accounts get `acct_<16 hex>@id.openagents.com`, marked verified; nothing is ever sent there. A real address replaces it only when the person adds one (owner decision 3). |
| Nostr keys | `nostr:<hex>` principals | Linked, verified identities in our table, keyed by account ID and WorkOS User ID |
| Workspace | `ws_…` | WorkOS Organization with `external_id = ws_…` for organization workspaces; personal workspaces per owner decision 5 |
| Role | Member, Admin, Owner | WorkOS role slugs `member`, `admin`, `owner` on the organization membership |
| Session | `sess_` in our book | WorkOS session: a sealed cookie on the web, access and refresh tokens for apps and the API |
| Device | NIP-HOST grant | Unchanged; the grant names the owner's npub, which our table maps to the User |
| Agent | `acct_` with `agent: true`, or a published agent key | A principal owned by the User, in our table; not a WorkOS User |
| Visitor | `oa_visitor`, `ovis_`, `onb_public` | Unchanged; no WorkOS User until the first key sign-in |
| Service | An ordinary account with an `oa_sk_` key | A WorkOS User like a person (the land integrator signs in with its key) |

### Where Nostr keys live: recommendation

Keep the links in **our own table, keyed by the WorkOS User ID**. Put only the
primary npub in WorkOS metadata, for support and display.

| Option | For | Against |
| --- | --- | --- |
| WorkOS `external_id` | Unique, searchable (`GET /user_management/users/external_id/{id}`) | One per User; we need it for `acct_`, and a person has several keys |
| WorkOS user metadata | Visible in the dashboard | 10 keys, 600 ASCII characters each, no lookup by value, not in access tokens without a JWT template, and WorkOS says it isn't for authorization |
| **Our table (recommended)** | Unique index on the pubkey; one person, many keys; the sign-in path reads it without calling WorkOS; works offline from WorkOS | One more table, which `identity.principals` already is |

Concretely, `identity.principals` stays as it is, and a new
`identity.workos_users (account_id PRIMARY KEY, workos_user_id UNIQUE,
linked_at)` joins the two IDs. Metadata `npub` on the User holds the primary
key in bech32 (63 characters). The npub is public, so this discloses nothing.

### What stays Nostr-native

| Piece | Why it stays | Change |
| --- | --- | --- |
| Event signing (kind 0, 3, NIP-17, NIP-29, NIP-MV, receipts) | Signed on the device; WorkOS has no role | None |
| NIP-05 `name@openagents.com` | Served from our name book at `/.well-known/nostr.json` | None; the name book keys on the account |
| NIP-OA owner tags | The tag is a signature by the owner's key; any client checks it | None |
| NIP-HOST device grants and browser grants | Hosts and `coder-sync` check them offline, with revocation epochs | None |
| Room tokens `ort1_` | Short-lived signed events checked by `coder-sync` | Minted from the WorkOS session instead of `sess_` |
| Browser wallet grants | Spend limits signed by the phone | None |
| Agent keys | Agents sign as themselves | None |
| Relay NIP-42 | Relay admission | None |

**WorkOS never holds a private key, the 12 words, or the wallet seed.** Don't
use WorkOS Vault for them: Vault is server-side envelope encryption, and the
whole point of the 12 words is that only the person's devices hold them.

## Sign-in flows

### Nostr key sign-in

The answer to the key question: **no WorkOS API creates a session for a user
we verified ourselves.** Here is what each candidate does.

| Candidate | What the docs say | Fit |
| --- | --- | --- |
| `POST /user_management/authenticate` | Grants: `authorization_code`, `password`, Magic Auth code, email verification code, organization selection, TOTP, device code, Radar challenges, `refresh_token`. All need something WorkOS issued. | No custom grant |
| Impersonation | Started from the WorkOS dashboard only; no API | No |
| Standalone Connect, `POST /authkit/oauth2/complete` | Our page authenticates the person and AuthKit finishes an OAuth flow for a **Connect OAuth application**. It upserts the User by `id` (as `external_id`) and `email`. One Login URI per environment. | Gives Connect OAuth tokens to a client that started at `/oauth2/authorize`; not an AuthKit session for our own site |
| Agent Registration and Agent Blueprints | Identity assertions and minted agent tokens; early access since 2026-09-02 | For agents, not people |
| **Magic Auth bridge** | `POST /user_management/magic_auth` returns the plaintext `code` (10-minute expiry). Default emails can be turned off in **Emails > Configuration**, so WorkOS sends nothing. The code is then exchanged on `/user_management/authenticate`. | **Works with documented calls.** Not a described use, so W0 asks WorkOS. |

Recommended flow, with the bridge:

```text
app or browser                  account service                     WorkOS
  │ GET  /v1/sessions/nostr/challenge ─>│
  │<──────────── challenge ─────────────│
  │ POST /v1/sessions/nostr             │
  │   Authorization: Nostr <NIP-98> ───>│ verify signature, URL, method,
  │                                     │ time, challenge, body hash; burn
  │                                     │ nostr:<hex> -> acct_ -> user_…
  │                                     │── POST magic_auth {email} ───────>│
  │                                     │<──────── {code} ──────────────────│
  │                                     │── authenticate (magic-auth) ─────>│
  │                                     │<─ access, refresh, user, sid ─────│
  │<── same body as today + access_token, refresh_token, expires_in ──────│
  │    (web: Set-Cookie sealed session)  │
```

Every check we run today (`oa-auth/src/nostr.rs:390-608`, the key gate for
new accounts) runs first and unchanged. WorkOS sees only "this User signed in
by Magic Auth". We record the Nostr proof in our own access history, so the
audit trail still says which key signed in.

Limits that apply: Magic Auth send is 3 per 60 seconds per email, and
authenticate is 10 per 60 seconds per email. Apps that refresh instead of
re-signing stay far under both.

If WorkOS says the bridge isn't supported, the fallback is to keep minting
our own `sess_` for key sign-ins, bound to the WorkOS User ID in the session
record, and to use WorkOS sessions for AuthKit sign-ins only. The person
is still one WorkOS User; there are two session issuers. W0 decides.

### GitHub, email, and passkeys through AuthKit

| Method | AuthKit support | Use |
| --- | --- | --- |
| GitHub | Built-in OAuth provider, with custom scopes | Sign-in only, if the owner reopens it (owner decision 4). Repository access stays our GitHub App connection in `oa-auth/src/repos.rs`. |
| Email Magic Auth | Six-digit code, 10 minutes | Sign-in for people who add an email |
| Passkeys | Hosted AuthKit UI only; needs an AuthKit custom domain before production | `auth.openagents.com`, $99 a month |
| Google, Microsoft, Apple | Built in | Off; the 2026-10-17 direction keeps providers as connections |

AuthKit links a provider sign-in to the User that holds the same verified email. A
person who signs in by GitHub and later by key links the key through
`/v1/account/keys`, as today.

### Phone and desktop apps

| Flow | How | Notes |
| --- | --- | --- |
| Silent key sign-in (default) | The app signs NIP-98 with its device key; the answer carries WorkOS access and refresh tokens | Replaces re-signing every 23 days with refresh. Refresh tokens are single use: the inherited Zeron client already single-flights refreshes (`apps/openagents/crates/client/src/auth.rs:1-12`). |
| Device code | Coder 1.0.0 keeps `/v1/device/code` and `/v1/device/token` and receives `sess_`. New CLIs can use WorkOS CLI Auth (`POST /user_management/authorize/device`). | The approval page reads the WorkOS web session |
| PKCE | AuthKit authorization code with PKCE, redirect `openagents://callback` | Only for GitHub or email sign-in in the apps, if the owner turns it on |

### Anonymous visitors

No change. Visitors stay `oa_visitor` and `ovis_`, and the funded anonymous
lane stays `onb_public`. A WorkOS User is created at the first key sign-in.
The phone makes its key on first launch, so in practice every install that
signs in becomes a User; that is free up to 1 million monthly active users.

### Service and agent tokens

| Credential | Recommendation | Why |
| --- | --- | --- |
| `oak_` API keys | Keep ours | Bound to the tenancy door registry, the quota ledger, and sealed receipts. WorkOS API keys need a validation call per request. |
| `oa_sk_` sign-in keys | Keep ours, bound to the account and so to the User | Agents use them without a browser; `openagents keys` manages them |
| Land integrator and other service accounts | A WorkOS User that signs in with its own Nostr key | No special path. WorkOS M2M applications (`client_credentials`, an `org_id` claim, no user) fit a partner service, not one that acts as an account. |
| Self-registered agent accounts | Stay ours until claimed; a claimed agent is a principal owned by the User | Each agent as a WorkOS User would cost an email and inflate users |
| WorkOS Agent Blueprints | Revisit after general availability | User-delegated agent tokens with a permission ceiling map well to "an agent acts for its owner", but the feature is early access |

## Authorization

| Rule | Where it lives today | Target |
| --- | --- | --- |
| Workspace role (member, admin, owner) | `tenancy/src/accounts.rs:93-103` | WorkOS organization membership role; the access token's `role` claim is read for speed |
| Who may invite whom, ownership transfer, seats, personal workspace refuses invitations | `tenancy` | Stays in our code; WorkOS roles don't express these |
| Site admin | GitHub ID in the invite list (`oa-auth/src/service.rs:178`) | A permission on a WorkOS environment role, or our account list from #12003; owner's call, default ours |
| Device rights (observe, operate, terminal, review, access_read, access_admin, world) | NIP-HOST grants | Stay Nostr-native; hosts check them offline |
| Room membership | `coder-sync` rooms and room tokens | Stays ours |
| Spending limits | Spend grants, billing | Stay ours |
| Resource-level sharing (projects, chats) | Chat store, shared projects | Ours now; WorkOS FGA (resources, role assignments, a check API with a 50 ms p95 target) is an option later, not part of this plan |

The rule: WorkOS answers *who is this and what role do they hold in this
organization*. Our code answers *may this device, agent, or role do this
action here*.

## Using workos-rust

Inspected at `a0a3881` (release 3.5.0, 2026-10-19), cloned into scratch and
treated as untrusted. Nothing from it is copied here.

| Question | Answer |
| --- | --- |
| License | MIT, copyright WorkOS |
| Maintenance | Official WorkOS SDK. Repository since 2022-04-18; 3.x is generated from the API description (`oagen`) plus hand-written helpers. Commits on 2026-10-15 and 2026-10-19, release 3.5.0 on 2026-10-19; 8 contributors, 2 open issues. |
| Runtime and HTTP | Async on Tokio; `reqwest` 0.12 with `rustls-tls` by default (`native-tls` optional); a pluggable `HttpTransport` trait |
| Crypto | `aws-lc-rs` and `jsonwebtoken` pinned at `=10.3.0` with the `aws_lc_rs` backend. Our root lock has neither; it uses `ring`. |
| Toolchain | Edition 2024, MSRV 1.88 |
| Native build | `cargo check` passes on `aarch64-apple-darwin` and `aarch64-apple-ios` |
| Wasm build | Fails on `wasm32-unknown-unknown`, even with `--no-default-features`: `getrandom` needs its `js` feature and `aws-lc-sys` won't build. Not needed: only servers call WorkOS. |
| Android | Not checked (target not installed); not needed for the same reason |

Coverage:

| Area | Covered |
| --- | --- |
| User Management | Users (by ID and external ID), all authenticate grants, Magic Auth, email verification, invitations, sessions list and revoke, JWT template, redirect URIs, CORS origins, waitlists, user API keys |
| Helpers | Sealed sessions (AES-256-GCM), JWKS verification with identity binding, refresh and reseal, logout URL, PKCE, device authorization and polling, webhook signature verification, Vault local crypto |
| Organizations | Organizations, domains, memberships |
| Authorization | Environment and organization roles, permissions, resources, role assignments, `check`, effective permissions |
| Other | SSO, Directory Sync, Audit Logs, Events, Webhooks, Vault, API keys, Connect (applications, M2M, `complete_oauth_2`), Agents (blueprints, tokens, instances, sessions), MFA, Radar, Admin Portal, Widgets, Pipes, feature flags |
| Missing | The AuthKit-domain agent registration routes (`/agent/identity/*`); anything Nostr, which is ours by design |

Recommendation: **depend on the official crate**, pinned to an exact version,
in server crates only, behind our own trait. It's generated and maintained
daily by WorkOS, and a vendored thin client would only trail the API. Put it
in `crates/oa-auth` as `oa_auth::workos` behind a `workos` feature, next to
`oa_auth::nostr` and the fake GitHub, with an `IdentityProvider` trait and an
in-process fake so `cargo test` never reaches WorkOS. Don't make a separate
`crates/auth`: `oa-auth` is already the sign-in crate both the gateway and
the website link.

Revisit vendoring only if the `aws-lc-rs` build (C and CMake) costs too much
in the land queue. In that case write a small client over `reqwest` and
`ring` for the about 15 calls we use, and say in the commit that it
reimplements the SDK's request shapes.

The apps don't link the SDK. They talk to our API, and they already have a
WorkOS-shaped token client inherited from Zeron behind the
`upstream-workos` feature (`apps/openagents/crates/client/src/auth.rs`,
`apps/openagents/crates/engine/src/auth.rs`), which our builds keep off.

## Migration

Every phase sits behind one switch, `accounts.workos.mode = off | shadow |
dual | primary`, so rollback is a configuration change until W4 drops rows.
Old API addresses keep answering with the same shapes; new fields are added,
none are removed.

| Phase | Epic | What changes | Done when | Rollback |
| --- | --- | --- | --- | --- |
| W0 Foundation | [#12320](https://github.com/OpenAgentsInc/openagents/issues/12320) | WorkOS staging environment; WorkOS's written answer on the bridge; `oa_auth::workos` and its fake; JWKS verification; `identity.workos_users` and `workspace.workos_orgs` tables; the mode switch | Staging in `shadow` logs the User it would use for the smoke account, with no change to responses | `off` |
| W1 Dual-write | [#12321](https://github.com/OpenAgentsInc/openagents/issues/12321) | An outbox writes every account, key link, workspace, membership, and deletion to WorkOS, idempotent by `external_id`; WorkOS events feed a drift report | 48 hours of `dual` in staging with an empty drift report | `shadow`; delete WorkOS records by external ID |
| W2 Backfill | [#12322](https://github.com/OpenAgentsInc/openagents/issues/12322) | Resumable backfill with `--dry-run`, under 500 writes per 10 seconds | Empty reconcile report in production | Delete by recorded external IDs; our stores never changed |
| W3 Sessions | [#12323](https://github.com/OpenAgentsInc/openagents/issues/12323) | Sign-in mints WorkOS sessions; the gateway and `coder-sync` accept WorkOS access tokens; the web uses a sealed cookie; apps store refresh tokens; sign-out revokes; cohorts from test accounts to 100 % | Test account works on web, phone, and CLI; Coder 1.0.0 device code still works | `dual`; apps re-sign silently with their key, so people see nothing |
| W4 Retire | [#12324](https://github.com/OpenAgentsInc/openagents/issues/12324) | No new `sess_` except for Coder 1.0.0's device code; session rows read-only for 30 days, then dropped | No `sess_` minted in 7 days outside the device-code route | `dual` until rows drop; after that, the snapshot |

What keeps the account IDs stable: the WorkOS User's `external_id` is the
`acct_` ID, the backfill never mints a new account, and every resolver maps
`user_…` back to `acct_…` before any store is read. Chats, projects, billing,
computers, and Verse all keep keying on `acct_…`.

Test and staging accounts move first in each phase:

| Account | Where | Phase order |
| --- | --- | --- |
| Smoke account `acct_dc7a799879686fc5` | Staging | First in W1, W2, and W3 |
| SPACE test account `acct_4570d993d9897bdd` | Staging | With the smoke account |
| Test account `acct_83d16a35955ef908` | Production | First production account in W2 and W3 |
| Land integrator `acct_711fc0446cf0b234` | Production | After the test account; its `oa_sk_` key keeps working throughout |

Staging and production use separate WorkOS environments (staging is free), as
their account stores are separate today. Test runs (`X-OpenAgents-Test: 1`)
that create Users mark them with metadata `test=1` and delete them with the
run, per the "never test on the owner's account" rule.

## Data, privacy, and cost

What leaves our infrastructure:

| Sent to WorkOS | Not sent |
| --- | --- |
| Account ID (`external_id`), the email (synthetic for key-only accounts), the primary npub | Private keys, the 12 words, the wallet seed |
| Organization names and workspace IDs, memberships, roles | Chats, projects, files, memory, receipts |
| Session start and refresh times; IP address and user agent only if we pass them (we don't have to) | GitHub, Gmail, and Drive tokens |
| | Device grants, room tokens, spend grants |

Cost at our scale:

| Item | Price | Need |
| --- | --- | --- |
| AuthKit | Free to 1,000,000 monthly active users, then $2,500 a month per million | Yes |
| Custom domain | $99 a month | Only for passkeys and a branded hosted page |
| SSO, Directory Sync | $125 a connection a month, less with volume | Only if an enterprise asks |
| Audit Logs | $125 a month per SIEM stream, $99 a month per million events retained | No |
| Radar | 1,000 checks free, then $100 a month per 50,000 | Optional |
| Staging environments | Free | Yes |
| FGA, Vault, Agent Auth | Not on the pricing page | No |

Lock-in and exit: we keep the `acct_` IDs, the key links, workspaces, and
roles in our own tables throughout (WorkOS mirrors them), and no password
hashes ever exist. Leaving WorkOS means switching key sign-in back to minting
`sess_` (the code path stays behind the trait until W4 plus one release) and
re-importing nothing. The cost of leaving is the AuthKit-only sign-ins
(GitHub, email, passkeys) that we'd have to rebuild.

## Risks and open questions

Risks:

- **The bridge is off-label.** Magic Auth minted and exchanged by our server
  may be refused by WorkOS or by a later Radar rule. W0 asks first; the
  fallback keeps one User and two session issuers.
- **WorkOS outage.** Access tokens verify locally from cached JWKS, so
  signed-in people keep working until their access token expires. New
  sign-ins and refreshes fail. Keep the access-token lifetime at 5 to 15
  minutes and let `coder-sync` accept a just-expired token during an outage
  window.
- **Free keys make free Users.** The key gate (App Attest, Play Integrity,
  proof of work) runs before a User is created; keep it.
- **Email is required.** Synthetic addresses on `id.openagents.com` need an
  MX that accepts and drops mail, and must never be shown to people.
- **Two directions.** The 2026-10-08 direction says no third-party auth
  service; the 2026-10-16 merge removed Zeron's WorkOS sign-in (#11848).
  This plan reverses both and needs the owner's explicit yes.

### Owner decisions

Decided 2026-10-20 ("fully implement workos now, /login and /signup etc -
all methods we have supported in our account"):

1. **Yes.** WorkOS replaces the "own it ourselves" direction of 2026-10-08
   for people.
2. **Our `sess_` for key sign-ins**, tied to the account's WorkOS User. The
   Magic Auth bridge waits until WorkOS confirms it in writing (asked; see
   `NEEDS_OWNER.md`).
3. **Synthetic email** `acct_…@id.openagents.com` for key-only accounts,
   marked verified. An account with a verified GitHub email gets that one.
4. **Every method each environment has on in WorkOS.** Production: GitHub,
   email and password (sign-up with email verification), and Magic Auth.
   Staging: those plus Google and SSO. Passkeys and MFA stay off. The pages
   are our own over the User Management API; the hosted AuthKit page isn't
   used. GitHub sign-in makes new accounts again, through WorkOS.
5. **Only organization workspaces** become Organizations; a personal
   workspace is its User.
6. **No custom domain** for now, since passkeys are off.
7. **Yes**, `oak_` and `oa_sk_` stay our own credentials.

## What's built

| Part | Where |
| --- | --- |
| Rollout switch `OPENAGENTS_AUTH=legacy\|dual\|workos` (also `accounts.workos.mode` / `workos.mode`), `legacy` by default | `oa_auth::workos::{Mode, Settings}` |
| Client over `workos` `=3.5.0`, copy-safe refusals | `crates/oa-auth/src/workos/client.rs` |
| Access tokens checked against the cached JWKS | `crates/oa-auth/src/workos/token.rs` |
| Link table, migration 4 (`identity.workos_users`, `workspace.workos_orgs`; `workos-links.json` on files) | `crates/tenancy/src/workos_links.rs`, `crates/tenancy/migrations/0004_workos_links.sql` |
| Which account a User signs in to: link, `external_id`, GitHub identity, verified email, else new; never a second account | `crates/oa-auth/src/workos/link.rs` |
| `POST /v1/sessions/workos`: WorkOS access token in, our session out | `crates/oa-auth/src/workos/session.rs`, `crates/gateway/src/workos_auth.rs` |
| WorkOS access tokens as bearers on the account routes, beside `sess_`, `oa_sk_`, `oak_` | `gateway::workos_auth::principal` |
| Dual-write after every account change; backfill (dry run first) and reconcile under the admin token | `crates/oa-auth/src/workos/sync.rs`, `POST /v1/admin/workos/backfill`, `GET /v1/admin/workos/reconcile` |
| `/login` and `/signup` with the environment's methods and the Nostr key sign-in; `/auth/workos/{github,google}`, `/login/sso`, `/auth/callback`, `/login/code`, `/login/verify`, `/login/forgot`, `/login/reset` | `crates/openagents-web/src/workos_auth.rs` |
| Sealed WorkOS session cookie `oa_workos` beside `oa_cloud_session`, refreshed when its token expires; in `workos` mode a refused refresh signs the browser out; sign-out revokes the WorkOS session | `workos_auth::layer`, `cloud::sign_out` |
| Old addresses: `/auth/github` sends a sign-in through WorkOS; the GitHub repository connection, the device code (Coder 1.0.0), key sign-in for the apps, and `sess_` keep working | `crates/openagents-web/src/auth.rs` |
| An in-process fake WorkOS the real SDK talks to | `crates/oa-auth/src/workos/fake.rs` |
| Deploy: `WORKOS_CLIENT_ID`, `WORKOS_API_KEY`, `WORKOS_METHODS`, `OPENAGENTS_AUTH` in the web and gateway launchers; `render.py --workos` on staging once its secrets exist | `deploy/staging/{web,gateway}.sh`, `deploy/production/web.sh`, `deploy/staging/render.py` |

Differences from the plan: the link table is its own store beside the
account document rather than rows in it, so an older binary still opens the
stores. WorkOS sign-ins mint our `sess_` in every mode until W4; `workos`
mode ties that session's life to the WorkOS session. The access-token
bearer works on the account routes; `coder-sync` still takes `sess_` and
`oa_sk_`.

## Sources

- [AuthKit authentication reference](https://workos.com/docs/reference/authkit/authentication)
- [Standalone Connect](https://workos.com/docs/authkit/connect/standalone)
- [Magic Auth reference](https://workos.com/docs/reference/authkit/magic-auth)
  and [custom emails](https://workos.com/docs/authkit/custom-emails)
- [Impersonation](https://workos.com/docs/user-management/impersonation)
- [Sessions](https://workos.com/docs/authkit/sessions),
  [CLI Auth](https://workos.com/docs/authkit/cli-auth),
  [passkeys](https://workos.com/docs/authkit/passkeys),
  [social login](https://workos.com/docs/authkit/social-login)
- [Metadata and external IDs](https://workos.com/docs/authkit/metadata),
  [create a user](https://workos.com/docs/reference/authkit/user/create)
- [RBAC](https://workos.com/docs/rbac), [FGA](https://workos.com/docs/fga),
  [API keys](https://workos.com/docs/authkit/api-keys),
  [M2M](https://workos.com/docs/authkit/connect/m2m),
  [Agent Blueprints](https://workos.com/docs/authkit/agent-blueprints)
- [Rate limits](https://workos.com/docs/reference/rate-limits),
  [pricing](https://workos.com/pricing), [Vault](https://workos.com/docs/vault),
  [events](https://workos.com/docs/events)
- [workos-rust](https://github.com/workos/workos-rust) at `a0a3881`
- [Nostr-only identity audit](../audits/2026-10-17-nostr-only-identity.md),
  [auth README](README.md),
  [unified plan](../audits/2026-10-20-openagents-com-unified-plan.md)
