# NIP-GS git-signature projection verification

`nostr.block.gs.v1alpha1.GitSignature` is an untrusted representation of one
NIP-GS v1 detached signature for the git signing/verifying program. It is not
a signature check: no `lift_x`, no BIP-340 verification of `sig` or of the
owner attestation, no payload hash, conditions evaluation or trust level. No
signing program uses it yet.

The descriptor profile `proto/descriptors/block-git-signature.pb` (SHA-256
`4ab9fce75886b64693d7ce4d9f87104762209604be60cad23ceef6c5dc8daf3e`) holds
`options.proto`, `nostr/block/oa/v1alpha1/owner.proto` and
`nostr/block/gs/v1alpha1/signature.proto`; the `nostr` module excludes the GS
schema, so `nostr.pb` and every earlier descriptor are byte-identical
(`scripts/proto.sh check`). The manifest pins NIP-GS to Block commit
`24a370606044c0c7be787cc2fd98ec0bfe1d606b`
(`d1cc73bbfc7fc8ef54afd509c868500938a43857726deafcb48e181e8abbc894`).

`coax/lib/nostr/adapters/gs-projection.coax` (`#authority`) rebuilds the
canonical envelope from the projection and compares it with the retained
armor byte for byte, allowing only the one extra LF; that covers NIP-GS
verification steps 1 to 4. 37 protoc fixtures
(`generate_gs_signature_fixtures.py`, which asserts it rebuilds the NIP's own
JSON and base64) cover both NIP vectors, `t` at 0 and 4294967295, decoded JSON
at 2048 and 2049 bytes, armor spellings (no final LF, two extra, CRLF, a stray
byte, wrapped base64, PGP markers, non-canonical padding), JSON spellings
(space, key order, uppercase hex, `1.7e9`, unknown and duplicate keys), every
projected field diverging, an owner present on one side only,
self-attestation and conditions JSON would escape.

- `coax test coax/lib/nostr/adapters/gs-projection.coax`: 8 checks pass.
- `coax mutate ... --root-only`: 214 mutants, 214 killed, none marked equivalent.
- `block_git_signature.py` (protoc 29.3): 48 cases through the generated
  reader, checked and release, native ASan/UBSan and WASI.

Compiler: the provenance-verified build with input fingerprint
`9f27926b00838a244d47ada6e9198e66ae3ced299b33bc036277517b34a9b982`.
The full-reference mutation run (generated reader included) is not done for
GS; it remains with the task.
