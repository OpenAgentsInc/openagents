# 17. JSON you can sign

An event ID is the SHA-256 of one exact JSON array. Any difference in
whitespace or escaping changes the ID. This chapter builds the NIP-01
serialization on a bounded JSON writer, reads JSON back with a bounded pull
parser, and shows why RFC 8785 JCS is a different thing that never produces
an event preimage.

Part IV, Encodings. Result: A bounded JSON reader and writer that keep
original bytes, and the exact NIP-01 serialization `[0, pubkey, created_at,
kind, tags, content]` with its escape rules, distinct from RFC 8785 JCS.

## What this chapter teaches

- The preimage: `[0,pubkey,created_at,kind,tags,content]` with no
  whitespace, in UTF-8, with the seven escapes NIP-01 lists and `\u00XX` for
  the other control bytes.
- A pull parser that yields one token at a time with its byte range, under
  limits on input size, depth and string length, and that never truncates.
- The refusals of event admission: duplicate keys (also when spelled with an
  escape), lone surrogates, and `kind` or `created_at` outside
  `±(2^53 - 1)`.
- Why a relay keeps the original bytes of an event it accepted.
- RFC 8785 JCS for OpenAgents envelopes, and why it is never an event
  preimage.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [json.coax](../../coax/lib/nostr/json.coax) | `json_start`, `json_next` | Starts a parser over a byte view; returns the next token and its byte range |
| | `json_text`, `json_unescape` | The token's raw bytes; a string's raw bytes decoded into a caller buffer |
| | `json_valid` | True when the whole text is one JSON value |
| | `json_int` | A number token as an integer within `±2^53` |
| | `json_raw`, `json_write_string`, `json_write_int` | Append raw text, an escaped string or an integer to a caller buffer |
| [json-admission.coax](../../coax/lib/nostr/adapters/json-admission.coax) | `ja_admit(text, policy, frame, proto)` | Runs the parser and feeds every token to the admission policy |
| [json-policy.coax](../../coax/lib/nostr/adapters/json-policy.coax) | `ja_policy`, `ja_feed`, `ja_integer` | The `#authority` admission rules: budgets, duplicate keys, Unicode scalars, safe integers |
| [jcs.coax](../../coax/lib/nostr/jcs.coax) | `jcs_canonicalize_checked` | The integer-only RFC 8785 profile, `#authority` |

Nothing allocates. The parser state is a `Json` struct that the caller
owns, and every writer appends to a caller buffer and returns the new
length, or -1 when the output doesn't fit. A -1 stays -1 down a chain of
writer calls, so a caller checks once at the end.

## The preimage

NIP-01 defines the ID as the SHA-256 of the UTF-8 bytes of this array:

```jai
[0,<pubkey>,<created_at>,<kind>,<tags>,<content>]
```

- `0` is a fixed number.
- `pubkey` is the 32-byte public key as a string of 64 lowercase hex digits.
- `created_at` and `kind` are numbers.
- `tags` is an array of arrays of strings.
- `content` is a string.

There is no whitespace between tokens. Strings follow one escaping rule:

| Byte | Written as |
| --- | --- |
| `0x0a` line feed | `\n` |
| `0x22` double quote | `\"` |
| `0x5c` backslash | `\\` |
| `0x0d` carriage return | `\r` |
| `0x09` tab | `\t` |
| `0x08` backspace | `\b` |
| `0x0c` form feed | `\f` |
| any other byte below `0x20` | `\u00` and two lowercase hex digits |
| every other byte | as it is |

NIP-01 lists the first seven and says every other character is included
verbatim. A raw byte below `0x20` inside a string is not JSON (RFC 8259,
section 7), so no parser would accept it. The writer writes those bytes as
`\u00XX` with lowercase hex, which is what JavaScript's `JSON.stringify`
and Python's `json.dumps` do. `/`, `0x7f` and all non-ASCII UTF-8 bytes are
written as they are; nothing is written as a `\u` escape above `0x1f`.

`json_write_string` applies that table:

<!-- source: coax/lib/nostr/json.coax write-string -->
```jai
// s as a JSON string, escaped the way NIP-01 serializes an event: \" \\ \n
// \r \t \b \f, other control bytes as \u00XX, everything else as it is.
json_write_string :: (out: [] u8, at: int, s: string) -> int {
  n := json_raw(out, at, "\"");
  for i: 0..<s.count {
    if n == -1 { return -1; }
    c := s[i];
    if c == '"' { n = json_raw(out, n, "\\\""); }
    else if c == '\\' { n = json_raw(out, n, "\\\\"); }
    else if c == 10 { n = json_raw(out, n, "\\n"); }
    else if c == 13 { n = json_raw(out, n, "\\r"); }
    else if c == 9 { n = json_raw(out, n, "\\t"); }
    else if c == 8 { n = json_raw(out, n, "\\b"); }
    else if c == 12 { n = json_raw(out, n, "\\f"); }
    else if c < 0x20 {
      n = json_raw(out, n, "\\u00");
      digits := "0123456789abcdef";
      if n == -1 { return -1; }
      if n + 2 > out.count { return -1; }
      out[n] = digits[c >> 4]; out[n + 1] = digits[c & 15]; n += 2;
    } else {
      if n >= out.count { return -1; }
      out[n] = c; n += 1;
    }
  }
  return json_raw(out, n, "\"");
}
```

### Worked example: a small event

The event has the BIP-340 vector 1 public key, `created_at` 1761686400,
`kind` 1, one tag `["t","nostr"]` and an 8-byte content:

```text
68 69 0a 22 78 22 5c 01
h  i  LF "  x  "  \  0x01
```

Escape each byte by the table:

| Byte | Written | Bytes out |
| --- | --- | ---: |
| `68` | `h` | 1 |
| `69` | `i` | 1 |
| `0a` | `\n` | 2 |
| `22` | `\"` | 2 |
| `78` | `x` | 1 |
| `22` | `\"` | 2 |
| `5c` | `\\` | 2 |
| `01` | `\u0001` | 6 |

The escaped content is 17 bytes, and 19 with its quotes. Now count the
whole array, piece by piece:

| Piece | Bytes | Starts at byte |
| --- | ---: | ---: |
| `[0,` | 3 | 0 |
| `"dff1d77f…502ba659"` | 66 | 3 |
| `,` | 1 | 69 |
| `1761686400` | 10 | 70 |
| `,` | 1 | 80 |
| `1` | 1 | 81 |
| `,` | 1 | 82 |
| `[["t","nostr"]]` | 15 | 83 |
| `,` | 1 | 98 |
| `"hi\n\"x\"\\\u0001"` | 19 | 99 |
| `]` | 1 | 118 |

The preimage is 119 bytes:

```text
[0,"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",1761686400,1,[["t","nostr"]],"hi\n\"x\"\\\u0001"]
```

Python's `hashlib.sha256` of those 119 bytes is

```text
ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
```

and that is the event's ID. The example below computes the same bytes and
the same hash in Coax.

## Reading JSON

`json_next` is a pull parser. Each call returns one token: `{`, `}`, `[`,
`]`, a string, a number, `true`, `false` or `null`, then `JSON_END` once the
one value and its trailing whitespace are done. Each token comes with its
byte range in `from` and `to`. For a string the range is its raw bytes
between the quotes, still escaped; `json_unescape` decodes them into a
caller buffer only when the caller needs the value. A key is a string token
with `key` set.

The parser checks everything as it goes:

- Strings: no raw byte below `0x20`, only the escapes `\" \\ \/ \b \f \n \r
  \t` and `\u` with four hex digits, and UTF-8 with no overlong forms, no
  encoded surrogates and nothing above U+10FFFF.
- Numbers: the JSON grammar, so no leading zero, no `+`, no bare `.`.
- Structure: commas and colons where the grammar puts them, so `[1,]` and
  `{"a":1,}` refuse, and nothing after the one value but whitespace.
- Limits: the input at most 1 MiB, depth at most 16, a string at most 64 KiB
  of raw bytes. `json_start_bounded` lets a caller set other input and
  string limits.

On the first byte that breaks a rule, the parser returns `JSON_ERROR`,
records the byte offset in `error_at`, and returns `JSON_ERROR` on every
later call. It never truncates a string or skips a bad value.

The parser doesn't decide what a value means. Two jobs are left to later
layers:

- A lone surrogate such as `"\ud800"` is valid JSON syntax, so the parser
  accepts the token. `json_unescape` refuses it, because it is not a Unicode
  scalar and has no UTF-8 encoding.
- Duplicate keys are valid JSON syntax too. The parser doesn't remember
  keys; the admission policy does.

### Integers

