---
id: numerics.cosine-zero-vector
version: 1
kind: edge-case
title: Cosine distance of a zero vector
summary: >-
  Cosine similarity divides by both vectors' norms, so an all-zero vector
  gives 0/0 and nan. Guard the norm, clip the similarity to [-1, 1], and
  remember cosine distance (1 - similarity) runs from 0 to 2.
tags: [numerics, cosine, cosine-distance, cosine-similarity, embeddings, nan, division-by-zero, normalization]
applies_when: >-
  Code computes cosine similarity or distance between vectors or embeddings,
  normalizes vectors, or compares a vector with a centroid.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Manning, Raghavan, and Schütze, Introduction to Information Retrieval (Cambridge, 2008), section 6.3.1"
    - "SciPy documentation, scipy.spatial.distance.cosine"
evidence: []
---

## Details

    similarity(a, b) = a · b / (||a|| ||b||)
    distance(a, b)   = 1 - similarity(a, b)

Edge cases:

- **A zero vector.** The norm is 0 and the ratio is undefined; NumPy
  returns `nan` with a warning, and library functions differ in what they
  do. A `nan` then poisons every mean, max, and comparison after it
  (`nan > threshold` is false, so an alert never fires). Pick a stated
  convention: similarity 0 (distance 1) for a zero vector, or skip it, and
  implement it explicitly: `denominator = max(||a|| ||b||, eps)`.
- **Rounding.** Floating-point error can give a similarity of `1.0000001`;
  clip to `[-1, 1]` before `arccos` or before a check that assumes the range.
- **Range.** Cosine distance is in [0, 2], not [0, 1]; opposite vectors give 2.
- **Epsilon placement.** Adding epsilon to each norm separately
  (`||a|| + eps`) changes every result slightly; guarding the product with
  `max` changes only the degenerate case.
- **Centroids.** The mean of unit vectors isn't a unit vector; normalize the
  centroid before comparing, or use the formula above, which divides by its
  norm.
- **Row-wise norms.** For a matrix of embeddings, take norms along axis 1
  with `keepdims=True`, not the norm of the whole matrix.

## How to check

Identical vectors give distance 0, opposite vectors 2, orthogonal vectors 1,
and a zero vector gives the chosen finite value, never `nan`.
