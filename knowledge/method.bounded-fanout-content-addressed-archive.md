---
id: method.bounded-fanout-content-addressed-archive
version: 1
kind: method
title: Lossless bounded-fanout archival with verification and staged restore
summary: >-
  When output constraints limit both individual file size and directory entry
  count, stream an archive into bounded shards arranged in a fanout tree, then
  verify and restore transactionally. Preserve filesystem metadata explicitly
  and validate the complete round trip rather than relying on archive creation
  success.
tags: [archive, filesystem, integrity, sharding, python]
applies_when: >-
  Building a portable archive or dataset transformation under per-file and
  per-directory limits, especially when restoration must preserve source
  contents and failures must not damage existing data.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - reshard-c4-data
  cites:
    - POSIX, pax - Portable Archive Interchange, IEEE Std 1003.1, pax utility and archive format sections
    - Deutsch, GZIP file format specification version 4.3, RFC 1952
    - NIST, Secure Hash Standard, FIPS PUB 180-4, SHA-256 section
    - Python Software Foundation, tarfile — Read and write tar archive files, extraction filters and streaming modes
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

A practical design separates storage from namespace structure: stream a tar archive through gzip, split its compressed byte stream into fixed maximum-size shard files, and place shard files in a directory tree whose fanout also obeys the directory-entry bound. Use a bounded-radix path mapping for shard indexes; for radix `b`, encode index `i` as bijective base `b` (repeatedly compute `(i-1) divmod b`) so paths remain unique and fanout is bounded. Count both files and child directories against the entry limit.

Record a manifest containing format/version, shard count and sizes, total stream size, and a cryptographic digest over the ordered concatenated shard bytes. Restoration must validate manifest fields, shard membership, sizes, and digest before trusting archive contents. Reject unexpected entries and unsafe archive paths (absolute paths, parent traversal, or paths escaping the destination). Restore into a staging directory and publish only after extraction and validation succeed; avoid deleting a pre-existing destination as a recovery shortcut.

Tar archives can represent regular files, directories, symbolic links, hard links, and metadata. Decide and document which file types and metadata are in scope; silently following links or treating special files as regular files breaks fidelity or introduces security hazards. For large files, stream their contents rather than loading them into memory. Gzip/tar framing adds overhead, so the shard limit applies to the emitted shard bytes, not the original input size.

This design follows the tar format and gzip stream semantics; shard integrity can use SHA-256. Sources: POSIX, *pax - Portable Archive Interchange*, IEEE Std 1003.1, `pax` utility and archive format sections; Deutsch, *GZIP file format specification version 4.3*, RFC 1952; NIST, *Secure Hash Standard (SHS)*, FIPS PUB 180-4, SHA-256 section; Python Software Foundation, *tarfile — Read and write tar archive files*, documentation sections on extraction filters and streaming modes.

## How to check

Test with a temporary tree containing empty files/directories, nested paths, arbitrary Unicode names, links if supported, and incompressible content larger than one shard. After archiving and restoring, compare relative paths, entry types, sizes, and hashes; separately assert every file and directory satisfies its bound. Corrupt or remove a shard and verify restore fails while leaving the original archive and destination intact.

```python
import hashlib, os
from pathlib import Path

def digest(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def tree(root):
    result = {}
    for p in root.rglob('*'):
        rel = str(p.relative_to(root))
        if p.is_symlink():
            result[rel] = ('link', os.readlink(p))
        elif p.is_dir():
            result[rel] = ('dir',)
        else:
            result[rel] = ('file', p.stat().st_size, digest(p))
    return result

assert tree(source) == tree(restored)
for base, dirs, files in os.walk(sharded):
    assert len(dirs) + len(files) <= max_entries
    for name in files:
        assert (Path(base) / name).stat().st_size <= max_file_bytes
```
