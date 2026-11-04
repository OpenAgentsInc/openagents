# Bitcoin Lab

An unlisted learning workbench at https://openagents.com/worlds/bitcoin-lab.
**FIXTURE:** the Grid and linked blocks are synthetic geometry. There is no
node snapshot, network, chain cursor, transaction history or validation claim.
The existing explorer remains pow-only. No real addresses, balances, secrets
or spending controls appear here.

Walk from spawn at (0, -7) to the Chain plinth at (0, 0) and back. Seven
stations surround the walkway: Chain; Transactions/UTXOs; Proof/Script; Node;
Mempool/Mining; Wallets; Lightning. Plinth outlines are floor markers, not
solid obstacles. All seven stations can be selected from the control bar,
their visible world labels, or keys 1–7. N selects the next station.

Use WASD to walk, Shift to walk slowly, Space to jump, and left/right arrows
to turn the shared follow camera. On touch, drag the left side to walk and
the right side to look. Selecting a control releases walking. Tab and Enter
reach the same controls; Escape releases focus, then exits to [Worlds](/worlds).
M hides or shows the map. Previous/Next and PageUp/PageDown browse all 125
chapters, wrapping at either end. Source opens the selected source chapter.

The map separates the book's exact registry status from the availability of
a visual exercise. **No chapter has a visual exercise in this first slice.**
Selecting a station starts at its first chapter. Chapters may cover multiple
subjects; each has one primary station below. This table is the text fallback.

This realm stays out of normal Worlds, galaxy, Start, sitemap and agent
discovery. A deliberate `?realms=all` diagnostic may show it. Unlisted is not
authentication; direct links are accessible. No public listing is authorized.

<!-- generated chapter map -->

| Chapter | Station | Book status | Visual exercise |
| --- | --- | --- | --- |
| [1. Bytes, hex and base64](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch001-bytes-hex-and-base64.md) | Proof/Script | implemented | not available |
| [2. Big integers in fixed limbs](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch002-big-integers-in-fixed-limbs.md) | Proof/Script | implemented | not available |
| [3. Constant-time rules and secrets](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch003-constant-time-rules.md) | Proof/Script | implemented | not available |
| [4. SHA-256](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch004-sha-256.md) | Proof/Script | implemented | not available |
| [5. SHA-256d and tagged hashes](https://github.com/OpenAgentsInc/openagents/blob/main/bitcoin/book/ch005-sha-256d-and-tagged-hashes.md) | Proof/Script | implemented | not available |
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
