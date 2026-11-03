# 115. Offers

A BOLT 11 invoice (chapter 114) is good for one payment: it carries one
payment hash, and a second payment to it can be stolen by anyone who saw
the first preimage. A shop that prints a QR code needs something it can
show to everyone, for as long as it likes. BOLT 12's answer is the offer.
An offer says what is for sale and how to reach the seller. A payer who
wants to pay sends an invoice request over the Lightning network, and the
seller answers with a fresh invoice for that payer alone.

Offers, invoice requests and invoices are all TLV streams (chapter 104).
A request copies the offer's fields and adds its own, and an invoice
copies the request's and adds its own, so each message carries the whole
history. Requests and invoices are signed with BIP-340 Schnorr signatures
(chapters 23 and 24), but not over the bytes: over the root of a Merkle
tree with one leaf per field. That lets a payer later prove what it paid
for while hiding the rest of the invoice, in a payer proof. This chapter
builds the reader, the writer and the checks for all four.

## What this chapter teaches

- The text: a prefix (`lno`, `lnr`, `lni` or `lnp`), `1`, and the
  stream's bits in Bech32's characters with no checksum. A writer may
  split long text with `+` and whitespace; a reader removes them.
- The TLV ranges: an offer uses types 1 to 79, a request adds 0 and 80 to
  159, an invoice adds 160 to 239, each with an experimental range from
  one billion. Types 240 to 1000 hold signatures.
- The Merkle tree: a leaf `H("LnLeaf", record)` for each field, paired
  with a nonce leaf `H("LnNonce" || first record, type)`, joined by
  `H("LnBranch", lesser || greater)` into a tree whose lowest types sit
  deepest. The signature signs `H("lightning" || stream || field, root)`.
- `offer_issuer_id` versus blinded paths: an offer can name its node, or
  hide it behind a blinded path (chapter 111). Then the invoice is signed
  with the key of the path's last blinded hop.
- The flow: offer, invoice request, invoice, and the rules each reader
  applies, from amounts and quantities to the invoice repeating the
  request it answers.
- Payer proofs: the fields a payer keeps, marker numbers for the fields
  it hides, and the hashes that let anyone rebuild the invoice's root and
  check the node's signature.

## Where Lightning uses offers

| Where | What it uses | Chapter |
| --- | --- | --- |
| Asking for an invoice | the request travels in an onion message to `offer_paths` or `offer_issuer_id` | 111 |
| Paying | `invoice_paths` and `invoice_blindedpay` are the blinded routes and their fees | 111, 113 |
| Proof of payment | the preimage of `invoice_payment_hash`, now tied to the payer by a payer proof | 109 |
| One-time payments | BOLT 11 invoices stay the simpler choice for a single payment | 114 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [offers.coax](../../coax/lib/lightning/offers.coax) | `offer_decode_vartime(text, b)` | reads and checks any of the four kinds |
| | `offer_text_decode_vartime`, `offer_text_encode_vartime` | the text without a checksum, with `+` joins |
| | `ofr_stream_vartime(b)`, `ofr_record_vartime(kind, type, value)` | the stream's framing, ranges, odd rule and formats |
| | `ofr_paths_vartime(v, which)`, `ofr_payinfo_vartime(v)` | blinded paths and their payinfo |
| | `ofr_merkle_vartime(b, out)`, `ofr_sig_hash`, `ofr_verify_vartime` | the Merkle root and the signature over it |
| | `ofr_check_offer_vartime`, `ofr_check_request_vartime`, `ofr_check_invoice_vartime`, `ofr_check_proof_vartime` | the rules of each kind |
| | `offer_begin`, `offer_put_vartime`, `offer_sign_vartime`, `offer_write_vartime` | the writer |
| | `offer_request_vartime`, `offer_invoice_vartime`, `offer_proof_vartime` | a request for an offer, an invoice for a request, a payer proof |
| | `offer_matches_vartime`, `offer_invoice_expired_vartime` | the payer's checks of an invoice |
| [bolt1.coax](../../coax/lib/lightning/bolt1.coax) | `tlv_next_vartime`, `bigsize_*`, `tu_*`, `ln_feature_refuse_vartime` | TLVs, BigSize, truncated numbers and the odd rule (chapter 104) |
| [schnorr_sign.coax](../../coax/lib/crypto/schnorr_sign.coax) | `schnorr_sign_step`, `schnorr_fast_verify_vartime` | BIP-340 signing and verifying (chapters 23 and 24) |
| [onion.coax](../../coax/lib/lightning/onion.coax) | `onion_blind_hop`, `onion_blinded_secret` | a blinded path, and a hop's key on it (chapter 111) |

An offer is meant to be public, and a request or invoice is shown to the
other side, so the readers branch on their bytes and end in `_vartime`.
The writer's secrets are the payer's and the node's keys, which go
straight to chapter 24's constant-time signer.

The library's codes and sizes:

<!-- source: coax/lib/lightning/offers.coax codes -->
```jai
OFR_OK :: 0;
OFR_BAD_TEXT :: 1;       // mixed case, a stray '+' or character, no '1', or more than 4 bits of padding
OFR_BAD_PREFIX :: 2;     // not lno, lnr, lni or lnp
OFR_TRUNCATED :: 3;      // the bytes end inside a type, a length or a value
OFR_NOT_MINIMAL :: 4;    // a BigSize, number or feature bitmap longer than it needs
OFR_ORDER :: 5;          // TLV types not strictly increasing
OFR_RANGE :: 6;          // a type outside the ranges this kind of stream allows
OFR_UNKNOWN_EVEN :: 7;   // an even type the reader does not know
OFR_BAD_LENGTH :: 8;     // a known field of a length its type does not allow
OFR_BAD_UTF8 :: 9;       // a text field that is not UTF-8
OFR_BAD_POINT :: 10;     // a key that is not a point on the curve
OFR_BAD_PATH :: 11;      // a blinded path that does not parse, or no path a payer can use
OFR_BAD_FEATURES :: 12;  // an unknown even feature bit
OFR_BAD_VALUE :: 13;     // a BIP-353 name with a character it may not have
OFR_REQUIRED :: 14;      // a field the rules require is missing
OFR_BAD_AMOUNT :: 15;    // an amount of zero, below the offer's, or not the request's
OFR_BAD_QUANTITY :: 16;  // a quantity the offer does not allow
OFR_FORBIDDEN :: 17;     // a field this stream must not have
OFR_BAD_SIGNATURE :: 18; // a signature that does not verify
OFR_WRONG_NODE :: 19;    // an invoice signed by a node other than the offer's
OFR_BAD_PREIMAGE :: 20;  // a payer proof whose preimage does not hash to the payment hash
OFR_BAD_OMITTED :: 21;   // proof_omitted_tlvs out of order or with a number it may not have
OFR_BAD_LEAVES :: 22;    // not one proof_leaf_hashes entry per field shown
OFR_BAD_MISSING :: 23;   // too few or too many proof_missing_hashes to rebuild the root
OFR_CAPACITY :: 24;      // more bytes, records or hashes than this reader keeps

OFR_OFFER :: 1; OFR_REQUEST :: 2; OFR_INVOICE :: 3; OFR_PROOF :: 4;
OFR_PREFIXES :: [4] string.["lno", "lnr", "lni", "lnp"];
OFR_NAMES :: [4] string.["offer", "invoice_request", "invoice", "payer_proof"];

OFR_MAX_BYTES :: 4096;
OFR_MAX_TEXT :: 8192;
OFR_MAX_RECORDS :: 64;
```

A reader stops at the first rule a message breaks and returns its code,
so a test can say why each invalid vector fails.

## The fields

Each kind of stream owns a range of types. A request copies every offer
field, so the ranges cannot overlap: an invoice can hold an offer's,
a request's and its own fields side by side, and a reader can tell
which is which by its number.

<!-- source: coax/lib/lightning/offers.coax types -->
```jai
// The fields. An offer has types 1 to 79; a request copies them and adds
// 0 and 80 to 159; an invoice copies those and adds 160 to 239. Each
// range also has an experimental billion: 1,000,000,000 up. Types 240 to
// 1000 are signatures, outside the Merkle tree.
OFR_CHAINS :: 2; OFR_METADATA :: 4; OFR_CURRENCY :: 6; OFR_AMOUNT :: 8;
OFR_DESCRIPTION :: 10; OFR_FEATURES :: 12; OFR_ABSOLUTE_EXPIRY :: 14;
OFR_PATHS :: 16; OFR_ISSUER :: 18; OFR_QUANTITY_MAX :: 20; OFR_ISSUER_ID :: 22;
REQ_METADATA :: 0; REQ_CHAIN :: 80; REQ_AMOUNT :: 82; REQ_FEATURES :: 84;
REQ_QUANTITY :: 86; REQ_PAYER_ID :: 88; REQ_PAYER_NOTE :: 89; REQ_PATHS :: 90;
REQ_BIP_353_NAME :: 91;
INV_PATHS :: 160; INV_BLINDEDPAY :: 162; INV_CREATED_AT :: 164;
INV_RELATIVE_EXPIRY :: 166; INV_PAYMENT_HASH :: 168; INV_AMOUNT :: 170;
INV_FALLBACKS :: 172; INV_FEATURES :: 174; INV_NODE_ID :: 176;
OFR_SIGNATURE :: 240;
PROOF_SIGNATURE :: 241; PROOF_PREIMAGE :: 1001; PROOF_OMITTED :: 1002;
PROOF_MISSING :: 1003; PROOF_LEAVES :: 1004; PROOF_NOTE :: 1005;

OFR_BILLION :: cast(u64) 1000000000;
OFR_RELATIVE_EXPIRY :: 7200;   // an invoice's default: two hours after invoice_created_at
```

## A stream as read

A read stream keeps its bytes and, for each record, its type and where
its bytes are. The rules ask "is this field there?" and "what number does
it hold?", and the Merkle tree needs each record whole, type and length
included.

<!-- source: coax/lib/lightning/offers.coax bolt12 -->
```jai
// A stream as read: the bytes, and each record's type and where its
// bytes are. at is where the record starts; its value is
// data[start..<start+length].
OfrRecord :: struct { type: u64; at: int; start: int; length: int; }
Bolt12 :: struct {
  code: int; kind: int;
  data: [4096] u8; n: int;
  records: [64] OfrRecord; count: int;
}
// The index of the record of this type, or -1.
ofr_find :: (b: [] Bolt12, type: u64) -> int {
  for i: 0..<b[0].count { if b[0].records[i].type == type { return i; } }
  return -1;
}
ofr_has :: (b: [] Bolt12, type: u64) -> bool { return ofr_find(b, type) >= 0; }
// The value of the record of this type; empty when it is missing.
ofr_value :: (b: [] Bolt12, type: u64) -> [] u8 {
  i := ofr_find(b, type);
  if i < 0 { return b[0].data[0..<0]; }
  r := b[0].records[i];
  return b[0].data[r.start..<r.start + r.length];
}
// The whole record, type and length included.
ofr_raw :: (b: [] Bolt12, i: int) -> [] u8 {
  r := b[0].records[i];
  return b[0].data[r.at..<r.start + r.length];
}
// A truncated number (tu64) the stream holds, or zero when it is missing.
ofr_number :: (b: [] Bolt12, type: u64) -> u64 { return tu_read_vartime(ofr_value(b, type), 8).value; }
```

## Text without a checksum

The text uses Bech32's 32 characters (chapter 27), five bits each, but
not its checksum. A QR code already has error correction, and an error
costs nothing: a changed character gives a stream that fails to parse,
or a request or invoice whose signature fails. An offer is not signed,
so a changed character can give a different valid offer; the request a
payer builds from it would then not match the seller's offer, and the
seller would not answer.

