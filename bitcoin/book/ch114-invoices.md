# 114. Invoices

To be paid over Lightning, a node tells the payer four things: which
network, how much, the hash whose preimage proves payment, and who it is.
BOLT 11 packs these into one line of text, the invoice, that a wallet can
show as a QR code or a link.

An invoice is Bech32 (chapter 27) with no length limit. Its
human-readable part names the currency and the amount. Its data is 5-bit
words: a timestamp, tagged fields, and a recoverable ECDSA signature
(chapter 22) over everything before it. The signature does double duty:
it proves the invoice is the payee's, and the key it recovers to is the
payee's node ID. This chapter builds the reader and the writer.

## What this chapter teaches

- The human-readable part: `ln`, a currency prefix, and an amount in
  bitcoin with a multiplier `m`, `u`, `n` or `p`, where a `p` amount must
  end in 0 to be a whole number of millisatoshis.
- The data part: a 35-bit timestamp, then fields of a 5-bit type, a
  10-bit length and that many 5-bit words, then 104 words of signature.
- The fields: `p` payment hash, `s` payment secret, `d` description, `h`
  description hash, `n` payee, `x` expiry, `c` final CLTV delta, `f`
  fallback address, `r` route hints, `9` features and `m` metadata.
- The rules a reader enforces: exactly one `p` and one `s`, exactly one
  of `d` and `h`, fixed lengths for `p`, `s`, `h` and `n`, minimal
  numbers, and BOLT 9's "it's OK to be odd" for features. Unknown fields,
  and fallbacks of unknown versions, are skipped.
- The signature over the human-readable part and the data words, and why
  a reader either recovers the payee's key from it or, with an `n` field,
  checks it against that key with a low S.

## Where Lightning uses invoices

| Where | What it uses | Chapter |
| --- | --- | --- |
| Paying | the payment hash, secret, amount and final CLTV delta go into the last hop's onion payload | 111, 113 |
| Routing | `r` hints add private channels to the graph | 112, 113 |
| Proof of payment | the preimage of `p`, returned in `update_fulfill_htlc` | 109 |
| Fallback | an `f` address pays on chain when no route works | 29 |
| Offers | BOLT 12 replaces a reusable invoice string with an offer | 115 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [invoice.coax](../../coax/lib/lightning/invoice.coax) | `invoice_decode_vartime(text, inv)` | reads and checks an invoice |
| | `inv_hrp_vartime(hrp)` | the network and amount of a human-readable part |
| | `inv_field_vartime(inv, f, seen)` | one tagged field against its type's rules |
| | `inv_check_signature_vartime(inv)` | recovers the payee, or verifies against `n` |
| | `invoice_fallback_address_vartime(inv, i, out)` | a fallback as an address |
| | `invoice_begin_vartime`, `invoice_put_*_vartime` | the writer: the amount, then fields in order |
| | `invoice_sign_vartime(w, key, out)` | signs and writes the Bech32 text |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `bech32_decode_checked`, `b32_encode` | Bech32 with the extended limit (chapter 27) |
| [ecdsa_recover.coax](../../coax/lib/crypto/ecdsa_recover.coax) | `ecdsa_recover_vartime` | the key a signature recovers to (chapter 22) |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_sign_recoverable` | RFC 6979 signing with a recovery ID (chapters 20, 22) |
| [address.coax](../../coax/lib/bitcoin/address.coax) | `address_encode_vartime` | a fallback's address (chapter 29) |
| [bolt1.coax](../../coax/lib/lightning/bolt1.coax) | `ln_feature_refuse_vartime` | BOLT 9's odd rule for feature bits (chapter 104) |

An invoice is public: the payee shows it to anyone who will pay. Reading
it branches on its bytes, so those procedures end in `_vartime`. The
writer's only secret is the payee's key, which goes straight to chapter
20's constant-time signer.

The library's codes and sizes:

<!-- source: coax/lib/lightning/invoice.coax codes -->
```jai
INV_OK :: 0;
INV_BAD_BECH32 :: 1;     // not Bech32: a checksum, case, separator or character error
INV_BAD_PREFIX :: 2;     // the human-readable part is not ln and a known currency
INV_BAD_AMOUNT :: 3;     // a non-digit, an unknown multiplier, a p amount not ending in 0, or too large
INV_TOO_SHORT :: 4;      // fewer words than a timestamp and a signature
INV_TRUNCATED :: 5;      // a tagged field runs into the signature
INV_BAD_LENGTH :: 6;     // p, s or h not 52 words, n not 53, or x or c too long for 64 bits
INV_REQUIRED :: 7;       // not exactly one p and one s, or not exactly one of d and h
INV_NOT_MINIMAL :: 8;    // x, c or 9 that starts with a zero word
INV_BAD_UTF8 :: 9;       // a d field that is not UTF-8
INV_BAD_FEATURES :: 10;  // an unknown even feature bit, or basic_mpp without payment_secret
INV_BAD_FALLBACK :: 11;  // an f field of a known version with a wrong program length
INV_BAD_ROUTE :: 12;     // an r field that is not a whole number of 51-byte hops
INV_BAD_SIGNATURE :: 13; // no key recovers, or the n key does not verify a low-S signature
INV_CAPACITY :: 14;      // more words, fields, fallbacks or hops than this reader keeps

INV_MAINNET :: 1; INV_TESTNET :: 2; INV_SIGNET :: 3; INV_REGTEST :: 4;

// The tagged fields, by their 5-bit type: the letter is the type's Bech32
// character.
INV_P :: 1;   // payment_hash, 52 words
INV_S :: 16;  // payment_secret, 52 words
INV_D :: 13;  // description, UTF-8
INV_M :: 27;  // payment_metadata
INV_N :: 19;  // the payee's node ID, 53 words
INV_H :: 23;  // SHA-256 of a long description, 52 words
INV_X :: 6;   // expiry in seconds, default 3600
INV_C :: 24;  // min_final_cltv_expiry_delta, default 18
INV_F :: 9;   // a fallback on-chain address
INV_R :: 3;   // route hints: hops of 51 bytes
INV_9 :: 5;   // features

