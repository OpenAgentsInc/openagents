# Executor-Trace Work As The First Autonomous Artanis Loop Candidate

Date: 2026-06-10

Status: implemented as the first typed Artanis scheduled-runner work class in
`apps/openagents.com/workers/api/src/artanis-scheduled-runner.ts`,
`artanis-continual-learning-templates.ts`, and
`artanis-work-routing.ts` under openagents#4697. The full review lives in
section 5 of `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md`;
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

Implemented tie-in items:

- `executor_trace_replay` is a continual-learning template kind with
  Tassadar payload-schema refs, workload refs, the
  `capability.tassadar_poc.numeric_model_executor` capability ref, and a
  zero-sats default spend cap.
- The config-gated scheduled runner now records an executor-trace tick:
  no-spend Pylon dispatch refs, exact-replay verdict refs, deterministic
  closeout receipts, and a Forum publication intent. The loop still writes
  evidence only and grants no direct dispatch, settlement, spend, or Forum
  publish authority.
- The paid sample is represented as a `wallet_spend` approval requirement and
  pending approval gate whose authority ref is the operator spend-enable.
- The Forum intent body is pinned by test to the
  `compute.tassadar_executor_poc.v1` promise `safeCopy` and may not broaden
  the executor, earning, or model-capability claim.

Remaining prerequisites before green autonomy: production runner enablement
under operator control, a sustained unattended tick streak, the public
tick-ledger monitor, the first trace-to-distillation dataset receipt, Pylon
registration ownership for settlement-event writes, and either hosted-MDK
programmatic payouts or a registered local agent-wallet payout adapter.

Tracking issue: openagents#4697.
