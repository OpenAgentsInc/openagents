---
id: cryptanalysis.penalize-key-complexity
version: 1
kind: slip
title: Penalize oversized keys when ranking cryptanalytic models
summary: >-
  A flexible cipher model can score well by fitting accidental
  language-frequency variation. Rank decryptions with a complexity penalty and
  validate the selected model on fresh synthetic cases, not just the
  development ciphertext.
tags: [cryptanalysis, model-selection, overfitting]
applies_when: >-
  A search compares cipher hypotheses with different numbers of tracks, key
  symbols, or other free parameters using plaintext likelihood.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - interleaved-vigenere
  cites:
    - Hirotugu Akaike, “A New Look at the Statistical Model Identification,” *IEEE Transactions on Automatic Control* 19(6), 1974, Section II.
    - Jorma Rissanen, “Modeling by Shortest Data Description,” *Automatica* 14(5), 1978, Section 1.
evidence: []
---

## Details

Maximizing plaintext likelihood alone favors models with many free key symbols: they can fit noise in letter frequencies even when their structural hypothesis is wrong. Add a penalty for key complexity when ranking candidates. For example, if a model has `q` independently chosen letters from a 26-symbol alphabet and the key is assigned a uniform prior, its log-prior contribution is `-q * log(26)`. A normalized ranking score can therefore be `log_likelihood / N - q * log(26) / N`, where `N` is the number of scored letters. This is a key-code-length penalty, not a universal replacement for language-model validation; use word or n-gram scoring on leading candidates and keep the candidate search bounded.

This is an instance of penalized model selection: Akaike’s information criterion balances fit and parameter count (Hirotugu Akaike, “A New Look at the Statistical Model Identification,” Section II); minimum-description-length methods express the tradeoff as the cost of describing the model and data (Jorma Rissanen, “Modeling by Shortest Data Description,” Section 1).
