# 35. Txids, wtxids and malleability

A txid names a transaction before it confirms, and other transactions
spend its outputs by that name. If someone can change a signed
transaction's bytes without making it invalid, its txid changes, and
every transaction built on the old txid breaks. That is malleability.
This chapter changes real, pinned transactions in each way Bitcoin
allowed, shows which ID each change moves, and names the rule that closed
each one. Chapter 34 already computes both IDs; this chapter adds the
edits.

## What this chapter teaches

- What the txid and the wtxid cover, and why a signature can't cover
  itself.
- Third-party malleability: anyone who relays a transaction can change a
  signature's encoding, replace `s` with `n − s`, push data in a longer
  form, change the `CHECKMULTISIG` dummy or add a stack item.
- Second-party malleability: the signer can always sign again.
- The fixes: BIP-62 (withdrawn), BIP-66, BIP-146's low-S rule, BIP-147's
  `NULLDUMMY`, and segwit, which moves signatures out of the txid.
- Why a segwit spend's txid is fixed once its inputs and outputs are,
  even against its own signer.
- What malleability did and didn't cause at Mt. Gox.

## Where txids are used

| Where | What the txid names | Chapter |
| --- | --- | --- |
| An outpoint | the transaction whose output an input spends | 34 |
| A block's Merkle root | each transaction in the block | 46 |
| Transaction relay | a transaction a peer has or wants | 83 |
| A wallet | a payment sent or received | 99 |
| Lightning | the funding transaction, before anyone signs the commitment transactions that spend it | 109 |

The wtxid names the full bytes. Since segwit, the coinbase commits to a
Merkle root of wtxids (chapter 49), and peers can announce transactions
by wtxid (BIP-339).

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [malleability.coax](../../coax/lib/bitcoin/malleability.coax) | `mal_reencode_vartime(sig, flip, out)` | a script signature in strict DER, with `s` or with `n − s` |
| | `mal_replace_vartime(b, prefix, new, out)` | a transaction with one scriptSig or witness item replaced |
| | `mal_push_vartime(data, minimal, out, at)` | a push in the shortest form or with `OP_PUSHDATA1` |
| | `mal_add_witness_vartime(b, t, stacks, out)` | a legacy transaction given witness stacks |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_txid_vartime`, `tx_wtxid_vartime` | the two IDs (chapter 34) |
| [der.coax](../../coax/lib/bitcoin/der.coax) | `der_check_vartime`, `der_low_s_vartime`, `der_checksig_vartime` | BIP-66, the low-S rule and verification (chapter 21) |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sighash_v0` | the BIP-143 digest a segwit v0 signature signs |

Everything here is public data, so every procedure runs in variable time
and its name ends in `_vartime` (chapter 3).

## What a signature can't sign

A signature signs a digest of the transaction, and the digest can't
include the signature. For a legacy input the signature sits in the
scriptSig, and the signature hash (chapter 37) blanks every scriptSig
before hashing. The txid, though, is SHA-256d of the bytes with the
scriptSigs in them. So whatever in a scriptSig can change without
breaking the signature check can change the txid.

The first edit rewrites a signature. Core's lax parser (chapter 21)
reads `r` and `s` from many encodings, and an ECDSA signature `(r, s)`
verifies exactly when `(r, n − s)` does: verification uses `s` only
through `R = (z/s)·G + (r/s)·Q`, and negating `s` negates `R`, which has
the same x-coordinate. `mal_reencode_vartime` reads a script signature
with the lax parser and writes it in strict DER, with either `s`:

<!-- source: coax/lib/bitcoin/malleability.coax reencode -->
```jai
// A script signature (DER, read by Core's lax parser, then the sighash
// byte) written again in strict DER: with the same r and s, or with s
// replaced by n - s when flip is true. Both forms verify for the same key
// and digest. Returns the new length, or -1 when the lax parser can't read
// the signature, r or s is zero, or out has fewer than 73 bytes.
mal_reencode_vartime :: (sig: [] u8, flip: bool, out: [] u8) -> int #export_c {
  compact: [64] u8;
  if sig.count < 2 || out.count < 73 || !der_read_lax_vartime(sig[0..<sig.count-1], compact[..]) { return -1; }
  if flip {
    s := sc_load(compact[32..<64]);
    #assert sc_store(sc_negate(s.value), compact[32..<64]);
  }
  n := ecdsa_der(compact[..], out);
  if n < 0 { return -1; }
  out[n] = sig[sig.count-1];
  return n + 1;
}
```

The other edits put new bytes in a length-prefixed field and write the
push opcodes a scriptSig is made of:

<!-- source: coax/lib/bitcoin/malleability.coax replace -->
```jai
// Replace a length-prefixed field of b, a scriptSig or one witness item,
// whose CompactSize length starts at byte prefix, with new. Writes the
// result to out, which must not overlap b, and returns its size, or -1
// when the length doesn't read, the field runs past the end, or out is too
// small. Nothing else changes: a scriptSig still has its input, an item
// its stack.
mal_replace_vartime :: (b: [] u8, prefix: int, new: [] u8, out: [] u8) -> int #export_c {
  c := tx_compact_read_vartime(b, prefix);
  if c.code != TX_OK || c.value > b.count - c.next { return -1; }
  after := c.next + c.value;                     // the first byte after the old field
  size := prefix + tx_compact_size_vartime(new.count) + new.count + b.count - after;
  if size > out.count { return -1; }
  for i: 0..<prefix { out[i] = b[i]; }
  at := tx_compact_write_vartime(out, prefix, new.count);
  for i: 0..<new.count { out[at+i] = new[i]; }
  at += new.count;
  for i: after..<b.count { out[at+i-after] = b[i]; }
  return size;
}
```

