# 50. Chain parameters: mainnet, testnet4, signet and regtest

One Bitcoin node binary runs five networks: mainnet, the two public test
networks testnet3 and testnet4, signet, and regtest for local tests. The
consensus code is the same on all five. What differs is a set of
parameters: the genesis block, the four bytes that start every P2P
message, the default port, the address prefixes, the proof-of-work limit,
the heights at which old soft forks apply, and a few blocks that break
today's rules but stay in the chain. Bitcoin Core keeps them in one file,
`kernel/chainparams.cpp`.

This chapter builds that file as a typed record, `ChainParams`, one
constant per network. It builds and hashes each genesis block, derives
signet's message start from its challenge, and checks signet's block
signatures (BIP-325) with chapter 36's script interpreter. Every value is
checked against the pinned `chainparams.cpp` by a Python program that
reads Core's text.

## What this chapter teaches

- What a network's parameters are, and which chapters read each one.
- How `CreateGenesisBlock` builds a genesis block, and why four of the
  five networks share one coinbase.
- The message start (magic bytes), default ports, Base58 versions, BIP-32
  versions and bech32 human-readable parts, and how they tie to chapters
  26, 28, 29 and 32.
- Each network's `powLimit` and retargeting flags, including regtest's
  `fPowNoRetargeting` and testnet4's BIP-94.
- Buried deployments (BIP-90): the BIP-34, BIP-65, BIP-66, CSV and segwit
  heights, and why P2SH and Taproot have none.
- The script flag exception blocks and the BIP-30 exception blocks.
- `nMinimumChainWork`, `defaultAssumeValid`, and why the pinned Core has no
  checkpoints.
- Signet: the challenge, the message start derived from it, and how a
  block's signature spends a virtual transaction.

## Where Bitcoin uses chain parameters

| Parameter | Who reads it | Chapter |
| --- | --- | --- |
| Genesis block | the block index's first entry, and the start of headers sync | 59, 72 |
| Message start, default port | every P2P message, peer discovery | 65, 69 |
| Base58 versions, bech32 part, BIP-32 versions | addresses, WIF keys, extended keys | 26, 28, 29, 32 |
| `powLimit` and the retargeting fields | proof of work and the retarget | 47, 48 |
| Buried deployment heights | header version rules, script flags, the BIP-34 coinbase height | 51, 54, 55 |
| Script flag exceptions, BIP-30 blocks | which rules a historical block is checked with | 55, 57, 58 |
| `nMinimumChainWork` | the work headers must show before a node stores them | 72 |
| `defaultAssumeValid` | the block below which scripts are not checked during sync | 78 |
| Signet challenge | every signet block's signature | this chapter, 57 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [chainparams.coax](../../coax/lib/bitcoin/chainparams.coax) | `ChainParams`, `ChainGenesis` | the fields of Core's `CChainParams` and `Consensus::Params` a node reads |
| | `CHAIN_MAINNET_PARAMS`, `CHAIN_TESTNET3_PARAMS`, `CHAIN_TESTNET4_PARAMS`, `CHAIN_SIGNET_PARAMS`, `CHAIN_REGTEST_PARAMS`, `chain_params` | the five networks |
| | `chain_deployment_height`, `chain_deployment_active` | Core's `DeploymentHeight` and `DeploymentActiveAt` |
| | `chain_push_vartime` | `CScript`'s `operator<<` for bytes |
| | `chain_genesis_block_vartime`, `chain_genesis_check_vartime` | `CreateGenesisBlock`, and Core's asserts on its hashes |
| | `chain_signet_magic_vartime`, `chain_from_magic_vartime`, `chain_signet_custom_vartime` | `GetSignetMessageStart`, `GetNetworkForMagic`, and a signet with another challenge |
| | `chain_script_exception_vartime`, `chain_bip30_repeat_vartime`, `chain_bip30_unspendable_vartime` | the exception blocks |
| | `chain_commitment_index_vartime`, `chain_signet_fetch_vartime`, `chain_signet_check_vartime` | `GetWitnessCommitmentIndex`, `FetchAndClearCommitmentSection` and `CheckSignetBlockSolution` |
| [retarget.coax](../../coax/lib/bitcoin/retarget.coax) | `RetargetParams`, `RETARGET_MAINNET`, `RETARGET_TESTNET3`, `RETARGET_TESTNET4`, `RETARGET_REGTEST` | chapter 48's retargeting fields, which `ChainParams` holds |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_verify_vartime` | chapter 36's `VerifyScript`, which checks a signet solution |
| [merkle.coax](../../coax/lib/bitcoin/merkle.coax) | `merkle_block_ids_vartime`, `merkle_root_vartime` | chapter 46's block reader and Merkle root |

Parameters, blocks and signet solutions are public, so every procedure
that decides runs in variable time and its name ends in `_vartime`
(chapter 3).

## The record

Core's `CChainParams` holds the network's identity and `Consensus::Params`
holds what consensus reads. `ChainParams` keeps the fields a node uses from
both. Hashes are text, as explorers and Core's source show them; an empty
string is Core's null `uint256`.

<!-- source: coax/lib/bitcoin/chainparams.coax chains -->
```jai
// The networks, in the order of Core's ChainType. Testnet3 is still in
// the pinned Core; it is reset from time to time and testnet4 replaces it.
CHAIN_MAIN :: 1; CHAIN_TESTNET3 :: 2; CHAIN_TESTNET4 :: 3; CHAIN_SIGNET :: 4; CHAIN_REGTEST :: 5;
CHAIN_NAMES :: [6] string.["", "main", "test", "testnet4", "signet", "regtest"];
// Core's BuriedDeployment: soft forks whose activation height is fixed in
// the code (BIP-90). P2SH and Taproot are not here: Core enforces them from
// genesis, except in the script flag exception blocks.
DEP_HEIGHTINCB :: 1; DEP_CLTV :: 2; DEP_DERSIG :: 3; DEP_CSV :: 4; DEP_SEGWIT :: 5;
```

<!-- source: coax/lib/bitcoin/chainparams.coax params -->
```jai
// A genesis block's fields: CreateGenesisBlock's arguments, and the two
// hashes Core asserts, as explorers show them.
ChainGenesis :: struct {
  message: string;       // pszTimestamp, pushed in the coinbase's scriptSig
  script: string;        // the coinbase output's scriptPubKey, in hex
  time: u32; nonce: u32; bits: u32; version: u32;
  reward: int;           // satoshis
  hash: string;          // hashGenesisBlock
  merkle_root: string;   // hashMerkleRoot
}
// The fields of Core's CChainParams and Consensus::Params a node reads.
// Hashes are hex as explorers show them; "" is Core's null uint256.
ChainParams :: struct {
  chain: int;
  magic: [4] u8;         // pchMessageStart: the first bytes of every P2P message
  port: int;             // nDefaultPort
  pubkey_prefix: int;    // base58Prefixes: P2PKH, P2SH and WIF version bytes
  script_prefix: int;
  secret_prefix: int;
  xpub: u32; xprv: u32;  // BIP-32 versions, EXT_PUBLIC_KEY and EXT_SECRET_KEY
  hrp: string;           // bech32_hrp
  network: int;          // the SW_* network chapters 28 and 29 encode addresses for
  genesis: ChainGenesis;
  retarget: RetargetParams;  // chapter 48's powLimit, spacing, timespan and flags
  halving: int;          // nSubsidyHalvingInterval
  bip34_height: int; bip34_hash: string;
  bip65_height: int; bip66_height: int; csv_height: int; segwit_height: int;
  min_bip9_warning: int; // MinBIP9WarningHeight
  bip16_exception: string;   // script_flag_exceptions: verified with no flags
  taproot_exception: string; // verified with P2SH and WITNESS only
  min_chain_work: string;    // nMinimumChainWork, 64 hex digits
  assume_valid: string;      // defaultAssumeValid
  assume_valid_height: int;  // the height chainparams.cpp's comment gives it
  signet: bool;              // signet_blocks
  challenge: string;         // signet_challenge, in hex
}
```

Fields Core has that `ChainParams` leaves out are the DNS seeds and fixed
seed addresses (chapter 69), the AssumeUTXO snapshots (chapter 79),
`chainTxData` for progress estimates, the prune and size hints, and the
BIP-9 test deployment `testdummy`. None of them is consensus.

## The five networks

Each network is one constant. The values are Core's, field for field.

