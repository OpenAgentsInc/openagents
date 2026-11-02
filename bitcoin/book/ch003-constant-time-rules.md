# 3. Constant-time rules and secrets

A signing key, a nonce or a conversation key must not leak through time or
memory. This chapter states the rules that every later chapter follows when
it handles a secret. It shows the Coax functions that follow them, the tools
that check them, and what those tools don't prove.

## What this chapter teaches

- The three rules: a secret never decides a branch, never selects a memory
  address and never reaches a variable-time routine.
- Constant-time equality, masks and selection in
  [coax/lib/crypto/bytes.coax](../../coax/lib/crypto/bytes.coax), and why
  each mask passes through `value_barrier`.
- Explicit erasure: the bookkeeping in
  [coax/lib/crypto/zeroize.coax](../../coax/lib/crypto/zeroize.coax) and the
  C host's volatile writes.
- Three checks: a source audit that refuses a branch on a secret, a check of
  the emitted machine code, and timing measurements with deliberately leaky
  controls.
- What these checks don't claim.

## The three rules

A program leaks a secret when anything an observer can measure depends on
it. An observer can measure running time and, on a shared machine, which
memory the program touched. So code that handles a secret follows three
rules:

1. **No branch on a secret.** An `if`, `ifx`, `while`, `&&` or `||` whose
   condition depends on a secret runs different code for different secrets,
   and takes different time.
2. **No memory address from a secret.** An array index that depends on a
   secret loads a different cache line for different secrets. Another
   process on the same machine can measure which line was loaded.
3. **No variable-time routine on a secret.** Division, a variable shift, and
   any function whose name ends in `_vartime` take time that depends on
   their inputs. The public verification path of chapter 16 uses such
   routines on public data only.

Lengths, counts and positions that are public may decide branches and
indices. Every loop in `bytes.coax` runs over a public length.

## Comparing without a branch

The obvious comparison stops at the first byte that differs. An observer
who times it learns how many leading bytes of a guess are right.

### Worked example: guessing a 32-byte tag

An attacker wants to forge a 32-byte authentication tag and can time the
comparison.

1. With the early-exit comparison, a guess that is wrong in byte 0 reads 1
   byte, and a guess that is wrong only in byte 31 reads 32 bytes.
2. The attacker tries all 256 values of byte 0. The one that takes longest
   is right. They fix it and move to byte 1.
3. The whole tag takes at most 32 × 256 = 8,192 guesses, instead of 2^256.

`cb_ct_equal` reads every byte and has no branch on the data:

```jai
diff: u64;
for i: 0..<a.count { diff |= cast(u64) (a[i] ^ b[i]); }
return cb_mask(((diff + 255) >> 8) ^ 1);
```

Work it through by hand:

- If the inputs are equal, every XOR is 0, so `diff` is 0. Then
  (0 + 255) >> 8 = 0, and 0 ^ 1 = 1. `cb_mask(1)` is all ones.
- If any byte differs, `diff` is between 1 and 255. Then (`diff` + 255) >> 8
  = 1, and 1 ^ 1 = 0. `cb_mask(0)` is 0.

The result is a mask, not a `bool`. The caller combines it with other masks
or uses it to select, and never branches on it.

## Masks and selection

`cb_mask` turns the low bit of a word into a mask of all zeros or all ones,
and `cb_ct_select64` uses the mask to choose between two values:

```jai
cb_mask :: (bit: u64) -> u64 #export_c { return value_barrier(mul_wrap(bit & 1, CB_MAX64)); }
cb_ct_select64 :: (mask: u64, a: u64, b: u64) -> u64 #export_c { return b ^ ((a ^ b) & value_barrier(mask)); }
```

### Worked example: select 15 or 240

Take a = 15 (`0x0f`) and b = 240 (`0xf0`), so a ^ b = 255.

- With an all-ones mask: (a ^ b) & mask = 255, and b ^ 255 = 15. The result
  is a.
- With a zero mask: (a ^ b) & 0 = 0, and b ^ 0 = 240. The result is b.

The same instructions run in both cases.

`cb_mask` multiplies with `mul_wrap`, not `*`. A checked multiplication tests
for overflow, and that test branches on whether an operand is 0, which here
is the secret bit.

### Why `value_barrier`

A C compiler can see that a mask is either 0 or all ones and turn
`b ^ ((a ^ b) & mask)` back into a branch. `value_barrier(x)` returns `x`
unchanged, but the optimizer can't see through it. In native C it's an empty
`asm` statement, and in wasm it's a call through a `volatile` that is never
inlined ([spec](../../coax/spec.md)). It stops this one transformation. It
doesn't prove that a whole program runs in constant time.

