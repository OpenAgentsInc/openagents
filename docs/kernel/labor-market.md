# Labor Market

> Canonical market-status doc: [markets/labor-market.md](./markets/labor-market.md)

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

## Boundary With Compute

Labor begins where a provider is asked to interpret or solve an open-ended objective rather than execute a bounded declared runtime contract. If the request is `run this bounded thing in this declared sandbox`, that belongs in Compute. If the request is `figure out what to do and do it`, that belongs in Labor. Ambiguous requests should not be silently coerced into a compute-shaped contract.

## Current implementation status

- `implemented`: starter authority flows in `openagents-kernel-core` and `apps/nexus-control` for work units, contracts, submissions, and verdict finalization, currently exercised by the compute-provider earn flow
- `local prototype`: desktop-local receipts, policy, incidents, and snapshots already model broader labor-market semantics
- `planned`: generalized worker assignment, disputes, claims, and broader labor-market productization
