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

## The words account in use (2026-10-30, owner changes)

Since this spec was written, the owner changed what gets built first:

- **No passkey for now.** On a first visit, the page makes 16 bytes of
  entropy in the browser, keeps them in browser storage (the host's existing
  keep action), and logs in with the words' NIP-06 key. That key is the
  visitor's account.
- **The `/login` page:**
  - SHOW WORDS reveals the 12 words, and COPY copies them.
  - SWAP ACCOUNT replaces the account with 12 pasted words, after a warning.
  - The QR code for the phone app stays exactly as it works today.
- **Until the browser side is audited, keys are demo-grade**, the same
  posture as the Oct 30 Pylon keys ([Oct30MVP.md](Oct30MVP.md) §3). The swap
  box says "Don't paste words for a key you use elsewhere; this device keeps
  them."

## Pylons earn XP, and the site shows it

[NIP-XP](../nips/openagents/NIP-XP.md) already describes everything needed:
- the `pylon-check` rule, which gives XP for compute work a trusted checker
  verified;
- key links, so a person's other keys add up to one level;
- trainer profiles, the key's opt-in to having its level shown.

The pieces it builds on, the Pylon's receipts and check verdicts, are in
[NIP-PYLON](../nips/openagents/NIP-PYLON.md). This section connects them to
the Oct 30 Pylons and the words account. Spec only: nothing here is built.
Its conformance implementation was the Rust `crates/nostr` `xp` module,
deleted in `0b401561a0`, so every piece below is new Coax.

XP stays what NIP-XP says it is: evidence of verified accepted work. It is
never a balance. It never turns into sats, credit or permissions, and paying
a Pylon never creates XP.

### How one decision job becomes XP

1. **The job.** A buyer sends a decision job (25910) to a Pylon, and the
   Pylon answers (26910).
   - The buyer can be the stage buyer (`demo-buyer.ts`), the Chapter 25
     adapter, or a visitor's browser. For a visitor, `/pylon` gets **SEND A
     JOB**, signed with the words account's key in Coax.
   - These events are ephemeral, so relays don't store them.
2. **The receipt.** The buyer publishes a `3201` receipt for that job. It is
   a regular kind, so relays store it. The body is
   `openagents.pylon-receipt.v2`, with:
   - `lane` `decision`, `units` `{jobs, 1}`, `payment` null;
   - `outcome` `accepted` once the buyer has checked the answer.

   Today neither the stage buyer nor the adapter publishes receipts; both
   need to, and the browser job does it from the start.
3. **The check.** An OpenAgents **checker** key reruns the same request. It
   uses the reference toy engine, which serves the same exact identity
   `psionic-toy@sha256:b668…3573`. If the answer matches exactly, it
   publishes a NIP-32 label (`1985`):
   - `L` `openagents.pylon`, `l` `check-pass`;
   - `e` the receipt, `p` the Pylon, `x` the checker's own result digest;
   - content `suite:<digest>`, where the digest is the SHA-256 of the
     shipped self-check suite (`psionic/fixtures/pylon/psionic-toy.selfcheck`).

   A mismatch gets `check-fail`. The checker is never the receipt's buyer or
   its Pylon. It runs as a small Coax service next to the relay, subscribed
   to `3201`. Its limits (checks per minute, per Pylon) are configurable.
4. **The award.** An OpenAgents **referee** key publishes one `pylon-check`
   quest (`30193`) per season. The acceptance is
   `{"rule":"pylon-check","suite":"<digest>","checker":"<checker key>","max_awards":10000}`,
   with `award` `{"provider": 100}`.
   - For each receipt carrying a check-pass, the referee signs an award
     (`3193`) to the Pylon's key.
   - Its uniqueness key is
     `pylon-check:<season>:<suite>:provider:<pylon pubkey>`, so a Pylon
     earns once per suite per season.
   - **Seasons are weekly** (`2026-w44`, …), so a Pylon that keeps serving
     earns again each week. A second suite, Elephant on a GPU, would be a
     second quest that earns separately.

### Linking a Pylon to its person

A Pylon signs with a throwaway key. Real key files stay refused until the
owner signs off (Oct30MVP §3), so a person's XP comes from **key links**,
not from sharing a key:

- **The join script** takes `--trainer <npub>`: the npub that `/login` shows
  next to COPY. Once it's online, the Pylon publishes a key link (`13195`):
  `{"v":1,"requires":[],"type":"link","trainer":"<account hex>"}`.
- **The words account** publishes a trainer profile (`13193`):
  `{"v":1,"requires":[],"type":"profile","shown":true,"keys":["<pylon hex>",…]}`.
  - On `/login`, YOUR PYLONS lists the Pylons whose links name you but that
    your profile doesn't list yet, each with **CLAIM**.
  - CLAIM signs a new profile with that key added.
  - REMOVE drops it.
  - SHOW MY LEVEL toggles `shown`.
