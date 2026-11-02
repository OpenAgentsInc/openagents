# 99. The website and sign-in

This chapter follows one login to openagents.com from twelve words to a
session cookie. The page derives a Nostr key from the words, signs a NIP-98
request for `POST /api/auth/key`, and the API checks it with chapter 49's
Coax verifier and answers with a 30-day session. Chapter 49 teaches the
NIP-98 rules; this chapter teaches what the website adds around them.

Part XII, Nostr in OpenAgents. Result: Logging in to openagents.com with a
NIP-98 signed request from a words account (NIP-06) or a browser signer
(NIP-07), and the cutover of the C verifier to Coax.

## What this chapter teaches

- The words account: 16 bytes of entropy as 12 BIP-39 words, their NIP-06
  key, and a signed login. Each call derives the key it needs and zeroes it
  before it returns.
- The login route's rules on top of NIP-98: this site's `Origin`, the body
  exactly `{"kind":"login"}`, a `payload` tag, and first use of the event
  ID.
- Sessions: a server-made token whose SHA-256 is stored, 30 days, ended by
  sign-out.
- The browser signer path through NIP-07.
- The cutover: the C verifier was replaced by the Coax one after both agreed
  on every recorded request, and that comparison still runs.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [weblogin.coax](../../coax/lib/nostr/weblogin.coax) | `weblogin_check`, `weblogin_derive`, `weblogin_sign` | The page's words account: typed words to entropy, entropy to words and npub, and the signed login header |
| [private-face.c](../../apps/openagents-web/private-face.c) | `do_account` | The page's C face: 16 random bytes from the browser, then the `weblogin_*` calls; it wipes the entropy and aux bytes |
| [private.c](../../coax/web/api/private.c) | `auth_route` | The API's `/api/auth/<op>` routes; `key` is the login |
| [nip98.coax](../../coax/web/api/nip98.coax) | `nip98_check_coax` | The API's NIP-98 check (chapter 49) |
| [0067_signin_key.sql](../../coax/web/api/migrations/0067_signin_key.sql), [0019_phone_signin.sql](../../coax/web/api/migrations/0019_phone_signin.sql) | `signin.api`, `signin.session` | The session: insert, look up, sign out |
| [host.js](../../coax/web/host.js) | `coaxial_nip07_sign` | The browser signer bridge: passes an event template to `window.nostr.signEvent` |
| [nip01-verify.coax](../../apps/openagents-web/nip01-verify.coax) | `nv_run` | The `/nip/01` page's verifier: the shared event reader and BIP-340 on the page's captured event and its edited copies |
| [nip98_diff.c](../../coax/web/api/nip98_diff.c), [nip98_reference.c](../../coax/web/api/nip98_reference.c) | `main`, `nip98_reference_check` | The former C check, kept only to compare with the Coax one |

The design is [docs/web-log-in.md](../../docs/web-log-in.md). Its passkey
part is not built. What is built is its last section, "The words account in
use": the page makes the entropy itself and keeps it in browser storage.