## Erasing secrets

A secret left in memory after use can reach a crash dump, swap or a later
bug. Every secret buffer is erased when its work ends, on success and on
every failure path.

Erasure has two parts:

- **Bookkeeping in Coax.** `zeroize.coax` keeps a `ZeroScope` with up to
  eight registered buffers. Each record holds the buffer's capacity and how
  many of its leading bytes are secret. When the scope closes, every buffer
  with secret bytes is pending. The scope is complete only after the host
  acknowledges each wipe. A failed wipe makes the scope report
  `ZERO_EFFECT`, and it can't report success until a retry succeeds.
- **The effect in C.** `coax_secure_zero` in
  [coax/lib/crypto/platform/zeroize.c](../../coax/lib/crypto/platform/zeroize.c)
  checks the range with the Coax function `zero_admit`, then writes zeros
  through a `volatile` pointer followed by a memory fence. A compiler can't
  remove volatile writes as dead stores.

The scope also refuses mistakes before any memory is touched: a range
outside the buffer (`ZERO_RANGE`), a missing address (`ZERO_ADDRESS`), and a
buffer that overlaps one already registered (`ZERO_ALIAS`).

Erasure doesn't reach copies the compiler makes in registers or on the
stack, or copies the operating system makes. Passing a secret by value
makes such a copy. Later chapters keep secrets in caller-owned buffers for
this reason.

## The example

The example is
[bitcoin/examples/constant-time-rules.coax](../examples/constant-time-rules.coax).
It compares a 32-byte tag both ways, builds masks, selects, runs one
erasure scope from registration to completion, and shows four refusals. It
also defines `leaky_pick` and `masked_pick`, which the source audit checks
below.

<!-- source: bitcoin/examples/constant-time-rules.coax example -->
```jai
#load "../../coax/lib/crypto/bytes.coax";
#load "../../coax/lib/crypto/zeroize.coax";
#import "Basic";

// A leaky comparison. It stops at the first byte that differs, so the
// number of bytes it reads tells an observer where the first difference is.
// It returns that number so you can see the leak. Never compare secrets
// this way.
leaky_bytes_read :: (a: [] u8, b: [] u8) -> int {
  for i: 0..<a.count { if a[i] != b[i] { return i + 1; } }
  return a.count;
}

// Two ways to choose a or b from a secret bit. The first branches on the
// bit; the second builds a mask and never branches.
leaky_pick :: (bit: u64, a: u64, b: u64) -> u64 { return ifx bit == 1 then a else b; }
masked_pick :: (bit: u64, a: u64, b: u64) -> u64 { return cb_ct_select64(cb_mask(bit), a, b); }

ZERO_NAMES :: [10] string.["ok", "capacity", "range", "address", "alias",
  "state", "effect", "unknown", "reason", "pending"];

main :: () {
  // A 32-byte secret tag and three guesses: right, wrong in byte 0, and
  // wrong in byte 31.
  tag: [32] u8;
  for i: 0..<32 { tag[i] = cast(u8) (i * 7 + 3); }
  right := tag; first := tag; last := tag;
  first[0] ^= 1; last[31] ^= 1;

  print("leaky compare, wrong byte 0: read % bytes\n", leaky_bytes_read(tag[..], first[..]));
  print("leaky compare, wrong byte 31: read % bytes\n", leaky_bytes_read(tag[..], last[..]));

  // cb_ct_equal reads every byte and returns a mask: all ones or zero.
  print("cb_ct_equal, right guess: %\n", cb_ct_equal(tag[..], right[..]));
  print("cb_ct_equal, wrong byte 0: %\n", cb_ct_equal(tag[..], first[..]));
  print("cb_ct_equal, wrong byte 31: %\n", cb_ct_equal(tag[..], last[..]));

  // A mask from a bit, and a select with no branch.
  print("cb_mask(1) = %, cb_mask(0) = %\n", cb_mask(1), cb_mask(0));
  print("select(all ones, 15, 240) = %\n", cb_ct_select64(cb_mask(1), 15, 240));
  print("select(zero, 15, 240) = %\n", cb_ct_select64(cb_mask(0), 15, 240));
  print("masked_pick(1, 15, 240) = %\n", masked_pick(1, 15, 240));

  // Erasure bookkeeping. Register the tag as a 32-byte secret, close the
  // scope, and acknowledge the wipe. The scope is complete only after every
  // registered secret is acknowledged.
  s: ZeroScope;
  c := zero_scope_register(s, 1, 32, 32, true, false);
  s = c.scope;
  c = zero_scope_begin(s, 0);
  s = c.scope;
  print("after begin: % bytes pending, complete %\n", zero_scope_pending(s, 0), zero_scope_complete(s));
  for i: 0..<32 { tag[i] = 0; }
  c = zero_scope_ack(s, 1, ZERO_OK);
  s = c.scope;
  print("after the wipe: % bytes pending, complete %\n", zero_scope_pending(s, 0), zero_scope_complete(s));

  // Refusals.
  print("erase 4 bytes at offset 30 of 32: %\n", ZERO_NAMES[zero_admit(32, 30, 4, true)]);
  print("register a secret that overlaps another: %\n",
    ZERO_NAMES[zero_scope_register(ZeroScope.{}, 1, 32, 32, true, true).error]);
  open: ZeroScope;
  open = zero_scope_register(open, 1, 32, 32, true, false).scope;
  open = zero_scope_begin(open, 0).scope;
  print("a failed wipe: %\n", ZERO_NAMES[zero_scope_ack(open, 1, 77).error]);
  print("unequal lengths compare as: %\n", cb_ct_equal(right[..], right[0..<31]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/constant-time-rules.coax
```

