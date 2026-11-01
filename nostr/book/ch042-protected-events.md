# 42. Protected events

NIP-70 lets an author say that an event belongs only on relays they chose.
The author adds the tag `["-"]`. A relay that honors it accepts the event
only from a connection that has proved, with NIP-42, that it holds the
event's key. Anyone else who tries to republish the event to that relay is
refused. This chapter reads the tag, builds the relay's admission rule and
its `auth-required:` and `restricted:` answers, and gives the client a
helper that adds the tag.

Part IX, A relay. Result: Events tagged `["-"]` accepted only from their
authenticated author, so a third party cannot republish them to this relay.

## What this chapter teaches

- The tag `["-"]`, and how a client adds it before it signs.
- The relay's rule: refuse a protected event unless the connection
  authenticated as its `pubkey`.
- The refusals in order: `auth-required:` for a connection with no key,
  then `restricted:` for a connection whose keys belong to others.
- Why a relay that takes no protected events, NIP-70's default, refuses
  them outright, and why a repost that embeds one is refused too.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip70.coax](../../coax/lib/nostr/nip70.coax) | `nip70_tagged(e)` | Whether a read event carries the `-` tag |
| | `nip70_decide(tagged, accepts, authed, as_author)` | The rule as a table of four facts |
| | `nip70_admit(text, accepts, auth, scratch)` | A relay's admission: read the event, find the tag, check reposts, apply the rule to the connection's keys |
| | `nip70_prefix(d)`, `nip70_text(d)`, `nip70_reason(d, out)` | The `OK` message's reason |
| | `nip70_protect(tags, out)` | The client's side: append `["-"]` to a tags array |
| [nip70_fixtures.coax](../../coax/tests/nostr/nip70_fixtures.coax) | | Unsigned events with computed IDs and connections with chosen keys, for the library's checks |

`nip70.coax` decides admission, so it is marked `#authority`. It builds on
the event reader of [chapter 21](ch021-events-ids-and-signatures.md)
(`nostr_event_read`), the tag walker of
[chapter 22](ch022-kinds-and-tags.md) (`nt_start`), and the per-connection key
list of [chapter 38](ch038-nip-42-authentication.md) (`RrAuth`, with
`rr_authed` and `rr_authed_as` from `relay_route.coax`). It adds no JSON,
hash or signature code of its own.

<!-- source: coax/lib/nostr/nip70.coax codes -->
```jai
// nip70_tagged's answers.
N70_PLAIN :: 0; N70_TAGGED :: 1; N70_EVENT :: 2;
// Admission decisions.
N70_ACCEPT :: 0;          // not protected, or authenticated as the author
N70_AUTH_REQUIRED :: 1;   // protected, and the connection has no key
N70_RESTRICTED :: 2;      // protected, and the connection's keys are others'
N70_CLOSED :: 3;          // protected, and this relay takes none
N70_REPOST :: 4;          // a repost that embeds a protected event
N70_INVALID :: 5;         // the event doesn't read or its ID doesn't match
N70_DECISIONS :: 6;
// nip70_protect's errors.
N70P_OK :: 0; N70P_TAGS :: 1; N70P_OUTPUT :: 2;

N70Admit :: struct { decision:int=N70_INVALID; tagged:bool; id:[32]u8; author:[32]u8; }
N70Tags :: struct { error:int=N70P_TAGS; n:int; }
```

## The tag

NIP-70 defines the tag as "a simple tag with a single item: `["-"]`". It
may be added to any event. An event that carries it is *protected*.

`nip70_tagged` walks the tags and looks at the first element of each. A
tag whose name is `-` marks the event. The name is compared after JSON
unescaping, so `["-"]` counts. A tag `["-", "x"]` with more values
than NIP-70 defines also counts. A relay that read it as unprotected could
spread an event its author meant to keep, so the reading errs toward
refusal. A `-` in a value position, as in `["t", "-"]`, doesn't count.

<!-- source: coax/lib/nostr/nip70.coax tagged -->
```jai
// Whether the event carries the "-" tag. Tags were checked by the event
// reader as arrays of strings; a forged record that breaks that is
// N70_EVENT.
nip70_tagged :: (e:NostrEvent)->int {
  j:[1]Json;if !nt_start(j[..],e) {return N70_EVENT;}
  found:=false;tok:=json_next(j[..]);
  while tok==JSON_ARRAY {
    v:=json_next(j[..]);
    if v==JSON_STRING && json_is(json_text(j[..]),"-") {found=true;}
    while v==JSON_STRING {v=json_next(j[..]);}
    if v!=JSON_ARRAY_END {return N70_EVENT;}
    tok=json_next(j[..]);
  }
  if tok!=JSON_ARRAY_END {return N70_EVENT;}
  return ifx found then N70_TAGGED else N70_PLAIN;
}
```

The tag is part of the signed tags, so it is covered by the event ID and
the signature. A third party can't remove it from someone else's event
without changing the ID, and the changed event no longer verifies.

### The client's side

A client adds the tag before it signs. `nip70_protect` takes a tags array
and appends `["-"]`. It leaves the array unchanged when a `-` tag is
already there, so calling it twice gives the same tags. It refuses tags
that are not an array of arrays of strings, the same shape the event reader
accepts.