- **The phone key** links the same way. The phone app signs a `13195` naming
  the words account; the user starts it by scanning the npub's QR on
  `/login`, and the profile lists the phone key. This public link is the
  same pairing the `link-key` approval above records for sessions. The API
  can accept the two signed events in place of a separate approval.
- **Limits from NIP-XP:**
  - a profile lists at most 16 keys, so a person's level counts at most 16
    Pylons and phones;
  - links are one level deep;
  - a one-sided link counts nothing;
  - either side can end a link.

### What the site shows

The page trusts one referee, OpenAgents' own key. NIP-XP says there's no
global ledger, so the page labels it "XP from OpenAgents checks".

- **`/pylon` board:**
  - Each Pylon row gets an **XP** column and a small level badge, from the
    awards to its key. Awards are stored on the relay, so they are counted
    from history, unlike the ephemeral job counters.
  - When a new award arrives during the session, the row flashes **+100 XP**,
    alongside the existing live answer count.
  - A Pylon linked both ways to a trainer whose profile says `shown: true`
    shows that trainer's short npub and level ("run by npub1ab…, level 3").
    Otherwise it shows no owner.
  - Rows can be sorted by XP. The board is a reader's view, with no global
    rank claimed.
- **`/login` account panel:**
  - **LEVEL N** and an XP bar to the next level, summed over the account
    key and every key linked both ways;
  - YOUR PYLONS, with each one's XP this season, online or offline from its
    beacon, CLAIM and REMOVE;
  - the season name and when it ends;
  - SHOW MY LEVEL.
- **Levels:** levels are the client's reading of the ledger, not part of the
  protocol. We use the curve NIP-XP's trainer cards name,
  `trainer-curve-v1`, and set its thresholds in one Coax table that both
  pages share.
- **Later:** export a trainer card (`30194`) as an `naddr` link people can
  share.

### Abuse, in this setting

- **Self-buying:** a Pylon's own key publishing a receipt for itself never
  counts (NIP-PYLON). A second key is cheap, but the award still needs the
  checker to rerun the job and get the same answer, so the XP stands for
  work the Pylon actually did.
- **Farming with many Pylons:**
  - Each Pylon key earns once per suite per week.
  - A person's level counts at most 16 linked keys.
  - The checker's rate limits bound what OpenAgents spends on reruns.
  - Throwaway keys are free, so a person *can* run many Pylons, but beyond
    16 they don't add to one level.
- **A stolen referee or checker key:** revoke with `3194` and rotate the key
  the page trusts, as NIP-XP's abuse table says.
- **Relay:** check that `relay.openagents.com` accepts kinds `3201`,
  `1985`, `30193`, `3193`, `13193` and `13195` from arbitrary keys, the same
  check Oct30MVP test A ran for the Pylon kinds.

### Coax pieces

| Piece | Where |
| --- | --- |
| XP codec and validator: quest, award, revocation, profile, link, with closed bodies, markers, one `t` per kind | `coax/lib/nostr/xp.coax` |
| `pylon-check` rule and ledger derivation, with trust list, uniqueness, links | `coax/lib/nostr/xp_ledger.coax` |
| Receipt (`3201`) and check label (`1985`) codecs | `coax/lib/nostr/pylon_receipt.coax` |
| Buyers publish receipts | `demo-buyer.ts` and the Chapter 25 adapter (`psionic-fleet`); the browser job in Coax |
| Checker service: subscribe, rerun on the reference engine, label | `psionic/lib/pylon/checker.coax` plus a host |
| Referee service: weekly quest, award on check-pass | `psionic/lib/pylon/referee.coax` |
| `--trainer` and the `13195` link | `psionic/lib/pylon/run.coax`, `scripts/join-pylon.sh` |
| XP column, `+XP`, owner | `/pylon` page (`apps/openagents-web/pylon/`) |
| Level, YOUR PYLONS, CLAIM, SHOW MY LEVEL | `/login` (`private.coax` page 2) |

### XP tasks, in order (after the login tasks)

8. `xp-codec`: NIP-XP events in Coax, with the NIP's examples as vectors.
9. `pylon-receipts`: receipts and check labels in Coax, and published by the
   stage buyer and the adapter.
10. `pylon-checker`: the checker service and `check-pass`, tested against a
    local relay with a matching and a lying Pylon.
11. `xp-referee`: the weekly `pylon-check` quest and its awards, with the
    uniqueness and season checks.
12. `xp-links`: the join script's `--trainer`, the Pylon's `13195`, and the
    profile, CLAIM and REMOVE on `/login`.
13. `xp-board`: the XP column, level badge, `+XP` flash and owner on
    `/pylon`, plus the level and XP bar on `/login`, with the shared curve
    table.
14. `xp-live`: on the public relay, a script-joined Pylon linked to a words
    account answers a browser job. It gets a receipt, a check-pass and an
    award, `/pylon` shows **+100 XP** on its row, and `/login` shows the
    level.
