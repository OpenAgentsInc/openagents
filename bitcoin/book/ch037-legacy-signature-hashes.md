# 37. Legacy signature hashes

A signature in legacy script doesn't sign a transaction. It signs a 32-byte
hash of a copy of the transaction, edited by the signature's *hash type*:
some inputs and outputs are left out, other inputs' scripts are emptied,
and the input being signed carries the script it spends. This chapter
explains that copy field by field, with Bitcoin Core's `SignatureHash` for
`SigVersion::BASE`, which chapter 36's interpreter already calls.

The code is chapter 36's. This chapter reads it, shows what each hash type
commits to by changing one byte at a time, and covers the three behaviors
that are consensus by accident: the `SIGHASH_SINGLE` bug, FindAndDelete
and hash types that were never defined. All 500 rows of Bitcoin Core's
`sighash.json` give their published hash.

## What this chapter teaches

- What a legacy signature hash serializes: the version, the inputs with
  every scriptSig but one emptied, the outputs, the lock time and the hash
  type as 4 bytes, hashed with SHA-256d.
- The hash types: `ALL`, `NONE` and `SINGLE`, each with or without
  `ANYONECANPAY`, and what each lets another signer change.
- The `SIGHASH_SINGLE` bug: an input with no output at its index signs
  the number 1.
- The scriptCode: from the last `OP_CODESEPARATOR` that ran, without
  separators, and without the signature itself (FindAndDelete).
- Undefined hash types: consensus accepts any byte, and policy refuses
  all but six.
- Why legacy hashing is quadratic in the number of inputs, with the
  numbers for a 1,000,000-byte transaction.

## Where Bitcoin uses the legacy signature hash

Every `CHECKSIG` and `CHECKMULTISIG` outside a witness program hashes this
way: P2PK, P2PKH, bare multisig and P2SH redeem scripts that aren't
segwit.

| Where | What it hashes | Chapter |
| --- | --- | --- |
| The script interpreter | every legacy signature check | 36 |
| Block validation | every legacy input of every block | 55, 57 |
| A wallet signing a legacy input | the input it signs | 94, 99 |
| PSBT signers | a non-segwit input, with the whole previous transaction | 94 |

Segwit v0 inputs hash BIP-143's summary instead (chapter 38), and Taproot
inputs hash BIP-341's (chapter 43).

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_sighash_legacy_vartime(sp, code, hashtype, out)` | Core's `SignatureHash` for `SigVersion::BASE` |
| | `si_absorb_code` | `CTransactionSignatureSerializer::SerializeScriptCode` |
| | `si_find_delete` | `FindAndDelete`, which `CHECKSIG` runs before it hashes (chapter 36) |
| [der.coax](../../coax/lib/bitcoin/der.coax) | `der_hashtype_defined_vartime` | `IsDefinedHashtypeSignature`, the `STRICTENC` rule (chapter 21) |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_read_vartime`, `tx_absorb` | the transaction's fields and piecewise SHA-256 (chapter 34) |

The transaction, the scriptCode and the hash are public, so the code runs
in variable time and its procedures end in `_vartime` (chapter 3).

## The hash types

The hash type is the last byte of the signature. Core names four values:

| Name | Value | Meaning |
| --- | --- | --- |
| `SIGHASH_ALL` | `0x01` | sign every input and every output |
| `SIGHASH_NONE` | `0x02` | sign every input and no output |
| `SIGHASH_SINGLE` | `0x03` | sign every input and the output at this input's index |
| `SIGHASH_ANYONECANPAY` | `0x80` | combined with one of the three: sign only this input |

The hash reads the type as Core does. The low five bits (`type & 0x1f`)
select the outputs: 2 is `NONE`, 3 is `SINGLE`, and anything else behaves
as `ALL`. Bit `0x80` selects the inputs. The other two bits are ignored
by the selection, but the whole type is hashed, so a different byte always
gives a different hash.

## The serialization

The hash is SHA-256d over this copy of the transaction:

| Field | `ALL` | `NONE` | `SINGLE` | with `ANYONECANPAY` |
| --- | --- | --- | --- | --- |
| version | 4 bytes | the same | the same | the same |
| input count | all inputs | all | all | 1 |
| each input's outpoint | yes | yes | yes | this input's only |
| each input's script | the scriptCode for this input, empty for the others | the same | the same | the scriptCode |
| each input's sequence | yes | this input's; others 0 | this input's; others 0 | this input's |
| output count | all outputs | 0 | this input's index + 1 | as without it |
| each output | value and script | none | this index's; the earlier ones as value -1 and an empty script | as without it |
| lock time | 4 bytes | the same | the same | the same |
| hash type | 4 bytes, little-endian | the same | the same | the same |

Setting other inputs' sequences to 0 under `NONE` and `SINGLE` lets their
owners change them, the "let the others update at will" of Core's comment.
The `-1` placeholder outputs under `SINGLE` keep each output at its index
without committing to what the earlier ones pay.

