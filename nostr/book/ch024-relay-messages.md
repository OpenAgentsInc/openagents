# 24. Relay messages

Clients and relays exchange JSON arrays over a WebSocket. This chapter reads
and writes every NIP-01 frame and the machine-readable reason prefixes.

Part VI, The event model. Result: Client frames `EVENT`, `REQ`, `CLOSE`,
`AUTH` and `COUNT`, relay frames `EVENT`, `OK`, `EOSE`, `CLOSED`, `NOTICE` and
`AUTH`, and the `OK`/`CLOSED` reason prefixes NIP-20 introduced.

## What this chapter teaches

- Frame shapes, with spans into the original frame.
- `OK` with `true`/`false` and a message; prefixes `duplicate:`, `pow:`,
  `blocked:`, `rate-limited:`, `invalid:`, `restricted:`, `mute:`, `error:`,
  `auth-required:`.
- `CLOSED` for a refused or ended subscription.
- A parsed `EVENT` frame carries an unverified event.

## Where the code is

There are two codecs, one for each end of the socket. Each reads what the
other side sends and writes what its own side sends.

| File | Procedure | What it does |
| --- | --- | --- |
| [message.coax](../../coax/lib/nostr/message.coax) | `nostr_message_read(text)` | The client's reader: one relay frame (`EVENT`, `OK`, `EOSE`, `CLOSED`, `NOTICE`) into a `NostrMessage` of spans into `text`, or a refusal (`#authority`) |
| | `nostr_message_event_verify_vartime(text, out, overlap)` | Reads an `EVENT` frame and verifies the event it carries with chapter 21's verifier |
| | `nostr_message_write_event`, `_req`, `_close` | The client's writers: `["EVENT",<event>]`, `["REQ",<sub>,<filter>...]` and `["CLOSE",<sub>]`; each returns the frame's length or -1 |
| [message_server.coax](../../coax/lib/nostr/message_server.coax) | `ms_read(text)` | The relay's reader: one client frame (`EVENT`, `REQ`, `CLOSE`, `AUTH`, `COUNT`) into an `MsMsg` with the decoded subscription id and the byte ranges of the event or filters (`#authority`) |
| | `ms_write_event`, `ms_write_ok`, `ms_write_eose`, `ms_write_closed`, `ms_write_notice` | The relay's writers; each returns the frame's length or -1 |
| [message_agree.coax](../../coax/lib/nostr/message_agree.coax) | `ma_prefixes`, `ma_sub`, `ma_relay_frames`, `ma_client_frames` | Ties the two codecs together: the same bounds, the same prefixes, and each side reads what the other writes. The relay doesn't build when they disagree |

`message.coax` loads chapter 23's `filter.coax`, so the client writes `REQ`
filters with `nf_write`. `message_server.coax` loads only `json.coax`: the
relay's reader finds where each filter and event is, and the relay then reads
them with chapter 23's `filter_read` and chapter 21's event reader. Neither
codec decides policy. Whether to accept an event or a subscription is the
relay's job (chapters 33 to 36).

The `AUTH` frame belongs to NIP-42 (chapter 38). `ms_read` reports a client
`AUTH` by its verb and the byte range of its event; `nostr_message_read`
refuses a relay `AUTH` with `NM_TYPE`, and the host passes it to
`nip42.coax`. `COUNT` (NIP-45) is read by verb and filters only.

## Frames

Every frame is one JSON array whose first element is a string naming its
type. NIP-01 defines three client frames and five relay frames:

| From | Frame | Fields after the verb |
| --- | --- | --- |
| client | `["EVENT", <event>]` | one event object |
| client | `["REQ", <sub>, <filter>, ...]` | a subscription id and filters |
| client | `["CLOSE", <sub>]` | a subscription id |
| relay | `["EVENT", <sub>, <event>]` | the subscription the event answers, and the event |
| relay | `["OK", <id>, <true\|false>, <message>]` | the event id, the verdict and a message |
| relay | `["EOSE", <sub>]` | the subscription whose stored events are done |
| relay | `["CLOSED", <sub>, <message>]` | the subscription the relay ended or refused, and why |
| relay | `["NOTICE", <message>]` | text for a person |

Both readers follow the same steps:

1. Refuse a frame over 270,336 bytes (`NM_LIMIT`, `MS_TOO_BIG`) before
   parsing it.
2. Refuse text that isn't one complete JSON value (`NM_JSON`, `MS_JSON`).
   `nostr_message_read` validates the whole frame first, so a truncated
   frame never reads as a shorter valid one. `ms_read` reports a JSON error
   where it meets one. Both refuse anything after the closing `]`.
3. Read the verb. It is compared after JSON unescaping and is
   case-sensitive: `"EOSE"` is `EOSE`, `"eose"` is not. A verb the
   reader doesn't know refuses (`NM_TYPE`, `MS_VERB`). Each reader knows
   only the frames the other side sends: a client that receives `REQ`
   refuses it.
4. Read each field in order, with its JSON type checked, and require the
   array to end after the last one. A missing field, an extra field or a
   field of the wrong type refuses with `NM_SHAPE` or `MS_SHAPE`.

The readers copy nothing they don't have to. `NostrMessage` holds spans
(`NmSpan`, a `from` and a `to`) into the original frame: `sub` and `message`
cover the raw JSON string bytes between the quotes, and `event` covers the
event object from `{` to `}`. A span is valid only while the frame's bytes
are. `MsMsg` decodes the subscription id into its own 256-byte buffer,
because the relay keys subscriptions by it, and keeps byte ranges for the
event and each filter.

### Subscription ids

