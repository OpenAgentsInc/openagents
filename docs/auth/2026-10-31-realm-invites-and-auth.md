# Realm invites and accounts

Status: spec, and a first build behind two flags that are off
(2026-10-31). Nothing is deployed. Owner's ask: only approved users make
realms; add user authentication, probably WorkOS with Nostr.

Code: `coax/web/api/access.c`, `coax/web/api/migrations/0090_realm_invites.sql` and `0091_access_times.sql`,
the gate in `realms.c`; checks in `coax/web/api/test_access.py`. Pages:
`builder.coax` (the invite screen), `private.coax` (`/invite`), the admin
access view, and the MAKE A REALM links in `realmlist.coax`, `verse.coax`,
`realm.coax`, `myrealm.coax`.

## 1. What exists today

| Piece | How it works |
| --- | --- |
| Guest player | `POST /api/playtest/player` mints a random key per browser; pages send it as `X-Verse-Player`; only its SHA-256 is kept (`verse.players`). A guest owns the realms it makes (`verse.worlds.owner_player`). |
| Accounts | `verse.accounts`: `n-<pubkey>` for a Nostr key, `e-<hash>` for an email. One account has one key. |
| Email join | `POST /api/playtest/join`: the email is never verified (a format check). It is not proof of anything. |
| `/login` QR | `signin.challenges`: the page shows a one-time challenge; the iOS app approves it with a NIP-98 request; the page's poll gets `oa_session` (30 days, `signin.sessions`, keyed by pubkey) and the guest key moves to `n-<pubkey>` with its realms (0022). |
| Words account | `/login` makes 16 bytes of entropy in the browser, derives 12 words and a NIP-06 key in Coax, and signs `POST /api/auth/key` (0067). Same cookie, same move. |
| NIP-98 | `nip98.coax` checks every signed request (kind 27235, URL, method, payload hash, 60 s, each id once). Agents, the iOS app and tools use it instead of cookies. |
| Makers | An admin mints a one-time claim code (only its hash kept); `/realms/<realm>/claim#code=` logs in like `/login` and binds the key, a name and a Lightning address. |
| Admins | Pubkeys in `admins.conf` (`OA_ADMIN_PUBKEYS`). `GET /api/admin/login?auth=` (NIP-98) sets `oa_admin` (12 hours); an admin's `oa_session` counts too (`admin_who`). |
| Playtest | `/verse` takes an email (`/api/playtest/join`) or ENTER as a guest. No login is needed to make a realm. |

Gaps: anyone can make realms; one account cannot hold two keys; an email
is never verified; there is no way back into an account if the words are
lost and no phone was linked.

## 2. Identity model

- **An account** is the person. Its id stays what it is today (`n-<hex>`),
  so nothing existing moves. New accounts from WorkOS are `w-<hash>`.
- **Identities** (`auth.identities`, kind and subject) point at one
  account each:
  - `nostr`: a pubkey, from the words account (NIP-06), a NIP-07 browser
    extension, a NIP-46 remote signer (bunker), or the phone app's QR.
  - `workos`: an AuthKit user id, with WorkOS's verified email. AuthKit
    gives email, Google and enterprise SSO behind one integration.
- A key with no identity row is its own account, `n-<pubkey>`, exactly as
  today (`auth.account_of`).
- **Guest upgrade:** logging in by any identity moves the guest key and its
  realms to the account, as `/login` does now. Nothing is lost.
- **Linking:** while logged in, the new key signs `POST /api/auth/v2/link/nostr`
  (NIP-98) and joins the session's account. A WorkOS login while logged in
  links that WorkOS user to the account. A key or WorkOS user that is already
  another account's is refused (409); merging two accounts is not built.
- **Recovery:** any linked identity gets you back in. The expected setup is
  words or the phone key plus a WorkOS email. Losing all of them loses the
  account (we hold no keys).

## 3. Sessions

| Client | Today | With auth v2 |
| --- | --- | --- |
| Web | `oa_session` cookie (HttpOnly, Secure, Lax, 30 days) from the QR or the words key; the hash of its token is kept | The same cookie for Nostr logins. A WorkOS login gets an `auth.sessions` row (by account, `via workos`); `site_identity` reads both (to wire up) |
| iOS | Its Keychain key signs NIP-98 per request | Unchanged. It can approve the web QR and be linked as an identity |
| Agents and tools | NIP-98 per request, each event once | Unchanged. A linked agent key acts as its account. Admins stay pubkeys |

