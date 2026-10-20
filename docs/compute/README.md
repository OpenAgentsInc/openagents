# Compute

These documents cover shared compute: people selling their machines' spare
capacity for bitcoin, and agents buying it. OpenAgents built this several
times between 2023 and 2026. It returns in Coder, and its first visible home
is Everglade in the Verse.

| Document | What it covers |
| --- | --- |
| [Compute in the Verse](verse-compute.md) | The vision and implementable spec: the Pylon Field and the Wellspring in Everglade, the provider side, the job flow, payments and splits, verification, the world projection, the NIPs, abuse and economics, and the phased plan. |
| [Run a Pylon, and use one](pylon.md) | The operator guide for what runs today: serving, decisions, checks, pools, paid jobs on test networks. |
| [Pylon network runbook](pylon-network-runbook.md) | Installing Pylon 1.0 on a Mac (Metal), NixOS with an NVIDIA GPU (CUDA), and a Google Compute Engine VM (CPU); how they find each other; the cross-job test and its results; troubleshooting; and how to stop each pylon. |
| [Pylon 1.0](pylon-1.0.md) | The spec for Pylon 1.0 in the `openagents` CLI: one command online, the pylon's Nostr identity, compute, calibrated checks, revenue-share pay, the MCP server, and our version of Decision Models' and REAX's work. |
| [Compute for bitcoin: the history](compute-for-bitcoin.md) | The public posts, episodes, and implementing commits from GPUtopia in 2023 through Pylon, the compute market, the paid training run, and shared compute's return in Coder. |

The protocol draft is [NIP-PYLON](../../nips/openagents/NIP-PYLON.md):
provider beacons, buyer-signed service receipts, and recomputable pool
aggregates. Related plans live in [payments](../payments/README.md),
[sales](../sales/README.md), and the [Verse](../verse/README.md).
