# Plan and prerequisites

This plan gives the order of the book, what you have at the end of each part,
and, for every chapter, its dependencies, what "done" means and the
specifications it implements. Chapters 1–4, 8, 9, 11, 13–16, 18, 23, 24 and
27 moved here from [Nostr From Scratch](../../nostr/book/README.md), whose
chapters 1–13, 15 and 18 they were; their rows keep Nostr's done criteria.
The other chapters are new; each row says whether it is implemented or
planned.

The book ends in a full Bitcoin node of our own, written in Coax. It
validates every consensus rule, keeps the chain state, syncs mainnet from
genesis, relays transactions and blocks, keeps a mempool, and answers the
RPCs wallets and Lightning call. Bitcoin Core is the reference. Our node must
reach the same verdict as Core on every block. [The full node](#the-full-node)
below says which parts build it and how it earns trust.

## What you need first

- The Coax language: [Coax book](../../coax/book/README.md) Parts I–III.
- A built compiler: `coax/build.sh`.
- No outside cryptography, Bitcoin or Lightning library. libsecp256k1,
  Bitcoin Core and other implementations appear only in tests and in
  comparisons (Parts XII, XIV and XVIII), never in the code the book builds.

## The order

| Part | Chapters | What you have at the end |
| --- | --- | --- |
| I. Foundations | 1–3 | Bytes, big integers and the constant-time rules every later chapter follows. |
| II. Hashes, MACs and ciphers | 4–12 | Every hash, MAC and cipher Bitcoin and Lightning use, each checked against its published vectors. |
| III. secp256k1 | 13–18 | The curve from field arithmetic to scalar multiplication, point encodings and keys. |
| IV. Signatures | 19–25 | ECDSA as Bitcoin uses it, BIP-340 Schnorr signatures and batch verification. |
| V. Encodings and addresses | 26–30 | Base58Check, Bech32 and Bech32m, addresses and output descriptors. |
| VI. Keys from words | 31–33 | BIP-39 seed phrases, BIP-32 hierarchical keys and the standard derivation paths. |
| VII. Transactions and script | 34–40 | Transactions, their IDs, the script interpreter, signature hashes, timelocks and fees. |
| VIII. Taproot | 41–45 | The key tweak, script trees, the Taproot signature hash and tapscript. |
| IX. Blocks and consensus | 46–57 | Merkle trees, headers and proof of work, the chain parameters, every consensus rule with the chapter that checks it, and the UTXO set. |
| X. Chain state and storage | 58–64 | Connecting and disconnecting blocks, the block index, reorganizations, block files, a key-value store of our own, the coins cache and pruning. |
| XI. The network | 65–75 | P2P messages, BIP-324 encrypted transport, the handshake, peer discovery and connections, denial-of-service limits, headers sync and block download, compact blocks, filters, and Tor and I2P. |
| XII. Initial block download | 76–80 | From genesis to the tip: parallel script checks, faster verification, assumevalid, AssumeUTXO, and a mainnet sync whose UTXO set matches Bitcoin Core's. |
| XIII. The mempool and policy | 81–88 | Standardness, mempool acceptance, transaction relay, replacement, packages, eviction, fee estimation and block templates. |
| XIV. Operating the node | 89–93 | The node as a program: configuration and shutdown, the RPC and REST interface, indexes, notifications, and shadow mode beside Bitcoin Core. |
| XV. Wallets | 94–99 | PSBTs, coin selection, message signing, silent payments, miniscript and a descriptor wallet on our node. |
| XVI. Many signers | 100–103 | Multisig, MuSig2, FROST threshold signatures and adaptor signatures. |
| XVII. Lightning | 104–115 | Lightning built in Coax on our node: messages, the Noise handshake, channels, onions, gossip, payments, invoices and offers. |
| XVIII. Interop and verification | 116–120 | Test vector provenance, differential tests, Bitcoin Core's functional tests against our node, fuzzing and formal properties. |
| XIX. Bitcoin in OpenAgents | 121–124 | Where OpenAgents uses the stack: our node on Pylons and Verse OS, one seed, agent treasuries and paid requests. |

Each part uses only earlier parts, except where a chapter's dependencies say
otherwise. Nostr From Scratch needs Parts I–V.

## The full node

Until 2026-11-02 the plan stopped at a bounded block-validation profile
(the old chapter 50) and said it was not a node. Parts I–VIII and the first
chapters of Parts IX and XI (Merkle trees, headers, retargeting, the witness
commitment, the UTXO set, P2P messages, BIP-324, compact blocks and filters)
are the pieces a node is made of, but they are not a node. A node also
needs:

| Missing piece | Where it is now |
| --- | --- |
| Chain parameters for mainnet, testnet4, signet and regtest | Chapter 50 |
| Every consensus rule, not a profile: time and version rules, buried deployments, checks outside script, sigop and weight limits, the subsidy, script flags by height and their historical exceptions | Chapters 51–57 |
| Connecting and disconnecting blocks with undo data, the block index, reorganizations | Chapters 58–60 |
| Storage: block files, our own key-value store (Core uses LevelDB), the coins cache with crash-safe flushes, pruning | Chapters 61–64 |
| Peers: the handshake and feature negotiation, the address manager, outbound and inbound connection management, denial-of-service limits, Tor and I2P | Chapters 68–71 and 75 |
| Initial block download: headers sync, parallel block download, the signature cache, parallel script checks, faster verification, assumevalid, AssumeUTXO | Chapters 72 and 76–80 |
| The mempool and relay policy: standardness, acceptance, relay, RBF and TRUC, packages, eviction, fee estimation, block templates | Chapters 81–88 |
| Operation: the process, JSON-RPC and REST, indexes, notifications | Chapters 89–92 |
| Evidence that it agrees with Bitcoin Core | Chapters 80, 93, 117 and 118 |

Wallets (Part XV) and Lightning (Part XVII) then run on our node instead of
on a third-party chain source.

**What "a full node" means here.** Chapter 80 is the milestone. The node
syncs mainnet from genesis over the public network. Its UTXO set at a
pinned height must equal Bitcoin Core's: `gettxoutsetinfo` gives the same
`hash_serialized_3` and `muhash`. Chapter 93 runs the node in shadow mode
beside Core. Chapter 118 runs Core's own functional tests against it.

**Consensus is a compatibility problem.** A second implementation is
dangerous in one way: if it disagrees with Core about one block, it splits
from the network. Reimplementing the rules is not enough. The node must also
reproduce Core's historical behaviour, including its bugs: the BIP-30
exception blocks, the `OP_CHECKMULTISIG` dummy element, `FindAndDelete`,
`SIGHASH_SINGLE` with no matching output, and the script flag exceptions.
Four rules follow:

- Every consensus rule is in chapter 57's rule table. Each row names the
  chapter that checks the rule and a block that breaks it.
- Bitcoin Core is pinned to one release in chapter 116. Policy that changes
  between releases (standardness, mempool limits, `OP_RETURN` size) follows
  that release and says so.
- Until chapters 80, 93 and 118 are done, our node decides no money for
  OpenAgents on its own. A wallet or Lightning decision checks Core's
  verdict as well.
- The node is not file-compatible with Core's chainstate. Chapter 61 reads
  Core's `blk*.dat` files so you can import them. Everything else uses our
  own formats.

**What stays out.** The GUI, Core's built-in wallet (Part XV builds a
descriptor wallet instead), and the legacy BIP-37 bloom-filter service.
Chapter 75 covers Tor and I2P. CJDNS, UPnP and NAT-PMP are left out until
there is a need.

