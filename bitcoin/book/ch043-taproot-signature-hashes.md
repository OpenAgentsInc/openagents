# 43. Taproot signature hashes

A signature signs a hash of the transaction, not the transaction. Which
parts go into that hash decides what a signer agrees to and what others
can still change. Chapter 38 built the segwit v0 hash of BIP-143.
BIP-341 replaces it for Taproot with one message, *SigMsg*, used by the
key path and, with an extension from BIP-342, by every signature a
tapscript checks. This chapter builds SigMsg in full: every hash type,
the annex, and the script path's extension.

Chapter 41 signed BIP-341's seven key-path inputs with a sighash this
code computed. This chapter shows the message behind those hashes field
by field. BIP-341's seven published messages match byte for byte, and
for the annex and the script path, which BIP-341's vectors don't cover,
30 messages match those of Bitcoin Core's own Python, pinned.

## What this chapter teaches

- SigMsg's fields, in order: the epoch, the hash type, the transaction's
  version and locktime, five hashes over every input and output, the
  spend type, this input, the annex's hash and the output at this index.
- What each hash type keeps: SIGHASH_DEFAULT and ALL keep everything,
  NONE drops the outputs, SINGLE keeps one output, and ANYONECANPAY drops
  the other inputs.
- Why SIGHASH_DEFAULT and SIGHASH_ALL sign the same fields but give
  different hashes.
- The annex, the spend type, and the script path's extension: the leaf
  hash, the key version and the position of the last `OP_CODESEPARATOR`.
- The hash types and inputs BIP-341 refuses.

## Where Bitcoin uses Taproot signature hashes

| Where | What it does | Chapter |
| --- | --- | --- |
| A wallet signing a key-path spend | the hash chapter 41's signature signs | 41, 99 |
| The tapscript interpreter | the hash each `OP_CHECKSIG`, `OP_CHECKSIGVERIFY` and `OP_CHECKSIGADD` checks | 44 |
| A wallet spending by a script | the hash a script-path signature signs | 45, 99 |
| PSBT signers | the same hash, from the spent outputs a PSBT carries | 94 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_tap_hashes` | sha_prevouts, sha_amounts, sha_scriptpubkeys, sha_sequences and sha_outputs |
| | `bt_sigmsg_taproot(b, t, index, amounts, spk, hashtype, annex, leaf, codesep, m)` | the epoch byte and SigMsg, with or without an annex and the extension |
| | `bt_sighash_taproot_ext(…, out)` | `hash_TapSighash` of that message |
| | `bt_sighash_taproot(…, out)` | the key-path hash with no annex, as chapter 41 uses it |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_leaf_hash_vartime` | the leaf hash the extension carries (chapter 42) |

The transaction, the spent outputs and the hash are public, so none of
this needs constant time.

## What a Taproot signature signs

The hash is

```text
sighash = hash_TapSighash(0x00 || SigMsg(hash_type, ext_flag) || ext)
```

The first byte, 0x00, is the *epoch*. A later soft fork can define a new
message under the same tag with another epoch byte, and no message of
the new kind can equal one of the old. BIP-341's vectors and Bitcoin
Core both count the epoch as part of the message, and so does this code:
`bt_sigmsg_taproot` writes it as byte 0.

Then come the fields, in this order:

| Field | Bytes | When |
| --- | --- | --- |
| hash_type | 1 | always |
| nVersion, nLockTime | 4 + 4 | always |
| sha_prevouts, sha_amounts, sha_scriptpubkeys, sha_sequences | 32 each | unless ANYONECANPAY |
| sha_outputs | 32 | DEFAULT and ALL |
| spend_type | 1 | always: ext_flag × 2 + annex_present |
| outpoint, amount, scriptPubKey, nSequence of this input | 36 + 8 + 35 + 4 | ANYONECANPAY |
| input_index | 4 | otherwise |
| sha_annex | 32 | with an annex |
| sha_single_output | 32 | SINGLE |

The five `sha_` hashes are single SHA-256, not SHA-256d, over every
input's outpoint, amount, scriptPubKey with its length, and sequence,
and over every output. BIP-143 hashed the outpoints, sequences and
outputs too. BIP-341 adds two things BIP-143 left out.

- **Every spent amount.** BIP-143 commits only to the amount of the
  input being signed. A hardware wallet signing one input couldn't check
  the others' amounts, so it couldn't be sure of the fee it was paying.
- **Every spent scriptPubKey.** A signer learns whether each input is
  its own, and which kind of output it spends.

Both come from the outputs the transaction spends, not from the
transaction, so a signer must be given them. That is why
`bt_sigmsg_taproot` takes `amounts` and `spk` for every input.