NIP-01 says a subscription id is "an arbitrary, non-empty string of max
length 64 chars". Both codecs count characters, not bytes: `nm_sub_ok` and
`ms_sub_ok` decode the JSON string, check that it is valid UTF-8, and count
the bytes that don't continue a UTF-8 sequence. 64 copies of `é` are 128
bytes and 64 characters, and pass. An empty id or a 65th character refuses
with `NM_SUB` or `MS_SUB`, and the writers return -1 for the same ids.

When `ms_read` refuses a `REQ` after it has read the subscription id, it
keeps the id and `has_sub`. The relay can then answer that subscription
with `CLOSED` instead of only a `NOTICE`.

### Bounds

- A `REQ` carries 1 to 16 filters when a client writes it. The relay's
  reader accepts 0 to 16 and refuses a 17th with `MS_TOO_MANY`.
- An `EOSE` may carry up to four trailing values after the subscription id.
  NIP-01 has none, but the deployed relay at relay.openagents.com appends
  `["finish"]` (see the captured session below). A fifth refuses with
  `NM_LIMIT`.
- An `OK` or `CLOSED` message is bounded by the JSON reader's longest
  string, 65,536 bytes.

## OK, CLOSED and the prefixes

`OK` answers a client's `EVENT`. NIP-01 says the fourth field "MUST always be
present, but MAY be an empty string when the 3rd is `true`, otherwise it MUST
be a string formed by a machine-readable single-word prefix followed by a `:`
and then a human-readable message." `CLOSED` uses the same message form, and
always needs a prefix. NIP-20 defined these command results first; NIP-01 now
holds them.

`nm_prefix` reads the decoded message:

- `""` is `NM_PREFIX_NONE`.
- A word of letters, digits, `-` and `_` followed by `:` is a prefix. The
  eight words NIP-01 standardizes, `duplicate`, `pow`, `blocked`,
  `rate-limited`, `invalid`, `restricted`, `mute` and `error`, read as 1 to
  8 in that order. Any other word, such as NIP-42's `auth-required` or the
  `unsupported` in NIP-01's own `CLOSED` example, reads as
  `NM_PREFIX_OTHER`. Words are case-sensitive: `Error:` is another word.
- Anything else, such as `no prefix here` or `two words: x`, has no prefix
  (-1).

A message with no prefix refuses an `OK false` and every `CLOSED` with
`NM_MESSAGE`, and so does an empty message there. An `OK true` takes any
message with a prefix or an empty one. The verdict must be the JSON literal
`true` or `false`; `1` and `"true"` refuse with `NM_SHAPE`. The id must be 64
lowercase hex digits, or the frame refuses with `NM_ID`.

The relay's writers are stricter than the reader. `ms_write_ok`,
`ms_write_closed` and `ms_write_notice` take a prefix and a text and write
`"<prefix>: <text>"`, and the prefix must be one of the eight words or
`auth-required` (`ms_prefix_ok`). The relay can't invent a ninth word, and it
can't refuse an event or close a subscription without a prefix. A `NOTICE` it
writes also has one, though a client reads a `NOTICE` of any text.

## A parsed EVENT is not a verified event

A relay `EVENT` frame's span is the event object exactly as the relay sent
it. `nostr_message_read` checks only that it is a JSON object. The event may
have a wrong id, a bad signature, or an author the client never asked for.
To use it, a client passes the span to chapter 21's reader and verifier, or
calls `nostr_message_event_verify_vartime`, which does both for one frame.
Even a verified event says nothing about whether the client asked for it or
whether it is new; a client checks it against its own filters (chapter 23)
before it trusts the subscription id.

The writers copy an event's original bytes. `nostr_message_write_event`
refuses bytes that don't parse as an event, but it doesn't verify them; the
relay does. `ms_write_event` copies the admitted event as it was stored, so
its id and signature still verify at the client.

## Worked example: a REQ and an OK by hand

A client asks for chapter 23's kind-1 events and `t`-tagged events on one
subscription named `feed`, with two filters:

```text
F1 = {"kinds":[1],"limit":2}       23 bytes
F2 = {"#t":["nostr"]}              16 bytes
```

The frame is the verb, the id, then each filter, separated by commas. Count
the bytes from 0:

| Bytes | Text | Field |
| --- | --- | --- |
| 0..7 | `["REQ",` | the array's `[`, the verb and a comma |
| 7..13 | `"feed"` | the subscription id, 4 characters |
| 13 | `,` | |
| 14..37 | `{"kinds":[1],"limit":2}` | filter 1, 14 + 23 = 37 |
| 37 | `,` | |
| 38..54 | `{"#t":["nostr"]}` | filter 2, 38 + 16 = 54 |
| 54 | `]` | the end; the frame is 55 bytes |

So the relay's reader should report subscription `feed` and two filters at
bytes 14..37 and 38..54 (each `to` is one past the last byte). Each range
then goes to `filter_read`.

The relay refuses one event with `invalid: bad signature`. NIP-01's example
id `b1a649eb...ce30` gives:

```text
["OK","b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30",false,"invalid: bad signature"]
```

Split it into its four fields:

1. The verb `"OK"`, bytes 1..5.
2. The id, a 66-byte JSON string at 6..72: 64 lowercase hex digits, so it
   decodes to 32 bytes starting `b1 a6` and ending `ce 30`.
3. The verdict `false`, bytes 73..78. It is a JSON literal, not a string.
4. The message at 79..103. Its text between the quotes is 80..102. The
   characters before the first `:` are `invalid`, the fifth standard word,
   so the prefix is 5. A refusal must have a prefix, and this one does.

