# 37. NIP-11 relay information

Clients learn a relay's limits from its NIP-11 document. This chapter serves
the document and checks that each advertised limit is the one the relay
enforces.

Part IX, A relay. Result: The relay information document served on `Accept:
application/nostr+json`: name, pubkey, `supported_nips`, software, version and
`limitation` fields that match what the relay enforces.

## What this chapter teaches

- How the relay tells three requests on its one URL apart: a plain `GET`
  with `Accept: application/nostr+json` gets the NIP-11 document, a request
  with `Upgrade: websocket` gets the WebSocket handshake, and anything else
  gets `400 Bad Request`.
- The answer byte for byte: the status line, `Content-Type`,
  `Content-Length`, the three CORS headers NIP-11 requires, and
  `Connection: close`.
- The fields: `name`, `description`, `supported_nips`, `software` and
  `version`, and the ones the Coax relay leaves out (`pubkey`, `self`,
  `contact`, `icon`, `banner`).
- What each `limitation` field means, and which line of the relay enforces
  it: `max_message_length`, `max_subscriptions`, `max_limit`,
  `max_subid_length`, `max_event_tags`, `created_at_upper_limit`,
  `default_limit`, `auth_required` and `restricted_writes`.
- A small client-side reader for the document, and a comparison of every
  advertised value with the value the code enforces.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ws_server.coax](../../coax/lib/net/ws_server.coax) | `wss_request`, `wss_media` | reads the request head: `WSS_OK` for a valid upgrade, `WSS_NIP11` for a plain `GET` whose `Accept` names `application/nostr+json`, a refusal code otherwise (chapter 27) |
| | `wss_http_write` | one HTTP/1.1 response with the CORS headers, then the connection closes |
| [relay_route.coax](../../coax/lib/nostr/relay_route.coax) | `rr_http` | the action for a head: wait, upgrade, NIP-11 or 400 (`#authority`) |
| | `RR_NIP11` | the document `relay.coax` serves |
| | `rr_nip11_pow`, `rr_nip13_at` | with `--pow`, NIP-13 added to `supported_nips` and `min_pow_difficulty` to `limitation` (`#authority`) |
| [relay_server.coax](../../coax/lib/nostr/relay_server.coax) | `RS_MAX_MESSAGE`, `RS_MAX_SUBS`, `RS_MAX_TAGS`, `RS_FUTURE`, `rs_limit` | the limits the document advertises (`#authority`) |
| | `RS_NIP11`, `rs_nip11` | a second copy of the document with its own checks; the same bytes as `RR_NIP11` since this chapter |
| [message_server.coax](../../coax/lib/nostr/message_server.coax) | `MS_SUB_MAX`, `ms_sub_ok` | subscription ids of 1 to 64 characters |
| [relay.coax](../../coax/relay/relay.coax) | `ry_http` | the relay program: reads the head and writes the answer |

There is no client-side NIP-11 reader in `coax/lib/nostr/`. The example
defines a small one, `info_read`, with its checks; it isn't library code yet.

## One URL, three answers

A relay has one URL, such as `wss://relay.example/`. A WebSocket client
opens it with an upgrade request. A client that wants the relay's
information sends an ordinary HTTP `GET` to the same URL with
`Accept: application/nostr+json`. NIP-11 asks the relay to answer that
request with a JSON document.

`wss_request` reads the head up to the blank line and records three things:
whether `Upgrade` is `websocket`, whether any `Accept` header names
`application/nostr+json`, and the WebSocket fields. Then it decides:

1. The head isn't finished: `WSS_MORE`. The relay waits for more bytes.
2. The request line isn't `GET /... HTTP/1.1`: `WSS_METHOD`.
3. No `Upgrade: websocket`, and `Accept` names `application/nostr+json`:
   `WSS_NIP11`.
4. No `Upgrade: websocket` and no such `Accept`: `WSS_UPGRADE` (a missing
   upgrade).
5. `Upgrade: websocket`: the WebSocket checks of chapter 27 (`Connection`,
   version 13, one 16-byte key). All pass: `WSS_OK`.

`rr_http` maps `WSS_MORE` to wait, `WSS_OK` to upgrade, `WSS_NIP11` to the
NIP-11 answer and every other code to a `400` with a short text. After the
NIP-11 or `400` answer the relay closes the connection.

Two details follow from the order:

- A request with both headers is an upgrade. `Accept` is only consulted
  when there is no `Upgrade: websocket`.
- `wss_media` reads `Accept` as a comma-separated list of media ranges,
  drops each range's parameters after `;`, trims spaces, and compares the
  rest without case. `text/html, Application/Nostr+JSON;q=0.9` matches;
  `application/nostr+jsonx` doesn't.

A browser page can fetch the document from another origin. The response
carries `Access-Control-Allow-Origin: *`, `Access-Control-Allow-Headers: *`
and `Access-Control-Allow-Methods: GET`, as NIP-11 requires. `Accept` is a
CORS-safelisted request header, so the browser sends this `GET` without a
preflight. The relay answers an `OPTIONS` request with `400`.

### Worked example: route four requests by hand

The example's requests, and what the rules above give:

| Request head (after `GET / HTTP/1.1`) | `wss_request` | `rr_http` | First line of the answer |
| --- | --- | --- | --- |
| `Host`, `Accept: application/nostr+json` | `WSS_NIP11` | NIP-11 | `HTTP/1.1 200 OK` |
| `Upgrade: websocket`, `Connection: Upgrade`, key, version 13 | `WSS_OK` | upgrade | `HTTP/1.1 101 Switching Protocols` |
| both of the above | `WSS_OK` | upgrade | `HTTP/1.1 101 Switching Protocols` |
| `Accept: text/html` | `WSS_UPGRADE` | 400 | `HTTP/1.1 400 Bad Request` |

The NIP-11 answer is the head and then the document. The document,
`RR_NIP11`, is 517 bytes, so the head says `Content-Length: 517`. Each
header line ends in CRLF, two bytes:

| Line | Bytes |
| --- | --- |
| `HTTP/1.1 200 OK` | 15 + 2 = 17 |
| `Content-Type: application/nostr+json` | 36 + 2 = 38 |
| `Content-Length: 517` | 19 + 2 = 21 |
| `Access-Control-Allow-Origin: *` | 30 + 2 = 32 |
| `Access-Control-Allow-Headers: *` | 31 + 2 = 33 |
| `Access-Control-Allow-Methods: GET` | 33 + 2 = 35 |
| `Connection: close` | 17 + 2 = 19 |
| the empty line | 2 |

That is 197 bytes of head, and 197 + 517 = 714 bytes in all.

The `400` answer has the same header lines with three values changed:
`400 Bad Request` is 9 bytes longer than `200 OK`, `text/plain` is 12 bytes
shorter than `application/nostr+json`, and the body is the 83-byte text,
so `83` is one digit shorter than `517`. Its head is 197 + 9 − 12 − 1 = 193
bytes, and the answer is 193 + 83 = 276 bytes.

## The document

The Coax relay serves this document (the example prints it on one line, as
it is sent):

```text
{
 "name": "OpenAgents Coax relay (R1)",
 "description": "A Nostr relay written in Coax. Tailnet preview; relay.openagents.com is unchanged. Publishing needs NIP-42 AUTH; reading is open.",
 "supported_nips": [1, 9, 11, 40, 42, 45, 62, 70],
 "software": "https://github.com/OpenAgentsInc/openagents",
 "version": "r1",
 "limitation": {
  "max_message_length": 131072,
  "max_subscriptions": 32,
  "max_limit": 127,
  "max_subid_length": 64,
  "max_event_tags": 256,
  "auth_required": false,
  "restricted_writes": true,
  "created_at_upper_limit": 900,
  "default_limit": 127
 }
}
```

Every NIP-11 field is optional. The relay sends:

- `name`, 26 characters. NIP-11 asks for fewer than 30, so clients don't
  truncate it.
- `description`, plain text with no markup.
- `supported_nips`: the relay-side NIPs it implements. NIP-11 says not to
  list client-side NIPs.
- `software`, a URL to the project, and `version`, a string the relay
  chooses.

