# Experimental outer AEAD workspace

This Coax slice moves the explicitly named outer XChaCha20-Poly1305 MAC input,
Poly1305 one-time key, nonce-derived stream buffer, ciphertext and tag into a
caller-owned `AeadScratch`. Results are staged in `AeadOperation`, and the
cleanup policy withholds publication until the host reports a workspace wipe.
Authentication failure and pre-publication cancellation require both workspace
and staged-result wipes. Successful publication copies a result to caller-owned
output; the host must subsequently erase the staged copy. Early metadata or
alias refusal occurs before initializing the workspace and therefore leaves
pre-existing workspace bytes untouched.

This is an internal prototype, not a checked public pointer adapter. The host
must validate target-width addresses, exact extents and all pairwise aliases;
keep the key, nonce, AAD, input, scratch, state and output stable and disjoint;
serialize calls; own policy-state provenance; and acknowledge only real erasure.
This slice has no production host adapter, cancellation inside the synchronous
cipher operation, target timing approval, or physical-device result. Crucially,
the called XChaCha20, HChaCha20, ChaCha20 and Poly1305 routines still contain
key-derived local temporaries whose erasure is not established. The test's
observable outer-workspace wipe does not prove those copies, registers, OS
snapshots or crashes are handled. No production-secret approval follows.

All 37 loaded Coax checks passed; the complete 984/984 mutation gate ran 3,225
selective checks and skipped 3,197 through the scheduler, with no exclusions.
An initial pass killed 931/981 and prompted independent frozen-vector and
boundary checks and simplification of a redundant initializer. No equivalence
waivers were used.

Checked/release native and actual Wasm each passed 132 cases against independent
HChaCha plus OpenSSL ChaCha20-Poly1305. Each case checks all 16 tag-byte
alterations, first ciphertext byte when present, first nonce byte and first AAD
byte when present, with transactional output preservation. Cases include empty
and maximum AAD and plaintext. This is finite position coverage, not all byte
positions. The test host performs actual volatile erasure and checks the full
outer workspace after every case; failure also wipes the staged region. Native
ASan/UBSan covers a maximum-size seal, not the full tamper loop. Checked/release
iOS 26.5 Simulator executes the draft vector and all tag-byte alterations; it
is not physical-device coverage.

Independent review of source SHA-256
`b15168b63f2a251e55ae7c55c9475ce07e073dc69b213d122ff2be3bf27ba161`
found no concrete authentication or lifecycle blocker under the stated
trusted-host and truthful-extent premises. The reviewer also checked the final
fixture scope and identified the exact sanitizer and byte-position limits above.

Checked generated C SHA-256 `e91b1e694fe644f35ae12e55c01173b50d55e046d92257d48e409e7432038274`,
Wasm `dd524ab1afa6ce23e1804ae125d1fb6748798d603e58c8961b380c91ec63d547`;
release C `bcf69823a56370c3b9b5deee3693ff8057d9d9eca15731051733fcd4e68a4d1b`,
Wasm `7f3f1ba4673e4daf607e14138065477f118304a3e760bf16d10e65dd11f50b66`.
Checked/release simulator executable hashes were
`d0f781d7cc947a4524f931d872b95188e96309bdd2e0da994c25c34d49927602` and
`acc4179281b1099b773dc872c6c46e5cc43cc3cc576bd1d2f4ea54d0a3a25cb5`.
The compiler was coax0 0.22.0 SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.

```sh
coax/bin/coax test coax/lib/crypto/aead_workspace.coax
coax/bin/coax mutate coax/lib/crypto/aead_workspace.coax --mutation-stats
COAX_IOS_SIMULATOR=<booted-simulator-udid> python3 coax/tests/nostr/aead_workspace.py
```

Python/Node are test tools only; runtime code is Coax and platform effects.