## Every chapter

### Part I. Foundations

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) — implemented | coax:5, coax:6 | Every RFC 4648 test vector for base64 (`coax/tests/vectors/upstream/rfc/rfc4648.txt`) passes, and every non-canonical form refuses. Hex and base64 round trips hold for every length up to the profile limit in a `#forall` check. `coax mutate` leaves zero survivors on the codec files. | RFC 4648 |
| [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) — implemented | 1, coax:13 | Limb operations match the big-integer oracle on edge values: 0, 1, `p - 1`, `2^256 - 1` and values at each magnitude bound. A `#forall` check confirms no intermediate exceeds its declared bound. Native and wasm builds give the same bytes. | — |
| [3. Constant-time rules and secrets](ch003-constant-time-rules.md) — implemented | 1, 2, coax:23 | A leaky branch fixture and a leaky index fixture are each detected by the timing harness. The constant-time checker refuses a seeded secret-dependent branch with its diagnostic. Every secret buffer in Parts II–V is erased on success and on every failure path. | — |

### Part II. Hashes, MACs and ciphers

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [4. SHA-256](ch004-sha-256.md) — implemented | 1 | Every NIST CAVS 11.0 vector in `coax/tests/vectors/upstream/nist/SHA256ShortMsg.rsp` and `SHA256LongMsg.rsp` passes. Every split of a 1,000-byte message gives the same digest. The compression kernel moves out of the compiler-fingerprint module without changing a digest. | FIPS 180-4 |
| [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md) — implemented | 4 | The txids of pinned mainnet transactions and the genesis block hash match their published values. The tag hashes for `BIP0340/challenge`, `TapLeaf`, `TapBranch`, `TapTweak` and `TapSighash` match the BIP texts. | BIP-340, BIP-341 |
| [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md) — implemented | 1, 4 | The test vectors in the RIPEMD-160 paper pass. HASH160 of BIP-32 vector 1's master public key gives its published fingerprint. | RIPEMD-160 (Dobbertin, Bosselaers and Preneel, 1996) |
| [7. SHA-512](ch007-sha-512.md) — implemented | 1, 4 | Every NIST CAVS vector in `SHA512ShortMsg.rsp` and `SHA512LongMsg.rsp` passes. | FIPS 180-4 |
| [8. HMAC-SHA256](ch008-hmac-sha256.md) — implemented | 4 | RFC 4231 SHA-256 cases 1–7 (`coax/tests/vectors/upstream/rfc/rfc4231.txt`) pass. A tag that differs in one bit is refused by the constant-time comparison. | RFC 2104, RFC 4231 |
| [9. HKDF](ch009-hkdf.md) — implemented | 8 | RFC 5869 SHA-256 cases 1–3 (`coax/tests/vectors/upstream/rfc/rfc5869.txt`) pass. An expand request of `255 * 32 + 1` bytes refuses. | RFC 5869 |
| [10. SipHash-2-4](ch010-siphash-2-4.md) — implemented | 1, 3, 5 | The 64 vectors of the SipHash reference implementation pass. | SipHash (Aumasson and Bernstein, 2012) |
| [11. ChaCha20](ch011-chacha20.md) — implemented | 1 | RFC 8439 sections 2.3.2 and 2.4.2 vectors (`coax/tests/vectors/upstream/rfc/rfc8439.txt`) pass. A request that would pass counter `2^32 - 1` refuses. | RFC 8439 |
| [12. Poly1305 and ChaCha20-Poly1305](ch012-poly1305-and-chacha20-poly1305.md) — implemented | 11, 3 | RFC 8439 sections 2.5.2 and 2.8.2 vectors pass. A tag that differs in one bit refuses and releases no plaintext. | RFC 8439 |

