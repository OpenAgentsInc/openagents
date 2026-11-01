# 16. NIP-19 entities and nostr: URIs

People share Nostr keys and events as Bech32 strings, and NIP-21 puts them in
`nostr:` URIs. This chapter builds every NIP-19 form, including the TLV
records, and the NIP-21 rules about which forms a URI may carry.

Part IV, Encodings. Result: `npub`, `nsec` and `note`, and the TLV forms
`nprofile`, `nevent` and `naddr` with relay hints, plus the `nostr:` URI
scheme of NIP-21.

## What this chapter teaches

- Bare forms: `npub`, `nsec`, `note`, each exactly 32 bytes.
- TLV forms: type 0 special, 1 relay, 2 author, 3 kind; `nprofile`, `nevent`,
  `naddr`.
- Unknown TLV types are skipped, as NIP-19 says; lengths past the input
  refuse.
- NIP-21: `nostr:npub1…`; `nsec` is not allowed in a URI.
- Display only: a decoded entity is a pointer, not a verified event.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip19-core.coax](../../coax/lib/nostr/nip19-core.coax) | `n19_prefix(type)`, `n19_type(hrp)` | The six HRPs and their type numbers, `N19_NPUB` (1) to `N19_ADDR` (6) |
| | `n19_byte(source, at)` | Byte `at` of the payload, read straight from the Bech32 text or from raw bytes |
| | `n19_field(source, at)` | One TLV record at `at`: its type, the start and length of its value, and where the next record starts |
| | `n19_validate(source, type)` | The rules for one form, applied before anything is written |
| | `nip19_encode_checked`, `nip19_decode_checked` | Encode a payload, or decode a string, with caller buffers |
| | `nip21_decode_checked` | Decode a `nostr:` URI; refuses `nsec` |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `b32_inspect`, `bech32_encode_checked` | Chapter 15's checks and encoder |
| [bech32-memory.coax](../../coax/lib/nostr/bech32-memory.coax) | `nip19_encode`, `nip19_decode` | The exported entry points; they add the buffer-overlap check |

Every call takes caller-owned buffers. On an error it returns a code from
[codecs-common.coax](../../coax/lib/nostr/codecs-common.coax) (`NC_INVALID`,
`NC_CAPACITY`, `NC_LIMIT` or `NC_ALIAS`) and writes nothing. A decode also
returns an `N19_Result` that says where the fields are: `special_at` and
`special_length`, `author_at`, `kind` and the number of relays, each with a
`has_` flag. Decoding keeps the original TLV bytes and their order, so a
decoded payload re-encodes to the same string.

## The six forms

NIP-19 names six forms by their HRP. All use Bech32 with constant 1, as in
chapter 15, but NIP-19 raises the length limit from 90 to 5,000 characters,
because a TLV form with several relays is longer than 90 characters.
`b32_inspect(input, true)` applies the 5,000 limit, and
`N19_MAX_BYTES` (3,120) is the largest payload that fits in it.

| HRP | Payload | Use |
| --- | --- | --- |
| `npub` | 32-byte x-only public key | Share a profile |
| `nsec` | 32-byte secret key | Back up or move a key; never shared |
| `note` | 32-byte event ID | Share an event |
| `nprofile` | TLV: key and relays | Share a profile and where to find it |
| `nevent` | TLV: event ID, relays, optional author and kind | Share an event and where to find it |
| `naddr` | TLV: `d` identifier, relays, author and kind | Share an addressable event's coordinate |

The bare forms are the 32 bytes and nothing else. A payload of 31 or 33
bytes refuses, even when its Bech32 checksum is valid. A fifth TLV form,
`nrelay`, is deprecated; `n19_type` doesn't know it, so it refuses as an
unknown HRP.

These strings are for display and sharing only. NIP-19 says `npub` must not
appear in events or in NIP-05 JSON, which use hex. A decoded `nevent` is a
pointer: the client still has to fetch the event and check its ID and
signature (chapters 11 and 21).

## TLV records

The payload of `nprofile`, `nevent` and `naddr` is a list of records. Each
record is one type byte, one length byte, and that many value bytes. The
types are:

| Type | Name | `nprofile` | `nevent` | `naddr` |
| ---: | --- | --- | --- | --- |
| 0 | special | the public key, 32 bytes | the event ID, 32 bytes | the `d` identifier, any length, may be empty |
| 1 | relay | a relay URL in ASCII, any number | the same | the same |
| 2 | author | — | the author's key, 32 bytes, optional | the author's key, 32 bytes, required |
| 3 | kind | — | the kind, 4 bytes big-endian, optional | the kind, 4 bytes big-endian, required |

