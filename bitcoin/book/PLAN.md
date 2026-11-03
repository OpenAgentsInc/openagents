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
- Bitcoin Core is pinned to one revision in chapter 116: `ddea9196`, a
  `master` revision of 2026-11-02 past v32, not a release. Policy that
  changes between releases (standardness, mempool limits, `OP_RETURN` size)
  follows that revision.
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
| [30. Output descriptors and their checksum](ch030-output-descriptors-and-their-checksum.md) — implemented | 29, 32, 33 | The BIP-380 checksum vectors and the BIP-381–386 examples parse and give their stated scripts. | BIP-380 to BIP-386 |

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
| [42. Script trees: tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md) — implemented | 41 | BIP-341's script-tree vectors give every leaf hash, Merkle root and output key. | BIP-341 |
| [43. Taproot signature hashes](ch043-taproot-signature-hashes.md) — implemented | 34, 41 | Every BIP-341 key-path and script-path signature hash vector matches, for every hash type. | BIP-341 |
| 44. Tapscript — planned | 36, 43 | Bitcoin Core's Taproot script tests (`script_assets_test.json`) give their expected results. | BIP-342 |
| 45. Control blocks and script-path spends — planned | 42, 44 | Script-path spends from BIP-341's vectors verify. A control block with the wrong parity or path refuses. | BIP-341 |

### Part IX. Blocks and consensus

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) — implemented | 3, 5, 34 | Merkle roots of pinned mainnet blocks match their headers. A block whose last transaction is duplicated is detected (CVE-2012-2459). | CVE-2012-2459 |
| [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md) — implemented | 2, 3, 5 | The genesis header and pinned headers hash below their targets; a header with one changed bit does not. `nBits` decodes and encodes as Bitcoin Core's `SetCompact` and `GetCompact` on Core's cases, and the work of each pinned header matches. | Bitcoin Core `pow.cpp`, `arith_uint256.cpp` |
| [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md) — implemented | 47 | Chapter 47 builds the compact form and the work of one header; this chapter adds the retarget and the sum. `nBits` round-trips for every mainnet retarget to a pinned height (973,728), computed retargets match the chain, and chainwork to block 966,143 matches Bitcoin Core's `nMinimumChainWork`, the `getblockheader` chainwork of its `defaultAssumeValid` block. | Bitcoin Core `pow.cpp`, BIP-54, BIP-94 |
| [49. The witness commitment](ch049-the-witness-commitment.md) — implemented | 35, 46 | Block 514,999's coinbase commitment equals SHA-256d of its computed witness root and reserved value, and the script `GenerateCoinbaseCommitment` writes for them. Eighteen blocks, built from it with Core's `blocktools.py`, get Core's verdicts: `bad-witness-nonce-size`, `bad-witness-merkle-match`, `unexpected-witness` or none, with the last matching output counting; `IsBlockMutated` tells a changed witness, a repeated transaction and a wrong root apart. Every line of the example agrees with Core's rules transliterated over `messages.py`. | BIP-141 |
| [50. Chain parameters: mainnet, testnet4, signet and regtest](ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md) — implemented | 5, 36, 47, 48 | Each network's genesis block hashes to its published hash, and its message start, default port, address prefixes and deployment heights match Bitcoin Core's `chainparams.cpp` at the pinned release. A signet block whose solution fails the network's challenge refuses. | BIP-94, BIP-325, Bitcoin Core `chainparams.cpp` |
| [51. Header rules: time, versions and deployments](ch051-header-rules-time-versions-and-deployments.md) — implemented | 47, 48, 50 | Median time past and the two-hour future limit decide pinned headers as Bitcoin Core does, and testnet4's timewarp rule refuses its counterexample. The buried deployments (BIP-34, BIP-65, BIP-66, CSV, segwit and Taproot) activate at the chain parameters' heights, and a header whose version is below an active deployment's minimum refuses. | BIP-9, BIP-90, BIP-94, BIP-113 |
| [52. Transaction checks outside script](ch052-transaction-checks-outside-script.md) — implemented | 34, 35, 39, 40 | Every case in `tx_invalid.json` that fails before the script interpreter refuses with its reason: no inputs or outputs, a value out of range, an overflowing total (CVE-2010-5139), duplicate inputs (CVE-2018-17144), a coinbase spent before 100 confirmations and a non-final transaction. | BIP-68, BIP-113, CVE-2010-5139, CVE-2018-17144 |
| [53. Sigops, weight and block limits](ch053-sigops-weight-and-block-limits.md) — implemented | 36, 40 | Legacy, P2SH and witness signature-operation costs of pinned transactions and blocks match Bitcoin Core. A block over 4,000,000 weight units or 80,000 sigop cost refuses. | BIP-16, BIP-141 |
| [54. The subsidy and the coinbase](ch054-the-subsidy-and-the-coinbase.md) — implemented | 34, 50 | The subsidy at every halving matches, and is zero from height 6,930,000. A coinbase that claims one satoshi more than subsidy plus fees refuses, and so does a coinbase without its BIP-34 height after activation. | BIP-34, BIP-42 |
| 55. Script flags by height and their exceptions — planned | 36, 44, 50, 51 | The script verification flags for every block of a pinned mainnet range match Bitcoin Core's, including the BIP-16 and Taproot exception blocks, and the two blocks exempt from BIP-30 are handled as Core handles them. | BIP-16, BIP-30, BIP-341, Bitcoin Core `validation.cpp` |
| [56. The UTXO set and MuHash3072](ch056-the-utxo-set-and-muhash3072.md) — implemented | 2, 4, 11, 17, 34 | MuHash3072 of pinned UTXO sets matches Bitcoin Core's `gettxoutsetinfo muhash`. Adding then removing an element restores the hash. | Bitcoin Core `muhash.cpp` |
| 57. Validate a block: every consensus rule — planned | 43, 45, 46, 49, 51, 52, 53, 54, 55, 56 | Pinned mainnet, testnet4 and signet blocks, among them every soft-fork activation block, validate against the outputs they spend. A crafted block for each consensus rule refuses with Bitcoin Core's reject reason, and the chapter's rule table names the chapter that checks every rule. | BIP-30, BIP-34, BIP-141, BIP-341, BIP-342, Bitcoin Core `validation.cpp` |

