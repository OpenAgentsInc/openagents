# Kernel Markets

`docs/kernel/markets/` is the canonical per-market status layer for the
OpenAgents Economy Kernel.

Use this subtree when the question is:

- what does this market mean,
- what is actually implemented in the repo now,
- what only exists as a local prototype or design draft,
- and what is still planned.

Keep `docs/kernel/README.md` as the high-level kernel overview and market
taxonomy. Keep `docs/kernel/economy-kernel.md` and
`docs/kernel/economy-kernel-proto.md` as the normative kernel and wire-level
specs. Use the files in this subtree for definitive market-by-market reality.

## Status legend

- `implemented`: shipped in the current repo entry points
- `local prototype`: modeled in desktop-local receipts, snapshots, or adjacent
  docs, but not yet generalized into the canonical authoritative market surface
- `planned`: target architecture or product lane, not yet shipped

## Market matrix

| Market | Product surface today | Kernel authority today | Wire/proto today | Canonical doc |
| --- | --- | --- | --- | --- |
| `Compute` | `implemented` and productized in Autopilot's provider earn loop | `implemented` and deepest of the five, with a materially stronger Psionic execution/train/eval substrate beneath future compute families | `implemented` thin `openagents.compute.v1` slice plus generated Rust types | [compute-market.md](./compute-market.md) |
| `Data` | not productized | `implemented` starter authority slice | no dedicated checked-in `openagents.data.v1` package yet | [data-market.md](./data-market.md) |
| `Labor` | partially productized through the compute-provider flow and local Codex orchestration | `implemented` starter authority slice | `implemented` thin `openagents.labor.v1` slice | [labor-market.md](./labor-market.md) |
| `Liquidity` | not productized | `implemented` starter authority slice | no dedicated checked-in liquidity proto package yet | [liquidity-market.md](./liquidity-market.md) |
| `Risk` | not productized | `implemented` starter authority slice | no dedicated checked-in risk proto package yet | [risk-market.md](./risk-market.md) |

## Placement decision

These docs live under `docs/kernel/markets/` because they are:

- kernel-domain documentation, not MVP product docs,
- market-specific rather than kernel-wide,
- and status-oriented rather than deep protocol or runtime design notes.

That keeps `docs/kernel/` root for:

- kernel-wide specs,
- diagrams,
- coordination contracts,
- and compute-adjacent extension docs that are narrower than a whole market.

## Current supporting docs outside this subtree

- [../README.md](../README.md): kernel overview and the five-market taxonomy
- [../economy-kernel.md](../economy-kernel.md): normative kernel spec
- [../economy-kernel-proto.md](../economy-kernel-proto.md): proto and policy
  plan
- [../compute-environment-packages.md](../compute-environment-packages.md):
  compute environment registry
- [../compute-evaluation-runs.md](../compute-evaluation-runs.md): compute eval
  lifecycle
- [../compute-synthetic-data.md](../compute-synthetic-data.md): compute
  synthetic-data lifecycle
- [../compute-benchmark-adapters.md](../compute-benchmark-adapters.md):
  benchmark-to-eval import layer
- [../../../crates/psionic/docs/ARCHITECTURE.md](../../../crates/psionic/docs/ARCHITECTURE.md):
  Psionic execution-substrate ownership and current state
- [../../../crates/psionic/docs/TRAIN_SYSTEM.md](../../../crates/psionic/docs/TRAIN_SYSTEM.md):
  Psionic train/eval/runtime state and remaining boundaries
- [../prediction-markets.md](../prediction-markets.md): deeper risk-market
  background on prediction, coverage, and underwriting semantics