<!-- source: coax/lib/nostr/nip70.coax protect -->
```jai
// The client's side: tags (a JSON array of arrays of strings) with ["-"]
// appended, into out; unchanged, but for whitespace around it, when a "-"
// tag is there.
nip70_protect :: (tags:string,out:[]u8)->N70Tags {
  r:N70Tags;j:[1]Json;json_start(j[..],tags);first:=json_next(j[..]);
  span:=ne_tags(j[..]);
  if !span.ok || json_next(j[..])!=JSON_END {return r;}
  body:=tags[span.from..<span.to];
  t:=nip70_tagged(nt_event(body));
  n:=json_raw(out,0,"[[\"-\"]]");
  if t==N70_TAGGED {n=json_raw(out,0,body);}
  else if span.count>0 {n=json_raw(out,json_raw(out,0,body[0..<body.count-1]),",[\"-\"]]");}
  if n== -1 {r.error=N70P_OUTPUT;return r;}
  r.error=N70P_OK;r.n=n;return r;
}
```

## The rule

NIP-70 gives the relay two choices. "The default behavior of a relay MUST be
to reject any event that contains `["-"]`." A relay that wants to accept
such events "MUST first require that the client perform the NIP-42 `AUTH`
flow and then check if the authenticated client has the same pubkey as the
event being published and only accept the event in that case."

The rule needs four facts:

1. `tagged`: the event carries `["-"]`.
2. `accepts`: this relay takes protected events at all.
3. `authed`: the connection has authenticated at least one key.
4. `as_author`: one of the connection's keys is the event's `pubkey`.

A connection can hold several keys (chapter 38 keeps up to eight), so
`as_author` asks whether any of them matches, not whether the first one
does.

<!-- source: coax/lib/nostr/nip70.coax decide -->
```jai
// The rule. tagged: the event has the "-" tag. accepts: this relay takes
// protected events at all. authed: the connection has authenticated some
// key. as_author: one of its keys is the event's pubkey.
nip70_decide :: (tagged:bool,accepts:bool,authed:bool,as_author:bool)->int {
  if !tagged {return N70_ACCEPT;}
  if !accepts {return N70_CLOSED;}
  if !authed {return N70_AUTH_REQUIRED;}
  if !as_author {return N70_RESTRICTED;}
  return N70_ACCEPT;
}
```

An unprotected event is accepted by this rule; the relay's other policies
still apply. A protected event on a relay that takes none is `N70_CLOSED`,
whatever the connection's keys. On a relay that takes them, a connection
with no key gets `auth-required:`, and one whose keys are all others' gets
`restricted:`.

The order of the last two follows NIP-42's prefixes. `auth-required:`
means the client has not authenticated and should. `restricted:` means it
has authenticated and the key it used is not allowed. A client that sees
`auth-required:` can answer the challenge and send the event again. A
client authenticated as the wrong key is not asked again.

### Worked example: the decision table by hand

Take NIP-70's event, signed by key 1 (public key `79be…1798`), and a
relay that takes protected events. Key 3 is someone else. Work out each
row from the four facts before the program runs.

| Protected | Keys on the connection | `authed` | `as_author` | Decision |
| --- | --- | --- | --- | --- |
| no | none | no | no | accept |
| no | key 3 | yes | no | accept |
| no | key 1 | yes | yes | accept |
| no | keys 1 and 3 | yes | yes | accept |
| yes | none | no | no | `auth-required: this event may only be published by its author` |
| yes | key 3 | yes | no | `restricted: this event may only be published by its author` |
| yes | key 1 | yes | yes | accept |
| yes | keys 1 and 3 | yes | yes | accept |

On a relay that takes no protected events, the last four rows all become
`restricted: this relay does not accept protected events`. Asking for
`AUTH` there would be pointless: no key would be accepted.

The `auth-required:` text is NIP-70's own, from its example flow. The
other texts are this relay's; NIP-01 fixes only the prefixes.

## Admission

`nip70_admit` takes the event's JSON text and the connection's keys. It
reads the event with `nostr_event_read`, which recomputes the ID from the
fields and refuses a mismatch as `invalid:`. It verifies no signature. A
relay checks the signature first: a `pubkey` compared with the
connection's keys means something only once the signature holds.

<!-- source: coax/lib/nostr/nip70.coax admit -->
```jai
// A relay's NIP-70 admission of one EVENT on a connection whose
// authenticated keys are auth. scratch holds the event's serialization and
// then a repost's unescaped content. The signature is not verified here.
nip70_admit :: (text:string,accepts:bool,auth:[]RrAuth,scratch:[]u8)->N70Admit {
  a:N70Admit;
  e:=nostr_event_read(text,scratch,false);if e.error!=NE_OK {return a;}
  // The reader checked the tags, so nip70_tagged can't answer N70_EVENT.
  a.id=e.id;a.author=e.public_key;a.tagged=nip70_tagged(e)==N70_TAGGED;
  if n70_embeds(e,scratch) {a.decision=N70_REPOST;return a;}
  key:=e.public_key;
  a.decision=nip70_decide(a.tagged,accepts,rr_authed(auth),rr_authed_as(auth,key[..]));
  return a;
}
// A kind 6 or 16 repost whose content reads as a protected event. The
// content is shorter unescaped than in the serialization, so it fits in
// scratch.
n70_embeds :: (e:NostrEvent,scratch:[]u8)->bool {
  if e.kind!=6 && e.kind!=16 {return false;}
  n:=json_unescape(e.original[e.content_from..<e.content_to],scratch);
  #assert n>=0;
  inner:=ne_parse(cast(string)scratch[0..<n]);
  return inner.error==NE_OK && nip70_tagged(inner)==N70_TAGGED;
}
```

