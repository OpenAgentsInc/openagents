# 108. Commitment and HTLC transactions

A Lightning channel is one Bitcoin output that both peers must sign to
spend. Each peer holds a signed transaction that spends it, its
*commitment*, which pays out the channel's current balances and every
payment still in flight. Either peer can broadcast its commitment at any
time, so the channel is always as safe as the latest pair of
commitments.

This chapter builds BOLT 3's transactions: the funding output, the
commitment with its `to_local`, `to_remote`, HTLC and anchor outputs, and
the HTLC-timeout and HTLC-success transactions that spend the HTLC
outputs. Every commitment and HTLC transaction in BOLT 3's test vectors
comes out byte for byte, signed by both peers.

## What this chapter teaches

- The funding output: a P2WSH of a 2-of-2 with the two funding keys
  sorted.
- The commitment transaction, and how it hides the commitment number in
  its locktime and sequence.
- `to_local`, which pays the owner after `to_self_delay` blocks or the
  peer at once with the revocation key; `to_remote`, as P2WPKH or, with
  `option_anchors`, behind a one-block delay; and the two 330-satoshi
  anchors.
- The offered and received HTLC scripts, and the HTLC-timeout and
  HTLC-success transactions that spend them.
- Fees by expected weight, dust trimming by feerate, and BIP-69 output
  order with BOLT 3's expiry tiebreak.
- Signing a commitment and its HTLC transactions, including
  `SIGHASH_SINGLE|SIGHASH_ANYONECANPAY` under `option_anchors`.

## Where Lightning uses these transactions

| Where | What it uses | Chapter |
| --- | --- | --- |
| Opening a channel | the funding output, the first commitment's signatures | 109 |
| Each update | a new commitment and its HTLC signatures in `commitment_signed` | 109 |
| Closing alone | broadcasting the commitment, then the HTLC transactions | 110 |
| Penalties | spending a revoked commitment's outputs with the revocation key | 110 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [commitment.coax](../../coax/lib/lightning/commitment.coax) | `lnc_funding_script_vartime(a, b, out)` | the 2-of-2 funding script |
| | `lnc_obscured_vartime(number, opener, accepter)` | the obscured commitment number |
| | `lnc_to_local_script_vartime`, `lnc_to_remote_script_vartime`, `lnc_anchor_script_vartime` | the balance and anchor scripts |
| | `lnc_offered_script_vartime`, `lnc_received_script_vartime` | the HTLC scripts |
| | `lnc_htlc_fee`, `lnc_trimmed`, `lnc_commit_fee` | fees and trimming |
| | `lnc_commitment_vartime(channel, to_local_msat, to_remote_msat, feerate, htlcs)` | a commitment's outputs, sorted |
| | `lnc_unsigned_vartime`, `lnc_sighash_vartime`, `lnc_signed_vartime` | the commitment, its hash and its witness |
| | `lnc_htlc_unsigned_vartime`, `lnc_htlc_sighash_vartime`, `lnc_htlc_signed_vartime` | the HTLC transactions |
| [keys.coax](../../coax/lib/lightning/keys.coax) | `lnk_pubkey_vartime`, `lnk_revocation_pubkey_vartime` | the commitment's keys (chapter 106) |
| [multisig.coax](../../coax/lib/bitcoin/multisig.coax) | `multisig_script_vartime`, `ms_number` | the sorted 2-of-2 and script numbers (chapter 100) |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sighash_v0` | BIP-143 signature hashes (chapter 38) |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_sign`, `ecdsa_der` | RFC 6979 signatures in DER (chapters 20 and 21) |

Keys, amounts, scripts and transactions are public: peers exchange them,
and a commitment is published when it is broadcast. The procedures that
build them branch on public data and end in `_vartime` (chapter 3). The
signing keys are secret, and they go only to chapter 20's constant-time
signer.

The library's codes, limits and output kinds:

<!-- source: coax/lib/lightning/commitment.coax codes -->
```jai
LNC_OK :: 0;
LNC_BAD_KEY :: 1;          // a key that is not a compressed point
LNC_TOO_MANY :: 2;         // more HTLCs than LNC_MAX_HTLCS
LNC_BAD_AMOUNT :: 3;       // a negative amount, or more than the funding output holds
LNC_NOT_HTLC :: 4;         // an output index that is not an HTLC output
LNC_SIGN :: 5;             // the signer refused the key
LNC_CAPACITY :: 6;         // the caller's buffer is too small

LNC_MAX_HTLCS :: 8;        // HTLCs in one commitment here; BOLT 2 allows 483 each way
LNC_MAX_OUTPUTS :: 12;     // the HTLCs, to_local, to_remote and two anchors
LNC_SCRIPT :: 160;         // the longest witness script: a received HTLC with anchors, 143 bytes
LNC_TX :: 1024;            // the longest signed commitment
LNC_HTLC_TX :: 512;        // the longest signed HTLC transaction
LNC_DER :: 72;             // the longest DER signature
LNC_ANCHOR_SAT :: 330;     // an anchor output: P2WSH's dust limit

// Output kinds.
LNC_TO_LOCAL :: 0; LNC_TO_REMOTE :: 1; LNC_LOCAL_ANCHOR :: 2; LNC_REMOTE_ANCHOR :: 3;
LNC_OFFERED :: 4; LNC_RECEIVED :: 5;
LNC_KINDS :: [6] string.["to_local", "to_remote", "local_anchor", "remote_anchor", "offered", "received"];
```

## The channel and its outputs

A commitment belongs to one peer, its *owner*. The owner's balance is
`to_local` and the peer's is `to_remote`. An HTLC the owner sent is
*offered*, and one it was sent is *received*. Each peer holds its own
commitment, so the two commitments of one state mirror each other:
Alice's offered HTLC is a received HTLC on Bob's commitment.

`LncChannel` holds what one commitment needs: the funding outpoint, the
keys chapter 106 derives for this commitment, the owner's delay and dust
limit, the obscured commitment number and the channel type.

<!-- source: coax/lib/lightning/commitment.coax channel -->
```jai
// An HTLC as the commitment's owner sees it: offered by the owner, or
// received from the peer.
LncHtlc :: struct { offered: bool; amount_msat: int; expiry: int; payment_hash: [32] u8; }
// One side's commitment of a channel: the funding outpoint, the keys of
// this commitment (chapter 106), the owner's to_self_delay and dust limit,
// the obscured commitment number, and the channel type.
LncChannel :: struct {
  funding_txid: [32] u8;       // hash order, as an outpoint holds it
  funding_index: int; funding_sat: int;
  local_funding: [33] u8; remote_funding: [33] u8;
  revocation: [33] u8;         // revocationpubkey
  local_delayed: [33] u8;      // local_delayedpubkey
  remote: [33] u8;             // remotepubkey
  local_htlc: [33] u8; remote_htlc: [33] u8;
  to_self_delay: int; dust_limit: int;
  obscured: int;               // the commitment number XOR the 48-bit mask
  anchors: bool;               // option_anchors
  local_funder: bool;          // the owner opened the channel and pays the fee
}
// One output of a commitment. htlc indexes the caller's HTLCs.
LncOutput :: struct { sat: int; spk: [34] u8; spk_n: int; kind: int; htlc: int; expiry: int; }
// A commitment's outputs in BIP-69 order, the base fee, and how many HTLCs
// are not trimmed.
LncCommit :: struct { code: int; n: int; out: [LNC_MAX_OUTPUTS] LncOutput; fee: int; untrimmed: int; }
```

## The funding output and the commitment number

The funding output is a P2WSH of `2 <pubkey1> <pubkey2> 2
OP_CHECKMULTISIG`, with the lesser key first. That is chapter 100's
sorted multisig, so either peer can rebuild it from the two keys alone.