<!-- source: coax/lib/bitcoin/chainparams.coax networks -->
```jai
CHAIN_MAINNET_PARAMS :: ChainParams.{
  chain=CHAIN_MAIN, magic=[4]u8.[0xf9, 0xbe, 0xb4, 0xd9], port=8333,
  pubkey_prefix=0, script_prefix=5, secret_prefix=128, xpub=0x0488b21e, xprv=0x0488ade4,
  hrp="bc", network=SW_MAINNET,
  genesis=ChainGenesis.{message=SATOSHI_GENESIS_MESSAGE, script=SATOSHI_GENESIS_SCRIPT,
    time=1231006505, nonce=2083236893, bits=0x1d00ffff, version=1, reward=50 * CHAIN_COIN,
    hash="000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f", merkle_root=SATOSHI_MERKLE_ROOT},
  retarget=RETARGET_MAINNET, halving=210000,
  bip34_height=227931, bip34_hash="000000000000024b89b42a942fe0d9fea3bb44ab7bd1b19115dd6a759c0808b8",
  bip65_height=388381, bip66_height=363725, csv_height=419328, segwit_height=481824, min_bip9_warning=711648,
  bip16_exception="00000000000002dc756eebf4f49723ed8d30cc28a5f108eb94b1ba88ac4f9c22",
  taproot_exception="0000000000000000000f14c35b2d841e986ab5441de8c585d5ffe55ea1e395ad",
  min_chain_work="000000000000000000000000000000000000000145ec036acc5ba740052af1a0",
  assume_valid="00000000000000000000748969ec33043c0e52a763c6dd5193861f559f2c72e3", assume_valid_height=966143};
CHAIN_TESTNET3_PARAMS :: ChainParams.{
  chain=CHAIN_TESTNET3, magic=[4]u8.[0x0b, 0x11, 0x09, 0x07], port=18333,
  pubkey_prefix=111, script_prefix=196, secret_prefix=239, xpub=0x043587cf, xprv=0x04358394,
  hrp="tb", network=SW_TESTNET,
  genesis=ChainGenesis.{message=SATOSHI_GENESIS_MESSAGE, script=SATOSHI_GENESIS_SCRIPT,
    time=1296688602, nonce=414098458, bits=0x1d00ffff, version=1, reward=50 * CHAIN_COIN,
    hash="000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943", merkle_root=SATOSHI_MERKLE_ROOT},
  retarget=RETARGET_TESTNET3, halving=210000,
  bip34_height=21111, bip34_hash="0000000023b3a96d3484e5abb3755c413e7d41500f8e2a5c3f0dd01299cd8ef8",
  bip65_height=581885, bip66_height=330776, csv_height=770112, segwit_height=834624, min_bip9_warning=2013984,
  bip16_exception="00000000dd30457c001f4095d208cc1296b0eed002427aa599874af7a432b105", taproot_exception="",
  min_chain_work="0000000000000000000000000000000000000000000017f49f702147f10c0eb6",
  assume_valid="00000000b318a3703d14a844c55ef507f4c2fc8f8766e24271fd43c180c51637", assume_valid_height=5128859};
CHAIN_TESTNET4_PARAMS :: ChainParams.{
  chain=CHAIN_TESTNET4, magic=[4]u8.[0x1c, 0x16, 0x3f, 0x28], port=48333,
  pubkey_prefix=111, script_prefix=196, secret_prefix=239, xpub=0x043587cf, xprv=0x04358394,
  hrp="tb", network=SW_TESTNET,
  genesis=ChainGenesis.{message="03/May/2024 000000000000000000001ebd58c244970b3aa9d783bb001011fbe8ea8e98e00e",
    script="21000000000000000000000000000000000000000000000000000000000000000000ac",
    time=1714777860, nonce=393743547, bits=0x1d00ffff, version=1, reward=50 * CHAIN_COIN,
    hash="00000000da84f2bafbbc53dee25a72ae507ff4914b867c565be350b0da8bf043",
    merkle_root="7aa0a7ae1e223414cb807e40cd57e667b718e42aaf9306db9102fe28912b7b4e"},
  retarget=RETARGET_TESTNET4, halving=210000,
  bip34_height=1, bip34_hash="", bip65_height=1, bip66_height=1, csv_height=1, segwit_height=1, min_bip9_warning=0,
  bip16_exception="", taproot_exception="",
  min_chain_work="000000000000000000000000000000000000000000000e346a558455ade8eca9",
  assume_valid="0000000021df65b91665a342e26ceb05e54826ad7d8fcd40316230058fa3b865", assume_valid_height=151604};
// Signet's powLimit is 0x00000377ae followed by 54 zero digits: the
// target of nBits 0x1e0377ae, its genesis block's.
POW_LIMIT_SIGNET :: U256.{limb=[8]u32.[0, 0, 0, 0, 0, 0, 0xae000000, 0x00000377]};
RETARGET_SIGNET :: RetargetParams.{pow_limit=POW_LIMIT_SIGNET, spacing=600, timespan=TWO_WEEKS};
// The default signet challenge: a bare 1-of-2 multisig,
// OP_1 <key> <key> OP_2 OP_CHECKMULTISIG.
SIGNET_DEFAULT_CHALLENGE :: "512103ad5e0edad18cb1f0fc0d28a3d4f1f3e445640337489abb10404f2d1e086be430210359ef5021964fe22d6f8e05b2463c9540ce96883fe3b278760f048f5189f2e6c452ae";
CHAIN_SIGNET_PARAMS :: ChainParams.{
  chain=CHAIN_SIGNET, magic=[4]u8.[0x0a, 0x03, 0xcf, 0x40], port=38333,
  pubkey_prefix=111, script_prefix=196, secret_prefix=239, xpub=0x043587cf, xprv=0x04358394,
  hrp="tb", network=SW_TESTNET,
  genesis=ChainGenesis.{message=SATOSHI_GENESIS_MESSAGE, script=SATOSHI_GENESIS_SCRIPT,
    time=1598918400, nonce=52613770, bits=0x1e0377ae, version=1, reward=50 * CHAIN_COIN,
    hash="00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6", merkle_root=SATOSHI_MERKLE_ROOT},
  retarget=RETARGET_SIGNET, halving=210000,
  bip34_height=1, bip34_hash="", bip65_height=1, bip66_height=1, csv_height=1, segwit_height=1, min_bip9_warning=0,
  bip16_exception="", taproot_exception="",
  min_chain_work="00000000000000000000000000000000000000000000000000001090e9dc1520",
  assume_valid="00000002a5e0ba0498f1e9f4591af0b66b63c654665efe65206fd0ae7bbaf923", assume_valid_height=321295,
  signet=true, challenge=SIGNET_DEFAULT_CHALLENGE};
CHAIN_REGTEST_PARAMS :: ChainParams.{
  chain=CHAIN_REGTEST, magic=[4]u8.[0xfa, 0xbf, 0xb5, 0xda], port=18444,
  pubkey_prefix=111, script_prefix=196, secret_prefix=239, xpub=0x043587cf, xprv=0x04358394,
  hrp="bcrt", network=SW_REGTEST,
  genesis=ChainGenesis.{message=SATOSHI_GENESIS_MESSAGE, script=SATOSHI_GENESIS_SCRIPT,
    time=1296688602, nonce=2, bits=0x207fffff, version=1, reward=50 * CHAIN_COIN,
    hash="0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206", merkle_root=SATOSHI_MERKLE_ROOT},
  retarget=RETARGET_REGTEST, halving=150,
  bip34_height=1, bip34_hash="", bip65_height=1, bip66_height=1, csv_height=1, segwit_height=0, min_bip9_warning=0,
  bip16_exception="", taproot_exception="", min_chain_work="", assume_valid="", assume_valid_height=0};
```

A few things stand out:

- **Mainnet, testnet3, signet and regtest share one genesis coinbase.**
  `CreateGenesisBlock` builds the same transaction for all four, Satoshi's
  newspaper headline paying 50 bitcoin to a fixed key, so their Merkle
  roots are all `4a5e1e4b…`. Their headers differ in time, nonce and
  `nBits`. Testnet4, from May 2024, has its own message, a recent mainnet
  block hash, and pays to a key of 33 zero bytes that nobody can spend.
- **Testnet3, testnet4, signet and regtest share address prefixes.** All
  four use Base58 versions 111, 196 and 239 and the `tpub` and `tprv`
  BIP-32 versions. Testnet3, testnet4 and signet share bech32's `tb`;
  regtest has `bcrt`. An address alone does not say which test network it
  is for.
- **Regtest never retargets.** `fPowNoRetargeting` keeps every block at
  the genesis `nBits`, `0x207fffff`, a target of about 2^255, so a CPU
  finds a block in one or two tries. Its timespan is one day, which only
  `retarget_interval` reads: 144 blocks.
- **Signet's `powLimit` is easy too**, `0x00000377ae…`, but signet's
  security is its signature, not its work.
- **Testnet4 enforces BIP-94**, both the first-block-of-period rule of
  chapter 48 and the timewarp rule of chapter 51.

`chain_params` returns a network by number.
`chain_deployment_height` is Core's `DeploymentHeight`: the height from
which a buried deployment applies.

