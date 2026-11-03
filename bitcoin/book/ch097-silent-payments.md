# 97. Silent payments

A Bitcoin address is meant to be used once. If a donation page shows
one address, everyone who pays it can see every other payment to it,
and the recipient's whole balance. A fresh address per payment needs a
conversation: the payer asks, the recipient answers. BIP-47's payment
codes avoid the conversation but put a notification transaction on the
chain.

Silent payments (BIP-352) need neither. The receiver publishes one
static address, `sp1q...`, which holds two public keys. A sender pays
it to a fresh Taproot output that nobody else can link to the address
or to any other payment. The sender derives the output's key from an
elliptic-curve Diffie-Hellman (ECDH) secret: the sum of the private keys
of its own inputs, times the receiver's scan key. The receiver gets the
same secret from the inputs' public keys, which are on the chain, times
its private scan key. No extra data is published, and the output looks
like any other key-path Taproot output.

The cost moves to the receiver. It has to look at every transaction
with a Taproot output and do one ECDH for each. This chapter builds both
sides, labels and the rule that bounds the receiver's work. Every case
of BIP-352's `send_and_receive_test_vectors.json` gives its published
result.

## What this chapter teaches

- The scan and spend keys, their BIP-32 paths, and the `sp1` address:
  Bech32m with a version, 116 characters, and the rules for versions 1
  to 31.
- Which inputs count: P2TR, P2WPKH, P2SH-P2WPKH and P2PKH, and how a
  receiver reads each one's public key, including from a malleated
  P2PKH scriptSig. A P2TR script path from the NUMS point H doesn't
  count.
- `input_hash`, which binds the secret to the smallest outpoint and to
  the sum of the input keys.
- The ECDH shared secret, the tweaks t_k and the output keys
  B_m + t_k·G.
- Labels, which let one wallet tell payments to different addresses
  apart, and the change label m = 0.
- Scanning, with the per-group limit K_max = 2323.
- Which values are secret, and how the code keeps them out of branches
  and memory.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [silent.coax](../../coax/lib/bitcoin/silent.coax) | `silent_keys` | The scan and spend keys from a seed |
| | `silent_address_vartime`, `silent_decode_vartime` | The address, both ways |
| | `silent_label`, `silent_labeled_spend_vartime` | A label's tweak and point, and the labeled spend key |
| | `silent_input_key_vartime`, `silent_future_witness_vartime`, `silent_outpoint_vartime` | Which inputs count and their keys |
| | `silent_input_hash_vartime` | `input_hash` |
| | `silent_key_sum`, `silent_send` | The sender's key sum a, and the outputs |
| | `silent_tweak_vartime`, `silent_shared` | The receiver's tweak input_hash·A, and the shared secret |
| | `silent_scan` | Scanning a transaction's outputs |
| | `silent_spend_key` | The key that spends a found output |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `b32_encode`, `b32_decode` | Chapter 27's Bech32, with chapter 28's constant |
| [secp_mult_base_web.coax](../../coax/lib/crypto/secp_mult_base_web.coax) | `smb_multiply_web` | Chapter 16's fixed-base k·G |
| [secp_mult_const.coax](../../coax/lib/crypto/secp_mult_const.coax) | `smc_multiply` | Chapter 16's constant-time k·P |
| [bip32.coax](../../coax/lib/crypto/bip32.coax) | `bip32_path` | Chapter 32's derivation |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_witness_vartime` | Chapter 34's witness reader |

## Results

Sending and scanning return an `SL_*` code. Decoding an address
returns an `SLA_*` reason. An input's kind is `SL_IN_NONE` when it
doesn't count.

<!-- source: coax/lib/bitcoin/silent.coax codes -->
```jai
SL_OK :: 0;
SL_NO_INPUTS :: 1;     // no input from BIP-352's list: send nothing, scan nothing
SL_ZERO_SUM :: 2;      // the keys sum to 0: a = 0, A is the point at infinity
SL_HASH_RANGE :: 3;    // input_hash or a t_k is 0 or not below n
SL_GROUP_LIMIT :: 4;   // more than K_max recipients share one scan key
SL_BAD_KEY :: 5;       // a secret key out of range, or a public key not on the curve
SL_CAPACITY :: 6;      // a wrong length, or a buffer too small
SL_REASONS :: [7] string.["ok", "no inputs", "the keys sum to zero", "a hash out of range",
  "a group over K_max", "a bad key", "capacity"];
// Address refusals, in silent_reference.py's order.
SLA_OK :: 0; SLA_NOT_BECH32M :: 1; SLA_HRP :: 2; SLA_VERSION :: 3; SLA_LENGTH :: 4; SLA_POINT :: 5;
SLA_REASONS :: [6] string.["ok", "not Bech32m", "not sp or tsp", "version 31", "not 66 bytes", "not a public key"];
// The kinds of input BIP-352 takes a key from, and 0 for the others.
SL_IN_NONE :: 0; SL_IN_P2PKH :: 1; SL_IN_P2SH_P2WPKH :: 2; SL_IN_P2WPKH :: 3; SL_IN_P2TR :: 4;
SL_KINDS :: [5] string.["none", "P2PKH", "P2SH-P2WPKH", "P2WPKH", "P2TR"];
SL_K_MAX :: 2323;            // outputs per group a sender may make, a receiver looks for
SL_MAX_OUTPUTS :: 23255;     // the most outputs a block's weight allows one transaction
SL_ADDRESS_MAX :: 1023;      // characters, as BIP-352 recommends
SL_ITEMS :: 1003;            // witness items: a 1,000-item stack, a script, a control block, an annex
SL_USED_BYTES :: 2907;       // one bit per output, SL_MAX_OUTPUTS of them
// BIP-341's NUMS point H, as an x coordinate: SHA-256 of G uncompressed.
SL_NUMS :: [32] u8.[0x50,0x92,0x9b,0x74,0xc1,0xa0,0x49,0x54,0xb7,0x8b,0x4b,0x60,0x35,0xe9,0x7a,0x5e,
  0x07,0x8a,0x5a,0x0f,0x28,0xec,0x96,0xd5,0x47,0xbf,0xee,0x9a,0xce,0x80,0x3a,0xc0];
```

## Keys and addresses

A receiver has two key pairs. The scan key b_scan finds payments; the
spend key b_spend spends them. They are separate so that a wallet can
hand b_scan to a server, or keep it on an online machine, while
b_spend stays offline. b_scan alone reveals which outputs are the
wallet's, but can't spend them.

BIP-352 gives them BIP-32 paths under purpose 352. Both are hardened
below the account, so the scan key's extended key doesn't reveal the
spend key:

<!-- source: coax/lib/bitcoin/silent.coax keys -->
```jai
// BIP-352's BIP-32 paths: b_scan at m/352'/coin'/account'/1'/0 and b_spend
// at m/352'/coin'/account'/0'/0, hardened so that the scan key can be
// handed to a scanning server without exposing the spend key.
silent_keys :: (seed: [] u8, coin: int, account: int, scan: [] u8, spend: [] u8) -> bool #export_c {
  if scan.count != 32 || spend.count != 32 || coin < 0 || account < 0 || coin >= BIP32_HARDENED || account >= BIP32_HARDENED { return false; }
  chain: [32] u8;
  path := [5] int.[BIP32_HARDENED + 352, BIP32_HARDENED + coin, BIP32_HARDENED + account, BIP32_HARDENED + 1, 0];
  ok := bip32_path(seed, path[..], scan, chain[..]);
  path[3] = BIP32_HARDENED;
  ok = ok && bip32_path(seed, path[..], spend, chain[..]);
  for i: 0..<32 { chain[i] = 0; }
  return ok;
}
```