### Hash types

The hash type is one byte: the low bits pick the outputs, and 0x80,
ANYONECANPAY, drops the other inputs.

| Hash type | Inputs signed | Outputs signed |
| --- | --- | --- |
| 0x00 DEFAULT | all | all |
| 0x01 ALL | all | all |
| 0x02 NONE | all | none |
| 0x03 SINGLE | all | the one at this input's index |
| 0x81 ALL\|ANYONECANPAY | this one | all |
| 0x82 NONE\|ANYONECANPAY | this one | none |
| 0x83 SINGLE\|ANYONECANPAY | this one | the one at this input's index |

Every other byte fails, among them 0x80 and 0x04. Legacy and segwit v0
signatures accept any byte and read only its bits; BIP-341 defines seven
and refuses the rest, so a future soft fork can give the others a
meaning. SINGLE with no output at the input's index fails too. Legacy
signatures signed the number 1 in that case, a known bug, and BIP-143
signed a zero hash. BIP-341 refuses it.

SIGHASH_DEFAULT and SIGHASH_ALL commit to the same fields. A key-path
signature of 64 bytes means DEFAULT, and 65 bytes carry the hash type in
the last byte (chapter 41). The hash type byte itself is in SigMsg, so
the two give different hashes. A signature made for one can't be
replayed as the other, and one byte longer is the only cost of writing
ALL.

### The annex and the spend type

A Taproot witness may end with an *annex*: a last element that starts
with 0x50, after the control block or after the key-path signature.
Consensus doesn't read it yet. It is space for a later soft fork, and
the signature commits to it so that no one can add or change it after
signing: SigMsg carries `sha_annex`, SHA-256 of the annex with its
compact-size length, 0x50 included.

`spend_type` records what follows: bit 0 is set when there is an annex,
and bit 1 (ext_flag × 2) when an extension follows the message. A
key-path spend has ext_flag 0; a tapscript signature has ext_flag 1.

### The script path's extension

A signature checked by a tapscript (chapter 44) appends BIP-342's
extension:

| Field | Bytes | What |
| --- | --- | --- |
| tapleaf_hash | 32 | the TapLeaf hash of the script being run (chapter 42) |
| key_version | 1 | 0x00, for BIP-340 keys |
| codesep_pos | 4 | the opcode position of the last `OP_CODESEPARATOR` run, or 0xffffffff |

The leaf hash ties the signature to one script, so a signature for one
leaf can't be used in another leaf of the same tree. `codesep_pos` lets
a script commit to the part of itself that ran; the position counts
opcodes from the script's start, not bytes.

BIP-341 gives SigMsg's length as 174 − 49 × anyonecanpay − 32 × none +
32 × has_annex. With the epoch byte and the 37-byte extension, the
longest message is 1 + 174 + 32 + 37 = 244 bytes, `BT_SIGMSG`.

