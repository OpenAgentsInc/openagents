# Labor Market

## Purpose

The Labor Market buys and sells machine work.

It is where buyers create work, providers perform that work, verifiers evaluate the outcome, and the kernel decides whether the result can settle.

## Core objects

- `WorkUnit`
- `Contract`
- `Submission`
- `Verdict`
- `Claim`

## Authority flows

- create work
- assign worker or verifier
- submit output
- finalize verdict
- settle or dispute

## Settlement model

Labor settles against verified outcomes, not only against attempted execution.

That means the market does not stop at "a machine ran." It terminates only when the kernel can bind:

- the work definition
- the submitted output
- the verification evidence
- the verdict
- the settlement or remedy path

## Current implementation status

- `implemented`: starter authority flows in `openagents-kernel-core` and `apps/nexus-control` for work units, contracts, submissions, and verdict finalization, currently exercised by the compute-provider earn flow
- `local prototype`: desktop-local receipts, policy, incidents, and snapshots already model broader labor-market semantics
- `planned`: generalized worker assignment, disputes, claims, and broader labor-market productization