`n19_field` reads one record. It refuses when fewer than 2 bytes are left
for the type and length, or when the length runs past the end of the
payload:

<!-- source: coax/lib/nostr/nip19-core.coax validate -->
```jai
n19_field :: (source: N19_Source, at: int) -> N19_Field {
  if at < 0 || at > source.count { return N19_Field.{error = NC_INVALID}; }
  if at == source.count { return N19_Field.{done = true}; }
  if source.count - at < 2 { return N19_Field.{error = NC_INVALID}; }
  tag := n19_byte(source, at); length := cast(int) n19_byte(source, at + 1);
  if length > source.count - at - 2 { return N19_Field.{error = NC_INVALID}; }
  return N19_Field.{tag = tag, value = at + 2, length = length, next = at + 2 + length};
}
n19_validate :: (source: N19_Source, type: u32) -> N19_Result {
  if !n19_count(source.count) || type < 1 || type > 6 { return N19_Result.{error = NC_INVALID}; }
  r := N19_Result.{written = source.count, type = type};
  if type <= N19_NOTE {
    if source.count != 32 { return N19_Result.{error = NC_INVALID}; }
    r.has_special = true; r.special_length = 32;
    return r;
  }
  at := 0;
  while at < source.count {
    f := n19_field(source, at);
    if f.error != NC_OK { return N19_Result.{error = f.error}; }
    if f.tag == 0 {
      if r.has_special { return N19_Result.{error = NC_INVALID}; }
      if type != N19_ADDR && f.length != 32 { return N19_Result.{error = NC_INVALID}; }
      r.has_special = true; r.special_at = f.value; r.special_length = f.length;
    } else if f.tag == 1 {
      for i: 0..<f.length { if n19_byte(source, f.value + i) > 127 { return N19_Result.{error = NC_INVALID}; } }
      r.relays += 1;
    } else if f.tag == 2 && type != N19_PROFILE {
      if r.has_author || f.length != 32 { return N19_Result.{error = NC_INVALID}; }
      r.has_author = true; r.author_at = f.value;
    } else if f.tag == 3 && type != N19_PROFILE {
      if r.has_kind || f.length != 4 { return N19_Result.{error = NC_INVALID}; }
      r.has_kind = true;
      for i: 0..<4 { r.kind = (r.kind << 8) | cast(u32) n19_byte(source, f.value + i); }
    }
    // Unknown or unsupported fields are skipped without rewriting source bytes.
    at = f.next;
  }
  if !r.has_special { return N19_Result.{error = NC_INVALID}; }
  if type == N19_ADDR && (!r.has_author || !r.has_kind) { return N19_Result.{error = NC_INVALID}; }
  return r;
}
```

`n19_validate` walks the records and applies these rules:

1. Type 0 appears exactly once. For `nprofile` and `nevent` its length is
   32.
2. Every relay byte is ASCII, 0 to 127.
3. For `nevent` and `naddr`, type 2 appears at most once and is 32 bytes,
   and type 3 appears at most once and is 4 bytes.
4. `naddr` has both type 2 and type 3.
5. Any other type is skipped. NIP-19 says decoders ignore records they
   don't recognize. On an `nprofile`, types 2 and 3 are skipped too,
   because NIP-19 doesn't define them there.

The encoder runs the same `n19_validate` over the payload before it
encodes, so this code never writes a string it would refuse to read.

### Worked example: NIP-19's nprofile

NIP-19's example is:

```text
nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p
```

The HRP is `nprofile`. The data part is 122 characters; the last six are
the checksum, so there are 116 data words. 116 words are 580 bits: 72
bytes and 4 spare bits, which must be zero.

The first eight characters are `qqsrhuxx`. In chapter 15's alphabet they
are the words 0, 0, 16, 3, 23, 28, 6, 6. Written as 5-bit groups and
regrouped into bytes:

```text
words:  00000 00000 10000 00011 10111 11100 00110 00110
bits:   00000000 00100000 00111011 11110000 11000110
bytes:  00       20       3b       f0       c6
```

Byte 0 is the type, 0 (special). Byte 1 is the length, `0x20` = 32. The
value is bytes 2 to 33, and it starts `3b f0 c6`: the public key
`3bf0c63f…459d` that NIP-19 states. The next record starts at byte
2 + 32 = 34.

Decoding the rest the same way gives the whole payload. Byte 34 is `01`
(relay) and byte 35 is `0d` = 13, so bytes 36 to 48 are 13 ASCII
characters:

```text
77 73 73 3a 2f 2f 72 2e 78 2e 63 6f 6d
w  s  s  :  /  /  r  .  x  .  c  o  m
```

The next record starts at 36 + 13 = 49: `01`, then `15` = 21, then the 21
bytes of `wss://djbas.sadkb.com`, ending at 51 + 21 = 72, the end of the
payload. The three records:

| Offset | Type | Length | Value |
| ---: | ---: | ---: | --- |
| 0 | 0 | 32 | `3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d` |
| 34 | 1 | 13 | `wss://r.x.com` |
| 49 | 1 | 21 | `wss://djbas.sadkb.com` |

That is the profile NIP-19 says the string decodes to: one key and two
relays, 34 + 15 + 23 = 72 bytes.

## Decoding a string

`nip19_decode_checked` checks the whole string before it writes a byte:

<!-- source: coax/lib/nostr/nip19-core.coax decode -->
```jai
// Decodes one NIP-19 string. `secret` false refuses nsec from its HRP alone,
// before any byte is decoded or written.
n19_decode :: (input: string, out: [] u8, overlap: bool, secret: bool) -> N19_Result {
  h := b32_inspect(input, true);
  if h.error != NC_OK { return N19_Result.{error = h.error}; }
  type := n19_type(input[0..<h.hrp]);
  if type == 0 || (type == N19_NSEC && !secret) { return N19_Result.{error = NC_INVALID}; }
  last := b32_digit(b32_lower(input[h.hrp + h.words]));
  bytes := n19_byte_count(h.words, last);
  source := N19_Source.{input = input, start = h.hrp + 1, count = bytes, encoded = true};
  r := n19_validate(source, type);
  if r.error != NC_OK { return r; }
  room := NC_room(r.written, out.count, overlap);
  if room != NC_OK { return N19_Result.{error = room}; }
  for i: 0..<r.written { out[i] = n19_byte(source, i); }
  return r;
}
nip19_decode_checked :: (input: string, out: [] u8, overlap: bool) -> N19_Result {
  return n19_decode(input, out, overlap, true);
}
```

In order, it:

1. checks the Bech32 string with chapter 15's `b32_inspect`, with the
   5,000-character limit;
2. maps the HRP to a type, and refuses an unknown one;
3. turns the word count into a byte count, refusing bad padding;
4. validates the payload with `n19_validate`, reading bytes straight from
   the text with `n19_byte`;
5. checks that the payload fits the caller's buffer;
6. only then writes the bytes.

Because `n19_byte` reads from the text, an `nsec` never passes through a
temporary key array on its way to the caller's buffer.

## nostr: URIs

NIP-21 defines the URI scheme `nostr:`. What follows the scheme is a NIP-19
string, "except `nsec`". A secret key in a link would be sent to whatever
handles the link, so this code refuses it:

<!-- source: coax/lib/nostr/nip19-core.coax uri -->
```jai
// NIP-21: the scheme "nostr:" (in any case) and one NIP-19 string other than
// nsec, which a URI must never carry.
nip21_decode_checked :: (input: string, out: [] u8, overlap: bool) -> N19_Result {
  scheme := "nostr:";
  for i: 0..<scheme.count {
    if i >= input.count || b32_lower(input[i]) != scheme[i] { return N19_Result.{error = NC_INVALID}; }
  }
  return n19_decode(input[scheme.count..<input.count], out, overlap, false);
}
```

`nip21_decode_checked` compares the first six characters with `nostr:`,
ignoring case, as URI schemes are case-insensitive. It then decodes the
rest with `secret` set to false. `n19_decode` refuses an `nsec` as soon as
it has read the HRP, before it reads any payload byte, so the caller's
buffer keeps its old contents. A string without the scheme refuses, and so
does a second `nostr:` after the first.

## The example

The example is [nostr/examples/nip19.coax](../examples/nip19.coax). It
decodes NIP-19's three examples and re-encodes them, walks the nprofile's
records, decodes NIP-21's npub, nevent and naddr URIs, and shows the
refusals.

