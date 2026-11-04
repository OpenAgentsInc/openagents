# 57. Validate a block: every consensus rule

You now have the individual rules. This chapter applies them to one block,
in order, against the coins of its parent. A transaction may spend an
earlier transaction in the same block. A second spend sees that coin as
spent. Every input executes its script with chapter 55's consensus flags.

The implementation is [validation.coax](../../coax/lib/bitcoin/validation.coax).
It follows `CheckBlock`, `ContextualCheckBlock` and `ConnectBlock` in
[pinned Bitcoin Core](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/validation.cpp).
It performs all script checks. There is no assumevalid shortcut here.

## Start from the parent's coins

`validation_block_vartime` takes an entry in chapter 59's admitted header
tree, the block bytes, and a private coin workspace. Its caller supplies the
exact UTXO view of that entry's parent. The tree comes from
`bi_accept_header_vartime`: it has already checked proof of work, the parent,
required difficulty, time and version. A body must hash to the admitted
header. Setting a status flag on an arbitrary header is not admission.

The workspace uses chapter 62's key-value table. A key is `C`, the 32 wire
txid bytes, and a four-byte little-endian output index. A value is an
eight-byte signed satoshi amount, a four-byte height, a coinbase byte and
the scriptPubKey. This is a memory format. Chapter 56's compressed Core disk
encoding remains a separate representation.

The result carries a category, its underlying rule, a transaction and input
location, accumulated fees, sigop cost and the resulting workspace length.
`validation_reason` gives the rejection family; a script failure also retains
the interpreter's exact error in `detail`. Header errors retain chapter
59's reasons. A capacity or context refusal is a local failure, not evidence
that a block violates consensus.

<!-- source: coax/lib/bitcoin/validation.coax types -->
```jai
VALIDATE_OK :: 0; VALIDATE_CONTEXT :: 1; VALIDATE_CAPACITY :: 2;
VALIDATE_BLOCK :: 3; VALIDATE_TX :: 4; VALIDATE_WITNESS :: 5;
VALIDATE_SUBSIDY :: 6; VALIDATE_BIP30 :: 7; VALIDATE_SCRIPT :: 8;
VALIDATE_SIGNET :: 9;
ValidationResult :: struct {
  code: int; detail: int; tx: int; input: int;
  fees: int; sigops: int; used: int;
}
ValidationWork :: struct {
  ins: [] TxIn; outs: [] TxOut; items: [] TxItem;
  ids: [] u8; wids: [] u8; merkle: [] u8;
  coins: [] TxcCoin; spent: [] ScriptSpent; sigops: [] SigopSpent;
  heights: [] int; times: [] int; scripts: [] u8;
  script: [] ScriptWork; batch: [] u8;
  signet_ids: [] u8; signet_scratch: [] u8;
}
// Coin workspace v1: key 'C' + wire txid + uint32 LE output index;
// value int64 LE satoshis + uint32 LE height + one coinbase byte + script.
// This is a memory format, not Core's compressed chainstate disk format.
VALIDATION_COIN_HEAD :: 13;
```

All views are live and disjoint. The function changes only private scratch.
After **any** refusal you discard that workspace. On success you may prepare
the chainstate commit. This function does not publish a tip, write an undo
file, change block-index validity or decide a reorganization. Chapters 58
and 60 own those transitions. The explorer's proof-of-work-only profile is
unchanged by this chapter.

## Follow one block

First decode the block. On signet, chapter 50 executes the challenge
before the transaction Merkle-root check, as Core does. Then run chapter
53's `block_check_vartime`: the transaction Merkle root, duplicate leaves,
block size, coinbase placement, every context-free transaction rule and
legacy sigops. The genesis block adds no coinbase output to the UTXO set.

The contextual stage then checks absolute finality, the BIP-34 coinbase
prefix, the witness commitment and weight. Its `BlockCheck` parameter is the
measurement the public entry point just calculated from the bytes. Keeping
that internal stage explicit lets its checks model the exact weight
boundary without hashing a multi-megabyte fixture at compile time.

Before spending anything, apply the BIP-30 lookup to **every** output in the
pre-block view. An early transaction spending an old coin cannot hide a
later overwrite. Chapter 55 supplies the exact historical exceptions, the
known BIP-34 ancestry optimization and its end at height 1,983,702.

For each non-coinbase transaction, look up every input. Check maturity,
input values and fees, then calculate BIP-68 sequence locks. A time-based
lock uses the median time past of the block before its coin's block.
Relative height and time locks both use the strict last-invalid boundary.

Count contextual P2SH and witness sigops before executing the scripts.
Every script sees a stable table: all spent script views remain live until
the transaction finishes. Taproot also receives every input's spent amount
and script, in input order. Only after the scripts pass do you remove the
inputs and insert spendable outputs. This makes a parent available to a
later transaction, while preventing a second spend of its output.

