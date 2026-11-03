# 51. Header rules: time, versions and deployments

Chapter 47 checks a header on its own: it reads, it links to its parent,
and its hash meets its target. Chapter 48 computes the target the header
must have. A header also has to fit the chain it extends: its time must be
neither too early nor too far ahead, and its version must be one that the
soft forks active at its height allow. Bitcoin Core checks these rules in
`ContextualCheckBlockHeader`, before it stores the header.

This chapter builds that check. It reads the median time past of the 11
blocks before a header, applies the two-hour limit on future times, applies
testnet4's timewarp rule (BIP-94), and derives the minimum version from
chapter 50's deployment heights. It then builds BIP-9's version bits: which
versions signal a soft fork, and the state machine that moves a deployment
from defined to active. Taproot activated through that state machine in
2021, with a minimum activation height (Speedy Trial).

## What this chapter teaches

- The median time past (MTP), and why a block's time only has to exceed
  it, not its parent's time.
- The two-hour limit on future block times, and why it is the only rule
  that reads the node's clock.
- The timewarp attack, and testnet4's rule against it (BIP-94).
- How buried deployments (BIP-90) set a minimum header version.
- BIP-9 version bits: the top bits `001`, the deployment's bit, and
  version rolling.
- BIP-9's states, how one 2,016-block period moves a deployment to the
  next state, and Speedy Trial's minimum activation height.

## Where Bitcoin uses these rules

| Rule | Who reads it | Chapter |
| --- | --- | --- |
| Median time past | the header rule here, and BIP-113's lock-time cutoff | 39, this chapter |
| Two-hour future limit | header acceptance, and the time a node accepts headers from peers | this chapter, 72 |
| Timewarp rule | testnet4 headers at each retarget | this chapter |
| Minimum version | every header after BIP-34, BIP-66 and BIP-65 | this chapter |
| Deployment heights | script flags and block rules | 54, 55 |
| Version bits | the version a node mines with, and unknown-deployment warnings | this chapter, 89 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [header.coax](../../coax/lib/bitcoin/header.coax) | `header_times_fit` | checks that the previous times fit the previous height |
| | `header_min_version` | the lowest version allowed at a height |
| | `header_check_vartime` | Core's `ContextualCheckBlockHeader` |
| | `Bip9Deployment`, `header_testdummy`, `TAPROOT_SPEEDY_TRIAL` | Core's `BIP9Deployment`, each network's test deployment, and Taproot's |
| | `header_signals`, `header_bip9_count_vartime` | `VersionBitsConditionChecker::Condition`, and a count over a period |
| | `header_bip9_version` | `ComputeBlockVersion` for one deployment |
| | `header_bip9_state_vartime` | `GetStateFor` |
| [locktime.coax](../../coax/lib/bitcoin/locktime.coax) | `locktime_median_time_past_vartime` | `GetMedianTimePast`, chapter 39's |
| [retarget.coax](../../coax/lib/bitcoin/retarget.coax) | `retarget_next_bits_vartime`, `retarget_calculate_vartime` | the `nBits` a header needs, chapter 48's |
| [chainparams.coax](../../coax/lib/bitcoin/chainparams.coax) | `chain_deployment_active` | the deployment heights, chapter 50's |

## The context of a header

A header's rules read the chain before it: the previous block's height and
time, and the times of the 10 blocks before that. The check takes those
times, oldest first, as `prev_times`. There are always 11 of them, except
within 11 blocks of genesis, where there are as many as there are blocks.

<!-- source: coax/lib/bitcoin/header.coax context -->
```jai
// prev_times holds the times of the previous block and the up to 10
// before it, oldest first: 11 of them, or prev_height + 1 near genesis,
// the most GetMedianTimePast can read.
header_times_fit :: (prev_height: int, prev_times: [] int) -> bool #export_c {
  if prev_height < 0 { return false; }
  want := ifx prev_height + 1 < LOCKTIME_MEDIAN_SPAN then prev_height + 1 else LOCKTIME_MEDIAN_SPAN;
  return prev_times.count == want;
}
```

## Median time past

A miner chooses a block's time. Miners' clocks differ, and a miner can
also lie, so Bitcoin cannot require each block's time to be later than
its parent's. Instead, a block's time must be later than the median of
the times of the 11 blocks before it: the median time past. One miner
cannot move the median by much. Six of the 11 blocks would have to be
early or late together to move it.

`GetMedianTimePast` sorts the up to 11 times and takes the middle one;
with an even count it takes the upper of the two middle times. Chapter 39
built it for BIP-113, which uses the same median as the cutoff for lock
times. This chapter reuses it:

<!-- source: coax/lib/bitcoin/locktime.coax mtp -->
```jai
// GetMedianTimePast: the median of the times of a block and the up to 10
// before it (times holds 1 to 11 of them, in any order). With an even
// count it is the upper of the two middle times, as in Core.
LOCKTIME_MEDIAN_SPAN :: 11;
locktime_median_time_past_vartime :: (times: [] int) -> int {
  #assert times.count >= 1 && times.count <= LOCKTIME_MEDIAN_SPAN;
  sorted: [LOCKTIME_MEDIAN_SPAN] int;
  n := times.count;
  for i: 0..<n {
    v := times[i]; j := i;
    while j > 0 && sorted[j-1] > v { sorted[j] = sorted[j-1]; j -= 1; }
    sorted[j] = v;
  }
  return sorted[n / 2];
}
```

