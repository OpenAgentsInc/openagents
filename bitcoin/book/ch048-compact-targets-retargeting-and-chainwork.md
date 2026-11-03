# 48. Compact targets, retargeting and chainwork

Bitcoin aims for one block every ten minutes, but the hash rate that mines
it changes all the time. Every 2,016 blocks, each node computes a new
target from the time the last 2,016 blocks took: if they came too fast,
the target falls and blocks get harder; if too slow, it rises. The rule
fits in thirty lines of Bitcoin Core's `pow.cpp`, and its details matter:
a clamp of four in each direction, a cap at `powLimit`, rounding through
chapter 47's compact form, and an off-by-one that makes the time-warp
attack possible.

This chapter builds Core's `GetNextWorkRequired`,
`CalculateNextWorkRequired` and `PermittedDifficultyTransition` for
mainnet, with testnet's minimum-difficulty rule and BIP-94's change, and
chainwork, the sum of every header's work. It checks every mainnet
retarget from block 2,016 to block 973,728 against the chain, the
chainwork to block 966,143 against the `nMinimumChainWork` that Bitcoin
Core ships, Core's own tests and a Python transliteration of Core,
natively and in Wasm.

## What this chapter teaches

- The retarget interval (2,016 blocks) and the target timespan (two
  weeks), and where Core reads the two timestamps.
- The clamp: a period that took less than a quarter of two weeks counts as
  a quarter, and one that took more than four times counts as four.
- The off-by-one: the window from the first to the last block of a period
  measures 2,015 gaps, not 2,016.
- How the new target is rounded down through the compact form and capped
  at `powLimit`.
- The time-warp attack, why the off-by-one allows it, and the status of
  BIP-54's fix.
- Testnet3's 20-minute rule, the difficulty storms it caused, and BIP-94's
  change in testnet4.
- `PermittedDifficultyTransition`, the bound headers presync uses before
  it has whole periods.
- Chainwork, and how one number in Core's source checks every retarget.

## Where Bitcoin uses retargeting

| Where | What it uses | Chapter |
| --- | --- | --- |
| `ContextualCheckBlockHeader`: "bad-diffbits" | `GetNextWorkRequired` for the header's parent | this chapter, 51 |
| Headers presync | `PermittedDifficultyTransition` on each header | this chapter, 72 |
| Chain selection | chainwork, the sum of every header's work | this chapter, 59 |
| `nMinimumChainWork` | the chainwork a peer's headers must reach before a node stores them | this chapter, 50, 72 |
| Testnet4's timewarp rule: "time-timewarp-attack" | the first block of a period against the last of the previous | 51 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [retarget.coax](../../coax/lib/bitcoin/retarget.coax) | `RetargetParams`, `RETARGET_MAINNET`, `RETARGET_TESTNET3`, `RETARGET_TESTNET4`, `RETARGET_REGTEST` | the fields of Core's `Consensus::Params` that retargeting reads |
| | `u256_mul32`, `u256_div32_vartime` | Core's `operator*=(uint32_t)`, and division by a word |
| | `retarget_params_check_vartime`, `retarget_interval` | the parameters' sanity checks, and `DifficultyAdjustmentInterval` |
| | `retarget_calculate_vartime(p, last, first)` | Core's `CalculateNextWorkRequired` |
| | `retarget_next_bits_vartime(p, period, last_height, new_time)` | Core's `GetNextWorkRequired` |
| | `retarget_check_vartime(p, period, last_height, header)` | the "bad-diffbits" rule of `ContextualCheckBlockHeader` |
| | `retarget_permitted_vartime(p, height, old_bits, new_bits)` | Core's `PermittedDifficultyTransition` |
| | `retarget_chain_work_vartime(total, bits, count)` | chainwork: `count` more blocks of one `nBits` |
| [pow.coax](../../coax/lib/bitcoin/pow.coax) | `U256`, `u256_div_vartime`, `pow_compact_decode_vartime`, `pow_compact_encode_vartime`, `pow_work_vartime` | chapter 47's numbers, compact form and work |

Targets and timestamps are public, so every procedure that decides runs in
variable time and its name ends in `_vartime` (chapter 3).

Every procedure that checks returns one of these codes:

<!-- source: coax/lib/bitcoin/retarget.coax codes -->
```jai
// Result codes.
RETARGET_OK :: 0;
RETARGET_BAD_DIFFBITS :: 1;   // nBits is not what the rule requires: Core's "bad-diffbits"
RETARGET_NOT_PERMITTED :: 2;  // presync: a change of more than four times, or off a boundary
RETARGET_PERIOD :: 3;         // the view is not the current period's blocks, or a height is negative
RETARGET_PARAMS :: 4;         // the parameters fail Core's sanity checks
```

## The parameters

Retargeting reads six fields of Core's `Consensus::Params`. Chapter 50
builds each network's whole parameter set; this chapter defines the
retargeting fields of mainnet, testnet3, testnet4 and regtest so that the
rules can be tested. The interval is the timespan over the spacing:
1,209,600 / 600 = 2,016 blocks.

<!-- source: coax/lib/bitcoin/retarget.coax params -->
```jai
// The fields of Core's Consensus::Params that retargeting reads.
RetargetParams :: struct {
  pow_limit: U256;           // powLimit: the easiest target
  spacing: int;              // nPowTargetSpacing: seconds per block
  timespan: int;             // nPowTargetTimespan: seconds per period
  allow_min_difficulty: bool; // fPowAllowMinDifficultyBlocks: testnet3, testnet4
  bip94: bool;               // enforce_BIP94: testnet4
  no_retargeting: bool;      // fPowNoRetargeting: regtest
}
TWO_WEEKS :: 14 * 24 * 60 * 60;
RETARGET_MAINNET :: RetargetParams.{pow_limit=POW_LIMIT_MAINNET, spacing=600, timespan=TWO_WEEKS};
RETARGET_TESTNET3 :: RetargetParams.{pow_limit=POW_LIMIT_MAINNET, spacing=600, timespan=TWO_WEEKS, allow_min_difficulty=true};
RETARGET_TESTNET4 :: RetargetParams.{pow_limit=POW_LIMIT_MAINNET, spacing=600, timespan=TWO_WEEKS, allow_min_difficulty=true, bip94=true};
// Regtest: powLimit 2^255 - 1, a one-day timespan, and no retargeting.
RETARGET_REGTEST :: RetargetParams.{pow_limit=U256.{limb=[8]u32.[0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0x7fffffff]}, spacing=600, timespan=24 * 60 * 60, allow_min_difficulty=true, no_retargeting=true};
```

Core multiplies a target by the measured time with `operator*=(uint32_t)`:
each limb times the word, plus the carry, with the carry out of the top
limb lost. The division that follows is by the timespan, which fits in a
word, so this chapter divides limb by limb, as by hand, instead of using
chapter 47's shift-and-subtract division. The quotient is the same; the
Python harness checks both against Python's integers.