<!-- source: coax/lib/bitcoin/script.coax legacy -->
```jai
// The legacy signature hash (Core's SignatureHash for SigVersion::BASE;
// chapter 37 explains it): the transaction with every other input's
// scriptSig empty, this input's replaced by the scriptCode, and the parts
// NONE, SINGLE and ANYONECANPAY leave out, then the 4-byte hash type,
// SHA-256d into out. SIGHASH_SINGLE with no output at this input's index
// signs the number 1, Core's historical bug.
script_sighash_legacy_vartime :: (sp: ScriptSpend, code: [] u8, hashtype: int, out: [] u8) {
  b := sp.b; t := sp.t; index := sp.index;
  anyone := (hashtype & 0x80) != 0;
  single := (hashtype & 0x1f) == 3; none := (hashtype & 0x1f) == 2;
  if single && index >= t.nout {
    for i: 0..<32 { out[i] = 0; }  // mutant: equivalent (out[0] is set to 1 next)
    out[0] = 1;
    return;
  }
  state: [SHA_STATE_BYTES] u8; si_start(state[..]);
  si_absorb(state[..], b[0..<4]);
  inputs := ifx anyone then 1 else t.nin;
  si_absorb_compact(state[..], inputs);
  for k: 0..<inputs {
    i := ifx anyone then index else k;
    si_absorb(state[..], b[sp.ins[i].outpoint..<sp.ins[i].outpoint+36]);
    if i == index { si_absorb_code(state[..], code); } else { si_absorb_compact(state[..], 0); }
    if i != index && (single || none) { si_absorb_le(state[..], 0, 4); }
    else { si_absorb(state[..], b[sp.ins[i].sequence..<sp.ins[i].sequence+4]); }
  }
  outputs := ifx none then 0 else ifx single then index + 1 else t.nout;
  si_absorb_compact(state[..], outputs);
  for k: 0..<outputs {
    o := sp.outs[k];
    if single && k != index { si_absorb_le(state[..], -1, 8); si_absorb_compact(state[..], 0); }
    else { si_absorb(state[..], b[o.value..<o.script+o.script_n]); }
  }
  si_absorb(state[..], b[t.locktime..<t.locktime+4]);
  si_absorb_le(state[..], hashtype, 4);
  si_finish(state[..], out);
}
```

The procedure absorbs the fields into one SHA-256 state as it reads them
from the transaction's bytes, through chapter 34's offsets. It never
builds the copy, so a 1,000,000-byte transaction needs no second megabyte.
`hashtype` is a 32-bit value: script passes the signature's last byte,
while `sighash.json` uses random 32-bit types, negative ones among them,
and the last field writes its low 4 bytes.

Core's `SignatureHash` also returns 1 when the input index is past the
inputs. The interpreter never asks for such an index, so the procedure
leaves that case out.

### Worked example: what each hash type signs

The example takes BIP-143's first transaction, unsigned, which has 2
inputs and 2 outputs. It hashes input 1 the legacy way with a P2PKH
scriptCode. Then it changes one byte of each of six parts in turn and
checks whether the hash changes:

| Type | Input 0's outpoint | Input 0's sequence | Input 1's sequence | Output 0 | Output 1 | Lock time |
| --- | --- | --- | --- | --- | --- | --- |
| `0x01` `ALL` | yes | yes | yes | yes | yes | yes |
| `0x02` `NONE` | yes | | yes | | | yes |
| `0x03` `SINGLE` | yes | | yes | | yes | yes |
| `0x81` `ALL\|ANYONECANPAY` | | | yes | yes | yes | yes |
| `0x82` `NONE\|ANYONECANPAY` | | | yes | | | yes |
| `0x83` `SINGLE\|ANYONECANPAY` | | | yes | | yes | yes |

Each type also signs the version, the input's own outpoint and the
scriptCode. Three uses follow from the table:

- `ALL|ANYONECANPAY` lets anyone add inputs to a fixed set of outputs: a
  crowdfund that is only valid once the inputs reach the amount.
- `SINGLE|ANYONECANPAY` signs one input against one output: an offer that
  a taker completes with their own inputs and outputs.
- `NONE` signs no output at all, so whoever holds the signature, together
  with any other signatures the transaction needs, decides where the
  money goes.

## The SIGHASH_SINGLE bug

`SIGHASH_SINGLE` signs the output at the input's index. When there is no
output there, the original client returned the number 1 as an error value
and then used it as the hash. That is now consensus: such a signature
signs the 32 bytes `01 00 … 00`, the uint256 1, and not the transaction.

The example finds the first row of `sighash.json` with more inputs than
outputs, a transaction with 3 inputs and 2 outputs, and hashes input 2:

```text
0x03: 0000000000000000000000000000000000000000000000000000000000000001
0x83: 0000000000000000000000000000000000000000000000000000000000000001
0x01: 77860e49cffcf4ff62bff9259712fca87395bf04d8768491e5e43577b7cea8ed
```

