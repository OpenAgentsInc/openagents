---
id: method.reconstruct-fragmented-zip-entry
version: 1
kind: method
title: Reconstruct a ZIP entry from separated local and central records
summary: >-
  When a damaged or fragmented disk image contains remnants of a ZIP file, use
  the local header, central-directory header, and end record to recover entry
  metadata and locate split payload fragments; validate the result with CRC
  and a ZIP reader.
tags: [digital-forensics, zip, data-recovery, python]
applies_when: >-
  A raw image contains ZIP signatures and entry-name or content fragments, but
  no directly readable complete archive.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - password-recovery
  cites:
    - "PKWARE, *APPNOTE.TXT: .ZIP File Format Specification*, sections “Local file header,” “Central directory structure,” and “End of central directory record.”"
    - Python Software Foundation, *zipfile — Work with ZIP archives*, `ZipFile.testzip` and `ZipFile.read` documentation.
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

ZIP local file headers describe an entry's compression method, CRC-32, compressed and uncompressed sizes, filename, and extra-field lengths. The central-directory record independently describes the entry and gives the offset of its local header; the end-of-central-directory record locates the directory and records the archive's entry counts and size. Parse these records using their documented little-endian field layouts rather than relying on file carving heuristics alone. See PKWARE, *APPNOTE.TXT: .ZIP File Format Specification*, sections “Local file header,” “Central directory structure,” and “End of central directory record.”

For a stored, uncompressed entry, the payload begins immediately after the local header, filename, and extra field. If the payload is split across image regions, the expected uncompressed size and CRC can help establish fragment length and test candidate joins. Preserve bytes exactly; do not infer missing characters from a format pattern alone. Rebuild a minimal archive in memory if useful, then verify both the entry CRC and archive readability. This approach is specific to stored entries unless decompression and compressed-size accounting are handled explicitly.

## How to check

Use the Python standard library to validate a reconstructed archive and independently check the entry bytes against the CRC recorded in the local header:

```python
import io, zipfile, zlib

assert len(payload) == expected_uncompressed_size
assert zlib.crc32(payload) & 0xffffffff == expected_crc
with zipfile.ZipFile(io.BytesIO(reconstructed_zip)) as archive:
    assert archive.testzip() is None
    assert archive.read(entry_name) == payload
```

For each suspected record, also check that declared filename and extra-field lengths place the payload and following records at plausible, in-bounds offsets.
