# 81. Policy is not consensus: standardness

Consensus decides which blocks are valid. Every node must agree on it, and
a change to it is a fork. Policy decides which transactions a node relays
and keeps in its mempool before they are in a block. Each node picks its
own, and a change to it needs no one's agreement. Bitcoin Core's default
policy is called standardness, and most nodes run it.

Standardness refuses transactions that consensus accepts. It keeps the
outputs to known types, so that later soft forks can give new meaning to
the rest. It caps sizes and signature operations, so that a transaction
cannot cost the network more to check than it pays. It refuses outputs
too small to be worth spending. A miner may still put any of these
transactions in a block, and every node accepts that block.

This chapter writes Bitcoin Core's standardness rules and the reason each
one gives. The example builds one transaction per rule, shows the reason
Core refuses it, and then checks it the way a block would: every one is
valid. A Python transliteration of Core's `policy.cpp` agrees with every
line.

## What this chapter teaches

- What Core's `Solver` calls each kind of output, and which kinds are
  standard.
- How the dust threshold follows from the cost of spending an output, and
  why a witness output's threshold is lower.
- Each rule of `IsStandardTx`, in its order, with its reason.
- What Core checks about the outputs a transaction spends: BIP-54's sigop
  limit, the spent output's type and the P2SH redeem script.
- The witness limits for P2WSH and Taproot spends.
- Why each refused transaction is still valid in a block.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [policy.coax](../../coax/lib/bitcoin/policy.coax) | `pol_solver_vartime` | `Solver` |
| | `pol_standard_output_vartime` | `IsStandard` for one scriptPubKey |
| | `pol_dust_threshold_vartime` | `GetDustThreshold` |
| | `pol_standard_tx_vartime` | `IsStandardTx` |
| | `pol_inputs_vartime`, `pol_stack_vartime` | `ValidateInputsStandardness` with `CheckSigopsBIP54`, and the stack a push-only scriptSig leaves |
| | `pol_witness_vartime` | `IsWitnessStandard` |
| | `pol_check_vartime` | the standardness steps of `PreChecks`, in order |
| | `pol_consensus_vartime` | what a block checks of the same transaction |
| | `pol_case`, `pol_run` | the example's transactions, and both checks of each |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_verify_vartime`, `script_push_only_vartime` | chapter 36's `VerifyScript` and `IsPushOnly` |
| [sigops.coax](../../coax/lib/bitcoin/sigops.coax) | `sigop_count_vartime`, `sigop_cost_vartime` | chapter 53's sigop counts |
| [txcheck.coax](../../coax/lib/bitcoin/txcheck.coax) | `txcheck_vartime` | chapter 52's `CheckTransaction` |
| [fees.coax](../../coax/lib/bitcoin/fees.coax) | `fee_for_rate_vartime` | chapter 40's fee for a rate, rounded up |

## The limits

The numbers come from Core's `policy/policy.h`. Each has a reason:
400,000 weight is a tenth of a block, 1,650 bytes of scriptSig fits a
15-of-15 P2SH multisig spend, and 65 bytes is the smallest transaction
that cannot be mistaken for an inner node of a Merkle tree
(CVE-2017-12842).

<!-- source: coax/lib/bitcoin/policy.coax limits -->
```jai
// policy/policy.h at the pinned release.
POL_MIN_VERSION :: 1;               // TX_MIN_STANDARD_VERSION
POL_MAX_VERSION :: 3;               // TX_MAX_STANDARD_VERSION
POL_MAX_WEIGHT :: 400000;           // MAX_STANDARD_TX_WEIGHT
POL_MIN_NONWITNESS :: 65;           // MIN_STANDARD_TX_NONWITNESS_SIZE (CVE-2017-12842)
POL_MAX_SCRIPTSIG :: 1650;          // MAX_STANDARD_SCRIPTSIG_SIZE
POL_MAX_P2SH_SIGOPS :: 15;          // MAX_P2SH_SIGOPS
POL_MAX_LEGACY_SIGOPS :: 2500;      // MAX_TX_LEGACY_SIGOPS (BIP-54)
POL_MAX_SIGOPS_COST :: 16000;       // MAX_STANDARD_TX_SIGOPS_COST, a fifth of a block's
POL_MAX_P2WSH_ITEMS :: 100;         // MAX_STANDARD_P2WSH_STACK_ITEMS
POL_MAX_P2WSH_ITEM :: 80;           // MAX_STANDARD_P2WSH_STACK_ITEM_SIZE
POL_MAX_P2WSH_SCRIPT :: 3600;       // MAX_STANDARD_P2WSH_SCRIPT_SIZE
POL_MAX_TAPSCRIPT_ITEM :: 80;       // MAX_STANDARD_TAPSCRIPT_STACK_ITEM_SIZE
POL_DUST_RELAY_FEE :: 3000;         // DUST_RELAY_TX_FEE, sat/kvB
POL_MAX_OP_RETURN :: 100000;        // MAX_OP_RETURN_RELAY, -datacarriersize's default
POL_MAX_DUST_OUTPUTS :: 1;          // MAX_DUST_OUTPUTS_PER_TX
// MANDATORY_SCRIPT_VERIFY_FLAGS: the flags every block enforces.
POL_CONSENSUS_FLAGS :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_NULLDUMMY
  | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_TAPROOT;
// Taproot's annex tag and leaf version mask (script/interpreter.h).
POL_ANNEX_TAG :: 0x50;
POL_LEAF_MASK :: 0xfe;
POL_LEAF_TAPSCRIPT :: 0xc0;
```

A node can change some of them. `-datacarriersize` caps the bytes all of
a transaction's `OP_RETURN` outputs may hold; its default is the most a
standard transaction can hold anyway. `-permitbaremultisig=0` refuses
bare multisig outputs, and `-dustrelayfee` sets the rate dust is measured
at:

<!-- source: coax/lib/bitcoin/policy.coax options -->
```jai
// The node's settings: -datacarriersize (the OP_RETURN bytes one
// transaction may carry; -datacarrier=0 makes it 0), -permitbaremultisig
// and -dustrelayfee in sat/kvB.
PolOptions :: struct { datacarrier: int; bare_multisig: bool; dust_rate: int; }
pol_defaults :: () -> PolOptions {
  return PolOptions.{datacarrier=POL_MAX_OP_RETURN, bare_multisig=true, dust_rate=POL_DUST_RELAY_FEE};
}
```

Each rule refuses with a short reason, the string Core returns from
`testmempoolaccept` and logs. The reasons about spent outputs also carry a
longer message:

<!-- source: coax/lib/bitcoin/policy.coax codes -->
```jai
// The result of a check: a code with Core's reject reason, the input or
// output it is about (-1 for the transaction), and for
// "bad-txns-nonstandard-inputs" which of Core's debug messages, with its
// numbers in a and b.
PolResult :: struct { code: int; index: int; detail: int; a: int; b: int; }
POL_OK :: 0;
POL_VERSION :: 1;            // "version"
POL_TX_SIZE :: 2;            // "tx-size"
POL_SCRIPTSIG_SIZE :: 3;     // "scriptsig-size"
POL_SCRIPTSIG_PUSHONLY :: 4; // "scriptsig-not-pushonly"
POL_SCRIPTPUBKEY :: 5;       // "scriptpubkey"
POL_DATACARRIER :: 6;        // "datacarrier"
POL_BARE_MULTISIG :: 7;      // "bare-multisig"
POL_DUST :: 8;               // "dust"
POL_TX_SIZE_SMALL :: 9;      // "tx-size-small"
POL_INPUTS :: 10;            // "bad-txns-nonstandard-inputs"
POL_WITNESS :: 11;           // "bad-witness-nonstandard"
POL_SIGOPS :: 12;            // "bad-txns-too-many-sigops"
POL_CONTEXT :: 13;           // the transaction did not read, or the spent outputs are not one per input
POL_REASONS :: [14] string.["standard", "version", "tx-size", "scriptsig-size", "scriptsig-not-pushonly",
  "scriptpubkey", "datacarrier", "bare-multisig", "dust", "tx-size-small", "bad-txns-nonstandard-inputs",
  "bad-witness-nonstandard", "bad-txns-too-many-sigops", "bad context"];
// ValidateInputsStandardness's debug messages.
POL_IN_BIP54 :: 1;           // "non-witness sigops exceed bip54 limit"
POL_IN_UNKNOWN :: 2;         // "input %u script unknown"
POL_IN_UNDEFINED :: 3;       // "input %u witness program is undefined"
POL_IN_MALFORMED :: 4;       // "p2sh scriptsig malformed (input %u: %s)", the script error in a
POL_IN_MISSING :: 5;         // "input %u P2SH redeemscript missing"
POL_IN_P2SH_SIGOPS :: 6;     // "p2sh redeemscript sigops exceed limit (input %u: %u > %u)", a > b
pol_refused :: (code: int, index: int) -> PolResult { return PolResult.{code=code, index=index}; }
pol_inputs_refused :: (detail: int, index: int, a: int, b: int) -> PolResult {
  return PolResult.{code=POL_INPUTS, index=index, detail=detail, a=a, b=b};
}
```

## Output types

`Solver` reads a scriptPubKey and names its type. It checks the forms
with fixed shapes first: P2SH, then a witness program by version and
length, then `OP_RETURN` followed only by pushes, then pay-to-public-key,
pay-to-public-key-hash and bare multisig. Anything else is nonstandard.

<!-- source: coax/lib/bitcoin/policy.coax types -->
```jai
// TxoutType (script/solver.h), with GetTxnOutputType's names.
POL_NONSTANDARD :: 0; POL_ANCHOR :: 1; POL_PUBKEY :: 2; POL_PUBKEYHASH :: 3; POL_SCRIPTHASH :: 4;
POL_MULTISIG :: 5; POL_NULL_DATA :: 6; POL_WITNESS_V0_KEYHASH :: 7; POL_WITNESS_V0_SCRIPTHASH :: 8;
POL_WITNESS_V1_TAPROOT :: 9; POL_WITNESS_UNKNOWN :: 10;
POL_TYPE_NAMES :: [11] string.["nonstandard", "anchor", "pubkey", "pubkeyhash", "scripthash", "multisig",
  "nulldata", "witness_v0_keyhash", "witness_v0_scripthash", "witness_v1_taproot", "witness_unknown"];
// A scriptPubKey's type, and for multisig its m and n.
PolSolved :: struct { type: int; m: int; n: int; }
```

The shapes it matches are byte patterns. A public key must have a size
its first byte allows. A multisig script's counts must be minimal pushes,
and the key count must match the keys:

<!-- source: coax/lib/bitcoin/policy.coax shapes -->
```jai
// CScript::IsPayToAnchor: OP_1 and the two bytes 4e73 (P2A).
pol_anchor_vartime :: (s: [] u8) -> bool #export_c {
  return s.count == 4 && s[0] == OP_1 && s[1] == 2 && s[2] == 0x4e && s[3] == 0x73;
}
// CScript::IsUnspendable: OP_RETURN first, or longer than a script may be.
pol_unspendable_vartime :: (s: [] u8) -> bool #export_c {
  return (s.count > 0 && s[0] == OP_RETURN) || s.count > SCRIPT_MAX_SIZE;
}
// CPubKey::ValidSize: 33 bytes after 02 or 03, 65 after 04, 06 or 07.
pol_key_size :: (s: [] u8, at: int, n: int) -> bool {
  if n == 0 { return false; }
  h := s[at];
  return (n == 33 && (h == 2 || h == 3)) || (n == 65 && (h == 4 || h == 6 || h == 7));
}
// GetScriptNumber: OP_1 to OP_16, or a minimal push of a minimal
// CScriptNum, from lo to hi; -1 otherwise.
pol_number :: (s: [] u8, op: ScriptOp, lo: int, hi: int) -> int {
  v := -1;
  if op.op >= OP_1 && op.op <= OP_16 { v = op.op - 0x50; }
  else if op.ok && op.op <= OP_PUSHDATA4 {
    data := s[op.at..<op.at+op.n];
    if !script_minimal_push_vartime(data, op.op) { return -1; }
    x := script_num_vartime(data, true, 4);
    if !x.ok { return -1; }
    v = x.value;
  }
  if v < lo || v > hi { return -1; }
  return v;
}
// MatchMultisig: m, keys of a valid size, n equal to the keys counted,
// then OP_CHECKMULTISIG as the last byte. m and n are 0 when it doesn't
// match.
pol_multisig :: (s: [] u8) -> PolSolved {
  if s.count < 1 || s[s.count-1] != OP_CHECKMULTISIG { return PolSolved.{}; }
  op := script_op_vartime(s, 0);
  if !op.ok { return PolSolved.{}; }
  m := pol_number(s, op, 1, SCRIPT_MAX_KEYS);
  if m < 0 { return PolSolved.{}; }
  n := 0;
  op = script_op_vartime(s, op.next);
  while op.ok && op.op <= OP_PUSHDATA4 && pol_key_size(s, op.at, op.n) { n += 1; op = script_op_vartime(s, op.next); }
  count := pol_number(s, op, m, SCRIPT_MAX_KEYS);
  if count < 0 || count != n || op.next + 1 != s.count { return PolSolved.{}; }
  return PolSolved.{type=POL_MULTISIG, m=m, n=n};
}
```