### Part X. Chain state and storage

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 58. Connect and disconnect a block — planned | 56, 57 | Connecting then disconnecting each block of a pinned mainnet range restores the UTXO set's MuHash, using only the undo data the connect step wrote. Undo data with a wrong spent output refuses. | Bitcoin Core `validation.cpp` |
| [59. The block index and the most-work chain](ch059-the-block-index-and-the-most-work-chain.md) — implemented | 48, 51 | Mainnet's headers from genesis to 2,016 are accepted with Core's header checks and select block 2,016, with chainwork 2,017 times that of `0x1d00ffff`; a changed header fails `high-hash` and an orphan `prev-blk-not-found`. On mined regtest forks a fork with less work loses, equal work goes to the block whose data arrived first (then the lower pointer), and a header without data never becomes the tip. A block found invalid is marked with all its descendants, never becomes the tip, and headers on it refuse as `bad-prevblk` and `duplicate-invalid`. Core's `skiplist_tests.cpp` and `blockchain_tests.cpp` cases pass, and every line of the example agrees with Core's transliterated `chain.cpp`, `blockstorage.cpp` and `validation.cpp`. | Bitcoin Core `chain.cpp`, `node/blockstorage.cpp`, `validation.cpp` |
| 60. Reorganizations and invalid chains — planned | 58, 59 | On regtest a fork with more work replaces the tip: the node disconnects to the fork point and connects the new branch, and its UTXO set matches Bitcoin Core's after the same reorganization. A fork that contains an invalid block never becomes the tip. | Bitcoin Core `validation.cpp` |
| 61. Block files and undo files — planned | 34, 58 | Blocks and undo data are appended to files and read back by position, and the node imports Bitcoin Core's `blk*.dat` files. A file cut mid-record at any byte is detected at start and its damaged tail dropped. | Bitcoin Core `blockstorage.cpp` |
| [62. A key-value store for the chainstate](ch062-a-key-value-store-for-the-chainstate.md) — implemented | 1, 5 | Reads, writes, deletes and ordered iteration match a reference map on random workloads. A crash at every write of a recorded workload restarts to exactly the last committed batch. | — |
| 63. The coins cache and crash-safe flushes — planned | 56, 58, 62 | A crash at any point of a flush restarts to a consistent tip and replays to the same MuHash. The cache stays under its configured memory limit. | Bitcoin Core `coins.cpp` |
| 64. Pruning — planned | 61, 63 | A pruned node keeps its block files under the configured target, keeps at least the last 288 blocks, and still reorganizes across them. | Bitcoin Core `blockstorage.cpp` |

