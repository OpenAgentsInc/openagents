# libsecp256k1 (vendored)

Bitcoin Core's secp256k1 library, used by the app for the phone's Nostr key:
BIP-340 Schnorr signatures (`OpenAgents/Platform/NostrSign.c`). The server
verifies with the same library (`coax/web/api/nostr.c`). This stays until
Coax's own secp256k1 and Schnorr land and pass their own checks.

- Release: v0.6.0, tag object 22616f64b96353b817fe7b3cdc8b7df5d51898e9,
  commit 0cdc758a56360bf58a851fe91085a327ec97685a.
- Source: https://github.com/bitcoin-core/secp256k1/archive/refs/tags/v0.6.0.tar.gz,
  SHA-256 785bb98e7d6705c51c8dfa8ac3af6aa2ccfa3774714d51c0b9e28fac1146e9f1.
- Cross-checked against the copy in the Rust crate secp256k1-sys 0.11.0
  (same commit): with its `rustsecp256k1_v0_11_` prefix removed, the files
  differ only by that crate's known patches (it drops the context and scratch
  allocators).
- Kept: `COPYING` (MIT), the headers the app uses, `src/*.h` minus test and
  benchmark helpers, `src/secp256k1.c`, `src/precomputed_ecmult_gen.c`, and the
  extrakeys and schnorrsig modules.
- Changed: `src/precomputed_ecmult.c` keeps only the table entries for
  `ECMULT_WINDOW_SIZE` up to 8 (the `#if WINDOW_G > n` blocks for n >= 8
  removed, values untouched): it only speeds up verification, which the phone
  does once per signature. Built with `ECMULT_WINDOW_SIZE=8`,
  `ENABLE_MODULE_EXTRAKEYS=1` and `ENABLE_MODULE_SCHNORRSIG=1` (`project.yml`).
- Checked by `Tests/SignInUITests.swift` (BIP-340 vectors 0 and 1, and every
  request the app signs).

None of the library's build scripts were run.
