---
id: method.adaptive-arithmetic-encoder-from-decoder
version: 1
kind: method
title: Construct an encoder by reversing an adaptive arithmetic decoder
summary: >-
  When only a small adaptive arithmetic decoder is available, derive the
  matching encoder from its interval updates, bit contexts, and symbol
  grammar, then validate with round trips. This applies to custom compressed
  formats whose decoder is inspectable.
tags: [compression, arithmetic-coding, reverse-engineering]
applies_when: >-
  A decoder uses adaptive probability counts, renormalization, and
  variable-length symbol or back-reference coding, but a compatible encoder is
  missing.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - write-compressor
  cites:
    - Ian H. Witten, Radford M. Neal, and John G. Cleary, “Arithmetic Coding for Data Compression,” Communications of the ACM 30(6), 1987, sections 2–4
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Treat the decoder as the specification. First record, in execution order, how it initializes the coding interval, rescales it, computes each context's split, chooses a subinterval, and updates that context's counts. The encoder must mirror those operations exactly; a different but mathematically similar arithmetic-coding convention can produce incompatible bytes.

Separate the format into two layers:

1. Convert the source into the exact sequence of logical symbols/tokens accepted by the decoder, including end/count fields, literal sign or magnitude conventions, and back-reference distance/length conventions.
2. Map each token into the decoder's context-indexed bits, then arithmetic-encode that bit stream using matching interval and renormalization rules.

For a byte-oriented coder that renormalizes by a radix when the interval is below a threshold, track the interval width and emitted prefix/carry state explicitly. Model-based counts must be updated once per decoded bit, using the same context and timing as the decoder. Keep the implementation small and instrumentable; dump the token bits and interval transitions when debugging.

Validate in stages: test integer/symbol encodings, then short token streams, then full data. Decode the produced stream using the actual target decoder and compare output bytes, not text-normalized content. Check any size limit separately from correctness.

Sources: Ian H. Witten, Radford M. Neal, and John G. Cleary, “Arithmetic Coding for Data Compression,” *Communications of the ACM* 30(6), 1987, sections 2–4 (arithmetic interval coding and adaptive models).
