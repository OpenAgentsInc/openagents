# 44. Tapscript

A Taproot output can be spent two ways. The key path is one signature by
the output key, which chapter 41 built. The script path reveals one leaf
of the output's script tree and runs it. BIP-342 defines the language
that leaf is written in, *tapscript*: Bitcoin script with Schnorr
signatures, a new opcode for counting them, and room for future soft
forks.

This chapter adds both paths to [chapter 36](ch036-the-script-interpreter.md)'s
interpreter, so `VerifyScript` now runs a version 1 witness program under
the `TAPROOT` flag in place of refusing it. The seven Taproot rows of
Bitcoin Core's `script_tests.json`, which chapter 36 could not run, give
Core's results, and 83 more Taproot spends give the results of a Python
transliteration of Core.

## What this chapter teaches

- How `VerifyScript` tells a key-path spend from a script-path spend, and
  where the annex sits in the witness.
- How the script path checks the leaf script against the output key, and
  what an unknown leaf version does.
- What tapscript changes in the interpreter:
  - `OP_CHECKSIG` and `OP_CHECKSIGVERIFY` check BIP-340 signatures against
    32-byte x-only keys.
  - `OP_CHECKSIGADD` counts signatures, and `OP_CHECKMULTISIG` fails.
  - An empty signature is a false check, not a failure.
  - A key of any length other than 0 or 32 is an unknown key type, which
    passes.
  - `OP_SUCCESSx`: 87 opcodes that make a script valid wherever they
    appear.
  - MINIMALIF is consensus.
  - The 10,000-byte script limit and the 201-opcode limit are gone. The
    validation-weight budget takes their place.
- How `OP_CODESEPARATOR` changes what a tapscript signature signs.

## Where Bitcoin uses tapscript

| Where | What it does | Chapter |
| --- | --- | --- |
| Block validation | runs every Taproot input, under the `TAPROOT` flag from its activation height | 55, 58 |
| Mempool policy | adds the three DISCOURAGE flags, which refuse Taproot's upgrade hooks | 81 |
| A wallet spending by a script | builds the control block and the witness this code checks | 45 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `si_taproot` | BIP-341's checks: annex, key path, control block, leaf version, budget |
| | `si_witness_size` | the witness's serialized size, the start of the budget |
| | `si_execute_witness` | tapscript's pre-scan for `OP_SUCCESSx` and its stack limit |
| | `si_eval` | chapter 36's EvalScript, with tapscript's changes |
| | `si_checksig_tapscript` | `OP_CHECKSIG`, `OP_CHECKSIGVERIFY` and `OP_CHECKSIGADD` |
| | `si_schnorr` | a 64- or 65-byte signature against a 32-byte key |
| | `si_tap_hashes`, `si_sighash_schnorr` | chapter 43's signature hash for this input, cached per spend |
| | `script_op_success_vartime` | the `OP_SUCCESSx` opcodes |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_sigmsg_taproot_parts` | SigMsg from its parts, so the interpreter can reuse its hashes |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_commitment_vartime`, `taproot_leaf_hash_vartime` | chapter 42's commitment check and leaf hash |

The transaction and its witness are public, so none of this needs
constant time.

## A Taproot output

A Taproot scriptPubKey is `OP_1` followed by a 32-byte x-only key, the
output key. With the `TAPROOT` flag, chapter 36's `si_witness_program`
sends such a program, outside P2SH, to `si_taproot`.

<!-- source: coax/lib/bitcoin/script.coax taproot -->
```jai
// VerifyWitnessProgram for a Taproot output (BIP-341), with the TAPROOT
// flag set: a last witness item starting 0x50, with at least one before
// it, is the annex, which the signature hash covers and nothing else
// reads. One item left is a key-path signature by the output key. More
// is a script path: the last item is the control block, the one before it
// the leaf script, and chapter 42's check must find the script committed
// to the output key. Leaf version 0xc0 runs the script as tapscript on
// the items before it, with a budget of 50 plus the witness's serialized
// size; any other leaf version passes unrun, for future soft forks.
si_taproot :: (sp: ScriptSpend, w: [] ScriptWork, program: [] u8, flags: int) -> int {
  count := sp.ins[sp.index].count;
  if count == 0 { return SCRIPT_ERR_WITNESS_PROGRAM_WITNESS_EMPTY; }
  last := si_witness(sp, count - 1);
  if count >= 2 && last.n > 0 && sp.b[last.at] == BT_ANNEX_TAG { w[0].annex = last; count -= 1; }
  if count == 1 { return si_schnorr(sp, w, si_bytes(sp, w, si_witness(sp, 0)), program, SCRIPT_TAPROOT); }
  control := si_bytes(sp, w, si_witness(sp, count - 1));
  script := si_witness(sp, count - 2);
  code := si_bytes(sp, w, script);
  check := taproot_commitment_vartime(control, code, program);
  if check == TR_BAD_CONTROL_SIZE { return SCRIPT_ERR_TAPROOT_WRONG_CONTROL_SIZE; }
  if check != TR_OK { return SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH; }
  if (control[0] & 0xfe) != TR_LEAF_TAPSCRIPT {
    return ifx (flags & SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_TAPROOT_VERSION) != 0 then SCRIPT_ERR_DISCOURAGE_UPGRADABLE_TAPROOT_VERSION else SCRIPT_ERR_OK;
  }
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, code, w[0].leaf[..]) == TR_OK;
  w[0].weight = si_witness_size(sp) + SCRIPT_WEIGHT_OFFSET;
  return si_execute_witness(sp, w, count - 2, script, flags, SCRIPT_TAPSCRIPT);
}
// The witness's serialized size: its item count and each item with its
// length, all as compact sizes.
si_witness_size :: (sp: ScriptSpend) -> int {
  c: [9] u8;
  count := sp.ins[sp.index].count;
  size := tx_compact_write_vartime(c[..], 0, count);
  for k: 0..<count { n := si_witness(sp, k).n; size += tx_compact_write_vartime(c[..], 0, n) + n; }
  return size;
}
```