```text
leaky compare, wrong byte 0: read 1 bytes
leaky compare, wrong byte 31: read 32 bytes
cb_ct_equal, right guess: 18446744073709551615
cb_ct_equal, wrong byte 0: 0
cb_ct_equal, wrong byte 31: 0
cb_mask(1) = 18446744073709551615, cb_mask(0) = 0
select(all ones, 15, 240) = 15
select(zero, 15, 240) = 240
masked_pick(1, 15, 240) = 15
after begin: 32 bytes pending, complete false
after the wipe: 0 bytes pending, complete true
erase 4 bytes at offset 30 of 32: range
register a secret that overlaps another: alias
a failed wipe: effect
unequal lengths compare as: 0
```

The output matches the values worked out by hand:

- The early-exit comparison reads 1 byte or 32 bytes, depending on where the
  guess is wrong. `cb_ct_equal` gives all ones for the right guess and 0 for
  both wrong ones.
- The selections return 15 and 240.
- After `zero_scope_begin`, 32 bytes are pending and the scope isn't
  complete. After the wipe is acknowledged, nothing is pending and the scope
  is complete.

### The refusals

The last four lines are refusals:

- `zero_admit` refuses to erase 4 bytes at offset 30 of a 32-byte buffer
  with `range`, because the range ends 2 bytes past the buffer.
- `zero_scope_register` refuses a buffer that overlaps another with
  `alias`.
- `zero_scope_ack` with a non-zero host status reports `effect`. The scope
  stays incomplete.
- `cb_ct_equal` returns 0 for inputs of different lengths. The length is
  public, so this check may branch.

## How the code is checked

### The example's own checks

The example compares `cb_ct_equal` with an independent model: one flipped
bit at any of the 256 bit positions makes the tags unequal, and the
early-exit comparison reads exactly up to that byte. It also checks the
selection worked out by hand:

<!-- source: bitcoin/examples/constant-time-rules.coax check -->
```jai
// cb_ct_equal against an independent model: a byte-by-byte comparison of the
// whole slice. Every single-bit difference at every position is refused.
#forall "one flipped bit anywhere makes the tags unequal" at: 0..<32, bit: 0..<8 {
  a: [32] u8; b: [32] u8;
  for i: 0..<32 { a[i] = cast(u8) (i * 7 + 3); b[i] = a[i]; }
  #assert cb_ct_equal(a[..], b[..]) == CB_MAX64;
  b[at] ^= cast(u8) (1 << bit);
  #assert cb_ct_equal(a[..], b[..]) == 0;
  #assert leaky_bytes_read(a[..], b[..]) == at + 1;
}
// The hand-worked select: b ^ ((a ^ b) & mask).
#test "select by hand" {
  #assert (15 ^ 240) == 255;
  #assert (240 ^ (255 & CB_MAX64)) == 15;
  #assert cb_ct_select64(CB_MAX64, 15, 240) == 15 && cb_ct_select64(0, 15, 240) == 240;
  #assert cb_mask(3) == CB_MAX64 && cb_mask(2) == 0;
  #assert leaky_pick(1, 15, 240) == 15 && leaky_pick(0, 15, 240) == 240;
  #assert masked_pick(1, 15, 240) == 15 && masked_pick(0, 15, 240) == 240;
}
```

### The source audit refuses a branch on a secret

The [secret-path audit](../../tools/coax-nostr/secret-check/README.md)
parses the source with the real Coax frontend. It treats every parameter of
the entry function as secret, follows each value through locals and calls,
and refuses a secret branch, index, variable shift or `_vartime` call.