<!-- source: nostr/examples/nip19.coax example -->
```jai
#load "../../coax/lib/nostr/nip19-core.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [80] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// The name of a codec error code.
error_name :: (e: u32) -> string {
  if e == NC_OK { return "ok"; }
  if e == NC_INVALID { return "invalid"; }
  if e == NC_CAPACITY { return "capacity"; }
  if e == NC_LIMIT { return "limit"; }
  return "alias";
}
// Print every TLV record of a decoded nprofile, nevent or naddr: type,
// length and value. Relays and an naddr identifier are text; keys are hex.
walk :: (bytes: [] u8, r: N19_Result) {
  source := N19_Source.{input = cast(string) bytes, count = bytes.count};
  at := 0;
  while at < bytes.count {
    f := n19_field(source, at);
    value := bytes[f.value..<f.next];
    print("  at %: type %, length %, ", at, f.tag, f.length);
    if f.tag == 1 || (f.tag == 0 && r.type == N19_ADDR) {
      print("text \"%\"\n", cast(string) value);
    } else if f.tag == 3 {
      print("kind %\n", r.kind);
    } else {
      show("hex", value);
    }
    at = f.next;
  }
}
// Decode a nostr: URI and print its form and records.
try_uri :: (uri: string) {
  out: [128] u8;
  r := nip21_decode_checked(uri, out[..], false);
  if r.error != NC_OK { print("%...: %\n", uri[0..<16], error_name(r.error)); return; }
  print("%...: %, % bytes, % relays\n", uri[0..<16], n19_prefix(r.type), r.written, r.relays);
  if r.type <= N19_NOTE { show("  key", out[0..<32]); return; }
  walk(out[0..<r.written], r);
}
// Bech32-encode raw bytes under an HRP, with no NIP-19 rules applied, then
// decode the result as NIP-19 and print the outcome.
try_payload :: (label: string, hrp: string, payload: [] u8) {
  text: [400] u8; out: [128] u8;
  e := bech32_encode_checked(hrp, cast(string) payload, true, text[..], true, false);
  r := nip19_decode_checked(cast(string) text[0..<e.written], out[..], false);
  if r.error != NC_OK { print("%: %\n", label, error_name(r.error)); return; }
  print("%: ok, %, % relays\n", label, n19_prefix(r.type), r.relays);
}

// NIP-19's published examples. The nsec is the specification's example
// key, published there; it guards nothing.
NPUB :: "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg";
NSEC :: "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";
NPROFILE :: "nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p";
// NIP-21's examples.
URI_NPUB :: "nostr:npub1sn0wdenkukak0d9dfczzeacvhkrgz92ak56egt7vdgzn8pv2wfqqhrjdv9";
URI_NEVENT :: "nostr:nevent1qqstna2yrezu5wghjvswqqculvvwxsrcvu7uc0f78gan4xqhvz49d9spr3mhxue69uhkummnw3ez6un9d3shjtn4de6x2argwghx6egpr4mhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet5nxnepm";
URI_NADDR :: "nostr:naddr1qqyrzwrxvc6ngvfkqyghwumn8ghj7enfv96x5ctx9e3k7mgzyqalp33lewf5vdq847t6te0wvnags0gs0mu72kz8938tn24wlfze6qcyqqq823cph95ag";

main :: () {
  // The bare forms: exactly 32 bytes each, and back to the same text.
  out: [128] u8; text: [400] u8;
  r := nip19_decode_checked(NPUB, out[..], false);
  print("npub: %, % bytes\n", error_name(r.error), r.written);
  show("  key", out[0..<32]);
  e := nip19_encode_checked(N19_NPUB, cast(string) out[0..<32], text[..], false);
  print("  re-encodes to the same text: %\n", cast(string) text[0..<e.written] == NPUB);
  r = nip19_decode_checked(NSEC, out[..], false);
  print("nsec: %, % bytes\n", error_name(r.error), r.written);
  show("  key", out[0..<32]);
  e = nip19_encode_checked(N19_NSEC, cast(string) out[0..<32], text[..], false);
  print("  re-encodes to the same text: %\n", cast(string) text[0..<e.written] == NSEC);

  // The nprofile, word by word and byte by byte.
  hrp: [83] u8; words: [400] u8;
  d := bech32_decode_checked(NPROFILE, hrp[..], words[..], true, false);
  print("nprofile: % words, first 8:", d.written);
  for i: 0..<8 { print(" %", words[i]); }
  print("\n");
  profile: [128] u8;
  p := nip19_decode_checked(NPROFILE, profile[..], false);
  print("  % bytes, % relays, key at byte %\n", p.written, p.relays, p.special_at);
  show("  first 5 bytes", profile[0..<5]);
  walk(profile[0..<p.written], p);
  e = nip19_encode_checked(N19_PROFILE, cast(string) profile[0..<p.written], text[..], false);
  print("  re-encodes to the same text: %\n", cast(string) text[0..<e.written] == NPROFILE);

  // NIP-21 URIs: "nostr:" and a NIP-19 string.
  try_uri(URI_NPUB);
  try_uri(URI_NEVENT);
  try_uri(URI_NADDR);

  // A URI must not carry nsec. The HRP is checked before any byte is
  // decoded, so the output buffer keeps its old contents.
  for i: 0..<128 { out[i] = 0; }
  uri: [128] u8;
  scheme := "nostr:";
  for i: 0..<6 { uri[i] = scheme[i]; }
  for i: 0..<NSEC.count { uri[6 + i] = NSEC[i]; }
  s := nip21_decode_checked(cast(string) uri[0..<6 + NSEC.count], out[..], false);
  untouched := true;
  for i: 0..<128 { if out[i] != 0 { untouched = false; } }
  print("nostr:nsec1...: %, output untouched: %\n", error_name(s.error), untouched);
  print("bare npub as a URI: %\n", error_name(nip21_decode_checked(NPUB, out[..], false).error));

  // TLV refusals, built from the nprofile's 72 bytes.
  buf: [128] u8;
  for i: 0..<72 { buf[i] = profile[i]; }
  try_payload("nprofile cut to 71 bytes", "nprofile", buf[0..<71]);
  try_payload("nprofile without its key", "nprofile", buf[34..<72]);
  buf[36] = 0x80;
  try_payload("relay byte 0x80", "nprofile", buf[0..<72]);
  buf[36] = profile[36];
  for i: 0..<34 { buf[72 + i] = profile[i]; }
  try_payload("two keys", "nprofile", buf[0..<106]);
  buf[72] = 99; buf[73] = 2; buf[74] = 0xff; buf[75] = 0xff;
  try_payload("unknown type 99, skipped", "nprofile", buf[0..<76]);
  buf[1] = 31;
  try_payload("special of 31 bytes", "nevent", buf[0..<72]);
  try_payload("npub of 33 bytes", "npub", profile[1..<34]);
  try_payload("npub of 32 bytes", "npub", profile[2..<34]);
  try_payload("deprecated nrelay", "nrelay", profile[34..<49]);
  naddr: [128] u8;
  a := nip21_decode_checked(URI_NADDR, naddr[..], false);
  try_payload("naddr without its kind", "naddr", naddr[0..<a.written - 6]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/nip19.coax
```