It leaves out `pubkey` and `contact` (an administrator's key and another
address), `self` (the relay's own key, which chapter 67 needs), `icon`,
`banner` and `terms_of_service`. A client must not assume any of them.

### `supported_nips`

The list names what the relay does, not what its libraries could do:

| NIP | What the relay does | Where |
| --- | --- | --- |
| 1 | `EVENT`, `REQ`, `CLOSE`; `OK`, `EOSE`, `CLOSED`, `NOTICE` | `message_server.coax`, `relay_server.coax` (chapters 24, 34) |
| 9 | a stored kind 5 retires the events it names, and they are refused `blocked:` afterwards | `store_delete.coax` in `rd_admit` (chapter 39) |
| 11 | this document on `Accept: application/nostr+json` | `ws_server.coax`, `relay_route.coax` |
| 40 | refuses an expired event and never serves one | `rs_expiration`, `rs_live` (chapter 34) |
| 42 | sends a challenge, checks `AUTH`, and requires a key for writes | `nip42.coax`, `rr_auth` (chapter 38) |
| 45 | answers `COUNT` with the exact number of matching stored events | `nip45.coax` through `rd_count` (chapter 40) |
| 62 | a kind 62 naming this relay, or `ALL_RELAYS`, retires its author's events | `store_delete.coax` in `rd_admit` (chapter 39) |
| 70 | an event with the `"-"` tag only from a connection authenticated as its author | `nip70.coax`'s `nip70_admit` (chapter 42) |

One NIP is listed only when it is turned on:

- NIP-13, proof of work: off by default. With `--pow BITS` the relay asks
  each `EVENT` for that many leading zero bits (chapter 56), and
  `rr_nip11_pow` puts 13 into `supported_nips` before the first larger
  NIP and `min_pow_difficulty` at the end of `limitation`.

NIPs the relay doesn't list, and what it does instead:

- NIP-50, `search`: a filter with `search` is refused with `CLOSED` and
  `invalid: malformed filter`.

### `limitation`

NIP-11 calls these *practical* limits: a client should expect a request
past one to fail. Each field, what it means, and the relay's code that
enforces it:

| Field | Value | Meaning | Enforced by |
| --- | --- | --- | --- |
| `max_message_length` | 131072 | the largest incoming message in bytes, from `[` to `]`; the largest WebSocket message | `wss_frame_read(bytes, RS_MAX_MESSAGE)` for one frame, and `ws_take` into a `[RS_MAX_MESSAGE] u8` buffer for a fragmented message: one byte more is close 1009 |
| `max_subscriptions` | 32 | open subscriptions on one connection | `rs_sub_open(subs, m, text, RS_MAX_SUBS)`: the 33rd is `CLOSED` `restricted:` |
| `max_limit` | 127 | the largest `limit` a filter gets; a larger one is clamped silently | `rs_limit` |
| `max_subid_length` | 64 | a subscription id's length, in characters | `ms_sub_ok` counts UTF-8 characters, 1 to `MS_SUB_MAX` |
| `max_event_tags` | 256 | entries in an event's `tags` | `rs_admit_policy`: 257 is `OK false` `invalid:` |
| `created_at_upper_limit` | 900 | how many seconds ahead of the relay's clock `created_at` may be | `rs_admit_policy`: `now + RS_FUTURE` |
| `default_limit` | 127 | the `limit` of a filter that has none | `rs_limit` |
| `auth_required` | false | NIP-42 before any action at all | reads (`REQ`, `CLOSE`) never ask `rr_authed` |
| `restricted_writes` | true | some writes are refused by a condition other than validity | an `EVENT` needs a NIP-42 key: `rr_writes_need_auth(true)` |

`auth_required` and `restricted_writes` describe the R1 relay's mixed
policy: anyone reads, only an authenticated connection writes. NIP-11 says
`auth_required: false` doesn't mean no action ever needs authentication.

The relay sends no `created_at_lower_limit`, because it accepts any past
time; no `max_content_length` or `min_pow_difficulty`, because it checks
neither; and no `payment_required`. It has two limits without a NIP-11
field: 16 filters per `REQ` and a query cost of 100,000 (chapter 36).

### Worked example: the limits at their edges

Each row is one value at the limit and one past it, worked out from the
table before the program runs. The policy rows use the relay's fixture
events (`relay_server_fixtures.coax`), written for a clock of 1000:

| Limit | At the limit | One past it |
| --- | --- | --- |
| 131,072-byte frame | header admitted, waits for the payload | close 1009 before any payload byte |
| fragments of 65,536 + 65,536 / + 65,537 | one message of 131,072 bytes | close 1009 |
| 32 subscriptions | `s1`..`s32` in slots 0..31 | `s33`: `CLOSED` `restricted: too many active subscriptions` |
| 64-character sub id | 64 `a`: routed to `REQ`; 64 `é` (128 bytes): routed to `REQ` | 65 `a`: `NOTICE` `invalid: subscription id must contain 1 to 64 characters` |
| `limit` 127 | 127 | 128 and 5000 become 127; no `limit` is 127 |
| 256 tags | passes to the signature check | `OK false` `invalid: event has too many tags` |
| `created_at` 1000 + 900 = 1900 | passes | 1901: `OK false` with the `created_at` text |

The sub id row shows why NIP-11 says "as a string": `é` is two bytes in
UTF-8, and the relay counts characters, so 64 of them are accepted.

## The example

The example is [nostr/examples/relay-info.coax](../examples/relay-info.coax).
It loads `relay_server.coax` (which loads chapter 27's server half and
chapter 24's codec) and `relay_route.coax`, and no store and no signer. It:

1. routes eight request heads and prints each status, action and first
   line;
2. prints the NIP-11 answer and the `400` answer byte for byte, with each
   CRLF shown as `\r\n`;
3. reads the document with `info_read` and compares every field with the
   value the relay enforces;
4. hits each limit at its value and one past it, through the same
   procedures the relay calls;
5. edits the document, a field at a time, and shows the mismatch or the
   reader's refusal.

The answer to one request, as `ry_http` writes it:

<!-- source: nostr/examples/relay-info.coax answer -->
```jai
// What the relay does with one HTTP request head, as coax/relay/relay.coax's
// ry_http does it, with the socket replaced by out: wss_request reads the
// head, rr_http picks the action, and the answer is written into out.
Answer :: struct { status: int; action: int; n: int; }
http_answer :: (request: string, out: [] u8) -> Answer {
  h := wss_request(request);
  a := Answer.{status = h.status, action = rr_http(h.status)};
  if a.action == RR_H_WAIT { return a; }
  if a.action == RR_H_UPGRADE { a.n = wss_accept_write(request[h.key_from..<h.key_to], out); return a; }
  if a.action == RR_H_NIP11 { a.n = wss_http_write("200 OK", "application/nostr+json", RR_NIP11, out); return a; }
  a.n = wss_http_write("400 Bad Request", "text/plain", "This is a Nostr relay: connect with a WebSocket, or ask for application/nostr+json.", out);
  return a;
}
```

The client side. NIP-11 says a client must ignore fields it doesn't
understand, so `info_read` skips any other key with `json_skip`, at the top
level and inside `limitation`. It refuses a document that isn't one JSON
object, a `supported_nips` that isn't an array of positive integers, and a
known `limitation` field of the wrong type:

<!-- source: nostr/examples/relay-info.coax client -->
```jai
// What a client reads from a NIP-11 document. An integer field the document
// leaves out is -1; a boolean is -1 (absent), 0 or 1. NIP-11 says clients
// MUST ignore fields they don't know, so info_read skips them.
INFO_OK :: 0; INFO_JSON :: 1; INFO_SHAPE :: 2; INFO_NIPS :: 3; INFO_LIMIT :: 4;
INFO_MAX_NIPS :: 32;
LIMIT_NAMES :: [9] string.["max_message_length", "max_subscriptions", "max_limit", "max_subid_length",
  "max_event_tags", "created_at_upper_limit", "default_limit", "auth_required", "restricted_writes"];
LIMIT_BOOL :: 7;   // fields from here on are booleans
RelayInfo :: struct {
  error: int;
  name: string; software: string; version: string;   // raw JSON string bytes
  nips: [INFO_MAX_NIPS] int; nips_n: int;
  limit: [9] int;
}
info_read :: (doc: string) -> RelayInfo {
  i: RelayInfo; i.error = INFO_JSON;
  for k: 0..<9 { i.limit[k] = -1; }
  j: [1] Json; json_start(j[..], doc);
  t := json_next(j[..]);
  if t == JSON_ERROR { return i; }
  if t != JSON_OBJECT { i.error = INFO_SHAPE; return i; }
  t = json_next(j[..]);
  while t == JSON_STRING {
    key := json_text(j[..]);
    v := json_next(j[..]);
    if json_is(key, "supported_nips") {
      if v != JSON_ARRAY { return info_fail(i, v, INFO_NIPS); }
      v = json_next(j[..]);
      while v == JSON_NUMBER {
        n := json_int(json_text(j[..]));
        if !n.ok || n.value < 1 || i.nips_n >= INFO_MAX_NIPS { i.error = INFO_NIPS; return i; }
        i.nips[i.nips_n] = n.value; i.nips_n += 1;
        v = json_next(j[..]);
      }
      if v != JSON_ARRAY_END { return info_fail(i, v, INFO_NIPS); }
    } else if json_is(key, "limitation") {
      if v != JSON_OBJECT { return info_fail(i, v, INFO_LIMIT); }
      v = json_next(j[..]);
      while v == JSON_STRING {
        name := json_text(j[..]);
        w := json_next(j[..]);
        k := 0; while k < 9 && !json_is(name, LIMIT_NAMES[k]) { k += 1; }
        if k < LIMIT_BOOL {
          n := json_int(json_text(j[..]));
          if w != JSON_NUMBER || !n.ok || n.value < 0 { return info_fail(i, w, INFO_LIMIT); }
          i.limit[k] = n.value;
        } else if k < 9 {
          if w != JSON_TRUE && w != JSON_FALSE { return info_fail(i, w, INFO_LIMIT); }
          i.limit[k] = ifx w == JSON_TRUE then 1 else 0;
        } else if !json_skip(j[..]) { return i; }
        v = json_next(j[..]);
      }
      if v != JSON_OBJECT_END { return info_fail(i, v, INFO_LIMIT); }
    } else if v == JSON_STRING && json_is(key, "name") { i.name = json_text(j[..]);
    } else if v == JSON_STRING && json_is(key, "software") { i.software = json_text(j[..]);
    } else if v == JSON_STRING && json_is(key, "version") { i.version = json_text(j[..]);
    } else if !json_skip(j[..]) { return i; }
    t = json_next(j[..]);
  }
  if t != JSON_OBJECT_END || json_next(j[..]) != JSON_END { i.error = INFO_JSON; return i; }
  i.error = INFO_OK;
  return i;
}
// A refusal while reading: JSON_ERROR stays a JSON error, anything else is
// the field's error.
info_fail :: (i: RelayInfo, tok: int, error: int) -> RelayInfo {
  r := i; r.error = ifx tok == JSON_ERROR then INFO_JSON else error;
  return r;
}
info_has :: (i: RelayInfo, nip: int) -> bool {
  for k: 0..<i.nips_n { if i.nips[k] == nip { return true; } }
  return false;
}
```

The comparison. `enforced` reads each value from the constant or procedure
the relay calls, not from the document, so a change to either side shows
up:

<!-- source: nostr/examples/relay-info.coax enforced -->
```jai
// The value of each limitation field as the relay's code enforces it, read
// from the constants and procedures the relay calls, never typed by hand.
enforced :: () -> [9] int {
  big: NostrFilter; big.has_limit = true; big.limit = 1 << 40;
  none: NostrFilter;
  v: [9] int;
  v[0] = RS_MAX_MESSAGE;          // wss_frame_read's limit and ws_take's message buffer
  v[1] = RS_MAX_SUBS;             // rs_sub_open's cap in ry_req
  v[2] = rs_limit(big);           // the clamp on a filter's limit
  v[3] = MS_SUB_MAX;              // ms_read's subscription id length, in characters
  v[4] = RS_MAX_TAGS;             // rs_admit_policy's tag count
  v[5] = RS_FUTURE;               // rs_admit_policy's created_at window, seconds ahead
  v[6] = rs_limit(none);          // a filter with no limit
  v[7] = 0;                       // REQ and CLOSE never ask rr_authed
  v[8] = ifx rr_writes_need_auth(true) then 1 else 0;   // EVENT needs a NIP-42 key
  return v;
}
// The NIPs the relay implements, each with the code that implements it.
// NIP-13 is not here: proof of work is off unless the relay starts with
// --pow, and then rr_nip11_pow adds 13 to the served list.
RELAY_NIPS :: [8] int.[
  1,     // message_server.coax, relay_server.coax: EVENT, REQ, CLOSE, OK, EOSE, CLOSED, NOTICE
  9,     // store_delete.coax in rd_admit: a stored kind 5 retires what it names
  11,    // this chapter: RR_NIP11 on Accept: application/nostr+json
  40,    // rs_expiration and rs_live: expired events refused and never served
  42,    // nip42.coax and rr_auth: AUTH, and writes need an authenticated key
  45,    // nip45.coax through rd_count: COUNT answered with an exact count
  62,    // store_delete.coax in rd_admit: a kind 62 retires its author's events
  70     // nip70.coax's nip70_admit: a "-" event only from its authenticated author
];
// The number of fields where the document disagrees with the relay.
info_compare :: (i: RelayInfo, say: bool) -> int {
  want := enforced(); bad := 0;
  for k: 0..<9 {
    same := i.limit[k] == want[k];
    if !same { bad += 1; }
    if say { print("  %: advertised %, enforced % %\n", LIMIT_NAMES[k], i.limit[k], want[k], ifx same then "ok" else "MISMATCH"); }
  }
  nips_same := i.nips_n == RELAY_NIPS.count;
  for n: RELAY_NIPS { if !info_has(i, n) { nips_same = false; } }
  if !nips_same { bad += 1; }
  if say {
    print("  supported_nips: % listed, relay implements", i.nips_n);
    for n, k: RELAY_NIPS { print("% %", ifx k == 0 then "" else ",", n); }
    print(" %\n", ifx nips_same then "ok" else "MISMATCH");
  }
  return bad;
}
```

The whole program:

<!-- source: nostr/examples/relay-info.coax example -->
```jai
#load "../../coax/lib/nostr/relay_server.coax";
#load "../../coax/lib/nostr/relay_route.coax";
#import "Basic";

// The relay's clock for the policy probes: relay_server_fixtures.coax's
// events (RST_*) are written for now = 1000.
NOW :: 1000;

// ---- the server side: one request head, one answer -------------------------

// book:begin answer
// What the relay does with one HTTP request head, as coax/relay/relay.coax's
// ry_http does it, with the socket replaced by out: wss_request reads the
// head, rr_http picks the action, and the answer is written into out.
Answer :: struct { status: int; action: int; n: int; }
http_answer :: (request: string, out: [] u8) -> Answer {
  h := wss_request(request);
  a := Answer.{status = h.status, action = rr_http(h.status)};
  if a.action == RR_H_WAIT { return a; }
  if a.action == RR_H_UPGRADE { a.n = wss_accept_write(request[h.key_from..<h.key_to], out); return a; }
  if a.action == RR_H_NIP11 { a.n = wss_http_write("200 OK", "application/nostr+json", RR_NIP11, out); return a; }
  a.n = wss_http_write("400 Bad Request", "text/plain", "This is a Nostr relay: connect with a WebSocket, or ask for application/nostr+json.", out);
  return a;
}
// book:end answer
status_name :: (s: int) -> string {
  names := [10] string.["OK", "MORE", "TOO_BIG", "METHOD", "HEADER", "UPGRADE", "CONNECTION", "VERSION", "KEY", "NIP11"];
  return names[s];
}
action_name :: (a: int) -> string {
  names := [4] string.["wait", "upgrade", "NIP-11", "400"];
  return names[a];
}
// Print bytes with each CRLF shown as "\r\n" and a line break.
show_crlf :: (bytes: string) {
  line := 0;
  for c, i: bytes {
    if c == '\r' && i + 1 < bytes.count && bytes[i + 1] == '\n' { print("  %\\r\\n\n", bytes[line..<i]); line = i + 2; }
  }
  if line < bytes.count { print("  %\n", bytes[line..<bytes.count]); }
}
show_route :: (label: string, request: string) {
  out: [1024] u8;
  a := http_answer(request, out[..]);
  first := 0; while first < a.n && out[first] != '\r' { first += 1; }
  print("%: % -> %, answer \"%\"\n", label, status_name(a.status), action_name(a.action), cast(string) out[0..<first]);
}

// ---- the client side: read a NIP-11 document --------------------------------

// book:begin client
// What a client reads from a NIP-11 document. An integer field the document
// leaves out is -1; a boolean is -1 (absent), 0 or 1. NIP-11 says clients
// MUST ignore fields they don't know, so info_read skips them.
INFO_OK :: 0; INFO_JSON :: 1; INFO_SHAPE :: 2; INFO_NIPS :: 3; INFO_LIMIT :: 4;
INFO_MAX_NIPS :: 32;
LIMIT_NAMES :: [9] string.["max_message_length", "max_subscriptions", "max_limit", "max_subid_length",
  "max_event_tags", "created_at_upper_limit", "default_limit", "auth_required", "restricted_writes"];
LIMIT_BOOL :: 7;   // fields from here on are booleans
RelayInfo :: struct {
  error: int;
  name: string; software: string; version: string;   // raw JSON string bytes
  nips: [INFO_MAX_NIPS] int; nips_n: int;
  limit: [9] int;
}
info_read :: (doc: string) -> RelayInfo {
  i: RelayInfo; i.error = INFO_JSON;
  for k: 0..<9 { i.limit[k] = -1; }
  j: [1] Json; json_start(j[..], doc);
  t := json_next(j[..]);
  if t == JSON_ERROR { return i; }
  if t != JSON_OBJECT { i.error = INFO_SHAPE; return i; }
  t = json_next(j[..]);
  while t == JSON_STRING {
    key := json_text(j[..]);
    v := json_next(j[..]);
    if json_is(key, "supported_nips") {
      if v != JSON_ARRAY { return info_fail(i, v, INFO_NIPS); }
      v = json_next(j[..]);
      while v == JSON_NUMBER {
        n := json_int(json_text(j[..]));
        if !n.ok || n.value < 1 || i.nips_n >= INFO_MAX_NIPS { i.error = INFO_NIPS; return i; }
        i.nips[i.nips_n] = n.value; i.nips_n += 1;
        v = json_next(j[..]);
      }
      if v != JSON_ARRAY_END { return info_fail(i, v, INFO_NIPS); }
    } else if json_is(key, "limitation") {
      if v != JSON_OBJECT { return info_fail(i, v, INFO_LIMIT); }
      v = json_next(j[..]);
      while v == JSON_STRING {
        name := json_text(j[..]);
        w := json_next(j[..]);
        k := 0; while k < 9 && !json_is(name, LIMIT_NAMES[k]) { k += 1; }
        if k < LIMIT_BOOL {
          n := json_int(json_text(j[..]));
          if w != JSON_NUMBER || !n.ok || n.value < 0 { return info_fail(i, w, INFO_LIMIT); }
          i.limit[k] = n.value;
        } else if k < 9 {
          if w != JSON_TRUE && w != JSON_FALSE { return info_fail(i, w, INFO_LIMIT); }
          i.limit[k] = ifx w == JSON_TRUE then 1 else 0;
        } else if !json_skip(j[..]) { return i; }
        v = json_next(j[..]);
      }
      if v != JSON_OBJECT_END { return info_fail(i, v, INFO_LIMIT); }
    } else if v == JSON_STRING && json_is(key, "name") { i.name = json_text(j[..]);
    } else if v == JSON_STRING && json_is(key, "software") { i.software = json_text(j[..]);
    } else if v == JSON_STRING && json_is(key, "version") { i.version = json_text(j[..]);
    } else if !json_skip(j[..]) { return i; }
    t = json_next(j[..]);
  }
  if t != JSON_OBJECT_END || json_next(j[..]) != JSON_END { i.error = INFO_JSON; return i; }
  i.error = INFO_OK;
  return i;
}
// A refusal while reading: JSON_ERROR stays a JSON error, anything else is
// the field's error.
info_fail :: (i: RelayInfo, tok: int, error: int) -> RelayInfo {
  r := i; r.error = ifx tok == JSON_ERROR then INFO_JSON else error;
  return r;
}
info_has :: (i: RelayInfo, nip: int) -> bool {
  for k: 0..<i.nips_n { if i.nips[k] == nip { return true; } }
  return false;
}
// book:end client
INFO_ERRORS :: [5] string.["ok", "not JSON", "not an object", "bad supported_nips", "bad limitation field"];

// ---- what the relay enforces -------------------------------------------------

// book:begin enforced
// The value of each limitation field as the relay's code enforces it, read
// from the constants and procedures the relay calls, never typed by hand.
enforced :: () -> [9] int {
  big: NostrFilter; big.has_limit = true; big.limit = 1 << 40;
  none: NostrFilter;
  v: [9] int;
  v[0] = RS_MAX_MESSAGE;          // wss_frame_read's limit and ws_take's message buffer
  v[1] = RS_MAX_SUBS;             // rs_sub_open's cap in ry_req
  v[2] = rs_limit(big);           // the clamp on a filter's limit
  v[3] = MS_SUB_MAX;              // ms_read's subscription id length, in characters
  v[4] = RS_MAX_TAGS;             // rs_admit_policy's tag count
  v[5] = RS_FUTURE;               // rs_admit_policy's created_at window, seconds ahead
  v[6] = rs_limit(none);          // a filter with no limit
  v[7] = 0;                       // REQ and CLOSE never ask rr_authed
  v[8] = ifx rr_writes_need_auth(true) then 1 else 0;   // EVENT needs a NIP-42 key
  return v;
}
// The NIPs the relay implements, each with the code that implements it.
// NIP-13 is not here: proof of work is off unless the relay starts with
// --pow, and then rr_nip11_pow adds 13 to the served list.
RELAY_NIPS :: [8] int.[
  1,     // message_server.coax, relay_server.coax: EVENT, REQ, CLOSE, OK, EOSE, CLOSED, NOTICE
  9,     // store_delete.coax in rd_admit: a stored kind 5 retires what it names
  11,    // this chapter: RR_NIP11 on Accept: application/nostr+json
  40,    // rs_expiration and rs_live: expired events refused and never served
  42,    // nip42.coax and rr_auth: AUTH, and writes need an authenticated key
  45,    // nip45.coax through rd_count: COUNT answered with an exact count
  62,    // store_delete.coax in rd_admit: a kind 62 retires its author's events
  70     // nip70.coax's nip70_admit: a "-" event only from its authenticated author
];
// The number of fields where the document disagrees with the relay.
info_compare :: (i: RelayInfo, say: bool) -> int {
  want := enforced(); bad := 0;
  for k: 0..<9 {
    same := i.limit[k] == want[k];
    if !same { bad += 1; }
    if say { print("  %: advertised %, enforced % %\n", LIMIT_NAMES[k], i.limit[k], want[k], ifx same then "ok" else "MISMATCH"); }
  }
  nips_same := i.nips_n == RELAY_NIPS.count;
  for n: RELAY_NIPS { if !info_has(i, n) { nips_same = false; } }
  if !nips_same { bad += 1; }
  if say {
    print("  supported_nips: % listed, relay implements", i.nips_n);
    for n, k: RELAY_NIPS { print("% %", ifx k == 0 then "" else ",", n); }
    print(" %\n", ifx nips_same then "ok" else "MISMATCH");
  }
  return bad;
}
// book:end enforced

// ---- the limits, hit one at a time --------------------------------------------

// A masked text frame with an all-zero mask key (so the payload is sent as
// is): fin, opcode, count bytes of 'x'. Its length.
fragment :: (fin: bool, op: int, count: int, out: [] u8) -> int {
  out[0] = cast(u8) op; if fin { out[0] += 0x80; }
  at := 2;
  if count < 126 { out[1] = cast(u8) (0x80 | count); }
  else if count < 65536 { out[1] = 0x80 | 126; out[2] = cast,trunc(u8) (count >> 8); out[3] = cast,trunc(u8) count; at = 4; }
  else { out[1] = 0x80 | 127; for k: 0..<8 { out[2 + k] = cast,trunc(u8) (count >> (8 * (7 - k))); } at = 10; }
  for k: 0..<4 { out[at + k] = 0; }
  at += 4;
  for k: 0..<count { out[at + k] = 'x'; }
  return at + count;
}
// Two fragments, first + second bytes, through ws_take into the relay's
// message buffer: the event and, on refusal, the close code.
Wire :: struct { bytes: [65560] u8; payload: [65537] u8; msg: [RS_MAX_MESSAGE] u8; }
two_fragments :: (w: [] Wire, first: int, second: int) -> int {
  r: [1] WsReader; ev := 0;
  for part: 0..<2 {
    n := fragment(part == 1, ifx part == 0 then WS_OP_TEXT else 0, ifx part == 0 then first else second, w[0].bytes[..]);
    f := wss_frame_read(cast(string) w[0].bytes[0..<n], RS_MAX_MESSAGE);
    #assert f.status == WS_FRAME_OK;
    p := wss_unmask(cast(string) w[0].bytes[0..<n], f, w[0].payload[..]);
    ev = ws_take(r[..], f, cast(string) w[0].payload[0..<p], w[0].msg[..]);
  }
  return ev;
}
// A REQ for sub id, one filter, read by ms_read: how the relay routes it and
// the reply when it refuses, into out.
req_reply :: (frame: string, out: [] u8) -> string {
  m := ms_read(frame);
  route := rr_message(m.error, m.type, m.has_sub);
  if route.action == RR_REQ { return "routed to REQ"; }
  if route.action == RR_COUNT { return "routed to COUNT (nip45.coax)"; }
  sub := m.sub;
  n := ifx route.action == RR_NOTICE then ms_write_notice(rs_prefix(route.reason), rs_text(route.reason), out)
    else ms_write_closed(cast(string) sub[0..<m.sub_n], rs_prefix(route.reason), rs_text(route.reason), out);
  return cast(string) out[0..<n];
}
// ["REQ","<count copies of unit>",{}] into out.
req_with_sub :: (unit: string, count: int, out: [] u8) -> string {
  n := json_raw(out, 0, "[\"REQ\",\"");
  for k: 0..<count { n = json_raw(out, n, unit); }
  n = json_raw(out, n, "\",{}]");
  return cast(string) out[0..<n];
}
// The effective limit of one filter's JSON.
limit_of :: (filter: string) -> int {
  strs: [8] string; ints: [8] int; tags: [4] NostrTagFilter; bytes: [256] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter;
  #assert filter_read(filter, f[..], sp[..]) == FR_OK;
  return rs_limit(f[0]);
}
// The admission policy's reply to one event before its signature: "passes
// to the signature check", or the OK false the relay sends.
policy_reply :: (event: string, authed: bool, out: [] u8) -> string {
  scratch: [8192] u8;
  a := rs_admit_read(event, NOW, rr_writes_need_auth(true), authed, scratch[..]);
  if a.reason == RS_ACCEPT { return "passes to the signature check"; }
  id := a.e.id;
  n := ms_write_ok(id[..], false, rs_prefix(a.reason), rs_text(a.reason), out);
  return cast(string) out[0..<n];
}

// The relay's own document with one field edited, into out.
edited :: (old: string, new: string, out: [] u8) -> string {
  doc := RR_NIP11; at := -1;
  for i: 0..<doc.count - old.count + 1 { if at < 0 && doc[i..<i + old.count] == old { at = i; } }
  #assert at >= 0;
  n := json_raw(out, 0, doc[0..<at]); n = json_raw(out, n, new); n = json_raw(out, n, doc[at + old.count..<doc.count]);
  return cast(string) out[0..<n];
}

NIP11_GET :: "GET / HTTP/1.1\r\nHost: relay.example\r\nAccept: application/nostr+json\r\n\r\n";
UPGRADE :: "GET / HTTP/1.1\r\nHost: relay.example\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";
BOTH :: "GET / HTTP/1.1\r\nAccept: application/nostr+json\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";

main :: () {
  print("routing: one request head, one answer\n");
  show_route("1. Accept: application/nostr+json", NIP11_GET);
  show_route("2. Upgrade: websocket", UPGRADE);
  show_route("3. both headers", BOTH);
  show_route("4. a browser's plain GET", "GET / HTTP/1.1\r\nHost: relay.example\r\nAccept: text/html\r\n\r\n");
  show_route("5. Accept with a list and q", "GET / HTTP/1.1\r\nAccept: text/html, Application/Nostr+JSON;q=0.9\r\n\r\n");
  show_route("6. Accept: application/nostr+jsonx", "GET / HTTP/1.1\r\nAccept: application/nostr+jsonx\r\n\r\n");
  show_route("7. OPTIONS", "OPTIONS / HTTP/1.1\r\nAccept: application/nostr+json\r\n\r\n");
  show_route("8. head not finished", "GET / HTTP/1.1\r\nAccept: application/nostr+json\r\n");

  out: [1024] u8;
  a := http_answer(NIP11_GET, out[..]);
  head := 0; while cast(string) out[head..<head + 4] != "\r\n\r\n" { head += 1; }
  head += 4;
  print("\nthe NIP-11 answer: % bytes, head % and body %\n", a.n, head, a.n - head);
  show_crlf(cast(string) out[0..<head]);
  print("  %\n", cast(string) out[head..<a.n]);
  b := http_answer("GET / HTTP/1.1\r\n\r\n", out[..]);
  print("\nthe 400 answer: % bytes\n", b.n);
  show_crlf(cast(string) out[0..<b.n]);

  print("\nthe client reads the document\n");
  i := info_read(RR_NIP11);
  print("  error: %, name \"%\", software \"%\", version \"%\"\n", INFO_ERRORS[i.error], i.name, i.software, i.version);
  print("  supported_nips:"); for k: 0..<i.nips_n { print(" %", i.nips[k]); } print("\n");
  print("  NIP-45 COUNT listed: %, NIP-50 search listed: %\n", info_has(i, 45), info_has(i, 50));
  print("advertised against enforced\n");
  print("  % mismatches\n", info_compare(i, true));

  print("\neach limit, at the limit and one past it\n");
  w: [1] Wire;
  print("max_message_length, one frame:\n");
  for length: [2] int.[RS_MAX_MESSAGE, RS_MAX_MESSAGE + 1] {
    h: [14] u8; h[0] = 0x81; h[1] = 0xff;
    for k: 0..<8 { h[2 + k] = cast,trunc(u8) (length >> (8 * (7 - k))); }
    fr := rr_frame_read(wss_frame_read(cast(string) h[..], RS_MAX_MESSAGE).status);
    print("  payload %: %\n", length, ifx fr.action == RR_F_WAIT then "header admitted, waiting for the payload" else "refused, close 1009");
    #assert fr.action == RR_F_WAIT || fr.code == WS_CLOSE_TOO_BIG;
  }
  print("max_message_length, two fragments:\n");
  wv := w[..];
  ev := two_fragments(wv, 65536, 65536);
  print("  65536 + 65536 = 131072: %\n", ifx ev == WS_EV_MESSAGE then "one message" else "refused");
  ev = two_fragments(wv, 65536, 65537);
  print("  65536 + 65537 = 131073: %\n", ifx ev == WS_EV_TOO_BIG then "refused, close 1009" else "one message");
  #assert ws_error_code(ev) == WS_CLOSE_TOO_BIG;

  print("max_subscriptions:\n");
  subs: [RS_MAX_SUBS] RsSub;
  sv := subs[..];
  for k: 0..<RS_MAX_SUBS { sv[k].live = false; }
  last := 0; text: [64] u8;
  for k0: 0..<RS_MAX_SUBS + 1 {
    k := k0 + 1;
    n := json_raw(text[..], 0, "[\"REQ\",\"s"); n = json_write_int(text[..], n, k); n = json_raw(text[..], n, "\",{}]");
    t := cast(string) text[0..<n];
    last = rs_sub_open(sv, ms_read(t), t, RS_MAX_SUBS);
    if k == RS_MAX_SUBS { print("  s1..s%: slots 0..%, % live\n", k, last, rs_subs_live(sv)); }
  }
  c := ms_write_closed("s33", rs_prefix(-last), rs_text(-last), out[..]);
  print("  s33: %\n", cast(string) out[0..<c]);
  again := "[\"REQ\",\"s1\",{\"kinds\":[1]}]";
  print("  s1 again (replaces it): slot %, % live\n", rs_sub_open(sv, ms_read(again), again, RS_MAX_SUBS), rs_subs_live(sv));

  print("max_subid_length:\n");
  q: [512] u8;
  print("  64 x \"a\": %\n", req_reply(req_with_sub("a", 64, q[..]), out[..]));
  print("  65 x \"a\": %\n", req_reply(req_with_sub("a", 65, q[..]), out[..]));
  print("  64 x \"\\u00e9\" (128 bytes): %\n", req_reply(req_with_sub("é", 64, q[..]), out[..]));

  print("max_limit and default_limit (clamped, never refused):\n");
  for f: [5] string.["{\"limit\":5}", "{\"limit\":127}", "{\"limit\":128}", "{\"limit\":5000}", "{}"] {
    print("  %: %\n", f, limit_of(f));
  }

  print("max_event_tags (now = %):\n", NOW);
  print("  256 tags: %\n", policy_reply(RST_TAGS256, true, out[..]));
  print("  257 tags: %\n", policy_reply(RST_TAGS, true, out[..]));
  print("created_at_upper_limit:\n");
  print("  created_at 1900: %\n", policy_reply(RST_EDGE, true, out[..]));
  print("  created_at 1901: %\n", policy_reply(RST_FUTURE, true, out[..]));
  print("restricted_writes and auth_required:\n");
  print("  EVENT, no AUTH yet: %\n", policy_reply(RST_PLAIN, false, out[..]));
  print("  EVENT, authenticated: %\n", policy_reply(RST_PLAIN, true, out[..]));
  print("  REQ, no AUTH: %\n", req_reply("[\"REQ\",\"open\",{\"kinds\":[1]}]", out[..]));
  print("supported_nips:\n");
  print("  NIP-40, expiration 1000 at now 1000: %\n", policy_reply(RST_EXPIRED, true, out[..]));
  print("  NIP-45 COUNT: %\n", req_reply("[\"COUNT\",\"c\",{}]", out[..]));
  sr := "[\"REQ\",\"s\",{\"search\":\"relay\"}]";
  v := rs_req(ms_read(sr), sr);
  c = ms_write_closed("s", rs_prefix(v), rs_text(v), out[..]);
  print("  NIP-50 search (not listed): %\n", cast(string) out[0..<c]);

  print("\nbroken documents\n");
  d: [1024] u8;
  i = info_read(edited("131072", "131073", d[..]));
  print("max_message_length edited to 131073: read %, % mismatch\n", INFO_ERRORS[i.error], info_compare(i, false));
  want := enforced();
  print("  max_message_length: advertised %, enforced %\n", i.limit[0], want[0]);
  i = info_read(edited("\"restricted_writes\":true", "\"restricted_writes\":false", d[..]));
  print("restricted_writes edited to false: read %, % mismatch\n", INFO_ERRORS[i.error], info_compare(i, false));
  i = info_read(edited("45,62", "45,50,62", d[..]));
  print("NIP-50 added to supported_nips: read %, % mismatch\n", INFO_ERRORS[i.error], info_compare(i, false));
  print("last byte cut: %\n", INFO_ERRORS[info_read(RR_NIP11[0..<RR_NIP11.count - 1]).error]);
  print("an array: %\n", INFO_ERRORS[info_read("[]").error]);
  print("\"supported_nips\":[\"11\"]: %\n", INFO_ERRORS[info_read("{\"supported_nips\":[\"11\"]}").error]);
  print("\"max_limit\":-1: %\n", INFO_ERRORS[info_read("{\"limitation\":{\"max_limit\":-1}}").error]);
  print("unknown fields skipped: %\n", INFO_ERRORS[info_read("{\"x\":[{\"y\":1}],\"limitation\":{\"z\":{},\"max_limit\":9}}").error]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/relay-info.coax
```

The output (booleans print as 0 and 1):

```text
routing: one request head, one answer
1. Accept: application/nostr+json: NIP11 -> NIP-11, answer "HTTP/1.1 200 OK"
2. Upgrade: websocket: OK -> upgrade, answer "HTTP/1.1 101 Switching Protocols"
3. both headers: OK -> upgrade, answer "HTTP/1.1 101 Switching Protocols"
4. a browser's plain GET: UPGRADE -> 400, answer "HTTP/1.1 400 Bad Request"
5. Accept with a list and q: NIP11 -> NIP-11, answer "HTTP/1.1 200 OK"
6. Accept: application/nostr+jsonx: UPGRADE -> 400, answer "HTTP/1.1 400 Bad Request"
7. OPTIONS: METHOD -> 400, answer "HTTP/1.1 400 Bad Request"
8. head not finished: MORE -> wait, answer ""

the NIP-11 answer: 714 bytes, head 197 and body 517
  HTTP/1.1 200 OK\r\n
  Content-Type: application/nostr+json\r\n
  Content-Length: 517\r\n
  Access-Control-Allow-Origin: *\r\n
  Access-Control-Allow-Headers: *\r\n
  Access-Control-Allow-Methods: GET\r\n
  Connection: close\r\n
  \r\n
  {"name":"OpenAgents Coax relay (R1)","description":"A Nostr relay written in Coax. Tailnet preview; relay.openagents.com is unchanged. Publishing needs NIP-42 AUTH; reading is open.","supported_nips":[1,9,11,40,42,45,62,70],"software":"https://github.com/OpenAgentsInc/openagents","version":"r1","limitation":{"max_message_length":131072,"max_subscriptions":32,"max_limit":127,"max_subid_length":64,"max_event_tags":256,"auth_required":false,"restricted_writes":true,"created_at_upper_limit":900,"default_limit":127}}

the 400 answer: 276 bytes
  HTTP/1.1 400 Bad Request\r\n
  Content-Type: text/plain\r\n
  Content-Length: 83\r\n
  Access-Control-Allow-Origin: *\r\n
  Access-Control-Allow-Headers: *\r\n
  Access-Control-Allow-Methods: GET\r\n
  Connection: close\r\n
  \r\n
  This is a Nostr relay: connect with a WebSocket, or ask for application/nostr+json.

the client reads the document
  error: ok, name "OpenAgents Coax relay (R1)", software "https://github.com/OpenAgentsInc/openagents", version "r1"
  supported_nips: 1 9 11 40 42 45 62 70
  NIP-45 COUNT listed: true, NIP-50 search listed: false
advertised against enforced
  max_message_length: advertised 131072, enforced 131072 ok
  max_subscriptions: advertised 32, enforced 32 ok
  max_limit: advertised 127, enforced 127 ok
  max_subid_length: advertised 64, enforced 64 ok
  max_event_tags: advertised 256, enforced 256 ok
  created_at_upper_limit: advertised 900, enforced 900 ok
  default_limit: advertised 127, enforced 127 ok
  auth_required: advertised 0, enforced 0 ok
  restricted_writes: advertised 1, enforced 1 ok
  supported_nips: 8 listed, relay implements 1, 9, 11, 40, 42, 45, 62, 70 ok
  0 mismatches

each limit, at the limit and one past it
max_message_length, one frame:
  payload 131072: header admitted, waiting for the payload
  payload 131073: refused, close 1009
max_message_length, two fragments:
  65536 + 65536 = 131072: one message
  65536 + 65537 = 131073: refused, close 1009
max_subscriptions:
  s1..s32: slots 0..31, 32 live
  s33: ["CLOSED","s33","restricted: too many active subscriptions"]
  s1 again (replaces it): slot 0, 32 live
max_subid_length:
  64 x "a": routed to REQ
  65 x "a": ["NOTICE","invalid: subscription id must contain 1 to 64 characters"]
  64 x "\u00e9" (128 bytes): routed to REQ
max_limit and default_limit (clamped, never refused):
  {"limit":5}: 5
  {"limit":127}: 127
  {"limit":128}: 127
  {"limit":5000}: 127
  {}: 127
max_event_tags (now = 1000):
  256 tags: passes to the signature check
  257 tags: ["OK","1cbac746adff2816c942a3ca190029aa4cb893d89c96c05a33e0b8819a161a9c",false,"invalid: event has too many tags"]
created_at_upper_limit:
  created_at 1900: passes to the signature check
  created_at 1901: ["OK","05e5d33da3af2eabf686cb09e2a8139b68a6e3e831c280396da89097f55eb0e5",false,"invalid: the event's created_at timestamp is too far in the past or the future for this relay"]
restricted_writes and auth_required:
  EVENT, no AUTH yet: ["OK","18c059d7cc15e21c516db0807a502f8ab482a23b7e9d64c27ecf4af5b010bc53",false,"auth-required: authenticate before publishing"]
  EVENT, authenticated: passes to the signature check
  REQ, no AUTH: routed to REQ
supported_nips:
  NIP-40, expiration 1000 at now 1000: ["OK","2428ab7235e4d5085e98c604a8bd041883da188674d85a63a2f808fd6b839c1b",false,"invalid: event has expired"]
  NIP-45 COUNT: routed to COUNT (nip45.coax)
  NIP-50 search (not listed): ["CLOSED","s","invalid: malformed filter"]

broken documents
max_message_length edited to 131073: read ok, 1 mismatch
  max_message_length: advertised 131073, enforced 131072
restricted_writes edited to false: read ok, 1 mismatch
NIP-50 added to supported_nips: read ok, 1 mismatch
last byte cut: not JSON
an array: not an object
"supported_nips":["11"]: bad supported_nips
"max_limit":-1: bad limitation field
unknown fields skipped: ok
```

The broken cases at the end:

- The document with `max_message_length` edited to 131073 still reads,
  and the comparison reports one mismatch: advertised 131073, enforced
  131072. A client that trusted it would send a 131,073-byte message and
  have its connection closed with 1009.
- `restricted_writes` edited to `false`, and NIP-50 added to
  `supported_nips`, are one mismatch each.
- The document without its last byte isn't JSON; `[]` isn't an object;
  `"supported_nips":["11"]` and `"max_limit":-1` are refused by field.
- Unknown fields, nested or not, are skipped and the known ones still read.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-info.coax](../examples/checks/relay-info.coax),
which loads the example, so running the example doesn't run them:

- the served answer equals, byte for byte, a head and document written out
  by hand from NIP-11's field list and the limits traced in chapter 34: 197
  + 517 = 714 bytes;
- `relay_server.coax`'s `RS_NIP11` equals `RR_NIP11`, the document
  `relay.coax` serves;
- with proof of work at 8 bits, `rr_nip11_pow` adds 13 as the fourth NIP
  and 26 bytes in all, and the comparison names that one difference;
- a request with both headers gets the RFC 6455 section 1.3 accept value,
  `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`, in a 129-byte `101`; the `400` answer is
  276 bytes;
- `info_read` gives the integers and booleans that Python's `json.loads`
  gives for the hand-written document;
- `enforced` equals the hand values, and editing each of the nine fields
  and `supported_nips` (one NIP removed at each end, one added, one replaced) gives
  exactly one mismatch; a missing `default_limit` reads as absent and is a
  mismatch;
- the reader's refusals, unknown fields skipped, and 32 NIPs read where 33
  are refused;
- each limit at its value and one past it, as in the worked table.

<!-- source: nostr/examples/checks/relay-info.coax check -->
```jai
// The document, written out by hand from the NIP-11 field list and the
// limits chapter 34 traced (frames of 131072 bytes, 256 tags, 900 s ahead)
// and the REQ limits in relay_server.coax's header comment (32
// subscriptions, sub ids 1..64, max_limit and default_limit 127).
HAND_DOC :: "{\"name\":\"OpenAgents Coax relay (R1)\",\"description\":\"A Nostr relay written in Coax. Tailnet preview; relay.openagents.com is unchanged. Publishing needs NIP-42 AUTH; reading is open.\",\"supported_nips\":[1,9,11,40,42,45,62,70],\"software\":\"https://github.com/OpenAgentsInc/openagents\",\"version\":\"r1\",\"limitation\":{\"max_message_length\":131072,\"max_subscriptions\":32,\"max_limit\":127,\"max_subid_length\":64,\"max_event_tags\":256,\"auth_required\":false,\"restricted_writes\":true,\"created_at_upper_limit\":900,\"default_limit\":127}}";
// The head counted by hand: 17 + 38 + 21 + 32 + 33 + 35 + 19 + 2 = 197 bytes.
HAND_HEAD :: "HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\nContent-Length: 517\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nAccess-Control-Allow-Methods: GET\r\nConnection: close\r\n\r\n";

#test "the served answer is the hand-written head and document, byte for byte" {
  out: [1024] u8;
  a := http_answer(NIP11_GET, out[..]);
  #assert a.status == WSS_NIP11 && a.action == RR_H_NIP11;
  #assert HAND_HEAD.count == 197 && HAND_DOC.count == 517 && a.n == 714;
  #assert cast(string) out[0..<197] == HAND_HEAD && cast(string) out[197..<714] == HAND_DOC;
}
#test "relay_server.coax's copy of the document is the one relay.coax serves" {
  #assert RS_NIP11 == RR_NIP11;
  out: [1024] u8;
  #assert rs_nip11(out[..]) == 714;
}
// RFC 6455 section 1.3's key and accept value.
// With --pow, the served list gains 13 in order and the limitation object
// min_pow_difficulty; the comparison against RELAY_NIPS then names one
// difference, the optional NIP-13.
#test "proof of work adds 13 to supported_nips" {
  out: [1024] u8;
  n := rr_nip11_pow(RR_NIP11, 8, out[..]);
  i := info_read(cast(string) out[0..<n]);
  #assert n == RR_NIP11.count + 3 + 23 && i.error == INFO_OK && i.nips_n == 9 && i.nips[3] == 13 && info_has(i, 13);
  #assert info_compare(i, false) == 1;
  #assert rr_nip11_pow(RR_NIP11, 0, out[..]) == RR_NIP11.count;
}
#test "routing: NIP-11, upgrade (which wins over NIP-11), refusal or wait" {
  out: [1024] u8;
  #assert http_answer(UPGRADE, out[..]).action == RR_H_UPGRADE;
  a := http_answer(BOTH, out[..]);
  #assert a.action == RR_H_UPGRADE && a.n == 129;
  #assert cast(string) out[0..<a.n] == "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
  #assert http_answer("GET / HTTP/1.1\r\n\r\n", out[..]).n == 276;
  #assert cast(string) out[0..<24] == "HTTP/1.1 400 Bad Request";
  #assert http_answer("OPTIONS / HTTP/1.1\r\n\r\n", out[..]).action == RR_H_BAD;
  #assert http_answer("GET / HTTP/1.1\r\nAccept: application/nostr+json\r\n", out[..]).action == RR_H_WAIT;
}
// A document Python's json module reads to the same values (json.loads on
// HAND_DOC; the integers and booleans below were copied from its output).
#test "the client reads every field the document states" {
  i := info_read(HAND_DOC);
  #assert i.error == INFO_OK && i.name == "OpenAgents Coax relay (R1)" && i.version == "r1";
  nips := [8] int.[1, 9, 11, 40, 42, 45, 62, 70];
  #assert i.nips_n == 8;
  for k: 0..<8 { #assert i.nips[k] == nips[k]; }
  want := [9] int.[131072, 32, 127, 64, 256, 900, 127, 0, 1];
  for k: 0..<9 { #assert i.limit[k] == want[k]; }
  #assert info_compare(i, false) == 0;
}
#test "every advertised value is the value the relay's code enforces" {
  want := enforced();
  hand := [9] int.[131072, 32, 127, 64, 256, 900, 127, 0, 1];
  for k: 0..<9 { #assert want[k] == hand[k]; }
}
#test "each edited field is one mismatch" {
  d: [1024] u8;
  edits := [8] string.["131072", "\"max_subscriptions\":32", "\"max_limit\":127", "\"max_subid_length\":64",
    "256", "900", "\"default_limit\":127", "\"auth_required\":false"];
  news := [8] string.["131073", "\"max_subscriptions\":33", "\"max_limit\":128", "\"max_subid_length\":65",
    "257", "901", "\"default_limit\":126", "\"auth_required\":true"];
  for k: 0..<8 {
    i := info_read(edited(edits[k], news[k], d[..]));
    #assert i.error == INFO_OK && info_compare(i, false) == 1;
  }
  i := info_read(edited("\"restricted_writes\":true", "\"restricted_writes\":false", d[..]));
  #assert i.error == INFO_OK && i.limit[8] == 0 && info_compare(i, false) == 1;
  i = info_read(edited(",70]", "]", d[..])); #assert info_compare(i, false) == 1;
  i = info_read(edited("45,62", "45,50,62", d[..])); #assert info_compare(i, false) == 1;
  i = info_read(edited("[1,9,", "[1,13,", d[..])); #assert info_compare(i, false) == 1;
  i = info_read(edited("[1,9,", "[1,", d[..])); #assert info_compare(i, false) == 1;
  i = info_read(edited("\"default_limit\":127", "\"x\":1", d[..])); #assert i.limit[6] == -1 && info_compare(i, false) == 1;
}
#test "the client's refusals" {
  #assert info_read("").error == INFO_JSON && info_read("{").error == INFO_JSON && info_read("{} x").error == INFO_JSON;
  #assert info_read("[]").error == INFO_SHAPE && info_read("1").error == INFO_SHAPE;
  #assert info_read("{}").error == INFO_OK && info_read("{}").limit[0] == -1 && info_read("{}").nips_n == 0;
  #assert info_read("{\"supported_nips\":1}").error == INFO_NIPS && info_read("{\"supported_nips\":[0]}").error == INFO_NIPS;
  #assert info_read("{\"supported_nips\":[1.5]}").error == INFO_NIPS && info_read("{\"supported_nips\":[1,]}").error == INFO_JSON;
  #assert info_read("{\"limitation\":[]}").error == INFO_LIMIT && info_read("{\"limitation\":{\"max_limit\":\"9\"}}").error == INFO_LIMIT;
  #assert info_read("{\"limitation\":{\"auth_required\":1}}").error == INFO_LIMIT && info_read("{\"limitation\":{\"max_limit\":1e3}}").error == INFO_LIMIT;
  #assert info_read("{\"limitation\":{\"max_limit\":0}}").limit[2] == 0;
  // NIP-11: clients MUST ignore fields they don't understand
  i := info_read("{\"icon\":\"x\",\"fees\":{\"admission\":[{\"amount\":1}]},\"name\":\"n\",\"limitation\":{\"payment_required\":true,\"max_limit\":9},\"supported_nips\":[]}");
  #assert i.error == INFO_OK && i.name == "n" && i.limit[2] == 9 && i.nips_n == 0;
  // the name only when it is a string
  #assert info_read("{\"name\":7}").error == INFO_OK && info_read("{\"name\":7}").name.count == 0;
  // INFO_MAX_NIPS entries are read; one more is refused
  long: [256] u8; m := json_raw(long[..], 0, "{\"supported_nips\":[");
  for k: 0..<INFO_MAX_NIPS { if k > 0 { m = json_raw(long[..], m, ","); } m = json_raw(long[..], m, "1"); }
  #assert info_read(cast(string) long[0..<json_raw(long[..], m, "]}")]).error == INFO_OK;
  m = json_raw(long[..], m, ",1]}");
  #assert info_read(cast(string) long[0..<m]).error == INFO_NIPS;
}
#test "each limit is hit at its value and refused one past it" {
  w: [1] Wire;
  #assert two_fragments(w[..], 65536, 65536) == WS_EV_MESSAGE && two_fragments(w[..], 65536, 65537) == WS_EV_TOO_BIG;
  out: [512] u8; q: [512] u8;
  #assert req_reply(req_with_sub("a", 64, q[..]), out[..]) == "routed to REQ";
  #assert req_reply(req_with_sub("a", 65, q[..]), out[..]) == "[\"NOTICE\",\"invalid: subscription id must contain 1 to 64 characters\"]";
  #assert req_reply(req_with_sub("é", 64, q[..]), out[..]) == "routed to REQ";
  #assert req_reply(req_with_sub("é", 65, q[..]), out[..]) != "routed to REQ";
  #assert limit_of("{\"limit\":127}") == 127 && limit_of("{\"limit\":128}") == 127 && limit_of("{\"limit\":126}") == 126 && limit_of("{}") == 127;
  #assert policy_reply(RST_TAGS256, true, out[..]) == "passes to the signature check";
  #assert policy_reply(RST_EDGE, true, out[..]) == "passes to the signature check";
  #assert policy_reply(RST_PLAIN, true, out[..]) == "passes to the signature check";
  #assert policy_reply(RST_PLAIN, false, out[..]) != "passes to the signature check";
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-info.coax
```

On 2026-11-01 `coax test` ran 279 checks, including the loaded units' own
checks, and all passed in 22 s on a loaded machine. The limit check is the largest, at 17.7
million steps, because it copies two 65,536-byte fragments. The checks
file has no procedures of its own, so `coax mutate --root-only` on it has
nothing to mutate; `info_read` and `info_compare` are example code, checked
by the refusal and one-field-edit checks above, and were not
mutation-tested.

### The library change

Before this chapter `relay_server.coax` held an older copy of the document,
`RS_NIP11`, that listed NIPs 1, 11 and 40 and said `restricted_writes:
false`. The relay never served it (`ry_http` sends `RR_NIP11`), but its
check claimed it stated the limits in force, and since NIP-42 writes it
didn't. The checks file's `RS_NIP11 == RR_NIP11` check failed first; then
`RS_NIP11` became the same bytes. No procedure changed.
`relay_server.coax`'s 251 checks pass, and on 2026-11-01
`coax mutate coax/lib/nostr/relay_server.coax --root-only` killed 340 of
340 mutants.

### The supported_nips update

The relay first served `[1,11,40,42]` while NIP-09, NIP-45, NIP-62 and
NIP-70 were already wired into it (chapter 103). `RR_NIP11` and
`RS_NIP11` now list those four as well, and `rr_nip11_pow` adds 13 with
`--pow`. The new procedure `rr_nip13_at` finds where 13 goes. On
2026-11-01, `coax mutate --root-only` killed 438 of 438 mutants in
`relay_route.coax` and 344 of 344 in `relay_server.coax`.

### Records not rerun

`ws_server.coax` is unchanged since these records, and `relay_route.coax`
changed only as above; the records were not rerun:

- [2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md):
  `relay_server.coax`'s whole graph, including `ws_server.coax`, 6,087 of
  6,087 mutants killed.
- [2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md):
  `relay_route.coax`, 255 of 255, and the Rust and Coax relays given the
  same observations, including the NIP-11 request.

The NIP-11 request over a real socket is in `relay_session.py`, which
checks the served `supported_nips`, with and without `--pow`. The wss://
session of
[2026-10-30-coax-relay-r1-tls.md](../../docs/verification/2026-10-30-coax-relay-r1-tls.md)
was not rerun.

## Summary

- One URL serves three answers: the NIP-11 document for a plain `GET` with
  `Accept: application/nostr+json`, the WebSocket handshake for
  `Upgrade: websocket` (which wins when both are present), and `400`
  otherwise.
- The answer is 714 bytes: a 197-byte head with `Content-Length: 517`, the
  three CORS headers and `Connection: close`, then the document.
- `supported_nips` lists 1, 9, 11, 40, 42, 45, 62 and 70, the relay-side
  NIPs the relay implements, and 13 when `--pow` is set; `search` is
  refused.
- Every `limitation` value is read from the code that enforces it, and
  each is hit at its value and refused one past it.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [27. A WebSocket server](ch027-a-websocket-server.md)

## Sources

- [NIP-11](../nips/official/11.md), Relay Information Document: the
  fields, the CORS headers and the `limitation` object.
- [NIP-01](../nips/official/01.md), [NIP-09](../nips/official/09.md),
  [NIP-13](../nips/official/13.md), [NIP-40](../nips/official/40.md),
  [NIP-42](../nips/official/42.md), [NIP-45](../nips/official/45.md),
  [NIP-62](../nips/official/62.md) and [NIP-70](../nips/official/70.md):
  the NIPs the relay lists.
- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) section 1.3 (the
  accept value) and section 7.4.1 (close code 1009).
- The Fetch standard's CORS-safelisted request headers:
  <https://fetch.spec.whatwg.org/#cors-safelisted-request-header>.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 17.
