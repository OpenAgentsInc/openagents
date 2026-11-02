# 34. Admit an event

A relay decides which events it accepts. This chapter follows one `EVENT`
frame through the Coax relay's admission path, from the WebSocket frame to
the `OK` reply, with the limits of the Rust relay it replaces.

Part V, A relay. Result: The relay's EVENT admission: verify ID and
signature, check size, tag and time limits, answer `OK` with the standard
prefixes, and route each frame from one connection.

## What this chapter teaches

- The order of the checks: the WebSocket frame's length, the NIP-01 frame,
  the event's JSON and recomputed id, the relay's policy, the signature, and
  the store. Each check is cheaper than the next one.
- The limits: frames of 131,072 bytes, 256 tags, `created_at` at most 900 s
  ahead, no expired NIP-40 event, no kind 22242, and writes only after
  NIP-42 authentication.
- The reasons the relay sends: `invalid:`, `duplicate:`, `auth-required:`
  and `error:` on `OK`, and a `NOTICE` when the frame has no event id to
  answer. The R1 relay sends no `blocked:` or `rate-limited:`; rate limits
  are chapter 44's.
- `OK true` for a new event only after the store has taken it, and `OK true`
  with `duplicate:` for an event the relay already holds.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [relay_route.coax](../../coax/lib/nostr/relay_route.coax) | `rr_frame_read`, `rr_message`, `rr_writes_need_auth`, `rr_authed` | which handler a frame and a message go to; whether a write needs a NIP-42 key, and whether the connection has one (`#authority`) |
