# 38. NIP-42 authentication

Some relays require a client to prove it holds a key. This chapter builds
NIP-42 on both sides and states what a successful AUTH does and does not
grant.

Part V, A relay. Result: Challenges bound to one connection and one relay
URL, kind 22242 events with `relay` and `challenge` tags checked within a time
window, and AUTH on both the client and the relay.

## What this chapter teaches

- The relay sends `["AUTH", challenge]`; a new challenge replaces the old one.
- The client signs kind 22242 with `["relay", url]` and `["challenge", c]`.
- Relay checks: kind, challenge, canonical URL, `created_at` within ten
  minutes, signature.
- Answers to an old challenge, another relay or another connection generation
  refuse.
- AUTH proves possession of a key; it does not grant membership or any other
  right.

Chapter 31 showed the client's side of an `auth-required:` refusal: the
publish waits, the client answers the challenge, and the publish goes out
again. Chapter 34 showed the write gate on the relay. This chapter is the
part between them: the challenge, the signed answer and the relay's check.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip42.coax](../../coax/lib/nostr/nip42.coax) (`#authority`) | `nip42_url` | the canonical form of a relay URL (chapter 28) |
| | `nip42_init`, `nip42_issue`, `nip42_challenge` | the per-connection `Nip42` record: the relay's canonical URL, the current challenge and the connection generation it belongs to; the relay writes the challenge frame, the client reads it |
| | `nip42_template` | the client's unsigned kind 22242 event: `created_at`, kind, tags and content, the inputs of chapter 21's signer |
| | `nip42_judge`, `nip42_admit_fields` | the relay's check of an AUTH event's fields; the key is returned only on `N42_OK` |
| | `nip42_input`, `nip42_send_fields` | the adapter to chapter 29's `relay.coax`: the challenge is kept only when `relay.coax` takes it, and the answer is sent only when the relay would admit it |
| [nip42-event.coax](../../coax/lib/nostr/nip42-event.coax) | `nip42_admit`, `nip42_send` | the same two calls on whole event JSON: `nostr_event_verify_vartime` (relay) or `nostr_event_read` (client) first, then the field check. It decides nothing itself, so it is not `#authority` |
| [relay_route.coax](../../coax/lib/nostr/relay_route.coax) (`#authority`) | `rr_challenge`, `rr_auth`, `rr_authed`, `rr_auth_prefix`, `rr_auth_text` | the challenge's hex, the connection's admitted keys (at most 8, each once) and the `OK` text for each result |
| [relay.coax](../../coax/relay/relay.coax) | `ry_challenge`, `ry_auth` | the relay program: a challenge after the 101, and each client `AUTH` through the procedures above |

`relay.coax` has a `main` and calls the host's sockets, so the example
can't load it. The example's `relay_auth` repeats `ry_message`'s route and
`ry_auth` line for line, with the clock passed in and the reply returned
instead of written to a socket.

## The challenge

A relay that supports NIP-42 can send `["AUTH", <challenge>]` at any
time. The Coax relay sends it once, right after the WebSocket handshake:
`ry_challenge` draws 32 bytes from host entropy, `rr_challenge` writes them
as 64 hex digits, and `nip42_issue` writes the frame and records the
challenge in the connection's `Nip42` record with the connection's
generation.

NIP-42 says a challenge "is valid for the duration of the connection or
until another challenge is sent by the relay". The record holds one
challenge, so the rule is a single assignment: `nip42_issue` and
`nip42_challenge` overwrite the challenge and its generation. An answer to
the old challenge no longer equals the current one and is refused.

On the client, `nip42_challenge` reads the frame. It accepts exactly a
two-element array: the string `AUTH` (any JSON spelling of it) and a string
of 1 to 256 bytes after unescaping, in a frame of at most 4,096 bytes.
Anything else is `N42_FRAME` and changes nothing. `nip42_input` calls it on
a copy of the record, then tells `relay.coax` with `RL_IN_AUTH`, and keeps
the copy only when `relay.coax` answers `RL_AUTH`. The two machines can't
disagree about whether a challenge is pending. `rl_next` then returns
`RL_OUT_AUTH`: the host owes the relay an answer.

## The generation

Each connection gets a generation number: `rl_start`'s result on the
client, and the connection's counter on the relay. The challenge is stored
with the generation it arrived on. Every later call passes the generation it
is acting for, and `nip42_ready` refuses `N42_STALE` when the two differ.

This matters on the client after a reconnect. The `Nip42` record still
holds the old challenge, because nothing told it about the drop. `relay.coax`
knows: `rl_dropped` and `rl_start` move it to a new generation, and
`nip42_send_fields` compares the caller's generation with `relay.coax`'s
before anything else. An answer built for generation 1 is not sent on
generation 2. Chapter 29 covers the reconnect itself.

## The answer: kind 22242

`nip42_template(n, gen, now, buf)` builds the unsigned event:

- `created_at` is `now`, which must be from 0 to 2^53 - 1.
- `kind` is 22242.
- `tags` is `[["relay",<canonical url>],["challenge",<challenge>]]`, written
  into `buf` with NIP-01's escapes. The URL is the canonical form of the URL
  the client connected to.
- `content` is empty.

These are exactly the inputs of `nostr_event_sign_step` (chapter 21), so
signing is a loop that calls it until it stops returning `EVENT_MORE`. Then
`nip42_send` reads the signed event back with `nostr_event_read`, which
recomputes the id, and applies the relay's own field check to it. Only an
event that passes is wrapped as `["AUTH",<event>]`, and only then is
`rl_auth_sent(id)` called, so `relay.coax` waits for an `OK` with that id. A
second answer before the `OK` is `N42_MACHINE`: there is no unanswered
challenge left.

Kind 22242 is ephemeral (20000 to 29999), and NIP-42 says a relay must not
broadcast it. The Coax relay never sends an `AUTH` event to the store or to
fan-out, and chapter 34's `rs_admit_read` refuses kind 22242 sent as an
`EVENT`.

## The relay's check

`ry_auth` handles one client `AUTH` in three steps:

1. `nostr_event_read` recomputes the id. A wrong id gets `OK false`
   `invalid: event id does not match its content` before any signature
   work, and JSON that isn't an event gets a `NOTICE`.
2. `nip42_admit` verifies the BIP-340 signature with
   `nostr_event_verify_vartime`, then runs `nip42_judge` on the event's
   fields.
3. `rr_auth` turns the result into an answer and, on success, adds the key
   to the connection's list.

