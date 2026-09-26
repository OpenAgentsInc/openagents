---
id: numerics.lapack-real-eigenpairs
version: 1
kind: method
title: Select dominant eigenpairs from real LAPACK output
summary: >-
  For a real nonsymmetric matrix, use a real LAPACK eigensolver to avoid
  unnecessary full complex eigensystem conversion; reconstruct a selected
  conjugate-pair eigenvector using LAPACK's adjacent-column convention.
tags: [linear-algebra, eigenvalues, lapack, performance]
applies_when: >-
  A real-valued square matrix needs one eigenpair selected by eigenvalue
  magnitude, and a low-level real LAPACK eigensolver is available.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - largest-eigenval
  cites:
    - Anderson et al., LAPACK Users' Guide, 3rd ed., §2.4 and routine DGEEV
    - SciPy documentation, scipy.linalg.lapack.dgeev
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

A real nonsymmetric eigensolver such as LAPACK `DGEEV` returns eigenvalues as separate real and imaginary arrays. A real eigenvalue's right eigenvector is one real column. For a nonreal conjugate pair, the two eigenvalues are adjacent, with the positive-imaginary member first; the corresponding real right-eigenvector columns encode the complex eigenvector as `v[:, j] + 1j*v[:, j+1]`. Select the index by `hypot(wr, wi)` rather than squaring components, which can overflow unnecessarily. Request right eigenvectors but omit left eigenvectors if they are not needed, check the LAPACK `info` status, and remember that the returned representation assumes the real-matrix interface and its documented conjugate-pair layout.

Sources: Anderson et al., *LAPACK Users' Guide*, 3rd ed., §2.4 and routine `DGEEV`; SciPy documentation, `scipy.linalg.lapack.dgeev`.

## How to check

For the returned pair `(lam, v)`, verify a scale-relative residual such as `np.linalg.norm(A @ v - lam * v) <= tol * (np.linalg.norm(A) * np.linalg.norm(v) + abs(lam) * np.linalg.norm(v))`. Also compare `abs(lam)` to the maximum magnitude from a trusted full eigensolver on small test matrices, including one with a dominant nonreal conjugate pair and one whose dominant eigenvalue is real.