The keys are the ones NIP-19 states, and the nprofile's words, bytes and
records are the ones worked out by hand:

```text
npub: ok, 32 bytes
  key: 7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e
  re-encodes to the same text: true
nsec: ok, 32 bytes
  key: 67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa
  re-encodes to the same text: true
nprofile: 116 words, first 8: 0 0 16 3 23 28 6 6
  72 bytes, 2 relays, key at byte 2
  first 5 bytes: 00203bf0c6
  at 0: type 0, length 32, hex: 3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d
  at 34: type 1, length 13, text "wss://r.x.com"
  at 49: type 1, length 21, text "wss://djbas.sadkb.com"
  re-encodes to the same text: true
nostr:npub1sn0wd...: npub, 32 bytes, 0 relays
  key: 84dee6e676e5bb67b4ad4e042cf70cbd8681155db535942fcc6a0533858a7240
nostr:nevent1qqs...: nevent, 95 bytes, 2 relays
  at 0: type 0, length 32, hex: b9f5441e45ca39179320e0031cfb18e34078673dcc3d3e3a3b3a981760aa5696
  at 34: type 1, length 28, text "wss://nostr-relay.untethr.me"
  at 64: type 1, length 29, text "wss://nostr-pub.wellorder.net"
nostr:naddr1qqyr...: naddr, 69 bytes, 1 relays
  at 0: type 0, length 8, text "18ff5416"
  at 10: type 1, length 17, text "wss://fiatjaf.com"
  at 29: type 2, length 32, hex: 3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d
  at 63: type 3, length 4, kind 30023
nostr:nsec1...: invalid, output untouched: true
bare npub as a URI: invalid
nprofile cut to 71 bytes: invalid
nprofile without its key: invalid
relay byte 0x80: invalid
two keys: invalid
unknown type 99, skipped: ok, nprofile, 2 relays
special of 31 bytes: invalid
npub of 33 bytes: invalid
npub of 32 bytes: ok, npub, 0 relays
deprecated nrelay: invalid
naddr without its kind: invalid
```

The nsec in the output is NIP-19's published example key, printed in the
specification. Never print a real secret key.

The naddr carries the identifier `18ff5416`, a relay, the author's key and
kind 30023 (`00 00 75 47`), a long-form article. The nevent has no author
or kind records, which NIP-19 allows.

The refusals and the one accepted edit:

- `nostr:` followed by NIP-19's nsec refuses, and the 128-byte output
  buffer is still all zeros.