A Nostr `created_at` or `kind` is an integer. JavaScript stores numbers as
IEEE doubles, which hold every integer only up to `2^53`. `json_int` reads
an integer token and refuses fractions, exponents, leading zeros and values
beyond `±2^53`:

<!-- source: coax/lib/nostr/json.coax int -->
```jai
// A number token's text as an integer: no fraction or exponent, |n| <= 2^53.
JsonInt :: struct { ok: bool; value: int; }
JSON_INT_MOST :: 9007199254740992;
json_int :: (s: string) -> JsonInt {
  r:JsonInt;if s.count==0 {return r;}
  i:=0;negative:=false;if s[0]=='-' {negative=true;i=1;}
  if i>=s.count {return r;}
  if s[i]=='0' && s.count-i>1 {return r;}
  v:=0;
  while i<s.count {
    if !json_digit(s[i]) {return r;}
    digit:=cast(int)(s[i]-'0');
    if v>(JSON_INT_MOST-digit)/10 {return r;}
    v=v*10+digit;i+=1;
  }
  return JsonInt.{ok=true,value=ifx negative then -v else v};
}
```

`json_int` admits `2^53` itself. Event admission is one step stricter:
`ja_integer` in `json-policy.coax` admits only `±(2^53 - 1)`, the safe
integers, because `2^53 + 1` reads as `2^53` in a double, so two different
texts would mean the same number.

## Event admission

`ja_admit(text, ja_policy(0), frame_bytes, 0)` runs the parser over a whole
event and feeds every token to `ja_feed`, the policy for domain 0, a Nostr
event. It returns one code:

| Code | Name | Meaning |
| ---: | --- | --- |
| 0 | `JA_OK` | admitted |
| 2 | `JA_SIZE` | over a size, depth or frame budget |
| 3 | `JA_JSON` | the parser refused the text |
| 4 | `JA_STRING` | a string doesn't decode to Unicode scalars, or is too long |
| 5 | `JA_DUPLICATE` | a key appears twice in one object |
| 8 | `JA_NUMBER` | `kind` or `created_at` isn't a safe integer |

The policy compares decoded keys, so `{"kind":1,"kind":2}` is a
duplicate: `k` is `k`. The same key in two different objects is
allowed. A `kind` of `1.0` or `"1"` refuses, while an unknown field may
hold any JSON number and keeps its bytes.

### Keep the original bytes

Admission produces no new text. A relay that accepts an event stores the
bytes it received. To check the ID, chapter 21 decodes the fields and
writes the preimage with the writer above, then compares the hash with the
`id` field. The two texts differ: an event may arrive with spaces, with
fields in any order, or with content written as `"hi"`, and its
preimage still writes `"hi"`. Writing a stored event from decoded fields
would change bytes that the author never signed, so the relay serves the
original.

## JCS is not the preimage

RFC 8785, the JSON Canonicalization Scheme, turns any JSON object into one
canonical text: no whitespace, object members sorted by their UTF-16 code
units, shortest escapes and a fixed number format. OpenAgents envelopes
hash JCS text ([envelope.coax](../../coax/lib/nostr/envelope.coax)).

`jcs_canonicalize_checked` implements a profile of it: integers within
`±(2^53 - 1)` only, with duplicates, lone surrogates and every fraction or
exponent refused (`JCS_NUMBER`, code 8). Its string escapes are the same as
the NIP-01 writer's, since it calls `json_write_string`.

An event ID is not JCS. The preimage is an array in a fixed order, starts
with `0`, and has no `id` or `sig`; JCS of the event object sorts the keys
`content`, `created_at`, `id`, `kind`, ... and hashes something else. The
two must never be swapped: a JCS hash of an event is not its ID, and the
NIP-01 array is not a canonical form of anything but an event.

## The example

The example is [nostr/examples/signable-json.coax](../examples/signable-json.coax).
It writes the worked event's preimage and hashes it, writes one byte of
each escape class, reads the preimage back token by token, and shows the
refusals of the parser and of event admission, then two JCS calls.

