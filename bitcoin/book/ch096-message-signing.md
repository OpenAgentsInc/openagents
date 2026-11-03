# 96. Message signing

Chapter 22 signed messages the way Bitcoin Core's `signmessage` does: a
recoverable ECDSA signature whose key hashes to a P2PKH address. That
works only for P2PKH. A P2WSH multisig address, a Taproot address or a
time-locked script has no single key to recover.

BIP-322 signs for any address by treating the signature as a spend. It
defines two transactions that can never be mined. The first, `to_spend`,
pays nothing to the address and commits to the message in its input.
The second, `to_sign`, spends that output. A signature is whatever
satisfies the address's script in `to_sign`: one witness stack, the
whole transaction, or a PSBT that spends other coins too. A verifier
runs the script interpreter, so anything a wallet can spend from, it can
sign for.

## What this chapter teaches

- The message hash, a BIP-340 tagged hash, and the two virtual
  transactions `to_spend` and `to_sign`.
- The three encodings: simple (`smp`, a witness stack), full (`ful`, the
  signed `to_sign`) and proof of funds (`pof`, a finalized PSBT with more
  inputs), and the legacy format for P2PKH.
- Verification's three answers: valid at time T and age S, inconclusive,
  and invalid, and the required and upgradeable rules that separate them.
- Signing as a PSBT, with BIP-322's `PSBT_GLOBAL_GENERIC_SIGNED_MESSAGE`
  field, through chapter 94's Signer, Input Finalizer and Transaction
  Extractor.
- Taproot key and script paths, with a bounded tapscript evaluator that
  answers inconclusive for any opcode it doesn't know.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bip322.coax](../../coax/lib/bitcoin/bip322.coax) | `bip322_message_hash` | The tagged hash of the message |
| | `bip322_to_spend`, `bip322_to_sign` | The two virtual transactions |
| | `b322_decode` | A signature's bytes as `to_sign` |
| | `b322_input` | One input's script, under the required and the upgradeable rules |
| | `b322_taproot`, `b322_tapscript`, `b322_tap_sig` | Taproot key paths, script paths and a bounded tapscript evaluator |
| | `b322_pof_spent` | A proof of funds input's spent output |
| | `bip322_verify_vartime` | BIP-322's verification |
| | `bip322_sign_vartime`, `b322_else_branch`, `b322_sign_leaf` | Signing through the PSBT roles |
| [psbt.coax](../../coax/lib/bitcoin/psbt.coax) | `psbt_sign_vartime`, `psbt_finalize_input_vartime`, `psbt_extract_vartime` | Chapter 94's roles |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_verify_vartime`, `si_run` | Chapter 36's interpreter |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_commitment_vartime` | Chapter 42's control-block check |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sighash_taproot_ext` | Chapter 43's BIP-341 and BIP-342 signature hash |
| [signmessage.coax](../../coax/lib/bitcoin/signmessage.coax) | `signed_message_sign`, `verify_message_vartime` | Chapter 22's legacy format |

## Results

A verification returns a state, a reason when the state is invalid, and
the format the signature's prefix named. A valid signature also carries
`to_sign`'s lock time and its first input's sequence: BIP-322 calls it
valid at time T and age S, because a signature can promise that coins
stay locked until then.

<!-- source: coax/lib/bitcoin/bip322.coax codes -->
```jai
// The three states BIP-322's verification outputs.
B322_VALID :: 0; B322_INVALID :: 1; B322_INCONCLUSIVE :: 2;
B322_STATES :: [3] string.["valid", "invalid", "inconclusive"];
// Why a signature is invalid.
B322_OK :: 0;
B322_BASE64 :: 1;         // not Base64
B322_SHORT :: 2;          // nothing after the prefix
B322_FULL :: 3;           // a full or proof-of-funds signature that doesn't parse
B322_SHAPE :: 4;          // to_sign doesn't spend to_spend, or doesn't pay 0 to OP_RETURN
B322_SCRIPT :: 5;         // an input's script fails
B322_ADDRESS :: 6;        // not a mainnet address
B322_LEGACY_SIG :: 7;     // a legacy signature that doesn't verify
B322_REASONS :: [8] string.["ok", "base64", "signature too short", "full variant does not parse",
  "not a BIP-322 transaction", "script failed", "address", "legacy signature"];
// The signature's format, from its prefix.
B322_LEGACY :: 0; B322_SIMPLE :: 1; B322_FULL_FORMAT :: 2; B322_POF :: 3; B322_BARE :: 4;
B322_FORMATS :: [5] string.["legacy", "smp", "ful", "pof", "bare"];
```

## The message hash and to_spend

The message is hashed as it is, without a length or a terminator, under
the tag `BIP0322-signed-message`. The tag keeps the hash apart from
every other use of SHA-256 in Bitcoin.

<!-- source: coax/lib/bitcoin/bip322.coax hash -->
```jai
// message_hash: the BIP-340 tagged hash of the message as it is, with
// the tag BIP0322-signed-message.
bip322_message_hash :: (message: string, out: [] u8) -> bool #export_c {
  if out.count != 32 { return false; }
  state: [SHA_STATE_BYTES] u8;
  ok := sha256_tagged_init(state[..], "BIP0322-signed-message") == SHA_OK && sm_absorb(state[..], message);
  return ok && sha256_final_checked(state[..], out, false) == SHA_OK;
}
```

`to_spend` has version 0 and one input from the outpoint
`000...000:FFFFFFFF`, which no transaction can create. Its scriptSig
pushes the message hash, so its txid commits to the message. Its one
output pays 0 to the address's scriptPubKey, which BIP-322 calls the
message challenge. `to_sign` spends that output and pays 0 to
`OP_RETURN`, so even if both were broadcast, no coin would move.

<!-- source: coax/lib/bitcoin/bip322.coax tospend -->
```jai
// to_spend: version 0, one input from 000...000:FFFFFFFF with sequence 0
// and the scriptSig OP_0 PUSH32[message_hash], one output of 0 to the
// challenge (the address's scriptPubKey), lock time 0. Its length in out.
bip322_to_spend :: (challenge: [] u8, message: string, out: [] u8) -> int #export_c {
  if challenge.count > AD_SCRIPT_MAX || out.count < B322_SPEND { return -1; }
  for k: 0..<B322_SPEND { out[k] = 0; }
  out[4] = 1;                                              // one input
  for k: 37..<41 { out[k] = 0xff; }                        // the outpoint's index
  out[41] = 34; out[42] = 0; out[43] = 32;                 // scriptSig: OP_0 PUSH32
  #assert bip322_message_hash(message, out[44..<76]);
  out[80] = 1;                                             // sequence 0, then one output
  out[89] = cast(u8) challenge.count;                      // value 0, then the challenge
  at := tx_put(out, 90, challenge);
  return at + 4;                                           // lock time 0
}
// to_sign before it is signed: the version, one input spending to_spend's
// output 0 with an empty scriptSig and the sequence, one output of 0 to
// OP_RETURN, the lock time. Its length in out (61 bytes).
bip322_to_sign :: (txid: [] u8, version: int, locktime: int, sequence: int, out: [] u8) -> int #export_c {
  if txid.count != 32 || out.count < 61 { return -1; }
  at := bt_le(out, 0, version, 4);
  out[at] = 1; at = tx_put(out, at + 1, txid);
  at = bt_le(out, at, 0, 4);                               // output 0
  out[at] = 0; at = bt_le(out, at + 1, sequence, 4);
  out[at] = 1; at = bt_le(out, at + 1, 0, 8);              // one output of 0
  out[at] = 1; out[at+1] = OP_RETURN;
  return bt_le(out, at + 2, locktime, 4);
}
```