<!-- source: coax/lib/bitcoin/chainparams.coax lookup -->
```jai
// The parameters of a network, 1 to 5; anything else is mainnet's with
// chain 0, which no procedure accepts as a network.
chain_params :: (chain: int) -> ChainParams #export_c {
  if chain == CHAIN_MAIN { return CHAIN_MAINNET_PARAMS; }
  if chain == CHAIN_TESTNET3 { return CHAIN_TESTNET3_PARAMS; }
  if chain == CHAIN_TESTNET4 { return CHAIN_TESTNET4_PARAMS; }
  if chain == CHAIN_SIGNET { return CHAIN_SIGNET_PARAMS; }
  if chain == CHAIN_REGTEST { return CHAIN_REGTEST_PARAMS; }
  p := CHAIN_MAINNET_PARAMS; p.chain = 0;
  return p;
}
// Core's DeploymentHeight: the height from which a buried deployment's
// rules apply. Core's DeploymentActiveAt is height >= this.
chain_deployment_height :: (p: ChainParams, dep: int) -> int #export_c {
  if dep == DEP_HEIGHTINCB { return p.bip34_height; }
  if dep == DEP_CLTV { return p.bip65_height; }
  if dep == DEP_DERSIG { return p.bip66_height; }
  if dep == DEP_CSV { return p.csv_height; }
  if dep == DEP_SEGWIT { return p.segwit_height; }
  return -1;
}
chain_deployment_active :: (p: ChainParams, dep: int, height: int) -> bool #export_c {
  h := chain_deployment_height(p, dep);
  return h >= 0 && height >= h;
}
```

## The genesis block

Every chain starts at a block whose previous hash is null. A node does not
download it; it builds it. Core's `CreateGenesisBlock` makes one
transaction and a header over it:

<!-- source: coax/lib/bitcoin/chainparams.coax genesis -->
```jai
// The first two pushes of every genesis coinbase's scriptSig: Core writes
// CScript() << 486604799 << CScriptNum(4). 486,604,799 is 0x1d00ffff,
// four bytes little-endian; CScriptNum(4) is the one byte 04.
CHAIN_GENESIS_PREFIX :: "04ffff001d0104";
SATOSHI_GENESIS_MESSAGE :: "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks";
SATOSHI_GENESIS_SCRIPT :: "4104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac";
SATOSHI_MERKLE_ROOT :: "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
CHAIN_COIN :: 100000000;
```

The scriptSig is built with `CScript`'s `operator<<`, which pushes bytes
with the shortest length prefix. It never turns one byte into `OP_1` to
`OP_16`: those are opcodes, and `operator<<` for bytes always pushes data.

<!-- source: coax/lib/bitcoin/chainparams.coax push -->
```jai
// CScript's operator<< for bytes: the shortest of a direct push (to 75
// bytes), OP_PUSHDATA1, OP_PUSHDATA2 and OP_PUSHDATA4. It never uses OP_0
// or OP_1 to OP_16; those are opcodes, not pushed bytes. Writes at at and
// returns where writing goes on, or -1 with no room.
chain_push_vartime :: (out: [] u8, at: int, data: [] u8) -> int #export_c {
  n := data.count;
  width := ifx n < OP_PUSHDATA1 then 0 else ifx n <= 0xff then 1 else ifx n <= 0xffff then 2 else 4;
  if at < 0 || 1 + width + n > out.count - at { return -1; }
  out[at] = cast(u8) (ifx width == 0 then n else ifx width == 1 then cast(int) OP_PUSHDATA1 else ifx width == 2 then cast(int) OP_PUSHDATA2 else cast(int) OP_PUSHDATA4);
  for i: 0..<width { out[at+1+i] = cast(u8) ((n >> (8 * i)) & 255); }
  for i: 0..<n { out[at+1+width+i] = data[i]; }
  return at + 1 + width + n;
}
```

The block is the 80-byte header, a transaction count of one, and the
coinbase. The Merkle root of a single transaction is its txid (chapter 46).

<!-- source: coax/lib/bitcoin/chainparams.coax build -->
```jai
// CreateGenesisBlock: the 80-byte header, a count of one, and the coinbase:
// version 1, one input with the null outpoint (32 zero bytes, index
// 0xffffffff), the scriptSig 486604799, 4 and the message, sequence
// 0xffffffff; one output of the reward to the genesis script; locktime 0.
// The Merkle root of one transaction is its txid. Returns the block's
// length, or -1 when out is too small or a field does not decode.
CHAIN_GENESIS_MAX :: 512;
chain_genesis_block_vartime :: (g: ChainGenesis, out: [] u8) -> int #export_c {
  if out.count < CHAIN_GENESIS_MAX || g.message.count > 255 || g.script.count > 2 * 100 { return -1; }
  sig: [300] u8; spk: [100] u8; prefix: [7] u8; message: [255] u8;
  #assert cp_unhex(CHAIN_GENESIS_PREFIX, prefix[..]) == 7;
  for i: 0..<7 { sig[i] = prefix[i]; }
  for i: 0..<g.message.count { message[i] = g.message[i]; }
  sig_n := chain_push_vartime(sig[..], 7, message[0..<g.message.count]);
  spk_n := cp_unhex(g.script, spk[..]);
  if spk_n < 0 { return -1; }
  // The coinbase, after the header and the count.
  at := 81;
  at = cp_le(out, at, 1, 4);
  out[at] = 1; at += 1;
  for i: 0..<32 { out[at+i] = 0; }
  at = cp_le(out, at + 32, 0xffffffff, 4);
  at = tx_compact_write_vartime(out, at, sig_n);
  at = cp_put(out, at, sig[0..<sig_n]);
  at = cp_le(out, at, 0xffffffff, 4);
  out[at] = 1; at += 1;
  at = cp_le(out, at, g.reward, 8);
  at = tx_compact_write_vartime(out, at, spk_n);
  at = cp_put(out, at, spk[0..<spk_n]);
  at = cp_le(out, at, 0, 4);
  // The header: version, a null previous hash, the root, time, bits, nonce.
  root: [32] u8;
  #assert sha256d_checked(out[81..<at], root[..], false) == SHA_OK;
  h := cp_le(out, 0, cast(int) g.version, 4);
  for i: 0..<32 { out[h+i] = 0; }
  h = cp_put(out, h + 32, root[..]);
  h = cp_le(out, h, cast(int) g.time, 4);
  h = cp_le(out, h, cast(int) g.bits, 4);
  h = cp_le(out, h, cast(int) g.nonce, 4);
  out[80] = 1;
  return at;
}
// Builds the genesis block and checks it as Core's asserts do: the Merkle
// root, then the hash; then the hash meets the block's nBits under the
// network's powLimit, as every block must.
chain_genesis_check_vartime :: (p: ChainParams) -> int #export_c {
  b: [CHAIN_GENESIS_MAX] u8; hash: [32] u8;
  n := chain_genesis_block_vartime(p.genesis, b[..]);
  if n < 0 { return CHAIN_CAPACITY; }
  if !chain_hash_is_vartime(b[36..<68], p.genesis.merkle_root) { return CHAIN_BAD_MERKLE; }
  #assert pow_block_hash_vartime(b[0..<80], hash[..]) == POW_OK;
  if !chain_hash_is_vartime(hash[..], p.genesis.hash) { return CHAIN_BAD_GENESIS; }
  if pow_check_vartime(hash[..], p.genesis.bits, p.retarget.pow_limit) != POW_OK { return CHAIN_HIGH_HASH; }
  return CHAIN_OK;
}
```

`chain_genesis_check_vartime` checks what Core asserts when it builds its
parameters: the Merkle root, then the block hash. It also checks that the
hash meets the header's `nBits` under `powLimit` (chapter 47), as every
block must. Each genesis block claims the easiest target its network
allows: its `nBits` is `powLimit` in compact form.

### Worked example: the mainnet genesis block

The coinbase is 204 bytes:

```text
01000000                                  version 1
01                                        one input
00…00 ffffffff                            the null outpoint
4d                                        a scriptSig of 77 bytes:
  04 ffff001d                               push 4: 486604799
  01 04                                     push 1: 4
  45 5468652054696d65…62616e6b73            push 69: "The Times 03/Jan/2009 …"
ffffffff                                  sequence
01                                        one output
00f2052a01000000                          5,000,000,000 satoshis
43 4104678afd…1d5f ac                     push 65: an uncompressed key, OP_CHECKSIG
00000000                                  locktime
```

Its SHA-256d, reversed, is `4a5e1e4b…eda33b`, the Merkle root. The header
is version 1, 32 zero bytes, that root, time 1,231,006,505 (3 January
2009, 18:15:05 UTC), `nBits` `1d00ffff` and nonce 2,083,236,893. Its
SHA-256d, reversed, is
`000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f`, below
the target `00000000ffff0000…`. Testnet4's message is 76 bytes, one more
than a direct push allows, so its push starts `4c 4c`: `OP_PUSHDATA1` and
the length.

## Message start and ports

Every P2P message starts with four bytes that name the network
(chapter 65). Core chose bytes that are rare in other data and not valid
UTF-8. A node that reads another network's magic drops the connection.
Each network also has a default port: 8333, 18333, 48333, 38333 and 18444.

Signet's magic is not a constant. BIP-325 derives it from the challenge,
the script every block must satisfy: the first four bytes of SHA-256d of
the challenge serialized with its length. A signet with another challenge
gets other magic bytes, so its nodes find each other and no one else.

