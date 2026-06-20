Advance `provider.compliant_usage_labor.v1` by addressing `blocker.product_promises.labor_self_serve_earning_missing`.

Built `labor-self-serve-earning-payout.ts` and `labor-self-serve-earning-payout-routes.ts` in `workers/api` to implement the self-serve planner and route for withdrawing labor earnings to a Lightning wallet.
- Reads `bitcoinWithdrawableMsat` from the existing `agent_balances` ledger.
- Plans a `NexusTreasuryPayoutIntentRecord` with `sourceKind: 'accepted_work'` covering the available withdrawable balance.
- Exposed at `POST /api/public/labor-earnings/payout` under `AgentBalanceAuth`.
- Left FLAG-GATED INERT (`LABOR_SELF_SERVE_PAYOUT_ENABLED`) via `config.ts` so it plans but moves no money.
- Specifically drops `blocker.product_promises.labor_self_serve_earning_missing`.

Green still requires the deployment of this flow, actual un-gated ladder-settled receipts, and owner sign-off per `proof.claim_upgrade_receipts.v1`.