| [message_server.coax](../../coax/lib/nostr/message_server.coax) | `ms_read`, `ms_write_ok`, `ms_write_notice` | chapter 24's relay codec: the client frame in, the reply out |
| [relay_server.coax](../../coax/lib/nostr/relay_server.coax) | `rs_admit_read`, `rs_admit_policy`, `rs_admit_verified` | reads the event, recomputes its id and applies the policy; then admits only a verified result for the same id (`#authority`) |
| | `rs_prefix`, `rs_text`, `rs_accepted` | each reason's prefix, text and verdict |
| [store.coax](../../coax/lib/nostr/store.coax), [store_core.coax](../../coax/lib/nostr/store_core.coax) | `rsd_admit` | the in-memory store's verdict: new, duplicate, superseded or full (store_core is `#authority`) |
| [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_read`, `nostr_event_verify_vartime` | chapter 21's reader and BIP-340 verifier |
| [store_admit.coax](../../coax/lib/nostr/store_admit.coax) | `ns_admit_one` | exact-byte verification and single-event snapshot staging for the durable store (chapter 35) |
| [relay.coax](../../coax/relay/relay.coax) | `ry_message`, `ry_event` | the relay program: moves bytes and calls the procedures above |

`relay.coax` has a `main` and calls the host's sockets, so the example
can't load it. The example's `relay_message` repeats `ry_message` and
`ry_event` line for line, with the clock passed in and the reply returned
instead of written to a socket. Every decision it makes is a call into the
libraries above.

## The admission path

`ry_frames` assembles a text message from WebSocket frames (chapter 27)
and passes it to `ry_message`. The steps:

1. **The frame's length.** `wss_frame_read(bytes, RS_MAX_MESSAGE)` reads the
   frame header. A payload over 131,072 bytes refuses with
   `WS_FRAME_TOO_BIG` before any payload byte is read, and `rr_frame_read`
   closes the connection with code 1009. No `OK` is sent: the relay never
   saw an event.
2. **The NIP-01 frame.** `ms_read` checks that the text is one JSON array
   with a known verb and the right fields (chapter 24). For a refused
   message `rr_message` picks a `NOTICE` (or a `CLOSED` when a subscription
   id was read); a malformed `EVENT` gets
   `NOTICE "invalid: malformed message"`. An admitted `EVENT` goes to
   `ry_event`.
3. **The event and its id.** `rs_admit_read` calls `nostr_event_read`
   (chapter 21) on the event object's bytes. It refuses JSON that isn't an
   event, fields of the wrong form and, with `NE_ID`, an `id` that isn't the
   SHA-256 of the event's serialization. A malformed event gets
   `NOTICE "invalid: malformed event"`; an id mismatch gets an `OK false`
   that echoes the id the client sent. For an addressable kind it also
   reads the `d` tag, which must fit in 256 bytes.
4. **The policy.** `rs_admit_policy` checks, in this order: kind 22242 is
   refused (an AUTH event is never published), more than 256 tags, a
   `created_at` more than 900 s after the relay's clock, an expired NIP-40
   event (`expiration` at or before now), and a write from a connection
   with no NIP-42 key. The relay sets no lower bound on `created_at`.
5. **The signature.** Only an event that passed steps 3 and 4 is verified.
   `rs_admit_verified` takes `nostr_event_verify_vartime`'s result and
   admits it only when it is verified and has the same id as the event
   that was read.
6. **The store.** `rsd_admit` refuses anything not verified, then answers:
   new (stored), the same id again (`duplicate`), an older version of a
   replaceable or addressable event (`superseded`), or no room (`full`).
   An ephemeral kind (20000–29999) is accepted and not stored.
7. **The reply.** `ms_write_ok(id, rs_accepted(reason), rs_prefix(reason),
   rs_text(reason))`. With `--store FILE`, the program uses
   `rsd_admit_saved`, which appends the event to the file and syncs it
   before it stores it, so `OK true` follows the write.

The signature check is the slowest step: one BIP-340 verification is a
variable-time multi-scalar multiplication (Bitcoin chapter 23). Every check before
it reads bytes and compares numbers. So a flood of events that fail the
policy costs the relay no curve arithmetic, and an event that breaks two
rules is answered with the first one in this order.

### The reasons

| Reason | `OK` | Message |
| --- | --- | --- |
| `RS_ACCEPT` | `true` | `""` |
| `RS_DUP` | `true` | `duplicate: already have this event` |
| `RS_SUPERSEDED` | `true` | `duplicate: newer replaceable event already stored` |
| `RS_BAD_ID` | `false` | `invalid: event id does not match its content` |
| `RS_BAD_SIG` | `false` | `invalid: bad signature` |
| `RS_AUTH_KIND` | `false` | `invalid: authentication events cannot be published` |
| `RS_TAGS` | `false` | `invalid: event has too many tags` |
| `RS_TIME` | `false` | `invalid: the event's created_at timestamp is too far in the past or the future for this relay` |
| `RS_EXPIRED` | `false` | `invalid: event has expired` |
| `RS_AUTH_REQUIRED` | `false` | `auth-required: authenticate before publishing` |
| `RS_FULL` | `false` | `error: relay storage is full` |
| `RS_BAD_EVENT` | — | `NOTICE` `invalid: malformed event` |

NIP-01 lets a relay answer a duplicate with `true`: the client's event is
held, so there is nothing to resend. The texts are the Rust relay's where
it had one; the 2026-10-30 differential lists the texts that differ and
keeps them, since only the prefix is machine-readable.

### Worked example: one EVENT by hand

The event is a kind 1 note, `hello relay`, with no tags, created at the
relay's clock, 1793491200 (2026-11-01T00:00:00Z). It is signed with the
BIP-340 vector 1 test key, whose public key is `dff1d77f...502ba659`.

The frame is `["EVENT",` (9 bytes, 0..9), the event object, and `]`. The
object is these pieces:

| Piece | Bytes |
| --- | --- |
| `{"id":"` + 64 hex digits | 7 + 64 = 71 |
| `","pubkey":"` + 64 hex digits | 12 + 64 = 76 |
| `","created_at":1793491200` | 15 + 10 = 25 |
| `,"kind":1` | 8 + 1 = 9 |
| `,"tags":[]` | 8 + 2 = 10 |
| `,"content":"hello relay"` | 11 + 13 = 24 |
| `,"sig":"` + 128 hex digits + `"}` | 8 + 128 + 2 = 138 |

That is 353 bytes, so `ms_read` should report the object at bytes 9..362
and the frame is 9 + 353 + 1 = 363 bytes. A 363-byte frame is far below
131,072, so step 1 passes.

Step 3 writes the NIP-01 serialization `[0,pubkey,created_at,kind,tags,
content]`:

```text
[0,"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",1793491200,1,[],"hello relay"]
```

`[0,"` is 4 bytes, the key 64, `",` 2, the time 10, `,1,[],` 6, the quoted
content 13 and `]` 1: 100 bytes. Its SHA-256 must equal the `id` field.
Python's `hashlib` gives `0753e8ad...df8560b60`, the id the signer wrote.

Step 4, the policy, with now = 1793491200:

1. Kind 1 is not 22242.
2. 0 tags is at most 256.
3. 1793491200 is at most 1793491200 + 900 = 1793492100.
4. There is no `expiration` tag, so the event never expires.
5. The relay requires a NIP-42 key for writes. Before the connection
   authenticates, the event is refused here with `auth-required`. After it
   authenticates, the policy accepts.

Step 5 verifies the signature over the 32-byte id. Step 6 finds no record
with this id in an empty store and stores it. The reply is

```text
["OK","0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60",true,""]
```

`["OK","` is 7 bytes, the id 64 and `",true,""]` 10: 81 bytes. The same
event sent again reaches step 6, finds its id, and gets `true` with
`duplicate: already have this event`.

Change one input and a different step answers:

- `hello relay` edited to `hello Relay`: step 3 hashes a different
  serialization, so the id doesn't match. `OK false`, `invalid: event id
  does not match its content`, with the id the client sent.
- The last signature digit changed: steps 3 and 4 pass and step 5 fails,
  `invalid: bad signature`. This holds even after the real event is
  stored: the store only sees verified events, so the forged copy never
  reaches the duplicate check.
- `created_at` = 1793492101, one second past the limit: step 4 refuses with
  the `created_at` text. With a broken signature too, the answer is the
  same, because step 5 never runs.

## The example

The example is [nostr/examples/relay-admit.coax](../examples/relay-admit.coax).
It loads the store (which loads `relay_server.coax` and chapter 24's
codec), the routing unit and chapter 21's signer. It:

1. builds a WebSocket header that announces 131,072 bytes and one that
   announces 131,073, and shows that the first waits for its payload and
   the second gets close 1009;
2. walks the worked frame through each step and prints what each step
   returns;
3. replays a session of 19 frames on one connection and prints each reply.
   The store has room for five events, so the last new event finds it full.

Every event is signed with the BIP-340 vector 1 test key. The NIP-42
handshake is chapter 38's: the example records an admitted key with
`rr_auth(auth, RR_N42_OK, key)`, which is what `ry_auth` does after
`nip42_admit` accepts an AUTH event.

The admission step itself:

<!-- source: nostr/examples/relay-admit.coax admit -->
```jai
// One text message from a client, handled as coax/relay/relay.coax's
// ry_message and ry_event handle it, with the socket and the clock replaced:
// the reply the relay sends.
relay_message :: (r: [] Relay, frame: string, now: int) -> string {
  out := r[0].out[..];
  m := ms_read(frame);
  route := rr_message(m.error, m.type, m.has_sub);
  if route.action == RR_NOTICE { return reply_of(ms_write_notice(rs_prefix(route.reason), rs_text(route.reason), out), out); }
  if route.action != RR_EVENT { return "(not an EVENT: this example sends only EVENT)"; }
  event := frame[m.event_from..<m.event_to];
  // 1. Read the event and recompute its id; 2. the policy, cheapest first.
  a := rs_admit_read(event, now, rr_writes_need_auth(true), rr_authed(r[0].auth[..]), r[0].scratch[..]);
  if a.reason == RS_BAD_EVENT { return reply_of(ms_write_notice(rs_prefix(RS_BAD_EVENT), rs_text(RS_BAD_EVENT), out), out); }
  // 3. The signature, only for an event the policy accepted.
  if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(event, r[0].scratch[..], false)); }
  // 4. The store: new, duplicate, superseded or full.
  reason := a.reason;
  if reason == RS_ACCEPT { reason = rsd_admit(r[0].store[..], a, event); }
  id := a.e.id;
  return reply_of(ms_write_ok(id[..], rs_accepted(reason), rs_prefix(reason), rs_text(reason), out), out);
}
```

The whole program:

<!-- source: nostr/examples/relay-admit.coax example -->
```jai
#load "../../coax/lib/nostr/store.coax";
#load "../../coax/lib/nostr/relay_route.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relay's clock for the whole session: 2026-11-01T00:00:00Z.
NOW :: 1793491200;
// BIP-340 vector 1's published test key and aux. Never sign with a real key
// in an example.
KEY_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";