The frame is 1 + 4 + 1 + 66 + 1 + 5 + 1 + 24 + 1 = 104 bytes.

## The example

The example is
[nostr/examples/relay-messages.coax](../examples/relay-messages.coax). It
loads `message_agree.coax`, which loads both codecs, and chapter 23's
`filter_read.coax`. It:

1. writes the worked `REQ` with the client's writer, compares it with the
   hand frame, reads it with the relay's reader, and reads each filter range
   back with `filter_read` and `nf_write`;
2. writes the worked `OK` with the relay's writer and reads it with the
   client's reader;
3. writes and reads every client frame and every relay frame, with the other
   side's reader;
4. shows the refusals of both readers and of the writers;
5. reads the captured relay session, all 243 frames.

`client_view` and `relay_view` print what the other side's reader returns:
the type, the subscription id, and the byte ranges or the message. The
events are the library's `NE_FIXTURE`, an event with escapes in its tags and
content, so the example shows that the bytes are copied, not rewritten.

<!-- source: nostr/examples/relay-messages.coax example -->
```jai
#load "../../coax/lib/nostr/message_agree.coax";
#load "../../coax/lib/nostr/filter_read.coax";
#import "Basic";

// Status names, in the order of each file's constants.
NM :: [10] string.["ok", "json", "shape", "type", "sub", "id", "message", "limit", "output", "alias"];
MS :: [7] string.["ok", "json", "shape", "verb", "sub", "too many", "too big"];
RELAY_TYPE :: [6] string.["-", "EVENT", "OK", "EOSE", "CLOSED", "NOTICE"];
CLIENT_VERB :: [6] string.["-", "EVENT", "REQ", "CLOSE", "AUTH", "COUNT"];
PREFIX :: [10] string.["none", "duplicate", "pow", "blocked", "rate-limited", "invalid", "restricted", "mute", "error", "another word"];

// NIP-01's example OK id, written out in full.
ID :: "b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30";

// The worked REQ's two filters (chapter 23's JSON).
F1 :: "{\"kinds\":[1],\"limit\":2}";
F2 :: "{\"#t\":[\"nostr\"]}";
Space :: struct { strs: [16] string; ints: [16] int; tags: [4] NostrTagFilter; bytes: [1024] u8; }
read_filter :: (text: string, f: [] NostrFilter, s: [] Space) -> int {
  sp: [1] FrSpace;
  sp[0] = FrSpace.{strs = s[0].strs[..], ints = s[0].ints[..], tags = s[0].tags[..], bytes = s[0].bytes[..]};
  return filter_read(text, f, sp[..]);
}

// What the client reads from one relay frame.
client_view :: (frame: string) {
  m := nostr_message_read(frame);
  if m.error != NM_OK { print("  client reads: refused, %\n", NM[m.error]); return; }
  print("  client reads: %", RELAY_TYPE[m.type]);
  if m.type == NM_EVENT || m.type == NM_EOSE || m.type == NM_CLOSED { print(", sub %", nm_text(m, m.sub)); }
  if m.type == NM_EVENT { print(", event bytes %..%", m.event.from, m.event.to); }
  if m.type == NM_EOSE && m.extra > 0 { print(", % extra values", m.extra); }
  if m.type == NM_ACCEPT { print(", id "); print_hex(m.id[0..<2]); print("..."); print_hex(m.id[30..<32]); print(", %", m.accepted); }
  if m.type == NM_ACCEPT || m.type == NM_CLOSED { print(", prefix %", PREFIX[m.prefix]); }
  if m.type != NM_EVENT && m.type != NM_EOSE { print(", message \"%\"", nm_text(m, m.message)); }
  print("\n");
}
print_hex :: (bytes: [] u8) {
  digits := "0123456789abcdef"; s: [2] u8;
  for b: bytes { s[0] = digits[cast(int) (b >> 4)]; s[1] = digits[cast(int) (b & 15)]; print("%", cast(string) s[..]); }
}
// What the relay reads from one client frame.
relay_view :: (frame: string) {
  m := ms_read(frame);
  if m.error != MS_OK { print("  relay reads: refused, %\n", MS[m.error]); return; }
  print("  relay reads: %", CLIENT_VERB[m.type]);
  if m.has_sub { print(", sub %", cast(string) m.sub[0..<m.sub_n]); }
  if m.type == MS_EVENT || m.type == MS_AUTH { print(", object bytes %..%", m.event_from, m.event_to); }
  if m.type == MS_REQ || m.type == MS_COUNT {
    print(", % filters", m.filters);
    for i: 0..<m.filters { print(" %..%", m.filter_from[i], m.filter_to[i]); }
  }
  print("\n");
}
// Print a written frame, or the writer's refusal.
show :: (label: string, n: int, out: [] u8) -> string {
  if n < 0 { print("%: writer refuses (-1)\n", label); return ""; }
  print("%: %\n", label, cast(string) out[0..<n]);
  return cast(string) out[0..<n];
}
client_refuses :: (label: string, frame: string) { print("client, %: %\n", label, NM[nostr_message_read(frame).error]); }
relay_refuses :: (label: string, frame: string) {
  m := ms_read(frame);
  print("relay, %: %", label, MS[m.error]);
  if m.error != MS_OK && m.has_sub { print(" (sub % already read)", cast(string) m.sub[0..<m.sub_n]); }
  print("\n");
}
// ["<verb>","<count copies of c>"] into out.
sub_frame :: (out: [] u8, verb: string, c: string, count: int) -> string {
  n := json_raw(out, 0, "[\""); n = json_raw(out, n, verb); n = json_raw(out, n, "\",\"");
  for i: 0..<count { n = json_raw(out, n, c); }
  n = json_raw(out, n, "\"]");
  #assert n > 0;
  return cast(string) out[0..<n];
}

main :: () {
  print("worked REQ\n");
  s: [2] Space; f: [2] NostrFilter;
  #assert read_filter(F1, f[0..<1], s[0..<1]) == FR_OK && read_filter(F2, f[1..<2], s[1..<2]) == FR_OK;
  req: [256] u8;
  frame := show("client writes", nostr_message_write_req("feed", f[..], req[..], false), req[..]);
  print("same bytes as the hand frame: %\n", frame == "[\"REQ\",\"feed\",{\"kinds\":[1],\"limit\":2},{\"#t\":[\"nostr\"]}]");
  relay_view(frame);
  m := ms_read(frame);
  for i: 0..<m.filters {
    span := frame[m.filter_from[i]..<m.filter_to[i]];
    g: [1] NostrFilter; t: [1] Space; out: [256] u8;
    r := read_filter(span, g[..], t[..]); w := nf_write(g[0], out[..], false);
    print("  filter %: %, filter_read %, written back %\n", i + 1, span, ifx r == FR_OK then "ok" else "refused",
          ifx w == span.count && cast(string) out[0..<w] == span then "byte for byte" else "differently");
  }

  print("worked OK\n");
  id: [32] u8; #assert ne_lower_hex(ID, id[..]);
  ok: [256] u8;
  frame = show("relay writes", ms_write_ok(id[..], false, "invalid", "bad signature", ok[..]), ok[..]);
  client_view(frame);

  print("client to relay\n");
  out: [1024] u8;
  frame = show("EVENT", nostr_message_write_event(NE_FIXTURE, out[..], false), out[..]);
  relay_view(frame);
  e := ms_read(frame);
  print("  object is the event's original bytes: %\n", frame[e.event_from..<e.event_to] == NE_FIXTURE);
  relay_view(show("REQ, one filter", nostr_message_write_req("q\"1", f[0..<1], out[..], false), out[..]));
  relay_view(show("CLOSE", nostr_message_write_close("feed", out[..], false), out[..]));
  print("AUTH: [\"AUTH\",{}]\n"); relay_view("[\"AUTH\",{}]");
  print("COUNT: [\"COUNT\",\"q\",{}]\n"); relay_view("[\"COUNT\",\"q\",{}]");

  print("relay to client\n");
  frame = show("EVENT", ms_write_event("feed", NE_FIXTURE, out[..]), out[..]);
  client_view(frame);
  ev := nostr_message_read(frame);
  print("  event is the original bytes: %\n", nm_text(ev, ev.event) == NE_FIXTURE);
  client_view(show("OK accepted", ms_write_ok(id[..], true, "", "", out[..]), out[..]));
  client_view(show("OK duplicate", ms_write_ok(id[..], true, "duplicate", "already have this event", out[..]), out[..]));
  client_view(show("OK blocked", ms_write_ok(id[..], false, "blocked", "you are banned from posting here", out[..]), out[..]));
  client_view(show("OK auth-required", ms_write_ok(id[..], false, "auth-required", "sign in first", out[..]), out[..]));
  client_view(show("EOSE", ms_write_eose("feed", out[..]), out[..]));
  print("EOSE with metadata: [\"EOSE\",\"feed\",[\"finish\"]]\n"); client_view("[\"EOSE\",\"feed\",[\"finish\"]]");
  client_view(show("CLOSED", ms_write_closed("feed", "restricted", "too many filters", out[..]), out[..]));
  client_view(show("NOTICE", ms_write_notice("error", "bad frame", out[..]), out[..]));
  print("NOTICE from another relay: [\"NOTICE\",\"slow down\"]\n"); client_view("[\"NOTICE\",\"slow down\"]");

  print("refusals the relay reads\n");
  buf: [256] u8;
  relay_refuses("[\"PING\"]", "[\"PING\"]");
  relay_refuses("[\"OK\",...] from a client", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",true,\"\"]");
  relay_refuses("sub of 64 a", sub_frame(buf[..], "CLOSE", "a", 64));
  relay_refuses("sub of 65 a", sub_frame(buf[..], "CLOSE", "a", 65));
  relay_refuses("sub of 64 é (128 bytes)", sub_frame(buf[..], "CLOSE", "é", 64));
  relay_refuses("[\"REQ\",\"\",{}]", "[\"REQ\",\"\",{}]");
  relay_refuses("[\"REQ\",1,{}]", "[\"REQ\",1,{}]");
  relay_refuses("[\"REQ\",\"s\",1]", "[\"REQ\",\"s\",1]");
  relay_refuses("[\"CLOSE\"]", "[\"CLOSE\"]");
  relay_refuses("[\"CLOSE\",\"s\",{}]", "[\"CLOSE\",\"s\",{}]");
  relay_refuses("[\"EVENT\",{},{}]", "[\"EVENT\",{},{}]");
  relay_refuses("[\"EVENT\",{}] x", "[\"EVENT\",{}] x");
  relay_refuses("{}", "{}");
  n := json_raw(buf[..], 0, "[\"REQ\",\"s\"");
  for k: 0..<17 { n = json_raw(buf[..], n, ",{}"); }
  n = json_raw(buf[..], n, "]");
  relay_refuses("REQ with 17 filters", cast(string) buf[0..<n]);
  buf[n - 4] = ']';
  relay_refuses("REQ with 16 filters", cast(string) buf[0..<n - 3]);

  print("refusals the client reads\n");
  client_refuses("OK with 1 for the verdict", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",1,\"\"]");
  client_refuses("OK with \"true\" for the verdict", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",\"true\",\"\"]");
  client_refuses("OK with three values", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",true]");
  client_refuses("OK with five values", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",true,\"\",\"x\"]");
  client_refuses("OK false with \"\"", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",false,\"\"]");
  client_refuses("OK false with no prefix", "[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",false,\"no prefix here\"]");
  client_refuses("OK with an uppercase id", "[\"OK\",\"B1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",true,\"\"]");
  client_refuses("OK with a 6-digit id", "[\"OK\",\"b1a649\",true,\"\"]");
  client_refuses("[\"eose\",\"a\"]", "[\"eose\",\"a\"]");
  client_refuses("[\"REQ\",\"a\",{}] from a relay", "[\"REQ\",\"a\",{}]");
  client_refuses("[\"EOSE\",\"\"]", "[\"EOSE\",\"\"]");
  client_refuses("sub of 65 s", sub_frame(buf[..], "EOSE", "s", 65));
  client_refuses("[\"CLOSED\",\"sub1\",\"\"]", "[\"CLOSED\",\"sub1\",\"\"]");
  client_refuses("[\"CLOSED\",\"sub1\"]", "[\"CLOSED\",\"sub1\"]");
  client_refuses("[\"EOSE\",\"s\",1,2,3,4,5]", "[\"EOSE\",\"s\",1,2,3,4,5]");
  client_refuses("[\"EOSE\",\"a\"", "[\"EOSE\",\"a\"");

  print("refusals the writers return\n");
  written: string;
  written = show("OK false with no prefix", ms_write_ok(id[..], false, "", "", out[..]), out[..]);
  written = show("CLOSED with prefix closed", ms_write_closed("feed", "closed", "bye", out[..]), out[..]);
  written = show("REQ with an empty sub", nostr_message_write_req("", f[..], out[..], false), out[..]);
  written = show("REQ with no filters", nostr_message_write_req("feed", f[0..<0], out[..], false), out[..]);
  written = show("EVENT of {}", nostr_message_write_event("{}", out[..], false), out[..]);
  written = show("CLOSE into 15 bytes", nostr_message_write_close("feed", out[0..<15], false), out[..]);

  print("captured session\n");
  from := 0; frames := 0; refused := 0; same := 0; verified := 0; extras := 0;
  counts: [6] int; scratch: [262144] u8;
  for i: 0..<COAX_RELAY.count + 1 {
    if i < COAX_RELAY.count && COAX_RELAY[i] != 10 { continue; }
    line := COAX_RELAY[from..<i]; from = i + 1;
    if line.count == 0 { continue; }
    frames += 1;
    r := nostr_message_read(line);
    if r.error != NM_OK { refused += 1; continue; }
    counts[r.type] += 1;
    if r.type == NM_EOSE { extras += r.extra; }
    if r.type != NM_EVENT { continue; }
    // Write the frame again from its parts; it must be the same bytes.
    w := ms_write_event(nm_text(r, r.sub), nm_text(r, r.event), scratch[..]);
    if w == line.count && cast(string) scratch[0..<w] == line { same += 1; }
    // The event span is a whole event: its id and signature verify.
    v := nostr_message_event_verify_vartime(line, scratch[..], false);
    if v.error == NE_OK && v.verified { verified += 1; }
  }
  print("% frames: % EVENT, % EOSE, % OK, % CLOSED, % NOTICE, % refused\n", frames, counts[NM_EVENT], counts[NM_EOSE], counts[NM_ACCEPT], counts[NM_CLOSED], counts[NM_NOTICE], refused);
  head := nm_line(COAX_RELAY, 0);
  print("refused frame: %..., %\n", COAX_RELAY[0..<9], NM[nostr_message_read(COAX_RELAY[head.from..<head.to]).error]);
  print("EVENT frames written again byte for byte: % of %\n", same, counts[NM_EVENT]);
  print("events whose id and signature verify: % of %\n", verified, counts[NM_EVENT]);
  print("EOSE metadata values: % in % frames\n", extras, counts[NM_EOSE]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/relay-messages.coax
```