## Decoding a signature

A simple signature is a witness stack, encoded as in a transaction.
The verifier builds `to_sign` around it: segwit's marker and flag after
the version, the stack before the lock time. Then chapter 34's reader
checks it like any transaction, so a stack that runs short or leaves
bytes over doesn't parse. A full signature is `to_sign` itself. A proof
of funds is a PSBT, read by chapter 94's reader, whose finalized
transaction is `to_sign`.

<!-- source: coax/lib/bitcoin/bip322.coax decode -->
```jai
// The signature's bytes into x.tx: a simple signature's witness stack
// becomes to_sign with that witness; a full one is to_sign; a proof of
// funds is a PSBT whose finalized transaction is to_sign. Returns
// B322_OK or the reason it is invalid.
b322_decode :: (format: int, text: string, x: [] B322Work) -> int {
  n := base64_decode_size(text);
  if n < 0 || n > B322_RAW { return B322_BASE64; }
  if n == 0 { return B322_SHORT; }
  #assert base64_decode_checked(text, x[0].raw[0..<n], false).error == NC_OK;
  raw := x[0].raw[0..<n];
  if format == B322_FULL_FORMAT {
    if n > B322_TX { return B322_FULL; }
    x[0].tx_n = tx_put(x[0].tx[..], 0, raw);
  } else if format == B322_POF {
    p := psbt_read_vartime(raw, x[0].fields[..], x[0].read[..]);
    if p.code != PSBT_OK || !psbt_maps_vartime(raw, p, x[0].fields[..], x[0].maps[..]) { return B322_FULL; }
    x[0].tx_n = psbt_extract_vartime(x[0].maps[..], x[0].tx[..], x[0].sign[..]);
    if x[0].tx_n < 0 { return B322_FULL; }
  } else {
    // to_sign with the witness: segwit's marker and flag after the
    // version, the witness before the lock time. An empty stack (the
    // single byte 0) has no witness to mark.
    unsigned: [61] u8;
    #assert bip322_to_sign(x[0].txid[..], 0, 0, 0, unsigned[..]) == 61;
    empty := n == 1 && raw[0] == 0;
    at := tx_put(x[0].tx[..], 0, unsigned[0..<4]);
    if !empty { x[0].tx[at] = 0; x[0].tx[at+1] = 1; at += 2; }
    at = tx_put(x[0].tx[..], at, unsigned[4..<57]);
    if n + at + 4 > B322_TX { return B322_SCRIPT; }
    if !empty { at = tx_put(x[0].tx[..], at, raw); }
    x[0].tx_n = tx_put(x[0].tx[..], at, unsigned[57..<61]);
  }
  x[0].t = tx_read_vartime(x[0].tx[0..<x[0].tx_n], true, x[0].ins[..], x[0].outs[..], x[0].items[..]);
  if x[0].t.code != TX_OK { return ifx format == B322_SIMPLE || format == B322_BARE then B322_SCRIPT else B322_FULL; }
  return B322_OK;
}
```

## The rules

BIP-322 runs the scripts under two sets of rules. The required rules
are consensus plus the policy rules that make a signature
non-malleable: low S, strict encodings, an empty failed signature,
minimal pushes, one element left on the stack, minimal `IF` arguments,
and no `OP_CODESEPARATOR`. If a script fails them, the signature is
invalid.

The upgradeable rules cover what a soft fork may give meaning later:
reserved NOPs, witness versions above 1, Taproot's unknown leaf
versions, `OP_SUCCESSx`, unknown key types and the annex. A signature
that relies on one of them may be valid under rules the verifier
doesn't know, so it is inconclusive.

<!-- source: coax/lib/bitcoin/bip322.coax flags -->
```jai
// The required rules: consensus, plus LOW_S, STRICTENC, NULLFAIL,
// MINIMALDATA, CLEANSTACK and MINIMALIF, and CONST_SCRIPTCODE, which
// refuses OP_CODESEPARATOR and FindAndDelete. A failure is invalid.
B322_REQUIRED :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_STRICTENC | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_LOW_S
  | SCRIPT_VERIFY_NULLDUMMY | SCRIPT_VERIFY_MINIMALDATA | SCRIPT_VERIFY_CLEANSTACK | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY
  | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_MINIMALIF | SCRIPT_VERIFY_NULLFAIL
  | SCRIPT_VERIFY_CONST_SCRIPTCODE;
// The upgradeable rules: NOPs reserved for soft forks and witness
// versions above 1. A failure is inconclusive.
B322_UPGRADEABLE :: SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_NOPS | SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM;
```

Chapter 36's `CONST_SCRIPTCODE` flag refuses `OP_CODESEPARATOR` only in
legacy script, as Core does. BIP-322 forbids it everywhere, so
`b322_codesep` scans a P2WSH witness script for it too.

<!-- source: coax/lib/bitcoin/bip322.coax input -->
```jai
// A P2WSH witnessScript, bare or under P2SH, with OP_CODESEPARATOR:
// BIP-322 forbids it, and CONST_SCRIPTCODE refuses it only in legacy
// script.
b322_codesep :: (sp: ScriptSpend) -> bool {
  input := sp.ins[sp.index];
  sig := sp.b[input.script..<input.script+input.script_n];
  nested := script_p2sh_vartime(sp.spk) && sig.count == 35 && sig[0] == 34 && sig[1] == 0 && sig[2] == 32;
  if !(ps_p2wsh(sp.spk) || nested) || input.count == 0 { return false; }
  item := sp.items[input.first + input.count - 1];
  s := sp.b[item.at..<item.at+item.n];
  pc := 0;
  while pc < s.count {
    o := script_op_vartime(s, pc);
    if !o.ok { return false; }
    if o.op == OP_CODESEPARATOR { return true; }
    pc = o.next;
  }
  return false;
}
// One input of to_sign against the output it spends: Taproot here, any
// other script through chapter 36's VerifyScript, once with the required
// rules (a failure is invalid), once with the upgradeable ones too (a
// failure is inconclusive).
b322_input :: (x: [] B322Work, n: int) -> int {
  spk := x[0].spks[n].b[0..<x[0].spks[n].n];
  sp := ScriptSpend.{b=x[0].tx[0..<x[0].tx_n], t=x[0].t, ins=x[0].ins[..], outs=x[0].outs[..], items=x[0].items[..],
    index=n, amount=x[0].amounts[n], spk=spk};
  if ps_p2tr(spk) { return b322_taproot(sp, x, spk[2..<34]); }
  if b322_codesep(sp) { return B322_INVALID; }
  if script_verify_vartime(sp, B322_REQUIRED, x[0].script[..]) != SCRIPT_ERR_OK { return B322_INVALID; }
  if script_verify_vartime(sp, B322_REQUIRED | B322_UPGRADEABLE, x[0].script[..]) != SCRIPT_ERR_OK { return B322_INCONCLUSIVE; }
  return B322_VALID;
}
```