### Reposts

NIP-70 adds: "Reposts of protected events MUST NOT embed the reposted
event. If a repost of a protected event embeds the event anyway, relays
SHOULD summarily reject it." A NIP-18 repost
([chapter 52](ch052-reposts-reactions-and-comments.md)) is kind 6 for a note and kind
16 for anything else, and its content may hold the reposted event's JSON.

`n70_embeds` unescapes a kind 6 or 16 event's content and reads it as an
event's fields with `ne_parse`. If that event carries `["-"]`, the repost
refuses as `N70_REPOST`, before the rule runs and whatever the connection's
keys. "Summarily" means the author's own repost refuses too. The embedded
event's ID is not checked: a repost that embeds a protected event with a
wrong ID still spreads the protected content. A repost with empty content
names its target by ID only and is not affected.

### The answers

<!-- source: coax/lib/nostr/nip70.coax reason -->
```jai
// The OK message's machine-readable prefix and its text.
nip70_prefix :: (d:int)->string {
  if d==N70_ACCEPT {return "";}
  if d==N70_AUTH_REQUIRED {return "auth-required";}
  if d==N70_RESTRICTED || d==N70_CLOSED || d==N70_REPOST {return "restricted";}
  if d==N70_INVALID {return "invalid";}
  return "error";
}
nip70_text :: (d:int)->string {
  if d==N70_ACCEPT {return "";}
  if d==N70_AUTH_REQUIRED || d==N70_RESTRICTED {return "this event may only be published by its author";}
  if d==N70_CLOSED {return "this relay does not accept protected events";}
  if d==N70_REPOST {return "a repost may not embed a protected event";}
  if d==N70_INVALID {return "the event is malformed or its id does not match";}
  return "unknown decision";
}
// "prefix: text" into out: its length, 0 for an accepted event, or -1 when
// out is too short.
nip70_reason :: (d:int,out:[]u8)->int {
  if d==N70_ACCEPT {return 0;}
  n:=json_raw(out,0,nip70_prefix(d));n=json_raw(out,n,": ");
  return json_raw(out,n,nip70_text(d));
}
```

| Decision | `OK` | Reason |
| --- | --- | --- |
| `N70_ACCEPT` | `true` | `""` |
| `N70_AUTH_REQUIRED` | `false` | `auth-required: this event may only be published by its author` |
| `N70_RESTRICTED` | `false` | `restricted: this event may only be published by its author` |
| `N70_CLOSED` | `false` | `restricted: this relay does not accept protected events` |
| `N70_REPOST` | `false` | `restricted: a repost may not embed a protected event` |
| `N70_INVALID` | `false` | `invalid: the event is malformed or its id does not match` |

## NIP-70's example event

NIP-70's example flow shows a kind 1 event by `79be…1798` with the tag
`["-"]` and the content `hello members of the secret group`. Its claimed
ID is `cb8feca5…f6f2`. [Chapter 21](ch021-events-ids-and-signatures.md)
found that the ID of its fields is `f017727a…b2e6`, so a relay refuses it
as `invalid:` before NIP-70 is asked. The example shows that refusal.

The public key `79be…1798` is the x coordinate of secp256k1's generator,
so its secret key is 1. The example signs the same fields with secret key
1 and gets the ID `f017727a…b2e6`, which matches chapter 21's computed
ID and Python's. That event verifies, and the rest of the example uses it.
Key 1 and BIP-340's vector 0 key (3) are test keys: anyone can sign with
them. Never use them for real events.

## The example

The example is [nostr/examples/protected-events.coax](../examples/protected-events.coax).
It runs offline: the relay and the client are procedures in one program,
and frames pass between them as strings. Each connection is a
`RelayConn`, with chapter 38's `Nip42` record and `RrAuth` key list. Its
AUTH frames are made by chapter 38's client code (`nip42_input`,
`nip42_template`, `nip42_send`) and admitted by its relay code
(`nip42_admit`, `rr_auth`).

The example adds the tag with `nip70_protect`, refuses NIP-70's published
event, re-signs its fields with key 1, prints the decision table, and plays
NIP-70's flow: the event refused with `auth-required:`, `AUTH` as key 1,
the event accepted. A second connection authenticated as key 3 then tries
to republish key 1's event, a relay that takes no protected events refuses
it even from key 1, and three broken events are refused.