The program prints:

```text
worked REQ
client writes: ["REQ","feed",{"kinds":[1],"limit":2},{"#t":["nostr"]}]
same bytes as the hand frame: true
  relay reads: REQ, sub feed, 2 filters 14..37 38..54
  filter 1: {"kinds":[1],"limit":2}, filter_read ok, written back byte for byte
  filter 2: {"#t":["nostr"]}, filter_read ok, written back byte for byte
worked OK
relay writes: ["OK","b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30",false,"invalid: bad signature"]
  client reads: OK, id b1a6...ce30, false, prefix invalid, message "invalid: bad signature"
client to relay
EVENT: ["EVENT",{"pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":0,"kind":1,"tags":[[],["x","\u00e9"],["x","/\n"]],"content":"a\"\\\n\r\t\b\f\u0000\u00e9\ud83d\ude00/","sig":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","id":"53d4f42ea78a74ab5ab47515040729f9ae9348f4ed05b59d07c7d24fb90116dc"}]
  relay reads: EVENT, object bytes 9..411
  object is the event's original bytes: true
REQ, one filter: ["REQ","q\"1",{"kinds":[1],"limit":2}]
  relay reads: REQ, sub q"1, 1 filters 14..37
CLOSE: ["CLOSE","feed"]
  relay reads: CLOSE, sub feed
AUTH: ["AUTH",{}]
  relay reads: AUTH, object bytes 8..10
COUNT: ["COUNT","q",{}]
  relay reads: COUNT, sub q, 1 filters 13..15
relay to client
EVENT: ["EVENT","feed",{"pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":0,"kind":1,"tags":[[],["x","\u00e9"],["x","/\n"]],"content":"a\"\\\n\r\t\b\f\u0000\u00e9\ud83d\ude00/","sig":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","id":"53d4f42ea78a74ab5ab47515040729f9ae9348f4ed05b59d07c7d24fb90116dc"}]
  client reads: EVENT, sub feed, event bytes 16..418
  event is the original bytes: true
OK accepted: ["OK","b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30",true,""]
  client reads: OK, id b1a6...ce30, true, prefix none, message ""
OK duplicate: ["OK","b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30",true,"duplicate: already have this event"]
  client reads: OK, id b1a6...ce30, true, prefix duplicate, message "duplicate: already have this event"
OK blocked: ["OK","b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30",false,"blocked: you are banned from posting here"]
  client reads: OK, id b1a6...ce30, false, prefix blocked, message "blocked: you are banned from posting here"
OK auth-required: ["OK","b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30",false,"auth-required: sign in first"]
  client reads: OK, id b1a6...ce30, false, prefix another word, message "auth-required: sign in first"
EOSE: ["EOSE","feed"]
  client reads: EOSE, sub feed
EOSE with metadata: ["EOSE","feed",["finish"]]
  client reads: EOSE, sub feed, 1 extra values
CLOSED: ["CLOSED","feed","restricted: too many filters"]
  client reads: CLOSED, sub feed, prefix restricted, message "restricted: too many filters"
NOTICE: ["NOTICE","error: bad frame"]
  client reads: NOTICE, message "error: bad frame"
NOTICE from another relay: ["NOTICE","slow down"]
  client reads: NOTICE, message "slow down"
refusals the relay reads
relay, ["PING"]: verb
relay, ["OK",...] from a client: verb
relay, sub of 64 a: ok
relay, sub of 65 a: sub
relay, sub of 64 é (128 bytes): ok
relay, ["REQ","",{}]: sub
relay, ["REQ",1,{}]: shape
relay, ["REQ","s",1]: shape (sub s already read)
relay, ["CLOSE"]: shape
relay, ["CLOSE","s",{}]: shape (sub s already read)
relay, ["EVENT",{},{}]: shape
relay, ["EVENT",{}] x: json
relay, {}: shape
relay, REQ with 17 filters: too many (sub s already read)
relay, REQ with 16 filters: ok
refusals the client reads
client, OK with 1 for the verdict: shape
client, OK with "true" for the verdict: shape
client, OK with three values: shape
client, OK with five values: shape
client, OK false with "": message
client, OK false with no prefix: message
client, OK with an uppercase id: id
client, OK with a 6-digit id: id
client, ["eose","a"]: type
client, ["REQ","a",{}] from a relay: type
client, ["EOSE",""]: sub
client, sub of 65 s: sub
client, ["CLOSED","sub1",""]: message
client, ["CLOSED","sub1"]: shape
client, ["EOSE","s",1,2,3,4,5]: limit
client, ["EOSE","a": json
refusals the writers return
OK false with no prefix: writer refuses (-1)
CLOSED with prefix closed: writer refuses (-1)
REQ with an empty sub: writer refuses (-1)
REQ with no filters: writer refuses (-1)
EVENT of {}: writer refuses (-1)
CLOSE into 15 bytes: writer refuses (-1)
captured session
243 frames: 237 EVENT, 5 EOSE, 0 OK, 0 CLOSED, 0 NOTICE, 1 refused
refused frame: ["AUTH","..., type
EVENT frames written again byte for byte: 237 of 237
events whose id and signature verify: 237 of 237
EOSE metadata values: 5 in 5 frames
```

