# Nostr-only identity: feasibility audit

Date: 2026-10-17. Scope: every sign-in path, session, account record, client
credential store, Nostr key and pairing flow in this repository, measured
against one owner request:

> Open the app and start using it right away. Generate a Nostr keypair that
> is the person's ID, and authenticate to our servers with NIP-42 (and
> whatever else is needed). GitHub and anything else are connected
> separately and are never needed to log in. Pair a computer primarily by
> scanning a QR code. No Apple login next to GitHub login next to Google
> login.

## Verdict

**Feasible, and most of the hard parts already exist.** The server already
treats an account as an opaque `acct_…` ID that one or more principals
resolve to, and `nostr:<64 hex pubkey>` is already a valid principal kind.
Every downstream route, store and cache keys on the account ID, not on
GitHub or email. Our NIP-HOST stack already does single-use, rotating QR
pairing with signed device grants and revocation. NIP-98 verification
exists and is in production use in four services. NIP-VAULT already
designs key backup with recovery words, passkeys and pairing links.

What is missing is small in code and large in product decisions:

1. **One new server route** that turns a signed Nostr event into the same
   `sess_` session every client already uses (`POST /v1/sessions/nostr`).
   Because it answers the existing session shape, nothing downstream
   changes.
2. **A first-launch identity in the apps**, with the sign-in screens
   removed and the account created silently in the background.
3. **QR pairing wired into the current apps.** It is built in the crates
   but the phone has no scanner and the desktop has no QR window.
4. **Abuse controls that don't depend on GitHub.** Today the free
   allowances ($1/day of models, $3 of cloud time) are bounded because
   each account needs a GitHub or Apple identity. Keys cost nothing.
5. **Recovery.** A lost phone without backup is a lost account.
6. **Coder 1.0.0 compatibility.** The released binary uses the device-code
   flow and `sess_`/`oa_sk_` tokens. Those stay, so this is an addition,
   not a removal, until 1.0 ages out.

Estimated size: the minimum version (phases 1 and 2 below: instant start on
the phone and desktop, QR pairing) is about 4 to 6 focused agent-days. The
full version, including recovery, website sign-in, key consolidation and
retiring Apple and visitor tokens, is about 2 to 3 weeks.

The rest of this document is the evidence and the plan.

## What exists today

### Sign-in paths on the server

The account service is the gateway (`crates/gateway/src/accounts.rs`), on
the stores in `crates/tenancy`. The website (`crates/openagents-web`) never
decides who someone is; it forwards proof and receives a session.