## Taproot

Chapter 36's interpreter stops at Taproot: a version 1 program returns
`SCRIPT_TAPROOT_UNBUILT`. Chapters 44 and 45 will build tapscript and
script-path spends in full. Until then this chapter checks Taproot
inputs itself.

A key path is one signature of 64 bytes, or 65 ending in
`SIGHASH_ALL`. It is verified with BIP-340 under the output key, over
chapter 43's signature hash. A script path ends with the script and the
control block. Chapter 42's `taproot_commitment_vartime` checks that
the control block commits the script to the output key. Only then does
the leaf version matter: tapscript is 0xc0, and anything else is an
upgrade, so inconclusive.

<!-- source: coax/lib/bitcoin/bip322.coax taproot -->
```jai
// A P2TR input spending program: an empty scriptSig and a witness. An
// annex is an upgrade mechanism: inconclusive. One item is a key-path
// signature: 64 bytes, or 65 ending in SIGHASH_ALL, verified under the
// output key. More items are a script path: the control block must
// commit the script to the output key (chapter 42), a leaf version other
// than tapscript's 0xc0 is inconclusive, and the script runs on the rest.
b322_taproot :: (sp: ScriptSpend, x: [] B322Work, program: [] u8) -> int {
  input := sp.ins[sp.index];
  count := input.count;
  if input.script_n != 0 || count == 0 { return B322_INVALID; }
  w := x[0].script[..];
  last := si_bytes(sp, w, si_witness(sp, count - 1));
  if count >= 2 && last.count > 0 && last[0] == BT_ANNEX_TAG { return B322_INCONCLUSIVE; }
  amounts := x[0].amounts[0..<x[0].tp.nin]; spks := x[0].spks[0..<x[0].tp.nin];
  if count == 1 {
    sig := last;
    if sig.count != 64 && (sig.count != 65 || sig[64] != 1) { return B322_INVALID; }
    hash: [32] u8;
    #assert bt_sighash_taproot(x[0].strip[..], x[0].tp, sp.index, amounts, spks, sig.count - 64, hash[..]);
    return ifx schnorr_verify_vartime(hash[..], program, sig[0..<64]) then B322_VALID else B322_INVALID;
  }
  script := si_witness(sp, count - 2);
  if taproot_commitment_vartime(last, si_bytes(sp, w, script), program) != TR_OK { return B322_INVALID; }
  if (last[0] & 0xfe) != TR_LEAF_TAPSCRIPT { return B322_INCONCLUSIVE; }
  if count - 2 > SCRIPT_MAX_STACK { return B322_INVALID; }
  si_reset(w);
  x[0].budget = 50 + tx_compact_size_vartime(count);
  for k: 0..<count {
    item := si_witness(sp, k);
    x[0].budget += tx_compact_size_vartime(item.n) + item.n;
    if k < count - 2 {
      if item.n > SCRIPT_MAX_ELEMENT { return B322_INVALID; }
      si_push(w, item);
    }
  }
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, si_bytes(sp, w, script), x[0].leaf[..]) == TR_OK;
  return b322_tapscript(sp, x, script);
}
```

BIP-322's step 2 lets a verifier without a full interpreter answer
inconclusive for a script it doesn't understand. `b322_tapscript` uses
that. It first scans the script: any `OP_SUCCESSx` makes it
inconclusive, as BIP-342 says. Then it runs the opcodes that time-lock
and multisig scripts use, with tapscript's own rules: `MINIMALIF` is
consensus, `CHECKMULTISIG` is gone, and there is no limit of 201
opcodes. Everything except the signature opcodes runs in chapter 36's
`si_run`. Any other opcode is inconclusive.

<!-- source: coax/lib/bitcoin/bip322.coax tapscript -->
```jai
// A tapscript leaf on the stack the witness left: first a scan, in which
// any OP_SUCCESSx makes the script inconclusive; then the opcodes, with
// tapscript's rules: MINIMALIF as consensus, no CHECKMULTISIG, no op
// limit. OP_CODESEPARATOR is forbidden by BIP-322. An opcode outside
// B322_TAP_KNOWN is inconclusive; the others run in chapter 36's si_run.
// The script must leave one true element.
b322_tapscript :: (sp: ScriptSpend, x: [] B322Work, script: ScriptItem) -> int {
  w := x[0].script[..];
  s := si_bytes(sp, w, script);
  pc := 0;
  while pc < s.count {
    o := script_op_vartime(s, pc);
    if !o.ok { return B322_INVALID; }
    if b322_success(o.op) { return B322_INCONCLUSIVE; }
    pc = o.next;
  }
  flags := SCRIPT_VERIFY_MINIMALDATA | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY;
  cond := ScriptCond.{first_false=-1};
  pc = 0;
  while pc < s.count {
    running := cond.first_false < 0;
    o := script_op_vartime(s, pc);
    pc = o.next; op := o.op;
    if op <= OP_PUSHDATA4 {
      if running {
        if o.n > SCRIPT_MAX_ELEMENT || !script_minimal_push_vartime(s[o.at..<o.at+o.n], op) { return B322_INVALID; }
        si_push(w, ScriptItem.{src=script.src, at=script.at+o.at, n=o.n});
      }
    } else if op == OP_IF || op == OP_NOTIF {
      value := false;
      if running {
        if si_size(w) < 1 { return B322_INVALID; }
        top := si_bytes(sp, w, si_top(w, 1));
        if top.count > 1 || (top.count == 1 && top[0] != 1) { return B322_INVALID; }
        value = (top.count == 1) != (op == OP_NOTIF);
        si_pop(w, 1);
      }
      cond = si_cond_push(cond, value);
    } else if op == OP_ELSE || op == OP_ENDIF {
      if cond.size == 0 { return B322_INVALID; }
      cond = ifx op == OP_ELSE then si_cond_toggle(cond) else si_cond_pop(cond);
    } else if op == OP_CODESEPARATOR || op == OP_CHECKMULTISIG || op == OP_CHECKMULTISIGVERIFY {
      return B322_INVALID;
    } else if !b322_known(op) {
      return B322_INCONCLUSIVE;
    } else if running {
      if op == OP_CHECKSIG || op == OP_CHECKSIGVERIFY || op == B322_CHECKSIGADD {
        state := b322_tap_sig(sp, x, op);
        if state != B322_VALID { return state; }
      } else if si_run(sp, w, s, 0, op, flags, SCRIPT_V0) != SCRIPT_ERR_OK { return B322_INVALID; }
    }
    if si_size(w) > SCRIPT_MAX_STACK { return B322_INVALID; }
  }
  if cond.size != 0 || si_size(w) != 1 || !script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { return B322_INVALID; }
  return B322_VALID;
}
```

