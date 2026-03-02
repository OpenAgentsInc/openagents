# Swap Operations

Use this reference for deterministic BTC <-> USD wallet conversion in the Blink skill.

## Source Of Truth

- `blink/scripts/swap_quote.js`
- `blink/scripts/swap_execute.js`
- `blink/scripts/_swap_common.js`
- Blink GraphQL public schema (`currencyConversionEstimation`, `intraLedgerPaymentSend`, `intraLedgerUsdPaymentSend`)

## Supported Directions

- `btc-to-usd` (`sell-btc`, `buy-usd`)
- `usd-to-btc` (`sell-usd`, `buy-btc`)

## Quote Script

```bash
node swap_quote.js <direction> <amount> [--unit sats|cents] [--ttl-seconds N] [--immediate]
```

What it does:
- Reads both wallet balances before execution (`preBalance`).
- Uses `currencyConversionEstimation` to build quote terms.
- Returns deterministic metadata:
  - `quoteId`
  - `expiresAtEpochSeconds`
  - `immediateExecution`
  - `amountIn` / `amountOut`
  - `executionPath`

Notes:
- Quote IDs are local deterministic IDs for audit/replay tracking.
- These are not settlement guarantees; execution can still fail due to policy/limits/balance changes.
- Default quote TTL is 60 seconds; for autonomous loops, re-quote before each execution.

## Execute Script

```bash
node swap_execute.js <direction> <amount> [--unit sats|cents] [--dry-run] [--memo "text"]
```

Execution path by direction:
- `btc-to-usd` -> `intraLedgerPaymentSend`
  - Sender: BTC wallet
  - Recipient: USD wallet
  - Amount unit sent: sats
- `usd-to-btc` -> `intraLedgerUsdPaymentSend`
  - Sender: USD wallet
  - Recipient: BTC wallet
  - Amount unit sent: cents

Receipt fields:
- `preBalance`
- `postBalance`
- `balanceDelta`
- `quote` (the exact quote terms used)
- `status` (`SUCCESS`, `PENDING`, `FAILURE`, `ALREADY_PAID`, or `DRY_RUN`)
- `execution.transactionId` when available

## Units, Rounding, And Effective Cost

Conversion is always between integer units:
- BTC wallet in `sats`
- USD wallet in `cents`

Because both sides are integers, settlement often differs slightly from quoted output.

- `btc-to-usd`: common outcome is `actual_usd_cents = quoted_usd_cents - 1`
- `usd-to-btc`: common outcome is `actual_btc_sats = quoted_btc_sats - 1`

Live wallet runs on March 2, 2026 showed:
- `quote.feeSats = 0`, `quote.feeBps = 0`, `quote.slippageBps = 0`
- occasional 1-unit quote-to-settlement rounding differences as above

Treat these as execution rounding spread, not explicit fee fields.

Recommended effective-cost calculation:
- `btc-to-usd`: `effective_cost_cents = quote.amountOut.value - balanceDelta.usdDeltaCents`
- `usd-to-btc`: `effective_cost_sats = quote.amountOut.value - balanceDelta.btcDeltaSats`

## Failure Modes

Common execution failures to handle in loops:
- `INSUFFICIENT_BALANCE`
- `INVALID_INPUT` (amount below viable conversion threshold)

Operational rule:
1. Quote.
2. Execute.
3. Verify `status`.
4. Confirm settlement from `postBalance`/`balanceDelta`.
5. Re-quote before retrying.

## Fallback Behavior

If swap execution is unavailable (mutation rejected or policy error), the scripts:
- Return a non-zero exit code.
- Emit a machine-readable error string to stderr.
- Preserve quote metadata in output paths where possible for diagnosis/replay.

Recommended fallback:
1. Re-run `swap_quote.js` to refresh terms.
2. Check `account_info.js` limits and balances.
3. Retry `swap_execute.js` with adjusted amount.