The current registry records `private-face.c` as a direct caller of the Coax
words-account component and `coax/web/api/nostr.c` as the caller of the Coax
NIP-98 verifier. These are specific consumer edges. They do not qualify a
phone-signer profile, audit browser key custody or migrate the hosted runner
and encrypted presentation opener. Those exits remain in the
[completion route](COMPLETION.md#2-qualify-the-consumers-and-their-target-boundaries).

## The words account

A first visit to `/login` asks the browser for 16 random bytes. Those bytes
are the account. The page keeps them as 32 hex digits and never sends them.
Chapter 46 turns them into 12 words and the words into a key:

1. `weblogin_derive(hex, words, npub)` writes the 12 BIP-39 words and the
   NIP-06 key's `npub`, from the path `m/44'/1237'/0'/0/0` with no
   passphrase. It returns the words' length times 256 plus the npub's
   length, or `-1`.
2. `weblogin_check(typed, hex)` is SWAP ACCOUNT's step: it checks the typed
   words' checksum and writes their entropy as hex.
3. `weblogin_sign(hex, url, created_at, body, aux, out)` signs the login.

None of the three returns a key. Each one derives the key into a local
32-byte array and writes zeros over it before it returns. `wl_key` also
zeroes its copy of the entropy. The page holds only the hex entropy and the
words.

Until the browser side is audited, these keys are demo-grade. The swap box
says not to paste words that guard a key used elsewhere.

<!-- source: coax/lib/nostr/weblogin.coax sign -->
```jai
// "Nostr <base64 kind-27235 event>" for POST url with body, signed by the entropy's key; its length or -1.
weblogin_sign :: (hex: string, url: string, created_at: int, body: string, aux: [] u8, out: [] u8) -> int #export_c {
  if url.count < 10 || url.count > 200 || aux.count != 32 || created_at < 0 { return -1; }
  for c: url { if c == '"' || c == '\\' || c < 32 { return -1; } }
  words: [160] u8; key: [32] u8;
  if wl_key(hex, words[..], key[..]) <= 0 { return -1; }
  for i: 0..<160 { words[i] = 0; }
  // the tags: u, method, payload (SHA-256 of the body, hex)
  bb: [1024] u8; h: [32] u8;
  if body.count > 1024 { return -1; }
  for i: 0..<body.count { bb[i] = body[i]; }
  if sha256_hash_checked(bb[0..<body.count], h[..], false) != SHA_OK { return -1; }
  tags: [512] u8; hx: [64] u8;
  digits := "0123456789abcdef";
  for i: 0..<32 { hx[2 * i] = digits[cast(int) (h[i] >> 4)]; hx[2 * i + 1] = digits[cast(int) (h[i] & 15)]; }
  t := wl_put(tags[..], 0, "[[\"u\",\""); t = wl_put(tags[..], t, url);
  t = wl_put(tags[..], t, "\"],[\"method\",\"POST\"],[\"payload\",\""); t = wl_put(tags[..], t, cast(string) hx[..]);
  t = wl_put(tags[..], t, "\"]]");
  work: [1] NostrSignWork; scratch: [1024] u8; ev: [1024] u8;
  r := EVENT_MORE; steps := 0;
  while r == EVENT_MORE && steps < 64 {
    r = nostr_event_sign_step(key[..], aux, created_at, WL_LOGIN_KIND, cast(string) tags[0..<t], "", work[..], scratch[..], ev[..], false);
    steps += 1;
  }
  for i: 0..<32 { key[i] = 0; }
  if r <= 0 { return -1; }
  n := wl_put(out, 0, "Nostr ");
  e := base64_encode_checked(cast(string) ev[0..<r], out[n..<out.count], false);
  if e.error != NC_OK { return -1; }
  return n + e.written;
}
```

`weblogin_sign` refuses a URL shorter than 10 or longer than 200 bytes, a
URL with a quote, backslash or control byte (it writes the URL into the JSON
tags without escaping, so it refuses what would need escaping), a body over
1,024 bytes, aux bytes that are not 32 long, and a negative time. The page
passes `coaxial_now_seconds()` as the time and 32 random bytes as aux.

## The browser signer

A visitor with a NIP-07 extension can sign without the words account. The
page builds an event template, `{kind, created_at, tags, content}`, and the
host's `coaxial_nip07_sign` passes it to `window.nostr.signEvent`. The
signed event comes back to Coax as a message. JavaScript judges nothing:
Coax's signer reply checks the event, and the API checks the NIP-98 header
as for any other request. The project drop page uses this path today. The
words account does not need an extension.

## The login route

The API's `auth_route` takes `POST /api/auth/key`. It runs these steps in
order, and the first failure answers:

1. The `Origin` header is this site's, and an `Authorization: Nostr ...`
   header is present. Otherwise `403 {"error":"cross-site"}`. A login is
   both a page request and a signed request, so it needs both.
2. The address is under its rate limits (200 requests a minute in all, 10
   logins). Otherwise `429`.
3. The body is at most 4,096 bytes (`413`) and is exactly the 16 bytes
   `{"kind":"login"}`. Otherwise `400 {"error":"not a login"}`.
4. `nip98_check` passes for `POST` and the URL `https://openagents.com` +
   the request target, at the server's clock. The event has a `payload`
   tag, and `first_use` inserts its ID into `projects.seen`. Any failure
   answers `401 {"error":"not signed"}`. The reason is not sent.
5. The pubkey is under its own limit (30 logins a minute). Otherwise `429`.
6. `signin.api('key', body, who)` makes the session.

The `payload` tag matters here. NIP-98 makes it optional, and chapter 49's
check accepts an event without one. The login route requires it, so a login
event signs this exact body.

