# 104. Messages and TLVs

Lightning nodes talk in messages. BOLT 1 defines their shape: a 2-byte
type, then a payload, then an optional extension. It also defines the
type-length-value (TLV) format that lets the protocol add fields without
breaking old nodes, and the first messages two peers exchange.

This chapter builds that base layer: message framing with the "it's OK to
be odd" rule, BigSize integers, TLV streams, truncated integers, and the
`init`, `error`, `warning`, `ping` and `pong` messages with BOLT 9's
feature bits. It is the first chapter of Part XVII, and every later
Lightning chapter reads and writes messages through it.

## What this chapter teaches

- The message format: a big-endian 2-byte type, a payload and an optional
  TLV extension, at most 65,535 bytes in all.
- The "it's OK to be odd" rule: a reader ignores an unknown odd type and
  closes the connection on an unknown even one. The same rule covers TLV
  types and feature bits.
- BigSize, Bitcoin's CompactSize in big-endian, and why a reader refuses
  any encoding but the shortest.
- TLV streams: records in strictly increasing type order, each read or
  skipped by its length.
- Truncated integers (`tu16`, `tu32`, `tu64`), which drop their leading
  zero bytes.
- `init` and its feature bitmaps, `error` and `warning`, and `ping` and
  `pong`.

## Where Lightning uses messages and TLVs

| Where | What it uses | Chapter |
| --- | --- | --- |
| The Noise transport | carries each message, at most 65,535 bytes | 105 |
| Channels | `open_channel`, `update_add_htlc` and the others, with TLV extensions | 109 |
| Onions | the hop payloads are TLV streams | 111 |
| Gossip | announcements with feature bits and TLV extensions | 112 |
| Offers | every BOLT 12 message is a TLV stream | 115 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bolt1.coax](../../coax/lib/lightning/bolt1.coax) | `bigsize_read_vartime`, `bigsize_write_vartime` | BigSize |
| | `tlv_next_vartime(b, at, previous, first)` | one TLV record, in order |
| | `tlv_check_vartime(b, known)` | a whole stream against its known types |
| | `tu_read_vartime`, `tu_write_vartime` | truncated integers |
| | `ln_message_read_vartime`, `ln_dispatch_vartime` | the type, and what to do with it |
| | `ln_feature_set`, `ln_feature_refuse_vartime` | feature bits and the odd rule |
| | `ln_init_read_vartime`, `ln_init_write_vartime` | `init` |
| | `ln_error_read_vartime`, `ln_error_write_vartime` | `error` and `warning` |
| | `ln_ping_read_vartime`, `ln_pong_reply_vartime` | `ping` and `pong` |
| [secp_encode.coax](../../coax/lib/crypto/secp_encode.coax) | `se_decode` | a compressed point, for BOLT 1's test namespace (chapter 17) |

The transport of chapter 105 decrypts messages before this code sees
them, and a message's bytes are then public. Every procedure here runs in
variable time, and its name ends in `_vartime` (chapter 3). Readers copy
nothing: a read message is positions into the caller's bytes, as in
chapters 34 and 65.

The library's codes and sizes:

<!-- source: coax/lib/lightning/bolt1.coax codes -->
```jai
LN_OK :: 0;
LN_TRUNCATED :: 1;     // the bytes end inside a field: "unexpected EOF"
LN_NOT_MINIMAL :: 2;   // a BigSize or truncated integer longer than it needs
LN_ORDER :: 3;         // TLV types not strictly increasing, a repeat included
LN_UNKNOWN_EVEN :: 4;  // an even type or feature bit the reader does not know
LN_BAD_LENGTH :: 5;    // a known TLV record whose length its type does not allow
LN_BAD_VALUE :: 6;     // a value its type refuses: a point off the curve, say
LN_CAPACITY :: 7;      // the caller's buffer is too small
LN_TOO_LARGE :: 8;     // a message above 65,535 bytes

LN_MAX_MESSAGE :: 65535;  // BOLT 8's 2-byte length
LN_TYPE :: 2;             // the big-endian message type

LN_WARNING :: 1;
LN_INIT :: 16;
LN_ERROR :: 17;
LN_PING :: 18;
LN_PONG :: 19;
LN_PONG_LIMIT :: 65532;   // a ping asking for this many pong bytes or more gets no reply
```

## BigSize

TLV types and lengths are BigSize integers. BigSize is Bitcoin's
CompactSize (chapter 34) with the multi-byte values in big-endian: one
byte for a value below `0xfd`, else a marker byte `0xfd`, `0xfe` or `0xff`
followed by 2, 4 or 8 bytes. A value has exactly one valid encoding, the
shortest. A reader that accepted `fd00fc` as 252 would let two different
byte strings carry the same stream, and a signature over one would not
cover the other.

Unlike CompactSize in a transaction, BigSize has no upper limit below
2^64: a TLV type can use all 64 bits.