The witness is read from its end:

- With at least two items, a last item that starts with `0x50` is the
  annex. Nothing reads it, but chapter 43's signature hash covers it, so
  a signer commits to it.
- One item left is the key path: a Schnorr signature by the output key
  itself, checked against the 32 bytes of the program.
- More items are the script path. The last is the control block, the one
  before it the leaf script, and the rest are the script's inputs.
  `taproot_commitment_vartime` from chapter 42 checks the control block's
  size, rebuilds the root from the leaf and the path, and checks that
  tweaking the internal key by that root gives the output key with the
  control block's parity bit. A bad size is `TAPROOT_WRONG_CONTROL_SIZE`;
  any other mismatch is `WITNESS_PROGRAM_MISMATCH`.
- Leaf version `0xc0` is tapscript. Any other version passes without
  running, so a soft fork can define it. Under
  `DISCOURAGE_UPGRADABLE_TAPROOT_VERSION` mempool policy refuses it.

Before it runs, the leaf hash goes into the workspace for the signature
hash, and the budget is set to the witness's serialized size plus 50.

## Running tapscript

`si_execute_witness`, chapter 36's ExecuteWitnessScript, first reads the
whole tapscript. A push that runs past the end is `BAD_OPCODE`. An
`OP_SUCCESSx` anywhere, even after an `OP_RETURN` or in a branch that
never runs, makes the spend valid at once, before any element limit. The
stack may then start with at most 1,000 elements, each at most 520 bytes.

<!-- source: coax/lib/bitcoin/script.coax witness -->
```jai
// ExecuteWitnessScript: the witness items (count of them) become the
// stack, none may exceed 520 bytes, the script runs, and it must leave
// exactly one element, which must be true. Tapscript first reads the
// whole script: an OP_SUCCESSx anywhere makes the spend valid at once
// (chapter 44), and a push that runs past the end fails. Its stack may
// start with at most 1,000 elements. A witness v0 stack larger than
// SCRIPT_ROOM keeps its top SCRIPT_ROOM elements and counts the rest.
si_execute_witness :: (sp: ScriptSpend, w: [] ScriptWork, count: int, script: ScriptItem, flags: int, version: int) -> int {
  if version == SCRIPT_TAPSCRIPT {
    s := si_bytes(sp, w, script); pc := 0;
    while pc < s.count {
      o := script_op_vartime(s, pc);
      if !o.ok { return SCRIPT_ERR_BAD_OPCODE; }
      if script_op_success_vartime(o.op) {
        return ifx (flags & SCRIPT_VERIFY_DISCOURAGE_OP_SUCCESS) != 0 then SCRIPT_ERR_DISCOURAGE_OP_SUCCESS else SCRIPT_ERR_OK;
      }
      pc = o.next;
    }
    if count > SCRIPT_MAX_STACK { return SCRIPT_ERR_STACK_SIZE; }
  }
  for k: 0..<count { if si_witness(sp, k).n > SCRIPT_MAX_ELEMENT { return SCRIPT_ERR_PUSH_SIZE; } }
  w[0].hidden = ifx count > SCRIPT_ROOM then count - SCRIPT_ROOM else 0;
  w[0].depth = 0;
  for k: w[0].hidden..<count { si_push(w, si_witness(sp, k)); }
  error := si_eval(sp, w, script, flags, version);
  if error != SCRIPT_ERR_OK { return error; }
  if si_size(w) != 1 { return SCRIPT_ERR_CLEANSTACK; }
  if !script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { return SCRIPT_ERR_EVAL_FALSE; }
  return SCRIPT_ERR_OK;
}
// VerifyWitnessProgram for a program at program (its bytes in prog):
// version 0 with 32 bytes is P2WSH, whose last witness item is the script
// and must hash to the program; 20 bytes is P2WPKH, whose two items run
// against DUP HASH160 <program> EQUALVERIFY CHECKSIG; any other length
// refuses. Version 1 with 32 bytes outside P2SH is Taproot (chapter 44,
// si_taproot) when its flag is set; the 2-byte anchor 4e73 (P2A) and
// every other version and length succeed, unless policy discourages them.
si_witness_program :: (sp: ScriptSpend, w: [] ScriptWork, version: int, program: ScriptItem, flags: int, p2sh: bool) -> int {
  prog := si_bytes(sp, w, program);
  count := sp.ins[sp.index].count;
  if version == 0 && prog.count == 32 {
    if count == 0 { return SCRIPT_ERR_WITNESS_PROGRAM_WITNESS_EMPTY; }
    script := si_witness(sp, count - 1);
    hash: [32] u8;
    #assert sha256_hash_checked(si_bytes(sp, w, script), hash[..], false) == SHA_OK;
    if !si_same(hash[..], prog) { return SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH; }
    return si_execute_witness(sp, w, count - 1, script, flags, SCRIPT_V0);
  }
  if version == 0 && prog.count == 20 {
    if count != 2 { return SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH; }
    script := si_new(w, 25);
    script_p2wpkh_code_vartime(prog, w[0].arena[script.at..<script.at+25]);
    return si_execute_witness(sp, w, 2, script, flags, SCRIPT_V0);
  }
  if version == 0 { return SCRIPT_ERR_WITNESS_PROGRAM_WRONG_LENGTH; }
  if version == 1 && prog.count == 32 && !p2sh {
    return ifx (flags & SCRIPT_VERIFY_TAPROOT) != 0 then si_taproot(sp, w, prog, flags) else SCRIPT_ERR_OK;
  }
  if version == 1 && prog.count == 2 && prog[0] == 0x4e && prog[1] == 0x73 && !p2sh { return SCRIPT_ERR_OK; }
  if (flags & SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM) != 0 { return SCRIPT_ERR_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM; }
  return SCRIPT_ERR_OK;
}
```