A tapscript signature opcode differs from the legacy one in three ways.
It verifies BIP-340 signatures over BIP-342's signature hash, which adds
the leaf hash. An empty signature is simply false, but a non-empty one
that fails ends the script. Each non-empty signature also spends 50 of
the validation weight, a budget of the witness's size plus 50.
`CHECKSIGADD` replaces `CHECKMULTISIG`: it adds 1 to a counter for each
good signature, and the script compares the counter with the threshold.

<!-- source: coax/lib/bitcoin/bip322.coax tapsig -->
```jai
// OP_SUCCESSx (BIP-342): opcodes a future soft fork may give any meaning.
b322_success :: (op: int) -> bool {
  return op == 80 || op == 98 || (op >= 126 && op <= 129) || (op >= 131 && op <= 134) || op == 137 || op == 138
    || op == 141 || op == 142 || (op >= 149 && op <= 153) || (op >= 187 && op <= 254);
}
// The opcodes the bounded evaluator knows besides pushes and the
// conditionals: OP_1NEGATE, OP_1 to OP_16, NOP, VERIFY, RETURN, DROP, DUP,
// SWAP, SIZE, EQUAL, EQUALVERIFY, NUMEQUAL, NUMEQUALVERIFY, SHA256,
// HASH160, CHECKSIG, CHECKSIGVERIFY, CHECKLOCKTIMEVERIFY,
// CHECKSEQUENCEVERIFY and CHECKSIGADD.
B322_CHECKSIGADD :: 0xba;
B322_TAP_KNOWN :: [19] int.[0x4f, 0x61, 0x69, 0x6a, 0x75, 0x76, 0x7c, 0x82, 0x87, 0x88, 0x9c, 0x9d, 0xa8, 0xa9, 0xac, 0xad, 0xb1, 0xb2, 0xba];
b322_known :: (op: int) -> bool {
  if op >= OP_1 && op <= OP_16 { return true; }
  for k: B322_TAP_KNOWN { if op == k { return true; } }
  return false;
}
// A tapscript signature opcode: CHECKSIG, CHECKSIGVERIFY or CHECKSIGADD.
// Each non-empty signature spends 50 of the validation weight. An empty
// key fails; a key of other than 32 bytes is an unknown key type, an
// upgradeable rule. An empty signature is false; a non-empty one must be
// a valid BIP-340 signature with SIGHASH_DEFAULT or SIGHASH_ALL, or the
// script fails.
b322_tap_sig :: (sp: ScriptSpend, x: [] B322Work, op: int) -> int {
  w := x[0].script[..];
  need := ifx op == B322_CHECKSIGADD then 3 else 2;
  if si_size(w) < need { return B322_INVALID; }
  key := si_bytes(sp, w, si_top(w, 1));
  sig := si_bytes(sp, w, si_top(w, need));
  n := ScriptNum.{ok=true};
  if op == B322_CHECKSIGADD { n = script_num_vartime(si_bytes(sp, w, si_top(w, 2)), true, 4); }
  if !n.ok { return B322_INVALID; }
  if sig.count > 0 { x[0].budget -= 50; if x[0].budget < 0 { return B322_INVALID; } }
  if key.count == 0 { return B322_INVALID; }
  if key.count != 32 { return B322_INCONCLUSIVE; }
  ok := sig.count > 0;
  if ok {
    if sig.count != 64 && sig.count != 65 { return B322_INVALID; }
    if sig.count == 65 && sig[64] != 1 { return B322_INVALID; }
    hash: [32] u8;
    amounts := x[0].amounts[0..<x[0].tp.nin]; spks := x[0].spks[0..<x[0].tp.nin];
    if !bt_sighash_taproot_ext(x[0].strip[..], x[0].tp, sp.index, amounts, spks, sig.count - 64, sig[0..<0], x[0].leaf[..], BT_NO_CODESEP, hash[..]) { return B322_INVALID; }
    if !schnorr_verify_vartime(hash[..], key, sig[0..<64]) { return B322_INVALID; }
  }
  si_pop(w, need);
  if op == B322_CHECKSIGADD { si_push(w, si_number(w, n.value + ifx ok then 1 else 0)); }
  else if op == OP_CHECKSIG { si_push(w, si_bool(ok)); }
  else if !ok { return B322_INVALID; }
  return B322_VALID;
}
```

## Verification

`bip322_verify_vartime` follows BIP-322's process. The address gives
the challenge and `to_spend`. An unprefixed signature of 88 characters
for a P2PKH address is the legacy format. Otherwise the prefix names
the format, and a signature without one is read as simple, as BIP-322
allows for signatures made before prefixes existed.

`to_sign` must spend `to_spend`'s output 0 with its first input, have
one input unless it is a proof of funds, and pay 0 to `OP_RETURN` in
its one output. Then every input runs. The first spends the challenge
with an amount of 0. The others spend the outputs their PSBT fields
give. Last, a version other than 0 or 2 is an upgradeable rule.

