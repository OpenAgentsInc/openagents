# 54. The subsidy and the coinbase

Every block's first transaction is its coinbase. It spends no coins: it
creates them. The coinbase is how new bitcoin enter circulation and how a
miner collects the fees of the transactions it confirms, so a node must
check exactly how much it may create. It must also check that the
coinbase names the block's height, because two coinbases of 2010 were
byte-for-byte copies of earlier ones.

This chapter writes Bitcoin Core's `GetBlockSubsidy`, the rule that a
coinbase claims at most the subsidy plus the block's fees, and the BIP-34
rule that the coinbase starts with the block's height. Fourteen pinned
mainnet coinbases, each proved in its block's header, pass at their
heights, among them both sides of every halving so far. The two repeated
coinbases of 2010 have the same txids as the coinbases they copied, and
their blocks are chapter 50's BIP-30 exceptions.

## What this chapter teaches

- The subsidy: 50 bitcoin, halved every 210,000 blocks, and zero after
  the 64th halving (BIP-42).
- Why the total is 20,999,999.9769 bitcoin and not 21 million.
- What a coinbase may claim: the subsidy and the fees, and not a satoshi
  more.
- How Core writes a height into a script, and why the coinbase starts
  with it (BIP-34).
- Why two coinbases have the same txids as earlier ones (BIP-30).

## Where Bitcoin checks the coinbase

| Rule | Who checks it | Chapter |
| --- | --- | --- |
| One coinbase, first in the block | `CheckBlock` | 53 |
| scriptSig of 2 to 100 bytes | `CheckTransaction` | 52 |
| The height at the start of the scriptSig | `ContextualCheckBlock` | 54 |
| The witness commitment in an output | `ContextualCheckBlock` | 49 |
| The claim: subsidy plus fees at most | `ConnectBlock` | 54 |
| The fees' running total within `MAX_MONEY` | `ConnectBlock` | 54 |
| No txid that is already unspent, except two blocks | `ConnectBlock` (BIP-30) | 50, 55, 58 |
| Outputs spent only after 100 blocks | `CheckTxInputs` | 52 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [subsidy.coax](../../coax/lib/bitcoin/subsidy.coax) | `subsidy_vartime` | Core's `GetBlockSubsidy` |
| | `subsidy_total_vartime` | Every subsidy there will be |
| | `subsidy_height_script_vartime` | `CScript() << nHeight` |
| | `subsidy_height_check_vartime`, `subsidy_starts_with_height_vartime` | `ContextualCheckBlock`'s BIP-34 rule |
| | `subsidy_add_fee_vartime` | `ConnectBlock`'s fee total |
| | `subsidy_claim_vartime`, `subsidy_amount_check_vartime` | `ConnectBlock`'s coinbase rule |
| [chainparams.coax](../../coax/lib/bitcoin/chainparams.coax) | `chain_deployment_active`, `chain_bip30_repeat_vartime` | chapter 50's BIP-34 height and BIP-30 blocks |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_num_write_vartime` | chapter 36's `CScriptNum::serialize` |
| [txcheck.coax](../../coax/lib/bitcoin/txcheck.coax) | `txcheck_inputs_vartime` | chapter 52's `CheckTxInputs`, which gives each fee |

## Result codes

Each code is one of Core's reject reasons:

<!-- source: coax/lib/bitcoin/subsidy.coax codes -->
```jai
// Result codes, with Core's reject reason.
SUBSIDY_OK :: 0;
SUBSIDY_CB_HEIGHT :: 1;    // "bad-cb-height"
SUBSIDY_CB_AMOUNT :: 2;    // "bad-cb-amount"
SUBSIDY_FEE_RANGE :: 3;    // "bad-txns-accumulated-fee-outofrange"
SUBSIDY_CONTEXT :: 4;      // the transaction is not a coinbase, or did not read
SUBSIDY_REASONS :: [5] string.["ok", "bad-cb-height", "bad-cb-amount", "bad-txns-accumulated-fee-outofrange", "bad context"];
```

## The subsidy

A block may create a fixed amount of new bitcoin, its subsidy. It started
at 50 bitcoin and halves every 210,000 blocks, about every four years:

<!-- source: coax/lib/bitcoin/subsidy.coax subsidy -->
```jai
SUBSIDY_INITIAL :: 50 * CHAIN_COIN;   // 50 bitcoin
SUBSIDY_MAX_HALVINGS :: 64;           // a shift of 64 or more is undefined in C++
// GetBlockSubsidy: 50 bitcoin shifted right once per halving interval,
// and 0 from the 64th halving.
subsidy_vartime :: (height: int, halving: int) -> int #export_c {
  halvings := height / halving;
  if halvings >= SUBSIDY_MAX_HALVINGS { return 0; }
  return SUBSIDY_INITIAL >> halvings;
}
// Every subsidy there will ever be: each interval's subsidy times the
// interval, until the subsidy is 0.
subsidy_total_vartime :: (halving: int) -> int #export_c {
  total := 0;
  for i: 0..<SUBSIDY_MAX_HALVINGS { total += subsidy_vartime(i * halving, halving) * halving; }
  return total;
}
```

The halving is a right shift of a 64-bit number of satoshis. A shift of
64 or more bits is undefined in C++, and on x86 a shift by 64 shifts by
0, so before 2014 the subsidy would have returned to 50 bitcoin at the
64th halving, about 256 years after the genesis block. BIP-42 made the
subsidy 0 from there on. By then it has long been 0 anyway: 5,000,000,000 satoshis shifted 33
times is 0.

```text
GetBlockSubsidy on mainnet, a halving every 210000 blocks:
  halving 0 from height 0: 5000000000
  halving 1 from height 210000: 2500000000
  halving 2 from height 420000: 1250000000
  halving 3 from height 630000: 625000000
  halving 4 from height 840000: 312500000
...
  halving 31 from height 6510000: 2
  halving 32 from height 6720000: 1
  halving 33 from height 6930000: 0
  halving 34 from height 7140000: 0
  the last satoshi at height 6929999; the halving at 64: 0
  every subsidy there will be: 2099999997690000 satoshis
  regtest, every 150 blocks: 5000000000 at 149, 2500000000 at 150, 1499999998350 in all
```

Each shift drops the remainder of an odd number of satoshis, from the
tenth halving on, so the sum of every subsidy is 2,099,999,997,690,000
satoshis: 20,999,999.9769 bitcoin. That is the value Core's
`subsidy_limit_test` checks, by summing every thousandth height. Regtest
halves every 150 blocks, so that tests can reach a halving.

The supply that can be spent is smaller still. The genesis block's
coinbase was never added to the set of unspent outputs, and the outputs
of two coinbases of 2010 were overwritten, as the BIP-30 section shows.

## The height in the coinbase

A coinbase has one input, with a null outpoint, and its scriptSig is free
data, 2 to 100 bytes (chapter 52). Before 2012 nothing made two coinbases
differ: a coinbase that paid the same key with the same scriptSig as an
earlier one serialized to the same bytes and had the same txid.

BIP-34 made each coinbase unique by requiring its scriptSig to start with
the block's height. Core writes the height as `CScript() << nHeight`,
which turns -1 and 1 to 16 into one opcode, 0 into `OP_0`, and any other
number into a push of its minimal `CScriptNum` (chapter 36):

<!-- source: coax/lib/bitcoin/subsidy.coax height -->
```jai
// CScript() << nHeight: -1 and 1 to 16 as one opcode (OP_1NEGATE, OP_1 to
// OP_16), 0 as OP_0, anything else as a push of its minimal CScriptNum.
// Writes the bytes to out (10 bytes of room); returns their length.
subsidy_height_script_vartime :: (height: int, out: [] u8) -> int #export_c {
  if height == -1 || (height >= 1 && height <= 16) { out[0] = cast(u8) (height + 0x50); return 1; }
  if height == 0 { out[0] = OP_0; return 1; }
  num: [9] u8;
  n := script_num_write_vartime(height, num[..]);
  out[0] = cast(u8) n;
  for i: 0..<n { out[1+i] = num[i]; }
  return n + 1;
}
// ContextualCheckBlock's BIP-34 rule for a block at height whose coinbase
// is t: once BIP-34 is active, the coinbase's scriptSig starts with the
// height's script. Bytes after it are free.
subsidy_height_check_vartime :: (p: ChainParams, b: [] u8, t: Tx, ins: [] TxIn, height: int) -> int #export_c {
  if t.code != TX_OK || !tx_coinbase_vartime(b, t, ins) { return SUBSIDY_CONTEXT; }
  if !chain_deployment_active(p, DEP_HEIGHTINCB, height) { return SUBSIDY_OK; }
  return ifx subsidy_starts_with_height_vartime(b, ins, height) then SUBSIDY_OK else SUBSIDY_CB_HEIGHT;
}
// Whether the first input's scriptSig starts with height's script.
subsidy_starts_with_height_vartime :: (b: [] u8, ins: [] TxIn, height: int) -> bool #export_c {
  want: [10] u8;
  n := subsidy_height_script_vartime(height, want[..]);
  if ins[0].script_n < n { return false; }
  for i: 0..<n { if b[ins[0].script+i] != want[i] { return false; } }
  return true;
}
```

