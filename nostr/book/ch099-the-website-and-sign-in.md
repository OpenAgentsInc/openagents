# 99. The website and sign-in

This chapter follows a Nostr key into openagents.com. A login is a NIP-98
signature over a challenge from the server, and it ends in a session
cookie. In the browser a NIP-07 extension makes that signature. The command
line has no browser, so `openagents login` makes a key of its own and asks a
logged-in person to join it to their account; after that the CLI signs every
request. Chapter 49 teaches the NIP-98 rules; this chapter teaches what the
website builds on them.

Part VIII, Nostr in OpenAgents. Result: Logging in to openagents.com with a
Nostr key: a NIP-98 signed challenge from a browser signer (NIP-07) or any
other signer becomes a session, a signed request is its key's account at L2,
and `openagents login` joins a computer's key to an account.

## What this chapter teaches

- One credential per request: a NIP-98 signature, a bearer token or the
  session cookie. Two at once are refused.
- A signature is a login in itself: its key's account at level L2, proved at
  that moment. No session is needed to use it.
- The key login: a challenge good for five minutes and one use, signed with
  the body, and the event ID accepted once.
- The browser signer: the page asks a NIP-07 extension for the event; the
  key never reaches the page.
- Sessions: a server-made token whose SHA-256 is stored, with a new token
  every 24 hours, an end after 30 unused days and an end 180 days after
  login.
- The CLI's device login: a new key starts a request, a person approves its
  code within ten minutes, and the key becomes one of the account's keys.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip98_sign.coax](../../coax/lib/nostr/nip98_sign.coax) | `n98s_header(key, aux, nonce, created_at, url, method, body, bind, out)` | The signer: the `Authorization` value for one request, for any URL, method and body (`#authority`) |
| [nip98.coax](../../coax/web/api/nip98.coax), [nostr.c](../../coax/web/api/nostr.c) | `nip98_check_coax`, `nip98_check` | The API's NIP-98 check (chapter 49) and the C shim the routes call |
| [auth.c](../../coax/web/api/auth.c) | `resolve`, `auth_gate` | Who is asking, from the request's one credential, and whether the route's rule admits them |
| | `nostr_challenge`, `nostr_login`, `auth_logged_in` | `POST /api/auth/nostr/challenge`, `POST /api/auth/nostr`, and the session and cookie every login ends in |
| [auth-policy.coax](../../coax/web/api/auth-policy.coax) | `ap_credentials`, `ap_admit`, `ap_limited_admit`, `ap_session` | The decisions `auth.c` asks for: one credential, a route's rule, an email session's limits, a session's clocks (`#authority`) |
| [0146_auth_login.sql](../../coax/web/api/migrations/0146_auth_login.sql) | `auth.nostr_challenge`, `auth.nostr_spend` | The challenge row, and spending a challenge and an event ID together |
| [login.coax](../../apps/openagents-web/login.coax), [key.js](../../apps/openagents-web/login/key.js) | `lg_key`, `lg_signed`; `sign` | The `/login` page's key login, and the page JavaScript that asks the NIP-07 extension to sign |
| [account.coax](../../apps/openagents-cli/account.coax) | `acct_call`, `cli_login`, `cli_logout` | The CLI: every request signed with `n98s_header`, and the device login |
| [account_rules.coax](../../apps/openagents-cli/account_rules.coax) | `ar_machine`, `ar_link`, `ar_poll`, `ar_logout` | The CLI's decisions: the machine name it sends, the link it prints, when it stops polling, what logout may forget (`#authority`) |
| [agents.c](../../coax/web/api/agents.c) | `device` | The API's `/api/auth/device/` routes |
| [agent-policy.coax](../../coax/web/api/agent-policy.coax) | `ag_link` | Whether a logged-in person may link a new key (`#authority`) |
| [0098_cli_callers.sql](../../coax/web/api/migrations/0098_cli_callers.sql), [0147_auth_agents.sql](../../coax/web/api/migrations/0147_auth_agents.sql) | `auth.device_start`, `auth.device_poll`, `auth.device_look`; `auth.device_link` | The device request's table and its rules |
| [private.coax](../../apps/openagents-web/private.coax) | `pv_device_due`, `pv_device_approve` | The `/login/device` page: the computer's name, and APPROVE |
| [nip01-verify.coax](../../apps/openagents-web/nip/nip01-verify.coax) | `nv_run` | The `/nip/01` page's verifier: the shared event reader and BIP-340 on the page's captured event and its edited copies |

The design is [docs/auth](../../docs/auth/README.md):
[identity-and-methods.md](../../docs/auth/identity-and-methods.md) for the
ways in and the levels, and
[sessions-and-tokens.md](../../docs/auth/sessions-and-tokens.md) for the
sessions. The CLI's plan is
[docs/cli](../../docs/cli/2026-11-01-openagents-cli-v1.md). The earlier
words account, where the page itself made 12 words and signed with their
key, was removed on 2026-11-11 (commit `ff92633a41`); this chapter no longer
describes it.

## Who is asking

Every request to the API carries at most one credential. `resolve` in
`auth.c` finds it, and `ap_credentials` refuses a request that carries two,
with 400:

| Credential | What it is | Level |
| --- | --- | --- |
| `Authorization: Nostr <base64 event>` | A NIP-98 signature over this request | L2, proved now |
| `Cookie: __Host-oa_session=oas_...` | A web session | The level of the login that made it |
| `Authorization: Bearer oa_at_...` | An app's access token, good for one hour | Its session's |
| `Authorization: Bearer oa_key_...` | An agent's API key | Its grant decides |

An account has two levels. L1 is an email code. L2 is a passkey, a Nostr
key or a recovery code. Routes that change who can log in ask for L2, and
some ask for L2 within the last five minutes, called L2-fresh.

For a signature, `resolve` builds the URL as this site's origin plus the
request's target, query included, and calls `nip98_check` with the
request's method, its body and the server's clock. If the check refuses,
the request is refused with 401; it is never read as "nobody". If it
passes, the signer is the event's pubkey at L2 with an L2 age of zero. If
`auth.nostr_keys` holds that pubkey, the request acts as that key's
account.

So a signature needs no session. It is a fresh strong login for exactly one
request. The CLI works this way and holds no token at all.

Then `auth_gate` looks up the route's rule in `auth.c`'s table and asks
`ap_admit`. The rules this chapter meets:

| Rule | Admits | Example route |
| --- | --- | --- |
| public | Anyone; the route reads its own credential | `POST /api/auth/nostr`, `POST /api/auth/device/start` |
| account | Any account | `GET /api/auth/ways` |
| human | A person's account, not an agent's | `POST /api/auth/device/approve` |
| L2-fresh | A person at L2 within five minutes | `POST /api/auth/unlink` |

After `ap_admit`, `ap_limited_admit` holds back one case: a session made by
an email code, on an account that also has a key, a passkey, a device key or
a recovery code. That session may read and play, and it may not change the
account. Whoever reads the inbox does not get what the key protects.

## The key login

Logging in with a key takes two requests.

1. `POST /api/auth/nostr/challenge` answers 64 random hex digits, the URL to
   sign and `expires_in` 300. The server keeps only the SHA-256 of the
   challenge, in `auth.pending`, for five minutes.
2. `POST /api/auth/nostr` carries the body `{"challenge":"<64 hex>"}` and
   an `Authorization: Nostr` header signed for that URL, method and body.

The second request runs these steps in order, and the first failure
answers:

| Step | Refusal |
| --- | --- |
| The `Origin` header, when there is one, is this site's. Apps and tools send none | 403 `Not from this site.` |
| The network address is under 30 key logins a minute | 429 |
| One credential | 400 `Log out first, or send only the signature.` |
| The NIP-98 check passes | 401 `That signature isn't good for this request.` |
| The credential is a signature | 401 `Sign the challenge with your key (NIP-98).` |
| The event has a `payload` tag | 401 `Sign the body too (a payload tag).` |
| The signer is not an agent's key | 403 |
| The body has a 64-character challenge | 400 `Ask for a challenge first.` |
| The event ID is new (`auth.nostr_spend`) | 401 `That signature was used already.` |
| The challenge is open: known, unused and under five minutes old | 401 `That challenge has ended. Ask for a new one.` |