<!-- source: coax/lib/bitcoin/sighash.coax bip341 -->
```jai
BT_SIGMSG :: 244;            // the longest message: epoch 0, 206 bytes of SigMsg, 37 of extension
BT_NO_CODESEP :: 0xffffffff; // codesep when no OP_CODESEPARATOR has run
BT_ANNEX_TAG :: 0x50;        // an annex's first byte

// sha_annex: SHA-256 of the annex with its CompactSize length, fed in
// pieces so that an annex of any length hashes.
bt_annex_hash :: (annex:[]u8,out:[]u8) {
  st:[SHA_STATE_BYTES]u8;c:[9]u8;
  #assert sha256_init(st[..])==SHA_OK && sha256_update_checked(st[..],c[0..<bt_var_put(c[..],0,annex.count)],false)==SHA_OK;
  at:=0;
  while at<annex.count {
    end:=at+65536;if end>annex.count {end=annex.count;}
    #assert sha256_update_checked(st[..],annex[at..<end],false)==SHA_OK;
    at=end;
  }
  #assert sha256_final_checked(st[..],out,false)==SHA_OK;
}
// The epoch byte 0 then BIP-341's SigMsg for input index, into m
// (BT_SIGMSG bytes or more), given every spent output's amount and
// scriptPubKey; returns the length written,
// or -1 on a refusal. annex is empty when the witness has none, else the
// whole annex, 0x50 first. leaf is empty for a key-path spend; for a
// script-path spend it is the TapLeaf hash of the script being run, and
// BIP-342's extension follows: leaf, key_version 0, and codesep, the
// position of the last OP_CODESEPARATOR run (BT_NO_CODESEP for none). Hash
// types 0 (SIGHASH_DEFAULT), 1, 2, 3 and 0x81 to 0x83; SIGHASH_SINGLE
// needs an output at index.
bt_sigmsg_taproot :: (b:[]u8,tp:BtTx,index:int,amounts:[]int,spk:[]BtScript,hashtype:int,annex:[]u8,leaf:[]u8,codesep:int,m:[]u8)->int {t:=tp;
  if !t.ok || index<0 || index>=t.nin || amounts.count!=t.nin || spk.count!=t.nin || m.count<BT_SIGMSG {return -1;}
  // A hash type that doesn't exist refuses before any hashing.
  if !((hashtype>=0 && hashtype<=3) || (hashtype>=0x81 && hashtype<=0x83)) || ((hashtype&3)==BT_SINGLE && index>=t.nout) {return -1;}
  h:[5][32]u8;if !bt_tap_hashes(b,t,amounts,spk,h[..]) {return -1;}
  single:=b[0..<0];if index<t.nout {single=b[t.out_at[index]..<t.out_at[index]+t.out_n[index]];}
  return bt_sigmsg_taproot_parts(h[..],t.version[..],t.locktime[..],index,t.inputs[index].prev[..],amounts[index],
    spk[index].b[0..<spk[index].n],t.inputs[index].seq[..],single,hashtype,annex,leaf,codesep,m);
}
// The same message from its parts, for a verifier that reads the
// transaction where it lies and has the five hashes already (Bitcoin
// Core's PrecomputedTransactionData; chapter 44's interpreter): h holds
// sha_prevouts, sha_amounts, sha_scriptpubkeys, sha_sequences and
// sha_outputs; version, locktime, prev (the outpoint) and seq are the
// transaction's bytes; amount and spk are the spent output's; single is the
// output at index, serialized, or empty when there is none.
bt_sigmsg_taproot_parts :: (h:[][32]u8,version:[]u8,locktime:[]u8,index:int,prev:[]u8,amount:int,spk:[]u8,seq:[]u8,single:[]u8,hashtype:int,annex:[]u8,leaf:[]u8,codesep:int,m:[]u8)->int {
  base:=hashtype&3;anyone:=(hashtype&BT_ANYONE)!=0;
  if !((hashtype>=0 && hashtype<=3) || (hashtype>=0x81 && hashtype<=0x83)) {return -1;}
  if m.count<BT_SIGMSG || amount<0 || (base==BT_SINGLE && single.count==0) {return -1;}
  if (annex.count>0 && annex[0]!=BT_ANNEX_TAG) || (leaf.count!=0 && leaf.count!=32) || codesep<0 || codesep>BT_NO_CODESEP {return -1;}
  m[0]=0;m[1]=cast(u8)hashtype;                               // epoch 0, then hash_type
  n:=bt_put(m,2,version);n=bt_put(m,n,locktime);
  if !anyone {for k:0..<4 {n=bt_put(m,n,h[k][..]);}}          // sha_prevouts to sha_sequences
  if base!=BT_SINGLE && base!=BT_NONE {n=bt_put(m,n,h[4][..]);} // sha_outputs: ALL and DEFAULT
  ext:=0;if leaf.count==32 {ext=1;}
  present:=0;if annex.count>0 {present=1;}
  m[n]=cast(u8)(ext*2+present);n+=1;                          // spend_type
  if anyone {
    n=bt_put(m,n,prev);n=bt_le(m,n,amount,8);
    n=bt_var_put(m,n,spk.count);n=bt_put(m,n,spk);
    n=bt_put(m,n,seq);
  } else {n=bt_le(m,n,index,4);}
  if present==1 {bt_annex_hash(annex,m[n..<n+32]);n+=32;}      // sha_annex
  if base==BT_SINGLE {bt_sha(single,m[n..<n+32]);n+=32;}
  if ext==1 {n=bt_put(m,n,leaf);m[n]=0;n+=1;n=bt_le(m,n,codesep,4);}
  return n;
}
// The signature hash hash_TapSighash(0x00 || SigMsg), into out (32
// bytes); false when the message refuses.
bt_sighash_taproot_ext :: (b:[]u8,tp:BtTx,index:int,amounts:[]int,spk:[]BtScript,hashtype:int,annex:[]u8,leaf:[]u8,codesep:int,out:[]u8)->bool {
  if out.count!=32 {return false;}
  m:[BT_SIGMSG]u8;n:=bt_sigmsg_taproot(b,tp,index,amounts,spk,hashtype,annex,leaf,codesep,m[..]);
  if n<0 {return false;}
  bt_tagged(BH_TAPSIGHASH,m[0..<n],out);
  return true;
}
// The key-path signature hash with no annex.
bt_sighash_taproot :: (b:[]u8,tp:BtTx,index:int,amounts:[]int,spk:[]BtScript,hashtype:int,out:[]u8)->bool {
  return bt_sighash_taproot_ext(b,tp,index,amounts,spk,hashtype,b[0..<0],b[0..<0],BT_NO_CODESEP,out);
}
```

