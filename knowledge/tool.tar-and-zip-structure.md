---
id: tool.tar-and-zip-structure
version: 1
kind: tool
title: Read, write, and verify tar and zip archives at the byte level
summary: >-
  tar is a stream of 512-byte headers and padded data ending in two zero
  blocks; zip is local entries plus a central directory found from the end.
  Know the header fields, the long-name and large-size extensions, and how to
  make output deterministic, then verify with independent tools.
tags: [archive, tar, zip, file-format, binary-format, reproducible-builds]
applies_when: >-
  Implementing, cloning, repairing, or checking a tool that reads or writes
  tar or zip archives, or making archive output byte-reproducible.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "IEEE Std 1003.1-2017 (POSIX), pax utility: ustar Interchange Format and pax Interchange Format"
    - "PKWARE, APPNOTE.TXT - .ZIP File Format Specification, version 6.3.10"
    - "GNU tar manual, sections on archive format and --sort, --mtime, --owner, --numeric-owner"
    - "Python Software Foundation, tarfile: extraction filters (filter='data'); zipfile"
    - "Reproducible Builds, Archive metadata (reproducible-builds.org/docs/archives)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**tar (ustar/pax).** Each member is a 512-byte header followed by its data
padded with zeros to a multiple of 512; the archive ends with at least two
all-zero blocks (GNU tar pads the file to a 10240-byte record by default).
Header fields are fixed-width ASCII: `name` (100 bytes) plus `prefix` (155),
`mode`, `uid`, `gid`, `size` (12 bytes, octal), `mtime` (octal), `chksum`,
`typeflag` (`0` file, `5` directory, `2` symlink, `1` hard link, `x`/`g` pax
headers, `L`/`K` GNU long name/link), `linkname`, magic `ustar\0` + version
`00` (POSIX) or `ustar  \0` (old GNU). Numeric fields are octal terminated by
NUL or space. The checksum is the sum of all header bytes as unsigned values
with the checksum field itself counted as eight spaces, stored as six octal
digits, NUL, space. Names longer than ustar allows, sizes of 8 GiB or more,
and non-ASCII metadata go into a pax `x` record (`"%d %s=%s\n"` lines where
the leading length counts the whole line) or GNU `L` records.

**zip.** Each entry has a local file header (signature `PK\3\4`) then data;
the authoritative index is the central directory near the end, located from
the End of Central Directory record (`PK\5\6`, searched backwards because a
comment may follow it). All integers are little-endian. Method `0` is stored,
`8` is deflate (raw deflate, no zlib header). If flag bit 3 is set, the CRC
and sizes follow the data in a data descriptor. More than 65535 entries or
4 GiB sizes need ZIP64 records. DOS timestamps have two-second resolution and
no time zone.

**Deterministic output.** Fix member order (sort by name), mtimes
(`--mtime=@$SOURCE_DATE_EPOCH` or a constant), owner and group
(`--owner=0 --group=0 --numeric-owner`), permissions, and compression
settings; for gzip use `gzip -n` so the name and time are not stored.

**Safe extraction.** Reject absolute paths, `..` components, and links that
point outside the destination; in Python use `tarfile` with `filter='data'`.

## How to check

Round-trip against independent implementations: list and extract with
`tar -tvf`/`bsdtar`, `python -m tarfile -l`, `unzip -t`, and `zipinfo -v`, and
compare extracted trees with `diff -r` plus modes and mtimes. When cloning
another tool's output byte for byte, hex-dump both archives (`xxd`, `cmp -l`)
and compare header fields one at a time, especially padding, checksum
formatting, and the end-of-archive blocks.