Long text may be split with `+`, and whitespace may follow the `+`, so
an offer can be broken over lines or posts. The `+` must sit between two
characters of the text.

<!-- source: coax/lib/lightning/offers.coax text -->
```jai
// The text is the prefix, '1', then the stream's bits five at a time in
// Bech32's characters, with no checksum: a QR code has its own, and a
// changed character gives at worst a stream that fails to parse or to
// verify. A writer may split long text with '+', and whitespace may
// follow a '+'; a reader removes them. All lowercase or all uppercase.
OFR_CHARSET :: "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
OfrText :: struct { code: int; kind: int; length: int; }
ofr_lower_char :: (c: u8) -> u8 { if c >= 'A' && c <= 'Z' { return c + 32; } return c; }
ofr_alnum :: (c: u8) -> bool { return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9'); }
ofr_space :: (c: u8) -> bool { return c == ' ' || c == '\t' || c == '\r' || c == '\n'; }
offer_text_decode_vartime :: (text: string, out: [] u8) -> OfrText {
  lower := false; upper := false;
  for c: text { if c >= 'a' && c <= 'z' { lower = true; } if c >= 'A' && c <= 'Z' { upper = true; } }
  if lower && upper { return OfrText.{code = OFR_BAD_TEXT}; }
  if text.count > OFR_MAX_TEXT { return OfrText.{code = OFR_CAPACITY}; }
  kept: [8192] u8; n := 0; i := 0;
  while i < text.count {
    c := ofr_lower_char(text[i]);
    if c == '+' {
      // A '+' joins two characters: one before it, one after it and any
      // whitespace.
      if n == 0 { return OfrText.{code = OFR_BAD_TEXT}; }
      i += 1;
      while i < text.count && ofr_space(text[i]) { i += 1; }
      if i == text.count || !ofr_alnum(ofr_lower_char(text[i])) { return OfrText.{code = OFR_BAD_TEXT}; }
      continue;
    }
    if !ofr_alnum(c) { return OfrText.{code = OFR_BAD_TEXT}; }
    kept[n] = c; n += 1; i += 1;
  }
  one := -1;
  for k: 0..<n { if kept[k] == '1' { one = k; } }
  if one < 0 { return OfrText.{code = OFR_BAD_TEXT}; }
  r: OfrText;
  for p: 0..<4 { if cast(string) kept[0..<one] == OFR_PREFIXES[p] { r.kind = p + 1; } }
  if r.kind == 0 { return OfrText.{code = OFR_BAD_PREFIX}; }
  acc: u32 = 0; bits := 0;
  for k: one + 1..<n {
    d := -1;
    for j: 0..<32 { if OFR_CHARSET[j] == kept[k] { d = j; } }
    if d < 0 { return OfrText.{code = OFR_BAD_TEXT}; }
    acc = ((acc << 5) | cast(u32) d) & 0xfff; bits += 5;
    if bits >= 8 {
      bits -= 8;
      if r.length == out.count { return OfrText.{code = OFR_CAPACITY}; }
      out[r.length] = cast(u8) ((acc >> cast(u32) bits) & 255); r.length += 1;
    }
  }
  // At most 4 bits of padding, all zero.
  if bits > 4 || (acc & ((cast(u32) 1 << cast(u32) bits) - 1)) != 0 { return OfrText.{code = OFR_BAD_TEXT}; }
  return r;
}
// The text of a stream, lowercase and with no '+'. Returns its length,
// or -1 when out is too small.
offer_text_encode_vartime :: (kind: int, data: [] u8, out: [] u8) -> int {
  if kind < OFR_OFFER || kind > OFR_PROOF { return -1; }
  prefix := OFR_PREFIXES[kind - 1];
  if out.count < 4 + (data.count * 8 + 4) / 5 { return -1; }
  for i: 0..<3 { out[i] = prefix[i]; }
  out[3] = '1'; n := 4;
  acc: u32 = 0; bits := 0;
  for x: data {
    acc = ((acc << 8) | cast(u32) x) & 0xfff; bits += 8;
    while bits >= 5 { bits -= 5; out[n] = OFR_CHARSET[cast(int) ((acc >> cast(u32) bits) & 31)]; n += 1; }
  }
  if bits > 0 { out[n] = OFR_CHARSET[cast(int) ((acc << cast(u32) (5 - bits)) & 31)]; n += 1; }
  return n;
}
```

## Blinded paths and payinfo

`offer_paths`, `invreq_paths` and `invoice_paths` hold BOLT 4's blinded
paths, the ones chapter 111 builds. The first node of a path is a real
node ID (or a short channel ID and a direction); every later hop is a
blinded node ID with its encrypted data. An invoice gives a
`blinded_payinfo` for each path: the fees and CLTV delta of the whole
path, so a payer can route to it without learning the hops.

<!-- source: coax/lib/lightning/offers.coax paths -->
```jai
// BOLT 4's blinded paths, one after another: first_node_id (a 33-byte
// key, or 0 or 1 and an 8-byte short channel ID), first_path_key, then
// num_hops hops of blinded_node_id, a 2-byte length and the encrypted
// data. final returns the last blinded_node_id of path which, the key an
// invoice is signed with when the offer names no issuer.
OfrPaths :: struct { code: int; count: int; final: [33] u8; }
ofr_paths_vartime :: (v: [] u8, which: int) -> OfrPaths {
  r: OfrPaths; at := 0;
  while at < v.count {
    if v[at] == 0 || v[at] == 1 {
      if v.count - at < 9 { return OfrPaths.{code = OFR_BAD_PATH}; }
      at += 9;
    } else {
      if v.count - at < 33 || !ofr_point_vartime(v[at..<at + 33]) { return OfrPaths.{code = OFR_BAD_PATH}; }
      at += 33;
    }
    if v.count - at < 34 || !ofr_point_vartime(v[at..<at + 33]) { return OfrPaths.{code = OFR_BAD_PATH}; }
    hops := cast(int) v[at + 33]; at += 34;
    if hops == 0 { return OfrPaths.{code = OFR_BAD_PATH}; }
    for h: 0..<hops {
      if v.count - at < 33 || !ofr_point_vartime(v[at..<at + 33]) { return OfrPaths.{code = OFR_BAD_PATH}; }
      if v.count - at < 35 { return OfrPaths.{code = OFR_BAD_PATH}; }
      n := ln_u16(v, at + 33);
      if v.count - at - 35 < n { return OfrPaths.{code = OFR_BAD_PATH}; }
      if r.count == which && h == hops - 1 { for i: 0..<33 { r.final[i] = v[at + i]; } }
      at += 35 + n;
    }
    r.count += 1;
  }
  if r.count == 0 { return OfrPaths.{code = OFR_BAD_PATH}; }
  return r;
}
// An invoice's blinded_payinfo, one per path: fee_base_msat (4),
// fee_proportional_millionths (4), cltv_expiry_delta (2),
// htlc_minimum_msat (8), htlc_maximum_msat (8), then a 2-byte length and
// features. A path whose features have an even bit cannot be used: no
// payinfo feature is known yet.
OfrPayinfo :: struct { code: int; count: int; usable: int; fee_base: u64; fee_ppm: u64; cltv: int; }
ofr_payinfo_vartime :: (v: [] u8) -> OfrPayinfo {
  r: OfrPayinfo; at := 0; none := OFR_NO_FEATURES;
  while at < v.count {
    if v.count - at < 28 { return OfrPayinfo.{code = OFR_BAD_LENGTH}; }
    flen := ln_u16(v, at + 26);
    if v.count - at - 28 < flen { return OfrPayinfo.{code = OFR_BAD_LENGTH}; }
    if r.count == 0 {
      r.fee_base = cast(u64) cb_load_be32(v, at).value; r.fee_ppm = cast(u64) cb_load_be32(v, at + 4).value;
      r.cltv = ln_u16(v, at + 8);
    }
    if ln_feature_refuse_vartime(v[at + 28..<at + 28 + flen], none[..]) < 0 { r.usable += 1; }
    r.count += 1; at += 28 + flen;
  }
  return r;
}
```

## One record at a time

A record must be in its stream's ranges. A type the reader does not
know is skipped if odd and refused if even, as in chapter 104. A known
type must hold what BOLT 12 says: whole chain hashes, UTF-8 text,
minimal numbers, minimal feature bitmaps, points on the curve, blinded
paths that parse, and so on. The reader frames the whole stream first,
so a stream cut short reports that before any record's format.