<!-- source: coax/lib/bitcoin/malleability.coax push -->
```jai
// Push data (0 to 255 bytes) onto a script at out[at]. When minimal is
// true the push takes its shortest form: OP_0 for no bytes, one length
// byte for 1 to 75 (the opcode is the length), else OP_PUSHDATA1 (0x4c)
// and a length byte. When minimal is false it always uses OP_PUSHDATA1.
// Pushing 1 to 16 or 0x81 with OP_1..OP_16 and OP_1NEGATE is also
// shortest; chapter 36's interpreter checks that. Returns where writing
// ends, or -1.
mal_push_vartime :: (data: [] u8, minimal: bool, out: [] u8, at: int) -> int #export_c {
  if data.count > 255 { return -1; }
  head := ifx minimal && data.count <= 75 then 1 else 2;
  if at + head + data.count > out.count { return -1; }
  if head == 1 { out[at] = cast(u8) data.count; }
  else { out[at] = 0x4c; out[at+1] = cast(u8) data.count; }
  for i: 0..<data.count { out[at+head+i] = data[i]; }
  return at + head + data.count;
}
```

## Third-party malleability

Transaction `23b397ed` is a mainnet spend from 2011 of a 1-of-2 bare
multisig output. Its scriptSig is `OP_0`, then a 71-byte push of the
signature. Bitcoin Core's `tx_valid.json` keeps it because its signature
is not valid DER: `s` starts with `0xdd`, whose top bit is set, without
the `0x00` that DER needs before it, so a strict reader sees a negative
number. OpenSSL accepted it, and so did every node.

Five kinds of change keep the spend valid under the rules of 2011:

| Change | Why the script still passes | Rule that refuses it | Kind |
| --- | --- | --- | --- |
| Another encoding of the same `r` and `s` | the lax parser reads it | BIP-66 strict DER | consensus since block 363,725 (2015) |
| `s` replaced by `n − s` | both verify | the low-S rule (`SCRIPT_VERIFY_LOW_S`) | policy only |
| A push in a longer form, such as `OP_PUSHDATA1` for 71 bytes | the same bytes reach the stack | `SCRIPT_VERIFY_MINIMALDATA` | policy only |
| The `CHECKMULTISIG` dummy set to anything | `CHECKMULTISIG` pops one item too many and never reads it | BIP-147 `NULLDUMMY` | consensus with segwit, block 481,824 (2017) |
| An extra item under the others | the script never reads it, and the top of the stack is still true | `SCRIPT_VERIFY_CLEANSTACK` | policy only |

A policy rule stops nodes running Bitcoin Core from relaying or mining
the change. It doesn't stop a miner who mines the changed transaction
directly. Only a consensus rule does that.

`tx_valid.json` keeps four changed forms of `23b397ed`: a byte stuffed
into the signature before its sighash byte, and the dummy set to `0xff`,
`OP_1` and `OP_1NEGATE`. Core marks them valid when the DER and
`NULLDUMMY` flags are off. The checks rebuild each one from the mined
form with `mal_replace_vartime` and get Core's bytes exactly. The
example adds five more forms built here. The canonical form has the
signature in strict DER with the low `s`; it is what the transaction
would look like if it were signed today. Ten forms of one spend give ten
txids.

### Worked example: s and n − s

The mined signature's `s` is

```text
s     = ddc15ecdfc3507ac48e1681a33eb60996631bf6bf5bc0a0682c4db743ce7ca2b
n − s = 223ea13203caf853b71e97e5cc149f65547d1d7ab98c96353d0d8318934e7716
```

`s` is above `n / 2`, so it is the high one. In strict DER it needs a
leading `0x00`, so the signature grows from 71 to 72 bytes with its
sighash byte. `n − s` starts with `0x22`, needs no padding, and gives 71
bytes. Flipping the low form again gives the 72-byte strict form back,
and the library checks that.

## Second-party malleability

The signer can always make a new valid signature: with another nonce, or
with another sighash type that covers the same inputs and outputs. No
rule can stop this, because every new signature is as valid as the
first. Chapter 20's signer is deterministic (RFC 6979), so the example
signs BIP-143's P2SH-P2WPKH example twice with its published key: once
with `SIGHASH_ALL`, which reproduces BIP-143's published signature, and
once with `SIGHASH_ALL|ANYONECANPAY`. Both verify. The question is what
each signature changes.

## Segwit: the txid without the signatures

Segwit (BIP-141) put the signatures of segwit inputs in the witness,
which the txid doesn't hash. The scriptSig of a segwit input is fixed:

- A native witness program (P2WPKH, P2WSH, P2TR) needs an empty
  scriptSig. Anything else fails with `SCRIPT_ERR_WITNESS_MALLEATED`.
- A P2SH-wrapped program needs a scriptSig of exactly one push, the
  redeem script, or it fails with `SCRIPT_ERR_WITNESS_MALLEATED_P2SH`.
  The redeem script is fixed by the address, so that push is too.

A P2WSH script must leave exactly one item on the stack, by consensus,
and a P2WPKH witness must be exactly a signature and a key. So once a
transaction's inputs are all segwit, its txid depends only on its
version, outpoints, sequences, outputs and locktime. A changed signature
changes the wtxid and nothing else, whoever changes it. That is what
lets Lightning sign transactions that spend a funding transaction before
the funding transaction is broadcast: its txid can't change.

A segwit transaction with one legacy input is still malleable through
that input. Taproot key-path and script-path spends use BIP-340
signatures, where `s` is unique for a given `R`, key and message, and
the sighash type is part of the signed message: a 64-byte signature
can't be turned into a 65-byte one with `SIGHASH_ALL` appended. An annex,
the optional last witness item that starts with `0x50`, is signed too.

`mal_add_witness_vartime` builds the segwit form the example needs:

<!-- source: coax/lib/bitcoin/malleability.coax witness -->
```jai
// Give a legacy transaction t, read from b without witnesses, the
// witness form: the marker 0x00 and flag 0x01 after the version, and
// stacks, one serialized stack per input in order, before the locktime.
// Returns the new size, or -1 when t is not a read legacy transaction or
// out is too small. Chapter 34's reader checks the result.
mal_add_witness_vartime :: (b: [] u8, t: Tx, stacks: [] u8, out: [] u8) -> int #export_c {
  if t.code != TX_OK || t.segwit { return -1; }
  size := t.total + 2 + stacks.count;
  if size > out.count { return -1; }
  for i: 0..<4 { out[i] = b[i]; }
  out[4] = 0x00; out[5] = 0x01;
  for i: 4..<t.locktime { out[i+2] = b[i]; }
  at := t.locktime + 2;
  for i: 0..<stacks.count { out[at+i] = stacks[i]; }
  at += stacks.count;
  for i: 0..<4 { out[at+i] = b[t.locktime+i]; }
  return size;
}
```

The example shows both halves on real transactions:

- Third party: the P2WPKH spend `f5864806` signs a BIP-143 digest of
  its stripped bytes and the 511,884 satoshis it spends. Replacing `s`
  with `n − s` still verifies, and only the low-S policy refuses it. The
  txid stays `f5864806…`; the wtxid changes, and the weight grows from
  442 to 443 because the high `s` needs a padding byte. Putting `OP_0`
  in the empty scriptSig changes the txid, and BIP-141 makes that spend
  invalid.
- Second party: signed as P2SH-P2WPKH, the two signatures give one txid
  and two wtxids. The same two signatures in a P2PKH scriptSig give two
  txids.

## The fixes, in order

| Proposal | What it did | Outcome |
| --- | --- | --- |
| BIP-62 (2014) | listed nine sources of malleability (high `s`, non-DER signatures, extra pushes, non-push opcodes in the scriptSig, non-minimal pushes, zero-padded numbers, inputs a script ignores, sighash-flag masking and new signatures by the sender) and a rule against each | withdrawn: the rules couldn't cover every script, and segwit solved the problem for new outputs |
| BIP-66 (2015) | strict DER for every signature, by consensus | active at block 363,725; it also freed consensus from OpenSSL's DER parser |
| BIP-146 (2016) | `LOW_S` and `NULLFAIL` as consensus rules | deployed as policy only; Core relays only low-S signatures since 0.11.1 (2015) |
| BIP-147 (2016) | the `CHECKMULTISIG` dummy must be empty | consensus with segwit, block 481,824 |
| BIP-141 segwit (2017) | signatures move to the witness; the txid excludes them | third-party malleability ends for transactions whose inputs are all segwit |

## Mt. Gox

On 7 February 2014 Mt. Gox stopped withdrawals, and on 10 February it
blamed transaction malleability. Its software tracked withdrawals by
txid. When a changed copy of a withdrawal confirmed instead, Mt. Gox saw
its own txid never confirm and could pay the withdrawal again. That flaw
was real. It was not the cause of Mt. Gox's loss. Decker and
Wattenhofer analyzed the changed transactions on the network and in the
block chain and found that malleability attacks before the announcement
could have taken at most 386 bitcoins; the wave of changed transactions
started after it. Mt. Gox reported about 850,000 bitcoins missing when it
filed for bankruptcy that month. Later investigations traced the loss to
theft from its wallets that had begun in 2011.

The lesson for wallets doesn't depend on Mt. Gox: until a transaction
confirms, don't identify it by a txid that a legacy input lets anyone
change. Track the outpoints it spends.

## The example

The example is
[bitcoin/examples/malleability.coax](../examples/malleability.coax). It
prints the ten forms of `23b397ed` with their txids and whether each
signature is strict DER and low-S. It verifies `f5864806`'s signature
against its BIP-143 digest, replaces `s` with `n − s` and puts `OP_0` in
its scriptSig. It signs BIP-143's P2SH-P2WPKH example twice and compares
the IDs.

