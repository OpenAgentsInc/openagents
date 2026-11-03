# 39. Timelocks

A timelock keeps a transaction, or a coin, from being spent before a
height or a time. Bitcoin has two kinds. An absolute lock names a block
height or a time: the transaction's lock time, and
`OP_CHECKLOCKTIMEVERIFY` (BIP-65), which lets a script demand one. A
relative lock counts from the block that holds the coin: BIP-68's reading
of an input's sequence, and `OP_CHECKSEQUENCEVERIFY` (BIP-112). BIP-113
measures every time lock against the median time of the last 11 blocks.

Chapter 36's interpreter already runs both opcodes. This chapter explains
them and adds the rules around them that are not script: Bitcoin Core's
`IsFinalTx`, `GetMedianTimePast`, `CalculateSequenceLocks` and
`EvaluateSequenceLocks`, in a new library file,
[locktime.coax](../../coax/lib/bitcoin/locktime.coax).

## What this chapter teaches

- The lock time: a height below 500,000,000 and a time from there, the
  last height or time at which the transaction is still locked, and how a
  sequence of `0xffffffff` on every input turns it off.
- The median time past of 11 blocks, and why BIP-113 compares lock times
  with it instead of with a block's own time.
- `OP_CHECKLOCKTIMEVERIFY`: the five conditions under which it fails.
- BIP-68: the disable bit, the type bit, 16 bits of blocks or of 512-second
  units, and what each bit pattern of a sequence means.
- Sequence locks: the last locked height and time of a transaction, from
  each input's coin height and the median time past before it.
- `OP_CHECKSEQUENCEVERIFY`, and why it needs a version 2 transaction.
- Worked examples from Bitcoin Core's `tx_valid.json` and
  `tx_invalid.json`.

## Where Bitcoin uses timelocks

- Every block: each transaction in it must be final for the block's height
  and median time past (`ContextualCheckBlock`), and each input's
  sequence locks must be past (`ConnectBlock`, through `SequenceLocks`).
- Lightning: a channel's commitment transaction delays the broadcaster's
  own output with `OP_CHECKSEQUENCEVERIFY`, so the other side has time to
  punish an old state, and its HTLCs time out with
  `OP_CHECKLOCKTIMEVERIFY`.
- Wallets: Core and other wallets set the lock time of a new transaction
  to the current height, so a miner that reorganizes the chain gains
  nothing by moving it back (anti-fee-sniping).
- Vaults, inheritance and recovery paths: a script branch that only opens
  after a height, a time or a delay.

Both opcodes were soft forks. `OP_CHECKLOCKTIMEVERIFY` replaced `OP_NOP2`
at height 388,381 and `OP_CHECKSEQUENCEVERIFY` replaced `OP_NOP3` at
height 419,328, when BIP-68 and BIP-113 also took effect (Core's
`BIP65Height` and `CSVHeight` for mainnet). A node that never upgraded
reads them as NOPs, so every script they make invalid was valid before.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [locktime.coax](../../coax/lib/bitcoin/locktime.coax) | `locktime_final_vartime(b, t, ins, height, time)` | Core's `IsFinalTx` |
| | `locktime_median_time_past_vartime(times)` | `GetMedianTimePast` of up to 11 block times |
| | `locktime_relative_vartime(sequence)` | BIP-68's reading of a sequence |
| | `locktime_sequence_blocks_vartime(blocks)`, `locktime_sequence_seconds_vartime(seconds)` | a sequence that locks for a number of blocks or seconds |
| | `locktime_sequence_locks_vartime(b, t, ins, enforce, heights, times)` | `CalculateSequenceLocks` |
| | `locktime_sequence_ok_vartime(lock, height, prev_mtp)` | `EvaluateSequenceLocks` |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `si_locktime`, `si_sequence`, `si_lock_op` | the opcodes (chapter 36) |

The chain is not in `locktime.coax`. A caller passes the heights and
times, so the same procedures serve a node, a wallet that checks a
transaction before it broadcasts, and the tests. Lock times are public,
so every procedure runs in variable time.

## The lock time

The last four bytes of a transaction are its lock time. Zero means no
lock. Below 500,000,000 it is a block height, and from 500,000,000 a Unix
time: 500,000,000 seconds is 5 November 1985, long before Bitcoin, and
500,000,000 blocks is about 9,500 years of blocks. The constants are
chapter 36's:

<!-- source: coax/lib/bitcoin/script.coax limits -->
```jai
SCRIPT_MAX_SIZE :: 10000;        // MAX_SCRIPT_SIZE
SCRIPT_MAX_ELEMENT :: 520;       // MAX_SCRIPT_ELEMENT_SIZE
SCRIPT_MAX_OPS :: 201;           // MAX_OPS_PER_SCRIPT
SCRIPT_MAX_STACK :: 1000;        // MAX_STACK_SIZE: the stack and the alt stack together
SCRIPT_MAX_KEYS :: 20;           // MAX_PUBKEYS_PER_MULTISIG
SCRIPT_LOCKTIME_THRESHOLD :: 500000000;
SCRIPT_SEQUENCE_FINAL :: 0xffffffff;
SCRIPT_SEQUENCE_DISABLE :: 1 << 31;
SCRIPT_SEQUENCE_TYPE :: 1 << 22;
SCRIPT_SEQUENCE_MASK :: 0xffff;
```

The lock time is the last height, or the last time, at which the
transaction is still locked. A transaction with lock time 700,000 can
enter block 700,001 and no earlier one. `IsFinalTx` compares the lock time
with the block's height or with a time cutoff, and has one way out: if
every input's sequence is `0xffffffff`, the lock time doesn't count.