The `OP_SUCCESSx` opcodes are those that were disabled or never
assigned. A soft fork can give any of them a meaning, since a script
that uses one is valid today whatever follows.

<!-- source: coax/lib/bitcoin/script.coax success -->
```jai
// IsOpSuccess (script/script.cpp): the opcodes BIP-342 reserves for
// upgrades. In tapscript any of them, anywhere in the script, makes the
// spend valid, so a soft fork can give one any meaning. They are the
// opcodes that were disabled or unassigned: OP_RESERVED (80), OP_VER
// (98), OP_CAT to OP_RIGHT, OP_INVERT to OP_XOR, OP_RESERVED1 and 2,
// OP_2MUL and OP_2DIV, OP_MUL to OP_RSHIFT, and 187 to 254.
script_op_success_vartime :: (op: int) -> bool #export_c {
  return op == 80 || op == 98 || (op >= 126 && op <= 129) || (op >= 131 && op <= 134) || (op >= 137 && op <= 138)
    || (op >= 141 && op <= 142) || (op >= 149 && op <= 153) || (op >= 187 && op <= 254);
}
```

`si_eval`, chapter 36's EvalScript, takes the script version. In
tapscript it skips the 10,000-byte script limit and the 201-opcode count,
fails `OP_CHECKMULTISIG` and `OP_CHECKMULTISIGVERIFY` with
`TAPSCRIPT_CHECKMULTISIG` when they run, and requires the argument of
`OP_IF` and `OP_NOTIF` to be empty or the single byte `01`, as consensus.
It counts every opcode it reads, and `OP_CODESEPARATOR` records that
count as `codesep_pos`. A signature after it signs that position, not
the script after it as in legacy script.

<!-- source: coax/lib/bitcoin/script.coax eval -->
```jai
// EvalScript: runs the script (an element, so its pushes are views into
// the same bytes) on the workspace's stack, with an empty alt stack. Each
// opcode is read, its push checked against 520 bytes, counted against
// 201 when it is above OP_16, and refused when disabled, all whether or
// not its branch runs. Pushes and opcodes run only in a running branch;
// IF, NOTIF, ELSE and ENDIF always run, and so do VERIF and VERNOTIF,
// which sit between them and fail. After each opcode the two stacks
// together may hold 1,000 elements.
si_eval :: (sp: ScriptSpend, w: [] ScriptWork, script: ScriptItem, flags: int, version: int) -> int {
  s := si_bytes(sp, w, script);
  tapscript := version == SCRIPT_TAPSCRIPT;
  if !tapscript && s.count > SCRIPT_MAX_SIZE { return SCRIPT_ERR_SCRIPT_SIZE; }
  w[0].alt_depth = 0;
  w[0].codesep = BT_NO_CODESEP;
  minimal := (flags & SCRIPT_VERIFY_MINIMALDATA) != 0;
  cond := ScriptCond.{first_false=-1};
  ops := 0; pc := 0; begin := 0; position := -1;
  while pc < s.count {
    running := cond.first_false < 0;
    position += 1;                     // opcodes read so far, for codesep_pos
    o := script_op_vartime(s, pc);
    if !o.ok { return SCRIPT_ERR_BAD_OPCODE; }
    pc = o.next; op := o.op;
    if o.n > SCRIPT_MAX_ELEMENT { return SCRIPT_ERR_PUSH_SIZE; }
    if op > OP_16 && !tapscript { ops += 1; if ops > SCRIPT_MAX_OPS { return SCRIPT_ERR_OP_COUNT; } }
    if si_disabled(op) { return SCRIPT_ERR_DISABLED_OPCODE; }
    if op == OP_CODESEPARATOR && version == SCRIPT_BASE && (flags & SCRIPT_VERIFY_CONST_SCRIPTCODE) != 0 { return SCRIPT_ERR_OP_CODESEPARATOR; }
    error := SCRIPT_ERR_OK;
    if running && op <= OP_PUSHDATA4 {
      if minimal && !script_minimal_push_vartime(s[o.at..<o.at+o.n], op) { return SCRIPT_ERR_MINIMALDATA; }
      si_push(w, ScriptItem.{src=script.src, at=script.at+o.at, n=o.n});
    } else if op >= OP_IF && op <= OP_ENDIF {
      if op == OP_IF || op == OP_NOTIF {
        value := false;  // mutant: equivalent (in a skipped branch the pushed value is never read)
        if running {
          if si_size(w) < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
          top := si_bytes(sp, w, si_top(w, 1));
          // Tapscript: 0 or 1 exactly, as consensus.
          if tapscript && (top.count > 1 || (top.count == 1 && top[0] != 1)) { return SCRIPT_ERR_TAPSCRIPT_MINIMALIF; }
          if version == SCRIPT_V0 && (flags & SCRIPT_VERIFY_MINIMALIF) != 0 && (top.count > 1 || (top.count == 1 && top[0] != 1)) { return SCRIPT_ERR_MINIMALIF; }
          value = script_bool_vartime(top) != (op == OP_NOTIF);
          si_pop(w, 1);
        }
        cond = si_cond_push(cond, value);
      } else if op == OP_ELSE || op == OP_ENDIF {
        if cond.size == 0 { return SCRIPT_ERR_UNBALANCED_CONDITIONAL; }
        cond = ifx op == OP_ELSE then si_cond_toggle(cond) else si_cond_pop(cond);
      } else { return SCRIPT_ERR_BAD_OPCODE; }               // OP_VERIF, OP_VERNOTIF
    } else if running && (op == OP_CHECKMULTISIG || op == OP_CHECKMULTISIGVERIFY) {
      if tapscript { return SCRIPT_ERR_TAPSCRIPT_CHECKMULTISIG; }
      step := si_checkmultisig(sp, w, s, begin, op, flags, version, ops);
      error = step.error; ops += step.keys;
    } else if running {
      error = si_run(sp, w, s, begin, op, flags, version);
      if op == OP_CODESEPARATOR { begin = pc; w[0].codesep = position; }
    }
    if error != SCRIPT_ERR_OK { return error; }
    if si_size(w) + w[0].alt_depth > SCRIPT_MAX_STACK { return SCRIPT_ERR_STACK_SIZE; }
  }
  if cond.size != 0 { return SCRIPT_ERR_UNBALANCED_CONDITIONAL; }
  return SCRIPT_ERR_OK;
}
```