Hashes print the way Core prints a uint256 and `sighash.json` lists them,
last byte first. Input 1 of a second transaction with 1 output gives the
same hash. A key that signs `SIGHASH_SINGLE` on such an input has signed
"1", so anyone holding that signature can reuse it for any other input
locked to the same key, in any transaction where that input's index is
past the outputs. Wallets never produce it.
Segwit v0 and Taproot remove the bug: BIP-143 hashes zeros for the
missing output (chapter 38), and BIP-341 makes the signature invalid.

## The scriptCode

The input being signed carries the scriptCode in place of its scriptSig:
the script being run, from just after the last `OP_CODESEPARATOR` that
executed. Chapter 36's `CHECKSIG` cuts it there and then removes the
signature from it with FindAndDelete, since a signature cannot sign
itself. The serializer then drops every `OP_CODESEPARATOR` left in it,
and writes the length minus their number:

<!-- source: coax/lib/bitcoin/script.coax scriptcode -->
```jai
// CTransactionSignatureSerializer::SerializeScriptCode: the scriptCode
// without its OP_CODESEPARATORs, prefixed by its length minus their
// number. Like Core, it copies only up to where GetOp stopped, so a
// script that ends inside a push contributes fewer bytes than its prefix
// says.
si_absorb_code :: (state: [] u8, code: [] u8) {
  separators := 0; pc := 0;
  while true {
    op := script_op_vartime(code, pc);
    if !op.ok { break; }
    if op.op == OP_CODESEPARATOR { separators += 1; }
    pc = op.next;
  }
  si_absorb_compact(state, code.count - separators);
  begin := 0; pc = 0;
  while true {
    op := script_op_vartime(code, pc);
    pc = op.next;
    if !op.ok { break; }
    if op.op == OP_CODESEPARATOR { si_absorb(state, code[begin..<pc-1]); begin = pc; }
  }
  if begin != code.count { si_absorb(state, code[begin..<pc]); }
}
```

The example hashes four scriptCodes for the same input:

```text
CHECKSIG: 5669e48ce61af6f01cfecc8145bc226b726443929089179e3f859fcd5e2ddea6
CODESEPARATOR CHECKSIG: 5669e48ce61af6f01cfecc8145bc226b726443929089179e3f859fcd5e2ddea6
CODESEPARATOR CODESEPARATOR CHECKSIG: 5669e48ce61af6f01cfecc8145bc226b726443929089179e3f859fcd5e2ddea6
CHECKSIG CODESEPARATOR CHECKSIG: e0a8cc35da5224b839b21d434bb797f7ea66506dc42a6295730d68e98f5708d0
```

The first three are the same hash: separators never reach it. The fourth
keeps both `CHECKSIG`s. The interpreter cuts the scriptCode at the last
separator that ran, so a separator still in it is one that didn't run,
for example one inside an `IF` branch not taken. The serializer drops it
and keeps the code around it.

FindAndDelete, which chapter 36 lists, removes whole pushes of the
signature that start where an opcode starts, and every copy:

| scriptCode | Removed | Left |
| --- | --- | --- |
| `02aabb 75 02aabb ac` (push, `DROP`, push, `CHECKSIG`) | 2 | `75ac` |
| `03 02aabb ac` (a push of `02aabb`) | 0 | `0302aabbac` |
| `02aabb 02aabb 51` | 2 | `51` |

In the second row the bytes `02aabb` are inside another push, so they are
data and stay. Both rules are policy-discouraged for new scripts:
`CONST_SCRIPTCODE` fails a legacy script that runs `OP_CODESEPARATOR` or
whose signature FindAndDelete would find. Neither applies to segwit, which
hashes the scriptCode as it is.

## Undefined hash types

Consensus checks nothing about the hash type. A signature whose last byte
is `0x00`, `0x04` or `0xff` verifies if it signs the hash of that type,
and it commits to whatever `type & 0x1f` and `type & 0x80` select:

| Type | Behaves as | Under `STRICTENC` |
| --- | --- | --- |
| `0x00`, `0x04`, `0x21` | `ALL` | refused |
| `0x84`, `0xff` | `ALL\|ANYONECANPAY` | refused |
| `0xc3` | `SINGLE\|ANYONECANPAY` | refused |

`STRICTENC` is a policy flag (chapter 81). Under it, chapter 21's
`IsDefinedHashtypeSignature` allows only the six defined types: the byte
without `0x80` must be 1, 2 or 3. So `0xc3`, which hashes exactly as
`SINGLE|ANYONECANPAY` selects, is still refused, and a block may contain
any of them. Core generates `sighash.json` with random 32-bit types, so
488 of its 500 rows are undefined types, and all of them hash as Core
does.

BIP-143 keeps this behavior for segwit v0. BIP-341 ends it: a Taproot
signature with an undefined type is invalid.

## The quadratic hashing problem

Each input's hash reads about the whole transaction: every other input's
outpoint and sequence, and every output. A transaction with n inputs
therefore hashes about n times its own size, and its size grows with n,
so the work grows with n².