<!-- source: coax/lib/bitcoin/retarget.coax mul -->
```jai
// Core's operator*=(uint32_t): each limb times the word plus the carry.
// The carry out of the top limb is lost, so the product wraps at 2^256.
u256_mul32 :: (a: U256, b: u32) -> U256 #export_c {
  r: U256; carry := cast(u64) 0;
  for i: 0..<8 {
    wide := cast(u64) a.limb[i] * cast(u64) b + carry;
    r.limb[i] = cast,trunc(u32) wide; carry = wide >> 32;
  }
  return r;
}
// Division by a word, from the top limb down, as by hand: each step
// divides the remainder and one limb, a 64-bit number, by d. Core divides
// by the timespan with its general operator/= (chapter 47's shift and
// subtract); the quotient is the same, in eight steps instead of about
// two hundred. A zero divisor is not ok.
u256_div32_vartime :: (a: U256, d: u32) -> U256Quotient #export_c {
  r: U256Quotient;
  if d == 0 { return r; }
  r.ok = true;
  rem := cast(u64) 0;
  for k: 0..<8 {
    i := 7 - k;
    cur := (rem << 32) | cast(u64) a.limb[i];
    r.q.limb[i] = cast,trunc(u32) (cur / cast(u64) d);
    rem = cur % cast(u64) d;
  }
  return r;
}
```

Before it uses parameters, the code checks what Core's
`sanity_check_chainparams` test checks for every network: the timespan is
a whole number of blocks, and `powLimit` times four timespans still fits in
256 bits. Core multiplies by a `uint32_t`, so four timespans must fit one.
Any failure is `RETARGET_PARAMS`.

<!-- source: coax/lib/bitcoin/retarget.coax sanity -->
```jai
// The checks Core's test/pow_tests.cpp makes on every network's parameters
// (sanity_check_chainparams) that concern retargeting: the timespan is a
// whole number of blocks, and four times the largest target still fits in
// 256 bits after the multiplication. Core multiplies by a uint32_t, so
// four timespans must fit one.
retarget_params_check_vartime :: (p: RetargetParams) -> int #export_c {
  if p.spacing <= 0 || p.timespan <= 0 || p.timespan % p.spacing != 0 { return RETARGET_PARAMS; }
  if u256_is_zero(p.pow_limit) || p.timespan > 0xffffffff / 4 { return RETARGET_PARAMS; }
  if !p.no_retargeting {
    q := u256_div32_vartime(u256_not(U256.{}), cast(u32) (p.timespan * 4));
    if u256_compare_vartime(p.pow_limit, q.q) >= 0 { return RETARGET_PARAMS; }
  }
  return RETARGET_OK;
}
// Core's DifficultyAdjustmentInterval: 2,016 on mainnet.
retarget_interval :: (p: RetargetParams) -> int #export_c {
  return ifx p.spacing > 0 then p.timespan / p.spacing else 0;
}
```

## The retarget

`CalculateNextWorkRequired` takes the last block of a period and the time
of its first block:

1. The measured time is `last.time - first.time`, a signed number. A miner
   can set a timestamp up to two hours ahead and down to the median of the
   last eleven blocks plus one second (chapter 51), so the measured time
   can even be negative.
2. It is clamped to between a quarter of the timespan, 302,400 seconds,
   and four times the timespan, 4,838,400 seconds. No single retarget
   changes the difficulty by more than four times.
3. The old target is multiplied by the measured time and divided by the
   timespan.
4. The result is capped at `powLimit`.
5. The result is encoded as `nBits`, which keeps the top three bytes and
   drops the rest. The encoding rounds down, so the new target is never
   easier than the exact one.

<!-- source: coax/lib/bitcoin/retarget.coax calculate -->
```jai
// The time and nBits of one block: what retargeting reads of a header.
BlockStamp :: struct { time: u32; bits: u32; }
// Core's CalculateNextWorkRequired, for parameters that pass the sanity
// check. last is the period's last block, first its first: the block
// interval - 1 heights earlier, so the period spans interval - 1 gaps
// between blocks, not interval. The time they took is clamped to between
// a quarter and four times the timespan; the target is multiplied by it,
// divided by the timespan, capped at powLimit and encoded, which rounds it
// down to three bytes of mantissa. BIP-94 starts from the first block's
// nBits instead of the last's.
retarget_calculate_vartime :: (p: RetargetParams, last: BlockStamp, first: BlockStamp) -> u32 #export_c {
  if p.no_retargeting { return last.bits; }
  actual := cast(int) last.time - cast(int) first.time;
  if actual < p.timespan / 4 { actual = p.timespan / 4; }    // mutant: equivalent (at the bound both give a quarter)
  if actual > p.timespan * 4 { actual = p.timespan * 4; }    // mutant: equivalent (at the bound both give four times)
  start := ifx p.bip94 then first.bits else last.bits;
  target := pow_compact_decode_vartime(start).target;
  target = u256_mul32(target, cast,trunc(u32) actual);
  q := u256_div32_vartime(target, cast(u32) p.timespan);
  #assert q.ok;    // the sanity check makes the timespan positive
  target = q.q;
  if u256_compare_vartime(target, p.pow_limit) > 0 { target = p.pow_limit; }    // mutant: equivalent (>= assigns the same number)
  return pow_compact_encode_vartime(target, false);
}
```

`GetNextWorkRequired` decides when that runs. Off a boundary, the new
block takes the last block's `nBits`. On a boundary, when the new block's
height is a multiple of 2,016, the first block of the closing period is
the ancestor at `last_height - 2015`. Our procedure takes a view of the
current period instead of a block index: the blocks from the last height
that is a multiple of 2,016 to the last block, oldest first. In both cases
that view has `last_height % 2016 + 1` blocks; any other length is
`RETARGET_PERIOD`.

<!-- source: coax/lib/bitcoin/retarget.coax next -->
```jai
// The nBits a block must have: Core's GetNextWorkRequired. period holds
// the blocks of the current period, oldest first: from the last height
// that is a multiple of the interval up to the last block, last_height.
// new_time is the new block's time, which only testnet's rule reads.
//
// - Off a boundary, nBits stays the last block's. On testnet3 and
//   testnet4 a block more than twice the spacing after the last may use
//   powLimit, and otherwise takes the nBits of the last block that did not:
//   the walk back stops at the period's first block.
// - On a boundary, the period's first and last blocks give the new target.
Retarget :: struct { code: int; bits: u32; }
retarget_next_bits_vartime :: (p: RetargetParams, period: [] BlockStamp, last_height: int, new_time: u32) -> Retarget #export_c {
  r: Retarget;
  r.code = retarget_params_check_vartime(p);
  if r.code != RETARGET_OK { return r; }
  interval := retarget_interval(p);
  if last_height < 0 || period.count != last_height % interval + 1 { r.code = RETARGET_PERIOD; return r; }
  last := period[period.count - 1];
  if (last_height + 1) % interval != 0 {
    r.bits = last.bits;
    if p.allow_min_difficulty {
      limit := pow_compact_encode_vartime(p.pow_limit, false);
      if cast(int) new_time > cast(int) last.time + p.spacing * 2 { r.bits = limit; return r; }
      i := period.count - 1;
      while i > 0 && period[i].bits == limit { i -= 1; }
      r.bits = period[i].bits;
    }
    return r;
  }
  r.bits = retarget_calculate_vartime(p, last, period[0]);
  return r;
}
// Core's ContextualCheckBlockHeader, its first rule: a header whose nBits
// is not the required one is refused as "bad-diffbits".
retarget_check_vartime :: (p: RetargetParams, period: [] BlockStamp, last_height: int, header: BlockStamp) -> int #export_c {
  r := retarget_next_bits_vartime(p, period, last_height, header.time);
  if r.code != RETARGET_OK { return r.code; }
  if header.bits != r.bits { return RETARGET_BAD_DIFFBITS; }
  return RETARGET_OK;
}
```

### Worked example: the first retarget

Blocks 0 to 32,255 all have the genesis target, `1d00ffff`. Blocks
30,240 to 32,255 took 1,022,578 seconds, 11.8 days, so block 32,256 gets a
smaller target. The example reads the pinned headers and computes it:

