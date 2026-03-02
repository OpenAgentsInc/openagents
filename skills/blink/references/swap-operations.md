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

## Fallback Behavior

If swap execution is unavailable (mutation rejected or policy error), the scripts:
- Return a non-zero exit code.
- Emit a machine-readable error string to stderr.
- Preserve quote metadata in output paths where possible for diagnosis/replay.

Recommended fallback:
1. Re-run `swap_quote.js` to refresh terms.
2. Check `account_info.js` limits and balances.
3. Retry `swap_execute.js` with adjusted amount.