<!-- source: coax/lib/bitcoin/chainparams.coax magic -->
```jai
// GetSignetMessageStart (BIP-325): the first four bytes of SHA-256d of
// the challenge serialized as a script, its length first. Every signet
// with another challenge has other magic bytes, so its peers find each
// other and nobody else.
chain_signet_magic_vartime :: (challenge: [] u8, out: [] u8) -> bool #export_c {
  if out.count != 4 || challenge.count > SCRIPT_MAX_SIZE { return false; }
  s: [SCRIPT_MAX_SIZE + 3] u8; digest: [32] u8;
  at := tx_compact_write_vartime(s[..], 0, challenge.count);
  at = cp_put(s[..], at, challenge);
  #assert sha256d_checked(s[0..<at], digest[..], false) == SHA_OK;
  for i: 0..<4 { out[i] = digest[i]; }
  return true;
}
// Core's GetNetworkForMagic: which network a message's first four bytes
// name, or 0. Signet means the default signet.
chain_from_magic_vartime :: (magic: [] u8) -> int #export_c {
  if magic.count != 4 { return 0; }
  for chain: CHAIN_MAIN..<CHAIN_REGTEST + 1 {
    p := chain_params(chain);
    same := true;
    for i: 0..<4 { if magic[i] != p.magic[i] { same = false; } }
    if same { return chain; }
  }
  return 0;
}
// SigNetParams with a custom challenge (-signetchallenge), given in hex:
// its own magic, and no minimum chain work or assumed-valid block. chain
// is 0 when the challenge does not decode.
chain_signet_custom_vartime :: (challenge: string) -> ChainParams #export_c {
  p := CHAIN_SIGNET_PARAMS;
  p.challenge = challenge; p.min_chain_work = ""; p.assume_valid = ""; p.assume_valid_height = 0;
  b: [SCRIPT_MAX_SIZE] u8;
  n := cp_unhex(challenge, b[..]);
  if n < 0 { p.chain = 0; return p; }
  #assert chain_signet_magic_vartime(b[0..<n], p.magic[..]);
  return p;
}
```

For the default challenge, 71 bytes, the hash input is `47` and the
challenge, and the magic is `0a03cf40`. A signet whose challenge is
`OP_TRUE` (`51`) has magic `54d26fbd`.

## Buried deployments

Soft forks before Taproot activated by miner signalling (BIP-9) or by
height. Once they were deep in the chain, BIP-90 replaced the signalling
logic with the height at which each one activated. Core calls these buried
deployments:

| Deployment | Rules | Mainnet height | Testnet3 | Testnet4, signet | Regtest |
| --- | --- | --- | --- | --- | --- |
| `DEPLOYMENT_HEIGHTINCB` | BIP-34: the coinbase starts with the height | 227,931 | 21,111 | 1 | 1 |
| `DEPLOYMENT_DERSIG` | BIP-66: strict DER signatures | 363,725 | 330,776 | 1 | 1 |
| `DEPLOYMENT_CLTV` | BIP-65: `OP_CHECKLOCKTIMEVERIFY` | 388,381 | 581,885 | 1 | 1 |
| `DEPLOYMENT_CSV` | BIP-68, BIP-112, BIP-113: relative locktimes and median time | 419,328 | 770,112 | 1 | 1 |
| `DEPLOYMENT_SEGWIT` | BIP-141, BIP-143, BIP-147 | 481,824 | 834,624 | 1 | 0 |

A deployment is active at a height at or above its own. Chapter 51 checks
header versions against these heights, chapter 54 the BIP-34 height in the
coinbase, and chapter 55 the script flags each height turns on.

P2SH (BIP-16) and Taproot (BIP-341) have no height. Core enforces both
from genesis and lists the blocks that would fail as exceptions. Mainnet
block 170,060 (`…4f9c22`) spends an output of the P2SH form before
P2SH, so it is verified with no flags. Block 692,261 (`…e395ad`) spends a
witness v1 output before Taproot activated, so it is verified with P2SH
and WITNESS only. Testnet3 has one BIP-16 exception. Chapter 55 applies
them.

`MinBIP9WarningHeight` is the height below which Core does not warn about
unknown version bits: Taproot's activation plus one period on mainnet,
711,648. The pinned Core has no BIP-9 deployment other than `testdummy`.

## Exception blocks

<!-- source: coax/lib/bitcoin/chainparams.coax exceptions -->
```jai
// script_flag_exceptions: the flags a block's scripts are verified with
// when Core's default would refuse it, or -1 for every other block. The
// BIP-16 exception (mainnet block 170,060, and one testnet3 block) spends
// an output of the P2SH form that P2SH would refuse; the Taproot exception
// (mainnet block 692,261) spends a witness v1 output before Taproot.
chain_script_exception_vartime :: (p: ChainParams, hash: [] u8) -> int #export_c {
  if chain_hash_is_vartime(hash, p.bip16_exception) { return 0; }
  if chain_hash_is_vartime(hash, p.taproot_exception) { return SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS; }
  return -1;
}
// validation.cpp's BIP-30 blocks. Two coinbases of 2010 have the txids of
// earlier ones (blocks 91,722 and 91,812), so the earlier outputs could
// never be spent. IsBIP30Repeat names the blocks that repeated them, which
// ConnectBlock lets through; IsBIP30Unspendable the blocks whose outputs
// were overwritten, which DisconnectBlock skips.
CHAIN_BIP30_REPEAT_HEIGHTS :: [2] int.[91842, 91880];
CHAIN_BIP30_REPEAT_HASHES :: [2] string.["00000000000a4d0a398161ffc163c503763b1f4360639393e0e4c8e300e0caec",
  "00000000000743f190a18c5577a3c2d2a1f610ae9601ac046a38084ccb7cd721"];
CHAIN_BIP30_UNSPENDABLE_HEIGHTS :: [2] int.[91722, 91812];
CHAIN_BIP30_UNSPENDABLE_HASHES :: [2] string.["00000000000271a2dc26e7667f8419f2e15416dc6955e5a6c6cdf3f2574dd08e",
  "00000000000af0aed4792b1acee3d966af36cf5def14935db8de83d6f9306f2f"];
// From this height the BIP-34 height in a coinbase no longer prevents a
// repeat: a coinbase of block 1,983,702 could match an early coinbase's
// scriptSig, so ConnectBlock checks BIP-30 again from there.
CHAIN_BIP34_IMPLIES_BIP30_LIMIT :: 1983702;
chain_bip30_repeat_vartime :: (height: int, hash: [] u8) -> bool #export_c {
  for i: 0..<2 { if height == CHAIN_BIP30_REPEAT_HEIGHTS[i] && chain_hash_is_vartime(hash, CHAIN_BIP30_REPEAT_HASHES[i]) { return true; } }
  return false;
}
chain_bip30_unspendable_vartime :: (height: int, hash: [] u8) -> bool #export_c {
  for i: 0..<2 { if height == CHAIN_BIP30_UNSPENDABLE_HEIGHTS[i] && chain_hash_is_vartime(hash, CHAIN_BIP30_UNSPENDABLE_HASHES[i]) { return true; } }
  return false;
}
```

The BIP-30 blocks are not in `chainparams.cpp` but in `validation.cpp`,
for mainnet only. In 2010 the coinbases of blocks 91,812 and 91,722 were
repeated, byte for byte, in blocks 91,842 and 91,880. The same txid meant
the later outputs overwrote the earlier ones, and 100 bitcoin became
unspendable. BIP-30 forbids a transaction whose txid matches an unspent
one; BIP-34 later made coinbases unique by putting the height in them.
Chapter 58 uses these blocks when it connects and disconnects blocks.

## Minimum chain work and assumed-valid blocks

`nMinimumChainWork` is the chainwork (chapter 48) a chain of headers must
reach before a node stores it, so a peer cannot fill a new node's memory
with cheap headers (chapter 72). Mainnet's is
`0x…0145ec036acc5ba740052af1a0`, the chainwork to block 966,143, which
chapter 48 recomputes from the headers. `defaultAssumeValid` is that block
(`…2c72e3`): a syncing node skips script checks in its ancestors
(chapter 78) but checks everything else. Regtest and a custom signet have
neither.

The pinned Core has no checkpoints. Core used to hard-code hashes at
fixed heights and refuse any chain without them. `nMinimumChainWork` and
headers presync now do that job without naming a chain, and Core removed
the last checkpoints. The Python check confirms the word does not appear
in the pinned `chainparams.cpp`.

## Signet's block signature

A signet block is a normal block plus a signature. The signer, or signers,
are named by the challenge, a script. The default signet's challenge is a
bare 1-of-2 multisig:

```text
51                                                 OP_1
21 03ad5e0edad18cb1f0fc0d28a3d4f1f3e445640337489abb10404f2d1e086be430
21 0359ef5021964fe22d6f8e05b2463c9540ce96883fe3b278760f048f5189f2e6c4
52                                                 OP_2
ae                                                 OP_CHECKMULTISIG
```