The example counts the bytes the first SHA-256 reads for one input of a
transaction with n P2PKH inputs (148 bytes each) and two P2PKH outputs.
One input hashes 106 bytes plus 41 for each input and the input count's
CompactSize:

| Inputs | Transaction bytes | Hashed per input | Hashed in all |
| --- | --- | --- | --- |
| 1 | 226 | 148 | 148 |
| 10 | 1,558 | 517 | 5,170 |
| 100 | 14,878 | 4,207 | 420,700 |
| 1,000 | 148,080 | 41,109 | 41,109,000 |
| 5,000 | 740,080 | 205,109 | 1,025,545,000 |
| 6,756 | 999,968 | 277,105 | 1,872,121,380 |

6,756 inputs is the most that fit in 1,000,000 bytes. Checking that
transaction hashes 1.87 GB, about 1,870 times its size, for one valid
transaction. Scripts with many `CHECKSIG`s per input multiply it again,
since each signature check can hash the transaction once. Core keeps the
SHA-256 midstate per input, hash type and scriptCode (`SigHashCache`),
which helps only when one input repeats a check. This denial-of-service
is CVE-2013-2292.

Three limits contain it today:

- Policy: Core relays no transaction over 400,000 weight units
  (`MAX_STANDARD_TX_WEIGHT`), 100,000 bytes of a legacy transaction, so
  the slow case only reaches a node in a block.
- Segwit: BIP-143 hashes each input in constant size, from three hashes
  computed once (chapter 38).
- BIP-54, the consensus cleanup soft fork, proposes a limit of 2,500
  signature operations in the scripts of each transaction's legacy
  inputs, which reduces the worst-case block validation time by a factor
  of 40.

## The example

The example is
[bitcoin/examples/legacy-sighash.coax](../examples/legacy-sighash.coax).
It hashes one input under twelve hash types and finds what each one
signs, shows the `SIGHASH_SINGLE` bug on two transactions, hashes four
scriptCodes and runs FindAndDelete on three, counts the bytes quadratic
hashing reads, and checks every row of `sighash.json`.