## Signatures in tapscript

`OP_CHECKSIG`, `OP_CHECKSIGVERIFY` and `OP_CHECKSIGADD` share one
procedure. `OP_CHECKSIGADD` pops a signature, a number and a key, and
pushes the number plus one when the signature is valid. A k-of-n
multisig is `<key1> OP_CHECKSIG <key2> OP_CHECKSIGADD ... <k>
OP_NUMEQUAL`, with an empty signature for each key that doesn't sign.

<!-- source: coax/lib/bitcoin/script.coax tapchecksig -->
```jai
// OP_CHECKSIG, OP_CHECKSIGVERIFY and OP_CHECKSIGADD in tapscript
// (EvalChecksigTapscript). An empty signature is a false check that the
// script may go on from; any other signature costs 50 of the budget and
// must verify, or the script fails. A 32-byte key is a BIP-340 key, an
// empty one fails, and any other length is a key type for a future soft
// fork, which passes unchecked. OP_CHECKSIGADD (sig n key -- n+success)
// counts signatures in place of CHECKMULTISIG.
si_checksig_tapscript :: (sp: ScriptSpend, w: [] ScriptWork, op: int, flags: int) -> int {
  args := ifx op == OP_CHECKSIGADD then 3 else 2;
  if si_size(w) < args { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  count := 0;
  if op == OP_CHECKSIGADD {
    n := script_num_vartime(si_bytes(sp, w, si_top(w, 2)), (flags & SCRIPT_VERIFY_MINIMALDATA) != 0, 4);
    if !n.ok { return SCRIPT_ERR_SCRIPTNUM; }
    count = n.value;
  }
  sig := si_bytes(sp, w, si_top(w, args)); key := si_bytes(sp, w, si_top(w, 1));
  success := sig.count > 0;
  if success {
    w[0].weight -= SCRIPT_WEIGHT_PER_SIGOP;
    if w[0].weight < 0 { return SCRIPT_ERR_TAPSCRIPT_VALIDATION_WEIGHT; }
  }
  if key.count == 0 { return SCRIPT_ERR_TAPSCRIPT_EMPTY_PUBKEY; }
  if key.count == 32 {
    if success { error := si_schnorr(sp, w, sig, key, SCRIPT_TAPSCRIPT); if error != SCRIPT_ERR_OK { return error; } }
  } else if (flags & SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_PUBKEYTYPE) != 0 {
    return SCRIPT_ERR_DISCOURAGE_UPGRADABLE_PUBKEYTYPE;
  }
  si_pop(w, args);
  if op == OP_CHECKSIGADD { si_push(w, si_number(w, count + (ifx success then 1 else 0))); return SCRIPT_ERR_OK; }
  if op == OP_CHECKSIGVERIFY { return ifx success then SCRIPT_ERR_OK else SCRIPT_ERR_CHECKSIGVERIFY; }
  si_push(w, si_bool(success));
  return SCRIPT_ERR_OK;
}
```

The rules, in Core's order:

- A non-empty signature costs 50 of the budget. A budget below zero is
  `TAPSCRIPT_VALIDATION_WEIGHT`. A witness pays for its signature checks
  with its own size, so a block can't be filled with cheap scripts that
  each check many signatures.
- An empty key is `TAPSCRIPT_EMPTY_PUBKEY`.
- A 32-byte key checks a non-empty signature, and any failure ends the
  script. A valid script can't carry a wrong signature, so a batch
  verifier can check every signature in a block at once.
- A key of any other length is a key type for a future soft fork. Its
  signature is not checked, but it still costs 50.
- An empty signature is a false result the script can go on from.

A signature of 64 bytes signs with `SIGHASH_DEFAULT`. One of 65 bytes
carries its hash type last, which may not be `0x00`, since that would be
a second encoding of the same signature.