### Part XI. The network

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md) — implemented | 5, 34 | The developer reference's `version`, `getblocks`, `headers`, `inv`, `addr`, `addrv2`, `feefilter` and `ping` samples parse and re-serialize exactly, and every line the example prints matches Bitcoin Core's own `messages.py`. A bad magic, a bad checksum, a length above 4,000,000 and a badly padded command each refuse in Core's order. | Bitcoin P2P protocol |
| [66. ElligatorSwift](ch066-elligatorswift.md) — implemented | 15, 17 | All 76 rows of BIP-324's `ellswift_decode_test_vectors.csv` decode to their x, all 256 cases of `xswiftec_inv_test_vectors.csv` give BIP-324's `t` or refuse where it lists none, and the key exchange of every `packet_encoding_test_vectors.csv` row gives its shared x and secret. Every line of the example agrees with BIP-324's `reference.py`. | BIP-324 |
| [67. The v2 encrypted transport](ch067-the-v2-encrypted-transport.md) — implemented | 9, 12, 65, 66 | All 7 rows of BIP-324's `packet_encoding_test_vectors.csv` give their keys, terminators, session ID and ciphertext from the private key, including the rows after 999 and 1,024 packets (rekeying) and the 4,000,001- and 16,777,215-byte packets, and a receiver opens each one that Core accepts. Every line of the example agrees with BIP-324's `reference.py` and Core's `net.cpp`. | BIP-324 |
| [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md) — implemented | 65, 67 | Two nodes complete `version` and `verack` in memory over v1, over v2, with a v1 initiator at a v2 responder (detected from 16 bytes), and with a v2 initiator at a v1-only node (retried in v1), negotiating `wtxidrelay`, `sendaddrv2` and `sendcmpct`; our `version` and every payload the handshake sends equal Core's bytes. The version checks refuse in Core's order, each negotiation message after `verack` disconnects as Core does, and every line of the example agrees with Core's transliterated `ProcessMessage`, `messages.py` and BIP-324's `reference.py`. Interop with a running Core is chapter 118's. | BIP-130, BIP-133, BIP-152, BIP-155, BIP-339 |
| [69. Addresses and the address manager](ch069-addresses-and-the-address-manager.md) — implemented | 5, 10, 65, 68 | `addr` and `addrv2` round-trip for every BIP-155 network through chapter 65's reader. Core's bucket vectors (tried 40, new 786) and its `addrman_tests.cpp` cases pass: collisions, multiplicity, test-before-evict, `GetAddr`'s 2,006 and 461, `peers.dat` in Core's format 4 with its corruption cases. One source group sending 20,000 addresses fills 62 of 1,024 new buckets and wins 38 of 400 selections once 1,000 honest addresses arrive. DNS seeds are all queried at once only with no known address; otherwise after 11 s or 5 min, 3 at a time while outbound connections are short. Every line of the example agrees with Core's transliterated `AddrMan`, `ProcessAddrs` and `RelayAddress`. | BIP-155, Bitcoin Core `addrman.cpp` |
| [70. Connections: outbound, block-relay-only, feelers and eviction](ch070-connections-outbound-block-relay-only-feelers-and-eviction.md) — implemented | 68, 69 | A simulated node fills 8 full-relay and 2 block-relay-only slots from 10 network groups, starting with its anchor, and makes feeler and extra block-relay-only connections on exponential timers; the stale extra peer is dropped. With its inbound slots full it evicts by Bitcoin Core's protected-peer rules: Core's `peer_protection_test` cases and `peer_eviction_test` for 0 to 199 candidates pass, and 329 connections from 3 attacker groups evict none of 60 honest peers. Every line of the example agrees with Core's transliterated `ThreadOpenConnections`, `SelectNodeToEvict` and `EvictExtraOutboundPeers`. | Bitcoin Core `net.cpp`, `node/eviction.cpp` |
| [71. Misbehavior, limits and denial of service](ch071-misbehavior-limits-and-denial-of-service.md) — implemented | 68, 70 | Every message limit refuses its oversized case with Core's verdict and reason; a misbehaving peer is discouraged and disconnected unless noban, manual or local; MurmurHash3 matches Core's 14 vectors and the rolling bloom filter remembers its last entries in fixed memory (100,000 discouragements keep the last 50,000, forget the oldest and match none of 20,000 others); headers below the anti-DoS threshold start a presync bounded at about 1.4 MB; and flood control holds a peer to about 15 MB. Every line agrees with Core's transliterated `net_processing.cpp`, `net.cpp`, `bloom.cpp` and `headerssync.cpp`. | Bitcoin Core `net_processing.cpp`, `banman.cpp`, `common/bloom.cpp`, `headerssync.cpp` |
| 72. Headers sync and block download — planned | 59, 60, 65, 68, 71 | From genesis the node downloads headers by locator, presyncs them against low-work spam, and fetches blocks from many peers through a moving window, replacing a peer that stalls. It syncs regtest and signet from Bitcoin Core peers to their tips. | Bitcoin Core `net_processing.cpp` |
| [73. Compact blocks](ch073-compact-blocks.md) — implemented | 10, 35, 49, 65, 68 | Block 413,567's SipHash keys, first short IDs and 9,614-byte `cmpctblock` equal those of Core's `messages.py`. Mainnet block 514,999 is rebuilt byte for byte from a mempool and an extra pool after a `getblocktxn` for the two missing transactions; a duplicate short ID fails, colliding pool entries are asked for, and a wrong transaction or witness fails `IsBlockMutated`. Core's `blockencodings_tests.cpp` cases pass, and every line of the example agrees with `InitData`, `FillBlock` and `MaybeSetPeerAsAnnouncingHeaderAndIDs` transliterated over `messages.py`. | BIP-152 |
| [74. Compact block filters](ch074-compact-block-filters.md) — implemented | 10, 46 | BIP-158's `testnet-19.json` filters and filter headers match. | BIP-157, BIP-158 |
| 75. Proxies, Tor and I2P — planned | 68, 70 | The node reaches peers through a SOCKS5 proxy and a local Tor daemon, accepts inbound connections on an onion service, and connects to I2P peers through the SAM bridge. Each network's addresses stay apart in the address manager. | BIP-155, Tor `control-spec`, I2P SAM v3 |

