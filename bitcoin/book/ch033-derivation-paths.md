# 33. Derivation paths

Chapter 32 derives a key from a seed and a list of indexes. A wallet needs
to know which list. A *derivation path* such as `m/84'/0'/0'/0/5` writes
that list as text, and BIP-43 and BIP-44 give each level of it a meaning:
the purpose, the coin, the account, receiving or change, and the address
number. The purpose also says which kind of address the keys make, so two
wallets restored from the same words find the same coins. This chapter
reads and writes path strings with every refusal they need, builds the
standard paths of BIP-44, BIP-49, BIP-84 and BIP-86, makes their addresses
with chapter 29's code, and explains the gap limit that a restored wallet
uses to find its addresses. It checks the result against the BIP-84,
BIP-86 and BIP-49 vectors, BIP-44's examples, NIP-06's key and a Python
implementation.

## What this chapter teaches

- Path strings: `m`, then steps separated by `/`, each a number below
  2^31 with an optional `h` or `'` that marks it hardened.
- The refusals a path reader needs: no `m`, an empty step, a step that
  isn't a number, a number of 2^31 or more and more than 255 steps.
- BIP-43's purpose field and the BIP-44 levels: purpose, coin type,
  account, change and address index.
- Which output each purpose gives: 44 is P2PKH, 49 is P2SH-P2WPKH, 84 is
  P2WPKH and 86 is key-path P2TR.
- Coin type 0 for mainnet and 1 for every test network.
- Why the first three levels are hardened, and why an account `xpub` is
  enough for a watch-only wallet.
- The SLIP-132 versions (`ypub`, `zpub`, `upub`) that some wallets use to
  say which purpose an extended key belongs to.
- The gap limit and account discovery.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [path.coax](../../coax/lib/bitcoin/path.coax) | `path_step_vartime(step)` | one step, `5`, `84'` or `84h`, as a BIP-32 index |
| | `path_parse_vartime(text, indexes)` | a path string to its list of indexes |
| | `path_format_vartime(indexes, marker, out)` | a list of indexes back to a path string, with `h` or `'` |
| | `path_purpose_kind(purpose)` | the output type a purpose gives, as chapter 29's `AD_*` kind |
| | `path_standard_vartime(purpose, network, account, change, index, indexes)` | the five indexes of a standard path |
| | `path_script_vartime(purpose, key, out)` | the scriptPubKey a purpose gives a compressed public key |
| | `path_derive_ct(seed, testnet, text, work, node)` | the node at a path string below a seed |
| | `path_account_script_ct(purpose, account, change, index, work, out)` | a scriptPubKey from an account node, private or public |

The file builds on earlier chapters: `bip32_master_ct`,
`bip32_descend_ct` and `bip32_neuter_ct` from
[chapter 32](ch032-hierarchical-deterministic-keys.md), and
`p2pkh_script_vartime`, `p2sh_p2wpkh_script_vartime`,
`p2wpkh_script_vartime` and `p2tr_script_vartime` from
[chapter 29](ch029-addresses.md). The seed comes from
[chapter 31](ch031-seed-phrases.md).

A path is public, so the path procedures branch and index on it; the
`_vartime` suffix says so. The seed and private nodes go only through
chapter 32's constant-time procedures, which leave their work buffer as
zeros. Each procedure returns a count or one negative `DP_*` refusal, and
a refusal writes nothing:

<!-- source: coax/lib/bitcoin/path.coax codes -->
```jai
DP_MAX_DEPTH :: 255;          // a node's depth is one byte, so a path has at most 255 steps
DP_ACCOUNT_DEPTH :: 3;        // m / purpose' / coin_type' / account'

// Refusals.
DP_NO_ROOT :: -1;             // a path that isn't "m" or doesn't start with "m/"
DP_EMPTY_STEP :: -2;          // an empty step: "m/", "m//0" or "m/0/"
DP_BAD_STEP :: -3;            // a step that isn't digits and an optional h or ': "0H", "+1", "1a"
DP_OVERFLOW :: -4;            // a step of 2^31 or more before its marker, or an index past 2^32 - 1
DP_TOO_DEEP :: -5;            // more than 255 steps
DP_CAPACITY :: -6;            // an output buffer that is too small
DP_BAD_PURPOSE :: -7;         // a purpose other than 44, 49, 84 or 86
DP_BAD_FIELD :: -8;           // an account or address index of 2^31 or more, or a change other than 0 or 1
DP_BAD_NETWORK :: -9;         // a network outside 1-3
DP_NOT_ACCOUNT :: -10;        // a node that isn't at depth 3 under a hardened index
// Refusals from the chapters underneath keep their codes, offset:
DP_BIP32 :: -20;              // plus chapter 32's BIP32_* code: -21 to -30
DP_ADDRESS :: -40;            // plus chapter 29's AD_* code: -41 to -48
```

## Path strings

BIP-32 writes a path as `m` followed by the indexes of each step. Index
`2^31` and above is hardened, and a path writes it as the number below
`2^31` followed by a mark. BIP-32 uses `H` in its text, wallets and BIP-44
use `'`, and descriptors (chapter 30) added `h` because `'` needs quoting
in a shell. Bitcoin Core reads `'` and `h`, and so does this code; it
refuses `H`, as BIP-380 does.

<!-- source: coax/lib/bitcoin/path.coax step -->
```jai
// One step: decimal digits, then h or ' for a hardened step. The number
// must be below 2^31 either way; a hardened step adds 2^31 to it. Returns
// the BIP-32 index or a refusal. Leading zeros are allowed, as Bitcoin
// Core allows them.
path_step_vartime :: (step: string) -> int #export_c {
  if step.count == 0 { return DP_EMPTY_STEP; }
  n := step.count;
  last := step[n - 1];
  hardened := last == 'h' || last == '\'';
  if hardened { n -= 1; }
  if n == 0 { return DP_BAD_STEP; }
  for i: 0..<n { if step[i] < '0' || step[i] > '9' { return DP_BAD_STEP; } }
  value := 0;
  for i: 0..<n {
    value = value * 10 + cast(int) (step[i] - '0');
    if value >= BIP32_HARDENED { return DP_OVERFLOW; }   // stops before a long run of digits can overflow
  }
  return ifx hardened then value + BIP32_HARDENED else value;
}
```

