# CS336 A1 Homework No-Spend Rehearsal

Date: 2026-06-10

Issue: [#4675](https://github.com/OpenAgentsInc/openagents/issues/4675)

This is the current OpenAgents-owned rehearsal path for CS336 A1 homework on
the rebuilt Worker stack. It packages the existing Psionic lane
`psion_cs336_a1_demo_v1` as a public-safe job kind for the buy-mode dispatcher
rail and binds the returned closeout evidence to the verification queue.

## Boundary

OpenAgents owns:

- the CS336 A1 dispatch payload shape;
- public-safe run, window, assignment, closeout, and verification refs;
- the buy-mode/NIP-90 dispatch rail;
- deterministic recompute and Freivalds/Merkle verification challenge records;
- no-spend rehearsal receipts and blockers.

Psionic owns actual training execution, release manifests, job contracts,
artifact identity, and worker receipt format. The live paid path remains
blocked until the external Psionic training boundary and explicit operator
spend approval are both present.

## Job Kind

`src/cs336-a1-homework.ts` defines:

- `jobKind`: `cs336_a1_homework`;
- `psionicLaneRef`: `psion_cs336_a1_demo_v1`;
- request schema: `psion.cs336_a1_demo_automatic_execution_request.v1`;
- output schema: `psion.cs336_a1_demo_automatic_execution_outputs.v1`.

The dispatch payload includes verification bindings:

- tokenizer/BPE shard work uses `deterministic_recompute`;
- training-step matrix work uses `freivalds_merkle`;
- sampling policy is `per_contribution`.

## Smoke

Run from `apps/openagents.com/workers/api`:

```sh
bunx vitest run src/cs336-a1-homework.test.ts src/training-verification.test.ts
```

The smoke starts a disabled-spend buy-mode campaign, dispatches a CS336 A1
homework payload through the NIP-90 rail, imports closeout-shaped public
evidence, creates both verification challenge kinds, finalizes verified
verdicts, and projects a no-spend accepted rehearsal.

The projection intentionally includes:

- `blocker.cs336_a1.paid_settlement_requires_operator_spend_approval`;
- `blocker.cs336_a1.psionic_execution_boundary_external`.

Those blockers prevent the rehearsal from being mistaken for a paid live
training closeout.

## Public-Safe Evidence Rules

Do not put raw model weights, local paths, mnemonics, invoices, payment hashes,
preimages, wallet state, provider credentials, raw worker logs, or private
Psionic payloads into dispatch content, closeout evidence, Forum posts, issue
comments, or public receipts.
