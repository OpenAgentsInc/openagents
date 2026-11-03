# 55. Script flags by height and their exceptions

Chapter 36 runs a script with a flag mask. This chapter chooses that mask for
a block. You also select the BIP-30 duplicate-output check that chapter 58
will apply when it connects a block.

The code is [scriptflags.coax](../../coax/lib/bitcoin/scriptflags.coax).
It follows `GetBlockScriptFlags` and `ConnectBlock` in Bitcoin Core's
[`validation.cpp` at ddea9196](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/validation.cpp).
Chapter 50 supplies the network parameters, exception hashes and BIP-30
constants. Chapter 51 explains buried deployments. All inputs here are
public; the procedures run in variable time.

## Start with the mask Core uses

You might expect to turn P2SH on at its historical activation time and
Taproot on at height 709,632. That would reconstruct the deployment history,
but it would not implement the pinned Core function.

Core starts with P2SH, witness and Taproot enabled for every height. It trims
that base for a small set of historical block hashes. It then adds four
flags at the network's buried deployment heights:

| Rule | Bit value | Mainnet height |
| --- | ---: | ---: |
| P2SH | 1 | Base mask, with one hash exception |
| Witness | 2,048 | Base mask, with one hash exception |
| Taproot | 131,072 | Base mask, with two hash exceptions |
| Strict DER signatures, BIP-66 | 4 | 363,725 |
| `CHECKLOCKTIMEVERIFY`, BIP-65 | 512 | 388,381 |
| `CHECKSEQUENCEVERIFY`, BIP-112 | 1,024 | 419,328 |
| `NULLDUMMY`, BIP-147 | 16 | 481,824 |

The deployment lookup uses the height of the block you are checking. It is
not the height of its parent. `NULLDUMMY` activates with segwit, even though
the witness bit is already in the base mask.

### Work out a mask

The base is `1 + 2048 + 131072 = 133121`. Mainnet block 363,724 adds nothing.
Block 363,725 adds strict DER, giving `133121 + 4 = 133125`. At height
481,824 all four buried additions apply:

```text
133121 + 4 + 512 + 1024 + 16 = 134677
```

Policy flags such as `LOW_S`, `STRICTENC` and `CLEANSTACK` are absent. Chapter
81's standardness rules are a separate decision.

<!-- source: coax/lib/bitcoin/scriptflags.coax flags -->
```jai
SCRIPTFLAGS_BASE :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_TAPROOT;
// Returns a mask, or -1 for an invalid network, height or hash extent.
// Hash bytes are SHA256d/wire order, the reverse of an explorer's text.
scriptflags_vartime :: (p: ChainParams, height: int, hash: [] u8) -> int #export_c {
  if !scriptflags_context_vartime(p, height, hash) { return -1; }
  flags := SCRIPTFLAGS_BASE;
  exception := chain_script_exception_vartime(p, hash);
  if exception >= 0 { flags &= exception; }
  if chain_deployment_active(p, DEP_DERSIG, height) { flags |= SCRIPT_VERIFY_DERSIG; }
  if chain_deployment_active(p, DEP_CLTV, height) { flags |= SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY; }
  if chain_deployment_active(p, DEP_CSV, height) { flags |= SCRIPT_VERIFY_CHECKSEQUENCEVERIFY; }
  if chain_deployment_active(p, DEP_SEGWIT, height) { flags |= SCRIPT_VERIFY_NULLDUMMY; }
  return flags;
}
```

## Exceptions identify a block by its hash

The mainnet exceptions are:

| Block | Displayed hash | Base after trimming |
| --- | --- | ---: |
| 170,060, BIP-16 | `00000000000002dc756eebf4f49723ed8d30cc28a5f108eb94b1ba88ac4f9c22` | 0 |
| 692,261, Taproot | `0000000000000000000f14c35b2d841e986ab5441de8c585d5ffe55ea1e395ad` | 2,049 |

Testnet3 has its own BIP-16 exception in chapter 50's parameters. Testnet4,
default signet and regtest have no script flag exceptions in the pinned
parameters.

The map lookup uses the full hash, with no height-based exemption. A fork's
block at height 170,060 does not inherit the mainnet block's exemption.
Changing any byte of the hash loses it. The caller supplies the digest in
wire order; reverse the displayed hex bytes once before calling.

Trimming happens **before** the buried additions. At mainnet block 692,261,
the trimmed base is `1 + 2048 = 2049`. Add strict DER, CLTV, CSV and
NULLDUMMY, giving `2049 + 1556 = 3605`. Returning 2,049 immediately from the
exception lookup would omit rules that Core still enforces.

The tests also pair exception hashes with synthetic heights. Such pairs
are not claimed to be real blocks; they check the order of the mask
operations independently of the historical examples.

## Decide whether BIP-30 needs an output lookup

