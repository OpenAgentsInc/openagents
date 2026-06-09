# Blink API And Auth

Use this reference for Blink API configuration, authentication, and environment setup.

## Source Of Truth

- Blink API docs: https://dev.blink.sv
- Blink Dashboard: https://dashboard.blink.sv
- Blink GraphQL schema: https://api.blink.sv/graphql (introspection)
- Skill scripts: `blink/scripts/*.js`

## API Endpoints

| Environment | GraphQL Endpoint | WebSocket Endpoint | Purpose |
|-------------|-----------------|--------------------|--------|
| Production | `https://api.blink.sv/graphql` | `wss://ws.blink.sv/graphql` | Live wallets, real sats |
| Staging | `https://api.staging.blink.sv/graphql` | `wss://ws.staging.blink.sv/graphql` | Signet testnet, free test sats |

The endpoint is controlled by the `BLINK_API_URL` environment variable. If unset, production is used.

```bash
# Production (default)
export BLINK_API_KEY="blink_..."

# Staging / signet testnet
export BLINK_API_URL="https://api.staging.blink.sv/graphql"
export BLINK_API_KEY="blink_..."
```

WebSocket URLs are derived automatically by scripts: replace `api` with `ws` and `https` with `wss`.

## Authentication

All authenticated requests use the `X-API-KEY` header:

```
X-API-KEY: blink_...
```

API keys are created in the Blink Dashboard under API Keys. Each key has one or more scopes.

## API Key Scopes

| Scope | Allows | Scripts That Require It |
|-------|--------|------------------------|
| **Read** | Query balances, transactions, invoice status, account info, price | `balance.js`, `check_invoice.js`, `transactions.js`, `account_info.js`, `fee_probe.js` |
| **Receive** | Create invoices (BTC and USD) | `create_invoice.js`, `create_invoice_usd.js` |
| **Write** | Send payments (invoice, LN address, LNURL) | `pay_invoice.js`, `pay_lnaddress.js`, `pay_lnurl.js` |

**Public endpoints** (no API key required): `price.js` for exchange rates, price history, and currency list.

Use the minimum scope needed:
- Balance monitoring agent: Read only
- Invoice-receiving agent: Read + Receive
- Full payment agent: Read + Receive + Write

## Wallet Types

Every Blink account has two wallets:

| Wallet | Currency | Balance Unit | ID Format |
|--------|----------|--------------|----------|
| BTC | Bitcoin | satoshis | UUID string |
| USD | Stablesats (USD-pegged) | cents | UUID string |

Wallet IDs are resolved automatically by scripts via the `query me { defaultAccount { wallets { id walletCurrency } } }` query.

## Error Handling

Blink API errors are returned in the GraphQL `errors` array. Common patterns:

| Error Code | Meaning | Action |
|------------|---------|--------|
| `INSUFFICIENT_BALANCE` | Not enough funds | Check balance first |
| `INVOICE_ALREADY_PAID` | Duplicate payment attempt | Check status, no retry needed |
| `ROUTE_FINDING_ERROR` | No path to destination | Fee probe will also fail; may need smaller amount |
| `INVOICE_EXPIRED` | Invoice TTL exceeded | Request a new invoice |
| Authentication error | Invalid or missing API key | Verify `BLINK_API_KEY` is set and has correct scopes |

All scripts exit with code 0 on success and code 1 on failure. Errors are written to stderr; structured JSON goes to stdout.

## Rate Limits

The Blink API enforces per-key rate limits. For agent automation:
- Space out rapid-fire queries (balance polling, transaction listing)
- Use WebSocket subscriptions instead of polling where possible
- Fee probes count toward rate limits

## Staging Environment Setup

1. Create a staging account at the Blink Dashboard (staging mode)
2. Get signet test sats from the Blink faucet or signet faucet
3. Set both environment variables:

```bash
export BLINK_API_URL="https://api.staging.blink.sv/graphql"
export BLINK_API_KEY="blink_..."
```

4. Verify with `balance.js` before running payment scripts

Staging uses Bitcoin signet, not mainnet. Transactions are free and reversible.