INV_TIMESTAMP :: 7;      // 35 bits
INV_SIGNATURE :: 104;    // 520 bits: r, s and the recovery ID
INV_HOP :: 51;           // pubkey 33, short_channel_id 8, fee_base_msat 4, proportional 4, cltv 2
INV_MAX_WORDS :: 4096;
INV_MAX_FIELDS :: 128;
INV_MAX_FALLBACKS :: 4;
INV_MAX_HOPS :: 16;
```

## What an invoice holds

The reader copies every value out of the text, so the caller can drop the
text afterward. It also keeps the words and where each field starts, in
the order the fields were written; the order matters, because the
signature covers it.

<!-- source: coax/lib/lightning/invoice.coax invoice -->
```jai
// What an invoice says. Every byte value is a copy, so the text can go.
InvField :: struct { type: int; start: int; length: int; }   // a field's words in words[]
InvFallback :: struct { version: int; program: [40] u8; length: int; }
InvHop :: struct { pubkey: [33] u8; scid: u64; fee_base: u64; fee_ppm: u64; cltv: int; hint: int; }
Invoice :: struct {
  code: int;
  network: int;
  hrp: [83] u8; hrp_n: int;
  has_amount: bool; amount_msat: u64;
  timestamp: u64;
  payment_hash: [32] u8; secret: [32] u8;
  description: [640] u8; description_n: int; has_description: bool;
  description_hash: [32] u8; has_hash: bool;
  payee: [33] u8; has_n: bool;     // the n field's key, or the recovered key
  expiry: u64; min_final_cltv: u64;
  features: [640] u8; features_n: int;   // a big-endian bitmap, as in BOLT 9
  metadata: [640] u8; metadata_n: int; has_metadata: bool;
  fallbacks: [4] InvFallback; fallbacks_n: int;
  hops: [16] InvHop; hops_n: int; hints: int;
  ignored: int;                    // unknown fields and fallbacks of unknown versions skipped
  words: [4096] u8; words_n: int; data_n: int;   // data_n: the words before the signature
  fields: [128] InvField; fields_n: int;
  signature: [64] u8; recovery: int;
  digest: [32] u8;                 // SHA-256 of the signed bytes
}
```

## Five bits at a time

Bech32 carries 5-bit words. A field's value in bytes is its words read
as one bit string, most significant bit first. A 32-byte payment hash is
256 bits, which takes 52 words and 4 padding bits; a 33-byte key takes
53 words and 1 padding bit. The signature is 65 bytes, exactly 104
words. The signed bytes are the words before the signature packed the
same way, with zero bits to finish the last byte.

<!-- source: coax/lib/lightning/invoice.coax words -->
```jai
// Words to bytes, 5 bits each, most significant first, with zero bits
// added at the end to fill the last byte. A field's value is the first
// (5 * words) / 8 bytes: the partial byte is padding. The signature
// covers all of them.
inv_pack :: (words: [] u8, out: [] u8) -> int {
  n := (words.count * 5 + 7) / 8;
  for i: 0..<n { out[i] = 0; }
  for k: 0..<words.count * 5 {
    if (words[k / 5] >> cast(u8) (4 - k % 5)) & 1 == 1 { out[k / 8] |= cast(u8) (128 >> (k % 8)); }
  }
  return n;
}
// Bytes to words, with zero bits added at the end to fill the last word.
inv_unpack :: (bytes: [] u8, out: [] u8) -> int {
  n := (bytes.count * 8 + 4) / 5;
  for i: 0..<n { out[i] = 0; }
  for k: 0..<bytes.count * 8 {
    if (bytes[k / 8] >> cast(u8) (7 - k % 8)) & 1 == 1 { out[k / 5] |= cast(u8) (16 >> (k % 5)); }
  }
  return n;
}
// A big-endian number in words: x, c and the timestamp.
inv_number :: (words: [] u8) -> u64 {
  v: u64 = 0;
  for w: words { v = (v << 5) | cast(u64) w; }
  return v;
}
```

## The human-readable part

Bech32 splits the text at its last `1`. Before it, an invoice has `ln`,
then the currency prefix: `bc` for mainnet, `tb` for testnet, `tbs` for
signet and `bcrt` for regtest. An amount may follow, counted in bitcoin.
A multiplier scales it down: `2500u` is 2,500 micro-bitcoin, 0.0025 BTC,
which is 250,000,000 millisatoshis. HTLCs are counted in whole
millisatoshis and `p` is a tenth of one, so a `p` amount must end in 0.
No amount means the payer chooses one, as for a donation.

<!-- source: coax/lib/lightning/invoice.coax hrp -->
```jai
// ln, the currency prefix, then an optional amount: a positive decimal
// number of bitcoin and an optional multiplier. A millisatoshi is 10^-11
// bitcoin, so m is 10^8 msat, u 10^5 and n 10^2, and p is a tenth of a
// msat: a p amount must end in 0.
INV_PREFIXES :: [4] string.["bc", "tb", "tbs", "bcrt"];
InvAmount :: struct { code: int; network: int; has: bool; msat: u64; }
inv_hrp_vartime :: (hrp: [] u8) -> InvAmount {
  if hrp.count < 2 || hrp[0] != 'l' || hrp[1] != 'n' { return InvAmount.{code = INV_BAD_PREFIX}; }
  at := 2;
  while at < hrp.count && (hrp[at] < '0' || hrp[at] > '9') { at += 1; }
  r: InvAmount;
  for p: 0..<4 { if cast(string) hrp[2..<at] == INV_PREFIXES[p] { r.network = p + 1; } }
  if r.network == 0 { return InvAmount.{code = INV_BAD_PREFIX}; }
  if at == hrp.count { return r; }
  value: u64 = 0;
  while at < hrp.count && hrp[at] >= '0' && hrp[at] <= '9' {
    d := cast(u64) (hrp[at] - '0');
    if value > (CB_MAX64 - d) / 10 { return InvAmount.{code = INV_BAD_AMOUNT}; }
    value = value * 10 + d; at += 1;
  }
  scale: u64 = 100000000000;
  if at < hrp.count {
    m := hrp[at]; at += 1;
    if at != hrp.count { return InvAmount.{code = INV_BAD_AMOUNT}; }
    if m == 'm' { scale = 100000000; }
    else if m == 'u' { scale = 100000; }
    else if m == 'n' { scale = 100; }
    else if m == 'p' {
      if value % 10 != 0 { return InvAmount.{code = INV_BAD_AMOUNT}; }
      r.has = true; r.msat = value / 10; return r;
    } else { return InvAmount.{code = INV_BAD_AMOUNT}; }
  }
  if value > CB_MAX64 / scale { return InvAmount.{code = INV_BAD_AMOUNT}; }
  r.has = true; r.msat = value * scale;
  return r;
}
```

The writer picks the largest multiplier that divides the amount, so the
text is as short as it can be. One millisatoshi is `10p`.

## The fields

After the 7-word timestamp, each field is a 5-bit type, a 10-bit length
in words and the data. The type's letter is its Bech32 character: type 1
is `p`, type 16 is `s`. A length is at most 1,023 words, 639 bytes.

Each type has its rules. `p`, `s` and `h` are 32 bytes and `n` is 33, so
any other length refuses the invoice. `x` and `c` are big-endian numbers
in the fewest words: a leading zero word is not minimal. `9` is a feature
bitmap in the same order, bit 0 being the last word's lowest bit, and it
too starts with a nonzero word. `d` must be UTF-8. An `f` field starts
with a version: 0 to 16 for a witness program, 17 for a P2PKH hash and
18 for a P2SH hash, each with its own lengths; a reader skips versions
19 to 31, which are kept for future address types. An `r` field is one
private route: hops of 51 bytes, each a node ID, a short channel ID, the
base fee, the proportional fee and the CLTV delta that channel's
`channel_update` would give (chapter 112).

<!-- source: coax/lib/lightning/invoice.coax field -->
```jai
// One tagged field: its value checked against its type and kept. A field
// whose type the reader does not know is skipped; so is a fallback of an
// unknown version.
inv_field_vartime :: (inv: [] Invoice, f: InvField, seen: [] int) -> int {
  w := inv[0].words[f.start..<f.start + f.length];
  b: [640] u8; nb := (f.length * 5) / 8; packed := inv_pack(w, b[..]);
  t := f.type;
  if t == INV_P || t == INV_S || t == INV_H || t == INV_N {
    if f.length != ifx t == INV_N then 53 else 52 { return INV_BAD_LENGTH; }
    if seen[t] > 1 { return INV_REQUIRED; }
    for i: 0..<32 {
      if t == INV_P { inv[0].payment_hash[i] = b[i]; }
      if t == INV_S { inv[0].secret[i] = b[i]; }
      if t == INV_H { inv[0].description_hash[i] = b[i]; }
    }
    if t == INV_H { inv[0].has_hash = true; }
    if t == INV_N { for i: 0..<33 { inv[0].payee[i] = b[i]; } inv[0].has_n = true; }
  } else if t == INV_D {
    if !inv_utf8_vartime(b[0..<nb]) { return INV_BAD_UTF8; }
    for i: 0..<nb { inv[0].description[i] = b[i]; }
    inv[0].description_n = nb; inv[0].has_description = true;
  } else if t == INV_X || t == INV_C {
    if f.length == 0 || w[0] == 0 { return INV_NOT_MINIMAL; }
    if f.length > 12 { return INV_BAD_LENGTH; }
    if t == INV_X { inv[0].expiry = inv_number(w); } else { inv[0].min_final_cltv = inv_number(w); }
  } else if t == INV_9 {
    if f.length == 0 || w[0] == 0 { return INV_NOT_MINIMAL; }
    // Bit k counts from the last word's least significant bit.
    n := (f.length * 5 + 7) / 8;
    for i: 0..<n { inv[0].features[i] = 0; }
    for k: 0..<f.length * 5 {
      if (w[f.length - 1 - k / 5] >> cast(u8) (k % 5)) & 1 == 1 { inv[0].features[n - 1 - k / 8] |= cast(u8) (1 << (k % 8)); }
    }
    inv[0].features_n = n;
  } else if t == INV_M {
    for i: 0..<nb { inv[0].metadata[i] = b[i]; }
    inv[0].metadata_n = nb; inv[0].has_metadata = true;
  } else if t == INV_F {
    if f.length == 0 { return INV_BAD_FALLBACK; }
    version := cast(int) w[0];
    if version > 18 { inv[0].ignored += 1; return INV_OK; }
    pb: [640] u8; n := (f.length - 1) * 5 / 8; packed = inv_pack(w[1..<f.length], pb[..]);
    good := ifx version >= 17 then n == 20 else ifx version == 0 then n == 20 || n == 32 else n >= 2 && n <= 40;
    if !good { return INV_BAD_FALLBACK; }
    if inv[0].fallbacks_n == INV_MAX_FALLBACKS { return INV_CAPACITY; }
    fb := InvFallback.{version = version, length = n};
    for i: 0..<n { fb.program[i] = pb[i]; }
    inv[0].fallbacks[inv[0].fallbacks_n] = fb; inv[0].fallbacks_n += 1;
  } else if t == INV_R {
    if nb == 0 || nb % INV_HOP != 0 { return INV_BAD_ROUTE; }
    for e: 0..<nb / INV_HOP {
      if inv[0].hops_n == INV_MAX_HOPS { return INV_CAPACITY; }
      h: InvHop; at := e * INV_HOP;
      for i: 0..<33 { h.pubkey[i] = b[at + i]; }
      h.scid = cb_load_be64(b[..], at + 33).value;
      h.fee_base = cast(u64) cb_load_be32(b[..], at + 41).value;
      h.fee_ppm = cast(u64) cb_load_be32(b[..], at + 45).value;
      h.cltv = ln_u16(b[..], at + 49);
      h.hint = inv[0].hints;
      inv[0].hops[inv[0].hops_n] = h; inv[0].hops_n += 1;
    }
    inv[0].hints += 1;
  } else {
    inv[0].ignored += 1;
  }
  return INV_OK;
}
```

BOLT 11 once told readers to skip a `p`, `s`, `h` or `n` field of the
wrong length. The pinned revision makes them fail the payment instead,
and its "`n` field of invalid length" example is invalid for that
reason: a reader that skipped the field would accept an invoice the
writer never meant. This reader follows the pinned text.

## Reading an invoice

The reader decodes the Bech32, reads the human-readable part, takes the
timestamp, and walks the fields up to the signature. Then it checks what
must appear once: exactly one `p` and one `s`, so no two parts of a
wallet can pick different hashes or secrets from one signed invoice, and
exactly one of `d` and `h`. Last come the features: an even bit the
reader does not know refuses, an odd one is ignored, and `basic_mpp`
needs `payment_secret`.

<!-- source: coax/lib/lightning/invoice.coax decode -->
```jai
// Reads an invoice into inv[0]. Returns INV_OK or the first refusal.
invoice_decode_vartime :: (text: string, inv: [] Invoice) -> int {
  fresh: Invoice; inv[0] = fresh;
  inv[0].expiry = 3600; inv[0].min_final_cltv = 18; inv[0].recovery = -1;
  // Bech32 without the 90-character limit (extended).
  r := bech32_decode_checked(text, inv[0].hrp[..], inv[0].words[..], true, false);
  if r.error == NC_CAPACITY { return INV_CAPACITY; }
  if r.error != NC_OK { return INV_BAD_BECH32; }
  inv[0].hrp_n = r.hrp_written; inv[0].words_n = r.written;
  a := inv_hrp_vartime(inv[0].hrp[0..<r.hrp_written]);
  if a.code != INV_OK { return a.code; }
  inv[0].network = a.network; inv[0].has_amount = a.has; inv[0].amount_msat = a.msat;
  if r.written < INV_TIMESTAMP + INV_SIGNATURE { return INV_TOO_SHORT; }
  end := r.written - INV_SIGNATURE; inv[0].data_n = end;
  inv[0].timestamp = inv_number(inv[0].words[0..<INV_TIMESTAMP]);
  // Tagged fields: type (5 bits), data_length (10 bits), data.
  seen: [32] int; at := INV_TIMESTAMP;
  while at < end {
    if end - at < 3 { return INV_TRUNCATED; }
    f := InvField.{type = cast(int) inv[0].words[at], start = at + 3,
                   length = cast(int) inv[0].words[at + 1] * 32 + cast(int) inv[0].words[at + 2]};
    if f.length > end - f.start { return INV_TRUNCATED; }
    if inv[0].fields_n == INV_MAX_FIELDS { return INV_CAPACITY; }
    inv[0].fields[inv[0].fields_n] = f; inv[0].fields_n += 1;
    seen[f.type] += 1;
    code := inv_field_vartime(inv, f, seen[..]);
    if code != INV_OK { return code; }
    at = f.start + f.length;
  }
  if seen[INV_P] != 1 || seen[INV_S] != 1 || seen[INV_D] + seen[INV_H] != 1 { return INV_REQUIRED; }
  known := INV_KNOWN_FEATURES;
  if ln_feature_refuse_vartime(inv[0].features[0..<inv[0].features_n], known[..]) >= 0 { return INV_BAD_FEATURES; }
  f := inv[0].features[0..<inv[0].features_n];
  if (ln_feature_set(f, 16) || ln_feature_set(f, 17)) && !ln_feature_set(f, 14) && !ln_feature_set(f, 15) { return INV_BAD_FEATURES; }
  return inv_check_signature_vartime(inv);
}
```

## The signature names the payee

The signed bytes are the human-readable part as text and the data words
before the signature, packed to bytes. Their SHA-256 is the message hash.
The signature is r and s, 32 bytes each, then a recovery ID from 0 to 3.

Without an `n` field, the reader runs chapter 22's recovery: the ID names
the nonce point R, and from r, s and the hash comes the one public key Q
the signature verifies under. That key is the payee. Recovery always
names someone, so a wallet must still compare Q with the node it
expects, or with the route it finds. With an `n` field, the reader checks
the signature against that key and requires a low S, as Bitcoin does
(chapter 20); without one, both s and n - s recover a key, so it accepts
either.

<!-- source: coax/lib/lightning/invoice.coax signature -->
```jai
// The signed bytes: the human-readable part as text, then every word
// before the signature, packed with zero bits to a whole byte.
inv_digest :: (hrp: [] u8, words: [] u8, out: [] u8) {
  m: [83 + 2560] u8;
  for i: 0..<hrp.count { m[i] = hrp[i]; }
  n := inv_pack(words, m[hrp.count..<m.count]);
  #assert sha256_hash_checked(m[0..<hrp.count + n], out, false) == SHA_OK;
}
// With an n field, the signature must verify under that key with a low s.
// Without one, the recovery ID names the nonce point, and the key that
// signature recovers to is the payee: any s is accepted.
inv_check_signature_vartime :: (inv: [] Invoice) -> int {
  sig: [65] u8;
  packed := inv_pack(inv[0].words[inv[0].data_n..<inv[0].words_n], sig[..]);
  for i: 0..<64 { inv[0].signature[i] = sig[i]; }
  inv[0].recovery = cast(int) sig[64];
  inv_digest(inv[0].hrp[0..<inv[0].hrp_n], inv[0].words[0..<inv[0].data_n], inv[0].digest[..]);
  if inv[0].has_n {
    if !ecdsa_verify_vartime(inv[0].digest[..], inv[0].payee[..], sig[0..<64], true) { return INV_BAD_SIGNATURE; }
    return INV_OK;
  }
  if ecdsa_recover_vartime(inv[0].digest[..], sig[0..<64], inv[0].recovery, SE_COMPRESSED, inv[0].payee[..]) != SE_COMPRESSED {
    return INV_BAD_SIGNATURE;
  }
  return INV_OK;
}
```

## Fallback addresses

A fallback is a version and a program. Chapter 29 builds the
scriptPubKey and its address. Signet has its own invoice prefix but uses
testnet's address formats.

<!-- source: coax/lib/lightning/invoice.coax fallback -->
```jai
// A fallback's address on the invoice's network: version 17 is P2PKH,
// 18 is P2SH, and 0 to 16 are witness programs (chapter 29). Signet
// addresses are testnet's. Returns the length or a negative AD_ code.
invoice_fallback_address_vartime :: (inv: [] Invoice, i: int, out: [] u8) -> int {
  fb := inv[0].fallbacks[i]; script: [42] u8; n := 0;
  if fb.version >= 17 { n = ad_hash_script(fb.version == 17, fb.program[0..<20], script[..]); }
  else { n = ad_witness_script(fb.version, fb.program[0..<fb.length], script[..]); }
  network := ifx inv[0].network == INV_MAINNET then SW_MAINNET else ifx inv[0].network == INV_REGTEST then SW_REGTEST else SW_TESTNET;
  return address_encode_vartime(network, script[0..<n], out);
}
```

## Writing an invoice

The writer starts with the network, the amount and the timestamp, then
takes fields in order: bytes, a number, a feature bitmap, a fallback or
a route. It signs with chapter 20's RFC 6979 signer, so the same key and
fields always give the same invoice. That is why it can reproduce BOLT
11's examples character for character.

<!-- source: coax/lib/lightning/invoice.coax writer -->
```jai
// An invoice being written: the human-readable part, then words. Fields
// go in the order they are put; the signature comes last.
InvoiceWriter :: struct { code: int; hrp: [90] u8; hrp_n: int; words: [4096] u8; n: int; }
// The amount in the fewest digits: the largest multiplier that divides
// it, or p with a trailing 0 for a whole number of msat.
invoice_begin_vartime :: (w: [] InvoiceWriter, network: int, has_amount: bool, amount_msat: u64, timestamp: u64) -> int {
  fresh: InvoiceWriter; w[0] = fresh;
  if network < INV_MAINNET || network > INV_REGTEST { w[0].code = INV_BAD_PREFIX; return w[0].code; }
  if timestamp >> 35 != 0 { w[0].code = INV_BAD_LENGTH; return w[0].code; }
  w[0].hrp[0] = 'l'; w[0].hrp[1] = 'n'; w[0].hrp_n = 2;
  for c: INV_PREFIXES[network - 1] { w[0].hrp[w[0].hrp_n] = c; w[0].hrp_n += 1; }
  if has_amount {
    value := amount_msat; m: u8 = 'p';
    if amount_msat % 100000000000 == 0 { value = amount_msat / 100000000000; m = 0; }
    else if amount_msat % 100000000 == 0 { value = amount_msat / 100000000; m = 'm'; }
    else if amount_msat % 100000 == 0 { value = amount_msat / 100000; m = 'u'; }
    else if amount_msat % 100 == 0 { value = amount_msat / 100; m = 'n'; }
    else if amount_msat > CB_MAX64 / 10 { w[0].code = INV_BAD_AMOUNT; return w[0].code; }
    else { value = amount_msat * 10; }
    digits: [20] u8; d := 0;
    while d == 0 || value > 0 { digits[d] = cast(u8) (value % 10) + '0'; value /= 10; d += 1; }
    for i: 0..<d { w[0].hrp[w[0].hrp_n] = digits[d - 1 - i]; w[0].hrp_n += 1; }
    if m != 0 { w[0].hrp[w[0].hrp_n] = m; w[0].hrp_n += 1; }
  }
  for i: 0..<INV_TIMESTAMP { w[0].words[i] = cast(u8) ((timestamp >> cast(u64) (5 * (6 - i))) & 31); }
  w[0].n = INV_TIMESTAMP;
  return INV_OK;
}
// A field of the given words: type, data_length, data.
invoice_put_words_vartime :: (w: [] InvoiceWriter, type: int, words: [] u8) -> int {
  if w[0].code != INV_OK { return w[0].code; }
  if type < 0 || type > 31 || words.count > 1023 { w[0].code = INV_BAD_LENGTH; return w[0].code; }
  if w[0].n + 3 + words.count + INV_SIGNATURE > INV_MAX_WORDS { w[0].code = INV_CAPACITY; return w[0].code; }
  w[0].words[w[0].n] = cast(u8) type;
  w[0].words[w[0].n + 1] = cast(u8) (words.count >> 5); w[0].words[w[0].n + 2] = cast(u8) (words.count & 31);
  for i: 0..<words.count { w[0].words[w[0].n + 3 + i] = words[i]; }
  w[0].n += 3 + words.count;
  return INV_OK;
}
// Bytes, padded with zero bits to whole words: p, s, h, n, d and m.
invoice_put_bytes_vartime :: (w: [] InvoiceWriter, type: int, bytes: [] u8) -> int {
  if bytes.count > 639 { w[0].code = INV_BAD_LENGTH; return w[0].code; }
  words: [1023] u8; n := inv_unpack(bytes, words[..]);
  return invoice_put_words_vartime(w, type, words[0..<n]);
}
// A number in the fewest words, no leading zero word: x and c.
invoice_put_number_vartime :: (w: [] InvoiceWriter, type: int, v: u64) -> int {
  words: [13] u8; n := 0;
  while n < 13 && (v >> cast(u64) (5 * n)) != 0 { n += 1; }
  for i: 0..<n { words[i] = cast(u8) ((v >> cast(u64) (5 * (n - 1 - i))) & 31); }
  return invoice_put_words_vartime(w, type, words[0..<n]);
}
// A feature bitmap, big-endian as in BOLT 9, in the fewest words. An
// empty bitmap writes no field at all.
invoice_put_features_vartime :: (w: [] InvoiceWriter, f: [] u8) -> int {
  top := -1;
  for bit: 0..<f.count * 8 { if ln_feature_set(f, bit) { top = bit; } }
  if top < 0 { return w[0].code; }
  n := top / 5 + 1; words: [1023] u8;
  if n > 1023 { w[0].code = INV_BAD_LENGTH; return w[0].code; }
  for k: 0..<n * 5 { if ln_feature_set(f, k) { words[n - 1 - k / 5] |= cast(u8) (1 << (k % 5)); } }
  return invoice_put_words_vartime(w, INV_9, words[0..<n]);
}
// A fallback: the version as one word, then the program or hash.
invoice_put_fallback_vartime :: (w: [] InvoiceWriter, version: int, program: [] u8) -> int {
  if version < 0 || version > 18 || program.count > 40 { w[0].code = INV_BAD_FALLBACK; return w[0].code; }
  words: [66] u8; words[0] = cast(u8) version;
  n := inv_unpack(program, words[1..<66]);
  return invoice_put_words_vartime(w, INV_F, words[0..<n + 1]);
}
// One r field: the hops of one private route, 51 bytes each.
invoice_put_route_vartime :: (w: [] InvoiceWriter, hops: [] InvHop) -> int {
  if hops.count == 0 || hops.count > 12 { w[0].code = INV_BAD_ROUTE; return w[0].code; }
  b: [612] u8;
  for e: 0..<hops.count {
    at := e * INV_HOP; h := hops[e];
    for i: 0..<33 { b[at + i] = h.pubkey[i]; }
    for i: 0..<8 { b[at + 33 + i] = cast(u8) ((h.scid >> cast(u64) (56 - 8 * i)) & 255); }
    for i: 0..<4 { b[at + 41 + i] = cast(u8) ((h.fee_base >> cast(u64) (24 - 8 * i)) & 255); }
    for i: 0..<4 { b[at + 45 + i] = cast(u8) ((h.fee_ppm >> cast(u64) (24 - 8 * i)) & 255); }
    ln_put16(b[..], at + 49, h.cltv);
  }
  return invoice_put_bytes_vartime(w, INV_R, b[0..<hops.count * INV_HOP]);
}
// Signs the hrp and words with the payee's key and writes the invoice:
// r || s || recovery ID as 104 words, then the Bech32 checksum.
InvoiceText :: struct { code: int; length: int; }
invoice_sign_vartime :: (w: [] InvoiceWriter, key: [] u8, out: [] u8) -> InvoiceText {
  if w[0].code != INV_OK { return InvoiceText.{code = w[0].code}; }
  digest: [32] u8; sig: [65] u8; work: [1] EcdsaWork;
  inv_digest(w[0].hrp[0..<w[0].hrp_n], w[0].words[0..<w[0].n], digest[..]);
  if !ecdsa_sign_recoverable(key, digest[..], work[..], sig[..], false) { return InvoiceText.{code = INV_BAD_SIGNATURE}; }
  n := inv_unpack(sig[..], w[0].words[w[0].n..<w[0].n + INV_SIGNATURE]);
  e := b32_encode(cast(string) w[0].hrp[0..<w[0].hrp_n], cast(string) w[0].words[0..<w[0].n + n], false, out, true, false, B32_BECH32);
  if e.error != NC_OK { return InvoiceText.{code = INV_CAPACITY}; }
  return InvoiceText.{length = e.written};
}
```

## The example

The example,
[bitcoin/examples/lightning-invoice.coax](../examples/lightning-invoice.coax),
reads all 31 of BOLT 11's examples, shows three of them field by field,
writes a regtest invoice with every kind of field, and then changes it.

<!-- source: bitcoin/examples/lightning-invoice.coax example -->
```jai
#load "../../coax/lib/lightning/invoice.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [1300] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
INV_NAMES :: [15] string.["OK", "BAD_BECH32", "BAD_PREFIX", "BAD_AMOUNT", "TOO_SHORT", "TRUNCATED", "BAD_LENGTH",
                          "REQUIRED", "NOT_MINIMAL", "BAD_UTF8", "BAD_FEATURES", "BAD_FALLBACK", "BAD_ROUTE",
                          "BAD_SIGNATURE", "CAPACITY"];