A commitment spends the funding output and hides its 48-bit commitment
number. The number is XORed with the lower 48 bits of
SHA256(opener's `payment_basepoint` || accepter's `payment_basepoint`).
The upper 24 bits go into the input's sequence after a `0x80` byte, and
the lower 24 bits into the locktime after a `0x20` byte. Locktime
`0x20xxxxxx` is a time in 1987, so it never delays the transaction, and a
sequence with the top bit set turns off relative locktimes. Only the two
peers know the mask, so only they can read the number of a commitment
broadcast on chain and find which one it was.

<!-- source: coax/lib/lightning/commitment.coax funding -->
```jai
// 2 <pubkey1> <pubkey2> 2 OP_CHECKMULTISIG with the lesser key first:
// chapter 100's sorted 2-of-2. Its P2WSH is the funding output.
lnc_funding_script_vartime :: (a: [] u8, b: [] u8, out: [] u8) -> int {
  if !lnc_key_ok(a) || !lnc_key_ok(b) { return -LNC_BAD_KEY; }
  keys: [66] u8; for i: 0..<33 { keys[i] = a[i]; keys[33 + i] = b[i]; }
  return multisig_script_vartime(2, keys[..], 2, true, out);
}
// The commitment number XOR the lower 48 bits of SHA256(the opener's
// payment_basepoint || the accepter's).
lnc_obscured_vartime :: (number: int, opener: [] u8, accepter: [] u8) -> int {
  joined: [66] u8; h: [32] u8;
  for i: 0..<33 { joined[i] = opener[i]; joined[33 + i] = accepter[i]; }
  #assert sha256_hash_checked(joined[..], h[..], false) == SHA_OK;
  mask := 0;
  for i: 26..<32 { mask = (mask << 8) | cast(int) h[i]; }
  return number ^ mask;
}
```

## The scripts

`to_local` pays the owner, but only after `to_self_delay` blocks
(`OP_CHECKSEQUENCEVERIFY`). Until then the peer can take it with the
revocation key, which the peer can sign for only once the owner has
revoked this commitment (chapter 106). So an owner who broadcasts an old
commitment loses its whole balance.

`to_remote` pays the peer, who never needs a delay: it is a P2WPKH to
`remotepubkey`. With `option_anchors` it is a P2WSH that adds `1
OP_CHECKSEQUENCEVERIFY`, so the peer cannot spend it in the same block,
and every output except the anchors is delayed. Each anchor locks 330
satoshis to one peer's funding key. Its owner spends it at once to bump
the commitment's fee with child-pays-for-parent. Anyone can sweep it
after 16 blocks, so unspent anchors do not stay in the UTXO set.

An HTLC output has three ways out. The revocation key takes it at once
on a revoked commitment. The peer takes an offered HTLC with the payment
preimage, or a received one after `cltv_expiry`. The owner's own branch
is a 2-of-2 of both HTLC keys: it spends through an HTLC transaction the
peer signed in advance, so the owner's funds still pass through the
`to_self_delay`. `OP_SIZE 32 OP_EQUAL` tells the branches apart: a
32-byte item is a preimage. With `option_anchors`, both HTLC scripts end
with `1 OP_CHECKSEQUENCEVERIFY OP_DROP` on the peer's branches.

<!-- source: coax/lib/lightning/commitment.coax scripts -->
```jai
// to_local and the output of an HTLC transaction:
//   OP_IF <revocationpubkey> OP_ELSE <to_self_delay> OP_CHECKSEQUENCEVERIFY
//   OP_DROP <local_delayedpubkey> OP_ENDIF OP_CHECKSIG
lnc_to_local_script_vartime :: (revocation: [] u8, delay: int, delayed: [] u8, out: [] u8) -> int {
  at := lnc_ops(out, 0, "63");                  // OP_IF
  at = lnc_push(out, at, revocation);
  at = lnc_ops(out, at, "67");                  // OP_ELSE
  at = ms_number(delay, out, at);
  at = lnc_ops(out, at, "b275");                // OP_CHECKSEQUENCEVERIFY OP_DROP
  at = lnc_push(out, at, delayed);
  return lnc_ops(out, at, "68ac");              // OP_ENDIF OP_CHECKSIG
}
// to_remote with option_anchors: <remotepubkey> OP_CHECKSIGVERIFY 1 OP_CHECKSEQUENCEVERIFY.
lnc_to_remote_script_vartime :: (remote: [] u8, out: [] u8) -> int {
  return lnc_ops(out, lnc_push(out, 0, remote), "ad51b2");
}
// An anchor: <funding_pubkey> OP_CHECKSIG OP_IFDUP OP_NOTIF OP_16
// OP_CHECKSEQUENCEVERIFY OP_ENDIF. Its owner spends it at once; anyone may
// after 16 blocks.
lnc_anchor_script_vartime :: (funding: [] u8, out: [] u8) -> int {
  return lnc_ops(out, lnc_push(out, 0, funding), "ac736460b268");
}
// The start both HTLC scripts share: the revocation branch, then the
// remote HTLC key and a test of the top item's size.
lnc_htlc_head :: (ch: LncChannel, out: [] u8) -> int {
  c := ch; h: [20] u8;
  #assert hash160_checked(c.revocation[..], h[..], false) == RMD_OK;
  at := lnc_ops(out, 0, "76a9");                // OP_DUP OP_HASH160
  at = lnc_push(out, at, h[..]);                // RIPEMD160(SHA256(revocationpubkey))
  at = lnc_ops(out, at, "8763ac67");            // OP_EQUAL OP_IF OP_CHECKSIG OP_ELSE
  at = lnc_push(out, at, c.remote_htlc[..]);
  return lnc_ops(out, at, "7c82012087");        // OP_SWAP OP_SIZE 32 OP_EQUAL
}
lnc_htlc_tail :: (ch: LncChannel, out: [] u8, at: int) -> int {
  n := ifx ch.anchors then lnc_ops(out, at, "51b275") else at;   // 1 OP_CHECKSEQUENCEVERIFY OP_DROP
  return lnc_ops(out, n, "68");                 // OP_ENDIF
}
// An offered HTLC: the remote node takes it with the preimage; the owner
// takes it back through an HTLC-timeout transaction both signed.
lnc_offered_script_vartime :: (ch: LncChannel, payment_hash: [] u8, out: [] u8) -> int {
  c := ch; h: [20] u8;
  at := lnc_htlc_head(c, out);
  at = lnc_ops(out, at, "6475527c");            // OP_NOTIF OP_DROP 2 OP_SWAP
  at = lnc_push(out, at, c.local_htlc[..]);
  at = lnc_ops(out, at, "52ae67a9");            // 2 OP_CHECKMULTISIG OP_ELSE OP_HASH160
  #assert ripemd160_hash_checked(payment_hash, h[..], false) == RMD_OK;
  at = lnc_push(out, at, h[..]);                // RIPEMD160(payment_hash)
  at = lnc_ops(out, at, "88ac68");              // OP_EQUALVERIFY OP_CHECKSIG OP_ENDIF
  return lnc_htlc_tail(c, out, at);
}
// A received HTLC: the owner takes it with the preimage through an
// HTLC-success transaction; the remote node takes it back after
// cltv_expiry.
lnc_received_script_vartime :: (ch: LncChannel, payment_hash: [] u8, expiry: int, out: [] u8) -> int {
  c := ch; h: [20] u8;
  at := lnc_htlc_head(c, out);
  at = lnc_ops(out, at, "63a9");                // OP_IF OP_HASH160
  #assert ripemd160_hash_checked(payment_hash, h[..], false) == RMD_OK;
  at = lnc_push(out, at, h[..]);
  at = lnc_ops(out, at, "88527c");              // OP_EQUALVERIFY 2 OP_SWAP
  at = lnc_push(out, at, c.local_htlc[..]);
  at = lnc_ops(out, at, "52ae6775");            // 2 OP_CHECKMULTISIG OP_ELSE OP_DROP
  at = ms_number(expiry, out, at);
  at = lnc_ops(out, at, "b175ac68");            // OP_CHECKLOCKTIMEVERIFY OP_DROP OP_CHECKSIG OP_ENDIF
  return lnc_htlc_tail(c, out, at);
}
// The witness script of an output; 0 for a P2WPKH to_remote, which has none.
lnc_witness_script_vartime :: (ch: LncChannel, htlcs: [] LncHtlc, o: LncOutput, out: [] u8) -> int {
  c := ch;
  if o.kind == LNC_TO_LOCAL { return lnc_to_local_script_vartime(c.revocation[..], c.to_self_delay, c.local_delayed[..], out); }
  if o.kind == LNC_TO_REMOTE { return ifx c.anchors then lnc_to_remote_script_vartime(c.remote[..], out) else 0; }
  if o.kind == LNC_LOCAL_ANCHOR { return lnc_anchor_script_vartime(c.local_funding[..], out); }
  if o.kind == LNC_REMOTE_ANCHOR { return lnc_anchor_script_vartime(c.remote_funding[..], out); }
  h := htlcs[o.htlc];
  if o.kind == LNC_OFFERED { return lnc_offered_script_vartime(c, h.payment_hash[..], out); }
  return lnc_received_script_vartime(c, h.payment_hash[..], h.expiry, out);
}
```