Because each block's time is later than the median before it, the median
never decreases. Block times can go backwards; medians cannot. Mainnet
block 840,113 shows it. Its time is 67 seconds before its parent's, and
3,569 seconds after the median:

```text
  840111 time 1713648469 version 0x2015c000
  840112 time 1713650002 version 0x22956000
  840113 time 1713649935 version 0x20002000
  median time past 1713646366 ; 840113 is 3569 s after it and 67 s before its parent
```

## Times in the future

The median sets a lower bound. The upper bound is the node's own clock: a
header more than two hours ahead of it is refused. The rule keeps a miner
from setting times far ahead, which would raise the median for everyone
after it.

<!-- source: coax/lib/bitcoin/header.coax limits -->
```jai
HEADER_MAX_FUTURE :: 2 * 60 * 60;   // MAX_FUTURE_BLOCK_TIME (chain.h): two hours
HEADER_MAX_TIMEWARP :: 600;         // MAX_TIMEWARP (consensus.h): ten minutes
```

This is the only header rule that depends on the node rather than the
chain. A header refused as too new is not invalid forever: Core's
`AcceptBlockHeader` does not add a refused header to its block index, so
the same header is checked again when a peer sends it after the clock has
caught up. Core reports the refusal as `BLOCK_TIME_FUTURE`, not as an
invalid header. The check takes
the clock as `now`, in seconds since 1970, so that a test can set it.

## The timewarp attack and BIP-94

Retargeting (chapter 48) measures a period by the times of its first and
last blocks. Mainnet's retarget uses the period's first and last blocks
but does not count the time between the last block of one period and the
first of the next. A majority of miners can exploit that gap. They set
the last block of each period as late as the rules allow and the first
block of the next period back to the median. Each period then appears to
take longer than it did, the difficulty falls, and blocks come faster.
This is the timewarp attack. It is still possible on mainnet; proposals
such as the Great Consensus Cleanup would close it there.

Testnet4 closes it with BIP-94: the first block of each period may be no
more than 10 minutes (`MAX_TIMEWARP`, 600 seconds) before the block it
follows. The rule applies only at heights that are multiples of 2,016,
and only on networks with `enforce_BIP94`. In the pinned Core that is
testnet4 alone; regtest takes it from an option (`opts.enforce_bip94`). BIP-94 also has
testnet4 compute the new target from the period's first block's `nBits`,
which chapter 48's `retarget_calculate_vartime` does.

The real testnet4 chain never needed the rule at its first retarget.
Block 2,016 came 13 seconds after block 2,015. The chapter's
counterexample moves block 2,015 an hour ahead and puts 2,016 back, at
600 and 601 seconds before it:

```text
  2015 an hour ahead, at 1715033132
  2016 600 s before it: testnet4 ok, mainnet's rules ok
  2016 601 s before it: testnet4 time-timewarp-attack, mainnet's rules ok
  the same header at height 2017: ok
```

Mainnet's rules accept the same header, because mainnet does not enforce
BIP-94. At height 2,017 testnet4 accepts it too, because 2,017 is not the
first block of a period.

## Versions after buried deployments

A header's version is a signed 32-bit number. Early soft forks raised it
to announce support: version 2 for BIP-34 (the height in the coinbase),
version 3 for BIP-66 (strict DER signatures) and version 4 for BIP-65
(`OP_CHECKLOCKTIMEVERIFY`). Each soft fork activated when enough blocks
carried the new version. BIP-90 later replaced those activation rules
with fixed heights, which chapter 50's `ChainParams` holds.

Core still refuses a header whose version is below an active
deployment's: below 2 once BIP-34 is active, below 3 once BIP-66 is, and
below 4 once BIP-65 is. The minimum at a height is the highest of these.
On testnet3, BIP-66 activated before BIP-65, so the three steps come in
the same order there too:

<!-- source: coax/lib/bitcoin/header.coax version -->
```jai
// The lowest version a header at this height may have. Core refuses a
// version below 2 once BIP-34 is active, below 3 once BIP-66 is, and below
// 4 once BIP-65 is; the highest active minimum covers the others.
header_min_version :: (p: ChainParams, height: int) -> int #export_c {
  if chain_deployment_active(p, DEP_CLTV, height) { return 4; }
  if chain_deployment_active(p, DEP_DERSIG, height) { return 3; }
  if chain_deployment_active(p, DEP_HEIGHTINCB, height) { return 2; }
  return 1;
}
```

CSV, segwit and Taproot did not raise the version. They activated through
version bits, described below, so they set no minimum.

A negative version is below every minimum. `pow_header_read_vartime`
(chapter 47) reads the version as a signed number, so a header whose top
bit is set is refused once BIP-34 is active.