### The session

`signin.api('key')` makes a token with `projects.token()`, stores only its
SHA-256 in `signin.sessions` with the pubkey and an expiry 30 days ahead,
and returns the token. The route puts the token only in a cookie:

```text
Set-Cookie: oa_session=<token>; Path=/; Max-Age=2592000; HttpOnly; Secure; SameSite=Lax
```

- **Fixation.** The server makes the token. A cookie the visitor already
  had is not reused or upgraded, so an attacker can't plant a token and
  wait for it to be signed in.
- **Lookup.** `signin.session(token)` returns the pubkey of a row whose
  hash matches and whose `expires_ms` is later than now. A stolen database
  row does not give the cookie, because the row has only the hash.
- **Expiry.** After 30 days the lookup returns nothing.
- **Sign-out.** `POST /api/auth/logout` deletes the cookie's row and sends
  the cookie back with `Max-Age=0`.
- **Rotation.** Each login makes a new token. An earlier session of the
  same key stays valid until it expires or signs out; a login does not end
  it.

If the request carries the page's Verse player key (`X-Verse-Player`), the
same call moves that player to the new account, by migration 0022's rule.

## Worked example: one login by hand

The words are NIP-06's first published vector:

```text
leader monkey parrot ring guide accident before fence cannon height naive bean
```

Never sign with published words, or with words you use anywhere else,
outside an example.

### Step 1: words to entropy

Chapter 46 worked these 12 words: their 11-bit indexes give 132 bits, 128
of entropy and 4 of checksum. The entropy is
`7e91e681dd067802c50aa8216d564a89`, and the first byte of its SHA-256 is
`c0`, whose top 4 bits `1100` are the checksum: the low 4 bits of `bean`
(index 156, `00010011100`). The page keeps the 32 hex digits.

If the last word is `naive` (index 1173, `10010010101`), its low 4 bits are
`0101`, not `1100`, so `weblogin_check` returns false and nothing is signed.

### Step 2: the key

Chapter 46 derives the NIP-06 key `7f7ff03d...f1ccba9a`. Its x-only public
key is `17162c92...ca5cd917`, and its `npub` is
`npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu`, as NIP-06
publishes.

### Step 3: the tags

The URL `https://openagents.com/api/auth/key` is 35 bytes: `https://` (8),
`openagents.com` (14) and `/api/auth/key` (13). The body
`{"kind":"login"}` is 16 bytes, and its SHA-256 is `5371489d...f576014a`.

| Part | Bytes |
| --- | ---: |
| `[["u","` + URL + `"],` | 7 + 35 + 3 = 45 |
| `["method","POST"],` | 18 |
| `["payload","` + hash + `"]` | 12 + 64 + 2 = 78 |
| `]` | 1 |
| Total | 142 |

### Step 4: the event and the header

The NIP-01 preimage `[0,"<pubkey>",1790000000,27235,<tags>,""]` is
4 + 64 + 2 + 10 + 1 + 5 + 1 + 142 + 4 = 233 bytes. Its SHA-256 is the ID,
`a5414fb7...7946ce94`.

| Part | Bytes |
| --- | ---: |
| `{"id":"` + ID + `",` | 73 |
| `"pubkey":"` + pubkey + `",` | 76 |
| `"created_at":1790000000,` | 24 |
| `"kind":27235,` | 13 |
| `"tags":` + tags + `,` | 7 + 142 + 1 = 150 |
| `"content":"",` | 13 |
| `"sig":"` + signature + `"}` | 137 |
| Total | 486 |

`486 = 3·162`, so base64 writes 162 groups of 4 characters with no padding:
648 characters. With `Nostr ` (6), the header value is 654 bytes. The
pubkey starts at byte 83 of the event (`{"id":"` 7, the ID 64, `","pubkey":"`
12), and the payload hash at byte 268.

### Step 5: the server

The server's clock reads `now = 1790000030`.

1. `Origin: https://openagents.com` and `Authorization: Nostr ...`: pass.
2. The body is the 16 login bytes: pass.
3. NIP-98: the event verifies; kind 27235; `now - created_at = 30 ≤ 60`;
   `u` is the URL; `method` is `POST`; `payload` is the body's hash: pass.
