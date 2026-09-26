---
id: cryptanalysis.feal-exact-linear-round-key-recovery
version: 1
kind: method
title: Recover FEAL round keys with exact F-function linear relations
summary: >-
  Exploit exact parity identities in FEAL's byte-addition-and-rotation F
  function to constrain outer round keys, then invert inner rounds and verify
  candidate keys against known pairs. Applies when a FEAL implementation's
  precise byte layout and round convention are available.
tags: [cryptanalysis, feal, known-plaintext, linear-cryptanalysis]
applies_when: >-
  Attacking a FEAL-style Feistel cipher where F is built from byte additions
  and fixed rotations, especially when the exact source implementation can be
  inspected.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - feal-linear-cryptanalysis
  cites:
    - Bruce Schneier, Applied Cryptography, 2nd ed., FEAL section
    - Mitsuru Matsui, Linear Cryptanalysis Method for DES Cipher, EUROCRYPT '93, section 2
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Do not assume a probabilistic linear approximation is needed: for the FEAL F function formed from the specified byte additions and two-bit rotations, low-bit parity relationships are exact because the least significant bit of an addition is the XOR of its operand low bits, independent of carries. Rotation maps these bit relations into selected output bits. Derive masks and constants from the actual F implementation and byte order rather than transplanting masks from a different FEAL variant.

Use the identities as equations over GF(2) involving known plaintext/ciphertext and guessed outer round keys. Enumerate feasible outer-key candidates, invert the known Feistel rounds to obtain intermediate states, and apply the relations to narrow candidates. Recover remaining subkeys from the resulting states, then check the complete cipher in both directions on every known pair. If subkeys are generated from shorter seeds, confirm the expansion mapping and check whether each expanded key has unique seed preimages.

This is an exact-identity attack, distinct from classical linear cryptanalysis that ranks keys by a biased approximation over many samples. Cipher-specific state naming, whitening, byte packing, and final-round swap conventions must be derived from the implementation.

Sources: Bruce Schneier, *Applied Cryptography*, 2nd ed., section on FEAL; Mitsuru Matsui, “Linear Cryptanalysis Method for DES Cipher,” EUROCRYPT ’93, §2 (background on parity approximations; the FEAL identities here must be derived from the target F definition).
