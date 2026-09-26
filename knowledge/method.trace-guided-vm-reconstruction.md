---
id: method.trace-guided-vm-reconstruction
version: 1
kind: method
title: Reconstruct a small virtual machine from execution traces
summary: >-
  Use before/after register, flag, and memory snapshots to infer instruction
  semantics, then implement an emulator and validate it against held-out
  traces before using it to derive cryptographic material.
tags: [reverse-engineering, vm, emulation]
applies_when: >-
  A binary contains a compact custom bytecode and supplied traces expose
  instruction bytes and machine state transitions.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - shadow-relay
  cites:
    - Intel, Intel 64 and IA-32 Architectures Software Developer's Manual, Volume 3, §4
    - Unicorn Engine documentation, Emulation basics
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details
First establish instruction framing and field positions from program bytes and trace PCs. For each opcode, compare pre/post state and design probes or select trace instances where operands differ; infer register writes, flag behavior, control flow, and memory side effects independently. Pay particular attention to integer width and wraparound, shift/rotate edge cases, branch target units, and effective-address computation (for example, concatenated page/offset registers versus addition).

Implement the inferred ISA in a small emulator rather than manually translating a long bytecode. Keep the memory model and initial state explicit, put a step limit on execution, and reject unknown opcodes or out-of-range accesses. Validate every available trace step—including unchanged registers and flags—against the emulator. Prefer some traces for inference and others as holdouts, so a mistaken interpretation is exposed before interpreting output as a key.

For memory safety and emulator construction principles, see Intel, *Intel 64 and IA-32 Architectures Software Developer's Manual*, Volume 3, §4 (Paging), and Unicorn Engine documentation, “Emulation basics”; these describe general address/state concepts, while the target ISA's actual semantics must be derived from its own traces and artifacts.

## How to check
```python
for trace in traces:
    state = initial_state(trace)
    for step in trace:
        before = snapshot(state)
        execute_one(state, step.instruction)
        assert snapshot(state) == step.expected_after
```
Also assert bounded termination, valid instruction alignment, and that memory/register values stay within the inferred machine widths.