What the output shows:

- The `REQ` the client writes is the hand frame byte for byte. The relay's
  reader finds `feed` and the filters at 14..37 and 38..54, as counted by
  hand, and each range reads with `filter_read` and writes back unchanged.
- The `OK` reads as the hand split: id `b1a6...ce30`, `false`, prefix
  `invalid`.
- Every client frame reads at the relay and every relay frame reads at the
  client. An event is the same bytes at both ends (`true` twice). The
  subscription id `q"1` is written with an escaped quote and decoded back by
  the relay. `auth-required` reads as another word, because it is not one of
  NIP-01's eight. An `EOSE` with `["finish"]` reads with one extra value.
- Each refusal has a status:

| Reader | Input | Status | Why |
| --- | --- | --- | --- |
| relay | `["PING"]`, an `OK` | `MS_VERB` | not a client verb |
| relay | 65 `a`, `""` | `MS_SUB` | an id is 1 to 64 characters; 64 `a` and 64 `é` pass |
| relay | `["REQ",1,{}]`, `["REQ","s",1]`, `["CLOSE"]`, `["CLOSE","s",{}]`, `["EVENT",{},{}]`, `{}` | `MS_SHAPE` | a field of the wrong type, a missing or extra field, not an array |
| relay | `["EVENT",{}] x` | `MS_JSON` | text after the frame |
| relay | 17 filters | `MS_TOO_MANY` | 16 is the bound; 16 pass |
| client | `OK` with `1` or `"true"` for the verdict, with three or five values, `["CLOSED","sub1"]` | `NM_SHAPE` | the verdict is a JSON literal, and `OK` has exactly four fields |
| client | `OK false ""`, `OK false "no prefix here"`, `CLOSED ""` | `NM_MESSAGE` | a refusal or a `CLOSED` needs a prefix |
| client | uppercase id, 6-digit id | `NM_ID` | 64 lowercase hex digits |
| client | `["eose","a"]`, a `REQ` | `NM_TYPE` | verbs are case-sensitive, and a client doesn't receive `REQ` |
| client | `["EOSE",""]`, 65 `s` | `NM_SUB` | the id bound |
| client | `EOSE` with five extra values | `NM_LIMIT` | at most four |
| client | `["EOSE","a"` | `NM_JSON` | a truncated frame |

  `MS_SHAPE` for `["REQ","s",1]`, `["CLOSE","s",{}]` and `MS_TOO_MANY` keep
  the id that was read, so the relay can send `CLOSED` for `s`.