<!-- source: coax/lib/lightning/bolt1.coax bigsize -->
```jai
// BigSize is Bitcoin's CompactSize (chapter 34) in big-endian: one byte
// below 0xfd, else 0xfd, 0xfe or 0xff and then 2, 4 or 8 bytes. Every value
// has one encoding, the shortest, and a reader refuses any other.
LnBigSize :: struct { code: int; value: u64; next: int; }
bigsize_read_vartime :: (b: [] u8, at: int) -> LnBigSize {
  if at >= b.count { return LnBigSize.{code = LN_TRUNCATED, next = at}; }
  first := b[at];
  if first < 0xfd { return LnBigSize.{value = cast(u64) first, next = at + 1}; }
  width := ifx first == 0xfd then 2 else ifx first == 0xfe then 4 else 8;
  if width > b.count - at - 1 { return LnBigSize.{code = LN_TRUNCATED, next = at}; }
  value: u64 = 0;
  for i: 0..<width { value = (value << 8) | cast(u64) b[at + 1 + i]; }
  least: u64 = ifx width == 2 then 0xfd else ifx width == 4 then 0x10000 else 0x100000000;
  if value < least { return LnBigSize.{code = LN_NOT_MINIMAL, next = at}; }
  return LnBigSize.{value = value, next = at + 1 + width};
}
bigsize_size :: (value: u64) -> int {
  if value < 0xfd { return 1; }
  if value < 0x10000 { return 3; }
  if value < 0x100000000 { return 5; }
  return 9;
}
// Writes value at out[at] and returns where writing goes on, or -1.
bigsize_write_vartime :: (out: [] u8, at: int, value: u64) -> int {
  size := bigsize_size(value);
  if at < 0 || size > out.count - at { return -1; }
  if size == 1 { out[at] = cast(u8) value; return at + 1; }
  out[at] = cast(u8) (ifx size == 3 then 0xfd else ifx size == 5 then 0xfe else 0xff);
  for i: 0..<size - 1 { out[at + size - 1 - i] = cast(u8) ((value >> cast(u64) (8 * i)) & 255); }
  return at + size;
}
```

## TLV streams

A TLV record is a BigSize type, a BigSize length, and that many bytes of
value. A TLV stream is zero or more records, one after another, until the
bytes end. BOLT 1's rules for a reader:

- Types must be strictly increasing. A repeated type is an error, so each
  type appears at most once, and a stream has one canonical order.
- A length that runs past the end of the stream is an error.
- A known type is decoded by its own rules, and a value of the wrong
  length is an error.
- An unknown even type is an error. An unknown odd type is skipped.

`tlv_next_vartime` reads one record and checks the order against the
previous type. The caller then decides what each type means.
`tlv_check_vartime` checks a whole stream's framing against a list of
known types.

<!-- source: coax/lib/lightning/bolt1.coax tlv -->
```jai
// One record of a TLV stream: type, length, then the value at
// b[start..<start+length]. A stream is records in strictly increasing type
// order until the bytes end. Pass the previous record's type; first is true
// for the first record.
LnRecord :: struct { code: int; type: u64; start: int; length: int; next: int; }
tlv_next_vartime :: (b: [] u8, at: int, previous: u64, first: bool) -> LnRecord {
  t := bigsize_read_vartime(b, at);
  if t.code != LN_OK { return LnRecord.{code = t.code, next = at}; }
  if !first && t.value <= previous { return LnRecord.{code = LN_ORDER, type = t.value, next = at}; }
  l := bigsize_read_vartime(b, t.next);
  if l.code != LN_OK { return LnRecord.{code = l.code, type = t.value, next = at}; }
  if l.value > cast(u64) (b.count - l.next) { return LnRecord.{code = LN_TRUNCATED, type = t.value, next = at}; }
  n := cast(int) l.value;
  return LnRecord.{type = t.value, start = l.next, length = n, next = l.next + n};
}
// "It's OK to be odd": a reader that does not know a type skips an odd
// one and refuses an even one. The same rule covers message types and
// feature bits.
ln_unknown_vartime :: (type: u64) -> int { return ifx type % 2 == 0 then LN_UNKNOWN_EVEN else LN_OK; }
// A whole stream whose known types are listed, in any order. It checks the
// framing and the odd rule only; the caller reads each known value.
tlv_check_vartime :: (b: [] u8, known: [] u64) -> int {
  at := 0; previous: u64 = 0; first := true;
  while at < b.count {
    r := tlv_next_vartime(b, at, previous, first);
    if r.code != LN_OK { return r.code; }
    found := false; for k: known { if k == r.type { found = true; } }
    if !found && ln_unknown_vartime(r.type) != LN_OK { return LN_UNKNOWN_EVEN; }
    previous = r.type; first = false; at = r.next;
  }
  return LN_OK;
}
// Writes one record and returns where writing goes on, or -1.
tlv_write_vartime :: (out: [] u8, at: int, type: u64, value: [] u8) -> int {
  w := bigsize_write_vartime(out, at, type);
  if w < 0 { return -1; }
  w = bigsize_write_vartime(out, w, cast(u64) value.count);
  if w < 0 || value.count > out.count - w { return -1; }
  for i: 0..<value.count { out[w + i] = value[i]; }
  return w + value.count;
}
```

The odd rule is how Lightning grows. A node that adds an optional field
gives it an odd type, and old nodes skip it. A field that every reader
must understand gets an even type, and an old node refuses the stream
instead of misreading it.

## Truncated integers

The last field of a record can be a truncated integer: `tu16`, `tu32` or
`tu64`. It drops its leading zero bytes, so 1 is one byte, 256 is two, and
0 is no bytes at all. A leading zero byte is not minimal and is refused,
and more bytes than the type's width is a length error.