NIP-98 makes the `payload` tag optional, and chapter 49's check accepts an
event without one. The login requires it, because the challenge is in the
body: without the tag the signature would not cover the challenge.

`auth.nostr_spend` records the event ID before it looks for the challenge.
So a signature over a dead challenge is spent too: sending it again answers
`That signature was used already.` The steps above it record nothing. A
request refused for its origin can be sent again from this site.

The challenge and the 60-second window do different work. The window and
the event ID stop a copied header from being used twice. The challenge
makes the signature an answer to this server's question, so an event signed
earlier for another purpose can't be a login.

A key that no account holds makes one: `auth.account_for_key` creates the
account on the key's first login, with no email. New accounts are limited
to 10 an hour from one network address.

### The session

`auth_logged_in` makes the session for every way of logging in. For a web
login it draws a token that starts `oas_`, stores its SHA-256 in
`auth.sessions` with the account and the level, and sends the token only in
a cookie:

```text
Set-Cookie: __Host-oa_session=<token>; Path=/; Max-Age=2592000; HttpOnly; Secure; SameSite=Lax
```

- **Fixation.** The server makes the token. A cookie the visitor already
  had is never upgraded into a login.
- **Lookup.** The cookie's SHA-256 finds the row. A copy of the database
  does not give the cookie, because the row has only the hash.
- **Clocks.** `ap_session` decides from three times. A web session ends when
  it has not been used for 30 days, and 180 days after it began however
  much it was used. After 24 hours of use it is due a new token: `auth.c`
  sets a new cookie, and the old token works for 60 seconds more so that
  other open tabs don't fail.
- **Level.** A key login's session is L2, and the row keeps when L2 was
  proved. Five minutes later the session no longer passes an L2-fresh rule,
  and the page asks for the key again (a step-up).
- **Logout.** `POST /api/auth/logout` ends the session's row and clears the
  cookie.

An app's login gets no cookie. The challenge request says `"client":"app"`,
and the login answers an access token for one hour and a refresh token.
Those are outside this chapter.

## The browser signer

The `/login` page never holds a key. Its LOG IN WITH A KEY button works
through a NIP-07 extension:

1. `login/key.js` tells the page whether `window.nostr.signEvent` exists.
   Without it the page says the browser has no key extension and sends
   nothing.
2. If a session is already here, the page logs out first, because the API
   refuses a cookie and a signature together.
3. The page asks for a challenge and reads `challenge` and `url` from the
   answer. It asks for its answers as lines, so the URL it will sign is
   `https://openagents.com/api/auth/nostr?as=lines`.
4. The page hands `key.js` the URL and the body. `key.js` computes the
   body's SHA-256 with the browser's `crypto.subtle` and asks the extension
   to sign `{kind: 27235, created_at: now, content: "", tags: [["u", url],
   ["method", "POST"], ["payload", hash]]}`.
5. The extension shows its own prompt. If the person agrees, the signed
   event comes back; `key.js` writes it as `Nostr <base64>` and gives it to
   the page, which sends the login.

JavaScript judges nothing here. The page decides what to ask, and the API
checks the event as it checks any other. An extension that signs something
else fails the NIP-98 check.

`host.js` has a second bridge, `coaxial_nip07_sign`, for the signer
requests of [keys.coax](../../coax/lib/nostr/keys.coax): it passes an event
template to the extension and returns the signed event for Coax to check.
The login page does not use it.

## The CLI: a computer joins an account

`openagents login` runs where there may be no browser, so the roles are
reversed: the computer shows a link, and a browser that is already logged
in approves it.

1. The CLI makes a key for the profile if it has none, and keeps it in a
   file only the user can read, under `~/.openagents/cli/keys`.
2. It signs `POST /api/auth/device/start` with the body
   `{"machine":"NAME"}`. `ar_machine` makes the name: printable ASCII, at
   most 64 bytes, anything else as `-`. The answer has `id` (16 hex digits),
   `code` (32 hex digits) and `expires_ms`, ten minutes ahead. The server
   keeps the code only as its SHA-256. A key may have five requests open.
3. The CLI prints the link `ar_link` builds,
   `https://openagents.com/login/device#code=<code>`, and its QR code. The
   code is in the fragment, so no server log sees it.
4. On that page a logged-in person sees the computer's name (`POST
   /api/auth/device/look`) and presses APPROVE (`POST
   /api/auth/device/approve`).
5. Every two seconds the CLI signs `GET /api/auth/device/<id>`. `ar_poll`
   reads the answer: go on while it says `waiting` and there is time, stop
   on `approved` or `expired`, and try again after a failed request, a 429
   or a 5xx.

Only the key that started a request may read it. A poll signed by any other
key answers 404, as if there were no such request.

The approval runs these steps:

| Step | Refusal |
| --- | --- |
| The gate's human rule: a person is logged in | 401 `Log in first.` |
| An email session on an account with a stronger way in is limited | 401 `You logged in with email, and this account has a key. ...` |
| The `Origin` header, when there is one, is this site's | 403 `Not from this site.` |
| `ag_link`: L2, or L1 when the account has no key or passkey | 401 `Log in again with your key or passkey to do this.` |
| The code is some request's | 404 `That code isn't good.` |
| The request is not approved yet | 409 `That computer was approved already, or its key is another account's.` |
| The request is under ten minutes old | 410 `That code expired: run openagents login again.` |
| The key is no other account's | 409, the same words |

On 200, `auth.device_link` adds the CLI's key to `auth.nostr_keys` for the
approving account, labelled `cli:MACHINE`. Nothing is sent to the computer.
Its next poll says `approved`, and from then on its signatures are that
account at L2.

An account that has only an email may approve at L1: it has nothing
stronger to protect. Once the CLI's key is linked, the account has a key,
and the same email session is limited.

The device routes check each signature and its 60-second window. They do
not record event IDs. A copied start header can open another request under
the same key within the window, and a copied poll header can read the same
state.

`openagents logout` signs `POST /api/auth/unlink` for its own key, which
is an L2-fresh route: the signature is the fresh login. Then it deletes the
key file. `ar_logout` refuses to forget a key that is its account's last
way in unless `--yes` is given.

`openagents login --words` and `--key-stdin` skip the device login. They
read 12 words (chapter 46) or a secret key from standard input, never from
the arguments, and keep that key.

## Worked example: one login by hand

The key is NIP-06's first published vector:

```text
leader monkey parrot ring guide accident before fence cannon height naive bean
```

Never sign with published words, or with words you use anywhere else,
outside an example. Chapter 46 derives its private key `7f7ff03d...f1ccba9a`
and its public key `17162c92...ca5cd917`.

### Step 1: the challenge and the body

The server's challenge is 64 hex digits. The example fixes it as 64 ones.
The body is `{"challenge":"` (14 bytes), the challenge (64) and `"}` (2):
80 bytes. Its SHA-256 is `c1ac31c1...3a22012b`.

### Step 2: the tags

The URL `https://openagents.com/api/auth/nostr` is 37 bytes: `https://` (8),
`openagents.com` (14) and `/api/auth/nostr` (15). The signer adds a `nonce`
tag of 16 bytes as hex. The example's nonce is 16 bytes of `01`.

| Part | Bytes |
| --- | ---: |
| `[["u","` + URL + `"],` | 7 + 37 + 3 = 47 |
| `["method","POST"],` | 18 |
| `["payload","` + hash + `"],` | 12 + 64 + 3 = 79 |
| `["nonce","` + nonce + `"]` | 10 + 32 + 2 = 44 |
| `]` | 1 |
| Total | 189 |

The nonce is there because an event's ID covers its tags and its time, not
the signature. Without it, two requests to one URL with one body in one
second would be the same event, and the second would be refused as used.

### Step 3: the event and the header

The event is signed at `created_at = 1790000030`.

| Part | Bytes |
| --- | ---: |
| `{"id":"` + ID + `",` | 73 |
| `"pubkey":"` + pubkey + `",` | 76 |
| `"created_at":1790000030,` | 24 |
| `"kind":27235,` | 13 |
| `"tags":` + tags + `,` | 7 + 189 + 1 = 197 |
| `"content":"",` | 13 |
| `"sig":"` + signature + `"}` | 137 |
| Total | 533 |