<!-- source: coax/lib/lightning/offers.coax record -->
```jai
// The ranges of types each kind of stream allows, and the types it knows.
ofr_in_range :: (kind: int, t: u64) -> bool {
  if kind == OFR_OFFER { return (t >= 1 && t <= 79) || (t >= OFR_BILLION && t <= 1999999999); }
  sig := t >= 240 && t <= 1000;
  if kind == OFR_REQUEST { return t <= 159 || sig || (t >= OFR_BILLION && t <= 2999999999); }
  if kind == OFR_INVOICE { return t <= 1000 || (t >= OFR_BILLION && t <= 3999999999); }
  return (t >= 1 && t <= 1005) || (t >= OFR_BILLION && t <= 3999999999);
}
ofr_known :: (kind: int, t: u64) -> bool {
  offer := t >= 2 && t <= 22 && t % 2 == 0;
  request := t == 0 || t == 80 || t == 82 || t == 84 || t == 86 || t == 88 || t == 89 || t == 90 || t == 91 || t == 240;
  invoice := t >= 160 && t <= 176 && t % 2 == 0;
  if kind == OFR_OFFER { return offer; }
  if kind == OFR_REQUEST { return offer || request; }
  if kind == OFR_INVOICE { return offer || request || invoice; }
  return (offer || request || invoice || t == 241 || (t >= 1001 && t <= 1005)) && t != 0;
}
// One record checked against its type: the range, the odd rule, then the
// value's format.
ofr_record_vartime :: (kind: int, t: u64, v: [] u8) -> int {
  if !ofr_in_range(kind, t) { return OFR_RANGE; }
  if !ofr_known(kind, t) { return ifx t % 2 == 0 then OFR_UNKNOWN_EVEN else OFR_OK; }
  none := OFR_NO_FEATURES; mpp := OFR_INVOICE_FEATURES;
  if t == OFR_CHAINS { return ifx v.count == 0 || v.count % 32 != 0 then OFR_BAD_LENGTH else OFR_OK; }
  if t == OFR_CURRENCY || t == OFR_DESCRIPTION || t == OFR_ISSUER || t == REQ_PAYER_NOTE || t == PROOF_NOTE {
    return ifx ofr_utf8_vartime(v) then OFR_OK else OFR_BAD_UTF8;
  }
  if t == OFR_AMOUNT || t == OFR_ABSOLUTE_EXPIRY || t == OFR_QUANTITY_MAX || t == REQ_AMOUNT || t == REQ_QUANTITY
     || t == INV_CREATED_AT || t == INV_AMOUNT { return ofr_tu_code(v, 8); }
  if t == INV_RELATIVE_EXPIRY { return ofr_tu_code(v, 4); }
  if t == OFR_FEATURES || t == REQ_FEATURES { return ofr_features_code(v, none[..]); }
  if t == INV_FEATURES { return ofr_features_code(v, mpp[..]); }
  if t == OFR_PATHS || t == REQ_PATHS || t == INV_PATHS { return ofr_paths_vartime(v, 0).code; }
  if t == OFR_ISSUER_ID || t == REQ_PAYER_ID || t == INV_NODE_ID { return ifx ofr_point_vartime(v) then OFR_OK else OFR_BAD_POINT; }
  if t == REQ_CHAIN || t == INV_PAYMENT_HASH || t == PROOF_PREIMAGE { return ifx v.count == 32 then OFR_OK else OFR_BAD_LENGTH; }
  if t == REQ_BIP_353_NAME {
    if v.count < 1 || v.count < 2 + cast(int) v[0] { return OFR_BAD_LENGTH; }
    name := cast(int) v[0]; domain := cast(int) v[1 + name];
    if v.count != 2 + name + domain { return OFR_BAD_LENGTH; }
    for i: 0..<v.count {
      if i == 0 || i == 1 + name { continue; }
      c := v[i];
      ok := (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '-' || c == '_' || c == '.';
      if !ok { return OFR_BAD_VALUE; }
    }
    return OFR_OK;
  }
  if t == INV_BLINDEDPAY { return ofr_payinfo_vartime(v).code; }
  if t == INV_FALLBACKS {
    at := 0;
    while at < v.count {
      if v.count - at < 3 || v.count - at - 3 < ln_u16(v, at + 1) { return OFR_BAD_LENGTH; }
      at += 3 + ln_u16(v, at + 1);
    }
    return OFR_OK;
  }
  if t == OFR_SIGNATURE || t == PROOF_SIGNATURE { return ifx v.count == 64 then OFR_OK else OFR_BAD_LENGTH; }
  if t == PROOF_OMITTED {
    at := 0;
    while at < v.count {
      s := bigsize_read_vartime(v, at);
      if s.code == LN_TRUNCATED { return OFR_TRUNCATED; }
      if s.code != LN_OK { return OFR_NOT_MINIMAL; }
      at = s.next;
    }
    return OFR_OK;
  }
  if t == PROOF_MISSING || t == PROOF_LEAVES { return ifx v.count % 32 == 0 then OFR_OK else OFR_BAD_LENGTH; }
  return OFR_OK;   // offer_metadata, invreq_metadata: any bytes
}
// The records of b[0].data[0..<n]: first the framing of the whole stream,
// then each record against its type.
ofr_frame_vartime :: (b: [] Bolt12) -> int {
  b[0].count = 0; at := 0; previous: u64 = 0;
  while at < b[0].n {
    r := tlv_next_vartime(b[0].data[0..<b[0].n], at, previous, b[0].count == 0);
    if r.code == LN_TRUNCATED { return OFR_TRUNCATED; }
    if r.code == LN_NOT_MINIMAL { return OFR_NOT_MINIMAL; }
    if r.code != LN_OK { return OFR_ORDER; }
    if b[0].count == OFR_MAX_RECORDS { return OFR_CAPACITY; }
    b[0].records[b[0].count] = OfrRecord.{type = r.type, at = at, start = r.start, length = r.length};
    b[0].count += 1; previous = r.type; at = r.next;
  }
  return OFR_OK;
}
ofr_stream_vartime :: (b: [] Bolt12) -> int {
  framed := ofr_frame_vartime(b);
  if framed != OFR_OK { return framed; }
  for i: 0..<b[0].count {
    rec := b[0].records[i];
    code := ofr_record_vartime(b[0].kind, rec.type, b[0].data[rec.start..<rec.start + rec.length]);
    if code != OFR_OK { return code; }
  }
  return OFR_OK;
}
```

## The Merkle tree

A signature over the bytes would cover the whole invoice: to prove one
field, you would have to show them all. BOLT 12 signs a Merkle root
instead, so a field can be shown with the hashes of the others.

Each field gives two leaves. `H("LnLeaf", record)` is the record itself.
`H("LnNonce" || first record, type)` is a nonce: the tag includes the
stream's first record, which in a request or invoice is
`invreq_metadata`, bytes the payer picks at random. Without the nonce,
anyone could guess a hidden field by hashing likely values until one
matches its leaf. The two leaves join into one branch, and the branches
join into a tree. A branch hashes its two children smaller first, so a
proof never needs to say which side a hash is on.

When the number of fields is not a power of two, the tree joins
neighbors, then neighbors of neighbors, and a branch without a neighbor
waits for the next level. The root then joins the first 2^k branches,
the largest power of two below the count, with the rest.

<!-- source: coax/lib/lightning/offers.coax merkle -->
```jai
// H(tag, msg) is BIP-340's tagged hash: SHA256(SHA256(tag) ||
// SHA256(tag) || msg). Each field outside the signature range is a leaf
// H("LnLeaf", record), paired with a nonce leaf H("LnNonce" || first
// record, type): the first record of a request or invoice is
// invreq_metadata, unpredictable bytes, so a hidden field cannot be
// guessed from its leaf. A branch is H("LnBranch", lesser || greater):
// sorted, so a proof needs no left or right.
ofr_tag :: (tag: [] u8, out: [] u8) { #assert sha256_hash_checked(tag, out, false) == SHA_OK; }
ofr_tagged :: (t: [] u8, a: [] u8, b: [] u8, out: [] u8) {
  sha: [1] ShaOwned;
  #assert so_init(sha[..]) && so_update(sha[..], t, false) && so_update(sha[..], t, false);
  #assert so_update(sha[..], a, false) && so_update(sha[..], b, false) && so_finish(sha[..], out, false);
}
ofr_less :: (a: [] u8, b: [] u8) -> bool {
  for i: 0..<32 { if a[i] != b[i] { return a[i] < b[i]; } }
  return false;
}
ofr_branch :: (t: [] u8, a: [] u8, b: [] u8, out: [] u8) {
  if ofr_less(b, a) { ofr_tagged(t, b, a, out); } else { ofr_tagged(t, a, b, out); }
}
OfrTags :: struct { leaf: [32] u8; nonce: [32] u8; branch: [32] u8; }
// The tags of a stream whose first record is first.
ofr_tags :: (first: [] u8) -> OfrTags {
  r: OfrTags; m: [4200] u8;
  ofr_tag(ofr_text("LnLeaf", m[..]), r.leaf[..]); ofr_tag(ofr_text("LnBranch", m[..]), r.branch[..]);
  word := "LnNonce";
  for i: 0..<7 { m[i] = word[i]; }
  for i: 0..<first.count { m[7 + i] = first[i]; }
  ofr_tag(m[0..<7 + first.count], r.nonce[..]);
  return r;
}
// A field's nonce leaf, and the branch of its leaf and nonce.
ofr_nonce :: (tags: [] OfrTags, type: u64, out: [] u8) {
  t: [9] u8; n := bigsize_write_vartime(t[..], 0, type);
  ofr_tagged(tags[0].nonce[..], t[0..<n], t[0..<0], out);
}
ofr_leaf :: (tags: [] OfrTags, record: [] u8, nonce: [] u8, out: [] u8) {
  leaf: [32] u8; ofr_tagged(tags[0].leaf[..], record, record[0..<0], leaf[..]);
  ofr_branch(tags[0].branch[..], leaf[..], nonce, out);
}
ofr_signed_type :: (t: u64) -> bool { return t >= 240 && t <= 1000; }
// The tree joins pairs, then pairs of pairs: with n leaves not a power of
// two, the root joins the first 2^k (the largest power of two below n)
// and the rest, so the lowest types sit deepest.
ofr_split :: (n: int) -> int { k := 1; while k * 2 < n { k *= 2; } return k; }
ofr_root :: (tags: [] OfrTags, nodes: [] [32] u8, lo: int, hi: int, out: [] u8) {
  if hi - lo == 1 { for i: 0..<32 { out[i] = nodes[lo][i]; } return; }
  k := ofr_split(hi - lo); a: [32] u8; c: [32] u8;
  ofr_root(tags, nodes, lo, lo + k, a[..]); ofr_root(tags, nodes, lo + k, hi, c[..]);
  ofr_branch(tags[0].branch[..], a[..], c[..], out);
}
// The root of every record of b outside the signature range. False for
// a stream with no such record.
ofr_merkle_vartime :: (b: [] Bolt12, out: [] u8) -> bool {
  first := -1;
  for i: 0..<b[0].count { if first < 0 && !ofr_signed_type(b[0].records[i].type) { first = i; } }
  if first < 0 { return false; }
  tags: [1] OfrTags; tags[0] = ofr_tags(ofr_raw(b, first));
  nodes: [64] [32] u8; n := 0; nonce: [32] u8;
  for i: 0..<b[0].count {
    t := b[0].records[i].type;
    if ofr_signed_type(t) { continue; }
    ofr_nonce(tags[..], t, nonce[..]); ofr_leaf(tags[..], ofr_raw(b, i), nonce[..], nodes[n][..]); n += 1;
  }
  ofr_root(tags[..], nodes[..], 0, n, out);
  return true;
}
// The message a signature signs: H("lightning" || stream name || field
// name, root).
ofr_sig_hash :: (kind: int, field: string, root: [] u8, out: [] u8) {
  m: [64] u8; n := 0;
  for c: "lightning" { m[n] = c; n += 1; }
  for c: OFR_NAMES[kind - 1] { m[n] = c; n += 1; }
  for c: field { m[n] = c; n += 1; }
  t: [32] u8; ofr_tag(m[0..<n], t[..]);
  ofr_tagged(t[..], root, root[0..<0], out);
}
// A BIP-340 signature under a 33-byte key: the x coordinate is the key.
ofr_verify_vartime :: (b: [] Bolt12, field: string, key: [] u8, sig: [] u8) -> bool {
  root: [32] u8; h: [32] u8;
  if key.count != 33 || sig.count != 64 || !ofr_merkle_vartime(b, root[..]) { return false; }
  ofr_sig_hash(b[0].kind, field, root[..], h[..]);
  return schnorr_fast_verify_vartime(h[..], key[1..<33], sig);
}
```

The signature signs `H("lightning" || "invoice_request" || "signature",
root)` for a request, and the same with `invoice` for an invoice. The
tag names the stream and the field, so a signature for one cannot be
used as the other.

## Offers

An offer is not signed. Its rules are about meaning: an amount needs a
description of what it buys, a currency needs an amount, and an amount
must not be zero. Most importantly, an offer must say where to send a
request: to `offer_issuer_id`, a node ID, or through `offer_paths`.

