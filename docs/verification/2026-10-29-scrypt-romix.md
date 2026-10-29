# Resumable scrypt ROMix, initial evidence

This experimental ROMix operates on caller-owned scratch in bounded steps. It
accepts `r=1..8`, power-of-two `N=2..262144`, at most 256 MiB of scratch and
1–64 BlockMix operations per step. It is not an approved password KDF or a
complete NIP-49 backup. Callers must keep state and scratch stable, untampered
and disjoint, and establish truthful view/overlap observations. The state,
scratch and intermediate password-derived values are not proven erased on
success, failure or cancellation. There is no public pointer gate, cancellation
or production-secret approval.
The step budget limits iterations, not wall-clock latency. A failed internal
BlockMix invalidates the ROMix phase but does not erase state or scratch;
shape validation alone does not authenticate a restored state.

ROMix deliberately indexes scratch with a password-derived value. This is an
inherent property of the chosen algorithm and is not a constant-time memory
access claim; its threat model and target execution remain separate work.

The RFC 7914 section 10 vector and phase/budget/admission checks pass, as do
all 549 mutants in the loaded graph. `coax/tests/nostr/scrypt_romix.py` compares
27 inputs at `r=1,2,8`, `N=2,4,16` and step budgets 1, 3 and 64 with an
independent byte-oriented model in checked/release native and Wasm. Native
ASan/UBSan and arm64 iOS Simulator compilation also pass; no iOS execution or
physical-device result is claimed.

With `--nip49-native`, the test composes the Coax one-iteration PBKDF2 stage,
ROMix at `N=65536, r=8` using 64 MiB scratch, and PBKDF2 again, and matches
OpenSSL-backed `hashlib.scrypt` in both checked and release native builds.
The measured computation on this host was 0.239 s checked and 0.178 s release;
these are single test observations, not a device or performance guarantee.

The run used coax0 0.22.0 (`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`),
ROMix source `e37b56a7f68bb5d8dda43e0609ff0394450e1dbb19a5d51dff2885faf88c701b`,
and PBKDF2 source `1f8d045b408151b881c3230d84be2d015de4d4b671ccf3b4f60fd53d42e56d83`.
Checked C and Wasm SHA-256 values were
`74eecd22aeee498b3f1d26b4c5c372332657c30d5bff9e7a97bf2f364d0734e8` and
`2b80b9ad7a12c42ea5b82ae1406ad257b1b264c3f4a07508bffcda40be3b72aa`;
release values were `520584c83a3827893f3eacf8bc37327b8fa56c1bed12ca7a50689bae2ea122b3`
and `b04fc896ef9dc78488ae349d30c3e9bda11c90a6f0ca1b6333596e91194023c8`.
The run used arm64 macOS, Homebrew clang 23.1.4, Node 26.12.1, and iOS
Simulator SDK 26.5.
An independent source review found no concrete ROMix correctness blocker
under the documented state and memory premises. It did not approve erasure,
target timing or production use.