## Fees and trimming

The funder pays the commitment's fee. BOLT 3 fixes it from an *expected
weight*, so both peers compute the same fee without knowing the final
signature lengths: 724 weight units, plus 172 for each HTLC output, at
`feerate_per_kw` satoshis per 1,000 weight units. With `option_anchors`
the base is 1,124 and the funder also pays both anchors.

An output below the owner's `dust_limit_satoshis` is left out, and its
amount goes to the fee. For an HTLC, what counts is what its HTLC
transaction would pay out: the amount less that transaction's fee at
663 (timeout) or 703 (success) weight units. So as the feerate rises,
the smaller HTLCs drop out one by one. With `option_anchors` the HTLC
transactions pay no fee of their own, so only the dust limit decides.

<!-- source: coax/lib/lightning/commitment.coax fees -->
```jai
// BOLT 3's expected weights: 724 for a commitment (1,124 with anchors)
// plus 172 per untrimmed HTLC; 663 for HTLC-timeout and 703 for
// HTLC-success. With anchors the HTLC transactions pay no fee: they are
// signed SIGHASH_SINGLE|SIGHASH_ANYONECANPAY and the owner adds inputs.
lnc_htlc_fee :: (feerate: int, offered: bool, anchors: bool) -> int {
  if anchors { return 0; }
  return feerate * (ifx offered then 663 else 703) / 1000;
}
// An HTLC is trimmed when what its HTLC transaction would pay out is below
// the owner's dust limit; its amount goes to the fee.
lnc_trimmed :: (h: LncHtlc, feerate: int, dust_limit: int, anchors: bool) -> bool {
  return h.amount_msat / 1000 < dust_limit + lnc_htlc_fee(feerate, h.offered, anchors);
}
lnc_commit_fee :: (feerate: int, untrimmed: int, anchors: bool) -> int {
  return feerate * ((ifx anchors then 1124 else 724) + 172 * untrimmed) / 1000;
}
```

## Output order

Outputs are sorted as BIP-69 sorts them: by amount, then by
scriptPubKey. Two offered HTLCs of the same amount and payment hash have
the same script, but they can differ in `cltv_expiry`, and their
HTLC-timeout transactions differ in locktime. The peers send HTLC
signatures in output order, so BOLT 3 breaks that tie by expiry.

<!-- source: coax/lib/lightning/commitment.coax order -->
```jai
// BIP-69 with BOLT 3's tiebreak: by amount, then scriptPubKey as bytes (a
// prefix first), then cltv_expiry. Two offered HTLCs of one amount and
// payment hash have the same script, and only the expiry orders them.
lnc_before :: (a: LncOutput, b: LncOutput) -> bool {
  if a.sat != b.sat { return a.sat < b.sat; }
  n := ifx a.spk_n < b.spk_n then a.spk_n else b.spk_n;
  for k: 0..<n { if a.spk[k] != b.spk[k] { return a.spk[k] < b.spk[k]; } }
  if a.spk_n != b.spk_n { return a.spk_n < b.spk_n; }
  return a.expiry < b.expiry;
}
lnc_sort :: (c: [] LncCommit) {
  for i: 1..<c[0].n {
    j := i;
    while j > 0 && lnc_before(c[0].out[j], c[0].out[j-1]) {
      t := c[0].out[j]; c[0].out[j] = c[0].out[j-1]; c[0].out[j-1] = t; j -= 1;
    }
  }
}
```

## Building a commitment

`lnc_commitment_vartime` follows BOLT 3's "Commitment Transaction
Construction" step by step. The balances it takes are after the HTLCs
come out, in millisatoshis, and each output is rounded down to whole
satoshis. It refuses a key that is not a point, more HTLCs than it has
room for, and amounts that are negative or add up to more than the
funding output.

<!-- source: coax/lib/lightning/commitment.coax commitment -->
```jai
// The outputs of a commitment, by BOLT 3's "Commitment Transaction
// Construction": trim the HTLCs, take the base fee (and both anchors)
// from the funder, add each output that is not dust, add the anchors,
// sort. to_local_msat and to_remote_msat are the balances after the
// HTLCs are taken out.
lnc_commitment_vartime :: (ch: LncChannel, to_local_msat: int, to_remote_msat: int, feerate: int, htlcs: [] LncHtlc) -> LncCommit {
  c := ch; r: [1] LncCommit;
  if !lnc_channel_ok(c) { r[0].code = LNC_BAD_KEY; return r[0]; }
  if htlcs.count > LNC_MAX_HTLCS { r[0].code = LNC_TOO_MANY; return r[0]; }
  total := to_local_msat + to_remote_msat;
  bad := to_local_msat < 0 || to_remote_msat < 0 || feerate < 0;
  for i: 0..<htlcs.count { total += htlcs[i].amount_msat; bad = bad || htlcs[i].amount_msat < 0; }
  if bad || total > c.funding_sat * 1000 { r[0].code = LNC_BAD_AMOUNT; return r[0]; }
  for i: 0..<htlcs.count { if !lnc_trimmed(htlcs[i], feerate, c.dust_limit, c.anchors) { r[0].untrimmed += 1; } }
  r[0].fee = lnc_commit_fee(feerate, r[0].untrimmed, c.anchors);
  local := to_local_msat / 1000; remote := to_remote_msat / 1000;    // whole satoshis, rounded down
  charge := r[0].fee + (ifx c.anchors then 2 * LNC_ANCHOR_SAT else 0);
  if c.local_funder { local -= charge; } else { remote -= charge; }
  for i: 0..<htlcs.count {
    h := htlcs[i];
    if !lnc_trimmed(h, feerate, c.dust_limit, c.anchors) {
      lnc_add(r[..], c, htlcs, ifx h.offered then LNC_OFFERED else LNC_RECEIVED, h.amount_msat / 1000, i, h.expiry);
    }
  }
  if local >= c.dust_limit { lnc_add(r[..], c, htlcs, LNC_TO_LOCAL, local, -1, 0); }
  if remote >= c.dust_limit { lnc_add(r[..], c, htlcs, LNC_TO_REMOTE, remote, -1, 0); }
  if c.anchors {
    // An anchor for each side with an output, both while any HTLC is untrimmed.
    if local >= c.dust_limit || r[0].untrimmed > 0 { lnc_add(r[..], c, htlcs, LNC_LOCAL_ANCHOR, LNC_ANCHOR_SAT, -1, 0); }
    if remote >= c.dust_limit || r[0].untrimmed > 0 { lnc_add(r[..], c, htlcs, LNC_REMOTE_ANCHOR, LNC_ANCHOR_SAT, -1, 0); }
  }
  lnc_sort(r[..]);
  return r[0];
}
```