<!-- source: coax/lib/bitcoin/bip322.coax verify -->
```jai
// BIP-322's verification of signature for message under address: the
// legacy format for a P2PKH address and an unprefixed 88-character
// signature, else the prefix's format (none: simple). Then to_sign's
// shape: its first input spends to_spend's output, only a proof of funds
// has more inputs, and its one output pays 0 to OP_RETURN. Then every
// input's script, and the upgradeable version rule: 0 or 2.
bip322_verify_vartime :: (address: string, signature: string, message: string, x: [] B322Work) -> B322Result #export_c {
  r := B322Result.{state=B322_INVALID};
  if !b322_spend(address, message, x) { r.reason = B322_ADDRESS; return r; }
  prefix := ifx signature.count >= 3 then signature[0..<3] else "";
  r.format = ifx prefix == "smp" then B322_SIMPLE else ifx prefix == "ful" then B322_FULL_FORMAT
    else ifx prefix == "pof" then B322_POF else B322_BARE;
  body := ifx r.format == B322_BARE then signature else signature[3..<signature.count];
  if r.format == B322_BARE && ps_p2pkh(x[0].spk[0..<x[0].spk_n]) && body.count == SM_SIGNATURE_TEXT {
    r.format = B322_LEGACY;
    if verify_message_vartime(address, body, message, false) != SM_OK { r.reason = B322_LEGACY_SIG; return r; }
    r.state = B322_VALID;
    return r;
  }
  r.reason = b322_decode(r.format, body, x);
  if r.reason != B322_OK { return r; }
  t := x[0].t; b := x[0].tx[0..<x[0].tx_n];
  first := ifx t.nin > 0 then b[x[0].ins[0].outpoint..<x[0].ins[0].outpoint+36] else b[0..<0];
  spends := first.count == 36 && ps_equal(first[0..<32], x[0].txid[..]) && tx_le_vartime(first, 32, 4) == 0;
  pays := t.nout == 1 && tx_le_vartime(b, x[0].outs[0].value, 8) == 0 && x[0].outs[0].script_n == 1 && b[x[0].outs[0].script] == OP_RETURN;
  if !spends || !pays || (r.format != B322_POF && t.nin != 1) || t.nin > BT_IO { r.reason = B322_SHAPE; return r; }
  // Each input's spent output: to_spend's for the first.
  x[0].amounts[0] = 0; x[0].spks[0].n = x[0].spk_n;
  for k: 0..<x[0].spk_n { x[0].spks[0].b[k] = x[0].spk[k]; }
  for n: 1..<t.nin { if !b322_pof_spent(x, n) { r.reason = B322_SHAPE; return r; } }
  // The stripped transaction, for BIP-341's signature hash.
  strip_n := tx_write_vartime(b, t, x[0].ins[..], x[0].outs[..], x[0].items[..], false, x[0].strip[..]);
  x[0].tp = bt_tx_read(x[0].strip[0..<strip_n]);
  r.reason = B322_SCRIPT;
  inconclusive := false;
  for n: 0..<t.nin {
    state := b322_input(x, n);
    if state == B322_INVALID { return r; }
    if state == B322_INCONCLUSIVE { inconclusive = true; }
  }
  r.reason = B322_OK;
  version := tx_le_vartime(b, 0, 4);
  if inconclusive || (version != 0 && version != 2) { r.state = B322_INCONCLUSIVE; return r; }
  r.state = B322_VALID;
  r.locktime = tx_le_vartime(b, t.locktime, 4);
  r.sequence = tx_le_vartime(b, x[0].ins[0].sequence, 4);
  return r;
}
```

A proof of funds takes each extra input's spent output from its
witness UTXO or non-witness UTXO field. BIP-322 lets a PSBT give the
whole previous transaction only once for several inputs that spend
it, so `b322_pof_spent` falls back to an earlier input's field.
Chapter 94's reader has already checked that each non-witness UTXO
hashes to its input's txid. A proof of funds shows control of the
coins, not that they are unspent. Only a node with the UTXO set can
answer that.

## Signing

A signer builds `to_sign` as a PSBT and lets chapter 94's roles do the
rest. BIP-322 defines one new global field,
`PSBT_GLOBAL_GENERIC_SIGNED_MESSAGE` (type 0x09), so that a hardware
signer can show "sign message M for address A" instead of "spend 0
satoshis". The input's UTXO is `to_spend`: the whole transaction for a
legacy script, its output for a segwit one. The redeem script and
witness script go in their fields.

<!-- source: coax/lib/bitcoin/bip322.coax sign -->
```jai
// A signature of message for address by keys (32 bytes each, in the
// script's order), into out as text: legacy, simple (smp) or full (ful).
// script is the address's script when it has one: a P2WSH witnessScript,
// a P2SH redeemScript, the witnessScript of a P2SH-P2WSH, a P2TR leaf
// (else the key path). to_sign is built as a BIP-322 PSBT, with
// PSBT_GLOBAL_GENERIC_SIGNED_MESSAGE (0x09) set to the message and
// to_spend as the input's UTXO, signed without low-R grinding, finalized
// and extracted. Returns the text's length, or -1.
bip322_sign_vartime :: (address: string, message: string, keys: [] u8, script: [] u8, format: int, version: int, locktime: int, sequence: int, aux: [] u8, x: [] B322Work, out: [] u8) -> int #export_c {
  if !b322_spend(address, message, x) || keys.count == 0 || keys.count % 32 != 0 || message.count > B322_MESSAGE { return -1; }
  spk := x[0].spk[0..<x[0].spk_n];
  if format == B322_LEGACY {
    if !ps_p2pkh(spk) || keys.count != 32 || out.count < SM_SIGNATURE_TEXT { return -1; }
    return signed_message_sign(keys, true, message, x[0].sign[0].ecdsa[..], out);
  }
  if format != B322_SIMPLE && format != B322_FULL_FORMAT { return -1; }
  unsigned: [61] u8;
  #assert bip322_to_sign(x[0].txid[..], version, locktime, sequence, unsigned[..]) == 61;
  m := x[0].maps[..];
  #assert psbt_create_vartime(unsigned[..], m);
  for k: 0..<message.count { x[0].message[k] = message[k]; }
  #assert psbt_set1_vartime(m, PSBT_GLOBAL, 0, 0x09, x[0].message[0..<message.count]);
  // The scripts the input needs.
  redeem: [34] u8; hash: [32] u8; h: [20] u8; legacy := ps_p2pkh(spk);
  if script_p2sh_vartime(spk) {
    #assert hash160_checked(script, h[..], false) == RMD_OK;
    if ps_equal(h[..], spk[2..<22]) {
      #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x04, script);
      legacy = !ps_p2wpkh(script);
    } else {
      redeem[0] = 0; redeem[1] = 32;
      #assert sha256_hash_checked(script, redeem[2..<34], false) == SHA_OK;
      #assert hash160_checked(redeem[..], h[..], false) == RMD_OK;
      if !ps_equal(h[..], spk[2..<22]) { return -1; }
      #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x04, redeem[..]) && psbt_set1_vartime(m, PSBT_INPUT, 0, 0x05, script);
    }
  } else if ps_p2wsh(spk) { #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x05, script); }
  // to_spend as the input's UTXO: whole for a legacy script, else its output.
  utxo: [64] u8;
  if legacy { #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x00, x[0].spend[0..<x[0].spend_n]); }
  else {
    for k: 0..<8 { utxo[k] = 0; }
    utxo[8] = cast(u8) spk.count;
    #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x01, utxo[0..<tx_put(utxo[..], 9, spk)]);
  }
  if ps_p2tr(spk) && script.count > 0 {
    if keys.count != 32 || !b322_sign_leaf(x, unsigned[..], keys, script, aux) { return -1; }
  } else {
    for k: 0..<keys.count / 32 { if psbt_sign_vartime(m, 0, keys[32*k..<32*k+32], aux, false, x[0].sign[..]) != PSBT_SIGN_OK { return -1; } }
    if !psbt_finalize_input_vartime(m, 0, x[0].sign[..]) && !b322_else_branch(m) { return -1; }
  }
  // The encoding: input 0's witness, or the whole of to_sign.
  data := x[0].tx[0..<0];
  if format == B322_SIMPLE {
    j := psbt_find1_vartime(m, PSBT_INPUT, 0, 0x08);
    if j < 0 || version != 0 || locktime != 0 || sequence != 0 { return -1; }
    data = pm_value(m, j);
  } else {
    x[0].tx_n = psbt_extract_vartime(m, x[0].tx[..], x[0].sign[..]);
    data = x[0].tx[0..<x[0].tx_n];
  }
  if out.count < 3 + base64_encode_size(data.count) { return -1; }
  name := B322_FORMATS[format];
  for k: 0..<3 { out[k] = name[k]; }
  return 3 + base64_encode_checked(cast(string) data, out[3..<out.count], false).written;
}
```