`533 = 3·177 + 2`, so base64 writes 177 groups of 4 characters and one more
group that ends in `=`: 712 characters. With `Nostr ` (6), the header value
is 718 bytes. The pubkey starts at byte 83 of the event (`{"id":"` 7, the
ID 64, `","pubkey":"` 12). The payload hash starts at byte 270: 186 bytes
up to the tags, `"tags":` (7), the first two tags (47 + 18) and
`["payload","` (12).

### Step 4: the server

The challenge was issued at second 1790000000 and the request arrives at
1790000030.

1. `Origin: https://openagents.com`: pass.
2. One credential, the signature: pass.
3. NIP-98: the event verifies; kind 27235; `now - created_at = 0 ≤ 60`; `u`
   is this URL; `method` is `POST`; `payload` is the body's hash: pass.
4. The event has a `payload` tag: pass.
5. The event ID `eca9e80e...` is new, so it is recorded.
6. The challenge is 30 seconds old, under 300, and unused: it is spent.
7. No account holds the key, so account 1 is made.

The answer is 200 with `"state":"logged-in"` and the cookie. The session is
L2 from this moment. An L2-fresh rule admits it for 300,000 ms and refuses
it at 300,001.

### Step 5: the same header again

One second later the same header fails at step 5: its ID is recorded. A new
signature over the same body passes step 5 with a new ID and fails at step
6, because the challenge is spent. A header signed 61 seconds before the
server's clock fails at step 3 and records nothing.

## The example

The example is [nostr/examples/web-sign-in.coax](../examples/web-sign-in.coax).
It loads the signer, the API's NIP-98 check, the API's two policy files and
the CLI's rules, and calls them as `auth.c`, `agents.c` and the CLI do. What
the API keeps in Postgres (challenges, used event IDs, keys, sessions and
device requests) is a struct in memory. The clock, the challenges, the
tokens, the device codes and the signer's random bytes are fixed, so every
run prints the same bytes; the server and the CLI draw all of them at
random. The rate limits, agents and app tokens are not modelled.

The client. `sign` makes a header the way the CLI's `acct_call` does:

<!-- source: nostr/examples/web-sign-in.coax client -->
```jai
#load "../../coax/lib/nostr/nip06.coax";
#load "../../coax/lib/nostr/nip98_sign.coax";
#load "../../coax/web/api/nip98.coax";
#load "../../coax/web/api/auth-policy.coax";
#load "../../coax/web/api/agent-policy.coax";
#load "../../apps/openagents-cli/account_rules.coax";
#import "Basic";

ORIGIN :: "https://openagents.com";
// NIP-06's two published vectors (nostr/nips/official/06.md). Published test
// words: never sign with them, or with words you use anywhere else, outside
// an example.
PERSON_WORDS :: "leader monkey parrot ring guide accident before fence cannon height naive bean";
CLI_WORDS :: "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade";
T0 :: 1790000000;   // the clock's start, in seconds

// Text as bytes, in buf.
bytes :: (text: string, buf: [] u8) -> [] u8 {
  for i: 0..<text.count { buf[i] = text[i]; }
  return buf[0..<text.count];
}
// The Authorization value for one request, made as the CLI's acct_call makes
// it: n98s_header, with the body bound unless the request is a GET. The CLI
// draws aux and the nonce at random; here aux is zero and the nonce is 16
// copies of one byte, so every run signs the same bytes. "" when the signer
// refuses.
sign :: (key: [] u8, nonce: u8, now_s: int, url: string, method: string, body: string, bind: bool, out: [] u8) -> string {
  aux: [32] u8; n16: [16] u8; bb: [256] u8;
  for i: 0..<16 { n16[i] = nonce; }
  n := n98s_header(key, aux[..], n16[..], now_s, url, method, bytes(body, bb[..]), bind, out);
  if n < 0 { return ""; }
  return cast(string) out[0..<n];
}
```

The server: who is asking, the gate, the challenge, the login and logout,
in `auth.c`'s order:

<!-- source: nostr/examples/web-sign-in.coax server -->
```jai
// The server's rows. A challenge and a session token are kept only as their
// SHA-256 (auth.pending, auth.sessions); an event ID is kept until its window
// has passed (auth.nip98_seen); a key belongs to one account (auth.nostr_keys).
Challenge :: struct { sha: [32] u8; expires_ms: int; done: bool; }
Key :: struct { pubkey: [64] u8; account: int; machine: [64] u8; nmachine: int; }   // nmachine > 0: labelled cli:MACHINE
Session :: struct { token_sha: [32] u8; account: int; level: int; created_ms: int; seen_ms: int; rotated_ms: int; l2_ms: int; ended: bool; }
Server :: struct {
  challenges: [4] Challenge; nchallenges: int;
  seen: [16] [64] u8; nseen: int;
  keys: [4] Key; nkeys: int; accounts: int;
  sessions: [4] Session; nsessions: int;
  devices: [4] Device; ndevices: int;
}
CHALLENGE_MS :: 5 * AP_MINUTE_MS;
// An answer: the status and the words the client gets. why stays on the server.
Answer :: struct { status: int; words: string; why: string; account: int; made: bool; }
no :: (status: int, words: string) -> Answer { a: Answer; a.status = status; a.words = words; return a; }

sha :: (text: string, out: [] u8) {
  b: [128] u8;
  #assert sha256_hash_checked(bytes(text, b[..]), out, false) == SHA_OK;
}
key_of :: (s: [] Server, pubkey: string) -> int {
  for k: 0..<s[0].nkeys { if cast(string) s[0].keys[k].pubkey[..] == pubkey { return k; } }
  return -1;
}
// How many keys an account has: its strong ways in. The API also counts
// passkeys, device keys and unspent recovery codes.
strong :: (s: [] Server, account: int) -> int {
  n := 0;
  for k: 0..<s[0].nkeys { if s[0].keys[k].account == account { n += 1; } }
  return n;
}

// Who is asking (auth.c's resolve): the request's one credential. A NIP-98
// signature is its key at L2, proved now, and the key's account when it has
// one. A session cookie is its session's account and level, unless
// ap_session says the session has ended.
Who :: struct { refused: int; why: string; kind: int; nostr: bool; payload: bool; id: [64] u8; pubkey: [64] u8;
                account: int; level: int; l2_age_ms: int = -1; session: int = -1; }
resolve :: (s: [] Server, method: string, target: string, authorization: string, cookie: [] u8, body: string, now_ms: int) -> Who {
  w: Who;
  nostr := authorization.count >= 6 && authorization[0..<6] == "Nostr ";
  two := ap_credentials(nostr, false, cookie.count > 0);
  if two != AP_OK { w.refused = ap_status(two); w.why = ap_words(two); return w; }
  if nostr {
    // the URL the event must name: this site's origin and the request's target
    url: [300] u8; u := 0;
    for c: ORIGIN { url[u] = c; u += 1; }
    for c: target { url[u] = c; u += 1; }
    work: [N98_WORK] u8; out: [N98_OUT] u8; bb: [256] u8;
    r := nip98_check_coax(authorization, method, cast(string) url[0..<u], bytes(body, bb[..]), now_ms / 1000, work[..], out[..]);
    if r.error != N98_OK { w.refused = 401; w.why = nip98_error_text(r.error); return w; }
    for i: 0..<64 { w.id[i] = out[i]; w.pubkey[i] = out[64 + i]; }
    w.nostr = true; w.payload = r.payload > 0; w.kind = AP_PERSON; w.level = 2; w.l2_age_ms = 0;
    k := key_of(s, cast(string) w.pubkey[..]);
    if k >= 0 { w.account = s[0].keys[k].account; }
    return w;
  }
  if cookie.count > 0 {
    h: [32] u8;
    #assert sha256_hash_checked(cookie, h[..], false) == SHA_OK;
    for k: 0..<s[0].nsessions {
      x := s[0].sessions[k];
      if x.ended || cast(string) x.token_sha[..] != cast(string) h[..] { continue; }
      if ap_session(false, now_ms, x.created_ms, x.seen_ms, x.rotated_ms) == AP_SESSION_ENDED { s[0].sessions[k].ended = true; continue; }
      w.kind = AP_PERSON; w.account = x.account; w.level = x.level; w.session = k;
      w.l2_age_ms = ifx x.l2_ms > 0 then now_ms - x.l2_ms else -1;
      s[0].sessions[k].seen_ms = now_ms;
    }
  }
  return w;
}
// The gate (auth.c's auth_gate) for a route's rule: ap_admit on who is
// asking, then ap_limited_admit for an email session on an account that has
// a key.
gate :: (s: [] Server, rule: int, write: bool, w: Who) -> Answer {
  v := AP_LOG_IN;
  if w.refused == 400 { v = AP_TWO_CREDENTIALS; }
  if w.refused == 0 {
    v = ap_admit(rule, w.kind, w.level, w.l2_age_ms, false, false);
    if v == AP_OK { v = ap_limited_admit(rule, write, ap_limited(w.level, strong(s, w.account))); }
  }
  if v == AP_OK { return no(200, "admitted"); }
  return no(ap_status(v), ap_words(v));
}

// POST /api/auth/nostr/challenge: 64 hex digits to sign over, good for five
// minutes and for one login. The server draws them at random.
challenge :: (s: [] Server, ch: string, now_ms: int) {
  k := s[0].nchallenges;
  sha(ch, s[0].challenges[k].sha[..]);
  s[0].challenges[k].expires_ms = now_ms + CHALLENGE_MS;
  s[0].nchallenges += 1;
}
// The challenge in the body {"challenge":"<64 hex>"}, or "".
challenge_of :: (body: string) -> string {
  pre := "{\"challenge\":\"";
  if body.count != pre.count + 66 || body[0..<pre.count] != pre || body[body.count - 2..<body.count] != "\"}" { return ""; }
  return body[pre.count..<pre.count + 64];
}
// A new session for an account (auth.session_new). token is the cookie's
// value; only its SHA-256 is kept.
session_new :: (s: [] Server, account: int, level: int, now_ms: int, token: [] u8) {
  k := s[0].nsessions;
  #assert sha256_hash_checked(token, s[0].sessions[k].token_sha[..], false) == SHA_OK;
  s[0].sessions[k].account = account; s[0].sessions[k].level = level;
  s[0].sessions[k].created_ms = now_ms; s[0].sessions[k].seen_ms = now_ms; s[0].sessions[k].rotated_ms = now_ms;
  s[0].sessions[k].l2_ms = ifx level == 2 then now_ms else 0;
  s[0].nsessions += 1;
}
// POST /api/auth/nostr, in auth.c's order (auth_api_route, then nostr_login).
// The first failure answers. On 200 the answer sets the session cookie to
// token. The rate limits are not modelled.
login :: (s: [] Server, origin: string, authorization: string, cookie: [] u8, body: string, now_ms: int, token: [] u8) -> Answer {
  // a page's request names its Origin; apps and tools send none
  if origin != "" && origin != ORIGIN { return no(403, "Not from this site."); }
  w := resolve(s, "POST", "/api/auth/nostr", authorization, cookie, body, now_ms);
  if w.refused == 400 { return no(400, "Log out first, or send only the signature."); }
  if w.refused != 0 { a := no(401, "That signature isn't good for this request."); a.why = w.why; return a; }
  if !w.nostr { return no(401, "Sign the challenge with your key (NIP-98)."); }
  if !w.payload { return no(401, "Sign the body too (a payload tag)."); }
  ch := challenge_of(body);
  if ch == "" { return no(400, "Ask for a challenge first."); }
  // auth.nostr_spend: the event ID once, then the challenge once
  for k: 0..<s[0].nseen {
    if cast(string) s[0].seen[k][..] == cast(string) w.id[..] { return no(401, "That signature was used already."); }
  }
  for i: 0..<64 { s[0].seen[s[0].nseen][i] = w.id[i]; }
  s[0].nseen += 1;
  h: [32] u8; open := -1;
  sha(ch, h[..]);
  for k: 0..<s[0].nchallenges {
    c := s[0].challenges[k];
    if cast(string) c.sha[..] == cast(string) h[..] && !c.done && c.expires_ms > now_ms { open = k; }
  }
  if open < 0 { return no(401, "That challenge has ended. Ask for a new one."); }
  s[0].challenges[open].done = true;
  // auth.account_for_key: the key's account, made on its first login
  a := no(200, "logged-in");
  if w.account == 0 {
    s[0].accounts += 1; w.account = s[0].accounts; a.made = true;
    for i: 0..<64 { s[0].keys[s[0].nkeys].pubkey[i] = w.pubkey[i]; }
    s[0].keys[s[0].nkeys].account = w.account;
    s[0].nkeys += 1;
  }
  session_new(s, w.account, 2, now_ms, token);
  a.account = w.account;
  return a;
}
// POST /api/auth/logout: this session ends.
logout :: (s: [] Server, cookie: [] u8, now_ms: int) {
  w := resolve(s, "POST", "/api/auth/logout", "", cookie, "", now_ms);
  if w.session >= 0 { s[0].sessions[w.session].ended = true; }
}
```

The device login's three routes:

<!-- source: nostr/examples/web-sign-in.coax device -->
```jai
// `openagents login` on the server (agents.c's device, and the SQL of
// migrations 0098 and 0147): a request the CLI's new key starts and polls,
// and a logged-in person approves by its code. The code is kept only as its
// SHA-256.
Device :: struct { id: [16] u8; code_sha: [32] u8; pubkey: [64] u8; machine: [64] u8; nmachine: int; expires_ms: int; account: int; }
DEVICE_MS :: 10 * AP_MINUTE_MS;

// The name in the body {"machine":"NAME"}, or "".
machine_of :: (body: string) -> string {
  pre := "{\"machine\":\"";
  if body.count < pre.count + 2 || body[0..<pre.count] != pre || body[body.count - 2..<body.count] != "\"}" { return ""; }
  return body[pre.count..<body.count - 2];
}
// POST /api/auth/device/start, signed by the CLI's key. The server draws id
// (16 hex) and code (32 hex) at random; the code is in this answer only.
device_start :: (s: [] Server, authorization: string, body: string, now_ms: int, id: string, code: string) -> Answer {
  w := resolve(s, "POST", "/api/auth/device/start?as=lines", authorization, bytes("", s[0].seen[0][0..<0]), body, now_ms);
  if w.refused != 0 || !w.nostr { return no(401, "Sign it with the new key (NIP-98)."); }
  m := machine_of(body);
  if m.count == 0 || m.count > 64 { return no(400, "name the machine"); }
  d := s[0].ndevices;
  for i: 0..<16 { s[0].devices[d].id[i] = id[i]; }
  sha(code, s[0].devices[d].code_sha[..]);
  for i: 0..<64 { s[0].devices[d].pubkey[i] = w.pubkey[i]; }
  for i: 0..<m.count { s[0].devices[d].machine[i] = m[i]; }
  s[0].devices[d].nmachine = m.count;
  s[0].devices[d].expires_ms = now_ms + DEVICE_MS;
  s[0].ndevices += 1;
  return no(200, "started");
}
// GET /api/auth/device/ID, signed by the key that started it: waiting,
// approved or expired. Another key's signature is answered as if there were
// no such request.
device_poll :: (s: [] Server, authorization: string, target: string, id: string, now_ms: int) -> Answer {
  w := resolve(s, "GET", target, authorization, bytes("", s[0].seen[0][0..<0]), "", now_ms);
  if w.refused != 0 || !w.nostr { return no(401, "Sign it with the new key (NIP-98)."); }
  for d: 0..<s[0].ndevices {
    x := s[0].devices[d];
    if cast(string) x.id[..] != id || cast(string) x.pubkey[..] != cast(string) w.pubkey[..] { continue; }
    if x.account != 0 { a := no(200, "approved"); a.account = x.account; return a; }
    if x.expires_ms <= now_ms { return no(200, "expired"); }
    return no(200, "waiting");
  }
  return no(404, "no such request");
}
// POST /api/auth/device/approve {code}, from a logged-in person's page: the
// gate's human rule, this site's Origin, ag_link, then auth.device_link. On
// 200 the CLI's key is the session's account's, labelled cli:MACHINE.
device_approve :: (s: [] Server, origin: string, cookie: [] u8, code: string, now_ms: int) -> Answer {
  w := resolve(s, "POST", "/api/auth/device/approve", "", cookie, "", now_ms);
  g := gate(s, AP_HUMAN, true, w);
  if g.status != 200 { return g; }
  if origin != "" && origin != ORIGIN { return no(403, "Not from this site."); }
  v := ag_link(w.level, strong(s, w.account) > 0);
  if v != AG_OK { return no(ag_status(v), ag_words(v)); }
  h: [32] u8; d := -1;
  sha(code, h[..]);
  for k: 0..<s[0].ndevices { if cast(string) s[0].devices[k].code_sha[..] == cast(string) h[..] { d = k; } }
  if d < 0 { return no(404, "That code isn't good."); }
  x := s[0].devices[d];
  held := key_of(s, cast(string) x.pubkey[..]);
  taken := "That computer was approved already, or its key is another account's.";
  if x.account != 0 { return no(409, taken); }
  if x.expires_ms <= now_ms { return no(410, "That code expired: run openagents login again."); }
  if held >= 0 && s[0].keys[held].account != w.account { return no(409, taken); }
  s[0].devices[d].account = w.account;
  if held < 0 {
    k := s[0].nkeys;
    s[0].keys[k].pubkey = x.pubkey; s[0].keys[k].account = w.account;
    s[0].keys[k].machine = x.machine; s[0].keys[k].nmachine = x.nmachine;
    s[0].nkeys += 1;
  }
  a := no(200, "approved"); a.account = w.account;
  return a;
}
```

