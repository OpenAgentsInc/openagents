---
id: method.numpy-c-extension-safe-buffer-access
version: 1
kind: method
title: Safe NumPy C-extension buffer access with normalization
summary: >-
  Validate ndarray dimensionality and shape before acquiring typed buffers;
  convert inputs to aligned, native-dtype, contiguous arrays through NumPy’s
  conversion API before raw pointer access. This avoids incorrect assumptions
  about arbitrary strides, byte order, dtype, and alignment.
tags: [numpy, c-extension, buffer, array-layout]
applies_when: >-
  A C extension receives NumPy arrays and loops over their elements using raw
  data pointers.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - portfolio-optimization
  cites:
    - "NumPy Developers, NumPy C API: Array API, “Converting an arbitrary object to a NumPy array” and “Array flags”"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Check rank and mutually dependent dimensions before computing from an input. Then use `PyArray_FROM_OTF` with the required dtype and `NPY_ARRAY_IN_ARRAY` to obtain an input array suitable for ordinary C-order pointer indexing. This requests a read-only, aligned, contiguous representation and converts dtype or byte order when needed; it may return the original array with an incremented reference or a temporary copy. Check for conversion failure, release any earlier acquired arrays on every error path, and `Py_DECREF` all acquired arrays after computation.

Do not infer contiguous layout from ndarray type or dtype alone: sliced, reversed, Fortran-order, non-native-endian, or unaligned arrays can otherwise make flat pointer indexing wrong or unsafe. If avoiding copies is essential, instead honor strides explicitly and separately validate alignment and dtype.

Source: NumPy Developers, *NumPy C API: Array API*, “Converting an arbitrary object to a NumPy array” (`PyArray_FROM_OTF`) and “Array flags” (`NPY_ARRAY_IN_ARRAY`).

## How to check

Build the extension, then compare its result with a trusted reference using inputs representing different layouts and conversion cases (for example, transposed/Fortran-order, reversed slices, non-native byte order, lower-precision dtype, and unaligned buffers). Also verify wrong ranks and incompatible shapes raise the documented exception, and run a memory checker if available.
