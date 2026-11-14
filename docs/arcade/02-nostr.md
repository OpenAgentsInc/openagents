# Arcade audit 02: Nostr

Part of the Arcade audit (issue #13607). This doc lists every use of Nostr in
Arcade, exactly, then maps each one to the current NIP and to the module in
`coax/lib/nostr/` that covers it today. It is analysis only. It changes no
code. Another agent owns all Nostr implementation in this repository.

Sources, pinned:

| Source | Revision |
| --- | --- |
| Arcade app, read-only clone `/Users/christopherdavid/work/arcade` | `3f10e1e` (2023-08-05) |
| arclib submodule `app/arclib` | `542ecec` (2023-07-28), author Erik Aronesty |
| Current NIP texts: `/Users/christopherdavid/work/projects/repos/nips` | `1fb3a95` (2026-11-12) |
| The repo's pinned mirror `nostr/nips/official/` (what our library targets) | upstream `400d157c`, synced 2026-10-28 |
| The "NIP-112" proposal | nostr-protocol/nips PR #580, read with `gh` |
| Our library | `coax/lib/nostr/`, `nostr/relay/`, `apps/openagents-web/nostr-*` at `origin/main` `450ca4738f` |

Every NIP status, deprecation note and kind in Part B was checked against the
`projects/repos/nips` clone at `1fb3a95`. The status lines of 04, 06, 08, 15,
17, 26, 28, 29, 49, 51, 57, 65, 72, 99, C7 and EE match the pinned mirror
word for word.

"Verified" means read in the code or the NIP text. "Inferred" is marked.

---

## Part A: Arcade as it is

### A0. Stack

| Piece | Where | Notes |
| --- | --- | --- |
| Nostr library | `app/arclib/src/*.ts` (2,707 lines) | Exports in `index.ts`: listing, comment_to_trade, ident, util, pool, private, nip28channel, encchannel, db, channel, social. `contacts.ts` and `profile.ts` are imported by path. |
| nostr-tools | app `^1.11.1`, arclib `^1.13.0` (`package.json`) | `SimplePool`, `nip04`, `nip05`, `nip19`, `parseReferences`, `matchFilter`, `getEventHash`, `getSignature`, `verifySignature`. |
| Curves and hashes | `@noble/curves` secp256k1, `@noble/hashes` (hkdf, sha256), `@scure/base` base64 | Used for Arcade's own encryption. |
| WebCrypto | `isomorphic-webcrypto` (a fork at `github.com/earonesty/isomorphic-webcrypto`), `expo-crypto` | AES-CBC through `crypto.subtle`. |
| Event cache | `any-sqlite` with the `react-native-quick-sqlite` driver | `arclib/src/db/base.ts`. |
| Secrets | `expo-secure-store` | iOS Keychain / Android Keystore. |
| State | MobX-State-Tree, persisted to AsyncStorage | `models/helpers/setupRootStore.ts`. |

Wiring: `components/RelayProvider.tsx` builds one `ArcadeIdentity` from the
stored private key, one `NostrPool` (with the SQLite cache and
`{ skipVerification: true }`, line 40), and one each of `ChannelManager`,
`ContactManager`, `ProfileManager`, `PrivateMessageManager` and `ArcadeSocial`.
It exposes them through `RelayContext`. On every change of key or relay list it
calls `pool.setRelays(getRelays)` and closes the pool on unmount.

### A1. Identity and keys

**Generation.** `UserStore.signup` (`models/UserStore.ts:216-256`) calls
nostr-tools `generatePrivateKey()` (32 random bytes, hex). There is no seed
phrase. `ArcadeIdentity.generate()` (`ident.ts:71-75`) does the same and is
used only by tests.

**Import.** `LoginScreen.tsx:32-78` accepts one text field, placeholder
"nsec or hex private key". It refuses input shorter than 60 characters. An
`nsec1` string is decoded with `nip19.decode`; anything else is used as hex.
There is no check that hex is 64 characters before `getPublicKey`. There is
no mnemonic (NIP-06), no `ncryptsec` (NIP-49), no remote signer (NIP-46) and
no browser signer.

History (verified in `git log`): NIP-06 key generation existed in the
pre-Ignite app (`39dc9e7`, `1ac55d1`, Dec 2022 to Jan 2023; `ce460a8`,
Apr 2023), and NIP-26 delegation "via Nostr Connect" existed in `6daef1a`
(Feb 2023). Both lived in the old `src/` tree and went away with the Ignite
conversion (`ac83f18`, `296e658`, May 2023). The changelog screen still
claims "Nostr-based user authentication (NIP-06)"
(`screens/ChangelogScreen.tsx`). That claim is stale.

**`ident.ts` in full.**

| Member | Lines | What it does |
| --- | --- | --- |
| `CURRENT_ENCRYPTION_VERSION = 1` | 10 | Version suffix of Arcade's own ciphertext format. |
| `ssCache` | 31 | LRU of 500 ECDH shared secrets keyed by `[privkey, pubkey]` arrays. Array keys compare by identity, so the cache never hits for new arrays (inferred from LRU semantics). |
| `NostrEvent`, `UnsignedEvent` | 33-46 | NIP-01 shape. |
| `constructor(nsec_or_priv, bitcoinAddress='', lnUrl='')` | 52-69 | Accepts nsec or hex, keeps both forms, derives `pubKey`. `bitcoinAddress` and `lnUrl` are stored and never used. |
| `nip04Encrypt` / `nip04Decrypt` | 77-83 | nostr-tools NIP-04. |
| `selfEncrypt` / `selfDecrypt` | 85-105 | Encrypt to your own key with a one-time key; content is `JSON.stringify([epub, ciphertext])`. |
| `nip04XEncryptSS` / `nip04XEncrypt` / `nip04XDecrypt` | 107-181 | Arcade's own cipher, below. |
| `fastSS(pubkey, privkey)` | 183-194 | `secp256k1.getSharedSecret(privkey, '02' + pubkey).slice(1, 33)`: the raw x coordinate, unhashed. |
| `nip44XIdent(pubkey)` | 196-201 | A new identity whose private key is the shared x with `pubkey`. |
| `nip44XEncrypt` / `nip44XDecrypt` / `nip44XDecryptAny` | 203-253 | "Blinded" DMs, A6. Not NIP-44. |
| `nipXXEncrypt` / `nipXXDecrypt` | 256-327 | Kind 99 wrap, A5. Not NIP-59. |
| `signEvent` / `signEventWith` | 329-353 | NIP-01 id and BIP-340 signature through nostr-tools. |
| `isMine` | 355-358 | `event.pubkey == this.pubKey`. |

Arcade's own cipher (the "??" format), verified at `ident.ts:107-181`:

```
x       = ECDH x-coordinate(sender_priv, 02 || recipient_pub)       (32 bytes)
iv      = 16 random bytes
key     = HKDF-SHA256(ikm = x, salt = iv, info = empty, length = 32)
content = base64(AES-256-CBC(key, iv, PKCS#7(utf8(plaintext))))
          + "??" + base64(iv) + "??" + "1"
```

There is no MAC. Decryption refuses any version other than `"1"`. The name
"nip04X" says it is a NIP-04 variant; it is not NIP-04 (no `?iv=`, HKDF step)
and not NIP-44 (no ChaCha20, no HMAC, no padding scheme).

**Storage.** Verified:

| What | Where | Protection |
| --- | --- | --- |
| Private key (hex) | SecureStore key `privkey` (`UserStore.ts:41-49, 204, 254, 265`) | Keychain / Keystore. |
| Private key again, plus every private channel key | MST snapshot under AsyncStorage key `root-v1` (`setupRootStore.ts:19, 43`); `UserStore.privkey` and `ChannelModel.privkey` are snapshot props | AES-256-CBC under a 32-byte app key in SecureStore key `appk`; IV = first 16 bytes of SHA-256 of the storage key name (`utils/storage/storage.ts:9-53`). The IV is fixed per key name, so every save of `root-v1` reuses one IV. |
| Profile cache | AsyncStorage `meta`, same wrapping | |
| Anonymous id | SecureStore `ARCADE_ANON_USER_ID`, a UUID v4 (`hooks/useAnonUser.ts`) | Dead code: `useAnonUser`/`UserProvider` are defined and never mounted. It was for RevenueCat (commented out). It is not a Nostr identity. |

**Export.** `BackupScreen.tsx` shows the hex key and the nsec in masked fields
with Copy buttons that write them to the system clipboard.

**Logout.** `UserStore.logout` (`277-296`) clears the joined and contact
sets, deletes SecureStore `privkey` and `applySnapshot`s `userStore` with
`privkey: ""` and `channels: []`. It does not touch `rootStore.channelStore`
(a separate child of the root, `RootStore.ts:9`), whose `ChannelModel` entries
keep each private channel's `privkey`, so `root-v1` still holds the channel
keys after logout (agrees with `docs/arcade/01-codebase.md` §5.14). It also does
not clear the SQLite cache `arcade.1`, the AsyncStorage `meta` key or
SecureStore `appk`. The next account on the device sees the previous account's
cached events and private channel keys (verified: no call to `db.reset()` or
any channel-store reset anywhere in `app/`).

**NIP-19.** `npub`/`nsec` encode and decode (`hooks/useNpub.ts`,
`utils/shortenKey.tsx` shows the first 12 characters of the npub plus "...",
`BackupScreen.tsx`, `contacts.ts:resolvePubkey`). No `nprofile`, `nevent`,
`naddr` or `note` handling beyond what `parseReferences` does in A9.

**NIP-05.** Two uses:

1. Registration of `<username>@arcade.chat` at sign-up
   (`UserStore.ts:51-67`). The app sends a GET to an AWS Lambda function URL
   in us-east-2 with `name`, `pubkey` and `sig`, where `sig` is a BIP-340
   signature over `sha256(JSON.stringify([0, pubkey, name]))`. It is not a
   Nostr event. The returned name is written into kind 0 as `nip05`. A name
   containing `@` is refused with "Log in with your private key instead".
   Usernames must match `^[0-9a-zA-Z_.-]+$` (`CreateAccountScreen.tsx:88`).
2. Resolution (`contacts.ts:132-164`, used by Add Contact and Invite):
   `npub...` is decoded; 64 hex is used as is; `@domain.tld` queries the root
   identifier `_@domain.tld`; `@name` tries `name` + each of `@arcade.chat`,
   `@damus.io`, `@iris.to`, `@nostrplebs.com`, `@nip05.social` in order;
   `name@domain` queries that identifier. nostr-tools `nip05.queryProfile`
   does the fetch (whether it follows redirects is not visible here).

The profile edit screen lets a user type any `nip05` value with no check.

**NIP-26, NIP-46.** None in the current code (see history above).

### A2. Relays, pool and cache

**Default relays** (`UserStore.ts:97-101`), in order:

```
wss://relay.arcade.city
wss://arc1.arcadelabs.co
wss://relay.damus.io
```

A second list, `ARCADE_RELAYS` (`utils/profile.tsx:16-22`), is used only for
the push registration event (A3, kind 30199):

```
wss://relay.arcade.city
wss://arc1.arcadelabs.co
wss://relay.nostr.band/all
wss://relay.damus.io
wss://nos.lol
```

**Where the list lives.** Only in the MST snapshot (`UserStore.relays`). It is
never published: no kind 10002 (NIP-65), no relay map in kind 3 content, no
kind 10050. Each device has its own list.

**Relay manager UI** (`screens/RelayManagerScreen.tsx`): a "Connected" section
(remove needs at least one left, "You always need at least one relay to use
Arcade") and "Suggested relays" fetched from `https://api.nostr.watch/v1/online`.
A custom relay must parse as a URL with protocol `wss:` or `ws:` and a host
matching `^([a-z0-9]+(-[a-z0-9]+)*\.)+[a-z]{2,}$`; the stored value is
`URL.origin`, so any path (such as `/all`) is dropped.

**`pool.ts`** (verified):

- `ReconnPool extends SimplePool` (`13-45`): on a relay's `disconnect` it waits
  `reconnectTimeout` ms and calls `ensureRelay` again unless the URL is in
  `keepClosed`. Two bugs: the constructor ignores `opts.reconnectTimeout`
  (`this.reconnectTimeout = this.reconnectTimeout || 5000`, so always 5,000 ms),
  and one shared `timer` means a second disconnect cancels the first relay's
  reconnect.
- `NostrPool` (`48-375`) wraps it with the identity and the optional cache.
- **Subscriptions**: `sub(filters, cb, eose?, since?, closeOnEose?)` keys each
  filter by `JSON.stringify(filter)`. A filter already open gets the new
  callback added. New filters open one nostr-tools sub each, across all relays.
  At most **3** live subscriptions are kept: `lruSub = new LRUCache({ max: 3 })`
  (`69`), and the least recently used one is unsubscribed on eviction.
  Filters whose `kinds` include 0 or 3 bypass the LRU and close on EOSE
  (`267, 281-283, 294`).
- **Dedup**: by event id in the SQLite queue and the `on conflict (id) do
  nothing` insert; nostr-tools also dedups per subscription (inferred).
- **EOSE**: `list()` waits for the first EOSE of a new subscription (the
  nostr-tools EOSE across relays; nostr-tools default `eoseSubTimeout` applies,
  inferred), then answers from SQLite.
- **Publish**: `send()` signs, publishes to every relay and resolves on the
  first `ok` from any relay; it rejects with "send timed out" after 3,000 ms
  (`341-352`). `failed` is only logged. `sendRaw()` does the same for an event
  that is already signed (one-time keys).
- **NIP-11**: `setAndCheckRelays(relays, nips)` fetches each relay's NIP-11
  document and keeps those listing every NIP asked for. Nothing in the app calls
  it. `start()` and `addEventCallback()` are also unused.
- **Signature verification**: off. `RelayProvider` passes
  `{ skipVerification: true }` as the subscription options. In nostr-tools 1.x
  that option skips `verifySignature` on every received event (inferred from the
  nostr-tools 1.x API; its source is not installed in the clone). The SQLite
  column `verified` is always written as `0`. Only `nipXXDecrypt` verifies the
  inner event of a kind 99 wrap.

**SQLite cache** (`db/base.ts`), database name `arcade.1`:

```sql
create table if not exists posts (
  id string not null primary key, content string, kind integer,
  pubkey string, sig string, tags string, p1 string, e1 string,
  created_at integer, verified boolean);
```

- `tags` is the JSON array; `p1` and `e1` are the first `p` and first `e` tag
  values only.
- Writes queue in memory and flush in one batch after 500 ms.
- `filterToQuery` turns each filter into `authors -> pubkey IN`, `ids -> id IN`,
  `kinds -> kind IN`, `#e -> e1 IN`, `#p -> p1 IN`, ANDed, filters ORed. It
  ignores `since`, `until`, every other tag filter (`#d`, `#x`, `#g`) and every
  `limit` but the first filter's. So `#d: ["arcade-channels"]` and
  `#x: ["listing"]` do not filter in the cache, and `#e` matches only the first
  `e` tag.
- `latest(filters)` returns `max(created_at)` of matching rows; `list()` passes
  it as `since` to the relays, so a reopened screen asks only for newer events.
- No index besides the primary key.

### A3. Every event kind

"Writer" is who signs. Kinds 40-44 and 4 are standard; the rest marked
"Arcade" are Arcade's own numbers.

| Kind | Name (Arcade) | NIP then | Writer | Tags and content | Filters used | Code |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | Profile | NIP-01 | user | content JSON `{picture, banner, display_name, username, about, nip05, other?}`; `other` is a `selfEncrypt` blob of private settings (A7); tags `[]` | `{kinds:[0], authors:[pk]}` everywhere a name is shown | `UserStore.ts:235-240`, `profile.ts:12-34`, `utils/profile.tsx:29-34`, components |
| 3 | Follows | NIP-02 | user | tags `[["p", pk], ...]` (no relay, no petname), content `""` | `{authors:[me], kinds:[3], limit:1}`; social graph `{kinds:[3,0], authors:[pk]}` | `contacts.ts:56-71, 93-127`, `social.ts:117` |
| 4 | DM, NIP-04 | NIP-04 | user | `[["p", them], ["e", replyTo, relay0, "reply"]?]`, content `b64?iv=b64` | `{kinds:[4], "#p":[me]}` (+ authors) | `private.ts:19-35` |
| 4 | "Blinded" DM | Arcade on kind 4 | pair key (A6) | `[["e", replyTo, relay0, "reply"]?, ["p", them]]`; content `??` cipher of `{"pubkey", "content"}` | `{kinds:[4], authors:[pair keys]}` and the `#p` filter | `ident.ts:203-253`, `private.ts:37-48`, `DirectMessageForm.tsx:106` |
| 5 | Delete | NIP-09 | user | `[["e", id]]`, content `""` | none | `nip28channel.ts:122-130`; never called from the app |
| 40 | Channel create | NIP-28 | user | content JSON `{name, about, picture, is_private:false}`; tags `[["d", name]]` | `{kinds:[40]}` (every channel on every relay) | `nip28channel.ts:6-17, 55-66` |
| 41 | Channel metadata | NIP-28 | user | content JSON meta; tags `[["e", channel_id, relay0]]` (no marker) | `{kinds:[41], "#e":[id]}` with `{kinds:[40], ids:[id]}` | `nip28channel.ts:68-101` |
| 42 | Channel message | NIP-28 | user | `[["e", channel_id, relay0, "root"], ["e", reply_id, relay0, "reply"]?, ...extra]` | `{kinds:[42], "#e":[id], since: now-72h}` | `nip28channel.ts:103-158`, `models/Channel.ts:54-91` |
| 42 | Listing / offer / action | Arcade on NIP-28 | user | extra tags `["x", "listing"|"offer"|"action"]`, `["data", JSON]`, `["g", geohash5]?` (A8) | channel filter plus `#x` | `listing.ts` |
| 42 | "accepted" | Arcade | user | `[["e", channel, "", "root"], ["e", listing, "", "reply"], ["e", offer, "", "accepted"], ["p", owner, ""]]`, content `"accepted"` | none | `ListingDetailScreen.tsx:48-68` |
| 43, 44 | Hide, mute | NIP-28 | — | not built: `muteUser` throws "not implemented yet" | — | `nip28channel.ts:160-162`, `encchannel.ts:288-290` |
| 99 | Wrap | Arcade ("nipXX") | one-time key | `[["p", recipient]]`; content `??` cipher of a signed inner event; `created_at` = inner's | `{kinds:[99], "#p":[me]}` | `ident.ts:256-327` |
| 400 | Encrypted channel create / invite | Arcade, inside 99 | user (inner, signed) | content JSON `{privkey, name, about, picture, id, author}`; tags `[]` | via 99 | `encchannel.ts:84-134` |
| 402 | Encrypted channel message | Arcade | one-time key | `[["p", channel_pub]]`; content `??` cipher of `{"content", "tags", "pubkey"}` | `{kinds:[402], "#p":[channel_pub]}` | `encchannel.ts:174-286` |
| 403 | Encrypted channel metadata | Arcade | one-time key | `[["p", channel_pub]]`; content `??` cipher of `{name, about, picture}` | `{kinds:[403], "#p":[channel_pub]}` | `encchannel.ts:136-172` |
| 1985 | Label (reputation) | NIP-32 (2023 draft) | anyone | read only: first `l` tag's 4th element is JSON with `quality` in 0..1 | `{kinds:[1985], "#p":[pk]}` | `social.ts:277-349` |
| 10003 | Private contacts | Arcade | user | tags `[]`; content `selfEncrypt(JSON [[pk, {legacy, secret}], ...])` | `{authors:[me], kinds:[20003, 10003], limit:1}` | `contacts.ts:56-71, 100-104` |
| 20003 | Private contacts, old | Arcade | — | read only, legacy (commit `f60fb9f` moved to 10003) | as above | `contacts.ts:102` |
| 30040 | Joined channels | Arcade | user | tags `[["d", "arcade-channels"]]`; content `selfEncrypt(JSON [ids])` | `{authors:[me], kinds:[30040], "#d":["arcade-channels"], limit:1}` | `channel.ts:175-219` |
| 30199 | Push registration | Arcade | user | tags `[["d", "arcade-push"], ["p", ARCADE_PUBKEY]]`; content `??` cipher, user key to `ARCADE_PUBKEY`, of `{pubkey, token, privchat_push_enabled, channel_push_enabled, buyoffer_push_enabled, selloffer_push_enabled}` | none | `utils/profile.tsx:51-101` |
| 10101010 | AI conversation | not Nostr | — | a kind number stamped onto rows from `https://api.arcade.chat/user/<npub>/conversations` so the home list can tell them apart | — | `hooks/useConversations.ts:25`, `HomeMessagesScreen.tsx:57` |

`ARCADE_PUBKEY` is `c4899d1312a7ccf42cc4bfd0559826d20f7564293de4588cb8b089a574d71757`
(`utils/profile.tsx:24`), a public key of Arcade's push service.

Redacted example shapes (values shortened):

```json
{"kind":402,"pubkey":"<one-time pubkey>","created_at":1688000000,
 "tags":[["p","<channel pubkey>"]],
 "content":"<b64 ciphertext>??<b64 iv>??1","id":"...","sig":"..."}
```

```json
{"kind":42,"pubkey":"<user>","tags":[
   ["e","<channel id>","wss://relay.arcade.city","root"],
   ["x","listing"],
   ["data","{\"type\":\"l1\",\"action\":\"sell\",\"amt\":0.1,\"price\":29000,\"item\":\"bitcoin\",\"currency\":\"USD\",\"expiration\":3600,\"payments\":[\"...\"]}"],
   ["g","9vfgp"]],
 "content":"<free text>"}
```

`relay0` above is `pool.relays[0]`, the first relay in the user's list.

Kinds Arcade picked collide with today's registry
(`nostr/nips/official/README.md`): **10003** is now NIP-51 "Bookmark list",
**30040** is NKBIP-01 "Curated Publication Index". 99, 400-403, 20003 and 30199
are unregistered. 20003 is in the ephemeral range (NIP-01: 20000-29999 are not
expected to be stored), so the old private contact list would not persist on
standard relays.

### A4. Public channels (`nip28channel.ts`, `channel.ts`)

- **Create** (`Nip28Channel.create`): refuses a name that matches a channel in
  `_knownChannels`, then publishes kind 40 with content
  `JSON.stringify(meta)` and tag `["d", name]`. The `d` tag has no meaning on
  kind 40 (a regular kind); Erik proposed a name tag for NIP-28 in
  nostr-protocol/nips PR #525 ("Idea, add 'name' tag to channel create",
  closed). The duplicate-name check never fetches: `_knownChannels` starts as
  `[]`, which is truthy, so `knownChannels()` returns the local list.
  `CreateChannelScreen` then also calls `setMeta`, so every new channel gets a
  kind 41 too.
- **Channel id** = the kind 40 event id.
- **Metadata**: kind 41, tag `["e", id, relay0]`. `getMeta` reads all 41s and
  the 40 and keeps the one that wins `b.kind > a.kind || b.created_at > a.created_at`.
  It does not check that a 41's author is the 40's author, so anyone can rename
  any channel for Arcade users (NIP-28 says clients SHOULD ignore such 41s).
- **Messages**: kind 42 with the root `e` tag; replies add a second `e` tag
  marked `reply`. No `p` tag for the replied-to author. Chat loads the last
  72 hours (`models/Channel.ts:59-65`) and prefetches each author's kind 0.
- **Reply display** (`ChatScreen.tsx:131`): the `e` tag whose 4th element is
  `reply`; `components/Reply.tsx` fetches that id from the cache.
- **Mute / hide** (43, 44): not built.
- **Members**: none for public channels.
- **Joining**: `ChannelManager.join/joinAll/leave` keep a set of channel ids
  and publish it as kind 30040 `d=arcade-channels`, `selfEncrypt`ed (A3).
  New accounts join three default channels (`UserStore.ts:35-39`,
  `RootStore.ts:9-33`):

  | Channel id (kind 40 event id) | Name | About |
  | --- | --- | --- |
  | `8b28c7374ba5891ea65db9a2d1234ecc369755c35f6db1a54f18424500dea4a0` | The Lobby | Chat about anything |
  | `5b93e807c4bc055693be881f8cfe65b36d1f7e6d3b473ee58e8275216ff74393` | Global Trade | Buy, sell, trade |
  | `3ff1f0a932e0a51f8a7d0241d5882f0b26c76de83f83c1b4c1efe42adadb27bd` | Arcade Feedback | What can we improve? |

- **Discovery** (`ChannelsScreen.tsx:65-103`): a curated list from
  `https://raw.githubusercontent.com/ArcadeLabsInc/arcade-static/main/top.json`
  (kind 40 events keyed by id) merged with every kind 40 in the cache.
- **Invites to public channels**: the changelog claims them; the Invite screen
  only does private channels (A5).

### A5. Encrypted channels ("NIP-112")

#### What the code does (verified, `encchannel.ts`, `ident.ts`)

1. **Create** (`createPrivate`, `84-116`). Generate a channel key pair
   `(c_priv, c_pub)`. The channel id is `c_pub` (hex). For every member and the
   creator, send a kind 99 wrap holding a signed kind 400 whose content is
   `{privkey: c_priv, name, about, picture, id: c_pub, author: creator}`.
   `ChannelManager.create` passes no members, so creation wraps only to self.
2. **Invite** (`invite`, `118-134`; `InviteScreen.tsx:66-114`). The same kind 400
   for each selected pubkey. The screen sends `about: ""` and `picture: ""`.
   "Members" are recorded only locally (`ChannelModel.memberList`).
3. **Wrap (kind 99, "nipXX")** (`ident.ts:256-296`): sign the inner event with
   the user's key; draw a one-time key `e`; encrypt the inner JSON with the
   `??` cipher from `e` to the recipient; outer event signed by `e`, tags
   `[["p", recipient]]`, `created_at` copied from the inner event.
   Unwrap (`298-327`): decrypt with ECDH(me, outer.pubkey) and verify the inner
   signature. A branch for reading your own sent wraps derives a key from
   `privKey * iv mod n`, but encryption uses a random key (the deterministic
   line is commented out at `270-275`), so that branch cannot work. The test for
   it is disabled (`test/ident.test.ts`).
4. **Message (kind 402)** (`send`, `174-201`): one-time key `e`; content =
   `??` cipher from `e` to `c_pub` of `{"content", "tags", "pubkey": sender}`;
   outer tags `[["p", c_pub]]`; signed by `e`. The kind is not inside ("skipping
   kind here, cuz it makes indexing suck", line 188). Reading
   (`decrypt`, `226-257`): ECDH(`c_priv`, outer.pubkey), decrypt, then replace
   the outer pubkey, content and tags with the inner JSON's.
5. **Metadata (kind 403)** (`setMeta`, `136-150`): the same construction with
   `{name, about, picture}`. `getMeta` takes the newest 403 that decrypts, else
   the kind 400 fields.
6. **Listing your channels** (`listChannels`, `31-57`): every kind 99 `#p` me,
   unwrapped; inner kind 400s become channels with `id = getPublicKey(privkey)`.

#### What this scheme provides and leaks

| Property | Verified behavior |
| --- | --- |
| Confidentiality | Anyone holding `c_priv` reads every message ever sent. All members hold it; it is never rotated. |
| Write access | Anyone who knows `c_pub` can write. Encryption is to a public key, and `c_pub` is in the `p` tag of every 402 and 403. A non-member can post messages and change the metadata. |
| Sender authenticity | None. The sender's pubkey is a field in the plaintext JSON, not signed. Any writer can claim to be any member. |
| Integrity | None. AES-CBC without a MAC; ciphertext is malleable. |
| Removing a member | Not possible. No rekey, no "moved to new channel" event. |
| Forward secrecy | None. |
| Metadata | Relays see one stable `p` value per channel: they can count and time all its traffic. They do not see senders (one-time keys). Invites (kind 99) carry the invitee in a `p` tag and the real `created_at`. |
| Membership list | Not published, by design. Not kept anywhere but the inviter's device. |
| Persistence across devices | Broken in the app: login and the channel manager recreate every joined id with `privkey: ""` and `is_private: false` (`LoginScreen.tsx:55-65`, `ChannelManagerScreen.tsx:56-66`). Nothing in the app calls `listChannels` for private channels (`ChannelsScreen` filters them out; `fetchChannelInvite` is commented out in `HomeMessagesScreen.tsx:48`). An invitee never sees an invite in the UI, and the creator loses the channel key on a new device. The key survives only in the `root-v1` snapshot on the original device. |

#### Which proposal this is

The app calls it **NIP-112** (`ChangelogScreen.tsx:121, 156`: "Encrypted
private channels (NIP-112)"). Findings:

- **Confirmed**: NIP-112 is nostr-protocol/nips **PR #580, "NIP-112: Encrypted
  Group Events"**, by `earonesty` (Erik Aronesty, arclib's author), opened
  2023-06-04, 20 commits to 2023-06-09, **still open and never merged**
  (state `OPEN`, last activity 2026-03-04, 33 comments). It adds one file,
  `112.md`. The nips clone has no `112.md`: `git log --all -- 112.md` is
  empty, and no commit message mentions "112", "encrypted group" or
  "encrypted channel" except unrelated merges (#1124, #1126, #1129).
  Erik's only merged nips commits are two NIP-28 doc fixes (#526, 2023-05-13).
- **The text** (head `88ecba85`): every event is a NIP-59 gift wrap (kind
  **1059**, NIP-44 encrypted, one-time key) around a signed inner event;
  wrapped kinds `400` create, `401` invite (carries `["privkey", ...]`),
  `402` metadata, `403` message, `404` moved to new channel (member removal by
  rotation), `405` delegate owner, `406` accept ownership; kinds 407-419
  reserved; an optional NIP-40 expiration "forward secrecy" extension. Only the
  owner's 402/404/405 count.
- **Arcade does not implement that text.** Arcade's numbers are swapped (402 is
  the message, 403 the metadata; the PR has the reverse), its wrap is kind 99
  with Arcade's own AES-CBC cipher (NIP-44 v2 was not final until
  2023-12-20, commit `822b70a`), channel messages are not wrapped around signed
  inner events at all (the sender is an unsigned JSON field), invites are kind
  400 not 401, and 401, 404, 405 and 406 do not exist. Arcade's code is an
  earlier, simpler prototype of the same idea. I could not find a PR revision
  that matches Arcade's numbering exactly: the PR's first commit (`e9783b1a`)
  already has 402 = metadata and 403 = message.
- **Review on the PR**: Paul Miller noted Signal groups are pairwise with no
  shared secret; Jonathan Staab argued for separating key distribution
  (bunker, DMs, FROST) from the channel format; others asked for a member list
  and owner deletion. Nothing converged.
- **What superseded it**: NIP-17 (merged 2024-04-24, #686) for small groups,
  NIP-29 (merged 2024-02-28, #566) for relay-based groups, NIP-EE (MLS,
  merged 2025-08-27, #1427) which upstream itself now marks `unrecommended`:
  "superseded by the Marmot Protocol" (#2154, 2025-12-11). See B4.

### A6. Direct messages (`private.ts`)

Two modes, chosen per contact (`Contact.legacy`) and per DM screen
(`legacy: secret ? false : legacy`, `UserScreen.tsx:156`):

1. **Legacy NIP-04** (`send`, `19-35`): kind 4, `["p", them]`, optional
   `["e", replyTo, relay0, "reply"]`, nostr-tools `nip04.encrypt`
   (AES-256-CBC, key = raw ECDH x, `?iv=`). New contacts default to
   `legacy: true` (`AddContactScreen.tsx:72`, `ContactModel.legacy` default).
2. **"Blinded" (`send44X`, `ident.ts:203-225`)**:

   ```
   x       = ECDH x(me, them)                 (same both ways)
   P       = x * G                            the pair's shared "pseudonym" key
   inner   = JSON {"pubkey": me, "content": text}
   event   = kind 4, pubkey P, signed with secret x,
             tags = caller tags (reply e tag, then ["p", them] added by the app),
             content = ?? cipher(me -> them, inner)
   ```

   The library dropped the `p` tag in `ccb1fe3` ("remove p tag") and added it
   back in `8ea435d` ("dox myself", with the comment "no way to sender-index
   these properly"). Today the app passes it (`DirectMessageForm.tsx:106`).
   So the relay sees the recipient and a stable pair key; it does not see the
   sender's real key. Either party can sign as `P`, and the inner `pubkey` is
   unsigned, so either party can forge a message "from" the other.

**Reading** (`decrypt`, `71-116`): own NIP-04 sent messages decrypt with the
`p` tag; content ending in `??1` is tried against each known contact's pair key
(`nip44XDecryptAny`); kind 99 goes through `nipXXDecrypt`; anything else is
NIP-04. Results are cached by id (LRU 1,000). Filters (`filter`, `148-168`):
`{kinds:[4], "#p":[me]}` (with `authors` when pubkeys are given),
`{kinds:[4], authors:[me], "#p":[them]}`, and `{kinds:[4], authors:[pair keys]}`.
The DM screen loads 100 (`DirectMessageScreen.tsx:82`).
`components/Reply.tsx` decrypts a quoted kind 4 with NIP-04 only, so a quoted
blinded message shows ciphertext.

The privacy screen also saves `prefersLegacyDMs` into the private part of
kind 0 ("Currently, this feature only supported by Arcade and not it compatible
with other clients."). Nothing reads it.

No NIP-17, NIP-44 or NIP-59 is used anywhere (the `nip44X` names are Arcade's).

### A7. Contacts and social (`contacts.ts`, `social.ts`, `utils/nip02.tsx`, `profile.ts`)

- **Contact** = `{pubkey, secret, legacy}`. `ContactManager.write` publishes
  **two** events every time (`56-71`): kind 3 with a `p` tag for every
  non-secret contact, and kind 10003 with `selfEncrypt` of every contact that
  is `secret` or `legacy`. "Private follow" (`UserScreen.tsx:53-62`) sets
  `secret`, which removes the key from kind 3.
- `readContacts` merges the newest kind 3 and the newest 10003/20003. A legacy
  kind 3 from another client is read with `legacy: true`
  (`utils/profile.tsx:36-46`).
- `utils/nip02.tsx` turns a key list into `["p", key]` tags; unused.
- **Profile private settings** (`profile.ts`): `save(prof, private_keys)` moves
  the named keys into `selfEncrypt(JSON)` under content field `other` of kind 0.
  Used for the four push flags and `prefersLegacyDMs`. Every other client sees
  an opaque `other` string in the public profile.
- **Feeds**: no kind 1 notes, no home feed of follows. `UserFeed.tsx` is unused.
- **Reactions** (NIP-25): none in the current code (the pre-Ignite app had them,
  `2571ea4`, Jan 2023).
- **Threading**: NIP-10 marked `e` tags (`root`, `reply`) on kinds 42 and 4,
  with `relay0` as the relay hint. No `p` tags on replies. Plus Arcade's own
  `accepted` marker (A3).
- **Social graph** (`ArcadeSocial`, `social.ts`): starts automatically when the
  provider mounts. Breadth-first over kind 3 (and kind 0 events whose tags are
  all valid `p` tags), `GRAPH_DEPTH = 2`, `STALE_GRAPH = 1 week`, one step per
  second when idle. `distance()`: 0 self, 1 followed, 2 followed-by-followed,
  3 unknown. `weight = 1/d²` (self 10,000).
- **Reputation** (`getReputation`, `277-349`; the "ArcTrust" wiki page
  `github.com/ArcadeLabsInc/arcade/wiki/ArcTrust-Reputation-via-Social-Graph-and-NIP-32`):
  newest kind 1985 per author tagging the user; the first `l` tag's 4th element
  must be JSON with `quality` between 0 and 1; result = weighted mean.
  `components/User.tsx:47-53` runs it for every rendered user. That 4th-element
  JSON was in NIP-32 for six months: added 2023-05-23 (`2372874`), removed
  2023-11-09 (`c945d8b`, "Remove label annotations from NIP 32"). Nothing in
  Arcade writes a kind 1985.

### A8. Marketplace (`listing.ts`, `comment_to_trade.ts`)

Listings are a peer-to-peer trade board on top of NIP-28 channel messages.

- **Listing** (`post`): kind 42 in the channel, tags
  `["x", "listing"]`, `["data", JSON {type:"l1", action:"buy"|"sell", amt, price, item, currency, expiration, payments[]}]`,
  optional `["g", geohash.substring(0, 5)]` ("roughly the size of a large
  airport"). `min_amt` is in the type but never written. `expiration` is input
  as text ("1 hour", "2d", "3 weeks") and stored as seconds (`convertToSeconds`,
  month = 30 days).
- **Offer** (`postOffer`): `["x", "offer"]`, `["data", {type:"o1", amt, price, currency, expiration, payment}]`,
  `g`; sent as a kind 42 reply to the listing (`e ... reply`), or, when
  `listing_pubkey` is given, as a NIP-04 DM with the same tags.
- **Action** (`postAction`): `["x", "action"]`, `["data", {type:"a1", action:"accept"|"finalize"|"comment"}]`,
  reply to the offer, public or NIP-04. Unused by the app.
- **Reading**: `list` / `listOffers` / `listActions` filter on `#x` (relays
  apply it; the cache does not), parse `data`, drop expired
  (`created_at + expiration < now`). `public = kind != 4`.
- **`comment_to_trade.ts`**: a text command grammar,
  `^(B?U?Y?|S?E?L?L?)\s+([.\d,]+)-?([.\d,]+)\s*(\w+)\s*@\s*([\d,.]+)\s*\[(\w+)\]\s*(.*)?$`,
  for example `BUY 0.1-0.5 BTC @ 29000 [PAYPAL] exp:7d`, and `createCommand` to
  write one. Nothing in the app uses it (only its test).
- **The UI is unreachable and broken** (verified): no screen navigates to
  `Listing`. `ListingScreen.tsx` calls `channelStore.fetchMessages`,
  `channelStore.listing` and `channelStore.reset`, which `ChannelStoreModel`
  does not have, against a hard-coded channel
  `d4de13fde818830703539f80ae31ce3419f8f18d39c3043013bee224be341c3b` ("#TODO:
  replace with real channel id"). `ListingDetailScreen.tsx` and
  `UserOffer.tsx` treat the `RelayContext` object as a `NostrPool`, so
  `pool.send` is undefined. The offer form's placeholders are "USD, EUR, etc."
  and "PayPal, Venmo, Cash App,...".

### A9. Content parsing (`utils/parser.tsx`, `MessageContent.tsx`, `LinkPreview.tsx`)

- `parseReferences(event)` from nostr-tools 1.x finds `nostr:` URIs (NIP-21/27)
  and NIP-08 `#[n]` index mentions (inferred from that function's 1.x
  behavior). Profile and note references are colored cyan and are not
  tappable; note ids collect into `content.notes` and are never rendered.
  Mentions are not replaced with names.
- URLs via `extract-urls`. Ending in `.jpg .jpeg .gif .png .webp .avif`: shown
  as an image and removed from the text. `.mp4 .webm .mov .ogv .avi .mp3`: an
  `expo-av` video. The first other link becomes a `link-preview-js` Open Graph
  card and is removed from the text; later links stay as tappable text.
- Hashtags: `#(\w+)` colored; no `t` tags are read or written.
- Images are uploaded to `https://nostrimg.com/api/upload` and the URL is
  appended to the message text. No NIP-94, NIP-92 `imeta` or Blossom.
- The app's deep-link scheme is `arcade` (`app.json`); there is no `nostr:`
  handler.

### A10. Lightning and zaps

None. No NIP-57, no LNURL, no `lud06`/`lud16`, no NWC. `ArcadeIdentity`
carries unused `bitcoinAddress` and `lnUrl` fields.

### A11. Other services that see identity

| Service | What it receives | Code |
| --- | --- | --- |
| AWS Lambda NIP-05 registrar | name, pubkey, a signature | `UserStore.ts:51-67` |
| `api.arcade.chat` (AI chat) | the user's npub and every AI message | `hooks/useConversations.ts`, `useSendMessage.ts`, `components/Faerie/MessageInput.tsx` |
| Arcade push service via kind 30199 | Expo push token and preferences, encrypted to `ARCADE_PUBKEY`, on 5 relays | `utils/profile.tsx` |
| `nostrimg.com` | uploaded images | four forms |
| `api.nostr.watch`, `api.nostr.band`, `raw.githubusercontent.com` | relay suggestions, trending profiles, channel list | Relay manager, Add Contact, Channels |

### A12. Security review

| # | Finding | Severity | Where |
| --- | --- | --- | --- |
| 1 | Received events are not signature-checked (`skipVerification: true`); the cache stores them as fact. A relay can forge any profile, follow list, channel message, listing or NIP-04 DM header. | High | `RelayProvider.tsx:40` |
| 2 | Encrypted channels: anyone who sees the channel pubkey can write; senders are unsigned claims; no MAC; no removal; one static key forever. | High | A5 |
| 3 | Kind 41 metadata is taken from any author. | Medium | `nip28channel.ts:78-101` |
| 4 | NIP-04 (unauthenticated AES-CBC, sender and recipient public) is the default DM mode. | Medium | A6 |
| 5 | Arcade's `??` cipher is unauthenticated AES-CBC everywhere it is used (blinded DMs, wraps, channels, self-encrypted lists, push registration). | Medium | `ident.ts:107-181` |
| 6 | Blinded DMs expose the recipient (`p` tag) and a stable pair key; either party can forge messages from the other. | Low | A6 |
| 7 | The private key is stored twice (Keychain, and inside the AsyncStorage snapshot under an app key); channel keys only in the snapshot. Fixed IV per storage key. | Medium | A1 |
| 8 | Logout leaves the SQLite cache and every private channel key (`channelStore` in `root-v1`) for the next account. | High | A1 |
| 9 | The nsec and hex key are copied to the clipboard on request; no warning or expiry. | Low | `BackupScreen.tsx` |
| 10 | Private settings ride inside public kind 0 as an opaque `other` field; push registration publicly tags `ARCADE_PUBKEY`, showing the user enabled Arcade push. | Low | A7, A3 |
| 11 | No replay or freshness rules: an old kind 3, 10003 or 30040 resent by a relay is accepted when newer ones are missing (newest-wins only among what arrives). | Low | `pool.get` |
| 12 | Login accepts any string of 60+ characters as hex. | Low | `LoginScreen.tsx:33-42` |
| 13 | Kind numbers 10003 and 30040 now mean bookmarks and publication indexes; other clients may read or overwrite Arcade's private data there. | Medium | A3 |

### A13. Dead, broken and boilerplate

- Dead: `useAnonUser`/`useUser`, `UserFeed`, `utils/nip02.tsx`,
  `comment_to_trade.ts`, `postAction`, `Nip28Channel.delete`,
  `NostrPool.setAndCheckRelays/start/addEventCallback`, `addEventCallback` and
  `muteUser` stubs in both channel classes, the "decrypt my own sent wrap"
  branch of `nipXXDecrypt`, the `bitcoinAddress`/`lnUrl` fields.
- Broken: private channels across devices and for invitees (A5), the
  marketplace screens (A8), `ReconnPool` timeout and timer (A2), cache tag
  filters (A2), quoted blinded DMs (A6).
- Boilerplate: the Ignite `DemoScreen`, `Demos` route and `docs/ignite` are
  template leftovers and have no Nostr use.

---

## Part B: what each becomes now

Naming on our side: the app is OpenAgents iOS; payments are Bitcoin and
Lightning only; auth is email code plus Nostr. NIP status below is from
`nostr/nips/official/` (`400d157c`). "Module" means a file in
`coax/lib/nostr/` unless another path is given; each was read, not guessed.

### B0. Summary map

| Arcade item | Current NIP and status | Kinds now | Our module | State |
| --- | --- | --- | --- | --- |
| Keys, nsec/hex import | NIP-19 | — | `nip19-core.coax`, `bech32-core.coax` | Decode/encode complete, nsec included (`nostr:` never carries nsec). |
| Phone key custody | — | — | `apps/openagents-ios/OpenAgents/SignIn/NostrKey.swift` | Exists: Keychain service `com.openagents.app.nostr`, account `nsec-v1`, device only, libsecp256k1 signing. Generate only, no import. |
| Seed words | NIP-06, `unrecommended` ("prefer a single nsec") | — | `nip06.coax` | Key at `m/44'/1237'/0'/0/0`; used by web `/login` words. |
| Encrypted key backup | NIP-49, draft | — | `crypto/scrypt_nip49.coax` (experimental), `xchacha20_poly1305.coax`, `text/nfkc.coax`; composed in `nostr-lab-keys.coax` | No `nip49.coax`; scrypt erasure and production approval open. |
| Remote signing | NIP-46, draft | 24133 | `nip46.coax` | URIs, request/response, NIP-44 transport, permissions; stricter than the NIP (NIP-04 methods refused). |
| Signer contract | NIP-07/46 | — | `keys.coax` | Request lifecycle and reply verification; local test slot. |
| NIP-26 delegation | `unrecommended` | — | `nip26.coax` | Read-only claim reporting; never authority. |
| NIP-05 | NIP-05 | — | `nip05.coax` | Offline parse/URL/check; host fetches; redirects refused. |
| Relay list | NIP-65 | 10002 | `nip65.coax`, `outbox.coax` | Reader and outbox planner (64 relays, 64 authors). |
| DM inbox relays | NIP-17/51 | 10050 | `nip17.coax`, `nip51.coax` | Read and required by `nip17_plan`. |
| Relay client | NIP-01, NIP-42 | 22242 | `relay.coax`, `message.coax`, `coax/lib/net/relay_conn.coax`, `nip42.coax`, `nip42-event.coax` | Connection state machine, AUTH, resubscribe with `since`, resend unacknowledged. |
| Event verify/sign | NIP-01 | — | `event.coax`, `event_verify_fast.coax`, `event_sign.coax` | Complete. |
| Local event cache | — | — | `store.coax` (in memory, relay face) | No durable client cache. |
| Profile | NIP-01 (+ NIP-24 fields) | 0 | `metadata.coax`, `profiles.coax` | Reads name/about/picture; others kept. |
| Follows | NIP-02 | 3 | `follows.coax` | Reader. |
| Private follows | NIP-51 private items | 30000 follow sets | `nip51.coax`, `nip51-private.coax` | NIP-44 sealing to self; reads NIP-04 legacy content when allowed. |
| Joined chats | NIP-51 | 10005 (NIP-28 chats), 10009 (NIP-29 groups) | `nip51.coax` | Tag rules for both. |
| NIP-04 DMs | NIP-04 `unrecommended`, "deprecated in favor of NIP-17" | 4 | `nip04.coax` | Read only; `nip04_encrypt` is a named refusal. |
| DMs | NIP-17 + NIP-44 + NIP-59 | 14, 15, 13, 1059 | `nip17.coax`, `nip59.coax`, `nip44.coax`, `nip44-key.coax` | Send and open complete for ≤10 recipients; see B3 for what is not built. |
| Small private groups | NIP-17 rooms | 14 in 1059 | `nip17.coax` | Same module. |
| Relay-based groups | NIP-29, draft | 9000-9022, 39000-39003, h tag | `nip29.coax` (relay side), `nostr/relay/relay_groups.coax`, `group_read_policy.coax` | Relay side only; relay does not advertise 29 (B4). |
| MLS groups | NIP-EE `unrecommended`, superseded by Marmot | 443-445, 10051 | none | Not built. |
| Public channels | NIP-28 `unrecommended`, "try NIP-29 instead" | 40-44 | `retired.coax` | Classified retired; `rt_write` refuses kinds 40-44 and 4. No reader module. |
| Chat messages | NIP-C7, draft | 9, `q` replies | `nipc7.coax` | Reader and reply-tag writer. |
| Communities | NIP-72 `unrecommended` | 34550, 4550 | `nip72.coax` | Read only. |
| Threads | NIP-10 (kind 1 only) | 1 | `nip10.coax` | Marked and positional e tags. |
| Comments | NIP-22 | 1111 | `nip22.coax` | Reader with OpenAgents queue tags. |
| Reactions | NIP-25 | 7, 17 | `nip25.coax` | Reader and tally. |
| Deletion | NIP-09 | 5 | `nostr/relay/store_delete.coax` | Relay side; no client writer module. |
| Mentions, links | NIP-21, NIP-27, NIP-08 (`unrecommended`) | — | `nip27.coax`, `nip19-core.coax` | Finds `nostr:`, `#[n]`, `#tag`, `:emoji:` by byte range. |
| Listings | NIP-99, draft (NIP-15 `unrecommended`, "try NIP-99") | 30402, 30403 | `nip99.coax`; `nip15.coax` read only | NIP-99 reader only; Bitcoin-only amounts. |
| Labels | NIP-32 | 1985 | `nip32.coax` | Reader and trusted grouping. |
| Reports | NIP-56 | 1984 | `nip56.coax` | Reader and tally. |
| Zaps | NIP-57 | 9734, 9735 | `nip57.coax` | LUD-16, request writer, receipt check. |
| HTTP auth | NIP-98 | 27235 | `nip98_sign.coax`, `coax/web/api/nip98.coax` | Complete; iOS phone sign-in uses it. |
| Search | NIP-50 | — | `nip50.coax` (relay side) | Our relay answers search; client `NostrFilter` has no `search` field. |
| App data | NIP-78 | 30078 | none needed | Plain addressable event. |

### B1. Identity and keys

- **Current NIPs.** NIP-19 for `npub`/`nsec`. NIP-49 (`ncryptsec`) is the
  standard encrypted export. NIP-46 lets a user keep the key in another app.
  NIP-06 is now `unrecommended`. NIP-26 is `unrecommended`.
- **What we have.** OpenAgents iOS already has a phone key
  (`NostrKey.swift`): 32 random bytes in the Keychain, service
  `com.openagents.app.nostr`, account `nsec-v1`, not synchronizable, available
  after first unlock, kept across reinstall, signed in app memory by
  libsecp256k1 (`OpenAgents/Platform/NostrSign.c`), wiped after each use. The
  phone already signs NIP-98 requests to log in to openagents.com
  (`PhoneSignIn.swift`). Our signer contract is `keys.coax`; NIP-46 is
  `nip46.coax`. NIP-49 exists only as the experimental `scrypt_nip49.coax` plus
  the Nostr Lab composition.
- **Migration.** Default to the phone key, as today. Arcade's "Enter access
  key" flow becomes an import that writes an nsec or 64-hex key into the same
  Keychain slot (needs an owner decision, B14). Never keep a copy in app
  storage the way `root-v1` did. Export goes through NIP-49 once its module is
  approved, not the clipboard. External keys go through NIP-46. The
  `@arcade.chat` registrar is Arcade's service; do not depend on it. NIP-05
  checks use `nip05.coax` with the host doing the fetch and refusing
  redirects. Kind 0 uses NIP-01/24 field names (`name`, `display_name`,
  `about`, `picture`, `banner`, `nip05`, `lud16`); read Arcade's `username` as
  a fallback for `name`, and never write an `other` field.

### B2. Relays, outbox and cache

- **Current NIPs.** NIP-65 kind 10002 publishes a user's read and write relays
  (the outbox model). NIP-17 adds kind 10050 for DM inboxes. NIP-42 AUTH.
  NIP-11 for relay documents.
- **What we have.** `relay.coax` is the per-connection state machine:
  generations, capped jittered backoff, resubscribe with `since` = newest seen
  (inclusive, dedup drops the boundary), resend publishes with no `OK`, the
  `auth-required:` retry once after AUTH. `outbox.coax` picks a small relay set
  per author (`ob_cover`) and merges one subscription across relays
  (`ob_sub`: one copy per id, caught up when every relay sent EOSE, closed or
  timed out). `nip65.coax` reads 10002. `message.coax` writes REQ/EVENT/CLOSE
  and reads the five relay frames; `nostr_message_event_verify_vartime`
  verifies. relay.openagents.com is the Coax relay (`nostr/relay/`, cut over
  2026-11-12); it requires NIP-42 for writes and restricts reads of kind
  1059/21059 to the event's one `p` recipient (`nostr/relay/relay_private.coax`).
- **Migration.** Arcade's per-device list becomes a published kind 10002 and a
  kind 10050; default both to relay.openagents.com, and let the user add
  relays. Replace the 3-subscription LRU with the outbox planner. Verify every
  event before it enters any store (Arcade skipped this). Arcade's SQLite cache
  has no equivalent client module: `store.coax` is the relay's in-memory face,
  and the durable store work (`nostr/relay/store_*.coax`) is server-side.

### B3. Direct messages

- **Current NIP.** NIP-17: an unsigned kind 14 (chat) or 15 (file) rumor,
  sealed in kind 13 by the author (NIP-44 v2), gift wrapped in kind 1059 from a
  one-time key to each recipient and once to the sender. Seal and wrap
  timestamps are randomized up to two days back. Wraps go only to the
  recipient's kind 10050 relays; no list means "not ready", send nothing.
  NIP-04 is `unrecommended`.
- **What we have.** `nip17.coax` exposes `nip17_plan` (reads every
  participant's 10050 before any wrap exists; refuses a list signed by anyone
  else; refuses the whole send if one member is not ready), `nip17_send`
  (N+1 wraps, refuses any kind but 14/15), `nip17_open` (via
  `nip59_unwrap_checked`: wrap id, signature, `p` tag, NIP-44, seal id/kind/
  signature, rumor unsigned, seal key equals rumor key), `nip17_order`,
  `nip17_subject`. Limits: `N17_MOST = 10` recipients, 8 relays per list.
  Room id = SHA-256 of sorted member keys, local only. `nip59.coax`: rumor at
  most 28,672 bytes, `NIP44_COMPAT` profile. `nip44.coax`: MAC checked before
  decrypting; `NIP44_COMPAT` (65,535) and `NIP44_EXTENDED` (917,504).
  `nip04.coax` decrypts old kind 4 and cannot encrypt.
- **Not built in `nip17.coax`** (its own header): reactions and deletes inside
  a room, edits, disappearing messages (expiration on each wrap), and kind 15
  file encryption ("this stack has no AES-GCM"; note that
  `coax/lib/crypto/aes_gcm.coax` now exists for NIP-VAULT, so the primitive is
  available).
- **Migration.** Every Arcade DM mode becomes NIP-17. Old NIP-04 threads are
  readable with `nip04.coax`. Old blinded DMs need the Arcade decrypt adapter
  (B15). Arcade's per-contact "legacy" toggle and `prefersLegacyDMs` go away.

### B4. Encrypted groups: what replaces "NIP-112"

Three candidates exist today.

| | NIP-17 rooms | NIP-29 groups | Marmot (MLS) |
| --- | --- | --- | --- |
| Status | draft, merged 2024-04-24 | draft, merged 2024-02-28 | Marmot repo "Status: adopted"; NIP-EE is `unrecommended` in its favor |
| Encryption | End to end, NIP-44 per wrap | None by the protocol; `private` hides reads from non-members at the relay; the relay reads everything | End to end, MLS (RFC 9420); forward secrecy and post-compromise security |
| Size | NIP-17: "more than 10 participants should find a more suitable messaging scheme" | Any | Large, log-scale key updates |
| Membership | The `p` tags of each message; changing them makes a new room; no admins, no bans | Relay-enforced, roles, invites (9009, `code`), join/leave (9021/9022), moderation (9000-9008) | MLS add/remove commits; admins in group state |
| Metadata | No public group id; relays see one-time wraps per recipient | The relay knows members, roles and content; `h` tag on every event | One group id in the `h` tag of kind 445; members and size hidden |
| Our library | `nip17.coax`, complete for send/open | `nip29.coax` relay decisions, `relay_groups.coax` effects, `group_read_policy.coax`; no client module | Nothing; crypto lacks X25519, HPKE and AES-128-GCM, and Ed25519 is verify only |
| Our relay | Serves 1059 to its `p` recipient only | Group code present, but `supported_nips` in `relay_server.coax:597` is `[1,9,11,17,40,42,45,50,59,62,65,67,70,77,94]`, no 29 | — |

Notes from the texts:

- NIP-29 group ids are relay-scoped; the relay signs 39000 (metadata, flags
  `private`, `restricted`, `hidden`, `closed`), 39001 admins, 39002 members,
  39003 roles. Events carry `["h", id]` and SHOULD carry
  `["previous", <8 hex>, ...]` references to recent events. Chat inside a group
  is NIP-C7 kind 9 (split out of NIP-29 in #1591, 2024-11-21). A user's groups
  are listed in kind 10009.
- Marmot's spec lives at github.com/marmot-protocol/marmot (MIT); the old
  NIP-EE kinds are 443 (KeyPackage), 444 (Welcome, sealed and gift wrapped),
  445 (group message, `h` = Nostr group id) and 10051 (KeyPackage relays).
  `retired.coax` classifies 443-445 and 10051 as retired NIP-EE forms, so
  `rt_write` would refuse to sign them today.

**Recommendation.**

- **Small private groups (up to 10 people): NIP-17 rooms.** They are end to
  end encrypted, publish no group id, and `nip17.coax` already sends and opens
  them, refusing a send if any member is not ready. They fit Arcade's "private
  channel with a few contacts". Adding or removing a person starts a new room;
  the app shows rooms with the same name in sequence.
- **Large groups: NIP-29 on relay.openagents.com,** public or `private` +
  `closed`. It gives what Arcade's channels lacked: real membership,
  removal, moderation and invites, all enforced by one relay we run. It is not
  end to end encrypted, and the app must say so plainly. The relay side is
  mostly written; the work is a client module and advertising 29.
- **Marmot only if the owner needs end-to-end encryption above 10 people.**
  It is the only current design with that property, and it is a large build
  (MLS, HPKE, X25519, AES-128-GCM, KeyPackage handling, ratchet tree state).

### B5. Public channels

- **Current NIP.** NIP-28 is `unrecommended`, "try NIP-29 instead". NIP-72
  communities are also `unrecommended` toward NIP-29. NIP-C7 defines the chat
  message (kind 9, `q` replies).
- **What we have.** `retired.coax` marks kinds 40-44 as retired and `rt_write`
  refuses to sign them. There is no NIP-28 reader module, but reading needs
  only `event.coax`, `tags.coax` and the marked `e` tags (`nip10.coax` reads
  markers for kind 1 only). `nipc7.coax` reads kind 9 and writes reply tags.
  Nostr Lab's social card already walks a NIP-29 group (create, join, post,
  moderator delete) on the relay side.
- **Migration.** New public channels are open NIP-29 groups with kind 9 chat.
  Arcade's "joined channels" list becomes kind 10009 (NIP-51 "Simple groups").
  If the three default NIP-28 channels still have history on public relays,
  show it read-only; never post kind 42.

### B6. Contacts and social

- **Follows**: kind 3 (NIP-02) as Arcade did; `follows.coax` reads it.
- **Private follows**: NIP-51 private items, NIP-44 to self, in a kind 30000
  follow set; `nip51-private.coax` seals and opens them (`nl_seal` writes only
  NIP-44). Do not write kind 10003: that is the bookmark list now.
- **Joined chats**: kind 10009 for NIP-29 groups; kind 10005 only to remember
  old NIP-28 channels. Do not write kind 30040.
- **App settings**: kind 30078 (NIP-78) with NIP-44-to-self content, or keep
  them in the account; never inside kind 0.
- **Threads and replies**: NIP-10 now covers kind 1 only and says replies to
  other kinds MUST use NIP-22 kind 1111 (`nip22.coax`). Chat replies use the
  C7 `q` tag. Put the replied-to author in a `p` tag (Arcade did not).
- **Reactions**: NIP-25 kind 7, `nip25.coax` (reader and tally; one per
  author per target).
- **Reputation**: drop ArcTrust as built; its `quality` annotation is no longer
  in NIP-32. If wanted later, `nip32.coax` groups labels from signers the
  caller trusts and `nip56.coax` tallies reports; the social-distance weights
  are app policy.
- **Mute**: NIP-51 kind 10000, `nip51.coax` answers whether a mute list mutes
  a pubkey, event, hashtag or word.

### B7. Marketplace

- **Current NIP.** NIP-99 classified listings (kind 30402 active, 30403 draft),
  addressable by `d`, with `title`, `summary`, `published_at`, `location`,
  `status` and `["price", amount, currency, frequency?]`. NIP-15 stalls and
  products are `unrecommended` ("too complicated, try NIP-99").
- **What we have.** `nip99.coax` reads listings and turns a BTC or sats price
  into satoshis; any other currency reads but never converts. It has no
  writer. `nip15.coax` reads stalls, products and checkout messages, Bitcoin
  only. `mkt/order.coax` is OpenAgents' own RFQ-to-paid state machine
  (Lightning settlement).
- **Migration.** A listing becomes kind 30402 with a BTC/sats price and a
  `g` geohash (5 characters, as Arcade did), not a kind 42 in a channel.
  Offers and accepts go over NIP-17 to the listing's author, referencing the
  listing's `a` address; public questions are NIP-22 comments. Arcade's fiat
  payment methods (PayPal, Venmo, Cash App) do not carry over: payment is
  Bitcoin and Lightning only. `comment_to_trade.ts` has no successor; if a
  command line is wanted, it should go through the app's typed command parser,
  not a regex.

### B8. Content parsing

`nip27.coax` finds `nostr:` references (decoded by `nip21_decode_checked`,
which never decodes nsec), NIP-08 `#[n]` (read only), `#hashtags` and
NIP-30 `:emoji:` by byte range, with a rule that keeps `https://site/nostr:...`
and `12:30:45` as text, and matches each against the event's tags. Use it for
mentions (render names from kind 0, tap to open a profile) and quotes. Hashtags
should be written as `t` tags too. Media should move from nostrimg.com to our
own upload, described with NIP-92 `imeta` (no module checked here; open
question).

### B9. Zaps

New, not a port. `nip57.coax` covers LUD-16 to LNURL-pay, the LNURL response
(`allowsNostr`, `nostrPubkey`), the kind 9734 request and kind 9735 receipt
checks (description hash, amount, mainnet `lnbc` only). Nostr Lab's Lightning
card exercises it with test invoices. OpenAgents iOS today only reads a Spark
balance; paying needs a wallet path, which is outside this doc.

### B10. Auth

- NIP-42: `nip42.coax` (client template and relay check, bound to challenge,
  relay URL and connection generation). Our relay requires it for writes and
  for reading one's own gift wraps.
- NIP-98: `nip98_sign.coax` signs; `coax/web/api/nip98.coax` checks (one use,
  60 s). iOS phone sign-in already uses it. Arcade had neither.

### B11. Search, discovery and lists

- NIP-50: our relay answers `search` (`nip50.coax`, word index). The client
  `NostrFilter` in `filter.coax` has no `search` field, so a client cannot ask
  yet.
- Discovery of people: NIP-05 lookups and follows-of-follows from kind 3, not
  `api.nostr.band` trending. Discovery of groups: kind 39000 from our relay
  and kind 10009 lists of people you follow, not a GitHub JSON file.
- NIP-51 lists cover mutes, pins, bookmarks, follow sets, public chats and
  groups (`nip51.coax`, `nl_allowed`).

### B12. Existing UI the port can reuse

- `apps/openagents-web/nostr-chat.coax` (1,226 lines): a scripted workbench on
  `components/kit/frames.coax` and `stage.coax` in the mono skin, showing a key,
  an event's bytes, hash and signature, a relay round trip, NIP-44 and a NIP-59
  wrap, all computed by the library. Its stage pieces show what a message did
  on the wire; it is a demo, not a chat client.
- `nostr-lab-private.coax`: NIP-44 between two keys, a NIP-59 wrap, and a NIP-17
  message to two people with each wrap opened by its addressee. This is the
  closest existing code to a NIP-17 room.
- `nostr-lab-relays.coax`: the library's relay in process (message_server,
  relay_route, store with tombstones), NIP-42, NIP-65 outbox, NIP-77, NIP-13.
  Usable as a local test relay for the port.
- `nostr-lab-social.coax`: follows, NIP-10 + NIP-25, NIP-23, NIP-51 private
  items, and a NIP-29 group on the relay side.
- `nostr-lab-keys.coax`: NIP-06, NIP-49, NIP-46, NIP-98, NIP-05 cards.
- `nostr-lab-more.coax`: NIP-22, NIP-C7, NIP-7D, NIP-A4, kind 0, NIP-39.
- `nostr-lab-lightning.coax`: NIP-57, NIP-47, NIP-75, NIP-A3 with test invoices.
- OpenAgents iOS: `NostrKey.swift`, `NostrSign.c`, `PhoneSignIn.swift`.

The lab cards are teaching surfaces with published test keys; they show which
library calls a real screen makes, not production screens.

### B13. Gap list

What the Arcade port needs that `coax/lib/nostr` lacks or only partly has:

1. **NIP-29 client.** No module writes join/leave requests (9021/9022), `h`
   and `previous` tags, or reads relay-signed 39000-39003 against the relay's
   NIP-11 `self` key. The relay side exists but NIP-29 is not in
   `supported_nips`.
2. **Client event cache.** No durable, verified, client-side store with
   replaceable/addressable rules and logout wipe (Arcade's SQLite role).
3. **NIP-17 extras.** Reactions, deletes and edits inside rooms, expiration,
   kind 15 file encryption; the 10-recipient cap is the NIP's.
4. **NIP-99 writer.** Reader only.
5. **NIP-09 client writer.** Deletion exists only in the relay store.
6. **Search in client filters.** `NostrFilter` lacks `search`.
7. **NIP-49 module.** Experimental scrypt, no reviewed `ncryptsec` reader/writer.
8. **Key import on iOS.** `NostrKey` only generates.
9. **NIP-28 reader** for read-only history (small; kinds 40-42 with marked `e`
   tags).
10. **Arcade legacy decrypt** (only if the owner wants old data, B15): no
    module, though ECDH x (`crypto/ecdh.coax`), HKDF-SHA256 (`hkdf.coax`) and
    AES-256-CBC decrypt (`aes.coax`) all exist.
11. **MLS / Marmot.** Nothing, and missing primitives (X25519, HPKE,
    AES-128-GCM, Ed25519 signing).
12. **Media metadata.** NIP-92 `imeta` / NIP-94 / Blossom upload not checked
    for this port (open question).
13. **Kind 0 writer** for NIP-24 fields including `lud16`: `metadata.coax`
    reads three fields only.
14. **ECDH on OpenAgents iOS.** The vendored libsecp256k1 is built with
    `-DENABLE_MODULE_EXTRAKEYS=1 -DENABLE_MODULE_SCHNORRSIG=1` only
    (`apps/openagents-ios/project.yml` line 88), so the phone can sign but
    cannot compute the NIP-44 conversation key. Every encrypted item in B3
    and B4 needs the ECDH module (or a pure-Coax ECDH) on the phone first.
    This confirms `docs/arcade/04-target-app.md` §1.

Cross-check with `docs/arcade/04-target-app.md`: its Keychain item
(`com.openagents.app.nostr` / `nsec-v1`) and the missing ECDH module match the
code. It says `coax/lib/nostr` has no NIP-28; that is right (only
`retired.coax` names kinds 40-44). Its note that the library has no group
module is only partly right: `nip29.coax` exists, but it covers the relay side
only (B4).

### B14. Decisions for the owner

| # | Decision | Recommendation |
| --- | --- | --- |
| 1 | Group design | NIP-17 rooms for private groups of up to 10; NIP-29 on relay.openagents.com for larger and public groups, labelled as not end to end encrypted; Marmot only if large encrypted groups are required. |
| 2 | Enable NIP-29 on relay.openagents.com | Yes: wire and advertise 29 (it was dropped from NIP-11 at the 2026-11-12 cutover until wired). |
| 3 | Public channels | Replace NIP-28 with open NIP-29 groups; show the three default Arcade channels read-only only if their history still exists. |
| 4 | Old Arcade data | Import keys (nsec/hex), kind 0, kind 3 and NIP-04 DMs. Skip Arcade's custom ciphers (blinded DMs, private channels, 30040, 10003) unless you know people who still hold that data. |
| 5 | Key import | Allow importing an nsec into the phone's Keychain slot, with a confirm that it replaces the current key; offer NIP-46 for keys kept elsewhere. |
| 6 | Listings | Keep the idea as NIP-99 listings priced in BTC/sats with offers over NIP-17; drop fiat payment methods. |
| 7 | Reputation | Drop ArcTrust for the first version. |
| 8 | Push notifications | Do not publish a Nostr event to a push service the way kind 30199 did; register devices through our account service. |
| 9 | Default relays | relay.openagents.com as the default 10002 and 10050 relay, with user additions. |

### B15. Compatibility: can the new app read old Arcade data?

| Data | Readable today? | What it takes |
| --- | --- | --- |
| Identity (nsec or hex) | Yes | Import path on iOS (gap 8). Same key, same npub. |
| Profile (kind 0) | Yes | Read `username` as a fallback for `name`; ignore `other`. |
| Follows (kind 3) | Yes | `follows.coax`. |
| NIP-04 DMs | Yes, read only | `nip04.coax`, from relays that still hold them. |
| Public channel history (40-42) | Yes, with a small reader | Gap 9; never write. |
| Blinded DMs (kind 4, `??1`) | No | Adapter: x = ECDH(me, them); pair key `P = x·G` to filter `authors`; HKDF-SHA256(x, salt = iv, info empty, 32); AES-256-CBC decrypt; PKCS#7 unpad; JSON `{pubkey, content}`. Treat `pubkey` as an unverified claim. |
| Private channels (99, 400, 402, 403) | No | Same adapter: unwrap kind 99 with ECDH(me, outer pubkey), verify the inner kind 400, take `privkey`; decrypt 402/403 with ECDH(channel privkey, outer pubkey). Senders are unsigned. Most channel keys were never recoverable off the creating device (A5), so expect little. |
| Private contacts (10003/20003), joined channels (30040), kind 0 `other` | No | `selfDecrypt`: content is `[epub, ??cipher]`; ECDH(me, epub). Then rewrite as NIP-51 (30000 private items, 10009). Kind 10003 may since have been overwritten by another client's bookmarks. |
| Push registration (30199) | Not needed | Drop. |
| `@arcade.chat` NIP-05 names | Depends on Arcade's server | Not ours; do not migrate. |

Where the data lives matters as much as the format. Arcade wrote to
`relay.arcade.city`, `arc1.arcadelabs.co` and `relay.damus.io`. I did not check
whether the first two still answer (no network access for this audit). Any
import is a one-time, read-only step; the new app never writes Arcade's kinds
or ciphers.

---

## Files read

Arcade (`/Users/christopherdavid/work/arcade`):

- `app/arclib/src/`: `index.ts`, `util.ts`, `ident.ts`, `pool.ts`, `channel.ts`,
  `nip28channel.ts`, `encchannel.ts`, `private.ts`, `contacts.ts`, `profile.ts`,
  `listing.ts`, `comment_to_trade.ts`, `social.ts`, `db/base.ts`, `db/index.ts`
- `app/arclib/test/`: `encchannel.test.ts`, `private.test.ts`, `ident.test.ts`
  (others listed, not needed beyond names); `README.md`, `package.json`, `git log`
- `app/components/RelayProvider.tsx`, `ChannelItem.tsx`, `ChannelManagerItem.tsx`,
  `ChannelMessageForm.tsx`, `ContactItem.tsx`, `ContactPicker.tsx`,
  `DirectMessageForm.tsx`, `DirectMessageItem.tsx`, `Reply.tsx`, `ReplyUser.tsx`,
  `User.tsx`, `UserFeed.tsx`, `UserOffer.tsx`, `MessageContent.tsx`,
  `LinkPreview.tsx`, `ListingItem.tsx`, `ListingOfferItem.tsx`, `OfferForm.tsx`,
  `Faerie/MessageInput.tsx`
- `app/models/`: `UserStore.ts`, `Channel.ts`, `ChannelStore.ts`, `Contact.ts`,
  `Message.ts`, `RootStore.ts`, `helpers/setupRootStore.ts`
- `app/hooks/`: all eight files
- `app/utils/`: `profile.tsx`, `nip02.tsx`, `parser.tsx`, `useUserContacts.tsx`,
  `shortenKey.tsx`, `isImage.tsx`, `notification.ts`, `storage/storage.ts`
- `app/screens/`: Login, CreateAccount, Backup, HomeMessages, Channels,
  ChannelManager, CreateChannel, Chat, DirectMessage, Invite, ListingDetail,
  Listing, User, RelayManager, PrivacySetting, NotificationSetting,
  EditProfile, AddContact, ChannelMembers, NewMessage, Contacts, Profile,
  Changelog (logic sections; Discover and Home have no Nostr use)
- `app/navigators/AppNavigator.tsx`, `app/app.tsx`, `app.json`, `package.json`,
  `git log`

Sibling docs: `docs/arcade/01-codebase.md` (`89fc8c8c27`),
`docs/arcade/04-target-app.md` (`26baeb6c18`); `apps/openagents-ios/project.yml`.

NIPs: `projects/repos/nips` at `1fb3a95` (status lines and 17, 29, C7 detail) and `nostr/nips/official/` 01, 04, 06, 08, 10, 15, 17, 26, 28, 29, 32, 51, 78,
99, C7, EE and `README.md` (statuses of 44, 46, 49, 57, 59, 65 checked);
`/Users/christopherdavid/code/nips` history; PR #580 (diff, commits, comments)
and earonesty's PR list via `gh`; marmot-protocol/marmot `README.md` via `gh`.

Our side: `coax/nostr.md`; in `coax/lib/nostr/`: `nip04`, `nip17`, `nip44`,
`nip44-key`, `nip44-envelope`, `nip59`, `nip29`, `nipc7`, `nip72`, `nip65`,
`outbox`, `relay`, `store`, `follows`, `metadata`, `keys`, `vault`, `vault-seal`,
`vault-wallet`, `nipkeys`, `nipkeys_carry`, `nipkeys_possess`, `nipkeys_proof`,
`nipkeys_sign`, `nip46`, `nip42`, `nip98_sign`, `nip05`, `nip06`, `nip19-core`,
`bech32`, `nip26`, `nip27`, `nip25`, `nip10`, `nip22`, `nip57`, `nip50`, `nip51`,
`nip51-private`, `nip99`, `nip15`, `mkt/order`, `retired`, `nip32`, `nip56`,
`nip18`, `nip38`, `event`, `filter`, `message` (headers, declarations and the
load-bearing lines); `coax/lib/crypto/aes.coax`, `aes_gcm.coax`,
`scrypt_nip49.coax`, `ed25519.coax` (headers) and the crypto directory listing;
`nostr/relay/relay_server.coax` (NIP-11), `relay_groups.coax`,
`group_read_policy.coax`, `relay_private.coax`, `store_delete.coax`;
`nostr/nips/README.md`, `manifest.json`, `upstream-lock.json`;
`apps/openagents-web/nostr-chat.coax` and every `nostr-lab-*.coax` (headers);
`apps/openagents-ios/README.md`, `OpenAgents/SignIn/NostrKey.swift`,
`PhoneSignIn.swift`, `OpenAgents/Platform/NostrSign.h`.

nipkeys is NIP-KEYS, OpenAgents' Bitcoin multisig key network (BIP-32, PSBT,
BIP-322), not Nostr identity keys; vault is NIP-VAULT, used today for the wallet
seed backup. Neither replaces NIP-49 for an nsec.

## Open questions

1. Do `relay.arcade.city` and `arc1.arcadelabs.co` still serve events? No
   network access was used beyond git and gh.
2. Did anyone outside the team use Arcade's private channels or blinded DMs,
   so that a legacy decrypt adapter is worth building?
3. nostr-tools 1.x is not installed in the clone. Its `skipVerification`
   behavior, `parseReferences` NIP-08 handling, EOSE timeout and per-sub dedup
   are stated from that library's 1.x API, not read.
4. Which exact revision of PR #580, if any, matched Arcade's 402/403 numbering?
   Every commit I read has 402 = metadata, 403 = message.
5. Is NIP-29 on relay.openagents.com deliberately off, or waiting on the
   "until each is wired" follow-up (#13382)?
6. Is there an approved media-upload path (NIP-92/94/Blossom) the port should
   use instead of nostrimg.com? I did not search for one.
7. Should importing an nsec replace the phone's existing key, or should the
   app hold more than one identity?
