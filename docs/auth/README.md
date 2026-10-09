# OpenAgents authentication

Status: specification, 2026-10-08. Phase 1 (GitHub sign-in) is being built
now; later phases are design.

OpenAgents owns its sign-in. There is no third-party auth service in the path:
the account service (the `tenancy` stores behind the gateway's account
routes) is the only thing that decides who an account is, and the web server
is one of its clients. Owner direction (2026-10-08): own it ourselves, in
Rust; GitHub first, collecting everything GitHub will tell us; one button on
the website signs you in; people can stay Nostr-only and link an account
later; passkeys later.

Related: [GitHub sign-in details](github.md), the account store
(`crates/tenancy/src/accounts.rs`), the session store
(`crates/tenancy/src/sessions.rs`), the account HTTP routes
(`crates/gateway/src/accounts.rs`), the web Cloud session
(`crates/openagents-web/src/cloud/session.rs`), and the auth crate
(`crates/oa-auth`).

## Goals

1. One click from any page: "Log in" or "Sign up" in the top right, then
   "Continue with GitHub", then you are signed in where you were.
2. Sign up is just the first sign-in. No separate registration form.
3. One account, many ways in. GitHub today; Google, email codes, passkeys,
   Nostr keys, and enterprise SSO attach to the same account later.
4. Nostr-only accounts are first-class. Nobody is forced to give us a GitHub,
   Google, or email identity.
5. The account service is the authority. The web server never asserts who
   someone is; it forwards proof (a GitHub authorization code, a signed Nostr
   event, a key) and receives a session.
6. Every secret is stored as a digest or encrypted at rest, is never logged,
   and never appears in a URL we control.
7. Local testing needs no real third party: a fake GitHub runs in-process for
   tests and the local fixture. Real GitHub works locally too.

Non-goals for now: passwords (never), SMS, social sign-in beyond GitHub and
Google, and federated login *into other sites* with an OpenAgents account
(we may become an OAuth provider later; out of scope here).

## Identity model

**Account.** `acct_<hex>`, stable, created on first sign-in. It has a display
label (the GitHub name, else the login), a creation time, and a set of
*principals*: the credentials that resolve to it.

**Principal (linked credential).** A typed, unique reference. One principal
names exactly one account, enforced by the store's validation.

| Kind | Reference | Proof | Phase |
| --- | --- | --- | --- |
| API key | `key:<16 hex>` | `oak_` bearer key | exists |
| Nostr key | `nostr:<64 hex pubkey>` | signed event (NIP-98 style challenge) | exists in the store; web sign-in P2 |
| Enterprise SSO | `sso:<64 hex>` | OIDC token (REV-50) | exists |
| GitHub | `github:<numeric user id>` | OAuth code + PKCE | **P1** |
| Google | `google:<sub>` | OIDC code + PKCE | P2 |
| Email | `email:<sha256 of normalized address>` | one-time code or link | P2 |
| Passkey | `passkey:<credential id digest>` | WebAuthn assertion | P3 |

The GitHub reference is the numeric user id, never the login: logins can be
renamed and reused, ids cannot.

