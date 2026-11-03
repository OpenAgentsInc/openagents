# Contents

- [Introduction](README.md)
- [Plan and prerequisites](PLAN.md)

The contents list the chapters written so far. The [plan](PLAN.md) lists all
124, with what each one needs and what "done" means.

**I. Foundations**

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) — implemented
- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) — implemented
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md) — implemented

**II. Hashes, MACs and ciphers**

- [4. SHA-256](ch004-sha-256.md) — implemented
- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md) — implemented
- [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md) — implemented
- [7. SHA-512](ch007-sha-512.md) — implemented
- [8. HMAC-SHA256](ch008-hmac-sha256.md) — implemented
- [9. HKDF](ch009-hkdf.md) — implemented
- [10. SipHash-2-4](ch010-siphash-2-4.md) — implemented
- [11. ChaCha20](ch011-chacha20.md) — implemented
- [12. Poly1305 and ChaCha20-Poly1305](ch012-poly1305-and-chacha20-poly1305.md) — implemented

**III. secp256k1**

- [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md) — implemented
- [14. Scalars mod n](ch014-scalars-mod-n.md) — implemented
- [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md) — implemented
- [16. Scalar multiplication](ch016-scalar-multiplication.md) — implemented
- [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md) — implemented
- [18. Keys and randomness](ch018-keys-and-randomness.md) — implemented

**IV. Signatures**

- [19. Verify ECDSA](ch019-verify-ecdsa.md) — implemented
- [20. Sign with ECDSA](ch020-sign-with-ecdsa.md) — implemented
- [21. Strict DER](ch021-strict-der.md) — implemented
- [22. Public key recovery and signed messages](ch022-public-key-recovery-and-signed-messages.md) — implemented
- [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md) — implemented
- [24. Sign with Schnorr](ch024-sign-with-schnorr.md) — implemented

**V. Encodings and addresses**

- [26. Base58Check](ch026-base58check.md) — implemented
- [27. Bech32](ch027-bech32.md) — implemented
- [28. Bech32m](ch028-bech32m.md) — implemented
- [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](ch029-addresses.md) — implemented
- [30. Output descriptors and their checksum](ch030-output-descriptors-and-their-checksum.md) — implemented

**VI. Keys from words**

- [31. Seed phrases](ch031-seed-phrases.md) — implemented
- [32. Hierarchical deterministic keys](ch032-hierarchical-deterministic-keys.md) — implemented
- [33. Derivation paths](ch033-derivation-paths.md) — implemented

**VII. Transactions and script**

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md) — implemented
- [35. Txids, wtxids and malleability](ch035-txids-wtxids-and-malleability.md) — implemented
- [36. The script interpreter](ch036-the-script-interpreter.md) — implemented
- [37. Legacy signature hashes](ch037-legacy-signature-hashes.md) — implemented
- [38. Segwit v0 signature hashes](ch038-segwit-v0-signature-hashes.md) — implemented
- [39. Timelocks](ch039-timelocks.md) — implemented
- [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md) — implemented

**VIII. Taproot**

- [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md) — implemented
- [42. Script trees: tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md) — implemented
- [43. Taproot signature hashes](ch043-taproot-signature-hashes.md) — implemented

**IX. Blocks and consensus**

- [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) — implemented
- [47. Block headers and proof of work](ch047-block-headers-and-proof-of-work.md) — implemented
- [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md) — implemented
- [49. The witness commitment](ch049-the-witness-commitment.md) — implemented
- [50. Chain parameters: mainnet, testnet4, signet and regtest](ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md) — implemented
- [51. Header rules: time, versions and deployments](ch051-header-rules-time-versions-and-deployments.md) — implemented
- [52. Transaction checks outside script](ch052-transaction-checks-outside-script.md) — implemented
- [53. Sigops, weight and block limits](ch053-sigops-weight-and-block-limits.md) — implemented
- [54. The subsidy and the coinbase](ch054-the-subsidy-and-the-coinbase.md) — implemented
- [56. The UTXO set and MuHash3072](ch056-the-utxo-set-and-muhash3072.md) — implemented
- [62. A key-value store for the chainstate](ch062-a-key-value-store-for-the-chainstate.md) — implemented

**XI. The network**

- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md) — implemented
- [66. ElligatorSwift](ch066-elligatorswift.md) — implemented
- [67. The v2 encrypted transport](ch067-the-v2-encrypted-transport.md) — implemented
- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md) — implemented
- [69. Addresses and the address manager](ch069-addresses-and-the-address-manager.md) — implemented
- [73. Compact blocks](ch073-compact-blocks.md) — implemented
- [74. Compact block filters](ch074-compact-block-filters.md) — implemented

**XVII. Lightning**

- [104. Messages and TLVs](ch104-messages-and-tlvs.md) — implemented
- [105. The Noise handshake](ch105-the-noise-handshake.md) — implemented
- [106. Commitment keys and revocation](ch106-commitment-keys-and-revocation.md) — implemented
- [107. Shachain](ch107-shachain.md) — implemented