### Part III. secp256k1

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md) — implemented | 2, 3 | Every operation matches the big-integer oracle on edge and random values. Inverse and square root use fixed chains, checked by the constant-time harness. Zero mutation survivors in `secp_field.coax`. | SEC 2 |
| [14. Scalars mod n](ch014-scalars-mod-n.md) — implemented | 2, 3 | Scalar operations match the big-integer oracle. Out-of-range keys and `s` values refuse, each with its own status. | SEC 2, BIP-340 |
| [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md) — implemented | 13 | Group laws hold on random points: associativity, `P + (-P) = O`, `2P = P + P`. `lift_x` refuses every x whose `x^3 + 7` is not a square. | SEC 1, SEC 2 |
| [16. Scalar multiplication](ch016-scalar-multiplication.md) — implemented | 14, 15, 3 | All three paths give the oracle's point for random and edge scalars (0, 1, `n - 1`). The constant-time paths pass the timing harness with leaky controls detected. | SEC 2 |
| [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md) — implemented | 15 | Compressed, uncompressed and x-only encodings round-trip for random points. Hybrid encodings (`0x06`, `0x07`) and points off the curve refuse. | SEC 1, BIP-340 |
| [18. Keys and randomness](ch018-keys-and-randomness.md) — implemented | 14, 3, 16 | A key drawn on each host is a valid scalar, and a forced out-of-range draw is retried. A signer reply with the wrong ID, pubkey or body is refused. | BIP-340 |

### Part IV. Signatures

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [19. Verify ECDSA](ch019-verify-ecdsa.md) — implemented | 5, 14, 16, 17 | Every case in Wycheproof's `ecdsa_secp256k1_sha256_bitcoin_test.json` gives its expected result. A high-S signature refuses under the low-S rule. | SEC 1, BIP-62, BIP-146 |
| [20. Sign with ECDSA](ch020-sign-with-ecdsa.md) — implemented | 19, 8, 18, 3 | RFC 6979 nonces match published secp256k1 vectors. Every signature is low-S and verifies in an independent library. A secret-probe run finds no secret-dependent branch. | RFC 6979, BIP-62, BIP-146 |
| [21. Strict DER](ch021-strict-der.md) — implemented | 19, 20 | Every DER case in Bitcoin Core's `script_tests.json` that BIP-66 decides gives its expected result. | BIP-66 |
| [22. Public key recovery and signed messages](ch022-public-key-recovery-and-signed-messages.md) — implemented | 19, 20, 17, 26, 6, 1 | Bitcoin Core `signmessage` signatures for pinned keys verify, and recovery returns the signing key. A recovery ID out of range refuses. | Bitcoin Core `signmessage` |
| [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md) — implemented | 4, 14, 16 | All 19 BIP-340 vectors in `coax/tests/vectors/upstream/bip340/test-vectors.csv` give their expected result. Fast and reference paths agree on every vector and on random signatures. | BIP-340 |
| [24. Sign with Schnorr](ch024-sign-with-schnorr.md) — implemented | 23, 18, 3 | The BIP-340 signing rows (secret key, aux and message given) produce the expected signatures byte for byte. A secret-probe run finds no secret-dependent branch in the emitted signer. An independent library verifies 10,000 signatures from the Coax signer. | BIP-340 |
| 25. Batch verification — planned | 23 | A batch of the valid BIP-340 vectors verifies; a batch with one invalid signature refuses. Batch and single verification agree on random batches. | BIP-340 |