## The check

`header_check_vartime` applies Core's rules in Core's order. The order
decides which reason a header is refused with when it breaks several
rules:

<!-- source: coax/lib/bitcoin/header.coax check -->
```jai
// Core's ContextualCheckBlockHeader, its rules in its order. The previous
// block is at prev_height; required_bits is the nBits chapter 48's
// retarget_next_bits_vartime gives for the new header; now is the node's
// clock, in seconds since 1970.
header_check_vartime :: (p: ChainParams, prev_height: int, prev_times: [] int, required_bits: u32, h: BlockHeader, now: int) -> int #export_c {
  if !header_times_fit(prev_height, prev_times) { return HEADER_CONTEXT; }
  height := prev_height + 1;
  if h.bits != required_bits { return HEADER_BAD_DIFFBITS; }
  time := cast(int) h.time;
  if time <= locktime_median_time_past_vartime(prev_times) { return HEADER_TIME_TOO_OLD; }
  if p.retarget.bip94 && height % retarget_interval(p.retarget) == 0 {
    if time < prev_times[prev_times.count - 1] - HEADER_MAX_TIMEWARP { return HEADER_TIMEWARP; }
  }
  if time > now + HEADER_MAX_FUTURE { return HEADER_TIME_TOO_NEW; }
  if h.version < header_min_version(p, height) { return HEADER_BAD_VERSION; }
  return HEADER_OK;
}
```

The required `nBits` comes from chapter 48. Off a retarget boundary on
mainnet it is the parent's `nBits`; at a boundary it is
`retarget_calculate_vartime` of the period's first and last blocks. The
check takes it as an argument, so that the caller can compute it from the
period it already holds.

## Result codes

<!-- source: coax/lib/bitcoin/header.coax codes -->
```jai
// Result codes, with Core's reject reason in quotes.
HEADER_OK :: 0;
HEADER_BAD_DIFFBITS :: 1;   // "bad-diffbits": nBits is not the required target
HEADER_TIME_TOO_OLD :: 2;   // "time-too-old": not after the median time past
HEADER_TIMEWARP :: 3;       // "time-timewarp-attack": BIP-94, testnet4
HEADER_TIME_TOO_NEW :: 4;   // "time-too-new": more than two hours ahead
HEADER_BAD_VERSION :: 5;    // "bad-version(0x%08x)": below a buried deployment's minimum
HEADER_CONTEXT :: 6;        // the previous times do not fit the previous height
HEADER_REASONS :: [7] string.["ok", "bad-diffbits", "time-too-old", "time-timewarp-attack", "time-too-new", "bad-version", "bad context"];
```

| Code | Core | When |
| --- | --- | --- |
| `HEADER_BAD_DIFFBITS` | "bad-diffbits" | `nBits` is not the required target |
| `HEADER_TIME_TOO_OLD` | "time-too-old" | the time is not after the median time past |
| `HEADER_TIMEWARP` | "time-timewarp-attack" | BIP-94: a period's first block is too early |
| `HEADER_TIME_TOO_NEW` | "time-too-new" | more than two hours ahead of the clock |
| `HEADER_BAD_VERSION` | "bad-version(0x%08x)" | the version is below an active deployment's minimum |
| `HEADER_CONTEXT` | none: Core reads the times from its block index | the times do not fit the height |

## Version bits

From 2016 on, soft forks activated through BIP-9 instead of version
numbers. A version that starts with the three bits `001` (`0x20000000` to
`0x3fffffff`) is a set of 29 flags. Each deployment chooses one bit, and a
block signals for the deployment by setting it. Versions with other top
bits signal nothing, so the old versions 1 to 4 do not count:

<!-- source: coax/lib/bitcoin/header.coax bip9 -->
```jai
// BIP-9's states, and the special start times of Core's BIP9Deployment.
BIP9_DEFINED :: 0;
BIP9_STARTED :: 1;
BIP9_LOCKED_IN :: 2;
BIP9_ACTIVE :: 3;
BIP9_FAILED :: 4;
BIP9_NAMES :: [5] string.["defined", "started", "locked_in", "active", "failed"];
BIP9_ALWAYS_ACTIVE :: -1;
BIP9_NEVER_ACTIVE :: -2;
BIP9_NO_TIMEOUT :: 0x7fffffffffffffff;
VERSIONBITS_TOP_BITS :: 0x20000000;   // the version's top three bits are 001
VERSIONBITS_TOP_MASK :: 0xe0000000;
// One deployment: its bit, the median times it starts and times out, the
// lowest height it may activate at, and how many of a period's blocks
// must signal.
Bip9Deployment :: struct {
  bit: int; start: int; timeout: int;
  min_activation_height: int; period: int; threshold: int;
}
// Core's DEPLOYMENT_TESTDUMMY on each network: never active outside
// regtest, where it starts at once and counts 108 of 144 blocks.
header_testdummy :: (chain: int) -> Bip9Deployment #export_c {
  d := Bip9Deployment.{bit=28, start=BIP9_NEVER_ACTIVE, timeout=BIP9_NO_TIMEOUT, period=2016, threshold=1815};
  if chain == CHAIN_TESTNET3 || chain == CHAIN_TESTNET4 { d.threshold = 1512; }
  if chain == CHAIN_REGTEST { d.start = 0; d.period = 144; d.threshold = 108; }
  return d;
}
// Taproot's mainnet deployment, Speedy Trial, from Core v22.0: bit 2,
// starting 2021-04-24, timing out 2021-08-11, and activating no lower
// than height 709,632. It has since been buried (chapter 50).
TAPROOT_SPEEDY_TRIAL :: Bip9Deployment.{bit=2, start=1619222400, timeout=1628640000, min_activation_height=709632, period=2016, threshold=1815};
```

