# Executor-Trace Work As The First Autonomous Artanis Loop Candidate

Date: 2026-06-10

Status: analysis pointer. The full review lives in section 5 of
`docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md`;
this note keeps the Artanis doc set self-contained.

Summary of the argument:

- The green `compute.tassadar_executor_poc.v1` proof of concept
  (registry 2026-06-10.12, epic #4687) was one agent performing the
  Artanis tick by hand: pylon triage → dispatch → closeout acceptance on
  a digest predicate → replay verification → settlement bridge → Forum
  report. Every step has a typed Artanis surface waiting to own it, and
  the Nexus-Pylon payout adapters already carry `artanisDispatchRef` /
  `artanisRunRef` fields.
- Executor-trace is the one work class whose entire
  dispatch → execute → verify → accept span can be `safe`-classed under
  the loop contract's risk rules, because acceptance is a digest
  comparison and verification is deterministic replay. The single
  remaining `approval_required` action is `wallet_spend` at payout —
  exactly where the owner's standing spend posture draws the line.
- Psionic's own conformance backlog (fixture sweeps, harness replays,
  cross-language digest parity) gives the loop a queue that never
  empties, and the cleanest unit for the comparative-economics packets:
  cost per verified trace.

Concrete tie-in items (beyond the v0.3 release; see audit §5.4): an
executor-trace continual job template, scheduled-runner tick wiring,
`wallet_spend` approval plumbing onto the existing adapters, copy gates
pinned to the promise safeCopy, and the two PoC residuals
(registration ownership; hosted-MDK programmatic payouts or a
registered agent-wallet adapter) as prerequisites.

Tracking issue: openagents#4697.
