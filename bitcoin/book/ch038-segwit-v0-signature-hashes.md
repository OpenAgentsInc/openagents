# 38. Segwit v0 signature hashes

Segwit v0 replaced the legacy signature hash with BIP-143's. It hashes a
fixed-size summary instead of an edited copy of the transaction: three
hashes of the whole transaction, then the input being signed with the
amount it spends. This chapter explains the summary field by field, works
through every signature hash BIP-143 publishes, and shows the two problems
it solves: quadratic hashing, and a signer that cannot see what it spends.

The hash is the one chapter 36's interpreter calls for P2WPKH and P2WSH.
This chapter splits it into the steps BIP-143 describes, so that the three
hashes are computed once for a transaction, and moves the P2WPKH
scriptCode into a procedure of its own.

## What this chapter teaches

- The ten fields BIP-143 hashes, and which of them each hash type zeroes.
- `hashPrevouts`, `hashSequence` and `hashOutputs`, and why computing them
  once per transaction makes the hashing linear in the number of inputs.
- The amount: a signature commits to the value of the coin it spends, so
  a hardware wallet can trust an amount it cannot check.
- The scriptCode of P2WPKH, and of P2WSH with `OP_CODESEPARATOR`.
- What BIP-143 leaves out of the legacy hash: FindAndDelete, the
  `SIGHASH_SINGLE` bug, and the input index under `SINGLE|ANYONECANPAY`.
- All thirteen signature hashes in BIP-143's examples, with their
  intermediate hashes.

## Where Bitcoin uses the BIP-143 hash

Every `CHECKSIG` and `CHECKMULTISIG` in a version 0 witness program hashes
this way, whether the program is native or wrapped in P2SH.

| Where | What it hashes | Chapter |
| --- | --- | --- |
| The script interpreter | every P2WPKH and P2WSH signature check | 36 |
| Block validation | every segwit v0 input of every block | 55, 57 |
| A wallet or PSBT signer | each segwit v0 input it signs | 94, 99 |
| Lightning | the funding output's 2-of-2, commitment and HTLC transactions | 108 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_v0_hashes_vartime(sp)` | the three hashes of a transaction, Core's `PrecomputedTransactionData` |
| | `script_v0_select_vartime(sp, pre, hashtype)` | the three a hash type signs |
| | `script_sighash_v0_from_vartime(sp, pre, code, hashtype, out)` | the hash from them |
| | `script_sighash_v0_vartime(sp, code, hashtype, out)` | all of it for one signature, as the interpreter calls it |
| | `script_p2wpkh_code_vartime(program, out)` | the P2WPKH scriptCode |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sighash_v0` | a second BIP-143 hash over unsigned transactions, for Nostr chapter 104's signer |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_read_vartime`, `tx_absorb` | the transaction's fields and piecewise SHA-256 (chapter 34) |

`sighash.coax` keeps its own hash because its reader takes unsigned
transactions of at most 16 inputs into fixed room, for a signer with no
interpreter. Both give BIP-143's digests.

## The summary

BIP-143 hashes ten fields with SHA-256d:

| # | Field | Size | Under the hash type |
| --- | --- | --- | --- |
| 1 | version | 4 | always |
| 2 | `hashPrevouts` | 32 | zero under `ANYONECANPAY` |
| 3 | `hashSequence` | 32 | zero under `ANYONECANPAY`, `NONE` or `SINGLE` |
| 4 | this input's outpoint | 36 | always |
| 5 | scriptCode, with its length | varies | always |
| 6 | the amount this input spends | 8 | always |
| 7 | this input's sequence | 4 | always |
| 8 | `hashOutputs` | 32 | zero under `NONE`; under `SINGLE`, the hash of this input's output, or zero if there is none |
| 9 | lock time | 4 | always |
| 10 | hash type | 4 | always |

The three hashes are SHA-256d of every input's outpoint, every input's
sequence and every output, serialized as in the transaction. None of them
depends on the input being signed or on the hash type, so the code
computes them once:

<!-- source: coax/lib/bitcoin/script.coax bip143 -->
```jai
// The three hashes of BIP-143, Core's PrecomputedTransactionData: the
// SHA-256d of every input's outpoint, of every input's sequence, and of
// every output. They depend on neither the input nor the hash type, so a
// transaction needs them once, however many inputs it signs.
ScriptV0Hashes :: struct { prevouts: [32] u8; sequences: [32] u8; outputs: [32] u8; }
script_v0_hashes_vartime :: (sp: ScriptSpend) -> ScriptV0Hashes {
  b := sp.b; t := sp.t;
  h: ScriptV0Hashes; part: [SHA_STATE_BYTES] u8;
  si_start(part[..]);
  for k: 0..<t.nin { si_absorb(part[..], b[sp.ins[k].outpoint..<sp.ins[k].outpoint+36]); }
  si_finish(part[..], h.prevouts[..]);
  si_start(part[..]);
  for k: 0..<t.nin { si_absorb(part[..], b[sp.ins[k].sequence..<sp.ins[k].sequence+4]); }
  si_finish(part[..], h.sequences[..]);
  si_start(part[..]);
  for k: 0..<t.nout { si_absorb(part[..], b[sp.outs[k].value..<sp.outs[k].script+sp.outs[k].script_n]); }
  si_finish(part[..], h.outputs[..]);
  return h;
}
// The three hashes a signature of this hash type signs. ANYONECANPAY
// zeroes hashPrevouts; ANYONECANPAY, NONE and SINGLE zero hashSequence;
// NONE zeroes hashOutputs, and SINGLE replaces it with the hash of the
// output at this input's index, or zeros when there is none.
script_v0_select_vartime :: (sp: ScriptSpend, pre: ScriptV0Hashes, hashtype: int) -> ScriptV0Hashes {
  b := sp.b; index := sp.index;
  anyone := (hashtype & 0x80) != 0;
  single := (hashtype & 0x1f) == 3; none := (hashtype & 0x1f) == 2;
  h := pre;
  zero: [32] u8;
  if anyone { h.prevouts = zero; }
  if anyone || single || none { h.sequences = zero; }
  if none || (single && index >= sp.t.nout) { h.outputs = zero; }
  else if single {
    part: [SHA_STATE_BYTES] u8; si_start(part[..]);
    si_absorb(part[..], b[sp.outs[index].value..<sp.outs[index].script+sp.outs[index].script_n]);
    si_finish(part[..], h.outputs[..]);
  }
  return h;
}
// The BIP-143 signature hash (SigVersion::WITNESS_V0) from the three
// hashes: the version, the selected hashes around this input's outpoint,
// scriptCode, amount and sequence, the lock time and the hash type.
script_sighash_v0_from_vartime :: (sp: ScriptSpend, pre: ScriptV0Hashes, code: [] u8, hashtype: int, out: [] u8) {
  b := sp.b; t := sp.t; index := sp.index;
  h := script_v0_select_vartime(sp, pre, hashtype);
  state: [SHA_STATE_BYTES] u8; si_start(state[..]);
  si_absorb(state[..], b[0..<4]);
  si_absorb(state[..], h.prevouts[..]);
  si_absorb(state[..], h.sequences[..]);
  si_absorb(state[..], b[sp.ins[index].outpoint..<sp.ins[index].outpoint+36]);
  si_absorb_compact(state[..], code.count);
  si_absorb(state[..], code);
  si_absorb_le(state[..], sp.amount, 8);
  si_absorb(state[..], b[sp.ins[index].sequence..<sp.ins[index].sequence+4]);
  si_absorb(state[..], h.outputs[..]);
  si_absorb(state[..], b[t.locktime..<t.locktime+4]);
  si_absorb_le(state[..], hashtype, 4);
  si_finish(state[..], out);
}
// The same for one signature: the three hashes, then the hash. The
// interpreter calls this, so it computes a transaction's three hashes
// again for each signature it checks.
script_sighash_v0_vartime :: (sp: ScriptSpend, code: [] u8, hashtype: int, out: [] u8) {
  script_sighash_v0_from_vartime(sp, script_v0_hashes_vartime(sp), code, hashtype, out);
}
```

