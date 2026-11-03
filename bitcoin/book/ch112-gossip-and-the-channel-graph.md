# 112. Gossip and the channel graph

To pay a node it has no channel with, a Lightning node needs a map: which
channels exist, who is at each end, and what each end charges to relay a
payment. No server hands out that map. Nodes tell each other, and pass on
what they hear: that is gossip, and BOLT 7 defines it.

Gossip has three messages. A `channel_announcement` says that a channel
exists, and proves it with four signatures. A `channel_update` gives one
end's fees and limits. A `node_announcement` gives a node's name, color
and addresses. Each is signed, so a node can relay what it heard without
being trusted, and a reader keeps only what checks. BOLT 7 also defines
queries, so a node that was offline can ask a peer for what it missed.
This chapter reads, checks and writes all of them, and keeps the result
in a channel graph.

## What this chapter teaches

- The short channel ID: the block, the transaction and the output of a
  channel's funding output in 8 bytes, written `539268x845x1`.
- `channel_announcement`: the two node IDs, lesser first, and the two
  funding keys, with a signature from each of the four keys over the
  double SHA-256 of everything after the signatures.
- `node_announcement`: a timestamp, an alias in BOLT 1's restricted UTF-8,
  and address descriptors.
- `channel_update`: the direction bit that names the signing end, the
  disable bit, the timestamp that orders updates, the fees, the CLTV
  delta and the HTLC limits; and the CRC32C checksum of an update.
- The queries: `gossip_timestamp_filter`, `query_channel_range` with
  `reply_channel_range`, and `query_short_channel_ids` with its flags,
  in the only encoding BOLT 7 still allows, plain IDs with no zlib.
- A channel graph that accepts an announcement only when its signatures
  verify, an update only from the right end and only when newer, and a
  node announcement only from a node with a channel.

## Where Lightning uses gossip

