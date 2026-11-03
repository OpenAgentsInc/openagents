# 36. The script interpreter

Every coin is locked by a script, and spending it means running a second
script that unlocks it. This chapter builds the interpreter that runs
them: Bitcoin Core's `EvalScript` and `VerifyScript` for legacy script,
P2SH and segwit version 0, with every opcode, every limit and every
verification flag Core has for them. Each failure is one of Core's
`ScriptError` codes, with Core's number.

It runs all 1,256 runnable rows of Bitcoin Core's `script_tests.json`
under their flags, every input of the 206 script transactions in
`tx_valid.json` and `tx_invalid.json` under 4,532 flag sets, and checks
the interpreter against a Python transliteration of Core, natively and in
Wasm. Tapscript, the script of Taproot's script path, is chapter 44; here
a version 1 witness program under the `TAPROOT` flag refuses rather than
runs.

## What this chapter teaches

- Script as a stack machine over byte strings: how an opcode is read, what
  a push is, and how an element's bytes become a number or a boolean.
- CScriptNum: little-endian sign and magnitude, at most 4 bytes as an
  operand, and the minimal-encoding rule.
- The condition stack of `IF`, `NOTIF`, `ELSE` and `ENDIF`, and the
  opcodes that fail even in a branch that doesn't run.
- `CHECKSIG` and `CHECKMULTISIG`: the scriptCode, `OP_CODESEPARATOR`,
  FindAndDelete, the extra element `CHECKMULTISIG` pops, and the signature
  and key encoding checks.
- The limits: 10,000-byte scripts, 520-byte elements, 201 opcodes, 1,000
  elements on the two stacks, 20 keys.
- `VerifyScript`: the scriptSig and the scriptPubKey on one stack, P2SH's
  redeem script, witness programs (P2WPKH and P2WSH) and the flags that
  tighten each step.
- Which flags are consensus and which are policy, and why every flag can
  only refuse more.

## Where Bitcoin uses the interpreter

Every input of every transaction runs it. Core calls `VerifyScript` from
`CheckInputScripts` with the consensus flags when it validates a block,
and with the stricter policy flags when it accepts a transaction into its
mempool.

| Where | What runs | Chapter |
| --- | --- | --- |
| Block validation | every input, under the consensus flags of the block's height | 55, 57 |
| The mempool | every input, under the standard (policy) flags | 81, 82 |
| A wallet signing a transaction | the input it signed, to check the result | 94, 99 |
| Lightning | the funding output's 2-of-2 and the commitment and HTLC scripts | 106 to 110 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_op_vartime(s, pc)` | Core's `GetScriptOp`: one opcode and its push |
| | `script_push_only_vartime`, `script_minimal_push_vartime` | `IsPushOnly` and `CheckMinimalPush` |
| | `script_num_vartime`, `script_num_write_vartime`, `script_bool_vartime` | CScriptNum and `CastToBool` |
| | `script_sighash_legacy_vartime`, `script_sighash_v0_vartime` | the two signature hashes `CHECKSIG` signs |
| | `script_witness_version_vartime`, `script_p2sh_vartime` | `IsWitnessProgram` and `IsPayToScriptHash` |
| | `script_verify_vartime(sp, flags, w)` | `VerifyScript` for one input |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_read_vartime` | the spending transaction (chapter 34) |
| [der.coax](../../coax/lib/bitcoin/der.coax) | `der_signature_check_vartime`, `der_read_lax_vartime` | the signature encoding checks and the lax parser (chapter 21) |
| [ecdsa_verify.coax](../../coax/lib/crypto/ecdsa_verify.coax) | `ecdsa_verify_vartime` | the verification (chapter 19) |
| [hash160.coax](../../coax/lib/bitcoin/hash160.coax), [sha1.coax](../../coax/lib/crypto/sha1.coax) | `hash160_checked`, `sha1_hash` | the hash opcodes (chapter 6) |

Scripts, stacks and transactions are public, so the interpreter runs in
variable time and every procedure that decides ends in `_vartime`
(chapter 3).

### The results

A result is `SCRIPT_ERR_OK` or a failure, numbered as in Core's
`script_error.h`. Codes 35 to 37 and 45 to 52 are Taproot's and
tapscript's; this chapter never returns them. Two results are not Core's:

<!-- source: coax/lib/bitcoin/script.coax errors -->
```jai
// Bitcoin Core's ScriptError (script/script_error.h), in its order, so
// each code is Core's number.
SCRIPT_ERR_OK :: 0;
SCRIPT_ERR_UNKNOWN_ERROR :: 1;
SCRIPT_ERR_EVAL_FALSE :: 2;
SCRIPT_ERR_OP_RETURN :: 3;
SCRIPT_ERR_SCRIPTNUM :: 4;
SCRIPT_ERR_SCRIPT_SIZE :: 5;
SCRIPT_ERR_PUSH_SIZE :: 6;
SCRIPT_ERR_OP_COUNT :: 7;
SCRIPT_ERR_STACK_SIZE :: 8;
SCRIPT_ERR_SIG_COUNT :: 9;
SCRIPT_ERR_PUBKEY_COUNT :: 10;
SCRIPT_ERR_VERIFY :: 11;
SCRIPT_ERR_EQUALVERIFY :: 12;
SCRIPT_ERR_CHECKMULTISIGVERIFY :: 13;
SCRIPT_ERR_CHECKSIGVERIFY :: 14;
SCRIPT_ERR_NUMEQUALVERIFY :: 15;
SCRIPT_ERR_BAD_OPCODE :: 16;
SCRIPT_ERR_DISABLED_OPCODE :: 17;
SCRIPT_ERR_INVALID_STACK_OPERATION :: 18;
SCRIPT_ERR_INVALID_ALTSTACK_OPERATION :: 19;
SCRIPT_ERR_UNBALANCED_CONDITIONAL :: 20;
SCRIPT_ERR_NEGATIVE_LOCKTIME :: 21;
SCRIPT_ERR_UNSATISFIED_LOCKTIME :: 22;
SCRIPT_ERR_SIG_HASHTYPE :: 23;
SCRIPT_ERR_SIG_DER :: 24;
SCRIPT_ERR_MINIMALDATA :: 25;
SCRIPT_ERR_SIG_PUSHONLY :: 26;
SCRIPT_ERR_SIG_HIGH_S :: 27;
SCRIPT_ERR_SIG_NULLDUMMY :: 28;
SCRIPT_ERR_PUBKEYTYPE :: 29;
SCRIPT_ERR_CLEANSTACK :: 30;
SCRIPT_ERR_MINIMALIF :: 31;
SCRIPT_ERR_SIG_NULLFAIL :: 32;
SCRIPT_ERR_DISCOURAGE_UPGRADABLE_NOPS :: 33;
SCRIPT_ERR_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM :: 34;
SCRIPT_ERR_WITNESS_PROGRAM_WRONG_LENGTH :: 38;     // 35 to 37: Taproot's DISCOURAGE codes
SCRIPT_ERR_WITNESS_PROGRAM_WITNESS_EMPTY :: 39;
SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH :: 40;
SCRIPT_ERR_WITNESS_MALLEATED :: 41;
SCRIPT_ERR_WITNESS_MALLEATED_P2SH :: 42;
SCRIPT_ERR_WITNESS_UNEXPECTED :: 43;
SCRIPT_ERR_WITNESS_PUBKEYTYPE :: 44;
SCRIPT_ERR_OP_CODESEPARATOR :: 53;                 // 45 to 52: Taproot and tapscript
SCRIPT_ERR_SIG_FINDANDDELETE :: 54;
SCRIPT_ERR_COUNT :: 55;                            // SCRIPT_ERR_ERROR_COUNT
// Not Core's: a v1 witness program under TAPROOT (chapters 43 and 44), and
// a flag set Core's VerifyScript asserts against (CLEANSTACK without
// WITNESS and P2SH, or WITNESS without P2SH).
SCRIPT_TAPROOT_UNBUILT :: 100;
SCRIPT_BAD_FLAGS :: 101;
```

`SCRIPT_BAD_FLAGS` stands for an assertion. Core's `VerifyScript` asserts
that `CLEANSTACK` comes with `WITNESS` and `WITNESS` with `P2SH`, and its
tests never break that rule. A caller that breaks it here gets a result
instead of a crash.

### The flags

Each flag turns on one rule. The consensus flags are soft forks: `P2SH`
(BIP-16), `DERSIG` (BIP-66), `CHECKLOCKTIMEVERIFY` (BIP-65),
`CHECKSEQUENCEVERIFY` (BIP-112), `WITNESS` (BIP-141), `NULLDUMMY`
(BIP-147) and `TAPROOT` (BIP-341). The rest are policy: Core's mempool
refuses what they refuse, and a block may still contain it.

<!-- source: coax/lib/bitcoin/script.coax flags -->
```jai
// The script verification flags (script/interpreter.h), one bit each in
// Core's order.
SCRIPT_VERIFY_P2SH :: 1 << 0;                 // BIP-16
SCRIPT_VERIFY_STRICTENC :: 1 << 1;            // policy: key and sighash shapes
SCRIPT_VERIFY_DERSIG :: 1 << 2;               // BIP-66
SCRIPT_VERIFY_LOW_S :: 1 << 3;                // policy
SCRIPT_VERIFY_NULLDUMMY :: 1 << 4;            // BIP-147
SCRIPT_VERIFY_SIGPUSHONLY :: 1 << 5;          // policy
SCRIPT_VERIFY_MINIMALDATA :: 1 << 6;          // policy
SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_NOPS :: 1 << 7;
SCRIPT_VERIFY_CLEANSTACK :: 1 << 8;           // policy
SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY :: 1 << 9;  // BIP-65
SCRIPT_VERIFY_CHECKSEQUENCEVERIFY :: 1 << 10; // BIP-112
SCRIPT_VERIFY_WITNESS :: 1 << 11;             // BIP-141
SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM :: 1 << 12;
SCRIPT_VERIFY_MINIMALIF :: 1 << 13;           // policy, witness v0
SCRIPT_VERIFY_NULLFAIL :: 1 << 14;            // policy
SCRIPT_VERIFY_WITNESS_PUBKEYTYPE :: 1 << 15;  // policy
SCRIPT_VERIFY_CONST_SCRIPTCODE :: 1 << 16;    // policy
SCRIPT_VERIFY_TAPROOT :: 1 << 17;             // BIP-341 and BIP-342
SCRIPT_VERIFY_FLAG_BITS :: 21;                // three more: Taproot's DISCOURAGE flags
```

A flag only adds refusals. A script that passes under a set of flags
passes under every subset of it, which is what lets an old node accept
every block a new one accepts. The harness checks that property on every
row that succeeds, with two seeded subsets of its flags.

## Reading a script

A script is a run of opcodes. Opcodes 1 to 75 push that many bytes;
`OP_PUSHDATA1`, `OP_PUSHDATA2` and `OP_PUSHDATA4` push a length given in
the next 1, 2 or 4 bytes, little-endian. Every other byte is an opcode
with no data. `GetScriptOp` reads one:

<!-- source: coax/lib/bitcoin/script.coax getop -->
```jai
// GetScriptOp at s[pc]: one opcode and, for a push, its data at s[at..<at+n].
// A push whose length bytes or data run past the end refuses, and next is
// where Core's iterator stopped: after the opcode, and after the length
// bytes when they were all there.
script_op_vartime :: (s: [] u8, pc: int) -> ScriptOp #export_c {
  if pc >= s.count { return ScriptOp.{next=pc, op=0xff}; }
  op := cast(int) s[pc]; at := pc + 1;
  if op > OP_PUSHDATA4 { return ScriptOp.{ok=true, op=op, at=at, next=at}; }
  width := ifx op < OP_PUSHDATA1 then 0 else ifx op == OP_PUSHDATA1 then 1 else ifx op == OP_PUSHDATA2 then 2 else 4;
  if width > s.count - at { return ScriptOp.{op=0xff, next=at}; }
  n := ifx width == 0 then op else tx_le_vartime(s, at, width);
  at += width;
  if n > s.count - at { return ScriptOp.{op=0xff, next=at}; }
  return ScriptOp.{ok=true, op=op, at=at, n=n, next=at+n};
}
```

A push that runs past the end of the script is not a push: the
interpreter fails with `BAD_OPCODE`. The signature hash, which also walks
the script, stops at the same place.

`IsPushOnly` asks whether a scriptSig only pushes; `SIGPUSHONLY` requires
it of every scriptSig, and P2SH of its own. `CheckMinimalPush` asks whether
a push used the shortest form, which `MINIMALDATA` requires:

<!-- source: coax/lib/bitcoin/script.coax pushonly -->
```jai
// IsPushOnly: every op parses and none is above OP_16. OP_RESERVED (0x50)
// counts as a push here; running it fails.
script_push_only_vartime :: (s: [] u8) -> bool #export_c {
  pc := 0;
  while pc < s.count {
    op := script_op_vartime(s, pc);
    if !op.ok || op.op > OP_16 { return false; }
    pc = op.next;
  }
  return true;
}
// CheckMinimalPush: the data was pushed by the shortest form: OP_0 for
// nothing, OP_1NEGATE and OP_1 to OP_16 for those single bytes, a direct
// push to 75 bytes, then PUSHDATA1, then PUSHDATA2.
script_minimal_push_vartime :: (data: [] u8, op: int) -> bool #export_c {
  n := data.count;
  if n == 0 { return op == OP_0; }
  if n == 1 && data[0] >= 1 && data[0] <= 16 { return false; }
  if n == 1 && data[0] == 0x81 { return false; }
  if n <= 75 { return op == n; }  // mutant: equivalent (at 76 bytes op == 76 is OP_PUSHDATA1, the next line's answer)
  if n <= 255 { return op == OP_PUSHDATA1; }
  if n <= 65535 { return op == OP_PUSHDATA2; }
  return true;
}
```

## Numbers and booleans

An element is bytes. The numeric opcodes read it as a CScriptNum: the
magnitude little-endian, with the top bit of the last byte as the sign.
So `0x81` is −1, `0xff00` is 255 and `0x80` is negative zero. An operand
has at most 4 bytes; `CHECKLOCKTIMEVERIFY` and `CHECKSEQUENCEVERIFY` take
5, so that they can compare against any 32-bit lock time. A result can be
5 bytes (`0x7fffffff 1ADD`), and no numeric opcode will read it back.

<!-- source: coax/lib/bitcoin/script.coax scriptnum -->
```jai
// CScriptNum from a stack element: little-endian, the top bit of the last
// byte the sign, at most max bytes (4, or 5 for the locktime opcodes).
// With minimal, the last byte must carry a bit other than the sign,
// unless the byte before it needs the sign bit to itself: so 0x80
// (negative zero) and 0x0100 refuse, and 0xff00 (255) does not.
script_num_vartime :: (b: [] u8, minimal: bool, max: int) -> ScriptNum #export_c {
  n := b.count;
  if n > max { return ScriptNum.{}; }
  if n == 0 { return ScriptNum.{ok=true}; }
  last := cast(int) b[n-1];
  if minimal && (last & 0x7f) == 0 && (n == 1 || (b[n-2] & 0x80) == 0) { return ScriptNum.{}; }
  value := 0;
  for i: 0..<n-1 { value |= cast(int) b[i] << (8 * i); }
  value |= (last & 0x7f) << (8 * (n - 1));
  return ScriptNum.{value=ifx (last & 0x80) != 0 then -value else value, ok=true};
}
// CScriptNum::serialize into out (9 bytes of room): the magnitude,
// little-endian, and the sign in the top bit, with one more byte when the
// top bit is taken. Zero is no bytes. Returns the length.
script_num_write_vartime :: (value: int, out: [] u8) -> int #export_c {
  magnitude := ifx value < 0 then -value else value;  // mutant: equivalent (the negation of zero is zero)
  n := 0;
  while magnitude > 0 { out[n] = cast(u8) (magnitude & 255); magnitude >>= 8; n += 1; }
  if n == 0 { return 0; }
  if (out[n-1] & 0x80) != 0 { out[n] = ifx value < 0 then 0x80 else 0; return n + 1; }  // mutant: equivalent (zero returned above)
  if value < 0 { out[n-1] |= 0x80; }  // mutant: equivalent (zero returned above)
  return n;
}
// CastToBool: any nonzero byte is true, except a last byte of 0x80 with
// zeros before it: negative zero is false.
script_bool_vartime :: (b: [] u8) -> bool #export_c {
  for i: 0..<b.count {
    if b[i] != 0 { return i != b.count - 1 || b[i] != 0x80; }
  }
  return false;
}
```

### Worked example: CScriptNum

The example writes seven values:

```text
CScriptNum
  0: (0 bytes)
  1: 01 (1 bytes)
  -1: 81 (1 bytes)
  127: 7f (1 bytes)
  128: 80 00 (2 bytes)
  -255: ff 80 (2 bytes)
  2147483648: 00 00 00 80 00 (5 bytes)