- The writers return -1 for an `OK false` with no prefix, a `CLOSED` with
  the unknown prefix `closed`, a `REQ` with an empty id or no filters, an
  `EVENT` of `{}` (not an event), and a `CLOSE` with one byte too little
  room (it needs 16).
- The captured session is what relay.openagents.com sent for five
  subscriptions on 2026-10-28 (`coax/lib/nostr/fixtures/coax_relay.coax`,
  243 frames). The first frame is the relay's NIP-42 `AUTH` challenge, which
  the client's reader refuses with `NM_TYPE` and the host routes to
  chapter 38. The other 242 read: 237 `EVENT` and 5 `EOSE`. Every `EVENT`
  frame is written again by `ms_write_event` from its subscription and event
  spans and comes out the same bytes, and every event's id and signature
  verify. That is the plan's "captured relay sessions parse without loss".
  Each `EOSE` carries one `["finish"]` value, which `ms_write_eose` doesn't
  write, so those five are not rewritten.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/relay-messages.out](../fixtures/relay-messages.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-messages.coax](../examples/checks/relay-messages.coax),
which loads the example. They compare the code with values from outside it:

- NIP-01's own eleven `OK` examples and three `CLOSED` examples, each with
  the verdict and prefix read by hand from the NIP. NIP-01 shortens the id
  to `b1a649ebe8...`; the check uses the full 64 digits;