<!-- source: nostr/examples/signable-json.coax example -->
```jai
#load "../../coax/lib/nostr/adapters/json-admission.coax";
#load "../../coax/lib/nostr/jcs.coax";
#load "../../coax/lib/crypto/sha256.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex that the program itself supplies; it must be valid.
bytes_of :: (text: string, out: [] u8) -> string {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}

// The NIP-01 ID preimage [0,pubkey,created_at,kind,tags,content] into out.
// The tags are one flat list of strings; sizes[i] says how many belong to
// tag i. Returns the length, or -1 when out is too small.
id_preimage :: (out: [] u8, pubkey: string, created_at: int, kind: int,
                tags: [] string, sizes: [] int, content: string) -> int {
  n := json_raw(out, 0, "[0,");
  n = json_write_string(out, n, pubkey);
  n = json_raw(out, n, ",");
  n = json_write_int(out, n, created_at);
  n = json_raw(out, n, ",");
  n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",[");
  next := 0;
  for t: 0..<sizes.count {
    if t > 0 { n = json_raw(out, n, ","); }
    n = json_raw(out, n, "[");
    for v: 0..<sizes[t] {
      if v > 0 { n = json_raw(out, n, ","); }
      n = json_write_string(out, n, tags[next]);
      next += 1;
    }
    n = json_raw(out, n, "]");
  }
  n = json_raw(out, n, "],");
  n = json_write_string(out, n, content);
  return json_raw(out, n, "]");
}

TOKEN :: [11] string.["error", "end", "{", "}", "[", "]", "string", "number", "true", "false", "null"];
JA :: [9] string.["ok", "policy", "size", "json", "string", "duplicate key", "keys", "work", "number"];

// Read text to its end and print where it was refused, if it was.
try_parse :: (label: string, text: string) {
  j: [1] Json; json_start(j[..], text);
  t := json_next(j[..]);
  while t != JSON_END && t != JSON_ERROR { t = json_next(j[..]); }
  if t == JSON_END { print("%: valid\n", label); }
  else { print("%: refused at byte %\n", label, j[0].error_at); }
}
// The admission policy for a Nostr event (domain 0) on a whole text.
try_admit :: (label: string, text: string) {
  print("%: %\n", label, JA[ja_admit(text, ja_policy(0), text.count, 0)]);
}

PUBKEY :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";

main :: () {
  // The worked event: content is h i LF " x " \ 0x01.
  raw: [8] u8;
  content := bytes_of("68690a2278225c01", raw[..]);
  tags := [2] string.["t", "nostr"];
  sizes := [1] int.[2];
  out: [256] u8;
  n := id_preimage(out[..], PUBKEY, 1761686400, 1, tags[..], sizes[..], content);
  print("content bytes: %\n", content.count);
  print("preimage: %\n", cast(string) out[0..<n]);
  print("preimage bytes: %\n", n);
  id: [32] u8;
  #assert sha256_hash_checked(out[0..<n], id[..], false) == SHA_OK;
  show("id", id[..]);

  // One byte of each class through the writer.
  print("escapes:");
  for c: [12] u8.[10, 34, 92, 13, 9, 8, 12, 0, 1, 31, 47, 127] {
    one: [1] u8; one[0] = c;
    w: [8] u8;
    k := json_write_string(w[..], 0, cast(string) one[..]);
    print(" %=", c);
    if c == 127 { print("raw"); } else { print("%", cast(string) w[1..<k - 1]); }
  }
  print("\n");
  small: [18] u8; fit: [19] u8;
  print("content into 18 bytes: %, into 19 bytes: %\n",
        json_write_string(small[..], 0, content), json_write_string(fit[..], 0, content));

  // Read the preimage back, token by token.
  text := cast(string) out[0..<n];
  j: [1] Json; json_start(j[..], text);
  print("tokens:");
  t := json_next(j[..]);
  while t != JSON_END && t != JSON_ERROR {
    print(" %@%", TOKEN[t + 1], j[0].from);
    t = json_next(j[..]);
  }
  print(" %\n", TOKEN[t + 1]);
  // The content string is the last string token; unescape it.
  json_start(j[..], text);
  // Token 12: [ 0 pubkey created_at kind [ [ "t" "nostr" ] ] content.
  for i: 0..<12 { t = json_next(j[..]); }
  back: [64] u8;
  m := json_unescape(json_text(j[..]), back[..]);
  print("content raw in the text: % bytes, unescaped: % bytes, same as the original: %\n",
        j[0].to - j[0].from, m, cast(string) back[0..<m] == content);
  ca := json_int("1761686400");
  print("created_at as an integer: ok %, value %\n", ca.ok, ca.value);

  // Refusals by the reader.
  try_parse("trailing comma [1,]", "[1,]");
  try_parse("raw line feed in a string", "\"a\nb\"");
  try_parse("unknown escape \\x41", "\"\\x41\"");
  try_parse("leading zero 01", "01");
  try_parse("two values", "1 2");
  deep: [17] u8; for i: 0..<17 { deep[i] = '['; }
  try_parse("17 nested arrays", cast(string) deep[..]);
  try_parse("lone surrogate \\ud800", "\"\\ud800\"");
  lone: [8] u8;
  print("lone surrogate unescaped: %\n", json_unescape("\\ud800", lone[..]));
  print("2^53 + 1 as an integer: ok %\n", json_int("9007199254740993").ok);

  // Refusals by event admission.
  try_admit("duplicate key", "{\"kind\":1,\"kind\":2}");
  try_admit("duplicate key by escape", "{\"kind\":1,\"\\u006bind\":2}");
  try_admit("lone surrogate", "{\"content\":\"\\ud800\"}");
  try_admit("kind 2^53 - 1", "{\"kind\":9007199254740991}");
  try_admit("kind 2^53", "{\"kind\":9007199254740992}");
  try_admit("kind 1.0", "{\"kind\":1.0}");
  try_admit("trailing comma", "{\"kind\":1,}");

  // JCS sorts object members; NIP-01 is a fixed array.
  jcs: [128] u8;
  r := jcs_canonicalize_checked("{\"pubkey\":\"ab\",\"kind\":1,\"content\":\"\\u0068i\"}", jcs[..], false);
  print("JCS: error %, %\n", r.error, cast(string) jcs[0..<r.written]);
  r = jcs_canonicalize_checked("{\"created_at\":1.5}", jcs[..], false);
  print("JCS of 1.5: error %\n", r.error);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/signable-json.coax
```