The unsigned commitment is version 2, with one input and the sorted
outputs:

<!-- source: coax/lib/lightning/commitment.coax unsigned -->
```jai
// The commitment without its witness: version 2, the funding outpoint
// with sequence 0x80 and the obscured number's upper 24 bits, the
// outputs, and locktime 0x20 and its lower 24 bits. Its length, or -1.
lnc_unsigned_vartime :: (ch: LncChannel, cm: LncCommit, out: [] u8) -> int {
  c := ch; m := cm;
  if m.code != LNC_OK || out.count < LNC_TX { return -1; }
  at := bt_le(out, 0, 2, 4);
  at = bt_var_put(out, at, 1);
  at = bt_put(out, at, c.funding_txid[..]); at = bt_le(out, at, c.funding_index, 4);
  at = bt_var_put(out, at, 0);                                    // no scriptSig
  at = bt_le(out, at, (0x80 << 24) | ((c.obscured >> 24) & 0xffffff), 4);
  at = bt_var_put(out, at, m.n);
  for k: 0..<m.n {
    at = bt_le(out, at, m.out[k].sat, 8);
    at = bt_var_put(out, at, m.out[k].spk_n); at = bt_put(out, at, m.out[k].spk[0..<m.out[k].spk_n]);
  }
  return bt_le(out, at, (0x20 << 24) | (c.obscured & 0xffffff), 4);
}
```

## Signing

Both peers sign the commitment's BIP-143 hash, input 0 spending the
funding amount under the funding script, with `SIGHASH_ALL`. BOLT 2 sends
signatures as 64 bytes; on chain they are DER with the hash type after
them. The witness is `0 <sig1> <sig2> <script>`: the empty item is the
one `OP_CHECKMULTISIG` pops by mistake, and the signatures follow the
script's key order, not local then remote.

<!-- source: coax/lib/lightning/commitment.coax sign -->
```jai
// A transaction's txid in hash order: SHA-256d of the unsigned form.
lnc_txid_vartime :: (unsigned: [] u8, out: [] u8) { bt_sha2(unsigned, out); }
// The BIP-143 hash both peers sign: input 0, the funding script, the
// funding amount, SIGHASH_ALL.
lnc_sighash_vartime :: (ch: LncChannel, cm: LncCommit, out: [] u8) -> bool {
  c := ch; u: [LNC_TX] u8; ws: [73] u8;
  n := lnc_unsigned_vartime(c, cm, u[..]);
  if n < 0 { return false; }
  wn := lnc_funding_script_vartime(c.local_funding[..], c.remote_funding[..], ws[..]);
  return bt_sighash_v0(u[0..<n], bt_tx_read(u[0..<n]), 0, ws[0..<wn], c.funding_sat, BT_ALL, out);
}
// A low-S RFC 6979 signature of a 32-byte hash, DER-encoded into out
// (LNC_DER bytes); its length, or -LNC_SIGN. The key is secret and goes
// only to chapter 20's constant-time signer.
lnc_sign :: (key: [] u8, digest: [] u8, out: [] u8) -> int {
  w: [1] EcdsaWork; sig: [64] u8;
  if out.count < LNC_DER || !ecdsa_sign(key, digest, w[..], sig[..], false) { return -LNC_SIGN; }
  return ecdsa_der(sig[..], out);
}
// One witness item: its length, then its bytes, with a hash type after a
// signature (hashtype -1 for none).
lnc_item :: (out: [] u8, at: int, b: [] u8, hashtype: int) -> int {
  n := bt_put(out, bt_var_put(out, at, b.count + ifx hashtype >= 0 then 1 else 0), b);
  if hashtype >= 0 { out[n] = cast(u8) hashtype; n += 1; }
  return n;
}
// An unsigned transaction with one input and its witness (count items,
// already written): the segwit marker and flag after the version, the
// witness before the locktime.
lnc_wrap :: (u: [] u8, count: int, witness: [] u8, out: [] u8) -> int {
  at := bt_put(out, 0, u[0..<4]);
  out[at] = 0; out[at+1] = 1;
  at = bt_put(out, at + 2, u[4..<u.count-4]);
  at = bt_put(out, bt_var_put(out, at, count), witness);
  return bt_put(out, at, u[u.count-4..<u.count]);
}
// The signed commitment: witness 0 <sig for pubkey1> <sig for pubkey2>
// <funding script>, the signatures in the script's key order. local_sig
// and remote_sig are DER, without the hash type. Its length, or -1.
lnc_signed_vartime :: (ch: LncChannel, cm: LncCommit, local_sig: [] u8, remote_sig: [] u8, out: [] u8) -> int {
  c := ch; u: [LNC_TX] u8; w: [256] u8; ws: [73] u8;
  n := lnc_unsigned_vartime(c, cm, u[..]);
  if n < 0 || out.count < LNC_TX { return -1; }
  wn := lnc_funding_script_vartime(c.local_funding[..], c.remote_funding[..], ws[..]);
  local_first := cb_ct_equal(c.local_funding[..], ws[2..<35]) == CB_MAX64;   // after OP_2 and the push
  at := lnc_item(w[..], 0, w[0..<0], -1);                       // CHECKMULTISIG's extra item
  at = lnc_item(w[..], at, ifx local_first then local_sig else remote_sig, BT_ALL);
  at = lnc_item(w[..], at, ifx local_first then remote_sig else local_sig, BT_ALL);
  at = lnc_item(w[..], at, ws[0..<wn], -1);
  return lnc_wrap(u[0..<n], 4, w[0..<at], out);
}
```

## HTLC transactions

An HTLC-timeout transaction spends an offered HTLC once its
`cltv_expiry` is reached: its locktime is the expiry. An HTLC-success
transaction spends a received HTLC with the preimage, and its locktime
is 0. Both pay the HTLC's amount, less their fee, to the same script as
`to_local`, so the owner waits `to_self_delay` again and a revoked one
can still be punished.

The owner cannot sign alone: the HTLC branch is a 2-of-2, and the peer
sends its HTLC signatures with each new commitment. With
`option_anchors` the peer signs with `SIGHASH_SINGLE|SIGHASH_ANYONECANPAY`.
That signature covers only its own input and the matching output, so the
owner can add an input and a change output to pay the fee when the HTLC
transaction is broadcast. The input's sequence is then 1, for the
`1 OP_CHECKSEQUENCEVERIFY` in the HTLC scripts.

