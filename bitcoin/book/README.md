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

Of the 125 chapters, 98 are implemented, 4 are partly implemented and 23 are planned. The 98 implemented
chapters have runnable examples, qualified on native CPU, and 32 of them on
Wasm. Fifteen of them are the cryptography Nostr already used; chapters 5, 6,
7, 10, 12, 17, 19, 20, 21, 22, 25, 26, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37,
38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 61, 62,
65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 77, 81, 94, 95, 96, 97, 100, 101,
102, 103, 104, 105, 106, 107, 108, 109, 111, 112, 113, 114, 115, 116, 119, 121
and 122 are new.

Chapter 58 implements full validated connects and disconnects with exact Core
undo, complete inverses and atomic storage commits. Its validated-v1 profile
refuses light metadata. Chapter 60 remains partly implemented for explorer
light reorganizations; validated reorganization integration is next.
Chapter 82 implements bounded, unvalidated local mempool observations and
their fees; script execution and full acceptance/relay policy remain planned.
Chapter 90 implements the read-only explorer HTTP/RPC profile; full node RPC
compatibility and cookie authentication remain planned.
Chapter 91 is partly implemented for its transaction index; filters and
historical coin statistics remain planned.

The code is under `coax/lib/crypto/`, `coax/lib/bitcoin/` and
`coax/lib/lightning/`, with examples in `bitcoin/examples/` and their expected output in `bitcoin/fixtures/`. Code
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
| [25. Batch verification](ch025-batch-verification.md) | Runnable: native C | BIP-340's `BatchVerify`: randomizers from a SHA-256 seed of every input and ChaCha20, one equation for the whole batch, and Strauss's multi-scalar multiplication with chapter 77's endomorphism, which shares the doublings of every signature. Why the randomizers are needed, why a failed batch names no signature, what a batch saves and costs, measured, and why Bitcoin Core verifies each signature alone. The BIP-340 vectors give their stated results in batches, and every line agrees with an independent Python model. |

### Part V. Encodings and addresses (chapters 26–30)

Base58Check, Bech32 and Bech32m, addresses and output descriptors.

| Chapter | Status | Result |
| --- | --- | --- |
| [26. Base58Check](ch026-base58check.md) | Runnable: native C and wasm | Base58 and Base58Check with the P2PKH, P2SH and WIF version bytes, checked against all of Bitcoin Core's `base58_encode_decode.json`, the Base58 entries of `key_io_valid.json` and `key_io_invalid.json`, BIP-32's extended public keys and a Python implementation, natively and in Wasm. WIF keys encode and decode in constant time. A one-character change fails the checksum. |
| [27. Bech32](ch027-bech32.md) | Runnable: native C | BIP-173 Bech32 (checksum constant 1, never Bech32m) with the pinned vectors and canonical lowercase re-encoding. |
| [28. Bech32m](ch028-bech32m.md) | Runnable: native C and wasm | BIP-350 Bech32m (constant `0x2bc830a3`) added to chapter 27's code, and segwit address encoding and decoding with BIP-350's rule (Bech32 for version 0, Bech32m for 1–16), checked against every BIP-350 and BIP-173 vector, Bitcoin Core's Bech32 key I/O entries and BIP-350's reference Python implementation, natively and in Wasm. Twelve refusals have codes of their own. |
| [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](ch029-addresses.md) | Runnable: native C and wasm | The five output types built from what they commit to, with the BIP-86 key-path Taproot tweak and P2SH-P2WPKH, and scriptPubKey to address and back on mainnet, testnet and regtest. Checked against every address of Bitcoin Core's key I/O files, every BIP-173 and BIP-350 segwit vector, BIP-86, BIP-341, BIP-84 and BIP-49, and a Python implementation, natively and in Wasm. |
| [30. Output descriptors and their checksum](ch030-output-descriptors-and-their-checksum.md) | Runnable: native C | BIP-380's checksum and the `pk`, `pkh`, `wpkh`, `sh`, `wsh`, `multi`, `sortedmulti`, `combo`, `addr`, `raw` and key-path `tr` functions, with key origins, ranged `xpub`s and BIP-389's `<0;1>` multipath step, expanded to the scriptPubKey of any branch, index and output. Public descriptors only: private keys and hardened steps after an `xpub` refuse, each with its own code. Checked against the vectors of BIP-380 to BIP-386 and BIP-389, Bitcoin Core's `descriptor_tests.cpp`, BIP-380's own Python checksum code and a separate Python implementation. |

### Part VI. Keys from words (chapters 31–33)

BIP-39 seed phrases, BIP-32 hierarchical keys and the standard derivation paths.

| Chapter | Status | Result |
| --- | --- | --- |
| [31. Seed phrases](ch031-seed-phrases.md) | Runnable: native C and wasm | BIP-39 English phrases of 12 to 24 words to entropy and back, and the seed through PBKDF2-HMAC-SHA512, checked against all 24 English vectors of trezor/python-mnemonic, Python's `hashlib` and OpenSSL, natively and in Wasm. Word lookups read the whole list; non-ASCII text refuses instead of skipping NFKD. A phrase with a bad checksum refuses. |
| [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md) | Runnable: native C and wasm | BIP-32 master keys, CKDpriv, CKDpub, fingerprints and the 78-byte `xprv`, `xpub`, `tprv` and `tpub` serialization, checked against BIP-32 test vectors 1–5, the `xprv` column of trezor/python-mnemonic's vectors and a separate Python implementation, natively and in Wasm. Private derivation and `xprv` text run in constant time; every vector 5 key refuses with its own code. |
| [33. Derivation paths](ch033-derivation-paths.md) | Runnable: native C | Path strings with `h` or `'` to BIP-32 indexes and back, BIP-44's five levels for purposes 44, 49, 84 and 86 on mainnet and coin type 1, and each purpose's script from a seed or an account `xpub`, checked against BIP-84's, BIP-86's and BIP-49's addresses, BIP-44's example paths, NIP-06's key and a separate Python implementation. Empty steps, steps of `2^31` or more, other marks and 256 steps refuse with their own codes. |

### Part VII. Transactions and script (chapters 34–40)

Transactions, their IDs, the script interpreter, signature hashes, timelocks and fees.