The run: one login, each refusal of the login route, the session's life,
the device login with each refusal of its approval, and the CLI's signed
requests afterwards.

<!-- source: nostr/examples/web-sign-in.coax main -->
```jai
main :: () {
  ms := T0 * 1000;
  s: [1] Server; none: [1] u8; nobody := none[0..<0];
  t1: [32] u8; t2: [32] u8; t3: [32] u8; t4: [32] u8;   // the server's session tokens, fixed here
  for i: 0..<32 { t1[i] = cast(u8) (i + 1); t2[i] = cast(u8) (i + 51); t3[i] = cast(u8) (i + 101); t4[i] = cast(u8) (i + 151); }

  print("the key\n");
  key: [32] u8; np: [80] u8;
  #assert nip06_key(PERSON_WORDS, key[..]);
  print("npub: %\n", cast(string) np[0..<nip06_npub(key[..], np[..])]);

  print("a login: POST /api/auth/nostr\n");
  challenge(s[..], CH1, ms);
  b1: [100] u8; body1 := login_body(CH1, b1[..]);
  print("body: %\n", body1);
  hb: [N98S_HEADER] u8; ev: [N98S_EVENT] u8;
  header := sign(key[..], 1, T0 + 30, LOGIN_URL, "POST", body1, true, hb[..]);
  d := base64_decode_checked(header[6..<header.count], ev[..], false);
  print("event (% bytes): %\n", d.written, cast(string) ev[0..<d.written]);
  print("header value: % bytes\n", header.count);
  show("first use", login(s[..], ORIGIN, header, nobody, body1, ms + 30000, t1[..]));
  show("same header again", login(s[..], ORIGIN, header, nobody, body1, ms + 31000, t2[..]));
  h2: [N98S_HEADER] u8;
  show("a new signature over the spent challenge", login(s[..], ORIGIN, sign(key[..], 2, T0 + 32, LOGIN_URL, "POST", body1, true, h2[..]), nobody, body1, ms + 32000, t2[..]));

  print("refusals, with a second challenge open\n");
  challenge(s[..], CH2, ms + 40000);
  b2: [100] u8; body2 := login_body(CH2, b2[..]); now := ms + 41000;
  h3: [N98S_HEADER] u8; h4: [N98S_HEADER] u8; h5: [N98S_HEADER] u8; h6: [N98S_HEADER] u8; h7: [N98S_HEADER] u8;
  good := sign(key[..], 3, T0 + 41, LOGIN_URL, "POST", body2, true, h3[..]);
  show("Origin https://evil.example", login(s[..], "https://evil.example", good, nobody, body2, now, t2[..]));
  show("no Authorization", login(s[..], ORIGIN, "", nobody, body2, now, t2[..]));
  show("a session cookie and a signature", login(s[..], ORIGIN, good, t1[..], body2, now, t2[..]));
  show("signed 61 s ago", login(s[..], ORIGIN, sign(key[..], 4, T0 - 20, LOGIN_URL, "POST", body2, true, h4[..]), nobody, body2, now, t2[..]));
  show("signed for staging", login(s[..], ORIGIN, sign(key[..], 5, T0 + 41, "https://staging.openagents.com/api/auth/nostr", "POST", body2, true, h5[..]), nobody, body2, now, t2[..]));
  show("signed for another body", login(s[..], ORIGIN, sign(key[..], 6, T0 + 41, LOGIN_URL, "POST", body1, true, h6[..]), nobody, body2, now, t2[..]));
  show("body not signed", login(s[..], ORIGIN, sign(key[..], 7, T0 + 41, LOGIN_URL, "POST", body2, false, h7[..]), nobody, body2, now, t2[..]));
  show("no challenge in the body", login(s[..], ORIGIN, sign(key[..], 8, T0 + 41, LOGIN_URL, "POST", "{}", true, h4[..]), nobody, "{}", now, t2[..]));
  show("the first of these, from this site", login(s[..], ORIGIN, good, nobody, body2, now, t2[..]));
  challenge(s[..], CH3, ms + 50000);
  b3: [100] u8; body3 := login_body(CH3, b3[..]);
  show("a challenge 5 minutes old", login(s[..], ORIGIN, sign(key[..], 9, T0 + 350, LOGIN_URL, "POST", body3, true, h5[..]), nobody, body3, ms + 350000, t3[..]));
  print("signer refusals\n");
  zero: [32] u8;
  print("a URL with a quote: \"%\"\n", sign(key[..], 1, T0, "https://x\"y/api/auth/nostr", "POST", body1, true, h6[..]));
  print("a key of 32 zero bytes: \"%\"\n", sign(zero[..], 1, T0, LOGIN_URL, "POST", body1, true, h6[..]));

  print("the sessions\n");
  login_ms := ms + 30000; hour := 60 * AP_MINUTE_MS;
  print("first cookie, 1 hour after login: %\n", session_state(s[..], t1[..], login_ms + hour));
  print("24 hours after: %\n", session_state(s[..], t1[..], login_ms + AP_ROTATE_MS));
  print("30 days after, unused: %\n", session_state(s[..], t1[..], login_ms + AP_WEB_IDLE_MS));
  logout(s[..], t1[..], login_ms + hour);
  print("after logout: %\n", session_state(s[..], t1[..], login_ms + hour));
  print("a cookie the server never issued: %\n", session_state(s[..], t4[..], login_ms + hour));
  // the second login's session: L2 since ms + 41000
  unlink := gate(s[..], AP_L2_FRESH, true, resolve(s[..], "POST", "/api/auth/unlink", "", t2[..], "", now + AP_FRESH_MS));
  show("second cookie, POST /api/auth/unlink 5 minutes after login", unlink);
  unlink = gate(s[..], AP_L2_FRESH, true, resolve(s[..], "POST", "/api/auth/unlink", "", t2[..], "", now + AP_FRESH_MS + 1));
  show("1 ms later", unlink);

  print("openagents login\n");
  cli: [32] u8;
  #assert nip06_key(CLI_WORDS, cli[..]);
  mb: [AR_MACHINE_MAX] u8; sb: [100] u8;
  machine := ar_machine("  dev box\t1 ", mb[..]);
  n := 0;
  for c: "{\"machine\":" { sb[n] = c; n += 1; }
  n += ar_json_str(machine, sb[n..<sb.count]);
  sb[n] = '}'; n += 1;
  start := cast(string) sb[0..<n];
  print("body: %\n", start);
  at := ms + 1000000; at_s := T0 + 1000;
  show("start, not signed", device_start(s[..], "", start, at, DEV_ID, DEV_CODE));
  show("start", device_start(s[..], sign(cli[..], 10, at_s, "https://openagents.com/api/auth/device/start?as=lines", "POST", start, true, h3[..]), start, at, DEV_ID, DEV_CODE));
  lb: [300] u8;
  print("link: %\n", ar_link(ORIGIN, DEV_CODE, lb[..]));
  target := "/api/auth/device/00a1b2c3d4e5f607?as=lines";
  poll_url := "https://openagents.com/api/auth/device/00a1b2c3d4e5f607?as=lines";
  deadline := at + DEVICE_MS;
  p := device_poll(s[..], sign(cli[..], 11, at_s + 2, poll_url, "GET", "", false, h3[..]), target, DEV_ID, at + 2000);
  print("poll: %, %; the CLI: %\n", p.status, p.words, poll_word(ar_poll(p.status, p.words, at + 2000, deadline)));
  p = device_poll(s[..], sign(key[..], 12, at_s + 2, poll_url, "GET", "", false, h3[..]), target, DEV_ID, at + 2000);
  print("poll signed by another key: %, %; the CLI: %\n", p.status, p.words, poll_word(ar_poll(p.status, p.words, at + 2000, deadline)));
  session_new(s[..], 1, 1, at, t3[..]);   // an email login to account 1: a session at L1
  show("approve, not logged in", device_approve(s[..], ORIGIN, nobody, DEV_CODE, at + 3000));
  show("approve, logged in by email", device_approve(s[..], ORIGIN, t3[..], DEV_CODE, at + 3000));
  show("approve from https://evil.example", device_approve(s[..], "https://evil.example", t2[..], DEV_CODE, at + 3000));
  show("approve another code", device_approve(s[..], ORIGIN, t2[..], DEV_CODE2, at + 3000));
  show("approve", device_approve(s[..], ORIGIN, t2[..], DEV_CODE, at + 3000));
  show("approve again", device_approve(s[..], ORIGIN, t2[..], DEV_CODE, at + 4000));
  p = device_poll(s[..], sign(cli[..], 13, at_s + 4, poll_url, "GET", "", false, h3[..]), target, DEV_ID, at + 4000);
  print("poll: %, %, account %; the CLI: %\n", p.status, p.words, p.account, poll_word(ar_poll(p.status, p.words, at + 4000, deadline)));
  k := s[0].keys[1];
  print("account 1 has % keys; the new one is cli:%\n", strong(s[..], 1), cast(string) k.machine[0..<k.nmachine]);
  // a second request that nobody approves
  show("a second start", device_start(s[..], sign(cli[..], 14, at_s + 10, "https://openagents.com/api/auth/device/start?as=lines", "POST", start, true, h3[..]), start, at + 10000, DEV_ID2, DEV_CODE2));
  late := at + 10000 + DEVICE_MS;
  show("approve 10 minutes later", device_approve(s[..], ORIGIN, t2[..], DEV_CODE2, late));
  p = device_poll(s[..], sign(cli[..], 15, at_s + 610, "https://openagents.com/api/auth/device/10a1b2c3d4e5f607?as=lines", "GET", "", false, h3[..]), "/api/auth/device/10a1b2c3d4e5f607?as=lines", DEV_ID2, late);
  print("poll: %, %; the CLI: %\n", p.status, p.words, poll_word(ar_poll(p.status, p.words, late, late)));

  print("the CLI's requests after login\n");
  w := resolve(s[..], "GET", "/api/auth/ways?as=lines", sign(cli[..], 16, at_s + 700, "https://openagents.com/api/auth/ways?as=lines", "GET", "", false, h3[..]), nobody, "", at + 700000);
  print("GET /api/auth/ways: account %, L%, proved % ms ago\n", w.account, w.level, w.l2_age_ms);
  show("the account rule", gate(s[..], AP_ACCOUNT, false, w));
  out := "{\"kind\":\"nostr\",\"id\":2}";
  w = resolve(s[..], "POST", "/api/auth/unlink", sign(cli[..], 17, at_s + 700, "https://openagents.com/api/auth/unlink", "POST", out, true, h3[..]), nobody, out, at + 700000);
  show("POST /api/auth/unlink, the L2-fresh rule", gate(s[..], AP_L2_FRESH, true, w));
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/web-sign-in.coax --run-steps 250000000
```