The signature, called the solution, lives in the coinbase's witness
commitment output (chapter 49), as a push that starts with the four bytes
`ecc7daa2`.

<!-- source: coax/lib/bitcoin/chainparams.coax commitment -->
```jai
// GetWitnessCommitmentIndex: the last coinbase output whose scriptPubKey
// has at least 38 bytes and starts OP_RETURN, a push of 36 and aa21a9ed.
// -1 when there is none. Chapter 49 checks the commitment itself.
SIGNET_COMMITMENT_HEADER :: [6] u8.[0x6a, 0x24, 0xaa, 0x21, 0xa9, 0xed];
chain_commitment_index_vartime :: (b: [] u8, t: Tx, outs: [] TxOut) -> int #export_c {
  found := -1;
  for o: 0..<t.nout {
    s := b[outs[o].script..<outs[o].script+outs[o].script_n];
    if s.count < 38 { continue; }
    same := true;
    for i: 0..<6 { if s[i] != SIGNET_COMMITMENT_HEADER[i] { same = false; } }
    if same { found = o; }
  }
  return found;
}
```

The solution cannot sign itself, so Core removes it before computing what
is signed. `FetchAndClearCommitmentSection` rewrites the commitment script
op by op. The push that holds the solution keeps only `ecc7daa2`, and
every other push is rewritten with `operator<<`, so a push written with a
longer prefix than needed changes too.

<!-- source: coax/lib/bitcoin/chainparams.coax fetch -->
```jai
// FetchAndClearCommitmentSection: the first push in the commitment script
// longer than four bytes that starts ecc7daa2 holds the solution after
// those four bytes. The script is rewritten op by op, every push with
// CScript's operator<<, and that push keeps only the four bytes. An empty
// push is written as its opcode alone. Reading stops at an op that runs
// past the end, as Core's loop does. Writes the new script into out and
// returns its length; at and n locate the solution in s, n = -1 when there
// is none (then the script is unchanged).
SIGNET_HEADER :: [4] u8.[0xec, 0xc7, 0xda, 0xa2];
SignetFetch :: struct { length: int; at: int; n: int; }
chain_signet_fetch_vartime :: (s: [] u8, out: [] u8) -> SignetFetch #export_c {
  r := SignetFetch.{n=-1};
  w := 0; pc := 0;
  while pc < s.count {
    op := script_op_vartime(s, pc);
    if !op.ok { break; }
    if op.n > 0 {
      data := s[op.at..<op.at+op.n];
      if r.n < 0 && op.n > 4 && data[0] == SIGNET_HEADER[0] && data[1] == SIGNET_HEADER[1] && data[2] == SIGNET_HEADER[2] && data[3] == SIGNET_HEADER[3] {
        r.at = op.at + 4; r.n = op.n - 4;
        data = data[0..<4];
      }
      w = chain_push_vartime(out, w, data);
    } else {
      if w >= out.count { w = -1; }
      else { out[w] = cast(u8) op.op; w += 1; }
    }
    if w < 0 { return SignetFetch.{length=-1, n=-1}; }
    pc = op.next;
  }
  if r.n < 0 {
    if s.count > out.count { return SignetFetch.{length=-1, n=-1}; }
    for i: 0..<s.count { out[i] = s[i]; }
    w = s.count;
  }
  r.length = w;
  return r;
}
```

The solution is a scriptSig and a witness stack, serialized as in a
transaction. It must spend a virtual output whose scriptPubKey is the
challenge. BIP-325 builds two transactions that never appear in a block:

- `to_spend` commits to the block. Its one input has the null outpoint and
  the scriptSig `OP_0` and a 72-byte push: the header's version, previous
  hash, a Merkle root and time. The root is the block's, with the
  coinbase's txid taken after the solution was cleared. Its one output
  pays 0 to the challenge.
- `to_sign` spends `to_spend`'s output 0 with the solution, and pays 0 to
  `OP_RETURN`.

`to_spend`'s txid enters `to_sign`'s signature hash, so the signature
commits to the header fields and to every transaction. The nonce and
`nBits` are left out: a miner grinds the nonce after the signer has
signed.

<!-- source: coax/lib/bitcoin/chainparams.coax signet -->
```jai
// What CheckSignetBlockSolution decided: a SIGNET_ code and, for
// SIGNET_INVALID, the script interpreter's error.
SignetCheck :: struct { code: int; error: int; }
// The script flags Core verifies a signet solution with.
SIGNET_FLAGS :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_NULLDUMMY;
// CheckSignetBlockSolution (BIP-325) for a serialized block. The genesis
// block passes. Otherwise the solution in the coinbase's witness
// commitment is a scriptSig and a witness stack that must spend a virtual
// output whose scriptPubKey is the challenge:
//
//   to_spend: version 0, one input with the null outpoint, the scriptSig
//     OP_0 and a push of the block's version, previous hash, a Merkle root
//     and time (72 bytes), sequence 0; one output of 0 to the challenge;
//     locktime 0. The root is the block's, with the coinbase's txid taken
//     after the solution is cleared from it, since the solution cannot sign
//     itself.
//   to_sign: version 0, one input spending to_spend's output 0 with the
//     solution, sequence 0; one output of 0 to OP_RETURN; locktime 0.
//
// ins, outs and items must hold the block's largest transaction; ids
// holds 96 bytes per transaction; scratch holds the coinbase, the
// challenge or the solution with 200 bytes to spare.
chain_signet_check_vartime :: (p: ChainParams, b: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, ids: [] u8, scratch: [] u8, w: [] ScriptWork) -> SignetCheck #export_c {
  hash: [32] u8; root: [32] u8; spend_id: [32] u8;
  if b.count < 80 { return SignetCheck.{code=SIGNET_BAD_BLOCK}; }
  #assert pow_block_hash_vartime(b[0..<80], hash[..]) == POW_OK;
  if chain_hash_is_vartime(hash[..], p.genesis.hash) { return SignetCheck.{}; }
  challenge: [SCRIPT_MAX_SIZE] u8;
  cn := cp_unhex(p.challenge, challenge[..]);
  if cn < 0 { return SignetCheck.{code=SIGNET_CAPACITY}; }
  // The block's transactions and their txids.
  count := tx_compact_read_vartime(b, 80);
  if count.code != TX_OK || count.value == 0 { return SignetCheck.{code=SIGNET_BAD_BLOCK}; }
  if ids.count < 96 * count.value { return SignetCheck.{code=SIGNET_CAPACITY}; }
  n := count.value;
  k := merkle_block_ids_vartime(b, false, ins, outs, items, ids[0..<32*n], ids[32*n..<64*n]);
  if k.code == MERKLE_CAPACITY { return SignetCheck.{code=SIGNET_CAPACITY}; }
  if k.code != MERKLE_OK { return SignetCheck.{code=SIGNET_BAD_BLOCK}; }
  // The coinbase and its commitment.
  cb := b[count.next..<b.count];
  t := tx_parse_vartime(cb, true, ins, outs, items);
  cidx := chain_commitment_index_vartime(cb, t, outs);
  if cidx < 0 { return SignetCheck.{code=SIGNET_NO_COMMITMENT}; }
  if scratch.count < cp_room(t.total) + 200 || scratch.count < cn + 200 { return SignetCheck.{code=SIGNET_CAPACITY}; }
  spk := cb[outs[cidx].script..<outs[cidx].script+outs[cidx].script_n];
  cleared: [SCRIPT_MAX_SIZE] u8;
  if spk.count > SCRIPT_MAX_SIZE { return SignetCheck.{code=SIGNET_CAPACITY}; }
  f := chain_signet_fetch_vartime(spk, cleared[..]);
  #assert f.length >= 0;
  // The solution: a scriptSig, then a witness stack, then nothing.
  sig_at := 0; sig_n := 0; wit_at := 0; wit_n := 0;
  if f.n >= 0 {
    base := outs[cidx].script + f.at;   // the solution's place in the coinbase
    sol := cb[base..<base+f.n];
    s := tx_span_vartime(sol, 0);
    if s.code != TX_OK { return SignetCheck.{code=SIGNET_BAD_SOLUTION}; }
    items_n := tx_compact_read_vartime(sol, s.next);
    if items_n.code != TX_OK { return SignetCheck.{code=SIGNET_BAD_SOLUTION}; }
    at := items_n.next;
    for j: 0..<items_n.value {
      item := tx_span_vartime(sol, at);
      if item.code != TX_OK { return SignetCheck.{code=SIGNET_BAD_SOLUTION}; }
      at = item.next;
    }
    if at != sol.count { return SignetCheck.{code=SIGNET_BAD_SOLUTION}; }
    sig_at = base + s.at; sig_n = s.n;
    wit_at = base + s.next; wit_n = ifx items_n.value == 0 then 0 else sol.count - s.next;
  }
  // The modified coinbase's txid replaces the first leaf.
  m := cp_stripped_with(cb, t, ins, outs, cidx, cleared[0..<f.length], scratch);
  #assert sha256d_checked(scratch[0..<m], ids[0..<32], false) == SHA_OK;
  #assert merkle_root_vartime(ids[0..<32*n], ids[64*n..<96*n], root[..]).code == MERKLE_OK;
  // to_spend, and its txid.
  at := cp_le(scratch, 0, 0, 4);
  scratch[at] = 1; at += 1;
  for i: 0..<32 { scratch[at+i] = 0; }
  at = cp_le(scratch, at + 32, 0xffffffff, 4);
  scratch[at] = 74; scratch[at+1] = OP_0; scratch[at+2] = 72; at += 3;
  at = cp_put(scratch, at, b[0..<36]);
  at = cp_put(scratch, at, root[..]);
  at = cp_put(scratch, at, b[68..<72]);
  at = cp_le(scratch, at, 0, 4);
  scratch[at] = 1; at += 1;
  at = cp_le(scratch, at, 0, 8);
  at = tx_compact_write_vartime(scratch, at, cn);
  at = cp_put(scratch, at, challenge[0..<cn]);
  at = cp_le(scratch, at, 0, 4);
  #assert sha256d_checked(scratch[0..<at], spend_id[..], false) == SHA_OK;
  // to_sign, with the witness form only when the stack has items.
  room := 60 + sig_n + wit_n + 9;
  if scratch.count < room { return SignetCheck.{code=SIGNET_CAPACITY}; }
  at = cp_le(scratch, 0, 0, 4);
  if wit_n > 0 { scratch[at] = 0; scratch[at+1] = 1; at += 2; }
  scratch[at] = 1; at += 1;
  at = cp_put(scratch, at, spend_id[..]);
  at = cp_le(scratch, at, 0, 4);
  at = tx_compact_write_vartime(scratch, at, sig_n);
  at = cp_put(scratch, at, cb[sig_at..<sig_at+sig_n]);
  at = cp_le(scratch, at, 0, 4);
  scratch[at] = 1; at += 1;
  at = cp_le(scratch, at, 0, 8);
  scratch[at] = 1; scratch[at+1] = OP_RETURN; at += 2;
  at = cp_put(scratch, at, cb[wit_at..<wit_at+wit_n]);
  at = cp_le(scratch, at, 0, 4);
  u := tx_read_vartime(scratch[0..<at], true, ins, outs, items);
  if u.code == TX_CAPACITY { return SignetCheck.{code=SIGNET_CAPACITY}; }
  if u.code != TX_OK { return SignetCheck.{code=SIGNET_BAD_SOLUTION}; }
  sp := ScriptSpend.{b=scratch[0..<at], t=u, ins=ins[0..<u.nin], outs=outs[0..<u.nout], items=items[0..<u.nitems],
    index=0, amount=0, spk=challenge[0..<cn]};
  error := script_verify_vartime(sp, SIGNET_FLAGS, w);
  if error != SCRIPT_ERR_OK { return SignetCheck.{code=SIGNET_INVALID, error=error}; }
  return SignetCheck.{};
}
```