| Chapter | Status | Result |
| --- | --- | --- |
| [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md) | Runnable: native C and wasm | Every transaction in Bitcoin Core's `tx_valid.json` and `tx_invalid.json` and six pinned mainnet transactions read with Core's rules and write back byte for byte, with and without witnesses, with their txids, wtxids and weights, checked against a Python transliteration of Core natively and in Wasm. Non-canonical CompactSizes, counts above `MAX_SIZE`, unknown flags, empty witnesses and trailing bytes refuse with their own codes, and bytes that read both ways decode as Core's `DecodeTx` decodes them. |
| [35. Txids, wtxids and malleability](ch035-txids-wtxids-and-malleability.md) | Runnable: native C | Ten forms of the 2011 spend `23b397ed`, Core's four changed forms rebuilt byte for byte among them, give ten txids: another DER encoding, `n − s`, `OP_PUSHDATA1`, another `CHECKMULTISIG` dummy and an extra item, each named with the rule that refuses it (BIP-66, low-S, `MINIMALDATA`, BIP-147, `CLEANSTACK`). The P2WPKH spend `f5864806` verifies against its BIP-143 digest, and its high-S twin changes only the wtxid; a signer's two signatures of BIP-143's P2SH-P2WPKH example share a txid. Checked against Python natively. |
| [36. The script interpreter](ch036-the-script-interpreter.md) | Runnable: native C and wasm | Bitcoin Core's `EvalScript` and `VerifyScript` for legacy script, P2SH and segwit v0, with Core's error numbers. All 1,263 rows of `script_tests.json` and all 4,532 flag sets of `tx_valid.json` and `tx_invalid.json` give Core's result, checked against a Python transliteration of Core on 16,178 cases natively and in Wasm. Chapter 44 runs Taproot spends in it. |
| [37. Legacy signature hashes](ch037-legacy-signature-hashes.md) | Runnable: native C and wasm | The legacy signature hash of chapter 36's interpreter, field by field: what each of the six hash types signs, found by changing one byte at a time, the `SIGHASH_SINGLE` bug, FindAndDelete and `OP_CODESEPARATOR` in the scriptCode, undefined hash types, and quadratic hashing (1.87 GB for a 1,000,000-byte transaction of 6,756 inputs). All 500 rows of `sighash.json` match, 488 of them with undefined types, and the example agrees with a Python transliteration of Core. |
| [38. Segwit v0 signature hashes](ch038-segwit-v0-signature-hashes.md) | Runnable: native C and wasm | BIP-143's hash as its steps: the three hashes computed once per transaction, the ones each hash type signs, and the hash, with the P2WPKH scriptCode, the amount commitment and its two-input limit, and linear hashing (1,499,900 bytes at 6,756 inputs against legacy's 1.87 GB). All thirteen signature hashes BIP-143 publishes match with their intermediate hashes, two signed BIP spends pass `VerifyScript`, and the example agrees with a Python transliteration of Core. |
| [39. Timelocks](ch039-timelocks.md) | Runnable: native C | The lock time and its 500,000,000 threshold, `IsFinalTx` and final sequences, BIP-113's median time past, `OP_CHECKLOCKTIMEVERIFY`, BIP-68's disable bit, type bit and 512-second units, sequence locks (`CalculateSequenceLocks`, `EvaluateSequenceLocks`) and `OP_CHECKSEQUENCEVERIFY`, in a new `locktime.coax`. All 192 cases of a Python transliteration of Core's `tx_verify.cpp` and `chain.h` agree, and every Core row that reaches either opcode gives Core's result: 21 of `script_tests.json` and 1,672 flag sets of `tx_valid.json` and `tx_invalid.json`. |
| [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md) | Runnable: native C | The fee as inputs minus outputs with Core's `MoneyRange` and in-below-out refusals, matching mempool.space for three segwit mainnet transactions; feerates in sat/vB, sat/kvB and sat/kWU and the fee for a rate, rounded up; Core's sigop-adjusted vsize (`GetVirtualTransactionSize`, 20 bytes per sigop); and pre-signing weights with the longest signatures (P2PKH 592, P2WPKH 272, key-path P2TR 230) that equal the signed weights of `f5864806` and `33e794d0`. Checked against Python natively. |

### Part VIII. Taproot (chapters 41–45)

The key tweak, script trees, the Taproot signature hash and tapscript.

| Chapter | Status | Result |
| --- | --- | --- |
| [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md) | Runnable: native C | BIP-341's tweak on both sides: the output key Q = P + H_TapTweak(P ‖ root)G with its parity, and the tweaked secret key (d or n − d, plus t). All seven scriptPubKey vectors give their key, scriptPubKey and address, and all seven key-path inputs give their tweaked key and published witness, which verifies against Q and not P. t ≥ n and Q at infinity refuse, and the example's 46 lines agree with BIP-341's own Python, pinned. |
| [42. Script trees: tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md) | Runnable: native C | TapLeaf and TapBranch hashes, a tree's root and each leaf's path built from leaves with their depths, control blocks of 33 + 32m bytes, and BIP-341's commitment check. All six script trees in BIP-341's vectors give every leaf hash, root and control block, and every control block commits to its output key; a flipped parity bit, path bit, leaf version or script bit, a reordered or shortened path and a bad size refuse. The example's 48 lines agree with BIP-341's own Python, pinned. |
| [43. Taproot signature hashes](ch043-taproot-signature-hashes.md) | Runnable: native C | BIP-341's signature message in full: every hash type, the annex, and BIP-342's script-path extension. BIP-341's seven key-path messages and hashes match byte for byte, and 30 annex and script-path messages match Bitcoin Core's TaprootSignatureMsg, pinned; 0x80, the other undefined hash types and SINGLE past the outputs refuse. The example's 57 lines agree with Core's Python. |
| [44. Tapscript](ch044-tapscript.md) | Runnable: native C | BIP-341's key-path and script-path checks and BIP-342's tapscript in chapter 36's `VerifyScript`: `OP_CHECKSIGADD`, Schnorr signatures against 32-byte keys, unknown key types, `OP_SUCCESSx`, MINIMALIF and the validation-weight budget. The seven Taproot rows of `script_tests.json` give Core's results, and 83 more Taproot spends match a Python transliteration of Core. |
| [45. Control blocks and script-path spends](ch045-control-blocks-and-script-path-spends.md) | Runnable: native C | The spender's side of a script path: a leaf's BIP-342 signature by its untweaked key, and the witness of stack, script, control block and annex. Every leaf of BIP-341's six script trees passes the commitment check with BIP-341's control block; a tree of three leaves spends by each leaf and by its key path, and a wrong parity, path or leaf refuses. BIP-341's and Core's code in Python print the example's 42 lines. |
| 44. Tapscript | Planned | Bitcoin Core's Taproot script tests (`script_assets_test.json`) give their expected results. |
| 45. Control blocks and script-path spends | Planned | Script-path spends from BIP-341's vectors verify. A control block with the wrong parity or path refuses. |