The address is Bech32m (chapter 28) with the human-readable part `sp`,
or `tsp` on testnets and signet. Its data is a version, 0 for the
character `q`, then the two compressed public keys B_scan and B_m. B_m
is B_spend for an address with no label. The 66 bytes need 106 5-bit
groups, so an address is 116 characters: longer than BIP-173's limit
of 90, which BIP-352 lifts to the 1,023 that Bech32's checksum is
designed for. `b32_encode` with `extended` set allows that.

<!-- source: coax/lib/bitcoin/silent.coax address -->
```jai
// An address: Bech32m with the human-readable part sp (mainnet) or tsp
// (testnets and signet), then the version (0, the character q) and the
// 66 bytes ser_P(B_scan) || ser_P(B_m) in 5-bit groups: 116 characters on
// mainnet, 117 on a testnet. Returns the length written, or -1.
silent_address_vartime :: (testnet: bool, scan: [] u8, spend: [] u8, out: [] u8) -> int #export_c {
  if scan.count != 33 || spend.count != 33 || !sl_decode_vartime(scan).ok || !sl_decode_vartime(spend).ok { return -1; }
  body: [66] u8; words: [107] u8;
  for i: 0..<33 { body[i] = scan[i]; body[33 + i] = spend[i]; }
  words[0] = 0;
  for i: 0..<106 { words[1 + i] = b32_word(cast(string) body[..], true, i); }
  r := b32_encode(ifx testnet then "tsp" else "sp", cast(string) words[..], false, out, true, false, B32_BECH32M);
  return ifx r.error == NC_OK then r.written else -1;
}
```

The version is how later BIPs will extend the address. A version 0
sender can pay versions 1 to 30: it reads the first 66 bytes and
ignores the rest. Version 31 is kept for a change that a version 0
sender couldn't follow, so it refuses it.

<!-- source: coax/lib/bitcoin/silent.coax decode -->
```jai
SlAddress :: struct { reason: int; testnet: bool; version: int; scan: [33] u8; spend: [33] u8; }
// Decoding, by BIP-352's version rules: version 0 holds exactly 66 bytes;
// versions 1 to 30 hold at least 66, and a version 0 sender reads the
// first 66 and ignores the rest; version 31 is reserved for a change that
// breaks compatibility, and refused. Both keys must be on the curve.
silent_decode_vartime :: (text: string) -> SlAddress #export_c {
  r: SlAddress;
  hrp: [84] u8; words: [SL_ADDRESS_MAX] u8;
  if text.count > SL_ADDRESS_MAX { r.reason = SLA_NOT_BECH32M; return r; }
  d := b32_decode(text, hrp[..], words[..], true, false, B32_BECH32M);
  if d.error != NC_OK { r.reason = SLA_NOT_BECH32M; return r; }
  name := cast(string) hrp[0..<d.hrp_written];
  if name != "sp" && name != "tsp" { r.reason = SLA_HRP; return r; }
  if d.written == 0 { r.reason = SLA_LENGTH; return r; }
  version := cast(int) words[0];
  if version == 31 { r.reason = SLA_VERSION; return r; }
  // 5-bit groups to bytes: the spare bits must be fewer than 5, and zero.
  body: [SL_ADDRESS_MAX] u8; n := 0; acc := 0; bits := 0;
  for i: 1..<d.written {
    acc = ((acc << 5) | cast(int) words[i]) & 0xfff; bits += 5;
    if bits >= 8 { bits -= 8; body[n] = cast(u8) ((acc >> bits) & 255); n += 1; }
  }
  if bits >= 5 || ((acc << (8 - bits)) & 255) != 0 { r.reason = SLA_LENGTH; return r; }
  if n < 66 || (version == 0 && n != 66) { r.reason = SLA_LENGTH; return r; }
  for i: 0..<33 { r.scan[i] = body[i]; r.spend[i] = body[33 + i]; }
  if !sl_decode_vartime(r.scan[..]).ok || !sl_decode_vartime(r.spend[..]).ok { r.reason = SLA_POINT; return r; }
  r.testnet = name == "tsp"; r.version = version;
  return r;
}
```

## Labels

One wallet may want several addresses, for example one per donation
page, to tell the payments apart. Fresh scan keys would multiply the
scanning work. A label changes only the spend key:
B_m = B_spend + hash(b_scan || m)·G. The scanner keeps a table of label
points and recognizes each with one lookup.

Label m = 0 is reserved for change. A wallet that sends from its own
silent payment outputs can send the change to its own address with
label 0, and every BIP-352 wallet scans for label 0. The change is then
found from the seed alone, even if the wallet forgot which labels it
used.

<!-- source: coax/lib/bitcoin/silent.coax label -->
```jai
// A label: the tweak hash_BIP0352/Label(ser_256(b_scan) || ser_32(m)) and
// its point, tweak G. The labeled address uses B_m = B_spend + tweak G.
// m = 0 is reserved for change, which a wallet always scans for.
SlLabel :: struct { m: int; point: [33] u8; tweak: [32] u8; }
silent_label :: (b_scan: [] u8, m: int, out: [] SlLabel, w: [] SlWork) -> int #export_c {
  if b_scan.count != 32 || out.count != 1 || w.count != 1 || m < 0 || m > 0xffffffff { return SL_CAPACITY; }
  for i: 0..<32 { w[0].data[i] = b_scan[i]; }
  sl_put32(w[0].data[32..<36], m);
  sl_tagged(w, "BIP0352/Label", w[0].data[0..<36]);
  t := sl_digest_scalar(w);
  if !t.ok { return sl_done(w, SL_HASH_RANGE); }
  out[0].m = m;
  #assert sc_store(t.value, out[0].tweak[..]);
  #assert sl_encode(smb_multiply_web(t.value), out[0].point[..]);
  t = ScLoad.{};
  return sl_done(w, SL_OK);
}
// B_m = B_spend + the label's point, into out (33 bytes).
silent_labeled_spend_vartime :: (spend: [] u8, label: SlLabel, out: [] u8) -> int #export_c {
  point := label.point;
  b := sl_decode_vartime(spend); l := sl_decode_vartime(point[..]);
  if !b.ok || !l.ok || out.count != 33 { return SL_BAD_KEY; }
  return ifx sl_encode(sg_add(b.value, l.value).value, out) then SL_OK else SL_BAD_KEY;
}
```

The label tweak hashes b_scan, so it is secret, and `silent_label`
computes it with the erasable SHA-256 and the fixed-base comb. The
labeled point is public: it is in the labeled address.

## Which inputs count

The sender sums the private keys of some of its inputs, and the receiver
sums the matching public keys. Both must agree on which inputs, and the
receiver must be able to read each key from the chain. BIP-352 counts
four kinds of input, each with one public key in a known place:

<!-- source: coax/lib/bitcoin/silent.coax inputs -->
```jai
// The public key an input contributes, into out (33 bytes), and its kind;
// SL_IN_NONE when it is not one of BIP-352's inputs or its key can't be
// used. witness is the input's serialized stack (a count, then each item
// with its length), empty for a legacy input.
//
// - P2PKH: the last 33 bytes of the scriptSig whose HASH160 is the key
//   hash. The receiver searches from the end, so a non-standard scriptSig
//   (a third party can malleate one) still gives the key.
// - P2SH-P2WPKH: the scriptSig pushes a P2WPKH program; the key is the
//   last witness item.
// - P2WPKH: the last witness item.
// - P2TR: the output key in the scriptPubKey. The annex (a last item
//   starting 0x50, with another item before it) doesn't count. A script
//   path whose internal key, in the control block, is H is excluded:
//   nobody can sign for H, so such an input can't share the secret.
//
// Only compressed and x-only keys count; an uncompressed key excludes the
// input.
silent_input_key_vartime :: (script_sig: [] u8, witness: [] u8, prevout: [] u8, out: [] u8) -> int #export_c {
  if out.count != 33 { return SL_IN_NONE; }
  ins: [1] TxIn; items: [SL_ITEMS] TxItem; n := 0;
  if witness.count > 0 {
    s := tx_witness_vartime(witness, 0, ins[..], items[..]);
    if s.code != TX_OK || s.at != witness.count { return SL_IN_NONE; }
    n = s.count;
  }
  last := ifx n > 0 then witness[items[n-1].at..<items[n-1].at + items[n-1].n] else witness[0..<0];
  h: [20] u8;
  if prevout.count == 25 && prevout[0] == 0x76 && prevout[1] == 0xa9 && prevout[2] == 0x14 && prevout[23] == 0x88 && prevout[24] == 0xac {
    end := script_sig.count;
    while end >= 33 {
      key := script_sig[end-33..<end];
      #assert hash160_checked(key, h[..], false) == RMD_OK;
      if sl_same_vartime(h[..], prevout[3..<23]) && sl_decode_vartime(key).ok {
        for i: 0..<33 { out[i] = key[i]; }
        return SL_IN_P2PKH;
      }
      end -= 1;
    }
    return SL_IN_NONE;
  }
  kind := SL_IN_NONE;
  if prevout.count == 23 && prevout[0] == 0xa9 && prevout[1] == 0x14 && prevout[22] == 0x87 {
    if script_sig.count == 23 && script_sig[1] == 0 && script_sig[2] == 0x14 { kind = SL_IN_P2SH_P2WPKH; }
  } else if prevout.count == 22 && prevout[0] == 0 && prevout[1] == 0x14 {
    kind = SL_IN_P2WPKH;
  }
  if kind != SL_IN_NONE {
    if n == 0 || !sl_decode_vartime(last).ok { return SL_IN_NONE; }
    for i: 0..<33 { out[i] = last[i]; }
    return kind;
  }
  if prevout.count == 34 && prevout[0] == 0x51 && prevout[1] == 0x20 && n > 0 {
    if n > 1 && last.count > 0 && last[0] == 0x50 { n -= 1; }
    if n > 1 {
      control := witness[items[n-1].at..<items[n-1].at + items[n-1].n];
      nums := SL_NUMS;
      if control.count >= 33 && sl_same_vartime(control[1..<33], nums[..]) { return SL_IN_NONE; }
    }
    p := sg_lift_x_public(prevout[2..<34]);
    if !p.ok { return SL_IN_NONE; }
    out[0] = 2;
    for i: 0..<32 { out[1 + i] = prevout[2 + i]; }
    return SL_IN_P2TR;
  }
  return SL_IN_NONE;
}
// Whether an input spends a witness version above 1 (OP_2 to OP_16 and a
// program of 2 to 40 bytes). A transaction with one is not scanned: a
// later version of silent payments may count those inputs.
silent_future_witness_vartime :: (prevout: [] u8) -> bool #export_c {
  return prevout.count >= 4 && prevout.count <= 42 && prevout[0] >= 0x52 && prevout[0] <= 0x60 && cast(int) prevout[1] == prevout.count - 2;
}
// An outpoint as a transaction serializes it: the txid's 32 bytes as
// stored (the reverse of how it is displayed), then the output index,
// 4 bytes little-endian.
silent_outpoint_vartime :: (txid: [] u8, vout: int, out: [] u8) -> bool #export_c {
  if txid.count != 32 || out.count != 36 || vout < 0 || vout > 0xffffffff { return false; }
  for i: 0..<32 { out[i] = txid[i]; }
  for i: 0..<4 { out[32 + i] = cast(u8) ((vout >> (8 * i)) & 255); }
  return true;
}
```

Two of these rules exist because of collaborative transactions such as
CoinJoins:

- An input with a script, such as a multisig, could be re-signed with
  a different set of keys after the output was derived. BIP-352 leaves
  those inputs out.
- A P2TR input counts by its output key even when it is spent through a
  script path. Otherwise a participant could derive the payment with a
  key-path spend in mind and broadcast a script-path spend instead, and
  the receiver would sum different keys. The one exception is an
  internal key of H, the NUMS point of chapter 41: nobody knows its
  private key, so the sender can't include it.

The P2PKH rule reads the key from the end of the scriptSig, because a
third party can prepend data to a P2PKH scriptSig without breaking its
signature, which doesn't cover the scriptSig. The receiver searches backwards for 33 bytes
whose HASH160 is the key hash.

A transaction that spends a witness version above 1 isn't scanned.
`silent_future_witness_vartime` finds those inputs. A later version of
silent payments may count them, and wallets shouldn't have to scan the
same transaction twice under two rule sets.

## input_hash

If the shared secret depended only on the keys, a sender paying the same
address twice from the same keys would make the same output twice, and
so would anyone else spending coins with those keys. `input_hash` mixes
in the transaction. It hashes the smallest outpoint of all its inputs,
compared byte by byte as serialized, and the key sum A:

<!-- source: coax/lib/bitcoin/silent.coax hash -->
```jai
// input_hash = hash_BIP0352/Inputs(outpoint_L || ser_P(A)), outpoint_L
// the smallest outpoint, byte by byte, of all the transaction's inputs
// (not only BIP-352's). It binds the secret to this transaction: the same
// keys spending other coins give other outputs. Into out (32 bytes);
// false when it is 0 or not below n.
silent_input_hash_vartime :: (outpoints: [] u8, a_point: [] u8, out: [] u8) -> bool #export_c {
  if outpoints.count == 0 || outpoints.count % 36 != 0 || a_point.count != 33 || out.count != 32 { return false; }
  low := 0;
  for k: 1..<outpoints.count / 36 {
    for i: 0..<36 {
      a := outpoints[36*k + i]; b := outpoints[36*low + i];
      if a != b { if a < b { low = k; } break; }
    }
  }
  w: [1] SlWork; data: [69] u8;
  for i: 0..<36 { data[i] = outpoints[36*low + i]; }
  for i: 0..<33 { data[36 + i] = a_point[i]; }
  sl_tagged(w[..], "BIP0352/Inputs", data[..]);
  ok := sl_digest_scalar(w[..]).ok;
  for i: 0..<32 { out[i] = w[0].digest[i]; }
  return ok;
}
```

Any input will do, since an outpoint is spent once. The smallest one
needs no agreement between the parties. A transaction serializes its
outpoints with the txid byte-reversed and the index little-endian, so
the order isn't the displayed one: index 256 (`00010000`) sorts before
index 1 (`01000000`). BIP-352's vector "Outpoint ordering
byte-lexicographically vs. vout-integer" checks this.

## The sender

The sender adds the private keys of the inputs that count. A P2TR
input's key is read as x only, which chapter 41 lifts to the point with
an even y. The receiver sums that even point, so the sender negates each
P2TR key whose point has an odd y. The negation is a masked select on
the parity, not a branch:

<!-- source: coax/lib/bitcoin/silent.coax sum -->
```jai
// a = a_1 + ... + a_n, each a P2TR input's key negated when its point has
// an odd y (the receiver reads that key as x-only, so as the even one).
// keys holds 32 bytes per input of BIP-352's list, taproot says which are
// P2TR. Writes a into out (32 bytes) and A = a G into a_point (33).
silent_key_sum :: (keys: [] u8, taproot: [] bool, out: [] u8, a_point: [] u8, w: [] SlWork) -> int #export_c {
  if w.count != 1 || keys.count % 32 != 0 || taproot.count != keys.count / 32 || out.count != 32 || a_point.count != 33 { return SL_CAPACITY; }
  if keys.count == 0 { return SL_NO_INPUTS; }
  a := sc_small(0);
  for k: 0..<taproot.count {
    d := sc_load(keys[32*k..<32*k+32]);
    if !d.ok || sc_is_zero(d.value) { a = ScScalar.{}; return sl_done(w, SL_BAD_KEY); }
    if taproot[k] {
      #assert sl_encode(smb_multiply_web(d.value), w[0].shared[..]);
      d.value = sc_select(cast(u64) (w[0].shared[0] & 1), sc_negate(d.value), d.value);
    }
    a = sc_add(a, d.value);
    d = ScLoad.{};
  }
  if sc_is_zero(a) { return sl_done(w, SL_ZERO_SUM); }
  #assert sc_store(a, out) && sl_encode(smb_multiply_web(a), a_point);
  a = ScScalar.{};
  return sl_done(w, SL_OK);
}
```