### Part V. Encodings and addresses

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [26. Base58Check](ch026-base58check.md) — implemented | 5 | BIP-32 extended keys and pinned P2PKH and P2SH addresses decode and re-encode exactly. A one-character change fails the checksum. | Bitcoin Core `base58.cpp` |
| [27. Bech32](ch027-bech32.md) — implemented | 1 | Every valid and invalid BIP-173 Bech32 vector in `coax/tests/vectors/upstream/bip173/bip-0173.mediawiki` gives its expected result. A Bech32m string refuses (BIP-350's strings, pinned for chapter 28). | — |
| [28. Bech32m](ch028-bech32m.md) — implemented | 27 | Every valid and invalid BIP-350 vector gives its expected result, including the witness-version rules. | BIP-350 |
| [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](ch029-addresses.md) — implemented | 6, 17, 26, 28 | Every BIP-173 and BIP-350 segwit address vector and pinned P2PKH and P2SH addresses map to their scriptPubKeys and back. | BIP-13, BIP-141, BIP-173, BIP-341, BIP-350 |
| 30. Output descriptors and their checksum — planned | 29, 32 | The BIP-380 checksum vectors and the BIP-381–386 examples parse and give their stated scripts. | BIP-380 to BIP-386 |

### Part VI. Keys from words

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [31. Seed phrases](ch031-seed-phrases.md) — implemented | 4, 7, 8 | The BIP-39 English vectors give their entropy, mnemonic and seed through PBKDF2-HMAC-SHA512. A phrase with a bad checksum refuses. | BIP-39, RFC 8018 |
| [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md) — implemented | 6, 16, 31 | BIP-32 test vectors 1–5 give every extended key, and vector 5's invalid keys refuse. | BIP-32 |
| [33. Derivation paths](ch033-derivation-paths.md) — implemented | 29, 32 | The BIP-44, BIP-84 and BIP-86 vectors give their addresses, and NIP-06's path gives the Nostr book's chapter 46 key. | BIP-44, BIP-84, BIP-86, NIP-06 |

### Part VII. Transactions and script

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md) — implemented | 1, 3, 5 | Every transaction in Bitcoin Core's `tx_valid.json` and `tx_invalid.json` parses, and serialization round-trips byte for byte with and without witnesses. | BIP-144 |
| [35. Txids, wtxids and malleability](ch035-txids-wtxids-and-malleability.md) — implemented | 34 | Txids and wtxids of pinned segwit transactions match. Changing a witness changes the wtxid and leaves the txid. Each third-party change to a pinned legacy spend (encoding, `n − s`, push form, dummy, extra item) gives a new txid, and Core's four changed forms of `23b397ed` are rebuilt byte for byte. | BIP-141 |
| [36. The script interpreter](ch036-the-script-interpreter.md) — implemented | 6, 19, 21, 34 | Every case in Bitcoin Core's `script_tests.json` gives its expected result under its stated flags. | Bitcoin Core `script_tests.json` |
| [37. Legacy signature hashes](ch037-legacy-signature-hashes.md) — implemented | 34, 36 | Every case in Bitcoin Core's `sighash.json` gives its digest, including `SIGHASH_SINGLE` with no matching output. | Bitcoin Core `sighash.json` |
| [38. Segwit v0 signature hashes](ch038-segwit-v0-signature-hashes.md) — implemented | 5, 34, 36 | BIP-143's examples give their digests for every hash type. | BIP-143 |
| [39. Timelocks](ch039-timelocks.md) — implemented | 36 | The `CHECKLOCKTIMEVERIFY` and `CHECKSEQUENCEVERIFY` cases in `script_tests.json` and `tx_valid.json` pass, and BIP-68 sequence encodings round-trip. | BIP-65, BIP-68, BIP-112, BIP-113 |
| [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md) — implemented | 34 | Weight and vsize of the pinned mainnet transactions match their published values (mempool.space, rust-bitcoin), and their fees match mempool.space's. The sigop-adjusted vsize follows Bitcoin Core's `GetVirtualTransactionSize`, and pre-signing estimates of the pinned P2WPKH and key-path P2TR spends equal their signed weights. | BIP-141 |

### Part VIII. Taproot

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md) — implemented | 5, 24 | BIP-341's `wallet-test-vectors.json` key-path tweaks and signatures match. | BIP-341 |
| 42. Script trees: tapleaves and tapbranches — planned | 41 | BIP-341's script-tree vectors give every leaf hash, Merkle root and output key. | BIP-341 |
| 43. Taproot signature hashes — planned | 34, 41 | Every BIP-341 key-path and script-path signature hash vector matches, for every hash type. | BIP-341 |
| 44. Tapscript — planned | 36, 43 | Bitcoin Core's Taproot script tests (`script_assets_test.json`) give their expected results. | BIP-342 |
| 45. Control blocks and script-path spends — planned | 42, 44 | Script-path spends from BIP-341's vectors verify. A control block with the wrong parity or path refuses. | BIP-341 |