<!-- source: nostr/examples/protected-events.coax example -->
```jai
#load "../../coax/lib/nostr/relay_server.coax";
#load "../../coax/lib/nostr/nip42-event.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/nip70.coax";
#import "Basic";

// The relay's clock and the client's (2026-10-28T19:13:41Z), as in
// chapter 38's example.
NOW :: 1793214821;
RELAY_URL :: "wss://relay.example.com/";
CLIENT_URL :: "WSS://Relay.Example.COM:443";
// NIP-70's example event, compacted. Its claimed id is not the id of its
// fields (chapter 21), so a relay refuses it before NIP-70 is asked.
NIP70 :: "{\"id\":\"cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2\",\"pubkey\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"created_at\":1707409439,\"kind\":1,\"tags\":[[\"-\"]],\"content\":\"hello members of the secret group\",\"sig\":\"fa163f5cfb75d77d9b6269011872ee22b34fb48d23251e9879bb1e4ccbdd8aaaf4b6dc5f5084a65ef42c52fbcde8f3178bac3ba207de827ec513a6aa39fa684c\"}";
NIP70_CREATED :: 1707409439;
NIP70_CONTENT :: "hello members of the secret group";
// Test keys, never real ones. Key 1 is the scalar 1, whose public key 79be..
// is NIP-70's example author. Key 3 is BIP-340 test vector 0's.
KEY_1 :: "0000000000000000000000000000000000000000000000000000000000000001";
KEY_3 :: "0000000000000000000000000000000000000000000000000000000000000003";
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";

text_of :: (n: int, out: [] u8) -> string { #assert n > 0; return cast(string) out[0..<n]; }
hex_of :: (bytes: [32] u8, out: [] u8) -> string {
  b := bytes; #assert hex_encode_checked(cast(string) b[..], out, false).error == NC_OK;
  return cast(string) out[0..<64];
}
// Signs one event with a test key into out.
sign :: (key_hex: string, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(key_hex, key[..], false).error == NC_OK;
  #assert hex_decode_checked(AUX, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [1024] u8; k := EVENT_MORE;
  while k == EVENT_MORE {
    k = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  return text_of(k, out);
}
// ["EVENT",<event>] or ["AUTH",<event>], the frame a client sends.
frame_of :: (verb: string, event: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[\""); n = json_raw(out, n, verb); n = json_raw(out, n, "\",");
  n = json_raw(out, n, event); return text_of(json_raw(out, n, "]"), out);
}

// What the relay holds for one connection.
RelayConn :: struct {
  gen: int;
  accepts: bool;            // this relay takes protected events
  n42: [1] Nip42;           // the relay URL and this connection's challenge
  auth: [1] RrAuth;         // the keys admitted on this connection
  scratch: [65536] u8;
  out: [1024] u8;
}
// A new connection: the challenge frame it sends.
relay_open :: (c: [] RelayConn, gen: int, accepts: bool, challenge: string) -> string {
  c[0].gen = gen; c[0].accepts = accepts; rr_auth_reset(c[0].auth[..]);
  #assert nip42_init(c[0].n42[..], RELAY_URL) == N42_OK;
  f := nip42_issue(c[0].n42[..], gen, challenge, c[0].out[..]);
  #assert f.error == N42_OK;
  return text_of(f.written, c[0].out[..]);
}
// One AUTH frame, as chapter 38's relay_auth handles it: the OK it sends.
relay_auth :: (c: [] RelayConn, frame: string) -> string {
  out := c[0].out[..];
  m := ms_read(frame); #assert m.error == MS_OK && m.type == MS_AUTH;
  obj := frame[m.event_from..<m.event_to];
  a := nip42_admit(c[0].n42[..], c[0].gen, NOW, obj, c[0].scratch[..], false);
  key := a.public_key;
  k := rr_auth(c[0].auth[..], a.error, key[..]);
  e := nostr_event_read(obj, c[0].scratch[..], false); id := e.id;
  return text_of(ms_write_ok(id[..], k == RR_A_OK, rr_auth_prefix(k), rr_auth_text(k), out), out);
}

// book:begin relay
// One EVENT frame: the id and signature first, then NIP-70's rule against
// this connection's authenticated keys. The OK the relay sends. (The store
// and the relay's other policies are chapter 34's and are left out.)
relay_event :: (c: [] RelayConn, frame: string) -> string {
  out := c[0].out[..];
  m := ms_read(frame); #assert m.error == MS_OK && m.type == MS_EVENT;
  obj := frame[m.event_from..<m.event_to];
  e := nostr_event_verify_vartime(obj, c[0].scratch[..], false); id := e.id;
  if e.error == NE_SIGNATURE { return text_of(ms_write_ok(id[..], false, "invalid", "bad signature", out), out); }
  if e.error != NE_OK {
    return text_of(ms_write_ok(id[..], false, nip70_prefix(N70_INVALID), nip70_text(N70_INVALID), out), out);
  }
  a := nip70_admit(obj, c[0].accepts, c[0].auth[..], c[0].scratch[..]);
  return text_of(ms_write_ok(id[..], a.decision == N70_ACCEPT, nip70_prefix(a.decision), nip70_text(a.decision), out), out);
}
// book:end relay

// The client's side of chapter 38: read the challenge frame, sign the
// kind 22242 answer with key_hex, and send it. The AUTH frame, in out.
client_auth :: (challenge_frame: string, key_hex: string, out: [] u8) -> string {
  r: [1] Relay; n: [1] Nip42;
  g := rl_start(r[..], 0); opened := rl_opened(r[..], g, 0);
  #assert nip42_init(n[..], CLIENT_URL) == N42_OK && nip42_input(r[..], n[..], g, challenge_frame) == RL_AUTH;
  tb: [512] u8; t := nip42_template(n[..], g, NOW, tb[..]); #assert t.error == N42_OK;
  eb: [1024] u8; event := sign(key_hex, t.created_at, t.kind, t.tags, t.content, eb[..]);
  sb: [4096] u8;
  f := nip42_send(r[..], n[..], g, NOW, event, sb[..], out, false); #assert f.error == N42_OK;
  return text_of(f.written, out);
}
keys_of :: (c: [] RelayConn) -> int { return c[0].auth[0].n; }
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }

main :: () {
  print("== 1. The tag, added by the client\n");
  pb: [256] u8;
  ins := [3] string.["[]", "[[\"t\",\"club\"]]", "[[\"-\"]]"];
  for t: ins {
    p := nip70_protect(t, pb[..]);
    print("  nip70_protect % -> %\n", t, cast(string) pb[0..<p.n]);
  }
  print("  nip70_protect [[\"t\",1]] -> error % (N70P_TAGS is %)\n", nip70_protect("[[\"t\",1]]", pb[..]).error, N70P_TAGS);

  print("\n== 2. NIP-70's example event\n");
  sc: [4096] u8; hx: [64] u8;
  pub := nostr_event_read(NIP70, sc[..], false);
  print("  claimed id  cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2\n");
  print("  computed id %\n", hex_of(pub.computed_id, hx[..]));
  print("  nostr_event_read: % (NE_ID is %)\n", pub.error, NE_ID);
  rb: [128] u8; none: [1] RrAuth;
  print("  nip70_admit: %\n", text_of(nip70_reason(nip70_admit(NIP70, true, none[..], sc[..]).decision, rb[..]), rb[..]));
  tb: [64] u8; t := nip70_protect("[]", tb[..]);
  eb: [1024] u8; event := sign(KEY_1, NIP70_CREATED, 1, cast(string) tb[0..<t.n], NIP70_CONTENT, eb[..]);
  print("the same fields signed with test key 1:\n  %\n", event);
  v := nostr_event_verify_vartime(event, sc[..], false);
  print("  verifies: %, id %\n", yes(v.error == NE_OK && v.verified), hex_of(v.id, hx[..]));

  print("\n== 3. The rule, worked as a table (relay takes protected events)\n");
  print("  protected  keys on the connection  decision\n");
  rows := [4] string.["none        ", "key 3 only  ", "key 1 only  ", "keys 1 and 3"];
  for p: 0..<2 {
    for r: 0..<4 {
      d := nip70_decide(p == 1, true, r > 0, r >= 2);
      print("  %        %            %\n", ifx p == 1 then "yes" else "no ", rows[r], ifx d == N70_ACCEPT then "accept" else text_of(nip70_reason(d, rb[..]), rb[..]));
    }
  }

  print("\n== 4. NIP-70's flow, with chapter 38's AUTH\n");
  c1: [1] RelayConn; cf: [128] u8; ef: [1200] u8; af: [1200] u8;
  challenge := frame_of_copy(relay_open(c1[..], 1, true, "challengestringhere"), cf[..]);
  ev := frame_of("EVENT", event, ef[..]);
  print("relay sends %\n", challenge);
  print("client sends key 1's protected event\nrelay answers %\n", relay_event(c1[..], ev));
  print("client authenticates as key 1\nrelay answers %\n", relay_auth(c1[..], client_auth(challenge, KEY_1, af[..])));
  print("client sends the event again\nrelay answers %\n", relay_event(c1[..], ev));

  print("\n== 5. A third party republishes it\n");
  c2: [1] RelayConn; cf2: [128] u8; af2: [1200] u8;
  ch2 := frame_of_copy(relay_open(c2[..], 2, true, "second"), cf2[..]);
  print("connection 2, not authenticated: %\n", relay_event(c2[..], ev));
  print("connection 2 authenticates as key 3: %\n", relay_auth(c2[..], client_auth(ch2, KEY_3, af2[..])));
  print("connection 2 (key 3) sends key 1's event: %\n", relay_event(c2[..], ev));
  nb: [1024] u8; nf: [1200] u8;
  note := sign(KEY_3, NOW, 1, "[]", "an unprotected note", nb[..]);
  print("connection 2 sends key 3's unprotected note: %\n", relay_event(c2[..], frame_of("EVENT", note, nf[..])));
  qb: [2048] u8; qf: [2200] u8;
  rt := json_raw(qb[..], 0, "[[\"e\",\""); rt = json_raw(qb[..], rt, hex_of(v.id, hx[..])); rt = json_raw(qb[..], rt, "\",\"wss://relay.example.com\"]]");
  rp := sign(KEY_3, NOW, 6, text_of(rt, qb[..]), event, qf[..]);
  rf: [4096] u8;
  print("connection 2 sends a kind 6 repost embedding it: %\n", relay_event(c2[..], frame_of("EVENT", rp, rf[..])));

  print("\n== 6. A relay that takes no protected events (NIP-70's default)\n");
  c3: [1] RelayConn; cf3: [128] u8; af3: [1200] u8;
  ch3 := frame_of_copy(relay_open(c3[..], 3, false, "third"), cf3[..]);
  print("not authenticated: %\n", relay_event(c3[..], ev));
  print("authenticates as key 1: %\n", relay_auth(c3[..], client_auth(ch3, KEY_1, af3[..])));
  print("key 1 sends its event: %\n", relay_event(c3[..], ev));

  print("\n== 7. Broken events\n");
  xb: [1024] u8; xf: [1200] u8;
  at := find(event, "\"content\":\"");
  bad := edited(event, at, 'j', xb[..]);
  print("content edited after signing, on connection 1 (key 1): %\n", relay_event(c1[..], frame_of("EVENT", bad, xf[..])));
  yb: [1024] u8; yf: [1200] u8;
  s := find(event, "\"sig\":\"");
  badsig := edited(event, s, ifx event[s] == '0' then cast(u8) '1' else cast(u8) '0', yb[..]);
  print("first signature digit changed: %\n", relay_event(c1[..], frame_of("EVENT", badsig, yf[..])));
  print("NIP-70's own example on connection 1: %\n", relay_event(c1[..], frame_of("EVENT", NIP70, yf[..])));
  print("keys on connections 1, 2, 3: %, %, %\n", keys_of(c1[..]), keys_of(c2[..]), keys_of(c3[..]));
}
frame_of_copy :: (s: string, out: [] u8) -> string {
  for i: 0..<s.count { out[i] = s[i]; }
  return cast(string) out[0..<s.count];
}
// A copy of s with the byte at `at` replaced by b.
edited :: (s: string, at: int, b: u8, out: [] u8) -> string {
  for i: 0..<s.count { out[i] = s[i]; }
  out[at] = b;
  return cast(string) out[0..<s.count];
}
// The index just after the first occurrence of field in s.
find :: (s: string, field: string) -> int {
  for i: 0..<s.count - field.count + 1 { if s[i..<i + field.count] == field { return i + field.count; } }
  return -1;
}
```