Each network has a test deployment, `DEPLOYMENT_TESTDUMMY`, on bit 28.
Only regtest starts it; on the other networks it is never active. Taproot
was the last deployment that used version bits on mainnet. Its
parameters come from Core v22.0, before it was buried.

<!-- source: coax/lib/bitcoin/header.coax signal -->
```jai
// A version signals a deployment when its top three bits are 001 and the
// deployment's bit is set. A negative version never signals.
header_signals :: (d: Bip9Deployment, version: int) -> bool #export_c {
  return (version & VERSIONBITS_TOP_MASK) == VERSIONBITS_TOP_BITS && ((version >> d.bit) & 1) != 0;
}
header_bip9_count_vartime :: (d: Bip9Deployment, versions: [] int) -> int #export_c {
  n := 0;
  for i: 0..<versions.count { if header_signals(d, versions[i]) { n += 1; } }
  return n;
}
// The version Core mines with (ComputeBlockVersion), for one deployment:
// the top bits, and the deployment's bit while it is started or locked in.
header_bip9_version :: (d: Bip9Deployment, state: int) -> int #export_c {
  v := VERSIONBITS_TOP_BITS;
  if state == BIP9_STARTED || state == BIP9_LOCKED_IN { v |= 1 << d.bit; }
  return v;
}
```

Not every set bit is a signal. Since BIP-320, miners use bits 13 to 28 as
extra nonce space ("version rolling"). In the 12 mainnet headers from
840,102, seven set bit 13 and none set bit 2:

```text
  840102 to 840113 with bit 2 set: 0 of 12; with bit 13 set: 7 of 12
```

A deployment that chose bit 13 would see those blocks as signals.

## The deployment states

BIP-9 moves a deployment through five states. Every block of a 2,016-block
period has the same state, and the state changes only at period
boundaries:

- **Defined**: the start. A deployment stays defined until the median time
  past at the end of a period reaches its start time.
- **Started**: blocks signal. A period in which at least `threshold`
  blocks signal (1,815 of 2,016 on mainnet, 90%) moves it to locked in.
  Otherwise a period whose median time past reaches the timeout moves it
  to failed.
- **Locked in**: the deployment will activate. It becomes active at the
  first period that starts at or after `min_activation_height`.
- **Active** and **failed**: final.

The count comes before the timeout: a period that reaches the threshold
locks in even if its median time past is past the timeout. Speedy Trial
added the minimum activation height. Taproot locked in in June 2021 but
activated only at block 709,632, giving node operators five months to
upgrade.

<!-- source: coax/lib/bitcoin/header.coax state -->
```jai
// One period, heights k * period to k * period + period - 1: the median
// time past at its last block, and how many of its blocks signal.
Bip9Period :: struct { mtp: int; count: int; }
// The state of the blocks of the period after the ones summarized. A
// period's state follows from the one before it, so the walk starts in
// DEFINED at genesis. As in Core, any period whose median time past is
// before the start puts the next period back in DEFINED, so the walk
// starts again after the last such period.
header_bip9_state_vartime :: (d: Bip9Deployment, periods: [] Bip9Period) -> int #export_c {
  if d.start == BIP9_ALWAYS_ACTIVE { return BIP9_ACTIVE; }
  if d.start == BIP9_NEVER_ACTIVE { return BIP9_FAILED; }
  first := 0;
  for k: 0..<periods.count { if periods[k].mtp < d.start { first = k + 1; } }
  state := BIP9_DEFINED;
  for k: first..<periods.count {
    s := periods[k];
    if state == BIP9_DEFINED {
      if s.mtp >= d.start { state = BIP9_STARTED; }
    } else if state == BIP9_STARTED {
      if s.count >= d.threshold { state = BIP9_LOCKED_IN; }
      else if s.mtp >= d.timeout { state = BIP9_FAILED; }
    } else if state == BIP9_LOCKED_IN {
      // The next period starts at height (k + 1) * period.
      if (k + 1) * d.period >= d.min_activation_height { state = BIP9_ACTIVE; }
    }
  }
  return state;
}
```

Core computes the state differently. `GetStateFor` walks back from the
latest period, one period at a time, until it reaches genesis or a period
whose median time past is before the start, and then walks forward,
caching each period's state. The forward walk here starts after the last
period whose median time past is before the start, which gives the same
state. The checks compare it with a transliteration of Core's walk on 400
made-up histories.