The output matches the hand calculation: 119 bytes, the same text and the
same ID. The token offsets are the starts in the table above, plus one for
each string because a string's range starts after its quote:

```text
content bytes: 8
preimage: [0,"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",1761686400,1,[["t","nostr"]],"hi\n\"x\"\\\u0001"]
preimage bytes: 119
id: ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
escapes: 10=\n 34=\" 92=\\ 13=\r 9=\t 8=\b 12=\f 0=\u0000 1=\u0001 31=\u001f 47=/ 127=raw
content into 18 bytes: -1, into 19 bytes: 19
tokens: [@0 number@1 string@4 number@70 number@81 [@83 [@84 string@86 string@90 ]@96 ]@97 string@100 ]@118 end
content raw in the text: 17 bytes, unescaped: 8 bytes, same as the original: true
created_at as an integer: ok true, value 1761686400
trailing comma [1,]: refused at byte 3
raw line feed in a string: refused at byte 2
unknown escape \x41: refused at byte 1
leading zero 01: refused at byte 1
two values: refused at byte 2
17 nested arrays: refused at byte 16
lone surrogate \ud800: valid
lone surrogate unescaped: -1
2^53 + 1 as an integer: ok false
duplicate key: duplicate key
duplicate key by escape: duplicate key
lone surrogate: string
kind 2^53 - 1: ok
kind 2^53: number
kind 1.0: number
trailing comma: json
JCS: error 0, {"content":"hi","kind":1,"pubkey":"ab"}
JCS of 1.5: error 8
```

The refusals:

- `[1,]` refuses at byte 3, the `]` where a value must come.
- A raw line feed inside a string refuses at byte 2, and `\x41` at byte 1,
  the backslash of an escape JSON doesn't have.
- `01` reads the number `0`, then refuses at byte 1 because the value is
  complete; `1 2` refuses at byte 2 for the same reason.
- Seventeen `[` refuse at byte 16, the one that would make depth 17.
- `"\ud800"` is valid syntax, and `json_unescape` returns -1 for it.
- `json_int` refuses `2^53 + 1`.
- Admission refuses both duplicate keys, the lone surrogate (`string`),
  `kind` `2^53` and `1.0` (`number`) and the trailing comma (`json`), and
  admits `kind` `2^53 - 1`.
- JCS sorts `pubkey`, `kind`, `content` into `content`, `kind`, `pubkey`,
  writes `hi` as `hi`, and refuses `1.5` with code 8.
- The escaped content needs 19 bytes; with 18 the writer returns -1.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/signable-json.out](../fixtures/signable-json.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/signable-json.coax](../examples/checks/signable-json.coax),
which loads the example, so running the example doesn't run them. They
compare the code with independent values:

