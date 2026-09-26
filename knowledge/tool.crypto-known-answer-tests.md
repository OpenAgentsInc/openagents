---
id: tool.crypto-known-answer-tests
version: 1
kind: tool
title: Check cryptographic code against published known-answer test vectors
summary: >-
  A cipher, hash, MAC, or signature implementation that round-trips with
  itself can still be wrong. Test it against the known-answer vectors in the
  defining standard or RFC and against a trusted library, paying attention to
  byte order, bit versus byte lengths, padding, nonce layout, and encodings.
tags: [cryptography, test-vectors, kat, rfc, nist, interoperability]
applies_when: >-
  Implementing, porting, fixing, or formally specifying a cryptographic
  primitive or protocol step, or code that must interoperate with a standard
  encoding of keys, ciphertexts, or signatures.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "NIST, Cryptographic Algorithm Validation Program (CAVP) test vectors; FIPS 180-4 and FIPS 197 example values"
    - "RFC 8439, ChaCha20 and Poly1305 for IETF Protocols (test vectors in section 2 and appendix A)"
    - "RFC 4231, Identifier and Test Vectors for HMAC-SHA-224, -256, -384, -512"
    - "RFC 8017, PKCS #1 v2.2, section 4 (I2OSP and OS2IP)"
    - "Python Software Foundation, hmac.compare_digest"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Encrypt-then-decrypt or sign-then-verify with your own code only shows the two
halves agree with each other. Interoperability and correctness come from
**known-answer tests** (KATs): fixed inputs with published outputs.

- Sources: the standard itself (FIPS 180-4 and 197 examples, NIST CAVP
  response files), RFC appendices (RFC 8439 for ChaCha20-Poly1305, RFC 4231
  for HMAC-SHA-2, RFC 7748 for X25519), and Wycheproof-style edge-case
  vectors for invalid inputs.
- Differential check: compare random inputs against a trusted implementation
  (`cryptography`, OpenSSL's `openssl dgst`, `openssl enc -K ... -iv ...
  -nopad`, or `hashlib`/`hmac`).

Where implementations usually go wrong:

- **Byte order and word size:** SHA-2 and most block ciphers are big-endian,
  ChaCha20, Poly1305, and Curve25519 are little-endian.
- **Lengths:** bits versus bytes in length fields and key sizes; counters that
  wrap at 32 bits.
- **Nonce and counter layout** (for example 96-bit nonce plus 32-bit counter
  in RFC 8439 ChaCha20).
- **Padding:** PKCS#7 on block boundaries adds a full block; unpadding must
  validate every padding byte.
- **Integer encodings:** fixed-width big-endian (`I2OSP`) with leading zeros
  kept; DER versus raw `r||s` signatures.
- **Comparisons of secrets:** use a constant-time comparison such as
  `hmac.compare_digest` for MACs and tags.

## How to check

Encode at least one KAT per primitive and mode as an automated test, include
a vector whose output has a leading zero byte and one with an empty message,
and make sure each test fails if you change one input byte. Only then add
self-round-trip and randomized differential tests.