A miner sets a deployment's bit while the deployment is started or locked
in:

```text
  mined version while defined: 0x20000000
  mined version while started: 0x20000004
  mined version while locked_in: 0x20000004
  mined version while active: 0x20000000
  mined version while failed: 0x20000000
```

The example runs Taproot's parameters over a made-up history: the median
time past reaches the start in period 338, and period 340 reaches the
threshold. The deployment locks in at period 341 and becomes active at
period 352, which starts at height 709,632:

```text
  period 338 (from height 681408): defined
  period 339 (from height 683424): started
  period 340 (from height 685440): started
  period 341 (from height 687456): locked_in
  period 351 (from height 707616): locked_in
  period 352 (from height 709632): active
  period 353 (from height 711648): active
  without the minimum height, period 342: active
```

The counts and times are made up; only the parameters and the activation
height are Taproot's.

## The audit

Chapter 39's `locktime.coax` already had the median time past, which this
chapter uses for the header rule. Chapter 48's `retarget.coax` has the
`nBits` rule and BIP-94's first-block target, and states that the
timestamp rules are this chapter's. Chapter 50's `ChainParams` has the
deployment heights and `retarget.bip94`. Chapter 47's
`pow_header_read_vartime` reads the version as a signed number. Nothing
checked a header's time or version, or computed a BIP-9 state, before
this chapter.

## The example

The example is
[bitcoin/examples/header-rules.coax](../examples/header-rules.coax). It
checks mainnet block 840,113 and changed copies of it, prints the minimum
versions at each deployment's height, checks testnet4's first retarget and
the timewarp counterexample, and runs version bits over the mainnet
headers and a made-up history.