The keys can sum to zero: a key and its negation, as in one of BIP-352's
vectors. Then A is the point at infinity, and sending fails.

The rest is ECDH and a hash. The sender groups the recipients by scan
key. For each group, ecdh_shared_secret = (input_hash·a)·B_scan, by the
constant-time ladder, since a is secret. Each recipient in the group
takes the next k:

<!-- source: coax/lib/bitcoin/silent.coax tk -->
```jai
// t_k = hash_BIP0352/SharedSecret(ser_P(ecdh_shared_secret) || ser_32(k)),
// from the shared secret in w.shared; false when it is 0 or not below n.
sl_t :: (k: int, w: [] SlWork) -> ScLoad {
  for i: 0..<33 { w[0].data[i] = w[0].shared[i]; }
  sl_put32(w[0].data[33..<37], k);
  sl_tagged(w, "BIP0352/SharedSecret", w[0].data[..]);
  return sl_digest_scalar(w);
}
// P_k = B_m + t_k G, as an x-only key into out.
sl_output :: (b_m: SgPoint, t: ScScalar, out: [] u8) -> bool {
  e := sg_encode_public(sg_add(b_m, smb_multiply_web(t)).value);
  for i: 0..<32 { out[i] = e.bytes[1 + i]; }
  return e.length == 33;
}
```

<!-- source: coax/lib/bitcoin/silent.coax send -->
```jai
SlSent :: struct { code: int; count: int; }
// The outputs for recipients, 66 bytes each (B_scan || B_m, as decoded),
// into outputs, 32 bytes each: x-only Taproot keys, in groups by scan
// key, each group in the order its recipients appear. Recipients sharing
// a scan key share one shared secret and take k = 0, 1, 2, ...; a
// recipient listed twice gets two outputs. When shared has room, it gets
// each recipient's shared secret (33 bytes): a secret. Sending fails
// when no input counts, a = 0, input_hash is out of range or a group
// is over K_max.
silent_send :: (keys: [] u8, taproot: [] bool, outpoints: [] u8, recipients: [] u8, outputs: [] u8, shared: [] u8, w: [] SlWork) -> SlSent #export_c {
  r := recipients.count / 66;
  if w.count != 1 || recipients.count % 66 != 0 || r == 0 || outputs.count < 32 * r || (shared.count != 0 && shared.count < 33 * r) {
    return SlSent.{code=SL_CAPACITY};
  }
  a: [32] u8; a_point: [33] u8; h: [32] u8;
  code := silent_key_sum(keys, taproot, a[..], a_point[..], w);
  if code != SL_OK { return SlSent.{code=code}; }
  if !silent_input_hash_vartime(outpoints, a_point[..], h[..]) { sl_wipe(a[..]); return SlSent.{code=SL_HASH_RANGE}; }
  // The groups: a recipient starts one when no earlier recipient has its
  // scan key. Check every group's size before making any output.
  for i: 0..<r {
    first := true;
    for j: 0..<i { if sl_same_vartime(recipients[66*j..<66*j+33], recipients[66*i..<66*i+33]) { first = false; break; } }
    if !first { continue; }
    size := 0;
    for j: i..<r { if sl_same_vartime(recipients[66*j..<66*j+33], recipients[66*i..<66*i+33]) { size += 1; } }
    if size > SL_K_MAX { sl_wipe(a[..]); return SlSent.{code=SL_GROUP_LIMIT}; }
  }
  // Each group: ecdh_shared_secret = (input_hash a) B_scan, then its outputs.
  written := 0;
  for i: 0..<r {
    first := true;
    for j: 0..<i { if sl_same_vartime(recipients[66*j..<66*j+33], recipients[66*i..<66*i+33]) { first = false; break; } }
    if !first { continue; }
    scan := sl_decode_vartime(recipients[66*i..<66*i+33]);
    if !scan.ok { sl_wipe(a[..]); return SlSent.{code=sl_done(w, SL_BAD_KEY)}; }
    s := sc_mul(sc_load(h[..]).value, sc_load(a[..]).value);
    e := smc_multiply(s, scan.value);
    s = ScScalar.{};
    if !sl_encode(e.value, w[0].shared[..]) { sl_wipe(a[..]); return SlSent.{code=sl_done(w, SL_BAD_KEY)}; }
    e = SgResult.{};
    k := 0;
    for j: i..<r {
      if !sl_same_vartime(recipients[66*j..<66*j+33], recipients[66*i..<66*i+33]) { continue; }
      b_m := sl_decode_vartime(recipients[66*j+33..<66*j+66]);
      t := sl_t(k, w);
      if !b_m.ok || !t.ok { sl_wipe(a[..]); return SlSent.{code=sl_done(w, ifx b_m.ok then SL_HASH_RANGE else SL_BAD_KEY)}; }
      if !sl_output(b_m.value, t.value, outputs[32*written..<32*written+32]) { sl_wipe(a[..]); return SlSent.{code=sl_done(w, SL_BAD_KEY)}; }
      t = ScLoad.{};
      if shared.count != 0 { for b: 0..<33 { shared[33*j + b] = w[0].shared[b]; } }
      written += 1; k += 1;
    }
  }
  sl_wipe(a[..]);
  sl_clear(w);
  return SlSent.{code=SL_OK, count=written};
}
```

The counter k runs across a whole group, not per address. If two
labeled addresses of one wallet both took k = 0, the difference of
their outputs would be the difference of their published spend keys,
and an observer could see that both outputs went to that wallet.

The group limit K_max = 2323 bounds the receiver. A transaction can
hold up to 23,255 outputs. If all of them paid one wallet, a scanner
would test each new k against every remaining output: about N² point
comparisons for N outputs, minutes for one block. With at most 2,323
outputs per group, the scanner stops at k = 2323. That is as many P2TR
outputs as fit in a standard 100,000-vbyte transaction, so a standard
transaction never reaches the limit.

## The receiver

The receiver sums the input public keys to get A and computes the tweak
input_hash·A. The tweak needs no secret, so an indexing server can
compute it for every transaction once and send it to light clients:
33 bytes per transaction, from which only the holder of b_scan learns
anything. The shared secret is then b_scan·tweak, the same point as the
sender's (input_hash·a)·B_scan.

<!-- source: coax/lib/bitcoin/silent.coax tweak -->
```jai
// What a receiver needs from a transaction's inputs: A = A_1 + ... + A_n,
// the keys BIP-352's inputs give (33 bytes each), and the tweak
// input_hash A (33 bytes). A server can publish the tweak for every
// transaction; it reveals nothing without b_scan. Writes A too.
silent_tweak_vartime :: (pubs: [] u8, outpoints: [] u8, tweak: [] u8, a_point: [] u8) -> int #export_c {
  if pubs.count % 33 != 0 || tweak.count != 33 || a_point.count != 33 { return SL_CAPACITY; }
  if pubs.count == 0 { return SL_NO_INPUTS; }
  sum := sg_infinity();
  for k: 0..<pubs.count / 33 {
    p := sl_decode_vartime(pubs[33*k..<33*k+33]);
    if !p.ok { return SL_BAD_KEY; }
    sum = sg_add(sum, p.value).value;
  }
  if !sl_encode(sum, a_point) { return SL_ZERO_SUM; }
  h: [32] u8;
  if !silent_input_hash_vartime(outpoints, a_point, h[..]) { return SL_HASH_RANGE; }
  return ifx sl_encode(ecmult_vartime(sc_small(0), sum, sc_load(h[..]).value).value, tweak) then SL_OK else SL_ZERO_SUM;
}
// ecdh_shared_secret = b_scan (input_hash A), the same point the sender
// made as (input_hash a) B_scan, into w.shared. Copies it to out (33
// bytes) when out has room: a secret.
silent_shared :: (b_scan: [] u8, tweak: [] u8, out: [] u8, w: [] SlWork) -> int #export_c {
  if b_scan.count != 32 || w.count != 1 || (out.count != 0 && out.count != 33) { return SL_CAPACITY; }
  p := sl_decode_vartime(tweak);
  b := sc_load(b_scan);
  if !p.ok || !b.ok || sc_is_zero(b.value) { b = ScLoad.{}; return SL_BAD_KEY; }
  e := smc_multiply(b.value, p.value);
  b = ScLoad.{};
  ok := sl_encode(e.value, w[0].shared[..]);
  e = SgResult.{};
  for i: 0..<out.count { out[i] = w[0].shared[i]; }
  return ifx ok then SL_OK else SL_BAD_KEY;
}
```