NETWORK_NAMES :: [5] string.["", "mainnet", "testnet", "signet", "regtest"];

// Everything a decoded invoice says, field by field in the order written.
report :: (inv: [] Invoice) {
  print("  %, ", NETWORK_NAMES[inv[0].network]);
  if inv[0].has_amount { print("% msat", inv[0].amount_msat); } else { print("any amount"); }
  print(", timestamp %\n", inv[0].timestamp);
  for f: inv[0].fields[0..<inv[0].fields_n] {
    print("  %: % words", cast(string) B32_ALPHABET[f.type..<f.type + 1], f.length);
    if f.type == INV_P { print(", payment_hash "); show(inv[0].payment_hash[..]); }
    if f.type == INV_S { print(", payment_secret "); show(inv[0].secret[..]); }
    if f.type == INV_D { print(", description \"%\"", cast(string) inv[0].description[0..<inv[0].description_n]); }
    if f.type == INV_H { print(", description_hash "); show(inv[0].description_hash[..]); }
    if f.type == INV_N { print(", payee "); show(inv[0].payee[..]); }
    if f.type == INV_X { print(", expiry % s", inv[0].expiry); }
    if f.type == INV_C { print(", min_final_cltv_expiry_delta %", inv[0].min_final_cltv); }
    if f.type == INV_M { print(", payment_metadata "); show(inv[0].metadata[0..<inv[0].metadata_n]); }
    if f.type == INV_9 {
      print(", features");
      for bit: 0..<inv[0].features_n * 8 {
        if ln_feature_set(inv[0].features[0..<inv[0].features_n], bit) { print(" %", bit); }
      }
    }
    if f.type == INV_F {
      print(", fallback version %", cast(int) inv[0].words[f.start]);
      if inv[0].words[f.start] <= 18 {
        a: [100] u8; n := invoice_fallback_address_vartime(inv, inv_fallback_index(inv, f), a[..]);
        print(" %", cast(string) a[0..<n]);
      }
    }
    if f.type == INV_R {
      for h: inv[0].hops[0..<inv[0].hops_n] {
        if h.hint == inv_hint_index(inv, f) {
          print("\n    hop "); k := h.pubkey; show(k[0..<4]);
          print("... over %x%x%, fee % msat + % ppm, cltv delta %", h.scid >> 40, (h.scid >> 16) & 0xffffff,
                h.scid & 0xffff, h.fee_base, h.fee_ppm, h.cltv);
        }
      }
    }
    print("\n");
  }
  print("  signature recovery ID %, payee ", inv[0].recovery); show(inv[0].payee[..]);
  print("%\n", ifx inv[0].has_n then " (from n)" else " (recovered)");
}
// Which fallback and which route hint a field is, counting fields of its type.
inv_fallback_index :: (inv: [] Invoice, f: InvField) -> int {
  k := 0;
  for g: inv[0].fields[0..<inv[0].fields_n] {
    if g.start == f.start { return k; }
    if g.type == INV_F && inv[0].words[g.start] <= 18 { k += 1; }
  }
  return k;
}
inv_hint_index :: (inv: [] Invoice, f: InvField) -> int {
  k := 0;
  for g: inv[0].fields[0..<inv[0].fields_n] {
    if g.start == f.start { return k; }
    if g.type == INV_R { k += 1; }
  }
  return k;
}