### Part IX. Blocks and consensus

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) — implemented | 3, 5, 34 | Merkle roots of pinned mainnet blocks match their headers. A block whose last transaction is duplicated is detected (CVE-2012-2459). | CVE-2012-2459 |
| [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md) — implemented | 2, 3, 5 | The genesis header and pinned headers hash below their targets; a header with one changed bit does not. `nBits` decodes and encodes as Bitcoin Core's `SetCompact` and `GetCompact` on Core's cases, and the work of each pinned header matches. | Bitcoin Core `pow.cpp`, `arith_uint256.cpp` |
| [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md) — implemented | 47 | Chapter 47 builds the compact form and the work of one header; this chapter adds the retarget and the sum. `nBits` round-trips for every mainnet retarget to a pinned height (973,728), computed retargets match the chain, and chainwork to block 966,143 matches Bitcoin Core's `nMinimumChainWork`, the `getblockheader` chainwork of its `defaultAssumeValid` block. | Bitcoin Core `pow.cpp`, BIP-54, BIP-94 |
| 49. The witness commitment — planned | 35, 46 | The coinbase commitments of pinned segwit blocks match their computed witness roots. | BIP-141 |
| 50. Chain parameters: mainnet, testnet4, signet and regtest — planned | 5, 36, 47, 48 | Each network's genesis block hashes to its published hash, and its message start, default port, address prefixes and deployment heights match Bitcoin Core's `chainparams.cpp` at the pinned release. A signet block whose solution fails the network's challenge refuses. | BIP-94, BIP-325, Bitcoin Core `chainparams.cpp` |
| 51. Header rules: time, versions and deployments — planned | 47, 48, 50 | Median time past and the two-hour future limit decide pinned headers as Bitcoin Core does, and testnet4's timewarp rule refuses its counterexample. The buried deployments (BIP-34, BIP-65, BIP-66, CSV, segwit and Taproot) activate at the chain parameters' heights, and a header whose version is below an active deployment's minimum refuses. | BIP-9, BIP-90, BIP-94, BIP-113 |
| 52. Transaction checks outside script — planned | 34, 35, 39, 40 | Every case in `tx_invalid.json` that fails before the script interpreter refuses with its reason: no inputs or outputs, a value out of range, an overflowing total (CVE-2010-5139), duplicate inputs (CVE-2018-17144), a coinbase spent before 100 confirmations and a non-final transaction. | BIP-68, BIP-113, CVE-2010-5139, CVE-2018-17144 |
| 53. Sigops, weight and block limits — planned | 36, 40 | Legacy, P2SH and witness signature-operation costs of pinned transactions and blocks match Bitcoin Core. A block over 4,000,000 weight units or 80,000 sigop cost refuses. | BIP-16, BIP-141 |
| 54. The subsidy and the coinbase — planned | 34, 50 | The subsidy at every halving matches, and is zero from height 6,930,000. A coinbase that claims one satoshi more than subsidy plus fees refuses, and so does a coinbase without its BIP-34 height after activation. | BIP-34, BIP-42 |
| 55. Script flags by height and their exceptions — planned | 36, 44, 50, 51 | The script verification flags for every block of a pinned mainnet range match Bitcoin Core's, including the BIP-16 and Taproot exception blocks, and the two blocks exempt from BIP-30 are handled as Core handles them. | BIP-16, BIP-30, BIP-341, Bitcoin Core `validation.cpp` |
| 56. The UTXO set and MuHash3072 — planned | 2, 11, 34 | MuHash3072 of pinned UTXO sets matches Bitcoin Core's `gettxoutsetinfo muhash`. Adding then removing an element restores the hash. | Bitcoin Core `muhash.cpp` |
| 57. Validate a block: every consensus rule — planned | 43, 45, 46, 49, 51, 52, 53, 54, 55, 56 | Pinned mainnet, testnet4 and signet blocks, among them every soft-fork activation block, validate against the outputs they spend. A crafted block for each consensus rule refuses with Bitcoin Core's reject reason, and the chapter's rule table names the chapter that checks every rule. | BIP-30, BIP-34, BIP-141, BIP-341, BIP-342, Bitcoin Core `validation.cpp` |

### Part X. Chain state and storage

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 58. Connect and disconnect a block — planned | 56, 57 | Connecting then disconnecting each block of a pinned mainnet range restores the UTXO set's MuHash, using only the undo data the connect step wrote. Undo data with a wrong spent output refuses. | Bitcoin Core `validation.cpp` |
| 59. The block index and the most-work chain — planned | 48, 51 | A header chain from genesis to a pinned height selects the most-work tip; a fork with less work loses. A header that fails a header rule is marked invalid with all its descendants. | Bitcoin Core `validation.cpp` |
| 60. Reorganizations and invalid chains — planned | 58, 59 | On regtest a fork with more work replaces the tip: the node disconnects to the fork point and connects the new branch, and its UTXO set matches Bitcoin Core's after the same reorganization. A fork that contains an invalid block never becomes the tip. | Bitcoin Core `validation.cpp` |
| 61. Block files and undo files — planned | 34, 58 | Blocks and undo data are appended to files and read back by position, and the node imports Bitcoin Core's `blk*.dat` files. A file cut mid-record at any byte is detected at start and its damaged tail dropped. | Bitcoin Core `blockstorage.cpp` |
| 62. A key-value store for the chainstate — planned | 1, 5 | Reads, writes, deletes and ordered iteration match a reference map on random workloads. A crash at every write of a recorded workload restarts to exactly the last committed batch. | — |
| 63. The coins cache and crash-safe flushes — planned | 56, 58, 62 | A crash at any point of a flush restarts to a consistent tip and replays to the same MuHash. The cache stays under its configured memory limit. | Bitcoin Core `coins.cpp` |
| 64. Pruning — planned | 61, 63 | A pruned node keeps its block files under the configured target, keeps at least the last 288 blocks, and still reorganizes across them. | Bitcoin Core `blockstorage.cpp` |