The flags are P2SH, WITNESS, DERSIG and NULLDUMMY, so a solution can be a
legacy, P2SH or segwit v0 spend. TAPROOT is not among them, so, as in
Core, a version 1 challenge passes without its signature being checked.
Core's CheckBlock calls this check, and a failure is "bad-signet-blksig".
A block without a solution is checked with an empty scriptSig and witness,
which passes only a challenge such as `OP_TRUE`.

### Worked example: signet block 1

Block 1 (`00000086…533b53`) is 329 bytes: a header and a coinbase with
two outputs. Output 1 is the commitment, 119 bytes:

```text
6a 24 aa21a9ed e2f61c3f…974e8cf9         OP_RETURN, push 36: the witness commitment
4c 4f ecc7daa2                           OP_PUSHDATA1 79: the header
      49                                   a scriptSig of 73 bytes:
        00                                   OP_0, the CHECKMULTISIG dummy
        47 30440220…7e01                     push 71: a DER signature, SIGHASH_ALL
      00                                   no witness items
```

Cleared, the push becomes `04 ecc7daa2`, so the commitment is 43 bytes.
The coinbase's txid changes, and with one transaction it is the root that
`to_spend` commits to. The scriptSig pushes the dummy and the signature,
the challenge pushes 1, two keys and 2, and `OP_CHECKMULTISIG` checks the
signature against the keys until one matches. Block 19,990 has three
spends after the coinbase; its root is the Merkle root of the cleared
coinbase's txid and their three txids.

## The result codes

<!-- source: coax/lib/bitcoin/chainparams.coax codes -->
```jai
// Result codes. Core's reject reason, where there is one, is in quotes.
CHAIN_OK :: 0;
CHAIN_BAD_GENESIS :: 1;      // the built genesis block does not hash to the published hash
CHAIN_BAD_MERKLE :: 2;       // its coinbase does not hash to the published Merkle root
CHAIN_HIGH_HASH :: 3;        // the genesis hash does not meet its nBits under powLimit: "high-hash"
CHAIN_CAPACITY :: 4;         // a buffer is too small, or a parameter does not decode
// Signet's block solution: Core's CheckSignetBlockSolution fails, and
// CheckBlock refuses the block as "bad-signet-blksig".
SIGNET_OK :: 0;
SIGNET_NO_COMMITMENT :: 1;   // the coinbase has no witness commitment
SIGNET_BAD_SOLUTION :: 2;    // the solution does not read as a scriptSig and a witness, or has extra bytes
SIGNET_INVALID :: 3;         // the solution does not satisfy the challenge; error says why
SIGNET_BAD_BLOCK :: 4;       // the block's bytes don't read, or it has no transactions
SIGNET_CAPACITY :: 5;        // a caller's buffer is too small: a limit of ours, not Core's
```

| Code | Core | Returned by |
| --- | --- | --- |
| `CHAIN_BAD_MERKLE`, `CHAIN_BAD_GENESIS` | the asserts in each network's constructor | the genesis check |
| `CHAIN_HIGH_HASH` | "high-hash" | the genesis check |
| `SIGNET_NO_COMMITMENT`, `SIGNET_BAD_SOLUTION` | "block solution parse failure", then "bad-signet-blksig" | the signet check |
| `SIGNET_INVALID` | "block solution invalid", then "bad-signet-blksig" | the signet check, with the script error |
| `SIGNET_BAD_BLOCK`, `SIGNET_CAPACITY`, `CHAIN_CAPACITY` | none: Core has a parsed block and growable buffers | the checks |

## The audit

The library had parts of this chapter in other places. Chapter 48's
`retarget.coax` defined the retargeting fields of four networks; this
chapter adds signet's and holds them in `ChainParams` instead of copying
them. Chapter 26's `B58_VERSIONS`, chapter 28's `SW_HRPS` and chapter 32's
`BIP32_*` versions are the address prefixes; the checks below compare each
network's fields with them instead of replacing them. Chapter 47's
`pow.coax` had mainnet's `powLimit` and the mainnet genesis header as a
test vector. Nothing built a genesis block, derived a message start or
checked a signet block before this chapter.

## The example

The example is
[bitcoin/examples/chain-params.coax](../examples/chain-params.coax). It
prints each network's parameters with its genesis block built and checked,
the mainnet genesis coinbase, signet's challenge and magic, the solutions
of signet blocks 1 and 19,990, five changed copies of block 1, the
network for each magic, and the BIP-30 blocks.