<!-- source: coax/lib/bitcoin/locktime.coax final -->
```jai
// IsFinalTx: a transaction may enter a block of this height whose lock-time
// cutoff is time (the block's time before BIP-113, the median time past of
// the 11 blocks before it after). A lock time of 0 is final. Below
// 500,000,000 it is a height and above a time, and the transaction is
// final once the lock time is less than the block's. If it isn't, it is
// still final when every input's sequence is 0xffffffff.
locktime_final_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, height: int, time: int) -> bool {
  locktime := tx_le_vartime(b, t.locktime, 4);
  if locktime == 0 { return true; }
  cutoff := ifx locktime < SCRIPT_LOCKTIME_THRESHOLD then height else time;
  if locktime < cutoff { return true; }
  for k: 0..<t.nin {
    if tx_le_vartime(b, ins[k].sequence, 4) != SCRIPT_SEQUENCE_FINAL { return false; }
  }
  return true;
}
```

The way out comes from Bitcoin's first release, where a sequence number
was meant to let the parties to an unconfirmed transaction replace it with
a higher-numbered version until the lock time. Replacement by sequence
never shipped, but the rule that a final sequence ends the lock stayed.
It is why a wallet that wants its lock time enforced sets some input's
sequence below `0xffffffff`; `0xfffffffe` is the usual choice, since it
also leaves BIP-68 and replace-by-fee signaling off.

Since BIP-113 the cutoff for a time lock is the median time past of the
11 blocks before the block, not the block's own time. Before height
419,328 a block's own time was the cutoff.

## The median time past

A block's time may be up to two hours ahead of the network's clock, and
need only be later than the median of the 11 blocks before it. So a
single block's time can run backwards, and a miner could set its time
ahead to include a time-locked transaction early and collect its fee.
The median of 11 blocks moves forward with the chain and can't be pushed
by one miner:

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

Core's `GetMedianTimePast` sorts the times of the block and up to 10
before it and takes the element at index `count / 2`. With 11 times that
is the sixth smallest. Near the genesis block there are fewer than 11, and
with an even count it is the upper of the two middle times.

Each block's time must exceed the median time past of its parent, so the
median time past never decreases. It runs about an hour behind the clock.
A time lock against it opens about an hour later than the clock says.

## OP_CHECKLOCKTIMEVERIFY

`OP_CHECKLOCKTIMEVERIFY` makes a coin unspendable until a height or a
time. It doesn't read the chain. It checks that the spending
transaction's own lock time is at least its argument, and `IsFinalTx`
then keeps the transaction out of every block before that. The script
`<n> OP_CHECKLOCKTIMEVERIFY OP_DROP <key> OP_CHECKSIG` pays a key once
height `n` has passed.

Chapter 36's `si_lock_op` and `si_locktime` run it:

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

It fails, and the script with it, when:

1. The stack is empty (`INVALID_STACK_OPERATION`).
2. The top element is not a number of at most 5 bytes (`SCRIPTNUM`). Five
   bytes, not the usual four, because lock times go up to `0xffffffff`
   and a 4-byte number stops at 2^31 − 1.
3. The number is negative (`NEGATIVE_LOCKTIME`).
4. The number and the lock time are of different kinds, one a height and
   one a time (`UNSATISFIED_LOCKTIME`). Otherwise a time argument could
   be satisfied by a height lock time that is numerically larger and
   means something earlier.
5. The number is larger than the lock time, or this input's sequence is
   `0xffffffff`, which would turn the lock time off
   (`UNSATISFIED_LOCKTIME`).

It leaves its argument on the stack, as the NOP it replaced did, so a
script drops it.

## BIP-68: the sequence as a relative lock

An input's sequence is 32 bits that nothing used. BIP-68 gives it a
meaning in transactions of version 2 or later:

| Bits | Meaning |
| --- | --- |
| 31 (`0x80000000`) | disable: no relative lock |
| 22 (`0x00400000`) | type: units of 512 seconds if set, blocks if not |
| 0–15 (`0x0000ffff`) | the number of blocks or units |
| the rest | ignored, left for later soft forks |

<!-- source: coax/lib/bitcoin/locktime.coax bip68 -->
```jai
// BIP-68's reading of an input's sequence. With bit 31 set the sequence
// has no relative lock. Otherwise bit 22 chooses the unit, blocks or 512
// seconds, and the low 16 bits count them; the other bits are ignored.
LOCKTIME_GRANULARITY :: 9;   // SEQUENCE_LOCKTIME_GRANULARITY: units of 2^9 = 512 seconds
LockRelative :: struct { enabled: bool; by_time: bool; blocks: int; seconds: int; }
locktime_relative_vartime :: (sequence: int) -> LockRelative {
  r: LockRelative;
  if (sequence & SCRIPT_SEQUENCE_DISABLE) != 0 { return r; }
  r.enabled = true;
  r.by_time = (sequence & SCRIPT_SEQUENCE_TYPE) != 0;
  value := sequence & SCRIPT_SEQUENCE_MASK;
  if r.by_time { r.seconds = value << LOCKTIME_GRANULARITY; } else { r.blocks = value; }
  return r;
}
// A sequence that locks an input for this many blocks, or -1 past 65,535.
locktime_sequence_blocks_vartime :: (blocks: int) -> int {
  if blocks < 0 || blocks > SCRIPT_SEQUENCE_MASK { return -1; }
  return blocks;
}
// A sequence that locks an input for at least this many seconds, rounded
// up to whole units of 512, or -1 past 65,535 units (about 388 days).
locktime_sequence_seconds_vartime :: (seconds: int) -> int {
  if seconds < 0 { return -1; }
  units := (seconds + (1 << LOCKTIME_GRANULARITY) - 1) >> LOCKTIME_GRANULARITY;
  if units > SCRIPT_SEQUENCE_MASK { return -1; }
  return SCRIPT_SEQUENCE_TYPE | units;
}
```