```

128 is `0x80` in one byte, but that byte's top bit is the sign, so the
number needs a second byte, `00`, to carry a positive sign. −255 is `ff`
with the sign in a second byte, `80`. 2³¹ needs five bytes, so it is a
valid result and an invalid operand.

## The condition stack

`IF` pops a value and opens a branch that runs when the value is true;
`NOTIF` runs it when the value is false. `ELSE` flips the innermost
branch, and `ENDIF` closes it. An opcode runs only when every open branch
is running. Core keeps only the size of the stack and the position of the
first false value, since nothing else changes what runs:

<!-- source: coax/lib/bitcoin/script.coax cond -->
```jai
// Core's ConditionStack: IF pushes a value, ELSE flips the top one, ENDIF
// pops it, and an opcode runs when every value is true. Only the first
// false value is observable, so the stack is its size and that position.
si_cond_push :: (c: ScriptCond, value: bool) -> ScriptCond {
  r := c;
  if r.first_false < 0 && !value { r.first_false = r.size; }
  r.size += 1;
  return r;
}
si_cond_pop :: (c: ScriptCond) -> ScriptCond {
  r := c;
  r.size -= 1;
  if r.first_false == r.size { r.first_false = -1; }
  return r;
}
si_cond_toggle :: (c: ScriptCond) -> ScriptCond {
  r := c;
  if r.first_false < 0 { r.first_false = r.size - 1; }
  else if r.first_false == r.size - 1 { r.first_false = -1; }
  return r;
}
```

Some checks happen on every opcode, running or not. A push longer than 520
bytes, an opcode past the 201st, a disabled opcode (`OP_CAT` and the other
14 turned off in 2010), and `VERIF` and `VERNOTIF` all fail even inside a
branch that is skipped. `OP_RESERVED`, `OP_VER` and the undefined opcodes
fail only when they run.

## The stack

An element is a view into bytes that already exist: the spending
transaction (scriptSig pushes and witness items), the spent scriptPubKey,
or an arena for the values opcodes compute. `DUP` copies the view, not the
bytes. The workspace holds the stack, the alt stack, a copy of the stack
for P2SH, the arena and the scriptCode:

<!-- source: coax/lib/bitcoin/script.coax work -->
```jai
// A stack element is a view: n bytes at at of one of three sources, the
// spending transaction's bytes (scriptSigs and witness items), the spent
// scriptPubKey, or the workspace's arena (values the opcodes compute).
// Copying an element copies the view, never the bytes.
SRC_TX :: 0; SRC_SPK :: 1; SRC_ARENA :: 2;
ScriptItem :: struct { src: int; at: int; n: int; }
// The input being checked (Core's checker): the spending transaction as
// chapter 34's tx_read_vartime read it, which input, the value and the
// scriptPubKey of the output it spends.
ScriptSpend :: struct {
  b: [] u8; t: Tx; ins: [] TxIn; outs: [] TxOut; items: [] TxItem;
  index: int; amount: int; spk: [] u8;
}
// Room for the stack: 1,000 elements and the three an opcode may add
// before the size check, or the top SCRIPT_ROOM of a larger witness stack
// with the rest counted in hidden.
SCRIPT_ROOM :: 1100;
SCRIPT_CAP :: 1104;
// Each run of EvalScript computes at most 201 values of at most 32 bytes,
// and VerifyScript runs it at most four times.
SCRIPT_ARENA :: 32768;
ScriptWork :: struct {
  stack: [SCRIPT_CAP] ScriptItem; depth: int; hidden: int;
  alt: [SCRIPT_CAP] ScriptItem; alt_depth: int;
  saved: [SCRIPT_CAP] ScriptItem; saved_depth: int;
  arena: [SCRIPT_ARENA] u8; used: int;
  code: [SCRIPT_MAX_SIZE] u8; code_n: int;   // the scriptCode a signature hash signs
}
```

The arena is enough: an `EvalScript` run computes at most 201 values (one
per counted opcode), the largest is a 32-byte hash, and `VerifyScript`
runs at most four scripts. A witness stack may hold more than 1,000
items, since Core checks the size only after the first opcode runs. The
workspace keeps the top 1,100 and counts the rest. If the script runs any
opcode, the first one fails `STACK_SIZE` either way.

## EvalScript

`EvalScript` reads each opcode, applies the checks that always apply, and
runs the opcode if its branch is running:

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
  if s.count > SCRIPT_MAX_SIZE { return SCRIPT_ERR_SCRIPT_SIZE; }
  w[0].alt_depth = 0;
  minimal := (flags & SCRIPT_VERIFY_MINIMALDATA) != 0;
  cond := ScriptCond.{first_false=-1};
  ops := 0; pc := 0; begin := 0;
  while pc < s.count {
    running := cond.first_false < 0;
    o := script_op_vartime(s, pc);
    if !o.ok { return SCRIPT_ERR_BAD_OPCODE; }
    pc = o.next; op := o.op;
    if o.n > SCRIPT_MAX_ELEMENT { return SCRIPT_ERR_PUSH_SIZE; }
    if op > OP_16 { ops += 1; if ops > SCRIPT_MAX_OPS { return SCRIPT_ERR_OP_COUNT; } }
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
      step := si_checkmultisig(sp, w, s, begin, op, flags, version, ops);
      error = step.error; ops += step.keys;
    } else if running {
      error = si_run(sp, w, s, begin, op, flags, version);
      if op == OP_CODESEPARATOR { begin = pc; }
    }
    if error != SCRIPT_ERR_OK { return error; }
    if si_size(w) + w[0].alt_depth > SCRIPT_MAX_STACK { return SCRIPT_ERR_STACK_SIZE; }
  }
  if cond.size != 0 { return SCRIPT_ERR_UNBALANCED_CONDITIONAL; }
  return SCRIPT_ERR_OK;
}
```

