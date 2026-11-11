# Identity and methods

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).

## The model

An **account** is a person, an agent or a service. Its id is opaque,
`acct_` plus 26 base32 characters (128 random bits). It never changes and
never encodes a key, an email or a provider, so adding or removing any
authenticator moves no data.

An **authenticator** is one way to prove you are an account. One account
holds many, in any combination. One authenticator belongs to exactly one
account (a unique index enforces it).

| Kind | Proves | Strength | Can create an account | Notes |
| --- | --- | --- | --- | --- |
| `email` | You read mail at an address | Possession of inbox (weak to medium) | Yes | 6-digit code and a link in one message |
| `phone` | You receive SMS at a number | Possession of SIM (weak: SIM swap) | Yes | US and Canada first |
| `passkey` | You hold a WebAuthn credential and unlocked it | Phishing-resistant (strong) | Yes | RP ID `openagents.com`, user verification required |
| `nostr` | You hold a secp256k1 key | Strong if the key is safe | Yes | NIP-98, NIP-07, NIP-46, phone QR |
| `device` | This install holds a non-exportable P-256 key | Strong, bound to hardware | No, only linked by a logged-in session | Secure Enclave, Android Keystore, WebCrypto |
| `agent_key` | An agent holds a Nostr key or `oa_key_` API key under a grant | Scoped | No, made by the owner | See [Agents](#agents) |
| `recovery_code` | You kept the code we showed once | Strong, single use | No | 24 BIP-39 words, scrypt-stretched |
| `oauth` (connection) | You control a GitHub or Google account | Not a login | No | Repository and file access only |

**Assurance levels.** Each authentication event records its level, and
routes ask for a level ([sessions](sessions-and-tokens.md#middleware)):

| Level | Reached by | Enough for |
| --- | --- | --- |
| `L1` | email code, SMS code | Reading and ordinary writes |
| `L2` | passkey, Nostr key, device key, or two different L1 factors | Spending above the free limit, adding an authenticator, creating agent grants |
| `L2-fresh` | L2 within the last 5 minutes | Seed export, removing an authenticator, raising a spend limit, deleting the account |

This follows NIST SP 800-63B rev. 4 in spirit: syncable passkeys count at
AAL2, and SMS is a "restricted" authenticator that needs a risk
assessment, user notice and a path to something stronger
([NIST SP 800-63B-4](https://csrc.nist.gov/pubs/sp/800/63/b/4/final)).
We don't claim formal AAL conformance.

**No passwords.** Passwords add a hash store, a breach-check service,
stuffing defences and reset flows, and every method above is stronger or
equal. If the owner ever wants them, Argon2id goes in first (see
[build order](build-order.md#crypto-we-must-write-in-coax)).

**Guests.** A visitor without an account plays the Verse and Everglade
with a random player key (`X-Verse-Player`, stored as SHA-256). Logging in
attaches the guest's player, characters and realms to the account once.
A guest key is never an authenticator.

## Methods

### Email

1. You type an address. We normalize it (trim, lowercase the domain, NFC)
   and look it up by its keyed hash.
2. We send one message with a 6-digit code and a link,
   `https://openagents.com/login/email#t=<token>`. The token stays in the
   fragment, so it never reaches a server log or a Referer.
3. The browser that asked holds a pending-login cookie
   (`__Host-oa_pending`, 10 minutes). The link and the code complete only
   in that browser. Opened on another device, the link says "Type this
   code in the window where you asked: 482 913". This stops a phisher who
   starts a login with your address from receiving your session when you
   click.
4. Typing the code (or opening the link) in the same browser logs you in
   at L1. A new address creates an account; a known one logs in.

Rules: a code lasts 10 minutes and allows 5 tries; asking again voids the
old code; we store only `SHA-256(code || login_id)`. The same flow
verifies an email you link while logged in.

### Phone (SMS)

The same flow by SMS: "OpenAgents code: 482913. Don't share it.
@openagents.com #482913". The last line is the WebOTP and iOS
domain-bound code format, so the browser and the app fill it only on our
origin.

- **Countries:** US and Canada at launch. Other countries open one at a
  time, because SMS pumping fraud targets open verification endpoints in
  high-cost destinations
  ([Stytch on toll fraud](https://stytch.com/docs/resources/policies/messaging/toll-fraud)).
- **SIM swap:** the FBI's IC3 recorded 1,611 SIM-swap complaints and over
  $68 million in losses in 2021 ([IC3 PSA](https://www.ic3.gov/PSA/2022/PSA220208)).
  So SMS alone is L1. An account whose only factor is a phone gets a
  "Add a passkey" nudge once it holds money, and SMS alone never reaches
  `L2-fresh`.
- **Cost:** we generate the codes; the provider only delivers. Raw US SMS
  costs about a cent a message; Twilio's Verify product adds $0.05 per
  verification on top
  ([Twilio Verify pricing](https://twilio.com/verify/pricing)), which we
  don't need. A monthly SMS budget with a hard stop is shown in admin.

### Passkeys (WebAuthn)

- **RP ID** `openagents.com`, fixed forever (changing it orphans every
  passkey). Staging uses `staging.openagents.com` and its own passkeys.
- **Create:** `residentKey: "required"`, `userVerification: "required"`,
  `attestation: "none"`, algorithms ES256 (-7), EdDSA (-8) and RS256
  (-257, for Windows Hello). The user handle is 32 random bytes per
  account, never the account id or an email.
- **Log in:** a discoverable-credential request with an empty
  `allowCredentials`, plus conditional UI (`autocomplete="webauthn"`) on
  the email field, so a browser with a passkey offers it before you type.
- **Stored:** credential id, COSE public key, sign count, the backup-
  eligible and backed-up flags, AAGUID, transports, a name ("iCloud
  Keychain", from the AAGUID), created and last-used times.
- **Native apps share the web's passkeys:** the iOS and Mac apps declare
  `webcredentials:openagents.com` in Associated Domains; the Android app is
  listed in `/.well-known/assetlinks.json` with
  `delegate_permission/common.get_login_creds`.
- **PRF:** we ask for the PRF extension on every passkey. Its output can
  unlock an encrypted wallet backup ([wallets](wallets-and-custody.md)),
  never the account itself. PRF support still differs by platform: Chrome
  and Google Password Manager have it; Safari and iOS reports conflict;
  Firefox is uneven ([Corbado](https://www.corbado.com/blog/passkeys-prf-webauthn),
  [caniuse](https://caniuse.com/mdn-api_credentialscontainer_get_publickey_option_extensions_prf)).
  So a PRF slot is never the only way to open anything.

### Nostr

Our library already has NIP-98, NIP-42, NIP-44, NIP-46, NIP-49, NIP-06 and
BIP-39/32 in Coax (`coax/lib/nostr/`, the
[Nostr book](../../nostr/book/README.md) chapters 38, 46 to 49).

| Path | Flow |
| --- | --- |
| NIP-98 (any client) | `POST /api/auth/nostr` with `Authorization: Nostr <kind 27235>` over a server challenge: `u`, `method`, `payload` tags, 60 seconds, each event id once. Answers a session. |
| NIP-07 (browser extension) | The page asks `window.nostr.signEvent` for that same event. |
| NIP-46 (remote signer) | The page shows a `nostrconnect://` QR, or takes a `bunker://` URL; the signer signs the login event over our relay. The page never holds the key. |
| Phone approves the web | The page shows a QR of a one-time challenge; the OpenAgents app signs it with its account's Nostr key; the page's poll gets the session. |
| Agents and the CLI | NIP-98 on every request, no session at all. |

A Nostr key can create an account with no email or phone. That account is
as recoverable as the key: if you lose the key and linked nothing else,
the account is gone, and the page says so when you create it.

We drop the browser "words account" (keys from entropy kept in browser
storage). Browser storage is not a safe home for a key, and passkeys
cover the "no app" case.

### OAuth: connections only

GitHub and Google attach to an account for what they do (repositories,
Drive, Gmail), with tokens encrypted at rest. They never log anyone in.

The consequence on iOS: Apple's guideline 4.8 says an app that uses "a
third-party or social login service ... to set up or authenticate the
user's primary account" must also offer an equivalent privacy-preserving
login, which in practice means Sign in with Apple. It exempts an app that
"exclusively uses your company's own account setup and sign-in systems"
([App Review Guidelines 4.8](https://developer.apple.com/app-store/review/guidelines/#login-services)).
Email codes, SMS codes, passkeys and Nostr keys verified by our server are
our own system. Linking GitHub for repository access doesn't set up or
authenticate the primary account. So no Sign in with Apple. If we ever add
"Log in with Google" to the iOS app, Sign in with Apple must ship in the
same release. Google Play has no equivalent rule.

### Agents

An agent is an account with `kind = agent` and an `owner` account. Its
owner makes it, sees it, and can stop it at any time.

- **Its authenticator** is its own Nostr key (preferred: it signs every
  request with NIP-98 and signs its public events) or an `oa_key_` API key
  for tools that can't sign. Never the owner's key, email or session.
- **Its grant** is a row the owner signs off at L2: scopes (for example
  `realms:write`, `inference:call`, `wallet:spend`), a spend cap per day
  and per payment in sats, an expiry (default 90 days, at most one year),
  and optionally the IP ranges or hosts it may call from. The page shows
  all of it.
- **Public attribution** uses NIP-OA: the owner key signs an `auth` tag
  binding the agent key and its conditions
  (`coax/lib/nostr/nipoa.coax`; [Nostr book chapter 87](../../nostr/book/ch087-owner-attestation-and-agent-authentication.md)).
- **What an agent can never do**, whatever its scopes: add or remove an
  authenticator, change recovery, make or widen a grant (its own or
  another's), raise a spend cap, export a seed, delete an account, or act
  at `L2-fresh`. Those routes require a human authenticator, which agent
  credentials can't be.
- **Leaks:** an owner revokes one agent, all agents, or rotates a key.
  `oa_key_` keys have a fixed prefix and checksum so GitHub secret scanning
  can find and report them ([sessions](sessions-and-tokens.md#token-formats)).

A service (the Pylon checker, the room servers, the publish job) is the
same shape with `kind = service` and owner `acct_openagents`.

## Linking and unlinking

- **Link:** while logged in, prove the new authenticator (code, passkey
  ceremony, NIP-98 signature). Needs L2, or L1 plus the new factor itself
  if the account has only L1 factors. An authenticator that belongs to
  another account is refused with "This email already has an account. Log
  in to it to merge." Nothing moves.
- **Unlink:** needs `L2-fresh`. Refused if it would leave no way to log in
  ("Add another way to log in first"). Removing an authenticator ends
  every session it created.
- **Notify:** every link or unlink sends a notice to every email and
  phone on the account, with a "This wasn't me" link that freezes the
  account and ends all sessions.

## Recovery

| You still have | You get back in by |
| --- | --- |
| Any passkey, Nostr key or device | Logging in with it |
| Your email or phone | Logging in with it (L1), then adding a passkey |
| Only your recovery code | Typing it: one-time L2, then the code is spent and a new one is shown |
| Nothing above, but your email *and* phone | The delayed path below |
| Nothing | The account is gone. We can't and won't recover it |

**The delayed path.** When the account has passkeys or keys you've lost,
email or phone alone could be a SIM swap or a hacked inbox. So an L1
login on an account with stronger factors gets a limited session: it can
read and play, but not spend, export or change authenticators. To lift
that, you ask for recovery; we wait 72 hours and notify every channel,
and any channel can cancel. After 72 hours you may replace the lost
factors. No staff member can skip the wait (decision D10).

**Recovery codes** are 24 BIP-39 words, shown once when you add your
first strong factor and again on request at `L2-fresh`. We store
`scrypt(words, salt, N=2^17, r=8, p=1)`, the same parameters as NIP-VAULT's
recovery slot, so one code can also open a wallet backup
([wallets](wallets-and-custody.md#encrypted-backups-we-cant-open)).

## Merging accounts

Two accounts merge only when you prove both in one flow: logged in to the
survivor, you log in to the other. Authenticators, realms, characters,
agents and grants move; the other account becomes a tombstone pointing at
the survivor. Wallet balances and credit never merge automatically: each
needs its own confirmation, because merging money between two people's
accounts by mistake can't be undone.

## Per-platform flows

| Platform | First screen | Then | Session |
| --- | --- | --- | --- |
| Web | Email field with passkey autofill; "Log in with a key" (NIP-07, NIP-46, phone QR) | Offer a passkey after the first email login | `__Host-oa_session` cookie |
| iOS and Mac apps | **Email**, with "Use phone number instead" | Make a Secure Enclave device key silently; offer a passkey | Device-bound refresh token |
| Android | Same as iOS | Android Keystore device key (StrongBox when present); Credential Manager passkey | Device-bound refresh token |
| CLI (`openagents login`) | Prints a link and QR to `/login/device#code=...`; you approve in a logged-in browser | Its own Nostr key, linked as `cli:<machine>` | None: NIP-98 per request |
| Agents | Made by the owner on `/settings/agents` or by `openagents agent create` | Grant shown and signed at L2 | NIP-98 per request, or `oa_key_` |
| Everglade, Verse | Playable as a guest | Log in through any of the above | The page's session; rooms take signed seat tickets as today |

**The mobile default is email.** The owner asked for email and SMS
centred mobile login. Email is free, works worldwide and is easy to
change; SMS costs money, is restricted to two countries at first and is
weak against SIM swap. So the app opens on email, offers phone one tap
away, and right after the first login asks once: "Save a passkey so you
can log in with Face ID next time." That keeps the app inside Apple's
own-system exemption and gets most people to a phishing-resistant factor.