### Part IX. Blocks and consensus (chapters 46–57)

Merkle trees, headers and proof of work, the chain parameters, every consensus rule with the chapter that checks it, and the UTXO set.

| Chapter | Status | Result |
| --- | --- | --- |
| [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) | Runnable: native C and wasm | Merkle roots of blocks 100,000, 514,999 and 413,567 match their headers, with Core's `mutated` flag on every equal pair. A block with its last transaction repeated keeps its header and is refused as mutated (CVE-2012-2459). Branches build and verify for SPV proofs, the witness root reaches block 514,999's commitment, and a 64-byte transaction shows why a proof needs the true count; checked against a Python transliteration of Core natively and in Wasm. |
| [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md) | Runnable: native C and wasm | The genesis header, blocks 32,250 to 32,260 across the first retarget and blocks 100,000, 514,999 and 900,000 hash below their targets and link, and any one of their 640 header bits changed fails. `nBits` decodes and encodes as Bitcoin Core's `SetCompact` and `GetCompact` on all of Core's cases, refusals come in Core's order, and the work of each header is `2^256 / (target + 1)`; checked against a Python transliteration of Core natively and in Wasm. |
| [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md) | Runnable: native C and wasm | All 483 mainnet retargets from block 2,016 to block 973,728 match the chain and round-trip through `nBits`, including the factor-of-four clamp at 68,544 and the `powLimit` cap of 2009. The chainwork to block 966,143 equals Bitcoin Core's `nMinimumChainWork`. Core's `pow_tests.cpp` retarget cases, `PermittedDifficultyTransition`, testnet's 20-minute rule, BIP-94 and a time-warp simulation; checked against a Python transliteration of Core natively and in Wasm. |
| [49. The witness commitment](ch049-the-witness-commitment.md) | Runnable: native C | BIP-141's commitment found as Core finds it (the last matching coinbase output of at least 38 bytes), recomputed from the witness root and the reserved value, and checked with `CheckWitnessMalleation`'s three reject reasons in Core's order, then `IsBlockMutated`. Block 514,999's commitment is the script Core would generate, and every verdict agrees with Core's rules transliterated over blocks built by Core's `blocktools.py`. |
| [50. Chain parameters: mainnet, testnet4, signet and regtest](ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md) | Runnable: native C | Mainnet, testnet3, testnet4, signet and regtest as one typed record each, every field checked against the pinned `chainparams.cpp`. Each genesis block is built and hashes to its published hash; the magic bytes, ports, Base58, BIP-32 and bech32 prefixes agree with chapters 26–32; the buried deployment heights, exception blocks, `nMinimumChainWork` and `defaultAssumeValid` are Core's. Signet's magic comes from its challenge, and signet blocks 1 and 19,990 pass BIP-325's signature check while a changed signature, time or solution refuses. Checked against Python natively. |
| [51. Header rules: time, versions and deployments](ch051-header-rules-time-versions-and-deployments.md) | Runnable: native C | Mainnet block 840,113, 67 seconds before its parent, passes because it is after the median time past; at the median it refuses as time-too-old, two hours and a second ahead of the clock as time-too-new, and at version 3 as bad-version. Testnet4's first retarget passes, and its timewarp counterexample refuses at 601 seconds and passes at 600. The minimum version steps up at chapter 50's BIP-34, BIP-66 and BIP-65 heights, and BIP-9's state machine agrees with Core's backward walk, including Taproot's minimum activation height. Checked against Python natively. |
| [52. Transaction checks outside script](ch052-transaction-checks-outside-script.md) | Runnable: native C | Every case of Core's tx_invalid.json marked BADTX refuses with its reason (no outputs, a negative or too-large output, an overflowing total as in CVE-2010-5139, duplicate inputs as in CVE-2018-17144, a coinbase script of 1 or 101 bytes, a null outpoint outside a coinbase), and every other case of both files passes. CheckTxInputs refuses a coinbase spent at 99 confirmations and accepts it at 100, refuses out-of-range input values and inputs below the outputs, and a time-locked transaction is judged by the block's time before CSV and the median time past after. Checked against Python natively. |
| [53. Sigops, weight and block limits](ch053-sigops-weight-and-block-limits.md) | Runnable: native C | Core's three sigop counts (legacy, P2SH and witness) and the sigop cost: every case of Core's `sigopcount_tests` gives its count, and the cost of each of mainnet block 514,999's 19 transactions, with the outputs it spends, matches mempool.space's (116 in all). `CheckBlock` in Core's order passes block 514,999 and refuses a changed root, a repeated last transaction, a missing and a second coinbase and duplicate inputs with Core's reasons. Core's `miner_tests` cluster of 20,001 legacy sigops refuses and 20,000 passes, and blocks one byte past 1,000,000 stripped bytes and 4,000,000 weight units refuse. Checked against Python natively. |
| [54. The subsidy and the coinbase](ch054-the-subsidy-and-the-coinbase.md) | Runnable: native C | Core's `GetBlockSubsidy` at every halving, zero from height 6,930,000 and after 64 halvings (BIP-42), summing to Core's 2,099,999,997,690,000 satoshis. Fourteen pinned mainnet coinbases, each proved in its header by its Merkle branch, among them both sides of every halving and of BIP-34's activation, pass the BIP-34 height rule and claim exactly the subsidy plus mempool.space's fees. One satoshi more refuses as bad-cb-amount, a changed height after activation as bad-cb-height, and a fee total past MAX_MONEY as bad-txns-accumulated-fee-outofrange. The two repeated coinbases of 2010 share their txids with the coinbases they copied, and their blocks are Core's BIP-30 exceptions. Checked against Python natively. |
| [55. Script flags by height and their exceptions](ch055-script-flags-by-height-and-their-exceptions.md) | Runnable: native C | Core's block script mask, hash-specific BIP-16 and Taproot exceptions, and the BIP-30 lookup gate with known BIP-34 ancestry and its 1,983,702 limit. Independent native comparisons cover all five networks' deployment boundaries and every pinned mainnet header 0–2,016. Chapter 58 applies the selected rules to UTXOs. |
| [56. The UTXO set and MuHash3072](ch056-the-utxo-set-and-muhash3072.md) | Runnable: native C | Core's coin record (VARINT, `CompressAmount` and the six special scripts), the chainstate key, `TxOutSer`, `hash_serialized_3` and MuHash3072 with insert, remove and a modular inverse. Both hashes of the coins of Core's `feature_utxo_set_hash.py` and `rpc_dumptxoutset.py` match the values those tests publish, and adding then removing a coin restores the MuHash. Checked against Core's `crypto_tests.cpp` and `compress_tests.cpp` vectors and Core's own Python `muhash.py`. |
| [57. Validate a block: every consensus rule](ch057-validate-a-block-every-consensus-rule.md) | Runnable: native C | Pinned mainnet, testnet4 and signet blocks, among them every soft-fork activation block, validate against the outputs they spend. A crafted block for each consensus rule refuses with Bitcoin Core's reject reason, and the chapter's rule table names the chapter that checks every rule. |