<!-- source: coax/lib/lightning/commitment.coax htlc -->
```jai
// The hash type of the remote node's HTLC signature: SIGHASH_ALL, or with
// anchors SIGHASH_SINGLE|SIGHASH_ANYONECANPAY, so the owner can add inputs
// and outputs for the fee.
lnc_remote_hashtype :: (ch: LncChannel) -> int { return ifx ch.anchors then BT_SINGLE | BT_ANYONE else BT_ALL; }
// The HTLC-timeout (offered) or HTLC-success (received) transaction for
// output index: version 2, that output with sequence 0 (1 with anchors),
// one output of the HTLC's amount less its fee to the owner's delayed key
// or the revocation key, and locktime cltv_expiry (timeout) or 0. Its
// length, or -LNC_NOT_HTLC.
lnc_htlc_unsigned_vartime :: (ch: LncChannel, cm: LncCommit, index: int, feerate: int, out: [] u8) -> int {
  c := ch; m := cm; u: [LNC_TX] u8; txid: [32] u8; ws: [LNC_SCRIPT] u8; spk: [34] u8;
  if index < 0 || index >= m.n || (m.out[index].kind != LNC_OFFERED && m.out[index].kind != LNC_RECEIVED) { return -LNC_NOT_HTLC; }
  if out.count < LNC_HTLC_TX { return -LNC_CAPACITY; }
  n := lnc_unsigned_vartime(c, m, u[..]);
  lnc_txid_vartime(u[0..<n], txid[..]);
  o := m.out[index]; offered := o.kind == LNC_OFFERED;
  at := bt_le(out, 0, 2, 4);
  at = bt_var_put(out, at, 1);
  at = bt_put(out, at, txid[..]); at = bt_le(out, at, index, 4);
  at = bt_var_put(out, at, 0);
  at = bt_le(out, at, ifx c.anchors then 1 else 0, 4);
  at = bt_var_put(out, at, 1);
  at = bt_le(out, at, o.sat - lnc_htlc_fee(feerate, offered, c.anchors), 8);
  wn := lnc_to_local_script_vartime(c.revocation[..], c.to_self_delay, c.local_delayed[..], ws[..]);
  at = bt_var_put(out, at, 34); at = bt_put(out, at, spk[0..<p2wsh_script_vartime(ws[0..<wn], spk[..])]);
  return bt_le(out, at, ifx offered then o.expiry else 0, 4);
}
// The BIP-143 hash of an HTLC transaction under hashtype: the HTLC's
// witness script and amount.
lnc_htlc_sighash_vartime :: (ch: LncChannel, cm: LncCommit, htlcs: [] LncHtlc, index: int, feerate: int, hashtype: int, out: [] u8) -> bool {
  c := ch; m := cm; u: [LNC_HTLC_TX] u8; ws: [LNC_SCRIPT] u8;
  n := lnc_htlc_unsigned_vartime(c, m, index, feerate, u[..]);
  if n < 0 { return false; }
  wn := lnc_witness_script_vartime(c, htlcs, m.out[index], ws[..]);
  return bt_sighash_v0(u[0..<n], bt_tx_read(u[0..<n]), 0, ws[0..<wn], m.out[index].sat, hashtype, out);
}
// The signed HTLC transaction: witness 0 <remotehtlcsig> <localhtlcsig>
// <payment_preimage> (success) or <> (timeout), then the HTLC's script.
lnc_htlc_signed_vartime :: (ch: LncChannel, cm: LncCommit, htlcs: [] LncHtlc, index: int, feerate: int,
                            remote_sig: [] u8, local_sig: [] u8, preimage: [] u8, out: [] u8) -> int {
  c := ch; m := cm; u: [LNC_HTLC_TX] u8; w: [LNC_HTLC_TX] u8; ws: [LNC_SCRIPT] u8;
  n := lnc_htlc_unsigned_vartime(c, m, index, feerate, u[..]);
  if n < 0 { return n; }
  if out.count < LNC_HTLC_TX { return -LNC_CAPACITY; }
  wn := lnc_witness_script_vartime(c, htlcs, m.out[index], ws[..]);
  at := lnc_item(w[..], 0, w[0..<0], -1);
  at = lnc_item(w[..], at, remote_sig, lnc_remote_hashtype(c));
  at = lnc_item(w[..], at, local_sig, BT_ALL);
  at = lnc_item(w[..], at, ifx m.out[index].kind == LNC_RECEIVED then preimage else w[0..<0], -1);
  at = lnc_item(w[..], at, ws[0..<wn], -1);
  return lnc_wrap(u[0..<n], 5, w[0..<at], out);
}
```

## The example

The example,
[bitcoin/examples/lightning-commitment.coax](../examples/lightning-commitment.coax),
builds Appendix B's funding output, then every commitment of Appendices
C and F with all their HTLC transactions, each signed by both peers and
compared with BOLT 3. It lists one commitment's outputs with and
without anchors, then spends the outputs under chapter 36's interpreter.