The relay's handling of one `EVENT`:

<!-- source: nostr/examples/protected-events.coax relay -->
```jai
// One EVENT frame: the id and signature first, then NIP-70's rule against
// this connection's authenticated keys. The OK the relay sends. (The store
// and the relay's other policies are chapter 34's and are left out.)
relay_event :: (c: [] RelayConn, frame: string) -> string {
  out := c[0].out[..];
  m := ms_read(frame); #assert m.error == MS_OK && m.type == MS_EVENT;
  obj := frame[m.event_from..<m.event_to];
  e := nostr_event_verify_vartime(obj, c[0].scratch[..], false); id := e.id;
  if e.error == NE_SIGNATURE { return text_of(ms_write_ok(id[..], false, "invalid", "bad signature", out), out); }
  if e.error != NE_OK {
    return text_of(ms_write_ok(id[..], false, nip70_prefix(N70_INVALID), nip70_text(N70_INVALID), out), out);
  }
  a := nip70_admit(obj, c[0].accepts, c[0].auth[..], c[0].scratch[..]);
  return text_of(ms_write_ok(id[..], a.decision == N70_ACCEPT, nip70_prefix(a.decision), nip70_text(a.decision), out), out);
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/protected-events.coax --run-steps 250000000
```

Its output, the same as [the fixture](../fixtures/protected-events.out):