REASON :: [27] string.["accept", "duplicate", "superseded", "bad event", "bad id", "bad signature",
  "auth kind", "tags", "time", "expired", "auth required", "full", "subid", "filters", "subs",
  "empty", "hex e", "hex p", "cost", "filter", "sub space", "frame", "verb", "binary", "auth off",
  "count", "too big"];

// What the relay holds for one connection and for the whole relay.
Relay :: struct {
  auth: [1] RrAuth;                 // the connection's authenticated keys
  arena: [16384] u8; recs: [5] RsdRec;
  store: [1] RsdStore;              // room for five events
  scratch: [65536] u8;              // the id preimage
  out: [512] u8;                    // one reply
}
relay_init :: (r: [] Relay) { r[0].store[0].arena = r[0].arena[..]; r[0].store[0].recs = r[0].recs[..]; }

// book:begin admit
// One text message from a client, handled as coax/relay/relay.coax's
// ry_message and ry_event handle it, with the socket and the clock replaced:
// the reply the relay sends.
relay_message :: (r: [] Relay, frame: string, now: int) -> string {
  out := r[0].out[..];
  m := ms_read(frame);
  route := rr_message(m.error, m.type, m.has_sub);
  if route.action == RR_NOTICE { return reply_of(ms_write_notice(rs_prefix(route.reason), rs_text(route.reason), out), out); }
  if route.action != RR_EVENT { return "(not an EVENT: this example sends only EVENT)"; }
  event := frame[m.event_from..<m.event_to];
  // 1. Read the event and recompute its id; 2. the policy, cheapest first.
  a := rs_admit_read(event, now, rr_writes_need_auth(true), rr_authed(r[0].auth[..]), r[0].scratch[..]);
  if a.reason == RS_BAD_EVENT { return reply_of(ms_write_notice(rs_prefix(RS_BAD_EVENT), rs_text(RS_BAD_EVENT), out), out); }
  // 3. The signature, only for an event the policy accepted.
  if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(event, r[0].scratch[..], false)); }
  // 4. The store: new, duplicate, superseded or full.
  reason := a.reason;
  if reason == RS_ACCEPT { reason = rsd_admit(r[0].store[..], a, event); }
  id := a.e.id;
  return reply_of(ms_write_ok(id[..], rs_accepted(reason), rs_prefix(reason), rs_text(reason), out), out);
}
// book:end admit
reply_of :: (n: int, out: [] u8) -> string {
  #assert n > 0;
  return cast(string) out[0..<n];
}

