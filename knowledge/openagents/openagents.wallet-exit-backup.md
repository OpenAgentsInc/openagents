---
id: openagents.wallet-exit-backup
version: 1
kind: product
title: "The exit backup"
summary: >-
  The Wallet saves Spark's exit state, encrypted, after each sync, and its
  Exit backup section exports it to Files.
tags: [wallet, exit, backup, recovery]
applies_when: >-
  The user asks what the exit backup is, why recovery words might not be
  enough, or how to recover funds if Spark's operators stop.
answer: >-
  Recovery words alone may not recover funds while Spark's operators are down.
  So the Wallet saves your exit state, encrypted on the phone, after each
  sync, and its Exit backup section says when it was last saved and exports it
  to Files. Running an exit from that file is a later recovery tool.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/breez/wallet-design.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Recovery words alone may not recover funds while Spark's operators are down. So the Wallet saves your exit state, encrypted on the phone, after each sync, and its Exit backup section says when it was last saved and exports it to Files. Running an exit from that file is a later recovery tool.

## Details

- The exit state holds leaf transactions, no keys.
- Restoring to other recovery words clears it.

## Sources

- `docs/breez/wallet-design.md`
- `INVARIANTS.md`