```text
== 1. The tag, added by the client
  nip70_protect [] -> [["-"]]
  nip70_protect [["t","club"]] -> [["t","club"],["-"]]
  nip70_protect [["-"]] -> [["-"]]
  nip70_protect [["t",1]] -> error 1 (N70P_TAGS is 1)

== 2. NIP-70's example event
  claimed id  cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2
  computed id f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6
  nostr_event_read: 7 (NE_ID is 7)
  nip70_admit: invalid: the event is malformed or its id does not match
the same fields signed with test key 1:
  {"id":"f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6","pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":1707409439,"kind":1,"tags":[["-"]],"content":"hello members of the secret group","sig":"05a6cc87a4e03cd93efd843d45ec927b13dedcaf653518c0dc7e929f2cd3588587ea4402009afaa9d4dfff5c7e74a8e15aee9cd71c2aaadf04ff3e7e690a3e9d"}
  verifies: yes, id f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6

== 3. The rule, worked as a table (relay takes protected events)
  protected  keys on the connection  decision
  no         none                    accept
  no         key 3 only              accept
  no         key 1 only              accept
  no         keys 1 and 3            accept
  yes        none                    auth-required: this event may only be published by its author
  yes        key 3 only              restricted: this event may only be published by its author
  yes        key 1 only              accept
  yes        keys 1 and 3            accept

== 4. NIP-70's flow, with chapter 38's AUTH
relay sends ["AUTH","challengestringhere"]
client sends key 1's protected event
relay answers ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"auth-required: this event may only be published by its author"]
client authenticates as key 1
relay answers ["OK","1690e70a99b967a5c6d37c6465bc02086863a52b237e90fecd69d9ea0e4efe02",true,""]
client sends the event again
relay answers ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",true,""]

== 5. A third party republishes it
connection 2, not authenticated: ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"auth-required: this event may only be published by its author"]
connection 2 authenticates as key 3: ["OK","5bdb19ba2d89db06b83bb687e03d51c9693a0530b2c8945c25b1ea74ce6c2935",true,""]
connection 2 (key 3) sends key 1's event: ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"restricted: this event may only be published by its author"]
connection 2 sends key 3's unprotected note: ["OK","9d53f10f4be635ebeca2a5724acd9a0e03709d6666cc086ce8917a27c014b333",true,""]
connection 2 sends a kind 6 repost embedding it: ["OK","3ad060b2bb0993e92fba09b5a18893fa29cf99551869ea11a6d9e7be70083430",false,"restricted: a repost may not embed a protected event"]

== 6. A relay that takes no protected events (NIP-70's default)
not authenticated: ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"restricted: this relay does not accept protected events"]
authenticates as key 1: ["OK","1cfe1aa93fb441c3ca680be516ae8ca5d3e33f55f5bffdb5d72068acb3903764",true,""]
key 1 sends its event: ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"restricted: this relay does not accept protected events"]

== 7. Broken events
content edited after signing, on connection 1 (key 1): ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"invalid: the event is malformed or its id does not match"]
first signature digit changed: ["OK","f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6",false,"invalid: bad signature"]
NIP-70's own example on connection 1: ["OK","cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2",false,"invalid: the event is malformed or its id does not match"]
keys on connections 1, 2, 3: 1, 1, 1
```