`bt_tap_hashes`, which computes the five `sha_` hashes, sits above this
region in the same file. `bt_sighash_taproot` is the key-path hash
chapter 41 used, and is now the extended hash with no annex and no leaf.

## The example

The example is
[bitcoin/examples/taproot-sighash.coax](../examples/taproot-sighash.coax).
It computes the messages of BIP-341's seven key-path inputs, takes one
apart field by field, compares SIGHASH_DEFAULT with ALL, builds the four
spend types and every hash type of a script-path spend with an annex,
checks 30 messages against Bitcoin Core's, and tries what BIP-341
refuses.

<!-- source: bitcoin/examples/taproot-sighash.coax example -->
```jai
#load "../../coax/lib/bitcoin/taproot.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// One byte as two hex digits.
hex_byte :: (v: int) {
  b: [1] u8; text: [2] u8; b[0] = cast(u8) v;
  r := hex_encode_checked(cast(string) b[..], text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A hash type's name.
hashtype_name :: (ht: int) -> string {
  names := [4] string.["DEFAULT", "ALL", "NONE", "SINGLE"];
  anyone := [4] string.["?", "ALL|ANYONECANPAY", "NONE|ANYONECANPAY", "SINGLE|ANYONECANPAY"];
  if ht >= 0 && ht <= 3 { return names[ht]; }
  if ht >= 0x81 && ht <= 0x83 { return anyone[ht & 3]; }
  return "?";
}
// "matches" or "differs".
same :: (ok: bool) -> string { return ifx ok then "matches" else "differs"; }
// The byte offset of spend_type in a message of this hash type.
spend_at :: (ht: int) -> int {
  at := 2 + 8;
  if (ht & BT_ANYONE) == 0 { at += 128; }
  if (ht & 3) != BT_NONE && (ht & 3) != BT_SINGLE { at += 32; }
  return at;
}

main :: () {
  tx: [BT_TX] u8; spk: [9] BtScript; m: [BT_SIGMSG] u8; e: [BT_SIGMSG] u8; h: [32] u8;
  raw := bt_unhex(BT_V341_TX, tx[..]); t := bt_tx_read(raw);
  amounts := BT_V341_AMOUNT;
  for k: 0..<9 { spk[k] = bt_script(BT_V341_SPK[k]); }
  none := m[0..<0];

  // BIP-341's seven key-path inputs: the message and its hash.
  print("BIP-341's key-path signature messages\n");
  for i: 0..<BT_V341.count {
    v := BT_V341[i];
    n := bt_sigmsg_taproot(raw, t, v.index, amounts[..], spk[..], v.hashtype, none, none, BT_NO_CODESEP, m[..]);
    #assert n > 0 && bt_sighash_taproot(raw, t, v.index, amounts[..], spk[..], v.hashtype, h[..]);
    print("  input % (0x", v.index); hex_byte(v.hashtype);
    print(" %): % bytes, message %, sighash %\n", hashtype_name(v.hashtype), n,
          same(bt_same(m[0..<n], bt_unhex(TR_SIGMSG[i], e[..]))), same(bt_hex_is(h[..], v.sighash)));
  }

  // One message, field by field.
  print("input 1 with SIGHASH_DEFAULT, field by field\n");
  n := bt_sigmsg_taproot(raw, t, 1, amounts[..], spk[..], BT_DEFAULT, none, none, BT_NO_CODESEP, m[..]);
  show("  epoch", m[0..<1]); show("  hash_type", m[1..<2]);
  show("  nVersion", m[2..<6]); show("  nLockTime", m[6..<10]);
  show("  sha_prevouts", m[10..<42]); show("  sha_amounts", m[42..<74]);
  show("  sha_scriptpubkeys", m[74..<106]); show("  sha_sequences", m[106..<138]);
  show("  sha_outputs", m[138..<170]); show("  spend_type", m[170..<171]);
  show("  input_index", m[171..<175]);
  print("  % bytes\n", n);

  // DEFAULT and ALL commit to the same fields; only the hash type byte differs.
  print("SIGHASH_DEFAULT and SIGHASH_ALL at input 1\n");
  d: [BT_SIGMSG] u8; hd: [32] u8;
  nd := bt_sigmsg_taproot(raw, t, 1, amounts[..], spk[..], BT_DEFAULT, none, none, BT_NO_CODESEP, d[..]);
  na := bt_sigmsg_taproot(raw, t, 1, amounts[..], spk[..], BT_ALL, none, none, BT_NO_CODESEP, m[..]);
  differ := 0; at := -1;
  for k: 0..<nd { if d[k] != m[k] { differ += 1; at = k; } }
  print("  % and % bytes, % byte differs, at offset %\n", nd, na, differ, at);
  #assert bt_sighash_taproot(raw, t, 1, amounts[..], spk[..], BT_DEFAULT, hd[..]) && bt_sighash_taproot(raw, t, 1, amounts[..], spk[..], BT_ALL, h[..]);
  show("  DEFAULT sighash", hd[..]); show("  ALL sighash", h[..]);

  // The four spend types: an annex adds sha_annex, the script path adds
  // BIP-342's extension.
  print("input 1, SIGHASH_DEFAULT, four spend types\n");
  an: [32] u8; an[0] = BT_ANNEX_TAG; for k: 1..<32 { an[k] = cast(u8) k; }
  sc: [64] u8; leaf: [32] u8;
  script := bt_unhex(TR_LEAVES[0].script, sc[..]);
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, script, leaf[..]) == TR_OK;
  labels := [4] string.["key path", "key path with an annex", "script path", "script path with an annex"];
  for s: 0..<4 {
    annex := ifx (s & 1) == 1 then an[..] else none;
    lh := ifx s >= 2 then leaf[..] else none;
    n = bt_sigmsg_taproot(raw, t, 1, amounts[..], spk[..], BT_DEFAULT, annex, lh, BT_NO_CODESEP, m[..]);
    #assert bt_sighash_taproot_ext(raw, t, 1, amounts[..], spk[..], BT_DEFAULT, annex, lh, BT_NO_CODESEP, h[..]);
    print("  %: spend_type %, % bytes\n", labels[s], m[spend_at(BT_DEFAULT)], n);
    show("    sighash", h[..]);
  }
  print("the script path's extension, after the annex's hash\n");
  show("  annex", an[..]);
  show("  sha_annex", m[175..<207]);
  show("  tapleaf_hash", m[207..<239]); show("  key_version", m[239..<240]);
  show("  codesep_pos", m[240..<244]);
  n = bt_sigmsg_taproot(raw, t, 1, amounts[..], spk[..], BT_DEFAULT, none, leaf[..], 3, m[..]);
  show("  codesep_pos after an OP_CODESEPARATOR at opcode 3", m[n - 4..<n]);

  // Every hash type, script path with an annex: which fields each keeps.
  print("input 1, script path with an annex, every hash type\n");
  types := [7] int.[0, 1, 2, 3, 0x81, 0x82, 0x83];
  for ht: types {
    n = bt_sigmsg_taproot(raw, t, 1, amounts[..], spk[..], ht, an[..], leaf[..], BT_NO_CODESEP, m[..]);
    print("  0x"); hex_byte(ht);
    print(" %: % bytes\n", hashtype_name(ht), n);
  }

  // Bitcoin Core's TaprootSignatureMsg, on the annex and script-path cases.
  matched := 0;
  for i: 0..<TR_EXT.count {
    v := TR_EXT[i]; a2: [300] u8; s2: [64] u8; l2: [32] u8;
    annex := bt_unhex(v.annex, a2[..]); sct := bt_unhex(v.script, s2[..]);
    lh := l2[0..<0];
    if sct.count > 0 { #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, sct, l2[..]) == TR_OK; lh = l2[..]; }
    n = bt_sigmsg_taproot(raw, t, v.index, amounts[..], spk[..], v.hashtype, annex, lh, v.codesep, m[..]);
    if n > 0 && bt_same(m[0..<n], bt_unhex(v.msg, e[..])) { matched += 1; }
  }
  print("Bitcoin Core's TaprootSignatureMsg: % of % annex and script-path messages match\n", matched, TR_EXT.count);

  // What BIP-341 refuses.
  print("refusals\n");
  signs := 0;
  for ht: 0..<256 { if bt_sigmsg_taproot(raw, t, 0, amounts[..], spk[..], ht, none, none, BT_NO_CODESEP, m[..]) > 0 { signs += 1; } }
  print("  hash types that sign: % of 256\n", signs);
  r := bt_sigmsg_taproot(raw, t, 0, amounts[..], spk[..], 0x80, none, none, BT_NO_CODESEP, m[..]);
  print("  0x80, ANYONECANPAY alone: %\n", ifx r < 0 then "refused" else "signed");
  r = bt_sigmsg_taproot(raw, t, 2, amounts[..], spk[..], BT_SINGLE, none, none, BT_NO_CODESEP, m[..]);
  print("  SINGLE at input 2, with 2 outputs: %\n", ifx r < 0 then "refused" else "signed");
  an[0] = 0x51;
  r = bt_sigmsg_taproot(raw, t, 0, amounts[..], spk[..], BT_DEFAULT, an[..], none, BT_NO_CODESEP, m[..]);
  print("  an annex that starts with 0x51: %\n", ifx r < 0 then "refused" else "signed");
  r = bt_sigmsg_taproot(raw, t, 0, amounts[..], spk[..], BT_DEFAULT, none, leaf[0..<31], BT_NO_CODESEP, m[..]);
  print("  a 31-byte leaf hash: %\n", ifx r < 0 then "refused" else "signed");
  r = bt_sigmsg_taproot(raw, t, 0, amounts[..], spk[..], BT_DEFAULT, none, leaf[..], BT_NO_CODESEP + 1, m[..]);
  print("  codesep_pos 2^32: %\n", ifx r < 0 then "refused" else "signed");
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/taproot-sighash.coax --run-steps 250000000
```

