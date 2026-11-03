# Bitcoin From Scratch

Building Bitcoin in [Coax](../../coax/book/README.md).

This book builds Bitcoin in Coax from bytes up. It starts with big integers
and the constant-time rules, builds SHA-256, RIPEMD-160, HMAC, HKDF, ChaCha20,
secp256k1, ECDSA and BIP-340 Schnorr signatures from their specifications,
then Base58Check, Bech32 and Bech32m, addresses, descriptors, seed phrases and
hierarchical keys. On those it builds transactions, the script interpreter,
every signature hash and Taproot. Parts IX to XIV build a full node:
every consensus rule, the chain state and its storage, the P2P network with
BIP-324 encryption, initial block download, the mempool and relay policy,
and the RPC interface. The node syncs mainnet from genesis to the same UTXO
set as Bitcoin Core. On the node the book builds PSBTs and wallets,
multi-party signatures (MuSig2, FROST and adaptor signatures), and Lightning
in Coax: messages, the Noise handshake, channels, onions, gossip, payments,
invoices and offers. The last parts check the stack against published
vectors, Bitcoin Core and other implementations, and show where OpenAgents
uses it.

[Nostr From Scratch](../../nostr/book/README.md) is the companion book. Nostr
uses Bitcoin's curve, hashes, signatures and Bech32, so it starts after this
book's Parts I–V. Chapters 1–4, 8, 9, 11, 13–16, 18, 23, 24 and 27 were Nostr
chapters 1–13, 15 and 18; they moved here on 2026-11-02.