A unit of 512 seconds (2^9, `SEQUENCE_LOCKTIME_GRANULARITY`) is close to
the 600-second block interval, and lets 16 bits reach 33,553,920 seconds,
about 388 days. Sixteen bits of blocks reach 65,535 blocks, about 455
days. A time lock rounds up to whole units: one hour is 3,600 seconds,
which needs 8 units, 4,096 seconds, and the sequence `0x00400008`.

Bit 31 also matters for the lock time. A sequence of `0xffffffff` has it
set, so the input has no relative lock, and it is final, so the lock time
doesn't apply either. `0xfffffffe` has bit 31 set too: no relative lock,
but the lock time applies.

## Sequence locks

A relative lock counts from the block that holds the coin. A lock of `n`
blocks on a coin in block `h` opens at block `h + n`. A time lock of `n`
units opens once the median time past passes the median time past of the
block before `h` by `n * 512` seconds. Core's `CalculateSequenceLocks`
turns each input's lock into a height and a time at which the transaction
is still locked, as the lock time does, and takes the latest of each over
the inputs. `EvaluateSequenceLocks` lets a block hold the transaction when
its height is above the height and its parent's median time past is above
the time:

<!-- source: coax/lib/bitcoin/locktime.coax sequencelocks -->
```jai
// CalculateSequenceLocks: the last height and the last median time past at
// which the transaction is still locked, -1 for none. Only a version 2 or
// later transaction, with LOCKTIME_VERIFY_SEQUENCE (enforce), has relative
// locks. An input whose coin is in the block at heights[k] adds
// heights[k] + blocks - 1 or, by time, times[k] + seconds - 1, where
// times[k] is the median time past of the block before the coin's.
LockPair :: struct { height: int; time: int; }
locktime_sequence_locks_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, enforce: bool, heights: [] int, times: [] int) -> LockPair {
  lock := LockPair.{height=-1, time=-1};
  if !enforce || tx_le_vartime(b, 0, 4) < 2 { return lock; }
  for k: 0..<t.nin {
    r := locktime_relative_vartime(tx_le_vartime(b, ins[k].sequence, 4));
    if !r.enabled { continue; }
    if r.by_time {
      lock.time = lt_max(lock.time, times[k] + r.seconds - 1);
    } else {
      lock.height = lt_max(lock.height, heights[k] + r.blocks - 1);
    }
  }
  return lock;
}
// EvaluateSequenceLocks: a block at height whose previous block's median
// time past is prev_mtp may hold the transaction once both locks are past.
locktime_sequence_ok_vartime :: (lock: LockPair, height: int, prev_mtp: int) -> bool {
  return lock.height < height && lock.time < prev_mtp;
}
```

Three details come from Core:

- Nothing is locked unless the transaction's version is at least 2 and
  the caller enforces BIP-68 (`LOCKTIME_VERIFY_SEQUENCE`, set for blocks
  after height 419,328). Core reads the version as an unsigned 32-bit
  number, so `0x80000000` and `0xffffffff` count as at least 2.
- The time starts from the median time past of the block before the coin's
  block, the earliest time the coin's block could have had, not from the
  coin's block's own time.
- Both values are one less than the first height or time that is enough,
  so that `-1` means no lock.

## OP_CHECKSEQUENCEVERIFY

`OP_CHECKSEQUENCEVERIFY` makes a coin unspendable until a relative lock
has passed. As with `OP_CHECKLOCKTIMEVERIFY`, it reads no chain: it
checks that this input's sequence carries a relative lock at least as
long as its argument, and BIP-68 enforces that lock. `si_sequence` above
is BIP-112's check. An argument with bit 31 set is a NOP, left for later
soft forks. Otherwise the opcode fails when:

1. The stack is empty, the top is not a number of at most 5 bytes, or it
   is negative, as for `OP_CHECKLOCKTIMEVERIFY`.
2. The transaction's version is below 2, so BIP-68 doesn't apply.
3. This input's sequence has bit 31 set, so it has no relative lock.
4. The argument and the sequence are of different types, blocks and time.
5. The argument's 16 bits are more than the sequence's.

Only bit 22 and the low 16 bits of each are compared. The other bits are
ignored, as BIP-68 ignores them.

## Worked example: Core's transaction tests

`tx_valid.json` and `tx_invalid.json` test each opcode with one-input
transactions that spend a scriptPubKey with the lock. The example runs
twelve of them with only `P2SH` set, where the opcodes are NOPs, and with
both lock flags on:

| Row | Lock | Transaction | With the flags |
| --- | --- | --- | --- |
| `tx_valid` 102 | CLTV 499,999,999 | lock time 499,999,999 | OK: equal is enough |
| `tx_invalid` 67 | CLTV 1 | lock time 0 | the argument is larger |
| `tx_invalid` 70 | CLTV 500,000,001 | lock time 500,000,000 | the argument is larger, by time |
| `tx_invalid` 86 | CLTV 0, a height | lock time 500,000,000, a time | different kinds |
| `tx_invalid` 81 | CLTV 0 | sequence `0xffffffff` | the input is final |
| `tx_invalid` 76 | CLTV −1 | lock time 0 | negative |
| `tx_valid` 115 | CLTV of a 5-byte zero | lock time 0 | OK: 5 bytes are allowed |
| `tx_invalid` 96 | CLTV of a 6-byte zero | lock time 0 | too long for a number |
| `tx_invalid` 105 | CSV 1 | sequence 0 | the argument is larger |
| `tx_invalid` 108 | CSV `0x00400001` | sequence `0x00400000` | larger, in 512-second units |
| `tx_invalid` 117 | CSV 0, blocks | sequence `0x00400000`, time | different types |
| `tx_invalid` 128 | CSV 0 | version 1 | the version is below 2 |

Every row spends with the flags off. `tx_valid` 115's 5-byte zero is not
minimally encoded, so it fails under `MINIMALDATA`; that row's flags leave
`MINIMALDATA` out. `tx_invalid` 128 spends once its version byte is 2.

The example then finds every row of `script_tests.json`, and every
transaction of `tx_valid.json` and `tx_invalid.json`, whose scripts
contain `OP_CHECKLOCKTIMEVERIFY` or `OP_CHECKSEQUENCEVERIFY` (or `NOP2`
and `NOP3`, the same bytes), and runs them under every flag set chapter
36's vectors list: 21 rows, and 1,672 flag sets of 76 transactions, all
with Core's result.

## The example

The example is
[bitcoin/examples/timelocks.coax](../examples/timelocks.coax).