- The bare npub is a valid NIP-19 string but not a URI.
- Cut to 71 bytes, the nprofile's second relay says 21 bytes but only 20
  remain, so `n19_field` refuses.
- Without its first 34 bytes, the nprofile has relays but no type 0.
- A relay byte of `0x80` is not ASCII.
- A second type 0 record refuses: a profile has one key.
- A record of type 99 with 2 bytes is skipped, and the profile still
  decodes with its 2 relays.
- With byte 1 set to 31, the special record is 31 bytes, which an `nevent`
  refuses.
- 33 bytes under `npub` refuse; the 32 bytes after them decode.
- `nrelay` is deprecated and refuses as an unknown HRP.
- The naddr without its last 6 bytes, the kind record, refuses, because an
  `naddr` needs a kind.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/nip19.out](../fixtures/nip19.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/nip19.coax](../examples/checks/nip19.coax), which
loads the example, so running the example doesn't run them. They compare
the code with the published examples and the hand values:

- NIP-19's npub and nsec decode to the hex it states, and the hex encodes
  back to the same strings;
- NIP-19's nprofile decodes to the stated key and the two relays, at the
  offsets 0, 34 and 49 worked out above, and its first eight words are
  0, 0, 16, 3, 23, 28, 6, 6;
- all five NIP-21 example URIs decode to the right form, size and relay
  count, and each re-encodes to the same text; the nevent, the naddr (kind
  30023, identifier `18ff5416`) and the relay-first nprofile carry their
  fields;
- a `NOSTR:nsec1…` URI refuses and leaves the buffer unchanged, while the
  bare nsec decodes;
- the encoder refuses each broken TLV payload from the example, accepts the
  record of unknown type, and accepts relay byte `0x7f`.