The number before the mark must be below `2^31`, because the mark adds
`2^31`: `m/2147483648` and `m/2147483648'` both refuse with
`DP_OVERFLOW`. The reader checks that every character is a digit before it
computes the value, and it stops as soon as the value reaches `2^31`, so a
long run of digits can't overflow a 64-bit integer. Leading zeros are
allowed, because Bitcoin Core allows them; `m/007` is `m/7`.

<!-- source: coax/lib/bitcoin/path.coax parse -->
```jai
// A path string, "m" then zero to 255 steps each after a "/", into
// indexes. Returns the number of steps or a refusal, and writes indexes
// only on success.
path_parse_vartime :: (text: string, indexes: [] int) -> int #export_c {
  if text.count == 0 || text[0] != 'm' { return DP_NO_ROOT; }
  if text.count == 1 { return 0; }
  if text[1] != '/' { return DP_NO_ROOT; }
  found: [DP_MAX_DEPTH] int;
  count := 0; start := 2;
  for i: 2..<text.count + 1 {
    if i == text.count || text[i] == '/' {
      index := path_step_vartime(text[start..<i]);
      if index < 0 { return index; }
      if count == DP_MAX_DEPTH { return DP_TOO_DEEP; }
      found[count] = index; count += 1; start = i + 1;
    }
  }
  if indexes.count < count { return DP_CAPACITY; }
  for k: 0..<count { indexes[k] = found[k]; }
  return count;
}
```

A node's depth is one byte, so no node is more than 255 steps below the
master. A path of 256 steps refuses with `DP_TOO_DEEP` before any
derivation runs. A refusal leaves the caller's index buffer as it was.

Writing a path back is the reverse. The caller chooses the mark:

<!-- source: coax/lib/bitcoin/path.coax format -->
```jai
// A list of indexes as a path string, with marker ('h' or '\'') after each
// hardened step. Returns the text's length or a refusal.
path_format_vartime :: (indexes: [] int, marker: u8, out: [] u8) -> int #export_c {
  if marker != 'h' && marker != '\'' { return DP_BAD_STEP; }
  if indexes.count > DP_MAX_DEPTH { return DP_TOO_DEEP; }
  text: [DP_MAX_DEPTH * 12 + 1] u8;   // "/", 10 digits and a marker per step
  text[0] = 'm'; n := 1;
  for index: indexes {
    if index < 0 || index > 4294967295 { return DP_OVERFLOW; }
    value := ifx index >= BIP32_HARDENED then index - BIP32_HARDENED else index;
    digits: [10] u8; d := 0;
    while true {
      digits[d] = cast(u8) ('0' + value % 10); d += 1; value /= 10;
      if value == 0 { break; }
    }
    text[n] = '/'; n += 1;
    while d > 0 { d -= 1; text[n] = digits[d]; n += 1; }
    if index >= BIP32_HARDENED { text[n] = marker; n += 1; }
  }
  if out.count < n { return DP_CAPACITY; }
  for k: 0..<n { out[k] = text[k]; }
  return n;
}
```

### Worked example: `m/84'/0'/0'/0/5`

| Step | Text | Number | Hardened | Index |
| --- | --- | --- | --- | --- |
| purpose | `84'` | 84 | yes | 2,147,483,732 |
| coin type | `0'` | 0 | yes | 2,147,483,648 |
| account | `0'` | 0 | yes | 2,147,483,648 |
| change | `0` | 0 | no | 0 |
| address index | `5` | 5 | no | 5 |

`m/84h/0h/0h/0/5` gives the same five indexes, and
`path_format_vartime` writes them back with either mark.

## The standard paths

BIP-43 reserves the first level below the master for a *purpose*: the
number of the BIP that defines what the levels below it mean. BIP-44
defines five levels:

```text
m / purpose' / coin_type' / account' / change / address_index
```

- **purpose**: 44, 49, 84 or 86, the BIP that sets the address type.
- **coin type**: 0 for Bitcoin and 1 for every test network (testnet,
  testnet4, signet and regtest), from the SLIP-44 registry. A key made for
  testnet never pays a mainnet address by mistake.
- **account**: 0, 1, 2 and so on. Each account is a separate set of
  addresses that the user can name, such as savings or a shop.
- **change**: 0 for the addresses the wallet gives out to receive, the
  *external* chain, and 1 for the change it pays to itself, the *internal*
  chain.
- **address index**: 0, 1, 2 and so on within the chain.

The first three levels are hardened, and the last two are normal. Chapter
32 showed why: an `xpub` and one normal child's private key give the
parent's private key. Hardening the account stops that leak at the
account: a leaked address key and the account's `xpub` expose at most that
account, never the other accounts or the master. The last two levels are
normal so that the account `xpub` can make every address of the account
without a private key.

Each purpose names the output type its keys pay:

| Purpose | BIP | Output | Mainnet address | SLIP-132 versions |
| --- | --- | --- | --- | --- |
| 44 | BIP-44 | P2PKH | `1…` | `xprv`, `xpub` |
| 49 | BIP-49 | P2SH-P2WPKH | `3…` | `yprv`, `ypub` (testnet `uprv`, `upub`) |
| 84 | BIP-84 | P2WPKH | `bc1q…` | `zprv`, `zpub` (testnet `vprv`, `vpub`) |
| 86 | BIP-86 | P2TR, key path only | `bc1p…` | `xprv`, `xpub` |

