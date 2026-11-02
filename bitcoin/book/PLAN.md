# Plan

The proposed chapters, the specifications each implements, and what exists
today. "From Nostr N" means the chapter is Nostr From Scratch chapter N,
moved. "Code exists" names Coax source that a chapter would teach. Every
other chapter is new work. Numbers will change as chapters are written.

## I. Foundations

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 1 | Bytes, hex, little-endian and CompactSize | Bitcoin serialization | From Nostr 1 (base64 stays there) |
| 2 | Big integers in fixed limbs | | From Nostr 2 |
| 3 | Constant-time rules and secrets | | From Nostr 3 |

## II. Hashes and MACs

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 4 | SHA-256 | FIPS 180-4 | From Nostr 4 |
| 5 | SHA-256d and tagged hashes | BIP-340 tags | Tags built in `schnorr*.coax` |
| 6 | RIPEMD-160 and HASH160 | RIPEMD-160 paper | New |
| 7 | SHA-512 | FIPS 180-4 | Code exists: `sha512.coax` |
| 8 | HMAC | RFC 2104 | From Nostr 5, plus HMAC-SHA512 |
| 9 | PBKDF2 and HKDF | RFC 8018, RFC 5869 | From Nostr 6; `pbkdf2*.coax` exists |
| 10 | SipHash-2-4 | SipHash paper | New |

## III. secp256k1

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 11 | Field arithmetic mod p | SEC 2 | From Nostr 7 |
| 12 | Scalars mod n | SEC 2 | From Nostr 8 |
| 13 | The group law | SEC 1 | From Nostr 9 |
| 14 | Scalar multiplication | | From Nostr 10 |
| 15 | Point encodings: compressed, uncompressed, x-only | SEC 1, BIP-340 | Partly in Nostr 9–11 |
| 16 | Keys and randomness | | From Nostr 12 |

## IV. Signatures

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 17 | Verify ECDSA | SEC 1 | New |
| 18 | Sign ECDSA with RFC 6979 and low-S | RFC 6979, BIP-62, BIP-146 | Code exists: `ecdsa_sign.coax` (Nostr 104) |
| 19 | Strict DER | BIP-66 | Encoding exists in `ecdsa_sign.coax`; strict parsing new |
| 20 | Public key recovery and signed messages | Bitcoin Core `signmessage` | New |
| 21 | Verify a Schnorr signature | BIP-340 | From Nostr 11 |
| 22 | Sign with Schnorr | BIP-340 | From Nostr 13 |
| 23 | Batch verification | BIP-340 | New |

## V. Encodings and addresses

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 24 | Base58Check | | Decoding in `nipkeys.coax`; becomes its own codec |
| 25 | Bech32 and Bech32m | BIP-173, BIP-350 | From Nostr 15; Bech32m in `nipa3.coax` |
| 26 | Addresses: P2PKH, P2SH, P2WPKH, P2WSH, P2TR | BIP-13, BIP-141, BIP-341 | Checks in `nipa3.coax` |
| 27 | Output descriptors and their checksum | BIP-380–386 | Parts in `nipkeys.coax` (Nostr 104) |

## VI. Keys from words

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 28 | Seed phrases | BIP-39 | Code exists: `bip39.coax` (Nostr 46) |
| 29 | Hierarchical deterministic keys | BIP-32 | Code exists: `bip32.coax`, `nipkeys_sign.coax` |
| 30 | Derivation paths and extended keys | BIP-44, 84, 86; SLIP-132 | NIP-06's path stays in Nostr 46 |

## VII. Transactions and script

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 31 | Transactions and their serialization | BIP-144 | Reader in `sighash.coax` |
| 32 | Txids, wtxids and malleability | BIP-141 | New |
| 33 | The script interpreter | Bitcoin Core `script_tests.json` | New |
| 34 | Legacy sighash and its quirks | `FindAndDelete`, `SIGHASH_SINGLE` bug | New |
| 35 | Segwit v0 sighash | BIP-143 | Code exists: `sighash.coax` |
| 36 | Timelocks | BIP-65, BIP-68, BIP-112, BIP-113 | New |
| 37 | Weight, vsize and fees | BIP-141 | New |

## VIII. Taproot

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 38 | The key tweak and the key path | BIP-341 | Code exists: `sighash.coax` |
| 39 | Script trees: tapleaves and tapbranches | BIP-341 | New |
| 40 | Taproot sighash, every hash type | BIP-341 | Key path exists; script path new |
| 41 | Tapscript | BIP-342 | New |
| 42 | Control blocks and script-path spends | BIP-341 | New |

## IX. Blocks and consensus

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 43 | Merkle trees and the duplicate-leaf flaw | CVE-2012-2459 | New |
| 44 | Block headers and proof of work | | New |
| 45 | Compact targets, retargeting and chainwork | | New |
| 46 | The witness commitment | BIP-141 | New |
| 47 | Validate a block (bounded profile) | BIP-30, BIP-34 | New |
| 48 | The UTXO set and MuHash3072 | Bitcoin Core | New |
| 49 | Headers first and the most-work chain | | New |

## X. The network

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 50 | P2P messages and checksums | Bitcoin P2P protocol | New |
| 51 | ElligatorSwift | BIP-324 | New |
| 52 | ChaCha20, Poly1305 and FSChaCha20Poly1305 | RFC 8439, BIP-324 | From Nostr 18; `poly1305*.coax` exists |
| 53 | The encrypted transport handshake | BIP-324 | New |
| 54 | Compact blocks | BIP-152 | New |
| 55 | Compact block filters | BIP-157, BIP-158 | New |

## XI. Wallets

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 56 | PSBT | BIP-174, BIP-370 | v2 parts in `nipkeys.coax` (Nostr 104) |
| 57 | Coin selection and change | | New |
| 58 | Message signing | BIP-322 | Read in `nipkeys.coax`; verification new |
| 59 | Silent payments | BIP-352 | New |
| 60 | Miniscript | BIP-379 | New |

## XII. Many signers

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 61 | Multisig: CHECKMULTISIG and sortedmulti | BIP-11, BIP-67 | P2WSH 2-of-3 in `nipkeys_sign.coax` (Nostr 104) |
| 62 | MuSig2 | BIP-327 | New |
| 63 | FROST threshold signatures | Draft BIPs | New |
| 64 | Adaptor signatures | | New |

## XIII. Lightning

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 65 | The Noise handshake | BOLT 8 | New |
| 66 | Commitment keys and revocation | BOLT 3 | New |
| 67 | Shachain | BOLT 3 | New |
| 68 | Onion routing and blinded paths | BOLT 4 | New |
| 69 | Invoices | BOLT 11 | Parts in `nip57.coax`, `x402/` |
| 70 | Offers | BOLT 12 | New |

## XIV. Interop and verification

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 71 | Test vectors and their provenance | BIP vectors, Core's `script_tests.json`, `sighash.json`, `tx_valid.json` | Nostr 93 covers the moved chapters |
| 72 | Differential tests against libsecp256k1 and Bitcoin Core | | Nostr 94 covers the moved chapters |
| 73 | Fuzzing | | New for Bitcoin formats |
| 74 | Formal properties | | Nostr 96 covers the moved chapters |

## XV. Bitcoin in OpenAgents

| # | Chapter | Specs | Today |
| --- | --- | --- | --- |
| 75 | One seed for Nostr and Bitcoin | BIP-39, NIP-06 | `docs/lightning/`, Spark wallet in the iOS app |
| 76 | Agent treasuries | NIP-KEYS | Bitcoin half of Nostr 104 |
| 77 | Paying for requests | x402, BOLT 11 | `coax/lib/x402/` |