`MINIMALIF`, for witness v0 scripts, requires the value `IF` and `NOTIF`
pop to be empty or exactly `0x01`. Without it, a third party could change
`0x01` to `0x02` in a witness without breaking the script.

The opcodes that are not pushes, conditionals or `CHECKMULTISIG`:

<!-- source: coax/lib/bitcoin/script.coax run -->
```jai
// One opcode in a running branch, other than a push, a conditional and
// CHECKMULTISIG.
si_run :: (sp: ScriptSpend, w: [] ScriptWork, s: [] u8, begin: int, op: int, flags: int, version: int) -> int {
  minimal := (flags & SCRIPT_VERIFY_MINIMALDATA) != 0;
  if op == OP_1NEGATE || (op >= OP_1 && op <= OP_16) { si_push(w, si_constant(op - 0x50)); return SCRIPT_ERR_OK; }
  if op == OP_NOP || op == OP_CODESEPARATOR { return SCRIPT_ERR_OK; }
  // CLTV and CSV without their flags are the NOP2 and NOP3 they replaced.
  if op == OP_CHECKLOCKTIMEVERIFY {
    return ifx (flags & SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY) != 0 then si_lock_op(sp, w, op, minimal) else SCRIPT_ERR_OK;
  }
  if op == OP_CHECKSEQUENCEVERIFY {
    return ifx (flags & SCRIPT_VERIFY_CHECKSEQUENCEVERIFY) != 0 then si_lock_op(sp, w, op, minimal) else SCRIPT_ERR_OK;
  }
  if op >= OP_NOP1 && op <= OP_NOP10 {
    // NOP1 and NOP4 to NOP10: reserved for soft forks, so policy may
    // refuse them.
    return ifx (flags & SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_NOPS) != 0 then SCRIPT_ERR_DISCOURAGE_UPGRADABLE_NOPS else SCRIPT_ERR_OK;
  }
  if op == OP_VERIFY {
    if si_size(w) < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    if !script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { return SCRIPT_ERR_VERIFY; }
    si_pop(w, 1);
    return SCRIPT_ERR_OK;
  }
  if op == OP_RETURN { return SCRIPT_ERR_OP_RETURN; }
  if (op >= OP_TOALTSTACK && op <= OP_TUCK) { return si_stack_op(sp, w, op, minimal); }  // mutant: equivalent (0x7e is OP_CAT, refused as disabled before)
  if op == OP_SIZE {
    if si_size(w) < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    si_push(w, si_number(w, si_top(w, 1).n));
    return SCRIPT_ERR_OK;
  }
  if op == OP_EQUAL || op == OP_EQUALVERIFY {
    if si_size(w) < 2 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    equal := si_same(si_bytes(sp, w, si_top(w, 2)), si_bytes(sp, w, si_top(w, 1)));
    si_pop(w, 2);
    if op == OP_EQUALVERIFY { return ifx equal then SCRIPT_ERR_OK else SCRIPT_ERR_EQUALVERIFY; }
    si_push(w, si_bool(equal));
    return SCRIPT_ERR_OK;
  }
  if (op >= OP_1ADD && op <= OP_0NOTEQUAL && op != 0x8d && op != 0x8e) || (op >= OP_ADD && op <= OP_WITHIN) { return si_numeric(sp, w, op, minimal); }  // mutant: equivalent (0x8d and 0x8e are disabled, and 0x93 is OP_ADD)
  if op >= OP_RIPEMD160 && op <= OP_HASH256 { return si_hash(sp, w, op); }  // mutant: equivalent (0xab is OP_CODESEPARATOR, handled above)
  if op == OP_CHECKSIG || op == OP_CHECKSIGVERIFY { return si_checksig(sp, w, s, begin, op, flags, version); }
  return SCRIPT_ERR_BAD_OPCODE;      // OP_RESERVED, OP_VER, OP_RESERVED1/2, OP_CHECKSIGADD, 0xbb and up
}
```

The stack opcodes check the depth they need before they touch the stack:

<!-- source: coax/lib/bitcoin/script.coax stackops -->
```jai
// The stack opcodes. Each checks the depth it needs first; OP_PICK and
// OP_ROLL read their index as a 4-byte number.
si_stack_op :: (sp: ScriptSpend, w: [] ScriptWork, op: int, minimal: bool) -> int {
  size := si_size(w);
  if op == OP_TOALTSTACK {
    if size < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    w[0].alt[w[0].alt_depth] = si_top(w, 1); w[0].alt_depth += 1; si_pop(w, 1);
  } else if op == OP_FROMALTSTACK {
    if w[0].alt_depth < 1 { return SCRIPT_ERR_INVALID_ALTSTACK_OPERATION; }
    w[0].alt_depth -= 1; si_push(w, w[0].alt[w[0].alt_depth]);
  } else if op == OP_2DROP {
    if size < 2 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    si_pop(w, 2);
  } else if op == OP_2DUP || op == OP_3DUP || op == OP_2OVER {
    need := ifx op == OP_2DUP then 2 else ifx op == OP_3DUP then 3 else 4;
    if size < need { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    copies := ifx op == OP_3DUP then 3 else 2;
    for k: 0..<copies { si_push(w, si_top(w, need)); }
  } else if op == OP_2ROT {
    if size < 6 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    a := si_top(w, 6); b := si_top(w, 5);
    si_erase(w, 6); si_erase(w, 5);
    si_push(w, a); si_push(w, b);
  } else if op == OP_2SWAP {
    if size < 4 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    si_swap(w, 4, 2); si_swap(w, 3, 1);
  } else if op == OP_IFDUP {
    if size < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    if script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { si_push(w, si_top(w, 1)); }
  } else if op == OP_DEPTH {
    si_push(w, si_number(w, size));
  } else if op == OP_DROP || op == OP_DUP {
    if size < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    if op == OP_DROP { si_pop(w, 1); } else { si_push(w, si_top(w, 1)); }
  } else if op == OP_PICK || op == OP_ROLL {
    return si_pick(sp, w, op, minimal);
  } else {
    if size < (ifx op == OP_ROT then 3 else 2) { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
    if op == OP_NIP { si_erase(w, 2); }
    else if op == OP_OVER { si_push(w, si_top(w, 2)); }
    else if op == OP_ROT { si_swap(w, 3, 2); si_swap(w, 2, 1); }
    else if op == OP_SWAP { si_swap(w, 2, 1); }
    else {                                                   // OP_TUCK: (x1 x2 -- x2 x1 x2)
      top := si_top(w, 1);
      si_push(w, top); si_swap(w, 3, 2);
    }
  }
  return SCRIPT_ERR_OK;
}
si_swap :: (w: [] ScriptWork, i: int, j: int) {
  a := si_top(w, i);
  w[0].stack[w[0].depth-i] = si_top(w, j);
  w[0].stack[w[0].depth-j] = a;
}
// OP_PICK and OP_ROLL. An index can reach below the stored elements only
// in the first opcode of a witness script whose stack is larger than
// SCRIPT_ROOM; there the size check after the opcode fails whatever the
// element was, so the result is STACK_SIZE at once.
si_pick :: (sp: ScriptSpend, w: [] ScriptWork, op: int, minimal: bool) -> int {
  if si_size(w) < 2 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  n := script_num_vartime(si_bytes(sp, w, si_top(w, 1)), minimal, 4);
  if !n.ok { return SCRIPT_ERR_SCRIPTNUM; }
  si_pop(w, 1);
  if n.value < 0 || n.value >= si_size(w) { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  if n.value >= w[0].depth { return SCRIPT_ERR_STACK_SIZE; }
  el := si_top(w, n.value + 1);
  if op == OP_ROLL { si_erase(w, n.value + 1); }
  si_push(w, el);
  return SCRIPT_ERR_OK;
}
```

The numeric opcodes read 4-byte operands and write results of any
length:

<!-- source: coax/lib/bitcoin/script.coax numeric -->
```jai
// The numeric opcodes: operands are 4-byte CScriptNums, results are
// written back however long they are (OP_ADD can make 5 bytes, which no
// later numeric opcode accepts).
si_numeric :: (sp: ScriptSpend, w: [] ScriptWork, op: int, minimal: bool) -> int {
  arity := ifx op <= OP_0NOTEQUAL then 1 else ifx op == OP_WITHIN then 3 else 2;
  if si_size(w) < arity { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  v: [3] int;
  for k: 0..<arity {
    n := script_num_vartime(si_bytes(sp, w, si_top(w, arity - k)), minimal, 4);
    if !n.ok { return SCRIPT_ERR_SCRIPTNUM; }
    v[k] = n.value;
  }
  si_pop(w, arity);
  a := v[0]; b := v[1];
  if op == OP_WITHIN { si_push(w, si_bool(b <= a && a < v[2])); return SCRIPT_ERR_OK; }
  r := 0;  // mutant: equivalent (every branch assigns r)
  if op == OP_1ADD { r = a + 1; }
  else if op == OP_1SUB { r = a - 1; }
  else if op == OP_NEGATE { r = -a; }
  else if op == OP_ABS { r = ifx a < 0 then -a else a; }  // mutant: equivalent (the absolute value of zero is zero)
  else if op == OP_NOT { r = ifx a == 0 then 1 else 0; }
  else if op == OP_0NOTEQUAL { r = ifx a != 0 then 1 else 0; }
  else if op == OP_ADD { r = a + b; }
  else if op == OP_SUB { r = a - b; }
  else if op == OP_BOOLAND { r = ifx a != 0 && b != 0 then 1 else 0; }
  else if op == OP_BOOLOR { r = ifx a != 0 || b != 0 then 1 else 0; }
  else if op == OP_NUMEQUAL || op == OP_NUMEQUALVERIFY { r = ifx a == b then 1 else 0; }
  else if op == OP_NUMNOTEQUAL { r = ifx a != b then 1 else 0; }
  else if op == OP_LESSTHAN { r = ifx a < b then 1 else 0; }
  else if op == OP_GREATERTHAN { r = ifx a > b then 1 else 0; }
  else if op == OP_LESSTHANOREQUAL { r = ifx a <= b then 1 else 0; }
  else if op == OP_GREATERTHANOREQUAL { r = ifx a >= b then 1 else 0; }
  else if op == OP_MIN { r = ifx a < b then a else b; }  // mutant: equivalent (equal operands give the same minimum)
  else { r = ifx a > b then a else b; }                  // OP_MAX; mutant: equivalent (equal operands give the same maximum)
  if op == OP_NUMEQUALVERIFY { return ifx r != 0 then SCRIPT_ERR_OK else SCRIPT_ERR_NUMEQUALVERIFY; }
  si_push(w, si_number(w, r));
  return SCRIPT_ERR_OK;
}
```

The hash opcodes use chapter 4's SHA-256, chapter 6's RIPEMD-160 and
HASH160, and SHA-1:

<!-- source: coax/lib/bitcoin/script.coax crypto -->
```jai
// The hash opcodes: RIPEMD-160, SHA-1 and HASH160 give 20 bytes, SHA-256
// and HASH256 32.
si_hash :: (sp: ScriptSpend, w: [] ScriptWork, op: int) -> int {
  if si_size(w) < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  input := si_bytes(sp, w, si_top(w, 1));
  el := si_new(w, ifx op <= OP_SHA1 || op == OP_HASH160 then 20 else 32);
  out := w[0].arena[el.at..<el.at+el.n];
  if op == OP_RIPEMD160 { #assert ripemd160_hash_checked(input, out, false) == RMD_OK; }
  else if op == OP_SHA1 { #assert sha1_hash(cast(string) input, out); }
  else if op == OP_SHA256 { #assert sha256_hash_checked(input, out, false) == SHA_OK; }
  else if op == OP_HASH160 { #assert hash160_checked(input, out, false) == RMD_OK; }
  else { #assert sha256d_checked(input, out, false) == SHA_OK; }
  si_pop(w, 1); si_push(w, el);
  return SCRIPT_ERR_OK;
}
```

`CHECKLOCKTIMEVERIFY` and `CHECKSEQUENCEVERIFY` compare a number on the
stack with the transaction's lock time or this input's sequence, and
leave the stack as it was. Without their flags they are the `NOP2` and
`NOP3` they replaced, and `DISCOURAGE_UPGRADABLE_NOPS` doesn't apply to
them. Chapter 39 explains both locks:

<!-- source: coax/lib/bitcoin/script.coax locktime -->
```jai
// CheckLockTime and CheckSequence (BIP-65 and BIP-112; chapter 39): the
// script's number against this input's transaction.
si_locktime :: (sp: ScriptSpend, value: int) -> bool {
  locktime := tx_le_vartime(sp.b, sp.t.locktime, 4);
  if (locktime < SCRIPT_LOCKTIME_THRESHOLD) != (value < SCRIPT_LOCKTIME_THRESHOLD) { return false; }
  if value > locktime { return false; }
  return tx_le_vartime(sp.b, sp.ins[sp.index].sequence, 4) != SCRIPT_SEQUENCE_FINAL;
}
si_sequence :: (sp: ScriptSpend, value: int) -> bool {
  sequence := tx_le_vartime(sp.b, sp.ins[sp.index].sequence, 4);  // mutant: equivalent (a fifth byte lands above bit 31, outside every mask)
  if tx_le_vartime(sp.b, 0, 4) < 2 { return false; }
  if (sequence & SCRIPT_SEQUENCE_DISABLE) != 0 { return false; }
  mask := SCRIPT_SEQUENCE_TYPE | SCRIPT_SEQUENCE_MASK;
  have := sequence & mask; want := value & mask;
  if (have < SCRIPT_SEQUENCE_TYPE) != (want < SCRIPT_SEQUENCE_TYPE) { return false; }
  return want <= have;
}
// OP_CHECKLOCKTIMEVERIFY and OP_CHECKSEQUENCEVERIFY, when their flags are
// set: a 5-byte number at the top, left there, not negative, and
// satisfied by the transaction. CSV with the disable bit set is a NOP.
si_lock_op :: (sp: ScriptSpend, w: [] ScriptWork, op: int, minimal: bool) -> int {
  if si_size(w) < 1 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  n := script_num_vartime(si_bytes(sp, w, si_top(w, 1)), minimal, 5);
  if !n.ok { return SCRIPT_ERR_SCRIPTNUM; }
  if n.value < 0 { return SCRIPT_ERR_NEGATIVE_LOCKTIME; }
  if op == OP_CHECKLOCKTIMEVERIFY {
    if !si_locktime(sp, n.value) { return SCRIPT_ERR_UNSATISFIED_LOCKTIME; }
  } else if (n.value & SCRIPT_SEQUENCE_DISABLE) == 0 && !si_sequence(sp, n.value) {
    return SCRIPT_ERR_UNSATISFIED_LOCKTIME;
  }
  return SCRIPT_ERR_OK;
}
```

### Worked example: sixteen scripts

The example runs sixteen of the library's cases, each a scriptSig and a
scriptPubKey in a transaction built the way Core's tests build theirs:

| Case | scriptSig | scriptPubKey | Flags | Result |
| --- | --- | --- | --- | --- |
| ADD | `1 2` | `ADD 3 EQUAL` | | `OK` |
| ELSE taken | `0` | `IF 0 ELSE 1 ENDIF` | | `OK` |
| IF without ENDIF | `1` | `IF 1` | | `UNBALANCED_CONDITIONAL` |
| VERIF unexecuted | `0` | `IF VERIF ENDIF 1` | | `BAD_OPCODE` |
| disabled CAT | `1` | `0 IF CAT ENDIF` | | `DISABLED_OPCODE` |
| 2ROT | | `1 2 3 4 5 6 2ROT`, then checks the order | | `OK` |
| PICK past the end | | `1 1 PICK` | | `INVALID_STACK_OPERATION` |
| a push of 521 | | `PUSHDATA2` of 521 bytes, `DROP 1` | | `PUSH_SIZE` |
| 202 ops | `1` | 202 `NOP`s | | `OP_COUNT` |
| 1,001 elements | | 1,001 `1`s | | `STACK_SIZE` |
| negative zero, minimal | | `0x01 0x80 NOT` | `MINIMALDATA` | `SCRIPTNUM` |
| CHECKMULTISIG, NULLDUMMY | `1 0` | `0 0 CHECKMULTISIG NOT NOT` | `NULLDUMMY` | `OK` |
| P2SH | a push of `51` (`OP_1`) | `HASH160 <hash of 51> EQUAL` | `P2SH` | `OK` |
| P2WSH, MINIMALIF | | v0 program of `IF 1 ENDIF`, witness `02` | `P2SH,WITNESS,MINIMALIF` | `MINIMALIF` |
| CSV, more | | `11 CHECKSEQUENCEVERIFY`, sequence 10 | `CHECKSEQUENCEVERIFY` | `UNSATISFIED_LOCKTIME` |
| v1 of 32 bytes, Taproot on | | `OP_1` and 32 bytes | `P2SH,WITNESS,TAPROOT` | `TAPROOT_UNBUILT` |

In the `NULLDUMMY` case, `0 0 CHECKMULTISIG` has no keys and no
signatures, so it pops the two counts and then the extra element below
them, the scriptSig's `0`. That element is empty, so `NULLDUMMY` passes.
The scriptSig's `1` stays under the result; under `CLEANSTACK` the script
would fail.

## CHECKSIG

A signature commits to a hash of the transaction, and part of that hash is
the *scriptCode*: the script being run, from just after the last
`OP_CODESEPARATOR` that ran. In legacy script, `CHECKSIG` first removes
every push of the signature itself from the scriptCode (FindAndDelete),
because the signature cannot have signed itself. Then it checks the
signature's encoding and the key's, and verifies:

<!-- source: coax/lib/bitcoin/script.coax checksig -->
```jai
// OP_CHECKSIG and OP_CHECKSIGVERIFY (EvalChecksigPreTapscript): the
// scriptCode from the last OP_CODESEPARATOR, with the signature's push
// removed in legacy script; the encoding checks; the verification; and
// NULLFAIL, which turns a failed non-empty signature into an error.
si_checksig :: (sp: ScriptSpend, w: [] ScriptWork, script: [] u8, begin: int, op: int, flags: int, version: int) -> int {
  if si_size(w) < 2 { return SCRIPT_ERR_INVALID_STACK_OPERATION; }
  sig := si_bytes(sp, w, si_top(w, 2)); key := si_bytes(sp, w, si_top(w, 1));
  si_code(w, script, begin);
  if version == SCRIPT_BASE && si_find_delete(w, sig) > 0 && (flags & SCRIPT_VERIFY_CONST_SCRIPTCODE) != 0 { return SCRIPT_ERR_SIG_FINDANDDELETE; }
  error := si_sig_check(sig, flags);
  if error == SCRIPT_ERR_OK { error = si_key_check(key, flags, version); }
  if error != SCRIPT_ERR_OK { return error; }
  ok := si_ecdsa_vartime(sp, sig, key, w[0].code[0..<w[0].code_n], version);
  if !ok && (flags & SCRIPT_VERIFY_NULLFAIL) != 0 && sig.count > 0 { return SCRIPT_ERR_SIG_NULLFAIL; }
  si_pop(w, 2);
  if op == OP_CHECKSIGVERIFY { return ifx ok then SCRIPT_ERR_OK else SCRIPT_ERR_CHECKSIGVERIFY; }
  si_push(w, si_bool(ok));
  return SCRIPT_ERR_OK;
}
```

FindAndDelete removes only whole pushes that start where an opcode
starts, and every copy of them, including copies made adjacent by an
earlier removal:

<!-- source: coax/lib/bitcoin/script.coax finddelete -->
```jai
// FindAndDelete(scriptCode, CScript() << sig) on w.code[0..<w.code_n]:
// removes every copy of the signature's push that starts where an op
// starts, in place, and returns how many it removed. The push of an empty
// signature is the single byte 0x00, so an empty signature removes every
// OP_0.
si_find_delete :: (w: [] ScriptWork, sig: [] u8) -> int {
  pattern: [SCRIPT_MAX_ELEMENT + 3] u8;
  head := ifx sig.count < OP_PUSHDATA1 then 1 else ifx sig.count <= 255 then 2 else 3;
  if head == 1 { pattern[0] = cast(u8) sig.count; }
  else if head == 2 { pattern[0] = OP_PUSHDATA1; pattern[1] = cast(u8) sig.count; }
  else { pattern[0] = OP_PUSHDATA2; pattern[1] = cast(u8) (sig.count & 255); pattern[2] = cast(u8) (sig.count >> 8); }
  for i: 0..<sig.count { pattern[head+i] = sig[i]; }
  p := pattern[0..<head+sig.count];
  code := w[0].code[0..<w[0].code_n];
  found := 0; out := 0; pc := 0; kept := 0;  // mutant: equivalent (the first copy runs from kept to pc = 0, empty for kept 0 or 1)
  while true {
    for k: kept..<pc { code[out] = code[k]; out += 1; }
    while code.count - pc >= p.count && si_same(code[pc..<pc+p.count], p) { pc += p.count; found += 1; }
    kept = pc;
    op := script_op_vartime(code, pc);
    if !op.ok { break; }
    pc = op.next;
  }
  if found > 0 {  // mutant: equivalent (with nothing found out equals kept, so the copy is onto itself)
    for k: kept..<code.count { code[out] = code[k]; out += 1; }
    w[0].code_n = out;
  }
  return found;
}
si_same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
```

`CONST_SCRIPTCODE` is the policy against both: under it, a legacy script
with `OP_CODESEPARATOR`, or a signature FindAndDelete would find, fails.

### Encodings

Chapter 21 built the signature checks: strict DER under `DERSIG`, low S
under `LOW_S`, and a defined hash type under `STRICTENC`. The key checks
are shapes only: under `STRICTENC` a key must be 33 bytes with prefix 02
or 03, or 65 with 04, and under `WITNESS_PUBKEYTYPE` a v0 script's key
must be compressed.

