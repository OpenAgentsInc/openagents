---
id: edge-case.split-empty-output
version: 1
kind: edge-case
title: A newline split cannot represent an empty result list
summary: >-
  When an interface decodes records with `text.split("\n")`, an empty string
  becomes a one-element list containing an empty record, not an empty list.
  Detect this representation mismatch before attempting to solve it in a
  producer that cannot change the decoder.
tags: [serialization, python, edge-case, interface-contract]
applies_when: >-
  A producer emits zero records but a fixed consumer splits its text on a
  literal newline and interprets every field as a record.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - regex-chess
  cites:
    - Python Standard Library documentation, Built-in Types — str.split and str.splitlines
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Python's `str.split(sep)` returns `['']` for an empty string, whereas `str.splitlines()` returns `[]`. Thus a newline-delimited protocol with no records needs an explicit empty-result convention or a decoder that special-cases empty input. If the decoder is immutable and there is no legal sentinel, the producer cannot express the empty collection faithfully; document the limitation rather than treating the blank field as a valid record.

This also applies to trailing separators: `split("\n")` preserves the final empty field, so an output ending in a newline may create a spurious record. Define whether empty records and terminal newlines are legal in the protocol.

Cite: Python Standard Library documentation, *Built-in Types*, `str.split` and `str.splitlines`.
