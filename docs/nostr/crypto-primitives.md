# Nostr cryptographic primitives

`crates/nostr` implements NIP-44 v2 with four hand-written primitives in
`crates/nostr/src/nip44.rs`: HMAC-SHA256, HKDF-Expand, ChaCha20, and a
constant-time byte comparison. SHA-256 itself and every secp256k1
operation come from the `sha2` and `secp256k1` crates. This page records
what checks those hand-written primitives, why they are retained, and what
was and was not run. It is the record for issue #9433.

## Decision: retain the hand-written primitives

The four primitives stay as they are. The evidence is in
`crates/nostr/tests/`:

| Primitive | Oracle | Test |
| --- | --- | --- |
| `hmac_sha256` | `hmac::Hmac<sha2::Sha256>` | `nip44_differential.rs`, boundary sizes plus 512 random cases |
| `hkdf_expand`, `message_keys` | `hkdf::Hkdf<Sha256>::from_prk(..).expand(..)` | `nip44_differential.rs`, boundary sizes plus 512 random cases each |
| `chacha20_xor` | `chacha20::ChaCha20` from counter 0, and RFC 8439 section 2.4.2 | `nip44_differential.rs`, boundary sizes up to 65536 bytes plus 512 random cases |
| `constant_time_equal` | `subtle::ConstantTimeEq` and `==` | `nip44_differential.rs`, boundary sizes, random pairs, single-bit flips |
| The whole construction | Official NIP-44 vectors, pinned and checksummed | `nip44_vectors.rs`, every valid and invalid vector |

Boundary sizes are 0, 1, 31, 32, 33, 55, 56, 63, 64, 65, 127, 128, 129, and
1000 bytes: the SHA-256 digest and block edges, the ChaCha20 block edge,
and the empty input.

Why retain rather than migrate:

- The crate's dependency allowlist, carried over from `immortal-core`, is
  `secp256k1`, `sha2`, `serde`, and `serde_json`. Adding `hmac`, `hkdf`,
  `chacha20`, and `subtle` at runtime widens the supply-chain surface of a
  crate whose whole point is to stay small and auditable, and the relay
  builds on it.
- The hand-written code is about 120 lines, has no `unsafe` (the crate is
  `#![forbid(unsafe_code)]`), and now agrees with RustCrypto on every input
  the differential tests generate.
- The comparison is length-then-fold-XOR, the same shape `subtle` uses. It
  is not a formal constant-time proof; neither is `subtle`'s on a compiler
  that may still optimize it. NIP-44's MAC check does not leak more than
  the payload length either way, because the length is public.

What would change the decision: a differential failure, a change to the
NIP-44 construction that needs a primitive this crate does not have, or a
relay decision to accept the larger dependency set. In that case migrate
one primitive per commit, keep `nip44::primitives` as the test seam, and
keep the differential test as a guard until the hand-written body is gone.

## Test surface

`nostr::nip44::primitives` is a `#[doc(hidden)]` public module so the
integration tests can reach the primitives without `cfg(test)` tricks. It
is not a stable API. Nothing outside `crates/nostr/tests/` should import
it.

The RustCrypto crates and `proptest` are dev-dependencies only, pinned to
exact versions in `crates/nostr/Cargo.toml`. They do not enter the relay
binary.

## Vectors and provenance

`crates/nostr/tests/fixtures/README.md` records the vector file's URL,
commit, checksum, and license, and one divergence: the vectors predate the
NIP-44 revision that allows plaintexts of 65536 bytes or more with a 6-byte
length prefix, so the two entries `65536` and `100000` in
`invalid.encrypt_msg_lengths` are asserted as valid, which is what the
pinned `nips/official/44.md` says and what this crate does up to its
256 KiB client bound.

