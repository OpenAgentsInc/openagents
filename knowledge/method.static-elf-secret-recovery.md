---
id: method.static-elf-secret-recovery
version: 1
kind: method
title: Recover embedded secrets from ELF data and verify dynamically
summary: >-
  When an executable contains secret-printing or decryption logic, combine
  section-aware static inspection with a controlled runtime check; do not
  assume the apparent string start or terminator is correct.
tags: [elf, reverse-engineering, binary-analysis, secret-recovery]
applies_when: >-
  Analyzing a local ELF whose strings, symbols, or disassembly suggest
  embedded encrypted data or a hidden secret-printing path.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - vulnerable-secret
  cites:
    - System V Application Binary Interface, Generic ABI, Program Loading and Dynamic Linking
    - Intel, Intel 64 and IA-32 Architectures Software Developer’s Manual, Volume 2, Instruction Set Reference
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Start with low-cost observations (`strings`, `readelf -S`, `nm`, `objdump -d`) and identify the code path that consumes the candidate data. ELF virtual addresses are not file offsets: use the containing section or, for general loadable segments, the `PT_LOAD` mapping (`p_offset + (vaddr - p_vaddr)`). Confirm the address lies within the file-backed portion of the segment; `.bss` has no corresponding bytes on disk.

Follow the actual loop precisely: establish its initial pointer, per-byte transform, and termination condition from instructions rather than inferring them from a nearby label or a presumed C string. A bytewise XOR transform can be checked with `plain[i] = cipher[i] ^ key`, but an off-by-one start or sentinel interpretation can corrupt the recovered prefix. If a legitimate program path can reveal the value, use it as an independent confirmation. Treat anti-debug checks as control-flow conditions to understand, not as evidence that static inspection is impossible.

## How to check

For a candidate virtual address, map it through `readelf -lW`'s loadable segment table and check the resulting offset is in file bounds. Then compare the recovered bytes against the value emitted through the program's ordinary input path, where safe. For simple section-relative mapping:

```python
offset = section_file_offset + (candidate_vaddr - section_vaddr)
assert 0 <= offset < len(binary)
```

A successful check should validate both the decoded bytes and the loop's exact start and stop behavior. Sources: System V Application Binary Interface, *Generic ABI*, “Program Loading and Dynamic Linking”; Intel, *Intel 64 and IA-32 Architectures Software Developer’s Manual*, Volume 2, instruction reference (for interpreting disassembly).