<!-- source: bitcoin/examples/lightning-commitment.coax example -->
```jai
#load "../../coax/lib/lightning/commitment.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [2048] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// Chapter 36's interpreter with the standard flags BOLT 3's scripts meet:
// input 0 of tx spends amount satoshis locked by spk.
FLAGS :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_STRICTENC | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_LOW_S
  | SCRIPT_VERIFY_NULLDUMMY | SCRIPT_VERIFY_MINIMALDATA | SCRIPT_VERIFY_CLEANSTACK | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY
  | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_MINIMALIF | SCRIPT_VERIFY_NULLFAIL
  | SCRIPT_VERIFY_WITNESS_PUBKEYTYPE;
runs :: (tx: [] u8, spk: [] u8, amount: int) -> string {
  ins: [1] TxIn; outs: [LNC_MAX_OUTPUTS] TxOut; items: [8] TxItem; w: [1] ScriptWork;
  t := tx_read_vartime(tx, true, ins[..], outs[..], items[..]);
  if t.code != TX_OK { return "unreadable"; }
  sp := ScriptSpend.{b=tx, t=t, ins=ins[0..<t.nin], outs=outs[0..<t.nout], items=items[0..<t.nitems], index=0, amount=amount, spk=spk};
  return SCRIPT_ERR_NAMES[script_verify_vartime(sp, FLAGS, w[..])];
}
// A spend of the commitment's to_local output at index, with sequence:
// by the revocation key (witness <sig> 1) or the delayed key (<sig> <>).
spend_to_local :: (c: LncChannel, m: LncCommit, index: int, sequence: int, key: [] u8, revocation: bool, out: [] u8) -> int {
  ch := c; u: [LNC_TX] u8; txid: [32] u8; ws: [LNC_SCRIPT] u8; h: [32] u8; der: [LNC_DER] u8; w: [256] u8; spk: [22] u8;
  n := lnc_unsigned_vartime(ch, m, u[..]); lnc_txid_vartime(u[0..<n], txid[..]);
  at := bt_le(u[..], 0, 2, 4); at = bt_var_put(u[..], at, 1);
  at = bt_put(u[..], at, txid[..]); at = bt_le(u[..], at, index, 4); at = bt_var_put(u[..], at, 0); at = bt_le(u[..], at, sequence, 4);
  at = bt_var_put(u[..], at, 1); at = bt_le(u[..], at, m.out[index].sat - 500, 8);
  at = bt_var_put(u[..], at, 22); at = bt_put(u[..], at, spk[0..<p2wpkh_script_vartime(ch.local_delayed[..], spk[..])]);
  at = bt_le(u[..], at, 0, 4);
  wn := lnc_to_local_script_vartime(ch.revocation[..], ch.to_self_delay, ch.local_delayed[..], ws[..]);
  #assert bt_sighash_v0(u[0..<at], bt_tx_read(u[0..<at]), 0, ws[0..<wn], m.out[index].sat, BT_ALL, h[..]);
  one: [1] u8; one[0] = 1;
  k := lnc_item(w[..], 0, der[0..<lnc_sign(key, h[..], der[..])], BT_ALL);
  k = lnc_item(w[..], k, ifx revocation then one[..] else one[0..<0], -1);
  k = lnc_item(w[..], k, ws[0..<wn], -1);
  return lnc_wrap(u[0..<at], 3, w[0..<k], out);
}
// The outputs of a commitment, one line each.
outputs :: (c: LncChannel, m: LncCommit, feerate: int) {
  for i: 0..<m.n {
    o := m.out[i];
    print("  % % % sat", i, LNC_KINDS[o.kind], o.sat);
    if o.kind == LNC_OFFERED || o.kind == LNC_RECEIVED {
      print(", expiry %, its HTLC-% pays %", o.expiry, ifx o.kind == LNC_OFFERED then "timeout" else "success",
            o.sat - lnc_htlc_fee(feerate, o.kind == LNC_OFFERED, c.anchors));
    }
    print("\n");
  }
}
CODES :: [7] string.["OK", "BAD_KEY", "TOO_MANY", "BAD_AMOUNT", "NOT_HTLC", "SIGN", "CAPACITY"];

main :: () {
  // ---- Appendix B: the funding output and transaction
  print("BOLT 3 Appendix B: the funding transaction\n");
  a: [33] u8; b: [33] u8; ws: [LNC_SCRIPT] u8; spk: [34] u8;
  lnc_fill(LNC_FUNDING.local_funding_pubkey, a[..]); lnc_fill(LNC_FUNDING.remote_funding_pubkey, b[..]);
  wn := lnc_funding_script_vartime(a[..], b[..], ws[..]);
  print("  funding script: "); show(ws[0..<wn]); print("\n");
  sn := p2wsh_script_vartime(ws[0..<wn], spk[..]);
  print("  funding output: "); show(spk[0..<sn]); print("\n");
  print("  in the funding transaction: %\n", tx_hex_is(spk[0..<sn], "0020c015c4a6be010e21657068fc2e6a9d02b27ebe4d490a25846f7237f104d1a3cd"));

  // ---- Appendix C and F: every commitment, signed by both peers
  print("BOLT 3 Appendix C (static_remotekey) and F (anchors)\n");
  commits := 0; htlc_txs := 0;
  for k: 0..<LNC_VECTORS.count {
    v := LNC_VECTORS[k]; s: [1] LncSecrets;
    c := lnc_vector_channel(v.anchors, v.dust_limit, s[..]);
    hs: [LNC_MAX_HTLCS] LncHtlc; pre: [LNC_MAX_HTLCS] [32] u8;
    m := lnc_commitment_vartime(c, v.to_local_msat, v.to_remote_msat, v.feerate, hs[0..<lnc_vector_htlcs(v.htlcs, hs[..], pre[..])]);
    ok := lnc_vector_matches(v, s[..], c);
    if ok { commits += 1; htlc_txs += v.count; }
    print("  % %: outputs %, base fee %, HTLC transactions %: %\n", ifx v.anchors then "F" else "C", k, m.n, m.fee, v.count, ok);
  }
  print("  % of % commitments and % of % HTLC transactions match\n", commits, LNC_VECTORS.count, htlc_txs, LNC_HTLC_TXS.count);

  // ---- one commitment, output by output
  s: [1] LncSecrets; hs: [LNC_MAX_HTLCS] LncHtlc; pre: [LNC_MAX_HTLCS] [32] u8;
  v := LNC_VECTORS[2];
  c := lnc_vector_channel(false, v.dust_limit, s[..]);
  n := lnc_vector_htlcs(v.htlcs, hs[..], pre[..]);
  m := lnc_commitment_vartime(c, v.to_local_msat, v.to_remote_msat, v.feerate, hs[0..<n]);
  print("Appendix C at feerate %: the outputs in order\n", v.feerate);
  outputs(c, m, v.feerate);
  u: [LNC_TX] u8; un := lnc_unsigned_vartime(c, m, u[..]);
  sequence := tx_le_vartime(u[..], 42, 4); locktime := tx_le_vartime(u[..], un - 4, 4);
  pay: [33] u8; lnc_fill(LNC_PARAMS.local_payment_basepoint, pay[..]);
  number := lnc_obscured_vartime(((sequence & 0xffffff) << 24) | (locktime & 0xffffff), pay[..], c.remote[..]);
  be: [8] u8; for i: 0..<4 { be[i] = cast(u8) ((sequence >> (24 - 8 * i)) & 255); be[4 + i] = cast(u8) ((locktime >> (24 - 8 * i)) & 255); }
  print("  sequence "); show(be[0..<4]); print(", locktime "); show(be[4..<8]); print(": commitment number %\n", number);

  // ---- the same HTLCs with anchors
  v = LNC_VECTORS[18];
  ca := lnc_vector_channel(true, v.dust_limit, s[..]);
  ma := lnc_commitment_vartime(ca, v.to_local_msat, v.to_remote_msat, v.feerate, hs[0..<n]);
  print("Appendix F at feerate %: the same HTLCs with anchors\n", v.feerate);
  outputs(ca, ma, v.feerate);
  wn = lnc_to_remote_script_vartime(ca.remote[..], ws[..]);
  print("  to_remote script: "); show(ws[0..<wn]); print("\n");
  wn = lnc_anchor_script_vartime(ca.local_funding[..], ws[..]);
  print("  local anchor script: "); show(ws[0..<wn]); print("\n");

  // ---- spending the outputs under chapter 36's interpreter
  print("spending the outputs (chapter 36's interpreter)\n");
  h: [32] u8; ls: [LNC_DER] u8; rs: [LNC_DER] u8; tx: [LNC_TX] u8; fund: [34] u8;
  #assert lnc_sighash_vartime(c, m, h[..]);
  ln := lnc_sign(s[0].local_funding[..], h[..], ls[..]); rn := lnc_sign(s[0].remote_funding[..], h[..], rs[..]);
  t := lnc_signed_vartime(c, m, ls[0..<ln], rs[0..<rn], tx[..]);
  wn = lnc_funding_script_vartime(c.local_funding[..], c.remote_funding[..], ws[..]);
  fn := p2wsh_script_vartime(ws[0..<wn], fund[..]);
  print("  the commitment spends the funding output: %\n", runs(tx[0..<t], fund[0..<fn], c.funding_sat));
  // Output 0 is HTLC 0, received: HTLC-success. Output 1 is HTLC 2, offered: HTLC-timeout.
  for i: 0..<2 {
    #assert lnc_htlc_sighash_vartime(c, m, hs[0..<n], i, LNC_VECTORS[2].feerate, BT_ALL, h[..]);
    rn = lnc_sign(s[0].remote_htlc[..], h[..], rs[..]); ln = lnc_sign(s[0].local_htlc[..], h[..], ls[..]);
    t = lnc_htlc_signed_vartime(c, m, hs[0..<n], i, LNC_VECTORS[2].feerate, rs[0..<rn], ls[0..<ln], pre[m.out[i].htlc][..], tx[..]);
    print("  HTLC-% for output %: %\n", ifx m.out[i].kind == LNC_OFFERED then "timeout" else "success", i,
          runs(tx[0..<t], m.out[i].spk[0..<34], m.out[i].sat));
  }
  wrong: [32] u8; wrong[0] = 9;
  #assert lnc_htlc_sighash_vartime(c, m, hs[0..<n], 0, LNC_VECTORS[2].feerate, BT_ALL, h[..]);
  rn = lnc_sign(s[0].remote_htlc[..], h[..], rs[..]); ln = lnc_sign(s[0].local_htlc[..], h[..], ls[..]);
  t = lnc_htlc_signed_vartime(c, m, hs[0..<n], 0, LNC_VECTORS[2].feerate, rs[0..<rn], ls[0..<ln], wrong[..], tx[..]);
  print("  HTLC-success with the wrong preimage: %\n", runs(tx[0..<t], m.out[0].spk[0..<34], m.out[0].sat));
  // to_local is the last output here. The revocation private key needs the
  // remote node's revocation secret and the owner's per-commitment secret.
  last := m.n - 1;
  secret: [32] u8; pcs: [32] u8; key: [32] u8; pcp: [33] u8;
  lnc_fill(LNC_PARAMS.remote_revocation_basepoint_secret, secret[..]); lnc_fill(LNC_PARAMS.local_per_commitment_secret, pcs[..]);
  code := lnk_revocation_privkey(secret[..], pcs[..], key[..]);
  t = spend_to_local(c, m, last, 0, key[..], true, tx[..]);
  print("  to_local by the revocation key, at once: %\n", runs(tx[0..<t], m.out[last].spk[0..<34], m.out[last].sat));
  lnc_fill(LNC_PARAMS.local_delayed_payment_basepoint_secret, secret[..]); lnc_fill(LNC_PARAMS.local_per_commitment_point, pcp[..]);
  code = lnk_privkey(secret[..], pcp[..], key[..]);
  t = spend_to_local(c, m, last, 0, key[..], false, tx[..]);
  print("  to_local by the delayed key at sequence 0: %\n", runs(tx[0..<t], m.out[last].spk[0..<34], m.out[last].sat));
  t = spend_to_local(c, m, last, c.to_self_delay - 1, key[..], false, tx[..]);
  print("  to_local by the delayed key at sequence %: %\n", c.to_self_delay - 1, runs(tx[0..<t], m.out[last].spk[0..<34], m.out[last].sat));
  t = spend_to_local(c, m, last, c.to_self_delay, key[..], false, tx[..]);
  print("  to_local by the delayed key at sequence %: %\n", c.to_self_delay, runs(tx[0..<t], m.out[last].spk[0..<34], m.out[last].sat));

  // ---- refusals
  print("refusals\n");
  many: [LNC_MAX_HTLCS + 1] LncHtlc;
  print("  % HTLCs: %\n", many.count, CODES[lnc_commitment_vartime(c, 1000, 1000, 253, many[..]).code]);
  print("  more than the funding output: %\n", CODES[lnc_commitment_vartime(c, 10000000000, 1000, 253, hs[0..<0]).code]);
  print("  an HTLC transaction for to_remote: %\n", CODES[-lnc_htlc_unsigned_vartime(c, m, m.n - 2, 647, u[..])]);
}
```

