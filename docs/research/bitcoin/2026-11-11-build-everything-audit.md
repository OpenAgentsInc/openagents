# Bitcoin from scratch: what it takes to build everything ourselves

Date: 2026-11-11. Status: audit and proposed plan. Nothing here is decided
until the owner accepts a decision in section 4.

This audit answers one question: what does it take to build our own version
of every relevant piece of Bitcoin and Lightning infrastructure, from
scratch, in [Coax](../../../coax/start-here.md)?

It has two inputs:

- [The implementation survey](chatgpt-pro-implementation-research.md), a
  ChatGPT Pro survey of Bitcoin and Lightning implementations. The survey
  assumes you adopt existing projects: Bitcoin Core as a separate process,
  BDK, LDK Node, CLN, electrs. **That assumption is wrong for us.** The
  owner's position is that we build everything. This audit uses the survey
  as a map of what exists and what each project teaches. It never uses it
  as a list of things to adopt.
- Our own work: [*Bitcoin From Scratch*](../../../bitcoin/book/README.md)
  and its [plan](../../../bitcoin/book/PLAN.md), the code under
  `coax/lib/crypto/`, `coax/lib/bitcoin/` and `coax/lib/lightning/`, the
  examples, fixtures, lab, explorer and platform code under `bitcoin/`, and
  the verification records under `docs/verification/bitcoin-*`.

Every statement about our code cites a file. Where a search found nothing,
the audit says "not found". Line counts are `wc -l` at commit `7aa8d77f91`.

## Contents

