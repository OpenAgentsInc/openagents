# 93. Test vectors and their provenance

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A test vector is only useful if its origin is known. This chapter sets out how
the stack pins, reads and checks every vector file.

Part XI, Interop and verification. Result: Every vector file pinned by source
URL, revision, license, retrieval date and SHA-256, with derived adapters
labelled and their input hashes recorded.

## What this chapter teaches

- The inventory: NIST SHA-256, RFC 4231/5869/7914/8439, BIP-173, BIP-340,
  NIP-44, Wycheproof, JSONTestSuite.
- Upstream bytes are never reformatted; derived files name their inputs.
- Offline integrity checks and the explicit `--fetch` comparison.

## What exists today

- `coax/tests/vectors/` with `check_vectors.py`, `test_integrity.py`,
  `manifest.json` and `SOURCES.md`.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)
- [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Done means

- `python3 coax/tests/vectors/check_vectors.py` and `test_integrity.py` pass.
- Every chapter's "done means" names the vector file it uses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