<!-- source: coax/lib/bitcoin/script.coax schnorr -->
```jai
// CheckSchnorrSignature: a signature of 64 bytes signs with
// SIGHASH_DEFAULT, one of 65 carries its hash type last, which must not
// be 0; then BIP-340's verification (chapter 23) against the 32-byte key.
// Each failure is its own error, and every one ends the script.
si_schnorr :: (sp: ScriptSpend, w: [] ScriptWork, sig: [] u8, key: [] u8, version: int) -> int {
  if sig.count != 64 && sig.count != 65 { return SCRIPT_ERR_SCHNORR_SIG_SIZE; }
  hashtype := BT_DEFAULT;
  if sig.count == 65 {
    hashtype = cast(int) sig[64];
    if hashtype == BT_DEFAULT { return SCRIPT_ERR_SCHNORR_SIG_HASHTYPE; }
  }
  hash: [32] u8;
  if !si_sighash_schnorr(sp, w, hashtype, version, hash[..]) { return SCRIPT_ERR_SCHNORR_SIG_HASHTYPE; }
  if !schnorr_verify_vartime(hash[..], key, sig[0..<64]) { return SCRIPT_ERR_SCHNORR_SIG; }
  return SCRIPT_ERR_OK;
}
```

## The signature hash

A tapscript signature signs chapter 43's SigMsg with BIP-342's
extension: the leaf hash, key version 0 and `codesep_pos`. SigMsg covers
five hashes over the whole transaction: the outpoints, the spent amounts,
the spent scriptPubKeys, the sequences and the outputs. A script can
check many signatures, so `si_tap_hashes` computes the five hashes once
per `VerifyScript` and keeps them in the workspace, as Core's
`PrecomputedTransactionData` does. SigMsg needs every spent output, not
just this input's. `ScriptSpend` now carries them in `spent`; a
transaction of one input may leave it empty and use `amount` and `spk`.

<!-- source: coax/lib/bitcoin/script.coax taphashes -->
```jai
// BIP-341's five hashes over the whole transaction (Core's
// PrecomputedTransactionData), single SHA-256 each, computed once per
// VerifyScript into w.tap: the outpoints, the spent amounts, the spent
// scriptPubKeys with their lengths, the sequences and the outputs. False
// when the spent outputs are not all known.
si_tap_hashes :: (sp: ScriptSpend, w: [] ScriptWork) -> bool {
  if w[0].tap_ready { return true; }
  b := sp.b; t := sp.t;
  if sp.spent.count != t.nin && !(sp.spent.count == 0 && t.nin == 1) { return false; }
  state: [SHA_STATE_BYTES] u8;
  for part: 0..<5 {
    si_start(state[..]);
    for k: 0..<(ifx part == 4 then t.nout else t.nin) {
      if part == 0 { si_absorb(state[..], b[sp.ins[k].outpoint..<sp.ins[k].outpoint+36]); }
      else if part == 1 { p := si_spent(sp, k); if p.amount < 0 { return false; } si_absorb_le(state[..], p.amount, 8); }
      else if part == 2 { p := si_spent(sp, k); si_absorb_compact(state[..], p.spk.count); si_absorb(state[..], p.spk); }
      else if part == 3 { si_absorb(state[..], b[sp.ins[k].sequence..<sp.ins[k].sequence+4]); }
      else { si_absorb(state[..], b[sp.outs[k].value..<sp.outs[k].script+sp.outs[k].script_n]); }
    }
    #assert sha256_final_checked(state[..], w[0].tap[part][..], false) == SHA_OK;
  }
  w[0].tap_ready = true;
  return true;
}
// The output input k spends.
si_spent :: (sp: ScriptSpend, k: int) -> ScriptSpent {
  if sp.spent.count == 0 { return ScriptSpent.{amount=sp.amount, spk=sp.spk}; }
  return sp.spent[k];
}
// SignatureHashSchnorr: chapter 43's SigMsg for this input from those
// hashes, with the annex, and for tapscript the leaf hash and codesep_pos,
// then hash_TapSighash. False when the hash type is not one of the seven,
// SINGLE has no output at this index, or a spent output is unknown.
si_sighash_schnorr :: (sp: ScriptSpend, w: [] ScriptWork, hashtype: int, version: int, out: [] u8) -> bool {
  if !si_tap_hashes(sp, w) { return false; }
  b := sp.b; t := sp.t; i := sp.ins[sp.index];
  single := b[0..<0];
  if sp.index < t.nout { o := sp.outs[sp.index]; single = b[o.value..<o.script+o.script_n]; }
  leaf := w[0].leaf[0..<ifx version == SCRIPT_TAPSCRIPT then 32 else 0];
  m: [BT_SIGMSG] u8; p := si_spent(sp, sp.index);
  n := bt_sigmsg_taproot_parts(w[0].tap[..], b[0..<4], b[t.locktime..<t.locktime+4], sp.index, b[i.outpoint..<i.outpoint+36],
    p.amount, p.spk, b[i.sequence..<i.sequence+4], single, hashtype, si_bytes(sp, w, w[0].annex), leaf, w[0].codesep, m[..]);
  if n < 0 { return false; }
  bt_tagged(BH_TAPSIGHASH, m[0..<n], out);
  return true;
}
```

`bt_sigmsg_taproot_parts` is chapter 43's message builder with the
transaction already taken apart, so the interpreter can pass its cached
hashes. Chapter 43's `bt_sigmsg_taproot` now computes the hashes and
calls it.

## The example

The example is [bitcoin/examples/tapscript.coax](../examples/tapscript.coax).
It runs the seven Taproot rows of `script_tests.json`, then a selection of
the library's Taproot cases by group: the key path, the script path,
signatures, the budget and the rest of tapscript. Then it runs all 83
cases and counts the `OP_SUCCESSx` opcodes.

