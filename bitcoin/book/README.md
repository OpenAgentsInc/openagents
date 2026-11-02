# Bitcoin From Scratch

Building Bitcoin in [Coax](../../coax/book/README.md).

This book builds Bitcoin in Coax from bytes up. It starts with big integers
and the constant-time rules, builds SHA-256, RIPEMD-160, HMAC, HKDF, ChaCha20,
secp256k1, ECDSA and BIP-340 Schnorr signatures from their specifications,
then Base58Check, Bech32 and Bech32m, addresses, descriptors, seed phrases and
hierarchical keys. On those it builds transactions, the script interpreter,
every signature hash, Taproot, block headers and proof of work, a bounded
block-validation profile, the P2P layer with BIP-324 encryption, PSBTs and
wallets, and multi-party signatures (MuSig2, FROST and adaptor signatures).
Part XIII builds Lightning in Coax: messages, the Noise handshake, channels,
onions, gossip, payments, invoices and offers. The last parts check the stack
against published vectors and other implementations, and show where
OpenAgents uses it.

[Nostr From Scratch](../../nostr/book/README.md) is the companion book. Nostr
uses Bitcoin's curve, hashes, signatures and Bech32, so it starts after this
book's Parts I–V. Chapters 1–4, 8, 9, 11, 13–16, 18, 23, 24 and 27 were Nostr
chapters 1–13, 15 and 18; they moved here on 2026-11-02.

You need the Coax language (its book's Parts I–III). The [plan](PLAN.md)
gives the order and each chapter's dependencies, done criteria and
specifications. The [contents](SUMMARY.md) list the written chapters.

## Rules

- No outside cryptography, Bitcoin or Lightning library. libsecp256k1,
  Bitcoin Core and other implementations appear only in Part XIV's tests,
  never in the code the book builds.
- Secrets never decide a branch or an index ([chapter 3](ch003-constant-time-rules.md)).
- Every chapter names its BIP, BOLT or RFC and the test vectors that pin it.
- Bitcoin only, following [AGENTS.md](../../AGENTS.md): on-chain Bitcoin and
  Lightning are built; Cashu, Fedimint and other chains are not.
- The consensus chapters check data against vectors and state the rules
  their profile leaves out. They are not a node to replace Bitcoin Core.

## Status

Of the 85 chapters, 16 are implemented and 69 are planned. The 16
implemented chapters have runnable examples, qualified on native CPU, and 13
of them on Wasm. Fifteen of them are the cryptography Nostr already used;
chapter 6 is the first new one.

The code is under `coax/lib/crypto/` and `coax/lib/bitcoin/`, with examples in
`bitcoin/examples/` and their expected output in `bitcoin/fixtures/`. Code
that later chapters will teach already exists in places: ECDSA signing
(`coax/lib/crypto/ecdsa_sign.coax`), BIP-39 and BIP-32 (`bip39.coax`,
`bip32.coax`), Poly1305 (`poly1305.coax`), and the BIP-143 and BIP-341
signature hashes (`coax/lib/bitcoin/sighash.coax`), written for Nostr
chapter 104's agent treasuries.

The [chapter registry](chapters.json) records each chapter's status (one of
*implemented*, *partly implemented*, *design draft* or *planned*), its
profile, its dependencies, its implemented source, the targets with recorded
evidence and the code outside the book that uses it. The status table below,
the contents and each chapter's status marker must agree with it;
`python3 psionic/tests/book.py --book bitcoin` fails when they don't.

## Promoting a chapter

A planned chapter becomes implemented when it has a worked calculation, a
runnable example in `bitcoin/examples/` listed in [examples.json](examples.json)
with an exact transcript, a broken case and its refusal, and an independent
check against published vectors or another implementation. Update the
chapter's status marker, its registry entry, its line in the
[contents](SUMMARY.md) and its row below in the same commit.

## Writing style