<!-- source: bitcoin/examples/malleability.coax example -->
```jai
#load "../../coax/lib/bitcoin/malleability.coax";
#load "../../coax/lib/bitcoin/sighash.coax";
#load "../../coax/lib/bitcoin/hash160.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/tx_core_vectors.coax";
#load "../../coax/tests/bitcoin/spent_vectors.coax";
#import "Basic";

show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
equal_bytes :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// A txid or wtxid as explorers show it: the digest reversed.
shown :: (digest: [] u8, out: [] u8) -> [] u8 {
  #assert bitcoin_reverse32(digest, out[0..<32]) == SHA_OK;
  return out[0..<32];
}
// Both IDs and the weight of a transaction, read with witnesses allowed.
Ids :: struct { txid: [32] u8; wtxid: [32] u8; weight: int; }
ids :: (b: [] u8) -> Ids {
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem; r: Ids;
  t := tx_read_vartime(b, true, ins[..], outs[..], items[..]);
  #assert tx_txid_vartime(b, t, r.txid[..]) && tx_wtxid_vartime(b, t, r.wtxid[..]);
  r.weight = tx_weight_vartime(t);
  return r;
}
// Print one form of a transaction: its txid as shown, and whether the
// signature passes BIP-66 and the low-S rule. Returns the txid.
form :: (label: string, b: [] u8, sig: [] u8) -> [32] u8 {
  d := ids(b); x: [32] u8;
  print("%\n", label);
  show("  txid", shown(d.txid[..], x[..]));
  print("  strict DER %, low S %\n", der_check_vartime(sig) == DER_OK, der_low_s_vartime(sig));
  return d.txid;
}
// Add id to the first count of seen unless it is there; the new count.
note :: (seen: [] [32] u8, count: int, given: [32] u8) -> int {
  id := given;
  for i: 0..<count { if equal_bytes(seen[i][..], id[..]) { return count; } }
  seen[count] = id;
  return count + 1;
}
// The 23b397ed spend with a new scriptSig: dummy (1 or 2 bytes), then sig
// pushed in the shortest form or with OP_PUSHDATA1, after an extra OP_0
// when extra is true.
spend :: (s: [] u8, dummy: [] u8, sig: [] u8, minimal: bool, extra: bool, out: [] u8) -> [] u8 {
  script: [80] u8; at := 0;
  if extra { script[0] = 0x00; at = 1; }
  for i: 0..<dummy.count { script[at+i] = dummy[i]; }
  at = mal_push_vartime(sig, minimal, script[..], at + dummy.count);
  n := mal_replace_vartime(s, 41, script[0..<at], out);
  #assert n > 0;
  return out[0..<n];
}

main :: () {
  b: [512] u8; o: [600] u8; x: [32] u8; y: [32] u8;
  seen: [16] [32] u8; count := 0;
  op0 := [1] u8.[0x00]; op1 := [1] u8.[0x51];

  // ---- third parties, before segwit: one legacy spend, many txids
  print("23b397ed: a 1-of-2 bare multisig spend, as mined in 2011\n");
  s := decode(TX_MAINNET[0].tx, b[..]);
  sig := s[44..<44+71];                         // after OP_0 and the push byte 0x47
  count = note(seen[..], count, form("as mined", s, sig));
  core := [4] string.["a byte stuffed into the signature", "dummy 0x01ff instead of OP_0", "dummy OP_1", "dummy OP_1NEGATE"];
  print("tx_valid.json's four changed forms, valid without BIP-66 and NULLDUMMY\n");
  for k: 1..<5 {
    c: [512] u8; t := decode(TX_CORE[k].tx, c[..]);
    d := ids(t);
    print("  %\n", core[k-1]);
    show("    txid", shown(d.txid[..], x[..]));
    count = note(seen[..], count, d.txid);
  }
  strict_sig: [80] u8; low: [80] u8; w: [600] u8;
  ns := mal_reencode_vartime(sig, false, strict_sig[..]);
  nl := mal_reencode_vartime(sig, true, low[..]);
  print("built here from the mined form\n");
  count = note(seen[..], count, form("strict DER, the same s", spend(s, op0[..], strict_sig[0..<ns], true, false, o[..]), strict_sig[0..<ns]));
  count = note(seen[..], count, form("strict DER, s replaced by n - s: the canonical form", spend(s, op0[..], low[0..<nl], true, false, o[..]), low[0..<nl]));
  count = note(seen[..], count, form("canonical, dummy OP_1", spend(s, op1[..], low[0..<nl], true, false, o[..]), low[0..<nl]));
  count = note(seen[..], count, form("canonical, the signature pushed with OP_PUSHDATA1", spend(s, op0[..], low[0..<nl], false, false, o[..]), low[0..<nl]));
  count = note(seen[..], count, form("canonical, an extra OP_0 first", spend(s, op0[..], low[0..<nl], true, true, o[..]), low[0..<nl]));
  print("% forms, % different txids\n", 10, count);

  // ---- third parties after segwit: a P2WPKH spend
  m := TX_MAINNET[3]; spent := TX_SPENT[0];
  print("f5864806: a P2WPKH spend\n");
  s = decode(m.tx, b[..]);
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  sig = s[items[0].at..<items[0].at+items[0].n];
  key := s[items[1].at..<items[1].at+items[1].n];
  // The BIP-143 digest: the stripped transaction, the P2PKH script code of
  // the key hash in the spent output, and the spent value.
  n := tx_write_vartime(s, t, ins[..], outs[..], items[..], false, w[..]);
  bt := bt_tx_read(w[0..<n]);
  spk: [40] u8; p2pkh: [25] u8; h: [32] u8;
  hash := decode(spent.scripts[0], spk[..])[2..<22];
  p2pkh[0] = 0x76; p2pkh[1] = 0xa9; p2pkh[2] = 0x14; p2pkh[23] = 0x88; p2pkh[24] = 0xac;
  for i: 0..<20 { p2pkh[3+i] = hash[i]; }
  #assert bt_sighash_v0(w[0..<n], bt, 0, p2pkh[..], spent.values[0], BT_ALL, h[..]);
  print("spends % satoshis; BIP-143 digest computed\n", spent.values[0]);
  print("signature: % bytes, verifies %, low S %\n", sig.count, der_checksig_vartime(h[..], key, sig), der_low_s_vartime(sig));
  high: [80] u8;
  nh := mal_reencode_vartime(sig, true, high[..]);
  print("s replaced by n - s: % bytes, verifies %, low S %\n", nh, der_checksig_vartime(h[..], key, high[0..<nh]), der_low_s_vartime(high[0..<nh]));
  before := ids(s);
  n = mal_replace_vartime(s, items[0].at - 1, high[0..<nh], o[..]);
  after := ids(o[0..<n]);
  show("  txid ", shown(after.txid[..], x[..]));
  show("  wtxid", shown(after.wtxid[..], y[..]));
  print("  txid unchanged %, wtxid unchanged %, weight % -> %\n", equal_bytes(before.txid[..], after.txid[..]),
    equal_bytes(before.wtxid[..], after.wtxid[..]), before.weight, after.weight);
  n = mal_replace_vartime(s, ins[0].script - 1, op0[..], o[..]);
  after = ids(o[0..<n]);
  print("OP_0 in the empty scriptSig: txid unchanged %; BIP-141 requires it empty\n", equal_bytes(before.txid[..], after.txid[..]));

  // ---- the signer: BIP-143's P2SH-P2WPKH example, signed twice
  v := BT_V143[1];
  print("BIP-143's P2SH-P2WPKH example, signed twice with its published key\n");
  u := decode(v.tx, b[..]);
  #assert tx_read_vartime(u, false, ins[..], outs[..], items[..]).code == TX_OK;
  prefix := ins[0].script - 1;              // the scriptSig's length byte
  bu := bt_tx_read(u);
  kb: [32] u8; c: [32] u8; pub: [33] u8; kh: [20] u8;
  k := decode(v.key, kb[..]);
  code := decode(v.code, c[..])[1..<26];
  #assert se_encode(smb_multiply_web(sc_load(k).value), SE_COMPRESSED, pub[..]) == SE_COMPRESSED;
  #assert hash160_checked(pub[..], kh[..], false) == RMD_OK;
  print("the key's HASH160 is the script code's: %\n", equal_bytes(kh[..], code[3..<23]));
  types := [2] int.[BT_ALL, BT_ALL | BT_ANYONE];
  names := [2] string.["SIGHASH_ALL", "SIGHASH_ALL|ANYONECANPAY"];
  sigs: [2] [80] u8; lens: [2] int; wtx: [2] Ids; legacy: [2] Ids;
  for i: 0..<2 {
    d: [32] u8; rs: [64] u8; wk: [1] EcdsaWork; e: [80] u8;
    #assert bt_sighash_v0(u, bu, 0, code, v.amount, types[i], d[..]);
    #assert ecdsa_sign(k, d[..], wk[..], rs[..], false);
    lens[i] = ecdsa_der(rs[..], sigs[i][..]) + 1;
    sigs[i][lens[i]-1] = cast(u8) types[i];
    one := sigs[i][0..<lens[i]];
    print("%: % bytes, verifies %", names[i], lens[i], der_checksig_vartime(d[..], pub[..], one));
    if i == 0 { print(", BIP-143's signature %", equal_bytes(one[0..<lens[i]-1], decode(v.sig, e[..]))); }
    print("\n");
    // As P2SH-P2WPKH: the scriptSig pushes the redeem script 0014<hash>;
    // the witness is the signature and the key.
    redeem: [22] u8; script: [24] u8; stack: [120] u8; p: [600] u8;
    redeem[0] = 0x00; redeem[1] = 0x14;
    for j: 0..<20 { redeem[2+j] = kh[j]; }
    q := mal_replace_vartime(u, prefix, script[0..<mal_push_vartime(redeem[..], true, script[..], 0)], p[..]);
    r := tx_read_vartime(p[0..<q], false, ins[..], outs[..], items[..]);
    stack[0] = 2; at := mal_push_vartime(one, true, stack[..], 1); at = mal_push_vartime(pub[..], true, stack[..], at);
    wn := mal_add_witness_vartime(p[0..<q], r, stack[0..<at], w[..]);
    wtx[i] = ids(w[0..<wn]);
    // As if the coin were P2PKH: the signature and the key in the scriptSig.
    at = mal_push_vartime(one, true, stack[..], 0); at = mal_push_vartime(pub[..], true, stack[..], at);
    q = mal_replace_vartime(u, prefix, stack[0..<at], p[..]);
    legacy[i] = ids(p[0..<q]);
  }
  print("as P2SH-P2WPKH\n");
  for i: 0..<2 { show("  txid ", shown(wtx[i].txid[..], x[..])); show("  wtxid", shown(wtx[i].wtxid[..], y[..])); }
  print("  same txid %, same wtxid %\n", equal_bytes(wtx[0].txid[..], wtx[1].txid[..]), equal_bytes(wtx[0].wtxid[..], wtx[1].wtxid[..]));
  print("as if the coin were P2PKH\n");
  for i: 0..<2 { show("  txid ", shown(legacy[i].txid[..], x[..])); }
  print("  same txid %\n", equal_bytes(legacy[0].txid[..], legacy[1].txid[..]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/malleability.coax --run-steps 250000000
```