<!-- source: bitcoin/examples/tapscript.coax example -->
```jai
#load "../../coax/lib/bitcoin/script.coax";
#load "../../coax/tests/bitcoin/script_vectors.coax";
#import "Basic";

// The transaction, its inputs, outputs and witness items, and the bytes of
// the scriptPubKey the input spends: room for the largest case.
Spend :: struct {
  b: [16384] u8; spk: [128] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [1100] TxItem;
}
// Reads hex into the spend and returns the input-0 view of it. Each
// transaction here has one input, so its spent output is amount and spk.
spend_of :: (s: [] Spend, tx: string, spk: string, amount: int) -> ScriptSpend {
  b := tx_unhex(tx, s[0].b[..]);
  t := tx_read_vartime(b, true, s[0].ins[..], s[0].outs[..], s[0].items[..]);
  #assert t.code == TX_OK;
  return ScriptSpend.{b=b, t=t, ins=s[0].ins[0..<t.nin], outs=s[0].outs[0..<t.nout], items=s[0].items[0..<t.nitems],
    index=0, amount=amount, spk=tx_unhex(spk, s[0].spk[..])};
}
name :: (error: int) -> string { return ifx error == SCRIPT_BAD_FLAGS then "BAD_FLAGS" else SCRIPT_ERR_NAMES[error]; }
// Runs the library's Taproot case with this label and prints its result.
show :: (s: [] Spend, w: [] ScriptWork, label: string) {
  for c: SI_TAPROOT {
    if c.label != label { continue; }
    print("  %: %\n", label, name(script_verify_vartime(spend_of(s, c.tx, c.spk, c.amount), c.flags, w)));
    return;
  }
  #assert false;
}

main :: () {
  s: [1] Spend; w: [1] ScriptWork;

  // The seven rows of script_tests.json that chapter 36 could not run.
  print("Bitcoin Core script_tests.json, the Taproot rows\n");
  for l: SCRIPT_TAPROOT_LINES {
    for r: SCRIPT_ROWS {
      if r.line != l { continue; }
      error := script_verify_vartime(spend_of(s[..], r.tx, r.spk, r.amount), r.flags, w[..]);
      print("  line %: %, Core expects %\n", l, name(error), name(r.error));
    }
  }

  print("the key path\n");
  for l: [6] string.["key path, SIGHASH_DEFAULT", "key path, SINGLE|ANYONECANPAY", "key path, a spoiled signature",
    "key path, hash type 0 written out", "key path, 63 bytes", "key path, signed without the annex it carries"] { show(s[..], w[..], l); }

  print("the script path\n");
  for l: [7] string.["script path, a leaf that leaves 1", "script path, the parity bit flipped", "script path, the path hash changed",
    "script path, a control block of 34 bytes", "script path, leaf version 0xc2", "script path, leaf version 0xc2, discouraged",
    "script path, with an annex"] { show(s[..], w[..], l); }

  print("signatures in tapscript\n");
  for l: [12] string.["CHECKSIG, a valid signature", "CHECKSIG, the wrong key signed", "CHECKSIG, an empty signature",
    "CHECKSIG NOT, an empty signature", "CHECKSIG, an empty key", "CHECKSIG, a 33-byte key, any signature",
    "CODESEPARATOR, signed at its position", "CODESEPARATOR, signed with none", "CHECKSIGADD, 2 of 3, keys 1 and 3",
    "CHECKSIGADD, 2 of 3, one signature", "CHECKSIGADD, 2 of 3, a spoiled one", "CHECKMULTISIG"] { show(s[..], w[..], l); }

  print("the validation-weight budget\n");
  for l: [4] string.["the budget: 6 checks, a 1-byte signature", "the budget: 7 checks, a 1-byte signature",
    "the budget: 7 checks, a 14-byte signature", "the budget: 8 checks, a 1-byte signature"] { show(s[..], w[..], l); }

  print("the rest of tapscript\n");
  for l: [11] string.["tapscript, MINIMALIF, 0x02", "tapscript, OP_SUCCESS 0x50 after RETURN", "tapscript, OP_SUCCESS 0x50, discouraged",
    "tapscript, OP_SUCCESS then a bad push", "tapscript, a bad push, no OP_SUCCESS", "tapscript, 300 NOPs",
    "tapscript, a script of 10,001 bytes", "tapscript, 1,001 stack elements", "tapscript, an element of 521 bytes",
    "tapscript, 2,000 SHA256s", "tapscript, 900 SHA256s kept, then 200 more"] { show(s[..], w[..], l); }

  // Every Taproot case of the library, against script_reference.py's result.
  good := 0;
  for c: SI_TAPROOT {
    if script_verify_vartime(spend_of(s[..], c.tx, c.spk, c.amount), c.flags, w[..]) == c.error { good += 1; }
  }
  print("all % of % Taproot cases give the expected result\n", good, SI_TAPROOT.count);
  count := 0;
  for op: 0..<256 { if script_op_success_vartime(op) { count += 1; } }
  print("OP_SUCCESSx opcodes: %\n", count);
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/tapscript.coax --run-steps 250000000
```

It prints:

```text
Bitcoin Core script_tests.json, the Taproot rows
  line 1296: OK, Core expects OK
  line 1297: OK, Core expects OK
  line 1298: EVAL_FALSE, Core expects EVAL_FALSE
  line 1299: OK, Core expects OK
  line 1300: TAPSCRIPT_MINIMALIF, Core expects TAPSCRIPT_MINIMALIF
  line 1301: TAPSCRIPT_MINIMALIF, Core expects TAPSCRIPT_MINIMALIF
  line 1302: TAPSCRIPT_EMPTY_PUBKEY, Core expects TAPSCRIPT_EMPTY_PUBKEY
the key path
  key path, SIGHASH_DEFAULT: OK
  key path, SINGLE|ANYONECANPAY: OK
  key path, a spoiled signature: SCHNORR_SIG
  key path, hash type 0 written out: SCHNORR_SIG_HASHTYPE
  key path, 63 bytes: SCHNORR_SIG_SIZE
  key path, signed without the annex it carries: SCHNORR_SIG
the script path
  script path, a leaf that leaves 1: OK
  script path, the parity bit flipped: WITNESS_PROGRAM_MISMATCH
  script path, the path hash changed: WITNESS_PROGRAM_MISMATCH
  script path, a control block of 34 bytes: TAPROOT_WRONG_CONTROL_SIZE
  script path, leaf version 0xc2: OK
  script path, leaf version 0xc2, discouraged: DISCOURAGE_UPGRADABLE_TAPROOT_VERSION
  script path, with an annex: OK
signatures in tapscript
  CHECKSIG, a valid signature: OK
  CHECKSIG, the wrong key signed: SCHNORR_SIG
  CHECKSIG, an empty signature: EVAL_FALSE
  CHECKSIG NOT, an empty signature: OK
  CHECKSIG, an empty key: TAPSCRIPT_EMPTY_PUBKEY
  CHECKSIG, a 33-byte key, any signature: OK
  CODESEPARATOR, signed at its position: OK
  CODESEPARATOR, signed with none: SCHNORR_SIG
  CHECKSIGADD, 2 of 3, keys 1 and 3: OK
  CHECKSIGADD, 2 of 3, one signature: EVAL_FALSE
  CHECKSIGADD, 2 of 3, a spoiled one: SCHNORR_SIG
  CHECKMULTISIG: TAPSCRIPT_CHECKMULTISIG
the validation-weight budget
  the budget: 6 checks, a 1-byte signature: OK
  the budget: 7 checks, a 1-byte signature: TAPSCRIPT_VALIDATION_WEIGHT
  the budget: 7 checks, a 14-byte signature: OK
  the budget: 8 checks, a 1-byte signature: TAPSCRIPT_VALIDATION_WEIGHT
the rest of tapscript
  tapscript, MINIMALIF, 0x02: TAPSCRIPT_MINIMALIF
  tapscript, OP_SUCCESS 0x50 after RETURN: OK
  tapscript, OP_SUCCESS 0x50, discouraged: DISCOURAGE_OP_SUCCESS
  tapscript, OP_SUCCESS then a bad push: OK
  tapscript, a bad push, no OP_SUCCESS: BAD_OPCODE
  tapscript, 300 NOPs: OK
  tapscript, a script of 10,001 bytes: OK
  tapscript, 1,001 stack elements: STACK_SIZE
  tapscript, an element of 521 bytes: PUSH_SIZE
  tapscript, 2,000 SHA256s: OK
  tapscript, 900 SHA256s kept, then 200 more: OK
all 83 of 83 Taproot cases give the expected result
OP_SUCCESSx opcodes: 87
```

What the output shows:

- All seven Taproot rows give Core's result. Lines 1300 and 1301 pass
  `0x02` and `0x0101` to `OP_IF`, which tapscript refuses. Line 1302
  checks a signature against an empty key.
- A key-path signature of 63 bytes is `SCHNORR_SIG_SIZE`. One of 65
  bytes ending in `0x00` is `SCHNORR_SIG_HASHTYPE`. A signature made
  without the annex the witness carries fails, since the annex is signed.
- A flipped parity bit, a changed path hash or a control block of 34
  bytes fails. Leaf version `0xc2` passes unrun, except under its
  DISCOURAGE flag.
- An empty signature leaves false, so `CHECKSIG NOT` passes. A 33-byte
  key passes with any signature.
- A signature made for the position of the last `OP_CODESEPARATOR`
  passes, and the same signature made with no position fails.
- `OP_CHECKSIGADD` 2-of-3 passes with keys 1 and 3, leaves false with one
  signature, and fails with a wrong one. `OP_CHECKMULTISIG` fails.
- Seven signature checks need a witness of at least 300 bytes. With a
  1-byte filler signature for a key of unknown type the witness is too
  small, and with a 14-byte one it is large enough.