<!-- source: coax/lib/bitcoin/script.coax keys -->
```jai
// IsCompressedOrUncompressedPubKey (STRICTENC) and IsCompressedPubKey
// (WITNESS_PUBKEYTYPE): shapes only, not points.
si_key_shape :: (key: [] u8) -> bool {
  if key.count == 33 { return key[0] == 2 || key[0] == 3; }
  return key.count == 65 && key[0] == 4;
}
si_key_compressed :: (key: [] u8) -> bool { return key.count == 33 && (key[0] == 2 || key[0] == 3); }
// CheckPubKeyEncoding.
si_key_check :: (key: [] u8, flags: int, version: int) -> int {
  if (flags & SCRIPT_VERIFY_STRICTENC) != 0 && !si_key_shape(key) { return SCRIPT_ERR_PUBKEYTYPE; }
  if (flags & SCRIPT_VERIFY_WITNESS_PUBKEYTYPE) != 0 && version == SCRIPT_V0 && !si_key_compressed(key) { return SCRIPT_ERR_WITNESS_PUBKEYTYPE; }
  return SCRIPT_ERR_OK;
}
// CheckSignatureEncoding, through chapter 21's check, whose flags are
// DERSIG 1, LOW_S 2 and STRICTENC 4.
si_sig_check :: (sig: [] u8, flags: int) -> int {
  der := (ifx (flags & SCRIPT_VERIFY_DERSIG) != 0 then SCRIPT_DERSIG else 0)
    | (ifx (flags & SCRIPT_VERIFY_LOW_S) != 0 then SCRIPT_LOW_S else 0)
    | (ifx (flags & SCRIPT_VERIFY_STRICTENC) != 0 then SCRIPT_STRICTENC else 0);
  error := der_signature_check_vartime(sig, der);
  if error == SIG_DER { return SCRIPT_ERR_SIG_DER; }
  if error == SIG_HIGH_S { return SCRIPT_ERR_SIG_HIGH_S; }
  if error == SIG_HASHTYPE { return SCRIPT_ERR_SIG_HASHTYPE; }
  return SCRIPT_ERR_OK;
}
```

### Verification

Core's `CheckECDSASignature` takes what `CPubKey` takes, which includes
*hybrid* keys: 65 bytes with prefix 06 or 07, where the low bit of the
prefix must match y's. It parses the signature with the lax parser of
chapter 21, so a signature that `DERSIG` would refuse still verifies
without it, and it accepts either half of s. A segwit signature commits to
the amount spent, so the same signature fails for any other amount.

<!-- source: coax/lib/bitcoin/script.coax checker -->
```jai
// CheckECDSASignature: a key CPubKey reads (33 bytes with prefix 02 or
// 03, 65 with 04, or a hybrid 65 with 06 or 07 whose low bit is y's
// parity), a signature with its sighash byte, the hash for its
// SigVersion, then chapter 21's lax parser and chapter 19's verifier,
// which accepts either half of s as Core does after normalizing.
si_ecdsa_vartime :: (sp: ScriptSpend, sig: [] u8, key: [] u8, code: [] u8, version: int) -> bool {
  if sig.count == 0 || key.count == 0 { return false; }
  prefix := cast(int) key[0];
  wanted := ifx prefix == 2 || prefix == 3 then 33 else ifx prefix == 4 || prefix == 6 || prefix == 7 then 65 else 0;  // mutant: equivalent (else 1 lets a 1-byte key on to a verification that fails)
  if key.count != wanted { return false; }
  point: [65] u8;
  for i: 0..<key.count { point[i] = key[i]; }
  if prefix >= 6 {
    if (cast(int) key[64] & 1) != (prefix & 1) { return false; }
    point[0] = 4;
  }
  hash: [32] u8;
  hashtype := cast(int) sig[sig.count-1];
  if version == SCRIPT_V0 { script_sighash_v0_vartime(sp, code, hashtype, hash[..]); }
  else { script_sighash_legacy_vartime(sp, code, hashtype, hash[..]); }
  compact: [64] u8;
  if !der_read_lax_vartime(sig[0..<sig.count-1], compact[..]) { return false; }
  return ecdsa_verify_vartime(hash[..], point[0..<key.count], compact[..], false);
}
```

### The two signature hashes

A legacy signature hashes the transaction with this input's scriptSig
replaced by the scriptCode and every other scriptSig empty; the hash type
then drops other inputs (`ANYONECANPAY`) or outputs (`NONE`, `SINGLE`).
A v0 signature hashes BIP-143's summary instead: hashes of the outpoints,
sequences and outputs, then this input's outpoint, scriptCode, amount and
sequence. Chapters 37 and 38 explain them field by field; here they are
computed over chapter 34's reader, for transactions of any size and with
witnesses. `sighash.coax`, written for Nostr chapter 104, has its own
BIP-143 hash over unsigned transactions of at most 16 inputs and 4,096
bytes. Both give BIP-143's digests, and
[chapter 38](ch038-segwit-v0-signature-hashes.md) explains why both remain.

The scriptCode a legacy signature hashes leaves out the
`OP_CODESEPARATOR`s:

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

One behavior is a bug that is now consensus. A legacy `SIGHASH_SINGLE`
signature on an input with no output at its index doesn't hash the
transaction at all. It signs the number 1, so anyone holding that
signature can spend the input to any output. The library's checks use a
row of `sighash.json` with four inputs and three outputs, and input 3
gives 1.

## CHECKMULTISIG

`CHECKMULTISIG` takes `<dummy> <signatures> m <keys> n`. It checks the
signatures against the keys in order: each key is tried once, a
signature moves on only when it matches, and the loop stops as soon as
fewer keys remain than signatures. So the signatures must be in the same
order as their keys. Each key counts as an opcode toward the 201.

The first release popped one element more than it used. That element, the
*dummy*, is still popped, and `NULLDUMMY` (BIP-147) requires it to be
empty so that nobody can change it.

<!-- source: coax/lib/bitcoin/script.coax multisig -->
```jai
// OP_CHECKMULTISIG and OP_CHECKMULTISIGVERIFY: <dummy> <sigs...> m
// <keys...> n. Each key counts toward the 201-op limit. Signatures are
// matched to keys in order, each key tried once, and the loop stops as
// soon as too few keys remain. Then every argument is popped, under
// NULLFAIL each signature must be empty if the check failed, and one more
// element, the dummy, is popped: an off-by-one in the first release that
// NULLDUMMY (BIP-147) now requires to be empty.
si_checkmultisig :: (sp: ScriptSpend, w: [] ScriptWork, script: [] u8, begin: int, op: int, flags: int, version: int, ops: int) -> ScriptStep {
  minimal := (flags & SCRIPT_VERIFY_MINIMALDATA) != 0;
  i := 1;
  if si_size(w) < i { return ScriptStep.{error=SCRIPT_ERR_INVALID_STACK_OPERATION}; }
  keys := script_num_vartime(si_bytes(sp, w, si_top(w, i)), minimal, 4);
  if !keys.ok { return ScriptStep.{error=SCRIPT_ERR_SCRIPTNUM}; }
  if keys.value < 0 || keys.value > SCRIPT_MAX_KEYS { return ScriptStep.{error=SCRIPT_ERR_PUBKEY_COUNT}; }
  if ops + keys.value > SCRIPT_MAX_OPS { return ScriptStep.{error=SCRIPT_ERR_OP_COUNT}; }
  nkeys := keys.value;
  i += 1; ikey := i; ikey2 := nkeys + 2;
  i += nkeys;
  if si_size(w) < i { return ScriptStep.{error=SCRIPT_ERR_INVALID_STACK_OPERATION}; }
  sigs := script_num_vartime(si_bytes(sp, w, si_top(w, i)), minimal, 4);
  if !sigs.ok { return ScriptStep.{error=SCRIPT_ERR_SCRIPTNUM}; }
  if sigs.value < 0 || sigs.value > nkeys { return ScriptStep.{error=SCRIPT_ERR_SIG_COUNT}; }
  nsigs := sigs.value;
  i += 1; isig := i;
  i += nsigs;
  if si_size(w) < i { return ScriptStep.{error=SCRIPT_ERR_INVALID_STACK_OPERATION}; }
  si_code(w, script, begin);
  for k: 0..<nsigs {
    if version == SCRIPT_BASE && si_find_delete(w, si_bytes(sp, w, si_top(w, isig + k))) > 0 && (flags & SCRIPT_VERIFY_CONST_SCRIPTCODE) != 0 { return ScriptStep.{error=SCRIPT_ERR_SIG_FINDANDDELETE}; }
  }
  ok := true;
  while ok && nsigs > 0 {
    sig := si_bytes(sp, w, si_top(w, isig)); key := si_bytes(sp, w, si_top(w, ikey));
    error := si_sig_check(sig, flags);
    if error == SCRIPT_ERR_OK { error = si_key_check(key, flags, version); }
    if error != SCRIPT_ERR_OK { return ScriptStep.{error=error}; }
    if si_ecdsa_vartime(sp, sig, key, w[0].code[0..<w[0].code_n], version) { isig += 1; nsigs -= 1; }
    ikey += 1; nkeys -= 1;
    if nsigs > nkeys { ok = false; }                      // too many have failed
  }
  while i > 1 {
    i -= 1;
    if !ok && (flags & SCRIPT_VERIFY_NULLFAIL) != 0 && ikey2 == 0 && si_top(w, 1).n > 0 { return ScriptStep.{error=SCRIPT_ERR_SIG_NULLFAIL}; }
    if ikey2 > 0 { ikey2 -= 1; }
    si_pop(w, 1);
  }
  if si_size(w) < 1 { return ScriptStep.{error=SCRIPT_ERR_INVALID_STACK_OPERATION}; }  // mutant: equivalent (unreachable: the size check before the loop counted the dummy)
  if (flags & SCRIPT_VERIFY_NULLDUMMY) != 0 && si_top(w, 1).n > 0 { return ScriptStep.{error=SCRIPT_ERR_SIG_NULLDUMMY}; }
  si_pop(w, 1);
  if op == OP_CHECKMULTISIGVERIFY { return ScriptStep.{error=ifx ok then SCRIPT_ERR_OK else SCRIPT_ERR_CHECKMULTISIGVERIFY, keys=keys.value}; }
  si_push(w, si_bool(ok));
  return ScriptStep.{keys=keys.value};
}
```

`NULLFAIL` requires every signature of a failed check to be empty, so a
script that wants `CHECKSIG` to return false must push an empty signature,
not a wrong one.

## VerifyScript

`VerifyScript` runs one input. The scriptSig runs first, then the
scriptPubKey on the stack it left, and the top must then be true. Then,
in order:

1. A scriptPubKey that is a witness program (`OP_0` to `OP_16` and one
   push of 2 to 40 bytes) needs an empty scriptSig, and its witness runs.
2. Under `P2SH`, a scriptPubKey of the form `HASH160 <20 bytes> EQUAL`
   needs a push-only scriptSig. Its last push is the redeem script, which
   runs on the stack the scriptSig left, without that push. A redeem
   script that is a witness program needs a scriptSig that is exactly its
   push, and its witness runs.
3. `CLEANSTACK` requires exactly one element at the end.
4. `WITNESS` refuses an input with witness data that no program used.