<!-- source: bitcoin/examples/timelocks.coax example -->
```jai
#load "../../coax/lib/bitcoin/locktime.coax";
#load "../../coax/tests/bitcoin/script_vectors.coax";
#import "Basic";

// A transaction read into fixed room, with the scriptPubKey its input
// spends.
Spend :: struct {
  b: [10240] u8; spk: [10100] u8;
  ins: [16] TxIn; outs: [16] TxOut; items: [1300] TxItem;
}
spend_of :: (s: [] Spend, tx: string, spk: string, index: int, amount: int) -> ScriptSpend {
  b := tx_unhex(tx, s[0].b[..]);
  t := tx_read_vartime(b, true, s[0].ins[..], s[0].outs[..], s[0].items[..]);
  #assert t.code == TX_OK;
  return ScriptSpend.{b=b, t=t, ins=s[0].ins[0..<t.nin], outs=s[0].outs[0..<t.nout], items=s[0].items[0..<t.nitems],
    index=index, amount=amount, spk=tx_unhex(spk, s[0].spk[..])};
}
show_hex32 :: (v: int) {
  digits := "0123456789abcdef";
  print("0x");
  for k: 0..<8 { pair: [1] u8; pair[0] = digits[(v >> (28 - 4*k)) & 15]; print("%", cast(string) pair[..]); }
}
final_word :: (f: bool) -> string { return ifx f then "final" else "not final"; }

// One transaction of tx_valid.json or tx_invalid.json, copied with its row
// number, and the one scriptPubKey its input spends.
Row :: struct { file: string; line: int; what: string; tx: string; spk: string; }
ROWS :: [12] Row.[
  Row.{file="tx_valid", line=102, what="CLTV 499999999, lock time 499999999",
    tx="0100000001000100000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000ff64cd1d", spk="04ff64cd1db1"},
  Row.{file="tx_invalid", line=67, what="CLTV 1, lock time 0",
    tx="010000000100010000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", spk="51b1"},
  Row.{file="tx_invalid", line=70, what="CLTV 500000001, lock time 500000000",
    tx="01000000010001000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000065cd1d", spk="040165cd1db1"},
  Row.{file="tx_invalid", line=86, what="CLTV 0 (a height), lock time 500000000 (a time)",
    tx="01000000010001000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000065cd1d", spk="00b151"},
  Row.{file="tx_invalid", line=81, what="CLTV 0, sequence 0xffffffff",
    tx="010000000100010000000000000000000000000000000000000000000000000000000000000000000000ffffffff0100000000000000000000000000", spk="00b151"},
  Row.{file="tx_invalid", line=76, what="CLTV -1",
    tx="010000000100010000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", spk="4fb1"},
  Row.{file="tx_valid", line=115, what="CLTV of a 5-byte zero",
    tx="010000000100010000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", spk="050000000000b151"},
  Row.{file="tx_invalid", line=96, what="CLTV of a 6-byte zero",
    tx="010000000100010000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", spk="06000000000000b151"},
  Row.{file="tx_invalid", line=105, what="CSV 1, sequence 0",
    tx="020000000100010000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", spk="51b2"},
  Row.{file="tx_invalid", line=108, what="CSV 0x00400001, sequence 0x00400000",
    tx="020000000100010000000000000000000000000000000000000000000000000000000000000000000000000040000100000000000000000000000000", spk="03010040b2"},
  Row.{file="tx_invalid", line=117, what="CSV 0 (blocks), sequence 0x00400000 (time)",
    tx="020000000100010000000000000000000000000000000000000000000000000000000000000000000000000040000100000000000000000000000000", spk="00b251"},
  Row.{file="tx_invalid", line=128, what="CSV 0, version 1",
    tx="010000000100010000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", spk="00b251"},
];

// Whether a script has OP_CHECKLOCKTIMEVERIFY or OP_CHECKSEQUENCEVERIFY
// (as NOP2 and NOP3 before their soft forks), read as GetScriptOp reads it.
uses_lock :: (s: [] u8) -> bool {
  pc := 0;
  while pc < s.count {
    o := script_op_vartime(s, pc);
    if !o.ok { return false; }
    if o.op == OP_CHECKLOCKTIMEVERIFY || o.op == OP_CHECKSEQUENCEVERIFY { return true; }
    pc = o.next;
  }
  return false;
}
// The same for a spend: its scriptPubKey, scriptSig or a witness item.
spend_uses_lock :: (sp: ScriptSpend) -> bool {
  if uses_lock(sp.spk) { return true; }
  i := sp.ins[sp.index];
  if uses_lock(sp.b[i.script..<i.script+i.script_n]) { return true; }
  for k: 0..<i.count {
    item := sp.items[i.first + k];
    if uses_lock(sp.b[item.at..<item.at+item.n]) { return true; }
  }
  return false;
}

main :: () {
  s: [1] Spend; w: [1] ScriptWork;
  x: [1] LtTx;

  // IsFinalTx: lock time 499,999,999 is a height and 500,000,000 a time.
  print("IsFinalTx\n");
  t := lt_read(x[..], "020000000100000000000000000000000000000000000000000000000000000000000000000000000000feffffff01000000000000000000ff64cd1d");
  b := x[0].b[..]; ins := x[0].ins[..];
  print("  lock time 499999999 in block 499999999: %\n", final_word(locktime_final_vartime(b, t, ins, 499999999, 0)));
  print("  lock time 499999999 in block 500000000: %\n", final_word(locktime_final_vartime(b, t, ins, 500000000, 0)));
  b[t.locktime] = 0x00; b[t.locktime+1] = 0x65;
  print("  lock time 500000000, cutoff 500000000: %\n", final_word(locktime_final_vartime(b, t, ins, 1000000000, 500000000)));
  print("  lock time 500000000, cutoff 500000001: %\n", final_word(locktime_final_vartime(b, t, ins, 0, 500000001)));
  for k: 0..<4 { b[ins[0].sequence + k] = 0xff; }
  print("  lock time 500000000, sequence 0xffffffff, cutoff 0: %\n", final_word(locktime_final_vartime(b, t, ins, 0, 0)));
  agree := 0;
  for v: LT_FINAL {
    y := lt_read(x[..], v.tx);
    if locktime_final_vartime(x[0].b[..], y, x[0].ins[..], v.height, v.time) == v.final { agree += 1; }
  }
  print("  reference cases: % of % agree\n", agree, LT_FINAL.count);

  // The median time past: block times need not rise, the median does not
  // fall.
  print("median time past\n");
  times := [11] int.[1700000000, 1700000660, 1700000300, 1700001500, 1700001200, 1700002400,
    1700001900, 1700003100, 1700002700, 1700003600, 1700003000];
  for n: [4] int.[1, 2, 6, 11] {
    print("  the first %: %\n", n, locktime_median_time_past_vartime(times[0..<n]));
  }
  agree = 0;
  for v: LT_MTP {
    window: [LOCKTIME_MEDIAN_SPAN] int;
    for k: 0..<v.count { window[k] = LT_MTP_TIMES[v.first + k]; }
    if locktime_median_time_past_vartime(window[0..<v.count]) == v.median { agree += 1; }
  }
  print("  reference windows: % of % agree\n", agree, LT_MTP.count);

  // BIP-68 sequences, written and read back.
  print("BIP-68 sequences\n");
  for blocks: [3] int.[10, 144, 65535] {
    print("  % blocks: ", blocks); show_hex32(locktime_sequence_blocks_vartime(blocks)); print("\n");
  }
  for seconds: [4] int.[1, 512, 3600, 33553920] {
    q := locktime_sequence_seconds_vartime(seconds);
    print("  % seconds: ", seconds); show_hex32(q);
    print(" (% seconds)\n", locktime_relative_vartime(q).seconds);
  }
  print("  33553921 seconds: %\n", locktime_sequence_seconds_vartime(33553921));
  for q: [4] int.[0x7fbf0001, 0x7fff0001, 0x80000000, 0xfffffffe] {
    r := locktime_relative_vartime(q);
    print("  "); show_hex32(q);
    if !r.enabled { print(": no relative lock\n"); }
    else if r.by_time { print(": % seconds\n", r.seconds); }
    else { print(": % blocks\n", r.blocks); }
  }

  // Sequence locks: a coin in block 100, the block before it with median
  // time past 1,700,000,000.
  print("sequence locks\n");
  heights := [1] int.[100]; coin_times := [1] int.[1700000000];
  for q: [4] int.[10, 0x00400001, 0x80000000 | 10, 0] {
    for version: [2] int.[2, 1] {
      raw := tx_unhex("020000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000", x[0].b[..]);
      y := tx_read_vartime(raw, true, x[0].ins[..], x[0].outs[..], x[0].items[..]);
      x[0].b[0] = cast(u8) version;
      for k: 0..<4 { x[0].b[x[0].ins[0].sequence + k] = cast(u8) ((q >> (8*k)) & 0xff); }
      lock := locktime_sequence_locks_vartime(x[0].b[..], y, x[0].ins[..], true, heights[..], coin_times[..]);
      print("  version %, sequence ", version); show_hex32(q);
      print(": last locked height %, time %", lock.height, lock.time);
      print("; block 110 with MTP 1700000511: %", ifx locktime_sequence_ok_vartime(lock, 110, 1700000511) then "ok" else "locked");
      print(", 1700000512: %\n", ifx locktime_sequence_ok_vartime(lock, 110, 1700000512) then "ok" else "locked");
      if version == 2 && q == 10 {
        print("    block 109: %\n", ifx locktime_sequence_ok_vartime(lock, 109, 1800000000) then "ok" else "locked");
      }
    }
  }
  agree = 0;
  for v: LT_SEQ {
    y := lt_read(x[..], v.tx);
    hs: [8] int; ts: [8] int;
    for k: 0..<y.nin { hs[k] = LT_SEQ_HEIGHTS[v.first + k]; ts[k] = LT_SEQ_TIMES[v.first + k]; }
    lock := locktime_sequence_locks_vartime(x[0].b[..], y, x[0].ins[..], v.enforce, hs[0..<y.nin], ts[0..<y.nin]);
    if lock.height == v.lock_height && lock.time == v.lock_time && locktime_sequence_ok_vartime(lock, v.height, v.prev_mtp) == v.ok { agree += 1; }
  }
  print("  reference cases: % of % agree\n", agree, LT_SEQ.count);

  // Rows of Core's transaction tests, with each opcode's flag on and off.
  print("Bitcoin Core rows, flags off / CLTV and CSV on\n");
  locks := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY;
  for r: ROWS {
    off := script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, 0), SCRIPT_VERIFY_P2SH, w[..]);
    on := script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, 0), locks, w[..]);
    print("  % %, %: % / %\n", r.file, r.line, r.what, SCRIPT_ERR_NAMES[off], SCRIPT_ERR_NAMES[on]);
  }

  // Every script_tests.json row and every tx_valid.json and
  // tx_invalid.json flag set that reaches one of the two opcodes.
  print("Bitcoin Core rows that use CHECKLOCKTIMEVERIFY or CHECKSEQUENCEVERIFY\n");
  rows := 0; same := 0;
  for r: SCRIPT_ROWS {
    sp := spend_of(s[..], r.tx, r.spk, 0, r.amount);
    if !spend_uses_lock(sp) { continue; }
    rows += 1;
    if script_verify_vartime(sp, r.flags, w[..]) == r.error { same += 1; }
  }
  print("  script_tests.json: % of % rows give Core's result\n", same, rows);
  txs: [2] int; sets: [2] int; agree_sets: [2] int; last := -1;
  for c: SCRIPT_TX_CASES {
    e := SCRIPT_TXS[c.tx];
    uses := false;
    for k: 0..<e.count {
      p := SCRIPT_PREVS[e.first + k];
      if spend_uses_lock(spend_of(s[..], e.tx, p.spk, k, p.amount)) { uses = true; }
    }
    if !uses { continue; }
    file := ifx e.valid then 0 else 1;
    if c.tx != last { txs[file] += 1; last = c.tx; }
    sets[file] += 1;
    valid := true;
    for k: 0..<e.count {
      p := SCRIPT_PREVS[e.first + k];
      if script_verify_vartime(spend_of(s[..], e.tx, p.spk, k, p.amount), c.flags, w[..]) != SCRIPT_ERR_OK { valid = false; break; }
    }
    if valid == c.valid { agree_sets[file] += 1; }
  }
  print("  tx_valid.json: % transactions, % of % flag sets give Core's verdict\n", txs[0], agree_sets[0], sets[0]);
  print("  tx_invalid.json: % transactions, % of % flag sets give Core's verdict\n", txs[1], agree_sets[1], sets[1]);
}
```