main :: () {
  inv: [1] Invoice;
  // ---- BOLT 11's examples
  print("BOLT 11 examples\n");
  valid := 0; refused := 0; same_key := 0;
  key: [32] u8; payee := inv_public(ln_unhex(INVOICE_KEY, key[..]));
  for row: INVOICE_ROWS {
    code := invoice_decode_vartime(row.text, inv[..]);
    if row.valid && code == INV_OK {
      valid += 1;
      if cb_ct_equal(inv[0].payee[..], payee[..]) == CB_MAX64 { same_key += 1; }
    }
    if !row.valid && code != INV_OK { refused += 1; }
  }
  print("  % of 16 valid invoices decode, % to the key that signed them\n", valid, same_key);
  print("  % of 15 invalid invoices refuse:", refused);
  for row: INVOICE_ROWS { if !row.valid { print(" %", INV_NAMES[invoice_decode_vartime(row.text, inv[..])]); } }
  print("\n");

  // ---- three of them, field by field
  print("$3 for a cup of coffee\n");
  code := invoice_decode_vartime(INVOICE_ROWS[1].text, inv[..]);
  report(inv[..]);
  print("  signed: % bytes, SHA-256 ", inv[0].hrp_n + (inv[0].data_n * 5 + 7) / 8); show(inv[0].digest[..]); print("\n");
  print("a fallback address and two route hints\n");
  code = invoice_decode_vartime(INVOICE_ROWS[5].text, inv[..]);
  report(inv[..]);
  print("an amount in pico-bitcoin and fields to skip\n");
  code = invoice_decode_vartime(INVOICE_ROWS[10].text, inv[..]);
  print("  % msat from %\n", inv[0].amount_msat, cast(string) inv[0].hrp[0..<inv[0].hrp_n]);
  code = invoice_decode_vartime(INVOICE_ROWS[13].text, inv[..]);
  print("  % unknown fields skipped, % fallbacks kept\n", inv[0].ignored, inv[0].fallbacks_n);

  // ---- writing an invoice on regtest
  print("an invoice we write\n");
  node: [32] u8; for i: 0..<32 { node[i] = cast(u8) (7 * i + 3); }
  id := inv_public(node[..]);
  preimage: [32] u8; for i: 0..<32 { preimage[i] = 0x42; }
  hash: [32] u8; done := sha256_hash_checked(preimage[..], hash[..], false);
  secret: [32] u8; for i: 0..<32 { secret[i] = cast(u8) (255 - i); }
  features: [3] u8; put := ln_feature_put(features[..], 8) && ln_feature_put(features[..], 14) && ln_feature_put(features[..], 17);
  text: [64] u8; d := "Bitcoin From Scratch, chapter 114";
  for i: 0..<d.count { text[i] = d[i]; }
  tap: [32] u8; for i: 0..<32 { tap[i] = cast(u8) (i + 1); }
  peer: [32] u8; for i: 0..<32 { peer[i] = cast(u8) ((11 * i + 5) & 255); }
  hop: [1] InvHop; hop[0].pubkey = inv_public(peer[..]);
  hop[0].scid = (cast(u64) 800000 << 40) | (cast(u64) 7 << 16) | 1; hop[0].fee_base = 1000; hop[0].fee_ppm = 100; hop[0].cltv = 144;
  w: [1] InvoiceWriter;
  code = invoice_begin_vartime(w[..], INV_REGTEST, true, 150000000, 1762156800);
  code = invoice_put_bytes_vartime(w[..], INV_P, hash[..]);
  code = invoice_put_bytes_vartime(w[..], INV_S, secret[..]);
  code = invoice_put_bytes_vartime(w[..], INV_D, text[0..<d.count]);
  code = invoice_put_number_vartime(w[..], INV_X, 600);
  code = invoice_put_number_vartime(w[..], INV_C, 80);
  code = invoice_put_fallback_vartime(w[..], 1, tap[..]);
  code = invoice_put_route_vartime(w[..], hop[..]);
  code = invoice_put_features_vartime(w[..], features[..]);
  out: [5000] u8;
  t := invoice_sign_vartime(w[..], node[..], out[..]);
  print("  % characters: %\n", t.length, cast(string) out[0..<t.length]);
  code = invoice_decode_vartime(cast(string) out[0..<t.length], inv[..]);
  print("  decodes: %\n", INV_NAMES[code]);
  report(inv[..]);
  print("  the payee is our node: %\n", cb_ct_equal(inv[0].payee[..], id[..]) == CB_MAX64);

  // ---- changes
  print("changing it\n");
  copy: [5000] u8; for i: 0..<t.length { copy[i] = out[i]; }
  copy[60] = ifx copy[60] == 'q' then 'p' else 'q';
  print("  one character changed: %\n", INV_NAMES[invoice_decode_vartime(cast(string) copy[0..<t.length], inv[..])]);
  // The amount changed and the checksum made again: the signature still
  // recovers a key, but not the payee's.
  w[0].hrp[w[0].hrp_n - 1] = 'm';
  hrp_text := cast(string) w[0].hrp[0..<w[0].hrp_n];
  code = invoice_decode_vartime(cast(string) out[0..<t.length], inv[..]);
  e := b32_encode(hrp_text, cast(string) inv[0].words[0..<inv[0].words_n], false, copy[..], true, false, B32_BECH32);
  code = invoice_decode_vartime(cast(string) copy[0..<e.written], inv[..]);
  print("  % msat with the checksum made again: %, payee ", inv[0].amount_msat, INV_NAMES[code]); show(inv[0].payee[..]); print("\n");
  print("  is it our node: %\n", cb_ct_equal(inv[0].payee[..], id[..]) == CB_MAX64);
  // The same change to an invoice that names its payee in n.
  code = invoice_begin_vartime(w[..], INV_REGTEST, true, 150000000, 1762156800);
  code = invoice_put_bytes_vartime(w[..], INV_P, hash[..]);
  code = invoice_put_bytes_vartime(w[..], INV_S, secret[..]);
  code = invoice_put_bytes_vartime(w[..], INV_D, text[0..<d.count]);
  code = invoice_put_bytes_vartime(w[..], INV_N, id[..]);
  t = invoice_sign_vartime(w[..], node[..], out[..]);
  code = invoice_decode_vartime(cast(string) out[0..<t.length], inv[..]);
  print("  with an n field: % characters, %\n", t.length, INV_NAMES[code]);
  w[0].hrp[w[0].hrp_n - 1] = 'm';
  e = b32_encode(cast(string) w[0].hrp[0..<w[0].hrp_n], cast(string) inv[0].words[0..<inv[0].words_n], false, copy[..], true, false, B32_BECH32);
  print("  its amount changed: %\n", INV_NAMES[invoice_decode_vartime(cast(string) copy[0..<e.written], inv[..])]);
}
```

From the repository root:

```shell
coax/bin/coax run bitcoin/examples/lightning-invoice.coax --run-steps 250000000
```

The output:

```text
BOLT 11 examples
  16 of 16 valid invoices decode, 15 to the key that signed them
  15 of 15 invalid invoices refuse: BAD_FEATURES BAD_LENGTH REQUIRED REQUIRED REQUIRED REQUIRED BAD_BECH32 BAD_BECH32 BAD_BECH32 BAD_SIGNATURE TOO_SHORT BAD_AMOUNT BAD_AMOUNT REQUIRED REQUIRED
