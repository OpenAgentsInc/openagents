# TreasuryRouter

`TreasuryRouter` is the backend authority lane inside OpenAgents that cuts, signs, and settles accepted-work payouts — and the only component allowed to move sats on behalf of the kernel. It is not a UI, it is not a wallet, and it is not run from the Autopilot desktop. It is a server-side authority named in [ADR-0001](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md) as one of the two domain-scoped authorities for money-moving work (the other being the Kernel Authority API).

In the 2026-04-23 Autopilot earning proof, TreasuryRouter produced this payout id:

```
accepted_work:…:019db8a2-98d2-7890-95e4-6a1d78709a3c
```

That id reconciled exactly against the worker wallet's receive history (same `019db8a2-…` payment id, `25 sats`, `method: spark`, `status: completed`). That reconciliation is the whole point of TreasuryRouter: a payout is the _same object_ on the kernel side and the wallet side, with the same id, so no party has to trust anyone else's UI.

Internally, TreasuryRouter coordinates across Spark destinations (e.g. `spark1pgssyt9agft907ew09l6kndl59gtguccvpyuv6h90489ct7hm0drz7rzmswm7g`), Lightning rails, and on-chain paths. Status states include `confirmed` (accepted) and `settled` (reconciled); degraded-but-benign states like `treasury_degraded` on a `wallet_snapshot_stale` can surface without blocking payouts.

The open-treasury roadmap turns TreasuryRouter into a public, kernel-signed reporting surface so any holder can reconcile balances, payouts, and reserves without trusting a Treasury blob.

See also: [Chapter 7 — Economy Kernel](../../investors/07-economy-kernel.md), [Chapter 8 — Authority Model](../../investors/08-authority-model.md), [Chapter 9 — Proof Receipts](../../investors/09-proof-receipts.md).