<!-- source: nostr/examples/checks/nip19.coax check -->
```jai
// True when bytes equal the hex text.
is_hex :: (bytes: [] u8, hex: string) -> bool {
  want: [64] u8;
  r := hex_decode_checked(hex, want[..], false);
  if r.error != NC_OK || r.written != bytes.count { return false; }
  for i: 0..<bytes.count { if bytes[i] != want[i] { return false; } }
  return true;
}
#test "is_hex refuses bad hex, a wrong length and a wrong byte" {
  two := [2] u8.[1, 0];
  #assert is_hex(two[..], "0100") && !is_hex(two[0..<0], "zz") && !is_hex(two[0..<1], "0100");
  #assert !is_hex(two[..], "0000") && !is_hex(two[..], "0101");
}
// NIP-19's two bare examples, from the pinned nostr/nips/official/19.md,
// decode to their stated hex "and vice-versa".
BARE :: [4] string.[
    "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg",
    "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e",
    "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5",
    "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa"];
#forall "NIP-19's npub and nsec examples decode to their hex and back" i: 0..<2 {
  out: [32] u8; key: [32] u8; text: [100] u8;
  r := nip19_decode_checked(BARE[2 * i], out[..], false);
  #assert r.error == NC_OK && r.type == cast(u32)(i + 1) && r.written == 32;
  #assert is_hex(out[..], BARE[2 * i + 1]);
  #assert hex_decode_checked(BARE[2 * i + 1], key[..], false).error == NC_OK;
  e := nip19_encode_checked(cast(u32)(i + 1), cast(string) key[..], text[..], false);
  #assert e.error == NC_OK && cast(string) text[0..<e.written] == BARE[2 * i];
}
// NIP-19's nprofile example: the stated pubkey and two relays, at the
// offsets worked out by hand in the chapter.
#test "NIP-19's nprofile example has its stated key and relays" {
  out: [128] u8;
  r := nip19_decode_checked(NPROFILE, out[..], false);
  #assert r.error == NC_OK && r.type == N19_PROFILE && r.written == 72 && r.relays == 2;
  #assert r.special_at == 2 && r.special_length == 32 && !r.has_author && !r.has_kind;
  #assert is_hex(out[0..<5], "00203bf0c6");
  #assert is_hex(out[2..<34], "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d");
  #assert out[34] == 1 && out[35] == 13 && cast(string) out[36..<49] == "wss://r.x.com";
  #assert out[49] == 1 && out[50] == 21 && cast(string) out[51..<72] == "wss://djbas.sadkb.com";
  hrp: [83] u8; words: [200] u8;
  d := bech32_decode_checked(NPROFILE, hrp[..], words[..], true, false);
  want := [8] u8.[0, 0, 16, 3, 23, 28, 6, 6];
  #assert d.error == NC_OK && d.written == 116;
  for k: 0..<8 { #assert words[k] == want[k]; }
}
// NIP-21's five example URIs, from the pinned nostr/nips/official/21.md.
// Each decodes, and its entity re-encodes to the same text.
URIS :: [5] string.[
    "nostr:npub1sn0wdenkukak0d9dfczzeacvhkrgz92ak56egt7vdgzn8pv2wfqqhrjdv9",
    "nostr:nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p",
    "nostr:nevent1qqstna2yrezu5wghjvswqqculvvwxsrcvu7uc0f78gan4xqhvz49d9spr3mhxue69uhkummnw3ez6un9d3shjtn4de6x2argwghx6egpr4mhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet5nxnepm",
    "nostr:naddr1qqyrzwrxvc6ngvfkqyghwumn8ghj7enfv96x5ctx9e3k7mgzyqalp33lewf5vdq847t6te0wvnags0gs0mu72kz8938tn24wlfze6qcyqqq823cph95ag",
    "nostr:nprofile1qyxhwumn8ghj7mn0wvhxcmmvqyd8wumn8ghj7un9d3shjtnhv4ehgetjde38gcewvdhk6qpq80cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwswpnfsn"];
TYPES :: [5] u32.[N19_NPUB, N19_PROFILE, N19_EVENT, N19_ADDR, N19_PROFILE];
SIZES :: [5] int.[32, 72, 95, 69, 77];
RELAYS :: [5] int.[0, 2, 2, 1, 2];
#forall "every NIP-21 example URI decodes and re-encodes" i: 0..<5 {
  out: [128] u8; text: [400] u8;
  r := nip21_decode_checked(URIS[i], out[..], false);
  #assert r.error == NC_OK && r.type == TYPES[i] && r.written == SIZES[i] && r.relays == RELAYS[i];
  e := nip19_encode_checked(r.type, cast(string) out[0..<r.written], text[..], false);
  #assert e.error == NC_OK && cast(string) text[0..<e.written] == URIS[i][6..<URIS[i].count];
}
#test "the NIP-21 nevent, naddr and relay-first nprofile carry their fields" {
  out: [128] u8;
  r := nip21_decode_checked(URIS[2], out[..], false);
  #assert is_hex(out[2..<34], "b9f5441e45ca39179320e0031cfb18e34078673dcc3d3e3a3b3a981760aa5696");
  #assert cast(string) out[36..<64] == "wss://nostr-relay.untethr.me" && !r.has_author && !r.has_kind;
  r = nip21_decode_checked(URIS[3], out[..], false);
  #assert r.has_special && r.special_length == 8 && cast(string) out[2..<10] == "18ff5416";
  #assert r.has_author && r.author_at == 31 && r.has_kind && r.kind == 30023;
  #assert is_hex(out[31..<63], "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d");
  r = nip21_decode_checked(URIS[4], out[..], false);
  #assert r.special_at == 45 && cast(string) out[2..<15] == "wss://nos.lol";
  #assert is_hex(out[45..<77], "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d");
}
// NIP-21: "except nsec". The URI refuses and the caller's buffer keeps its
// contents; the same string without the scheme decodes.
#test "a nostr:nsec URI refuses and writes nothing" {
  out: [32] u8; for k: 0..<32 { out[k] = 77; }
  uri: [100] u8; scheme := "NOSTR:";
  for k: 0..<6 { uri[k] = scheme[k]; }
  for k: 0..<BARE[2].count { uri[6 + k] = BARE[2][k]; }
  #assert nip21_decode_checked(cast(string) uri[0..<6 + BARE[2].count], out[..], false).error == NC_INVALID;
  for k: 0..<32 { #assert out[k] == 77; }
  #assert nip21_decode_checked(BARE[0], out[..], false).error == NC_INVALID;
  #assert nip19_decode_checked(BARE[2], out[..], false).error == NC_OK;
}
// The TLV refusals of the example, with the rule each one breaks.
#test "truncated, keyless, non-ASCII, duplicate and wrong-length TLVs refuse" {
  profile: [128] u8; buf: [128] u8; text: [400] u8;
  p := nip19_decode_checked(NPROFILE, profile[..], false);
  for k: 0..<72 { buf[k] = profile[k]; }
  #assert nip19_encode_checked(N19_PROFILE, cast(string) buf[0..<71], text[..], false).error == NC_INVALID;  // relay past the end
  #assert nip19_encode_checked(N19_PROFILE, cast(string) buf[34..<72], text[..], false).error == NC_INVALID; // no type 0
  #assert nip19_encode_checked(N19_EVENT, cast(string) buf[0..<72], text[..], false).error == NC_OK;
  buf[36] = 0x80;
  #assert nip19_encode_checked(N19_PROFILE, cast(string) buf[0..<72], text[..], false).error == NC_INVALID;  // relay not ASCII
  buf[36] = 0x7f;
  #assert nip19_encode_checked(N19_PROFILE, cast(string) buf[0..<72], text[..], false).error == NC_OK;
  buf[36] = profile[36];
  for k: 0..<34 { buf[72 + k] = profile[k]; }
  #assert nip19_encode_checked(N19_PROFILE, cast(string) buf[0..<106], text[..], false).error == NC_INVALID; // two type 0
  buf[72] = 99; buf[73] = 2; buf[74] = 0xff; buf[75] = 0xff;
  #assert nip19_encode_checked(N19_PROFILE, cast(string) buf[0..<76], text[..], false).error == NC_OK;       // unknown, skipped
  buf[1] = 31;
  #assert nip19_encode_checked(N19_EVENT, cast(string) buf[0..<72], text[..], false).error == NC_INVALID;    // special not 32
  #assert nip19_encode_checked(N19_NPUB, cast(string) profile[1..<34], text[..], false).error == NC_INVALID; // 33 bytes
  #assert nip19_encode_checked(N19_NOTE, cast(string) profile[2..<33], text[..], false).error == NC_INVALID; // 31 bytes
  naddr: [128] u8;
  a := nip21_decode_checked(URIS[3], naddr[..], false);
  #assert nip19_encode_checked(N19_ADDR, cast(string) naddr[0..<a.written - 6], text[..], false).error == NC_INVALID;  // no kind
  #assert nip19_encode_checked(N19_ADDR, cast(string) naddr[0..<29], text[..], false).error == NC_INVALID;  // no author
  #assert p.error == NC_OK;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/nip19.coax
coax/bin/coax mutate nostr/examples/checks/nip19.coax --root-only
```