The broken cases:

- `content edited after signing` changes `hello` to `jello` in key 1's
  signed event. The fields now hash to `245f78c8…be0d`, not the claimed
  `f017727a…b2e6`, so the reader refuses it before NIP-70 is asked, even on
  connection 1, which is authenticated as key 1.
- `first signature digit changed` keeps the ID but breaks the signature.
  The relay's signature check refuses it with `invalid: bad signature`.
- NIP-70's own example refuses as `invalid:` because its ID is wrong.

Each refusal is an `OK false` frame; the program's exit status is 0.

## How the code is checked

### Checks in the library

`nip70.coax` holds its own checks, written before the code: the tag
anywhere in the tags, escaped, with extra values, and names and values that
are not it; forged tag records; the rule over all 16 combinations of its
four facts against the table worked by hand; admission of a protected and
an unprotected event on connections with no key, key 1, key 2 and both, on
relays that do and don't take protected events; bad IDs, unreadable events
and a short scratch buffer; kind 6 and 16 reposts that embed a protected
event (also the author's own, and a protected repost), and contents that
don't (an unprotected event, empty content, a tags-shaped string, an event
with a short ID, and kinds 1, 5, 7 and 17 quoting a protected event); every
reason; and `nip70_protect` on empty, filled, already protected,
whitespace-padded and malformed tags and on short outputs.

```shell
coax/bin/coax test coax/lib/nostr/nip70.coax
coax/bin/coax mutate coax/lib/nostr/nip70.coax --root-only
```

On 2026-11-01, `coax test` ran 221 checks, including the loaded files'
own, in about 13 seconds. Root-only mutation killed 133 of 133 mutants in
about 26 seconds. The first run left 11 survivors. Eight were in the check
helpers, which moved to
[nip70_fixtures.coax](../../coax/tests/nostr/nip70_fixtures.coax) as
chapter 56's fixtures did. Three were guards that could never fire (a tag
error after the reader had checked the tags, an unescape that can't
overflow the scratch, an output length that can't be 0); they became
simpler code or an `#assert`.

### Checks in the example

The example's checks compare the code with values computed outside Coax
and with text copied from NIP-70.