### Part X. Chain state and storage (chapters 58–64)

Connecting and disconnecting blocks, the block index, reorganizations, block files, a key-value store of our own, the coins cache and pruning.

| Chapter | Status | Result |
| --- | --- | --- |
| [58. Connect and disconnect a block](ch058-connect-and-disconnect-a-block.md) | Runnable: native C (validated and explorer light) | Full consensus connects, exact Core undo and complete inverses share an atomic validated-v1 commit. A 102-block regtest journey matches Core’s full UTXO MuHash and every rev body, with two disconnects, all 1,335 frame cuts, a four-block mainnet touched-coin range and native file/sanitizer checks. The explorer light profile remains distinct. |
| [59. The block index and the most-work chain](ch059-the-block-index-and-the-most-work-chain.md) | Runnable: native C | Bitcoin Core's block index: `CBlockIndex` entries with height, chainwork, status levels and flags, parent and `GetSkipHeight` skip pointers, `GetAncestor` in O(log n) steps, `LastCommonAncestor`, `FindFork` and the locator; `AcceptBlockHeader` with `ContextualCheckBlockHeader` over the index; sequence ids, `m_blocks_unlinked` and `FindMostWorkChain` with Core's tie-breaks; `InvalidBlockFound`, `InvalidChainFound` and `InvalidateBlock`. Mainnet's first 2,017 headers and mined regtest forks; Core's `skiplist_tests.cpp` and `blockchain_tests.cpp` cases pass, and every line agrees with Core's transliterated `chain.cpp`, `blockstorage.cpp` and `validation.cpp`. |
| [60. Reorganizations and invalid chains](ch060-reorganizations-and-invalid-chains.md) | Partly implemented: native C (explorer light) | Chooses a preferred admitted-header branch, disconnects to its fork and connects its bodies as one atomic prepared batch. Late tracking failures preserve the live snapshot and do not mark the shared index consensus-invalid. Both fork directions restore exact UTXO metadata; validated invalid-chain propagation remains planned. |
| [61. Block files and undo files](ch061-block-files-and-undo-files.md) | Runnable: native C | Core-compatible block and undo records, reads by body position, position-dependent `xor.dat`, and a file tool for durable append, import and torn-tail recovery. Imports all 2,501 records of Core 32.1's regtest file byte for byte; every physical cut retains the preceding complete records, and a wrong previous hash refuses undo data without publishing its body. |
| [62. A key-value store for the chainstate](ch062-a-key-value-store-for-the-chainstate.md) | Runnable: native C | Our own store, not LevelDB: a sorted table with get, put, delete and ordered iteration, atomic batches, and an append-only log of SHA-256d-checksummed frames with replay that cuts a torn last frame and refuses any other damage, plus compaction by copy-on-write and Bitcoin Core's obfuscation key. On real files, a crash at every byte of every append reopens to exactly the last committed batch, and every single-byte flip of a log gets the same verdict from Coax and a separate Python reader. |
| 63. The coins cache and crash-safe flushes | Planned | A crash at any point of a flush restarts to a consistent tip and replays to the same MuHash. The cache stays under its configured memory limit. |
| 64. Pruning | Planned | A pruned node keeps its block files under the configured target, keeps at least the last 288 blocks, and still reorganizes across them. |

### Part XI. The network (chapters 65–75)

P2P messages, BIP-324 encrypted transport, the handshake, peer discovery and connections, denial-of-service limits, headers sync and block download, compact blocks, filters, and Tor and I2P.