Run it on the two functions in the example:

```shell
python3 tools/coax-nostr/secret-check/check.py bitcoin/examples/constant-time-rules.coax --entry leaky_pick
python3 tools/coax-nostr/secret-check/check.py bitcoin/examples/constant-time-rules.coax --entry masked_pick
```

The first command exits with status 1. Its JSON report says:

```text
"accepted": false,
"error": 6,
"error_name": "secret-branch",
"line": 17,
"column": 58,
```

Line 17, column 58 is the `ifx` whose condition is `bit == 1`. The second
command exits with status 0 and reports `"accepted": true` for
`masked_pick`, which reaches `cb_mask` and `cb_ct_select64` and never
branches.

The audit is a source policy. It doesn't check the machine code, and it
supports only `u32`, `u64` and `bool` values, not arrays or loops.

### The machine code has no conditional branch

[coax/tests/constant_time.py](../../coax/tests/constant_time.py) compiles a
masked select in debug and release modes, disassembles it, and checks that it
contains no conditional branch instruction (`b.cond`, `cbz`, `cbnz`, `tbz` or
`tbnz` on arm64, `jcc` on x86-64). It also checks the wasm build.

### Timing measurements

[tools/coax-nostr/timing/](../../tools/coax-nostr/timing/README.md) runs a
fixed-versus-random Welch t-test in the style of dudect. It times 1,000,000
calls of each of three fixtures: fixed work, a deliberate branch on the input
and a deliberate input-dependent table index. A result with |t| ≥ 10 is a
detected difference.

| Target | Fixed work | Planted branch | Planted index |
| --- | ---: | ---: | ---: |
| Apple M5 Max, arm64, 2026-10-28 | 0.18 | −328.27 | −771.73 |
| Core i7-14700K, x86-64, 2026-10-28 | −0.13 | −683.16 | −1,395.26 |
| Chrome 156, wasm, 2026-10-28 | 1.80 | −327.23 | −483.59 |
| Apple M5 Max, arm64, 2026-10-31, loaded machine | −0.09 | −593.58 | −39.84 |

Both planted leaks are detected on every target, and the fixed-work fixture
never crosses the threshold. The first three rows are the recorded evidence
in
[controls-2026-10-28.json](../../tools/coax-nostr/timing/evidence/controls-2026-10-28.json).
The last row is a rerun for this chapter.

A result below the threshold means only that this experiment found no
difference. It isn't a proof of constant time.

### Mutation testing

To run the checks:

```shell
coax/bin/coax test bitcoin/examples/constant-time-rules.coax
coax/bin/coax mutate bitcoin/examples/constant-time-rules.coax
coax/bin/coax mutate coax/lib/crypto/zeroize.coax
python3 coax/tests/constant_time.py
python3 tools/coax-nostr/secret-check/test_secret.py
python3 tools/coax-nostr/timing/run.py
```

`coax test` on the example runs 21 checks, including the checks in the loaded
files. `coax mutate` kills every mutant: 7 in the example, 145 in
`bytes.coax` and 252 in `zeroize.coax`, with zero survivors. The audit tests
`test_secret.py` and `test_revalidate.py` pass.

## What these checks don't claim

- They don't prove constant time on every CPU, compiler flag or JIT tier.
- They don't cover power, electromagnetic or second-order cache observers.
- Erasure doesn't reach register copies, stack spills or operating-system
  copies.
- None of them approves a key for production use.

## Summary

- A secret never decides a branch, never selects a memory address and never
  reaches a variable-time routine.
- `cb_ct_equal` reads every byte and returns a mask. `cb_mask` and
  `cb_ct_select64` choose a value with the same instructions either way, and
  `value_barrier` keeps the optimizer from turning the mask into a branch.
- `zeroize.coax` tracks every secret buffer until the host acknowledges its
  wipe, and the host erases with volatile writes.
- A source audit, a machine-code check and timing measurements with leaky
  controls each check part of the rules. None of them is a proof.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [Coax chapter 23, What verification
  establishes](../../coax/book/ch23-verification-boundaries.md)

## Sources

- [dudect: dude, is my code constant time?](https://eprint.iacr.org/2016/1123),
  the fixed-versus-random t-test method.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  which mixes auxiliary random data into the signing nonce to protect
  against side-channel attacks.
- [NIP-44](../../nostr/nips/official/44.md), which requires a constant-time MAC
  comparison.
- The Coax specification on `value_barrier`: [coax/spec.md](../../coax/spec.md).
- The timing harness: [tools/coax-nostr/timing/README.md](../../tools/coax-nostr/timing/README.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