The output:

```text
23b397ed: a 1-of-2 bare multisig spend, as mined in 2011
as mined
  txid: 23b397edccd3740a74adb603c9756370fafcde9bcc4483eb271ecad09a94dd63
  strict DER false, low S false
tx_valid.json's four changed forms, valid without BIP-66 and NULLDUMMY
  a byte stuffed into the signature
    txid: fcabc409d8e685da28536e1e5ccc91264d755cd4c57ed4cae3dbaa4d3b93e8ed
  dummy 0x01ff instead of OP_0
    txid: c9aa95f2c48175fdb70b34c23f1c3fc44f869b073a6f79b1343fbce30c3cb575
  dummy OP_1
    txid: da94fda32b55deb40c3ed92e135d69df7efc4ee6665e0beb07ef500f407c9fd2
  dummy OP_1NEGATE
    txid: f76f897b206e4f78d60fe40f2ccb542184cfadc34354d3bb9bdc30cc2f432b86
built here from the mined form
strict DER, the same s
  txid: cf75b86dc018b6221f6737911e2ef18683a54c41faee1dd8bf53923e5702971f
  strict DER true, low S false
strict DER, s replaced by n - s: the canonical form
  txid: fd4b7c3593d43a831471b34958dd0da9a7ca50c0a4b2c1667d4baddbaae175a1
  strict DER true, low S true
canonical, dummy OP_1
  txid: 77013068c90bcce4969df420774c108d6a1a2adcd1d6e37f8616584f1232703d
  strict DER true, low S true
canonical, the signature pushed with OP_PUSHDATA1
  txid: 34bf28fcc1adcd52987f7a06976a431c8b970ee3af400b76ef101f0ab147e080
  strict DER true, low S true
canonical, an extra OP_0 first
  txid: 6b6e7dbf7eca76ab49c9b6c6222201f0b5d68e1d1a36419ae32cc7403d5a155e
  strict DER true, low S true
10 forms, 10 different txids
f5864806: a P2WPKH spend
spends 511884 satoshis; BIP-143 digest computed
signature: 72 bytes, verifies true, low S true
s replaced by n - s: 73 bytes, verifies true, low S false
  txid : f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206
  wtxid: 0e9d5bd032d7fbe4cf95cd47821ce9226a54036d3385c2276176d3eb9b4b03a6
  txid unchanged true, wtxid unchanged false, weight 442 -> 443
OP_0 in the empty scriptSig: txid unchanged false; BIP-141 requires it empty
BIP-143's P2SH-P2WPKH example, signed twice with its published key
the key's HASH160 is the script code's: true
SIGHASH_ALL: 71 bytes, verifies true, BIP-143's signature true
SIGHASH_ALL|ANYONECANPAY: 72 bytes, verifies true
as P2SH-P2WPKH
  txid : ef48d9d0f595052e0f8cdcf825f7a5e50b6a388a81f206f3f4846e5ecd7a0c23
  wtxid: 680f483b2bf6c5dcbf111e69e885ba248a41a5e92070cfb0afec3cfc49a9fabb
  txid : ef48d9d0f595052e0f8cdcf825f7a5e50b6a388a81f206f3f4846e5ecd7a0c23
  wtxid: 5d2328c642651bbd0eda310233363cc05c72854de0e19cb2c2083bc66bbbdebe
  same txid true, same wtxid false
as if the coin were P2PKH
  txid : 81dcdd3729126c5cbf3a08ce8270c9944ae961f0b9ae80e6e88772ef21af3f36
  txid : 92b61525d296494d2850b401ce2cfaadd078f2f03a201525794056048d5a117f
  same txid false
```