4. The ID `a5414fb7...` is not in the seen set, so it is inserted.
5. The session's expiry is `1790000030 + 30·86400 = 1792592030`.

The answer is 200 with `{"state":"signed-in","npub":"npub1zutz..."}` and
the cookie.

### Step 6: the same header again

A copy of the header at the same `now` passes steps 1 to 3, but its ID is
in the seen set: 401. A copy at `now = 1790000061` fails earlier, at the
time check: `61 > 60`. So after the window, a reused login event refuses
whether or not the server still remembers its ID. That is why the seen set
can drop an ID once its window has passed.

## The example

The example is [nostr/examples/web-sign-in.coax](../examples/web-sign-in.coax).
It runs the page's `weblogin_*` calls and the API's `nip98_check_coax` on
the worked login with a fixed clock, and models the route's other steps and
the session table in memory. The token is fixed in the example; the server
makes a random one.

The client:

<!-- source: nostr/examples/web-sign-in.coax client -->
```jai
#load "../../coax/lib/nostr/weblogin.coax";
#load "../../coax/web/api/nip98.coax";
#import "Basic";

// NIP-06's first published vector. A published test mnemonic: never sign
// with words you use anywhere else.
MNEMONIC :: "leader monkey parrot ring guide accident before fence cannon height naive bean";
ORIGIN :: "https://openagents.com";
LOGIN_URL :: "https://openagents.com/api/auth/key";
LOGIN_BODY :: "{\"kind\":\"login\"}";
CREATED :: 1790000000;

// The page's steps: check the typed words and keep their entropy as hex,
// then derive the words and npub. Each weblogin call derives the key it
// needs and zeroes it before it returns.
Client :: struct { hex: [32] u8; words: [160] u8; nwords: int; npub: [80] u8; nnpub: int; ok: bool; }
client_words :: (typed: string) -> Client {
  c: Client;
  if !weblogin_check(typed, c.hex[..]) { return c; }
  both := weblogin_derive(cast(string) c.hex[..], c.words[..], c.npub[..]);
  if both <= 0 { return c; }
  c.nwords = both / 256; c.nnpub = both % 256; c.ok = true;
  return c;
}
// "Nostr <base64>" for POST url with body at created_at, signed with aux
// all zero (the page passes 32 random bytes), into out; "" when refused.
client_sign :: (c: Client, url: string, created_at: int, body: string, out: [] u8) -> string {
  aux: [32] u8;
  h := c.hex;
  n := weblogin_sign(cast(string) h[..], url, created_at, body, aux[..], out);
  if n <= 0 { return ""; }
  return cast(string) out[0..<n];
}
```

The route and the session, in `auth_route`'s order:

<!-- source: nostr/examples/web-sign-in.coax route -->
```jai
// The server's state: event IDs already used (the API's projects.seen) and
// site sessions (signin.sessions: the token's SHA-256, the pubkey, the expiry).
Session :: struct { token_sha: [32] u8; pubkey: [64] u8; expires: int; }
Server :: struct { seen: [8] [64] u8; nseen: int; sessions: [4] Session; nsessions: int; }
SESSION_SECONDS :: 30 * 86400;
Answer :: struct { status: int; why: string; }

// One POST /api/auth/key, in auth_route's order: the Origin and the
// Authorization scheme, the body, NIP-98 with a payload tag, first use of
// the event ID. On 200 it stores a session for token, the cookie's value.
login :: (s: [] Server, origin: string, authorization: string, body: string, now: int,
          token: [] u8) -> Answer {
  a: Answer;
  if origin != ORIGIN || authorization.count < 6 || authorization[0..<6] != "Nostr " {
    a.status = 403; a.why = "{\"error\":\"cross-site\"}"; return a;
  }
  if body != LOGIN_BODY { a.status = 400; a.why = "{\"error\":\"not a login\"}"; return a; }
  work: [N98_WORK] u8; out: [N98_OUT] u8; b: [64] u8;
  for i: 0..<body.count { b[i] = body[i]; }
  r := nip98_check_coax(authorization, "POST", LOGIN_URL, b[0..<body.count], now, work[..], out[..]);
  if r.error != N98_OK { a.status = 401; a.why = nip98_error_text(r.error); return a; }
  if r.payload == 0 { a.status = 401; a.why = "no payload tag"; return a; }
  for k: 0..<s[0].nseen {
    if cast(string) s[0].seen[k][..] == cast(string) out[0..<64] { a.status = 401; a.why = "event id already used"; return a; }
  }
  if s[0].nseen == 8 || s[0].nsessions == 4 { a.status = 503; a.why = "store full"; return a; }
  for i: 0..<64 { s[0].seen[s[0].nseen][i] = out[i]; }
  s[0].nseen += 1;
  // signin.api('key'): a new session bound to the event's pubkey, 30 days
  k := s[0].nsessions;
  #assert sha256_hash_checked(token, s[0].sessions[k].token_sha[..], false) == SHA_OK;
  for i: 0..<64 { s[0].sessions[k].pubkey[i] = out[64 + i]; }
  s[0].sessions[k].expires = now + SESSION_SECONDS;
  s[0].nsessions += 1;
  a.status = 200; a.why = "signed-in";
  return a;
}
// signin.session(token): the pubkey of an unexpired session, or "".
session :: (s: [] Server, token: [] u8, now: int) -> string {
  h: [32] u8;
  #assert sha256_hash_checked(token, h[..], false) == SHA_OK;
  for k: 0..<s[0].nsessions {
    if cast(string) s[0].sessions[k].token_sha[..] == cast(string) h[..] && s[0].sessions[k].expires > now {
      return cast(string) s[0].sessions[k].pubkey[..];
    }
  }
  return "";
}
// signin.api('logout'): the cookie's session ends.
logout :: (s: [] Server, token: [] u8) {
  h: [32] u8;
  #assert sha256_hash_checked(token, h[..], false) == SHA_OK;
  for k: 0..<s[0].nsessions {
    if cast(string) s[0].sessions[k].token_sha[..] == cast(string) h[..] { s[0].sessions[k].expires = 0; }
  }
}
```

The run: the login, a replay inside and after the window, a refusal for each
route step, two client refusals, and the session's life.

<!-- source: nostr/examples/web-sign-in.coax main -->
```jai
main :: () {
  print("the words\n");
  c := client_words(MNEMONIC);
  print("checksum: %, entropy: %\n", c.ok, cast(string) c.hex[..]);
  print("npub: %\n", cast(string) c.npub[0..<c.nnpub]);
  bad := client_words("leader monkey parrot ring guide accident before fence cannon height naive naive");
  print("last word naive: checksum %\n", bad.ok);

  print("the login\n");
  hb: [1024] u8;
  header := client_sign(c, LOGIN_URL, CREATED, LOGIN_BODY, hb[..]);
  ev: [1024] u8;
  d := base64_decode_checked(header[6..<header.count], ev[..], false);
  print("event (% bytes): %\n", d.written, cast(string) ev[0..<d.written]);
  print("header value: % bytes\n", header.count);

  print("the server, now = created_at + 30\n");
  s: [1] Server; now := CREATED + 30;
  t1: [32] u8; for i: 0..<32 { t1[i] = cast(u8) (i + 1); }   // the server's projects.token(), fixed here
  show("first use", s[..], ORIGIN, header, LOGIN_BODY, now, t1[..]);
  print("session: %...\n", session(s[..], t1[..], now)[0..<16]);
  show("same header again", s[..], ORIGIN, header, LOGIN_BODY, now, t1[..]);
  show("same header at created_at + 61", s[..], ORIGIN, header, LOGIN_BODY, CREATED + 61, t1[..]);

  print("refusals, on a fresh server\n");
  f: [1] Server;
  show("Origin https://evil.example", f[..], "https://evil.example", header, LOGIN_BODY, now, t1[..]);
  show("no Authorization", f[..], ORIGIN, "", LOGIN_BODY, now, t1[..]);
  show("body {\"kind\":\"logout\"}", f[..], ORIGIN, header, "{\"kind\":\"logout\"}", now, t1[..]);
  sb: [1024] u8;
  staging := client_sign(c, "https://staging.openagents.com/api/auth/key", CREATED, LOGIN_BODY, sb[..]);
  show("signed for staging", f[..], ORIGIN, staging, LOGIN_BODY, now, t1[..]);
  e2: [1024] u8; nb: [2048] u8;
  nopay := sign_tags(c, "[[\"u\",\"https://openagents.com/api/auth/key\"],[\"method\",\"POST\"]]", CREATED, e2[..], nb[..]);
  show("no payload tag", f[..], ORIGIN, nopay, LOGIN_BODY, now, t1[..]);
  print("the original, after those refusals: %\n", login(f[..], ORIGIN, header, LOGIN_BODY, now, t1[..]).status);
  print("client refusals\n");
  qb: [1024] u8;
  print("url with a quote: \"%\"\n", client_sign(c, "https://x\"y/api/auth/key", CREATED, LOGIN_BODY, qb[..]));
  print("entropy 7f7f: derive %\n", weblogin_derive("7f7f", c.words[..], c.npub[..]));

  print("the session\n");
  t2: [32] u8; for i: 0..<32 { t2[i] = cast(u8) (100 + i); }
  print("a cookie the server never issued: \"%\"\n", session(s[..], t2[..], now));
  print("29 days later: %...\n", session(s[..], t1[..], now + 29 * 86400)[0..<16]);
  print("30 days later: \"%\"\n", session(s[..], t1[..], now + SESSION_SECONDS));
  logout(s[..], t1[..]);
  print("after logout: \"%\"\n", session(s[..], t1[..], now + 60));
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/web-sign-in.coax --run-steps 250000000
```