The output:

```text
BOLT 3 Appendix B: the funding transaction
  funding script: 5221023da092f6980e58d2c037173180e9a465476026ee50f96695963e8efe436f54eb21030e9f7b623d2ccc7c9bd44d66d5ce21ce504c0acf6385a132cec6d3c39fa711c152ae
  funding output: 0020c015c4a6be010e21657068fc2e6a9d02b27ebe4d490a25846f7237f104d1a3cd
  in the funding transaction: true
BOLT 3 Appendix C (static_remotekey) and F (anchors)
  C 0: outputs 2, base fee 10860, HTLC transactions 0: true
  C 1: outputs 7, base fee 0, HTLC transactions 5: true
  C 2: outputs 7, base fee 1024, HTLC transactions 5: true
  C 3: outputs 6, base fee 914, HTLC transactions 4: true
  C 4: outputs 6, base fee 2921, HTLC transactions 4: true
  C 5: outputs 5, base fee 2566, HTLC transactions 3: true
  C 6: outputs 5, base fee 2720, HTLC transactions 3: true
  C 7: outputs 4, base fee 2344, HTLC transactions 2: true
  C 8: outputs 4, base fee 3953, HTLC transactions 2: true
  C 9: outputs 3, base fee 3317, HTLC transactions 1: true
  C 10: outputs 3, base fee 4402, HTLC transactions 1: true
  C 11: outputs 2, base fee 3558, HTLC transactions 0: true
  C 12: outputs 2, base fee 6987454, HTLC transactions 0: true
  C 13: outputs 1, base fee 6987455, HTLC transactions 0: true
  C 14: outputs 1, base fee 6988001, HTLC transactions 0: true
  C 15: outputs 5, base fee 313, HTLC transactions 3: true
  F 16: outputs 4, base fee 16860, HTLC transactions 0: true
  F 17: outputs 2, base fee 16860, HTLC transactions 0: true
  F 18: outputs 9, base fee 1277, HTLC transactions 5: true
  F 19: outputs 8, base fee 1168, HTLC transactions 4: true
  F 20: outputs 6, base fee 3207, HTLC transactions 2: true
  F 21: outputs 5, base fee 4778, HTLC transactions 1: true
  F 22: outputs 4, base fee 5500, HTLC transactions 0: true
  F 23: outputs 2, base fee 6986795, HTLC transactions 0: true
  F 24: outputs 7, base fee 414, HTLC transactions 3: true
  25 of 25 commitments and 48 of 48 HTLC transactions match
Appendix C at feerate 647: the outputs in order
  0 received 1000 sat, expiry 500, its HTLC-success pays 546
  1 offered 2000 sat, expiry 502, its HTLC-timeout pays 1572
  2 received 2000 sat, expiry 501, its HTLC-success pays 1546
  3 offered 3000 sat, expiry 503, its HTLC-timeout pays 2572
  4 received 4000 sat, expiry 504, its HTLC-success pays 3546
  5 to_remote 3000000 sat
  6 to_local 6986976 sat
  sequence 802bb038, locktime 2052193e: commitment number 42
Appendix F at feerate 644: the same HTLCs with anchors
  0 local_anchor 330 sat
  1 remote_anchor 330 sat
  2 received 1000 sat, expiry 500, its HTLC-success pays 1000
  3 offered 2000 sat, expiry 502, its HTLC-timeout pays 2000
  4 received 2000 sat, expiry 501, its HTLC-success pays 2000
  5 offered 3000 sat, expiry 503, its HTLC-timeout pays 3000
  6 received 4000 sat, expiry 504, its HTLC-success pays 4000
  7 to_remote 3000000 sat
  8 to_local 6986063 sat
  to_remote script: 21032c0b7cf95324a07d05398b240174dc0c2be444d96b159aa6c7f7b1e668680991ad51b2
  local anchor script: 21023da092f6980e58d2c037173180e9a465476026ee50f96695963e8efe436f54ebac736460b268
spending the outputs (chapter 36's interpreter)
  the commitment spends the funding output: OK
  HTLC-success for output 0: OK
  HTLC-timeout for output 1: OK
  HTLC-success with the wrong preimage: EQUALVERIFY
  to_local by the revocation key, at once: OK
  to_local by the delayed key at sequence 0: UNSATISFIED_LOCKTIME
  to_local by the delayed key at sequence 143: UNSATISFIED_LOCKTIME
  to_local by the delayed key at sequence 144: OK
refusals
  9 HTLCs: TOO_MANY
  more than the funding output: BAD_AMOUNT
  an HTLC transaction for to_remote: NOT_HTLC
```

What the output shows:

- Appendix B's funding script and its P2WSH output are the ones in the
  funding transaction.
- All 25 commitments of Appendix C (16, with `option_static_remotekey`)
  and Appendix F (9, with `option_anchors`), and all 48 of their HTLC
  transactions, match BOLT 3 byte for byte. As the feerate rises from
  vector to vector, HTLC outputs drop out, until the fee takes the
  funder's whole balance (vectors 13, 14 and 23).
- At feerate 647, each HTLC transaction pays its HTLC less its fee: the
  1,000-satoshi received HTLC keeps exactly 546, the dust limit. The
  commitment number 42 comes back from the sequence and the locktime.
- With anchors, the HTLC transactions pay the whole HTLC: their fee is
  added when they are broadcast. The funder's balance pays for both
  330-satoshi anchors.
- The signed commitment spends the funding output. The HTLC-success and
  HTLC-timeout transactions spend their outputs, and a wrong preimage
  fails at `OP_EQUALVERIFY`.
- The revocation key spends `to_local` at once. The owner's delayed key
  needs a sequence of at least `to_self_delay`, 144 blocks.
- Too many HTLCs, more money than the channel holds, and an HTLC
  transaction for a non-HTLC output are refused.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-commitment.out](../fixtures/lightning-commitment.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-commitment.coax](../examples/checks/lightning-commitment.coax),
which loads the example:

- for Appendix C's five HTLCs at twelve feerates, with and without
  anchors, the outputs are sorted, the HTLC outputs are exactly the
  untrimmed HTLCs, and the outputs and fee add up to the funding amount
- every HTLC transaction of Appendix C at feerate 0, and of Appendix F,
  spends its HTLC output under chapter 36's interpreter
- the commitment number comes back from the locktime and sequence for
  numbers from 0 to 2^48 - 1

