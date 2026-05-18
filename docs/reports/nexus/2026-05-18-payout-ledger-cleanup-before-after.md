# Nexus Payout Ledger Cleanup Before/After Report

Date: 2026-05-18

Issue: `#4506`

## Purpose

Separate current LDK payout health from historical payout rows that cannot be
paid through the LDK-only Nexus/Pylon path.

The intended end state is:

- failed rows with non-LDK or unknown targets are historical audit records, not
  active LDK payout failures;
- failed rows with LDK-compatible targets remain visible as current LDK
  attention or retryable pending state;
- `accepted_work_pending_payout_count` stays zero unless fresh accepted work is
  actually waiting for dispatch or confirmation.

## Production Baseline Before Code Change

The live audit taken earlier on 2026-05-18 reported:

| Metric | Value |
| --- | ---: |
| failed payout records | `90` |
| accepted-work payout records needing attention | `86` |
| failed payouts in latest 24h snapshot | `38` |
| accepted-work pending payout count | `0` |
| payout backlog retryable count | `0` |

Failure buckets in that snapshot were:

| Reason | Count | Total sats |
| --- | ---: | ---: |
| `retired_unpayable_non_ldk_payout_record` | `9` | `171` |
| `treasury_provider_error:ldk:invalid_request:unsupported_ldk_payment_target_kind:unknown` | `29` | `1245` |

Before this change, the broad ledger summary mixed those historical rows into
the same failed/attention counters as current LDK failures. That made the fresh
LDK proof look dirtier than it was and made future deploy health harder to
interpret.

## Code-Level After State

`nexus-control treasury payout-ledger-cleanup` now produces an explicit cleanup
report with:

- before and after `training_payout_ledger_summary`;
- disposition counts for all payout rows;
- reason counts for failed/skipped rows;
- the list of rows retired when `--apply` is used.

The summary now separates:

- `current_ldk_failed_payout_count`
- `current_ldk_attention_payout_count`
- `retired_historical_payout_count`
- `retired_historical_accepted_work_payout_count`
- `retired_historical_payout_sats`

Only failed rows with LDK-compatible targets (`bolt12_offer`, `bolt11_invoice`,
`bip353_name`, or `lnurl_pay`) can be treated as retryable pending payouts.
Failed rows with old provider-style targets are not re-entered into dispatch.

## Operator Commands

Dry-run first:

```bash
nexus-control treasury payout-ledger-cleanup \
  --report-path /var/lib/nexus-relay/payout-ledger-cleanup-dry-run.json \
  --json
```

Apply:

```bash
nexus-control treasury payout-ledger-cleanup \
  --apply \
  --report-path /var/lib/nexus-relay/payout-ledger-cleanup-apply.json \
  --json
```

Then verify:

```bash
nexus-control treasury status --json | jq '.training_payout_ledger_summary | {
  reconciliation_status,
  accepted_work_pending_payout_count,
  current_ldk_attention_payout_count,
  retired_historical_payout_count,
  retired_historical_accepted_work_payout_count,
  retired_historical_payout_sats
}'
```

Expected post-apply interpretation:

- `accepted_work_pending_payout_count` remains `0` unless new accepted work is
  waiting.
- `current_ldk_attention_payout_count` describes active LDK payout problems.
- `retired_historical_payout_count` describes rows intentionally kept for
  audit but removed from current LDK failure attention.

## Verification

Code-level verification in this commit:

```bash
cargo test -p nexus-control payout_ledger_cleanup --lib
cargo test -p nexus-control failed_non_ldk_records_are_historical_cleanup_not_current_ldk_attention --lib
cargo test -p nexus-control retryable_failed_accepted_work_counts_as_pending_in_training_summary --lib
cargo test -p nexus-control retryable_failed_availability_payout_counts_as_pending_not_attention --lib
cargo test -p nexus-control failed_accepted_work_retry_claim_requires_ldk_target_and_placeholder_disable --lib
cargo test -p nexus-control retryable_failed_availability_dispatch_is_capped_per_cycle --lib
cargo test -p nexus-control retryable_failed_backlog_counts_exclude_non_retryable_and_non_ldk_failures --lib
cargo check -p nexus-control
bash scripts/deploy/nexus/test-ldk-deploy-invariants.sh
git diff --check
```

These tests prove that historical non-LDK failures are not retried, LDK
retryable failures still count as pending, and the cleanup report writes
before/after evidence.