[BIP-30](https://github.com/bitcoin/bips/blob/master/bip-0030.mediawiki)
prevents a transaction from overwriting unspent outputs of an earlier
transaction with the same txid. A txid match is allowed when the earlier
outputs are already completely spent. Two historical mainnet blocks are
explicit exceptions:

| Repeating block | Earlier coinbase's block |
| ---: | ---: |
| 91,842 | 91,812 |
| 91,880 | 91,722 |

Chapter 54 proves these coinbases in pinned headers and shows their equal
txids. The connect exceptions require both the repeating block's exact
height and its exact hash. Chapter 50 also names the earlier blocks for
disconnect handling; they are different pairs of height and hash.

Core can skip the duplicate-output lookups after BIP-34 if the previous
block's ancestor at `BIP34Height` has the network's recorded `BIP34Hash`.
For mainnet that anchor is block 227,931. Merely reaching that height on
another branch does not establish the optimization.

Work through the boundary. While checking block 227,931, its parent is
227,930 and has no ancestor at 227,931: keep BIP-30 checking. While checking
227,932, its parent can supply the known 227,931 anchor: skip the lookup.
An absent or different ancestor keeps checking.

BIP-34's height prefix does not prevent every possible future repeat. Early
coinbases sometimes started with bytes that encode a much later height.
Pinned Core resumes BIP-30 checking at height 1,983,702, even on the known
chain. Thus the last optimized height is 1,983,701. Networks whose pinned
`BIP34Hash` is null cannot use this known-anchor optimization.

<!-- source: coax/lib/bitcoin/scriptflags.coax bip30 -->
```jai
// Whether ConnectBlock must check for unspent outputs with a repeated txid:
// 1 checks, 0 skips, -1 refuses malformed context. ancestor is the previous
// block's ancestor at p.bip34_height, or empty when none is supplied. Missing
// ancestry keeps checking. The caller proves that a supplied hash is really
// that ancestor; a height alone is never proof of the known chain.
scriptflags_bip30_vartime :: (p: ChainParams, height: int, hash: [] u8, ancestor: [] u8) -> int #export_c {
  if !scriptflags_context_vartime(p, height, hash) { return -1; }
  if ancestor.count != 0 && ancestor.count != 32 { return -1; }
  if height >= CHAIN_BIP34_IMPLIES_BIP30_LIMIT { return 1; }
  if chain_bip30_repeat_vartime(height, hash) { return 0; }
  if height > p.bip34_height && chain_hash_is_vartime(ancestor, p.bip34_hash) { return 0; }
  return 1;
}
```

This function selects the lookup. Chapter 58 still has to inspect the
UTXO set and return Core's `bad-txns-BIP30` rejection when an overwrite is
forbidden. It also owns the genesis special case: Core does not add the
genesis coinbase to the spendable UTXO set.

## Run the example

The [example](../examples/script-flags.coax) prints the worked boundaries
and the historical exemptions:

```shell
coax/bin/coax run bitcoin/examples/script-flags.coax
```

```text
Mainnet GetBlockScriptFlags (decimal masks):
  0: 133121
  363724: 133121
  363725: 133125
  388380: 133125
  388381: 133637
  419327: 133637
  419328: 134661
  481823: 134661
  481824: 134677
  BIP16 exception at 170060: 0
  its hash changed: 133121
  Taproot exception at 692261: 3605
BIP30 (1 checks, 0 skips):
  repeat 91842: 0
  its hash changed: 1
  repeat 91880: 0
  its hash changed: 1
  known BIP34 ancestor, height 227931: 1
  known BIP34 ancestor, height 227932: 0
  known BIP34 ancestor, height 1983701: 0
  known BIP34 ancestor, height 1983702: 1
  missing ancestor at 227932: 1
```

### Deliberately break the context

The [refused example](../examples/script-flags-refused.coax) supplies a
31-byte block hash:

```shell
coax/bin/coax run bitcoin/examples/script-flags-refused.coax
```

```text
script flags: refused 31-byte block hash (code -1)
```

Its exit status is 0: the example asserts the expected refusal code. The
two examples' stdout and exit statuses are checked
in debug and release. An invalid network, a negative height, a height above
`INT_MAX`, or an ancestor extent other than zero or 32 also returns `-1`.
Never pass a negative result to the script interpreter as a flag mask.

## Check it independently

```shell
coax/bin/coax test coax/lib/bitcoin/scriptflags.coax --library
coax/bin/coax mutate coax/lib/bitcoin/scriptflags.coax --library --root-only --jobs 4
python3 -I coax/tests/bitcoin/scriptflags.py --coax coax/bin/coax
python3 psionic/tests/book.py --book bitcoin
```

The Coax checks cover every buried boundary on all five networks, both
mainnet script exceptions, testnet3's exception, every changed byte of the
mainnet exceptions, both BIP-30 repeat blocks, the ancestor boundary and
the height where BIP-30 checking resumes. Mutation checks cover this new
module; loaded prerequisites retain their own qualification scope.

The [Python oracle](../../coax/tests/bitcoin/scriptflags.py) verifies the
manifest hashes of Core's source files before reading their rules. It derives
flag bits from Core's enum, network heights and exemptions from its parameter
classes, and BIP-30's conditions from `validation.cpp`. It compiles the
[Coax adapter](../../coax/tests/bitcoin/scriptflags-native.coax) to a native
library in each profile, then compares its answers with Python's independent
decisions. It also verifies the hashes and links of every pinned mainnet
header from genesis through 2,016 and compares the selector at each height.
No upstream program is run.

The [verification record](../../docs/verification/bitcoin-ch55-2026-11-03.md)
records the compiler, native comparison counts, mutation scope and repository
gate results.

The qualification here is native CPU rule selection. The parameters and
ancestry must come from a validated block index. A 32-byte input is an extent
check, not evidence of a valid header or chain. Full block validation,
UTXO updates and reorganization remain chapters 57, 58 and 60. The node
continues to require Core's verdict for money decisions under the book's
[full-node plan](PLAN.md#the-full-node).

Next: [56. The UTXO set and MuHash3072](ch056-the-utxo-set-and-muhash3072.md).