- four events whose fields hold every escape class: all 32 control bytes,
  the quote and backslash, `/`, `0x7f`, `é`, `€` and an emoji in content,
  and tabs, quotes, backslashes, an empty string and control bytes in tags.
  Each preimage and its SHA-256 must equal the bytes that Python 3's
  `json.dumps(..., separators=(",", ":"), ensure_ascii=False)` and
  `hashlib` produced, recorded as hex. One byte less of output must refuse.
- the content string of each preimage must read back to the original bytes.
- the nine JSONTestSuite files pinned in
  [coax/tests/vectors/upstream/json/](../../coax/tests/vectors/upstream/json/LICENSE)
  are embedded as hex, must hash to the SHA-256 pinned in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md), and must give their
  expected result: the three `y_` files parse, the four `n_` files refuse.
  The two `i_` files leave the choice to the parser; Coax refuses both,
  because a UTF-16 byte order mark is not UTF-8 and 500 nested arrays
  exceed depth 16.

<!-- source: nostr/examples/checks/signable-json.coax check -->
```jai
// Four events whose fields hold every escape class: all 32 control bytes,
// the quote, the backslash, and bytes written as they are (/, 0x7f, é, €,
// an emoji). The expected preimages and IDs come from Python 3's
// json.dumps(..., separators=(",", ":"), ensure_ascii=False) and hashlib,
// recorded here as hex; Python is not needed to run the checks.
EVENTS :: 4;
EV_PUBKEY :: [4] string.["dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
    "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"];
EV_CREATED_AT :: [4] int.[1761686400, 1761686400, 0, 9007199254740991];
EV_KIND :: [4] int.[1, 1, 0, 30023];
// Event i has EV_TAG_COUNT[i] tags; TAG_SIZES from EV_TAG_FROM[i] gives
// each tag's number of strings, read in order from TAG_HEX.
EV_TAG_FROM :: [4] int.[0, 1, 1, 4];
EV_TAG_COUNT :: [4] int.[1, 0, 3, 1];
TAG_SIZES :: [5] int.[2, 2, 3, 2, 2];
TAG_HEX :: [11] string.["74", "6e6f737472", "65", "610962", "70", "712272", "5c", "78", "", "64", "0d0a080c1f"];
EV_CONTENT :: [4] string.["68690a2278225c01",
    "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f225c2f7fc3a9e282acf09f9880",
    "",
    "0d0a080c20706c61696e202f2074657874"];
EV_PREIMAGE :: [4] string.[
    "5b302c2264666631643737663261363731633566333631383337323664623233343162653538666561653164613264656365643834333234306637623530326261363539222c313736313638363430302c312c5b5b2274222c226e6f737472225d5d2c2268695c6e5c22785c225c5c5c7530303031225d",
    "5b302c2264666631643737663261363731633566333631383337323664623233343162653538666561653164613264656365643834333234306637623530326261363539222c313736313638363430302c312c5b5d2c225c75303030305c75303030315c75303030325c75303030335c75303030345c75303030355c75303030365c75303030375c625c745c6e5c75303030625c665c725c75303030655c75303030665c75303031305c75303031315c75303031325c75303031335c75303031345c75303031355c75303031365c75303031375c75303031385c75303031395c75303031615c75303031625c75303031635c75303031645c75303031655c75303031665c225c5c2f7fc3a9e282acf09f9880225d",
    "5b302c2237396265363637656639646362626163353561303632393563653837306230373032396266636462326463653238643935396632383135623136663831373938222c302c302c5b5b2265222c22615c7462225d2c5b2270222c22715c2272222c225c5c225d2c5b2278222c22225d5d2c22225d",
    "5b302c2237396265363637656639646362626163353561303632393563653837306230373032396266636462326463653238643935396632383135623136663831373938222c393030373139393235343734303939312c33303032332c5b5b2264222c225c725c6e5c625c665c7530303166225d5d2c225c725c6e5c625c6620706c61696e202f2074657874225d"];
EV_ID :: [4] string.["ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3",
    "665d776489bcc3d514c975f570a937822eebb9e7a255e2252ef26d7990208612",
    "6619027c835fd86e929b3760ae6347c306b2f77069eef6510a84e974a0235709",
    "26084b40e4aec0d2f4e5aacb55844b9631918fea0397c2ecdd4d436779880f6e"];

// Event i's preimage written by id_preimage.
event_preimage :: (i: int, out: [] u8) -> int {
  arena: [64] u8; tags: [16] string; used := 0; next := 0;
  for t: 0..<EV_TAG_FROM[i] { next += TAG_SIZES[t]; }
  values := 0;
  for t: 0..<EV_TAG_COUNT[i] { values += TAG_SIZES[EV_TAG_FROM[i] + t]; }
  // Each value gets its own bytes of the arena; the views must not overlap.
  hex_digits := 0;
  for v: 0..<values {
    tags[v] = bytes_of(TAG_HEX[next + v], arena[used..<64]);
    used += tags[v].count;
    hex_digits += TAG_HEX[next + v].count;
  }
  #assert 2 * used == hex_digits;
  content_bytes: [64] u8;
  content := bytes_of(EV_CONTENT[i], content_bytes[..]);
  sizes: [8] int;
  for t: 0..<EV_TAG_COUNT[i] { sizes[t] = TAG_SIZES[EV_TAG_FROM[i] + t]; }
  return id_preimage(out, EV_PUBKEY[i], EV_CREATED_AT[i], EV_KIND[i], tags[0..<values],
                     sizes[0..<EV_TAG_COUNT[i]], content);
}
#forall "each preimage and ID match Python byte for byte" i: 0..<EVENTS {
  got: [512] u8; want: [512] u8;
  n := event_preimage(i, got[..]);
  expected := bytes_of(EV_PREIMAGE[i], want[..]);
  #assert n == expected.count && cast(string) got[0..<n] == expected;
  #assert json_valid(cast(string) got[0..<n]);
  id: [32] u8; want_id: [32] u8;
  #assert sha256_hash_checked(got[0..<n], id[..], false) == SHA_OK;
  #assert cast(string) id[..] == bytes_of(EV_ID[i], want_id[..]);
  // One byte too few refuses instead of truncating.
  #assert event_preimage(i, got[0..<n - 1]) == -1;
}
#forall "the content string reads back to the original bytes" i: 0..<EVENTS {
  got: [512] u8; n := event_preimage(i, got[..]);
  j: [1] Json; json_start(j[..], cast(string) got[0..<n]);
  last: string; t := json_next(j[..]);
  while t != JSON_END && t != JSON_ERROR {
    if t == JSON_STRING { last = json_text(j[..]); }
    t = json_next(j[..]);
  }
  #assert t == JSON_END;
  back: [64] u8; original: [64] u8;
  m := json_unescape(last, back[..]);
  #assert m >= 0 && cast(string) back[0..<m] == bytes_of(EV_CONTENT[i], original[..]);
}

// The JSONTestSuite files pinned in coax/tests/vectors/upstream/json/, as
// hex. Each must hash to its pin in coax/tests/vectors/SOURCES.md. y_ files
// must parse and n_ files must be refused. i_ files leave the choice to the
// parser; Coax refuses both (a UTF-16 byte order mark is not UTF-8, and 500
// nested arrays exceed depth 16).
SUITE :: 9;
SUITE_NAME :: [9] string.["i_string_UTF-16LE_with_BOM.json",
    "i_structure_500_nested_arrays.json",
    "n_object_trailing_comma.json",
    "n_string_incomplete_escape.json",
    "n_string_invalid-utf-8-in-escape.json",
    "n_string_unescaped_ctrl_char.json",
    "y_number_int_with_exp.json",
    "y_string_escaped_control_character.json",
    "y_string_utf8.json"];
// The 500 nested arrays file is 500 "[" then 500 "]", built below.
SUITE_HEX :: [9] string.["fffe5b002200e90022005d00",
    "",
    "7b226964223a302c7d",
    "5b225c225d",
    "5b225c75e5225d",
    "5b22610061225d",
    "5b323065315d",
    "5b225c7530303132225d",
    "5b22e282acf09d849e225d"];
SUITE_SHA256 :: [9] string.["6a9c15ecc8fc3da72b0ba5e3539e07f2aad3c704b496fe3496579dc723ce49c5",
    "a6c6b45361ff77e7372a015a1f0289e9c09d1fe1ed59cf9773d599c55acf57cf",
    "60e04bc71fcc4c57abeef95a9604a96aa60f833881a2f028d27bd4f03e176603",
    "df3709e1b19bc2562a9bc0301265c237ca5a5b35bd9bc5c3a7f45c57092e0537",
    "1c1ef70149258ff7ee082817269431c45e34f75a2268c3dff56f35d3a4859f1e",
    "c85db33857913bc4c4d4f044c72cd06d50ffb5644b2e5093ef8b46ebfe340ed0",
    "6d014f6467daa9753d57f0d75067fe62c9f3b0116455a3222c26e58432541b0d",
    "1a9f37085203c6d712945c5fb87ad5e08fa38515e51ee4a637c7558bb1b82725",
    "adec79032c3509dce21c9b0b96445a0fe398533d6647d12abc639eef16e621ab"];
SUITE_VALID :: [9] bool.[false, false, false, false, false, false, true, true, true];
#forall "the pinned JSONTestSuite files give their expected results" i: 0..<SUITE {
  buf: [1000] u8; count := 1000;
  if SUITE_HEX[i].count > 0 { count = bytes_of(SUITE_HEX[i], buf[..]).count; }
  else { for k: 0..<500 { buf[k] = '['; buf[500 + k] = ']'; } }
  text := cast(string) buf[0..<count];
  digest: [32] u8; pinned: [32] u8;
  #assert sha256_hash_checked(buf[0..<count], digest[..], false) == SHA_OK;
  #assert cast(string) digest[..] == bytes_of(SUITE_SHA256[i], pinned[..]);
  #assert json_valid(text) == SUITE_VALID[i];
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/signable-json.coax
coax/bin/coax mutate nostr/examples/checks/signable-json.coax --root-only
```