### Part XI. The network

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md) — implemented | 5, 34 | The developer reference's `version`, `getblocks`, `headers`, `inv`, `addr`, `addrv2`, `feefilter` and `ping` samples parse and re-serialize exactly, and every line the example prints matches Bitcoin Core's own `messages.py`. A bad magic, a bad checksum, a length above 4,000,000 and a badly padded command each refuse in Core's order. | Bitcoin P2P protocol |
| 66. ElligatorSwift — planned | 15, 17 | BIP-324's `ellswift_decode_test_vectors.csv` and `xswiftec_inv_test_vectors.csv` pass. | BIP-324 |
| 67. The v2 encrypted transport — planned | 9, 12, 65, 66 | BIP-324's `packet_encoding_test_vectors.csv` give every ciphertext, including rekeying. | BIP-324 |
| 68. The handshake and feature negotiation — planned | 65, 67 | The node completes `version` and `verack` with Bitcoin Core over the v1 and v2 transports and negotiates `wtxidrelay`, `sendaddrv2`, `sendheaders`, `sendcmpct` and `feefilter`. A feature message after `verack` is handled as Core handles it. | BIP-130, BIP-133, BIP-152, BIP-155, BIP-339 |
| 69. Addresses and the address manager — planned | 5, 65, 68 | `addr` and `addrv2` messages round-trip for every network BIP-155 names. The address manager's buckets keep one source from filling its tables, and the eclipse cases ported from Bitcoin Core's address-manager tests pass. DNS seeds are queried only when the manager has no usable address. | BIP-155, Bitcoin Core `addrman.cpp` |
| 70. Connections: outbound, block-relay-only, feelers and eviction — planned | 68, 69 | The node keeps 8 full-relay and 2 block-relay-only outbound connections and makes feeler connections. With its inbound slots full it evicts by Bitcoin Core's protected-peer rules, and Core's eviction cases pass. | Bitcoin Core `net.cpp` |
| 71. Misbehavior, limits and denial of service — planned | 68, 70 | Each message limit (size, `inv` and `addr` counts and rates, unconnecting headers and the low-work headers presync) refuses its oversized case. A peer that sends an invalid block is disconnected, and memory per peer stays under its stated bound under a flood. | Bitcoin Core `net_processing.cpp` |
| 72. Headers sync and block download — planned | 59, 60, 65, 68, 71 | From genesis the node downloads headers by locator, presyncs them against low-work spam, and fetches blocks from many peers through a moving window, replacing a peer that stalls. It syncs regtest and signet from Bitcoin Core peers to their tips. | Bitcoin Core `net_processing.cpp` |
| 73. Compact blocks — planned | 10, 65 | Short IDs and block reconstruction for pinned blocks match Bitcoin Core. | BIP-152 |
| 74. Compact block filters — planned | 10, 46 | BIP-158's `testnet-19.json` filters and filter headers match. | BIP-157, BIP-158 |
| 75. Proxies, Tor and I2P — planned | 68, 70 | The node reaches peers through a SOCKS5 proxy and a local Tor daemon, accepts inbound connections on an onion service, and connects to I2P peers through the SAM bridge. Each network's addresses stay apart in the address manager. | BIP-155, Tor `control-spec`, I2P SAM v3 |

### Part XII. Initial block download

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 76. The signature cache and parallel script checks — planned | 57 | Script checks run on every core and give the same verdicts as one thread over a pinned block range. A transaction verified on mempool entry is not verified again in its block, and the cache is keyed by a salted hash. | Bitcoin Core `sigcache.cpp`, `checkqueue.h` |
| 77. Faster verification: the GLV endomorphism — planned | 16, 19, 23 | Verification with the endomorphism and wider windows agrees with chapter 16's path on random and edge inputs, and a pinned block's signatures verify at a measured rate within a stated factor of libsecp256k1 on the same machine. | Gallant, Lambert and Vanstone (2001) |
| 78. assumevalid — planned | 57, 72 | Below the pinned assumevalid block the node skips script checks only for ancestors of that block buried under its stated work; every other rule still runs. With assumevalid off the node reaches the same UTXO set. | Bitcoin Core `validation.cpp` |
| 79. AssumeUTXO snapshots — planned | 56, 63, 72 | The node loads a pinned mainnet UTXO snapshot whose hash matches the chain parameters, follows the tip from it at once, and validates from genesis in the background. A snapshot with one changed coin refuses. | Bitcoin Core `dumptxoutset`, `loadtxoutset` |
| 80. Sync mainnet from genesis — planned | 63, 64, 72, 76, 77, 78 | The node syncs mainnet from genesis to a pinned height over the public network, and its UTXO set at that height equals Bitcoin Core's `gettxoutsetinfo` (`hash_serialized_3` and `muhash`). The chapter records time, memory and disk on stated hardware next to Bitcoin Core's on the same machine. | — |

