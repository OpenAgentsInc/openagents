# Plan and prerequisites

This plan gives the order of the book, what you have at the end of each part,
and, for every chapter, its dependencies, what "done" means and the
specifications it implements. Chapters 1–4, 8, 9, 11, 13–16, 18, 23, 24 and
27 moved here from [Nostr From Scratch](../../nostr/book/README.md), whose
chapters 1–13, 15 and 18 they were; their rows keep Nostr's done criteria.
Every other chapter is planned.

## What you need first

- The Coax language: [Coax book](../../coax/book/README.md) Parts I–III.
- A built compiler: `coax/build.sh`.
- No outside cryptography, Bitcoin or Lightning library. libsecp256k1,
  Bitcoin Core and other implementations appear only in the tests of
  Part XIV, never in the code the book builds.

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
| IX. Blocks and consensus | 46–52 | Merkle trees, headers and proof of work, a bounded block-validation profile and the UTXO set. |
| X. The network | 53–57 | P2P messages, BIP-324 encrypted transport, compact blocks and block filters. |
| XI. Wallets | 58–62 | PSBTs, coin selection, message signing, silent payments and miniscript. |
| XII. Many signers | 63–66 | Multisig, MuSig2, FROST threshold signatures and adaptor signatures. |
| XIII. Lightning | 67–78 | Lightning built in Coax: messages, the Noise handshake, channels, onions, gossip, payments, invoices and offers. |
| XIV. Interop and verification | 79–82 | Test vector provenance, differential tests, fuzzing and formal properties. |
| XV. Bitcoin in OpenAgents | 83–85 | Where OpenAgents uses the stack: one seed, agent treasuries and paid requests. |

Each part uses only earlier parts, except where a chapter's dependencies say
otherwise. Nostr From Scratch needs Parts I–V.

## Every chapter

### Part I. Foundations

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) — implemented | coax:5, coax:6 | Every RFC 4648 test vector for base64 (`coax/tests/vectors/upstream/rfc/rfc4648.txt`) passes, and every non-canonical form refuses. Hex and base64 round trips hold for every length up to the profile limit in a `#forall` check. `coax mutate` leaves zero survivors on the codec files. | — |
| [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) — implemented | 1, coax:13 | Limb operations match the big-integer oracle on edge values: 0, 1, `p - 1`, `2^256 - 1` and values at each magnitude bound. A `#forall` check confirms no intermediate exceeds its declared bound. Native and wasm builds give the same bytes. | — |
| [3. Constant-time rules and secrets](ch003-constant-time-rules.md) — implemented | 1, 2, coax:23 | A leaky branch fixture and a leaky index fixture are each detected by the timing harness. The constant-time checker refuses a seeded secret-dependent branch with its diagnostic. Every secret buffer in Parts II–V is erased on success and on every failure path. | — |

