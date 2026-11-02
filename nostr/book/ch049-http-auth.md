# 49. HTTP auth

NIP-98 signs one HTTP request with a Nostr key. The client puts a signed
kind 27235 event, base64 encoded, in the `Authorization` header. The server
checks that the event names this exact URL, method and body, that it is
recent, that its signature verifies, and that its ID has not been used
before. OpenAgents uses it for API calls and for sign-in. This chapter
builds both sides on chapter 21's events and Bitcoin chapter 4's SHA-256.

Part VI, Every NIP. Result: NIP-98 kind 27235 events in `Authorization: Nostr
<base64>`, binding the exact URL, method and payload hash within 60 seconds,
with single-use event IDs.

## What this chapter teaches

- Tags: `u` (the absolute URL, query included), `method`, and an optional
  `payload` (the SHA-256 of the body, as lowercase hex).
- The header: `Nostr `, then the event's JSON in base64.
- Server checks: the event reads and verifies, kind 27235, `created_at`
  within 60 seconds of now, the exact URL, the method, and the payload hash.
- Replay: each event ID is accepted once. A refused request does not use up
  its ID.
- A valid NIP-98 event proves that the signer made this one request. It
  grants nothing else: accounts and rights are the route's decision.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip98.coax](../../coax/web/api/nip98.coax) | `nip98_check_coax(authorization, method, url, body, now, work, out)` | The API's whole NIP-98 check: base64, the strict event reader, the NIP-01 ID, BIP-340, then the NIP-98 rules (`#authority`). On success `out` holds the ID, the pubkey and the tags |
| | `n98_parse`, `n98_id`, `n98_rules`, `nip98_error_text` | The reader, the ID check, the rules after the signature, and the words for each refusal code |
| [nostr.c](../../coax/web/api/nostr.c) | `nip98_check` | The C shim that the API's routes call; it passes the request to `nip98_check_coax` |
| [private.c](../../coax/web/api/private.c), [drops.c](../../coax/web/api/drops.c), [admin.c](../../coax/web/api/admin.c) | `first_use` | Replay: inserts the event ID into a Postgres table and accepts only when the row is new |
| [weblogin.coax](../../coax/lib/nostr/weblogin.coax) | `weblogin_sign(hex, url, created_at, body, aux, out)` | The website's client side: a signed `POST` login with `u`, `method` and `payload`, as `Nostr <base64>` |
| [nip98-tags.coax](../../coax/lib/nostr/nip98-tags.coax), [nip98-observe.coax](../../coax/lib/nostr/nip98-observe.coax) | `n98_source_tags`, `n98_observe` | A stricter tag reader and field comparison (one `u`, one `method`, at most one `payload`), compared with the API but not used by it |

`nip98.coax` is a port of the API's former C check, decision for decision.
It replaced that C in the API on 2026-10-31. The example in this chapter
calls `nip98_check_coax` directly.

## The event

A NIP-98 event is an ordinary chapter 21 event:

```jai
kind:       27235
content:    ""
tags:       ["u", "<absolute URL>"]
            ["method", "<HTTP method>"]
            ["payload", "<SHA-256 of the body, lowercase hex>"]   (optional)
created_at: the time the client signs it
```

NIP-98 says a client SHOULD send `payload` when the request has a body,
and a server MAY check it. `nip98_check_coax` always checks a `payload` tag
that is present. Without one, the body is not bound: any body passes. The
API's writing routes require the tag; for example, `private.c` refuses a
signed write whose event has no `payload`.

## The header

The `Authorization` value is the scheme `Nostr`, one space, and the base64
of the event's JSON bytes. Base64 writes every 3 bytes as 4 characters
(Bitcoin chapter 1). A final group of 1 or 2 bytes becomes 4 characters with 2 or 1
`=`. So `n` event bytes give `4·⌈n/3⌉` characters with padding.

The reader in `nip98.coax` takes the standard and the URL-safe alphabets,
and padding is optional: NIP-98's own example header has none.

## The server's checks

`nip98_check_coax` runs these steps in order. The first failure refuses,
with one code.

1. The value starts with `Nostr ` and the rest is base64 (`no Nostr
   authorization`, `authorization is not base64`).
2. The JSON is one event object that the strict reader accepts: known
   fields only, each once, 16 tags of at most 4 fields, 1,023-byte fields,
   8,191-byte content, a 12,287-byte event. Each reader failure has its own
   code (`bad id`, `unknown field`, `missing a field`, and others).
