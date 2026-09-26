---
id: edge-case.c-prefix-numeric-conversion
version: 1
kind: edge-case
title: Match C numeric prefix conversions rather than strict parsing
summary: >-
  When porting code that uses C conversion routines, do not assume the source
  rejects trailing junk or handles invalid text like a strict
  high-level-language parser. Reproduce the specific conversion routine's
  prefix, default, range, and locale semantics.
tags: [c, parsing, compatibility]
applies_when: >-
  A compatibility implementation replaces C code that parses numeric strings
  with routines such as strtod or strtol, or uses scanf conversions.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - risk-scorer-replay
  cites:
    - ISO/IEC 9899:2018, §7.22.1.3, “The strtod, strtof, and strtold functions.”
    - ISO/IEC 9899:2018, §7.22.1.4, “The strtol, strtoll, strtoul, and strtoull functions.”
    - ISO/IEC 9899:2018, §7.21.6.2, “The fscanf function.”
evidence: []
---

## Details

C's `strtod` and `strtol` parse an initial portion of the input, stopping at the first character that cannot belong to the conversion. Thus a string with a valid numeric prefix followed by other characters may convert successfully; a strict `float()` or `int()` call may reject it instead. If no conversion is performed, the functions return zero, with additional state available through the end pointer and range-error mechanisms. `strtol` also depends on its requested base. `scanf` has its own format-driven conversion rules, so do not treat it as interchangeable with either function.

For compatibility, identify the exact source routine and options, then reproduce its accepted prefixes, whitespace/sign behavior, overflow handling, and relevant locale assumptions. Avoid a single broad regular expression unless it has been checked against those semantics. The definitions are in ISO/IEC 9899:2018, §7.22.1.3 (`strtod` family), §7.22.1.4 (`strtol` family), and §7.21.6.2 (`fscanf` family).

## How to check

On a POSIX-like development system, a small `ctypes` probe makes prefix behavior visible:

```python
import ctypes

libc = ctypes.CDLL(None)
libc.strtod.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_char_p)]
libc.strtod.restype = ctypes.c_double
end = ctypes.c_char_p()
value = libc.strtod(b"12.5tail", ctypes.byref(end))
assert value == 12.5
assert end.value == b"tail"
```

Build a compatibility test matrix for valid prefixes with suffixes, leading whitespace, signs, exponent forms, invalid prefixes, and range limits. Compare the replacement's result and consumed portion with the source routine, not merely whether both accept a clean numeric string.
