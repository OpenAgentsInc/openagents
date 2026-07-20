# AFS-10 owner-local experience memory

Date: 2026-07-20
Status: implemented, default off
Package: `@openagentsinc/agent-experience-memory`
Issue: #9088

## Purpose

This document records the design of the optional owner-local experience memory.
The memory adds redacted experience recall for the Apple FM router and the
coding loop. The memory is off by default. With the memory off, the router and
the integrated development environment behave the same as a build without the
package.

## Design rules

The memory obeys these rules:

- The memory is off by default. A checked-in flag gates every read and write.
  With the flag off, the code reads no record, writes no record, and adds no
  slice to a prompt.
- The memory is never necessary. No router path and no development-environment
  path depends on the memory. The memory is an advisory input only.
- The memory is redacted. Each stored fact and each recalled slice passes the
  ATIF redaction service. A fact with a secret, a wallet value, a payment value,
  or a local path is rejected. Soft personal data is scrubbed.
- The memory is owner-scoped and project-scoped. One owner scope never reads
  another owner scope. Recall stays inside one project.
- The memory is local only. The portable package holds no cloud client, no
  structured-query-language driver, no provider software-development kit, and no
  Node host. The durable adapter for local storage lives in the application.
- The memory is frozen and one-shot. The code freezes one eligible bank at the
  start of a turn. The code runs at most one adaptation before the turn. A
  digest binds the adaptation to the turn. Current-turn data cannot change
  current-turn input.
- The memory is structured to measure. A report compares the off state and the
  on state. The report gives the acceptance delta and the correction delta.

## Structure

The package has these parts:

- A portable contract. The contract holds the references, the scopes, the
  default-off flag, the per-case experience record, the distilled global
  pattern, the frozen bank, and the recall result.
- A redaction guard. The guard reuses the ATIF redaction service. The guard
  rejects a hard-unsafe fact and scrubs a soft-unsafe fact.
- Ranking primitives. The primitives reuse the reviewed algorithm ideas of the
  unwired Pylon TAS kit. The package imports nothing from that kit.
- A store port. The port owns scope isolation, consent filtering, freeze, and
  the owner lifecycle. The package ships an in-memory adapter and a disabled
  no-op adapter. The durable local adapter lives in the application.
- A measurement harness. The harness computes the benefit report from paired
  samples.

## Reuse

The design extends the existing substrate. It reuses the ATIF redaction
boundary, the ATIF trace store as the per-case layer, and the reviewed TAS
algorithm ideas. The only new surface is the distilled global-pattern layer and
the recall path behind the flag.

## Benefit status

Version one ships with the flag off. The team has measured no live benefit. The
measurement harness exists so that a later decision to turn the memory on rests
on evidence, not on assertion.

## Boundaries

The check `scripts/check-afs-boundaries.ts` governs the package. The package
imports no application, no platform interface, no provider software-development
kit, no structured-query-language driver, no cloud client, and no Node host.
The package uses no unchecked type cast.

## References

- Grounded design analysis:
  `docs/research/2026-07-20-memoharness-openagents-integration-analysis.md`
- Paper summary: `docs/research/2026-07-18-memoharness-paper-summary.md`
- Plan section AFS-10:
  `docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md`
- ATIF redaction: `packages/atif/src/redaction.ts`
- ATIF trace tripwire: `packages/atif/src/trace-schema.ts`