3. The `id` is the SHA-256 of the NIP-01 serialization (`id does not match
   the event`).
4. The `pubkey` is a curve point and the signature verifies (`pubkey is not
   a point`, `signature does not verify`).
5. `kind` is 27235 (`not a kind 27235 event`).
6. `now - 60 ≤ created_at ≤ now + 60` (`created_at is not within 60 s`).
7. The `u` tag equals the request URL byte for byte (`the u tag is not this
   URL`).
8. The `method` tag equals the request method, with ASCII letters
   compared without case (`the method tag is not this method`).
9. If there is a `payload` tag, it equals the hex SHA-256 of the body
   (`the payload tag is not this body`).

NIP-98 lists kind, time, URL and method as its four checks. This code
verifies the event first, because an event whose signature fails proves
nothing. If a tag appears twice, the last one counts, as in the former C.

### Replay

A valid header can be copied and sent again within its window. The server
stops that by accepting each event ID once. `first_use` in the API runs

```jai
INSERT INTO projects.seen (id, ms) VALUES ($1, ...) ON CONFLICT DO NOTHING
```

and accepts only when one row was inserted. The table is shared by every
API worker and survives restarts, so another worker, or the same one after
a restart, still refuses the copy.

The order matters. The server records an ID only after every NIP-98 check
passes, so a request refused for a wrong method does not use up its ID.

The time window bounds the store. An event passes the time check only while
`now ≤ created_at + 60`, so after that its ID can be deleted: a copy would
fail the time check anyway.

The example keeps the IDs in an eight-entry list instead of Postgres. When
the list is full, it refuses with 503 rather than forget an ID.

## Worked example: one header by hand

The client signs `POST https://example.com/api/notes?limit=2` with the body
`{"a":1}` at `created_at = 1790000000`. The key is BIP-340 test vector 1's
published secret key, as in chapter 21, so the pubkey is
`dff1d77f...502ba659`. Never sign with a real key in an example.

### Step 1: the tags

The URL is 37 bytes: `https://` (8), `example.com` (11), `/api` (4),
`/notes` (6) and `?limit=2` (8). The body's SHA-256 is
`015abd7f...6a97f862`, 64 hex digits. The tag array is

```jai
[["u","https://example.com/api/notes?limit=2"],["method","POST"],["payload","015abd7f...6a97f862"]]
```

| Part | Bytes |
| --- | ---: |
| `[["u","` + URL + `"],` | 7 + 37 + 3 = 47 |
| `["method","POST"],` | 18 |
| `["payload","` + hash + `"]` | 12 + 64 + 2 = 78 |
| `]` | 1 |
| Total | 144 |

### Step 2: the ID

The NIP-01 preimage (chapter 21) is
`[0,"<pubkey>",1790000000,27235,<tags>,""]`:
`[0,"` (4), the pubkey (64), `",` (2), the time (10), `,` (1), the kind (5),
`,` (1), the tags (144) and `,""]` (4), 235 bytes. Its SHA-256 is the ID,
`572b6da4...a9566104`.

### Step 3: the event's length

| Part | Bytes |
| --- | ---: |
| `{"id":"` + ID + `",` | 7 + 64 + 2 = 73 |
| `"pubkey":"` + pubkey + `",` | 10 + 64 + 2 = 76 |
| `"created_at":1790000000,` | 24 |
| `"kind":27235,` | 13 |
| `"tags":` + tags + `,` | 7 + 144 + 1 = 152 |
| `"content":"",` | 13 |
| `"sig":"` + signature + `"}` | 7 + 128 + 2 = 137 |
| Total | 488 |

### Step 4: the header's length

`488 = 3·162 + 2`, so base64 writes 162 full groups and one group of 2
bytes with one `=`: `163·4 = 652` characters. With `Nostr ` (6), the
header value is 658 bytes.

NIP-98's example event is 412 bytes. `412 = 3·137 + 1`, so it takes
`137·4 + 2 = 550` characters without padding, and its header value is 556
bytes.

### Step 5: what the server decides

The server's clock reads `now = 1790000030`. The time check is
`1789999970 ≤ 1790000000 ≤ 1790000090`, which holds. The URL, the method and
the hash match, and the ID is new, so the answer is 200 and the ID goes in
the list. The same header sent again passes every NIP-98 check, but its ID
is in the list, so the answer is 401.