| Where | What it uses | Chapter |
| --- | --- | --- |
| Opening a public channel | both ends sign the `channel_announcement` after six confirmations | 109 |
| Finding a route | the graph's channels, with each end's fees, CLTV delta and HTLC limits | 113 |
| Connecting to a node | the addresses of its `node_announcement` | 105 |
| Invoices | an `r` field carries a private channel's policy the way a `channel_update` would | 114 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [gossip.coax](../../coax/lib/lightning/gossip.coax) | `gossip_scid`, `gossip_scid_text`, `gossip_scid_parse_vartime` | short channel IDs and their text |
| | `gossip_hash`, `gossip_verify_vartime` | the double SHA-256 every gossip signature signs, and ECDSA verification |
| | `gossip_announcement_read_vartime` | reads and verifies a `channel_announcement` |
| | `gossip_node_read_vartime`, `gossip_text_ok_vartime` | reads and verifies a `node_announcement`, and BOLT 1's text rule |
| | `gossip_update_read_vartime`, `gossip_update_verify_vartime`, `gossip_crc32c` | reads a `channel_update`, verifies it under a node ID, its checksum |
| | `gossip_query_scids_read_vartime`, `gossip_query_range_read_vartime`, `gossip_reply_range_read_vartime`, `gossip_filter_read_vartime`, `gossip_end_read_vartime` | the query messages |
| | `gossip_read_vartime` | any of the eight messages, as a code |
| | `gossip_announcement_write_vartime`, `gossip_node_write_vartime`, `gossip_update_write_vartime`, and the query writers | the writers, signing with chapter 20's ECDSA |
| | `gossip_graph_begin`, `gossip_graph_add_vartime`, `gossip_reply_range_vartime` | the channel graph and its answer to a range query |
| [bolt1.coax](../../coax/lib/lightning/bolt1.coax) | `tlv_next_vartime`, `bigsize_read_vartime`, `ln_feature_refuse_vartime` | TLVs, BigSize and feature bits (chapter 104) |
| [ecdsa_verify.coax](../../coax/lib/crypto/ecdsa_verify.coax), [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_verify_vartime`, `ecdsa_sign` | ECDSA (chapters 19 and 20) |

Gossip is public: anyone can read it and anyone can send it. So every
reader branches on the message's bytes and ends in `_vartime`. Only the
writers hold a secret, and they pass it straight to chapter 20's
constant-time signer.

The library's codes:

<!-- source: coax/lib/lightning/gossip.coax codes -->
```jai
GSP_OK :: 0;
GSP_TRUNCATED :: 1;        // the bytes end inside a field
GSP_WRONG_TYPE :: 2;       // not the message the reader expects
GSP_BAD_POINT :: 3;        // a node ID or bitcoin key that is not a compressed point
GSP_KEY_ORDER :: 4;        // node_id_1 is not the lesser of the two node IDs
GSP_NOT_MINIMAL :: 5;      // a feature bitmap with a leading zero byte, or a long BigSize
GSP_BAD_SIGNATURE :: 6;    // a signature that does not verify
GSP_BAD_ALIAS :: 7;        // an alias that is not BOLT 1's restricted UTF-8
GSP_BAD_ADDRESS :: 8;      // an address descriptor that runs past addrlen
GSP_BAD_ENCODING :: 9;     // an encoding type other than 0, zlib's 1 included
GSP_BAD_LENGTH :: 10;      // short channel IDs that are not whole, or a field of the wrong size
GSP_ORDER :: 11;           // short channel IDs not ascending, or TLV types not increasing
GSP_UNKNOWN_EVEN :: 12;    // an unknown even TLV type
GSP_BAD_COUNT :: 13;       // not one flag, timestamp pair or checksum pair per short channel ID
GSP_UNKNOWN_CHAIN :: 14;   // a chain hash the graph does not follow
GSP_UNKNOWN_CHANNEL :: 15; // an update for a channel the graph has no announcement for
GSP_UNKNOWN_NODE :: 16;    // a node announcement from a node with no channel
GSP_STALE :: 17;           // not newer than what the graph holds
GSP_BAD_TEXT :: 18;        // short channel ID text that is not three decimal numbers
GSP_CAPACITY :: 19;        // the caller's buffer or the graph is full

GSP_CHANNEL_ANNOUNCEMENT :: 256;
GSP_NODE_ANNOUNCEMENT :: 257;
GSP_CHANNEL_UPDATE :: 258;
GSP_QUERY_SCIDS :: 261;
GSP_REPLY_SCIDS_END :: 262;
GSP_QUERY_RANGE :: 263;
GSP_REPLY_RANGE :: 264;
GSP_TIMESTAMP_FILTER :: 265;

GSP_SIGNATURE :: 64;       // r || s, 32 bytes each
GSP_POINT :: 33;           // a compressed public key
GSP_UPDATE_SIZE :: 138;    // a channel_update with its type
```

A reader stops at the first rule a message breaks and returns its code.
BOLT 7 asks a node to ignore such a message, and sometimes to send a
`warning` or close the connection; that choice is the caller's.

## What a signature signs

All three gossip messages put their signatures first and sign the double
SHA-256 (chapter 5) of everything after them. The signatures cannot sign
themselves, and the rule "everything after" means a field that a later
BOLT appends to the message is signed too, even by a reader that does not
know what it means.

<!-- source: coax/lib/lightning/gossip.coax hash -->
```jai
// Every gossip signature signs the double SHA-256 of the message after
// its signatures: the type and the signatures themselves are left out,
// and any field a later BOLT appends is in.
gossip_hash :: (signed: [] u8, out: [] u8) {
  sha: [1] ShaOwned; once: [32] u8;
  #assert so_init(sha[..]) && so_update(sha[..], signed, false) && so_finish(sha[..], once[..], false);
  #assert so_init(sha[..]) && so_update(sha[..], once[..], false) && so_finish(sha[..], out, false);
}
// BOLT 7 does not ask for a low s, and its rationale notes that anyone
// can turn s into n - s: both verify.
gossip_verify_vartime :: (digest: [] u8, key: [] u8, signature: [] u8) -> bool {
  return ecdsa_verify_vartime(digest, key, signature, false);
}
```

BOLT 7 does not ask for a low s. Its rationale for `channel_update` notes
that a relaying node can replace s with n - s and the signature still
verifies, so a reader must not take a changed signature as a changed
message. The reader here accepts both, and the graph compares timestamps,
not signatures.

## Short channel IDs

A channel is its funding output, and a confirmed output has a short
address: the height of its block, its transaction's place in that block,
and its index in the transaction. Three, three and two bytes make 8, and
the text joins the three numbers with `x`, a letter that a double click
selects with the digits.

<!-- source: coax/lib/lightning/gossip.coax scid -->
```jai
// A short channel ID names the funding output: 3 bytes of block height,
// 3 of the transaction's index in the block, 2 of the output's index.
// Its text is the three numbers in decimal, joined by 'x'.
gossip_scid :: (height: u64, tx: u64, output: u64) -> u64 { return (height << 40) | (tx << 16) | output; }
gossip_scid_height :: (v: u64) -> u64 { return v >> 40; }
gossip_scid_tx :: (v: u64) -> u64 { return (v >> 16) & 0xffffff; }
gossip_scid_output :: (v: u64) -> u64 { return v & 0xffff; }
gsp_decimal :: (v: u64, out: [] u8, at: int) -> int {
  digits: [20] u8; n := 0; x := v;
  while true { digits[n] = cast(u8) ('0' + cast(int) (x % 10)); n += 1; x /= 10; if x == 0 { break; } }
  for i: 0..<n { out[at + i] = digits[n - 1 - i]; }
  return at + n;
}
// Writes the text and returns its length; out needs 23 bytes.
gossip_scid_text :: (v: u64, out: [] u8) -> int {
  at := gsp_decimal(gossip_scid_height(v), out, 0); out[at] = 'x';
  at = gsp_decimal(gossip_scid_tx(v), out, at + 1); out[at] = 'x';
  return gsp_decimal(gossip_scid_output(v), out, at + 1);
}
GspScid :: struct { code: int; value: u64; }
// Three decimal numbers without leading zeros, each in its field's range.
gossip_scid_parse_vartime :: (t: string) -> GspScid {
  parts: [3] u64; limits := [3] u64.[1 << 24, 1 << 24, 1 << 16];
  k := 0; at := 0;
  while k < 3 {
    start := at; v: u64 = 0;
    while at < t.count && t[at] >= '0' && t[at] <= '9' {
      v = v * 10 + cast(u64) (t[at] - '0'); at += 1;
      if v >= limits[k] { return GspScid.{code = GSP_BAD_TEXT}; }
    }
    if at == start || (t[start] == '0' && at - start > 1) { return GspScid.{code = GSP_BAD_TEXT}; }
    parts[k] = v; k += 1;
    if k < 3 {
      if at >= t.count || t[at] != 'x' { return GspScid.{code = GSP_BAD_TEXT}; }
      at += 1;
    }
  }
  if at != t.count { return GspScid.{code = GSP_BAD_TEXT}; }
  return GspScid.{value = gossip_scid(parts[0], parts[1], parts[2])};
}
```

## Channel announcements

A channel announcement proves three things to a node that sees the
funding output in its own copy of the chain. The funding output pays to
`bitcoin_key_1` and `bitcoin_key_2`; that check needs the UTXO set and is
left to the caller. `node_id_1` controls `bitcoin_key_1`, and `node_id_2`
controls `bitcoin_key_2`. And both nodes agree to announce. Each of the
four keys signs the same hash, so one message carries all four proofs.

`node_id_1` must be the lesser of the two compressed keys, compared byte
by byte. With the order fixed, a channel has one announcement, and a
`channel_update` can name its end with one bit.

<!-- source: coax/lib/lightning/gossip.coax announcement -->
```jai
// channel_announcement: four signatures, then the features, the chain,
// the short channel ID, the two node IDs and the two funding keys. Its
// positions are offsets into the message.
GspAnnouncement :: struct {
  code: int;
  features: int; features_n: int;
  chain: int; scid: u64;
  node1: int; node2: int; bitcoin1: int; bitcoin2: int;
  routable: bool;        // no even feature bit the reader does not know
  hash: [32] u8;
}
gossip_announcement_read_vartime :: (m: [] u8) -> GspAnnouncement {
  if gsp_type(m) != GSP_CHANNEL_ANNOUNCEMENT { return GspAnnouncement.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < 2 + 4 * GSP_SIGNATURE + 2 { return GspAnnouncement.{code = GSP_TRUNCATED}; }
  a: GspAnnouncement;
  at := 2 + 4 * GSP_SIGNATURE;
  a.features_n = ln_u16(m, at); a.features = at + 2; at = a.features + a.features_n;
  if m.count - at < 32 + 8 + 4 * GSP_POINT { a.code = GSP_TRUNCATED; return a; }
  a.chain = at; a.scid = gsp_u64(m, at + 32);
  a.node1 = at + 40; a.node2 = a.node1 + 33; a.bitcoin1 = a.node2 + 33; a.bitcoin2 = a.bitcoin1 + 33;
  if a.features_n > 0 && m[a.features] == 0 { a.code = GSP_NOT_MINIMAL; return a; }
  keys := [4] int.[a.node1, a.node2, a.bitcoin1, a.bitcoin2];
  for k: keys { if !gsp_point(m[k..<k + GSP_POINT]) { a.code = GSP_BAD_POINT; return a; } }
  if !gsp_less(m[a.node1..<a.node1 + GSP_POINT], m[a.node2..<a.node2 + GSP_POINT]) { a.code = GSP_KEY_ORDER; return a; }
  gossip_hash(m[2 + 4 * GSP_SIGNATURE..<m.count], a.hash[..]);
  // node_signature_1 and _2, then bitcoin_signature_1 and _2: each key signs the same hash.
  for i: 0..<4 {
    s := 2 + GSP_SIGNATURE * i; k := keys[i];
    if !gossip_verify_vartime(a.hash[..], m[k..<k + GSP_POINT], m[s..<s + GSP_SIGNATURE]) { a.code = GSP_BAD_SIGNATURE; return a; }
  }
  none: [1] int;
  a.routable = ln_feature_refuse_vartime(m[a.features..<a.features + a.features_n], none[0..<0]) < 0;
  return a;
}
```

An unknown even feature bit does not make the announcement invalid: BOLT
7 says a node must not route through such a channel. The reader reports
that as `routable`.

## Node announcements

A node announcement is signed by its node ID, and gives a timestamp, a
color, an alias and addresses. The alias is 32 bytes, padded with zero
bytes, and must be BOLT 1's restricted UTF-8: no control characters,
format characters, surrogates or private-use characters, and no line or
paragraph separator. A map that shows aliases would otherwise show
invisible characters, or text that reverses its direction. The forbidden
code points are the ranges of those Unicode categories:

<!-- source: coax/lib/lightning/gossip.coax text -->
```jai
// BOLT 1's text: valid UTF-8 with no character of the Unicode categories
// Cc, Cf, Cs and Co, and neither U+2028 nor U+2029. The ranges are those
// code points as of Unicode 17.
GSP_FORBIDDEN :: [52] u32.[
  0x0, 0x1f, 0x7f, 0x9f, 0xad, 0xad, 0x600, 0x605, 0x61c, 0x61c, 0x6dd, 0x6dd,
  0x70f, 0x70f, 0x890, 0x891, 0x8e2, 0x8e2, 0x180e, 0x180e, 0x200b, 0x200f, 0x2028, 0x202e,
  0x2060, 0x2064, 0x2066, 0x206f, 0xd800, 0xf8ff, 0xfeff, 0xfeff, 0xfff9, 0xfffb, 0x110bd, 0x110bd,
  0x110cd, 0x110cd, 0x13430, 0x1343f, 0x1bca0, 0x1bca3, 0x1d173, 0x1d17a, 0xe0001, 0xe0001,
  0xe0020, 0xe007f, 0xf0000, 0xffffd, 0x100000, 0x10fffd
];
gossip_text_ok_vartime :: (b: [] u8) -> bool {
  at := 0;
  while at < b.count {
    c := cast(int) b[at];
    n := 0; v := c; least := 0;
    if c >= 0x80 {
      if c >= 0xc2 && c <= 0xdf { n = 1; least = 0x80; v = c & 0x1f; }
      else if c >= 0xe0 && c <= 0xef { n = 2; least = 0x800; v = c & 0x0f; }
      else if c >= 0xf0 && c <= 0xf4 { n = 3; least = 0x10000; v = c & 0x07; }
      else { return false; }
      if at + n >= b.count { return false; }
      for i: 1..<n + 1 {
        d := cast(int) b[at + i];
        if d < 0x80 || d > 0xbf { return false; }
        v = (v << 6) | (d & 0x3f);
      }
      if v < least || v > 0x10ffff || (v >= 0xd800 && v <= 0xdfff) { return false; }
    }
    for r: 0..<26 { if cast(u32) v >= GSP_FORBIDDEN[2 * r] && cast(u32) v <= GSP_FORBIDDEN[2 * r + 1] { return false; } }
    at += n + 1;
  }
  return true;
}
```

Each address descriptor starts with its type: IPv4 (1), IPv6 (2), the
retired Tor v2 (3), Tor v3 (4) and a DNS hostname (5). A reader cannot
know the length of a type it does not know, so an unknown type ends the
list. A port of 0, a Tor v2 address, a hostname that is not ASCII, and a
second hostname are skipped. A descriptor that runs past `addrlen` is an
error.

<!-- source: coax/lib/lightning/gossip.coax node -->
```jai
// node_announcement: a signature, features, a timestamp, the node ID, a
// color, a 32-byte alias padded with zeros, and address descriptors. An
// address's start is its data after the type byte, port last.
GSP_MAX_ADDRESSES :: 8;
GspAddress :: struct { type: int; start: int; length: int; }
GspNode :: struct {
  code: int;
  features: int; features_n: int;
  timestamp: u64; node: int; rgb: int; alias: int; alias_n: int;
  addresses: [8] GspAddress; address_count: int;
}
// The descriptors of known types, as BOLT 7 lists them: the length of
// each type's data, -1 for a hostname (a length byte, then the name).
gsp_address_length :: (type: int) -> int {
  if type == 1 { return 6; } if type == 2 { return 18; } if type == 3 { return 12; }
  if type == 4 { return 37; } if type == 5 { return -1; }
  return 0;
}
// A descriptor of an unknown type ends the list, since its length is
// unknown. Tor v2 (type 3), port 0, a second hostname and a hostname that
// is not ASCII are skipped.
gsp_addresses_vartime :: (m: [] u8, from: int, end: int, n: [] GspNode) -> int {
  hostnames := 0; at := from;
  while at < end {
    type := cast(int) m[at]; length := gsp_address_length(type);
    if length == 0 { return GSP_OK; }
    if length < 0 {
      if at + 1 >= end { return GSP_BAD_ADDRESS; }
      length = 1 + cast(int) m[at + 1] + 2;
    }
    if length > end - at - 1 { return GSP_BAD_ADDRESS; }
    start := at + 1; at = start + length;
    keep := type != 3 && ln_u16(m, at - 2) != 0;
    if type == 5 {
      hostnames += 1;
      if hostnames > 1 { keep = false; }
      for i: start + 1..<at - 2 { if m[i] >= 0x80 { keep = false; } }
    }
    if keep && n[0].address_count < GSP_MAX_ADDRESSES {
      n[0].addresses[n[0].address_count] = GspAddress.{type = type, start = start, length = length};
      n[0].address_count += 1;
    }
  }
  return GSP_OK;
}
gossip_node_read_vartime :: (m: [] u8, n: [] GspNode) -> int {
  fresh: GspNode; n[0] = fresh;
  if gsp_type(m) != GSP_NODE_ANNOUNCEMENT { n[0].code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE; return n[0].code; }
  at := 2 + GSP_SIGNATURE;
  if m.count - at < 2 { n[0].code = GSP_TRUNCATED; return n[0].code; }
  n[0].features_n = ln_u16(m, at); n[0].features = at + 2; at = n[0].features + n[0].features_n;
  if m.count - at < 4 + 33 + 3 + 32 + 2 { n[0].code = GSP_TRUNCATED; return n[0].code; }
  n[0].timestamp = gsp_u32(m, at); n[0].node = at + 4; n[0].rgb = at + 37; n[0].alias = at + 40;
  at += 72;
  addresses := ln_u16(m, at); at += 2;
  if addresses > m.count - at { n[0].code = GSP_TRUNCATED; return n[0].code; }
  if !gsp_point(m[n[0].node..<n[0].node + GSP_POINT]) { n[0].code = GSP_BAD_POINT; return n[0].code; }
  digest: [32] u8;
  gossip_hash(m[2 + GSP_SIGNATURE..<m.count], digest[..]);
  if !gossip_verify_vartime(digest[..], m[n[0].node..<n[0].node + GSP_POINT], m[2..<2 + GSP_SIGNATURE]) { n[0].code = GSP_BAD_SIGNATURE; return n[0].code; }
  if n[0].features_n > 0 && m[n[0].features] == 0 { n[0].code = GSP_NOT_MINIMAL; return n[0].code; }
  // The alias is the text before its trailing zero bytes.
  n[0].alias_n = 32;
  while n[0].alias_n > 0 && m[n[0].alias + n[0].alias_n - 1] == 0 { n[0].alias_n -= 1; }
  if !gossip_text_ok_vartime(m[n[0].alias..<n[0].alias + n[0].alias_n]) { n[0].code = GSP_BAD_ALIAS; return n[0].code; }
  n[0].code = gsp_addresses_vartime(m, at, at + addresses, n);
  return n[0].code;
}
```

## Channel updates

Each end of a channel says what it charges to forward a payment through
it: a base fee in millisatoshis, a proportional fee in millionths, the
CLTV delta it subtracts from an HTLC's expiry, and the smallest and
largest HTLC it relays. Bit 0 of `channel_flags` says which end this is:
0 for `node_id_1`, 1 for `node_id_2`. Bit 1 disables the channel, for
example while the peer is offline. An update has no node ID of its own:
the reader takes the key from the channel's announcement by the
direction bit. A newer timestamp replaces an older update.

A peer catching up on gossip may want to know whether its copy of an
update is current without fetching it. BOLT 7's answer is a checksum:
CRC32C of the update without its signature and timestamp. Two updates
with the same policy have the same checksum, whenever they were signed.

<!-- source: coax/lib/lightning/gossip.coax crc -->
```jai
// CRC32C (RFC 3720, appendix B.4): the reflected polynomial 0x82F63B78,
// started at all ones and finished by inverting.
gsp_crc :: (c: u32, data: [] u8) -> u32 {
  x := c;
  for b: data {
    x ^= cast(u32) b;
    for k: 0..<8 { x = ifx (x & 1) == 1 then (x >> 1) ^ 0x82f63b78 else x >> 1; }
  }
  return x;
}
gossip_crc32c :: (data: [] u8) -> u32 { return gsp_crc(0xffffffff, data) ^ 0xffffffff; }
```

<!-- source: coax/lib/lightning/gossip.coax update -->
```jai
// channel_update: a signature, the chain, the short channel ID, a
// timestamp, two flag bytes and the relay policy. Bit 0 of channel_flags
// says which end signed it, bit 1 disables the channel.
GspUpdate :: struct {
  code: int;
  chain: int; scid: u64; timestamp: u64;
  message_flags: int; channel_flags: int; direction: int; disabled: bool;
  cltv: int; htlc_min: u64; fee_base: u64; fee_ppm: u64; htlc_max: u64;
  checksum: u32;   // CRC32C of the update without signature and timestamp
  hash: [32] u8;
}
gossip_update_read_vartime :: (m: [] u8) -> GspUpdate {
  if gsp_type(m) != GSP_CHANNEL_UPDATE { return GspUpdate.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < GSP_UPDATE_SIZE { return GspUpdate.{code = GSP_TRUNCATED}; }
  u: GspUpdate;
  u.chain = 66; u.scid = gsp_u64(m, 98); u.timestamp = gsp_u32(m, 106);
  u.message_flags = cast(int) m[110]; u.channel_flags = cast(int) m[111];
  u.direction = u.channel_flags & 1; u.disabled = (u.channel_flags & 2) != 0;
  u.cltv = ln_u16(m, 112); u.htlc_min = gsp_u64(m, 114);
  u.fee_base = gsp_u32(m, 122); u.fee_ppm = gsp_u32(m, 126); u.htlc_max = gsp_u64(m, 130);
  gossip_hash(m[2 + GSP_SIGNATURE..<m.count], u.hash[..]);
  u.checksum = gsp_crc(gsp_crc(0xffffffff, m[66..<106]), m[110..<m.count]) ^ 0xffffffff;
  return u;
}
// An update carries no key: the graph takes node_id_1 or node_id_2 from
// the channel's announcement, by the direction bit.
gossip_update_verify_vartime :: (m: [] u8, u: GspUpdate, key: [] u8) -> bool {
  digest := u.hash;
  return gossip_verify_vartime(digest[..], key, m[2..<2 + GSP_SIGNATURE]);
}
```

## Queries

A node that has been offline asks a peer what it missed. BOLT 7 gives it
three ways:

- `gossip_timestamp_filter`: send me gossip with timestamps from
  `first_timestamp`, for `timestamp_range` seconds. A node must send one
  to receive any gossip at all.
- `query_channel_range`: which channels were opened in these blocks? Each
  `reply_channel_range` lists short channel IDs and, if asked, each
  end's update timestamp and checksum, so the node can tell which updates
  it lacks.
- `query_short_channel_ids`: send me these channels' messages. A flag per
  ID can ask for the announcement, either end's update, or either end's
  node announcement.

Lists of IDs, timestamps and flags start with an encoding byte. Encoding
1 was zlib; BOLT 7 now says it "MUST NOT be used", so 0, the plain list
in ascending order, is the only one left, and a reader refuses 1.

<!-- source: coax/lib/lightning/gossip.coax queries -->
```jai
// encoded_short_ids: an encoding byte, then the IDs. Only 0, plain 8-byte
// IDs in ascending order, is left: BOLT 7 now forbids 1, zlib.
GspScids :: struct { code: int; start: int; count: int; }
gsp_scids_vartime :: (m: [] u8, at: int, n: int) -> GspScids {
  if n == 0 { return GspScids.{code = GSP_TRUNCATED}; }
  if m[at] != 0 { return GspScids.{code = GSP_BAD_ENCODING}; }
  if (n - 1) % 8 != 0 { return GspScids.{code = GSP_BAD_LENGTH}; }
  s := GspScids.{start = at + 1, count = (n - 1) / 8};
  for i: 1..<s.count { if gsp_u64(m, s.start + 8 * i) <= gsp_u64(m, s.start + 8 * i - 8) { return GspScids.{code = GSP_ORDER}; } }
  return s;
}
gossip_scid_at :: (m: [] u8, s: GspScids, i: int) -> u64 { return gsp_u64(m, s.start + 8 * i); }
// The TLV stream after the fixed fields: records of the known types are
// returned by position, unknown odd ones skipped.
GspTlvs :: struct { code: int; one: int; one_n: int; three: int; three_n: int; }
gsp_tlvs_vartime :: (m: [] u8, at: int, three_known: bool) -> GspTlvs {
  r := GspTlvs.{one = -1, three = -1};
  previous: u64 = 0; first := true; b := m[at..<m.count];
  p := 0;
  while p < b.count {
    t := tlv_next_vartime(b, p, previous, first);
    if t.code != LN_OK { r.code = gsp_ln_code(t.code); return r; }
    if t.type == 1 { r.one = at + t.start; r.one_n = t.length; }
    else if t.type == 3 && three_known { r.three = at + t.start; r.three_n = t.length; }
    else if t.type % 2 == 0 { r.code = GSP_UNKNOWN_EVEN; return r; }
    previous = t.type; first = false; p = t.next;
  }
  return r;
}
// query_short_channel_ids, with optional query_flags: one BigSize per ID.
GspQuery :: struct { code: int; chain: int; scids: GspScids; flags: int; flags_n: int; }
gossip_query_scids_read_vartime :: (m: [] u8) -> GspQuery {
  if gsp_type(m) != GSP_QUERY_SCIDS { return GspQuery.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < 2 + 32 + 2 { return GspQuery.{code = GSP_TRUNCATED}; }
  q := GspQuery.{chain = 2, flags = -1};
  n := ln_u16(m, 34);
  if n > m.count - 36 { q.code = GSP_TRUNCATED; return q; }
  q.scids = gsp_scids_vartime(m, 36, n);
  if q.scids.code != GSP_OK { q.code = q.scids.code; return q; }
  t := gsp_tlvs_vartime(m, 36 + n, false);
  if t.code != GSP_OK { q.code = t.code; return q; }
  if t.one >= 0 {
    if t.one_n == 0 { q.code = GSP_TRUNCATED; return q; }
    if m[t.one] != 0 { q.code = GSP_BAD_ENCODING; return q; }
    q.flags = t.one + 1;
    v := m[q.flags..<t.one + t.one_n]; p := 0;
    while p < v.count {
      f := bigsize_read_vartime(v, p);
      if f.code != LN_OK { q.code = gsp_ln_code(f.code); return q; }
      p = f.next; q.flags_n += 1;
    }
    if q.flags_n != q.scids.count { q.code = GSP_BAD_COUNT; return q; }
  }
  return q;
}
// The i-th query flag: bit 0 asks for the channel_announcement, bits 1
// and 2 for each end's channel_update, bits 3 and 4 for each end's
// node_announcement.
gossip_query_flag :: (m: [] u8, q: GspQuery, i: int) -> u64 {
  p := q.flags; f: LnBigSize;
  for k: 0..<i + 1 { f = bigsize_read_vartime(m, p); p = f.next; }
  return f.value;
}
// reply_short_channel_ids_end: the chain and full_information.
GspEnd :: struct { code: int; chain: int; full: int; }
gossip_end_read_vartime :: (m: [] u8) -> GspEnd {
  if gsp_type(m) != GSP_REPLY_SCIDS_END { return GspEnd.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < 2 + 32 + 1 { return GspEnd.{code = GSP_TRUNCATED}; }
  return GspEnd.{chain = 2, full = cast(int) m[34]};
}
// query_channel_range and reply_channel_range share one shape. A query's
// option bit 0 asks for timestamps, bit 1 for checksums; a reply's
// timestamps and checksums are pairs of u32, one pair per ID.
GspRange :: struct {
  code: int; chain: int; first: u64; number: u64; options: u64; complete: int;
  scids: GspScids; timestamps: int; checksums: int;   // -1 when absent
}
gossip_query_range_read_vartime :: (m: [] u8) -> GspRange {
  if gsp_type(m) != GSP_QUERY_RANGE { return GspRange.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < 2 + 32 + 8 { return GspRange.{code = GSP_TRUNCATED}; }
  r := GspRange.{chain = 2, first = gsp_u32(m, 34), number = gsp_u32(m, 38), timestamps = -1, checksums = -1};
  t := gsp_tlvs_vartime(m, 42, false);
  if t.code != GSP_OK { r.code = t.code; return r; }
  if t.one >= 0 {
    v := m[t.one..<t.one + t.one_n];
    f := bigsize_read_vartime(v, 0);
    if f.code != LN_OK { r.code = gsp_ln_code(f.code); return r; }
    if f.next != v.count { r.code = GSP_BAD_LENGTH; return r; }
    r.options = f.value;
  }
  return r;
}
gossip_reply_range_read_vartime :: (m: [] u8) -> GspRange {
  if gsp_type(m) != GSP_REPLY_RANGE { return GspRange.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < 2 + 32 + 9 + 2 { return GspRange.{code = GSP_TRUNCATED}; }
  r := GspRange.{chain = 2, first = gsp_u32(m, 34), number = gsp_u32(m, 38), complete = cast(int) m[42], timestamps = -1, checksums = -1};
  n := ln_u16(m, 43);
  if n > m.count - 45 { r.code = GSP_TRUNCATED; return r; }
  r.scids = gsp_scids_vartime(m, 45, n);
  if r.scids.code != GSP_OK { r.code = r.scids.code; return r; }
  t := gsp_tlvs_vartime(m, 45 + n, true);
  if t.code != GSP_OK { r.code = t.code; return r; }
  if t.one >= 0 {
    if t.one_n == 0 { r.code = GSP_TRUNCATED; return r; }
    if m[t.one] != 0 { r.code = GSP_BAD_ENCODING; return r; }
    if t.one_n - 1 != 8 * r.scids.count { r.code = GSP_BAD_COUNT; return r; }
    r.timestamps = t.one + 1;
  }
  if t.three >= 0 {
    if t.three_n != 8 * r.scids.count { r.code = GSP_BAD_COUNT; return r; }
    r.checksums = t.three;
  }
  return r;
}
// gossip_timestamp_filter: send me gossip with first <= timestamp <
// first + range. The sum is taken in 64 bits, so it cannot wrap.
GspFilter :: struct { code: int; chain: int; first: u64; range: u64; }
gossip_filter_read_vartime :: (m: [] u8) -> GspFilter {
  if gsp_type(m) != GSP_TIMESTAMP_FILTER { return GspFilter.{code = ifx m.count < 2 then GSP_TRUNCATED else GSP_WRONG_TYPE}; }
  if m.count < 2 + 32 + 8 { return GspFilter.{code = GSP_TRUNCATED}; }
  return GspFilter.{chain = 2, first = gsp_u32(m, 34), range = gsp_u32(m, 38)};
}
gossip_filter_passes :: (f: GspFilter, timestamp: u64) -> bool { return timestamp >= f.first && timestamp < f.first + f.range; }
```

`gossip_read_vartime` reads any of the eight messages:

<!-- source: coax/lib/lightning/gossip.coax read -->
```jai
// Any of the eight messages, as a code.
gossip_read_vartime :: (m: [] u8) -> int {
  t := gsp_type(m);
  if t == GSP_CHANNEL_ANNOUNCEMENT { return gossip_announcement_read_vartime(m).code; }
  if t == GSP_NODE_ANNOUNCEMENT { n: [1] GspNode; return gossip_node_read_vartime(m, n[..]); }
  if t == GSP_CHANNEL_UPDATE { return gossip_update_read_vartime(m).code; }
  if t == GSP_QUERY_SCIDS { return gossip_query_scids_read_vartime(m).code; }
  if t == GSP_REPLY_SCIDS_END { return gossip_end_read_vartime(m).code; }
  if t == GSP_QUERY_RANGE { return gossip_query_range_read_vartime(m).code; }
  if t == GSP_REPLY_RANGE { return gossip_reply_range_read_vartime(m).code; }
  if t == GSP_TIMESTAMP_FILTER { return gossip_filter_read_vartime(m).code; }
  return ifx t < 0 then GSP_TRUNCATED else GSP_WRONG_TYPE;
}
```

## Writing

The writers lay out the fields and sign. The channel announcement writer
orders the two nodes, and their funding keys with them, so that
`node_id_1` is the lesser key.

<!-- source: coax/lib/lightning/gossip.coax writer -->
```jai
// The writers return the message's length, or -1 when out is too small.
gsp_sign :: (key: [] u8, signed: [] u8, out: [] u8) {
  digest: [32] u8; work: [1] EcdsaWork;
  gossip_hash(signed, digest[..]);
  #assert ecdsa_sign(key, digest[..], work[..], out, false);
}
gsp_put :: (out: [] u8, at: int, v: [] u8) -> int { for i: 0..<v.count { out[at + i] = v[i]; } return at + v.count; }
// Both ends sign: node_id_1 is the lesser node ID, so the writer orders
// the two nodes and their funding keys before it signs.
gossip_announcement_write_vartime :: (chain: [] u8, scid: u64, node_a: [] u8, node_b: [] u8,
                                      bitcoin_a: [] u8, bitcoin_b: [] u8, features: [] u8, out: [] u8) -> int {
  size := 2 + 4 * GSP_SIGNATURE + 2 + features.count + 32 + 8 + 4 * GSP_POINT;
  if out.count < size { return -1; }
  a := gsp_public(node_a); b := gsp_public(node_b);
  swap := gsp_less(b[..], a[..]);
  ka := ifx swap then node_b else node_a; kb := ifx swap then node_a else node_b;
  ba := ifx swap then bitcoin_b else bitcoin_a; bb := ifx swap then bitcoin_a else bitcoin_b;
  ln_put16(out, 0, GSP_CHANNEL_ANNOUNCEMENT);
  at := 2 + 4 * GSP_SIGNATURE;
  ln_put16(out, at, features.count); at = gsp_put(out, at + 2, features);
  at = gsp_put(out, at, chain); gsp_put64(out, at, scid); at += 8;
  p1 := gsp_public(ka); p2 := gsp_public(kb); p3 := gsp_public(ba); p4 := gsp_public(bb);
  at = gsp_put(out, at, p1[..]); at = gsp_put(out, at, p2[..]);
  at = gsp_put(out, at, p3[..]); at = gsp_put(out, at, p4[..]);
  signed := out[2 + 4 * GSP_SIGNATURE..<size];
  gsp_sign(ka, signed, out[2..<66]); gsp_sign(kb, signed, out[66..<130]);
  gsp_sign(ba, signed, out[130..<194]); gsp_sign(bb, signed, out[194..<258]);
  return size;
}
gossip_node_write_vartime :: (key: [] u8, timestamp: u64, rgb: [] u8, alias: string, addresses: [] u8,
                              features: [] u8, out: [] u8) -> int {
  size := 2 + GSP_SIGNATURE + 2 + features.count + 4 + 33 + 3 + 32 + 2 + addresses.count;
  if out.count < size || alias.count > 32 { return -1; }
  ln_put16(out, 0, GSP_NODE_ANNOUNCEMENT);
  at := 2 + GSP_SIGNATURE;
  ln_put16(out, at, features.count); at = gsp_put(out, at + 2, features);
  gsp_put32(out, at, timestamp); at += 4;
  id := gsp_public(key); at = gsp_put(out, at, id[..]);
  at = gsp_put(out, at, rgb);
  for i: 0..<32 { out[at + i] = ifx i < alias.count then alias[i] else 0; }
  at += 32;
  ln_put16(out, at, addresses.count); at = gsp_put(out, at + 2, addresses);
  gsp_sign(key, out[2 + GSP_SIGNATURE..<size], out[2..<2 + GSP_SIGNATURE]);
  return size;
}
// One end's policy for a channel: must_be_one is set in message_flags,
// direction and disable are channel_flags' bits 0 and 1.
gossip_update_write_vartime :: (key: [] u8, chain: [] u8, scid: u64, timestamp: u64, channel_flags: int,
                                cltv: int, htlc_min: u64, fee_base: u64, fee_ppm: u64, htlc_max: u64, out: [] u8) -> int {
  if out.count < GSP_UPDATE_SIZE { return -1; }
  ln_put16(out, 0, GSP_CHANNEL_UPDATE);
  at := gsp_put(out, 66, chain);
  gsp_put64(out, at, scid); gsp_put32(out, at + 8, timestamp);
  out[110] = 1; out[111] = cast(u8) channel_flags;
  ln_put16(out, 112, cltv); gsp_put64(out, 114, htlc_min);
  gsp_put32(out, 122, fee_base); gsp_put32(out, 126, fee_ppm); gsp_put64(out, 130, htlc_max);
  gsp_sign(key, out[66..<GSP_UPDATE_SIZE], out[2..<66]);
  return GSP_UPDATE_SIZE;
}
gsp_put_scids :: (out: [] u8, at: int, ids: [] u64) -> int {
  ln_put16(out, at, 1 + 8 * ids.count); out[at + 2] = 0;
  for i: 0..<ids.count { gsp_put64(out, at + 3 + 8 * i, ids[i]); }
  return at + 3 + 8 * ids.count;
}
// query_short_channel_ids, with one flag byte per ID when flags is not
// empty (flags below 0xfd are their own BigSize).
gossip_query_scids_write_vartime :: (chain: [] u8, ids: [] u64, flags: [] u8, out: [] u8) -> int {
  size := 2 + 32 + 3 + 8 * ids.count + ifx flags.count > 0 then 3 + flags.count else 0;
  if out.count < size { return -1; }
  ln_put16(out, 0, GSP_QUERY_SCIDS);
  at := gsp_put(out, 2, chain);
  at = gsp_put_scids(out, at, ids);
  if flags.count > 0 {
    out[at] = 1; out[at + 1] = cast(u8) (1 + flags.count); out[at + 2] = 0;
    at = gsp_put(out, at + 3, flags);
  }
  return at;
}
gossip_query_range_write_vartime :: (chain: [] u8, first: u64, number: u64, options: int, out: [] u8) -> int {
  size := 2 + 32 + 8 + ifx options >= 0 then 3 else 0;
  if out.count < size { return -1; }
  ln_put16(out, 0, GSP_QUERY_RANGE);
  at := gsp_put(out, 2, chain);
  gsp_put32(out, at, first); gsp_put32(out, at + 4, number); at += 8;
  if options >= 0 { out[at] = 1; out[at + 1] = 1; out[at + 2] = cast(u8) options; at += 3; }
  return at;
}
gossip_filter_write_vartime :: (chain: [] u8, first: u64, range: u64, out: [] u8) -> int {
  if out.count < 42 { return -1; }
  ln_put16(out, 0, GSP_TIMESTAMP_FILTER);
  at := gsp_put(out, 2, chain);
  gsp_put32(out, at, first); gsp_put32(out, at + 4, range);
  return at + 8;
}
```

## The channel graph

The graph keeps what has checked: each channel's nodes, each end's latest
policy, and each node's latest announcement. It applies BOLT 7's
receiving rules in order. An announcement must verify and name the graph's
chain. An update must name a known channel, be signed by the end its
direction bit names, and be newer than that end's last. A node
announcement must come from a node that has a channel, which stops anyone
from filling the graph with nodes for free, and must be newer than the
last.

<!-- source: coax/lib/lightning/gossip.coax graph -->
```jai
// The channel graph: channels from verified announcements, each end's
// latest policy, and the nodes those channels name.
GSP_MAX_CHANNELS :: 16;
GSP_MAX_NODES :: 32;
GspPolicy :: struct {
  present: bool; timestamp: u64; channel_flags: int; cltv: int;
  htlc_min: u64; fee_base: u64; fee_ppm: u64; htlc_max: u64; checksum: u32;
}
GspChannel :: struct { scid: u64; node1: [33] u8; node2: [33] u8; policy: [2] GspPolicy; }
GspNodeInfo :: struct { id: [33] u8; announced: bool; timestamp: u64; alias: [32] u8; alias_n: int; }
GspGraph :: struct {
  chain: [32] u8;
  channels: [16] GspChannel; count: int;
  nodes: [32] GspNodeInfo; node_count: int;
}
gossip_graph_begin :: (g: [] GspGraph, chain: [] u8) {
  fresh: GspGraph; g[0] = fresh;
  for i: 0..<32 { g[0].chain[i] = chain[i]; }
}
gossip_find_channel :: (g: [] GspGraph, scid: u64) -> int {
  for i: 0..<g[0].count { if g[0].channels[i].scid == scid { return i; } }
  return -1;
}
gossip_find_node :: (g: [] GspGraph, id: [] u8) -> int {
  for i: 0..<g[0].node_count { if cast(string) g[0].nodes[i].id[..] == cast(string) id { return i; } }
  return -1;
}
gsp_know_node :: (g: [] GspGraph, id: [] u8) -> bool {
  if gossip_find_node(g, id) >= 0 { return true; }
  if g[0].node_count == GSP_MAX_NODES { return false; }
  k := g[0].node_count;
  for i: 0..<33 { g[0].nodes[k].id[i] = id[i]; }
  g[0].node_count += 1;
  return true;
}
// Adds one gossip message. An announcement must verify and be on the
// graph's chain; an update must name a known channel, be signed by the
// end its direction bit names, and be newer than that end's last; a node
// announcement must come from a node with a channel and be newer than
// its last.
gossip_graph_add_vartime :: (g: [] GspGraph, m: [] u8) -> int {
  t := gsp_type(m);
  if t == GSP_CHANNEL_ANNOUNCEMENT {
    a := gossip_announcement_read_vartime(m);
    if a.code != GSP_OK { return a.code; }
    if cast(string) m[a.chain..<a.chain + 32] != cast(string) g[0].chain[..] { return GSP_UNKNOWN_CHAIN; }
    if gossip_find_channel(g, a.scid) >= 0 { return GSP_STALE; }
    if g[0].count == GSP_MAX_CHANNELS { return GSP_CAPACITY; }
    if !gsp_know_node(g, m[a.node1..<a.node1 + 33]) || !gsp_know_node(g, m[a.node2..<a.node2 + 33]) { return GSP_CAPACITY; }
    k := g[0].count;
    fresh: GspChannel; g[0].channels[k] = fresh;
    g[0].channels[k].scid = a.scid;
    for i: 0..<33 { g[0].channels[k].node1[i] = m[a.node1 + i]; g[0].channels[k].node2[i] = m[a.node2 + i]; }
    g[0].count += 1;
    return GSP_OK;
  }
  if t == GSP_CHANNEL_UPDATE {
    u := gossip_update_read_vartime(m);
    if u.code != GSP_OK { return u.code; }
    if cast(string) m[u.chain..<u.chain + 32] != cast(string) g[0].chain[..] { return GSP_UNKNOWN_CHAIN; }
    c := gossip_find_channel(g, u.scid);
    if c < 0 { return GSP_UNKNOWN_CHANNEL; }
    key: [33] u8;
    key = ifx u.direction == 0 then g[0].channels[c].node1 else g[0].channels[c].node2;
    if !gossip_update_verify_vartime(m, u, key[..]) { return GSP_BAD_SIGNATURE; }
    old := g[0].channels[c].policy[u.direction];
    if old.present && u.timestamp <= old.timestamp { return GSP_STALE; }
    g[0].channels[c].policy[u.direction] = GspPolicy.{
      present = true, timestamp = u.timestamp, channel_flags = u.channel_flags, cltv = u.cltv,
      htlc_min = u.htlc_min, fee_base = u.fee_base, fee_ppm = u.fee_ppm, htlc_max = u.htlc_max,
      checksum = u.checksum};
    return GSP_OK;
  }
  if t == GSP_NODE_ANNOUNCEMENT {
    n: [1] GspNode;
    code := gossip_node_read_vartime(m, n[..]);
    if code != GSP_OK { return code; }
    k := gossip_find_node(g, m[n[0].node..<n[0].node + 33]);
    if k < 0 { return GSP_UNKNOWN_NODE; }
    if g[0].nodes[k].announced && n[0].timestamp <= g[0].nodes[k].timestamp { return GSP_STALE; }
    g[0].nodes[k].announced = true; g[0].nodes[k].timestamp = n[0].timestamp;
    for i: 0..<32 { g[0].nodes[k].alias[i] = m[n[0].alias + i]; }
    g[0].nodes[k].alias_n = n[0].alias_n;
    return GSP_OK;
  }
  return ifx t < 0 then GSP_TRUNCATED else GSP_WRONG_TYPE;
}
// The answer to query_channel_range: one reply_channel_range with every
// channel whose block is in [first, first + number), in ascending order,
// with timestamps for option bit 0 and checksums for bit 1 (0 for an end
// with no update).
gossip_reply_range_vartime :: (g: [] GspGraph, first: u64, number: u64, options: u64, out: [] u8) -> int {
  ids: [16] u64; n := 0;
  for i: 0..<g[0].count {
    s := g[0].channels[i].scid; h := gossip_scid_height(s);
    if h >= first && h < first + number {
      k := n;
      while k > 0 && ids[k - 1] > s { ids[k] = ids[k - 1]; k -= 1; }
      ids[k] = s; n += 1;
    }
  }
  size := 2 + 32 + 9 + 3 + 8 * n + (ifx (options & 1) != 0 then 3 + 8 * n else 0) + (ifx (options & 2) != 0 then 2 + 8 * n else 0);
  if out.count < size || 1 + 8 * n >= 0xfd { return -1; }
  ln_put16(out, 0, GSP_REPLY_RANGE);
  at := gsp_put(out, 2, g[0].chain[..]);
  gsp_put32(out, at, first); gsp_put32(out, at + 4, number); out[at + 8] = 1;
  at = gsp_put_scids(out, at + 9, ids[0..<n]);
  for pass: 0..<2 {
    if (options >> cast(u64) pass) & 1 == 1 {
      out[at] = cast(u8) (1 + 2 * pass);
      out[at + 1] = cast(u8) (8 * n + 1 - pass); at += 2;
      if pass == 0 { out[at] = 0; at += 1; }
      for i: 0..<n {
        c := gossip_find_channel(g, ids[i]);
        for d: 0..<2 {
          p := g[0].channels[c].policy[d];
          v: u64 = ifx !p.present then 0 else ifx pass == 0 then p.timestamp else cast(u64) p.checksum;
          gsp_put32(out, at, v); at += 4;
        }
      }
    }
  }
  return at;
}
```

## The example

The example,
[bitcoin/examples/lightning-gossip.coax](../examples/lightning-gossip.coax),
reads BOLT 7's vectors, then builds the graph of three nodes, Alice, Bob
and Carol, with two channels, and changes each kind of message to show
what refuses. BOLT 7 has no vectors for `channel_announcement` or
`channel_update`, so the example makes them with keys of its own, on
regtest, and the cross-check below computes every line again in Python.

<!-- source: bitcoin/examples/lightning-gossip.coax example -->
```jai
#load "../../coax/lib/lightning/gossip.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [600] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
CODES :: [20] string.["OK", "TRUNCATED", "WRONG_TYPE", "BAD_POINT", "KEY_ORDER", "NOT_MINIMAL", "BAD_SIGNATURE",
                      "BAD_ALIAS", "BAD_ADDRESS", "BAD_ENCODING", "BAD_LENGTH", "ORDER", "UNKNOWN_EVEN", "BAD_COUNT",
                      "UNKNOWN_CHAIN", "UNKNOWN_CHANNEL", "UNKNOWN_NODE", "STALE", "BAD_TEXT", "CAPACITY"];
// A secret of 32 bytes made from two small numbers, for the example.
secret :: (k: int, c: int) -> [32] u8 {
  out: [32] u8;
  for i: 0..<32 { out[i] = cast(u8) ((k * i + c) & 255); }
  return out;
}
scid_show :: (v: u64) { t: [23] u8; print("%", cast(string) t[0..<gossip_scid_text(v, t[..])]); }
// The curve's order n, big-endian, to turn s into n - s.
ORDER :: [32] u8.[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
                  0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41];
negate_s :: (s: [] u8) {
  borrow := 0;
  for k: 0..<32 {
    i := 31 - k;
    d := cast(int) ORDER[i] - cast(int) s[i] - borrow;
    borrow = ifx d < 0 then 1 else 0;
    s[i] = cast(u8) ((d + 256) & 255);
  }
}
// Each channel's ends, as the graph holds them.
policies :: (g: [] GspGraph) {
  for c: 0..<g[0].count {
    for d: 0..<2 {
      p := g[0].channels[c].policy[d];
      if !p.present { continue; }
      print("  "); scid_show(g[0].channels[c].scid);
      print(" direction %: % msat + % ppm, cltv delta %, htlcs % to % msat%\n", d, p.fee_base, p.fee_ppm, p.cltv,
            p.htlc_min, p.htlc_max, ifx (p.channel_flags & 2) != 0 then ", disabled" else "");
    }
  }
}

main :: () {
  print("BOLT 7's vectors\n");
  m: [512] u8;
  verified := 0;
  print("  node announcements:");
  for row: ALIAS_ROWS {
    b := ln_unhex(row.announcement, m[..]);
    n: [1] GspNode;
    code := gossip_node_read_vartime(b, n[..]);
    print(" %", CODES[code]);
    digest: [32] u8; gossip_hash(b[66..<b.count], digest[..]);
    if gossip_verify_vartime(digest[..], b[n[0].node..<n[0].node + 33], b[2..<66]) { verified += 1; }
  }
  print("\n  % of 6 signatures verify under ", verified); print("%\n", ALIAS_NODE_ID);
  print("  extended queries:");
  for row: QUERY_ROWS { print(" %", CODES[gossip_read_vartime(ln_unhex(row.hex, m[..]))]); }
  print("\n");

  print("short channel IDs\n");
  v := gossip_scid_parse_vartime("539268x845x1").value;
  print("  539268x845x1 is block %, transaction %, output %: ", gossip_scid_height(v), gossip_scid_tx(v), gossip_scid_output(v));
  e: [8] u8; gsp_put64(e[..], 0, v); show(e[..]); print("\n");

  chain := QUERY_CHAIN_BYTES;
  alice := secret(3, 1); bob := secret(5, 2); carol := secret(7, 3); dave := secret(9, 4);
  ids := [4] [33] u8.[gsp_public(alice[..]), gsp_public(bob[..]), gsp_public(carol[..]), gsp_public(dave[..])];
  names := [4] string.["Alice", "Bob", "Carol", "Dave"];
  for k: 0..<3 { print("  % ", names[k]); show(ids[k][..]); print("\n"); }

  print("two channels\n");
  g: [1] GspGraph; gossip_graph_begin(g[..], chain[..]);
  ab: [512] u8; bc: [512] u8;
  fa := secret(11, 4); fb := secret(13, 5); fb2 := secret(17, 6); fc := secret(19, 7);
  s1 := gossip_scid(105, 1, 0); s2 := gossip_scid(106, 2, 1);
  n1 := gossip_announcement_write_vartime(chain[..], s1, alice[..], bob[..], fa[..], fb[..], chain[0..<0], ab[..]);
  n2 := gossip_announcement_write_vartime(chain[..], s2, bob[..], carol[..], fb2[..], fc[..], chain[0..<0], bc[..]);
  pairs := [2] string.["Alice and Bob", "Bob and Carol"];
  for c: 0..<2 {
    msg := ifx c == 0 then ab[0..<n1] else bc[0..<n2];
    a := gossip_announcement_read_vartime(msg);
    print("  % ", pairs[c]); scid_show(a.scid); print(": % bytes, %\n", msg.count, CODES[a.code]);
    first := 0; for k: 0..<4 { if cast(string) msg[a.node1..<a.node1 + 33] == cast(string) ids[k][..] { first = k; } }
    print("    node_id_1 is %, the lesser key; signed hash ", names[first]); show(a.hash[..]); print("\n");
    print("    added to the graph: %\n", CODES[gossip_graph_add_vartime(g[..], msg)]);
  }

  print("channel updates\n");
  u: [GSP_UPDATE_SIZE] u8;
  // Alice is node_id_1 of 105x1x0 (direction 0) when her key is the lesser.
  a1 := gossip_announcement_read_vartime(ab[0..<n1]);
  alice_one := cast(string) ab[a1.node1..<a1.node1 + 33] == cast(string) ids[0][..];
  a2 := gossip_announcement_read_vartime(bc[0..<n2]);
  bob_one := cast(string) bc[a2.node1..<a2.node1 + 33] == cast(string) ids[1][..];
  da := ifx alice_one then 0 else 1; db1 := 1 - da;
  db2 := ifx bob_one then 0 else 1; dc := 1 - db2;
  updates := [4] int.[0, 1, 1, 2];   // who signs: Alice, Bob, Bob, Carol
  secrets := [4] [32] u8.[alice, bob, bob, carol];
  scids := [4] u64.[s1, s1, s2, s2];
  directions := [4] int.[da, db1, db2, dc];
  fees := [4] u64.[1000, 0, 500, 2000];
  for k: 0..<4 {
    n := gossip_update_write_vartime(secrets[k][..], chain[..], scids[k], 1762156800 + cast(u64) k, directions[k],
                                     40 + 10 * k, 1000, fees[k], 1 + cast(u64) k, 990000000, u[..]);
    r := gossip_update_read_vartime(u[0..<n]);
    print("  % for ", names[updates[k]]); scid_show(scids[k]);
    print(", direction %: %, checksum %\n", r.direction, CODES[gossip_graph_add_vartime(g[..], u[0..<n])], r.checksum);
  }
  policies(g[..]);

  print("node announcements\n");
  addrs := [7] u8.[1, 127, 0, 0, 1, 0x26, 0x07];    // ipv4 127.0.0.1, port 9735
  na: [256] u8;
  for k: 0..<4 {
    key := ifx k == 0 then alice else ifx k == 1 then bob else ifx k == 2 then carol else dave;
    rgb := [3] u8.[cast(u8) (60 * k), 0x80, 0xff];
    n := gossip_node_write_vartime(key[..], 1762156800, rgb[..], names[k], addrs[..], chain[0..<0], na[..]);
    nn: [1] GspNode;
    code := gossip_node_read_vartime(na[0..<n], nn[..]);
    print("  %: % bytes, %, % address, added: %\n", names[k], n, CODES[code], nn[0].address_count,
          CODES[gossip_graph_add_vartime(g[..], na[0..<n])]);
  }

  print("changing them\n");
  ab[296] ^= 1;
  print("  one byte of a channel announcement changed: %\n", CODES[gossip_announcement_read_vartime(ab[0..<n1]).code]);
  ab[296] ^= 1;
  swapped: [512] u8; for i: 0..<n1 { swapped[i] = ab[i]; }
  for i: 0..<33 { swapped[a1.node1 + i] = ab[a1.node2 + i]; swapped[a1.node2 + i] = ab[a1.node1 + i]; }
  print("  its node IDs swapped: %\n", CODES[gossip_announcement_read_vartime(swapped[0..<n1]).code]);
  mainnet: [1] GspGraph; other := secret(0, 0x6f); gossip_graph_begin(mainnet[..], other[..]);
  print("  added to a graph that follows another chain: %\n", CODES[gossip_graph_add_vartime(mainnet[..], ab[0..<n1])]);
  n := gossip_update_write_vartime(alice[..], chain[..], s1, 1762160400, db1, 40, 1000, 0, 1, 990000000, u[..]);
  print("  Alice signs Bob's direction: %\n", CODES[gossip_graph_add_vartime(g[..], u[0..<n])]);
  n = gossip_update_write_vartime(alice[..], chain[..], s1, 1762156800, da, 40, 1000, 1000, 1, 990000000, u[..]);
  print("  Alice's first update again: %\n", CODES[gossip_graph_add_vartime(g[..], u[0..<n])]);
  negate_s(u[34..<66]);
  r := gossip_update_read_vartime(u[0..<n]);
  print("  with n - s for s: verifies %, added: %\n", gossip_update_verify_vartime(u[0..<n], r, ids[0][..]),
        CODES[gossip_graph_add_vartime(g[..], u[0..<n])]);
  n = gossip_update_write_vartime(alice[..], chain[..], s1, 1762160400, da + 2, 40, 1000, 1000, 1, 990000000, u[..]);
  print("  Alice disables 105x1x0 an hour later: %\n", CODES[gossip_graph_add_vartime(g[..], u[0..<n])]);
  n = gossip_update_write_vartime(carol[..], chain[..], gossip_scid(107, 1, 0), 1762160400, 0, 40, 1000, 1000, 1, 990000000, u[..]);
  print("  an update for 107x1x0: %\n", CODES[gossip_graph_add_vartime(g[..], u[0..<n])]);
  policies(g[..]);

  print("catching up\n");
  f: [64] u8;
  n = gossip_filter_write_vartime(chain[..], 1762156801, 2, f[..]);
  filter := gossip_filter_read_vartime(f[0..<n]);
  print("  gossip_timestamp_filter from % for %: updates at", filter.first, filter.range);
  for k: 0..<4 { t := 1762156800 + cast(u64) k; if gossip_filter_passes(filter, t) { print(" %", t); } }
  print(" pass\n");
  q: [64] u8;
  n = gossip_query_range_write_vartime(chain[..], 100, 10, 3, q[..]);
  qr := gossip_query_range_read_vartime(q[0..<n]);
  print("  query_channel_range blocks % to %, options %: % bytes\n", qr.first, qr.first + qr.number - 1, qr.options, n);
  reply: [256] u8;
  n = gossip_reply_range_vartime(g[..], qr.first, qr.number, qr.options, reply[..]);
  rr := gossip_reply_range_read_vartime(reply[0..<n]);
  print("  reply_channel_range: % bytes, %, sync_complete %\n", n, CODES[rr.code], rr.complete);
  for i: 0..<rr.scids.count {
    print("    "); scid_show(gossip_scid_at(reply[0..<n], rr.scids, i));
    print(" timestamps % %, checksums % %\n", gsp_u32(reply[..], rr.timestamps + 8 * i), gsp_u32(reply[..], rr.timestamps + 8 * i + 4),
          gsp_u32(reply[..], rr.checksums + 8 * i), gsp_u32(reply[..], rr.checksums + 8 * i + 4));
  }
  want := [2] u64.[s1, s2]; flags := [2] u8.[1, 6];
  n = gossip_query_scids_write_vartime(chain[..], want[..], flags[..], q[..]);
  qs := gossip_query_scids_read_vartime(q[0..<n]);
  print("  query_short_channel_ids: % bytes, %, flags % %\n", n, CODES[qs.code],
        gossip_query_flag(q[0..<n], qs, 0), gossip_query_flag(q[0..<n], qs, 1));
  q[36] = 1;
  print("  the same with encoding 1, zlib: %\n", CODES[gossip_read_vartime(q[0..<n])]);
}
```

The output:

```text
BOLT 7's vectors
  node announcements: BAD_ALIAS BAD_ALIAS BAD_ALIAS BAD_ALIAS BAD_ALIAS BAD_ALIAS
  6 of 6 signatures verify under 034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa
  extended queries: OK OK OK BAD_ENCODING OK BAD_ENCODING OK BAD_ENCODING BAD_ENCODING BAD_ENCODING
short channel IDs
  539268x845x1 is block 539268, transaction 845, output 1: 083a8400034d0001
  Alice 021379cc96aa0aa0f16d54bacc78dca79c4ef9cbb1d898f0f1a2eea3521c7ec38a
  Bob 03b5cd2183c7ae0aff0a9526a1faa97ab6b441bbb2b57e2442a9827b6b0d8e4dda
  Carol 03f324133f92f203537cd7f9c4f1cf1078020159111a18457809ce3c11950d87eb
two channels
  Alice and Bob 105x1x0: 432 bytes, OK
    node_id_1 is Alice, the lesser key; signed hash aa454a79cd58a6dbac0e42860e34b1e2da83c1b12e949b597b5e02621ddf75e9
    added to the graph: OK
  Bob and Carol 106x2x1: 432 bytes, OK
    node_id_1 is Bob, the lesser key; signed hash 8f2735f2bd174f6a4eecf2fa05a454aa12696ea81e28f19fb9f44ffdd806e609
    added to the graph: OK
channel updates
  Alice for 105x1x0, direction 0: OK, checksum 274220191
  Bob for 105x1x0, direction 1: OK, checksum 4206943366
  Bob for 106x2x1, direction 0: OK, checksum 3146641915
  Carol for 106x2x1, direction 1: OK, checksum 2723939177
  105x1x0 direction 0: 1000 msat + 1 ppm, cltv delta 40, htlcs 1000 to 990000000 msat
  105x1x0 direction 1: 0 msat + 2 ppm, cltv delta 50, htlcs 1000 to 990000000 msat
  106x2x1 direction 0: 500 msat + 3 ppm, cltv delta 60, htlcs 1000 to 990000000 msat
  106x2x1 direction 1: 2000 msat + 4 ppm, cltv delta 70, htlcs 1000 to 990000000 msat
node announcements
  Alice: 149 bytes, OK, 1 address, added: OK
  Bob: 149 bytes, OK, 1 address, added: OK
  Carol: 149 bytes, OK, 1 address, added: OK
  Dave: 149 bytes, OK, 1 address, added: UNKNOWN_NODE
changing them
  one byte of a channel announcement changed: BAD_SIGNATURE
  its node IDs swapped: KEY_ORDER
  added to a graph that follows another chain: UNKNOWN_CHAIN
  Alice signs Bob's direction: BAD_SIGNATURE
  Alice's first update again: STALE
  with n - s for s: verifies true, added: STALE
  Alice disables 105x1x0 an hour later: OK
  an update for 107x1x0: UNKNOWN_CHANNEL
  105x1x0 direction 0: 1000 msat + 1 ppm, cltv delta 40, htlcs 1000 to 990000000 msat, disabled
  105x1x0 direction 1: 0 msat + 2 ppm, cltv delta 50, htlcs 1000 to 990000000 msat
  106x2x1 direction 0: 500 msat + 3 ppm, cltv delta 60, htlcs 1000 to 990000000 msat
  106x2x1 direction 1: 2000 msat + 4 ppm, cltv delta 70, htlcs 1000 to 990000000 msat
catching up
  gossip_timestamp_filter from 1762156801 for 2: updates at 1762156801 1762156802 pass
  query_channel_range blocks 100 to 109, options 3: 45 bytes
  reply_channel_range: 99 bytes, OK, sync_complete 1
    105x1x0 timestamps 1762160400 1762156801, checksums 2888131039 4206943366
    106x2x1 timestamps 1762156802 1762156803, checksums 3146641915 2723939177
  query_short_channel_ids: 58 bytes, OK, flags 1 6
  the same with encoding 1, zlib: BAD_ENCODING
```

- BOLT 7's six node announcements are signed with the key 0x11...11, and
  each signature verifies; each alias holds a character BOLT 1 forbids
  (NUL, DEL, a zero-width space, a private-use character, and the line
  and paragraph separators), so each is refused.
- `extended-queries.json` was written when encoding 1 was zlib. The five
  messages that use it now refuse; the other five read with the fields
  the file states.
- Alice's key is less than Bob's, and Bob's less than Carol's, so Alice
  is `node_id_1` of the first channel and Bob of the second.
- The four updates come from the right ends, and the graph holds each
  end's policy. Each checksum is CRC32C of its update.
- Dave has no channel, so his node announcement is refused.
- A changed byte breaks the signatures, swapped node IDs break the
  order, and a graph on another chain does not take the channel. Alice
  cannot sign Bob's end. Her first update sent again is stale, and still
  stale with s replaced by n - s, though that signature verifies. A later
  update that disables the channel replaces the first.
- The reply to a range query lists both channels with their timestamps
  and checksums: the first end of 105x1x0 now has the later timestamp and
  a new checksum, since the disable bit changed its policy.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-gossip.out](../fixtures/lightning-gossip.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-gossip.coax](../examples/checks/lightning-gossip.coax),
which loads the example:

- a changed byte in each 8-byte stretch of a signed update refuses
- every update, filter and range query cut short refuses
- short channel IDs survive their text at the edges of each field
- an update's checksum is CRC32C of its bytes without the signature and
  timestamp, and the timestamp does not change it
- the timestamp filter passes exactly `first <= t < first + range`, also
  where the sum passes 2^32
- a range reply holds exactly the channels in its blocks
- query flags come back one per ID, and any other count refuses

<!-- source: bitcoin/examples/checks/lightning-gossip.coax check -->
```jai
// One signed update, made once for the checks below.
update :: (out: [] u8) -> int {
  key := secret(3, 1); chain := QUERY_CHAIN_BYTES;
  return gossip_update_write_vartime(key[..], chain[..], gossip_scid(105, 1, 0), 1762156800, 0, 40, 1000, 1000, 1, 990000000, out);
}
// A changed byte anywhere in what the signature covers, or in the
// signature, refuses: one byte of each 8-byte stretch of the update.
#forall "one changed byte of an update refuses" k: 0..<17 #steps 4000000000 {
  u: [GSP_UPDATE_SIZE] u8; n := update(u[..]);
  key := secret(3, 1); id := gsp_public(key[..]);
  at := 2 + 8 * k + (k % 8);
  u[at] ^= 0x40;
  r := gossip_update_read_vartime(u[0..<n]);
  #assert r.code != GSP_OK || !gossip_update_verify_vartime(u[0..<n], r, id[..]);
}
// Every message cut short refuses.
#forall "a cut update refuses" cut: 0..<138 #steps 2000000000 {
  u: [GSP_UPDATE_SIZE] u8; n := update(u[..]);
  #assert gossip_read_vartime(u[0..<cut]) == GSP_TRUNCATED;
}
#forall "a cut filter, end or range query refuses" cut: 0..<42 {
  chain := QUERY_CHAIN_BYTES; m: [64] u8;
  n := gossip_filter_write_vartime(chain[..], 5, 6, m[..]);
  #assert gossip_read_vartime(m[0..<cut]) == GSP_TRUNCATED;
  n = gossip_query_range_write_vartime(chain[..], 5, 6, -1, m[..]);
  #assert gossip_read_vartime(m[0..<cut]) == GSP_TRUNCATED && gossip_read_vartime(m[0..<n]) == GSP_OK;
}
// Short channel IDs survive their text, at the edges of each field.
#forall "short channel ID text round-trips" k: 0..<27 {
  h := ifx k % 3 == 0 then 0 else ifx k % 3 == 1 then 1 else 16777215;
  t := ifx (k / 3) % 3 == 0 then 0 else ifx (k / 3) % 3 == 1 then 845 else 16777215;
  o := ifx k / 9 == 0 then 0 else ifx k / 9 == 1 then 1 else 65535;
  v := gossip_scid(cast(u64) h, cast(u64) t, cast(u64) o);
  text: [23] u8; n := gossip_scid_text(v, text[..]);
  back := gossip_scid_parse_vartime(cast(string) text[0..<n]);
  #assert back.code == GSP_OK && back.value == v && gossip_scid_height(v) == cast(u64) h;
}
// An update's checksum is CRC32C of the update without its signature and
// timestamp, whatever its fees.
#forall "the checksum leaves out the signature and timestamp" fee: 0..<8 #steps 2000000000 {
  key := secret(3, 1); chain := QUERY_CHAIN_BYTES;
  u: [GSP_UPDATE_SIZE] u8; v: [GSP_UPDATE_SIZE] u8;
  n := gossip_update_write_vartime(key[..], chain[..], 7, 1000, 0, 40, 1, cast(u64) fee, 1, 2, u[..]);
  m := gossip_update_write_vartime(key[..], chain[..], 7, 2000 + cast(u64) fee, 0, 40, 1, cast(u64) fee, 1, 2, v[..]);
  joined: [68] u8;
  for i: 0..<40 { joined[i] = u[66 + i]; }
  for i: 0..<28 { joined[40 + i] = u[110 + i]; }
  a := gossip_update_read_vartime(u[0..<n]); b := gossip_update_read_vartime(v[0..<m]);
  #assert a.checksum == gossip_crc32c(joined[..]) && a.checksum == b.checksum;
}
// The filter passes exactly first <= t < first + range, even where the
// sum passes 2^32.
#forall "the timestamp filter's bounds" k: 0..<64 {
  first: u64 = ifx k < 32 then 100 else 0xfffffff0;
  f := GspFilter.{first = first, range = cast(u64) (k % 8)};
  for d: 0..<12 {
    t := first - 2 + cast(u64) d;
    #assert gossip_filter_passes(f, t) == (t >= first && t - first < cast(u64) (k % 8));
  }
}
// A reply holds exactly the graph's channels in the range, ascending.
#forall "a range reply holds the channels in its blocks" first: 0..<12 {
  chain := QUERY_CHAIN_BYTES;
  g: [1] GspGraph; gossip_graph_begin(g[..], chain[..]);
  heights := [6] u64.[7, 3, 9, 3, 1, 5];
  for i: 0..<6 { g[0].channels[i].scid = gossip_scid(heights[i], cast(u64) i, 0); }
  g[0].count = 6;
  out: [256] u8;
  n := gossip_reply_range_vartime(g[..], cast(u64) first, 4, 3, out[..]);
  r := gossip_reply_range_read_vartime(out[0..<n]);
  want := 0;
  for i: 0..<6 { if heights[i] >= cast(u64) first && heights[i] < cast(u64) first + 4 { want += 1; } }
  #assert r.code == GSP_OK && r.scids.count == want;
  for i: 0..<r.scids.count {
    h := gossip_scid_height(gossip_scid_at(out[0..<n], r.scids, i));
    #assert h >= cast(u64) first && h < cast(u64) first + 4;
  }
}
// Query flags come back one per ID, and any other count refuses.
#forall "query flags come back" n: 1..<8 {
  chain := QUERY_CHAIN_BYTES;
  ids: [8] u64; flags: [8] u8;
  for i: 0..<n { ids[i] = gossip_scid(100 + cast(u64) i, 0, 0); flags[i] = cast(u8) ((i * 5) % 32); }
  out: [128] u8;
  m := gossip_query_scids_write_vartime(chain[..], ids[0..<n], flags[0..<n], out[..]);
  q := gossip_query_scids_read_vartime(out[0..<m]);
  #assert q.code == GSP_OK && q.scids.count == n;
  for i: 0..<n { #assert gossip_query_flag(out[0..<m], q, i) == cast(u64) flags[i] && gossip_scid_at(out[0..<m], q.scids, i) == ids[i]; }
  m = gossip_query_scids_write_vartime(chain[..], ids[0..<n], flags[0..<n - 1], out[..]);
  #assert n == 1 || gossip_read_vartime(out[0..<m]) == GSP_BAD_COUNT;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-gossip.coax --run-steps 250000000
```

`coax test` runs 229 checks, including the loaded files' own checks, in
about a minute on 2026-11-03.

### The library's own checks

`gossip.coax` is an `#authority` file. Its checks cover short channel IDs
and their refusals, RFC 3720's CRC32C examples, BOLT 1's text rule, BOLT
7's six node announcements, every message of `extended-queries.json`
with its fields, the query refusals (encodings, lengths, order, counts
and TLVs), an announced channel with its updates and nodes in a graph,
address descriptors, the timestamp filter and range replies.

```shell
coax/bin/coax test coax/lib/lightning/gossip.coax --run-steps 250000000
```

`coax test` runs 218 checks, including those of the files it loads, on
2026-11-03. Mutation testing: not run (owner speed order).

### BOLT 7 and an independent reader

[gossip.py](../../coax/tests/lightning/gossip.py) computes every line
the example prints with [bolt7.py](../../coax/tests/lightning/bolt7.py),
its own BOLT 7 reader and writer, CRC32C and channel graph, written from
BOLT 7 and BOLT 1 on
[ln_crypto.py](../../coax/tests/lightning/ln_crypto.py)'s secp256k1 and
[bolt11.py](../../coax/tests/lightning/bolt11.py)'s ECDSA, sharing no
code with Coax. It also checks the library's table of forbidden code
points against Python's own Unicode database.

```shell
python3 -I coax/tests/lightning/gossip.py --coax coax/bin/coax
```

On 2026-11-03 every vector passes, the table matches Unicode 17.0.0, all
51 lines agree, and the example's output in native debug and release
builds equals them. Wasm was not run.

[make_gossip_vectors.py](../../coax/tests/lightning/make_gossip_vectors.py)
reads the pinned BOLT 7 vectors into
[gossip_vectors.coax](../../coax/tests/lightning/gossip_vectors.coax).

## What this chapter leaves out

- Checking that the funding output exists, is unspent, pays to the two
  funding keys and has six confirmations: that needs the UTXO set.
- `announcement_signatures`, the message two peers exchange to build the
  announcement: chapter 109.
- Rebroadcasting, rate limits, pruning updates older than two weeks, and
  blacklisting nodes that sign conflicting announcements.
- Splices, which announce a new short channel ID for the same channel.
- Finding a route over the graph: chapter 113.

## Summary

- A short channel ID is a funding output's block, transaction and output
  in 8 bytes.
- Every gossip signature signs the double SHA-256 of the message after
  its signatures, so fields added later are signed too.
- A channel announcement carries four signatures over one hash: two node
  IDs, lesser first, and two funding keys.
- A node announcement's alias must be BOLT 1's restricted UTF-8; its
  addresses end at the first unknown type.
- A channel update names its end with a direction bit, is verified with
  that end's node ID from the announcement, and replaces an older one.
  Its checksum is CRC32C without the signature and timestamp.
- Queries let a node catch up, in the plain encoding only; zlib is gone.
- A graph keeps only what checks, and only nodes with channels.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [19. Verify ECDSA](ch019-verify-ecdsa.md)
- [20. Sign with ECDSA](ch020-sign-with-ecdsa.md)
- [104. Messages and TLVs](ch104-messages-and-tlvs.md)

## Sources

- [BOLT 7](https://github.com/lightning/bolts/blob/master/07-routing-gossip.md),
  "Definition of `short_channel_id`", "The `channel_announcement`
  Message", "The `node_announcement` Message" with its test vectors, "The
  `channel_update` Message" and "Query Messages", and
  `bolt07/extended-queries.json`, at lightning/bolts revision `19146282`,
  pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/07-routing-gossip.md).
- [BOLT 1](https://github.com/lightning/bolts/blob/master/01-messaging.md)'s
  "Unicode characters validation", at the same revision, pinned since
  chapter 104.
- [RFC 3720](https://www.rfc-editor.org/rfc/rfc3720), appendix B.4, for
  CRC32C and its examples.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
