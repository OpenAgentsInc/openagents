---
id: method.controlled-authentication-state-overwrite
version: 1
kind: method
title: Verify a bounded-input overwrite against the compiled stack layout
summary: >-
  When disassembly shows unchecked input adjacent to an authorization
  variable, derive the overwrite from observed offsets and value width, then
  test the normal program path rather than guessing an exploit string.
tags: [binary-exploitation, stack, reverse-engineering]
applies_when: >-
  A native executable reads attacker-controlled input into a stack buffer and
  later branches on nearby stack-resident state.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - vulnerable-secret
  cites:
    - System V Application Binary Interface, AMD64 Architecture Processor Supplement, Function Calling Sequence and Data Representation
    - "MITRE, CWE-120: Buffer Copy without Checking Size of Input"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Use disassembly and calling-convention evidence to determine the buffer base, the target variable's offset, its width, and the comparison performed. An overflow payload must account for the exact distance from the input start to the variable, the variable's native representation and endianness, and any input terminator behavior. Do not infer offsets solely from source-like comments or a suspected buffer size. A direct overwrite of an authorization flag is distinct from overwriting a saved return address; choose the least complex path supported by control flow.

Where possible, demonstrate the result by running the executable normally with a controlled input and observing that the intended branch is reached. This validates the computed layout and also exposes environmental checks that may alter behavior. Consult the target ABI for stack and data representation conventions.

## How to check

Derive the offset from instructions that address the input buffer and tested variable, then construct a payload with that many filler bytes and a correctly encoded value. Run it through the program and verify the expected branch marker; avoid relying only on a crash or a plausible disassembly interpretation.

Sources: System V Application Binary Interface, *AMD64 Architecture Processor Supplement*, “Function Calling Sequence” and “Data Representation”; MITRE, *CWE-120: Buffer Copy without Checking Size of Input* (description).