## The example

The example is [nostr/examples/http-auth.coax](../examples/http-auth.coax).
It builds the header of the worked example and serves it twice. It signs a
second event one second later and sends it with one field wrong at a time,
then accepts and replays it. Last, it checks NIP-98's own example header.

<!-- source: nostr/examples/http-auth.coax example -->
```jai
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/base64.coax";
#load "../../coax/web/api/nip98.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
// Append text to out at n; the new length.
put :: (out: [] u8, at: int, text: string) -> int {
  n := at;
  for c: text { out[n] = c; n += 1; }
  return n;
}

// The client: SHA-256 of the body as hex, into hex (64 bytes).
body_hash :: (body: string, hex: [] u8) {
  h: [32] u8; b: [64] u8;
  for i: 0..<body.count { b[i] = body[i]; }
  #assert sha256_hash_checked(b[0..<body.count], h[..], false) == SHA_OK;
  r := hex_encode_checked(cast(string) h[..], hex, false);
  #assert r.written == 64;
}
// The client: the tags for url and method, with a payload tag when the
// body is not empty, written into out.
auth_tags :: (url: string, method: string, body: string, out: [] u8) -> string {
  n := put(out, 0, "[[\"u\",\""); n = put(out, n, url);
  n = put(out, n, "\"],[\"method\",\""); n = put(out, n, method); n = put(out, n, "\"]");
  if body.count > 0 {
    hex: [64] u8; body_hash(body, hex[..]);
    n = put(out, n, ",[\"payload\",\""); n = put(out, n, cast(string) hex[..]); n = put(out, n, "\"]");
  }
  n = put(out, n, "]");
  return cast(string) out[0..<n];
}
// The client: "Nostr <base64 of event>" into header.
encode_header :: (event: string, header: [] u8) -> string {
  h := put(header, 0, "Nostr ");
  e := base64_encode_checked(event, header[h..<header.count], false);
  #assert e.error == NC_OK;
  return cast(string) header[0..<h + e.written];
}
// The client: sign an event of this kind with empty content into ev.
sign :: (key: [] u8, aux: [] u8, created_at: int, kind: int, tags: string, ev: [] u8) -> string {
  work: [1] NostrSignWork; scratch: [1024] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux, created_at, kind, tags, "", work[..], scratch[..], ev, false);
  }
  #assert n > 0;
  return cast(string) ev[0..<n];
}
// event with the byte at position at replaced by c, into out.
edit :: (event: string, at: int, c: u8, out: [] u8) -> string {
  for i: 0..<event.count { out[i] = event[i]; }
  out[at] = c;
  return cast(string) out[0..<event.count];
}

// The server: the event IDs already used inside the window. A real server
// keeps them in a shared store (the API's first_use inserts into Postgres);
// this list is the same rule in memory. A full list refuses: the server
// never forgets an ID to make room.
Seen :: struct { ids: [8] [64] u8; count: int; }
REPLAYED :: 100; FULL :: 101;
// The server's whole decision for one request: the NIP-98 check, then
// first use of the event ID. N98_OK, a N98_ refusal, REPLAYED or FULL;
// on N98_OK, out holds the id, the pubkey and the tags (nip98.coax).
decide :: (seen: [] Seen, header: string, method: string, url: string, body: string,
           now: int, out: [] u8) -> Nip98 {
  work: [N98_WORK] u8; b: [1024] u8;
  for i: 0..<body.count { b[i] = body[i]; }
  r := nip98_check_coax(header, method, url, b[0..<body.count], now, work[..], out);
  if r.error != N98_OK { return r; }
  for k: 0..<seen[0].count {
    if cast(string) seen[0].ids[k][..] == cast(string) out[0..<64] { r.error = REPLAYED; return r; }
  }
  if seen[0].count == 8 { r.error = FULL; return r; }
  for i: 0..<64 { seen[0].ids[seen[0].count][i] = out[i]; }
  seen[0].count += 1;
  return r;
}
// decide, and print the HTTP status and why.
serve :: (label: string, seen: [] Seen, header: string, method: string, url: string,
          body: string, now: int) {
  out: [N98_OUT] u8;
  r := decide(seen, header, method, url, body, now, out[..]);
  if r.error == REPLAYED { print("%: 401, event id already used\n", label); return; }
  if r.error == FULL { print("%: 503, too many event ids in the window\n", label); return; }
  if r.error != N98_OK { print("%: 401, %\n", label, nip98_error_text(r.error)); return; }
  print("%: 200, pubkey %..., u %, method %, payload %\n", label,
        cast(string) out[64..<72], cast(string) out[N98_OUT_U..<N98_OUT_U + r.u],
        cast(string) out[N98_OUT_METHOD..<N98_OUT_METHOD + r.method],
        ifx r.payload > 0 then "bound" else "absent");
}

// BIP-340 vector 1's published test key and aux, as in chapter 21. Never
// sign with a real key in an example.
KEY_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";
URL :: "https://example.com/api/notes?limit=2";
BODY :: "{\"a\":1}";
CREATED :: 1790000000;

main :: () {
  key: [32] u8; aux: [32] u8;
  decode_hex(KEY_HEX, key[..]); decode_hex(AUX_HEX, aux[..]);

  print("the client\n");
  hex: [64] u8; body_hash(BODY, hex[..]);
  print("sha256(%): %\n", BODY, cast(string) hex[..]);
  tb: [256] u8; tags := auth_tags(URL, "POST", BODY, tb[..]);
  print("tags (% bytes): %\n", tags.count, tags);
  ev: [1024] u8; hb: [2048] u8;
  event := sign(key[..], aux[..], CREATED, 27235, tags, ev[..]);
  header := encode_header(event, hb[..]);
  print("event (% bytes): %\n", event.count, event);
  print("header value: % bytes\n", header.count);
  print("header: %...%\n", header[0..<24], header[header.count - 8..<header.count]);

  print("the server, now = created_at + 30\n");
  seen: [1] Seen;
  now := CREATED + 30;
  serve("first use", seen[..], header, "POST", URL, BODY, now);
  serve("same header again", seen[..], header, "POST", URL, BODY, now);
  print("refusals, from a second event one second later (a new id)\n");
  eb: [1024] u8; fb: [2048] u8;
  second := sign(key[..], aux[..], CREATED + 1, 27235, tags, eb[..]);
  fresh := encode_header(second, fb[..]);
  serve("method GET", seen[..], fresh, "GET", URL, BODY, now);
  serve("query limit=3", seen[..], fresh, "POST", "https://example.com/api/notes?limit=3", BODY, now);
  serve("path /notes/", seen[..], fresh, "POST", "https://example.com/api/notes/?limit=2", BODY, now);
  serve("body {\"a\":2}", seen[..], fresh, "POST", URL, "{\"a\":2}", now);
  serve("now = created_at + 61", seen[..], fresh, "POST", URL, BODY, CREATED + 1 + 61);
  serve("now = created_at - 61", seen[..], fresh, "POST", URL, BODY, CREATED + 1 - 61);
  serve("scheme Bearer", seen[..], "Bearer abc", "POST", URL, BODY, now);
  serve("not base64", seen[..], "Nostr !!!!", "POST", URL, BODY, now);
  xb: [1024] u8; hx: [2048] u8;
  last := second.count - 3;
  serve("last sig digit changed", seen[..],
        encode_header(edit(second, last, ifx second[last] == '0' then '1' else '0', xb[..]), hx[..]),
        "POST", URL, BODY, now);
  serve("first id digit changed", seen[..],
        encode_header(edit(second, 7, ifx second[7] == '0' then '1' else '0', xb[..]), hx[..]),
        "POST", URL, BODY, now);
  ke: [1024] u8; kb: [2048] u8;
  serve("kind 1, signed", seen[..], encode_header(sign(key[..], aux[..], CREATED, 1, tags, ke[..]), kb[..]),
        "POST", URL, BODY, now);
  print("the second event, accepted, then replayed\n");
  serve("method post, now = created_at + 60", seen[..], fresh, "post", URL, BODY, CREATED + 1 + 60);
  serve("the same again", seen[..], fresh, "POST", URL, BODY, CREATED + 1 + 60);

  print("NIP-98's example header\n");
  spec: [1] Seen;
  print("header value: % bytes\n", NIP98_HEADER.count);
  serve("at its created_at", spec[..], NIP98_HEADER, "GET", NIP98_URL, "", NIP98_CREATED);
  work: [N98_WORK] u8; hash: [32] u8;
  e := n98_parse(NIP98_HEADER, work[..]);
  print("read: %, kind %, created_at %\n", e.error == N98_OK, e.kind, e.created);
  print("claimed id : %\n", cast(string) e.id[..]);
  print("id check: %\n", nip98_error_text(n98_id(e, work[..], hash[..])));
  print_hex("computed id", hash[..]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/http-auth.coax --run-steps 250000000
```