Finally check that the coinbase claims no more than subsidy plus fees.
`OP_RETURN` outputs and scripts larger than 10,000 bytes do not enter the
coin view. Empty scripts do enter it; being unspendable under execution is
different from Core's `IsUnspendable` storage rule.

## Work out the fees

Start with a non-coinbase coin worth 1,000 satoshis. A transaction spends it
and creates a 999-satoshi output. Its fee is `1000 - 999 = 1`. Its child in
the same block spends 999 and creates 998, adding `999 - 998 = 1`. The total
fee is two satoshis. At regtest height 1 the subsidy is 5,000,000,000
satoshis, so the coinbase ceiling is 5,000,000,002.

The [example](../examples/validate-block.coax) runs this bounded coin model,
then prints six distinct refusals and checks real ECDSA and two-input
Taproot spends. It uses the checked fixture builders in
[validation_checks.coax](../../coax/tests/bitcoin/validation_checks.coax) and
[validation_edges.coax](../../coax/tests/bitcoin/validation_edges.coax).
The expected output is [validate-block.out](../fixtures/validate-block.out).

```shell
coax/bin/coax run bitcoin/examples/validate-block.coax
coax/bin/coax run bitcoin/examples/validate-block.coax --release
```

For a broken case, the same parent cannot be spent twice. The second
lookup returns `bad-txns-inputs-missingorspent`. A false script returns
`block-script-verify-flag-failed` with its interpreter error. Neither
refusal authorizes publishing the partly changed scratch table.

## The consensus rule table

This table identifies every validation family and its checking chapter.
The crafted block names are cases in the independent native oracle; the
listed component suites cover the detailed variants within each family.