$3 for a cup of coffee
  mainnet, 250000000 msat, timestamp 1496314658
  s: 52 words, payment_secret 1111111111111111111111111111111111111111111111111111111111111111
  p: 52 words, payment_hash 0001020304050607080900010203040506070809000102030405060708090102
  d: 20 words, description "1 cup coffee"
  x: 2 words, expiry 60 s
  9: 3 words, features 8 14
  signature recovery ID 1, payee 03e7156ae33b0a208d0744199163177e909e80176e55d97a2f221ede0f934dd9ad (recovered)
  signed: 104 bytes, SHA-256 047e24bf270b25d42a56d57b2578faa3a10684641bab817c2851a871cb41dbc0
a fallback address and two route hints
  mainnet, 2000000000 msat, timestamp 1496314658
  s: 52 words, payment_secret 1111111111111111111111111111111111111111111111111111111111111111
  p: 52 words, payment_hash 0001020304050607080900010203040506070809000102030405060708090102
  h: 52 words, description_hash 3925b6f67e2c340036ed12093dd44e0368df1b6ea26c53dbe4811f58fd5db8c1
  f: 33 words, fallback version 17 1RustyRX2oai4EYYDpQGWvEL62BBGqN9T
  r: 164 words
    hop 029e03a9... over 66051x263430x1800, fee 1 msat + 20 ppm, cltv delta 3
    hop 039e03a9... over 197637x395016x2314, fee 2 msat + 30 ppm, cltv delta 4
  9: 3 words, features 8 14
  signature recovery ID 0, payee 03e7156ae33b0a208d0744199163177e909e80176e55d97a2f221ede0f934dd9ad (recovered)
