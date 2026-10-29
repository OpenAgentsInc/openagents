# Bounded scrypt BlockMix, initial evidence

This is RFC 7914 BlockMix for `r=1..8`, including NIP-49's `r=8`; it is
not ROMix, PBKDF2, or complete scrypt. It is not approved for production
secrets. The internal ABI requires valid views and a truthful non-overlap
observation. Temporary values are not yet proven erased, and source checks
do not prove target side-channel behavior. ROMix has password-dependent
memory indexing and needs a distinct threat model rather than a blanket
constant-time claim.

The RFC 7914 section 9 vector and an `r=2` permutation check pass. All 375
mutants in the loaded graph were killed. The independent byte/block oracle in
`coax/tests/nostr/scrypt_blockmix.py` checked 128 inputs covering every admitted
`r` on each checked and release native and Wasm profile, including boundary
refusals and ASan/UBSan on native. Both profiles also compile for arm64 iOS
Simulator; this does not claim iOS execution or a physical-device result.

The run used coax0 0.22.0 (`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`),
BlockMix source `2d309c6398b6c4c335e591b3922c6fefb967010c6fb9ff630c7d261e30f16b95`,
and the Salsa core `315fc51b00354f48bd45781675e0017bf8564f895b837933cbf27d663e4df816`.
Checked C and Wasm SHA-256 values were
`3b0052d5b7fdfd01b87b18f87adf0fba387614bde55b957e2fad229f6d910871` and
`022b9781484fdcba3d04c1557b77c84b625a593d802517f1d4f048b0bed9062f`;
release values were `e788450f3829ca9d375a624f30f607a1d2a1c8a289e1cc5b3b7425ac5df3bd42`
and `86921b57a25626a33e89bbcfc9a4aec3868897d877b35b8ce2ef2e176f8d8aab`.
The run used arm64 macOS, Homebrew clang 23.1.4, Node 26.12.1, and iOS
Simulator SDK 26.5.
An independent source review found no concrete BlockMix arithmetic or
admission defect in this scope; it did not approve target timing, erasure,
ROMix or production use.