`nip42_judge` checks, in this order:

1. `nip42_ready`: the generation is the current one (`N42_STALE`), there is
   a challenge (`N42_NO_CHALLENGE`) and a URL (`N42_URL`).
2. The kind is 22242 (`N42_KIND`).
3. `|created_at - now| <= 600` seconds, inclusive (`N42_TIME`).
4. The tags are an array of arrays of strings with exactly one `relay` tag
   and exactly one `challenge` tag, each with exactly one value. Other tags
   and empty tags are ignored (`N42_TAGS`).
5. The relay tag's canonical form equals the relay's own (`N42_RELAY`).
6. The challenge tag, unescaped, equals the current challenge byte for byte
   (`N42_CHALLENGE`).

The content is not read. NIP-42 says "e.g. within ~10 minutes" for the
time; the code uses 600 seconds both ways. It says "URL normalization
techniques can be applied"; the code compares chapter 28's canonical forms,
so `wss://relay.example.com/` and `WSS://Relay.Example.COM:443` are the same
relay, and `ws://relay.example.com` and `wss://relay.example.com:8443` are
not.

### The answers

`rr_auth_answer` maps each result to an `OK` reason, with the Rust relay's
texts:

| `nip42_admit` | `OK` message |
| --- | --- |
| `N42_OK` | `true`, `""` |
| `N42_EVENT` (signature) | `invalid: bad signature` |
| `N42_KIND` | `invalid: authentication event must have kind 22242` |
| `N42_TIME` | `invalid: the authentication event's timestamp is more than 10 minutes from the relay's clock` |
| `N42_TAGS` | `invalid: authentication event must have exactly one relay tag and one challenge tag` |
| `N42_RELAY` | `invalid: authentication relay URL does not match` |
| `N42_CHALLENGE`, `N42_STALE`, `N42_NO_CHALLENGE` | `invalid: authentication challenge does not match` |
| an admitted key when 8 are already kept | `restricted: too many authenticated keys on this connection` |

### What an admitted key grants

An admitted `AUTH` event proves one thing: whoever holds this connection
holds the secret key for that public key, now. `rr_auth` adds the key to
the connection's list, and the relay uses the list for one decision:
`rr_authed` is passed to `rs_admit_read`, and a write from a connection with
no key is refused with `auth-required: authenticate before publishing`
(chapter 34). Any admitted key opens writes, including events signed by
other keys, as on the Rust relay.

It grants nothing else. It is not membership, payment, a role or a right to
any channel. Chapters 42, 59 and 87 build those decisions on top of the
proven key, and each makes its own check.

NIP-42 lets a client authenticate several keys on one connection, and says
relays "MUST treat all pubkeys as authenticated". The Coax relay keeps up to
8 per connection, and the same key again is `OK true` and not counted twice.
A new challenge on the same connection doesn't remove keys already admitted;
a new connection starts with none.

### Reuse

Nothing in NIP-42 says an `AUTH` event can be used only once, and the code
doesn't refuse a second use. The same event sent again on the same
connection, while its challenge is current and its `created_at` is within
600 seconds, is `OK true` again and changes nothing. Reuse elsewhere is
refused by the rules above: on another connection the challenge differs (32
fresh random bytes), after a new challenge it no longer matches, and after
ten minutes the time check fails.

## Worked example: one answer by hand

The relay was started as `wss://relay.example.com/`. The client dialed
`WSS://Relay.Example.COM:443`. Both clocks read 1793214821
(2026-10-28T19:13:41Z). The test key is BIP-340 test vector 0's secret key,
the scalar 3, with zero aux, the same key and aux that
`coax/lib/nostr/fixtures/nip42_events.ts` uses with `@noble/curves`.

1. **The relay's URL.** `wss://` is the scheme, `relay.example.com` the
   host, there is no port, and the path `/` loses its trailing slash:
   `wss://relay.example.com`, 23 bytes.
2. **The client's URL.** `WSS://` is written `wss://`, `Relay.Example.COM`
   is lowercased, and `443` is the default port for `wss`, so it is dropped:
   `wss://relay.example.com`. The two are equal.
3. **The challenge.** The relay sends `["AUTH","challengestringhere"]`. The
   client records `challengestringhere` (19 bytes) for generation 1.
4. **The tags.** `[["relay","wss://relay.example.com"],["challenge","challengestringhere"]]`,
   73 bytes.
5. **The id.** The NIP-01 preimage is
   `[0,"f9308a01…36f9",1793214821,22242,[["relay","wss://relay.example.com"],["challenge","challengestringhere"]],""]`,
   164 bytes with the full 64-digit public key, and its SHA-256 is
   `0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523`.
6. **The frame.** The signed event is 417 bytes. `["AUTH",` adds 8 and `]`
   adds 1: 426 bytes.
7. **The relay's check.** Generation 1 is current, the challenge and URL are
   set; the kind is 22242; `created_at - now = 0`; one `relay` tag and one
   `challenge` tag; the relay tag's canonical form is
   `wss://relay.example.com`, equal; the challenge is equal. `N42_OK`.
8. **The window.** The same event is admitted while the relay's clock reads
   from 1793214221 to 1793215421 inclusive, and refused at 1793214220 or
   1793215422.
9. **The answer.** `["OK","0b3b48de…7523",true,""]`, 81 bytes with the full
   id. The connection has one key.

Then the refusals, each worked from the rules above:

| Input | Refused at | `OK` message |
| --- | --- | --- |
| the same tags with kind 22243 | step 2 | `invalid: authentication event must have kind 22242` |
| relay tag `wss://other.example.com` | step 5 | `invalid: authentication relay URL does not match` |
| a second `challenge` tag | step 4 | `invalid: authentication event must have exactly one relay tag and one challenge tag` |
| the first signature digit changed | the signature | `invalid: bad signature` |
| `created_at` edited after signing | the id | `invalid: event id does not match its content` |
| the good event, relay clock 601 s later | step 3 | `invalid: the authentication event's timestamp is more than 10 minutes from the relay's clock` |
| the good event after the relay sent `["AUTH","secondchallenge"]` | step 6 | `invalid: authentication challenge does not match` |
| the good event on a second connection | step 6 | `invalid: authentication challenge does not match` |

## The example

The example is [nostr/examples/nip42-auth.coax](../examples/nip42-auth.coax).
It loads `relay_server.coax`, `relay_route.coax`, `nip42-event.coax` (which
loads `nip42.coax` and the client `relay.coax`) and chapter 21's signer. It
runs offline, with fixed clocks, and:

1. canonicalizes nine relay-tag spellings and compares each with the
   relay's own URL;
2. runs the handshake on both sides: the relay's challenge, the client's
   template, the signed event, the `AUTH` frame, the relay's `OK` and the
   client's state after it;
3. moves the relay's clock through the 600-second window;
4. sends the relay each refused input from the table above, a non-event
   `AUTH`, the same event again, a second key, a new challenge and a second
   connection;
5. shows the one thing an admitted key changes: the `EVENT` policy;
6. shows what the client refuses to send: bad challenge frames, an answer
   the relay would refuse, a second answer before the `OK`, and an answer
   from the previous generation.

The relay's handling of one `AUTH` frame:

<!-- source: nostr/examples/nip42-auth.coax relay -->
```jai
// One AUTH frame from the client, handled as coax/relay/relay.coax's
// ry_message and ry_auth handle it, with the socket and the clock replaced:
// the reply the relay sends.
relay_auth :: (c: [] RelayConn, frame: string, now: int) -> string {
  out := c[0].out[..];
  m := ms_read(frame);
  route := rr_message(m.error, m.type, m.has_sub);
  if route.action == RR_NOTICE { return text_of(ms_write_notice(rs_prefix(route.reason), rs_text(route.reason), out), out); }
  #assert route.action == RR_AUTH;
  obj := frame[m.event_from..<m.event_to];
  // 1. The id, for the OK. A wrong id is refused before any signature work.
  e := nostr_event_read(obj, c[0].scratch[..], false);
  id := e.id;
  if e.error == NE_ID { return text_of(ms_write_ok(id[..], false, rs_prefix(RS_BAD_ID), rs_text(RS_BAD_ID), out), out); }
  if e.error != NE_OK { return text_of(ms_write_notice(rs_prefix(RS_BAD_EVENT), rs_text(RS_BAD_EVENT), out), out); }
  // 2. The signature, then nip42.coax's check against this connection.
  a := nip42_admit(c[0].n42[..], c[0].gen, now, obj, c[0].scratch[..], false);
  // 3. Keep the key; name the answer.
  key := a.public_key;
  k := rr_auth(c[0].auth[..], a.error, key[..]);
  return text_of(ms_write_ok(id[..], k == RR_A_OK, rr_auth_prefix(k), rr_auth_text(k), out), out);
}
```

The whole program:

<!-- source: nostr/examples/nip42-auth.coax example -->
```jai
#load "../../coax/lib/nostr/relay_server.coax";
#load "../../coax/lib/nostr/relay_route.coax";
#load "../../coax/lib/nostr/nip42-event.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// Both clocks are fixed: the relay's and the client's read this Unix time
// (2026-10-28T19:13:41Z), the time of nip42_events.ts's signed fixtures.
NOW :: 1793214821;
// The relay's own URL (coax/relay/relay.coax --url) and the one the client
// dialed: two spellings of one relay.
RELAY_URL :: "wss://relay.example.com/";
CLIENT_URL :: "WSS://Relay.Example.COM:443";
CHALLENGE :: "challengestringhere";
// BIP-340 test vector 0's secret key (the scalar 3) with zero aux, the key
// and aux nip42_events.ts signs with, and vector 1's key as a second
// signer. Published test keys: never sign with a real key in an example.
KEY_A :: "0000000000000000000000000000000000000000000000000000000000000003";
KEY_B :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";

n42_name :: (e: int) -> string {
  names := [13] string.["N42_OK", "N42_FRAME", "N42_URL", "N42_STALE", "N42_NO_CHALLENGE", "N42_EVENT",
    "N42_KIND", "N42_TIME", "N42_TAGS", "N42_RELAY", "N42_CHALLENGE", "N42_OUTPUT", "N42_MACHINE"];
  return names[e];
}
url_name :: (e: int) -> string {
  names := [7] string.["OK", "OUTPUT", "LIMIT", "SCHEME", "HOST", "PORT", "PATH"];
  return names[e];
}
auth_name :: (a: int) -> string {
  names := [4] string.["RA_NONE", "RA_CHALLENGED", "RA_SENT", "RA_OK"];
  return names[a];
}
rl_name :: (d: int) -> string {
  if d == -1 { return "-1 (not a challenge)"; }
  names := [12] string.["RL_DELIVER", "RL_DUPLICATE", "RL_STALE", "RL_UNKNOWN", "RL_FORGED", "RL_EOSE",
                        "RL_ACK", "RL_CLOSED", "RL_NOTICE", "RL_AUTH", "RL_NOT_OPEN", "RL_BAD"];
  return names[d];
}
text_of :: (n: int, out: [] u8) -> string {
  #assert n > 0;
  return cast(string) out[0..<n];
}

// Signs one event with a test key into out; the event's text.
sign :: (key_hex: string, created_at: int, kind: int, tags: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(key_hex, key[..], false).error == NC_OK;
  #assert hex_decode_checked(AUX, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [1024] u8; k := EVENT_MORE;
  while k == EVENT_MORE {
    k = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, "", work[..], scratch[..], out, false);
  }
  return text_of(k, out);
}
// ["AUTH",<event>], the frame a client sends.
auth_frame :: (event: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[\"AUTH\","); n = json_raw(out, n, event);
  return text_of(json_raw(out, n, "]"), out);
}

// What the relay holds for one connection.
RelayConn :: struct {
  gen: int;
  n42: [1] Nip42;           // the relay URL and this connection's challenge
  auth: [1] RrAuth;         // the keys admitted on this connection
  scratch: [65536] u8;
  out: [1024] u8;
}
// A new connection (generation gen): the relay's URL, then the challenge,
// sent as ["AUTH", challenge]. The frame it sends.
relay_open :: (c: [] RelayConn, gen: int, challenge: string) -> string {
  c[0].gen = gen; rr_auth_reset(c[0].auth[..]);
  #assert nip42_init(c[0].n42[..], RELAY_URL) == N42_OK;
  f := nip42_issue(c[0].n42[..], gen, challenge, c[0].out[..]);
  #assert f.error == N42_OK;
  return text_of(f.written, c[0].out[..]);
}

// book:begin relay
// One AUTH frame from the client, handled as coax/relay/relay.coax's
// ry_message and ry_auth handle it, with the socket and the clock replaced:
// the reply the relay sends.
relay_auth :: (c: [] RelayConn, frame: string, now: int) -> string {
  out := c[0].out[..];
  m := ms_read(frame);
  route := rr_message(m.error, m.type, m.has_sub);
  if route.action == RR_NOTICE { return text_of(ms_write_notice(rs_prefix(route.reason), rs_text(route.reason), out), out); }
  #assert route.action == RR_AUTH;
  obj := frame[m.event_from..<m.event_to];
  // 1. The id, for the OK. A wrong id is refused before any signature work.
  e := nostr_event_read(obj, c[0].scratch[..], false);
  id := e.id;
  if e.error == NE_ID { return text_of(ms_write_ok(id[..], false, rs_prefix(RS_BAD_ID), rs_text(RS_BAD_ID), out), out); }
  if e.error != NE_OK { return text_of(ms_write_notice(rs_prefix(RS_BAD_EVENT), rs_text(RS_BAD_EVENT), out), out); }
  // 2. The signature, then nip42.coax's check against this connection.
  a := nip42_admit(c[0].n42[..], c[0].gen, now, obj, c[0].scratch[..], false);
  // 3. Keep the key; name the answer.
  key := a.public_key;
  k := rr_auth(c[0].auth[..], a.error, key[..]);
  return text_of(ms_write_ok(id[..], k == RR_A_OK, rr_auth_prefix(k), rr_auth_text(k), out), out);
}
// book:end relay

// The relay's check alone, without the signature: what nip42_admit decides
// after nostr_event_verify_vartime.
judged :: (c: [] RelayConn, event: string, now: int) -> string {
  s: [4096] u8; e := nostr_event_read(event, s[..], false);
  return n42_name(nip42_judge(c[0].n42[..], c[0].gen, now, e.kind, e.created_at, n42e_tags(e)));
}
show_keys :: (c: [] RelayConn) {
  print("  relay: % key(s) authenticated on this connection\n", c[0].auth[0].n);
}
// A copy of s with byte at replaced by b.
edited :: (s: string, at: int, b: u8, out: [] u8) -> string {
  for i: 0..<s.count { out[i] = s[i]; }
  out[at] = b;
  return cast(string) out[0..<s.count];
}
// The index of the first byte of `field` in s, after its opening quote.
find :: (s: string, field: string) -> int {
  for i: 0..<s.count - field.count + 1 { if s[i..<i + field.count] == field { return i + field.count; } }
  return -1;
}

main :: () {
  print("== 1. Relay URLs: does a relay tag name this relay?\n");
  own: [N42_MAX_URL] u8; o := nip42_url(RELAY_URL, own[..]);
  print("relay's own URL % -> %\n", RELAY_URL, cast(string) own[0..<o.count]);
  tags := [9] string.["wss://relay.example.com", "WSS://Relay.Example.COM:443", "wss://relay.example.com:0443//",
    "ws://relay.example.com", "wss://relay.example.com:8443", "wss://relay.example.com/nostr",
    "wss://other.example.com", "wss://relay.example.com/?x=1", "https://relay.example.com"];
  for t: tags {
    canon: [N42_MAX_URL] u8; u := nip42_url(t, canon[..]);
    if u.error != N42U_OK { print("  % -> refused (%): no match\n", t, url_name(u.error)); continue; }
    c := cast(string) canon[0..<u.count];
    print("  % -> % : %\n", t, c, ifx c == cast(string) own[0..<o.count] then "match" else "no match");
  }

  print("\n== 2. The handshake, both sides\n");
  conn: [1] RelayConn;
  // relay_open's frame is a view into conn's reply buffer: keep a copy.
  cb: [64] u8; frame := copy_of(relay_open(conn[..], 1, CHALLENGE), cb[..]);
  print("relay sends %\n", frame);
  r: [1] Relay; n: [1] Nip42;
  g := rl_start(r[..], 0); opened := rl_opened(r[..], g, 0);
  #assert nip42_init(n[..], CLIENT_URL) == N42_OK;
  print("client: connection generation %, relay URL % read as %\n", g, CLIENT_URL, nip42_url_text(n[..]));
  d := nip42_input(r[..], n[..], g, frame);
  print("client: nip42_input -> %, challenge %, state %\n", rl_name(d), nip42_challenge_text(n[..]), auth_name(r[0].auth));
  print("client: rl_next -> % (RL_OUT_AUTH is %)\n", rl_next(r[..], 0).kind, RL_OUT_AUTH);
  tb: [512] u8; t := nip42_template(n[..], g, NOW, tb[..]);
  print("client: template created_at % kind % content \"%\"\n  tags %\n", t.created_at, t.kind, t.content, t.tags);
  eb: [1024] u8; event := sign(KEY_A, t.created_at, t.kind, t.tags, eb[..]);
  print("client: signed with test key A (%  bytes)\n  %\n", event.count, event);
  sb: [4096] u8; fb: [1200] u8;
  f := nip42_send(r[..], n[..], g, NOW, event, sb[..], fb[..], false);
  sent := text_of(f.written, fb[..]);
  print("client: nip42_send -> %, frame % bytes, state %\n", n42_name(f.error), f.written, auth_name(r[0].auth));
  reply := relay_auth(conn[..], sent, NOW);
  print("relay answers %\n", reply);
  show_keys(conn[..]);
  e := nostr_event_read(event, sb[..], false);
  print("client: OK true for its AUTH id -> %, state %\n",
        rl_name(rl_input(r[..], g, RlIn.{kind = RL_IN_OK, id = e.id, accepted = true})), auth_name(r[0].auth));

  print("\n== 3. The time window, on the relay's clock\n");
  drift := [5] int.[-601, -600, 0, 600, 601];
  for dt: drift {
    print("  relay clock created_at % %: %\n", ifx dt < 0 then "-" else "+", ifx dt < 0 then -dt else dt, judged(conn[..], event, NOW + dt));
  }
  print("  at created_at+601 the relay answers %\n", relay_auth(conn[..], sent, NOW + 601));

  print("\n== 4. Refusals at the relay (connection 1, challenge %)\n", CHALLENGE);
  b1: [1024] u8; f1: [1200] u8;
  kind := sign(KEY_A, NOW, 22243, t.tags, b1[..]);
  print("kind 22243:\n  %\n", relay_auth(conn[..], auth_frame(kind, f1[..]), NOW));
  b2: [1024] u8; f2: [1200] u8;
  other := sign(KEY_A, NOW, 22242, "[[\"relay\",\"wss://other.example.com\"],[\"challenge\",\"challengestringhere\"]]", b2[..]);
  print("relay tag wss://other.example.com:\n  %\n", relay_auth(conn[..], auth_frame(other, f2[..]), NOW));
  b3: [1024] u8; f3: [1200] u8;
  two := sign(KEY_A, NOW, 22242, "[[\"relay\",\"wss://relay.example.com\"],[\"challenge\",\"challengestringhere\"],[\"challenge\",\"other\"]]", b3[..]);
  print("two challenge tags:\n  %\n", relay_auth(conn[..], auth_frame(two, f3[..]), NOW));
  x1: [1024] u8; f4: [1200] u8;
  sig_at := find(event, "\"sig\":\"");
  badsig := edited(event, sig_at, ifx event[sig_at] == '0' then cast(u8) '1' else cast(u8) '0', x1[..]);
  print("first signature digit changed (the id still matches):\n  %\n", relay_auth(conn[..], auth_frame(badsig, f4[..]), NOW));
  x2: [1024] u8; f5: [1200] u8;
  ca := find(event, "\"created_at\":");
  badid := edited(event, ca + 9, ifx event[ca + 9] == '0' then cast(u8) '1' else cast(u8) '0', x2[..]);
  print("created_at changed after signing (the id no longer matches):\n  %\n", relay_auth(conn[..], auth_frame(badid, f5[..]), NOW));
  print("[\"AUTH\",\"x\"] from a client (not an event):\n  %\n", relay_auth(conn[..], "[\"AUTH\",\"x\"]", NOW));
  print("the same AUTH frame again, on the same connection and challenge:\n  %\n", relay_auth(conn[..], sent, NOW));
  show_keys(conn[..]);
  b4: [1024] u8; f6: [1200] u8;
  keyb := sign(KEY_B, NOW, 22242, t.tags, b4[..]);
  print("the same challenge answered by test key B:\n  %\n", relay_auth(conn[..], auth_frame(keyb, f6[..]), NOW));
  show_keys(conn[..]);
  print("the relay sends a new challenge on connection 1: %\n", relay_open_again(conn[..], "secondchallenge"));
  print("key A's first answer now:\n  %\n", relay_auth(conn[..], sent, NOW));
  print("  (a new challenge leaves the keys already admitted: % key(s))\n", conn[0].auth[0].n);
  conn2: [1] RelayConn;
  bytes: [32] u8; for i: 0..<32 { bytes[i] = cast(u8) i; }
  hex: [64] u8; #assert rr_challenge(bytes[..], hex[..]) == 64;
  print("connection 2 opens: %\n", relay_open(conn2[..], 2, cast(string) hex[..]));
  print("key A's first answer replayed on connection 2:\n  %\n", relay_auth(conn2[..], sent, NOW));
  show_keys(conn2[..]);

  print("\n== 5. What an admitted key changes: the EVENT policy\n");
  b5: [1024] u8; note := sign(KEY_A, NOW, 1, "[]", b5[..]);
  s5: [4096] u8;
  print("kind 1 note, connection 2 (no key): %\n", rs_text(rs_admit_read(note, NOW, rr_writes_need_auth(true), rr_authed(conn2[0].auth[..]), s5[..]).reason));
  print("kind 1 note, connection 1 (keys A, B): %\n", ifx rs_admit_read(note, NOW, rr_writes_need_auth(true), rr_authed(conn[0].auth[..]), s5[..]).reason == RS_ACCEPT then "accepted by the policy" else "refused");
  print("the AUTH event itself sent as EVENT: %\n", rs_text(rs_admit_read(event, NOW, rr_writes_need_auth(true), rr_authed(conn[0].auth[..]), s5[..]).reason));

  print("\n== 6. Refusals at the client\n");
  r2: [1] Relay; n2: [1] Nip42;
  g1 := rl_start(r2[..], 0); opened = rl_opened(r2[..], g1, 0);
  #assert nip42_init(n2[..], CLIENT_URL) == N42_OK;
  bad_frames := [4] string.["[\"AUTH\",\"\"]", "[\"AUTH\",{\"id\":\"x\"}]", "[\"AUTH\",\"a\",\"b\"]", "[\"auth\",\"a\"]"];
  for bf: bad_frames { print("  nip42_input % -> %\n", bf, rl_name(nip42_input(r2[..], n2[..], g1, bf))); }
  print("  nip42_input % -> %, state %\n", frame, rl_name(nip42_input(r2[..], n2[..], g1, frame)), auth_name(r2[0].auth));
  o2: [1200] u8;
  print("  send the kind 22243 event: %, state %\n", n42_name(nip42_send(r2[..], n2[..], g1, NOW, kind, sb[..], o2[..], false).error), auth_name(r2[0].auth));
  print("  send the event for another relay: %\n", n42_name(nip42_send(r2[..], n2[..], g1, NOW, other, sb[..], o2[..], false).error));
  print("  send the event with a changed created_at: %\n", n42_name(nip42_send(r2[..], n2[..], g1, NOW, badid, sb[..], o2[..], false).error));
  print("  send it 601 s late: %\n", n42_name(nip42_send(r2[..], n2[..], g1, NOW + 601, event, sb[..], o2[..], false).error));
  f7 := nip42_send(r2[..], n2[..], g1, NOW, event, sb[..], o2[..], false);
  print("  send the good event: %, state %\n", n42_name(f7.error), auth_name(r2[0].auth));
  print("  send it again before the OK: %\n", n42_name(nip42_send(r2[..], n2[..], g1, NOW, event, sb[..], o2[..], false).error));
  print("  OK false for it -> %, state %\n",
        rl_name(rl_input(r2[..], g1, RlIn.{kind = RL_IN_OK, id = e.id, accepted = false})), auth_name(r2[0].auth));
  dropped := rl_dropped(r2[..], g1, 0, 0); g2 := rl_start(r2[..], 1000); opened = rl_opened(r2[..], g2, 1000);
  print("  reconnect: generation %, state %\n", g2, auth_name(r2[0].auth));
  // The Nip42 record still holds generation 1's challenge; only relay.coax
  // knows about the reconnect, and nip42_send asks it.
  print("  template for generation %: %; for generation %: %\n", g1, n42_name(nip42_template(n2[..], g1, NOW, tb[..]).error),
        g2, n42_name(nip42_template(n2[..], g2, NOW, tb[..]).error));
  print("  send the old answer on generation %: %\n", g2, n42_name(nip42_send(r2[..], n2[..], g2, NOW, event, sb[..], o2[..], false).error));
}
copy_of :: (s: string, out: [] u8) -> string {
  for i: 0..<s.count { out[i] = s[i]; }
  return cast(string) out[0..<s.count];
}
// A new challenge on an open connection; the frame sent.
relay_open_again :: (c: [] RelayConn, challenge: string) -> string {
  f := nip42_issue(c[0].n42[..], c[0].gen, challenge, c[0].out[..]);
  #assert f.error == N42_OK;
  return text_of(f.written, c[0].out[..]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/nip42-auth.coax --run-steps 250000000
```