The output matches the worked values: 144 bytes of tags, a 488-byte event
with ID `572b6da4...`, and a 658-byte header that ends in one `=`.

```text
the client
sha256({"a":1}): 015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862
tags (144 bytes): [["u","https://example.com/api/notes?limit=2"],["method","POST"],["payload","015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862"]]
event (488 bytes): {"id":"572b6da44f9013c80638b6898f2c0f8ba2febb29695e07d236ac8e19a9566104","pubkey":"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659","created_at":1790000000,"kind":27235,"tags":[["u","https://example.com/api/notes?limit=2"],["method","POST"],["payload","015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862"]],"content":"","sig":"5c2c982d4fb36d1d75f36a44200083c292f5deb0829e8ad8944de7a08e9eda3c9838e431fae62d3508ea5a70bfdcf165d93714b41631e0b2c302fff27969d2b3"}
header value: 658 bytes
header: Nostr eyJpZCI6IjU3MmI2ZG...MmIzIn0=
the server, now = created_at + 30
first use: 200, pubkey dff1d77f..., u https://example.com/api/notes?limit=2, method POST, payload bound
same header again: 401, event id already used
refusals, from a second event one second later (a new id)
method GET: 401, the method tag is not this method
query limit=3: 401, the u tag is not this URL
path /notes/: 401, the u tag is not this URL
body {"a":2}: 401, the payload tag is not this body
now = created_at + 61: 401, created_at is not within 60 s
now = created_at - 61: 401, created_at is not within 60 s
scheme Bearer: 401, no Nostr authorization
not base64: 401, authorization is not base64
last sig digit changed: 401, signature does not verify
first id digit changed: 401, id does not match the event
kind 1, signed: 401, not a kind 27235 event
the second event, accepted, then replayed
method post, now = created_at + 60: 200, pubkey dff1d77f..., u https://example.com/api/notes?limit=2, method POST, payload bound
the same again: 401, event id already used
NIP-98's example header
header value: 556 bytes
at its created_at: 401, id does not match the event
read: true, kind 27235, created_at 1682327852
claimed id : fe964e758903360f28d8424d092da8494ed207cba823110be3a57dfe4b578734
id check: id does not match the event
computed id: 2dd2dfec3df85dd0d4c32af50241f56a077b0969cb508f987afac1e25b0d4c76
```

