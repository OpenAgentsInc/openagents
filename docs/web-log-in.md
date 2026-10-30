# Logging in on the web with a passkey

Spec, not built yet (2026-10-30). Today `/login` only works with the
OpenAgents iOS app ([phone-sign-in.md](phone-sign-in.md)): the page shows a
QR code and the phone approves it. Someone without the app can't log in.
This adds a way to log in from the browser alone. The Nostr key comes from
entropy that a passkey produces, the way Breez's Glow does it. Everything we
write is Coax; the only JavaScript is the WebAuthn call itself.

## What Glow does

Glow (`breez/glow-web`) is a Spark wallet that runs entirely in the browser.
It stores no seed: the passkey *is* the backup. It follows Breez's
[seedless-restore spec](https://github.com/breez/seedless-restore),
implemented in `spark-sdk` `crates/breez-sdk/core/src/passkey/`:

- **The seed:** the WebAuthn PRF extension evaluates the passkey on a salt
  (a label such as "personal"). The first 16 of the 32 output bytes are
  BIP-39 entropy, giving 12 words. The same passkey on any device it syncs
  to (iCloud Keychain, Google Password Manager, a security key) gives the
  same words.
- **Labels:** `PRF(passkey, magic salt)` gives an "account master". From it
  a Nostr key is derived at `m/44'/1237'/55'/0/0`, and that key publishes the
  wallet's labels as kind-1 notes, so a new device can find them. One passkey
  can hold several wallets.
- **One ceremony:** `prf.eval.first` and `prf.eval.second` derive two salts
  with one prompt, where the authenticator allows it.
- **Lessons from their code:**
  - Firefox's PRF support is unreliable, so they turn it off there.
  - A browser can't tell "no passkey" apart from "the user cancelled". So
    there are two buttons, "Create" and "Log in", never a single button that
    guesses.
  - A passkey is bound to the hostname (the RP ID) it was made on.
    Changing it later orphans everyone's account, so pick it once.
  - If the passkey is created but the derivation then fails, the user should
    log in with the passkey that now exists, never register a second one.

## What we take, and what we change

We need one key per person, not many wallets, so we skip the label store.

- **RP ID:** `openagents.com`, fixed. Staging gets its own RP ID,
  `staging.openagents.com`, and so its own keys. A staging account is a test
  account.
- **Entropy:** `PRF(passkey, "openagents.com/log-in/v1")`, first 16 bytes,
  as in Breez's spec. These 16 bytes are the root of everything below.
- **Words:** BIP-39 English, 12 words from that entropy. They're shown once,
  after creating the account, behind a warning, and again only after a fresh
  passkey prompt ("Show my words").
- **Nostr key:** NIP-06, `m/44'/1237'/0'/0/0` from the words with no
  passphrase. The words then work in any Nostr app and in our own apps.
- **Spark wallet, later:** the same 12 words, as the iOS app already does.
  Logging in and having a wallet stay one backup.

The server never sees the entropy, the words or the private key. It sees
only the public key and the signatures.

## The flow

### Create an account (no passkey yet)

1. `/login` shows **CREATE ACCOUNT**, **LOG IN WITH PASSKEY** and, below
   them, **USE THE PHONE APP** (today's QR flow, unchanged).
2. CREATE ACCOUNT calls `navigator.credentials.create`:
   - RP `openagents.com`, user name "OpenAgents" plus a date (Apple
     deduplicates passkeys by RP ID and user name);
   - `residentKey: "required"`, `userVerification: "required"`;
   - `extensions: {prf: {eval: {first: salt}}}`.

   A browser that returns PRF output at creation saves a second prompt.
   Otherwise the page immediately calls `credentials.get` with the same salt.
3. Coax derives the entropy, the words and the key, then logs in (below).
4. The page shows the words once: "These 12 words are your account. Your
   passkey brings them back on any device it syncs to; the words work even
   without it." The user checks a box to continue. No quiz.

### Log in (passkey exists)

1. LOG IN WITH PASSKEY calls `navigator.credentials.get` with
   `allowCredentials: []` (any passkey for this site), `userVerification:
   "required"` and the PRF salt.
2. Coax derives the key as above.
3. The page signs its own login. A new route, `POST /api/auth/key`, takes a
   NIP-98 `Authorization` header: kind 27235, tags `u` (the route's URL),
   `method` POST and `payload` (SHA-256 of the body). The body is
   `{"kind":"login"}`. It answers with the same `oa_session` cookie the poll
   gives today: HttpOnly, Secure, SameSite=Lax, 30 days, bound to that
   pubkey.
   - The NIP-98 event's freshness window and replay rules are the existing
     ones in `private.c`. An event id is accepted once.
   - The Verse player key is moved to the new account by the rule `poll`
     already uses (`0022_signin_verse_player.sql`).
4. After the cookie is set, the key stays only in the tab's Coax memory, for
   actions that need a signature. It's cleared on logout or when the tab
   closes. It's never written to localStorage, IndexedDB or a cookie. A new
   tab uses the cookie; it prompts for the passkey only when it needs to
   sign something.

### Without PRF

When the browser has no PRF (Firefox, older Safari, some Windows Hello
setups), or the user prefers it:

- **TYPE MY WORDS:** 12 words go into a field. Coax checks the BIP-39
  checksum, derives the same key and logs in. The words are cleared from
  memory after the key is derived.
- **USE THE PHONE APP:** today's QR flow.

The page detects PRF with `PublicKeyCredential.getClientCapabilities()`
where the browser has it, and otherwise from whether the ceremony returned
PRF output. Firefox is treated as having no PRF, as Glow does.

### People who already log in with the phone

Today the phone's Nostr key is 32 random bytes in its Keychain
(`NostrKey.swift`), unrelated to its Spark words. Their account is that
pubkey. We don't move secrets between devices. We link keys instead, as
`projects.member_keys` already does for a member's phone:

1. A phone user who wants web login logs in once with USE THE PHONE APP.
2. The page offers **ADD A PASSKEY**, which creates the passkey and derives
   its key.
3. The new key is linked to the account with the same approval the phone
   gives today: a QR of kind `link-key`, which the phone approves. From then
   on, logging in with either key opens the same account.

A later iOS release can derive the phone's Nostr key from its Spark words by
NIP-06. New phone users and web users then share one key from the same
words. Existing phone keys stay linked.

## Where each piece lives

| Piece | Where | Language |
| --- | --- | --- |
| WebAuthn create and get with PRF, returning 32 bytes and a credential id | `coax/web/host.js`: a narrow `passkey_*` bridge, no other logic | JavaScript (browser API only) |
| SHA-512 and HMAC-SHA512 | `coax/lib/crypto/sha512.coax`, `hmac` extended | Coax |
| BIP-39: entropy to words, words to entropy with checksum, seed (PBKDF2-HMAC-SHA512, 2048 rounds) | `coax/lib/crypto/bip39.coax` with the English list (SHA-256 `2f5eed53…dbda`, the same check as `BIP39.swift`) | Coax |
| BIP-32 private derivation and NIP-06 | `coax/lib/crypto/bip32.coax`, `coax/lib/nostr/nip06.coax` using the existing secp256k1 code | Coax |
| NIP-98 event for `/api/auth/key` | `coax/lib/nostr/event_sign.coax` (exists) | Coax |
| The `/login` page: buttons, words screen, typed words, errors | `apps/openagents-web/private.coax` page 2 | Coax |
| `POST /api/auth/key`, link kind `link-key`, rate limits | `coax/web/api/private.c`, a migration for linked account keys | C (the API's existing language) |
| Wiping secrets | entropy, words, seed and key zeroed after use (`coax/lib/crypto/platform/zeroize`) | Coax and C |

The PRF output crosses from JavaScript to Coax once, as bytes. JavaScript
doesn't keep it or log it.

## Tests that must pass

- **Vectors:**
  - SHA-512: the NIST short and long message vectors.
  - BIP-39: the official vectors without passphrase, from entropy to words
    to seed.
  - BIP-32: test vectors 1 and 2.
  - NIP-06: the two vectors in the NIP.
- **The whole chain:** a fixed 32-byte "PRF output" gives fixed words, a
  fixed npub and a valid NIP-98 event.
- **The API (`test.sh`):**
  - a good signed login gets a cookie;
  - a replayed event, a wrong `u`, a wrong payload hash or a stale timestamp
    gets 401;
  - the rate limit gives 429;
  - the Verse player moves to the new account;
  - a linked key opens the same account.
- **The page, in headless Chrome:** a CDP virtual authenticator with PRF
  support (`WebAuthn.addVirtualAuthenticator`, `hasPrf: true`) creates an
  account, logs out and logs back in to the same npub. TYPE MY WORDS reaches
  the same npub. A browser without PRF shows only the fallbacks.
- **No secrets leak:** no entropy, words or key appear in the API logs, the
  analytics events, localStorage or the page's DOM text, except the words
  screen while it is shown.

## Risks and decisions

- **Losing the passkey loses the account unless the words were kept.** This
  is why the words are shown at creation. Synced passkeys (iCloud, Google)
  make this rare; a passkey that only lives on one device makes it likely.
- **Browser coverage:**
  - PRF works in Chrome, Edge and Safari 18+ with iCloud Keychain, and on
    Android with Google Password Manager.
  - Cross-device login (scanning the browser's QR with a phone) supports PRF
    in Chrome.
  - Windows Hello and Firefox are spotty, so they get TYPE MY WORDS and the
    phone app.
- **A malicious page script could read the PRF output.** Our pages already
  load only our own scripts, so the passkey adds no new risk there. The
  login page must keep a strict CSP with no third-party scripts.
- **Not built here:**
  - a Spark wallet in the browser (Breez's WASM SDK could open the same
    words later);
  - NIP-46 remote signing;
  - choosing among several accounts per passkey (Breez's labels).

## Coax project tasks, in order

1. `sha512`: SHA-512 and HMAC-SHA512 in Coax, with the NIST vectors.
2. `bip39`: words and seed, with the official vectors.
3. `bip32-nip06`: private derivation and the NIP-06 key, with the BIP-32 and
   NIP-06 vectors.
4. `passkey-bridge`: the `host.js` WebAuthn bridge with PRF, and a
   virtual-authenticator test.
5. `auth-key-route`: `POST /api/auth/key` and the `link-key` approval, with
   API checks.
6. `login-page`: the new `/login` with the words screen and the fallbacks,
   plus the headless end-to-end test.
7. `login-copy`: update `apps/openagents-web/login/index.md` and the page's
   `<h1>` so they say you can log in with a passkey, your words or the phone
   app.