With `offer_issuer_id`, the payer knows which node will sign the
invoice. With only `offer_paths`, the seller hides its node: the payer
sends its request into a blinded path, and the invoice must be signed
with the key of that path's last blinded hop. Only the node at the end
of the path can compute that key, from its own secret and the path key
(chapter 111's `onion_blinded_secret`), so the payer still knows the
invoice came from the offer's issuer.

<!-- source: coax/lib/lightning/offers.coax offer -->
```jai
// An offer: an amount needs a description and must be above zero, a
// currency needs an amount, and a payer must have somewhere to send a
// request: offer_issuer_id, offer_paths, or both.
ofr_check_offer_vartime :: (b: [] Bolt12) -> int {
  if ofr_has(b, OFR_AMOUNT) && ofr_number(b, OFR_AMOUNT) == 0 { return OFR_BAD_AMOUNT; }
  if ofr_has(b, OFR_AMOUNT) && !ofr_has(b, OFR_DESCRIPTION) { return OFR_REQUIRED; }
  if ofr_has(b, OFR_CURRENCY) && !ofr_has(b, OFR_AMOUNT) { return OFR_REQUIRED; }
  if !ofr_has(b, OFR_ISSUER_ID) && !ofr_has(b, OFR_PATHS) { return OFR_REQUIRED; }
  return OFR_OK;
}
```

## Invoice requests

A request copies the offer, adds `invreq_metadata`, `invreq_payer_id`
and what the payer chooses (an amount, a quantity, a note), and is signed
with the payer's key. `invreq_payer_id` should be a fresh key for each
request, so requests cannot be linked. A request without an offer is an
offer to send money, as a refund or an ATM would make: it names its own
amount and must not carry the fields only an offer has.

<!-- source: coax/lib/lightning/offers.coax request -->
```jai
// An invoice request: invreq_metadata, invreq_payer_id and a signature by
// that key. Answering an offer, it keeps the offer's quantity rule and
// pays at least the offer's amount when the amount is in msat. Without
// an offer it is an offer to send money: it names its amount and has no
// chains, features or quantity of an offer.
ofr_check_request_vartime :: (b: [] Bolt12) -> int {
  if !ofr_has(b, REQ_METADATA) || !ofr_has(b, REQ_PAYER_ID) || !ofr_has(b, OFR_SIGNATURE) { return OFR_REQUIRED; }
  if !ofr_verify_vartime(b, "signature", ofr_value(b, REQ_PAYER_ID), ofr_value(b, OFR_SIGNATURE)) { return OFR_BAD_SIGNATURE; }
  if ofr_has(b, OFR_ISSUER_ID) || ofr_has(b, OFR_PATHS) {
    code := ofr_check_offer_vartime(b);
    if code != OFR_OK { return code; }
    if ofr_has(b, OFR_QUANTITY_MAX) {
      q := ofr_number(b, REQ_QUANTITY); m := ofr_number(b, OFR_QUANTITY_MAX);
      if !ofr_has(b, REQ_QUANTITY) || q == 0 || (m != 0 && q > m) { return OFR_BAD_QUANTITY; }
    } else if ofr_has(b, REQ_QUANTITY) { return OFR_BAD_QUANTITY; }
    if !ofr_has(b, OFR_AMOUNT) && !ofr_has(b, REQ_AMOUNT) { return OFR_REQUIRED; }
    if ofr_has(b, OFR_AMOUNT) && !ofr_has(b, OFR_CURRENCY) && ofr_has(b, REQ_AMOUNT) {
      q: u64 = ifx ofr_has(b, REQ_QUANTITY) then ofr_number(b, REQ_QUANTITY) else 1;
      a := ofr_number(b, OFR_AMOUNT);
      if q != 0 && a > CB_MAX64 / q { return OFR_BAD_AMOUNT; }
      if ofr_number(b, REQ_AMOUNT) < a * q { return OFR_BAD_AMOUNT; }
    }
  } else {
    if ofr_has(b, OFR_CHAINS) || ofr_has(b, OFR_FEATURES) || ofr_has(b, OFR_QUANTITY_MAX) { return OFR_FORBIDDEN; }
    if !ofr_has(b, REQ_AMOUNT) { return OFR_REQUIRED; }
  }
  return OFR_OK;
}
```

An amount in another currency (`offer_currency`) is a guide only: the
seller converts it when it writes the invoice. The reader checks a
request's amount against the offer only when both are in millisatoshis.

## Invoices

An invoice copies the request without its signature and adds the
payment: blinded paths with their payinfo, the creation time, the payment
hash, the amount, and the node ID that signs it. The payer then checks
that the invoice answers its request: every field of the request's
ranges must be the same, with nothing added. A seller that changed the
description or the quantity would be caught there.

<!-- source: coax/lib/lightning/offers.coax invoice -->
```jai
// An invoice: an amount, a creation time, a payment hash and a node ID,
// blinded paths with one payinfo each and at least one a payer can use,
// and a signature by invoice_node_id. When the offer named its issuer,
// that is the node; when the request named an amount, that is the amount.
ofr_check_invoice_vartime :: (b: [] Bolt12) -> int {
  if !ofr_has(b, INV_AMOUNT) || !ofr_has(b, INV_CREATED_AT) || !ofr_has(b, INV_PAYMENT_HASH) || !ofr_has(b, INV_NODE_ID) {
    return OFR_REQUIRED;
  }
  if !ofr_has(b, INV_PATHS) || !ofr_has(b, INV_BLINDEDPAY) { return OFR_REQUIRED; }
  paths := ofr_paths_vartime(ofr_value(b, INV_PATHS), 0); pay := ofr_payinfo_vartime(ofr_value(b, INV_BLINDEDPAY));
  if paths.count != pay.count || pay.usable == 0 { return OFR_BAD_PATH; }
  if !ofr_has(b, OFR_SIGNATURE) { return OFR_REQUIRED; }
  if !ofr_verify_vartime(b, "signature", ofr_value(b, INV_NODE_ID), ofr_value(b, OFR_SIGNATURE)) { return OFR_BAD_SIGNATURE; }
  if ofr_has(b, OFR_ISSUER_ID) && cast(string) ofr_value(b, OFR_ISSUER_ID) != cast(string) ofr_value(b, INV_NODE_ID) {
    return OFR_WRONG_NODE;
  }
  if ofr_has(b, REQ_AMOUNT) && ofr_number(b, REQ_AMOUNT) != ofr_number(b, INV_AMOUNT) { return OFR_BAD_AMOUNT; }
  return OFR_OK;
}
// An invoice is out of date when now is past invoice_created_at plus
// invoice_relative_expiry, or two hours without one.
offer_invoice_expired_vartime :: (b: [] Bolt12, now: u64) -> bool {
  expiry: u64 = ifx ofr_has(b, INV_RELATIVE_EXPIRY) then ofr_number(b, INV_RELATIVE_EXPIRY) else OFR_RELATIVE_EXPIRY;
  return now > ofr_number(b, INV_CREATED_AT) + expiry;
}
// A payer accepts an invoice for its own request only when every field of
// the request's ranges (0 to 159, and the request's experimental billion)
// is the same, and no field is added there.
offer_matches_vartime :: (request: [] Bolt12, invoice: [] Bolt12) -> bool {
  mine := 0;
  for i: 0..<invoice[0].count {
    t := invoice[0].records[i].type;
    if t <= 159 || (t >= OFR_BILLION && t <= 2999999999) {
      j := ofr_find(request, t);
      if j < 0 || cast(string) ofr_raw(request, j) != cast(string) ofr_raw(invoice, i) { return false; }
      mine += 1;
    }
  }
  theirs := 0;
  for i: 0..<request[0].count { if !ofr_signed_type(request[0].records[i].type) { theirs += 1; } }
  return mine == theirs;
}
```

## Reading any of them

`offer_decode_vartime` reads the text, frames and checks the stream, then
applies the rules of its kind.

<!-- source: coax/lib/lightning/offers.coax decode -->
```jai
// Reads text into b[0]: the text, the stream, then the kind's rules.
// Returns OFR_OK or the first refusal.
offer_decode_vartime :: (text: string, b: [] Bolt12) -> int {
  fresh: Bolt12; b[0] = fresh;
  t := offer_text_decode_vartime(text, b[0].data[..]);
  if t.code != OFR_OK { b[0].code = t.code; return t.code; }
  b[0].kind = t.kind; b[0].n = t.length;
  code := ofr_stream_vartime(b);
  if code == OFR_OK {
    if t.kind == OFR_OFFER { code = ofr_check_offer_vartime(b); }
    else if t.kind == OFR_REQUEST { code = ofr_check_request_vartime(b); }
    else if t.kind == OFR_INVOICE { code = ofr_check_invoice_vartime(b); }
    else { code = ofr_check_proof_vartime(b); }
  }
  b[0].code = code;
  return code;
}
```

## Writing

The writer collects records in any order and keeps them sorted, since a
stream must be. Signing builds the stream, takes its Merkle root and puts
the signature as one more record. BIP-340 with zero auxiliary randomness
gives the same signature every time, which is what BOLT 12's vectors use.

<!-- source: coax/lib/lightning/offers.coax writer -->
```jai
// A stream being written. Records may be put in any order; the writer
// keeps them sorted by type and refuses a type twice.
OfrWriter :: struct {
  code: int; kind: int;
  pool: [4096] u8; used: int;
  types: [64] u64; starts: [64] int; lengths: [64] int; count: int;
}
offer_begin :: (w: [] OfrWriter, kind: int) { fresh: OfrWriter; w[0] = fresh; w[0].kind = kind; }
offer_put_vartime :: (w: [] OfrWriter, type: u64, value: [] u8) -> int {
  if w[0].code != OFR_OK { return w[0].code; }
  if w[0].count == OFR_MAX_RECORDS || value.count > OFR_MAX_BYTES - w[0].used { w[0].code = OFR_CAPACITY; return w[0].code; }
  at := w[0].count;
  while at > 0 && w[0].types[at - 1] > type { at -= 1; }
  if at > 0 && w[0].types[at - 1] == type { w[0].code = OFR_ORDER; return w[0].code; }
  k := w[0].count;
  while k > at { w[0].types[k] = w[0].types[k - 1]; w[0].starts[k] = w[0].starts[k - 1]; w[0].lengths[k] = w[0].lengths[k - 1]; k -= 1; }
  w[0].types[at] = type; w[0].starts[at] = w[0].used; w[0].lengths[at] = value.count;
  for i: 0..<value.count { w[0].pool[w[0].used + i] = value[i]; }
  w[0].used += value.count; w[0].count += 1;
  return OFR_OK;
}
// A truncated number: no leading zero bytes, and zero as no bytes.
offer_put_number_vartime :: (w: [] OfrWriter, type: u64, v: u64) -> int {
  b: [8] u8; n := tu_write_vartime(b[..], 0, v);
  return offer_put_vartime(w, type, b[0..<n]);
}
// The stream so far, written into b[0] and read back.
offer_stream_vartime :: (w: [] OfrWriter, b: [] Bolt12) -> int {
  fresh: Bolt12; b[0] = fresh; b[0].kind = w[0].kind;
  if w[0].code != OFR_OK { return w[0].code; }
  at := 0;
  for i: 0..<w[0].count {
    at = tlv_write_vartime(b[0].data[..], at, w[0].types[i], w[0].pool[w[0].starts[i]..<w[0].starts[i] + w[0].lengths[i]]);
    if at < 0 { return OFR_CAPACITY; }
  }
  b[0].n = at;
  return ofr_stream_vartime(b);
}
// Signs the stream's Merkle root as field (a type in 240 to 1000, named
// name) with key, BIP-340 with zero auxiliary randomness, and puts the
// signature.
offer_sign_vartime :: (w: [] OfrWriter, type: u64, name: string, key: [] u8) -> int {
  b: [1] Bolt12; root: [32] u8; h: [32] u8; sig: [64] u8; aux: [32] u8; work: [1] SignWork;
  code := offer_stream_vartime(w, b[..]);
  if code != OFR_OK { return code; }
  if !ofr_merkle_vartime(b[..], root[..]) { return OFR_REQUIRED; }
  ofr_sig_hash(w[0].kind, name, root[..], h[..]);
  status := SIGN_REFUSED;
  for k: 0..<6 { status = schnorr_sign_step(key, aux[..], h[..], work[..], sig[..], false); }
  if status != SIGN_DONE { w[0].code = OFR_BAD_SIGNATURE; return w[0].code; }
  return offer_put_vartime(w, type, sig[..]);
}
// The text of the stream. Returns its length, or -1.
offer_write_vartime :: (w: [] OfrWriter, out: [] u8) -> int {
  b: [1] Bolt12;
  if offer_stream_vartime(w, b[..]) != OFR_OK { return -1; }
  return offer_text_encode_vartime(w[0].kind, b[0].data[0..<b[0].n], out);
}
// The compressed public key of a 32-byte secret.
offer_public :: (key: [] u8) -> [33] u8 {
  out: [33] u8; k := sc_load(key);
  n := se_encode(smc_multiply(k.value, sg_generator()).value, SE_COMPRESSED, out[..]);
  return out;
}
```

The payer's and the seller's steps:

<!-- source: coax/lib/lightning/offers.coax flow -->
```jai
// A payer's request for an offer: every field of the offer, then
// invreq_metadata, invreq_payer_id, and the amount and quantity when the
// payer sets them (0 leaves them out), signed with the payer's key.
offer_request_vartime :: (offer: [] Bolt12, metadata: [] u8, payer_key: [] u8, amount: u64, quantity: u64, w: [] OfrWriter) -> int {
  offer_begin(w, OFR_REQUEST);
  if offer[0].kind != OFR_OFFER || offer[0].code != OFR_OK { w[0].code = OFR_FORBIDDEN; return w[0].code; }
  for i: 0..<offer[0].count {
    r := offer[0].records[i];
    code := offer_put_vartime(w, r.type, offer[0].data[r.start..<r.start + r.length]);
  }
  payer := offer_public(payer_key);
  code := offer_put_vartime(w, REQ_METADATA, metadata);
  code = offer_put_vartime(w, REQ_PAYER_ID, payer[..]);
  if amount != 0 { code = offer_put_number_vartime(w, REQ_AMOUNT, amount); }
  if quantity != 0 { code = offer_put_number_vartime(w, REQ_QUANTITY, quantity); }
  return offer_sign_vartime(w, OFR_SIGNATURE, "signature", payer_key);
}
// A payee's invoice for a request: every field of the request but its
// signature, then the blinded paths and their payinfo, the creation time,
// the payment hash, the amount and the node ID, signed with the node key.
// When the offer named no issuer, node_key is the secret of the last
// blinded_node_id of the path the request came by (chapter 111).
offer_invoice_vartime :: (request: [] Bolt12, node_key: [] u8, paths: [] u8, payinfo: [] u8, created_at: u64,
                          payment_hash: [] u8, amount: u64, w: [] OfrWriter) -> int {
  offer_begin(w, OFR_INVOICE);
  if request[0].kind != OFR_REQUEST || request[0].code != OFR_OK { w[0].code = OFR_FORBIDDEN; return w[0].code; }
  for i: 0..<request[0].count {
    r := request[0].records[i];
    if !ofr_signed_type(r.type) { code := offer_put_vartime(w, r.type, request[0].data[r.start..<r.start + r.length]); }
  }
  node := offer_public(node_key);
  code := offer_put_vartime(w, INV_PATHS, paths);
  code = offer_put_vartime(w, INV_BLINDEDPAY, payinfo);
  code = offer_put_number_vartime(w, INV_CREATED_AT, created_at);
  code = offer_put_vartime(w, INV_PAYMENT_HASH, payment_hash);
  code = offer_put_number_vartime(w, INV_AMOUNT, amount);
  code = offer_put_vartime(w, INV_NODE_ID, node[..]);
  return offer_sign_vartime(w, OFR_SIGNATURE, "signature", node_key);
}
```

## Payer proofs

Once paid, a payer holds the preimage, which proves the invoice was
paid, but not by whom: the seller knows the preimage too. A payer proof
adds a signature by `invreq_payer_id`, the key only the payer has, over
the invoice fields it chooses to show, the preimage and an optional note.

The proof keeps the invoice's signature, so a reader must rebuild the
invoice's Merkle root without the hidden fields. For each field it keeps,
the proof gives its nonce leaf (`proof_leaf_hashes`); the reader hashes
the record itself. For each hidden field it gives a marker number
(`proof_omitted_tlvs`): not the real type, which would leak what kind of
field was hidden, but the smallest number that keeps the order. For each
hidden subtree next to a kept one, it gives that subtree's hash
(`proof_missing_hashes`), in the order a walk of the tree reaches them.
`invreq_metadata` is always hidden: it seeds every nonce, and showing it
would let anyone guess the hidden fields.

<!-- source: coax/lib/lightning/offers.coax proof -->
```jai
// A payer proof keeps some fields of a paid invoice and hides the rest.
// It carries the invoice's signature, the preimage, a nonce leaf for each
// field it keeps (proof_leaf_hashes), a marker number for each field it
// hides (proof_omitted_tlvs), and the hash of each hidden subtree that
// sits next to a kept one (proof_missing_hashes). From those a reader
// rebuilds the invoice's root and checks the node's signature; the
// payer signs the proof with invreq_payer_id.
//
// The tree's nodes: a kept leaf (its hash), a hidden leaf the writer
// knows the hash of, or a hidden leaf the reader does not.
OFR_KEPT :: 0; OFR_HIDDEN :: 1; OFR_UNKNOWN :: 2;
OfrNode :: struct { state: int; hash: [32] u8; }
OfrHashes :: struct { bytes: [2048] u8; n: int; taken: int; }   // up to 64 hashes
OfrSub :: struct { code: int; known: bool; kept: bool; hash: [32] u8; }
// Walks the tree smallest type first, children before parents. Where one
// side holds no kept leaf and the other does, the writer appends that
// side's hash to h, and the reader takes the next hash from h instead.
ofr_proof_tree :: (tags: [] OfrTags, nodes: [] OfrNode, lo: int, hi: int, h: [] OfrHashes, reading: bool) -> OfrSub {
  if hi - lo == 1 {
    n := nodes[lo];
    return OfrSub.{known = n.state != OFR_UNKNOWN, kept = n.state == OFR_KEPT, hash = n.hash};
  }
  k := ofr_split(hi - lo);
  a := ofr_proof_tree(tags, nodes, lo, lo + k, h, reading);
  if a.code != OFR_OK { return a; }
  c := ofr_proof_tree(tags, nodes, lo + k, hi, h, reading);
  if c.code != OFR_OK { return c; }
  if a.kept != c.kept {
    side := ifx a.kept then c else a;
    if reading {
      if h[0].taken == h[0].n { return OfrSub.{code = OFR_BAD_MISSING}; }
      for i: 0..<32 { side.hash[i] = h[0].bytes[32 * h[0].taken + i]; }
      side.known = true; h[0].taken += 1;
    } else {
      if h[0].n == 64 { return OfrSub.{code = OFR_CAPACITY}; }
      for i: 0..<32 { h[0].bytes[32 * h[0].n + i] = side.hash[i]; }
      h[0].n += 1;
    }
    if a.kept { c = side; } else { a = side; }
  }
  r := OfrSub.{known = a.known && c.known, kept = a.kept || c.kept};
  if r.known { ofr_branch(tags[0].branch[..], a.hash[..], c.hash[..], r.hash[..]); }
  return r;
}
ofr_invoice_type :: (t: u64) -> bool { return t <= 239 || (t >= OFR_BILLION && t <= 3999999999); }
// The marker of a hidden field: one more than the field before it when
// that one is kept, else one more than the last marker (1 for the first,
// and a billion after 239). Field 0 has no marker: it is always hidden.
ofr_marker :: (previous_kept: bool, previous_type: u64, last: u64, count: int) -> u64 {
  if previous_kept { return previous_type + 1; }
  if count == 0 { return 1; }
  if last == 239 { return OFR_BILLION; }
  return last + 1;
}
// Reads proof_omitted_tlvs into out; returns the count, or -1.
ofr_markers :: (b: [] Bolt12, out: [] u64) -> int {
  v := ofr_value(b, PROOF_OMITTED); at := 0; n := 0;
  while at < v.count {
    s := bigsize_read_vartime(v, at);
    if s.code != LN_OK || n == out.count { return -1; }
    out[n] = s.value; n += 1; at = s.next;
  }
  return n;
}
ofr_check_proof_vartime :: (b: [] Bolt12) -> int {
  need := [8] u64.[REQ_PAYER_ID, INV_PAYMENT_HASH, INV_NODE_ID, OFR_SIGNATURE, PROOF_PREIMAGE, PROOF_MISSING, PROOF_LEAVES, PROOF_SIGNATURE];
  for t: need { if !ofr_has(b, t) { return OFR_REQUIRED; } }
  h: [32] u8;
  #assert sha256_hash_checked(ofr_value(b, PROOF_PREIMAGE), h[..], false) == SHA_OK;
  if cast(string) h[..] != cast(string) ofr_value(b, INV_PAYMENT_HASH) { return OFR_BAD_PREIMAGE; }
  // The markers: rising, never 0, inside the invoice's ranges, never a
  // kept type, and each one more than a kept type or than the marker
  // before it (or a billion after 239).
  marks: [64] u64; m := ofr_markers(b, marks[..]);
  if m < 0 { return OFR_CAPACITY; }
  kept: [64] u64; nk := 0;
  for i: 0..<b[0].count { t := b[0].records[i].type; if ofr_invoice_type(t) { kept[nk] = t; nk += 1; } }
  previous: u64 = 0;
  for i: 0..<m {
    x := marks[i];
    if x == 0 || !ofr_invoice_type(x) { return OFR_BAD_OMITTED; }
    if i > 0 && x <= marks[i - 1] { return OFR_BAD_OMITTED; }
    after_kept := false;
    for j: 0..<nk { if kept[j] == x { return OFR_BAD_OMITTED; } if kept[j] + 1 == x { after_kept = true; } }
    if !after_kept && x != previous + 1 && !(previous == 239 && x == OFR_BILLION) { return OFR_BAD_OMITTED; }
    previous = x;
  }
  leaves := ofr_value(b, PROOF_LEAVES);
  if leaves.count / 32 != nk { return OFR_BAD_LEAVES; }
  // The invoice's leaves in type order: 0 and the markers hidden, the
  // kept fields from their records and nonces.
  if nk + m + 1 > 128 { return OFR_CAPACITY; }
  nodes: [128] OfrNode; n := 0; tags: [1] OfrTags; tags[0] = ofr_tags(b[0].data[0..<0]);
  nodes[0].state = OFR_UNKNOWN; n = 1;
  ki := 0; mi := 0;
  while ki < nk || mi < m {
    if mi < m && (ki == nk || marks[mi] < kept[ki]) { nodes[n].state = OFR_UNKNOWN; mi += 1; }
    else {
      j := ofr_find(b, kept[ki]);
      ofr_leaf(tags[..], ofr_raw(b, j), leaves[32 * ki..<32 * ki + 32], nodes[n].hash[..]);
      nodes[n].state = OFR_KEPT; ki += 1;
    }
    n += 1;
  }
  missing: [1] OfrHashes; v := ofr_value(b, PROOF_MISSING);
  if v.count > 2048 { return OFR_CAPACITY; }
  for i: 0..<v.count { missing[0].bytes[i] = v[i]; }
  missing[0].n = v.count / 32;
  root := ofr_proof_tree(tags[..], nodes[0..<n], 0, n, missing[..], true);
  if root.code != OFR_OK { return root.code; }
  if !root.known || missing[0].taken != missing[0].n { return OFR_BAD_MISSING; }
  sh: [32] u8; ofr_sig_hash(OFR_INVOICE, "signature", root.hash[..], sh[..]);
  if !schnorr_fast_verify_vartime(sh[..], ofr_value(b, INV_NODE_ID)[1..<33], ofr_value(b, OFR_SIGNATURE)) { return OFR_BAD_SIGNATURE; }
  if !ofr_verify_vartime(b, "proof_signature", ofr_value(b, REQ_PAYER_ID), ofr_value(b, PROOF_SIGNATURE)) { return OFR_BAD_SIGNATURE; }
  return OFR_OK;
}
```

The writer walks the same tree with every leaf known, and records the
hashes the reader will need:

<!-- source: coax/lib/lightning/offers.coax prove -->
```jai
// A payer proof of a paid invoice: the fields of keep (never 0), the
// invoice's signature, the preimage, the markers, missing hashes and
// nonces, and a note when it is not empty, signed with the payer's key.
// explicit writes an empty proof_omitted_tlvs when nothing is hidden.
offer_proof_vartime :: (invoice: [] Bolt12, preimage: [] u8, keep: [] u64, payer_key: [] u8, note: [] u8,
                        explicit: bool, w: [] OfrWriter) -> int {
  offer_begin(w, OFR_PROOF);
  if invoice[0].kind != OFR_INVOICE || invoice[0].code != OFR_OK || invoice[0].count == 0 { w[0].code = OFR_FORBIDDEN; return w[0].code; }
  tags: [1] OfrTags; tags[0] = ofr_tags(ofr_raw(invoice, 0));
  nodes: [64] OfrNode; n := 0;
  nonces: [2048] u8; nn := 0;
  marks: [576] u8; mn := 0; last: u64 = 0; count := 0;
  previous_kept := false; previous_type: u64 = 0;
  for i: 0..<invoice[0].count {
    t := invoice[0].records[i].type;
    if ofr_signed_type(t) { continue; }
    nonce: [32] u8; ofr_nonce(tags[..], t, nonce[..]);
    ofr_leaf(tags[..], ofr_raw(invoice, i), nonce[..], nodes[n].hash[..]);
    shown := false; for k: keep { if k == t && t != 0 { shown = true; } }
    if shown {
      nodes[n].state = OFR_KEPT;
      for j: 0..<32 { nonces[32 * nn + j] = nonce[j]; } nn += 1;
      code := offer_put_vartime(w, t, invoice[0].data[invoice[0].records[i].start..<invoice[0].records[i].start + invoice[0].records[i].length]);
    } else {
      nodes[n].state = OFR_HIDDEN;
      if t != 0 {
        last = ofr_marker(previous_kept, previous_type, last, count); count += 1;
        mn = bigsize_write_vartime(marks[..], mn, last);
      }
    }
    previous_kept = shown; previous_type = t; n += 1;
  }
  missing: [1] OfrHashes;
  root := ofr_proof_tree(tags[..], nodes[0..<n], 0, n, missing[..], false);
  if root.code != OFR_OK { w[0].code = root.code; return w[0].code; }
  code := offer_put_vartime(w, OFR_SIGNATURE, ofr_value(invoice, OFR_SIGNATURE));
  code = offer_put_vartime(w, PROOF_PREIMAGE, preimage);
  if count > 0 || explicit { code = offer_put_vartime(w, PROOF_OMITTED, marks[0..<mn]); }
  code = offer_put_vartime(w, PROOF_MISSING, missing[0].bytes[0..<32 * missing[0].n]);
  code = offer_put_vartime(w, PROOF_LEAVES, nonces[0..<32 * nn]);
  if note.count > 0 { code = offer_put_vartime(w, PROOF_NOTE, note); }
  return offer_sign_vartime(w, PROOF_SIGNATURE, "proof_signature", payer_key);
}
```

## The example

The example,
[bitcoin/examples/lightning-offers.coax](../examples/lightning-offers.coax),
reads BOLT 12's vectors, takes apart one of its offers and its signed
invoice request, then follows Dave, a seller hiding behind a blinded path
from Carol, and Erin, who buys three. Dave's offer leads to Erin's
request, Dave's invoice and Erin's payer proof, and then each is changed.

<!-- source: bitcoin/examples/lightning-offers.coax example -->
```jai
#load "../../coax/lib/lightning/offers.coax";
#load "../../coax/lib/lightning/onion.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [600] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
CODES :: [25] string.["OK", "BAD_TEXT", "BAD_PREFIX", "TRUNCATED", "NOT_MINIMAL", "ORDER", "RANGE", "UNKNOWN_EVEN",
                      "BAD_LENGTH", "BAD_UTF8", "BAD_POINT", "BAD_PATH", "BAD_FEATURES", "BAD_VALUE", "REQUIRED",
                      "BAD_AMOUNT", "BAD_QUANTITY", "FORBIDDEN", "BAD_SIGNATURE", "WRONG_NODE", "BAD_PREIMAGE",
                      "BAD_OMITTED", "BAD_LEAVES", "BAD_MISSING", "CAPACITY"];
// A field's name in BOLT 12.
name :: (t: u64) -> string {
  if t == 0 { return "invreq_metadata"; } if t == 2 { return "offer_chains"; } if t == 4 { return "offer_metadata"; }
  if t == 6 { return "offer_currency"; } if t == 8 { return "offer_amount"; } if t == 10 { return "offer_description"; }
  if t == 12 { return "offer_features"; } if t == 14 { return "offer_absolute_expiry"; } if t == 16 { return "offer_paths"; }
  if t == 18 { return "offer_issuer"; } if t == 20 { return "offer_quantity_max"; } if t == 22 { return "offer_issuer_id"; }
  if t == 80 { return "invreq_chain"; } if t == 82 { return "invreq_amount"; } if t == 86 { return "invreq_quantity"; }
  if t == 88 { return "invreq_payer_id"; } if t == 89 { return "invreq_payer_note"; }
  if t == 160 { return "invoice_paths"; } if t == 162 { return "invoice_blindedpay"; } if t == 164 { return "invoice_created_at"; }
  if t == 168 { return "invoice_payment_hash"; } if t == 170 { return "invoice_amount"; } if t == 176 { return "invoice_node_id"; }
  if t == 240 { return "signature"; } if t == 241 { return "proof_signature"; } if t == 1001 { return "proof_preimage"; }
  if t == 1002 { return "proof_omitted_tlvs"; } if t == 1003 { return "proof_missing_hashes"; }
  if t == 1004 { return "proof_leaf_hashes"; } if t == 1005 { return "proof_note"; }
  return "unknown";
}
// Each record: type, name and length.
fields :: (b: [] Bolt12) {
  for i: 0..<b[0].count {
    r := b[0].records[i];
    print("  % %: % bytes\n", r.type, name(r.type), r.length);
  }
}
// A secret of 32 bytes made from two small numbers, for the example.
secret :: (k: int, c: int) -> [32] u8 {
  out: [32] u8;
  for i: 0..<32 { out[i] = cast(u8) ((k * i + c) & 255); }
  return out;
}
// Text with a '+' and a newline every 64 characters, as a writer may
// split it.
split :: (t: [] u8, out: [] u8) -> int {
  n := 0;
  for i: 0..<t.count {
    if i > 0 && i % 64 == 0 { out[n] = '+'; out[n + 1] = '\n'; n += 2; }
    out[n] = t[i]; n += 1;
  }
  return n;
}
// Puts every record of b but one type into w.
copy_but :: (b: [] Bolt12, skip: u64, w: [] OfrWriter) {
  offer_begin(w, b[0].kind);
  for i: 0..<b[0].count {
    r := b[0].records[i];
    if r.type != skip { code := offer_put_vartime(w, r.type, b[0].data[r.start..<r.start + r.length]); }
  }
}

main :: () {
  b: [1] Bolt12; out: [2048] u8; n := 0; code := 0;

  // ---- BOLT 12's vectors
  print("BOLT 12's vectors\n");
  valid := 0; refused := 0;
  for f: OFFER_FORMATS { r := offer_text_decode_vartime(f.text, out[..]); if r.code == OFR_OK { valid += 1; } else { refused += 1; } }
  print("  format strings: % read, % refused\n", valid, refused);
  valid = 0; print("  offers refused:");
  for row: OFFER_ROWS {
    code = offer_decode_vartime(row.text, b[..]);
    if code == OFR_OK { valid += 1; } else { print(" %", CODES[code]); }
  }
  print("\n  % of 20 valid offers read\n", valid);
  valid = 0;
  for row: PROOF_VALID { if offer_decode_vartime(row.proof, b[..]) == OFR_OK { valid += 1; } }
  print("  % of 5 payer proofs check; invalid ones refused:", valid);
  for row: PROOF_INVALID { print(" %", CODES[offer_decode_vartime(row.text, b[..])]); }
  print("\n");

  // ---- an offer in dollars
  print("an offer priced in dollars\n");
  code = offer_decode_vartime(OFFER_ROWS[7].text, b[..]);
  fields(b[..]);
  print("  % % cents, \"%\"\n", cast(string) ofr_value(b[..], OFR_CURRENCY), ofr_number(b[..], OFR_AMOUNT),
        cast(string) ofr_value(b[..], OFR_DESCRIPTION));

  // ---- the request's Merkle tree
  print("BOLT 12's invoice request\n");
  code = offer_decode_vartime(OFFER_REQUEST, b[..]);
  fields(b[..]);
  root: [32] u8; h: [32] u8;
  ok := ofr_merkle_vartime(b[..], root[..]);
  print("  Merkle root "); show(root[..]); print("\n");
  ofr_sig_hash(OFR_REQUEST, "signature", root[..], h[..]);
  print("  signed hash "); show(h[..]); print("\n");
  print("  the signature checks under invreq_payer_id: %\n", CODES[code]);

  // ---- Dave's offer, behind a blinded path that starts at Carol
  print("Dave's offer, behind a blinded path from Carol\n");
  carol := secret(7, 3); dave := secret(11, 5); erin := secret(13, 1);
  carol_id := offer_public(carol[..]); dave_id := offer_public(dave[..]);
  e := secret(17, 9); path_id := secret(19, 2);
  carol_data: [35] u8; carol_data[0] = 4; carol_data[1] = 33; for i: 0..<33 { carol_data[2 + i] = dave_id[i]; }
  dave_data: [34] u8; dave_data[0] = 6; dave_data[1] = 32; for i: 0..<32 { dave_data[2 + i] = path_id[i]; }
  carol_box: [51] u8; dave_box: [50] u8;
  cb := onion_blind_hop(e[..], carol_id[..], carol_data[..], carol_box[..]);
  db := onion_blind_hop(e[..], dave_id[..], dave_data[..], dave_box[..]);
  // first_node_id, first_path_key, num_hops, then each hop.
  path: [256] u8; p := 0;
  for i: 0..<33 { path[p + i] = carol_id[i]; } p += 33;
  for i: 0..<33 { path[p + i] = cb.path_key[i]; } p += 33;
  path[p] = 2; p += 1;
  for i: 0..<33 { path[p + i] = cb.blinded[i]; } p += 33; ln_put16(path[..], p, 51); p += 2;
  for i: 0..<51 { path[p + i] = carol_box[i]; } p += 51;
  for i: 0..<33 { path[p + i] = db.blinded[i]; } p += 33; ln_put16(path[..], p, 50); p += 2;
  for i: 0..<50 { path[p + i] = dave_box[i]; } p += 50;
  w: [1] OfrWriter; t: [64] u8;
  offer_begin(w[..], OFR_OFFER);
  code = offer_put_vartime(w[..], OFR_DESCRIPTION, ofr_text("Bitcoin From Scratch, chapter 115", t[..]));
  code = offer_put_number_vartime(w[..], OFR_AMOUNT, 50000);
  code = offer_put_vartime(w[..], OFR_PATHS, path[0..<p]);
  code = offer_put_vartime(w[..], OFR_ISSUER, ofr_text("Dave", t[..]));
  code = offer_put_number_vartime(w[..], OFR_QUANTITY_MAX, 10);
  n = offer_write_vartime(w[..], out[..]);
  offer: [1] Bolt12;
  code = offer_decode_vartime(cast(string) out[0..<n], offer[..]);
  print("  % characters, %\n", n, CODES[code]);
  fields(offer[..]);
  joined: [2100] u8; j := split(out[0..<n], joined[..]); again: [1] Bolt12;
  code = offer_decode_vartime(cast(string) joined[0..<j], again[..]);
  print("  split with '+' and newlines: %, same bytes: %\n", CODES[code],
        cast(string) again[0].data[0..<again[0].n] == cast(string) offer[0].data[0..<offer[0].n]);
  final := ofr_paths_vartime(ofr_value(offer[..], OFR_PATHS), 0);
  print("  no offer_issuer_id; the path ends at blinded node "); show(final.final[..]); print("\n");
  print("  is that Dave's node ID: %\n", cast(string) final.final[..] == cast(string) dave_id[..]);

  // ---- Erin asks for three
  print("Erin's invoice request\n");
  metadata := secret(23, 4);
  code = offer_request_vartime(offer[..], metadata[0..<16], erin[..], 0, 3, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  request: [1] Bolt12;
  code = offer_decode_vartime(cast(string) out[0..<n], request[..]);
  print("  % characters, %\n", n, CODES[code]);
  for i: 0..<request[0].count {
    r := request[0].records[i];
    if r.type >= 80 || r.type == 0 { print("  added % %: % bytes\n", r.type, name(r.type), r.length); }
  }
  print("  invreq_metadata is first, so it seeds every nonce: %\n", request[0].records[0].type == 0);

  // ---- Dave's invoice, signed with his key for the path
  print("Dave's invoice\n");
  blinded: [32] u8;
  code = onion_blinded_secret(dave[..], db.path_key[..], blinded[..]);
  key := offer_public(blinded[..]);
  print("  Dave's blinded secret gives the path's last key: %\n", cast(string) key[..] == cast(string) final.final[..]);
  preimage := secret(29, 6); hash: [32] u8;
  ok = sha256_hash_checked(preimage[..], hash[..], false) == SHA_OK;
  payinfo: [28] u8;
  ok = cb_store_be32(payinfo[..], 0, 1000) && cb_store_be32(payinfo[..], 4, 100);
  ln_put16(payinfo[..], 8, 144); payinfo[17] = 1;
  ok = cb_store_be64(payinfo[..], 18, 10000000000);
  amount := ofr_number(offer[..], OFR_AMOUNT) * ofr_number(request[..], REQ_QUANTITY);
  code = offer_invoice_vartime(request[..], blinded[..], path[0..<p], payinfo[..], 1762156800, hash[..], amount, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  invoice: [1] Bolt12;
  code = offer_decode_vartime(cast(string) out[0..<n], invoice[..]);
  print("  % characters, %, % msat for 3\n", n, CODES[code], ofr_number(invoice[..], INV_AMOUNT));
  for i: 0..<invoice[0].count {
    r := invoice[0].records[i];
    if r.type >= 160 { print("  added % %: % bytes\n", r.type, name(r.type), r.length); }
  }
  print("  Erin: it repeats her request: %\n", offer_matches_vartime(request[..], invoice[..]));
  print("  Erin: invoice_node_id is the path's last key: %\n",
        cast(string) ofr_value(invoice[..], INV_NODE_ID) == cast(string) final.final[..]);
  print("  Erin: expired two hours and a second later: %\n", offer_invoice_expired_vartime(invoice[..], 1762156800 + 7201));

  // ---- Erin's payer proof, after she pays
  print("Erin's payer proof\n");
  keep := [5] u64.[OFR_DESCRIPTION, REQ_PAYER_ID, INV_PAYMENT_HASH, INV_AMOUNT, INV_NODE_ID];
  code = offer_proof_vartime(invoice[..], preimage[..], keep[..], erin[..], ofr_text("paid", t[..]), false, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  proof: [1] Bolt12;
  code = offer_decode_vartime(cast(string) out[0..<n], proof[..]);
  print("  % characters, %\n", n, CODES[code]);
  fields(proof[..]);
  marks: [64] u64; m := ofr_markers(proof[..], marks[..]);
  print("  proof_omitted_tlvs:"); for i: 0..<m { print(" %", marks[i]); } print("\n");
  print("  % hashes stand for the % hidden fields\n", ofr_value(proof[..], PROOF_MISSING).count / 32, m + 1);

  // ---- changes
  print("changing them\n");
  changed: [1] Bolt12;
  copy_but(invoice[..], INV_AMOUNT, w[..]);
  code = offer_put_number_vartime(w[..], INV_AMOUNT, 1);
  n = offer_write_vartime(w[..], out[..]);
  print("  the invoice's amount set to 1 msat: %\n", CODES[offer_decode_vartime(cast(string) out[0..<n], changed[..])]);
  copy_but(invoice[..], 0xffffffff, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  out[100] = ifx out[100] == 'q' then 'p' else 'q';
  print("  one character of the invoice changed: %\n", CODES[offer_decode_vartime(cast(string) out[0..<n], changed[..])]);
  code = offer_invoice_vartime(request[..], dave[..], path[0..<p], payinfo[..], 1762156800, hash[..], amount, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  code = offer_decode_vartime(cast(string) out[0..<n], changed[..]);
  print("  the invoice signed with Dave's node key: %, but the path's key: %\n", CODES[code],
        cast(string) ofr_value(changed[..], INV_NODE_ID) == cast(string) final.final[..]);
  copy_but(proof[..], PROOF_PREIMAGE, w[..]);
  code = offer_put_vartime(w[..], PROOF_PREIMAGE, hash[..]);
  n = offer_write_vartime(w[..], out[..]);
  print("  the proof with another preimage: %\n", CODES[offer_decode_vartime(cast(string) out[0..<n], changed[..])]);
  copy_but(proof[..], PROOF_NOTE, w[..]);
  code = offer_put_vartime(w[..], PROOF_NOTE, ofr_text("not paid", t[..]));
  n = offer_write_vartime(w[..], out[..]);
  print("  the proof with another note: %\n", CODES[offer_decode_vartime(cast(string) out[0..<n], changed[..])]);
  code = offer_request_vartime(offer[..], metadata[0..<16], erin[..], 0, 11, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  print("  a request for 11 of at most 10: %\n", CODES[offer_decode_vartime(cast(string) out[0..<n], changed[..])]);
  code = offer_request_vartime(offer[..], metadata[0..<16], erin[..], 149999, 3, w[..]);
  n = offer_write_vartime(w[..], out[..]);
  print("  a request for 3 that pays 149999 msat: %\n", CODES[offer_decode_vartime(cast(string) out[0..<n], changed[..])]);
}
```

The output:

```text
BOLT 12's vectors
  format strings: 6 read, 6 refused
  offers refused: ORDER UNKNOWN_EVEN REQUIRED TRUNCATED TRUNCATED TRUNCATED TRUNCATED BAD_LENGTH BAD_UTF8 BAD_UTF8 BAD_UTF8 BAD_UTF8 BAD_PATH ORDER ORDER ORDER ORDER ORDER BAD_UTF8 BAD_UTF8 BAD_POINT RANGE TRUNCATED TRUNCATED BAD_FEATURES REQUIRED REQUIRED BAD_AMOUNT BAD_AMOUNT REQUIRED BAD_PATH BAD_LENGTH BAD_TEXT
  20 of 20 valid offers read
  5 of 5 payer proofs check; invalid ones refused: REQUIRED REQUIRED REQUIRED REQUIRED REQUIRED REQUIRED REQUIRED REQUIRED BAD_PREIMAGE BAD_OMITTED BAD_OMITTED BAD_OMITTED BAD_OMITTED BAD_OMITTED BAD_OMITTED BAD_OMITTED BAD_LEAVES BAD_LEAVES BAD_MISSING BAD_MISSING BAD_SIGNATURE BAD_SIGNATURE RANGE
an offer priced in dollars
  6 offer_currency: 3 bytes
  8 offer_amount: 2 bytes
  10 offer_description: 12 bytes
  22 offer_issuer_id: 33 bytes
  USD 10000 cents, "Test vectors"
BOLT 12's invoice request
  0 invreq_metadata: 8 bytes
  6 offer_currency: 3 bytes
  8 offer_amount: 1 bytes
  10 offer_description: 23 bytes
  22 offer_issuer_id: 33 bytes
  88 invreq_payer_id: 33 bytes
  240 signature: 64 bytes
  Merkle root 608407c18ad9a94d9ea2bcdbe170b6c20c462a7833a197621c916f78cf18e624
  signed hash aefe3aa88a69772c246dcaef75ed3e7566c08ecc4e9f995233526a5651fc34cd
  the signature checks under invreq_payer_id: OK
Dave's offer, behind a blinded path from Carol
  465 characters, OK
  8 offer_amount: 2 bytes
  10 offer_description: 33 bytes
  16 offer_paths: 238 bytes
  18 offer_issuer: 4 bytes
  20 offer_quantity_max: 1 bytes
  split with '+' and newlines: OK, same bytes: true
  no offer_issuer_id; the path ends at blinded node 03745dbfe9c55c4315bc0c50017024f6bfa8b78067f883a996a874fdb587127d3f
  is that Dave's node ID: false
Erin's invoice request
  660 characters, OK
  added 0 invreq_metadata: 16 bytes
  added 86 invreq_quantity: 1 bytes
  added 88 invreq_payer_id: 33 bytes
  added 240 signature: 64 bytes
  invreq_metadata is first, so it seeds every nonce: true
Dave's invoice
  Dave's blinded secret gives the path's last key: true
  1220 characters, OK, 150000 msat for 3
  added 160 invoice_paths: 238 bytes
  added 162 invoice_blindedpay: 28 bytes
  added 164 invoice_created_at: 4 bytes
  added 168 invoice_payment_hash: 32 bytes
  added 170 invoice_amount: 3 bytes
  added 176 invoice_node_id: 33 bytes
  added 240 signature: 64 bytes
  Erin: it repeats her request: true
  Erin: invoice_node_id is the path's last key: true
  Erin: expired two hours and a second later: true
Erin's payer proof
  1112 characters, OK
  10 offer_description: 33 bytes
  88 invreq_payer_id: 33 bytes
  168 invoice_payment_hash: 32 bytes
  170 invoice_amount: 3 bytes
  176 invoice_node_id: 33 bytes
  240 signature: 64 bytes
  241 proof_signature: 64 bytes
  1001 proof_preimage: 32 bytes
  1002 proof_omitted_tlvs: 8 bytes
  1003 proof_missing_hashes: 192 bytes
  1004 proof_leaf_hashes: 160 bytes
  1005 proof_note: 4 bytes
  proof_omitted_tlvs: 1 11 12 13 14 89 90 91
  6 hashes stand for the 9 hidden fields
changing them
  the invoice's amount set to 1 msat: BAD_SIGNATURE
  one character of the invoice changed: BAD_SIGNATURE
  the invoice signed with Dave's node key: OK, but the path's key: false
  the proof with another preimage: BAD_PREIMAGE
  the proof with another note: BAD_SIGNATURE
  a request for 11 of at most 10: BAD_QUANTITY
  a request for 3 that pays 149999 msat: BAD_AMOUNT
```

- All 20 valid offers read, and each invalid one refuses. The codes show
  that seven of BOLT 12's invalid offers are malformed before the point
  their description names: five blinded-path vectors leave out the
  `offer_paths` length, so the next bytes read as records out of order,
  and two type-range vectors write a 4-byte type after `0xfd`, which
  reads as a 2-byte type and then runs out of bytes.
- BOLT 12's request has six fields and a signature; its Merkle root and
  signed hash are the ones `signature-test.json` states.
- Dave's offer has no `offer_issuer_id`. Its path ends at a blinded node
  ID that is not Dave's node ID, and the offer reads the same split over
  lines with `+`.
- Erin's request adds `invreq_metadata`, `invreq_quantity`,
  `invreq_payer_id` and a signature. The metadata sorts first, so it is
  the first record that seeds every nonce.
- Dave's blinded secret gives the path's last key, and his invoice is
  signed with it. Erin checks that it repeats her request and that its
  node ID is the path's last key.
- Erin's proof keeps five fields and hides nine; six hashes stand for the
  hidden subtrees. `invreq_metadata` needs no marker, and the markers for
  the other eight are 1, 11 to 14 and 89 to 91, not the real types 8, 16,
  18, 20, 86 and 160 to 164.
- Each change refuses, except the invoice signed with Dave's own node
  key: it is a valid invoice, and only Erin's check against the path's
  last key catches it.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-offers.out](../fixtures/lightning-offers.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-offers.coax](../examples/checks/lightning-offers.coax),