<!-- source: coax/lib/bitcoin/path.coax standard -->
```jai
// The output type a BIP-43 purpose gives: BIP-44 is P2PKH, BIP-49 is
// P2SH-P2WPKH (a P2SH output), BIP-84 is P2WPKH and BIP-86 is key-path
// P2TR. Returns chapter 29's AD_* kind or DP_BAD_PURPOSE.
path_purpose_kind :: (purpose: int) -> int #export_c {
  if purpose == 44 { return AD_P2PKH; }
  if purpose == 49 { return AD_P2SH; }
  if purpose == 84 { return AD_P2WPKH; }
  if purpose == 86 { return AD_P2TR; }
  return DP_BAD_PURPOSE;
}
// m / purpose' / coin_type' / account' / change / address_index into
// indexes[0..<5]. The coin type is 0 on mainnet and 1 on every test
// network (SLIP-44). Change is 0 for receiving and 1 for change.
path_standard_vartime :: (purpose: int, network: int, account: int, change: int, index: int, indexes: [] int) -> int #export_c {
  if path_purpose_kind(purpose) < 0 { return DP_BAD_PURPOSE; }
  if network < SW_MAINNET || network > SW_REGTEST { return DP_BAD_NETWORK; }
  if account < 0 || account >= BIP32_HARDENED || change < 0 || change > 1 { return DP_BAD_FIELD; }
  if index < 0 || index >= BIP32_HARDENED { return DP_BAD_FIELD; }
  if indexes.count < 5 { return DP_CAPACITY; }
  indexes[0] = purpose + BIP32_HARDENED;
  indexes[1] = (ifx network == SW_MAINNET then 0 else 1) + BIP32_HARDENED;
  indexes[2] = account + BIP32_HARDENED;
  indexes[3] = change;
  indexes[4] = index;
  return 5;
}
```

The scripts come from chapter 29's builders. BIP-86 takes the 33-byte key
and uses its x coordinate as the Taproot internal key; the parity byte is
dropped, as BIP-340 keys do.

<!-- source: coax/lib/bitcoin/path.coax script -->
```jai
// The scriptPubKey a purpose gives a 33-byte compressed public key. BIP-86
// uses the key's x coordinate as the Taproot internal key.
path_script_vartime :: (purpose: int, key: [] u8, out: [] u8) -> int #export_c {
  kind := path_purpose_kind(purpose);
  if kind < 0 { return kind; }
  if key.count != SE_COMPRESSED { return DP_ADDRESS + AD_BAD_KEY; }
  n: int;
  if kind == AD_P2PKH { n = p2pkh_script_vartime(key, out); }
  else if kind == AD_P2SH { n = p2sh_p2wpkh_script_vartime(key, out); }
  else if kind == AD_P2WPKH { n = p2wpkh_script_vartime(key, out); }
  else { n = p2tr_script_vartime(key[1..<SE_COMPRESSED], out); }
  return ifx n < 0 then DP_ADDRESS + n else n;
}
```

### SLIP-132 versions

BIP-49 and BIP-84 publish their account keys as `uprv`/`upub` and
`zprv`/`zpub`, not `xprv`/`xpub`. These are SLIP-132 versions: the same 78
bytes as chapter 32's node with other version bytes, which tell a wallet
which purpose the key is for. The example decodes BIP-84's `zpub` with
chapter 26's Base58Check and finds version `04b24746`, where an `xpub` has
`0488b21e`; the other 74 bytes match the `xpub` that this code derives at
`m/84'/0'/0'`.

Chapter 32's decoder reads only the four BIP-32 versions, and this code
writes only them. A version that names an address type works for one
purpose and fails for every new one: Taproot has no SLIP-132 version.
Output descriptors (chapter 30) replaced the idea. A descriptor such as
`wpkh([73c5da0a/84h/0h/0h]xpub…/0/*)` names the script type, the key's
origin and the path in one string, with a plain `xpub`.

## From a seed to an address

`path_derive_ct` reads a path string, derives the master from a seed and
follows the indexes with chapter 32's code. `path_account_script_ct` takes
an account node and makes the scriptPubKey of one change and index below
it. Both steps below an account are normal, so the account node may be
private or public:

<!-- source: coax/lib/bitcoin/path.coax derive -->
```jai
// The node at a path string below a seed's master, into node: an xprv, or
// a tprv for the test networks. Returns 78 or a refusal, and writes node
// only on success. work (BIP32_WORK_BYTES) comes back as zeros.
path_derive_ct :: (seed: [] u8, testnet: bool, text: string, work: [] u8, node: [] u8) -> int #export_c {
  indexes: [DP_MAX_DEPTH] int;
  n := path_parse_vartime(text, indexes[..]);
  if n < 0 { return n; }
  if node.count < BIP32_NODE_BYTES { return DP_CAPACITY; }
  here: [BIP32_NODE_BYTES] u8;
  r := bip32_master_ct(seed, testnet, work, here[..]);
  if r > 0 { r = bip32_descend_ct(here[..], indexes[0..<n], work, here[..]); }
  if r > 0 { for j: 0..<BIP32_NODE_BYTES { node[j] = here[j]; } }
  for j: 0..<BIP32_NODE_BYTES { here[j] = 0; }
  return ifx r < 0 then DP_BIP32 + r else r;
}
// The scriptPubKey of change / index below an account node, for a purpose.
// The account node may be private or public: a watch-only wallet holds
// only the account's xpub and makes the same scripts. Both steps are
// normal, so CKDpub can take them. work comes back as zeros.
path_account_script_ct :: (purpose: int, account: [] u8, change: int, index: int, work: [] u8, out: [] u8) -> int #export_c {
  if path_purpose_kind(purpose) < 0 { return DP_BAD_PURPOSE; }
  if change < 0 || change > 1 || index < 0 || index >= BIP32_HARDENED { return DP_BAD_FIELD; }
  if account.count != BIP32_NODE_BYTES { return DP_BIP32 + BIP32_BAD_SIZE; }
  depth := cast(int) account[HDK_DEPTH];
  if depth != DP_ACCOUNT_DEPTH || account[HDK_INDEX] < 0x80 { return DP_NOT_ACCOUNT; }
  node: [BIP32_NODE_BYTES] u8; steps := [2] int.[change, index];
  r := bip32_descend_ct(account, steps[..], work, node[..]);
  if r > 0 { r = bip32_neuter_ct(node[..], work, node[..]); }
  n := ifx r < 0 then DP_BIP32 + r else path_script_vartime(purpose, node[HDK_KEY..<BIP32_NODE_BYTES], out);
  for j: 0..<BIP32_NODE_BYTES { node[j] = 0; }
  return n;
}
```