The hash type selects as in the legacy hash: the low five bits pick
`NONE` (2) or `SINGLE` (3), anything else behaves as `ALL`, and bit `0x80`
is `ANYONECANPAY`. Consensus still accepts undefined types, as chapter 37
explains.

The interpreter calls `script_sighash_v0_vartime` for each signature, so
it recomputes the three hashes each time. Core computes them once per
transaction in `PrecomputedTransactionData` and passes them to every
input's check. A caller of this library that signs or checks many inputs
of one transaction calls `script_v0_hashes_vartime` once and
`script_sighash_v0_from_vartime` for each input, as the example does.

## The P2WPKH scriptCode

A P2WPKH output is `OP_0` and a push of a 20-byte key hash. Its witness is
a signature and a key, and the script it runs is the P2PKH script of the
same hash, which is also the scriptCode its signature hashes: BIP-143's
`0x1976a914{20-byte-pubkey-hash}88ac`. The interpreter builds it with this
procedure:

<!-- source: coax/lib/bitcoin/script.coax p2wpkh -->
```jai
// The script a P2WPKH program runs, and the scriptCode its signature
// hashes (BIP-143): DUP HASH160 <program> EQUALVERIFY CHECKSIG, the P2PKH
// script of the same 20-byte key hash. Writes its 25 bytes to out.
script_p2wpkh_code_vartime :: (program: [] u8, out: [] u8) {
  out[0] = OP_DUP; out[1] = OP_HASH160; out[2] = 20;
  for i: 0..<20 { out[3+i] = program[i]; }
  out[23] = OP_EQUALVERIFY; out[24] = OP_CHECKSIG;
}
```

For BIP-143's first example, the scriptPubKey
`00141d0f172a0ecb48aee1be1f2687d2963ae33f71a1` gives the scriptCode
`1976a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac`. P2SH-P2WPKH, where
the redeem script is the witness program, uses the same scriptCode.

For P2WSH the scriptCode is the witness script from just after the last
`OP_CODESEPARATOR` that ran. Unlike the legacy hash, the separators left
in it stay, and FindAndDelete isn't applied, so the scriptCode is exactly
a suffix of the script.

## The amount

A legacy signature commits to the outpoint it spends but not to the
value of that coin. A signer that only has the spending transaction
cannot tell how much it spends, and therefore how much fee it pays. An
offline signer, such as a hardware wallet, had to receive every previous
transaction and hash each one to check its outputs' values.

BIP-143 puts the amount in the hash. If a compromised computer tells the
wallet a wrong amount, the wallet signs a hash no node will compute, and
the signature is invalid. The example hashes input 1 of BIP-143's first
transaction with 600,000,000 satoshis and with one satoshi more:

```text
BIP-143, 600000000 sat: c37af31116d1b27caf68aae9e3ac82f1477929014d5b917657d0eb49478cb670
BIP-143, 600000001 sat: bef2e3e3d672fd4cac54808b91f6a91bf0511368de0eb1b32bea16a74318c611
legacy, 600000000 sat: c46030820cbc48402a47cc5b5d3d41648f4e3a711f56b804d601d09dc112a6a4
legacy, 600000001 sat: c46030820cbc48402a47cc5b5d3d41648f4e3a711f56b804d601d09dc112a6a4
```

The guarantee covers one input. Each signature commits to its own
input's amount only, so a wallet asked to sign a two-input transaction
twice, each time with the other input's amount understated, produces two
valid signatures for a transaction that pays far more fee than either
session showed. This was published in 2020, and wallets went back to
asking for the previous transactions of segwit v0 inputs. BIP-341
closes it for Taproot: every signature commits to the amounts and
scriptPubKeys of all inputs (chapter 43).

## Linear hashing

A legacy input hashes about the whole transaction (chapter 37). A
BIP-143 input hashes 182 bytes for P2WPKH, whatever the transaction's
size: 4 + 32 + 32 + 36 + 26 + 8 + 4 + 32 + 4 + 4. The three hashes read
36 bytes per input, 4 bytes per input and the outputs, once. For n
inputs and two 34-byte outputs, the bytes the first SHA-256 reads are:

| Inputs | Legacy (P2PKH) | BIP-143 (P2WPKH) | Of which once |
| --- | --- | --- | --- |
| 1 | 148 | 290 | 108 |
| 10 | 5,170 | 2,288 | 468 |
| 100 | 420,700 | 22,268 | 4,068 |
| 1,000 | 41,109,000 | 222,068 | 40,068 |
| 5,000 | 1,025,545,000 | 1,110,068 | 200,068 |
| 6,756 | 1,872,121,380 | 1,499,900 | 270,308 |

At 6,756 inputs the legacy hash reads 1,248 times as many bytes. The
BIP-143 total is 222 bytes per input plus 68, a straight line.

## What BIP-143 changes beyond the summary

- No FindAndDelete. The scriptCode is hashed as it is, even if it pushes
  the signature that signs it.
- `OP_CODESEPARATOR`: everything up to and including the last executed
  separator is removed, and separators after it stay in the scriptCode.
- `SIGHASH_SINGLE` with no output at the input's index hashes zeros for
  `hashOutputs` and signs the transaction, instead of signing the number
  1. Core's `SignatureHash` returns 1 only for a non-segwit hash.
- `SINGLE|ANYONECANPAY` commits to no index: the outpoint, the amount and
  the one output are hashed, but not where they sit. Pairs of inputs and
  outputs can move together to other indexes and keep their signatures.
  Without `ANYONECANPAY`, `hashPrevouts` and the outpoint fix the index.

## Worked example: BIP-143's signature hashes

BIP-143 publishes thirteen signature hashes in five examples:

| Example | Inputs signed | Hash types | What it shows |
| --- | --- | --- | --- |
| Native P2WPKH | input 1 of 2 | `ALL` | the P2WPKH scriptCode, and the three hashes |
| P2SH-P2WPKH | input 0 of 1 | `ALL` | the same scriptCode inside P2SH |
| Native P2WSH, first transaction | input 1 of 2, two keys | `SINGLE` | separators, and `SINGLE` past the only output |
| Native P2WSH, second transaction | inputs 0 and 1 | `SINGLE\|ANYONECANPAY` | a separator in an `IF` not taken, and no index |
| P2SH-P2WSH | input 0 of 1, a 6-of-6 | all six | each hash type's three hashes |
| No FindAndDelete | input 0 of 1 | `ALL` | a witness script that pushes its own signature |

The library's vectors (`sighash_vectors.coax`) hold eight of them. The
example copies the other five from the pinned BIP text, then prints each
hash's three selected hashes and the result. Every one matches what
BIP-143 prints, among them:

- Native P2WPKH: `hashPrevouts` `96b827c8…4efd37`, `hashSequence`
  `52b0a642…339a3b`, `hashOutputs` `863ef3e1…47e5e5`, and the hash
  `c37af311…8cb670`.
- The first Native P2WSH transaction has one output and signs input 1
  with `SINGLE`, so `hashOutputs` is zero. Of its two scriptCodes, the
  first keeps the separator that hadn't run yet, and the second starts
  after it.
- In the second Native P2WSH transaction, input 0's script is
  `0 IF CODESEPARATOR ENDIF <key> CHECKSIG`. The separator doesn't run, so
  the scriptCode is the whole script. Input 1's is `1 IF …`, the separator
  runs, and the scriptCode is `ENDIF <key> CHECKSIG`.

Two signed transactions from the BIP then run through chapter 36's
`VerifyScript` under `P2SH` and `WITNESS`:

- The second Native P2WSH transaction with its two input-output pairs
  swapped. Both `SINGLE|ANYONECANPAY` signatures still verify.
- The No FindAndDelete spend, whose witness script is
  `CHECKSIGVERIFY <signature>`. It verifies, and the legacy hash's
  FindAndDelete would have removed that push of the signature. With one
  more satoshi of amount it fails `CHECKSIGVERIFY`.

## The example

The example is
[bitcoin/examples/segwit-sighash.coax](../examples/segwit-sighash.coax).

