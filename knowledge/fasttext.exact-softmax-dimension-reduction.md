---
id: fasttext.exact-softmax-dimension-reduction
version: 1
kind: method
title: Reduce fastText softmax embeddings exactly using logit differences
summary: >-
  A supervised fastText softmax classifier can often be represented with fewer
  embedding dimensions without changing probabilities by projecting onto
  differences between class output vectors. This is useful when model size is
  constrained but quantization causes unacceptable accuracy loss.
tags: [fasttext, model-compression, softmax, linear-algebra]
applies_when: >-
  A trained fastText supervised model uses softmax loss, has a small number of
  output classes, and its input-embedding matrix dominates serialized model
  size.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - train-fasttext
  cites:
    - Joulin et al., Bag of Tricks for Efficient Text Classification, §2
    - Goodfellow, Bengio, and Courville, Deep Learning, §6.2.2
    - Strang, Linear Algebra and Its Applications, section on rank and column spaces
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

For a hidden representation \(h(x)\), fastText computes class logits \(z_c=h(x)^T o_c+b_c\), where \(o_c\) and \(b_c\) are the output weight and bias for class \(c\). Softmax probabilities are unchanged if the same scalar is subtracted from every class logit. Choose a reference class \(r\): the probabilities depend only on \(h(x)^T(o_c-o_r)+(b_c-b_r)\). Therefore project each input vector onto the span of the output-vector differences, whose dimension is at most \(C-1\) for \(C\) classes, and express class output differences in that projected basis. Keep the bias differences intact; the reference class can have zero output vector and zero relative bias. This is an exact algebraic reduction in real arithmetic, unlike lossy quantization.

For row-oriented matrices `A = model.get_input_matrix()` (vocabulary/subword rows by embedding dimension) and `O = model.get_output_matrix()` (classes by embedding dimension), form `D = (O[:-1] - O[-1]).T`, then `A_reduced = A @ D`. A corresponding output matrix has an identity for the first `C-1` classes and a zero row for the reference class. Preserve or explicitly adjust the class biases to their differences from the reference. Install matrices through the fastText binding's matrix setter only if the library version and matrix layout are verified; then serialize and reload using the ordinary fastText API. fastText uses a separate bias in supported versions, so confirm how the binding stores and exposes it before applying this construction.

Sources: Joulin et al., “Bag of Tricks for Efficient Text Classification,” §2 (fastText supervised model); Goodfellow, Bengio, and Courville, *Deep Learning*, §6.2.2 (softmax invariance to adding a common logit offset); Strang, *Linear Algebra and Its Applications*, section on column spaces and rank factorization.