Its output, which `book.py` compares in debug and release:

```text
== 1. Relay URLs: does a relay tag name this relay?
relay's own URL wss://relay.example.com/ -> wss://relay.example.com
  wss://relay.example.com -> wss://relay.example.com : match
  WSS://Relay.Example.COM:443 -> wss://relay.example.com : match
  wss://relay.example.com:0443// -> wss://relay.example.com : match
  ws://relay.example.com -> ws://relay.example.com : no match
  wss://relay.example.com:8443 -> wss://relay.example.com:8443 : no match
  wss://relay.example.com/nostr -> wss://relay.example.com/nostr : no match
  wss://other.example.com -> wss://other.example.com : no match
  wss://relay.example.com/?x=1 -> refused (PATH): no match
  https://relay.example.com -> refused (SCHEME): no match

== 2. The handshake, both sides
relay sends ["AUTH","challengestringhere"]
client: connection generation 1, relay URL WSS://Relay.Example.COM:443 read as wss://relay.example.com
client: nip42_input -> RL_AUTH, challenge challengestringhere, state RA_CHALLENGED
client: rl_next -> 4 (RL_OUT_AUTH is 4)
client: template created_at 1793214821 kind 22242 content ""
  tags [["relay","wss://relay.example.com"],["challenge","challengestringhere"]]
client: signed with test key A (417  bytes)
  {"id":"0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523","pubkey":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","created_at":1793214821,"kind":22242,"tags":[["relay","wss://relay.example.com"],["challenge","challengestringhere"]],"content":"","sig":"f588fffcd1d585576c8e45caef8d61108da7ae02f745ba9e973e19283cefe7e6247e99012199635f50eb2f3c14a2c79a9708fbdb5f64eaa57313b3981bf4c60b"}
client: nip42_send -> N42_OK, frame 426 bytes, state RA_SENT
relay answers ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",true,""]
  relay: 1 key(s) authenticated on this connection
client: OK true for its AUTH id -> RL_AUTH, state RA_OK

== 3. The time window, on the relay's clock
  relay clock created_at - 601: N42_TIME
  relay clock created_at - 600: N42_OK
  relay clock created_at + 0: N42_OK
  relay clock created_at + 600: N42_OK
  relay clock created_at + 601: N42_TIME
  at created_at+601 the relay answers ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",false,"invalid: the authentication event's timestamp is more than 10 minutes from the relay's clock"]

== 4. Refusals at the relay (connection 1, challenge challengestringhere)
kind 22243:
  ["OK","0224ba5911a3730152d68dad37d95dce694d85ba971aa4d785e0747c4f498e45",false,"invalid: authentication event must have kind 22242"]
relay tag wss://other.example.com:
  ["OK","0f06064812622c23be9d19bafc9b07d316e939de144186350402efb7eea761ec",false,"invalid: authentication relay URL does not match"]
two challenge tags:
  ["OK","da02f95157f0585e1e11a14abede7aca218320e9c2cc5d208880606969f2adea",false,"invalid: authentication event must have exactly one relay tag and one challenge tag"]
first signature digit changed (the id still matches):
  ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",false,"invalid: bad signature"]
created_at changed after signing (the id no longer matches):
  ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",false,"invalid: event id does not match its content"]
["AUTH","x"] from a client (not an event):
  ["NOTICE","invalid: malformed message"]
the same AUTH frame again, on the same connection and challenge:
  ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",true,""]
  relay: 1 key(s) authenticated on this connection
the same challenge answered by test key B:
  ["OK","0d5d40a7d02788776532c76cebd3a648fc1f94342514814919aed09151891083",true,""]
  relay: 2 key(s) authenticated on this connection
the relay sends a new challenge on connection 1: ["AUTH","secondchallenge"]
key A's first answer now:
  ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",false,"invalid: authentication challenge does not match"]
  (a new challenge leaves the keys already admitted: 2 key(s))
connection 2 opens: ["AUTH","000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"]
key A's first answer replayed on connection 2:
  ["OK","0b3b48de11adfaab6beb7fc410a56df52957ba0dfcb15e18e3473f2fc9067523",false,"invalid: authentication challenge does not match"]
  relay: 0 key(s) authenticated on this connection

== 5. What an admitted key changes: the EVENT policy
kind 1 note, connection 2 (no key): authenticate before publishing
kind 1 note, connection 1 (keys A, B): accepted by the policy
the AUTH event itself sent as EVENT: authentication events cannot be published

== 6. Refusals at the client
  nip42_input ["AUTH",""] -> -1 (not a challenge)
  nip42_input ["AUTH",{"id":"x"}] -> -1 (not a challenge)
  nip42_input ["AUTH","a","b"] -> -1 (not a challenge)
  nip42_input ["auth","a"] -> -1 (not a challenge)
  nip42_input ["AUTH","challengestringhere"] -> RL_AUTH, state RA_CHALLENGED
  send the kind 22243 event: N42_KIND, state RA_CHALLENGED
  send the event for another relay: N42_RELAY
  send the event with a changed created_at: N42_EVENT
  send it 601 s late: N42_TIME
  send the good event: N42_OK, state RA_SENT
  send it again before the OK: N42_MACHINE
  OK false for it -> RL_AUTH, state RA_NONE
  reconnect: generation 2, state RA_NONE
  template for generation 1: N42_OK; for generation 2: N42_STALE
  send the old answer on generation 2: N42_STALE
```

