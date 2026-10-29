# X-only ECDH acceptance

The controlled owner-only public artifacts return raw 32-byte affine x for
admitted x-only ECDH. They do not use libsecp256k1's default compressed-point
hash. The independently generated fixtures compare the public artifact's
result with raw-x computation and derive the NIP-44 conversation key from it;
the artifact itself does not implement that key derivation.

The [migration evidence](2026-10-29-ecdh-abi-migration.json) records 783
cases in each checked/release native and Wasm public artifact: 35 official
conversation-key cases, 473 valid and 13 invalid-x strictly adapted Wycheproof
cases, 256 seeded random cases, four invalid scalars, one invalid peer and a
seeded default-hash fault. The seeded fault distinguishes the default hash
from the required raw x. Another 266 Wycheproof cases are excluded by the
explicit x-only adaptation. The current source passed 107 Coax checks and
1,842 of 1,842 authority mutations; the full-path check fits the unchanged
20,000,000-step default. These are public correctness observations.

The [owned lifecycle](2026-10-29-ecdh-owned-lifecycle.md) maps every return
of the new public ABI. The private slot starts without previously owned
secret data. Under the stated valid, live, exclusive and disjoint storage and
whole-wipe-effect premises, an accepted peer/scalar error completes only
after the owned work and staging are wiped; cancellation and failure after
an output copy retain their cleanup obligations. Wipe failure is pending,
not a completed error. Pre-acceptance metadata refusal does not claim to wipe
arbitrary caller memory. The legacy caller-workspace API and arbitrary builds
of the raw adapter are outside this controlled artifact and are not silently
treated as having the new contract.

The raw classified source audit remains `accepted=false` with two public
guard obligations. A separate reviewed decision is conditional on the exact
stage and adapter identities, public-peer/secret-scalar classifications,
caller views, source-to-model correspondence and trusted tools; it is not a
target audit. Wasm host memory is accessible to its host. Hidden compiler,
register and ABI copies, crashes, fork/reload cleanup, target/JIT timing and
production-secret suitability remain outside this acceptance.
