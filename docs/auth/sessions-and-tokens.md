# Sessions and tokens

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).

## Session model

A **session** is a server-side row: account, the authenticator that
created it, the device (if any), assurance level, `auth_at` per level,
created, last seen, expiry, and a coarse location (country and network
from the request, kept 30 days). Every token we hand out is an opaque
random string whose SHA-256 is the row's key. Revoking means deleting the
row, and it takes effect on the next request. We don't use JWTs: we run
one API, so self-contained tokens would only buy us revocation delays.

| Client | Credential | Lifetime | Rotation |
| --- | --- | --- | --- |
| Web | `__Host-oa_session` cookie | 30 days idle, 180 days absolute | New token every 24 hours of use; the old one works for 60 more seconds (parallel tabs) |
| iOS, Mac, Android apps | `oa_at_` access token + `oa_rt_` refresh token, refresh bound to a device key | Access 1 hour; refresh 90 days idle | Each refresh is single use and returns a new one |
| CLI, tools, agents with keys | NIP-98 per request | No session | — |
| Agents with API keys | `oa_key_` | Until the grant expires | Owner rotates |
| Rooms (Everglade, Verse) | Signed seat ticket from the API | Minutes | Re-minted on join |

### Native: refresh tokens bound to a device key

Built in #13216 (`coax/web/api/auth-device.c`, rules in
`auth-device-policy.coax`, the proof's bytes in `auth-device-crypto.coax`,
migration 0153).

On first login, the app makes a **device key**: P-256, non-exportable, in
the Secure Enclave on iPhone and Mac, in Android Keystore (StrongBox when
present). The Secure Enclave and Keystore support P-256 but not
secp256k1, which is why the device key is a separate key from any Nostr
key. The key is made silently: it asks for no Face ID, passcode or
password. One key lasts one login; logging out forgets it.

**At login** the app sends the public key with the request that logs it
in, whichever way it logs in:

```text
OA-Device-Key: <130 hex>            04 || x || y
OA-Device-Platform: ios | mac | android
OA-Device-Store: enclave | strongbox | tee | software
OA-Device-Name: iPhone 17 Pro       up to 80 printable ASCII characters
```

The session is bound to that key before any token is made, and the answer
says `"device_bound": true`. A session's device is set once: the database
refuses to change or clear it. A login that sends a key we can't hold (not
a P-256 point, an unknown platform) is refused; it is never quietly left
unbound. A login that sends no key (the web, an older build) is unbound and
refreshes as before.

**At every refresh** of a bound session, `POST /api/auth/refresh` must
carry a signature by the device key (the shape of OAuth DPoP, RFC 9449,
with our own header):

```text
OA-Device-Proof: v1 <ms> <signature hex>

signed (ES256, SHA-256 inside), one part a line:
  OA-Device-Proof-v1
  POST
  https://openagents.com/api/auth/refresh
  <ms: the device's time, milliseconds since 1970>
  <SHA-256 of the refresh token, hex>
```

The signature is `r || s` (64 bytes) or DER, as the hardware gives it. The
time must be within 60 seconds of ours, and each proof works once. A
refusal changes nothing: the token stays good for the real device, the
audit log gets a `refresh.proof` row, and the answer says why in
`device_proof`:

| `device_proof` | Status | Means |
| --- | --- | --- |
| `none` | 401 | No proof: the token is somewhere other than its device |
| `malformed`, `signature` | 401 | Not a proof, or not this session's key over this request and token |
| `clock` | 400 | More than 60 seconds off; the answer carries our `now_ms`, and the app signs again with it |
| `replay` | 409 | This exact proof was already used |
| `removed` | 401 | The device was removed from the account |

A refresh token copied off the device, from a backup or a log, is useless
without the key that never left the hardware. The database enforces it
too: `auth.app_refresh` moves a bound session's token only when told which
device proved itself, and only if that is the session's own live device.

