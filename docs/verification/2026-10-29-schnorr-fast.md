# Public bounded Schnorr verification

`schnorr_fast_verify_vartime` performs the same strict byte admission, public
key lift and tagged challenge as the original BIP-340 verifier. Its public
joint multiplication uses width-5 signed digits and the existing fused field
kernel; eight fixed generator odd multiples are checked against the existing
group implementation. `nostr_fast_verify_vartime` additionally requires an
exactly 32-byte event ID. Inputs are borrowed and must remain valid and stable
for the call. These are public-data variable-time procedures, not secret-key
operations or an authorization decision.

On source SHA-256
`d27a4b855d5d350888d46d0a2ef68f88c6d30e48ad54e1e201f46c3e432f8f4c`,
`coax0 0.22.0` SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`
passed 159 checks. The official vector-zero complete byte-view verification
used 18,328,270 of the existing 20,000,000 per-check steps. Separate full
checks cover changed message, changed signature, and noncanonical r, s and
public key refusals, and the group and affine exceptional cases.

The full authority mutation gate on this source and compiler killed all
2,535 mutants, with no survivors. It executed 95,091 full checks and
skipped none; the valid-vector check retained the default step budget.

The [differential record](2026-10-29-schnorr-fast-differential.json) covers 790
cases per checked/release native and Wasm build, including the 19 pinned
upstream vectors, seeded valid signatures, mutated messages, keys and
signatures, r/s boundaries, exact and over-limit messages, and read-only
input equality. The over-limit message reuses the maximum-length signature,
so it is not an otherwise valid oversized signature; synthetic over-limit
counts separately exercise admission. Both fast and original verifiers are compared to pinned
libsecp and an independent Python affine reference. The native fixture also
exercises malformed counts and an actual read-only message/key alias. The
tested fixture driver SHA-256 is
`ff9f2c189368bfa334cb85ea028c9b136c648ecd070ec8ae5ae5b57b8ef490dc`.
The committed driver SHA-256 is
`76acecf603bbe9eb7e5d021e22a785081ab923d743e0dc9b329fb543c433fc61`;
the only difference removes whitespace from an empty line. Its C and JavaScript test shims are
`cac7828cf4d120e927c74578f74f3d24b2bd1f4de6871f42d25d2ee54a26b672` and
`ec09d6a50039cca19db2c87b312bf0bc66f3e98d3415b25f9dc4a234e87ecdc1`.

Independent source review found no concrete blocker in finite canonical point
closure, exceptional additions, signed-digit selection, byte admission or the
final even-y/x comparison. It independently decoded all eight generator table
points against affine odd multiples. A separate fixture review checked the
comparison and target scope. These conclusions rely on the reviewed
field and recoding premises and stable borrowed views. Finite test coverage
does not establish target/JIT timing, secure erasure, physical-device behavior
or production-secret approval. The caller must choose the fast verifier
explicitly; this change does not itself modify a signer.