The refusals, one per check:

- `GET` for a `POST` event refuses as the method. `post` passes, because
  the method compares ASCII letters without case.
- `?limit=3` and the path `/notes/` refuse as the URL: the comparison is
  byte for byte, so the query and a trailing slash count.
- The body `{"a":2}` refuses as the payload.
- `now` 61 seconds after or before `created_at` refuses as the time.
  60 seconds after passes: the window is inclusive.
- A `Bearer` value and a value that is not base64 refuse before any JSON
  is read.
- The last signature digit changed refuses as the signature. The first ID
  digit changed refuses as the ID, before the signature is tried.
- A kind 1 event with the same tags, correctly signed, refuses as the kind.
- The first header and the second event each refuse their second use as
  `event id already used`. The second event's refusals before its
  acceptance did not use up its ID.

NIP-98's example header reads as a kind 27235 event, but it refuses as
printed: its claimed ID is `fe964e75...` and the SHA-256 of its fields is
`2dd2dfec...`. An independent BIP-340 verifier (below) accepts its
signature over the claimed ID, so the event was signed and then its fields
were changed. A server that skipped the ID check and verified the signature
over the claimed ID would accept it.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/http-auth.out](../fixtures/http-auth.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/http-auth.coax](../examples/checks/http-auth.coax),
which loads the example, so running the example doesn't run them. The
independent values come from
[nip59_reference.py](../../coax/tests/nostr/nip59_reference.py), a
pure-Python BIP-340 signer and verifier and NIP-01 serializer written from
the documents. It shares no code with Coax.

- The worked example's event, byte for byte, and the body hash from
  Python's `hashlib`.