<!-- source: bitcoin/examples/legacy-sighash.coax example -->
```jai
#load "../../coax/lib/bitcoin/script.coax";
#load "../../coax/tests/bitcoin/script_vectors.coax";
#import "Basic";

// A transaction read into fixed room, and the view the signature hash
// takes of one of its inputs.
Spend :: struct {
  b: [4096] u8;
  ins: [16] TxIn; outs: [16] TxOut; items: [1] TxItem;
}
spend_of :: (s: [] Spend, tx: string, index: int) -> ScriptSpend {
  b := tx_unhex(tx, s[0].b[..]);
  t := tx_read_vartime(b, true, s[0].ins[..], s[0].outs[..], s[0].items[..]);
  #assert t.code == TX_OK;
  return ScriptSpend.{b=b, t=t, ins=s[0].ins[0..<t.nin], outs=s[0].outs[0..<t.nout], items=s[0].items[0..<0],
    index=index, spk=b[0..<0]};
}
// A hash as Core prints a uint256 and as sighash.json lists it: byte 31
// first.
show_hash :: (h: [] u8) {
  digits := "0123456789abcdef";
  for k: 0..<32 {
    pair: [2] u8; x := h[31-k];
    pair[0] = digits[x >> 4]; pair[1] = digits[x & 15];
    print("%", cast(string) pair[..]);
  }
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

// The six parts of a 2-input, 2-output transaction a signature on input 1
// might cover. Bit k of what signs() returns is part k.
PARTS :: [6] string.["input 0's outpoint", "input 0's sequence", "input 1's sequence", "output 0", "output 1", "the lock time"];
part_at :: (sp: ScriptSpend, k: int) -> int {
  if k == 0 { return sp.ins[0].outpoint; }
  if k == 1 { return sp.ins[0].sequence; }
  if k == 2 { return sp.ins[1].sequence; }
  if k == 3 { return sp.outs[0].value; }
  if k == 4 { return sp.outs[1].value; }
  return sp.t.locktime;
}
// Which parts the hash of this type commits to: change one byte of each
// part and see whether the hash changes.
signs :: (sp: ScriptSpend, code: [] u8, hashtype: int) -> int {
  base: [32] u8; h: [32] u8;
  script_sighash_legacy_vartime(sp, code, hashtype, base[..]);
  mask := 0;
  for k: 0..<6 {
    at := part_at(sp, k);
    sp.b[at] ^= 1;
    script_sighash_legacy_vartime(sp, code, hashtype, h[..]);
    sp.b[at] ^= 1;
    if !same32(base[..], h[..]) { mask |= 1 << k; }
  }
  return mask;
}

// Bytes the first SHA-256 of a legacy signature hash reads, for one input
// of a transaction with n P2PKH inputs (148 bytes each: a 107-byte
// scriptSig) and two P2PKH outputs (34 bytes each): the version, the input
// count, 41 bytes for each other input (outpoint, an empty script, the
// sequence), 66 for this one (its 25-byte scriptCode with its length), the
// outputs with their count, the lock time and the 4-byte hash type.
legacy_preimage_bytes :: (n: int) -> int { return 4 + tx_compact_size_vartime(n) + 41 * (n - 1) + 66 + 1 + 68 + 4 + 4; }
legacy_tx_bytes :: (n: int) -> int { return 4 + tx_compact_size_vartime(n) + 148 * n + 1 + 68 + 4; }

main :: () {
  s: [1] Spend; h: [32] u8; c: [64] u8;

  // BIP-143's first transaction, unsigned: 2 inputs, 2 outputs. Input 1,
  // hashed the legacy way with a P2PKH scriptCode.
  sp := spend_of(s[..], BT_V143[0].tx, 1);
  code := tx_unhex("76a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac", c[..]);
  print("what each hash type signs, for input 1 of 2 with 2 outputs\n");
  types := [12] int.[0x01, 0x02, 0x03, 0x81, 0x82, 0x83, 0x00, 0x04, 0x21, 0x84, 0xc3, 0xff];
  names := [12] string.["ALL", "NONE", "SINGLE", "ALL|ANYONECANPAY", "NONE|ANYONECANPAY", "SINGLE|ANYONECANPAY",
    "undefined", "undefined", "undefined", "undefined", "undefined", "undefined"];
  for i: 0..<12 {
    sig: [1] u8; sig[0] = cast(u8) types[i];
    script_sighash_legacy_vartime(sp, code, types[i], h[..]);
    print("  0x");
    show_hex(sig[..]);
    print(" %, % under STRICTENC: ", names[i], ifx der_hashtype_defined_vartime(sig[..]) then "allowed" else "refused");
    show_hash(h[..]);
    print("\n    signs:");
    mask := signs(sp, code, types[i]);
    first := true;
    for k: 0..<6 {
      if (mask & (1 << k)) == 0 { continue; }
      print("% %", ifx first then "" else ",", PARTS[k]);
      first = false;
    }
    print("\n");
  }

  // SIGHASH_SINGLE on an input with no output at its index: the first row
  // of sighash.json with more inputs than outputs.
  print("SIGHASH_SINGLE with no output at the input's index\n");
  first: [32] u8; found := 0;
  for r: SCRIPT_SIGHASH {
    sp = spend_of(s[..], r.tx, 0);
    if sp.t.nin <= sp.t.nout { continue; }
    sp.index = sp.t.nout;
    found += 1;
    if found == 2 {
      script_sighash_legacy_vartime(sp, code, 3, h[..]);
      print("  input % of another transaction, with % outputs: %\n", sp.index, sp.t.nout,
        ifx same32(h[..], first[..]) then "the same hash" else "a different hash");
      break;
    }
    print("  % inputs, % outputs, input %\n", sp.t.nin, sp.t.nout, sp.index);
    for hashtype: [3] int.[3, 0x83, 1] {
      script_sighash_legacy_vartime(sp, code, hashtype, h[..]);
      print("    0x%: ", ifx hashtype == 3 then "03" else ifx hashtype == 0x83 then "83" else "01");
      show_hash(h[..]);
      print("\n");
    }
    script_sighash_legacy_vartime(sp, code, 3, first[..]);
  }

  // The scriptCode: OP_CODESEPARATORs are left out, and so is the push of
  // the signature (FindAndDelete, which CHECKSIG does before it hashes).
  print("the scriptCode\n");
  sp = spend_of(s[..], BT_V143[0].tx, 1);
  scripts := [4] string.["ac", "abac", "ababac", "acabac"];
  labels := [4] string.["CHECKSIG", "CODESEPARATOR CHECKSIG", "CODESEPARATOR CODESEPARATOR CHECKSIG", "CHECKSIG CODESEPARATOR CHECKSIG"];
  for i: 0..<4 {
    script_sighash_legacy_vartime(sp, tx_unhex(scripts[i], c[..]), 1, h[..]);
    print("  %: ", labels[i]);
    show_hash(h[..]);
    print("\n");
  }
  w: [1] ScriptWork; sig: [2] u8; sig[0] = 0xaa; sig[1] = 0xbb;
  deletes := [3] string.["02aabb7502aabbac", "0302aabbac", "02aabb02aabb51"];
  for d: deletes {
    si_code(w[..], tx_unhex(d, c[..]), 0);
    n := si_find_delete(w[..], sig[..]);
    print("  FindAndDelete(%, push of aabb): % removed, left ", d, n);
    show_hex(w[0].code[0..<w[0].code_n]);
    print("\n");
  }

  // Each input hashes about the whole transaction, so the work grows with
  // the square of the number of inputs.
  print("legacy hashing, n P2PKH inputs and 2 outputs\n");
  for n: [6] int.[1, 10, 100, 1000, 5000, 6756] {
    print("  n %: % bytes, % hashed per input, % in all\n", n, legacy_tx_bytes(n), legacy_preimage_bytes(n), n * legacy_preimage_bytes(n));
  }

  // Every row of sighash.json, by whether STRICTENC allows its hash type.
  print("Bitcoin Core sighash.json\n");
  rows: [2] int; same: [2] int; big: [10100] u8;
  for r: SCRIPT_SIGHASH {
    sp = spend_of(s[..], r.tx, r.index);
    b: [1] u8; b[0] = cast(u8) (r.hashtype & 255);
    defined := ifx der_hashtype_defined_vartime(b[..]) then 1 else 0;
    script_sighash_legacy_vartime(sp, tx_unhex(r.code, big[..]), r.hashtype, h[..]);
    #assert bitcoin_reverse32(h[..], h[..]) == SHA_OK;
    rows[defined] += 1;
    if tx_hex_is(h[..], r.hash) { same[defined] += 1; }
  }
  print("  defined hash types: % of % match\n", same[1], rows[1]);
  print("  undefined hash types: % of % match\n", same[0], rows[0]);
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/legacy-sighash.coax --run-steps 250000000
```