<!-- source: bitcoin/examples/segwit-sighash.coax example -->
```jai
#load "../../coax/lib/bitcoin/script.coax";
#import "Basic";

// A transaction read into fixed room, with witnesses.
Spend :: struct {
  b: [4096] u8; spk: [64] u8;
  ins: [8] TxIn; outs: [8] TxOut; items: [16] TxItem;
}
spend_of :: (s: [] Spend, tx: string, index: int, amount: int) -> ScriptSpend {
  b := tx_unhex(tx, s[0].b[..]);
  t := tx_read_vartime(b, true, s[0].ins[..], s[0].outs[..], s[0].items[..]);
  #assert t.code == TX_OK;
  return ScriptSpend.{b=b, t=t, ins=s[0].ins[0..<t.nin], outs=s[0].outs[0..<t.nout], items=s[0].items[0..<t.nitems],
    index=index, amount=amount, spk=b[0..<0]};
}
show_hex :: (b: [] u8) {
  digits := "0123456789abcdef";
  for k: 0..<b.count {
    pair: [2] u8;
    pair[0] = digits[b[k] >> 4]; pair[1] = digits[b[k] & 15];
    print("%", cast(string) pair[..]);
  }
}
same32 :: (a: [] u8, b: [] u8) -> bool {
  for k: 0..<32 { if a[k] != b[k] { return false; } }
  return true;
}

// BIP-143's other published signature hashes, copied from its "Native
// P2WSH" and "No FindAndDelete" examples
// (coax/tests/vectors/upstream/bip143/bip-0143.mediawiki). Each scriptCode
// is without its length byte.
Example :: struct { label: string; tx: string; index: int; code: string; amount: int; hashtype: int; sighash: string; }
P2WSH_A :: "0100000002fe3dc9208094f3ffd12645477b3dc56f60ec4fa8e6f5d67c565d1c6b9216b36e0000000000ffffffff0815cf020f013ed6cf91d29f4202e8a58726b1ac6c79da47c23d1bee0a6925f80000000000ffffffff0100f2052a010000001976a914a30741f8145e5acadf23f751864167f32e0963f788ac00000000";
P2WSH_B :: "0100000002e9b542c5176808107ff1df906f46bb1f2583b16112b95ee5380665ba7fcfc0010000000000ffffffff80e68831516392fcd100d186b3c2c7b95c80b53c77e77c35ba03a66b429a2a1b0000000000ffffffff0280969800000000001976a914de4b231626ef508c9a74a8517e6783c0546d6b2888ac80969800000000001976a9146648a8cd4531e1ec47f35916de8e259237294d1e88ac00000000";
// The same transaction signed, with its two input-output pairs swapped.
P2WSH_B_SWAPPED :: "0100000000010280e68831516392fcd100d186b3c2c7b95c80b53c77e77c35ba03a66b429a2a1b0000000000ffffffffe9b542c5176808107ff1df906f46bb1f2583b16112b95ee5380665ba7fcfc0010000000000ffffffff0280969800000000001976a9146648a8cd4531e1ec47f35916de8e259237294d1e88ac80969800000000001976a914de4b231626ef508c9a74a8517e6783c0546d6b2888ac024730440220032521802a76ad7bf74d0e2c218b72cf0cbc867066e2e53db905ba37f130397e02207709e2188ed7f08f4c952d9d13986da504502b8c3be59617e043552f506c46ff83275163ab68210392972e2eb617b2388771abe27235fd5ac44af8e61693261550447a4c3e39da98ac02483045022100f6a10b8604e6dc910194b79ccfc93e1bc0ec7c03453caaa8987f7d6c3413566002206216229ede9b4d6ec2d325be245c5b508ff0339bf1794078e20bfe0babc7ffe683270063ab68210392972e2eb617b2388771abe27235fd5ac44af8e61693261550447a4c3e39da98ac00000000";
P2WSH_B_SPK :: [2] string.["0020ba468eea561b26301e4cf69fa34bde4ad60c81e70f059f045ca9a79931004a4d", "0020d9bbfbe56af7c4b7f960a70d7ea107156913d9e5a26b0a71429df5e097ca6537"];
NO_FAD_TX :: "010000000169c12106097dc2e0526493ef67f21269fe888ef05c7a3a5dacab38e1ac8387f14c1d000000ffffffff0101000000000000000000000000";
NO_FAD_SIGNED :: "0100000000010169c12106097dc2e0526493ef67f21269fe888ef05c7a3a5dacab38e1ac8387f14c1d000000ffffffff01010000000000000000034830450220487fb382c4974de3f7d834c1b617fe15860828c7f96454490edd6d891556dcc9022100baf95feb48f845d5bfc9882eb6aeefa1bc3790e39f59eaa46ff7f15ae626c53e012102a9781d66b61fb5a7ef00ac5ad5bc6ffc78be7b44a566e3c87870e1079368df4c4aad4830450220487fb382c4974de3f7d834c1b617fe15860828c7f96454490edd6d891556dcc9022100baf95feb48f845d5bfc9882eb6aeefa1bc3790e39f59eaa46ff7f15ae626c53e0100000000";
NO_FAD_SPK :: "00209e1be07558ea5cc8e02ed1d80c0911048afad949affa36d5c3951e3159dbea19";
NO_FAD_SIG :: "30450220487fb382c4974de3f7d834c1b617fe15860828c7f96454490edd6d891556dcc9022100baf95feb48f845d5bfc9882eb6aeefa1bc3790e39f59eaa46ff7f15ae626c53e01";
MORE :: [5] Example.[
  Example.{label="native P2WSH, the key before CODESEPARATOR", tx=P2WSH_A, index=1, amount=4900000000, hashtype=3,
    code="21026dccc749adc2a9d0d89497ac511f760f45c47dc5ed9cf352a58ac706453880aeadab210255a9626aebf5e29c0e6538428ba0d1dcf6ca98ffdf086aa8ced5e0d0215ea465ac",
    sighash="82dde6e4f1e94d02c2b7ad03d2115d691f48d064e9d52f58194a6637e4194391"},
  Example.{label="native P2WSH, the key after CODESEPARATOR", tx=P2WSH_A, index=1, amount=4900000000, hashtype=3,
    code="210255a9626aebf5e29c0e6538428ba0d1dcf6ca98ffdf086aa8ced5e0d0215ea465ac",
    sighash="fef7bd749cce710c5c052bd796df1af0d935e59cea63736268bcbe2d2134fc47"},
  Example.{label="native P2WSH, CODESEPARATOR in an IF not taken", tx=P2WSH_B, index=0, amount=16777215, hashtype=0x83,
    code="0063ab68210392972e2eb617b2388771abe27235fd5ac44af8e61693261550447a4c3e39da98ac",
    sighash="e9071e75e25b8a1e298a72f0d2e9f4f95a0f5cdf86a533cda597eb402ed13b3a"},
  Example.{label="native P2WSH, CODESEPARATOR in an IF taken", tx=P2WSH_B, index=1, amount=16777215, hashtype=0x83,
    code="68210392972e2eb617b2388771abe27235fd5ac44af8e61693261550447a4c3e39da98ac",
    sighash="cd72f1f1a433ee9df816857fad88d8ebd97e09a75cd481583eb841c330275e54"},
  Example.{label="P2WSH without FindAndDelete", tx=NO_FAD_TX, index=0, amount=200000, hashtype=1,
    code="ad4830450220487fb382c4974de3f7d834c1b617fe15860828c7f96454490edd6d891556dcc9022100baf95feb48f845d5bfc9882eb6aeefa1bc3790e39f59eaa46ff7f15ae626c53e01",
    sighash="71c9cd9b2869b9c70b01b1f0360c148f42dee72297db312638df136f43311f23"},
];
LABELS :: [8] string.["native P2WPKH", "P2SH-P2WPKH", "P2SH-P2WSH 6-of-6", "P2SH-P2WSH 6-of-6", "P2SH-P2WSH 6-of-6",
  "P2SH-P2WSH 6-of-6", "P2SH-P2WSH 6-of-6", "P2SH-P2WSH 6-of-6"];
hashtype_name :: (hashtype: int) -> string {
  if hashtype == 1 { return "ALL"; } if hashtype == 2 { return "NONE"; } if hashtype == 3 { return "SINGLE"; }
  if hashtype == 0x81 { return "ALL|ANYONECANPAY"; } if hashtype == 0x82 { return "NONE|ANYONECANPAY"; }
  return "SINGLE|ANYONECANPAY";
}
// One published example: the three hashes this type signs, the hash, and
// whether it is BIP-143's.
show_example :: (s: [] Spend, label: string, tx: string, index: int, code: [] u8, amount: int, hashtype: int, want: string) -> bool {
  sp := spend_of(s, tx, index, amount);
  pre := script_v0_hashes_vartime(sp);
  h := script_v0_select_vartime(sp, pre, hashtype);
  out: [32] u8;
  script_sighash_v0_from_vartime(sp, pre, code, hashtype, out[..]);
  ok := tx_hex_is(out[..], want);
  print("  %, input %, %: %\n    hashPrevouts ", label, index, hashtype_name(hashtype), ifx ok then "matches" else "DIFFERS");
  show_hex(h.prevouts[..]); print("\n    hashSequence "); show_hex(h.sequences[..]);
  print("\n    hashOutputs  "); show_hex(h.outputs[..]); print("\n    sigHash      "); show_hex(out[..]); print("\n");
  return ok;
}

// Bytes the first SHA-256 reads, for n inputs and two 34-byte outputs. A
// legacy P2PKH input hashes about the whole transaction (chapter 37). A
// BIP-143 P2WPKH input hashes 182 bytes: the version, the three hashes,
// its outpoint, its 26-byte scriptCode, amount and sequence, the lock time
// and the hash type. The three hashes read 36 and 4 bytes per input and
// the 68 bytes of outputs, once for the transaction.
legacy_bytes :: (n: int) -> int { return n * (4 + tx_compact_size_vartime(n) + 41 * (n - 1) + 66 + 1 + 68 + 4 + 4); }
V0_INPUT :: 4 + 32 + 32 + 36 + 26 + 8 + 4 + 32 + 4 + 4;
v0_once :: (n: int) -> int { return 36 * n + 4 * n + 68; }

main :: () {
  s: [1] Spend; c: [512] u8; h: [32] u8;

  // Every published signature hash of BIP-143, worked through.
  print("BIP-143's examples\n");
  good := 0;
  for i: 0..<8 {
    v := BT_V143[i];
    code := tx_unhex(v.code, c[..]);
    if show_example(s[..], LABELS[i], v.tx, v.index, code[1..<code.count], v.amount, v.hashtype, v.sighash) { good += 1; }
  }
  for e: MORE {
    if show_example(s[..], e.label, e.tx, e.index, tx_unhex(e.code, c[..]), e.amount, e.hashtype, e.sighash) { good += 1; }
  }
  print("  % of % match\n", good, BT_V143.count + MORE.count);

  // The scriptCode of a P2WPKH input: its program in a P2PKH script.
  print("the P2WPKH scriptCode\n");
  spk := tx_unhex("00141d0f172a0ecb48aee1be1f2687d2963ae33f71a1", c[..]);
  code: [25] u8;
  script_p2wpkh_code_vartime(spk[2..<22], code[..]);
  print("  scriptPubKey 00141d0f172a0ecb48aee1be1f2687d2963ae33f71a1\n  scriptCode   19");
  show_hex(code[..]);
  print("\n");

  // The amount is signed: one satoshi more is another hash. A legacy hash
  // of the same input has no amount to change.
  print("the amount\n");
  sp := spend_of(s[..], BT_V143[0].tx, 1, 600000000);
  for amount: [2] int.[600000000, 600000001] {
    sp.amount = amount;
    script_sighash_v0_vartime(sp, code[..], 1, h[..]);
    print("  BIP-143, % sat: ", amount);
    show_hex(h[..]);
    print("\n");
  }
  for amount: [2] int.[600000000, 600000001] {
    sp.amount = amount;
    script_sighash_legacy_vartime(sp, code[..], 1, h[..]);
    print("  legacy, % sat: ", amount);
    show_hex(h[..]);
    print("\n");
  }

  // SINGLE|ANYONECANPAY signs no index: with the pairs swapped, both
  // signatures still verify. And FindAndDelete isn't applied: the witness
  // script pushes the signature that signs it.
  print("signed spends\n");
  w: [1] ScriptWork;
  flags := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS;
  for k: 0..<2 {
    sp = spend_of(s[..], P2WSH_B_SWAPPED, k, 16777215);
    sp.spk = tx_unhex(P2WSH_B_SPK[1 - k], s[0].spk[..]);
    print("  pairs swapped, input %: %\n", k, SCRIPT_ERR_NAMES[script_verify_vartime(sp, flags, w[..])]);
  }
  sp = spend_of(s[..], NO_FAD_SIGNED, 0, 200000);
  sp.spk = tx_unhex(NO_FAD_SPK, s[0].spk[..]);
  print("  without FindAndDelete: %\n", SCRIPT_ERR_NAMES[script_verify_vartime(sp, flags, w[..])]);
  sig: [80] u8;
  si_code(w[..], tx_unhex(MORE[4].code, c[..]), 0);
  print("    FindAndDelete would remove % push of the signature\n", si_find_delete(w[..], tx_unhex(NO_FAD_SIG, sig[..])));

  // One computation of the three hashes serves every input.
  print("the three hashes, once\n");
  sp = spend_of(s[..], BT_V143[0].tx, 0, 625000000);
  pre := script_v0_hashes_vartime(sp);
  for k: 0..<2 {
    sp.index = k;
    one: [32] u8;
    script_sighash_v0_from_vartime(sp, pre, code[..], 1, h[..]);
    script_sighash_v0_vartime(sp, code[..], 1, one[..]);
    print("  input %: % as computed alone\n", k, ifx same32(h[..], one[..]) then "the same" else "NOT the same");
  }

  // Linear against quadratic.
  print("bytes hashed, n inputs and 2 outputs\n");
  for n: [6] int.[1, 10, 100, 1000, 5000, 6756] {
    print("  n %: legacy %, BIP-143 % (% once, % per input)\n", n, legacy_bytes(n), v0_once(n) + n * V0_INPUT, v0_once(n), V0_INPUT);
  }
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/segwit-sighash.coax --run-steps 250000000
```

