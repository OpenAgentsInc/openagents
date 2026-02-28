# Invoice Lifecycle

Use this reference for creating, monitoring, and managing Lightning invoices with the Blink skill.

## Source Of Truth

- Blink API docs: https://dev.blink.sv
- `blink/scripts/create_invoice.js`
- `blink/scripts/create_invoice_usd.js`
- `blink/scripts/check_invoice.js`
- `blink/scripts/subscribe_invoice.js`
- `blink/scripts/qr_invoice.js`

## Invoice Types

| Type | Script | Amount Unit | Credited To | Expiration |
|------|--------|-------------|-------------|------------|
| BTC | `create_invoice.js` | satoshis | BTC wallet | Standard (hours) |
| USD | `create_invoice_usd.js` | cents | USD wallet | ~5 minutes (exchange rate lock) |

BTC invoices are denominated in satoshis. USD invoices lock an exchange rate at creation time, so the sender pays in Lightning (BTC) but the receiver gets a fixed USD amount credited to their USD wallet.

## Two-Phase Output Pattern

Both `create_invoice.js` and `create_invoice_usd.js` output **two separate JSON objects** to stdout:

**Phase 1 — Immediate** (invoice created):
```json
{"event": "invoice_created", "paymentRequest": "lnbc...", "paymentHash": "abc123...", "satoshis": 1000, "status": "PENDING"}
```

**Phase 2 — Resolution** (payment settles or expires):
```json
{"event": "subscription_result", "status": "PAID", "isPaid": true}
```

The agent should:
1. Parse the first JSON immediately to get `paymentRequest` and share it with the user
2. Optionally generate a QR code with `qr_invoice.js`
3. Wait for the second JSON to confirm payment status

**Important**: These are two separate JSON objects separated by a newline, not a JSON array. Parse them independently.

## Monitoring Strategies

### Strategy 1: Auto-Subscribe (Recommended)

The default behavior. `create_invoice.js` opens a WebSocket subscription after creating the invoice and blocks until PAID, EXPIRED, or timeout.

```bash
# Creates invoice and auto-subscribes (blocks until resolved)
node create_invoice.js 1000 "Payment for service"
```

Pros: Single command, real-time notification, no polling overhead.
Cons: Blocks the process until resolution.

### Strategy 2: No-Subscribe + Polling

Use `--no-subscribe` to create the invoice and exit immediately, then poll with `check_invoice.js`.

```bash
# Create without subscribing
node create_invoice.js 1000 --no-subscribe "Payment for service"
# → Single JSON output with paymentRequest and paymentHash

# Poll for status
node check_invoice.js <payment_hash>
# Repeat until status is PAID or EXPIRED
```

Pros: Non-blocking, can do other work between polls.
Cons: Adds latency, uses more API calls.

### Strategy 3: Standalone Subscription

Use `subscribe_invoice.js` to watch an existing invoice via WebSocket.

```bash
# Create invoice first (no-subscribe)
node create_invoice.js 1000 --no-subscribe "Payment for service"

# Subscribe separately
node subscribe_invoice.js <bolt11_invoice> --timeout 600
```

Pros: Decouples creation from monitoring, configurable timeout.
Cons: Requires Node 22+ (or Node 20+ with `--experimental-websocket`).

## Invoice Status Values

| Status | Meaning | Terminal? |
|--------|---------|----------|
| `PENDING` | Created, awaiting payment | No |
| `PAID` | Payment received and settled | Yes |
| `EXPIRED` | TTL exceeded, no payment received | Yes |

## QR Code Generation

After creating an invoice, generate a QR code for the payer:

```bash
node qr_invoice.js <paymentRequest>
```

Output:
- **Terminal QR** rendered to stderr (for visual verification)
- **PNG file** written to `/tmp/blink_qr_<timestamp>.png`
- **JSON to stdout** with `pngPath` field pointing to the PNG file

The agent should use the `pngPath` to send the QR image to the user as a media attachment.

## Recommended Receive Workflow

```bash
# 1. Create invoice (auto-subscribes)
node create_invoice.js 1000 "Service payment"
# → Read first JSON: get paymentRequest

# 2. Generate QR (while invoice script is still running)
node qr_invoice.js <paymentRequest>
# → Send PNG to user

# 3. Wait for second JSON from create_invoice.js
# → PAID: confirm to user
# → EXPIRED: notify user, offer to create new invoice
```

For USD invoices, the flow is identical but use `create_invoice_usd.js` with amount in cents. Note the ~5 minute expiration due to exchange rate lock.

## USD Invoice Specifics

- Amount is in **cents** (e.g., 500 = $5.00)
- Exchange rate is locked at creation time
- Sender pays in Lightning (sats), receiver gets USD credited
- Short expiration (~5 minutes) due to rate volatility
- Credited to the USD wallet, not BTC wallet

```bash
# Create a $5.00 USD invoice
node create_invoice_usd.js 500 "Five dollars"
```

## Timeout Configuration

```bash
# Default timeout: 300 seconds (5 minutes)
node create_invoice.js 1000

# Custom timeout: 10 minutes
node create_invoice.js 1000 --timeout 600

# No timeout (wait indefinitely)
node create_invoice.js 1000 --timeout 0
```

For USD invoices, set timeout to match or be shorter than the ~5 minute invoice expiration.

## WebSocket Requirements

- **Node 22+**: WebSocket is built-in, no flags needed
- **Node 20-21**: Requires `--experimental-websocket` flag
- **Node < 20**: Not supported for subscription features

Scripts that use WebSocket: `create_invoice.js`, `create_invoice_usd.js`, `subscribe_invoice.js`, `subscribe_updates.js`.