`coax test` runs 96 checks, including the loaded files' own checks, in
about 13 seconds. On 2026-11-01 root-only mutation killed 28 of 28 mutants
in the checks file. A run with the example and the checks in one file
killed 58 of 58 in `id_preimage`, `bytes_of` and the checks; the printing
helpers `show`, `try_parse` and `try_admit` are checked by the transcript
instead.

### The library's own checks

`json.coax` carries 20 checks, including a `#test` over 267 JSONTestSuite
cases (every `y_` parses, every `n_` refuses) generated into
[json_suite.coax](../../coax/lib/nostr/fixtures/json_suite.coax) from the
pinned commit `1ef36fa0`. The records give the results on 2026-10-28:

- [coax-json-decoder-checks-2026-10-28.md](../../docs/verification/coax-json-decoder-checks-2026-10-28.md):
  `coax mutate` killed 971 of 971 mutants in the parser and writer, and
  [json_parser.py](../../coax/tests/nostr/json_parser.py) compared token
  kinds, ranges, error offsets, decoded strings and writer output with a
  reference on 42,328 cases per build mode under ASan and UBSan.
- [coax-nostr-json-admission-2026-10-28.md](../../docs/verification/coax-nostr-json-admission-2026-10-28.md):
  `coax mutate` killed 336 of 336 mutants in `json-policy.coax`, and
  [json_admission.py](../../coax/tests/nostr/json_admission.py) compared
  admission with Python's JSON handling on 3,363 cases per build mode.

