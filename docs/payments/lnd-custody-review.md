# LND custody review brief (#11761)

Prepared for the owner's counsel review, October 16, 2026 (UTC). The LND
rollout remains experimental. The owner has not recorded a legal review outcome;
LND third-party payouts and mainnet asset trials remain disabled.

## Money flows and control

| Flow | What we hold or owe | Who can authorize movement |
| --- | --- | --- |
| Paid API, plugin, and hosted-resource calls | Bitcoin reaches the central receiver. The ledger records service revenue and amounts owed to authors, resource owners, affiliates, and Pylon providers under the applicable split rule. | OpenAgents operates the receiver and payout worker. Payees register destinations and hold claims; they do not hold the central node's keys. |
| Purchased agent or compute credits | A customer prepays for services. Unspent credits and unresolved holds require their own accounting, refund terms, and backing. | OpenAgents controls the service ledger and its funding wallets. Per-person embedded LDK nodes and personal Spark wallets remain separate. |
| Proposed LND payouts | Earnings leave our node for a payee's Lightning invoice, or fund the server's Spark wallet before a Spark transfer. An uncertain send stays reserved while its original payment is checked. | The worker receives a capped litd account. Node administrators retain broader authority. Receiver, read-only, and operator credentials are separate. |
| Proposed Taproot Assets receipts and payouts | A Bitcoin asset moves through an RFQ edge into a fresh integrated litd node. We record its units and quote, then credit the service balance at that saved BTC rate. Holding the asset against a BTC-valued credit creates a valuation and liquidity question for counsel and accounting. | OpenAgents operates the asset node. The issuer and edge have separate roles. Asset ID, issuer, real price oracle, redemption rights, and payout approval are still unselected. |

The current central receiver is our Rust LDK wallet using MDK's LSPS4 liquidity;
it is separate from the campaign treasury. The proposed LND node uses a fresh
seed on our GCP host, with encrypted backups. Production remote signing has not
been deployed by this work. A remote signer changes where signing occurs;
counsel should review who can control, recover, freeze, or restore funds across
the node, signer, cold wallet, backups, and cloud administrators.

## Amounts and evidence

The active front, payout worker, and flow service were checked against the same
central SQLite ledger on `oa-pay-1`. A read-only transaction at
**2026-10-16 01:58:08 UTC** found **0 settlements, 0 payouts, 0 recorded
unreserved third-party claims, and 0 msat of legacy prepaid balance**.
Compute-credit and asset-receipt tables were absent. API credit books,
consolidated wallet assets, the treasury, and off-ledger obligations were outside
this export. The owner has the private JSON attachment
`lnd-custody-ledger-snapshot.json`; refresh it and supply those additional books
before review. This is a scoped ledger snapshot, not consolidated accounts.

The experimental LND node's saved read-only liquidity report also showed zero
hot, confirmed, inbound, and outbound sats on October 16. No production asset
node was started. The asset drill's 10,000 units and 10,000,000-msat credit are
**regtest evidence with a mock price**, and have no mainnet value.

The [plan](2026-10-15-lnd-sdk-plan.md#liquidity-capital) proposes 5–10 million
sats of inbound capacity, 2–5 million sats of outbound seed capital, and
100,000–500,000 sats of confirmed reserves. These are proposals, not approved
amounts; inbound capacity is distinct from our capital and LSP fees. The hot
cap, cold address, LND worker allowance, and mainnet asset trial budget remain
unset. The staged payout policy selects MDK and gives LND no allowance.

## Jurisdictions and questions for counsel

The deployment is in GCP `us-central1`; that does not establish the business's
or a customer's jurisdiction. The owner must supply the contracting entity,
formation and operating locations, customer and payee countries and states,
target markets, transaction volumes, and refund or redemption promises. Ledger
IDs and wallet destinations do not establish residency. Keep each credit book's
currency and scale: compute balances use msat; the API monetary ledger uses
millionths of its configured currency.

Ask counsel to address custody and beneficial ownership, third-party payment
and exchange activity, registration or licensing in each relevant jurisdiction,
customer-fund segregation and insolvency treatment, refunds and complaints,
AML and sanctions controls, and the asset issuer's redemption and transfer
restrictions. For US activity, review the published
[FinCEN CVC guidance](https://www.fincen.gov/sites/default/files/2019-05/FinCEN%20Guidance%20CVC%20FINAL%20508.pdf)
and [OFAC virtual-currency guidance](https://ofac.treasury.gov/media/913571/download?inline)
alongside the requirements current when counsel reviews the actual business.
No legal classification or exemption is asserted here.

The owner records the review date, covered entities, jurisdictions and flows,
approved limits, required controls, and conditions for another review in
[#11761](https://github.com/OpenAgentsInc/openagents/issues/11761). Keep legal
advice and customer information private. Record the approval before enabling
P2 payouts or a mainnet asset trial; completion of this engineering brief grants
no spending approval.

Sources: [central receive and splits](2026-10-02-central-receive-and-splits.md),
[pay-host deployment](../deployment/pay-host.md),
[rollout and asset runbooks](lnd-host.md),
[compute balances](../../crates/pay-ledger/src/compute/hold.rs), and
[API monetary accounting](../decision-models/service/monetary-ledger.md).
