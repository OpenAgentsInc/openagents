# Scrypt PBKDF2 building block, initial evidence

This experimental PBKDF2-HMAC-SHA256 operation uses **exactly one iteration**
for scrypt's two PBKDF2 stages. It is not standalone password hardening, full
scrypt, or a complete NIP-49 backup. It accepts password and salt lengths up
to 1,024 bytes and output lengths 1–1,024 bytes. The internal ABI requires
stable valid views and a truthful aggregate overlap observation. Password,
HMAC state, and other secret temporaries are not proven erased; target timing,
public pointer admission and production-secret use remain unapproved.

The RFC 7914 section 11 one-iteration vector passes. All 931 mutants in the
loaded graph were killed. The independent OpenSSL-backed Python PBKDF2 oracle
checked 104 inputs per checked and release native and Wasm profile, including
empty and maximum-size inputs, partial digest blocks and maximum output.
Native ASan/UBSan covered refusals and maximum-size success; both profiles
compiled for arm64 iOS Simulator. No iOS execution or device result is claimed.

The run used coax0 0.22.0 (`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`)
and source `1f8d045b408151b881c3230d84be2d015de4d4b671ccf3b4f60fd53d42e56d83`.
Checked C and Wasm SHA-256 values were
`cb8b669989b497f11466cb5bf15b7bfa394d62860572805eff83e2a649fedb73` and
`09f67a6754dd72bbaf10cfe3593abf3a8e9f51e14d089b325eee9a2c0285f16e`;
release values were `1db8d81679cf0204f109d7a68c97aa2cbd4ba86555455ec2218b10032ee44662`
and `cb2d350ad0a25f97a1f1729c0b3bf8c192783d3038ab4caafc460fede6a4c8e6`.
The run used arm64 macOS, Homebrew clang 23.1.4, Node 26.12.1, and iOS
Simulator SDK 26.5.
An independent source review found no concrete construction or admission
defect in this scope; it did not approve erasure, timing or production use.