### Part XIII. The mempool and policy

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 81. Policy is not consensus: standardness — planned | 36, 40, 53 | Every standardness rule (output types, size, dust, `OP_RETURN` data and witness limits) refuses its pinned non-standard transaction with Bitcoin Core's reason at the pinned release, and each of those transactions still validates in a block. | Bitcoin Core `policy.cpp` |
| 82. Mempool acceptance and limits — planned | 57, 81 | Accept and refuse decisions and reasons match Bitcoin Core's `testmempoolaccept` on a pinned corpus, and the mempool keeps the size and package limits of the pinned release. | Bitcoin Core `validation.cpp`, `txmempool.cpp` |
| 83. Transaction relay and orphans — planned | 68, 82 | Transactions announced by wtxid are requested, accepted and announced onward with Bitcoin Core's randomized delays. A transaction with a missing parent waits in a bounded orphan pool and is accepted when the parent arrives. | BIP-339, Bitcoin Core `net_processing.cpp` |
| 84. Replacement: RBF and TRUC — planned | 82 | Replacements are accepted or refused as Bitcoin Core decides on its replacement cases, including the fee-rate and total-fee rules and the version-3 (TRUC) topology limits. | BIP-125, BIP-431 |
| 85. Packages, CPFP and anchors — planned | 82, 84 | One-parent-one-child packages, and pay-to-anchor outputs with zero-fee parents, are accepted and relayed as Bitcoin Core accepts them. A package below the minimum fee rate refuses. | BIP-331, Bitcoin Core `policy/packages.cpp` |
| 86. Eviction, expiry and persistence — planned | 82 | A full mempool evicts the lowest fee-rate transactions first and raises its minimum fee, transactions expire after two weeks, and a restart reloads the saved mempool. | Bitcoin Core `txmempool.cpp` |
| 87. Fee estimation — planned | 82 | Over a replayed history of blocks and mempool entries, estimates for 2, 6 and 144 blocks meet the success rate Bitcoin Core's `estimatesmartfee` targets. | Bitcoin Core `fees.cpp` |
| 88. Block templates and mining — planned | 49, 54, 82 | `getblocktemplate` chooses transactions by fee rate within the weight and sigop limits, and a regtest block mined from its template is accepted by Bitcoin Core. | BIP-22, BIP-23 |

### Part XIV. Operating the node

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 89. The node process: configuration, data directory, logs and shutdown — planned | 63 | The node reads its configuration file and flags, keeps its state in one data directory, logs by category, and on a signal at any point shuts down cleanly and restarts at the same tip. | — |
| 90. JSON-RPC and REST — planned | 59, 82 | The node answers the Bitcoin Core RPCs that wallets and Lightning implementations call (`getblockchaininfo`, `getblock`, `getblockheader`, `getrawtransaction`, `gettxout`, `sendrawtransaction`, `estimatesmartfee`, `getblocktemplate`, `submitblock` and the others the chapter names) with Core's JSON for the same chain, behind cookie authentication. | Bitcoin Core RPC |
| 91. Indexes: transactions, filters and coin statistics — planned | 56, 61, 74 | With each index on, `getrawtransaction` finds any confirmed transaction, the node serves BIP-157 filters to light clients, and `gettxoutsetinfo` answers for past heights, each matching Bitcoin Core. | BIP-157 |
| 92. Notifications for wallets and Lightning — planned | 60, 82 | A subscriber receives every connected and disconnected block and every mempool entry and removal in order, across a reorganization, and resumes from its last block after a restart. | — |
| 93. Shadow mode beside Bitcoin Core — planned | 80, 90 | Run beside a Bitcoin Core node on the same peers for a stated period, the node gives the same verdict on every header and block and holds the same tip. Any difference stops it and records the block. | — |

### Part XV. Wallets

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 94. PSBT — planned | 34, 38, 43 | Every valid and invalid PSBT in BIP-174 and BIP-370 parses or refuses as stated. | BIP-174, BIP-370 |
| 95. Coin selection and change — planned | 40, 94 | Branch-and-bound and fallback selection pass Bitcoin Core's coin-selection cases, and no selection pays more than its stated fee rate allows. | Bitcoin Core `coinselection.cpp` |
| 96. Message signing — planned | 36, 94 | BIP-322's simple and full vectors verify. | BIP-322 |
| 97. Silent payments — planned | 16, 24, 29 | BIP-352's `send_and_receive_test_vectors.json` give every output and scan result. | BIP-352 |
| 98. Miniscript — planned | 30, 36, 44 | The BIP-379 examples type-check, compile to their stated scripts and satisfy. | BIP-379 |
| 99. A descriptor wallet on our node — planned | 30, 33, 92, 94, 95 | A descriptor wallet finds its coins by rescan and by notification, builds, signs and broadcasts a spend on signet through our node, and survives a reorganization that removes a payment. | BIP-380 |

