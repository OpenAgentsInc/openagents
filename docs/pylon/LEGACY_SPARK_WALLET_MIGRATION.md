# Legacy Spark Wallet Migration

Pylon v0.2 uses MoneyDevKit as the default wallet runtime. Older v0.1 Pylon
builds used a Spark wallet. Those are separate wallet runtimes, so an old Spark
balance is not part of the new MDK wallet state just because the same Pylon
identity mnemonic still exists.

Starting with `pylon-v0.2.5`, Pylon automatically detects retained wallet
history rows whose payment method is `spark` and labels the current wallet
balance as the live MDK balance. It does not silently spend old Spark funds on
startup or status refresh. Moving value from Spark into MDK requires creating a
fresh MDK Lightning invoice and paying it from the old Spark wallet.

## Normal Path

Update to `pylon-v0.2.5` or newer, then run:

```bash
pylon wallet status
pylon wallet migrate-spark
pylon wallet migrate-spark --yes
```

The first command shows the live MDK balance and any retained Spark history. The
second command is a dry run: it checks the old Spark wallet, reports the
spendable Spark balance, and blocks if anything needs attention. The third
command creates a Pylon-scoped MDK invoice, pays it from the old Spark wallet,
polls for the MDK balance increase, and records a redacted local migration
receipt.

The standalone `pylon-v0.2.5` release archive includes `spark-wallet-cli` beside
`pylon`, so most direct-release users do not need to install anything else. If
the helper is somewhere else, point Pylon at it:

```bash
PYLON_LEGACY_SPARK_WALLET_CLI=/absolute/path/to/spark-wallet-cli \
  pylon wallet migrate-spark --yes
```

## What The User Has To Do

In the common case, the only user action is:

```bash
pylon wallet migrate-spark --yes
```

Do not re-enter the 12-word mnemonic for the normal migration. Pylon uses the
existing local Pylon identity path that the old Spark wallet used.

Keep the old mnemonic and old local wallet directory backed up until the command
reports `status: completed` and `wallet balance` shows the expected live MDK
balance.

## Blocked Cases

If `wallet migrate-spark` reports that the helper is missing, install or build
the old helper and set `PYLON_LEGACY_SPARK_WALLET_CLI`.

If it reports unclaimed deposits, claim those deposits with the legacy helper
first, then rerun the migration. Pylon does not auto-claim deposits because that
requires deposit-specific transaction inputs and fee policy.

Automatic sweeping is enabled only for mainnet Pylon wallets. Pylon refuses
testnet/signet-style sweeps because the old Spark helper and the new MDK invoice
would not be on a safely matching payment network.

## Why This Is Not Fully Silent

An old Spark balance can be moved into MDK, but it cannot be ported by copying
state. The safe migration is a real Lightning payment from the old Spark wallet
to a new MDK invoice. Pylon therefore detects the issue automatically and makes
the sweep one confirmed command, rather than spending funds during a normal
startup, status check, or TUI refresh.