**Identity record.** Beside the principal, the store keeps what the provider
told us (for GitHub, see [github.md](github.md#what-we-store)), refreshed on
every sign-in. Principals live on the account; identity records live in the
account store's `identities` book keyed by provider id, and validation
requires the two to agree.

**Workspace.** Each new account gets a personal workspace bound to the
deployment's sign-up tenant, exactly as `POST /v1/accounts` does today.
Cloud, billing, keys, and bring-your-own credentials hang off workspaces and
accounts, not off the GitHub identity, so unlinking GitHub never strands work.

### Linking

- Linking adds a principal to the signed-in account. It always requires a
  fresh proof of the new credential (a new GitHub consent, a new Nostr
  signature) *and* an active session for the account.
- One identity per provider per account (one GitHub, one Google). Several
  Nostr keys and passkeys may be linked.
- A Nostr-only account links GitHub later from Settings; nothing about the
  account changes except the new principal and identity record.

### Conflicts

If the credential being linked already belongs to a different account, the
link is refused with "That GitHub account is already used by another
OpenAgents account." Nothing moves. The person can sign in with that GitHub
account instead, or unlink it there first.

We never auto-link by email. Two accounts with the same verified email are two
accounts until a person links them with proofs for both.

### Unlinking

- Unlinking removes a principal and its identity record. It requires an active
  session, and is refused if it would leave the account with no way to sign
  in ("Add another way to sign in first.").
- Unlinking ends every session that the removed credential minted (sessions
  record the credential reference that created them, P2; in P1 unlinking is
  not exposed in the UI).

### Merge

Account merge is an explicit, owner-reviewed operation, not a sign-in side
effect. Policy: the surviving account is the one the person is signed in to;
the other must also be proven (sign in to it in the same flow); principals,
personal-workspace contents, and organization memberships move; billing
balances and keys move only with an explicit confirmation; the merged account
is tombstoned with a pointer for audit. Not built before P3.

## Flows

All browser flows run on the web origin (`openagents.com`, staging, or
`127.0.0.1:4301` locally). The callback is always `/auth/<provider>/callback`.

### Sign up and sign in (same flow)

1. The visitor clicks **Log in** or **Sign up** (top right, signed out only),
   lands on `/login` (or `/signup`), and clicks **Continue with GitHub**. Any
   page may link straight to `/auth/github?return_to=/path`.
2. `GET /auth/github` validates `return_to`, makes a random `state` and a PKCE
   verifier, stores both with `return_to` in a short-lived HttpOnly flow
   cookie (`oa_auth_flow`, `SameSite=Lax`, `Path=/auth/`, 10 minutes), and
   redirects to GitHub's authorize URL with the S256 challenge.
3. GitHub sends the browser to `/auth/github/callback?code&state` (or
   `?error=access_denied` when the person cancels).
4. The web server checks `state` against the cookie, clears the cookie, and
   posts `{code, code_verifier}` to the account service's
   `POST /v1/sessions/github`. It never sees the GitHub token.
5. The account service exchanges the code (with its client secret and the
   verifier), reads the profile and emails, discards the GitHub token, finds
   the account by `github:<id>` or creates it with a personal workspace,
   refreshes the identity record, issues a `sess_` session, and records the
   sign-in in the access log.
6. The web server reads the session back like any other, sets the
   `oa_cloud_session` cookie, and answers a small page that continues to
   `return_to` (a same-origin step, so the `SameSite=Strict` session cookie
   is sent on the next request). Before it answers, it moves the web chats
   this browser made signed out (its `oa_visitor` cookie's) to the account,
   once; from then on, signed-in chats belong to the account, not the
   browser (#11039, `crates/openagents-web/src/chat_owner.rs`).

### Sign out

`POST /cloud/sign-out` with its CSRF ticket (the account menu's Sign out)
ends the session at the account service and clears the cookies. Unchanged.

### Sessions

- `sess_` bearer tokens, stored only as SHA-256 digests (`sessions.json`).
- Lifetime: the account service's `session_ttl_secs` (8 hours default). P2
  adds sliding renewal: a request in the last quarter of the lifetime gets a
  fresh session and the old one is revoked (rotation), capped at 30 days from
  the original sign-in.
- Revocation: sign out ends one session; a key rotation, a membership removal,
  or "Sign out everywhere" (P2, Settings) ends all of an account's sessions.
- The browser cookie is `HttpOnly; SameSite=Strict; Path=/`, `Secure` on
  HTTPS, and lives no longer than the session.
- The web server checks a session once per request: the account menu and a
  Cloud page share one read of the account service
  (`cloud::session::shared`).

### Apps and the command line (P2)

Desktop, phone, and `openagents` CLI sign in through the website, never with
their own GitHub app:

- **Loopback redirect** (desktop, CLI on a machine with a browser): the app
  listens on `127.0.0.1:<random port>`, opens
  `https://openagents.com/auth/app?redirect=http://127.0.0.1:<port>/cb&challenge=<S256>`,
  the person signs in on the website as usual and approves "Sign in to
  OpenAgents Desktop", and the site redirects to the loopback with a one-time
  code the app redeems with its verifier for its own session.
- **Device code** (SSH sessions, headless): the app shows a short code and
  `openagents.com/device`; the person enters it while signed in and approves;
  the app polls and receives its session.
  Built (#11045); see [Device sign-in](#device-sign-in-built) below.
- **Phone**: the app opens the same `/auth/app` URL in the system browser
  session and receives the code through its registered universal link.

App sessions are separate sessions on the same account, listed in Settings
and revocable one by one.

### Device sign-in (built)

RFC 8628's shape. Code: `tenancy::sessions` (`sessions/device.rs`: grants
and app sessions), `oa_auth::device` (the account-service routes, served by
the gateway and `oa_auth::local`), `openagents-web` (`device.rs`,
`cloud/session/device.rs`), `openagents-login` (the client), and
`coder-new` (`account.rs`).

| Where | Route | What |
| --- | --- | --- |
| Website | `POST /device/code` `{app, computer}` | `device_code`, `user_code` (`BCDF-GHJK`), `verification_uri` (`{origin}/device`), `verification_uri_complete`, `expires_in` (600), `interval` (5) |
| Website | `GET /device[?code=]` | Signed in (sign-in comes back here): "Sign in to Coder on <computer>?", Approve / Deny |
| Website | `POST /device/token` `{device_code}` | `{access_token, token_type, expires_in, account}`, or `authorization_pending`, `slow_down` (+5 s, with `interval`), `access_denied`, `expired_token`, `invalid_grant` |
| Website | `POST /device/sign-out` (app's bearer) | Ends the app's own token |
| Website | Settings → Computers, `POST /settings/computers/remove` | Lists signed-in apps; Remove ends one |
| Account service | `POST /v1/sessions/device`, `/device/poll`, `/device/lookup`, `/device/decide`; `GET /v1/account/sessions`; `DELETE /v1/account/sessions/{id}` | The same, over the stores; lookup and decide need a browser session (not an app's own) |

- Both codes are stored only as SHA-256 digests in `sessions.json`; a grant
  lasts 10 minutes and issues one session, once. User codes are 8 letters
  from `BCDFGHJKLMNPQRSTVWXZ` (no vowels, no look-alikes); typing is
  case- and dash-insensitive.
- The app's token is an ordinary `sess_` user session labeled with the app
  and computer, lasting 30 days, so every account route takes it.
- `coder-new login` / `logout` and `/login` / `/logout` keep it in
  `~/.openagents/coder-new/account.json` (0600, folder 0700); it is never
  printed or written to session files. `OPENAGENTS_ORIGIN` points the
  client at another site (`https`, or `http://127.0.0.1:PORT`).

Try it locally with the GitHub fixture ([github.md](github.md#local-testing))
on `127.0.0.1:4301`, signed in there, then:

```sh
OPENAGENTS_ORIGIN=http://127.0.0.1:4301 cargo run -p coder-new -- login --state "$TMPDIR/coder-login"
```

## Security

- **CSRF on sign-in**: the OAuth `state` is 32 random bytes bound to the flow
  cookie; a callback without the matching cookie is refused. All state-changing
  web forms keep the existing HMAC CSRF tickets.
- **PKCE** (S256) on every code flow, even with a client secret.
- **Open redirects**: `return_to` must be a same-site path: it starts with a
  single `/`, has no `//` or `\` prefix, no scheme, no control characters, and
  is at most 512 bytes. Anything else becomes `/`.
- **Cookies**: session cookies `HttpOnly; SameSite=Strict`; the flow cookie
  `HttpOnly; SameSite=Lax` (it must survive the top-level return from
  github.com) and scoped to `/auth/`; `Secure` everywhere on HTTPS.
- **Secrets**: the GitHub client secret lives only in the account service's
  private config file (mode 0600, never committed, never logged). The web
  server needs only the client id. GitHub access tokens are discarded after
  the profile read; if a later feature must keep one, it is encrypted at rest
  with the per-deployment `token_encryption_key` (AES-256-GCM, key in the same
  private file) and scoped to that feature.
- **Outbound requests**: the account service talks only to the configured
  GitHub endpoints, follows no redirects, and bounds time and response size.
- **Rate limits** (P2): per IP and per account on `/auth/*` and
  `/v1/sessions/*`; the account service refuses bursts with 429.
- **Audit**: every sign-in, sign-up, link, unlink, and sign-out appends to the
  session store's access log with the account, the session digest, and the
  principal reference (never a token or a profile field).

## Privacy

- We store what the provider gives us under the scopes we request, so we can
  show who you are, reach you, and keep sign-in working when you rename your
  GitHub login. See [github.md](github.md#what-we-store).
- Shown to others: nothing by default. Shown to you: your name, avatar, login,
  and linked sign-in methods in Settings.
- Deletion: deleting an account removes its principals, identity records, and
  personal workspace data, keeps the access log entries (references only) for
  their retention period, and frees the GitHub id to sign up again.
- The privacy policy page lists GitHub as a sign-in provider and the fields
  above.

## Data model and storage

| Store | File | Holds |
| --- | --- | --- |
| Accounts | `accounts.json` (+ `accounts-history/`) | accounts, principals, workspaces, memberships, invitations, and the `identities` book (GitHub profiles) |
| Sessions | `sessions.json` | session digests, credentials digests, recovery digests, access log |
| Keys | `keys.json` | `oak_` key digests |

All three are sealed, digested, versioned JSON with a writer lock, as today.
The identities book is omitted when empty, so stores written before it keep
their digests.

What hangs off the account:

- **Workspaces** (personal and organization), and through them Cloud
  computers, tasks, billing, credits, and spending limits.
- **API keys** (`oak_`), issued per workspace.
- **Bring-your-own credentials** (the Claude credential page and BYOK keys),
  bound to the account and workspace, never to the GitHub identity.

## Migration from keys-only accounts

Today an account is created by an operator or `POST /v1/accounts`, and a
browser signs in by pasting an `oak_` key. With P1:

- The product web UI no longer shows the key form when GitHub sign-in is
  configured; `/cloud/sign-in` sends people to `/login`. The key form stays for
  deployments without GitHub (and for tests), and `POST /v1/sessions` with an
  `oak_` key keeps working for scripts.
- An existing key-only account links GitHub from Settings (P1.5: the link
  route exists in the account service; the Settings button follows). Until
  then a GitHub sign-in by the same person creates a new account; an operator
  can link by hand.
- Nothing about workspaces, keys, or billing changes.

## Phases

- **P1 GitHub** (now): `oa-auth` crate (OAuth client, PKCE, profile reader,
  fake GitHub), the `github:` principal and identities book in `tenancy`,
  `POST /v1/sessions/github` and `POST /v1/account/identities/github` in the
  account service, `/login`, `/signup`, `/auth/github`,
  `/auth/github/callback` on the web, the Log in and Sign up buttons, and a
  local fixture.
- **P2 Google and email codes**: Google OIDC (same flow shape); email one-time
  codes (6 digits, 10 minutes, 5 attempts, digest stored); Nostr sign-in on
  the web (NIP-07 extension signs a challenge event; NIP-46 remote signer
  later); Settings: linked methods, link and unlink, sessions list, sign out
  everywhere; app and CLI sign-in (loopback and device code); sliding
  sessions; rate limits.
- **P3 Passkeys**: WebAuthn registration and sign-in, passkey-only accounts,
  account merge.
- **P4 Enterprise SSO**: SAML and broader OIDC on top of the existing REV-50
  provider book, domain verification, SCIM later.