<!-- source: coax/lib/bitcoin/script.coax verify -->
```jai
// VerifyScript for input sp.index: the scriptSig and the scriptPubKey run
// in turn on one stack, and the top must be true. A bare witness program
// then needs an empty scriptSig and runs its witness. Under P2SH, a
// scriptPubKey of the P2SH form needs a push-only scriptSig, whose last
// push is the redeem script: it runs on the scriptSig's other pushes, and
// if it is a witness program the scriptSig must be exactly its push.
// CLEANSTACK wants one element left, and WITNESS refuses witness data no
// program used. Returns SCRIPT_ERR_OK or the error, Core's number.
script_verify_vartime :: (sp: ScriptSpend, flags: int, w: [] ScriptWork) -> int {
  if (flags & SCRIPT_VERIFY_WITNESS) != 0 && (flags & SCRIPT_VERIFY_P2SH) == 0 { return SCRIPT_BAD_FLAGS; }  // mutant: equivalent (with P2SH clear, flags & 2049 is flags & 2048)
  if (flags & SCRIPT_VERIFY_CLEANSTACK) != 0 && (flags & SCRIPT_VERIFY_WITNESS) == 0 { return SCRIPT_BAD_FLAGS; }
  si_reset(w);
  input := sp.ins[sp.index];
  sig := ScriptItem.{src=SRC_TX, at=input.script, n=input.script_n};
  spk := ScriptItem.{src=SRC_SPK, n=sp.spk.count};
  sig_bytes := si_bytes(sp, w, sig);
  if (flags & SCRIPT_VERIFY_SIGPUSHONLY) != 0 && !script_push_only_vartime(sig_bytes) { return SCRIPT_ERR_SIG_PUSHONLY; }
  error := si_eval(sp, w, sig, flags, SCRIPT_BASE);
  if error != SCRIPT_ERR_OK { return error; }
  for k: 0..<w[0].depth { w[0].saved[k] = w[0].stack[k]; }
  w[0].saved_depth = w[0].depth;
  error = si_eval(sp, w, spk, flags, SCRIPT_BASE);
  if error != SCRIPT_ERR_OK { return error; }
  if w[0].depth == 0 || !script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { return SCRIPT_ERR_EVAL_FALSE; }
  witnessed := false;
  version := script_witness_version_vartime(sp.spk);
  if (flags & SCRIPT_VERIFY_WITNESS) != 0 && version >= 0 {
    witnessed = true;
    if sig.n != 0 { return SCRIPT_ERR_WITNESS_MALLEATED; }
    error = si_witness_program(sp, w, version, ScriptItem.{src=SRC_SPK, at=2, n=sp.spk.count-2}, flags, false);
    if error != SCRIPT_ERR_OK { return error; }  // mutant: equivalent (returning OK early skips only checks a witnessed spend passes)
    w[0].depth = 1;
  }
  if (flags & SCRIPT_VERIFY_P2SH) != 0 && script_p2sh_vartime(sp.spk) {
    if !script_push_only_vartime(sig_bytes) { return SCRIPT_ERR_SIG_PUSHONLY; }
    for k: 0..<w[0].saved_depth { w[0].stack[k] = w[0].saved[k]; }
    w[0].depth = w[0].saved_depth;  // mutant: equivalent (a P2SH scriptPubKey leaves the depth it started with)
    redeem := si_top(w, 1);
    si_pop(w, 1);
    error = si_eval(sp, w, redeem, flags, SCRIPT_BASE);
    if error != SCRIPT_ERR_OK { return error; }
    if w[0].depth == 0 || !script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { return SCRIPT_ERR_EVAL_FALSE; }
    code := si_bytes(sp, w, redeem);
    version = script_witness_version_vartime(code);
    if (flags & SCRIPT_VERIFY_WITNESS) != 0 && version >= 0 {
      witnessed = true;
      // The scriptSig must be exactly one push of the redeem script, a
      // direct push since a program has at most 42 bytes.
      if sig.n != redeem.n + 1 || sig_bytes[0] != cast(u8) redeem.n || !si_same(sig_bytes[1..<sig.n], code) { return SCRIPT_ERR_WITNESS_MALLEATED_P2SH; }  // mutant: equivalent (a longer scriptSig also fails the comparison, and one of n + 1 bytes ending in an n-byte push is that direct push)
      error = si_witness_program(sp, w, version, ScriptItem.{src=redeem.src, at=redeem.at+2, n=redeem.n-2}, flags, true);
      if error != SCRIPT_ERR_OK { return error; }  // mutant: equivalent (returning OK early skips only checks a witnessed spend passes)
      w[0].depth = 1;
    }
  }
  if (flags & SCRIPT_VERIFY_CLEANSTACK) != 0 && w[0].depth != 1 { return SCRIPT_ERR_CLEANSTACK; }
  if (flags & SCRIPT_VERIFY_WITNESS) != 0 && !witnessed && input.count != 0 { return SCRIPT_ERR_WITNESS_UNEXPECTED; }
  return SCRIPT_ERR_OK;
}
```

### Witness programs

Version 0 has two lengths. A 20-byte program is P2WPKH: the witness is a
signature and a key, run against `DUP HASH160 <program> EQUALVERIFY
CHECKSIG`. A 32-byte program is P2WSH: the last witness item is the
script, its SHA-256 must be the program, and it runs on the other items.
Either way the script must leave exactly one element, and it must be
true. Any other length fails.

Version 1 with 32 bytes, outside P2SH, is Taproot under its flag (chapters
43 and 44), and `TAPROOT_UNBUILT` here. Every other version and length
succeeds, so that later soft forks can give them meaning, unless policy's
`DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM` refuses them. The two-byte
program `4e73` of version 1 is pay-to-anchor (P2A), which policy keeps.

<!-- source: coax/lib/bitcoin/script.coax program -->
```jai
// CScript::IsWitnessProgram: a version opcode (OP_0, or OP_1 to OP_16)
// and one direct push of 2 to 40 bytes, nothing else. Returns the
// version, or -1.
script_witness_version_vartime :: (s: [] u8) -> int #export_c {
  if s.count < 4 || s.count > 42 { return -1; }
  if s[0] != OP_0 && (s[0] < OP_1 || s[0] > OP_16) { return -1; }
  if cast(int) s[1] + 2 != s.count { return -1; }
  return ifx s[0] == OP_0 then 0 else cast(int) s[0] - 0x50;
}
// CScript::IsPayToScriptHash: OP_HASH160, a 20-byte push, OP_EQUAL.
script_p2sh_vartime :: (s: [] u8) -> bool #export_c {
  return s.count == 23 && s[0] == OP_HASH160 && s[1] == 0x14 && s[22] == OP_EQUAL;
}
```

<!-- source: coax/lib/bitcoin/script.coax witness -->
```jai
// ExecuteWitnessScript for SigVersion::WITNESS_V0: the witness items
// (count of them) become the stack, none may exceed 520 bytes, the script
// runs, and it must leave exactly one element, which must be true. A
// stack larger than SCRIPT_ROOM keeps its top SCRIPT_ROOM elements and
// counts the rest.
si_execute_witness :: (sp: ScriptSpend, w: [] ScriptWork, count: int, script: ScriptItem, flags: int) -> int {
  for k: 0..<count { if si_witness(sp, k).n > SCRIPT_MAX_ELEMENT { return SCRIPT_ERR_PUSH_SIZE; } }
  w[0].hidden = ifx count > SCRIPT_ROOM then count - SCRIPT_ROOM else 0;
  w[0].depth = 0;
  for k: w[0].hidden..<count { si_push(w, si_witness(sp, k)); }
  error := si_eval(sp, w, script, flags, SCRIPT_V0);
  if error != SCRIPT_ERR_OK { return error; }
  if si_size(w) != 1 { return SCRIPT_ERR_CLEANSTACK; }
  if !script_bool_vartime(si_bytes(sp, w, si_top(w, 1))) { return SCRIPT_ERR_EVAL_FALSE; }
  return SCRIPT_ERR_OK;
}
// VerifyWitnessProgram for a program at program (its bytes in prog):
// version 0 with 32 bytes is P2WSH, whose last witness item is the script
// and must hash to the program; 20 bytes is P2WPKH, whose two items run
// against DUP HASH160 <program> EQUALVERIFY CHECKSIG; any other length
// refuses. Version 1 with 32 bytes outside P2SH is Taproot (chapter 43)
// when its flag is set; the 2-byte anchor 4e73 (P2A) and every other
// version and length succeed, unless policy discourages them.
si_witness_program :: (sp: ScriptSpend, w: [] ScriptWork, version: int, program: ScriptItem, flags: int, p2sh: bool) -> int {
  prog := si_bytes(sp, w, program);
  count := sp.ins[sp.index].count;
  if version == 0 && prog.count == 32 {
    if count == 0 { return SCRIPT_ERR_WITNESS_PROGRAM_WITNESS_EMPTY; }
    script := si_witness(sp, count - 1);
    hash: [32] u8;
    #assert sha256_hash_checked(si_bytes(sp, w, script), hash[..], false) == SHA_OK;
    if !si_same(hash[..], prog) { return SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH; }
    return si_execute_witness(sp, w, count - 1, script, flags);
  }
  if version == 0 && prog.count == 20 {
    if count != 2 { return SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH; }
    script := si_new(w, 25);
    script_p2wpkh_code_vartime(prog, w[0].arena[script.at..<script.at+25]);
    return si_execute_witness(sp, w, 2, script, flags);
  }
  if version == 0 { return SCRIPT_ERR_WITNESS_PROGRAM_WRONG_LENGTH; }
  if version == 1 && prog.count == 32 && !p2sh {
    return ifx (flags & SCRIPT_VERIFY_TAPROOT) != 0 then SCRIPT_TAPROOT_UNBUILT else SCRIPT_ERR_OK;
  }
  if version == 1 && prog.count == 2 && prog[0] == 0x4e && prog[1] == 0x73 && !p2sh { return SCRIPT_ERR_OK; }
  if (flags & SCRIPT_VERIFY_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM) != 0 { return SCRIPT_ERR_DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM; }
  return SCRIPT_ERR_OK;
}
```

### Worked example: three signed spends

The library makes three spends with real signatures, from keys derived
from fixed labels:

- A legacy 1-of-2 `CHECKMULTISIG`. The scriptPubKey starts with
  `OP_CODESEPARATOR`, then pushes the signature itself and drops it, then
  `1 <hybrid key> <bad key> 2 CHECKMULTISIG`. The scriptCode starts after
  the separator, and FindAndDelete takes the signature's push out of it.
  The first key tried is the last pushed, a 33-byte key with prefix 05, so
  the hybrid key is the one that matches.
- A P2WPKH spend of 123,456,789 satoshis, under the policy flags, with a
  compressed key.
- A legacy spend whose scriptPubKey is `NOP CODESEPARATOR`, then a
  `CHECKSIGVERIFY` with a key of prefix 02, then a 4-of-4
  `CHECKMULTISIGVERIFY` with keys of prefix 03, 04, 06 and 07, and `1`.
  Every signature matches, one with the high s, which verifies because
  `LOW_S` is off. The scriptCode starts after the separator, so it leaves
  out the `NOP`.

