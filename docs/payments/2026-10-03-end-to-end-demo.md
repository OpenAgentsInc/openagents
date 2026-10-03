# Payments end-to-end demo: the agent run (2026-10-03)

Issue [#10199](https://github.com/OpenAgentsInc/openagents/issues/10199),
design [section 8](2026-10-02-central-receive-and-splits.md#8-end-to-end-demo).
This records what an agent set up and verified on production, and what is
left for the owner, whose wallet and phone hold the real money. The owner's
steps are in the workspace `NEEDS_OWNER.md` ("Make the first real payments").

The demo is one script, [`scripts/payments-demo.sh`](../../scripts/payments-demo.sh):
quote, 402, payment from the owner's wallet, settlement, split, the live
flow, payout, reconciliation, printing where to look at each step.
`--quote-only` stops before any money moves; `--operator` also reads the
ledger, payout list, and a fresh reconciliation on `oa-pay-1` over IAP ssh.

## What was set up

| Piece | State |
| --- | --- |
| Receiver | `oa-pay-1`, node `0343a0f1…db32e27` on bitcoin with MoneyDevKit LSPS4. No channel yet: the first receive (the owner's 2,000-sat invoice) opens it. |
| Pay front | `openagents pay serve` as `openagents-pay-front.service` on `oa-pay-1`, route file [`deploy/pay/openagents-pay-routes.toml`](../../deploy/pay/openagents-pay-routes.toml): `POST /v1/plugins/{id}/invoke` at 5 sats plus the release's fee, the hosted `/x/{resource}` routes, `POST /v1/resources`, `GET /v1/paid-key`; settlements to the ledger. |
| Public URL | `api.openagents.com` load balancer (`one-production-url-map`, matcher `api`): route rules send `/v1/plugins/{id=*}/invoke`, `/x/*`, `/v1/resources`, and `/v1/paid-key` to backend `oa-pay-front-backend` (zonal NEG `oa-pay-front-neg`, `oa-pay-1:8402`, health check `oa-pay-front-hc` on `/v1/paid-key`); `/v1/sessions*` still goes to voice and everything else to the API service. Firewall `oa-pay-front-from-lb` admits only the load balancer ranges on 8402. |
| Relay | `relay.openagents.com` was refusing `release.fee_msat` (`unsupported_feature`): its image predated NIP-EXT G9. Rebuilt from `44edd848ed` and shifted to revision `openagents-nostr-relay-00041-fed` (no migrations; `next` smoke: health, NIP-11 key, REQ, a write read back through production, `PUT /upload` 401). Rollback: `00036-toy`. |
| Check plugin | `097201496ea4…b002d:explain-error-check` 0.1.0 (release `f59b6399…c275d`), published by the pay host's own key with `fee_msat = 10000` and the payout Spark wallet as `payout`, so the priced path could be checked end to end without inventing an author address. It is the same guest as `explain-error`. |
| Flow names | `pay-host` now names a registry plugin (`<publisher>:<slug>`) by its slug once its signed listing is registered (`OPENAGENTS_PAY_PUBLICATIONS`), so `/live` puts the dots on the committed `plugin-explain-error` node and the author shows by npub; unregistered plugins stay salted aliases. |
| Health and backups | Since `29f3ac33d6` moved the node's commands to `openagents x402 node`, `openagents-pay-health` and `openagents-pay-backup` had been calling the removed `wallet info` and `wallet backup` and failing; both scripts and `openagents-pay-restore` now use `x402 node`. Health is `healthy` and the hourly backup uploads again. |

## Verified (2026-10-03, no owner funds)

```text
$ scripts/payments-demo.sh --plugin explain-error-check --quote-only
== 1. Quote: POST https://api.openagents.com/v1/plugins/explain-error-check/invoke with no payment
   HTTP 402
   This call costs 15 sats (endpoint 5 sats + author fee 10 sats). ...
   part endpoint: 5000 msat
   part author_fee: 10000 msat
   payTo 0343a0f10d0856187ad55e8e64427b8902479ef510d9f5587ba6f124280db32e27
   network lnbtc:000000000019d6689c085ae165831e93
   amount 15000 msat
   invoice lnbc150n1p4vqm7d...
   www-authenticate: Payment id="...", realm="api.openagents.com", method...
== 2. Before: public totals
   {"totals":{"received_sats":0,"paid_out_sats":0,"pending_accruals_sats":0,"calls":2,"earnings_sats":0},"reconciliation":"ok"}
```

- The keyless call resolved the plugin on the relay, fetched and checked its
  release from the blob server, priced it at the endpoint plus the author's
  fee, and answered `402` with one mainnet invoice from the receiver, in
  x402 v2 and the HTTP `Payment` scheme.
- Each challenged call is written to the ledger's `call` table and appears
  on `openagents.com/api/flow/snapshot` as a `call` event within the
  flow server's poll, and `/stats` counts it (`calls` went 0, 1, 2).
- `/stats` reports `reconciliation: ok` (ledger empty, receiver and Spark
  wallets listed).

Not verified, because the receiver has no channel and no wallet here holds
owner funds: the paid retry, the settlement and its shares, the payout, and
reconciliation with money in it. `crates/x402` (`tests/front.rs`),
`crates/pay-ledger`, `crates/pay-host` (`tests/ledger.rs`), and
`openagents-cli`'s `pay_plugin` tests cover that path against fakes.

## The owner's run

After the 2,000-sat first receive opens the channel, the owner publishes
**Explain this error** under their own key with their Spark address
(`scripts/payments-demo.sh --publish --payout spark1… --operator`, or an
agent does it with the address they give), then runs the script; each paid
call is approved on the phone (`x402 fetch --pay-with phone`). Expected per
call: 15 sats received less the LSP's fee; shares author 10 sats, OpenAgents
the rest; on the plugin's first paid call the 1,000-sat bonus, funded only
out of OpenAgents' share (so `bonus_unfunded` beyond it); the launch match
up to the fee from the same share. The author's Spark payout goes out once
100 sats are owed or a day after the first share.

| Step | Time | Amount | Fee | Notes |
| --- | --- | --- | --- | --- |
| First receive (channel open) | | 2,000 sats | LSP | |
| Call 1 (`x402 fetch --pay-with phone`) | | 15 sats | | |
| Call 2 | | 15 sats | | |
| Payout to the author | | | | |
| Reconciliation | | | | |