<!-- source: coax/lib/bitcoin/policy.coax solver -->
```jai
// Solver: P2SH first, then witness programs by version and length, then
// OP_RETURN followed only by pushes, then P2PK, P2PKH and bare multisig.
// A version 0 program of another length is nonstandard; another version
// is witness_unknown, except P2A.
pol_solver_vartime :: (s: [] u8) -> PolSolved #export_c {
  if script_p2sh_vartime(s) { return PolSolved.{type=POL_SCRIPTHASH}; }
  version := script_witness_version_vartime(s);
  if version >= 0 {
    n := s.count - 2;
    if version == 0 && n == 20 { return PolSolved.{type=POL_WITNESS_V0_KEYHASH}; }
    if version == 0 && n == 32 { return PolSolved.{type=POL_WITNESS_V0_SCRIPTHASH}; }
    if version == 1 && n == 32 { return PolSolved.{type=POL_WITNESS_V1_TAPROOT}; }
    if pol_anchor_vartime(s) { return PolSolved.{type=POL_ANCHOR}; }
    if version != 0 { return PolSolved.{type=POL_WITNESS_UNKNOWN}; }
    return PolSolved.{type=POL_NONSTANDARD};
  }
  if s.count >= 1 && s[0] == OP_RETURN && script_push_only_vartime(s[1..<s.count]) { return PolSolved.{type=POL_NULL_DATA}; }
  if s.count == 67 && s[0] == 65 && s[66] == OP_CHECKSIG && pol_key_size(s, 1, 65) { return PolSolved.{type=POL_PUBKEY}; }
  if s.count == 35 && s[0] == 33 && s[34] == OP_CHECKSIG && pol_key_size(s, 1, 33) { return PolSolved.{type=POL_PUBKEY}; }
  if s.count == 25 && s[0] == OP_DUP && s[1] == OP_HASH160 && s[2] == 20 && s[23] == OP_EQUALVERIFY && s[24] == OP_CHECKSIG {
    return PolSolved.{type=POL_PUBKEYHASH};
  }
  return pol_multisig(s);   // POL_NONSTANDARD, 0, when it is not multisig either
}
// IsStandard for one scriptPubKey: any type but nonstandard, and
// multisig only up to x-of-3.
pol_standard_output_vartime :: (s: [] u8) -> bool #export_c {
  t := pol_solver_vartime(s);
  if t.type == POL_NONSTANDARD { return false; }
  if t.type == POL_MULTISIG { return t.n >= 1 && t.n <= 3 && t.m >= 1 && t.m <= t.n; }  // mutant: equivalent (MatchMultisig already gives 1 <= m <= n)
  return true;
}
```

A witness program of an undefined version or length is
`witness_unknown`, and standard to pay to. Soft forks give those programs
their meaning, so wallets may already pay to them; spending them is a
different matter, as the section on spent outputs shows. Version 0 has
only two lengths, so any other length is nonstandard. Bare multisig is
standard only up to three keys.

## Dust

An output is dust when spending it would cost more in fees than it is
worth. Core prices the spend at `-dustrelayfee`, 3,000 sat/kvB by
default: the output's own bytes plus a typical input that spends it, 148
bytes for a legacy output and 67 bytes for a witness output, whose
107-byte signature and key weigh a quarter. An output that cannot be
spent, `OP_RETURN` or a script over 10,000 bytes, is never dust.

<!-- source: coax/lib/bitcoin/policy.coax dust -->
```jai
// GetDustThreshold: what spending the output would cost at rate sat/kvB,
// rounded up: the output's size plus the input that spends it, 148 bytes,
// or for a witness program 67, its 107-byte witness weighed at a quarter.
// An unspendable output is never dust.
pol_dust_threshold_vartime :: (spk: [] u8, rate: int) -> int #export_c {
  if pol_unspendable_vartime(spk) { return 0; }
  size := 8 + tx_compact_size_vartime(spk.count) + spk.count;
  if script_witness_version_vartime(spk) >= 0 { size += 32 + 4 + 1 + 107 / TX_SCALE + 4; }
  else { size += 32 + 4 + 1 + 107 + 4; }
  return fee_for_rate_vartime(rate, size);
}
```

The fee rounds up. At 3,702 sat/kvB, a P2PKH output's 182 bytes cost
673.764 sat, so 673 sat is dust and 674 is not; Core's `test_IsStandard`
checks both.

The example prints each type with its threshold:

```text
output types, and dust at -dustrelayfee=3000 sat/kvB
  P2PK, compressed key: 35 bytes, pubkey, dust below 576 sat
  P2PK, uncompressed key: 67 bytes, pubkey, dust below 672 sat
  P2PKH: 25 bytes, pubkeyhash, dust below 546 sat
  P2SH: 23 bytes, scripthash, dust below 540 sat
  P2WPKH: 22 bytes, witness_v0_keyhash, dust below 294 sat
  P2WSH: 34 bytes, witness_v0_scripthash, dust below 330 sat
  P2TR: 34 bytes, witness_v1_taproot, dust below 330 sat
  a version 2 program: 4 bytes, witness_unknown, dust below 240 sat
  P2A: 4 bytes, anchor, dust below 240 sat
  OP_RETURN: 7 bytes, nulldata, never dust
  bare 2-of-4 multisig: 139 bytes, multisig, not standard
  OP_1: 1 byte, nonstandard, not standard
```

## IsStandardTx

`IsStandardTx` looks only at the transaction itself. It checks the
version, then the weight, then each input's scriptSig, then each output,
then dust:

<!-- source: coax/lib/bitcoin/policy.coax standard -->
```jai
// IsStandardTx, its rules in its order. One dust output is allowed: it is
// ephemeral dust, which the mempool accepts only from a transaction that
// pays no fee (chapter 82).
pol_standard_tx_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut, o: PolOptions) -> PolResult #export_c {
  version := tx_le_vartime(b, 0, 4);     // unsigned
  if version > POL_MAX_VERSION || version < POL_MIN_VERSION { return pol_refused(POL_VERSION, -1); }
  if tx_weight_vartime(t) > POL_MAX_WEIGHT { return pol_refused(POL_TX_SIZE, -1); }
  for k: 0..<t.nin {
    sig := b[ins[k].script..<ins[k].script+ins[k].script_n];
    if sig.count > POL_MAX_SCRIPTSIG { return pol_refused(POL_SCRIPTSIG_SIZE, k); }
    if !script_push_only_vartime(sig) { return pol_refused(POL_SCRIPTSIG_PUSHONLY, k); }
  }
  left := ifx o.datacarrier > 0 then o.datacarrier else 0;
  for k: 0..<t.nout {
    spk := b[outs[k].script..<outs[k].script+outs[k].script_n];
    if !pol_standard_output_vartime(spk) { return pol_refused(POL_SCRIPTPUBKEY, k); }
    type := pol_solver_vartime(spk).type;
    if type == POL_NULL_DATA {
      if spk.count > left { return pol_refused(POL_DATACARRIER, k); }
      left -= spk.count;
    } else if type == POL_MULTISIG && !o.bare_multisig { return pol_refused(POL_BARE_MULTISIG, k); }
  }
  dust := 0;
  for k: 0..<t.nout {
    spk := b[outs[k].script..<outs[k].script+outs[k].script_n];
    if tx_le_vartime(b, outs[k].value, 8) < pol_dust_threshold_vartime(spk, o.dust_rate) { dust += 1; }
  }
  if dust > POL_MAX_DUST_OUTPUTS { return pol_refused(POL_DUST, -1); }
  return PolResult.{index=-1};
}
```

- Version 1 and 2 are the versions in use, and 3 is TRUC (BIP-431). A
  higher version is left for a future rule, so the rule refuses it now.
- A scriptSig may only push data. Under P2SH consensus already demands
  that; standardness demands it of every input, so that no one else can
  change what a scriptSig does by adding operations to it.
- `OP_RETURN` outputs share one budget of `-datacarriersize` bytes,
  counted as whole scripts.
- One output may be dust. That output is ephemeral dust: chapter 82's
  mempool takes it only from a transaction that pays no fee, so that a
  child spends it in the same package.

## The outputs a transaction spends

The next checks need the outputs the transaction spends. Core runs them
after it has found those outputs in its UTXO set.

To see a P2SH redeem script, Core runs the scriptSig with no flags and
takes the last element. The scriptSig is push-only by now, so this
evaluation only pushes, but it can still fail: `OP_RESERVED` counts as a
push for `IsPushOnly` but stops the script, and pushes are limited to 520
bytes and the stack to 1,000 elements.