It prints:

```text
BIP-143's examples
  native P2WPKH, input 1, ALL: matches
    hashPrevouts 96b827c8483d4e9b96712b6713a7b68d6e8003a781feba36c31143470b4efd37
    hashSequence 52b0a642eea2fb7ae638c36f6252b6750293dbe574a806984b8e4d8548339a3b
    hashOutputs  863ef3e1a92afbfdb97f31ad0fc7683ee943e9abcf2501590ff8f6551f47e5e5
    sigHash      c37af31116d1b27caf68aae9e3ac82f1477929014d5b917657d0eb49478cb670
  P2SH-P2WPKH, input 0, ALL: matches
    hashPrevouts b0287b4a252ac05af83d2dcef00ba313af78a3e9c329afa216eb3aa2a7b4613a
    hashSequence 18606b350cd8bf565266bc352f0caddcf01e8fa789dd8a15386327cf8cabe198
    hashOutputs  de984f44532e2173ca0d64314fcefe6d30da6f8cf27bafa706da61df8a226c83
    sigHash      64f3b0f4dd2bb3aa1ce8566d220cc74dda9df97d8490cc81d89d735c92e59fb6
  P2SH-P2WSH 6-of-6, input 0, ALL: matches
    hashPrevouts 74afdc312af5183c4198a40ca3c1a275b485496dd3929bca388c4b5e31f7aaa0
    hashSequence 3bb13029ce7b1f559ef5e747fcac439f1455a2ec7c5f09b72290795e70665044
    hashOutputs  bc4d309071414bed932f98832b27b4d76dad7e6c1346f487a8fdbb8eb90307cc
    sigHash      185c0be5263dce5b4bb50a047973c1b6272bfbd0103a89444597dc40b248ee7c
  P2SH-P2WSH 6-of-6, input 0, NONE: matches
    hashPrevouts 74afdc312af5183c4198a40ca3c1a275b485496dd3929bca388c4b5e31f7aaa0
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  0000000000000000000000000000000000000000000000000000000000000000
    sigHash      e9733bc60ea13c95c6527066bb975a2ff29a925e80aa14c213f686cbae5d2f36
  P2SH-P2WSH 6-of-6, input 0, SINGLE: matches
    hashPrevouts 74afdc312af5183c4198a40ca3c1a275b485496dd3929bca388c4b5e31f7aaa0
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  9efe0c13a6b16c14a41b04ebe6a63f419bdacb2f8705b494a43063ca3cd4f708
    sigHash      1e1f1c303dc025bd664acb72e583e933fae4cff9148bf78c157d1e8f78530aea
  P2SH-P2WSH 6-of-6, input 0, ALL|ANYONECANPAY: matches
    hashPrevouts 0000000000000000000000000000000000000000000000000000000000000000
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  bc4d309071414bed932f98832b27b4d76dad7e6c1346f487a8fdbb8eb90307cc
    sigHash      2a67f03e63a6a422125878b40b82da593be8d4efaafe88ee528af6e5a9955c6e
  P2SH-P2WSH 6-of-6, input 0, NONE|ANYONECANPAY: matches
    hashPrevouts 0000000000000000000000000000000000000000000000000000000000000000
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  0000000000000000000000000000000000000000000000000000000000000000
    sigHash      781ba15f3779d5542ce8ecb5c18716733a5ee42a6f51488ec96154934e2c890a
  P2SH-P2WSH 6-of-6, input 0, SINGLE|ANYONECANPAY: matches
    hashPrevouts 0000000000000000000000000000000000000000000000000000000000000000
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  9efe0c13a6b16c14a41b04ebe6a63f419bdacb2f8705b494a43063ca3cd4f708
    sigHash      511e8e52ed574121fc1b654970395502128263f62662e076dc6baf05c2e6a99b
  native P2WSH, the key before CODESEPARATOR, input 1, SINGLE: matches
    hashPrevouts ef546acf4a020de3898d1b8956176bb507e6211b5ed3619cd08b6ea7e2a09d41
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  0000000000000000000000000000000000000000000000000000000000000000
    sigHash      82dde6e4f1e94d02c2b7ad03d2115d691f48d064e9d52f58194a6637e4194391
  native P2WSH, the key after CODESEPARATOR, input 1, SINGLE: matches
    hashPrevouts ef546acf4a020de3898d1b8956176bb507e6211b5ed3619cd08b6ea7e2a09d41
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  0000000000000000000000000000000000000000000000000000000000000000
    sigHash      fef7bd749cce710c5c052bd796df1af0d935e59cea63736268bcbe2d2134fc47
  native P2WSH, CODESEPARATOR in an IF not taken, input 0, SINGLE|ANYONECANPAY: matches
    hashPrevouts 0000000000000000000000000000000000000000000000000000000000000000
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  b258eaf08c39fbe9fbac97c15c7e7adeb8df142b0df6f83e017f349c2b6fe3d2
    sigHash      e9071e75e25b8a1e298a72f0d2e9f4f95a0f5cdf86a533cda597eb402ed13b3a
  native P2WSH, CODESEPARATOR in an IF taken, input 1, SINGLE|ANYONECANPAY: matches
    hashPrevouts 0000000000000000000000000000000000000000000000000000000000000000
    hashSequence 0000000000000000000000000000000000000000000000000000000000000000
    hashOutputs  91ea93dd77f702b738ebdbf3048940a98310e869a7bb8fa2c6cb3312916947ca
    sigHash      cd72f1f1a433ee9df816857fad88d8ebd97e09a75cd481583eb841c330275e54
  P2WSH without FindAndDelete, input 0, ALL: matches
    hashPrevouts b67c76d200c6ce72962d919dc107884b9d5d0e26f2aea7474b46a1904c53359f
    hashSequence 3bb13029ce7b1f559ef5e747fcac439f1455a2ec7c5f09b72290795e70665044
    hashOutputs  e5d196bfb21caca9dbd654cafb3b4dc0c4882c8927d2eb300d9539dd0b934228
    sigHash      71c9cd9b2869b9c70b01b1f0360c148f42dee72297db312638df136f43311f23
  13 of 13 match
the P2WPKH scriptCode
  scriptPubKey 00141d0f172a0ecb48aee1be1f2687d2963ae33f71a1
  scriptCode   1976a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac
the amount
  BIP-143, 600000000 sat: c37af31116d1b27caf68aae9e3ac82f1477929014d5b917657d0eb49478cb670
  BIP-143, 600000001 sat: bef2e3e3d672fd4cac54808b91f6a91bf0511368de0eb1b32bea16a74318c611
  legacy, 600000000 sat: c46030820cbc48402a47cc5b5d3d41648f4e3a711f56b804d601d09dc112a6a4
  legacy, 600000001 sat: c46030820cbc48402a47cc5b5d3d41648f4e3a711f56b804d601d09dc112a6a4
signed spends
  pairs swapped, input 0: OK
  pairs swapped, input 1: OK
  without FindAndDelete: OK
    FindAndDelete would remove 1 push of the signature
the three hashes, once
  input 0: the same as computed alone
  input 1: the same as computed alone
bytes hashed, n inputs and 2 outputs
  n 1: legacy 148, BIP-143 290 (108 once, 182 per input)
  n 10: legacy 5170, BIP-143 2288 (468 once, 182 per input)
  n 100: legacy 420700, BIP-143 22268 (4068 once, 182 per input)
  n 1000: legacy 41109000, BIP-143 222068 (40068 once, 182 per input)
  n 5000: legacy 1025545000, BIP-143 1110068 (200068 once, 182 per input)
  n 6756: legacy 1872121380, BIP-143 1499900 (270308 once, 182 per input)
```

