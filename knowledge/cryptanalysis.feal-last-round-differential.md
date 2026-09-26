---
id: cryptanalysis.feal-last-round-differential
version: 1
kind: method
title: Recover a final FEAL round key with a probability-one differential
summary: >-
  Exploit FEAL's round-function differential to test candidate last-round
  subkeys through an encryption oracle, without recovering earlier round keys.
  Apply when the cipher exposes 64-bit chosen-plaintext encryption and a small
  key candidate domain.
tags: [feal, differential-cryptanalysis, chosen-plaintext, oracle]
applies_when: >-
  A FEAL-style four-round Feistel implementation has a bytewise round function
  with the specified differential, and the goal is to identify a final-round
  subkey from oracle access.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - feal-differential-cryptanalysis
  cites:
    - Biham and Shamir, Differential Cryptanalysis of the Data Encryption Standard, Sections 2–3
    - "FEAL cipher specification: round function and Feistel round equations (cite the implementation's authoritative specification when applying)"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

For the common FEAL byte function built from 2-bit left rotations of byte sums, the 32-bit input difference `0x80800000` produces the output difference `0x02000000` with probability one. Verify this against the implementation rather than assuming byte order or bit placement: implementations often represent the four bytes in a different order.

Choose plaintext pairs whose two 32-bit halves are each XORed with `0x80800000`. Equal whitening key XORs cancel. The pre-round Feistel mixing then gives the internal input difference `(0x80800000, 0)`. Propagate the difference round by round using the Feistel equations; after two rounds the relevant difference entering the final round function is `0x02000000`.

Undo the final ciphertext mixing to expose the corresponding final-round right inputs. For each candidate final subkey `k`, evaluate `F(x xor k) xor F(x' xor k)` and retain candidates matching the predicted output difference. Additional independently chosen pairs can disambiguate survivors. The earlier round keys need not be guessed because their effects cancel in this differential relation. Maintain explicit conventions for which ciphertext half is the Feistel left/right state and for whether the reported subkey is the whitening key or the last round key.

The attack's validity depends on the exact round structure and differential; it does not generalize merely from the cipher being called FEAL. Differential cryptanalysis and chosen-plaintext attack conventions are described in Biham and Shamir, *Differential Cryptanalysis of the Data Encryption Standard*, Sections 2–3; the FEAL-specific function and round equations should be checked against the relevant cipher specification or implementation.

## How to check

First exhaust or randomly sample inputs to confirm `F(x) xor F(x xor alpha) == beta`; then compare the recovered subkey with the implementation's actual final-round key under many independently generated keys. For example:

```python
for x in samples:
    assert round_function(x) ^ round_function(x ^ 0x80800000) == 0x02000000

candidate = attack(encrypt_oracle)
assert candidate == actual_last_round_key
```

Also test boundary-valued subkeys and confirm each additional oracle pair only removes false candidates, never the true one.