<!-- source: nostr/examples/checks/protected-events.coax check -->
```jai
// IDs that Python's hashlib computed from
// json.dumps([0, pubkey, created_at, kind, tags, content], separators=(",", ":"),
// ensure_ascii=False). They share no code with Coax.
PY_NIP70_ID :: "f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6";   // NIP-70's fields
PY_EDITED_ID :: "245f78c85d0ccc8f00b3e457a22574d48df04ef0245860df77000bf68662be0d";  // "jello members ..."
PY_PLAIN_ID :: "73eddcdd8eb8050a00786252d6c8dd8025682768b77e732ad08d42251e8f46d9";   // the same with tags []
// NIP-70's claimed id, and the relay's answer in its example flow, copied
// from nostr/nips/official/70.md.
NIP70_CLAIMED :: "cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2";
NIP70_ANSWER :: "auth-required: this event may only be published by its author";
// secp256k1's generator x coordinate (SEC 2, section 2.4.1): key 1's
// public key.
SEC2_GX :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

same_id :: (id: [32] u8, text: string) -> bool {
  want: [32] u8; #assert hex_decode_checked(text, want[..], false).error == NC_OK;
  for i: 0..<32 { if id[i] != want[i] { return false; } }
  return true;
}
signed_note :: (tags: string, content: string, out: [] u8) -> string {
  return sign(KEY_1, NIP70_CREATED, 1, tags, content, out);
}

#test "NIP-70's example: its fields hash to Python's id, not the one it claims" {
  s: [4096] u8; e := nostr_event_read(NIP70, s[..], false);
  #assert e.error == NE_ID && same_id(e.computed_id, PY_NIP70_ID) && same_id(e.id, NIP70_CLAIMED);
  #assert !same_id(e.computed_id, NIP70_CLAIMED);
}
#test "key 1 re-signs NIP-70's fields: Python's id, G's x as pubkey, a valid signature" #steps 400000000 {
  b: [1024] u8; s: [4096] u8;
  e := nostr_event_verify_vartime(signed_note("[[\"-\"]]", NIP70_CONTENT, b[..]), s[..], false);
  #assert e.error == NE_OK && e.verified && same_id(e.id, PY_NIP70_ID) && same_id(e.public_key, SEC2_GX);
  p := nostr_event_read(signed_note("[]", NIP70_CONTENT, b[..]), s[..], false);
  #assert p.error == NE_OK && same_id(p.id, PY_PLAIN_ID);
  j := nostr_event_read(signed_note("[[\"-\"]]", "jello members of the secret group", b[..]), s[..], false);
  #assert j.error == NE_OK && same_id(j.id, PY_EDITED_ID);
}
#test "the relay's first answer is NIP-70's text, byte for byte" {
  out: [128] u8; n := nip70_reason(N70_AUTH_REQUIRED, out[..]);
  #assert n == NIP70_ANSWER.count && cast(string) out[0..<n] == NIP70_ANSWER;
}
// The table worked by hand in the chapter: for each connection (no key,
// key 3, key 1) the OK reason for key 1's protected note, then for its
// unprotected note. "" is acceptance.
HAND :: [6] string.[
  "auth-required: this event may only be published by its author", "",
  "restricted: this event may only be published by its author", "",
  "", ""];
// The reason text inside the OK frame the example's relay sends: the last
// string, which ends just before the closing "].
reason_of :: (frame: string) -> string {
  at := frame.count - 2; #assert frame[at] == '"';
  from := at - 1; while frame[from] != '"' { from -= 1; }
  return frame[from + 1..<at];
}
#forall "the example's relay answers as the hand table says" i: 0..<3 #steps 2000000000 {
  c: [1] RelayConn; cf: [128] u8; af: [1200] u8;
  ch := frame_of_copy(relay_open(c[..], 10 + i, true, "x"), cf[..]);
  if i == 1 { r := relay_auth(c[..], client_auth(ch, KEY_3, af[..])); }
  if i == 2 { r := relay_auth(c[..], client_auth(ch, KEY_1, af[..])); }
  #assert keys_of(c[..]) == ifx i == 0 then 0 else 1;
  b: [1024] u8; f: [1200] u8;
  #assert reason_of(relay_event(c[..], frame_of("EVENT", signed_note("[[\"-\"]]", NIP70_CONTENT, b[..]), f[..]))) == HAND[2 * i];
  #assert reason_of(relay_event(c[..], frame_of("EVENT", signed_note("[]", NIP70_CONTENT, b[..]), f[..]))) == HAND[2 * i + 1];
}
#test "reason_of reads the last string of an OK frame" {
  #assert reason_of("[\"OK\",\"ab\",false,\"x: y\"]") == "x: y" && reason_of("[\"OK\",\"ab\",true,\"\"]") == "";
}
#test "same_id notices a change in the first or last byte" {
  id: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert same_id(id, zero);
  id[0] = 1; #assert !same_id(id, zero);
  id[0] = 0; id[31] = 1; #assert !same_id(id, zero);
}
```

- Python's `hashlib` gives the ID of NIP-70's fields, `f017727a…b2e6`. The
  reader computes the same and refuses the claimed `cb8feca5…f6f2`.
- Key 1's re-signed event has Python's ID, its public key is secp256k1's
  generator x from SEC 2, and chapter 11's verifier accepts its signature.
  The unprotected and the edited variants also have Python's IDs.
- The relay's first answer equals the text in NIP-70's example flow, byte
  for byte.
- On connections with no key, key 3 and key 1, the example's relay gives
  the answers of the hand table for key 1's protected and unprotected
  notes, with AUTH done by chapter 38's client and relay code.

```shell
coax/bin/coax test nostr/examples/checks/protected-events.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/protected-events.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 478 checks in about 37 seconds. Root-only mutation killed 25 of 25 mutants in the checks file; the first run's three survivors were a backward search that never ran, now an `#assert`.
Mutation of the example's own `main` was not run; its output is pinned by
the fixture.

### What is not built

- `coax/relay/relay.coax` calls `nip70_admit` with `accepts` true and the
  connection's `RrAuth`, after the signature check and the rate limit. What
  is not built there: a fresh `["AUTH", challenge]` with an
  `auth-required:` answer. The relay already asks for a key before any
  write, so a protected event never reaches `nip70_admit` without one.
- A setting for `accepts`. The example passes it per connection; a relay
  would read it from its configuration and could advertise it in NIP-11
  ([chapter 37](ch037-nip-11-relay-information.md)).
- Checking reposts more deeply. The rule reads an embedded event's tags
  only; it doesn't follow an `e` tag to a stored protected event when the
  content is empty, which NIP-70 doesn't ask for.

## Summary

- An event is protected when it carries `["-"]`. The tag is signed, so
  nobody else can remove it.
- A relay takes no protected events by default. One that takes them
  accepts a protected event only from a connection authenticated as its
  `pubkey`.
- The refusals come in order: `auth-required:` for a connection with no
  key, `restricted:` for one whose keys are others'.
- A repost that embeds a protected event is refused, even from its author.
- NIP-70's published example has the wrong ID; signed again with secret
  key 1, its fields give `f017727a…b2e6`.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)

## Sources

- [NIP-70](../nips/official/70.md), pinned, with its example flow.
- [NIP-42](../nips/official/42.md) for `AUTH` and the `auth-required:` and
  `restricted:` prefixes.
- [NIP-18](../nips/official/18.md) for reposts.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