<!-- source: coax/lib/lightning/bolt1.coax truncated -->
```jai
// tu16, tu32 and tu64: the last value of a record may drop its leading
// zero bytes, and must. Zero is no bytes at all.
LnInt :: struct { code: int; value: u64; }
tu_read_vartime :: (value: [] u8, width: int) -> LnInt {
  if value.count > width { return LnInt.{code = LN_BAD_LENGTH}; }
  if value.count > 0 && value[0] == 0 { return LnInt.{code = LN_NOT_MINIMAL}; }
  v: u64 = 0;
  for i: 0..<value.count { v = (v << 8) | cast(u64) value[i]; }
  return LnInt.{value = v};
}
tu_size :: (v: u64) -> int { n := 0; while n < 8 && (v >> cast(u64) (8 * n)) != 0 { n += 1; } return n; }
tu_write_vartime :: (out: [] u8, at: int, v: u64) -> int {
  n := tu_size(v);
  if at < 0 || n > out.count - at { return -1; }
  for i: 0..<n { out[at + n - 1 - i] = cast(u8) ((v >> cast(u64) (8 * i)) & 255); }
  return at + n;
}
```

## Messages

After decryption, a message is a big-endian 2-byte type and a payload.
The payload's format depends on the type, and an optional TLV extension
can follow it. The transport limits a message to 65,535 bytes.

BOLT 1 groups the types by size: 0 to 31 are setup and control, 32 to 127
channel messages, 128 to 255 commitment messages, 256 to 511 routing, and
32,768 to 65,535 custom. The odd rule applies to the type as well: a
receiver ignores an unknown odd type and closes the connection on an
unknown even one.

<!-- source: coax/lib/lightning/bolt1.coax message -->
```jai
// A message is a 2-byte big-endian type, then its payload. The receiver
// ignores an unknown odd type and closes the connection on an unknown
// even one.
LnMessage :: struct { code: int; type: int; payload: int; }
ln_message_read_vartime :: (m: [] u8) -> LnMessage {
  if m.count > LN_MAX_MESSAGE { return LnMessage.{code = LN_TOO_LARGE}; }
  if m.count < LN_TYPE { return LnMessage.{code = LN_TRUNCATED}; }
  return LnMessage.{type = ln_u16(m, 0), payload = LN_TYPE};
}
ln_known_vartime :: (type: int) -> bool {
  return type == LN_WARNING || type == LN_INIT || type == LN_ERROR || type == LN_PING || type == LN_PONG;
}
// What a receiver does with a message's type: LN_OK to read it, LN_OK with
// ignore true to drop it, or LN_UNKNOWN_EVEN to close the connection.
LnDispatch :: struct { code: int; ignore: bool; }
ln_dispatch_vartime :: (type: int) -> LnDispatch {
  if ln_known_vartime(type) { return LnDispatch.{}; }
  if type % 2 == 1 { return LnDispatch.{ignore = true}; }
  return LnDispatch.{code = LN_UNKNOWN_EVEN};
}
```

This chapter knows five types. Chapter 109 adds the channel messages, and
chapter 112 the gossip messages.

## Feature bits

A node announces what it supports as a bitmap of features. BOLT 9 assigns
the bits in pairs: the even bit of a pair says the node requires the
feature, the odd bit says it supports it. Bits are numbered from the least
significant bit of the last byte, so bit 0 is the low bit of the final
byte.

The odd rule decides compatibility. A peer that sets an even bit the
reader does not know requires something the reader cannot do, so the
reader closes the connection. An unknown odd bit is ignored.

<!-- source: coax/lib/lightning/bolt1.coax features -->
```jai
// BOLT 9's feature bits, numbered from the least significant bit of the
// last byte. Each feature is a pair: the even bit says "required", the
// odd bit "supported".
LN_FEATURES :: [26] string.["option_data_loss_protect", "", "option_upfront_shutdown_script",
  "gossip_queries", "var_onion_optin", "gossip_queries_ex", "option_static_remotekey",
  "payment_secret", "basic_mpp", "option_support_large_channel", "", "option_anchors",
  "option_route_blinding", "option_shutdown_anysegwit", "option_dual_fund", "", "",
  "option_quiesce", "option_attribution_data", "option_onion_messages", "zero_fee_commitments",
  "option_provide_storage", "option_channel_type", "option_scid_alias",
  "option_payment_metadata", "option_zeroconf"];
LN_FEATURES_MORE :: [4] string.["option_simple_close", "option_splice", "", "option_onion_messages_only_channels"];
ln_feature_name :: (bit: int) -> string {
  pair := bit / 2;
  if pair < 26 { return LN_FEATURES[pair]; }
  if pair >= 30 && pair < 34 { return LN_FEATURES_MORE[pair - 30]; }
  return "";
}
ln_feature_set :: (f: [] u8, bit: int) -> bool {
  if bit < 0 || bit / 8 >= f.count { return false; }
  return (f[f.count - 1 - bit / 8] >> cast(u8) (bit % 8)) & 1 == 1;
}
// Sets bit in f, a big-endian bitmap of f.count bytes; false if it does
// not fit.
ln_feature_put :: (f: [] u8, bit: int) -> bool {
  if bit < 0 || bit / 8 >= f.count { return false; }
  f[f.count - 1 - bit / 8] |= cast(u8) (1 << (bit % 8));
  return true;
}
// The bytes a bitmap needs: up to its highest set bit, no leading zeros.
ln_feature_bytes :: (f: [] u8) -> int {
  for i: 0..<f.count { if f[i] != 0 { return f.count - i; } }
  return 0;
}
// The first set bit, from bit 0, that the reader cannot accept: an even
// bit of a feature it does not know. -1 when every bit is acceptable.
ln_feature_refuse_vartime :: (f: [] u8, known: [] int) -> int {
  for bit: 0..<f.count * 8 {
    if ln_feature_set(f, bit) && bit % 2 == 0 {
      found := false; for k: known { if k / 2 == bit / 2 { found = true; } }
      if !found { return bit; }
    }
  }
  return -1;
}
```

