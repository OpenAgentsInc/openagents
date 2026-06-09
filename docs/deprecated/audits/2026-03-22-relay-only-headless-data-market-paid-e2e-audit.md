# 2026-03-22 Relay-Only Headless Data Market Paid E2E Audit

Date: 2026-03-22

## Scope

Validate the current DS-first Data Market launch path without `nexus-control`
or `OA_CONTROL_*`:

- publish DS listing `30404`
- publish DS offer `30406`
- publish DS-DVM request `5960`
- settle a priced Lightning payment
- publish DS access contract `30407`
- publish DS-DVM result `6960`
- consume the delivered payload locally from buyer-side relay state

## Commands run

Portable verifier:

```bash
scripts/autopilot/verify-data-market-cli-headless.sh
```

Fresh paid local proof:

```bash
OPENAGENTS_HEADLESS_DATA_MARKET_E2E_RUN_DIR=target/headless-data-market-e2e-relay-only-paid \
OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_IDENTITY_PATH=target/data-market-runs/alpha-juicy-tidbit-2026-03-22/headless-public-spark-buy/buyer-home/.openagents/pylon/identity.mnemonic \
OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_STORAGE_DIR=target/data-market-runs/alpha-juicy-tidbit-2026-03-22/headless-public-spark-buy/buyer-home/.openagents/pylon/spark/mainnet \
scripts/autopilot/headless-data-market-e2e.sh
```

Targeted regression coverage:

```bash
CARGO_INCREMENTAL=0 cargo test -p autopilot-desktop --lib relay_delivery_synthesis_uses_ds_result_when_kernel_delivery_state_is_empty -- --nocapture
```

## Result

Passed.

The portable verifier completed with:

- relay-only smoke publish proof
- relay-only zero-price local publish -> request -> delivery -> consume proof
- relay catalog link-hint regression coverage
- remote materialization coverage

The fresh priced local run also completed successfully with:

- buyer prefund from an existing Spark wallet
- seller-side `payment-required` feedback
- observed paid settlement
- DS access contract publication
- DS-DVM result publication
- buyer-side consume from relay-derived delivery metadata
- byte-for-byte payload match against the source dataset

## Key artifacts

Paid run summary:

- `target/headless-data-market-e2e-relay-only-paid/summary.json`

Buyer prefund status:

- `target/headless-data-market-e2e-relay-only-paid/buyer-wallet-status-prefunded.json`

Seller paid request snapshot:

- `target/headless-data-market-e2e-relay-only-paid/seller-payment-settled.json`

Verifier zero-price summary:

- `target/headless-data-market-e2e/summary.json`

## Bugs fixed during this audit

1. The relay-only harness still tried to publish buyer requests against the
   legacy asset id instead of the selected DS listing coordinate. The harness
   now uses the DS listing coordinate directly.
2. The headless scripts were blocked on a host-specific `autopilotctl` linker
   failure under incremental builds. The Data Market verification scripts now
   force `CARGO_INCREMENTAL=0`.
3. Buyer `consume-delivery` still assumed a local kernel `DeliveryBundle` row
   existed. Delivery resolution now synthesizes a compatible delivery payload
   from DS relay results and DS access contracts when the legacy row is absent.
4. The final E2E verifier still required buyer-side linked kernel asset/grant
   ids. It now accepts the relay-only DS-coordinate path where those linked ids
   are intentionally absent on the buyer side.

## Current operator truth

- The local launch gate is now relay-only.
- `nexus-control` is not required for the current headless seller/buyer flow.
- Priced local verification still requires a funded Spark payer wallet.
- Public-relay testing remains useful, but local relay verification is the
  portable release gate.
