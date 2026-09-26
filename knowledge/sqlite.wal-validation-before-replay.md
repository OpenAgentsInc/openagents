---
id: sqlite.wal-validation-before-replay
version: 1
kind: method
title: Validate a damaged or transformed SQLite WAL before letting SQLite replay it
summary: >-
  When a SQLite write-ahead log is damaged or transformed (for example by a
  bytewise XOR), work on copies, decode candidates, validate the WAL header,
  salts, and frame checksums, and only then let SQLite replay the verified
  sidecar before exporting data.
tags: [sqlite, wal, forensics, recovery, database, file-format]
applies_when: >-
  A SQLite database has a matching WAL sidecar whose bytes appear consistently
  transformed and must be recovered without modifying the base database
  unnecessarily.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - db-wal-recovery
  cites:
    - SQLite, Database File Format, Write-Ahead Log (WAL) and WAL File Format sections, https://www.sqlite.org/fileformat.html
    - SQLite, PRAGMA Statements, integrity_check section, https://www.sqlite.org/pragma.html#pragma_integrity_check
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Work on copies first: SQLite may checkpoint or otherwise change the WAL when it opens a writable database. A uniform XOR transformation is self-inverse, so decode each byte as `byte ^ key`; do not accept a candidate merely because its first bytes resemble a WAL header.

The SQLite WAL header is 32 bytes. Its big-endian fields include magic, format version, page size, checkpoint sequence, salts, and a pair of checksum words. Magic `0x377f0682` specifies that checksum input words are interpreted little-endian; `0x377f0683` specifies big-endian. Each frame consists of a 24-byte header and one database page. The frame header carries page number, commit database size, salts, and checksum pair. The checksum is a rolling pair over the first 24 WAL-header bytes, then each frame's first eight header bytes concatenated with its page payload; the stored checksum words themselves are excluded. Validate the header checksum, every frame checksum in sequence, matching frame/header salts, sensible page size, and complete frame boundaries before asking SQLite to consume the WAL. A nonzero database-size field marks a commit; data in frames after the most recent commit must not be mistaken for committed state.

Keep the original database and WAL untouched, place the decoded WAL under the exact matching sidecar name beside a working copy, and open that database normally so SQLite applies its own recovery rules. Export through a read-only connection, run `PRAGMA integrity_check`, and compare the export against a fresh parse. Sources: SQLite, *Database File Format*, “Write-Ahead Log (WAL)” and “WAL File Format” sections (https://www.sqlite.org/fileformat.html); SQLite, *PRAGMA Statements*, `integrity_check` section (https://www.sqlite.org/pragma.html#pragma_integrity_check).