### Part II. Hashes, MACs and ciphers

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [4. SHA-256](ch004-sha-256.md) — implemented | 1 | Every NIST CAVS 11.0 vector in `coax/tests/vectors/upstream/nist/SHA256ShortMsg.rsp` and `SHA256LongMsg.rsp` passes. Every split of a 1,000-byte message gives the same digest. The compression kernel moves out of the compiler-fingerprint module without changing a digest. | — |
| 5. SHA-256d and tagged hashes — planned | 4 | The txids of pinned mainnet transactions and the genesis block hash match their published values. The tag hashes for `BIP0340/challenge`, `TapLeaf`, `TapBranch`, `TapTweak` and `TapSighash` match the BIP texts. | BIP-340, BIP-341 |
| 6. RIPEMD-160 and HASH160 — planned | 1 | The test vectors in the RIPEMD-160 paper pass. HASH160 of BIP-32 vector 1's master public key gives its published fingerprint. | RIPEMD-160 (Dobbertin, Bosselaers and Preneel, 1996) |
| 7. SHA-512 — planned | 1 | Every NIST CAVS vector in `SHA512ShortMsg.rsp` and `SHA512LongMsg.rsp` passes. | FIPS 180-4 |
| [8. HMAC-SHA256](ch008-hmac-sha256.md) — implemented | 4 | RFC 4231 SHA-256 cases 1–7 (`coax/tests/vectors/upstream/rfc/rfc4231.txt`) pass. A tag that differs in one bit is refused by the constant-time comparison. | — |
| [9. HKDF](ch009-hkdf.md) — implemented | 8 | RFC 5869 SHA-256 cases 1–3 (`coax/tests/vectors/upstream/rfc/rfc5869.txt`) pass. An expand request of `255 * 32 + 1` bytes refuses. | — |
| 10. SipHash-2-4 — planned | 1 | The 64 vectors of the SipHash reference implementation pass. | SipHash (Aumasson and Bernstein, 2012) |
| [11. ChaCha20](ch011-chacha20.md) — implemented | 1 | RFC 8439 sections 2.3.2 and 2.4.2 vectors (`coax/tests/vectors/upstream/rfc/rfc8439.txt`) pass. A request that would pass counter `2^32 - 1` refuses. | — |
| 12. Poly1305 and ChaCha20-Poly1305 — planned | 11, 3 | RFC 8439 sections 2.5.2 and 2.8.2 vectors pass. A tag that differs in one bit refuses and releases no plaintext. | RFC 8439 |

### Part III. secp256k1

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md) — implemented | 2, 3 | Every operation matches the big-integer oracle on edge and random values. Inverse and square root use fixed chains, checked by the constant-time harness. Zero mutation survivors in `secp_field.coax`. | — |
| [14. Scalars mod n](ch014-scalars-mod-n.md) — implemented | 2, 3 | Scalar operations match the big-integer oracle. Out-of-range keys and `s` values refuse, each with its own status. | — |
| [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md) — implemented | 13 | Group laws hold on random points: associativity, `P + (-P) = O`, `2P = P + P`. `lift_x` refuses every x whose `x^3 + 7` is not a square. | — |
| [16. Scalar multiplication](ch016-scalar-multiplication.md) — implemented | 14, 15, 3 | All three paths give the oracle's point for random and edge scalars (0, 1, `n - 1`). The constant-time paths pass the timing harness with leaky controls detected. | — |
| 17. Point encodings: compressed, uncompressed and x-only — planned | 15 | Compressed, uncompressed and x-only encodings round-trip for random points. Hybrid encodings (`0x06`, `0x07`) and points off the curve refuse. | SEC 1, BIP-340 |
| [18. Keys and randomness](ch018-keys-and-randomness.md) — implemented | 14, 3 | A key drawn on each host is a valid scalar, and a forced out-of-range draw is retried. A signer reply with the wrong ID, pubkey or body is refused. | — |

### Part IV. Signatures

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 19. Verify ECDSA — planned | 5, 14, 16, 17 | Every case in Wycheproof's `ecdsa_secp256k1_sha256_bitcoin_test.json` gives its expected result. A high-S signature refuses under the low-S rule. | SEC 1, BIP-146 |
| 20. Sign with ECDSA — planned | 19, 8, 18, 3 | RFC 6979 nonces match published secp256k1 vectors. Every signature is low-S and verifies in an independent library. A secret-probe run finds no secret-dependent branch. | RFC 6979, BIP-62, BIP-146 |
| 21. Strict DER — planned | 19 | Every DER case in Bitcoin Core's `script_tests.json` that BIP-66 decides gives its expected result. | BIP-66 |
| 22. Public key recovery and signed messages — planned | 19 | Bitcoin Core `signmessage` signatures for pinned keys verify, and recovery returns the signing key. A recovery ID out of range refuses. | Bitcoin Core `signmessage` |
| [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md) — implemented | 4, 14, 16 | All 19 BIP-340 vectors in `coax/tests/vectors/upstream/bip340/test-vectors.csv` give their expected result. Fast and reference paths agree on every vector and on random signatures. | — |
| [24. Sign with Schnorr](ch024-sign-with-schnorr.md) — implemented | 23, 18, 3 | The BIP-340 signing rows (secret key, aux and message given) produce the expected signatures byte for byte. A secret-probe run finds no secret-dependent branch in the emitted signer. An independent library verifies 10,000 signatures from the Coax signer. | — |
| 25. Batch verification — planned | 23 | A batch of the valid BIP-340 vectors verifies; a batch with one invalid signature refuses. Batch and single verification agree on random batches. | BIP-340 |