```text
signed spends
  legacy 1 of 2, a hybrid key, CODESEPARATOR, FindAndDelete: OK
    with the locktime changed: EVAL_FALSE
  P2WPKH: OK
    with the locktime changed: NULLFAIL
  legacy CHECKSIGVERIFY and 4 of 4, keys 02 to 07, a high s: OK
    with the locktime changed: NULLFAIL
```

Changing one byte of the lock time changes every hash, so every
signature fails. The first legacy spend runs without flags, so
`CHECKMULTISIG` pushes false and the script ends false. The other two run
under `NULLFAIL`, so the first failed non-empty signature is an error.

## What this chapter replaces

Chapter 21's example ran its 89 rows of `script_tests.json` through
`run_row`, a harness that knew only `CHECKSIG` and `CHECKMULTISIG` over
push-only scripts. The interpreter here runs every row, so later chapters
use it instead. Chapter 21's files are unchanged: they still show the
encoding rules on their own.

## The example

The example is
[bitcoin/examples/script-interpreter.coax](../examples/script-interpreter.coax).
It writes CScriptNums, runs sixteen library cases and the three signed
spends with a lock time byte changed, then runs every runnable row of
`script_tests.json`, every input of every script transaction in
`tx_valid.json` and `tx_invalid.json` under the first flag set Core's
test gives it, and every row of `sighash.json`.

<!-- source: bitcoin/examples/script-interpreter.coax example -->
```jai
#load "../../coax/lib/bitcoin/script.coax";
#load "../../coax/tests/bitcoin/script_vectors.coax";
#import "Basic";

// The transaction, its inputs, outputs and witness items, and the bytes of
// the scriptPubKey the input spends: room for the largest test case.
Spend :: struct {
  b: [10240] u8; spk: [10100] u8;
  ins: [16] TxIn; outs: [16] TxOut; items: [1300] TxItem;
}
// Reads hex into the spend and returns the input-0 view of it.
spend_of :: (s: [] Spend, tx: string, spk: string, index: int, amount: int) -> ScriptSpend {
  b := tx_unhex(tx, s[0].b[..]);
  t := tx_read_vartime(b, true, s[0].ins[..], s[0].outs[..], s[0].items[..]);
  #assert t.code == TX_OK;
  return ScriptSpend.{b=b, t=t, ins=s[0].ins[0..<t.nin], outs=s[0].outs[0..<t.nout], items=s[0].items[0..<t.nitems],
    index=index, amount=amount, spk=tx_unhex(spk, s[0].spk[..])};
}
// The name of a result.
name :: (error: int) -> string {
  if error == SCRIPT_TAPROOT_UNBUILT { return "TAPROOT_UNBUILT"; }
  if error == SCRIPT_BAD_FLAGS { return "BAD_FLAGS"; }
  return SCRIPT_ERR_NAMES[error];
}
// Runs one library case and prints its label and result.
show_case :: (s: [] Spend, w: [] ScriptWork, label: string) {
  for c: SI_CASES {
    if c.label != label { continue; }
    error := script_verify_vartime(spend_of(s, c.tx, c.spk, 0, c.amount), c.flags, w);
    print("  %: %\n", label, name(error));
    return;
  }
  #assert false;
}

main :: () {
  s: [1] Spend; w: [1] ScriptWork;

  // Numbers on the stack: CScriptNum's little-endian sign-magnitude form.
  print("CScriptNum\n");
  for v: [7] int.[0, 1, -1, 127, 128, -255, 2147483648] {
    b: [9] u8;
    n := script_num_write_vartime(v, b[..]);
    print("  %:", v);
    digits := "0123456789abcdef";
    for k: 0..<n {
      pair: [2] u8; pair[0] = digits[b[k] >> 4]; pair[1] = digits[b[k] & 15];
      print(" %", cast(string) pair[..]);
    }
    print(" (% bytes)\n", n);
  }

  // A few scripts and what VerifyScript says about each.
  print("scripts\n");
  labels := [16] string.["ADD", "ELSE taken", "IF without ENDIF", "VERIF unexecuted", "disabled CAT", "2ROT",
    "PICK past the end", "a push of 521", "202 ops", "1,001 elements", "negative zero, minimal",
    "CHECKMULTISIG, NULLDUMMY", "P2SH", "P2WSH, MINIMALIF", "CSV, more", "v1 of 32 bytes, Taproot on"];
  for l: labels { show_case(s[..], w[..], l); }

  // Three spends with real signatures.
  print("signed spends\n");
  for c: SI_SIGNED {
    sp := spend_of(s[..], c.tx, c.spk, 0, c.amount);
    print("  %: %\n", c.label, name(script_verify_vartime(sp, c.flags, w[..])));
    // One byte of the signed transaction changed: its locktime.
    sp.b[sp.t.locktime] ^= 1;
    print("    with the locktime changed: %\n", name(script_verify_vartime(sp, c.flags, w[..])));
  }

  // Every row of script_tests.json, counted by the error it expects.
  print("Bitcoin Core script_tests.json\n");
  rows: [SCRIPT_ERR_COUNT] int; passed: [SCRIPT_ERR_COUNT] int;
  for r: SCRIPT_ROWS {
    error := script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, r.amount), r.flags, w[..]);
    rows[r.error] += 1;
    if error == r.error { passed[r.error] += 1; }
  }
  total := 0; good := 0;
  for e: 0..<SCRIPT_ERR_COUNT {
    if rows[e] == 0 { continue; }
    print("  %: % of %\n", SCRIPT_ERR_NAMES[e], passed[e], rows[e]);
    total += rows[e]; good += passed[e];
  }
  print("  all: % of % rows give the expected result\n", good, total);
  print("  not run: % rows that need Taproot (chapter 44):", SCRIPT_SKIPPED.count);
  for r: SCRIPT_SKIPPED { print(" %", r.line); }
  print("\n");

  // tx_valid.json and tx_invalid.json: every input of every transaction,
  // under the first flag set Core's test uses for it: every flag but the
  // listed ones for a valid transaction, the listed ones for an invalid
  // one. The other flag sets run in coax/tests/bitcoin/script.py.
  print("Bitcoin Core tx_valid.json and tx_invalid.json\n");
  agree := 0; runs := 0; last := -1;
  for c: SCRIPT_TX_CASES {
    if c.tx == last { continue; }
    last = c.tx;
    x := SCRIPT_TXS[c.tx];
    valid := true;
    for k: 0..<x.count {
      p := SCRIPT_PREVS[x.first + k];
      runs += 1;
      if script_verify_vartime(spend_of(s[..], x.tx, p.spk, k, p.amount), c.flags, w[..]) != SCRIPT_ERR_OK { valid = false; break; }
    }
    if valid == c.valid { agree += 1; }
  }
  print("  % transactions, % inputs run, % verdicts as Core's\n", SCRIPT_TXS.count, runs, agree);

  // sighash.json: the legacy signature hash.
  print("Bitcoin Core sighash.json\n");
  same := 0;
  for r: SCRIPT_SIGHASH {
    sp := spend_of(s[..], r.tx, "", r.index, 0);
    c: [10100] u8; h: [32] u8;
    script_sighash_legacy_vartime(sp, tx_unhex(r.code, c[..]), r.hashtype, h[..]);
    #assert bitcoin_reverse32(h[..], h[..]) == SHA_OK;
    if tx_hex_is(h[..], r.hash) { same += 1; }
  }
  print("  % of % signature hashes match\n", same, SCRIPT_SIGHASH.count);
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/script-interpreter.coax --run-steps 250000000
```

It prints:

```text
CScriptNum
  0: (0 bytes)
  1: 01 (1 bytes)
  -1: 81 (1 bytes)
  127: 7f (1 bytes)
  128: 80 00 (2 bytes)
  -255: ff 80 (2 bytes)
  2147483648: 00 00 00 80 00 (5 bytes)
scripts
  ADD: OK
  ELSE taken: OK
  IF without ENDIF: UNBALANCED_CONDITIONAL
  VERIF unexecuted: BAD_OPCODE
  disabled CAT: DISABLED_OPCODE
  2ROT: OK
  PICK past the end: INVALID_STACK_OPERATION
  a push of 521: PUSH_SIZE
  202 ops: OP_COUNT
  1,001 elements: STACK_SIZE
  negative zero, minimal: SCRIPTNUM
  CHECKMULTISIG, NULLDUMMY: OK
  P2SH: OK
  P2WSH, MINIMALIF: MINIMALIF
  CSV, more: UNSATISFIED_LOCKTIME
  v1 of 32 bytes, Taproot on: TAPROOT_UNBUILT
signed spends
  legacy 1 of 2, a hybrid key, CODESEPARATOR, FindAndDelete: OK
    with the locktime changed: EVAL_FALSE
  P2WPKH: OK
    with the locktime changed: NULLFAIL
  legacy CHECKSIGVERIFY and 4 of 4, keys 02 to 07, a high s: OK
    with the locktime changed: NULLFAIL
Bitcoin Core script_tests.json
  OK: 685 of 685
  EVAL_FALSE: 81 of 81
  OP_RETURN: 5 of 5
  SCRIPTNUM: 67 of 67
  SCRIPT_SIZE: 1 of 1
  PUSH_SIZE: 2 of 2
  OP_COUNT: 6 of 6
  STACK_SIZE: 5 of 5
  SIG_COUNT: 2 of 2
  PUBKEY_COUNT: 2 of 2
  VERIFY: 1 of 1
  EQUALVERIFY: 9 of 9
  CHECKMULTISIGVERIFY: 1 of 1
  CHECKSIGVERIFY: 1 of 1
  NUMEQUALVERIFY: 1 of 1
  BAD_OPCODE: 89 of 89
  DISABLED_OPCODE: 24 of 24
  INVALID_STACK_OPERATION: 104 of 104
  INVALID_ALTSTACK_OPERATION: 3 of 3
  UNBALANCED_CONDITIONAL: 17 of 17
  NEGATIVE_LOCKTIME: 2 of 2
  UNSATISFIED_LOCKTIME: 8 of 8
  SIG_HASHTYPE: 2 of 2
  SIG_DER: 30 of 30
  MINIMALDATA: 21 of 21
  SIG_PUSHONLY: 5 of 5
  SIG_HIGH_S: 1 of 1
  SIG_NULLDUMMY: 3 of 3
  PUBKEYTYPE: 8 of 8
  CLEANSTACK: 16 of 16
  MINIMALIF: 12 of 12
  NULLFAIL: 3 of 3
  DISCOURAGE_UPGRADABLE_NOPS: 10 of 10
  DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM: 1 of 1
  WITNESS_PROGRAM_WRONG_LENGTH: 1 of 1
  WITNESS_PROGRAM_WITNESS_EMPTY: 1 of 1
  WITNESS_PROGRAM_MISMATCH: 4 of 4
  WITNESS_MALLEATED: 3 of 3
  WITNESS_MALLEATED_P2SH: 1 of 1
  WITNESS_UNEXPECTED: 3 of 3
  WITNESS_PUBKEYTYPE: 11 of 11
  OP_CODESEPARATOR: 2 of 2
  SIG_FINDANDDELETE: 2 of 2
  all: 1256 of 1256 rows give the expected result
  not run: 7 rows that need Taproot (chapter 44): 1296 1297 1298 1299 1300 1301 1302
Bitcoin Core tx_valid.json and tx_invalid.json
  206 transactions, 268 inputs run, 206 verdicts as Core's
Bitcoin Core sighash.json
  500 of 500 signature hashes match
```