| Rule | Implementation and checking chapter | Counterexample or boundary |
| --- | --- | --- |
| Header decode, target sign/range and hash below target | `pow.coax`, [47](ch047-block-headers-and-proof-of-work.md) | `header-high-hash`; Core compact-target cases |
| Required difficulty, retarget, minimum-difficulty exceptions and chainwork | `retarget.coax`, [48](ch048-compact-targets-retargeting-and-chainwork.md); `blockindex.coax`, [59](ch059-the-block-index-and-the-most-work-chain.md) | `header-difficulty`; pinned mainnet retargets |
| Known parent, genesis identity and failed ancestors | `blockindex.coax`, [59](ch059-the-block-index-and-the-most-work-chain.md) | `prev-blk-not-found`, `bad-prevblk`, `duplicate-invalid`; genesis model |
| Median time past, future time and testnet4 timewarp | `header.coax`, [51](ch051-header-rules-time-versions-and-deployments.md) | `header-time-old`, `header-time-new`; BIP-94 counterexample |
| Buried version floors and deployment selection | `header.coax`, [51](ch051-header-rules-time-versions-and-deployments.md) | `header-bad-version`; every network's height boundaries |
| Canonical transaction and block framing | `tx.coax`, [34](ch034-transactions-and-their-serialization.md); `merkle.coax`, [46](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) | short reads, noncanonical CompactSize and trailing bytes |
| Transaction Merkle root and duplicate leaves | `merkle.coax`, [46](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) | `merkle-root`, `merkle-duplicate` |
| Nonempty block, stripped size and transaction count | `sigops.coax`, [53](ch053-sigops-weight-and-block-limits.md) | `stripped-size`; block-length component boundaries |
| Exactly one coinbase, first | `sigops.coax`, [53](ch053-sigops-weight-and-block-limits.md) | `no-coinbase`, `multiple-coinbase` |
| Inputs and outputs nonempty; transaction size | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `output-empty`; Core BADTX and size-bound checks |
| Signed output values, individual and total money bounds | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `output-negative`, `output-too-large`, `output-total` |
| No duplicate inputs or null non-coinbase prevouts | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `duplicate-input`, `null-prevout` |
| Coinbase script length 2 through 100 | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `coinbase-length-1`, `coinbase-length-101` |
| Absolute finality and BIP-113 cutoff | `locktime.coax`, [39](ch039-timelocks.md); `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `absolute-lock`; pre/post CSV cutoff checks |
| BIP-34 serialized coinbase height | `subsidy.coax`, [54](ch054-the-subsidy-and-the-coinbase.md) | `coinbase-height` |
| BIP-325 signet solution | `chainparams.coax`, [50](ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md) | pinned signet block 1; changed challenge refuses |
| Witness nonce, highest matching commitment, root and absent commitment | `witness.coax`, [49](ch049-the-witness-commitment.md) | `witness-nonce`, `witness-mismatch`, `witness-unexpected`; Core's 18 commitment cases |
| Maximum weight 4,000,000 | `sigops.coax`, [53](ch053-sigops-weight-and-block-limits.md) | `witness-weight`; internal stage at 4,000,000 and 4,000,001 |
| BIP-30 no overwrite, exact repeat exceptions and BIP-34 ancestry | `scriptflags.coax`, [55](ch055-script-flags-by-height-and-their-exceptions.md); this chapter | pre-block collision model; blocks 91,842 and 91,880 |
| Every input exists and is spent at most once | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md); this chapter | `missing`, `double-spend`; valid parent/child |
| Coinbase maturity at least 100 | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `immature`; 99/100 boundary |
| Input values and fees in range, inputs cover outputs | `txcheck.coax`, [52](ch052-transaction-checks-outside-script.md) | `overspend`; malformed-coin range models |
| Accumulated fees in range | `subsidy.coax`, [54](ch054-the-subsidy-and-the-coinbase.md); this chapter | two-coin model exceeds MAX_MONEY by one |
| BIP-68 relative height and time finality | `locktime.coax`, [39](ch039-timelocks.md) | `relative-lock`; same-block child and time-lock models |
| Legacy, P2SH and witness sigop cost at most 80,000 | `sigops.coax`, [53](ch053-sigops-weight-and-block-limits.md) | `legacy-sigops-20000`, `legacy-sigops-20001`; contextual P2SH boundary |
| Base script, P2SH and strict DER | `script.coax`, [36](ch036-the-script-interpreter.md), [37](ch037-legacy-signature-hashes.md) | `false-script`; Core's script and transaction corpus |
| CLTV, CSV and NULLDUMMY under the selected mask | `script.coax`, [36](ch036-the-script-interpreter.md), [39](ch039-timelocks.md), [55](ch055-script-flags-by-height-and-their-exceptions.md) | Core flag cases; pinned BIP-65, CSV and SegWit activations |
| Witness v0 execution and BIP-143 signatures | `script.coax`, [35](ch035-txids-wtxids-and-malleability.md), [38](ch038-segwit-v0-signature-hashes.md) | signed P2WPKH amount changed by one refuses |
| Taproot key/script path, control blocks, BIP-341 hashes and tapscript | `script.coax`, [41](ch041-the-key-tweak-and-the-key-path.md)–[45](ch045-control-blocks-and-script-path-spends.md) | real Schnorr and two-input spends; control, annex, parity, OP_SUCCESS and budget component cases |
| Subsidy, halving and coinbase value | `subsidy.coax`, [54](ch054-the-subsidy-and-the-coinbase.md) | `coinbase-overpay`; every halving and final zero subsidy |
| Genesis exclusion and output insertion | this chapter; [56](ch056-the-utxo-set-and-muhash3072.md) | empty genesis state; independent exact post-block coin comparisons |

The fee-range branch after individually valid input/output totals is
unreachable under those preconditions, as Core documents. The accumulated
fee overflow model deliberately supplies more than MAX_MONEY in predecessor
coins to check the defensive boundary. It does not claim that such a UTXO
set can arise on the valid main chain.

## Check against independent state

[validation.py](../../coax/tests/bitcoin/validation.py) mines and crafts an
isolated regtest prefix, then asks a running Bitcoin Core 32.1 for 35 block
verdicts. It runs with no peers or wallet. Core and the own header loader
agree on header failures; the block validator agrees on the body failures.
For script failures the oracle reconstructs Core's full reason from the
returned interpreter code and pinned `ScriptErrorString`, then compares it
verbatim. It also checks the exact version value in `bad-version`.
Accepted cases also agree with Python's independent exact coin-map update.
The [recorded verdicts](../fixtures/validation/core-regtest.json) include
each deterministic block's digest. Separate live testnet4 and signet nodes
check block 1 and a changed body with its original header. On signet the
changed body fails the challenge before the Merkle root; on testnet4 it
fails the Merkle root. Each node then accepts the original valid body,
showing that the refusal did not poison that header.

[validation-corpus.py](../../coax/tests/bitcoin/validation-corpus.py) records
historical raw blocks and Core's spent coins, checks every serialized txid
and the Merkle root independently, and pins the result. The
[fixture directory](../fixtures/validation/) contains mainnet heights
91,842, 91,880, 170,060, 173,805, 227,931, 363,725, 388,381, 419,328, 481,824,
692,261 and 709,632, plus testnet4 and signet block 1. These cover the buried
activations, Taproot activation and the historical hash exceptions.

Historical mainnet fixtures supply the independently admitted parent chain
and the exact coins spent by the block. Their sparse header context includes
every time and hash that can affect the decision. This is an oracle boundary,
not a node header loader or evidence of a sync from genesis. Testnet4 and
signet block 1 use the actual own header loader from genesis. Chapter 80
still owes full sync; chapters 93 and 118 still owe node-level parity.

The [verification record](../../docs/verification/bitcoin-ch57-2026-11-04.md)
names the compiler, profiles, mutation result, reference version and fixture
hashes. Qualification is native CPU. No Wasm result is claimed.

Next: [58. Connect and disconnect a block](ch058-connect-and-disconnect-a-block.md).