<!-- source: bitcoin/examples/chain-params.coax example -->
```jai
#load "../../coax/lib/bitcoin/chainparams.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A hash in hash order, shown reversed as explorers show it.
put_shown :: (digest: [] u8) {
  r: [32] u8;
  for i: 0..<32 { r[i] = digest[31-i]; }
  put_hex(r[..]);
}
put32 :: (v: u32) {
  b: [4] u8;
  for i: 0..<4 { b[i] = cast,trunc(u8) (v >> (8 * cast(u32) (3 - i))); }
  put_hex(b[..]);
}
put_number :: (a: U256) {
  b: [32] u8;
  #assert u256_to_hash_vartime(a, b[..]);
  put_shown(b[..]);
}
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }
decode :: (text: string, out: [] u8) -> [] u8 {
  n := cp_unhex(text, out);
  #assert n >= 0;
  return out[0..<n];
}
GENESIS_CODES :: [5] string.["ok", "wrong hash", "wrong Merkle root", "high-hash", "does not build"];
SIGNET_CODES :: [6] string.["ok", "no witness commitment", "solution does not read", "solution invalid", "block does not read", "capacity"];

// One network's parameters, and its genesis block built and checked.
show :: (params: ChainParams) {
  p := params;
  print("%: magic ", CHAIN_NAMES[p.chain]); put_hex(p.magic[..]); print(", port %\n", p.port);
  print("  Base58 versions P2PKH %, P2SH %, WIF %; BIP-32 ", p.pubkey_prefix, p.script_prefix, p.secret_prefix);
  put32(p.xpub); print(" and "); put32(p.xprv); print("; bech32 \"%\"\n", p.hrp);
  b: [CHAIN_GENESIS_MAX] u8; hash: [32] u8;
  n := chain_genesis_block_vartime(p.genesis, b[..]);
  #assert pow_block_hash_vartime(b[0..<80], hash[..]) == POW_OK;
  print("  genesis "); put_shown(hash[..]); print(": % bytes, time %, nonce %, nBits ", n, p.genesis.time, p.genesis.nonce);
  put32(p.genesis.bits); print(": %\n", GENESIS_CODES[chain_genesis_check_vartime(p)]);
  print("    Merkle root "); put_shown(b[36..<68]); print("\n");
  print("  powLimit "); put_number(p.retarget.pow_limit); print("\n");
  print("  % s a block, % s a period (% blocks); minimum-difficulty blocks %, BIP-94 %, retargeting %\n",
    p.retarget.spacing, p.retarget.timespan, retarget_interval(p.retarget), yes(p.retarget.allow_min_difficulty),
    yes(p.retarget.bip94), yes(!p.retarget.no_retargeting));
  print("  the subsidy halves every % blocks\n", p.halving);
  print("  heights: BIP-34 %, BIP-65 %, BIP-66 %, CSV %, segwit %\n", p.bip34_height, p.bip65_height,
    p.bip66_height, p.csv_height, p.segwit_height);
  if p.bip34_hash != "" { print("    BIP-34 block %\n", p.bip34_hash); }
  if p.bip16_exception != "" { print("    verified with no flags: %\n", p.bip16_exception); }
  if p.taproot_exception != "" { print("    verified with P2SH and WITNESS only: %\n", p.taproot_exception); }
  if p.min_chain_work == "" { print("  no minimum chain work, no assumed-valid block\n"); }
  else {
    print("  minimum chain work 0x"); put_number(chain_number_vartime(p.min_chain_work)); print("\n");
    print("  assumed valid: block % (%)\n", p.assume_valid_height, p.assume_valid);
  }
}

// What the signet check decides for a block, with the parameters p.
check :: (label: string, p: ChainParams, b: [] u8) {
  ins: [8] TxIn; outs: [8] TxOut; items: [16] TxItem; ids: [96 * 8] u8; s: [2048] u8; w: [1] ScriptWork;
  r := chain_signet_check_vartime(p, b, ins[..], outs[..], items[..], ids[..], s[..], w[..]);
  print("  %: %", label, SIGNET_CODES[r.code]);
  if r.code == SIGNET_INVALID { print(" (%)", SCRIPT_ERR_NAMES[r.error]); }
  print("\n");
}

main :: () {
  for chain: CHAIN_MAIN..<CHAIN_REGTEST + 1 { show(chain_params(chain)); }

  // The mainnet genesis coinbase, byte by byte.
  b: [CHAIN_GENESIS_MAX] u8;
  n := chain_genesis_block_vartime(CHAIN_MAINNET_PARAMS.genesis, b[..]);
  print("the mainnet genesis coinbase, % bytes:\n  ", n - 81); put_hex(b[81..<n]); print("\n");
  print("  scriptSig: push 4 ffff001d (486604799), push 1 04, push 69 \"%\"\n", CHAIN_MAINNET_PARAMS.genesis.message);
  t := chain_genesis_block_vartime(CHAIN_TESTNET4_PARAMS.genesis, b[..]);
  print("testnet4's message is 76 bytes, so its push is OP_PUSHDATA1 76: ");
  put_hex(b[81+42+7..<81+42+9]); print("; its output pays a key of 33 zero bytes\n");

  // Signet: the challenge, its magic, and the solutions of two blocks.
  c: [100] u8; m: [4] u8;
  challenge := decode(SIGNET_DEFAULT_CHALLENGE, c[..]);
  #assert chain_signet_magic_vartime(challenge, m[..]);
  print("signet's challenge, % bytes: OP_1 <", challenge.count); put_hex(challenge[2..<35]); print(">\n  <");
  put_hex(challenge[36..<69]); print("> OP_2 OP_CHECKMULTISIG\n");
  print("  magic: the first 4 bytes of SHA-256d(47 || challenge) = "); put_hex(m[..]); print("\n");
  blocks := [2] string.[SIGNET_BLOCK_1, SIGNET_BLOCK_19990];
  heights := [2] int.[1, 19990];
  k: [1000] u8; cleared: [100] u8; hash: [32] u8;
  for i: 0..<2 {
    blk := decode(blocks[i], k[..]);
    ins: [8] TxIn; outs: [8] TxOut; items: [16] TxItem; ids: [32 * 8] u8; wids: [32 * 8] u8;
    count := merkle_block_ids_vartime(blk, false, ins[..], outs[..], items[..], ids[..], wids[..]).count;
    cb := blk[81..<blk.count];
    tx := tx_parse_vartime(cb, true, ins[..], outs[..], items[..]);
    o := chain_commitment_index_vartime(cb, tx, outs[..]);
    f := chain_signet_fetch_vartime(cb[outs[o].script..<outs[o].script+outs[o].script_n], cleared[..]);
    #assert pow_block_hash_vartime(blk[0..<80], hash[..]) == POW_OK;
    print("block % (", heights[i]); put_shown(hash[..]);
    print("): % bytes, % %\n", blk.count, count, ifx count == 1 then "transaction" else "transactions");
    print("  commitment in output %, % bytes; the solution is % bytes after ecc7daa2\n", o, outs[o].script_n, f.n);
    sol := cb[outs[o].script+f.at..<outs[o].script+f.at+f.n];
    print("  scriptSig % bytes: OP_0 and a % byte signature; % witness items\n", sol[0], sol[2], sol[sol.count-1]);
    print("  cleared, the commitment is % bytes, ending ", f.length); put_hex(cleared[f.length-5..<f.length]); print("\n");
    check("default signet", CHAIN_SIGNET_PARAMS, blk);
  }

  // Refusals, on block 1.
  print("block 1 changed:\n");
  blk := decode(SIGNET_BLOCK_1, k[..]);
  blk[230] ^= 1; check("one bit of the signature's r", CHAIN_SIGNET_PARAMS, blk); blk[230] ^= 1;
  blk[68] ^= 1; check("one bit of the time", CHAIN_SIGNET_PARAMS, blk); blk[68] ^= 1;
  blk[216] = 0x4a; check("a scriptSig length one too long", CHAIN_SIGNET_PARAMS, blk); blk[216] = 0x49;
  blk[215] ^= 1; check("ecc7daa3, so no solution", CHAIN_SIGNET_PARAMS, blk);
  op_true := chain_signet_custom_vartime("51");
  print("  a signet whose challenge is OP_TRUE has magic "); put_hex(op_true.magic[..]); print("\n");
  check("ecc7daa3 on that signet", op_true, blk); blk[215] ^= 1;
  blk[174] ^= 1; check("ab21a9ed, so no commitment", CHAIN_SIGNET_PARAMS, blk); blk[174] ^= 1;
  check("unchanged, on the OP_TRUE signet", op_true, blk);

  // Which network a message is for.
  print("network for magic:");
  for chain: CHAIN_MAIN..<CHAIN_REGTEST + 1 {
    p := chain_params(chain);
    print(" "); put_hex(p.magic[..]); print(" %;", CHAIN_NAMES[chain_from_magic_vartime(p.magic[..])]);
  }
  print(" "); put_hex(op_true.magic[..]); print(" %\n", ifx chain_from_magic_vartime(op_true.magic[..]) == 0 then "none" else "?");

  // BIP-30's blocks, from validation.cpp: 91,842 repeats 91,812's
  // coinbase, and 91,880 repeats 91,722's.
  for i: 0..<2 {
    print("BIP-30: block % repeats the coinbase of block %\n", CHAIN_BIP30_REPEAT_HEIGHTS[i], CHAIN_BIP30_UNSPENDABLE_HEIGHTS[1-i]);
  }
  print("BIP-30 is checked again from height %\n", CHAIN_BIP34_IMPLIES_BIP30_LIMIT);
}
```

Its output for signet:

```text
block 1 (00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53): 329 bytes, 1 transaction
  commitment in output 1, 119 bytes; the solution is 75 bytes after ecc7daa2
  scriptSig 73 bytes: OP_0 and a 71 byte signature; 0 witness items
  cleared, the commitment is 43 bytes, ending 04ecc7daa2
  default signet: ok
...
block 1 changed:
  one bit of the signature's r: solution invalid (EVAL_FALSE)
  one bit of the time: solution invalid (EVAL_FALSE)
  a scriptSig length one too long: solution does not read
  ecc7daa3, so no solution: solution invalid (INVALID_STACK_OPERATION)
  a signet whose challenge is OP_TRUE has magic 54d26fbd
  ecc7daa3 on that signet: ok
  ab21a9ed, so no commitment: no witness commitment
  unchanged, on the OP_TRUE signet: ok
```