- `OP_SUCCESS` passes after `OP_RETURN` and before a bad push, but fails
  under its DISCOURAGE flag. 300 NOPs, a 10,001-byte script and 2,000
  hashes pass, past legacy script's limits. 1,001 stack elements and a
  521-byte element fail.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/tapscript.out](../fixtures/tapscript.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/tapscript.coax](../examples/checks/tapscript.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/tapscript.coax check -->
```jai
// The seven Taproot rows of script_tests.json give Core's results.
#test "tapscript: script_tests.json's Taproot rows" #steps 2000000000 {
  s: [1] Spend; w: [1] ScriptWork; ran := 0;
  for l: SCRIPT_TAPROOT_LINES {
    for r: SCRIPT_ROWS {
      if r.line != l { continue; }
      #assert script_verify_vartime(spend_of(s[..], r.tx, r.spk, r.amount), r.flags, w[..]) == r.error;
      ran += 1;
    }
  }
  #assert ran == 7;
}
// The budget is 50 plus the witness's size, and seven checks cost 350, so
// the witness needs 300 bytes: a 2-byte filler signature leaves it short,
// and a 14-byte one, which costs the same 50, makes room.
#test "tapscript: the budget follows the witness's size" #steps 400000000 {
  s: [1] Spend; w: [1] ScriptWork;
  for c: SI_TAPROOT {
    if c.label == "the budget: 7 checks, a 2-byte signature" {
      #assert script_verify_vartime(spend_of(s[..], c.tx, c.spk, c.amount), c.flags, w[..]) == SCRIPT_ERR_TAPSCRIPT_VALIDATION_WEIGHT;
    }
    if c.label == "the budget: 7 checks, a 14-byte signature" {
      #assert script_verify_vartime(spend_of(s[..], c.tx, c.spk, c.amount), c.flags, w[..]) == SCRIPT_ERR_OK;
    }
  }
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/tapscript.coax --run-steps 250000000 --only tapscript:
```

On 2026-11-03 both checks pass.

### The library's own checks

`script.coax` checks the 87 `OP_SUCCESSx` opcodes against BIP-342's
list, the new error and flag names against Core's, and runs the 83
Taproot cases in four parts, each with a library case beside it. The
cases come from
[make_script_vectors.py](../../coax/tests/bitcoin/make_script_vectors.py),
which builds each spend, signs it with BIP-340 after the transaction is
known, and records the result of
[script_reference.py](../../coax/tests/bitcoin/script_reference.py). They
cover:

- the key path with valid and undefined hash types, a spoiled signature,
  hash type 0 written out, wrong sizes, an annex and the flag off;
- the script path with a wrong parity, path, control size and leaf
  version, an empty witness, and an annex;
- `OP_CHECKSIG`, `OP_CHECKSIGVERIFY` and `OP_CHECKSIGADD` with valid,
  wrong, empty and 65-byte signatures and keys of 0, 32 and 33 bytes, and
  `OP_CHECKSIGADD` with a number that isn't minimal or is past 4 bytes;
- `OP_CODESEPARATOR` at several positions;
- the budget at 6, 7 and 8 checks;
- MINIMALIF, `OP_SUCCESSx` in skipped branches and under DISCOURAGE, bad
  pushes, the stack and element limits, and a script past 10,000 bytes;
- 900 elements kept while 200 more are made, which fills the workspace's
  arena and makes it compact.

`taproot.coax` and `sighash.coax` still pass with chapter 43's message
builder split in two.

```shell
coax/bin/coax test coax/lib/bitcoin/script.coax --run-steps 250000000
coax/bin/coax test coax/lib/bitcoin/sighash.coax --run-steps 250000000
coax/bin/coax test coax/lib/bitcoin/taproot.coax --run-steps 250000000
```

On 2026-11-03 `script.coax` runs 439 checks, with those of the files it
loads, all passing.

### Core in Python

`script_reference.py`, chapter 36's transliteration of Core's
interpreter, now also transliterates `VerifyWitnessProgram`'s Taproot
branch, `ExecuteWitnessScript`'s tapscript pre-scan,
`EvalChecksigTapscript`, `CheckSchnorrSignature` and `IsOpSuccess`. It
takes the signature message from Core's own `TaprootSignatureMsg`,
through chapter 43's
[taproot_reference.py](../../coax/tests/bitcoin/taproot_reference.py),
and builds the seven rows' `#SCRIPT#`, `#CONTROLBLOCK#` and
`#TAPROOTOUTPUT#` as Core's `script_tests.cpp` does: a single leaf
under the internal key of secret 1. `make_script_vectors.py` refuses to
write the vectors unless every row gives Core's result.

```shell
python3 coax/tests/bitcoin/make_script_vectors.py
```

On 2026-11-03 it prints "1263 script rows, 7 of them Taproot, 206
transactions, 4532 flag cases, 500 sighash rows; 439 library cases, 83
Taproot cases". Chapter 36's `script.py`, the native and Wasm run, was
not run again for this chapter. Mutation testing was not run (owner
speed order).

## What this chapter leaves out

- Building script-path spends: the control block a wallet writes and
  BIP-341's script-path vectors. That is
  [chapter 45](ch045-control-blocks-and-script-path-spends.md).
- Core's `script_assets_test.json` and the cases of
  `feature_taproot.py`, which Core generates at test time and does not
  pin as data.
- Which flags apply at which height: chapter 55. Mempool policy:
  chapter 81.
- Batch verification of a block's Schnorr signatures:
  [chapter 25](ch025-batch-verification.md).

## Summary

- A Taproot output is `OP_1 <32-byte key>`. One witness item is a key-path
  signature; more are a script path, with the control block last and the
  leaf script before it.
- An annex is a last item starting `0x50`. It is signed and otherwise
  ignored.
- Leaf version `0xc0` runs tapscript; other versions pass unrun.
- Tapscript checks BIP-340 signatures against 32-byte keys, counts them
  with `OP_CHECKSIGADD`, and fails `OP_CHECKMULTISIG`.
- An empty signature is false. A wrong one fails the script. A key of an
  unknown length passes.
- `OP_SUCCESSx` makes a script valid. MINIMALIF is consensus.
- Each signature check costs 50 of a budget of 50 plus the witness's
  size, in place of the script size and opcode limits.

## Dependencies

- [36. The script interpreter](ch036-the-script-interpreter.md)
- [43. Taproot signature hashes](ch043-taproot-signature-hashes.md)

## Sources

- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  "Script validation rules", pinned at
  [coax/tests/vectors/upstream/bip341/bip-0341.mediawiki](../../coax/tests/vectors/upstream/bip341/bip-0341.mediawiki).
- [BIP-342](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki),
  "Validating Tapscript" and "Resource limits".
- Bitcoin Core at `ddea9196`: `VerifyWitnessProgram`,
  `ExecuteWitnessScript`, `EvalChecksigTapscript` and
  `CheckSchnorrSignature` in `script/interpreter.cpp`, `IsOpSuccess` in
  `script/script.cpp`, and the Taproot rows of `script_tests.json` with
  `script_tests.cpp`, all pinned under
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
