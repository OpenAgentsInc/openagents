# Contents

- [Introduction](README.md)
- [Plan and prerequisites](PLAN.md)

**I. Foundations**

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) — implemented
- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) — implemented
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md) — implemented

**II. Hashes and MACs**

- [4. SHA-256](ch004-sha-256.md) — implemented
- [5. HMAC-SHA256](ch005-hmac-sha256.md) — implemented
- [6. HKDF](ch006-hkdf.md) — implemented

**III. Elliptic curves**

- [7. Field arithmetic mod p](ch007-field-arithmetic-mod-p.md) — implemented
- [8. Scalars mod n](ch008-scalars-mod-n.md) — implemented
- [9. The secp256k1 group law](ch009-the-secp256k1-group-law.md) — implemented
- [10. Scalar multiplication](ch010-scalar-multiplication.md) — implemented
- [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md) — implemented
- [12. Keys and randomness](ch012-keys-and-randomness.md) — implemented
- [13. Sign with Schnorr](ch013-sign-with-schnorr.md) — implemented
- [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md) — implemented

**IV. Encodings**

- [15. Bech32](ch015-bech32.md) — implemented
- [16. NIP-19 entities and nostr: URIs](ch016-nip-19-entities-and-nostr-uris.md) — implemented
- [17. JSON you can sign](ch017-json-you-can-sign.md) — implemented

**V. Encryption**

- [18. ChaCha20](ch018-chacha20.md) — implemented
- [19. NIP-04 and why it is retired](ch019-nip-04-and-why-it-is-retired.md) — implemented
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md) — implemented

**VI. The event model**

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md) — implemented
- [22. Kinds and tags](ch022-kinds-and-tags.md) — implemented
- [23. Filters](ch023-filters.md) — implemented
- [24. Relay messages](ch024-relay-messages.md) — implemented
- [25. Gift wrap](ch025-gift-wrap.md) — implemented

**VII. Transport**

- [26. WebSocket frames](ch026-websocket-frames.md) — implemented
- [27. A WebSocket server](ch027-a-websocket-server.md) — implemented
- [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md) — implemented

**VIII. A client**

- [29. A relay connection](ch029-a-relay-connection.md) — implemented
- [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md) — implemented
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md) — implemented
- [32. Many relays and the outbox model](ch032-many-relays-and-the-outbox-model.md) — implemented
- [33. A client that survives restarts](ch033-a-client-that-survives-restarts.md) — implemented

**IX. A relay**

- [34. Admit an event](ch034-admit-an-event.md) — implemented
- [35. Store events](ch035-store-events.md) — implemented
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md) — implemented
- [37. NIP-11 relay information](ch037-nip-11-relay-information.md) — implemented
- [38. NIP-42 authentication](ch038-nip-42-authentication.md) — implemented
- [39. Delete, expire and vanish](ch039-delete-expire-and-vanish.md) — implemented
- [40. Count events](ch040-count-events.md) — implemented
- [41. Search](ch041-search.md) — implemented
- [42. Protected events](ch042-protected-events.md) — implemented
- [43. Negentropy sync](ch043-negentropy-sync.md) — implemented
- [44. Limits, policy and floods](ch044-limits-policy-and-floods.md) — implemented

**X. Every NIP**