It prints:

```text
BIP-341's key-path signature messages
  input 0 (0x03 SINGLE): 175 bytes, message matches, sighash matches
  input 1 (0x83 SINGLE|ANYONECANPAY): 126 bytes, message matches, sighash matches
  input 3 (0x01 ALL): 175 bytes, message matches, sighash matches
  input 4 (0x00 DEFAULT): 175 bytes, message matches, sighash matches
  input 6 (0x02 NONE): 143 bytes, message matches, sighash matches
  input 7 (0x82 NONE|ANYONECANPAY): 94 bytes, message matches, sighash matches
  input 8 (0x81 ALL|ANYONECANPAY): 126 bytes, message matches, sighash matches
input 1 with SIGHASH_DEFAULT, field by field
  epoch: 00
  hash_type: 00
  nVersion: 02000000
  nLockTime: 0065cd1d
  sha_prevouts: e3b33bb4ef3a52ad1fffb555c0d82828eb22737036eaeb02a235d82b909c4c3f
  sha_amounts: 58a6964a4f5f8f0b642ded0a8a553be7622a719da71d1f5befcefcdee8e0fde6
  sha_scriptpubkeys: 23ad0f61ad2bca5ba6a7693f50fce988e17c3780bf2b1e720cfbb38fbdd52e21
  sha_sequences: 18959c7221ab5ce9e26c3cd67b22c24f8baa54bac281d8e6b05e400e6c3a957e
  sha_outputs: a2e6dab7c1f0dcd297c8d61647fd17d821541ea69c3cc37dcbad7f90d4eb4bc5
  spend_type: 00
  input_index: 01000000
  175 bytes
SIGHASH_DEFAULT and SIGHASH_ALL at input 1
  175 and 175 bytes, 1 byte differs, at offset 1
  DEFAULT sighash: fac9b8d698672e521b61aa35991571e304fca66c308faa33ee061779a2f4c88f
  ALL sighash: 8efb150df36b7b9e7b0e8c610819f73615f62c284273bf3dfb7720b4f4e07671
input 1, SIGHASH_DEFAULT, four spend types
  key path: spend_type 0, 175 bytes
    sighash: fac9b8d698672e521b61aa35991571e304fca66c308faa33ee061779a2f4c88f
  key path with an annex: spend_type 1, 207 bytes
    sighash: 008220b1967361bd387950cb536fb50e2d8b6ee6f209a8273f03dda93b47ede9
  script path: spend_type 2, 212 bytes
    sighash: c6c88b99952a1da24f5efe63b6da76032667e88e0caeb2059bdb64ebfce0fcbe
  script path with an annex: spend_type 3, 244 bytes
    sighash: d2be8edbd2bbba7c9e1988dad9c9eb2816516355ea061d6e11fa6c769384a9ef
the script path's extension, after the annex's hash
  annex: 500102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
  sha_annex: 1f8a963682305474b8236bfea00a8a326948c1e24634f51d2adc1ed835737479
  tapleaf_hash: 5b75adecf53548f3ec6ad7d78383bf84cc57b55a3127c72b9a2481752dd88b21
  key_version: 00
  codesep_pos: ffffffff
  codesep_pos after an OP_CODESEPARATOR at opcode 3: 03000000
input 1, script path with an annex, every hash type
  0x00 DEFAULT: 244 bytes
  0x01 ALL: 244 bytes
  0x02 NONE: 212 bytes
  0x03 SINGLE: 244 bytes
  0x81 ALL|ANYONECANPAY: 195 bytes
  0x82 NONE|ANYONECANPAY: 163 bytes
  0x83 SINGLE|ANYONECANPAY: 195 bytes
Bitcoin Core's TaprootSignatureMsg: 30 of 30 annex and script-path messages match
refusals
  hash types that sign: 7 of 256
  0x80, ANYONECANPAY alone: refused
  SINGLE at input 2, with 2 outputs: refused
  an annex that starts with 0x51: refused
  a 31-byte leaf hash: refused
  codesep_pos 2^32: refused
```