The signer signs with RFC 6979 nonces and no low-R grinding. That is
how btcd made BIP-322's published vectors, so the signer reproduces
every published ECDSA signature that wasn't ground to a low R, byte for
byte.

BIP-174's finalizer knows standard templates only. A time lock's ELSE
branch, `<signature> <empty> <script>`, needs one more case.

<!-- source: coax/lib/bitcoin/bip322.coax else -->
```jai
// A script the Input Finalizer has no template for: the time lock's ELSE
// branch, <signature> <empty> <witnessScript>, from input 0's one partial
// signature.
b322_else_branch :: (m: [] PsbtMaps) -> bool {
  ws := psbt_find1_vartime(m, PSBT_INPUT, 0, 0x05);
  sig := -1;
  for j: 0..<m[0].n { if m[0].pairs[j].map == PSBT_INPUT && m[0].pairs[j].index == 0 && pm_type(m, j) == 0x02 { sig = j; } }
  if ws < 0 || sig < 0 { return false; }
  wit: [PSBT_TX] u8;
  at := tx_compact_write_vartime(wit[..], 0, 3);
  at = tx_compact_write_vartime(wit[..], at, pm_value(m, sig).count); at = tx_put(wit[..], at, pm_value(m, sig));
  wit[at] = 0; at += 1;
  at = tx_compact_write_vartime(wit[..], at, pm_value(m, ws).count); at = tx_put(wit[..], at, pm_value(m, ws));
  return psbt_set1_vartime(m, PSBT_INPUT, 0, 0x08, wit[0..<at]);
}
```

Chapter 94's Signer signs Taproot key paths only. A time-lock leaf is a
script path, so `b322_sign_leaf` signs BIP-342's signature hash with
the untweaked key. It also writes the control block of a tree with one
leaf and checks that the result is the address's output key.

<!-- source: coax/lib/bitcoin/bip322.coax tapleaf -->
```jai
// A P2TR script-path signature by key for the leaf script, whose time
// lock's ELSE branch it takes: BIP-342's signature hash, a BIP-340
// signature by the untweaked key, and the control block of a tree with
// the one leaf. The witness <signature> <empty> <script> <control> goes
// into input 0 as its final witness.
b322_sign_leaf :: (x: [] B322Work, unsigned: [] u8, key: [] u8, script: [] u8, aux: [] u8) -> bool {
  amounts: [1] int; spks: [1] BtScript;
  spks[0].n = x[0].spk_n;
  for k: 0..<x[0].spk_n { spks[0].b[k] = x[0].spk[k]; }
  leaf: [32] u8; hash: [32] u8; pub: [33] u8; q: [32] u8; sig: [64] u8;
  if taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, script, leaf[..]) != TR_OK { return false; }
  #assert bt_sighash_taproot_ext(unsigned, bt_tx_read(unsigned), 0, amounts[..], spks[..], BT_DEFAULT, unsigned[0..<0], leaf[..], BT_NO_CODESEP, hash[..]);
  if !bip32_public(key, pub[..]) { return false; }
  parity := taproot_output_key_vartime(pub[1..<33], leaf[..], q[..]);
  if parity < 0 || !ps_equal(q[..], x[0].spk[2..<34]) { return false; }
  w := x[0].sign[0].tap[0].sign[..];
  status := 0;
  for k: 0..<6 { status = schnorr_sign_step(key, aux, hash[..], w, sig[..], false); }
  if status != SIGN_DONE { return false; }
  wit: [256 + SCRIPT_MAX_SIZE] u8;
  if script.count > SCRIPT_MAX_SIZE { return false; }
  at := tx_compact_write_vartime(wit[..], 0, 4);
  at = tx_compact_write_vartime(wit[..], at, 64); at = tx_put(wit[..], at, sig[..]);
  wit[at] = 0; at += 1;
  at = tx_compact_write_vartime(wit[..], at, script.count); at = tx_put(wit[..], at, script);
  wit[at] = 33; wit[at+1] = cast(u8) (TR_LEAF_TAPSCRIPT | parity); at += 2;
  at = tx_put(wit[..], at, pub[1..<33]);
  return psbt_set1_vartime(x[0].maps[..], PSBT_INPUT, 0, 0x08, wit[0..<at]);
}
```

## The example

[bitcoin/examples/bip322.coax](../examples/bip322.coax) hashes "Hello
World" and builds both transactions for the
address of BIP-322's basic vectors. It then signs the message in each
format from the published keys, verifies each signature, verifies a
published proof of funds, and runs every vector.