### Part XII. Initial block download

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 76. The signature cache and parallel script checks — planned | 57 | Script checks run on every core and give the same verdicts as one thread over a pinned block range. A transaction verified on mempool entry is not verified again in its block, and the cache is keyed by a salted hash. | Bitcoin Core `sigcache.cpp`, `checkqueue.h` |
| [77. Faster verification: the GLV endomorphism](ch077-faster-verification-the-glv-endomorphism.md) — implemented | 16, 19, 23 | Verification with the endomorphism and wider windows agrees with chapter 16's path on random and edge inputs (2,000 splits and 300 multiplications against Python, libsecp256k1's near-bound scalars), chapters 19 and 23's suites (464 Wycheproof cases, 19 BIP-340 vectors, libsecp256k1 differentials) give the same results on it, and 200 fresh signatures verify at a measured rate within a stated factor of libsecp256k1 on the same machine (about 1.6 times chapter 16's rate; libsecp256k1 about 45 times faster). | Gallant, Lambert and Vanstone (2001) |
| 78. assumevalid — planned | 57, 72 | Below the pinned assumevalid block the node skips script checks only for ancestors of that block buried under its stated work; every other rule still runs. With assumevalid off the node reaches the same UTXO set. | Bitcoin Core `validation.cpp` |
| 79. AssumeUTXO snapshots — planned | 56, 63, 72 | The node loads a pinned mainnet UTXO snapshot whose hash matches the chain parameters, follows the tip from it at once, and validates from genesis in the background. A snapshot with one changed coin refuses. | Bitcoin Core `dumptxoutset`, `loadtxoutset` |
| 80. Sync mainnet from genesis — planned | 63, 64, 72, 76, 77, 78 | The node syncs mainnet from genesis to a pinned height over the public network, and its UTXO set at that height equals Bitcoin Core's `gettxoutsetinfo` (`hash_serialized_3` and `muhash`). The chapter records time, memory and disk on stated hardware next to Bitcoin Core's on the same machine. | — |