// Sign one event with the test key into out; the event's text.
sign :: (created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(KEY_HEX, key[..], false).error == NC_OK;
  #assert hex_decode_checked(AUX_HEX, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [8192] u8;
  n := EVENT_MORE;
  while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false); }
  #assert n > 0;
  return cast(string) out[0..<n];
}
// ["EVENT",<event>] into out.
frame_of :: (event: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[\"EVENT\","); n = json_raw(out, n, event); n = json_raw(out, n, "]");
  return reply_of(n, out);
}
// text with the first occurrence of old replaced by new, into out.
replace :: (text: string, old: string, new: string, out: [] u8) -> string {
  at := -1;
  for i: 0..<text.count - old.count + 1 { if at < 0 && text[i..<i + old.count] == old { at = i; } }
  #assert at >= 0;
  n := json_raw(out, 0, text[0..<at]); n = json_raw(out, n, new); n = json_raw(out, n, text[at + old.count..<text.count]);
  return reply_of(n, out);
}
// The event with the last hex digit of its signature changed (the event
// ends with that digit, a quote and a brace).
break_sig :: (event: string, out: [] u8) -> string {
  for i: 0..<event.count { out[i] = event[i]; }
  at := event.count - 3;
  out[at] = ifx out[at] == '0' then '1' else '0';
  return cast(string) out[0..<event.count];
}
// count copies of ["t","x"] as a tags array, into out.
many_tags :: (count: int, out: [] u8) -> string {
  n := json_raw(out, 0, "[");
  for i: 0..<count { if i > 0 { n = json_raw(out, n, ","); } n = json_raw(out, n, "[\"t\",\"x\"]"); }
  return reply_of(json_raw(out, n, "]"), out);
}
// Send one EVENT frame and print the reply.
send :: (r: [] Relay, label: string, event: string) {
  f: [8192] u8;
  print("%\n  <- %\n", label, relay_message(r, frame_of(event, f[..]), NOW));
}
print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  h := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<h.written]);
}