The table is BOLT 9's at the pinned revision. The cross-check reads the
names from the pinned `09-features.md`.

## init

`init` (type 16) is the first message on every connection, in both
directions. It carries two bitmaps, `globalfeatures` and `features`, each
a `u16` length and bytes. For historical reasons features were split
across the two; a reader ORs them into one map, aligned at their last
bytes. Then comes `init_tlvs`, a TLV stream with `networks` (type 1, the
chain hashes the node cares about, 32 bytes each) and `remote_addr`
(type 3).

<!-- source: coax/lib/lightning/bolt1.coax init -->
```jai
// init: globalfeatures and features, each a u16 length and bytes, then
// init_tlvs: networks (type 1, chain hashes) and remote_addr (type 3).
// The two bitmaps are ORed into one, aligned at their last bytes.
LN_INIT_NETWORKS :: 1;
LN_INIT_REMOTE_ADDR :: 3;
LnInit :: struct {
  code: int;
  global: int; global_n: int; features: int; features_n: int;
  networks: int; networks_n: int;       // networks_n chain hashes of 32 bytes
  remote_addr: int; remote_addr_n: int; // -1 when absent
  ignored: int;                         // unknown odd records skipped
}
ln_init_read_vartime :: (m: [] u8) -> LnInit {
  r := LnInit.{networks = -1, remote_addr = -1};
  h := ln_message_read_vartime(m);
  if h.code != LN_OK { r.code = h.code; return r; }
  if h.type != LN_INIT { r.code = LN_BAD_VALUE; return r; }
  at := LN_TYPE;
  if m.count - at < 2 { r.code = LN_TRUNCATED; return r; }
  r.global_n = ln_u16(m, at); r.global = at + 2; at = r.global + r.global_n;
  if at + 2 > m.count { r.code = LN_TRUNCATED; return r; }
  r.features_n = ln_u16(m, at); r.features = at + 2; at = r.features + r.features_n;
  if at > m.count { r.code = LN_TRUNCATED; return r; }
  tlvs := m[at..<m.count]; p := 0; previous: u64 = 0; first := true;
  while p < tlvs.count {
    rec := tlv_next_vartime(tlvs, p, previous, first);
    if rec.code != LN_OK { r.code = rec.code; return r; }
    if rec.type == LN_INIT_NETWORKS {
      if rec.length % 32 != 0 { r.code = LN_BAD_LENGTH; return r; }
      r.networks = at + rec.start; r.networks_n = rec.length / 32;
    } else if rec.type == LN_INIT_REMOTE_ADDR {
      r.remote_addr = at + rec.start; r.remote_addr_n = rec.length;
    } else {
      if ln_unknown_vartime(rec.type) != LN_OK { r.code = LN_UNKNOWN_EVEN; return r; }
      r.ignored += 1;
    }
    previous = rec.type; first = false; p = rec.next;
  }
  return r;
}
// The combined bitmap: globalfeatures ORed into features, right-aligned,
// written to out[0..<n]; returns n, or -1 when out is too small.
ln_init_features_vartime :: (m: [] u8, r: LnInit, out: [] u8) -> int {
  n := ifx r.global_n > r.features_n then r.global_n else r.features_n;
  if n > out.count { return -1; }
  for i: 0..<n { out[i] = 0; }
  for i: 0..<r.global_n { out[n - r.global_n + i] |= m[r.global + i]; }
  for i: 0..<r.features_n { out[n - r.features_n + i] |= m[r.features + i]; }
  return n;
}
// An init with empty globalfeatures, the given features, and networks
// when chains is not empty. Returns the length, or -1.
ln_init_write_vartime :: (features: [] u8, chains: [] u8, out: [] u8) -> int {
  if chains.count % 32 != 0 || out.count < 6 + features.count { return -1; }
  ln_put16(out, 0, LN_INIT); ln_put16(out, 2, 0); ln_put16(out, 4, features.count);
  for i: 0..<features.count { out[6 + i] = features[i]; }
  w := 6 + features.count;
  if chains.count > 0 { w = tlv_write_vartime(out, w, LN_INIT_NETWORKS, chains); }
  if w > LN_MAX_MESSAGE { return -1; }
  return w;
}
```

A node waits for its peer's `init` before it sends anything else, so an
incompatible peer is found before any channel work starts.

## error and warning

`error` (type 17) and `warning` (type 1) name a channel by its 32-byte
`channel_id`, or every channel with all zeros, and carry a text. An
`error` fails the channel; a `warning` does not. The data may come from a
hostile peer, so a reader prints it only when it is printable ASCII.

<!-- source: coax/lib/lightning/bolt1.coax error -->
```jai
// error (17) and warning (1): a channel_id, all zeros for every channel,
// then a u16 length and that much data, which should be printable ASCII.
LnError :: struct { code: int; type: int; channel: int; all: bool; data: int; data_n: int; printable: bool; }
ln_error_read_vartime :: (m: [] u8) -> LnError {
  h := ln_message_read_vartime(m);
  if h.code != LN_OK { return LnError.{code = h.code}; }
  if h.type != LN_ERROR && h.type != LN_WARNING { return LnError.{code = LN_BAD_VALUE, type = h.type}; }
  if m.count < LN_TYPE + 34 { return LnError.{code = LN_TRUNCATED, type = h.type}; }
  r := LnError.{type = h.type, channel = LN_TYPE, all = true, data = LN_TYPE + 34, printable = true};
  for i: 0..<32 { if m[LN_TYPE + i] != 0 { r.all = false; } }
  r.data_n = ln_u16(m, LN_TYPE + 32);
  if r.data_n > m.count - r.data { return LnError.{code = LN_TRUNCATED, type = h.type}; }
  for i: 0..<r.data_n { c := m[r.data + i]; if c < 32 || c > 126 { r.printable = false; } }
  return r;
}
ln_error_write_vartime :: (type: int, channel: [32] u8, text: string, out: [] u8) -> int {
  n := LN_TYPE + 34 + text.count;
  if (type != LN_ERROR && type != LN_WARNING) || n > out.count || n > LN_MAX_MESSAGE { return -1; }
  ln_put16(out, 0, type);
  for i: 0..<32 { out[2 + i] = channel[i]; }
  ln_put16(out, 34, text.count);
  for i: 0..<text.count { out[36 + i] = text[i]; }
  return n;
}
```