an amount in pico-bitcoin and fields to skip
  967878534 msat from lnbc9678785340p
  2 unknown fields skipped, 0 fallbacks kept
an invoice we write
  453 characters: lnbcrt1500u1p5sscgqpp5gf0dfe9rdvcw5gdepcsuwykxf85zznpfkl40dqyf6ypecmj48pxqsp5lll0ml8mltul3alk7h608uh37rh7am0va04wn688umj7fclzu8sqdp4gf5hgcm0d9hzq3njdakjq5mrwfshgcmg9ssxx6rpwp6x2u3qxycngxqzjccqzzsfp4pqypqxpq9qcrsszg2pvxq6rs0zqg3yyc5z5tpwxqergd3c8g7rusqrzjqfz0tdqnsddcpxavv07qpxh2kpvumw8mwjujup5fnqzsztca9tdmgrp4qqqqqpcqqyqqqqlgqqqqqeqqjq9qyysgqvg5u8sluuk03m37xjmq9g335vycs6nzs556zdql8r0g8pcah5dwktchrkupd98j7k44tfg77v9k2dnz4qztfjj3pkjqc25ljhqvlkggq5msaat
  decodes: OK
  regtest, 150000000 msat, timestamp 1762156800
  p: 52 words, payment_hash 425ed4e4a36b30ea21b90e21c712c649e8214c29b7eaf68089d1039c6e55384c
  s: 52 words, payment_secret fffefdfcfbfaf9f8f7f6f5f4f3f2f1f0efeeedecebeae9e8e7e6e5e4e3e2e1e0
  d: 53 words, description "Bitcoin From Scratch, chapter 114"
  x: 2 words, expiry 600 s
  c: 2 words, min_final_cltv_expiry_delta 80
  f: 53 words, fallback version 1 bcrt1pqypqxpq9qcrsszg2pvxq6rs0zqg3yyc5z5tpwxqergd3c8g7rusq58nmtx
  r: 82 words
    hop 0244f5b4... over 800000x7x1, fee 1000 msat + 100 ppm, cltv delta 144
  9: 4 words, features 8 14 17
  signature recovery ID 0, payee 03f324133f92f203537cd7f9c4f1cf1078020159111a18457809ce3c11950d87eb (recovered)
  the payee is our node: true