It prints:

```text
the key
npub: npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
a login: POST /api/auth/nostr
body: {"challenge":"1111111111111111111111111111111111111111111111111111111111111111"}
event (533 bytes): {"id":"eca9e80edb0aa9887464fdf3e1d61c965358dc36e39fdec61b4bfbd090c11afe","pubkey":"17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917","created_at":1790000030,"kind":27235,"tags":[["u","https://openagents.com/api/auth/nostr"],["method","POST"],["payload","c1ac31c1d8ce5836fc782d33cd051b7a45e75871b64be20753364cd13a22012b"],["nonce","01010101010101010101010101010101"]],"content":"","sig":"9bcb21b1eb985eedaf012fa8e48461ee60beddf7f4a3f515522d087704dab82d9e4267107fd5d2a8fc1794b3e5ad1d3dc932cbc9ddec6588f5bdc9d9cf566d8e"}
header value: 718 bytes
first use: 200, logged-in, account 1, new
same header again: 401, That signature was used already.
a new signature over the spent challenge: 401, That challenge has ended. Ask for a new one.
refusals, with a second challenge open
Origin https://evil.example: 403, Not from this site.
no Authorization: 401, Sign the challenge with your key (NIP-98).
a session cookie and a signature: 400, Log out first, or send only the signature.
signed 61 s ago: 401, That signature isn't good for this request. [created_at is not within 60 s]
signed for staging: 401, That signature isn't good for this request. [the u tag is not this URL]
signed for another body: 401, That signature isn't good for this request. [the payload tag is not this body]
body not signed: 401, Sign the body too (a payload tag).
no challenge in the body: 400, Ask for a challenge first.
the first of these, from this site: 200, logged-in, account 1
a challenge 5 minutes old: 401, That challenge has ended. Ask for a new one.
signer refusals
a URL with a quote: ""
a key of 32 zero bytes: ""
the sessions
first cookie, 1 hour after login: good
24 hours after: good, and due a new token
30 days after, unused: ended
after logout: ended
a cookie the server never issued: no such session
second cookie, POST /api/auth/unlink 5 minutes after login: 200, admitted
1 ms later: 401, Log in again with your key or passkey to do this.
openagents login
body: {"machine":"dev box-1"}
start, not signed: 401, Sign it with the new key (NIP-98).
start: 200, started
link: https://openagents.com/login/device#code=00112233445566778899aabbccddeeff
poll: 200, waiting; the CLI: keep waiting
poll signed by another key: 404, no such request; the CLI: refused: stop
approve, not logged in: 401, Log in first.
approve, logged in by email: 401, You logged in with email, and this account has a key. Log in with your key to change it, or ask for recovery.
approve from https://evil.example: 403, Not from this site.
approve another code: 404, That code isn't good.
approve: 200, approved, account 1
approve again: 409, That computer was approved already, or its key is another account's.
poll: 200, approved, account 1; the CLI: approved: stop
account 1 has 2 keys; the new one is cli:dev box-1
a second start: 200, started
approve 10 minutes later: 410, That code expired: run openagents login again.
poll: 200, expired; the CLI: expired: stop
the CLI's requests after login
GET /api/auth/ways: account 1, L2, proved 0 ms ago
the account rule: 200, admitted
POST /api/auth/unlink, the L2-fresh rule: 200, admitted
```

Words in square brackets are the NIP-98 check's reason. The server has
them; the answer does not.

The login's refusals, in the order printed:

- `Origin https://evil.example` stops at the first step with 403. The same
  header is sent again from this site eight lines later and logs in: a
  request refused before the signature is spent uses up nothing.
- `no Authorization` has no credential: 401.
- `a session cookie and a signature` is two credentials: 400. This is why
  the `/login` page logs out before a key login.
- `signed 61 s ago`, `signed for staging` and `signed for another body` are
  valid events that fail one NIP-98 rule each. A signature for
  `https://staging.openagents.com` is not a signature for production.