What the output shows:

- Every one of the 1,256 rows gives Core's expected result, counted by
  that result. 685 succeed; the most common failures are
  `INVALID_STACK_OPERATION` (104), `BAD_OPCODE` (89), `EVAL_FALSE` (81) and
  `SCRIPTNUM` (67).
- Seven rows are not run. Lines 1296 to 1302 of `script_tests.json` build
  a Taproot output or control block inside the test (`#TAPROOTOUTPUT#`,
  `#SCRIPT#`, `#CONTROLBLOCK#`) and run tapscript, which is chapter 44's.
- The transaction files hold 206 transactions with prevouts and flags,
  after the 9 `BADTX` rows, which fail `CheckTransaction` (chapter 52)
  before any script runs. A valid transaction must pass under every flag
  except those listed, and an invalid one must fail under its listed
  flags. Running inputs in order until one fails takes 268 runs, and all
  206 verdicts are Core's.
- Core's test then runs each transaction under more flag sets. A valid
  one must also pass with any one more flag removed, and must fail when a
  listed flag is put back, because each listed flag is one it needs to be
  off. An invalid one must also fail with any flag added, and pass with a
  listed flag removed. That gives 4,532 flag sets in all. They run in
  `script.py`, below, where a signature takes microseconds in every
  build.
- All 500 rows of `sighash.json` give their published hash, which
  chapter 37 covers.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/script-interpreter.out](../fixtures/script-interpreter.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/script-interpreter.coax](../examples/checks/script-interpreter.coax),
which loads the example. In the checking interpreter one ECDSA
verification takes tens of millions of steps, so the checks run every
row that never reaches the curve, one in sixteen of the rows that verify
one or two signatures, and every row of `sighash.json`. The example and
`script.py` run every row natively.

<!-- source: bitcoin/examples/checks/script-interpreter.coax check -->
```jai
// Every row of script_tests.json whose signatures, if any, never reach the
// curve gives exactly the error Core's test expects. The rows that verify
// a signature run in the example, natively, where a verification takes
// microseconds rather than seconds. One workspace serves every row, since
// VerifyScript resets it.
#test "script_tests.json rows without curve work" #steps 100000000 {
  s: [1] Spend; w: [1] ScriptWork;
  for r: SCRIPT_ROWS {
    if r.curve == 0 { #assert script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, r.amount), r.flags, w[..]) == r.error; }
  }
}
// One row in sixteen of those that verify signatures, to keep the
// interpreter's share of them in every run.
#forall "script_tests.json rows with signatures, a sample" i: 0..<SCRIPT_ROWS.count #steps 2000000000 {
  r := SCRIPT_ROWS[i];
  if r.curve > 0 && r.curve <= 2 && i % 16 == 0 {
    s: [1] Spend; w: [1] ScriptWork;
    #assert script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, r.amount), r.flags, w[..]) == r.error;
  }
}
// Every row of sighash.json: the published legacy signature hash.
#forall "sighash.json" i: 0..<SCRIPT_SIGHASH.count #steps 100000000 {
  r := SCRIPT_SIGHASH[i];
  s: [1] Spend; c: [10100] u8; h: [32] u8;
  sp := spend_of(s[..], r.tx, "", r.index, 0);
  script_sighash_legacy_vartime(sp, tx_unhex(r.code, c[..]), r.hashtype, h[..]);
  #assert bitcoin_reverse32(h[..], h[..]) == SHA_OK && tx_hex_is(h[..], r.hash);
}
// The rows not run are the seven that need Taproot, and each names it.
#test "the rows not run" {
  #assert SCRIPT_SKIPPED.count == 7 && SCRIPT_ROWS.count + SCRIPT_SKIPPED.count == 1263;
  for r: SCRIPT_SKIPPED { #assert r.comment.count > 9 && r.comment[0..<9] == "TAPSCRIPT"; }
}
// The example's names for the two errors that are not Core's.
#test "the example's names" {
  #assert name(SCRIPT_TAPROOT_UNBUILT) == "TAPROOT_UNBUILT" && name(SCRIPT_BAD_FLAGS) == "BAD_FLAGS" && name(SCRIPT_ERR_OK) == "OK";
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/script-interpreter.coax
coax/bin/coax mutate bitcoin/examples/checks/script-interpreter.coax --root-only
```

`coax test` runs 179 checks, including the loaded files' own checks, in about 45 seconds of CPU time on 2026-11-03. The checks file has no procedures of its own, so root-only mutation finds no site in it; the example's helpers are checked by the output comparison.

### The library's own checks

`script.coax` is an `#authority` file. Its own checks cover:

- `GetScriptOp` on each push form and on pushes cut short;
  `IsPushOnly` and `CheckMinimalPush` at each boundary (75, 76, 255, 256,
  65,535 and 65,536 bytes);
- CScriptNum reading with the size limit and the minimal rule, and
  writing, read back, for 24 values;
- `CastToBool`, the condition stack, FindAndDelete (adjacent copies,
  copies inside other pushes, the empty signature, pushes by
  `PUSHDATA1` and `PUSHDATA2`), the witness program
  and P2SH shapes, and the names;
- the legacy signature hash against ten rows of `sighash.json`, one per
  hash type and input position, and `SIGHASH_SINGLE` past the outputs;
  the BIP-143 hash against BIP-143's eight examples and nine hashes the
  Python reference computes for the positions they leave out;
- 437 library cases, each with the result the Python reference decided,
  each run in a workspace the next case has just used: every opcode,
  every disabled opcode, each limit at and past its edge, each flag's
  refusal and each flag left off with only `P2SH` set, P2SH, P2WPKH, P2WSH
  and P2SH-wrapped programs, the lock-time opcodes, and witness stacks of
  1,000 to 1,200 items, some with `OP_PICK` reaching past the stored
  elements;
- the three signed spends, and the P2WPKH spend with another amount, which
  fails `NULLFAIL`.

```shell
coax/bin/coax test coax/lib/bitcoin/script.coax
coax/bin/coax mutate coax/lib/bitcoin/script.coax --root-only
```

On 2026-11-03 `coax test` runs 174 checks, all passing, in about 34 seconds of CPU time.
Root-only mutation of an earlier version made 2,009 mutants and killed 1,848.
For the 161 that survived, this version adds the cases above and simplifies three lines.
It marks each survivor that no test can kill `// mutant: equivalent` with the reason.
Mutation has not been run again on this version.

### Core in Python, natively and in Wasm

[script_reference.py](../../coax/tests/bitcoin/script_reference.py) is a
transliteration of the pinned Core sources: `EvalScript`, `VerifyScript`,
`VerifyWitnessProgram`, `ExecuteWitnessScript`, FindAndDelete, the
encoding checks, CScriptNum, the condition stack, both signature hashes,
`CheckLockTime`, `CheckSequence`, CPubKey's parser and `ParseScript`, the
language `script_tests.json` is written in. It reads the opcode table from
the pinned `script.h` and `script.cpp`, and shares no code with Coax.
[make_script_vectors.py](../../coax/tests/bitcoin/make_script_vectors.py)
checks each pinned file's SHA-256, runs every row and transaction case
through the reference and refuses to write the vectors unless every result
is Core's.

[script.py](../../coax/tests/bitcoin/script.py) compiles `script.coax` to
C in checked and release builds, through a flat test entry point
([script_report.coax](../../coax/tests/bitcoin/script_report.coax)), and
runs:

- every runnable row under its flags and under three seeded random flag
  sets, some of them sets Core asserts against;
- two seeded edits of each row: a byte of the scriptSig, scriptPubKey or
  a witness item changed, inserted or removed, the script cut short,
  doubled or prefixed;
- each row that succeeds again with seeded flags removed, which must still
  succeed;
- every input of the transaction files under each of the 4,532 flag
  sets, each set's verdict Core's, and under a random set;
- P2WSH spends with 999 to 1,500 witness items, around the stack limit and
  past the workspace's 1,100;
- seeded legacy and BIP-143 signature hashes of the transaction files'
  inputs, and `SIGHASH_SINGLE` past the outputs in both.

Each result must equal the reference's. Then it builds the same C for
wasm32 and runs every case in Node.

```shell
python3 coax/tests/bitcoin/script.py
```

On 2026-11-02, 16,178 cases agree with Python in native checked and release builds and in Wasm checked and release builds: 15,350 runs of `VerifyScript` and 828 signature hashes. Among them are 44 different results: every one this chapter can return except `UNKNOWN_ERROR`, which no path returns, and `TAPROOT_UNBUILT`, which the library's checks cover. 1,370 runs repeat a passing row with flags removed, and all of them pass. The run takes about 2 minutes of CPU time.

## What this chapter leaves out

- Taproot and tapscript: the key path, the control block, `OP_SUCCESS`,
  `CHECKSIGADD` and the validation weight budget are chapters 43 and 44.
  The seven rows they need are not run, and a v1 program under `TAPROOT`
  refuses.
- The signature hashes in detail: chapters
  [37 (legacy)](ch037-legacy-signature-hashes.md) and
  [38 (BIP-143)](ch038-segwit-v0-signature-hashes.md).
- What the lock times mean for a transaction, and BIP-68's relative lock
  times: chapter 39.
- Which flags apply at which height: chapter 55. The transaction rules
  that come before any script (`CheckTransaction`, the 9 `BADTX` rows):
  chapter 52.
- The policy flag set and the standardness rules around it: chapter 81.
- Signature caching and running inputs in parallel: chapter 76.

## Summary

- Script is a stack machine over byte strings. Numbers are little-endian
  sign and magnitude, at most 4 bytes as operands, and negative zero is
  false.
- Some checks apply to every opcode, running or not: the push size, the
  opcode count, the disabled opcodes and `VERIF`.
- `CHECKSIG` hashes the scriptCode after the last `OP_CODESEPARATOR`, and
  in legacy script without the signature's own push. `CHECKMULTISIG`
  matches signatures to keys in order and pops one extra element.
- `VerifyScript` runs the scriptSig, then the scriptPubKey, then a P2SH
  redeem script or a witness program, and the flags tighten each step.
- A flag only refuses more, so a script that passes under some flags
  passes under fewer.
- Every runnable row of `script_tests.json` and every flag set of Core's
  transaction tests gives Core's result.

## Dependencies

- [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md)
- [19. Verify ECDSA](ch019-verify-ecdsa.md)
- [21. Strict DER](ch021-strict-der.md)
- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `script_tests.json`,
  `tx_valid.json`, `tx_invalid.json`, `sighash.json`,
  `script/interpreter.cpp`, `script/interpreter.h`, `script/script.h`,
  `script/script.cpp`, `script/script_error.h`, `script/script_error.cpp`,
  `test/script_tests.cpp`, `test/transaction_tests.cpp` and
  `test/util/transaction_utils.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/interpreter.h).
- [BIP-16](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki)
  (P2SH),
  [BIP-66](https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki)
  (strict DER),
  [BIP-65](https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki)
  and [BIP-112](https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki)
  (the lock-time opcodes),
  [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  and [BIP-143](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki)
  (segwit), and
  [BIP-147](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki)
  (NULLDUMMY).
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