`path_account_script_ct` refuses a node that isn't an account: one that
isn't at depth 3, or that sits under a normal index. That catches a common
mistake, deriving `0/5` below the master or below an address node.

### Worked example: BIP-84's first address

BIP-84's vectors use the twelve words `abandon abandon … about`, whose
BIP-39 seed (chapter 31) starts `5eb00bbd`.

1. The path is `m/84'/0'/0'/0/0`: indexes `2147483732`, `2147483648`,
   `2147483648`, `0`, `0`.
2. The account node `m/84'/0'/0'` is BIP-84's `zprv` with the `xprv`
   version, and its public node is the `zpub` with the `xpub` version.
3. The address node's public key is
   `0330d54fd0dd420a6e5f8d3624f5f3482cae350f79d5f0753bf5beef9c2d91af3c`.
4. Purpose 84 gives P2WPKH: `0014` and HASH160 of the key,
   `c0cebcd6c3d3ca8c75dc5ec62ebe55330ef910e2`.
5. The address is `bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu`.

The same seed gives `1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA` at
`m/44'/0'/0'/0/0`, `37VucYSaXLCAsxYyAPfbSi9eh4iEcbShgf` at
`m/49'/0'/0'/0/0` and BIP-86's
`bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr` at
`m/86'/0'/0'/0/0`. One set of words is four different wallets, and a
wallet that restores the words must look under each purpose it supports.

### Testnet: coin type 1

BIP-49's vector is for testnet. `path_standard_vartime(49, SW_TESTNET, 0,
0, 0, …)` gives `m/49'/1'/0'/0/0`, and the master is a `tprv` because the
seed is for a test network. The private key there is BIP-49's
`c9bdb49c…`, its public key is chapter 29's BIP-49 key `03a1af80…`, and
P2SH-P2WPKH with the test version gives
`2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2`.

## Watch-only wallets

A shop's server needs a new address for each order but should never hold a
private key. It holds the account `xpub` only. For BIP-86's account,
`xpub6BgBgsespWvE…`, `path_account_script_ct` with change 0 and index 0
gives `bc1p5cyx…kedrcr`, the same address the seed gives. Index `2^31`
refuses with `DP_BAD_FIELD`: a hardened step needs the private key, and a
standard path has none below the account.

## The gap limit and account discovery

A wallet restored from its words knows the seed but not how many addresses
it gave out. BIP-44 answers with a scan:

1. Derive the account's external chain, index 0, 1, 2 and so on.
2. Look up each address's history on the chain, using a full node's
   index or a block filter (chapter 74).
3. Stop after 20 unused addresses in a row. This number is the *gap
   limit*. The highest used index found sets where new addresses start.
4. Scan the internal chain the same way.
5. If the account has any history, do the same for the next account;
   stop at the first account with none.

The gap limit is also a rule for the wallet that gives addresses out: it
must not give out more than 20 unused addresses in a row, or a restored
copy will not find the payments past them. The example's `scan` shows
both sides. With used indexes 0, 1, 2, 5 and 24, a gap of 20 finds index
24, because the run of unused addresses from 6 to 23 is 18 long. With 26
in place of 24, the run from 6 to 25 is 20 long, the scan stops, and the
payment at 26 stays hidden until the user raises the gap to 25. Wallets
that hand out many addresses, such as a shop's, raise their gap limit;
descriptor wallets (chapter 99) keep the limit per descriptor.

## NIP-06

The Nostr book's chapter 46 takes a Nostr key from BIP-39 words at
`m/44'/1237'/0'/0/0`: purpose 44 with SLIP-44's coin type 1237 for Nostr.
Its code, [nip06.coax](../../coax/lib/nostr/nip06.coax), keeps the path
as five integers. `path_parse_vartime` reads the string to the same five,
and the seed of NIP-06's first phrase gives its private key
`7f7ff03d…ccba9a` there. Nostr uses only the key, not an address.

## Refusals

| Input | Refusal |
| --- | --- |
| `84'/0'`, `M/0`, `m0` | `DP_NO_ROOT` |
| `m/`, `m//0`, `m/0/` | `DP_EMPTY_STEP` |
| `m/0H`, `m/+1`, `m/1a`, `m/-0`, `m/0''` | `DP_BAD_STEP` |
| `m/2147483648`, `m/2147483648'` | `DP_OVERFLOW` |
| 256 steps | `DP_TOO_DEEP` |
| purpose 48 | `DP_BAD_PURPOSE` |
| account or index `2^31`, change 2 | `DP_BAD_FIELD` |
| network 4 | `DP_BAD_NETWORK` |
| a depth-2 node as an account | `DP_NOT_ACCOUNT` |

Refusals from chapter 32 and chapter 29 keep their own codes, offset so
that no two collide: a seed of 15 bytes is `DP_BIP32 + BIP32_BAD_SIZE`,
-21, and a 21-byte buffer for a P2WPKH script is
`DP_ADDRESS + AD_CAPACITY`, -48.

## The example

[bitcoin/examples/derivation-paths.coax](../examples/derivation-paths.coax)
parses paths and writes them back, makes the first address of account 0
for each purpose from the `abandon … about` seed, derives BIP-86's
addresses from the account `xpub`, compares BIP-84's `zpub` with the
`xpub`, works BIP-49's testnet address, derives NIP-06's key, runs the gap
scan and prints a refusal of each kind.