What the output shows:

- Every one of BIP-341's key-path messages and hashes matches, with
  lengths from 94 bytes (NONE|ANYONECANPAY) to 175.
- SIGHASH_DEFAULT and SIGHASH_ALL messages differ in one byte, the hash
  type at offset 1, and their hashes are unrelated.
- An annex adds 32 bytes and sets bit 0 of spend_type; the script path
  adds 37 and sets bit 1. With both, the message is the longest, 244
  bytes.
- The extension's leaf hash is the published TapLeaf hash of vector 1's
  leaf, `5b75adec…` (chapter 42).
- Seven hash types of 256 sign; 0x80, SINGLE past the outputs, an annex
  without 0x50, a short leaf hash and a codesep past four bytes refuse.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/taproot-sighash.out](../fixtures/taproot-sighash.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/taproot-sighash.coax](../examples/checks/taproot-sighash.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/taproot-sighash.coax check -->
```jai
// What each hash type leaves out can change without changing the hash:
// ANYONECANPAY drops the other inputs, NONE drops every output, SINGLE
// keeps only the output at the input's index.
#test "sighash: what a hash type leaves out can change" #steps 50000000 {
  b: [BT_TX] u8; spk: [9] BtScript; h: [7][32] u8; g: [32] u8;
  raw := bt_unhex(BT_V341_TX, b[..]); t := bt_tx_read(raw);
  amounts := BT_V341_AMOUNT; for k: 0..<9 { spk[k] = bt_script(BT_V341_SPK[k]); }
  types := [7] int.[0, 1, 2, 3, 0x81, 0x82, 0x83];
  for k: 0..<7 { #assert bt_sighash_taproot(raw, t, 1, amounts[..], spk[..], types[k], h[k][..]); }
  // Another input's amount: only the ANYONECANPAY types keep their hash.
  amounts[0] += 1;
  for k: 0..<7 {
    #assert bt_sighash_taproot(raw, t, 1, amounts[..], spk[..], types[k], g[..]);
    #assert bt_same(g[..], h[k][..]) == ((types[k] & BT_ANYONE) != 0);
  }
  amounts[0] -= 1;
  // Output 0's value: NONE keeps its hash, and so does SINGLE at input 1.
  b[t.out_at[0]] ^= 1;
  for k: 0..<7 {
    #assert bt_sighash_taproot(raw, t, 1, amounts[..], spk[..], types[k], g[..]);
    #assert bt_same(g[..], h[k][..]) == ((types[k] & 3) == BT_NONE || (types[k] & 3) == BT_SINGLE);
  }
}
// The length BIP-341 gives, 174 - 49 for ANYONECANPAY - 32 for NONE + 32
// for an annex, plus the epoch byte and BIP-342's 37 bytes, for every hash
// type and spend type.
#test "sighash: every message has BIP-341's length" #steps 50000000 {
  b: [BT_TX] u8; spk: [9] BtScript; m: [BT_SIGMSG] u8; an: [1] u8; leaf: [32] u8; an[0] = 0x50;
  raw := bt_unhex(BT_V341_TX, b[..]); t := bt_tx_read(raw);
  amounts := BT_V341_AMOUNT; for k: 0..<9 { spk[k] = bt_script(BT_V341_SPK[k]); }
  types := [7] int.[0, 1, 2, 3, 0x81, 0x82, 0x83];
  for ht: types { for s: 0..<4 {
    annex := an[0..<s & 1]; lh := leaf[0..<ifx s >= 2 then 32 else 0];
    n := bt_sigmsg_taproot(raw, t, 0, amounts[..], spk[..], ht, annex, lh, BT_NO_CODESEP, m[..]);
    want := 1 + 174 + 32 * (s & 1) + 37 * (s >> 1);
    if (ht & BT_ANYONE) != 0 { want -= 49; }
    if (ht & 3) == BT_NONE { want -= 32; }
    #assert n == want && cast(int) m[spend_at(ht)] == s;
  } }
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/taproot-sighash.coax --run-steps 250000000 --only sighash:
```

On 2026-11-03 both checks pass.

### The library's own checks

`taproot.coax` compares `bt_sigmsg_taproot` with BIP-341's seven
published key-path messages and hashes, and with 30 annex and
script-path messages computed by Bitcoin Core's `TaprootSignatureMsg`:
every hash type with an annex alone, the script path alone, both, and a
codesep position, plus a 300-byte annex (a three-byte compact size) and
ANYONECANPAY at the last input. Each check also reads spend_type and the
length. Other checks cover DEFAULT against ALL, the 249 hash types that
refuse, SINGLE past the outputs, the annex tag, leaf hash sizes, codesep
from −1 to 2^32, a buffer one byte short of the longest message, and a
changed annex, leaf or codesep changing the hash. `sighash.coax`
checks the extension's offsets directly and hashes a 70,000-byte annex,
longer than one SHA-256 update and with a five-byte compact size. Its
existing checks of BIP-341's key-path hashes and intermediate hashes
still pass through the new code.

```shell
coax/bin/coax test coax/lib/bitcoin/taproot.coax --run-steps 250000000
coax/bin/coax test coax/lib/bitcoin/sighash.coax --run-steps 250000000
```

### Bitcoin Core's own Python

BIP-341's vectors cover only key-path spends without an annex. For the
rest, [taproot_reference.py](../../coax/tests/bitcoin/taproot_reference.py)
reads `TaprootSignatureMsg` and the `BIP341_sha_*` functions out of
Bitcoin Core's `test_framework/script.py`, pinned, and runs them as
written, on BIP-341's transaction. They give BIP-341's seven published
messages first.
[make_taproot_sighash_vectors.py](../../coax/tests/bitcoin/make_taproot_sighash_vectors.py)
uses them to write the 30 extension messages the Coax checks compare.
[taproot_examples.py](../../coax/tests/bitcoin/taproot_examples.py)
writes the example's output from Core's function and recomputes those
30 messages from the generated file.

```shell
python3 coax/tests/bitcoin/taproot_reference.py
python3 coax/tests/bitcoin/taproot_examples.py sighash bitcoin/fixtures/taproot-sighash.out
```

Core's test framework signs SINGLE past the outputs with 32 zero bytes,
for tests that expect that signature to fail. Consensus refuses it, as
`SignatureHashSchnorr` in `interpreter.cpp` does, and so does this code;
the Python check treats it as a refusal.

On 2026-11-03 all 57 lines agree. Mutation testing was not run for
this chapter (owner speed order).

## What this chapter leaves out

- Running tapscript, and the `OP_CODESEPARATOR` positions it records:
  chapter 44.
- The script-path witness as a whole and spending BIP-341's script
  paths: chapter 45.
- Legacy signature hashes and their SINGLE bug: chapter 37. Segwit v0
  hashes: chapter 38.
- Caching the five `sha_` hashes across a transaction's inputs, as
  Bitcoin Core does in `PrecomputedTransactionData`: this code
  recomputes them for each input.

## Summary

- A Taproot signature signs `hash_TapSighash(0x00 || SigMsg || ext)`.
- SigMsg commits to every spent amount and scriptPubKey, so a signer
  knows its fee and which inputs are its own.
- Seven hash types are defined; every other byte, and SINGLE past the
  outputs, fails.
- DEFAULT and ALL sign the same fields, under different hash types.
- An annex adds `sha_annex`; a tapscript signature adds the leaf hash,
  key version 0 and `codesep_pos`. `spend_type` says which are there.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md)

## Sources

- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  "Signature validation rules" and "Common signature message", pinned at
  [coax/tests/vectors/upstream/bip341/bip-0341.mediawiki](../../coax/tests/vectors/upstream/bip341/bip-0341.mediawiki),
  with its wallet test vectors at
  [wallet-test-vectors.json](../../coax/tests/vectors/upstream/bip341/wallet-test-vectors.json).
- [BIP-342](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki),
  "Common signature message extension".
- Bitcoin Core at `ddea9196`: `SignatureHashSchnorr` in
  `script/interpreter.cpp`, pinned at
  [coax/tests/vectors/upstream/bitcoin-core/interpreter.cpp](../../coax/tests/vectors/upstream/bitcoin-core/interpreter.cpp),
  and `TaprootSignatureMsg` in `test/functional/test_framework/script.py`,
  pinned at
  [coax/tests/vectors/upstream/bitcoin-core/script.py](../../coax/tests/vectors/upstream/bitcoin-core/script.py).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