<!-- source: bitcoin/examples/header-rules.coax example -->
```jai
#load "../../coax/lib/bitcoin/header.coax";
#import "Basic";

// A 32-bit word as 0x and eight hex places; a negative version as its
// two's complement, as Core prints it.
put32 :: (v: int) {
  hex := "0123456789abcdef";
  out: [10] u8;
  out[0] = '0'; out[1] = 'x';
  for i: 0..<8 { out[2 + i] = hex[(v >> (4 * (7 - i))) & 15]; }
  print("%", cast(string) out[..]);
}
reason :: (code: int) -> string { return HEADER_REASONS[code]; }
read_headers :: (text: string, out: [] BlockHeader) -> int {
  raw: [1040] u8;
  n := cp_unhex(text, raw[..]);
  #assert n >= 0 && n % POW_HEADER == 0;
  for i: 0..<n / POW_HEADER { out[i] = pow_header_read_vartime(raw[i * POW_HEADER..<(i + 1) * POW_HEADER]); }
  return n / POW_HEADER;
}

main :: () {
  print("limits: % seconds ahead of the clock, % seconds of timewarp\n", HEADER_MAX_FUTURE, HEADER_MAX_TIMEWARP);

  // Mainnet 840,102 to 840,113: block 840,113 is earlier than its parent.
  hs: [13] BlockHeader;
  #assert read_headers(HEADERS_MAINNET_840102, hs[..]) == 12;
  times: [11] int;
  print("mainnet:\n");
  for i: 0..<12 {
    if i < 11 { times[i] = cast(int) hs[i].time; }
    print("  % time % version ", 840102 + i, hs[i].time); put32(hs[i].version); print("\n");
  }
  m := CHAIN_MAINNET_PARAMS;
  h := hs[11];
  mtp := locktime_median_time_past_vartime(times[..]);
  now := cast(int) h.time;
  print("  median time past % ; 840113 is % s after it and % s before its parent\n", mtp, cast(int) h.time - mtp, times[10] - cast(int) h.time);
  print("  840113: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits, h, now)));
  e := h;
  e.time = cast(u32) mtp;
  print("  time at the median: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits, e, now)));
  e.time = cast(u32) (mtp + 1);
  print("  one second later: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits, e, now)));
  print("  clock two hours behind: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits, h, now - HEADER_MAX_FUTURE)));
  print("  and one second more: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits, h, now - HEADER_MAX_FUTURE - 1)));
  print("  other nBits: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits ^ 1, h, now)));
  e = h;
  e.version = 3;
  print("  version 3: %\n", reason(header_check_vartime(m, 840112, times[..], hs[10].bits, e, now)));

  print("minimum versions:\n");
  heights := [8] int.[0, 227930, 227931, 363724, 363725, 388380, 388381, 840113];
  for i: 0..<8 { print("  mainnet % : %\n", heights[i], header_min_version(m, heights[i])); }
  t3 := CHAIN_TESTNET3_PARAMS;
  print("  testnet3 21111 : %, 330776 : %, 581885 : %\n", header_min_version(t3, 21111), header_min_version(t3, 330776), header_min_version(t3, 581885));
  print("  testnet4, signet and regtest from height 1 : %, %, %\n", header_min_version(CHAIN_TESTNET4_PARAMS, 1), header_min_version(CHAIN_SIGNET_PARAMS, 1), header_min_version(CHAIN_REGTEST_PARAMS, 1));

  // Testnet4's first retarget, block 2,016, after its genesis.
  #assert read_headers(HEADERS_TESTNET4_2005, hs[..]) == 13;
  t := CHAIN_TESTNET4_PARAMS;
  for i: 0..<11 { times[i] = cast(int) hs[i + 1].time; }
  b := hs[12];
  bits := retarget_calculate_vartime(t.retarget, BlockStamp.{time=hs[11].time, bits=hs[11].bits}, BlockStamp.{time=hs[0].time, bits=hs[0].bits});
  print("testnet4:\n");
  print("  2015 time %, median time past %; 2016 time %\n", times[10], locktime_median_time_past_vartime(times[..]), b.time);
  print("  2016 needs nBits "); put32(cast(int) bits); print(", has "); put32(cast(int) b.bits);
  print(": %\n", reason(header_check_vartime(t, 2015, times[..], bits, b, cast(int) b.time)));
  times[10] += 3600;
  print("  2015 an hour ahead, at %\n", times[10]);
  for back: 600..<602 {
    e = b;
    e.time = cast(u32) (times[10] - back);
    print("  2016 % s before it: testnet4 %, mainnet's rules %\n", back, reason(header_check_vartime(t, 2015, times[..], bits, e, cast(int) e.time)), reason(header_check_vartime(m, 2015, times[..], bits, e, cast(int) e.time)));
  }
  e.time = cast(u32) (times[10] - 601);
  print("  the same header at height 2017: %\n", reason(header_check_vartime(t, 2016, times[..], bits, e, cast(int) e.time)));

  // Version bits.
  d := TAPROOT_SPEEDY_TRIAL;
  print("version bits:\n");
  print("  Taproot: bit %, start %, timeout %, minimum height %, % of %\n", d.bit, d.start, d.timeout, d.min_activation_height, d.threshold, d.period);
  #assert read_headers(HEADERS_MAINNET_840102, hs[..]) == 12;
  versions: [12] int;
  for i: 0..<12 { versions[i] = hs[i].version; }
  rolled := d;
  rolled.bit = 13;
  print("  840102 to 840113 with bit 2 set: % of 12; with bit 13 set: % of 12\n", header_bip9_count_vartime(d, versions[..]), header_bip9_count_vartime(rolled, versions[..]));
  vs := [5] int.[0x20000004, 0x20000000, 0x00000004, 0x60000004, 0x3ffffffc];
  for i: 0..<5 { print("  "); put32(vs[i]); print(" signals Taproot: %\n", ifx header_signals(d, vs[i]) then "yes" else "no"); }
  for s: BIP9_DEFINED..<BIP9_FAILED + 1 { print("  mined version while %: ", BIP9_NAMES[s]); put32(header_bip9_version(d, s)); print("\n"); }

  // A made-up history with Taproot's parameters: the median time passes
  // the start in period 338, period 340 reaches the threshold.
  p: [353] Bip9Period;
  for k: 0..<353 {
    p[k].mtp = ifx k < 338 then d.start - 100 else d.start + (k - 337) * 1000;
    p[k].count = ifx k == 340 then d.threshold else d.threshold - 1;
  }
  ks := [7] int.[338, 339, 340, 341, 351, 352, 353];
  for i: 0..<7 { print("  period % (from height %): %\n", ks[i], ks[i] * d.period, BIP9_NAMES[header_bip9_state_vartime(d, p[0..<ks[i]])]); }
  e2 := d;
  e2.min_activation_height = 0;
  print("  without the minimum height, period 342: %\n", BIP9_NAMES[header_bip9_state_vartime(e2, p[0..<342])]);
  p[340].count = d.threshold - 1;
  p[345].mtp = d.timeout;
  print("  never enough, timeout reached in period 345: period 346 %\n", BIP9_NAMES[header_bip9_state_vartime(d, p[0..<346])]);
  p[345].count = d.threshold;
  print("  the threshold reached in that period: period 346 %\n", BIP9_NAMES[header_bip9_state_vartime(d, p[0..<346])]);
  for chain: CHAIN_MAIN..<CHAIN_REGTEST + 1 {
    td := header_testdummy(chain);
    print("  % testdummy: start %, % of %\n", CHAIN_NAMES[chain], td.start, td.threshold, td.period);
  }
}
```

Its output for mainnet:

```text
  median time past 1713646366 ; 840113 is 3569 s after it and 67 s before its parent
  840113: ok
  time at the median: time-too-old
  one second later: ok
  clock two hours behind: ok
  and one second more: time-too-new
  other nBits: bad-diffbits
  version 3: bad-version
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/header-rules.out](../fixtures/header-rules.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/header-rules.coax](../examples/checks/header-rules.coax),
which loads the example:

- the header rule's median is chapter 39's lock-time cutoff, at 80 times
  around it;
- testnet4's timewarp rule fires at a period's first block and at no other
  height, and testnet3 never applies it;
- the minimum version steps up at chapter 50's heights on every network;
- the state machine equals a transliteration of Core's backward walk on
  400 made-up histories, each checked after every period;
- a mined version signals exactly while the deployment is started or
  locked in.

<!-- source: bitcoin/examples/checks/header-rules.coax check -->
```jai
// The median time past is chapter 39's BIP-113 cutoff: a time passes the
// header rule exactly when it is past that cutoff.
#forall "the header's median is the lock time's" delta: -40..<40 #steps 100000000 {
  hs: [13] BlockHeader;
  #assert read_headers(HEADERS_MAINNET_840102, hs[..]) == 12;
  times: [11] int;
  for i: 0..<11 { times[i] = cast(int) hs[i].time; }
  e := hs[11];
  cutoff := locktime_median_time_past_vartime(times[..]);
  e.time = cast(u32) (cutoff + delta);
  code := header_check_vartime(CHAIN_MAINNET_PARAMS, 840112, times[..], hs[10].bits, e, cutoff + delta);
  #assert (code == HEADER_OK) == (delta > 0) && (code == HEADER_TIME_TOO_OLD) == (delta <= 0);
}
// Testnet4's timewarp rule reads only the first block of a period: the
// same early header passes at every other height.
#forall "the timewarp rule fires only on a boundary" k: 0..<64 #steps 100000000 {
  times: [11] int;
  for i: 0..<11 { times[i] = 1000000 + 100 * i; }
  times[10] += 3600;
  h: BlockHeader;
  h.version = 0x20000000; h.bits = 0x1d00ffff; h.time = cast(u32) (times[10] - 601);
  prev := 2016 * 3 - 33 + k;
  code := header_check_vartime(CHAIN_TESTNET4_PARAMS, prev, times[..], h.bits, h, cast(int) h.time);
  #assert (code == HEADER_TIMEWARP) == ((prev + 1) % 2016 == 0) && (code == HEADER_OK) == ((prev + 1) % 2016 != 0);
  #assert header_check_vartime(CHAIN_TESTNET3_PARAMS, prev, times[..], h.bits, h, cast(int) h.time) == HEADER_OK;
}
// The minimum version steps up exactly at chapter 50's deployment heights.
#forall "minimum versions follow the chain parameters" chain: 1..<6 {
  p := chain_params(chain);
  deps := [3] int.[DEP_HEIGHTINCB, DEP_DERSIG, DEP_CLTV];
  mins := [3] int.[2, 3, 4];
  for i: 0..<3 {
    at := chain_deployment_height(p, deps[i]);
    #assert header_min_version(p, at) >= mins[i];
    if at > 0 { #assert header_min_version(p, at - 1) <= header_min_version(p, at); }
  }
  #assert header_min_version(p, 0) == 1 && header_min_version(p, 2000000) == 4;
}
// Core's GetStateFor, transliterated: walk back a period at a time to
// genesis or to a median time before the start, then forward.
core_state :: (d: Bip9Deployment, periods: [] Bip9Period) -> int {
  if d.start == BIP9_ALWAYS_ACTIVE { return BIP9_ACTIVE; }
  if d.start == BIP9_NEVER_ACTIVE { return BIP9_FAILED; }
  todo: [64] int; n := 0;
  k := periods.count - 1;
  while k >= 0 && periods[k].mtp >= d.start { todo[n] = k; n += 1; k -= 1; }
  state := BIP9_DEFINED;
  while n > 0 {
    n -= 1;
    i := todo[n];
    next := state;
    if state == BIP9_DEFINED && periods[i].mtp >= d.start { next = BIP9_STARTED; }
    if state == BIP9_STARTED {
      if periods[i].count >= d.threshold { next = BIP9_LOCKED_IN; }
      else if periods[i].mtp >= d.timeout { next = BIP9_FAILED; }
    }
    if state == BIP9_LOCKED_IN && (i + 1) * d.period >= d.min_activation_height { next = BIP9_ACTIVE; }
    state = next;
  }
  return state;
}
// The forward walk agrees with Core's on made-up histories: times that
// wander around the start and timeout, counts around the threshold.
#forall "the state machine is Core's" seed: 0..<400 #steps 100000000 {
  d := Bip9Deployment.{bit=5, start=1000, timeout=1600, min_activation_height=(seed % 7) * 10, period=10, threshold=8};
  x := (seed * 2654435761 + 12345) % 2147483648;
  p: [24] Bip9Period;
  t := 900;
  for k: 0..<24 {
    x = (x * 1103515245 + 12345) % 2147483648;
    t += (x >> 8) % 120 - 30;
    p[k].mtp = t;
    p[k].count = 6 + (x >> 4) % 4;
  }
  for n: 0..<25 { #assert header_bip9_state_vartime(d, p[0..<n]) == core_state(d, p[0..<n]); }
}
// A block mined in a state signals exactly when the state is started or
// locked in.
#forall "mined versions signal while the deployment counts" s: 0..<5 {
  d := TAPROOT_SPEEDY_TRIAL;
  #assert header_signals(d, header_bip9_version(d, s)) == (s == BIP9_STARTED || s == BIP9_LOCKED_IN);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/header-rules.coax