- A Python-signed `GET` header with no `payload` tag, encoded by Python's
  `base64`, which passes and refuses as the rules say.
- The ID of NIP-98's example fields, which Coax computes and refuses.
- Base64 lengths against RFC 4648's vectors.

They also check every refusal code, the inclusive time bounds, and replay:
the same ID refuses the second time, a refused request leaves its ID unused,
and a full list refuses.

<!-- source: nostr/examples/checks/http-auth.coax check -->
```jai
// The example's request signed by coax/tests/nostr/nip59_reference.py, a
// pure-Python BIP-340 signer and NIP-01 serializer that shares no code with
// Coax: signed(key, aux, 1790000000, 27235, tags, "") with BIP-340 vector 1's
// key and aux, and the body hash from Python's hashlib.
PY_BODY_SHA256 :: "015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862";
PY_EVENT :: "{\"id\":\"572b6da44f9013c80638b6898f2c0f8ba2febb29695e07d236ac8e19a9566104\",\"pubkey\":\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"created_at\":1790000000,\"kind\":27235,\"tags\":[[\"u\",\"https://example.com/api/notes?limit=2\"],[\"method\",\"POST\"],[\"payload\",\"015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862\"]],\"content\":\"\",\"sig\":\"5c2c982d4fb36d1d75f36a44200083c292f5deb0829e8ad8944de7a08e9eda3c9838e431fae62d3508ea5a70bfdcf165d93714b41631e0b2c302fff27969d2b3\"}";
// A GET with no payload tag, signed by the same Python with the test key 3
// and zero aux, then encoded by Python's base64.b64encode.
PY_GET :: "Nostr eyJpZCI6ImJlYmQ4MTc0NDY4ODc4MDQzMjJkNDg4MjA5OWM3YTZlNTUxMzI1ZWJjMDIzMmYwMDQxOTdmNTk1OWIwMzQxOTgiLCJwdWJrZXkiOiJmOTMwOGEwMTkyNThjMzEwNDkzNDRmODVmODlkNTIyOWI1MzFjODQ1ODM2Zjk5YjA4NjAxZjExM2JjZTAzNmY5IiwiY3JlYXRlZF9hdCI6MTc5MDAwMDAwMCwia2luZCI6MjcyMzUsInRhZ3MiOltbInUiLCJodHRwczovL2V4YW1wbGUuY29tL2FwaS9ub3RlcyJdLFsibWV0aG9kIiwiR0VUIl1dLCJjb250ZW50IjoiIiwic2lnIjoiMjcxYWM2Y2MwNTg2MTc5ZjE2YTY0YjVkYzI3YmRkMjkwODAyZGFjMzk4ZWIxYzExNmZjZjBlN2I3ZDBkYTU2NmE1M2M2NDk5ZTY0ZWUyYWI4ZDU0NTQ4YzVkZDM3YzAwNTMxNTJjOWJhYzZlYmE5ZjJlZjI3MjhlNTI1ZmMzOTAifQ==";
// For NIP-98's example event, Python's NIP-01 id of its fields, and the
// result of Python's BIP-340 verify of its sig over the claimed id: true.
// The id was signed, but the fields were changed after signing.
PY_NIP98_ID :: "2dd2dfec3df85dd0d4c32af50241f56a077b0969cb508f987afac1e25b0d4c76";

key_aux :: (key: [] u8, aux: [] u8) { decode_hex(KEY_HEX, key); decode_hex(AUX_HEX, aux); }
// The decision on a fresh server.
once :: (header: string, method: string, url: string, body: string, now: int) -> int {
  seen: [1] Seen; out: [N98_OUT] u8;
  return decide(seen[..], header, method, url, body, now, out[..]).error;
}
// The base64 length of n bytes with padding, written from RFC 4648.
b64_len :: (n: int) -> int { return (n + 2) / 3 * 4; }
// RFC 4648's vectors: "", "f", ..., "foobar" encode to "", "Zg==",
// "Zm8=", "Zm9v", "Zm9vYg==", "Zm9vYmE=", "Zm9vYmFy".
#test "base64 lengths are RFC 4648's" {
  want := [7] int.[0, 4, 4, 4, 8, 8, 8];
  for n: 0..<7 { #assert b64_len(n) == want[n]; }
}

#test "the client builds Python's event, byte for byte" #steps 2000000000 {
  hex: [64] u8; body_hash(BODY, hex[..]);
  #assert cast(string) hex[..] == PY_BODY_SHA256;
  key: [32] u8; aux: [32] u8; key_aux(key[..], aux[..]);
  tb: [256] u8; ev: [1024] u8; hb: [2048] u8;
  event := sign(key[..], aux[..], CREATED, 27235, auth_tags(URL, "POST", BODY, tb[..]), ev[..]);
  #assert event == PY_EVENT && event.count == 488;
  header := encode_header(event, hb[..]);
  #assert header.count == 6 + b64_len(488) && header.count == 658;
  #assert once(header, "POST", URL, BODY, CREATED + 30) == N98_OK;
}
#test "no body, no payload tag" {
  tb: [256] u8;
  #assert auth_tags("https://example.com/api/notes", "GET", "", tb[..]) ==
          "[[\"u\",\"https://example.com/api/notes\"],[\"method\",\"GET\"]]";
}
#test "Python's GET verifies; without a payload tag any body passes" #steps 2000000000 {
  #assert PY_GET.count == 6 + b64_len(400);
  #assert once(PY_GET, "GET", "https://example.com/api/notes", "", 1790000000) == N98_OK;
  #assert once(PY_GET, "get", "https://example.com/api/notes", "anything", 1790000060) == N98_OK;
  #assert once(PY_GET, "GET", "https://example.com/api/notes", "", 1790000061) == N98_TIME;
  #assert once(PY_GET, "GET", "https://example.com/api/notes", "", 1789999939) == N98_TIME;
  #assert once(PY_GET, "HEAD", "https://example.com/api/notes", "", 1790000000) == N98_METHOD;
  #assert once(PY_GET, "GET", "https://example.com/api/notes?", "", 1790000000) == N98_URL;
  #assert once(PY_GET, "GET", "http://example.com/api/notes", "", 1790000000) == N98_URL;
}
#test "each server check refuses, with its own code" #steps 2000000000 {
  key: [32] u8; aux: [32] u8; key_aux(key[..], aux[..]);
  tb: [256] u8; ev: [1024] u8; hb: [2048] u8;
  tags := auth_tags(URL, "POST", BODY, tb[..]);
  event := sign(key[..], aux[..], CREATED, 27235, tags, ev[..]);
  h := encode_header(event, hb[..]);
  #assert once(h, "POST", URL, BODY, CREATED + 60) == N98_OK;
  #assert once(h, "POST", URL, BODY, CREATED - 60) == N98_OK;
  #assert once(h, "PUT", URL, BODY, CREATED) == N98_METHOD;
  #assert once(h, "POST", "https://example.com/api/notes?limit=2&x", BODY, CREATED) == N98_URL;
  #assert once(h, "POST", URL, "{\"a\":1} ", CREATED) == N98_PAYLOAD;
  #assert once(h, "POST", URL, "", CREATED) == N98_PAYLOAD;
  #assert once(h, "POST", URL, BODY, CREATED + 61) == N98_TIME;
  #assert once(h, "POST", URL, BODY, CREATED - 61) == N98_TIME;
  #assert once(h[1..<h.count], "POST", URL, BODY, CREATED) == N98_NO_AUTH;
  #assert once("Nostr ****", "POST", URL, BODY, CREATED) == N98_BASE64;
  xb: [1024] u8; xh: [2048] u8;
  #assert once(encode_header(edit(event, event.count - 3, '4', xb[..]), xh[..]), "POST", URL, BODY, CREATED) == N98_SIGNATURE;
  #assert once(encode_header(edit(event, 7, '6', xb[..]), xh[..]), "POST", URL, BODY, CREATED) == N98_ID;
  ke: [1024] u8; kh: [2048] u8;
  #assert once(encode_header(sign(key[..], aux[..], CREATED, 1, tags, ke[..]), kh[..]), "POST", URL, BODY, CREATED) == N98_KIND;
}
#test "the same event id refuses the second time; a new id passes" #steps 2000000000 {
  key: [32] u8; aux: [32] u8; key_aux(key[..], aux[..]);
  tb: [256] u8; ev: [1024] u8; hb: [2048] u8; e2: [1024] u8; h2: [2048] u8;
  tags := auth_tags(URL, "POST", BODY, tb[..]);
  h := encode_header(sign(key[..], aux[..], CREATED, 27235, tags, ev[..]), hb[..]);
  seen: [1] Seen; out: [N98_OUT] u8;
  #assert decide(seen[..], h, "POST", URL, BODY, CREATED, out[..]).error == N98_OK;
  #assert decide(seen[..], h, "POST", URL, BODY, CREATED + 1, out[..]).error == REPLAYED;
  // A refused request does not use up its id.
  h2s := encode_header(sign(key[..], aux[..], CREATED + 1, 27235, tags, e2[..]), h2[..]);
  #assert decide(seen[..], h2s, "GET", URL, BODY, CREATED, out[..]).error == N98_METHOD;
  #assert decide(seen[..], h2s, "POST", URL, BODY, CREATED, out[..]).error == N98_OK;
  #assert decide(seen[..], h2s, "POST", URL, BODY, CREATED, out[..]).error == REPLAYED;
  #assert seen[0].count == 2;
  // A full list refuses rather than forgetting an id.
  seen[0].count = 8;
  #assert decide(seen[..], PY_GET, "GET", "https://example.com/api/notes", "", 1790000000, out[..]).error == FULL;
}
#test "NIP-98's example: its claimed id is not its fields' id, so it refuses" {
  #assert NIP98_HEADER.count == 6 + 550;
  #assert once(NIP98_HEADER, "GET", NIP98_URL, "", NIP98_CREATED) == N98_ID;
  work: [N98_WORK] u8; hash: [32] u8;
  e := n98_parse(NIP98_HEADER, work[..]);
  #assert e.error == N98_OK && e.kind == 27235 && e.created == NIP98_CREATED;
  #assert n98_id(e, work[..], hash[..]) == N98_ID;
  hex: [64] u8; r := hex_encode_checked(cast(string) hash[..], hex[..], false);
  #assert cast(string) hex[..] == PY_NIP98_ID;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/http-auth.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/http-auth.coax --root-only --run-steps 250000000
```

