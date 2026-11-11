# Bitcoin Lab

An unlisted learning world at https://openagents.com/worlds/bitcoin-lab.
You walk your Everglade character down a lit lane on a dark grid. Seven
stations stand beside it, in order:

1. **A byte**: eight bits, the number they make, its hex and its character.
2. **Hex**: the genesis block's 80 header bytes, coloured by field, with one
   byte split into its two nibbles.
3. **Base64**: three bytes regrouped into four six-bit values and their letters.
4. **Byte order**: the little-endian time field, its bytes reversed into the
   number and the date it means.
5. **SHA-256**: 80 bytes in, 32 bytes out.
6. **SHA-256d**: the hash of the hash, its leading zeros, and the target it
   must be under.
7. **The chain**: recorded regtest blocks 95 to 102 as cubes, each joined to
   the one before by its previous-block hash.

Every hash, digit and date is computed in Coax as the page runs, from the
genesis header and the recorded blocks.

**RECORDED REGTEST / VALIDATED-V1:** the cubes represent actual retained
Bitcoin Core regtest blocks, rebuilt from genesis through our complete
validator and atomically indexed with coins and undo. This is a bounded
recording, with unknown observation time. It is not a live node. The lab has
no secrets, wallet ownership inference, signing or spending controls.

Use WASD or the arrow keys to walk, Shift to walk slowly and Space to jump;
drag to turn the follow camera. On touch, drag the left side to walk and the
right side to look. WORLDS (top right) goes back to [Worlds](/worlds).

The chapter map below lists the book's chapters. Chapters 1, 4 and 5 are
taught at the stations; the rest are in the book.

This realm stays out of normal Worlds, galaxy, Start, sitemap and agent
discovery. A deliberate `?realms=all` diagnostic may show it. Unlisted is not
authentication; direct links are accessible. No public listing is authorized.

<!-- generated chapter map -->