The rule compares bytes: the scriptSig must start with exactly the bytes
Core writes, and anything may follow them. A height of 128 needs a second
byte, `0x00`, because `0x80` alone would be negative zero; 32,768 needs a
third for the same reason.

```text
The height a coinbase starts with (CScript() << nHeight):
  0: 00
  1: 51
  16: 60
  17: 0111
  127: 017f
  128: 028000
  255: 02ff00
  256: 020001
  32768: 03008000
  227931: 035b7a03
  514999: 03b7db07
  840000: 0340d10c
```

BIP-34 applied to every block from mainnet height 227,931, once 950 of
1,000 blocks signalled version 2. Core now hard-codes that height (BIP-90), and chapter 50 holds
it in `bip34_height`. Below it the rule does not apply, although most
miners already wrote the height: block 209,999's coinbase starts with it.

## What a coinbase may claim

A coinbase's outputs may add up to at most the subsidy at its height plus
the fees of every other transaction in the block. Each fee is the inputs
minus the outputs of one transaction, which chapter 52's `CheckTxInputs`
computes from the coins it spends. `ConnectBlock` adds the fees in
order, refusing once the total leaves the range 0 to `MAX_MONEY`:

<!-- source: coax/lib/bitcoin/subsidy.coax amount -->
```jai
// ConnectBlock's fee total: each transaction's fee added in turn, refused
// once the total leaves 0..MAX_MONEY.
subsidy_add_fee_vartime :: (total: int, fee: int) -> int #export_c {
  if total < 0 || fee < 0 { return -1; }
  sum := total + fee;
  return ifx txc_money_range(sum) then sum else -1;
}
// The coinbase's claim: the sum of its outputs. CheckTransaction (chapter
// 52) has kept each value and the sum within MAX_MONEY.
subsidy_claim_vartime :: (b: [] u8, t: Tx, outs: [] TxOut) -> int #export_c {
  claim := 0;
  for k: 0..<t.nout { claim += tx_le_vartime(b, outs[k].value, 8); }
  return claim;
}
// ConnectBlock's coinbase rule: the coinbase may claim at most the
// subsidy at its height plus the fees of the block's other transactions.
subsidy_amount_check_vartime :: (p: ChainParams, b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut, height: int, fees: int) -> int #export_c {
  if t.code != TX_OK || !tx_coinbase_vartime(b, t, ins) || fees < 0 { return SUBSIDY_CONTEXT; }
  if subsidy_claim_vartime(b, t, outs) > subsidy_vartime(height, p.halving) + fees { return SUBSIDY_CB_AMOUNT; }
  return SUBSIDY_OK;
}
```

The claim counts every output, including the zero-value output that holds
chapter 49's witness commitment. A coinbase may claim less than it is
allowed: what it leaves out is never created.

## Pinned coinbases

The fourteen pinned blocks are the four of the repeated coinbases, both
sides of BIP-34's activation, and both sides of each halving so far.
Each coinbase is proved to be in its block: its header hashes to the
block's hash, and its Merkle branch leads from its txid to the header's
root (chapter 46).