The output matches the hand work:

- The canonical forms in section 1 are those of steps 1 and 2. A query is
  refused (`PATH`), as is `https://` (`SCHEME`): a tag that doesn't
  canonicalize can't match.
- The signed event in section 2 has the id from step 5 and is 417 bytes;
  the frame is 426. It is byte for byte `N42_FX_TEMPLATE`, the event
  `@noble/curves` signed for the same fields.
- Section 3 is step 8: -600, 0 and +600 pass, -601 and +601 refuse.
- Section 4 is the refusals table, plus three cases the table doesn't
  show. `["AUTH","x"]` is not an event, so there is no id to answer and the
  relay sends a `NOTICE`. The same frame again is `OK true` with one key
  still kept. Test key B answering the same challenge is admitted as a
  second key.
- In section 6 the client refuses every answer the relay would refuse, so a
  wrong answer never reaches the wire, and `relay.coax`'s state stays
  `RA_CHALLENGED`. After an `OK false` the state is `RA_NONE`: the client
  waits for a new challenge. After the reconnect, `nip42_template` still
  builds an answer for generation 1, because the `Nip42` record keeps the
  old challenge, but `nip42_send` refuses to send it on generation 2.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/nip42-auth.coax](../examples/checks/nip42-auth.coax),
which loads the example, so running the example doesn't run them:

- the canonical URLs, the challenge frame, the template's 73-byte tags and
  the 426-byte frame are the values worked out by hand;
- the client's signed answer is byte for byte `N42_FX_TEMPLATE` from
  `coax/lib/nostr/fixtures/nip42_events.coax`, which
  `nip42_events.ts` signed with `@noble/curves`, an implementation that
  shares no code with Coax;
- `@noble/curves`-signed events get the `OK` frames written by hand: the
  spec's own spelling `wss://relay.example.com/` and the spelling
  `WSS://Relay.Example.COM:443` with an extra tag are admitted (one key,
  counted once); kind 22243 and two challenge tags are refused; 601 s of
  drift is refused; an answer to a replaced challenge is refused;
- a changed signature digit and a changed `created_at` get the signature
  and id refusals, and a non-event `AUTH` a `NOTICE`;
- the window is inclusive at 600 s both ways;
- the client refuses a wrong kind and a late answer without changing
  `relay.coax`'s state, refuses a second answer before the `OK`, and refuses
  an old generation's answer after a reconnect.

<!-- source: nostr/examples/checks/nip42-auth.coax check -->
```jai
// The chapter's hand trace: the canonical URLs, the tags, the frame lengths.
HAND_TAGS :: "[[\"relay\",\"wss://relay.example.com\"],[\"challenge\",\"challengestringhere\"]]";
// The OK frame worked out by hand from an id, a verdict and a message.
ok_frame :: (id: string, accepted: bool, message: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[\"OK\",\""); n = json_raw(out, n, id); n = json_raw(out, n, "\",");
  n = json_raw(out, n, ifx accepted then "true" else "false"); n = json_raw(out, n, ",\"");
  n = json_raw(out, n, message);
  return text_of(json_raw(out, n, "\"]"), out);
}
// The id field of an event's JSON (it is the first field in every fixture).
id_of :: (event: string) -> string { return event[7..<71]; }
// A relay connection on generation 1 with the chapter's challenge.
open_conn :: (c: [] RelayConn) { f := relay_open(c, 1, CHALLENGE); }

#test "relay URLs: the hand canonical forms" {
  urls := [6] string.[RELAY_URL, CLIENT_URL, "wss://relay.example.com:0443//", "ws://relay.example.com", "wss://relay.example.com:8443", "wss://relay.example.com/nostr"];
  want := [6] string.["wss://relay.example.com", "wss://relay.example.com", "wss://relay.example.com", "ws://relay.example.com", "wss://relay.example.com:8443", "wss://relay.example.com/nostr"];
  for i: 0..<urls.count { out: [N42_MAX_URL] u8; u := nip42_url(urls[i], out[..]); #assert u.error == N42U_OK && cast(string) out[0..<u.count] == want[i]; }
  out: [N42_MAX_URL] u8;
  #assert nip42_url("wss://relay.example.com/?x=1", out[..]).error == N42U_PATH && nip42_url("https://relay.example.com", out[..]).error == N42U_SCHEME;
}
#test "the challenge frame and the template are the hand bytes" {
  c: [1] RelayConn;
  #assert relay_open(c[..], 1, CHALLENGE) == "[\"AUTH\",\"challengestringhere\"]";
  r: [1] Relay; n: [1] Nip42; g := rl_start(r[..], 0); ok := rl_opened(r[..], g, 0);
  #assert nip42_init(n[..], CLIENT_URL) == N42_OK && nip42_input(r[..], n[..], g, "[\"AUTH\",\"challengestringhere\"]") == RL_AUTH;
  tb: [512] u8; t := nip42_template(n[..], g, NOW, tb[..]);
  #assert t.error == N42_OK && t.created_at == 1793214821 && t.kind == 22242 && t.content == "" && t.tags == HAND_TAGS;
  #assert HAND_TAGS.count == 73 && N42_FX_TEMPLATE.count == 417;
}
#test "the client's signed answer is @noble/curves' event, byte for byte" #steps 400000000 {
  eb: [1024] u8;
  #assert sign(KEY_A, NOW, 22242, HAND_TAGS, eb[..]) == N42_FX_TEMPLATE;
}
#test "the time window: 600 s either way, inclusive, on the relay's clock" {
  c: [1] RelayConn; open_conn(c[..]);
  #assert judged(c[..], N42_FX_TEMPLATE, NOW - 600) == "N42_OK" && judged(c[..], N42_FX_TEMPLATE, NOW + 600) == "N42_OK";
  #assert judged(c[..], N42_FX_TEMPLATE, NOW - 601) == "N42_TIME" && judged(c[..], N42_FX_TEMPLATE, NOW + 601) == "N42_TIME";
}
#test "independently signed events get the replies worked out by hand" #steps 2000000000 {
  c: [1] RelayConn; open_conn(c[..]);
  // the challenge is recorded with the connection's generation
  #assert c[0].gen == 1 && c[0].n42[0].gen == 1 && nip42_challenge_text(c[0].n42[..]) == CHALLENGE;
  f: [1200] u8; w: [512] u8;
  // the spec's spelling of the relay URL, and another spelling with an extra tag
  #assert relay_auth(c[..], auth_frame(N42_FX_SPEC, f[..]), NOW) == ok_frame(id_of(N42_FX_SPEC), true, "", w[..]);
  #assert relay_auth(c[..], auth_frame(N42_FX_SPELLED, f[..]), NOW) == ok_frame(id_of(N42_FX_SPELLED), true, "", w[..]);
  #assert c[0].auth[0].n == 1;
  #assert relay_auth(c[..], auth_frame(N42_FX_KIND, f[..]), NOW) == ok_frame(id_of(N42_FX_KIND), false, "invalid: authentication event must have kind 22242", w[..]);
  #assert relay_auth(c[..], auth_frame(N42_FX_TWO_CHALLENGES, f[..]), NOW) ==
    ok_frame(id_of(N42_FX_TWO_CHALLENGES), false, "invalid: authentication event must have exactly one relay tag and one challenge tag", w[..]);
  #assert relay_auth(c[..], auth_frame(N42_FX_TEMPLATE, f[..]), NOW + 601) ==
    ok_frame(id_of(N42_FX_TEMPLATE), false, "invalid: the authentication event's timestamp is more than 10 minutes from the relay's clock", w[..]);
  // a new challenge, then another connection: the old answer no longer counts
  more := relay_open_again(c[..], "secondchallenge");
  #assert relay_auth(c[..], auth_frame(N42_FX_TEMPLATE, f[..]), NOW) == ok_frame(id_of(N42_FX_TEMPLATE), false, "invalid: authentication challenge does not match", w[..]);
  #assert c[0].auth[0].n == 1;
}
#test "a changed signature or id is refused before nip42.coax's check" #steps 2000000000 {
  c: [1] RelayConn; open_conn(c[..]);
  f: [1200] u8; w: [512] u8; x: [1024] u8;
  sig := edited(N42_FX_TEMPLATE, find(N42_FX_TEMPLATE, "\"sig\":\""), '0', x[..]);
  #assert relay_auth(c[..], auth_frame(sig, f[..]), NOW) == ok_frame(id_of(N42_FX_TEMPLATE), false, "invalid: bad signature", w[..]);
  at := find(N42_FX_TEMPLATE, "\"created_at\":") + 9;
  #assert N42_FX_TEMPLATE[at] == '1';
  id := edited(N42_FX_TEMPLATE, at, '0', x[..]);
  #assert relay_auth(c[..], auth_frame(id, f[..]), NOW) == ok_frame(id_of(N42_FX_TEMPLATE), false, "invalid: event id does not match its content", w[..]);
  #assert relay_auth(c[..], "[\"AUTH\",\"x\"]", NOW) == "[\"NOTICE\",\"invalid: malformed message\"]";
  #assert c[0].auth[0].n == 0;
}
#test "the client sends only an answer the relay would admit, once" {
  r: [1] Relay; n: [1] Nip42; g := rl_start(r[..], 0); ok := rl_opened(r[..], g, 0);
  #assert nip42_init(n[..], CLIENT_URL) == N42_OK && nip42_input(r[..], n[..], g, "[\"AUTH\",\"challengestringhere\"]") == RL_AUTH;
  s: [4096] u8; o: [1200] u8;
  #assert nip42_send(r[..], n[..], g, NOW, N42_FX_KIND, s[..], o[..], false).error == N42_KIND && r[0].auth == RA_CHALLENGED;
  #assert nip42_send(r[..], n[..], g, NOW + 601, N42_FX_TEMPLATE, s[..], o[..], false).error == N42_TIME;
  f := nip42_send(r[..], n[..], g, NOW, N42_FX_TEMPLATE, s[..], o[..], false);
  #assert f.error == N42_OK && f.written == 426 && r[0].auth == RA_SENT;
  #assert nip42_send(r[..], n[..], g, NOW, N42_FX_TEMPLATE, s[..], o[..], false).error == N42_MACHINE;
  dropped := rl_dropped(r[..], g, 0, 0); g2 := rl_start(r[..], 1000); ok = rl_opened(r[..], g2, 1000);
  #assert nip42_send(r[..], n[..], g2, NOW, N42_FX_TEMPLATE, s[..], o[..], false).error == N42_STALE;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/nip42-auth.coax --run-steps 250000000
```

