# att-platforms

NIP-ATT client verification beyond Google Confidential Space
([NIP-ATT](../../nips/openagents/NIP-ATT.md#client-verification)):

- `tdx`: raw Intel TDX DCAP quotes v4. PCK chain to Intel's SGX root, the
  PCK CRL, the QE report signature and attestation-key binding, the quote
  signature, debug refusal, and signed, current TCB Info whose matching
  level is `UpToDate`.
- `snp`: raw AMD SEV-SNP reports. VCEK → ASK → ARK with the ARK pinned,
  AMD's CRL, the VCEK's chip ID and TCB extensions against the report,
  debug-policy refusal, and the P-384 report signature.
- `nras`: NVIDIA Remote Attestation Service answers. ES384 tokens whose JWKS
  `x5c` chain ends at a pinned NVIDIA certificate, issuer and freshness,
  the binding as `eat_nonce`, per-GPU digests, measurement, secure boot,
  debug, and the release's GPU models.
- `verify`: parses the release, head and endpoint with `oa-att`, dispatches
  by evidence kind, matches the release's registers, finds the binding in
  REPORTDATA, and requires GPU evidence exactly when the release names a
  GPU. Unknown evidence kinds refuse.

`roots/` holds the real vendor anchors, pinned by SHA-256 in `lib.rs`:
Intel's SGX Provisioning Certification Root CA
(certificates.trustedservices.intel.com), AMD's Milan and Genoa ARK/ASK
(kdsintf.amd.com `/vcek/v1/<product>/cert_chain`), and NVIDIA's NRAS GPU
intermediate 004 (the end of the `x5c` chains in
nras.attestation.nvidia.com `/.well-known/jwks.json`; NVIDIA does not
distribute the root above it).

`vectors/` holds synthetic chains, keys, TCB Info and CRLs from
`vectors/generate.py` (`python3 generate.py`, needs `cryptography`). The
tests sign quotes, reports and tokens with those keys. Nothing here is
evidence from real hardware, and a passing test is not a hardware
qualification; that is recorded in `NEEDS_OWNER.md`.

Collateral fetching (Intel PCS, AMD KDS, NRAS) is the caller's job; this
crate takes the bytes and checks them.

```sh
cargo test -p att-platforms
```