main :: () {
  r: [1] Relay; relay_init(r[..]);
  b1: [1024] u8;
  e1 := sign(NOW, 1, "[]", "hello relay", b1[..]);

  print("the wire: a WebSocket frame's length is checked before its payload\n");
  // A masked text frame's header with the 8-byte length form: 0x81, 0xff,
  // the length big-endian, then a 4-byte mask key.
  for length: [2] int.[RS_MAX_MESSAGE, RS_MAX_MESSAGE + 1] {
    head: [14] u8; head[0] = 0x81; head[1] = 0xff;
    for k: 0..<8 { head[2 + k] = cast,trunc(u8) (length >> (8 * (7 - k))); }
    fr := rr_frame_read(wss_frame_read(cast(string) head[..], RS_MAX_MESSAGE).status);
    print("  payload % bytes: ", length);
    if fr.action == RR_F_WAIT { print("header admitted, waiting for the payload\n"); }
    if fr.action == RR_F_CLOSE {
      c: [4] u8; n := wss_close_write(fr.code, c[..]);
      print("refused, "); print_hex("close frame", c[0..<n]);
    }
  }

  print("\nworked frame, step by step\n");
  f1: [1024] u8; frame := frame_of(e1, f1[..]);
  print("frame (% bytes): %\n", frame.count, frame);
  m := ms_read(frame);
  print("ms_read: verb EVENT %, event object bytes %..%\n", m.type == MS_EVENT, m.event_from, m.event_to);
  event := frame[m.event_from..<m.event_to];
  e := nostr_event_read(event, r[0].scratch[..], false);
  print("preimage (% bytes): %\n", e.written, cast(string) r[0].scratch[0..<e.written]);
  print_hex("claimed id ", e.id[..]);
  print_hex("computed id", e.computed_id[..]);
  print("kind %, created_at %, tags %\n", e.kind, e.created_at, e.tags);
  print("policy, not authenticated: %\n", REASON[rs_admit_policy(e, -1, NOW, true, false)]);
  pub := e.public_key;
  #assert rr_auth(r[0].auth[..], RR_N42_OK, pub[..]) == RR_A_OK;   // as if NIP-42 AUTH had succeeded
  print("policy, authenticated: %\n", REASON[rs_admit_policy(e, -1, NOW, true, true)]);
  v := nostr_event_verify_vartime(event, r[0].scratch[..], false);
  print("signature verified: %\n", v.verified);
  rr_auth_reset(r[0].auth[..]);

  print("\nthe session\n");
  send(r[..], "1. the worked event before AUTH", e1);
  #assert rr_auth(r[0].auth[..], RR_N42_OK, pub[..]) == RR_A_OK;
  print("   (the connection authenticates with NIP-42; chapter 38)\n");
  send(r[..], "2. the worked event", e1);
  send(r[..], "3. the same event again", e1);
  b2: [1024] u8; send(r[..], "4. content changed after signing", replace(e1, "hello relay", "hello Relay", b2[..]));
  b3: [1024] u8; send(r[..], "5. last signature digit changed", break_sig(e1, b3[..]));
  b4: [1024] u8; send(r[..], "6. created_at now + 900", sign(NOW + 900, 1, "[]", "edge", b4[..]));
  b5: [1024] u8; future := sign(NOW + 901, 1, "[]", "early", b5[..]);
  send(r[..], "7. created_at now + 901", future);
  b6: [1024] u8; send(r[..], "8. now + 901 and a broken signature", break_sig(future, b6[..]));
  b7: [1024] u8; send(r[..], "9. kind 22242 (a NIP-42 AUTH event)", sign(NOW, 22242, "[[\"relay\",\"ws://127.0.0.1:7777\"],[\"challenge\",\"c\"]]", "", b7[..]));
  t1: [4096] u8; b8: [4096] u8; send(r[..], "10. 257 tags", sign(NOW, 1, many_tags(257, t1[..]), "", b8[..]));
  t2: [4096] u8; b9: [4096] u8; send(r[..], "11. 256 tags", sign(NOW, 1, many_tags(256, t2[..]), "", b9[..]));
  b10: [1024] u8; send(r[..], "12. expiration now (NIP-40)", sign(NOW - 60, 1, "[[\"expiration\",\"1793491200\"]]", "gone", b10[..]));
  b11: [1024] u8; send(r[..], "13. kind 20001, ephemeral", sign(NOW, 20001, "[]", "typing", b11[..]));
  b12: [1024] u8; send(r[..], "14. kind 0 profile", sign(NOW, 0, "[]", "{\"name\":\"new\"}", b12[..]));
  b13: [1024] u8; send(r[..], "15. kind 0, one second older", sign(NOW - 1, 0, "[]", "{\"name\":\"old\"}", b13[..]));
  b14: [1024] u8; send(r[..], "16. a fifth new event", sign(NOW, 1, "[]", "one more", b14[..]));
  b15: [1024] u8; send(r[..], "17. a sixth new event", sign(NOW, 1, "[]", "no room", b15[..]));
  print("18. [\"EVENT\",{}]\n  <- %\n", relay_message(r[..], "[\"EVENT\",{}]", NOW));
  print("19. [\"EVENT\",\"x\"]\n  <- %\n", relay_message(r[..], "[\"EVENT\",\"x\"]", NOW));
  print("stored: % events, % live\n", r[0].store[0].n, r[0].store[0].live);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/relay-admit.coax --run-steps 250000000
```

`--run-steps` gives the compile-time tables of the signer and verifier
(Bitcoin chapter 16's fixed-base table) the budget they need. The output:

```text
the wire: a WebSocket frame's length is checked before its payload
  payload 131072 bytes: header admitted, waiting for the payload
  payload 131073 bytes: refused, close frame: 880203f1

worked frame, step by step
frame (363 bytes): ["EVENT",{"id":"0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60","pubkey":"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659","created_at":1793491200,"kind":1,"tags":[],"content":"hello relay","sig":"025bc40033dfbca3ef4e8e50f83799d8ed3090bf79083ec74c32ad187cdddb178707c7dbde6224f0c4af344669b033b93b6c61dfa8f59927016decdc735f6d7a"}]
ms_read: verb EVENT true, event object bytes 9..362
preimage (100 bytes): [0,"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",1793491200,1,[],"hello relay"]
claimed id : 0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60
computed id: 0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60
kind 1, created_at 1793491200, tags 0
policy, not authenticated: auth required
policy, authenticated: accept
signature verified: true

the session
1. the worked event before AUTH
  <- ["OK","0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60",false,"auth-required: authenticate before publishing"]
   (the connection authenticates with NIP-42; chapter 38)
2. the worked event
  <- ["OK","0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60",true,""]
3. the same event again
  <- ["OK","0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60",true,"duplicate: already have this event"]
4. content changed after signing
  <- ["OK","0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60",false,"invalid: event id does not match its content"]
5. last signature digit changed
  <- ["OK","0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60",false,"invalid: bad signature"]
6. created_at now + 900
  <- ["OK","39c49b2b67c1d83c2b96d97b8da63bf25e5b7d74779eaa63c50087f3831615cb",true,""]
7. created_at now + 901
  <- ["OK","64cb2f8c3ec65ae79800cf23d3b7abf1301d7d15b55b82378e236381416f7173",false,"invalid: the event's created_at timestamp is too far in the past or the future for this relay"]
8. now + 901 and a broken signature
  <- ["OK","64cb2f8c3ec65ae79800cf23d3b7abf1301d7d15b55b82378e236381416f7173",false,"invalid: the event's created_at timestamp is too far in the past or the future for this relay"]
9. kind 22242 (a NIP-42 AUTH event)
  <- ["OK","b2cbcf494538d05be55b66d37484b2f47f7edd3e5a15c37e43a5486087ef6db9",false,"invalid: authentication events cannot be published"]
10. 257 tags
  <- ["OK","c5fecb919cddba11664f4adeeaf7fc83094ba099264b76eb9dc5f4437069f68a",false,"invalid: event has too many tags"]
11. 256 tags
  <- ["OK","eae890592b1a5566fcd14cb48ed9970646f5b42cdf30a96a5bca6756f5751844",true,""]
12. expiration now (NIP-40)
  <- ["OK","8f3e5212002db0bed22938b6303cef64560015027171e1c2ae5549ee9ab3a63a",false,"invalid: event has expired"]
13. kind 20001, ephemeral
  <- ["OK","f032419da3fccec2680604b493d9ea451bed31b941a51d169d34359510be5dde",true,""]
14. kind 0 profile
  <- ["OK","3df0dfbcbbf71ddb7b7008929de734d949e12b2981c8acd97912df09736ce5f7",true,""]
15. kind 0, one second older
  <- ["OK","548e2f52e61bd0eb04eed5fa68e61d0bb9c414262edd286152a2a29f6f1cdabb",true,"duplicate: newer replaceable event already stored"]
16. a fifth new event
  <- ["OK","dc296123dad01c9b89b97b6ff9556b10dd82cfa4279ef982f17174f41f5e3451",true,""]
17. a sixth new event
  <- ["OK","2f18b69b09b2a168f497ab8623fb7685dc04395d92a2b4c520d714cd5711b746",false,"error: relay storage is full"]
18. ["EVENT",{}]
  <- ["NOTICE","invalid: malformed event"]
19. ["EVENT","x"]
  <- ["NOTICE","invalid: malformed message"]
stored: 5 events, 5 live
```

The worked frame is 363 bytes with the object at 9..362, the preimage is
the 100 bytes worked out above, and the computed id equals the claimed one.
In the session:

- Frame 1 is refused at step 4 because the connection has no NIP-42 key.
  Frame 2, the same bytes after AUTH, is stored.
- Frames 4 and 5 echo the worked event's id: the client's claimed id is the
  only id the relay can name in a refusal.
- Frames 6 and 7 are the 900-second edge: 1793492100 is accepted and
  1793492101 is refused. Frame 8 shows that the policy answers before the
  signature is checked.
- Frames 10 and 11 are the tag limit: 257 is refused, 256 accepted.
- Frame 12's `expiration` equals now. NIP-40 says an expired event is
  refused, and `rs_live` treats `expiration` = now as expired.
- Frame 13 is ephemeral: `OK true`, and the store's count doesn't change.
- Frames 14 and 15 are kind 0 from the same key. The older one is
  superseded, and NIP-01 lets the relay say `true` with `duplicate:`.
- Frame 17 is the sixth new event for a store with five record slots:
  `error: relay storage is full`. The relay's store has 2,048 slots and a
  4 MiB arena.
- Frames 18 and 19 have no id to answer, so the relay sends `NOTICE`:
  `{}` fails at step 3, and `"x"` (not an object) at step 2.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-admit.coax](../examples/checks/relay-admit.coax),
which loads the example, so running the example doesn't run them:

- the worked event signed by Coax is byte for byte the event that
  `coax/tests/nostr/nip59_reference.py` writes and signs. That script is
  pure Python (BIP-340 from the BIP, `hashlib`, `json`) and shares no code
  with Coax. The event's public key is BIP-340 `test-vectors.csv` row 1's;
- the worked preimage is the 100 bytes written by hand above;
- the session replayed over Python-signed events, in a store of four slots,
  gets exactly the `OK` frames written by hand from Python's ids and the
  reasons table. The 256- and 257-tag events carry Python's ids and an
  all-zero signature: 257 is refused by the tag limit, and 256 passes the
  policy and is refused by the signature. Their verdicts and prefixes are
  the ones the Rust relay gave in the 2026-10-30 differential;
- a header announcing 131,072 bytes waits for its payload and one
  announcing 131,073 gets close 1009.

Python's signed events are constants at the top of the checks file, above
the listing.

<!-- source: nostr/examples/checks/relay-admit.coax check -->
```jai
// Python's SHA-256 ids of the 256- and 257-tag events (["t","x"] repeated,
// created_at NOW, kind 1, content "").
PY_TAGS256_ID :: "eae890592b1a5566fcd14cb48ed9970646f5b42cdf30a96a5bca6756f5751844";
PY_TAGS257_ID :: "c5fecb919cddba11664f4adeeaf7fc83094ba099264b76eb9dc5f4437069f68a";
// BIP-340 test-vectors.csv row 1: the public key of the example's secret key.
BIP340_PUB1 :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
ZERO_SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// An event with Python's id and an all-zero signature: the id matches, so
// only the policy or the signature can refuse it.
unsigned_tags :: (count: int, id: string, out: [] u8) -> string {
  t: [4096] u8;
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, id);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, BIP340_PUB1);
  n = json_raw(out, n, "\",\"created_at\":1793491200,\"kind\":1,\"tags\":"); n = json_raw(out, n, many_tags(count, t[..]));
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\""); n = json_raw(out, n, ZERO_SIG);
  return reply_of(json_raw(out, n, "\"}"), out);
}
// The OK frame worked out by hand from an id, a verdict and a message.
ok_frame :: (id: string, accepted: bool, message: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[\"OK\",\""); n = json_raw(out, n, id); n = json_raw(out, n, "\",");
  n = json_raw(out, n, ifx accepted then "true" else "false"); n = json_raw(out, n, ",\"");
  n = json_raw(out, n, message);
  return reply_of(json_raw(out, n, "\"]"), out);
}
expect_reply :: (r: [] Relay, event: string, id: string, accepted: bool, message: string) {
  f: [8192] u8; want: [512] u8;
  #assert relay_message(r, frame_of(event, f[..]), NOW) == ok_frame(id, accepted, message, want[..]);
}
TIME :: "invalid: the event's created_at timestamp is too far in the past or the future for this relay";

