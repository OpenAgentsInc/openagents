# Pay per request

You can call the API with no account and no key: pay for each request in
bitcoin over Lightning. This uses [x402](https://github.com/x402-foundation/x402),
an open standard for paying for HTTP requests.

**Beta.**

## How it works

1. Send the request with no `Authorization` header.
2. The answer is `402 Payment Required`. Its body says the price, and its
   `PAYMENT-REQUIRED` header holds a Lightning invoice for exactly that
   request.
3. Pay the invoice with any Lightning wallet. The wallet gives you a
   proof of payment (the preimage).
4. Send the same request again, byte for byte, with the proof in a
   `PAYMENT-SIGNATURE` header. You get the answer, with a
   `PAYMENT-RESPONSE` header saying the payment was accepted.

```sh
curl -i https://api.openagents.com/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"model": "google/gemini-3.8-flash", "input": "Say hello.", "max_output_tokens": 200}'
```

```http
HTTP/1.1 402 Payment Required
PAYMENT-REQUIRED: eyJ4NDAyVmVyc2lvbiI6Mi...

{"error": {"type": "payment_required", "message": "This request costs up to 3 sats ($0.0021). ..."},
 "price_sats": 3, "price_usd": "0.0021", "x402Version": 2}
```

The `PAYMENT-REQUIRED` header is base64 JSON. Its `accepts[0]` is the
payment terms; `accepts[0].extra.invoice` is the invoice. To send the
proof, base64-encode
`{"x402Version": 2, "accepted": <accepts[0]>, "payload": {"preimage": "<hex>"}}`
as the `PAYMENT-SIGNATURE` header.

## From the command line

The `openagents` command pays from your OpenAgents wallet and does all
four steps:

```sh
openagents inference google/gemini-3.8-flash "Say hello." --pay x402 --max-msat 10000
```

`--max-msat` is the most you will pay, in millisatoshis (10000 is 10
sats). It refuses an invoice above it.

## What you pay

- **The most the request could cost.** We price the request before it
  runs: the model's price from the [rate card](/docs/api/models), plus our
  margin, for the most input your request can hold and the most output it
  allows. Set `max_output_tokens` to lower the price.
- **Whole sats, rounded up**, at the bitcoin price the rate card shows.
- **No refund of the unused part.** The answer's actual cost is in its
  `x-openagents-cost-usd` header, but the payment covers the request as
  quoted. To pay only what each answer costs, use a key with credit
  instead.
- **No answer, no charge.** If every provider fails before answering, or
  no provider can take the request, send the same request with the same
  `PAYMENT-SIGNATURE` again: the payment still counts.
- **One payment, one answer.** A proof is used once, and only for the
  request it was issued for. A different body needs a new invoice.

## What needs a key

A paid request covers one model turn and keeps nothing. Stored responses
(`store`, `previous_response_id`), hosted tools such as web search, and
the WebSocket need an API key.

## The API's description

Every route is described in OpenAPI 3.1 at
`https://api.openagents.com/v1/openapi.json`.