## ping and pong

`ping` (type 18) keeps a connection alive and asks for a `pong` (type 19)
of `num_pong_bytes` zeros. Both carry padding that should be zeros, never
memory that might hold a secret. A `num_pong_bytes` of 65,532 or more asks
for no reply: 65,531 is the largest padding that fits in a 65,535-byte
`pong` with its type and length.

<!-- source: coax/lib/lightning/bolt1.coax ping -->
```jai
// ping (18): num_pong_bytes, then byteslen bytes to ignore. pong (19):
// byteslen bytes to ignore. Both pads should be zeros, never secrets.
LnPing :: struct { code: int; type: int; pong_bytes: int; ignored_n: int; }
ln_ping_read_vartime :: (m: [] u8) -> LnPing {
  h := ln_message_read_vartime(m);
  if h.code != LN_OK { return LnPing.{code = h.code}; }
  if h.type != LN_PING && h.type != LN_PONG { return LnPing.{code = LN_BAD_VALUE, type = h.type}; }
  at := LN_TYPE; r := LnPing.{type = h.type, pong_bytes = -1};
  if h.type == LN_PING {
    if m.count < at + 2 { return LnPing.{code = LN_TRUNCATED, type = h.type}; }
    r.pong_bytes = ln_u16(m, at); at += 2;
  }
  if m.count < at + 2 { return LnPing.{code = LN_TRUNCATED, type = h.type}; }
  r.ignored_n = ln_u16(m, at);
  if r.ignored_n > m.count - at - 2 { return LnPing.{code = LN_TRUNCATED, type = h.type}; }
  return r;
}
ln_ping_write_vartime :: (pong_bytes: int, ignored: int, out: [] u8) -> int {
  n := LN_TYPE + 4 + ignored;
  if pong_bytes < 0 || pong_bytes > 65535 || ignored < 0 || n > out.count || n > LN_MAX_MESSAGE { return -1; }
  ln_put16(out, 0, LN_PING); ln_put16(out, 2, pong_bytes); ln_put16(out, 4, ignored);
  for i: 0..<ignored { out[6 + i] = 0; }
  return n;
}
// The reply to a ping: a pong of num_pong_bytes zeros, or nothing (0)
// when the ping asked for 65,532 bytes or more. Returns the length, or -1.
ln_pong_reply_vartime :: (ping: LnPing, out: [] u8) -> int {
  if ping.code != LN_OK || ping.type != LN_PING { return -1; }
  if ping.pong_bytes >= LN_PONG_LIMIT { return 0; }
  n := LN_TYPE + 2 + ping.pong_bytes;
  if n > out.count { return -1; }
  ln_put16(out, 0, LN_PONG); ln_put16(out, 2, ping.pong_bytes);
  for i: 0..<ping.pong_bytes { out[4 + i] = 0; }
  return n;
}
```

Peers can use `ping` and `pong` to send traffic of chosen sizes, which
hides the sizes of real messages from an observer.

## The example

The example,
[bitcoin/examples/lightning-messages.coax](../examples/lightning-messages.coax),
runs every BigSize, TLV and `init` vector in BOLT 1, then
builds an `init`, checks feature bits, dispatches message types, and
answers a `ping`.