which loads the example:

- each of the 291 characters of BOLT 12's signed request, changed,
  refuses, though the text has no checksum
- a `+` and a newline between any two characters of an offer leave its
  bytes as they were
- 0 to 63 bytes survive the trip to text and back
- the Merkle root of 1 to 23 records is the tree built level by level,
  as BOLT 12 describes it
- a payer proof of BOLT 12's invoice that keeps any of 16 choices of
  fields checks, with one marker per hidden field after the first

<!-- source: bitcoin/examples/checks/lightning-offers.coax check -->
```jai
// Without a checksum, a changed character of a signed invoice request
// still refuses: the prefix, the separator, the padding, the framing or
// the signature over every field catches it.
changed :: (at: int) -> int {
  copy: [291] u8; b: [1] Bolt12;
  for i: 0..<291 { copy[i] = OFFER_REQUEST[i]; }
  copy[at] = ifx copy[at] == 'q' then 'p' else 'q';
  return offer_decode_vartime(cast(string) copy[..], b[..]);
}
#forall "one changed character of a request refuses: 0 to 72" at: 0..<73 #steps 2000000000 { #assert changed(at) != OFR_OK; }
#forall "one changed character of a request refuses: 73 to 145" at: 73..<146 #steps 2000000000 { #assert changed(at) != OFR_OK; }
#forall "one changed character of a request refuses: 146 to 218" at: 146..<219 #steps 2000000000 { #assert changed(at) != OFR_OK; }
#forall "one changed character of a request refuses: 219 to 290" at: 219..<291 #steps 2000000000 { #assert changed(at) != OFR_OK; }
// A '+' and whitespace between any two characters leave the bytes as
// they were.
#forall "a '+' joins anywhere" at: 1..<97 {
  text := OFFER_ROWS[7].text; joined: [100] u8; a: [64] u8; c: [64] u8;
  for i: 0..<at { joined[i] = text[i]; }
  joined[at] = '+'; joined[at + 1] = '\n';
  for i: at..<97 { joined[i + 2] = text[i]; }
  r := offer_text_decode_vartime(text, a[..]); s := offer_text_decode_vartime(cast(string) joined[0..<99], c[..]);
  #assert r.code == OFR_OK && s.code == OFR_OK && r.length == s.length && cast(string) a[0..<r.length] == cast(string) c[0..<s.length];
}
// Any bytes survive the text and come back.
#forall "bytes survive the text" n: 0..<64 {
  b: [64] u8; for i: 0..<n { b[i] = cast(u8) ((37 * i + n) & 255); }
  t: [110] u8; back: [64] u8;
  m := offer_text_encode_vartime(OFR_OFFER, b[0..<n], t[..]);
  r := offer_text_decode_vartime(cast(string) t[0..<m], back[..]);
  #assert m == 4 + (8 * n + 4) / 5 && r.code == OFR_OK && r.length == n && cast(string) back[0..<n] == cast(string) b[0..<n];
}
// The root is BOLT 12's tree built level by level: join neighbors, then
// neighbors of neighbors, a leaf with none waiting for the next level.
#forall "the root is the tree built level by level" n: 1..<24 #steps 400000000 {
  b: [1] Bolt12; at := 0; v := [1] u8.[7];
  for k: 0..<n { at = tlv_write_vartime(b[0].data[..], at, cast(u64) (2 * k + 1), v[..]); }
  b[0].n = at;
  #assert ofr_frame_vartime(b[..]) == OFR_OK;
  tags: [1] OfrTags; tags[0] = ofr_tags(ofr_raw(b[..], 0));
  level: [24] [32] u8; nonce: [32] u8;
  for k: 0..<n { ofr_nonce(tags[..], b[0].records[k].type, nonce[..]); ofr_leaf(tags[..], ofr_raw(b[..], k), nonce[..], level[k][..]); }
  step := 1;
  while step < n {
    k := 0;
    while k + step < n { joined: [32] u8; ofr_branch(tags[0].branch[..], level[k][..], level[k + step][..], joined[..]); level[k] = joined; k += 2 * step; }
    step *= 2;
  }
  root: [32] u8;
  #assert ofr_merkle_vartime(b[..], root[..]) && cast(string) root[..] == cast(string) level[0][..];
}
// A payer proof of BOLT 12's invoice that keeps any choice of four more
// fields reads back, and its marker list hides the rest.
proves :: (mask: int) -> bool {
  inv: [1] Bolt12; w: [1] OfrWriter; key: [32] u8; pre: [32] u8; out: [2048] u8; proof: [1] Bolt12;
  if offer_decode_vartime(PROOF_VALID[1].invoice, inv[..]) != OFR_OK { return false; }
  if ln_unhex(PROOF_PAYER_KEY, key[..]).count != 32 || ln_unhex(PROOF_VALID[1].preimage, pre[..]).count != 32 { return false; }
  more := [4] u64.[OFR_ISSUER_ID, REQ_AMOUNT, INV_PATHS, INV_AMOUNT];
  keep: [7] u64; keep[0] = REQ_PAYER_ID; keep[1] = INV_PAYMENT_HASH; keep[2] = INV_NODE_ID; k := 3;
  for i: 0..<4 { if (mask >> i) & 1 == 1 { keep[k] = more[i]; k += 1; } }
  if offer_proof_vartime(inv[..], pre[..], keep[0..<k], key[..], pre[0..<0], false, w[..]) != OFR_OK { return false; }
  n := offer_write_vartime(w[..], out[..]);
  if offer_decode_vartime(cast(string) out[0..<n], proof[..]) != OFR_OK { return false; }
  marks: [64] u64;
  return ofr_markers(proof[..], marks[..]) == 9 - k && ofr_value(proof[..], PROOF_LEAVES).count == 32 * k;
}
#forall "any choice of fields gives a proof that checks: masks 0 to 3" mask: 0..<4 #steps 2000000000 { #assert proves(mask); }
#forall "any choice of fields gives a proof that checks: masks 4 to 7" mask: 4..<8 #steps 2000000000 { #assert proves(mask); }
#forall "any choice of fields gives a proof that checks: masks 8 to 11" mask: 8..<12 #steps 2000000000 { #assert proves(mask); }
#forall "any choice of fields gives a proof that checks: masks 12 to 15" mask: 12..<16 #steps 2000000000 { #assert proves(mask); }
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-offers.coax --run-steps 250000000
```