What the output shows:

- The mined form of `23b397ed` fails BIP-66, and its `s` is high.
- Core's four changed forms each have their own txid.
- Re-encoding in strict DER makes the signature pass BIP-66 but not the
  low-S rule; `n − s` passes both. The canonical form then still has
  three more variants that only `NULLDUMMY` or policy refuses. Ten forms,
  ten txids.
- `f5864806`'s signature verifies against the digest computed here. Its
  high-S twin verifies too and is refused only by policy. It changes the
  wtxid, not the txid. A byte in the scriptSig changes the txid, and
  BIP-141 refuses the result.
- The signer's two signatures of BIP-143's example share a txid as
  P2SH-P2WPKH and have different txids as P2PKH.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/malleability.out](../fixtures/malleability.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/malleability.coax](../examples/checks/malleability.coax),
which loads the example:

- Core's four changed forms, rebuilt from the mined form byte for byte;
- the six forms built from the mined one, each with its own txid, with
  the signature checks of BIP-66 and the low-S rule;
- `f5864806`: its signature verifies and fails for a changed digest, the
  high-S form verifies and fails the low-S rule, it moves the wtxid and
  the weight by one, and a scriptSig byte moves the txid.

<!-- source: bitcoin/examples/checks/malleability.coax check -->
```jai
// tx_valid.json's four changed forms of 23b397ed are the mined form with a
// new scriptSig: a byte before the sighash byte, or another dummy.
#test "Core's changed forms, rebuilt from the mined one" #steps 100000000 {
  b: [512] u8; c: [512] u8; o: [600] u8; e: [80] u8;
  s := decode(TX_MAINNET[0].tx, b[..]);
  sig := s[44..<44+71];
  for i: 0..<70 { e[i] = sig[i]; }
  e[70] = 0xab; e[71] = 0x01;
  op0 := [1] u8.[0x00]; ff := [2] u8.[0x01, 0xff]; op1 := [1] u8.[0x51]; neg := [1] u8.[0x4f];
  #assert equal_bytes(spend(s, op0[..], e[0..<72], true, false, o[..]), decode(TX_CORE[1].tx, c[..]));
  #assert equal_bytes(spend(s, ff[..], sig, true, false, o[..]), decode(TX_CORE[2].tx, c[..]));
  #assert equal_bytes(spend(s, op1[..], sig, true, false, o[..]), decode(TX_CORE[3].tx, c[..]));
  #assert equal_bytes(spend(s, neg[..], sig, true, false, o[..]), decode(TX_CORE[4].tx, c[..]));
  #assert equal_bytes(spend(s, op0[..], sig, true, false, o[..]), s);
}
// Each form built here has its own txid, and none is the mined one. A
// legacy transaction's wtxid is its txid.
#test "every form of 23b397ed has its own txid" #steps 200000000 {
  b: [512] u8; o: [600] u8; low: [80] u8; same: [80] u8; seen: [8] [32] u8;
  s := decode(TX_MAINNET[0].tx, b[..]);
  sig := s[44..<44+71];
  nl := mal_reencode_vartime(sig, true, low[..]); ns := mal_reencode_vartime(sig, false, same[..]);
  #assert der_signature_check_vartime(low[0..<nl], SCRIPT_DERSIG | SCRIPT_LOW_S | SCRIPT_STRICTENC) == SIG_OK;
  #assert der_signature_check_vartime(same[0..<ns], SCRIPT_DERSIG) == SIG_OK && der_signature_check_vartime(same[0..<ns], SCRIPT_LOW_S) == SIG_HIGH_S;
  #assert der_signature_check_vartime(sig, SCRIPT_DERSIG) == SIG_DER;
  op0 := [1] u8.[0x00]; op1 := [1] u8.[0x51];
  count := note(seen[..], 0, ids(s).txid);
  count = note(seen[..], count, ids(spend(s, op0[..], same[0..<ns], true, false, o[..])).txid);
  count = note(seen[..], count, ids(spend(s, op0[..], low[0..<nl], true, false, o[..])).txid);
  count = note(seen[..], count, ids(spend(s, op1[..], low[0..<nl], true, false, o[..])).txid);
  count = note(seen[..], count, ids(spend(s, op0[..], low[0..<nl], false, false, o[..])).txid);
  d := ids(spend(s, op0[..], low[0..<nl], true, true, o[..]));
  count = note(seen[..], count, d.txid);
  #assert count == 6 && equal_bytes(d.txid[..], d.wtxid[..]);
}
// The P2WPKH spend: its signature verifies, and so does the high-S form,
// which only the low-S policy refuses. The high-S form moves the wtxid and
// the weight by one; a byte in the scriptSig moves the txid.
#test "f5864806: n - s moves the wtxid, a scriptSig byte the txid" #steps 1000000000 {
  b: [512] u8; w: [600] u8; o: [600] u8; spk: [40] u8; code: [25] u8; h: [32] u8; high: [80] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(TX_MAINNET[3].tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  sig := s[items[0].at..<items[0].at+items[0].n]; key := s[items[1].at..<items[1].at+items[1].n];
  n := tx_write_vartime(s, t, ins[..], outs[..], items[..], false, w[..]);
  hash := decode(TX_SPENT[0].scripts[0], spk[..])[2..<22];
  code[0] = 0x76; code[1] = 0xa9; code[2] = 0x14; code[23] = 0x88; code[24] = 0xac;
  for i: 0..<20 { code[3+i] = hash[i]; }
  #assert bt_sighash_v0(w[0..<n], bt_tx_read(w[0..<n]), 0, code[..], TX_SPENT[0].values[0], BT_ALL, h[..]);
  #assert der_checksig_vartime(h[..], key, sig) && der_signature_check_vartime(sig, SCRIPT_DERSIG | SCRIPT_LOW_S | SCRIPT_STRICTENC) == SIG_OK;
  h[0] ^= 1;
  #assert !der_checksig_vartime(h[..], key, sig);
  h[0] ^= 1;
  nh := mal_reencode_vartime(sig, true, high[..]);
  #assert nh == 73 && der_checksig_vartime(h[..], key, high[0..<nh]) && der_signature_check_vartime(high[0..<nh], SCRIPT_LOW_S) == SIG_HIGH_S;
  before := ids(s);
  m := mal_replace_vartime(s, items[0].at - 1, high[0..<nh], o[..]);
  after := ids(o[0..<m]);
  #assert equal_bytes(before.txid[..], after.txid[..]) && !equal_bytes(before.wtxid[..], after.wtxid[..]) && after.weight == before.weight + 1;
  op0 := [1] u8.[0x00];
  m = mal_replace_vartime(s, ins[0].script - 1, op0[..], o[..]);
  after = ids(o[0..<m]);
  #assert !equal_bytes(before.txid[..], after.txid[..]) && after.weight == before.weight + 4;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/malleability.coax --run-steps 250000000
```