<!-- source: bitcoin/examples/derivation-paths.coax example -->
```jai
#load "../../coax/lib/bitcoin/path.coax";
#load "../../coax/lib/crypto/bip39.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
REFUSALS :: [10] string.["no root", "empty step", "bad step", "overflow", "too deep", "capacity",
  "bad purpose", "bad field", "bad network", "not an account"];
refusal :: (code: int) -> string { return ifx code < 0 && code >= -10 then REFUSALS[-code - 1] else "?"; }
KINDS :: [6] string.["", "P2PKH", "P2SH-P2WPKH", "P2WPKH", "P2WSH", "P2TR"];
// A path's indexes, and the path written back with h.
show_path :: (text: string) {
  indexes: [DP_MAX_DEPTH] int; back: [64] u8;
  n := path_parse_vartime(text, indexes[..]);
  if n < 0 { print("% -> %\n", text, refusal(n)); return; }
  print("% ->", text);
  if n == 0 { print(" (no steps)"); }
  for k: 0..<n { print(" %", indexes[k]); }
  m := path_format_vartime(indexes[0..<n], 'h', back[..]);
  print(" -> %\n", cast(string) back[0..<m]);
}
// The address of a scriptPubKey on a network.
address :: (network: int, script: [] u8, out: [] u8) -> string {
  n := address_encode_vartime(network, script, out);
  #assert n > 0;
  return cast(string) out[0..<n];
}
// A node as its xprv, xpub, tprv or tpub.
text_of :: (node: [] u8, out: [] u8) -> string {
  work: [BIP32_WORK_BYTES] u8;
  #assert bip32_encode_ct(node, work[..], out) == BIP32_TEXT;
  return cast(string) out[0..<BIP32_TEXT];
}
// The seed of a phrase with no passphrase (chapter 31).
seed_of :: (phrase: string, out: [] u8) -> [] u8 {
  work: [BIP39_WORK_BYTES] u8;
  #assert bip39_seed_ct(phrase, "", work[..], out) == 64;
  return out[0..<64];
}
// Account discovery on one chain: scan from index 0 and stop after gap
// unused addresses in a row. Returns the highest used index found, or -1.
scan :: (used: [] int, gap: int) -> int {
  last := -1; index := 0;
  while index - last <= gap {
    for u: used { if u == index { last = index; } }
    index += 1;
  }
  return last;
}

ABANDON :: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
NIP06_WORDS :: "leader monkey parrot ring guide accident before fence cannon height naive bean";
// BIP-84's account zpub (SLIP-132 version 04b24746).
BIP84_ZPUB :: "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";

main :: () {
  // Path strings: h and ' both mark a hardened step.
  print("Paths\n");
  show_path("m");
  show_path("m/84'/0'/0'/0/5");
  show_path("m/84h/0h/0h/0/5");
  show_path("m/44'/1237'/0'/0/0");
  show_path("m/0/2147483647'");
  show_path("m/2147483648");
  show_path("m/0H");
  show_path("m//0");
  show_path("m/0/");
  show_path("84'/0'");
  show_path("m/+1");

  // The standard paths of one wallet: the first receiving address of
  // account 0 for each purpose.
  print("Standard paths\n");
  sb: [64] u8; work: [BIP32_WORK_BYTES] u8; node: [BIP32_NODE_BYTES] u8; text: [BIP32_TEXT] u8;
  script: [AD_SCRIPT_MAX] u8; addr: [90] u8; indexes: [5] int; ptext: [64] u8;
  seed := seed_of(ABANDON, sb[..]);
  show("seed", seed);
  purposes := [4] int.[44, 49, 84, 86];
  for purpose: purposes {
    #assert path_standard_vartime(purpose, SW_MAINNET, 0, 0, 0, indexes[..]) == 5;
    p := cast(string) ptext[0..<path_format_vartime(indexes[..], '\'', ptext[..])];
    #assert path_derive_ct(seed, false, p, work[..], node[..]) == BIP32_NODE_BYTES;
    #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES;
    n := path_script_vartime(purpose, node[HDK_KEY..<BIP32_NODE_BYTES], script[..]);
    print("% %: %\n", p, KINDS[path_purpose_kind(purpose)], address(SW_MAINNET, script[0..<n], addr[..]));
  }

  // A watch-only wallet: BIP-86's account xpub makes the same addresses.
  print("Watch-only\n");
  account: [BIP32_NODE_BYTES] u8;
  #assert path_derive_ct(seed, false, "m/86'/0'/0'", work[..], account[..]) == BIP32_NODE_BYTES;
  print("m/86'/0'/0' xprv: %\n", text_of(account[..], text[..]));
  #assert bip32_neuter_ct(account[..], work[..], account[..]) == BIP32_NODE_BYTES;
  print("m/86'/0'/0' xpub: %\n", text_of(account[..], text[..]));
  for c: 0..<2 { for i: 0..<2 {
    n := path_account_script_ct(86, account[..], c, i, work[..], script[..]);
    print("xpub/%/%: %\n", c, i, address(SW_MAINNET, script[0..<n], addr[..]));
  } }
  print("xpub/0/2147483648: %\n", refusal(path_account_script_ct(86, account[..], 0, BIP32_HARDENED, work[..], script[..])));

  // BIP-84 publishes its account key as a zpub: the same 78 bytes with
  // another version.
  print("SLIP-132\n");
  payload: [78] u8;
  #assert base58check_decode_vartime(BIP84_ZPUB, payload[..]) == 78;
  #assert path_derive_ct(seed, false, "m/84'/0'/0'", work[..], node[..]) == BIP32_NODE_BYTES;
  #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES;
  show("zpub version", payload[0..<4]);
  show("xpub version", node[0..<4]);
  same := true;
  for k: 4..<78 { if payload[k] != node[k] { same = false; } }
  print("the other 74 bytes match: %\n", same);
  print("m/84'/0'/0' xpub: %\n", text_of(node[..], text[..]));

  // Testnet: coin type 1, a tprv, and BIP-49's testnet address.
  print("Testnet\n");
  #assert path_standard_vartime(49, SW_TESTNET, 0, 0, 0, indexes[..]) == 5;
  p := cast(string) ptext[0..<path_format_vartime(indexes[..], '\'', ptext[..])];
  #assert path_derive_ct(seed, true, p, work[..], node[..]) == BIP32_NODE_BYTES;
  show("m/49'/1'/0'/0/0 private key", node[HDK_KEY + 1..<BIP32_NODE_BYTES]);
  #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES;
  n := path_script_vartime(49, node[HDK_KEY..<BIP32_NODE_BYTES], script[..]);
  print("%: %\n", p, address(SW_TESTNET, script[0..<n], addr[..]));
  #assert path_derive_ct(seed, true, "m/49'/1'/0'", work[..], node[..]) == BIP32_NODE_BYTES;
  print("m/49'/1'/0' tprv: %\n", text_of(node[..], text[..]));

  // NIP-06: the Nostr book's chapter 46 key is a BIP-44 path.
  print("NIP-06\n");
  s2: [64] u8;
  #assert path_derive_ct(seed_of(NIP06_WORDS, s2[..]), false, "m/44'/1237'/0'/0/0", work[..], node[..]) == BIP32_NODE_BYTES;
  show("m/44'/1237'/0'/0/0 private key", node[HDK_KEY + 1..<BIP32_NODE_BYTES]);

  // The gap limit: a wallet restored from words finds its addresses by
  // scanning until 20 unused ones in a row.
  print("Gap limit\n");
  used := [5] int.[0, 1, 2, 5, 24];
  print("used 0 1 2 5 24, gap 20: found up to %\n", scan(used[..], 20));
  missed := [5] int.[0, 1, 2, 5, 26];
  print("used 0 1 2 5 26, gap 20: found up to %\n", scan(missed[..], 20));
  print("used 0 1 2 5 26, gap 25: found up to %\n", scan(missed[..], 25));

  // Refusals.
  print("Refusals\n");
  print("purpose 48: %\n", refusal(path_standard_vartime(48, SW_MAINNET, 0, 0, 0, indexes[..])));
  print("change 2: %\n", refusal(path_standard_vartime(84, SW_MAINNET, 0, 2, 0, indexes[..])));
  print("account 2^31: %\n", refusal(path_standard_vartime(84, SW_MAINNET, BIP32_HARDENED, 0, 0, indexes[..])));
  print("network 4: %\n", refusal(path_standard_vartime(84, 4, 0, 0, 0, indexes[..])));
  #assert path_derive_ct(seed, false, "m/84'/0'", work[..], node[..]) == BIP32_NODE_BYTES;
  print("a depth-2 node as an account: %\n", refusal(path_account_script_ct(84, node[..], 0, 0, work[..], script[..])));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/derivation-paths.coax
```