Scanning computes P_k = B_spend + t_k·G for k = 0, 1, 2, ... and looks
for each among the transaction's Taproot outputs. A labeled output is
P_k + label·G, so output − P_k is a label's point. The output is x-only,
and the label point could be either output − P_k or −output − P_k, so
the scanner tries both. Each match moves k on; the first k with no match
ends the scan. Whether a wallet keeps an output (it may ignore dust,
say) mustn't affect this: the scan continues on any match, or it would
miss the outputs after it.

<!-- source: coax/lib/bitcoin/silent.coax scan -->
```jai
// An output found: its index, its x-only key, the label it was paid to
// (its m, or -1 for none) and the secret tweak that spends it (t_k, plus
// the label's tweak for a labeled one).
SlFound :: struct { output: int; key: [32] u8; label: int; tweak: [32] u8; }
SlScan :: struct { code: int; count: int; }
// The label whose point is p's compressed encoding, or -1.
sl_label_of_vartime :: (p: SgPoint, labels: [] SlLabel) -> int {
  e := sg_encode_public(p);
  if e.length != 33 { return -1; }
  for i: 0..<labels.count { if sl_same_vartime(e.bytes[..], labels[i].point[..]) { return i; } }
  return -1;
}
// Scanning: for k = 0, 1, 2, ... compute P_k = B_spend + t_k G and look
// for it among the outputs not yet found (32 bytes each). An output
// matches P_k itself, or, for a labeled address, P_k + label G: then
// output - P_k, or -output - P_k (an x-only output has two candidate
// points), is a label's point. A match moves on to k + 1; a k with no
// match ends the scan, and so does k = limit. found must have room for
// every output the scan can find.
sl_scan_upto :: (b_scan: [] u8, spend: [] u8, tweak: [] u8, outputs: [] u8, labels: [] SlLabel, found: [] SlFound, limit: int, w: [] SlWork) -> SlScan {
  n := outputs.count / 32;
  if outputs.count % 32 != 0 || n > SL_MAX_OUTPUTS || spend.count != 33 { return SlScan.{code=SL_CAPACITY}; }
  b_spend := sl_decode_vartime(spend);
  if !b_spend.ok { return SlScan.{code=SL_BAD_KEY}; }
  code := silent_shared(b_scan, tweak, b_scan[0..<0], w);
  if code != SL_OK { return SlScan.{code=sl_done(w, code)}; }
  used: [SL_USED_BYTES] u8;
  k := 0;
  while k < limit {
    t := sl_t(k, w);
    if !t.ok { return SlScan.{code=sl_done(w, SL_HASH_RANGE), count=k}; }
    p := sg_add(b_spend.value, smb_multiply_web(t.value)).value;
    e := sg_encode_public(p);
    hit := -1; label := -1;
    for i: 0..<n {
      if ((used[i / 8] >> (i % 8)) & 1) == 1 { continue; }
      out := outputs[32*i..<32*i+32];
      if sl_same_vartime(e.bytes[1..<33], out) { hit = i; break; }
      if labels.count == 0 { continue; }
      o := sg_lift_x_public(out);
      if !o.ok { continue; }
      minus := sg_negate(p).value;
      label = sl_label_of_vartime(sg_add(o.value, minus).value, labels);
      if label < 0 { label = sl_label_of_vartime(sg_add(sg_negate(o.value).value, minus).value, labels); }
      if label >= 0 { hit = i; break; }
    }
    if hit < 0 { t = ScLoad.{}; break; }
    if k == found.count { t = ScLoad.{}; return SlScan.{code=sl_done(w, SL_CAPACITY), count=k}; }
    used[hit / 8] |= cast(u8) (1 << (hit % 8));
    found[k].output = hit;
    found[k].label = ifx label >= 0 then labels[label].m else -1;
    for i: 0..<32 { found[k].key[i] = outputs[32*hit + i]; }
    total := ifx label >= 0 then sc_add(t.value, sc_load(labels[label].tweak[..]).value) else t.value;
    #assert sc_store(total, found[k].tweak[..]);
    total = ScScalar.{}; t = ScLoad.{};
    k += 1;
  }
  sl_clear(w);
  return SlScan.{code=SL_OK, count=k};
}
silent_scan :: (b_scan: [] u8, spend: [] u8, tweak: [] u8, outputs: [] u8, labels: [] SlLabel, found: [] SlFound, w: [] SlWork) -> SlScan #export_c {
  return sl_scan_upto(b_scan, spend, tweak, outputs, labels, found, SL_K_MAX, w);
}
```

The comparison of P_k with an output is variable-time. That's safe:
P_k is either an output on the chain or a point nobody uses.

## Spending

A found output's key is B_spend + t_k·G (+ label·G), so its private key
is b_spend + t_k (+ the label tweak). It spends the output by the key
path, signed as chapter 24 signs. There is no TapTweak: the output key
is d·G itself.

<!-- source: coax/lib/bitcoin/silent.coax spend -->
```jai
// The key that spends a found output: d = b_spend + tweak mod n, into out
// (32 bytes). It signs a BIP-341 key-path spend as it is, with no
// TapTweak: the output key is d G itself. A BIP-340 signer negates d
// when d G has an odd y.
silent_spend_key :: (b_spend: [] u8, tweak: [] u8, out: [] u8) -> bool #export_c {
  if b_spend.count != 32 || tweak.count != 32 || out.count != 32 { return false; }
  b := sc_load(b_spend); t := sc_load(tweak);
  d := sc_add(b.value, t.value);
  ok := b.ok && t.ok && !sc_is_zero(d);
  ok = ok && sc_store(d, out);
  b = ScLoad.{}; t = ScLoad.{}; d = ScScalar.{};
  return ok;
}
```

## Secrets

BIP-352's secret values are the sender's input keys and their sum, the
receiver's b_scan and b_spend, the ECDH point, each t_k, each label
tweak and each spend key. The ECDH point is as sensitive as b_scan for
the one transaction: with it anyone can recognize the outputs. The code
follows chapter 3's rules for all of them:

- Multiplication by G uses chapter 16's fixed-base comb,
  `smb_multiply_web`, which does the same work for every scalar.
  Multiplication of another point uses `smc_multiply`, the fixed
  schedule that chapter 105's Noise handshake uses.
- Tagged hashes of secrets use chapter 24's erasable SHA-256.
- The P2TR negation is a masked select.
- Each procedure that handles a secret keeps it in a caller-owned
  `SlWork` and clears it before returning.