| Path | Route | Where | Fate under Nostr-only |
|---|---|---|---|
| GitHub OAuth | `POST /v1/sessions/github`, `/auth/github/callback` | `gateway/src/accounts.rs:807`, `openagents-web/src/auth.rs:300-470` | Becomes a connection (link only); keep the route for old clients |
| Sign in with Apple | `POST /v1/sessions/apple`, `POST /v1/device/apple` | `accounts.rs:845`, `oa-auth/src/apple.rs:306` (#11980) | Drop from new apps; keep the route answering |
| Device code (RFC 8628 shape) | `/v1/sessions/device*`, `/v1/device/code`, `/v1/device/token`, `/device` page | `oa-auth/src/device.rs`, `openagents-web/src/device.rs:62-75` | **Must stay** for Coder 1.0.0 |
| API key to session | `POST /v1/sessions` with an `oak_` key | `accounts.rs:635` | Stays |
| Sign-in keys | `oa_sk_…` | `oa-auth/src/sign_in_keys.rs` | Stays (agents use them) |
| Visitor tokens | `POST /v1/visitors`, `/v1/visitors/claim` | `openagents-web/src/visitor_api.rs` (#11978) | Replaced by the first-launch key in new apps; keep the route for build 1012 |
| OAuth 2.1 for MCP clients | `/oauth/*` | `openagents-web/src/oauth.rs:108-113` | Stays; needs a website sign-in that works without GitHub |
| Agent self-registration | `/v1/agents/challenge`, `/register` | `gateway/src/agent_accounts.rs:72-79` | Stays; it is the model for keyless abuse controls |
| Workspace invitations and recovery | `inv_…`, `rcv_…` | `accounts.rs:1843-2376` | Stay |
| Email or magic link | none | — | Never needed |

The invite list (`oa-auth/src/invite.rs`) is GitHub-only. Production runs
it open since 2026-10-15, so today it only marks admins by GitHub ID
(`14167547`).

### Sessions and tokens

Every client holds an opaque bearer. The server ties it to an account.

| Token | Shape | Lifetime | Notes |
|---|---|---|---|
| Browser session | `sess_<64 hex>` | 8 hours | Cookie `oa_cloud_session` |
| App session | `sess_<64 hex>` | 30 days, no refresh | Device code, Apple and OAuth all issue this |
| Sign-in key | `oa_sk_<64 hex>` | never, 30 or 90 days | Same path as `sess_` |
| API key | `oak_…` | until revoked | Resolves through principal `key:<16 hex>` |
| Visitor | `ovis_…` | none | HMAC only, not stored |

The shape checks that matter are `openagents_login::is_sign_in_token`
(`crates/openagents-login/src/lib.rs:88`), the website's `session_token()`
(`openagents-web/src/cloud/session.rs:1115`) and the gateway's
`principal_unbound` (`accounts.rs:339`). Any new sign-in that **answers a
`sess_` token passes all three unchanged.** That is the main reason the
recommended design below mints sessions instead of verifying a signature on
every request.

### The account model already supports keys

`crates/tenancy/src/accounts.rs`:

- `Account { id: acct_<hex>, label, principals: Vec<String>, created, agent }`
  (`:137-156`).
- `valid_principal` (`:981-984`) accepts `key:`, `nostr:` (64 hex), `sso:`,
  `apple:` and `github:`.
- `account_of_principal` (`:1304`), `create_account` (`:1333`) and
  `update_principals` (`:1364`) are what a new route needs.
- Several `nostr:` principals on one account are allowed, so one person can
  have several device keys.

`docs/auth/README.md` (owner direction, 2026-10-08) already says:
"**Nostr-only accounts are first-class.** Nobody is forced to give us a
GitHub, Google, or email identity." Its P2 planned Nostr sign-in on the web
with NIP-07 and later NIP-46. No route was built.

### What keys on what

About 70 files in `openagents-web` key their data on `account_owner(acct)`
(`chat_store.rs:3052`) or on the raw account ID: chats, projects, drafts,
files, vault and memory, schedules, computers (`X-OpenAgents-Account` into
`oa-computers`), Mac jobs, custom agents, connections and GitHub repository
tokens. Billing keys on the workspace; the Stripe customer is per
workspace and no email is sent to Stripe. **None of that changes**, because
the account ID stays.

Exceptions that do key on GitHub or email, and must move to account IDs:

| What | Keyed on | Where |
|---|---|---|
| Site admin | GitHub ID in the invite list | `oa-auth/src/service.rs:178`, `gateway/src/accounts.rs:251` |
| Gmail and Drive access | email allowlist | `openagents-web/src/cloud/connections.rs:175`, `connections/access.rs:72` |
| Saving a Claude subscription token | email allowlist | `openagents-web/src/cloud/byo.rs:84,150` |
| Project invites by username | GitHub user ID | `openagents-web/src/projects/pending.rs:1-6` |
| `GET /v1/account` email | GitHub identity only | `gateway/src/accounts.rs:1555` |

### Clients

**Phone (iOS shell over `apps/openagents/crates/mobile`).** One Rust
controller, `openagents_account::Controller`
(`apps/openagents/crates/account/src/lib.rs`), has the phases SignedOut,
Restoring, Starting, Waiting (device code), SignedIn and Failed. The session
is a Keychain item, service `com.openagents.app.account`
(`account/src/platform.rs:217-235`). The screens are
`Shell/SignInViewController.swift`, `Shell/SignInSheet.swift`
(`ASWebAuthenticationSession` plus the Apple button) and the visitor
countdown and `SignInPromptController` in
`Session/CloudChatViewController.swift`. `SceneDelegate.swift:91-97` sends a
signed-out person to the visitor chat or the sign-in screen. **Everything
except the visitor chat and the demo sits behind the session**: sessions,
projects, computers, saved history, Home, custom agents, Agent Lab, sharing,
push and Our cloud.

**Desktop (GPUI).** The same controller
(`account/src/desktop.rs:84-163`), Keychain service
`com.openagents.desktop.account`, and a sidebar row
(`apps/openagents/crates/ui/src/openagents_account.rs:339-412`). The desktop already works
signed out in a local-only scope (`engine/src/lib.rs:732-752`); sync, the
computer registry, saved history and sharing need the session.

**CLIs.** `crates/openagents-login` is the shared device-code library that
writes `~/.openagents/coder/account.json` (mode 0600). Coder 1.0.0
(`crates/coder-new/src/account.rs`) uses it, and its `free_tier.rs` already
registers an anonymous agent account by proof of work when nobody is signed
in. That is our existing "start right away" precedent, built without Nostr.

**SDKs.** `crates/openagents-sdk/src/transport.rs:373,397` and
`apps/openagents/crates/client` send `Authorization: Bearer`. WebSockets
put the token in the query (`client/src/live/urls.rs:31`).

### Nostr keys a person holds today

None of them is the person's identity. Each serves one feature.

| Key | Created in | Stored | Scope |
|---|---|---|---|
| Phone push key | `apps/openagents/crates/mobile/src/push.rs:87-124` | `push.json` in the account data folder | Per account install; signs NIP-98 to the push gateway |
| Phone pairing key | `apps/openagents/crates/sync/src/paired_device.rs:122` | memory only | One per pasted connect code |
| Project-sharing key | `crates/coder-sharing/src/store.rs:140-172` | `device.key`, bound to the account ID | Sharing |
| Desktop owner, host and host-iroh keys | `crates/coder-host/src/serve/keys.rs:111-125` | Keychain `com.openagents.desktop` | Only with `OPENAGENTS_NIP_HOST=1` (`engine/src/lib.rs:984`) |
| Computers-client device key | `crates/coder-computers/src/live.rs:228-245` | `device.key` | CLI and desktop Computers client |
| Chat device key | `crates/openagents-chat/src/client.rs:3623` | `~/.openagents/chat/device.key` | Local chat host |
| Verse player key | `crates/verse-net/src/identity.rs` | `~/.openagents/verse/<profile>.key` | Verse |
| Wallet seed (BIP-39, not Nostr) | `crates/spark-wallet` | `~/.openagents/spark/seed`, Keychain on the phone | Shared wallet |

A person using the phone, desktop and CLI has 8 to 10 separate secrets and
no shared npub. The old phone app (removed in #11901) did what the owner is
asking for: it made a device key on first launch, needed no sign-in, opened
on a chat, and showed the npub with **Reveal nsec**
(`docs/mobile/retired-ios.md`, `INVARIANTS.md:325-331`). Its rows in
`INVARIANTS.md` still cite tests in the removed `crates/openagents-mobile`.

### Nostr building blocks

| Building block | Status | Where |
|---|---|---|
| NIP-01 events, signing, verification | Exists | `crates/nostr/src/domain/event.rs`, `expanded.rs:12` |
| NIP-42 on the relay | Exists, with NIP-AA owner-attested agents | `crates/nostr-relay/src/gateway/auth.rs`, `server.rs:1150-1245` |
| NIP-42 client | Exists | `crates/nostr-transport/src/lib.rs:105` |
| NIP-98 verification | Exists; each server adds its own replay store | `crates/nostr/src/domain/expanded.rs:58-150` |
| NIP-98 in production | push gateway, coder-sync device proofs, verse-private, x402 | `push-gateway/src/nip98.rs`, `coder-sync/src/server/proof.rs`, `coder-access/src/http_proof.rs` |
| NIP-98 as the only credential | **Missing**; always paired with a bearer | `coder-sync/src/server/auth.rs` |
| Sign-in route for a pubkey | **Missing**; principal type exists | `tenancy/src/accounts.rs:984` |
| NIP-44 | Exists | `crates/nostr/src/nip44.rs` |
| NIP-19 | Partial (npub, nsec, naddr) | `crates/nostr/src/nip19.rs` |
| NIP-46 remote signer | Protocol exists; used only by `sov-custody` | `crates/nostr/src/domain/remote_sign.rs` |
| NIP-49 encrypted key export | Missing | — |
| NIP-06 key from words | Missing (path string only) | `crates/nostr/src/lane.rs:110` |
| Owner attests a key (NIP-OA) | Exists; used for agents, not user devices | `crates/nostr/src/domain/agent.rs:55-123` |
| Device grants, rights, epochs, revocation, delegation | Exists | NIP-HOST, `crates/coder-access` |
| Single-use rotating QR pairing | Exists in crates; unused by the current apps | `crates/openagents-connect/src/code.rs`, `crates/coder-computers` (`connect.rs`, `qr.rs`) |
| Reverse enrollment for a headless computer | Exists | `crates/coder-host/src/enroll.rs` |
| Sealed transfer of a secret by QR and code | Exists for the wallet seed | `crates/spark-wallet/src/link.rs`, `crates/coder-access/src/wallet_link.rs` |
| Key backup slots (device, passkey PRF, recovery words, pairing) | Exists for vault data, not identity | `crates/oa-vault/src/slot.rs`, `recovery.rs`, NIP-VAULT |
| iCloud Keychain sync | Missing; every key is this-device-only | `bins/coder-ios/host/App/DeviceIdentity.swift:19` |
| Pay with no account (x402 per request) | Exists | `crates/x402` |
| Credits for a key-only person | Partial; Lightning top-ups exist, keyed by workspace | `gateway/src/funding.rs:657-669` |

### Computer pairing today

The phone finds computers through the account-keyed registry room, with
the bearer in the query string (`client/src/live/urls.rs:31`), listed under
Settings > Devices. **Without an account no computer appears.** The
Nostr-based device channel exists, but on the phone it is clipboard-only
("Paste connect code", `Shell/MoreViewController.swift:288-309`). It still
requires a registry entry first (`paired_device.rs:117,190-195`) and keeps
its grant in memory only. On the desktop the NIP-HOST host is off unless
`OPENAGENTS_NIP_HOST=1`, invitations go to a file, and there is no QR
window. `openagents connect invite` does print a QR code in a terminal.

NIP-HOST (`nips/openagents/NIP-HOST.md`) already defines what the owner is
asking for:

- Connect codes are `https://openagents.com/connect#<payload>` with the
  host key, iroh address, a one-time 32-byte capability and a 300-second
  expiry (`:148-228`).
- They rotate every 60 seconds and are cancelled when the window hides
  (`:232-248`).
- The first redemption binds; later ones are refused (`:276-310`).
- Reverse enrollment shows an 8-character code for a headless machine
  (`:405-470`).
- Grants carry rights, a per-device revocation epoch, renewal and
  narrowing delegation (`:620-700`).

`INVARIANTS.md` (pairing rows, lines 103-127) already says a QR scan "only
introduces devices. Rights come only from a host-signed NIP-HOST grant."

## Recommended design

### Identity

- **One identity key per person, held by the first device.** On first
  launch the phone (or desktop) generates a secp256k1 key and stores it in
  the Keychain. Its npub is the person's ID everywhere a person sees one.
- **Keep `acct_` as the server's internal ID.** The first sign-in creates an
  account whose principal is `nostr:<hex>`. This keeps every store, cache and
  room unchanged, lets one person hold several keys, and allows key
  rotation without moving data. People never see `acct_`.
- **Each additional device has its own key**, linked to the same account by
  a QR approval from a device that is already linked (see
  [Multi-device](#multi-device)). The identity key is never copied between
  devices by default. Losing one device means revoking one key, not
  replacing the identity.

### Authenticating to our servers

**Recommendation: mint a session from a signed event, then use the session
as today.**

1. `GET /v1/sessions/nostr/challenge` answers a random challenge with a
   short expiry.
2. The client signs a NIP-98 event (kind 27235) over
   `POST /v1/sessions/nostr` with the challenge as a tag and the body hash.
3. The account service verifies the signature, the URL, the method, the
   time window and the challenge. It burns the event ID, then finds or
   creates the account holding `nostr:<hex>`, creates its personal
   workspace on `signup_tenant`, and answers the same 30-day `sess_` app
   session the device flow issues.
4. The client stores the session and renews it by signing again before it
   expires. No refresh token, no password, no browser.

Why not verify NIP-98 on every request: it touches every place that checks
the token's shape (`session_token()`, `coder_sync::account`, `chat_owner`,
`computer_account`, `principal_unbound`, the WebSocket query token), and
WebSockets need a second mechanism anyway. Minting a session is one route.
Per-request NIP-98 can come later as a third bearer kind
(`Authorization: Nostr <base64 event>`) where it earns its keep, for example
for agents that hold only a key.

NIP-42 stays where it belongs, on the relay. Optionally, link the relay's
NIP-42 pubkey to the account so relay admission and plan limits line up
(`docs/pro/nostr.md` already assumes that mapping).

### Connected accounts

GitHub, Apple, Google and any later provider become **connections** on the
account, never a way to sign in from the new apps:

- GitHub linking exists (`POST /v1/account/identities/github`,
  `accounts.rs:904`). It is still needed for repositories, issues and pull
  requests. The GitHub repository tokens already key on the account
  (`oa-auth/src/repos.rs`).
- Gmail, Drive and Calendar stay connections; their email allowlist moves
  to account IDs.
- **Drop Sign in with Apple from the new apps.** App Store guideline 4.8
  requires an equivalent Apple option only when an app uses a third-party
  login *to set up or authenticate the primary account*. With the key as the
  account and GitHub only as a connection for repository access, that rule
  should not apply. Confirm the current wording before App Store submission
  (TestFlight is unaffected). Keep the server route answering for build
  1012.
- A connection can also serve as a **recovery method** (see below), which is
  the one place a provider identity helps sign-in, and only when the person
  chooses it.

### Multi-device

The phone has the camera, so **the phone scans and the desktop shows the
code.** One scan does two things:

1. **Pairs the computer** with a NIP-HOST grant, as designed today.
2. **Links the computer's key to the person's account**: the phone signs an
   approval naming the desktop's pubkey, and the server adds it as another
   `nostr:` principal. The desktop then mints its own session with its own
   key.

The QR payload is the existing `openagents.com/connect#…` code plus the
desktop's account-link request. A headless computer uses reverse
enrollment (the 8-character code) or `openagents connect invite`, as today.

A second phone joins the same way: the old phone shows a link code and the
new phone scans it. Without the old phone, use recovery.

For people who want one key everywhere (power users, existing Nostr users),
offer **Use my existing key**: import an nsec, or connect a NIP-46 bunker
(the protocol is in `crates/nostr/src/domain/remote_sign.rs`).

### Recovery

Without recovery, a lost phone is a lost account. Offer these, in order:

1. **iCloud Keychain sync for the identity key**
   (`kSecAttrSynchronizable: true`), on by default. A new iPhone on the same
   Apple account restores it with no steps. This is a product choice: it
   trades key exposure to Apple's sync for not losing accounts. Recommend
   on, with a setting to keep the key on this device only.
2. **Another linked device.** Any linked desktop can approve a new phone.
3. **Recovery words** (24 words) and **a passkey**, as the NIP-VAULT slots
   already design (`crates/oa-vault/src/slot.rs`, `recovery.rs`). Show the
   prompt after the account holds something worth keeping (first credit
   purchase, first connected computer), not at first launch.
4. **A connected GitHub or Apple account as a recovery method**, chosen by
   the person: signing in with it adds a new key to the account it is
   connected to. This is the only sign-in role a provider keeps.
5. **Export**: Reveal nsec and NIP-49 encrypted export, under the
   `INVARIANTS.md` rules (explicit reveal, 60-second pasteboard expiry, no
   screenshots).

### Abuse controls without GitHub

This is the largest real risk. Today the free allowances are bounded by
"one per GitHub or Apple identity." A key costs nothing, so anyone can
create unlimited accounts and multiply:

- the free daily model allowance (`gateway/src/inference_public.rs:13-22`,
  about $1/day),
- the $3 free cloud allowance (`oa-computers/src/usage.rs:18-35`, #11981),
- the 10 visitor messages, if they survive.

Controls to add, cheapest first:

| Control | What it bounds | Exists? |
|---|---|---|
| **App Attest** on the phone's first sign-in (`DCAppAttestService`), verified by the server | One account per genuine app install; blocks scripts | Missing |
| Proof of work plus per-IP, per-/24 and global daily caps on new accounts | Script farms | Exists for agents (`agent_accounts.rs`); reuse |
| A global daily circuit breaker on free spend | The worst case | Exists for free agents ($2/day); add one for free people |
| Free model allowance per account, small and daily | Model cost | Exists |
| **Free cloud time only after App Attest or a connected GitHub account, or a payment** | Cloud cost, the expensive part | Missing |
| Relay rate limits per pubkey | Relay spam | Exists (`nostr-relay/src/gateway/rate.rs`) |

With App Attest plus the existing agent caps, the free tier is about as
hard to farm as it is today with "make a GitHub account." The $3 cloud
allowance is the one to gate harder, because a farmed account costs real
money.

### Website sign-in

The website still needs a signed-in browser for `/device` (Coder 1.0's
approval page), `/oauth/authorize` (MCP clients), Settings and billing. Add
two ways in that need no provider:

1. **Scan with your phone**: the website shows a QR code; the phone scans
   it, signs the website's challenge, and the browser session is issued.
   This is the main path for most people.
2. **NIP-07 browser extension** (Alby, nos2x) for people who already use
   Nostr.

Keep GitHub sign-in on the website for existing accounts, presented as
"Sign in with a connected account."

### Coder 1.0.0 compatibility

The released binary (`3230ded511`) needs, unchanged: `/v1/device/code`,
`/v1/device/token`, `/v1/device/sign-out` and their old `/device/*`
aliases; the `/device` approval page; `sess_` and `oa_sk_` bearers;
`GET /v1/session`, `GET /v1/account`, `GET /v1/keys`, `POST /v1/keys`;
`/v1/agents/register`; the error codes `authorization_pending`,
`slow_down`, `access_denied`, `expired_token`, `invalid_grant`,
`invite_only`, `signup_disabled` and `quota_exhausted`; and the
`account.json` format. Every change in this plan is additive, so all of
these keep working. A Nostr-only person approves a 1.0 device code through
the website's phone-scan sign-in. The next Coder release can sign in with a
local key and skip the device code (`docs/coder/next-release.md`).

### Account deletion

App Store guideline 5.1.1(v) requires in-app account deletion for any app
that creates an account. Nothing implements it today
(`docs/security/sensitive-data-vault.md:88`). A key-based account makes
this more urgent, not less, because the app creates an account at first
launch. Add `DELETE /v1/account` (deletes the account's chats, computers,
projects, connections and keys, with the existing export first) and a
Settings row.

## What changes, by component

### Server

| Change | Where | Size |
|---|---|---|
| `GET /v1/sessions/nostr/challenge`, `POST /v1/sessions/nostr`: NIP-98 to `sess_`, find or create `nostr:` account, replay store | `crates/gateway/src/accounts.rs`, `crates/oa-auth` (new `nostr.rs`), `crates/tenancy/src/accounts` | 1 day |
| Website forwarder for the route (`/v1/device/nostr`, like `/v1/device/apple`) | `crates/openagents-web/src/device.rs`, `cloud/session/device.rs` | small |
| `POST /v1/account/keys` (add a pubkey, signed by a linked key or session), `DELETE` (revoke), list | `gateway/src/accounts.rs`, `tenancy` `update_principals` | 1 day |
| App Attest verification at first sign-in; PoW and IP caps from agent registration | new `oa-auth/src/attest.rs`, `agent_accounts.rs` patterns | 1-2 days |
| Free cloud allowance gated on attest, connection or payment; global free-spend breaker | `oa-computers/src/usage.rs`, `gateway/src/inference_public.rs` | 1 day |
| Site admin by account ID or pubkey instead of GitHub ID | `oa-auth/src/invite.rs`, `service.rs:178`, `accounts.rs:251` | small |
| Email allowlists to account IDs | `cloud/connections.rs:175`, `cloud/byo.rs:84` | small |
| Project invites by npub as well as GitHub username | `projects/pending.rs` | small |
| Website: phone-scan sign-in and NIP-07 sign-in | `openagents-web/src/auth.rs` | 2 days |
| `DELETE /v1/account` | `gateway`, `openagents-web`, `oa-computers` | 1-2 days |
| GitHub as recovery: signing in with a connected GitHub adds a key to its account | `accounts.rs` `github_sign_in` | small |

### Shared Rust (account, SDK, client)

| Change | Where |
|---|---|
| `Identity` type: generate, load, store in Keychain (synchronizable by default), sign NIP-98 | `apps/openagents/crates/account` (new module), reusing `crates/nostr` and `coder_access::http_proof` |
| `Controller`: on launch, load or create the identity, then sign in silently; `Phase::Waiting` and the device code leave the app path | `apps/openagents/crates/account/src/lib.rs` |
| SDK: `sign_in_with_key(identity)` beside `sign_in_with_apple` | `crates/openagents-sdk/src/auth.rs` |
| Retire the visitor token and claim in the new app | `crates/openagents-sdk/src/visitor.rs`, `account/src/cloud_chat.rs` |
| Pairing: keep the grant in the Keychain, and let a paired computer appear without a prior registry entry | `apps/openagents/crates/sync/src/paired_device.rs:117,190-195` |
| Later: derive the push, sharing and chat device keys from (or attest them with) the device's identity key | `apps/openagents/crates/mobile/src/push.rs`, `crates/coder-sharing/src/store.rs`, `crates/openagents-chat` |

### Phone (iOS)

- Remove `SignInViewController`, the sign-in sheet, the Apple button, the
  visitor countdown and `SignInPromptController`.
- `SceneDelegate` always opens the shell; the first screen is a chat.
- Add a QR scanner (AVFoundation) that feeds
  `coder_computers::connect::classify`, plus a universal-link handler for
  `https://openagents.com/connect#…`.
- Settings: **Your identity** (npub, copy, Reveal nsec, back up with words or
  passkey), **Linked devices** (list, revoke), **Connected accounts**
  (GitHub, Google), **Delete account**.
- App Attest at first sign-in.

### Desktop (GPUI)

- The sidebar account row becomes an identity row (npub, connected
  accounts, linked devices).
- On first launch, create the identity. The natural key to promote is the
  owner key `coder-host` already creates, because it already signs the host
  directory.
- Turn the NIP-HOST host on by default (drop the `OPENAGENTS_NIP_HOST=1`
  gate) and add a **Connect your phone** window that shows the rotating QR
  code from `device_channel_invitation`.
- `configure_sync` signs in with the key; the bearer comes from the minted
  session.

### CLIs

- `openagents identity create|show|import|export`, and the key-based sign-in
  in the shared login library, writing the same `account.json`.
- Coder 1.0.0: no change; the server keeps its routes.

### Docs and protocol

- `docs/auth/README.md`: replace "GitHub first" with key first; record the
  route and the recovery methods.
- A NIP-HOST or new NIP section for "link a device key to an account by
  QR" (the approval event).
- `INVARIANTS.md`: point the device-identity rows at the current app and its
  tests; add rows for the identity key's storage and sync.
- `docs/coder/next-release.md`: key-based sign-in for the next Coder.

## Plan

Each step is issue-sized and closes on code with its own tests, per the
repository rules.

**Phase 1: instant start (the owner's core request).**

1. Server: `POST /v1/sessions/nostr` with challenge, replay store and
   find-or-create, answering `sess_`. Tests: valid signature signs in and
   creates once; wrong URL, method, time, challenge or signature is refused;
   replay is refused.
2. Server: App Attest at first sign-in, plus the agent-registration caps for
   key accounts; gate the free cloud allowance on attest, connection or
   payment.
3. Shared Rust: the `Identity` type and silent sign-in in `Controller`; the
   SDK call.
4. Phone: remove the sign-in screens and the visitor flow; open on the shell
   with a chat; Settings > Your identity with Reveal nsec.
5. Desktop: identity on first launch and silent sign-in; identity row.

**Phase 2: QR-first pairing.**

6. Desktop: NIP-HOST on by default and the Connect your phone QR window.
7. Phone: QR scanner and universal link; persistent grants; computers
   appear from pairing alone.
8. Server: `POST /v1/account/keys` and the link-on-scan approval, so a
   paired desktop signs in as the same person.

**Phase 3: recovery and the website.**

9. iCloud Keychain sync on by default, with a setting; recovery words and
   passkey on the NIP-VAULT slots; NIP-49 export.
10. Website: phone-scan sign-in and NIP-07; `/device` and `/oauth` work for
    key-only people.
11. Connected GitHub or Apple as an optional recovery method.
12. `DELETE /v1/account` and its Settings row.

**Phase 4: cleanup.**

13. Admin, email allowlists and project invites move to account IDs or
    npubs.
14. Consolidate the per-feature device keys under the device's identity
    key.
15. When build 1012 and Coder 1.0.0 age out, stop advertising Apple
    sign-in and visitor tokens. Keep the routes answering while any
    released client calls them.

Phases 1 and 2 are the owner's request. Phase 3 is what makes it safe to
leave people on it.

## Risks and open decisions

| Risk | Mitigation |
|---|---|
| Farmed accounts drain free allowances | App Attest, agent-registration caps, a global breaker; gate free cloud time harder |
| Lost phone, lost account | iCloud Keychain sync by default; linked devices; recovery words or passkey; connected GitHub as recovery |
| Stolen key is permanent identity theft | Several keys per account with revocation; sessions expire; Linked devices shows and revokes each key |
| Existing accounts orphaned | A signed-in app adds its new key to the existing account on first launch of the new build (call `POST /v1/account/keys` with the old session); never auto-link by email |
| Paid credit stranded by a lost key | Prompt for backup before or at first purchase |
| App Review | Account deletion is required either way; confirm guideline 4.8 no longer applies once GitHub is only a connection |
| Monday's build already uses Apple and visitor sign-in | Ship build 1012 as is; land phase 1 after it; the server stays compatible with both |

Decisions for the owner:

1. **iCloud Keychain sync for the identity key: on by default?** Recommended
   yes, with a this-device-only setting.
2. **Free cloud time for a brand-new key: after App Attest alone, or only
   after a connection or payment?** Recommended: App Attest plus a smaller
   first allowance, the full $3 after a connection or payment.
3. **Drop Sign in with Apple from the next build?** Recommended yes, keeping
   the route for build 1012.
4. **One key everywhere, or one key per device linked to the account?**
   Recommended one per device, with "Use my existing key" for Nostr users.
5. **Timing.** Recommended: Monday ships build 1012 unchanged; phase 1
   starts right after and is the next TestFlight build.