Write chapters to the
[Google developer documentation style guide](https://developers.google.com/style):
short, plain sentences in active voice and present tense, addressed to "you".
Say what the code does and what was measured. Use code font for code, file
names and identifiers.

## Checking the book

Commands run from the repository root.

```shell
python3 psionic/tests/book.py --book bitcoin
python3 psionic/tests/book.py --book bitcoin --coax coax/bin/coax
```

The first checks structure: chapter files and headings, the contents, local
links, status markers, the chapter registry and the example inventory. The
second also runs every example in `bitcoin/examples/` in debug and release and
compares its output and exit status. Build the compiler once with
`coax/build.sh`.

## Chapters

### Part I. Foundations (chapters 1–3)

Bytes, big integers and the constant-time rules every later chapter follows.

| Chapter | Status | Result |
| --- | --- | --- |
| [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) | Runnable: native C | Caller-owned byte views, big- and little-endian loads and stores, strict lowercase hex and canonical padded base64, with every decoder refusing bad input instead of guessing. |
| [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) | Runnable: native C and wasm | 256-bit numbers as ten 26-bit limbs in `u64` and as eight 32-bit limbs, with carry schedules that never overflow and an independent big-integer oracle. |
| [3. Constant-time rules and secrets](ch003-constant-time-rules.md) | Runnable: native C and wasm | The rules for secret data: no branch, index or variable-time call on a secret, explicit erasure, value barriers, and timing measurements that are evidence, not proof. |

### Part II. Hashes, MACs and ciphers (chapters 4–12)

Every hash, MAC and cipher Bitcoin and Lightning use, each checked against its published vectors.

| Chapter | Status | Result |
| --- | --- | --- |
| [4. SHA-256](ch004-sha-256.md) | Runnable: native C and wasm | Streaming SHA-256 over caller-owned state, checked against every NIST CAVS ShortMsg and LongMsg vector, with split-stream and padding-boundary checks, plus BIP-340 tagged hashes. |
| 5. SHA-256d and tagged hashes | Planned | The txids of pinned mainnet transactions and the genesis block hash match their published values. The tag hashes for `BIP0340/challenge`, `TapLeaf`, `TapBranch`, `TapTweak` and `TapSighash` match the BIP texts. |
| [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md) | Runnable: native C | Streaming RIPEMD-160 over caller-owned state and HASH160 on SHA-256, checked against the paper's nine test values, every parent fingerprint in BIP-32 vectors 1–3 and Python's `hashlib`. |
| 7. SHA-512 | Planned | Every NIST CAVS vector in `SHA512ShortMsg.rsp` and `SHA512LongMsg.rsp` passes. |
| [8. HMAC-SHA256](ch008-hmac-sha256.md) | Runnable: native C and wasm | HMAC-SHA256 with keys longer than a block hashed first, checked against RFC 4231 cases 1–7 including the truncated case. |
| [9. HKDF](ch009-hkdf.md) | Runnable: native C and wasm | HKDF-SHA256 extract and expand with output bounds, checked against RFC 5869 cases 1–3, and the exact NIP-44 v2 use: extract with salt `nip44-v2`, expand to 76 bytes. |
| 10. SipHash-2-4 | Planned | The 64 vectors of the SipHash reference implementation pass. |
| [11. ChaCha20](ch011-chacha20.md) | Runnable: native C and wasm | RFC 8439 ChaCha20 with caller-supplied nonce and counter, counter exhaustion refused, and HChaCha20 for NIP-49's XChaCha20. |
| 12. Poly1305 and ChaCha20-Poly1305 | Planned | RFC 8439 sections 2.5.2 and 2.8.2 vectors pass. A tag that differs in one bit refuses and releases no plaintext. |

### Part III. secp256k1 (chapters 13–18)

The curve from field arithmetic to scalar multiplication, point encodings and keys.

| Chapter | Status | Result |
| --- | --- | --- |
| [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md) | Runnable: native C and wasm | Addition, multiplication, squaring, inversion and square roots in F_p for p = 2^256 − 2^32 − 977, with canonical outputs and exact addition chains for inverse and root. |
| [14. Scalars mod n](ch014-scalars-mod-n.md) | Runnable: native C and wasm | x-only ECDH that validates the peer key and returns the raw shared x, checked against labelled Wycheproof secp256k1 inputs, for NIP-44 conversation keys. |
| [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md) | Runnable: native C and wasm | Points on y² = x³ + 7 in affine and Jacobian coordinates, doubling and complete addition with the point at infinity and the P + (−P) case handled. |
| [16. Scalar multiplication](ch016-scalar-multiplication.md) | Runnable: native C and wasm | `npub`, `nsec` and `note`, and the TLV forms `nprofile`, `nevent` and `naddr` with relay hints, plus the `nostr:` URI scheme of NIP-21. |
| 17. Point encodings: compressed, uncompressed and x-only | Planned | Compressed, uncompressed and x-only encodings round-trip for random points. Hybrid encodings (`0x06`, `0x07`) and points off the curve refuse. |
| [18. Keys and randomness](ch018-keys-and-randomness.md) | Runnable: native C and wasm | Private keys from host entropy with rejection of 0 and values at least n, the entropy contract per host, and one signer-request contract for a local key, a NIP-07 browser signer and a phone over NIP-46. |

### Part IV. Signatures (chapters 19–25)

ECDSA as Bitcoin uses it, BIP-340 Schnorr signatures and batch verification.

| Chapter | Status | Result |
| --- | --- | --- |
| 19. Verify ECDSA | Planned | Every case in Wycheproof's `ecdsa_secp256k1_sha256_bitcoin_test.json` gives its expected result. A high-S signature refuses under the low-S rule. |
| 20. Sign with ECDSA | Planned | RFC 6979 nonces match published secp256k1 vectors. Every signature is low-S and verifies in an independent library. A secret-probe run finds no secret-dependent branch. |
| 21. Strict DER | Planned | Every DER case in Bitcoin Core's `script_tests.json` that BIP-66 decides gives its expected result. |
| 22. Public key recovery and signed messages | Planned | Bitcoin Core `signmessage` signatures for pinned keys verify, and recovery returns the signing key. A recovery ID out of range refuses. |
| [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md) | Runnable: native C and wasm | BIP-340 verification over byte-view messages, checked against all 19 rows of the official `test-vectors.csv`, including messages of 0, 1, 17 and 100 bytes, with a Nostr wrapper that requires 32-byte event IDs. |
| [24. Sign with Schnorr](ch024-sign-with-schnorr.md) | Runnable: native C and wasm | BIP-340 signing with auxiliary randomness, a nonce that never repeats for different messages, and fixed-work self-verification before a signature is released. |
| 25. Batch verification | Planned | A batch of the valid BIP-340 vectors verifies; a batch with one invalid signature refuses. Batch and single verification agree on random batches. |

### Part V. Encodings and addresses (chapters 26–30)

Base58Check, Bech32 and Bech32m, addresses and output descriptors.

| Chapter | Status | Result |
| --- | --- | --- |
| 26. Base58Check | Planned | BIP-32 extended keys and pinned P2PKH and P2SH addresses decode and re-encode exactly. A one-character change fails the checksum. |
| [27. Bech32](ch027-bech32.md) | Runnable: native C | BIP-173 Bech32 (checksum constant 1, never Bech32m) with the pinned vectors and canonical lowercase re-encoding. |
| 28. Bech32m | Planned | Every valid and invalid BIP-350 vector gives its expected result, including the witness-version rules. |
| 29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR | Planned | Every BIP-173 and BIP-350 segwit address vector and pinned P2PKH and P2SH addresses map to their scriptPubKeys and back. |
| 30. Output descriptors and their checksum | Planned | The BIP-380 checksum vectors and the BIP-381–386 examples parse and give their stated scripts. |

### Part VI. Keys from words (chapters 31–33)

BIP-39 seed phrases, BIP-32 hierarchical keys and the standard derivation paths.

| Chapter | Status | Result |
| --- | --- | --- |
| 31. Seed phrases | Planned | The BIP-39 English vectors give their entropy, mnemonic and seed through PBKDF2-HMAC-SHA512. A phrase with a bad checksum refuses. |
| 32. Hierarchical deterministic keys | Planned | BIP-32 test vectors 1–5 give every extended key, and vector 5's invalid keys refuse. |
| 33. Derivation paths | Planned | The BIP-44, BIP-84 and BIP-86 vectors give their addresses, and NIP-06's path gives the Nostr book's chapter 46 key. |

### Part VII. Transactions and script (chapters 34–40)

Transactions, their IDs, the script interpreter, signature hashes, timelocks and fees.

| Chapter | Status | Result |
| --- | --- | --- |
| 34. Transactions and their serialization | Planned | Every transaction in Bitcoin Core's `tx_valid.json` and `tx_invalid.json` parses, and serialization round-trips byte for byte with and without witnesses. |
| 35. Txids, wtxids and malleability | Planned | Txids and wtxids of pinned segwit transactions match. Changing a witness changes the wtxid and leaves the txid. |
| 36. The script interpreter | Planned | Every case in Bitcoin Core's `script_tests.json` gives its expected result under its stated flags. |
| 37. Legacy signature hashes | Planned | Every case in Bitcoin Core's `sighash.json` gives its digest, including `SIGHASH_SINGLE` with no matching output. |
| 38. Segwit v0 signature hashes | Planned | BIP-143's examples give their digests for every hash type. |
| 39. Timelocks | Planned | The `CHECKLOCKTIMEVERIFY` and `CHECKSEQUENCEVERIFY` cases in `script_tests.json` and `tx_valid.json` pass, and BIP-68 sequence encodings round-trip. |
| 40. Weight, vsize and fees | Planned | Weight and vsize of pinned transactions match Bitcoin Core's `decoderawtransaction`. |

### Part VIII. Taproot (chapters 41–45)

The key tweak, script trees, the Taproot signature hash and tapscript.

| Chapter | Status | Result |
| --- | --- | --- |
| 41. The key tweak and the key path | Planned | BIP-341's `wallet-test-vectors.json` key-path tweaks and signatures match. |
| 42. Script trees: tapleaves and tapbranches | Planned | BIP-341's script-tree vectors give every leaf hash, Merkle root and output key. |
| 43. Taproot signature hashes | Planned | Every BIP-341 key-path and script-path signature hash vector matches, for every hash type. |
| 44. Tapscript | Planned | Bitcoin Core's Taproot script tests (`script_assets_test.json`) give their expected results. |
| 45. Control blocks and script-path spends | Planned | Script-path spends from BIP-341's vectors verify. A control block with the wrong parity or path refuses. |

### Part IX. Blocks and consensus (chapters 46–52)

Merkle trees, headers and proof of work, a bounded block-validation profile and the UTXO set.

| Chapter | Status | Result |
| --- | --- | --- |
| 46. Merkle trees and the duplicate-leaf flaw | Planned | Merkle roots of pinned mainnet blocks match their headers. A block whose last transaction is duplicated is detected (CVE-2012-2459). |
| 47. Block headers and proof of work | Planned | The genesis header and pinned headers hash below their targets; a header with one changed bit does not. |
| 48. Compact targets, retargeting and chainwork | Planned | `nBits` round-trips for every mainnet retarget to a pinned height, computed retargets match the chain, and chainwork matches Bitcoin Core's `getblockheader`. |
| 49. The witness commitment | Planned | The coinbase commitments of pinned segwit blocks match their computed witness roots. |
| 50. Validate a block (bounded profile) | Planned | Pinned mainnet and signet blocks validate under the bounded profile. Blocks with stated failures refuse with their reasons. The chapter states the consensus rules the profile does not check. |
| 51. The UTXO set and MuHash3072 | Planned | MuHash3072 of pinned UTXO sets matches Bitcoin Core's `gettxoutsetinfo muhash`. Adding then removing an element restores the hash. |
| 52. Headers first and the most-work chain | Planned | A header chain from genesis to a pinned height selects the most-work tip; a fork with less work loses. |

### Part X. The network (chapters 53–57)

P2P messages, BIP-324 encrypted transport, compact blocks and block filters.

| Chapter | Status | Result |
| --- | --- | --- |
| 53. P2P messages and checksums | Planned | Captured `version`, `inv`, `tx` and `block` messages parse and re-serialize exactly. A bad checksum refuses. |
| 54. ElligatorSwift | Planned | BIP-324's `ellswift_decode_test_vectors.csv` and `xswiftec_inv_test_vectors.csv` pass. |
| 55. The v2 encrypted transport | Planned | BIP-324's `packet_encoding_test_vectors.csv` give every ciphertext, including rekeying. |
| 56. Compact blocks | Planned | Short IDs and block reconstruction for pinned blocks match Bitcoin Core. |
| 57. Compact block filters | Planned | BIP-158's `testnet-19.json` filters and filter headers match. |

### Part XI. Wallets (chapters 58–62)

PSBTs, coin selection, message signing, silent payments and miniscript.

| Chapter | Status | Result |
| --- | --- | --- |
| 58. PSBT | Planned | Every valid and invalid PSBT in BIP-174 and BIP-370 parses or refuses as stated. |
| 59. Coin selection and change | Planned | Branch-and-bound and fallback selection pass Bitcoin Core's coin-selection cases, and no selection pays more than its stated fee rate allows. |
| 60. Message signing | Planned | BIP-322's simple and full vectors verify. |
| 61. Silent payments | Planned | BIP-352's `send_and_receive_test_vectors.json` give every output and scan result. |
| 62. Miniscript | Planned | The BIP-379 examples type-check, compile to their stated scripts and satisfy. |

### Part XII. Many signers (chapters 63–66)

Multisig, MuSig2, FROST threshold signatures and adaptor signatures.

| Chapter | Status | Result |
| --- | --- | --- |
| 63. Multisig | Planned | 2-of-3 P2WSH `sortedmulti` descriptors give their addresses, and a spend signed by two keys validates. |
| 64. MuSig2 | Planned | Every BIP-327 vector file (key aggregation, nonces, signing, aggregation and tweaks) gives its result. A reused nonce refuses. |
| 65. FROST threshold signatures | Planned | The FROST draft BIP's vectors give their results. A signature from fewer than t signers refuses. |
| 66. Adaptor signatures | Planned | An adaptor signature verifies against its point, completes with the secret, and the secret is recovered from the final signature. |

### Part XIII. Lightning (chapters 67–78)

Lightning built in Coax: messages, the Noise handshake, channels, onions, gossip, payments, invoices and offers.

| Chapter | Status | Result |
| --- | --- | --- |
| 67. Messages and TLVs | Planned | BOLT 1's TLV vectors give their results. An unknown even type refuses. |
| 68. The Noise handshake | Planned | BOLT 8's handshake and message vectors match byte for byte, including key rotation. |
| 69. Commitment keys and revocation | Planned | BOLT 3's key-derivation vectors give every key. |
| 70. Shachain | Planned | BOLT 3's per-commitment secret storage vectors pass, including the cases that must fail. |
| 71. Commitment and HTLC transactions | Planned | BOLT 3's commitment and HTLC transaction vectors match byte for byte, with and without anchors. |
| 72. Open, update and close a channel | Planned | Two Coax nodes open, update and close a channel on regtest. The state machine refuses every out-of-order message BOLT 2 names. |
| 73. Watch the chain | Planned | A revoked commitment broadcast on regtest is swept by the penalty transaction, and an expired HTLC times out. |
| 74. Onion routing and blinded paths | Planned | BOLT 4's onion and blinded-path vectors match. A tampered onion fails its HMAC. |
| 75. Gossip and the channel graph | Planned | Announcements from a pinned gossip snapshot verify. An announcement with a bad signature refuses. |
| 76. Find a route and pay | Planned | A payment routes over a three-node regtest network and settles. A failure returns its error to the sender. |
| 77. Invoices | Planned | BOLT 11's examples decode to their stated fields and payee. A changed character refuses. |
| 78. Offers | Planned | BOLT 12's offer, invoice-request and invoice vectors decode and verify. |

### Part XIV. Interop and verification (chapters 79–82)

Test vector provenance, differential tests, fuzzing and formal properties.

| Chapter | Status | Result |
| --- | --- | --- |
| 79. Test vectors and their provenance | Planned | Every vector the book uses is pinned with its source and hash, and the pin check fails on an edited vector. |
| 80. Differential tests | Planned | Signatures, signature hashes and script results agree with libsecp256k1 and Bitcoin Core on generated inputs. |
| 81. Fuzzing | Planned | Every parser (transactions, scripts, PSBTs, addresses and invoices) runs under the fuzzer with no crash and no accepted malformed input. |
| 82. Formal properties | Planned | Each named property (round trips, refusal of non-canonical encodings, the bounded validation profile) has a checked model or a stated boundary. |

### Part XV. Bitcoin in OpenAgents (chapters 83–85)

Where OpenAgents uses the stack: one seed, agent treasuries and paid requests.

| Chapter | Status | Result |
| --- | --- | --- |
| 83. One seed for Nostr and Bitcoin | Planned | One BIP-39 phrase gives the Nostr key and the wallet keys, and restoring from the phrase gives the same addresses. |
| 84. Agent treasuries | Planned | A 2-of-3 treasury spend is signed by independent key agents and broadcast on signet, and an independent wallet tool recovers the coins. |
| 85. Paying for requests | Planned | A request pays a BOLT 11 invoice, and the server releases its response only after settlement. |