It prints:

```text
the words
checksum: true, entropy: 7e91e681dd067802c50aa8216d564a89
npub: npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
last word naive: checksum false
the login
event (486 bytes): {"id":"a5414fb785055eb585a91d9bac252ca1dcaa35c16efa8a002bd4e1417946ce94","pubkey":"17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917","created_at":1790000000,"kind":27235,"tags":[["u","https://openagents.com/api/auth/key"],["method","POST"],["payload","5371489dd1001782407703fc88c301aca38be28649aa7f1f68bdb52cf576014a"]],"content":"","sig":"f89383c32edc1d88e5d8e501d8fe2105f8d33707e2c133a26713a8d211d5131cbdd526b39c5bbf7796dadd62e1af90fc40648852d78a3866340af45a65f3625c"}
header value: 654 bytes
the server, now = created_at + 30
first use: 200, signed-in
session: 17162c921dc4d251...
same header again: 401, {"error":"not signed"} (event id already used)
same header at created_at + 61: 401, {"error":"not signed"} (created_at is not within 60 s)
refusals, on a fresh server
Origin https://evil.example: 403, {"error":"cross-site"}
no Authorization: 403, {"error":"cross-site"}
body {"kind":"logout"}: 400, {"error":"not a login"}
signed for staging: 401, {"error":"not signed"} (the u tag is not this URL)
no payload tag: 401, {"error":"not signed"} (no payload tag)
the original, after those refusals: 200
client refusals
url with a quote: ""
entropy 7f7f: derive -1
the session
a cookie the server never issued: ""
29 days later: 17162c921dc4d251...
30 days later: ""
after logout: ""
```

Each refusal is one step of the route:

- `Origin https://evil.example` and `no Authorization` stop at step 1 with
  403.
- `{"kind":"logout"}` stops at step 3 with 400, before any signature work.
- `signed for staging` is a valid event for
  `https://staging.openagents.com/api/auth/key`. Its `u` tag is not this
  URL: 401. Staging and production keep separate accounts.
- `no payload tag` is a valid NIP-98 event without `payload`. Chapter 49's
  check accepts it; the login route refuses it.
- After those five refusals, the original header still logs in: a refused
  request does not use up an ID.
- The client refuses a URL with a quote (an empty header) and entropy that
  is not 32 hex digits (`-1`).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/web-sign-in.coax](../examples/checks/web-sign-in.coax),
which loads the example. The independent values:

- NIP-06's published pubkey and `npub` for the words.
- The whole login header from a Python script that shares no code with
  Coax: the BIP-39 seed with `hashlib.pbkdf2_hmac`, BIP-32 with `hmac` and
  integers, then [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  BIP-340 signer and NIP-01 serializer with zero aux, and `base64`. The
  Coax header equals it byte for byte.
- The body's SHA-256 from Python's `hashlib`.

They also check both edges of the window (±60 s pass, ±61 s refuse), that a
replay and the 403 and 400 refusals leave the stores as they were, the
session's last second, and that a wrong checksum signs nothing.

<!-- source: nostr/examples/checks/web-sign-in.coax check -->
```jai
// NIP-06's first vector, as published in nostr/nips/official/06.md.
NIP06_PUBKEY :: "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
NIP06_NPUB :: "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
// The same login made by a Python script that shares no code with Coax:
// BIP-39's seed with hashlib.pbkdf2_hmac, BIP-32 with hmac and integer
// arithmetic, then coax/tests/nostr/nip59_reference.py's BIP-340 signer and
// NIP-01 serializer, with aux all zero, and base64.b64encode.
PY_HEADER :: "Nostr eyJpZCI6ImE1NDE0ZmI3ODUwNTVlYjU4NWE5MWQ5YmFjMjUyY2ExZGNhYTM1YzE2ZWZhOGEwMDJiZDRlMTQxNzk0NmNlOTQiLCJwdWJrZXkiOiIxNzE2MmM5MjFkYzRkMjUxOGY5YTEwMWRiMzM2OTVkZjFhZmI1NmFiODJmNWZmM2U1ZGE2ZWVjM2NhNWNkOTE3IiwiY3JlYXRlZF9hdCI6MTc5MDAwMDAwMCwia2luZCI6MjcyMzUsInRhZ3MiOltbInUiLCJodHRwczovL29wZW5hZ2VudHMuY29tL2FwaS9hdXRoL2tleSJdLFsibWV0aG9kIiwiUE9TVCJdLFsicGF5bG9hZCIsIjUzNzE0ODlkZDEwMDE3ODI0MDc3MDNmYzg4YzMwMWFjYTM4YmUyODY0OWFhN2YxZjY4YmRiNTJjZjU3NjAxNGEiXV0sImNvbnRlbnQiOiIiLCJzaWciOiJmODkzODNjMzJlZGMxZDg4ZTVkOGU1MDFkOGZlMjEwNWY4ZDMzNzA3ZTJjMTMzYTI2NzEzYThkMjExZDUxMzFjYmRkNTI2YjM5YzViYmY3Nzk2ZGFkZDYyZTFhZjkwZmM0MDY0ODg1MmQ3OGEzODY2MzQwYWY0NWE2NWYzNjI1YyJ9";
// SHA-256 of {"kind":"login"}, from Python's hashlib.
PY_LOGIN_SHA256 :: "5371489dd1001782407703fc88c301aca38be28649aa7f1f68bdb52cf576014a";

once :: (header: string, now: int) -> int {
  s: [1] Server; t: [32] u8;
  return login(s[..], ORIGIN, header, LOGIN_BODY, now, t[..]).status;
}

#test "the words give NIP-06's published key, and the login matches Python's byte for byte" #steps 4000000000 {
  c := client_words(MNEMONIC);
  #assert c.ok && cast(string) c.npub[0..<c.nnpub] == NIP06_NPUB;
  #assert cast(string) c.words[0..<c.nwords] == MNEMONIC;
  hb: [1024] u8;
  header := client_sign(c, LOGIN_URL, CREATED, LOGIN_BODY, hb[..]);
  #assert header == PY_HEADER;
  ev: [1024] u8;
  d := base64_decode_checked(header[6..<header.count], ev[..], false);
  #assert d.error == NC_OK && d.written == 486 && header.count == 654;
  // the pubkey and payload hash sit at fixed places in the event
  #assert cast(string) ev[83..<147] == NIP06_PUBKEY;
  #assert cast(string) ev[268..<332] == PY_LOGIN_SHA256;
}
#test "the 60-second window, both edges, on a fresh server each time" #steps 4000000000 {
  #assert once(PY_HEADER, CREATED - 60) == 200 && once(PY_HEADER, CREATED + 60) == 200;
  #assert once(PY_HEADER, CREATED - 61) == 401 && once(PY_HEADER, CREATED + 61) == 401;
}
#test "one login, one session; a replay and every refusal leave the store as it was" #steps 4000000000 {
  s: [1] Server; t: [32] u8; u: [32] u8;
  t[0] = 1; u[0] = 2;
  #assert login(s[..], ORIGIN, PY_HEADER, LOGIN_BODY, CREATED, t[..]).status == 200;
  #assert login(s[..], ORIGIN, PY_HEADER, LOGIN_BODY, CREATED, u[..]).status == 401;
  #assert s[0].nseen == 1 && s[0].nsessions == 1;
  #assert session(s[..], t[..], CREATED) == NIP06_PUBKEY && session(s[..], u[..], CREATED) == "";
  #assert login(s[..], "http://openagents.com", PY_HEADER, LOGIN_BODY, CREATED, u[..]).status == 403;
  #assert login(s[..], ORIGIN, PY_HEADER, "{\"kind\": \"login\"}", CREATED, u[..]).status == 400;
  #assert s[0].nseen == 1 && s[0].nsessions == 1;
  #assert session(s[..], t[..], CREATED + SESSION_SECONDS - 1) == NIP06_PUBKEY;
  #assert session(s[..], t[..], CREATED + SESSION_SECONDS) == "";
}
#test "a wrong checksum signs nothing" #steps 4000000000 {
  c := client_words("leader monkey parrot ring guide accident before fence cannon height naive naive");
  hb: [1024] u8;
  #assert !c.ok && client_sign(c, LOGIN_URL, CREATED, LOGIN_BODY, hb[..]) == "";
}
```

```shell
coax/bin/coax test nostr/examples/checks/web-sign-in.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/web-sign-in.coax --root-only --run-steps 250000000
```

On 2026-11-01 this ran 382 checks, the loaded files' own included, in about
30 seconds; all passed. `weblogin.coax`'s own checks are part of that run.
`coax mutate --root-only` finds no mutation sites in the checks file: its
helpers make no decisions. The decisions are in `nip98.coax`, whose
mutation record is zero survivors (commit `4825cffd58`).

### The Coax and C verifiers

The API's NIP-98 check was C (`nip98_reference.c`, on libsecp256k1 and
OpenSSL) until 2026-10-31, when `nip98.coax` replaced it (commit
`d4cdfc2e8d`). `nostr.c` keeps the `nip98_check` signature and calls
`nip98_check_coax`. The C stays only for comparison.

