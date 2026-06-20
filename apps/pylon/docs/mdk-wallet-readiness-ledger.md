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

## Daemon Port Conflict (`MDK_WALLET_PORT`)

The MDK agent-wallet daemon binds a TCP port to serve wallet commands. When
`MDK_WALLET_PORT` is unset it falls back to the default port **3456**, which
collides with other common local Bitcoin tools — e.g. the Orange wallet webhook
also listens on `:3456` (reported in #5505). On a fresh machine that already
runs such a tool, the daemon's bind fails with an `EADDRINUSE`-class error.

To make that first-run failure actionable instead of opaque, Pylon classifies
the daemon's command output with `describeMdkPortConflict` (`src/wallet.ts`),
mirroring the control-port guidance in `formatNodeStartupError`. On a detected
bind conflict it surfaces a clear remediation:

```
MDK wallet daemon could not start: port 3456 is already in use.
Another local service (e.g. an Orange wallet webhook) is already bound to 127.0.0.1:3456, the MDK default.
Set a free port and rerun, e.g.:
  MDK_WALLET_PORT=3458 pylon
Or stop the process already holding 3456 before starting the wallet daemon.
```

Set `MDK_WALLET_PORT` to any free port to avoid the collision entirely; this is
also what clears `blocker.wallet.mdk_port_unset` in the send-readiness preflight
above. The classifier emits only the env-var name, the numeric port, and a
redacted stable ref — never any wallet secret — so it is public-projection safe.

## Daemon Pidfile Recovery

Before invoking `@moneydevkit/agent-wallet`, Pylon now checks the local
`~/.mdk-wallet/daemon.pid` file. A pidfile is honored only when it names a live
process. If the file is malformed or points at a dead PID, Pylon removes it and
lets the MDK wallet command start normally.

This is deliberately narrow:

- live PIDs are left untouched;
- PIDs that exist but cannot be signaled because of permissions are treated as
  live;
- only the MDK agent-wallet pidfile path is reclaimed;
- no wallet material, payment ids, invoices, hashes, preimages, or mnemonics are
  read or projected.

Regression coverage lives in `apps/pylon/tests/wallet.test.ts`
(`reclaimStaleMdkDaemonPidfile`).

## Ledger

`ledger.jsonl` is append-only and idempotent by `eventId`. The current
implementation records public refs such as `wallet.mdk_receive_target.*`,
`wallet.payment.*`, `wallet.spark_backup_transfer.*`, and settlement refs. Raw
wallet/payment material is rejected by the public projection guard.