**No hardware.** The iOS Simulator, an Intel Mac without a T2 chip and a
virtual machine have no Secure Enclave. There the app makes an ordinary
P-256 key and says `OA-Device-Store: software`, so the devices list never
claims hardware that isn't there. We ask for no attestation, so the store
is the app's word.

**Where the app keeps it.** iOS keeps the Secure Enclave's wrapped blob in
the Keychain beside the tokens. The Mac keeps it in a file only the user
can read (`~/.openagents/<app>/device-key`, mode 0600), never in the
Keychain: a rebuilt debug app has a new code signature, and a Keychain
item would ask for the login password on every build. The blob only works
in that Mac's own enclave. Android keeps the key in the Keystore under an
alias. The shared code is `apps/shared/host/apple/DeviceKey.swift` and
`apps/shared/host/android/device-key.c`.

**A device key is not a way to log in.** It binds its own login's refresh
tokens and nothing else. It doesn't count as a stronger way in, and it
doesn't raise a session's level: an email-only person who logs in on a
phone keeps a full session on the web. Spend step-up (#13217) asks for the
passkey, which the phone's system verifies with Face ID and our server
checks; a device key proves which phone a login is on, not who holds it.

**Threats and their checks** (`coax/web/api/tests/auth_device_checks.coax`,
and the rules' own checks in `auth-device-policy.coax`):

| Threat | What happens | Check |
| --- | --- | --- |
| A token copied to another device | No proof, or another key's proof, refreshes nothing; the audit log records it; the real device's token still works | "a copied token: ..." |
| A proof replayed | Refused as `replay`; the login lives | "a replayed proof is refused as a replay" |
| A clock that is off | 61 seconds either way is refused with our time; 50 seconds passes | "a clock 61 seconds slow ..." |
| A key that was removed | Removing a device ends its logins at once; a revoked key's good proof refreshes nothing | "removing a device ends its logins", "a revoked key's good proof ..." |
| A downgrade to no proof | A bound login never refreshes without proof, through the API or the database's own function, and can't be unbound | "the database's own refresh refuses ...", "a session's device can't be cleared or changed" |

**Reuse detection.** Refresh tokens form a family. If a spent refresh
token is ever presented again, someone copied it: we end the whole family
and notify the account.

**The web.** A web session can optionally bind to a non-extractable
WebCrypto P-256 key in IndexedDB, signing the same header on sensitive
writes. This raises the cost of cookie theft by malware but doesn't stop
script on our own origin, so the strict CSP stays the main defence.

### Storage on each client

| Client | Where secrets live |
| --- | --- |
| Web | HttpOnly cookie only. No token in `localStorage`, IndexedDB or JavaScript memory. Keys from NIP-07 or NIP-46 stay in the extension or signer |
| iOS | Keychain, `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, not synced; device key in the Secure Enclave |
| Mac | Device key in the Secure Enclave, its wrapped blob in a `0600` file under `~/.openagents/`; the tokens in `~/.openagents/desktop/login.json` (`0600`); no Keychain, so a rebuilt app never asks for the login password. The login is the iPhone app's `AppAuth.swift` (#13229) |
| Android | Device key in Keystore; tokens, and the key "Log in with a key" makes on the phone, AES-GCM encrypted with a Keystore key in `0600` files in app-private storage (`apps/shared/host/android/app-login.c`) |
| CLI and dev tools | Files with mode `0600` under `~/.openagents/`. No Keychain by default, because a rebuilt unsigned binary triggers a Keychain password prompt on every build; `OPENAGENTS_KEYCHAIN=1` opts in |
| Agents on servers | Secret Manager or a `0600` file; never an environment variable that ends up in a process listing or crash log, never a command-line flag |

### Revocation and "log out everywhere"

- **Log out** deletes this session's row and clears the cookie or tokens.
- **Log out everywhere** (`/settings/security`, needs L2) deletes every
  session and refresh family of the account and bumps the account's
  `session_epoch`. A checkbox, off by default, also suspends every agent
  grant.
- **Devices list** ([/settings/devices](https://openagents.com/settings/devices),
  `GET /api/auth/devices`) shows each device key with its name, platform,
  where the key is kept, when it was last used and how many logins it
  holds. REMOVE (`POST /api/auth/devices/<id>/remove`) revokes the key and
  ends every login bound to it at once. `GET /api/auth/sessions` lists
  every login, browsers too, each with a remove. An app also gives its
  device a push target here (`POST /api/auth/devices/push`), so the
  account's security notices reach it
  ([Notify](identity-and-methods.md#linking-and-unlinking)).
- **Freeze** ("This wasn't me" from any notice, or FREEZE MY ACCOUNT on
  `/settings/data` from any login, even email alone) ends every session,
  suspends every agent grant and blocks spends, at once (#13214). A frozen
  account can still log in, but its sessions can only see the account,
  log out, unfreeze, step up and delete or cancel a deletion; anything else
  answers 423, and a NIP-98 signature carries no key. Unfreezing needs the
  strongest proof from the last 5 minutes: a key, passkey or recovery code,
  or, on an account with only email, a new email code. Agents stay paused
  until their owner resumes each one.

## Token formats

Every secret we issue has a fixed prefix (so it's recognizable in a leak
and scannable), 32 random bytes in base32, and a 6-character checksum
(CRC-32 in base32), the pattern GitHub uses for its tokens. We store
SHA-256 of the whole string and show the token once.

| Token | Shape | Where |
| --- | --- | --- |
| Web session | `oas_` + 52 + 6 | Cookie value |
| Access token | `oa_at_` + 52 + 6 | `Authorization: Bearer` |
| Refresh token | `oa_rt_` + 52 + 6 | Body of `POST /api/auth/refresh` |
| Agent API key | `oa_key_` + 8-char id + `_` + 52 + 6 | `Authorization: Bearer`; the id is shown in lists |
| Email link token | 43 base64url chars | URL fragment only |
| Email code | 6 digits | Typed |
| Recovery code | 24 BIP-39 words | Typed |
| NIP-98 event | kind 27235 JSON, base64 | `Authorization: Nostr` |
| Device proof | `v1 <ms> <signature hex>`: ES256 over method, URL, time, token hash | `OA-Device-Proof` header |

We register the `oa_key_` and `oa_rt_` patterns with GitHub's secret
scanning partner program so a pushed key is reported to us and revoked.

## Middleware

One function resolves every request before any route runs:

```text
auth_resolve(request) -> principal | refusal
  principal = { account, kind (person|agent|service|guest), session_id,
                grant, level, auth_at[L1], auth_at[L2], device }
```

1. Collect credentials: `Authorization: Nostr`, `Authorization: Bearer`,
   the session cookie, `X-Verse-Player`. More than one of the first three
   is `400 one credential`.
2. Verify: NIP-98 checks signature, `u`, `method`, `payload`, 60 seconds
   and the replay book; bearer and cookie look up the SHA-256; device
   proof checks the signature when the session is device-bound.
3. Load the account and grant; refuse a frozen account, an expired grant
   or an old `session_epoch`.
4. Each route declares what it needs: `public`, `guest`, `account`,
   `human`, `L2`, `L2-fresh`, `scope:<name>`, `admin`. The check is a
   table, not code in each route, so a test can list every route and its
   rule.
5. Writes from a cookie need our `Origin` (as today) and `SameSite=Lax`
   cookies; GETs never change state.
6. Log one line per refusal: route, rule, principal kind. Never the token.

**Admins** are accounts with the `admin` role, granted only by a root
admin. Root admins stay public keys in `admins.conf`, changed only by a
deploy, so no database write can create a root admin. Admin reads need
`L2`; an admin change (any `POST /api/admin/...`) needs `L2-fresh` and a
human principal. An `oa_admin` session from `admin.sh`'s signed link
counts as a signature made when it signed in.

### Step-up

A route that needs more than the request has answers `401` with a typed
refusal (`coax/web/api/auth-recover.c`, rules in `auth-policy.coax`):

```json
{"error": "Log in again with your key or passkey to do this.",
 "step_up": "L2-fresh", "methods": ["nostr", "recovery_code"],
 "limited": true, "level": "L1", "recovery": {"state": "none", ...}}
```

`step_up` is `L2` or `L2-fresh`, and `methods` lists what the account can
confirm with. A **limited** session is an email login (L1) on an account
with a strong way in (a key, passkey or unspent recovery
code). It reads and plays, but every write except recovery and logging
out is refused, and it carries no key, so it owns nothing a key owns.

To step up, a logged-in page asks `POST /api/auth/proof/challenge
{purpose}` (`stepup`, `link` or `finish`), has the key sign a NIP-98 event
over the returned URL (`POST`, no body), and posts `{"proof": base64}` to
that URL. Each challenge works once, for its purpose and account, within
5 minutes. A recovery code steps up too: `POST /api/auth/step-up {words}`
spends it and shows a new one.

The re-auth prompt is [/settings/security](https://openagents.com/settings/security)'s
CONFIRM IT'S YOU. Any page that gets a step-up refusal sends people to
`/settings/security?step=1&next=/its/path`, and they come back after they
sign (the admin pages do).

**Our own testing.** Team accounts carry a `team` flag (from `team.conf`
and admin status). Analytics read the flag, so every session of a team
account, and every guest player later attached to one, is excluded from
usage counts.

## Rate limits

Every limit below is shown to the person who hits it, with the time it
lifts ("Too many codes. Try again at 3:42 PM."), and listed on
`/docs/limits`.

| Action | Limit | Keyed by |
| --- | --- | --- |
| Email code sends | 5 an hour, 20 a day | Address hash; also 30 an hour per IP |
| Code tries | 5 per code, then the code dies | Login attempt |
| Passkey or Nostr login | 30 a minute | IP |
| Refresh | 60 an hour | Refresh family |
| Account creation | 10 an hour | IP, plus a proof of work above that |
| Recovery requests | 3 a week | Account |
| Agent requests | Set in the grant | Grant |

## Threat model

| Threat | What it looks like | Defence |
| --- | --- | --- |
| Credential stuffing | Leaked password lists tried against us | No passwords. Codes are per-login, 5 tries |
| Code bombing | Bots trigger code emails to addresses they don't own | Per-address and per-IP limits, proof of work on new accounts. No SMS, so no SMS pumping or SIM swap exposure (D3) |
| Inbox takeover | Attacker controls your email and logs in by code | Email is L1; accounts with stronger factors get a limited session and the 72-hour recovery path; notices on every channel |
| Phishing | Fake login page relays codes | Passkeys can't be relayed; email links complete only in the browser that asked; we nudge everyone to a passkey |
| Session theft | Malware or XSS copies a cookie or token | HttpOnly, `__Host-`, strict CSP with no third-party scripts, device-bound refresh, reuse detection, short access tokens, devices list |
| Agent key leak | An agent's key lands in a log, repo or prompt | Keys are scoped and capped; secret scanning; one-click revoke; agent keys can't reach human-only routes |
| Agent mistake | An agent with access deletes or rewrites data | Database roles that can't drop tables, locked backups in another project ([data](data-and-backups.md)) |
| Insider | A teammate or admin abuses access | No staff recovery override; every admin action in the audit log; root admins only by deploy; PII encrypted with keys the API holds, not humans |
| Account takeover by recovery | Attacker claims a lost account | 72-hour wait, cancellable from any channel |
| Our provider is breached | Email vendor leaks logs | They hold no user table; codes expire in 10 minutes; we can swap them ([build order](build-order.md#outside-services-we-cant-avoid)) |
| Server breach | Database dump stolen | Tokens and codes are hashes; emails are ciphertext; we hold no phone numbers; no passwords; no seeds we can open |
