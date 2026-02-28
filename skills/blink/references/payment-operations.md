# Payment Operations

Use this reference for sending payments and estimating fees with the Blink skill.

## Source Of Truth

- Blink API docs: https://dev.blink.sv
- `blink/scripts/pay_invoice.js`
- `blink/scripts/pay_lnaddress.js`
- `blink/scripts/pay_lnurl.js`
- `blink/scripts/fee_probe.js`

## Payment Methods

Three ways to send Lightning payments:

| Method | Script | Input | Use When |
|--------|--------|-------|----------|
| BOLT-11 invoice | `pay_invoice.js` | `lnbc...` string | Recipient gave you an invoice |
| Lightning Address | `pay_lnaddress.js` | `user@domain` + amount | Recipient has a Lightning Address |
| LNURL | `pay_lnurl.js` | `lnurl1...` + amount | Recipient gave you an LNURL payRequest |

All three support the `--wallet BTC|USD` flag to choose which wallet to send from.

## BTC vs USD Wallet Selection

The `--wallet` flag controls which wallet is debited:

```bash
# Pay from BTC wallet (default)
node pay_invoice.js lnbc1000n1... --wallet BTC

# Pay from USD wallet
node pay_invoice.js lnbc1000n1... --wallet USD
```

**How it works under the hood:**
- Scripts query `me { defaultAccount { wallets { id walletCurrency } } }` to get both wallet IDs
- The selected wallet's ID is passed to the GraphQL mutation
- The same `lnInvoicePaymentSend` mutation handles both BTC and USD wallets
- For `lnAddressPaymentSend` and `lnurlPaymentSend`, the amount is always in satoshis regardless of wallet type
- When a USD wallet ID is provided, the Blink API automatically converts and debits the USD equivalent

## Fee Estimation

Always probe fees before sending, especially for non-trivial amounts:

```bash
# BTC wallet fee probe
node fee_probe.js lnbc1000n1...

# USD wallet fee probe (uses lnUsdInvoiceFeeProbe mutation)
node fee_probe.js lnbc1000n1... --wallet USD
```

**Important**: BTC and USD wallets use different fee probe mutations:
- BTC: `lnInvoiceFeeProbe` — estimates routing fee in sats
- USD: `lnUsdInvoiceFeeProbe` — estimates routing fee from USD wallet perspective

Fee probe results:
- `0 sats` — intraledger (both sender and receiver are Blink users) or direct channel peer
- `> 0 sats` — routing fee for multi-hop payment
- Error — no route found (payment will likely fail too)

## Payment Status Values

| Status | Meaning | Action |
|--------|---------|--------|
| `SUCCESS` | Payment delivered | Done |
| `PENDING` | In flight, not yet settled | Wait and check transactions |
| `FAILURE` | Payment failed | Check error, possibly retry |
| `ALREADY_PAID` | Invoice was already paid | No action needed |

## Standard Send Workflow

1. **Check balance** — ensure sufficient funds in the chosen wallet
2. **Probe fee** — estimate the routing cost
3. **Confirm** — verify balance covers amount + fee
4. **Send** — execute the payment
5. **Verify** — check transaction history for confirmation

```bash
# Full workflow example (BTC wallet)
source ~/.profile
node balance.js
node fee_probe.js lnbc5000n1...
node pay_invoice.js lnbc5000n1...
node transactions.js --first 1
```

```bash
# Full workflow example (USD wallet)
source ~/.profile
node balance.js
node fee_probe.js lnbc5000n1... --wallet USD
node pay_invoice.js lnbc5000n1... --wallet USD
node transactions.js --first 1 --wallet USD
```

## Lightning Address Sends

```bash
# Send 1000 sats to a Lightning Address (BTC wallet)
node pay_lnaddress.js user@blink.sv 1000

# Send 1000 sats to a Lightning Address (USD wallet)
# Amount is in sats; Blink debits USD equivalent
node pay_lnaddress.js user@blink.sv 1000 --wallet USD
```

Lightning Address sends do not require a fee probe — the Blink API resolves the address, fetches an invoice, and pays it in one step.

## LNURL Sends

```bash
# Send 1000 sats via LNURL (BTC wallet)
node pay_lnurl.js lnurl1... 1000

# Send 1000 sats via LNURL (USD wallet)
node pay_lnurl.js lnurl1... 1000 --wallet USD
```

For Lightning Addresses (`user@domain`), prefer `pay_lnaddress.js` over `pay_lnurl.js`. Use `pay_lnurl.js` only for raw LNURL strings.

## Safety Guardrails

- **Always check balance first** — payments fail with `INSUFFICIENT_BALANCE` if funds are short
- **Probe fees for large payments** — routing fees can vary; intraledger is always free
- **Write scope required** — all send operations need a Write-scoped API key
- **Payments are irreversible** — Lightning payments cannot be reversed once settled
- **Test on staging** — use signet staging environment for development and testing
- **USD wallet amounts are in sats** — for LN address and LNURL sends, the amount parameter is always satoshis even when paying from USD wallet
