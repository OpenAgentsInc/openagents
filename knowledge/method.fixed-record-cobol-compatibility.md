---
id: method.fixed-record-cobol-compatibility
version: 1
kind: method
title: Reproduce fixed-width sequential COBOL file behavior in another language
summary: >-
  When porting a sequential fixed-record COBOL program, preserve byte-level
  record layouts, record traversal, update ordering, and runtime failure
  behavior rather than translating only the apparent business logic.
tags: [cobol, fixed-width, file-io, compatibility]
applies_when: >-
  A program reads and rewrites fixed-length records through sequential COBOL
  files and an implementation in another language must match its observable
  behavior.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - cobol-modernization
  cites:
    - GnuCOBOL Project, GnuCOBOL Programmer's Guide, sections on file control and file input-output verbs
    - ISO/IEC 1989:2023, Programming languages — COBOL, clauses on data description and arithmetic statements
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details
Model records as bytes with explicit widths and offsets. Sequential reads normally consume fixed-size records; a trailing short record can behave differently from a clean EOF, so probe the target runtime and reproduce its record buffer and file-status behavior rather than silently padding or discarding data. For sequential `REWRITE`, preserve the distinction between the record most recently read and arbitrary writes: a second rewrite without another successful read may fail. Keep the operation order intact, since earlier record updates may already be committed when a later operation fails.

Treat numeric display fields as encoded data, not ordinary text. Determine how the COBOL runtime interprets invalid, space-filled, signed, or out-of-range digit bytes, and how arithmetic results are formatted or truncated into the destination field. Likewise preserve file open modes: appending to an existing file and opening an existing file for update are not interchangeable when the file is absent. Make mutations and failure timing observable in the same order as the reference implementation.

For faithful ports, compare both final file bytes and process-level behavior (status, output, and errors) against the target runtime over valid and adversarial fixtures. GnuCOBOL documentation describes file organization, record handling, and `READ`/`REWRITE` semantics in the GnuCOBOL Programmer's Guide, sections on file control and file input-output verbs; COBOL's fixed-format display data and arithmetic semantics are specified by ISO/IEC 1989, clauses on data description and arithmetic statements.