| Chapter | Station | Book status | Visual exercise |
| --- | --- | --- | --- |
| [1. Bytes, hex and base64](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch001-bytes-hex-and-base64.md) | Proof/Script | implemented | at the stations |
| [2. Big integers in fixed limbs](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch002-big-integers-in-fixed-limbs.md) | Proof/Script | implemented | not available |
| [3. Constant-time rules and secrets](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch003-constant-time-rules.md) | Proof/Script | implemented | not available |
| [4. SHA-256](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch004-sha-256.md) | Proof/Script | implemented | at the stations |
| [5. SHA-256d and tagged hashes](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch005-sha-256d-and-tagged-hashes.md) | Proof/Script | implemented | at the stations |
| [6. RIPEMD-160 and HASH160](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch006-ripemd-160-and-hash160.md) | Proof/Script | implemented | not available |
| [7. SHA-512](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch007-sha-512.md) | Proof/Script | implemented | not available |
| [8. HMAC-SHA256](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch008-hmac-sha256.md) | Proof/Script | implemented | not available |
| [9. HKDF](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch009-hkdf.md) | Proof/Script | implemented | not available |
| [10. SipHash-2-4](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch010-siphash-2-4.md) | Proof/Script | implemented | not available |
| [11. ChaCha20](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch011-chacha20.md) | Proof/Script | implemented | not available |
| [12. Poly1305 and ChaCha20-Poly1305](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch012-poly1305-and-chacha20-poly1305.md) | Proof/Script | implemented | not available |
| [13. Field arithmetic mod p](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch013-field-arithmetic-mod-p.md) | Proof/Script | implemented | not available |
| [14. Scalars mod n](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch014-scalars-mod-n.md) | Proof/Script | implemented | not available |
| [15. The secp256k1 group law](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch015-the-secp256k1-group-law.md) | Proof/Script | implemented | not available |
| [16. Scalar multiplication](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch016-scalar-multiplication.md) | Proof/Script | implemented | not available |
| [17. Point encodings: compressed, uncompressed and x-only](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch017-point-encodings-compressed-uncompressed-and-x-only.md) | Proof/Script | implemented | not available |
| [18. Keys and randomness](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch018-keys-and-randomness.md) | Proof/Script | implemented | not available |
| [19. Verify ECDSA](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch019-verify-ecdsa.md) | Proof/Script | implemented | not available |
| [20. Sign with ECDSA](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch020-sign-with-ecdsa.md) | Proof/Script | implemented | not available |
| [21. Strict DER](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch021-strict-der.md) | Proof/Script | implemented | not available |
| [22. Public key recovery and signed messages](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch022-public-key-recovery-and-signed-messages.md) | Proof/Script | implemented | not available |
| [23. Verify a Schnorr signature](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch023-verify-a-schnorr-signature.md) | Proof/Script | implemented | not available |
| [24. Sign with Schnorr](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch024-sign-with-schnorr.md) | Proof/Script | implemented | not available |
| [25. Batch verification](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch025-batch-verification.md) | Proof/Script | implemented | not available |
| [26. Base58Check](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch026-base58check.md) | Wallets | implemented | not available |
| [27. Bech32](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch027-bech32.md) | Wallets | implemented | not available |
| [28. Bech32m](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch028-bech32m.md) | Wallets | implemented | not available |
| [29. Addresses: P2PKH, P2SH, P2WPKH, P2WSH and P2TR](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch029-addresses.md) | Wallets | implemented | not available |
| [30. Output descriptors and their checksum](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch030-output-descriptors-and-their-checksum.md) | Wallets | implemented | not available |
| [31. Seed phrases](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch031-seed-phrases.md) | Wallets | implemented | not available |
| [32. Hierarchical deterministic keys](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch032-hierarchical-deterministic-keys.md) | Wallets | implemented | not available |
| [33. Derivation paths](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch033-derivation-paths.md) | Wallets | implemented | not available |
| [34. Transactions and their serialization](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch034-transactions-and-their-serialization.md) | Transactions/UTXOs | implemented | not available |
| [35. Txids, wtxids and malleability](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch035-txids-wtxids-and-malleability.md) | Proof/Script | implemented | not available |
| [36. The script interpreter](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch036-the-script-interpreter.md) | Proof/Script | implemented | not available |
| [37. Legacy signature hashes](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch037-legacy-signature-hashes.md) | Proof/Script | implemented | not available |
| [38. Segwit v0 signature hashes](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch038-segwit-v0-signature-hashes.md) | Proof/Script | implemented | not available |
| [39. Timelocks](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch039-timelocks.md) | Proof/Script | implemented | not available |
| [40. Weight, vsize and fees](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch040-weight-vsize-and-fees.md) | Transactions/UTXOs | implemented | not available |
| [41. The key tweak and the key path](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch041-the-key-tweak-and-the-key-path.md) | Proof/Script | implemented | not available |
| [42. Script trees: tapleaves and tapbranches](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch042-script-trees-tapleaves-and-tapbranches.md) | Proof/Script | implemented | not available |
| [43. Taproot signature hashes](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch043-taproot-signature-hashes.md) | Proof/Script | implemented | not available |
| [44. Tapscript](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch044-tapscript.md) | Proof/Script | implemented | not available |
| [45. Control blocks and script-path spends](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch045-control-blocks-and-script-path-spends.md) | Proof/Script | implemented | not available |
| [46. Merkle trees and the duplicate-leaf flaw](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch046-merkle-trees-and-the-duplicate-leaf-flaw.md) | Chain | implemented | not available |
| [47. Block headers and proof of work](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch047-block-headers-and-proof-of-work.md) | Chain | implemented | not available |
| [48. Compact targets, retargeting and chainwork](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch048-compact-targets-retargeting-and-chainwork.md) | Chain | implemented | not available |
| [49. The witness commitment](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch049-the-witness-commitment.md) | Chain | implemented | not available |
| [50. Chain parameters: mainnet, testnet4, signet and regtest](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch050-chain-parameters-mainnet-testnet4-signet-and-regtest.md) | Chain | implemented | not available |
| [51. Header rules: time, versions and deployments](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch051-header-rules-time-versions-and-deployments.md) | Chain | implemented | not available |
| [52. Transaction checks outside script](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch052-transaction-checks-outside-script.md) | Chain | implemented | not available |
| [53. Sigops, weight and block limits](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch053-sigops-weight-and-block-limits.md) | Chain | implemented | not available |
| [54. The subsidy and the coinbase](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch054-the-subsidy-and-the-coinbase.md) | Chain | implemented | not available |
| [55. Script flags by height and their exceptions](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch055-script-flags-by-height-and-their-exceptions.md) | Chain | implemented | not available |
| [56. The UTXO set and MuHash3072](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch056-the-utxo-set-and-muhash3072.md) | Chain | implemented | not available |
| [57. Validate a block: every consensus rule](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch057-validate-a-block-every-consensus-rule.md) | Transactions/UTXOs | implemented | not available |
| [58. Connect and disconnect a block](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch058-connect-and-disconnect-a-block.md) | Chain | implemented | not available |
| [59. The block index and the most-work chain](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch059-the-block-index-and-the-most-work-chain.md) | Chain | implemented | not available |
| [60. Reorganizations and invalid chains](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch060-reorganizations-and-invalid-chains.md) | Chain | implemented | not available |
| [61. Block files and undo files](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch061-block-files-and-undo-files.md) | Node | implemented | not available |
| [62. A key-value store for the chainstate](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch062-a-key-value-store-for-the-chainstate.md) | Node | implemented | not available |
| [63. The coins cache and crash-safe flushes](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch063-the-coins-cache-and-crash-safe-flushes.md) | Node | implemented | not available |
| [64. Pruning](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L223) | Node | planned | not available |
| [65. P2P messages and checksums](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch065-p2p-messages-and-checksums.md) | Node | implemented | not available |
| [66. ElligatorSwift](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch066-elligatorswift.md) | Node | implemented | not available |
| [67. The v2 encrypted transport](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch067-the-v2-encrypted-transport.md) | Node | implemented | not available |
| [68. The handshake and feature negotiation](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch068-the-handshake-and-feature-negotiation.md) | Node | implemented | not available |
| [69. Addresses and the address manager](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch069-addresses-and-the-address-manager.md) | Node | implemented | not available |
| [70. Connections: outbound, block-relay-only, feelers and eviction](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch070-connections-outbound-block-relay-only-feelers-and-eviction.md) | Node | implemented | not available |
| [71. Misbehavior, limits and denial of service](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch071-misbehavior-limits-and-denial-of-service.md) | Node | implemented | not available |
| [72. Headers sync and block download](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch072-headers-sync-and-block-download.md) | Node | implemented | not available |
| [73. Compact blocks](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch073-compact-blocks.md) | Node | implemented | not available |
| [74. Compact block filters](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch074-compact-block-filters.md) | Node | implemented | not available |
| [75. Proxies, Tor and I2P](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch075-proxies-tor-and-i2p.md) | Node | implemented | not available |
| [76. The signature cache and parallel script checks](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L245) | Node | planned | not available |
| [77. Faster verification: the GLV endomorphism](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch077-faster-verification-the-glv-endomorphism.md) | Node | implemented | not available |
| [78. assumevalid](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L247) | Node | planned | not available |
| [79. AssumeUTXO snapshots](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L248) | Node | planned | not available |
| [80. Sync mainnet from genesis](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L249) | Node | planned | not available |
| [81. Policy is not consensus: standardness](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch081-policy-is-not-consensus-standardness.md) | Mempool/Mining | implemented | not available |
| [82. Mempool acceptance and limits](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch082-mempool-acceptance-and-limits.md) | Mempool/Mining | partly implemented | not available |
| [83. Transaction relay and orphans](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L257) | Mempool/Mining | planned | not available |
| [84. Replacement: RBF and TRUC](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L258) | Mempool/Mining | planned | not available |
| [85. Packages, CPFP and anchors](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L259) | Mempool/Mining | planned | not available |
| [86. Eviction, expiry and persistence](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L260) | Mempool/Mining | planned | not available |
| [87. Fee estimation](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L261) | Mempool/Mining | planned | not available |
| [88. Block templates and mining](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L262) | Mempool/Mining | planned | not available |
| [89. The node process: configuration, data directory, logs and shutdown](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L268) | Node | planned | not available |
| [90. JSON-RPC and REST](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch090-json-rpc-and-rest.md) | Node | partly implemented | not available |
| [91. Indexes: transactions, filters and coin statistics](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch091-indexes-transactions-filters-and-coin-statistics.md) | Node | partly implemented | not available |
| [92. Notifications for wallets and Lightning](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L271) | Node | planned | not available |
| [93. Shadow mode beside Bitcoin Core](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L272) | Node | planned | not available |
| [94. PSBT](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch094-psbt.md) | Wallets | implemented | not available |
| [95. Coin selection and change](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch095-coin-selection-and-change.md) | Wallets | implemented | not available |
| [96. Message signing](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch096-message-signing.md) | Wallets | implemented | not available |
| [97. Silent payments](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch097-silent-payments.md) | Wallets | implemented | not available |
| [98. Miniscript](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L282) | Wallets | planned | not available |
| [99. A descriptor wallet on our node](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L283) | Wallets | planned | not available |
| [100. Multisig](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch100-multisig.md) | Proof/Script | implemented | not available |
| [101. MuSig2](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch101-musig2.md) | Proof/Script | implemented | not available |
| [102. FROST threshold signatures](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch102-frost-threshold-signatures.md) | Proof/Script | implemented | not available |
| [103. Adaptor signatures](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch103-adaptor-signatures.md) | Proof/Script | implemented | not available |
| [104. Messages and TLVs](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch104-messages-and-tlvs.md) | Lightning | implemented | not available |
| [105. The Noise handshake](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch105-the-noise-handshake.md) | Lightning | implemented | not available |
| [106. Commitment keys and revocation](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch106-commitment-keys-and-revocation.md) | Lightning | implemented | not available |
| [107. Shachain](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch107-shachain.md) | Lightning | implemented | not available |
| [108. Commitment and HTLC transactions](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch108-commitment-and-htlc-transactions.md) | Lightning | implemented | not available |
| [109. Open, update and close a channel](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch109-open-update-and-close-a-channel.md) | Lightning | implemented | not available |
| [110. Watch the chain](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L304) | Lightning | planned | not available |
| [111. Onion routing and blinded paths](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch111-onion-routing-and-blinded-paths.md) | Lightning | implemented | not available |
| [112. Gossip and the channel graph](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch112-gossip-and-the-channel-graph.md) | Lightning | implemented | not available |
| [113. Find a route and pay](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch113-find-a-route-and-pay.md) | Lightning | implemented | not available |
| [114. Invoices](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch114-invoices.md) | Lightning | implemented | not available |
| [115. Offers](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch115-offers.md) | Lightning | implemented | not available |
| [116. Test vectors and their provenance](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch116-test-vectors-and-their-provenance.md) | Node | implemented | not available |
| [117. Differential tests](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L316) | Node | planned | not available |
| [118. Bitcoin Core's functional tests against our node](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L317) | Node | planned | not available |
| [119. Fuzzing](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch119-fuzzing.md) | Node | implemented | not available |
| [120. Formal properties](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L319) | Node | planned | not available |
| [121. One seed for Nostr and Bitcoin](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch121-one-seed-for-nostr-and-bitcoin.md) | Node | implemented | not available |
| [122. Agent treasuries](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch122-agent-treasuries.md) | Wallets | implemented | not available |
| [123. Paying for requests](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L327) | Wallets | planned | not available |
| [124. Our node on Pylons and Verse OS](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/PLAN.md#L328) | Node | planned | not available |
| [125. Address history, balances and spends](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch125-address-history-balances-and-spends.md) | Transactions/UTXOs | implemented | not available |

## Recorded regtest blocks

Validated-v1 recording; observation time unknown. Full transaction inputs, outputs, scripts and spend occurrences are in the HTML fallback and compiled recording.

### Snapshot 0 — height 102

Committed tip: `055359b6e245148e0e02bd513f35c86bb00b82fe80bff6f4c85884db207e01df`

| Height | State | Full block hash | Bytes | Weight | Transactions |
| --- | --- | --- | --- | --- | --- |
| 0 | active | 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206 | 285 | 1140 | 1 |
| 1 | active | 76f75e47d8652eda37915e724301619a49c0a1341004a551c6f505333142ce16 | 249 | 888 | 1 |
| 2 | active | 3410c782209337f022f26682ef3e44a2d0f76bde02da648dc30376eec3873451 | 249 | 888 | 1 |
| 3 | active | 0472c2bdc5d7de0943e76e4606d7029e7b4a4a70bd3efdd560bc9c9fce8bafd6 | 249 | 888 | 1 |
| 4 | active | 3048455d01da5e28484f925a715347d64ae929f8ccbdc65d965cc9b08db3e31d | 249 | 888 | 1 |
| 5 | active | 204578ccded43310ae189235fba1b7c9e9a165217f8e6b8d1c084fd86989879e | 249 | 888 | 1 |
| 6 | active | 01644b09300f0c51b4180389766e67f05e3e9197c68450b18ecaa1cc58dd8268 | 249 | 888 | 1 |
| 7 | active | 25d612b52dc7bab081b289bd262048a9d400a6e0f2f98d1a989c972094c82bb9 | 249 | 888 | 1 |
| 8 | active | 434a731bc3cab56877b9a2b9f00493526a188d6a1e2d0b8e98502981a9500087 | 249 | 888 | 1 |
| 9 | active | 6f94e44d7976a2e4cf817160d643214d87cc96ade3f7a28ccec161ddd28e7acb | 249 | 888 | 1 |
| 10 | active | 4317629ed1039f2e825fa2a06ac34a011b89ed2f9f6903d9b7664bdbc3d8c676 | 249 | 888 | 1 |
| 11 | active | 192f2587955c7d145c9dee82d5bc0d45411f431b8b8c70126a1cb6775fdfab64 | 249 | 888 | 1 |
| 12 | active | 2925e49a676d83b5c265605327b74c4f210ce859c7f0d8f354f0feab8cfe78f8 | 249 | 888 | 1 |
| 13 | active | 0572d35235ef2a3c373f2f6559e4ccfe61bdd788fe359057ed881f156707c747 | 249 | 888 | 1 |
| 14 | active | 7d0a4efd72b64efa182ea0f48ddd06751be13b59dd3ef89d3a5bc1360e1981b2 | 249 | 888 | 1 |
| 15 | active | 071f0734f81c7aaeff4238a92ef8ca91f3887b0e90d73e4d088aa9a5bc450491 | 249 | 888 | 1 |
| 16 | active | 381d9dc73001ec97522ab23ce63f1648ee7fd885a18820b102b790ed78458b38 | 249 | 888 | 1 |
| 17 | active | 4222d3ca963d94c18be39f3bd9ab8436259781f68fde24a036cbf40590d28edf | 249 | 888 | 1 |
| 18 | active | 14dd829094a46717514ddd54a548dac11f55d81e0a418fee7857bca65f6e26af | 249 | 888 | 1 |
| 19 | active | 034c3d6f31f56affc5caab7d870244449db030463ce33ca1fa1e147a02fcc89f | 249 | 888 | 1 |
| 20 | active | 4f1ef228d3a144ed7305fa6bbc937cf21f5f32a814d56aa9ca9fe0661776f8b8 | 249 | 888 | 1 |
| 21 | active | 3007d87907afffcb4fa4b54ee8db060e7c563660017dd0253a47b16ea256a8cf | 249 | 888 | 1 |
| 22 | active | 19fc44c80c919be640b39ff73b62bf20584dde2415f70e67be2a481b91ec373c | 249 | 888 | 1 |
| 23 | active | 03082f3452b286ae98f2beaa2bfb3026020c67f829828086d456c290bff03eb7 | 249 | 888 | 1 |
| 24 | active | 4c669ac3dce90064358bca86c75be6abbd7eca2b11723dc7c15b5eb3610c0d50 | 249 | 888 | 1 |
| 25 | active | 47ea486acced9f7f25e1ad735fee4df29f2199b2eb3e31937b4aedfe0b657bfa | 249 | 888 | 1 |
| 26 | active | 0b4d2b410776ea8e475aee0fe792e1722e5cfa4c32ae247d8e7e00afd59340c3 | 249 | 888 | 1 |
| 27 | active | 5692e7f73a43e327cab1cc308c7aa729a952a7614fec2af0f8bbae4e59083bb7 | 249 | 888 | 1 |
| 28 | active | 47dfbcc72e5ca32c1c139b38e7f68626478a8bd4e3d953b4629311314b76c35a | 249 | 888 | 1 |
| 29 | active | 63fad2ecacbe616afa49e251257ef9412db3adbcce65b465e072d60bf4427d00 | 249 | 888 | 1 |
| 30 | active | 24e5c1746ead2405a24f2e7e3dba0d112f1bac8d68d13518667d151c18efe523 | 249 | 888 | 1 |
| 31 | active | 10657c3ac6e2ac950a64a9d0e0498b2886d1ca808ac4ffd47efeea5eb3d8db75 | 249 | 888 | 1 |
| 32 | active | 26b619ebef3d8b73a31d58caea5503c89df953fab788d444b6e2015f0ea57730 | 249 | 888 | 1 |
| 33 | active | 0642e4ae5c0fd277349b7f4498bc199596ef904a3a1e63f87437200eb4459208 | 249 | 888 | 1 |
| 34 | active | 688451da7195693c140c32cca06e11ecca99f8d9b778bc5bc0c1d0fcceeabef7 | 249 | 888 | 1 |
| 35 | active | 19ed3a4d54e28f32ac732ec73b70df45b18e56ffff1e9e413c0db2a27711b812 | 249 | 888 | 1 |
| 36 | active | 03e59173d32547e232407672e994ea8b39ddc019fc5a6ff9389a59dc5741f8d0 | 249 | 888 | 1 |
| 37 | active | 65c2ae1e4164ea31bc23f0036dc951bced5f5a6081cf1b7d4fe24dba23004d5e | 249 | 888 | 1 |
| 38 | active | 7b9a8dcd89bb6017c4be4530a23d5bf2d550bdd9fc9fcc2aee92dd59792ab6af | 249 | 888 | 1 |
| 39 | active | 74994d8ed0e49af1f5caff6a73d9747c44df6558b619f60758cb117091af6fce | 249 | 888 | 1 |
| 40 | active | 0b77efc74d8a12fe8ac37a0dbe7fe3de261a7bd2263888d42abbcf6d8a747497 | 249 | 888 | 1 |
| 41 | active | 55a0ff916ab2e8c26759dec7c2e790ca5bb80f26d84d74f8c3c514501ff11acd | 249 | 888 | 1 |
| 42 | active | 1280df94b3003cc5bb91e5a880bf5ea19c33ebb6e86487f7c0d4552cdae05cc8 | 249 | 888 | 1 |
| 43 | active | 325216c52cea590d09d8e7dd0d28b41e7b032c0dcfc07321050ebeff4e485bc7 | 249 | 888 | 1 |
| 44 | active | 5b13d652042535ecde80f09c79bb0961095853a3dd20a95b74539e541a5133b4 | 249 | 888 | 1 |
| 45 | active | 238bc0aa234034f0f346c64868aa9c6a020b0da546e8263555477ca0142cddf6 | 249 | 888 | 1 |
| 46 | active | 7c159e25a5817b4f0130dc20b74136b3b22f7be89e9048f06210e93642e14dfd | 249 | 888 | 1 |
| 47 | active | 36b69b7dcd13f06f120dfafa78c2c3423dc8bef8e73a4d490206885804dc0034 | 249 | 888 | 1 |
| 48 | active | 19ee286c6ac23b34660d303dd4013d55b6798ab93f7808061ffef12b551eb74a | 249 | 888 | 1 |
| 49 | active | 03920514851fc5379e2acaa351ad89c0f2cbcd5269aeac8067a8aab5bb903fe6 | 249 | 888 | 1 |
| 50 | active | 62aea72eecad6a5c340503ba739b19ee437f338b62f8b00a88eaacd33718d3d7 | 249 | 888 | 1 |
| 51 | active | 3e0c774e914e0ccf6dd704e33e81ee85c2712eebcc1a786835ed0c408fdee598 | 249 | 888 | 1 |
| 52 | active | 3ebf261c4f8978633775c56912250fe80bef0cd5b105258466ee2b251b9233d0 | 249 | 888 | 1 |
| 53 | active | 6fada913cf70bed14f092229646347bf106c681b80a34fbb92bd04898239a5b4 | 249 | 888 | 1 |
| 54 | active | 35a2a187c273e60aa79ebdee27b62d4b4b99f268698508c8bd31bc605eea77fb | 249 | 888 | 1 |
| 55 | active | 70910c657c69f3cb299fc24b9c46478c13296f23f6cf5f29e2deed2e857683fb | 249 | 888 | 1 |
| 56 | active | 41292abd63aeda18cf9a6dc2fdd789cb3cbe92934c860247137ce7eba002489f | 249 | 888 | 1 |
| 57 | active | 3fd12019a1059e97944f652d0fb9ec16d3cddaee3ebb5f835bb419e559e46663 | 249 | 888 | 1 |
| 58 | active | 780eeaa9eef080a9cf96c5f36f683526d2f98a1573b151bc374feb1cb7fe5c7d | 249 | 888 | 1 |
| 59 | active | 7fda5f280e22b661b76e85f8a84f33e9dd91b76745feca8a017d89f7035e8138 | 249 | 888 | 1 |
| 60 | active | 5688dcb86c98878b3f1eb905ae5ffb0fab0081ba2aa6738dbd2ee37b8c47ad80 | 249 | 888 | 1 |
| 61 | active | 1efbc8fb8fd2b443f34938166fa34359eb52a7865309f8f3f0e29e6596531f10 | 249 | 888 | 1 |
| 62 | active | 680d19e60c9b69348fb073119ff0f805fd7f1ec227be574105ece8f33eeb55dc | 249 | 888 | 1 |
| 63 | active | 65d7623c27e10ac87b4eb3f99c718fb8ca0565f189286f6af16b439c3f7f668e | 249 | 888 | 1 |
| 64 | active | 18d205c685ee90737efe71b4ca7a206e3e68228667848f525e544537e9f9a092 | 249 | 888 | 1 |
| 65 | active | 0eaa4fab31706d044c108c474f90b6dcedf0d8494688550d84c318f1c2cb574f | 249 | 888 | 1 |
| 66 | active | 32e8fbc3e811542c51876ed52e64d96db80c8a23be15c3f78d2c6b0e31e27e24 | 249 | 888 | 1 |
| 67 | active | 09c33856f6e8560da89dc4e9e3e042699ec457f5f649f5b8e76c86c4157d87a9 | 249 | 888 | 1 |
| 68 | active | 4be9aff9f6a35659387329512061e3097e928b36477ecf1cbf5bbd394b8c0b64 | 249 | 888 | 1 |
| 69 | active | 045edc89e4bd6734694e1da62bab71edc13468623dc248739e0a7f1186d05542 | 249 | 888 | 1 |
| 70 | active | 35bef8fefaf6de48bdfb6936cbeec87bc42760a548e45d61fb0d21c3283636eb | 249 | 888 | 1 |
| 71 | active | 0d91b77867ac9c0f798adb0d6268277a5ed5b7c5d1dce463a48c12cfeee20d0b | 249 | 888 | 1 |
| 72 | active | 1b882cb143b8790abeefc96e02ab16f9c8d89d6c026bf2a125b117e9e5d40b7d | 249 | 888 | 1 |
| 73 | active | 6abcd131db0a87b7ae2dfc1b6bf26c7aae9961a37b4d0fdf0f6934abfe15dcda | 249 | 888 | 1 |
| 74 | active | 36bf916bdbeeba47a2c65dd222411c1e6a53ed4c4f77a115f62f12c7a377a526 | 249 | 888 | 1 |
| 75 | active | 62ae2b09e0c05c164497d9ad4798ae2d4ac04faac6c4ed0770d22cb602b15ac0 | 249 | 888 | 1 |
| 76 | active | 0e77c9fd481459dc95a8b05aee552fd9a80eebfc4535b0beaf45666874d63d0b | 249 | 888 | 1 |
| 77 | active | 1f97d2caa0f259fb39937e2d11fb74c73fcad3e8f16fbe019eba8a9a6a2e279d | 249 | 888 | 1 |
| 78 | active | 1e4ab46ff4595113165ae45c5b9d3bfbc38cfdcbce8b703ef2e26d3888138788 | 249 | 888 | 1 |
| 79 | active | 555793e88c0fab7a39187c6d70ab5ef1678dd01c3f830d9a37e87a46ec762eda | 249 | 888 | 1 |
| 80 | active | 5bc31b5db9392a9e6afe478b9cd52329c1ec2f4ae3d5f8505e7044b5b038553c | 249 | 888 | 1 |
| 81 | active | 325a973f8f05cb108a5d42e0f0790e4bdd19610434d3546d88e6a15b8faa82a0 | 249 | 888 | 1 |
| 82 | active | 584131ab962c9606b78bc74b8c4522446c3f3bd3d31e66741ce1d0c48ff09aef | 249 | 888 | 1 |
| 83 | active | 6265b2be357c5e2a974955df6156e7844cca5ca513e1303e6adab0a299eeca23 | 249 | 888 | 1 |
| 84 | active | 1edd9ce5c70d7b9588ce3b1bfe55cde624a89e5f77cc9c51baac38c98011725f | 249 | 888 | 1 |
| 85 | active | 35bba695b1aa48b35900ff350e4f8c8910eac0c4ce64979ead4eb84d97713f63 | 249 | 888 | 1 |
| 86 | active | 2c99e4d25e52600592474e49cff0bf76dbb92cbb7c9963fa7164e5c42ef2e098 | 249 | 888 | 1 |
| 87 | active | 2203217d5350b6db3eb8e70861e5b6e80a5a60764bddef74374f0d5ac1c7a423 | 249 | 888 | 1 |
| 88 | active | 39710ee4980f4c2c414b6e4c6c61b08290ea20076b6176829a6b33c68fdfcff8 | 249 | 888 | 1 |
| 89 | active | 3667b57919674344fcf78065365ab6e57c9fabcf647fc57737307b162ba3c3c0 | 249 | 888 | 1 |
| 90 | active | 13012d53693aabcd92f0276dbe2771e1050af9e0b9ed904419291b3c240a6cfe | 249 | 888 | 1 |
| 91 | active | 32e4e4ac15584509cc5362c62fedf5cdcc92b59eb01caea751e34b49d9c06ab2 | 249 | 888 | 1 |
| 92 | active | 70113fb6a560703d241bea7c3211c2609a6eb190d92c5272c1e68bcaecb8e1f3 | 249 | 888 | 1 |
| 93 | active | 7be67df995019e0ef3b342abaf6edac150595d9a8336fd5c9922da176df54706 | 249 | 888 | 1 |
| 94 | active | 2d80f22f6b1898f03fba80c021a5bd02ccb5372b26661088c0582a3b50779232 | 249 | 888 | 1 |
| 95 | active | 343eaeb49e430020a84bf891d17d39e0f028ae66f1c03128650cca9dbb0801c6 | 249 | 888 | 1 |
| 96 | active | 3cab99e4beaa649377240cd69203ce7387303f0c6ad5d21facf538c7f5e12a54 | 249 | 888 | 1 |
| 97 | active | 02cf01994c96f2296ed6ea7a83f09b10ca1abea42264f47bb333b13b777ad402 | 249 | 888 | 1 |
| 98 | active | 18ad435943597d31d14d42c710f369fde8a1fb484d22a72a1cca42c893102874 | 249 | 888 | 1 |
| 99 | active | 395115dea022305d4d1a3ef869fe9eeeaba12c8d19f3406920117167e7c0172d | 249 | 888 | 1 |
| 100 | active | 584e045cd9f3b45855b8f3b4fbcf56c281d716932f30e8f35265271208885f6d | 249 | 888 | 1 |
| 101 | active | 291fb977943bf9cec15f2572c6b9ba204344a0d896b3adf85c570759ab6d9fdb | 485 | 1832 | 3 |
| 102 | active | 055359b6e245148e0e02bd513f35c86bb00b82fe80bff6f4c85884db207e01df | 249 | 888 | 1 |

### Snapshot 1 — height 103

Committed tip: `61896cbe35d327e8b1d4e567a4bf05d928d0c761421440cb7055eff87d89cbd3`

| Height | State | Full block hash | Bytes | Weight | Transactions |
| --- | --- | --- | --- | --- | --- |
| 0 | active | 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206 | 285 | 1140 | 1 |
| 1 | active | 76f75e47d8652eda37915e724301619a49c0a1341004a551c6f505333142ce16 | 249 | 888 | 1 |
| 2 | active | 3410c782209337f022f26682ef3e44a2d0f76bde02da648dc30376eec3873451 | 249 | 888 | 1 |
| 3 | active | 0472c2bdc5d7de0943e76e4606d7029e7b4a4a70bd3efdd560bc9c9fce8bafd6 | 249 | 888 | 1 |
| 4 | active | 3048455d01da5e28484f925a715347d64ae929f8ccbdc65d965cc9b08db3e31d | 249 | 888 | 1 |
| 5 | active | 204578ccded43310ae189235fba1b7c9e9a165217f8e6b8d1c084fd86989879e | 249 | 888 | 1 |
| 6 | active | 01644b09300f0c51b4180389766e67f05e3e9197c68450b18ecaa1cc58dd8268 | 249 | 888 | 1 |
| 7 | active | 25d612b52dc7bab081b289bd262048a9d400a6e0f2f98d1a989c972094c82bb9 | 249 | 888 | 1 |
| 8 | active | 434a731bc3cab56877b9a2b9f00493526a188d6a1e2d0b8e98502981a9500087 | 249 | 888 | 1 |
| 9 | active | 6f94e44d7976a2e4cf817160d643214d87cc96ade3f7a28ccec161ddd28e7acb | 249 | 888 | 1 |
| 10 | active | 4317629ed1039f2e825fa2a06ac34a011b89ed2f9f6903d9b7664bdbc3d8c676 | 249 | 888 | 1 |
| 11 | active | 192f2587955c7d145c9dee82d5bc0d45411f431b8b8c70126a1cb6775fdfab64 | 249 | 888 | 1 |
| 12 | active | 2925e49a676d83b5c265605327b74c4f210ce859c7f0d8f354f0feab8cfe78f8 | 249 | 888 | 1 |
| 13 | active | 0572d35235ef2a3c373f2f6559e4ccfe61bdd788fe359057ed881f156707c747 | 249 | 888 | 1 |
| 14 | active | 7d0a4efd72b64efa182ea0f48ddd06751be13b59dd3ef89d3a5bc1360e1981b2 | 249 | 888 | 1 |
| 15 | active | 071f0734f81c7aaeff4238a92ef8ca91f3887b0e90d73e4d088aa9a5bc450491 | 249 | 888 | 1 |
| 16 | active | 381d9dc73001ec97522ab23ce63f1648ee7fd885a18820b102b790ed78458b38 | 249 | 888 | 1 |
| 17 | active | 4222d3ca963d94c18be39f3bd9ab8436259781f68fde24a036cbf40590d28edf | 249 | 888 | 1 |
| 18 | active | 14dd829094a46717514ddd54a548dac11f55d81e0a418fee7857bca65f6e26af | 249 | 888 | 1 |
| 19 | active | 034c3d6f31f56affc5caab7d870244449db030463ce33ca1fa1e147a02fcc89f | 249 | 888 | 1 |
| 20 | active | 4f1ef228d3a144ed7305fa6bbc937cf21f5f32a814d56aa9ca9fe0661776f8b8 | 249 | 888 | 1 |
| 21 | active | 3007d87907afffcb4fa4b54ee8db060e7c563660017dd0253a47b16ea256a8cf | 249 | 888 | 1 |
| 22 | active | 19fc44c80c919be640b39ff73b62bf20584dde2415f70e67be2a481b91ec373c | 249 | 888 | 1 |
| 23 | active | 03082f3452b286ae98f2beaa2bfb3026020c67f829828086d456c290bff03eb7 | 249 | 888 | 1 |
| 24 | active | 4c669ac3dce90064358bca86c75be6abbd7eca2b11723dc7c15b5eb3610c0d50 | 249 | 888 | 1 |
| 25 | active | 47ea486acced9f7f25e1ad735fee4df29f2199b2eb3e31937b4aedfe0b657bfa | 249 | 888 | 1 |
| 26 | active | 0b4d2b410776ea8e475aee0fe792e1722e5cfa4c32ae247d8e7e00afd59340c3 | 249 | 888 | 1 |
| 27 | active | 5692e7f73a43e327cab1cc308c7aa729a952a7614fec2af0f8bbae4e59083bb7 | 249 | 888 | 1 |
| 28 | active | 47dfbcc72e5ca32c1c139b38e7f68626478a8bd4e3d953b4629311314b76c35a | 249 | 888 | 1 |
| 29 | active | 63fad2ecacbe616afa49e251257ef9412db3adbcce65b465e072d60bf4427d00 | 249 | 888 | 1 |
| 30 | active | 24e5c1746ead2405a24f2e7e3dba0d112f1bac8d68d13518667d151c18efe523 | 249 | 888 | 1 |
| 31 | active | 10657c3ac6e2ac950a64a9d0e0498b2886d1ca808ac4ffd47efeea5eb3d8db75 | 249 | 888 | 1 |
| 32 | active | 26b619ebef3d8b73a31d58caea5503c89df953fab788d444b6e2015f0ea57730 | 249 | 888 | 1 |
| 33 | active | 0642e4ae5c0fd277349b7f4498bc199596ef904a3a1e63f87437200eb4459208 | 249 | 888 | 1 |
| 34 | active | 688451da7195693c140c32cca06e11ecca99f8d9b778bc5bc0c1d0fcceeabef7 | 249 | 888 | 1 |
| 35 | active | 19ed3a4d54e28f32ac732ec73b70df45b18e56ffff1e9e413c0db2a27711b812 | 249 | 888 | 1 |
| 36 | active | 03e59173d32547e232407672e994ea8b39ddc019fc5a6ff9389a59dc5741f8d0 | 249 | 888 | 1 |
| 37 | active | 65c2ae1e4164ea31bc23f0036dc951bced5f5a6081cf1b7d4fe24dba23004d5e | 249 | 888 | 1 |
| 38 | active | 7b9a8dcd89bb6017c4be4530a23d5bf2d550bdd9fc9fcc2aee92dd59792ab6af | 249 | 888 | 1 |
| 39 | active | 74994d8ed0e49af1f5caff6a73d9747c44df6558b619f60758cb117091af6fce | 249 | 888 | 1 |
| 40 | active | 0b77efc74d8a12fe8ac37a0dbe7fe3de261a7bd2263888d42abbcf6d8a747497 | 249 | 888 | 1 |
| 41 | active | 55a0ff916ab2e8c26759dec7c2e790ca5bb80f26d84d74f8c3c514501ff11acd | 249 | 888 | 1 |
| 42 | active | 1280df94b3003cc5bb91e5a880bf5ea19c33ebb6e86487f7c0d4552cdae05cc8 | 249 | 888 | 1 |
| 43 | active | 325216c52cea590d09d8e7dd0d28b41e7b032c0dcfc07321050ebeff4e485bc7 | 249 | 888 | 1 |
| 44 | active | 5b13d652042535ecde80f09c79bb0961095853a3dd20a95b74539e541a5133b4 | 249 | 888 | 1 |
| 45 | active | 238bc0aa234034f0f346c64868aa9c6a020b0da546e8263555477ca0142cddf6 | 249 | 888 | 1 |
| 46 | active | 7c159e25a5817b4f0130dc20b74136b3b22f7be89e9048f06210e93642e14dfd | 249 | 888 | 1 |
| 47 | active | 36b69b7dcd13f06f120dfafa78c2c3423dc8bef8e73a4d490206885804dc0034 | 249 | 888 | 1 |
| 48 | active | 19ee286c6ac23b34660d303dd4013d55b6798ab93f7808061ffef12b551eb74a | 249 | 888 | 1 |
| 49 | active | 03920514851fc5379e2acaa351ad89c0f2cbcd5269aeac8067a8aab5bb903fe6 | 249 | 888 | 1 |
| 50 | active | 62aea72eecad6a5c340503ba739b19ee437f338b62f8b00a88eaacd33718d3d7 | 249 | 888 | 1 |
| 51 | active | 3e0c774e914e0ccf6dd704e33e81ee85c2712eebcc1a786835ed0c408fdee598 | 249 | 888 | 1 |
| 52 | active | 3ebf261c4f8978633775c56912250fe80bef0cd5b105258466ee2b251b9233d0 | 249 | 888 | 1 |
| 53 | active | 6fada913cf70bed14f092229646347bf106c681b80a34fbb92bd04898239a5b4 | 249 | 888 | 1 |
| 54 | active | 35a2a187c273e60aa79ebdee27b62d4b4b99f268698508c8bd31bc605eea77fb | 249 | 888 | 1 |
| 55 | active | 70910c657c69f3cb299fc24b9c46478c13296f23f6cf5f29e2deed2e857683fb | 249 | 888 | 1 |
| 56 | active | 41292abd63aeda18cf9a6dc2fdd789cb3cbe92934c860247137ce7eba002489f | 249 | 888 | 1 |
| 57 | active | 3fd12019a1059e97944f652d0fb9ec16d3cddaee3ebb5f835bb419e559e46663 | 249 | 888 | 1 |
| 58 | active | 780eeaa9eef080a9cf96c5f36f683526d2f98a1573b151bc374feb1cb7fe5c7d | 249 | 888 | 1 |
| 59 | active | 7fda5f280e22b661b76e85f8a84f33e9dd91b76745feca8a017d89f7035e8138 | 249 | 888 | 1 |
| 60 | active | 5688dcb86c98878b3f1eb905ae5ffb0fab0081ba2aa6738dbd2ee37b8c47ad80 | 249 | 888 | 1 |
| 61 | active | 1efbc8fb8fd2b443f34938166fa34359eb52a7865309f8f3f0e29e6596531f10 | 249 | 888 | 1 |
| 62 | active | 680d19e60c9b69348fb073119ff0f805fd7f1ec227be574105ece8f33eeb55dc | 249 | 888 | 1 |
| 63 | active | 65d7623c27e10ac87b4eb3f99c718fb8ca0565f189286f6af16b439c3f7f668e | 249 | 888 | 1 |
| 64 | active | 18d205c685ee90737efe71b4ca7a206e3e68228667848f525e544537e9f9a092 | 249 | 888 | 1 |
| 65 | active | 0eaa4fab31706d044c108c474f90b6dcedf0d8494688550d84c318f1c2cb574f | 249 | 888 | 1 |
| 66 | active | 32e8fbc3e811542c51876ed52e64d96db80c8a23be15c3f78d2c6b0e31e27e24 | 249 | 888 | 1 |
| 67 | active | 09c33856f6e8560da89dc4e9e3e042699ec457f5f649f5b8e76c86c4157d87a9 | 249 | 888 | 1 |
| 68 | active | 4be9aff9f6a35659387329512061e3097e928b36477ecf1cbf5bbd394b8c0b64 | 249 | 888 | 1 |
| 69 | active | 045edc89e4bd6734694e1da62bab71edc13468623dc248739e0a7f1186d05542 | 249 | 888 | 1 |
| 70 | active | 35bef8fefaf6de48bdfb6936cbeec87bc42760a548e45d61fb0d21c3283636eb | 249 | 888 | 1 |
| 71 | active | 0d91b77867ac9c0f798adb0d6268277a5ed5b7c5d1dce463a48c12cfeee20d0b | 249 | 888 | 1 |
| 72 | active | 1b882cb143b8790abeefc96e02ab16f9c8d89d6c026bf2a125b117e9e5d40b7d | 249 | 888 | 1 |
| 73 | active | 6abcd131db0a87b7ae2dfc1b6bf26c7aae9961a37b4d0fdf0f6934abfe15dcda | 249 | 888 | 1 |
| 74 | active | 36bf916bdbeeba47a2c65dd222411c1e6a53ed4c4f77a115f62f12c7a377a526 | 249 | 888 | 1 |
| 75 | active | 62ae2b09e0c05c164497d9ad4798ae2d4ac04faac6c4ed0770d22cb602b15ac0 | 249 | 888 | 1 |
| 76 | active | 0e77c9fd481459dc95a8b05aee552fd9a80eebfc4535b0beaf45666874d63d0b | 249 | 888 | 1 |
| 77 | active | 1f97d2caa0f259fb39937e2d11fb74c73fcad3e8f16fbe019eba8a9a6a2e279d | 249 | 888 | 1 |
| 78 | active | 1e4ab46ff4595113165ae45c5b9d3bfbc38cfdcbce8b703ef2e26d3888138788 | 249 | 888 | 1 |
| 79 | active | 555793e88c0fab7a39187c6d70ab5ef1678dd01c3f830d9a37e87a46ec762eda | 249 | 888 | 1 |
| 80 | active | 5bc31b5db9392a9e6afe478b9cd52329c1ec2f4ae3d5f8505e7044b5b038553c | 249 | 888 | 1 |
| 81 | active | 325a973f8f05cb108a5d42e0f0790e4bdd19610434d3546d88e6a15b8faa82a0 | 249 | 888 | 1 |
| 82 | active | 584131ab962c9606b78bc74b8c4522446c3f3bd3d31e66741ce1d0c48ff09aef | 249 | 888 | 1 |
| 83 | active | 6265b2be357c5e2a974955df6156e7844cca5ca513e1303e6adab0a299eeca23 | 249 | 888 | 1 |
| 84 | active | 1edd9ce5c70d7b9588ce3b1bfe55cde624a89e5f77cc9c51baac38c98011725f | 249 | 888 | 1 |
| 85 | active | 35bba695b1aa48b35900ff350e4f8c8910eac0c4ce64979ead4eb84d97713f63 | 249 | 888 | 1 |
| 86 | active | 2c99e4d25e52600592474e49cff0bf76dbb92cbb7c9963fa7164e5c42ef2e098 | 249 | 888 | 1 |
| 87 | active | 2203217d5350b6db3eb8e70861e5b6e80a5a60764bddef74374f0d5ac1c7a423 | 249 | 888 | 1 |
| 88 | active | 39710ee4980f4c2c414b6e4c6c61b08290ea20076b6176829a6b33c68fdfcff8 | 249 | 888 | 1 |
| 89 | active | 3667b57919674344fcf78065365ab6e57c9fabcf647fc57737307b162ba3c3c0 | 249 | 888 | 1 |
| 90 | active | 13012d53693aabcd92f0276dbe2771e1050af9e0b9ed904419291b3c240a6cfe | 249 | 888 | 1 |
| 91 | active | 32e4e4ac15584509cc5362c62fedf5cdcc92b59eb01caea751e34b49d9c06ab2 | 249 | 888 | 1 |
| 92 | active | 70113fb6a560703d241bea7c3211c2609a6eb190d92c5272c1e68bcaecb8e1f3 | 249 | 888 | 1 |
| 93 | active | 7be67df995019e0ef3b342abaf6edac150595d9a8336fd5c9922da176df54706 | 249 | 888 | 1 |
| 94 | active | 2d80f22f6b1898f03fba80c021a5bd02ccb5372b26661088c0582a3b50779232 | 249 | 888 | 1 |
| 95 | active | 343eaeb49e430020a84bf891d17d39e0f028ae66f1c03128650cca9dbb0801c6 | 249 | 888 | 1 |
| 96 | active | 3cab99e4beaa649377240cd69203ce7387303f0c6ad5d21facf538c7f5e12a54 | 249 | 888 | 1 |
| 97 | active | 02cf01994c96f2296ed6ea7a83f09b10ca1abea42264f47bb333b13b777ad402 | 249 | 888 | 1 |
| 98 | active | 18ad435943597d31d14d42c710f369fde8a1fb484d22a72a1cca42c893102874 | 249 | 888 | 1 |
| 99 | active | 395115dea022305d4d1a3ef869fe9eeeaba12c8d19f3406920117167e7c0172d | 249 | 888 | 1 |
| 100 | active | 584e045cd9f3b45855b8f3b4fbcf56c281d716932f30e8f35265271208885f6d | 249 | 888 | 1 |
| 101 | stale | 291fb977943bf9cec15f2572c6b9ba204344a0d896b3adf85c570759ab6d9fdb | 485 | 1832 | 3 |
| 102 | stale | 055359b6e245148e0e02bd513f35c86bb00b82fe80bff6f4c85884db207e01df | 249 | 888 | 1 |
| 101 | active | 3f1e7cf14eb24b57d72a2e3e8e21b3dcaba18509a20526585cbb0e56723699a1 | 398 | 1484 | 2 |
| 102 | active | 3a26bc96d24f94cf84642bafbc66fb97c85166f10e61d1a365f5bea8f68972c5 | 336 | 1236 | 2 |
| 103 | active | 61896cbe35d327e8b1d4e567a4bf05d928d0c761421440cb7055eff87d89cbd3 | 249 | 888 | 1 |

### Snapshot 2 — height 104

Committed tip: `72a69d0d11efe8c4ea596e5e741925ad21d8c7cd329b0227aec907bde8a1f5da`

| Height | State | Full block hash | Bytes | Weight | Transactions |
| --- | --- | --- | --- | --- | --- |
| 0 | active | 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206 | 285 | 1140 | 1 |
| 1 | active | 76f75e47d8652eda37915e724301619a49c0a1341004a551c6f505333142ce16 | 249 | 888 | 1 |
| 2 | active | 3410c782209337f022f26682ef3e44a2d0f76bde02da648dc30376eec3873451 | 249 | 888 | 1 |
| 3 | active | 0472c2bdc5d7de0943e76e4606d7029e7b4a4a70bd3efdd560bc9c9fce8bafd6 | 249 | 888 | 1 |
| 4 | active | 3048455d01da5e28484f925a715347d64ae929f8ccbdc65d965cc9b08db3e31d | 249 | 888 | 1 |
| 5 | active | 204578ccded43310ae189235fba1b7c9e9a165217f8e6b8d1c084fd86989879e | 249 | 888 | 1 |
| 6 | active | 01644b09300f0c51b4180389766e67f05e3e9197c68450b18ecaa1cc58dd8268 | 249 | 888 | 1 |
| 7 | active | 25d612b52dc7bab081b289bd262048a9d400a6e0f2f98d1a989c972094c82bb9 | 249 | 888 | 1 |
| 8 | active | 434a731bc3cab56877b9a2b9f00493526a188d6a1e2d0b8e98502981a9500087 | 249 | 888 | 1 |
| 9 | active | 6f94e44d7976a2e4cf817160d643214d87cc96ade3f7a28ccec161ddd28e7acb | 249 | 888 | 1 |
| 10 | active | 4317629ed1039f2e825fa2a06ac34a011b89ed2f9f6903d9b7664bdbc3d8c676 | 249 | 888 | 1 |
| 11 | active | 192f2587955c7d145c9dee82d5bc0d45411f431b8b8c70126a1cb6775fdfab64 | 249 | 888 | 1 |
| 12 | active | 2925e49a676d83b5c265605327b74c4f210ce859c7f0d8f354f0feab8cfe78f8 | 249 | 888 | 1 |
| 13 | active | 0572d35235ef2a3c373f2f6559e4ccfe61bdd788fe359057ed881f156707c747 | 249 | 888 | 1 |
| 14 | active | 7d0a4efd72b64efa182ea0f48ddd06751be13b59dd3ef89d3a5bc1360e1981b2 | 249 | 888 | 1 |
| 15 | active | 071f0734f81c7aaeff4238a92ef8ca91f3887b0e90d73e4d088aa9a5bc450491 | 249 | 888 | 1 |
| 16 | active | 381d9dc73001ec97522ab23ce63f1648ee7fd885a18820b102b790ed78458b38 | 249 | 888 | 1 |
| 17 | active | 4222d3ca963d94c18be39f3bd9ab8436259781f68fde24a036cbf40590d28edf | 249 | 888 | 1 |
| 18 | active | 14dd829094a46717514ddd54a548dac11f55d81e0a418fee7857bca65f6e26af | 249 | 888 | 1 |
| 19 | active | 034c3d6f31f56affc5caab7d870244449db030463ce33ca1fa1e147a02fcc89f | 249 | 888 | 1 |
| 20 | active | 4f1ef228d3a144ed7305fa6bbc937cf21f5f32a814d56aa9ca9fe0661776f8b8 | 249 | 888 | 1 |
| 21 | active | 3007d87907afffcb4fa4b54ee8db060e7c563660017dd0253a47b16ea256a8cf | 249 | 888 | 1 |
| 22 | active | 19fc44c80c919be640b39ff73b62bf20584dde2415f70e67be2a481b91ec373c | 249 | 888 | 1 |
| 23 | active | 03082f3452b286ae98f2beaa2bfb3026020c67f829828086d456c290bff03eb7 | 249 | 888 | 1 |
| 24 | active | 4c669ac3dce90064358bca86c75be6abbd7eca2b11723dc7c15b5eb3610c0d50 | 249 | 888 | 1 |
| 25 | active | 47ea486acced9f7f25e1ad735fee4df29f2199b2eb3e31937b4aedfe0b657bfa | 249 | 888 | 1 |
| 26 | active | 0b4d2b410776ea8e475aee0fe792e1722e5cfa4c32ae247d8e7e00afd59340c3 | 249 | 888 | 1 |
| 27 | active | 5692e7f73a43e327cab1cc308c7aa729a952a7614fec2af0f8bbae4e59083bb7 | 249 | 888 | 1 |
| 28 | active | 47dfbcc72e5ca32c1c139b38e7f68626478a8bd4e3d953b4629311314b76c35a | 249 | 888 | 1 |
| 29 | active | 63fad2ecacbe616afa49e251257ef9412db3adbcce65b465e072d60bf4427d00 | 249 | 888 | 1 |
| 30 | active | 24e5c1746ead2405a24f2e7e3dba0d112f1bac8d68d13518667d151c18efe523 | 249 | 888 | 1 |
| 31 | active | 10657c3ac6e2ac950a64a9d0e0498b2886d1ca808ac4ffd47efeea5eb3d8db75 | 249 | 888 | 1 |
| 32 | active | 26b619ebef3d8b73a31d58caea5503c89df953fab788d444b6e2015f0ea57730 | 249 | 888 | 1 |
| 33 | active | 0642e4ae5c0fd277349b7f4498bc199596ef904a3a1e63f87437200eb4459208 | 249 | 888 | 1 |
| 34 | active | 688451da7195693c140c32cca06e11ecca99f8d9b778bc5bc0c1d0fcceeabef7 | 249 | 888 | 1 |
| 35 | active | 19ed3a4d54e28f32ac732ec73b70df45b18e56ffff1e9e413c0db2a27711b812 | 249 | 888 | 1 |
| 36 | active | 03e59173d32547e232407672e994ea8b39ddc019fc5a6ff9389a59dc5741f8d0 | 249 | 888 | 1 |
| 37 | active | 65c2ae1e4164ea31bc23f0036dc951bced5f5a6081cf1b7d4fe24dba23004d5e | 249 | 888 | 1 |
| 38 | active | 7b9a8dcd89bb6017c4be4530a23d5bf2d550bdd9fc9fcc2aee92dd59792ab6af | 249 | 888 | 1 |
| 39 | active | 74994d8ed0e49af1f5caff6a73d9747c44df6558b619f60758cb117091af6fce | 249 | 888 | 1 |
| 40 | active | 0b77efc74d8a12fe8ac37a0dbe7fe3de261a7bd2263888d42abbcf6d8a747497 | 249 | 888 | 1 |
| 41 | active | 55a0ff916ab2e8c26759dec7c2e790ca5bb80f26d84d74f8c3c514501ff11acd | 249 | 888 | 1 |
| 42 | active | 1280df94b3003cc5bb91e5a880bf5ea19c33ebb6e86487f7c0d4552cdae05cc8 | 249 | 888 | 1 |
| 43 | active | 325216c52cea590d09d8e7dd0d28b41e7b032c0dcfc07321050ebeff4e485bc7 | 249 | 888 | 1 |
| 44 | active | 5b13d652042535ecde80f09c79bb0961095853a3dd20a95b74539e541a5133b4 | 249 | 888 | 1 |
| 45 | active | 238bc0aa234034f0f346c64868aa9c6a020b0da546e8263555477ca0142cddf6 | 249 | 888 | 1 |
| 46 | active | 7c159e25a5817b4f0130dc20b74136b3b22f7be89e9048f06210e93642e14dfd | 249 | 888 | 1 |
| 47 | active | 36b69b7dcd13f06f120dfafa78c2c3423dc8bef8e73a4d490206885804dc0034 | 249 | 888 | 1 |
| 48 | active | 19ee286c6ac23b34660d303dd4013d55b6798ab93f7808061ffef12b551eb74a | 249 | 888 | 1 |
| 49 | active | 03920514851fc5379e2acaa351ad89c0f2cbcd5269aeac8067a8aab5bb903fe6 | 249 | 888 | 1 |
| 50 | active | 62aea72eecad6a5c340503ba739b19ee437f338b62f8b00a88eaacd33718d3d7 | 249 | 888 | 1 |
| 51 | active | 3e0c774e914e0ccf6dd704e33e81ee85c2712eebcc1a786835ed0c408fdee598 | 249 | 888 | 1 |
| 52 | active | 3ebf261c4f8978633775c56912250fe80bef0cd5b105258466ee2b251b9233d0 | 249 | 888 | 1 |
| 53 | active | 6fada913cf70bed14f092229646347bf106c681b80a34fbb92bd04898239a5b4 | 249 | 888 | 1 |
| 54 | active | 35a2a187c273e60aa79ebdee27b62d4b4b99f268698508c8bd31bc605eea77fb | 249 | 888 | 1 |
| 55 | active | 70910c657c69f3cb299fc24b9c46478c13296f23f6cf5f29e2deed2e857683fb | 249 | 888 | 1 |
| 56 | active | 41292abd63aeda18cf9a6dc2fdd789cb3cbe92934c860247137ce7eba002489f | 249 | 888 | 1 |
| 57 | active | 3fd12019a1059e97944f652d0fb9ec16d3cddaee3ebb5f835bb419e559e46663 | 249 | 888 | 1 |
| 58 | active | 780eeaa9eef080a9cf96c5f36f683526d2f98a1573b151bc374feb1cb7fe5c7d | 249 | 888 | 1 |
| 59 | active | 7fda5f280e22b661b76e85f8a84f33e9dd91b76745feca8a017d89f7035e8138 | 249 | 888 | 1 |
| 60 | active | 5688dcb86c98878b3f1eb905ae5ffb0fab0081ba2aa6738dbd2ee37b8c47ad80 | 249 | 888 | 1 |
| 61 | active | 1efbc8fb8fd2b443f34938166fa34359eb52a7865309f8f3f0e29e6596531f10 | 249 | 888 | 1 |
| 62 | active | 680d19e60c9b69348fb073119ff0f805fd7f1ec227be574105ece8f33eeb55dc | 249 | 888 | 1 |
| 63 | active | 65d7623c27e10ac87b4eb3f99c718fb8ca0565f189286f6af16b439c3f7f668e | 249 | 888 | 1 |
| 64 | active | 18d205c685ee90737efe71b4ca7a206e3e68228667848f525e544537e9f9a092 | 249 | 888 | 1 |
| 65 | active | 0eaa4fab31706d044c108c474f90b6dcedf0d8494688550d84c318f1c2cb574f | 249 | 888 | 1 |
| 66 | active | 32e8fbc3e811542c51876ed52e64d96db80c8a23be15c3f78d2c6b0e31e27e24 | 249 | 888 | 1 |
| 67 | active | 09c33856f6e8560da89dc4e9e3e042699ec457f5f649f5b8e76c86c4157d87a9 | 249 | 888 | 1 |
| 68 | active | 4be9aff9f6a35659387329512061e3097e928b36477ecf1cbf5bbd394b8c0b64 | 249 | 888 | 1 |
| 69 | active | 045edc89e4bd6734694e1da62bab71edc13468623dc248739e0a7f1186d05542 | 249 | 888 | 1 |
| 70 | active | 35bef8fefaf6de48bdfb6936cbeec87bc42760a548e45d61fb0d21c3283636eb | 249 | 888 | 1 |
| 71 | active | 0d91b77867ac9c0f798adb0d6268277a5ed5b7c5d1dce463a48c12cfeee20d0b | 249 | 888 | 1 |
| 72 | active | 1b882cb143b8790abeefc96e02ab16f9c8d89d6c026bf2a125b117e9e5d40b7d | 249 | 888 | 1 |
| 73 | active | 6abcd131db0a87b7ae2dfc1b6bf26c7aae9961a37b4d0fdf0f6934abfe15dcda | 249 | 888 | 1 |
| 74 | active | 36bf916bdbeeba47a2c65dd222411c1e6a53ed4c4f77a115f62f12c7a377a526 | 249 | 888 | 1 |
| 75 | active | 62ae2b09e0c05c164497d9ad4798ae2d4ac04faac6c4ed0770d22cb602b15ac0 | 249 | 888 | 1 |
| 76 | active | 0e77c9fd481459dc95a8b05aee552fd9a80eebfc4535b0beaf45666874d63d0b | 249 | 888 | 1 |
| 77 | active | 1f97d2caa0f259fb39937e2d11fb74c73fcad3e8f16fbe019eba8a9a6a2e279d | 249 | 888 | 1 |
| 78 | active | 1e4ab46ff4595113165ae45c5b9d3bfbc38cfdcbce8b703ef2e26d3888138788 | 249 | 888 | 1 |
| 79 | active | 555793e88c0fab7a39187c6d70ab5ef1678dd01c3f830d9a37e87a46ec762eda | 249 | 888 | 1 |
| 80 | active | 5bc31b5db9392a9e6afe478b9cd52329c1ec2f4ae3d5f8505e7044b5b038553c | 249 | 888 | 1 |
| 81 | active | 325a973f8f05cb108a5d42e0f0790e4bdd19610434d3546d88e6a15b8faa82a0 | 249 | 888 | 1 |
| 82 | active | 584131ab962c9606b78bc74b8c4522446c3f3bd3d31e66741ce1d0c48ff09aef | 249 | 888 | 1 |
| 83 | active | 6265b2be357c5e2a974955df6156e7844cca5ca513e1303e6adab0a299eeca23 | 249 | 888 | 1 |
| 84 | active | 1edd9ce5c70d7b9588ce3b1bfe55cde624a89e5f77cc9c51baac38c98011725f | 249 | 888 | 1 |
| 85 | active | 35bba695b1aa48b35900ff350e4f8c8910eac0c4ce64979ead4eb84d97713f63 | 249 | 888 | 1 |
| 86 | active | 2c99e4d25e52600592474e49cff0bf76dbb92cbb7c9963fa7164e5c42ef2e098 | 249 | 888 | 1 |
| 87 | active | 2203217d5350b6db3eb8e70861e5b6e80a5a60764bddef74374f0d5ac1c7a423 | 249 | 888 | 1 |
| 88 | active | 39710ee4980f4c2c414b6e4c6c61b08290ea20076b6176829a6b33c68fdfcff8 | 249 | 888 | 1 |
| 89 | active | 3667b57919674344fcf78065365ab6e57c9fabcf647fc57737307b162ba3c3c0 | 249 | 888 | 1 |
| 90 | active | 13012d53693aabcd92f0276dbe2771e1050af9e0b9ed904419291b3c240a6cfe | 249 | 888 | 1 |
| 91 | active | 32e4e4ac15584509cc5362c62fedf5cdcc92b59eb01caea751e34b49d9c06ab2 | 249 | 888 | 1 |
| 92 | active | 70113fb6a560703d241bea7c3211c2609a6eb190d92c5272c1e68bcaecb8e1f3 | 249 | 888 | 1 |
| 93 | active | 7be67df995019e0ef3b342abaf6edac150595d9a8336fd5c9922da176df54706 | 249 | 888 | 1 |
| 94 | active | 2d80f22f6b1898f03fba80c021a5bd02ccb5372b26661088c0582a3b50779232 | 249 | 888 | 1 |
| 95 | active | 343eaeb49e430020a84bf891d17d39e0f028ae66f1c03128650cca9dbb0801c6 | 249 | 888 | 1 |
| 96 | active | 3cab99e4beaa649377240cd69203ce7387303f0c6ad5d21facf538c7f5e12a54 | 249 | 888 | 1 |
| 97 | active | 02cf01994c96f2296ed6ea7a83f09b10ca1abea42264f47bb333b13b777ad402 | 249 | 888 | 1 |
| 98 | active | 18ad435943597d31d14d42c710f369fde8a1fb484d22a72a1cca42c893102874 | 249 | 888 | 1 |
| 99 | active | 395115dea022305d4d1a3ef869fe9eeeaba12c8d19f3406920117167e7c0172d | 249 | 888 | 1 |
| 100 | active | 584e045cd9f3b45855b8f3b4fbcf56c281d716932f30e8f35265271208885f6d | 249 | 888 | 1 |
| 101 | active | 291fb977943bf9cec15f2572c6b9ba204344a0d896b3adf85c570759ab6d9fdb | 485 | 1832 | 3 |
| 102 | active | 055359b6e245148e0e02bd513f35c86bb00b82fe80bff6f4c85884db207e01df | 249 | 888 | 1 |
| 101 | stale | 3f1e7cf14eb24b57d72a2e3e8e21b3dcaba18509a20526585cbb0e56723699a1 | 398 | 1484 | 2 |
| 102 | stale | 3a26bc96d24f94cf84642bafbc66fb97c85166f10e61d1a365f5bea8f68972c5 | 336 | 1236 | 2 |
| 103 | stale | 61896cbe35d327e8b1d4e567a4bf05d928d0c761421440cb7055eff87d89cbd3 | 249 | 888 | 1 |
| 103 | active | 58e67492d1634ca0c51fce66fdff328bdb2a120472bcd730a53dbf18b8802006 | 249 | 888 | 1 |
| 104 | active | 72a69d0d11efe8c4ea596e5e741925ad21d8c7cd329b0227aec907bde8a1f5da | 249 | 888 | 1 |