- the worked `REQ` and `OK` frames, with every byte offset counted by hand
  above;
- subscription ids of 0 to 65 characters drawn from `a`, `"`, `\` and `é`.
  The check builds the `CLOSE` frame it expects with its own escaping and
  compares bytes with `nostr_message_write_close`; the relay must decode the
  same id, and the client must read the relay's `EOSE` for it. An empty id
  and a 65-character id must refuse in the writer and in the reader.

<!-- source: nostr/examples/checks/relay-messages.coax check -->
```jai
// NIP-01's own OK and CLOSED examples, with each verdict and prefix read by
// hand from the NIP's list. NIP-01 shortens the id to "b1a649ebe8..."; the
// frames here carry the whole 64-digit id. CLOSED's "unsupported" is not one
// of the eight standard words, so it reads as another word.
NIP01_OK :: [11] string.[
  "\"\"", "\"pow: difficulty 25>=24\"", "\"duplicate: already have this event\"",
  "\"blocked: you are banned from posting here\"",
  "\"blocked: please register your pubkey at https://my-expensive-relay.example.com\"",
  "\"rate-limited: slow down there chief\"",
  "\"invalid: event creation date is too far off from the current time\"",
  "\"pow: difficulty 26 is less than 30\"", "\"restricted: not allowed to write.\"",
  "\"error: could not connect to the database\"",
  "\"mute: no one was listening to your ephemeral event and it wasn't handled in any way, it was ignored\""];
NIP01_OK_ACCEPTED :: [11] bool.[true, true, true, false, false, false, false, false, false, false, false];
NIP01_OK_PREFIX :: [11] int.[0, 2, 1, 3, 3, 4, 5, 2, 6, 8, 7];
NIP01_CLOSED :: [3] string.[
  "\"unsupported: filter contains unknown elements\"",
  "\"error: could not connect to the database\"",
  "\"error: shutting down idle subscription\""];
NIP01_CLOSED_PREFIX :: [3] int.[9, 8, 8];

#test "NIP-01's OK and CLOSED examples read with the hand verdicts and prefixes" {
  frame: [256] u8;
  for k: 0..<11 {
    n := json_raw(frame[..], 0, "[\"OK\",\""); n = json_raw(frame[..], n, ID);
    n = json_raw(frame[..], n, ifx NIP01_OK_ACCEPTED[k] then "\",true," else "\",false,");
    n = json_raw(frame[..], n, NIP01_OK[k]); n = json_raw(frame[..], n, "]");
    m := nostr_message_read(cast(string) frame[0..<n]);
    #assert m.error == NM_OK && m.type == NM_ACCEPT && m.accepted == NIP01_OK_ACCEPTED[k] && m.prefix == NIP01_OK_PREFIX[k];
    #assert m.id[0] == 0xb1 && m.id[1] == 0xa6 && m.id[31] == 0x30;
  }
  for k: 0..<3 {
    n := json_raw(frame[..], 0, "[\"CLOSED\",\"sub1\","); n = json_raw(frame[..], n, NIP01_CLOSED[k]); n = json_raw(frame[..], n, "]");
    m := nostr_message_read(cast(string) frame[0..<n]);
    #assert m.error == NM_OK && m.type == NM_CLOSED && nm_text(m, m.sub) == "sub1" && m.prefix == NIP01_CLOSED_PREFIX[k];
  }
}

