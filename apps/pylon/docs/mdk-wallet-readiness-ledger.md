# Pylon v0.3 MDK Wallet Readiness And Ledger

Status: implemented for `0.3.0-rc1` classifier, command wrappers, payout-target
admission refs, and local ledger events.

## Boundary

MDK owns wallet operations. Pylon owns launch-safe classification and public
projection:

- balance unknown/offline is not zero;
- receive readiness is not send readiness;
- send readiness is blocked unless explicit MDK evidence proves it;
- payout-target admission is a public-safe ref, not a raw invoice or offer;
- settlement claims require a settlement/ledger ref.

## CLI

```sh
pylon wallet status
pylon wallet report-readiness --base-url https://openagents.com
pylon wallet receive --amount 1000
pylon wallet send --destination-ref payout.bolt12.<hash> --amount 21
pylon wallet admit-payout-target --kind bolt12_offer --ref payout.bolt12.<hash>
pylon wallet request-payout-target-admission --base-url https://openagents.com --kind bolt12_offer --ref payout.bolt12.<hash>
```

Supported payout-target kinds:

- `bolt12_offer`
- `bolt11_invoice`
- `bip353_name`
- `lnurl_pay`

The command surface emits receipt refs. It does not print raw mnemonics,
preimages, raw invoices, raw offers, or raw payment material.

## Readiness States

Pylon classifies:

- `daemon-offline`
- `balance-unknown`
- `receive-ready`
- `send-ready`
- `send-ready-blocked`
- `payout-target-admitted`
- `payable-pending-settlement`
- `settlement-recorded`

Mnemonic-only restores, zero outbound capacity, and missing send-readiness
evidence produce `blocker.wallet.send_readiness_unproven`.

`pylon wallet status` also emits `sendReadinessPreflight`. Send readiness is
true only when MDK reports send readiness, balance is known, outbound capacity
is known and positive, the wallet is not a mnemonic-only restore, and
`MDK_WALLET_PORT` is explicitly set. Missing port isolation reports
`blocker.wallet.mdk_port_unset` because MDK daemon restarts can fall back to the
default port and cross-talk with the wrong wallet.

## Ledger

`ledger.jsonl` is append-only and idempotent by `eventId`. The current
implementation records public refs such as `wallet.mdk_receive_target.*`,
`wallet.payment.*`, `wallet.spark_backup_transfer.*`, and settlement refs. Raw
wallet/payment material is rejected by the public projection guard.