```

`coax test` runs 295 checks, including the loaded files' own checks, in
about 150 seconds on 2026-11-03.

### The library's own checks

`header.coax` is an `#authority` file. Its checks read the pinned mainnet
and testnet4 headers and check that each links to the one before it. They
check block 840,113 and its changed copies (the median and one second
after it, the clock at two hours and one second more, another `nBits`,
versions 3, 4 and a negative one). They compute testnet4's first retarget
and check the timewarp counterexample at 600 and 601 seconds and at the
next height. They check the minimum versions on mainnet, testnet3 and
regtest at each step, each network's test deployment, and Taproot's
history with and without the minimum height, a timeout, a lock-in at the
timeout, and a median time past before the start.

```shell
coax/bin/coax test coax/lib/bitcoin/header.coax
```

`coax test` runs 290 checks, including those of the files it loads, in
about 130 seconds on 2026-11-03. Most of them are chapter 50's
signet checks, which this file loads. Mutation testing: not run (owner
speed order).

### Python

[header.py](../../coax/tests/bitcoin/header.py) reads the rules from the
pinned Core sources with regular expressions: the reject reasons of
`ContextualCheckBlockHeader` in their order, the three version minimums,
`MAX_FUTURE_BLOCK_TIME`, `MAX_TIMEWARP`, the median's index, the version
bits masks, BIP-9's special start times and state names, each network's
test deployment from `chainparams.cpp`, and Taproot's deployment from
`chainparams.cpp` at v22.0. It checks the constants of `header.coax`
against them. It then replays the example with transliterations of
`GetMedianTimePast`, `ContextualCheckBlockHeader` and `GetStateFor` (with
Core's backward walk and cache), using chapter 48's
[retarget_reference.py](../../coax/tests/bitcoin/retarget_reference.py)
for testnet4's target, and writes the transcript the example must print.

```shell
python3 -I coax/tests/bitcoin/header.py --coax coax/bin/coax
```

On 2026-11-03 all 68 lines agree with Python, and the example's
output in native debug and release builds equals them. Changing a limit,
a reason, a mask or Taproot's parameters in `header.coax` makes it fail.
Wasm was not run.

## What this chapter leaves out

- `nBits`: chapter 48. The header's own proof of work: chapter 47.
- Checkpoints: the pinned Core has none (chapter 50).
- The block rules that read deployment heights: the BIP-34 coinbase height
  and the witness commitment are chapter 54's, script flags chapter 55's.
- Headers presync and the time a node accepts from peers: chapter 72.
- Core's warnings about unknown version bits, and the `getdeploymentinfo`
  RPC: chapter 89.
- The Great Consensus Cleanup's mainnet timewarp fix: not in the pinned
  Core.

## Summary

- A header's time must be after the median time past of the 11 blocks
  before it and at most two hours ahead of the node's clock.
- Block times can go backwards; the median time past cannot.
- On testnet4, the first block of each period may be at most 10 minutes
  before the block it follows (BIP-94).
- After BIP-34, BIP-66 and BIP-65, a header's version must be at least 2,
  3 and 4.
- BIP-9 versions start with `001`, and each deployment uses one bit.
  Version rolling uses bits 13 to 28 as nonce space.
- A deployment moves from defined to started, then locked in or failed,
  then active, one period at a time. Speedy Trial's minimum height delays
  activation after lock-in.

## Dependencies

- [39. Timelocks](ch039-timelocks.md)
- [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md)
- [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md)
- [50. Chain parameters: mainnet, testnet4, signet and regtest](ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `validation.cpp`
  (`ContextualCheckBlockHeader`), `chain.h`, `consensus/consensus.h`,
  `consensus/params.h`, `versionbits.h`, `versionbits_impl.h`,
  `versionbits.cpp` and `kernel/chainparams.cpp`; and
  `chainparams.cpp` at v22.0 for Taproot's deployment. Pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/versionbits.cpp).
- [BIP-9](https://github.com/bitcoin/bips/blob/master/bip-0009.mediawiki)
  (version bits), [BIP-90](https://github.com/bitcoin/bips/blob/master/bip-0090.mediawiki)
  (buried deployments), [BIP-94](https://github.com/bitcoin/bips/blob/master/bip-0094.mediawiki)
  (testnet4), [BIP-113](https://github.com/bitcoin/bips/blob/master/bip-0113.mediawiki)
  (median time past), [BIP-320](https://github.com/bitcoin/bips/blob/master/bip-0320.mediawiki)
  (version rolling) and [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
  (Taproot).
- Mainnet headers 840,102 to 840,113 and testnet4's genesis and headers
  2,005 to 2,016 from mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/mainnet-840102-840113.headers)
  and [upstream/testnet4/](../../coax/tests/vectors/upstream/testnet4/testnet4-0-2005-2016.headers),
  and copied into [header_vectors.coax](../../coax/tests/bitcoin/header_vectors.coax).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