```text
Paths
m -> (no steps) -> m
m/84'/0'/0'/0/5 -> 2147483732 2147483648 2147483648 0 5 -> m/84h/0h/0h/0/5
m/84h/0h/0h/0/5 -> 2147483732 2147483648 2147483648 0 5 -> m/84h/0h/0h/0/5
m/44'/1237'/0'/0/0 -> 2147483692 2147484885 2147483648 0 0 -> m/44h/1237h/0h/0/0
m/0/2147483647' -> 0 4294967295 -> m/0/2147483647h
m/2147483648 -> overflow
m/0H -> bad step
m//0 -> empty step
m/0/ -> empty step
84'/0' -> no root
m/+1 -> bad step
Standard paths
seed: 5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4
m/44'/0'/0'/0/0 P2PKH: 1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA
m/49'/0'/0'/0/0 P2SH-P2WPKH: 37VucYSaXLCAsxYyAPfbSi9eh4iEcbShgf
m/84'/0'/0'/0/0 P2WPKH: bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu
m/86'/0'/0'/0/0 P2TR: bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr
Watch-only
m/86'/0'/0' xprv: xprv9xgqHN7yz9MwCkxsBPN5qetuNdQSUttZNKw1dcYTV4mkaAFiBVGQziHs3NRSWMkCzvgjEe3n9xV8oYywvM8at9yRqyaZVz6TYYhX98VjsUk
m/86'/0'/0' xpub: xpub6BgBgsespWvERF3LHQu6CnqdvfEvtMcQjYrcRzx53QJjSxarj2afYWcLteoGVky7D3UKDP9QyrLprQ3VCECoY49yfdDEHGCtMMj92pReUsQ
xpub/0/0: bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr
xpub/0/1: bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh
xpub/1/0: bc1p3qkhfews2uk44qtvauqyr2ttdsw7svhkl9nkm9s9c3x4ax5h60wqwruhk7
xpub/1/1: bc1ptdg60grjk9t3qqcqczp4tlyy3z47yrx9nhlrjsmw36q5a72lhdrs9f00nj
xpub/0/2147483648: bad field
SLIP-132
zpub version: 04b24746
xpub version: 0488b21e
the other 74 bytes match: true
m/84'/0'/0' xpub: xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V
Testnet
m/49'/1'/0'/0/0 private key: c9bdb49cfbaedca21c4b1f3a7803c34636b1d7dc55a717132443fc3f4c5867e8
m/49'/1'/0'/0/0: 2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2
m/49'/1'/0' tprv: tprv8gRrNu65W2Msef2BdBSUgFdRTGzC8EwVXnV7UGS3faeXtuMVtGfEdidVeGbThs4ELEoayCAzZQ4uUji9DUiAs7erdVskqju7hrBcDvDsdbY
NIP-06
m/44'/1237'/0'/0/0 private key: 7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a
Gap limit
used 0 1 2 5 24, gap 20: found up to 24
used 0 1 2 5 26, gap 20: found up to 5
used 0 1 2 5 26, gap 25: found up to 26
Refusals
purpose 48: bad purpose
change 2: bad field
account 2^31: bad field
network 4: bad network
a depth-2 node as an account: not an account
```

What the lines show:

- `'` and `h` give the same indexes, and the path is written back with
  `h`. The bad paths refuse with their own codes.
- The four purposes give four different addresses from one seed. The
  BIP-84 and BIP-86 ones are the published vectors.
- The account `xprv` and `xpub` are BIP-86's, and the `xpub` alone gives
  BIP-86's three addresses and a fourth, `1/1`.
- BIP-84's `zpub` is the `xpub` with another version.
- On testnet the coin type is 1, the key is BIP-49's and the address is
  BIP-49's.