<!-- source: coax/lib/bitcoin/silent.coax work -->
```jai
// Every procedure that touches a secret takes one SlWork and clears it
// before it returns.
SlWork :: struct { sha: [1] ShaOwned; data: [37] u8; digest: [32] u8; shared: [33] u8; }
sl_clear :: (w: [] SlWork) { fresh: SlWork; w[0] = fresh; }
sl_done :: (w: [] SlWork, code: int) -> int { sl_clear(w); return code; }
// hash_tag(data): SHA-256(SHA-256(tag) || SHA-256(tag) || data) into
// w.digest, with the erasable SHA-256.
sl_tagged :: (w: [] SlWork, tag: string, data: [] u8) {
  t: [32] u8; th: [32] u8; s := w[0].sha[..];
  for i: 0..<tag.count { t[i] = tag[i]; }
  #assert so_init(s) && so_update(s, t[0..<tag.count], false) && so_finish(s, th[..], false);
  #assert so_init(s) && so_update(s, th[..], false) && so_update(s, th[..], false);
  #assert so_update(s, data, false) && so_finish(s, w[0].digest[..], false);
}
// The digest as a scalar; ok is false when it is 0 or not below n.
sl_digest_scalar :: (w: [] SlWork) -> ScLoad {
  r := sc_load(w[0].digest[..]);
  r.ok = r.ok && !sc_is_zero(r.value);
  return r;
}
// ser_32(i): four bytes, most significant first.
sl_put32 :: (out: [] u8, i: int) { for j: 0..<4 { out[j] = cast(u8) ((i >> (8 * (3 - j))) & 255); } }
```

The limits are chapter 101's. By-value copies of scalars aren't
erased, and the source schedule doesn't prove the compiled code's
timing. The `_vartime` procedures read only public data: addresses,
scripts, outpoints, A, input_hash, the tweak and the output keys.

## The example

[bitcoin/examples/silent.coax](../examples/silent.coax) makes three
wallets from seeds. It prints the receiver's address, its signet form
and its label 1 address, and the sender's change address. It takes the
inputs of one BIP-352 vector, a P2TR key whose point has an odd y and a
P2PKH key, and pays the receiver twice, to its address and to its label
1 address, with change to the sender's own label 0. Each wallet then
scans the three outputs. Last, it runs every vector.

<!-- source: bitcoin/examples/silent.coax example -->
```jai
#load "../../coax/lib/bitcoin/silent.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [144] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A wallet: its keys from a seed, its address and its labels.
Wallet :: struct { scan: [32] u8; spend: [32] u8; b_scan: [33] u8; b_spend: [33] u8; labels: [2] SlLabel; }
wallet :: (seed: [] u8, w: [] SlWork) -> Wallet {
  r: Wallet;
  if !silent_keys(seed, 0, 0, r.scan[..], r.spend[..]) || !bip32_public(r.scan[..], r.b_scan[..]) || !bip32_public(r.spend[..], r.b_spend[..]) { return r; }
  for m: 0..<2 { if silent_label(r.scan[..], m, r.labels[m..<m+1], w) != SL_OK { return r; } }
  return r;
}
// The address of a wallet, labeled with m or, for m = -1, not; its keys
// (B_scan || B_m) into keys when keys has room.
address :: (x: [] Wallet, m: int, testnet: bool, keys: [] u8, text: [] u8) -> string {
  b_m: [33] u8;
  for i: 0..<33 { b_m[i] = x[0].b_spend[i]; }
  if m >= 0 && silent_labeled_spend_vartime(x[0].b_spend[..], x[0].labels[m], b_m[..]) != SL_OK { return ""; }
  for i: 0..<keys.count { keys[i] = ifx i < 33 then x[0].b_scan[i] else b_m[i - 33]; }
  n := silent_address_vartime(testnet, x[0].b_scan[..], b_m[..], text);
  return ifx n > 0 then cast(string) text[0..<n] else "";
}
// Scans the outputs for a wallet; prints what it finds and whether each
// found key spends its output.
scan :: (name: string, x: [] Wallet, tweak: [] u8, outputs: [] u8, w: [] SlWork) -> int {
  found: [4] SlFound;
  s := silent_scan(x[0].scan[..], x[0].b_spend[..], tweak, outputs, x[0].labels[..], found[..], w);
  print("  % finds %", name, s.count);
  if s.count > 0 { print(":"); }
  spends := 0;
  for k: 0..<s.count {
    d: [32] u8; pub: [33] u8;
    print("% output %", ifx k == 0 then "" else ",", found[k].output);
    if found[k].label >= 0 { print(" (label %)", found[k].label); }
    if silent_spend_key(x[0].spend[..], found[k].tweak[..], d[..]) && bip32_public(d[..], pub[..]) {
      same := true;
      for i: 0..<32 { if pub[1 + i] != found[k].key[i] { same = false; } }
      if same { spends += 1; }
    }
  }
  print("\n");
  return spends;
}

main :: () {
  w: [1] SlWork;
  // A receiver, and a sender that also receives: its change comes back
  // to its own address with the change label m = 0.
  seed: [16] u8;
  for i: 0..<16 { seed[i] = cast(u8) i; }
  wallets: [3] Wallet;
  wallets[0] = wallet(seed[..], w[..]);
  for i: 0..<16 { seed[i] = 0x97; }
  wallets[1] = wallet(seed[..], w[..]);
  for i: 0..<16 { seed[i] = 0x52; }
  wallets[2] = wallet(seed[..], w[..]);
  receiver := wallets[0..<1]; sender := wallets[1..<2]; other := wallets[2..<3];
  recipients: [3 * 66] u8; text: [130] u8;
  print("The receiver (seed 000102...0f, m/352'/0'/0'):\n");
  a := address(receiver, -1, false, recipients[0..<66], text[..]);
  print("  address  % (% characters)\n", a, a.count);
  d := silent_decode_vartime(a);
  print("  signet   %\n", address(receiver, -1, true, recipients[0..<0], text[..]));
  print("  label 1  %\n", address(receiver, 1, false, recipients[66..<132], text[..]));
  print("  decoded: version %, B_scan ", d.version); put_hex(d.scan[..]); print("\n");
  print("The sender's change address (seed 9797...97, label 0):\n  %\n", address(sender, 0, false, recipients[132..<198], text[..]));

  // The inputs of one of BIP-352's cases: a P2TR key whose point has an
  // odd y, and a P2PKH key.
  c := SL_CASES[9];
  print("The inputs (\"%\"):\n", c.comment);
  x := sl_inputs(c);
  for k: 0..<c.inputs {
    v := SL_INPUTS[c.first + k];
    sig: [600] u8; wit: [1200] u8; spk: [60] u8; pub: [33] u8;
    kind := silent_input_key_vartime(tx_unhex(v.script_sig, sig[..]), tx_unhex(v.witness, wit[..]), tx_unhex(v.prevout, spk[..]), pub[..]);
    print("  %: % ", k, SL_KINDS[kind]);
    if kind == SL_IN_NONE { print("\n"); continue; }
    put_hex(pub[..]);
    // The receiver reads a P2TR key as even; this one's secret key gives
    // an odd y, so the sender adds its negation.
    key: [32] u8; own: [33] u8;
    if kind == SL_IN_P2TR && bip32_public(tx_unhex(v.key, key[..]), own[..]) && own[0] == 3 { print(" (odd y: negated)"); }
    print("\n");
  }
  sum: [32] u8; a_point: [33] u8; h: [32] u8; tweak: [33] u8;
  if silent_key_sum(x.keys[0..<32*x.n], x.taproot[0..<x.n], sum[..], a_point[..], w[..]) != SL_OK { return; }
  if !silent_input_hash_vartime(x.outpoints[0..<36*x.all], a_point[..], h[..]) { return; }
  print("  A          "); put_hex(a_point[..]); print("\n");
  print("  input_hash "); put_hex(h[..]); print("\n");

  // Three outputs: two to the receiver (k = 0 and 1), one change (k = 0).
  outputs: [3 * 32] u8; shared: [3 * 33] u8;
  s := silent_send(x.keys[0..<32*x.n], x.taproot[0..<x.n], x.outpoints[0..<36*x.all], recipients[..], outputs[..], shared[..], w[..]);
  if s.code != SL_OK { return; }
  names := [3] string.["to the receiver, k = 0", "to its label 1, k = 1  ", "change, k = 0          "];
  for k: 0..<s.count { print("  % ", names[k]); put_hex(outputs[32*k..<32*k+32]); print("\n"); }

  // Anyone may compute the tweak; each wallet scans with its own b_scan.
  if silent_tweak_vartime(x.pubs[0..<33*x.n], x.outpoints[0..<36*x.all], tweak[..], a_point[..]) != SL_OK { return; }
  print("Scanning with the tweak input_hash A = "); put_hex(tweak[..]); print(":\n");
  spends := scan("the receiver", receiver, tweak[..], outputs[..], w[..]);
  spends += scan("the sender", sender, tweak[..], outputs[..], w[..]);
  spends += scan("another wallet", other, tweak[..], outputs[..], w[..]);
  print("  b_spend + tweak is the secret key of each output found: % of %\n", spends, s.count);

  // Every vector.
  print("The BIP-352 vectors:\n");
  inputs := 0; pass := 0; kmax := 0;
  for each: SL_CASES { inputs += sl_inputs(each).all; }
  print("  inputs: % read, each giving the published key or none\n", inputs);
  for i: 0..<SL_CASES.count { if sl_sends(i) { pass += 1; } }
  print("  sending: % of % cases make a published output set, or fail as published\n", pass, SL_CASES.count);
  pass = 0;
  for r: SL_RECEIVES {
    if sl_receiver_addresses(r) && sl_receiver_finds(r, SL_K_MAX) { pass += 1; }
    if r.n == SL_K_MAX { kmax = r.outputs.count / 64; }
  }
  print("  receiving: % of % find the published outputs, tweaks and signatures;\n", pass, SL_RECEIVES.count);
  print("    one stops at K_max, finding % of its % outputs\n", SL_K_MAX, kmax);
  pass = 0;
  for e: SL_ADDRESSES {
    r := silent_decode_vartime(e.address);
    if r.reason == e.reason { pass += 1; }
  }
  print("  addresses: % of % constructed ones decode as decided\n", pass, SL_ADDRESSES.count);
}
```