```text
Pinned mainnet coinbases, each proved in its header by its Merkle branch:
  91722: proved yes, starts with its height no, BIP-34 ok, claims 5000000000 of subsidy 5000000000 and fees 0: ok
  91812: proved yes, starts with its height no, BIP-34 ok, claims 5000000000 of subsidy 5000000000 and fees 0: ok
  91842: proved yes, starts with its height no, BIP-34 ok, claims 5000000000 of subsidy 5000000000 and fees 0: ok
  91880: proved yes, starts with its height no, BIP-34 ok, claims 5000000000 of subsidy 5000000000 and fees 0: ok
  209999: proved yes, starts with its height yes, BIP-34 ok, claims 5053620000 of subsidy 5000000000 and fees 53620000: ok
  210000: proved yes, starts with its height yes, BIP-34 ok, claims 3856295554 of subsidy 2500000000 and fees 1356295554: ok
  227930: proved yes, starts with its height yes, BIP-34 ok, claims 2508015017 of subsidy 2500000000 and fees 8015017: ok
  227931: proved yes, starts with its height yes, BIP-34 ok, claims 2528700000 of subsidy 2500000000 and fees 28700000: ok
  419999: proved yes, starts with its height yes, BIP-34 ok, claims 2500000000 of subsidy 2500000000 and fees 0: ok
  420000: proved yes, starts with its height yes, BIP-34 ok, claims 1307569681 of subsidy 1250000000 and fees 57569681: ok
  629999: proved yes, starts with its height yes, BIP-34 ok, claims 1356876397 of subsidy 1250000000 and fees 106876397: ok
  630000: proved yes, starts with its height yes, BIP-34 ok, claims 715968084 of subsidy 625000000 and fees 90968084: ok
  839999: proved yes, starts with its height yes, BIP-34 ok, claims 729455386 of subsidy 625000000 and fees 104455386: ok
  840000: proved yes, starts with its height yes, BIP-34 ok, claims 4075061499 of subsidy 312500000 and fees 3762561499: ok
```

Each miner claimed exactly the subsidy and the fees mempool.space reports.
Block 840,000 is the fourth halving: its subsidy is 3.125 bitcoin, and its
fees, 37.6 bitcoin, are more than ten times that. The Runes protocol
launched at that block, and its first transactions bid for a place in it.

## The repeated coinbases (BIP-30)

```text
Coinbases with the same txid (BIP-30):
  91722 and 91880: the same txid yes; 91880 is one of Core's BIP-30 exceptions: yes
  91812 and 91842: the same txid yes; 91842 is one of Core's BIP-30 exceptions: yes
```

Block 91,880's coinbase is a copy of block 91,722's, and block 91,842's
of block 91,812's. The second copy replaced the first in every node's
set of unspent outputs, so the first coinbase's 50 bitcoin can never be
spent.

BIP-30 forbids a transaction whose txid is still unspent. Core exempts
the two blocks that broke it, by height and hash, and chapter 50's
`chain_bip30_repeat_vartime` names them. The check needs the set of
unspent outputs, so it runs in `ConnectBlock`: chapter 55 says when it
applies, and chapter 58 connects blocks. From BIP-34 the height makes
every later coinbase differ from these, so Core skips the check until
height 1,983,702, where a height could match an early coinbase's free
scriptSig.

## Mainnet block 514,999

Chapter 53 checked the sigops of block 514,999 with the outputs it
spends. Those outputs also give its fees:

```text
Mainnet block 514,999:
  fees from the outputs its transactions spend: 49221
  the coinbase claims 1250049221 of subsidy 1250000000 and fees 49221: ok
  one satoshi more: bad-cb-amount
  its height: ok
  with the height's first byte changed: bad-cb-height at 514999, ok at 227930
  fees whose total passes MAX_MONEY: bad-txns-accumulated-fee-outofrange
```

The coinbase claims 12.5 bitcoin and 49,221 satoshis, exactly the subsidy
and the fees. One satoshi more refuses as `bad-cb-amount`. A changed
height refuses at 514,999 and passes at 227,930, below BIP-34.

## The audit

Chapter 50's `chainparams.coax` held each chain's halving interval, its
BIP-34 height and the BIP-30 blocks, and chapter 52's `txcheck.coax`
checked a coinbase's scriptSig length and computed each fee. Nothing
computed a subsidy, wrote a height as Core writes it, checked a coinbase's
height or its claim, or added a block's fees before this chapter.

## The example

The example is
[bitcoin/examples/subsidy-coinbase.coax](../examples/subsidy-coinbase.coax).
It prints the subsidy at each halving, the height scripts, the pinned
coinbases and block 514,999's fees and claim.

