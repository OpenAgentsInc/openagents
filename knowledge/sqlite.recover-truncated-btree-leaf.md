---
id: sqlite.recover-truncated-btree-leaf
version: 1
kind: method
title: Recover SQLite rows from surviving table-leaf pages
summary: >-
  When truncation or header damage leaves SQLite table b-tree pages but
  removes database metadata, parse intact leaf cells directly and validate
  page structure before exporting records.
tags: [sqlite, forensics, binary-format, recovery]
applies_when: >-
  The database cannot be opened normally, but a surviving byte region may
  contain a SQLite table b-tree leaf page with row payloads.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - sqlite-db-truncate
  cites:
    - SQLite Documentation, Database File Format, sections “B-tree Pages,” “Record Format,” and “Variable-Length Integers”
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details
SQLite database pages use a b-tree page header, cell-pointer array, and cells. A table leaf page has page type `0x0d`; its header gives the cell count and cell-content-area boundary, and its pointer array identifies cell starts. Each table-leaf cell contains payload length and rowid varints followed by a record payload. A record begins with a header-size varint and serial-type varints; serial types determine value encoding and byte length (including the special integer constants and IEEE-754 binary64 encoding). Decode SQLite varints carefully: the first eight bytes contribute seven bits each, while the ninth contributes all eight bits. Values in an `INTEGER PRIMARY KEY` alias slot may be represented by a NULL serial type because the rowid carries the value.

Do not assume a surviving page is page one or that the table schema can be inferred uniquely from one cell. Establish page size/layout and candidate page boundaries first; validate every pointer, varint, payload extent, and serial type against the buffer. Recover schema/column names and interpretation from surviving metadata or corroborating evidence where possible. An empty freeblock chain and zeroed unallocated region support the conclusion that no recoverable deleted cells remain in those areas, but do not prove that no deleted data exists elsewhere.

Source: SQLite Documentation, “Database File Format,” sections “B-tree Pages,” “Cell Payload Overflow Pages,” “Record Format,” and “Variable-Length Integers.”

## How to check
For each proposed cell, assert that its pointer lies within the page content area, that payload/header sizes stay inside the buffer, and that the record header consumes exactly its declared size. Check that decoded cell intervals do not overlap unexpectedly and that the number of successfully parsed cells matches the header count. After serialization, parse the JSON again and verify every row has the intended field types. Example varint decoder:

```python
def varint(buf, pos):
    value = 0
    for i in range(9):
        b = buf[pos + i]
        if i == 8:
            return (value << 8) | b, pos + 9
        value = (value << 7) | (b & 0x7f)
        if not b & 0x80:
            return value, pos + 1
    raise AssertionError("unreachable")
```