```shell
coax/bin/coax run bitcoin/examples/silent.coax
```

It prints:

```text
The receiver (seed 000102...0f, m/352'/0'/0'):
  address  sp1qqwjnwdugzyeqe6xhj7unvjy7aw984l2v08kgcn8aypqthf00tckd6qu3hlqqjy9y523c4vyll6hz9svyjmdlppfwsxh7hdzj99kh859z9vc56rgj (116 characters)
  signet   tsp1qqwjnwdugzyeqe6xhj7unvjy7aw984l2v08kgcn8aypqthf00tckd6qu3hlqqjy9y523c4vyll6hz9svyjmdlppfwsxh7hdzj99kh859z9vvm6kzf
  label 1  sp1qqwjnwdugzyeqe6xhj7unvjy7aw984l2v08kgcn8aypqthf00tckd6qlx4nf6p0pn4lnfzk906ahje3n0rrrd3nvnrzznl9k3sel4sppkdsmdwe86
  decoded: version 0, B_scan 03a537378811320ce8d797b936489eeb8a7afd4c79ec8c4cfd2040bba5ef5e2cdd
The sender's change address (seed 9797...97, label 0):
  sp1qqvdp4leqx5f6kw0syqxl37nmrxxfk2tjw7u3p6dddqv67dmzxem6cqmq4xkpzxm604zd7da7rq9ka9lrmflpd28rhj7m6j3y4965t4pr0qhydq7v
The inputs ("Single recipient: taproot input with odd y-value and non-taproot input"):
  0: P2TR 028c8d23d4764feffcd5e72e380802540fa0f88e3d62ad5e0b47955f74d7b283c4 (odd y: negated)
  1: P2PKH 03e0ec4f64b3fa2e463ccfcf4e856e37d5e1e20275bc89ec1def9eb098eff1f85d
  A          03bc118b1c8178915b716d6137633722c71adfe721551ec7b3938054691de6a2b9
  input_hash 2c648edc10c4f485a825c6cbc12db0966c98c4b05ed13fde156d3647bcd061d9
  to the receiver, k = 0 bde04da346920e14ac17438727fc12ca1798c4cca78a931e6a95b3fcbb258ba5
  to its label 1, k = 1   584d614148a9d9ecf3e7214c8b624bf8684e52692eb5800eb6cacfbec8f9b411
  change, k = 0           7dc18cbd165253db2970c6efdbfc7d05035679995b3f2af5ddea57cd437d472b
Scanning with the tweak input_hash A = 02d4e4f2c4cdb71c9c39a700a9ee1a0fc05b98362a441183f5770af7d6e2b3038c:
  the receiver finds 2: output 0, output 1 (label 1)
  the sender finds 1: output 2 (label 0)
  another wallet finds 0
  b_spend + tweak is the secret key of each output found: 3 of 3
The BIP-352 vectors:
  inputs: 62 read, each giving the published key or none
  sending: 29 of 29 cases make a published output set, or fail as published
  receiving: 30 of 30 find the published outputs, tweaks and signatures;
    one stops at K_max, finding 2323 of its 2324 outputs
  addresses: 15 of 15 constructed ones decode as decided
```

The receiver finds both of its outputs, k = 0 and k = 1, the second
through its label 1. The sender finds its change through label 0, and
the third wallet finds nothing. The P2TR input's key shows as `02...`:
the receiver reads it as even. Its secret key's point has an odd y, so
the sender negated it.

The K_max vector pays one wallet 2,324 outputs, all to its change
label. The sender refuses to make them. The receiver, given all 2,324,
stops after 2,323.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/silent.coax](../examples/checks/silent.coax),
which loads the example. For six seeds, two inputs pay a wallet's
address and its label 1 address. With a P2TR first input whose point has
an odd y for three seeds and an even y for one, the receiver still
finds both outputs. Each found key is its output's secret key, another
wallet finds nothing, and the inputs in the other order make the same
outputs.