<!-- source: bitcoin/examples/lightning-messages.coax example -->
```jai
#load "../../coax/lib/lightning/bolt1.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex, at most 256 of them.
show :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A stream's hex, shortened past 24 characters.
short :: (s: string) -> string { return ifx s.count > 24 then s[0..<24] else s; }
CODES :: [9] string.["ok", "truncated", "not minimal", "order", "unknown even", "length", "value", "capacity", "too large"];
SPACES :: [3] string.["n1/n2", "n1", "n2"];
// The mainnet chain_hash: the genesis block's hash in its internal byte order.
MAINNET :: "6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000";

main :: () {
  b: [400] u8; out: [400] u8;

  // ---- BigSize: BOLT 1's Appendix A
  print("bigsize\n");
  written := 0; encodings := 0;
  for row: LN_BIGSIZE {
    got := bigsize_read_vartime(ln_unhex(row.bytes, b[..]), 0);
    if got.code == LN_OK { print("  %: % -> %\n", row.name, row.bytes, got.value); }
    else { print("  %: % -> %\n", row.name, row.bytes, CODES[got.code]); }
    if row.encode {
      encodings += 1;
      n := bigsize_write_vartime(out[..], 0, row.value);
      if ln_hex_is(out[0..<n], row.bytes) { written += 1; }
    }
  }
  print("  encodings written byte for byte: % of %\n", written, encodings);

  // ---- TLV streams: BOLT 1's Appendix B
  print("tlv streams\n");
  agree := 0;
  for row: LN_TLV {
    stream := ln_unhex(row.stream, b[..]);
    first := ifx row.space == 2 then 2 else 1;
    last := ifx row.space == 1 then 1 else 2;
    print("  % %", SPACES[row.space], ifx row.stream.count == 0 then "(empty)" else short(row.stream));
    if row.stream.count > 24 { print("..."); }
    print(":");
    ok := true;
    for ns: first..<last + 1 {
      got := ln_test_decode_vartime(stream, ns);
      print(" %", CODES[got.code]);
      if (got.code == LN_OK) != row.ok { ok = false; }
      if row.ok && row.record > 0 {
        if row.record == 1 { print(" amount_msat=%", got.amount); }
        if row.record == 2 { print(" scid=%x%x%", got.scid >> 40, (got.scid >> 16) & 0xffffff, got.scid & 0xffff); }
        if row.record == 3 { print(" amount_msat_1=% amount_msat_2=%", got.amount_1, got.amount_2); }
        if row.record == 4 { print(" cltv_delta=%", got.cltv); }
      }
    }
    if ok { agree += 1; }
    print("\n");
  }
  print("  agree with BOLT 1: % of %\n", agree, LN_TLV.count);
  // Appending: a valid stream then a refused one is refused.
  refused := 0; pairs := 0;
  for good: LN_TLV {
    if good.ok && good.space == 1 {
      for bad: LN_TLV {
        if !bad.ok && bad.space != 2 {
          g := ln_unhex(good.stream, b[..]); c := ln_unhex(bad.stream, b[g.count..<400]);
          pairs += 1;
          if ln_test_decode_vartime(b[0..<g.count + c.count], 1).code != LN_OK { refused += 1; }
        }
      }
    }
  }
  print("  a valid n1 stream followed by a refused one: % of % refused\n", refused, pairs);

  // ---- messages: BOLT 1's Appendix C, then our own
  print("messages\n");
  for row: LN_INIT_ROWS {
    got := ln_init_read_vartime(ln_unhex(row.message, b[..]));
    print("  init %: %", row.message, CODES[got.code]);
    if got.code == LN_OK { print(", % unknown odd records skipped", got.ignored); }
    print("\n");
  }
  f: [3] u8; chains: [32] u8;
  for bit: [3] int.[8, 14, 17] { put := ln_feature_put(f[..], bit); }
  hash := ln_unhex(MAINNET, chains[..]);
  n := ln_init_write_vartime(f[..], hash, out[..]);
  print("  our init, % bytes: ", n); show(out[0..<n]); print("\n");
  r := ln_init_read_vartime(out[0..<n]);
  all: [8] u8; c := ln_init_features_vartime(out[0..<n], r, all[..]);
  print("  read back: %, % network, features", CODES[r.code], r.networks_n);
  for bit: 0..<c * 8 {
    if ln_feature_set(all[0..<c], bit) { print(" % (%, %)", bit, ln_feature_name(bit), ifx bit % 2 == 0 then "required" else "supported"); }
  }
  print("\n");
  known := [3] int.[8, 14, 17];
  for bit: [2] int.[101, 100] {
    p: [13] u8; for i: 0..<3 { p[10 + i] = f[i]; } put := ln_feature_put(p[..], bit);
    refuse := ln_feature_refuse_vartime(p[..], known[..]);
    print("  a peer that sets bit %: %\n", bit, ifx refuse < 0 then "accept" else "close");
  }
  for t: [4] int.[18, 32768, 32769, 1] {
    d := ln_dispatch_vartime(t);
    print("  message type %: %\n", t, ifx d.code != LN_OK then "close" else ifx d.ignore then "ignore" else "read");
  }
  n = ln_ping_write_vartime(4, 2, out[..]);
  print("  ping: "); show(out[0..<n]);
  m := ln_pong_reply_vartime(ln_ping_read_vartime(out[0..<n]), b[..]);
  print(", pong: "); show(b[0..<m]); print("\n");
  n = ln_ping_write_vartime(65532, 0, out[..]);
  print("  a ping asking for 65532 bytes: % reply bytes\n", ln_pong_reply_vartime(ln_ping_read_vartime(out[0..<n]), b[..]));
  zero: [32] u8;
  n = ln_error_write_vartime(LN_ERROR, zero, "internal error", out[..]);
  e := ln_error_read_vartime(out[0..<n]);
  print("  error: "); show(out[0..<n]); print("\n");
  print("  read: %, every channel %, % bytes of data, printable %\n", CODES[e.code], e.all, e.data_n, e.printable);
}
```

From the repository root:

```shell
coax/bin/coax run bitcoin/examples/lightning-messages.coax
```

The output:

```text
bigsize
  zero: 00 -> 0
  one byte high: fc -> 252
  two byte low: fd00fd -> 253
  two byte high: fdffff -> 65535
  four byte low: fe00010000 -> 65536
  four byte high: feffffffff -> 4294967295
  eight byte low: ff0000000100000000 -> 4294967296
  eight byte high: ffffffffffffffffff -> 18446744073709551615
  two byte not canonical: fd00fc -> not minimal
  four byte not canonical: fe0000ffff -> not minimal
  eight byte not canonical: ff00000000ffffffff -> not minimal
  two byte short read: fd00 -> truncated
  four byte short read: feffff -> truncated
  eight byte short read: ffffffffff -> truncated
  one byte no read:  -> truncated
  two byte no read: fd -> truncated
  four byte no read: fe -> truncated
  eight byte no read: ff -> truncated
  encodings written byte for byte: 8 of 8
tlv streams
  n1/n2 fd: truncated truncated
  n1/n2 fd01: truncated truncated
  n1/n2 fd000100: not minimal not minimal
  n1/n2 fd0101: truncated truncated
  n1/n2 0ffd: truncated truncated
  n1/n2 0ffd26: truncated truncated
  n1/n2 0ffd2602: truncated truncated
  n1/n2 0ffd000100: not minimal not minimal
  n1/n2 0ffd02010000000000000000...: truncated truncated
  n1/n2 1200: unknown even unknown even
  n1/n2 fd010200: unknown even unknown even
  n1/n2 fe0100000200: unknown even unknown even
  n1/n2 ff010000000000000200: unknown even unknown even
  n1 0109ffffffffffffffffff: length
  n1 010100: not minimal
  n1 01020001: not minimal
  n1 0103000100: not minimal
  n1 010400010000: not minimal
  n1 01050001000000: not minimal
  n1 0106000100000000: not minimal
  n1 010700010000000000: not minimal
  n1 01080001000000000000: not minimal
  n1 020701010101010101: length
  n1 0209010101010101010101: length
  n1 0321023da092f6980e58d2c0...: length
  n1 0329023da092f6980e58d2c0...: length
  n1 0330023da092f6980e58d2c0...: length
  n1 0331043da092f6980e58d2c0...: value
  n1 0332023da092f6980e58d2c0...: length
  n1 fd00fe00: length
  n1 fd00fe0101: length
  n1 fd00fe03010101: length
  n1 0000: unknown even
  n1/n2 (empty): ok ok
  n1/n2 2100: ok ok
  n1/n2 fd020100: ok ok
  n1/n2 fd00fd00: ok ok
  n1/n2 fd00ff00: ok ok
  n1/n2 fe0200000100: ok ok
  n1/n2 ff020000000000000100: ok ok
  n1 0100: ok amount_msat=0
  n1 010101: ok amount_msat=1
  n1 01020100: ok amount_msat=256
  n1 0103010000: ok amount_msat=65536
  n1 010401000000: ok amount_msat=16777216
  n1 01050100000000: ok amount_msat=4294967296
  n1 0106010000000000: ok amount_msat=1099511627776
  n1 010701000000000000: ok amount_msat=281474976710656
  n1 01080100000000000000: ok amount_msat=72057594037927936
  n1 02080000000000000226: ok scid=0x0x550
  n1 0331023da092f6980e58d2c0...: ok amount_msat_1=1 amount_msat_2=2
  n1 fd00fe020226: ok cltv_delta=550
  n1 020800000000000002260101...: order
  n1 020800000000000002310208...: order
  n1 1f000f012a: order
  n1 1f001f012a: order
  n2 ffffffffffffffffff000000: order
  agree with BOLT 1: 57 of 57
  a valid n1 stream followed by a refused one: 444 of 444 refused
messages
  init 001000000000: ok, 0 unknown odd records skipped
  init 001000000000c9012acb0104: ok, 2 unknown odd records skipped
  init 00100000000001: truncated
  init 001000000000ca012a: unknown even
  init 001000000000c90101c90102: order
  our init, 43 bytes: 00100000000302410001206fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
  read back: ok, 1 network, features 8 (var_onion_optin, required) 14 (payment_secret, required) 17 (basic_mpp, supported)
  a peer that sets bit 101: accept
  a peer that sets bit 100: close
  message type 18: read
  message type 32768: close
  message type 32769: ignore
  message type 1: read
  ping: 0012000400020000, pong: 0013000400000000
  a ping asking for 65532 bytes: 0 reply bytes
  error: 00110000000000000000000000000000000000000000000000000000000000000000000e696e7465726e616c206572726f72
  read: ok, every channel true, 14 bytes of data, printable true
```

What the output shows:

- All 18 BigSize decodings give BOLT 1's value or refuse: the three
  non-minimal encodings as `not minimal`, the seven short reads as
  `truncated`. All 8 encodings match byte for byte.
- All 57 TLV streams give BOLT 1's verdict, in every namespace the vector
  names, and the 12 valid `n1` streams give BOLT 1's values. A tu64 with
  a leading zero, a point with prefix `04`, and records out of order or
  repeated are each refused.
- A valid `n1` stream followed by a refused one is refused, all 444 pairs.
- Of BOLT 1's five `init` messages, the two valid ones read, one with two
  unknown odd records skipped. The three invalid ones are refused for a
  truncated record, an unknown even type and a repeated type.
- Our `init` sets bits 8 and 14 (required) and 17 (supported) in 3 bytes,
  with the mainnet chain hash under `networks`: 43 bytes.
- A peer that sets the unknown odd bit 101 is accepted; one that sets the
  unknown even bit 100 is refused.
- Type 32,768 closes the connection and 32,769 is ignored.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-messages.out](../fixtures/lightning-messages.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-messages.coax](../examples/checks/lightning-messages.coax),
which loads the example:

- both sides of every BigSize boundary, 2^k - 1 and 2^k for k from 0 to
  63, write in the fewest bytes and read back
- every prefix that cuts a valid 51-byte record short is refused
- valid `n1` streams concatenate in type order, and refuse in the other
  order
- for each of 128 unknown feature bits alone, an even bit closes and an
  odd bit is ignored
- for message types 0 to 1,023, an unknown even type closes and an unknown
  odd type is ignored