<!-- source: bitcoin/examples/subsidy-coinbase.coax example -->
```jai
#load "../../coax/lib/bitcoin/subsidy.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [64] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }

main :: () {
  m := CHAIN_MAINNET_PARAMS;

  // The subsidy at every halving.
  print("GetBlockSubsidy on mainnet, a halving every % blocks:\n", m.halving);
  for i: 0..<35 { print("  halving % from height %: %\n", i, i * m.halving, subsidy_vartime(i * m.halving, m.halving)); }
  print("  the last satoshi at height %; the halving at 64: %\n", 33 * m.halving - 1, subsidy_vartime(64 * m.halving, m.halving));
  print("  every subsidy there will be: % satoshis\n", subsidy_total_vartime(m.halving));
  r := CHAIN_REGTEST_PARAMS;
  print("  regtest, every % blocks: % at 149, % at 150, % in all\n", r.halving, subsidy_vartime(149, r.halving), subsidy_vartime(150, r.halving), subsidy_total_vartime(r.halving));

  // CScript() << nHeight for heights around the encodings' edges.
  print("The height a coinbase starts with (CScript() << nHeight):\n");
  heights := [12] int.[0, 1, 16, 17, 127, 128, 255, 256, 32768, 227931, 514999, 840000];
  for i: 0..<12 {
    out: [10] u8;
    n := subsidy_height_script_vartime(heights[i], out[..]);
    print("  %: ", heights[i]);
    put_hex(out[0..<n]);
    print("\n");
  }

  // The pinned coinbases: proved in their headers, their heights and claims.
  print("Pinned mainnet coinbases, each proved in its header by its Merkle branch:\n");
  raw: [512] u8; ins: [4] TxIn; outs: [8] TxOut; items: [8] TxItem; ids: [14 * 32] u8;
  for i: 0..<SUBSIDY_PROOFS.count {
    c := SUBSIDY_PROOFS[i];
    x := sb_read(c.coinbase, raw[..], ins[..], outs[..], items[..]);
    b := raw[0..<x.n];
    #assert tx_txid_vartime(b, x.t, ids[32*i..<32*i+32]);
    print("  %: proved %, starts with its height %, BIP-34 %, claims % of subsidy % and fees %: %\n", c.height, yes(sb_proved(i)),
      yes(subsidy_starts_with_height_vartime(b, ins[..], c.height)), reason(subsidy_height_check_vartime(m, b, x.t, ins[..], c.height)),
      subsidy_claim_vartime(b, x.t, outs[..]), subsidy_vartime(c.height, m.halving), c.fees,
      reason(subsidy_amount_check_vartime(m, b, x.t, ins[..], outs[..], c.height, c.fees)));
  }

  // BIP-30: two pairs of these coinbases have one txid.
  print("Coinbases with the same txid (BIP-30):\n");
  // SUBSIDY_PROOFS starts with blocks 91,722, 91,812, 91,842 and 91,880:
  // 91,880 repeats 91,722's coinbase and 91,842 repeats 91,812's.
  for k: 0..<2 {
    a := k; z := 3 - k;
    same := true;
    for j: 0..<32 { if ids[32*a+j] != ids[32*z+j] { same = false; } }
    h: [80] u8; hash: [32] u8;
    #assert cp_unhex(SUBSIDY_PROOFS[z].header, h[..]) == 80 && pow_block_hash_vartime(h[..], hash[..]) == POW_OK;
    print("  % and %: the same txid %; % is one of Core's BIP-30 exceptions: %\n", SUBSIDY_PROOFS[a].height, SUBSIDY_PROOFS[z].height,
      yes(same), SUBSIDY_PROOFS[z].height, yes(chain_bip30_repeat_vartime(SUBSIDY_PROOFS[z].height, hash[..])));
  }

  // Block 514,999: its fees from the outputs it spends, and its coinbase.
  print("Mainnet block 514,999:\n");
  blk: [6000] u8;
  n := sg_block(blk[..]);
  fees := sb_fees(blk[0..<n]);
  print("  fees from the outputs its transactions spend: %\n", fees);
  cb := blk[81..<sg_first_end(blk[0..<n])];
  t := tx_read_vartime(cb, true, ins[..], outs[..], items[..]);
  print("  the coinbase claims % of subsidy % and fees %: %\n", subsidy_claim_vartime(cb, t, outs[..]), subsidy_vartime(514999, m.halving), fees,
    reason(subsidy_amount_check_vartime(m, cb, t, ins[..], outs[..], 514999, fees)));
  txc_put(cb, outs[0].value, tx_le_vartime(cb, outs[0].value, 8) + 1, 8);
  print("  one satoshi more: %\n", reason(subsidy_amount_check_vartime(m, cb, t, ins[..], outs[..], 514999, fees)));
  txc_put(cb, outs[0].value, tx_le_vartime(cb, outs[0].value, 8) - 1, 8);
  print("  its height: %\n", reason(subsidy_height_check_vartime(m, cb, t, ins[..], 514999)));
  cb[ins[0].script + 1] ^= 1;
  print("  with the height's first byte changed: % at 514999, % at 227930\n",
    reason(subsidy_height_check_vartime(m, cb, t, ins[..], 514999)), reason(subsidy_height_check_vartime(m, cb, t, ins[..], 227930)));
  print("  fees whose total passes MAX_MONEY: %\n", ifx subsidy_add_fee_vartime(FEE_MAX_MONEY, 1) < 0 then SUBSIDY_REASONS[SUBSIDY_FEE_RANGE] else "ok");
}
reason :: (code: int) -> string { return SUBSIDY_REASONS[code]; }
// The fees of block 514,999's transactions, with chapter 52's
// CheckTxInputs over the values of the outputs they spend.
sb_fees :: (blk: [] u8) -> int {
  ins: [64] TxIn; outs: [64] TxOut; items: [64] TxItem; coins: [8] TxcCoin;
  at := 81; used := 0; total := 0;
  for i: 0..<19 {
    s := blk[at..<blk.count];
    t := tx_parse_vartime(s, true, ins[..], outs[..], items[..]);
    if i > 0 {
      for k: 0..<t.nin { coins[k] = TxcCoin.{found=true, value=SIGOPS_SPENT_514999[used+k].value, height=0}; }
      x := txcheck_inputs_vartime(s[0..<t.total], t, outs[..], coins[0..<t.nin], 514999);
      #assert x.code == TXC_OK;
      total = subsidy_add_fee_vartime(total, x.fee);
      used += t.nin;
    }
    at += t.total;
  }
  return total;
}
sg_first_end :: (b: [] u8) -> int {
  ins: [64] TxIn; outs: [64] TxOut; items: [64] TxItem;
  return 81 + tx_parse_vartime(b[81..<b.count], true, ins[..], outs[..], items[..]).total;
}
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/subsidy-coinbase.out](../fixtures/subsidy-coinbase.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/subsidy-coinbase.coax](../examples/checks/subsidy-coinbase.coax),
which loads the example:

- each halving halves the subsidy, and the block before it still pays
  the old one, for mainnet's interval, regtest's and 1,000 (Core's
  `TestBlockSubsidyHalvings`);
- the height script of every height below 70,000 reads back as that
  height, through chapter 36's minimal `CScriptNum` reader;
- each pinned coinbase passes with exactly the fees its claim needs and
  refuses with one satoshi less.

<!-- source: bitcoin/examples/checks/subsidy-coinbase.coax check -->
```jai
// Core's TestBlockSubsidyHalvings: at every halving the subsidy is half
// the one before, for mainnet's interval, regtest's 150 and 1,000.
#forall "each halving halves the subsidy" k: 1..<SUBSIDY_MAX_HALVINGS #steps 100000000 {
  intervals := [3] int.[CHAIN_MAINNET_PARAMS.halving, 150, 1000];
  for i: 0..<3 {
    h := intervals[i];
    #assert subsidy_vartime(k * h, h) == subsidy_vartime((k - 1) * h, h) / 2;
    #assert subsidy_vartime(k * h - 1, h) == subsidy_vartime((k - 1) * h, h);
  }
}
// The height a coinbase starts with reads back as that height: one
// opcode up to 16, otherwise a push whose bytes are a minimal CScriptNum.
#forall "the height script reads back minimally" height: 0..<70000 #steps 1000000000 {
  out: [10] u8;
  n := subsidy_height_script_vartime(height, out[..]);
  if height == 0 { #assert n == 1 && out[0] == OP_0; }
  else if height <= 16 { #assert n == 1 && out[0] == cast(u8) (OP_1 - 1 + height); }
  else {
    x := script_num_vartime(out[1..<n], true, 8);
    #assert cast(int) out[0] == n - 1 && x.ok && x.value == height;
  }
}
// Each pinned coinbase passes with exactly the fees its claim needs, and
// refuses with one satoshi less, whenever its claim is above the subsidy.
#forall "the claim is decided at subsidy plus fees" i: 0..<SUBSIDY_PROOFS.count #steps 1000000000 {
  m := CHAIN_MAINNET_PARAMS;
  c := SUBSIDY_PROOFS[i];
  raw: [512] u8; ins: [4] TxIn; outs: [8] TxOut; items: [8] TxItem;
  x := sb_read(c.coinbase, raw[..], ins[..], outs[..], items[..]);
  b := raw[0..<x.n];
  need := subsidy_claim_vartime(b, x.t, outs[..]) - subsidy_vartime(c.height, m.halving);
  #assert need >= 0 && need <= c.fees;
  #assert subsidy_amount_check_vartime(m, b, x.t, ins[..], outs[..], c.height, need) == SUBSIDY_OK;
  if need > 0 { #assert subsidy_amount_check_vartime(m, b, x.t, ins[..], outs[..], c.height, need - 1) == SUBSIDY_CB_AMOUNT; }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/subsidy-coinbase.coax
```

`coax test` runs 313 checks, including the loaded files' own checks, in about 125 seconds on 2026-11-03.

### The library's own checks

`subsidy.coax` is an `#authority` file. Its checks run Core's
`block_subsidy_test` and `subsidy_limit_test`, the subsidy at each
halving and at height 6,930,000, the height scripts at each encoding's
edge, the BIP-34 rule on every pinned coinbase before and after
activation and with a changed height, each claim with one satoshi more,
the fee total at `MAX_MONEY`, and a transaction that is not a coinbase.

```shell
coax/bin/coax test coax/lib/bitcoin/subsidy.coax
```

`coax test` runs 308 checks, including those of the files it loads, in about 125 seconds on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[subsidy.py](../../coax/tests/bitcoin/subsidy.py) reads `GetBlockSubsidy`,
the BIP-34 rule, the fee total, the coinbase rule and the BIP-30 blocks
from the pinned `validation.cpp`, `push_int64` from `script.h`, `COIN`
and `MAX_MONEY` from `amount.h`, the halving intervals and BIP-34 height
from `chainparams.cpp`, and the expected total from
`validation_tests.cpp`. It checks that `subsidy.coax`'s reasons and
constants are Core's, then runs transliterations over the pinned
coinbases and block 514,999, with chapter 34's reader and chapter 46's
Merkle root, and writes the transcript the example must print.
[make_subsidy_vectors.py](../../coax/tests/bitcoin/make_subsidy_vectors.py)
writes the coinbases from the pinned JSON and proves each in its header.

```shell
python3 -I coax/tests/bitcoin/subsidy.py --coax coax/bin/coax
```

On 2026-11-03 all 77 lines agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- One coinbase, first: chapter 53. Its scriptSig length and the maturity
  of its outputs: chapter 52.
- The witness commitment in the coinbase: chapter 49.
- The BIP-30 check itself, which needs the set of unspent outputs:
  chapters 55 and 58.
- How a miner builds a coinbase and a block template: chapter 88.

## Summary

- The subsidy is 50 bitcoin shifted right once per 210,000 blocks, and 0
  from the 64th halving. It is 0 from height 6,930,000.
- Every subsidy adds up to 20,999,999.9769 bitcoin.
- A coinbase may claim at most the subsidy plus the block's fees, and the
  fees' total stays within `MAX_MONEY`.
- From height 227,931 a coinbase's scriptSig starts with the block's
  height, as `CScript() << nHeight` writes it.
- Two coinbases of 2010 repeated earlier txids; Core exempts their blocks
  from BIP-30.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [50. Chain parameters: mainnet, testnet4, signet and regtest](ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `validation.cpp`
  (`GetBlockSubsidy`, `ContextualCheckBlock`, `ConnectBlock`,
  `IsBIP30Repeat`), `script/script.h` (`CScript::push_int64`),
  `consensus/amount.h`, `kernel/chainparams.cpp` and
  `test/validation_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/validation_tests.cpp).
- [BIP-30](https://github.com/bitcoin/bips/blob/master/bip-0030.mediawiki)
  (duplicate transactions),
  [BIP-34](https://github.com/bitcoin/bips/blob/master/bip-0034.mediawiki)
  (the height in the coinbase) and
  [BIP-42](https://github.com/bitcoin/bips/blob/master/bip-0042.mediawiki)
  (a finite monetary supply).
- The headers, coinbases and fees of 14 mainnet blocks, from
  mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/coinbase-proofs.json).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