- NIP-06's first phrase gives its published key.
- The gap scan finds index 24 and misses 26 until the gap is 25.

The book's checks run this program in debug and release builds and compare
the output with
[bitcoin/fixtures/derivation-paths.out](../fixtures/derivation-paths.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/derivation-paths.coax](../examples/checks/derivation-paths.coax),
which loads the example and [nip06.coax](../../coax/lib/nostr/nip06.coax).
They copy the published values from BIP-86, BIP-84, BIP-49, BIP-44 and
NIP-06; [path.py](../../coax/tests/bitcoin/path.py) checks each copy
against its pinned file.

<!-- source: bitcoin/examples/checks/derivation-paths.coax check -->
```jai
// BIP-86: the account and each address node give the published xprv and
// xpub, the address nodes give the published addresses, and the account
// xpub alone gives the same addresses.
#forall "BIP-86's nodes and addresses" i: 0..<4 #steps 3000000000 {
  v := DPV_BIP86[i]; node: [BIP32_NODE_BYTES] u8; text: [BIP32_TEXT] u8; account: [BIP32_NODE_BYTES] u8;
  script: [AD_SCRIPT_MAX] u8; addr: [90] u8; work: [BIP32_WORK_BYTES] u8;
  dpv_node(DPV_SEED, false, v.path, node[..]);
  #assert dpv_text(node[..], false, text[..]) == v.xprv && dpv_text(node[..], true, text[..]) == v.xpub;
  if i > 0 {
    #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES;
    n := path_script_vartime(86, node[HDK_KEY..<BIP32_NODE_BYTES], script[..]);
    #assert address(SW_MAINNET, script[0..<n], addr[..]) == v.address;
    dpv_node(DPV_SEED, false, DPV_BIP86[0].path, account[..]);
    #assert bip32_neuter_ct(account[..], work[..], account[..]) == BIP32_NODE_BYTES;
    indexes: [5] int;
    #assert path_parse_vartime(v.path, indexes[..]) == 5;
    m := path_account_script_ct(86, account[..], indexes[3], indexes[4], work[..], script[..]);
    #assert m == n && address(SW_MAINNET, script[0..<m], addr[..]) == v.address;
  }
}
// BIP-84: each key and address, and the account's zprv and zpub: the same
// 78 bytes as the xprv and xpub with SLIP-132's versions.
#forall "BIP-84's keys, addresses and account" i: 0..<3 #steps 2000000000 {
  v := DPV_BIP84[i]; node: [BIP32_NODE_BYTES] u8; script: [AD_SCRIPT_MAX] u8; addr: [90] u8; work: [BIP32_WORK_BYTES] u8;
  dpv_node(DPV_SEED, false, v.path, node[..]);
  #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES && ad_hex_is(node[HDK_KEY..<BIP32_NODE_BYTES], v.key);
  n := path_script_vartime(84, node[HDK_KEY..<BIP32_NODE_BYTES], script[..]);
  #assert address(SW_MAINNET, script[0..<n], addr[..]) == v.address;
  if i == 0 {
    dpv_node(DPV_SEED, false, "m/84'/0'/0'", node[..]);
    #assert dpv_same_but_version(DPV_BIP84_ZPRV, node[..]);
    #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES && dpv_same_but_version(DPV_BIP84_ZPUB, node[..]);
  }
}
// BIP-49 on testnet: coin type 1, the account's uprv and upub, the key and
// the P2SH-P2WPKH address.
#test "BIP-49's testnet account and address" #steps 2000000000 {
  node: [BIP32_NODE_BYTES] u8; script: [AD_SCRIPT_MAX] u8; addr: [90] u8; work: [BIP32_WORK_BYTES] u8; indexes: [5] int; p: [40] u8;
  #assert path_standard_vartime(49, SW_TESTNET, 0, 0, 0, indexes[..]) == 5;
  path := cast(string) p[0..<path_format_vartime(indexes[..], '\'', p[..])];
  #assert path == "m/49'/1'/0'/0/0";
  dpv_node(DPV_SEED, true, path, node[..]);
  #assert ad_hex_is(node[HDK_KEY + 1..<BIP32_NODE_BYTES], DPV_BIP49_PRIVATE);
  #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES && ad_hex_is(node[HDK_KEY..<BIP32_NODE_BYTES], DPV_BIP49_KEY);
  n := path_script_vartime(49, node[HDK_KEY..<BIP32_NODE_BYTES], script[..]);
  #assert address(SW_TESTNET, script[0..<n], addr[..]) == DPV_BIP49_ADDRESS;
  dpv_node(DPV_SEED, true, "m/49'/1'/0'", node[..]);
  #assert dpv_same_but_version(DPV_BIP49_UPRV, node[..]);
  #assert bip32_neuter_ct(node[..], work[..], node[..]) == BIP32_NODE_BYTES && dpv_same_but_version(DPV_BIP49_UPUB, node[..]);
}
// BIP-44's examples parse to purpose 44', coin 0' or 1', an account, a
// change and an index, the same as the standard path built from those
// fields.
#forall "BIP-44's example paths" i: 0..<6 {
  got: [5] int; want: [5] int;
  #assert path_parse_vartime(DPV_BIP44[i], got[..]) == 5;
  network := ifx got[1] == BIP32_HARDENED then SW_MAINNET else SW_TESTNET;
  #assert path_standard_vartime(44, network, got[2] - BIP32_HARDENED, got[3], got[4], want[..]) == 5;
  for k: 0..<5 { #assert got[k] == want[k]; }
}
// NIP-06: the path string gives the indexes the Nostr code hard-codes, and
// the seed of its phrase gives its private key there.
#test "NIP-06's path and key" #steps 1000000000 {
  got: [5] int; node: [BIP32_NODE_BYTES] u8; nip06 := NIP06_PATH;
  #assert path_parse_vartime("m/44'/1237'/0'/0/0", got[..]) == 5;
  for k: 0..<5 { #assert got[k] == nip06[k]; }
  dpv_node(DPV_NIP06_SEED, false, "m/44'/1237'/0'/0/0", node[..]);
  #assert ad_hex_is(node[HDK_KEY + 1..<BIP32_NODE_BYTES], DPV_NIP06_KEY);
}
// The gap-limit scan in the example.
#test "the gap limit" {
  a := [5] int.[0, 1, 2, 5, 24]; b := [5] int.[0, 1, 2, 5, 26]; none: [1] int;
  #assert scan(a[..], 20) == 24 && scan(b[..], 20) == 5 && scan(b[..], 25) == 26 && scan(none[0..<0], 20) == -1;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/derivation-paths.coax
```

