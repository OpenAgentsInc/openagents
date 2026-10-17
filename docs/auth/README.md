# OpenAgents authentication

Status: implementation and specification, updated 2026-10-17. Your Nostr key
is your account: an app makes 12 words on first launch, derives its key from
them, and signs in with that key. GitHub, Google, and other providers are
connections, not ways to sign in from the apps. Much of this is still
planned: the table says which parts are built, and each section below says
the same, with its issue. The plan and its evidence are in the
[Nostr-only identity audit](../audits/2026-10-17-nostr-only-identity.md);
the tracking epic is
[#12006](https://github.com/OpenAgentsInc/openagents/issues/12006).

**GitHub sign-in is deprecated (#12005).** It signs in accounts made with it
before and makes no new accounts: the account service answers a GitHub user
no account holds with `403 signup_disabled` unless the deployment sets
`accounts.github.new_accounts`. GitHub stays as a connection for
repositories, issues, and pull requests
(`POST /v1/account/identities/github`). Coder 1.0.0's device-code flow keeps
working: its approval page uses an existing website session. **Sign in with
Apple was removed**; see [below](#sign-in-with-apple-removed).

| Part | State |
| --- | --- |
| NIP-06 key from 12 or 24 words (`crates/nostr/src/nip06.rs`) | Built ([#11992](https://github.com/OpenAgentsInc/openagents/issues/11992)) |
| Phone first launch: a short tutorial, no sign-in screen | Built ([#11996](https://github.com/OpenAgentsInc/openagents/issues/11996)); backup and restore rows wait for #11995 |
| Sign in with a Nostr key (`/v1/sessions/nostr`) | Built ([#11993](https://github.com/OpenAgentsInc/openagents/issues/11993)) |
| Link and unlink keys (`/v1/account/keys`) | Built ([#11993](https://github.com/OpenAgentsInc/openagents/issues/11993)) |
| The app's identity from 12 words, silent sign-in | Planned: shared Rust [#11995](https://github.com/OpenAgentsInc/openagents/issues/11995), desktop [#11997](https://github.com/OpenAgentsInc/openagents/issues/11997) |
| Link a computer by scanning its QR code | Planned: desktop [#11998](https://github.com/OpenAgentsInc/openagents/issues/11998), phone [#12000](https://github.com/OpenAgentsInc/openagents/issues/12000); wire in [NIP-HOST](../../nips/openagents/NIP-HOST.md#account-key-link-by-qr-planned) |
| Website sign-in by phone scan and NIP-07 | Planned ([#12001](https://github.com/OpenAgentsInc/openagents/issues/12001)) |
| Limits for key-only accounts | Planned ([#11994](https://github.com/OpenAgentsInc/openagents/issues/11994)) |
| Account deletion | Planned ([#12002](https://github.com/OpenAgentsInc/openagents/issues/12002)) |
| Admin and allowlists by account, not GitHub ID or email | Planned ([#12003](https://github.com/OpenAgentsInc/openagents/issues/12003)) |
| Remove Sign in with Apple; GitHub becomes a connection | Built ([#12005](https://github.com/OpenAgentsInc/openagents/issues/12005)): the Apple routes answer `410`, and GitHub sign-in makes no new accounts |
| GitHub website sign-in, device-code sign-in, app OAuth, sign-in keys | Built; they keep working, and the device-code flow stays for Coder 1.0.0 |

App API calls use `https://api.openagents.com/v1`; browser approval stays on
`openagents.com`. The SDK keeps those deployment addresses explicit and does
not expose the account service's internal session and approval endpoints. An
app keeps its credentials in its own OS storage, not in the CLI's saved
account file.

OpenAgents owns its sign-in. There is no third-party auth service in the path:
the account service (the `tenancy` stores behind the gateway's account
routes) is the only thing that decides who an account is, and the web server
is one of its clients. Owner direction (2026-10-08): own it ourselves, in
Rust. Owner direction (2026-10-17): open the app and use it right away, with
no sign-in; the 12 words are the one backup, for the account and the wallet;
GitHub and other providers are connections only; computers pair by QR code.

Related: [GitHub sign-in details](github.md), the account store
(`crates/tenancy/src/accounts.rs`), the session store
(`crates/tenancy/src/sessions.rs`), the account HTTP routes
(`crates/gateway/src/accounts.rs`), the web Cloud session
(`crates/openagents-web/src/cloud/session.rs`), the auth crate
(`crates/oa-auth`), and NIP-06 (`crates/nostr/src/nip06.rs`,
[the NIP](../../nips/official/06.md)).

## Goals

1. No sign-in. Opening an app for the first time makes an account in the
   background, and the person starts using it at once.
2. One backup. The 12 words restore the account's key and the wallet. The
   nsec is an optional extra for people who use the key in other Nostr apps.
3. One account, many keys. Every phone and computer has its own key, linked
   to the same account, so losing a device means removing one key, not
   replacing the account.
4. Providers are connections. GitHub, Google, Gmail, and Drive attach to the
   account for what they do (repositories, mail, files); none is needed to
   get in.
5. The account service is the authority. The web server never asserts who
   someone is; it forwards proof (a signed Nostr event, a GitHub
   authorization code, a key) and receives a session.
6. Every secret is stored as a digest or encrypted at rest, is never logged,
   and never appears in a URL we control.
7. Released clients keep working. Coder 1.0.0's device-code flow, `sess_`
   and `oa_sk_` tokens, routes, and error codes are unchanged
   ([Coder 1.0 freeze](../coder/1.0-freeze.md)); every change here is
   additive.
8. Local testing needs no real third party: a fake GitHub runs in-process for
   tests and the local fixture, and tests sign with throwaway keys.

Non-goals for now: passwords (never), SMS, email codes, and federated login
*into other sites* with an OpenAgents account. Apps acting for a person on
OpenAgents itself (MCP clients) do sign in with OAuth: see "Apps signing in
with OAuth" below.

## Identity model

### The 12 words and the identity key

On first launch an app makes a 12-word BIP-39 phrase (16 bytes of entropy).
From it:

- The **identity key** is NIP-06: the words become a 64-byte BIP-39 seed
  with an empty passphrase, and BIP-32 derives the secp256k1 key at
  `m/44'/1237'/0'/0/0`. Its npub is the person's ID wherever a person sees
  one. Built: `nostr::nip06::Mnemonic` parses or makes the words (12 or 24,
  English list, checksum checked) and `Mnemonic::identity` derives the key;
  the tests include the NIP-06 and BIP-32 vectors (`nip06_vectors`,
  `bip32_vector_one` in `crates/nostr/src/nip06.rs`).
- The **wallet** opens from the same words: `crates/spark-wallet` hands them
  to Breez Spark (`Seed::Mnemonic`, no passphrase), and Spark derives its own
  keys on its own paths. The Nostr key and the wallet keys are different keys
  from one backup.

Where a phone already holds a wallet seed (Keychain service
`com.openagents.app.spark`, account `seed-v1`, from the earlier app), those
words are the identity's words too; the app reuses them and never makes new
ones in their place. The rules are in
[INVARIANTS.md](../../INVARIANTS.md#device-identity-key) and the
[Phone wallet rows](../../INVARIANTS.md#phone-wallet). Planned in #11995 and
#11999.

Each device's pairing and push keys stay random and separate from the words,
so removing a lost device never means changing the words.

**Back up and restore** (planned, #11995 and #11996). Settings > **Back up
your account** shows the 12 words after Face ID, with **Reveal nsec** as an
optional extra. **Restore from 12 words** derives the same key on a new
device, signs in, and brings back the account and the wallet. The app asks
for the backup once the account holds something worth keeping, not at first
launch.

### Accounts and principals

**Account.** `acct_<hex>`, stable, created on first sign-in. People never see
it. It has a display label, a creation time, and a set of *principals*: the
credentials that resolve to it. Every store, cache, and room keys on the
account ID, so adding or removing a key moves no data.

**Principal (linked credential).** A typed, unique reference. One principal
names exactly one account, enforced by the store's validation
(`valid_principal` in `crates/tenancy/src/accounts.rs`).

| Kind | Reference | Proof | Role | State |
| --- | --- | --- | --- | --- |
| Nostr key | `nostr:<64 hex pubkey>` | NIP-98 event over a server challenge | How the apps sign in; one per device, several per account | Built (#11993) |
| API key | `key:<16 hex>` | `oak_` bearer key | Scripts and the decision API | Built |
| Enterprise SSO | `sso:<64 hex>` | OIDC token (REV-50) | Organizations | Built |
| GitHub | `github:<numeric user id>` | OAuth code + PKCE | A connection (repositories, issues, pull requests); still signs in existing accounts on the website | Built; makes no new accounts (#12005) |
| Apple | `apple:<Apple sub>` | none | Removed; linked accounts keep their data | The routes answer `410 apple_removed` (#12005) |
| Passkey | `passkey:<credential id digest>` | WebAuthn assertion | Later | Not built |

The GitHub reference is the numeric user id, never the login: logins can be
renamed and reused, ids cannot.

**Connections.** Gmail, Drive, Calendar, and GitHub's repository access are
connections on the account (`crates/openagents-web/src/cloud/connections.rs`
and [GitHub repository access](github.md#repository-access)). They keep their
own tokens, encrypted at rest, and are never a way in. Their email
allowlists move to account IDs in
[#12003](https://github.com/OpenAgentsInc/openagents/issues/12003).

**Identity record.** Beside a provider principal, the store keeps what the
provider told us (for GitHub, see [github.md](github.md#what-we-store)),
refreshed on every sign-in or link. Principals live on the account; identity
records live in the account store's `identities` book keyed by provider id,
and validation requires the two to agree. A Nostr key has no identity record.

**Workspace.** Each new account gets a personal workspace bound to the
deployment's sign-up tenant, exactly as `POST /v1/accounts` does today.
Cloud, billing, keys, and bring-your-own credentials hang off workspaces and
accounts, not off any key or provider identity, so removing a key or a
connection never strands work.

### Linking

- Linking adds a principal to the signed-in account. It always requires a
  fresh proof of the new credential (a signature by the new Nostr key, a new
  GitHub consent) *and* an active session for the account.
- Several Nostr keys may be linked: one for each phone and computer. One
  identity per provider per account (one GitHub, one Google).
- A computer's key is linked by a phone scanning the computer's QR code:
  see [Link a computer by QR code](#link-a-computer-by-qr-code-planned).
- An account that predates key sign-in (GitHub, Apple, or device code) gets
  a key the same way: the new app signs in with its old session, then adds
  its key with `POST /v1/account/keys`. We never link by email.

### Conflicts

If the credential being linked already belongs to a different account, the
link is refused ("That GitHub account is already used by another OpenAgents
account."). Nothing moves. The person can sign in to that account instead,
or remove the credential there first.

We never auto-link by email. Two accounts with the same verified email are two
accounts until a person links them with proofs for both.

### Unlinking

- Unlinking removes a principal and its identity record. It requires an active
  session, and is refused if it would leave the account with no way to sign
  in ("Add another way to sign in first."). The last Nostr key can't be
  removed.
- Unlinking ends every session that the removed credential minted (sessions
  record the credential reference that created them).

### Merge

Account merge is an explicit, owner-reviewed operation, not a sign-in side
effect. Policy: the surviving account is the one the person is signed in to;
the other must also be proven (sign in to it in the same flow); principals,
personal-workspace contents, and organization memberships move; billing
balances and keys move only with an explicit confirmation; the merged account
is tombstoned with a pointer for audit. Not built.

## Flows

All browser flows run on the web origin (`openagents.com`, staging, or
`127.0.0.1:4301` locally). The callback is always `/auth/<provider>/callback`.

### Sign in with a Nostr key

Built in [#11993](https://github.com/OpenAgentsInc/openagents/issues/11993) (`oa_auth::nostr`).
A signed event becomes the same 30-day `sess_` app session a device sign-in
issues, so every route, room, and cache that takes a session takes it
unchanged.

| Where | Route | What |
| --- | --- | --- |
| Account service | `GET /v1/sessions/nostr/challenge` | A random challenge with a short expiry |
| Account service | `POST /v1/sessions/nostr`, `Authorization: Nostr <base64 event>` | Finds or creates the account holding `nostr:<hex>` and answers a 30-day app session |
| Website forwarder | `GET /v1/device/nostr/challenge`, `POST /v1/device/nostr` `{app, computer}` | The same on the API host, forwarded byte for byte |

1. The app gets a challenge.
2. It signs a [NIP-98](../../nips/official/98.md) event (kind 27235) over
   the exact URL and `POST`, with a `challenge` tag and a `payload` tag
   holding the SHA-256 of the exact body.
3. The account service checks the signature, the URL, the method, the
   60-second time window, the challenge, and the body hash. It burns the event
   ID and the challenge, so each works once. It finds or creates the account
   holding `nostr:<hex>` (a new one gets a personal workspace on
   `signup_tenant`) and answers the app session. No invite list applies: the
   key is the account.
4. The app stores the session in its own OS storage and, before it expires,
   signs in again. There is no refresh token, password, or browser.

The relay keeps NIP-42 for relay admission; this route doesn't replace it.

### Website sign-in with a key

Built in #12001 (`crates/openagents-web/src/web_sign_in.rs`). `/login`
shows a QR code of `https://openagents.com/login/scan#request=…&challenge=…`
that stands five minutes and works once. The OpenAgents phone app scans it,
can read `GET /v1/web-sign-in/{request}` to show which browser it signs in,
and sends `POST /v1/web-sign-in/approve` with `{"request"}`, signed as
above over that URL with the scanned challenge
(`openagents_sdk::Client::approve_scanned_login`). The page polls
`GET /login/key/{request}/status` and receives the usual `oa_cloud_session`
cookie. A browser extension (NIP-07) signs the same event over
`POST /login/key/extension` instead. Both go through
`POST /v1/sessions/nostr`, which issues a browser session, not an app
session, for these two paths (`oa_auth::nostr::WEB_SIGN_IN_PATHS`). A
key-only account then uses `/device`, `/oauth/authorize`, and Settings as
a GitHub account does.

### Linking keys

Built in #11993. A signed-in app manages the account's Nostr keys:

| Route | What |
| --- | --- |
| `GET /v1/account/keys/challenge` | A challenge bound to this account |
| `POST /v1/account/keys` `{proof}` | Adds the key that signed `proof`: a kind 27235 event over `POST /v1/account/keys` with that challenge |
| `GET /v1/account/keys` | Lists the account's keys |
| `DELETE /v1/account/keys/{pubkey}` | Removes one key, never the last |

The website forwards the same routes under `/v1/device/keys`. A challenge
made for one account can't add a key to another.

### Link a computer by QR code (planned)

Planned in #11998 (desktop) and #12000 (phone). The computer shows its
NIP-HOST connect code as a QR code; the phone scans it. One scan pairs the
computer with a host grant and links the computer's own key to the phone's
account, so the computer then signs in as the same person with its own key.
The wire is in
[NIP-HOST, Account key link by QR](../../nips/openagents/NIP-HOST.md#account-key-link-by-qr-planned).
A headless computer uses reverse enrollment or `openagents connect invite`,
as today. A second phone joins the same way, or restores from the 12 words.

### Website sign-in without a provider (planned)

Planned in [#12001](https://github.com/OpenAgentsInc/openagents/issues/12001).
The website still needs a signed-in browser for `/device` (Coder 1.0.0's
approval page), `/oauth/authorize`, Settings, and billing. Two ways in need
no provider: **scan with your phone** (the page shows a QR code, the phone
signs the page's challenge, and the browser gets its session) and a
[NIP-07](../../nips/official/07.md) browser extension. GitHub sign-in stays
for existing accounts.

### Apps signing in with OAuth (#11084)

MCP clients and other apps that act for a person use OAuth 2.1 against the
site itself (`crates/openagents-web/src/oauth.rs`; the steps for app authors
are in `/auth.md`). Registration (`POST /oauth/register`) stores nothing:
the `client_id` carries the app's name and redirect addresses under a MAC.
`GET /oauth/authorize` checks the client, the redirect address, and PKCE
`S256` before showing anything, sends a signed-out visitor through sign-in
(the request waits in an `oa_oauth_request` cookie, `Path=/oauth/`, 10
minutes), and shows "Let <app> use your OpenAgents account?". Approve starts
a device sign-in for the app and approves it at once under the browser
session; the authorization code seals that sign-in's device code with the
challenge, client, redirect address, and a 5-minute expiry (AES-256-GCM,
key derived from the CSRF secret). `POST /oauth/token` checks the verifier
and redeems the device code once, answering the app session (30 days, no
refresh token). The app shows in Settings' Computers as "<app> on <host>".

### GitHub on the website (built)

Built, and it keeps working for accounts that have GitHub linked. The apps
never ask for it, and after
[#12005](https://github.com/OpenAgentsInc/openagents/issues/12005) the website
offers it as "Sign in with a connected account" beside the provider-free ways
in. GitHub stays the connection for repositories, issues, and pull requests.

1. The visitor clicks **Log in** or **Sign up** (top right, signed out only)
   and lands on `/login` (or `/signup`). The page leads with the app, which
   needs no sign-in, and keeps **Continue with GitHub** under "Signed up with
   GitHub before? Sign in here." Any page may link straight to
   `/auth/github?return_to=/path`.
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
   the account by `github:<id>` (or, only with `accounts.github.new_accounts`,
   creates it with a personal workspace; otherwise it refuses with
   `403 signup_disabled` and the website says new accounts start in the app),
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

### Apps and the command line

- **Nostr key** (phone, desktop, and the next Coder): the app signs in with
  its own key, with nothing to approve. Planned; see
  [Sign in with a Nostr key](#sign-in-with-a-nostr-key-planned). The phone
  already opens with no sign-in screen (#11996).
- **Loopback redirect** (not built, and no longer planned for the apps): the app
  listens on `127.0.0.1:<random port>`, opens
  `https://openagents.com/auth/app?redirect=http://127.0.0.1:<port>/cb&challenge=<S256>`,
  the person signs in on the website as usual and approves "Sign in to
  OpenAgents Desktop", and the site redirects to the loopback with a one-time
  code the app redeems with its verifier for its own session.
- **Device code** (Coder 1.0.0, SSH sessions, headless): the app shows a
  short code and `openagents.com/device`; the person enters it while signed
  in and approves; the app polls and receives its session. Built (#11045), and
  kept unchanged for Coder 1.0.0; see [Device sign-in](#device-sign-in-built)
  below. A person with only a key approves it after signing in to the website
  by phone scan (#12001). The next Coder signs in with a key instead
  ([next release](../coder/next-release.md)).

- **Sign-in key** (agents, scripts, sandboxes; no code to approve): the
  person makes a key in Settings, Sign-in keys for agents, and the agent
  uses it. Built (#11305); see [Sign-in keys for agents](#sign-in-keys-for-agents-built)
  below.

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
| Website | `GET /settings/terminal` (#11089) | "Connect your terminal": install, `coder login --pair <code>`, the waiting sign-in with Approve right there, then where the computer's chats live |
| Website | Settings → Computers, `POST /settings/computers/remove` | Lists signed-in apps; Remove ends one |
| Account service | `POST /v1/sessions/device`, `/device/poll`, `/device/lookup`, `/device/decide`, `/device/paired`; `GET /v1/account/sessions`; `DELETE /v1/account/sessions/{id}` | The same, over the stores; lookup, decide, and paired need a browser session (not an app's own) |

- Both codes are stored only as SHA-256 digests in `sessions.json`; a grant
  lasts 10 minutes and issues one session, once. User codes are 8 letters
  from `BCDFGHJKLMNPQRSTVWXZ` (no vowels, no look-alikes); typing is
  case- and dash-insensitive.
- A pair code (`coder login --pair <code>`, from the Connect page; the
  page's code is an HMAC of the account id under the server's key) rides
  along with `/device/code`. The grant keeps its digest and the user code
  masked with it, so `/device/paired` can list the waiting sign-ins (with
  their codes) to the browser holding the pair code, and nothing stored is
  a code.
- The app's token is an ordinary `sess_` user session labeled with the app
  and computer, lasting 30 days, so every account route takes it.
- `coder-new login` / `logout` and `/login` / `/logout` keep it in
  `~/.openagents/coder/account.json` (0600, folder 0700); it is never
  printed or written to session files. `OPENAGENTS_ORIGIN` points the
  client at another site (`https`, or `http://127.0.0.1:PORT`).

Try it locally with the GitHub fixture ([github.md](github.md#local-testing))
on `127.0.0.1:4301`, signed in there, then:

```sh
OPENAGENTS_ORIGIN=http://127.0.0.1:4301 cargo run -p coder-new -- login --state "$TMPDIR/coder-login"
```

### Sign-in keys for agents (built)

A sign-in key lets an agent act as you without the device code. Make one at
[openagents.com/settings/sign-in-keys](https://openagents.com/settings/sign-in-keys)
(Settings, API, Sign-in keys for agents): name it, pick when it expires
(never, in 30 days, or in 90 days), and copy it; it is shown once. The list
there shows each key's name, when it was made and last used, when it
expires, and Revoke.

Give it to an agent in one line, with nothing saved:

```sh
OPENAGENTS_TOKEN=oa_sk_... coder
```

Or save it on a computer (paste the key when asked, or pipe it in; never put
it on the command line, where shell history keeps it):

```sh
coder login --key
openagents login --key < key.txt
```

Manage keys from any signed-in terminal (`--json` for scripts):

```sh
openagents keys list
openagents keys create --name "build agent" --expires 30d
openagents keys show "build agent"
openagents keys rotate "build agent"
openagents keys revoke "build agent"
openagents keys list --kind api        # API keys for model calls, same commands
```

Code: `tenancy::sessions::keys` (the record), `oa_auth::sign_in_keys` (the
account-service routes, served by the gateway and `oa_auth::local`),
`openagents-web` (`sign_in_keys.rs`, `cloud/session/device.rs`),
`openagents-login` (`TOKEN_ENV`, `sign_in_with_key`, `read_key`),
`coder-new` (`coder login --key`), and `openagents-cli` (`keys.rs`).

| Where | Route | What |
| --- | --- | --- |
| Website | `GET /settings/sign-in-keys`, `POST` (make), `POST /settings/sign-in-keys/revoke` | Settings |
| Website | `GET /v1/keys[?kind=api]` | `{kind, account: {id, label}, keys: [{id, name, created_at, last_used_at, expires_at, current}]}`; never a secret |
| Website | `POST /v1/keys` `{name, expires_in_days?, kind?}` | `{kind, key, token}`; the token is in this answer only |
| Website | `DELETE /v1/keys/{id}[?kind=api]` | Revokes one |
| Account service | `GET`/`POST /v1/account/sign-in-keys`, `DELETE /v1/account/sign-in-keys/{id}` | The same, over the session store |

- A key is `oa_sk_` and 64 hex characters: a user session on the account,
  so every route that takes an app's `Bearer sess_…` takes it, through the
  same check, as that account only. Only its SHA-256 is stored; its last use
  is written at most hourly.
- `/v1/keys` takes any signed-in credential (a `sess_` sign-in or a key);
  an `oak_` API key can't manage sign-in keys, and a key can't approve
  another sign-in on `/device`.
- `OPENAGENTS_TOKEN` (or the older `OPENAGENTS_APP_TOKEN`) with
  `OPENAGENTS_ORIGIN` signs in every app that reads its sign-in through
  `openagents-login` (Coder and its sync, `openagents mac serve`, `chat
  work`) with no saved file. `coder logout` forgets a saved key without
  revoking it; revoke it in Settings or with `openagents keys revoke`.
- The `oa_sk_` prefix is on the redaction lists, so a key pasted into a chat,
  a log, or a skill is caught.

Try it against staging:

```sh
OPENAGENTS_ORIGIN=https://<staging host> OPENAGENTS_TOKEN=oa_sk_... openagents keys list
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
  server needs only the client id. The sign-in token is discarded after the
  profile read. Repository access ([GitHub](github.md#repository-access))
  keeps its own token, encrypted at rest with the per-deployment
  `token_encryption_key` (AES-256-GCM, key in the same private file).
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
- Shown to others: nothing by default. Shown to you: your npub, linked devices, and connected
  accounts in Settings.
- Deletion (in the app, planned in
  [#12002](https://github.com/OpenAgentsInc/openagents/issues/12002)):
  deleting an account removes its principals, identity records, and
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

- **P1 GitHub** (built): `oa-auth` crate (OAuth client, PKCE, profile
  reader, fake GitHub), the `github:` principal and identities book in
  `tenancy`, `POST /v1/sessions/github` and
  `POST /v1/account/identities/github` in the account service, `/login`,
  `/signup`, `/auth/github`, `/auth/github/callback` on the web, and a local
  fixture. Device-code sign-in (#11045) and sign-in keys (#11305) followed.
- **Nostr identity** (now, epic
  [#12006](https://github.com/OpenAgentsInc/openagents/issues/12006)): NIP-06
  (#11992, built); key sign-in and key linking (#11993); limits for key-only
  accounts (#11994); the identity from 12 words and silent sign-in in the
  apps (#11995, #11997); the phone tutorial, backup, and restore (#11996,
  tutorial built); QR linking (#11998, #12000); website sign-in by phone scan
  and NIP-07 (#12001); account deletion (#12002); admin and allowlists by
  account (#12003); Apple removed and GitHub as a connection (#12005); the
  wallet from the same words (#11999).
- **Later**: per-request NIP-98 as a third bearer kind, a NIP-46 remote
  signer, the per-feature device keys consolidated under each device's key,
  the vault's recovery slot derived from the same 12 words, passkeys, and
  SAML and broader OIDC on the REV-50 provider book.

## Public app authentication contract

The app subset of the served OpenAPI is also checked in at
[`docs/sdk/openapi.app.json`](../sdk/openapi.app.json). Regenerate it with
`cargo run -p openagents-web --example app_openapi > docs/sdk/openapi.app.json`.
Both `/openapi.json` and `/v1/openapi.json` merge this subset with the gateway
schema. Account-service session and approval routes remain private.

Use `POST https://api.openagents.com/v1/device/code` with
`{"app":"Coder Desktop","computer":"My Mac"}`. Open the returned
`verification_uri_complete` in the system browser; it belongs to the website,
not the API host. Poll `/v1/device/token` with the device code at `interval`
seconds, increasing the interval on `slow_down`. Codes expire after 600 seconds.
A successful poll returns the existing account, `access_token`, and
`expires_in`; the app session lasts 30 days and has no refresh token.
Redemption succeeds once. If its response is lost, restart login.

Device errors use `{"error":"code","error_description":"text"}`; account
errors use `{"error":{"code":"code","message":"text"}}`. Keep them distinct.
Use the app bearer on `GET /v1/account` to revalidate the account and discover
its workspaces after restart. This route also accepts supported sign-in keys
and account-bound API keys; those keys do not automatically grant every API
operation. API-host cookies do not authenticate a caller.

`POST /v1/device/sign-out` revokes the presented app session. An expired or
revoked session is already signed out. Website Settings can remove an app
session too. Authenticated chat reads may reuse a successful session check for
up to 30 seconds; account discovery checks current authority directly. A
transient service failure is not a successful check or proof of logout.

For staging/local use the matching explicit API and website origins. Keep the
app credential in its own OS storage; never adopt CLI or Zeron credentials.

## Sign in with Apple, removed

Sign in with Apple (#11980) was removed before it shipped outside TestFlight
(#12005). `POST /v1/sessions/apple` on the account service and
`POST /v1/device/apple` on the API host stay, so build 1012 gets a plain
answer: `410 apple_removed`, "Sign in with Apple has been removed." Accounts
already linked to Apple keep their `apple:<sub>` principal, identity record,
and data, and add a key from a signed-in device.

The device-code flow stays, and build 1012 opens its approval page in an
in-app sheet (`ASWebAuthenticationSession`) with `return=app`: the page
then lets its form land on `openagents:`, and Approve or Deny sends the
sheet to `openagents://signed-in` (`?denied=1`), which closes it. Without
`return=app` the page is unchanged.