changing it
  one character changed: BAD_BECH32
  150000000000 msat with the checksum made again: OK, payee 02ff1dc25adb6959702ee27431606e75dd8d432394bcca529230762833c3031b43
  is it our node: false
  with an n field: 351 characters, OK
  its amount changed: BAD_SIGNATURE
```

What the output shows:

- All 16 valid examples decode. Fifteen recover the key BOLT 11 signed
  them with; the sixteenth is the first example with s replaced by
  n - s, which BOLT 11 marks valid, and its recovery gives another key.
- All 15 invalid examples refuse, each for the reason its title gives:
  an unknown even feature bit 100, an `n` of 52 words, a missing or
  doubled `p` or `s`, three Bech32 errors, a signature with no key, a
  string too short for a signature, an `x` multiplier, and a `p` amount
  that is not a whole millisatoshi. The last example, a high S with an
  `n` field, also has two `s` fields, which refuses it first.
- The coffee invoice's fields are in the order written: `s`, `p`, `d`,
  `x` and `9`. Its 104 signed bytes hash to the SHA-256 that BOLT 11
  states.
- The route-hint example's `r` field holds two hops of 51 bytes in 164
  words, and its `f` field is the P2PKH address `1RustyRX...`.
- `9678785340p` is 967,878,534 msat. In the example with fields to skip,
  an unknown type 2 and a version 19 fallback are both skipped.
- The invoice we write is 453 characters, decodes, and recovers our
  node's key.
- One changed character fails the checksum. Changing `u` to `m` and
  making the checksum again gives a valid invoice for 1,000 times the
  amount, but the signature now recovers to a stranger's key. A wallet
  that checks the payee catches it; with an `n` field, the reader itself
  refuses it.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-invoice.out](../fixtures/lightning-invoice.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-invoice.coax](../examples/checks/lightning-invoice.coax),
which loads the example:

- each of the 271 characters of the coffee invoice, changed, refuses
- 64 amounts from 1 msat to 18 million bitcoin, the most a `p` amount
  can count in 64 bits, are written and read back
- 0 to 63 bytes survive the trip to words and back
- an invoice with every field type, signed by three nodes, decodes to
  what was written and recovers each node's key

<!-- source: bitcoin/examples/checks/lightning-invoice.coax check -->
```jai
// Any one character of BOLT 11's coffee invoice changed to another
// character of the alphabet refuses: Bech32 finds every single change.
#forall "one changed character refuses" at: 0..<271 #steps 400000000 {
  row := INVOICE_ROWS[1]; copy: [271] u8; inv: [1] Invoice;
  for i: 0..<271 { copy[i] = row.text[i]; }
  copy[at] = ifx copy[at] == 'q' then 'p' else 'q';
  #assert invoice_decode_vartime(cast(string) copy[..], inv[..]) == INV_BAD_BECH32;
}
// The writer's shortest amount text reads back to the same msat, for
// amounts from 1 msat to 18 million bitcoin, the most a p amount can
// count in 64 bits.
#forall "an amount is written and read back" k: 0..<64 {
  msat: u64 = 1; for i: 0..<k { msat = (msat * 3 + cast(u64) (i % 7)) % 1844674407370955161; }
  w: [1] InvoiceWriter;
  #assert invoice_begin_vartime(w[..], INV_TESTNET, true, msat, 0) == INV_OK;
  a := inv_hrp_vartime(w[0].hrp[0..<w[0].hrp_n]);
  #assert a.code == INV_OK && a.has && a.msat == msat && a.network == INV_TESTNET;
}
// Bytes to words and back gives the same bytes, then zero padding.
#forall "bytes survive words" n: 0..<64 {
  b: [64] u8; for i: 0..<n { b[i] = cast(u8) ((37 * i + n) & 255); }
  words: [103] u8; back: [65] u8;
  m := inv_unpack(b[0..<n], words[..]);
  #assert m == (8 * n + 4) / 5;
  #assert inv_pack(words[0..<m], back[..]) >= n;
  for i: 0..<n { #assert back[i] == b[i]; }
}
// An invoice with every field type, written by three nodes, decodes to
// what was written and recovers each node's key.
#forall "every field written is read back" k: 0..<3 #steps 3000000000 {
  node: [32] u8; for i: 0..<32 { node[i] = cast(u8) ((13 * i + k + 1) & 255); }
  id := inv_public(node[..]);
  hash: [32] u8; for i: 0..<32 { hash[i] = cast(u8) (i * k); }
  meta := [3] u8.[1, 2, 3]; f: [7] u8; t: [8] u8; pkh: [20] u8;
  #assert ln_feature_put(f[..], 14) && ln_feature_put(f[..], 49);
  w: [1] InvoiceWriter; inv: [1] Invoice; out: [5000] u8;
  #assert invoice_begin_vartime(w[..], INV_MAINNET, k > 0, cast(u64) (k * 1000), cast(u64) (1700000000 + k)) == INV_OK;
  #assert invoice_put_bytes_vartime(w[..], INV_P, hash[..]) == INV_OK && invoice_put_bytes_vartime(w[..], INV_S, node[..]) == INV_OK;
  #assert invoice_put_bytes_vartime(w[..], INV_D, inv_text("coffee", t[..])) == INV_OK;
  #assert invoice_put_bytes_vartime(w[..], INV_M, meta[..]) == INV_OK && invoice_put_number_vartime(w[..], INV_X, cast(u64) (k + 1)) == INV_OK;
  #assert invoice_put_number_vartime(w[..], INV_C, 40) == INV_OK && invoice_put_fallback_vartime(w[..], 18, pkh[..]) == INV_OK;
  #assert invoice_put_features_vartime(w[..], f[..]) == INV_OK;
  s := invoice_sign_vartime(w[..], node[..], out[..]);
  #assert s.code == INV_OK && invoice_decode_vartime(cast(string) out[0..<s.length], inv[..]) == INV_OK;
  #assert cb_ct_equal(inv[0].payee[..], id[..]) == CB_MAX64 && !inv[0].has_n;
  #assert inv[0].has_amount == (k > 0) && inv[0].timestamp == cast(u64) (1700000000 + k) && inv[0].expiry == cast(u64) (k + 1);
  #assert inv[0].min_final_cltv == 40 && inv[0].metadata_n == 3 && inv[0].fallbacks_n == 1 && inv[0].fields_n == 8;
  #assert ln_feature_set(inv[0].features[0..<inv[0].features_n], 49) && inv[0].features_n == 7;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-invoice.coax --run-steps 250000000