`coax test` runs 368 checks, including the loaded files' own checks, in
about 30 seconds. Root-only mutation of the checks file killed 8 of 8
mutants on 2026-11-01.

### The verifier's own checks

`nip98.coax` carries 86 cases in
[nip98_vectors.coax](../../coax/web/api/nip98_vectors.coax), generated by
[nip98_fixtures.ts](../../coax/web/api/nip98_fixtures.ts) with
`nostr-effect`, an independent TypeScript Nostr library: accepted requests,
each rule's refusal, the reader's limits, forged and malformed signatures,
and tags cut at a decoded NUL. Its checks run every case through the reader,
the ID and the rules without BIP-340. They ran above, because the example
loads the file.
[nip98_checks.coax](../../coax/web/api/nip98_checks.coax) runs the signed
cases with BIP-340 to the end. Root-only mutation of `nip98.coax` killed
1,020 of 1,024 mutants with none surviving (4 marked equivalent, each with
its reason on the line), and `nip98_diff` against the former C check found
no disagreement (commit `4825cffd58`, 2026-10-31). Neither was rerun today.

The stricter tag reader and observer have their own records:
[2026-10-29-nip98-tags.md](../../docs/verification/2026-10-29-nip98-tags.md)
and
[2026-10-29-nip98-observation.md](../../docs/verification/2026-10-29-nip98-observation.md),
with the event verifier comparison in
[2026-10-29-api-event-parity.md](../../docs/verification/2026-10-29-api-event-parity.md).
[weblogin_vectors.coax](../../coax/tests/nostr/weblogin_vectors.coax) checks
`weblogin.coax`'s chain from words to a signed login. These were not rerun
today.

## Summary

- A NIP-98 event is kind 27235 with `u`, `method` and an optional
  `payload` tag, sent as `Authorization: Nostr <base64>`.
- `nip98_check_coax` verifies the event, then the kind, the 60-second
  window, the exact URL, the method and the payload hash, each with its own
  refusal.
- The server accepts each event ID once, records it only after the checks
  pass, and can forget it once its window has passed.
- The worked header is 658 bytes, and NIP-98's own example refuses because
  its ID does not match its fields.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [4. SHA-256](../../bitcoin/book/ch004-sha-256.md)

## Sources

- [NIP-98](../nips/official/98.md)
- [RFC 4648](https://www.rfc-editor.org/rfc/rfc4648), base64.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