Those runs were not repeated for this chapter. Since then `json.coax`
gained only the `// book:` comment markers that this chapter's listings
use.

## Summary

- The ID preimage is `[0,pubkey,created_at,kind,tags,content]` with no
  whitespace; strings escape `\n \" \\ \r \t \b \f`, write other control
  bytes as `\u00XX` and every other byte as it is.
- The parser yields tokens with byte ranges, refuses at the first bad byte
  and never truncates; limits are 1 MiB, depth 16 and 64 KiB per string.
- Event admission refuses duplicate decoded keys, lone surrogates and
  `kind` or `created_at` outside `±(2^53 - 1)`.
- A relay stores and serves the original bytes; the preimage is rebuilt
  only to check the ID.
- JCS sorts object members and canonicalizes envelopes; it never makes an
  event preimage.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [4. SHA-256](ch004-sha-256.md), for the ID in the example
- [Coax chapter 18, The protocol boundary](../../coax/book/ch18-protocol.md)

## Sources

- [NIP-01](../nips/official/01.md), the serialization rules.
- [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259), The JavaScript Object
  Notation (JSON) Data Interchange Format.
- [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785), JSON Canonicalization
  Scheme (JCS).
- [JSONTestSuite](https://github.com/nst/JSONTestSuite) at commit
  `1ef36fa01286573e846ac449e8683f8833c5b26a`, pinned in
  [coax/tests/vectors/upstream/json/](../../coax/tests/vectors/upstream/json/LICENSE).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
