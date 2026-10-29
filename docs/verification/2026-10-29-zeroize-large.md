# Large scratch erasure, initial evidence

This experimental policy tracks one caller-owned region up to 256 MiB and
requires each chunk of at most 1 MiB to be acknowledged before completion.
The C adapter validates pointer-range arithmetic and scope overlap, calls the
existing observable volatile-byte eraser, and advances only after its successful
return. A new scope must be zero-initialized before begin. Calls must be serialized;
the caller must keep the allocation and scope
stable, disjoint, truthful and protected from tampering. An acknowledgment is
a trusted host observation, not a cryptographic proof of erasure.

This component is not yet integrated into scrypt's success, failure or
cancellation paths. It does not erase registers, compiler-generated copies,
OS snapshots, or other unregistered regions; it does not establish production
secret-handling approval. Replacing or freeing a live allocation during wiping
violates the contract.

Coax tests exercise chunk boundaries, malformed state, missing buffers,
incorrect acknowledgments and failed effects. All 329 mutants in the loaded
graph were killed. `coax/tests/nostr/zeroize_large.py` checked a real 64 MiB
native wipe, guards, pending/reset behavior and ASan/UBSan for both checked and
release modes; emitted Wasm exercised a real multi-chunk wipe and scope reuse.
The host adapter compiled for arm64 iOS Simulator, without an iOS execution or
physical-device claim.

The run used coax0 0.22.0 (`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`),
policy `ed12ee82292938f71c30495bdff8f9ede98603eeb8b6251ca92df59d8d382839`,
and adapter `2b4ff4cb592bb2fbaac9999ead7e1cac147e23e3a3f8c407039a63552541f008`.
Checked C and Wasm SHA-256 values were
`34066debc7b69a696be59ca7ce42edce9a80659dcaa29757e1a499c94fcf3f2a` and
`ad5b1dd0ec2f5f44fe7202cef536f3880597e5dd885cbdd28d88ec8371c051ff`;
release values were `1c2f0cb70435c36ed12db22d93ee13ee92b877cc64f000379b986bab8c8e037a`
and `e41f4f65c12aef5c4309f29d5e214facb8b801620ad786e83f92451cbb4d6959`.
The run used arm64 macOS, Homebrew clang 23.1.4, Node 26.12.1, and iOS
Simulator SDK 26.5.

An independent source review of the policy and adapter found no concrete defect
under these stated ownership and trust assumptions. It did not approve use for
production secrets or claim erasure of hidden copies.
