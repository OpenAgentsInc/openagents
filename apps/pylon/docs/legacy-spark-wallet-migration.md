# Legacy Spark Wallet Migration

Date: 2026-06-10

Issue: [#4672](https://github.com/OpenAgentsInc/openagents/issues/4672)

Pylon v0.2.5 included a `pylon wallet migrate-spark` compatibility path for
old Spark/Breez wallet balances. Some users can still have old spendable
balance secured by their 12-word identity mnemonic, but the old helper can fail
during initialization with `Missing Breez API key`.

The current v0.3 behavior must not recommend a spend command after only seeing
old balance history. It first projects a public-safe preflight.

## Commands

Dry-run/preflight is the default:

```sh
pylon wallet migrate-spark
```

The preflight checks:

- whether legacy spendable Spark balance was detected;
- whether the helper can initialize;
- whether a Breez/Spark credential is available through the supported local
  helper path;
- whether the original identity mnemonic file exists or the user has selected a
  private mnemonic-recovery flow;
- whether the destination MDK invoice is ready.

The command only moves past preflight when every blocker is clear and the user
explicitly confirms:

```sh
pylon wallet migrate-spark --destination-invoice-ready --yes --execute
```

This compatibility command never asks users to paste a mnemonic into GitHub,
support threads, logs, or issue comments. If a user has only the 12-word phrase,
recovery must happen locally and privately on their machine.

## Breez / Spark Credential Behavior

If the helper reports `Missing Breez API key`, Pylon now returns:

- `blocker.wallet.legacy_spark.breez_api_key_missing`;
- `blocker.wallet.legacy_spark.helper_init_failed`;
- `action.wallet.legacy_spark.configure_bundled_spark_credential_or_wait_for_fix`.

That is an actionable blocked state, not a recommendation to run
`migrate-spark --yes`. Normal users should not have to hunt through support
threads for raw API keys. If the migration must depend on a Breez/Spark
credential, Pylon should surface the supported local setup path before any
migration recommendation is shown.

## Public-Safe Evidence

The migration projection may include public refs, counts, and redacted balance
numbers. It must not include:

- the 12-word mnemonic;
- Breez/Spark API keys;
- raw SDK state;
- raw invoices;
- payment preimages or payment hashes;
- wallet home paths.

## Verification

Run from `apps/pylon`:

```sh
bun test tests/wallet.test.ts --max-concurrency=1
```