What the output shows:

- All thirteen published hashes match, each computed from the three
  hashes. Every non-zero intermediate hash printed is one BIP-143 prints.
- The P2WPKH scriptCode is the program's P2PKH script with its length
  byte, `0x19`.
- One satoshi more changes the BIP-143 hash and not the legacy one.
- The swapped pairs and the witness script that pushes its own signature
  pass `VerifyScript`.
- One computation of the three hashes gives each input's hash, the same
  as computing it alone.
- The bytes hashed grow linearly: 1,499,900 at 6,756 inputs against
  legacy's 1,872,121,380.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/segwit-sighash.out](../fixtures/segwit-sighash.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/segwit-sighash.coax](../examples/checks/segwit-sighash.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/segwit-sighash.coax check -->
```jai
// Every published BIP-143 signature hash, from the three hashes computed
// once, and the same hash computed alone.
#forall "BIP-143's thirteen signature hashes" i: 0..<13 #steps 100000000 {
  s: [1] Spend; c: [512] u8; h: [32] u8; one: [32] u8;
  sp: ScriptSpend; code: [] u8; hashtype: int; want: string;
  if i < 8 {
    v := BT_V143[i];
    sp = spend_of(s[..], v.tx, v.index, v.amount);
    all := tx_unhex(v.code, c[..]);
    code = all[1..<all.count]; hashtype = v.hashtype; want = v.sighash;
  } else {
    e := MORE[i - 8];
    sp = spend_of(s[..], e.tx, e.index, e.amount);
    code = tx_unhex(e.code, c[..]); hashtype = e.hashtype; want = e.sighash;
  }
  script_sighash_v0_from_vartime(sp, script_v0_hashes_vartime(sp), code, hashtype, h[..]);
  script_sighash_v0_vartime(sp, code, hashtype, one[..]);
  #assert tx_hex_is(h[..], want) && same32(h[..], one[..]);
}
// The intermediate hashes BIP-143 publishes: P2SH-P2WPKH's three, SINGLE's
// one output in the 6-of-6, the zeros of SINGLE past the outputs, and the
// No FindAndDelete example's three.
#test "BIP-143's published intermediate hashes" #steps 100000000 {
  s: [1] Spend;
  sp := spend_of(s[..], BT_V143[1].tx, 0, BT_V143[1].amount);
  h := script_v0_select_vartime(sp, script_v0_hashes_vartime(sp), 1);
  #assert tx_hex_is(h.prevouts[..], "b0287b4a252ac05af83d2dcef00ba313af78a3e9c329afa216eb3aa2a7b4613a");
  #assert tx_hex_is(h.sequences[..], "18606b350cd8bf565266bc352f0caddcf01e8fa789dd8a15386327cf8cabe198");
  #assert tx_hex_is(h.outputs[..], "de984f44532e2173ca0d64314fcefe6d30da6f8cf27bafa706da61df8a226c83");
  sp = spend_of(s[..], BT_V143[4].tx, 0, BT_V143[4].amount);
  h = script_v0_select_vartime(sp, script_v0_hashes_vartime(sp), 3);
  #assert tx_hex_is(h.outputs[..], "9efe0c13a6b16c14a41b04ebe6a63f419bdacb2f8705b494a43063ca3cd4f708");
  sp = spend_of(s[..], P2WSH_A, 1, 4900000000);
  h = script_v0_select_vartime(sp, script_v0_hashes_vartime(sp), 3);
  #assert tx_hex_is(h.prevouts[..], "ef546acf4a020de3898d1b8956176bb507e6211b5ed3619cd08b6ea7e2a09d41");
  for k: 0..<32 { #assert h.sequences[k] == 0 && h.outputs[k] == 0; }
  sp = spend_of(s[..], NO_FAD_TX, 0, 200000);
  h = script_v0_select_vartime(sp, script_v0_hashes_vartime(sp), 1);
  #assert tx_hex_is(h.prevouts[..], "b67c76d200c6ce72962d919dc107884b9d5d0e26f2aea7474b46a1904c53359f");
  #assert tx_hex_is(h.sequences[..], "3bb13029ce7b1f559ef5e747fcac439f1455a2ec7c5f09b72290795e70665044");
  #assert tx_hex_is(h.outputs[..], "e5d196bfb21caca9dbd654cafb3b4dc0c4882c8927d2eb300d9539dd0b934228");
}
// The amount is in the BIP-143 hash and not in the legacy one; the P2WPKH
// scriptCode is the program's P2PKH script.
#test "the amount and the P2WPKH scriptCode" #steps 100000000 {
  s: [1] Spend; c: [32] u8; code: [25] u8; a: [32] u8; b: [32] u8;
  script_p2wpkh_code_vartime(tx_unhex("1d0f172a0ecb48aee1be1f2687d2963ae33f71a1", c[..]), code[..]);
  #assert tx_hex_is(code[..], "76a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac");
  sp := spend_of(s[..], BT_V143[0].tx, 1, 600000000);
  script_sighash_v0_vartime(sp, code[..], 1, a[..]);
  #assert tx_hex_is(a[..], BT_V143[0].sighash);
  sp.amount = 600000001;
  script_sighash_v0_vartime(sp, code[..], 1, b[..]);
  #assert !same32(a[..], b[..]);
  script_sighash_legacy_vartime(sp, code[..], 1, a[..]);
  sp.amount = 600000000;
  script_sighash_legacy_vartime(sp, code[..], 1, b[..]);
  #assert same32(a[..], b[..]);
}
// The signed spends verify: both inputs with their pairs swapped, and the
// witness script that pushes its own signature. Legacy FindAndDelete would
// have found that push.
#test "signed spends: pairs swapped, no FindAndDelete" #steps 2000000000 {
  s: [1] Spend; w: [1] ScriptWork; c: [128] u8; sig: [80] u8;
  flags := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS;
  for k: 0..<2 {
    sp := spend_of(s[..], P2WSH_B_SWAPPED, k, 16777215);
    sp.spk = tx_unhex(P2WSH_B_SPK[1 - k], s[0].spk[..]);
    #assert script_verify_vartime(sp, flags, w[..]) == SCRIPT_ERR_OK;
  }
  sp := spend_of(s[..], NO_FAD_SIGNED, 0, 200000);
  sp.spk = tx_unhex(NO_FAD_SPK, s[0].spk[..]);
  #assert script_verify_vartime(sp, flags, w[..]) == SCRIPT_ERR_OK;
  sp.amount = 200001;
  #assert script_verify_vartime(sp, flags, w[..]) == SCRIPT_ERR_CHECKSIGVERIFY;
  si_code(w[..], tx_unhex(MORE[4].code, c[..]), 0);
  #assert si_find_delete(w[..], tx_unhex(NO_FAD_SIG, sig[..])) == 1;
}
// Linear against quadratic: 182 bytes per P2WPKH input, and the three
// hashes once.
#test "BIP-143 hashing is linear" {
  #assert V0_INPUT == 182 && v0_once(1) == 108 && v0_once(6756) + 6756 * V0_INPUT == 1499900;
  #assert legacy_bytes(6756) == 1872121380 && legacy_bytes(1) == 148;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/segwit-sighash.coax --run-steps 250000000
```