<!-- source: coax/lib/bitcoin/policy.coax redeem -->
```jai
// What EvalScript leaves from a push-only scriptSig: how many elements,
// with the last copied to out (at most 520 bytes), or a script error.
// OP_RESERVED counts as a push for IsPushOnly but fails here; a push over
// 520 bytes or a stack over 1,000 fails too. The scriptSig is push-only,
// as IsStandardTx made it; any other opcode is SCRIPT_ERR_SIG_PUSHONLY.
PolStack :: struct { error: int; depth: int; n: int; }
pol_stack_vartime :: (sig: [] u8, out: [] u8) -> PolStack {
  if sig.count > SCRIPT_MAX_SIZE { return PolStack.{error=SCRIPT_ERR_SCRIPT_SIZE}; }
  depth := 0; n := 0; pc := 0;
  while pc < sig.count {
    op := script_op_vartime(sig, pc);
    if !op.ok { return PolStack.{error=SCRIPT_ERR_BAD_OPCODE}; }
    if op.n > SCRIPT_MAX_ELEMENT { return PolStack.{error=SCRIPT_ERR_PUSH_SIZE}; }
    if op.op <= OP_PUSHDATA4 { n = op.n; for i: 0..<n { out[i] = sig[op.at+i]; } }
    else if op.op == OP_1NEGATE { n = 1; out[0] = 0x81; }
    else if op.op >= OP_1 && op.op <= OP_16 { n = 1; out[0] = cast(u8) (op.op - 0x50); }
    else if op.op == 0x50 { return PolStack.{error=SCRIPT_ERR_BAD_OPCODE}; }   // OP_RESERVED
    else { return PolStack.{error=SCRIPT_ERR_SIG_PUSHONLY}; }
    depth += 1;
    if depth > SCRIPT_MAX_STACK { return PolStack.{error=SCRIPT_ERR_STACK_SIZE}; }
    pc = op.next;
  }
  return PolStack.{depth=depth, n=n};
}
```

`ValidateInputsStandardness` first applies BIP-54's limit of 2,500
signature operations across the scriptSigs and the spent scriptPubKeys,
with each P2SH redeem script counted in place of its output. It then
refuses a spent output of a nonstandard type and an undefined witness
program, whose spending rules a soft fork may still define, and a P2SH
redeem script with more than 15 signature operations:

<!-- source: coax/lib/bitcoin/policy.coax inputs -->
```jai
// ValidateInputsStandardness: BIP-54's limit on the scriptSigs' and spent
// scriptPubKeys' sigops across the whole transaction, counted the
// accurate way and with each P2SH redeem script in place of its output;
// then each spent output's type, which may be neither nonstandard nor an
// undefined witness program; and each P2SH redeem script, which must be
// there and hold at most 15 sigops. A coinbase spends nothing.
pol_inputs_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, prev: [] u8, spent: [] SigopSpent) -> PolResult #export_c {
  if tx_coinbase_vartime(b, t, ins) { return PolResult.{index=-1}; }
  sigops := 0;
  for k: 0..<t.nin {
    sig := b[ins[k].script..<ins[k].script+ins[k].script_n];
    spk := prev[spent[k].script..<spent[k].script+spent[k].script_n];
    sigops += sigop_count_vartime(sig, true) + sigop_p2sh_vartime(spk, sig);
    if sigops > POL_MAX_LEGACY_SIGOPS { return pol_inputs_refused(POL_IN_BIP54, -1, 0, 0); }
  }
  for k: 0..<t.nin {
    spk := prev[spent[k].script..<spent[k].script+spent[k].script_n];
    type := pol_solver_vartime(spk).type;
    if type == POL_NONSTANDARD { return pol_inputs_refused(POL_IN_UNKNOWN, k, 0, 0); }
    if type == POL_WITNESS_UNKNOWN { return pol_inputs_refused(POL_IN_UNDEFINED, k, 0, 0); }
    if type == POL_SCRIPTHASH {
      redeem: [SCRIPT_MAX_ELEMENT] u8;
      st := pol_stack_vartime(b[ins[k].script..<ins[k].script+ins[k].script_n], redeem[..]);
      if st.error != SCRIPT_ERR_OK { return pol_inputs_refused(POL_IN_MALFORMED, k, st.error, 0); }
      if st.depth == 0 { return pol_inputs_refused(POL_IN_MISSING, k, 0, 0); }
      count := sigop_count_vartime(redeem[0..<st.n], true);
      if count > POL_MAX_P2SH_SIGOPS { return pol_inputs_refused(POL_IN_P2SH_SIGOPS, k, count, POL_MAX_P2SH_SIGOPS); }
    }
  }
  return PolResult.{index=-1};
}
```

## Witness limits

`IsWitnessStandard` limits witnesses. A P2A output, the keyless anchor
that anyone may spend to bump a transaction's fee, may not carry a
witness: one there would only make the transaction heavier. A P2WSH
witness script may have at most 3,600 bytes, under at most 100 items of at
most 80 bytes each. A Taproot spend may not have an annex, which has no
meaning yet, and a tapscript spend's items may have at most 80 bytes:

<!-- source: coax/lib/bitcoin/policy.coax witness -->
```jai
// IsWitnessStandard, for each input with a witness: a P2A output may not
// have one (witness stuffing); under P2SH the redeem script takes the
// output's place; the script must be a witness program. A P2WSH witness
// script may have at most 3,600 bytes, and the items under it at most 100
// of at most 80 bytes each. A Taproot spend, not under P2SH, may not have
// an annex, and a tapscript spend's items under the script and the
// control block at most 80 bytes each.
pol_witness_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, items: [] TxItem, prev: [] u8, spent: [] SigopSpent) -> PolResult #export_c {
  if tx_coinbase_vartime(b, t, ins) { return PolResult.{index=-1}; }
  for k: 0..<t.nin {
    count := ins[k].count;
    if count == 0 { continue; }
    first := ins[k].first;
    spk := prev[spent[k].script..<spent[k].script+spent[k].script_n];
    if pol_anchor_vartime(spk) { return pol_refused(POL_WITNESS, k); }
    redeem: [SCRIPT_MAX_ELEMENT] u8;
    script := spk;
    p2sh := false;
    if script_p2sh_vartime(spk) {
      st := pol_stack_vartime(b[ins[k].script..<ins[k].script+ins[k].script_n], redeem[..]);
      if st.error != SCRIPT_ERR_OK || st.depth == 0 { return pol_refused(POL_WITNESS, k); }
      script = redeem[0..<st.n];
      p2sh = true;
    }
    version := script_witness_version_vartime(script);
    if version < 0 { return pol_refused(POL_WITNESS, k); }
    n := script.count - 2;
    if version == 0 && n == 32 {
      if items[first+count-1].n > POL_MAX_P2WSH_SCRIPT { return pol_refused(POL_WITNESS, k); }
      if count - 1 > POL_MAX_P2WSH_ITEMS { return pol_refused(POL_WITNESS, k); }
      for j: 0..<count-1 { if items[first+j].n > POL_MAX_P2WSH_ITEM { return pol_refused(POL_WITNESS, k); } }
    }
    if version == 1 && n == 32 && !p2sh {
      top := items[first+count-1];
      if count >= 2 && top.n > 0 && b[top.at] == POL_ANNEX_TAG { return pol_refused(POL_WITNESS, k); }
      if count >= 2 {
        if top.n == 0 { return pol_refused(POL_WITNESS, k); }    // the control block
        if (cast(int) b[top.at] & POL_LEAF_MASK) == POL_LEAF_TAPSCRIPT {
          for j: 0..<count-2 { if items[first+j].n > POL_MAX_TAPSCRIPT_ITEM { return pol_refused(POL_WITNESS, k); } }
        }
      }
    }
  }
  return PolResult.{index=-1};
}
```