The vector test also asserts the boundaries the issue names: plaintext
length 0 refused; 1 producing the 132-character minimum payload; 65535 as
the last 2-byte-prefix length; 65536 as the first extended-prefix length,
and a noncanonical extended prefix below it refused; the client maximum
accepted and the client maximum plus one refused; and, on the decode side,
a 128-character payload refused before any key material is used, 98
decoded bytes refused as too short, and 99 decoded bytes reaching the MAC
check.

## Property tests

`crates/nostr/tests/properties.rs` runs 256 bounded cases per property by
default (`PROPTEST_CASES` raises it; 3000 cases ran clean on 2026-09-20):

- NIP-01 event JSON round trips, the seven-field wire shape checked
  through `serde_json::Value`, and the canonical id preimage checked as the
  six-element array with no insignificant whitespace.
- Filter JSON round trips, unknown top-level fields refused, NIP-CW
  channel-window fields accepted and dropped, and `Filter::matches`
  compared against a naive matcher written separately in the test.
- NIP-19 bech32 round trips for arbitrary prefixes and payloads, refusal
  past 90 characters, detection of every single-character substitution in
  the data part (the BCH guarantee), and `npub`/`nsec` round trips on real
  keys.
- NIP-44 framing: the payload decomposes into version, nonce, ciphertext,
  and MAC with the sizes the spec gives; the MAC and ciphertext recompute
  from the RustCrypto-verified primitives; every single-byte corruption of
  a payload or of the conversation key is refused; padding and base64
  round trip; arbitrary text never panics the decoder.

Failure seeds persist under `crates/nostr/tests/fixtures/regressions/`.
No property test found a defect in the crate; the directory holds only
`.gitkeep`. Two early failures were defects in the test strategies (a
bech32 payload that legitimately exceeds 90 characters, and a wrong bound
for the padding slack) and were fixed in the tests, not kept as fixtures.

## Miri

Nightly installs on this machine, so Miri ran on the pure primitive tests
on 2026-09-20 with `rustc 1.100.0-nightly (feaadeeac 2026-09-19)` and
`miri 0.1.0 (feaadeeaca 2026-09-19)`:

```sh
PROPTEST_CASES=4 MIRIFLAGS=-Zmiri-disable-isolation \
  cargo +nightly miri test -p nostr --test nip44_differential
# 12 passed; 0 failed

MIRIFLAGS=-Zmiri-disable-isolation \
  cargo +nightly miri test -p nostr --test nip44_vectors -- \
  --skip conversation_key --skip long_messages \
  --skip invalid_plaintext_lengths --skip plaintext_length_boundaries \
  --skip valid_encrypt_decrypt
# 5 passed; 0 failed; 6 filtered out
```

Exclusions, stated rather than implied:

- `cargo +nightly miri test -p nostr` as a whole was not run. The crate's
  unit tests, the conversation-key vectors, and `valid_encrypt_decrypt`
  (which parses the vector secret keys) call `secp256k1`, which is C behind
  FFI. Miri stops at the extern static
  `rustsecp256k1_v0_11_context_no_precomp`. Those tests run under the
  ordinary `cargo test -p nostr` gate only.
- `PROPTEST_CASES=4` under Miri: the interpreter is two to three orders of
  magnitude slower than native, so the random cases are a smoke run. The
  boundary-size tests in the same file are exhaustive and ran in full.
- The long-message vectors, `invalid_plaintext_lengths`, and
  `plaintext_length_boundaries` encrypt 100 KB to 256 KiB plaintexts and
  did not finish within a 50-minute Miri budget; they run natively.
- No sanitizer run. The crate forbids `unsafe`, and its only unsafe
  dependency of interest is `secp256k1`, which is outside this issue's
  scope.

## How to rerun

```sh
export CARGO_TARGET_DIR=$HOME/target-nostr
cargo fmt --all --check
cargo clippy -p nostr --all-targets -- -D warnings
cargo test -p nostr
PROPTEST_CASES=3000 cargo test -p nostr --test properties --test nip44_differential
```