`coax test` runs 180 checks, including the loaded files' own checks, in about
3.5 minutes on 2026-11-03 on a heavily loaded machine. All pass.

### The library's own checks

`script.coax` checks the BIP-143 hash against the BIP's eight vectors and
nine hashes the Python reference computes for positions the BIP leaves
out, and this chapter adds a check of the native P2WPKH example's three
published hashes and its scriptCode.

```shell
coax/bin/coax test coax/lib/bitcoin/script.coax --run-steps 250000000
```

On 2026-11-03 `coax test` runs 175 checks, all passing.

### Core in Python, natively and in Wasm

[script_reference.py](../../coax/tests/bitcoin/script_reference.py)
transliterates Core's `SignatureHash` for `WITNESS_V0` without sharing
code with Coax.
[sighash_examples.py](../../coax/tests/bitcoin/sighash_examples.py)
recomputes the example's output with it: each of the thirteen hashes and
its three intermediate hashes, taken from the preimage the reference
hashes, the amount hashes, both signed spends through the reference's
`VerifyScript`, and each byte count. It also checks that every value the
example copies from BIP-143, and every non-zero intermediate hash it
prints, appears in the pinned BIP text.

```shell
python3 coax/tests/bitcoin/sighash_examples.py v0 bitcoin/fixtures/segwit-sighash.out
```