```text
block 32256: blocks 30240 to 32255 took 1022578 seconds, 11.8 days
  nBits 1d00ffff
  target × time / two weeks: 00000000d86a528bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8b
  encoded and decoded: 00000000d86a0000000000000000000000000000000000000000000000000000
  new nBits 1d00d86a, block 32256 has 1d00d86a, difficulty × 1.1828
```

The exact target is `0xffff × 256^26 × 1022578 / 1209600`. Its top three
bytes are `d86a52`, but `0xd8` has its top bit set, which the compact form
reads as the sign, so the encoder takes one byte more and keeps `00d86a`:
the rest is dropped. This is the case of Core's `get_next_work` test.

### Worked example: the cap and the clamp

The first 2,016 blocks took 23.8 days, longer than two weeks, so the
target would have risen above `powLimit`. The cap holds it at the
genesis target; this is Core's `get_next_work_pow_limit` test. The first
fifteen retargets of 2009 were all capped this way.

```text
block 2016: blocks 0 to 2015 took 2055491 seconds, 23.8 days
  nBits 1d00ffff
  target × time / two weeks: 00000001b30495e6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6
  above powLimit: capped
  encoded and decoded: 00000000ffff0000000000000000000000000000000000000000000000000000
  new nBits 1d00ffff, block 2016 has 1d00ffff, difficulty × 1.0000
```

One mainnet period has hit the clamp. In July 2010, as many new miners
arrived, blocks 66,528 to 68,543 took 289,434 seconds, 3.35
days, less than a quarter of two weeks. The retarget counted 302,400
seconds instead, and the difficulty rose by exactly four. This is Core's
`get_next_work_lower_limit_actual` test, and `make_retarget_vectors.py`
checks that no other period from genesis to block 973,728 hit the clamp in
either direction.

```text
block 68544: blocks 66528 to 68543 took 289434 seconds, 3.3 days
  under a quarter of two weeks: clamped to 302400
  nBits 1c05a3f4
  target × time / two weeks: 000000000168fd00000000000000000000000000000000000000000000000000
  encoded and decoded: 000000000168fd00000000000000000000000000000000000000000000000000
  new nBits 1c0168fd, block 68544 has 1c0168fd, difficulty × 4.0000
```

The largest fall in difficulty came at block 689,472, in July 2021, after
mining was banned in China: blocks 687,456 to 689,471 took 19.4 days and
the difficulty fell to 0.7205 of its value.

## The off-by-one and the time warp

The window from block `2016k` to block `2016k + 2015` holds 2,016 blocks
but only 2,015 gaps between them. Satoshi's code compares the time of those
2,015 gaps with two weeks, the time of 2,016. If every block comes exactly
ten minutes after the one before, the period measures 1,209,000 seconds
and the difficulty rises by 1/2015:

```text
2,016 blocks ten minutes apart measure 1209000 seconds: 1c05a3f4 becomes 1c05a33c, difficulty × 1.0004
```

That bias is small: on average blocks come about 0.3 seconds faster than
ten minutes. The worse effect is that periods do not overlap. The time of
the first block of a period is not tied to the last block of the period
before, so the two ends of the measured window can be chosen apart. That
is the *time-warp attack*: a majority of miners set every timestamp as low
as the median-time-past rule allows, rising one second every six blocks,
except the last block of each period, which they set to the real time.
Each period then measures the real time since the attack started, so the
difficulty falls every period, by up to four times, while the attackers'
hash rate stays the same.

The example simulates it from block 900,000's `nBits`:

```text
time warp from block 900,000's nBits, at a constant hash rate
  period 0: took 14.0 days, measured 14.0 days, next nBits 17023774
  period 1: took 14.0 days, measured 28.0 days, next nBits 17046ebf
  period 2: took 7.0 days, measured 35.0 days, next nBits 170b1450
  period 3: took 2.8 days, measured 37.8 days, next nBits 171de811
  period 4: took 1.0 days, measured 38.8 days, next nBits 1752ef5f
  period 5: took 9.0 hours, measured 39.2 days, next nBits 1800e82f
  period 6: took 3.2 hours, measured 39.3 days, next nBits 18028c2b
  period 7: took 1.1 hours, measured 39.4 days, next nBits 180729e2
  period 8: took 24.4 minutes, measured 39.4 days, next nBits 1814268d
  period 9: took 8.7 minutes, measured 39.4 days, next nBits 1838af56
  period 10: took 3.1 minutes, measured 39.4 days, next nBits 19009f73
  period 11: took 1.1 minutes, measured 39.4 days, next nBits 1901c07c
  24192 blocks in 39.4 days; on time it would be 5676 blocks
  with BIP-54's rule, after three periods: 17023ad9
```

In 39 days the attackers mine 24,192 blocks, more than four times the
5,676 the schedule allows, and collect their subsidies. The attack was used against
several altcoins that copied Bitcoin's code. It needs a majority of the
hash rate for weeks, in public, so on Bitcoin it remains a risk to remove
rather than an event.