### Part V. Encodings and addresses

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 26. Base58Check — planned | 5 | BIP-32 extended keys and pinned P2PKH and P2SH addresses decode and re-encode exactly. A one-character change fails the checksum. | Bitcoin Core `base58.cpp` |
| [27. Bech32](ch027-bech32.md) — implemented | 1 | Every valid and invalid BIP-173 Bech32 vector in `coax/tests/vectors/upstream/bip173/bip-0173.mediawiki` gives its expected result. A Bech32m string refuses (BIP-350's strings, not yet pinned in the corpus). | — |
| 28. Bech32m — planned | 27 | Every valid and invalid BIP-350 vector gives its expected result, including the witness-version rules. | BIP-350 |
| 29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR — planned | 6, 17, 26, 28 | Every BIP-173 and BIP-350 segwit address vector and pinned P2PKH and P2SH addresses map to their scriptPubKeys and back. | BIP-13, BIP-141, BIP-173, BIP-341, BIP-350 |
| 30. Output descriptors and their checksum — planned | 29, 32 | The BIP-380 checksum vectors and the BIP-381–386 examples parse and give their stated scripts. | BIP-380 to BIP-386 |

### Part VI. Keys from words

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 31. Seed phrases — planned | 4, 7, 8 | The BIP-39 English vectors give their entropy, mnemonic and seed through PBKDF2-HMAC-SHA512. A phrase with a bad checksum refuses. | BIP-39, RFC 8018 |
| 32. Hierarchical deterministic keys — planned | 6, 16, 31 | BIP-32 test vectors 1–5 give every extended key, and vector 5's invalid keys refuse. | BIP-32 |
| 33. Derivation paths — planned | 29, 32 | The BIP-44, BIP-84 and BIP-86 vectors give their addresses, and NIP-06's path gives the Nostr book's chapter 46 key. | BIP-44, BIP-84, BIP-86, NIP-06 |

### Part VII. Transactions and script

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 34. Transactions and their serialization — planned | 1, 5 | Every transaction in Bitcoin Core's `tx_valid.json` and `tx_invalid.json` parses, and serialization round-trips byte for byte with and without witnesses. | BIP-144 |
| 35. Txids, wtxids and malleability — planned | 34 | Txids and wtxids of pinned segwit transactions match. Changing a witness changes the wtxid and leaves the txid. | BIP-141 |
| 36. The script interpreter — planned | 6, 19, 21, 34 | Every case in Bitcoin Core's `script_tests.json` gives its expected result under its stated flags. | Bitcoin Core `script_tests.json` |
| 37. Legacy signature hashes — planned | 34, 36 | Every case in Bitcoin Core's `sighash.json` gives its digest, including `SIGHASH_SINGLE` with no matching output. | Bitcoin Core `sighash.json` |
| 38. Segwit v0 signature hashes — planned | 5, 34 | BIP-143's examples give their digests for every hash type. | BIP-143 |
| 39. Timelocks — planned | 36 | The `CHECKLOCKTIMEVERIFY` and `CHECKSEQUENCEVERIFY` cases in `script_tests.json` and `tx_valid.json` pass, and BIP-68 sequence encodings round-trip. | BIP-65, BIP-68, BIP-112, BIP-113 |
| 40. Weight, vsize and fees — planned | 34 | Weight and vsize of pinned transactions match Bitcoin Core's `decoderawtransaction`. | BIP-141 |

### Part VIII. Taproot

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 41. The key tweak and the key path — planned | 5, 24 | BIP-341's `wallet-test-vectors.json` key-path tweaks and signatures match. | BIP-341 |
| 42. Script trees: tapleaves and tapbranches — planned | 41 | BIP-341's script-tree vectors give every leaf hash, Merkle root and output key. | BIP-341 |
| 43. Taproot signature hashes — planned | 34, 41 | Every BIP-341 key-path and script-path signature hash vector matches, for every hash type. | BIP-341 |
| 44. Tapscript — planned | 36, 43 | Bitcoin Core's Taproot script tests (`script_assets_test.json`) give their expected results. | BIP-342 |
| 45. Control blocks and script-path spends — planned | 42, 44 | Script-path spends from BIP-341's vectors verify. A control block with the wrong parity or path refuses. | BIP-341 |

### Part IX. Blocks and consensus

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 46. Merkle trees and the duplicate-leaf flaw — planned | 5 | Merkle roots of pinned mainnet blocks match their headers. A block whose last transaction is duplicated is detected (CVE-2012-2459). | CVE-2012-2459 |
| 47. Block headers and proof of work — planned | 2, 5 | The genesis header and pinned headers hash below their targets; a header with one changed bit does not. | Bitcoin Core `pow.cpp` |
| 48. Compact targets, retargeting and chainwork — planned | 47 | `nBits` round-trips for every mainnet retarget to a pinned height, computed retargets match the chain, and chainwork matches Bitcoin Core's `getblockheader`. | Bitcoin Core `pow.cpp` |
| 49. The witness commitment — planned | 35, 46 | The coinbase commitments of pinned segwit blocks match their computed witness roots. | BIP-141 |
| 50. Validate a block (bounded profile) — planned | 36, 39, 43, 46, 48, 49 | Pinned mainnet and signet blocks validate under the bounded profile. Blocks with stated failures refuse with their reasons. The chapter states the consensus rules the profile does not check. | BIP-30, BIP-34, BIP-90, BIP-141 |
| 51. The UTXO set and MuHash3072 — planned | 2, 11, 34 | MuHash3072 of pinned UTXO sets matches Bitcoin Core's `gettxoutsetinfo muhash`. Adding then removing an element restores the hash. | Bitcoin Core `muhash.cpp` |
| 52. Headers first and the most-work chain — planned | 48 | A header chain from genesis to a pinned height selects the most-work tip; a fork with less work loses. | Bitcoin Core `validation.cpp` |

### Part X. The network

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 53. P2P messages and checksums — planned | 5, 34 | Captured `version`, `inv`, `tx` and `block` messages parse and re-serialize exactly. A bad checksum refuses. | Bitcoin P2P protocol |
| 54. ElligatorSwift — planned | 15, 17 | BIP-324's `ellswift_decode_test_vectors.csv` and `xswiftec_inv_test_vectors.csv` pass. | BIP-324 |
| 55. The v2 encrypted transport — planned | 9, 12, 53, 54 | BIP-324's `packet_encoding_test_vectors.csv` give every ciphertext, including rekeying. | BIP-324 |
| 56. Compact blocks — planned | 10, 53 | Short IDs and block reconstruction for pinned blocks match Bitcoin Core. | BIP-152 |
| 57. Compact block filters — planned | 10, 46 | BIP-158's `testnet-19.json` filters and filter headers match. | BIP-157, BIP-158 |

### Part XI. Wallets

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 58. PSBT — planned | 34, 38, 43 | Every valid and invalid PSBT in BIP-174 and BIP-370 parses or refuses as stated. | BIP-174, BIP-370 |
| 59. Coin selection and change — planned | 40, 58 | Branch-and-bound and fallback selection pass Bitcoin Core's coin-selection cases, and no selection pays more than its stated fee rate allows. | Bitcoin Core `coinselection.cpp` |
| 60. Message signing — planned | 36, 58 | BIP-322's simple and full vectors verify. | BIP-322 |
| 61. Silent payments — planned | 16, 24, 29 | BIP-352's `send_and_receive_test_vectors.json` give every output and scan result. | BIP-352 |
| 62. Miniscript — planned | 30, 36, 44 | The BIP-379 examples type-check, compile to their stated scripts and satisfy. | BIP-379 |

### Part XII. Many signers

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 63. Multisig — planned | 30, 36 | 2-of-3 P2WSH `sortedmulti` descriptors give their addresses, and a spend signed by two keys validates. | BIP-11, BIP-67 |
| 64. MuSig2 — planned | 24, 41 | Every BIP-327 vector file (key aggregation, nonces, signing, aggregation and tweaks) gives its result. A reused nonce refuses. | BIP-327 |
| 65. FROST threshold signatures — planned | 64 | The FROST draft BIP's vectors give their results. A signature from fewer than t signers refuses. | RFC 9591, FROST draft BIPs |
| 66. Adaptor signatures — planned | 24 | An adaptor signature verifies against its point, completes with the secret, and the secret is recovered from the final signature. | BIP-340 |

### Part XIII. Lightning

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 67. Messages and TLVs — planned | 1 | BOLT 1's TLV vectors give their results. An unknown even type refuses. | BOLT 1 |
| 68. The Noise handshake — planned | 9, 12, 16 | BOLT 8's handshake and message vectors match byte for byte, including key rotation. | BOLT 8 |
| 69. Commitment keys and revocation — planned | 4, 16 | BOLT 3's key-derivation vectors give every key. | BOLT 3 |
| 70. Shachain — planned | 4 | BOLT 3's per-commitment secret storage vectors pass, including the cases that must fail. | BOLT 3 |
| 71. Commitment and HTLC transactions — planned | 38, 63, 69 | BOLT 3's commitment and HTLC transaction vectors match byte for byte, with and without anchors. | BOLT 3 |
| 72. Open, update and close a channel — planned | 67, 68, 70, 71 | Two Coax nodes open, update and close a channel on regtest. The state machine refuses every out-of-order message BOLT 2 names. | BOLT 2 |
| 73. Watch the chain — planned | 50, 72 | A revoked commitment broadcast on regtest is swept by the penalty transaction, and an expired HTLC times out. | BOLT 5 |
| 74. Onion routing and blinded paths — planned | 8, 11, 16 | BOLT 4's onion and blinded-path vectors match. A tampered onion fails its HMAC. | BOLT 4 |
| 75. Gossip and the channel graph — planned | 19, 67 | Announcements from a pinned gossip snapshot verify. An announcement with a bad signature refuses. | BOLT 7 |
| 76. Find a route and pay — planned | 72, 74, 75 | A payment routes over a three-node regtest network and settles. A failure returns its error to the sender. | BOLT 2, BOLT 4 |
| 77. Invoices — planned | 22, 27 | BOLT 11's examples decode to their stated fields and payee. A changed character refuses. | BOLT 11 |
| 78. Offers — planned | 5, 24, 67 | BOLT 12's offer, invoice-request and invoice vectors decode and verify. | BOLT 12 |

### Part XIV. Interop and verification

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 79. Test vectors and their provenance — planned | — | Every vector the book uses is pinned with its source and hash, and the pin check fails on an edited vector. | — |
| 80. Differential tests — planned | 24, 36, 43 | Signatures, signature hashes and script results agree with libsecp256k1 and Bitcoin Core on generated inputs. | — |
| 81. Fuzzing — planned | 34, 58, 77 | Every parser (transactions, scripts, PSBTs, addresses and invoices) runs under the fuzzer with no crash and no accepted malformed input. | — |
| 82. Formal properties — planned | 50 | Each named property (round trips, refusal of non-canonical encodings, the bounded validation profile) has a checked model or a stated boundary. | — |

### Part XV. Bitcoin in OpenAgents

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| 83. One seed for Nostr and Bitcoin — planned | 31, 33 | One BIP-39 phrase gives the Nostr key and the wallet keys, and restoring from the phrase gives the same addresses. | BIP-39, NIP-06 |
| 84. Agent treasuries — planned | 58, 63, 64 | A 2-of-3 treasury spend is signed by independent key agents and broadcast on signet, and an independent wallet tool recovers the coins. | NIP-KEYS |
| 85. Paying for requests — planned | 76, 77 | A request pays a BOLT 11 invoice, and the server releases its response only after settlement. | x402, BOLT 11 |