### Part XIII. The mempool and policy

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [81. Policy is not consensus: standardness](ch081-policy-is-not-consensus-standardness.md) — implemented | 36, 40, 53 | Every standardness rule (output types, size, dust, `OP_RETURN` data and witness limits) refuses its pinned non-standard transaction with Bitcoin Core's reason at the pinned release, and each of those transactions still validates in a block: CheckTransaction, the inputs' value, VerifyScript under the consensus flags, and the block's weight and sigop cost. Core's `test_IsStandard` and `max_standard_legacy_sigops` cases pass, and every line of the example agrees with Core's transliterated `policy.cpp` and `solver.cpp`. | Bitcoin Core `policy.cpp` |
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
| [94. PSBT](ch094-psbt.md) — implemented | 34, 38, 43 | Every valid and invalid PSBT in BIP-174 and BIP-370 parses or refuses as stated. | BIP-174, BIP-370 |
| [95. Coin selection and change](ch095-coin-selection-and-change.md) — implemented | 40, 94 | Branch-and-bound and fallback selection pass Bitcoin Core's coin-selection cases, and no selection pays more than its stated fee rate allows. | Bitcoin Core `coinselection.cpp` |
| [96. Message signing](ch096-message-signing.md) — implemented | 22, 36, 42, 43, 94 | BIP-322's simple and full vectors verify. | BIP-322 |
| [97. Silent payments](ch097-silent-payments.md) — implemented | 16, 24, 28, 29, 32, 41 | BIP-352's `send_and_receive_test_vectors.json` give every output and scan result. | BIP-352 |
| 98. Miniscript — planned | 30, 36, 44 | The BIP-379 examples type-check, compile to their stated scripts and satisfy. | BIP-379 |
| 99. A descriptor wallet on our node — planned | 30, 33, 92, 94, 95 | A descriptor wallet finds its coins by rescan and by notification, builds, signs and broadcasts a spend on signet through our node, and survives a reorganization that removes a payment. | BIP-380 |

### Part XVI. Many signers

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [100. Multisig](ch100-multisig.md) — implemented | 30, 36 | 2-of-3 P2WSH `sortedmulti` descriptors give their addresses, and a spend signed by two keys validates. | BIP-11, BIP-67 |
| [101. MuSig2](ch101-musig2.md) — implemented | 24, 41 | Every BIP-327 vector file (key aggregation, nonces, signing, aggregation and tweaks) gives its result. A reused nonce refuses. | BIP-327 |
| [102. FROST threshold signatures](ch102-frost-threshold-signatures.md) — implemented | 101 | The FROST draft BIP's vectors give their results. A signature from fewer than t signers refuses. | RFC 9591, FROST draft BIPs |
| [103. Adaptor signatures](ch103-adaptor-signatures.md) — implemented | 24 | A BIP-340 pre-signature for a point T pre-verifies against its key, message and T and not against others, completes with t into a signature that BIP-340 verifies, and gives t back from that signature. A reused nonce recovers the key and a reused point lets a third party complete a second pre-signature. Six vectors from an independent Python implementation on BIP-340's pinned reference.py match byte for byte, and every line agrees with it. | BIP-340 |