- [45. Profiles, follows and names](ch045-profiles-follows-and-names.md) — implemented
- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md) — implemented
- [47. Encrypted key backup](ch047-encrypted-key-backup.md) — implemented
- [48. Signers outside the app](ch048-signers-outside-the-app.md) — implemented
- [49. HTTP auth](ch049-http-auth.md) — implemented
- [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md) — implemented
- [51. References inside content](ch051-references-inside-content.md) — implemented
- [52. Reposts, reactions and comments](ch052-reposts-reactions-and-comments.md) — implemented
- [53. Long-form, drafts and wiki](ch053-long-form-drafts-and-wiki.md) — implemented
- [54. Lists, sets and bookmarks](ch054-lists-sets-and-bookmarks.md) — implemented
- [55. Labels, reports and trusted assertions](ch055-labels-reports-and-trusted-assertions.md) — implemented
- [56. Proof of work](ch056-proof-of-work.md) — implemented
- [57. Media and files](ch057-media-and-files.md) — implemented
- [58. Calendars, polls and highlights](ch058-calendars-polls-and-highlights.md) — implemented
- [59. Groups and communities](ch059-groups-and-communities.md) — implemented
- [60. Private messages](ch060-private-messages.md) — design draft
- [61. Live activities](ch061-live-activities.md) — implemented
- [62. Badges and statuses](ch062-badges-and-statuses.md) — implemented
- [63. Git on Nostr](ch063-git-on-nostr.md) — implemented
- [64. App data and handlers](ch064-app-data-and-handlers.md) — implemented
- [65. Data vending machines](ch065-data-vending-machines.md) — implemented
- [66. Markets and orders](ch066-markets-and-orders.md) — implemented
- [67. Relay discovery, access and management](ch067-relay-discovery-access-and-management.md) — design draft
- [68. Bridges, external IDs and timestamps](ch068-bridges-external-ids-and-timestamps.md) — implemented
- [69. Small content kinds](ch069-small-content-kinds.md) — implemented
- [70. Retired and experimental NIPs](ch070-retired-and-experimental-nips.md) — implemented
- [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md) — implemented
- [72. Wallet Connect](ch072-wallet-connect.md) — implemented
- [73. Payment targets](ch073-payment-targets.md) — implemented
- [74. Cashu and Fedimint are excluded](ch074-cashu-and-fedimint-are-excluded.md) — implemented
- [75. Hosts and reachability](ch075-hosts-and-reachability.md) — implemented
- [76. Work, coordination and control](ch076-work-coordination-and-control.md) — design draft
- [77. Capabilities, policy and extensions](ch077-capabilities-policy-and-extensions.md) — design draft
- [78. Runs, sessions, terminals and trajectories](ch078-runs-sessions-terminals-and-trajectories.md) — design draft
- [79. Agents, skills and automation](ch079-agents-skills-and-automation.md) — design draft
- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md) — design draft
- [81. Decisions, evaluations, optimization and programs](ch081-decisions-evaluations-optimization-and-programs.md) — design draft
- [82. Pylons, environments and deploys](ch082-pylons-environments-and-deploys.md) — implemented
- [83. Attested work and sealed data](ch083-attested-work-and-sealed-data.md) — design draft
- [84. Knowledge and workspaces](ch084-knowledge-and-workspaces.md) — design draft
- [85. Live media, worlds and experience](ch085-live-media-worlds-and-experience.md) — design draft
- [86. Lightning payments for requests](ch086-lightning-payments-for-requests.md) — implemented
- [87. Owner attestation and agent authentication](ch087-owner-attestation-and-agent-authentication.md) — design draft
- [88. Federated identity](ch088-federated-identity.md) — implemented
- [89. Channels](ch089-channels.md) — design draft
- [90. Artifacts, archives and reminders](ch090-artifacts-archives-and-reminders.md) — design draft
- [91. Git signing, projects, leases and read state](ch091-git-signing-projects-leases-and-read-state.md) — implemented
- [92. Private agent state](ch092-private-agent-state.md) — design draft

**XI. Interop and verification**

- [93. Test vectors and their provenance](ch093-test-vectors-and-their-provenance.md) — implemented
- [94. Differential tests](ch094-differential-tests.md) — implemented
- [95. Fuzzing](ch095-fuzzing.md) — implemented
- [96. Formal properties](ch096-formal-properties.md) — design draft
- [97. A conformance report](ch097-a-conformance-report.md)

**XII. Nostr in OpenAgents**

- [98. Projects on Nostr](ch098-projects-on-nostr.md) — implemented
- [99. The website and sign-in](ch099-the-website-and-sign-in.md) — implemented
- [100. Pylons and the mesh](ch100-pylons-and-the-mesh.md) — implemented
- [101. The Verse and the wallet](ch101-the-verse-and-the-wallet.md) — design draft
- [102. Agents on Nostr](ch102-agents-on-nostr.md) — design draft
- [103. Replace relay.openagents.com](ch103-replace-relay-openagents-com.md) — design draft
- [104. Agent treasuries and a network of keys](ch104-agent-treasuries-and-a-network-of-keys.md) — design draft
