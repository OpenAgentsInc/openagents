---
id: method.elf-pt-load-memory-dump
version: 1
kind: method
title: Dump ELF file-backed memory by parsing PT_LOAD segments
summary: >-
  To extract values mapped by an ELF executable, use its program headers
  rather than assuming sections or that virtual addresses equal file offsets.
  Map each file-backed segment from p_offset to p_vaddr and decode words in
  the ELF byte order.
tags: [elf, binary-format, program-headers, memory]
applies_when: >-
  Implementing a binary memory extractor or validating addresses and contents
  in ELF files, especially across PIE and non-PIE variants.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - extract-elf
  cites:
    - System V Application Binary Interface, Generic ABI, “ELF Header” and “Program Header” sections
    - Linux man-pages, elf(5), “Program header (Phdr)”
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

ELF program headers describe segments used to construct a process image; `PT_LOAD` entries give file offset, virtual address, file size, and memory size. For a byte at segment-relative offset `i`, its file byte is at `p_offset + i` and its virtual address is `p_vaddr + i`. Do not treat virtual addresses as file offsets: equality may hold for a particular layout but is not guaranteed.

For a dump of file-backed data, process only `p_filesz` bytes. The additional range up to `p_memsz` is zero-initialized memory (commonly BSS), not bytes present in the file. To emit aligned 32-bit words, choose and document an alignment convention; iterating at four-byte intervals from each segment's start can omit values under a globally aligned-address interpretation when segment starts are unaligned. A robust extractor should align according to the desired address grid and ensure each complete word lies within the mapped/file-backed range. Handle overlapping segment ranges deliberately if present.

Read ELF identification fields for class and data encoding, then use the corresponding 32- or 64-bit program-header layouts and endianness. Use unsigned 32-bit reads for word values. For 64-bit addresses, preserve integer precision (for example, JavaScript `BigInt`) until serialization; JSON object property names can be decimal strings.

Sources: System V Application Binary Interface, *Generic ABI*, sections “ELF Header” and “Program Header”; Linux man-pages, `elf(5)`, “Program header (Phdr).”

## How to check

For every emitted address `a`, verify that it lies in a `PT_LOAD` segment and that the four source bytes are within that segment's `p_filesz` range. Independently map back with `fileOffset = p_offset + (a - p_vaddr)` and decode using the ELF's byte order; assert equality with the emitted value. Cross-check selected ranges using `readelf -lW` and `objdump -s`, while remembering that section dumps do not necessarily cover every loadable segment.

```js
const off = segment.p_offset + (address - segment.p_vaddr);
if (address < segment.p_vaddr || address + 4 > segment.p_vaddr + segment.p_filesz) throw new Error('not file-backed');
const value = littleEndian ? buf.readUInt32LE(off) : buf.readUInt32BE(off);
```
