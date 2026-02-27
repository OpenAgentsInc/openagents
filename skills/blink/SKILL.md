---
name: blink
description: Blink Lightning wallet for agents — balances, invoices, payments, QR codes, price conversion, and transaction history.
metadata:
  oa:
    project: blink
    identifier: blink
    version: "0.3.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
      - process:spawn
---

# Blink Skill

Bitcoin Lightning wallet operations via the Blink API. Enables agents to check balances, receive payments via invoices, send payments over Lightning, track transactions, and monitor prices.

## What is Blink?

Blink is a custodial Bitcoin Lightning wallet with a GraphQL API. Key concepts:
- **API Key** — authentication token (format: `blink_...`) with scoped permissions (Read, Receive, Write)
- **BTC Wallet** — balance denominated in satoshis
- **USD Wallet** — balance denominated in cents (stablecoin pegged to USD)
- **Lightning Invoice** — BOLT-11 payment request string (`lnbc...`) used to receive payments
- **Lightning Address** — human-readable address (`user@domain`) for sending payments without an invoice
- **LNURL** — protocol for interacting with Lightning services via encoded URLs

## Setup

Store your API key in `~/.profile`:
```bash
export BLINK_API_KEY="blink_..."
```

Get your API key from the [Blink Dashboard](https://dashboard.blink.sv) under API Keys.

**API Key Scopes:**
- **Read** — query balances, transaction history, price, account info
- **Receive** — create invoices
- **Write** — send payments (use with caution)

No npm dependencies required. Scripts use Node.js built-in `fetch` (Node 18+) and `WebSocket` (Node 22+ native, or Node 20+ with `--experimental-websocket` flag).

### Staging / Testnet

To use the Blink staging environment (signet), set:
```bash
export BLINK_API_URL="https://api.staging.blink.sv/graphql"
```

If not set, production (`https://api.blink.sv/graphql`) is used by default.

## Core Commands

### Check Wallet Balances
```bash
source ~/.profile && node {baseDir}/scripts/balance.js
```

Returns JSON with all wallet balances (BTC in sats, USD in cents), wallet IDs, pending incoming amounts, and a **pre-computed USD estimate** for the BTC wallet. Use `btcBalanceUsd` for the BTC wallet's USD value — do not calculate it yourself.

### Create Lightning Invoice (BTC)
```bash
source ~/.profile && node {baseDir}/scripts/create_invoice.js <amount_sats> [--timeout <seconds>] [--no-subscribe] [memo...]
```

Generates a BOLT-11 Lightning invoice for the specified amount in satoshis. Returns the `paymentRequest` string that can be paid by any Lightning wallet. The BTC wallet ID is resolved automatically.

**Auto-subscribe**: After creating the invoice, the script automatically opens a WebSocket subscription and waits for payment. It outputs **two JSON objects** to stdout:
1. **Immediately** — `{"event": "invoice_created", ...}` with `paymentRequest`, `paymentHash`, etc.
2. **When resolved** — `{"event": "subscription_result", "status": "PAID"|"EXPIRED"|"TIMEOUT", ...}`

The agent should read the first JSON to share the invoice/QR with the user right away, then wait for the second JSON to confirm payment.

- `amount_sats` — amount in satoshis (required)
- `--timeout <seconds>` — subscription timeout (default: 300). Use 0 for no timeout.
- `--no-subscribe` — skip WebSocket auto-subscribe, just create the invoice and exit
- `memo...` — optional description attached to the invoice (remaining args joined)

### Create Lightning Invoice (USD)
```bash
source ~/.profile && node {baseDir}/scripts/create_invoice_usd.js <amount_cents> [--timeout <seconds>] [--no-subscribe] [memo...]
```

Creates a Lightning invoice denominated in USD cents. The sender pays in BTC/Lightning, but the received amount is locked to a USD value at the current exchange rate. Credited to the USD wallet. **Expires in ~5 minutes** due to exchange rate lock.

**Auto-subscribe**: Same two-phase output as `create_invoice.js` — first JSON is the created invoice, second JSON is the payment resolution (PAID/EXPIRED/TIMEOUT).

- `amount_cents` — amount in USD cents, e.g. 100 = $1.00 (required)
- `--timeout <seconds>` — subscription timeout (default: 300). Use 0 for no timeout.
- `--no-subscribe` — skip WebSocket auto-subscribe, just create the invoice and exit
- `memo...` — optional description attached to the invoice (remaining args joined)

### Check Invoice Status
```bash
source ~/.profile && node {baseDir}/scripts/check_invoice.js <payment_hash>
```

Checks the payment status of a Lightning invoice by its payment hash. Use after creating an invoice to detect when it has been paid. Returns status: `PAID`, `PENDING`, or `EXPIRED`.

- `payment_hash` — the 64-char hex payment hash from `create_invoice.js` output (required)

### Pay Lightning Invoice
```bash
source ~/.profile && node {baseDir}/scripts/pay_invoice.js <bolt11_invoice>
```

Pays a BOLT-11 Lightning invoice from the BTC wallet. Returns payment status: `SUCCESS`, `PENDING`, `FAILURE`, or `ALREADY_PAID`. The BTC wallet ID is resolved automatically.

- `bolt11_invoice` — the BOLT-11 payment request string, e.g. `lnbc...` (required)

**Requires Write scope on the API key.**

### Pay to Lightning Address
```bash
source ~/.profile && node {baseDir}/scripts/pay_lnaddress.js <lightning_address> <amount_sats>
```

Sends satoshis to a Lightning Address (e.g. `user@blink.sv`). Returns payment status. The BTC wallet ID is resolved automatically.

- `lightning_address` — recipient in `user@domain` format (required)
- `amount_sats` — amount in satoshis (required)

**Requires Write scope on the API key.**

### Pay to LNURL
```bash
source ~/.profile && node {baseDir}/scripts/pay_lnurl.js <lnurl> <amount_sats>
```

Sends satoshis to a raw LNURL payRequest string. For Lightning Addresses (`user@domain`), use `pay_lnaddress.js` instead.

- `lnurl` — LNURL string, e.g. `lnurl1...` (required)
- `amount_sats` — amount in satoshis (required)

**Requires Write scope on the API key.**

### Estimate Payment Fee
```bash
source ~/.profile && node {baseDir}/scripts/fee_probe.js <bolt11_invoice>
```

### Render Invoice QR Code
```bash
source ~/.profile && node {baseDir}/scripts/qr_invoice.js <bolt11_invoice>
```

Renders a terminal QR code for a Lightning invoice (BOLT-11) to stderr and generates a **PNG image file** to `/tmp`. The stdout JSON includes a `pngPath` field with the absolute path to the PNG file.

**Sending the QR image to a user**: After running this script, use the `pngPath` from the JSON output to send the PNG as a media attachment to the user in the current chat. The agent should use its native message-send capability with the file path.

- `bolt11_invoice` — the BOLT-11 payment request string (required)

Output JSON includes:
- `invoice` — uppercased invoice string
- `qrRendered` — always `true`
- `qrSize` — QR module count
- `errorCorrection` — `"L"` (LOW)
- `pngPath` — absolute path to the generated PNG file (e.g. `/tmp/blink_qr_1234567890.png`)
- `pngBytes` — file size in bytes

Estimates the fee for paying a Lightning invoice without actually sending. Use before `pay_invoice.js` to check costs. Payments to other Blink users and direct-channel nodes are free (0 sats).

- `bolt11_invoice` — the BOLT-11 payment request string (required)

### List Transactions
```bash
source ~/.profile && node {baseDir}/scripts/transactions.js [--first N] [--after CURSOR] [--wallet BTC|USD]
```

Lists recent transactions (incoming and outgoing) with pagination. Returns direction, amount, status, type (lightning/onchain/intraledger), and metadata.

- `--first N` — number of transactions to return (default: 20, max: 100)
- `--after CURSOR` — pagination cursor from previous response's `endCursor`
- `--wallet BTC|USD` — filter to a specific wallet currency

### Get BTC/USD Price
```bash
source ~/.profile && node {baseDir}/scripts/price.js [amount_sats]
source ~/.profile && node {baseDir}/scripts/price.js --usd <amount_usd>
source ~/.profile && node {baseDir}/scripts/price.js --history <range>
source ~/.profile && node {baseDir}/scripts/price.js --currencies
```

Multi-purpose exchange rate tool. All price queries are **public (no API key required)**, though the key is sent if available.

**Modes:**
- **No args** — current BTC/USD price and sats-per-dollar rate
- **`<amount_sats>`** — convert a satoshi amount to USD (e.g. `price.js 1760` → `$1.20`)
- **`--usd <amount>`** — convert a USD amount to sats (e.g. `price.js --usd 5.00` → `7350 sats`)
- **`--history <range>`** — historical BTC price data with summary stats (high/low/change). Ranges: `ONE_DAY`, `ONE_WEEK`, `ONE_MONTH`, `ONE_YEAR`, `FIVE_YEARS`
- **`--currencies`** — list all supported display currencies (IDs, names, symbols, flags)

### Account Info
```bash
source ~/.profile && node {baseDir}/scripts/account_info.js
```

## Realtime Subscriptions

Blink supports GraphQL subscriptions over WebSocket using the `graphql-transport-ws` protocol. Node 20 requires the `--experimental-websocket` flag.

### Subscribe to Invoice Payment Status
```bash
source ~/.profile && node --experimental-websocket {baseDir}/scripts/subscribe_invoice.js <bolt11_invoice> [--timeout <seconds>]
```

Watches a single invoice and exits when it is **PAID** or **EXPIRED**. Status updates are printed to stderr. JSON result is printed to stdout.

### Subscribe to Account Updates (myUpdates)
```bash
source ~/.profile && node --experimental-websocket {baseDir}/scripts/subscribe_updates.js [--timeout <seconds>] [--max <count>]
```

Streams account updates in real time. Each event is output as a JSON line (NDJSON) to stdout. Use `--max` to stop after N events.

Shows account level, spending limits (withdrawal, internal send, convert), default wallet, and wallet summary with **pre-computed USD estimates** for BTC balances. Limits are denominated in USD cents with a rolling 24-hour window.

## API Reference

| Operation | GraphQL | Scope Required |
|-----------|---------|----------------|
| Check balance | `query me` + `currencyConversionEstimation` | Read |
| Create BTC invoice | `mutation lnInvoiceCreate` | Receive |
| Create USD invoice | `mutation lnUsdInvoiceCreate` | Receive |
| Check invoice | `query invoiceByPaymentHash` | Read |
| Pay invoice | `mutation lnInvoicePaymentSend` | Write |
| Pay LN address | `mutation lnAddressPaymentSend` | Write |
| Pay LNURL | `mutation lnurlPaymentSend` | Write |
| Fee estimate | `mutation lnInvoiceFeeProbe` | Read |
| Transactions | `query transactions` | Read |
| Price / convert | `query currencyConversionEstimation` | **None (public)** |
| Price history | `query btcPriceList` | **None (public)** |
| Currency list | `query currencyList` | **None (public)** |
| Realtime price | `query realtimePrice` | **None (public)** |
| Account info | `query me` + `currencyConversionEstimation` | Read |
| Subscribe invoice | `subscription lnInvoicePaymentStatus` | Read |
| Subscribe updates | `subscription myUpdates` | Read |

**API Endpoint:** `https://api.blink.sv/graphql` (production)
**Authentication:** `X-API-KEY` header

## Output Format

All scripts output structured JSON to stdout. Status messages and errors go to stderr. Exit code 0 on success, 1 on failure.

### Balance output example
```json
{
  "wallets": [
    { "id": "abc123", "currency": "BTC", "balance": 1760, "unit": "sats" },
    { "id": "def456", "currency": "USD", "balance": 1500, "unit": "cents" }
  ],
  "btcWalletId": "abc123",
  "btcBalance": 1760,
  "btcBalanceSats": 1760,
  "btcBalanceUsd": 1.2,
  "btcBalanceUsdFormatted": "$1.20",
  "usdWalletId": "def456",
  "usdBalance": 1500,
  "usdBalanceCents": 1500,
  "usdBalanceFormatted": "$15.00"
}
```

### Invoice creation output example (two-phase)
First JSON (immediate):
```json
{
  "event": "invoice_created",
  "paymentRequest": "lnbc500n1...",
  "paymentHash": "abc123...",
  "satoshis": 500,
  "status": "PENDING",
  "createdAt": "2026-02-23T00:00:00Z",
  "walletId": "abc123"
}
```
Second JSON (when payment resolves):
```json
{
  "event": "subscription_result",
  "paymentRequest": "lnbc500n1...",
  "status": "PAID",
  "isPaid": true,
  "isExpired": false,
  "isPending": false
}
```

### Invoice status output example
```json
{
  "paymentHash": "abc123...",
  "paymentStatus": "PAID",
  "satoshis": 500,
  "isPaid": true,
  "isExpired": false,
  "isPending": false
}
```

### Payment output example
```json
{
  "status": "SUCCESS",
  "walletId": "abc123",
  "balanceBefore": 50000
}
```

### Price output example
```json
{
  "btcPriceUsd": 68036.95,
  "satsPerDollar": 1470,
  "conversion": {
    "sats": 1760,
    "usd": 1.2,
    "usdFormatted": "$1.20"
  }
}
```

### USD-to-sats conversion output example
```json
{
  "btcPriceUsd": 68036.95,
  "satsPerDollar": 1470,
  "conversion": {
    "usd": 5.0,
    "usdFormatted": "$5.00",
    "sats": 7350
  }
}
```

### Price history output example
```json
{
  "range": "ONE_DAY",
  "dataPoints": 24,
  "summary": {
    "current": 68036.95,
    "oldest": 67500.00,
    "high": 68500.00,
    "low": 67200.00,
    "changeUsd": 536.95,
    "changePct": 0.8
  },
  "prices": [
    { "timestamp": 1740000000, "date": "2025-02-20T00:00:00.000Z", "btcPriceUsd": 67500.00 }
  ]
}
```

### Transaction list output example
```json
{
  "transactions": [
    {
      "id": "tx_123",
      "direction": "RECEIVE",
      "status": "SUCCESS",
      "amount": 1000,
      "currency": "BTC",
      "type": "lightning",
      "paymentHash": "abc...",
      "createdAt": 1740000000
    }
  ],
  "count": 1,
  "pageInfo": {
    "hasNextPage": false,
    "endCursor": "cursor_abc"
  }
}
```

## Typical Agent Workflows

### Receive a payment (recommended — auto-subscribe + QR image)
```bash
# 1. Create invoice — script auto-subscribes and outputs two JSON objects
source ~/.profile && node {baseDir}/scripts/create_invoice.js 1000 "Payment for service"
# → First JSON: {"event": "invoice_created", "paymentRequest": "lnbc...", ...}
# → Read paymentRequest from first JSON immediately

# 2. Generate QR code PNG
source ~/.profile && node {baseDir}/scripts/qr_invoice.js <paymentRequest>
# → JSON includes "pngPath": "/tmp/blink_qr_123456.png"
# → Send the PNG file to the user as a media attachment in the current chat

# 3. The create_invoice.js script is still running, waiting for payment
# → Second JSON: {"event": "subscription_result", "status": "PAID", ...}
# → When PAID: notify the user that payment has been received
# → When EXPIRED: notify the user the invoice expired
```

**Important**: The `create_invoice.js` script outputs two JSON objects separated by a newline. Parse them as separate JSON objects, not as a single JSON array. The first object arrives immediately; the second arrives when payment status resolves.

### Receive a payment (polling fallback)
```bash
# 1. Create invoice without auto-subscribe
source ~/.profile && node {baseDir}/scripts/create_invoice.js 1000 --no-subscribe "Payment for service"
# 2. Give the paymentRequest to the payer
# 3. Poll for payment
source ~/.profile && node {baseDir}/scripts/check_invoice.js <payment_hash>
# 4. Verify balance
source ~/.profile && node {baseDir}/scripts/balance.js
```

### Receive a USD payment
```bash
# Same two-phase pattern as BTC, but using create_invoice_usd.js
# Note: USD invoices expire in ~5 minutes
source ~/.profile && node {baseDir}/scripts/create_invoice_usd.js 500 "Five dollars for service"
# → First JSON: {"event": "invoice_created", "amountCents": 500, "amountUsd": "$5.00", ...}
# Generate QR and send to user, then wait for second JSON
```

### Receive a payment (standalone WebSocket subscription)
```bash
# If you need to subscribe to an existing invoice separately
source ~/.profile && node --experimental-websocket {baseDir}/scripts/subscribe_invoice.js <payment_request> --timeout 300
```

### Send a payment (with fee check)
```bash
# 1. Check current balance
node {baseDir}/scripts/balance.js
# 2. Estimate fee
node {baseDir}/scripts/fee_probe.js lnbc1000n1...
# 3. Send payment
node {baseDir}/scripts/pay_invoice.js lnbc1000n1...
# 4. Verify in transaction history
node {baseDir}/scripts/transactions.js --first 1
```

### Convert sats to USD value
```bash
# Check how much 1760 sats is worth in USD
node {baseDir}/scripts/price.js 1760
# → $1.20
```

### Convert USD to sats
```bash
# How many sats is $5.00?
node {baseDir}/scripts/price.js --usd 5.00
# → 7350 sats
```

### Check price history
```bash
# Get BTC price over the last 24 hours
node {baseDir}/scripts/price.js --history ONE_DAY
# Get BTC price over the last month
node {baseDir}/scripts/price.js --history ONE_MONTH
```

## Security Notes

- **API key is your wallet access** — anyone with a Write-scoped key can spend your balance
- **Use minimum scopes** — Read-only for balance checks, Receive for invoices, Write only when sending
- **Never expose keys in client-side code** — keys are for server-side / agent use only
- **Sending is irreversible** — Lightning payments cannot be reversed once sent
- **Test on staging first** — use `BLINK_API_URL` to point at the signet staging environment
- **USD invoices expire fast** — ~5 minutes due to exchange rate lock
- **Price queries are public** — `price.js` works without an API key; only wallet operations require authentication

## Files

- `{baseDir}/scripts/balance.js` — Check wallet balances
- `{baseDir}/scripts/create_invoice.js` — Create BTC Lightning invoices (auto-subscribes to payment status)
- `{baseDir}/scripts/create_invoice_usd.js` — Create USD-denominated Lightning invoices (auto-subscribes to payment status)
- `{baseDir}/scripts/check_invoice.js` — Check invoice payment status (polling)
- `{baseDir}/scripts/pay_invoice.js` — Pay BOLT-11 invoices
- `{baseDir}/scripts/pay_lnaddress.js` — Pay to Lightning Addresses
- `{baseDir}/scripts/pay_lnurl.js` — Pay to LNURL strings
- `{baseDir}/scripts/fee_probe.js` — Estimate payment fees
- `{baseDir}/scripts/qr_invoice.js` — Render invoice QR code (terminal + PNG file)
- `{baseDir}/scripts/transactions.js` — List transaction history
- `{baseDir}/scripts/price.js` — Get BTC/USD exchange rate
- `{baseDir}/scripts/account_info.js` — Show account info and limits
- `{baseDir}/scripts/subscribe_invoice.js` — Subscribe to invoice payment status (standalone)
- `{baseDir}/scripts/subscribe_updates.js` — Subscribe to realtime account updates
