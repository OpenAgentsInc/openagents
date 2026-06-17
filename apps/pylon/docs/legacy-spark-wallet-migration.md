# Legacy Spark Wallet Migration

Date: 2026-06-10

Issues:

- [#4672](https://github.com/OpenAgentsInc/openagents/issues/4672)
- [#4685](https://github.com/OpenAgentsInc/openagents/issues/4685)
- [#4686](https://github.com/OpenAgentsInc/openagents/issues/4686)

Pylon v0.2.5 included a `pylon wallet migrate-spark` compatibility path for
old Spark/Breez wallet balances. Some users can still have old spendable
balance secured by their 12-word identity mnemonic, but the old helper can fail
during initialization with `Missing Breez API key`.

The current v1.0 release-candidate behavior must not recommend a spend command
after only seeing old balance history. It first projects a public-safe preflight.

This release exposes the migration as a CLI JSON flow. It does not yet ship a
graphical wallet migration screen. The CLI projection includes a
`guidedRecovery` object with the user-facing answer, destination readiness,
consent state, next-step summary, and public-safe secret-handling refs.

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

When the preflight passes and the command runs with explicit consent, Pylon uses
the same #5169 sweep path as the Spark backup reconcile: create a fresh local
MDK receive target, pay it from the recovered Spark balance, then verify the
MDK balance increased before emitting
`receipt.pylon.spark_backup_reconcile.<digest>`. If the transfer is sent but
the MDK balance is not visible yet, the projection stays
`sweep-pending-mdk-credit` and must not be described as spendable.

If the old helper cannot initialize because the Breez/Spark credential is
missing, and the user has the local 12-word mnemonic, they should run a local
mnemonic recovery preflight:

```sh
pylon wallet migrate-spark --mnemonic-recovery --destination-invoice-ready
```

That mode keeps the mnemonic private. The flag only tells Pylon to use the
local recovery path; it does not print, upload, or ask for the phrase. The
command returns `consent-required` until the user reviews the local recovery
plan and reruns with `--yes --execute`.

## Breez / Spark Credential Behavior

User-facing answer: normal users should not manually add or paste a Breez API
key into support channels to recover an old Spark balance. If the helper says
`Missing Breez API key` and the user still has the 12-word recovery phrase, use
the local recovery flow on the user's machine:

```sh
pylon wallet migrate-spark --mnemonic-recovery --destination-invoice-ready
```

Pylon will keep returning `state: "consent-required"` until the destination is
prepared and the user explicitly reruns with `--yes --execute`.

If the helper reports `Missing Breez API key` and the user has not selected
local mnemonic recovery, Pylon returns:

- `blocker.wallet.legacy_spark.breez_api_key_missing`;
- `blocker.wallet.legacy_spark.helper_init_failed`;
- `action.wallet.legacy_spark.rerun_with_mnemonic_recovery_local_only` when an
  identity mnemonic is present; otherwise
- `action.wallet.legacy_spark.configure_supported_local_spark_credential`.

That is an actionable blocked state, not a recommendation to run
`migrate-spark --yes`. Normal users should not have to hunt through support
threads for raw API keys.

If the user selects `--mnemonic-recovery`, old balance is detected, and the
destination invoice is prepared, Pylon sets:

- `recoveryMode: "local-recovery"`;
- `mnemonicBackedRecoveryReady: true`;
- `state: "consent-required"` for dry-run/no-consent calls;
- `action.wallet.legacy_spark.review_private_local_recovery_plan`;
- `action.wallet.legacy_spark.review_and_confirm_migrate_spark_yes`.

Destination readiness remains load-bearing. `--mnemonic-recovery` is not send
readiness and does not prove outbound capacity; the new MDK destination must be
prepared before any migration can execute.

The guided projection uses:

- `guidedRecovery.localRecoveryAvailable` to show that local recovery is a
  supported path for the detected old balance;
- `guidedRecovery.localRecoverySelected` to show whether the local recovery
  path was selected for this preflight;
- `guidedRecovery.destinationState` to separate destination preparation from
  old balance detection;
- `guidedRecovery.consentState` to make the explicit consent step visible;
- `guidedRecovery.nextStepSummary` for concise CLI copy without exposing
  private material.

## Public-Safe Evidence

The migration projection may include public refs, counts, and redacted balance
numbers. It must not include:

- the 12-word mnemonic;
- Breez/Spark API keys;
- raw SDK state;
- raw invoices;
- payment preimages or payment hashes;
- raw Spark transfer targets;
- wallet home paths.

## Verification

Run from `apps/pylon`:

```sh
bun test tests/wallet.test.ts --max-concurrency=1
```