<!-- source: bitcoin/examples/checks/lightning-commitment.coax check -->
```jai
// For Appendix C's five HTLCs at twelve feerates, with and without
// anchors: the outputs are in BIP-69 order, the HTLC outputs are the
// untrimmed HTLCs, and the outputs and the fee spend the funding output
// exactly, the fee at least the base fee.
FEERATES :: [12] int.[0, 253, 647, 648, 2069, 2070, 2194, 3702, 3703, 4914, 9651180, 9651936];
#forall "the outputs are sorted and account for every satoshi" k: 0..<24 #steps 3000000000 {
  s: [1] LncSecrets; hs: [LNC_MAX_HTLCS] LncHtlc; pre: [LNC_MAX_HTLCS] [32] u8;
  c := lnc_vector_channel(k >= 12, 546, s[..]);
  n := lnc_vector_htlcs(31, hs[..], pre[..]);
  feerate := FEERATES[k % 12];
  m := lnc_commitment_vartime(c, 6988000000, 3000000000, feerate, hs[0..<n]);
  #assert m.code == LNC_OK;
  sum := 0; htlc_outputs := 0;
  for i: 0..<m.n {
    sum += m.out[i].sat;
    if m.out[i].kind == LNC_OFFERED || m.out[i].kind == LNC_RECEIVED { htlc_outputs += 1; }
    if i > 0 { #assert !lnc_before(m.out[i], m.out[i-1]); }
  }
  #assert htlc_outputs == m.untrimmed && m.fee == lnc_commit_fee(feerate, m.untrimmed, c.anchors);
  #assert sum <= c.funding_sat && c.funding_sat - sum >= ifx m.fee < 6988000 then m.fee else 0;
}
// Every HTLC transaction of a commitment runs its HTLC script under
// chapter 36's interpreter: Appendix C's at feerate 0 (SIGHASH_ALL,
// sequence 0) and Appendix F's (SIGHASH_SINGLE|ANYONECANPAY from the
// remote node, sequence 1 for the CSV of 1).
#forall "every HTLC transaction spends its output" k: 0..<10 #steps 4000000000 {
  v := LNC_VECTORS[ifx k < 5 then 1 else 18]; s: [1] LncSecrets;
  c := lnc_vector_channel(v.anchors, v.dust_limit, s[..]);
  hs: [LNC_MAX_HTLCS] LncHtlc; pre: [LNC_MAX_HTLCS] [32] u8;
  n := lnc_vector_htlcs(v.htlcs, hs[..], pre[..]);
  m := lnc_commitment_vartime(c, v.to_local_msat, v.to_remote_msat, v.feerate, hs[0..<n]);
  i := k % 5 + ifx v.anchors then 2 else 0;                  // with anchors, the two anchors come first
  h: [32] u8; rs: [LNC_DER] u8; ls: [LNC_DER] u8; tx: [LNC_HTLC_TX] u8;
  #assert lnc_htlc_sighash_vartime(c, m, hs[0..<n], i, v.feerate, lnc_remote_hashtype(c), h[..]);
  rn := lnc_sign(s[0].remote_htlc[..], h[..], rs[..]);
  #assert lnc_htlc_sighash_vartime(c, m, hs[0..<n], i, v.feerate, BT_ALL, h[..]);
  ln := lnc_sign(s[0].local_htlc[..], h[..], ls[..]);
  t := lnc_htlc_signed_vartime(c, m, hs[0..<n], i, v.feerate, rs[0..<rn], ls[0..<ln], pre[m.out[i].htlc][..], tx[..]);
  #assert runs(tx[0..<t], m.out[i].spk[0..<34], m.out[i].sat) == "OK";
}
// The commitment number survives the trip through locktime and sequence
// for numbers up to 2^48 - 1, and the opener's point must come first.
#forall "the obscured number comes back from locktime and sequence" k: 0..<6 {
  a: [33] u8; b: [33] u8;
  lnc_fill(LNC_PARAMS.local_payment_basepoint, a[..]); lnc_fill(LNC_PARAMS.remote_payment_basepoint, b[..]);
  numbers := [6] int.[0, 1, 42, 0xffffff, 0x1000000, (1 << 48) - 1];
  x := lnc_obscured_vartime(numbers[k], a[..], b[..]);
  locktime := (0x20 << 24) | (x & 0xffffff); sequence := (0x80 << 24) | ((x >> 24) & 0xffffff);
  #assert lnc_obscured_vartime(((sequence & 0xffffff) << 24) | (locktime & 0xffffff), a[..], b[..]) == numbers[k];
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-commitment.coax
```

`coax test` runs 501 checks, including the loaded files' own checks, in
about three minutes on 2026-11-03.

### The library's own checks

`commitment.coax` is an `#authority` file. Its checks cover Appendix B's
funding script and the whole funding transaction, signed with chapter
37's legacy signature hash; Appendix C's keys derived by `keys.coax`
from its secrets; all 25 commitments and 48 HTLC transactions of
Appendices C and F, signed and compared byte for byte; BOLT 3's fee
example; the obscured number; and refusals.

```shell
coax/bin/coax test coax/lib/lightning/commitment.coax
```

`coax test` runs 496 checks, including those of the files it loads, in
about three minutes on 2026-11-03. Mutation testing: not run (owner speed
order).

### BOLT 3 and an independent builder

[commitment.py](../../coax/tests/lightning/commitment.py) computes every
line the example prints. Its transactions come from
[bolt3_tx.py](../../coax/tests/lightning/bolt3_tx.py), a separate Python
builder and RFC 6979 signer written from BOLT 3 and BIP-143, and its
spends run under chapter 36's Python transliteration of Bitcoin Core's
interpreter. Neither shares code with Coax. Every vector of Appendices
B, C and F must come out byte for byte before the transcript is compared
with the fixture.

```shell
python3 -I coax/tests/lightning/commitment.py --coax coax/bin/coax
```

On 2026-11-03 all 65 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

[make_commitment_vectors.py](../../coax/tests/lightning/make_commitment_vectors.py)
reads the pinned BOLT 3 into
[commitment_vectors.coax](../../coax/tests/lightning/commitment_vectors.coax).
BOLT 3 names the HTLCs each test uses in prose, so the script records
them as a bit mask, and `bolt3_tx.py` must rebuild every vector before
the file is written.

## What this chapter leaves out

- Sending signatures and revoking commitments in `commitment_signed` and
  `revoke_and_ack`: chapter 109.
- Watching the chain, and the penalty and sweep transactions that spend
  a broadcast commitment's outputs: chapter 110.
- Closing transactions, which chapter 109 builds from the same funding
  output.
- `zero_fee_commitments` (version 3 transactions with one shared anchor)
  and the dual-funded transactions of Appendix G.
- The older `to_remote` key from before `option_static_remotekey`, which
  BOLT 3 no longer has vectors for.

## Summary

- The funding output is a P2WSH of a sorted 2-of-2 of the funding keys.
- A commitment hides its number, XORed with a mask from both payment
  basepoints, in its locktime and sequence.
- `to_local` waits `to_self_delay` blocks or goes to the revocation key;
  `to_remote` is P2WPKH, or with anchors waits one block; each anchor is
  330 satoshis.
- HTLC outputs are spent by the peer with the preimage or after the
  expiry, by the revocation key, or by the owner through an HTLC
  transaction the peer signed.
- The fee comes from an expected weight; outputs below the dust limit,
  counting the HTLC transaction's fee, are trimmed into it.
- Outputs follow BIP-69, with HTLC expiry as the last tiebreak.

## Dependencies

- [38. Segwit v0 signature hashes](ch038-segwit-v0-signature-hashes.md)
- [100. Multisig](ch100-multisig.md)
- [106. Commitment keys and revocation](ch106-commitment-keys-and-revocation.md)

## Sources

- [BOLT 3](https://github.com/lightning/bolts/blob/master/03-transactions.md),
  "Transactions", "Fees", "Commitment Transaction Construction" and
  Appendices B, C and F, at lightning/bolts revision `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/03-transactions.md).
- [BIP-69](https://github.com/bitcoin/bips/blob/master/bip-0069.mediawiki),
  lexicographical ordering of transaction outputs.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