[nip98_diff.c](../../coax/web/api/nip98_diff.c) runs both on every request
in the recorded corpus
[nip98_corpus.tsv](../../coax/web/api/nip98_corpus.tsv) (made by
`nip98_fixtures.ts`), and on seeded random edits of each one's event,
method, URL, time and body. Every decision, refusal word and returned field
must agree. The API's `test.sh` runs it with 400 edits a request. The
cutover commit recorded 16,043 cases with 0 disagreements, and 200,043 at
5,000 edits a request.

On 2026-11-01, built on Linux with gcc, AddressSanitizer and
UndefinedBehaviorSanitizer against Nix's libsecp256k1 0.7.1 and OpenSSL
3.6.3, it gave:

| Corpus | Cases | Accepted by both | Disagreements |
| --- | ---: | ---: | ---: |
| `nip98_corpus.tsv`, 83 requests, 400 edits each | 33,286 | 925 | 0 |
| The same plus this chapter's login at +30 s, +60 s, +61 s, for staging and as `GET` | 35,291 | 1,000 | 0 |

The 16 cases listed as "numbers past the end" in both runs are the one
documented difference: a number that runs to the very end of the event,
which the C read past and the Coax refuses as `bad object`. Both refuse.

### The API's own tests

[test_private_phone_key.py](../../coax/web/api/test_private_phone_key.py)
drives the real route against Postgres: a signed login gets the cookie
(HttpOnly, Secure, Lax, 30 days), and a replay, a wrong `u`, a wrong
payload, a stale event, no signature, another site's page and another body
are refused. It needs the API's `test.sh` (Postgres and the macOS paths it
names) and was not run for this chapter.

## Summary

- The words account is 16 bytes of entropy. The page keeps them as hex,
  shows them as 12 words, and derives the NIP-06 key inside each call and
  zeroes it before returning.
- A login is a NIP-98 `POST` to `/api/auth/key` with the body
  `{"kind":"login"}` and a `payload` tag; the worked header is 654 bytes.
- The route checks the origin, the body, NIP-98 and first use, in that
  order, and refuses with 403, 400 or 401.
- A session is a server-made token whose hash is stored for 30 days;
  sign-out deletes it.
- A login event reused after its 60-second window refuses at the time check.
- The Coax verifier and the former C verifier agree on every recorded
  request, the login included.

## Dependencies

- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md)
- [49. HTTP auth](ch049-http-auth.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-98](../nips/official/98.md)
- [NIP-06](../nips/official/06.md)
- [NIP-07](../nips/official/07.md)
- [docs/web-log-in.md](../../docs/web-log-in.md), the website's login
  design.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