### Part XVII. Lightning

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [104. Messages and TLVs](ch104-messages-and-tlvs.md) — implemented | 1 | All 18 BigSize decodings and 8 encodings of BOLT 1's Appendix A, all 57 TLV streams of Appendix B in their namespaces (with the `n1` values), and the 5 `init` messages of Appendix C give BOLT 1's results. An unknown even type, feature bit or message type refuses; an unknown odd one is ignored. Every line of the example agrees with an independent Python reader. | BOLT 1 |
| [105. The Noise handshake](ch105-the-noise-handshake.md) — implemented | 9, 12, 16 | All 15 handshake vectors of BOLT 8's Appendix A, 5 initiator and 10 responder, give its acts and session keys or the error it names, and the message vectors' six outputs match byte for byte across two key rotations. Every line of the example agrees with an independent Python handshake. | BOLT 8 |
| [106. Commitment keys and revocation](ch106-commitment-keys-and-revocation.md) — implemented | 4, 16 | BOLT 3's Appendix E gives its base point, per-commitment point, localpubkey, localprivkey, revocationpubkey and revocationprivkey byte for byte, and each private key times G is its public key. Every line of the example agrees with an independent Python derivation. | BOLT 3 |
| [107. Shachain](ch107-shachain.md) — implemented | 4 | All 5 generation vectors of BOLT 3's Appendix D give its secrets, and all 9 storage vectors accept their good inserts and refuse at the insert BOLT 3 marks as an error. A 10,000-commitment store looks up every secret in 14 slots. Every line of the example agrees with an independent Python store. | BOLT 3 |
| [108. Commitment and HTLC transactions](ch108-commitment-and-htlc-transactions.md) — implemented | 38, 100, 106 | All 16 commitments of BOLT 3's Appendix C (static_remotekey) and all 9 of Appendix F (anchors), each signed by both peers, and all 48 of their HTLC-timeout and HTLC-success transactions match byte for byte, and Appendix B's funding transaction does too. The signed transactions spend their outputs under chapter 36's interpreter, and a wrong preimage or an early delayed spend fails. Every line of the example agrees with an independent Python builder. | BOLT 3 |
| [109. Open, update and close a channel](ch109-open-update-and-close-a-channel.md) — implemented | 104, 105, 107, 108 | Two in-process nodes open a channel with BOLT 2's v1 messages, add, fulfill and fail HTLCs both ways with `commitment_signed` and `revoke_and_ack`, lose a message and recover with `channel_reestablish`, refuse messages out of order, and close with a `closing_signed` fee negotiation. Every commitment is built and its signatures checked with chapter 108's builder, the force-close and closing transactions spend the funding output under chapter 36's interpreter, and every line agrees with an independent Python model. | BOLT 2 |
| 110. Watch the chain — planned | 92, 109 | A revoked commitment broadcast on regtest is swept by the penalty transaction, and an expired HTLC times out, with our node as the only chain source. | BOLT 5 |
| [111. Onion routing and blinded paths](ch111-onion-routing-and-blinded-paths.md) — implemented | 8, 11, 16 | The onion of BOLT 4's `onion-test.json` matches byte for byte and every hop reads its payload; the error packets of `onion-error-test.json` and the "Returning Errors" trace match at every node and the origin finds the erring hop; the blinded paths of `route-blinding-test.json`, the blinded payment onion and the onion message match at every hop. A tampered onion fails its HMAC. Every line of the example agrees with an independent Python Sphinx. | BOLT 4 |
| [112. Gossip and the channel graph](ch112-gossip-and-the-channel-graph.md) — implemented | 19, 104 | BOLT 7's six node_announcement vectors verify and each alias refuses; the ten messages of `extended-queries.json` read with their stated fields, except the five in zlib, which BOLT 7 now forbids and which refuse. BOLT 7 has no announcement or update vectors, so the example signs its own with generated keys: a changed byte, swapped node IDs, the wrong end's key, a stale timestamp and an unknown channel or node refuse. Every line of the example agrees with an independent Python reader and writer. | BOLT 7 |
| [113. Find a route and pay](ch113-find-a-route-and-pay.md) — implemented | 109, 111, 112, 114 | Payments route over five in-process nodes whose channels open, update and settle with chapter 109's messages: the cheapest route under fees and a uniform liquidity model, a multi-part payment the payee holds until `total_msat`, a `temporary_channel_failure` read from the error onion and retried another way, a route hint to a private channel, and an unknown invoice refused without retry. Every line of the example agrees with an independent Python payer and network. | BOLT 2, BOLT 4, BOLT 7, BOLT 11 |
| [114. Invoices](ch114-invoices.md) — implemented | 22, 27 | All 31 examples of BOLT 11's `invoice-test.json` are valid or invalid as stated, each invalid one for its reason; the 11 signature breakdowns match; every valid example decodes to its stated fields and payee and is written again to the same text. A changed character refuses. Every line of the example agrees with an independent Python reader and writer. | BOLT 11 |
| [115. Offers](ch115-offers.md) — implemented | 5, 24, 104 | BOLT 12's format strings, all 53 offers of `offers-test.json` (each invalid one for its reason), the Merkle roots, tagged hash and signature of `signature-test.json`, and the 5 valid and 23 invalid payer proofs of `payer-proof-test.json` give their stated results; the request and every payer proof are written again byte for byte. A changed character of a signed request refuses. Every line of the example agrees with an independent Python reader and writer. | BOLT 12 |