Writes from cookies need the site's `Origin`. Signed writes must bind
their body (payload tag) and are good once.

## 4. Realm-creation authorization

- **Flag** `realm_invites` (a row in `access.flags`; no row is off).
- **Approved creator** (`access.creators.who`): an account, or a guest
  player id when grandfathered. Site admins always may.
- **Gate:** with the flag on, `POST /api/playtest/realms/plan`, `/pick`,
  `/plan/doc`, `POST /api/playtest/realms` (MAKE IT) and
  `POST /api/playtest/realms/<slug>/fork` answer 403
  `{"error":"invite only","need":"invite"}` (`invite only` with `?as=lines`)
  unless the requester's player key, its account, or the logged-in key's
  account is a creator. Editing, revising and visiting an existing realm are
  not gated.
- **Invites:** an admin mints a code (32 hex; 1 to 1000 uses, default 1;
  1 to 90 days, default 14; a note). The link is
  `https://openagents.com/invite#code=<code>`: the code stays in the hash,
  so it never reaches a server log or a Referer. Only its SHA-256 is kept. A
  use is spent in the same statement that checks it; a creator redeeming
  spends none. Unknown, used up, expired and revoked are one answer.
  Admins revoke any time.
- **Waitlist:** a logged-in, not approved user sends a note (what they'd
  make, up to 280 characters) and an optional email. One waiting request an
  account; asking again rewrites it. Admins approve or decline once.
- **Grandfathering:** realms already made stay their owners'. GRANDFATHER
  (admin) makes everyone who owns a made realm a creator: the account, or
  the guest player id when the guest never logged in.
- **Pages:** `GET /api/playtest/access` says `invites`, `make`, `in`,
  `creator`, `asked`. With `make 0` the MAKE A REALM links are hidden and
  `/realms/new` shows the invite screen (code box, request form). Any error
  reads as `make 1`, today's behaviour; the server still enforces.
- **Text copies:** `/realms`' markdown copy (`realms/index.md`, from
  `coax/web/realms.ts`) links to `/realms/new`. Turning the flag on or off
  writes the marker `realms/invite-only.txt` and the copy without or with
  that link into the site's bucket (`access.c`); `coax/web/upload.py` keeps
  the marker and drops the link from every release's copy while it says
  `on`. Its HTML copy has no such link.

## 5. API

| Route | Who | Answer |
| --- | --- | --- |
| `GET /api/playtest/access` | anyone | lines; off: `invites 0`, `make 1` |
| `POST /api/playtest/access/redeem {code}` | logged in | `creator 1`; 401, 404, 429 |
| `POST /api/playtest/access/request {note, contact?}` | logged in | `asked waiting`; 400, 401, 429 |
| `GET /api/admin/access` | admin | rows `F` flags, `I` invites, `R` requests, `C` creators; each ends with its time in Chicago (0091) |
| `POST /api/admin/access/flag {name, on}` | admin | `realm_invites`, `auth_v2` |
| `POST /api/admin/access/invites {uses?, days?, note?}` | admin | `{id, code, link}`, the code once |
| `POST /api/admin/access/invites/<id>/revoke` | admin | 404 none |
| `POST /api/admin/access/requests/<id>/approve\|decline` | admin | 409 decided |
| `POST /api/admin/access/creators {who}`, `.../creators/<who>/remove` | admin | a pubkey or an account id |
| `POST /api/admin/access/grandfather` | admin | `{approved: N}` |
| `GET /api/auth/v2/me` | logged in, or NIP-98 signed | `account ID`, `id\|nostr\|npub…`, `id\|workos\|email`; answers with the flags off too |
| `POST /api/auth/v2/device/start {machine}` | the new key's NIP-98 | `{id, code, expires_ms}`; 400, 401, 429 (five open a key) |
| `GET /api/auth/v2/device/<id>` | that key's NIP-98 | `state waiting\|approved\|expired`, `account ID` when approved; 404 for another key |
| `POST /api/auth/v2/device/look {code}` | anyone (same site) | `{machine, state}`; 404 |
| `POST /api/auth/v2/device/approve {code}` | logged in | the key joins the account as `cli:MACHINE`; 401, 404, 409 approved or another account's, 410 expired |
| `POST /api/auth/v2/unlink {pubkey}` | a key of the account | 404 not this account's, 409 the account's own key |
| `POST /api/auth/v2/link/nostr {"kind":"link"}` | session + the new key's NIP-98 | 401, 409 |
| `GET /api/auth/v2/workos/start` | anyone | 302 to AuthKit (`provider=GoogleOAuth`); 503 without `OA_WORKOS_CLIENT_ID` |
| `GET /api/auth/v2/workos/callback?code&state` | WorkOS | state spent once; the code exchange answers 501 (not built) |