A changed time fails because the time is in `to_spend`; a changed nonce
would not. Without a solution the default challenge's `OP_CHECKMULTISIG`
finds too few stack items, while the `OP_TRUE` signet accepts any block,
signed or not.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/chain-params.out](../fixtures/chain-params.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/chain-params.coax](../examples/checks/chain-params.coax),
which loads the example:

- each network's Base58, bech32 and BIP-32 prefixes against chapters 26,
  28 and 32's constants, and a P2WPKH address for the network through
  chapter 29's encoder;
- mainnet's minimum chain work and assumed-valid block against chapter
  48's vectors, which sum the chainwork to that block;
- each network's retargeting fields against Core's sanity checks, its
  genesis block, and its genesis `nBits` against `powLimit`;
- signet block 1, a changed signature, and custom challenges.

<!-- source: bitcoin/examples/checks/chain-params.coax check -->
```jai
// Each network's prefixes are the ones chapters 26, 28, 29 and 32 encode:
// Base58 versions, the bech32 human-readable part, and the BIP-32
// versions. A P2WPKH address for the network starts with its part and "1".
#forall "prefixes agree with the encoding chapters" chain: 1..<6 #steps 100000000 {
  p := chain_params(chain);
  main := p.chain == CHAIN_MAIN;
  #assert p.pubkey_prefix == B58_VERSIONS[ifx main then B58_MAIN_P2PKH else B58_TEST_P2PKH];
  #assert p.script_prefix == B58_VERSIONS[ifx main then B58_MAIN_P2SH else B58_TEST_P2SH];
  #assert p.secret_prefix == B58_VERSIONS[ifx main then B58_MAIN_WIF else B58_TEST_WIF];
  #assert p.xpub == cast(u32) (ifx main then BIP32_XPUB else BIP32_TPUB) && p.xprv == cast(u32) (ifx main then BIP32_XPRV else BIP32_TPRV);
  #assert SW_HRPS[p.network] == p.hrp;
  s: [22] u8; text: [100] u8;
  s[0] = 0; s[1] = 20;
  for i: 2..<22 { s[i] = cast(u8) i; }
  n := address_encode_vartime(p.network, s[..], text[..]);
  #assert n > p.hrp.count + 1 && text[p.hrp.count] == '1';
  for i: 0..<p.hrp.count { #assert text[i] == p.hrp[i]; }
}
// Mainnet's minimum chain work and assumed-valid height are the ones
// chapter 48 summed the chainwork to; each network's retargeting fields
// pass Core's sanity checks; every genesis block checks.
#test "chapter 48's numbers and each genesis" #steps 100000000 {
  #assert CHAIN_MAINNET_PARAMS.min_chain_work == RT_MIN_CHAIN_WORK && CHAIN_MAINNET_PARAMS.assume_valid_height == RT_ASSUME_VALID_HEIGHT;
  for chain: 1..<6 {
    p := chain_params(chain);
    #assert retarget_params_check_vartime(p.retarget) == RETARGET_OK;
    #assert chain_genesis_check_vartime(p) == CHAIN_OK;
    // Each genesis block claims the easiest target its network allows:
    // its nBits is powLimit in compact form, rounded down.
    #assert pow_compact_encode_vartime(p.retarget.pow_limit, false) == p.genesis.bits;
  }
  #assert CHAIN_MAINNET_PARAMS.assume_valid == RT_ASSUME_VALID_HASH;
}
// The two signet blocks pass, each changed byte the example shows refuses,
// and a signet with another challenge has other magic.
#test "signet's challenge" #steps 1000000000 {
  b: [1000] u8; ins: [8] TxIn; outs: [8] TxOut; items: [16] TxItem; ids: [96 * 8] u8; s: [2048] u8; w: [1] ScriptWork;
  n := cp_unhex(SIGNET_BLOCK_1, b[..]);
  #assert chain_signet_check_vartime(CHAIN_SIGNET_PARAMS, b[0..<n], ins[..], outs[..], items[..], ids[..], s[..], w[..]).code == SIGNET_OK;
  b[231] ^= 0x80;
  #assert chain_signet_check_vartime(CHAIN_SIGNET_PARAMS, b[0..<n], ins[..], outs[..], items[..], ids[..], s[..], w[..]).code == SIGNET_INVALID;
  custom := chain_signet_custom_vartime(SIGNET_DEFAULT_CHALLENGE);
  #assert custom.magic[0] == CHAIN_SIGNET_PARAMS.magic[0] && custom.magic[3] == CHAIN_SIGNET_PARAMS.magic[3];
  other := chain_signet_custom_vartime("52");
  #assert other.magic[0] != CHAIN_SIGNET_PARAMS.magic[0] || other.magic[1] != CHAIN_SIGNET_PARAMS.magic[1];
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/chain-params.coax
```

`coax test` runs 346 checks, including the loaded files' own checks, in
about 275 seconds on 2026-11-03.

### The library's own checks

`chainparams.coax` is an `#authority` file. Its checks build each genesis
block and change one field at a time (the nonce, the reward, the message,
the limit, an undecodable script); test the push encoding at every length
boundary; derive signet's magic and look up each network by magic; check
the deployment heights at their edges; check every exception block hash
in both byte orders; clear solutions from crafted commitments, including
a non-minimal push, a four-byte header with no data, an empty
`OP_PUSHDATA1` and a truncated push; and check signet blocks 1 and 19,990,
the genesis block and every refusal.

```shell
coax/bin/coax test coax/lib/bitcoin/chainparams.coax
```

`coax test` runs 277 checks, including those of the files it loads, in
about 215 seconds on 2026-11-03. The two signet checks that verify
signatures take most of that time. Mutation testing: not run (owner speed
order).

### Python

[chainparams.py](../../coax/tests/bitcoin/chainparams.py) reads every
parameter from the pinned `chainparams.cpp` with regular expressions,
the default challenge from `kernel/signet.h`, the BIP-30 blocks from
`validation.cpp` and the buried deployments from `consensus/params.h`.
It compares each one with the fields of the five constants in
`chainparams.coax` and the retargeting constants in `retarget.coax`. It
builds each genesis block in Python, transliterates `signet.cpp` over
chapters 34 and 36's Python references
([tx_reference.py](../../coax/tests/bitcoin/tx_reference.py),
[script_reference.py](../../coax/tests/bitcoin/script_reference.py)),
and writes the transcript the example must print.

```shell
python3 -I coax/tests/bitcoin/chainparams.py --coax coax/bin/coax
```

On 2026-11-03 all 84 lines agree with Python, and the example's output in
native debug and release builds equals them. Changing any one height,
hash, prefix or flag in `chainparams.coax` makes it fail. Wasm was not
run.

## What this chapter leaves out

- The witness commitment's own check: chapter 49.
- The rules that read the deployment heights: chapters 51, 54 and 55.
- DNS seeds and fixed seeds: chapter 69. AssumeUTXO snapshots: chapter 79.
- Command-line options that change parameters: `-signetchallenge` is
  `chain_signet_custom_vartime`; `-signetseednode`, `-testactivationheight`
  and regtest's `-vbparams` are chapter 89's.
- Signet challenges that need Taproot: chapter 44.
- Testnet3 is in the pinned Core but is being retired; testnet4
  replaces it.

## Summary

- A network is its parameters: genesis block, magic, port, prefixes,
  `powLimit`, retarget flags, deployment heights, exception blocks, minimum
  work and assumed-valid block.
- A genesis block is built, not downloaded. Four networks share Satoshi's
  coinbase; testnet4 has its own.
- Testnet3, testnet4 and signet share `tb`; regtest is `bcrt`.
- BIP-34, BIP-65, BIP-66, CSV and segwit are buried at fixed heights;
  P2SH and Taproot apply from genesis with exception blocks.
- Signet's magic comes from its challenge, and each block carries a
  solution that spends a virtual output locked by the challenge.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [36. The script interpreter](ch036-the-script-interpreter.md)
- [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md)
- [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `kernel/chainparams.cpp`,
  `kernel/signet.h`, `signet.cpp`, `consensus/params.h` and
  `validation.cpp` (`IsBIP30Repeat`, `IsBIP30Unspendable`,
  `BIP34_IMPLIES_BIP30_LIMIT`, "bad-signet-blksig"), pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/chainparams.cpp).
- [BIP-325](https://github.com/bitcoin/bips/blob/master/bip-0325.mediawiki)
  (signet), [BIP-90](https://github.com/bitcoin/bips/blob/master/bip-0090.mediawiki)
  (buried deployments), [BIP-94](https://github.com/bitcoin/bips/blob/master/bip-0094.mediawiki)
  (testnet4) and [BIP-30](https://github.com/bitcoin/bips/blob/master/bip-0030.mediawiki).
- Signet blocks 1 and 19,990 from mempool.space, pinned in
  [coax/tests/vectors/upstream/signet/](../../coax/tests/vectors/upstream/signet/00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53.raw)
  and copied into
  [chainparams_vectors.coax](../../coax/tests/bitcoin/chainparams_vectors.coax).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