<!-- source: bitcoin/examples/bip322.coax example -->
```jai
#load "../../coax/lib/bitcoin/bip322.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [64] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A txid as it is displayed: byte-reversed.
put_txid :: (id: [] u8) {
  r: [32] u8;
  for k: 0..<32 { r[k] = id[31-k]; }
  put_hex(r[..]);
}
// A verification's answer in BIP-322's words.
put_verdict :: (r: B322Result) {
  if r.state == B322_VALID { print("  valid at time % and age % (%)\n", r.locktime, r.sequence, B322_FORMATS[r.format]); }
  else if r.state == B322_INVALID { print("  invalid: % (%)\n", B322_REASONS[r.reason], B322_FORMATS[r.format]); }
  else { print("  inconclusive (%)\n", B322_FORMATS[r.format]); }
}
show :: (title: string, address: string, signature: string, message: string, x: [] B322Work) {
  print("%: %... (% characters)\n", title, signature[0..<44], signature.count);
  put_verdict(bip322_verify_vartime(address, signature, message, x));
}
// The keys of a vector: 52-character WIFs, 32 bytes each.
keys_of :: (c: B322Case, out: [] u8) -> [] u8 {
  work: [WIF_WORK_BYTES] u8;
  n := 0;
  for k: 0..<c.keys.count / 52 {
    if wif_decode_ct(c.keys[52*k..<52*k+52], work[..], out[32*k..<32*k+32]) == B58_MAIN_WIF_COMPRESSED { n += 32; }
  }
  return out[0..<n];
}
// Signs and shows the verdict; returns the signature.
sign_and_show :: (title: string, c: B322Case, script: [] u8, format: int, version: int, locktime: int, sequence: int, x: [] B322Work, out: [] u8) -> string {
  keys: [160] u8; aux: [32] u8;
  n := bip322_sign_vartime(c.address, "Hello World", keys_of(c, keys[..]), script, format, version, locktime, sequence, aux[..], x, out);
  s := cast(string) out[0..<n];
  show(title, c.address, s, "Hello World", x);
  return s;
}

main :: () {
  x: [1] B322Work;
  // The message hash and the two virtual transactions.
  h := B322_HASHES[1];
  hash: [32] u8; unsigned: [61] u8; id: [32] u8;
  if !bip322_message_hash(h.message, hash[..]) || !b322_spend(h.address, h.message, x[..]) { return; }
  if bip322_to_sign(x[0].txid[..], 0, 0, 0, unsigned[..]) != 61 || sha256d_checked(unsigned[..], id[..], false) != SHA_OK { return; }
  print("\"%\" for %:\n", h.message, h.address);
  print("  message_hash "); put_hex(hash[..]); print("\n");
  print("  to_spend "); put_txid(x[0].txid[..]); print("\n");
  print("  to_sign  "); put_txid(id[..]); print(" (unsigned)\n");

  // A signature in each format, verified.
  out: [4096] u8; script: [600] u8; none := out[0..<0];
  w := B322_CASES[3]; tr := B322_CASES[5]; leaf := B322_CASES[13]; pkh := B322_CASES[10]; pof := B322_CASES[22];
  s := sign_and_show("P2WPKH, simple", w, none, B322_SIMPLE, 0, 0, 0, x[..], out[..]);
  print("  the published signature: %\n", ifx s == w.signature then "the same" else "another");
  show("P2WPKH, the same signature under \"Hello World!\"", w.address, s, "Hello World!", x[..]);
  s = sign_and_show("P2WPKH, full", w, none, B322_FULL_FORMAT, 0, 0, 0, x[..], out[..]);
  s = sign_and_show("P2TR key path, simple", tr, none, B322_SIMPLE, 0, 0, 0, x[..], out[..]);
  s = sign_and_show("P2TR time-lock leaf, full, version 2", leaf, tx_unhex(leaf.script, script[..]), B322_FULL_FORMAT, 2, 2016, 2016, x[..], out[..]);
  s = sign_and_show("P2PKH, legacy", pkh, none, B322_LEGACY, 0, 0, 0, x[..], out[..]);
  s = sign_and_show("P2PKH, full", pkh, none, B322_FULL_FORMAT, 0, 0, 0, x[..], out[..]);
  show("P2TR proof of funds, published", pof.address, pof.signature, pof.message, x[..]);

  // Every vector.
  print("The BIP-322 vectors:\n");
  pass := 0;
  for v: B322_HASHES {
    ok := bip322_message_hash(v.message, hash[..]) && tx_hex_is(hash[..], v.message_hash) && b322_spend(v.address, v.message, x[..]);
    ok = ok && bip322_to_sign(x[0].txid[..], 0, 0, 0, unsigned[..]) == 61 && sha256d_checked(unsigned[..], id[..], false) == SHA_OK;
    if ok && b322_displayed(x[0].txid[..], v.to_spend) && b322_displayed(id[..], v.to_sign) { pass += 1; }
  }
  print("  hashes: % of % match\n", pass, B322_HASHES.count);
  pass = 0; signed := 0; signs := 0; same := 0;
  for c: B322_CASES {
    if b322_case_passes(c, x[..]) { pass += 1; }
    if c.keys.count == 0 { continue; }
    signed += 1;
    if b322_case_signs(c, x[..]) { signs += 1; }
    if c.ours == c.signature || (c.ours[3..<c.ours.count] == c.signature && c.signature[0..<3] != "smp") { same += 1; }
  }
  print("  signatures: % of % verify as published\n", pass, B322_CASES.count);
  print("  the signer: % of % as the reference signs; % are the published ones byte for byte,\n", signs, signed, same);
  print("    % were published with other nonces (BIP-340, or ECDSA ground to a low R)\n", signed - same);
  pass = 0;
  for e: B322_ERRORS {
    r := bip322_verify_vartime(e.address, e.signature, e.message, x[..]);
    if r.state == B322_INVALID && r.reason == e.reason && r.format == e.format { pass += 1; }
  }
  print("  errors: % of % invalid for their reason\n", pass, B322_ERRORS.count);
  pass = 0; states: [3] int;
  for c: B322_BUILT {
    if b322_case_passes(c, x[..]) { pass += 1; }
    states[c.state] += 1;
  }
  print("  constructed: % of % as decided: % valid, % invalid, % inconclusive\n", pass, B322_BUILT.count, states[0], states[1], states[2]);
}
```

```shell
coax/bin/coax run bitcoin/examples/bip322.coax
```

It prints:

```text
"Hello World" for bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l:
  message_hash f0eb03b1a75ac6d9847f55c624a99169b5dccba2a31f5b23bea77ba270de0a7a
  to_spend b79d196740ad5217771c1098fc4a4b51e0535c32236c71f1ea4d61a2d603352b
  to_sign  88737ae86f2077145f93cc4b153ae9a1cb8d56afa511988c149c5c8c9d93bddf (unsigned)
P2WPKH, simple: smpAkgwRQIhAOzyynlqt93lOKJr+wmmxIens//zPzl9t... (147 characters)
  valid at time 0 and age 0 (smp)
  the published signature: the same
P2WPKH, the same signature under "Hello World!": smpAkgwRQIhAOzyynlqt93lOKJr+wmmxIens//zPzl9t... (147 characters)
  invalid: script failed (smp)
P2WPKH, full: fulAAAAAAABASs1A9aiYU3q8XFsIzJcU+BRS0r8mBAcd... (231 characters)
  valid at time 0 and age 0 (ful)
P2TR key path, simple: smpAUCqgciSEySmMUQ3lpV3buLiwvaFvzR2+zLNgPrVC... (91 characters)
  valid at time 0 and age 0 (smp)
P2TR time-lock leaf, full, version 2: fulAgAAAAABATnWNhKALOvtONDvYpy3Lk+Nj+lOFuy0u... (323 characters)
  valid at time 2016 and age 2016 (ful)
P2PKH, legacy: IH5DRv5UMcdOv0FGoUUtIzhPkCqihzuVpewHHX2D0Umr... (88 characters)
  valid at time 0 and age 0 (legacy)
P2PKH, full: fulAAAAAAE9CIMeQTy+rGFfT0wNSCOQgoNCWTIJkfdk5... (227 characters)
  valid at time 0 and age 0 (ful)
P2TR proof of funds, published: pofcHNidP8BALgCAAAABDzMFysa2DX0k4ZymoVfzNzTI... (883 characters)
  valid at time 123 and age 456 (pof)
The BIP-322 vectors:
  hashes: 3 of 3 match
  signatures: 23 of 23 verify as published
  the signer: 20 of 20 as the reference signs; 14 are the published ones byte for byte,
    6 were published with other nonces (BIP-340, or ECDSA ground to a low R)
  errors: 36 of 36 invalid for their reason
  constructed: 22 of 22 as decided: 5 valid, 9 invalid, 8 inconclusive
```

The message hash and both txids are the ones BIP-322 publishes. The
P2WPKH signature is the published one, byte for byte. Under "Hello
World!" the same signature fails, because the message changes
`to_spend`'s txid and so the hash that was signed.

The time-lock leaf signature is valid at time 2016 and age 2016. Its
script takes the ELSE branch, `2016 OP_CHECKSEQUENCEVERIFY`, which
needs version 2 and a sequence of at least 2016. The full format
carries both.