- `body not signed` is a valid NIP-98 event without a `payload` tag.
  Chapter 49's check accepts it; the login refuses it.
- `no challenge in the body` signs the body `{}` correctly, and there is
  nothing to spend: 400.
- `a challenge 5 minutes old` is signed at the moment it is sent, 300
  seconds after its challenge was issued. The signature is good and the
  challenge has ended.
- The signer itself refuses a URL that would need JSON escaping and a key
  of 32 zero bytes. It writes no header.

The device login's refusals:

- `start, not signed`: only a key's signature starts a request.
- `poll signed by another key` is the person's own key asking for the CLI's
  request: 404, and `ar_poll` tells the CLI to stop.
- `approve, logged in by email` is an L1 session on account 1, which has a
  key since its first login. `ap_limited_admit` refuses it before the code
  is looked at.
- `approve another code` uses a code no request has: 404.
- `approve again` is the same code a second time: 409.
- `approve 10 minutes later` is the second request at exactly its expiry:
  410. The CLI's poll then says `expired` and stops.

The last three lines are the CLI after login. Its signature on `GET
/api/auth/ways` is account 1 at L2, proved 0 ms ago, with no session. The
same key's signature passes the L2-fresh rule of `POST /api/auth/unlink`,
which the second cookie stopped passing 5 minutes and 1 ms after its
login.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/web-sign-in.coax](../examples/checks/web-sign-in.coax),
which loads the example. The independent values:

- NIP-06's published private keys, public keys and `npub` for both sets of
  words.
- The whole login header from Python, which shares no code with Coax: the
  payload with `hashlib.sha256`, then
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s NIP-01
  serializer and BIP-340 signer on NIP-06's published key with zero aux,
  and `base64`. The Coax header equals it byte for byte.

They also check both edges of the 60-second window, the challenge's last
millisecond, that a refused challenge spends the event ID while the earlier
refusals spend nothing, a login with no `Origin`, the session's three
clocks at their edges, L2-fresh at 300,000 and 300,001 ms, and the device
login: an empty machine name, the code's last millisecond, an email-only
account approving at L1 and then being limited, and a key that is already
one account's refused for another.

<!-- source: nostr/examples/checks/web-sign-in.coax check -->
```jai
// NIP-06's two vectors, as published in nostr/nips/official/06.md.
NIP06_KEY :: "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
NIP06_PUBKEY :: "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
NIP06_NPUB :: "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
NIP06_KEY2 :: "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add";
NIP06_PUBKEY2 :: "d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573";
// The example's first login made by Python, which shares no code with Coax:
// the payload with hashlib.sha256, then coax/tests/nostr/nip59_reference.py's
// NIP-01 serializer and BIP-340 signer on NIP-06's published key with aux
// all zero, and base64.b64encode.
PY_PAYLOAD :: "c1ac31c1d8ce5836fc782d33cd051b7a45e75871b64be20753364cd13a22012b";
PY_HEADER :: "Nostr eyJpZCI6ImVjYTllODBlZGIwYWE5ODg3NDY0ZmRmM2UxZDYxYzk2NTM1OGRjMzZlMzlmZGVjNjFiNGJmYmQwOTBjMTFhZmUiLCJwdWJrZXkiOiIxNzE2MmM5MjFkYzRkMjUxOGY5YTEwMWRiMzM2OTVkZjFhZmI1NmFiODJmNWZmM2U1ZGE2ZWVjM2NhNWNkOTE3IiwiY3JlYXRlZF9hdCI6MTc5MDAwMDAzMCwia2luZCI6MjcyMzUsInRhZ3MiOltbInUiLCJodHRwczovL29wZW5hZ2VudHMuY29tL2FwaS9hdXRoL25vc3RyIl0sWyJtZXRob2QiLCJQT1NUIl0sWyJwYXlsb2FkIiwiYzFhYzMxYzFkOGNlNTgzNmZjNzgyZDMzY2QwNTFiN2E0NWU3NTg3MWI2NGJlMjA3NTMzNjRjZDEzYTIyMDEyYiJdLFsibm9uY2UiLCIwMTAxMDEwMTAxMDEwMTAxMDEwMTAxMDEwMTAxMDEwMSJdXSwiY29udGVudCI6IiIsInNpZyI6IjliY2IyMWIxZWI5ODVlZWRhZjAxMmZhOGU0ODQ2MWVlNjBiZWRkZjdmNGEzZjUxNTUyMmQwODc3MDRkYWI4MmQ5ZTQyNjcxMDdmZDVkMmE4ZmMxNzk0YjNlNWFkMWQzZGM5MzJjYmM5ZGRlYzY1ODhmNWJkYzlkOWNmNTY2ZDhlIn0=";
SIGNED_MS :: (T0 + 30) * 1000;   // PY_HEADER's created_at

unhex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK;
  #assert r.written == out.count;
}
// PY_HEADER on a fresh server whose challenge was issued age_ms before now_ms.
once :: (s: [] Server, now_ms: int, age_ms: int) -> Answer {
  none: [1] u8; t: [32] u8; b: [100] u8;
  challenge(s, CH1, now_ms - age_ms);
  return login(s, ORIGIN, PY_HEADER, none[0..<0], login_body(CH1, b[..]), now_ms, t[..]);
}

#test "the words give NIP-06's published keys, and the login matches Python's byte for byte" #steps 8000000000 {
  key: [32] u8; cli: [32] u8; want: [32] u8; np: [80] u8;
  unhex(NIP06_KEY, want[..]);
  #assert nip06_key(PERSON_WORDS, key[..]) && cast(string) key[..] == cast(string) want[..];
  #assert cast(string) np[0..<nip06_npub(key[..], np[..])] == NIP06_NPUB;
  unhex(NIP06_KEY2, want[..]);
  #assert nip06_key(CLI_WORDS, cli[..]) && cast(string) cli[..] == cast(string) want[..];
  b: [100] u8; hb: [N98S_HEADER] u8; ev: [N98S_EVENT] u8;
  header := sign(key[..], 1, T0 + 30, LOGIN_URL, "POST", login_body(CH1, b[..]), true, hb[..]);
  #assert header == PY_HEADER && header.count == 718;
  d := base64_decode_checked(header[6..<header.count], ev[..], false);
  #assert d.error == NC_OK && d.written == 533;
  // the pubkey and the payload hash sit at the places the chapter works out
  #assert cast(string) ev[83..<147] == NIP06_PUBKEY && cast(string) ev[270..<334] == PY_PAYLOAD;
}
#test "the 60-second window, both edges, on a fresh server each time" #steps 8000000000 {
  a: [1] Server; b: [1] Server; c: [1] Server; d: [1] Server;
  #assert once(a[..], SIGNED_MS - 60000, 1000).status == 200 && once(b[..], SIGNED_MS + 60999, 1000).status == 200;
  early := once(c[..], SIGNED_MS - 61000, 1000); late := once(d[..], SIGNED_MS + 61000, 1000);
  #assert early.status == 401 && late.status == 401 && late.why == "created_at is not within 60 s" && early.why == late.why;
  // a refused signature uses up nothing
  #assert c[0].nseen == 0 && d[0].nseen == 0 && d[0].nsessions == 0 && !d[0].challenges[0].done;
}
#test "a challenge is good for five minutes and once; its refusal still uses up the signature" #steps 8000000000 {
  a: [1] Server; b: [1] Server; none: [1] u8; t: [32] u8; body: [100] u8;
  #assert once(a[..], SIGNED_MS, CHALLENGE_MS - 1).status == 200 && a[0].challenges[0].done;
  r := once(b[..], SIGNED_MS, CHALLENGE_MS);
  #assert r.status == 401 && r.words == "That challenge has ended. Ask for a new one." && b[0].nseen == 1 && b[0].nsessions == 0;
  challenge(b[..], CH1, SIGNED_MS);
  r = login(b[..], ORIGIN, PY_HEADER, none[0..<0], login_body(CH1, body[..]), SIGNED_MS, t[..]);
  #assert r.status == 401 && r.words == "That signature was used already." && !b[0].challenges[1].done;
}
#test "one login makes one account and one session; the refusals before the signature leave the server as it was" #steps 8000000000 {
  s: [1] Server; none: [1] u8; t: [32] u8; u: [32] u8; body: [100] u8;
  t[0] = 1; u[0] = 2;
  b := login_body(CH1, body[..]);
  challenge(s[..], CH1, SIGNED_MS);
  #assert login(s[..], "http://openagents.com", PY_HEADER, none[0..<0], b, SIGNED_MS, t[..]).status == 403;
  #assert login(s[..], ORIGIN, "Bearer x", none[0..<0], b, SIGNED_MS, t[..]).status == 401;
  #assert s[0].nseen == 0 && s[0].nsessions == 0 && s[0].accounts == 0;
  // no Origin at all: an app or a tool
  r := login(s[..], "", PY_HEADER, none[0..<0], b, SIGNED_MS, t[..]);
  #assert r.status == 200 && r.made && r.account == 1 && s[0].nseen == 1 && s[0].nsessions == 1 && s[0].nkeys == 1;
  #assert cast(string) s[0].keys[0].pubkey[..] == NIP06_PUBKEY;
  w := resolve(s[..], "GET", "/api/auth/account", "", t[..], "", SIGNED_MS + 1);
  #assert w.account == 1 && w.level == 2 && w.l2_age_ms == 1 && !w.nostr;
  #assert resolve(s[..], "GET", "/api/auth/account", "", u[..], "", SIGNED_MS + 1).kind == AP_NOBODY;
  // the cookie with a signature is two credentials, whatever the signature
  #assert resolve(s[..], "GET", "/api/auth/account", PY_HEADER, t[..], "", SIGNED_MS + 1).refused == 400;
}
#test "a session's clocks, and L2-fresh for five minutes" {
  s: [1] Server; t: [32] u8;
  t[0] = 1; s[0].accounts = 1;
  session_new(s[..], 1, 2, SIGNED_MS, t[..]);
  #assert session_state(s[..], t[..], SIGNED_MS + AP_ROTATE_MS - 1) == "good";
  #assert session_state(s[..], t[..], SIGNED_MS + AP_WEB_IDLE_MS - 1) == "good, and due a new token";
  #assert session_state(s[..], t[..], SIGNED_MS + AP_WEB_IDLE_MS) == "ended";
  fresh := resolve(s[..], "POST", "/api/auth/unlink", "", t[..], "", SIGNED_MS + AP_FRESH_MS);
  #assert gate(s[..], AP_L2_FRESH, true, fresh).status == 200;
  stale := resolve(s[..], "POST", "/api/auth/unlink", "", t[..], "", SIGNED_MS + AP_FRESH_MS + 1);
  #assert gate(s[..], AP_L2_FRESH, true, stale).status == 401 && gate(s[..], AP_L2, true, stale).status == 200;
  // used every day, a session still ends 180 days after it began
  s[0].sessions[0].seen_ms = SIGNED_MS + AP_WEB_ABSOLUTE_MS - 1;
  #assert session_state(s[..], t[..], SIGNED_MS + AP_WEB_ABSOLUTE_MS - 1) != "ended";
  #assert session_state(s[..], t[..], SIGNED_MS + AP_WEB_ABSOLUTE_MS) == "ended";
  #assert resolve(s[..], "GET", "/api/auth/account", "", t[..], "", SIGNED_MS + AP_WEB_ABSOLUTE_MS).kind == AP_NOBODY && s[0].sessions[0].ended;
  logout(s[..], t[..], SIGNED_MS);
  #assert gate(s[..], AP_ACCOUNT, false, resolve(s[..], "GET", "/api/auth/ways", "", t[..], "", SIGNED_MS)).status == 401;
}
#test "a device login: ten minutes, once, and never a key that is another account's" #steps 8000000000 {
  s: [1] Server; cli: [32] u8; e: [32] u8; p: [32] u8; hb: [N98S_HEADER] u8;
  unhex(NIP06_KEY2, cli[..]);
  e[0] = 1; p[0] = 2; s[0].accounts = 2;
  session_new(s[..], 1, 1, SIGNED_MS, e[..]);   // account 1: email only, no key
  session_new(s[..], 2, 2, SIGNED_MS, p[..]);   // account 2: logged in at L2
  body := "{\"machine\":\"box\"}";
  url := "https://openagents.com/api/auth/device/start?as=lines";
  #assert device_start(s[..], sign(cli[..], 1, T0 + 30, url, "POST", "{\"machine\":\"\"}", true, hb[..]), "{\"machine\":\"\"}", SIGNED_MS, DEV_ID, DEV_CODE).status == 400;
  #assert device_start(s[..], sign(cli[..], 2, T0 + 30, url, "POST", body, true, hb[..]), body, SIGNED_MS, DEV_ID, DEV_CODE).status == 200;
  #assert cast(string) s[0].devices[0].pubkey[..] == NIP06_PUBKEY2 && s[0].ndevices == 1;
  // an email session may link a key while the account has nothing stronger; then it has
  #assert device_approve(s[..], ORIGIN, e[..], DEV_CODE, SIGNED_MS + DEVICE_MS).status == 410;
  #assert device_approve(s[..], ORIGIN, e[..], DEV_CODE, SIGNED_MS + DEVICE_MS - 1).status == 200;
  #assert strong(s[..], 1) == 1 && s[0].keys[0].account == 1 && cast(string) s[0].keys[0].machine[0..<s[0].keys[0].nmachine] == "box";
  #assert gate(s[..], AP_HUMAN, true, resolve(s[..], "POST", "/api/auth/device/approve", "", e[..], "", SIGNED_MS)).status == 401;
  // the same key asks again: account 2 can't take it
  #assert device_start(s[..], sign(cli[..], 3, T0 + 30, url, "POST", body, true, hb[..]), body, SIGNED_MS, DEV_ID2, DEV_CODE2).status == 200;
  #assert device_approve(s[..], ORIGIN, p[..], DEV_CODE2, SIGNED_MS + 1).status == 409 && strong(s[..], 2) == 0;
}
```