`coax test` runs 342 checks, including the loaded files' own checks, in
about 2 minutes on 2026-11-03. The changed-character and payer-proof
checks are split into four ranges each to keep each within its step
budget.

### The library's own checks

`offers.coax` is an `#authority` file. Its checks cover BOLT 12's format
strings, every offer as valid or invalid for its reason, each valid
offer's fields and the same text written again, the Merkle roots of
`signature-test.json`, its request signed again to the same text, every
payer proof's invoice and proof roots, each payer proof written again
from its invoice, each invalid payer proof refused for its reason, the
flow from offer to proof with its refusals, the text, the field formats,
and the tree's shape, markers and writer order.

```shell
coax/bin/coax test coax/lib/lightning/offers.coax --run-steps 250000000
```

`coax test` runs 286 checks, including those of the files it loads, on
2026-11-03. Mutation testing: not run (owner speed order).

### BOLT 12 and an independent reader

[offers.py](../../coax/tests/lightning/offers.py) computes every line
the example prints with
[bolt12.py](../../coax/tests/lightning/bolt12.py), its own BOLT 12
reader and writer, Merkle tree and BIP-340 signer and verifier, written
from BOLT 12 and BIP-340 on
[ln_crypto.py](../../coax/tests/lightning/ln_crypto.py)'s secp256k1,
sharing no code with Coax, and
[sphinx.py](../../coax/tests/lightning/sphinx.py)'s route blinding. All
four `bolt12/*.json` files must pass, each invalid offer and payer proof
for its reason, and each payer proof must be written again byte for byte,
before the transcript is compared.

