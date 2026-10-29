# Experimental BIP-340 key preparation

`sk_prepare` derives the public x coordinate and parity from a supplied key,
normalizes the scalar for BIP-340, and carries validity and point finiteness
forward. Invalid keys use a fallback for computation and must never be
published merely because the operation returned. Admitted intermediate failure
clears stale validity; metadata refusal leaves existing state untouched.

The recorded source SHA-256 is
`d96f9096b132f002baf75451c6698e2ec0e9656183028380faa60e2869eac305`.
The complete mutation run killed 1,886 of 1,886 mutants. Checked and release
native and WebAssembly tests exercised 509 independent pinned-libsecp and
integer cases; the iOS Simulator exercised valid and invalid keys. A separate
review checked the validity and disjoint-workspace assumptions.

The caller must enforce stable, live, disjoint buffers and honor both the
return value and validity. This slice alone is not a signer. Copies created by
value, inversion, compiler, ABI or registers are not proven erased; target
constant-time behavior and production secret handling are not approved.