It prints:

```text
what each hash type signs, for input 1 of 2 with 2 outputs
  0x01 ALL, allowed under STRICTENC: a4a612c19dd001d604b8561f713a4e8f64413d5d5bcc472a4048bc0c823060c4
    signs: input 0's outpoint, input 0's sequence, input 1's sequence, output 0, output 1, the lock time
  0x02 NONE, allowed under STRICTENC: e289cbfaf7b86b3660a6934a1894f191c8ed05d2b79da7765fe5eb4d55cfbbff
    signs: input 0's outpoint, input 1's sequence, the lock time
  0x03 SINGLE, allowed under STRICTENC: c2aeaf1559462e7bf2a21117b1e3de6f1d528c740b18efbc042fb8d68b46cd33
    signs: input 0's outpoint, input 1's sequence, output 1, the lock time
  0x81 ALL|ANYONECANPAY, allowed under STRICTENC: 5607a4f393ed89761f2b37160c4e962d68160790531fc32e33353afe8ceafe8c
    signs: input 1's sequence, output 0, output 1, the lock time
  0x82 NONE|ANYONECANPAY, allowed under STRICTENC: 21671d0cd5c93489b32980a020ead5aba968c08bd51b45b88d0ab61acba48cbd
    signs: input 1's sequence, the lock time
  0x83 SINGLE|ANYONECANPAY, allowed under STRICTENC: a6dc3752518f5408daec502f6c3ac546f102c37621404c8a491789b891775c86
    signs: input 1's sequence, output 1, the lock time
  0x00 undefined, refused under STRICTENC: e684c508b10f725e3c8e89baff5fde283b8595063ad6b4d7430cb814dbb7afc2
    signs: input 0's outpoint, input 0's sequence, input 1's sequence, output 0, output 1, the lock time
  0x04 undefined, refused under STRICTENC: 8c4b80a941b9e59e994ac69c8e3c0cbe675c591db457893f9734e0461c96b23b
    signs: input 0's outpoint, input 0's sequence, input 1's sequence, output 0, output 1, the lock time
  0x21 undefined, refused under STRICTENC: 503f250efe0e229cabfbec36e40091c3de8b4cb7ae3481ca7a754470be7125e5
    signs: input 0's outpoint, input 0's sequence, input 1's sequence, output 0, output 1, the lock time
  0x84 undefined, refused under STRICTENC: 59a564eb79e00b239df759c52dfd38e6a4064c8603402fa88fb75bd47ef2b8ad
    signs: input 1's sequence, output 0, output 1, the lock time
  0xc3 undefined, refused under STRICTENC: e79c7bc1d2eb06e739b84db83877ade742f46d1fc45dfd4716b803435ddab4b2
    signs: input 1's sequence, output 1, the lock time
  0xff undefined, refused under STRICTENC: d1d06cfe45606da62629f08d94c37c7512344532229ff32c6bd6a32a2e05a22a
    signs: input 1's sequence, output 0, output 1, the lock time
SIGHASH_SINGLE with no output at the input's index
  3 inputs, 2 outputs, input 2
    0x03: 0000000000000000000000000000000000000000000000000000000000000001
    0x83: 0000000000000000000000000000000000000000000000000000000000000001
    0x01: 77860e49cffcf4ff62bff9259712fca87395bf04d8768491e5e43577b7cea8ed
  input 1 of another transaction, with 1 outputs: the same hash
the scriptCode
  CHECKSIG: 5669e48ce61af6f01cfecc8145bc226b726443929089179e3f859fcd5e2ddea6
  CODESEPARATOR CHECKSIG: 5669e48ce61af6f01cfecc8145bc226b726443929089179e3f859fcd5e2ddea6
  CODESEPARATOR CODESEPARATOR CHECKSIG: 5669e48ce61af6f01cfecc8145bc226b726443929089179e3f859fcd5e2ddea6
  CHECKSIG CODESEPARATOR CHECKSIG: e0a8cc35da5224b839b21d434bb797f7ea66506dc42a6295730d68e98f5708d0
  FindAndDelete(02aabb7502aabbac, push of aabb): 2 removed, left 75ac
  FindAndDelete(0302aabbac, push of aabb): 0 removed, left 0302aabbac
  FindAndDelete(02aabb02aabb51, push of aabb): 2 removed, left 51
legacy hashing, n P2PKH inputs and 2 outputs
  n 1: 226 bytes, 148 hashed per input, 148 in all
  n 10: 1558 bytes, 517 hashed per input, 5170 in all
  n 100: 14878 bytes, 4207 hashed per input, 420700 in all
  n 1000: 148080 bytes, 41109 hashed per input, 41109000 in all
  n 5000: 740080 bytes, 205109 hashed per input, 1025545000 in all
  n 6756: 999968 bytes, 277105 hashed per input, 1872121380 in all
Bitcoin Core sighash.json
  defined hash types: 12 of 12 match
  undefined hash types: 488 of 488 match
```