`coax test` runs 260 checks, including the loaded files' own checks, and all pass. On 2026-11-03, on a loaded machine, they took about 7.5 minutes of test time; BIP-86's four nodes, each derived from the seed, take most of it.

### The library's own checks

`path.coax` is an `#authority` file. Its own checks cover every step form
and refusal, including `2^31 - 1` and `2^31` with and without a mark and a
26-digit number; paths with every refusal and a full buffer left as it
was; 255 steps that parse and 256 that refuse, and an empty step after 255
good ones; formatting with both marks, index `2^32 - 1`, indexes out of
range, 256 indexes and a buffer one byte short; every purpose, network and
field limit of the standard paths; the four scripts from G and the key
refusals; BIP-32 test vector 1's `m/0H/1` by path string, with the work
buffer zero; and BIP-86's first address from an account `xprv` and `xpub`,
with every refusal of `path_account_script_ct`.

```shell
coax/bin/coax test coax/lib/bitcoin/path.coax
```

`coax test` runs 237 checks, including those of the files it loads, and all pass. On 2026-11-03, on a loaded machine, they took about 3 minutes of test time.

### A Python implementation

[path.py](../../coax/tests/bitcoin/path.py) compiles `path.coax` to C in
checked and release builds and runs its eight exported procedures against
a Python implementation that shares no code with Coax: Python's `str` and
`int` for paths, and chapter 32's `bip32.py` and chapter 29's `address.py`
references for derivation and scripts. Before the Coax code runs, the
Python implementation must reproduce BIP-86's `xprv`s and addresses,
BIP-84's keys and addresses, BIP-49's testnet key and address, NIP-06's key
and BIP-44's example paths, from the pinned files. The cases are:

- the published paths and seeded paths of every depth from 0 to 257, with
  steps around `2^31`, leading zeros and both marks, each with seeded
  edits: a character replaced, inserted or removed;
- seeded index lists to format with `h`, `'` and `H`, indexes out of
  range and buffers one byte short;
- every purpose from 0 to 100, and seeded fields for the standard paths;
- seeded keys for every purpose, with short, uncompressed and off-curve
  keys and short buffers;
- seeds of 15 to 65 bytes with the published and seeded paths;
- account nodes, private and public, and nodes at depths 2 to 4, with
  every change and seeded indexes.

Every result and output must match, nothing may be written past the
output, and the work buffer must come back as zeros.

```shell
python3 coax/tests/bitcoin/path.py
```

On 2026-11-03, on a loaded machine, all 20,311 cases agree with Python in the checked and release native builds, in about 3.5 minutes. The chapter is qualified on native CPU only; the Wasm builds were not run.

Mutation testing was not run for this chapter (owner speed order,
2026-11-03).

## What this chapter leaves out

- Output descriptors, which carry a path in a key's origin and after an
  `xpub`: [chapter 30](ch030-output-descriptors-and-their-checksum.md).
- BIP-48 multisig paths and BIP-87, which descriptors replaced.
- Reading SLIP-132 keys as keys. The example reads one only to show that
  its 78 bytes are an `xpub`'s.
- Looking up an address's history. A wallet asks its node or block filters
  (chapters 74 and 99).
- BIP-85, which derives new seeds from a path.

## Summary

- A path string is `m` and up to 255 steps; each step is a number below
  `2^31`, with `h` or `'` for a hardened step. Empty steps, other marks,
  numbers of `2^31` or more and 256 steps refuse with their own codes.
- BIP-44's levels are purpose, coin type, account, change and address
  index. The first three are hardened, so an account `xpub` makes every
  address of the account and a leak stops at the account.
- Purpose 44 gives P2PKH, 49 P2SH-P2WPKH, 84 P2WPKH and 86 key-path P2TR.
  Coin type 1 is every test network.
- SLIP-132's `zpub` and `ypub` are `xpub`s with another version;
  descriptors replaced them.
- A restored wallet scans each chain until 20 unused addresses in a row.
- BIP-84's, BIP-86's and BIP-49's addresses, BIP-44's paths and NIP-06's
  key come out as published, and the code agrees with a Python
  implementation.

## Dependencies

- [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](ch029-addresses.md)
- [31. Seed phrases](ch031-seed-phrases.md)
- [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md)

## Sources

- [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki),
  for the path notation.
- [BIP-43](https://github.com/bitcoin/bips/blob/master/bip-0043.mediawiki),
  for the purpose field.
- [BIP-44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki),
  for the levels, the gap limit and account discovery, pinned in
  [coax/tests/vectors/upstream/bip44/](../../coax/tests/vectors/upstream/bip44/bip-0044.mediawiki).
- [BIP-49](https://github.com/bitcoin/bips/blob/master/bip-0049.mediawiki),
  [BIP-84](https://github.com/bitcoin/bips/blob/master/bip-0084.mediawiki)
  and [BIP-86](https://github.com/bitcoin/bips/blob/master/bip-0086.mediawiki),
  pinned for chapter 29 in
  [coax/tests/vectors/upstream/](../../coax/tests/vectors/SOURCES.md).
- [SLIP-44](https://github.com/satoshilabs/slips/blob/master/slip-0044.md),
  for coin types 0, 1 and 1237, and
  [SLIP-132](https://github.com/satoshilabs/slips/blob/master/slip-0132.md),
  for the `ypub` and `zpub` versions.
- [NIP-06](../../nostr/nips/official/06.md), for the Nostr path and its
  test vectors.
