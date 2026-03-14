# Labor Market

This is the canonical status doc for the `Labor Market`.

## Purpose

The Labor Market buys and sells machine work.

Kernel-facing objects:

- `WorkUnit`
- `Contract`
- `Submission`
- `Verdict`
- `Claim`

Labor is where buyers define work, workers submit outcomes, verifiers evaluate
those outcomes, and the kernel decides whether the result can settle.

## Current repo verdict

| Dimension | Status | Notes |
| --- | --- | --- |
| Product surface | partial | labor semantics exist in the compute-provider flow and local Codex orchestration, but there is no generalized labor market product lane |
| Kernel authority | `implemented` starter slice | work units, contracts, submissions, and verdicts are live in kernel authority |
| Wire/proto | `implemented`, thin | `proto/openagents/labor/v1/work.proto` exists |
| Local prototype | `implemented` | broader receipt, incident, policy, and snapshot semantics still live partly in desktop-local or adjacent modeling |
| Planned | yes | generalized worker assignment, disputes, claims, and broader labor-market productization remain planned |

## Implemented now

- create a `WorkUnit`
- create a `Contract`
- submit output
- finalize a `Verdict`

These flows are implemented in:

- `crates/openagents-kernel-core/src/labor.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- `proto/openagents/labor/v1/work.proto`

This starter labor slice is already exercised by the active compute-provider
earn loop, which means labor is not merely theoretical. It is just not yet a
standalone market lane.

## Local prototype or partial only

- desktop-local receipts, policies, incidents, and snapshots model broader
  labor semantics than the current backend slice exposes
- local Codex-oriented labor orchestration exists, but it is not yet a
  generalized market for arbitrary third-party labor supply
- broader claims and disputes are named in the object model and kernel spec,
  but not yet productized as a complete labor-market lifecycle

## Not implemented yet

- a generalized open labor market in Autopilot
- worker assignment and matching as a full market product
- richer claims, disputes, and remedies as a live market surface
- buyer-facing discovery and purchasing UX for machine labor
- seller-facing inventory and pricing UX for labor providers beyond the current
  compute-led path

## Current repo truth lives in

- `crates/openagents-kernel-core/src/labor.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- `proto/openagents/labor/v1/work.proto`
- [../economy-kernel.md](../economy-kernel.md)
- [../economy-kernel-proto.md](../economy-kernel-proto.md)

## Boundary notes

- labor starts when the request is outcome-oriented and open-ended
- if the request is only declared runtime execution, it belongs in Compute
- labor may consume compute and data, but it is not reducible to either
- risk and liquidity overlay labor after the work exists and begins to settle