`coax test` runs 20 checks, including the loaded files' own checks, in
under a second. On 2026-11-01, root-only mutation killed 12 of 12 mutants
in the checks file.

### The library's own checks

`nip19-core.coax` is `#authority` and carries its own checks: every HRP,
count and padding boundary, empty `naddr` identifiers, kind 0, repeated
relays, unknown records, every TLV end condition, and the rule that a
refused call writes nothing. The record
[coax-nostr-bech32-2026-10-28.md](../../docs/verification/coax-nostr-bech32-2026-10-28.md)
gives the results on 2026-10-28:

- `coax mutate` killed 511 of 511 mutants in `nip19-core.coax` with the
  Bech32 core loaded.
- [nostr_bech32.py](../../coax/tests/nostr_bech32.py) ran vector and native
  span records through checked and optimized emitted C under clang
  ASan/UBSan, with no sanitizer errors.
- 10,000 generated NIP-19 entities agreed with `nostr-effect 0.0.13` and
  `@scure/base 1.2.4`.

For this chapter the library gained `nip21_decode_checked`, which NIP-21's
rule needed and which did not exist, and `nip19_decode_checked` became a
call to `n19_decode` with `secret` true. Its new check decodes NIP-21 URIs
in both cases, refuses `nostr:nsec1…` in both cases without writing, and
refuses an empty string, `nostr`, `nostr:`, `xostr:`, `nostr;`, a bare
npub and a doubled scheme. On 2026-11-01, root-only mutation of
`nip19-core.coax` killed 264 of 264 mutants. The full mutation run with
the core, the sanitizer harness and the 10,000-entity comparison were not
repeated.

## Summary

- `npub`, `nsec` and `note` are exactly 32 bytes under a Bech32 HRP, with
  NIP-19's 5,000-character limit.
- `nprofile`, `nevent` and `naddr` carry TLV records: one type byte, one
  length byte, the value. Type 0 is required and single; relays are ASCII;
  `naddr` needs an author and a kind; unknown types are skipped.
- A length past the end, a missing or doubled special, a wrong-length key
  or kind, and a non-ASCII relay refuse before anything is written.
- A `nostr:` URI carries any NIP-19 form except `nsec`, which refuses from
  its HRP alone.
- A decoded entity is a pointer to fetch and verify, not a verified event.

## Dependencies

- [15. Bech32](ch015-bech32.md)

## Sources

- [NIP-19](../nips/official/19.md), also pinned in
  [coax/tests/vectors/upstream/nips/19.md](../../coax/tests/vectors/upstream/nips/19.md).
- [NIP-21](../nips/official/21.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
