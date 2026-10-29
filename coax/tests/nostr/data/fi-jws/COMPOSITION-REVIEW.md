# FI signature composition: partial evidence and source review

This is partial work on #bd06. `fi_jws_document_signature` parses the
complete caller-supplied JWKS and exact protected `kid`, then checks the
original compact signing bytes against the selected RSA key. It establishes
signature evidence only: the caller-supplied document is not an authenticated
key source, and no result grants FI identity authority.

The latest Coax test run passed 92/92 checks. `fi_jws_jwks.py` exercised 13 cases per
checked/release native and wasm32-wasip1 build, including position-independent
kid lookup, absent/duplicate kids, private and unsupported key fields, altered
moduli, payloads, signatures and algorithms. The same cases ran in native
ASan/UBSan checked and release executables. OpenSSL independently verified the
original signature and rejected the altered payload and altered signature. These are finite checks. The first full composition mutation killed 2607/2616; nine survivors were not waived. Additional boundary/refusal checks were added; the full default-budget rerun killed 2616/2616 with no waivers.

A separate agent (`fixedbase_rescue`) independently reviewed the source without
rerunning tests. The review covered compact bounds and original signing span,
canonical base64url/uint, closed protected header, whole-JWKS exact kid
selection, RSA restrictions and ranges, Montgomery arithmetic, and exact
PKCS#1 v1.5 SHA-256 padding. No concrete blocker was found in that inspected
partial snapshot. The normal base64url decoder needs disjoint input/output;
the composition uses local disjoint buffers. This is bounded source review,
not JOSE conformance, production authorization, or a target-code audit.

Reviewed source SHA-256 (all paths under `coax/lib/`):

- `crypto/rsa_verify.coax`: `9b4af0562d5b4e1364b2af316c69a7fb280820bfce2f662ea0c657673ba45c25`
- `nostr/block/fi-base64url.coax`: `be749bd53ce734f6170847a791b3d0894b2e584e72ca4d7065b6a336f30a3d3b`
- `nostr/block/fi-compact.coax`: `33cba54530fec124bf5187e931a02ff3c5f1a0a29f1d7f5b9bf01a77ee4f4427`
- `nostr/block/fi-protected.coax`: `b5230480ce459eba2f0720c6bb59a782d2ee0c12ed1bbecf2a06bfe45b9c6970`
- `nostr/block/fi-jwks-parse.coax`: `cf8f4409579654820ef5b43745a794bcd0da37a9054220d52f1db82f29ed1973`
- `nostr/block/fi-jws-signature.coax`: `66102a4d7aec8ba3d0330d59e925647f1ca84fbe70ddec0c9b7f34a1361f5de0`
- `nostr/block/fi-jws-jwks.coax`: `a0970947509e7cffb3ecb76cc7c388c9c7970ac8a33df31da74a0fdf9336336a` (reviewed version; tests were later added, current source SHA-256 `c24889248b2f81dcf4090939015b16baf2718789d4d7169478001bdfae5cee16`; implementation unchanged)

Authenticated DNS/TLS fetch and installed snapshot identity, claim projection,
policy digest, fresh verified NIP-42 possession, rotation/session lifecycle and
per-target integration remain open. In particular, a configured access-token
marker may equal `iss`; claim projection must use the same decoded member for
both roles and select the policy using the exact issuer before dependencies.
No fixture observation or caller-supplied DTO is authority.

The reviewer later compared the current `c248...` source against a
reconstructed copy whose SHA-256 exactly matched the reviewed `a097...`
source. The delta contains only `#test` boundary and refusal assertions; no
production procedure changed. This narrow delta review adds no provider or
production approval.