It prints:

```text
IsFinalTx
  lock time 499999999 in block 499999999: not final
  lock time 499999999 in block 500000000: final
  lock time 500000000, cutoff 500000000: not final
  lock time 500000000, cutoff 500000001: final
  lock time 500000000, sequence 0xffffffff, cutoff 0: final
  reference cases: 105 of 105 agree
median time past
  the first 1: 1700000000
  the first 2: 1700000660
  the first 6: 1700001200
  the first 11: 1700001900
  reference windows: 24 of 24 agree
BIP-68 sequences
  10 blocks: 0x0000000a
  144 blocks: 0x00000090
  65535 blocks: 0x0000ffff
  1 seconds: 0x00400001 (512 seconds)
  512 seconds: 0x00400001 (512 seconds)
  3600 seconds: 0x00400008 (4096 seconds)
  33553920 seconds: 0x0040ffff (33553920 seconds)
  33553921 seconds: -1
  0x7fbf0001: 1 blocks
  0x7fff0001: 512 seconds
  0x80000000: no relative lock
  0xfffffffe: no relative lock
sequence locks
  version 2, sequence 0x0000000a: last locked height 109, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
    block 109: locked
  version 1, sequence 0x0000000a: last locked height -1, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
  version 2, sequence 0x00400001: last locked height -1, time 1700000511; block 110 with MTP 1700000511: locked, 1700000512: ok
  version 1, sequence 0x00400001: last locked height -1, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
  version 2, sequence 0x8000000a: last locked height -1, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
  version 1, sequence 0x8000000a: last locked height -1, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
  version 2, sequence 0x00000000: last locked height 99, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
  version 1, sequence 0x00000000: last locked height -1, time -1; block 110 with MTP 1700000511: ok, 1700000512: ok
  reference cases: 63 of 63 agree
Bitcoin Core rows, flags off / CLTV and CSV on
  tx_valid 102, CLTV 499999999, lock time 499999999: OK / OK
  tx_invalid 67, CLTV 1, lock time 0: OK / UNSATISFIED_LOCKTIME
  tx_invalid 70, CLTV 500000001, lock time 500000000: OK / UNSATISFIED_LOCKTIME
  tx_invalid 86, CLTV 0 (a height), lock time 500000000 (a time): OK / UNSATISFIED_LOCKTIME
  tx_invalid 81, CLTV 0, sequence 0xffffffff: OK / UNSATISFIED_LOCKTIME
  tx_invalid 76, CLTV -1: OK / NEGATIVE_LOCKTIME
  tx_valid 115, CLTV of a 5-byte zero: OK / OK
  tx_invalid 96, CLTV of a 6-byte zero: OK / SCRIPTNUM
  tx_invalid 105, CSV 1, sequence 0: OK / UNSATISFIED_LOCKTIME
  tx_invalid 108, CSV 0x00400001, sequence 0x00400000: OK / UNSATISFIED_LOCKTIME
  tx_invalid 117, CSV 0 (blocks), sequence 0x00400000 (time): OK / UNSATISFIED_LOCKTIME
  tx_invalid 128, CSV 0, version 1: OK / UNSATISFIED_LOCKTIME
Bitcoin Core rows that use CHECKLOCKTIMEVERIFY or CHECKSEQUENCEVERIFY
  script_tests.json: 21 of 21 rows give Core's result
  tx_valid.json: 40 transactions, 880 of 880 flag sets give Core's verdict
  tx_invalid.json: 36 transactions, 792 of 792 flag sets give Core's verdict
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/timelocks.out](../fixtures/timelocks.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/timelocks.coax](../examples/checks/timelocks.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/timelocks.coax check -->
```jai
// Each copied row gives Core's verdict with the opcodes' flags on, and
// spends with them off: before its soft fork each opcode was a NOP.
#test "Core's lock-time rows, flags on and off" #steps 200000000 {
  s: [1] Spend; w: [1] ScriptWork;
  locks := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY;
  want := [12] int.[SCRIPT_ERR_OK, SCRIPT_ERR_UNSATISFIED_LOCKTIME, SCRIPT_ERR_UNSATISFIED_LOCKTIME,
    SCRIPT_ERR_UNSATISFIED_LOCKTIME, SCRIPT_ERR_UNSATISFIED_LOCKTIME, SCRIPT_ERR_NEGATIVE_LOCKTIME,
    SCRIPT_ERR_OK, SCRIPT_ERR_SCRIPTNUM, SCRIPT_ERR_UNSATISFIED_LOCKTIME, SCRIPT_ERR_UNSATISFIED_LOCKTIME,
    SCRIPT_ERR_UNSATISFIED_LOCKTIME, SCRIPT_ERR_UNSATISFIED_LOCKTIME];
  for r, i: ROWS {
    #assert script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, 0), locks, w[..]) == want[i];
    #assert script_verify_vartime(spend_of(s[..], r.tx, r.spk, 0, 0), SCRIPT_VERIFY_P2SH, w[..]) == SCRIPT_ERR_OK;
  }
  // tx_valid 115's 5-byte zero is a CScriptNum MINIMALDATA refuses.
  #assert script_verify_vartime(spend_of(s[..], ROWS[6].tx, ROWS[6].spk, 0, 0), locks | SCRIPT_VERIFY_MINIMALDATA, w[..]) == SCRIPT_ERR_SCRIPTNUM;
  // tx_invalid 128 at version 2 spends.
  sp := spend_of(s[..], ROWS[11].tx, ROWS[11].spk, 0, 0);
  sp.b[0] = 2;
  #assert script_verify_vartime(sp, locks, w[..]) == SCRIPT_ERR_OK;
}
// CSV and BIP-68 read a sequence the same way: a script's CSV argument
// passes exactly when the input's own sequence lock is at least as long
// and of the same kind.
#forall "CSV agrees with BIP-68's reading" i: 0..<64 {
  kinds := [4] int.[0, SCRIPT_SEQUENCE_TYPE, 0x7fbf0000, 0x7fbf0000 | SCRIPT_SEQUENCE_TYPE];
  have := kinds[i & 3] | ((i * 977) & 0xff);
  want := kinds[(i >> 2) & 3] | ((i * 131) & 0xff);
  a := locktime_relative_vartime(have); b := locktime_relative_vartime(want);
  x: [1] LtTx;
  t := lt_read(x[..], "020000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000");
  for k: 0..<4 { x[0].b[x[0].ins[0].sequence + k] = cast(u8) ((have >> (8*k)) & 0xff); }
  sp := ScriptSpend.{b=x[0].b[..], t=t, ins=x[0].ins[0..<1], outs=x[0].outs[0..<1], items=x[0].items[0..<0]};
  same_kind := a.by_time == b.by_time;
  longer := ifx a.by_time then a.seconds >= b.seconds else a.blocks >= b.blocks;
  #assert si_sequence(sp, want) == (same_kind && longer);
}
// A coin locked for n blocks or n units of 512 seconds is spendable first
// in the block n after its own, or once the previous block's median time
// past is n * 512 seconds past the one before the coin's block.
#forall "relative locks open exactly on time" n: 1..<300 {
  x: [1] LtTx;
  t := lt_read(x[..], "020000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000");
  heights := [1] int.[1000]; times := [1] int.[1700000000];
  for kind: 0..<2 {
    q := ifx kind == 0 then locktime_sequence_blocks_vartime(n) else locktime_sequence_seconds_vartime(n * 512);
    for k: 0..<4 { x[0].b[x[0].ins[0].sequence + k] = cast(u8) ((q >> (8*k)) & 0xff); }
    lock := locktime_sequence_locks_vartime(x[0].b[..], t, x[0].ins[..], true, heights[..], times[..]);
    if kind == 0 {
      #assert !locktime_sequence_ok_vartime(lock, 1000 + n - 1, 2000000000) && locktime_sequence_ok_vartime(lock, 1000 + n, 2000000000);
    } else {
      #assert !locktime_sequence_ok_vartime(lock, 5000, 1700000000 + n * 512 - 1) && locktime_sequence_ok_vartime(lock, 5000, 1700000000 + n * 512);
    }
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/timelocks.coax --run-steps 250000000
```

On 2026-11-03 `coax test` runs 184 checks, including the loaded files'
own checks, all passing. The first runs each copied row with the flags
on and off; the second compares `si_sequence` with BIP-68's reading over
64 pairs of sequences and arguments, ignored bits among them; the third
checks that a lock of 1 to 299 blocks or units opens on exactly the
height or median time past it names.

### The library's own checks

`locktime.coax` checks `IsFinalTx` on each side of the 500,000,000
threshold and with final sequences, the median time past with odd and
even counts, and BIP-68 encodings that round-trip, among them `0x7fbf0001`
read as 1 block. It runs every case of
[locktime_vectors.coax](../../coax/tests/bitcoin/locktime_vectors.coax):
105 `IsFinalTx` cases, 24 windows of block times and 63 sequence-lock
cases on a seeded chain of 200 blocks whose times sometimes run
backwards, with versions 0, 1, 2, 3, `0x80000000` and `0xffffffff`, the
disable and type bits and the ignored bits.

```shell
coax/bin/coax test coax/lib/bitcoin/locktime.coax --run-steps 250000000
```

On 2026-11-03 `coax test` runs 181 checks, including `script.coax`'s, all
passing.

### Core in Python

Core publishes no vectors for these rules: its unit tests build a chain
in memory.
[make_locktime_vectors.py](../../coax/tests/bitcoin/make_locktime_vectors.py)
transliterates `IsFinalTx`, `CalculateSequenceLocks` and
`EvaluateSequenceLocks` from the pinned `consensus/tx_verify.cpp` and
`GetMedianTimePast` from the pinned `chain.h`, checks both files' SHA-256,
and writes the vectors with its own verdicts. It shares no code with Coax.

```shell
python3 -I coax/tests/bitcoin/make_locktime_vectors.py
```

[locktime_examples.py](../../coax/tests/bitcoin/locktime_examples.py)
recomputes the example's output with that transliteration and with
[script_reference.py](../../coax/tests/bitcoin/script_reference.py)'s
`VerifyScript`. It checks that each copied row is that row of the pinned
JSON file, recomputes both verdicts for it, and recounts the Core rows
that reach either opcode with the reference's verdicts.

```shell
python3 -I coax/tests/bitcoin/locktime_examples.py bitcoin/fixtures/timelocks.out
```

On 2026-11-03 it reports 1,733 values that agree with the reference.
Chapter 36's
[script.py](../../coax/tests/bitcoin/script.py) runs the opcodes'
`script_tests.json` and transaction cases natively and in Wasm against the
same reference. `locktime.coax` itself is checked natively only.

Mutation testing was not run for this chapter (owner speed order).

## What this chapter leaves out

- Which flags apply at which height, and BIP-9 deployment: chapter 55.
- The mempool's checks, which test a transaction against the next block
  (`CheckFinalTxAtTip`, `CheckSequenceLocksAtTip`): chapter 82.
- Replace-by-fee and its sequence signaling: chapter 84.
- Taproot's script paths, where the same opcodes run in tapscript:
  chapter 44.
- Lightning's commitment and HTLC transactions, which use both locks:
  chapter 108. Timelocks in miniscript: chapter 98.

## Summary

- A lock time below 500,000,000 is a height and from there a time. It is
  the last height or time at which the transaction is still locked, and
  it doesn't apply when every input's sequence is `0xffffffff`.
- Time locks compare with the median time past of the last 11 blocks
  (BIP-113), which never decreases and can't be moved by one miner.
- `OP_CHECKLOCKTIMEVERIFY` requires the transaction's lock time to be at
  least its argument, of the same kind, with this input not final.
- BIP-68: in version 2 transactions, a sequence without bit 31 locks its
  input for its low 16 bits of blocks, or of 512-second units with bit 22,
  counted from the coin's block.
- `OP_CHECKSEQUENCEVERIFY` requires this input's sequence to carry a
  relative lock at least as long as its argument, of the same type.

## Dependencies

- [36. The script interpreter](ch036-the-script-interpreter.md)

## Sources

- [BIP-65](https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki)
  (`OP_CHECKLOCKTIMEVERIFY`),
  [BIP-68](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki)
  (relative lock times),
  [BIP-112](https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki)
  (`OP_CHECKSEQUENCEVERIFY`) and
  [BIP-113](https://github.com/bitcoin/bips/blob/master/bip-0113.mediawiki)
  (median time past).
- Bitcoin Core at revision `ddea9196`: `consensus/tx_verify.cpp`
  (`IsFinalTx`, `CalculateSequenceLocks`, `EvaluateSequenceLocks`),
  `chain.h` (`GetMedianTimePast`), and `script/interpreter.cpp`
  (`CheckLockTime`, `CheckSequence`), pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/tx_verify.cpp).
- Bitcoin Core's `tx_valid.json`, `tx_invalid.json` and
  `script_tests.json` at the same revision, pinned beside them.
- Mainnet activation heights: Core's `kernel/chainparams.cpp`
  (`BIP65Height`, `CSVHeight`).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