On 2026-11-03 it reports 104 values that agree with the reference.

[script.py](../../coax/tests/bitcoin/script.py) runs the interpreter's
cases, BIP-143 hashes among them, natively and in Wasm, in checked and
release builds, against the same reference:

```shell
python3 coax/tests/bitcoin/script.py
```

On 2026-11-03 it reports 16,178 cases that agree with Python in all four
builds, 828 of them signature hashes.

Mutation testing was not run for this chapter (owner speed order).

## What this chapter leaves out

- The legacy hash and the hash types in detail: chapter 37.
- Taproot's hash, which commits to every input's amount and
  scriptPubKey: chapter 43.
- Witness serialization, txids and wtxids: chapters 34 and 35.
- Signing in a wallet and PSBT's `witness_utxo` field: chapters 94 and
  99.

## Summary

- BIP-143 hashes ten fields: the version, `hashPrevouts`,
  `hashSequence`, this input's outpoint, scriptCode, amount and sequence,
  `hashOutputs`, the lock time and the hash type.
- The three hashes don't depend on the input, so a transaction computes
  them once and each input's hash reads a fixed number of bytes: linear,
  not quadratic.
- The amount is signed, so a wrong amount gives an invalid signature.
  It covers only the signed input, which Taproot fixes.
- The P2WPKH scriptCode is `0x1976a914{key hash}88ac`.
- FindAndDelete is gone, `SIGHASH_SINGLE` past the outputs hashes zeros,
  and `SINGLE|ANYONECANPAY` doesn't fix the index.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [36. The script interpreter](ch036-the-script-interpreter.md)

## Sources

- [BIP-143](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki),
  pinned at
  [coax/tests/vectors/upstream/bip143/bip-0143.mediawiki](../../coax/tests/vectors/upstream/bip143/bip-0143.mediawiki).
- Bitcoin Core at revision `ddea9196`: `script/interpreter.cpp`
  (`SignatureHash`, `PrecomputedTransactionData`) and
  `script/interpreter.h`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/interpreter.cpp).
- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  (witness programs) and
  [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
  (Taproot's hash).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