<!-- source: bitcoin/examples/checks/silent.coax check -->
```jai
// A wallet whose two keys hash from seed: the chapter's BIP-32 paths cost
// more than these checks need.
hashed_wallet :: (seed: [] u8, w: [] SlWork) -> Wallet {
  r: Wallet; data: [17] u8;
  for i: 0..<16 { data[i] = seed[i]; }
  data[16] = 1; #assert sha256_hash_checked(data[..], r.scan[..], false) == SHA_OK && bip32_public(r.scan[..], r.b_scan[..]);
  data[16] = 2; #assert sha256_hash_checked(data[..], r.spend[..], false) == SHA_OK && bip32_public(r.spend[..], r.b_spend[..]);
  for m: 0..<2 { #assert silent_label(r.scan[..], m, r.labels[m..<m+1], w) == SL_OK; }
  return r;
}
// A payment from two inputs of seed s, the first P2TR when taproot, to a
// wallet's address and its label 1. The wallet finds both outputs, the
// second as label 1, and each found key is its output's secret key.
// Another wallet finds nothing. The inputs in the other order make the
// same outputs: A is a sum, and the smallest outpoint is the same.
pays :: (s: int, taproot: bool) -> bool {
  w: [1] SlWork; seed: [16] u8; wallets: [2] Wallet; recipients: [132] u8; text: [130] u8;
  for i: 0..<16 { seed[i] = cast(u8) ((s * 7 + i) & 255); }
  wallets[0] = hashed_wallet(seed[..], w[..]);
  seed[0] ^= 0xff;
  wallets[1] = hashed_wallet(seed[..], w[..]);
  if address(wallets[0..<1], -1, false, recipients[0..<66], text[..]).count != 116 { return false; }
  if address(wallets[0..<1], 1, false, recipients[66..<132], text[..]).count != 116 { return false; }
  keys: [64] u8; flags: [2] bool; outpoints: [72] u8; outputs: [64] u8;
  #assert sha256_hash_checked(seed[..], keys[0..<32], false) == SHA_OK && sha256_hash_checked(keys[0..<32], keys[32..<64], false) == SHA_OK;
  flags[0] = taproot;
  for i: 0..<72 { outpoints[i] = cast(u8) ((s * 13 + i * 5) & 255); }
  sent := silent_send(keys[..], flags[..], outpoints[..], recipients[..], outputs[..], outputs[0..<0], w[..]);
  if sent.code != SL_OK || sent.count != 2 { return false; }
  // What the receiver sees: each input's key, a P2TR one as x only.
  pubs: [66] u8; tweak: [33] u8; a_point: [33] u8; found: [2] SlFound;
  for k: 0..<2 { #assert bip32_public(keys[32*k..<32*k+32], pubs[33*k..<33*k+33]); }
  if taproot { pubs[0] = 2; }
  if silent_tweak_vartime(pubs[..], outpoints[..], tweak[..], a_point[..]) != SL_OK { return false; }
  r := silent_scan(wallets[0].scan[..], wallets[0].b_spend[..], tweak[..], outputs[..], wallets[0].labels[..], found[..], w[..]);
  ok := r.code == SL_OK && r.count == 2 && found[0].output == 0 && found[0].label == -1 && found[1].output == 1 && found[1].label == 1;
  for k: 0..<r.count {
    d: [32] u8; pub: [33] u8;
    ok = ok && silent_spend_key(wallets[0].spend[..], found[k].tweak[..], d[..]) && bip32_public(d[..], pub[..]);
    for i: 0..<32 { ok = ok && pub[1 + i] == outputs[32*k + i]; }
  }
  r = silent_scan(wallets[1].scan[..], wallets[1].b_spend[..], tweak[..], outputs[..], wallets[1].labels[..], found[..], w[..]);
  ok = ok && r.code == SL_OK && r.count == 0;
  // The other order.
  swapped: [64] u8; flipped: [2] bool; moved: [72] u8; again: [64] u8;
  for i: 0..<32 { swapped[i] = keys[32 + i]; swapped[32 + i] = keys[i]; }
  for i: 0..<36 { moved[i] = outpoints[36 + i]; moved[36 + i] = outpoints[i]; }
  flipped[1] = taproot;
  sent = silent_send(swapped[..], flipped[..], moved[..], recipients[..], again[..], again[0..<0], w[..]);
  for i: 0..<64 { ok = ok && again[i] == outputs[i]; }
  return ok && sent.code == SL_OK;
}
// One check per input type: each check has its own budget.
#forall "two inputs pay a wallet and its label; it finds both and can spend them" s: 0..<2 #steps 20000000000 {
  #assert pays(s, false);
}
#forall "with a P2TR input whose y is odd or even, the receiver finds the same" s: 2..<4 #steps 20000000000 {
  #assert pays(s, true);
}
#forall "the same, for two more P2TR inputs" s: 4..<6 #steps 20000000000 {
  #assert pays(s, true);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/silent.coax
```

`coax test` runs 446 checks, including the loaded files' own checks, on 2026-11-03.

### The library's own checks

`silent.coax` is an `#authority` file. Its checks run every case of
BIP-352's `send_and_receive_test_vectors.json`:

- All 62 inputs give the published public key, or none: the malleated
  P2PKH, the uncompressed P2PKH and P2WPKH keys, the invalid P2SH
  inputs, the NUMS script paths and the annex.
- All 38 recipient addresses decode to their published keys, and each
  of the 30 receivers' addresses, unlabeled and labeled, is the
  published one.
- Each of the 29 senders has the published key sum a, makes the
  published shared secret and one of the published output sets, or
  fails as published: no inputs, keys that sum to zero, and a group of
  2,324 over K_max.
- Each receiver has the published A, tweak and shared secret, finds the
  published outputs with their tweaks, and signs with each found key
  the BIP-340 signature the vectors publish.

The K_max receiver's full scan, 2,323 rounds, is too slow for the
checker; the example runs it natively. The checks run that scan
stopped at 3, which finds the first 3 outputs, and show that output
2,323 is P_2323 with the change label, so that only the limit stops it.
Fifteen constructed addresses cover the version rules: versions 1 and
30 with extra bytes, version 31, the wrong length for versions 0 and 1,
a Bech32 checksum, the `bc` prefix, mixed case, a changed character, a
key off the curve and an address over 1,023 characters. Other checks
cover inputs the vectors don't (a P2TR witness with trailing bytes,
future witness versions) and the refusals.

```shell
coax/bin/coax test coax/lib/bitcoin/silent.coax
```

`coax test` runs 441 checks, including those of the files it loads, on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[silent_reference.py](../../coax/tests/bitcoin/silent_reference.py)
is BIP-352 in Python, written from the specification with its own
curve arithmetic and Bech32m.
[make_silent_vectors.py](../../coax/tests/bitcoin/make_silent_vectors.py)
decides every published case with it, checks each result against the
published one, builds the constructed addresses and writes
`silent_vectors.coax`. [silent.py](../../coax/tests/bitcoin/silent.py)
runs BIP-352's own `reference.py` on the pinned vectors, with the
secp256k1lab it vendors, after checking every file's hash. It then
writes the transcript the example must print.

```shell
python3 -I coax/tests/bitcoin/make_silent_vectors.py
python3 -I coax/tests/bitcoin/silent.py --coax coax/bin/coax
```

On 2026-11-03 BIP-352's reference passes all 29 cases, every case agrees with `silent_reference.py`, all 26 lines of the transcript agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Building and signing the transaction. The outputs are ordinary P2TR
  outputs, and a found output is spent by an ordinary key path; chapters
  40, 43 and 94 cover both.
- Collecting tweaks from a node, BIP-158 filters for light clients
  (chapter 74), and the rest of BIP-352's appendix on light clients.
- Senders that share inputs, such as CoinJoins with MuSig2 or FROST
  inputs. They need each party's share of the ECDH and a proof that it
  is honest, which BIP-352 leaves open.
- The DLEQ proofs of BIP-374, which let a hardware signer prove its
  share of the ECDH.

## Summary

- A silent payment address holds a scan key and a spend key. Each
  payment goes to a fresh Taproot output only the receiver can find.
- The sender sums its input keys; the receiver sums the matching public
  keys from the chain. `input_hash` binds the secret to the
  transaction's smallest outpoint.
- Output k of a group is B_m + t_k·G, with t_k a hash of the ECDH
  secret and k. Labels change B_m, and label 0 is change.
- The receiver pays with scanning: one ECDH per transaction, bounded by
  K_max per group.

## Dependencies

- [16. Scalar multiplication](ch016-scalar-multiplication.md)
- [24. Sign with Schnorr](ch024-sign-with-schnorr.md)
- [28. Bech32m](ch028-bech32m.md)
- [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](ch029-addresses.md)
- [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md)
- [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md)

## Sources

- [BIP-352](https://github.com/bitcoin/bips/blob/master/bip-0352.mediawiki)
  (Silent Payments), version 1.1.2, with its
  `send_and_receive_test_vectors.json`, its `reference.py` and the
  secp256k1lab 1.0.0 it vendors, pinned at revision `5aadddb5` in
  [coax/tests/vectors/upstream/bip352/](../../coax/tests/vectors/upstream/bip352/bip-0352.mediawiki).
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  (Schnorr signatures),
  [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
  (Taproot) and
  [BIP-350](https://github.com/bitcoin/bips/blob/master/bip-0350.mediawiki)
  (Bech32m).
- Ruben Somsen, "Silent Payments", 2022, the original proposal,
  [gist](https://gist.github.com/RubenSomsen/c43b79517e7cb701ebf77eec6dbb46b8).