`GET /api/auth/v2/me` answers whatever the flags say, so the CLI can ask
who its key is (`openagents whoami`). With a flag off, its POSTs and the rest
of `/api/auth/v2/*` fall through to the routes
that answered those paths before (404). The admin routes work with the flags
off, so invites can be prepared first.

**NIP-98 callers** (`docs/cli/2026-11-01-openagents-cli-v1.md`, 0098). A
request signed with NIP-98 (kind 27235, `u`, `method`, and `payload` on every
write) is its key's account on `/api/playtest/realms` and `/api/auth/v2/me`.
Each signature is good once (`projects.seen`) and within 60 s. A bad one is
401, never the player key. The signer acts through a player key the database
derives for its account (`auth.caller_key`): never stored or sent, so the
realm rules are unchanged. `GET /api/playtest/realms/mine` lists the caller's
made realms (`?as=lines`: `realm SLUG|REV|PUBLISHED|LISTED|NAME`).

**Data** (0090, additive: new schemas `access` and `auth`; no existing
table, column, constraint, row or function changes): `access.flags`,
`access.creators`, `access.invites`, `access.redemptions`,
`access.requests`, `auth.identities`, `auth.workos_states`,
`auth.sessions`. When the protocol's `openagents.account.v1` and
`credential.v1` (linked identity, invitation) get an API surface, these are
their rows; the proto does not change for this build.

**Security.** Codes, session tokens and WorkOS states are kept as SHA-256
only and never logged. Redeem is limited to 10 tries a minute per address
(codes are 128 bits). The waitlist holds at most `OA_ACCESS_WAITING` (2000)
waiting requests. The WorkOS API key is a secret (`OA_WORKOS_API_KEY`, Secret
Manager); the client id is not. The invariant is in `INVARIANTS.md`
("Realm creation is gated by one server-side rule").

## 6. Rollout

1. Deploy as usual: the migration applies and nothing changes (flags off).
2. An admin mints invites and runs GRANDFATHER if wanted (`/admin/access`).
3. An admin turns `realm_invites` on: the links hide, `/realms/new` shows
   the invite screen, the gate answers 403.
4. WorkOS: create the account, add the secret and env (below), build the
   code exchange (`workos_exchange` in `access.c`: one POST to
   `/user_management/authenticate`), wire `auth.sessions` into
   `site_identity`, add LOG IN WITH GOOGLE to `/login`, then turn `auth_v2` on.
5. Off is one admin request: `realm_invites` off restores today's behaviour;
   approvals are kept.

## 7. Owner decisions still needed

1. **WorkOS:** create the account and an AuthKit environment (staging and
   production). Send the client ids; put the API keys in Secret Manager as
   `oa-workos-api-key-staging` and `oa-workos-api-key` (never in chat).
2. **Redirect URIs** to register in WorkOS:
   `https://staging.openagents.com/api/auth/v2/workos/callback` and
   `https://openagents.com/api/auth/v2/workos/callback`.
3. **Connections:** Google only at first (`GoogleOAuth`), or AuthKit's whole
   page (email, Google, SSO: `OA_WORKOS_PROVIDER=authkit`)?
4. **Who gets invites,** and how many uses each; who on the team may mint
   (today every admin in `admins.conf`).
5. **Grandfather** everyone who already made a realm, or not?
6. **Guests:** should a guest with a code be allowed to redeem without
   logging in (approving only that browser's key)? Built: login required.
7. **Waitlist contact:** keep the optional email box, or Nostr DMs only?
   Should approval send an email (Resend is wired, `email.c`)?
8. **Account merging:** when two existing accounts meet (two keys that each
   made realms), merge them, or keep refusing?
9. **Other world creation** stays open: `POST /api/playtest/worlds` (the old
   editor's worlds) and Everglade replies. Gate them too?