```shell
coax/bin/coax test nostr/examples/checks/web-sign-in.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/web-sign-in.coax --root-only --run-steps 250000000
```

On 2026-11-12 this ran 526 checks, the loaded files' own included, in about
10 seconds; all passed. The checks of `nip98_sign.coax`, `nip98.coax`,
`auth-policy.coax`, `agent-policy.coax` and `account_rules.coax` are part
of that run.
`coax mutate --root-only` made 8 mutants in the checks file and all 8 were
killed. The mutation runs of the `#authority` files the example loads were
not repeated for this chapter.

### The API's own tests

[auth_checks.coax](../../coax/web/api/tests/auth_checks.coax) drives the
real login routes against Postgres, and
[cli_auth.coax](../../coax/web/api/tests/cli_auth.coax) drives the device
routes. They run in the API's `test.sh`, which needs Postgres and a build
host, and were not run for this chapter. The `/login` page's own checks are
in `login.coax`: the challenge, the request to `key.js`, the signed login,
and logging out first when a session is there.

The API's NIP-98 check and its comparison with the former C check are
chapter 49's.

## Summary

- A request carries one credential. A NIP-98 signature is its key's account
  at L2, proved now, for that one request.
- A key login is a signature over a server challenge: five minutes, one
  use, the body signed, the event ID accepted once. The worked header is
  718 bytes.
- In the browser a NIP-07 extension signs that event. The page never holds
  a key.
- A session is a server-made token whose hash is stored. It gets a new
  token every 24 hours and ends after 30 unused days or 180 days.
- `openagents login` starts a request with a new key, a logged-in person
  approves its code within ten minutes, and the key joins the account as
  `cli:MACHINE`. The CLI then signs every request and holds no token.
- An email session on an account that has a key can't approve a computer or
  change the account.

## Dependencies

- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md)
- [49. HTTP auth](ch049-http-auth.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-98](../nips/official/98.md)
- [NIP-06](../nips/official/06.md)
- [NIP-07](../nips/official/07.md)
- [docs/auth](../../docs/auth/README.md), the design of OpenAgents' own
  authentication.
- [docs/cli/2026-11-01-openagents-cli-v1.md](../../docs/cli/2026-11-01-openagents-cli-v1.md),
  the CLI's plan.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