```shell
python3 -I coax/tests/lightning/offers.py --coax coax/bin/coax
```

On 2026-11-03 every vector passes, all 76 lines agree, and the example's
output in native debug and release builds equals them. Wasm was not run.

[make_offer_vectors.py](../../coax/tests/lightning/make_offer_vectors.py)
reads the pinned BOLT 12 vectors into
[offer_vectors.coax](../../coax/tests/lightning/offer_vectors.coax).

## What this chapter leaves out

- Sending the request: onion messages carry it through `offer_paths` and
  carry the invoice back through a reply path (BOLT 4's
  `onion_message`).
- Paying the invoice over its blinded paths: chapter 113.
- `invoice_error` messages, and currency conversion for offers priced in
  another currency.
- Checking `offer_absolute_expiry` against a clock, and which chains a
  node accepts in `offer_chains` and `invreq_chain`.
- Deriving `invreq_payer_id` from a base key and `invreq_metadata`, and
  BIP-353 names beyond their character check.

## Summary

- An offer is a reusable TLV stream; a payer turns it into an invoice
  request, and the seller answers with an invoice for that payer.
- The text is Bech32's characters with no checksum, in one case, with
  optional `+` joins.
- Each kind owns a range of types, so a request copies an offer and an
  invoice copies a request without collisions, and a reader refuses
  unknown even types.