## The order

Core's `MemPoolAccept::PreChecks` runs the rules in this order, with
CheckTransaction before them and, in between, the checks that need the
chain: finality, the spent outputs, the fee. Those belong to chapter 82;
`pol_check_vartime` runs the standardness steps alone. The last one
caps the sigop cost at 16,000, a fifth of a block's:

<!-- source: coax/lib/bitcoin/policy.coax check -->
```jai
// The standardness steps of PreChecks, in its order. prev holds the spent
// outputs' scriptPubKeys, spent[k] the one input k spends.
pol_check_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, prev: [] u8, spent: [] SigopSpent, o: PolOptions) -> PolResult #export_c {
  if t.code != TX_OK || spent.count != t.nin { return pol_refused(POL_CONTEXT, -1); }
  r := pol_standard_tx_vartime(b, t, ins, outs, o);
  if r.code != POL_OK { return r; }
  if t.stripped < POL_MIN_NONWITNESS { return pol_refused(POL_TX_SIZE_SMALL, -1); }
  r = pol_inputs_vartime(b, t, ins, prev, spent);
  if r.code != POL_OK { return r; }
  r = pol_witness_vartime(b, t, ins, items, prev, spent);
  if r.code != POL_OK { return r; }
  cost := sigop_cost_vartime(b, t, ins, outs, items, prev, spent, POL_CONSENSUS_FLAGS);
  if cost > POL_MAX_SIGOPS_COST { return PolResult.{code=POL_SIGOPS, index=-1, a=cost}; }
  return PolResult.{index=-1, a=cost};
}
```

## Still valid in a block

A refusal by policy says nothing about validity. `pol_consensus_vartime`
checks a transaction as a block would: chapter 52's `CheckTransaction`,
that the inputs pay for the outputs, each input's script under the
consensus flags with chapter 36's interpreter, and chapter 53's weight
and sigop cost:

<!-- source: coax/lib/bitcoin/policy.coax consensus -->
```jai
// What a block would check of the same transaction: CheckTransaction
// (check, chapter 52's code), that the inputs pay for the outputs, each
// input's VerifyScript under the consensus flags (input, script: the
// first that fails, chapter 36's code), and the weight and sigop cost a
// block may hold. valid when all of it passes.
PolConsensus :: struct { valid: bool; check: int; funded: bool; input: int; script: int; weight: int; cost: int; }
pol_consensus_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, prev: [] u8, spent: [] SigopSpent, w: [] ScriptWork) -> PolConsensus #export_c {
  c := PolConsensus.{check=txcheck_vartime(b, t, ins, outs), input=-1};
  if c.check != TXC_OK || spent.count != t.nin { return c; }
  value_in := 0; value_out := 0;
  for k: 0..<t.nin { value_in += spent[k].value; }
  for k: 0..<t.nout { value_out += tx_le_vartime(b, outs[k].value, 8); }
  c.funded = value_in >= value_out;
  for k: 0..<t.nin {
    sp := ScriptSpend.{b=b, t=t, ins=ins[0..<t.nin], outs=outs[0..<t.nout], items=items[0..<t.nitems],
      index=k, amount=spent[k].value, spk=prev[spent[k].script..<spent[k].script+spent[k].script_n]};
    e := script_verify_vartime(sp, POL_CONSENSUS_FLAGS, w);
    if e != SCRIPT_ERR_OK && c.input < 0 { c.input = k; c.script = e; }
  }
  c.weight = tx_weight_vartime(t);
  c.cost = sigop_cost_vartime(b, t, ins, outs, items, prev, spent, POL_CONSENSUS_FLAGS);
  c.valid = c.funded && c.input < 0 && c.weight <= FEE_MAX_BLOCK_WEIGHT && c.cost <= BLOCK_MAX_SIGOPS_COST;
  return c;
}
```

The example's transactions spend outputs that need no signature: P2WSH
and P2SH outputs whose script is `OP_1`, a bare `OP_1`, a version 2
program and a P2A output. Each one breaks one rule:

<!-- source: coax/lib/bitcoin/policy.coax cases -->
```jai
// The example's transactions, one per rule, by number: what each is, the
// options it runs under, and the reason Core gives it. Each one spends
// outputs of FUNDING worth POL_COIN each: case k spends output k, and
// the case of 167 inputs outputs 0 to 166.
POL_COIN :: 1000000;
POL_CASES :: 21;
POL_CASE_NAMES :: [21] string.["a P2WSH spend to P2WPKH", "version 0", "version 4",
  "an OP_RETURN of 99,990 bytes", "a scriptSig of 2,014 bytes", "a scriptSig with OP_NOP",
  "an output of OP_1", "a 2-of-4 bare multisig output", "an 84-byte OP_RETURN, -datacarriersize=83",
  "a 1-of-1 bare multisig output, -permitbaremultisig=0", "two dust outputs", "61 bytes without the witness",
  "a spend of a bare OP_1", "a spend of a version 2 program", "a redeem script with 16 sigops",
  "167 redeem scripts with 15 sigops each", "an 81-byte P2WSH item", "101 P2WSH items",
  "a P2WSH script of 3,631 bytes", "a witness on a P2A output", "201 bare multisig outputs"];
POL_CASE_CODES :: [21] int.[POL_OK, POL_VERSION, POL_VERSION, POL_TX_SIZE, POL_SCRIPTSIG_SIZE,
  POL_SCRIPTSIG_PUSHONLY, POL_SCRIPTPUBKEY, POL_SCRIPTPUBKEY, POL_DATACARRIER, POL_BARE_MULTISIG,
  POL_DUST, POL_TX_SIZE_SMALL, POL_INPUTS, POL_INPUTS, POL_INPUTS, POL_INPUTS, POL_WITNESS,
  POL_WITNESS, POL_WITNESS, POL_WITNESS, POL_SIGOPS];
// The options case k runs under.
pol_case_options :: (k: int) -> PolOptions {
  o := pol_defaults();
  if k == 8 { o.datacarrier = 83; }
  if k == 9 { o.bare_multisig = false; }
  return o;
}
```

They are written field by field:

<!-- source: coax/lib/bitcoin/policy.coax build -->
```jai
// A transaction written field by field: the version, BIP-144's marker
// and flag when segwit, the input count; each input spends output index
// of the funding transaction FUNDING (32 bytes of 0x11) with scriptSig
// sig; the output count and outputs; one witness stack per input when
// segwit; a locktime of 0. Each returns where writing goes on.
POL_FUNDING :: 0x11;
pol_le :: (out: [] u8, at: int, value: int, width: int) -> int {
  for i: 0..<width { out[at+i] = cast(u8) ((value >> (8 * i)) & 255); }
  return at + width;
}
pol_start :: (out: [] u8, version: int, segwit: bool, nin: int) -> int {
  at := pol_le(out, 0, version, 4);
  if segwit { out[at] = 0; out[at+1] = 1; at += 2; }
  return tx_compact_write_vartime(out, at, nin);
}
pol_input :: (out: [] u8, at: int, index: int, sig: [] u8) -> int {
  for i: 0..<32 { out[at+i] = POL_FUNDING; }
  a := pol_le(out, at + 32, index, 4);
  a = tx_compact_write_vartime(out, a, sig.count);
  a = tx_put(out, a, sig);
  return pol_le(out, a, 0xffffffff, 4);
}
pol_output :: (out: [] u8, at: int, value: int, spk: [] u8) -> int {
  a := pol_le(out, at, value, 8);
  a = tx_compact_write_vartime(out, a, spk.count);
  return tx_put(out, a, spk);
}
// A push of n copies of byte, in the shortest form, as CScript << does.
pol_push :: (out: [] u8, at: int, byte: int, n: int) -> int {
  a := at;
  if n < OP_PUSHDATA1 { out[a] = cast(u8) n; a += 1; }
  else if n <= 0xff { out[a] = OP_PUSHDATA1; out[a+1] = cast(u8) n; a += 2; }
  else if n <= 0xffff { out[a] = OP_PUSHDATA2; a = pol_le(out, a + 1, n, 2); }
  else { out[a] = OP_PUSHDATA4; a = pol_le(out, a + 1, n, 4); }
  for i: 0..<n { out[a+i] = cast(u8) byte; }
  return a + n;
}
// One spent output: its value, and its scriptPubKey appended to prev at at.
pol_spent :: (prev: [] u8, at: int, spent: [] SigopSpent, k: int, value: int, spk: [] u8) -> int {
  spent[k] = SigopSpent.{value=value, script=at, script_n=spk.count};
  return tx_put(prev, at, spk);
}
```

<!-- source: coax/lib/bitcoin/policy.coax run -->
```jai
// Case k checked both ways: the policy result under its options, and
// what a block checks; and its wtxid.
PolRun :: struct { r: PolResult; c: PolConsensus; stripped: int; nin: int; nout: int; wtxid: [32] u8; }
pol_run :: (k: int, w: [] ScriptWork) -> PolRun {
  out: [110000] u8; prev: [10000] u8; spent: [200] SigopSpent;
  ins: [200] TxIn; outs: [300] TxOut; items: [200] TxItem;
  c := pol_case(k, out[..], prev[..], spent[..]);
  b := out[0..<c.n];
  t := tx_read_vartime(b, true, ins[..], outs[..], items[..]);
  r := pol_check_vartime(b, t, ins[..], outs[..], items[..], prev[..], spent[0..<c.nspent], pol_case_options(k));
  cs := pol_consensus_vartime(b, t, ins[..], outs[..], items[..], prev[..], spent[0..<c.nspent], w);
  run := PolRun.{r=r, c=cs, stripped=t.stripped, nin=t.nin, nout=t.nout};
  #assert tx_wtxid_vartime(b, t, run.wtxid[..]);
  return run;
}
```

Each transaction prints its wtxid, the reason Core refuses it, and what a
block finds. The first is standard; it shows the shape the others change:

```text
one transaction per rule: the reason Bitcoin Core refuses it, then what a block checks
  607610f85e264ea0 a P2WSH spend to P2WPKH
    policy: standard
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
```

The next eleven break `IsStandardTx`'s rules and the size floor: a
version, the weight, a scriptSig too long or not push-only, an output of a
nonstandard type, an `OP_RETURN` over its budget, bare multisig when the
node refuses it, a second dust output, and a transaction refused only
because it is short:

```text
  7ae3c640ac98e883 version 0
    policy: version
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  2959e2b2b7efb6af version 4
    policy: version
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  13cfee8aacb8a646 an OP_RETURN of 99,990 bytes
    policy: tx-size
    a block: valid, 1 input, 1 output, weight 400245, sigop cost 0
  6c2e99a8aebbf567 a scriptSig of 2,014 bytes
    policy: scriptsig-size
    a block: valid, 1 input, 1 output, weight 8392, sigop cost 0
  dc6c8710a9e48165 a scriptSig with OP_NOP
    policy: scriptsig-not-pushonly
    a block: valid, 1 input, 1 output, weight 332, sigop cost 0
  9da298143dff767e an output of OP_1
    policy: scriptpubkey
    a block: valid, 1 input, 1 output, weight 249, sigop cost 0
  d328c011ea4b0968 a 2-of-4 bare multisig output
    policy: scriptpubkey
    a block: valid, 1 input, 1 output, weight 801, sigop cost 80
  c20116e96f5d078b an 84-byte OP_RETURN, -datacarriersize=83
    policy: datacarrier
    a block: valid, 1 input, 1 output, weight 581, sigop cost 0
  ee2635833afd921c a 1-of-1 bare multisig output, -permitbaremultisig=0
    policy: bare-multisig
    a block: valid, 1 input, 1 output, weight 393, sigop cost 80
  3066631dc42b7b4d two dust outputs
    policy: dust
    a block: valid, 1 input, 3 outputs, weight 581, sigop cost 0
  ea5c0c75ac0b3319 61 bytes without the witness
    policy: tx-size-small
    a block: valid, 1 input, 1 output, weight 249, sigop cost 0
```

The spent outputs and the witnesses:

```text
  7efcf278bc72e4dd a spend of a bare OP_1
    policy: bad-txns-nonstandard-inputs (input 0 script unknown)
    a block: valid, 1 input, 1 output, weight 328, sigop cost 0
  3dbc365a70179e34 a spend of a version 2 program
    policy: bad-txns-nonstandard-inputs (input 0 witness program is undefined)
    a block: valid, 1 input, 1 output, weight 328, sigop cost 0
  1a6331a31c3655d9 a redeem script with 16 sigops
    policy: bad-txns-nonstandard-inputs (p2sh redeemscript sigops exceed limit (input 0: 16 > 15))
    a block: valid, 1 input, 1 output, weight 592, sigop cost 64
  29938540986a52e4 167 redeem scripts with 15 sigops each
    policy: bad-txns-nonstandard-inputs (non-witness sigops exceed bip54 limit)
    a block: valid, 167 inputs, 1 output, weight 68968, sigop cost 10020
  49c38c05d10a9f11 an 81-byte P2WSH item
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 416, sigop cost 0
  d5fb10a36a464040 101 P2WSH items
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 535, sigop cost 0
  cb03544fcb791543 a P2WSH script of 3,631 bytes
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 3965, sigop cost 0
  f1f1ea4fc3d03bbe a witness on a P2A output
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  f9bb4a125ee70ffe 201 bare multisig outputs
    policy: bad-txns-too-many-sigops (16080)
    a block: valid, 1 input, 201 outputs, weight 37193, sigop cost 16080
21 of 21 transactions are valid in a block; 1 of them is standard
```

Every transaction is valid in a block. The two with 80 sigop cost carry a
bare multisig output, which counts 20 legacy sigops times 4 whether or not
anyone ever spends it; 201 of them pass the 16,000 standard limit while a
block holds 80,000.

## The example

[bitcoin/examples/standardness.coax](../examples/standardness.coax)
prints each output type with its dust threshold, then checks each
transaction both ways:

<!-- source: bitcoin/examples/standardness.coax example -->
```jai
#load "../../coax/lib/bitcoin/policy.coax";
#import "Basic";

hex_digit :: (x: int) -> string { return "0123456789abcdef"[x..<x + 1]; }
// The first 16 hex digits of a wtxid as explorers show it: bytes reversed.
show_wtxid :: (h: [] u8) {
  i := 31; while i >= 24 { print("%", hex_digit(cast(int) h[i] >> 4)); print("%", hex_digit(cast(int) h[i] & 15)); i -= 1; }
}

// ---- output types
// Each scriptPubKey's type as Solver reads it, whether IsStandard takes
// it, and the value below which it is dust at the default rate.
outputs :: () {
  print("output types, and dust at -dustrelayfee=3000 sat/kvB\n");
  names := [12] string.["P2PK, compressed key", "P2PK, uncompressed key", "P2PKH", "P2SH", "P2WPKH", "P2WSH",
    "P2TR", "a version 2 program", "P2A", "OP_RETURN", "bare 2-of-4 multisig", "OP_1"];
  spks := [12] string.["210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798ac",
    "410479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8ac",
    "76a914751e76e8199196d454941c45d1b3a323f1433bd688ac", "a914da1745e9b549bd0bfa1a569971c77eba30cd5a4b87",
    "0014751e76e8199196d454941c45d1b3a323f1433bd6", "00204ae81572f06e1b88fd5ced7a1a000945432e83e1551e6f721ee9c00b8cc33260",
    "51200000000000000000000000000000000000000000000000000000000000000001", "5202beef", "51024e73", "6a0568656c6c6f",
    "52210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f8179854ae",
    "51"];
  for i: 0..<12 {
    b: [200] u8;
    s := tx_unhex(spks[i], b[..]);
    print("  %: % byte%, %", names[i], s.count, ifx s.count == 1 then "" else "s", POL_TYPE_NAMES[pol_solver_vartime(s).type]);
    if !pol_standard_output_vartime(s) { print(", not standard\n"); continue; }
    d := pol_dust_threshold_vartime(s, POL_DUST_RELAY_FEE);
    if d == 0 { print(", never dust\n"); } else { print(", dust below % sat\n", d); }
  }
}

// ---- the pinned transactions
// Core's debug message for "bad-txns-nonstandard-inputs".
inputs_message :: (r: PolResult) {
  if r.detail == POL_IN_BIP54 { print(" (non-witness sigops exceed bip54 limit)"); }
  if r.detail == POL_IN_UNKNOWN { print(" (input % script unknown)", r.index); }
  if r.detail == POL_IN_UNDEFINED { print(" (input % witness program is undefined)", r.index); }
  if r.detail == POL_IN_P2SH_SIGOPS { print(" (p2sh redeemscript sigops exceed limit (input %: % > %))", r.index, r.a, r.b); }
}
cases :: () {
  print("one transaction per rule: the reason Bitcoin Core refuses it, then what a block checks\n");
  w: [1] ScriptWork;
  valid := 0; standard := 0;
  for k: 0..<POL_CASES {
    run := pol_run(k, w[..]);
    print("  "); show_wtxid(run.wtxid[..]); print(" %\n", POL_CASE_NAMES[k]);
    print("    policy: %", POL_REASONS[run.r.code]);
    if run.r.code == POL_INPUTS { inputs_message(run.r); }
    if run.r.code == POL_SIGOPS { print(" (%)", run.r.a); }
    print("\n");
    c := run.c;
    print("    a block: %, % input%, % output%, weight %, sigop cost %\n", ifx c.valid then "valid" else "invalid",
      run.nin, ifx run.nin == 1 then "" else "s", run.nout, ifx run.nout == 1 then "" else "s", c.weight, c.cost);
    if c.valid { valid += 1; }
    if run.r.code == POL_OK { standard += 1; }
  }
  print("% of % transactions are valid in a block; % of them is standard\n", valid, POL_CASES, standard);
}

main :: () { outputs(); cases(); }
```

```shell
coax/bin/coax run bitcoin/examples/standardness.coax
```

```text
output types, and dust at -dustrelayfee=3000 sat/kvB
  P2PK, compressed key: 35 bytes, pubkey, dust below 576 sat
  P2PK, uncompressed key: 67 bytes, pubkey, dust below 672 sat
  P2PKH: 25 bytes, pubkeyhash, dust below 546 sat
  P2SH: 23 bytes, scripthash, dust below 540 sat
  P2WPKH: 22 bytes, witness_v0_keyhash, dust below 294 sat
  P2WSH: 34 bytes, witness_v0_scripthash, dust below 330 sat
  P2TR: 34 bytes, witness_v1_taproot, dust below 330 sat
  a version 2 program: 4 bytes, witness_unknown, dust below 240 sat
  P2A: 4 bytes, anchor, dust below 240 sat
  OP_RETURN: 7 bytes, nulldata, never dust
  bare 2-of-4 multisig: 139 bytes, multisig, not standard
  OP_1: 1 byte, nonstandard, not standard
one transaction per rule: the reason Bitcoin Core refuses it, then what a block checks
  607610f85e264ea0 a P2WSH spend to P2WPKH
    policy: standard
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  7ae3c640ac98e883 version 0
    policy: version
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  2959e2b2b7efb6af version 4
    policy: version
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  13cfee8aacb8a646 an OP_RETURN of 99,990 bytes
    policy: tx-size
    a block: valid, 1 input, 1 output, weight 400245, sigop cost 0
  6c2e99a8aebbf567 a scriptSig of 2,014 bytes
    policy: scriptsig-size
    a block: valid, 1 input, 1 output, weight 8392, sigop cost 0
  dc6c8710a9e48165 a scriptSig with OP_NOP
    policy: scriptsig-not-pushonly
    a block: valid, 1 input, 1 output, weight 332, sigop cost 0
  9da298143dff767e an output of OP_1
    policy: scriptpubkey
    a block: valid, 1 input, 1 output, weight 249, sigop cost 0
  d328c011ea4b0968 a 2-of-4 bare multisig output
    policy: scriptpubkey
    a block: valid, 1 input, 1 output, weight 801, sigop cost 80
  c20116e96f5d078b an 84-byte OP_RETURN, -datacarriersize=83
    policy: datacarrier
    a block: valid, 1 input, 1 output, weight 581, sigop cost 0
  ee2635833afd921c a 1-of-1 bare multisig output, -permitbaremultisig=0
    policy: bare-multisig
    a block: valid, 1 input, 1 output, weight 393, sigop cost 80
  3066631dc42b7b4d two dust outputs
    policy: dust
    a block: valid, 1 input, 3 outputs, weight 581, sigop cost 0
  ea5c0c75ac0b3319 61 bytes without the witness
    policy: tx-size-small
    a block: valid, 1 input, 1 output, weight 249, sigop cost 0
  7efcf278bc72e4dd a spend of a bare OP_1
    policy: bad-txns-nonstandard-inputs (input 0 script unknown)
    a block: valid, 1 input, 1 output, weight 328, sigop cost 0
  3dbc365a70179e34 a spend of a version 2 program
    policy: bad-txns-nonstandard-inputs (input 0 witness program is undefined)
    a block: valid, 1 input, 1 output, weight 328, sigop cost 0
  1a6331a31c3655d9 a redeem script with 16 sigops
    policy: bad-txns-nonstandard-inputs (p2sh redeemscript sigops exceed limit (input 0: 16 > 15))
    a block: valid, 1 input, 1 output, weight 592, sigop cost 64
  29938540986a52e4 167 redeem scripts with 15 sigops each
    policy: bad-txns-nonstandard-inputs (non-witness sigops exceed bip54 limit)
    a block: valid, 167 inputs, 1 output, weight 68968, sigop cost 10020
  49c38c05d10a9f11 an 81-byte P2WSH item
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 416, sigop cost 0
  d5fb10a36a464040 101 P2WSH items
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 535, sigop cost 0
  cb03544fcb791543 a P2WSH script of 3,631 bytes
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 3965, sigop cost 0
  f1f1ea4fc3d03bbe a witness on a P2A output
    policy: bad-witness-nonstandard
    a block: valid, 1 input, 1 output, weight 333, sigop cost 0
  f9bb4a125ee70ffe 201 bare multisig outputs
    policy: bad-txns-too-many-sigops (16080)
    a block: valid, 1 input, 201 outputs, weight 37193, sigop cost 16080
21 of 21 transactions are valid in a block; 1 of them is standard
```

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/standardness.coax](../examples/checks/standardness.coax),
which loads the example. For 240 seeds a P2WSH spend gets a version, an
output value, an `OP_RETURN` size and a `-datacarriersize`. The policy
must give the first reason that applies in `IsStandardTx`'s order, and a
block must take every one of the transactions:

<!-- source: bitcoin/examples/checks/standardness.coax check -->
```jai
// Policy only adds refusals. A P2WSH spend pays a P2WPKH output, one of
// 0 sat (dust, which one output may be) and an OP_RETURN, with the
// version, the first output's value, the OP_RETURN's data and
// -datacarriersize drawn from the seed. The policy answers with the first
// of IsStandardTx's rules that applies, in its order, and a block takes
// every one of these transactions.
#forall "policy refuses in IsStandardTx's order, and a block takes every one" seed: 0..<240 #steps 4000000000 {
  version := seed % 6;                       // 0 to 5: 1, 2 and 3 are standard
  value := (seed * 7919) % 700;              // dust below 294
  data := (seed * 104729) % 120;             // OP_RETURN data bytes
  o := pol_defaults();
  o.datacarrier = (seed * 31) % 130;
  out: [600] u8; prev: [40] u8; spent: [1] SigopSpent; s: [200] u8; h: [40] u8;
  ins: [1] TxIn; outs: [3] TxOut; items: [1] TxItem;
  p := pol_spent(prev[..], 0, spent[..], 0, POL_COIN, tx_unhex(POL_P2WSH_TRUE, h[..]));
  at := pol_start(out[..], version, true, 1);
  at = pol_input(out[..], at, 0, h[0..<0]);
  at = tx_compact_write_vartime(out[..], at, 3);
  at = pol_output(out[..], at, value, tx_unhex(POL_P2WPKH, h[..]));
  at = pol_output(out[..], at, 0, tx_unhex(POL_P2WPKH, h[..]));
  s[0] = OP_RETURN;
  ret := s[0..<pol_push(s[..], 1, 0x6f, data)];
  at = pol_output(out[..], at, 0, ret);
  at = tx_put(out[..], at, tx_unhex("010151", h[..]));
  at = pol_le(out[..], at, 0, 4);
  t := tx_read_vartime(out[0..<at], true, ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && p == 34;
  want := POL_OK;
  if version < 1 || version > 3 { want = POL_VERSION; }
  else if ret.count > o.datacarrier { want = POL_DATACARRIER; }
  else if value < 294 { want = POL_DUST; }
  #assert pol_check_vartime(out[0..<at], t, ins[..], outs[..], items[..], prev[..], spent[..], o).code == want;
  w: [1] ScriptWork;
  #assert pol_consensus_vartime(out[0..<at], t, ins[..], outs[..], items[..], prev[..], spent[..], w[..]).valid;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/standardness.coax
```

`coax test` runs 312 checks, including the loaded files' own checks, in about 30 seconds on 2026-11-03.

### The library's own checks

`policy.coax` is an `#authority` file. Its checks port Core's
`transaction_tests.cpp`: `test_IsStandard`, with the versions, the dust
threshold at 3,000 and 3,702 sat/kvB, the `OP_RETURN` budgets, the
scriptSig sizes and each non-push opcode in a scriptSig, the 2,438-input
transaction of exactly 400,000 weight and its 400,004-weight neighbor,
bare multisig, and every output type's threshold; and
`max_standard_legacy_sigops`, where 166 inputs of 15 sigops pass and 167
refuse. They also check `Solver` on shapes that almost match, the stack a
scriptSig leaves and each of its errors, each of
`ValidateInputsStandardness`'s messages, the P2WSH, nested P2WSH and
Taproot witness limits, the example's transactions, and a refusal of each
kind by `pol_consensus_vartime`.

```shell
coax/bin/coax test coax/lib/bitcoin/policy.coax
```

`coax test` runs 311 checks, including those of the files it loads, in about 30 seconds on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[policy.py](../../coax/tests/bitcoin/policy.py) builds the example's
transactions again and transliterates `Solver` from `solver.cpp`,
`GetDustThreshold`, `IsStandardTx`, `ValidateInputsStandardness` and
`IsWitnessStandard` from `policy.cpp`, the order of `PreChecks` from
`validation.cpp`, and the sigop cost. For the block, it checks
`CheckTransaction`'s rules and the value, and runs a small interpreter of
the opcodes these transactions use. It reads the limits from the pinned
`policy.h`, checks that each reason it prints is in the pinned
`policy.cpp`, and writes the transcript the example must print, wtxids
included.

```shell
python3 -I coax/tests/bitcoin/policy.py --coax coax/bin/coax
```

On 2026-11-03 all 78 lines agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- The script flags that policy adds to consensus, such as `LOW_S`,
  `CLEANSTACK` and `NULLFAIL`: chapter 36 implements them, and chapter 82
  runs a mempool transaction under them.
- The rest of mempool acceptance: finality, the spent outputs, fees,
  ephemeral dust, TRUC, replacement and package limits: chapter 82.
- Tapscript, which a Taproot spend's witness runs: chapter 44. The Taproot
  witness limits here are checked on their own, and none of the example's
  transactions spends a Taproot output.

## Summary

- Policy is each node's choice; consensus is everyone's. Standardness
  refuses transactions that blocks may still hold.
- `Solver` names an output's type; nonstandard outputs, and bare multisig
  over three keys, are refused.
- Dust is an output worth less than the fee to spend it at
  `-dustrelayfee`; one dust output is allowed per transaction.
- `IsStandardTx` checks the version, the weight, the scriptSigs, the
  outputs and dust, in that order.
- The spent outputs must be of a standard, defined type, P2SH redeem
  scripts may hold 15 sigops, and the whole transaction 2,500 legacy
  sigops.
- P2WSH and Taproot witnesses have size limits, and a P2A output may not
  have a witness.

## Dependencies

- [36. The script interpreter](ch036-the-script-interpreter.md)
- [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md)
- [53. Sigops, weight and block limits](ch053-sigops-weight-and-block-limits.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `policy/policy.cpp` and
  `policy/policy.h`, `script/solver.cpp`, `validation.cpp`
  (`MemPoolAccept::PreChecks`), `script/script.cpp` and `script/script.h`,
  and `test/transaction_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/policy.cpp).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