You need the Coax language (its book's Parts I–III). The [plan](PLAN.md)
gives the order and each chapter's dependencies, done criteria and
specifications. The [contents](SUMMARY.md) list the written chapters.

## Rules

- No outside cryptography, Bitcoin or Lightning library. libsecp256k1,
  Bitcoin Core and other implementations appear only in tests and
  comparisons, never in the code the book builds.
- Secrets never decide a branch or an index ([chapter 3](ch003-constant-time-rules.md)).
- Every chapter names its BIP, BOLT or RFC and the test vectors that pin it.
- Bitcoin only, following [AGENTS.md](../../AGENTS.md): on-chain Bitcoin and
  Lightning are built; Cashu, Fedimint and other chains are not.
- The node must agree with Bitcoin Core on every block. Every consensus
  rule has a row in chapter 57's rule table, and the node decides no money
  for OpenAgents on its own until chapters 80, 93 and 118 are done
  ([plan](PLAN.md#the-full-node)).

## Status

Of the 124 chapters, 32 are implemented and 92 are planned. The 32
implemented chapters have runnable examples, qualified on native CPU, and 27
of them on Wasm. Seventeen of them are the cryptography Nostr already used;
chapters 5, 6, 7, 10, 12, 17, 19, 20, 21, 22, 26, 28, 34, 46 and 47 are new.

The code is under `coax/lib/crypto/` and `coax/lib/bitcoin/`, with examples in
`bitcoin/examples/` and their expected output in `bitcoin/fixtures/`. Code
that later chapters will teach already exists in places: the BIP-143 and
BIP-341 signature hashes (`coax/lib/bitcoin/sighash.coax`), written for Nostr
chapter 104's agent treasuries. Its transaction reader now runs on chapter
34's.

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
| [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md) | Runnable: native C | SHA-256d and the P2P checksum, with the genesis block hash and pinned mainnet txids matching their published values, and the BIP-340 and BIP-341 tagged hashes by tag ID, checked against BIP-341's leaf hashes, Merkle roots, tweaks and key-path sigHashes. |
| [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md) | Runnable: native C | Streaming RIPEMD-160 over caller-owned state and HASH160 on SHA-256, checked against the paper's nine test values, every parent fingerprint in BIP-32 vectors 1–3 and Python's `hashlib`. |
| [7. SHA-512](ch007-sha-512.md) | Runnable: native C and wasm | Streaming SHA-512 over caller-owned state, checked against all 257 NIST CAVS ShortMsg and LongMsg vectors natively and in Wasm, with the one-shot, HMAC-SHA512 and PBKDF2 calls that BIP-39 and BIP-32 use. |
| [8. HMAC-SHA256](ch008-hmac-sha256.md) | Runnable: native C and wasm | HMAC-SHA256 with keys longer than a block hashed first, checked against RFC 4231 cases 1–7 including the truncated case. |
| [9. HKDF](ch009-hkdf.md) | Runnable: native C and wasm | HKDF-SHA256 extract and expand with output bounds, checked against RFC 5869 cases 1–3, and the exact NIP-44 v2 use: extract with salt `nip44-v2`, expand to 76 bytes. |
| [10. SipHash-2-4](ch010-siphash-2-4.md) | Runnable: native C and wasm | SipHash-2-4 with a 16-byte key and an 8-byte tag, checked against all 64 reference vectors natively and in Wasm, Bitcoin Core's salted-hash case and a separate Python SipHash. |
| [11. ChaCha20](ch011-chacha20.md) | Runnable: native C and wasm | RFC 8439 ChaCha20 with caller-supplied nonce and counter, counter exhaustion refused, and HChaCha20 for NIP-49's XChaCha20. |
| [12. Poly1305 and ChaCha20-Poly1305](ch012-poly1305-and-chacha20-poly1305.md) | Runnable: native C and wasm | Streaming Poly1305 over caller-owned state and the RFC 8439 AEAD with a 12-byte nonce, checked against RFC 8439 sections 2.5.2 and 2.8.2, appendices A.3 to A.5, all 325 Wycheproof cases and OpenSSL, natively and in Wasm. A tag one bit wrong refuses and releases no plaintext. |

### Part III. secp256k1 (chapters 13–18)

The curve from field arithmetic to scalar multiplication, point encodings and keys.

| Chapter | Status | Result |
| --- | --- | --- |
| [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md) | Runnable: native C and wasm | Addition, multiplication, squaring, inversion and square roots in F_p for p = 2^256 − 2^32 − 977, with canonical outputs and exact addition chains for inverse and root. |
| [14. Scalars mod n](ch014-scalars-mod-n.md) | Runnable: native C and wasm | Scalars mod `n` in eight 32-bit limbs: `e` reduced mod `n` with one subtraction, keys of 0 or at least `n` and `s` at least `n` refused with their own status, masked add, subtract and negate, Montgomery multiplication and inversion as `a^(n-2)`. |
| [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md) | Runnable: native C and wasm | Points on y² = x³ + 7 in affine and Jacobian coordinates, doubling and addition with the point at infinity, equal points and the P + (−P) case selected by masks, never by a branch. |
| [16. Scalar multiplication](ch016-scalar-multiplication.md) | Runnable: native C and wasm | Three paths: a variable-time `a·G + b·P` for verification, a fixed-base comb for a secret `k·G` (8 teeth by 32 natively, 7 by 37 in Wasm) and a fixed-work signed radix-16 ladder for a secret `k·P`, all equal on random and edge scalars. |
| [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md) | Runnable: native C | One codec for SEC 1 compressed and uncompressed keys and BIP-340 x-only keys, checked against Bitcoin Core, Wycheproof, BIP-32, BIP-340 and BIP-341 keys and a Python reference. Hybrid keys, infinity, coordinates at least p and points off the curve refuse. |
| [18. Keys and randomness](ch018-keys-and-randomness.md) | Runnable: native C and wasm | Private keys from host entropy with rejection of 0 and values at least n, the entropy contract per host, and one signer-request contract for a local key, a NIP-07 browser signer and a phone over NIP-46. |

### Part IV. Signatures (chapters 19–25)

ECDSA as Bitcoin uses it, BIP-340 Schnorr signatures and batch verification.

| Chapter | Status | Result |
| --- | --- | --- |
| [19. Verify ECDSA](ch019-verify-ecdsa.md) | Runnable: native C and wasm | ECDSA verification over a 32-byte hash with the low-S rule as the caller's choice and one strict DER reader, checked against all 464 Wycheproof `ecdsa_secp256k1_sha256_bitcoin_test.json` cases, Python integers and libsecp256k1, natively and in Wasm. A high-S signature refuses under the low-S rule. |
| [20. Sign with ECDSA](ch020-sign-with-ecdsa.md) | Runnable: native C and wasm | RFC 6979 signing with low-S, equal byte for byte to libsecp256k1 on all 2,020 signing rows of tiny-secp256k1's `ecdsa.json`, natively and in Wasm, with every signature verified by libsecp256k1, Python and chapter 19's verifier. A Memcheck secret probe finds one key-dependent branch, the declared validity decision. |
| [21. Strict DER](ch021-strict-der.md) | Runnable: native C and wasm | BIP-66's `IsValidSignatureEncoding` rule by rule, with a refusal code for each, the low-S and sighash-type policy checks and Core's lax parser. All 89 DER rows of Bitcoin Core's `script_tests.json` give their expected result, and 38,088 encodings agree with Python and libsecp256k1's strict and lax parsers, natively and in Wasm. |
| [22. Public key recovery and signed messages](ch022-public-key-recovery-and-signed-messages.md) | Runnable: native C and wasm | ECDSA public key recovery with all four recovery IDs, a recoverable signer that shares chapter 20's body, and Bitcoin Core's `signmessage` and `verifymessage`, checked against libsecp256k1's recovery module on all 2,020 tiny-secp256k1 rows, Bitcoin Core's own cases and Python, natively and in Wasm. A recovery ID out of range and a header byte outside 27 to 34 refuse. |
| [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md) | Runnable: native C and wasm | BIP-340 verification over byte-view messages, checked against all 19 rows of the official `test-vectors.csv`, including messages of 0, 1, 17 and 100 bytes, with a Nostr wrapper that requires 32-byte event IDs. |
| [24. Sign with Schnorr](ch024-sign-with-schnorr.md) | Runnable: native C and wasm | BIP-340 signing with auxiliary randomness, a nonce that never repeats for different messages, and fixed-work self-verification before a signature is released. |
| 25. Batch verification | Planned | A batch of the valid BIP-340 vectors verifies; a batch with one invalid signature refuses. Batch and single verification agree on random batches. |

### Part V. Encodings and addresses (chapters 26–30)

Base58Check, Bech32 and Bech32m, addresses and output descriptors.

| Chapter | Status | Result |
| --- | --- | --- |
| [26. Base58Check](ch026-base58check.md) | Runnable: native C and wasm | Base58 and Base58Check with the P2PKH, P2SH and WIF version bytes, checked against all of Bitcoin Core's `base58_encode_decode.json`, the Base58 entries of `key_io_valid.json` and `key_io_invalid.json`, BIP-32's extended public keys and a Python implementation, natively and in Wasm. WIF keys encode and decode in constant time. A one-character change fails the checksum. |
| [27. Bech32](ch027-bech32.md) | Runnable: native C | BIP-173 Bech32 (checksum constant 1, never Bech32m) with the pinned vectors and canonical lowercase re-encoding. |
| [28. Bech32m](ch028-bech32m.md) | Runnable: native C and wasm | BIP-350 Bech32m (constant `0x2bc830a3`) added to chapter 27's code, and segwit address encoding and decoding with BIP-350's rule (Bech32 for version 0, Bech32m for 1–16), checked against every BIP-350 and BIP-173 vector, Bitcoin Core's Bech32 key I/O entries and BIP-350's reference Python implementation, natively and in Wasm. Twelve refusals have codes of their own. |
| 29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR | Planned | Every BIP-173 and BIP-350 segwit address vector and pinned P2PKH and P2SH addresses map to their scriptPubKeys and back. |
| 30. Output descriptors and their checksum | Planned | The BIP-380 checksum vectors and the BIP-381–386 examples parse and give their stated scripts. |

### Part VI. Keys from words (chapters 31–33)

BIP-39 seed phrases, BIP-32 hierarchical keys and the standard derivation paths.

| Chapter | Status | Result |
| --- | --- | --- |
| [31. Seed phrases](ch031-seed-phrases.md) | Runnable: native C and wasm | BIP-39 English phrases of 12 to 24 words to entropy and back, and the seed through PBKDF2-HMAC-SHA512, checked against all 24 English vectors of trezor/python-mnemonic, Python's `hashlib` and OpenSSL, natively and in Wasm. Word lookups read the whole list; non-ASCII text refuses instead of skipping NFKD. A phrase with a bad checksum refuses. |
| [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md) | Runnable: native C and wasm | BIP-32 master keys, CKDpriv, CKDpub, fingerprints and the 78-byte `xprv`, `xpub`, `tprv` and `tpub` serialization, checked against BIP-32 test vectors 1–5, the `xprv` column of trezor/python-mnemonic's vectors and a separate Python implementation, natively and in Wasm. Private derivation and `xprv` text run in constant time; every vector 5 key refuses with its own code. |
| 33. Derivation paths | Planned | The BIP-44, BIP-84 and BIP-86 vectors give their addresses, and NIP-06's path gives the Nostr book's chapter 46 key. |

### Part VII. Transactions and script (chapters 34–40)

Transactions, their IDs, the script interpreter, signature hashes, timelocks and fees.

| Chapter | Status | Result |
| --- | --- | --- |
| [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md) | Runnable: native C and wasm | Every transaction in Bitcoin Core's `tx_valid.json` and `tx_invalid.json` and six pinned mainnet transactions read with Core's rules and write back byte for byte, with and without witnesses, with their txids, wtxids and weights, checked against a Python transliteration of Core natively and in Wasm. Non-canonical CompactSizes, counts above `MAX_SIZE`, unknown flags, empty witnesses and trailing bytes refuse with their own codes, and bytes that read both ways decode as Core's `DecodeTx` decodes them. |
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

### Part IX. Blocks and consensus (chapters 46–57)

Merkle trees, headers and proof of work, the chain parameters, every consensus rule with the chapter that checks it, and the UTXO set.

| Chapter | Status | Result |
| --- | --- | --- |
| [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) | Runnable: native C and wasm | Merkle roots of blocks 100,000, 514,999 and 413,567 match their headers, with Core's `mutated` flag on every equal pair. A block with its last transaction repeated keeps its header and is refused as mutated (CVE-2012-2459). Branches build and verify for SPV proofs, the witness root reaches block 514,999's commitment, and a 64-byte transaction shows why a proof needs the true count; checked against a Python transliteration of Core natively and in Wasm. |
| [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md) | Runnable: native C and wasm | The genesis header, blocks 32,250 to 32,260 across the first retarget and blocks 100,000, 514,999 and 900,000 hash below their targets and link, and any one of their 640 header bits changed fails. `nBits` decodes and encodes as Bitcoin Core's `SetCompact` and `GetCompact` on all of Core's cases, refusals come in Core's order, and the work of each header is `2^256 / (target + 1)`; checked against a Python transliteration of Core natively and in Wasm. |
| 48. Compact targets, retargeting and chainwork | Planned | `nBits` round-trips for every mainnet retarget to a pinned height, computed retargets match the chain, and chainwork matches Bitcoin Core's `getblockheader`. |
| 49. The witness commitment | Planned | The coinbase commitments of pinned segwit blocks match their computed witness roots. |
| 50. Chain parameters: mainnet, testnet4, signet and regtest | Planned | Each network's genesis block hashes to its published hash, and its message start, default port, address prefixes and deployment heights match Bitcoin Core's `chainparams.cpp` at the pinned release. A signet block whose solution fails the network's challenge refuses. |
| 51. Header rules: time, versions and deployments | Planned | Median time past and the two-hour future limit decide pinned headers as Bitcoin Core does, and testnet4's timewarp rule refuses its counterexample. The buried deployments (BIP-34, BIP-65, BIP-66, CSV, segwit and Taproot) activate at the chain parameters' heights, and a header whose version is below an active deployment's minimum refuses. |
| 52. Transaction checks outside script | Planned | Every case in `tx_invalid.json` that fails before the script interpreter refuses with its reason: no inputs or outputs, a value out of range, an overflowing total (CVE-2010-5139), duplicate inputs (CVE-2018-17144), a coinbase spent before 100 confirmations and a non-final transaction. |
| 53. Sigops, weight and block limits | Planned | Legacy, P2SH and witness signature-operation costs of pinned transactions and blocks match Bitcoin Core. A block over 4,000,000 weight units or 80,000 sigop cost refuses. |
| 54. The subsidy and the coinbase | Planned | The subsidy at every halving matches, and is zero from height 6,930,000. A coinbase that claims one satoshi more than subsidy plus fees refuses, and so does a coinbase without its BIP-34 height after activation. |
| 55. Script flags by height and their exceptions | Planned | The script verification flags for every block of a pinned mainnet range match Bitcoin Core's, including the BIP-16 and Taproot exception blocks, and the two blocks exempt from BIP-30 are handled as Core handles them. |
| 56. The UTXO set and MuHash3072 | Planned | MuHash3072 of pinned UTXO sets matches Bitcoin Core's `gettxoutsetinfo muhash`. Adding then removing an element restores the hash. |
| 57. Validate a block: every consensus rule | Planned | Pinned mainnet, testnet4 and signet blocks, among them every soft-fork activation block, validate against the outputs they spend. A crafted block for each consensus rule refuses with Bitcoin Core's reject reason, and the chapter's rule table names the chapter that checks every rule. |

### Part X. Chain state and storage (chapters 58–64)

Connecting and disconnecting blocks, the block index, reorganizations, block files, a key-value store of our own, the coins cache and pruning.

| Chapter | Status | Result |
| --- | --- | --- |
| 58. Connect and disconnect a block | Planned | Connecting then disconnecting each block of a pinned mainnet range restores the UTXO set's MuHash, using only the undo data the connect step wrote. Undo data with a wrong spent output refuses. |
| 59. The block index and the most-work chain | Planned | A header chain from genesis to a pinned height selects the most-work tip; a fork with less work loses. A header that fails a header rule is marked invalid with all its descendants. |
| 60. Reorganizations and invalid chains | Planned | On regtest a fork with more work replaces the tip: the node disconnects to the fork point and connects the new branch, and its UTXO set matches Bitcoin Core's after the same reorganization. A fork that contains an invalid block never becomes the tip. |
| 61. Block files and undo files | Planned | Blocks and undo data are appended to files and read back by position, and the node imports Bitcoin Core's `blk*.dat` files. A file cut mid-record at any byte is detected at start and its damaged tail dropped. |
| 62. A key-value store for the chainstate | Planned | Reads, writes, deletes and ordered iteration match a reference map on random workloads. A crash at every write of a recorded workload restarts to exactly the last committed batch. |
| 63. The coins cache and crash-safe flushes | Planned | A crash at any point of a flush restarts to a consistent tip and replays to the same MuHash. The cache stays under its configured memory limit. |
| 64. Pruning | Planned | A pruned node keeps its block files under the configured target, keeps at least the last 288 blocks, and still reorganizes across them. |

### Part XI. The network (chapters 65–75)

P2P messages, BIP-324 encrypted transport, the handshake, peer discovery and connections, denial-of-service limits, headers sync and block download, compact blocks, filters, and Tor and I2P.

| Chapter | Status | Result |
| --- | --- | --- |
| 65. P2P messages and checksums | Planned | Captured `version`, `inv`, `tx` and `block` messages parse and re-serialize exactly. A bad checksum refuses. |
| 66. ElligatorSwift | Planned | BIP-324's `ellswift_decode_test_vectors.csv` and `xswiftec_inv_test_vectors.csv` pass. |
| 67. The v2 encrypted transport | Planned | BIP-324's `packet_encoding_test_vectors.csv` give every ciphertext, including rekeying. |
| 68. The handshake and feature negotiation | Planned | The node completes `version` and `verack` with Bitcoin Core over the v1 and v2 transports and negotiates `wtxidrelay`, `sendaddrv2`, `sendheaders`, `sendcmpct` and `feefilter`. A feature message after `verack` is handled as Core handles it. |
| 69. Addresses and the address manager | Planned | `addr` and `addrv2` messages round-trip for every network BIP-155 names. The address manager's buckets keep one source from filling its tables, and the eclipse cases ported from Bitcoin Core's address-manager tests pass. DNS seeds are queried only when the manager has no usable address. |
| 70. Connections: outbound, block-relay-only, feelers and eviction | Planned | The node keeps 8 full-relay and 2 block-relay-only outbound connections and makes feeler connections. With its inbound slots full it evicts by Bitcoin Core's protected-peer rules, and Core's eviction cases pass. |
| 71. Misbehavior, limits and denial of service | Planned | Each message limit (size, `inv` and `addr` counts and rates, unconnecting headers and the low-work headers presync) refuses its oversized case. A peer that sends an invalid block is disconnected, and memory per peer stays under its stated bound under a flood. |
| 72. Headers sync and block download | Planned | From genesis the node downloads headers by locator, presyncs them against low-work spam, and fetches blocks from many peers through a moving window, replacing a peer that stalls. It syncs regtest and signet from Bitcoin Core peers to their tips. |
| 73. Compact blocks | Planned | Short IDs and block reconstruction for pinned blocks match Bitcoin Core. |
| 74. Compact block filters | Planned | BIP-158's `testnet-19.json` filters and filter headers match. |
| 75. Proxies, Tor and I2P | Planned | The node reaches peers through a SOCKS5 proxy and a local Tor daemon, accepts inbound connections on an onion service, and connects to I2P peers through the SAM bridge. Each network's addresses stay apart in the address manager. |

### Part XII. Initial block download (chapters 76–80)

From genesis to the tip: parallel script checks, faster verification, assumevalid, AssumeUTXO, and a mainnet sync whose UTXO set matches Bitcoin Core's.

| Chapter | Status | Result |
| --- | --- | --- |
| 76. The signature cache and parallel script checks | Planned | Script checks run on every core and give the same verdicts as one thread over a pinned block range. A transaction verified on mempool entry is not verified again in its block, and the cache is keyed by a salted hash. |
| 77. Faster verification: the GLV endomorphism | Planned | Verification with the endomorphism and wider windows agrees with chapter 16's path on random and edge inputs, and a pinned block's signatures verify at a measured rate within a stated factor of libsecp256k1 on the same machine. |
| 78. assumevalid | Planned | Below the pinned assumevalid block the node skips script checks only for ancestors of that block buried under its stated work; every other rule still runs. With assumevalid off the node reaches the same UTXO set. |
| 79. AssumeUTXO snapshots | Planned | The node loads a pinned mainnet UTXO snapshot whose hash matches the chain parameters, follows the tip from it at once, and validates from genesis in the background. A snapshot with one changed coin refuses. |
| 80. Sync mainnet from genesis | Planned | The node syncs mainnet from genesis to a pinned height over the public network, and its UTXO set at that height equals Bitcoin Core's `gettxoutsetinfo` (`hash_serialized_3` and `muhash`). The chapter records time, memory and disk on stated hardware next to Bitcoin Core's on the same machine. |

### Part XIII. The mempool and policy (chapters 81–88)

Standardness, mempool acceptance, transaction relay, replacement, packages, eviction, fee estimation and block templates.

| Chapter | Status | Result |
| --- | --- | --- |
| 81. Policy is not consensus: standardness | Planned | Every standardness rule (output types, size, dust, `OP_RETURN` data and witness limits) refuses its pinned non-standard transaction with Bitcoin Core's reason at the pinned release, and each of those transactions still validates in a block. |
| 82. Mempool acceptance and limits | Planned | Accept and refuse decisions and reasons match Bitcoin Core's `testmempoolaccept` on a pinned corpus, and the mempool keeps the size and package limits of the pinned release. |
| 83. Transaction relay and orphans | Planned | Transactions announced by wtxid are requested, accepted and announced onward with Bitcoin Core's randomized delays. A transaction with a missing parent waits in a bounded orphan pool and is accepted when the parent arrives. |
| 84. Replacement: RBF and TRUC | Planned | Replacements are accepted or refused as Bitcoin Core decides on its replacement cases, including the fee-rate and total-fee rules and the version-3 (TRUC) topology limits. |
| 85. Packages, CPFP and anchors | Planned | One-parent-one-child packages, and pay-to-anchor outputs with zero-fee parents, are accepted and relayed as Bitcoin Core accepts them. A package below the minimum fee rate refuses. |
| 86. Eviction, expiry and persistence | Planned | A full mempool evicts the lowest fee-rate transactions first and raises its minimum fee, transactions expire after two weeks, and a restart reloads the saved mempool. |
| 87. Fee estimation | Planned | Over a replayed history of blocks and mempool entries, estimates for 2, 6 and 144 blocks meet the success rate Bitcoin Core's `estimatesmartfee` targets. |
| 88. Block templates and mining | Planned | `getblocktemplate` chooses transactions by fee rate within the weight and sigop limits, and a regtest block mined from its template is accepted by Bitcoin Core. |

### Part XIV. Operating the node (chapters 89–93)

The node as a program: configuration and shutdown, the RPC and REST interface, indexes, notifications, and shadow mode beside Bitcoin Core.

| Chapter | Status | Result |
| --- | --- | --- |
| 89. The node process: configuration, data directory, logs and shutdown | Planned | The node reads its configuration file and flags, keeps its state in one data directory, logs by category, and on a signal at any point shuts down cleanly and restarts at the same tip. |
| 90. JSON-RPC and REST | Planned | The node answers the Bitcoin Core RPCs that wallets and Lightning implementations call (`getblockchaininfo`, `getblock`, `getblockheader`, `getrawtransaction`, `gettxout`, `sendrawtransaction`, `estimatesmartfee`, `getblocktemplate`, `submitblock` and the others the chapter names) with Core's JSON for the same chain, behind cookie authentication. |
| 91. Indexes: transactions, filters and coin statistics | Planned | With each index on, `getrawtransaction` finds any confirmed transaction, the node serves BIP-157 filters to light clients, and `gettxoutsetinfo` answers for past heights, each matching Bitcoin Core. |
| 92. Notifications for wallets and Lightning | Planned | A subscriber receives every connected and disconnected block and every mempool entry and removal in order, across a reorganization, and resumes from its last block after a restart. |
| 93. Shadow mode beside Bitcoin Core | Planned | Run beside a Bitcoin Core node on the same peers for a stated period, the node gives the same verdict on every header and block and holds the same tip. Any difference stops it and records the block. |

### Part XV. Wallets (chapters 94–99)

PSBTs, coin selection, message signing, silent payments, miniscript and a descriptor wallet on our node.

| Chapter | Status | Result |
| --- | --- | --- |
| 94. PSBT | Planned | Every valid and invalid PSBT in BIP-174 and BIP-370 parses or refuses as stated. |
| 95. Coin selection and change | Planned | Branch-and-bound and fallback selection pass Bitcoin Core's coin-selection cases, and no selection pays more than its stated fee rate allows. |
| 96. Message signing | Planned | BIP-322's simple and full vectors verify. |
| 97. Silent payments | Planned | BIP-352's `send_and_receive_test_vectors.json` give every output and scan result. |
| 98. Miniscript | Planned | The BIP-379 examples type-check, compile to their stated scripts and satisfy. |
| 99. A descriptor wallet on our node | Planned | A descriptor wallet finds its coins by rescan and by notification, builds, signs and broadcasts a spend on signet through our node, and survives a reorganization that removes a payment. |

### Part XVI. Many signers (chapters 100–103)

Multisig, MuSig2, FROST threshold signatures and adaptor signatures.

| Chapter | Status | Result |
| --- | --- | --- |
| 100. Multisig | Planned | 2-of-3 P2WSH `sortedmulti` descriptors give their addresses, and a spend signed by two keys validates. |
| 101. MuSig2 | Planned | Every BIP-327 vector file (key aggregation, nonces, signing, aggregation and tweaks) gives its result. A reused nonce refuses. |
| 102. FROST threshold signatures | Planned | The FROST draft BIP's vectors give their results. A signature from fewer than t signers refuses. |
| 103. Adaptor signatures | Planned | An adaptor signature verifies against its point, completes with the secret, and the secret is recovered from the final signature. |

### Part XVII. Lightning (chapters 104–115)

Lightning built in Coax on our node: messages, the Noise handshake, channels, onions, gossip, payments, invoices and offers.

| Chapter | Status | Result |
| --- | --- | --- |
| 104. Messages and TLVs | Planned | BOLT 1's TLV vectors give their results. An unknown even type refuses. |
| 105. The Noise handshake | Planned | BOLT 8's handshake and message vectors match byte for byte, including key rotation. |
| 106. Commitment keys and revocation | Planned | BOLT 3's key-derivation vectors give every key. |
| 107. Shachain | Planned | BOLT 3's per-commitment secret storage vectors pass, including the cases that must fail. |
| 108. Commitment and HTLC transactions | Planned | BOLT 3's commitment and HTLC transaction vectors match byte for byte, with and without anchors. |
| 109. Open, update and close a channel | Planned | Two Coax nodes open, update and close a channel on regtest. The state machine refuses every out-of-order message BOLT 2 names. |
| 110. Watch the chain | Planned | A revoked commitment broadcast on regtest is swept by the penalty transaction, and an expired HTLC times out, with our node as the only chain source. |
| 111. Onion routing and blinded paths | Planned | BOLT 4's onion and blinded-path vectors match. A tampered onion fails its HMAC. |
| 112. Gossip and the channel graph | Planned | Announcements from a pinned gossip snapshot verify. An announcement with a bad signature refuses. |
| 113. Find a route and pay | Planned | A payment routes over a three-node regtest network and settles. A failure returns its error to the sender. |
| 114. Invoices | Planned | BOLT 11's examples decode to their stated fields and payee. A changed character refuses. |
| 115. Offers | Planned | BOLT 12's offer, invoice-request and invoice vectors decode and verify. |

### Part XVIII. Interop and verification (chapters 116–120)

Test vector provenance, differential tests, Bitcoin Core's functional tests against our node, fuzzing and formal properties.

| Chapter | Status | Result |
| --- | --- | --- |
| 116. Test vectors and their provenance | Planned | Every vector the book uses is pinned with its source and hash, and the pin check fails on an edited vector. |
| 117. Differential tests | Planned | Signatures, signature hashes, script results and block verdicts agree with libsecp256k1 and Bitcoin Core on generated inputs. |
| 118. Bitcoin Core's functional tests against our node | Planned | The Bitcoin Core functional tests that drive a node over P2P and RPC (`feature_block.py` and the `p2p_*` and `mempool_*` tests the chapter names) pass against our node on one of our own machines, never the owner's Mac. Each test that does not apply is listed with its reason. |
| 119. Fuzzing | Planned | Every parser (transactions, scripts, P2P messages, PSBTs, addresses and invoices) runs under the fuzzer with no crash and no accepted malformed input. |
| 120. Formal properties | Planned | Each named property (round trips, refusal of non-canonical encodings and the consensus rule table) has a checked model or a stated boundary. |

### Part XIX. Bitcoin in OpenAgents (chapters 121–124)

Where OpenAgents uses the stack: our node on Pylons and Verse OS, one seed, agent treasuries and paid requests.

| Chapter | Status | Result |
| --- | --- | --- |
| 121. One seed for Nostr and Bitcoin | Planned | One BIP-39 phrase gives the Nostr key and the wallet keys, and restoring from the phrase gives the same addresses. |
| 122. Agent treasuries | Planned | A 2-of-3 treasury spend is signed by independent key agents and broadcast on signet, and an independent wallet tool recovers the coins. |
| 123. Paying for requests | Planned | A request pays a BOLT 11 invoice, and the server releases its response only after settlement. |
| 124. Our node on Pylons and Verse OS | Planned | A Pylon and a Verse OS machine run the node from its Nix module and sync it, and OpenAgents' on-chain checks query it instead of a third-party chain source. |