- Signatures sign the root of a Merkle tree of tagged leaves with
  nonces, joined smaller first. The tag names the stream and the field.
- An offer can name its node or hide it behind a blinded path; then the
  invoice is signed with the path's last blinded key.
- A payer proof shows some fields of a paid invoice, hides the rest
  behind marker numbers and missing hashes, and carries both the node's
  and the payer's signatures.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [24. Sign with Schnorr](ch024-sign-with-schnorr.md)
- [104. Messages and TLVs](ch104-messages-and-tlvs.md)
- [111. Onion routing and blinded paths](ch111-onion-routing-and-blinded-paths.md)

## Sources

- [BOLT 12](https://github.com/lightning/bolts/blob/master/12-offer-encoding.md),
  "Encoding", "Signature Calculation", "Offers", "Invoice Requests",
  "Invoices" and "Payer Proofs", and `bolt12/format-string-test.json`,
  `offers-test.json`, `signature-test.json` and `payer-proof-test.json`,
  at lightning/bolts revision `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/12-offer-encoding.md).
- [BOLT 4](https://github.com/lightning/bolts/blob/master/04-onion-routing.md)'s
  blinded paths, at the same revision, pinned since chapter 111.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)'s
  tagged hashes and Schnorr signatures (chapters 5, 23 and 24).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