`coax test` runs 356 checks, including the loaded files' own checks, in
about 3 minutes on 2026-11-03 on a loaded machine.

### The library's own checks

`malleability.coax` is an `#authority` file. Its checks cover the
re-encoding of `23b397ed`'s signature both ways and back, its refusals
(an unreadable signature, `r` or `s` of zero, a short output), replacing
a witness item and a scriptSig with the same, shorter, longer and
253-byte fields, the push forms at 0, 1, 75, 76 and 255 bytes, and
rebuilding `f5864806` from its stripped form and its stacks.

```shell
coax/bin/coax test coax/lib/bitcoin/malleability.coax --run-steps 250000000
```

`coax test` runs 219 checks, including those of the files it loads, in
about 90 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Python

[malleability.py](../../coax/tests/bitcoin/malleability.py) builds
every form in Python that shares no code with Coax: transactions through
[tx_reference.py](../../coax/tests/bitcoin/tx_reference.py), signatures
through [der_reference.py](../../coax/tests/bitcoin/der_reference.py),
and its own BIP-143 digest and RFC 6979 signer. It reads the pinned
`tx_valid.json` and mainnet files, checks that Core's four rows are the
mined form with the scriptSigs the chapter names, and writes the whole
transcript. The transcript must equal the fixture.

```shell
python3 -I coax/tests/bitcoin/malleability.py --coax coax/bin/coax
```