<!-- source: bitcoin/examples/checks/lightning-messages.coax check -->
```jai
// Each side of every BigSize boundary, 2^k - 1 and 2^k, writes in the
// fewest bytes and reads back.
#forall "BigSize round-trips at every power of two" k: 0..<64 {
  out: [9] u8; v := cast(u64) 1 << cast(u64) k;
  for x: [2] u64.[v - 1, v] {
    n := bigsize_write_vartime(out[..], 0, x);
    got := bigsize_read_vartime(out[0..<n], 0);
    #assert n == bigsize_size(x) && got.code == LN_OK && got.value == x && got.next == n;
  }
}
// Any prefix of a valid record that cuts it short is refused.
#forall "a cut record refuses" cut: 1..<51 #steps 200000000 {
  b: [64] u8; s := ln_unhex(LN_TLV[50].stream, b[..]);
  #assert s.count == 51 && ln_test_decode_vartime(s, 1).code == LN_OK;
  #assert ln_test_decode_vartime(s[0..<cut], 1).code != LN_OK;
}
// BOLT 1: a valid stream followed by a valid stream of higher types is
// still valid.
#test "valid n1 streams in type order concatenate" {
  b: [128] u8; good := [4] int.[41, 49, 50, 51];   // tlv1, tlv2, tlv3 and tlv4
  for i: 0..<4 {
    for j: i + 1..<4 {
      a := ln_unhex(LN_TLV[good[i]].stream, b[..]); c := ln_unhex(LN_TLV[good[j]].stream, b[a.count..<128]);
      #assert ln_test_decode_vartime(b[0..<a.count + c.count], 1).code == LN_OK;
      // The other order repeats nothing but goes backwards.
      c = ln_unhex(LN_TLV[good[j]].stream, b[..]); a = ln_unhex(LN_TLV[good[i]].stream, b[c.count..<128]);
      #assert ln_test_decode_vartime(b[0..<a.count + c.count], 1).code == LN_ORDER;
    }
  }
}
// The odd rule for feature bits: a peer setting any one unknown bit is
// refused exactly when the bit is even.
#forall "unknown feature bits: even closes, odd is ignored" bit: 0..<128 {
  f: [16] u8; put := ln_feature_put(f[..], bit); known := [1] int.[8];
  refuse := ln_feature_refuse_vartime(f[..], known[..]);
  if bit / 2 == 4 { #assert refuse == -1; }
  else { #assert (refuse == bit) == (bit % 2 == 0) && (refuse == -1) == (bit % 2 == 1); }
}
// The odd rule for message types.
#forall "unknown message types: even closes, odd is ignored" t: 0..<1024 {
  d := ln_dispatch_vartime(t);
  if !ln_known_vartime(t) { #assert (d.code == LN_UNKNOWN_EVEN) == (t % 2 == 0) && d.ignore == (t % 2 == 1); }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-messages.coax
```

`coax test` runs 74 checks, including the loaded files' own checks, in
about 10 seconds on 2026-11-03.

### The library's own checks

`bolt1.coax` is an `#authority` file. Its checks cover BOLT 1's BigSize
decoding and encoding vectors and each refusal code, every TLV stream
vector in its namespaces and the `n1` values, the order, unknown-even,
length and value refusals, `tlv_write_vartime` and truncated integers
round-tripping, the `init` extension vectors, feature bits combined
across both bitmaps and refused by the odd rule, message dispatch and the
65,535-byte limit, and `error`, `warning`, `ping` and `pong` with their
refusals.

```shell
coax/bin/coax test coax/lib/lightning/bolt1.coax
```

`coax test` runs 66 checks, including those of the files it loads, in
about 10 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### BOLT 1 and an independent reader

[bolt1.py](../../coax/tests/lightning/bolt1.py) computes every line the
example prints with its own Python reader for BigSize, TLV streams, BOLT
1's test namespaces, `init`, `ping`, `pong` and `error`, written from
BOLT 1's requirements and sharing no code with Coax. Its verdict on each
vector must also match the outcome the pinned BOLT 1 states, and it reads
the feature names from the pinned BOLT 9. The transcript must equal the
fixture.

```shell
python3 -I coax/tests/lightning/bolt1.py --coax coax/bin/coax
```

On 2026-11-03 all 98 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

[make_bolt1_vectors.py](../../coax/tests/lightning/make_bolt1_vectors.py)
reads the pinned BOLT 1 into
[bolt1_vectors.coax](../../coax/tests/lightning/bolt1_vectors.coax).

## What this chapter leaves out

- The Noise transport that carries the messages: chapter 105.
- Feature dependencies, such as `basic_mpp` needing `payment_secret`: a
  node checks them when it knows the features, in chapter 109.
- `remote_addr`'s address descriptors, which BOLT 7 defines: chapter 112.
- `peer_storage` and `peer_storage_retrieval`, and BOLT 1's signed-integer
  vectors, which no chapter here uses yet.
- UTF-8 validation of text fields, which the messages here do not have.

## Summary

- A Lightning message is a 2-byte big-endian type and a payload, at most
  65,535 bytes.
- "It's OK to be odd": unknown odd types and feature bits are ignored,
  unknown even ones close the connection.
- BigSize is big-endian CompactSize, and only the shortest encoding is
  valid.
- A TLV stream is records in strictly increasing type order; a reader
  refuses repeats, short values and unknown even types, and skips unknown
  odd ones.
- Truncated integers drop their leading zeros, and must.
- `init` comes first and carries the feature bitmaps and networks.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Sources

- [BOLT 1](https://github.com/lightning/bolts/blob/master/01-messaging.md),
  "Lightning Message Format", "Type-Length-Value Format", "Fundamental
  Types", "Setup Messages", "Control Messages" and Appendices A to C, and
  [BOLT 9](https://github.com/lightning/bolts/blob/master/09-features.md),
  at lightning/bolts revision `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/01-messaging.md).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