#test "the worked frames, split by hand" {
  s: [2] Space; f: [2] NostrFilter; out: [256] u8;
  #assert read_filter(F1, f[0..<1], s[0..<1]) == FR_OK && read_filter(F2, f[1..<2], s[1..<2]) == FR_OK;
  n := nostr_message_write_req("feed", f[..], out[..], false);
  frame := cast(string) out[0..<n];
  // ["REQ", is bytes 0..6, "feed" 7..12, a comma at 13, then the filters:
  // F1 is 23 bytes at 14..37, a comma at 37, F2 16 bytes at 38..54, "]" at 54.
  #assert n == 55 && F1.count == 23 && F2.count == 16;
  #assert frame[0..<7] == "[\"REQ\"," && frame[7..<13] == "\"feed\"" && frame[13] == ',' && frame[37] == ',' && frame[54] == ']';
  m := ms_read(frame);
  #assert m.error == MS_OK && m.type == MS_REQ && m.sub_n == 4 && m.filters == 2;
  #assert m.filter_from[0] == 14 && m.filter_to[0] == 37 && m.filter_from[1] == 38 && m.filter_to[1] == 54;
  // The OK reply's four fields: "OK", the id, false, the message.
  id: [32] u8; #assert ne_lower_hex(ID, id[..]);
  n = ms_write_ok(id[..], false, "invalid", "bad signature", out[..]);
  frame = cast(string) out[0..<n];
  // [ "OK" , "<64>" , false , "invalid: bad signature" ]: 1+4+1+66+1+5+1+24+1.
  #assert n == 104 && frame[6..<72] == "\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\"" && frame[73..<78] == "false";
  r := nostr_message_read(frame);
  #assert r.error == NM_OK && !r.accepted && r.prefix == 5 && r.message.from == 80 && r.message.to == 102;
}

// A subscription id drawn from four characters: a, the quote and backslash
// that JSON escapes, and two-byte é. The check builds the CLOSE frame it
// expects by hand, escaping as RFC 8259 says, and compares bytes.
SUB_CHARS :: [4] string.["a", "\"", "\\", "é"];
SUB_JSON :: [4] string.["a", "\\\"", "\\\\", "é"];
#test "subscription ids of 1 to 65 characters, written and read by both codecs" {
  seed := 24;
  for count: 0..<66 {
    for trial: 0..<4 {
      sub: [260] u8; want: [300] u8; s := 0;
      w := json_raw(want[..], 0, "[\"CLOSE\",\"");
      for i: 0..<count {
        seed = (seed * 1103515245 + 12345) & 0x7fffffff; c := (seed >> 8) & 3;
        s = json_raw(sub[..], s, SUB_CHARS[c]); w = json_raw(want[..], w, SUB_JSON[c]);
      }
      w = json_raw(want[..], w, "\"]");
      id := cast(string) sub[0..<s]; out: [300] u8;
      n := nostr_message_write_close(id, out[..], false);
      if count < 1 || count > 64 { #assert n == -1 && ms_read(cast(string) want[0..<w]).error == MS_SUB; continue; }
      #assert n == w && cast(string) out[0..<n] == cast(string) want[0..<w];
      m := ms_read(cast(string) out[0..<n]);
      #assert m.error == MS_OK && m.type == MS_CLOSE && cast(string) m.sub[0..<m.sub_n] == id;
      // The relay names the same id in its EOSE, and the client reads it.
      e := ms_write_eose(id, out[..]);
      r := nostr_message_read(cast(string) out[0..<e]);
      #assert e > 0 && r.error == NM_OK && r.type == NM_EOSE && nm_text(r, r.sub) == cast(string) want[10..<w - 2];
    }
  }
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-messages.coax
coax/bin/coax mutate nostr/examples/checks/relay-messages.coax --root-only
```

On 2026-11-01, `coax test` ran 223 checks, including the loaded files' own
checks, in 6 seconds. Root-only mutation of the checks file reported no
mutation sites: the file holds only checks and constants, no procedures.
The example's procedures and the library were not mutated for this
chapter.

### The library's own checks

[message.coax](../../coax/lib/nostr/message.coax) checks the first frames
of the captured transcript, every prefix word and a `#forall` over all 256
byte values as a prefix character, every refusal status, the string and
frame bounds, a tampered id reaching the verifier, and the client writers'
exact bytes. [message_server.coax](../../coax/lib/nostr/message_server.coax)
checks every client frame and refusal, the 16-filter and frame bounds, and
the relay writers' exact bytes. [message_agree.coax](../../coax/lib/nostr/message_agree.coax)
fails the build when the two codecs' bounds or prefixes drift, and checks
that each side reads what the other writes.

The [2026-10-30 messages record](../../docs/verification/2026-10-30-nostr-messages.md)
reports, for `message.coax`, 214 of 214 checks, 4,162 of 4,162 mutants
killed with none marked equivalent, and a native run
(`coax/tests/nostr/message_native.coax`) that reads all 243 transcript
frames and verifies all 237 events. Those runs were not repeated for this
chapter.

## Summary

- A frame is one JSON array: a case-sensitive verb, then fixed fields. Each
  side reads only the frames the other side sends.
- The readers return spans into the original frame; nothing is copied but
  the relay's decoded subscription id.
- A subscription id is 1 to 64 characters of valid UTF-8, counted as
  characters.
- `OK` has an id, a JSON `true` or `false`, and a message. A refusal and
  every `CLOSED` need a one-word prefix and a colon; the eight standard
  words read as themselves, any other word as another word.
- A parsed `EVENT` carries an unverified event. Verify it before using it.
- The two codecs are tied by `message_agree.coax`: what one writes, the
  other reads, and their bounds can't drift.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [23. Filters](ch023-filters.md)

## Sources

- [NIP-01](../nips/official/01.md), the client and relay messages, the
  subscription id, and the `OK` and `CLOSED` messages with their prefixes.
- [NIP-20](../nips/official/20.md), the command results, now moved into
  NIP-01.
- [NIP-42](../nips/official/42.md), the `AUTH` frame and the
  `auth-required` prefix.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