### Part XVI. Many signers

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 100. Multisig — planned | 30, 36 | 2-of-3 P2WSH `sortedmulti` descriptors give their addresses, and a spend signed by two keys validates. | BIP-11, BIP-67 |
| 101. MuSig2 — planned | 24, 41 | Every BIP-327 vector file (key aggregation, nonces, signing, aggregation and tweaks) gives its result. A reused nonce refuses. | BIP-327 |
| 102. FROST threshold signatures — planned | 101 | The FROST draft BIP's vectors give their results. A signature from fewer than t signers refuses. | RFC 9591, FROST draft BIPs |
| 103. Adaptor signatures — planned | 24 | An adaptor signature verifies against its point, completes with the secret, and the secret is recovered from the final signature. | BIP-340 |

### Part XVII. Lightning

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 104. Messages and TLVs — planned | 1 | BOLT 1's TLV vectors give their results. An unknown even type refuses. | BOLT 1 |
| 105. The Noise handshake — planned | 9, 12, 16 | BOLT 8's handshake and message vectors match byte for byte, including key rotation. | BOLT 8 |
| 106. Commitment keys and revocation — planned | 4, 16 | BOLT 3's key-derivation vectors give every key. | BOLT 3 |
| 107. Shachain — planned | 4 | BOLT 3's per-commitment secret storage vectors pass, including the cases that must fail. | BOLT 3 |
| 108. Commitment and HTLC transactions — planned | 38, 100, 106 | BOLT 3's commitment and HTLC transaction vectors match byte for byte, with and without anchors. | BOLT 3 |
| 109. Open, update and close a channel — planned | 104, 105, 107, 108 | Two Coax nodes open, update and close a channel on regtest. The state machine refuses every out-of-order message BOLT 2 names. | BOLT 2 |
| 110. Watch the chain — planned | 92, 109 | A revoked commitment broadcast on regtest is swept by the penalty transaction, and an expired HTLC times out, with our node as the only chain source. | BOLT 5 |
| 111. Onion routing and blinded paths — planned | 8, 11, 16 | BOLT 4's onion and blinded-path vectors match. A tampered onion fails its HMAC. | BOLT 4 |
| 112. Gossip and the channel graph — planned | 19, 104 | Announcements from a pinned gossip snapshot verify. An announcement with a bad signature refuses. | BOLT 7 |
| 113. Find a route and pay — planned | 109, 111, 112 | A payment routes over a three-node regtest network and settles. A failure returns its error to the sender. | BOLT 2, BOLT 4 |
| 114. Invoices — planned | 22, 27 | BOLT 11's examples decode to their stated fields and payee. A changed character refuses. | BOLT 11 |
| 115. Offers — planned | 5, 24, 104 | BOLT 12's offer, invoice-request and invoice vectors decode and verify. | BOLT 12 |

### Part XVIII. Interop and verification

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 116. Test vectors and their provenance — planned | — | Every vector the book uses is pinned with its source and hash, and the pin check fails on an edited vector. | — |
| 117. Differential tests — planned | 24, 36, 43, 57 | Signatures, signature hashes, script results and block verdicts agree with libsecp256k1 and Bitcoin Core on generated inputs. | — |
| 118. Bitcoin Core's functional tests against our node — planned | 68, 90 | The Bitcoin Core functional tests that drive a node over P2P and RPC (`feature_block.py` and the `p2p_*` and `mempool_*` tests the chapter names) pass against our node on one of our own machines, never the owner's Mac. Each test that does not apply is listed with its reason. | Bitcoin Core `test/functional` |
| 119. Fuzzing — planned | 34, 65, 94, 114 | Every parser (transactions, scripts, P2P messages, PSBTs, addresses and invoices) runs under the fuzzer with no crash and no accepted malformed input. | — |
| 120. Formal properties — planned | 57 | Each named property (round trips, refusal of non-canonical encodings and the consensus rule table) has a checked model or a stated boundary. | — |

### Part XIX. Bitcoin in OpenAgents

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 121. One seed for Nostr and Bitcoin — planned | 31, 33 | One BIP-39 phrase gives the Nostr key and the wallet keys, and restoring from the phrase gives the same addresses. | BIP-39, NIP-06 |
| 122. Agent treasuries — planned | 94, 100, 101 | A 2-of-3 treasury spend is signed by independent key agents and broadcast on signet, and an independent wallet tool recovers the coins. | NIP-KEYS |
| 123. Paying for requests — planned | 113, 114 | A request pays a BOLT 11 invoice, and the server releases its response only after settlement. | x402, BOLT 11 |
| 124. Our node on Pylons and Verse OS — planned | 89, 93 | A Pylon and a Verse OS machine run the node from its Nix module and sync it, and OpenAgents' on-chain checks query it instead of a third-party chain source. | — |