Of the 20 published signatures the signer remakes, 14 match byte for
byte. The other 6 used other nonces: btcd's BIP-340 nonces for the four
Taproot ones, and Core's low-R grinding for two of the P2WPKH ones.
Each of the signer's own signatures verifies.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/bip322.coax](../examples/checks/bip322.coax),
which loads the example. Keys from seeds sign three messages for their
own P2WPKH and P2TR addresses, in the simple and full formats. Each
signature verifies. It fails under another message, and it fails with
one Base64 character changed.

<!-- source: bitcoin/examples/checks/bip322.coax check -->
```jai
// A key from a seed, and the P2WPKH or P2TR address it owns.
seeded :: (s: int, taproot: bool, key: [] u8, address: [] u8) -> string {
  seed: [2] u8; seed[0] = cast(u8) (s & 255); seed[1] = cast(u8) (s >> 8);
  #assert sha256_hash_checked(seed[..], key, false) == SHA_OK;
  pub: [33] u8; spk: [34] u8;
  #assert bip32_public(key, pub[..]);
  n := ifx taproot then p2tr_script_vartime(pub[1..<33], spk[..]) else p2wpkh_script_vartime(pub[..], spk[..]);
  return cast(string) address[0..<address_encode_vartime(SW_MAINNET, spk[0..<n], address)];
}
// Any key signs any message for its own address, in either format; the
// signature verifies, and it doesn't verify under another message or
// with one character of its Base64 changed.
signs_alone :: (s: int, taproot: bool) -> bool {
  x: [1] B322Work; key: [32] u8; address: [90] u8; out: [1024] u8; aux: [32] u8;
  format := ifx s % 2 == 0 then B322_SIMPLE else B322_FULL_FORMAT;
  a := seeded(s, taproot, key[..], address[..]);
  message := ifx s % 3 == 0 then "" else ifx s % 3 == 1 then "Hello World" else "Chapter 96: generic signed messages";
  n := bip322_sign_vartime(a, message, key[..], key[0..<0], format, 0, 0, 0, aux[..], x[..], out[..]);
  if n <= 6 { return false; }
  r := bip322_verify_vartime(a, cast(string) out[0..<n], message, x[..]);
  ok := r.state == B322_VALID && r.format == format && r.locktime == 0 && r.sequence == 0;
  ok = ok && bip322_verify_vartime(a, cast(string) out[0..<n], "another message", x[..]).state == B322_INVALID;
  at := 3 + (s * 7) % (n - 6);
  out[at] = ifx out[at] == 'A' then 'B' else 'A';
  return ok && bip322_verify_vartime(a, cast(string) out[0..<n], message, x[..]).state == B322_INVALID;
}
// One check per address type: each check has its own traversal budget.
#forall "a P2WPKH signature verifies for its message, and for nothing else" s: 0..<3 #steps 20000000000 {
  #assert signs_alone(s, false);
}
#forall "a P2TR signature verifies for its message, and for nothing else" s: 3..<6 #steps 20000000000 {
  #assert signs_alone(s, true);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/bip322.coax
```

`coax test` runs 500 checks, including the loaded files' own checks, on 2026-11-03.

### The library's own checks

`bip322.coax` is an `#authority` file. Its checks match all three
published message hashes and both txids of each. All 23 published
signatures verify, simple, full and proof of funds, each valid at its
published time and age. The signer remakes the reference signer's
signature from the published keys for all 20 that aren't proofs of
funds. All 36 published errors are invalid for the reason their text
names. Twenty-two constructed cases reach the paths the vectors don't:

- the upgradeable rules: `OP_NOP4`, version 1, witness version 2, a
  leaf version of 0xc2, `OP_SUCCESS80`, a 33-byte tapscript key and an
  annex, each inconclusive;
- `OP_CODESEPARATOR` in P2WSH and in tapscript, `CHECKMULTISIG` in
  tapscript, an empty key, a non-minimal `IF` argument, a wrong control
  block and two elements left on the stack, each invalid;
- a 2-of-2 `CHECKSIGADD` leaf with both signatures, one of them with
  `SIGHASH_ALL`, valid, and with one signature, invalid;
- the legacy format, valid, and under a wrong message, invalid.

The checks also round-trip a legacy signature and cover the signer's
refusals.

```shell
coax/bin/coax test coax/lib/bitcoin/bip322.coax
```

`coax test` runs 496 checks, including those of the files it loads, on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[bip322_reference.py](../../coax/tests/bitcoin/bip322_reference.py) is
BIP-322's signer and verifier in Python, written from the specification.
It runs scripts through chapter 36's `script_reference.py` and has its
own Taproot checks and tapscript evaluator.
[make_bip322_vectors.py](../../coax/tests/bitcoin/make_bip322_vectors.py)
decides every published vector with it, checks each verdict against the
published one, signs from the published keys, builds the constructed
cases, and writes `bip322_vectors.coax`.
[bip322.py](../../coax/tests/bitcoin/bip322.py) writes the transcript
the example must print.

```shell
python3 -I coax/tests/bitcoin/make_bip322_vectors.py
python3 -I coax/tests/bitcoin/bip322.py --coax coax/bin/coax
```

On 2026-11-03 every vector agrees, all 28 lines of the transcript agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Signing a proof of funds. The chapter verifies the published ones;
  signing one is chapter 94's roles with more inputs and their UTXOs.
- A full tapscript interpreter. Chapters 44 and 45 build it; until
  then, an opcode outside the bounded set is inconclusive.
- Checking a proof of funds against the UTXO set, which needs a node.
- Testnet and regtest addresses; the code reads mainnet addresses.

## Summary

- BIP-322 turns a signed message into a spend of a virtual output.
  `to_spend` commits to the message and pays the address; `to_sign`
  spends it.
- A simple signature is a witness stack, a full one is `to_sign`, and a
  proof of funds is a finalized PSBT with more inputs.
- The required rules decide invalid; the upgradeable rules decide
  inconclusive. A valid signature is valid at time T and age S.
- Signing is ordinary PSBT signing, with the message in a global field
  so that a signer can say what it signs.

## Dependencies

- [22. Public key recovery and signed messages](ch022-public-key-recovery-and-signed-messages.md)
- [36. The script interpreter](ch036-the-script-interpreter.md)
- [42. Script trees, tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md)
- [43. Taproot signature hashes](ch043-taproot-signature-hashes.md)
- [94. PSBT](ch094-psbt.md)

## Sources

- BIP-322, Generic Signed Message Format, version 2.1.0, with its
  `basic-test-vectors.json` and `generated-test-vectors.json`, pinned in
  [coax/tests/vectors/upstream/bip322/](../../coax/tests/vectors/upstream/bip322/bip-0322.mediawiki).
- BIP-340, BIP-341 and BIP-342, pinned for chapters 23, 41 and 43.
- BIP-174, pinned for chapter 94.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
