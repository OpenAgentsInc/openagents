# NIP-GS git-signature projection

`nostr.block.gs.v1alpha1.GitSignature` represents one NIP-GS v1 detached
signature: the exact armored bytes git stores in a commit or tag header, and
the envelope's `v`, `pk`, `sig`, `t` and optional `oa` (the NIP-OA
`OwnerAttestation` message, its conditions string kept exactly). It is the
profile for the git signing and verifying program (`gpg.x509.program`); no
such program is in this repository yet, and `nostr.pb` stays frozen (the
`block-git-signature` descriptor profile holds it).

The armor is authoritative. `coax/lib/nostr/adapters/gs-projection.coax`
rebuilds the canonical envelope from the projection (compact JSON in `v`,
`pk`, `sig`, `t`, `oa` order, lowercase hex, decimal `t`, standard padded
base64 on one line) and compares bytes, accepting only the single extra LF a
verifier must allow. That is NIP-GS verification steps 1 to 4: malformed or
wrapped armor, CRLF, extra bytes, whitespace, reordered, duplicate or unknown
keys, uppercase hex, other number spellings, non-canonical base64, more than
2048 decoded bytes, `v` other than 1 and `oa[0] == pk` all fail. Conditions
that JSON would escape, or that are not ASCII, are refused rather than
reinterpreted; valid NIP-OA conditions are ASCII clauses.

It does not run `lift_x`, verify `sig` over the payload hash, verify the owner
attestation, evaluate conditions, read the payload, write status lines or
decide trust. A consistent projection is not a valid signature. Fixtures come
from the NIP's own vectors (`generate_gs_signature_fixtures.py`, pinned protoc
29.3); `coax/tests/nostr/block_git_signature.py` runs them through protoc and
the generated Coax reader on native sanitizers and Wasm, checked and release.
