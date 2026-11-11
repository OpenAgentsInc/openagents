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

On first login, the app makes a **device key**: P-256, non-exportable, in
the Secure Enclave on iPhone and Mac, in Android Keystore (StrongBox when
present). The Secure Enclave and Keystore support P-256 but not
secp256k1, which is why the device key is a separate key from any Nostr
key. The app registers the public key with the session.

Every refresh request carries a signature by the device key over the
method, URL, a timestamp and the refresh token's hash (the shape of
OAuth DPoP, RFC 9449, with our own header). A refresh token copied off the
device, from a backup or a log, is useless without the key that never
left the hardware.

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
| iOS, Mac | Keychain, `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, not synced; device key in the Secure Enclave |
| Android | Device key in Keystore; tokens encrypted with a Keystore AES key in app-private storage |
| CLI and dev tools | Files with mode `0600` under `~/.openagents/`. No Keychain by default, because a rebuilt unsigned binary triggers a Keychain password prompt on every build; `OPENAGENTS_KEYCHAIN=1` opts in |
| Agents on servers | Secret Manager or a `0600` file; never an environment variable that ends up in a process listing or crash log, never a command-line flag |

### Revocation and "log out everywhere"

- **Log out** deletes this session's row and clears the cookie or tokens.
- **Log out everywhere** (`/settings/security`, needs L2) deletes every
  session and refresh family of the account and bumps the account's
  `session_epoch`. A checkbox, off by default, also suspends every agent
  grant.
- **Devices list** shows each session and device with its name, platform,
  last seen and coarse location; each has a remove button.
- **Freeze** ("This wasn't me" from any notice) does the same as log out
  everywhere, suspends agent grants and blocks spends until you log in at
  L2.

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
| Device proof | ES256 signature over method, URL, time, token hash | `OA-Device-Proof` header |

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
deploy, so no database write can create a root admin. Admin routes need
`L2-fresh` and a human principal.

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