The fix is to tie the first block of each period to the last block of the
one before. [BIP-54](https://github.com/bitcoin/bips/blob/master/bip-0054.md),
the Consensus Cleanup soft fork, proposes that the first block of a period
may not be more than 7,200 seconds (two hours) earlier than the block
before it, and that the last block of a period may not be earlier than its
first. With that rule the measured time follows real time, as the example's
last line shows: three periods later the difficulty is where it started.
BIP-54 is a proposal. The pinned Bitcoin Core, revision `ddea9196`, has no
deployment for it in `chainparams.cpp`, and enforces only the testnet4
version of the first rule (BIP-94, `MAX_TIMEWARP = 600` seconds in
`consensus/consensus.h`). Chapter 51 builds that rule with the other
timestamp rules.

## Testnet's rules

Testnet3 has the same parameters as mainnet, plus
`fPowAllowMinDifficultyBlocks`. When a new block's time is more than 20
minutes (twice the spacing) after its parent's, the block may have the
minimum difficulty, `powLimit`'s encoding `1d00ffff`. Otherwise it takes
the `nBits` of the last block that was not such a block, walking back no
further than the period's first block. Mainnet ignores the new block's
time.

```text
testnet
  20 minutes and 1 second after the last block: 1d00ffff
  20 minutes after, past three minimum blocks: 1c05a3f4
  mainnet keeps the last block's: 1d00ffff
  a period whose last block used the minimum: testnet3 1d00ffde, testnet4 1c05a33c
```

The rule has a flaw. The retarget starts from the last block's `nBits`.
If the last block of a period was a minimum-difficulty block, the next
period starts from `powLimit`, so the whole period has difficulty 1. On
testnet3 that let anyone mine thousands of blocks in minutes, the
*block storms*. BIP-94 (testnet4) starts the retarget from the first
block's `nBits` instead, which the 20-minute rule never touches, because
the walk back stops there. The example's last testnet line shows both: the
same period gives `1d00ffde` on testnet3 and `1c05a33c` on testnet4.

Testnet4's other parameters (its genesis block, `powLimit` and ports) are
chapter 50's; its timewarp rule is chapter 51's.

## Headers presync

A node that syncs headers first (chapter 72) downloads a peer's headers in
a cheap presync pass before it stores them, so a peer cannot fill its
memory with low-work headers. During presync the node checks each
header's `nBits` against its parent's with
`PermittedDifficultyTransition`, because it does not keep the period's
first block. Off a boundary `nBits` must not change. On a boundary the new
target must lie between the old one times a quarter and times four, each
capped at `powLimit` and rounded through the compact form the way the
retarget rounds. Testnet allows any transition. A transition outside the
range is `RETARGET_NOT_PERMITTED`.

<!-- source: coax/lib/bitcoin/retarget.coax permitted -->
```jai
// Core's PermittedDifficultyTransition, used while headers are presynced
// (chapter 72), before the timestamps of a whole period are known. At a
// boundary, new nBits must lie between the old target times a quarter and
// times four, each capped and rounded through the compact form as the
// retarget rounds; elsewhere nBits must not change. Testnet allows
// anything, because a minimum-difficulty block can come at any height.
retarget_permitted_vartime :: (p: RetargetParams, height: int, old_bits: u32, new_bits: u32) -> int #export_c {
  code := retarget_params_check_vartime(p);
  if code != RETARGET_OK { return code; }
  if height < 0 { return RETARGET_PERIOD; }
  if p.allow_min_difficulty { return RETARGET_OK; }
  if height % retarget_interval(p) != 0 {
    return ifx old_bits == new_bits then RETARGET_OK else RETARGET_NOT_PERMITTED;
  }
  observed := pow_compact_decode_vartime(new_bits).target;
  old := pow_compact_decode_vartime(old_bits).target;
  spans := [2] int.[p.timespan * 4, p.timespan / 4];
  for k: 0..<2 {
    t := u256_div32_vartime(u256_mul32(old, cast(u32) spans[k]), cast(u32) p.timespan).q;
    if u256_compare_vartime(t, p.pow_limit) > 0 { t = p.pow_limit; }    // mutant: equivalent (>= assigns the same number)
    bound := pow_compact_decode_vartime(pow_compact_encode_vartime(t, false)).target;    // mutant: equivalent (the target read back ignores the sign bit)
    side := u256_compare_vartime(observed, bound);
    if (k == 0 && side > 0) || (k == 1 && side < 0) { return RETARGET_NOT_PERMITTED; }
  }
  return RETARGET_OK;
}
```

```text
permitted at block 68,544 after 1c05a3f4
  1c0168fd: ok
  1c0168fc: not permitted
  1c168fd0: ok
  1c168fd1: not permitted
  1c05a3f4: ok
  block 68,545 changing it: not permitted
```

The rounding matters at the edges. Core's
`get_next_work_lower_limit_rounding` test starts from `1c05a3f5`, whose
mantissa is not a multiple of four: a quarter of it is `0x0168fd.4`, which
encodes as `1c0168fd`. If the bound were not rounded the same way, the
largest permitted increase would itself be refused.

## Chainwork

Chapter 47 computed the work of one header, `2^256 / (target + 1)`.
A block's chainwork is the sum of that work over every block from the
genesis block to it: Core's `nChainWork`, and the `chainwork` field of
`getblockheader`. Nodes follow the chain with the most chainwork, not the
longest. On mainnet every block of a period has the same `nBits`, so a
period adds 2,016 times one header's work.

<!-- source: coax/lib/bitcoin/retarget.coax chainwork -->
```jai
// Chainwork: the work of every header from genesis, Core's nChainWork and
// the chainwork getblockheader prints. Each block adds GetBitsProof of its
// nBits; this adds count blocks that share one nBits, as a mainnet period
// does. The sum wraps at 2^256, as Core's does; it would take 2^256 hashes.
retarget_chain_work_vartime :: (total: U256, bits: u32, count: u32) -> U256 #export_c {
  return u256_add(total, u256_mul32(pow_work_vartime(bits), count));
}
```

Bitcoin Core's source holds one chainwork. Each release sets
`nMinimumChainWork`, the least work a peer's headers must reach before a
node stores them, to the chainwork `getblockheader` prints for the block
named by `defaultAssumeValid`. At the pinned revision that block is
966,143. Its chainwork is the work of 479 whole periods and of the 480
blocks from 965,664 to 966,143, so it depends on every period's `nBits`:

```text
chainwork at block 966143
  ours: 000000000000000000000000000000000000000145ec036acc5ba740052af1a0
  nMinimumChainWork: 000000000000000000000000000000000000000145ec036acc5ba740052af1a0
```

The pinned headers give each period's `nBits`, and the retargets that
link them are checked one by one:

```text
mainnet retargets to block 973728: 483 of 483 match the chain
  difficulty rose 358 times, fell 110 times, stayed 15 times
```

The fifteen that stayed are the capped retargets of 2009.

## The result codes

| Code | Core's reject reason | Returned by |
| --- | --- | --- |
| `RETARGET_OK` | none | all |
| `RETARGET_BAD_DIFFBITS` | "bad-diffbits", "incorrect proof of work" | the check |
| `RETARGET_NOT_PERMITTED` | presync refuses the header | the permitted range |
| `RETARGET_PERIOD` | none: Core asserts on a missing ancestor | the next `nBits`, the check, the permitted range |
| `RETARGET_PARAMS` | none: Core's `sanity_check_chainparams` test | the sanity check, the next `nBits`, the check, the permitted range |

The parameters are checked before anything else, then the view, then the
rule.

## The audit

Nothing in the library computed a retarget or added work before this
chapter. Chapter 47's `pow.coax` already had the 256-bit numbers, the
compact form and the work of one header, and this chapter uses them; it
adds only multiplication and division by a word. Chapter 47's
`pow_wasm.cjs` loader takes an optional buffer size so that it can run
this chapter's whole-period cases too.

## The example

The example is
[bitcoin/examples/retargeting.coax](../examples/retargeting.coax). It
explains eight pinned boundaries step by step, checks every mainnet
retarget to block 973,728, adds the chainwork to block 966,143, shows the
off-by-one, simulates the time warp with and without BIP-54's rule, runs
testnet's rules, tests the permitted range at block 68,544 and shows the
refusals.

<!-- source: bitcoin/examples/retargeting.coax example -->
```jai
#load "../../coax/lib/bitcoin/retarget.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// A 256-bit number as 64 hex digits, most significant first, the way
// Core's GetHex prints it.
show_number :: (label: string, a: U256) {
  b: [32] u8; shown: [32] u8; text: [64] u8;
  #assert u256_to_hash_vartime(a, b[..]);
  for i: 0..<32 { shown[i] = b[31-i]; }
  r := hex_encode_checked(cast(string) shown[..], text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A 32-bit number as 8 hex digits.
put32 :: (v: u32) {
  b: [4] u8; text: [8] u8;
  for i: 0..<4 { b[i] = cast,trunc(u8) (v >> (8 * cast(u32) (3 - i))); }
  r := hex_encode_checked(cast(string) b[..], text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// Seconds as days, to a tenth.
put_days :: (seconds: int) {
  tenths := (seconds * 10 + 43200) / 86400;
  print("%.% days", tenths / 10, tenths % 10);
}
// Decode hex into out; every literal here is valid lowercase hex.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
CODES :: [5] string.["ok", "bad-diffbits", "not permitted", "not the period", "bad parameters"];
// The ratio of two targets, old over new, to four places, rounded down:
// how many times harder the new one is.
put_ratio :: (old: u32, new: u32) {
  a := pow_compact_decode_vartime(old).target; b := pow_compact_decode_vartime(new).target;
  q := u256_div_vartime(u256_mul32(a, 10000), b).q;
  v := cast(int) u256_low64(q);
  print("%.", v / 10000);
  for d: 0..<4 { print("%", (v / (ifx d == 0 then 1000 else ifx d == 1 then 100 else ifx d == 2 then 10 else 1)) % 10); }
}
// A span of time in days, hours or minutes, to a tenth.
put_span :: (seconds: int) {
  if seconds >= 86400 { put_days(seconds); return; }
  unit := ifx seconds >= 3600 then 3600 else 60;
  tenths := (seconds * 10 + unit / 2) / unit;
  print("%.% %", tenths / 10, tenths % 10, ifx unit == 3600 then "hours" else "minutes");
}
// One boundary from the pinned headers: what the period took and the
// target that follows, step by step.
explain :: (i: int) {
  hb: [240] u8;
  h := decode(RT_BOUNDARY_HEADERS[i], hb[..]);
  first := pow_header_read_vartime(h[0..<80]); last := pow_header_read_vartime(h[80..<160]);
  next := pow_header_read_vartime(h[160..<240]);
  height := RT_BOUNDARY_HEIGHT[i];
  actual := cast(int) last.time - cast(int) first.time;
  print("block %: blocks % to % took % seconds, ", height, height - 2016, height - 1, actual);
  put_days(actual); print("\n");
  clamped := actual;
  if clamped < TWO_WEEKS / 4 { clamped = TWO_WEEKS / 4; print("  under a quarter of two weeks: clamped to %\n", clamped); }
  if clamped > TWO_WEEKS * 4 { clamped = TWO_WEEKS * 4; print("  over four times two weeks: clamped to %\n", clamped); }
  old := pow_compact_decode_vartime(last.bits).target;
  raw := u256_div32_vartime(u256_mul32(old, cast(u32) clamped), TWO_WEEKS).q;
  print("  nBits "); put32(last.bits); print("\n");
  show_number("  target × time / two weeks", raw);
  if u256_compare_vartime(raw, POW_LIMIT_MAINNET) > 0 { print("  above powLimit: capped\n"); }
  bits := retarget_calculate_vartime(RETARGET_MAINNET, BlockStamp.{time=last.time, bits=last.bits}, BlockStamp.{time=first.time, bits=first.bits});
  show_number("  encoded and decoded", pow_compact_decode_vartime(bits).target);
  print("  new nBits "); put32(bits); print(", block % has ", height); put32(next.bits);
  print(", difficulty × "); put_ratio(last.bits, bits); print("\n");
}

main :: () {
  print("interval % blocks, timespan % seconds, clamp % to %\n", retarget_interval(RETARGET_MAINNET), TWO_WEEKS, TWO_WEEKS / 4, TWO_WEEKS * 4);

  // Pinned boundaries, header by header.
  for i: 0..<RT_BOUNDARY_COUNT { explain(i); }

  // Every retarget from 2,016 to the pinned height.
  agree := 0; up := 0; down := 0; same := 0;
  for k: 1..<RT_PERIODS + 1 {
    last := BlockStamp.{time=cast(u32) RT_LAST_TIME[k-1], bits=cast(u32) RT_BITS[k-1]};
    first := BlockStamp.{time=cast(u32) RT_FIRST_TIME[k-1], bits=cast(u32) RT_BITS[k-1]};
    bits := retarget_calculate_vartime(RETARGET_MAINNET, last, first);
    if bits == cast(u32) RT_BITS[k] && retarget_permitted_vartime(RETARGET_MAINNET, 2016 * k, last.bits, bits) == RETARGET_OK { agree += 1; }
    side := u256_compare_vartime(pow_compact_decode_vartime(bits).target, pow_compact_decode_vartime(last.bits).target);
    if side < 0 { up += 1; } else if side > 0 { down += 1; } else { same += 1; }
  }
  print("mainnet retargets to block %: % of % match the chain\n", 2016 * RT_PERIODS, agree, RT_PERIODS);
  print("  difficulty rose % times, fell % times, stayed % times\n", up, down, same);

  // Chainwork to Core's assumed-valid block.
  total := U256.{};
  for k: 0..<RT_ASSUME_VALID_HEIGHT / 2016 { total = retarget_chain_work_vartime(total, cast(u32) RT_BITS[k], 2016); }
  k := RT_ASSUME_VALID_HEIGHT / 2016;
  total = retarget_chain_work_vartime(total, cast(u32) RT_BITS[k], cast(u32) (RT_ASSUME_VALID_HEIGHT % 2016 + 1));
  print("chainwork at block %\n", RT_ASSUME_VALID_HEIGHT);
  show_number("  ours", total);
  show_number("  nMinimumChainWork", pw_number(RT_MIN_CHAIN_WORK));

  // The off-by-one: blocks exactly ten minutes apart.
  b: [2016] BlockStamp;
  for i: 0..<2016 { b[i] = BlockStamp.{time=cast(u32) (1000000 + 600 * i), bits=0x1c05a3f4}; }
  r := retarget_next_bits_vartime(RETARGET_MAINNET, b[..], 2016 * 34 - 1, 0);
  print("2,016 blocks ten minutes apart measure % seconds: ", cast(int) b[2015].time - cast(int) b[0].time);
  put32(0x1c05a3f4); print(" becomes "); put32(r.bits); print(", difficulty × "); put_ratio(0x1c05a3f4, r.bits); print("\n");

  // The time warp: every block's time as low as the median rule allows,
  // except each period's last, which shows the real time.
  print("time warp from block 900,000's nBits, at a constant hash rate\n");
  start := cast(u32) 0x17023774;
  t0 := pow_compact_decode_vartime(start).target;
  bits := start; real := 0; low := 0; blocks := 0;
  for period: 0..<12 {
    t := pow_compact_decode_vartime(bits).target;
    seconds := cast(int) u256_low64(u256_div_vartime(u256_mul32(t0, TWO_WEEKS), t).q);
    real += seconds; blocks += 2016;
    next := retarget_calculate_vartime(RETARGET_MAINNET, BlockStamp.{time=cast(u32) real, bits=bits}, BlockStamp.{time=cast(u32) low, bits=bits});
    print("  period %: took ", period); put_span(seconds); print(", measured "); put_span(real - low);
    print(", next nBits "); put32(next); print("\n");
    bits = next; low += 2016 / 6;
  }
  print("  % blocks in ", blocks); put_days(real); print("; on time it would be % blocks\n", real / 600);
  // BIP-54's rule: a period's first block is at most two hours before the
  // last block of the period before. Then the measured time is the real one.
  bits = start; real = 0; low = 0;
  for period: 0..<3 {
    t := pow_compact_decode_vartime(bits).target;
    real += cast(int) u256_low64(u256_div_vartime(u256_mul32(t0, TWO_WEEKS), t).q);
    bits = retarget_calculate_vartime(RETARGET_MAINNET, BlockStamp.{time=cast(u32) real, bits=bits}, BlockStamp.{time=cast(u32) low, bits=bits});
    low = real - 7200;
  }
  print("  with BIP-54's rule, after three periods: "); put32(bits); print("\n");

  // Testnet: the 20-minute rule, the walk back, and the storm BIP-94 ends.
  print("testnet\n");
  view := b[0..<10];
  for i: 0..<10 { view[i] = BlockStamp.{time=cast(u32) (5000 + 600 * i), bits=ifx i < 7 then cast(u32) 0x1c05a3f4 else 0x1d00ffff}; }
  r = retarget_next_bits_vartime(RETARGET_TESTNET3, view, 2016 * 7 + 9, view[9].time + 1201);
  print("  20 minutes and 1 second after the last block: "); put32(r.bits); print("\n");
  r = retarget_next_bits_vartime(RETARGET_TESTNET3, view, 2016 * 7 + 9, view[9].time + 1200);
  print("  20 minutes after, past three minimum blocks: "); put32(r.bits); print("\n");
  r = retarget_next_bits_vartime(RETARGET_MAINNET, view, 2016 * 7 + 9, view[9].time + 1201);
  print("  mainnet keeps the last block's: "); put32(r.bits); print("\n");
  for i: 0..<2016 { b[i] = BlockStamp.{time=cast(u32) (1000000 + 600 * i), bits=0x1c05a3f4}; }
  b[2015].bits = 0x1d00ffff;
  r3 := retarget_next_bits_vartime(RETARGET_TESTNET3, b[..], 2016 * 9 - 1, 0);
  r4 := retarget_next_bits_vartime(RETARGET_TESTNET4, b[..], 2016 * 9 - 1, 0);
  print("  a period whose last block used the minimum: testnet3 "); put32(r3.bits);
  print(", testnet4 "); put32(r4.bits); print("\n");

  // Headers presync: the range PermittedDifficultyTransition allows at 68,544.
  print("permitted at block 68,544 after 1c05a3f4\n");
  tries := [5] u32.[0x1c0168fd, 0x1c0168fc, 0x1c168fd0, 0x1c168fd1, 0x1c05a3f4];
  for i: 0..<5 {
    print("  "); put32(tries[i]);
    print(": %\n", CODES[retarget_permitted_vartime(RETARGET_MAINNET, 68544, 0x1c05a3f4, tries[i])]);
  }
  print("  block 68,545 changing it: %\n", CODES[retarget_permitted_vartime(RETARGET_MAINNET, 68545, 0x1c0168fd, 0x1c0168fe)]);

  // The refusals.
  print("refusals\n");
  for i: 0..<2016 { b[i] = BlockStamp.{time=cast(u32) RT_LAST_TIME[15], bits=0x1d00ffff}; }
  b[0].time = cast(u32) RT_FIRST_TIME[15];
  print("  block 32,256 claiming 1d00ffff: %\n", CODES[retarget_check_vartime(RETARGET_MAINNET, b[..], 32255, BlockStamp.{bits=0x1d00ffff})]);
  print("  block 32,256 claiming 1d00d86a: %\n", CODES[retarget_check_vartime(RETARGET_MAINNET, b[..], 32255, BlockStamp.{bits=0x1d00d86a})]);
  print("  a view one block short: %\n", CODES[retarget_check_vartime(RETARGET_MAINNET, b[0..<2015], 32255, BlockStamp.{bits=0x1d00d86a})]);
  bad := RETARGET_MAINNET; bad.timespan = TWO_WEEKS + 1;
  print("  a timespan that is not whole blocks: %\n", CODES[retarget_check_vartime(bad, b[..], 32255, BlockStamp.{bits=0x1d00d86a})]);
}
```

Its output:

```text
interval 2016 blocks, timespan 1209600 seconds, clamp 302400 to 4838400
block 2016: blocks 0 to 2015 took 2055491 seconds, 23.8 days
  nBits 1d00ffff
  target × time / two weeks: 00000001b30495e6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6ce6
  above powLimit: capped
  encoded and decoded: 00000000ffff0000000000000000000000000000000000000000000000000000
  new nBits 1d00ffff, block 2016 has 1d00ffff, difficulty × 1.0000
block 32256: blocks 30240 to 32255 took 1022578 seconds, 11.8 days
  nBits 1d00ffff
  target × time / two weeks: 00000000d86a528bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8bc8b
  encoded and decoded: 00000000d86a0000000000000000000000000000000000000000000000000000
  new nBits 1d00d86a, block 32256 has 1d00d86a, difficulty × 1.1828
block 68544: blocks 66528 to 68543 took 289434 seconds, 3.3 days
  under a quarter of two weeks: clamped to 302400
  nBits 1c05a3f4
  target × time / two weeks: 000000000168fd00000000000000000000000000000000000000000000000000
  encoded and decoded: 000000000168fd00000000000000000000000000000000000000000000000000
  new nBits 1c0168fd, block 68544 has 1c0168fd, difficulty × 4.0000
block 689472: blocks 687456 to 689471 took 1678665 seconds, 19.4 days
  nBits 170e1ef9
  target × time / two weeks: 0000000000000000001398ce3ce5fe5fe5fe5fe5fe5fe5fe5fe5fe5fe5fe5fe5
  encoded and decoded: 0000000000000000001398ce0000000000000000000000000000000000000000
  new nBits 171398ce, block 689472 has 171398ce, difficulty × 0.7205
block 901152: blocks 899136 to 901151 took 1215064 seconds, 14.1 days
  nBits 17023774
  target × time / two weeks: 000000000000000000023a043438ff4bb076c327ee3a9f65b216dd298e54a105
  encoded and decoded: 000000000000000000023a040000000000000000000000000000000000000000
  new nBits 17023a04, block 901152 has 17023a04, difficulty × 0.9955
block 969696: blocks 967680 to 969695 took 1209975 seconds, 14.0 days
  nBits 17021ec5
  target × time / two weeks: 000000000000000000021ef013b2cb2cb2cb2cb2cb2cb2cb2cb2cb2cb2cb2cb2
  encoded and decoded: 000000000000000000021ef00000000000000000000000000000000000000000
  new nBits 17021ef0, block 969696 has 17021ef0, difficulty × 0.9996
block 971712: blocks 969696 to 971711 took 1141962 seconds, 13.2 days
  nBits 17021ef0
  target × time / two weeks: 000000000000000000020093e48d159e26af37c048d159e26af37c048d159e26
  encoded and decoded: 0000000000000000000200930000000000000000000000000000000000000000
  new nBits 17020093, block 971712 has 17020093, difficulty × 1.0592
block 973728: blocks 971712 to 973727 took 1146119 seconds, 13.3 days
  nBits 17020093
  target × time / two weeks: 00000000000000000001e5ac7f60c3ea1c7fa5d83b6193f71d4fb2d90b6e94c7
  encoded and decoded: 00000000000000000001e5ac0000000000000000000000000000000000000000
  new nBits 1701e5ac, block 973728 has 1701e5ac, difficulty × 1.0553
mainnet retargets to block 973728: 483 of 483 match the chain
  difficulty rose 358 times, fell 110 times, stayed 15 times
chainwork at block 966143
  ours: 000000000000000000000000000000000000000145ec036acc5ba740052af1a0
  nMinimumChainWork: 000000000000000000000000000000000000000145ec036acc5ba740052af1a0
2,016 blocks ten minutes apart measure 1209000 seconds: 1c05a3f4 becomes 1c05a33c, difficulty × 1.0004
time warp from block 900,000's nBits, at a constant hash rate
  period 0: took 14.0 days, measured 14.0 days, next nBits 17023774
  period 1: took 14.0 days, measured 28.0 days, next nBits 17046ebf
  period 2: took 7.0 days, measured 35.0 days, next nBits 170b1450
  period 3: took 2.8 days, measured 37.8 days, next nBits 171de811
  period 4: took 1.0 days, measured 38.8 days, next nBits 1752ef5f
  period 5: took 9.0 hours, measured 39.2 days, next nBits 1800e82f
  period 6: took 3.2 hours, measured 39.3 days, next nBits 18028c2b
  period 7: took 1.1 hours, measured 39.4 days, next nBits 180729e2
  period 8: took 24.4 minutes, measured 39.4 days, next nBits 1814268d
  period 9: took 8.7 minutes, measured 39.4 days, next nBits 1838af56
  period 10: took 3.1 minutes, measured 39.4 days, next nBits 19009f73
  period 11: took 1.1 minutes, measured 39.4 days, next nBits 1901c07c
  24192 blocks in 39.4 days; on time it would be 5676 blocks
  with BIP-54's rule, after three periods: 17023ad9
testnet
  20 minutes and 1 second after the last block: 1d00ffff
  20 minutes after, past three minimum blocks: 1c05a3f4
  mainnet keeps the last block's: 1d00ffff
  a period whose last block used the minimum: testnet3 1d00ffde, testnet4 1c05a33c
permitted at block 68,544 after 1c05a3f4
  1c0168fd: ok
  1c0168fc: not permitted
  1c168fd0: ok
  1c168fd1: not permitted
  1c05a3f4: ok
  block 68,545 changing it: not permitted
refusals
  block 32,256 claiming 1d00ffff: bad-diffbits
  block 32,256 claiming 1d00d86a: ok
  a view one block short: not the period
  a timespan that is not whole blocks: bad parameters
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/retargeting.out](../fixtures/retargeting.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/retargeting.coax](../examples/checks/retargeting.coax),
which loads the example:

- `pow_tests.cpp`'s six `CalculateNextWorkRequired` cases, each also
  permitted, and the two one-step-too-far transitions;
- every mainnet retarget from block 2,016 to block 973,728: computed, permitted
  and canonical, and every 40th also through a whole period's view, with a
  wrong `nBits` and a short view refused;
- the eight pinned boundaries header by header, block 966,143's hash and
  the chainwork to it;
- the example's verdicts.

<!-- source: bitcoin/examples/checks/retargeting.coax check -->
```jai
// A number from 64 hex digits, most significant first.
number :: (text: string) -> U256 {
  b: [32] u8; r: [32] u8;
  x := decode(text, b[..]);
  for i: 0..<32 { r[i] = x[31-i]; }
  return u256_from_hash_vartime(r[..]);
}
same :: (a: U256, b: U256) -> bool { return u256_compare_vartime(a, b) == 0; }
// A mainnet period's ends.
stamp_last :: (k: int) -> BlockStamp { return BlockStamp.{time=cast(u32) RT_LAST_TIME[k], bits=cast(u32) RT_BITS[k]}; }
stamp_first :: (k: int) -> BlockStamp { return BlockStamp.{time=cast(u32) RT_FIRST_TIME[k], bits=cast(u32) RT_BITS[k]}; }
// pow_tests.cpp's CalculateNextWorkRequired cases, each also permitted.
#test "Core's pow_tests" {
  for i: 0..<RT_CORE_COUNT {
    last := BlockStamp.{time=cast(u32) RT_CORE_LAST_TIME[i], bits=cast(u32) RT_CORE_LAST_BITS[i]};
    first := BlockStamp.{time=cast(u32) RT_CORE_FIRST_TIME[i]};
    #assert retarget_calculate_vartime(RETARGET_MAINNET, last, first) == cast(u32) RT_CORE_EXPECTED[i];
    #assert retarget_permitted_vartime(RETARGET_MAINNET, RT_CORE_HEIGHT[i] + 1, last.bits, cast(u32) RT_CORE_EXPECTED[i]) == RETARGET_OK;
  }
  #assert retarget_permitted_vartime(RETARGET_MAINNET, 68544, 0x1c05a3f4, 0x1c0168fc) == RETARGET_NOT_PERMITTED;
  #assert retarget_permitted_vartime(RETARGET_MAINNET, 46368, 0x1c387f6f, 0x1d00e1fe) == RETARGET_NOT_PERMITTED;
}
// Every retarget from block 2,016 to block 973,728: computed from the
// period's first and last blocks, and through a whole period's view. Each
// is permitted and canonical, and one block more or less fails.
#test "every mainnet retarget" #steps 200000000 {
  b: [2016] BlockStamp;
  for k: 1..<RT_PERIODS + 1 {
    want := cast(u32) RT_BITS[k];
    #assert retarget_calculate_vartime(RETARGET_MAINNET, stamp_last(k-1), stamp_first(k-1)) == want;
    #assert retarget_permitted_vartime(RETARGET_MAINNET, 2016 * k, stamp_last(k-1).bits, want) == RETARGET_OK;
    #assert pow_compact_encode_vartime(pow_compact_decode_vartime(want).target, false) == want;
    if k % 40 == 1 {
      for i: 0..<2016 { b[i] = ifx i == 0 then stamp_first(k-1) else stamp_last(k-1); }
      #assert retarget_check_vartime(RETARGET_MAINNET, b[..], 2016 * k - 1, BlockStamp.{bits=want}) == RETARGET_OK;
      #assert retarget_check_vartime(RETARGET_MAINNET, b[..], 2016 * k - 1, BlockStamp.{bits=want + 1}) == RETARGET_BAD_DIFFBITS;
      #assert retarget_check_vartime(RETARGET_MAINNET, b[0..<2015], 2016 * k - 1, BlockStamp.{bits=want}) == RETARGET_PERIOD;
    }
  }
}
// The pinned headers at eight boundaries: each meets its target, the block
// at the boundary follows the period's last block, and the retarget gives
// its nBits. Block 966,143 is chainparams.cpp's defaultAssumeValid, and the
// chainwork to it is nMinimumChainWork.
#test "mainnet headers and chainwork" #steps 100000000 {
  hb: [240] u8; hash: [32] u8;
  for i: 0..<RT_BOUNDARY_COUNT {
    h := decode(RT_BOUNDARY_HEADERS[i], hb[..]);
    for j: 0..<3 { #assert pow_header_check_vartime(h[80*j..<80*j+80], POW_LIMIT_MAINNET) == POW_OK; }
    #assert pow_links_vartime(h[160..<240], h[80..<160]);
    #assert pow_block_hash_vartime(h[160..<240], hash[..]) == POW_OK && pw_shown_is(hash[..], RT_BOUNDARY_NEXT_HASH[i]);
    a := pow_header_read_vartime(h[0..<80]); z := pow_header_read_vartime(h[80..<160]);
    #assert retarget_calculate_vartime(RETARGET_MAINNET, BlockStamp.{time=z.time, bits=z.bits}, BlockStamp.{time=a.time, bits=a.bits}) == pow_header_read_vartime(h[160..<240]).bits;
  }
  h := decode(RT_ASSUME_VALID_HEADER, hb[..]);
  #assert pow_block_hash_vartime(h, hash[..]) == POW_OK && pw_shown_is(hash[..], RT_ASSUME_VALID_HASH);
  total := U256.{};
  for k: 0..<RT_ASSUME_VALID_HEIGHT / 2016 { total = retarget_chain_work_vartime(total, cast(u32) RT_BITS[k], 2016); }
  total = retarget_chain_work_vartime(total, pow_header_read_vartime(h).bits, cast(u32) (RT_ASSUME_VALID_HEIGHT % 2016 + 1));
  #assert same(total, number(RT_MIN_CHAIN_WORK));
  #assert same(number("000000000000000000000000000000000000000145ec036acc5ba740052af1a0"), total);
  #assert !same(number("000000000000000000000000000000000000000145ec036acc5ba740052af1a1"), total);
}
// The example's verdicts: the off-by-one, the time warp, testnet's rules,
// the permitted range at 68,544 and the refusals.
#test "the example's verdicts" #steps 20000000 {
  b: [2016] BlockStamp;
  for i: 0..<2016 { b[i] = BlockStamp.{time=cast(u32) (1000000 + 600 * i), bits=0x1c05a3f4}; }
  #assert retarget_next_bits_vartime(RETARGET_MAINNET, b[..], 2016 * 34 - 1, 0).bits == 0x1c05a33c;
  b[2015].bits = 0x1d00ffff;
  #assert retarget_next_bits_vartime(RETARGET_TESTNET3, b[..], 2016 * 9 - 1, 0).bits == 0x1d00ffde;
  #assert retarget_next_bits_vartime(RETARGET_TESTNET4, b[..], 2016 * 9 - 1, 0).bits == 0x1c05a33c;
  // The first steps of the time warp: two weeks measured as four, then
  // 604,885 seconds (twice the target, rounded down) measured as five weeks.
  #assert retarget_calculate_vartime(RETARGET_MAINNET, BlockStamp.{time=2 * TWO_WEEKS, bits=0x17023774}, BlockStamp.{time=336}) == 0x17046ebf;
  #assert retarget_calculate_vartime(RETARGET_MAINNET, BlockStamp.{time=2 * TWO_WEEKS + 604885, bits=0x17046ebf}, BlockStamp.{time=672}) == 0x170b1450;
  want := [5] int.[RETARGET_OK, RETARGET_NOT_PERMITTED, RETARGET_OK, RETARGET_NOT_PERMITTED, RETARGET_OK];
  tries := [5] u32.[0x1c0168fd, 0x1c0168fc, 0x1c168fd0, 0x1c168fd1, 0x1c05a3f4];
  for i: 0..<5 { #assert retarget_permitted_vartime(RETARGET_MAINNET, 68544, 0x1c05a3f4, tries[i]) == want[i]; }
  #assert retarget_permitted_vartime(RETARGET_MAINNET, 68545, 0x1c0168fd, 0x1c0168fe) == RETARGET_NOT_PERMITTED;
  bad := RETARGET_MAINNET; bad.timespan = TWO_WEEKS + 1;
  #assert retarget_check_vartime(bad, b[..], 32255, BlockStamp.{bits=0x1d00d86a}) == RETARGET_PARAMS;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/retargeting.coax
```

`coax test` runs 88 checks, including those of the files it loads, in about 13 seconds on 2026-11-03. Mutation of this file was not run (owner speed order).

### The library's own checks

`retarget.coax` is an `#authority` file. Its own checks cover:

- multiplication by a word, its carries and its wrap, and division by a
  word against chapter 47's division for 64 dividends and four divisors;
- the parameters' sanity checks on all four networks and on each broken
  field, at the largest allowed timespan and `powLimit`;
- Core's `pow_tests.cpp` retarget and `PermittedDifficultyTransition`
  cases, and its `get_next_work_first_block_time` case on a whole period;
- the view's refusals, the clamps at and past each bound, the cap, the
  rounding, regtest and BIP-94;
- the off-by-one, testnet's 20-minute rule on both sides of 1,200
  seconds, the walk back and the storm;
- every mainnet retarget, the chainwork to block 966,143, the eight
  pinned boundaries and block 966,143's header.

```shell
coax/bin/coax test coax/lib/bitcoin/retarget.coax
coax/bin/coax mutate coax/lib/bitcoin/retarget.coax --root-only
```

`coax test` runs 81 checks, including chapter 47's, in about 16 seconds on 2026-11-03. Root-only mutation on 2026-11-02 made 176 mutants: it killed 171, and five are marked equivalent in the source with their reason (the clamps and the caps at their bounds assign the same value, and the target read back from an encoding ignores the sign bit). None survived.

### Core in Python, natively and in Wasm

[retarget_reference.py](../../coax/tests/bitcoin/retarget_reference.py)
is a transliteration of the pinned `pow.cpp`: `GetNextWorkRequired` on a
linked list of block index entries walked through `pprev`,
`CalculateNextWorkRequired` and `PermittedDifficultyTransition`, with
`operator*=(uint32_t)` on chapter 47's
[pow_reference.py](../../coax/tests/bitcoin/pow_reference.py) limbs. It
shares no code with Coax. [retarget.py](../../coax/tests/bitcoin/retarget.py)
compiles `retarget.coax` to C in checked and release builds, through a
flat test entry point
([retarget_report.coax](../../coax/tests/bitcoin/retarget_report.coax)).
Its cases are:

- every mainnet retarget in the pinned headers, through
  `CalculateNextWorkRequired`, through `GetNextWorkRequired` on a whole
  period with a right or wrong `nBits`, and through the permitted range
  with `nBits` one less, equal and one more;
- Core's cases;
- 400 seeded periods on mainnet, testnet3, testnet4 and regtest, with
  minimum-difficulty blocks anywhere, new blocks on both sides of the
  20-minute rule, first blocks later than last blocks, and views of the
  wrong length;
- 400 seeded retargets of arbitrary `nBits`, some of whose products wrap;
- 400 seeded permitted-range cases at and off boundaries and at negative
  heights;
- the sanity check on the four networks and eleven broken parameter sets;
- 300 seeded multiplications, divisions and chainwork sums.

The report must equal the one built from the Python reference, and the
output buffer past it must be untouched. Then the harness builds the same
C for wasm32 and runs every case in Node with chapter 47's loader.

```shell
python3 coax/tests/bitcoin/retarget.py
```

On 2026-11-03, 3,982 cases agree with Python in native checked and release builds and in Wasm checked and release builds. The run takes about 85 seconds.

The vectors come from
[make_retarget_vectors.py](../../coax/tests/bitcoin/make_retarget_vectors.py),
which checks each pinned file's SHA-256; that each of the 968 headers
hashes to its hash and meets its target; that each block at a boundary
names the block before it; that a period's first and last blocks share
`nBits`; that every retarget, through both of Core's functions, gives the
next period's `nBits`; that the times in Core's tests are the real
blocks'; that block 966,143 is `defaultAssumeValid` and its chainwork is
`nMinimumChainWork`; and that each Core case it copies appears in Core's
test file. It writes
[retarget_vectors.coax](../../coax/tests/bitcoin/retarget_vectors.coax).

## What this chapter leaves out

- Every network's full parameters: genesis blocks, message starts, ports
  and signet's challenge: chapter 50.
- The timestamp rules: median time past, the two-hour future limit and
  testnet4's timewarp rule ("time-timewarp-attack"): chapter 51.
- BIP-54's rules: a proposal, not enforced by the pinned Core.
- The block index, `nChainWork` per entry and the most-work chain: chapter
  59.
- Headers presync and `nMinimumChainWork` as a sync rule: chapter 72.

## Summary

- Every 2,016 blocks the target is multiplied by the time the period took
  and divided by two weeks. The time is clamped to between a quarter and
  four times, the result is capped at `powLimit` and rounded down through
  the compact form.
- The window measures 2,015 gaps, not 2,016, and periods do not overlap.
  That off-by-one allows the time-warp attack; BIP-54 proposes to close
  it on mainnet, and testnet4 already does.
- Testnet allows minimum-difficulty blocks after 20 minutes; BIP-94 starts
  each retarget from the period's first block so that one late block
  cannot reset the difficulty.
- Chainwork is the sum of every header's work. The chainwork to block
  966,143 equals Core's `nMinimumChainWork`, which checks every mainnet
  period's `nBits` at once.

## Dependencies

- [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `pow.cpp`, `arith_uint256.cpp`,
  `test/pow_tests.cpp` and `kernel/chainparams.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/pow.cpp)
  by chapters 5 and 47, and `consensus/consensus.h` and `validation.cpp`
  from earlier chapters.
- Mainnet headers from mempool.space: the genesis block, both sides of
  every boundary from 2,016 to 973,728, and block 966,143, pinned in
  [coax/tests/vectors/upstream/mainnet/retarget-boundaries.txt](../../coax/tests/vectors/upstream/mainnet/retarget-boundaries.txt).
- BIP-54 (Consensus Cleanup) and BIP-94 (Testnet 4).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