On 2026-11-03 all 52 lines agree with Python, and the example's output
in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Running the scripts: chapter 36's interpreter applies `NULLDUMMY`,
  `MINIMALDATA` and `CLEANSTACK`. This chapter names the rule each edit
  breaks; it doesn't evaluate the script.
- The legacy signature hash that `23b397ed`'s signature signs: chapter
  37.
- The witness commitment that makes a block commit to wtxids: chapter
  49.
- Relay by wtxid (BIP-339): chapter 83.

## Summary

- The txid hashes the transaction without witnesses; the wtxid hashes
  it with them.
- A legacy signature lives in the scriptSig, which the txid hashes but
  the signature can't sign. Its encoding, its `s`, its push form, the
  multisig dummy and extra stack items could all change.
- BIP-66 and BIP-147 made two of those consensus rules; low-S, minimal
  pushes and clean stacks stayed policy; BIP-62 was withdrawn.
- Segwit moved signatures to the witness and fixed the scriptSig, so a
  transaction whose inputs are all segwit has a txid nobody can change,
  not even its signer. Changes reach only the wtxid.
- Malleability didn't empty Mt. Gox; at most 386 bitcoins could have
  been taken that way.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)

The chapter also uses [20. Sign with ECDSA](ch020-sign-with-ecdsa.md)
and [21. Strict DER](ch021-strict-der.md).

## Sources

- [BIP-62](https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki),
  [BIP-66](https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki),
  [BIP-146](https://github.com/bitcoin/bips/blob/master/bip-0146.mediawiki),
  [BIP-147](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki),
  [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  and [BIP-143](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki).
- Bitcoin Core at revision `ddea9196`: `tx_valid.json` and
  `script/interpreter.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/tx_valid.json).
- The mainnet transaction `f5864806` and the coin it spends, from
  mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.json)
  and copied into
  [spent_vectors.coax](../../coax/tests/bitcoin/spent_vectors.coax) by
  [make_spent_vectors.py](../../coax/tests/bitcoin/make_spent_vectors.py).
- Christian Decker and Roger Wattenhofer,
  [Bitcoin Transaction Malleability and MtGox](https://arxiv.org/abs/1403.6676),
  ESORICS 2014.
- Russell O'Connor on `23b397ed`'s signature:
  <https://r6.ca/blog/20111119T211504Z.html>.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