#test "the worked event signs to the Python signer's bytes" #steps 400000000 {
  b: [1024] u8;
  #assert sign(NOW, 1, "[]", "hello relay", b[..]) == PY_WORKED;
  s: [256] u8;
  e := nostr_event_read(PY_WORKED, s[..], false);
  pub := e.public_key; want: [32] u8;
  #assert hex_decode_checked(BIP340_PUB1, want[..], false).error == NC_OK && cast(string) pub[..] == cast(string) want[..];
}
// The preimage of the worked trace, written by hand from NIP-01.
#test "the worked event's preimage is the hand serialization" {
  s: [256] u8;
  e := nostr_event_read(PY_WORKED, s[..], false);
  #assert e.error == NE_OK && e.written == 100;
  #assert cast(string) s[0..<e.written] == "[0,\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",1793491200,1,[],\"hello relay\"]";
}
// The verdicts and prefixes are the Rust relay's, as the 2026-10-30
// differential recorded them (docs/verification/
// 2026-10-30-coax-relay-r1-rust-differential.md); the texts after the
// prefix are rs_text's.
#test "Python-signed events get the replies worked out by hand" #steps 2000000000 {
  r: [1] Relay; relay_init(r[..]);
  r[0].store[0].recs = r[0].recs[0..<4];   // room for four events
  w := "0753e8ad491d16259792761362658889e960a29e0829af2aefe981bdf8560b60";
  expect_reply(r[..], PY_WORKED, w, false, "auth-required: authenticate before publishing");
  pub: [32] u8; #assert hex_decode_checked(BIP340_PUB1, pub[..], false).error == NC_OK;
  #assert rr_auth(r[0].auth[..], RR_N42_OK, pub[..]) == RR_A_OK;
  expect_reply(r[..], PY_WORKED, w, true, "");
  expect_reply(r[..], PY_WORKED, w, true, "duplicate: already have this event");
  b: [1024] u8;
  expect_reply(r[..], replace(PY_WORKED, "hello relay", "hello Relay", b[..]), w, false, "invalid: event id does not match its content");
  expect_reply(r[..], break_sig(PY_WORKED, b[..]), w, false, "invalid: bad signature");
  expect_reply(r[..], PY_EDGE, "39c49b2b67c1d83c2b96d97b8da63bf25e5b7d74779eaa63c50087f3831615cb", true, "");
  fid := "64cb2f8c3ec65ae79800cf23d3b7abf1301d7d15b55b82378e236381416f7173";
  expect_reply(r[..], PY_FUTURE, fid, false, TIME);
  expect_reply(r[..], break_sig(PY_FUTURE, b[..]), fid, false, TIME);
  expect_reply(r[..], PY_AUTH, "b2cbcf494538d05be55b66d37484b2f47f7edd3e5a15c37e43a5486087ef6db9", false, "invalid: authentication events cannot be published");
  t: [4096] u8;
  expect_reply(r[..], unsigned_tags(257, PY_TAGS257_ID, t[..]), PY_TAGS257_ID, false, "invalid: event has too many tags");
  expect_reply(r[..], unsigned_tags(256, PY_TAGS256_ID, t[..]), PY_TAGS256_ID, false, "invalid: bad signature");
  expect_reply(r[..], PY_EXPIRED, "8f3e5212002db0bed22938b6303cef64560015027171e1c2ae5549ee9ab3a63a", false, "invalid: event has expired");
  expect_reply(r[..], PY_EPHEMERAL, "f032419da3fccec2680604b493d9ea451bed31b941a51d169d34359510be5dde", true, "");
  expect_reply(r[..], PY_PROFILE_NEW, "3df0dfbcbbf71ddb7b7008929de734d949e12b2981c8acd97912df09736ce5f7", true, "");
  expect_reply(r[..], PY_PROFILE_OLD, "548e2f52e61bd0eb04eed5fa68e61d0bb9c414262edd286152a2a29f6f1cdabb", true, "duplicate: newer replaceable event already stored");
  expect_reply(r[..], PY_FIFTH, "dc296123dad01c9b89b97b6ff9556b10dd82cfa4279ef982f17174f41f5e3451", true, "");
  expect_reply(r[..], PY_SIXTH, "2f18b69b09b2a168f497ab8623fb7685dc04395d92a2b4c520d714cd5711b746", false, "error: relay storage is full");
  #assert r[0].store[0].n == 4 && r[0].store[0].live == 4;
  #assert relay_message(r[..], "[\"EVENT\",{}]", NOW) == "[\"NOTICE\",\"invalid: malformed event\"]";
  #assert relay_message(r[..], "[\"EVENT\",\"x\"]", NOW) == "[\"NOTICE\",\"invalid: malformed message\"]";
}
// A WebSocket header announcing 131073 bytes is refused before the payload.
#test "frame lengths: 131072 waits for its payload, 131073 gets close 1009" {
  head: [14] u8; head[0] = 0x81; head[1] = 0xff; head[7] = 0x02;   // 0x020000 = 131072
  #assert rr_frame_read(wss_frame_read(cast(string) head[..], RS_MAX_MESSAGE).status).action == RR_F_WAIT;
  head[9] = 1;
  f := rr_frame_read(wss_frame_read(cast(string) head[..], RS_MAX_MESSAGE).status);
  #assert f.action == RR_F_CLOSE && f.code == 1009;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-admit.coax --run-steps 250000000