1. [Summary](#1-summary)
2. [Where we stand](#2-where-we-stand)
3. [The build plan](#3-the-build-plan)
4. [Decisions](#4-decisions)
5. [What to adapt from each project](#5-what-to-adapt-from-each-project)
6. [Risks, non-negotiables and gates](#6-risks-non-negotiables-and-gates)
7. [The first ten steps](#7-the-first-ten-steps)
8. [Appendix: facts this audit corrected](#8-appendix-facts-this-audit-corrected)

## 1. Summary

**We have the library. We don't have the programs.** The book has built
almost every pure piece of Bitcoin and Lightning as checked Coax: the
cryptography, the script interpreter, every signature hash, every consensus
rule, block connection with Core-exact undo, the P2P codecs and state
machines, PSBT, coin selection, MuSig2, FROST, and BOLTs 1, 2, 3, 4, 7, 8,
11 and 12 as codecs and in-process state machines. That is 56,490 lines of
library (34,792 in `coax/lib/bitcoin/`, 13,715 in `coax/lib/crypto/`,
7,983 in `coax/lib/lightning/`) and 61,673 lines of tests
(`coax/tests/bitcoin/`, `coax/tests/lightning/`).

What is missing is everything that turns a library into infrastructure
that holds money:

| Missing | Why it matters |
| --- | --- |
| A node process that syncs mainnet | No chapter of Part XII except 77 is built. The node has synced regtest and signet headers, never mainnet ([ch72](../../../bitcoin/book/ch072-headers-sync-and-block-download.md)). |
| A store that holds mainnet | [Chapter 62's store](../../../bitcoin/book/ch062-a-key-value-store-for-the-chainstate.md) keeps the whole table in memory and every operation is linear in its size. A signet import needed a 100 GB arena and was killed once for memory ([record](../../verification/bitcoin-native-explorer-2026-11-03.md)). |
| Speed | Our signature verification is about 45 times slower than libsecp256k1 ([ch77](../../../bitcoin/book/ch077-faster-verification-the-glv-endomorphism.md)), and Coax has no threads yet (`coax/spec.md` §12). |
| A mempool | [Chapter 82](../../../bitcoin/book/ch082-mempool-acceptance-and-limits.md) holds unvalidated explorer observations only. Chapters 83 to 88 are planned. |
| A wallet | No miniscript (chapter 98), no descriptor wallet (chapter 99), no private descriptors ([ch30](../../../bitcoin/book/ch030-output-descriptors-and-their-checksum.md) is public-only). |
| A Lightning node | The channel state machine runs two nodes in one process, in memory. It has no socket, no disk, no chain watcher and no penalty transaction ([ch109](../../../bitcoin/book/ch109-open-update-and-close-a-channel.md), "What this chapter leaves out"). |
| Taproot Assets | Not found in `coax/` or `bitcoin/`. |
| A signer boundary | Keys live in the process that asks for signatures. No signer process, no persisted signer state. |
| Evidence at the money gate | Chapters 80, 93, 117 and 118 are planned. The constant-time evidence covers two signers, on one build each. |

The rough size of the remaining work is 90,000 to 130,000 lines of Coax
library, plus about 1.5 times that in checks and oracles. The estimate is
in [section 3.9](#39-size-summary). The work is not mainly typing. It is
qualification: mainnet equivalence with Bitcoin Core, crash safety for
channel state, interop with LND, CLN and Eclair, and constant-time evidence
for every secret path.

**Five things decide whether this works:**

1. **One oracle discipline.** Bitcoin Core, libsecp256k1, LND, CLN, Eclair
   and tapd appear only in tests. The book already holds this rule
   ([README](../../../bitcoin/book/README.md), Rules). Keep it without
   exception.
2. **The storage engine.** Everything after chapter 63 depends on a store
   that holds mainnet. Decide it first ([decision 2](#decision-2-utxo-and-block-storage)).
3. **The persistence contract for Lightning.** Decide when a state change
   is durable and who owns a channel before writing the node
   ([decisions 9 and 10](#decision-9-lightning-persistence-semantics)).
4. **The signer boundary.** Put keys behind a process with policy before
   any key holds real money ([decision 8](#decision-8-signer-process-and-signing-policy)).
5. **Three Coax prerequisites.** Worker threads, a coverage-guided fuzzer
   and positional file I/O are language and host work that the Bitcoin
   work depends on ([section 3.1](#31-prerequisites-in-coax-itself)).

**One conflict to resolve first.** Three documents say the product wallet
is Breez's Spark SDK ([AGENTS.md](../../../AGENTS.md), "Bitcoin-only money";
[docs/lightning/README.md](../../lightning/README.md); the book's
[README](../../../bitcoin/book/README.md), "The book and the product
wallet"). The sibling `immortal` repo links the `secp256k1` Rust crate and
requires `bitcoind` and `lightningd` services. "Build everything" changes
all of that. [Decision 0](#decision-0-what-build-everything-covers) asks
the owner to say which of these stay as stopgaps and until which gate.

## 2. Where we stand

### 2.1 How to read the status words

The repository has no index that defines "qualified"
(`docs/verification/` has no README). Each record states its own scope.
This audit uses five words:

| Word | Meaning |
| --- | --- |
| **Vectors** | Passes the published vectors of its BIP, BOLT or RFC, pinned in [`coax/tests/vectors/SOURCES.md`](../../../coax/tests/vectors/SOURCES.md). |
| **Model** | Agrees line for line with an independent Python model or a Python transliteration of Bitcoin Core. |
| **Differential** | Agrees with a real reference implementation on generated or live inputs: libsecp256k1 v0.8.0, or a running Bitcoin Core 32.1 on regtest. |
| **Crash** | Recovers to a complete state from a cut at every byte of every write. |
| **Mutation** | `coax mutate` leaves zero survivors. |

No layer is yet **live** (run against mainnet) or **interop** (run against
another implementation over the wire), with two exceptions: chapter 72's
sync against Core peers on regtest and signet, and the explorer's signet
import.

The pinned oracles are Bitcoin Core `ddea9196` (a `master` revision of
2026-11-02, past v32) for vectors and sources, Core 32.1 binaries for live
regtest runs, `lightning/bolts` at `19146282`, libsecp256k1 v0.8.0, and
Wycheproof at `d17cc89f` ([SOURCES.md](../../../coax/tests/vectors/SOURCES.md);
[ch116](../../../bitcoin/book/ch116-test-vectors-and-their-provenance.md)).

### 2.2 The layer table

Chapter numbers refer to [the plan](../../../bitcoin/book/PLAN.md). File
names without a directory are under `coax/lib/bitcoin/`.

#### Cryptography

| Layer | Built (chapters, files) | Qualified against | Gap |
| --- | --- | --- | --- |
| Hashes, MACs, ciphers | Ch 4–12. `coax/lib/crypto/sha256.coax`, `sha512.coax`, `ripemd160.coax`, `hmac.coax`, `hkdf.coax`, `siphash.coax`, `chacha20.coax`, `poly1305.coax`, `chacha20_poly1305.coax`, `sha3.coax` | Vectors (NIST CAVS, RFC 4231, 5869, 8439), 325 Wycheproof AEAD cases, OpenSSL, native and Wasm | No hardware SHA-256 (SHA-NI, ARMv8). Mainnet sync hashes every block and transaction, so this is a speed gap. |
| secp256k1 field, scalar, group | Ch 13–17, 77. `secp_field.coax` (10×26 limbs), `secp_scalar.coax` (8×32), `secp_group.coax`, `secp_mult*.coax`, `secp_glv.coax`, `secp_encode.coax` | A big-integer oracle, group-law `#forall` checks, libsecp256k1 differentials, 464 Wycheproof ECDSA cases | About 45 times slower than libsecp256k1 (ch 77). No 5×52 field, no safegcd inverse, no exhaustive small-group test. |
| ECDSA | Ch 19–22. `ecdsa_verify.coax`, `ecdsa_sign.coax`, `ecdsa_recover.coax`, `der.coax` | Byte-equal to libsecp256k1 on 2,020 signing rows; 38,088 DER encodings against libsecp256k1's strict and lax parsers; Core `script_tests.json` DER rows | The signer is "for test keys until its independent review is done" (ch 20). |
| Schnorr (BIP-340), batch | Ch 23–25. `schnorr*.coax`, `schnorr_batch.coax` | All 19 BIP-340 vectors; 10,000 signatures verified by libsecp256k1 | Same review gate as ECDSA. |
| MuSig2 | Ch 101. `musig.coax` (746 lines) | Every case of BIP-327's eight vector files; `reference.py` | The secnonce is erased in memory after one use (`musig.coax` 336–343). No persisted nonce state, so a crash and restore can't be proven safe. No Memcheck probe. |
| FROST | Ch 102. `frost.coax` (1,209 lines) | Every case of the draft BIP-445 vector files (revision `ba299bad`) | The DKG is the book's own Pedersen construction (`frost.coax` 15–16), not ChillDKG. No probe. |
| Adaptor signatures | Ch 103. `adaptor.coax` | Six vectors from an independent Python implementation | No published standard to pin. |
| ElligatorSwift, BIP-324 | Ch 66–67. `ellswift.coax`, `v2transport.coax` | All BIP-324 vectors; `reference.py` | None for the codec. Not run on a socket against Core (chapter 118). |
| Noise_XK (BOLT 8) | Ch 105. `coax/lib/lightning/noise.coax` | All 15 BOLT 8 handshake vectors and the message vectors | Never run against LND, CLN or Eclair. |
| Constant time | Ch 3. `secret-flow.coax`; `coax/tests/constant_time.py`; Memcheck probes `coax/tests/bitcoin/ecdsa_sign_secret_probe.sh` and `coax/tests/nostr/schnorr_sign_secret_probe.sh`; timing harness `tools/coax-nostr/timing/` | The source audit handles scalar `u32`, `u64` and `bool` only, no arrays or loops (ch 3, line 358). The probes cover the ECDSA and Schnorr signers, one build, one input. The timing harness ran only on three synthetic controls. | No probe for BIP-32, ECDH, MuSig2, FROST, adaptor signatures, Noise, onion or Lightning key derivation. No timing run on real curve code. Chapter 3 says of its own checks: "None of them approves a key for production use." |
| Entropy | Ch 18. `entropy.coax` | Host entropy with no fallback: it "never substitutes a clock, a counter or a seeded generator" | No startup health test, no second source mixed in. |

#### The validating node

| Layer | Built | Qualified against | Gap |
| --- | --- | --- | --- |
| Script interpreter | Ch 36, 39, 44, 45. `script.coax` (1,665 lines), `locktime.coax`, `taproot.coax` | All 1,263 rows of Core's `script_tests.json`; 4,532 flag sets of `tx_valid.json` and `tx_invalid.json`; 83 Taproot spends against a transliteration of Core | Core's `script_assets_test.json` is generated at test time and is not used (PLAN, ch 44). Scripts are not fuzzed (ch 119). No differential fuzz against Core's interpreter (chapter 117). |
| Signature hashes | Ch 37, 38, 43. `sighash.coax` | All 500 rows of `sighash.json`; BIP-143 and BIP-341 vectors; 30 messages against Core's `TaprootSignatureMsg` | None known. |
| Transactions | Ch 34, 35, 52. `tx.coax`, `txcheck.coax` | Core's `tx_valid.json` and `tx_invalid.json`, byte for byte; fuzzed, 100,000 inputs | A search found no check for a transaction version with the high bit set, the class of the btcd bug ([section 5.12](#512-failure-lessons)). Add it. |
| Consensus rules | Ch 46–57. `merkle.coax`, `pow.coax`, `retarget.coax`, `witness.coax`, `chainparams.coax`, `header.coax`, `sigops.coax`, `subsidy.coax`, `scriptflags.coax`, `validation.coax` | 13 historical blocks (9,496 transactions) against independently supplied coins; 35 live Core 32.1 regtest verdicts; 4 testnet4 and signet cases; 425 of 425 mutants killed ([record](../../verification/bitcoin-ch57-2026-11-04.md)) | "This is not an own-node sync from genesis." Mainnet equivalence is unproven until chapter 80. |
| Chain state | Ch 58–60, 63. `chainstate.coax`, `chainstate-store.coax`, `blockundo.coax`, `chainreorg.coax`, `blockindex.coax`, `coinscache.coax`, `kvwriteback.coax` | A 102-block Core regtest journey: every undo body and the full UTXO MuHash match; both reorg directions against a running Core; 9,136 crash cuts ([ch58](../../verification/bitcoin-ch58-2026-11-04.md), [ch60](../../verification/bitcoin-ch60-2026-11-04.md), [ch63](../../verification/bitcoin-ch63-2026-11-04.md) records) | Regtest scale only. The records claim "no mainnet sync, Wasm result … or money admission". |
| Block and UTXO storage | Ch 61, 62. `blockfiles.coax`, `kvstore.coax`, `kvskip.coax`, `kvstream.coax`, `kvhash.coax` | Imports Core 32.1's 2,501-record regtest file byte for byte; a crash at every byte of every append; 990 mutants killed ([record](../../verification/bitcoin-indexed-store-2026-11-03.md)) | The table is in memory and operations are linear. No file rollover. No pruning (chapter 64). Chapter 62 notes that `fsync` on macOS doesn't flush the drive cache. This store can't hold mainnet. |
| P2P | Ch 65–75. `p2p.coax`, `handshake.coax`, `addrman.coax`, `connman.coax`, `dos.coax`, `sync.coax`, `cmpctblock.coax`, `proxy.coax` | Core's `messages.py`; Core's `addrman_tests`, eviction and headers-sync unit cases; live sync of regtest (2,500 blocks, byte-equal to `getblock`) and signet headers (`coax/tests/bitcoin/sync_live.py`) | State machines, not a running peer manager. No inbound listener in a node process. No transaction relay (chapter 83). No live Tor or I2P. Core's `p2p_*` functional tests have not run against it (chapter 118). |
| Initial block download | Ch 77 only | See secp256k1 above | Chapters 76, 78, 79 and 80 are planned and have no chapter file. |
| Mempool and relay policy | Ch 81 `policy.coax` (standardness); ch 82 `mempool.coax` (181 lines) | Standardness: Core's `test_IsStandard` cases. Mempool: three regtest transactions for fees ([record](../../verification/bitcoin-explorer-mempool-2026-11-03.md)) | `mempool.coax` line 1: "No script or relay acceptance." Chapters 83–86 are planned. |
| Fee estimation | `fees.coax` (fee and rate arithmetic, ch 40) | Python | No estimator (chapter 87). |
| Mining | Block weight, sigop and subsidy rules (ch 53, 54) | Core cases | No block template (chapter 88). Stratum: not found. |
| RPC | Ch 90, partly: read-only explorer REST and JSON-RPC (`explorer-rpc.coax`, `bitcoin/platform/explorer-http.coax`) | 73–75 responses against a Python oracle ([record](../../verification/bitcoin-explorer-api-2026-11-03.md)) | No `sendrawtransaction`, `estimatesmartfee`, `getblocktemplate`, cookie authentication or notifications (chapter 92). |
| Node process | Not found | — | Chapter 89 is planned. |

#### Light client, indexer and explorer

| Layer | Built | Qualified against | Gap |
| --- | --- | --- | --- |
| Light client (BIP-157/158) | Ch 74 `blockfilter.coax` (filters and filter headers); ch 46 Merkle branches; ch 72 headers sync with presync; the proof-of-work-only `lightchain.coax` profile | BIP-158's `testnet-19.json`; Core's Golomb code | The BIP-157 P2P messages are names in a table only (`v2transport.coax` 259–260); chapter 74 lists them as left out. No filter index to serve them (chapter 91 remainder). No client that scans a wallet with filters. |
| Indexer | Ch 91 partly (`txindex*.coax`), ch 125 (`addrindex*.coax`), `validated-index*.coax` | Python replay; 393 and 380 mutants killed; 3,535 and 5,413 crash cuts ([txindex](../../verification/bitcoin-txindex-2026-11-03.md), [address index](../../verification/bitcoin-address-index-2026-11-03.md)) | Exact-script histories exist. Filter and coin-statistics indexes don't. Scale is the store's gap. |
| Electrum protocol server | Not found. Electrs appears only as a test oracle (`coax/tests/bitcoin/explorer-reference.py`) | — | Build it, or decide not to ([decision 14](#decision-14-indexer-shape)). |
| Explorer | [`bitcoin/explorer/`](../../../bitcoin/explorer/README.md) (WebGPU), [`bitcoin/lab/`](../../../bitcoin/lab/README.md) (native and realm), `bitcoin/platform/explorer-node.coax` | 133 comparisons with Core 32.1 and Electrs 0.11.0 on regtest; 63 on frozen signet height 329,338 ([record](../../verification/bitcoin-explorer-reference-2026-11-03.md)) | Proof-of-work-only profile. Bounded at 4,096-block reorgs and a 600,000-entry header tree. Not mainnet. |

#### Wallet and signing

| Layer | Built | Qualified against | Gap |
| --- | --- | --- | --- |
| Keys, seeds, paths | Ch 31–33, 121. `coax/lib/crypto/bip39.coax`, `bip32.coax`, `path.coax`, `oneseed.coax` | BIP-39, BIP-32 (vectors 1–5), BIP-44/49/84/86 vectors | English word list only. No constant-time probe on BIP-32. |
| Addresses | Ch 26–29. `base58.coax`, `segwit.coax`, `address.coax` | Core's key I/O files; BIP-173 and BIP-350 | None known. |
| Descriptors | Ch 30. `descriptor.coax` (755 lines) | BIP-380 to 386 and 389; Core's `descriptor_tests.cpp` | Public descriptors only. No private keys, no miniscript fragments (chapter 30 refuses `and_v` and `older`), no `tr()` script trees beyond the key path. |
| Miniscript | Not found. `coax/lib/nostr/nipkeys.coax` line 24 says "Not built"; `treasury.coax` writes its `or_d` by hand | — | Chapter 98. |
| PSBT | Ch 94. `psbt.coax` (1,451 lines) | All 155 PSBTs of BIP-174, 370, 371 and Core's `rpc_psbt.json`; fuzzed | None known for the format. |
| Coin selection | Ch 95. `coinselect.coax` | 344 selection and 11 waste cases ported from Core; brute force | None known. |
| Wallet engine | Not found | — | Chapter 99: transaction graph, script index, full scan and sync, persistence, reorg handling, fee bumping. |
| Message signing, silent payments, multisig | Ch 96, 97, 100. `bip322.coax`, `silent.coax`, `multisig.coax` | BIP-322, BIP-352 and BIP-67 vectors | Silent-payment scanning needs the node's index. |
| Signer policy | Ch 18 (one signer-request contract), ch 122 `treasury.coax` (the agent signs a treasury PSBT only for a refill under a ceiling and fee cap) | Python; chapter 36's interpreter | Policy runs in the caller's process. No signer process, no persisted signer state, no hardware path. Mutation was not run on chapter 122. |
| Backup and recovery | Ch 121 (a card of the `npub` and two descriptors restores the wallets) | BIP vectors | No gap-limit scan, no encrypted backup format, no restore drill. |

#### Lightning

All Lightning chapters ran natively only, and mutation testing was not run
on chapters 109, 112, 113 and 115 (each chapter's verification section says
so). `coax/lib/lightning/` is not marked as qualified for money anywhere.

| Layer | Built | Qualified against | Gap |
| --- | --- | --- | --- |
| BOLT 1 | Ch 104. `bolt1.coax` | All BOLT 1 BigSize, TLV and `init` vectors | No peer manager, no ping timers, no peer storage messages. |
| BOLT 8 | Ch 105. `noise.coax` | All BOLT 8 vectors | Not on a socket. |
| BOLT 3 | Ch 106–108. `keys.coax`, `shachain.coax`, `commitment.coax` | Every BOLT 3 key, shachain, commitment and HTLC vector, `static_remotekey` and anchors, byte for byte | No zero-fee-HTLC anchors variant check in the state machine. No simple Taproot channels. |
| BOLT 2 | Ch 109. `channel.coax` (1,555 lines) | An independent Python model; chapter 108's builder; chapter 36's interpreter | In memory, in one process. Left out: `option_anchors` in the state machine, `update_fee`, `update_fail_malformed_htlc`, `open_channel2`, splicing, quiescence, `closing_complete`, HTLC expiry timeouts. No persistence. `channel_reestablish` detects lost state (`LNH_BEHIND`, `channel.coax` line 41) but nothing acts on it. |
| BOLT 5 | Not found. BOLT 5 isn't pinned in `SOURCES.md` | — | Chapter 110: chain watching, penalty, HTLC timeout and success claims, anchor fee bumping. |
| BOLT 4 | Ch 111. `onion.coax` | BOLT 4's onion, error and blinded-path vectors, and the onion-message packet vector | No onion-message transport or relay. No attributable failures. |
| BOLT 7 | Ch 112. `gossip.coax` | BOLT 7's vectors; its own signed announcements | Left out: checking the funding output against the UTXO set, `announcement_signatures`, rebroadcast, rate limits, pruning. |
| Pathfinding and scoring | Ch 113. `pay.coax` (1,168 lines) | A Python payer over five in-process nodes | Dijkstra with a uniform liquidity penalty. Left out: learning from success, the MPP timer, shadow routes, blinded paths in invoices, min-cost-flow splitting, trampoline, keysend. |
| BOLT 11 | Ch 114. `invoice.coax` | All 31 BOLT 11 examples; fuzzed | None known. |
| BOLT 12 | Ch 115. `offers.coax` (1,072 lines) | All 53 offer vectors, the signature vectors, 28 payer proofs | `offers.coax` line 11: "no onion message and no payment." The flow isn't built. |
| Watchtowers, splicing, dual funding, async payments, trampoline, LSP | Not found, except feature-bit names in `bolt1.coax` (172–176) | — | All of it. |
| Taproot Assets | Not found in Coax. The sibling `tap-ldk` repo (Rust, on forks of LDK) holds an experimental implementation and pinned fixtures | — | All of it ([section 3.7](#37-taproot-assets)). |
| Validating signer | Not found | — | [Decision 8](#decision-8-signer-process-and-signing-policy). |

#### Verification

| Layer | Built | Gap |
| --- | --- | --- |
| Vector provenance | Ch 116: 316 pins, checked by `check_vectors.py` and `check_pins.py` | BOLT 5 and the bLIPs are not pinned. |
| Fuzzing | Ch 119: a deterministic mutation fuzzer for five parsers, 100,000 inputs each | No coverage feedback. Scripts, descriptors, addresses, blocks and most P2P messages are not fuzzed. Core's `qa-assets` corpus is not used. `coax fuzz` is planned, not built ([docs/lightning/testing.md](../../lightning/testing.md)). |
| Differential tests | Scattered: libsecp256k1 for signers and DER; a live Core for chapters 57–63 | Chapter 117 (a standing harness on generated inputs) is planned. |
| Core's functional tests | Not run | Chapter 118. |
| Formal properties | `#forall` checks per chapter | Chapter 120 is planned. Coax has no model checker ([brainstorm](../../verification/2026-10-27-formal-verification-brainstorm.md), stage V4). |

### 2.3 What the book's plan doesn't cover at all

The plan stops at 125 chapters. These layers from the survey have no
chapter:

- BIP-157 serving and a filter client.
- An Electrum protocol server.
- A signer process and signer policy for Lightning.
- Lightning persistence, failover and backup.
- A Lightning node process on sockets.
- Anchors and fee bumping, `option_simple_close`, simple Taproot channels,
  zero-conf and SCID aliases.
- Onion messages as a transport, the BOLT 12 flow, async payments.
- Interactive transaction construction, dual funding and splicing.
- Watchtowers, trampoline, an LSP.
- Taproot Assets.
- Stratum.
- Wallet backup and the restore drill.
- A node on Utreexo proofs.

[Section 3.8](#38-the-roadmap-as-chapters) proposes chapters for each.

## 3. The build plan

Sizes are rough Coax library lines, without checks. They come from two
anchors: the size of what we have (for example, `channel.coax` is 1,555
lines for an in-memory BOLT 2), and the size of the same layer in a
reference implementation (rust-lightning's `channelmanager.rs`,
`channel.rs`, `channelmonitor.rs` and `router.rs` are 20,621, 18,799,
7,395 and 9,693 lines at commit `e9ce486a4`). Coax in this repository is
denser than Rust, so the estimates sit well below the Rust numbers. Treat
them as planning figures, not commitments.

### 3.1 Prerequisites in Coax itself

These are not Bitcoin work, but the Bitcoin work stops without them.

| # | Build | Why | Size | Qualify |
| --- | --- | --- | --- | --- |
| P1 | Worker threads for pure jobs: a fixed pool that runs a batch of independent checks and joins. `coax/spec.md` §12 says "No job syntax, scheduler integration, atomic API or static race checker exists in the bootstrap yet." | Script checks dominate initial block download. One thread can't sync mainnet in useful time. | Compiler and runtime work | The same verdicts with 1, 2, 4 and 8 workers over a pinned block range; ThreadSanitizer on the generated C. |
| P2 | Positional file I/O in the host: `pread`, `pwrite`, `fdatasync`, `F_FULLFSYNC` on macOS, `fallocate`, directory `fsync`. Today the host offers an append-only file (`coax/platform/store-file-native.h`). | The page store in decision 2. | Small | Fault-injection host that fails, shortens or reorders any call. |
| P3 | A coverage-guided fuzzer (`coax fuzz`): compile with edge counters, keep a corpus, minimize. | Chapter 119's fuzzer has no coverage feedback, so it rarely reaches deep parser states. | Compiler work | It rediscovers a planted bug behind three nested branches within a stated budget. |
| P4 | Sanitizers on the generated C as a standard target. Chapter 119 says Coax's own checks "stand in for AddressSanitizer and UndefinedBehaviorSanitizer". Chapters 57–63 already ran ASan and UBSan. | Release builds drop Coax's checks. | Small | Every money-path unit runs its suite under ASan and UBSan. |
| P5 | A secret type the compiler tracks through arrays and loops. Today "there is no `#secret` language annotation" (`coax/spec.md`), and the audit handles scalars only. | Constant-time evidence for field and scalar code, which is all arrays and loops. | Compiler work | The seeded leaky branch and index fixtures of chapter 3 are refused when they sit inside a loop over limbs. |
| P6 | Wide multiplication: a 64×64→128 multiply the C backend maps to `__int128` or `umulh`. | A 5×52 field and 4×64 scalar, the main speedup over 10×26. | Small | Equal to the 10×26 path on all field checks, native and Wasm (Wasm keeps 10×26). |
| P7 | A deterministic simulation host: virtual clock, virtual sockets, virtual disk with crash and fault injection, one seed. | Lightning and node tests need thousands of reproducible crash and reorder schedules. The 9,136 crash cuts of chapter 60 show the method works at file level. | 2,000–4,000 | A seed replays the same run byte for byte. |

### 3.2 Track A: the validating node

Order matters here. Each row depends on the rows above it unless stated.

| # | Build | Depends on | Size | Qualify |
| --- | --- | --- | --- | --- |
| A1 | **The page store** (new chapter 126): an on-disk hash table of fixed pages for coins, keyed by outpoint, with chapter 62's log as the write-ahead log and chapter 63's cache in front. See decision 2. | P2 | 3,000–5,000 | Chapter 62's method at scale: a crash at every write; a random workload against a reference map; a 200-million-entry load with a stated memory ceiling; byte flips refused. |
| A2 | Block file rollover and pruning (chapter 64) | A1 | 800–1,200 | A pruned node keeps its target, keeps 288 blocks and reorganizes across them, against Core on regtest. |
| A3 | Fast verification: 5×52 field, 4×64 scalar, safegcd inverse, wNAF with precomputed tables, hardware SHA-256 | P6 | 2,500–4,000 | Every existing vector and differential; a new differential of 10 million random and edge inputs against libsecp256k1; exhaustive tests on a small-order curve ([section 5.2](#52-libsecp256k1)). Target: within 3 times libsecp256k1. |
| A4 | Signature cache and parallel script checks (chapter 76) | P1, A3 | 800–1,200 | Same verdicts as one thread on a pinned range; cache keyed by a salted hash. |
| A5 | The node process (chapter 89): configuration, one data directory, a lock file, logs, clean shutdown at any point | A1 | 1,500–2,500 | Kill at random points under P7; restart at the same tip. |
| A6 | The peer manager as a running program: sockets, inbound and outbound, timers, chapters 65–75 wired together | A5 | 2,000–3,000 | Core's `p2p_*` functional tests (chapter 118) and a week on signet next to Core. |
| A7 | assumevalid (chapter 78) | A4 | 300–500 | The same UTXO set with it on and off, on signet and on a mainnet prefix. |
| A8 | **Sync mainnet from genesis** (chapter 80) | A1–A7 | Integration | `gettxoutsetinfo` at a pinned height: `muhash` and `hash_serialized_3` equal Core's. Time, memory and disk recorded beside Core's on the same machine. Once with every script checked. |
| A9 | The differential harness (chapter 117) | — (start now) | 2,000–3,000 of harness | See [section 6.3](#63-how-we-test-consensus-equivalence). |
| A10 | Mempool acceptance (finish chapter 82), relay and orphans (83), replacement (84), packages and anchors (85), eviction and persistence (86) | A6 | 6,000–9,000 | `testmempoolaccept` parity on a pinned corpus; Core's `mempool_*` and `rpc_packages` functional tests. Check the pinned Core revision first: if it carries cluster mempool, chapters 82–86 implement cluster linearization, not the ancestor and descendant limits the plan's wording suggests. |
| A11 | Fee estimation (87) | A10 | 1,000–1,500 | Replay of recorded blocks and mempool entries against `estimatesmartfee`. |
| A12 | Block templates (88) | A10 | 800–1,200 | Core accepts a regtest block mined from our template. |
| A13 | RPC (finish 90), notifications (92), remaining indexes (91) | A8, A10 | 3,000–4,000 | Core's JSON for the same chain; a subscriber sees every block and transaction in order across a reorg and a restart. |
| A14 | **Shadow mode** (93) and **Core's functional tests** (118) | A13 | Harness | The money gate ([section 6.2](#62-gates)). |
| A15 | AssumeUTXO (79) | A8 | 1,000–1,500 | A snapshot with one changed coin refuses; background validation reaches the same hash. |
| A16 | Formal properties (120) | — | Models | See [section 6.4](#64-formal-models-where-they-fit). |

### 3.3 Track B: wallet

| # | Build | Depends on | Size | Qualify |
| --- | --- | --- | --- | --- |
| B1 | Miniscript (chapter 98): types, correctness and malleability properties, the script encoding, the satisfier, `tr()` trees | Ch 30, 36, 44 | 3,000–4,000 | BIP-379's examples; Core's `miniscript_tests.cpp` cases; a differential against Core's `miniscript_stable` fuzz target's logic and against rust-miniscript as a second oracle; every satisfaction passes chapter 36's interpreter. |
| B2 | Private descriptors and descriptor signing: `xprv` in descriptors, hardened steps, a signer that reads a PSBT and a descriptor | Ch 30, 94 | 800–1,200 | Core's `descriptor_tests.cpp` private cases; constant-time probes. |
| B3 | The wallet engine (chapter 99): a transaction graph, a script index with lookahead, canonical-chain resolution with conflicts, change sets, full scan and sync as separate operations | A13 or C1 | 4,000–6,000 | A descriptor wallet on signet through our node survives a reorg that removes a payment; a property test replays random block and mempool event orders and gets one balance; a restore finds every coin. |
| B4 | Fee bumping: RBF replacement and CPFP for our own transactions | B3, A10 | 800–1,200 | Core accepts each bump on regtest. |
| B5 | Backup and the restore drill (new chapter) | B3 | 500–800 | A restore from the card alone, with a fresh node, recovers the balance on signet. |

### 3.4 Track C: light client and indexer

| # | Build | Depends on | Size | Qualify |
| --- | --- | --- | --- | --- |
| C1 | BIP-157: the filter index, the six P2P messages, serving, and a client that checks filter headers across peers and against checkpoints | Ch 74, A6 | 1,500–2,500 | Interop with Core's `-peerblockfilters`; a peer that serves a wrong filter is detected. |
| C2 | The indexer at mainnet scale on the page store: script-hash history, spends, transaction positions | A1, A8 | 2,000–3,000 | Sampled comparison against a reference indexer, as the signet record does, on mainnet. |
| C3 | An Electrum protocol server, if decision 14 says so | C2 | 1,500–2,500 | An unmodified Electrum client syncs a wallet against it. |
| C4 | The explorer on the validated profile | A8 | Small | The existing explorer checks, with the profile label changed from proof-of-work-only to validated. |

### 3.5 Track D: signing

| # | Build | Depends on | Size | Qualify |
| --- | --- | --- | --- | --- |
| D1 | Constant-time evidence for every secret path: Memcheck probes and timing runs for BIP-32, ECDH, MuSig2, FROST, adaptor signatures, Noise, onion and Lightning keys | P5 | Harness | [Section 6.5](#65-constant-time-qualification). |
| D2 | **The signer process**: a separate program that holds keys, speaks a narrow protocol over a Unix socket, and applies policy before it signs | Ch 18, 94 | 2,000–3,000 | The treasury policy of chapter 122 moves behind it with zero mutation survivors; a fuzzed host can't make it sign outside policy. |
| D3 | The Lightning validating signer: signer-side channel state and the rule set of section 5.8 | D2, E2 | 3,000–5,000 | Replay every Lightning test with a hostile node that asks for a revoked commitment, a wrong destination or a low fee; the signer refuses each. |
| D4 | Nonce and counter persistence for MuSig2 and FROST | D2 | 400–800 | A crash between nonce generation and signing never yields two signatures on one nonce. |
| D5 | ChillDKG for FROST | Ch 102 | 1,500–2,500 | The ChillDKG BIP's vectors when it has them; until then, its reference implementation. |

### 3.6 Track E: Lightning

| # | Build | Depends on | Size | Qualify |
| --- | --- | --- | --- | --- |
| E1 | Mutation testing on what exists (chapters 109, 112, 113, 115), and pin BOLT 5 and the bLIPs | — | — | Zero survivors. |
| E2 | **The persistence contract and channel store** ([decision 9](#decision-9-lightning-persistence-semantics)) | P7 | 2,000–3,000 | A crash at every byte of every write, at every protocol step; the node never releases a message before its state is durable. |
| E3 | Complete BOLT 2: anchors in the state machine, `update_fee`, `update_fail_malformed_htlc`, `closing_complete`, HTLC expiry handling, funding transaction construction with the wallet | E2, B3 | 2,500–4,000 | The Python model grows with it; lnprototest-style scripted peers. |
| E4 | **Chain watching and BOLT 5** (chapter 110): the monitor, the penalty transaction, HTLC claims, anchor CPFP, reorg handling | E3, A13, A10 | 4,000–6,000 | On regtest with our node as the only chain source: a revoked commitment is swept; every HTLC outcome resolves; the same after a reorg of each step. |
| E5 | The node on sockets: peer manager, BOLT 8 transport, timers, reconnect | E3 | 1,500–2,500 | First interop: open, pay and close against LND, CLN and Eclair on regtest. |
| E6 | Gossip as a service: funding-output checks against our UTXO set, `announcement_signatures`, queries, rate limits, pruning | E5, A8 | 1,500–2,500 | Sync the mainnet graph; the graph matches a reference node's on a sample. |
| E7 | Scoring and payment lifecycle: a probabilistic liquidity scorer with decay, the MPP timer, retries persisted across restart, shadow routes, blinded paths in invoices | E6 | 3,000–5,000 | Replay of recorded payment attempts; payments over a regtest network of all four implementations. |
| E8 | Onion messages and the BOLT 12 flow: relay with rate limits, invoice requests, blinded payment paths for receiving | E5 | 2,000–3,000 | Pay and be paid by CLN, Eclair and LDK-based nodes with offers. |
| E9 | Backup: an encrypted static backup per channel, peer storage, and the recovery flow that acts on `LNH_BEHIND` | E4 | 800–1,200 | Delete the store, restore from seed and backup, and recover every balance on regtest against each implementation. |
| E10 | Simple Taproot channels (MuSig2 funding, nonce exchange) | E3, D4 | 2,000–3,000 | The simple-taproot-channels vectors; interop with LND and Eclair private channels. |
| E11 | Zero-conf and SCID aliases; interactive transaction construction; dual funding | E3 | 2,500–4,000 | Interop with CLN and Eclair. |
| E12 | Quiescence and splicing | E11 | 3,000–5,000 | Interop with CLN and Eclair; crash at every step. |
| E13 | The LSP: LSPS0, LSPS1, LSPS2 and LSPS5 as server and client | E11 | 2,500–4,000 | The LSPS test vectors; a phone-profile client gets a just-in-time channel on regtest. |
| E14 | Watchtower | E4 | 1,500–2,500 | A tower with only encrypted blobs sweeps a revoked commitment while the node is off. |
| E15 | Async payments and static invoices | E8, E13 | 1,500–2,500 | Interop with an LDK-based node, while the spec settles. |
| E16 | Trampoline | E7 | 1,000–2,000 | Interop with Eclair. |

### 3.7 Taproot Assets

The Taproot Assets protocol is defined by Lightning Labs' draft BIPs and by
`tapd`'s behaviour. There is one production implementation. That makes
`tapd` (and `litd` for channels) the only interop oracle, and the vectors
in the draft BIPs the only published ones.

Our sibling repo `tap-ldk` already holds what a Coax build needs to start:
the pinned vectors under `tap-ldk/fixtures/tap-bips/` and
`tap-ldk/fixtures/lightning-labs/` with provenance in
`tap-ldk/fixtures/manifest.json`, and a Rust reference for each module
under `tap-ldk/crates/tap-ldk-core/src/` (`mssmt.rs`, `proof.rs`,
`tap_vm.rs`, `virtual_psbt.rs`, `simple_taproot_asset_channel.rs`,
`asset_htlc.rs`, `rfq_invoice.rs`). Its README says it is experimental and
not production-ready. Under "build everything", `tap-ldk` becomes a
reference and a second oracle; its fixtures move into
`coax/tests/vectors/` with pins.

| # | Build | Depends on | Size | Qualify |
| --- | --- | --- | --- | --- |
| T1 | The Merkle-sum sparse Merkle tree | Ch 4 | 600–1,000 | `mssmt_tree_proofs.json`. |
| T2 | Assets, TLV encoding, asset and Taproot Asset commitments, the tapscript sibling | T1, ch 42 | 1,500–2,500 | The `bip-tap` vectors; `commitment` test data. |
| T3 | The asset virtual machine: virtual transactions and witness validation | T2, ch 44 | 1,000–1,500 | The `bip-tap-vm` vectors. |
| T4 | Proofs and proof files | T2, ch 46 | 1,500–2,500 | `proof.hex`, `proof-file.hex`; a proof with one changed byte refuses. |
| T5 | Addresses and virtual PSBTs; send and receive on chain | T3, T4, B3 | 2,000–3,000 | The `bip-tap-addr` and `bip-tap-psbt` vectors; send to and receive from `tapd` on regtest. |
| T6 | A universe and proof courier client, then server | T4 | 1,500–2,500 | Sync an asset's issuance from a `tapd` universe. |
| T7 | Asset channels: auxiliary leaves on simple Taproot channels, asset HTLCs, RFQ and the invoice flow | T5, E10, E4 | 4,000–6,000 | `tapchannelmsg` test data; open, pay, close and force-close against `litd`. |

Taproot Assets is last for a reason: T7 needs simple Taproot channels
(E10), chain watching (E4) and the wallet (B3), and the protocol still
moves. T1 to T4 are pure and can start any time.

### 3.8 The roadmap as chapters

The existing 22 planned chapters keep their numbers. The list below adds
chapters from 126. Part numbers continue the book's.

**Milestone 1. Mainnet node.** Chapters 64, 76, 78, 80, 89, 117, plus:

- 126. A page store for the UTXO set.
- 127. A faster field and scalar: 5×52, 4×64 and safegcd.
- 128. The peer manager: sockets, timers and the node's event loop.

**Milestone 2. Node that decides money.** Chapters 82–88, 90–93, 118, 120,
79.

**Milestone 3. Wallet.** Chapters 98, 99, plus:

- 129. Private descriptors and descriptor signing.
- 130. The wallet's transaction graph and change sets.
- 131. Full scan, sync and the restore drill.
- 132. Fee bumping.

**Milestone 4. Signer.**

- 133. The signer process and its protocol.
- 134. Signing policy for on-chain spends.
- 135. Nonce state for MuSig2 and FROST.
- 136. ChillDKG.
- 137. Constant-time evidence for every secret path.

**Milestone 5. Light client and indexer.**

- 138. Serving and using compact block filters (BIP-157).
- 139. The address index at mainnet scale.
- 140. An Electrum protocol server.

**Milestone 6. A Lightning node that holds money.** Chapter 110, plus:

- 141. Lightning persistence: authoritative, reconstructible and
  recovery-only stores.
- 142. Anchors, `update_fee` and fee bumping.
- 143. Channel ownership and fencing.
- 144. The Lightning node: peers, timers and reconnects.
- 145. The validating Lightning signer.
- 146. Channel backups and recovery.
- 147. Interop with LND, Core Lightning and Eclair.

**Milestone 7. A Lightning node on the network.**

- 148. Gossip as a service.
- 149. Scoring and the payment lifecycle.
- 150. Onion messages.
- 151. Paying and being paid with offers.
- 152. Simple Taproot channels.
- 153. Zero-conf and aliases.
- 154. Interactive transaction construction and dual funding.
- 155. Quiescence and splicing.
- 156. The LSP: LSPS0, 1, 2 and 5.
- 157. Watchtowers.
- 158. Async payments.
- 159. Trampoline.

**Milestone 8. Taproot Assets.**

- 160. Merkle-sum sparse Merkle trees.
- 161. Assets and their commitments.
- 162. The asset virtual machine.
- 163. Proofs and proof files.
- 164. Asset addresses and virtual PSBTs.
- 165. Universes and proof couriers.
- 166. Asset channels and RFQ.

**Milestone 9. Mining and research.**

- 167. Stratum V2: the template provider.
- 168. Utreexo proofs (research).

Milestones 1, 3 and 4 run in parallel. Milestone 6 needs milestone 2's
notifications and mempool, milestone 3's wallet and milestone 4's signer.
Until milestone 2 passes its gate, Lightning and wallet work runs on
regtest and signet, with Bitcoin Core as the second verdict, as the plan's
rule already says ([PLAN](../../../bitcoin/book/PLAN.md), "The full node").

### 3.9 Size summary

| Track | Library lines (rough) |
| --- | --- |
| A. Node | 25,000–37,000 |
| B. Wallet | 9,000–13,000 |
| C. Light client and indexer | 5,000–8,000 |
| D. Signing | 7,000–11,000 |
| E. Lightning | 30,000–45,000 |
| T. Taproot Assets | 12,000–19,000 |
| Mining (Stratum V2 template provider) | 2,000–3,000 |
| **Total** | **90,000–136,000** |

For scale: what exists is 56,490 library lines, built with 61,673 lines of
tests. Expect the same ratio or more, because the remaining work is
stateful.

## 4. Decisions

Each decision gives options, the tradeoff and a recommendation. The
recommendations are this audit's; none is an owner decision yet.

### Decision 0: what "build everything" covers

Today's rules name things we didn't build:

- The product wallet is Breez Spark ([AGENTS.md](../../../AGENTS.md);
  [docs/lightning/README.md](../../lightning/README.md)). Its trust model
  is "acceptable for a person's spending wallet with small balances … Not
  acceptable for a treasury" ([breez-spark.md](../../lightning/breez-spark.md)).
- The `immortal` repo's allowlist includes the `secp256k1` Rust crate, and
  its provider requires `bitcoind` and `lightningd`
  (`immortal/AGENTS.md`).
- The C host may link one TLS library (AGENTS.md, "Coax first").

| Option | Tradeoff |
| --- | --- |
| A. Replace all of it at once | Stops shipped payments until milestone 6. |
| B. Keep each as a named stopgap with an exit gate | Honest about what runs today; each stopgap has a date it ends. |
| C. Leave them out of scope for good | Contradicts "build everything". |

**Recommendation: B.** Spark stays the person's spending wallet until our
Lightning node passes gate L3 ([section 6.2](#62-gates)). Treasury and
receiver roles move to our stack first, since Spark was never acceptable
there. Immortal's Rust crates and external daemons are a separate
migration, out of this audit's scope; record the intent in its AGENTS.md.
TLS stays a host library: it encrypts sockets to third-party HTTPS and
`wss://` endpoints, and neither Bitcoin P2P nor Lightning uses it.

### Decision 1: full validation or an assumeutxo bootstrap

| Option | Tradeoff |
| --- | --- |
| A. Always validate from genesis, every script | The strongest claim. Slow on every new machine. |
| B. assumevalid by default, full validation available | Core's default. Skips scripts under a pinned block buried by work; every other rule runs. |
| C. AssumeUTXO by default | Usable in minutes. The snapshot hash is trusted until background validation ends. |

**Recommendation.** Build in this order: A, then B, then C. Chapter 80's
gate runs A once on our own hardware and records the UTXO hashes. That run
is the evidence that makes B and C safe: the assumevalid block and the
snapshot hash we ship are ones **our own node** computed and Core confirmed.
For Pylons and Metaverse OS machines, C is the default, with background
validation on. A node on a snapshot whose background validation hasn't
finished labels itself so, and decides no treasury spend.

### Decision 2: UTXO and block storage

The mainnet UTXO set is on the order of 170 million coins and more than
10 GB in Core's format; check the exact figure with `gettxoutsetinfo` when
chapter 80 pins its height. The workload is point lookups, inserts and
deletes by outpoint, in bursts of thousands per block. Range scans are
needed only for a snapshot dump.

| Option | Tradeoff |
| --- | --- |
| A. Chapter 62's log with the table in memory | Built and crash-qualified. Needs RAM above the set's size, like Gocoin's design, and its operations are linear. Fine for regtest and signet. |
| B. An on-disk hash table of fixed pages, with chapter 62's log as the write-ahead log and chapter 63's cache in front | Point operations cost one page read. Small code. No ordered iteration: `hash_serialized_3` needs an external sort, while MuHash doesn't care about order. |
| C. A log-structured merge tree, as LevelDB | Ordered iteration and good write throughput. Compaction is the hard part to make crash-safe and to test. More code than B. |
| D. Postgres | Reuses the one database we already run. Hundreds of millions of rows churned at block speed is the wrong workload: every coin costs a tuple header, an index entry and WAL. |
| E. Memory-mapped files, as libbitcoin | Fast reads. An I/O error arrives as a signal, write ordering is the kernel's, and `msync` semantics differ by platform. Hard to fault-inject. |

**Recommendation: B, with `pread` and `pwrite` through our own cache, not
`mmap`.** Keep the discipline that chapters 58 to 63 already qualified: a
batch is one checksummed frame, it is committed when `fsync` returns, and
nothing is published before that. The page table is a checkpoint of the
log. Block and undo files stay in Core's format (chapter 61), with rollover.

**On the Postgres rule.** The "one binary, one Postgres" rule comes from
the Immortal relay plan. That file was deleted from this repository in
commit `dabc08102f`; its text (read with
`git show dabc08102f^:docs/nostr/2026-08-03-immortal-relay-build-plan.md`)
governs a Nostr relay, where Postgres is "the store, the transaction
engine, the query engine". It doesn't govern a Bitcoin node. The spirit
does carry over: one process, no broker, no cache tier, no sidecar. Option
B honours it. The node is one binary with one data directory. Postgres
keeps the role it suits: product records, and the server-side Lightning
store of decision 9.

### Decision 3: pruning

| Option | Tradeoff |
| --- | --- |
| A. Archive everywhere | Over 700 GB per node. Serves blocks, rescans and the indexer. |
| B. Prune everywhere | Small. No rescan past the prune point, no indexer, no block serving. |
| C. By role | Each machine pays for what it does. |

**Recommendation: C.** One or two archive nodes run the indexer, the
explorer and BIP-157 serving. Pylons and Metaverse OS machines prune, keep
filters for their own wallets, and take old blocks from our archive nodes
or any peer when a rescan needs them, checking each against its header.

### Decision 4: relay policy

| Option | Tradeoff |
| --- | --- |
| A. Core's policy at the pinned revision, exactly | Our transactions propagate, compact blocks reconstruct, fee estimates match, and Lightning's assumptions (anchors, packages, TRUC) hold. |
| B. Our own stricter policy | A node that filters more sees less of what miners mine: worse fee estimates and slower block reconstruction. |
| C. Our own looser policy | We relay what others drop, to no one. |

**Recommendation: A.** Policy is not consensus
([ch81](../../../bitcoin/book/ch081-policy-is-not-consensus-standardness.md)),
so this is a choice, and the right choice for infrastructure is to match
the network. Expose the same knobs Core does and no others. The survey's
note on Knots is the cautionary case: a policy fork became a consensus
fork.

### Decision 5: Utreexo

| Option | Tradeoff |
| --- | --- |
| A. Build a Utreexo node now | Tiny state. Needs bridge nodes to supply proofs, more bandwidth, and a second commitment structure to keep equivalent to the UTXO set. |
| B. Not now; keep the door open | No new trust or availability dependency. |

**Recommendation: B.** Utreexo solves a disk problem that pruning plus
AssumeUTXO already solves for our machines. Its real cost is who generates
proofs and what happens when peers withhold them, as the survey says of
Utreexod and Floresta. Keep it as research chapter 168. If we build it,
build the bridge on our archive node first.

### Decision 6: the light client's trust model

| Option | Tradeoff |
| --- | --- |
| A. BIP-157 against public peers | No server learns our scripts. Peers can withhold filters or blocks; fetching matched blocks leaks which blocks matter. |
| B. A query server (Electrum-style), ours | Fast. The server sees every script. |
| C. BIP-157 against our own archive nodes first, public peers as cross-check | Our nodes are honest by construction; public peers catch our outage or compromise. |

**Recommendation: C.** A phone or browser client verifies headers and
proof of work itself, takes filters from two or more sources including one
of ours, checks filter headers against each other, and downloads matched
blocks whole. It states what it can't know: it doesn't validate scripts,
and it sees no mempool. Never build BIP-37 bloom filters; the plan already
excludes them.

### Decision 7: descriptors and miniscript in the wallet

| Option | Tradeoff |
| --- | --- |
| A. Fixed script templates per wallet type | Small. Every new policy is new code and a new recovery story. |
| B. Descriptors with miniscript as the only wallet model | One model for single-key, multisig, treasury and Lightning sweep outputs. Recovery data is the descriptor. Miniscript's type system is real work (B1). |

**Recommendation: B.** Chapter 121's restore card already records
descriptors, and chapter 122's treasury writes an `or_d` policy by hand
and says its recovery "needs miniscript". Make the descriptor the unit of
backup: a seed alone doesn't describe a multisig or a timelocked wallet.

### Decision 8: signer process and signing policy

| Option | Tradeoff |
| --- | --- |
| A. Keys in the node process | Simplest. Any memory-safety bug or host compromise is a key compromise. |
| B. A signer process that signs what it's asked | Stops key extraction. A compromised node still asks it to sign a theft. |
| C. A signer process that validates state and applies policy | The node must convince the signer. The signer has its own state to persist and recover. |

**Recommendation: C, in two steps.** Step one is D2: one signer program,
a Unix socket, and on-chain policy (destinations, amounts, change, fees,
sighash types, velocity limits). Chapter 122's treasury rules move there
unchanged. Step two is D3 for Lightning, with the rule set in
[section 5.8](#58-vls). The signer is the same Coax code on every host: a
process on a server, a separate enclave-backed task where the platform
has one. The contract in chapter 18 already treats a local key, a browser
signer and a phone as one request shape.

### Decision 9: Lightning persistence semantics

Label every store. The label is part of each store's API, not a comment.

| Store | Label | Rule |
| --- | --- | --- |
| Channel state: commitments, HTLCs, the peer's revocation secrets (shachain), our commitment number | **Authoritative** | Written and synced **before** the message that depends on it leaves the process. Never restored from a copy older than the newest write. |
| The chain monitor: every counterparty commitment's data needed to punish or claim | **Authoritative** | Same rule. Kept until the channel's outputs are buried 100 blocks deep. |
| The signer's state: per-channel high-water marks, nonce counters | **Authoritative** | Owned by the signer. Advances only. |
| Outbound payment records, preimages we learned | **Authoritative** | A preimage is written before the HTLC is fulfilled upstream. |
| Invoices and offers we issued | **Authoritative**, or derived from a signed blob to avoid a write per request (LND's roadmap idea in the survey) | |
| The gossip graph, scorer state, peer addresses, fee estimates | **Reconstructible** | May be deleted. Loaded without trust. |
| Wallet transaction graph and indexes | **Reconstructible** from the chain and the descriptor | |
| The static channel backup and peer storage blobs | **Recovery-only** | Can close channels with the peer's help. Can never resume one. The API that reads it can't produce a live channel object. |

| Option | Tradeoff |
| --- | --- |
| A. One blob per channel rewritten on every update | Simple. Large writes at every HTLC. |
| B. An append-only log of updates per channel with periodic snapshots | Small synchronous writes. Replay on open. This is chapter 62's design. |
| C. Postgres rows, one transaction per update | Durable and replicated by a system we already run. Needs a client; no Postgres client exists in `coax/lib` today. |

**Recommendation.** One persistence contract with two backends. The
contract is LDK's ([section 5.4](#54-ldk)): an update returns *done* only
after it is durable, or *in progress* with a later completion, and the
channel releases no message until completion. Backend one is chapter 62's
log (option B), for a device, a Pylon or any single machine. Backend two
is Postgres (option C) for our servers, where replication and fencing in
the same transaction matter (decision 10). Backend two needs the Postgres
wire protocol in Coax (startup, SCRAM-SHA-256, the extended query
protocol): about 2,000 to 3,000 lines, and we already have HMAC, PBKDF2
and SHA-256.

### Decision 10: channel ownership and failover fencing

Two processes that both act on one channel will sign two different states
at one commitment number. That loses the channel's funds to the penalty.

| Option | Tradeoff |
| --- | --- |
| A. One node, no failover | Safe. An outage lasts until a person restarts it. |
| B. A standby that takes over on a timeout | A partition makes two owners. |
| C. A fencing epoch checked inside every state write, and by the signer | The old owner's writes fail and its signatures are refused, whatever it believes. |

**Recommendation: C.** One owner per channel. Ownership is a row with an
epoch. Every authoritative write is a compare-and-swap on that epoch in
the same transaction. The signer stores the highest epoch it has seen and
refuses a lower one. A restored replica that is byte-perfect but stale
fails both checks. Run independent channels in parallel freely; never run
two writers on one. On a single machine, the epoch is the exclusive lock
on the data directory plus a counter in the log's header.

### Decision 11: our own LSP

| Option | Tradeoff |
| --- | --- |
| A. Use a third-party LSP | No capital. A dependency on their uptime, fees and view of our users' payments. |
| B. Run our own LSP | Capital, channel management and an always-on node. We already need an always-on node for the treasury and the x402 receiver. |

**Recommendation: B, speaking the LSPS specifications.** OpenAgents users
are agents and phones that can't hold inbound liquidity or stay online.
Build LSPS0, LSPS1 (channel purchase), LSPS2 (just-in-time channels) and
LSPS5 (webhooks) as both server and client, so our clients can also use
another LSP and our LSP can serve another wallet. The earlier history
supports this: [docs/lightning/history.md](../../lightning/history.md)
records that ldk-node "needs channels, inbound liquidity and an LSP".

### Decision 12: watchtowers

| Option | Tradeoff |
| --- | --- |
| A. None; nodes must stay online | Fine for servers. Wrong for phones and Pylons that sleep. |
| B. Our own tower, with encrypted blobs keyed by the breach transaction's ID | The tower learns nothing until a breach. Storage grows with every update. |
| C. A third-party tower protocol | Interop with LND's `wtwire` only; no BOLT exists. |

**Recommendation: B, after E4.** The monitor of E4 already holds
everything a tower needs; a tower is a monitor for someone else, fed
encrypted. Our LSP runs the tower for its clients. Anchor channels need
the tower to hold fee-bump funds or pre-signed variants; design that with
E4, not after.

### Decision 13: BOLT 12 first, and the order of channel features

| Option | Tradeoff |
| --- | --- |
| A. BOLT 11 only at first | Works with every wallet today. No reusable payment codes, no payer proofs, no route privacy for the payee. |
| B. BOLT 12 first, BOLT 11 kept for compatibility | Offers suit agents: one static code, signed invoices, blinded paths. LND's native flow was still open at 0.21.6 (survey), so BOLT 11 must stay. |

**Recommendation: B.** Both codecs exist (chapters 114 and 115). Build
onion messages (E8) straight after first interop (E5).

For channels: open only anchor channels with zero-fee HTLC transactions
from the first real channel. Never ship the legacy commitment format with
money, though the code keeps it for the vectors. Then simple Taproot
channels (E10), which Taproot Assets needs, then splicing (E12). Skip
dual funding as a goal in itself; build interactive transaction
construction because splicing needs it.

### Decision 14: indexer shape

| Option | Tradeoff |
| --- | --- |
| A. A global script-hash index (Fulcrum and Esplora shape) | Answers any address. Tens to hundreds of GB and an archive node. |
| B. A descriptor-scoped tracker (NBXplorer and Electrum Personal Server shape) | Small, works on a pruned node. Answers only for registered wallets. |
| C. A compact index that re-reads blocks (electrs shape) | Small index, slower queries, needs block access. |

**Recommendation: both A and B, for different jobs.** Chapter 125's index
is already shape A in miniature. Run it on the archive nodes for the
explorer and public queries. For our own wallets, agents and Lightning
nodes, use shape B: the wallet engine's script index fed by notifications
(chapter 92), which works on pruned nodes. Build the Electrum protocol
server (C3) only if we want third-party wallets to use our nodes; our own
clients use BIP-157 and our own API.

### Decision 15: how we test consensus equivalence

**Recommendation:** all six layers in
[section 6.3](#63-how-we-test-consensus-equivalence), not a choice among
them. The plan's rule stands and should be quoted in every money path:
"Until chapters 80, 93 and 118 are done, our node decides no money for
OpenAgents on its own."

### Decision 16: which cryptography we write, and how we qualify it

We write all of it. That is already the rule and already mostly done.
What remains:

| Item | Recommendation |
| --- | --- |
| Field and scalar | Keep 10×26 as the reference and Wasm path. Add 5×52 and 4×64 for native (A3). Qualify the fast path against the reference path, not only against vectors. |
| Inverse | Add safegcd with its constant-time variant; keep the addition chain as the oracle. |
| Verification multiply | wNAF with precomputed tables for `G`. Variable time is correct here: inputs are public. |
| Signing multiply | Keep the comb and the fixed ladder (chapter 16). Add scalar blinding on the fixed-base path. |
| Schnorr, ECDSA | Done. Finish the independent review chapter 20 names. |
| MuSig2 | Done as an algorithm. Add nonce-state persistence (D4) and probes (D1). Never offer deterministic nonces with a counterparty-chosen input except through BIP-327's `DeterministicSign` rules. |
| FROST | Done against the draft. Replace the book's DKG with ChillDKG (D5). FROST doesn't hold money before the BIP is final. |
| ChaCha20-Poly1305, Noise_XK | Done. Add probes. |
| Hardware acceleration | SHA-256 only. No assembly: use compiler intrinsics through the C backend, checked against the portable kernel on every vector. |

Constant-time qualification is in [section 6.5](#65-constant-time-qualification).

### Decision 17: mining and Stratum

| Option | Tradeoff |
| --- | --- |
| A. Nothing | We never produce a block. |
| B. Block templates only (chapter 88) | Enough for regtest, signet and tests. |
| C. A Stratum V2 template provider on our node | Lets a miner build its own templates from our node. Needs SV2's framing and its Noise handshake; we have the primitives (secp256k1, ElligatorSwift, ChaCha20-Poly1305, Schnorr). Check the handshake pattern against the SV2 specification before sizing it. |
| D. A pool | A business, not infrastructure. |

**Recommendation: B now, C later, never Stratum V1.** Mining transport is
separate from consensus, as the survey says. Nothing else waits on it.

## 5. What to adapt from each project

**Adapt means port the idea into Coax and keep the project as a test
oracle.** No code is copied; the workspace rule for `projects/` is to
study and port, and several of these licences (Gocoin's, libbitcoin's
AGPL terms) forbid more.

**A fact about our reference clones.** The Bitcoin and Lightning lanes
under the workspace's `projects/` are empty on this machine:
`projects/ldk/repos/`, `projects/lightninglabs/repos/` and
`projects/mutiny/repos/` hold no clones. Their manifests list the repos,
so a lane sync restores them. Paths below marked ✓ were verified, for
rust-lightning in the cargo git cache at commit `e9ce486a4` (2026-01-20)
and for Taproot Assets in `tap-ldk/fixtures/`. All other paths come from
the projects' public layouts and are **unverified until the clones
exist**. Confirm each when you open it.

### 5.1 Bitcoin Core

Repository: [bitcoin/bitcoin](https://github.com/bitcoin/bitcoin). Not in
any manifest. We already pin 155 of its files at `ddea9196`.

| Adapt | Study |
| --- | --- |
| The validation boundaries: what is context-free, contextual, and needs the UTXO set. Our chapters 52, 51 and 57 follow them. | `src/validation.cpp`, `src/consensus/tx_check.cpp`, `src/consensus/tx_verify.cpp`, `src/kernel/` |
| The kernel's API as a statement of what a validator owns | `src/kernel/bitcoinkernel.h` |
| AssumeUTXO: two chainstates, the snapshot's hash in chain parameters, background validation | `doc/design/assumeutxo.md`, `src/node/utxo_snapshot.cpp`, `test/functional/feature_assumeutxo.py` |
| The coins cache's flag states and flush rules | `src/coins.cpp`, `src/txdb.cpp` |
| Parallel script checks and the signature cache | `src/checkqueue.h`, `src/script/sigcache.cpp` |
| The mempool at the pinned revision, including cluster linearization if it is there | `src/txmempool.cpp`, `src/txgraph.cpp`, `src/cluster_linearize.h`, `src/policy/rbf.cpp`, `src/policy/packages.cpp`, `src/policy/truc_policy.cpp` |
| Fee estimation's buckets and decay | `src/policy/fees/` (or `src/policy/fees.cpp` at older revisions) |
| Block assembly | `src/node/miner.cpp` |
| Miniscript | `src/script/miniscript.cpp`, `src/test/miniscript_tests.cpp`, `src/test/fuzz/miniscript.cpp` |
| The wallet's descriptor script-pubkey managers | `src/wallet/scriptpubkeyman.cpp` |
| Filter and coin-statistics indexes | `src/index/blockfilterindex.cpp`, `src/index/coinstatsindex.cpp` |
| **The test framework**: a Python peer and block builder that drives a node over P2P and RPC | `test/functional/test_framework/`, `test/functional/feature_block.py`, `p2p_*.py`, `mempool_*.py` |
| Fuzz targets and their corpus as seeds for ours | `src/test/fuzz/`, and [bitcoin-core/qa-assets](https://github.com/bitcoin-core/qa-assets) |

Chapter 118 runs that framework against our node. That needs our RPC to
answer what the framework calls; list those methods first, since they set
chapter 90's real scope.

### 5.2 libsecp256k1

Repository: [bitcoin-core/secp256k1](https://github.com/bitcoin-core/secp256k1).
Not in any manifest. We pin v0.8.0 as an oracle.

| Adapt | Study |
| --- | --- |
| The 5×52 field with lazy reduction and magnitude tracking | `src/field_5x52_impl.h`, `src/field_5x52_int128_impl.h`, `src/field.h` |
| The 4×64 scalar | `src/scalar_4x64_impl.h` |
| safegcd modular inverse, constant-time and variable-time | `src/modinv64_impl.h`, `doc/safegcd_implementation.md` |
| Verification multiplication (Strauss, wNAF, the endomorphism) | `src/ecmult_impl.h` (already pinned for chapter 77) |
| Constant-time fixed-base and variable-base multiplication, with blinding | `src/ecmult_gen_impl.h`, `src/ecmult_const_impl.h` |
| **Exhaustive tests** on a small-order curve that shares the code | `src/tests_exhaustive.c`, `src/precompute_ecmult.c` |
| **The constant-time test**: mark secrets undefined, run under Memcheck, declassify by hand | `src/ctime_tests.c`, `src/checkmem.h` |
| The group-law proofs in Sage | `sage/` |
| Module layouts for Schnorr, MuSig and ElligatorSwift, and their test suites | `src/modules/schnorrsig/`, `src/modules/musig/`, `src/modules/ellswift/` |

Our probes already follow the `ctime_tests` pattern for two signers. The
exhaustive test is the one we lack and the cheapest strong evidence there
is: run our group and multiplication code on a curve of small order and
check every input.

### 5.3 BDK

Repositories: [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk)
and [bitcoindevkit/bdk_wallet](https://github.com/bitcoindevkit/bdk_wallet).
Not in any manifest; the Mutiny lane lists a fork.

| Adapt | Study |
| --- | --- |
| The transaction graph that holds everything seen, with the canonical set computed against a chain | `crates/chain/src/tx_graph.rs` |
| The local chain as a sparse set of checkpoints, and how an update connects or refuses | `crates/chain/src/local_chain.rs`, `crates/core/src/checkpoint.rs` |
| The keychain script index with lookahead and revealed versus used indexes | `crates/chain/src/indexer/keychain_txout.rs` |
| **Change sets**: every mutation returns a mergeable delta, and persistence is appending deltas | `crates/chain/src/changeset.rs`; in bdk_wallet, `wallet/src/wallet/changeset.rs` |
| **Full scan versus sync**: two request types with different stopping rules | `crates/core/src/spk_client.rs`; [Book of BDK: full scan vs sync](https://bookofbdk.com/cookbook/syncing/full-scan-vs-sync/) |
| Caller-owned signers | [Book of BDK: signing](https://bookofbdk.com/design/signing/) |

Lessons to carry: an example store is not a production store (the survey's
`bdk_file_store` trap), and a sync of known scripts can't prove a restore
found every address. Our chapter 131 must make the restore a full scan
with a stated gap limit.

### 5.4 LDK

Repository: [lightningdevkit/rust-lightning](https://github.com/lightningdevkit/rust-lightning).
Listed in `projects/ldk/manifest.txt`. All paths ✓.

| Adapt | Study |
| --- | --- |
| The architecture: what the library owns and what the host supplies | `ARCH.md` ✓ |
| **The `Persist` contract**: completed versus in-progress, and how progress is gated | `lightning/src/chain/chainmonitor.rs` ✓ (`pub trait Persist`, line 124), `lightning/src/chain/mod.rs` ✓ (`pub trait Watch`, line 296) |
| **The manager and monitor split**: off-chain state in one object, on-chain enforcement data in another, and the rule for which is newer after a crash | `lightning/src/ln/channelmanager.rs` ✓, `lightning/src/chain/channelmonitor.rs` ✓ |
| On-chain claims: packaging, fee bumping, reorg handling | `lightning/src/chain/onchaintx.rs` ✓, `lightning/src/chain/package.rs` ✓ |
| Incremental monitor updates and their consolidation | `lightning/src/util/persist.rs` ✓ (`MonitorUpdatingPersister`, line 591) |
| The signer traits as the boundary for a validating signer | `lightning/src/sign/mod.rs` ✓ (`ChannelSigner` 755, `NodeSigner` 882, `SignerProvider` 1036), `sign/ecdsa.rs` ✓, `sign/taproot.rs` ✓ |
| The probabilistic scorer: liquidity bounds per channel, decay, historical buckets | `lightning/src/routing/scoring.rs` ✓ |
| The router's MPP path construction | `lightning/src/routing/router.rs` ✓ |
| Offers, static invoices and the async receive cache | `lightning/src/offers/` ✓, `lightning/src/onion_message/` ✓, `lightning/src/ln/async_payments_tests.rs` ✓ |
| Interactive transactions and splicing | `lightning/src/ln/interactivetxs.rs` ✓, `lightning/src/ln/funding.rs` ✓, `lightning/src/ln/splicing_tests.rs` ✓ |
| LSPS | `lightning-liquidity/src/lsps0/`, `lsps1/`, `lsps2/`, `lsps5/` ✓ |
| **The tests to port as scenarios**: monitor update failure, reload and force-close | `lightning/src/ln/chanmon_update_fail_tests.rs` ✓, `reload_tests.rs` ✓, `monitor_tests.rs` ✓, `functional_tests.rs` ✓ |
| **The consistency fuzzer**: three nodes, random operations, persistence that fails and restores stale copies | `fuzz/src/chanmon_consistency.rs` ✓, `fuzz/src/full_stack.rs` ✓ |

`chanmon_consistency` is the single most valuable thing to port. Our
simulation host (P7) should run the same idea: random messages, random
crashes, random stale restores, and the invariant that no one loses money.

Our own fork, [OpenAgentsInc/ldk-node](https://github.com/OpenAgentsInc/ldk-node),
has interop harnesses to copy the shape of: `tests/integration_tests_cln.rs`,
`integration_tests_lnd.rs`, `integration_tests_eclair.rs` and
`tests/docker/docker-compose-*.yml`.

### 5.5 Core Lightning

Repository: [ElementsProject/lightning](https://github.com/ElementsProject/lightning).
Not in any manifest.

| Adapt | Study |
| --- | --- |
| **Process separation**: one process per job, and one process per channel | `lightningd/`, `channeld/`, `onchaind/`, `gossipd/`, `connectd/`, `openingd/`, `closingd/` |
| The signer as a process with a wire protocol and per-peer capabilities | `hsmd/`, `hsmd/hsmd_wire.csv`, `hsmd/libhsmd.c` |
| Wire messages generated from the BOLT text | `tools/generate-wire.py`, `wire/` |
| On-chain resolution as its own program | `onchaind/onchaind.c` |
| Min-cost-flow payments | `plugins/renepay/`, `plugins/askrene/` |
| Scripted protocol tests against a node | [rustyrussell/lnprototest](https://github.com/rustyrussell/lnprototest) |
| The database's ordered migrations | `wallet/db.c`, `wallet/migrations.c` |

Lesson: separating the signer process doesn't make it a validating signer.
That is why decision 8 has two steps.

### 5.6 Eclair

Repository: [ACINQ/eclair](https://github.com/ACINQ/eclair). Not in any
manifest. Also [ACINQ/lightning-kmp](https://github.com/ACINQ/lightning-kmp)
for the phone profile.

| Adapt | Study |
| --- | --- |
| **One actor per channel**: one owner, one ordered mailbox | `docs/Architecture.md`, `eclair-core/src/main/scala/fr/acinq/eclair/channel/fsm/Channel.scala` |
| Commitments as pure data with pure transitions | `eclair-core/src/main/scala/fr/acinq/eclair/channel/Commitments.scala`, `channel/Helpers.scala` |
| **Postgres with a lease**: one instance holds the database | `docs/PostgreSQL.md`, `eclair-core/src/main/scala/fr/acinq/eclair/db/pg/PgUtils.scala` |
| Splicing, finalized; and the migration off non-anchor channels | `docs/release-notes/eclair-v0.14.0.md`, `channel/fund/InteractiveTxBuilder.scala` |
| Trampoline | `payment/relay/NodeRelay.scala` |
| The wallet-side engine as a pure state machine with explicit actions | lightning-kmp: `src/commonMain/kotlin/fr/acinq/lightning/channel/` |

Our `channel.coax` is already "a pure state machine". Eclair and
lightning-kmp show how to keep that shape when persistence and I/O arrive:
the machine returns actions (store, send, publish), and a driver runs them
in order, store first.

### 5.7 LND

Repository: [lightningnetwork/lnd](https://github.com/lightningnetwork/lnd).
Listed in `projects/lightninglabs/manifest.txt`.

| Adapt | Study |
| --- | --- |
| **Static channel backups**: what is in one, how it is encrypted, and the recovery protocol | `chanbackup/`, `docs/recovery.md` |
| **Staged migrations** by subsystem, and the difference between a key-value layer on SQL and native tables | `docs/db_migration_guide.md`, `sqldb/`, `channeldb/migration*/` |
| Contract resolution as resolvers per output | `contractcourt/` |
| The breach arbiter and the tower | `contractcourt/breach_arbitrator.go`, `watchtower/blob/justice_kit.go`, `watchtower/wtwire/`, `watchtower/wtclient/` |
| Mission control: learning from payment results | `routing/missioncontrol.go`, `routing/probability_bimodal.go` |
| The HTLC switch and its circuit map for forwards across restarts | `htlcswitch/circuit_map.go` |

Lesson: version every stored record from the first byte and write
migrations as ordered, tested, one-way steps. Our stores already carry
schema bytes (`chainstate-store.coax` `CS_VERSION`, the Lab's schema 2);
keep that for every Lightning record.

### 5.8 VLS

Home: [gitlab.com/lightning-signer/validating-lightning-signer](https://gitlab.com/lightning-signer/validating-lightning-signer);
docs at [vls.tech](https://www.vls.tech/docs/latest/overview/intro/). Not
in any manifest, and `projects/sync.sh` only clones from GitHub.

| Adapt | Study |
| --- | --- |
| **The policy rules**: a named, numbered list a signer enforces | `vls-core/src/policy/`, the policy controls page in the docs |
| Signer-side channel state: what the signer must remember to judge a request | `vls-core/src/channel.rs`, `vls-core/src/node.rs` |
| The signer's own persistence and recovery | `vls-persist/` |

The rules to port, as checks in chapter 145: never sign a commitment
number at or below one already revoked; never reveal a revocation secret
before the next commitment is signed by the peer; only sign a mutual close
to an allowlisted or wallet-derived script; bound fees on every
transaction; require that each forwarded HTLC has an inbound HTLC of at
least its value; bound the total in flight; and limit on-chain spends by
amount and time.

### 5.9 Floresta and Utreexo

Repositories: [getfloresta/Floresta](https://github.com/getfloresta/Floresta),
[utreexo/utreexod](https://github.com/utreexo/utreexod),
[utreexo/utreexo](https://github.com/utreexo/utreexo). Not in any manifest.

| Adapt | Study |
| --- | --- |
| The accumulator: a forest of perfect Merkle trees, add, delete, proof | utreexo: `accumulator/`; Floresta's `rustreexo` dependency |
| A node shaped as chain, wire and wallet libraries | Floresta: `crates/floresta-chain/`, `crates/floresta-wire/`, `crates/floresta-watch-only/` |
| The watch-only wallet with an Electrum interface on a small node | Floresta: `crates/floresta-electrum/` |

Lesson, from the survey: the accumulator's mathematics, the availability
of proofs, and the trust in a hardcoded starting state are three separate
decisions. Utreexod's default skips historical validation. Any Utreexo
work of ours keeps decision 1's rule: we ship only a state our own node
computed.

### 5.10 electrs, Fulcrum and Esplora

Repositories: [romanz/electrs](https://github.com/romanz/electrs),
[Blockstream/electrs](https://github.com/Blockstream/electrs),
[cculianu/Fulcrum](https://github.com/cculianu/Fulcrum),
[spesmilo/electrumx](https://github.com/spesmilo/electrumx). Not in any
manifest; the Mutiny lane lists an electrs fork.

| Adapt | Study |
| --- | --- |
| The compact index: script-hash prefix to block height, then re-read the block | romanz: `doc/schema.md`, `src/index.rs`, `src/status.rs` |
| The full index: history rows, spend edges and a UTXO cache per script | Blockstream: `src/new_index/schema.rs`, `doc/schema.md` |
| Throughput: batched block processing and undo records per block for reorgs | Fulcrum: `src/Storage.cpp`, `src/BlockProc.cpp` |
| The Electrum protocol and its status hash | electrumx: `docs/protocol-methods.rst`, `docs/protocol-basics.rst` |

Chapter 125 already matches the full-index shape at small scale, and it
already keeps reorg-aware active membership. The idea to add from Fulcrum
is per-block undo for the index, so a reorg doesn't rescan.

### 5.11 Neutrino

Repository: [lightninglabs/neutrino](https://github.com/lightninglabs/neutrino).
Not in any manifest. Specifications:
[BIP-157](https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki),
[BIP-158](https://github.com/bitcoin/bips/blob/master/bip-0158.mediawiki).

| Adapt | Study |
| --- | --- |
| Filter-header sync with checkpoints, and resolving a conflict between peers by downloading the block | `blockmanager.go` |
| Query scheduling across peers with ranking and retries | `query/` |
| Header and filter stores as flat files | `headerfs/`, `filterdb/` |

Lesson: a filter client sees no unconfirmed transactions and reveals which
blocks it fetches. A Lightning node on filters learns of a breach only
when it is mined; size `to_self_delay` for that.

### 5.12 Failure lessons

| Failure | Lesson | Our position |
| --- | --- | --- |
| btcd, signed transaction version ([disclosure](https://delvingbitcoin.org/t/disclosure-btcd-consensus-bugs-due-to-usage-of-signed-transaction-version/455)): BIP-68 and BIP-112 comparisons differed from Core for versions with the high bit set | "Implements the BIPs" is not evidence. Test integer boundaries. | `Tx` fields are `int` in `tx.coax`; a search found no explicit check for a version of `0x80000000` or above against BIP-68. Add the case to chapters 39 and 52 and to the differential corpus (step 2). |
| btcd, witness parsing ([PR 1896](https://github.com/btcsuite/btcd/pull/1896)): a wire-level size limit refused a valid block | A parser limit is a consensus rule. | Our limits live in `tx.coax` and `dos.coax`. List every numeric limit in the parsers with the Core constant it copies, and test each at the limit and one past it. |
| Milk Sad ([disclosure](https://milksad.info/disclosure.html)): a seed tool drew from a 32-bit generator | Curve mathematics can't repair a weak seed. | Chapter 18 has no fallback entropy source. Add a startup self-test, mix two sources, and make every key-generation path go through `entropy.coax` with a check that fails the build otherwise. |
| bitcoinjs-lib, Merkle advisory ([GHSA-8vv3-6qpc-gpw6](https://github.com/bitcoinjs/bitcoinjs-lib/security/advisories/GHSA-8vv3-6qpc-gpw6)): duplicate handling kept a matching root | Checking a commitment is not checking validity. | Chapter 46 detects the mutated tree and explains the 64-byte transaction. Keep `merkle_root` and `block_is_valid` as different procedures with different names; a light client says "included", never "valid". |
| bcoin, per the survey: follows the chain without enforcing Taproot | Following the most-work chain is weaker than validating it. | Our `lightchain.coax` profile is exactly this, by design, and says so ("NOT consensus validation"). Keep the L and V profiles unmixable, as `chainstate-store.coax` already requires. |
| LND static channel backups, per the survey | A backup that is intact isn't a state that is current. | Decision 9's recovery-only label. |
| Knots, per the survey: a policy fork that became a consensus change | Policy divergence is a slope. | Decision 4. |

### 5.13 Reference repositories to add

The workspace manifest's format is `owner/repo[@branch] [checkout-name]`,
one per line, with a `#` comment above an entry by convention
(`projects/manifest.txt`, line 1). These lines are **suggested additions**;
this audit did not edit the manifest or run a sync. The slugs come from
the survey's links and public knowledge and were not fetched; the sync
script reports any that fail.

A dedicated `projects/bitcoin/` lane with its own `manifest.txt` and
`sync.sh`, like the `ldk` lane, would fit the workspace's convention
better than the root manifest. The lines are the same either way.

```text
# bitcoin/bitcoin (MIT): Bitcoin Core. The consensus oracle; validation, assumeutxo, mempool, the functional test framework
bitcoin/bitcoin
# bitcoin-core/secp256k1 (MIT): field, scalar, safegcd, ecmult, exhaustive and constant-time tests
bitcoin-core/secp256k1
# bitcoin-core/qa-assets: Core's fuzz corpora, seeds for our fuzzers (large)
bitcoin-core/qa-assets
# bitcoin/bips: the BIP texts and vectors we pin
bitcoin/bips
# bitcoinfuzz/bitcoinfuzz: differential fuzzing across Bitcoin implementations
bitcoinfuzz/bitcoinfuzz
# bitcoindevkit/bdk (MIT/Apache-2.0): bdk_core and bdk_chain: tx graph, local chain, change sets, full scan vs sync
bitcoindevkit/bdk
# bitcoindevkit/bdk_wallet (MIT/Apache-2.0): descriptor wallet, coin selection, tx builder
bitcoindevkit/bdk_wallet
# rust-bitcoin/rust-bitcoin (CC0): primitives; second oracle for encodings
rust-bitcoin/rust-bitcoin
# rust-bitcoin/rust-miniscript (CC0): miniscript types, compiler, satisfier; second oracle for chapter 98 (the survey says the repository moved; follow the redirect)
rust-bitcoin/rust-miniscript
# sipa/miniscript: the reference miniscript compiler and analysis
sipa/miniscript
# btcsuite/btcd (ISC): independent Go validator; consensus-bug history
btcsuite/btcd
# libbitcoin/libbitcoin-database (AGPL with exception): memory-mapped store design. Study ideas only
libbitcoin/libbitcoin-database
# libbitcoin/libbitcoin-node (AGPL with exception): staged "chaser" sync design. Study ideas only
libbitcoin/libbitcoin-node
# piotrnar/gocoin (custom non-commercial licence): in-RAM UTXO design. Study ideas only, never copy code
piotrnar/gocoin
# getfloresta/Floresta (MIT): Utreexo node, watch-only wallet, Electrum interface
getfloresta/Floresta
# utreexo/utreexod (ISC): btcd-derived Utreexo node, bridge and proofs
utreexo/utreexod
# utreexo/utreexo (MIT): the accumulator library
utreexo/utreexo
# lightninglabs/neutrino (MIT): BIP-157/158 light client
lightninglabs/neutrino
# romanz/electrs (MIT): compact Electrum index
romanz/electrs
# Blockstream/electrs (MIT): full index and Esplora REST
Blockstream/electrs blockstream-electrs
# cculianu/Fulcrum (GPL-3.0): high-throughput Electrum server. Study ideas only
cculianu/Fulcrum
# spesmilo/electrumx (MIT): Electrum protocol documentation and reference server
spesmilo/electrumx
# spesmilo/electrum (MIT): Python wallet with its own Lightning, trampoline and watchtower
spesmilo/electrum
# dgarage/NBXplorer (MIT): descriptor-scoped wallet tracker on Postgres
dgarage/NBXplorer
# ElementsProject/lightning (BSD-MIT): Core Lightning. Process separation, hsmd, onchaind, renepay
ElementsProject/lightning cln
# rustyrussell/lnprototest: scripted BOLT conformance tests
rustyrussell/lnprototest
# ACINQ/eclair (Apache-2.0): actor per channel, Postgres lease, splicing, trampoline
ACINQ/eclair
# ACINQ/lightning-kmp (Apache-2.0): wallet-side engine as a pure state machine
ACINQ/lightning-kmp
# stratum-mining/stratum (MIT/Apache-2.0): Stratum V2 reference implementation
stratum-mining/stratum
# stratum-mining/sv2-spec: the Stratum V2 specification
stratum-mining/sv2-spec
# bitcoin-core/HWI (MIT): hardware signer interface
bitcoin-core/HWI
# BlockstreamResearch/bip-frost-dkg: ChillDKG draft and reference
BlockstreamResearch/bip-frost-dkg
# C2SP/wycheproof (Apache-2.0): crypto test vectors we already pin
C2SP/wycheproof
```

Two more actions, neither taken by this audit, and one note:

- Run the existing lane syncs so the listed clones exist:
  `projects/ldk/` (rust-lightning, ldk-node, vss-server,
  lightning-liquidity, rapid-gossip-sync-server),
  `projects/lightninglabs/` (taproot-assets, lnd, lightning/bolts,
  lightning/blips, `Roasbeef/bips@bip-tap`) and `projects/mutiny/`.
- VLS lives on GitLab. `projects/sync.sh` builds only
  `https://github.com/<slug>.git` URLs, so VLS needs a GitHub mirror slug
  or a small extension to the script.
- The LSPS specifications live in `lightning/blips`, which the
  `lightninglabs` lane already lists.

## 6. Risks, non-negotiables and gates

### 6.1 Non-negotiables

1. **No outside cryptography, Bitcoin or Lightning library in anything we
   ship.** Other implementations are oracles in tests only. (Already the
   book's first rule.)
2. **Our node decides no money on its own until gate N3.** (Already the
   plan's rule.)
3. **No authoritative write is acknowledged before it is durable.** A
   message that depends on a state leaves the process only after that
   state's `fsync` or commit returns.
4. **One writer per channel, fenced.** A stale owner can't write and can't
   get a signature.
5. **A recovery-only store can never produce a live channel.**
6. **Secrets never decide a branch, an index or a variable-time call**
   (chapter 3), and no key holds real money until its code path has the
   evidence of section 6.5.
7. **No nonce is ever used twice.** Nonce state is persisted before a
   nonce's public part leaves the signer.
8. **Entropy has no fallback.** If the host can't give entropy, key
   generation fails (chapter 18).
9. **Every file that decides money is `#authority;` with zero mutation
   survivors** (AGENTS.md, "Check first"). Chapters 109, 112, 113, 115,
   119 and 122 record "mutation testing: not run"; that debt is paid
   before any of that code holds money.
10. **Every stored record carries a version, and every migration is
    one-way and tested.**
11. **Tests never move the owner's money**
    ([docs/lightning/README.md](../../lightning/README.md)).
12. **Bitcoin only.** Lightning, on-chain and Taproot Assets. No Cashu,
    no EVM.

### 6.2 Gates

A layer passes a gate only with a dated record in `docs/verification/`.

**Node**

| Gate | Requires | Allows |
| --- | --- | --- |
| N1 | Chapter 80: mainnet from genesis, every script checked once, `muhash` and `hash_serialized_3` equal Core's at a pinned height | Calling it a node. Explorer on the validated profile. |
| N2 | Chapter 117's harness green for 30 days; chapter 118: Core's named functional tests pass, with every skipped test listed and reasoned | Serving wallets and Lightning on signet. |
| N3 | Chapter 93: 90 days of shadow mode beside Core on mainnet with zero differences in header, block and tip verdicts, across at least one Core release; mempool acceptance parity on a pinned corpus | Deciding money without a second verdict from Core. |
| N4 | Standing: shadow mode never stops on at least one machine. A difference halts money decisions and pages a person. | Staying at N3. |

**Wallet and signer**

| Gate | Requires | Allows |
| --- | --- | --- |
| W1 | Chapter 99 on signet through our node, surviving a reorg; restore drill from the card alone | Signet funds. |
| W2 | Signer process with policy, zero mutation survivors; constant-time evidence (6.5) for every path the signer runs; independent review of the signers (chapter 20's condition) | Mainnet funds up to a small stated ceiling. |
| W3 | 90 days at W2 with a weekly restore drill; N3 | Treasury funds, under multisig with keys in separate signers. |

**Lightning**

| Gate | Requires | Allows |
| --- | --- | --- |
| L1 | Persistence contract with a crash at every write at every protocol step; BOLT 5 on regtest with our node as chain source; the consistency fuzzer (5.4) clean for a stated number of runs; zero mutation survivors | Signet channels. |
| L2 | Interop on regtest and signet with LND, CLN and Eclair at pinned versions: open, pay both ways, MPP, cooperative close, force close from each side, breach and penalty, reconnect at every message boundary; validating signer in the loop; backup restore drill | Mainnet channels with a small stated ceiling per channel and in total. |
| L3 | 90 days at L2 on mainnet with no loss and no stuck funds; fencing drill (two owners started on purpose; the stale one is refused); N3 | Replacing the Spark wallet for receiving; larger ceilings. |
| L4 | Per feature (splicing, Taproot channels, LSP, towers): its own interop record against each implementation that ships it, and a crash schedule | Enabling that feature on mainnet. |

**Taproot Assets**

| Gate | Requires | Allows |
| --- | --- | --- |
| T1 | Every pinned vector; proof verification fuzzed against `tapd`'s verdicts | Verifying proofs. |
| T2 | Send and receive against `tapd` on regtest and signet; L2 | On-chain assets on mainnet, small ceiling. |
| T3 | Asset channels against `litd`: open, pay, close, force-close; L3 | Asset channels on mainnet. |

### 6.3 How we test consensus equivalence

Six layers, each catching what the others miss.

1. **Vectors.** Core's JSON and unit-test cases at the pin. Done for
   script, transactions, signature hashes and many others.
2. **A crafted block per rule.** Chapter 57's rule table: every rule has a
   block that breaks it, judged by a running Core on regtest. Done for 35
   cases; extend it until every row has one.
3. **History.** Chapter 80: all of mainnet, plus testnet3, testnet4 and
   signet, which hold blocks mainnet never saw.
4. **Differential fuzzing** (chapter 117, step 2). Generate scripts,
   transactions and blocks with a structure-aware fuzzer; give each to our
   code and to Core's; any difference in verdict **or in reject reason** is
   a bug. Seed it with `qa-assets`. Fuzz the edges the btcd bugs came
   from: integer extremes, parser limits, counts at and past each limit,
   non-canonical encodings. Run it continuously on our own machines.
5. **Core's functional tests** against our node (chapter 118), starting
   with `feature_block.py`, which is a catalogue of consensus edge cases.
6. **Shadow mode** (chapter 93), forever.

Two cautions. First, the pinned oracle for vectors is a `master` revision
while live runs use the 32.1 release; write down for each test which one
judged it, and move both forward together on a schedule. Second,
equivalence is to Core's behaviour including its history: the plan's list
(BIP-30 exceptions, the `CHECKMULTISIG` dummy, `FindAndDelete`,
`SIGHASH_SINGLE` with no output) is right, and chapter 80 is where each
gets its real test.

### 6.4 Formal models where they fit

Coax has no model checker today. Until it has state-machine checking
(stage V4 of the [verification plan](../../verification/2026-10-27-formal-verification-brainstorm.md)),
write small external models as oracles, which AGENTS.md permits
("an independent oracle"). `tap-ldk` already carries TLA+ and Kani
harnesses (`tap-ldk/formal/tla`) as a pattern.

| Model | Property |
| --- | --- |
| The BOLT 2 commitment dance, two parties, lossy channel, crashes | No reachable state where a party holds a signed commitment that pays it more than it is owed while the other can't punish. |
| Persistence and message release | A released message implies a durable state that justifies it. |
| Fencing | Two owners with any interleaving never produce two signatures at one commitment number. |
| BOLT 5 resolution | Every output of every commitment is eventually claimed by its owner or expires to them, under any reorg shorter than the stated depth. |
| The coins cache and flush | Any crash recovers a state equal to some block boundary. (Chapter 63 tests this by exhaustive cuts; a model states why.) |
| MuSig2 and FROST sessions | A nonce is consumed at most once across crashes. |
| The consensus rule table | Each rule maps to one check and one counterexample. (Chapter 120's stated scope.) |

### 6.5 Constant-time qualification

Chapter 3 is honest that its checks "are evidence, not proof" and approve
no key. Before a key holds money, each secret path needs all of these:

1. **The source rule**, compiler-enforced on arrays and loops (P5).
2. **The Memcheck probe** on the generated C at `-O0`, `-O2` and `-O3`,
   with GCC and Clang, on x86-64 and ARM64. Declassifications bind to a
   line, as the ECDSA probe does, never to a procedure name: chapter 24
   records that a name-based exemption accepted a seeded branch.
3. **A disassembly scan** of the secret procedures for conditional
   branches and variable-time instructions (division, and multiplication
   on targets where it is variable-time), extending
   `coax/tests/constant_time.py`.
4. **A timing run** on the real code, fixed against random, on each
   deployment CPU class, with the leaky controls detected in the same run.
   The harness in `tools/coax-nostr/timing/` exists and has only ever run
   on synthetic controls.
5. **Exhaustive tests** on a small-order curve for group and
   multiplication code (5.2).
6. **Erasure checks** that every failure path zeroes its workspace.
7. **An independent review** by a person who did not write the code.

Paths that need this: private-key scalar multiplication, ECDSA and Schnorr
signing, BIP-32 private derivation, ECDH (Noise, onion, BIP-324, silent
payments), MuSig2 and FROST nonce generation and signing, adaptor
signatures, Lightning per-commitment secrets and key derivation, AEAD tag
comparison, and WIF and `xprv` text encoding.

Wasm is a separate question. A browser's JIT gives no timing guarantee.
Treat a Wasm signer as suitable for small balances only, and say so in the
product.

### 6.6 Risk register

| Risk | What goes wrong | Control |
| --- | --- | --- |
| Consensus divergence | We reject a valid block (we stall) or accept an invalid one (we follow a false chain and act on false payments) | Section 6.3; gates N1–N4; a difference in shadow mode halts money decisions. |
| A consensus bug copied from Core | We agree with Core and both are wrong | Out of our hands alone; same exposure as the network. Track Core's security advisories against the pin. |
| Key loss | Funds gone | Descriptor-based restore card (chapter 121); weekly restore drills; multisig treasury with a timelocked recovery key (chapter 122). |
| Key theft | Funds gone | Signer process with policy (decision 8); constant-time evidence; small hot balances with treasury refills under a ceiling. |
| Nonce reuse | Key revealed (chapter 101 shows it) | Non-negotiable 7; D4. |
| Weak entropy | Predictable keys | Non-negotiable 8; self-test; two sources. |
| Lightning state loss | Force-closes at best; if a stale state is broadcast, the whole channel balance is lost to the penalty | Decision 9; `LNH_BEHIND` handling; recovery-only backups. |
| Two owners of a channel | Same as above | Decision 10; the fencing drill in gate L3. |
| Missing a breach | The peer takes funds with an old state | Chain watching on our own node; towers for sleeping nodes; `to_self_delay` sized for the chain source. |
| Fee spikes at force-close | HTLC claims confirm too late; funds lost on timeout | Anchor channels only; package relay and CPFP (A10, E4); a fee reserve the signer protects. |
| Storage corruption | Wrong UTXO set, silently | Checksummed frames (chapter 62); MuHash checked against the chain periodically; shadow mode. |
| Denial of service | A peer exhausts memory or CPU | Chapter 71's limits, in a running node; onion-message rate limits; fuzzing every network parser. |
| Privacy leaks | Our scripts, balances or payment graph exposed | Our own nodes for chain data; BIP-157 over query servers; blinded paths and offers; BIP-324; Tor for the node (chapter 75, untested live). |
| Dependence on one spec implementation | Taproot Assets changes under us | Pin `tapd` versions as the oracle; build T1–T4 first, which are stable; T7 last. |
| Scope | 100,000 lines of stateful code | Milestones with gates; every milestone ships something used (explorer, signet wallet, regtest channels) before the next starts. |
| Speed of our own tools | Mainnet sync too slow to test | P1, A3, A4 are on the critical path, before chapter 80. |

## 7. The first ten steps

Each is one issue. Steps 1, 2, 3, 6 and 9 can start today and in parallel.

### 1. Decide and build the page store for the UTXO set

Write chapter 126 and `coax/lib/bitcoin/pagestore.coax` per decision 2,
with positional file I/O in the host (P2).

Acceptance:

- A random workload of puts, deletes and gets matches a reference map.
- A crash at every write of a recorded workload reopens to the last
  committed batch, as chapter 62's test does.
- A load of 200 million synthetic coins completes within a stated memory
  ceiling of 8 GB or less, with a measured rate of lookups per second.
- Chapters 58, 60 and 63's Core regtest journeys pass unchanged on it.
- `#authority;`, zero mutation survivors.

### 2. Stand up the differential harness against Bitcoin Core (chapter 117)

A standing harness that feeds generated scripts, transactions and blocks
to our code and to a pinned Core, and compares verdicts and reject
reasons.

Acceptance:

- Script: at least 10 million generated cases per night with no
  difference, seeded from `script_tests.json` and Core's `qa-assets`.
- Transactions: the integer-boundary corpus exists and passes, including
  a version of `0x80000000` and above under BIP-68, every parser limit at
  the limit and one past it, and non-canonical CompactSizes.
- A planted difference (one flipped flag) is found within one run.
- Every difference found becomes a pinned regression vector.

### 3. A faster field and scalar, qualified against the slow one

Chapter 127: 5×52 field, 4×64 scalar, safegcd inverse, precomputed tables,
needing wide multiplication (P6).

Acceptance:

- Every existing vector and differential passes on both paths.
- 10 million random and edge inputs agree between the paths and with
  libsecp256k1.
- An exhaustive test on a small-order curve passes for group addition and
  both multiplication paths.
- Verification is within 3 times libsecp256k1 on the same machine,
  measured as chapter 77 measures.
- Wasm keeps the 10×26 path and passes unchanged.

### 4. Worker threads and parallel script checks (P1 and chapter 76)

Acceptance:

- A pinned range of mainnet blocks gives the same verdicts with 1, 2, 4
  and 8 workers.
- ThreadSanitizer is clean on the generated C.
- The signature cache is keyed by a salted hash, and a transaction
  verified at mempool entry is not verified again in its block.
- Throughput scales to at least 6 times on 8 cores.

### 5. Sync signet, then mainnet, from genesis with full validation (chapters 89 and 80)

The node process, on the page store, with the peer manager.

Acceptance:

- Signet: synced from public peers; `gettxoutsetinfo` `muhash` equals
  Core's at a pinned height. A kill at 100 random points resumes and
  reaches the same hash.
- Mainnet: the same at a pinned height, with every script checked, plus
  `hash_serialized_3`.
- Time, memory and disk are recorded beside Core's on the same machine.
- This is gate N1.

### 6. Pay the mutation debt on Lightning and treasury code, and pin BOLT 5

Acceptance:

- `coax mutate` leaves zero survivors on `channel.coax`, `gossip.coax`,
  `pay.coax`, `offers.coax` and `treasury.coax`, with no mutant marked
  equivalent to pass.
- BOLT 5 and the bLIPs we use are pinned in `SOURCES.md`, and
  `check_pins.py` passes.
- Each chapter's "mutation testing: not run" line is replaced by the
  result.

### 7. Write the Lightning persistence contract and the channel store (chapter 141)

Acceptance:

- A document table labels every Lightning store authoritative,
  reconstructible or recovery-only, and the types enforce it: the
  recovery-only reader can't construct a live channel.
- `channel.coax` returns actions (store, send, publish) and the driver
  runs store before send.
- Under the simulation host, a crash at every byte of every write at
  every step of open, add, fulfill, fail, reestablish and close recovers
  to a state from which both in-process nodes finish with the right
  balances.
- A restore of any older snapshot is detected and refuses to send.

### 8. Chain watching and BOLT 5 on regtest (chapter 110)

Until gate N1, the chain source is Bitcoin Core on regtest behind the
notification interface of chapter 92, so this step doesn't wait for the
node.

Acceptance:

- Anchor commitments are in the state machine, with `update_fee`.
- A revoked commitment broadcast on regtest is swept by the penalty
  transaction, including its HTLC outputs.
- Each HTLC outcome (timeout, success, both directions, on either
  commitment) resolves to the right owner.
- Each scenario passes again with a reorg at each step.
- A fee spike scenario confirms the claim by CPFP on the anchor.

### 9. The signer process with on-chain policy (chapters 133 and 134)

Acceptance:

- A separate binary holds keys and serves signing requests over a Unix
  socket; the wallet and treasury code hold no private key.
- Chapter 122's refill rules run inside the signer: a payment, an
  overfill, a high fee and a foreign coin are each refused.
- A fuzzed client sends 1 million requests and gets no signature outside
  policy.
- Memcheck probes pass for every signing path the signer runs, with
  line-bound declassifications.
- `#authority;`, zero mutation survivors.

### 10. First interop: open, pay and close against LND, CLN and Eclair on regtest (chapter 147)

Acceptance:

- A harness in Coax (with the implementations as untrusted oracle
  processes on our own machine, never the owner's Mac) starts each
  implementation at a pinned version.
- Against each: BOLT 8 handshake, `init`, open an anchor channel, pay a
  BOLT 11 invoice in both directions, forward a payment across three
  nodes, cooperative close.
- Against each: a disconnect at every message boundary recovers through
  `channel_reestablish`.
- Every wire difference found is recorded with the BOLT text it comes
  from.
- This starts gate L2; force-close and breach scenarios follow from step
  8.

## 8. Appendix: facts this audit corrected

The brief's assumptions and what the checks found:

| Assumed | Found |
| --- | --- |
| The book has 108 chapters | 125 chapters in the plan and registry (`bitcoin/book/chapters.json`): 100 implemented, 3 partly implemented, 22 planned. 103 chapter files exist; no planned chapter has one. |
| The Immortal relay plan is at `docs/nostr/2026-08-03-immortal-relay-build-plan.md` | Deleted in commit `dabc08102f`. Read from history. Its rules cover a Rust Nostr relay. It doesn't mention custody, bitcoind or Lightning. |
| `projects/ldk/repos`, `projects/lightninglabs/repos` and `projects/mutiny/repos` are local clones | The three directories are empty on this machine. The lane manifests list the repos. |
| The payment rule excludes Spark | [AGENTS.md](../../../AGENTS.md) lists Spark as an allowed rail and names Breez Spark as the first wallet. See decision 0. |
| We have Taproot Assets or tap-ldk work under `docs/` | No Taproot Assets code or design in this repository beyond protocol enums and rail names (`docs/protocol/openagents-protocol-spec.md`). The work is in the sibling `tap-ldk` repo, in Rust. |
| MDK is current | MDK appears only in history ([docs/lightning/history.md](../../lightning/history.md)) and in a deprecation note. |

Things this audit could not check and says so:

- Every upstream path in section 5 not marked ✓.
- The exact size of the mainnet UTXO set today.
- Whether the pinned Core revision carries cluster mempool.
- The Stratum V2 handshake details.
- Whether `tx.coax` already handles a high-bit transaction version as Core
  does; only that no test for it was found.