What the output shows:

- The six defined types sign the parts the serialization table gives.
  The six undefined ones sign what their `0x1f` and `0x80` bits select,
  and `STRICTENC` refuses all six.
- `SIGHASH_SINGLE` past the outputs signs 1 with or without
  `ANYONECANPAY`, on any transaction, and `ALL` on the same input hashes
  as usual.
- Separators never reach the hash, and FindAndDelete removes only whole
  pushes.
- The bytes hashed grow with the square of the inputs: 148 for one input,
  1,872,121,380 for 6,756.
- All 500 rows of `sighash.json` match: the 12 with defined types and the
  488 with undefined ones.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/legacy-sighash.out](../fixtures/legacy-sighash.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/legacy-sighash.coax](../examples/checks/legacy-sighash.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/legacy-sighash.coax check -->
```jai
// What each hash type commits to, found by changing one byte of each part:
// bit 0 input 0's outpoint, 1 its sequence, 2 input 1's sequence, 3 output
// 0, 4 output 1, 5 the lock time. An undefined type commits to what its
// low five bits and its 0x80 bit select, and every type gives its own hash.
#test "each hash type signs its parts" #steps 400000000 {
  s: [1] Spend; c: [64] u8;
  sp := spend_of(s[..], BT_V143[0].tx, 1);
  code := tx_unhex("76a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac", c[..]);
  types := [12] int.[0x01, 0x02, 0x03, 0x81, 0x82, 0x83, 0x00, 0x04, 0x21, 0x84, 0xc3, 0xff];
  masks := [12] int.[63, 37, 53, 60, 36, 52, 63, 63, 63, 60, 52, 60];
  hashes: [12][32] u8;
  for i: 0..<12 {
    #assert signs(sp, code, types[i]) == masks[i];
    script_sighash_legacy_vartime(sp, code, types[i], hashes[i][..]);
    for j: 0..<i { #assert !same32(hashes[i][..], hashes[j][..]); }
  }
}
// SIGHASH_SINGLE past the last output signs the number 1, whatever the
// transaction, with or without ANYONECANPAY; ALL there hashes as usual.
#forall "SIGHASH_SINGLE past the outputs signs 1" i: 0..<SCRIPT_SIGHASH.count #steps 100000000 {
  s: [1] Spend; h: [32] u8;
  sp := spend_of(s[..], SCRIPT_SIGHASH[i].tx, 0);
  if sp.t.nin > sp.t.nout {
    sp.index = sp.t.nout;
    for hashtype: [3] int.[3, 0x83, 0x43] {
      script_sighash_legacy_vartime(sp, sp.b[0..<0], hashtype, h[..]);
      #assert h[0] == 1;
      for k: 1..<32 { #assert h[k] == 0; }
    }
    script_sighash_legacy_vartime(sp, sp.b[0..<0], 1, h[..]);
    #assert h[0] != 1 || h[1] != 0;
  }
}
// OP_CODESEPARATORs never reach the hash; FindAndDelete removes whole
// pushes of the signature at op boundaries.
#test "the scriptCode" #steps 100000000 {
  s: [1] Spend; c: [64] u8; a: [32] u8; h: [32] u8;
  sp := spend_of(s[..], BT_V143[0].tx, 1);
  script_sighash_legacy_vartime(sp, tx_unhex("ac", c[..]), 1, a[..]);
  script_sighash_legacy_vartime(sp, tx_unhex("abac", c[..]), 1, h[..]);
  #assert same32(a[..], h[..]);
  script_sighash_legacy_vartime(sp, tx_unhex("ababac", c[..]), 1, h[..]);
  #assert same32(a[..], h[..]);
  script_sighash_legacy_vartime(sp, tx_unhex("acabac", c[..]), 1, h[..]);
  #assert !same32(a[..], h[..]);
  w: [1] ScriptWork; sig: [2] u8; sig[0] = 0xaa; sig[1] = 0xbb;
  si_code(w[..], tx_unhex("02aabb7502aabbac", c[..]), 0);
  #assert si_find_delete(w[..], sig[..]) == 2 && tx_hex_is(w[0].code[0..<w[0].code_n], "75ac");
  si_code(w[..], tx_unhex("0302aabbac", c[..]), 0);
  #assert si_find_delete(w[..], sig[..]) == 0 && w[0].code_n == 5;
}
// The byte counts: one input hashes 148 bytes, and the largest transaction
// of P2PKH inputs under 1,000,000 bytes has 6,756.
#test "legacy hashing grows with the square of the inputs" {
  #assert legacy_preimage_bytes(1) == 148 && legacy_tx_bytes(1) == 226;
  #assert legacy_preimage_bytes(253) == 4 + 3 + 41 * 252 + 66 + 69 + 8;
  #assert legacy_tx_bytes(6756) <= 1000000 && legacy_tx_bytes(6757) > 1000000;
  #assert 6756 * legacy_preimage_bytes(6756) == 1872121380;
}
// Every row of sighash.json whose type STRICTENC refuses still has its
// published hash: consensus hashes any type.
#forall "sighash.json rows with undefined hash types" i: 0..<SCRIPT_SIGHASH.count #steps 100000000 {
  r := SCRIPT_SIGHASH[i];
  b: [1] u8; b[0] = cast(u8) (r.hashtype & 255);
  if !der_hashtype_defined_vartime(b[..]) {
    s: [1] Spend; c: [10100] u8; h: [32] u8;
    sp := spend_of(s[..], r.tx, r.index);
    script_sighash_legacy_vartime(sp, tx_unhex(r.code, c[..]), r.hashtype, h[..]);
    #assert bitcoin_reverse32(h[..], h[..]) == SHA_OK && tx_hex_is(h[..], r.hash);
  }
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/legacy-sighash.coax --run-steps 250000000
```

CHECKS37

### The library's own checks

Chapter 36 lists `script.coax`'s checks. For this hash they run ten rows
of `sighash.json`, one per hash type and input position, and
`SIGHASH_SINGLE` past the outputs. `script.py` runs seeded legacy hashes
of the transaction files' inputs, `SIGHASH_SINGLE` past the outputs among
them, natively and in Wasm, each equal to the Python reference's.

### Core in Python

[script_reference.py](../../coax/tests/bitcoin/script_reference.py)
transliterates Core's `SignatureHash` and `SerializeScriptCode` without
sharing code with Coax.
[sighash_examples.py](../../coax/tests/bitcoin/sighash_examples.py)
recomputes the example's output with it: every hash the example prints,
the hash of 1 past the outputs, and each byte count, by measuring the
preimage the reference hashes for transactions of 1 to 6,756 inputs.

```shell
python3 coax/tests/bitcoin/sighash_examples.py legacy bitcoin/fixtures/legacy-sighash.out
```

On 2026-11-03 it reports 23 values that agree with the reference.
Mutation testing was not run for this chapter (owner speed order).

## What this chapter leaves out

- The BIP-143 hash of segwit v0: chapter 38.
- Taproot's hash, where undefined types and the `SIGHASH_SINGLE` bug are
  invalid: chapter 43.
- Signature-operation counting and the block's sigop limit: chapter 53.
- Signing with each hash type in a wallet, and PSBT's sighash field:
  chapters 94 and 99.

## Summary

- A legacy signature signs SHA-256d of an edited copy of the
  transaction: this input's scriptSig replaced by the scriptCode, every
  other scriptSig empty, then the 4-byte hash type.
- `NONE` drops the outputs, `SINGLE` keeps the one at this input's index,
  and `ANYONECANPAY` keeps only this input. Under `NONE` and `SINGLE` the
  other inputs' sequences are zero.
- `SIGHASH_SINGLE` with no output at the index signs the number 1.
- The scriptCode starts after the last executed `OP_CODESEPARATOR`, loses
  its separators and loses the signature's own push.
- Consensus accepts any hash type; `STRICTENC` policy allows six.
- Each input hashes about the whole transaction, so the work is quadratic:
  1.87 GB for a 1,000,000-byte transaction of 6,756 inputs.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [36. The script interpreter](ch036-the-script-interpreter.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `script/interpreter.cpp`
  (`SignatureHash`, `CTransactionSignatureSerializer`, `FindAndDelete`),
  `script/interpreter.h`, `sighash.json` and `policy/policy.h`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/interpreter.cpp).
- [BIP-143](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki)
  and [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  for what they change.
- [BIP-54](https://github.com/bitcoin/bips/blob/master/bip-0054.md), the
  consensus cleanup, pinned at
  [coax/tests/vectors/upstream/bip54/bip-0054.md](../../coax/tests/vectors/upstream/bip54/bip-0054.md).
- CVE-2013-2292, the quadratic hashing denial-of-service.
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
