# Nexus Treasury

`nexus-control` now owns a hosted Spark treasury wallet beside the existing
provider-presence and receipt infrastructure. Operators can inspect wallet state
and generate fresh funding targets without touching wallet storage directly.

## Operator Surfaces

CLI:

```bash
cargo run -p nexus-control -- treasury status
cargo run -p nexus-control -- treasury funding-target
cargo run -p nexus-control -- treasury funding-target --amount-sats 2100 --description "fund nexus treasury"
```

HTTP:

- `GET /v1/treasury/status`
- `POST /v1/treasury/funding-target`

`treasury funding-target` uses the repo-owned Spark integration and returns the
current treasury Spark receive address, Bitcoin receive address, and an optional
Bolt11 invoice when an amount is requested.

## Runtime Configuration

The hosted treasury policy and wallet runtime are env-backed:

- `NEXUS_CONTROL_TREASURY_ENABLED`
- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW`
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS`
- `NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE`
- `NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS`
- `NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH`
- `NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR`
- `NEXUS_CONTROL_TREASURY_WALLET_NETWORK`
- `NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV`
- `NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS`
- `NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS`

## Public Stats

Public-safe treasury counters now project through `nexus-control /api/stats`:

- `nexus_wallet_runtime_status`
- `nexus_wallet_last_error`
- `nexus_wallet_balance_sats`
- `nexus_wallet_balance_updated_at_unix_ms`
- `nexus_treasury_enabled`
- `nexus_treasury_payout_sats_per_window`
- `nexus_treasury_payout_interval_seconds`
- `nexus_treasury_require_sellable`
- `nexus_treasury_daily_budget_cap_sats`
- `nexus_registered_payout_identities`
- `nexus_payout_sats_paid_total`
- `nexus_payout_sats_paid_24h`
- `nexus_payouts_dispatched_24h`
- `nexus_payouts_confirmed_24h`
- `nexus_payouts_failed_24h`
- `nexus_payouts_skipped_24h`