```

On 2026-11-01 `coax test` ran 467 checks, including the loaded units' own
checks, and all passed in 35 s. The session check takes 775 million steps
and 14 s in the interpreter, because it runs ten BIP-340 verifications.
`coax mutate nostr/examples/checks/relay-admit.coax --root-only` killed
15 of 15 mutants in the checks' own procedures. A first run left one
survivor: a signature of 128 `1` digits fails verification like one of
zeros, so the zero signature is now a constant.

### The library's own checks

`relay_server.coax`'s checks cover each refusal on fixtures with correct ids
and an all-f signature (`coax/tests/nostr/relay_server_fixtures.coax`), so
they never run a full verify: malformed JSON, a bad id, kind 22242, created_at
1901 against a clock of 1000 (900 is the edge), an expiration of 1000 and
1001, 257 and 256 tags, AUTH required and given, `d` tags, and the signature
step taking only a verified result with the same id. `store.coax` and
`store_core.coax` check verified-only admission, duplicates, replacement on
tied timestamps and the bounds.

These records were not rerun for this chapter, because the files are
unchanged since (the only later change to `relay_route.coax` is a comment,
`eaa10e85e9`):

- [2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md):
  `coax mutate` on `relay_server.coax`'s whole graph killed 6,087 of 6,087
  mutants (files at `2c69195ee8`), and `message_server.coax` 1,179 of 1,179.
- [2026-10-29-nostr-store-r1.md](../../docs/verification/2026-10-29-nostr-store-r1.md):
  `store_core.coax`, 590 of 590.
- [2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md):
  `relay_route.coax`, 255 of 255.

### Against the Rust relay

The same record holds the differential: the Rust relay built from
`0b401561a0^` in a sandbox and the Coax relay, built with ASan and UBSan,
given the same 114 observations. 88 matched exactly, 15 had the same
verdict and prefix with another text, 7 were intentional and 4 were left
for the owner, with 0 unexplained. For admission, these matched exactly:
valid, duplicate, created_at 1000 s ahead (refused) and 800 s ahead
(accepted), expiring later, kind 22242, 257 and 256 tags, and ephemeral.
The bad id, bad signature and expired texts differ; a write before AUTH is
an intentional difference (the Rust relay ran without `auth_required`). The
sessions over sockets, `relay_session.py` (72 of 72) and the wss:// run in
[2026-10-30-coax-relay-r1-tls.md](../../docs/verification/2026-10-30-coax-relay-r1-tls.md)
(85 of 85), were not rerun.

## Summary

- An `EVENT` passes six steps in order: the WebSocket frame's length
  (131,072 bytes), the NIP-01 frame, the event's JSON and recomputed id,
  the policy, the signature, and the store.
- The policy refuses kind 22242, more than 256 tags, `created_at` more than
  900 s ahead, an expired event, and a write without a NIP-42 key.
- The signature is verified only after every cheaper check passes, and the
  first failing step names the reason.
- `OK true` means stored, a duplicate or a superseded version; `OK false`
  carries `invalid:`, `auth-required:` or `error:`. A frame with no event
  id gets a `NOTICE`, and an oversized frame closes the connection with
  1009.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [24. Relay messages](ch024-relay-messages.md)
- [27. A WebSocket server](ch027-a-websocket-server.md)
- [Coax chapter 24, A complete
  boundary](../../coax/book/ch24-a-complete-boundary.md)

## Sources

- [NIP-01](../nips/official/01.md): events, the id, `OK` and its prefixes,
  and replaceable, ephemeral and addressable kinds.
- [NIP-40](../nips/official/40.md), Expiration Timestamp.
- [NIP-42](../nips/official/42.md), Authentication of clients to relays:
  kind 22242 and `auth-required:`.
- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) section 7.4.1: close
  code 1009, message too big.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 17.
