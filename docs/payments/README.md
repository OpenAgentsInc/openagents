# Payments

How OpenAgents receives every payment centrally, splits it between plugin
authors, resource owners, and OpenAgents, pays people out, and shows the money
moving in real time. The pieces (#10185 to #10199) are built and run on
the pay host `oa-pay-1` ([deployment](../deployment/pay-host.md)); the
first real payments are the owner's.

| Document | What it covers |
| --- | --- |
| [Central receive, splits, payouts, and the live flow view (design, 2026-10-02)](2026-10-02-central-receive-and-splits.md) | The owner's direction; why the receiver is our own `crates/wallet` node on MoneyDevKit's LSPS4 liquidity and not the MDK treasury container; the architecture; who gets paid and the launch bonus; the ledger, split rules, and reconciliation; payout destinations and rails; the public flow stream, `/stats`, `/live`, and the `routes-live` deck scene; the end-to-end demo; risks; phases; the issues. |
| [End-to-end demo, the agent run (2026-10-03)](2026-10-03-end-to-end-demo.md) | `scripts/payments-demo.sh`, what was set up for it (the pay front behind `api.openagents.com`, the relay update, the check plugin), what was verified without owner funds, and the table for the owner's paid run. |

Related: [the OpenAgents API](../api/README.md) (x402 payment, plugin fees),
[Breez and Spark](../breez/README.md) (the user Wallet),
[Bitcoin](../bitcoin/README.md) (node history), and
[NIP-X402](../../nips/openagents/NIP-X402.md).