| Chapter | Status | Result |
| --- | --- | --- |
| [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md) | Runnable: native C | The 24-byte v1 header with Core's checks in Core's order (message start, a length of at most 4,000,000, checksum, command padding), and the payloads of `version`, `ping`/`pong`, `inv`/`getdata` with the witness flag, `getheaders`/`headers` with the block locator, `addr` and `addrv2` (BIP-155), `feefilter`, `sendcmpct`, `wtxidrelay` and `sendaddrv2`. The developer reference's samples round-trip, and every line agrees with Bitcoin Core's `messages.py`. |
| [66. ElligatorSwift](ch066-elligatorswift.md) | Runnable: native C | BIP-324's `XSwiftEC` decoder, its eight-case inverse, an encoder that draws from caller randomness, and the hashed x-only ECDH. All 76 decoding vectors, all 256 inversion cases and the key exchange of all 7 packet vectors pass, and every line agrees with BIP-324's `reference.py`. |
| [67. The v2 encrypted transport](ch067-the-v2-encrypted-transport.md) | Runnable: native C | BIP-324's key derivation, FSChaCha20 and FSChaCha20Poly1305 with rekeying every 224 uses, packets sealed and opened in place with the ignore bit, the garbage and its terminator, and the short message IDs. All 7 packet vectors pass from the private key, and every line agrees with BIP-324's `reference.py` and Core's `net.cpp`. |
| [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md) | Runnable: native C | The `version` and `verack` exchange as a pure state machine with Core's checks in Core's order (services, minimum version, the payload's tail, self-connection by nonce), `wtxidrelay`, `sendaddrv2` and `feature` before `verack`, `sendheaders`, `sendcmpct`, `ping` and `feefilter`, the inactivity timeouts, and the transport underneath: v1 frames, the v2 handshake, v1 detection by a v2 responder and the retry in v1. Every line agrees with Core's transliterated `ProcessMessage`, `messages.py` and BIP-324's `reference.py`. |
| [69. Addresses and the address manager](ch069-addresses-and-the-address-manager.md) | Runnable: native C | Bitcoin Core's `AddrMan`: network groups without an asmap, the new and tried tables with keyed SHA-256d buckets, `Add`, `Good` with test-before-evict, `Attempt`, `Select` and `GetAddr` over `FastRandomContext`, and `peers.dat` in Core's format 4. Then `ProcessAddrs`' limits and token bucket, `RelayAddress`, the cached `getaddr` answer and DNS seed timing. Core's `addrman_tests.cpp` cases pass, and every line agrees with Core's transliterated `addrman.cpp`, `netgroup.cpp` and `net_processing.cpp`. |
| [70. Connections: outbound, block-relay-only, feelers and eviction](ch070-connections-outbound-block-relay-only-feelers-and-eviction.md) | Runnable: native C | Bitcoin Core's connection manager over chapter 69's address manager: 8 full-relay, 2 block-relay-only and 1 feeler slot, anchors first, exponential timers for feelers and extra block-relay-only peers, the address loop with one peer per network group and `IsBadPort`, `SelectNodeToEvict` with its protected peers, inbound acceptance and `EvictExtraOutboundPeers`. Core's `net_peer_eviction_tests.cpp` cases pass, and every line agrees with Core's transliterated `net.cpp`, `node/eviction.cpp` and `net_processing.cpp`. |
| [71. Misbehavior, limits and denial of service](ch071-misbehavior-limits-and-denial-of-service.md) | Runnable: native C | Bitcoin Core's denial-of-service defenses: each message's size limit, `Misbehaving` and `MaybeDiscourageAndDisconnect` with noban, manual and localhost peers, `MaybePunishNodeForBlock`, MurmurHash3 and `CRollingBloomFilter` behind `BanMan`'s discouragement, the anti-DoS work threshold and presync bound for headers, flood control that bounds a peer to about 15 MB, and the per-peer transaction announcement limits. Core's `murmurhash3`, `rolling_bloom`, `peer_discouragement` and `DoS_bantime` tests pass, and every line agrees with Core's transliterated `net_processing.cpp`, `net.cpp`, `bloom.cpp` and `headerssync.cpp`. |
| [72. Headers sync and block download](ch072-headers-sync-and-block-download.md) | Runnable: native C | Block locators, Core's `HeadersSyncState` (PRESYNC with salted commitments, then REDOWNLOAD), `ProcessHeadersMessage`, and block download through the 1,024-block window with 16 blocks in flight per peer, the staller and its doubling timeout, and a check of each block against its header and Merkle root. Core's `headers_sync_chainwork_tests` and `p2p_ibd_stalling.py` pass, every line agrees with Core's transliterated `headerssync.cpp` and `net_processing.cpp`, and it syncs regtest and signet from Bitcoin Core peers (signet: every header, then the last 200 blocks). Blocks are stored, not validated or connected (chapters 57, 58, 60 and 61). |
| [73. Compact blocks](ch073-compact-blocks.md) | Runnable: native C | BIP-152 version 2: short IDs from SipHash-2-4 keyed by SHA-256(header || nonce), the `cmpctblock`, `getblocktxn` and `blocktxn` messages with differential indexes, and `PartiallyDownloadedBlock`'s `InitData` and `FillBlock`, collisions and `IsBlockMutated` included, with Core's high-bandwidth peer choice. Block 413,567's keys, short IDs and message equal `messages.py`'s, and Core's `blockencodings_tests.cpp` cases pass. |
| [74. Compact block filters](ch074-compact-block-filters.md) | Runnable: native C | BIP-158's basic filter: a Golomb-coded set of a block's output scripts (without OP_RETURN ones) and the scripts its inputs spend, hashed with SipHash-2-4 keyed by the block hash and coded with P = 19 and M = 784,931, with Core's strict decode check and matching, plus BIP-157's filter headers. All ten testnet-19.json filters and headers match, and Bitcoin Core's own Python SipHash and element hash with a golombrice.h transliteration agree on seeded sets, matches and damaged filters. |
| [75. Proxies, Tor and I2P](ch075-proxies-tor-and-i2p.md) | Runnable: native C | How Core reaches Tor and I2P: Tor v3 onion addresses (an ed25519 key, a SHA3-256 checksum and version 3 in base32) and I2P addresses (base32 of a destination's SHA-256) with their BIP-155 encodings, Core's strict base32, its SOCKS5 client with RFC 1929 credentials for Tor stream isolation, I2P's SAM v3.1 sessions and streams, and the reachability rules of `-onlynet`, `-proxy`, `-onion` and `-i2psam`, all as pure procedures without a live Tor or I2P. The rend-spec and Core test vectors give their stated results, and every line agrees with an independent Python model. |

### Part XII. Initial block download (chapters 76–80)

From genesis to the tip: parallel script checks, faster verification, assumevalid, AssumeUTXO, and a mainnet sync whose UTXO set matches Bitcoin Core's.

| Chapter | Status | Result |
| --- | --- | --- |
| 76. The signature cache and parallel script checks | Planned | Script checks run on every core and give the same verdicts as one thread over a pinned block range. A transaction verified on mempool entry is not verified again in its block, and the cache is keyed by a salted hash. |
| [77. Faster verification: the GLV endomorphism](ch077-faster-verification-the-glv-endomorphism.md) | Runnable: native C | secp256k1's endomorphism `λ·(x, y) = (β·x, y)`: libsecp256k1's scalar split into two halves below `2^128`, and four width-5 wNAF terms over `G`, `λG`, `P` and `λP` on one chain of about 128 doublings. Chapters 19 and 23 verify on it with the same results; it is about 1.6 times faster than chapter 16's path, and libsecp256k1 about 45 times faster still. |
| 78. assumevalid | Planned | Below the pinned assumevalid block the node skips script checks only for ancestors of that block buried under its stated work; every other rule still runs. With assumevalid off the node reaches the same UTXO set. |
| 79. AssumeUTXO snapshots | Planned | The node loads a pinned mainnet UTXO snapshot whose hash matches the chain parameters, follows the tip from it at once, and validates from genesis in the background. A snapshot with one changed coin refuses. |
| 80. Sync mainnet from genesis | Planned | The node syncs mainnet from genesis to a pinned height over the public network, and its UTXO set at that height equals Bitcoin Core's `gettxoutsetinfo` (`hash_serialized_3` and `muhash`). The chapter records time, memory and disk on stated hardware next to Bitcoin Core's on the same machine. |

### Part XIII. The mempool and policy (chapters 81–88)

Standardness, mempool acceptance, transaction relay, replacement, packages, eviction, fee estimation and block templates.

| Chapter | Status | Result |
| --- | --- | --- |
| [81. Policy is not consensus: standardness](ch081-policy-is-not-consensus-standardness.md) | Runnable: native C | Bitcoin Core's standardness: `Solver`'s output types, `GetDustThreshold`, `IsStandardTx`, `ValidateInputsStandardness` with BIP-54's sigop limit and `IsWitnessStandard`, in `PreChecks`' order, each with Core's reason. One transaction per rule is refused and still valid in a block; Core's `test_IsStandard` and `max_standard_legacy_sigops` cases pass, and every line agrees with Core's transliterated `policy.cpp`. |
| [82. Mempool acceptance and limits](ch082-mempool-acceptance-and-limits.md) | Partly implemented: native C (unvalidated explorer observations) | Bounded, explicitly unvalidated explorer observations; exact fees from own confirmed coins and observed parents, conflicts and resource refusals, P2P observation and reorg refresh. Full Core acceptance and policy parity remain planned. |
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
| [90. JSON-RPC and REST](ch090-json-rpc-and-rest.md) | Partly implemented: native C (read-only explorer HTTP) | Read-only explorer REST and JSON-RPC serve block, transaction, balance and spend data from one durable pow-only snapshot. Full Bitcoin Core RPC compatibility, cookie authentication and wallet/miner methods remain planned. |
| [91. Indexes: transactions, filters and coin statistics](ch091-indexes-transactions-filters-and-coin-statistics.md) | Partly implemented: native C (explorer transaction index) | Durable full-txid positions share the light chainstate batch and cursor. Lookup verifies raw file spans, prefers active matches, preserves stale occurrences and original positions across reorgs, and excludes genesis. Filter and historical coin-statistics indexes, node backfill/pruning and RPC parity remain planned. |
| 92. Notifications for wallets and Lightning | Planned | A subscriber receives every connected and disconnected block and every mempool entry and removal in order, across a reorganization, and resumes from its last block after a restart. |
| 93. Shadow mode beside Bitcoin Core | Planned | Run beside a Bitcoin Core node on the same peers for a stated period, the node gives the same verdict on every header and block and holds the same tip. Any difference stops it and records the block. |

### Part XV. Wallets (chapters 94–99)

PSBTs, coin selection, message signing, silent payments, miniscript and a descriptor wallet on our node.

| Chapter | Status | Result |
| --- | --- | --- |
| [94. PSBT](ch094-psbt.md) | Runnable: native C | BIP-174 version 0, BIP-370 version 2 and BIP-371's Taproot fields, read as Bitcoin Core reads them: all 155 distinct PSBTs that BIP-174, BIP-370, BIP-371 and Core's rpc_psbt.json publish read or refuse as published, each refusal with Core's message, and BIP-370's lock-time cases give its lock time. BIP-174's worked example runs from its master key through the Creator, Updater, two Signers, Combiner, Input Finalizer and Transaction Extractor, matching every published result; its signer-check cases refuse for BIP-174's reasons; Core's signer (low-R ECDSA), combiner, finalizer and extractor cases match byte for byte. A P2WPKH, P2WSH 2-of-2 and P2TR key-path spend passes the script interpreter and BIP-340. Checked against Python natively. |
| [95. Coin selection and change](ch095-coin-selection-and-change.md) | Runnable: native C | Bitcoin Core's Branch and Bound, CoinGrinder, Single Random Draw and Knapsack with its waste metric, change target and dust threshold: all 344 selection cases and 11 waste cases ported from Core's coinselector_tests.cpp and coinselection_tests.cpp give Core's result, with Core's ChaCha20 random draws. Brute force over every subset of random wallets finds no changeless selection with less waste than Branch and Bound and none lighter than CoinGrinder, and every chosen selection pays its feerate. Checked against Python natively. |
| [96. Message signing](ch096-message-signing.md) | Runnable: native C | BIP-322 generic signed messages for any address: the tagged message hash, `to_spend` and `to_sign`, and the simple, full, proof-of-funds and legacy formats. All 3 hash vectors and 23 published signatures verify, the signer makes the reference's signature for each, all 36 published errors are invalid for their reason, and 22 constructed cases cover the upgradeable rules and Taproot script paths. Checked against Python natively. |
| [97. Silent payments](ch097-silent-payments.md) | Runnable: native C | BIP-352 silent payments: scan and spend keys, the `sp1` address and its versions, which inputs count, `input_hash`, the ECDH shared secret and the outputs t_k, labels with the change label 0, and scanning bounded by K_max. All 29 published cases give their outputs, or fail as published; all 30 receivers find the published outputs, tweaks and signatures; 15 constructed addresses cover the version rules. Checked against BIP-352's reference.py and Python natively. |
| 98. Miniscript | Planned | The BIP-379 examples type-check, compile to their stated scripts and satisfy. |
| 99. A descriptor wallet on our node | Planned | A descriptor wallet finds its coins by rescan and by notification, builds, signs and broadcasts a spend on signet through our node, and survives a reorganization that removes a payment. |

### Part XVI. Many signers (chapters 100–103)

Multisig, MuSig2, FROST threshold signatures and adaptor signatures.

| Chapter | Status | Result |
| --- | --- | --- |
| [100. Multisig](ch100-multisig.md) | Runnable: native C | m-of-n `OP_CHECKMULTISIG` scripts written as Bitcoin Core writes them, with BIP-67's sorted keys, and read back as Core's `MatchMultisig` reads them: all four BIP-67 vectors give their scripts and addresses, and twenty near misses match or refuse as Core decides. One `sortedmulti` 2-of-3 is locked bare, under P2SH, P2WSH and P2SH-P2WSH, signed by two keys through PSBT, finalized and extracted, and every input passes the script interpreter. BIP-387's `multi_a` leaves give every published scriptPubKey; a `sortedmulti_a` 2-of-3 is spent on the script path with two BIP-340 signatures, and one signature or swapped signatures fail. Checked against Python natively. |
| [101. MuSig2](ch101-musig2.md) | Runnable: native C | BIP-327 in full: KeyAgg with coefficient hashing and the second-key optimization, KeySort, plain and x-only tweaks, NonceGen with secret inputs kept in an erased workspace, NonceAgg, Sign, PartialSigVerify, PartialSigAgg and DeterministicSign. Every case of BIP-327's eight vector files, refusals included, gives its stated result. A secnonce signs once and a second Sign refuses; three signatures from a copied secnonce give the secret key away. The aggregate key tweaked by TapTweak is chapter 41's output key, and its key-path signature verifies. Checked against BIP-327's reference.py natively. |
| [102. FROST threshold signatures](ch102-frost-threshold-signatures.md) | Runnable: native C | BIP-445 (the FROST draft BIP, revision `ba299bad`) in full: threshold info validation, NonceGen, NonceAgg, the session with its signer-set binding factor, Sign, PartialSigVerify, PartialSigAgg and DeterministicSign, with plain and x-only tweaks. Every case of its seven vector files, refusals included, gives its stated result. Shamir shares from the trusted dealer recover the secret from any two of three, and a Pedersen DKG with Feldman commitments and BIP-340 proofs of possession makes a 2-of-3 with no dealer. A signer set below t refuses; the threshold key tweaked by TapTweak is chapter 41's output key and its key-path signature verifies. Checked against the BIP's reference implementation natively. |
| [103. Adaptor signatures](ch103-adaptor-signatures.md) | Runnable: native C | Schnorr adaptor signatures on BIP-340: pre-signing for a point T with the final nonce's parity in the pre-signature, pre-verification, adapting with t into a BIP-340 signature, and extracting t from it; nonce reuse recovers the key, and statement reuse lets anyone complete a second lock. Six vectors from an independent Python implementation on BIP-340's pinned reference.py match byte for byte, and every line agrees with it. |

### Part XVII. Lightning (chapters 104–115)

Lightning built in Coax on our node: messages, the Noise handshake, channels, onions, gossip, payments, invoices and offers.

| Chapter | Status | Result |
| --- | --- | --- |
| [104. Messages and TLVs](ch104-messages-and-tlvs.md) | Runnable: native C | BOLT 1's message framing with the "it's OK to be odd" rule, BigSize, TLV streams, truncated integers, `init` with BOLT 9's feature bits, `error`, `warning`, `ping` and `pong`. Every BigSize, TLV and `init` vector in BOLT 1 gives its result, and every line agrees with an independent Python reader. |
| [105. The Noise handshake](ch105-the-noise-handshake.md) | Runnable: native C | BOLT 8's Noise_XK handshake: the three acts with exact byte layouts, `h` and `ck`, ECDH as SHA-256 of the compressed point, HKDF, the encrypted-length transport and key rotation every 1,000 uses, with secrets in constant time. Every BOLT 8 handshake and message vector gives its result, and every line agrees with an independent Python handshake. |
| [106. Commitment keys and revocation](ch106-commitment-keys-and-revocation.md) | Runnable: native C | BOLT 3's key derivation: per-commitment points, localpubkey and the HTLC and delayed keys with their private keys, remotepubkey under option_static_remotekey, and the revocation key that needs both peers' secrets. Every BOLT 3 key vector gives its result, and every line agrees with an independent Python derivation. |
| [107. Shachain](ch107-shachain.md) | Runnable: native C | BOLT 3's per-commitment secrets: generation from one seed over 2^48 indices by bit flips and SHA-256, and the receiver's 49-slot store with its insert check and lookup. Every BOLT 3 generation and storage vector gives its result, and every line agrees with an independent Python store. |
| [108. Commitment and HTLC transactions](ch108-commitment-and-htlc-transactions.md) | Runnable: native C | BOLT 3's transactions: the sorted 2-of-2 funding output, the commitment with its obscured number, `to_local`, `to_remote`, offered and received HTLC outputs and `option_anchors`' anchors, fees by expected weight, dust trimming and BIP-69 order with the expiry tiebreak, and the HTLC-timeout and HTLC-success transactions, signed `SIGHASH_SINGLE` and `ANYONECANPAY` under anchors. Every BOLT 3 funding, commitment and HTLC transaction vector matches byte for byte, and every line agrees with an independent Python builder. |
| [109. Open, update and close a channel](ch109-open-update-and-close-a-channel.md) | Runnable: native C | BOLT 2's peer protocol as a pure state machine between two in-process nodes: `open_channel` to `channel_ready`, HTLC adds, fulfills and fails with `commitment_signed` and `revoke_and_ack`, `channel_reestablish` after a lost message, and `shutdown` with the `closing_signed` fee negotiation. Every commitment is checked with chapter 108's builder, and every line agrees with an independent Python model. |
| 110. Watch the chain | Planned | A revoked commitment broadcast on regtest is swept by the penalty transaction, and an expired HTLC times out, with our node as the only chain source. |
| [111. Onion routing and blinded paths](ch111-onion-routing-and-blinded-paths.md) | Runnable: native C | BOLT 4's Sphinx packet: shared secrets, blinding factors, the rho, mu, um, pad and ammag keys, the ChaCha20 stream, the filler and the HMAC chain over 1,300 bytes of TLV hop payloads; error onions and their decryption by the payer; and route blinding with `encrypted_recipient_data` and path keys. Every BOLT 4 onion, error and blinded-path vector gives its result, and every line agrees with an independent Python Sphinx. |
| [112. Gossip and the channel graph](ch112-gossip-and-the-channel-graph.md) | Runnable: native C | BOLT 7's gossip: short channel IDs, `channel_announcement` with four signatures over the double SHA-256 after them, `node_announcement` with BOLT 1's restricted UTF-8 alias and address descriptors, `channel_update` with its direction and disable bits, timestamps, fees, CLTV delta and CRC32C checksum, the query messages in the plain encoding with zlib refused, and a channel graph that keeps only what checks. BOLT 7's vectors give their stated results, and every line agrees with an independent Python reader and writer. |
| [113. Find a route and pay](ch113-find-a-route-and-pay.md) | Runnable: native C | Payments over chapter 109's channels: Dijkstra from the payee with BOLT 7 fees and CLTV deltas, `htlc_minimum_msat`, `htlc_maximum_msat`, disabled channels and a uniform liquidity model's failure penalty; BOLT 4 hop payloads and onions; forwarding nodes that check their fee and expiry; multi-part payments held until `total_msat`; route hints from BOLT 11 invoices; and retries that learn from error onions. Every line agrees with an independent Python payer and network. |
| [114. Invoices](ch114-invoices.md) | Runnable: native C | BOLT 11's invoices: the human-readable part with the currency and an amount with its multiplier, the 5-bit tagged fields `p`, `s`, `d`, `h`, `n`, `x`, `c`, `f`, `r`, `9` and `m` with their rules, and the recoverable signature that names the payee or verifies under `n` with a low S; a writer that reproduces BOLT 11's examples. All 31 BOLT 11 examples give their stated result, and every line agrees with an independent Python reader and writer. |
| [115. Offers](ch115-offers.md) | Runnable: native C | BOLT 12's offers, invoice requests, invoices and payer proofs: text with no checksum and `+` joins, the TLV ranges of each kind, the Merkle tree of `LnLeaf`, `LnNonce` and `LnBranch` tagged hashes signed with BIP-340, `offer_issuer_id` versus blinded paths, the request and invoice rules, and payer proofs with marker numbers and missing hashes; a writer for each step. Every BOLT 12 vector gives its stated result, and every line agrees with an independent Python reader and writer. |

### Part XVIII. Interop and verification (chapters 116–120)

Test vector provenance, differential tests, Bitcoin Core's functional tests against our node, fuzzing and formal properties.

| Chapter | Status | Result |
| --- | --- | --- |
| [116. Test vectors and their provenance](ch116-test-vectors-and-their-provenance.md) | Runnable: native C | How the book pins every file it reads: `manifest.json`, `SOURCES.md`, `check_vectors.py` offline and with `--fetch`, and published against generated vectors. The Bitcoin Core target is `ddea9196`, a `master` revision past v32, not a release; `check_pins.py` checks it, the inventory, every citation, a pins-by-chapter index and every generator. The example hashes BIP-340's CSV in Coax, refuses a one-byte edit, then verifies all 19 rows. Checked against Python's `hashlib`, `csv` and BIP-340's `reference.py`. |
| 117. Differential tests | Planned | Signatures, signature hashes, script results and block verdicts agree with libsecp256k1 and Bitcoin Core on generated inputs. |
| 118. Bitcoin Core's functional tests against our node | Planned | The Bitcoin Core functional tests that drive a node over P2P and RPC (`feature_block.py` and the `p2p_*` and `mempool_*` tests the chapter names) pass against our node on one of our own machines, never the owner's Mac. Each test that does not apply is listed with its reason. |
| [119. Fuzzing](ch119-fuzzing.md) | Runnable: native C | A deterministic mutation fuzzer for transactions, P2P headers and version payloads, PSBTs, BOLT 11 invoices and Base58Check, seeded from the pinned vectors. Three oracles: no crash in checked or release builds, every accepted input writes back to itself where the format has one encoding, and the refusal code equals a Python reference's. Structure-aware mutation repairs P2P and Bech32 checksums. 100,000 inputs per parser found no crash and no parser bug; they found a reference that accepted a CompactSize cut short, now fixed, and the inputs are kept as regressions. The example runs a fixed budget of 1,000 inputs per parser. |
| 120. Formal properties | Planned | Each named property (round trips, refusal of non-canonical encodings and the consensus rule table) has a checked model or a stated boundary. |

### Part XIX. Bitcoin in OpenAgents (chapters 121–124)

Where OpenAgents uses the stack: our node on Pylons and Verse OS, one seed, agent treasuries and paid requests.

| Chapter | Status | Result |
| --- | --- | --- |
| [121. One seed for Nostr and Bitcoin](ch121-one-seed-for-nostr-and-bitcoin.md) | Runnable: native C | One BIP-39 phrase gives the NIP-06 key at `m/44'/1237'/a'/0/0` and BIP-84 and BIP-86 wallets, recorded on a card of the `npub` and two descriptors with key origins, `<0;1>/*` and checksums. NIP-06's two vectors, BIP-39's vector 0 with `TREZOR`, BIP-86's account `xpub` and the six published BIP-84 and BIP-86 addresses come from the card. A restore with the same words gives the same card; a wrong passphrase, account or network gives another, and a wrong checksum refuses the words. A raw signature by the Nostr key verifies under the Nostr key and under no wallet key. Checked against a Python implementation natively. |
| [122. Agent treasuries](ch122-agent-treasuries.md) | Runnable: native C | An agent's treasury under a 2-of-3 of the agent, the owner and a recovery key, with the recovery key alone after a BIP-68 delay, as a P2WSH script and as a Taproot output with a MuSig2 key path, a `multi_a` leaf and a recovery leaf. The agent's limit is a separate hot wallet; it signs a treasury PSBT only for a refill under the ceiling and the fee cap, and refuses payments, overfills, high fees and foreign coins. A refill by agent and owner, the recovery path at the delay and one block before, and every pair of keys run through chapter 36's interpreter. Checked against a Python implementation natively. Broadcast on signet and recovery with an independent wallet wait for the node (chapters 80, 93, 99 and 118). |
| 123. Paying for requests | Planned | A request pays a BOLT 11 invoice, and the server releases its response only after settlement. |
| 124. Our node on Pylons and Verse OS | Planned | A Pylon and a Verse OS machine run the node from its Nix module and sync it, and OpenAgents' on-chain checks query it instead of a third-party chain source. |

### Part XX. Explorer extensions (chapter 125)

| Chapter | Status | Result |
| --- | --- | --- |
| [125. Address history, balances and spends](ch125-address-history-balances-and-spends.md) | Runnable: native C | Exact-script histories and balances, output spend links and reorg-aware active membership share the indexed light chain’s atomic batch and cursor. Independent raw-block replay qualifies native results and crash recovery. |