```

`coax test` runs 284 checks, including the loaded files' own checks, in
about 3 minutes on 2026-11-03.

### The library's own checks

`invoice.coax` is an `#authority` file. Its checks cover every BOLT 11
example as valid or invalid, the reason each invalid one refuses, the
signed data, digest, signature and recovery ID of the 11 examples whose
breakdown states them, the payee and payment secret of every valid one,
each valid example written again from its fields to the same text, the
fields of the coffee, hash, fallback, route-hint, pico-bitcoin, feature,
ignored-field and metadata examples, the typed writer giving the coffee
and route-hint invoices, a high S refused under `n`, the shortest amount
text, words and bytes, UTF-8, and a refusal for each field rule.

```shell
coax/bin/coax test coax/lib/lightning/invoice.coax --run-steps 250000000
```

`coax test` runs 278 checks, including those of the files it loads, in
about 3 minutes on 2026-11-03. Mutation testing: not run (owner speed order).

### BOLT 11 and an independent reader

[invoice.py](../../coax/tests/lightning/invoice.py) computes every line
the example prints with
[bolt11.py](../../coax/tests/lightning/bolt11.py), its own Bech32,
invoice reader and writer and recoverable ECDSA with RFC 6979 nonces,
written from BOLT 11 and BIP-173 on
[ln_crypto.py](../../coax/tests/lightning/ln_crypto.py)'s secp256k1,
sharing no code with Coax. Every example of `bolt11/invoice-test.json`
must be valid or invalid as stated, every valid one must be written again
to the same text, and every "Signature breakdown" must match before the
transcript is compared.

```shell
python3 -I coax/tests/lightning/invoice.py --coax coax/bin/coax
```

On 2026-11-03 all 47 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

[make_invoice_vectors.py](../../coax/tests/lightning/make_invoice_vectors.py)
reads the pinned BOLT 11 into
[invoice_vectors.coax](../../coax/tests/lightning/invoice_vectors.coax).

## What this chapter leaves out

- Paying the invoice: chapter 113 finds a route and puts the payment hash,
  secret, amount and CLTV delta in the onion.
- Checking the expiry against a clock, and fetching the description
  that `h` commits to.
- BIP-21 `bitcoin:` URIs with a `lightning=` parameter and the
  `lightning:` URI prefix.
- Feature dependencies beyond `basic_mpp` needing `payment_secret`.
- Reusable payment requests: chapter 115's offers.

## Summary

- An invoice is Bech32 without the 90-character limit: `ln`, a currency,
  an optional amount in bitcoin with a multiplier, then 5-bit words.
- The words are a 35-bit timestamp, tagged fields of type, length and
  data, and a 520-bit recoverable signature.
- A reader refuses a missing or doubled `p` or `s`, a wrong fixed length,
  non-minimal numbers, bad UTF-8 and unknown even features, and skips
  what it does not know.
- The signature covers the human-readable part and every data word; the
  key it recovers to is the payee, unless `n` names it, in which case the
  signature must verify with a low S.
- A writer that signs deterministically reproduces BOLT 11's examples
  exactly.

## Dependencies

- [22. Public key recovery and signed messages](ch022-public-key-recovery-and-signed-messages.md)
- [27. Bech32](ch027-bech32.md)

## Sources

- [BOLT 11](https://github.com/lightning/bolts/blob/master/11-payment-encoding.md),
  "Human-Readable Part", "Data Part", "Tagged Fields", "Feature Bits",
  "Examples" and "Examples of Invalid Invoices", and
  `bolt11/invoice-test.json`, at lightning/bolts revision `19146282`,
  pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/11-payment-encoding.md).
- [BOLT 9](https://github.com/lightning/bolts/blob/master/09-features.md)'s
  features with context 9, at the same revision.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