### Part XVIII. Interop and verification

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [116. Test vectors and their provenance](ch116-test-vectors-and-their-provenance.md) — implemented | — | Every vector the book uses is pinned with its source and hash, and the pin check fails on an edited vector. All 316 pins pass `check_vectors.py` (and `--fetch` for the 309 single-URL files), every Bitcoin Core file is at the target `ddea9196` or a listed exception, and `check_pins.py` fails on an edited vector, a stale inventory or index, an unpinned citation or a drifting generator. | — |
| 117. Differential tests — planned | 24, 36, 43, 57 | Signatures, signature hashes, script results and block verdicts agree with libsecp256k1 and Bitcoin Core on generated inputs. | — |
| 118. Bitcoin Core's functional tests against our node — planned | 68, 90 | The Bitcoin Core functional tests that drive a node over P2P and RPC (`feature_block.py` and the `p2p_*` and `mempool_*` tests the chapter names) pass against our node on one of our own machines, never the owner's Mac. Each test that does not apply is listed with its reason. | Bitcoin Core `test/functional` |
| [119. Fuzzing](ch119-fuzzing.md) — implemented | 26, 34, 65, 94, 114, 116 | Every parser (transactions, scripts, P2P messages, PSBTs, addresses and invoices) runs under the fuzzer with no crash and no accepted malformed input. Done for transactions, P2P headers and version payloads, PSBTs, BOLT 11 invoices and Base58Check: 100,000 seeded inputs per parser in checked and release builds, each compared code for code with a Python reference, with no crash and no accepted input that fails to write back. Scripts, descriptors and addresses are not fuzzed yet. | — |
| 120. Formal properties — planned | 57 | Each named property (round trips, refusal of non-canonical encodings and the consensus rule table) has a checked model or a stated boundary. | — |

### Part XIX. Bitcoin in OpenAgents

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [121. One seed for Nostr and Bitcoin](ch121-one-seed-for-nostr-and-bitcoin.md) — implemented | 31, 33 | One BIP-39 phrase gives the Nostr key and the wallet keys, and restoring from the phrase gives the same addresses. | BIP-39, NIP-06 |
| [122. Agent treasuries](ch122-agent-treasuries.md) — implemented | 94, 100, 101 | A 2-of-3 treasury spend is signed by independent key agents and broadcast on signet, and an independent wallet tool recovers the coins. | NIP-KEYS |
| 123. Paying for requests — planned | 113, 114 | A request pays a BOLT 11 invoice, and the server releases its response only after settlement. | x402, BOLT 11 |
| 124. Our node on Pylons and Verse OS — planned | 89, 93 | A Pylon and a Verse OS machine run the node from its Nix module and sync it, and OpenAgents' on-chain checks query it instead of a third-party chain source. | — |