On 2026-11-01 `coax test` ran 451 checks, including the loaded units' own
checks, and all passed in 36 s. The checks with signatures need larger step
budgets: signing one event takes about 400 million interpreter steps, and
each BIP-340 verification about 76 million.
`coax mutate nostr/examples/checks/nip42-auth.coax --root-only` killed 9 of 9
mutants in the checks' own procedures. A first run left one survivor: the
generation passed to `open_conn` was never observed, so the checks now
assert that the challenge is recorded with generation 1.

### The library's own checks

These files are unchanged by this chapter, and their records were not
rerun:

- [2026-10-29-nostr-nip42.md](../../docs/verification/2026-10-29-nostr-nip42.md):
  `nip42.coax`'s 48 checks (a 24-case URL table with a fixed-point check,
  every refusal path, the bounds 256/257 bytes of challenge, 4,096/4,097
  bytes of frame, ±600/±601 s, and the adapter through challenge, `AUTH`,
  `OK`, retry and reconnect); `coax mutate` killed 1,940 of 1,940 mutants.
  `nip42-event.coax`'s 214 checks, and
  `coax/tests/nostr/nip42_signed_checks.coax` (219 checks with real
  signatures: wrong kind, two challenges, two relays, no relay tag, short
  and long challenge tags, an old challenge, another relay, another
  generation and 601 s of drift).
- [2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md):
  `relay_route.coax`'s `rr_auth` checks (admitted keys only, each once, at
  most 8) and the signed-AUTH sessions over sockets in
  `coax/tests/nostr/relay_session.py`, signed with two test-only keys
  through `@noble/curves`.
- [2026-10-30-coax-relay-r1-tailnet-flood.md](../../docs/verification/2026-10-30-coax-relay-r1-tailnet-flood.md):
  the Psionic chapter 24 Pylon authenticated to the Coax relay over the
  tailnet, 52 of 52 end-to-end checks, before and after that change.

## Summary

- The relay sends `["AUTH", challenge]`; the connection's `Nip42` record
  keeps one challenge and its generation, and a new one replaces it.
- The client answers with kind 22242, tags `relay` (the canonical URL) and
  `challenge`, empty content and `created_at` now, signed by chapter 21's
  signer, and sends it only if the relay's own check would pass.
- The relay checks the id, the signature, the generation, the kind,
  `created_at` within 600 s, exactly one `relay` and one `challenge` tag,
  the canonical URL and the challenge, and answers `OK` with a reason.
- An admitted key opens writes on that connection and nothing else. The
  same event again on the same connection is accepted again; an old
  challenge, another connection or a late clock refuses it.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [29. A relay connection](ch029-a-relay-connection.md)
- [34. Admit an event](ch034-admit-an-event.md)

## Sources

- [NIP-42](../nips/official/42.md), Authentication of clients to relays.
- [NIP-01](../nips/official/01.md): the event id, `OK` and its prefixes.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 17.
